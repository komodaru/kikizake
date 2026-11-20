//! The HAL and T&L HAL devices of Direct3D 7 on Windows' runner: ddraw translates them to its
//! Direct3D 9 driver interface, which WARP (d3d10warp) serves on a machine without a GPU. Its
//! aliased lines, as the clean-room notes on ddraw 10.0.26100.7309 and d3d10warp describe them,
//! fitted to Windows' answers (spec/d3d7.txt D3D7.LINE.HW).
//!
//! WARP takes a pretransformed position as 24.8 fixed point of x + 1/2 (its pixel centres at
//! i + 1/2): exact for our sixteenths. Its per-pixel line test is single precision (Rust's f32
//! operations, rounded to nearest, no fused multiply-add).
use super::{Fail, Res};

/// a rational n / d (d > 0) rounded to the nearest f32, ties to even; None past f32's normal range
fn ratio_f32(n: i128, d: i128) -> Option<f32> {
    if d <= 0 {
        return None;
    }
    if n == 0 {
        return Some(0.0);
    }
    let (a, b) = (n.unsigned_abs(), d.unsigned_abs());
    // a / b = q · 2^e with q of 24 bits: shift the larger side so the quotient has 24 bits
    let mut e = i32::try_from(a.ilog2()).ok()?.wrapping_sub(i32::try_from(b.ilog2()).ok()?).wrapping_sub(23);
    let parts = |e: i32| -> Option<(u128, u128, u128)> {
        let (num, den) = if e >= 0 { (a, b.checked_shl(u32::try_from(e).ok()?)?) } else { (a.checked_shl(u32::try_from(e.wrapping_neg()).ok()?)?, b) };
        Some((num.checked_div(den)?, num.checked_rem(den)?, den))
    };
    let (mut q, mut r, mut den) = parts(e)?;
    if q < 1 << 23 {
        e = e.wrapping_sub(1);
        (q, r, den) = parts(e)?;
    }
    let r2 = r.checked_mul(2)?;
    if r2 > den || (r2 == den && q & 1 == 1) {
        q = q.wrapping_add(1);
    }
    if q == 1 << 24 {
        q >>= 1;
        e = e.wrapping_add(1);
    }
    let exp = u32::try_from(e.wrapping_add(150)).ok().filter(|x| (1..=254).contains(x))?;
    let man = u32::try_from(q & 0x7f_ffff).ok()?;
    Some(f32::from_bits(u32::from(n < 0) << 31 | exp << 23 | man))
}

/// a point in WARP's coordinates as a rational over a common denominator: (x, y) · 1/den
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Pt {
    x: i128,
    y: i128,
}

/// the pixels WARP's aliased line from v[0] to v[1] (16ths of a pixel, D3D7's pixel centres on
/// whole numbers) lights in a target w × h; LASTPIXEL plays no part (neither ddraw nor WARP reads
/// it). None: the f32 test of some pixel lands exactly on a bound (D3D7.LINE.HW's residual)
pub fn line(v: [(i64, i64); 2], w: i64, h: i64) -> Res<Option<Vec<bool>>> {
    let bad = || Fail::Bad;
    let n = usize::try_from(w.checked_mul(h).ok_or_else(bad)?).map_err(|_| bad())?;
    let mut cover = vec![false; n];
    // in 256ths of a pixel: X = 16 x + 128 (x + 1/2 in 16ths times 16)
    let p = |(x, y): (i64, i64)| Pt { x: i128::from(x) * 16 + 128, y: i128::from(y) * 16 + 128 };
    let (s0, s1) = (p(v[0]), p(v[1]));
    if s0 == s1 {
        return Ok(Some(cover));
    }
    let (dx, dy) = (s1.x - s0.x, s1.y - s0.y);
    let xmajor = dx.abs() >= dy.abs();
    // the pixel centre an endpoint belongs to: ceil(X) − 1/2, in 256ths
    let centre = |a: i128| (a + 255).div_euclid(256) * 256 - 128;
    let inside = |q: Pt| {
        let (cx, cy) = (centre(q.x), centre(q.y));
        let d = (q.x - cx).abs() + (q.y - cy).abs();
        d < 128 || (d == 128 && (q.y > cy || (q.y == cy && !xmajor && q.x > cx)))
    };
    let (i0, i1) = (inside(s0), inside(s1));
    if i0 && i1 && (centre(s0.x), centre(s0.y)) == (centre(s1.x), centre(s1.y)) {
        return Ok(Some(cover));
    }
    let sign = if (if xmajor { dx } else { dy }) > 0 { 1 } else { -1 };
    // an endpoint inside its diamond moved back along the line to its centre's major coordinate
    // less a 256th: (major, minor numerator over the line's major delta)
    let dmaj = if xmajor { dx } else { dy };
    let moved = |q: Pt| -> (i128, i128) {
        let qa = if xmajor { q.x } else { q.y };
        let (a0, b0, db) = if xmajor { (s0.x, s0.y, dy) } else { (s0.y, s0.x, dx) };
        let tgt = centre(qa) - sign;
        // minor = b0 + (tgt − a0) · db / dmaj
        (tgt * dmaj, b0 * dmaj + (tgt - a0) * db)
    };
    let plain = |q: Pt| -> (i128, i128) {
        let (qa, qb) = if xmajor { (q.x, q.y) } else { (q.y, q.x) };
        (qa * dmaj, qb * dmaj)
    };
    let (sa, sb) = if i0 { moved(s0) } else { plain(s0) };
    let (ea, eb) = if i1 { moved(s1) } else { plain(s1) };
    // in pixels: value / (256 · dmaj), dmaj made positive for the rounding
    let den = 256 * dmaj.abs();
    let fix = |v: i128| if dmaj < 0 { -v } else { v };
    let f = |v: i128| ratio_f32(fix(v), den).ok_or_else(bad);
    let (sx, sy, ddx, ddy) = if xmajor { (f(sa)?, f(sb)?, f(ea - sa)?, f(eb - sb)?) } else { (f(sb)?, f(sa)?, f(eb - sb)?, f(ea - sa)?) };
    let idx = (ddx != 0.0).then(|| -1.0 / ddx);
    let idy = (ddy != 0.0).then(|| -1.0 / ddy);
    let hi = f32::from_bits(0x3f00_0002);
    let mut tie = false;
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (f32::from(i16::try_from(i).map_err(|_| bad())?) + 0.5, f32::from(i16::try_from(j).map_err(|_| bad())?) + 0.5);
            let mut lit = false;
            if let Some(idx) = idx {
                let tx = (sx - px) * idx;
                if (0.0..=1.0).contains(&tx) {
                    let v = tx * ddy + (sy - py);
                    tie |= v == 0.5;
                    lit |= -0.5 < v && v < hi;
                }
            }
            if let Some(idy) = idy {
                let ty = (sy - py) * idy;
                if (0.0..=1.0).contains(&ty) {
                    let u = ty * ddx + (sx - px);
                    tie |= u == 0.5 && !xmajor;
                    lit |= -0.5 < u && u < if xmajor { 0.5 } else { hi };
                }
            }
            if lit && let Some(c) = usize::try_from(j * w + i).ok().and_then(|k| cover.get_mut(k)) {
                *c = true;
            }
        }
    }
    // (a pixel lit by a test landing exactly on +1/2, under the 0.5000001 bound: 21 of 22 such
    // measured lines lit it, d3d7dev linecorner-7-hal-1 one not)
    Ok(if tie { None } else { Some(cover) })
}

/// WARP's plane of one colour channel over a triangle (D3D7.GOURAUD, the HAL): set up in single
/// precision from vertex 0 as ddraw hands the triangle on
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    ddx: f32,
    ddy: f32,
    c: f32,
}

impl Plane {
    /// the plane through the channel values a (bytes, read as a / 255) at v (16ths of a pixel,
    /// D3D7's pixel centres on whole numbers); None for a triangle of no area
    pub fn new(v: [(i64, i64); 3], a: [u8; 3]) -> Res<Option<Plane>> {
        let bad = || Fail::Bad;
        // in 256ths of a pixel, WARP's centres at i + 1/2
        let p = v.map(|(x, y)| (i128::from(x) * 16 + 128, i128::from(y) * 16 + 128));
        let det = (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[2].0 - p[0].0) * (p[1].1 - p[0].1);
        if det == 0 {
            return Ok(None);
        }
        let px = |n: i128| ratio_f32(n, 256).ok_or_else(bad);
        let (x0, y0) = (px(p[0].0)?, px(p[0].1)?);
        let (e1x, e2x) = (px(p[1].0 - p[0].0)?, px(p[0].0 - p[2].0)?);
        let (e1y, e2y) = (px(p[0].1 - p[1].1)?, px(p[2].1 - p[0].1)?);
        // 1 / f32(f32(det) · 2^-16): the fixed determinant in pixels squared
        let inv = 1.0 / (ratio_f32(det, 1).ok_or_else(bad)? * f32::from_bits(0x3780_0000));
        let a = a.map(|b| ratio_f32(i128::from(b), 255));
        let [Some(a0), Some(a1), Some(a2)] = a else { return Err(bad()) };
        let (d1, d2) = (a1 - a0, a2 - a0);
        let ddx = (d1 * e2y + d2 * e1y) * inv;
        let ddy = (d1 * e2x + d2 * e1x) * inv;
        Ok(Some(Plane { ddx, ddy, c: (a0 - ddx * x0) - ddy * y0 }))
    }

    /// the channel at the centre of pixel (x, y)
    pub fn at(self, x: i64, y: i64) -> Res<f32> {
        let bad = || Fail::Bad;
        let c = |i: i64| -> Res<f32> { Ok(ratio_f32(i128::from(i) * 2 + 1, 2).ok_or_else(bad)?) };
        Ok((c(y)? * self.ddy + self.c) + c(x)? * self.ddx)
    }
}

/// a float written to an n-bit UNORM channel: RNE(clamp(v, 0, 1) · (2^n − 1)), denormals as 0
pub fn unorm(v: f32, bits: u32) -> Res<u32> {
    let bad = || Fail::Bad;
    if !(1..=16).contains(&bits) {
        return Err(bad());
    }
    let max = (1u32 << bits) - 1;
    let v = if v.is_subnormal() || v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    let s = v * ratio_f32(i128::from(max), 1).ok_or_else(bad)?;
    let q = crate::model::d3dim::trunc(s.round_ties_even()).ok_or_else(bad)?;
    u32::try_from(q).map_err(|_| bad())
}
