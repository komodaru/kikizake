//! DXGI's mirrors (spec/lean/Kikizake/Dxgi/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d12::{self, Present1};

const ARGS: &str = "spec/lean/Kikizake/Dxgi/Args.lean";
const SMALL_FLOAT: &str = "spec/lean/Kikizake/Dxgi/SmallFloat.lean";
const SMALL_FORMATS: [(u32, bool); 3] = [(10, true), (6, false), (5, false)];
const SRGB: &str = "spec/lean/Kikizake/Dxgi/Srgb.lean";

/// the description words of a chain CreateSwapChainForHwnd makes (0 by 0, R8G8B8A8_UNORM, one
/// sample, render target output, 2 buffers, FLIP_DISCARD)
const MADE: [i64; 12] = [0, 0, 0x57, 0, 1, 0, 0x20, 2, 0, 4, 0, 0];
/// each word's values swept with the others as MADE
const SWEEP: [(i64, i64); 12] = [(0, 300), (0, 300), (0, 200), (0, 3), (0, 9), (0, 9), (0, 0x200), (0, 40), (0, 5), (0, 8), (0, 8), (0, 0x4000)];
const CHAIN_FLAGS: [i64; 4] = [0, 0x40, 0x800, 0x840];

/// the entry `i` of a table of choices (0 past its end)
fn pick(t: &[i64], i: i64) -> i64 {
    usize::try_from(i).ok().and_then(|i| t.get(i)).copied().unwrap_or(0)
}

fn bit(a: &[i64], i: usize) -> Option<bool> {
    arg::<u8>(a, i).map(|v| v != 0)
}

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "dxgi.assoc",
        lean: ARGS,
        domain: Domain::Sampled("every flags value up to 0xffff, each bit alone, and 20,000 of the 32-bit values"),
        inputs: |each| {
            every(&[(0, 0xffff)], each);
            for b in 0..32 {
                each(&[1i64 << b]);
            }
            sampled(&[(0, 0xffff_ffff)], 20_000, 0xa550_c001, each);
        },
        model: |a| one(d3d12::assoc_hr(arg(a, 0)?).0),
    },
    Mirror {
        name: "dxgi.feature",
        lean: ARGS,
        domain: Domain::Every("features 0..64, sizes 0..64"),
        inputs: |each| every(&[(0, 64), (0, 64)], each),
        model: |a| one(d3d12::feature_args_taken(arg(a, 0)?, arg(a, 1)?)),
    },
    Mirror {
        name: "dxgi.hdr",
        lean: ARGS,
        domain: Domain::Every("types 0..8, sizes 0..128, NULL data or not"),
        inputs: |each| every(&[(0, 8), (0, 128), (0, 1)], each),
        model: |a| one(d3d12::hdr_taken(arg(a, 0)?, arg(a, 1)?, bit(a, 2)?)),
    },
    Mirror {
        name: "dxgi.spaces",
        lean: ARGS,
        domain: Domain::Every("formats 0..255"),
        inputs: |each| every(&[(0, 255)], each),
        model: |a| Some(d3d12::color_spaces(arg(a, 0)?).iter().map(|&c| i64::from(c)).collect()),
    },
    Mirror {
        name: "dxgi.chain",
        lean: ARGS,
        domain: Domain::Sampled("each description word over its range with the others a made chain's, x86 and x86-64; sizes around the x86 memory limit; 40,000 descriptions of words near the taken ones"),
        inputs: |each| {
            for arch in [32, 64] {
                for (i, r) in SWEEP.iter().enumerate() {
                    for v in r.0..=r.1 {
                        let mut d = MADE.to_vec();
                        if let Some(x) = d.get_mut(i) {
                            *x = v;
                        }
                        d.push(arch);
                        each(&d);
                    }
                }
                for (w, h, n) in [(16384, 14250, 2), (16384, 14249, 2), (16384, 14250, 1), (16385, 16385, 2), (8192, 8192, 16), (8192, 8192, 15)] {
                    each(&[w, h, 0x57, 0, 1, 0, 0x20, n, 0, 4, 0, 0, arch]);
                }
            }
            sampled(&[(0, 3), (0, 3), (9, 90), (0, 1), (0, 2), (0, 1), (0, 0x90), (1, 17), (0, 2), (2, 5), (0, 4), (0, 0x2840), (32, 33)], 40_000, 0xc4a1_0001, &mut |v| {
                let mut d = v.to_vec();
                if let Some(a) = d.get_mut(12) {
                    *a = if *a == 32 { 32 } else { 64 };
                }
                each(&d);
            });
        },
        model: |a| {
            let w: [u32; 12] = std::array::from_fn(|i| arg(a, i).unwrap_or(u32::MAX));
            if w.contains(&u32::MAX) {
                return None;
            }
            let was = crate::model::arch();
            crate::model::set_arch(arg(a, 12)?);
            let r = d3d12::chain_desc(&w);
            crate::model::set_arch(was);
            Some(match r {
                Ok(d) => std::iter::once(0).chain(d.iter().map(|&v| i64::from(v))).collect(),
                Err(hr) => vec![1, i64::from(hr.0)],
            })
        },
    },
    Mirror {
        name: "dxgi.present",
        lean: ARGS,
        domain: Domain::Every("intervals 0..5, flags 0..0x3ff and each higher bit alone, chains with and without FRAME_LATENCY_WAITABLE_OBJECT and ALLOW_TEARING, presented or not"),
        inputs: |each| {
            every(&[(0, 5), (0, 0x3ff), (0, 3), (0, 1)], &mut |v| {
                if let [s, f, c, fr] = *v {
                    each(&[s, f, pick(&CHAIN_FLAGS, c), fr]);
                }
            });
            for b in 10..32 {
                every(&[(0, 5), (0, 3), (0, 1)], &mut |v| {
                    if let [s, c, fr] = *v {
                        each(&[s, 1i64 << b, pick(&CHAIN_FLAGS, c), fr]);
                    }
                });
            }
        },
        model: |a| one(d3d12::present_refused(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, bit(a, 3)?)),
    },
    Mirror {
        name: "dxgi.present1",
        lean: ARGS,
        domain: Domain::Sampled("intervals 0..5, the flags, both flip effects, chains with and without ALLOW_TEARING, 64 and 100 wide buffers, a scroll or not, 0..3 dirty rects, corners -2..102, offsets -10..10, presented or not: 200,000 calls, and each full-buffer rect"),
        inputs: |each| {
            const FLAGS: [i64; 8] = [0, 1, 2, 3, 4, 0x100, 0x200, 0x400];
            for (bw, bh) in [(64, 64), (100, 50)] {
                every(&[(0, 1), (0, 1), (0, 1), (0, 7)], &mut |v| {
                    if let [fr, e, sc, f] = *v {
                        let f = pick(&FLAGS, f);
                        each(&[0, f, fr, 3 + e, 0, bw, bh, sc, 0, 0, bw, bh, 0, 0, 0, 0, bw, bh]);
                    }
                });
            }
            sampled(&[(0, 5), (0, 7), (0, 1), (3, 4), (0, 1), (0, 1), (0, 1), (-2, 102), (-2, 102), (-2, 102), (-2, 102), (-10, 10), (-10, 10), (0, 3)], 200_000, 0xd1a7_0001, &mut |v| {
                if let [s, f, fr, e, c, size, sc, l, t, r, b, dx, dy, n] = *v {
                    let f = pick(&FLAGS, f);
                    let (bw, bh) = if size == 0 { (64, 64) } else { (100, 50) };
                    let mut call = vec![s, f, fr, e, c * 0x800, bw, bh, sc, l, t, r, b, dx, dy];
                    // (the dirty rects: the scroll rectangle moved by k)
                    for k in 0..n {
                        call.extend([l + 7 * k - 3, t - 5 * k, r + 11 * k - 20, b + 3 * k]);
                    }
                    each(&call);
                }
            });
        },
        model: |a| {
            let r = |i: usize| -> Option<[i32; 4]> { Some([arg(a, i)?, arg(a, i + 1)?, arg(a, i + 2)?, arg(a, i + 3)?]) };
            let rects = a.len().checked_sub(14)?;
            if rects % 4 != 0 {
                return None;
            }
            let dirty: Vec<[i32; 4]> = (0..rects / 4).map(|k| r(14 + 4 * k)).collect::<Option<_>>()?;
            let scroll = if bit(a, 7)? { Some((r(8)?, arg(a, 12)?, arg(a, 13)?)) } else { None };
            let p = Present1 { sync: arg(a, 0)?, flags: arg(a, 1)?, scroll, dirty: &dirty };
            let mut d = [0u32; 12];
            (d[0], d[1], d[9], d[11]) = (arg(a, 5)?, arg(a, 6)?, arg(a, 3)?, arg(a, 4)?);
            one(d3d12::present1_refused(&p, &d, bit(a, 2)?))
        },
    },
    Mirror {
        name: "dxgi.smallDecode",
        lean: SMALL_FLOAT,
        domain: Domain::Every("every signed binary16 and unsigned 11/10-bit floating-point code"),
        inputs: |each| {
            for (kind, &(mantissa, signed)) in SMALL_FORMATS.iter().enumerate() {
                let codes = 1u32 << (5 + mantissa + u32::from(signed));
                for bits in 0..codes {
                    each(&[kind as i64, i64::from(bits)]);
                }
            }
        },
        model: |a| {
            let &(mantissa, signed) = SMALL_FORMATS.get(arg::<usize>(a, 0)?)?;
            one(crate::model::dxgiconv::from_small_float(arg(a, 1)?, 5, mantissa, signed).to_bits())
        },
    },
    Mirror {
        name: "dxgi.smallEncode",
        lean: SMALL_FLOAT,
        domain: Domain::Sampled("binary32 at and on either side of every positive small-float code, both signs, all exponent boundaries, and 20,000 full bit patterns per format"),
        inputs: |each| {
            use crate::model::dxgiconv::from_small_float;
            for (kind, &(mantissa, signed)) in SMALL_FORMATS.iter().enumerate() {
                let kind = kind as i64;
                for code in 0..1u32 << (5 + mantissa) {
                    let pivot = from_small_float(code, 5, mantissa, signed).to_bits();
                    for bits in [pivot.saturating_sub(1), pivot, pivot + 1] {
                        each(&[kind, i64::from(bits)]);
                        each(&[kind, i64::from(bits | 0x8000_0000)]);
                    }
                }
                for exponent in 0u32..256 {
                    for fraction in [0, 1, 0x0000_0fff, 0x0040_0000, 0x007f_ffff] {
                        let bits = (exponent << 23) | fraction;
                        each(&[kind, i64::from(bits)]);
                        each(&[kind, i64::from(bits | 0x8000_0000)]);
                    }
                }
                sampled(&[(0, 0xffff_ffff)], 20_000, 0x5f10_a732, &mut |a| each(&[kind, a[0]]));
            }
        },
        model: |a| {
            let &(mantissa, signed) = SMALL_FORMATS.get(arg::<usize>(a, 0)?)?;
            let value = f32::from_bits(arg(a, 1)?);
            one(crate::model::dxgiconv::to_small_float(value, 5, mantissa, signed))
        },
    },
    Mirror {
        name: "dxgi.srgbDecode",
        lean: SRGB,
        domain: Domain::Every("every 8-bit sRGB code"),
        inputs: |each| every(&[(0, 255)], each),
        model: |a| one(crate::model::dxgiconv::srgb_read(arg(a, 0)?).to_bits()),
    },
    Mirror {
        name: "dxgi.srgbCurve",
        lean: SRGB,
        domain: Domain::Every("every UNORM12 input to the sRGB write curve"),
        inputs: |each| every(&[(0, 4095)], each),
        model: |a| {
            use crate::model::dxgiconv::{srgb8, Unorm12};
            let code: u16 = arg(a, 0)?;
            let value = f32::from(code) / f32::from(Unorm12::MAX);
            (Unorm12::of(value).get() == code).then(|| vec![i64::from(srgb8(value))])
        },
    },
    Mirror {
        name: "dxgi.srgbEncode",
        lean: SRGB,
        domain: Domain::Sampled("both sides of each UNORM12 midpoint, every binary32 exponent boundary, and 20,000 sampled binary32 words"),
        inputs: |each| {
            for q in 0..4095u16 {
                let bits = ((f32::from(q) + 0.5) / 4095.0).to_bits();
                for b in [bits - 1, bits, bits + 1] {
                    each(&[i64::from(b)]);
                }
            }
            for exponent in 0..256u32 {
                for fraction in [0, 1, 0x7f_ffff] {
                    let bits = (exponent << 23) | fraction;
                    each(&[i64::from(bits)]);
                    each(&[i64::from(bits | 0x8000_0000)]);
                }
            }
            sampled(&[(0, 0xffff_ffff)], 20_000, 0x05e2_6b12, each);
        },
        model: |a| one(crate::model::dxgiconv::srgb8(f32::from_bits(arg(a, 0)?))),
    },
];
