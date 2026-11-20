//! The software rasterizers of Direct3D immediate mode (RGB, MMX and ramp): triangle setup, the
//! edge walk and the Gouraud spans, as the clean-room notes on d3dim700 (10.0.26100.5074)
//! describe them, each fitted to Windows' answers (spec/d3d7.txt D3D7.TRI.*, D3D7.GOURAUD).
//!
//! The rasterizer runs the x87 unit in single precision rounding toward zero (its divides,
//! products and float-to-int conversions), while its SSE arithmetic rounds to nearest: each step
//! below says which it takes. Rust's f32 operations are the SSE ones.
use super::{Fail, Res};

/// 1 − 2^-23, the ceiling helper's addend
const E: f32 = f32::from_bits(0x3f7f_fffe);
/// 2^31, the edge walk's fraction scale
const TWO31: f32 = 2_147_483_648.0;

/// an x87 result: the exact value (an f64 holding a product of two f32s exactly, or a quotient
/// close enough to decide the f32 below it) rounded toward zero to f32
fn x87(x: f64) -> f32 {
    let f = x as f32;
    if f.is_finite() && f64::from(f).abs() > x.abs() { f32::from_bits(f.to_bits() - 1) } else { f }
}

fn x87_div(a: f32, b: f32) -> f32 {
    x87(f64::from(a) / f64::from(b))
}

fn x87_mul(a: f32, b: f32) -> f32 {
    x87(f64::from(a) * f64::from(b))
}

/// float to int toward zero (the x87 fistp under chop, or cvttss2si), None outside i64
fn trunc(v: f32) -> Option<i64> {
    let t = v.trunc();
    (t.is_finite() && t.abs() < 9_223_372_036_854_775_808.0).then(|| t as i64)
}

/// ICEIL: x + (1 − 2^-23) rounded to nearest, then truncated, for x > 0 (a whole x from 2 up
/// gives x + 1); truncation (the ceiling) for x ≤ 0
fn iceil(v: f32) -> Option<i64> {
    if v > 0.0 { trunc(v + E) } else { trunc(v) }
}

/// IFLOOR: x − (1 − 2^-23) rounded to nearest, then truncated, for x < 0 (a whole x from −2
/// down gives x − 1); truncation for x ≥ 0
fn ifloor(v: f32) -> Option<i64> {
    if v < 0.0 { trunc(v - E) } else { trunc(v) }
}

/// the low 16 bits (the 8.8 colour words wrap)
fn low16(v: i64) -> u16 {
    u16::try_from(v.rem_euclid(1 << 16)).unwrap_or(0)
}

/// the four channels' sums a + k · b (an SSE product and add each)
fn add_scaled(a: &mut [f32; 4], k: f32, b: [f32; 4]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += k * y;
    }
}

/// the four channels' sums a + b
fn add(a: &mut [f32; 4], b: [f32; 4]) {
    for (x, y) in a.iter_mut().zip(b) {
        *x += y;
    }
}

/// a span's i-th pixel: its first colour plus i steps, each word wrapping
fn along(c0: [u16; 4], i: i64, step: [i64; 4]) -> [u16; 4] {
    let mut c = [0u16; 4];
    for ((o, s), d) in c.iter_mut().zip(c0).zip(step) {
        *o = low16(i64::from(s).wrapping_add(i.wrapping_mul(d)));
    }
    c
}

fn f32_of(v: i64) -> f32 {
    // (pixel counts and rows: small, exact)
    v as f32
}

/// one edge's walk: its column on the current row and a 31-bit error term
#[derive(Clone, Copy, Debug)]
struct Edge {
    x: i64,
    err: u32,
    istep: i64,
    fstep: u32,
    cstep: i64,
}

/// whether a row step carried (took cstep) or not
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Step {
    Plain,
    Carry,
}

impl Edge {
    /// the edge through x0 on its first row with this slope
    fn new(x0: f32, slope: f32) -> Option<Edge> {
        let x = iceil(x0)?;
        let frac = x0 - f32_of(ifloor(x0)?);
        let f = trunc(x87_mul(frac, TWO31))?;
        let err = u32::try_from((f - 1) & 0x7fff_ffff).ok()?;
        let istep = trunc(slope)?;
        let fs = trunc(x87_mul(slope - f32_of(istep), TWO31))?;
        // (the fraction's bits as the 32-bit add sees them: negative for a negative slope)
        let fstep = u32::try_from(fs.rem_euclid(1 << 32)).ok()?;
        let cstep = if slope < 0.0 { istep - 1 } else { istep + 1 };
        Some(Edge { x, err, istep, fstep, cstep })
    }
    fn step(&mut self) -> Step {
        self.err = self.err.wrapping_add(self.fstep);
        if self.err & 0x8000_0000 != 0 {
            self.err &= 0x7fff_ffff;
            self.x += self.cstep;
            Step::Carry
        } else {
            self.x += self.istep;
            Step::Plain
        }
    }
}

/// a vertex in pixels (f32) with its diffuse color's channels B, G, R, A
#[derive(Clone, Copy, Debug)]
pub struct Vert {
    pub x: f32,
    pub y: f32,
    pub c: [u8; 4],
}

/// the clip rectangle, exclusive right and bottom
#[derive(Clone, Copy, Debug)]
pub struct Clip {
    pub l: i64,
    pub t: i64,
    pub r: i64,
    pub b: i64,
}

/// which walker drew a triangle
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Walker {
    /// the triangle lies inside horizontally: spans unclamped (past the right side on in memory)
    NoClip,
    /// crossing a side, or a span too wide: each row cut at the sides, in chunks
    Clip,
}

/// one written pixel: its row and column as the span walks them (a NoClip span may run past
/// the right side) and its colour B, G, R, A in 8.8 (8-bit value the high byte)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Px {
    pub x: i64,
    pub y: i64,
    pub c: [u16; 4],
}

/// a rasterized triangle
#[derive(Debug)]
pub struct Raster {
    pub walker: Walker,
    pub px: Vec<Px>,
    /// a whole bottom vertex's row left out by ICEIL (D3D7.TRI.EDGE)
    pub whole_bottom: bool,
}

/// the vertices top, middle, bottom by y alone, ties in call order except that B and C level
/// below A put C first; and whether that order is an odd permutation
fn sort(a: Vert, b: Vert, c: Vert) -> ([Vert; 3], bool) {
    if b.y >= a.y {
        if c.y >= b.y {
            ([a, b, c], false)
        } else if c.y >= a.y {
            ([a, c, b], true)
        } else {
            ([c, a, b], false)
        }
    } else if c.y > b.y {
        if c.y < a.y { ([b, c, a], false) } else { ([b, a, c], true) }
    } else {
        ([c, b, a], true)
    }
}

/// the colour attribute: per channel its x and y gradients (8.8 per pixel) and accumulators
#[derive(Clone, Copy, Debug)]
struct Colors {
    dx: [f32; 4],
    dy: [f32; 4],
}

/// the pixels and Gouraud colours of triangle (a, b, c) in call order, None when nothing is
/// drawn; `gouraud` false: the colours are the first vertex's, whole
pub fn raster(v: [Vert; 3], clip: Clip, gouraud: bool) -> Res<Option<Raster>> {
    let bad = || Fail::Unmodeled(crate::spec::open::D3D7_TRI_CLIP);
    let [va, vb, vc] = v;
    let ([t, m, bo], _flip) = sort(va, vb, vc);
    let (px_, py_) = (m.x - t.x, m.y - t.y);
    let (qx, qy) = (bo.x - t.x, bo.y - t.y);
    let det = py_ * qx - qy * px_;
    if det == 0.0 {
        return Ok(None);
    }
    let reversed = det > 0.0;
    let (mut ytop, mut ybot) = (iceil(t.y).ok_or_else(bad)?, iceil(bo.y).ok_or_else(bad)?);
    if ytop >= clip.b || ybot <= clip.t {
        return Ok(None);
    }
    let xs = [iceil(t.x).ok_or_else(bad)?, iceil(m.x).ok_or_else(bad)?, iceil(bo.x).ok_or_else(bad)?];
    let (xmin, xmax) = (xs.iter().copied().min().unwrap_or(0), xs.iter().copied().max().unwrap_or(0));
    if !(xmin < clip.r && xmax > clip.l && xmax - xmin > 0) {
        return Ok(None);
    }
    let inside_h = xmin >= clip.l && xmax <= clip.r;
    let mut ymid = iceil(m.y).ok_or_else(bad)?;
    ytop = ytop.max(clip.t);
    ymid = ymid.max(clip.t);
    if ymid > clip.b {
        ymid = clip.b;
        ybot = clip.b;
    } else {
        ybot = ybot.min(clip.b);
    }
    let (n1, n2) = (ymid - ytop, ybot - ymid);
    if n1 + n2 <= 0 {
        return Ok(None);
    }
    let whole_bottom = bo.y.fract() == 0.0 && iceil(bo.y) == trunc(bo.y);
    let slope_l = x87_div(qx, qy);
    let fdy = f32_of(ytop) - t.y;
    let pl = fdy * slope_l;
    let xl0 = pl + t.x;
    let mut long = Edge::new(xl0, slope_l).ok_or_else(bad)?;
    let mut e1 = if n1 > 0 {
        let s1 = px_ / py_;
        Some(Edge::new(s1 * fdy + t.x, s1).ok_or_else(bad)?)
    } else {
        None
    };
    let mut e2 = if n2 > 0 {
        let s2 = (bo.x - m.x) / (bo.y - m.y);
        Some(Edge::new((f32_of(ymid) - m.y) * s2 + m.x, s2).ok_or_else(bad)?)
    } else {
        None
    };
    if reversed {
        long.x -= 1;
        if let Some(e) = e1.as_mut() {
            e.x -= 1;
        }
        if let Some(e) = e2.as_mut() {
            e.x -= 1;
        }
    }
    // the colours: gradients from 1/det rounded toward zero
    let col = if gouraud {
        let i = x87_div(1.0, det);
        let (a, b, c, d) = (px_ * i, py_ * i, qx * i, qy * i);
        let mut dx = [0f32; 4];
        let mut dy = [0f32; 4];
        for ((((gx, gy), ct), cm), cb) in dx.iter_mut().zip(dy.iter_mut()).zip(t.c).zip(m.c).zip(bo.c) {
            let d1 = f32_of(256 * (i64::from(cm) - i64::from(ct)));
            let d2 = f32_of(256 * (i64::from(cb) - i64::from(ct)));
            *gx = b * d2 - d * d1;
            *gy = c * d1 - a * d2;
        }
        Colors { dx, dy }
    } else {
        Colors { dx: [0.0; 4], dy: [0.0; 4] }
    };
    let base = if gouraud { t.c } else { va.c };
    let dir = if reversed { -1i64 } else { 1 };
    let sdx = col.dx.map(|g| if reversed { -g } else { g });
    // the per-pixel steps in 8.8; a colour changing by 128 levels a pixel or more: every pixel
    // its own span
    let steep = sdx.iter().any(|g| g.abs() >= 32768.0);
    let pstep = sdx.map(|g| trunc(g).unwrap_or(0));
    let max_span = if steep { 1 } else { 0x100 };
    let walker = if inside_h && xmax - xmin <= max_span { Walker::NoClip } else { Walker::Clip };
    // the first row's anchor: the long edge's pixel, held to the clip rectangle
    let x_first = if reversed { long.x.min(clip.r - 1) } else { long.x.max(clip.l) };
    let fdx = (f32_of(x_first) - xl0) + pl;
    let mut acc = [0f32; 4];
    for (((o, gx), gy), cb) in acc.iter_mut().zip(col.dx).zip(col.dy).zip(base) {
        *o = (fdx * gx + f32_of(256 * i64::from(cb))) + fdy * gy;
    }
    let istep_l = f32_of(long.istep);
    let mut step_nc = col.dy;
    add_scaled(&mut step_nc, istep_l, col.dx);
    let mut step_c = step_nc;
    add_scaled(&mut step_c, if long.cstep > long.istep { 1.0 } else { -1.0 }, col.dx);
    let outside = |x: i64| if reversed { x >= clip.r } else { x < clip.l };
    let side = if reversed { clip.r - 1 } else { clip.l };
    let mut parked = walker == Walker::Clip && outside(long.x);
    let start16 = |a: [f32; 4]| -> [u16; 4] { a.map(|v| trunc(v).map_or(0, low16)) };
    let mut out = Vec::new();
    let rows = (ytop..ymid).map(|y| (y, true)).chain((ymid..ybot).map(|y| (y, false)));
    for (y, upper) in rows {
        let short = if upper { e1.as_ref() } else { e2.as_ref() };
        let Some(short) = short else { continue };
        let xs = short.x;
        match walker {
            Walker::NoClip => {
                let n = if reversed { long.x - xs } else { xs - long.x };
                let c0 = start16(acc);
                for i in 0..n.max(0) {
                    out.push(Px { x: long.x + dir * i, y, c: along(c0, i, pstep) });
                }
            }
            Walker::Clip => {
                let ended = if reversed { long.x < clip.l && long.cstep <= 0 } else { long.x >= clip.r && long.cstep >= 0 };
                if ended {
                    break;
                }
                let start = if parked { side } else { long.x };
                let n = if reversed { start - xs.max(clip.l - 1) } else { xs.min(clip.r) - start };
                let mut tmp = acc;
                let mut done = 0;
                while done < n {
                    let len = (n - done).min(max_span);
                    if done > 0 {
                        // (a further chunk of the row: from a copy, by the previous chunk's length)
                        add_scaled(&mut tmp, f32_of(max_span), sdx);
                    }
                    let c0 = start16(tmp);
                    for i in 0..len {
                        out.push(Px { x: start + dir * (done + i), y, c: along(c0, i, pstep) });
                    }
                    done += len;
                }
            }
        }
        // the next row
        let st = long.step();
        if let Some(e) = if upper { e1.as_mut() } else { e2.as_mut() } {
            e.step();
        }
        match walker {
            Walker::NoClip => {
                add(&mut acc, if st == Step::Carry { step_c } else { step_nc });
            }
            Walker::Clip => {
                if parked {
                    add(&mut acc, col.dy);
                    if !outside(long.x) {
                        add_scaled(&mut acc, f32_of(if reversed { clip.r - 1 - long.x } else { long.x - clip.l }), sdx);
                        parked = false;
                    }
                } else {
                    add(&mut acc, if st == Step::Carry { step_c } else { step_nc });
                    if outside(long.x) {
                        add_scaled(&mut acc, f32_of(if reversed { long.x - clip.r + 1 } else { clip.l - long.x }), sdx);
                        parked = true;
                    }
                }
            }
        }
    }
    Ok(Some(Raster { walker, px: out, whole_bottom }))
}
