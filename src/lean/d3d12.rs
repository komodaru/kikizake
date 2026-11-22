//! Direct3D 12's mirrors (spec/lean/Kikizake/D3d12/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d12::{self, Dim, Layout};

const FOOTPRINT: &str = "spec/lean/Kikizake/D3d12/Footprint.lean";

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d12.layout",
        lean: FOOTPRINT,
        domain: Domain::Sampled("1D, 2D and 3D textures up to 300 texels a side, 1..9 levels, 1..4 slices, both planes, blocks 1x1 and 4x4, 1..16 bytes, every subresource range"),
        inputs: |each| {
            sampled(&[(0, 8), (0, 12), (0, 4096), (1, 9), (1, 4), (2, 4), (1, 300), (1, 300), (1, 40), (0, 120), (1, 16), (0, 1), (0, 1)], 60_000, 0xf0f0_0001, &mut |a| {
                if let [first, count, base, mips, array, kind, w, h, d, fmt, bytes, block, deep] = *a {
                    let b = if block == 1 { 4 } else { 1 };
                    each(&[first, count, base, mips, array, kind, w, h, d, fmt, bytes, b, b, if deep == 1 { 512 } else { 256 }]);
                }
            })
        },
        model: |a| {
            let l = Layout {
                mips: arg(a, 3)?,
                array: arg(a, 4)?,
                kind: Dim::of(arg(a, 5)?),
                w: arg(a, 6)?,
                h: arg(a, 7)?,
                d: arg(a, 8)?,
                fmt: arg(a, 9)?,
                block: (arg(a, 10)?, arg(a, 11)?, arg(a, 12)?),
                pitch_align: arg(a, 13)?,
            };
            let (fps, total) = d3d12::layout(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, &l);
            let mut v = vec![i64::try_from(total).ok()?];
            for (o, f, w, h, d, p, r, s) in fps {
                for x in [o, u64::from(f), u64::from(w), u64::from(h), u64::from(d), p, u64::from(r), s] {
                    v.push(i64::try_from(x).ok()?);
                }
            }
            Some(v)
        },
    },
    Mirror {
        name: "d3d12.chain",
        lean: FOOTPRINT,
        domain: Domain::Every("largest dimensions 0..65536"),
        inputs: |each| every(&[(0, 65536)], each),
        model: |a| one(d3d12::chain(arg(a, 0)?)),
    },
    Mirror {
        name: "d3d12.align",
        lean: FOOTPRINT,
        domain: Domain::Sampled("values up to 2^40, the alignments 256, 512, 4KB, 64KB, 4MB and others"),
        inputs: |each| {
            for a in [1, 3, 256, 512, 4096, 65536, 4 << 20] {
                sampled(&[(0, 1 << 40)], 20_000, 0xa1a1_0000 ^ u32::try_from(a).unwrap_or(0), &mut |v| {
                    if let [v] = *v {
                        each(&[v, a]);
                    }
                });
            }
        },
        model: |a| one(i64::try_from(d3d12::align(arg(a, 0)?, arg(a, 1)?)).ok()?),
    },
];
