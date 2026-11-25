//! GDI+ wide pens: GpPath::GetWidenedPathInternal, the widener (GpPathWidener: offsets, the
//! miter/bevel join kernel, the round join, flat, square, triangle and round caps, anchor caps
//! (GpEndCapCreator), closed subpaths, thin pens
//! widened in device space) and dashes (CreateDashedPath, getDashData), from the clean-room
//! notes; every float op in the arch's rounding (x64 down, x86 nearest). Used by GdipWidenPath
//! and by every stroke past the one-pixel test.
use super::gdiplus::{Matrix, Round};
use super::gplinear::invert;
use super::gppath::{CustomCap, add64, apply, axes, div64, mul64, sqrt32, to32, Kind, Path, Pen, Point, EPS, NEAR};
use super::{Fail, Res};
use crate::spec::{clause, hit, open};

type P = (f32, f32);

/// an anchor cap (GpEndCapCreator's reference caps)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Anchor {
    Square,
    Round,
    Diamond,
    Arrow,
}
/// a line cap the widener draws
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cap {
    Flat,
    Square,
    Round,
    Triangle,
    /// a cap figure of its own; the body's end flat
    Anchor(Anchor),
}
impl Cap {
    /// a pen's start or end cap: 1, 2, 3 their own, 0x11 ..= 0x14 the anchors, any other value
    /// flat (MEASURED on GDI+ itself, both arches: 4 ..= 15, 0x10, 0xf0, 0xff with no custom
    /// cap object, 21 ..= 255 and -1 widen as 0; no LineCap value reaches DoubleRoundCap)
    pub fn line(v: i64) -> Cap {
        match v {
            1 => Cap::Square,
            2 => Cap::Round,
            3 => Cap::Triangle,
            0x11 => Cap::Anchor(Anchor::Square),
            0x12 => Cap::Anchor(Anchor::Round),
            0x13 => Cap::Anchor(Anchor::Diamond),
            0x14 => Cap::Anchor(Anchor::Arrow),
            _ => Cap::Flat,
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
    Round,
    MiterClipped,
    /// a value past 0 ..= 3 (GdipSetPenLineJoin takes any): on x64 no point at an open
    /// subpath's interior vertex and a closed one fails the widening; on x86 every widening
    /// fails (golden penjoin-4, penjoin--1)
    Nothing,
}
impl Join {
    pub fn of(v: i64) -> Join {
        match v {
            0 => Join::Miter,
            1 => Join::Bevel,
            2 => Join::Round,
            3 => Join::MiterClipped,
            _ => Join::Nothing,
        }
    }
}

/// which of getSmallRoundJoin's lists to make
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Make {
    /// the outer side's arc and the inner side's points
    Both,
    /// the arcs only
    Arcs,
    /// the inner points only
    Inner,
}
impl Make {
    fn arcs(self) -> bool {
        match self {
            Make::Both | Make::Arcs => true,
            Make::Inner => false,
        }
    }
    fn inner(self) -> bool {
        match self {
            Make::Both | Make::Inner => true,
            Make::Arcs => false,
        }
    }
}

/// the dash pattern of a dash style in pen widths; Solid, Custom with no array and the values
/// past it draw solid
pub fn preset(style: i64) -> Option<&'static [f32]> {
    match style {
        1 => Some(&[3.0, 1.0]),
        2 => Some(&[1.0, 1.0]),
        3 => Some(&[3.0, 1.0, 1.0, 1.0]),
        4 => Some(&[3.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
        _ => None,
    }
}
/// the dash array a pen strokes by: a style's own, Custom (5) the array set (none when empty),
/// any other style none (geometry.md 6a)
pub fn pattern(pen: &Pen) -> Option<&[f32]> {
    match (preset(pen.dash), pen.dash) {
        (Some(p), _) => Some(p),
        (None, 5) if !pen.dash_array.is_empty() => Some(&pen.dash_array),
        (None, _) => None,
    }
}

/// a segment's unit gradient and its normal
#[derive(Clone, Copy, Debug)]
struct Seg {
    g: P,
    n: P,
}

/// how the widener's joins treat a turn's inner side (GpPathWidener's compound switch)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// a plain pen or a compound array of two: the inner side bevelled
    Plain,
    /// a compound array of four or more: a miter within the limit on both sides
    Compound,
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
    mode: Mode,
}

/// an outline point
#[derive(Clone, Copy, Debug)]
struct Out {
    p: P,
    kind: Kind,
    /// a point of a join's inner side (0x40)
    inner: bool,
}
fn line(p: P) -> Out {
    Out { p, kind: Kind::Line, inner: false }
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
    /// getMiterBevelJoin: the left and right side's points at vertex p, each with whether it is
    /// the inner side keeping its two bevel points (0x40, geometry.md 21g)
    fn kernel(&self, p: P, s0: Seg, s1: Seg, use_miter: bool) -> ((Vec<P>, bool), (Vec<P>, bool)) {
        let r = self.r;
        let (g0, g1, n0, n1) = (s0.g, s1.g, s0.n, s1.n);
        let tiny = |g: P| g.0.abs() < EPS && g.1.abs() < EPS;
        let near = r.add(sq(r, r.sub(g0.0, g1.0)), sq(r, r.sub(g0.1, g1.1))) < f32::from_bits(0x3727_c5ac);
        if tiny(g0) || tiny(g1) || near {
            let v = if n0 == (0.0, 0.0) { n1 } else { n0 };
            return ((vec![self.off(self.wl, v, p)], false), (vec![self.off(self.wr, v, p)], false));
        }
        let mut cross = r.sub(r.mul(g1.1, g0.0), r.mul(g0.1, g1.0));
        // (kept only past 2^-23: geometry.md 21h)
        if cross.abs() <= EPS {
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
                (mit_l, mit_r) = match self.mode {
                    Mode::Plain => (cross > 0.0, cross <= 0.0 && !inner_r),
                    Mode::Compound => (true, true),
                };
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
        let (l, rt) = (side(self.wl, mit_l, t[0]), side(self.wr, mit_r, t[1]));
        let (fl, fr) = (inner_l && !(mit_l && miter.is_some()) && l.len() == 2, inner_r && !(mit_r && miter.is_some()) && rt.len() == 2);
        ((l, fl), (rt, fr))
    }
    /// the join at an interior vertex (WidenLinePoints): the left and the right side's points
    fn join(&self, p: P, s0: Seg, s1: Seg) -> (Vec<Out>, Vec<Out>) {
        let marked = |(v, inner): (Vec<P>, bool)| -> Vec<Out> { v.into_iter().map(|p| Out { inner, ..line(p) }).collect() };
        let lines = |(l, rt): ((Vec<P>, bool), (Vec<P>, bool))| -> (Vec<Out>, Vec<Out>) { (marked(l), marked(rt)) };
        match self.join {
            Join::Miter => lines(self.kernel(p, s0, s1, true)),
            Join::Bevel => lines(self.kernel(p, s0, s1, false)),
            Join::MiterClipped => lines(self.kernel(p, s0, s1, !self.exceeded(s0.g, s1.g))),
            Join::Round => {
                let [l, rt] = self.round_join(p, s0, s1);
                (l, rt)
            }
            Join::Nothing => (vec![], vec![]),
        }
    }
    /// getTurningDirection's cross product: 0 for a tiny gradient, two gradients within 1e-5 or
    /// a cross within 2^-23
    fn turning(&self, g0: P, g1: P) -> f32 {
        let r = self.r;
        let tiny = |g: P| g.0.abs() < EPS && g.1.abs() < EPS;
        if tiny(g0) || tiny(g1) || f32::from_bits(0x3727_c5ac) > r.add(sq(r, r.sub(g0.0, g1.0)), sq(r, r.sub(g0.1, g1.1))) {
            return 0.0;
        }
        let c = r.sub(r.mul(g1.1, g0.0), r.mul(g0.1, g1.0));
        if c.abs() <= EPS { 0.0 } else { c }
    }
    /// getSmallRoundJoin: [left, right]; the outer side (offset past 0) one Bezier quarter of
    /// handle t = ((sqrt(2 (1 - dot)) - |cross|) 4) / (3 (1 - dot)), the inner side its two
    /// bevel points (the no-inner-miter flag 0x20 is set on every route, MEASURED in GDI+
    /// itself: clearing it breaks 43 of 60 random widenings)
    fn small(&self, p: P, (g0, g1): (P, P), (n0, n1): (P, P), dot: f32, cross: f32, make: Make) -> [Vec<Out>; 2] {
        let r = self.r;
        if f32::from_bits(0x3f7d_70a4).partial_cmp(&dot) != Some(std::cmp::Ordering::Greater) {
            if !make.inner() {
                return [vec![], vec![]];
            }
            return [vec![line(self.off(self.wl, n0, p))], vec![line(self.off(self.wr, n0, p))]];
        }
        let om = r.sub(1.0, dot);
        let t = r.div(r.mul(r.sub(sqrt32(r, r.add(om, om)), cross.abs()), 4.0), r.mul(om, 3.0));
        let neg = |v: P| (-v.0, -v.1);
        // (the inner miter: compound mode only, the round join's miter test of limit 3)
        let ac = cross.abs();
        let (dx, dy) = (r.sub(g0.0, g1.0), r.sub(g0.1, g1.1));
        let miter = match self.mode {
            Mode::Compound if EPS.partial_cmp(&ac) != Some(std::cmp::Ordering::Greater) && r.sub(r.add(r.mul(dy, dy), r.mul(dx, dx)), r.mul(r.mul(cross, cross), 9.0)).partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) => Some((r.div(dx, ac), r.div(dy, ac))),
            Mode::Compound | Mode::Plain => None,
        };
        let side = |w: f32| -> Vec<Out> {
            let (w, a, b) = if cross >= 0.0 { (w, n0, n1) } else { (-w, neg(n0), neg(n1)) };
            let at = |n: P| (r.add(r.mul(n.0, w), p.0), r.add(r.mul(n.1, w), p.1));
            if w > 0.0 {
                if !make.arcs() {
                    return vec![];
                }
                let k = r.mul(w, t);
                let p0 = at(a);
                let p1 = (r.add(r.mul(k, g0.0), p0.0), r.add(r.mul(k, g0.1), p0.1));
                let p3 = at(b);
                let p2 = (r.sub(p3.0, r.mul(k, g1.0)), r.sub(p3.1, r.mul(k, g1.1)));
                let bz = |p| Out { p, kind: Kind::Bezier, inner: false };
                vec![line(p0), bz(p1), bz(p2), bz(p3)]
            } else if !make.inner() {
                vec![]
            } else if w == 0.0 {
                vec![line(p)]
            } else if let Some(m) = miter {
                vec![line(at(m))]
            } else {
                vec![line(at(a)), line(at(b))]
            }
        };
        [side(self.wl), side(self.wr)]
    }
    /// getRoundJoin: a turn of less than about 90 degrees (dot past -0.001) one quarter, a
    /// sharper one split at the mid normal into two
    fn round_join(&self, p: P, s0: Seg, s1: Seg) -> [Vec<Out>; 2] {
        let r = self.r;
        let (g0, g1, n0, n1) = (s0.g, s1.g, s0.n, s1.n);
        let dot = r.add(r.mul(g1.1, g0.1), r.mul(g0.0, g1.0));
        let cross = self.turning(g0, g1);
        if dot > f32::from_bits(0xba83_126f) {
            return self.small(p, (g0, g1), (n0, n1), dot, cross, Make::Both);
        }
        let [il, ir] = self.small(p, (g0, g1), (n0, n1), dot, cross, Make::Inner);
        let (sx, sy) = (r.add(n0.0, n1.0), r.add(n0.1, n1.1));
        let (mx, my) = if EPS.partial_cmp(&sx.abs()) != Some(std::cmp::Ordering::Greater) || EPS.partial_cmp(&sy.abs()) != Some(std::cmp::Ordering::Greater) {
            let l = sqrt32(r, r.add(r.mul(sy, sy), r.mul(sx, sx)));
            (r.div(sx, l), r.div(sy, l))
        } else {
            // (an exact U-turn: the mid normal is g0)
            (-n0.1, n0.0)
        };
        let (mn, gm) = ((mx, my), (-my, mx));
        let dot2 = r.add(r.mul(g0.1, mx), r.mul(g0.0, -my));
        let cross2 = r.sub(r.mul(g0.0, mx), r.mul(g0.1, -my));
        let [a1l, a1r] = self.small(p, (g0, gm), (n0, mn), dot2, cross2, Make::Arcs);
        let dot3 = r.add(r.mul(g1.1, mx), r.mul(g1.0, -my));
        let cross3 = r.sub(r.mul(g1.1, -my), r.mul(g1.0, mx));
        let [a2l, a2r] = self.small(p, (gm, g1), (mn, n1), dot3, cross3, Make::Arcs);
        let join = |mut v: Vec<Out>, a: Vec<Out>, b: Vec<Out>| {
            // (arc 2's first point is written over arc 1's last)
            match (a.split_last(), b.split_first()) {
                (Some((_, head)), Some((b0, tail))) => {
                    v.extend_from_slice(head);
                    v.push(Out { p: b0.p, kind: Kind::Bezier, inner: false });
                    v.extend_from_slice(tail);
                }
                (Some(_), None) => v.extend(a),
                (None, _) => v.extend(b),
            }
            v
        };
        [join(il, a1l, a2l), join(ir, a1r, a2r)]
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
            .map(|(i, &(u, v))| Out { p: (r.add(r.sub(r.mul(u, a), r.mul(v, b)), c.0), r.add(r.add(r.mul(v, a), r.mul(u, b)), c.1)), kind: if i == 0 { Kind::Line } else { Kind::Bezier }, inner: false })
            .collect()
    }
    /// an open subpath's outline (WidenSubpath, CombineSubpathOutlines): one closed figure
    fn open(&self, p: &[P], caps: (Cap, Cap)) -> Res<Vec<Out>> {
        let r = self.r;
        let k = p.len();
        let segs: Vec<Seg> = p.windows(2).map(|q| self.seg(q[0], q[1])).collect();
        let e = r.mul(self.w, -0.5);
        let (first, last) = (segs[0], segs[k - 2]);
        // (the wrap-around gradient, last point to first: within the kernel's degenerate test
        // of the first one, its normal offsets the start, geometry.md 21h)
        let start_n = match k >= 3 {
            true => {
                let gw = self.seg(p[k - 1], p[0]);
                let tiny = |g: P| g.0.abs() < EPS && g.1.abs() < EPS;
                let near = r.add(sq(r, r.sub(gw.g.0, first.g.0)), sq(r, r.sub(gw.g.1, first.g.1))) < f32::from_bits(0x3727_c5ac);
                if tiny(gw.g) || tiny(first.g) || near {
                    if gw.n == (0.0, 0.0) { first.n } else { gw.n }
                } else {
                    first.n
                }
            }
            false => first.n,
        };
        let end_point = |w: f32, start: bool| {
            let (at, s, cap) = if start { (p[0], first, caps.0) } else { (p[k - 1], last, caps.1) };
            let o = self.off(w, if start { start_n } else { s.n }, at);
            match (cap, start) {
                (Cap::Square, true) => (r.add(o.0, r.mul(e, s.g.0)), r.add(o.1, r.mul(e, s.g.1))),
                (Cap::Square, false) => (r.sub(o.0, r.mul(e, s.g.0)), r.sub(o.1, r.mul(e, s.g.1))),
                (Cap::Flat | Cap::Round | Cap::Triangle | Cap::Anchor(_), _) => o,
            }
        };
        let (mut left, mut right) = (vec![line(end_point(self.wl, true))], vec![line(end_point(self.wr, true))]);
        for i in 1..k - 1 {
            let (l, rt) = self.join(p[i], segs[i - 1], segs[i]);
            left.extend(l);
            right.extend(rt);
        }
        left.push(line(end_point(self.wl, false)));
        right.push(line(end_point(self.wr, false)));
        // (an end point an inner miter falls behind takes that point's place, both kept:
        // clean-room notes geometry.md 18b)
        for side in [&mut left, &mut right] {
            let n = side.len();
            let dot = |a: P, b: P, g: P| r.add(r.mul(r.sub(b.0, a.0), g.0), r.mul(r.sub(b.1, a.1), g.1));
            // (a side of no join point, a join past 0 ..= 3, is kept: golden anchor-0, -10)
            if n > 2 && dot(side[0].p, side[1].p, first.g) < 0.0 {
                side[0].p = side[1].p;
            }
            if n > 2 && dot(side[n - 2].p, side[n - 1].p, last.g) < 0.0 {
                side[n - 1].p = side[n - 2].p;
            }
        }
        let first_left = left[0].p;
        let first_right = right[0].p;
        let last_left = left[left.len() - 1].p;
        let mut out = left;
        out[0].kind = Kind::Start;
        let mut back = reverse(right);
        match caps.1 {
            Cap::Triangle => out.push(line(self.tip(p[k - 1], last.g, false))),
            Cap::Round => {
                out.pop();
                // (its first point the band's offset point, geometry.md 18d)
                let mut c = self.round(p[k - 1], last.g, false);
                c[0].p = last_left;
                out.extend(c);
                back.remove(0);
            }
            Cap::Flat | Cap::Square | Cap::Anchor(_) => {}
        }
        // (the start offsets from the wrap-around normal: the start cap's own points from N[0],
        // its first one dropped within 0.000596 of R0 (combineTwoOpenSegments), geometry.md 22e)
        let wrapped = start_n != first.n;
        if caps.0 == Cap::Round && !wrapped {
            back.pop();
        }
        out.extend(back);
        let close_to = |a: P, b: P| r.add(r.sub(a.1, b.1).abs(), r.sub(a.0, b.0).abs()) < NEAR;
        match (caps.0, wrapped) {
            (Cap::Triangle, false) => {
                out.push(line(self.tip(p[0], first.g, true)));
                out.push(line(first_left));
            }
            (Cap::Triangle, true) => {
                let rc = self.off(self.wr, first.n, p[0]);
                if !close_to(first_right, rc) {
                    out.push(line(rc));
                }
                out.push(line(self.tip(p[0], first.g, true)));
                out.push(line(self.off(self.wl, first.n, p[0])));
            }
            (Cap::Round, false) => {
                let mut c = self.round(p[0], first.g, true);
                c[0].p = first_right;
                out.extend(c);
            }
            (Cap::Round, true) => {
                let c = self.round(p[0], first.g, true);
                let skip = usize::from(close_to(first_right, c[0].p));
                out.extend(c.into_iter().skip(skip));
            }
            (Cap::Flat | Cap::Square | Cap::Anchor(_), _) => {}
        }
        Ok(out)
    }
    /// AddCompoundCaps: a round or triangle cap over the whole width, its own closed figure,
    /// from the right side to the left at the start and back at the end (geometry.md 18d)
    fn whole_cap(&self, p: &[P], cap: Cap, start: bool) -> Option<Vec<Out>> {
        let k = p.len();
        let (at, s) = if start { (p[0], self.seg(p[0], p[1])) } else { (p[k - 1], self.seg(p[k - 2], p[k - 1])) };
        let (a, b) = (self.off(self.wr, s.n, at), self.off(self.wl, s.n, at));
        let mut f = match cap {
            Cap::Triangle if start => vec![line(a), line(self.tip(at, s.g, true)), line(b)],
            Cap::Triangle => vec![line(b), line(self.tip(at, s.g, false)), line(a)],
            Cap::Round => {
                let mut c = self.round(at, s.g, start);
                c[0].p = if start { a } else { b };
                c
            }
            Cap::Flat | Cap::Square | Cap::Anchor(_) => return None,
        };
        f[0].kind = Kind::Start;
        Some(f)
    }
    /// a closed subpath's two figures: the left lists, then the right ones reversed
    fn closed(&self, p: &[P]) -> [Vec<Out>; 2] {
        let k = p.len();
        let segs: Vec<Seg> = (0..k).map(|i| self.seg(p[i], p[(i + 1) % k])).collect();
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for i in 0..k {
            let (l, rt) = self.join(p[i], segs[(i + k - 1) % k], segs[i]);
            left.extend(l);
            right.extend(rt);
        }
        let fig = |mut o: Vec<Out>| -> Vec<Out> {
            if let Some(f) = o.first_mut() {
                f.kind = Kind::Start;
            }
            o
        };
        [fig(left), fig(reverse(right))]
    }
}

fn sq(r: Round, v: f32) -> f32 {
    r.mul(v, v)
}

/// a side's points backward: a point a Bezier one when the forward point after it was, so a
/// Bezier run stays one
fn reverse(v: Vec<Out>) -> Vec<Out> {
    let kinds: Vec<Kind> = v.iter().skip(1).map(|o| o.kind).chain([Kind::Line]).collect();
    v.into_iter().zip(kinds).rev().map(|(o, k)| Out { p: o.p, kind: match k { Kind::Bezier => Kind::Bezier, Kind::Start | Kind::Line => Kind::Line }, inner: o.inner }).collect()
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
pub(crate) struct Sub {
    pub(crate) pts: Vec<P>,
    closed: bool,
    pub(crate) caps: (Cap, Cap),
}

/// why GetWidenedPathInternal gives nothing back
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Widened {
    /// the matrix has no inverse
    NoInverse,
    /// a join past 0 ..= 3 (a closed subpath on x64, any on x86): status 3 (OutOfMemory),
    /// GdipWidenPath leaves the path as it was, a stroke draws nothing (golden penjoin-4,
    /// penjoin--1)
    Failed,
}

/// GetWidenedPathInternal: path widened by pen through m (the world to device matrix, or
/// GdipWidenPath's), flattened at flatness; the outline in m's device space
pub fn widen(r: Round, path: &Path, pen: &Pen, m: &Matrix, flatness: f32) -> Res<Result<Path, Widened>> {
    hit(clause::GP_PEN_WIDE);
    let Some(minv) = invert(r, m) else { return Ok(Err(Widened::NoInverse)) };
    let (start, end) = (Cap::line(pen.start_cap), Cap::line(pen.end_cap));
    let mut c = path.clone();
    c.flatten(r, Some(m), flatness)?;
    for q in &mut c.pts {
        (q.x, q.y) = apply(r, &minv, q.x, q.y);
    }
    let mut figs: Vec<(Vec<P>, bool)> = c.figures().iter().map(|f| (f.iter().map(|q| (q.x, q.y)).collect(), f.last().is_some_and(|q| q.close))).collect();
    // (PenAlignment Inset with a closed subpath: GetWidenedPath's regions; Inset on open
    // subpaths only and every other value is Center, MEASURED in GDI+ itself, golden
    // penalign-0, -2, --1)
    if pen.align == 1 && figs.iter().any(|f| f.1) {
        return inset(r, path, pen, m, flatness);
    }
    let (major, minor) = axes(r, m);
    let caps = match anchors(r, &mut figs, pen, m, &minv, major.min(minor), (start, end))? {
        Ok(c) => c,
        Err(e) => return Ok(Err(e)),
    };
    let mut out = match body(r, &figs, pen, m, &minv, (start, end), false)? {
        Ok(o) => o,
        Err(e) => return Ok(Err(e)),
    };
    out.extend(caps);
    for q in &mut out {
        (q.x, q.y) = apply(r, m, q.x, q.y);
    }
    Ok(Ok(Path::of(path.fill, out)))
}

/// the widener proper on flattened world figures (GpPathWidener): the outline in world space,
/// a thin pen's widened in device space and back; inner keeps the inner join bit (a custom
/// stroke cap's own widening, geometry.md 21g)
fn body(r: Round, figs: &[(Vec<P>, bool)], pen: &Pen, m: &Matrix, minv: &Matrix, (start, end): (Cap, Cap), inner: bool) -> Res<Result<Vec<Point>, Widened>> {
    let join = Join::of(pen.join);
    let (major, minor) = axes(r, m);
    let subs: Vec<Sub> = match pattern(pen) {
        Some(pat) => {
            // (CreateDashedPath's own set-up: Flatten(M) and Transform(Minv) again, geometry.md 22f)
            let again: Vec<(Vec<P>, bool)> = figs.iter().map(|(p, c)| (p.iter().map(|&(x, y)| {
                let d = apply(r, m, x, y);
                apply(r, minv, d.0, d.1)
            }).collect(), *c)).collect();
            dashes(r, &again, pen, major, true, pat, (start, end))
        }
        None => figs.iter().map(|(pts, closed)| Sub { pts: pts.clone(), closed: *closed, caps: (start, end) }).collect(),
    };
    let thin_width = f32::from_bits(0x3f80_0008);
    let thin = thin_width > r.mul(pen.width, minor);
    let w = if thin { thin_width } else { pen.width };
    let wl = r.mul(w, 0.5);
    let lim = sq(r, pen.miter.max(1.0));
    let whole = Widener { r, w, wl, wr: r.sub(wl, w), lim, join, mode: Mode::Plain };
    // (a compound array: one band a pair, h - c w each side, an empty pair none; four entries
    // or more the compound switch, clean-room notes geometry.md 14, 18)
    let (bands, mode): (Vec<(f32, f32)>, Mode) = match pen.compound.as_slice() {
        [] => (vec![(whole.wl, whole.wr)], Mode::Plain),
        c => (
            c.chunks(2).filter(|q| q[0] != q[1]).map(|q| (r.sub(wl, r.mul(q[0], w)), r.sub(wl, r.mul(q[1], w)))).collect(),
            if c.len() >= 4 { Mode::Compound } else { Mode::Plain },
        ),
    };
    let mut out = Vec::new();
    for s in subs {
        // (a thin pen: widened in device space, then back through the inverse)
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
        // (x86 fails at the first join it makes: a two-point subpath has none, golden
        // penobject-41)
        if let (Join::Nothing, true, _, _) | (Join::Nothing, false, 32, true) = (join, s.closed, super::arch(), pts.len() > 2) {
            return Ok(Err(Widened::Failed));
        }
        // (compound mode: round and triangle caps once over the whole width after the bands)
        let apart = |c: Cap| mode == Mode::Compound && matches!(c, Cap::Round | Cap::Triangle);
        let band_caps = (if apart(s.caps.0) { Cap::Flat } else { s.caps.0 }, if apart(s.caps.1) { Cap::Flat } else { s.caps.1 });
        let mut figs = Vec::new();
        for &(bl, br) in &bands {
            let wd = Widener { wl: bl, wr: br, mode, ..whole };
            if s.closed {
                figs.extend(wd.closed(&pts));
            } else {
                figs.push(wd.open(&pts, band_caps)?);
            }
        }
        if !s.closed {
            for (cap, start) in [(s.caps.0, true), (s.caps.1, false)] {
                if apart(cap)
                    && let Some(f) = whole.whole_cap(&pts, cap, start)
                {
                    figs.push(f);
                }
            }
        }
        for f in figs {
            let n = f.len();
            out.extend(f.into_iter().enumerate().map(|(i, o)| {
                let (x, y) = if thin { apply(r, minv, o.p.0, o.p.1) } else { o.p };
                Point { x, y, kind: o.kind, close: i + 1 == n, inner: inner && o.inner }
            }));
        }
    }
    Ok(Ok(out))
}

/// GetWidenedPath of an Inset pen: each closed subpath's own 2w centred outline and the whole
/// path's fill (its fill mode) as aliased device pixels (the pixel offset in m), ANDed, back to
/// figures by RegionToPath; then the open subpaths widened centred w (clean-room notes
/// geometry.md 17, MEASURED in GDI+ itself, golden penalign-1)
fn inset(r: Round, path: &Path, pen: &Pen, m: &Matrix, flatness: f32) -> Res<Result<Path, Widened>> {
    use super::gdiplus::FillMode;
    use super::gpregion::{pixels, points, to_path};
    hit(clause::GP_PEN_INSET);
    let fm = if path.fill == 0 { FillMode::Alternate } else { FillMode::Winding };
    let whole = pixels(r, path, m, fm)?;
    let id = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    let centred = |w: f32| Pen { width: w, align: 0, ..pen.clone() };
    let (mut out, mut open) = (Vec::new(), Vec::new());
    for f in path.figures() {
        if !f.last().is_some_and(|q| q.close) {
            open.extend_from_slice(f);
            continue;
        }
        let sub = Path::of(path.fill, f.to_vec());
        let w = match widen(r, &sub, &centred(r.add(pen.width, pen.width)), m, flatness)? {
            Ok(w) => w,
            Err(e) => return Ok(Err(e)),
        };
        let own = pixels(r, &w, &id, FillMode::Winding)?;
        out.extend(points(&to_path(&own.and(&whole))));
    }
    if !open.is_empty() {
        match widen(r, &Path::of(path.fill, open), &centred(pen.width), m, flatness)? {
            Ok(w) => out.extend(w.pts),
            Err(e) => return Ok(Err(e)),
        }
    }
    Ok(Ok(Path::of(path.fill, out)))
}

/// a cap object as GpEndCapCreator reads it: an anchor's reference cap, an adjustable arrow
/// or a GdipCreateCustomLineCap's
struct CapObj {
    /// its fill path's points (closed) and fillLength
    fill: Option<(Vec<Point>, f32)>,
    /// its stroke path's points (open) and strokeLength
    stroke: Option<(Vec<P>, f32)>,
    base: f32,
    /// the width scale (GdipSetCustomLineCapWidthScale; 1 for the anchors and arrows)
    scale: f32,
    /// the stroke join and the stroke start and end caps
    join: i64,
    caps: (i64, i64),
}

fn polygon(v: &[P]) -> Vec<Point> {
    let n = v.len();
    v.iter().enumerate().map(|(i, &(x, y))| Point { x, y, kind: if i == 0 { Kind::Start } else { Kind::Line }, close: i + 1 == n, inner: false }).collect()
}

fn reference(r: Round, a: Anchor) -> Res<CapObj> {
    let c = f32::from_bits(0x3f35_04f4);
    let s3 = f32::from_bits(0x3fdd_b3d7);
    let (fill, len, base) = match a {
        Anchor::Square => (polygon(&[(-c, -c), (c, -c), (c, c), (-c, c)]), c, 0.0),
        Anchor::Round => {
            let mut e = Path::new(0);
            e.add_ellipse(r, (-1.0, -1.0, 2.0, 2.0)).map_err(|_| Fail::Bad)?;
            (e.pts, 1.0, 0.0)
        }
        Anchor::Diamond => (polygon(&[(0.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (1.0, 0.0)]), 1.0, 0.0),
        Anchor::Arrow => (polygon(&[(0.0, 0.0), (-1.0, -s3), (1.0, -s3)]), s3, 1.0),
    };
    Ok(CapObj { fill: Some((fill, len)), stroke: None, base, scale: 1.0, join: 0, caps: (0, 0) })
}

/// SetFillPath / SetStrokePath's length: minus the lowest crossing of the path with the y axis
/// (a closed path's closing edge first), at least 0 (clean-room notes geometry.md 21b)
pub fn cap_length(r: Round, p: &[P], closed: bool) -> f32 {
    let mut m = 0.0f32;
    let n = p.len();
    if n <= 1 {
        return -m;
    }
    let mut prev = if closed { p[n - 1] } else { p[0] };
    for &cur in p {
        let (dx, dy) = (r.sub(prev.0, cur.0), r.sub(prev.1, cur.1));
        if EPS <= dx.abs() {
            let t = r.div(-cur.0, dx);
            // (a NaN t would leave m as it is either way)
            if t >= -EPS && r.sub(t, 1.0) <= EPS {
                let y = r.add(r.mul(dy, t), cur.1);
                m = if y < m { y } else { m };
            }
        }
        prev = cur;
    }
    -m
}

/// AddPathPolygon's points: a last point on the first dropped past three
fn fill_points(pts: &[(f32, f32)]) -> &[(f32, f32)] {
    match pts {
        [first, .., last] if pts.len() > 3 && first == last => &pts[..pts.len() - 1],
        _ => pts,
    }
}

/// the statuses of making a custom cap (GdipCreateCustomLineCap, then its setters), 0 when the
/// pen gets it: a fill path of fewer than 3 points or a stroke path of fewer than 2
/// InvalidParameter, a fill path that does not cross the negative y axis NotImplemented, stroke
/// caps past 0 ..= 3 InvalidParameter (geometry.md 21a)
pub fn cap_status(r: Round, c: &CustomCap) -> u32 {
    match c {
        CustomCap::Arrow { .. } => 0,
        CustomCap::Path { stroke: true, pts, caps, .. } => {
            if pts.len() < 2 || !(0..=3).contains(&caps.0) || !(0..=3).contains(&caps.1) {
                2
            } else {
                0
            }
        }
        CustomCap::Path { stroke: false, pts, caps, .. } => {
            let p = fill_points(pts);
            if p.len() < 3 {
                2
            } else if EPS > cap_length(r, p, true) {
                6
            } else if !(0..=3).contains(&caps.0) || !(0..=3).contains(&caps.1) {
                2
            } else {
                0
            }
        }
    }
}

/// a custom cap object's paths (GdipCreateAdjustableArrowCap's rebuild, geometry.md 21c)
fn custom(r: Round, c: &CustomCap) -> CapObj {
    match *c {
        CustomCap::Arrow { height, width, filled, middle } => {
            let base = if width != 0.0 { r.div(height, width) } else { 0.0 };
            let q = [(r.mul(width, 0.5), -height), (0.0, 0.0), (r.mul(-width, 0.5), -height)];
            let (fill, stroke) = if filled {
                let mut v = q.to_vec();
                if middle != 0.0 {
                    v.push((0.0, r.sub(middle, height)));
                }
                let l = cap_length(r, &v, true);
                (Some((polygon(&v), l)), None)
            } else {
                (None, Some((q.to_vec(), cap_length(r, &q, false))))
            };
            CapObj { fill, stroke, base, scale: 1.0, join: 0, caps: (0, 0) }
        }
        CustomCap::Path { stroke, ref pts, base_inset, width_scale, join, caps } => {
            let (fill, stroke) = if stroke {
                (None, Some((pts.clone(), cap_length(r, pts, false))))
            } else {
                let p = fill_points(pts);
                (Some((polygon(p), cap_length(r, p, true))), None)
            };
            CapObj { fill, stroke, base: base_inset, scale: width_scale, join, caps }
        }
    }
}

/// the cap object of a pen's end: an anchor's reference cap, or its custom cap object's
fn end_cap(r: Round, cap: Cap, value: i64, custom_cap: Option<&CustomCap>) -> Res<Option<CapObj>> {
    match (cap, value, custom_cap) {
        (Cap::Anchor(a), _, _) => reference(r, a).map(Some),
        (Cap::Flat, 0xff, Some(c)) => {
            hit(clause::GP_PEN_CUSTOM);
            Ok(Some(custom(r, c)))
        }
        (Cap::Flat | Cap::Square | Cap::Round | Cap::Triangle, _, _) => Ok(None),
    }
}

/// GpEndCapCreator on each open subpath (geometry.md 9, 21d-21g): the start's then the end's
/// fill caps, then their stroke caps, each walk erasing the points within the cap's radius of
/// its end and moving the first kept one to the circle pulled back by the base inset; the cap
/// figures placed at the original ends (start figures, then end figures), a stroke cap's
/// polyline widened by a pen of its own
fn anchors(r: Round, figs: &mut [(Vec<P>, bool)], pen: &Pen, m: &Matrix, minv: &Matrix, min_axis: f32, caps: (Cap, Cap)) -> Res<Result<Vec<Point>, Widened>> {
    let mut out = Vec::new();
    let sc = end_cap(r, caps.0, pen.start_cap, pen.custom[0].as_ref())?;
    let ec = end_cap(r, caps.1, pen.end_cap, pen.custom[1].as_ref())?;
    if sc.is_none() && ec.is_none() {
        return Ok(Ok(out));
    }
    let unit = r.div(1.0, min_axis);
    let s2 = r.div(2.0, min_axis);
    // (9d: a cap point (px, py) through d = -g scaled by s about o)
    let place = |q: (f32, f32), d: P, s: f32, o: P| -> P {
        let (a, b, cn) = (r.mul(d.0, s), r.mul(d.1, s), r.mul(-s, d.0));
        (r.add(r.add(r.mul(b, q.0), r.mul(a, q.1)), o.0), r.add(r.add(r.mul(cn, q.0), r.mul(b, q.1)), o.1))
    };
    for (pts, closed) in figs.iter_mut() {
        if *closed || pts.is_empty() {
            continue;
        }
        let orig = pts.clone();
        let n = pts.len();
        let mut marked = vec![false; n];
        let (mut starts, mut ends) = (Vec::new(), Vec::new());
        let steps: [(Option<&CapObj>, bool, bool); 4] = [(sc.as_ref(), true, false), (ec.as_ref(), false, false), (sc.as_ref(), true, true), (ec.as_ref(), false, true)];
        for (cap, forward, stroke) in steps {
            let Some(cap) = cap else { continue };
            let order: Vec<usize> = if forward { (0..n).collect() } else { (0..n).rev().collect() };
            let o = orig[order[0]];
            let s1 = r.mul(cap.scale, pen.width);
            let figs_out = if forward { &mut starts } else { &mut ends };
            match (stroke, &cap.fill, &cap.stroke) {
                (false, Some((fill, l)), _) => {
                    // (a fill length of 0, a degenerate adjustable arrow: no walk, every point on
                    // the end, geometry.md 19c, 21c)
                    if EPS > l.abs() {
                        figs_out.extend(fill.iter().map(|q| Point { x: o.0, y: o.1, ..*q }));
                        continue;
                    }
                    let mm = if s1 > unit { s1 } else { unit };
                    let s = if s2 > s1 { s2 } else { s1 };
                    let g = shorten(r, pts, &mut marked, &order, *l, cap.base, mm);
                    let d = (-g.0, -g.1);
                    figs_out.extend(fill.iter().map(|q| {
                        let (x, y) = place((q.x, q.y), d, s, o);
                        Point { x, y, ..*q }
                    }));
                }
                (true, _, Some((line, l))) => {
                    let l = if EPS > l.abs() { 1.0 } else { *l };
                    let g = shorten(r, pts, &mut marked, &order, l, cap.base, s1);
                    // (a width scale of 0 or less: GetTransformedStrokeCap writes nothing and the
                    // widener reads uninitialized buffers, undefined (geometry.md 22d): the
                    // spec's open GP.PEN)
                    if 0.0 >= s1 {
                        return Err(Fail::Unmodeled(open::GP_PEN));
                    }
                    let d = (-g.0, -g.1);
                    let placed: Vec<P> = line.iter().map(|&q| place(q, d, s1, o)).collect();
                    let cap_pen = Pen { width: s1, start_cap: cap.caps.0, end_cap: cap.caps.1, dash_cap: 0, join: cap.join, align: 0, miter: 10.0, dash: 0, compound: Vec::new(), custom: [None, None], ..pen.clone() };
                    // (an x86 stroke join past 0 ..= 3 at a vertex: that figure left out)
                    if let Ok(w) = body(r, &[(placed, false)], &cap_pen, m, minv, (Cap::line(cap.caps.0), Cap::line(cap.caps.1)), true)? {
                        figs_out.extend(w);
                    }
                }
                (false, None, _) | (true, _, None) => {}
            }
        }
        out.extend(starts);
        out.extend(ends);
        let kept: Vec<P> = pts.iter().zip(&marked).filter(|(_, m)| !**m).map(|(q, _)| *q).collect();
        // (the walks skip no marked point: the end walk can move the start walk's point again
        // and leave it alone; GpPathWidener::Initialize then fails the whole call, OutOfMemory,
        // clean-room notes geometry.md 20, golden anchor-22, anchor-23; none left is no body)
        if kept.len() == 1 {
            return Ok(Err(Widened::Failed));
        }
        *pts = kept;
    }
    Ok(Ok(out))
}

/// one end's walk (ComputeCapGradient): the direction from the end into the line
fn shorten(r: Round, pts: &mut [P], marked: &mut [bool], order: &[usize], l: f32, base: f32, mm: f32) -> P {
    let rad = r.mul(l, mm);
    let r2 = r.mul(rad, rad);
    let inset = if EPS > l.abs() { 0.0 } else { r.div(base, l) };
    let e = pts[order[0]];
    let (mut cur, mut beyond, mut was) = (order[0], None, false);
    for &i in order {
        let q = pts[i];
        if r.add(sq(r, r.sub(q.0, e.0)), sq(r, r.sub(q.1, e.1))) > r2 {
            beyond = Some(i);
            break;
        }
        (cur, was) = (i, marked[i]);
        marked[i] = true;
    }
    if beyond.is_some() && !was {
        marked[cur] = false;
    }
    let x = intersect(r, e, r2, pts[beyond.unwrap_or(cur)], pts[cur]).unwrap_or(pts[cur]);
    let g = normalize(r, (r.sub(x.0, e.0), r.sub(x.1, e.1)));
    let k = r.sub(1.0, inset);
    pts[cur] = (r.add(r.mul(r.sub(e.0, x.0), k), x.0), r.add(r.mul(r.sub(e.1, x.1), k), x.1));
    g
}

/// intersect_circle_line: the first crossing of the circle of squared radius r2 about c going
/// from a toward b
fn intersect(r: Round, c: P, r2: f32, a: P, b: P) -> Option<P> {
    let e = f64::from(EPS);
    let (dx, dy) = (r.sub(b.0, a.0), r.sub(b.1, a.1));
    let ld = sqrt64(r, f64::from(r.add(r.mul(dy, dy), r.mul(dx, dx))));
    if e > ld {
        return None;
    }
    let inv = to32(r, div64(r, 1.0, ld));
    let (ex, ey) = (r.sub(c.0, a.0), r.sub(c.1, a.1));
    let (ux, uy) = (r.mul(dx, inv), r.mul(dy, inv));
    let cc = f64::from(r.add(r.mul(ey, ey), r.mul(ex, ex)));
    let p = f64::from(r.add(r.mul(ey, uy), r.mul(ex, ux)));
    let r2 = f64::from(r2);
    if e > p && cc >= r2 {
        return None;
    }
    let disc = add64(r, add64(r, r2, -cc), mul64(r, p, p));
    if e > disc {
        return None;
    }
    let s = sqrt64(r, disc);
    let near = add64(r, p, -s);
    let far = add64(r, s, p);
    let t = if cc >= r2 && near > e && near >= 0.0 {
        near
    } else if far > e && far >= 0.0 {
        far
    } else {
        return None;
    };
    let tf = to32(r, t);
    Some((r.add(r.mul(tf, ux), a.0), r.add(r.mul(tf, uy), a.1)))
}

/// Normalize: the length in double, below 2^-23 (0, 0)
fn normalize(r: Round, (x, y): P) -> P {
    let d = add64(r, mul64(r, f64::from(x), f64::from(x)), mul64(r, f64::from(y), f64::from(y)));
    if 0.0 > d || sqrt64(r, d).abs() < f64::from(EPS) {
        return (0.0, 0.0);
    }
    let l = to32(r, sqrt64(r, d));
    (r.div(x, l), r.div(y, l))
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
    let Some(pat) = pattern(pen) else { return Ok(None) };
    let mut c = path.clone();
    c.flatten(r, None, 0.25)?;
    let figs: Vec<(Vec<P>, bool)> = c.figures().iter().map(|f| (f.iter().map(|q| (q.x, q.y)).collect(), f.last().is_some_and(|q| q.close))).collect();
    let mut out = Vec::new();
    for s in dashes(r, &figs, pen, 1.0, false, pat, (Cap::Flat, Cap::Flat)) {
        out.extend(s.pts.iter().enumerate().map(|(i, &(x, y))| Point { x, y, kind: if i == 0 { Kind::Start } else { Kind::Line }, close: false, inner: false }));
    }
    Ok(Some(Path::of(path.fill, out)))
}

/// CreateDashedPath: each figure cut into dashes (getDashData), each dash an open subpath with
/// its ends' caps; the dash cap adjusting the pattern when adjust
#[allow(clippy::too_many_arguments)]
pub(crate) fn dashes(r: Round, figs: &[(Vec<P>, bool)], pen: &Pen, major: f32, adjust: bool, pat: &[f32], caps: (Cap, Cap)) -> Vec<Sub> {
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
        (true, Cap::Flat | Cap::Square | Cap::Anchor(_)) | (false, _) => 0.0,
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
        let off = r.sub(r.mul(pen.width, pen.dash_offset), c);
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
pub fn sqrt64(r: Round, a: f64) -> f64 {
    let v = a.sqrt();
    match r {
        Round::Nearest => v,
        Round::Down => {
            if v.mul_add(v, -a) > 0.0 { f64::from_bits(v.to_bits() - 1) } else { v }
        }
    }
}
