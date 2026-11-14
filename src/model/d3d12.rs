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

use super::dxgi_format::{block, channels, dims, planar_depth, Chan, Enc, FLIP};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::gen::d3d12::Desc;
use crate::spec::hit;

pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const S_FALSE: u32 = 1;
pub const DXGI_ERROR_INVALID_CALL: u32 = 0x887a_0001;

fn align(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// the mip levels a full chain has
pub fn full_chain(d: &Desc) -> u32 {
    let m = match d.dim {
        4 => d.w.max(d.h).max(d.d),
        3 => d.w.max(d.h),
        _ => d.w,
    };
    32 - m.max(1).leading_zeros()
}

/// why a description is refused (the clause), or Ok
pub fn validate(d: &Desc) -> Result<(), &'static str> {
    check(d, true)
}

/// the checks of a description; `strict`: also the size and mip level limits (GetCopyableFootprints
/// does not check them: MEASURED, golden d3d12-32 fp-invalid)
pub fn check(d: &Desc, strict: bool) -> Result<(), &'static str> {
    let no = |c: bool, id: &'static str| {
        hit(id);
        if c {
            Err(id)
        } else {
            Ok(())
        }
    };
    no(!(1..=4).contains(&d.dim), "D3D12.DESC.DIM")?;
    if d.dim == 1 {
        no(d.align != 0 && d.align != 65536, "D3D12.DESC.BUFFER")?;
        no(d.h != 1 || d.d != 1 || d.mips != 1 || d.fmt != 0 || d.samples != 1 || d.quality != 0 || d.layout != 1 || d.w == 0, "D3D12.DESC.BUFFER")?;
        return Ok(());
    }
    no(d.w == 0 || d.h == 0 || d.d == 0, "D3D12.DESC.SIZE")?;
    let (wmax, hmax, dmax) = match d.dim {
        2 => (16384, 1, 2048),
        3 => (16384, 16384, 2048),
        _ => (2048, 2048, 2048),
    };
    no(d.d > dmax || d.h > hmax && d.dim == 2, "D3D12.DESC.SIZE")?;
    // (MEASURED: GetCopyableFootprints checks the 3D limits, not those of 1D and 2D textures)
    if strict || d.dim == 4 {
        no(d.w > wmax || d.h > hmax, "D3D12.DESC.SIZE")?;
        no(d.mips > full_chain(d), "D3D12.DESC.MIPS")?;
    } else if d.w > wmax || d.h > hmax || d.mips > full_chain(d) {
        hit("D3D12.FOOTPRINT.LIMITS");
    }
    no(!matches!(d.align, 0 | 4096 | 65536 | 4194304), "D3D12.DESC.ALIGN")?;
    no(d.samples == 0, "D3D12.DESC.MSAA")?;
    if d.samples > 1 || d.quality > 0 {
        no(d.dim != 3 || d.flags & 3 == 0 || d.mips != 1 || d.flags & 0x24 != 0 || d.layout == 3, "D3D12.DESC.MSAA")?;
    }
    no(d.layout > 3, "D3D12.DESC.LAYOUT")?;
    Ok(())
}

/// the layout of one subresource: (offset from the first, format, width, height, depth, row
/// pitch, rows, row size)
type Fp = (u64, u32, u32, u32, u32, u64, u32, u64);

/// what GetCopyableFootprints writes for an invalid description: every field all ones
fn invalid_fps(count: u32) -> (Vec<Fp>, u64) {
    hit("D3D12.FOOTPRINT.INVALID");
    ((0..count).map(|_| (u64::MAX, !0, !0, !0, !0, u64::MAX, !0, u64::MAX)).collect(), u64::MAX)
}

/// GetCopyableFootprints' layouts and total, or why the model has none
pub fn footprints(first: u32, count: u32, base: u64, d: &Desc) -> Res<(Vec<Fp>, u64)> {
    if check(d, false).is_err() {
        return Ok(invalid_fps(count));
    }
    if d.dim == 1 {
        hit("D3D12.FOOTPRINT.BUFFER");
        // (MEASURED: a row pitch past 32 bits is invalid)
        if align(d.w as u64, 256) > u32::MAX as u64 {
            return Ok(invalid_fps(count));
        }
        if first as u64 + count as u64 > 1 {
            hit("D3D12.FOOTPRINT.RANGE");
            return Ok(invalid_fps(count));
        }
        let w = d.w as u64;
        let v = (0..count).map(|_| (base, 0, d.w, 1, 1, align(w, 256), 1, w)).collect();
        return Ok((v, if count == 0 { 0 } else { w }));
    }
    let dm = dims(d.fmt).unwrap();
    hit("D3D12.FORMAT.DIMS");
    if dm >> (d.dim - 2) & 1 == 0 {
        return Ok(invalid_fps(count));
    }
    // (MEASURED: the two-texel formats want an even width, the packed video ones no mip levels)
    if matches!(d.fmt, 107..=109) && d.mips != 1 {
        hit("D3D12.FOOTPRINT.PAIRS");
        return Ok(invalid_fps(count));
    }
    if matches!(d.fmt, 68 | 69 | 107..=109) && d.w % 2 != 0 {
        hit("D3D12.FOOTPRINT.PAIRS");
        return Ok(invalid_fps(count));
    }
    // (MEASURED, golden d3d12-64 create-split: NV12, P010, P016 in two planes, luma R8 or R16 and
    // chroma R8G8 or R16G16 at half size, rows 512 apart, one mip level and an even size)
    if matches!(d.fmt, 103..=105) {
        hit("D3D12.FOOTPRINT.VIDEO");
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
        hit("D3D12.FOOTPRINT.PLANAR");
        (39, (4, 1, 1), 512)
    } else {
        (d.fmt, block(d.fmt).unwrap(), 256)
    };
    let mips = if d.mips == 0 {
        hit("D3D12.FOOTPRINT.MIPS0");
        full_chain(d)
    } else {
        d.mips
    };
    let array = if d.dim == 4 { 1 } else { d.d };
    if first as u64 + count as u64 > (mips * array) as u64 {
        hit("D3D12.FOOTPRINT.RANGE");
        return Ok(invalid_fps(count));
    }
    hit("D3D12.FOOTPRINT.LAYOUT");
    let mut v = Vec::new();
    let mut at = 0u64;
    let mut total = 0;
    for i in first..first + count {
        let m = i % mips;
        let w = (d.w >> m).max(1);
        let h = if d.dim == 2 { 1 } else { (d.h >> m).max(1) };
        let depth = if d.dim == 4 { (d.d >> m).max(1) } else { 1 };
        if bw > 1 || bh > 1 {
            hit("D3D12.FOOTPRINT.BLOCK");
        }
        let (fw, fh) = (align(w as u64, bw as u64) as u32, align(h as u64, bh as u64) as u32);
        let rows = fh / bh;
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
            hit("D3D12.CLEAR.FLOAT32");
            v as u64
        }
        Enc::Float if ch.bits == 16 => {
            hit("D3D12.CLEAR.FLOAT16");
            to_small_float(v, 5, 10, true)
        }
        Enc::Float => {
            hit("D3D12.CLEAR.FLOAT11");
            to_small_float(v, 5, ch.bits - 5, false)
        }
        Enc::Unorm => {
            hit("D3D12.CLEAR.UNORM");
            to_unorm(c, ch.bits)
        }
        Enc::Snorm => {
            hit("D3D12.CLEAR.SNORM");
            to_snorm(c, ch.bits)
        }
        Enc::Srgb => {
            hit("D3D12.CLEAR.SRGB");
            to_srgb(c, ch.bits)
        }
        Enc::Uint => {
            hit("D3D12.CLEAR.INT");
            to_int(c, ch.bits, false)
        }
        Enc::Sint => {
            hit("D3D12.CLEAR.INT");
            to_int(c, ch.bits, true)
        }
        Enc::Unused => {
            hit("D3D12.CLEAR.X");
            0
        }
        Enc::Alpha => {
            hit("D3D12.CLEAR.XALPHA");
            to_unorm(c, ch.bits)
        }
    }
}

/// the input a clear takes: denormals as 0 (of their sign), NaNs kept
pub fn clear_input(v: u32) -> u32 {
    if v & 0x7f80_0000 == 0 && v & 0x7f_ffff != 0 {
        hit("D3D12.CLEAR.DENORM");
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
    hit("D3D12.CLEARDSV");
    let raw = f32of(clear_input(depth));
    let d = if raw.is_nan() {
        hit("D3D12.CLEARDSV.NAN");
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
    factory: bool,
    /// the swap chain: its description (GetDesc1's 12 fields) and back buffer index
    chain: Option<([u32; 12], u32)>,
    /// its maximum frame latency
    latency: u32,
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
                hit("D3D12.DEVICE");
                self.dev = true;
                self.chain = None;
                out.push("device12 ok".into());
                return Ok(true);
            }
            "footprint12" | "alloc12" | "create12" | "clear12" | "cleardsv12" if !self.dev => {
                out.push("nodev".into());
                return Ok(true);
            }
            "footprint12" => {
                if t.len() != 15 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                if w[1] > 16 {
                    return Err(Fail::Bad);
                }
                let (fps, total) = footprints(w[0], w[1], w[2] as u64, &Desc::from_words(&w[3..]))?;
                for f in fps {
                    out.push(format!("fp {} {:x} {:x} {:x} {:x} {:x} {:x} {}", hex64(f.0), f.1, f.2, f.3, f.4, f.5 as u32, f.6, hex64(f.7)));
                }
                out.push(format!("total {}", hex64(total)));
            }
            "alloc12" => {
                if t.len() != 12 {
                    return Err(Fail::Bad);
                }
                let d = Desc::from_words(&words(&t[1..])?);
                let (size, al) = alloc(&d)?;
                out.push(format!("alloc {size:x} {al:x}"));
            }
            "create12" => {
                if t.len() != 14 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let (heap, state, d) = (w[0], w[1], Desc::from_words(&w[2..]));
                out.push(format!("create hr {:08x}", create_hr(heap, state, &d)?));
            }
            "clear12" => {
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                match clear_texel(w[0], [w[1], w[2], w[3], w[4]]) {
                    Some(b) => out.push(format!("clear12 {}", hex(&b))),
                    None if w[0] == 40 => {
                        // (MEASURED: a D32_FLOAT render target removes the device)
                        hit("D3D12.CLEAR.REMOVED");
                        out.push("clear12 hr 887a0005".into());
                    }
                    None => {
                        hit("D3D12.CLEAR.FORMAT");
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
                        hit("D3D12.CLEAR.FORMAT");
                        out.push(format!("cleardsv12 hr {E_INVALIDARG:08x}"));
                    }
                }
            }
            _ => return self.dxgi(op, out),
        }
        Ok(true)
    }
}

/// D3D12_RESOURCE_DESC (Alignment): 4KB is allowed when the most detailed mip level is at most
/// 16 tiles of 4KB, in the standard tile shapes of the format's element size
pub fn small(d: &Desc) -> bool {
    hit("D3D12.DESC.SMALL");
    let Some((bytes, bw, bh)) = block(d.fmt) else { return false };
    let (w, h) = (d.w.div_ceil(bw) as u64, d.h.div_ceil(bh) as u64);
    let i = (bytes as u64).trailing_zeros() as usize;
    if bytes.count_ones() != 1 || i > 4 {
        return false;
    }
    let tiles = match d.dim {
        2 => w.div_ceil(4096 / bytes as u64),
        3 => {
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
    if d.dim == 1 {
        if validate(d).is_err() {
            hit("D3D12.ALLOC.INVALID");
            return Ok((u64::MAX, 65536));
        }
        hit("D3D12.ALLOC.BUFFER");
        return Ok((align(d.w as u64, 65536), 65536));
    }
    if create_hr(1, 0, d)? != S_FALSE {
        hit("D3D12.ALLOC.INVALID");
        return Ok((u64::MAX, 65536));
    }
    // (MEASURED, golden d3d12-64 alloc-other: 64KB-swizzled textures in whole 64KB tiles of the
    // standard shapes, one mip level asked)
    if d.layout == 2 {
        let Some((bytes, bw, bh)) = block(d.fmt).filter(|_| d.mips == 1) else { return Err(Fail::Unmodeled("D3D12.ALLOC.OTHER")) };
        hit("D3D12.ALLOC.SWIZZLE");
        let (tw, th) = match bytes {
            1 => (256, 256),
            2 => (256, 128),
            4 => (128, 128),
            8 => (128, 64),
            _ => (64, 64),
        };
        let (w, h) = (d.w.div_ceil(bw) as u64, d.h.div_ceil(bh) as u64);
        return Ok((w.div_ceil(tw) * h.div_ceil(th) * 65536, 65536));
    }
    if d.layout != 0 || d.samples > 1 && d.align != 0 && d.align != 4194304 {
        return Err(Fail::Unmodeled("D3D12.ALLOC.OTHER"));
    }
    // (MEASURED, golden d3d12-64 alloc-other: NV12 in its two planes, 1.5 bytes a texel; the
    // two-plane depth stencil formats, D32_FLOAT_S8X24 as 8 bytes, D24_UNORM_S8 as 4)
    let (bytes, bw, bh) = match d.fmt {
        19..=22 => (8, 1, 1),
        44..=47 => (4, 1, 1),
        103..=105 => {
            hit("D3D12.ALLOC.VIDEO");
            let b = if d.fmt == 103 { 1 } else { 2 };
            let (w, h) = (d.w as u64, d.h as u64);
            let total = (w * h * b + (w / 2) * (h / 2) * 2 * b) * d.d as u64;
            return Ok((align(total, 65536), 65536));
        }
        _ => match block(d.fmt) {
            Some(b) => b,
            None => return Err(Fail::Unmodeled("D3D12.ALLOC.OTHER")),
        },
    };
    // (MEASURED, golden d3d12-32 alloc-texture-*, 165 descriptions of 9 formats: WARP packs every
    // subresource tightly, rows of whole elements, one after the other)
    hit("D3D12.ALLOC.TEXTURE");
    let mips = if d.mips == 0 { full_chain(d) } else { d.mips };
    let array = if d.dim == 4 { 1 } else { d.d };
    let mut total = 0u64;
    for _ in 0..array {
        for m in 0..mips {
            let w = (d.w >> m).max(1).div_ceil(bw) as u64;
            let h = if d.dim == 2 { 1 } else { (d.h >> m).max(1).div_ceil(bh) as u64 };
            let depth = if d.dim == 4 { (d.d >> m).max(1) as u64 } else { 1 };
            total += w * bytes as u64 * h * depth;
        }
    }
    if d.samples > 1 {
        // (MEASURED, golden d3d12-64 alloc-other, 6 descriptions: every sample stored, in 4MB)
        hit("D3D12.ALLOC.MSAA");
        return Ok((align(total * d.samples as u64, 4194304), 4194304));
    }
    if d.dim == 2 && d.flags & 1 != 0 {
        // (FITTED, golden d3d12-64 alloc-rt1d, 10 descriptions of 4 formats: a 1D render target
        // takes twice the tight size)
        hit("D3D12.ALLOC.RT1D");
        total *= 2;
    }
    let al = if d.align == 4096 { 4096 } else { 65536 };
    Ok((align(total, al), al))
}

/// CreateCommittedResource's verdict (ppvResource NULL: S_FALSE when it would make the resource)
/// on a description, a heap type and an initial state
pub fn create_hr(heap: u32, state: u32, d: &Desc) -> Res<u32> {
    use super::dxgi_format::{support, DS, MS, RT, SINGLE};
    hit("D3D12.CREATE");
    let no = |c: bool, id: &'static str| {
        hit(id);
        c
    };
    // (MEASURED: heap 5, D3D12_HEAP_TYPE_GPU_UPLOAD, is there on WARP)
    if no(!matches!(heap, 1..=3 | 5), "D3D12.HEAP.TYPE") {
        return Ok(E_INVALIDARG);
    }
    if d.dim != 1 && d.layout == 0xffff_ffff {
        // (MEASURED: E_NOTIMPL)
        hit("D3D12.CREATE.LAYOUTNOTIMPL");
        return Ok(0x8000_4001);
    }
    let flags = d.flags;
    // (MEASURED: these flags; 0x10 needs a shared heap, the video ones a video device)
    if no(flags & !(1 | 2 | 4 | 8 | 0x20 | 0x100 | 0x400) != 0 || flags & 3 == 3 || flags & 6 == 6, "D3D12.DESC.FLAGS") {
        return Ok(E_INVALIDARG);
    }
    if d.dim == 1 {
        if check(d, true).is_err() || no(flags & 0x22 != 0, "D3D12.CREATE.BUFFERFLAGS") {
            return Ok(E_INVALIDARG);
        }
        if heap == 2 || heap == 3 {
            hit("D3D12.HEAP.CPU");
            // (MEASURED: no render target or unordered access; upload starts in states GENERIC_READ
            // is made of (or COMMON), readback in COPY_DEST, RESOLVE_DEST (or COMMON))
            let states = if heap == 2 { 0xac3 } else { 0x1400 };
            return Ok(if flags & 7 != 0 || state & !states != 0 { E_INVALIDARG } else { S_FALSE });
        }
        hit("D3D12.CREATE.BUFFERSTATE");
        return Ok(S_FALSE);
    }
    if no(heap == 2 || heap == 3, "D3D12.HEAP.CPU") {
        return Ok(E_INVALIDARG);
    }
    if heap == 5 {
        // (MEASURED, golden d3d12-64 create-gpuupload: textures taken as on the default heap)
        hit("D3D12.CREATE.GPUUPLOAD");
    }
    if no(!(2..=4).contains(&d.dim), "D3D12.DESC.DIM") || no(d.w == 0 || d.h == 0 || d.d == 0 || d.dim == 2 && d.h != 1 || d.d > 2048, "D3D12.DESC.SIZE") {
        return Ok(E_INVALIDARG);
    }
    let ms = d.samples > 1;
    let target = flags & 3 != 0;
    // (MEASURED: a single-sampled 2D render target or depth stencil has no size limit checked)
    let (wmax, hmax) = match d.dim {
        4 => (2048, 2048),
        3 if target && !ms => (u32::MAX, u32::MAX),
        _ => (16384, 16384),
    };
    if no(d.w > wmax || d.h > hmax, "D3D12.DESC.SIZE") {
        return Ok(E_INVALIDARG);
    }
    let fs = support(d.fmt);
    let dimbit = 1 << (d.dim - 2);
    if no(fs & dimbit == 0 && !(target && fs & (RT | DS) != 0 && d.dim == 3), "D3D12.CREATE.FORMAT") {
        return Ok(E_INVALIDARG);
    }
    if no(flags & 1 != 0 && fs & RT == 0 || flags & 2 != 0 && fs & DS == 0 || flags & 8 != 0 && flags & 2 == 0, "D3D12.CREATE.TARGETS") {
        return Ok(E_INVALIDARG);
    }
    // (MEASURED, golden d3d12-64 create-split: the video formats take arrays but one mip level, the
    // 4:2:0 ones an even size, the 4:2:2 ones an even width)
    if fs & SINGLE != 0 && (d.mips != 1 || d.w % 2 != 0 || matches!(d.fmt, 103..=106) && d.h % 2 != 0) {
        hit("D3D12.CREATE.SINGLE");
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
    if no(!align_ok, "D3D12.DESC.ALIGN") {
        return Ok(E_INVALIDARG);
    }
    if no(d.samples == 0, "D3D12.DESC.MSAA") {
        return Ok(E_INVALIDARG);
    }
    if ms {
        // (MEASURED: 2, 4, 8, 16 samples, quality 0 or the standard pattern, one mip level)
        let ok = d.dim == 3 && target && d.mips == 1 && matches!(d.samples, 2 | 4 | 8 | 16) && matches!(d.quality, 0 | 0xffff_ffff) && fs & MS != 0 && flags & 0x520 == 0 && d.layout != 3;
        if no(!ok, "D3D12.DESC.MSAA") {
            return Ok(E_INVALIDARG);
        }
    } else if d.quality != 0 {
        // (MEASURED: a single sample's quality is not checked)
        hit("D3D12.CREATE.QUALITY");
    }
    match d.layout {
        0 => {}
        2 | 3 if d.dim == 2 => {
            hit("D3D12.DESC.LAYOUT");
            return Ok(E_INVALIDARG);
        }
        2 if d.dim == 3 && d.d > 1 => {
            // (MEASURED, golden d3d12-64 create-split: arrays refused, mip chains (3, or 0 for all) taken)
            hit("D3D12.CREATE.LAYOUT64K");
            return Ok(E_INVALIDARG);
        }
        3 if flags & 2 != 0 => {
            hit("D3D12.DESC.LAYOUT");
            return Ok(E_INVALIDARG);
        }
        2 | 3 => hit("D3D12.DESC.LAYOUT"),
        _ => {
            hit("D3D12.DESC.LAYOUT");
            return Ok(E_INVALIDARG);
        }
    }
    // (MEASURED: mip levels are not checked)
    if d.mips > full_chain(d) {
        hit("D3D12.CREATE.MIPS");
    }
    // (MEASURED: RENDER_TARGET needs the render target flag, DEPTH_WRITE the depth stencil one)
    if no(state & 4 != 0 && flags & 1 == 0 || state & 0x10 != 0 && flags & 2 == 0, "D3D12.CREATE.STATE") {
        return Ok(E_INVALIDARG);
    }
    Ok(S_FALSE)
}

impl D3d12 {
    fn dxgi(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        if !matches!(name, "dxgifactory" | "dxgiwarp" | "dxgiassoc" | "swapchain" | "scresize" | "scbuffer" | "scpresent" | "sclatency") {
            return Ok(false);
        }
        if name == "dxgifactory" {
            if t.len() != 2 {
                return Err(Fail::Bad);
            }
            self.factory = false;
            self.chain = None;
            let f = num(&t[1]).ok_or(Fail::Bad)? as u32;
            hit("DXGI.FACTORY");
            // (MEASURED: flags other than DXGI_CREATE_FACTORY_DEBUG are ignored; DEBUG is not asked:
            // it depends on the debug layer being installed)
            if f & 1 != 0 {
                return Err(Fail::Unmodeled("DXGI.FACTORY.DEBUG"));
            }
            let _ = f;
            self.factory = true;
            out.push("factory ok".into());
            return Ok(true);
        }
        if !self.factory && matches!(name, "dxgiwarp" | "dxgiassoc" | "swapchain") {
            out.push("nofactory".into());
            return Ok(true);
        }
        match name {
            "dxgiwarp" => {
                hit("DXGI.WARP");
                out.push("warp vendor 1414 device 8c flags 2".into());
            }
            "dxgiassoc" => {
                let null = t.get(2).is_some_and(|s| s == "null");
                if t.len() != 2 + null as usize {
                    return Err(Fail::Bad);
                }
                let f = num(&t[1]).ok_or(Fail::Bad)? as u32;
                hit("DXGI.ASSOC");
                // (MEASURED: a NULL window is taken with any of the three flags)
                let _ = null;
                let ok = f & !7 == 0;
                out.push(format!("assoc hr {:08x}", if ok { 0 } else { DXGI_ERROR_INVALID_CALL }));
            }
            "swapchain" => {
                if !self.dev {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if t.len() != 13 {
                    return Err(Fail::Bad);
                }
                let w: [u32; 12] = words(&t[1..])?.try_into().unwrap();
                self.chain = None;
                match chain_desc(&w) {
                    Ok(d) => {
                        self.chain = Some((d, 0));
                        self.latency = 1;
                        out.push(chain_line("swapchain", &d, 0));
                    }
                    Err(hr) => out.push(format!("swapchain hr {hr:08x}")),
                }
            }
            _ if self.chain.is_none() => out.push("nochain".into()),
            _ => {
                let Some((d, index)) = self.chain.as_mut() else { unreachable!() };
                let w = words(&t[1..])?;
                match (name, w.len()) {
                    ("scbuffer", 1) => {
                        hit("DXGI.BUFFER");
                        // (a buffer: a 2D render target of the chain's size and format)
                        if w[0] < d[7] {
                            out.push(format!("buffer 3 {:x} {:x} 1 1 {:x} 1 0 1", d[0], d[1], d[2]));
                        } else {
                            out.push(format!("buffer hr {DXGI_ERROR_INVALID_CALL:08x}"));
                        }
                    }
                    ("scpresent", 2) => {
                        hit("DXGI.PRESENT");
                        let (sync, flags) = (w[0], w[1]);
                        // (MEASURED: sync intervals 0..4; TEST and DO_NOT_SEQUENCE present nothing;
                        // ALLOW_TEARING needs the chain's flag; unknown flags refused)
                        let known = 1 | 2 | 4 | 0x100 | 0x200;
                        let bad = sync > 4 || flags & !known != 0 || flags & 0x200 != 0 && (d[11] & 0x800 == 0 || sync != 0);
                        if flags & 4 != 0 {
                            return Err(Fail::Unmodeled("DXGI.PRESENT.RESTART"));
                        }
                        if bad {
                            out.push(format!("present hr {DXGI_ERROR_INVALID_CALL:08x} index {index:x}"));
                        } else {
                            if flags & 3 == 0 {
                                *index = (*index + 1) % d[7];
                            } else {
                                hit("DXGI.PRESENT.TEST");
                            }
                            out.push(format!("present hr 00000000 index {index:x}"));
                        }
                    }
                    ("scresize", 5) => {
                        hit("DXGI.RESIZE");
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
                            out.push(chain_line("resize", d, 0));
                        }
                    }
                    ("sclatency", 1) => {
                        hit("DXGI.LATENCY");
                        // (MEASURED: only with FRAME_LATENCY_WAITABLE_OBJECT; 0 and 0xffffffff
                        // refused, 17 taken; the default 1)
                        if d[11] & 0x40 == 0 {
                            out.push(format!("latency hr {0:08x} get {0:08x} cccccccc", DXGI_ERROR_INVALID_CALL));
                        } else {
                            let hr = if w[0] != 0 && w[0] < 0x8000_0000 {
                                self.latency = w[0];
                                0
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
pub fn chain_desc(w: &[u32; 12]) -> Result<[u32; 12], u32> {
    let mut d = *w;
    let no = |c: bool, id: &'static str| {
        hit(id);
        if c {
            Err(DXGI_ERROR_INVALID_CALL)
        } else {
            Ok(())
        }
    };
    no(!matches!(d[9], 3 | 4), "DXGI.CHAIN.EFFECT")?;
    no(d[7] < 2 || d[7] > 16, "DXGI.CHAIN.BUFFERS")?;
    no(d[4] != 1 || d[5] != 0, "DXGI.CHAIN.SAMPLES")?;
    no(!FLIP.contains(&d[2]), "DXGI.CHAIN.FORMAT")?;
    // (MEASURED, golden dxgi-32 swapchain-*: scaling STRETCH or NONE (ASPECT_RATIO_STRETCH refused
    // for a window), alpha UNSPECIFIED or IGNORE, usage bits up to 0x40, these flags; stereo is
    // taken and kept)
    no(d[8] > 1, "DXGI.CHAIN.SCALING")?;
    no(!matches!(d[10], 0 | 3), "DXGI.CHAIN.ALPHA")?;
    no(d[6] & !0x70 != 0, "DXGI.CHAIN.USAGE")?;
    no(d[11] & !CHAIN_FLAGS != 0, "DXGI.CHAIN.FLAGS")?;
    if d[3] != 0 {
        hit("DXGI.CHAIN.STEREO");
    }
    if d[0] == 0 || d[1] == 0 {
        // (MEASURED: either one 0 takes both from the window)
        hit("DXGI.CHAIN.SIZE");
        (d[0], d[1]) = (64, 64);
    }
    // (MEASURED: x86 runs out of address space for 16385 by 16385)
    if super::arch() == 32 && d[0] as u64 * d[1] as u64 * 4 * d[7] as u64 >= 1 << 30 {
        hit("DXGI.CHAIN.MEMORY");
        return Err(0x8007_000e);
    }
    Ok(d)
}

/// the swap chain flags CreateSwapChainForHwnd takes on Direct3D 12 (MEASURED): ALLOW_MODE_SWITCH,
/// RESTRICT_SHARED_RESOURCE_DRIVER, FRAME_LATENCY_WAITABLE_OBJECT, ALLOW_TEARING, HW_PROTECTED, 0x2000
pub const CHAIN_FLAGS: u32 = 0x2 | 0x20 | 0x40 | 0x800 | 0x1000 | 0x2000;

fn chain_line(what: &str, d: &[u32; 12], index: u32) -> String {
    format!("{what} = {} index {index:x}", d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" "))
}
