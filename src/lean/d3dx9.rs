//! D3DX's DXT encoder's mirrors (spec/lean/Kikizake/D3dx9/DxtEnc.lean).
use super::{arg, Domain, Mirror};
use crate::model::dxtenc::{encode_alpha4, encode_alpha8, encode_bc1, Below, Real, Steps, X87};

const DXTENC: &str = "spec/lean/Kikizake/D3dx9/DxtEnc.lean";

/// a channel in 0..1 as the encoder takes it (sources clamped to 0..1, a NaN as 1): an 8-bit
/// read k * (the f32 nearest 1 / 255) one time in four, 0 or 1 one in four, else any binary32
/// in 0..1
fn unit(rng: &mut crate::rng::Sample64) -> i64 {
    match rng.range(0, 3) {
        0 => {
            let k = u8::try_from(rng.range(0, 255)).unwrap_or(0);
            i64::from((f32::from(k) * (1.0f32 / 255.0)).min(1.0).to_bits())
        }
        1 => i64::from(if rng.chance(2) { 0 } else { 1.0f32.to_bits() }),
        _ => rng.range(0, i64::from(1.0f32.to_bits())),
    }
}

/// `n` blocks of 16 (r, g, b, a) channels in 0..1 after `head`: half of them each channel on its
/// own (unit), half of 2 to 4 colours (unit's) each texel one of them
fn blocks(head: &[i64], n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    for _ in 0..n {
        let mut v = head.to_vec();
        if rng.chance(2) {
            v.extend((0..64).map(|_| unit(&mut rng)));
        } else {
            let k = rng.range(2, 4);
            let cs: Vec<[i64; 4]> = (0..k).map(|_| std::array::from_fn(|_| unit(&mut rng))).collect();
            for _ in 0..16 {
                v.extend(rng.pick(&cs));
            }
        }
        each(&v);
    }
}

/// the block of the arguments from `at`, in the build's float
fn px<F: Real>(a: &[i64], at: usize, of: fn(f32) -> F) -> Option<[[F; 4]; 16]> {
    if a.len() != at.wrapping_add(64) {
        return None;
    }
    let mut p = [[of(0.0); 4]; 16];
    for (i, t) in p.iter_mut().enumerate() {
        for (k, c) in t.iter_mut().enumerate() {
            *c = of(f32::from_bits(arg(a, at.wrapping_add(i.wrapping_mul(4)).wrapping_add(k))?));
        }
    }
    Some(p)
}

fn bytes(b: &[u8]) -> Vec<i64> {
    b.iter().map(|&x| i64::from(x)).collect()
}

/// a block's answer: its bytes, or the fault (-1, x64) or the unknown read (-2, x86)
fn bc1(r: Result<[u8; 8], Below>) -> Vec<i64> {
    match r {
        Ok(b) => bytes(&b),
        Err(Below::Fault) => vec![-1],
        Err(Below::Unknown) => vec![-2],
    }
}

pub const MIRRORS: &[Mirror] = &[
    Mirror {
        name: "x9.bc1",
        lean: DXTENC,
        domain: Domain::Sampled("20,000 blocks, 5,000 on each build in four colours and in three: 16 texels of channels in 0..1 (8-bit steps, 0 and 1, any binary32), half of them of 2 to 4 colours"),
        inputs: |each| {
            for b in 0..2 {
                for t in 0..2 {
                    blocks(&[b, t], 5_000, 0xd3d0_0001u32.wrapping_add(u32::try_from(b.wrapping_mul(2).wrapping_add(t)).unwrap_or(0)), each);
                }
            }
        },
        model: |a| {
            let steps = match arg::<u8>(a, 1)? {
                0 => Steps::Four,
                1 => Steps::Three,
                _ => return None,
            };
            match arg::<u8>(a, 0)? {
                0 => Some(bc1(encode_bc1(&px(a, 2, |v| v)?, steps))),
                1 => Some(bc1(encode_bc1(&px(a, 2, X87::of)?, steps))),
                _ => None,
            }
        },
    },
    Mirror {
        name: "x9.alpha4",
        lean: DXTENC,
        domain: Domain::Sampled("10,000 blocks, 5,000 on each build: 16 texels of channels in 0..1 (8-bit steps, 0 and 1, any binary32), half of them of 2 to 4 colours"),
        inputs: |each| {
            for b in 0..2 {
                blocks(&[b], 5_000, 0xd3d0_0011u32.wrapping_add(u32::try_from(b).unwrap_or(0)), each);
            }
        },
        model: |a| match arg::<u8>(a, 0)? {
            0 => Some(bytes(&encode_alpha4(&px(a, 1, |v| v)?))),
            1 => Some(bytes(&encode_alpha4(&px(a, 1, X87::of)?))),
            _ => None,
        },
    },
    Mirror {
        name: "x9.alpha8",
        lean: DXTENC,
        domain: Domain::Sampled("10,000 blocks, 5,000 on each build: 16 texels of channels in 0..1 (8-bit steps, 0 and 1, any binary32), half of them of 2 to 4 colours"),
        inputs: |each| {
            for b in 0..2 {
                blocks(&[b], 5_000, 0xd3d0_0021u32.wrapping_add(u32::try_from(b).unwrap_or(0)), each);
            }
        },
        model: |a| match arg::<u8>(a, 0)? {
            0 => Some(bytes(&encode_alpha8(&px(a, 1, |v| v)?))),
            1 => Some(bytes(&encode_alpha8(&px(a, 1, X87::of)?))),
            _ => None,
        },
    },
];
