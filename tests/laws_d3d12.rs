//! Laws of the Direct3D 12 and DXGI models (exhaustive over small domains, property-based
//! elsewhere): what the documentation states as properties, whatever Windows' measured details
//! turn out to be.
use kikizake::gen::d3d12::Desc;
use kikizake::model::d3d12::{chain_desc, clear_texel, footprints, to_int, to_small_float, to_snorm, to_srgb, to_unorm, validate};
use kikizake::model::dxgi_format::{block, channels};
use proptest::prelude::*;

/// "Data Conversion Rules": every UNORM value k is what k / (2^n - 1) converts to, and the
/// conversion never decreases (exhaustive for n <= 16)
#[test]
fn unorm_reaches_every_value_in_order() {
    for n in [1, 2, 4, 5, 6, 8, 10, 16] {
        let m = (1u64 << n) - 1;
        for k in 0..=m {
            assert_eq!(to_unorm(k as f32 / m as f32, n), k, "n {n} k {k}");
        }
        let mut last = 0;
        for i in 0..=4096u32 {
            let v = to_unorm(i as f32 / 4096.0, n);
            assert!(v >= last);
            last = v;
        }
        assert_eq!(to_unorm(-1.0, n), 0);
        assert_eq!(to_unorm(f32::INFINITY, n), m);
        assert_eq!(to_unorm(f32::NAN, n), 0);
    }
}

/// SNORM: every value but the most negative is reached from k / (2^(n-1) - 1), and the
/// conversion is odd (c and -c give opposite values)
#[test]
fn snorm_reaches_every_value_symmetrically() {
    for n in [8, 10, 16] {
        let m = (1i64 << (n - 1)) - 1;
        let mask = (1u64 << n) - 1;
        for k in -m..=m {
            assert_eq!(to_snorm(k as f32 / m as f32, n), (k as u64) & mask, "n {n} k {k}");
        }
        for i in 0..=2048u32 {
            let c = i as f32 / 2048.0;
            assert_eq!(to_snorm(-c, n), to_snorm(c, n).wrapping_neg() & mask, "{c}");
        }
        assert_eq!(to_snorm(-5.0, n), (-m as u64) & mask);
    }
}

/// a 16-bit float's value survives the trip through a 32-bit float (exhaustive), and the
/// conversion of any float never moves away from zero (round to zero)
#[test]
fn float16_round_trips_and_rounds_to_zero() {
    for h in 0..=0xffffu32 {
        let (e, m) = (h >> 10 & 31, h & 0x3ff);
        if e == 31 && m != 0 {
            continue;
        }
        let sign = if h & 0x8000 != 0 { -1.0f64 } else { 1.0 };
        let v = if e == 31 { f64::INFINITY } else if e == 0 { m as f64 * 2f64.powi(-24) } else { (1.0 + m as f64 / 1024.0) * 2f64.powi(e as i32 - 15) };
        let f = (sign * v) as f32;
        assert_eq!(to_small_float(f.to_bits(), 5, 10, true), h as u64, "{h:#x} {f}");
    }
}

fn f16_value(h: u64) -> f64 {
    let (e, m) = (h >> 10 & 31, h & 0x3ff);
    let v = if e == 0 { m as f64 * 2f64.powi(-24) } else { (1.0 + m as f64 / 1024.0) * 2f64.powi(e as i32 - 15) };
    if h & 0x8000 != 0 {
        -v
    } else {
        v
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn float16_is_round_to_zero(c in -70000.0f32..70000.0) {
        let h = to_small_float(c.to_bits(), 5, 10, true);
        let v = f16_value(h);
        prop_assert!(v.abs() <= (c as f64).abs() + 1e-30);
        // (and the next value away from zero is past c, or h is the largest)
        if h & 0x7fff < 0x7bff {
            prop_assert!(f16_value(h + 1).abs() > (c as f64).abs());
        }
    }

    #[test]
    fn unsigned_small_floats_never_negative(bits: u32) {
        for (e, m) in [(5, 6), (5, 5)] {
            let v = to_small_float(bits, e, m, false);
            prop_assert!(v < 1 << (e + m));
            if bits >> 31 == 1 && (bits & 0x7fff_ffff) <= 0x7f80_0000 {
                prop_assert_eq!(v, 0);
            }
        }
    }

    #[test]
    fn srgb_is_monotonic(a in 0.0f32..1.0, b in 0.0f32..1.0) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(to_srgb(lo, 8) <= to_srgb(hi, 8));
    }

    #[test]
    fn ints_are_clamped(c: f32, n in prop::sample::select(vec![8u32, 10, 16, 32])) {
        let u = to_int(c, n, false);
        prop_assert!(u <= (1u64 << n) - 1);
        let s = to_int(c, n, true);
        prop_assert!(s <= (1u64 << n) - 1);
        if !c.is_nan() && c >= 0.0 && (c as f64) < ((1u64 << n) as f64) - 1.0 {
            // (truncated toward zero, measured)
            prop_assert_eq!(u as f64, (c as f64).trunc());
        }
    }

    /// a cleared texel is as wide as its format and the same color gives the same bytes
    #[test]
    fn clears_fill_one_texel(f in 0u32..200, rgba: [u32; 4]) {
        if let (Some(t), Some((bytes, _, _))) = (clear_texel(f, rgba), block(f)) {
            prop_assert_eq!(t.len(), bytes as usize);
            prop_assert_eq!(Some(t), clear_texel(f, rgba));
            prop_assert!(channels(f).is_some());
        }
    }

    /// footprints: either all ones, or rows padded to 256, subresources at multiples of 512 apart, in order, never
    /// overlapping, all inside the total
    #[test]
    fn footprints_are_aligned_and_disjoint(dim in 2u32..=4, w in 1u32..3000, h in 1u32..3000, d in 1u32..20, mips in 0u32..5, f in 1u32..116, base in 0u64..100000) {
        let Some((_, bw, bh)) = block(f) else { return Ok(()) };
        let h = if dim == 2 { 1 } else { h.div_ceil(bh) * bh };
        let desc = Desc::tex(dim, w.div_ceil(bw) * bw, h, d, mips, f);
        if validate(&desc).is_err() {
            return Ok(());
        }
        let n = if mips == 0 { kikizake::model::d3d12::full_chain(&desc) } else { mips } * if dim == 4 { 1 } else { d };
        let Ok((fps, total)) = footprints(0, n.min(16), base, &desc) else { return Ok(()) };
        // (a layout GetCopyableFootprints refuses, such as a block format in 1D: every field all ones)
        if total == u64::MAX {
            prop_assert!(fps.iter().all(|fp| fp.0 == u64::MAX && fp.5 == u64::MAX));
            return Ok(());
        }
        let mut end = base;
        for (i, fp) in fps.iter().enumerate() {
            prop_assert_eq!(fp.5 % 256, 0);
            prop_assert!(fp.5 >= fp.7);
            prop_assert_eq!((fp.0 - base) % 512, 0);
            prop_assert!(fp.0 >= end, "{} overlaps", i);
            end = fp.0 + fp.5 * (fp.6 as u64 * fp.4 as u64 - 1) + fp.7;
        }
        prop_assert_eq!(base + total, end);
    }

    /// a swap chain description the model accepts has a flip effect, 2..16 buffers, no
    /// multisampling and a flip format; its size is never 0
    #[test]
    fn accepted_chains_are_flip_model(w: [u32; 12]) {
        if let Ok(d) = chain_desc(&w) {
            prop_assert!(matches!(d[9], 3 | 4));
            prop_assert!((2..=16).contains(&d[7]));
            prop_assert!(d[4] == 1 && d[5] == 0);
            prop_assert!(d[0] > 0 && d[1] > 0);
        }
    }
}

/// a buffer description is valid exactly as D3D12_RESOURCE_DESC's Buffers section says,
/// exhaustively over the values of interest of each field
#[test]
fn buffer_rules_are_the_documented_ones() {
    let b = Desc::buffer(1024);
    for (at, vals) in [(1usize, vec![0u32, 1, 4096, 65536, 4194304]), (3, vec![0, 1, 2]), (4, vec![0, 1, 2]), (5, vec![0, 1, 2]), (6, vec![0, 28]), (7, vec![0, 1, 2]), (8, vec![0, 1]), (9, vec![0, 1, 2, 3])] {
        for v in vals {
            let mut w = b.words();
            w[at] = v;
            let ok = validate(&Desc::from_words(&w)).is_ok();
            let want = match at {
                1 => v == 0 || v == 65536,
                9 => v == 1,
                6 | 8 => v == 0,
                _ => v == 1,
            };
            assert_eq!(ok, want, "field {at} = {v}");
        }
    }
}

/// the suites stay answered: no case the model leaves unmodeled but the open clauses' (the
/// comparison passes over an unmodeled case, so a model arm deleted would go unseen otherwise)
#[test]
fn suites_stay_modeled() {
    use kikizake::case::parse_cases;
    use kikizake::gen;
    use kikizake::model::{self, run_case, Fail};
    for s in ["d3d12", "dxgi", "d3d12rm"] {
        let cases = parse_cases(&gen::suite(s).unwrap());
        for arch in [32, 64] {
            model::set_arch(arch);
            let un: Vec<_> = cases.iter().filter_map(|c| match run_case(c) {
                Err(Fail::Unmodeled(id)) => Some(id),
                _ => None,
            }).collect();
            assert!(un.iter().all(|id| *id == "DXGI.COLORSPACE"), "{s}-{arch}: {un:?}");
        }
    }
    model::set_arch(32);
}
