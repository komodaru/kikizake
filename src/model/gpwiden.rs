//! GDI+ wide pens: GpPath::GetWidenedPathInternal, the widener (GpPathWidener: offsets, the
//! miter/bevel join kernel, flat, square, triangle and round caps, closed subpaths, thin pens
//! widened in device space) and dashes (CreateDashedPath, getDashData), from the clean-room
//! notes; every float op in the arch's rounding (x64 down, x86 nearest). Used by GdipWidenPath
//! and by every stroke past the one-pixel test.
use super::gdiplus::{Matrix, Round};
use super::gplinear::invert;
use super::gppath::{apply, axes, sqrt32, Kind, Path, Pen, Point, EPS, NEAR};
use super::{Fail, Res};
use crate::spec::{clause, hit, open};

type P = (f32, f32);

/// a line cap the widener draws
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap {
    Flat,
    Square,
    Round,
    Triangle,
}
impl Cap {
    /// a pen's start or end cap: 1, 2, 3 their own, anchors (0x10 ..= 0x14) and custom (0xff)
    /// through GpEndCapCreator (the spec's open GP.PEN), any other value flat (MEASURED on
    /// GDI+ itself, both arches: 5, 15, -1 widen as 0)
    pub fn line(v: i64) -> Res<Cap> {
        match v {
            1 => Ok(Cap::Square),
            2 => Ok(Cap::Round),
            3 => Ok(Cap::Triangle),
            0x10..=0x14 | 0xff => Err(Fail::Unmodeled(open::GP_PEN)),
            _ => Ok(Cap::Flat),
        }
    }
    /// a pen's dash cap: round and triangle their own, any other value flat (MEASURED on GDI+
    /// itself: 0, 1, 4, -1 dash as flat)
    pub fn dash(v: i64) -> Cap {
        match v {
            2 => Cap::Round,
            3 => Cap::Triangle,
            _ => Cap::Flat,
        }
    }
}

/// a pen's line join
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Join {
    Miter,
    Bevel,
    MiterClipped,
    /// a value past 0 ..= 3 (GdipSetPenLineJoin takes any): no point at an open subpath's
    /// interior vertex, no outline of a closed one
    Nothing,
}
impl Join {
    /// the round join (2) is getRoundJoin's arcs: the spec's open GP.PEN
    pub fn of(v: i64) -> Res<Join> {
        match v {
            0 => Ok(Join::Miter),
            1 => Ok(Join::Bevel),
            3 => Ok(Join::MiterClipped),
            2 => Err(Fail::Unmodeled(open::GP_PEN)),
            _ => Ok(Join::Nothing),
        }
    }
}

/// the dash pattern of a dash style in pen widths; Solid, Custom with no array and the values
/// past it draw solid
pub fn pattern(style: i64) -> Option<&'static [f32]> {
    match style {
        1 => Some(&[3.0, 1.0]),
        2 => Some(&[1.0, 1.0]),
        3 => Some(&[3.0, 1.0, 1.0, 1.0]),
        4 => Some(&[3.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        _ => None,
    }
}

/// a segment's unit gradient and its normal
#[derive(Clone, Copy, Debug)]
struct Seg {
    g: P,
    n: P,
}

/// what one subpath's outline is made of
struct Widener {
    r: Round,
    /// the pen width (a thin pen's in device pixels)
    w: f32,
    wl: f32,
    wr: f32,
    lim: f32,
    join: Join,
}

/// an outline point
#[derive(Clone, Copy, Debug)]
struct Out {
    p: P,
    kind: Kind,
}
fn line(p: P) -> Out {
    Out { p, kind: Kind::Line }
}

impl Widener {
    fn seg(&self, p: P, q: P) -> Seg {
        let r = self.r;
        let (dx, dy) = (r.sub(q.0, p.0), r.sub(q.1, p.1));
        let l = sqrt32(r, r.add(r.mul(dy, dy), r.mul(dx, dx)));
        let g = (r.div(dx, l), r.div(dy, l));
        Seg { g, n: (g.1, -g.0) }
    }
    /// P offset by w along n
    fn off(&self, w: f32, n: P, p: P) -> P {
        let r = self.r;
        (r.add(r.mul(w, n.0), p.0), r.add(r.mul(w, n.1), p.1))
    }
    /// exceeded(g0, g1) of MiterClipped
    fn exceeded(&self, g0: P, g1: P) -> bool {
        let r = self.r;
        let c = r.sub(r.mul(g0.0, g1.1), r.mul(g1.0, g0.1));
        if c.abs() <= EPS {
            return true;
        }
        let d2 = r.add(sq(r, r.sub(g0.1, g1.1)), sq(r, r.sub(g0.0, g1.0)));
        r.sub(d2, r.mul(r.mul(c, c), self.lim)) > 0.0
    }
    /// getMiterBevelJoin: the left and right side's points at vertex p
    fn kernel(&self, p: P, s0: Seg, s1: Seg, use_miter: bool) -> (Vec<P>, Vec<P>) {
        let r = self.r;
        let (g0, g1, n0, n1) = (s0.g, s1.g, s0.n, s1.n);
        let tiny = |g: P| g.0.abs() < EPS && g.1.abs() < EPS;
        let near = r.add(sq(r, r.sub(g0.0, g1.0)), sq(r, r.sub(g0.1, g1.1))) < f32::from_bits(0x3727_c5ac);
        if tiny(g0) || tiny(g1) || near {
            let v = if n0 == (0.0, 0.0) { n1 } else { n0 };
            return (vec![self.off(self.wl, v, p)], vec![self.off(self.wr, v, p)]);
        }
        let mut cross = r.sub(r.mul(g1.1, g0.0), r.mul(g0.1, g1.0));
        if cross.abs() < EPS {
            cross = 0.0;
        }
        let (inner_l, inner_r) = if cross > 0.0 { (self.wl < 0.0, self.wr < 0.0) } else { (self.wl > 0.0, self.wr > 0.0) };
        let use_miter = use_miter && self.lim > 1.0;
        let (mut t, mut miter) = ([(0.0f32, 0.0f32); 2], None);
        let (mut mit_l, mut mit_r) = (false, false);
        if use_miter && cross != 0.0 {
            let (dx, dy) = (r.sub(g0.0, g1.0), r.sub(g0.1, g1.1));
            let q = r.sub(r.add(r.mul(dy, dy), r.mul(dx, dx)), r.mul(r.mul(cross, cross), self.lim));
            if q <= 0.0 {
                miter = Some((r.div(dx, cross), r.div(dy, cross)));
                (mit_l, mit_r) = (cross > 0.0, cross <= 0.0 && !inner_r);
            } else {
                let tt = sqrt32(r, r.add(r.mul(0.0, 0.0), r.sub(self.lim, 1.0)));
                for (k, (inner, w)) in [(inner_l, self.wl), (inner_r, self.wr)].into_iter().enumerate() {
                    if !inner {
                        let s = if cross > 0.0 { w } else { -w };
                        t[k] = (r.mul(r.sub(tt, 0.0), s), r.mul(r.add(tt, 0.0), s));
                    }
                }
            }
        }
        let side = |w: f32, mit: bool, (t0, t1): (f32, f32)| -> Vec<P> {
            match (mit, miter) {
                (true, Some(m)) => vec![(r.add(r.mul(m.0, w), p.0), r.add(r.mul(m.1, w), p.1))],
                (true, None) | (false, _) => {
                    let a0 = self.off(w, n0, p);
                    let a = (r.add(a0.0, r.mul(t0, g0.0)), r.add(a0.1, r.mul(t0, g0.1)));
                    let b0 = self.off(w, n1, p);
                    let b = (r.sub(b0.0, r.mul(t1, g1.0)), r.sub(b0.1, r.mul(t1, g1.1)));
                    if r.add(r.sub(b.0, a.0).abs(), r.sub(b.1, a.1).abs()) > NEAR { vec![a, b] } else { vec![a] }
                }
            }
        };
        (side(self.wl, mit_l, t[0]), side(self.wr, mit_r, t[1]))
    }
    /// the join at an interior vertex (WidenLinePoints)
    fn join(&self, p: P, s0: Seg, s1: Seg) -> (Vec<P>, Vec<P>) {
        match self.join {
            Join::Miter => self.kernel(p, s0, s1, true),
            Join::Bevel => self.kernel(p, s0, s1, false),
            Join::MiterClipped => self.kernel(p, s0, s1, !self.exceeded(s0.g, s1.g)),
            Join::Nothing => (vec![], vec![]),
        }
    }
    /// SetTriangleCap's tip
    fn tip(&self, p: P, g: P, start: bool) -> P {
        let r = self.r;
        let d = if start { (-g.0, -g.1) } else { g };
        let a = (r.add(r.mul(g.1, self.wr), p.0), r.add(r.mul(-g.0, self.wr), p.1));
        let b = (r.add(r.mul(g.1, self.wl), p.0), r.add(r.mul(-g.0, self.wl), p.1));
        let k = r.sub(self.wl, self.wr).abs();
        (r.mul(r.add(r.add(a.0, b.0), r.mul(k, d.0)), 0.5), r.mul(r.add(r.add(a.1, b.1), r.mul(k, d.1)), 0.5))
    }
    /// SetRoundCap's seven points: a line point, two quarter circles
    fn round(&self, p: P, g: P, start: bool) -> Vec<Out> {
        let r = self.r;
        let d = if start { (-g.0, -g.1) } else { g };
        let rr = r.mul(r.sub(self.wl, self.wr), 0.5);
        let s = r.add(self.wl, self.wr);
        let c = (r.add(r.mul(r.mul(s, g.1), 0.5), p.0), r.sub(p.1, r.mul(r.mul(s, g.0), 0.5)));
        let (a, b) = (r.mul(rr, d.1), r.mul(-rr, d.0));
        const K: f32 = 0.552_284_8;
        const _: () = assert!(K.to_bits() == 0x3f0d_6289);
        let units = [(1.0, 0.0), (1.0, K), (K, 1.0), (0.0, 1.0), (-K, 1.0), (-1.0, K), (-1.0, 0.0)];
        units
            .iter()
            .enumerate()
            .map(|(i, &(u, v))| Out { p: (r.add(r.sub(r.mul(u, a), r.mul(v, b)), c.0), r.add(r.add(r.mul(v, a), r.mul(u, b)), c.1)), kind: if i == 0 { Kind::Line } else { Kind::Bezier } })
            .collect()
    }
    /// an open subpath's outline (WidenSubpath, CombineSubpathOutlines): one closed figure
    fn open(&self, p: &[P], caps: (Cap, Cap)) -> Vec<Out> {
        let r = self.r;
        let k = p.len();
        let segs: Vec<Seg> = p.windows(2).map(|q| self.seg(q[0], q[1])).collect();
        let e = r.mul(self.w, -0.5);
        let (first, last) = (segs[0], segs[k - 2]);
        let end_point = |w: f32, start: bool| {
            let (at, s, cap) = if start { (p[0], first, caps.0) } else { (p[k - 1], last, caps.1) };
            let o = self.off(w, s.n, at);
            match (cap, start) {
                (Cap::Square, true) => (r.add(o.0, r.mul(e, s.g.0)), r.add(o.1, r.mul(e, s.g.1))),
                (Cap::Square, false) => (r.sub(o.0, r.mul(e, s.g.0)), r.sub(o.1, r.mul(e, s.g.1))),
                (Cap::Flat | Cap::Round | Cap::Triangle, _) => o,
            }
        };
        let (mut left, mut right) = (vec![end_point(self.wl, true)], vec![end_point(self.wr, true)]);
        for i in 1..k - 1 {
            let (l, rt) = self.join(p[i], segs[i - 1], segs[i]);
            left.extend(l);
            right.extend(rt);
        }
        left.push(end_point(self.wl, false));
        right.push(end_point(self.wr, false));
        let mut out: Vec<Out> = left.iter().map(|&q| line(q)).collect();
        out[0].kind = Kind::Start;
        let mut back: Vec<P> = right.into_iter().rev().collect();
        match caps.1 {
            Cap::Triangle => out.push(line(self.tip(p[k - 1], last.g, false))),
            Cap::Round => {
                out.pop();
                out.extend(self.round(p[k - 1], last.g, false));
                back.remove(0);
            }
            Cap::Flat | Cap::Square => {}
        }
        if caps.0 == Cap::Round {
            back.pop();
        }
        out.extend(back.into_iter().map(line));
        match caps.0 {
            Cap::Triangle => {
                out.push(line(self.tip(p[0], first.g, true)));
                out.push(line(left[0]));
            }
            Cap::Round => out.extend(self.round(p[0], first.g, true)),
            Cap::Flat | Cap::Square => {}
        }
        out
    }
    /// a closed subpath's two figures; None for a join of no points
    fn closed(&self, p: &[P]) -> Option<[Vec<Out>; 2]> {
        if self.join == Join::Nothing {
            return None;
        }
        let k = p.len();
        let segs: Vec<Seg> = (0..k).map(|i| self.seg(p[i], p[(i + 1) % k])).collect();
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for i in 0..k {
            let (l, rt) = self.join(p[i], segs[(i + k - 1) % k], segs[i]);
            left.extend(l);
            right.extend(rt);
        }
        right.reverse();
        let fig = |v: Vec<P>| -> Vec<Out> {
            let mut o: Vec<Out> = v.into_iter().map(line).collect();
            if let Some(f) = o.first_mut() {
                f.kind = Kind::Start;
            }
            o
        };
        Some([fig(left), fig(right)])
    }
}

fn sq(r: Round, v: f32) -> f32 {
    r.mul(v, v)
}

/// consecutive points the same within 2^-23 dropped (GpPathWidener::Initialize)
fn dedup(p: &[P]) -> Vec<P> {
    let mut v: Vec<P> = Vec::with_capacity(p.len());
    for &q in p {
        if !v.last().is_some_and(|l: &P| (q.0 - l.0).abs() < EPS && (q.1 - l.1).abs() < EPS) {
            v.push(q);
        }
    }
    v
}

/// a subpath to widen: its points, closed, and for an open one which cap each end takes
struct Sub {
    pts: Vec<P>,
    closed: bool,
    caps: (Cap, Cap),
}

/// why GetWidenedPathInternal gives nothing back
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Widened {
    /// the matrix has no inverse
    NoInverse,
    /// a closed subpath of a join of no points: GdipWidenPath leaves the path as it was
    Unchanged,
}

/// GetWidenedPathInternal: path widened by pen through m (the world to device matrix, or
/// GdipWidenPath's), flattened at flatness; the outline in m's device space
pub fn widen(r: Round, path: &Path, pen: &Pen, m: &Matrix, flatness: f32) -> Res<Result<Path, Widened>> {
    hit(clause::GP_PEN_WIDE);
    let Some(minv) = invert(r, m) else { return Ok(Err(Widened::NoInverse)) };
    let (start, end) = (Cap::line(pen.start_cap)?, Cap::line(pen.end_cap)?);
    let join = Join::of(pen.join)?;
    // (PenAlignment past Center, compound pens: the spec's open GP.PEN)
    if pen.align != 0 {
        return Err(Fail::Unmodeled(open::GP_PEN));
    }
    let mut c = path.clone();
    c.flatten(r, Some(m), flatness)?;
    for q in &mut c.pts {
        (q.x, q.y) = apply(r, &minv, q.x, q.y);
    }
    let figs: Vec<(Vec<P>, bool)> = c.figures().iter().map(|f| (f.iter().map(|q| (q.x, q.y)).collect(), f.last().is_some_and(|q| q.close))).collect();
    let subs: Vec<Sub> = match pattern(pen.dash) {
        Some(pat) => dashes(r, &figs, pen, axes(r, m).0, true, pat, (start, end)),
        None => figs.into_iter().map(|(pts, closed)| Sub { pts, closed, caps: (start, end) }).collect(),
    };
    let (_, minor) = axes(r, m);
    let thin_width = f32::from_bits(0x3f80_0008);
    let thin = thin_width > r.mul(pen.width, minor);
    let w = if thin { thin_width } else { pen.width };
    let wl = r.mul(w, 0.5);
    let wd = Widener { r, w, wl, wr: r.sub(wl, w), lim: sq(r, pen.miter.max(1.0)), join };
    let mut out = Vec::new();
    for s in subs {
        let pts: Vec<P> = if thin { s.pts.iter().map(|&(x, y)| apply(r, m, x, y)).collect() } else { s.pts };
        let mut pts = dedup(&pts);
        // (a closed figure ending on its start: that point once, golden gdipluspath
        // pathwiden-11, an ellipse)
        if s.closed && pts.len() > 2 && pts.first() == pts.last() {
            pts.pop();
        }
        if pts.len() < 2 {
            continue;
        }
        let figs = if s.closed {
            match wd.closed(&pts) {
                Some(f) => f.to_vec(),
                None => return Ok(Err(Widened::Unchanged)),
            }
        } else {
            vec![wd.open(&pts, s.caps)]
        };
        for f in figs {
            let n = f.len();
            out.extend(f.into_iter().enumerate().map(|(i, o)| Point { x: o.p.0, y: o.p.1, kind: o.kind, close: i + 1 == n }));
        }
    }
    if !thin {
        for q in &mut out {
            (q.x, q.y) = apply(r, m, q.x, q.y);
        }
    }
    Ok(Ok(Path::of(path.fill, out)))
}

/// GpModF: x mod m truncating, a negative x from the top
fn modf(r: Round, x: f32, m: f32) -> f32 {
    let part = |y: f32| r.sub(y, r.mul(trunc(r.div(y, m)), m));
    if x > 0.0 {
        part(x)
    } else if x < 0.0 {
        let v = part(-x);
        if v > 0.0 { r.sub(m, v) } else { v }
    } else {
        0.0
    }
}
/// cvttss2si: toward zero, out of range the integer indefinite
fn trunc(v: f32) -> f32 {
    if v.is_nan() || v >= 2_147_483_648.0 || v < -2_147_483_648.0 { -2_147_483_648.0 } else { v.trunc() }
}

/// a dash's ends: whether each takes the dash cap (type bit 0x10)
struct Dash {
    pts: Vec<P>,
    dash_ends: (bool, bool),
}

/// a path's dashes in world space (CreateDashedPath of no matrix and no cap adjustment, the
/// aliased one-pixel route): open figures; None for a pen of no dash pattern
pub fn dashed(r: Round, path: &Path, pen: &Pen) -> Res<Option<Path>> {
    let Some(pat) = pattern(pen.dash) else { return Ok(None) };
    let mut c = path.clone();
    c.flatten(r, None, 0.25)?;
    let figs: Vec<(Vec<P>, bool)> = c.figures().iter().map(|f| (f.iter().map(|q| (q.x, q.y)).collect(), f.last().is_some_and(|q| q.close))).collect();
    let mut out = Vec::new();
    for s in dashes(r, &figs, pen, 1.0, false, pat, (Cap::Flat, Cap::Flat)) {
        out.extend(s.pts.iter().enumerate().map(|(i, &(x, y))| Point { x, y, kind: if i == 0 { Kind::Start } else { Kind::Line }, close: false }));
    }
    Ok(Some(Path::of(path.fill, out)))
}

/// CreateDashedPath: each figure cut into dashes (getDashData), each dash an open subpath with
/// its ends' caps; the dash cap adjusting the pattern when adjust
#[allow(clippy::too_many_arguments)]
fn dashes(r: Round, figs: &[(Vec<P>, bool)], pen: &Pen, major: f32, adjust: bool, pat: &[f32], caps: (Cap, Cap)) -> Vec<Sub> {
    hit(clause::GP_PEN_DASH);
    let min_w = 1.0f32;
    let u = if min_w > r.mul(major, pen.width) { r.div(min_w, major) } else { pen.width };
    let dash_cap = Cap::dash(pen.dash_cap);
    let mut d: Vec<f32> = pat.to_vec();
    if d.len() % 2 == 1 {
        d.push(0.0);
    }
    for v in &mut d {
        *v = r.mul(*v, u);
    }
    let ins = match (adjust, dash_cap) {
        (true, Cap::Round | Cap::Triangle) => r.mul(u, 0.5),
        (true, Cap::Flat | Cap::Square) | (false, _) => 0.0,
    };
    let k = r.add(ins, ins);
    if k > 0.0 {
        let floor = r.mul(u, f32::from_bits(0x3a83_126f));
        for (i, v) in d.iter_mut().enumerate() {
            if i % 2 == 0 {
                *v = r.sub(*v, k);
                if floor > *v {
                    *v = floor;
                }
            } else {
                *v = r.add(*v, k);
            }
        }
    }
    let mut out = Vec::new();
    for (p, closed) in figs {
        let c = if *closed && ins > 0.0 { k } else { 0.0 };
        let off = r.sub(r.mul(pen.width, 0.0), c);
        for dash in walk(r, p, *closed, &d, off, pen.width) {
            let pick = |dashed: bool, line: Cap| if dashed { dash_cap } else { line };
            out.push(Sub { pts: dash.pts, closed: false, caps: (pick(dash.dash_ends.0, caps.0), pick(dash.dash_ends.1, caps.1)) });
        }
    }
    out
}

/// getDashData for one subpath
fn walk(r: Round, p: &[P], closed: bool, d: &[f32], off: f32, width: f32) -> Vec<Dash> {
    let mut n = p.len();
    if n < 2 {
        return Vec::new();
    }
    let len = |a: P, b: P| {
        let (dx, dy) = (r.sub(b.0, a.0), r.sub(b.1, a.1));
        sqrt32(r, r.add(r.mul(dy, dy), r.mul(dx, dx)))
    };
    let mut l: Vec<f32> = (0..n).map(|i| if i == 0 { len(p[n - 1], p[0]) } else { len(p[i - 1], p[i]) }).collect();
    if closed && p[0] == p[n - 1] {
        n -= 1;
        l.truncate(n);
    }
    let m = d.len();
    let s = d.iter().fold(0.0f32, |a, &v| r.add(a, v));
    let mut rem = modf(r, off, s);
    let mut i = 0;
    while i < m {
        if d[i] > rem {
            rem = r.sub(d[i], rem);
            break;
        }
        rem = r.sub(rem, d[i]);
        i += 1;
    }
    let mut idx = i;
    let mut on = idx % 2 == 0;
    idx = (idx + 1) % m;
    let mut nxt = d[idx];
    let merge = |idx: &mut usize, rem: &mut f32, nxt: &mut f32, on: bool| {
        if *nxt == 0.0 && on && *idx == m - 1 {
            *idx = (*idx + 1) % m;
            *rem = r.add(*rem, d[*idx]);
            *idx = (*idx + 1) % m;
            *nxt = d[*idx];
        }
    };
    merge(&mut idx, &mut rem, &mut nxt, on);
    let (mut prev, mut seg, mut j, mut li) = if closed { (p[n - 1], norm(r, (r.sub(p[0].0, p[n - 1].0), r.sub(p[0].1, p[n - 1].1))), 0, 0) } else { (p[0], l[1], 1, 1) };
    let mut open = false;
    let mut dashes: Vec<Vec<P>> = Vec::new();
    let near = |a: P, b: P| (a.0 - b.0).abs() < EPS && (a.1 - b.1).abs() < EPS;
    let emit = |dashes: &mut Vec<Vec<P>>, open: bool, a: P, b: P| {
        if !open {
            dashes.push(vec![a]);
        }
        if let Some(last) = dashes.last_mut() {
            last.push(b);
        }
    };
    while j < n {
        if rem > seg {
            rem = r.sub(rem, seg);
            let q = p[j];
            if on {
                if near(prev, q) {
                    open = false;
                } else {
                    emit(&mut dashes, open, prev, q);
                    open = true;
                }
            }
            prev = q;
            li = if li + 1 >= n { 0 } else { li + 1 };
            j += 1;
            seg = l[li];
        } else {
            let t = r.div(rem, seg);
            let q = (r.add(r.mul(r.sub(p[j].0, prev.0), t), prev.0), r.add(r.mul(r.sub(p[j].1, prev.1), t), prev.1));
            seg = r.sub(seg, rem);
            if on && !near(prev, q) {
                emit(&mut dashes, open, prev, q);
            }
            prev = q;
            rem = nxt;
            on = idx % 2 == 0;
            idx = (idx + 1) % m;
            nxt = d[idx];
            merge(&mut idx, &mut rem, &mut nxt, on);
            open = false;
        }
    }
    let mut out: Vec<Dash> = dashes.into_iter().map(|pts| Dash { pts, dash_ends: (true, true) }).collect();
    // (an open subpath's dash within half a pen width of its start or end takes the line cap)
    if !closed {
        let ww = r.mul(r.mul(width, width), 0.25);
        let d2 = |a: P, b: P| r.add(sq(r, r.sub(a.0, b.0)), sq(r, r.sub(a.1, b.1)));
        if let Some(last) = out.last_mut()
            && let Some(&q) = last.pts.last()
            && ww > d2(p[n - 1], q)
        {
            last.dash_ends.1 = false;
        }
        if let Some(first) = out.first_mut()
            && ww > d2(p[0], first.pts[0])
        {
            first.dash_ends.0 = false;
        }
    }
    out
}

/// a vector's length in double, each op rounded, below 2^-23 none
fn norm(r: Round, (x, y): P) -> f32 {
    use super::gppath::{add64, mul64, to32};
    let (x, y) = (f64::from(x), f64::from(y));
    let s = add64(r, mul64(r, x, x), mul64(r, y, y));
    let v = sqrt64(r, s);
    if v < f64::from(EPS) { 0.0 } else { to32(r, v) }
}
/// sqrt in double in the rounding mode
fn sqrt64(r: Round, a: f64) -> f64 {
    let v = a.sqrt();
    match r {
        Round::Nearest => v,
        Round::Down => {
            if v.mul_add(v, -a) > 0.0 { f64::from_bits(v.to_bits() - 1) } else { v }
        }
    }
}
