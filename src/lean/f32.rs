//! binary32 arithmetic's mirrors (spec/lean/Kikizake/F32.lean): Rust's f32 operators on bits.
use super::{arg, every, one, sampled, Domain, Mirror};

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
];
