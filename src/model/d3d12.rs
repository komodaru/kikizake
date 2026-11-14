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

use super::dxgi_format::{block, channels, planar_depth, Chan, Enc, FLIP};
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
    no(d.w > wmax || d.h > hmax || d.d > dmax, "D3D12.DESC.SIZE")?;
    no(d.mips > full_chain(d), "D3D12.DESC.MIPS")?;
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

/// GetCopyableFootprints' layouts and total, or why the model has none
pub fn footprints(first: u32, count: u32, base: u64, d: &Desc) -> Res<(Vec<Fp>, u64)> {
    if d.dim == 1 {
        if validate(d).is_err() {
            return Err(Fail::Unmodeled("D3D12.FOOTPRINT.INVALID"));
        }
        hit("D3D12.FOOTPRINT.BUFFER");
        if first + count > 1 {
            return Err(Fail::Unmodeled("D3D12.FOOTPRINT.RANGE"));
        }
        let w = d.w as u64;
        let v = (0..count).map(|_| (base, 0, d.w, 1, 1, align(w, 256), 1, w)).collect();
        return Ok((v, if count == 0 { 0 } else { w }));
    }
    if planar_depth(d.fmt) {
        return Err(Fail::Unmodeled("D3D12.FOOTPRINT.PLANAR"));
    }
    let Some((bytes, bw, bh)) = block(d.fmt) else { return Err(Fail::Unmodeled("D3D12.FOOTPRINT.FORMAT")) };
    if validate(d).is_err() {
        return Err(Fail::Unmodeled("D3D12.FOOTPRINT.INVALID"));
    }
    let mips = if d.mips == 0 {
        hit("D3D12.FOOTPRINT.MIPS0");
        full_chain(d)
    } else {
        d.mips
    };
    let array = if d.dim == 4 { 1 } else { d.d };
    if first as u64 + count as u64 > (mips * array) as u64 {
        return Err(Fail::Unmodeled("D3D12.FOOTPRINT.RANGE"));
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
        let pitch = align(row, 256);
        v.push((base + at, d.fmt, fw, fh, depth, pitch, rows, row));
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
    (c.clamp(0.0, 1.0) * m + 0.5) as u64
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
            // (NaN)
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
    let r = (c as f64).round_ties_even();
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
    let d = f32of(clear_input(depth));
    let d = if d.is_nan() {
        hit("D3D12.CLEARDSV.NAN");
        0.0
    } else {
        d.clamp(0.0, 1.0)
    };
    let s = (stencil & 0xff) as u8;
    Some(match f {
        40 => vec![d.to_bits().to_le_bytes().to_vec()],
        55 => vec![(to_unorm(d, 16) as u16).to_le_bytes().to_vec()],
        45 => vec![(to_unorm(d, 24) as u32).to_le_bytes().to_vec(), vec![s]],
        20 => vec![d.to_bits().to_le_bytes().to_vec(), vec![s]],
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
                    out.push(format!("fp {} {:x} {:x} {:x} {:x} {:x} {:x} {}", hex64(f.0), f.1, f.2, f.3, f.4, f.5, f.6, hex64(f.7)));
                }
                out.push(format!("total {}", hex64(total)));
            }
            "alloc12" => {
                if t.len() != 12 {
                    return Err(Fail::Bad);
                }
                let d = Desc::from_words(&words(&t[1..])?);
                if d.dim != 1 {
                    return Err(Fail::Unmodeled("D3D12.ALLOC.TEXTURE"));
                }
                match validate(&d) {
                    Ok(()) => {
                        hit("D3D12.ALLOC.BUFFER");
                        out.push(format!("alloc {:x} 10000", align(d.w as u64, 65536)));
                    }
                    Err(_) => return Err(Fail::Unmodeled("D3D12.ALLOC.INVALID")),
                }
            }
            "create12" => {
                if t.len() != 14 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                let (heap, state, d) = (w[0], w[1], Desc::from_words(&w[2..]));
                let ok = create_ok(heap, state, &d)?;
                out.push(format!("create hr {:08x}", if ok { S_FALSE } else { E_INVALIDARG }));
            }
            "clear12" => {
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                let w = words(&t[1..])?;
                match clear_texel(w[0], [w[1], w[2], w[3], w[4]]) {
                    Some(b) => out.push(format!("clear12 {}", hex(&b))),
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

/// CreateCommittedResource's verdict on a description, a heap type and an initial state
pub fn create_ok(heap: u32, state: u32, d: &Desc) -> Res<bool> {
    hit("D3D12.CREATE");
    if !(1..=3).contains(&heap) {
        hit("D3D12.HEAP.TYPE");
        return Ok(false);
    }
    if validate(d).is_err() {
        return Ok(false);
    }
    if d.dim != 1 {
        let Some(_) = block(d.fmt) else {
            return if d.fmt == 0 || d.fmt > 191 { Ok(false) } else { Err(Fail::Unmodeled("D3D12.CREATE.FORMAT")) };
        };
        if d.fmt != 28 || d.layout != 0 || d.align == 4096 || d.flags & !3 != 0 {
            return Err(Fail::Unmodeled("D3D12.CREATE.FORMAT"));
        }
    }
    if d.flags & !0x1ff != 0 {
        hit("D3D12.DESC.FLAGS");
        return Ok(false);
    }
    if d.dim == 1 && d.flags & 3 != 0 {
        hit("D3D12.DESC.FLAGS");
        return Ok(false);
    }
    if heap != 1 {
        hit("D3D12.HEAP.CPU");
        if d.dim != 1 || d.flags & 7 != 0 {
            return Ok(false);
        }
        return Ok(state == if heap == 2 { 0xac3 } else { 0x400 });
    }
    Err(Fail::Unmodeled("D3D12.CREATE.STATE"))
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
            if f != 0 {
                return Err(Fail::Unmodeled("DXGI.FACTORY.FLAGS"));
            }
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
                let ok = f & !7 == 0 && (!null || f == 0);
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
                match chain_desc(&w)? {
                    Some(d) => {
                        self.chain = Some((d, 0));
                        out.push(chain_line("swapchain", &d, 0));
                    }
                    None => out.push(format!("swapchain hr {DXGI_ERROR_INVALID_CALL:08x}")),
                }
            }
            _ if self.chain.is_none() => out.push("nochain".into()),
            _ => return Err(Fail::Unmodeled("DXGI.CHAIN.OPS")),
        }
        Ok(true)
    }
}

/// GetDesc1 of the swap chain CreateSwapChainForHwnd makes from `w`, None when it refuses it
pub fn chain_desc(w: &[u32; 12]) -> Res<Option<[u32; 12]>> {
    let mut d = *w;
    let no = |c: bool, id: &'static str| {
        hit(id);
        c
    };
    if no(!matches!(d[9], 3 | 4), "DXGI.CHAIN.EFFECT")
        || no(d[7] < 2 || d[7] > 16, "DXGI.CHAIN.BUFFERS")
        || no(d[4] != 1 || d[5] != 0, "DXGI.CHAIN.SAMPLES")
        || no(!FLIP.contains(&d[2]), "DXGI.CHAIN.FORMAT")
    {
        return Ok(None);
    }
    if d[3] != 0 || d[8] > 2 || d[10] != 0 || d[11] != 0 || d[6] != 0x20 {
        return Err(Fail::Unmodeled("DXGI.CHAIN.OTHER"));
    }
    if d[0] == 0 || d[1] == 0 {
        hit("DXGI.CHAIN.SIZE");
        if d[0] == 0 {
            d[0] = 64;
        }
        if d[1] == 0 {
            d[1] = 64;
        }
    }
    if d[0] > 16384 || d[1] > 16384 {
        return Err(Fail::Unmodeled("DXGI.CHAIN.OTHER"));
    }
    Ok(Some(d))
}

fn chain_line(what: &str, d: &[u32; 12], index: u32) -> String {
    format!("{what} = {} index {index:x}", d.iter().map(|v| format!("{v:x}")).collect::<Vec<_>>().join(" "))
}
