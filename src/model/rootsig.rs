//! Root signatures: D3D12SerializeVersionedRootSignature's blob, a DXBC container holding one RTS0
//! part (the layout of d3d12.h's D3D12_ROOT_SIGNATURE_DESC / _DESC1 flattened, offsets from the
//! part's start), and its checksum (DXBC's MD5 variant).

/// a root parameter's descriptor kind (D3D12_ROOT_PARAMETER_TYPE_CBV, _SRV, _UAV)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescKind {
    Cbv,
    Srv,
    Uav,
}

impl DescKind {
    fn ty(self) -> u32 {
        match self {
            DescKind::Cbv => 2,
            DescKind::Srv => 3,
            DescKind::Uav => 4,
        }
    }
}

/// D3D12_DESCRIPTOR_RANGE1 (Flags is 1.1's)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Range {
    pub ty: u32,
    pub n: u32,
    pub base: u32,
    pub space: u32,
    pub offset: u32,
    pub flags: u32,
}

/// D3D12_ROOT_PARAMETER1
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Param {
    Table { vis: u32, ranges: Vec<Range> },
    Constants { reg: u32, space: u32, n: u32, vis: u32 },
    Desc { kind: DescKind, reg: u32, space: u32, vis: u32, flags: u32 },
}

/// a static sampler as the executor makes it: wrap, LOD bias 0, anisotropy 16, NEVER, transparent
/// black, LOD 0..FLT_MAX
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sampler {
    pub filter: u32,
    pub reg: u32,
    pub space: u32,
    pub vis: u32,
}

/// D3D_ROOT_SIGNATURE_VERSION
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Version {
    V1_0,
    V1_1,
}

impl Version {
    /// D3D_ROOT_SIGNATURE_VERSION_1_0 (1) and _1_1 (2); None for the others
    pub fn of(v: u32) -> Option<Version> {
        match v {
            1 => Some(Version::V1_0),
            2 => Some(Version::V1_1),
            _ => None,
        }
    }
}

/// what the executor holds at most: parameters, static samplers, ranges in all tables
pub const MAX_PARAMS: usize = 16;
pub const MAX_SAMPLERS: usize = 16;
pub const MAX_RANGES: usize = 32;

/// the rootsig12 op's parameters (after its version and flags), as the executor reads them: None
/// where it calls the case malformed
pub fn parse(words: &[String]) -> Option<(Vec<Param>, Vec<Sampler>)> {
    let mut ps = Vec::new();
    let mut ss = Vec::new();
    let mut nr = 0;
    let mut i = 0;
    let at = |i: usize| words.get(i).and_then(|s| crate::case::num(s)).map(|v| v as u32);
    while i < words.len() {
        let kind = match words[i].as_str() {
            "cbv" => Some(DescKind::Cbv),
            "srv" => Some(DescKind::Srv),
            "uav" => Some(DescKind::Uav),
            _ => None,
        };
        match (words[i].as_str(), kind) {
            ("s", None) => {
                if ss.len() == MAX_SAMPLERS {
                    return None;
                }
                ss.push(Sampler { filter: at(i + 1)?, reg: at(i + 2)?, space: at(i + 3)?, vis: at(i + 4)? });
                i += 5;
                continue;
            }
            _ if ps.len() == MAX_PARAMS => return None,
            ("c", None) => {
                ps.push(Param::Constants { reg: at(i + 1)?, space: at(i + 2)?, n: at(i + 3)?, vis: at(i + 4)? });
                i += 5;
            }
            (_, Some(kind)) => {
                ps.push(Param::Desc { kind, reg: at(i + 1)?, space: at(i + 2)?, vis: at(i + 3)?, flags: at(i + 4)? });
                i += 5;
            }
            ("t", None) => {
                let vis = at(i + 1)?;
                let count = at(i + 2)? as usize;
                if count > MAX_RANGES || nr + count > MAX_RANGES || i + 3 + 6 * count > words.len() {
                    return None;
                }
                let mut ranges = Vec::new();
                for k in 0..count {
                    let j = i + 3 + 6 * k;
                    ranges.push(Range { ty: at(j)?, n: at(j + 1)?, base: at(j + 2)?, space: at(j + 3)?, offset: at(j + 4)?, flags: at(j + 5)? });
                }
                nr += count;
                ps.push(Param::Table { vis, ranges });
                i += 3 + 6 * count;
            }
            (_, None) => return None,
        }
    }
    Some((ps, ss))
}

/// D3D12SerializeVersionedRootSignature's verdict on a description: whether it serializes
/// (Microsoft Learn "Root Signature Limits", "Using Descriptor Tables", "D3D12_ROOT_DESCRIPTOR_FLAGS",
/// "D3D12_DESCRIPTOR_RANGE_FLAGS", "D3D12_ROOT_SIGNATURE_FLAGS")
pub fn valid(ver: Version, flags: u32, ps: &[Param], ss: &[Sampler]) -> bool {
    let v11 = ver == Version::V1_1;
    // (visibility ALL..MESH)
    let vis_ok = |v: u32| v <= 7;
    // (the root signature's cost: 64 DWORDs, a table 1, a constant 1, a root descriptor 2)
    let cost: u64 = ps
        .iter()
        .map(|p| match p {
            Param::Table { .. } => 1,
            Param::Constants { n, .. } => *n as u64,
            Param::Desc { .. } => 2,
        })
        .sum();
    // (DATA_VOLATILE, DATA_STATIC_WHILE_SET_AT_EXECUTE, DATA_STATIC: at most one)
    let data_ok = |f: u32| (f & 0xe).count_ones() <= 1;
    let params = ps.iter().all(|p| match p {
        Param::Table { vis, ranges } => {
            let sampler = ranges.iter().filter(|r| r.ty == 3).count();
            vis_ok(*vis)
                && !ranges.is_empty()
                && (sampler == 0 || sampler == ranges.len())
                && ranges.iter().all(|r| {
                    r.ty <= 3
                        && r.n != 0
                        && (!v11
                            || if r.ty == 3 {
                                r.flags & !1 == 0
                            } else {
                                r.flags & !(1 | 0xe | 0x10000) == 0 && data_ok(r.flags)
                            })
                })
        }
        Param::Constants { vis, .. } => vis_ok(*vis),
        Param::Desc { vis, flags, .. } => vis_ok(*vis) && (!v11 || (*flags & !0xe == 0 && data_ok(*flags))),
    });
    let samplers = ss.iter().all(|s| vis_ok(s.vis));
    cost <= 64 && params && samplers && flags & !0xfff == 0
}

fn put(v: &mut Vec<u8>, w: u32) {
    v.extend_from_slice(&w.to_le_bytes());
}

/// the RTS0 part: header (version, parameters and their offset, samplers and their offset, flags),
/// the parameters' (type, visibility, payload offset), the payloads in order (a table's range
/// count and offset, its ranges after), then the samplers
pub fn rts0(ver: Version, flags: u32, ps: &[Param], ss: &[Sampler]) -> Vec<u8> {
    let v11 = ver == Version::V1_1;
    let mut v = Vec::new();
    put(&mut v, if v11 { 2 } else { 1 });
    put(&mut v, ps.len() as u32);
    put(&mut v, 24);
    put(&mut v, ss.len() as u32);
    put(&mut v, 0); // (the samplers' offset, set below)
    put(&mut v, flags);
    let headers = v.len();
    for _ in ps {
        v.extend_from_slice(&[0; 12]);
    }
    for (k, p) in ps.iter().enumerate() {
        let (ty, vis) = match p {
            Param::Table { vis, .. } => (0, *vis),
            Param::Constants { vis, .. } => (1, *vis),
            Param::Desc { kind, vis, .. } => (kind.ty(), *vis),
        };
        let h = headers + 12 * k;
        let payload = v.len() as u32;
        v[h..h + 4].copy_from_slice(&ty.to_le_bytes());
        v[h + 4..h + 8].copy_from_slice(&vis.to_le_bytes());
        v[h + 8..h + 12].copy_from_slice(&payload.to_le_bytes());
        match p {
            Param::Table { ranges, .. } => {
                put(&mut v, ranges.len() as u32);
                put(&mut v, payload + 8);
                for r in ranges {
                    put(&mut v, r.ty);
                    put(&mut v, r.n);
                    put(&mut v, r.base);
                    put(&mut v, r.space);
                    if v11 {
                        put(&mut v, r.flags);
                    }
                    put(&mut v, r.offset);
                }
            }
            Param::Constants { reg, space, n, .. } => {
                put(&mut v, *reg);
                put(&mut v, *space);
                put(&mut v, *n);
            }
            Param::Desc { reg, space, flags, .. } => {
                put(&mut v, *reg);
                put(&mut v, *space);
                if v11 {
                    put(&mut v, *flags);
                }
            }
        }
    }
    let so = v.len() as u32;
    v[16..20].copy_from_slice(&so.to_le_bytes());
    for s in ss {
        for w in [s.filter, 1, 1, 1, 0, 16, 1, 0, 0, f32::MAX.to_bits(), s.reg, s.space, s.vis] {
            put(&mut v, w);
        }
    }
    v
}

/// the DXBC container of one part: magic, checksum, version 1, size, one part at offset 36
pub fn dxbc(tag: &[u8; 4], part: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"DXBC");
    v.extend_from_slice(&[0; 16]);
    put(&mut v, 1);
    put(&mut v, (44 + part.len()) as u32);
    put(&mut v, 1);
    put(&mut v, 36);
    v.extend_from_slice(tag);
    put(&mut v, part.len() as u32);
    v.extend_from_slice(part);
    let sum = checksum(&v[20..]);
    for (k, w) in sum.iter().enumerate() {
        v[4 + 4 * k..8 + 4 * k].copy_from_slice(&w.to_le_bytes());
    }
    v
}

fn md5_block(st: &mut [u32; 4], b: &[u8]) {
    const S: [u32; 64] = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
    let k = |i: usize| ((i as f64 + 1.0).sin().abs() * 4294967296.0) as u32;
    let m: Vec<u32> = b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
    let [mut a, mut bb, mut c, mut d] = *st;
    for i in 0..64 {
        let (f, g) = match i / 16 {
            0 => ((bb & c) | (!bb & d), i),
            1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
            2 => (bb ^ c ^ d, (3 * i + 5) % 16),
            _ => (c ^ (bb | !d), (7 * i) % 16),
        };
        let t = d;
        d = c;
        c = bb;
        bb = bb.wrapping_add(a.wrapping_add(f).wrapping_add(k(i)).wrapping_add(m[g]).rotate_left(S[i]));
        a = t;
    }
    st[0] = st[0].wrapping_add(a);
    st[1] = st[1].wrapping_add(bb);
    st[2] = st[2].wrapping_add(c);
    st[3] = st[3].wrapping_add(d);
}

/// DXBC's checksum of the bytes after the checksum: MD5's rounds, its padding replaced: the bit
/// count first in the last block, (bits >> 2) | 1 last
pub fn checksum(data: &[u8]) -> [u32; 4] {
    let mut st = [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476];
    let full = data.len() / 64 * 64;
    for b in data[..full].chunks(64) {
        md5_block(&mut st, b);
    }
    let rest = &data[full..];
    let bits = (data.len() as u32).wrapping_mul(8);
    let last = (bits >> 2) | 1;
    let mut b = [0u8; 64];
    if rest.len() >= 56 {
        b[..rest.len()].copy_from_slice(rest);
        b[rest.len()] = 0x80;
        md5_block(&mut st, &b);
        let mut c = [0u8; 64];
        c[..4].copy_from_slice(&bits.to_le_bytes());
        c[60..].copy_from_slice(&last.to_le_bytes());
        md5_block(&mut st, &c);
    } else {
        b[..4].copy_from_slice(&bits.to_le_bytes());
        b[4..4 + rest.len()].copy_from_slice(rest);
        b[4 + rest.len()] = 0x80;
        b[60..].copy_from_slice(&last.to_le_bytes());
        md5_block(&mut st, &b);
    }
    st
}
