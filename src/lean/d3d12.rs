//! Direct3D 12's mirrors (spec/lean/Kikizake/D3d12/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d12::{self, Dim, Layout, Texels};

const FOOTPRINT: &str = "spec/lean/Kikizake/D3d12/Footprint.lean";
const ALLOC: &str = "spec/lean/Kikizake/D3d12/Alloc.lean";
/// element sizes (bytes) the formats have
const BYTES: [i64; 6] = [1, 2, 4, 8, 12, 16];

/// the entry `i` of a table of choices (0 past its end)
fn pick(t: &[i64], i: i64) -> i64 {
    usize::try_from(i).ok().and_then(|i| t.get(i)).copied().unwrap_or(0)
}

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
    Mirror {
        name: "d3d12.tight",
        lean: ALLOC,
        domain: Domain::Sampled("1D, 2D and 3D textures up to 300 texels a side and 40 slices, 1..9 levels, elements of 1, 2, 4, 8, 12, 16 bytes, blocks 1x1 and 4x4: 60,000 textures"),
        inputs: |each| {
            sampled(&[(2, 4), (1, 300), (1, 300), (1, 40), (1, 9), (0, 5), (0, 1)], 60_000, 0xa110_c001, &mut |v| {
                if let [kind, w, h, d, mips, b, block] = *v {
                    let k = if block == 1 { 4 } else { 1 };
                    each(&[kind, w, h, d, mips, pick(&BYTES, b), k, k]);
                }
            })
        },
        model: |a| {
            let t = Texels { kind: Dim::of(arg(a, 0)?), w: arg(a, 1)?, h: arg(a, 2)?, d: arg(a, 3)?, mips: arg(a, 4)?, elem: (arg(a, 5)?, arg(a, 6)?, arg(a, 7)?) };
            Some(vec![i64::try_from(d3d12::tight_bytes(&t)).ok()?, i64::try_from(d3d12::swizzle_bytes(&t)).ok()?])
        },
    },
    Mirror {
        name: "d3d12.alloc_size",
        lean: ALLOC,
        domain: Domain::Every("totals 0..70,000 and around 4MB, 0..17 samples, the alignments 0, 4KB, 64KB, 4MB and 1, a 1D render target or not"),
        inputs: |each| {
            for total in (0..=70_000).chain(4_194_000..=4_194_700) {
                for samples in [0, 1, 2, 4, 8, 16, 17] {
                    for asked in [0, 1, 4096, 65536, 4_194_304] {
                        for rt1d in [0, 1] {
                            each(&[total, samples, asked, rt1d]);
                        }
                    }
                }
            }
        },
        model: |a| {
            let (s, al) = d3d12::alloc_size(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg::<u8>(a, 3)? != 0);
            Some(vec![i64::try_from(s).ok()?, i64::try_from(al).ok()?])
        },
    },
    Mirror {
        name: "d3d12.small_tiles",
        lean: ALLOC,
        domain: Domain::Every("1D, 2D and 3D textures up to 140 texels a side (3D: 70) and 18 deep, elements of 1..20 bytes, blocks 1x1 and 4x4"),
        inputs: |each| {
            every(&[(2, 3), (1, 140), (1, 140), (1, 20), (0, 1)], &mut |v| {
                if let [kind, w, h, bytes, block] = *v {
                    let k = if block == 1 { 4 } else { 1 };
                    each(&[kind, w, h, 1, bytes, k, k]);
                }
            });
            every(&[(1, 70), (1, 70), (1, 18), (1, 20), (0, 1)], &mut |v| {
                if let [w, h, d, bytes, block] = *v {
                    let k = if block == 1 { 4 } else { 1 };
                    each(&[4, w, h, d, bytes, k, k]);
                }
            });
        },
        model: |a| {
            let t = d3d12::small_tiles(Dim::of(arg(a, 0)?), arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, (arg(a, 4)?, arg(a, 5)?, arg(a, 6)?));
            Some(t.into_iter().map(|n| i64::try_from(n).unwrap_or(-1)).collect())
        },
    },
];
