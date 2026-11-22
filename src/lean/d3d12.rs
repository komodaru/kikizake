//! Direct3D 12's mirrors (spec/lean/Kikizake/D3d12/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3d12::{self, Dim, Heap, Layout, Texels};
use crate::model::dxgi_format;
use crate::resource_desc::Desc;
use crate::spec::clause;

const FOOTPRINT: &str = "spec/lean/Kikizake/D3d12/Footprint.lean";
const ALLOC: &str = "spec/lean/Kikizake/D3d12/Alloc.lean";
const CREATE: &str = "spec/lean/Kikizake/D3d12/Create.lean";
const COPYABLE: &str = "spec/lean/Kikizake/D3d12/Copyable.lean";
const U32: i64 = 0xffff_ffff;
/// the values asked of a description's fields: alignments, widths and heights, depths or
/// slices, mip levels, sample counts, qualities, layouts
const ALIGNS: [i64; 9] = [0, 1, 256, 4096, 8192, 65536, 131_072, 4_194_304, U32];
const SIZES: [i64; 14] = [0, 1, 2, 3, 7, 64, 2047, 2048, 2049, 16384, 16385, 1 << 23, (1 << 23) + 1, U32];
const DEPTHS: [i64; 7] = [0, 1, 2, 3, 2048, 2049, 0xffff];
const MIPS: [i64; 7] = [0, 1, 2, 3, 8, 15, 0xffff];
const SAMPLES: [i64; 9] = [0, 1, 2, 3, 4, 8, 16, 17, 32];
const QUALITIES: [i64; 3] = [0, 1, U32];
const LAYOUTS: [i64; 6] = [0, 1, 2, 3, 4, U32];
/// initial states: none, each bit that means something, GENERIC_READ and its parts, pairs,
/// undefined bits
const STATES: [i64; 34] = [
    0, 1, 2, 4, 8, 0x10, 0x20, 0x30, 0x40, 0x80, 0xc0, 0x100, 0x200, 0x400, 0x800, 0xac3, 0xac2, 0x1000, 0x1400, 0x2000, 0x1_0000, 0xf_0000, 0x2_0000, 0x8_0000, 0x20_0000, 0x40_0000, 0x80_0000, 0x100_0000,
    0x200_0000, 0x4000_0000, 0x8000_0000, 0x44, 0x48, 0x18,
];
/// near a texture that is taken: heaps, sizes, depths, levels, samples, qualities, layouts,
/// flags, alignments, states
const NEAR_HEAPS: [i64; 3] = [1, 5, 2];
const NEAR_SIZES: [i64; 7] = [1, 2, 4, 64, 2048, 16384, 3];
const NEAR_DEPTHS: [i64; 3] = [1, 2, 6];
const NEAR_SAMPLES: [i64; 4] = [1, 1, 2, 4];
const NEAR_QUALITIES: [i64; 2] = [0, U32];
const NEAR_LAYOUTS: [i64; 3] = [0, 2, 3];
const NEAR_FLAGS: [i64; 10] = [0, 1, 2, 4, 5, 8, 10, 0x20, 0x21, 0x400];
const NEAR_ALIGNS: [i64; 4] = [0, 4096, 65536, 4_194_304];
const NEAR_STATES: [i64; 9] = [0, 4, 8, 0x10, 0x20, 0x30, 0x40, 0xac3, 0x40_0000];

/// a description from its eleven words (the model's field order)
fn desc(a: &[i64]) -> Option<Desc> {
    let mut w = [0u32; 11];
    for (i, x) in w.iter_mut().enumerate() {
        *x = arg(a, i)?;
    }
    Some(Desc::from_words(&w))
}

/// `validate`'s clause as the Lean code (0 taken)
fn refusal(r: Result<(), crate::spec::ClauseId>) -> Option<i64> {
    let Err(id) = r else { return Some(0) };
    [clause::D3D12_DESC_DIM, clause::D3D12_DESC_BUFFER, clause::D3D12_DESC_SIZE, clause::D3D12_DESC_ALIGN, clause::D3D12_DESC_MSAA, clause::D3D12_DESC_LAYOUT]
        .iter()
        .position(|c| *c == id)
        .and_then(|i| i64::try_from(i).ok()?.checked_add(1))
}
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
    Mirror {
        name: "d3d12.support",
        lean: CREATE,
        domain: Domain::Every("every format 0..1024 and 2^31, 0xfffffffe, 0xffffffff: its support bits and whether it has no element"),
        inputs: |each| {
            every(&[(0, 1024)], each);
            for f in [1 << 31, 0xffff_fffe, U32] {
                each(&[f]);
            }
        },
        model: |a| {
            let f = arg(a, 0)?;
            Some(vec![i64::from(dxgi_format::support(f)), i64::from(dxgi_format::block(f).is_none())])
        },
    },
    Mirror {
        name: "d3d12.validate",
        lean: CREATE,
        domain: Domain::Sampled("every dimension 0..6, formats 0..192, flags 0..0xfff, the edges of each size, alignment, level, sample, quality and layout: 300,000 descriptions"),
        inputs: |each| {
            sampled(&[(0, 6), (0, 8), (0, 13), (0, 13), (0, 6), (0, 6), (0, 192), (0, 8), (0, 2), (0, 5), (0, 0xfff)], 300_000, 0xde5c_0001, &mut |v| {
                if let [dim, al, w, h, d, m, fmt, sa, q, l, fl] = *v {
                    each(&[dim, pick(&ALIGNS, al), pick(&SIZES, w), pick(&SIZES, h), pick(&DEPTHS, d), pick(&MIPS, m), fmt, pick(&SAMPLES, sa), pick(&QUALITIES, q), pick(&LAYOUTS, l), fl]);
                }
            })
        },
        model: |a| Some(vec![refusal(d3d12::validate(&desc(a)?))?]),
    },
    Mirror {
        name: "d3d12.create",
        lean: CREATE,
        domain: Domain::Sampled("every heap type 0..6, dimension 0..6, format 0..192, flags 0..0xfff, 34 initial states, the edges of each size, alignment, level, sample, quality and layout, with and without an element and 16 small tiles: 400,000 descriptions, and 400,000 near ones that are taken"),
        inputs: |each| {
            sampled(&[(0, 6), (0, 33), (0, 6), (0, 8), (0, 13), (0, 13), (0, 6), (0, 6), (0, 192), (0, 8), (0, 2), (0, 5), (0, 0xfff), (0, 1), (0, 1)], 400_000, 0xc4ea_0001, &mut |v| {
                if let [heap, st, dim, al, w, h, d, m, fmt, sa, q, l, fl, b, sm] = *v {
                    each(&[heap, pick(&STATES, st), dim, pick(&ALIGNS, al), pick(&SIZES, w), pick(&SIZES, h), pick(&DEPTHS, d), pick(&MIPS, m), fmt, pick(&SAMPLES, sa), pick(&QUALITIES, q), pick(&LAYOUTS, l), fl, b, sm]);
                }
            });
            sampled(&[(0, 2), (0, 8), (2, 4), (0, 3), (0, 6), (0, 6), (0, 2), (0, 2), (0, 192), (0, 3), (0, 1), (0, 2), (0, 9), (0, 1), (0, 1)], 400_000, 0xc4ea_0002, &mut |v| {
                if let [heap, st, dim, al, w, h, d, m, fmt, sa, q, l, fl, b, sm] = *v {
                    let h = if dim == 2 { 1 } else { pick(&NEAR_SIZES, h) };
                    each(&[pick(&NEAR_HEAPS, heap), pick(&NEAR_STATES, st), dim, pick(&NEAR_ALIGNS, al), pick(&NEAR_SIZES, w), h, pick(&NEAR_DEPTHS, d), m, fmt, pick(&NEAR_SAMPLES, sa), pick(&NEAR_QUALITIES, q), pick(&NEAR_LAYOUTS, l), pick(&NEAR_FLAGS, fl), b, sm]);
                }
            });
        },
        model: |a| {
            let hr = d3d12::create_verdict(Heap::of(arg(a, 0)?), arg(a, 1)?, &desc(a.get(2..)?)?, arg::<u8>(a, 13)? != 0, arg::<u8>(a, 14)? != 0);
            one(hr.0)
        },
    },
    Mirror {
        name: "d3d12.dims",
        lean: COPYABLE,
        domain: Domain::Every("every format 0..1024 and 0xffffffff"),
        inputs: |each| {
            every(&[(0, 1024)], each);
            each(&[U32]);
        },
        model: |a| one(dxgi_format::dims(arg(a, 0)?)),
    },
    Mirror {
        name: "d3d12.footprints",
        lean: COPYABLE,
        domain: Domain::Sampled("every dimension 0..6 and format 0..192, the edges of each size up to 0xffffffff, alignment, sample, quality and layout, levels 0..3, 8, 15, flags 0..0x3f, ranges of up to 12 subresources from 0..12: 300,000 descriptions, and 300,000 valid ones of 1..300 texels a side"),
        inputs: |each| {
            sampled(&[(0, 12), (0, 12), (0, 4096), (0, 6), (0, 8), (0, 13), (0, 13), (0, 6), (0, 5), (0, 192), (0, 8), (0, 2), (0, 5), (0, 0x3f)], 300_000, 0xf007_0001, &mut |v| {
                if let [first, count, base, dim, al, w, h, d, m, fmt, sa, q, l, fl] = *v {
                    each(&[first, count, base, dim, pick(&ALIGNS, al), pick(&SIZES, w), pick(&SIZES, h), pick(&DEPTHS, d), pick(&MIPS, m), fmt, pick(&SAMPLES, sa), pick(&QUALITIES, q), pick(&LAYOUTS, l), fl]);
                }
            });
            sampled(&[(0, 12), (0, 12), (0, 4096), (1, 4), (1, 300), (1, 300), (1, 12), (0, 4), (0, 192)], 300_000, 0xf007_0002, &mut |v| {
                if let [first, count, base, dim, w, h, d, m, fmt] = *v {
                    let (h, d) = if dim == 2 { (1, d) } else { (h, d) };
                    let (fmt, w, h, d, m) = if dim == 1 { (0, w, 1, 1, 1) } else { (fmt, w, h, d, m) };
                    each(&[first, count, base, dim, 0, w, h, d, m, fmt, 1, 0, i64::from(dim == 1), 0]);
                }
            });
        },
        model: |a| {
            let (fps, total) = d3d12::footprints(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, &desc(a.get(3..)?)?).ok()?;
            if total == u64::MAX {
                return Some(vec![-1]);
            }
            let mut v = vec![i64::try_from(total).ok()?];
            for (o, f, w, h, d, p, r, s) in fps {
                for x in [o, u64::from(f), u64::from(w), u64::from(h), u64::from(d), p, u64::from(r), s] {
                    v.push(i64::try_from(x).ok()?);
                }
            }
            Some(v)
        },
    },
];
