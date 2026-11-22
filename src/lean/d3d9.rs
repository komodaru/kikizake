//! Direct3D 9's mirrors (spec/lean/Kikizake/D3d9/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d9draw::{self, BlendOp, Winding};

const PIXEL: &str = "spec/lean/Kikizake/D3d9/Pixel.lean";
const RASTER: &str = "spec/lean/Kikizake/D3d9/Raster.lean";
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
];
