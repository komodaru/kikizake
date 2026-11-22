//! How Direct3D 10 and 11 turn floats into each format's bits (a clear, a render target write)
//! and each format's bits into what a shader reads (ld, sample). Microsoft Learn: "Data
//! Conversion Rules" (FLOAT to UNORM / SNORM / SRGB, the 16, 11 and 10 bit floats, integer
//! conversions), "DXGI_FORMAT" (R9G9B9E5, R11G11B10), "Format Conversion using Direct3D 10.1".
use crate::spec::{clause, open};
use super::dxgifmt::{Format, Num};
use super::{Fail, Res};
use crate::spec::hit;

/// round half to even of a finite non-negative value
fn rne(x: f64) -> u64 {
    let f = x.floor();
    let d = x - f;
    let f = f as u64;
    if d > 0.5 || d == 0.5 && f % 2 == 1 {
        f + 1
    } else {
        f
    }
}

pub fn to_unorm(v: f32, n: u32) -> u32 {
    hit(clause::CONV_UNORM);
    if v.is_nan() {
        return 0;
    }
    // (MEASURED: the product in single precision, then to nearest even; clear-11-*)
    let max = ((1u64 << n) - 1) as f32;
    rne((v.clamp(0.0, 1.0) * max) as f64) as u32
}

pub fn to_snorm(v: f32, n: u32) -> u32 {
    hit(clause::CONV_SNORM);
    if v.is_nan() {
        return 0;
    }
    let max = ((1u64 << (n - 1)) - 1) as f32;
    let x = (v.clamp(-1.0, 1.0) * max) as f64;
    // (MEASURED: halves away from zero, unlike UNORM; clear-13-*, clear-31-*)
    let m = (x.abs() + 0.5).floor() as i64;
    let i = if x < 0.0 { -m } else { m };
    (i as u32) & ((1u64 << n) - 1) as u32
}

/// a 12-bit UNORM code, as WARP quantizes a linear value before its sRGB table
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Unorm12(u16);

impl Unorm12 {
    pub const MAX: u16 = 4095;
    pub fn of(v: f32) -> Unorm12 {
        // (to_unorm clamps to the code range: 12 bits always fit)
        Unorm12(u16::try_from(to_unorm(v, 12)).unwrap_or(Self::MAX))
    }
    pub fn get(self) -> u16 {
        self.0
    }
}

/// FLOAT to an 8-bit sRGB code (MEASURED, WARP, clear-srgb-sweep-* (12240 values, every 1/48
/// of a step) and clear-srgb-curve: the value first to 12-bit UNORM, then the exact curve of that
/// code, rounded: so a step lands up to 0.4 of a code off the exact curve near black)
pub fn srgb8(v: f32) -> u8 {
    let q = f64::from(Unorm12::of(v).get()) / f64::from(Unorm12::MAX);
    let c = if q <= 0.003_130_8 { q * 12.92 } else { 1.055 * q.powf(1.0 / 2.4) - 0.055 };
    // (no code of the 4096 lands within 4e-4 of a half step: the rounding is not in doubt)
    let k = (c * 255.0 + 0.5).floor().clamp(0.0, 255.0);
    u8::try_from(k as i64).unwrap_or(u8::MAX)
}

/// an 8-bit sRGB code as a shader reads it (MEASURED, WARP, d3d11draw ld-table-29 and
/// ld-table-91: every code; up to 6 ulps off the exact curve, so WARP's own table)
pub const SRGB_READ: [u32; 256] = [
    0x00000000, 0x399f22b4, 0x3a1f22b4, 0x3a6eb40f, 0x3a9f22b4, 0x3ac6eb61, 0x3aeeb40f, 0x3b0b3e5e,
    0x3b1f22b4, 0x3b33070b, 0x3b46eb61, 0x3b5b518d, 0x3b70f18d, 0x3b83e1c6, 0x3b8fe616, 0x3b9c87fd,
    0x3ba9c9b7, 0x3bb7ad6f, 0x3bc63549, 0x3bd56361, 0x3be539c1, 0x3bf5ba70, 0x3c0373b5, 0x3c0c6152,
    0x3c15a706, 0x3c1f45be, 0x3c293e6b, 0x3c3391f7, 0x3c3e414b, 0x3c494d46, 0x3c54b6ca, 0x3c607eb4,
    0x3c6ca5df, 0x3c792d22, 0x3c830aa8, 0x3c89afa0, 0x3c9085dd, 0x3c978dc7, 0x3c9ec7c2, 0x3ca63433,
    0x3cadd37d, 0x3cb5a601, 0x3cbdac22, 0x3cc5e63b, 0x3cce54ad, 0x3cd6f7d5, 0x3cdfd010, 0x3ce8ddb9,
    0x3cf2212e, 0x3cfb9ac6, 0x3d02a56c, 0x3d0798df, 0x3d0ca7e7, 0x3d11d2b0, 0x3d171965, 0x3d1c7c31,
    0x3d21fb3c, 0x3d2796b2, 0x3d2d4ebb, 0x3d332380, 0x3d39152e, 0x3d3f23e6, 0x3d454fd4, 0x3d4b991f,
    0x3d51ffef, 0x3d58846a, 0x3d5f26b7, 0x3d65e6fe, 0x3d6cc564, 0x3d73c20f, 0x3d7add25, 0x3d810b66,
    0x3d84b793, 0x3d887330, 0x3d8c3e4a, 0x3d9018f6, 0x3d940345, 0x3d97fd4a, 0x3d9c0716, 0x3da020bb,
    0x3da44a4b, 0x3da883d7, 0x3daccd70, 0x3db12728, 0x3db59110, 0x3dba0b38, 0x3dbe95b3, 0x3dc33092,
    0x3dc7dbe2, 0x3dcc97b6, 0x3dd1641f, 0x3dd6412c, 0x3ddb2eef, 0x3de02d77, 0x3de53cd4, 0x3dea5d19,
    0x3def8e56, 0x3df4d093, 0x3dfa23e8, 0x3dff8861, 0x3e027f07, 0x3e054280, 0x3e080ea5, 0x3e0ae379,
    0x3e0dc107, 0x3e10a755, 0x3e13966c, 0x3e168e53, 0x3e198f11, 0x3e1c98ae, 0x3e1fab32, 0x3e22c6a3,
    0x3e25eb09, 0x3e29186c, 0x3e2c4ed2, 0x3e2f8e45, 0x3e32d6c8, 0x3e362865, 0x3e398322, 0x3e3ce706,
    0x3e405419, 0x3e43ca62, 0x3e4749e8, 0x3e4ad2b1, 0x3e4e64c6, 0x3e52002b, 0x3e55a4e9, 0x3e595307,
    0x3e5d0a8b, 0x3e60cb7c, 0x3e6495e0, 0x3e6869bf, 0x3e6c4720, 0x3e702e08, 0x3e741e7f, 0x3e78188c,
    0x3e7c1c38, 0x3e8014c2, 0x3e82203c, 0x3e84308d, 0x3e8645ba, 0x3e885fc5, 0x3e8a7eb2, 0x3e8ca283,
    0x3e8ecb3d, 0x3e90f8e1, 0x3e932b74, 0x3e9562f8, 0x3e979f71, 0x3e99e0e2, 0x3e9c274e, 0x3e9e72b7,
    0x3ea0c322, 0x3ea31892, 0x3ea57308, 0x3ea7d289, 0x3eaa3718, 0x3eaca0b7, 0x3eaf0f69, 0x3eb18333,
    0x3eb3fc16, 0x3eb67a15, 0x3eb8fd34, 0x3ebb8576, 0x3ebe12e1, 0x3ec0a571, 0x3ec33d2d, 0x3ec5da17,
    0x3ec87c33, 0x3ecb2383, 0x3ecdd00b, 0x3ed081cd, 0x3ed338cc, 0x3ed5f50b, 0x3ed8b68d, 0x3edb7d54,
    0x3ede4965, 0x3ee11ac1, 0x3ee3f16b, 0x3ee6cd67, 0x3ee9aeb6, 0x3eec955d, 0x3eef815d, 0x3ef272ba,
    0x3ef56976, 0x3ef86594, 0x3efb6717, 0x3efe6e02, 0x3f00bd2b, 0x3f02460c, 0x3f03d1a5, 0x3f055ff9,
    0x3f06f106, 0x3f0884d1, 0x3f0a1b57, 0x3f0bb49d, 0x3f0d50a2, 0x3f0eef69, 0x3f1090f2, 0x3f123540,
    0x3f13dc53, 0x3f15862d, 0x3f1732cf, 0x3f18e23b, 0x3f1a9471, 0x3f1c4973, 0x3f1e0143, 0x3f1fbbe1,
    0x3f217950, 0x3f23398f, 0x3f24fca2, 0x3f26c288, 0x3f288b43, 0x3f2a56d5, 0x3f2c253f, 0x3f2df681,
    0x3f2fca9e, 0x3f31a197, 0x3f337b6c, 0x3f355820, 0x3f3737b5, 0x3f391a28, 0x3f3aff7e, 0x3f3ce7b7,
    0x3f3ed2d4, 0x3f40c0d6, 0x3f42b1c0, 0x3f44a592, 0x3f469c4d, 0x3f4895f3, 0x3f4a9284, 0x3f4c9203,
    0x3f4e9470, 0x3f5099cd, 0x3f52a21a, 0x3f54ad59, 0x3f56bb8c, 0x3f58ccb3, 0x3f5ae0cf, 0x3f5cf7e2,
    0x3f5f11ee, 0x3f612ef2, 0x3f634ef1, 0x3f6571ec, 0x3f6797e3, 0x3f69c0d8, 0x3f6beccd, 0x3f6e1bc2,
    0x3f704dba, 0x3f7282b4, 0x3f74bab1, 0x3f76f5b3, 0x3f7933bb, 0x3f7b74cb, 0x3f7db8e3, 0x3f800000,
];

pub fn srgb_read(c: u8) -> f32 {
    hit(clause::CONV_SRGBREAD);
    f32::from_bits(SRGB_READ[usize::from(c)])
}

pub fn srgb_to_linear(c: u32) -> f32 {
    let v = c as f64 / 255.0;
    (if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }) as f32
}

/// a float with `e` exponent and `m` mantissa bits (sign bit if `signed`) (MEASURED, WARP,
/// clear-10-*, clear-26-*: rounded toward zero, so too large a value is the largest finite one;
/// denormals kept; infinities kept; a NaN keeps the top mantissa bits of a 16-bit float (1 when
/// they are 0) and is all ones in an unsigned one; a negative value in an unsigned float is 0)
pub fn to_small_float(v: f32, e: u32, m: u32, signed: bool) -> u32 {
    hit(clause::CONV_SMALLFLOAT);
    let b = v.to_bits();
    let sign = b >> 31;
    let bias = (1i32 << (e - 1)) - 1;
    let emax = (1u32 << e) - 1;
    let sbit = if signed { sign << (e + m) } else { 0 };
    if v.is_nan() {
        if !signed {
            return (1 << (e + m)) - 1;
        }
        let top = (b & 0x7f_ffff) >> (23 - m);
        return sbit | emax << m | top.max(1);
    }
    if !signed && sign == 1 {
        return 0;
    }
    if v.is_infinite() {
        return sbit | emax << m;
    }
    let a = v.abs() as f64;
    if a == 0.0 {
        return sbit;
    }
    // (exact: the f64 exponent of a value from an f32)
    let log2 = |x: f64| (x.to_bits() >> 52) as i32 - 1023;
    let ex = log2(a).max(1 - bias);
    let ulp = 2f64.powi(ex - m as i32);
    let val = (a / ulp).floor() * ulp;
    if val >= 2f64.powi(emax as i32 - bias) {
        return sbit | (emax - 1) << m | ((1 << m) - 1);
    }
    let bits = if val < 2f64.powi(1 - bias) {
        (val / 2f64.powi(1 - bias - m as i32)) as u32
    } else {
        let e2 = log2(val);
        ((e2 + bias) as u32) << m | ((val / 2f64.powi(e2 - m as i32)) as u32 - (1 << m))
    };
    sbit | bits
}

pub fn from_small_float(b: u32, e: u32, m: u32, signed: bool) -> f32 {
    let sign = if signed && b >> (e + m) & 1 == 1 { -1.0 } else { 1.0 };
    let ex = (b >> m) & ((1 << e) - 1);
    let man = b & ((1 << m) - 1);
    let bias = (1i32 << (e - 1)) - 1;
    let v = if ex == (1 << e) - 1 {
        if man != 0 {
            // (MEASURED: a NaN keeps its mantissa bits on top, ld-10, ld-26)
            hit(clause::CONV_NANREAD);
            let sb = if sign < 0.0 { 0x8000_0000 } else { 0 };
            return f32::from_bits(sb | 0x7f80_0000 | man << (23 - m));
        }
        f64::INFINITY
    } else if ex == 0 {
        man as f64 * 2f64.powi(1 - bias - m as i32)
    } else {
        (man as f64 / (1u64 << m) as f64 + 1.0) * 2f64.powi(ex as i32 - bias)
    };
    (sign * v) as f32
}

/// R9G9B9E5_SHAREDEXP from three floats (the shared exponent of the largest, mantissas rounded)
pub fn to_rgb9e5(c: [f32; 3]) -> u32 {
    hit(clause::CONV_RGB9E5);
    let max = 65408.0f64; // (2^9 - 1) / 2^9 * 2^16
    let c: Vec<f64> = c.iter().map(|v| if v.is_nan() { 0.0 } else { (*v as f64).clamp(0.0, max) }).collect();
    let mx = c.iter().cloned().fold(0.0, f64::max);
    let mut e = (if mx > 0.0 { mx.log2().floor() as i32 } else { -16 }).max(-16) + 1;
    let mut scale = 2f64.powi(e - 9);
    if rne(mx / scale) == 512 {
        e += 1;
        scale *= 2.0;
    }
    let m: Vec<u32> = c.iter().map(|v| rne(v / scale) as u32).collect();
    m[0] | m[1] << 9 | m[2] << 18 | ((e + 15) as u32) << 27
}

pub fn from_rgb9e5(b: u32) -> [f32; 3] {
    let scale = 2f64.powi((b >> 27) as i32 - 15 - 9);
    [(b & 511) as f64 * scale, (b >> 9 & 511) as f64 * scale, (b >> 18 & 511) as f64 * scale].map(|v| v as f32)
}

fn put(out: &mut [u8], at: u32, n: u32, v: u64) {
    for i in 0..n {
        let bit = at + i;
        let byte = (bit / 8) as usize;
        if v >> i & 1 == 1 {
            out[byte] |= 1 << (bit % 8);
        } else {
            out[byte] &= !(1 << (bit % 8));
        }
    }
}

fn get(b: &[u8], at: u32, n: u32) -> u64 {
    (0..n).fold(0, |v, i| v | ((b[((at + i) / 8) as usize] >> ((at + i) % 8) & 1) as u64) << i)
}

/// a 32-bit float as a clear or a render target write stores it (MEASURED: denormals flushed
/// to a zero of their sign, clear-2-*)
pub fn f32_store(v: f32) -> u32 {
    hit(clause::CONV_F32);
    let b = v.to_bits();
    if b & 0x7f80_0000 == 0 { b & 0x8000_0000 } else { b }
}

/// a float clear of an n-bit UINT or SINT component, as its n bits (MEASURED: toward zero,
/// clamped to the type, NaN 0; clear-3-*, 4, 30, 32)
pub fn int_clear(v: f32, n: u32, signed: bool) -> u64 {
    hit(clause::CONV_INTCLEAR);
    let t = if v.is_nan() { 0.0 } else { (v as f64).trunc() };
    let (lo, hi) = if signed { (-((1u64 << (n - 1)) as f64), ((1u64 << (n - 1)) - 1) as f64) } else { (0.0, ((1u64 << n) - 1) as f64) };
    (t.clamp(lo, hi) as i64 as u64) & ((1u64 << n) - 1)
}

/// one element of `f` holding the color (r, g, b, a) as a render target write or a clear
/// stores it (unormalized and float formats; the integer formats take the values' bits as
/// integers: see `encode_int`)
pub fn encode(f: &Format, c: [f32; 4]) -> Res<Vec<u8>> {
    let mut out = vec![0u8; (f.bits / 8) as usize];
    match f.id {
        26 => {
            hit(clause::CONV_F11);
            let v = to_small_float(c[0], 5, 6, false) as u64 | (to_small_float(c[1], 5, 6, false) as u64) << 11 | (to_small_float(c[2], 5, 5, false) as u64) << 22;
            put(&mut out, 0, 32, v);
        }
        67 => put(&mut out, 0, 32, to_rgb9e5([c[0], c[1], c[2]]) as u64),
        // (MEASURED: B8G8R8X8_UNORM's X byte takes the alpha, as B8G8R8A8 does, clear-88-*;
        // B8G8R8X8_UNORM_SRGB's is 0, clear-93-*)
        88 => {
            hit(clause::CONV_X8);
            return encode(super::dxgifmt::format(87).ok_or(Fail::Bad)?, c);
        }
        93 => {
            hit(clause::CONV_X8);
            let mut b = encode(super::dxgifmt::format(91).ok_or(Fail::Bad)?, c)?;
            b[3] = 0;
            return Ok(b);
        }
        89 => return Err(Fail::Unmodeled(open::CONV_XRBIAS)),
        _ if !f.comps.is_empty() => {
            let mut at = 0;
            for (i, &n) in f.comps.iter().enumerate() {
                let ch = f.chans[i] as usize;
                let v = c[ch];
                let bits: u64 = match f.num {
                    Num::Unorm => to_unorm(v, n) as u64,
                    Num::Srgb if ch < 3 && n == 8 => {
                        hit(clause::CONV_SRGB);
                        u64::from(srgb8(v))
                    }
                    Num::Srgb => to_unorm(v, n) as u64,
                    Num::Snorm => to_snorm(v, n) as u64,
                    Num::Float if n == 32 => u64::from(f32_store(v)),
                    Num::Float => {
                        hit(clause::CONV_F16);
                        to_small_float(v, 5, 10, true) as u64
                    }
                    Num::Uint | Num::Sint => int_clear(v, n, f.num == Num::Sint),
                    _ => return Err(Fail::Unmodeled(open::CONV_OTHER)),
                };
                put(&mut out, at, n, bits);
                at += n;
            }
        }
        _ => return Err(Fail::Unmodeled(open::CONV_OTHER)),
    }
    Ok(out)
}

/// the integer formats: each component the low bits of the value (an unordered access view's
/// uint clear, a uint shader output)
pub fn encode_bits(f: &Format, c: [u32; 4]) -> Vec<u8> {
    let mut out = vec![0u8; (f.bits / 8) as usize];
    let mut at = 0;
    for (i, &n) in f.comps.iter().enumerate() {
        put(&mut out, at, n, c[f.chans[i] as usize] as u64);
        at += n;
    }
    out
}

/// what a shader reads from one element: (r, g, b, a) as the bits of floats, or of integers for
/// the integer formats
pub fn decode(f: &Format, b: &[u8]) -> Res<[u32; 4]> {
    let one = 1.0f32.to_bits();
    let mut v = [0, 0, 0, if matches!(f.num, Num::Uint | Num::Sint) { 1 } else { one }];
    match f.id {
        26 => {
            hit(clause::CONV_F11);
            let w = get(b, 0, 32) as u32;
            v[0] = from_small_float(w & 0x7ff, 5, 6, false).to_bits();
            v[1] = from_small_float(w >> 11 & 0x7ff, 5, 6, false).to_bits();
            v[2] = from_small_float(w >> 22, 5, 5, false).to_bits();
        }
        67 => {
            hit(clause::CONV_RGB9E5);
            let c = from_rgb9e5(get(b, 0, 32) as u32);
            v[..3].copy_from_slice(&c.map(f32::to_bits));
        }
        41 | 21 => {
            hit(clause::CONV_F32);
            v[0] = get(b, 0, 32) as u32;
        }
        46 => {
            v[0] = (get(b, 0, 24) as f32 / 16_777_215.0).to_bits();
            hit(clause::CONV_UNORM);
        }
        47 => {
            hit(clause::CONV_STENCIL);
            v = [0, get(b, 24, 8) as u32, 0, 1];
        }
        88 | 93 => {
            // (the X byte reads as an alpha of 1)
            hit(clause::CONV_X8);
            let mut v = decode(super::dxgifmt::format(if f.id == 88 { 87 } else { 91 }).ok_or(Fail::Bad)?, b)?;
            v[3] = one;
            return Ok(v);
        }
        89 => return Err(Fail::Unmodeled(open::CONV_XRBIAS)),
        _ if !f.comps.is_empty() => {
            let mut at = 0;
            for (i, &n) in f.comps.iter().enumerate() {
                let ch = f.chans[i] as usize;
                let c = get(b, at, n);
                at += n;
                v[ch] = match f.num {
                    Num::Unorm => {
                        hit(clause::CONV_UNORM);
                        (c as f32 / ((1u64 << n) - 1) as f32).to_bits()
                    }
                    Num::Srgb if ch < 3 => srgb_read(u8::try_from(c).map_err(|_| Fail::Bad)?).to_bits(),
                    Num::Srgb => (c as f32 / 255.0).to_bits(),
                    Num::Snorm => {
                        hit(clause::CONV_SNORM);
                        let s = ((c << (64 - n)) as i64) >> (64 - n);
                        let max = ((1i64 << (n - 1)) - 1) as f32;
                        // (MEASURED: times the reciprocal, in single precision; ld-31, ld-37)
                        ((s as f32 * (1.0 / max)).max(-1.0)).to_bits()
                    }
                    Num::Float if n == 32 => {
                        hit(clause::CONV_F32);
                        c as u32
                    }
                    Num::Float => {
                        hit(clause::CONV_F16);
                        from_small_float(c as u32, 5, 10, true).to_bits()
                    }
                    Num::Uint => c as u32,
                    Num::Sint => (((c << (64 - n)) as i64) >> (64 - n)) as u32,
                    _ => return Err(Fail::Unmodeled(open::CONV_OTHER)),
                };
            }
        }
        _ => return Err(Fail::Unmodeled(open::CONV_OTHER)),
    }
    Ok(v)
}

/// a BC1 colour block's 8-bit codes (r, g, b) at index `idx` (0..3) between its endpoints c0
/// and c1 (5:6:5); `four`: the four colour mode (always in BC2 and BC3, in BC1 when c0 > c1);
/// None: the black, transparent texel of the three colour mode (MEASURED, WARP, ld-71, 74, 77:
/// the endpoints widened to 8 bits by repeating their top bits, the thirds (171 a + 85 b) >> 8,
/// the half (a + b) >> 1)
pub fn bc1_codes(c0: u32, c1: u32, idx: u32, four: bool) -> Option<[u32; 3]> {
    let rep = |v: u32, n: u32| v << (8 - n) | v >> (2 * n - 8);
    let e = |v: u32| [rep(v >> 11, 5), rep(v >> 5 & 63, 6), rep(v & 31, 5)];
    let (a, z) = (e(c0), e(c1));
    Some(match (idx, four || c0 > c1) {
        (0, _) => a,
        (1, _) => z,
        (2, true) => [0, 1, 2].map(|k| (171 * a[k] + 85 * z[k]) >> 8),
        (3, true) => [0, 1, 2].map(|k| (85 * a[k] + 171 * z[k]) >> 8),
        (2, false) => [0, 1, 2].map(|k| (a[k] + z[k]) >> 1),
        _ => return None,
    })
}

/// BC3's alpha code at index `k` (0..7) between its endpoints (MEASURED: BC3's alpha steps in 8
/// bits, to nearest, ld-77)
pub fn bc3_alpha(a0: u32, a1: u32, k: u32) -> u32 {
    match k {
        0 => a0,
        1 => a1,
        _ if a0 > a1 => ((8 - k) * a0 + (k - 1) * a1 + 3) / 7,
        6 => 0,
        7 => 255,
        _ => ((6 - k) * a0 + (k - 1) * a1 + 2) / 5,
    }
}

/// a BC4 UNORM channel's 16-bit code at index `k` (0..7) between its endpoints: six or four
/// between them (with 0 and 1) (MEASURED, WARP, ld-80, ld-83: in 16 bits, the endpoints times
/// 257, the steps truncated; read as the code over 65535)
pub fn bc4_code(e0: u32, e1: u32, k: u32) -> u32 {
    let (a0, a1) = (e0.saturating_mul(257), e1.saturating_mul(257));
    match k {
        0 => a0,
        1 => a1,
        _ if e0 > e1 => ((8 - k) * a0 + (k - 1) * a1) / 7,
        6 => 0,
        7 => 65535,
        _ => ((6 - k) * a0 + (k - 1) * a1) / 5,
    }
}

/// a BC4 SNORM channel's 16-bit code at index `k` (0..7) between its endpoints (two's
/// complement bytes) (MEASURED, WARP, ld-table-81, ld-table-84: every texel of 3072: the
/// endpoints at least -127; the step's exact value over 127, times 32767, truncated toward zero
/// to a 16-bit code; read as the code times the float 1/32767)
pub fn bc4_snorm_code(r0: i8, r1: i8, k: i64) -> i16 {
    let (a0, a1) = (i64::from(r0.max(-127)), i64::from(r1.max(-127)));
    // (the step as a fraction n / d of the 8-bit range)
    let (n, d) = match k {
        0 => (a0, 1),
        1 => (a1, 1),
        _ if r0 > r1 => ((8 - k) * a0 + (k - 1) * a1, 7),
        6 => (-127, 1),
        7 => (127, 1),
        _ => ((6 - k) * a0 + (k - 1) * a1, 5),
    };
    i16::try_from(n * 32767 / (d * 127)).unwrap_or(i16::MAX)
}

/// the color (as floats) a format's element holds, for blending (dst read back)
/// the texel (x, y) of a block (block-compressed formats: Direct3D 11 functional spec, 19.5;
/// the 2x1 pairs of R8G8_B8G8 and G8R8_G8B8)
pub fn decode_block(f: &Format, b: &[u8], x: u32, y: u32) -> Res<[u32; 4]> {
    hit(clause::CONV_BLOCK);
    let one = 1.0f32;
    let un = |c: u32, n: u32| c as f32 / ((1u32 << n) - 1) as f32;
    let i = (y * 4 + x) as usize;
    // a BC1 color block (bc1_codes)
    let color = |c: &[u8], four: bool| -> [f32; 4] {
        let (c0, c1) = (u16::from_le_bytes([c[0], c[1]]) as u32, u16::from_le_bytes([c[2], c[3]]) as u32);
        let idx = (u32::from_le_bytes([c[4], c[5], c[6], c[7]]) >> (2 * i)) & 3;
        match bc1_codes(c0, c1, idx, four) {
            Some(rgb) => [un(rgb[0], 8), un(rgb[1], 8), un(rgb[2], 8), one],
            None => [0.0; 4],
        }
    };
    // a BC4 channel block's index at the texel
    let index = |c: &[u8]| ((u64::from_le_bytes([c[2], c[3], c[4], c[5], c[6], c[7], 0, 0]) >> (3 * i)) & 7) as u32;
    let chan = |c: &[u8]| -> f32 { bc4_code(c[0] as u32, c[1] as u32, index(c)) as f32 / 65535.0 };
    let schan = |c: &[u8]| -> f32 {
        let (r0, r1) = (i8::from_le_bytes([c[0]]), i8::from_le_bytes([c[1]]));
        f32::from(bc4_snorm_code(r0, r1, i64::from(index(c)))) * (1.0f32 / 32767.0)
    };
    let v: [f32; 4] = match f.id {
        68 | 69 => {
            let (r, g, bb) = if f.id == 68 { (b[0], [b[1], b[3]][x as usize], b[2]) } else { (b[1], [b[0], b[2]][x as usize], b[3]) };
            [un(r as u32, 8), un(g as u32, 8), un(bb as u32, 8), one]
        }
        71 => color(b, false),
        74 => {
            let mut c = color(&b[8..], true);
            c[3] = un((b[i / 2] >> (4 * (i % 2)) & 15) as u32, 4);
            c
        }
        77 => {
            let mut c = color(&b[8..], true);
            c[3] = un(bc3_alpha(b[0] as u32, b[1] as u32, index(b)), 8);
            c
        }
        80 => [chan(b), 0.0, 0.0, one],
        83 => [chan(b), chan(&b[8..]), 0.0, one],
        81 => {
            hit(clause::CONV_BC4SNORM);
            [schan(b), 0.0, 0.0, one]
        }
        84 => {
            hit(clause::CONV_BC4SNORM);
            [schan(b), schan(&b[8..]), 0.0, one]
        }
        95 | 96 => {
            hit(clause::CONV_BC6H);
            let h = super::bptc::bc6h(b.first_chunk().ok_or(Fail::Bad)?, f.id == 96, x, y);
            [from_small_float(h[0] as u32, 5, 10, true), from_small_float(h[1] as u32, 5, 10, true), from_small_float(h[2] as u32, 5, 10, true), one]
        }
        98 => {
            hit(clause::CONV_BC7);
            super::bptc::bc7(b.first_chunk().ok_or(Fail::Bad)?, x, y).map(|c| un(c as u32, 8))
        }
        // (the sRGB block formats: the UNORM block's codes through the sRGB table; the alpha as
        // it is)
        72 | 75 | 78 | 99 => {
            let unorm = super::dxgifmt::format(f.id - 1).ok_or(Fail::Bad)?;
            let v = decode_block(unorm, b, x, y)?.map(f32::from_bits);
            let code = |c: f32| u8::try_from((c * 255.0).round() as i64).map_err(|_| Fail::Bad);
            [srgb_read(code(v[0])?), srgb_read(code(v[1])?), srgb_read(code(v[2])?), v[3]]
        }
        _ => return Err(Fail::Unmodeled(open::CONV_OTHER)),
    };
    Ok(v.map(f32::to_bits))
}

pub fn decode_f(f: &Format, b: &[u8]) -> Res<[f32; 4]> {
    Ok(decode(f, b)?.map(f32::from_bits))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn half_floats() {
        let h = |v: f32| to_small_float(v, 5, 10, true);
        assert_eq!(h(1.0), 0x3c00);
        assert_eq!(h(-2.0), 0xc000);
        assert_eq!(h(65504.0), 0x7bff);
        assert_eq!(h(65519.0), 0x7bff);
        assert_eq!(h(65520.0), 0x7bff);
        assert_eq!(h(f32::INFINITY), 0x7c00);
        assert_eq!(h(2f32.powi(-24)), 1);
        assert_eq!(h(2f32.powi(-25)), 0);
        assert_eq!(h(3.0 * 2f32.powi(-25)), 1);
        assert_eq!(h(2f32.powi(-14)), 0x0400);
        assert_eq!(h(f32::from_bits(0x7f80_0001)), 0x7c01);
        assert_eq!(h(f32::from_bits(0xffc0_0000)), 0xfe00);
        assert_eq!(to_small_float(f32::NAN, 5, 6, false), 0x7ff);
        assert_eq!(to_small_float(-1.0, 5, 6, false), 0);
        assert_eq!(to_small_float(1.0, 5, 6, false), 0x3c0);
        for b in 0..0x7c00u32 {
            assert_eq!(h(from_small_float(b, 5, 10, true)), b);
        }
    }
    #[test]
    fn unorm() {
        assert_eq!(to_unorm(0.5, 8), 128);
        assert_eq!(to_snorm(-1.0, 8), 0x81);
        assert_eq!(to_rgb9e5([1.0, 0.0, 0.0]), 0x0000_0100 | 16 << 27);
    }
}
