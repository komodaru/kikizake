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
        hit("D2D.INV.SINGULAR");
        return (false, *m);
    }
    hit("D2D.INV");
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
        hit("D2D.ROT.NAN");
        let n = if angle.is_nan() { angle.to_bits() } else { INDEFINITE };
        let v = f32::from_bits(n);
        return [v, v, f32::from_bits(n ^ 0x8000_0000), v, v, v];
    }
    let mut r = angle % 360.0;
    if r < 0.0 {
        r += 360.0;
    }
    let (s, c) = if r % 90.0 == 0.0 {
        hit("D2D.ROT.RIGHT");
        [(0.0, 1.0), (1.0, 0.0), (0.0, -1.0), (-1.0, 0.0)][(r / 90.0) as usize]
    } else {
        hit("D2D.ROT");
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
    hit("D2D.SKEW");
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
        hit("D2D.CS.OTHER");
        return Ok(vec![0.0; 4]);
    }
    if from == to {
        hit("D2D.CS.SAME");
        return Ok(c.to_vec());
    }
    let mut o = c.to_vec();
    for v in o.iter_mut().take(3) {
        if v.is_nan() || *v <= 0.0 {
            *v = 0.0;
        } else if *v >= 1.0 {
            *v = 1.0;
        } else {
            return Err(Fail::Unmodeled("D2D.CS.CURVE"));
        }
    }
    hit("D2D.CS.CLAMP");
    Ok(o)
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

/// GetBounds, `m` the world transform (None: NULL)
pub fn bounds(g: &Geom, m: Option<&M>) -> Res<[f32; 4]> {
    match (g, m) {
        (Geom::Rect([l, t, r, b]) | Geom::RRect([l, t, r, b, ..]), None) => {
            // (MEASURED: the rectangle put in order, a rounded one's radii not mattering)
            hit(if matches!(g, Geom::Rect(_)) { "D2D.BOUNDS.RECT" } else { "D2D.BOUNDS.RRECT" });
            if [l, t, r, b].iter().any(|v| v.is_nan()) {
                hit("D2D.BOUNDS.NAN");
            }
            Ok([lo(*l, *r), lo(*t, *b), hi(*l, *r), hi(*t, *b)])
        }
        (Geom::RRect(_), Some(_)) => Err(Fail::Unmodeled("D2D.BOUNDS.RRECTXF")),
        (Geom::Rect(_) | Geom::Path { .. }, _) => {
            let (_, figs, m0) = g.lines().unwrap();
            let m = mul(&m0, m.unwrap_or(&IDENTITY));
            // (MEASURED: a path with a NaN anywhere has four NaN bounds, every bit set)
            if matches!(g, Geom::Path { .. }) && figs.iter().flat_map(|f| f.pts.iter()).any(|p| p.0.is_nan() || p.1.is_nan()) {
                hit("D2D.BOUNDS.PATHNAN");
                return Ok(NAN_BOUNDS);
            }
            let mut b = [f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY];
            hit(if matches!(g, Geom::Rect(_)) { "D2D.BOUNDS.RECTXF" } else { "D2D.BOUNDS.PATH" });
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
                hit("D2D.BOUNDS.NAN");
                (b[0], b[2]) = (NAN_BOUNDS[0], NAN_BOUNDS[0]);
            }
            if ny {
                hit("D2D.BOUNDS.NAN");
                (b[1], b[3]) = (NAN_BOUNDS[0], NAN_BOUNDS[0]);
            }
            Ok(b)
        }
        (Geom::Ell([cx, cy, rx, ry]), _) => {
            let m = m.unwrap_or(&IDENTITY);
            if [cx, cy, rx, ry].iter().any(|v| v.is_nan()) {
                hit("D2D.BOUNDS.NAN");
                return Ok(NAN_BOUNDS);
            }
            if m[1] != 0.0 || m[2] != 0.0 || [cx, cy, rx, ry].iter().any(|v| !v.is_finite()) {
                return Err(Fail::Unmodeled("D2D.BOUNDS.ELLXF"));
            }
            // (MEASURED: a negative radius counts as its size)
            hit("D2D.BOUNDS.ELL");
            let (rx, ry) = (rx.abs(), ry.abs());
            let (x0, y0) = apply(m, cx - rx, cy - ry);
            let (x1, y1) = apply(m, cx + rx, cy + ry);
            Ok([x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)])
        }
        (Geom::Xf(inner, m0), _) => {
            if !matches!(**inner, Geom::Rect(_) | Geom::Path { .. }) {
                return Err(Fail::Unmodeled("D2D.BOUNDS.ELLXF"));
            }
            hit("D2D.XF");
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
    if let Geom::Ell([cx, cy, rx, ry]) = g {
        let v = ((p.0 - *cx as f64) / *rx as f64).powi(2) + ((p.1 - *cy as f64) / *ry as f64).powi(2);
        // (the distance to the curve, near enough for a band around the tolerance)
        let d = (v.sqrt() - 1.0).abs() * (*rx).min(*ry) as f64;
        if d < tol as f64 * 2.0 + 0.0625 {
            return Err(Fail::Unmodeled("D2D.IN.ELLNEAR"));
        }
        hit("D2D.IN.ELL");
        return Ok(v < 1.0);
    }
    let Some((wind, figs, m)) = g.lines() else { return Err(Fail::Unmodeled("D2D.IN.RRECT")) };
    let ps = polys(&figs, &m);
    let d = ps.iter().flat_map(|q| segs(q)).map(|(a, b)| dist_seg(p, a, b)).fold(f64::INFINITY, f64::min);
    // (MEASURED: a tolerance of 0 is the default 0.25; inside when strictly nearer than it)
    let tol = if tol == 0.0 { hit("D2D.IN.TOL0"); 0.25 } else { tol as f64 };
    hit("D2D.IN.TOL");
    if d < tol {
        return Ok(true);
    }
    // (MEASURED: 0.2505 from a side with the tolerance 0.25 is out)
    if d < tol + 1e-3 {
        hit("D2D.IN.EDGE");
    }
    let w = winding(&ps, p);
    hit(if wind { "D2D.IN.WIND" } else { "D2D.IN.ALT" });
    Ok(if wind { w != 0 } else { w % 2 != 0 })
}

/// the area the fill mode fills: the figures (open ones as closed) cut into slabs at every
/// vertex and crossing, in each slab the inside spans by the winding at its middle, trapezoids
pub fn region_area(ps: &[Vec<(f64, f64)>], wind: bool) -> f64 {
    let edges: Vec<((f64, f64), (f64, f64))> = ps.iter().flat_map(|p| segs(p)).filter(|(a, b)| a.1 != b.1).collect();
    let mut ys: Vec<f64> = ps.iter().flatten().map(|p| p.1).collect();
    for &(a, b) in &edges {
        for &(c, d) in &edges {
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
    let mut area = 0.0;
    for w in ys.windows(2) {
        let (y0, y1) = (w[0], w[1]);
        let ym = (y0 + y1) / 2.0;
        let x = |e: &((f64, f64), (f64, f64)), y: f64| e.0 .0 + (y - e.0 .1) * (e.1 .0 - e.0 .0) / (e.1 .1 - e.0 .1);
        let mut act: Vec<&((f64, f64), (f64, f64))> = edges.iter().filter(|e| e.0 .1.min(e.1 .1) <= y0 && e.0 .1.max(e.1 .1) >= y1).collect();
        act.sort_by(|a, b| x(a, ym).total_cmp(&x(b, ym)));
        let (mut n, mut w0, mut w1, mut from) = (0i32, 0.0, 0.0, None);
        for e in act {
            let was = if wind { n != 0 } else { n % 2 != 0 };
            n += (e.1 .1 - e.0 .1).signum() as i32;
            let now = if wind { n != 0 } else { n % 2 != 0 };
            if !was && now {
                from = Some(e);
            } else if was && !now {
                let f = from.unwrap();
                w0 += x(e, y0) - x(f, y0);
                w1 += x(e, y1) - x(f, y1);
            }
        }
        area += (w0 + w1) / 2.0 * (y1 - y0);
    }
    area
}

/// ComputeArea. MEASURED (golden d2d-32/-64, 66 paths of up to three figures, self-crossing and
/// overlapping ones too): the area the fill mode fills, open figures filled as closed
pub fn area(g: &Geom) -> Res<f32> {
    let Some((wind, figs, m)) = g.lines() else { return Err(Fail::Unmodeled("D2D.AREA.CURVE")) };
    if let Geom::Rect([l, t, r, b]) = g {
        hit("D2D.AREA.RECT");
        return Ok(((r - l) * (b - t)).abs());
    }
    hit("D2D.AREA.POLY");
    Ok(region_area(&polys(&figs, &m), wind) as f32)
}

/// ComputeLength. MEASURED (golden d2d-32/-64, 66 paths bit for bit): each side's length as
/// sqrt(dx dx + dy dy) in f32, summed in f32 in order over every figure, the closing side of a
/// closed figure included
pub fn length(g: &Geom) -> Res<f32> {
    let Some((_, figs, m)) = g.lines() else { return Err(Fail::Unmodeled("D2D.LEN.CURVE")) };
    hit("D2D.LEN");
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
pub fn relation(a: &Geom, b: &Geom) -> Res<u32> {
    let (Geom::Rect(a), Geom::Rect(b)) = (a, b) else { return Err(Fail::Unmodeled("D2D.CMP.SHAPE")) };
    let n = |r: &[f32; 4]| [r[0].min(r[2]), r[1].min(r[3]), r[0].max(r[2]), r[1].max(r[3])];
    let (a, b) = (n(a), n(b));
    hit("D2D.CMP");
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
    pub aliased: bool,
    pub m: M,
}

/// a straight color (r, g, b, a as 0..255) as the premultiplied bytes Clear and an opaque brush
/// write, B G R A
pub fn premultiply(c: [i64; 4]) -> [u8; 4] {
    let f = |v: i64| v as f32 / 255.0;
    let a = f(c[3]);
    let ch = |v: i64| ((f(v) * a) * 255.0 + 0.5).floor() as u8;
    [ch(c[2]), ch(c[1]), ch(c[0]), (a * 255.0 + 0.5).floor() as u8]
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
                hit("D2D.TRIG");
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
                hit("D2D.VEC3");
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
                hit("D2D.CREATE");
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
                hit("D2D.XF");
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
                hit("D2D.PATH");
                self.g[g] = Some(Geom::Path { wind, figs });
                out.push("path 00000000 00000000".into());
            }
            "d2sink" => self.sink(op, out)?,
            "d2simp" | "d2outline" | "d2widen" | "d2combine" | "d2tess" => {
                let g = self.geom(op, 1, out)?;
                let mut at = 2;
                match t[0].as_str() {
                    "d2simp" => {
                        if !matches!(t.get(2).map(String::as_str), Some("lines" | "cubics")) {
                            return Err(Fail::Bad);
                        }
                        at = 3;
                    }
                    "d2widen" => {
                        f(op, 2)?;
                        at = 3;
                    }
                    "d2combine" => {
                        self.geom(op, 2, out)?.ok_or(Fail::Unmodeled("D2D.SIMP"))?;
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
                if g.is_none() {
                    return Ok(true);
                }
                return Err(Fail::Unmodeled("D2D.SIMP"));
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
                want(2)?;
                let Some(g) = g else { return Ok(true) };
                let v = if t[0] == "d2area" { area(&g)? } else { length(&g)? };
                out.push(format!("{} 00000000 {}", &t[0][2..], hex(v)));
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
                    hit("D2D.COUNT.NOPATH");
                    out.push("count nopath".into());
                    return Ok(true);
                };
                hit("D2D.COUNT");
                let segs: usize = figs.iter().map(|f| f.pts.len() - 1 + f.closed as usize).sum();
                out.push(format!("count 00000000 {} 00000000 {segs}", figs.len()));
            }
            "d2target" | "d2aa" | "d2xform" | "d2clear" | "d2fill" | "d2fillg" | "d2brush" | "d2read" => self.draw(op, out)?,
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn stroke(&mut self, op: &Op, out: &mut Vec<String>) -> Res<()> {
        let p: Vec<i64> = [1, 2, 3, 4, 6].iter().map(|&i| n(op, i)).collect::<Res<_>>()?;
        let (miter, offset) = (f(op, 5)?, f(op, 7)?);
        let dashes: Vec<f32> = (8..op.tok.len()).map(|i| f(op, i)).collect::<Res<_>>()?;
        if dashes.len() > 32 {
            return Err(Fail::Bad);
        }
        let (caps, join, style) = (&p[0..3], p[3], p[4]);
        hit("D2D.STROKE.ENUM");
        let ok = caps.iter().all(|c| (0..=3).contains(c)) && (0..=3).contains(&join) && (0..=5).contains(&style);
        if !ok {
            out.push(format!("stroke hr {E_INVALIDARG:08x}"));
            return Ok(());
        }
        let custom = style == 5;
        if custom != !dashes.is_empty() {
            hit("D2D.STROKE.DASHES");
            out.push(format!("stroke hr {E_INVALIDARG:08x}"));
            return Ok(());
        }
        // (MEASURED: any miter limit, offset and dash value is kept as given, NaN, infinite and
        // negative ones included)
        hit("D2D.STROKE.VALUES");
        // (MEASURED: the predefined styles' patterns, in stroke widths, whatever the caps)
        let pattern: &[f32] = match style {
            1 => &[2.0, 2.0],
            2 => &[0.0, 2.0],
            3 => &[2.0, 2.0, 0.0, 2.0],
            4 => &[2.0, 2.0, 0.0, 2.0, 0.0, 2.0],
            _ => &dashes,
        };
        if (1..=4).contains(&style) {
            hit("D2D.STROKE.PATTERN");
        }
        hit("D2D.STROKE.GET");
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
                    hit("D2D.SINK.OPEN");
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
                        hit("D2D.SINK.CLOSE");
                        k.state = SinkState::Closed;
                        s += " close 00000000";
                    } else {
                        hit("D2D.SINK.ERROR");
                        // (MEASURED: a second Close leaves the closed geometry as it was)
                        if k.state != SinkState::Closed {
                            k.state = SinkState::Failed;
                        }
                        s += &format!(" close {D2DERR_WRONG_STATE:08x}");
                    }
                }
                "count" => {
                    if k.state == SinkState::Closed {
                        hit("D2D.COUNT");
                        s += &format!(" count 00000000 {} 00000000 {}", k.figs, k.segs);
                    } else {
                        // (MEASURED: D2DERR_WRONG_STATE, the counts not written)
                        hit("D2D.SINK.COUNT");
                        s += &format!(" count {D2DERR_WRONG_STATE:08x} 57005 {D2DERR_WRONG_STATE:08x} 57005");
                    }
                }
                "begin" | "line" => {
                    f(op, i + 1)?;
                    f(op, i + 2)?;
                    if k.sink {
                        if k.state != SinkState::Open {
                            // (MEASURED: ignored once closed)
                            hit("D2D.SINK.AFTER");
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
                            hit("D2D.SINK.AFTER");
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
                        hit("D2D.SINK.FILL");
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
                hit("D2D.TARGET");
                self.t = Some(Target { w: w as usize, h: h as usize, px: vec![[0; 4]; (w * h) as usize], aliased: false, m: IDENTITY });
                out.push("target 00000000".into());
                return Ok(());
            }
            "d2aa" => {
                let al = match t.get(1).map(String::as_str) {
                    Some("aliased") => true,
                    Some("per") => false,
                    _ => return Err(Fail::Bad),
                };
                if len != 2 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    tg.aliased = al;
                    return Ok(());
                }
            }
            "d2xform" => {
                let m = mat(op, 1)?;
                if len != 7 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    tg.m = m;
                    return Ok(());
                }
            }
            "d2clear" => {
                let c = color(1)?;
                if len != 5 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &mut self.t {
                    hit("D2D.CLEAR");
                    let p = premultiply(c);
                    tg.px.iter_mut().for_each(|q| *q = p);
                    return Ok(());
                }
            }
            "d2fill" | "d2fillg" => {
                let geo = t[0] == "d2fillg";
                let (g, c) = if geo {
                    let g = Self::slot(op, 1)?;
                    let c = color(2)?;
                    if len != 6 {
                        return Err(Fail::Bad);
                    }
                    (self.g[g].clone(), c)
                } else {
                    let r = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                    let c = color(5)?;
                    if len != 9 {
                        return Err(Fail::Bad);
                    }
                    (Some(Geom::Rect(r)), c)
                };
                let Some(tg) = &mut self.t else {
                    out.push("nort".into());
                    return Ok(());
                };
                let Some(g) = g else {
                    out.push("nogeom".into());
                    return Ok(());
                };
                return fill(tg, &g, c);
            }
            "d2brush" => {
                let c = [f(op, 1)?, f(op, 2)?, f(op, 3)?, f(op, 4)?];
                if len != 5 {
                    return Err(Fail::Bad);
                }
                if self.t.is_some() {
                    hit("D2D.BRUSH");
                    out.push(format!("brush {} {} {} {} {}", hex(c[0]), hex(c[1]), hex(c[2]), hex(c[3]), hex(1.0)));
                    return Ok(());
                }
            }
            _ => {
                if len != 1 {
                    return Err(Fail::Bad);
                }
                if let Some(tg) = &self.t {
                    hit("D2D.READ");
                    out.push("end 00000000".into());
                    out.push(format!("px 0 {} {} 32", tg.w, tg.h));
                    for y in 0..tg.h {
                        out.push(tg.px[y * tg.w..(y + 1) * tg.w].iter().flat_map(|p| p.iter()).map(|b| format!("{b:02x}")).collect());
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
fn fill(tg: &mut Target, g: &Geom, c: [i64; 4]) -> Res<()> {
    let Some((wind, figs, m0)) = g.lines() else { return Err(Fail::Unmodeled("D2D.FILL.CURVE")) };
    let m = mul(&m0, &tg.m);
    let ps = polys(&figs, &m);
    let axis = matches!(g, Geom::Rect(_)) && m[1] == 0.0 && m[2] == 0.0;
    if !tg.aliased {
        let grid = ps.iter().flatten().all(|p| p.0.fract() == 0.0 && p.1.fract() == 0.0);
        if axis && grid {
            hit("D2D.FILL.AAGRID");
        } else {
            return fill_aa(tg, &ps, axis, c);
        }
    }
    let src = premultiply(c);
    if src[3] != 255 {
        hit("D2D.FILL.OVER");
    }
    for y in 0..tg.h {
        for x in 0..tg.w {
            let p = (x as f64 + 0.5, y as f64 + 0.5);
            // (MEASURED: a center on an edge is inside only on a left or top edge: the winding
            // counts sides whose y range [min, max) holds the center and that pass strictly to
            // its right)
            if ps.iter().flat_map(|q| segs(q)).any(|(a, b)| cross(a, b, p) == 0.0 && dist_seg(p, a, b) == 0.0) {
                hit("D2D.FILL.EDGE");
            }
            let w = winding(&ps, p);
            if if wind { w != 0 } else { w % 2 != 0 } {
                hit(if axis { "D2D.FILL.RECT" } else { "D2D.FILL.POLY" });
                let i = y * tg.w + x;
                tg.px[i] = over(src, tg.px[i]);
            }
        }
    }
    Ok(())
}

/// antialiased: an axis-aligned rectangle with edges on quarter pixels, a white opaque color on
/// an empty target. MEASURED (golden d2d-32/-64, aa-*): each pixel the exact covered fraction
/// of it, every byte round(coverage 255) (0.25 -> 0x40, 0.5 -> 0x80, 0.75 -> 0xbf)
fn fill_aa(tg: &mut Target, ps: &[Vec<(f64, f64)>], axis: bool, c: [i64; 4]) -> Res<()> {
    let quarter = ps.iter().flatten().all(|p| (p.0 * 4.0).fract() == 0.0 && (p.1 * 4.0).fract() == 0.0);
    if !axis || !quarter || c != [255; 4] || tg.px.iter().any(|p| *p != [0; 4]) {
        return Err(Fail::Unmodeled("D2D.FILL.AA"));
    }
    hit("D2D.FILL.AACOVER");
    let xs: Vec<f64> = ps[0].iter().map(|p| p.0).collect();
    let ys: Vec<f64> = ps[0].iter().map(|p| p.1).collect();
    let (l, r) = (xs.iter().cloned().fold(f64::INFINITY, f64::min), xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let (t, b) = (ys.iter().cloned().fold(f64::INFINITY, f64::min), ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    for y in 0..tg.h {
        for x in 0..tg.w {
            let cx = (r.min(x as f64 + 1.0) - l.max(x as f64)).max(0.0);
            let cy = (b.min(y as f64 + 1.0) - t.max(y as f64)).max(0.0);
            let v = (cx * cy * 255.0 + 0.5).floor() as u8;
            tg.px[y * tg.w + x] = [v; 4];
        }
    }
    Ok(())
}
