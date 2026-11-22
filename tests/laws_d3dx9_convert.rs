//! D3DXLoadSurfaceFromMemory's channel reads and writes (spec/d3dx9.txt D3DX9.LOAD.CONVERT,
//! LOAD.SIGNED): every raw value of every width; into every width the formats have every f32
//! of the clamp's range and every f32 of each sign and exponent past it (kind exhaustive).
use kikizake::model::d3dx9::{luminance, snorm_read, snorm_write, unorm_read, unorm_write};

mod every;
use every::all_u32;

/// a binary64 rounded to the nearest binary32 (the products and sums below are exact in
/// binary64, so this is the binary32 operation)
fn r32(x: f64) -> f64 {
    f64::from(x as f32)
}

/// the unsigned and the signed widths of D3DFORMAT's integer channels (D3D9.FORMATS)
const UNSIGNED: [u32; 9] = [1, 2, 3, 4, 5, 6, 8, 10, 16];
const SIGNED: [u32; 4] = [5, 8, 10, 16];

/// the writes' domain: every f32 from lo to hi (bit patterns of one sign), and every f32 whose
/// low 16 bits are 0 (each sign and exponent, the infinities and NaNs among them)
fn within(w: u32, lo: u32, hi: u32) -> bool {
    (lo..=hi).contains(&w) || w & 0xffff == 0
}

/// D3DX9.LOAD.CONVERT, LOAD.SIGNED: every raw value of every width 1..16 (signed 2..16): v
/// times the f32 nearest 1 / (2^n - 1); the two's complement times the f32 nearest
/// 1 / (2^(n-1) - 1), at least -1; each product rounded to f32
#[test]
fn every_raw_channel_reads_as_the_clause() {
    for n in 1..=16u32 {
        let top = (1u64 << n) - 1;
        let inv = r32(1.0 / f64::from(u32::try_from(top).unwrap()));
        for raw in 0..=top {
            let want = r32(f64::from(u32::try_from(raw).unwrap()) * inv);
            assert_eq!(f64::from(unorm_read(raw, n)), want, "{n} {raw}");
        }
        if n < 2 {
            continue;
        }
        let half = 1i64 << (n - 1);
        let inv = r32(1.0 / f64::from(i32::try_from(half - 1).unwrap()));
        for raw in 0..=top {
            let r = i64::try_from(raw).unwrap();
            let s = if r >= half { r - 2 * half } else { r };
            let want = r32(f64::from(i32::try_from(s).unwrap()) * inv).max(-1.0);
            assert_eq!(f64::from(snorm_read(raw, n)), want, "{n} {raw}");
        }
    }
}

/// D3DX9.LOAD.CONVERT: every f32 in 0..1 and every one of low 16 bits 0 into every unsigned
/// width: clamped to 0..1 (NaN 1), times 2^n - 1; x86 the exact product rounded to nearest (a
/// half to even), x64 the f32 product plus 1/2 in f32, truncated
#[test]
fn every_f32_writes_unsigned_as_the_clause() {
    assert_eq!(all_u32(|w| {
        if !within(w, 0, 0x3f80_0000) {
            return true;
        }
        let v = f32::from_bits(w);
        let c = if v.is_nan() { 1.0 } else { f64::from(v.clamp(0.0, 1.0)) };
        UNSIGNED.iter().all(|&n| {
            let m = f64::from(u32::try_from((1u64 << n) - 1).unwrap());
            let x = c * m;
            let lo = x.floor();
            let x86 = if x - lo > 0.5 || x - lo == 0.5 && lo % 2.0 != 0.0 { lo + 1.0 } else { lo };
            let x64 = r32(r32(x) + 0.5).trunc();
            let u = |q: u64| f64::from(u32::try_from(q).unwrap());
            u(unorm_write(v, n, false)) == x86 && u(unorm_write(v, n, true)) == x64
        })
    }), None);
}

/// D3DX9.LOAD.SIGNED: every f32 in -1..1 and every one of low 16 bits 0 into every signed
/// width: clamped to -1..1 (NaN 1), times 2^(n-1) - 1 plus 1/2 in f32, truncated toward 0, as
/// the channel's two's complement
#[test]
fn every_f32_writes_signed_as_the_clause() {
    assert_eq!(all_u32(|w| {
        if !within(w & 0x7fff_ffff, 0, 0x3f80_0000) {
            return true;
        }
        let v = f32::from_bits(w);
        let c = if v.is_nan() { 1.0 } else { f64::from(v.clamp(-1.0, 1.0)) };
        SIGNED.iter().all(|&n| {
            let half = 1i64 << (n - 1);
            let t = r32(r32(c * f64::from(i32::try_from(half - 1).unwrap())) + 0.5).trunc();
            let t = i64::from(i32::try_from(t as i64).unwrap());
            u64::try_from(t.rem_euclid(2 * half)).unwrap() == snorm_write(v, n)
        })
    }), None);
}

/// D3DX9.LOAD.CONVERT: luminance of every (r, g, b) of 8-bit reads: 0.2125 r + 0.7154 g +
/// 0.0721 b, each product and sum in f32, left to right
#[test]
fn luminance_is_rec_709_in_f32() {
    let k = |c: f32| f64::from(c);
    for r in 0..=255u64 {
        for g in 0..=255u64 {
            for b in 0..=255u64 {
                let (fr, fg, fb) = (unorm_read(r, 8), unorm_read(g, 8), unorm_read(b, 8));
                let want = r32(r32(r32(k(0.2125) * k(fr)) + r32(k(0.7154) * k(fg))) + r32(k(0.0721) * k(fb)));
                assert_eq!(f64::from(luminance(fr, fg, fb)), want, "{r} {g} {b}");
            }
        }
    }
}
