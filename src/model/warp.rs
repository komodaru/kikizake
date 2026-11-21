//! The HAL and T&L HAL devices of Direct3D 7 on Windows' runner: ddraw translates them to its
//! Direct3D 9 driver interface, which WARP (d3d10warp) serves on a machine without a GPU. Its
//! aliased lines, as the clean-room notes on ddraw 10.0.26100.7309 and d3d10warp describe them,
//! fitted to Windows' answers (spec/d3d7.txt D3D7.LINE.HW).
//!
//! WARP takes a pretransformed position as 24.8 fixed point of x + 1/2 (its pixel centres at
//! i + 1/2): exact for our sixteenths. Its per-pixel line test is single precision (Rust's f32
//! operations, rounded to nearest, no fused multiply-add).
use super::{Fail, Res};

/// a coordinate in 16ths of a pixel (D3D7's centres on whole numbers) in WARP's 256ths, its
/// centres at i + 1/2
fn fixed(v: i64) -> i128 {
    i128::from(v).wrapping_mul(16).wrapping_add(128)
}

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

/// the pixels WARP's aliased line from v[0] to v[1] (16ths of a pixel, D3D7's pixel centres on
/// whole numbers) lights in a target w × h; LASTPIXEL plays no part (neither ddraw nor WARP reads
/// it). The setup is single precision step by step: an endpoint inside its diamond moved to its
/// centre less a 256th along the major axis, the slope a divide of the deltas as they stand, the
/// deltas updated in place
pub fn line(v: [(i64, i64); 2], w: i64, h: i64) -> Res<Vec<bool>> {
    let bad = || Fail::Bad;
    let n = usize::try_from(w.checked_mul(h).ok_or_else(bad)?).ok().ok_or_else(bad)?;
    let mut cover = vec![false; n];
    // in 256ths of a pixel: X = 16 x + 128 (x + 1/2 in 16ths times 16)
    let p = |(x, y): (i64, i64)| (fixed(x), fixed(y));
    let (s0, s1) = (p(v[0]), p(v[1]));
    if s0 == s1 {
        return Ok(cover);
    }
    let xmajor = s1.0.wrapping_sub(s0.0).abs() >= s1.1.wrapping_sub(s0.1).abs();
    // the pixel centre an endpoint belongs to: ceil(X) − 1/2, in 256ths
    let centre = |a: i128| a.wrapping_add(255).div_euclid(256).wrapping_mul(256).wrapping_sub(128);
    let inside = |q: (i128, i128)| {
        let (cx, cy) = (centre(q.0), centre(q.1));
        let d = q.0.wrapping_sub(cx).abs().wrapping_add(q.1.wrapping_sub(cy).abs());
        d < 128 || (d == 128 && (q.1 > cy || (q.1 == cy && !xmajor && q.0 > cx)))
    };
    let (i0, i1) = (inside(s0), inside(s1));
    if i0 && i1 && (centre(s0.0), centre(s0.1)) == (centre(s1.0), centre(s1.1)) {
        return Ok(cover);
    }
    let f = |a: i128| ratio_f32(a, 256).ok_or_else(bad);
    let (x0, y0, x1, y1) = (f(s0.0)?, f(s0.1)?, f(s1.0)?, f(s1.1)?);
    let (mut dx, mut dy) = (x1 - x0, y1 - y0);
    let step = |d: f32| if d > 0.0 { f32::from_bits(0x3b80_0000) } else { -f32::from_bits(0x3b80_0000) };
    let (mut sx, mut sy) = (x0, y0);
    if i0 {
        let (cx, cy) = (f(centre(s0.0))?, f(centre(s0.1))?);
        let (mx, my) = if xmajor {
            let mx = (x0 - cx) + step(dx);
            (mx, (dy / dx) * mx)
        } else {
            let my = (y0 - cy) + step(dy);
            ((dx / dy) * my, my)
        };
        sx = x0 - mx;
        sy = y0 - my;
        dx += mx;
        dy += my;
    }
    if i1 {
        let (cx, cy) = (f(centre(s1.0))?, f(centre(s1.1))?);
        let (e, g) = if xmajor {
            let e = ((dx + sx) - cx) + step(dx);
            (e, (dy / dx) * e)
        } else {
            let g = ((dy + sy) - cy) + step(dy);
            ((dx / dy) * g, g)
        };
        dx -= e;
        dy -= g;
    }
    let idx = (dx != 0.0).then(|| -1.0 / dx);
    let idy = (dy != 0.0).then(|| -1.0 / dy);
    // the bounds: 1/2 strict, or the next float above for v always and for u on a y-major line
    let hi = f32::from_bits(0x3f00_0001);
    let uhi = if xmajor { 0.5 } else { hi };
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (f32::from(i16::try_from(i).ok().ok_or_else(bad)?) + 0.5, f32::from(i16::try_from(j).ok().ok_or_else(bad)?) + 0.5);
            let (ax, ay) = (sx - px, sy - py);
            let mut lit = false;
            if let Some(idx) = idx {
                let tx = ax * idx;
                if (0.0..=1.0).contains(&tx) {
                    let v = tx * dy + ay;
                    lit |= -0.5 < v && v < hi;
                }
            }
            if let Some(idy) = idy {
                let ty = ay * idy;
                if (0.0..=1.0).contains(&ty) {
                    let u = ty * dx + ax;
                    lit |= -0.5 < u && u < uhi;
                }
            }
            if lit && let Some(c) = usize::try_from(j.wrapping_mul(w).wrapping_add(i)).ok().and_then(|k| cover.get_mut(k)) {
                *c = true;
            }
        }
    }
    Ok(cover)
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
        let [p0, p1, p2] = v.map(|(x, y)| (fixed(x), fixed(y)));
        let sub = i128::wrapping_sub;
        let det = sub(sub(p1.0, p0.0).wrapping_mul(sub(p2.1, p0.1)), sub(p2.0, p0.0).wrapping_mul(sub(p1.1, p0.1)));
        if det == 0 {
            return Ok(None);
        }
        let px = |n: i128| ratio_f32(n, 256).ok_or_else(bad);
        let (x0, y0) = (px(p0.0)?, px(p0.1)?);
        let (e1x, e2x) = (px(sub(p1.0, p0.0))?, px(sub(p0.0, p2.0))?);
        let (e1y, e2y) = (px(sub(p0.1, p1.1))?, px(sub(p2.1, p0.1))?);
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
        let c = |i: i64| ratio_f32(i128::from(i).wrapping_mul(2).wrapping_add(1), 2).ok_or_else(bad);
        Ok((c(y)? * self.ddy + self.c) + c(x)? * self.ddx)
    }
}

/// a line's own attribute plane (SetupLineAttr1, clean-room notes D.4): MEASURED (d3d7dev
/// fillgouraud-7-hal-2, -tnl-2) along the major axis only, from the two ends' values a (bytes,
/// read as a / 255): g = (a1 − a0) · (1 / Δmajor), c = a0 − g · X0 (x-major; Y0 y-major), X0 the
/// start in WARP's coordinates
#[derive(Clone, Copy, Debug)]
pub struct LinePlane {
    g: f32,
    c: f32,
    xmajor: bool,
}

impl LinePlane {
    pub fn new(v: [(i64, i64); 2], a: [u8; 2]) -> Res<LinePlane> {
        let bad = || Fail::Bad;
        let px = |n: i128| ratio_f32(n, 256).ok_or_else(bad);
        let [(x0, y0), (x1, y1)] = v.map(|(x, y)| (fixed(x), fixed(y)));
        let xmajor = x1.wrapping_sub(x0).abs() >= y1.wrapping_sub(y0).abs();
        let (s0, s1) = if xmajor { (px(x0)?, px(x1)?) } else { (px(y0)?, px(y1)?) };
        let d = s1 - s0;
        if d == 0.0 {
            return Err(bad());
        }
        let inv = 1.0 / d;
        let [Some(a0), Some(a1)] = a.map(|b| ratio_f32(i128::from(b), 255)) else { return Err(bad()) };
        let g = (a1 - a0) * inv;
        Ok(LinePlane { g, c: a0 - g * s0, xmajor })
    }

    /// the value at the centre of pixel (x, y)
    pub fn at(self, x: i64, y: i64) -> Res<f32> {
        let bad = || Fail::Bad;
        let c = |i: i64| ratio_f32(i128::from(i).wrapping_mul(2).wrapping_add(1), 2).ok_or_else(bad);
        Ok(if self.xmajor { self.c + c(x)? * self.g } else { c(y)? * self.g + self.c })
    }
}

/// a float written to an n-bit UNORM channel: RNE(clamp(v, 0, 1) · (2^n − 1)), denormals as 0
pub fn unorm(v: f32, bits: u32) -> Res<u32> {
    let bad = || Fail::Bad;
    if !(1..=16).contains(&bits) {
        return Err(bad());
    }
    let max = (1u32 << bits).wrapping_sub(1);
    let v = if v.is_subnormal() || v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    let s = v * ratio_f32(i128::from(max), 1).ok_or_else(bad)?;
    let q = crate::model::d3dim::trunc(s.round_ties_even()).ok_or_else(bad)?;
    u32::try_from(q).ok().ok_or_else(bad)
}
