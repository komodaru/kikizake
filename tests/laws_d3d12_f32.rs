//! The clear conversions of Direct3D 12 (spec/d3d12.txt D3D12.CLEAR.*) for every 32-bit float: each
//! law runs over all 2^32 bit patterns, so it is a proof for the whole domain (kind exhaustive).
use kikizake::model::d3d12::{clear_input, to_int, to_small_float, to_snorm, to_srgb, to_unorm};

/// runs `f` over every u32 on the machine's threads; the first failing value, if any
fn all_u32(f: impl Fn(u32) -> bool + Sync) -> Option<u32> {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let chunk = (1u64 << 32) / n as u64;
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..n as u64)
            .map(|k| {
                let f = &f;
                s.spawn(move || {
                    let lo = k * chunk;
                    let hi = if k + 1 == n as u64 { 1u64 << 32 } else { lo + chunk };
                    (lo..hi).map(|v| v as u32).find(|&v| !f(v))
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().unwrap()).min()
    })
}

/// a small float's value (e exponent, m mantissa bits, the sign bit above them when signed)
fn small_value(h: u64, e: u32, m: u32, signed: bool) -> f64 {
    let (ex, man) = ((h >> m) & ((1 << e) - 1), h & ((1 << m) - 1));
    let bias = (1i32 << (e - 1)) - 1;
    let v = if ex == 0 { man as f64 * 2f64.powi(1 - bias - m as i32) } else { (1.0 + man as f64 / (1u64 << m) as f64) * 2f64.powi(ex as i32 - bias) };
    if signed && h >> (e + m) & 1 == 1 { -v } else { v }
}

/// D3D12.CLEAR.DENORM: a denormal input is 0 of its sign, every other input kept
#[test]
fn denormals_are_flushed_for_every_float() {
    assert_eq!(all_u32(|v| {
        let f = f32::from_bits(v);
        let r = clear_input(v);
        if f.is_subnormal() { r == v & 0x8000_0000 } else { r == v }
    }), None);
}

/// D3D12.CLEAR.UNORM: for every float: NaN 0, at most 0 is 0, at least 1 is 2^n - 1, and in between
/// the nearest of the 2^n levels to c (2^n - 1), the product taken in f32 as the measurement says
#[test]
fn unorm_is_the_nearest_level_for_every_float() {
    for n in [8u32, 16] {
        let m = ((1u64 << n) - 1) as f32;
        assert_eq!(all_u32(|v| {
            let c = f32::from_bits(v);
            let r = to_unorm(c, n);
            if c.is_nan() { return r == 0; }
            if c <= 0.0 { return r == 0; }
            if c >= 1.0 { return r == (1 << n) - 1; }
            let p = f64::from(c * m);
            (r as f64 - p).abs() <= 0.5 && r <= (1 << n) - 1
        }), None, "n {n}");
    }
}

/// D3D12.CLEAR.UNORM: the conversion never decreases: over the floats 0..1 in order (their bit
/// patterns are in order)
#[test]
fn unorm_never_decreases() {
    for n in [8u32, 16] {
        let mut last = 0;
        for v in 0..=1.0f32.to_bits() {
            let r = to_unorm(f32::from_bits(v), n);
            assert!(r >= last, "n {n} {v:#x}");
            last = r;
        }
    }
}

/// D3D12.CLEAR.SNORM: for every float: NaN 0, clamped to -1..1, c (2^(n-1) - 1) to the nearest
/// integer, halves away from zero, as n-bit two's complement; odd (-c gives the negation)
#[test]
fn snorm_is_the_nearest_for_every_float() {
    for n in [8u32, 16] {
        let mask = (1u64 << n) - 1;
        let m = ((1i64 << (n - 1)) - 1) as f32;
        assert_eq!(all_u32(|v| {
            let c = f32::from_bits(v);
            let r = to_snorm(c, n);
            if c.is_nan() { return r == 0; }
            let s = if r >> (n - 1) & 1 == 1 { r as i64 - (1i64 << n) } else { r as i64 };
            let p = f64::from(c.clamp(-1.0, 1.0) * m);
            let near = (s as f64 - p).abs() <= 0.5;
            near && to_snorm(-c, n) == r.wrapping_neg() & mask
        }), None, "n {n}");
    }
}

/// D3D12.CLEAR.SRGB: for every float: NaN 0, clamped to 0..1, the documented curve (12.92c up to
/// 0.0031308, else 1.055c^(1/2.4) - 0.055, here in f64) then as UNORM: the level within half a step
/// of it (and 1/1000 of a step for the f32 power the conversion takes)
#[test]
fn srgb_is_the_curve_for_every_float() {
    assert_eq!(all_u32(|v| {
        let c = f32::from_bits(v);
        let r = to_srgb(c, 8);
        if c.is_nan() { return r == 0; }
        let x = f64::from(c.clamp(0.0, 1.0));
        let s = if x <= 0.0031308 { 12.92 * x } else { 1.055 * x.powf(1.0 / 2.4) - 0.055 };
        r <= 255 && (r as f64 - s * 255.0).abs() <= 0.501
    }), None);
}

/// D3D12.CLEAR.SRGB: never decreasing over 0..1, 0 at 0, the top at 1
#[test]
fn srgb_never_decreases() {
    let mut last = 0;
    for v in 0..=1.0f32.to_bits() {
        let r = to_srgb(f32::from_bits(v), 8);
        assert!(r >= last, "{v:#x}");
        last = r;
    }
    assert_eq!((to_srgb(0.0, 8), to_srgb(1.0, 8)), (0, 255));
}

/// D3D12.CLEAR.FLOAT16: for every float: round to zero (the result's value at most the input's,
/// the next one away from zero past it), too large the largest finite with the sign, infinities
/// kept, NaN a quiet NaN
#[test]
fn float16_rounds_to_zero_for_every_float() {
    assert_eq!(all_u32(|v| {
        let c = f32::from_bits(v);
        let h = to_small_float(v, 5, 10, true);
        if h >> 16 != 0 { return false; }
        if c.is_nan() { return h & 0x7c00 == 0x7c00 && h & 0x3ff != 0; }
        if c.is_infinite() { return h & 0x7fff == 0x7c00 && (h >> 15 == 1) == (c < 0.0); }
        if h & 0x7c00 == 0x7c00 { return false; }
        let (x, val) = (f64::from(c).abs(), small_value(h, 5, 10, true).abs());
        let next_past = h & 0x7fff == 0x7bff || small_value(h + 1, 5, 10, true).abs() > x;
        // (the sign kept, a zero's too)
        val <= x && next_past && ((h >> 15 == 1) == c.is_sign_negative())
    }), None);
}

/// D3D12.CLEAR.FLOAT11: 11 and 10-bit floats for every float: as FLOAT16 (round to zero, too large
/// the largest), never negative (a negative value 0), a NaN all ones, +inf kept
#[test]
fn small_floats_round_to_zero_for_every_float() {
    for (e, m) in [(5u32, 6u32), (5, 5)] {
        let ones = (1u64 << (e + m)) - 1;
        let inf = 31u64 << m;
        let top = (30u64 << m) | ((1 << m) - 1);
        assert_eq!(all_u32(|v| {
            let c = f32::from_bits(v);
            let h = to_small_float(v, e, m, false);
            if c.is_nan() { return h == ones; }
            if c.is_sign_negative() || c == 0.0 { return h == 0; }
            if c.is_infinite() { return h == inf; }
            if h > top { return false; }
            let (x, val) = (f64::from(c), small_value(h, e, m, false));
            val <= x && (h == top || small_value(h + 1, e, m, false) > x)
        }), None, "{e} {m}");
    }
}

/// D3D12.CLEAR.INT: for every float: NaN 0, truncated toward zero and clamped to the channel's range
#[test]
fn ints_truncate_and_clamp_for_every_float() {
    for n in [8u32, 16, 32] {
        assert_eq!(all_u32(|v| {
            let c = f32::from_bits(v);
            let (u, s) = (to_int(c, n, false), to_int(c, n, true));
            if c.is_nan() { return u == 0 && s == 0; }
            let t = f64::from(c).trunc();
            let umax = ((1u64 << n) - 1) as f64;
            let (smin, smax) = (-((1i64 << (n - 1)) as f64), ((1i64 << (n - 1)) - 1) as f64);
            let sv = if s >> (n - 1) & 1 == 1 { s as i64 - (1i64 << n) } else { s as i64 };
            u as f64 == t.clamp(0.0, umax) && sv as f64 == t.clamp(smin, smax)
        }), None, "n {n}");
    }
}

/// D3D12.CLEAR.FLOAT32: an R32_FLOAT texel holds every input as it is (after D3D12.CLEAR.DENORM)
#[test]
fn float32_holds_every_float() {
    assert_eq!(all_u32(|v| {
        kikizake::model::d3d12::clear_texel(41, [v, 0, 0, 0]).is_some_and(|t| t == clear_input(v).to_le_bytes())
    }), None);
}
