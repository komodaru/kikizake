//! GDI+ paths and their rasterization: GraphicsPath's figures (AddLine(s), AddBezier(s),
//! AddRectangle, AddEllipse, AddArc, AddPie, AddPolygon, AddCurve, AddClosedCurve, CloseFigure,
//! StartFigure), FlattenPath, GetPathWorldBounds, and the scan converter every fill goes through
//! (FillPath, FillEllipse, FillPie, FillClosedCurve, FillPolygon).
//!
//! Sources (Microsoft Learn): "GraphicsPath", "Graphics::FillPath", "FillMode", "SmoothingMode",
//! "PixelOffsetMode"; the rules themselves MEASURED against Windows' answers (golden
//! gdipluspath-*, gdiplusline-*), as clean-room notes on GDI+'s own arithmetic predicted them.

use super::gdiplus::{FillMode, Matrix, PixelOffset, Round};
use super::{Fail, Res};
use crate::spec::{clause, hit};

/// FLT_EPSILON: AddLines and AddBeziers join a figure's last point to a first point this close
pub const EPS: f32 = 1.192_092_9e-7;
/// float pi as GDI+ spells it, and its multiples
const PI_F: f32 = std::f32::consts::PI;
const TWO_PI_F: f32 = std::f32::consts::TAU;
const HALF_PI_F: f32 = std::f32::consts::FRAC_PI_2;
const QUARTER_PI_F: f32 = std::f32::consts::FRAC_PI_4;

// ---- float arithmetic in the rounding GDI+ sets (GP.MATRIX.X86, GP.MATRIX.X64) ----

/// the f32 a mode makes of an f64 (cvtsd2ss)
pub fn to32(r: Round, v: f64) -> f32 {
    let f = v as f32;
    match r {
        Round::Nearest => f,
        Round::Down if f.is_finite() && (f as f64) > v => f.next_down(),
        Round::Down => f,
    }
}
/// sqrtss
pub fn sqrt32(r: Round, a: f32) -> f32 {
    let q = a.sqrt();
    match r {
        Round::Nearest => q,
        // (q q is exact in f64)
        Round::Down if q.is_finite() && q > 0.0 && (q as f64) * (q as f64) > a as f64 => q.next_down(),
        Round::Down => q,
    }
}
pub fn add64(r: Round, a: f64, b: f64) -> f64 {
    let s = a + b;
    match r {
        Round::Nearest => s,
        Round::Down => {
            if !s.is_finite() {
                return s;
            }
            let bb = s - a;
            let e = (a - (s - bb)) + (b - bb);
            if s == 0.0 && e == 0.0 {
                return if a.is_sign_positive() && b.is_sign_positive() { 0.0 } else { -0.0 };
            }
            if e < 0.0 { s.next_down() } else { s }
        }
    }
}
pub fn mul64(r: Round, a: f64, b: f64) -> f64 {
    let p = a * b;
    match r {
        Round::Nearest => p,
        Round::Down => {
            if !p.is_finite() || p == 0.0 {
                return p;
            }
            if a.mul_add(b, -p) < 0.0 { p.next_down() } else { p }
        }
    }
}
pub fn div64(r: Round, a: f64, b: f64) -> f64 {
    let q = a / b;
    match r {
        Round::Nearest => q,
        Round::Down => {
            if !q.is_finite() || q == 0.0 {
                return q;
            }
            // (a - q b: the true quotient is q + rest / b)
            let rest = (-q).mul_add(b, a);
            if rest != 0.0 && (rest < 0.0) != (b < 0.0) { q.next_down() } else { q }
        }
    }
}
/// the CRT's sinf / cosf (the exact value in the mode's rounding)
fn sinf(r: Round, a: f32) -> f32 {
    to32(r, (a as f64).sin())
}
fn cosf(r: Round, a: f32) -> f32 {
    to32(r, (a as f64).cos())
}

/// RasterizerCeiling: v to 1/16 (v 16 + 1/2 floored), then that ceiled to an integer; past the
/// int range the integer indefinite
pub fn ceil16(r: Round, v: f32) -> i32 {
    let t = r.add(r.mul(v, 16.0), 0.5).floor();
    let i = if t.is_finite() && t >= i32::MIN as f32 && t < 2147483648.0 { t as i32 } else { i32::MIN };
    i.wrapping_add(15) >> 4
}

/// how GpMatrix transforms a point: by the complexity ComputeComplexity gives the matrix, with a
/// tolerance (MEASURED, golden gdipluspath patharc-20: an arc piece rotated by almost 180
/// degrees transformed without its sin terms)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Complexity {
    Identity,
    Translate,
    Scale,
    General,
}
/// 0x3a1c4000, about 5.96e-4
pub const NEAR: f32 = 5.960_464_5e-4;
const _: () = assert!(NEAR.to_bits() == 0x3a1c_4000);
pub fn complexity(r: Round, m: &Matrix) -> Complexity {
    let big = m[0].abs().max(m[1].abs()).max(m[2].abs()).max(m[3].abs());
    let tol = r.mul(NEAR, big);
    let moved = !(m[4] == 0.0 && m[5] == 0.0);
    if !(tol > m[1].abs() && tol > m[2].abs()) {
        return Complexity::General;
    }
    match (NEAR > r.sub(m[0], 1.0).abs() && NEAR > r.sub(m[3], 1.0).abs(), moved) {
        (true, false) => Complexity::Identity,
        (true, true) => Complexity::Translate,
        (false, _) => Complexity::Scale,
    }
}
pub fn apply(r: Round, m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    match complexity(r, m) {
        Complexity::Identity => (x, y),
        Complexity::Translate => (r.add(m[4], x), r.add(m[5], y)),
        Complexity::Scale => (r.add(r.mul(m[0], x), m[4]), r.add(r.mul(y, m[3]), m[5])),
        Complexity::General => (r.add(r.add(r.mul(x, m[0]), r.mul(y, m[2])), m[4]), r.add(r.add(r.mul(x, m[1]), r.mul(y, m[3])), m[5])),
    }
}
/// a matrix that only moves (or not even that)
pub fn translates_only(r: Round, m: &Matrix) -> bool {
    match complexity(r, m) {
        Complexity::Identity | Complexity::Translate => true,
        Complexity::Scale | Complexity::General => false,
    }
}
fn scaled(r: Round, m: &Matrix, k: f32) -> Matrix {
    m.map(|e| r.mul(e, k))
}

// ---- paths ----

/// a point's PathPointType
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Start,
    Line,
    Bezier,
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub kind: Kind,
    pub close: bool,
    /// the widener's inner join side (0x40), kept on custom stroke caps only
    pub inner: bool,
}
impl Point {
    pub fn type_byte(self) -> u8 {
        let k = match self.kind {
            Kind::Start => 0,
            Kind::Line => 1,
            Kind::Bezier => 3,
        };
        k | if self.close { 0x80 } else { 0 } | if self.inner { 0x40 } else { 0 }
    }
}

/// why an Add* adds nothing
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    InvalidParameter,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Path {
    /// the FillMode it was made with, as given (GdipCreatePath takes any)
    pub fill: i64,
    pub pts: Vec<Point>,
    /// the next point starts a figure
    new_figure: bool,
}

impl Path {
    pub fn new(fill: i64) -> Path {
        hit(clause::GP_PATH_BUILD);
        Path { fill, pts: Vec::new(), new_figure: true }
    }
    /// a path of made points (a widened outline, dashes), the next Add* a new figure
    pub fn of(fill: i64, pts: Vec<Point>) -> Path {
        Path { fill, pts, new_figure: true }
    }
    fn open_last(&self) -> Option<Point> {
        if self.new_figure { None } else { self.pts.last().copied() }
    }
    /// the first point of new points: skipped on the open figure's last point, a line to it
    /// otherwise, or a figure's start
    fn join(&mut self, r: Round, p: (f32, f32)) {
        match self.open_last() {
            Some(l) if r.sub(p.0, l.x).abs() < EPS && r.sub(p.1, l.y).abs() < EPS => {}
            Some(_) => self.pts.push(Point { x: p.0, y: p.1, kind: Kind::Line, close: false, inner: false }),
            None => {
                self.pts.push(Point { x: p.0, y: p.1, kind: Kind::Start, close: false, inner: false });
                self.new_figure = false;
            }
        }
    }
    pub fn add_lines(&mut self, r: Round, pts: &[(f32, f32)]) -> Result<(), Refused> {
        let Some((&first, rest)) = pts.split_first() else {
            return Err(Refused::InvalidParameter);
        };
        self.join(r, first);
        self.pts.extend(rest.iter().map(|&(x, y)| Point { x, y, kind: Kind::Line, close: false, inner: false }));
        Ok(())
    }
    pub fn add_beziers(&mut self, r: Round, pts: &[(f32, f32)]) -> Result<(), Refused> {
        if pts.len() < 4 || !(pts.len() - 1).is_multiple_of(3) {
            return Err(Refused::InvalidParameter);
        }
        self.join(r, pts[0]);
        self.pts.extend(pts[1..].iter().map(|&(x, y)| Point { x, y, kind: Kind::Bezier, close: false, inner: false }));
        Ok(())
    }
    pub fn start_figure(&mut self) {
        self.new_figure = true;
    }
    pub fn close_figure(&mut self) {
        if let Some(l) = self.pts.last_mut() {
            l.close = true;
        }
        self.new_figure = true;
    }
    pub fn add_rectangle(&mut self, r: Round, (x, y, w, h): (f32, f32, f32, f32)) -> Result<(), Refused> {
        if w <= EPS || h <= EPS {
            return Ok(());
        }
        let (x1, y1) = (r.add(x, w), r.add(y, h));
        self.start_figure();
        self.add_lines(r, &[(x, y), (x1, y), (x1, y1), (x, y1)])?;
        self.close_figure();
        Ok(())
    }
    pub fn add_polygon(&mut self, r: Round, pts: &[(f32, f32)]) -> Result<(), Refused> {
        if pts.len() < 3 {
            return Err(Refused::InvalidParameter);
        }
        // (a last point exactly on the first dropped, past three)
        let pts = if pts.len() > 3 && pts[pts.len() - 1] == pts[0] { &pts[..pts.len() - 1] } else { pts };
        self.start_figure();
        self.add_lines(r, pts)?;
        self.close_figure();
        Ok(())
    }
    /// AddEllipse: four quarter Beziers from angle 0 toward +y
    pub fn add_ellipse(&mut self, r: Round, rect: (f32, f32, f32, f32)) -> Result<(), Refused> {
        hit(clause::GP_PATH_ARC);
        let k = r.div(r.mul(r.sub(sqrt32(r, 2.0), 1.0), 4.0), 3.0);
        let (rx, ry) = (r.mul(rect.2, 0.5), r.mul(rect.3, 0.5));
        let (cx, cy) = (r.add(rx, rect.0), r.add(ry, rect.1));
        let unit = [(1.0, 0.0), (1.0, k), (k, 1.0), (0.0, 1.0), (-k, 1.0), (-1.0, k), (-1.0, 0.0), (-1.0, -k), (-k, -1.0), (0.0, -1.0), (k, -1.0), (1.0, -k), (1.0, 0.0)];
        let pts = unit.map(|(u, v): (f32, f32)| (r.add(r.mul(rx, u), cx), r.add(r.mul(ry, v), cy)));
        self.start_figure();
        self.add_beziers(r, &pts)?;
        self.close_figure();
        Ok(())
    }
    pub fn add_arc(&mut self, r: Round, rect: (f32, f32, f32, f32), start: f32, sweep: f32) -> Result<(), Refused> {
        let (sweep, close) = if sweep >= 360.0 {
            (360.0, true)
        } else if sweep <= -360.0 {
            (-360.0, true)
        } else {
            (sweep, false)
        };
        let pts = arc_points(r, rect, start, sweep).ok_or(Refused::InvalidParameter)?;
        if !pts.is_empty() {
            self.add_beziers(r, &pts)?;
        }
        if close {
            self.close_figure();
        }
        Ok(())
    }
    pub fn add_pie(&mut self, r: Round, rect: (f32, f32, f32, f32), start: f32, sweep: f32) -> Result<(), Refused> {
        self.start_figure();
        self.add_lines(r, &[(r.add(r.mul(rect.2, 0.5), rect.0), r.add(r.mul(rect.3, 0.5), rect.1))])?;
        self.add_arc(r, rect, start, sweep)?;
        self.close_figure();
        Ok(())
    }
    pub fn add_curve(&mut self, r: Round, pts: &[(f32, f32)], tension: f32) -> Result<(), Refused> {
        if pts.len() < 2 {
            return Err(Refused::InvalidParameter);
        }
        let b = spline(r, pts, tension, false);
        self.add_beziers(r, &b)
    }
    pub fn add_closed_curve(&mut self, r: Round, pts: &[(f32, f32)], tension: f32) -> Result<(), Refused> {
        if pts.len() < 3 {
            return Err(Refused::InvalidParameter);
        }
        let b = spline(r, pts, tension, true);
        self.start_figure();
        self.add_beziers(r, &b)?;
        self.close_figure();
        Ok(())
    }
    fn has_curves(&self) -> bool {
        self.pts.iter().any(|p| p.kind == Kind::Bezier)
    }
    /// GetPathWorldBounds with no matrix and no pen: the points' box, all zeros for fewer than two
    pub fn bounds(&self, r: Round) -> (f32, f32, f32, f32) {
        hit(clause::GP_PATH_BOUNDS);
        if self.pts.len() < 2 {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let first = self.pts[0];
        let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
        for p in &self.pts[1..] {
            if x0 > p.x {
                x0 = p.x;
            } else if p.x > x1 {
                x1 = p.x;
            }
            if y0 > p.y {
                y0 = p.y;
            } else if p.y > y1 {
                y1 = p.y;
            }
        }
        // (CalcCacheBounds: a box under NEAR both ways is all zeros; GetBounds: the far corner,
        // back to a size, one under NEAR made 0)
        let (w, h) = (r.sub(x1, x0), r.sub(y1, y0));
        if w < NEAR && h < NEAR {
            return (0.0, 0.0, 0.0, 0.0);
        }
        let size = |a: f32, s: f32| {
            let w = r.sub(r.add(a, s), a);
            if w <= NEAR { 0.0 } else { w }
        };
        (x0, y0, size(x0, w), size(y0, h))
    }
    /// the figures: each point's index range
    pub fn figures(&self) -> Vec<&[Point]> {
        let mut v = Vec::new();
        let mut s = 0;
        for i in 1..=self.pts.len() {
            if i == self.pts.len() || self.pts[i].kind == Kind::Start {
                v.push(&self.pts[s..i]);
                s = i;
            }
        }
        v.retain(|f| !f.is_empty());
        v
    }
    /// FlattenPath
    pub fn flatten(&mut self, r: Round, m: Option<&Matrix>, flatness: f32) -> Res<()> {
        hit(clause::GP_PATH_FLATTEN);
        if !self.has_curves() {
            if let Some(m) = m {
                for p in &mut self.pts {
                    (p.x, p.y) = apply(r, m, p.x, p.y);
                }
            }
            return Ok(());
        }
        if flatness == 0.0 {
            // (16 / 0: every point the integer indefinite, the curves one point a segment, and
            // the scale by 0 back -0 everywhere)
            hit(clause::GP_PATH_FLATTEN_ZERO);
            let mut pts = Vec::new();
            for f in self.figures() {
                let mut i = 1;
                let mut out = vec![Kind::Start];
                while i < f.len() {
                    match f[i].kind {
                        Kind::Line | Kind::Start => i += 1,
                        Kind::Bezier => i += 3,
                    }
                    out.push(Kind::Line);
                }
                let close = f.last().is_some_and(|p| p.close);
                let n = out.len();
                let z = r.add(-0.0, 0.0);
                pts.extend(out.into_iter().enumerate().map(|(j, kind)| Point { x: z, y: z, kind, close: close && j + 1 == n, inner: false }));
            }
            self.pts = pts;
            self.new_figure = true;
            return Ok(());
        }
        let s = r.mul(flatness, 4.0);
        let k = r.div(16.0, s);
        let mm = scaled(r, m.unwrap_or(&[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]), k);
        let figs = enumerate(r, self, &mm)?;
        let back = [s, 0.0, 0.0, s, 0.0, 0.0];
        let mut out = Path::new(self.fill);
        for f in figs {
            let pts = f.pts.iter().map(|&(x, y)| (r.mul(x as f32, 0.0625), r.mul(y as f32, 0.0625))).collect::<Vec<_>>();
            out.start_figure();
            // (chunks of 32 overlap by a point AddLines drops: the same points)
            out.add_lines(r, &pts).map_err(|_| Fail::Bad)?;
            if f.closed {
                out.close_figure();
            }
        }
        for p in &mut out.pts {
            (p.x, p.y) = apply(r, &back, p.x, p.y);
        }
        out.new_figure = true;
        *self = Path { fill: self.fill, ..out };
        Ok(())
    }
}

/// NormalizeAngle: degrees to the ellipse's parameter in radians
fn normalize(r: Round, a: f32, w: f32, h: f32) -> f32 {
    let turns = |a: f32| r.sub(a, r.mul((r.div(a, 360.0) as i32) as f32, 360.0));
    let a = match a.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => turns(a),
        Some(std::cmp::Ordering::Less) => {
            let a = turns(-a);
            if a > 0.0 { r.sub(360.0, a) } else { a }
        }
        Some(std::cmp::Ordering::Equal) | None => 0.0,
    };
    let a = if !(0.0..=360.0).contains(&a) { 0.0 } else { a };
    let pi = std::f64::consts::PI;
    if w == h {
        return to32(r, div64(r, mul64(r, a as f64, pi), 180.0));
    }
    let (q, b) = if a <= 90.0 {
        (1, a)
    } else if a <= 180.0 {
        (2, r.sub(180.0, a))
    } else if a <= 270.0 {
        (3, r.sub(a, 180.0))
    } else {
        (4, r.sub(360.0, a))
    };
    let t = to32(r, div64(r, mul64(r, b as f64, pi), 180.0)) as f64;
    // (the atan2 stored rounded down on either: x86-64's MXCSR, x86's x87 control word)
    let f = to32(Round::Down, mul64(r, t.sin(), w as f64).atan2(mul64(r, h as f64, t.cos())));
    match q {
        1 => f,
        2 => r.sub(PI_F, f),
        3 => r.add(f, PI_F),
        _ => r.sub(TWO_PI_F, f),
    }
}

/// GetArcPoints: the Beziers of an arc (3 n + 1 points, n quarter pieces at most), none for a
/// sweep of 0, None for an empty rectangle
fn arc_points(r: Round, (x, y, w, h): (f32, f32, f32, f32), start: f32, sweep: f32) -> Option<Vec<(f32, f32)>> {
    hit(clause::GP_PATH_ARC);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    if sweep == 0.0 {
        return Some(Vec::new());
    }
    let dir: f32 = if sweep > 0.0 { 1.0 } else { -1.0 };
    let end = r.add(start, sweep);
    let mut st = normalize(r, start, w, h);
    let en = normalize(r, end, w, h);
    let mut span = if sweep.abs() >= 360.0 {
        TWO_PI_F
    } else {
        let s = if dir > 0.0 { r.sub(en, st) } else { r.sub(st, en) };
        if s < 0.0 { r.add(s, TWO_PI_F) } else { s }
    };
    let (rx, ry) = (r.mul(w, 0.5), r.mul(h, 0.5));
    let (cx, cy) = (r.add(rx, x), r.add(ry, y));
    let quarter = std::f64::consts::FRAC_PI_2;
    let mut n = (span as f64 / quarter) as i32;
    if span as f64 > n as f64 * quarter {
        n += 1;
    }
    let n = n.clamp(1, 4);
    let base = [rx, r.mul(rx, 0.0), r.mul(ry, 0.0), ry, r.add(r.add(r.mul(cy, 0.0), cx), 0.0), r.add(r.add(r.mul(cx, 0.0), cy), 0.0)];
    let mut out = Vec::new();
    for i in 0..n {
        let half = if span as f64 > quarter { QUARTER_PI_F } else { r.mul(span, 0.5) };
        let (c, s) = (cosf(r, half), sinf(r, half));
        let t = r.div(r.mul(r.sub(3.0, c), s), r.add(r.mul(c, 3.0), 3.0));
        let kx = r.div(r.sub(4.0, c), 3.0);
        let unit = if dir > 0.0 { [(c, -s), (kx, -t), (kx, t), (c, s)] } else { [(c, s), (kx, t), (kx, -t), (c, -s)] };
        let th = r.add(r.mul(dir, half), st);
        let th = r.mul(to32(r, div64(r, r.mul(th, 180.0) as f64, std::f64::consts::PI)), 0.017_453_292);
        let (sn, cs) = (sinf(r, th), cosf(r, th));
        let m = [
            r.add(r.mul(cs, base[0]), r.mul(sn, base[2])),
            r.add(r.mul(sn, base[3]), r.mul(cs, base[1])),
            r.sub(r.mul(cs, base[2]), r.mul(sn, base[0])),
            r.sub(r.mul(cs, base[3]), r.mul(sn, base[1])),
            base[4],
            base[5],
        ];
        let take = if i + 1 == n { 4 } else { 3 };
        out.extend(unit[..take].iter().map(|&(u, v)| apply(r, &m, u, v)));
        st = r.add(st, if dir > 0.0 { HALF_PI_F } else { -HALF_PI_F });
        span = r.add(span, -HALF_PI_F);
    }
    Some(out)
}

/// ConvertSplineToBezierPoints: a cardinal spline's Beziers
fn spline(r: Round, p: &[(f32, f32)], tension: f32, closed: bool) -> Vec<(f32, f32)> {
    hit(clause::GP_PATH_CURVE);
    let n = p.len();
    let t = r.div(tension, 3.0);
    let segs = if closed { n } else { n - 1 };
    let mut out = vec![p[0]];
    for i in 0..segs {
        let w = if closed { [p[(i + n - 1) % n], p[i], p[(i + 1) % n], p[(i + 2) % n]] } else { [p[i.saturating_sub(1)], p[i], p[(i + 1).min(n - 1)], p[(i + 2).min(n - 1)]] };
        let c1 = |a: fn(&(f32, f32)) -> f32| r.add(r.sub(a(&w[1]), r.mul(t, a(&w[0]))), r.mul(t, a(&w[2])));
        let c2 = |a: fn(&(f32, f32)) -> f32| r.sub(r.add(r.mul(t, a(&w[1])), a(&w[2])), r.mul(t, a(&w[3])));
        out.push((c1(|q| q.0), c1(|q| q.1)));
        out.push((c2(|q| q.0), c2(|q| q.1)));
        out.push(w[2]);
    }
    out
}

// ---- flattening in fixed point ----

/// one axis of GDI's 32-bit forward differencing
#[derive(Clone, Copy, Debug)]
struct Axis {
    x: i64,
    dx: i64,
    c: i64,
    p: i64,
}
fn bigger(a: i64, b: i64) -> i64 {
    if a.abs() > b.abs() { a } else { b }
}
impl Axis {
    fn new(v: [i64; 4]) -> Axis {
        Axis { x: v[0] << 10, dx: (v[3] - v[0]) << 10, c: (3 * (v[1] - 2 * v[2] + v[3])) << 11, p: (3 * (v[0] - 2 * v[1] + v[2])) << 11 }
    }
    fn step(&mut self) {
        self.x += self.dx;
        self.dx += self.c;
        (self.c, self.p) = (2 * self.c - self.p, self.c);
    }
    fn error(&self) -> i64 {
        bigger(self.c, self.p).abs()
    }
}

/// Bezier32: None past its 14-bit range (GDI takes Bezier64 there)
pub fn bezier32(q: [(i32, i32); 4]) -> Option<Vec<(i32, i32)>> {
    let ox = q.iter().map(|p| p.0).min()? as i64 - 16;
    let oy = q.iter().map(|p| p.1).min()? as i64 - 16;
    let xs = q.map(|p| p.0 as i64 - ox);
    let ys = q.map(|p| p.1 as i64 - oy);
    if xs.iter().chain(&ys).any(|v| !(0..=0x3fff).contains(v)) {
        return None;
    }
    let (mut a, mut b) = (Axis::new(xs), Axis::new(ys));
    let mut n: i64 = 1;
    let mut sh = 0;
    loop {
        let tol = 0x6000i64 << sh;
        if a.error() > tol || b.error() > tol {
            sh += 2;
            for e in [&mut a, &mut b] {
                e.c = (e.c + e.p) >> 1;
                e.dx = (e.dx - (e.c >> sh)) >> 1;
            }
            n <<= 1;
        } else {
            break;
        }
    }
    let rs = sh - 3;
    for e in [&mut a, &mut b] {
        e.x <<= 3;
        e.dx <<= 3;
        let f = |v: i64| if rs >= 0 { v >> rs } else { v << -rs };
        e.c = f(e.c);
        e.p = f(e.p);
        e.step();
    }
    n -= 1;
    let mut out = Vec::new();
    loop {
        out.push(((((a.x + 0x1000) >> 13) + ox) as i32, (((b.x + 0x1000) >> 13) + oy) as i32));
        if n == 0 {
            return Some(out);
        }
        if a.error().max(b.error()) > 0x30000 {
            for e in [&mut a, &mut b] {
                e.c = (e.c + e.p) >> 3;
                e.dx = (e.dx - e.c) >> 1;
                e.p >>= 2;
            }
            n *= 2;
        }
        while n % 2 == 0 {
            let ta = bigger(a.p, 2 * a.c - a.p);
            let tb = bigger(b.p, 2 * b.c - b.p);
            if ta.abs() > 0xc000 || tb.abs() > 0xc000 {
                break;
            }
            for e in [&mut a, &mut b] {
                e.p *= 4;
                e.dx = e.c + 2 * e.dx;
                e.c = 8 * e.c - e.p;
            }
            n >>= 1;
        }
        a.step();
        b.step();
        n -= 1;
    }
}

/// one axis of GDI's 64-bit hybrid forward differencing (28 fraction bits)
#[derive(Clone, Copy, Debug)]
struct Basis64 {
    e0: i128,
    e1: i128,
    e2: i128,
    e3: i128,
}
impl Basis64 {
    fn new(p: [i128; 4]) -> Basis64 {
        Basis64 { e0: p[0] << 28, e1: (p[3] - p[0]) << 28, e2: (3 * (p[1] - 2 * p[2] + p[3])) << 29, e3: (3 * (p[0] - 2 * p[1] + p[2])) << 29 }
    }
    fn err(&self) -> i128 {
        self.e2.abs().max(self.e3.abs())
    }
    fn parent_err(&self) -> i128 {
        (4 * self.e3).abs().max((8 * self.e2 - 4 * self.e3).abs())
    }
    fn halve(&mut self) {
        self.e2 = (self.e2 + self.e3) >> 3;
        self.e1 = (self.e1 - self.e2) >> 1;
        self.e3 >>= 2;
    }
    fn double(&mut self) {
        self.e3 <<= 2;
        self.e1 = self.e2 + 2 * self.e1;
        self.e2 = 8 * self.e2 - self.e3;
    }
    fn step(&mut self) {
        self.e0 += self.e1;
        self.e1 += self.e2;
        let t = self.e2;
        self.e2 = 2 * t - self.e3;
        self.e3 = t;
    }
    /// back to four integer control points (/ 18 truncating toward 0)
    fn untransform(&self) -> [i128; 4] {
        let half = 1i128 << 27;
        let r = 6 * self.e1 - self.e2;
        [(self.e0 + half) >> 28, (self.e0 + half + (r - 2 * self.e3) / 18) >> 28, (self.e0 + half + (2 * r - self.e3) / 18) >> 28, (self.e0 + self.e1 + half) >> 28]
    }
}

/// Bezier64: an outer pair of bases (tolerance 3 2^44) whose every step, made four points
/// again, seeds an inner pair (2^34) that gives the points
pub fn bezier64(q: [(i32, i32); 4]) -> Vec<(i32, i32)> {
    const T1: i128 = 3 << 44;
    const T2: i128 = 1 << 34;
    let (mut ox, mut oy) = (Basis64::new(q.map(|p| i128::from(p.0))), Basis64::new(q.map(|p| i128::from(p.1))));
    let mut n: i64 = 1;
    while ox.err() > T1 || oy.err() > T1 {
        n <<= 1;
        ox.halve();
        oy.halve();
    }
    let (mut ix, mut iy) = (ox, oy);
    let mut ni: i64 = 0;
    let mut out = Vec::new();
    loop {
        if ni == 0 {
            (ix, iy) = (Basis64::new(ox.untransform()), Basis64::new(oy.untransform()));
            ni = 1;
            while ix.err() > T2 || iy.err() > T2 {
                ni *= 2;
                ix.halve();
                iy.halve();
            }
            n -= 1;
            if n != 0 {
                ox.step();
                oy.step();
                if ox.err() > T1 || oy.err() > T1 {
                    n <<= 1;
                    ox.halve();
                    oy.halve();
                }
                while n % 2 == 0 && ox.parent_err() <= T1 && oy.parent_err() <= T1 {
                    ox.double();
                    oy.double();
                    n >>= 1;
                }
            }
        }
        ix.step();
        iy.step();
        let half = 1i128 << 27;
        out.push((((ix.e0 + half) >> 28) as i32, ((iy.e0 + half) >> 28) as i32));
        ni -= 1;
        if ni == 0 && n == 0 {
            return out;
        }
        if ix.err() > T2 || iy.err() > T2 {
            ni *= 2;
            ix.halve();
            iy.halve();
        }
        while ni % 2 == 0 && ix.parent_err() <= T2 && iy.parent_err() <= T2 {
            ix.double();
            iy.double();
            ni >>= 1;
        }
    }
}

/// a figure flattened in the integer space: its points (the start not repeated) and whether it
/// was closed
#[derive(Clone, Debug)]
pub struct Fig {
    pub pts: Vec<(i32, i32)>,
    pub closed: bool,
}

/// FixedPointPathEnumerate: each figure's points through m, ceiled to integers, its Beziers
/// flattened
pub fn enumerate(r: Round, path: &Path, m: &Matrix) -> Res<Vec<Fig>> {
    let conv = |p: &Point| {
        let (x, y) = apply(r, m, p.x, p.y);
        (ceil16(r, x), ceil16(r, y))
    };
    let mut figs = Vec::new();
    for f in path.figures() {
        let mut out = vec![conv(&f[0])];
        let mut i = 1;
        while i < f.len() {
            match f[i].kind {
                Kind::Line | Kind::Start => {
                    out.push(conv(&f[i]));
                    i += 1;
                }
                Kind::Bezier => {
                    if i + 2 >= f.len() {
                        return Err(Fail::Bad);
                    }
                    let prev = conv(&f[i - 1]);
                    let q = [prev, conv(&f[i]), conv(&f[i + 1]), conv(&f[i + 2])];
                    out.extend(bezier32(q).unwrap_or_else(|| {
                        hit(clause::GP_PATH_FLATTEN_BIG);
                        bezier64(q)
                    }));
                    i += 3;
                }
            }
        }
        figs.push(Fig { pts: out, closed: f.last().is_some_and(|p| p.close) });
    }
    Ok(figs)
}

// ---- the scan converter ----

/// how many of a pixel's samples a fill covers: 1 a pixel aliased, 32 (8 x 4) or 64 (8 x 8)
/// antialiased
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Coverage {
    pub w: u32,
    pub h: u32,
    pub full: u32,
    count: Vec<u32>,
}
impl Coverage {
    pub fn count(&self, x: u32, y: u32) -> u32 {
        self.count[(y * self.w + x) as usize]
    }
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    x: i64,
    dx: i64,
    err: i64,
    up: i64,
    down: i64,
    start: i64,
    end: i64,
    wind: i64,
}
impl Edge {
    fn new((x0, y0): (i64, i64), (x1, y1): (i64, i64)) -> Option<Edge> {
        let (wind, (sx, sy), mut dx, mut dy, ys, ye) = if y1 - y0 >= 0 { (1, (x0, y0), x1 - x0, y1 - y0, (y0 + 15) >> 4, (y1 + 15) >> 4) } else { (-1, (x1, y1), x0 - x1, y0 - y1, (y1 + 15) >> 4, (y0 + 15) >> 4) };
        if ye <= ys {
            return None;
        }
        let (q, rest) = (dx.div_euclid(dy), dx.rem_euclid(dy));
        let mut err = -1;
        let mut x = sx;
        let f = sy & 15;
        if f != 0 {
            for _ in 0..16 - f {
                err += rest;
                if err >= 0 {
                    err -= dy;
                    x += q + 1;
                } else {
                    x += q;
                }
            }
        }
        let f = x & 15;
        if f != 0 {
            err -= (16 - f) * dy;
            x += 15;
        }
        (dx, dy) = (q, dy);
        Some(Edge { x: x >> 4, dx, err: err >> 4, up: rest, down: dy, start: ys, end: ye, wind })
    }
    /// k rows down at once
    fn skip(&mut self, k: i64) {
        let e = self.err + self.up * k;
        let carries = (e + self.down).div_euclid(self.down);
        self.x += self.dx * k + carries;
        self.err = e - carries * self.down;
    }
    fn step(&mut self) {
        self.x += self.dx;
        self.err += self.up;
        if self.err >= 0 {
            self.err -= self.down;
            self.x += 1;
        }
    }
}

/// RasterizePath: the figures' 28.4 points closed, as edges, scanned in rows with the fill mode;
/// antialiased on a lattice of 8 x n subsamples a pixel
pub fn rasterize(figs: &[Fig], fm: FillMode, rows: Option<u32>, w: u32, h: u32) -> Coverage {
    hit(clause::GP_RASTER_FILL);
    let lattice = |(x, y): (i32, i32)| -> (i64, i64) {
        let (x, y) = (x as i64, y as i64);
        match rows {
            None => (x, y),
            Some(n) => (x * 8 + 0x40, (y + 8) << n.trailing_zeros()),
        }
    };
    let mut edges = Vec::new();
    for f in figs {
        let n = f.pts.len();
        for i in 0..n {
            if let Some(e) = Edge::new(lattice(f.pts[i]), lattice(f.pts[(i + 1) % n])) {
                edges.push(e);
            }
        }
    }
    let (sx, sy) = match rows {
        None => (1, 1),
        Some(n) => (8, n),
    };
    let (cols, lines) = (i64::from(w) * i64::from(sx), i64::from(h) * i64::from(sy));
    let mut count = vec![0u32; (w * h) as usize];
    let Some(first) = edges.iter().map(|e| e.start).min() else {
        return Coverage { w, h, full: sx * sy, count };
    };
    let first = first.max(0);
    let last = edges.iter().map(|e| e.end).max().unwrap_or(first).min(lines);
    edges.sort_by_key(|e| e.start);
    let mut pending = edges.into_iter().peekable();
    let mut active: Vec<Edge> = Vec::new();
    for y in first..last {
        while let Some(e) = pending.next_if(|e| e.start <= y) {
            if e.end > y {
                let mut e = e;
                e.skip(y - e.start);
                active.push(e);
            }
        }
        active.sort_by_key(|e| e.x);
        let mut cover = |a: i64, b: i64| {
            let (a, b) = (a.max(0), b.min(cols));
            for x in a..b {
                let (px, py) = (x / i64::from(sx), y / i64::from(sy));
                count[(py * i64::from(w) + px) as usize] += 1;
            }
        };
        match fm {
            FillMode::Alternate => {
                for pair in active.chunks(2) {
                    if let [a, b] = pair
                        && b.x > a.x
                    {
                        cover(a.x, b.x);
                    }
                }
            }
            FillMode::Winding => {
                let mut sum = 0;
                let mut from = 0;
                for e in &active {
                    let was = sum;
                    sum += e.wind;
                    if was == 0 && sum != 0 {
                        from = e.x;
                    } else if was != 0 && sum == 0 && e.x > from {
                        cover(from, e.x);
                    }
                }
            }
        }
        active.retain_mut(|e| {
            if e.end > y + 1 {
                e.step();
                true
            } else {
                false
            }
        });
    }
    Coverage { w, h, full: sx * sy, count }
}

/// the device matrix a fill rasterizes through: the world transform, the half pixel offset, then
/// 28.4 (each element times 16)
pub fn device(r: Round, world: Option<&Matrix>, po: PixelOffset) -> Matrix {
    scaled(r, &world_to_device(r, world, po), 16.0)
}
/// the world to device matrix (UpdateWorldToDeviceMatrix): the world transform, the half pixel
/// offset
pub fn world_to_device(r: Round, world: Option<&Matrix>, po: PixelOffset) -> Matrix {
    let mut m = *world.unwrap_or(&[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    if po.half() {
        m[4] = r.add(m[4], -0.5);
        m[5] = r.add(m[5], -0.5);
    }
    m
}

/// a path filled: every figure closed
#[allow(clippy::too_many_arguments)]
pub fn fill(r: Round, path: &Path, world: Option<&Matrix>, fm: FillMode, rows: Option<u32>, po: PixelOffset, w: u32, h: u32) -> Res<Coverage> {
    let figs = enumerate(r, path, &device(r, world, po))?;
    Ok(rasterize(&figs, fm, rows, w, h))
}

// ---- pens ----

/// a pen as GdipCreatePen1 and its setters leave it (every setter takes any value)
#[derive(Clone, PartialEq, Debug)]
pub struct Pen {
    pub color: u32,
    pub width: f32,
    pub dash: i64,
    pub start_cap: i64,
    pub end_cap: i64,
    pub dash_cap: i64,
    pub join: i64,
    pub align: i64,
    pub miter: f32,
    /// the dash array: GdipSetPenDashArray's (style Custom), or the one style 1 ..= 4 wrote
    /// into it; empty for none
    pub dash_array: Vec<f32>,
    /// GdipSetPenDashOffset's, in pen widths
    pub dash_offset: f32,
    /// GdipSetPenCompoundArray's array, empty for none
    pub compound: Vec<f32>,
    /// the custom cap objects GdipSetPenCustomStartCap and EndCap set
    pub custom: [Option<CustomCap>; 2],
}
/// a custom line cap object
#[derive(Clone, PartialEq, Debug)]
pub enum CustomCap {
    /// GdipCreateAdjustableArrowCap and its middle inset
    Arrow { height: f32, width: f32, filled: bool, middle: f32 },
    /// GdipCreateCustomLineCap of a fill (a polygon) or a stroke path (open lines), and its
    /// setters
    Path { stroke: bool, pts: Vec<(f32, f32)>, base_inset: f32, width_scale: f32, join: i64, caps: (i64, i64) },
}
impl Pen {
    pub fn new(color: u32, width: f32) -> Pen {
        Pen { color, width, dash: 0, start_cap: 0, end_cap: 0, dash_cap: 0, join: 0, align: 0, miter: 10.0, dash_array: Vec::new(), dash_offset: 0.0, compound: Vec::new(), custom: [None, None] }
    }
}

/// how a stroke of a straight line is drawn
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stroke {
    /// at most 1.5 wide, solid, opaque, no custom caps: aliased the one-pixel DDA, antialiased
    /// the line swept by a half pixel diamond
    OnePixel,
    /// at most 1.5 wide, solid but not opaque, no custom caps: the half pixel diamond line,
    /// aliased or antialiased
    Diamond,
    /// at most 1.5 wide, opaque, no custom caps, a dash pattern, aliased: the dashes cut in world
    /// space (GP.PEN.DASH), each by the one-pixel DDA without its last pixel
    DashedPixel,
    /// the widened outline (GP.PEN.WIDE) filled with Winding
    Wide,
}
/// the axes a world matrix stretches a unit circle to (GetMajorAndMinorAxis): (major, minor),
/// each at least NEAR, in the arch's rounding (MEASURED, golden gdiplusline world-8: a major
/// axis of 1.50000007 is 1.5 on x64, a one-pixel line, but 1.5000001 on x86, a widened one)
pub fn axes(r: Round, m: &Matrix) -> (f32, f32) {
    let sq = |v: f32| r.mul(v, v);
    let a = r.add(sq(m[1]), sq(m[0]));
    let b = r.add(r.mul(m[3], m[1]), r.mul(m[2], m[0]));
    let half = r.mul(r.sub(a, r.add(sq(m[2]), sq(m[3]))), 0.5);
    let r2 = r.add(sq(b), sq(half));
    let rr = if r2 > 0.0 { sqrt32(r, r2) } else { r2 };
    let mean = r.mul(r.add(r.add(sq(m[2]), a), sq(m[3])), 0.5);
    (NEAR.max(sqrt32(r, r.add(mean, rr))), NEAR.max(sqrt32(r, r.sub(mean, rr))))
}
/// which way a stroke goes (DpDriver::StrokePath)
pub fn stroke(r: Round, p: &Pen, world: Option<&Matrix>, aliased: bool, dashed: bool) -> Stroke {
    // (a world transform other than a translation scales the width by its major axis)
    let general = world.filter(|m| !translates_only(r, m));
    let device = match general {
        None => p.width,
        Some(m) => {
            hit(clause::GP_PEN_WORLD);
            r.mul(p.width, axes(r, m).0)
        }
    };
    let custom = (p.start_cap | p.end_cap) & 0xf0 != 0;
    let thin = device <= 1.5 && !custom;
    match (thin && p.dash == 0, p.color >> 24 == 0xff) {
        (true, true) => {
            hit(clause::GP_PEN_ONEPIXEL);
            Stroke::OnePixel
        }
        (true, false) => {
            hit(clause::GP_PEN_ONEPIXEL);
            Stroke::Diamond
        }
        (false, true) if thin && dashed && aliased => Stroke::DashedPixel,
        (false, _) => Stroke::Wide,
    }
}

/// a one-pixel aliased line (DrawSolidLineOnePixelAliased) through the device matrix (28.4,
/// GP.RASTER.FILL's device), clipped to the w x h bitmap
pub fn line_aliased(r: Round, m: &Matrix, (x1, y1): (f32, f32), (x2, y2): (f32, f32), w: u32, h: u32) -> Res<Coverage> {
    let snap = |x: f32, y: f32| {
        let (x, y) = apply(r, m, x, y);
        (ceil16(r, x), ceil16(r, y))
    };
    let mut cov = empty(w, h);
    dda(r, snap(x1, y1), snap(x2, y2), Last::Drawn, &mut cov)?;
    Ok(cov)
}

/// whether a one-pixel aliased segment draws its last pixel
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Last {
    Drawn,
    Skipped,
}

/// one segment between 28.4 points drawn by the aliased one-pixel DDA into cov, its last pixel
/// by last; a segment of no length (both ends one 28.4 point) draws nothing, its last pixel neither
/// (MEASURED on GDI+ itself, both arches: (1.95, 2) - (2, 2) empty)
fn dda(r: Round, p1: (i32, i32), p2: (i32, i32), last: Last, cov: &mut Coverage) -> Res<()> {
    let mut no_last = last == Last::Skipped;
    hit(clause::GP_PEN_ALIASED);
    let (w, h) = (cov.w, cov.h);
    let (p1, p2) = ((i64::from(p1.0), i64::from(p1.1)), (i64::from(p2.0), i64::from(p2.1)));
    if p1 == p2 {
        return Ok(());
    }
    let ymajor = (p2.1 - p1.1).abs() >= (p2.0 - p1.0).abs();
    // (major, minor)
    let mm = |p: (i64, i64)| if ymajor { (p.1, p.0) } else { (p.0, p.1) };
    let ((mut a_maj, mut a_min), (mut b_maj, mut b_min)) = (mm(p1), mm(p2));
    let reversed = b_maj < a_maj;
    if reversed {
        ((a_maj, a_min), (b_maj, b_min)) = ((b_maj, b_min), (a_maj, a_min));
    }
    let d: i64 = if b_min - a_min < 0 { -1 } else { 1 };
    let (dmaj, dmin) = (b_maj - a_maj, (b_min - a_min) * d);
    let slope = r.div((d * dmin.abs()) as f32, dmaj as f32);
    let invslope = if slope == 0.0 { 0.0 } else { r.div(1.0, slope) };
    // (the clip: the bitmap's pixel-centre edges, lo - 1/2 .. hi - 1/2, in 28.4)
    let (wmaj, wmin) = if ymajor { (i64::from(h), i64::from(w)) } else { (i64::from(w), i64::from(h)) };
    let (lo, hi) = (-8, wmaj * 16 - 8);
    let (nlo, nhi) = (-8, wmin * 16 - 8);
    if b_maj < lo || a_maj > hi {
        return Ok(());
    }
    let (mut o50, mut o54) = (0.0f32, 0.0f32);
    let mut clipped = false;
    if a_maj < lo {
        let v = r.add(r.mul((lo - a_maj) as f32, slope), a_min as f32);
        let f = v.floor();
        (a_min, a_maj, o50) = (f as i64, lo, r.sub(v, f));
        clipped = true;
    }
    if b_maj > hi {
        let v = r.add(r.mul((hi - b_maj) as f32, slope), b_min as f32);
        let f = v.floor();
        (b_min, b_maj, o54) = (f as i64, hi, r.sub(v, f));
        no_last = false;
        clipped = true;
    }
    // (the minor ends, low and high taken along d)
    let (start_edge, end_edge) = if d > 0 { (nlo, nhi) } else { (nhi, nlo) };
    let outside = |v: i64, edge: i64, start: bool| if (d > 0) == start { v < edge } else { v > edge };
    if outside(a_min, start_edge, true) {
        let k = r.mul(r.sub(start_edge as f32, r.add(a_min as f32, o50)), invslope).floor();
        (a_maj, a_min, o50) = (a_maj + k as i64, start_edge, 0.0);
        clipped = true;
    }
    if outside(b_min, end_edge, false) {
        let k = r.mul(r.sub(end_edge as f32, r.add(b_min as f32, o54)), invslope).floor();
        (b_maj, b_min, o54) = (b_maj + k as i64, end_edge, 0.0);
        no_last = false;
        clipped = true;
    }
    if b_maj < a_maj {
        return Ok(());
    }
    if o54 != 0.0 && b_min & 15 == 8 {
        b_min += 1;
    }
    let e3c = if o50 != 0.0 { r.mul((2 * dmaj * d) as f32, o50).floor() as i64 } else { 0 };
    let _ = clipped;
    // (SetupAliased)
    let e = !ymajor && dmaj == dmin && d == 1;
    let round = if e { 8 } else { 7 };
    let mut ebp = (a_maj + round) & !15;
    let mut esi = (b_maj + round) & !15;
    let inside = |r9: i64, r10: i64| {
        let s = r9.abs() + r10.abs();
        s < 8 || if ymajor { (r9 == 0 && r10 == 8) || (r10 == 0 && r9 == 8) } else { (r10 == 0 && r9 == if e { -8 } else { 8 }) || (r9 == 0 && r10 == 8) || (dmaj == dmin && s == 8 && (if e { r9 < 0 } else { r9 > 0 }) && r10 > 0) }
    };
    let start_in = inside(a_maj - ebp, a_min - ((a_min + 7) & !15));
    let end_in = inside(b_maj - esi, b_min - ((b_min + 7) & !15));
    if (reversed && no_last && (start_in || a_maj & 15 <= 8)) || (!(reversed && no_last) && a_maj & 15 <= 8 && !start_in) {
        ebp += 16;
    }
    let m0 = r.add(r.add(r.mul((ebp - a_maj) as f32, slope), a_min as f32), o50).floor() as i64;
    if (!reversed && no_last && (b_maj & 15 > 8 || end_in)) || (!(!reversed && no_last) && !end_in && b_maj & 15 > 8) {
        esi -= 16;
    }
    let m1 = r.add(r.add(r.mul((esi - b_maj) as f32, slope), b_min as f32), o54).floor() as i64;
    let (first, mut last) = (ebp >> 4, esi >> 4);
    if last < first {
        return Ok(());
    }
    let mut minor = ((m0 + 7) & !15) >> 4;
    let mut minor_end = (m1 + 7) >> 4;
    let mut err = (e3c - ((((m0 + 7) & !15) - m0) * d + 8) * 2 * dmaj) >> 4;
    // (the clip in pixels, inclusive)
    let (cmaj, cmin) = (wmaj - 1, wmin - 1);
    last = last.min(cmaj);
    minor_end = if d > 0 { minor_end.min(cmin) } else { minor_end.max(0) };
    let mut major = first;
    let step = |minor: &mut i64, err: &mut i64| {
        *err += 2 * dmin;
        if *err > 0 {
            *minor += d;
            *err -= 2 * dmaj;
            true
        } else {
            false
        }
    };
    while major <= last && (major < 0 || !(0..=cmin).contains(&minor)) {
        step(&mut minor, &mut err);
        major += 1;
    }
    let mut cnt = (minor_end - minor) * d;
    while major <= last {
        let (x, y) = if ymajor { (minor, major) } else { (major, minor) };
        if (0..i64::from(w)).contains(&x) && (0..i64::from(h)).contains(&y) {
            cov.count[(y * i64::from(w) + x) as usize] = 1;
        }
        if step(&mut minor, &mut err) {
            cnt -= 1;
            if cnt < 0 {
                break;
            }
        }
        major += 1;
    }
    Ok(())
}

fn empty(w: u32, h: u32) -> Coverage {
    Coverage { w, h, full: 1, count: vec![0; (w * h) as usize] }
}

/// a one-pixel antialiased line (RasterizePath, nominal width): the segment swept by a diamond
/// of half a pixel, filled as GP.RASTER.FILL fills
pub fn line_nominal(r: Round, m: &Matrix, (x1, y1): (f32, f32), (x2, y2): (f32, f32), rows: Option<u32>, w: u32, h: u32) -> Coverage {
    let snap = |x: f32, y: f32| {
        let (x, y) = apply(r, m, x, y);
        (ceil16(r, x), ceil16(r, y))
    };
    nominal(&[Fig { pts: vec![snap(x1, y1), snap(x2, y2)], closed: false }], rows, w, h)
}

/// the half pixel offsets along the minor axis, by octant class
const O: [(i32, i32); 4] = [(0, -8), (-8, 0), (0, 8), (8, 0)];
/// a segment's octant class: x-major right 0, left 2; y-major down 3, up 1 (a diagonal x-major)
fn class(p: (i32, i32), q: (i32, i32)) -> usize {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let idx = usize::from(dx < 0) | usize::from(dy < 0) << 1 | usize::from(dy.abs() > dx.abs()) << 2;
    [0, 2, 0, 2, 3, 3, 1, 1][idx]
}
/// InitializeNominal: a polyline (28.4) swept by the half pixel diamond, the forward chain then
/// the backward stack reversed; a polyline of one point its segment of no length, class 0: the
/// whole diamond (MEASURED on GDI+ itself, both arches: (1.95, 2) - (2, 2) antialiased one pixel
/// of alpha 0x80); None for no point
pub fn sweep(pts: &[(i32, i32)]) -> Option<Vec<(i32, i32)>> {
    let mut p: Vec<(i32, i32)> = Vec::with_capacity(pts.len());
    for &q in pts {
        if p.last() != Some(&q) {
            p.push(q);
        }
    }
    match p.len() {
        0 => return None,
        1 => p.push(p[0]),
        _ => {}
    }
    let add = |p: (i32, i32), q: (i32, i32)| (p.0 + q.0, p.1 + q.1);
    let sub = |p: (i32, i32), q: (i32, i32)| (p.0 - q.0, p.1 - q.1);
    let mut k = class(p[0], p[1]);
    let mut fwd = vec![sub(p[0], O[k]), add(p[0], O[(k + 1) & 3])];
    let mut back = Vec::new();
    for i in 0..p.len() - 1 {
        fwd.extend([add(p[i], O[k]), add(p[i + 1], O[k])]);
        back.extend([sub(p[i], O[k]), sub(p[i + 1], O[k])]);
        let Some(&next) = p.get(i + 2) else { break };
        let k2 = class(p[i + 1], next);
        if k2 == k {
            continue;
        }
        let ((x0, y0), (x1, y1), (x2, y2)) = (p[i], p[i + 1], next);
        let a = i64::from(x1 - x0) * i64::from(y2 - y1);
        let b = i64::from(y1 - y0) * i64::from(x2 - x1);
        if a < b {
            if k2 != (k + 1) & 3 {
                back.push(sub(p[i + 1], O[(k + 1) & 3]));
            }
            fwd.push(p[i + 1]);
        } else {
            if k2 != (k + 3) & 3 {
                fwd.push(add(p[i + 1], O[(k + 3) & 3]));
            }
            back.push(p[i + 1]);
        }
        k = k2;
    }
    let end = p[p.len() - 1];
    fwd.extend([add(end, O[(k + 3) & 3]), sub(end, O[k])]);
    fwd.extend(back.into_iter().rev());
    Some(fwd)
}
/// figures (28.4) stroked one pixel wide by the nominal rasterizer: each swept by the diamond
/// (a closed one with its closing segment), filled with Winding
pub fn nominal(figs: &[Fig], rows: Option<u32>, w: u32, h: u32) -> Coverage {
    hit(clause::GP_PEN_NOMINAL);
    let outlines: Vec<Fig> = figs
        .iter()
        .filter_map(|f| {
            let mut pts = f.pts.clone();
            if f.closed {
                pts.push(f.pts[0]);
            }
            sweep(&pts).map(|pts| Fig { pts, closed: true })
        })
        .collect();
    rasterize(&outlines, FillMode::Winding, rows, w, h)
}

/// a path stroked one pixel wide aliased by an opaque pen (SolidStrokePathOnePixel): each
/// segment by the DDA, its last pixel too, a closed figure's closing segment as well
pub fn outline_aliased(r: Round, path: &Path, m: &Matrix, w: u32, h: u32) -> Res<Coverage> {
    outline_dda(r, path, m, Last::Drawn, w, h)
}
/// SolidStrokePathOnePixel with draw-last-pixel given: each segment by the DDA, a closed
/// figure's (its closing segment too) always with its last pixel
pub fn outline_dda(r: Round, path: &Path, m: &Matrix, last: Last, w: u32, h: u32) -> Res<Coverage> {
    let mut cov = empty(w, h);
    for f in enumerate(r, path, m)? {
        let mut pts = f.pts.clone();
        if f.closed {
            pts.push(f.pts[0]);
        }
        for s in pts.windows(2) {
            dda(r, s[0], s[1], if f.closed { Last::Drawn } else { last }, &mut cov)?;
        }
    }
    Ok(cov)
}
/// a path stroked one pixel wide by the nominal rasterizer
pub fn outline_nominal(r: Round, path: &Path, m: &Matrix, rows: Option<u32>, w: u32, h: u32) -> Res<Coverage> {
    Ok(nominal(&enumerate(r, path, m)?, rows, w, h))
}
/// whether a path holds a Bezier point (strokes of such go the nominal way)
pub fn curved(path: &Path) -> bool {
    path.pts.iter().any(|p| matches!(p.kind, Kind::Bezier))
}

