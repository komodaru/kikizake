//! Direct3D 9's mirrors (spec/lean/Kikizake/D3d9/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d9draw::{self, BlendOp};

const PIXEL: &str = "spec/lean/Kikizake/D3d9/Pixel.lean";

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
];
