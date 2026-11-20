//! Direct2D: an executable specification of d2d1.dll's matrix helpers, stroke styles, the
//! simple and path geometries (bounds, FillContainsPoint, ComputeArea, ComputeLength,
//! CompareWithGeometry, the counts, the geometry sink's states) and of drawing solid rectangles
//! and polygons into a 32bppPBGRA WIC bitmap render target.
//!
//! Sources (Microsoft Learn): "D2D1MakeRotateMatrix", "D2D1MakeSkewMatrix",
//! "D2D1InvertMatrix", "D2D1IsMatrixInvertible", "D2D1_STROKE_STYLE_PROPERTIES",
//! "ID2D1Factory::CreateStrokeStyle", "ID2D1StrokeStyle::GetDashes", "ID2D1Geometry::GetBounds",
//! "ID2D1Geometry::FillContainsPoint", "ID2D1Geometry::ComputeArea",
//! "ID2D1Geometry::ComputeLength", "ID2D1Geometry::CompareWithGeometry",
//! "D2D1_GEOMETRY_RELATION", "ID2D1PathGeometry::Open", "ID2D1PathGeometry::GetFigureCount",
//! "ID2D1SimplifiedGeometrySink::Close", "D2D1_FILL_MODE", "ID2D1RenderTarget::Clear",
//! "ID2D1RenderTarget::FillRectangle", "D2D1_ANTIALIAS_MODE", "Supported Pixel Formats and Alpha
//! Modes". Each clause the model applies is in spec/d2d.txt, measured ones with their golden.

use crate::spec::open;
use crate::spec::clause;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const D2DERR_WRONG_STATE: u32 = 0x8899_0001;
pub const E_NOINTERFACE: u32 = 0x8000_4002;

/// a float token: a decimal, or 0x<bits> (as the executor reads it)
pub fn fl(t: &str) -> Option<f32> {
    if let Some(h) = t.strip_prefix("0x") {
        return u32::from_str_radix(h, 16).ok().map(f32::from_bits);
    }
    t.parse::<f64>().ok().map(|v| v as f32)
}

pub fn hex(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// a 3x2 matrix: m11 m12 m21 m22 dx dy
pub type M = [f32; 6];
pub const IDENTITY: M = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// a point through m (row vector times matrix, as D2D1_MATRIX_3X2_F::TransformPoint)
pub fn apply(m: &M, x: f32, y: f32) -> (f32, f32) {
    (x * m[0] + y * m[2] + m[4], x * m[1] + y * m[3] + m[5])
}

/// a then b
pub fn mul(a: &M, b: &M) -> M {
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// D2D1InvertMatrix and D2D1IsMatrixInvertible: (invertible, the matrix after). MEASURED (golden
/// d2d-32/-64, all 55 matrices bit for bit): the reciprocal of the f32 determinant, invertible
/// when it is finite (so a subnormal determinant is not, an infinite one is), each element
/// times it.
pub fn invert(m: &M) -> (bool, M) {
    let [a, b, c, d, x, y] = *m;
    let i = 1.0 / (a * d - b * c);
    if !i.is_finite() {
        hit(clause::D2D_INV_SINGULAR);
        return (false, *m);
    }
    hit(clause::D2D_INV);
    (true, [d * i, -b * i, -c * i, a * i, (c * y - d * x) * i, (b * x - a * y) * i])
}

/// the x86 "indefinite" NaN an invalid operation gives
pub const INDEFINITE: u32 = 0xffc0_0000;

/// a f32 function computed in f64 of the f32 argument and rounded (D2D1SinCos, D2D1Tan:
/// MEASURED bit for bit on 43 angles): a NaN argument comes back, an infinite one gives the
/// indefinite NaN
pub fn via_f64(a: f32, f: fn(f64) -> f64) -> f32 {
    if a.is_nan() {
        return a;
    }
    if a.is_infinite() {
        return f32::from_bits(INDEFINITE);
    }
    f(a as f64) as f32
}

/// pi / 180 as f32
const DEG: f32 = std::f32::consts::PI / 180.0;

/// D2D1MakeRotateMatrix. MEASURED (golden d2d-32/-64, 60 angles bit for bit): the angle reduced
/// to 0 <= r < 360 in f32 (fmod, plus 360 when negative); a multiple of 90 degrees gives the
/// exact sine and cosine; else sin and cos (in f64, rounded) of r * (pi / 180 as f32);
/// dx = cx (1 - cos) - cy (-sin), dy = cy (1 - cos) - cx sin, in f32. A NaN or infinite angle
/// makes every element that NaN (the indefinite one for an infinity), m21 with its sign flipped.
pub fn rotate(angle: f32, cx: f32, cy: f32) -> M {
    if !angle.is_finite() {
        hit(clause::D2D_ROT_NAN);
        let n = if angle.is_nan() { angle.to_bits() } else { INDEFINITE };
        let v = f32::from_bits(n);
        return [v, v, f32::from_bits(n ^ 0x8000_0000), v, v, v];
    }
    let mut r = angle % 360.0;
    if r < 0.0 {
        r += 360.0;
    }
    let (s, c) = if r % 90.0 == 0.0 {
        hit(clause::D2D_ROT_RIGHT);
        [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0), (-1.0, 0.0)][(r / 90.0) as usize]
    } else {
        hit(clause::D2D_ROT);
        let t = (r * DEG) as f64;
        (t.sin() as f32, t.cos() as f32)
    };
    let (m11, m12, m21, m22) = (c, s, -s, c);
    [m11, m12, m21, m22, cx * (1.0 - m11) - cy * m21, cy * (1.0 - m22) - cx * m12]
}

/// D2D1MakeSkewMatrix. MEASURED (golden d2d-32/-64, 38 skews bit for bit): m12 tan(angleY), m21
/// tan(angleX) (f64 tan of the angle times pi / 180 as f32, rounded), the diagonal 1,
/// dx = cx - (cx + cy m21), dy = cy - (cx m12 + cy) in f32
pub fn skew(ax: f32, ay: f32, cx: f32, cy: f32) -> M {
    hit(clause::D2D_SKEW);
    let tx = via_f64(ax * DEG, f64::tan);
    let ty = via_f64(ay * DEG, f64::tan);
    let (m11, m12, m21, m22) = (1.0f32, ty, tx, 1.0f32);
    // (m11 and m22 are 1: cx m11 is cx)
    [m11, m12, m21, m22, cx - (cx + cy * m21), cy - (cx * m12 + cy)]
}

/// D2D1ConvertColorSpace. MEASURED (golden d2d-32/-64, cs-*): a space other than sRGB (1) or
/// scRGB (2) on either side gives four zeros; the same space gives the color as given; between
/// them the color channels are clamped to 0..1 (NaN to 0) and the alpha kept; 0 and 1 map to
/// themselves, the curve between is open (near the IEC 61966-2-1 formulas, off by up to 3 ulps)
pub fn convert_space(from: i64, to: i64, c: &[f32]) -> Res<Vec<f32>> {
    if !(1..=2).contains(&from) || !(1..=2).contains(&to) {
        hit(clause::D2D_CS_OTHER);
        return Ok(vec![0.0; 4]);
    }
    if from == to {
        hit(clause::D2D_CS_SAME);
        return Ok(c.to_vec());
    }
    let mut o = c.to_vec();
    for v in o.iter_mut().take(3) {
        if v.is_nan() || *v <= 0.0 {
            *v = 0.0;
        } else if *v >= 1.0 {
            *v = 1.0;
        } else {
            hit(clause::D2D_CS_CURVE);
            *v = if from == 1 { srgb_to_linear(*v) } else { linear_to_srgb(*v) };
        }
    }
    hit(clause::D2D_CS_CLAMP);
    Ok(o)
}

/// MEASURED (D2D.CS.CURVE, csd-1-2, csr-1-2): sRGB to scRGB in f32 as v / 12.92 up to 0.04045,
/// above it pow((v + 0.055) / 1.055, 2.4) with the quotient in f32 and the power correctly
/// rounded
pub fn srgb_to_linear(v: f32) -> f32 {
    if v > 0.04045 {
        (f64::from((v + 0.055) / 1.055).powf(f64::from(2.4f32))) as f32
    } else {
        v / 12.92
    }
}

/// MEASURED (D2D.CS.CURVE, csd-2-1, csr-2-1): scRGB to sRGB in f32 as v 12.92 below 0.0031308,
/// else 1.055 pow(v, 1 / 2.4) - 0.055, 1 / 2.4 an f32 and the power correctly rounded
pub fn linear_to_srgb(v: f32) -> f32 {
    if v < 0.0031308 {
        v * 12.92
    } else {
        1.055 * (f64::from(v).powf(f64::from(1.0f32 / 2.4)) as f32) - 0.055
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub pts: Vec<(f32, f32)>,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Geom {
    Rect([f32; 4]),
    Ell([f32; 4]),
    RRect([f32; 6]),
    Path { wind: bool, figs: Vec<Figure> },
    Xf(Box<Geom>, M),
}

impl Geom {
    /// the geometry as a set of figures and a transform, when it is made of straight lines
    fn lines(&self) -> Option<(bool, Vec<Figure>, M)> {
        match self {
            Geom::Rect([l, t, r, b]) => Some((false, vec![Figure { pts: vec![(*l, *t), (*r, *t), (*r, *b), (*l, *b)], closed: true }], IDENTITY)),
            Geom::Path { wind, figs } => Some((*wind, figs.clone(), IDENTITY)),
            Geom::Xf(g, m) => g.lines().map(|(w, f, m0)| (w, f, mul(&m0, m))),
            _ => None,
        }
    }
}

pub type P = (f32, f32);

/// a segment as a sink is given it: a line to a point, or a cubic Bezier through two control
/// points to a point
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Seg {
    L(P),
    C(P, P, P),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fig {
    pub start: P,
    pub segs: Vec<Seg>,
    pub closed: bool,
}

/// a geometry as Simplify hands it on with D2D1_GEOMETRY_SIMPLIFICATION_OPTION_CUBICS_AND_LINES:
/// the fill mode, whether the segments are flagged FORCE_ROUND_LINE_JOIN, and the figures
pub fn shape(g: &Geom) -> (bool, bool, Vec<Fig>) {
    match g {
        Geom::Rect([l, t, r, b]) => {
            // (MEASURED, simp-rect: the rectangle put in order, three lines from the top left
            // corner clockwise, the fourth side is the close)
            let (l, r, t, b) = (&lo(*l, *r), &hi(*l, *r), &lo(*t, *b), &hi(*t, *b));
            (false, false, vec![Fig { start: (*l, *t), segs: vec![Seg::L((*r, *t)), Seg::L((*r, *b)), Seg::L((*l, *b))], closed: true }])
        }
        Geom::Path { wind, figs } => (
            *wind,
            false,
            figs.iter().map(|f| Fig { start: f.pts[0], segs: f.pts[1..].iter().map(|&p| Seg::L(p)).collect(), closed: f.closed }).collect(),
        ),
        Geom::Ell([cx, cy, rx, ry]) => {
            // (MEASURED, simp-ell-*: from the left end clockwise, four cubics with a line of no
            // length (or an ulp) to the end of each of the first three; a negative radius counts
            // as its size. From the clean-room notes: an ellipse is the rounded rectangle of its
            // f32 box with the radii cut to half the box's sides)
            hit(clause::D2D_ARCS);
            let (rx, ry) = (rx.abs(), ry.abs());
            let (l, r, t, b) = (cx - rx, cx + rx, cy - ry, cy + ry);
            let half = |lo: f32, hi: f32| if lo > hi { 0.0 } else { (hi - lo) * 0.5 };
            let rx = if rx > half(l, r) { half(l, r) } else { rx };
            let ry = if ry > half(t, b) { half(t, b) } else { ry };
            (false, true, vec![rrect_fig(rrect_points(l, t, r, b, rx, ry))])
        }
        Geom::RRect([l, t, r, b, rx, ry]) => {
            // (MEASURED, simp-rrect-*: the rectangle put in order, each radius taken as its size
            // and cut to half the side; from the top of the left side clockwise, a cubic for
            // every corner and a line along every side, of no length when the radius is half)
            hit(clause::D2D_ARCS);
            let (l, r, t, b) = (lo(*l, *r), hi(*l, *r), lo(*t, *b), hi(*t, *b));
            (false, true, vec![rrect_fig(rrect_points(l, t, r, b, rx.abs(), ry.abs()))])
        }
        Geom::Xf(g, m) => {
            let (wind, round, figs) = shape(g);
            let a = |p: P| apply(m, p.0, p.1);
            let figs = figs
                .into_iter()
                .map(|f| Fig {
                    start: a(f.start),
                    segs: f.segs.iter().map(|s| match *s { Seg::L(p) => Seg::L(a(p)), Seg::C(p, q, r) => Seg::C(a(p), a(q), a(r)) }).collect(),
                    closed: f.closed,
                })
                .collect();
            (wind, round, figs)
        }
    }
}

/// MEASURED (D2D.FLATTEN, flat-*): a cubic is flattened by forward differencing in f32 on
/// halved and doubled steps (milcore's CBezierFlattener): the step is halved while the second
/// derivative at either end of it (six times the control points' second differences, largest
/// coordinate) exceeds six times the tolerance and the step is above 1/1024, after each step
/// halved again while the one at its end does, else doubled while both are within a quarter
/// of that and the step count allows; the points come after `p0`, the last one `p3` itself
/// the float CBezierFlattener works in: f32 for Simplify, f64 in the scanner's workspace
pub trait Flt: Copy + PartialOrd + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self> + std::ops::Mul<Output = Self> {
    const ZERO: Self;
    fn k(v: f32) -> Self;
    fn abs(self) -> Self;
}
impl Flt for f32 {
    const ZERO: Self = 0.0;
    fn k(v: f32) -> Self { v }
    fn abs(self) -> Self { f32::abs(self) }
}
impl Flt for f64 {
    const ZERO: Self = 0.0;
    fn k(v: f32) -> Self { f64::from(v) }
    fn abs(self) -> Self { f64::abs(self) }
}

pub fn flatten(p0: P, p1: P, p2: P, p3: P, tol: f32) -> Vec<P> {
    hit(clause::D2D_FLATTEN);
    let mut out: Vec<P> = flatten_in([[p0.0, p0.1], [p1.0, p1.1], [p2.0, p2.1], [p3.0, p3.1]], tol).into_iter().map(|v| (v[0], v[1])).collect();
    out.push(p3);
    out
}

/// the points CBezierFlattener makes between the ends of a cubic
pub fn flatten_in<T: Flt>(b: [[T; 2]; 4], tol: T) -> Vec<[T; 2]> {
    type V<T> = [T; 2];
    let sub = |a: V<T>, b: V<T>| [a[0] - b[0], a[1] - b[1]];
    let add = |a: V<T>, b: V<T>| [a[0] + b[0], a[1] + b[1]];
    let sc = |a: V<T>, k: f32| [a[0] * T::k(k), a[1] * T::k(k)];
    let norm = |a: V<T>| { let (x, y) = (a[0].abs(), a[1].abs()); if y > x { y } else { x } };
    let tol = if tol >= T::ZERO { tol } else { T::ZERO } * T::k(6.0);
    let quarter = tol * T::k(0.25);
    let [b0, b1, b2, b3] = b;
    let mut e0 = b0;
    let mut e1 = sub(b3, b0);
    let mut e2 = sc(add(sub(b1, sc(b2, 2.0)), b3), 6.0);
    let mut e3 = sc(add(sub(b0, sc(b1, 2.0)), b2), 6.0);
    let mut steps: u32 = 1;
    let mut size: f64 = 1.0;
    const MIN: f64 = 1.0 / 1024.0;
    let halve = |e1: &mut V<T>, e2: &mut V<T>, e3: &mut V<T>, steps: &mut u32, size: &mut f64| {
        *e2 = sc(add(*e2, *e3), 0.125);
        *e1 = sc(sub(*e1, *e2), 0.5);
        *e3 = sc(*e3, 0.25);
        *steps *= 2;
        *size *= 0.5;
    };
    // (a NaN compares false: no halving)
    while (norm(e2) > tol || norm(e3) > tol) && size > MIN {
        halve(&mut e1, &mut e2, &mut e3, &mut steps, &mut size);
    }
    let mut out = Vec::new();
    while steps > 1 {
        e0 = add(e0, e1);
        let pt = e2;
        e1 = add(e1, pt);
        e2 = sub(add(e2, pt), e3);
        e3 = pt;
        out.push(e0);
        steps -= 1;
        if norm(e2) > tol && size > MIN {
            halve(&mut e1, &mut e2, &mut e3, &mut steps, &mut size);
        } else {
            while steps % 2 == 0 {
                let tmp = sub(sc(e2, 2.0), e3);
                if !(norm(e3) <= quarter && norm(tmp) <= quarter) {
                    break;
                }
                e1 = add(sc(e1, 2.0), e2);
                e3 = sc(e3, 4.0);
                e2 = sc(tmp, 4.0);
                steps /= 2;
                size *= 2.0;
            }
        }
    }
    out
}

/// the figures with every cubic flattened (Simplify's LINES option)
pub fn flat_figs(figs: &[Fig], tol: f32) -> Vec<Fig> {
    figs.iter()
        .map(|f| {
            let mut cur = f.start;
            let mut segs = Vec::new();
            for s in &f.segs {
                match *s {
                    Seg::L(p) => {
                        segs.push(Seg::L(p));
                        cur = p;
                    }
                    Seg::C(a, b, c) => {
                        segs.extend(flatten(cur, a, b, c, tol).into_iter().map(Seg::L));
                        cur = c;
                    }
                }
            }
            Fig { start: f.start, segs, closed: f.closed }
        })
        .collect()
}

/// the geometry as straight-line figures and a transform: curves flattened with `tol`
pub fn flat_lines(g: &Geom, tol: f32) -> (bool, Vec<Figure>, M) {
    if let Some(l) = g.lines() {
        return l;
    }
    let (wind, _, figs) = shape(g);
    let figs = flat_figs(&figs, tol)
        .into_iter()
        .map(|f| Figure { pts: std::iter::once(f.start).chain(f.segs.iter().map(|s| match *s { Seg::L(p) | Seg::C(_, _, p) => p })).collect(), closed: f.closed })
        .collect();
    (wind, figs, IDENTITY)
}

/// D2D1_GEOMETRY_SIMPLIFICATION_OPTION, as the executor names it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Simplification {
    CubicsAndLines,
    Lines,
}

impl Simplification {
    fn parse(t: Option<&str>) -> Res<Simplification> {
        match t {
            Some("cubics") => Ok(Simplification::CubicsAndLines),
            Some("lines") => Ok(Simplification::Lines),
            _ => Err(Fail::Bad),
        }
    }
}

/// what a sink is told for a shape, as the executor prints it
pub fn sink_text(wind: bool, round: bool, figs: &[Fig]) -> String {
    let mut o = format!(" F {}", wind as u32);
    if round {
        o += " S 2";
    }
    for f in figs {
        o += &format!(" B {} {} 0", hex(f.start.0), hex(f.start.1));
        for s in &f.segs {
            match *s {
                Seg::L(p) => o += &format!(" L {} {}", hex(p.0), hex(p.1)),
                Seg::C(a, b, c) => o += &format!(" C {} {} {} {} {} {}", hex(a.0), hex(a.1), hex(b.0), hex(b.1), hex(c.0), hex(c.1)),
            }
        }
        o += &format!(" E {}", f.closed as u32);
    }
    o
}

/// the smaller and larger of two floats as SSE's minss / maxss pick them (a NaN in the second
/// place comes back)
fn lo(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}
fn hi(a: f32, b: f32) -> f32 {
    if a > b { a } else { b }
}

/// MEASURED: bounds through a transform of anything with a NaN, or of an ellipse with a NaN
/// even without one, are four NaNs with every bit set
const NAN_BOUNDS: [f32; 4] = [f32::from_bits(!0); 4];

/// MEASURED (D2D.BOUNDS.ELLXF/RRECTXF: every value of cbounds/ell-xf/rrect-xf/xfb-*): milcore's CBounds in floats: the bounds of the
/// transformed points, and of each cubic's value where a coordinate's derivative is 0; the
/// derivative's zeros are those of a(1-t)^2 + 2b t(1-t) + c t^2 (a, b, c the control points'
/// differences in f32), solved as a quadratic in s = (1-t)/t (|a| > |c|) or t/(1-t) in f64,
/// each positive root kept as an f32 and turned into t in f32 (none when |b| 1e-6 in f32 is above
/// both |a| and |c|); the value is
/// a s^3 + 3b t s^2 + 3c t^2 s + d t^3 in f32
fn curve_zeros(a: f32, b: f32, c: f32, d: f32) -> Vec<f32> {
    if (b - a) * (d - b) >= 0.0 && (c - a) * (d - c) >= 0.0 {
        return vec![];
    }
    let (a, b, c) = (b - a, c - b, d - c);
    let fuzz = b.abs() * f32::from_bits(0x358637bd);
    if fuzz > a.abs() && fuzz > c.abs() {
        return vec![];
    }
    let roots = |a: f32, b: f32, c: f32| -> Vec<f32> {
        let (a, b, c) = (f64::from(a), f64::from(b), f64::from(c));
        let d = b * b - a * c;
        if !(d > 0.0) {
            return vec![];
        }
        let d = d.sqrt();
        [(-b - d) / a, (-b + d) / a].into_iter().map(|r| r as f32).filter(|&r| r > 0.0).collect()
    };
    if a.abs() > c.abs() {
        roots(a, b, c).into_iter().map(|r| 1.0 / (1.0 + r)).collect()
    } else {
        roots(c, b, a).into_iter().map(|r| r / (1.0 + r)).collect()
    }
}
fn cubic_at(a: f32, b: f32, c: f32, d: f32, t: f32) -> f32 {
    let (t2, s) = (t * t, 1.0 - t);
    let s2 = s * s;
    a * s * s2 + 3.0 * b * t * s2 + 3.0 * c * t2 * s + d * t * t2
}

fn curve_bounds(g: &Geom, m: &M) -> Res<[f32; 4]> {
    let (_, _, figs) = shape(g);
    let tp = |p: P| apply(m, p.0, p.1);
    let mut b = [f32::MAX, f32::MAX, -f32::MAX, -f32::MAX];
    let mut nan = false;
    let upd = |b: &mut [f32; 4], p: P| {
        if p.0 < b[0] { b[0] = p.0 }
        if p.0 > b[2] { b[2] = p.0 }
        if p.1 < b[1] { b[1] = p.1 }
        if p.1 > b[3] { b[3] = p.1 }
    };
    for f in &figs {
        let mut cur = tp(f.start);
        upd(&mut b, cur);
        for sg in &f.segs {
            match *sg {
                Seg::L(p) => {
                    cur = tp(p);
                    upd(&mut b, cur);
                }
                Seg::C(p1, p2, p3) => {
                    let (q1, q2, q3) = (tp(p1), tp(p2), tp(p3));
                    upd(&mut b, q3);
                    nan |= q1.0.is_nan() || q1.1.is_nan() || q2.0.is_nan() || q2.1.is_nan();
                    for t in curve_zeros(cur.0, q1.0, q2.0, q3.0) {
                        let x = cubic_at(cur.0, q1.0, q2.0, q3.0, t);
                        if x < b[0] { b[0] = x } else if x > b[2] { b[2] = x }
                        nan |= x.is_nan();
                    }
                    for t in curve_zeros(cur.1, q1.1, q2.1, q3.1) {
                        let y = cubic_at(cur.1, q1.1, q2.1, q3.1, t);
                        if y < b[1] { b[1] = y } else if y > b[3] { b[3] = y }
                        nan |= y.is_nan();
                    }
                    cur = q3;
                }
            }
        }
    }
    if nan {
        return Ok(NAN_BOUNDS);
    }
    hit(match g {
        Geom::RRect(_) => clause::D2D_BOUNDS_RRECTXF,
        _ => clause::D2D_BOUNDS_ELLXF,
    });
    Ok(b)
}

/// GetBounds, `m` the world transform (None: NULL)
pub fn bounds(g: &Geom, m: Option<&M>) -> Res<[f32; 4]> {
    match (g, m) {
        (Geom::Rect([l, t, r, b]) | Geom::RRect([l, t, r, b, ..]), None) => {
            // (MEASURED: the rectangle put in order, a rounded one's radii not mattering)
            hit(if matches!(g, Geom::Rect(_)) { clause::D2D_BOUNDS_RECT } else { clause::D2D_BOUNDS_RRECT });
            if [l, t, r, b].iter().any(|v| v.is_nan()) {
                hit(clause::D2D_BOUNDS_NAN);
            }
            Ok([lo(*l, *r), lo(*t, *b), hi(*l, *r), hi(*t, *b)])
        }
        (Geom::RRect(_), Some(m)) => curve_bounds(g, m),
        (Geom::Rect(_) | Geom::Path { .. }, _) => {
            let (_, figs, m0) = g.lines().unwrap();
            let m = mul(&m0, m.unwrap_or(&IDENTITY));
            // (MEASURED: a path with a NaN anywhere has four NaN bounds, every bit set)
            if matches!(g, Geom::Path { .. }) && figs.iter().flat_map(|f| f.pts.iter()).any(|p| p.0.is_nan() || p.1.is_nan()) {
                hit(clause::D2D_BOUNDS_PATHNAN);
                return Ok(NAN_BOUNDS);
            }
            let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
            hit(if matches!(g, Geom::Rect(_)) { clause::D2D_BOUNDS_RECTXF } else { clause::D2D_BOUNDS_PATH });
            let (mut nx, mut ny) = (false, false);
            for f in &figs {
                for &(x, y) in &f.pts {
                    let (x, y) = apply(&m, x, y);
                    nx |= x.is_nan();
                    ny |= y.is_nan();
                    b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
                }
            }
            // (MEASURED: an axis with a NaN corner has both its bounds NaN, every bit set)
            if nx {
                hit(clause::D2D_BOUNDS_NAN);
                (b[0], b[2]) = (NAN_BOUNDS[0], NAN_BOUNDS[0]);
            }
            if ny {
                hit(clause::D2D_BOUNDS_NAN);
                (b[1], b[3]) = (NAN_BOUNDS[0], NAN_BOUNDS[0]);
            }
            Ok(b)
        }
        (Geom::Ell([cx, cy, rx, ry]), _) => {
            let m = m.unwrap_or(&IDENTITY);
            if [cx, cy, rx, ry].iter().any(|v| v.is_nan()) {
                hit(clause::D2D_BOUNDS_NAN);
                return Ok(NAN_BOUNDS);
            }
            if m[1] != 0.0 || m[2] != 0.0 || [cx, cy, rx, ry].iter().any(|v| !v.is_finite()) {
                return curve_bounds(g, m);
            }
            // (MEASURED: a negative radius counts as its size)
            hit(clause::D2D_BOUNDS_ELL);
            let (rx, ry) = (rx.abs(), ry.abs());
            let (x0, y0) = apply(m, cx - rx, cy - ry);
            let (x1, y1) = apply(m, cx + rx, cy + ry);
            Ok([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
        }
        (Geom::Xf(inner, m0), _) => {
            // (the inner geometry's bounds through both transforms, the geometry's first; an
            // ellipse's or rounded rectangle's as D2D.BOUNDS.ELLXF says, xfell-*)
            hit(clause::D2D_XF);
            bounds(inner, Some(&mul(m0, m.unwrap_or(&IDENTITY))))
        }
    }
}

fn segs(p: &[(f64, f64)]) -> Vec<((f64, f64), (f64, f64))> {
    (0..p.len()).map(|i| (p[i], p[(i + 1) % p.len()])).collect()
}

fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64)) -> f64 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

/// the figures as polygons (f64, transformed), each closed for filling
fn polys(figs: &[Figure], m: &M) -> Vec<Vec<(f64, f64)>> {
    figs.iter().map(|f| f.pts.iter().map(|&(x, y)| {
        let (x, y) = apply(m, x, y);
        (x as f64, y as f64)
    }).collect()).collect()
}

/// the winding number of p (rays to +x; p on no side)
fn winding(ps: &[Vec<(f64, f64)>], p: (f64, f64)) -> i32 {
    let mut w = 0;
    for poly in ps {
        for (a, b) in segs(poly) {
            if a.1 <= p.1 && b.1 > p.1 && cross(a, b, p) > 0.0 {
                w += 1;
            } else if b.1 <= p.1 && a.1 > p.1 && cross(a, b, p) < 0.0 {
                w -= 1;
            }
        }
    }
    w
}

fn dist_seg(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx * dx + dy * dy;
    let t = if l == 0.0 { 0.0 } else { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l).clamp(0.0, 1.0) };
    ((a.0 + t * dx - p.0).powi(2) + (a.1 + t * dy - p.1).powi(2)).sqrt()
}

/// FillContainsPoint
pub fn contains(g: &Geom, x: f32, y: f32, tol: f32) -> Res<bool> {
    let p = (x as f64, y as f64);
    let ftol = if tol == 0.0 { 0.25 } else { tol };
    let (wind, figs, m) = flat_lines(g, ftol);
    let curved = g.lines().is_none();
    if curved {
        hit(if matches!(g, Geom::RRect(_)) { clause::D2D_IN_RRECT } else { clause::D2D_IN_ELL });
    }
    let ps = polys(&figs, &m);
    let d = ps.iter().flat_map(|q| segs(q)).map(|(a, b)| dist_seg(p, a, b)).fold(f64::INFINITY, f64::min);
    // (MEASURED: a tolerance of 0 is the default 0.25; inside when strictly nearer than it)
    let tol = if tol == 0.0 { hit(clause::D2D_IN_TOL0); 0.25 } else { tol as f64 };
    hit(clause::D2D_IN_TOL);
    if curved && d < tol * 2.0 {
        hit(clause::D2D_IN_ELLNEAR);
    }
    if d < tol {
        return Ok(true);
    }
    // (MEASURED: 0.2505 from a side with the tolerance 0.25 is out)
    if d < tol + 1e-3 {
        hit(clause::D2D_IN_EDGE);
    }
    let w = winding(&ps, p);
    hit(if wind { clause::D2D_IN_WIND } else { clause::D2D_IN_ALT });
    Ok(if wind { w != 0 } else { w % 2 != 0 })
}

/// the area the fill mode fills: the figures (open ones as closed) cut into slabs at every
/// vertex and crossing, in each slab the inside spans by the winding at its middle, trapezoids
pub fn region_area(ps: &[Vec<(f64, f64)>], wind: bool) -> f64 {
    region_area2(ps, wind, &[], false, |a, _| a)
}

type Edge = ((f64, f64), (f64, f64));

/// the area where `keep(in a, in b)` holds, a and b each by its own fill mode (as region_area)
pub fn region_area2(a: &[Vec<(f64, f64)>], wa: bool, b: &[Vec<(f64, f64)>], wb: bool, keep: impl Fn(bool, bool) -> bool) -> f64 {
    let tagged = |ps: &[Vec<(f64, f64)>], tag: bool| -> Vec<(Edge, bool)> { ps.iter().flat_map(|p| segs(p)).filter(|(p, q)| p.1 != q.1).map(|e| (e, tag)).collect() };
    let mut edges = tagged(a, false);
    edges.extend(tagged(b, true));
    let mut ys: Vec<f64> = a.iter().chain(b.iter()).flatten().map(|p| p.1).collect();
    for &((a, b), _) in &edges {
        for &((c, d), _) in &edges {
            let den = (b.0 - a.0) * (d.1 - c.1) - (b.1 - a.1) * (d.0 - c.0);
            if den == 0.0 {
                continue;
            }
            let t = ((c.0 - a.0) * (d.1 - c.1) - (c.1 - a.1) * (d.0 - c.0)) / den;
            let u = ((c.0 - a.0) * (b.1 - a.1) - (c.1 - a.1) * (b.0 - a.0)) / den;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                ys.push(a.1 + t * (b.1 - a.1));
            }
        }
    }
    ys.sort_by(f64::total_cmp);
    ys.dedup();
    let inside = |n: i32, wind: bool| if wind { n != 0 } else { n % 2 != 0 };
    let mut area = 0.0;
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        let ym = (y0 + y1) / 2.0;
        let x = |e: &Edge, y: f64| e.0 .0 + (y - e.0 .1) * (e.1 .0 - e.0 .0) / (e.1 .1 - e.0 .1);
        let mut act: Vec<&(Edge, bool)> = edges.iter().filter(|(e, _)| e.0 .1.min(e.1 .1) <= y0 && e.0 .1.max(e.1 .1) >= y1).collect();
        act.sort_by(|p, q| x(&p.0, ym).total_cmp(&x(&q.0, ym)));
        let (mut na, mut nb, mut w0, mut w1, mut from): (i32, i32, f64, f64, Option<&Edge>) = (0, 0, 0.0, 0.0, None);
        for (e, tag) in act {
            let was = keep(inside(na, wa), inside(nb, wb));
            let dir = (e.1 .1 - e.0 .1).signum() as i32;
            if *tag {
                nb += dir;
            } else {
                na += dir;
            }
            let now = keep(inside(na, wa), inside(nb, wb));
            if !was && now {
                from = Some(e);
            } else if was && !now {
                let f = from.unwrap_or(e);
                w0 += x(e, y0) - x(f, y0);
                w1 += x(e, y1) - x(f, y1);
            }
        }
        area += (w0 + w1) / 2.0 * (y1 - y0);
    }
    area
}

/// ComputeArea's workspace: every point, control points too, scaled by 2^20 about the origin
/// and rounded half away from zero to an integer (f64); the scale back is 2^-20 per axis
const AREA_SCALE: f64 = 1048576.0;

/// a point in the area workspace: (v 2^20 +- 0.5) truncated
fn area_snap(v: f64) -> f64 {
    let u = v * AREA_SCALE;
    (if u >= 0.0 { u + 0.5 } else { u - 0.5 }).trunc()
}

/// an ellipse's or rounded rectangle's figure from its 16 points: a cubic per corner, a line
/// along every side
fn rrect_fig(p: [P; 16]) -> Fig {
    let mut segs = Vec::new();
    for k in 0..4 {
        segs.push(Seg::C(p[4 * k + 1], p[4 * k + 2], p[4 * k + 3]));
        if k < 3 {
            segs.push(Seg::L(p[4 * k + 4]));
        }
    }
    Fig { start: p[0], segs, closed: true }
}

/// the f32 points of an ellipse or rounded rectangle (D2D.ARCS, from the clean-room notes): the rectangle's
/// corners and its radii (each cut to half its side), one minus kappa times each radius in f64
/// rounded to f32, from the top of the left side clockwise: [start, then per corner a cubic's
/// three points and the side's end]
fn rrect_points(l: f32, t: f32, r: f32, b: f32, rx: f32, ry: f32) -> [P; 16] {
    const IN: f64 = 0.44771525016920655;
    let rx = (0.5 * (r - l)).min(rx);
    let ry = (0.5 * (b - t)).min(ry);
    let kx = (f64::from(rx) * IN) as f32;
    let ky = (f64::from(ry) * IN) as f32;
    [
        (l, t + ry), (l, t + ky), (l + kx, t), (l + rx, t),
        (r - rx, t), (r - kx, t), (r, t + ky), (r, t + ry),
        (r, b - ry), (r, b - ky), (r - kx, b), (r - rx, b),
        (l + rx, b), (l + kx, b), (l, b - ky), (l, b - ry),
    ]
}

/// the figures ComputeArea's scanner sweeps, in workspace units, and the fill mode (winding);
/// None for a geometry the analysis does not cover yet
fn area_figures(g: &Geom, tol: f32, ext: f64) -> Option<(bool, Vec<Vec<(f64, f64)>>)> {
    let ws = |p: P| (area_snap(f64::from(p.0)), area_snap(f64::from(p.1)));
    let (wind, _, figs) = match g {
        Geom::Ell(_) | Geom::RRect(_) => shape(g),
        Geom::Path { wind, figs } => {
            return Some((*wind, figs.iter().map(|f| f.pts.iter().map(|&p| ws(p)).collect()).collect()));
        }
        Geom::Rect(_) | Geom::Xf(..) => return None,
    };
    // (the tolerance: 0.25 when not above 0, at least 1e-6, at least 3e-12 of the bounds'
    // larger side, in workspace units)
    let tol = if tol > 0.0 { tol.max(f32::from_bits(0x358637bd)) } else { 0.25 };
    let tol = AREA_SCALE * f64::from(tol).max(ext * 3e-12);
    // (the flattened points are rounded to integers too, half away from zero)
    let r = |u: f64| (if u >= 0.0 { u + 0.5 } else { u - 0.5 }).trunc();
    let mut all = Vec::new();
    for f in &figs {
        let mut out = vec![ws(f.start)];
        for sg in &f.segs {
            match *sg {
                Seg::L(p) => out.push(ws(p)),
                Seg::C(p1, p2, p3) => {
                    let p0 = out[out.len() - 1];
                    let b = [p0, ws(p1), ws(p2), ws(p3)].map(|p| [p.0, p.1]);
                    out.extend(flatten_in(b, tol).into_iter().map(|v| (r(v[0]), r(v[1]))));
                    out.push(ws(p3));
                }
            }
        }
        all.push(out);
    }
    Some((wind, all))
}

/// whether two figures' edges cross or touch anywhere but at shared ends of neighbours (exact
/// for integers below 2^26)
fn simple_polygon(p: &[(f64, f64)]) -> bool {
    let n = p.len();
    let e: Vec<Edge> = (0..n).map(|i| (p[i], p[(i + 1) % n])).filter(|(a, b)| a != b).collect();
    let m = e.len();
    for i in 0..m {
        for j in i + 1..m {
            if j == i + 1 || (i == 0 && j == m - 1) {
                continue;
            }
            let ((a, b), (c, d)) = (e[i], e[j]);
            if meet(a, b, c, d) {
                return false;
            }
        }
    }
    true
}

/// ComputeArea of what is not a rectangle. MEASURED (D2D.AREA.POLY, from the clean-room notes on
/// CArea, the scanner's workspace; crossing figures D2D.AREA.CURVE, open): points snapped to the 2^-20 grid (area_snap), curves
/// flattened there in f64 and their points snapped; the area is half the sum of the cross
/// products of the filled region's boundary edges, times 2^-20 2^-20, in f64, as an f32
fn scanner_area(g: &Geom, tol: f32) -> Res<Measure> {
    let bb = bounds(g, None)?;
    let ext = (f64::from(bb[2]) - f64::from(bb[0])).max(f64::from(bb[3]) - f64::from(bb[1]));
    // (MEASURED, ell-area-4: bounds not finite fail with D2DERR_BAD_NUMBER, the area untouched)
    if !ext.is_finite() {
        return Ok(Measure::BadNumber);
    }
    // (beyond about 1.8e8 the workspace is centered and scaled to 2^48 instead: not modelled)
    let big = 2f64.powi(48);
    let inside = (f64::from(bb[0]) - 1.0) * AREA_SCALE * 1.5 > -big && (f64::from(bb[1]) - 1.0) * AREA_SCALE * 1.5 > -big && (f64::from(bb[2]) + 1.0) * AREA_SCALE * 1.5 < big && (f64::from(bb[3]) + 1.0) * AREA_SCALE * 1.5 < big;
    let Some((wind, figs)) = area_figures(g, tol, ext).filter(|_| inside) else {
        return Err(Fail::Unmodeled(open::D2D_AREA_CURVE));
    };
    let inv = 1.0 / AREA_SCALE;
    let a = if let [f] = figs.as_slice() && simple_polygon(f) {
        let n = f.len();
        (0..n).map(|i| f[i].0 * f[(i + 1) % n].1 - f[(i + 1) % n].0 * f[i].1).sum::<f64>().abs() * 0.5
    } else {
        // (crossings: the intersection points are not on the grid and the order of the sum is
        // not settled; the area of what the fill mode fills, in f64)
        super::approx(open::D2D_AREA_CURVE)?;
        region_area(&figs, wind)
    };
    hit(clause::D2D_AREA_POLY);
    Ok(Measure::Value(((inv * inv) * a) as f32))
}

/// what ComputeArea / ComputeLength answer
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Measure {
    Value(f32),
    /// D2DERR_BAD_NUMBER, the out value left as it was
    BadNumber,
}

/// ComputeArea. MEASURED (golden d2d-32/-64, 66 paths of up to three figures, self-crossing and
/// overlapping ones too): the area the fill mode fills, open figures filled as closed
pub fn area(g: &Geom, tol: f32) -> Res<Measure> {
    if let Geom::Rect([l, t, r, b]) = g {
        hit(clause::D2D_AREA_RECT);
        return Ok(Measure::Value(((r - l) * (b - t)).abs()));
    }
    if let Geom::Xf(..) = g {
        // (MEASURED, xbounds-*: a rectangle through a transform, exact)
        let Some((wind, figs, m)) = g.lines() else {
            return Err(Fail::Unmodeled(open::D2D_AREA_CURVE));
        };
        hit(clause::D2D_AREA_POLY);
        return Ok(Measure::Value(region_area(&polys(&figs, &m), wind) as f32));
    }
    scanner_area(g, tol)
}

/// ComputeLength. MEASURED (golden d2d-32/-64, 66 paths bit for bit): each side's length as
/// sqrt(dx dx + dy dy) in f32, summed in f32 in order over every figure, the closing side of a
/// closed figure included
pub fn length(g: &Geom, tol: f32) -> Res<f32> {
    let (_, figs, m) = flat_lines(g, tol);
    if g.lines().is_none() {
        hit(clause::D2D_LEN_CURVE);
    }
    hit(clause::D2D_LEN);
    let mut sum = 0f32;
    for f in &figs {
        let p: Vec<(f32, f32)> = f.pts.iter().map(|&(x, y)| apply(&m, x, y)).collect();
        let n = p.len();
        for i in 0..if f.closed { n } else { n - 1 } {
            let (a, b) = (p[i], p[(i + 1) % n]);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            sum += (dx * dx + dy * dy).sqrt();
        }
    }
    Ok(sum)
}

/// CompareWithGeometry of two rectangles (D2D1_GEOMETRY_RELATION: 1 disjoint, 2 is contained, 3
/// contains, 4 overlap)
/// whether two closed segments have a point in common
fn meet(p: (f64, f64), q: (f64, f64), r: (f64, f64), t: (f64, f64)) -> bool {
    let (d1, d2, d3, d4) = (cross(r, t, p), cross(r, t, q), cross(p, q, r), cross(p, q, t));
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    let on = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| cross(a, b, c) == 0.0 && dist_seg(c, a, b) == 0.0;
    on(r, t, p) || on(r, t, q) || on(p, q, r) || on(p, q, t)
}

/// CompareWithGeometry of any two geometries: by the areas of their flattenings (0.25) and of
/// what both fill
fn shape_relation(a: &Geom, b: &Geom) -> Res<u32> {
    let poly = |g: &Geom| {
        let (w, f, m) = flat_lines(g, 0.25);
        (polys(&f, &m), w)
    };
    let ((pa, wa), (pb, wb)) = (poly(a), poly(b));
    let both = region_area2(&pa, wa, &pb, wb, |x, y| x && y);
    let (aa, ab) = (region_area(&pa, wa), region_area(&pb, wb));
    // (MEASURED, cmpg-12, -19, -28: outlines that only touch, at a point or along a side, are
    // OVERLAP, as closed sets)
    let touch = pa.iter().flat_map(|p| segs(p)).any(|(p, q)| pb.iter().flat_map(|r| segs(r)).any(|(r, t)| meet(p, q, r, t)));
    hit(clause::D2D_CMP_SHAPE);
    Ok(if both == 0.0 && !touch {
        1
    } else if both == 0.0 {
        4
    } else if both == aa {
        2
    } else if both == ab {
        3
    } else {
        4
    })
}

pub fn relation(a: &Geom, b: &Geom) -> Res<u32> {
    let (Geom::Rect(a), Geom::Rect(b)) = (a, b) else { return shape_relation(a, b) };
    let n = |r: &[f32; 4]| [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
    let (a, b) = (n(a), n(b));
    hit(clause::D2D_CMP);
    // (MEASURED: as closed sets, no tolerance: touching is OVERLAP, equal is IS_CONTAINED, a
    // gap of 1/8 is DISJOINT)
    let inside = |p: [f32; 4], q: [f32; 4]| p[0] >= q[0] && p[1] >= q[1] && p[2] <= q[2] && p[3] <= q[3];
    Ok(if a[2] < b[0] || b[2] < a[0] || a[3] < b[1] || b[3] < a[1] {
        1
    } else if inside(a, b) {
        2
    } else if inside(b, a) {
        3
    } else {
        4
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
enum SinkState {
    #[default]
    New,
    Open,
    Closed,
    Failed,
}

#[derive(Default)]
struct Sink {
    made: bool,
    sink: bool,
    state: SinkState,
    infig: bool,
    error: bool,
    figs: u32,
    segs: u32,
    open_segs: u32,
}

/// a 32bppPBGRA picture
#[derive(Clone)]
pub struct Target {
    pub w: usize,
    pub h: usize,
    /// B G R A each
    pub px: Vec<[u8; 4]>,
    pub st: DrawState,
    /// dots per inch, x and y
    pub dpi: (f32, f32),
    /// the drawing state block, once made: what its last save held (None: never saved)
    pub block: Option<Option<DrawState>>,
    /// the clips and layers pushed, innermost last
    pub stack: Vec<Pushed>,
    /// the error state's error, once in it
    pub fault: Option<Fault>,
    /// the pixels and the stack at BeginDraw (an error state throws the frame's drawing away)
    pub begin: (Vec<[u8; 4]>, Vec<Pushed>),
    /// pushes less pops in this frame
    pub depth: i64,
    /// the bitmaps made on this target, by slot
    pub bitmaps: [Option<Bitmap>; 4],
}

/// a premultiplied B8G8R8A8 bitmap
#[derive(Clone, PartialEq, Debug)]
pub struct Bitmap {
    pub w: usize,
    pub h: usize,
    pub px: Vec<[u8; 4]>,
}

/// D2D1_EXTEND_MODE
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Extend {
    Clamp,
    Wrap,
    Mirror,
}

impl Extend {
    fn parse(t: &str) -> Option<Self> {
        match t {
            "clamp" => Some(Self::Clamp),
            "wrap" => Some(Self::Wrap),
            "mirror" => Some(Self::Mirror),
            _ => None,
        }
    }
    /// texel i of a row or column of n (n > 0) by this mode
    fn index(self, i: i64, n: usize) -> usize {
        let n = n as i64;
        let k = match self {
            Self::Clamp => i.clamp(0, n - 1),
            Self::Wrap => i.rem_euclid(n),
            Self::Mirror => {
                let p = i.rem_euclid(2 * n);
                if p < n { p } else { 2 * n - 1 - p }
            }
        };
        k as usize
    }
}

/// which axis a linear sample lerps along first (D2D.BITMAP.LINEAR)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Lerp {
    ColumnsFirst,
    /// down the columns kept to 7 more bits (floored), then across, floored (bbrushwx-*)
    Weighed,
    RowsFirst,
}

/// D2D1_BITMAP_INTERPOLATION_MODE
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interp {
    Nearest,
    Linear,
}

impl Interp {
    fn parse(t: &str) -> Option<Self> {
        match t {
            "nearest" => Some(Self::Nearest),
            "linear" => Some(Self::Linear),
            _ => None,
        }
    }
}

/// where a pixel center falls in a bitmap: DrawBitmap's destination rectangle onto its source
/// rectangle, or through the inverse of a bitmap brush's transform (to pixels)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Place {
    Rect { dst: [f32; 4], src: [f32; 4] },
    /// the brush's matrix times the world's, and the world's alone
    Brush { m: M, world: M },
}

/// what a fill puts down at each pixel (premultiplied B G R A)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Paint<'a> {
    Solid([u8; 4]),
    Bitmap { bmp: &'a Bitmap, place: Place, ext: (Extend, Extend), interp: Interp, opacity: u32 },
}

/// a byte scaled by k / 255 rounded
fn scale_by(p: [u8; 4], k: u32) -> [u8; 4] {
    p.map(|b| ((u32::from(b) * k + 127) / 255) as u8)
}

impl Paint<'_> {
    /// MEASURED (D2D.BITMAP.NEAREST, D2D.BITMAP.LINEAR, D2D.BBRUSH): the color at pixel (x, y)
    fn at(&self, x: usize, y: usize) -> [u8; 4] {
        match *self {
            Paint::Solid(c) => c,
            Paint::Bitmap { bmp, place, ext, interp, opacity } => texel_at(bmp, place, ext, interp, opacity, x, y),
        }
    }
}

fn texel_at(bmp: &Bitmap, place: Place, ext: (Extend, Extend), interp: Interp, opacity: u32, x: usize, y: usize) -> [u8; 4] {
    {
        let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
        let (u, v) = match place {
            Place::Rect { dst, src } => {
                let [dl, dt, dr, db] = dst.map(f64::from);
                let [sl, st, sr, sb] = src.map(f64::from);
                (sl + (cx - dl) * (sr - sl) / (dr - dl), st + (cy - dt) * (sb - st) / (db - dt))
            }
            Place::Brush { m, .. } => {
                let m = m.map(f64::from);
                let det = m[0] * m[3] - m[1] * m[2];
                let (px, py) = (cx - m[4], cy - m[5]);
                ((m[3] * px - m[2] * py) / det, (-m[1] * px + m[0] * py) / det)
            }
        };
        // (MEASURED, bbrush-7 against bbrushx-7: an axis aligned mapping lerps down the
        // columns first, a mapping that mixes x into v or y into u lerps along the rows first)
        let order = match place {
            Place::Rect { .. } => Lerp::ColumnsFirst,
            // (MEASURED, bbrushfit-xf1: a world transform that shears, neither keeping nor
            // swapping the axes, weighs the four texels at once)
            Place::Brush { world, .. } if (world[1] != 0.0 || world[2] != 0.0) && (world[0] != 0.0 || world[3] != 0.0) => Lerp::Weighed,
            Place::Brush { m, .. } if m[1] == 0.0 && m[2] == 0.0 => Lerp::ColumnsFirst,
            Place::Brush { .. } => Lerp::RowsFirst,
        };
        let texel = |i: i64, j: i64| bmp.px[ext.1.index(j, bmp.h) * bmp.w + ext.0.index(i, bmp.w)];
        let c = match interp {
            Interp::Nearest => texel(u.floor() as i64, v.floor() as i64),
            Interp::Linear => {
                // (the texel centers at +0.5, the fractions in 1/256 rounded, a lerp along one
                // axis and then the other, each rounded))
                let (u, v) = (u - 0.5, v - 0.5);
                let (mut i, mut j) = (u.floor() as i64, v.floor() as i64);
                let (mut fu, mut fv) = (((u - u.floor()) * 256.0 + 0.5).floor() as u32, ((v - v.floor()) * 256.0 + 0.5).floor() as u32);
                if fu == 256 {
                    fu = 0;
                    i += 1;
                }
                if fv == 256 {
                    fv = 0;
                    j += 1;
                }
                let lerp = |a: u32, b: u32, f: u32| (a * (256 - f) + b * f + 128) / 256;
                let (a, b, c, d) = (texel(i, j), texel(i + 1, j), texel(i, j + 1), texel(i + 1, j + 1));
                std::array::from_fn(|k| {
                    let (a, b, c, d) = (u32::from(a[k]), u32::from(b[k]), u32::from(c[k]), u32::from(d[k]));
                    match order {
                        Lerp::ColumnsFirst => lerp(lerp(a, c, fv), lerp(b, d, fv), fu) as u8,
                        Lerp::RowsFirst => lerp(lerp(a, b, fu), lerp(c, d, fu), fv) as u8,
                        Lerp::Weighed => (((a * (256 - fv) + c * fv) >> 1) * (256 - fu) + ((b * (256 - fv) + d * fv) >> 1) * fu >> 15) as u8,
                    }
                })
            }
        };
        if opacity == 255 {
            c
        } else {
            hit(clause::D2D_BITMAP_OPACITY);
            scale_by(c, opacity)
        }
    }
}

pub const D2DERR_POP_CALL_DID_NOT_MATCH_PUSH: u32 = 0x8899_0014;
pub const D2DERR_PUSH_POP_UNBALANCED: u32 = 0x8899_0016;
pub const D2DERR_LAYER_ALREADY_IN_USE: u32 = 0x8899_0013;

/// D2D1_ANTIALIAS_MODE
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Antialias {
    PerPrimitive,
    Aliased,
}

impl Antialias {
    fn parse(t: &str) -> Option<Self> {
        match t {
            "per" => Some(Self::PerPrimitive),
            "aliased" => Some(Self::Aliased),
            _ => None,
        }
    }
    /// the enum's value
    fn code(self) -> u32 {
        match self {
            Self::PerPrimitive => 0,
            Self::Aliased => 1,
        }
    }
}

/// what a drawing state block saves and restores
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DrawState {
    pub m: M,
    pub aa: Antialias,
    pub tags: (u64, u64),
}

/// a new target's state, and a never saved block's
pub const FRESH: DrawState = DrawState { m: IDENTITY, aa: Antialias::PerPrimitive, tags: (0, 0) };

/// a pushed axis-aligned clip: the rectangle in pixels (the bounds of the given one through the
/// transform when pushed), its antialias mode
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Clip {
    pub r: [f32; 4],
    pub aa: Antialias,
    pub edge: ClipEdge,
}

/// how an antialiased clip covers a pixel
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClipEdge {
    /// pushed under an axis preserving transform: 8 x 8 samples (D2D.CLIP.BLEND)
    Samples,
    /// pushed under a turning transform: the bounds' edges floored to 1/256 pixel and the
    /// covered area (D2D.CLIP.XF)
    Area,
}

/// the error state: its error, and whether EndDraw throws the frame's drawing away
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fault {
    pub hr: u32,
    pub drawn: Drawn,
}

/// what an error state does to what the frame drew before it
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drawn {
    Kept,
    Dropped,
}

/// a pushed layer: the pixels under it (drawing goes into a new transparent surface), its
/// content bounds in pixels, its opacity byte, its mask's coverage per pixel
#[derive(Clone, PartialEq, Debug)]
pub struct Layer {
    pub below: Vec<[u8; 4]>,
    pub opacity: u32,
    /// the coverage byte of the content bounds and the mask together, per pixel
    pub cover: Vec<u32>,
}

/// what PushAxisAlignedClip and PushLayer push; each pop takes its own kind off the top
#[derive(Clone, PartialEq, Debug)]
pub enum Pushed {
    Clip(Clip),
    Layer(Layer),
}

/// the bounds of a rectangle's corners through a transform
fn corner_bounds(m: &M, r: [f32; 4]) -> [f32; 4] {
    let cs = [apply(m, r[0], r[1]), apply(m, r[2], r[1]), apply(m, r[2], r[3]), apply(m, r[0], r[3])];
    let lo = |v: [f32; 4]| v.into_iter().fold(f32::INFINITY, f32::min);
    let hi = |v: [f32; 4]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
    let (xs, ys) = (cs.map(|p| p.0), cs.map(|p| p.1));
    [lo(xs), lo(ys), hi(xs), hi(ys)]
}

/// a pixel center inside a rectangle by the fill rule (left and top edges in)
fn center_in(r: [f32; 4], x: usize, y: usize) -> bool {
    let [l, t, rr, b] = r.map(f64::from);
    let (cx, cy) = (x as f64 + 0.5, y as f64 + 0.5);
    l <= cx && cx < rr && t <= cy && cy < b
}

impl Layer {
    /// MEASURED (D2D.LAYER): the layer's surface `top` put over what was under it
    fn composite(&self, top: &[[u8; 4]], w: usize, h: usize) -> Vec<[u8; 4]> {
        let mut px = self.below.clone();
        for i in 0..w * h {
            if self.cover[i] != 0 {
                px[i] = over(scale_by(scale_by(top[i], self.opacity), self.cover[i]), self.below[i]);
            }
        }
        px
    }
}

impl Target {
    /// MEASURED (D2D.RT.FAULT, layer-pop5): what EndDraw leaves in the bitmap with layers still
    /// pushed: inside a layer's bounds what was drawn into it, as it is (transparent where
    /// nothing was), outside them what was under it
    fn view(&self) -> Vec<[u8; 4]> {
        let mut px = self.px.clone();
        for p in self.stack.iter().rev() {
            if let Pushed::Layer(l) = p {
                for i in 0..self.w * self.h {
                    if l.cover[i] == 0 {
                        px[i] = l.below[i];
                    }
                }
            }
        }
        px
    }

    /// MEASURED (D2D.RT.FAULT): the error state; the first error stays
    fn fail(&mut self, hr: u32, drawn: Drawn) {
        hit(clause::D2D_RT_FAULT);
        self.fault.get_or_insert(Fault { hr, drawn });
    }

    pub fn new(w: usize, h: usize) -> Self {
        Target { w, h, px: vec![[0; 4]; w * h], st: FRESH, dpi: (96.0, 96.0), block: None, stack: Vec::new(), fault: None, begin: (vec![[0; 4]; w * h], Vec::new()), depth: 0, bitmaps: [None, None, None, None] }
    }

    /// MEASURED (D2D.CLIP.BLEND): what a drawing call did to the pixels (`before` -> now) kept
    /// in each clip: aliased, where the pixel center is inside (the fill rule's edges); per
    /// primitive, the coverage n of 8 x 8 samples as k = (n 4 255 + 127) >> 8 and each byte
    /// (before (255 - k) + now k + 126) / 255 (clipfit-0: an exact half rounds down)
    fn keep_clipped(&mut self, before: &[[u8; 4]]) {
        // (MEASURED, layer-clip: drawing into a layer goes through the clips pushed after it;
        // the clips under it come at its pop)
        let top = self.stack.iter().rposition(|p| matches!(p, Pushed::Layer(_))).map_or(0, |i| i + 1);
        let clips: Vec<Clip> = self.stack[top..].iter().filter_map(|p| match p {
            Pushed::Clip(c) => Some(*c),
            Pushed::Layer(_) => None,
        }).collect();
        for c in &clips {
            let [l, t, r, b] = c.r.map(f64::from);
            let inside = |x: f64, y: f64| l <= x && x < r && t <= y && y < b;
            for y in 0..self.h {
                for x in 0..self.w {
                    let (fx, fy) = (x as f64, y as f64);
                    let k: u32 = match c.aa {
                        Antialias::Aliased => {
                            if center_in(c.r, x, y) { 255 } else { 0 }
                        }
                        Antialias::PerPrimitive if c.edge == ClipEdge::Area => {
                            // (MEASURED, clipxf-*: 4091 of 4096 pixels; four corner pixels a
                            // step higher)
                            let q = |v: f64| (v * 256.0).floor() / 256.0;
                            let (ql, qt, qr, qb) = (q(l), q(t), q(r), q(b));
                            let cx = (qr.min(fx + 1.0) - ql.max(fx)).max(0.0);
                            let cy = (qb.min(fy + 1.0) - qt.max(fy)).max(0.0);
                            (255.0 * cx * cy).round() as u32
                        }
                        Antialias::PerPrimitive => {
                            let mut n = 0u32;
                            for j in 0..8 {
                                for i in 0..8 {
                                    n += u32::from(inside(fx + (f64::from(i) + 0.5) / 8.0, fy + (f64::from(j) + 0.5) / 8.0));
                                }
                            }
                            Samples::Mask.byte(n)
                        }
                    };
                    let i = y * self.w + x;
                    let (d, f) = (before[i], self.px[i]);
                    self.px[i] = std::array::from_fn(|c| ((u32::from(d[c]) * (255 - k) + u32::from(f[c]) * k + 126) / 255) as u8);
                }
            }
        }
    }
}

/// a straight color (r, g, b, a as 0..255) as the premultiplied bytes Clear and an opaque brush
/// write, B G R A
pub fn premultiply(c: [i64; 4]) -> [u8; 4] {
    premultiply_o(c, 1.0)
}

/// MEASURED (D2D.OPACITY): a brush's opacity multiplies the alpha in f32 before premultiplying
pub fn premultiply_o(c: [i64; 4], o: f32) -> [u8; 4] {
    let f = |v: i64| v as f32 / 255.0;
    // brush opacity is clamped to 0..1 before it scales the alpha (D2D.OPACITY)
    let a = f(c[3]) * o.clamp(0.0, 1.0);
    let ch = |v: i64| ((f(v) * a) * 255.0).round_ties_even() as u8;
    [ch(c[2]), ch(c[1]), ch(c[0]), (a * 255.0).round_ties_even() as u8]
}


/// source over: a premultiplied source over a premultiplied destination
pub fn over(s: [u8; 4], d: [u8; 4]) -> [u8; 4] {
    let inv = 255 - s[3] as u32;
    let mut o = [0u8; 4];
    for i in 0..4 {
        o[i] = (s[i] as u32 + (d[i] as u32 * inv + 127) / 255).min(255) as u8;
    }
    o
}

#[derive(Default)]
pub struct D2d {
    g: [Option<Geom>; 8],
    sink: Sink,
    t: Option<Target>,
}

fn f(op: &Op, i: usize) -> Res<f32> {
    op.tok.get(i).and_then(|t| fl(t)).ok_or(Fail::Bad)
}
fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn mat(op: &Op, at: usize) -> Res<M> {
    let mut m = [0f32; 6];
    for (k, v) in m.iter_mut().enumerate() {
        *v = f(op, at + k)?;
    }
    Ok(m)
}
fn mhex(m: &M) -> String {
    m.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" ")
}

impl D2d {
    fn slot(op: &Op, i: usize) -> Res<usize> {
        let g = n(op, i)?;
        if !(0..8).contains(&g) {
            return Err(Fail::Bad);
        }
        Ok(g as usize)
    }

    /// the geometry in slot token i; None when there is none ("nogeom" printed)
    fn geom(&self, op: &Op, i: usize, out: &mut Vec<String>) -> Res<Option<Geom>> {
        let g = Self::slot(op, i)?;
        if self.g[g].is_none() {
            out.push("nogeom".into());
        }
        Ok(self.g[g].clone())
    }

    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = &op.tok;
        let len = t.len();
        let want = |k: usize| if len == k { Ok(()) } else { Err(Fail::Bad) };
        match t[0].as_str() {
            "d2rot" => {
                let (a, cx, cy) = (f(op, 1)?, f(op, 2)?, f(op, 3)?);
                want(4)?;
                out.push(format!("rot {}", mhex(&rotate(a, cx, cy))));
            }
            "d2skew" => {
                let (ax, ay, cx, cy) = (f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?);
                want(5)?;
                out.push(format!("skew {}", mhex(&skew(ax, ay, cx, cy))));
            }
            "d2inv" => {
                let m = mat(op, 1)?;
                want(7)?;
                let (ok, r) = invert(&m);
                out.push(format!("inv {} {} {}", ok as u32, ok as u32, mhex(&r)));
            }
            "d2sincos" | "d2tan" => {
                let a = f(op, 1)?;
                want(2)?;
                hit(clause::D2D_TRIG);
                out.push(if t[0] == "d2tan" {
                    format!("tan {}", hex(via_f64(a, f64::tan)))
                } else {
                    format!("sincos {} {}", hex(via_f64(a, f64::sin)), hex(via_f64(a, f64::cos)))
                });
            }
            "d2vec3" => {
                let (x, y, z) = (f(op, 1)?, f(op, 2)?, f(op, 3)?);
                want(4)?;
                // (MEASURED: in f32, so overflowing and underflowing squares stay so)
                hit(clause::D2D_VEC3);
                out.push(format!("vec3 {}", hex((x * x + y * y + z * z).sqrt())));
            }
            "d2cs" => {
                let (from, to) = (n(op, 1)?, n(op, 2)?);
                let c: Vec<f32> = (3..7).map(|i| f(op, i)).collect::<Res<_>>()?;
                want(7)?;
                out.push(format!("cs {}", convert_space(from, to, &c)?.iter().map(|v| hex(*v)).collect::<Vec<_>>().join(" ")));
            }
            "d2stroke" => self.stroke(op, out)?,
            "d2rect" | "d2ell" | "d2rrect" => {
                let g = Self::slot(op, 1)?;
                let k = if t[0] == "d2rrect" { 6 } else { 4 };
                want(2 + k)?;
                let v: Vec<f32> = (0..k).map(|i| f(op, 2 + i)).collect::<Res<_>>()?;
                hit(clause::D2D_CREATE);
                self.g[g] = Some(match t[0].as_str() {
                    "d2rect" => Geom::Rect(v.try_into().unwrap()),
                    "d2ell" => Geom::Ell(v.try_into().unwrap()),
                    _ => Geom::RRect(v.try_into().unwrap()),
                });
                out.push(format!("{} 00000000", &t[0][2..]));
            }
            "d2xf" => {
                let g = Self::slot(op, 1)?;
                let src = self.geom(op, 2, out)?;
                let m = mat(op, 3)?;
                want(9)?;
                let Some(src) = src else { return Ok(true) };
                hit(clause::D2D_XF);
                self.g[g] = Some(Geom::Xf(Box::new(src), m));
                out.push("xf 00000000".into());
            }
            "d2path" => {
                let g = Self::slot(op, 1)?;
                let wind = match t.get(2).map(String::as_str) {
                    Some("wind") => true,
                    Some("alt") => false,
                    _ => return Err(Fail::Bad),
                };
                let mut figs: Vec<Figure> = Vec::new();
                let mut infig = false;
                let mut i = 3;
                while i < len {
                    match t[i].as_str() {
                        "m" => {
                            if infig {
                                return Err(Fail::Bad);
                            }
                            figs.push(Figure { pts: vec![(f(op, i + 1)?, f(op, i + 2)?)], closed: false });
                            infig = true;
                            i += 3;
                        }
                        "z" | "o" => {
                            if !infig {
                                return Err(Fail::Bad);
                            }
                            figs.last_mut().unwrap().closed = t[i] == "z";
                            infig = false;
                            i += 1;
                        }
                        _ => {
                            if !infig {
                                return Err(Fail::Bad);
                            }
                            figs.last_mut().unwrap().pts.push((f(op, i)?, f(op, i + 1)?));
                            i += 2;
                        }
                    }
                }
                if infig {
                    return Err(Fail::Bad);
                }
                hit(clause::D2D_PATH);
                self.g[g] = Some(Geom::Path { wind, figs });
                out.push("path 00000000 00000000".into());
            }
            "d2sink" => self.sink(op, out)?,
            "d2simp" | "d2outline" | "d2widen" | "d2combine" | "d2tess" => {
                let g = self.geom(op, 1, out)?;
                let mut at = 2;
                let mut simp = None;
                match t[0].as_str() {
                    "d2simp" => {
                        simp = Some(Simplification::parse(t.get(2).map(String::as_str))?);
                        at = 3;
                    }
                    "d2widen" => {
                        f(op, 2)?;
                        at = 3;
                    }
                    "d2combine" => {
                        self.geom(op, 2, out)?.ok_or(Fail::Unmodeled(open::D2D_SIMP))?;
                        if !matches!(t.get(3).map(String::as_str), Some("union" | "intersect" | "xor" | "exclude")) {
                            return Err(Fail::Bad);
                        }
                        at = 4;
                    }
                    _ => {}
                }
                if len > at {
                    f(op, at)?;
                }
                if len > at + 1 {
                    return Err(Fail::Bad);
                }
                let Some(g) = g else { return Ok(true) };
                if t[0] == "d2tess" {
                    let tol = if len > at { f(op, at)? } else { 0.25 };
                    let ts = tessellate(&g, tol).ok_or(Fail::Unmodeled(open::D2D_SIMP))?;
                    let body: String = ts.iter().map(|t| format!(" T {} {} {} {} {} {}", hex(t[0].0), hex(t[0].1), hex(t[1].0), hex(t[1].1), hex(t[2].0), hex(t[2].1))).collect();
                    out.push(format!("tess{body} hr 00000000"));
                    return Ok(true);
                }
                let Some(simp) = simp else { return Err(Fail::Unmodeled(open::D2D_SIMP)) };
                hit(clause::D2D_SIMPLIFY);
                let tol = if len > at { f(op, at)? } else { 0.25 };
                let (wind, round, figs) = shape(&g);
                let figs = match simp {
                    Simplification::CubicsAndLines => figs,
                    Simplification::Lines => flat_figs(&figs, tol),
                };
                out.push(format!("simp{} hr 00000000", sink_text(wind, round, &figs)));
            }
            "d2bounds" => {
                let g = self.geom(op, 1, out)?;
                let m = if len > 2 { Some(mat(op, 2)?) } else { None };
                if len != 2 && len != 8 {
                    return Err(Fail::Bad);
                }
                let Some(g) = g else { return Ok(true) };
                let b = bounds(&g, m.as_ref())?;
                out.push(format!("bounds 00000000 {} {} {} {}", hex(b[0]), hex(b[1]), hex(b[2]), hex(b[3])));
            }
            "d2in" => {
                let g = self.geom(op, 1, out)?;
                let (x, y) = (f(op, 2)?, f(op, 3)?);
                let tol = if len > 4 { f(op, 4)? } else { 0.25 };
                if len != 4 && len != 5 {
                    return Err(Fail::Bad);
                }
                let Some(g) = g else { return Ok(true) };
                out.push(format!("in 00000000 {}", contains(&g, x, y, tol)? as u32));
            }
            "d2area" | "d2len" => {
                let g = self.geom(op, 1, out)?;
                let tol = if len > 2 { f(op, 2)? } else { 0.25 };
                if len > 3 {
                    return Err(Fail::Bad);
                }
                let Some(g) = g else { return Ok(true) };
                let v = if t[0] == "d2area" { area(&g, tol)? } else { Measure::Value(length(&g, tol)?) };
                match v {
                    Measure::Value(v) => out.push(format!("{} 00000000 {}", &t[0][2..], hex(v))),
                    Measure::BadNumber => out.push(format!("{} 88990011 {}", &t[0][2..], hex(-1.0))),
                }
            }
            "d2cmp" => {
                let a = self.geom(op, 1, out)?;
                let b = self.geom(op, 2, out)?;
                want(3)?;
                let (Some(a), Some(b)) = (a, b) else { return Ok(true) };
                out.push(format!("cmp 00000000 {}", relation(&a, &b)?));
            }
            "d2count" => {
                let g = self.geom(op, 1, out)?;
                want(2)?;
                let Some(g) = g else { return Ok(true) };
                let Geom::Path { figs, .. } = g else {
                    hit(clause::D2D_COUNT_NOPATH);
                    out.push("count nopath".into());
                    return Ok(true);
                };
                hit(clause::D2D_COUNT);
                let segs: usize = figs.iter().map(|f| f.pts.len() - 1 + f.closed as usize).sum();
                out.push(format!("count 00000000 {} 00000000 {segs}", figs.len()));
            }
            "d2target" | "d2aa" | "d2xform" | "d2clear" | "d2fill" | "d2fillg" | "d2fillo" | "d2brush" | "d2read" | "d2bmp" | "d2drawbmp" | "d2bbrush" | "d2line" | "d2drawrect" => self.draw(op, out)?,
            "d2info" | "d2dpi" | "d2tags" | "d2flush" | "d2state" | "d2clip" | "d2unclip" | "d2layer" | "d2poplayer" => self.state(op, out)?,
            // (gradients, geometry strokes: asked, not modelled yet)
            "d2drawg" => self.draw(op, out)?,
            "d2grad" => {
                return Err(Fail::Unmodeled(open::D2D_RT));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// the target's state ops: d2info, d2dpi, d2tags, d2flush, d2state, d2clip, d2unclip
    fn state(&mut self, op: &Op, out: &mut Vec<String>) -> Res<()> {
        let t = &op.tok;
        let len = t.len();
        let want = |n: usize| if len == n { Ok(()) } else { Err(Fail::Bad) };
        match t[0].as_str() {
            "d2info" => {
                want(1)?;
                let Some(tg) = &self.t else { out.push("nort".into()); return Ok(()) };
                // (MEASURED, D2D.RT.INFO: GetSize is the pixel size times 96 / dpi in f32,
                // GetMaximumBitmapSize 8388608, the format B8G8R8A8 premultiplied)
                hit(clause::D2D_RT_INFO);
                let (w, h) = (tg.w as f32 * 96.0 / tg.dpi.0, tg.h as f32 * 96.0 / tg.dpi.1);
                let m: Vec<String> = tg.st.m.iter().map(|&v| hex(v)).collect();
                out.push(format!("info size {} {} px {} {} dpi {} {} max 8388608 fmt 87 1 aa {} xform {} tags {} {}", hex(w), hex(h), tg.w, tg.h, hex(tg.dpi.0), hex(tg.dpi.1), tg.st.aa.code(), m.join(" "), tg.st.tags.0, tg.st.tags.1));
            }
            "d2dpi" => {
                let (x, y) = (f(op, 1)?, f(op, 2)?);
                want(3)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                hit(clause::D2D_RT_DPI);
                // (MEASURED: (0, 0) is 96 96, a pair of positives is kept, anything else leaves the
                // dpi as it was)
                if x == 0.0 && y == 0.0 {
                    tg.dpi = (96.0, 96.0);
                } else if x > 0.0 && y > 0.0 {
                    tg.dpi = (x, y);
                }
            }
            "d2tags" => {
                let (a, b) = (n(op, 1)?, n(op, 2)?);
                want(3)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                hit(clause::D2D_RT_TAGS);
                tg.st.tags = (a.cast_unsigned(), b.cast_unsigned());
            }
            "d2flush" => {
                want(1)?;
                if self.t.is_none() {
                    out.push("nort".into());
                    return Ok(());
                }
                hit(clause::D2D_RT_FLUSH);
                out.push("flush 00000000 0 0".into());
            }
            "d2state" => {
                let save = match t.get(1).map(String::as_str) {
                    Some("save") => true,
                    Some("restore") => false,
                    _ => return Err(Fail::Bad),
                };
                want(2)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                hit(clause::D2D_RT_STATE);
                let block = tg.block.get_or_insert(None);
                if save {
                    *block = Some(tg.st);
                } else {
                    tg.st = block.unwrap_or(FRESH);
                }
            }
            "d2clip" => {
                let r = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                let aa = t.get(5).and_then(|t| Antialias::parse(t)).ok_or(Fail::Bad)?;
                want(6)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                // (MEASURED, D2D.CLIP.XF: through the transform the clip is the bounds of the
                // rectangle's corners)
                let m = tg.st.m;
                if m != IDENTITY {
                    hit(clause::D2D_CLIP_XF);
                }
                hit(match aa {
                    Antialias::Aliased => clause::D2D_CLIP_ALIASED,
                    Antialias::PerPrimitive => clause::D2D_CLIP_BLEND,
                });
                if tg.fault.is_some() {
                    return Ok(());
                }
                let edge = if m[1] == 0.0 && m[2] == 0.0 { ClipEdge::Samples } else { ClipEdge::Area };
                // (FITTED, OPEN, D2D.RT: an antialiased clip through a transform that turns the
                // axes, clipxf-*: all but four corner pixels; two antialiased clips nested,
                // clipnest-1: all but one pixel)
                if matches!(aa, Antialias::PerPrimitive) && (matches!(edge, ClipEdge::Area) || tg.stack.iter().any(|p| matches!(p, Pushed::Clip(Clip { aa: Antialias::PerPrimitive, .. })))) {
                    super::approx(open::D2D_RT)?;
                }
                tg.stack.push(Pushed::Clip(Clip { r: corner_bounds(&m, r), aa, edge }));
                tg.depth += 1;
            }
            "d2layer" => {
                let r = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                let o = f(op, 5)?;
                let mask = if len == 7 { Some(Self::slot(op, 6)?) } else { None };
                if len != 6 && len != 7 {
                    return Err(Fail::Bad);
                }
                let mask = match mask {
                    Some(i) => match &self.g[i] {
                        Some(g) => Some(g.clone()),
                        None => return Err(Fail::Bad),
                    },
                    None => None,
                };
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                if tg.fault.is_some() {
                    return Ok(());
                }
                // (MEASURED, D2D.LAYER: the layer's content is drawn on a transparent surface;
                // at the pop each pixel inside the content bounds is scaled by the opacity byte
                // floor(o 255 + 0.5), then by the mask's coverage byte (D2D.FILL.AACOVER), and
                // put over what was under it)
                hit(clause::D2D_LAYER);
                // (MEASURED, D2D.LAYER.BOUNDS, layerx-*: without a mask the content bounds are
                // rasterized antialiased like a fill; a mask made of lines is first cut to the
                // bounds as geometry, in f64, and the piece is rasterized, so the cut adds
                // vertices that snap like any other)
                let inside = match mask {
                    None => samples16(tg, &fixed_polys(tg, &Geom::Rect(r), Frame::Antialiased)?.1, false),
                    Some(g) => {
                        hit(clause::D2D_LAYER_MASK);
                        let Some((wind, figs, m0)) = g.lines() else {
                            return Err(Fail::Unmodeled(open::D2D_RT));
                        };
                        let m = tg.st.m;
                        let quad = [apply(&m, r[0], r[1]), apply(&m, r[2], r[1]), apply(&m, r[2], r[3]), apply(&m, r[0], r[3])].map(|(x, y)| (f64::from(x), f64::from(y)));
                        let cut: Vec<Vec<(i64, i64)>> = polys(&figs, &mul(&m0, &m)).iter().map(|p| clip_convex(p, &quad)).filter(|p| p.len() >= 3).map(|p| p.into_iter().map(|(x, y)| (Frame::Antialiased.snap(x), Frame::Antialiased.snap(y))).collect()).collect();
                        if cut.iter().flatten().any(|p| !(p.0.abs() <= 16 * 8388608 && p.1.abs() <= 16 * 8388608)) {
                            return Err(Fail::Unmodeled(open::D2D_RT));
                        }
                        samples16(tg, &cut, wind)
                    }
                };
                let cover = inside.into_iter().map(|s| Samples::Mask.byte(s.count_ones())).collect();
                // (MEASURED, D2D.LAYER.INUSE: the one ID2D1Layer pushed again while pushed is
                // D2DERR_LAYER_ALREADY_IN_USE)
                if tg.stack.iter().any(|p| matches!(p, Pushed::Layer(_))) {
                    hit(clause::D2D_LAYER_INUSE);
                    tg.fail(D2DERR_LAYER_ALREADY_IN_USE, Drawn::Dropped);
                    return Ok(());
                }
                let opacity = (o * 255.0).round_ties_even().clamp(0.0, 255.0) as u32;
                let below = std::mem::replace(&mut tg.px, vec![[0; 4]; tg.w * tg.h]);
                tg.stack.push(Pushed::Layer(Layer { below, opacity, cover }));
                tg.depth += 1;
            }
            "d2poplayer" => {
                want(1)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                if tg.fault.is_some() {
                    return Ok(());
                }
                // (MEASURED, D2D.RT.FAULT: PopLayer on a clip is the error state with
                // D2DERR_POP_CALL_DID_NOT_MATCH_PUSH, on nothing with D2DERR_PUSH_POP_UNBALANCED)
                let l = match tg.stack.last() {
                    Some(Pushed::Layer(l)) => l.clone(),
                    Some(Pushed::Clip(_)) => {
                        tg.fail(D2DERR_POP_CALL_DID_NOT_MATCH_PUSH, Drawn::Kept);
                        return Ok(());
                    }
                    None => {
                        tg.fail(D2DERR_PUSH_POP_UNBALANCED, Drawn::Kept);
                        return Ok(());
                    }
                };
                tg.stack.pop();
                tg.depth -= 1;
                tg.px = l.composite(&tg.px, tg.w, tg.h);
                tg.keep_clipped(&l.below);
            }
            _ => {
                want(1)?;
                let Some(tg) = &mut self.t else { out.push("nort".into()); return Ok(()) };
                // (MEASURED, D2D.RT.FAULT, D2D.CLIP.UNBALANCED: PopAxisAlignedClip on a layer is
                // the error state; on nothing it only counts against EndDraw)
                if tg.fault.is_some() {
                    return Ok(());
                }
                match tg.stack.last() {
                    Some(Pushed::Clip(_)) => {
                        tg.stack.pop();
                    }
                    Some(Pushed::Layer(_)) => {
                        tg.fail(D2DERR_POP_CALL_DID_NOT_MATCH_PUSH, Drawn::Kept);
                        return Ok(());
                    }
                    None => {
                        hit(clause::D2D_CLIP_UNBALANCED);
                        tg.fail(D2DERR_PUSH_POP_UNBALANCED, Drawn::Kept);
                        return Ok(());
                    }
                }
                tg.depth -= 1;
            }
        }
        Ok(())
    }

    fn stroke(&mut self, op: &Op, out: &mut Vec<String>) -> Res<()> {
        let p: Vec<i64> = [1, 2, 3, 4, 6].iter().map(|&i| n(op, i)).collect::<Res<_>>()?;
        let (miter, offset) = (f(op, 5)?, f(op, 7)?);
        let dashes: Vec<f32> = (8..op.tok.len()).map(|i| f(op, i)).collect::<Res<_>>()?;
        if dashes.len() > 32 {
            return Err(Fail::Bad);
        }
        let (caps, join, style) = (&p[0..3], p[3], p[4]);
        hit(clause::D2D_STROKE_ENUM);
        let ok = caps.iter().all(|c| (0..=3).contains(c)) && (0..=3).contains(&join) && (0..=5).contains(&style);
        if !ok {
            out.push(format!("stroke hr {E_INVALIDARG:08x}"));
            return Ok(());
        }
        let custom = style == 5;
        if custom != !dashes.is_empty() {
            hit(clause::D2D_STROKE_DASHES);
            out.push(format!("stroke hr {E_INVALIDARG:08x}"));
            return Ok(());
        }
        // (MEASURED: any miter limit, offset and dash value is kept as given, NaN, infinite and
        // negative ones included)
        hit(clause::D2D_STROKE_VALUES);
        // (MEASURED: the predefined styles' patterns, in stroke widths, whatever the caps)
        let pattern: &[f32] = match style {
            1 => &[2.0, 2.0],
            2 => &[0.0, 2.0],
            3 => &[2.0, 2.0, 0.0, 2.0],
            4 => &[2.0, 2.0, 0.0, 2.0, 0.0, 2.0],
            _ => &dashes,
        };
        if (1..=4).contains(&style) {
            hit(clause::D2D_STROKE_PATTERN);
        }
        hit(clause::D2D_STROKE_GET);
        let mut s = format!("stroke = {} {} {} {join} {} {style} {} {}", caps[0], caps[1], caps[2], hex(miter), hex(offset), pattern.len());
        for d in pattern {
            s += " ";
            s += &hex(*d);
        }
        out.push(s);
        Ok(())
    }

    fn sink(&mut self, op: &Op, out: &mut Vec<String>) -> Res<()> {
        let k = &mut self.sink;
        k.made = true;
        let mut s = String::from("sink");
        let t = &op.tok;
        let mut i = 1;
        while i < t.len() {
            match t[i].as_str() {
                "open" => {
                    hit(clause::D2D_SINK_OPEN);
                    if k.state == SinkState::New {
                        k.state = SinkState::Open;
                        k.sink = true;
                        s += " open 00000000";
                    } else {
                        s += &format!(" open {D2DERR_WRONG_STATE:08x}");
                    }
                }
                "close" => {
                    if !k.sink {
                        s += " close -";
                    } else if k.state == SinkState::Open && !k.error && !k.infig {
                        hit(clause::D2D_SINK_CLOSE);
                        k.state = SinkState::Closed;
                        s += " close 00000000";
                    } else {
                        hit(clause::D2D_SINK_ERROR);
                        // (MEASURED: a second Close leaves the closed geometry as it was)
                        if k.state != SinkState::Closed {
                            k.state = SinkState::Failed;
                        }
                        s += &format!(" close {D2DERR_WRONG_STATE:08x}");
                    }
                }
                "count" => {
                    if k.state == SinkState::Closed {
                        hit(clause::D2D_COUNT);
                        s += &format!(" count 00000000 {} 00000000 {}", k.figs, k.segs);
                    } else {
                        // (MEASURED: D2DERR_WRONG_STATE, the counts not written)
                        hit(clause::D2D_SINK_COUNT);
                        s += &format!(" count {D2DERR_WRONG_STATE:08x} 57005 {D2DERR_WRONG_STATE:08x} 57005");
                    }
                }
                "begin" | "line" => {
                    f(op, i + 1)?;
                    f(op, i + 2)?;
                    if k.sink {
                        if k.state != SinkState::Open {
                            // (MEASURED: ignored once closed)
                            hit(clause::D2D_SINK_AFTER);
                        } else if t[i] == "begin" {
                            if k.infig {
                                k.error = true;
                            }
                            k.infig = true;
                            k.open_segs = 0;
                        } else if !k.infig {
                            k.error = true;
                        } else {
                            k.open_segs += 1;
                        }
                    }
                    i += 2;
                }
                "end" => {
                    if k.sink {
                        if k.state != SinkState::Open {
                            hit(clause::D2D_SINK_AFTER);
                        } else if !k.infig {
                            k.error = true;
                        } else {
                            k.figs += 1;
                            k.segs += k.open_segs + 1;
                        }
                        k.infig = false;
                    }
                }
                "fill" => {
                    if !matches!(t.get(i + 1).map(String::as_str), Some("alt" | "wind")) {
                        return Err(Fail::Bad);
                    }
                    if k.infig || k.figs > 0 {
                        // (MEASURED: accepted after a figure; the counts are not changed)
                        hit(clause::D2D_SINK_FILL);
                    }
                    i += 1;
                }
                _ => return Err(Fail::Bad),
            }
            i += 1;
        }
        out.push(s);
        Ok(())
    }

    fn draw(&mut self, op: &Op, out: &mut Vec<String>) -> Res<()> {
        let t = &op.tok;
        let len = t.len();
        let color = |at: usize| -> Res<[i64; 4]> { Ok([n(op, at)?, n(op, at + 1)?, n(op, at + 2)?, n(op, at + 3)?]) };
        match t[0].as_str() {
            "d2target" => {
                let (w, h) = (n(op, 1)?, n(op, 2)?);
                if len != 3 || !(1..=64).contains(&w) || !(1..=64).contains(&h) {
                    return Err(Fail::Bad);
                }
                hit(clause::D2D_TARGET);
                self.t = Some(Target::new(w as usize, h as usize));
                out.push("target 00000000".into());
                return Ok(());
            }
            "d2aa" => {
                let al = t.get(1).and_then(|t| Antialias::parse(t)).ok_or(Fail::Bad)?;
                if len != 2 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    tg.st.aa = al;
                    return Ok(());
                }
            }
            "d2xform" => {
                let m = mat(op, 1)?;
                if len != 7 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    tg.st.m = m;
                    return Ok(());
                }
            }
            "d2clear" => {
                let c = color(1)?;
                if len != 5 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    hit(clause::D2D_CLEAR);
                    if tg.fault.is_some() {
                        return Ok(());
                    }
                    let before = tg.px.clone();
                    let p = premultiply(c);
                    tg.px.iter_mut().for_each(|q| *q = p);
                    tg.keep_clipped(&before);
                    return Ok(());
                }
            }
            "d2fill" | "d2fillg" | "d2fillo" => {
                let (g, c, o) = if t[0] == "d2fillg" {
                    let g = Self::slot(op, 1)?;
                    let c = color(2)?;
                    if len != 6 {
                        return Err(Fail::Bad);
                    }
                    (self.g[g].clone(), c, 1.0)
                } else {
                    let r = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                    let c = color(5)?;
                    let o = if t[0] == "d2fillo" { f(op, 9)? } else { 1.0 };
                    if len != if t[0] == "d2fillo" { 10 } else { 9 } {
                        return Err(Fail::Bad);
                    }
                    (Some(Geom::Rect(r)), c, o)
                };
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                let Some(g) = g else {
                    out.push("nogeom".into());
                    return Ok(());
                };
                if o != 1.0 {
                    hit(clause::D2D_OPACITY);
                }
                if tg.fault.is_some() {
                    return Ok(());
                }
                let before = tg.px.clone();
                fill(tg, &g, &Paint::Solid(premultiply_o(c, o)))?;
                tg.keep_clipped(&before);
                return Ok(());
            }
            "d2drawg" => {
                let g = Self::slot(op, 1)?;
                let w = f(op, 2)?;
                let c = color(3)?;
                if len != 7 {
                    return Err(Fail::Bad);
                }
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                let Some(g) = self.g[g].clone() else {
                    out.push("nogeom".into());
                    return Ok(());
                };
                if tg.fault.is_some() {
                    return Ok(());
                }
                // (straight figures widen as stroke_pieces says, fitted to strokefit-* and
                // strokemore-*: all but three pixels one unit off; curves are not modelled yet)
                let Some((_, figs, m0)) = g.lines() else {
                    return Err(Fail::Unmodeled(open::D2D_RT));
                };
                if !w.is_finite() {
                    return Err(Fail::Unmodeled(open::D2D_RT));
                }
                super::approx(open::D2D_RT)?;
                let figs: Vec<Figure> = figs.iter().map(|f| Figure { pts: f.pts.iter().map(|&(x, y)| apply(&m0, x, y)).collect(), closed: f.closed }).collect();
                let pieces: Vec<Figure> = figs.iter().flat_map(|f| stroke_pieces(f, w * 0.5)).collect();
                let before = tg.px.clone();
                fill_union(tg, pieces, &Paint::Solid(premultiply(c)))?;
                tg.keep_clipped(&before);
                return Ok(());
            }
            "d2bmp" => {
                let i = n(op, 1)?;
                let (w, h) = (n(op, 2)?, n(op, 3)?);
                let hexs = t.get(4).ok_or(Fail::Bad)?;
                if len != 5 || !(0..4).contains(&i) || !(1..=16).contains(&w) || !(1..=16).contains(&h) || hexs.len() as i64 != w * h * 8 {
                    return Err(Fail::Bad);
                }
                let bytes: Vec<u8> = (0..hexs.len() / 2).map(|k| hexs.get(2 * k..2 * k + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect::<Option<_>>().ok_or(Fail::Bad)?;
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                hit(clause::D2D_BITMAP);
                let px = bytes.chunks_exact(4).map(|c| [c[0], c[1], c[2], c[3]]).collect();
                tg.bitmaps[i as usize] = Some(Bitmap { w: w as usize, h: h as usize, px });
                out.push("bmp 00000000".into());
                return Ok(());
            }
            "d2drawbmp" | "d2bbrush" => {
                let i = n(op, 1)?;
                let r = [f(op, 2)?, f(op, 3)?, f(op, 4)?, f(op, 5)?];
                let draw = t[0] == "d2drawbmp";
                let word = |k: usize| t.get(k).map(String::as_str).unwrap_or("");
                let (o, interp, ext, rest) = if draw {
                    (f(op, 6)?, Interp::parse(word(7)).ok_or(Fail::Bad)?, (Extend::Clamp, Extend::Clamp), 8)
                } else {
                    let ext = (Extend::parse(word(6)).ok_or(Fail::Bad)?, Extend::parse(word(7)).ok_or(Fail::Bad)?);
                    (1.0, Interp::parse(word(8)).ok_or(Fail::Bad)?, ext, 9)
                };
                let extra = if len == rest { None } else if len == rest + if draw { 4 } else { 6 } {
                    Some((rest..len).map(|k| f(op, k)).collect::<Res<Vec<f32>>>()?)
                } else {
                    return Err(Fail::Bad);
                };
                if !(0..4).contains(&i) {
                    return Err(Fail::Bad);
                }
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                let Some(bmp) = tg.bitmaps[i as usize].clone() else { return Ok(()) };
                if tg.fault.is_some() {
                    return Ok(());
                }
                let full = [0.0, 0.0, bmp.w as f32, bmp.h as f32];
                let (place, opacity) = if draw {
                    hit(match interp {
                        Interp::Nearest => clause::D2D_BITMAP_NEAREST,
                        Interp::Linear => clause::D2D_BITMAP_LINEAR,
                    });
                    // (MEASURED, bmpx-0, bmpsrcfit-*, bmpsrcmore-*: the source rectangle is put in order, then
                    // each side past the bitmap is moved onto it, the destination rectangle
                    // staying, so what is left stretches over all of it; a side moved past the
                    // other turns the rectangle round onto the texels beyond that edge, which the
                    // clamp repeats, either interpolation; one of no width or height draws nothing)
                    let src = extra.as_deref().map_or(full, |e| [lo(e[0], e[2]).max(0.0), lo(e[1], e[3]).max(0.0), hi(e[0], e[2]).min(full[2]), hi(e[1], e[3]).min(full[3])]);
                    hit(clause::D2D_BITMAP_SRC);
                    if src[0] == src[2] || src[1] == src[3] {
                        return Ok(());
                    }
                    if !(src.iter().all(|v| v.is_finite())) {
                        return Err(Fail::Unmodeled(open::D2D_RT));
                    }
                    // (MEASURED, D2D.BITMAP.OPACITY: the texel scaled by the opacity byte)
                    let opacity = (o * 255.0).round_ties_even().clamp(0.0, 255.0) as u32;
                    if tg.st.m == IDENTITY {
                        (Place::Rect { dst: r, src }, opacity)
                    } else {
                        let (sx, sy) = ((r[2] - r[0]) / (src[2] - src[0]), (r[3] - r[1]) / (src[3] - src[1]));
                        let bm = [sx, 0.0, 0.0, sy, r[0] - src[0] * sx, r[1] - src[1] * sy];
                        (Place::Brush { m: mul(&bm, &tg.st.m), world: tg.st.m }, opacity)
                    }
                } else {
                    hit(clause::D2D_BBRUSH);
                    let m = extra.as_deref().map_or(IDENTITY, |e| [e[0], e[1], e[2], e[3], e[4], e[5]]);
                    (Place::Brush { m: mul(&m, &tg.st.m), world: tg.st.m }, 255)
                };
                let paint = Paint::Bitmap { bmp: &bmp, place, ext, interp, opacity };
                let before = tg.px.clone();
                fill(tg, &Geom::Rect(r), &paint)?;
                tg.keep_clipped(&before);
                return Ok(());
            }
            "d2line" | "d2drawrect" => {
                let v = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?, f(op, 5)?];
                let c = color(6)?;
                if len != 10 {
                    return Err(Fail::Bad);
                }
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                if tg.fault.is_some() {
                    return Ok(());
                }
                let h = v[4] * 0.5;
                let figs = if t[0] == "d2line" {
                    // (MEASURED, D2D.STROKE.LINE: a line with flat caps is the rectangle of its
                    // length and the width about it)
                    hit(clause::D2D_STROKE_LINE);
                    let (dx, dy) = (v[2] - v[0], v[3] - v[1]);
                    let l = (dx * dx + dy * dy).sqrt();
                    if l == 0.0 {
                        vec![]
                    } else {
                        let (nx, ny) = (-dy / l * h, dx / l * h);
                        vec![Figure { pts: vec![(v[0] + nx, v[1] + ny), (v[2] + nx, v[3] + ny), (v[2] - nx, v[3] - ny), (v[0] - nx, v[1] - ny)], closed: true }]
                    }
                } else {
                    // (MEASURED, D2D.STROKE.RECT: a rectangle's stroke with miter joins is the
                    // rectangle grown by half the width less the one shrunk by it, when that is
                    // not empty)
                    hit(clause::D2D_STROKE_RECT);
                    rect_stroke([v[0].min(v[2]), v[1].min(v[3]), v[0].max(v[2]), v[1].max(v[3])], h)
                };
                let before = tg.px.clone();
                fill(tg, &Geom::Path { wind: true, figs }, &Paint::Solid(premultiply(c)))?;
                tg.keep_clipped(&before);
                return Ok(());
            }
            "d2brush" => {
                let c = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                if len != 5 {
                    return Err(Fail::Bad);
                }
                if self.t.is_some() {
                    hit(clause::D2D_BRUSH);
                    out.push(format!("brush {} {} {} {} {}", hex(c[0]), hex(c[1]), hex(c[2]), hex(c[3]), hex(1.0)));
                    return Ok(());
                }
            }
            _ => {
                if len != 1 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    hit(clause::D2D_READ);
                    // (MEASURED, D2D.RT.FAULT: after an error the frame's drawing is thrown away
                    // and EndDraw gives the error; D2D.CLIP.UNBALANCED: pushes and pops of the
                    // frame that do not balance give D2DERR_PUSH_POP_UNBALANCED, what is pushed
                    // stays pushed into the next frame, and the bitmap holds the layers put down)
                    let hr = match tg.fault.take() {
                        Some(f) => {
                            match f.drawn {
                                Drawn::Dropped => (tg.px, tg.stack) = tg.begin.clone(),
                                Drawn::Kept => {}
                            }
                            f.hr
                        }
                        None if tg.depth != 0 => {
                            hit(clause::D2D_CLIP_UNBALANCED);
                            D2DERR_PUSH_POP_UNBALANCED
                        }
                        None => 0,
                    };
                    tg.depth = 0;
                    tg.begin = (tg.px.clone(), tg.stack.clone());
                    let view = tg.view();
                    out.push(format!("end {hr:08x}"));
                    out.push(format!("px 0 {} {} 32", tg.w, tg.h));
                    for y in 0..tg.h {
                        out.push(view[y * tg.w..(y + 1) * tg.w].iter().flat_map(|p| p.iter()).map(|b| format!("{b:02x}")).collect());
                    }
                    return Ok(());
                }
            }
        }
        out.push("nort".into());
        Ok(())
    }
}

/// FillRectangle / FillGeometry with a solid color brush: the pixels whose centers the shape
/// covers (aliased, or antialiased where every edge is on the pixel grid), source over
/// a side of a stroke's center line: the normal (-dy, dx) points to the left one
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    Left,
    Right,
}

impl Side {
    fn sign(self) -> f32 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
}

/// how a figure turns at a vertex: a join is on the side it turns away from
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Turn {
    /// the outside of the corner on this side
    Corner(Side),
    /// the segments in line: no join
    Straight,
}

/// MEASURED (D2D.STROKE.RECT): a rectangle (in order) stroked with miter joins: grown by half
/// the width h, less the one shrunk by h when that is not empty
fn rect_stroke([l, t, r, b]: [f32; 4], h: f32) -> Vec<Figure> {
    let mut figs = vec![Figure { pts: vec![(l - h, t - h), (r + h, t - h), (r + h, b + h), (l - h, b + h)], closed: true }];
    if l + h < r - h && t + h < b - h {
        figs.push(Figure { pts: vec![(l + h, t + h), (l + h, b - h), (r - h, b - h), (r - h, t + h)], closed: true });
    }
    figs
}

/// FITTED (D2D.RT, strokefit-*, strokemore-*, strokewide-*, strokelobe-*: all but six pixels
/// of 168 cases, five of them one unit of the blend off): a figure of straight segments
/// stroked with flat caps and miter joins (limit 10), in f32, as pieces fill_union paints:
/// each segment the quad of its ends moved half the width h along its normal both ways; each
/// join the wedge on its outer side from the two inner offset points over the segments' outer
/// offset points and the miter (where the outer offset lines meet) between them, the miter cut
/// square at 10 h from the vertex when longer; an open figure no join at its ends. (WPF's
/// rails, one outline, a union of the pieces as one nonzero fill and a wedge split at the
/// vertex all matched worse)
fn stroke_pieces(fg: &Figure, h: f32) -> Vec<Figure> {
    let mut p = fg.pts.clone();
    p.dedup();
    if fg.closed && p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    let n = p.len();
    if n < 2 {
        return Vec::new();
    }
    let nrm = |u: (f32, f32), v: (f32, f32)| {
        let (dx, dy) = (v.0 - u.0, v.1 - u.1);
        let l = (dx * dx + dy * dy).sqrt();
        (-dy / l * h, dx / l * h)
    };
    let at = |q: (f32, f32), d: (f32, f32), s: f32| (q.0 + s * d.0, q.1 + s * d.1);
    let piece = |pts: Vec<(f32, f32)>| Figure { pts, closed: true };
    let segs = if fg.closed { n } else { n - 1 };
    let mut out = Vec::new();
    for k in 0..segs {
        let (a, b) = (p[k], p[(k + 1) % n]);
        let n1 = nrm(a, b);
        out.push(piece(vec![at(a, n1, 1.0), at(b, n1, 1.0), at(b, n1, -1.0), at(a, n1, -1.0)]));
        if !fg.closed && k + 2 >= n {
            continue;
        }
        let c = p[(k + 2) % n];
        let n2 = nrm(b, c);
        let cr = (b.0 - a.0) * (c.1 - b.1) - (b.1 - a.1) * (c.0 - b.0);
        let turn = if cr > 0.0 {
            Turn::Corner(Side::Right)
        } else if cr < 0.0 {
            Turn::Corner(Side::Left)
        } else {
            Turn::Straight
        };
        let side = match turn {
            Turn::Corner(side) => side,
            Turn::Straight => continue,
        };
        let s = side.sign();
        let (p1, p2) = (at(b, n1, s), at(b, n2, s));
        let (d1, d2) = ((b.0 - a.0, b.1 - a.1), (c.0 - b.0, c.1 - b.1));
        let crs = |u: (f32, f32), v: (f32, f32)| u.0 * v.1 - u.1 * v.0;
        // the miter: where the two outer offset lines meet, from the first one's point
        let t = crs((p2.0 - p1.0, p2.1 - p1.1), d2) / crs(d1, d2);
        let m = (p1.0 + d1.0 * t, p1.1 + d1.1 * t);
        // (the wedge crosses itself at the vertex: its inner lobe winds -1, which covers as
        // the pieces are filled each by itself, fill_union)
        let mut wedge = vec![at(b, n2, -s), at(b, n1, -s), p1];
        let (vx, vy) = (m.0 - b.0, m.1 - b.1);
        let ml = (vx * vx + vy * vy).sqrt();
        let lim = 10.0 * h;
        if ml > lim {
            let (dx, dy) = (vx / ml, vy / ml);
            let cut = |q: (f32, f32)| {
                let pq = (q.0 - b.0) * dx + (q.1 - b.1) * dy;
                let t = (lim - pq) / (ml - pq);
                (q.0 + t * (m.0 - q.0), q.1 + t * (m.1 - q.1))
            };
            wedge.extend([cut(p1), cut(p2)]);
        } else {
            wedge.push(m);
        }
        wedge.push(p2);
        out.push(piece(wedge));
    }
    out
}

/// pieces filled each by itself with nonzero winding, the samples any of them covers painted
/// once (a stroke's quads and wedges: a wedge's inner lobe winds -1 and still covers)
fn fill_union(tg: &mut Target, pieces: Vec<Figure>, paint: &Paint) -> Res<()> {
    let frame = match tg.st.aa {
        Antialias::Aliased => Frame::Aliased,
        Antialias::PerPrimitive => Frame::Antialiased,
    };
    let mut polys = Vec::new();
    for f in pieces {
        let (_, ps) = fixed_polys(tg, &Geom::Path { wind: true, figs: vec![f] }, frame)?;
        polys.push(ps);
    }
    match frame {
        Frame::Antialiased => {
            let mut m = vec![0u64; tg.w * tg.h];
            for ps in &polys {
                for (o, v) in m.iter_mut().zip(samples16(tg, ps, true)) {
                    *o |= v;
                }
            }
            put_coverage(tg, m.into_iter().map(u64::count_ones).collect(), paint);
        }
        Frame::Aliased => {
            let mut m = vec![false; tg.w * tg.h];
            for ps in &polys {
                for (o, v) in m.iter_mut().zip(cover_aliased(tg, ps, true)) {
                    *o |= v;
                }
            }
            for (i, inside) in m.into_iter().enumerate() {
                if inside {
                    hit(clause::D2D_FILL_POLY);
                    tg.px[i] = over(paint.at(i % tg.w, i / tg.w), tg.px[i]);
                }
            }
        }
    }
    Ok(())
}

/// FITTED (D2D.SIMP, tess-0, tess-4, tess-5, carea-*, parea-*: 39 cases bit for bit):
/// Tessellate of one convex figure: its points and control points rounded to 1/64 pixel, then
/// flattened as Simplify's lines (rounded again), repeats dropped; then
/// zipped from the bottom (largest y) up between its left and right chains: a step up the
/// right chain gives (new right, old right, left), one up the left chain (old left, new left,
/// right), the chain whose next point is lower going first (the right one on a tie, the left
/// one into a shared top point), empty triangles left out. None for what is not one convex
/// figure (not modelled yet)
pub fn tessellate(g: &Geom, tol: f32) -> Option<Vec<[P; 3]>> {
    let (_, _, figs) = shape(g);
    let q = |v: f32| (v * 64.0).round() / 64.0;
    let qq = |v: P| (q(v.0), q(v.1));
    let snapped: Vec<Fig> = figs.iter().map(|f| Fig {
        start: qq(f.start),
        segs: f.segs.iter().map(|s| match *s {
            Seg::L(v) => Seg::L(qq(v)),
            Seg::C(a, b, c) => Seg::C(qq(a), qq(b), qq(c)),
        }).collect(),
        closed: f.closed,
    }).collect();
    let figs = flat_figs(&snapped, tol);
    let [fg] = figs.as_slice() else { return None };
    let mut p: Vec<P> = Vec::new();
    for v in std::iter::once(fg.start).chain(fg.segs.iter().map(|s| match *s {
        Seg::L(v) => v,
        Seg::C(_, _, v) => v,
    })) {
        let v = (q(v.0), q(v.1));
        if p.last() != Some(&v) {
            p.push(v);
        }
    }
    while p.len() > 1 && p.first() == p.last() {
        p.pop();
    }
    let n = p.len();
    if n < 3 || !p.iter().all(|v| v.0.is_finite() && v.1.is_finite()) {
        return None;
    }
    let cr = |o: P, a: P, b: P| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let turns: Vec<f32> = (0..n).map(|i| cr(p[i], p[(i + 1) % n], p[(i + 2) % n])).collect();
    if !(turns.iter().all(|&c| c >= 0.0) || turns.iter().all(|&c| c <= 0.0)) || turns.iter().any(|&c| c == 0.0) {
        return None;
    }
    // (convex and simple: y turns round twice going round, a star's more)
    let dys: Vec<f32> = (0..n).map(|i| p[(i + 1) % n].1 - p[i].1).filter(|&d| d != 0.0).collect();
    if (0..dys.len()).filter(|&i| (dys[i] > 0.0) != (dys[(i + 1) % dys.len()] > 0.0)).count() != 2 {
        return None;
    }
    // the bottom and top points, leftmost and rightmost among equals
    let ymax = p.iter().map(|v| v.1).fold(f32::NEG_INFINITY, f32::max);
    let ymin = p.iter().map(|v| v.1).fold(f32::INFINITY, f32::min);
    let pick = |y: f32, left: bool| (0..n).filter(|&i| p[i].1 == y).min_by(|&a, &b| if left { p[a].0.total_cmp(&p[b].0) } else { p[b].0.total_cmp(&p[a].0) }).unwrap_or(0);
    let (bl, br, tl, tr) = (pick(ymax, true), pick(ymax, false), pick(ymin, true), pick(ymin, false));
    // the left chain leaves the bottom left point toward the neighbor left of the other one
    let (prev, next) = ((bl + n - 1) % n, (bl + 1) % n);
    let step: usize = if bl != br {
        if next == br { n - 1 } else { 1 }
    } else if cr(p[bl], p[prev], p[next]) > 0.0 {
        n - 1
    } else {
        1
    };
    let walk = |from: usize, to: usize, by: usize| {
        let mut c = vec![p[from]];
        let mut i = from;
        while i != to {
            i = (i + by) % n;
            c.push(p[i]);
        }
        c
    };
    let lc = walk(bl, tl, step);
    let rc = walk(br, tr, n - step);
    let (mut li, mut ri) = (0, 0);
    let mut out = Vec::new();
    let mut emit = |t: [P; 3]| {
        if t[0] != t[1] && t[1] != t[2] && t[0] != t[2] {
            out.push(t);
        }
    };
    loop {
        let (ln, rn) = (lc.get(li + 1), rc.get(ri + 1));
        let right = match (ln, rn) {
            (None, None) => break,
            (Some(_), None) => false,
            (None, Some(_)) => true,
            (Some(a), Some(b)) => a != b && b.1 >= a.1,
        };
        if right {
            emit([rc[ri + 1], rc[ri], lc[li]]);
            ri += 1;
        } else {
            emit([lc[li], lc[li + 1], rc[ri]]);
            li += 1;
        }
    }
    Some(out)
}

fn fill(tg: &mut Target, g: &Geom, paint: &Paint) -> Res<()> {
    let frame = match tg.st.aa {
        Antialias::Aliased => Frame::Aliased,
        Antialias::PerPrimitive => Frame::Antialiased,
    };
    let (wind, ps) = fixed_polys(tg, g, frame)?;
    match frame {
        Frame::Antialiased => {
            put_coverage(tg, coverage16(tg, &ps, wind), paint);
        }
        Frame::Aliased => {
            if matches!(paint, Paint::Solid(src) if src[3] != 255) {
                hit(clause::D2D_FILL_OVER);
            }
            let axis = matches!(g, Geom::Rect(_)) && tg.st.m[1] == 0.0 && tg.st.m[2] == 0.0;
            for (i, inside) in cover_aliased(tg, &ps, wind).into_iter().enumerate() {
                if inside {
                    hit(if axis { clause::D2D_FILL_RECT } else { clause::D2D_FILL_POLY });
                    tg.px[i] = over(paint.at(i % tg.w, i / tg.w), tg.px[i]);
                }
            }
        }
    }
    Ok(())
}

/// the two rasterizers' fixed point frames
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Frame {
    /// pixel centers at the 28.4 grid's integers: round-half-even(16 v - 7.5)
    Aliased,
    /// eighths of a pixel: ceil(16 v) - 1 (then times 8, D2D.FILL.AA28)
    Antialiased,
}

impl Frame {
    fn snap(self, v: f64) -> i64 {
        match self {
            Frame::Aliased => (16.0 * v - 7.5).round_ties_even() as i64,
            Frame::Antialiased => snap16(v),
        }
    }
}

/// MEASURED (D2D.FILL.FIXED, D2D.FILL.AACURVE): a geometry through the transform as closed
/// polygons in 1/16 pixel of the frame: straight figures' points snapped, a curve's four points
/// snapped and flattened in that fixed point by WPF's Bezier32
fn fixed_polys(tg: &Target, g: &Geom, frame: Frame) -> Res<(bool, Vec<Vec<(i64, i64)>>)> {
    hit(clause::D2D_FILL_FIXED);
    let sn = |v: f64| frame.snap(v);
    if let Some((wind, figs, m0)) = g.lines() {
        let mut ps = polys(&figs, &mul(&m0, &tg.st.m));
        if ps.iter().flatten().any(|p| !(p.0.is_finite() && p.1.is_finite())) {
            return Err(Fail::Unmodeled(open::D2D_RT));
        }
        if ps.iter().flatten().any(|p| !(p.0.abs() <= 8388608.0 && p.1.abs() <= 8388608.0)) {
            // (MEASURED, D2D.FILL.HUGE, fillhuge-*: a vertex past 2^23 pixels still fills,
            // the figure cut far outside the target first; where it is cut does not reach a
            // pixel)
            hit(clause::D2D_FILL_HUGE);
            let g = 65536.0;
            let quad = [(-g, -g), (g, -g), (g, g), (-g, g)];
            ps = ps.iter().map(|p| clip_convex(p, &quad)).filter(|p| p.len() >= 3).collect();
        }
        return Ok((wind, ps.iter().map(|p| p.iter().map(|&(x, y)| (sn(x), sn(y))).collect()).collect()));
    }
    hit(clause::D2D_FILL_CURVE);
    hit(clause::D2D_FILL_AACURVE);
    let (wind, _, figs) = shape(&Geom::Xf(Box::new(g.clone()), tg.st.m));
    let s = |p: P| (sn(f64::from(p.0)), sn(f64::from(p.1)));
    if figs.iter().any(|f| std::iter::once(f.start).chain(f.segs.iter().flat_map(|s| match *s {
        Seg::L(p) => vec![p],
        Seg::C(a, b, c) => vec![a, b, c],
    })).any(|p| !(p.0.abs() <= 8388608.0 && p.1.abs() <= 8388608.0))) {
        return Err(Fail::Unmodeled(open::D2D_RT));
    }
    // (the target in 28.4: in the antialiased frame WPF's 28.4 point plus 8)
    let clip = match frame {
        Frame::Aliased => [0, 0, 16 * tg.w as i64, 16 * tg.h as i64],
        Frame::Antialiased => [8, 8, 16 * tg.w as i64 + 8, 16 * tg.h as i64 + 8],
    };
    let mut ps = Vec::new();
    for f in &figs {
        let mut poly = vec![s(f.start)];
        for seg in &f.segs {
            match *seg {
                Seg::L(p) => poly.push(s(p)),
                Seg::C(a, b, c) => {
                    let p0 = *poly.last().ok_or(Fail::Bad)?;
                    let q = [p0, s(a), s(b), s(c)];
                    poly.extend(hfd32(q, clip).unwrap_or_else(|| hfd64(q, clip)));
                }
            }
        }
        ps.push(poly);
    }
    Ok((wind, ps))
}

/// the edges of closed polygons in fixed point: (top point, bottom point, first row, row past
/// the last, winding direction); a row r is at r 16 (rows of 1/16 units: rows where y0 <= 16 r
/// < y1, ceiled)
fn fixed_edges(ps: &[Vec<(i64, i64)>], scale: i64) -> Vec<((i64, i64), (i64, i64), i64, i64, i32)> {
    let mut edges = Vec::new();
    for poly in ps {
        for k in 0..poly.len() {
            let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
            let (mut p0, mut p1) = ((a.0 * scale, a.1 * scale), (b.0 * scale, b.1 * scale));
            let mut d = 1;
            if p1.1 < p0.1 {
                std::mem::swap(&mut p0, &mut p1);
                d = -1;
            }
            let (ys, ye) = (ceil_div(p0.1, 16), ceil_div(p1.1, 16));
            if ye > ys {
                edges.push((p0, p1, ys, ye, d));
            }
        }
    }
    edges
}

/// on row j, the spans [X_left, X_right) inside by the fill mode, x ceiled on the row
fn spans(edges: &[((i64, i64), (i64, i64), i64, i64, i32)], j: i64, wind: bool, act: &mut Vec<(i64, i32)>) -> Vec<(i64, i64)> {
    act.clear();
    for &(p0, p1, ys, ye, d) in edges {
        if ys <= j && j < ye {
            let dy = p1.1 - p0.1;
            let num = p0.0 * dy + (16 * j - p0.1) * (p1.0 - p0.0);
            if num.rem_euclid(16 * dy) == 0 || (16 * j - p0.1) == 0 {
                // (a sample exactly on an edge, or an edge starting on the row: the ceiling
                // and [ceil(y0), ceil(y1)) put it on the left and top side)
                hit(clause::D2D_FILL_EDGE);
            }
            act.push((ceil_div(num, 16 * dy), d));
        }
    }
    act.sort();
    let mut out = Vec::new();
    let mut w = 0;
    for k in 0..act.len().saturating_sub(1) {
        w += act[k].1;
        if if wind { w != 0 } else { w % 2 != 0 } {
            out.push((act[k].0, act[k + 1].0));
        }
    }
    out
}

/// MEASURED (D2D.FILL.FIXED): aliased, the pixels of the spans on each pixel row
fn cover_aliased(tg: &Target, ps: &[Vec<(i64, i64)>], wind: bool) -> Vec<bool> {
    let edges = fixed_edges(ps, 1);
    let mut out = vec![false; tg.w * tg.h];
    let mut act = Vec::new();
    for j in 0..tg.h as i64 {
        for (a, b) in spans(&edges, j, wind, &mut act) {
            for c in a.max(0)..b.min(tg.w as i64) {
                out[j as usize * tg.w + c as usize] = true;
            }
        }
    }
    out
}

fn put_coverage(tg: &mut Target, cov: Vec<u32>, paint: &Paint) {
    hit(clause::D2D_FILL_AACOVER);
    for (i, n) in cov.into_iter().enumerate() {
        if n == 0 {
            continue;
        }
        let src = paint.at(i % tg.w, i / tg.w);
        let p = &mut tg.px[i];
        // (MEASURED, aac-*, aa-c*: n of 64 samples is the coverage byte (n 4 255 + 0x80) >> 8,
        // each byte of the premultiplied color times it / 255 rounded, then over the target as
        // an aliased fill)
        hit(clause::D2D_FILL_AABLEND);
        let k = Samples::Fill.byte(n);
        *p = over(scale_by(src, k), *p);
    }
}

/// Sutherland-Hodgman: a polygon cut to a convex quadrilateral (corners in either order)
fn clip_convex(p: &[(f64, f64)], q: &[(f64, f64); 4]) -> Vec<(f64, f64)> {
    let area: f64 = (0..4).map(|k| cross0(q[k], q[(k + 1) % 4])).sum();
    let mut out = p.to_vec();
    for k in 0..4 {
        let (a, b) = (q[k], q[(k + 1) % 4]);
        let side = |p: (f64, f64)| ((b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0)) * area.signum();
        let input = std::mem::take(&mut out);
        for i in 0..input.len() {
            let (c, d) = (input[(i + input.len() - 1) % input.len()], input[i]);
            let (sc, sd) = (side(c), side(d));
            if sd >= 0.0 {
                if sc < 0.0 {
                    let t = sc / (sc - sd);
                    out.push((c.0 + t * (d.0 - c.0), c.1 + t * (d.1 - c.1)));
                }
                out.push(d);
            } else if sc >= 0.0 {
                let t = sc / (sc - sd);
                out.push((c.0 + t * (d.0 - c.0), c.1 + t * (d.1 - c.1)));
            }
        }
        if out.is_empty() {
            break;
        }
    }
    out
}

fn cross0(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// what a count of 8 x 8 samples is the coverage of
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Samples {
    /// a filled primitive's edge (D2D.FILL.AABLEND)
    Fill,
    /// an antialiased clip or a layer's bounds and mask (D2D.CLIP.BLEND, D2D.LAYER.MASK)
    Mask,
}

impl Samples {
    /// MEASURED: the byte of n samples of 64; the two differ only at n = 32 (128 against 127,
    /// layerfit-*: the half covered column of the mask sweep)
    fn byte(self, n: u32) -> u32 {
        debug_assert!(n <= 64);
        match self {
            Self::Fill => (n * 4 * 255 + 0x80) >> 8,
            Self::Mask => (n * 4 * 255 + 0x7f) >> 8,
        }
    }
}

/// MEASURED (D2D.FILL.AA28): a coordinate in 1/16 pixel, ceil(16 v) - 1
fn snap16(v: f64) -> i64 {
    hit(clause::D2D_FILL_AA28);
    (16.0 * v).ceil() as i64 - 1
}

/// the coverage of closed polygons in 1/16 pixel (D2D.FILL.AA28): samples in eighths
fn coverage16(tg: &Target, ps: &[Vec<(i64, i64)>], wind: bool) -> Vec<u32> {
    samples16(tg, ps, wind).into_iter().map(u64::count_ones).collect()
}

/// which of each pixel's 64 samples (bit 8 row + column) closed polygons in 1/16 pixel cover
fn samples16(tg: &Target, ps: &[Vec<(i64, i64)>], wind: bool) -> Vec<u64> {
    let edges = fixed_edges(ps, 8);
    let mut out = vec![0u64; tg.w * tg.h];
    let cols = 8 * tg.w as i64;
    let mut act = Vec::new();
    for j in 0..8 * tg.h as i64 {
        for (a, b) in spans(&edges, j, wind, &mut act) {
            for c in a.max(0)..b.min(cols) {
                out[(j >> 3) as usize * tg.w + (c >> 3) as usize] |= 1 << ((j & 7) * 8 + (c & 7));
            }
        }
    }
    out
}

/// the ceiling of a / b, b > 0
fn ceil_div(a: i64, b: i64) -> i64 {
    -(-a).div_euclid(b)
}

/// WPF's Bezier32 (bezier.cpp, HfdBasis32): a cubic in 28.4 fixed point flattened by forward
/// differences; the points after the start, the last the end. None when it does not fit
/// (Bezier64's case)
pub fn hfd32(p: [(i64, i64); 4], clip: [i64; 4]) -> Option<Vec<(i64, i64)>> {
    const INITIAL: i64 = 10;
    const ADDITIONAL: i64 = 3;
    const SHIFT: i64 = INITIAL + ADDITIONAL;
    const MAX_ERROR: i64 = 24 << ((2 * INITIAL) / 3);
    const INITIAL_TEST: i64 = 24 << INITIAL;
    const TEST: i64 = INITIAL_TEST << ADDITIONAL;
    #[derive(Clone, Copy)]
    struct Hfd {
        e: [i64; 4],
    }
    impl Hfd {
        fn init(p1: i64, p2: i64, p3: i64, p4: i64) -> Option<Hfd> {
            let mut h = Hfd { e: [p1 << INITIAL, (p4 - p1) << INITIAL, 6 * (p2 - p3 - p3 + p4), 6 * (p1 - p2 - p2 + p3)] };
            if h.error() >= MAX_ERROR {
                return None;
            }
            h.e[2] <<= INITIAL;
            h.e[3] <<= INITIAL;
            Some(h)
        }
        fn error(&self) -> i64 {
            self.e[2].abs().max(self.e[3].abs())
        }
        fn parent_error4(&self) -> i64 {
            self.e[3].abs().max((self.e[2] + self.e[2] - self.e[3]).abs())
        }
        fn value(&self) -> i64 {
            (self.e[0] + (1 << (SHIFT - 1))) >> SHIFT
        }
        fn lazy_halve(&mut self, shift: i64) {
            self.e[2] = (self.e[2] + self.e[3]) >> 1;
            self.e[1] = (self.e[1] - (self.e[2] >> shift)) >> 1;
        }
        fn steady(&mut self, shift: i64) {
            self.e[0] <<= ADDITIONAL;
            self.e[1] <<= ADDITIONAL;
            let l = shift - ADDITIONAL;
            if l < 0 {
                self.e[2] <<= -l;
                self.e[3] <<= -l;
            } else {
                self.e[2] >>= l;
                self.e[3] >>= l;
            }
        }
        fn halve(&mut self) {
            self.e[2] = (self.e[2] + self.e[3]) >> 3;
            self.e[1] = (self.e[1] - self.e[2]) >> 1;
            self.e[3] >>= 2;
        }
        fn double(&mut self) {
            self.e[1] += self.e[1] + self.e[2];
            self.e[3] <<= 2;
            self.e[2] = (self.e[2] << 3) - self.e[3];
        }
        fn step(&mut self) {
            self.e[0] += self.e[1];
            let t = self.e[2];
            self.e[1] += t;
            self.e[2] += t - self.e[3];
            self.e[3] = t;
        }
    }
    let xs = p.map(|q| q.0);
    let ys = p.map(|q| q.1);
    let bound = [xs.into_iter().min()? - 16, ys.into_iter().min()? - 16, xs.into_iter().max()? + 16, ys.into_iter().max()? + 16];
    let rel = p.map(|q| (q.0 - bound[0], q.1 - bound[1]));
    if rel.iter().any(|q| (q.0 | q.1) & 0xffff_c000 != 0) {
        return None;
    }
    let mut x = Hfd::init(rel[0].0, rel[1].0, rel[2].0, rel[3].0)?;
    let mut y = Hfd::init(rel[0].1, rel[1].1, rel[2].1, rel[3].1)?;
    let mut steps: i64 = 1;
    let mut shift = 0;
    let meets = bound[0] < clip[2] && bound[1] < clip[3] && bound[2] > clip[0] && bound[3] > clip[1];
    if meets {
        loop {
            let test = INITIAL_TEST << shift;
            if x.error() <= test && y.error() <= test {
                break;
            }
            shift += 2;
            x.lazy_halve(shift);
            y.lazy_halve(shift);
            steps <<= 1;
        }
    }
    x.steady(shift);
    y.steady(shift);
    x.step();
    y.step();
    steps -= 1;
    let mut out = Vec::new();
    loop {
        out.push((x.value() + bound[0], y.value() + bound[1]));
        if steps == 0 {
            return Some(out);
        }
        if x.error().max(y.error()) > TEST {
            x.halve();
            y.halve();
            steps <<= 1;
        }
        while steps & 1 == 0 && x.parent_error4() <= TEST >> 2 && y.parent_error4() <= TEST >> 2 {
            x.double();
            y.double();
            steps >>= 1;
        }
        steps -= 1;
        x.step();
        y.step();
    }
}

/// WPF's Bezier64 (bezier.cpp, HfdBasis64), for a cubic Bezier32 cannot take: 28 fraction
/// bits, a coarse subdivision whose pieces are flattened again to 3 << 31
pub fn hfd64(p: [(i64, i64); 4], clip: [i64; 4]) -> Vec<(i64, i64)> {
    const FRACTION: i64 = 28;
    const ERROR_HIGH: i128 = ((6 * (1i128 << 15)) >> (32 - FRACTION)) << 32;
    const ERROR_LOW: i128 = 3i128 << 31;
    #[derive(Clone, Copy)]
    struct Hfd {
        e: [i128; 4],
    }
    impl Hfd {
        fn init(p1: i64, p2: i64, p3: i64, p4: i64) -> Hfd {
            let (p1, p2, p3, p4) = (i128::from(p1), i128::from(p2), i128::from(p3), i128::from(p4));
            let e2 = p2 - p3 - p3 + p4;
            let e3 = p1 - p2 - p2 + p3;
            Hfd { e: [p1 << FRACTION, (p4 - p1) << FRACTION, (3 * e2) << (FRACTION + 1), (3 * e3) << (FRACTION + 1)] }
        }
        fn error(&self) -> i128 {
            self.e[2].abs().max(self.e[3].abs())
        }
        fn parent_error(&self) -> i128 {
            (self.e[3] << 2).abs().max(((self.e[2] << 3) - (self.e[3] << 2)).abs())
        }
        fn value(&self) -> i64 {
            ((self.e[0] + (1 << (FRACTION - 1))) >> FRACTION) as i64
        }
        fn untransform(&self) -> [i64; 4] {
            let e = self.e;
            let p2 = e[1] * 3;
            let mut p1 = p2 * 2 - e[2];
            let mut p2 = p1 * 2 - e[3];
            p1 -= e[3] * 2;
            p1 /= 18;
            p2 /= 18;
            p1 += e[0];
            p2 += e[0];
            let p3 = e[0] + e[1];
            let r = |v: i128| ((v + (1 << (FRACTION - 1))) >> FRACTION) as i64;
            [r(e[0]), r(p1), r(p2), r(p3)]
        }
        fn halve(&mut self) {
            self.e[2] = (self.e[2] + self.e[3]) >> 3;
            self.e[1] = (self.e[1] - self.e[2]) >> 1;
            self.e[3] >>= 2;
        }
        fn double(&mut self) {
            self.e[1] = (self.e[1] << 1) + self.e[2];
            self.e[3] <<= 2;
            self.e[2] = (self.e[2] << 3) - self.e[3];
        }
        fn step(&mut self) {
            self.e[0] += self.e[1];
            let t = self.e[2];
            self.e[1] += t;
            self.e[2] += t - self.e[3];
            self.e[3] = t;
        }
    }
    let mut xh = Hfd::init(p[0].0, p[1].0, p[2].0, p[3].0);
    let mut yh = Hfd::init(p[0].1, p[1].1, p[2].1, p[3].1);
    let mut high: i64 = 1;
    while xh.error() > ERROR_HIGH || yh.error() > ERROR_HIGH {
        high <<= 1;
        xh.halve();
        yh.halve();
    }
    let (mut xl, mut yl) = (xh, yh);
    let mut low: i64 = 0;
    let mut out = Vec::new();
    loop {
        if low == 0 {
            let (ax, ay) = (xh.untransform(), yh.untransform());
            xl = Hfd::init(ax[0], ax[1], ax[2], ax[3]);
            yl = Hfd::init(ay[0], ay[1], ay[2], ay[3]);
            low = 1;
            let bound = [ax.into_iter().min().unwrap_or(0) - 16, ay.into_iter().min().unwrap_or(0) - 16, ax.into_iter().max().unwrap_or(0) + 16, ay.into_iter().max().unwrap_or(0) + 16];
            if bound[0] < clip[2] && bound[1] < clip[3] && bound[2] > clip[0] && bound[3] > clip[1] {
                while xl.error() > ERROR_LOW || yl.error() > ERROR_LOW {
                    low <<= 1;
                    xl.halve();
                    yl.halve();
                }
            }
            high -= 1;
            if high != 0 {
                xh.step();
                yh.step();
                if xh.error() > ERROR_HIGH || yh.error() > ERROR_HIGH {
                    high <<= 1;
                    xh.halve();
                    yh.halve();
                }
                while high & 1 == 0 && xh.parent_error() <= ERROR_HIGH && yh.parent_error() <= ERROR_HIGH {
                    xh.double();
                    yh.double();
                    high >>= 1;
                }
            }
        }
        xl.step();
        yl.step();
        out.push((xl.value(), yl.value()));
        low -= 1;
        if low == 0 && high == 0 {
            return out;
        }
        if xl.error() > ERROR_LOW || yl.error() > ERROR_LOW {
            low <<= 1;
            xl.halve();
            yl.halve();
        }
        while low & 1 == 0 && xl.parent_error() <= ERROR_LOW && yl.parent_error() <= ERROR_LOW {
            xl.double();
            yl.double();
            low >>= 1;
        }
    }
}
