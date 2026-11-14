//! How Direct3D 10 and 11 turn floats into each format's bits (a clear, a render target write)
//! and each format's bits into what a shader reads (ld, sample). Microsoft Learn: "Data
//! Conversion Rules" (FLOAT to UNORM / SNORM / SRGB, the 16, 11 and 10 bit floats, integer
//! conversions), "DXGI_FORMAT" (R9G9B9E5, R11G11B10), "Format Conversion using Direct3D 10.1".
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
    hit("CONV.UNORM");
    if v.is_nan() {
        return 0;
    }
    // (MEASURED: the product in single precision, then to nearest even; clear-11-*)
    let max = ((1u64 << n) - 1) as f32;
    rne((v.clamp(0.0, 1.0) * max) as f64) as u32
}

pub fn to_snorm(v: f32, n: u32) -> u32 {
    hit("CONV.SNORM");
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

pub fn linear_to_srgb(v: f32) -> f32 {
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) as f64 };
    (if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }) as f32
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
    hit("CONV.SMALLFLOAT");
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
            hit("CONV.NANREAD");
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
    hit("CONV.RGB9E5");
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

/// one element of `f` holding the color (r, g, b, a) as a render target write or a clear
/// stores it (unormalized and float formats; the integer formats take the values' bits as
/// integers: see `encode_int`)
pub fn encode(f: &Format, c: [f32; 4]) -> Res<Vec<u8>> {
    let mut out = vec![0u8; (f.bits / 8) as usize];
    match f.id {
        26 => {
            hit("CONV.F11");
            let v = to_small_float(c[0], 5, 6, false) as u64 | (to_small_float(c[1], 5, 6, false) as u64) << 11 | (to_small_float(c[2], 5, 5, false) as u64) << 22;
            put(&mut out, 0, 32, v);
        }
        67 => put(&mut out, 0, 32, to_rgb9e5([c[0], c[1], c[2]]) as u64),
        // (MEASURED: the X byte takes the alpha, as B8G8R8A8 does; clear-88-*, clear-93-*)
        88 | 92 | 93 => {
            hit("CONV.X8");
            return encode(super::dxgifmt::format(f.id - 1).unwrap(), c);
        }
        89 => return Err(Fail::Unmodeled("CONV.XRBIAS")),
        _ if !f.comps.is_empty() => {
            let mut at = 0;
            for (i, &n) in f.comps.iter().enumerate() {
                let ch = f.chans[i] as usize;
                let v = c[ch];
                let bits: u64 = match f.num {
                    Num::Unorm => to_unorm(v, n) as u64,
                    Num::Srgb if ch < 3 => {
                        hit("CONV.SRGB");
                        let x = linear_to_srgb(v);
                        // (WARP's curve is not the exact one: 0.49999997 gives 187, not 188,
                        // clear-29-0; which values near a half step, open)
                        let k = (x.clamp(0.0, 1.0) as f64) * 255.0;
                        if (k - k.floor() - 0.5).abs() < 0.02 {
                            return Err(Fail::Unmodeled("CONV.SRGB.NEAR"));
                        }
                        to_unorm(x, n) as u64
                    }
                    Num::Srgb => to_unorm(v, n) as u64,
                    Num::Snorm => to_snorm(v, n) as u64,
                    Num::Float if n == 32 => {
                        // (MEASURED: denormals flushed to a zero of their sign, clear-2-*)
                        hit("CONV.F32");
                        let b = v.to_bits();
                        (if b & 0x7f80_0000 == 0 { b & 0x8000_0000 } else { b }) as u64
                    }
                    Num::Float => {
                        hit("CONV.F16");
                        to_small_float(v, 5, 10, true) as u64
                    }
                    // (MEASURED: toward zero, clamped to the type, NaN 0; clear-3-*, 4, 30, 32)
                    Num::Uint | Num::Sint => {
                        hit("CONV.INTCLEAR");
                        let t = if v.is_nan() { 0.0 } else { (v as f64).trunc() };
                        let (lo, hi) = if f.num == Num::Uint { (0.0, ((1u64 << n) - 1) as f64) } else { (-((1u64 << (n - 1)) as f64), ((1u64 << (n - 1)) - 1) as f64) };
                        (t.clamp(lo, hi) as i64 as u64) & ((1u64 << n) - 1)
                    }
                    _ => return Err(Fail::Unmodeled("CONV.OTHER")),
                };
                put(&mut out, at, n, bits);
                at += n;
            }
        }
        _ => return Err(Fail::Unmodeled("CONV.OTHER")),
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
            hit("CONV.F11");
            let w = get(b, 0, 32) as u32;
            v[0] = from_small_float(w & 0x7ff, 5, 6, false).to_bits();
            v[1] = from_small_float(w >> 11 & 0x7ff, 5, 6, false).to_bits();
            v[2] = from_small_float(w >> 22, 5, 5, false).to_bits();
        }
        67 => {
            hit("CONV.RGB9E5");
            let c = from_rgb9e5(get(b, 0, 32) as u32);
            v[..3].copy_from_slice(&c.map(f32::to_bits));
        }
        41 | 21 => {
            hit("CONV.F32");
            v[0] = get(b, 0, 32) as u32;
        }
        46 => {
            v[0] = (get(b, 0, 24) as f32 / 16_777_215.0).to_bits();
            hit("CONV.UNORM");
        }
        47 => {
            hit("CONV.STENCIL");
            v = [0, get(b, 24, 8) as u32, 0, 1];
        }
        88 | 92 | 93 => {
            // (the X byte reads as an alpha of 1)
            hit("CONV.X8");
            let mut v = decode(super::dxgifmt::format(f.id - 1).unwrap(), b)?;
            v[3] = one;
            return Ok(v);
        }
        89 => return Err(Fail::Unmodeled("CONV.XRBIAS")),
        _ if !f.comps.is_empty() => {
            let mut at = 0;
            for (i, &n) in f.comps.iter().enumerate() {
                let ch = f.chans[i] as usize;
                let c = get(b, at, n);
                at += n;
                v[ch] = match f.num {
                    Num::Unorm => {
                        hit("CONV.UNORM");
                        (c as f32 / ((1u64 << n) - 1) as f32).to_bits()
                    }
                    Num::Srgb if ch < 3 => {
                        // (WARP's curve is a few ulps off the exact one, ld-29; its table, open)
                        return Err(Fail::Unmodeled("CONV.SRGBREAD"));
                    }
                    Num::Srgb => (c as f32 / 255.0).to_bits(),
                    Num::Snorm => {
                        hit("CONV.SNORM");
                        let s = ((c << (64 - n)) as i64) >> (64 - n);
                        let max = ((1i64 << (n - 1)) - 1) as f32;
                        // (MEASURED: times the reciprocal, in single precision; ld-31, ld-37)
                        ((s as f32 * (1.0 / max)).max(-1.0)).to_bits()
                    }
                    Num::Float if n == 32 => {
                        hit("CONV.F32");
                        c as u32
                    }
                    Num::Float => {
                        hit("CONV.F16");
                        from_small_float(c as u32, 5, 10, true).to_bits()
                    }
                    Num::Uint => c as u32,
                    Num::Sint => (((c << (64 - n)) as i64) >> (64 - n)) as u32,
                    _ => return Err(Fail::Unmodeled("CONV.OTHER")),
                };
            }
        }
        _ => return Err(Fail::Unmodeled("CONV.OTHER")),
    }
    Ok(v)
}

/// the color (as floats) a format's element holds, for blending (dst read back)
/// the texel (x, y) of a block (block-compressed formats: Direct3D 11 functional spec, 19.5;
/// the 2x1 pairs of R8G8_B8G8 and G8R8_G8B8)
pub fn decode_block(f: &Format, b: &[u8], x: u32, y: u32) -> Res<[u32; 4]> {
    hit("CONV.BLOCK");
    let one = 1.0f32;
    let un = |c: u32, n: u32| c as f32 / ((1u32 << n) - 1) as f32;
    let i = (y * 4 + x) as usize;
    // a BC1 color block: the colors' endpoints in 5:6:5, two more between them (or one and
    // black with no alpha when the first is not the larger) (MEASURED, WARP, ld-71, 74, 77: the
    // endpoints widened to 8 bits by repeating their top bits, the thirds (171 a + 85 b) >> 8,
    // the half (a + b) >> 1)
    let color = |c: &[u8], four: bool| -> [f32; 4] {
        let (c0, c1) = (u16::from_le_bytes([c[0], c[1]]) as u32, u16::from_le_bytes([c[2], c[3]]) as u32);
        let rep = |v: u32, n: u32| v << (8 - n) | v >> (2 * n - 8);
        let e = |v: u32| [rep(v >> 11, 5), rep(v >> 5 & 63, 6), rep(v & 31, 5)];
        let (a, z) = (e(c0), e(c1));
        let idx = (u32::from_le_bytes([c[4], c[5], c[6], c[7]]) >> (2 * i)) & 3;
        let rgb = match (idx, four || c0 > c1) {
            (0, _) => a,
            (1, _) => z,
            (2, true) => [0, 1, 2].map(|k| (171 * a[k] + 85 * z[k]) >> 8),
            (3, true) => [0, 1, 2].map(|k| (85 * a[k] + 171 * z[k]) >> 8),
            (2, false) => [0, 1, 2].map(|k| (a[k] + z[k]) >> 1),
            _ => return [0.0; 4],
        };
        [un(rgb[0], 8), un(rgb[1], 8), un(rgb[2], 8), one]
    };
    // a BC4 channel block: two endpoints, six or four between them (with 0 and 1)
    // (MEASURED, WARP, ld-80, ld-83: in 16 bits, the endpoints times 257, the steps truncated,
    // then over 65535)
    let chan = |c: &[u8]| -> f32 {
        let (a0, a1) = (c[0] as u32 * 257, c[1] as u32 * 257);
        let bits = u64::from_le_bytes([c[2], c[3], c[4], c[5], c[6], c[7], 0, 0]);
        let k = ((bits >> (3 * i)) & 7) as u32;
        let r = match k {
            0 => a0,
            1 => a1,
            _ if c[0] > c[1] => ((8 - k) * a0 + (k - 1) * a1) / 7,
            6 => 0,
            7 => 65535,
            _ => ((6 - k) * a0 + (k - 1) * a1) / 5,
        };
        r as f32 / 65535.0
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
            // (MEASURED: BC3's alpha steps in 8 bits, to nearest, ld-77)
            let (a0, a1) = (b[0] as u32, b[1] as u32);
            let bits = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]);
            let k = ((bits >> (3 * i)) & 7) as u32;
            let a = match k {
                0 => a0,
                1 => a1,
                _ if a0 > a1 => ((8 - k) * a0 + (k - 1) * a1 + 3) / 7,
                6 => 0,
                7 => 255,
                _ => ((6 - k) * a0 + (k - 1) * a1 + 2) / 5,
            };
            let mut c = color(&b[8..], true);
            c[3] = un(a, 8);
            c
        }
        80 => [chan(b), 0.0, 0.0, one],
        83 => [chan(b), chan(&b[8..]), 0.0, one],
        // (the signed steps: within an ulp of 16-bit steps over 32767, which ulp, open)
        81 | 84 => return Err(Fail::Unmodeled("CONV.BC4SNORM")),
        95 | 96 => {
            hit("CONV.BC6H");
            let h = super::bptc::bc6h(b, f.id == 96, x, y);
            [from_small_float(h[0] as u32, 5, 10, true), from_small_float(h[1] as u32, 5, 10, true), from_small_float(h[2] as u32, 5, 10, true), one]
        }
        98 => {
            hit("CONV.BC7");
            super::bptc::bc7(b, x, y).map(|c| un(c as u32, 8))
        }
        72 | 75 | 78 | 99 => return Err(Fail::Unmodeled("CONV.SRGBREAD")),
        _ => return Err(Fail::Unmodeled("CONV.OTHER")),
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
