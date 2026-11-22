//! D3DXFloat32To16Array's and D3DXFloat16To32Array's conversions (spec/d3dx9.txt D3DX9.F16,
//! D3DX9.F32) for every 32-bit float and every half: each law runs over all 2^32 bit patterns
//! (or all 2^16), so it is a proof for the whole domain (kind exhaustive). The reference here is
//! integer arithmetic on the float's exact value, apart from the f32 the routines compute in.
use kikizake::model::d3dx9::{f16_one, f16_sse, f16_to_f32, f16_x64, f16_x64_tail};

mod every;
use every::all_u32;

/// how a value between two steps goes
#[derive(Clone, Copy)]
enum Ties {
    /// to the nearer, a tie to the even step
    Even,
    /// to the nearer, a tie to the step nearer 0
    Down,
    /// to the step nearer 0
    Trunc,
}

/// m 2^e (m < 2^25) as a whole number of steps 2^t, rounded as `ties` says
fn steps(m: u64, e: i32, t: i32, ties: Ties) -> u64 {
    if e >= t {
        return m << (e - t).min(40);
    }
    let sh = u32::try_from(t - e).unwrap_or(64).min(63);
    let (q, rest, half) = (m >> sh, m & ((1u64 << sh) - 1), 1u64 << (sh - 1));
    let up = match ties {
        Ties::Even => rest > half || rest == half && q & 1 == 1,
        Ties::Down => rest > half,
        Ties::Trunc => false,
    };
    q + u64::from(up)
}

/// a finite float's magnitude as m 2^e, m an integer below 2^24
fn exact(i: u32) -> (u64, i32) {
    let (ex, man) = (i >> 23 & 255, u64::from(i & 0x7f_ffff));
    if ex == 0 { (man, -149) } else { (man | 1 << 23, i32::try_from(ex).unwrap_or(0) - 150) }
}

/// the binade of m 2^e (m > 0): the k with 2^k <= m 2^e < 2^(k + 1)
fn binade(m: u64, e: i32) -> i32 {
    e + 63 - i32::try_from(m.leading_zeros()).unwrap_or(0)
}

/// m 2^e (a magnitude) as a half's 15 bits, exponent 31 a number, rounded as `ties` says at
/// the half's step (2^-24 below 2^-14, else 2^(k - 10)); a carry goes into the exponent
fn half_of(m: u64, e: i32, ties: Ties) -> u64 {
    if m == 0 {
        return 0;
    }
    let k = binade(m, e).max(-14);
    let r = steps(m, e, k - 10, ties);
    if k == -14 { r } else { (u64::try_from(k + 15).unwrap_or(0) << 10) + r - 1024 }
}

/// a magnitude below 2^-14 rounded first to a step of 2^-37 as `ties` says (the f32 sum with
/// 2^-14, the f32 product with 2^-112), else as it is
fn pre(m: u64, e: i32, ties: Ties) -> (u64, i32) {
    if m != 0 && binade(m, e) < -14 { (steps(m, e, -37, ties), -37) } else { (m, e) }
}

/// the sign as a half's
fn sign(x: u32) -> u16 {
    u16::try_from(x >> 16 & 0x8000).unwrap_or(0)
}

/// a 15-bit half code with the sign of x
fn with_sign(x: u32, h: u64) -> u16 {
    sign(x) | u16::try_from(h & 0x7fff).unwrap_or(0)
}

/// D3DX9.F16 (x86's 8 at a time, x64 GenuineIntel's): infinities and NaNs 0x7c00 of their sign;
/// a finite magnitude below 2^-14 first rounded to 2^-37 with ties to even (the f32 sum with
/// 2^-14), then to the half's step with ties toward zero, past 65504 0x7c00
#[test]
fn sse_halves_are_ties_toward_zero_for_every_float() {
    assert_eq!(all_u32(|x| {
        let i = x & 0x7fff_ffff;
        let r = f16_sse(x);
        if i >= 0x7f80_0000 {
            return r == sign(x) | 0x7c00;
        }
        let (m, e) = exact(i);
        let (m, e) = pre(m, e, Ties::Even);
        r == with_sign(x, half_of(m, e, Ties::Down).min(0x7c00))
    }), None);
}

/// D3DX9.F16 (x86's rest one by one): past 0x47ffefff (the tie above 131008, infinities and
/// NaNs) 0x7fff of the sign; a magnitude below 2^-14 first truncated to 2^-37, then every
/// value to the half's step with ties to even, exponent 31 a number
#[test]
fn one_by_one_halves_are_ties_to_even_for_every_float() {
    assert_eq!(all_u32(|x| {
        let i = x & 0x7fff_ffff;
        let r = f16_one(x);
        if i > 0x47ff_efff {
            return r == sign(x) | 0x7fff;
        }
        let (m, e) = exact(i);
        let (m, e) = pre(m, e, Ties::Trunc);
        r == with_sign(x, half_of(m, e, Ties::Even))
    }), None);
}

/// D3DX9.F16 (x64 AuthenticAMD's 4 at a time): a number clamped to +-131008 (infinities so),
/// a magnitude below 2^-14 rounded first to 2^-37 with ties to even (the f32 product with
/// 2^-112), then to the half's step with ties to even; a NaN quiet (mantissa m | 2^22) as
/// 0x7c00 plus m 2^-13 rounded with ties to even, its carry past 0x7fff leaving the sign alone
#[test]
fn amd_halves_are_ties_to_even_for_every_float() {
    assert_eq!(all_u32(|x| {
        let i = x & 0x7fff_ffff;
        let r = f16_x64(x);
        if i > 0x7f80_0000 {
            let q = steps(u64::from(i & 0x7f_ffff | 0x40_0000), 0, 13, Ties::Even);
            return r == with_sign(x, 0x7c00 + q);
        }
        let (m, e) = exact(i.min(0x47ff_e000));
        let (m, e) = pre(m, e, Ties::Even);
        r == with_sign(x, half_of(m, e, Ties::Even))
    }), None);
}

/// D3DX9.F16 (x64 AuthenticAMD's rest): a NaN as -131008; a number clamped to
/// +-131008 and v rounded as the f32 sum 8193 v less 8192 v gives it (the sum rounded to 24
/// bits with ties to even: the half's step, twice it where 8193 v passes the binade), a
/// magnitude below 2^-14 then rounded to 2^-37 with ties to even, the half's bits truncated
#[test]
fn amd_rest_is_the_sum_trick_for_every_float() {
    assert_eq!(all_u32(|x| {
        let r = f16_x64_tail(x);
        let x = if x & 0x7fff_ffff > 0x7f80_0000 { 0xc7ff_e000 } else { x };
        let i = x & 0x7fff_ffff;
        let (m, e) = exact(i.min(0x47ff_e000));
        if m == 0 {
            return r == sign(x);
        }
        // (8193 m 2^e rounded to 24 bits at step 2^t, less 8192 m 2^e, in steps of 2^(t - 1):
        // the sum has at most 38 bits, so t - e is at most 14)
        let s = m * 8193;
        let t = binade(s, e) - 23;
        let c = if t >= e { (m << 14) >> (t - e) } else { (m << 14) << (e - t) };
        let v = 2 * steps(s, e, t, Ties::Even) - c;
        let (v, ve) = pre(v, t - 1, Ties::Even);
        r == with_sign(x, half_of(v, ve, Ties::Trunc))
    }), None);
}

/// D3DX9.F32: every half as its value, exponent 31 a number like the others (no infinities or
/// NaNs), the sign kept (0x8000 is -0)
#[test]
fn every_half_is_its_value() {
    for h in 0..=u16::MAX {
        let (e, m) = (u32::from(h >> 10 & 31), u32::from(h & 1023));
        let mag = if e == 0 { f32::from_bits(0x3380_0000) * f32::from(h & 1023) } else { f32::from_bits((e + 112) << 23 | m << 13) };
        let want = if h & 0x8000 != 0 { -mag } else { mag };
        assert_eq!(f16_to_f32(h).to_bits(), want.to_bits(), "{h:#06x}");
    }
}

/// D3DX9.F16.CPU: the routine each place of an array goes through, for every length up to 64 on
/// x86 and on x64 with an Intel, an AMD and another vendor's cpuid: x86's and GenuineIntel's
/// 8 at a time as f16_sse and the rest as f16_one, any other vendor's on x64 4 at a time as
/// f16_x64 and the rest as f16_x64_tail. A NaN tells the four apart (7c00, 7fff, 7e00, ffff);
/// the arrays mix it with a number to show that the place, not the value, picks the routine.
#[test]
fn the_routine_is_the_place_and_the_processor() {
    use kikizake::{case, model};
    let nan = 0x7fc0_0000u32;
    assert_eq!([f16_sse(nan), f16_one(nan), f16_x64(nan), f16_x64_tail(nan)], [0x7c00, 0x7fff, 0x7e00, 0xffff]);
    let runs: [(u32, &str); 4] = [(32, "AuthenticAMD"), (64, "GenuineIntel"), (64, "AuthenticAMD"), (64, "HygonGenuine")];
    for (arch, vendor) in runs {
        model::set_arch(arch);
        model::set_cpu(Some(format!("cpu {vendor} fefa3203 1f8bfbff f1bf2fb9")));
        let amd = arch == 64 && vendor != "GenuineIntel";
        let step = if amd { 4 } else { 8 };
        for len in 0..=64usize {
            let w: Vec<u32> = (0..len).map(|i| if i % 3 == 2 { 0x3300_0401 } else { nan }).collect();
            let text = format!("case c\nx9f16{}\nend\n", w.iter().map(|x| format!(" {x:#x}")).collect::<String>());
            let out = model::run_case(&case::parse_cases(&text)[0]).unwrap();
            let bulk = len - len % step;
            let want: String = w
                .iter()
                .enumerate()
                .map(|(i, &x)| {
                    let h = match (amd, i < bulk) {
                        (false, true) => f16_sse(x),
                        (false, false) => f16_one(x),
                        (true, true) => f16_x64(x),
                        (true, false) => f16_x64_tail(x),
                    };
                    format!(" {h:04x}")
                })
                .collect();
            assert_eq!(out, [format!("f16{want}")], "{arch} {vendor} {len}");
        }
    }
    model::set_arch(32);
    model::set_cpu(None);
}
