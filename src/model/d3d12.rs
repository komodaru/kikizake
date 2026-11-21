//! Direct3D 12 resources and DXGI flip-model swap chains: an executable specification of
//! GetCopyableFootprints (the layout of every subresource in a buffer), GetResourceAllocationInfo
//! for buffers, CreateCommittedResource's validation of a description (ppvResource NULL), the
//! values ClearRenderTargetView and ClearDepthStencilView write, and of the DXGI calls of
//! runner/dxgi.c.
//!
//! Sources (Microsoft Learn): "D3D12_RESOURCE_DESC" (remarks: buffers, sizes, alignment, mip
//! levels, multisampling), "ID3D12Device::GetCopyableFootprints", "D3D12_SUBRESOURCE_FOOTPRINT"
//! (RowPitch a multiple of D3D12_TEXTURE_DATA_PITCH_ALIGNMENT), "D3D12_PLACED_SUBRESOURCE_FOOTPRINT"
//! (Offset a multiple of D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT), "ID3D12Device::GetResourceAllocationInfo"
//! (a buffer: the smallest multiple of 64KB), "ID3D12Device::CreateCommittedResource" (ppvResource
//! NULL: S_FALSE for a valid description), "Data Conversion Rules" (FLOAT to UNORM, SNORM, SRGB,
//! smaller floats, integers), "ID3D12GraphicsCommandList::ClearRenderTargetView" (denormals set
//! to 0, NaNs kept), "DXGI_SWAP_CHAIN_DESC1", "DXGI_SWAP_EFFECT", "DXGI_SCALING",
//! "IDXGIFactory2::CreateSwapChainForHwnd", "IDXGISwapChain::ResizeBuffers",
//! "IDXGIFactory::MakeWindowAssociation".

use crate::spec::clause;
use super::dxgi_format::{block, channels, dims, planar_depth, Chan, Enc, FLIP};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::resource_desc::Desc;
use crate::spec::hit;

/// an HRESULT: kept apart from the other numbers (printed as its 8 hex digits)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hr(pub u32);

impl std::fmt::LowerHex for Hr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::LowerHex::fmt(&self.0, f)
    }
}

pub const S_OK: Hr = Hr(0);
pub const S_FALSE: Hr = Hr(1);
pub const E_INVALIDARG: Hr = Hr(0x8007_0057);
pub const E_NOTIMPL: Hr = Hr(0x8000_4001);
pub const E_FAIL: Hr = Hr(0x8000_4005);
pub const DXGI_ERROR_DEVICE_REMOVED: Hr = Hr(0x887a_0005);
pub const E_OUTOFMEMORY: Hr = Hr(0x8007_000e);
pub const DXGI_ERROR_INVALID_CALL: Hr = Hr(0x887a_0001);
pub const DXGI_ERROR_NOT_FOUND: Hr = Hr(0x887a_0002);
pub const DXGI_ERROR_NOT_CURRENTLY_AVAILABLE: Hr = Hr(0x887a_002b);

/// D3D12_RESOURCE_DIMENSION: the description's dimension, the values past TEXTURE3D as Other
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dim {
    Unknown,
    Buffer,
    Tex1D,
    Tex2D,
    Tex3D,
    Other,
}

impl Dim {
    pub fn of(v: u32) -> Dim {
        match v {
            0 => Dim::Unknown,
            1 => Dim::Buffer,
            2 => Dim::Tex1D,
            3 => Dim::Tex2D,
            4 => Dim::Tex3D,
            _ => Dim::Other,
        }
    }

    /// a texture's bit in dxgi_format::dims (1D 1, 2D 2, 3D 4); None for the others
    pub fn tex_bit(self) -> Option<u32> {
        match self {
            Dim::Tex1D => Some(1),
            Dim::Tex2D => Some(2),
            Dim::Tex3D => Some(4),
            Dim::Unknown | Dim::Buffer | Dim::Other => None,
        }
    }
}

/// D3D12_HEAP_TYPE
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Heap {
    Default,
    Upload,
    Readback,
    Custom,
    GpuUpload,
    Other,
}

impl Heap {
    pub fn of(v: u32) -> Heap {
        match v {
            1 => Heap::Default,
            2 => Heap::Upload,
            3 => Heap::Readback,
            4 => Heap::Custom,
            5 => Heap::GpuUpload,
            _ => Heap::Other,
        }
    }

    /// a heap the CPU maps (upload, readback)
    pub fn cpu(self) -> bool {
        match self {
            Heap::Upload | Heap::Readback => true,
            Heap::Default | Heap::Custom | Heap::GpuUpload | Heap::Other => false,
        }
    }

    /// a type CreateHeap and CreateCommittedResource take with unknown page properties
    pub fn known(self) -> bool {
        match self {
            Heap::Default | Heap::Upload | Heap::Readback | Heap::GpuUpload => true,
            Heap::Custom | Heap::Other => false,
        }
    }
}

impl Desc {
    pub fn kind(&self) -> Dim {
        Dim::of(self.dim)
    }
}

fn align(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// the mip levels a full chain has
pub fn full_chain(d: &Desc) -> u32 {
    hit(clause::D3D12_DESC_MIPS);
    let m = match d.kind() {
        Dim::Tex3D => d.w.max(d.h).max(d.d),
        Dim::Tex2D => d.w.max(d.h),
        Dim::Unknown | Dim::Buffer | Dim::Tex1D | Dim::Other => d.w,
    };
    m.max(1).bit_width()
}

/// why a description is refused (the clause), or Ok: GetCopyableFootprints' checks of a description
/// (a buffer's are CreateCommittedResource's too; a texture's sizes CreateCommittedResource checks
/// its own way, create_hr)
pub fn validate(d: &Desc) -> Result<(), crate::spec::ClauseId> {
    let no = |c: bool, id: crate::spec::ClauseId| {
        hit(id);
        if c {
            Err(id)
        } else {
            Ok(())
        }
    };
    no(matches!(d.kind(), Dim::Unknown | Dim::Other), clause::D3D12_DESC_DIM)?;
    if d.kind() == Dim::Buffer {
        no(d.align != 0 && d.align != 65536, clause::D3D12_DESC_BUFFER)?;
        no(d.h != 1 || d.d != 1 || d.mips != 1 || d.fmt != 0 || d.samples != 1 || d.quality != 0 || d.layout != 1 || d.w == 0, clause::D3D12_DESC_BUFFER)?;
        return Ok(());
    }
    no(d.w == 0 || d.h == 0 || d.d == 0, clause::D3D12_DESC_SIZE)?;
    let (wmax, hmax, dmax) = match d.kind() {
        Dim::Tex1D => (16384, 1, 2048),
        Dim::Tex2D => (16384, 16384, 2048),
        Dim::Tex3D | Dim::Unknown | Dim::Buffer | Dim::Other => (2048, 2048, 2048),
    };
    no(d.d > dmax || d.h > hmax && d.kind() == Dim::Tex1D, clause::D3D12_DESC_SIZE)?;
    // (MEASURED: GetCopyableFootprints checks the 1D and 3D size limits, not those of 2D
    // textures (golden d3d12-* fp-edge-*), nor the mip count)
    hit(clause::D3D12_FOOTPRINT_LIMITS);
    no(d.kind() != Dim::Tex2D && (d.w > wmax || d.h > hmax), clause::D3D12_DESC_SIZE)?;
    no(!matches!(d.align, 0 | 4096 | 65536 | 4194304), clause::D3D12_DESC_ALIGN)?;
    no(d.samples == 0, clause::D3D12_DESC_MSAA)?;
    if d.samples > 1 || d.quality > 0 {
        no(d.kind() != Dim::Tex2D || d.flags & 3 == 0 || d.mips != 1 || d.flags & 0x20 != 0 || d.layout == 3, clause::D3D12_DESC_MSAA)?;
    }
    no(d.layout > 3, clause::D3D12_DESC_LAYOUT)?;
    Ok(())
}

/// the layout of one subresource: (offset from the first, format, width, height, depth, row
/// pitch, rows, row size)
type Fp = (u64, u32, u32, u32, u32, u64, u32, u64);

/// what GetCopyableFootprints writes for an invalid description: every field all ones
fn invalid_fps(count: u32) -> (Vec<Fp>, u64) {
    hit(clause::D3D12_FOOTPRINT_INVALID);
    ((0..count).map(|_| (u64::MAX, !0, !0, !0, !0, u64::MAX, !0, u64::MAX)).collect(), u64::MAX)
}

/// GetCopyableFootprints' layouts and total, or why the model has none
pub fn footprints(first: u32, count: u32, base: u64, d: &Desc) -> Res<(Vec<Fp>, u64)> {
    if validate(d).is_err() {
        return Ok(invalid_fps(count));
    }
    if d.kind() == Dim::Buffer {
        hit(clause::D3D12_FOOTPRINT_BUFFER);
        // (MEASURED: Width 0xffffffff is invalid; 0xffffff01 not, its row pitch wrapping to 0,
        // golden d3d12-* fp-buffer, fp-edge-buffer-*)
        if d.w == u32::MAX {
            return Ok(invalid_fps(count));
        }
        if first as u64 + count as u64 > 1 {
            hit(clause::D3D12_FOOTPRINT_RANGE);
            return Ok(invalid_fps(count));
        }
        let w = d.w as u64;
        let v = (0..count).map(|_| (base, 0, d.w, 1, 1, align(w, 256), 1, w)).collect();
        return Ok((v, if count == 0 { 0 } else { w }));
    }
    let dm = dims(d.fmt);
    hit(clause::D3D12_FORMAT_DIMS);
    // (no texture bit: Unknown and Other, which validate refuses as well)
    if d.kind().tex_bit().is_none_or(|bit| dm & bit == 0) {
        return Ok(invalid_fps(count));
    }
    // (MEASURED: the two-texel formats want an even width, the packed video ones no mip levels)
    if matches!(d.fmt, 89 | 101 | 107..=109 | 113 | 114) && d.mips != 1 {
        hit(clause::D3D12_FOOTPRINT_PAIRS);
        return Ok(invalid_fps(count));
    }
    if matches!(d.fmt, 68 | 69 | 107..=109) && d.w % 2 != 0 {
        hit(clause::D3D12_FOOTPRINT_PAIRS);
        return Ok(invalid_fps(count));
    }
    // (MEASURED, golden d3d12-64 create-split: NV12, P010, P016 in two planes, luma R8 or R16 and
    // chroma R8G8 or R16G16 at half size, rows 512 apart, one mip level and an even size)
    if matches!(d.fmt, 103..=105) {
        hit(clause::D3D12_FOOTPRINT_VIDEO);
        let array = d.d;
        if d.mips != 1 || d.w % 2 != 0 || d.h % 2 != 0 || first as u64 + count as u64 > 2 * array as u64 {
            return Ok(invalid_fps(count));
        }
        let b = if d.fmt == 103 { 1 } else { 2 };
        let (mut v, mut at, mut total) = (Vec::new(), 0u64, 0);
        for i in first..first + count {
            let (fmt, w, h, row) = if i < array { ([60, 53][b as usize - 1], d.w, d.h, d.w as u64 * b) } else { ([48, 33][b as usize - 1], d.w / 2, d.h / 2, d.w as u64 * b) };
            let pitch = align(row, 512);
            v.push((base + at, fmt, w, h, 1, pitch, h, row));
            total = at + pitch * (h as u64 - 1) + row;
            at = align(at + pitch * h as u64, 512);
        }
        return Ok((v, total));
    }
    // (MEASURED: plane 0 of the depth stencil formats is R32_TYPELESS, its rows 512 apart)
    let (fmt, (bytes, bw, bh), pitch_align) = if planar_depth(d.fmt) {
        hit(clause::D3D12_FOOTPRINT_PLANAR);
        (39, (4, 1, 1), 512)
    } else {
        // (every format of a texture bit that is not two planes has a block:
        // tests dims_have_blocks)
        let Some(b) = block(d.fmt) else {
            return Ok(invalid_fps(count));
        };
        (d.fmt, b, 256)
    };
    let mips = if d.mips == 0 {
        hit(clause::D3D12_FOOTPRINT_MIPS0);
        full_chain(d)
    } else {
        d.mips
    };
    let array = if d.kind() == Dim::Tex3D { 1 } else { d.d };
    // (the two-plane depth formats: the stencil plane after the depth one)
    let planes = if planar_depth(d.fmt) { 2 } else { 1 };
    if first as u64 + count as u64 > (mips * array) as u64 * planes {
        hit(clause::D3D12_FOOTPRINT_RANGE);
        return Ok(invalid_fps(count));
    }
    hit(clause::D3D12_FOOTPRINT_LAYOUT);
    let mut v = Vec::new();
    let mut at = 0u64;
    let mut total = 0;
    for i in first..first + count {
        let m = i % mips;
        let w = (d.w >> m).max(1);
        let h = if d.kind() == Dim::Tex1D { 1 } else { (d.h >> m).max(1) };
        let depth = if d.kind() == Dim::Tex3D { (d.d >> m).max(1) } else { 1 };
        if bw > 1 || bh > 1 {
            hit(clause::D3D12_FOOTPRINT_BLOCK);
        }
        let (fw, fh) = (align(w as u64, bw as u64) as u32, align(h as u64, bh as u64) as u32);
        let rows = fh / bh;
        // (the stencil plane R8_TYPELESS, a byte a texel, rows 512 apart as the depth plane's)
        let stencil = i >= mips * array;
        if stencil {
            hit(clause::D3D12_FOOTPRINT_STENCIL);
        }
        let (fmt, bytes) = if stencil { (60, 1) } else { (fmt, bytes) };
        let row = (fw / bw) as u64 * bytes as u64;
        let pitch = align(row, pitch_align);
        v.push((base + at, fmt, fw, fh, depth, pitch, rows, row));
        total = at + pitch * (rows as u64 * depth as u64 - 1) + row;
        at = align(at + pitch * rows as u64 * depth as u64, 512);
    }
    Ok((v, total))
}

fn f32of(v: u32) -> f32 {
    f32::from_bits(v)
}

/// FLOAT to UNORM n
pub fn to_unorm(c: f32, n: u32) -> u64 {
    if c.is_nan() {
        return 0;
    }
    let m = ((1u64 << n) - 1) as f32;
    // (MEASURED: the product rounded to nearest even, not plus 0.5 truncated)
    (c.clamp(0.0, 1.0) * m).round_ties_even() as u64
}

/// FLOAT to SNORM n (two's complement, n bits)
pub fn to_snorm(c: f32, n: u32) -> u64 {
    if c.is_nan() {
        return 0;
    }
    let m = ((1u64 << (n - 1)) - 1) as f32;
    let c = c.clamp(-1.0, 1.0) * m;
    let v = if c >= 0.0 { (c + 0.5) as i64 } else { (c - 0.5) as i64 };
    (v as u64) & ((1u64 << n) - 1)
}

/// FLOAT to SRGB n
pub fn to_srgb(c: f32, n: u32) -> u64 {
    if c.is_nan() {
        return 0;
    }
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    to_unorm(s, n)
}

/// FLOAT to a smaller float of `e` exponent and `m` mantissa bits, signed or not: round to zero,
/// too large to the largest finite, infinities kept, NaN to a NaN
pub fn to_small_float(v: u32, e: u32, m: u32, signed: bool) -> u64 {
    let sign = (v >> 31) as u64;
    let exp = (v >> 23 & 0xff) as i32;
    let man = v & 0x7f_ffff;
    let emax = (1u64 << e) - 1;
    let bias = (1i32 << (e - 1)) - 1;
    let sbit = if signed { sign << (e + m) } else { 0 };
    if exp == 255 {
        if man != 0 {
            // (NaN; MEASURED: the unsigned ones all ones)
            if !signed {
                return (1 << (e + m)) - 1;
            }
            return sbit | emax << m | ((man >> (23 - m)) as u64).max(1 << (m - 1));
        }
        if sign == 1 && !signed {
            return 0;
        }
        return sbit | emax << m;
    }
    if sign == 1 && !signed {
        return 0;
    }
    if exp == 0 {
        return sbit;
    }
    let ue = exp - 127 + bias;
    if ue >= emax as i32 {
        return sbit | (emax - 1) << m | ((1 << m) - 1);
    }
    if ue <= 0 {
        // (a denormal of the target, or 0)
        let full = (man | 0x80_0000) as u64;
        let shift = (23 - m as i32) + 1 - ue;
        if shift > 24 {
            return sbit;
        }
        return sbit | full >> shift;
    }
    sbit | (ue as u64) << m | (man >> (23 - m)) as u64
}

/// FLOAT to a UINT or SINT of n bits: NaN 0, round to nearest even, clamped
pub fn to_int(c: f32, n: u32, signed: bool) -> u64 {
    if c.is_nan() {
        return 0;
    }
    // (MEASURED: truncated, toward zero)
    let r = (c as f64).trunc();
    let (lo, hi) = if signed { (-(1i64 << (n - 1)) as f64, ((1i64 << (n - 1)) - 1) as f64) } else { (0.0, ((1u64 << n) - 1) as f64) };
    let v = r.clamp(lo, hi) as i64;
    (v as u64) & if n == 64 { !0 } else { (1u64 << n) - 1 }
}

fn chan_value(ch: &Chan, v: u32) -> u64 {
    let c = f32of(v);
    match ch.enc {
        Enc::Float if ch.bits == 32 => {
            hit(clause::D3D12_CLEAR_FLOAT32);
            v as u64
        }
        Enc::Float if ch.bits == 16 => {
            hit(clause::D3D12_CLEAR_FLOAT16);
            to_small_float(v, 5, 10, true)
        }
        Enc::Float => {
            hit(clause::D3D12_CLEAR_FLOAT11);
            to_small_float(v, 5, ch.bits - 5, false)
        }
        Enc::Unorm => {
            hit(clause::D3D12_CLEAR_UNORM);
            to_unorm(c, ch.bits)
        }
        Enc::Snorm => {
            hit(clause::D3D12_CLEAR_SNORM);
            to_snorm(c, ch.bits)
        }
        Enc::Srgb => {
            hit(clause::D3D12_CLEAR_SRGB);
            to_srgb(c, ch.bits)
        }
        Enc::Uint => {
            hit(clause::D3D12_CLEAR_INT);
            to_int(c, ch.bits, false)
        }
        Enc::Sint => {
            hit(clause::D3D12_CLEAR_INT);
            to_int(c, ch.bits, true)
        }
        Enc::Unused => {
            hit(clause::D3D12_CLEAR_X);
            0
        }
        Enc::Alpha => {
            hit(clause::D3D12_CLEAR_XALPHA);
            to_unorm(c, ch.bits)
        }
    }
}

/// the input a clear takes: denormals as 0 (of their sign), NaNs kept
pub fn clear_input(v: u32) -> u32 {
    if v & 0x7f80_0000 == 0 && v & 0x7f_ffff != 0 {
        hit(clause::D3D12_CLEAR_DENORM);
        return v & 0x8000_0000;
    }
    v
}

/// the bytes of a texel of format f cleared to `rgba` (float bits)
pub fn clear_texel(f: u32, rgba: [u32; 4]) -> Option<Vec<u8>> {
    let chans = channels(f)?;
    let (bytes, _, _) = block(f)?;
    let mut t = 0u128;
    for ch in &chans {
        let v = chan_value(ch, clear_input(rgba[ch.src]));
        t |= (v as u128) << ch.at;
    }
    Some(t.to_le_bytes()[..bytes as usize].to_vec())
}

/// the planes of a depth stencil texel cleared to `depth` (float bits) and `stencil`
pub fn clear_depth(f: u32, depth: u32, stencil: u32) -> Option<Vec<Vec<u8>>> {
    hit(clause::D3D12_CLEARDSV);
    let raw = f32of(clear_input(depth));
    let d = if raw.is_nan() {
        hit(clause::D3D12_CLEARDSV_NAN);
        0.0
    } else {
        raw.clamp(0.0, 1.0)
    };
    let s = (stencil & 0xff) as u8;
    Some(match f {
        // (MEASURED: a float depth is kept as given, not clamped)
        40 => vec![raw.to_bits().to_le_bytes().to_vec()],
        55 => vec![(to_unorm(d, 16) as u16).to_le_bytes().to_vec()],
        45 => vec![(to_unorm(d, 24) as u32).to_le_bytes().to_vec(), vec![s]],
        20 => vec![raw.to_bits().to_le_bytes().to_vec(), vec![s]],
        _ => return None,
    })
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn hex64(v: u64) -> String {
    format!("{v:x}")
}

#[derive(Default)]
pub struct D3d12 {
    dev: bool,
    /// RemoveDevice called (remove12)
    removed: bool,
    factory: bool,
    /// the swap chain: its description (GetDesc1's 12 fields) and back buffer index
    chain: Option<([u32; 12], u32)>,
    /// its maximum frame latency
    latency: u32,
    /// no frame presented since the chain was made or resized (re/dxgi: DO_NOT_SEQUENCE and a
    /// partial Present1 refused then)
    fresh: bool,
}

fn words(t: &[String]) -> Res<Vec<u32>> {
    t.iter().map(|s| num(s).map(|v| v as u32).ok_or(Fail::Bad)).collect()
}

impl D3d12 {
    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        match name {
            "device12" => {
                hit(clause::D3D12_DEVICE);
                self.dev = true;
                self.chain = None;
                out.push("device12 ok".into());
                return Ok(true);
            }
            "footprint12" | "alloc12" | "create12" | "clear12" | "cleardsv12" | "feature12" | "msq12" | "incr12" | "copy12" | "heap12" | "dheap12" | "queue12" | "cmdlist12" | "qheap12" | "fence12" | "placed12" | "rootsig12" | "remove12" | "featscan12" | "committed12" | "heapx12" if !self.dev => {
                out.push("nodev".into());
                return Ok(true);
            }
            "remove12" => {
                if t.len() != 1 {
                    return Err(Fail::Bad);
                }
                hit(clause::D3D12_CLEAR_REMOVED);
                self.removed = true;
                out.push("remove12 ok".into());
            }
            "rootsig12" => {
                use super::rootsig::{checksum, cost, dxbc, parse, rts0, valid, Version, MAXTOK};
                // (the executor reads the first MAXTOK tokens of a line)
                let t = &t[..t.len().min(MAXTOK)];
                if t.len() < 3 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..3])?;
                let (ps, ss) = parse(&t[3..]).ok_or(Fail::Empty)?;
                hit(clause::D3D12_ROOTSIG_VALID);
                let ver = match Version::of(w[0]) {
                    Some(v) if valid(v, w[1], &ps, &ss) => v,
                    Some(_) | None => {
                        out.push(format!("rs hr {E_INVALIDARG:08x}"));
                        return Ok(true);
                    }
                };
                // (the blob: a DXBC container of one RTS0 part, its checksum DXBC's)
                hit(clause::D3D12_ROOTSIG_BLOB);
                let b = dxbc(b"RTS0", &rts0(ver, w[1], &ps, &ss));
                debug_assert_eq!(checksum(&b[20..]).iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<u8>>(), b[4..20]);
                out.push(format!("rs hr {S_OK:08x} size {:x}", b.len()));
                for c in b[20..].chunks(64) {
                    out.push(format!("rs {}", c.iter().map(|x| format!("{x:02x}")).collect::<String>()));
                }
                out.push(format!("rs sum {}", b[4..20].iter().map(|x| format!("{x:02x}")).collect::<String>()));
                hit(clause::D3D12_ROOTSIG_MAKE);
                out.push(format!("rsmake hr {:08x}", if cost(&ps) <= 64 { S_OK } else { E_INVALIDARG }));
            }
            "footprint12" => {
                if t.len() != 15 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                if w[1] > 16 {
                    return Err(Fail::Bad);
                }
                let (fps, total) = footprints(w[0], w[1], w[2] as u64, &Desc::from_abi_words(&w[3..]).ok_or(Fail::Bad)?)?;
                for f in fps {
                    out.push(format!("fp {} {:x} {:x} {:x} {:x} {:x} {:x} {}", hex64(f.0), f.1, f.2, f.3, f.4, f.5 as u32, f.6, hex64(f.7)));
                }
                out.push(format!("total {}", hex64(total)));
            }
            "alloc12" => {
                if t.len() != 12 {
                    return Err(Fail::Bad);
                }
                let d = Desc::from_abi_words(&words(&t[1..])?).ok_or(Fail::Bad)?;
                let (size, al) = alloc(&d)?;
                out.push(format!("alloc {size:x} {al:x}"));
            }
            "create12" => {
                if t.len() != 14 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let (heap, state, d) = (w[0], w[1], Desc::from_abi_words(&w[2..]).ok_or(Fail::Bad)?);
                out.push(format!("create hr {:08x}", create_hr(Heap::of(heap), state, &d)?));
            }
            "clear12" => {
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                if self.removed {
                    // (MEASURED: a removed device makes no resource, golden d3d12rm-*)
                    hit(clause::D3D12_CLEAR_REMOVED);
                    out.push(format!("clear12 hr {DXGI_ERROR_DEVICE_REMOVED:08x}"));
                    return Ok(true);
                }
                match clear_texel(w[0], [w[1], w[2], w[3], w[4]]) {
                    Some(b) => out.push(format!("clear12 {}", hex(&b))),
                    None => {
                        hit(clause::D3D12_CLEAR_FORMAT);
                        out.push(format!("clear12 hr {E_INVALIDARG:08x}"));
                    }
                }
            }
            "cleardsv12" => {
                if t.len() != 4 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                match clear_depth(w[0], w[1], w[2]) {
                    Some(p) => out.push(format!("cleardsv12 {}", p.iter().map(|b| hex(b)).collect::<Vec<_>>().join(" "))),
                    None => {
                        hit(clause::D3D12_CLEAR_FORMAT);
                        out.push(format!("cleardsv12 hr {E_INVALIDARG:08x}"));
                    }
                }
            }
            "featscan12" => {
                if t.len() < 2 || t.len() > 18 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                hit(clause::D3D12_FEATURE_SCAN);
                out.push(match super::d3d12_features::answer(w[0], &w[1..]) {
                    Some((size, bytes)) => format!("feat size {size:x} hr {S_OK:08x} {bytes}"),
                    None if super::d3d12_features::UNEXPECTED.contains(&w[0]) => format!("feat none hr {:08x}", Hr(0x8000_ffff)),
                    None => format!("feat none hr {E_INVALIDARG:08x}"),
                });
            }
            "feature12" | "msq12" | "incr12" => {
                if t.len() != match name {
                    "incr12" => 1,
                    "msq12" => 3,
                    _ => 2,
                } {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                match name {
                    "incr12" => {
                        // (MEASURED, golden d3d12-* incr: CBV/SRV/UAV 64, sampler 64, RTV and DSV
                        // 32 and 8 bytes in a 64-bit process, 16 and 4 in a 32-bit one)
                        hit(clause::D3D12_FEATURE_INCREMENT);
                        out.push(if super::arch() == 64 { "incr 40 40 20 8" } else { "incr 40 40 10 4" }.into());
                    }
                    "msq12" => {
                        hit(clause::D3D12_FEATURE_MSAA);
                        out.push(match super::dxgi_format::quality_levels(w[0], w[1]) {
                            Some(n) => format!("msq {n:x}"),
                            None => format!("msq hr {E_FAIL:08x}"),
                        });
                    }
                    _ => {
                        hit(clause::D3D12_FEATURE_FORMAT);
                        let c = super::dxgi_format::caps(w[0]);
                        let s = match c.and_then(|c| c.0) {
                            Some((a, b)) => format!("fsup {a:x} {b:x}"),
                            None => format!("fsup hr {E_FAIL:08x}"),
                        };
                        out.push(match c {
                            Some((_, p)) => format!("{s} planes {p:x}"),
                            None => format!("{s} planes hr {E_INVALIDARG:08x}"),
                        });
                    }
                }
            }
            "copy12" => {
                if t.len() != 5 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                if w[1] > 4096 || w[2] > 4096 {
                    return Err(Fail::Bad);
                }
                let d = Desc { flags: if w[3] != 0 { 2 } else { 0 }, ..Desc::tex(3, w[1], w[2], 1, 1, w[0]) };
                let c = create_hr(Heap::Default, 0x400, &d)?;
                if c != S_FALSE {
                    out.push(format!("copy hr {c:08x}"));
                    return Ok(true);
                }
                // (a copy to a texture and back gives every byte of every row back)
                hit(clause::D3D12_COPY_ROUNDTRIP);
                let planes = super::dxgi_format::caps(w[0]).map_or(1, |c| c.1).clamp(1, 2);
                let (fps, total) = footprints(0, planes, 0, &d)?;
                if total == u64::MAX {
                    // (the executor does not copy through a layout GetCopyableFootprints lacks)
                    out.push("copy nofootprint".into());
                    return Ok(true);
                }
                let byte = |at: u64| (at * 7 + 3) as u8;
                // (MEASURED, golden d3d12-* copy-44..47: the depth plane of the D24S8 family keeps 24
                // bits, the top byte of each texel comes back 0)
                let d24 = matches!(w[0], 44..=47);
                if d24 {
                    hit(clause::D3D12_COPY_D24);
                }
                let back = |at: u64| if d24 && at < fps[0].0 + fps[0].5 * fps[0].6 as u64 && (at - fps[0].0) % fps[0].5 % 4 == 3 { 0 } else { byte(at) };
                let mut diff = 0;
                for fp in &fps {
                    for r in 0..fp.6 as u64 {
                        for k in 0..fp.7 {
                            let at = fp.0 + r * fp.5 + k;
                            diff += (back(at) != byte(at)) as u32;
                        }
                    }
                }
                let first: String = (0..fps[0].7.min(16)).map(|k| format!("{:02x}", back(k))).collect();
                out.push(format!("copy {diff:x} first {first}"));
            }
            "committed12" => {
                if t.len() != 25 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let c = match w[7] {
                    0 => Clear::None,
                    1 => Clear::Color(w[8]),
                    2 => Clear::Depth(w[8], f32::from_bits(w[9])),
                    _ => return Err(Fail::Bad),
                };
                let d = Desc::from_abi_words(&w[13..]).ok_or(Fail::Bad)?;
                out.push(format!("committed12 hr {:08x}", committed_hr(Props::of(&w[..5]), w[5], w[6], c, &d)?));
            }
            "heapx12" => {
                if t.len() != 10 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let size = u64::from(w[5]) | u64::from(w[6]) << 32;
                out.push(format!("heapx12 hr {:08x}", heapx_hr(Props::of(&w[..5]), size, u64::from(w[7]), w[8])));
            }
            "heap12" | "dheap12" | "queue12" | "cmdlist12" | "qheap12" | "fence12" | "placed12" => {
                let n = match name {
                    "heap12" => 5,
                    "dheap12" | "queue12" => 4,
                    "cmdlist12" => 2,
                    "placed12" => 17,
                    _ => 3,
                };
                if t.len() != n {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let ok = |b: bool| if b { S_OK } else { E_INVALIDARG };
                let hr = match name {
                    "heap12" => heap_hr(Heap::of(w[0]), w[1] as u64, w[2] as u64, w[3]),
                    "dheap12" => {
                        // (MEASURED, golden d3d12-* make-*: no count checked (0 and 1000001 taken),
                        // shader visible only CBV/SRV/UAV and samplers, no other flag)
                        hit(clause::D3D12_MAKE_DESCHEAP);
                        if w[0] > 3 {
                            return Err(Fail::Bad);
                        }
                        ok(w[2] & !1 == 0 && (w[2] == 0 || w[0] <= 1))
                    }
                    "queue12" => {
                        // (MEASURED: direct, compute and copy queues, not bundles nor the video
                        // ones; priorities normal and high; global realtime not available to the
                        // process; DISABLE_GPU_TIMEOUT taken)
                        hit(clause::D3D12_MAKE_QUEUE);
                        if w[1] == 10000 && w[0] == 0 && w[2] == 0 {
                            DXGI_ERROR_NOT_CURRENTLY_AVAILABLE
                        } else {
                            ok(matches!(w[0], 0 | 2 | 3) && matches!(w[1], 0 | 100) && w[2] & !1 == 0)
                        }
                    }
                    "cmdlist12" => {
                        // (MEASURED: direct, bundle, compute, copy; the allocator refuses the others)
                        hit(clause::D3D12_MAKE_LIST);
                        if w[0] > 3 {
                            out.push(format!("cmdlist12 allocator hr {E_INVALIDARG:08x}"));
                            return Ok(true);
                        }
                        S_OK
                    }
                    "qheap12" => {
                        // (MEASURED: every type but video decode statistics (4), at least one query)
                        hit(clause::D3D12_MAKE_QUERYHEAP);
                        let [kind, count, ..] = w.as_slice() else { return Err(Fail::Bad) };
                        let valid = *kind <= 7 && *kind != 4 && *count > 0;
                        // Repeated Windows runs at the x86 allocation boundary:
                        // make-285 and make-293 succeeded in a77fef4 and ran out of
                        // memory in be51a9f, with matching executor source hashes.
                        if super::arch() == 32 && *count == QHEAP32_OOM && matches!(*kind, 0 | 3) {
                            out.push("oneof qheap12 hr 00000000 | qheap12 hr 8007000e".into());
                            return Ok(true);
                        }
                        if valid && super::arch() == 32 && *count >= QHEAP32_OOM {
                            E_OUTOFMEMORY
                        } else {
                            ok(valid)
                        }
                    }
                    "fence12" => {
                        // (MEASURED: SHARED taken, alone or with SHARED_CROSS_ADAPTER (make-238); the
                        // cross-adapter one alone and the non-monitored one not; a new fence has
                        // completed its initial value)
                        hit(clause::D3D12_MAKE_FENCE);
                        if matches!(w[1], 0 | 1 | 3) {
                            out.push(format!("fence12 hr 00000000 value {:x}", w[0]));
                        } else {
                            out.push(format!("fence12 hr {E_INVALIDARG:08x}"));
                        }
                        return Ok(true);
                    }
                    _ => {
                        let h = heap_hr(Heap::of(w[0]), w[2] as u64, 0, w[1]);
                        if h != S_OK {
                            out.push(format!("placed12 heap hr {h:08x}"));
                            return Ok(true);
                        }
                        placed_hr(Heap::of(w[0]), w[1], w[2] as u64, w[3] as u64, w[4], &Desc::from_abi_words(&w[5..]).ok_or(Fail::Bad)?)?
                    }
                };
                out.push(format!("{name} hr {hr:08x}"));
            }
            _ => return self.dxgi(op, out),
        }
        Ok(true)
    }
}

/// D3D12_RESOURCE_DESC (Alignment): 4KB is allowed when the most detailed mip level is at most
/// 16 tiles of 4KB, in the standard tile shapes of the format's element size
pub fn small(d: &Desc) -> bool {
    hit(clause::D3D12_DESC_SMALL);
    let Some((bytes, bw, bh)) = block(d.fmt) else { return false };
    let (w, h) = (d.w.div_ceil(bw) as u64, d.h.div_ceil(bh) as u64);
    // (MEASURED, golden d3d12-* alloc-edge-small-*-rgb32-*: 12-byte elements as 16-byte ones)
    let i = bytes.next_power_of_two().trailing_zeros() as usize;
    if i > 4 {
        return false;
    }
    let tiles = match d.kind() {
        Dim::Tex1D => w.div_ceil(4096 / bytes as u64),
        Dim::Tex2D => {
            let (tw, th) = [(64, 64), (64, 32), (32, 32), (32, 16), (16, 16)][i];
            w.div_ceil(tw) * h.div_ceil(th)
        }
        _ => {
            let (tw, th, td) = [(16, 16, 16), (16, 8, 16), (16, 8, 8), (8, 8, 8), (8, 8, 4)][i];
            w.div_ceil(tw) * h.div_ceil(th) * (d.d as u64).div_ceil(td)
        }
    };
    tiles <= 16
}

/// GetResourceAllocationInfo: (size, alignment); an invalid description gives UINT64_MAX
pub fn alloc(d: &Desc) -> Res<(u64, u64)> {
    if d.kind() == Dim::Buffer {
        if validate(d).is_err() {
            hit(clause::D3D12_ALLOC_INVALID);
            return Ok((u64::MAX, 65536));
        }
        hit(clause::D3D12_ALLOC_BUFFER);
        return Ok((align(d.w as u64, 65536), 65536));
    }
    if create_hr(Heap::Default, 0, d)? != S_FALSE {
        hit(clause::D3D12_ALLOC_INVALID);
        return Ok((u64::MAX, 65536));
    }
    // (MEASURED, golden d3d12-64 alloc-other: both 64KB swizzles in whole 64KB tiles of the
    // standard shapes, each mip level its own tiles)
    if d.layout == 2 || d.layout == 3 {
        // (MEASURED, golden d3d12-* alloc-std-planar-*: standard swizzle of the two-plane formats, D24S8
        // as 4 bytes, D32S8 as 8, NV12 as its two planes in tiles of their own)
        if matches!(d.fmt, 103..=106) {
            hit(clause::D3D12_ALLOC_SWIZZLE);
            let b = if matches!(d.fmt, 104 | 105) { 2 } else { 1 };
            let tiles = |w: u32, h: u32, bytes: u32| -> u64 {
                let (tw, th) = if bytes == 1 { (256, 256) } else if bytes == 2 { (256, 128) } else { (128, 128) };
                (w as u64).div_ceil(tw) * (h as u64).div_ceil(th)
            };
            return Ok(((tiles(d.w, d.h, b) + tiles(d.w / 2, d.h / 2, 2 * b)) * d.d as u64 * 65536, 65536));
        }
        let (bytes, bw, bh) = match d.fmt {
            19..=22 => (8, 1, 1),
            44..=47 => (4, 1, 1),
            f => match block(f) {
                Some(b) => b,
                // (CreateCommittedResource refuses a swizzle of the other formats)
                None => {
                    hit(clause::D3D12_ALLOC_INVALID);
                    return Ok((u64::MAX, 65536));
                }
            },
        };
        hit(clause::D3D12_ALLOC_SWIZZLE);
        let (tw, th, td) = match (d.kind() == Dim::Tex2D, bytes) {
            (true, 1) => (256, 256, 1),
            (true, 2) => (256, 128, 1),
            (true, 4) => (128, 128, 1),
            (true, 8) => (128, 64, 1),
            (true, _) => (64, 64, 1),
            (_, 1) => (64, 32, 32),
            (_, 2) => (32, 32, 32),
            (_, 4) => (32, 32, 16),
            (_, 8) => (32, 16, 16),
            _ => (16, 16, 16),
        };
        let mips = if d.mips == 0 { full_chain(d) } else { d.mips };
        let (array, depth) = if d.kind() == Dim::Tex3D { (1, d.d) } else { (d.d, 1) };
        let tiles: u64 = (0..mips).map(|m| ((d.w >> m).max(1).div_ceil(bw) as u64).div_ceil(tw) * ((d.h >> m).max(1).div_ceil(bh) as u64).div_ceil(th) * ((depth >> m).max(1) as u64).div_ceil(td)).sum();
        return Ok((tiles * array as u64 * 65536, 65536));
    }
    assert_eq!(d.layout, 0, "CreateCommittedResource refuses the other layouts");
    // (MEASURED, golden d3d12-64 alloc-other: NV12 in its two planes, 1.5 bytes a texel; the
    // two-plane depth stencil formats, D32_FLOAT_S8X24 as 8 bytes, D24_UNORM_S8 as 4)
    let (bytes, bw, bh) = match d.fmt {
        19..=22 => (8, 1, 1),
        44..=47 => (4, 1, 1),
        103..=106 => {
            hit(clause::D3D12_ALLOC_VIDEO);
            let b = if d.fmt == 103 { 1 } else { 2 };
            let (w, h) = (d.w as u64, d.h as u64);
            let total = (w * h * b + (w / 2) * (h / 2) * 2 * b) * d.d as u64;
            return Ok((align(total, 65536), 65536));
        }
        _ => match block(d.fmt) {
            Some(b) => b,
            None => unreachable!("CreateCommittedResource refuses textures of the other formats"),
        },
    };
    // (MEASURED, golden d3d12-32 alloc-texture-*, 165 descriptions of 9 formats: WARP packs every
    // subresource tightly, rows of whole elements, one after the other)
    hit(clause::D3D12_ALLOC_TEXTURE);
    let mips = if d.mips == 0 { full_chain(d) } else { d.mips };
    let array = if d.kind() == Dim::Tex3D { 1 } else { d.d };
    let mut total = 0u64;
    for _ in 0..array {
        for m in 0..mips {
            let w = (d.w >> m).max(1).div_ceil(bw) as u64;
            let h = if d.kind() == Dim::Tex1D { 1 } else { (d.h >> m).max(1).div_ceil(bh) as u64 };
            let depth = if d.kind() == Dim::Tex3D { (d.d >> m).max(1) as u64 } else { 1 };
            total += w * bytes as u64 * h * depth;
        }
    }
    if d.samples > 1 {
        // (MEASURED, golden d3d12-64 alloc-other, 7 descriptions: every sample stored, in 4MB, or
        // in 64KB when that is the alignment asked)
        hit(clause::D3D12_ALLOC_MSAA);
        let al = if d.align == 65536 { 65536 } else { 4194304 };
        return Ok((align(total * d.samples as u64, al), al));
    }
    if d.kind() == Dim::Tex1D && d.flags & 1 != 0 {
        // (FITTED, golden d3d12-64 alloc-rt1d, 10 descriptions of 4 formats: a 1D render target
        // takes twice the tight size)
        hit(clause::D3D12_ALLOC_RT1D);
        total *= 2;
    }
    let al = if d.align == 4096 { 4096 } else { 65536 };
    Ok((align(total, al), al))
}

/// CreateHeap's verdict on a heap type, size, alignment and flags (MEASURED, golden d3d12-* make-*)
/// the fewest queries a 32-bit process's CreateQueryHeap runs out of memory for (the first of
/// the measured counts that did)
const QHEAP32_OOM: u32 = 31 << 18;

/// D3D12_CPU_PAGE_PROPERTY
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CpuPage {
    Unknown,
    NotAvailable,
    WriteCombine,
    WriteBack,
    Other,
}

impl CpuPage {
    pub fn of(v: u32) -> CpuPage {
        match v {
            0 => CpuPage::Unknown,
            1 => CpuPage::NotAvailable,
            2 => CpuPage::WriteCombine,
            3 => CpuPage::WriteBack,
            _ => CpuPage::Other,
        }
    }
}

/// D3D12_MEMORY_POOL
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pool {
    Unknown,
    L0,
    L1,
    Other,
}

impl Pool {
    pub fn of(v: u32) -> Pool {
        match v {
            0 => Pool::Unknown,
            1 => Pool::L0,
            2 => Pool::L1,
            _ => Pool::Other,
        }
    }
}

/// D3D12_HEAP_PROPERTIES
#[derive(Clone, Copy, Debug)]
pub struct Props {
    pub heap: Heap,
    pub page: CpuPage,
    pub pool: Pool,
    pub creation: u32,
    pub visible: u32,
}

impl Props {
    /// a heap type with no page properties and node masks 0 (what heap12 and create12 pass)
    pub fn plain(heap: Heap) -> Props {
        Props { heap, page: CpuPage::Unknown, pool: Pool::Unknown, creation: 0, visible: 0 }
    }

    pub fn of(w: &[u32]) -> Props {
        Props { heap: Heap::of(w[0]), page: CpuPage::of(w[1]), pool: Pool::of(w[2]), creation: w[3], visible: w[4] }
    }

    /// the CPU maps it: upload, readback, GPU upload, or a custom heap of write-combined or
    /// written-back pages
    fn cpu_access(self) -> bool {
        match self.heap {
            Heap::Upload | Heap::Readback | Heap::GpuUpload => true,
            Heap::Custom => matches!(self.page, CpuPage::WriteCombine | CpuPage::WriteBack),
            Heap::Default | Heap::Other => false,
        }
    }

    /// the heap type resource creation sees: a custom heap as the default one
    fn as_type(self) -> Heap {
        match self.heap {
            Heap::Custom => Heap::Default,
            Heap::Default | Heap::Upload | Heap::Readback | Heap::GpuUpload | Heap::Other => self.heap,
        }
    }
}

/// D3D12_HEAP_FLAGS bits
mod hflag {
    pub const SHARED: u32 = 0x1;
    pub const ALLOW_DISPLAY: u32 = 0x8;
    pub const CROSS_ADAPTER: u32 = 0x20;
    pub const WRITE_WATCH: u32 = 0x200;
    pub const ATOMICS: u32 = 0x400;
    pub const DENY: u32 = 0xc4;
    /// the bits a heap flag may have (0x2, 0x10, 0x4000 and up are none)
    pub const KNOWN: u32 = 0x3fed;
    /// TOOLS_USE_MANUAL_WRITE_TRACKING, which WARP does not support
    pub const MANUAL_TRACKING: u32 = 0x2000;
}

/// the heap properties and flags CreateHeap (committed false) and CreateCommittedResource
/// (committed true) take (re/d3d12: ValidateHeapPropertiesAndFlags; to be measured, golden
/// d3d12-* props-*)
pub fn props_ok(p: Props, flags: u32, committed: bool) -> bool {
    use hflag::*;
    hit(clause::D3D12_HEAP_PROPS);
    if matches!(p.heap, Heap::Other) || matches!(p.page, CpuPage::Other) || matches!(p.pool, Pool::Other) || flags & !KNOWN != 0 || flags & MANUAL_TRACKING != 0 {
        return false;
    }
    let props = match p.heap {
        // (WARP is UMA: a custom heap's memory is L0)
        Heap::Custom => p.page != CpuPage::Unknown && p.pool == Pool::L0,
        Heap::Default | Heap::Upload | Heap::Readback | Heap::GpuUpload | Heap::Other => p.page == CpuPage::Unknown && p.pool == Pool::Unknown,
    };
    if !props || p.cpu_access() && flags & SHARED != 0 || !p.cpu_access() && flags & WRITE_WATCH != 0 {
        return false;
    }
    let deny = flags & DENY;
    let deny_ok = if committed || flags & CROSS_ADAPTER != 0 { deny == 0 } else { matches!(deny, 0 | 0xc0 | 0x44 | 0x84) };
    let node = |m: u32| if m == 0 { 1 } else { m };
    // (MEASURED, golden d3d12-* props-nodes, heapx-nodes: the creation node is WARP's one node,
    // the visible mask holds it, other bits of the visible mask taken)
    if !deny_ok || node(p.creation) != 1 || node(p.visible) & 1 == 0 {
        return false;
    }
    let cpu_type = matches!(p.heap, Heap::Upload | Heap::Readback | Heap::GpuUpload);
    let gpu_only = p.heap != Heap::Custom || p.page == CpuPage::NotAvailable && p.pool == Pool::L0;
    if flags & CROSS_ADAPTER != 0 {
        flags & SHARED != 0 && !cpu_type && gpu_only && (flags & ALLOW_DISPLAY == 0 || committed)
    } else if flags & ALLOW_DISPLAY != 0 {
        !cpu_type && gpu_only && committed
    } else if flags & ATOMICS != 0 {
        p.heap == Heap::Custom || !p.cpu_access()
    } else {
        true
    }
}

/// CreateHeap with ppvHeap NULL (S_FALSE where it would make the heap)
pub fn heapx_hr(p: Props, size: u64, al: u64, flags: u32) -> Hr {
    hit(clause::D3D12_MAKE_HEAP);
    let size_ok = size != 0 && size < 0xffff_ffff_ffc0_0000 && !(size > 0xffff_0000 && p.cpu_access());
    if props_ok(p, flags, false) && size_ok && matches!(al, 0 | 65536 | 4194304) {
        S_FALSE
    } else {
        E_INVALIDARG
    }
}

/// D3D12_CLEAR_VALUE as committed12 passes it
#[derive(Clone, Copy, Debug)]
pub enum Clear {
    None,
    Color(u32),
    Depth(u32, f32),
}

/// the typeless family a format casts in, (typeless, members)
const FAMILIES: &[(u32, &[u32])] = &[
    (1, &[2, 3, 4]),
    (5, &[6, 7, 8]),
    (9, &[10, 11, 12, 13, 14]),
    (15, &[16, 17, 18]),
    (19, &[20, 21, 22]),
    (23, &[24, 25]),
    (27, &[28, 29, 30, 31, 32]),
    (33, &[34, 35, 36, 37, 38]),
    (39, &[40, 41, 42, 43]),
    (44, &[45, 46, 47]),
    (48, &[49, 50, 51, 52]),
    (53, &[54, 55, 56, 57, 58, 59]),
    (60, &[61, 62, 63, 64]),
    (70, &[71, 72]),
    (73, &[74, 75]),
    (76, &[77, 78]),
    (79, &[80, 81]),
    (82, &[83, 84]),
    (90, &[87, 91]),
    (92, &[88, 93]),
    (94, &[95, 96]),
    (97, &[98, 99]),
];

fn family(f: u32) -> Option<u32> {
    FAMILIES.iter().find(|(t, m)| *t == f || m.contains(&f)).map(|(t, _)| *t)
}

/// the clear value check of a texture's description (re/d3d12: ValidateResourceDescription; to
/// be measured, golden d3d12-* committed-clear-*)
fn clear_ok(c: Clear, d: &Desc) -> bool {
    use super::dxgi_format::{support, DS, RT};
    hit(clause::D3D12_COMMITTED_CLEAR);
    let f = match c {
        Clear::None => return true,
        Clear::Color(f) | Clear::Depth(f, _) => f,
    };
    if d.kind() == Dim::Buffer || d.flags & 3 == 0 || super::dxgi_format::caps(f).is_none() || super::dxgi_format::TYPELESS.contains(&f) {
        return false;
    }
    let same = match family(d.fmt) {
        Some(t) => family(f) == Some(t),
        None => f == d.fmt,
    };
    let depth = match c {
        Clear::Depth(_, z) => (0.0..=1.0).contains(&z),
        Clear::None | Clear::Color(_) => true,
    };
    // (MEASURED, golden d3d12-* committed-clear: a depth texture's clear format a depth format
    // (R32_FLOAT for D32_FLOAT refused); the depth from 0 to 1, denormals and -0 taken, NaN
    // refused)
    let _ = DS;
    same && (d.flags & 1 == 0 || support(f) & RT != 0) && (d.flags & 2 == 0 || super::dxgi_format::DEPTH.contains(&f) && depth)
}

/// the formats an ALLOW_DISPLAY resource takes (MEASURED, golden d3d12-* committed-display,
/// committed-display-fmt-*)
fn display_format(f: u32) -> bool {
    // (refused: the 128 and 96-bit formats, R32G32 and R32G8X24, R24G8, A8_UNORM, R1_UNORM,
    // the packed 4:2:2 RGB ones, block compressed, B5G6R5, B5G5R5A1, the palette formats,
    // B4G4R4A4 and the last, A4B4G4R4)
    !matches!(f, 1..=8 | 15..=22 | 44..=47 | 65 | 68..=86 | 94..=99 | 113..=115 | 191)
}

/// the formats a cross-adapter ROW_MAJOR texture takes (MEASURED, golden d3d12-*
/// committed-display, committed-cross-fmt-*)
fn cross_format(f: u32) -> bool {
    matches!(f, 10 | 24 | 28 | 29 | 87 | 89 | 91)
}

/// CreateCommittedResource with every argument, ppvResource NULL (re/d3d12; to be measured)
pub fn committed_hr(p: Props, flags: u32, state: u32, c: Clear, d: &Desc) -> Res<Hr> {
    use hflag::*;
    if !props_ok(p, flags, true) {
        return Ok(E_INVALIDARG);
    }
    // (MEASURED, golden d3d12-* committed-display: a cross-adapter heap takes the resource flag
    // ALLOW_CROSS_ADAPTER with ROW_MAJOR, for some formats, golden committed-cross-*)
    if flags & CROSS_ADAPTER != 0 || d.flags & 0x10 != 0 {
        hit(clause::D3D12_COMMITTED_FLAGS);
        let ok = flags & CROSS_ADAPTER != 0 && d.flags & 0x10 != 0 && d.kind() == Dim::Tex2D && d.layout == 1 && d.mips == 1 && d.d == 1 && d.samples == 1 && d.flags & !0x15 == 0 && cross_format(d.fmt);
        if !ok {
            return Ok(E_INVALIDARG);
        }
        let plain = Desc { flags: d.flags & !0x10, layout: 0, ..*d };
        let r = create_hr(p.as_type(), state, &plain)?;
        if r != S_FALSE {
            return Ok(r);
        }
        return Ok(if clear_ok(c, d) { S_FALSE } else { E_INVALIDARG });
    }
    let r = create_hr(p.as_type(), state, d)?;
    if r != S_FALSE {
        return Ok(r);
    }
    hit(clause::D3D12_COMMITTED_FLAGS);
    if flags & ATOMICS != 0 && d.flags & 4 == 0 {
        return Ok(E_INVALIDARG);
    }
    if flags & ALLOW_DISPLAY != 0 {
        let display = display_format(d.fmt);
        if !(d.kind() == Dim::Tex2D && d.align == 0 && matches!(d.d, 1 | 2) && d.mips == 1 && d.samples == 1 && d.quality == 0 && d.layout == 0 && d.flags & 0xc2 == 0 && display) {
            return Ok(E_INVALIDARG);
        }
    }
    Ok(if clear_ok(c, d) { S_FALSE } else { E_INVALIDARG })
}

/// CreateHeap through heap12: the executor reads the size as a C long (32 bits on Windows) and
/// widens it signed, so sizes from 2^31 reach CreateHeap as 0xffffffff_xxxxxxxx
pub fn heap_hr(heap: Heap, size: u64, al: u64, flags: u32) -> Hr {
    let wide = size as u32 as i32 as i64 as u64;
    if heapx_hr(Props::plain(heap), wide, al, flags) == S_FALSE {
        S_OK
    } else {
        E_INVALIDARG
    }
}

/// CreatePlacedResource's verdict, in a heap CreateHeap took (MEASURED, golden d3d12-* make-*)
pub fn placed_hr(heap: Heap, hflags: u32, hsize: u64, off: u64, state: u32, d: &Desc) -> Res<Hr> {
    hit(clause::D3D12_MAKE_PLACED);
    let c = create_hr(heap, state, d)?;
    if c != S_FALSE {
        return Ok(c);
    }
    // (what the heap denies: buffers, render target and depth stencil textures, the others)
    let kind = if d.kind() == Dim::Buffer { 4 } else if d.flags & 3 != 0 { 0x40 } else { 0x80 };
    if hflags & kind != 0 {
        return Ok(E_INVALIDARG);
    }
    // (the resource at a multiple of its alignment, inside the heap; a multisampled one wants a
    // heap aligned to 4MB)
    let (size, al) = alloc(d)?;
    Ok(if size == u64::MAX || d.samples > 1 || off % al != 0 || off + size > hsize { E_INVALIDARG } else { S_OK })
}

/// CreateCommittedResource's verdict (ppvResource NULL: S_FALSE when it would make the resource)
/// on a description, a heap type and an initial state
/// the size limit of a single 2D texture of one level
const SINGLE2D_MAX: u32 = 1 << 23;

/// the largest frame latency SetMaximumFrameLatency takes (MEASURED, golden dxgi-*
/// chain-edge*-latency-*: 40 taken, 41 not)
const LATENCY_MAX: u32 = 40;

/// the bytes of buffers a 32-bit process runs out of memory making a swap chain of (golden dxgi-32
/// chain-edge*-size-*: 16384 x 10000 x 2 taken, 16384 x 15000 x 2 not; between, 14000 to 14750,
/// the answer changed from run to run, so no case asks there)
const CHAIN32_OOM: u64 = 16384 * 14250 * 8;

/// D3D12_RESOURCE_STATES a texture may start in (MEASURED, golden d3d12-* create-state-*: each
/// bit alone; the same on x86 and x64)
mod tstate {
    pub const RENDER_TARGET: u32 = 0x4;
    pub const UNORDERED_ACCESS: u32 = 0x8;
    pub const DEPTH_WRITE: u32 = 0x10;
    pub const DEPTH_READ: u32 = 0x20;
    pub const GENERIC_READ: u32 = 0xac3;
    pub const RAYTRACING_AS: u32 = 0x40_0000;
    /// any texture: shader resources, copies, resolves, the video states, SHADING_RATE_SOURCE
    pub const ANY: u32 = 0x40 | 0x80 | 0x400 | 0x800 | 0x1000 | 0x2000 | 0xf_0000 | 0x20_0000 | 0x80_0000 | 0x100_0000;
    /// the states that write, taken alone only (MEASURED, create-state2-*: every pair of them
    /// with another state refused, the read states paired with each other taken)
    pub const WRITES: u32 = RENDER_TARGET | UNORDERED_ACCESS | DEPTH_WRITE | 0x100 | 0x400 | 0x1000 | 0x2_0000 | 0x8_0000 | 0x80_0000;
}

/// whether a texture of these flags may start in this state
fn tex_state_ok(state: u32, flags: u32) -> bool {
    use tstate::*;
    if flags & 0x20 != 0 {
        // (MEASURED, create-state-sim, create-state2-sim: a simultaneous-access texture takes
        // every bit, undefined ones too, any two and all at once, but
        // RAYTRACING_ACCELERATION_STRUCTURE alone)
        return state != RAYTRACING_AS;
    }
    if state == GENERIC_READ {
        // (MEASURED: GENERIC_READ taken although VERTEX_AND_CONSTANT_BUFFER, INDEX_BUFFER and
        // INDIRECT_ARGUMENT alone are not)
        return true;
    }
    let allowed = ANY | if flags & 1 != 0 { RENDER_TARGET } else { 0 } | if flags & 4 != 0 { UNORDERED_ACCESS } else { 0 } | if flags & 2 != 0 { DEPTH_WRITE | DEPTH_READ } else { 0 };
    // (MEASURED, create-state2-*: GENERIC_READ less or plus any bit refused; bits 24..30 but
    // SHADING_RATE_SOURCE refused)
    state & !allowed == 0 && (state & WRITES == 0 || state.count_ones() == 1)
}

pub fn create_hr(heap: Heap, state: u32, d: &Desc) -> Res<Hr> {
    use super::dxgi_format::{support, DS, MS, ONE_MIP, RT, SINGLE};
    hit(clause::D3D12_CREATE);
    let no = |c: bool, id: crate::spec::ClauseId| {
        hit(id);
        c
    };
    // (MEASURED: heap 5, D3D12_HEAP_TYPE_GPU_UPLOAD, is there on WARP)
    if no(!heap.known(), clause::D3D12_HEAP_TYPE) {
        return Ok(E_INVALIDARG);
    }
    if d.kind() != Dim::Buffer && d.layout == 0xffff_ffff {
        // (MEASURED: E_NOTIMPL)
        hit(clause::D3D12_CREATE_LAYOUTNOTIMPL);
        return Ok(E_NOTIMPL);
    }
    let flags = d.flags;
    if flags & 0x400 != 0 {
        // (MEASURED, golden d3d12-* create-tight-*, alloc-tight-*: WARP has no tight alignment
        // (featscan-54: tier 0), yet takes USE_TIGHT_ALIGNMENT and ignores it, against
        // DirectX-Specs' E_INVALIDARG without the cap)
        hit(clause::D3D12_TIGHT_IGNORED);
    }
    // (MEASURED: these flags; 0x10 needs a shared heap, the video ones a video device)
    if no(flags & !(1 | 2 | 4 | 8 | 0x20 | 0x100 | 0x400) != 0 || flags & 3 == 3 || flags & 6 == 6, clause::D3D12_DESC_FLAGS) {
        return Ok(E_INVALIDARG);
    }
    if d.kind() == Dim::Buffer {
        if validate(d).is_err() || no(flags & 0x22 != 0, clause::D3D12_CREATE_BUFFERFLAGS) {
            return Ok(E_INVALIDARG);
        }
        if heap.cpu() {
            hit(clause::D3D12_HEAP_CPU);
            // (MEASURED: no render target or unordered access; upload starts in states GENERIC_READ
            // is made of (or COMMON), readback in COPY_DEST, RESOLVE_DEST (or COMMON))
            let states = if heap == Heap::Upload { 0xac3 } else { 0x1400 };
            return Ok(if flags & 7 != 0 || state & !states != 0 { E_INVALIDARG } else { S_FALSE });
        }
        hit(clause::D3D12_CREATE_BUFFERSTATE);
        return Ok(S_FALSE);
    }
    if no(heap.cpu(), clause::D3D12_HEAP_CPU) {
        return Ok(E_INVALIDARG);
    }
    if heap == Heap::GpuUpload {
        // (MEASURED, golden d3d12-64 create-gpuupload: textures taken as on the default heap)
        hit(clause::D3D12_CREATE_GPUUPLOAD);
    }
    let dim = d.kind().tex_bit();
    if no(dim.is_none(), clause::D3D12_DESC_DIM) || no(d.w == 0 || d.h == 0 || d.d == 0 || d.kind() == Dim::Tex1D && d.h != 1 || d.d > 2048, clause::D3D12_DESC_SIZE) {
        return Ok(E_INVALIDARG);
    }
    let ms = d.samples > 1;
    let target = flags & 3 != 0;
    // (MEASURED, golden d3d12-* create-edge-*, create-edge2-*, create-edge3-*: a single-sampled
    // 2D texture of one slice and MipLevels 1, or 0 (the full chain: the check comes before it is
    // counted), render target or not, has a limit of its own, not of bytes (65536 x 131072 R8
    // taken): 2^23 taken, 2^23 + 1 not (create-edge4-*, create-edge5-*))
    let (wmax, hmax) = match d.kind() {
        Dim::Tex3D => (2048, 2048),
        Dim::Tex2D if !ms && d.mips <= 1 && d.d == 1 => (SINGLE2D_MAX, SINGLE2D_MAX),
        Dim::Tex2D | Dim::Tex1D | Dim::Unknown | Dim::Buffer | Dim::Other => (16384, 16384),
    };
    if no(d.w > wmax || d.h > hmax, clause::D3D12_DESC_SIZE) {
        return Ok(E_INVALIDARG);
    }
    let fs = support(d.fmt);
    let Some(dimbit) = dim else {
        return Ok(E_INVALIDARG);
    };
    if no(fs & dimbit == 0 && !(target && fs & (RT | DS) != 0 && d.kind() == Dim::Tex2D), clause::D3D12_CREATE_FORMAT) {
        return Ok(E_INVALIDARG);
    }
    if no(flags & 1 != 0 && fs & RT == 0 || flags & 2 != 0 && fs & DS == 0 || flags & 8 != 0 && flags & 2 == 0, clause::D3D12_CREATE_TARGETS) {
        return Ok(E_INVALIDARG);
    }
    // (MEASURED, golden d3d12-64 create-split: the video formats take arrays but one mip level, the
    // 4:2:0 ones an even size, the 4:2:2 ones an even width)
    if fs & ONE_MIP != 0 && d.mips != 1 {
        hit(clause::D3D12_CREATE_SINGLE);
        return Ok(E_INVALIDARG);
    }
    if fs & SINGLE != 0 && (d.mips != 1 || d.w % 2 != 0 || matches!(d.fmt, 103..=106) && d.h % 2 != 0) {
        hit(clause::D3D12_CREATE_SINGLE);
        return Ok(E_INVALIDARG);
    }
    // (MEASURED: 4KB only without render target, depth stencil or samples, and small (the
    // documentation's rule), 4MB only with samples)
    let align_ok = match d.align {
        0 | 65536 => true,
        4096 => !target && !ms && small(d),
        4194304 => ms,
        _ => false,
    };
    if no(!align_ok, clause::D3D12_DESC_ALIGN) {
        return Ok(E_INVALIDARG);
    }
    if no(d.samples == 0, clause::D3D12_DESC_MSAA) {
        return Ok(E_INVALIDARG);
    }
    if ms {
        // (MEASURED: 2, 4, 8, 16 samples, quality 0 or the standard pattern, one mip level)
        let ok = d.kind() == Dim::Tex2D && target && d.mips == 1 && matches!(d.samples, 2 | 4 | 8 | 16) && matches!(d.quality, 0 | 0xffff_ffff) && fs & MS != 0 && flags & 0x520 == 0 && d.layout != 3;
        if no(!ok, clause::D3D12_DESC_MSAA) {
            return Ok(E_INVALIDARG);
        }
    } else if d.quality != 0 {
        // (MEASURED: a single sample's quality is not checked)
        hit(clause::D3D12_CREATE_QUALITY);
    }
    match d.layout {
        0 => {}
        2 | 3 if d.kind() == Dim::Tex1D => {
            hit(clause::D3D12_DESC_LAYOUT);
            return Ok(E_INVALIDARG);
        }
        2 if d.kind() == Dim::Tex2D && d.d > 1 => {
            // (MEASURED, golden d3d12-64 create-split: arrays refused, mip chains (3, or 0 for all) taken)
            hit(clause::D3D12_CREATE_LAYOUT64K);
            return Ok(E_INVALIDARG);
        }
        3 if flags & 2 != 0 => {
            hit(clause::D3D12_DESC_LAYOUT);
            return Ok(E_INVALIDARG);
        }
        2 if block(d.fmt).is_none() => {
            // (MEASURED, golden d3d12-64 alloc-other-24..26, create-depth-layout: not the two-plane formats
            // (D24S8, D32S8, NV12); D32 taken)
            hit(clause::D3D12_CREATE_LAYOUT64K);
            return Ok(E_INVALIDARG);
        }
        2 | 3 => hit(clause::D3D12_DESC_LAYOUT),
        _ => {
            hit(clause::D3D12_DESC_LAYOUT);
            return Ok(E_INVALIDARG);
        }
    }
    // (MEASURED: mip levels are not checked)
    if d.mips > full_chain(d) {
        hit(clause::D3D12_CREATE_MIPS);
    }
    if no(!tex_state_ok(state, flags), clause::D3D12_CREATE_STATE) {
        return Ok(E_INVALIDARG);
    }
    Ok(S_FALSE)
}

impl D3d12 {
    fn dxgi(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        if !matches!(name, "dxgifactory" | "dxgiwarp" | "dxgiassoc" | "swapchain" | "scresize" | "scbuffer" | "scpresent" | "sclatency" | "dxgitearing" | "dxgiadapters" | "sccolor" | "scpresent1" | "schdr" | "scwait" | "dxgifeat" | "dxgipref") {
            return Ok(false);
        }
        if name == "dxgifactory" {
            if t.len() != 2 {
                return Err(Fail::Bad);
            }
            self.factory = false;
            self.chain = None;
            let f = num(&t[1]).ok_or(Fail::Bad)? as u32;
            hit(clause::DXGI_FACTORY);
            // (MEASURED: flags other than DXGI_CREATE_FACTORY_DEBUG are ignored; DEBUG is not asked:
            // it depends on the debug layer being installed)
            if f & 1 != 0 {
                // (MEASURED, golden dxgi-* factory-debug: taken on the windows-2025 runner)
                hit(clause::DXGI_FACTORY_DEBUG);
            }
            let _ = f;
            self.factory = true;
            out.push("factory ok".into());
            return Ok(true);
        }
        if !self.factory && matches!(name, "dxgiwarp" | "dxgiassoc" | "swapchain" | "dxgitearing" | "dxgiadapters" | "dxgifeat" | "dxgipref") {
            out.push("nofactory".into());
            return Ok(true);
        }
        match name {
            "dxgiwarp" => {
                hit(clause::DXGI_WARP);
                out.push("warp vendor 1414 device 8c flags 2".into());
            }
            "dxgiassoc" => {
                let null = t.get(2).is_some_and(|s| s == "null");
                if t.len() != 2 + null as usize {
                    return Err(Fail::Bad);
                }
                let f = num(&t[1]).ok_or(Fail::Bad)? as u32;
                hit(clause::DXGI_ASSOC);
                // (MEASURED: a NULL window is taken with any of the three flags)
                let _ = null;
                let ok = f & !7 == 0;
                out.push(format!("assoc hr {:08x}", if ok { S_OK } else { DXGI_ERROR_INVALID_CALL }));
            }
            "dxgifeat" | "dxgipref" => {
                if t.len() != 3 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                if name == "dxgifeat" {
                    // (re/dxgi: only DXGI_FEATURE_PRESENT_ALLOW_TEARING, 4 bytes, always TRUE)
                    hit(clause::DXGI_FEATURE_ARGS);
                    if w[1] > 16 {
                        return Err(Fail::Bad);
                    }
                    if w[0] != 0 || w[1] != 4 {
                        out.push(format!("feat hr {DXGI_ERROR_INVALID_CALL:08x} cccccccc"));
                    } else {
                        out.push("feat hr 00000000 1".into());
                    }
                } else {
                    // (re/dxgi: the preference not range checked; the runner's adapters in
                    // EnumAdapters order, the last WARP)
                    hit(clause::DXGI_ADAPTER_PREF);
                    match w[0] {
                        // (MEASURED, golden dxgi-* pref-args: the same order for every
                        // preference, 0 to 0xffffffff: an adapter of WARP's ids without the
                        // software flag (the render-only one), then WARP)
                        0 => out.push("pref hr 00000000 1414 8c 0".into()),
                        1 => out.push("pref hr 00000000 1414 8c 2".into()),
                        _ => out.push(format!("pref hr {DXGI_ERROR_NOT_FOUND:08x}")),
                    }
                }
            }
            "dxgitearing" | "dxgiadapters" => {
                if t.len() != 1 {
                    return Err(Fail::Bad);
                }
                // (MEASURED, golden dxgi-* tearing, adapters: the windows-2025 runner allows tearing
                // and has two adapters, the last WARP)
                if name == "dxgitearing" {
                    hit(clause::DXGI_TEARING);
                    out.push("tearing 1".into());
                } else {
                    hit(clause::DXGI_ADAPTERS);
                    out.push("adapters 2 last 1414 8c 2".into());
                }
            }
            "swapchain" => {
                if !self.dev {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if t.len() != 13 {
                    return Err(Fail::Bad);
                }
                let Ok(w) = <[u32; 12]>::try_from(words(&t[1..])?) else {
                    return Err(Fail::Bad);
                };
                self.chain = None;
                match chain_desc(&w) {
                    Ok(d) => {
                        self.chain = Some((d, 0));
                        self.latency = 1;
                        self.fresh = true;
                        out.push(chain_line("swapchain", &d, 0));
                    }
                    Err(hr) => out.push(format!("swapchain hr {hr:08x}")),
                }
            }
            _ if self.chain.is_none() => out.push("nochain".into()),
            "scwait" => {
                if t.len() != 1 {
                    return Err(Fail::Bad);
                }
                // (re/dxgi: a handle only for a FRAME_LATENCY_WAITABLE_OBJECT chain)
                hit(clause::DXGI_LATENCY);
                let waitable = self.chain.is_some_and(|c| c.0[11] & 0x40 != 0);
                out.push(format!("wait {}", if waitable { "handle" } else { "null" }));
            }
            "schdr" => {
                if t.len() != 4 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let (kind, size, null) = (w[0], w[1], w[2] != 0);
                if size > 128 {
                    return Err(Fail::Bad);
                }
                // (re/dxgi: NONE with no data, HDR10 28 bytes, HDR10+ 72 bytes; E_INVALIDARG else)
                hit(clause::DXGI_HDR);
                let ok = match kind {
                    0 => size == 0 && null,
                    1 => size == 28 && !null,
                    2 => size == 72 && !null,
                    _ => false,
                };
                out.push(format!("hdr hr {:08x}", if ok { S_OK } else { E_INVALIDARG }));
            }
            _ => {
                let Some((d, index)) = self.chain.as_mut() else {
                    out.push("nochain".into());
                    return Ok(true);
                };
                let w = words(&t[1..])?;
                match (name, w.len()) {
                    ("sccolor", 1) => {
                        let cs = w[0];
                        // (MEASURED, golden dxgi-* color-*, color-f*-*: the color spaces a flip chain
                        // presents in, by format; SetColorSpace1 takes exactly those)
                        let set: &[u32] = match d[2] {
                            10 => &[1],
                            24 | 28 | 87 => &[0, 2, 3, 12, 14, 17, 20, 21],
                            f => unreachable!("chain_desc takes only the flip formats, not {f}"),
                        };
                        hit(clause::DXGI_COLORSPACE_FLIP);
                        let ok = set.contains(&cs);
                        out.push(format!("color {} set {:08x}", ok as u32, if ok { S_OK } else { E_INVALIDARG }));
                    }
                    ("scbuffer", 1) => {
                        hit(clause::DXGI_BUFFER);
                        // (a buffer: a 2D render target of the chain's size and format)
                        if w[0] < d[7] {
                            out.push(format!("buffer 3 {:x} {:x} 1 1 {:x} 1 0 1", d[0], d[1], d[2]));
                        } else {
                            out.push(format!("buffer hr {DXGI_ERROR_INVALID_CALL:08x}"));
                        }
                    }
                    ("scpresent", 2) => {
                        hit(clause::DXGI_PRESENT);
                        let (sync, flags) = (w[0], w[1]);
                        // (MEASURED: sync intervals 0..4; TEST and DO_NOT_SEQUENCE present nothing;
                        // ALLOW_TEARING needs the chain's flag; unknown flags refused)
                        let known = 1 | 2 | 4 | 0x100 | 0x200;
                        // (TEST with any other flag refused: golden dxgi-* chain-edge-present-0-3,
                        // chain-edge-tear-0-201)
                        let bad = sync > 4 || flags & !known != 0 || flags & 0x200 != 0 && (d[11] & 0x800 == 0 || sync != 0) || flags & 1 != 0 && flags != 1;
                        // (re/dxgi: DO_NOT_SEQUENCE on a flip chain that has not presented)
                        let bad = bad || flags & 2 != 0 && self.fresh;
                        if flags & 4 != 0 {
                            // (MEASURED, golden dxgi-64 chain-present-restart: presents as without it)
                            hit(clause::DXGI_PRESENT_RESTART);
                        }
                        if bad {
                            out.push(format!("present hr {DXGI_ERROR_INVALID_CALL:08x} index {index:x}"));
                        } else {
                            if flags & 3 == 0 {
                                *index = (*index + 1) % d[7];
                                self.fresh = false;
                            } else {
                                hit(clause::DXGI_PRESENT_TEST);
                            }
                            out.push(format!("present hr 00000000 index {index:x}"));
                        }
                    }
                    ("scresize", 5) => {
                        hit(clause::DXGI_RESIZE);
                        let (n, rw, rh, f, fl) = (w[0], w[1], w[2], w[3], w[4]);
                        // (MEASURED: 0 keeps the count and format; a 0 size is the window's; the
                        // flags not to change are FRAME_LATENCY_WAITABLE_OBJECT and ALLOW_TEARING;
                        // unknown flags DXGI_ERROR_INVALID_CALL, the rest E_INVALIDARG)
                        if fl & !CHAIN_FLAGS != 0 {
                            out.push(format!("resize hr {DXGI_ERROR_INVALID_CALL:08x}"));
                        } else if n == 1 || n > 16 || f != 0 && !FLIP.contains(&f) || (fl ^ d[11]) & 0x840 != 0 {
                            out.push(format!("resize hr {E_INVALIDARG:08x}"));
                        } else {
                            if n != 0 {
                                d[7] = n;
                            }
                            if f != 0 {
                                d[2] = f;
                            }
                            (d[0], d[1]) = if rw == 0 || rh == 0 { (64, 64) } else { (rw, rh) };
                            d[11] = fl;
                            *index = 0;
                            self.fresh = true;
                            out.push(chain_line("resize", d, 0));
                        }
                    }
                    ("scpresent1", n) if n >= 10 => {
                        hit(clause::DXGI_PRESENT_DIRTY);
                        let (sync, flags, scroll) = (w[0], w[1], w[2] != 0);
                        let count = w[9] as usize;
                        if count > 8 || n != 10 + 4 * count {
                            return Err(Fail::Bad);
                        }
                        let rect = |r: &[u32]| r.iter().map(|v| *v as i32).collect::<Vec<i32>>();
                        let (bw, bh) = (d[0] as i32, d[1] as i32);
                        let empty = |r: &[i32]| r[2] <= r[0] || r[3] <= r[1];
                        let inside = |r: &[i32]| r[0] >= 0 && r[1] >= 0 && r[2] <= bw && r[3] <= bh;
                        let dirty: Vec<Vec<i32>> = (0..count).map(|i| rect(&w[10 + 4 * i..14 + 4 * i])).collect();
                        let sc = rect(&w[3..7]);
                        let (dx, dy) = (w[7] as i32, w[8] as i32);
                        let known = 1 | 2 | 4 | 0x100 | 0x200;
                        let mut bad = sync > 4 || flags & !known != 0 || flags & 0x200 != 0 && (d[11] & 0x800 == 0 || sync != 0) || flags & 1 != 0 && flags != 1 || flags & 2 != 0 && self.fresh;
                        if count > 0 || scroll {
                            let full = count == 1 && dirty[0] == [0, 0, bw, bh] && !scroll;
                            let src = [sc[0] - dx, sc[1] - dy, sc[2] - dx, sc[3] - dy];
                            bad = bad || self.fresh && !full || flags & 3 != 0 || count > 0 && d[9] == 4 || scroll && d[9] != 3;
                            bad = bad || scroll && (empty(&sc) || !inside(&sc) || !inside(&src)) || dirty.iter().any(|r| empty(r) || !inside(r));
                        }
                        if bad {
                            out.push(format!("present1 hr {DXGI_ERROR_INVALID_CALL:08x} index {index:x}"));
                        } else {
                            if flags & 3 == 0 {
                                *index = (*index + 1) % d[7];
                                self.fresh = false;
                            }
                            out.push(format!("present1 hr 00000000 index {index:x}"));
                        }
                    }
                    ("sclatency", 1) => {
                        hit(clause::DXGI_LATENCY);
                        // (MEASURED: only with FRAME_LATENCY_WAITABLE_OBJECT; 0 and 0xffffffff
                        // refused, 17 taken; the default 1)
                        if d[11] & 0x40 == 0 {
                            out.push(format!("latency hr {0:08x} get {0:08x} cccccccc", DXGI_ERROR_INVALID_CALL));
                        } else {
                            let hr = if w[0] != 0 && w[0] <= LATENCY_MAX {
                                self.latency = w[0];
                                S_OK
                            } else {
                                DXGI_ERROR_INVALID_CALL
                            };
                            out.push(format!("latency hr {hr:08x} get 00000000 {:x}", self.latency));
                        }
                    }
                    _ => return Err(Fail::Bad),
                }
            }
        }
        Ok(true)
    }
}

/// GetDesc1 of the swap chain CreateSwapChainForHwnd makes from `w` (Err(hr) when it refuses it)
pub fn chain_desc(w: &[u32; 12]) -> Result<[u32; 12], Hr> {
    let mut d = *w;
    let no = |c: bool, id: crate::spec::ClauseId| {
        hit(id);
        if c {
            Err(DXGI_ERROR_INVALID_CALL)
        } else {
            Ok(())
        }
    };
    no(!matches!(d[9], 3 | 4), clause::DXGI_CHAIN_EFFECT)?;
    no(d[7] < 2 || d[7] > 16, clause::DXGI_CHAIN_BUFFERS)?;
    no(d[4] != 1 || d[5] != 0, clause::DXGI_CHAIN_SAMPLES)?;
    no(!FLIP.contains(&d[2]), clause::DXGI_CHAIN_FORMAT)?;
    // (MEASURED, golden dxgi-32 swapchain-*: scaling STRETCH or NONE (ASPECT_RATIO_STRETCH refused
    // for a window), alpha UNSPECIFIED or IGNORE, usage bits up to 0x40, these flags; stereo is
    // taken and kept)
    no(d[8] > 1, clause::DXGI_CHAIN_SCALING)?;
    no(!matches!(d[10], 0 | 3), clause::DXGI_CHAIN_ALPHA)?;
    no(d[6] & !0x70 != 0, clause::DXGI_CHAIN_USAGE)?;
    no(d[11] & !CHAIN_FLAGS != 0, clause::DXGI_CHAIN_FLAGS)?;
    if d[3] != 0 {
        hit(clause::DXGI_CHAIN_STEREO);
    }
    if d[0] == 0 || d[1] == 0 {
        // (MEASURED: either one 0 takes both from the window)
        hit(clause::DXGI_CHAIN_SIZE);
        (d[0], d[1]) = (64, 64);
    }
    // (MEASURED: x86 runs out of address space for 16385 by 16385)
    // (MEASURED, golden dxgi-32 chain-edge-size-*, chain-edge2-size-*)
    if super::arch() == 32 && d[0] as u64 * d[1] as u64 * 4 * d[7] as u64 >= CHAIN32_OOM {
        hit(clause::DXGI_CHAIN_MEMORY);
        return Err(E_OUTOFMEMORY);
    }
    Ok(d)
}

/// the swap chain flags CreateSwapChainForHwnd takes on Direct3D 12 (MEASURED): ALLOW_MODE_SWITCH,
/// RESTRICT_SHARED_RESOURCE_DRIVER, FRAME_LATENCY_WAITABLE_OBJECT, ALLOW_TEARING, HW_PROTECTED, 0x2000
pub const CHAIN_FLAGS: u32 = 0x2 | 0x20 | 0x40 | 0x800 | 0x1000 | 0x2000;

fn chain_line(what: &str, d: &[u32; 12], index: u32) -> String {
    format!("{what} = {} index {index:x}", d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" "))
}
