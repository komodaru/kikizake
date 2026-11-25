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
const TWO31: f32 = f32::from_bits(0x4f00_0000);

/// an x87 result: the exact value (an f64 holding a product of two f32s exactly, or a quotient
/// precise enough to decide the f32 below it) rounded toward zero to f32: its sign, exponent and
/// top 23 fraction bits (0 below f32's normal range, infinity above it)
fn x87(x: f64) -> f32 {
    let b = x.to_bits();
    let sign = u32::from(b >> 63 == 1) << 31;
    let exp = i64::try_from(b >> 52 & 0x7ff).unwrap_or(0);
    let e = exp.wrapping_sub(1023 - 127);
    let man = u32::try_from(b >> 29 & 0x7f_ffff).unwrap_or(0);
    match (exp, u32::try_from(e)) {
        (0, _) | (_, Err(_)) | (_, Ok(0)) => f32::from_bits(sign),
        (_, Ok(e @ 1..=254)) => f32::from_bits(sign | e << 23 | man),
        (_, Ok(_)) => f32::from_bits(sign | 0x7f80_0000),
    }
}

fn x87_div(a: f32, b: f32) -> f32 {
    x87(f64::from(a) / f64::from(b))
}

pub(crate) fn x87_mul(a: f32, b: f32) -> f32 {
    x87(f64::from(a) * f64::from(b))
}

/// float to int toward zero (the x87 fistp under chop, or cvttss2si), from its bits; None for
/// 2^62 and above or not finite
pub(crate) fn trunc(v: f32) -> Option<i64> {
    let b = v.to_bits();
    let exp = i64::from(b >> 23 & 0xff).wrapping_sub(127);
    let man = i64::from(b & 0x7f_ffff | 0x80_0000);
    let mag = match exp {
        ..0 => 0,
        0..=22 => man.checked_shr(u32::try_from(23i64.wrapping_sub(exp)).ok()?)?,
        23..=61 => man.checked_shl(u32::try_from(exp.wrapping_sub(23)).ok()?)?,
        62.. => return None,
    };
    Some(if b >> 31 == 1 { mag.wrapping_neg() } else { mag })
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

/// an integer below 2^24 in magnitude as f32, exactly (two 16-bit halves); None above
pub(crate) fn f32_of(v: i64) -> Option<f32> {
    if v.unsigned_abs() >= 1 << 24 {
        return None;
    }
    raster_long(v)
}

/// Both 16-bit halves and their power-of-two shift are exact in f32. Only
/// the final addition rounds, including for a LONG outside the exact range.
fn raster_long(v: i64) -> Option<f32> {
    let hi = i16::try_from(v >> 16).ok()?;
    let lo = u16::try_from(v & 0xffff).ok()?;
    Some(f32::from(hi) * 65536.0 + f32::from(lo))
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
    fn new(x0: f32, slope: f32, advances: bool) -> Option<Edge> {
        let x = iceil(x0)?;
        let frac = x0 - raster_long(ifloor(x0)?)?;
        let f = trunc(x87_mul(frac, TWO31))?;
        let err = u32::try_from(f.wrapping_sub(1) & 0x7fff_ffff).ok()?;
        let istep = trunc(slope)?;
        let Some(whole) = raster_long(istep) else {
            // A short edge used once only contributes its first column.
            return (!advances).then_some(Edge { x, err, istep: 0, fstep: 0, cstep: 0 });
        };
        let fs = trunc(x87_mul(slope - whole, TWO31))?;
        // (the fraction's bits as the 32-bit add sees them: negative for a negative slope)
        let fstep = u32::try_from(fs.rem_euclid(1 << 32)).ok()?;
        let cstep = if slope < 0.0 { istep.wrapping_sub(1) } else { istep.wrapping_add(1) };
        Some(Edge { x, err, istep, fstep, cstep })
    }
    fn step(&mut self) -> Step {
        self.err = self.err.wrapping_add(self.fstep);
        if self.err & 0x8000_0000 != 0 {
            self.err &= 0x7fff_ffff;
            self.x = self.x.wrapping_add(self.cstep);
            Step::Carry
        } else {
            self.x = self.x.wrapping_add(self.istep);
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

/// one trapezoid as the walker hands it its spans: its row count, and each row's chunks (start
/// x, pixel count; none for an empty row), until the walker stops
#[derive(Debug, Default)]
pub struct Trap {
    pub rows: i64,
    /// its first row
    pub first: i64,
    /// (start x, pixel count, the f32 attribute accumulators B, G, R, A in 8.8 at its start)
    pub row: Vec<Vec<(i64, i64, [f32; 4])>>,
    /// the long edge left the target here: no row after this one
    pub stopped: bool,
}

/// a rasterized triangle
#[derive(Debug)]
pub struct Raster {
    pub walker: Walker,
    pub px: Vec<Px>,
    /// spans walked right to left
    pub reversed: bool,
    /// the upper and lower trapezoids the walker was called with
    pub traps: Vec<Trap>,
    /// the attributes' f32 change per pixel along a span (negated for spans right to left)
    pub step: [f32; 4],
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
    raster_as(v, clip, gouraud, Attrs::Rgb)
}

/// which attributes the setup interpolates: the RGB rasterizers' colours, or the ramp device's
/// index and alpha (TriSetup_DIdx: the alpha 256 times larger, a span as long as the RGB walk's
/// unless a step reaches 2^30)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attrs {
    Rgb,
    Ramp,
}

/// raster() for the given attributes
pub fn raster_as(v: [Vert; 3], clip: Clip, gouraud: bool, attrs: Attrs) -> Res<Option<Raster>> {
    let bad = || Fail::Unmodeled(crate::spec::open::D3D7_TRI_CLIP);
    let fl = |v: i64| raster_long(v).ok_or_else(bad);
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
    let width = xmax.wrapping_sub(xmin);
    if !(xmin < clip.r && xmax > clip.l && width > 0) {
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
    if ybot <= ytop {
        return Ok(None);
    }
    let whole_bottom = bo.y.fract() == 0.0 && iceil(bo.y) == trunc(bo.y);
    let slope_l = x87_div(qx, qy);
    let fdy = fl(ytop)? - t.y;
    let pl = fdy * slope_l;
    let xl0 = pl + t.x;
    let mut long = Edge::new(xl0, slope_l, true).ok_or_else(bad)?;
    let mut e1 = if ymid > ytop {
        let s1 = px_ / py_;
        Some(Edge::new(s1 * fdy + t.x, s1, ymid.saturating_sub(ytop) > 1).ok_or_else(bad)?)
    } else {
        None
    };
    let mut e2 = if ybot > ymid {
        let s2 = (bo.x - m.x) / (bo.y - m.y);
        Some(Edge::new((fl(ymid)? - m.y) * s2 + m.x, s2, ybot.saturating_sub(ymid) > 1).ok_or_else(bad)?)
    } else {
        None
    };
    // (spans walked right to left: every column the last covered one, ICEIL − 1)
    if reversed {
        for e in [Some(&mut long), e1.as_mut(), e2.as_mut()].into_iter().flatten() {
            e.x = e.x.wrapping_sub(1);
        }
    }
    // the colours: gradients from 1/det rounded toward zero
    let col = if gouraud {
        let i = x87_div(1.0, det);
        let (a, b, c, d) = (px_ * i, py_ * i, qx * i, qy * i);
        let mut dx = [0f32; 4];
        let mut dy = [0f32; 4];
        for ((((gx, gy), ct), cm), cb) in dx.iter_mut().zip(dy.iter_mut()).zip(t.c).zip(m.c).zip(bo.c) {
            let d1 = fl(i64::from(cm).wrapping_sub(i64::from(ct)))? * 256.0;
            let d2 = fl(i64::from(cb).wrapping_sub(i64::from(ct)))? * 256.0;
            *gx = b * d2 - d * d1;
            *gy = c * d1 - a * d2;
        }
        Colors { dx, dy }
    } else {
        Colors { dx: [0.0; 4], dy: [0.0; 4] }
    };
    let base = if gouraud { t.c } else { va.c };
    let (dir, side) = if reversed { (-1i64, clip.r.wrapping_sub(1)) } else { (1, clip.l) };
    let sdx = col.dx.map(|g| if reversed { -g } else { g });
    // the per-pixel steps in 8.8; a colour changing by 128 levels a pixel or more: every pixel
    // its own span
    let steep = match attrs {
        Attrs::Rgb => sdx.iter().any(|g| g.abs() >= 32768.0),
        // (|256 · dA/dx| from 2^30 in 16.16)
        Attrs::Ramp => sdx.get(3).is_some_and(|g| g.abs() >= f32::from_bits(0x4a80_0000)),
    };
    let pstep = sdx.map(|g| trunc(g).unwrap_or(0));
    let max_span = if steep { 1 } else { 0x100 };
    let walker = if inside_h && width <= max_span { Walker::NoClip } else { Walker::Clip };
    // the first row's anchor: the long edge's pixel, held to the clip rectangle
    let x_first = if reversed { long.x.min(side) } else { long.x.max(side) };
    let fdx = (fl(x_first)? - xl0) + pl;
    let mut acc = [0f32; 4];
    for (((o, gx), gy), cb) in acc.iter_mut().zip(col.dx).zip(col.dy).zip(base) {
        *o = (fdx * gx + f32::from(cb) * 256.0) + fdy * gy;
    }
    let mut step_nc = col.dy;
    add_scaled(&mut step_nc, fl(long.istep)?, col.dx);
    let mut step_c = step_nc;
    add_scaled(&mut step_c, if long.cstep > long.istep { 1.0 } else { -1.0 }, col.dx);
    let outside = |x: i64| if reversed { x > side } else { x < side };
    // (how far the long edge's pixel lies outside its side, in pixels: negative inside)
    let beyond = |x: i64| if reversed { x.wrapping_sub(side) } else { side.wrapping_sub(x) };
    let mut parked = walker == Walker::Clip && outside(long.x);
    let start16 = |a: [f32; 4]| -> [u16; 4] { a.map(|v| trunc(v).map_or(0, low16)) };
    let at = |x0: i64, i: i64| x0.wrapping_add(dir.wrapping_mul(i));
    let mut out = Vec::new();
    let mut traps: Vec<Trap> = Vec::new();
    if e1.is_some() {
        traps.push(Trap { rows: ymid.wrapping_sub(ytop), first: ytop, ..Trap::default() });
    }
    if e2.is_some() {
        traps.push(Trap { rows: ybot.wrapping_sub(ymid), first: ymid, ..Trap::default() });
    }
    let rows = (ytop..ymid).map(|y| (y, true)).chain((ymid..ybot).map(|y| (y, false)));
    for (y, upper) in rows {
        let short = if upper { e1.as_ref() } else { e2.as_ref() };
        let Some(short) = short else { continue };
        let ti = if upper || e1.is_none() { 0 } else { 1 };
        let mut chunks = Vec::new();
        let xs = short.x;
        match walker {
            Walker::NoClip => {
                let n = if reversed { long.x.wrapping_sub(xs) } else { xs.wrapping_sub(long.x) };
                let c0 = start16(acc);
                if n > 0 {
                    chunks.push((long.x, n, acc));
                }
                for i in 0..n.max(0) {
                    out.push(Px { x: at(long.x, i), y, c: along(c0, i, pstep) });
                }
            }
            Walker::Clip => {
                let ended = if reversed { long.x < clip.l && long.cstep <= 0 } else { long.x >= clip.r && long.cstep >= 0 };
                if ended {
                    if let Some(t) = traps.get_mut(ti) {
                        t.stopped = true;
                    }
                    break;
                }
                let start = if parked { side } else { long.x };
                let n = if reversed { start.wrapping_sub(xs.max(clip.l.wrapping_sub(1))) } else { xs.min(clip.r).wrapping_sub(start) };
                let mut tmp = acc;
                let mut done = 0i64;
                while done < n {
                    let len = n.wrapping_sub(done).min(max_span);
                    if done > 0 {
                        // (a further chunk of the row: from a copy, by the previous chunk's length)
                        add_scaled(&mut tmp, fl(max_span)?, sdx);
                    }
                    let c0 = start16(tmp);
                    chunks.push((at(start, done), len, tmp));
                    for i in 0..len {
                        out.push(Px { x: at(start, done.wrapping_add(i)), y, c: along(c0, i, pstep) });
                    }
                    done = done.wrapping_add(len);
                }
            }
        }
        if let Some(t) = traps.get_mut(ti) {
            t.row.push(chunks);
        }
        // the next row
        let st = long.step();
        if let Some(e) = if upper { e1.as_mut() } else { e2.as_mut() } {
            e.step();
        }
        let along_edge = if st == Step::Carry { step_c } else { step_nc };
        match (walker, parked) {
            (Walker::NoClip, _) => add(&mut acc, along_edge),
            // (parked at the side: straight down, then back onto the long edge when it returns)
            (Walker::Clip, true) => {
                add(&mut acc, col.dy);
                if !outside(long.x) {
                    add_scaled(&mut acc, fl(beyond(long.x).wrapping_neg())?, sdx);
                    parked = false;
                }
            }
            (Walker::Clip, false) => {
                add(&mut acc, along_edge);
                if outside(long.x) {
                    add_scaled(&mut acc, fl(beyond(long.x))?, sdx);
                    parked = true;
                }
            }
        }
    }
    Ok(Some(Raster { walker, px: out, reversed, traps, step: sdx, whole_bottom }))
}

/// PointDiamondCheck: whether an endpoint (fx, fy) in 16ths from its pixel's centre lies in
/// that pixel's diamond, corners by the diagonal's bias
fn diamond(fx: i64, fy: i64, diag: bool, bias: bool) -> bool {
    let d = fx.abs().wrapping_add(fy.abs());
    d < 8 || (fx == if bias { -8 } else { 8 } && fy == 0) || (fy == 8 && fx == 0) || (diag && d == 8 && if bias { fx < 0 && fy > 0 } else { fx > 0 && fy > 0 })
}

/// the pixels (x, y) of the software rasterizers' line from v[0] to v[1] (16ths of a pixel,
/// already on the 28.4 grid), in walking order (D3D7.LINE, from the clean-room notes on
/// d3dim700's LineSetup and WalkLinePattern): the major pixels from the start's to the end's,
/// each end kept by its diamond (the last one inside it by LASTPIXEL), each pixel's minor index
/// from the f32 start and a 31-bit error step
pub fn line(v: [(i64, i64); 2], lastpixel: bool) -> Res<Vec<(i64, i64)>> {
    Ok(walk_line(v, lastpixel)?.px)
}

/// a software line as LineSetup leaves it: its pixels in walking order (skipped ones included),
/// the start's distance `off` in 16ths to its first major centre and inv = 16 / |D|, which the
/// attributes step by (clean-room notes D.3)
pub struct LineWalk {
    pub px: Vec<(i64, i64)>,
    off: i64,
    inv: f32,
}

impl LineWalk {
    /// the span colour words (B, G, R, A in 8.8, low 16 bits) of each pixel from the start's
    /// bytes c0 to the end's c1 (flat: c1 = c0): MEASURED (d3d7dev fillgouraud-*-2 on RGB and MMX)
    /// start = grad · offp + c0 << 8 with offp = off / 16 (0 when negative), grad = ((c1 − c0) << 8)
    /// · inv, each pixel trunc(acc), 0 when below (an overshoot past the end's 0: MEASURED, 23
    /// pixels of fillgouraud-*-2 on both rasterizers and both DLLs), then acc += grad, all in f32
    pub fn colours(&self, c0: [u8; 4], c1: [u8; 4]) -> Res<Vec<[u16; 4]>> {
        let bad = || Fail::Bad;
        let f = |v: i64| f32_of(v).ok_or_else(bad);
        let offp = f(self.off)? * f32::from_bits(0x3d80_0000);
        let offp = if offp < 0.0 { 0.0 } else { offp };
        let mut grad = [0f32; 4];
        let mut acc = [0f32; 4];
        for (((g, a), &b0), &b1) in grad.iter_mut().zip(acc.iter_mut()).zip(&c0).zip(&c1) {
            let (b0, b1) = (i64::from(b0), i64::from(b1));
            *g = f(b1.wrapping_sub(b0).wrapping_mul(256))? * self.inv;
            *a = (*g * offp) + f(b0.wrapping_mul(256))?;
        }
        let mut out = Vec::with_capacity(self.px.len());
        for _ in &self.px {
            let mut c = [0u16; 4];
            for ((o, a), g) in c.iter_mut().zip(acc.iter_mut()).zip(grad) {
                *o = low16(trunc(*a).ok_or_else(bad)?.max(0));
                *a += g;
            }
            out.push(c);
        }
        Ok(out)
    }
}

/// d3dim700's LineSetup and WalkLinePattern (line)
pub fn walk_line(v: [(i64, i64); 2], lastpixel: bool) -> Res<LineWalk> {
    let bad = || Fail::Bad;
    let none = LineWalk { px: Vec::new(), off: 0, inv: 0.0 };
    let [(x0, y0), (x1, y1)] = v;
    let (dx, dy) = (x1.wrapping_sub(x0), y1.wrapping_sub(y0));
    if dx == 0 && dy == 0 {
        return Ok(none);
    }
    let xmajor = dx.abs() >= dy.abs();
    let diag = dx.abs() == dy.abs();
    let bias = diag && (dx > 0) == (dy > 0);
    let b = i64::from(bias);
    let pix = |(x, y): (i64, i64)| ((x.wrapping_add(7).wrapping_add(b)) >> 4, y.wrapping_add(7) >> 4);
    let ((px0, py0), (px1, py1)) = (pix(v[0]), pix(v[1]));
    let inside = |(x, y): (i64, i64), (px, py): (i64, i64)| diamond(x.wrapping_sub(px.wrapping_mul(16)), y.wrapping_sub(py.wrapping_mul(16)), diag, bias);
    let (start_in, end_in) = (inside(v[0], (px0, py0)), inside(v[1], (px1, py1)));
    // (start, end, start pixel, end pixel, delta) along the major axis, the other delta and the
    // start's minor coordinate (plus 7, in 16ths)
    let (s, e, mut c0, mut ce, d, od, m16) = if xmajor { (x0, x1, px0, px1, dx, dy, y0) } else { (y0, y1, py0, py1, dy, dx, x0) };
    let st = if d >= 0 { 1 } else { -1 };
    let c16 = |c: i64| c.wrapping_mul(16);
    if !start_in && !(if d >= 0 { s < c16(c0) } else { s > c16(c0) }) {
        c0 = c0.wrapping_add(st);
    }
    let keep_end = if end_in { lastpixel } else if d >= 0 { c16(ce) < e } else { c16(ce) > e };
    if !keep_end {
        ce = ce.wrapping_sub(st);
    }
    let count = (if d >= 0 { ce.wrapping_sub(c0) } else { c0.wrapping_sub(ce) }).wrapping_add(1);
    if count <= 0 {
        return Ok(none);
    }
    let f = |v: i64| f32_of(v).ok_or_else(bad);
    let sixteenth = f32::from_bits(0x3d80_0000);
    let off = if d >= 0 { c16(c0).wrapping_sub(s) } else { s.wrapping_sub(c16(c0)) };
    let inv = 16.0 / f(d.abs())?;
    let slope = (f(od)? * inv) * sixteenth;
    let m0 = ((f(off)? * sixteenth) * slope) + f(m16.wrapping_add(7))? * sixteenth;
    let i0 = ifloor(m0).ok_or_else(bad)?;
    let two31 = f32::from_bits(0x4f00_0000);
    let frac = i128::from(trunc((m0 - f(i0)?) * two31).ok_or_else(bad)?);
    let s31 = i128::from(trunc(slope * two31).ok_or_else(bad)?);
    let mut out = Vec::new();
    for k in 0..count {
        let minor = i128::from(i0).wrapping_add(frac.wrapping_add(i128::from(k).wrapping_mul(s31)).div_euclid(1 << 31));
        let minor = i64::try_from(minor).ok().ok_or_else(bad)?;
        let major = c0.wrapping_add(k.wrapping_mul(st));
        out.push(if xmajor { (major, minor) } else { (minor, major) });
    }
    Ok(LineWalk { px: out, off, inv })
}
