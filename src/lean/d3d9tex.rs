//! Direct3D 9's texture, stage chain and pixel mirrors (spec/lean/Kikizake/D3d9/Tex.lean and
//! the files after it).
use super::{arg, arg_f64, every, f64_halves, sampled, Domain, Mirror};
use crate::model::d3d9draw::{self, AlphaRule, BlendStates, DrawGate, StageDesc, Step, TexFilter};

const U32: i64 = 0xffff_ffff;
const TEX: &str = "spec/lean/Kikizake/D3d9/Tex.lean";
const TEXEL: &str = "spec/lean/Kikizake/D3d9/Texel.lean";
const CHAIN: &str = "spec/lean/Kikizake/D3d9/Chain.lean";
const BLEND: &str = "spec/lean/Kikizake/D3d9/Blend.lean";
const FILTER: &str = "spec/lean/Kikizake/D3d9/Filter.lean";

/// texel coordinates as the samplers make them: u of k / 4096 in -2..2 times widths 1 .. 256,
/// the points 1/128 and 1/64 apart either side of, and on, by 0 .. 2 ulps each way, and
/// 100,000 binary64s of any bits
fn coords(each: &mut dyn FnMut(&[i64])) {
    for w in [1.0f64, 2.0, 3.0, 4.0, 5.0, 7.0, 8.0, 16.0, 256.0] {
        for k in -8192..=8192 {
            each(&f64_halves(f64::from(k) / 4096.0 * w));
        }
    }
    for k in -1024..=1024 {
        let x = f64::from(k) / 128.0;
        for d in -2i64..=2 {
            let b = x.to_bits().cast_signed().saturating_add(d).cast_unsigned();
            each(&f64_halves(f64::from_bits(b)));
        }
    }
    let mut rng = crate::rng::Sample64::new(0xf11e_d009);
    for _ in 0..100_000 {
        let b = rng.next();
        each(&f64_halves(f64::from_bits(b)));
    }
}
/// COLOROPs and ALPHAOPs of the plan: DISABLE, SELECTARG1, MODULATE, PREMODULATE, the bump mapping
/// ones, DOTPRODUCT3, one past LERP
const PLAN_OPS: [i64; 8] = [1, 2, 4, 17, 22, 23, 24, 27];

/// filter values: every value of the enumerations, past them, and the 32-bit end
const FILTERS: [i64; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 0x100, 0xffff_ffff];
/// BLENDOP and address mode values: the enumeration, past it, and the 32-bit end
const MODES: [i64; 9] = [0, 1, 2, 3, 4, 5, 6, 7, 0xffff_ffff];

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d9.filter",
        lean: TEX,
        domain: Domain::Every("MAGFILTER and MINFILTER 0..7, 0x100 and 0xffffffff, both runtimes"),
        inputs: |each| {
            for mag in FILTERS {
                for min in FILTERS {
                    for v8 in 0..2 {
                        each(&[mag, min, v8]);
                    }
                }
            }
        },
        model: |a| {
            let (mag, min, v8) = (arg(a, 0)?, arg(a, 1)?, arg::<u8>(a, 2)? != 0);
            let f = match d3d9draw::sample_filter(mag, min, v8) {
                TexFilter::None => 0,
                TexFilter::Point => 1,
                TexFilter::Linear => 2,
                TexFilter::Anisotropic => 3,
                TexFilter::Quad => 4,
                TexFilter::Other => 5,
            };
            Some(vec![f, i64::from(d3d9draw::d3d10_setup(mag, min, v8))])
        },
    },
    Mirror {
        name: "d3d9.gate",
        lean: TEX,
        domain: Domain::Every("x86 and x64, BLENDOP 0..7 and 0xffffffff, palette set or not, a palette texture or not, 0..3 address modes each 0..7 or 0xffffffff"),
        inputs: |each| {
            let mut lists: Vec<Vec<i64>> = vec![vec![]];
            let last = i64::try_from(MODES.len()).unwrap_or(1).saturating_sub(1);
            for len in 1..=3 {
                every(&vec![(0, last); len], &mut |idx| {
                    lists.push(idx.iter().map(|&i| usize::try_from(i).ok().and_then(|i| MODES.get(i)).copied().unwrap_or(0)).collect());
                });
            }
            every(&[(0, 1), (0, 8), (0, 1), (0, 1)], &mut |h| {
                let op = MODES.get(usize::try_from(h.get(1).copied().unwrap_or(0)).unwrap_or(0)).copied().unwrap_or(0);
                for l in &lists {
                    let mut w = vec![h.first().copied().unwrap_or(0), op, h.get(2).copied().unwrap_or(0), h.get(3).copied().unwrap_or(0)];
                    w.extend(l);
                    each(&w);
                }
            });
        },
        model: |a| {
            let addrs: Vec<u32> = a.get(4..)?.iter().map(|&v| u32::try_from(v).ok()).collect::<Option<_>>()?;
            Some(match d3d9draw::draw_gate(arg::<u8>(a, 0)? != 0, arg(a, 1)?, &addrs, arg::<u8>(a, 2)? != 0, arg::<u8>(a, 3)? != 0) {
                DrawGate::Draw => vec![0, 0],
                DrawGate::BlendOpX86(op) => vec![1, i64::from(op)],
                DrawGate::AddressX86(m) => vec![2, i64::from(m)],
                DrawGate::NoPalette => vec![3, 0],
            })
        },
    },
    Mirror {
        name: "d3d9.palette",
        lean: TEX,
        domain: Domain::Sampled("P8 and A8P8, every own alpha byte and entry flags, and 200,000 samples of 8-bit entries (formats 0..64)"),
        inputs: |each| {
            for f in [40, 41] {
                every(&[(f, f), (0, 255), (0x5a, 0x5a), (0xa5, 0xa5), (0x3c, 0x3c), (0, 255)], each);
            }
            sampled(&[(0, 64), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255)], 200_000, 0x9a1e_d009, each);
        },
        model: |a| {
            let b = |i: usize| arg::<u8>(a, i);
            Some(d3d9draw::palette_texel(arg(a, 0)?, b(1)?, [b(2)?, b(3)?, b(4)?, b(5)?]).map(i64::from).to_vec())
        },
    },
    Mirror {
        name: "d3d9.stencil2",
        lean: TEX,
        domain: Domain::Every("both runtimes, TWOSIDEDSTENCILMODE 0..3 and 0xffffffff, either winding"),
        inputs: |each| {
            for t in [0, 1, 2, 3, 0xffff_ffff] {
                every(&[(0, 1), (t, t), (0, 1)], each);
            }
        },
        model: |a| Some(d3d9draw::stencil_states(arg::<u8>(a, 0)? != 0, arg(a, 1)?, arg::<u8>(a, 2)? != 0).map(i64::from).to_vec()),
    },
    Mirror {
        name: "d3d9.dxt",
        lean: TEXEL,
        domain: Domain::Sampled("DXT1..5 (and 0, 6, 7) blocks of 8 and 16 bytes: every texel of 3,000 random blocks each, and 300,000 samples of texels 0..16 of blocks of 0..16 bytes"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xd7c1_d009);
            for n in 0..8 {
                for _ in 0..3_000 {
                    let len = if n == 1 { 8 } else { 16 };
                    let b: Vec<i64> = (0..len).map(|_| rng.range(0, 255)).collect();
                    for t in 0..16 {
                        let mut w = vec![n, t];
                        w.extend(&b);
                        each(&w);
                    }
                }
            }
            for _ in 0..300_000 {
                let (n, t, len) = (rng.range(0, 7), rng.range(0, 16), rng.range(0, 16));
                let mut w = vec![n, t];
                w.extend((0..len).map(|_| rng.range(0, 255)));
                each(&w);
            }
        },
        model: |a| {
            let b: Vec<u8> = a.get(2..)?.iter().map(|&v| u8::try_from(v).ok()).collect::<Option<_>>()?;
            Some(d3d9draw::dxt_block_texel(arg(a, 0)?, &b, arg(a, 1)?).map_or(vec![], |v| v.map(i64::from).to_vec()))
        },
    },
    Mirror {
        name: "d3d9.rgba",
        lean: TEXEL,
        domain: Domain::Sampled("the channels of every format the model knows, and every list of 0..3 channel letters of r g b a l x p u v w q, on random binary64 channels (any bits, and k / 255)"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x76ba_d009);
            let mut lists: Vec<Vec<u8>> = (0..=255u32).filter_map(|f| d3d9draw::format(f)).map(|m| m.ch.iter().map(|c| c.c).collect()).collect();
            let letters = b"rgbalxpuvwq";
            for len in 0..=3 {
                every(&vec![(0, 10); len], &mut |idx| {
                    lists.push(idx.iter().filter_map(|&i| usize::try_from(i).ok().and_then(|i| letters.get(i)).copied()).collect());
                });
            }
            for l in &lists {
                for k in 0..4 {
                    let mut w = vec![];
                    for &c in l {
                        w.push(i64::from(c));
                        let v = if k % 2 == 0 { f64::from(u8::try_from(rng.range(0, 255)).unwrap_or(0)) / 255.0 } else { f64::from_bits(u64::from(rng.word()) << 32 | u64::from(rng.word())) };
                        w.extend(f64_halves(v));
                    }
                    each(&w);
                }
            }
        },
        model: |a| {
            let ch: Vec<(u8, f64)> = a.chunks(3).map(|c| Some((u8::try_from(*c.first()?).ok()?, arg_f64(c, 1)?))).collect::<Option<_>>()?;
            Some(d3d9draw::to_rgba(0, &ch).iter().flat_map(|&x| f64_halves(x)).collect())
        },
    },
    Mirror {
        name: "d3d9.level",
        lean: TEXEL,
        domain: Domain::Every("taken or not, DXT or not, 0..16 bytes a pixel (block), sizes 0..40 x 0..40"),
        inputs: |each| every(&[(0, 1), (0, 1), (0, 16), (0, 40), (0, 40)], each),
        model: |a| {
            Some(match d3d9draw::create_level(arg::<u8>(a, 0)? != 0, arg::<u8>(a, 1)? != 0, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?) {
                Ok(b) => vec![0, i64::try_from(b.len()).ok()?],
                Err(hr) => vec![1, i64::from(hr)],
            })
        },
    },
    Mirror {
        name: "d3d9.plan",
        lean: CHAIN,
        domain: Domain::Sampled("every one and two stages of COLOROP DISABLE, SELECTARG1, MODULATE, PREMODULATE, BUMPENVMAP(LUMINANCE), DOTPRODUCT3, 27, ALPHAOP DISABLE, SELECTARG1, PREMODULATE, a texture or not, reading it or not, reading CURRENT or not; 300,000 lists of 0..8 such stages"),
        inputs: |each| {
            let stage = |k: i64| -> [i64; 5] {
                let op = |i: i64| PLAN_OPS.get(usize::try_from(i).unwrap_or(0)).copied().unwrap_or(1);
                [op(k % 8), [1, 2, 17].get(usize::try_from(k / 8 % 3).unwrap_or(0)).copied().unwrap_or(1), k / 24 % 2, k / 48 % 2, k / 96 % 2]
            };
            every(&[(0, 191)], &mut |a| each(&stage(a.first().copied().unwrap_or(0))));
            every(&[(0, 191), (0, 191)], &mut |a| {
                let w: Vec<i64> = a.iter().flat_map(|&k| stage(k)).collect();
                each(&w);
            });
            let mut rng = crate::rng::Sample64::new(0x91a7_d009);
            for _ in 0..300_000 {
                let n = rng.range(0, 8);
                // (mostly enabled stages, so that the chains run long)
                let w: Vec<i64> = (0..n).flat_map(|_| {
                    let mut s = stage(rng.range(0, 191));
                    if s[0] == 1 && rng.range(0, 3) != 0 {
                        s[0] = 4;
                    }
                    s
                }).collect();
                each(&w);
            }
        },
        model: |a| {
            let d: Vec<StageDesc> = a.chunks(5).map(|c| Some(StageDesc { cop: arg(c, 0)?, aop: arg(c, 1)?, has_tex: arg::<u8>(c, 2)? != 0, reads_tex: arg::<u8>(c, 3)? != 0, reads_cur: arg::<u8>(c, 4)? != 0 })).collect::<Option<_>>()?;
            let b = |x: bool| i64::from(x);
            Some(d3d9draw::stage_plan(&d).into_iter().flat_map(|s| match s {
                Step::Run { st, moved, alpha, premod } => [0, i64::from(st), b(moved), match alpha { AlphaRule::Op(n) => i64::from(n), AlphaRule::Keep => -1, AlphaRule::One => -2 }, b(premod)],
                Step::Defer { st, moved } => [1, i64::from(st), b(moved), 0, 0],
                Step::BumpLast { st, moved, from } => [2, i64::from(st), b(moved), from.map_or(-1, i64::from), 0],
                Step::Off { st, moved, s } => [3, i64::from(st), b(moved), i64::from(s), 0],
            }).collect())
        },
    },
    Mirror {
        name: "d3d9.reads",
        lean: CHAIN,
        domain: Domain::Sampled("COLOROPs and ALPHAOPs 0..27 with color arguments of DIFFUSE, CURRENT, TEXTURE and TEXTURE | COMPLEMENT, and 300,000 samples of operations 0..32 and the 32-bit end, arguments 0..0x3f"),
        inputs: |each| {
            let sel = [0, 1, 2, 0x12];
            every(&[(0, 27), (0, 27), (0, 3), (0, 3), (0, 3)], &mut |a| {
                let g = |i: usize| a.get(i).and_then(|&k| usize::try_from(k).ok()).and_then(|k| sel.get(k)).copied().unwrap_or(0);
                each(&[a.first().copied().unwrap_or(0), a.get(1).copied().unwrap_or(0), g(2), g(3), g(4), 2, 0, 2]);
            });
            sampled(&[(0, 32), (0, 32), (0, 0x3f), (0, 0x3f), (0, 0x3f), (0, 0x3f), (0, 0x3f), (0, 0x3f)], 300_000, 0x2ead_d009, each);
            every(&[(0xffff_ffff, 0xffff_ffff), (0, 1), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3)], each);
        },
        model: |a| {
            let g = |i: usize| arg::<u32>(a, i);
            Some(vec![i64::from(d3d9draw::reads_texture(g(0)?, g(1)?, [g(2)?, g(3)?, g(4)?, g(5)?, g(6)?, g(7)?])), i64::from(d3d9draw::reads_current(g(0)?, [g(2)?, g(3)?, g(4)?]))])
        },
    },
    Mirror {
        name: "d3d9.bump",
        lean: CHAIN,
        domain: Domain::Sampled("300,000 samples of coordinates in -4..4, BUMPENVMAT, LSCALE, LOFFSET of f32 in -4..4, (du, dv), L and channels of 8-bit values (k / 255 and signed k / 127), and 100,000 of any f32 bits for each"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xb0b5_d009);
            let f32s = |rng: &mut crate::rng::Sample64, lo: f32, hi: f32| -> f64 { f64::from(lo + (hi - lo) * (f32::from(u16::try_from(rng.range(0, 0xffff)).unwrap_or(0)) / 65535.0)) };
            for _ in 0..300_000 {
                let mut w = vec![];
                for _ in 0..2 {
                    w.extend(f64_halves(f32s(&mut rng, -4.0, 4.0)));
                }
                for _ in 0..4 {
                    w.extend(f64_halves(f32s(&mut rng, -4.0, 4.0)));
                }
                for _ in 0..2 {
                    let k = rng.range(-127, 127);
                    w.extend(f64_halves(f64::from(f32::from(i16::try_from(k).unwrap_or(0)) * (1.0 / 127.0))));
                }
                w.extend(f64_halves(f64::from(f32::from(u8::try_from(rng.range(0, 255)).unwrap_or(0)) / 255.0)));
                for _ in 0..2 {
                    w.extend(f64_halves(f32s(&mut rng, -4.0, 4.0)));
                }
                for _ in 0..2 {
                    w.extend(f64_halves(f64::from(f32::from(u8::try_from(rng.range(0, 255)).unwrap_or(0)) / 255.0)));
                }
                each(&w);
            }
            for _ in 0..100_000 {
                let w: Vec<i64> = (0..13).flat_map(|_| f64_halves(f64::from(f32::from_bits(rng.word())))).collect();
                each(&w);
            }
        },
        model: |a| {
            let f = |i: usize| arg_f64(a, i);
            let (u, v) = d3d9draw::bump_uv((f(0)?, f(2)?), [f(4)?, f(6)?, f(8)?, f(10)?], (f(12)?, f(14)?));
            let l = d3d9draw::bump_luminance(f(16)?, f(18)?, f(20)?);
            let sc = d3d9draw::bump_scale(f(22)?, l);
            let pm = d3d9draw::premodulate(f(22)?, f(24)?);
            Some([u, v, l, sc, pm].iter().flat_map(|&x| f64_halves(x)).collect())
        },
    },
    Mirror {
        name: "d3d9.blendf",
        lean: BLEND,
        domain: Domain::Sampled("every SRCBLEND and DESTBLEND 0..16 by BLENDOP 0..7 and 0xffffffff, separate alpha on and off, on random channels; 400,000 samples of those with every alpha state; channels of f32 in -0.5..1.5, 0, -0, 1, NaN, infinities and any f32 bits"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xb1e0_d009);
            let special = [0.0f32, -0.0, 1.0, 0.5, f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 2.0, -1.0];
            let chan = |rng: &mut crate::rng::Sample64| -> f64 {
                f64::from(match rng.range(0, 3) {
                    0 => special.get(usize::try_from(rng.range(0, 8)).unwrap_or(0)).copied().unwrap_or(0.0),
                    1 => f32::from_bits(rng.word()),
                    _ => -0.5 + 2.0 * (f32::from(u16::try_from(rng.range(0, 0xffff)).unwrap_or(0)) / 65535.0),
                })
            };
            let mut send = |st: [i64; 8], rng: &mut crate::rng::Sample64| {
                let mut w: Vec<i64> = (0..8).flat_map(|_| f64_halves(chan(rng))).collect();
                w.extend(st);
                each(&w);
            };
            let ops = [0, 1, 2, 3, 4, 5, 6, 7, 0xffff_ffff];
            for sf in 0..=16 {
                for df in 0..=16 {
                    for op in ops {
                        for sep in 0..2 {
                            let st = [sf, df, op, sep, rng.range(0, 16), rng.range(0, 16), ops.get(usize::try_from(rng.range(0, 8)).unwrap_or(0)).copied().unwrap_or(1), rng.range(0, U32)];
                            send(st, &mut rng);
                        }
                    }
                }
            }
            for _ in 0..400_000 {
                let st = [rng.range(0, 16), rng.range(0, 16), rng.range(0, 7), rng.range(0, 1), rng.range(0, 16), rng.range(0, 16), rng.range(0, 7), rng.range(0, U32)];
                send(st, &mut rng);
            }
        },
        model: |a| {
            let c4 = |i: usize| -> Option<[f64; 4]> { Some([arg_f64(a, i)?, arg_f64(a, i.checked_add(2)?)?, arg_f64(a, i.checked_add(4)?)?, arg_f64(a, i.checked_add(6)?)?]) };
            let b = BlendStates { src: arg(a, 16)?, dst: arg(a, 17)?, op: arg(a, 18)?, separate: arg::<u8>(a, 19)? != 0, src_alpha: arg(a, 20)?, dst_alpha: arg(a, 21)?, op_alpha: arg(a, 22)?, factor: arg(a, 23)? };
            Some(d3d9draw::blend_float(c4(0)?, c4(8)?, &b).map_or(vec![], |v| v.iter().flat_map(|&x| f64_halves(x)).collect()))
        },
    },
    Mirror {
        name: "d3d9.point",
        lean: FILTER,
        domain: Domain::Sampled("texel coordinates (coords): u of k / 4096 in -2..2 times widths 1 .. 256, 64ths and 128ths and the binary64s 2 ulps about them, any binary64 bits"),
        inputs: coords,
        model: |a| Some(vec![d3d9draw::point_at(arg_f64(a, 0)?)]),
    },
    Mirror {
        name: "d3d9.bilinear",
        lean: FILTER,
        domain: Domain::Sampled("texel coordinates (coords): u of k / 4096 in -2..2 times widths 1 .. 256, 64ths and 128ths and the binary64s 2 ulps about them, any binary64 bits"),
        inputs: coords,
        model: |a| {
            let (i, f) = d3d9draw::bilinear_at(arg_f64(a, 0)?);
            Some(vec![i, f])
        },
    },
    Mirror {
        name: "d3d9.bibytes",
        lean: FILTER,
        domain: Domain::Sampled("every weight 0..32 each way, with 60 samples of four 8-bit texels each, their ends included"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xb1b1_d009);
            for fx in 0..=32 {
                for fy in 0..=32 {
                    for i in 0..60 {
                        let t: [i64; 4] = match i {
                            0 => [0; 4],
                            1 => [255; 4],
                            2 => [255, 0, 0, 255],
                            _ => std::array::from_fn(|_| rng.range(0, 255)),
                        };
                        each(&[t[0], t[1], t[2], t[3], fx, fy]);
                    }
                }
            }
        },
        model: |a| {
            let t = [arg::<u64>(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?];
            let w = d3d9draw::bilinear_weights(arg(a, 4)?, arg(a, 5)?);
            let v = d3d9draw::bilinear_bytes(t, w);
            let mut out = w.to_vec();
            out.push(v);
            out.extend(f64_halves(d3d9draw::bilinear_unit(v)));
            Some(out)
        },
    },
    Mirror {
        name: "d3d9.bifloat",
        lean: FILTER,
        domain: Domain::Sampled("every weight 0..32 each way, with 40 samples of four texels: f32 values in -2..2, 0, -0, 1, NaN, infinities and any binary64 bits"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xb1f1_d009);
            let special = [0.0f64, -0.0, 1.0, 0.5, f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0];
            for fx in 0..=32 {
                for fy in 0..=32 {
                    for i in 0..40 {
                        let mut w = vec![];
                        for _ in 0..4 {
                            let x = match (i, rng.range(0, 2)) {
                                (0, _) => -0.0,
                                (_, 0) => special.get(usize::try_from(rng.range(0, 7)).unwrap_or(0)).copied().unwrap_or(0.0),
                                (_, 1) => f64::from_bits(rng.next()),
                                _ => f64::from(-2.0 + 4.0 * (f32::from(u16::try_from(rng.range(0, 0xffff)).unwrap_or(0)) / 65535.0)),
                            };
                            w.extend(f64_halves(x));
                        }
                        w.extend([fx, fy]);
                        each(&w);
                    }
                }
            }
        },
        model: |a| {
            let t = [arg_f64(a, 0)?, arg_f64(a, 2)?, arg_f64(a, 4)?, arg_f64(a, 6)?];
            Some(f64_halves(d3d9draw::bilinear_float(t, d3d9draw::bilinear_weights(arg(a, 8)?, arg(a, 9)?))))
        },
    },
];
