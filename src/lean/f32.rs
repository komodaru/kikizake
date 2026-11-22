//! binary32 and binary64 arithmetic's mirrors (spec/lean/Kikizake/F32.lean, Float.lean): Rust's
//! f32 and f64 operators on bits (a binary64 as its two 32-bit halves).
use super::{arg, arg_f64 as g, every, f64_halves, one, sampled, Domain, Mirror};

const F32: &str = "spec/lean/Kikizake/F32.lean";
const U32: i64 = 0xffff_ffff;

/// a NaN as the one quiet NaN
fn canon(x: f32) -> i64 {
    if x.is_nan() { 0x7fc0_0000 } else { i64::from(x.to_bits()) }
}

fn f(a: &[i64], i: usize) -> Option<f32> {
    arg::<u32>(a, i).map(f32::from_bits)
}

/// the special and boundary values: zeros, the smallest subnormals, the largest subnormal, the
/// smallest normal, 1 and its neighbours, the largest finite value, infinity, a NaN, each signed
const EDGES: [u32; 16] = [0, 1, 2, 0x007f_ffff, 0x0080_0000, 0x0080_0001, 0x3f7f_ffff, 0x3f80_0000, 0x3f80_0001, 0x4000_0000, 0x3b80_8081, 0x437f_0000, 0x7f7f_ffff, 0x7f80_0000, 0x7fc0_0000, 0x3eff_ffff];

fn pairs(seed: u32, each: &mut dyn FnMut(&[i64])) {
    for &x in &EDGES {
        for &y in &EDGES {
            for (sx, sy) in [(0u32, 0u32), (1 << 31, 0), (0, 1 << 31), (1 << 31, 1 << 31)] {
                each(&[i64::from(x | sx), i64::from(y | sy)]);
            }
        }
    }
    sampled(&[(0, U32), (0, U32)], 200_000, seed, each);
    // (values near each other, where sums cancel and products land in the subnormals)
    sampled(&[(0x3000_0000, 0x4800_0000), (0x3000_0000, 0x4800_0000)], 100_000, seed ^ 0x5555, each);
    sampled(&[(0, 0x0400_0000), (0x3000_0000, 0x4000_0000)], 50_000, seed ^ 0xaaaa, each);
}

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "f32.add",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 32-bit patterns, 150,000 near each other or subnormal"),
        inputs: |each| pairs(0xadd0_f032, each),
        model: |a| one(canon(f(a, 0)? + f(a, 1)?)),
    },
    Mirror {
        name: "f32.sub",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 32-bit patterns, 150,000 near each other or subnormal"),
        inputs: |each| pairs(0x50b0_f032, each),
        model: |a| one(canon(f(a, 0)? - f(a, 1)?)),
    },
    Mirror {
        name: "f32.mul",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 32-bit patterns, 150,000 near each other or subnormal"),
        inputs: |each| pairs(0x3010_f032, each),
        model: |a| one(canon(f(a, 0)? * f(a, 1)?)),
    },
    Mirror {
        name: "f32.div",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 32-bit patterns, 150,000 near each other or subnormal"),
        inputs: |each| pairs(0xd170_f032, each),
        model: |a| one(canon(f(a, 0)? / f(a, 1)?)),
    },
    Mirror {
        name: "f32.sqrt",
        lean: F32,
        domain: Domain::Sampled("16 signed boundary values, the squares of 0..2^12 and their neighbours, and 300,000 32-bit patterns"),
        inputs: |each| {
            for &x in &EDGES {
                each(&[i64::from(x)]);
                each(&[i64::from(x | 1 << 31)]);
            }
            for k in 0u16..=4096 {
                let b = (f32::from(k) * f32::from(k)).to_bits();
                for v in [b.saturating_sub(1), b, b.saturating_add(1)] {
                    each(&[i64::from(v)]);
                }
            }
            sampled(&[(0, U32)], 300_000, 0x5a7f_f032, each);
        },
        model: |a| one(canon(f(a, 0)?.sqrt())),
    },
    Mirror {
        name: "f32.ofNat",
        lean: F32,
        domain: Domain::Sampled("every number to 2^20, and 200,000 32-bit numbers (against Rust's correctly rounded parse)"),
        inputs: |each| {
            every(&[(0, 1 << 20)], each);
            sampled(&[(0, U32)], 200_000, 0x0fa7_f032, each);
        },
        model: |a| {
            // (the nearest f32, halves to even, as `n as f32` gives it: Rust's parse rounds so too)
            let n: u32 = arg(a, 0)?;
            let v: f32 = n.to_string().parse().ok()?;
            one(i64::from(v.to_bits()))
        },
    },
    Mirror {
        name: "f32.clamp",
        lean: F32,
        domain: Domain::Sampled("16 signed boundary values against 0..1, and 200,000 32-bit patterns"),
        inputs: |each| {
            for &x in &EDGES {
                for s in [0, 1u32 << 31] {
                    each(&[i64::from(x | s), 0, 0x3f80_0000]);
                }
            }
            sampled(&[(0, U32)], 200_000, 0xc1a3_f032, &mut |v| {
                if let [x] = *v {
                    each(&[x, 0, 0x3f80_0000]);
                }
            });
        },
        model: |a| one(i64::from(f(a, 0)?.clamp(f(a, 1)?, f(a, 2)?).to_bits())),
    },
    Mirror {
        name: "f64.add",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 64-bit patterns, 100,000 near each other"),
        inputs: |each| pairs64(0xadd0_f064, each),
        model: |a| Some(f64_halves(g(a, 0)? + g(a, 2)?)),
    },
    Mirror {
        name: "f64.sub",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 64-bit patterns, 100,000 near each other"),
        inputs: |each| pairs64(0x50b0_f064, each),
        model: |a| Some(f64_halves(g(a, 0)? - g(a, 2)?)),
    },
    Mirror {
        name: "f64.mul",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 64-bit patterns, 100,000 near each other"),
        inputs: |each| pairs64(0x3010_f064, each),
        model: |a| Some(f64_halves(g(a, 0)? * g(a, 2)?)),
    },
    Mirror {
        name: "f64.div",
        lean: F32,
        domain: Domain::Sampled("every pair of 16 signed boundary values, 200,000 pairs of 64-bit patterns, 100,000 near each other"),
        inputs: |each| pairs64(0xd170_f064, each),
        model: |a| Some(f64_halves(g(a, 0)? / g(a, 2)?)),
    },
    Mirror {
        name: "f64.sqrt",
        lean: F32,
        domain: Domain::Sampled("12 boundary values' high halves (low half 0 or 1), signed, and 200,000 64-bit patterns"),
        inputs: |each| {
            for &x in &EDGES64 {
                for s in [0, 1u32 << 31] {
                    for lo in [0, 1] {
                        each(&[i64::from(x | s), lo]);
                    }
                }
            }
            sampled(&[(0, U32), (0, U32)], 200_000, 0x5a7f_f064, each);
        },
        model: |a| Some(f64_halves(g(a, 0)?.sqrt())),
    },
    Mirror {
        name: "f64.ofF32",
        lean: F32,
        domain: Domain::Sampled("16 signed boundary values and 300,000 32-bit patterns"),
        inputs: |each| {
            for &x in &EDGES {
                each(&[i64::from(x)]);
                each(&[i64::from(x | 1 << 31)]);
            }
            sampled(&[(0, U32)], 300_000, 0x0f32_f064, each);
        },
        model: |a| Some(f64_halves(f64::from(f(a, 0)?))),
    },
    Mirror {
        name: "f64.toF32",
        lean: F32,
        domain: Domain::Sampled("200,000 64-bit patterns and 200,000 near binary32's range and its halfway points, against the exact decimal parsed as f32"),
        inputs: |each| {
            sampled(&[(0, U32), (0, U32)], 200_000, 0x7f32_f064, each);
            sampled(&[(0x3600_0000, 0x47f0_0000), (0, U32)], 200_000, 0x7f33_f064, each);
            sampled(&[(0x3800_0000, 0x3801_0000), (0x1000_0000, 0x1000_0000)], 1, 0x7f34_f064, each);
        },
        model: |a| {
            // (`as f32`, the nearest binary32: the binary64's exact decimal expansion, which Rust
            // prints exactly to 1,100 digits, parsed with Rust's correctly rounded parse)
            let v: f32 = format!("{:.1100e}", g(a, 0)?).parse().ok()?;
            one(canon(v))
        },
    },
];

/// the binary64 boundary values' high halves (the low half 0 or 1)
const EDGES64: [u32; 12] = [0, 1, 0x000f_ffff, 0x0010_0000, 0x3fef_ffff, 0x3ff0_0000, 0x4000_0000, 0x3fe0_0000, 0x7fef_ffff, 0x7ff0_0000, 0x7ff8_0000, 0x3f70_1010];

fn pairs64(seed: u32, each: &mut dyn FnMut(&[i64])) {
    for &x in &EDGES64 {
        for &y in &EDGES64 {
            for (sx, sy) in [(0u32, 0u32), (1 << 31, 0), (0, 1 << 31), (1 << 31, 1 << 31)] {
                for lo in [0, 1] {
                    each(&[i64::from(x | sx), lo, i64::from(y | sy), if lo == 0 { 1 } else { 0 }]);
                }
            }
        }
    }
    sampled(&[(0, U32), (0, U32), (0, U32), (0, U32)], 200_000, seed, each);
    sampled(&[(0x3e00_0000, 0x4100_0000), (0, U32), (0x3e00_0000, 0x4100_0000), (0, U32)], 100_000, seed ^ 0x5555, each);
}
