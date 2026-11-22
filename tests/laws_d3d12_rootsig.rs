//! Root signatures, the feature scan and the copies of Direct3D 12 (spec/d3d12.txt
//! D3D12.ROOTSIG.*, FEATURE.SCAN, COPY.*): each law runs over the whole domain it names, so it
//! is a proof for that domain (kind exhaustive). The references are the clauses' words, apart
//! from the model's functions: the blob is read back by the clause's layout and its checksum is
//! MD5's rounds written here anew.
mod pairs;
use pairs::zip_eq;
use kikizake::model::d3d12_features::{answer, ANSWERS, UNEXPECTED};
use kikizake::model::rootsig::{checksum, cost, dxbc, rts0, valid, DescKind, Param, Range, Sampler, Version};

/// a case's answers from the model, as the executor prints them (x86 or x86-64)
fn run(arch: u32, calls: &str) -> Vec<String> {
    kikizake::model::set_arch(arch);
    let c = kikizake::case::parse_cases(&format!("case law\n{calls}end\n"));
    let r = kikizake::model::run_case(&c[0]).unwrap();
    kikizake::model::set_arch(32);
    r
}

fn sampler(filter: u32) -> Sampler {
    Sampler { filter, address: [1, 1, 1], bias: 0, aniso: 16, cmp: 1, border: 0, min_lod: 0, max_lod: f32::MAX.to_bits(), reg: 0, space: 0, vis: 0 }
}

/// D3D12_FILTER's defined values: point or linear min, mag, mip, or ANISOTROPIC, each with a
/// reduction (none, comparison, minimum, maximum)
fn defined_filter(f: u32) -> bool {
    [0u32, 1, 4, 5, 0x10, 0x11, 0x14, 0x15, 0x55].iter().any(|b| [0u32, 0x80, 0x100, 0x180].iter().any(|r| f == b | r))
}

/// D3D12.ROOTSIG.SAMPLER in the clause's words
fn sampler_rule(s: &Sampler) -> bool {
    let bias = f32::from_bits(s.bias);
    defined_filter(s.filter)
        && s.address.iter().all(|a| (1..=5).contains(a))
        && (-16.0..=15.99).contains(&bias)
        && s.aniso <= 16
        && (s.filter & 0x180 != 0x80 || (1..=8).contains(&s.cmp))
        && !f32::from_bits(s.min_lod).is_nan()
        && !f32::from_bits(s.max_lod).is_nan()
        && s.vis <= 7
}

/// D3D12.ROOTSIG.SAMPLER: a static sampler alone in a root signature of either version: every
/// filter 0..0x3ff and each bit alone; every three addresses 0..7; every MipLODBias whose low 12
/// bits are 0 and every one from -17 to 17 in steps of 2^-14; anisotropy 0..64; every comparison
/// function 0..10 with every filter's reduction; MinLOD and MaxLOD every float whose low 12 bits
/// are 0 (NaNs among them), MinLOD past MaxLOD; border colours 0..5 and 2^32 - 1; visibilities 0..9
#[test]
fn static_samplers_are_the_clause() {
    let check = |s: Sampler| {
        for ver in [Version::V1_0, Version::V1_1] {
            assert_eq!(valid(ver, 0, &[], &[s]), sampler_rule(&s), "{s:?}");
        }
    };
    for f in (0..0x400u32).chain((10..32).map(|b| 1 << b)) {
        check(sampler(f));
    }
    for a in 0..512u32 {
        check(Sampler { address: [a & 7, a >> 3 & 7, a >> 6], ..sampler(0x15) });
    }
    for h in 0..(1u32 << 20) {
        check(Sampler { bias: h << 12, ..sampler(0x15) });
        check(Sampler { min_lod: h << 12, ..sampler(0x15) });
        check(Sampler { max_lod: h << 12, ..sampler(0x15) });
    }
    for k in -(17 << 14)..=(17i32 << 14) {
        check(Sampler { bias: (k as f32 / 16384.0).to_bits(), ..sampler(0x15) });
    }
    for n in 0..=64 {
        check(Sampler { aniso: n, ..sampler(0x55) });
    }
    for r in [0u32, 0x80, 0x100, 0x180] {
        for c in 0..=10 {
            check(Sampler { cmp: c, ..sampler(0x15 | r) });
        }
    }
    check(Sampler { min_lod: 8.0f32.to_bits(), max_lod: 1.0f32.to_bits(), ..sampler(0) });
    for b in [0u32, 1, 2, 3, 4, 5, u32::MAX] {
        check(Sampler { border: b, ..sampler(0) });
    }
    for v in 0..=9 {
        check(Sampler { vis: v, ..sampler(0) });
    }
}

/// the register class of a range type (SRV t, UAV u, CBV b, SAMPLER s)
fn class(ty: u32) -> Option<u32> {
    (ty <= 3).then_some(ty)
}

/// D3D12.ROOTSIG.VALID for one range in a table: of a type SRV..SAMPLER, of at least one
/// descriptor, ending by the last register (UINT_MAX descriptors: unbounded); 1.1's flags: a
/// sampler range DESCRIPTORS_VOLATILE alone, the others DESCRIPTORS_VOLATILE (1), the DATA_*
/// ones (2, 4, 8) at most one, DESCRIPTORS_STATIC_KEEPING_BUFFER_BOUNDS_CHECKS (0x10000), not
/// with DESCRIPTORS_VOLATILE, and DESCRIPTORS_VOLATILE not with DATA_STATIC
fn range_rule(v11: bool, r: &Range) -> bool {
    let data = (r.flags & 0xe).count_ones() <= 1;
    let flags = !v11
        || if r.ty == 3 {
            r.flags & !1 == 0
        } else {
            r.flags & !0x1_000f == 0 && data && r.flags & 0x1_0001 != 0x1_0001 && r.flags & 9 != 9
        };
    class(r.ty).is_some() && r.n != 0 && (r.n == u32::MAX || u64::from(r.base) + u64::from(r.n) - 1 <= u64::from(u32::MAX)) && flags
}

/// D3D12.ROOTSIG.VALID: the root signature flags: every value 0..0x1fff and each bit 13..31
/// alone, with 0x80000000 too, an empty root signature of either version: the defined flags
/// (0xfff) and 0x80000000, LOCAL_ROOT_SIGNATURE (0x80) alone; versions 0..4; one table of one
/// range of every type 0..5, count 0, 1, 2, 2^32 - 1, base 0, 1, 2^32 - 2, 2^32 - 1, 1.1 flags
/// 0..0x1f, 0x10000..0x1001f and each bit alone, visibilities 0..9; every pair of range types in
/// one table; root descriptors of each kind, flags 0..0x1f and each bit alone, visibilities
/// 0..9; root constants of visibilities 0..9; every pair of bindings (a table's range of each
/// class, root constants, root descriptors, a static sampler) in spaces 0 and 1, visibilities
/// 0..7 each, of registers 0, 0..1, 1..2, 2, 5..unbounded: refused exactly where a register of a
/// class and space is bound twice where the visibilities meet (ALL, 0, meets every one)
#[test]
fn root_signatures_are_the_clause() {
    for flags in (0..0x2000u32).chain((13..32).map(|b| 1 << b)) {
        for f in [flags, flags | 0x8000_0000] {
            let want = f & !0x8000_0fff == 0 && (f & 0x80 == 0 || f == 0x80);
            for ver in [Version::V1_0, Version::V1_1] {
                assert_eq!(valid(ver, f, &[], &[]), want, "{f:#x}");
            }
        }
    }
    for v in 0..=4 {
        assert_eq!(Version::of(v).is_some(), v == 1 || v == 2);
    }
    let flag_values: Vec<u32> = (0..0x20u32).chain(0x1_0000..0x1_0020).chain((5..32).map(|b| 1 << b)).collect();
    for ty in 0..=5 {
        for n in [0u32, 1, 2, u32::MAX] {
            for base in [0u32, 1, u32::MAX - 1, u32::MAX] {
                for &flags in &flag_values {
                    for vis in 0..=9 {
                        let r = Range { ty, n, base, space: 0, offset: 0, flags };
                        for (ver, v11) in [(Version::V1_0, false), (Version::V1_1, true)] {
                            let want = vis <= 7 && range_rule(v11, &r);
                            assert_eq!(valid(ver, 0, &[Param::Table { vis, ranges: vec![r] }], &[]), want, "{r:?} {vis} {v11}");
                        }
                    }
                }
            }
        }
    }
    for a in 0..=4 {
        for b in 0..=4 {
            let r = |ty: u32, base: u32| Range { ty, n: 1, base, space: 0, offset: 0, flags: 0 };
            let t = Param::Table { vis: 0, ranges: vec![r(a, 0), r(b, 1)] };
            let mixed = (a == 3) != (b == 3);
            assert_eq!(valid(Version::V1_1, 0, &[t], &[]), a <= 3 && b <= 3 && !mixed, "{a} {b}");
        }
    }
    for kind in [DescKind::Cbv, DescKind::Srv, DescKind::Uav] {
        for &flags in &flag_values {
            for vis in 0..=9 {
                let p = Param::Desc { kind, reg: 0, space: 0, vis, flags };
                assert_eq!(valid(Version::V1_0, 0, std::slice::from_ref(&p), &[]), vis <= 7);
                assert_eq!(valid(Version::V1_1, 0, &[p], &[]), vis <= 7 && flags & !0xe == 0 && (flags & 0xe).count_ones() <= 1, "{flags:#x}");
            }
        }
    }
    for vis in 0..=9 {
        assert_eq!(valid(Version::V1_1, 0, &[Param::Constants { reg: 0, space: 0, n: 4, vis }], &[]), vis <= 7);
    }
    // (the bindings: (class t 0, u 1, b 2, s 3), how it is bound)
    let regs: [(u32, u32); 5] = [(0, 1), (0, 2), (1, 2), (2, 1), (5, u32::MAX)];
    for c1 in 0..7 {
        for c2 in 0..7 {
            for s1 in 0..2 {
                for s2 in 0..2 {
                    for v1 in 0..8 {
                        for v2 in 0..8 {
                            for &(b1, n1) in &regs {
                                for &(b2, n2) in &regs {
                                    let (ps, ss, cl1, hi1) = binding(c1, b1, n1, s1, v1, vec![], vec![]);
                                    let (ps, ss, cl2, hi2) = binding(c2, b2, n2, s2, v2, ps, ss);
                                    let meet = cl1 == cl2 && s1 == s2 && (v1 == v2 || v1 == 0 || v2 == 0) && b1 <= hi2 && b2 <= hi1;
                                    assert_eq!(valid(Version::V1_1, 0, &ps, &ss), !meet, "{c1} {b1} {n1} {s1} {v1} / {c2} {b2} {n2} {s2} {v2}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// a binding of kind k (0..3 a table's range of class k, 4 root constants, 5 a root SRV, 6 a
/// static sampler) of registers base .. base + n - 1 (a root parameter or sampler binds base
/// alone): the parameters and samplers with it added, its class and last register
fn binding(k: u32, base: u32, n: u32, space: u32, vis: u32, mut ps: Vec<Param>, mut ss: Vec<Sampler>) -> (Vec<Param>, Vec<Sampler>, u32, u32) {
    let hi = |n: u32| if n == u32::MAX { u32::MAX } else { base + n - 1 };
    match k {
        0..=3 => {
            ps.push(Param::Table { vis, ranges: vec![Range { ty: k, n, base, space, offset: 0, flags: 0 }] });
            (ps, ss, k, hi(n))
        }
        4 => {
            ps.push(Param::Constants { reg: base, space, n: 1, vis });
            (ps, ss, 2, base)
        }
        5 => {
            ps.push(Param::Desc { kind: DescKind::Srv, reg: base, space, vis, flags: 0 });
            (ps, ss, 0, base)
        }
        _ => {
            ss.push(Sampler { reg: base, space, vis, ..sampler(0) });
            (ps, ss, 3, base)
        }
    }
}

/// MD5 (RFC 1321): the compression of one 64-byte block
fn md5_compress(st: &mut [u32; 4], block: &[u8]) {
    const R: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];
    const K: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];
    let m = |i: usize| u32::from_le_bytes(block[4 * i..4 * i + 4].try_into().unwrap());
    let (mut a, mut b, mut c, mut d) = (st[0], st[1], st[2], st[3]);
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((b & c) | (!b & d), i),
            1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
            2 => (b ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (b | !d), (7 * i) % 16),
        };
        let x = a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m(g)).rotate_left(R[i / 16 * 4 + i % 4]);
        (a, b, c, d) = (d, b.wrapping_add(x), b, c);
    }
    for (s, v) in zip_eq(st.iter_mut(), [a, b, c, d]) {
        *s = s.wrapping_add(v);
    }
}

const MD5_START: [u32; 4] = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];

/// MD5 of `data` with RFC 1321's padding (to check md5_compress against the RFC's vectors)
fn md5(data: &[u8]) -> [u8; 16] {
    let mut m = data.to_vec();
    m.push(0x80);
    while m.len() % 64 != 56 {
        m.push(0);
    }
    m.extend_from_slice(&(data.len() as u64 * 8).to_le_bytes());
    let mut st = MD5_START;
    for b in m.chunks(64) {
        md5_compress(&mut st, b);
    }
    let mut out = [0u8; 16];
    for (k, w) in st.iter().enumerate() {
        out[4 * k..4 * k + 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// DXBC's checksum of `data`: MD5's rounds, the last block the bit count, the rest, 0x80, zeros,
/// (bits >> 2) | 1 last (two blocks where the rest leaves no room for the count)
fn dxbc_sum(data: &[u8]) -> [u32; 4] {
    let mut st = MD5_START;
    let full = data.len() - data.len() % 64;
    for b in data[..full].chunks(64) {
        md5_compress(&mut st, b);
    }
    let (rest, bits) = (&data[full..], (data.len() as u32).wrapping_mul(8));
    let mut tail = Vec::new();
    if rest.len() >= 56 {
        tail.extend_from_slice(rest);
        tail.push(0x80);
        tail.resize(64, 0);
        tail.extend_from_slice(&bits.to_le_bytes());
        tail.resize(124, 0);
    } else {
        tail.extend_from_slice(&bits.to_le_bytes());
        tail.extend_from_slice(rest);
        tail.push(0x80);
        tail.resize(60, 0);
    }
    tail.extend_from_slice(&((bits >> 2) | 1).to_le_bytes());
    for b in tail.chunks(64) {
        md5_compress(&mut st, b);
    }
    st
}

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// a root signature read back from its RTS0 part by the clause's layout
fn read_rts0(p: &[u8]) -> (u32, u32, Vec<Param>, Vec<Sampler>) {
    let (ver, np, po, ns, so, flags) = (word(p, 0), word(p, 4), word(p, 8), word(p, 12), word(p, 16), word(p, 20));
    assert_eq!(po, 24);
    let v11 = ver == 2;
    let mut ps = Vec::new();
    let mut end = 24 + 12 * np as usize;
    for k in 0..np as usize {
        let h = 24 + 12 * k;
        let (ty, vis, at) = (word(p, h), word(p, h + 4), word(p, h + 8) as usize);
        assert_eq!(at, end, "payloads in order");
        ps.push(match ty {
            0 => {
                let (n, ro) = (word(p, at), word(p, at + 4) as usize);
                assert_eq!(ro, at + 8);
                let size = if v11 { 24 } else { 20 };
                let ranges = (0..n as usize)
                    .map(|i| {
                        let r = ro + size * i;
                        let flags = if v11 { word(p, r + 16) } else { 0 };
                        Range { ty: word(p, r), n: word(p, r + 4), base: word(p, r + 8), space: word(p, r + 12), flags, offset: word(p, r + size - 4) }
                    })
                    .collect();
                end = ro + size * n as usize;
                Param::Table { vis, ranges }
            }
            1 => {
                end = at + 12;
                Param::Constants { reg: word(p, at), space: word(p, at + 4), n: word(p, at + 8), vis }
            }
            2..=4 => {
                end = at + if v11 { 12 } else { 8 };
                let kind = [DescKind::Cbv, DescKind::Srv, DescKind::Uav][ty as usize - 2];
                Param::Desc { kind, reg: word(p, at), space: word(p, at + 4), vis, flags: if v11 { word(p, at + 8) } else { 0 } }
            }
            _ => panic!("parameter type {ty}"),
        });
    }
    assert_eq!(so as usize, end, "the samplers after the payloads");
    let ss = (0..ns as usize)
        .map(|i| {
            let w = |k: usize| word(p, so as usize + 52 * i + 4 * k);
            Sampler { filter: w(0), address: [w(1), w(2), w(3)], bias: w(4), aniso: w(5), cmp: w(6), border: w(7), min_lod: w(8), max_lod: w(9), reg: w(10), space: w(11), vis: w(12) }
        })
        .collect();
    assert_eq!(p.len(), so as usize + 52 * ns as usize);
    (ver, flags, ps, ss)
}

/// D3D12.ROOTSIG.BLOB: md5_compress is MD5's (RFC 1321's vectors); for every root signature of
/// up to three parameters (each a table of 0..2 ranges, root constants, each root descriptor
/// kind) and up to two static samplers, both versions, flags 0 and 0x7f: the blob is a DXBC
/// container (magic, the checksum of the bytes after it as DXBC sums them, version 1, its size,
/// one part at 36) of one RTS0 part, read back by the clause's layout to the root signature; the
/// checksum is DXBC's for every length 0..300 of bytes
#[test]
fn blobs_are_dxbc_of_one_rts0_part() {
    let hex = |b: [u8; 16]| b.iter().map(|x| format!("{x:02x}")).collect::<String>();
    assert_eq!(hex(md5(b"")), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(hex(md5(b"abc")), "900150983cd24fb0d6963f7d28e17f72");
    assert_eq!(hex(md5(b"12345678901234567890123456789012345678901234567890123456789012345678901234567890")), "57edf4a22be3c955ac49da2e2107b67a");
    let data: Vec<u8> = (0..300u32).map(|i| (i * 37 + 11) as u8).collect();
    for n in 0..=300 {
        assert_eq!(checksum(&data[..n]), dxbc_sum(&data[..n]), "{n} bytes");
    }
    let r = |ty: u32, k: u32| Range { ty, n: k + 1, base: 3 * k, space: k, offset: 0xffff_ffff - k, flags: k };
    let kinds: Vec<Param> = vec![
        Param::Table { vis: 0, ranges: vec![] },
        Param::Table { vis: 1, ranges: vec![r(0, 0)] },
        Param::Table { vis: 5, ranges: vec![r(1, 1), r(2, 2)] },
        Param::Constants { reg: 4, space: 2, n: 7, vis: 3 },
        Param::Desc { kind: DescKind::Cbv, reg: 1, space: 0, vis: 0, flags: 2 },
        Param::Desc { kind: DescKind::Srv, reg: 2, space: 1, vis: 2, flags: 4 },
        Param::Desc { kind: DescKind::Uav, reg: 3, space: 9, vis: 7, flags: 8 },
    ];
    let samplers = [sampler(0x15), Sampler { reg: 7, space: 3, vis: 4, border: 2, ..sampler(0x95) }];
    let mut lists: Vec<Vec<Param>> = vec![vec![]];
    for _ in 0..3 {
        let more: Vec<Vec<Param>> = lists.iter().filter(|l| l.len() == lists.last().unwrap().len()).flat_map(|l| kinds.iter().map(move |k| [l.clone(), vec![k.clone()]].concat())).collect();
        lists.extend(more);
    }
    for ps in &lists {
        for ns in 0..=2 {
            let ss = &samplers[..ns];
            for (ver, vw) in [(Version::V1_0, 1u32), (Version::V1_1, 2)] {
                for flags in [0u32, 0x7f] {
                    let part = rts0(ver, flags, ps, ss);
                    let b = dxbc(b"RTS0", &part);
                    assert_eq!(&b[0..4], b"DXBC");
                    let sum = dxbc_sum(&b[20..]);
                    assert_eq!((0..4).map(|k| word(&b, 4 + 4 * k)).collect::<Vec<_>>(), sum.to_vec());
                    assert_eq!((word(&b, 20), word(&b, 24) as usize, word(&b, 28), word(&b, 32)), (1, b.len(), 1, 36));
                    assert_eq!((&b[36..40], word(&b, 40) as usize), (&b"RTS0"[..], b.len() - 44));
                    let (v, f, rps, rss) = read_rts0(&b[44..]);
                    let strip = |p: &Param| match p {
                        Param::Table { vis, ranges } => Param::Table { vis: *vis, ranges: ranges.iter().map(|r| Range { flags: if vw == 2 { r.flags } else { 0 }, ..*r }).collect() },
                        Param::Desc { kind, reg, space, vis, flags } => Param::Desc { kind: *kind, reg: *reg, space: *space, vis: *vis, flags: if vw == 2 { *flags } else { 0 } },
                        p => p.clone(),
                    };
                    assert_eq!((v, f), (vw, flags));
                    assert_eq!(rps, ps.iter().map(strip).collect::<Vec<_>>());
                    assert_eq!(rss, ss.to_vec());
                }
            }
        }
    }
    assert!(lists.len() > 300);
}

/// D3D12.ROOTSIG.MAKE: CreateRootSignature takes the serializer's blob of a root signature of
/// cost at most 64 DWORDs (a table 1, root constants their count, a root descriptor 2), and
/// refuses it past: every root signature of n tables, m root CBVs and root constants of c, for n
/// 0..66, m 0..33, c 0..66 with the cost at most 70, x86 and x86-64
#[test]
fn root_signatures_cost_at_most_64() {
    for n in 0..=66u64 {
        for m in 0..=33u64 {
            for c in 0..=66u64 {
                let want = n + 2 * m + c;
                if want > 70 || n + m + 1 > 64 {
                    continue;
                }
                let mut ps: Vec<Param> = (0..n).map(|_| Param::Table { vis: 0, ranges: vec![] }).collect();
                ps.extend((0..m).map(|k| Param::Desc { kind: DescKind::Cbv, reg: k as u32, space: 1, vis: 0, flags: 0 }));
                ps.push(Param::Constants { reg: 0, space: 2, n: c as u32, vis: 0 });
                assert_eq!(cost(&ps), want);
            }
        }
    }
    for arch in [32, 64] {
        for c in [0u32, 1, 62, 63, 64, 65, 66, 100] {
            for tables in 0..=2u32 {
                let line = format!("rootsig12 2 0{} c 0 0 {c} 0\n", " t 0 0".repeat(tables as usize));
                let got = run(arch, &format!("device12\n{line}"));
                let last = got.last().unwrap();
                let ok = c + tables <= 64;
                assert_eq!(last, &format!("rsmake hr {}", if ok { "00000000" } else { "80070057" }), "{arch} {c} {tables}");
            }
        }
    }
}

/// D3D12.FEATURE.SCAN: each feature answers at one structure size only (per process), its bytes
/// that size; the table's entries are unique; every feature id 0..0x10000 and the ends of u32
/// not in the table fails at every size (E_UNEXPECTED for PREDICATION's 50 and 52, else
/// E_INVALIDARG), on x86 and x86-64; the featscan12 answer is the table's for every entry
#[test]
fn features_answer_at_one_size() {
    for (i, a) in ANSWERS.iter().enumerate() {
        assert_eq!(a.4.len(), 2 * a.3 as usize, "feature {}", a.0);
        for b in &ANSWERS[i + 1..] {
            let overlap = a.2 == b.2 || format!("{:?}", a.2) == "Both" || format!("{:?}", b.2) == "Both";
            if a.0 == b.0 && overlap {
                assert_eq!(a.3, b.3, "feature {} at two sizes", a.0);
                assert!(a.1 != b.1, "feature {} twice", a.0);
            }
        }
    }
    for arch in [32u32, 64] {
        kikizake::model::set_arch(arch);
        for f in (0..0x1_0000u32).chain([1 << 31, u32::MAX]) {
            let listed = ANSWERS.iter().any(|a| a.0 == f);
            if !listed {
                assert_eq!(answer(f, &[]), None, "{f}");
                assert!(!UNEXPECTED.contains(&f) || f == 50 || f == 52);
            }
        }
        kikizake::model::set_arch(32);
        for a in ANSWERS {
            let arch_ok = format!("{:?}", a.2) == "Both" || (format!("{:?}", a.2) == "X86") == (arch == 32);
            if !arch_ok {
                continue;
            }
            let ins: String = a.1.iter().map(|v| format!(" {v:#x}")).collect();
            let got = run(arch, &format!("device12\nfeatscan12 {}{ins}\n", a.0));
            assert_eq!(got[1], format!("feat size {:x} hr 00000000 {}", a.3, a.4), "{arch} {}", a.0);
        }
        for f in [2u32, 50, 52, 59, 1000] {
            let got = run(arch, &format!("device12\nfeatscan12 {f}\n"));
            assert_eq!(got[1], format!("feat none hr {}", if UNEXPECTED.contains(&f) { "8000ffff" } else { "80070057" }));
        }
    }
}

/// D3D12.COPY.ROUNDTRIP, D3D12.COPY.D24: a copy from a buffer footprint to a texture and back
/// gives every byte of every row of every plane back, for every format 0..191 a 2D texture takes
/// (as a depth stencil one too), of sizes 1..5, 8, 17, 64 each way; but the D24S8 family's depth
/// plane keeps 24 bits: each texel's top byte comes back 0 (the bytes copied in are
/// (offset 7 + 3) mod 256, so a top byte that was 0 already does not count)
#[test]
fn copies_give_every_byte_back() {
    let sizes = [1u32, 2, 3, 4, 5, 8, 17, 64];
    let (mut checked, mut d24) = (0, 0);
    for f in 0..=191u32 {
        for &w in &sizes {
            for &h in &sizes {
                for ds in 0..=1 {
                    let got = run(64, &format!("device12\ncopy12 {f} {w} {h} {ds}\n"));
                    let l = &got[1];
                    if l.starts_with("copy hr") || l == "copy nofootprint" {
                        continue;
                    }
                    let diff = u64::from_str_radix(l.split(' ').nth(1).unwrap(), 16).unwrap();
                    if matches!(f, 44..=47) {
                        // (plane 0: rows of w texels of 4 bytes at a pitch of 256-byte multiples)
                        let pitch = (u64::from(w) * 4).div_ceil(256) * 256;
                        let zeroed = (0..u64::from(h)).flat_map(|r| (0..u64::from(w)).map(move |x| r * pitch + 4 * x + 3)).filter(|at| (at * 7 + 3) % 256 != 0).count();
                        assert_eq!(diff, zeroed as u64, "{f} {w} {h} {ds}");
                        d24 += 1;
                    } else {
                        assert_eq!(diff, 0, "{f} {w} {h} {ds}: {l}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 5000 && d24 > 200, "{checked} {d24}");
}
