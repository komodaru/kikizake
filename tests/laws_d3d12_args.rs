//! The argument checks of Direct3D 12 (spec/d3d12.txt: heap properties, CreateHeap,
//! CreatePlacedResource, CreateCommittedResource's heap flags and clear values,
//! GetResourceAllocationInfo, the format and multisample queries, the clears, the copies): each
//! law runs over the whole domain it names (every flags value of the bits named, every format
//! id, every heap property, a grid of descriptions), so it is a proof for that domain (kind
//! exhaustive). The references are the clauses' words, apart from the model's functions.
use kikizake::model::d3d12::{validate, alloc, clear_texel, committed_hr, create_hr, heapx_hr, placed_hr, props_ok, Clear, CpuPage, Heap, Hr, Pool, Props, E_INVALIDARG, S_FALSE, S_OK};
use kikizake::model::dxgi_format::{block, caps, quality_levels, support, CAPS, DEPTH, T1, T2, T3, TYPELESS};
use kikizake::resource_desc::Desc;

const SHARED: u32 = 0x1;
const ALLOW_DISPLAY: u32 = 0x8;
const CROSS_ADAPTER: u32 = 0x20;
const WRITE_WATCH: u32 = 0x200;
const ATOMICS: u32 = 0x400;
const MANUAL_TRACKING: u32 = 0x2000;
const DENY: u32 = 0xc4;

/// heap type, page property, pool as the words of D3D12_HEAP_PROPERTIES
fn props(heap: u32, page: u32, pool: u32, creation: u32, visible: u32) -> Props {
    Props::of(&[heap, page, pool, creation, visible])
}

/// a heap the CPU maps: upload, readback, GPU upload, a custom heap of write-combined or
/// written-back pages
fn cpu_mapped(heap: u32, page: u32) -> bool {
    matches!(heap, 2 | 3 | 5) || heap == 4 && matches!(page, 2 | 3)
}

/// a heap only the GPU reaches: the default one, a custom heap of no CPU pages in L0
fn gpu_only(heap: u32, page: u32, pool: u32) -> bool {
    heap == 1 || heap == 4 && page == 1 && pool == 1
}

/// D3D12.HEAP.PROPS in the clause's words
fn props_rule(heap: u32, page: u32, pool: u32, creation: u32, visible: u32, flags: u32, committed: bool) -> bool {
    let node = |m: u32| if m == 0 { 1 } else { m };
    let types = (1..=5).contains(&heap) && if heap == 4 { (1..=3).contains(&page) && pool == 1 } else { page == 0 && pool == 0 };
    let bits = flags & !0x3fed == 0 && flags & MANUAL_TRACKING == 0;
    let cpu = cpu_mapped(heap, page);
    let shared = !(cpu && flags & SHARED != 0) && (cpu || flags & WRITE_WATCH == 0);
    let deny = flags & DENY;
    let deny_ok = if committed || flags & CROSS_ADAPTER != 0 { deny == 0 } else { matches!(deny, 0 | 0xc0 | 0x84 | 0x44) };
    let nodes = node(creation) == 1 && node(visible) & 1 == 1;
    let cross = flags & CROSS_ADAPTER == 0 || flags & SHARED != 0 && gpu_only(heap, page, pool);
    let display = flags & ALLOW_DISPLAY == 0 || gpu_only(heap, page, pool) && committed;
    let atomics = flags & ATOMICS == 0 || heap == 4 || !cpu;
    types && bits && shared && deny_ok && nodes && cross && display && atomics
}

/// the flags values asked: every subset of the bits 0..13, then each bit 14..31 alone
fn flag_values() -> impl Iterator<Item = u32> {
    (0..0x4000u32).chain((14..32).map(|b| 1u32 << b))
}

/// D3D12.HEAP.PROPS: for every heap type 0..6, page property 0..4, pool 0..3, node masks (0, 0),
/// (1, 1), (2, 2), (0, 2), (1, 3), (2, 3) and flags value asked, for CreateHeap and
/// CreateCommittedResource: the properties and flags taken are the clause's
#[test]
fn heap_properties_are_the_clause() {
    for heap in 0..=6 {
        for page in 0..=4 {
            for pool in 0..=3 {
                for (c, v) in [(0, 0), (1, 1), (2, 2), (0, 2), (1, 3), (2, 3)] {
                    let p = props(heap, page, pool, c, v);
                    for f in flag_values() {
                        for committed in [false, true] {
                            assert_eq!(props_ok(p, f, committed), props_rule(heap, page, pool, c, v, f, committed), "{heap} {page} {pool} {c} {v} {f:#x} {committed}");
                        }
                    }
                }
            }
        }
    }
}

/// D3D12.MAKE.HEAP: CreateHeap takes the heap properties (as HEAP.PROPS, not committed), a size
/// from 1, at most 0xffff0000 on a heap the CPU maps and below 2^64 - 4MB on the others, and an
/// alignment of 0, 64KB or 4MB: every heap type, page property and pool, the sizes at the edges,
/// 7 alignments, every flags value asked
#[test]
fn heaps_take_the_clause() {
    let sizes = [0u64, 1, 65536, 0xffff_0000, 0xffff_0001, 1 << 32, 1 << 63, 0xffff_ffff_ffbf_ffff, 0xffff_ffff_ffc0_0000, u64::MAX];
    for heap in 0..=6 {
        for page in 0..=4 {
            for pool in 0..=3 {
                let p = props(heap, page, pool, 0, 0);
                for &size in &sizes {
                    let size_ok = size >= 1 && size < 0xffff_ffff_ffc0_0000 && !(cpu_mapped(heap, page) && size > 0xffff_0000);
                    for al in [0u64, 1, 4096, 65536, 131072, 4194304, 8388608] {
                        let al_ok = matches!(al, 0 | 65536 | 4194304);
                        for f in flag_values() {
                            let want = props_rule(heap, page, pool, 0, 0, f, false) && size_ok && al_ok;
                            assert_eq!(heapx_hr(p, size, al, f), if want { S_FALSE } else { E_INVALIDARG }, "{heap} {page} {pool} {size:#x} {al} {f:#x}");
                        }
                    }
                }
            }
        }
    }
    // (the flags the clause names: tier 2's deny groups, SHARED and ALLOW_SHADER_ATOMICS on the
    // GPU heaps, ALLOW_WRITE_WATCH on the CPU ones, HARDWARE_PROTECTED, CREATE_NOT_RESIDENT,
    // CREATE_NOT_ZEROED anywhere)
    let any = [0u32, 0x100, 0x800, 0x1000];
    for heap in [1u32, 2, 3, 5] {
        let p = Props::plain(Heap::of(heap));
        let cpu = heap != 1;
        for a in any {
            assert_eq!(heapx_hr(p, 65536, 0, a), S_FALSE);
            for deny in [0xc0, 0x84, 0x44] {
                assert_eq!(heapx_hr(p, 65536, 0, a | deny), S_FALSE);
            }
            assert_eq!(heapx_hr(p, 65536, 0, a | SHARED) == S_FALSE, !cpu);
            assert_eq!(heapx_hr(p, 65536, 0, a | ATOMICS) == S_FALSE, !cpu);
            assert_eq!(heapx_hr(p, 65536, 0, a | WRITE_WATCH) == S_FALSE, cpu);
        }
    }
}

/// a grid of descriptions: every dimension 0..5, sizes at the edges, 1 and 2 slices, mip levels
/// 0..2, every format id 0..191, 1 and 4 samples, layouts 0..3, the flags 0, RT, DS, UAV, RT|UAV,
/// ALLOW_CROSS_ADAPTER, USE_TIGHT_ALIGNMENT
fn descs(each: &mut dyn FnMut(&Desc)) {
    for dim in 0..=5 {
        for (w, h) in [(0u32, 1u32), (1, 1), (64, 64), (65, 33), (256, 1), (16384, 16384), (16385, 1)] {
            for d in [1u32, 2] {
                for mips in 0..=2 {
                    for fmt in 0..=191 {
                        for samples in [1u32, 4] {
                            for layout in 0..=3 {
                                for flags in [0u32, 1, 2, 4, 5, 0x10, 0x400] {
                                    each(&Desc { dim, align: 0, w, h, d, mips, fmt, samples, quality: 0, layout, flags });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// D3D12.ALLOC.INVALID: a texture description CreateCommittedResource refuses (on the default
/// heap, in COMMON), or a buffer description that is not valid, gives UINT64_MAX and 64KB; one it
/// takes a size below that, aligned to its alignment of 4KB, 64KB or 4MB; every description of
/// the grid
#[test]
fn refused_descriptions_allocate_nothing() {
    let mut n = 0;
    descs(&mut |d| {
        let (size, al) = alloc(d).unwrap();
        let taken = if d.dim == 1 { validate(d).is_ok() } else { create_hr(Heap::Default, 0, d).unwrap() == S_FALSE };
        if taken {
            assert!(size != u64::MAX && matches!(al, 4096 | 65536 | 4194304) && size % al == 0, "{d:?}: {size:#x} {al:#x}");
            n += 1;
        } else {
            assert_eq!((size, al), (u64::MAX, 65536), "{d:?}");
        }
    });
    assert!(n > 10_000);
}

/// D3D12.TIGHT.IGNORED: USE_TIGHT_ALIGNMENT (0x400) changes neither what CreateCommittedResource
/// takes nor the size and alignment, but a multisampled texture with it is refused: every
/// description of the grid with the flag added
#[test]
fn tight_alignment_is_ignored() {
    descs(&mut |d| {
        let t = Desc { flags: d.flags | 0x400, ..*d };
        let plain = Desc { flags: d.flags & !0x400, ..*d };
        if d.samples > 1 && d.dim != 1 {
            assert_eq!(create_hr(Heap::Default, 0, &t).unwrap(), E_INVALIDARG, "{d:?}");
            return;
        }
        assert_eq!(create_hr(Heap::Default, 0, &t).unwrap(), create_hr(Heap::Default, 0, &plain).unwrap(), "{d:?}");
        assert_eq!(alloc(&t).unwrap(), alloc(&plain).unwrap(), "{d:?}");
    });
}

/// D3D12.ALLOC.VIDEO: NV12 in its two planes, 1.5 bytes a texel, P010, P016 and 420_OPAQUE 3
/// (the luma's bytes and the chroma's at half the size each way, two of them), times the array,
/// rounded up to 64KB; every even size up to 512 by 256 (step 2 by 6), arrays 1..4
#[test]
fn video_formats_allocate_two_planes() {
    for fmt in 103..=106u32 {
        let b: u64 = if fmt == 103 { 1 } else { 2 };
        for w in (2..=512u32).step_by(2) {
            for h in (2..=256u32).step_by(6) {
                for d in 1..=4u32 {
                    let desc = Desc { dim: 3, align: 0, w, h, d, mips: 1, fmt, samples: 1, quality: 0, layout: 0, flags: 0 };
                    if create_hr(Heap::Default, 0, &desc).unwrap() != S_FALSE {
                        continue;
                    }
                    let (w, h) = (u64::from(w), u64::from(h));
                    let bytes = (w * h * b + (w / 2) * (h / 2) * 2 * b) * u64::from(d);
                    assert_eq!(alloc(&desc).unwrap(), (bytes.div_ceil(65536) * 65536, 65536), "{fmt} {w} {h} {d}");
                }
            }
        }
    }
}

/// D3D12.FEATURE.FORMAT: the table holds exactly the formats 0..115 but 66, and 189..191; NV11,
/// AI44 and IA44 (110..112) with planes but no support, every other with both; unknown ids none
/// (E_FAIL, E_INVALIDARG): every id 0..0x1ffff and 2^31, 0xfffffffe, 0xffffffff; the planes 2 of
/// exactly the two-plane formats (the D32S8 and D24S8 families, NV12, P010, P016, NV11); its
/// texture dimension bits (Support1 0x10, 0x20, 0x40) those CreateCommittedResource takes but for
/// the palette formats A8P8 and P8 (189, 190), which it does not make
#[test]
fn format_support_is_the_table() {
    let known = |f: u32| f <= 115 && f != 66 || (189..=191).contains(&f);
    for f in (0..0x2_0000u32).chain([1 << 31, 0xffff_fffe, 0xffff_ffff]) {
        let c = caps(f);
        assert_eq!(c.is_some(), known(f), "{f}");
        if let Some((s, planes)) = c {
            assert_eq!(s.is_none(), (110..=112).contains(&f), "{f}");
            assert!(planes >= 1);
            assert_eq!(planes == 2, matches!(f, 19..=22 | 44..=47 | 103..=105 | 110), "{f}");
            if let Some((s1, _)) = s {
                for (bit, t) in [(0x10, T1), (0x20, T2), (0x40, T3)] {
                    if f != 189 && f != 190 {
                        assert_eq!(s1 & bit != 0, support(f) & t != 0, "{f} {bit:#x}");
                    }
                }
            }
        }
    }
    assert_eq!(CAPS.len(), 118);
    assert!(support(189) == 0 && support(190) == 0);
}

/// D3D12.FEATURE.MSAA: for every format id 0..0x1ffff (and the ends of u32) and every sample
/// count 0..64, 2^31 and 0xffffffff: E_FAIL for an unknown format or 0 samples; 1 level for 1;
/// for 2, 4, 8, 16 one where the format has MULTISAMPLE_RENDERTARGET (0x200000; a typeless one:
/// its first typed format's), else 0; 0 for other counts
#[test]
fn quality_levels_are_the_clause() {
    let ms = |f: u32| caps(f).and_then(|c| c.0).is_some_and(|s| s.0 & 0x20_0000 != 0);
    for f in (0..0x2_0000u32).chain([1 << 31, 0xffff_ffff]) {
        for n in (0..=64u32).chain([1 << 31, 0xffff_ffff]) {
            let want = match n {
                _ if caps(f).is_none() || n == 0 => None,
                1 => Some(1),
                2 | 4 | 8 | 16 => Some(u32::from(ms(if TYPELESS.contains(&f) { f + 1 } else { f }))),
                _ => Some(0),
            };
            assert_eq!(quality_levels(f, n), want, "{f} {n}");
        }
    }
}

/// D3D12.CLEAR.FORMAT: ClearRenderTargetView's render target is not made for UNKNOWN, the block
/// compressed formats, the typeless ones and the unassigned ids: every id 0..0xffff and the
/// ends of u32; every format it is made for renders (Support1 RENDER_TARGET, 0x4000) and is one
/// texel of 1..16 bytes
#[test]
fn clears_refuse_formats_without_a_target() {
    for f in (0..0x1_0000u32).chain([1 << 31, 0xffff_ffff]) {
        let t = clear_texel(f, [0; 4]);
        let bc = block(f).is_some_and(|b| b.1 == 4);
        if f == 0 || bc || TYPELESS.contains(&f) || caps(f).is_none() {
            assert_eq!(t, None, "{f}");
        }
        if let Some(t) = t {
            let s1 = caps(f).and_then(|c| c.0).map_or(0, |s| s.0);
            assert!(s1 & 0x4000 != 0, "{f}");
            assert_eq!(Some(u32::try_from(t.len()).unwrap()), block(f).map(|b| b.0), "{f}");
        }
    }
}

/// D3D12.MAKE.PLACED: CreatePlacedResource in a heap CreateHeap took (heap12: alignment 0) takes
/// what CreateCommittedResource takes on that heap type, not of a kind the heap denies (buffers
/// 0x4, render target or depth stencil textures 0x40, other textures 0x80), at a multiple of the
/// resource's alignment and inside the heap, never multisampled (a heap of the default
/// alignment): every heap type 1..3, the deny flags none and each group, heap sizes 64KB, 4MB and
/// 16MB, offsets 0, 4KB, 64KB, 4MB and 12MB, the states COMMON and GENERIC_READ, every
/// description of the grid
#[test]
fn placed_resources_fit_their_heap() {
    descs(&mut |d| {
        for heap in [Heap::Default, Heap::Upload, Heap::Readback] {
            for hflags in [0u32, 0xc0, 0x84, 0x44] {
                for hsize in [65536u64, 4 << 20, 16 << 20] {
                    for off in [0u64, 4096, 65536, 4 << 20, 12 << 20] {
                        for state in [0u32, 0xac3] {
                            let c = create_hr(heap, state, d).unwrap();
                            let got = placed_hr(heap, hflags, hsize, off, state, d).unwrap();
                            let want = if c != S_FALSE {
                                c
                            } else {
                                let kind = if d.dim == 1 { 0x4 } else if d.flags & 3 != 0 { 0x40 } else { 0x80 };
                                let (size, al) = alloc(d).unwrap();
                                if hflags & kind != 0 || d.samples > 1 || off % al != 0 || off + size > hsize { E_INVALIDARG } else { S_OK }
                            };
                            assert_eq!(got, want, "{heap:?} {hflags:#x} {hsize} {off} {state:#x} {d:?}");
                        }
                    }
                }
            }
        }
    });
}

/// the typeless family of a format: (typeless, members)
const FAMILIES: &[(u32, &[u32])] = &[
    (1, &[2, 3, 4]), (5, &[6, 7, 8]), (9, &[10, 11, 12, 13, 14]), (15, &[16, 17, 18]), (19, &[20, 21, 22]),
    (23, &[24, 25]), (27, &[28, 29, 30, 31, 32]), (33, &[34, 35, 36, 37, 38]), (39, &[40, 41, 42, 43]),
    (44, &[45, 46, 47]), (48, &[49, 50, 51, 52]), (53, &[54, 55, 56, 57, 58, 59]), (60, &[61, 62, 63, 64]),
    (70, &[71, 72]), (73, &[74, 75]), (76, &[77, 78]), (79, &[80, 81]), (82, &[83, 84]), (90, &[87, 91]),
    (92, &[88, 93]), (94, &[95, 96]), (97, &[98, 99]),
];

/// D3D12.COMMITTED.CLEAR in the clause's words: a clear value only for a render target or depth
/// stencil texture, of a known typed format of the resource format's cast family (or the format
/// itself), that renders (a render target) or for a depth texture a depth format with the depth
/// from 0 to 1
fn clear_rule(c: Clear, d: &Desc) -> bool {
    let fam = |f: u32| FAMILIES.iter().find(|(t, m)| *t == f || m.contains(&f)).map(|x| x.0);
    let (f, z) = match c {
        Clear::None => return true,
        Clear::Color(f) => (f, None),
        Clear::Depth(f, z) => (f, Some(z)),
    };
    let same = match fam(d.fmt) {
        Some(t) => fam(f) == Some(t),
        None => f == d.fmt,
    };
    d.dim != 1 && d.flags & 3 != 0 && caps(f).is_some() && !TYPELESS.contains(&f) && same
        && (d.flags & 1 == 0 || support(f) & 8 != 0)
        && (d.flags & 2 == 0 || DEPTH.contains(&f) && z.is_none_or(|z| (0.0..=1.0).contains(&z)))
}

/// the formats an ALLOW_DISPLAY resource takes: not the 96 and 128-bit ones, R32G32, the
/// R32G8X24 and R24G8 families, A8_UNORM, R1_UNORM, the packed 4:2:2 RGB ones, block compressed,
/// B5G6R5, B5G5R5A1, the palette formats, B4G4R4A4, A4B4G4R4
fn displays(f: u32) -> bool {
    !(matches!(f, 1..=8 | 15..=22 | 44..=47 | 65 | 68 | 69 | 85 | 86 | 113..=115 | 191) || block(f).is_some_and(|b| b.1 == 4))
}

/// D3D12.COMMITTED.FLAGS in the clause's words, after the heap properties (HEAP.PROPS,
/// committed)
fn committed_rule(p: Props, flags: u32, state: u32, c: Clear, d: &Desc) -> Hr {
    let as_type = if p.heap == Heap::Custom { Heap::Default } else { p.heap };
    if flags & CROSS_ADAPTER != 0 || d.flags & 0x10 != 0 {
        let cross_fmt = matches!(d.fmt, 10 | 24 | 28 | 29 | 87 | 89 | 91);
        let ok = flags & CROSS_ADAPTER != 0 && d.flags & 0x10 != 0 && d.dim == 3 && d.layout == 1 && d.mips == 1 && d.d == 1 && d.samples == 1 && d.flags & !0x15 == 0 && cross_fmt;
        if !ok {
            return E_INVALIDARG;
        }
        let r = create_hr(as_type, state, &Desc { flags: d.flags & !0x10, layout: 0, ..*d }).unwrap();
        return if r != S_FALSE { r } else if clear_rule(c, d) { S_FALSE } else { E_INVALIDARG };
    }
    let r = create_hr(as_type, state, d).unwrap();
    if r != S_FALSE {
        return r;
    }
    let atomics = flags & ATOMICS == 0 || d.flags & 4 != 0;
    let display = flags & ALLOW_DISPLAY == 0 || d.dim == 3 && d.align == 0 && matches!(d.d, 1 | 2) && d.mips == 1 && d.samples == 1 && d.quality == 0 && d.layout == 0 && d.flags & 0xc2 == 0 && displays(d.fmt);
    if atomics && display && clear_rule(c, d) { S_FALSE } else { E_INVALIDARG }
}

/// D3D12.COMMITTED.FLAGS, D3D12.COMMITTED.CLEAR: CreateCommittedResource with the heap flags
/// none, SHARED, ALLOW_DISPLAY, SHARED | SHARED_CROSS_ADAPTER, ALLOW_SHADER_ATOMICS on the default
/// heap and a custom one of no CPU pages in L0, the clear values none, each format 0..191 as a
/// colour, and as a depth of 0, 1, 0.5, -0.5, 2 and NaN, every 2D description of the grid with
/// layouts 0 and 1 (and the 1D, 3D and buffer ones without a clear value)
#[test]
fn committed_flags_and_clears_are_the_clause() {
    let heaps = [props(1, 0, 0, 0, 0), props(4, 1, 1, 0, 0)];
    let hflags = [0u32, SHARED, ALLOW_DISPLAY, SHARED | CROSS_ADAPTER, ATOMICS];
    descs(&mut |d| {
        if d.layout > 1 {
            return;
        }
        for p in heaps {
            for &f in &hflags {
                if !props_ok(p, f, true) {
                    assert_eq!(committed_hr(p, f, 0, Clear::None, d).unwrap(), E_INVALIDARG);
                    continue;
                }
                let check = |c: Clear| assert_eq!(committed_hr(p, f, 0, c, d).unwrap(), committed_rule(p, f, 0, c, d), "{p:?} {f:#x} {c:?} {d:?}");
                check(Clear::None);
                if d.dim == 3 && d.d == 1 && d.mips == 1 && (d.w, d.h) == (64, 64) {
                    for cf in 0..=191 {
                        check(Clear::Color(cf));
                    }
                    for z in [0.0f32, 1.0, 0.5, -0.5, 2.0, f32::NAN] {
                        for cf in [d.fmt, 20, 40, 45, 55, 41] {
                            check(Clear::Depth(cf, z));
                        }
                    }
                }
            }
        }
    });
    let _ = (CpuPage::Unknown, Pool::Unknown);
}
