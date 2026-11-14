//! Which resource and view descriptions Direct3D 10 and 11 accept, and what GetDesc gives back;
//! the format support each device reports.

use super::d3dres::{Dev, Resource};
use super::Res;
use super::Fail;

pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const E_FAIL: u32 = 0x8000_4005;
pub const S_FALSE: u32 = 1;

/// a resource description: dim 1..3 textures, 4 a buffer (w: bytes, q: the stride)
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desc {
    pub dim: u32,
    pub w: u32,
    pub h: u32,
    pub da: u32,
    pub mips: u32,
    pub fmt: u32,
    pub usage: u32,
    pub bind: u32,
    pub cpu: u32,
    pub misc: u32,
    pub count: u32,
    pub q: u32,
}

use super::d3dfmt_table::{Row, TABLES};
use crate::spec::hit;

fn level_name(l: u32) -> String {
    format!("{}_{}", l >> 12, l >> 8 & 0xf)
}

/// the measured answers of a device for a format
pub fn support(dev: Dev, f: u32) -> Row {
    let key = match dev {
        Dev::D11(l) => format!("d3d11res:fmt-{}", level_name(l)),
        Dev::D10 => "d3d10res:fmt-10".into(),
        Dev::D101(l) => format!("d3d10res:fmt-{}", level_name(l)),
    };
    let t = TABLES.iter().find(|t| t.0 == key).map_or(&[][..], |t| t.1);
    hit("D3DRES.FMT");
    *t.iter().find(|r| r.0 == f).unwrap_or(&(f, E_FAIL, 0, E_FAIL, 0, [0; 4]))
}

/// the line `fmt` prints
pub fn fmt_line(dev: Dev, f: u32) -> Res<String> {
    let r = support(dev, f);
    let mut s = if dev.d3d11() { format!("fmt {f} {:08x} {:x} {:08x} {:x} ms", r.1, r.2, r.3, r.4) } else { format!("fmt {f} {:08x} {:x} - - ms", r.1, r.2) };
    for q in r.5 {
        if q == E_INVALIDARG {
            s += &format!(" x{q:x}");
        } else {
            s += &format!(" {q}");
        }
    }
    Ok(s)
}

pub const BIND_VB: u32 = 1;
pub const BIND_IB: u32 = 2;
pub const BIND_CB: u32 = 4;
pub const BIND_SRV: u32 = 8;
pub const BIND_SO: u32 = 0x10;
pub const BIND_RTV: u32 = 0x20;
pub const BIND_DSV: u32 = 0x40;
pub const BIND_UAV: u32 = 0x80;

/// the support bits of a format and of every format of its typeless group (a typeless resource
/// can be bound through any of them)
pub fn group_support(dev: Dev, f: u32) -> u32 {
    let g = super::d3dres::group(f);
    let mut s = support(dev, f).2;
    if super::dxgifmt::format(f).is_some_and(|x| x.num == super::dxgifmt::Num::Typeless) {
        for m in 1..=132 {
            if super::d3dres::group(m) == g {
                s |= support(dev, m).2;
            }
        }
    }
    s
}

/// the support bits UAV binding goes by (MEASURED, tex-format-*): a typeless format's group's,
/// an sRGB format's UNORM sibling's, R10G10B10_XR_BIAS_A2's the R10G10B10A2 group's, and
/// B8G8R8X8_TYPELESS takes B8G8R8A8_UNORM's; on 2D textures only (tex-random-154)
pub fn uav_support(dev: Dev, f: u32, dim: u32) -> u32 {
    match f {
        89 => group_support(dev, 23),
        92 => group_support(dev, 92) | support(dev, 87).2,
        _ if dim != 2 => group_support(dev, f),
        29 | 72 | 75 | 78 | 99 => support(dev, f - 1).2,
        91 => support(dev, 87).2,
        93 => support(dev, 88).2,
        _ => group_support(dev, f),
    }
}

/// the largest size of each dimension: (1D/2D, 3D, array, cube)
fn limits(dev: Dev) -> (u32, u32, u32, u32) {
    match dev.level() {
        l if l >= 0xb000 => (16384, 2048, 2048, 16384),
        l if l >= 0xa000 => (8192, 2048, 512, 8192),
        0x9300 => (4096, 256, 1, 4096),
        _ => (2048, 256, 1, 512),
    }
}

/// the mip levels of a full chain
pub fn full_chain(d: &Desc) -> u32 {
    let m = match d.dim {
        1 => d.w,
        2 => d.w.max(d.h),
        _ => d.w.max(d.h).max(d.da),
    };
    32 - m.max(1).leading_zeros()
}

/// Ok(the description GetDesc gives) or Err(the HRESULT)
pub fn check_desc(dev: Dev, d: &Desc, init: bool) -> Res<Result<Desc, u32>> {
    hit("D3DRES.DESC");
    let bad = Err(E_INVALIDARG);
    let mut g = *d;
    // usage and CPU access
    // (MEASURED: default resources may have CPU access; other CPU bits refused)
    if d.cpu & !0x30000 != 0 {
        return Ok(bad);
    }
    match d.usage {
        // (MEASURED: not a depth stencil target)
        0 => {
            if d.cpu != 0 && d.bind & BIND_DSV != 0 {
                return Ok(bad);
            }
        }
        1 => {
            // (MEASURED: an immutable resource is bound somewhere, never as a target)
            if !init || d.cpu != 0 || d.bind == 0 || d.bind & (BIND_RTV | BIND_DSV | BIND_UAV | BIND_SO) != 0 {
                return Ok(bad);
            }
        }
        2 => {
            // (MEASURED: a dynamic resource is bound somewhere, never as a target)
            if d.cpu != 0x10000 || d.bind == 0 || d.bind & (BIND_RTV | BIND_DSV | BIND_UAV | BIND_SO) != 0 {
                return Ok(bad);
            }
        }
        3 => {
            if d.bind != 0 || d.cpu == 0 {
                return Ok(bad);
            }
        }
        // (MEASURED: usage 0xffffffff passes as staging does)
        0xffff_ffff if d.bind == 0 && d.cpu != 0 => {
            hit("D3DRES.DESC.USAGEFF");
        }
        _ => return Ok(bad),
    }
    if d.dim == 4 {
        return Ok(check_buf(dev, d));
    }
    let Some(f) = super::dxgifmt::format(d.fmt) else { return Ok(bad) };
    let s = group_support(dev, d.fmt);
    let (l2, l3, la, _lc) = limits(dev);
    let dimbit = [0, 0x10, 0x20, 0x40][d.dim as usize];
    if s & dimbit == 0 {
        return Ok(bad);
    }
    let (w, h, da) = (d.w, if d.dim == 1 { 1 } else { d.h }, d.da);
    if w == 0 || h == 0 || da == 0 {
        return Ok(bad);
    }
    let lim = if d.dim == 3 { l3 } else { l2 };
    if d.dim != 3 && da > la.max(1) {
        return Ok(bad);
    }
    if w >= 0x8000_0000 || h >= 0x8000_0000 || d.dim == 3 && da >= 0x8000_0000 {
        hit("D3DRES.DESC.SIZE");
        return Ok(bad);
    }
    if w > lim || h > lim || d.dim == 3 && da > l3 {
        // (MEASURED: 16385 accepted at 11_0; where the limit is, open)
        return Err(Fail::Unmodeled("D3DRES.DESC.BIG"));
    }
    // (MEASURED: the top level of a block format is whole blocks, tex-random-169)
    if w % f.bw != 0 || h % f.bh != 0 {
        hit("D3DRES.DESC.BLOCKSIZE");
        return Ok(bad);
    }
    let full = full_chain(&Desc { h, ..*d });
    if d.mips > full {
        return Ok(bad);
    }
    if d.mips == 0 {
        g.mips = full;
    }
    // (MEASURED: more than one mip needs the MIP bit, a cube the TEXTURECUBE bit and CPU
    // access the CPU_LOCKABLE bit; tex-format-89, 101, 106)
    // (MEASURED: Direct3D 10 drops TEXTURECUBE from a 1D or 3D description, d3d10res
    // tex-random-16)
    if !dev.d3d11() && d.dim != 2 && d.misc & 4 != 0 {
        hit("D3DRES.DESC.CUBE10");
        g.misc &= !4;
    }
    let cube = g.misc & 4 != 0;
    if g.mips > 1 && s & 0x1000 == 0 || cube && s & 0x80 == 0 || d.cpu != 0 && s & 0x20000 == 0 {
        return Ok(bad);
    }
    let bs = if d.fmt == 89 { group_support(dev, 23) } else { s };
    // binding
    if d.bind & !(BIND_SRV | BIND_RTV | BIND_DSV | BIND_UAV) != 0 {
        return Ok(bad);
    }
    for (b, need) in [(BIND_SRV, 0x100), (BIND_RTV, 0x4000), (BIND_DSV, 0x10000), (BIND_UAV, 0x2000000)] {
        if d.bind & b != 0 && (if b == BIND_UAV { uav_support(dev, d.fmt, d.dim) } else { bs }) & need == 0 {
            return Ok(bad);
        }
    }
    // (MEASURED: no 3D depth stencil, tex-random-7)
    if d.bind & BIND_DSV != 0 && (d.bind & (BIND_RTV | BIND_UAV) != 0 || d.dim == 3) {
        return Ok(bad);
    }
    // flags (MEASURED: GENERATE_MIPS 1, SHARED 2, TEXTURECUBE 4, RESOURCE_CLAMP 0x80,
    // SHARED_KEYEDMUTEX 0x100, GDI_COMPATIBLE 0x200, RESTRICTED_CONTENT 0x1000 on textures)
    hit("D3DRES.DESC.MISC");
    // (MEASURED: Direct3D 10 has GENERATE_MIPS, SHARED and TEXTURECUBE only, d3d10res
    // tex2-*-misc-*)
    let known = if dev.d3d11() { 1 | 2 | 4 | 0x80 | 0x100 | 0x200 | 0x1000 } else { 1 | 2 | 4 };
    if d.misc & !known != 0 {
        return Ok(bad);
    }
    // (MEASURED: RESTRICTED_CONTENT only on 2D textures)
    if d.misc & 0x1000 != 0 && d.dim != 2 {
        return Ok(bad);
    }
    // (MEASURED: a 3D default texture with CPU access has one mip)
    if d.dim == 3 && d.usage == 0 && d.cpu != 0 && g.mips != 1 {
        return Ok(bad);
    }
    if cube && (d.dim != 2 || da % 6 != 0 || w != h) {
        return Ok(bad);
    }
    if d.misc & 1 != 0 && d.bind & (BIND_SRV | BIND_RTV) != BIND_SRV | BIND_RTV {
        return Ok(bad);
    }
    if d.misc & 0x200 != 0 && !matches!(d.fmt, 87 | 90 | 91) {
        return Ok(bad);
    }
    if (d.usage == 3 || d.usage == 0xffff_ffff) && d.misc & (1 | 0x80 | 0x1000) != 0 {
        return Ok(bad);
    }
    // (MEASURED: not immutable restricted content, tex-random-30)
    if d.usage == 1 && d.misc & 0x1000 != 0 {
        return Ok(bad);
    }
    // (MEASURED: a multisampled default texture has no CPU access, tex-random-207)
    if d.usage == 0 && d.cpu != 0 && d.count > 1 {
        return Ok(bad);
    }
    // (dynamic: one mip, one array slice, nothing shared)
    // (MEASURED: Direct3D 10 wants the 1 written, not 0 for a one-mip chain, d3d10res
    // tex-random-218)
    if d.usage == 2 && (g.mips != 1 || !dev.d3d11() && d.mips != 1 || d.dim != 3 && da != 1 || d.misc & !4 != 0) {
        return Ok(bad);
    }
    // multisampling
    if d.dim == 2 {
        if d.count == 0 {
            return Ok(bad);
        }
        if d.count > 1 {
            let q = match d.count {
                2 => support(dev, d.fmt).5[0],
                4 => support(dev, d.fmt).5[1],
                8 => support(dev, d.fmt).5[2],
                16 => support(dev, d.fmt).5[3],
                _ => 0,
            };
            // (MEASURED: Direct3D 10.0 has no standard patterns and multisampled depth is not
            // read by shaders, d3d10res tex-random-123, tex2-f44-count-u0)
            let d10 = dev == Dev::D10 && (d.q >= 0xffff_fffe || d.bind & (BIND_SRV | BIND_DSV) == BIND_SRV | BIND_DSV);
            if d10 || q == 0 || q == E_INVALIDARG || d.q >= q && d.q < 0xffff_fffe || g.mips != 1 || d.usage != 0 || init {
                // (MEASURED: no initial data for a multisampled texture, tex-random-190)
                return Ok(bad);
            }
        } else if d.q != 0 && d.q < 0xffff_fffe && dev.d3d11() {
            // (MEASURED: Direct3D 10 takes any quality of one sample, tex2-*-quality-u0)
            // (MEASURED: one sample takes the standard and center patterns' quality)
            return Ok(bad);
        }
    }
    Ok(Ok(g))
}

/// a buffer's description (MEASURED, buf-*: one word swept at a time over a vertex, constant,
/// dynamic constant, shader resource, staging, raw and structured buffer)
fn check_buf(dev: Dev, d: &Desc) -> Result<Desc, u32> {
    hit("D3DRES.DESC.BUF");
    let bad = Err(E_INVALIDARG);
    let mut g = *d;
    let d11 = dev.d3d11();
    // (Direct3D 10 knows no UAV binding, and passes 0x400 as 11 does the video encoder's)
    let binds = if d11 { 1 | 2 | 4 | 8 | 0x10 | 0x20 | 0x80 | 0x400 } else { 1 | 2 | 4 | 8 | 0x10 | 0x20 | 0x400 };
    let (raw, st) = (d.misc & 0x20 != 0 && d11, d.misc & 0x40 != 0 && d11);
    let miscs = if d11 { 2 | 0x10 | 0x20 | 0x40 | 0x100 } else { 2 | 4 };
    if d.w == 0 || d.bind & !binds != 0 || d.misc & !miscs != 0 {
        return bad;
    }
    // (a constant buffer: 16-byte multiples, bound as nothing else, no indirect arguments)
    if d.bind & BIND_CB != 0 && (d.w % 16 != 0 || d.bind != BIND_CB || d.misc & 0x10 != 0) {
        return bad;
    }
    // (CPU access: default buffers only for shader resources and 11, as for textures)
    if d.usage == 0 && d.cpu != 0 && (!d11 || d.bind & (BIND_VB | BIND_IB | BIND_CB | BIND_SO) != 0 || d.bind & (BIND_SRV | BIND_UAV) == 0) {
        return bad;
    }
    // (Direct3D 10 passes TEXTURECUBE everywhere and drops it)
    let cube = if d11 { 0 } else { 4 };
    if d.usage == 2 && (d.bind & (BIND_SO | BIND_RTV | BIND_UAV) != 0 || d.misc & !(0x20 | 0x40 | cube) != 0) {
        return bad;
    }
    if d.usage == 1 && d.bind & BIND_UAV != 0 {
        return bad;
    }
    if d.usage == 3 && d.misc & !(2 | 0x100 | cube) != 0 {
        return bad;
    }
    // (raw: bound as a shader resource or UAV, maybe a vertex, index buffer or target, not streamed out or a constant buffer;
    // structured: only as those, the stride a multiple of 4 that divides the size; not both)
    if raw && (st || d.bind & (BIND_SRV | BIND_UAV) == 0 || d.bind & !(BIND_VB | BIND_IB | BIND_SRV | BIND_RTV | BIND_UAV) != 0) {
        return bad;
    }
    if st && (d.bind & !(BIND_SRV | BIND_UAV | 0x400) != 0 || d.q == 0 || d.q % 4 != 0 || d.q > 2048 || d.w % d.q != 0) {
        return bad;
    }
    if !st {
        g.q = 0;
    }
    g.misc &= if d11 { !0 } else { !4 };
    Ok(g)
}

/// the creation of a description check_desc passed is refused all the same (what the check
/// alone, without a resource pointer, does not see): its HRESULT
pub fn create_refused(dev: Dev, d: &Desc) -> Option<u32> {
    // (MEASURED: a 2D default texture the CPU reads and renders to is one subresource,
    // tex-random-115, 121; the check alone passes it, tex-random-230; 1D ones are made,
    // tex-random-118, 247)
    // (Direct3D 10 makes them, d3d10res tex-random-33)
    if dev.d3d11() && d.dim == 2 && d.usage == 0 && d.cpu & 0x20000 != 0 && d.bind & BIND_RTV != 0 && (d.mips != 1 || d.da != 1) {
        hit("D3DRES.DESC.CPURTV");
        return Some(E_INVALIDARG);
    }
    None
}

/// GetDesc's words of a resource
pub fn desc_words(dev: Dev, d: &Desc) -> Vec<u32> {
    match d.dim {
        1 => vec![d.w, d.mips, d.da, d.fmt, d.usage, d.bind, d.cpu, d.misc],
        2 => vec![d.w, d.h, d.mips, d.da, d.fmt, d.count, d.q, d.usage, d.bind, d.cpu, d.misc],
        3 => vec![d.w, d.h, d.da, d.mips, d.fmt, d.usage, d.bind, d.cpu, d.misc],
        _ if dev.d3d11() => vec![d.w, d.usage, d.bind, d.cpu, d.misc, d.q],
        _ => vec![d.w, d.usage, d.bind, d.cpu, d.misc],
    }
}

/// Ok(GetDesc's words) or Err(the HRESULT) for a view of `kind` (0 srv 1 rtv 2 dsv 3 uav)
pub fn check_view(_dev: Dev, _kind: u32, _r: &Resource, _words: Option<&[u32]>) -> Res<Result<Vec<u32>, u32>> {
    Err(Fail::Unmodeled("D3DRES.VIEW"))
}

/// the staging copy of subresource `sub` that `read` makes is refused: its HRESULT (the copy is
/// created as any staging texture is, so a mip smaller than a block is refused)
pub fn staging_refused(dev: Dev, r: &Resource, sub: u32) -> Res<Option<u32>> {
    let d = r.d;
    if d.dim == 4 {
        return Ok(None);
    }
    let m = sub % d.mips;
    let s = Desc {
        w: (d.w >> m).max(1),
        h: if d.dim == 1 { 1 } else { (d.h >> m).max(1) },
        da: if d.dim == 3 { (d.da >> m).max(1) } else { 1 },
        mips: 1,
        usage: 3,
        bind: 0,
        cpu: 0x20000,
        misc: 0,
        count: 1,
        q: 0,
        ..d
    };
    Ok(check_desc(dev, &s, false)?.err())
}
