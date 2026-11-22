//! Direct3D 9's mirrors (spec/lean/Kikizake/D3d9/).
use super::{arg, arg_f64, every, f64_halves, one, sampled, Domain, Mirror};
use crate::model::d3d9draw::{self, BlendOp, Winding};
use crate::model::d3dx9;

const PIXEL: &str = "spec/lean/Kikizake/D3d9/Pixel.lean";
const RASTER: &str = "spec/lean/Kikizake/D3d9/Raster.lean";
const FVF: &str = "spec/lean/Kikizake/D3d9/Fvf.lean";
const STAGE: &str = "spec/lean/Kikizake/D3d9/Stage.lean";
const FOG: &str = "spec/lean/Kikizake/D3d9/Fog.lean";
const ARG: &str = "spec/lean/Kikizake/D3d9/Arg.lean";
const U32: i64 = 0xffff_ffff;

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d9.cmp",
        lean: PIXEL,
        domain: Domain::Every("functions 0..9, 8-bit values compared"),
        inputs: |each| every(&[(0, 9), (0, 255), (0, 255)], each),
        model: |a| one(d3d9draw::cmp(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
    Mirror {
        name: "d3d9.stencil",
        lean: PIXEL,
        domain: Domain::Every("operations 0..9, every 8-bit stencil value with every reference (write mask 0xa5) and with every write mask (reference 0x3c)"),
        inputs: |each| {
            every(&[(0, 9), (0, 255), (0, 255)], &mut |v| {
                if let [op, s, r] = *v {
                    each(&[op, s, r, 0xa5]);
                }
            });
            every(&[(0, 9), (0, 255), (0, 255)], &mut |v| {
                if let [op, s, w] = *v {
                    each(&[op, s, 0x3c, w]);
                }
            });
        },
        model: |a| {
            let (s, r) = (arg(a, 1)?, arg(a, 2)?);
            let ns = d3d9draw::stencil_op(arg(a, 0)?, s, r);
            Some(vec![i64::from(ns), i64::from(d3d9draw::stencil_write(s, ns, arg(a, 3)?))])
        },
    },
    Mirror {
        name: "d3d9.address",
        lean: PIXEL,
        domain: Domain::Every("modes 0..7, texel indices -60..60, textures 1..16 texels"),
        inputs: |each| every(&[(0, 7), (-60, 60), (1, 16)], each),
        model: |a| Some(d3d9draw::address(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?).ok()?.into_iter().collect()),
    },
    Mirror {
        name: "d3d9.blend8",
        lean: PIXEL,
        domain: Domain::Sampled("the five operations, 8-bit values and factors, ONE or not, both roundings: every combination of the ends and 300,000 samples"),
        inputs: |each| sampled(&[(1, 5), (0, 255), (0, 1), (0, 255), (0, 255), (0, 1), (0, 255), (0, 1)], 300_000, 0xb1e0_d008, each),
        model: |a| {
            let op = match arg::<u8>(a, 0)? {
                1 => BlendOp::Add,
                2 => BlendOp::Sub,
                3 => BlendOp::RevSub,
                4 => BlendOp::Min,
                5 => BlendOp::Max,
                _ => return None,
            };
            let b = |i: usize| arg::<u8>(a, i).map(|v| v != 0);
            one(d3d9draw::blend8(op, (arg(a, 1)?, b(2)?, arg(a, 3)?), (arg(a, 4)?, b(5)?, arg(a, 6)?), b(7)?))
        },
    },
    Mirror {
        name: "d3d9.covers",
        lean: RASTER,
        domain: Domain::Sampled("every triangle and point of coordinates -256..256 in half pixels (1/256 units), and 300,000 of -700..700"),
        inputs: |each| {
            every(&[(-2, 2); 8], &mut |v| {
                let w: Vec<i64> = v.iter().map(|x| x.saturating_mul(128)).collect();
                each(&w);
            });
            sampled(&[(-700, 700); 8], 300_000, 0xc0e7_d009, each);
        },
        model: |a| {
            let p = |i: usize| Some((arg::<i64>(a, i)?, arg::<i64>(a, i.checked_add(1)?)?));
            one(d3d9draw::covers([p(0)?, p(2)?, p(4)?], arg(a, 6)?, arg(a, 7)?))
        },
    },
    Mirror {
        name: "d3d9.cull",
        lean: RASTER,
        domain: Domain::Sampled("modes 0..9 and the 32-bit ends, both runtimes, the three windings, and 20,000 32-bit modes"),
        inputs: |each| {
            every(&[(0, 9), (0, 1), (0, 2)], each);
            sampled(&[(0, U32), (0, 1), (0, 2)], 20_000, 0xc011_d009, each);
        },
        model: |a| {
            let w = match arg::<u8>(a, 2)? {
                0 => Winding::Cw,
                1 => Winding::Ccw,
                2 => Winding::Flat,
                _ => return None,
            };
            one(d3d9draw::culls(arg(a, 0)?, arg::<u8>(a, 1)? != 0, w))
        },
    },
    Mirror {
        name: "d3d9.blendop",
        lean: RASTER,
        domain: Domain::Sampled("operations 0..64, the 32-bit ends and 20,000 32-bit numbers"),
        inputs: |each| {
            every(&[(0, 64)], each);
            sampled(&[(0, U32)], 20_000, 0xb10b_d009, each);
        },
        model: |a| {
            one(match d3d9draw::blend_op(arg(a, 0)?) {
                BlendOp::Add => 1,
                BlendOp::Sub => 2,
                BlendOp::RevSub => 3,
                BlendOp::Min => 4,
                BlendOp::Max => 5,
            })
        },
    },
    Mirror {
        name: "d3d9.factor8",
        lean: RASTER,
        domain: Domain::Sampled("factors 0..20, channels 0..4, 8-bit source and destination, both runtimes, 32-bit BLENDFACTOR: every combination of the ends and 300,000 samples"),
        inputs: |each| sampled(&[(0, 20), (0, 4), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 1), (0, U32)], 300_000, 0xfac7_d009, each),
        model: |a| {
            let c4 = |i: usize| -> Option<[i64; 4]> { Some([arg(a, i)?, arg(a, i.checked_add(1)?)?, arg(a, i.checked_add(2)?)?, arg(a, i.checked_add(3)?)?]) };
            let c = d3d9draw::blend_color(arg::<u8>(a, 10)? != 0, arg(a, 11)?);
            Some(d3d9draw::factor8(arg(a, 0)?, arg(a, 1)?, c4(2)?, c4(6)?, c).into_iter().collect())
        },
    },
    Mirror {
        name: "d3d9.bothalpha",
        lean: RASTER,
        domain: Domain::Every("source and destination factors 0..20, color and separate alpha"),
        inputs: |each| every(&[(0, 20), (0, 20), (0, 1)], each),
        model: |a| {
            let (s, d) = d3d9draw::both_alpha(arg(a, 0)?, arg(a, 1)?, arg::<u8>(a, 2)? != 0);
            Some(vec![i64::from(s), i64::from(d)])
        },
    },
    Mirror {
        name: "d3d9.colorwrite",
        lean: RASTER,
        domain: Domain::Sampled("masks 0..63 on two fixed colors, and 100,000 32-bit masks on 8-bit colors"),
        inputs: |each| {
            every(&[(0, 63)], &mut |v| {
                if let [m] = *v {
                    each(&[m, 1, 2, 3, 4, 5, 6, 7, 8]);
                }
            });
            sampled(&[(0, U32), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255), (0, 255)], 100_000, 0xc01e_d009, each);
        },
        model: |a| {
            let c4 = |i: usize| -> Option<[i64; 4]> { Some([arg(a, i)?, arg(a, i.checked_add(1)?)?, arg(a, i.checked_add(2)?)?, arg(a, i.checked_add(3)?)?]) };
            Some(d3d9draw::color_write(arg(a, 0)?, c4(1)?, c4(5)?).to_vec())
        },
    },
    Mirror {
        name: "d3dx9.fvfdecl",
        lean: FVF,
        domain: Domain::Sampled("every FVF of the low 16 bits, and 300,000 32-bit FVFs"),
        inputs: |each| {
            every(&[(0, 0xffff)], each);
            sampled(&[(0, U32)], 300_000, 0xf1fd_d009, each);
        },
        model: |a| Some(match d3dx9::fvf_decl(arg(a, 0)?) {
            Ok(e) => std::iter::once(0).chain(flat(&e)).collect(),
            Err(c) => vec![1, i64::from(c)],
        }),
    },
    Mirror {
        name: "d3dx9.declfvf",
        lean: FVF,
        domain: Domain::Sampled("the declarations D3DXDeclaratorFromFVF makes of every FVF of the low 16 bits and of 100,000 32-bit FVFs, each also with one element dropped, two swapped, an offset moved, a type, usage or index changed, a stream set and an end inside"),
        inputs: |each| {
            let mut rng = crate::rng::XorShift32::new(0xdec1_d009);
            let mut decl = |f: i64| {
                let Some(Ok(e)) = u32::try_from(f).ok().map(d3dx9::fvf_decl) else {
                    return;
                };
                let send = |e: &[(u32, u32)], each: &mut dyn FnMut(&[i64])| each(&flat(e).collect::<Vec<_>>());
                send(&e, each);
                if e.is_empty() {
                    return;
                }
                let n = i64::try_from(e.len()).unwrap_or(1);
                let pick = |rng: &mut crate::rng::XorShift32| usize::try_from(rng.range(0, n.saturating_sub(1))).unwrap_or(0);
                let mut d = e.clone();
                d.remove(pick(&mut rng));
                send(&d, each);
                let mut d = e.clone();
                let (i, j) = (pick(&mut rng), pick(&mut rng));
                d.swap(i, j);
                send(&d, each);
                for (k, change) in [(0u8, 4u32 << 16), (1, 1), (2, 1 << 16), (3, 1 << 24), (4, 1), (5, 0)] {
                    let mut d = e.clone();
                    let i = pick(&mut rng);
                    if let Some(x) = d.get_mut(i) {
                        match k {
                            0 | 4 => x.0 = x.0.wrapping_add(change),
                            5 => x.0 = x.0 & !0xffff | 0xff,
                            _ => x.1 = x.1.wrapping_add(change),
                        }
                    }
                    send(&d, each);
                }
            };
            every(&[(0, 0xffff)], &mut |v| {
                if let [f] = *v {
                    decl(f);
                }
            });
            sampled(&[(0, U32)], 100_000, 0xdec2_d009, &mut |v| {
                if let [f] = *v {
                    decl(f);
                }
            });
        },
        model: |a| {
            let e: Option<Vec<(u32, u32)>> = a.chunks(2).map(|p| match *p {
                [x, y] => Some((u32::try_from(x).ok()?, u32::try_from(y).ok()?)),
                _ => None,
            }).collect();
            Some(match d3dx9::decl_fvf(&e?) {
                Ok(f) => vec![0, i64::from(f)],
                Err(c) => vec![1, i64::from(c)],
            })
        },
    },
    Mirror {
        name: "d3dx9.fvfsize",
        lean: FVF,
        domain: Domain::Sampled("every FVF of the low 16 bits, and 300,000 32-bit FVFs"),
        inputs: |each| {
            every(&[(0, 0xffff)], each);
            sampled(&[(0, U32)], 300_000, 0xf1f5_d009, each);
        },
        model: |a| one(d3dx9::fvf_size(arg(a, 0)?)),
    },
    Mirror {
        name: "d3d9.fvfdecl",
        lean: FVF,
        domain: Domain::Sampled("every FVF of the low 16 bits, and 300,000 32-bit FVFs"),
        inputs: |each| {
            every(&[(0, 0xffff)], each);
            sampled(&[(0, U32)], 300_000, 0xf1f9_d009, each);
        },
        model: |a| Some(match d3d9draw::fvf_runtime_decl(arg(a, 0)?) {
            Ok(Some(e)) => std::iter::once(0).chain(flat(&e)).collect(),
            Ok(None) => vec![],
            Err(_) => vec![-1],
        }),
    },
    Mirror {
        name: "d3d9.stage",
        lean: STAGE,
        domain: Domain::Sampled("operations 0..33 on stage 0 and 3, color and alpha, the nine f32 inputs: 300,000 of 8-bit values (k / 255), 200,000 of any f32 in 0..1, 100,000 of any f32 in -4..4"),
        inputs: |each| {
            let mut send = |v: &[i64], map: &dyn Fn(i64) -> i64| {
                let mut w: Vec<i64> = v.get(..3).unwrap_or(&[]).to_vec();
                w.extend(v.iter().skip(3).map(|&x| map(x)));
                each(&w);
            };
            let byte = |k: i64| u8::try_from(k).map(|k| i64::from((f32::from(k) / 255.0).to_bits())).unwrap_or(0);
            let mut r = vec![(0, 33), (0, 3), (0, 1)];
            r.extend([(0, 255); 9]);
            sampled(&r, 300_000, 0x57a9_d009, &mut |v| send(v, &byte));
            let mut r = vec![(0, 33), (0, 3), (0, 1)];
            r.extend([(0, 0x3f80_0000); 9]);
            sampled(&r, 200_000, 0x57aa_d009, &mut |v| send(v, &|x| x));
            let mut r = vec![(0, 33), (0, 3), (0, 1)];
            r.extend([(0, 0x4080_0000); 9]);
            sampled(&r, 100_000, 0x57ab_d009, &mut |v| send(v, &|x| if x % 2 == 0 { x } else { x | 0x8000_0000 }));
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            let x = d3d9draw::StageIn { op: arg(a, 0)?, st: arg(a, 1)?, alpha: arg::<u8>(a, 2)? != 0, a0: f(3)?, a1: f(4)?, a2: f(5)?, a1a: f(6)?, da: f(7)?, ca: f(8)?, ta: f(9)?, fa: f(10)?, cur: f(11)? };
            Some(match d3d9draw::stage_value(x) {
                Ok(v) => vec![i64::from(v.clamp(0.0, 1.0).to_bits())],
                Err(_) => vec![],
            })
        },
    },
    Mirror {
        name: "d3d9.specular",
        lean: FOG,
        domain: Domain::Sampled("300,000 pairs of 8-bit channel values (k / 255 in f32, as f64) and 200,000 of any binary64 in 0..2"),
        inputs: |each| {
            sampled(&[(0, 255), (0, 255)], 300_000, 0x5bec_d009, &mut |v| {
                let w: Vec<i64> = v.iter().flat_map(|&k| f64_halves(byte64(k))).collect();
                each(&w);
            });
            sampled(&[(0, 0x4000_0000), (0, U32), (0, 0x4000_0000), (0, U32)], 200_000, 0x5bed_d009, each);
        },
        model: |a| Some(f64_halves(d3d9draw::specular_add(arg_f64(a, 0)?, arg_f64(a, 2)?))),
    },
    Mirror {
        name: "d3d9.fogexp",
        lean: FOG,
        domain: Domain::Sampled("EXP and EXP2, 300,000 depths 0..1 and densities 0..4 as f32 values in f64"),
        inputs: |each| {
            sampled(&[(1, 2), (0, 0x3f80_0000), (0, 0x4080_0000)], 300_000, 0xf0e0_d009, &mut |v| {
                if let [m, z, d] = *v {
                    let mut w = vec![m];
                    w.extend(f64_halves(f32w(z)));
                    w.extend(f64_halves(f32w(d)));
                    each(&w);
                }
            });
        },
        model: |a| Some(f64_halves(d3d9draw::fog_exp_arg(arg(a, 0)?, arg_f64(a, 1)?, arg_f64(a, 3)?))),
    },
    Mirror {
        name: "d3d9.fogfactor",
        lean: FOG,
        domain: Domain::Sampled("modes 0..4, 400,000 depths, specular alphas, starts, ends (f32 values in f64, -2..2) and densities 0..4; EXP's exponential Rust's exp of fogexp's exponent"),
        inputs: |each| {
            sampled(&[(0, 4), (0, 0x4000_0000), (0, 0x3f80_0000), (0, 0x4000_0000), (0, 0x4000_0000), (0, 0x4080_0000)], 400_000, 0xf0ef_d009, &mut |v| {
                if let [m, z, sa, s0, e0, d] = *v {
                    let neg = |x: i64| if x % 3 == 0 { x | 0x8000_0000 } else { x };
                    let (z, s0, e0, d) = (f32w(neg(z)), f32w(neg(s0)), f32w(e0), f32w(d));
                    let e = u32::try_from(m).map(|m| d3d9draw::fog_exp_arg(m, z, d).exp()).unwrap_or(0.0);
                    let mut w = vec![m];
                    for x in [z, f32w(sa), s0, e0, e, d] {
                        w.extend(f64_halves(x));
                    }
                    each(&w);
                }
            });
        },
        model: |a| {
            let r = d3d9draw::fog_factor(arg(a, 0)?, arg_f64(a, 1)?, arg_f64(a, 3)?, (arg_f64(a, 5)?, arg_f64(a, 7)?, arg_f64(a, 11)?));
            Some(r.ok().map(|f| if f.is_nan() { 0x7fc0_0000 } else { i64::from(f.to_bits()) }).into_iter().collect())
        },
    },
    Mirror {
        name: "d3d9.fogblend",
        lean: FOG,
        domain: Domain::Sampled("300,000 factors, channels and fog channels of 8-bit values (k / 255) and 200,000 of any f32 in 0..1"),
        inputs: |each| {
            let byte = |k: i64| u8::try_from(k).map(|k| i64::from((f32::from(k) / 255.0).to_bits())).unwrap_or(0);
            sampled(&[(0, 255); 3], 300_000, 0xf0eb_d009, &mut |v| {
                let w: Vec<i64> = v.iter().map(|&k| byte(k)).collect();
                each(&w);
            });
            sampled(&[(0, 0x3f80_0000); 3], 200_000, 0xf0ec_d009, each);
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            let v = d3d9draw::fog_blend(f(0)?, f(1)?, f(2)?);
            one(if v.is_nan() { 0x7fc0_0000 } else { i64::from(v.to_bits()) })
        },
    },
    Mirror {
        name: "d3d9.arg",
        lean: ARG,
        domain: Domain::Sampled("every selector and modifier 0..0x7f on both runtimes with a texture and without, on random 8-bit registers, and 300,000 samples of selectors 0..0x7f, registers of any f32 in 0..1, 32-bit TEXTUREFACTOR and CONSTANT"),
        inputs: |each| {
            let mut rng = crate::rng::XorShift32::new(0xa7a9_d009);
            let mut send = |a: i64, v8: i64, has: i64, unit: &mut dyn FnMut() -> f64, rng: &mut crate::rng::XorShift32| {
                let mut w = vec![a, v8];
                for _ in 0..16 {
                    w.extend(f64_halves(unit()));
                }
                w.push(has);
                for _ in 0..4 {
                    w.extend(f64_halves(unit()));
                }
                w.push(rng.range(0, U32));
                w.push(rng.range(0, U32));
                each(&w);
            };
            let mut r2 = crate::rng::XorShift32::new(0xa7aa_d009);
            for a in 0..0x80 {
                for v8 in 0..2 {
                    for has in 0..2 {
                        let mut unit = || byte64(r2.range(0, 255));
                        send(a, v8, has, &mut unit, &mut rng);
                    }
                }
            }
            let mut r3 = crate::rng::XorShift32::new(0xa7ab_d009);
            for _ in 0..300_000 {
                let (a, v8, has) = (rng.range(0, 0x7f), rng.range(0, 1), rng.range(0, 1));
                let mut unit = || f32w(r3.range(0, 0x3f80_0000));
                send(a, v8, has, &mut unit, &mut rng);
            }
        },
        model: |a| {
            let c4 = |i: usize| -> Option<[f64; 4]> { Some([arg_f64(a, i)?, arg_f64(a, i.checked_add(2)?)?, arg_f64(a, i.checked_add(4)?)?, arg_f64(a, i.checked_add(6)?)?]) };
            let r = d3d9draw::ArgRegs { diffuse: c4(2)?, current: c4(10)?, specular: c4(18)?, temp: c4(26)?, tex: if arg::<u8>(a, 34)? != 0 { Some(c4(35)?) } else { None }, factor: arg(a, 43)?, constant: arg(a, 44)? };
            Some(match d3d9draw::stage_arg(arg(a, 0)?, &r, arg::<u8>(a, 1)? != 0) {
                Ok(v) => v.iter().flat_map(|&x| f64_halves(x)).collect(),
                Err(_) => vec![],
            })
        },
    },
];

/// the elements' pairs one after the other
fn flat(e: &[(u32, u32)]) -> impl Iterator<Item = i64> + '_ {
    e.iter().flat_map(|x| [i64::from(x.0), i64::from(x.1)])
}

/// an 8-bit channel value k / 255 in f32, as f64
fn byte64(k: i64) -> f64 {
    u8::try_from(k).map(|k| f64::from(f32::from(k) / 255.0)).unwrap_or(0.0)
}

/// the f32 of 32 bits, as f64
fn f32w(x: i64) -> f64 {
    u32::try_from(x).map(|b| f64::from(f32::from_bits(b))).unwrap_or(0.0)
}
