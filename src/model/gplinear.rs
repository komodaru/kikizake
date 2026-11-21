//! GDI+ LinearGradientBrush made from two points (GdipCreateLineBrushI): its rectangle and
//! transform, and how it fills: a 32-entry premultiplied color table stepped by a 16.16 DDA.
//!
//! Sources (Microsoft Learn): "LinearGradientBrush::LinearGradientBrush", "WrapMode",
//! "LinearGradientBrush::GetRectangle", "LinearGradientBrush::GetTransform"; the arithmetic
//! MEASURED against Windows' answers (golden gdiplusbrush lin*-*) as clean-room notes on GDI+'s
//! own code predicted it, each float op rounded as the executor rounds (GP.MATRIX.X86, .X64).

use super::gdiplus::{Matrix, Round};
use super::gppath::{add64, div64, mul64, to32};
use crate::spec::{clause, hit};

/// why a line brush is not made
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    /// WrapModeClamp (GdipCreateLineBrushI's own check), or no transform
    InvalidParameter,
    /// the two points the same
    OutOfMemory,
}

/// WrapMode as a line gradient uses it: TileFlipX and TileFlipXY mirror every other period,
/// the rest (TileFlipY too, and values past the enum) tile
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repeat {
    Tile,
    Mirror,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Linear {
    pub rect: (f32, f32, f32, f32),
    pub matrix: Matrix,
    pub colors: (u32, u32),
    pub repeat: Repeat,
}

/// GpModF: v modulo m by a truncated quotient, a negative v folded up
fn modf(r: Round, v: f32, m: f32) -> f32 {
    let turns = |v: f32| r.sub(v, r.mul((r.div(v, m) as i32) as f32, m));
    match v.partial_cmp(&0.0) {
        Some(std::cmp::Ordering::Greater) => turns(v),
        Some(std::cmp::Ordering::Less) => {
            let b = turns(-v);
            if b > 0.0 { r.sub(m, b) } else { b }
        }
        Some(std::cmp::Ordering::Equal) | None => 0.0,
    }
}

/// x86-64's sin, under MXCSR's rounding down, just above 1 for an angle just under pi / 2
/// (MEASURED, golden gdiplusbrush-64 lininfo-*: vertical lines)
fn sin(r: Round, th: f64) -> f64 {
    let s = th.sin();
    match r {
        Round::Down if (s - 1.0).abs() < 1e-11 => {
            hit(clause::GP_BRUSH_LINEAR_SIN);
            1.0 + f64::EPSILON
        }
        Round::Down | Round::Nearest => s,
    }
}

/// InferAffineMatrix: the matrix taking the rectangle's corners (x, y), (x + w, y), (x, y + h)
/// to p0, p1, p2; None when the determinant is under FLT_EPSILON
pub fn infer(r: Round, p: [(f32, f32); 3], (x, y, w, h): (f32, f32, f32, f32)) -> Option<Matrix> {
    let det = r.add(r.sub(r.mul(r.add(w, x), r.add(y, h)), r.mul(y, x)), r.sub(r.mul(-h, x), r.mul(y, w)));
    if 1.192_092_9e-7 > det.abs() {
        return None;
    }
    let q = r.div(1.0, det);
    let a = r.sub(r.mul(r.add(x, w), r.add(y, h)), r.mul(x, y));
    let pick = |pt: (f32, f32), k: usize| if k == 0 { pt.0 } else { pt.1 };
    let lin = |s: f32, to: (f32, f32), k: usize| r.mul(r.add(r.mul(s, pick(to, k)), r.mul(-s, pick(p[0], k))), q);
    let tr = |k: usize| r.mul(r.add(r.add(r.mul(r.mul(-x, h), pick(p[1], k)), r.mul(a, pick(p[0], k))), r.mul(r.mul(-y, w), pick(p[2], k))), q);
    Some([lin(h, p[1], 0), lin(h, p[1], 1), lin(w, p[2], 0), lin(w, p[2], 1), tr(0), tr(1)])
}

/// GdipCreateLineBrushI
pub fn new(r: Round, p1: (i32, i32), p2: (i32, i32), colors: (u32, u32), wrap: i64) -> Result<Linear, Refused> {
    hit(clause::GP_BRUSH_LINEAR);
    if wrap == 4 {
        return Err(Refused::InvalidParameter);
    }
    if p1 == p2 {
        return Err(Refused::OutOfMemory);
    }
    let repeat = match wrap {
        1 | 3 => Repeat::Mirror,
        _ => Repeat::Tile,
    };
    let (p1, p2) = ((p1.0 as f32, p1.1 as f32), (p2.0 as f32, p2.1 as f32));
    // (the rectangle: a vertical or horizontal line made square about it, the test relative to
    // p2's coordinate)
    let mut x = if p2.0 < p1.0 { p2.0 } else { p1.0 };
    let mut y = if p2.1 < p1.1 { p2.1 } else { p1.1 };
    let mut w = r.sub(p1.0, p2.0).abs();
    let mut h = r.sub(p1.1, p2.1).abs();
    const NEAR: f32 = 1.192_092_9e-6;
    const _: () = assert!(NEAR.to_bits() == 0x35a0_0000);
    let den = if p2.0 != 0.0 { p2.0 } else { 1.0 };
    if NEAR > r.div(r.sub(p1.0, p2.0), den).abs() {
        w = h;
        x = r.sub(x, r.mul(h, 0.5));
    }
    let den = if p2.1 != 0.0 { p2.1 } else { 1.0 };
    if NEAR > r.div(r.sub(p1.1, p2.1), den).abs() {
        h = w;
        y = r.sub(y, r.mul(w, 0.5));
    }
    let rect = (x, y, w, h);
    let pi = std::f64::consts::PI;
    let (dx, dy) = (r.sub(p2.0, p1.0), r.sub(p2.1, p1.1));
    let angle = to32(r, div64(r, mul64(r, (dy as f64).atan2(dx as f64), 180.0), pi));
    let a = modf(r, angle, 360.0);
    let (q, k) = if 90.0 > a {
        (0, a)
    } else if 180.0 > a {
        (1, r.sub(180.0, a))
    } else if 270.0 > a {
        (2, r.sub(a, 180.0))
    } else {
        (3, r.sub(360.0, a))
    };
    let th = div64(r, mul64(r, k as f64, pi), 180.0);
    let (s, c) = (sin(r, th), th.cos());
    let (xd, yd, wd, hd) = (x as f64, y as f64, w as f64, h as f64);
    let (kk, sp, cp, x0, y0) = match q {
        0 => (mul64(r, wd, s), s, c, xd, yd),
        1 => (mul64(r, hd, c), s, -c, add64(r, wd, xd), yd),
        2 => (mul64(r, wd, s), -s, -c, add64(r, wd, xd), add64(r, hd, yd)),
        _ => (mul64(r, hd, c), -s, c, xd, add64(r, hd, yd)),
    };
    let l1 = add64(r, mul64(r, hd, c), mul64(r, wd, s));
    let l2 = add64(r, mul64(r, wd, c), mul64(r, hd, s));
    let p0 = (to32(r, add64(r, mul64(r, sp, kk), x0)), to32(r, add64(r, y0, -mul64(r, cp, kk))));
    let (a1, b1) = (to32(r, mul64(r, cp, l2)), to32(r, mul64(r, sp, l2)));
    let (a2, b2) = (to32(r, mul64(r, -l1, sp)), to32(r, mul64(r, cp, l1)));
    let pa = (r.add(a1, p0.0), r.add(b1, p0.1));
    let pb = (r.add(a2, p0.0), r.add(b2, p0.1));
    let matrix = infer(r, [p0, pa, pb], rect).ok_or(Refused::InvalidParameter)?;
    Ok(Linear { rect, matrix, colors, repeat })
}

/// GpMatrix::MultiplyMatrix (points as row vectors: p a b)
fn multiply(r: Round, a: &Matrix, b: &Matrix) -> Matrix {
    [
        r.add(r.mul(b[0], a[0]), r.mul(b[2], a[1])),
        r.add(r.mul(a[0], b[1]), r.mul(a[1], b[3])),
        r.add(r.mul(b[2], a[3]), r.mul(b[0], a[2])),
        r.add(r.mul(b[1], a[2]), r.mul(a[3], b[3])),
        r.add(r.add(r.mul(a[4], b[0]), r.mul(a[5], b[2])), b[4]),
        r.add(r.add(r.mul(a[4], b[1]), r.mul(a[5], b[3])), b[5]),
    ]
}
/// GpMatrix::Invert: an identity as it is
pub fn invert(r: Round, m: &Matrix) -> Option<Matrix> {
    if *m == [1.0, 0.0, 0.0, 1.0, 0.0, 0.0] {
        return Some(*m);
    }
    super::gdiplus::inverse(r, m)
}
/// RoundSat: floor(v + 1/2) saturated to int32
fn round_sat(r: Round, v: f32) -> i32 {
    if v < -2147483648.0 {
        i32::MIN
    } else if v >= 2147483648.0 {
        i32::MAX
    } else {
        r.add(v, 0.5).floor() as i32
    }
}

/// how the brush colors a device pixel: its DDA and color tables
pub struct Span {
    du: i32,
    dv: i32,
    u0: i32,
    a: [[u32; 4]; 32],
    b: [[u32; 4]; 32],
}
impl Span {
    /// None when the matrix has no inverse (nothing drawn)
    pub fn new(r: Round, l: &Linear, half_offset: bool) -> Option<Span> {
        hit(clause::GP_BRUSH_LINEAR_PAINT);
        let bw = if half_offset { multiply(r, &l.matrix, &[1.0, 0.0, 0.0, 1.0, -0.5, -0.5]) } else { l.matrix };
        let t = r.mul((32u32 << 16) as f32, 0.5);
        let (x, y, w, h) = l.rect;
        let (right, bottom) = (r.add(x, w), r.add(y, h));
        let (m11, m22) = (r.div(r.sub(right, x), r.sub(t, 0.0)), r.div(r.sub(bottom, y), r.sub(t, 0.0)));
        let m0 = [m11, 0.0, 0.0, m22, r.sub(right, r.mul(m11, t)), r.sub(bottom, r.mul(m22, t))];
        let mi = invert(r, &multiply(r, &m0, &bw))?;
        let du = r.add(mi[0], 0.5).floor() as i32;
        let dv = r.add(mi[2], 0.5).floor() as i32;
        let u0 = round_sat(r, mi[4]);
        // (each color as floats, premultiplied by a / 255 in float, 0 for alpha 0)
        let lin = |c: u32| -> [f32; 4] {
            let a = (c >> 24) as f32;
            let ch = |k: u32| (c >> (8 * k) & 255) as f32;
            if a == 0.0 {
                return [0.0, 0.0, 0.0, 0.0];
            }
            let k = if a != 255.0 { r.div(a, 255.0) } else { 1.0 };
            let f = |v: f32| if a != 255.0 { r.mul(k, v) } else { v };
            [f(ch(0)), f(ch(1)), f(ch(2)), a]
        };
        let (l1, l2) = (lin(l.colors.0), lin(l.colors.1));
        let step = r.div(1.0, 16.0);
        let mut tf = 0.0f32;
        let mut entry = [[0u32; 4]; 17];
        for e in &mut entry {
            for k in 0..4 {
                let v = r.add(r.mul(l1[k], r.sub(1.0, tf)), r.mul(l2[k], tf));
                e[k] = r.add(v, 0.5).floor() as u32;
            }
            tf = r.add(tf, step);
        }
        let (mut a, mut b) = ([[0u32; 4]; 32], [[0u32; 4]; 32]);
        a[..16].copy_from_slice(&entry[..16]);
        b[..16].copy_from_slice(&entry[1..17]);
        for i in 0..16 {
            match l.repeat {
                Repeat::Mirror => (a[16 + i], b[16 + i]) = (b[15 - i], a[15 - i]),
                Repeat::Tile => (a[16 + i], b[16 + i]) = (a[i], b[i]),
            }
        }
        Some(Span { du, dv, u0, a, b })
    }
    /// the premultiplied ARGB of device pixel (x, y)
    pub fn at(&self, x: i64, y: i64) -> u32 {
        let u = (x as i32).wrapping_mul(self.du).wrapping_add((y as i32).wrapping_mul(self.dv)).wrapping_add(self.u0);
        let idx = ((u >> 16) & 31) as usize;
        let w = ((u >> 8) & 255) as u32;
        (0..4).fold(0, |o, k| o | (((256 - w) * self.a[idx][k] + w * self.b[idx][k] + 128) >> 8) << (8 * k))
    }
}
