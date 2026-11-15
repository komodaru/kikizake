//! D3DX 9 (runner/d3dx9.c): the vertex format functions (D3DXGetFVFVertexSize,
//! D3DXDeclaratorFromFVF, D3DXFVFFromDeclarator, D3DXGetDeclVertexSize, D3DXGetDeclLength) and
//! the 16-bit float conversions (D3DXFloat32To16Array, D3DXFloat16To32Array), from Microsoft
//! Learn's pages of those names; what they leave open, from Windows' answers (spec/d3dx9.txt).
use crate::spec::{clause, open};
use super::d3d9draw::decl_type_size;
use super::{Api, Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const INVALIDCALL: u32 = 0x8876_086c;

#[derive(Default)]
pub struct D3dx9;

fn words(op: &Op, from: usize) -> Res<Vec<u32>> {
    op.tok[from..].iter().map(|t| num(t).map(|v| v as u32).ok_or(Fail::Bad)).collect()
}

/// a vertex element as the executor prints it: (Stream | Offset << 16, Type | Method << 8 |
/// Usage << 16 | UsageIndex << 24)
pub type Elem = (u32, u32);

pub fn elem(offset: u32, ty: u32, usage: u32, index: u32) -> Elem {
    (offset << 16, ty | usage << 16 | index << 24)
}

/// D3DFVF_TEXCOORDSIZEn: the number of floats of texture coordinate set i
pub fn tex_floats(fvf: u32, i: u32) -> u32 {
    // (sets 8..15, which the count's 4 bits allow, read bits 32 and up, which are 0: two floats.
    // MEASURED: d3dx9 fvf-*, 340 FVFs)
    [2, 3, 4, 1][((fvf as u64) >> (16 + 2 * i) & 3) as usize]
}

/// D3DXGetFVFVertexSize
pub fn fvf_size(fvf: u32) -> u32 {
    hit(clause::D3DX9_FVFSIZE);
    let mut n = 0;
    for (bit, size) in [(0x10, 12), (0x20, 4), (0x40, 4), (0x80, 4)] {
        if fvf & bit != 0 {
            n += size;
        }
    }
    n += match fvf & 0x400e {
        0x002 => 12,
        // (XYZW counts nothing: MEASURED, d3dx9 fvf-*)
        0x004 | 0x006 => 16,
        0x008 => 20,
        0x00a => 24,
        0x00c => 28,
        0x00e => 32,
        _ => 0,
    };
    for i in 0..(fvf >> 8 & 0xf) {
        n += 4 * tex_floats(fvf, i);
    }
    n
}

/// D3DXDeclaratorFromFVF: the elements, or INVALIDCALL
pub fn fvf_decl(fvf: u32) -> Result<Vec<Elem>, u32> {
    hit(clause::D3DX9_FVFDECL);
    if fvf & 0x6001 != 0 {
        hit(clause::D3DX9_FVFDECL_RESERVED);
        return Err(INVALIDCALL);
    }
    if fvf >> 8 & 0xf > 8 {
        // (MEASURED: d3dx9 fvf-*, every FVF of 9 to 15 texture coordinate sets)
        hit(clause::D3DX9_FVFDECL_TEXCOUNT);
        return Err(INVALIDCALL);
    }
    let mut v = Vec::new();
    let mut off = 0;
    let mut add = |ty: u32, usage: u32, index: u32| {
        v.push(elem(off, ty, usage, index));
        off += decl_type_size(ty).unwrap();
    };
    let pos = fvf & 0x400e;
    if pos != 0 {
        let blend = (6..=0xe).contains(&(pos & 0xe)) && pos & 0x4000 == 0;
        let idx = fvf & 0x9000 != 0;
        let count = if blend { 1 + ((pos - 6) >> 1) - idx as u32 } else { 0 };
        if count > 4 {
            hit(clause::D3DX9_FVFDECL_POSITION);
            return Err(INVALIDCALL);
        }
        if pos == 4 {
            add(3, 9, 0);
        } else {
            add(2, 0, 0);
        }
        if blend {
            if count > 0 {
                add(count - 1, 1, 0);
            }
            if fvf & 0x1000 != 0 {
                add(5, 2, 0);
            } else if fvf & 0x8000 != 0 {
                add(4, 2, 0);
            }
        }
    }
    for (bit, ty, usage, index) in [(0x10, 2, 3, 0), (0x20, 0, 4, 0), (0x40, 4, 10, 0), (0x80, 4, 10, 1)] {
        if fvf & bit != 0 {
            add(ty, usage, index);
        }
    }
    for i in 0..(fvf >> 8 & 0xf) {
        add(tex_floats(fvf, i) - 1, 5, i);
    }
    Ok(v)
}

/// D3DXFVFFromDeclarator: the FVF of elements in the order D3DXDeclaratorFromFVF makes them,
/// at the offsets it gives them, or INVALIDCALL
pub fn decl_fvf(e: &[Elem]) -> Result<u32, u32> {
    hit(clause::D3DX9_DECLFVF);
    // (the elements, and D3DDECL_END after them: stream 0xff, type UNUSED)
    let at = |i: usize| e.get(i).copied().unwrap_or((0xff, 17));
    let is = |i: usize, ty: u32, usage: u32, index: Option<u32>| {
        let (_, b) = at(i);
        at(i).0 & 0xffff != 0xff && b & 255 == ty && b >> 16 & 255 == usage && index.is_none_or(|x| b >> 24 == x)
    };
    let mut fvf = 0;
    let mut i = 0;
    // (MEASURED, declfvf-*: the position's and the blend weights' usage indices are not looked at)
    if is(0, 2, 0, None) {
        // (the blend weights: their count from the type; then the indices, UBYTE4 or D3DCOLOR)
        let mut pos = 0x002;
        i = 1;
        for (ty, n) in [(3, 4), (2, 3), (1, 2), (0, 1)] {
            if is(1, ty, 1, None) {
                pos = 0x006 + 2 * (n - 1);
                i = 2;
                break;
            }
        }
        if is(i, 0, 2, Some(0)) && i == 2 {
            hit(clause::D3DX9_DECLFVF_INVALID);
            return Err(INVALIDCALL);
        }
        for (ty, beta) in [(5, 0x1000), (4, 0x8000)] {
            if is(i, ty, 2, Some(0)) {
                // (indices without weights: one weight's room, XYZB1)
                pos = if i == 1 { 0x006 } else { pos + 2 };
                fvf |= beta;
                i += 1;
                break;
            }
        }
        fvf |= pos;
    } else if is(0, 3, 9, None) {
        fvf |= 0x004;
        i = 1;
    }
    for (bit, ty, usage, index) in [(0x10, 2, 3, None), (0x20, 0, 4, Some(0)), (0x40, 4, 10, Some(0)), (0x80, 4, 10, Some(1))] {
        if is(i, ty, usage, index) {
            fvf |= bit;
            i += 1;
        }
    }
    let mut n = 0;
    while at(i).0 & 0xffff != 0xff && n < 8 {
        let size = (0..4).find(|&k| is(i, k, 5, Some(n)));
        let Some(k) = size else {
            hit(clause::D3DX9_DECLFVF_INVALID);
            return Err(INVALIDCALL);
        };
        // (D3DFVF_TEXCOORDSIZEn: 1 float is 3, 2 is 0, 3 is 1, 4 is 2)
        fvf |= [3, 0, 1, 2][k as usize] << (16 + 2 * n);
        n += 1;
        i += 1;
    }
    fvf |= n << 8;
    let mut off = 0;
    for x in e.iter().take_while(|x| x.0 & 0xffff != 0xff) {
        // (and in stream 0: MEASURED, d3dx9 decl-6)
        if x.0 >> 16 != off || x.0 & 0xffff != 0 {
            hit(clause::D3DX9_DECLFVF_INVALID);
            return Err(INVALIDCALL);
        }
        off += decl_type_size(x.1 & 255).unwrap_or(0);
    }
    Ok(fvf)
}

/// x9load: D3DXLoadSurfaceFromMemory of a filled picture into a system memory surface
fn load(op: &Op, out: &mut Vec<String>) -> Res<()> {
    use super::d3d9draw::{fill_bytes, format, supported, Kind};
    let t = op.tok.as_slice();
    let w: Vec<u32> = (1..5).map(|i| t.get(i).and_then(|x| num(x)).map(|v| v as u32).ok_or(Fail::Bad)).collect::<Res<_>>()?;
    let (df, sf, wd, ht) = (w[0], w[1], w[2] as i32, w[3] as i32);
    let seed = if t.get(5).is_some_and(|s| s == "ramp") { -1 } else { num(&t[5]).ok_or(Fail::Bad)? };
    hit(clause::D3DX9_LOAD);
    if matches!(df, 20 | 27 | 29 | 33) || !supported(df, false, false) {
        // (MEASURED: CreateOffscreenPlainSurface takes none of the formats textures do not, nor
        // R8G8B8, X8B8G8R8: d3dx9load)
        out.push(format!("load surface hr {INVALIDCALL:08x}"));
        return Ok(());
    }
    let (sm, dm) = (format(sf).ok_or(Fail::Bad)?, format(df).ok_or(Fail::Bad)?);
    if !matches!(sm.kind, Kind::Int | Kind::F16 | Kind::F32) || !matches!(dm.kind, Kind::Int | Kind::F16 | Kind::F32) {
        return Err(Fail::Unmodeled(open::D3DX9_LOAD_FORMAT));
    }
    if sm.ch.iter().chain(&dm.ch).any(|c| is_bump(c.c)) {
        hit(clause::D3DX9_LOAD_SIGNED);
    }
    let src = fill_bytes(&sm, wd, ht, seed);
    out.push(format!("px rt {wd} {ht} {}", dm.bytes * 8));
    // (MEASURED load-R16F .. load-A32B32G32R32F: halves of exponent 31 read as numbers and NaNs,
    // numbers past the half range written as f16_load has them, D3DX9.LOAD.F16RANGE; open: on
    // x64 a denormal half into R16F rounded or truncated, b75ffa2a rounded and b66623f9 not)
    let pixels: Vec<[Sample; 4]> = src.chunks(sm.bytes).map(|p| load_decode(&sm, sf, p)).collect();
    let one_denormal = |p: &[Sample; 4]| p.iter().take(1).any(|&(Sample::Unsigned(v) | Sample::Signed(v))| f16_x64(v.to_bits(), false) != f16_x64(v.to_bits(), true));
    if crate::model::arch() == 64 && dm.kind == Kind::F16 && dm.ch.len() == 1 && sf != df && pixels.iter().any(one_denormal) {
        return Err(Fail::Unmodeled(open::D3DX9_LOAD_F16RANGE));
    }
    if sm.kind == Kind::F16 || dm.kind == Kind::F16 {
        hit(clause::D3DX9_LOAD_F16HALVES);
    }
    if sf == df {
        // (a format into itself: the bytes as they are)
        hit(clause::D3DX9_LOAD_COPY);
        for row in src.chunks(wd as usize * sm.bytes) {
            out.push(row.iter().map(|x| format!("{x:02x}")).collect());
        }
        return Ok(());
    }
    for row in src.chunks(wd as usize * sm.bytes) {
        out.push(row.chunks(sm.bytes).enumerate().flat_map(|(i, p)| load_encode(&dm, sm.kind, load_decode(&sm, sf, p), (i, wd as usize))).map(|x| format!("{x:02x}")).collect());
    }
    Ok(())
}

/// a channel as D3DX holds it between formats: an unsigned one in 0..1 (unorm) or as it is
/// (floats), a signed (bump) one in -1..1
#[derive(Clone, Copy)]
enum Sample {
    Unsigned(f32),
    Signed(f32),
}

/// a source pixel as D3DX reads it: (r, g, b, a), an n-bit channel v times the f32 nearest
/// 1 / (2^n - 1), a signed one its two's complement times 1 / (2^(n-1) - 1) (MEASURED:
/// d3dx9load)
fn load_decode(f: &super::d3d9draw::Fmt, code: u32, p: &[u8]) -> [Sample; 4] {
    use super::d3d9draw::{to_rgba, Kind};
    let mut word = [0u8; 16];
    word[..p.len()].copy_from_slice(p);
    let w = u128::from_le_bytes(word);
    let ch: Vec<(u8, f64)> = f
        .ch
        .iter()
        .map(|c| {
            let raw = (w >> c.shift) as u64 & ((1u64 << c.bits) - 1);
            // (floats as they are, NaNs too)
            let v = match f.kind {
                Kind::F16 => f16_to_f32(raw as u16),
                Kind::F32 => f32::from_bits(raw as u32),
                _ if is_bump(c.c) => {
                    let half = 1i64 << (c.bits - 1);
                    let sv = raw as i64 - if raw as i64 >= half { 2 * half } else { 0 };
                    (sv as f32 * (1.0 / (half - 1) as f32)).max(-1.0)
                }
                _ => raw as f32 * (1.0 / ((1u64 << c.bits) - 1) as f32),
            };
            // (a bump format's luminance is the alpha: MEASURED d3dx9load load-L6V5U5,
            // load-X8L8V8U8)
            let name = if c.c == b'l' && f.ch.iter().any(|c| is_bump(c.c)) { b'a' } else { c.c };
            (name, v as f64)
        })
        .collect();
    let rgba = to_rgba(code, &ch).map(|v| v as f32);
    std::array::from_fn(|k| {
        let bump = [b'u', b'v', b'w', b'q'][k];
        if k == 2 && code == 117 {
            // (CxV8U8's W: sqrt(1 - U^2 - V^2))
            Sample::Signed((1.0 - rgba[0] * rgba[0] - rgba[1] * rgba[1]).max(0.0).sqrt())
        } else if f.kind == Kind::Int && f.ch.iter().any(|c| c.c == bump) {
            Sample::Signed(rgba[k])
        } else {
            Sample::Unsigned(rgba[k])
        }
    })
}

/// a signed (bump) channel: U, V, W, Q
fn is_bump(c: u8) -> bool {
    matches!(c, b'u' | b'v' | b'w' | b'q')
}

/// (r, g, b, a) into a pixel as D3DX writes it: unorm channels rounded to nearest, X bits 0,
/// luminance Rec. 709's 0.2125 r + 0.7154 g + 0.0721 b, halves rounded to nearest (MEASURED:
/// d3dx9load)
fn load_encode(f: &super::d3d9draw::Fmt, src: super::d3d9draw::Kind, px: [Sample; 4], (at, row): (usize, usize)) -> Vec<u8> {
    use super::d3d9draw::{round_even, Kind};
    hit(clause::D3DX9_LOAD_CONVERT);
    // (a signed channel into an unsigned or luminance one as (s + 1) / 2, into a float as it is)
    let c: [f32; 4] = px.map(|x| match (x, f.kind) {
        (Sample::Unsigned(v), _) | (Sample::Signed(v), Kind::F16 | Kind::F32) => v,
        (Sample::Signed(v), _) => (v + 1.0) / 2.0,
    });
    let mut w: u128 = 0;
    for (ci, ch) in f.ch.iter().enumerate() {
        if f.kind == Kind::Int && is_bump(ch.c) {
            // (into a signed channel: an unsigned one as 2 x - 1, a signed one as it is,
            // rounded to the nearest of the 2^(n-1) - 1 steps)
            let k = [b'u', b'v', b'w', b'q'].iter().position(|&b| b == ch.c).unwrap_or(0);
            let v = match (px[k], src) {
                (Sample::Unsigned(x), Kind::Int) => 2.0 * x - 1.0,
                (Sample::Unsigned(x), _) | (Sample::Signed(x), _) => x,
            };
            let half = 1i64 << (ch.bits - 1);
            // (MEASURED load-*-signed: v (2^(n-1) - 1) + 1/2 truncated toward 0)
            // (a NaN as 1, as into unorm channels: load-R32F-signed)
            let v = if v.is_nan() { 1.0 } else { v.clamp(-1.0, 1.0) };
            let q = (v * (half - 1) as f32 + 0.5) as i64;
            w |= ((q & (2 * half - 1)) as u128) << ch.shift;
            continue;
        }
        // (into unorm channels each clamped to 0..1 first, luminance's too, a NaN to 1)
        let k = |i: usize| if f.kind != Kind::Int { c[i] } else if c[i].is_nan() { 1.0 } else { c[i].clamp(0.0, 1.0) };
        let v = match ch.c {
            b'r' => k(0),
            b'g' => k(1),
            b'b' => k(2),
            b'a' => k(3),
            b'l' => 0.2125 * k(0) + 0.7154 * k(1) + 0.0721 * k(2),
            _ => continue,
        };
        let raw = match f.kind {
            // (a row's floats through D3DXFloat32To16Array: whole blocks, in this suite)
            Kind::F16 => f16_load(v.to_bits(), f.ch.len(), ci + f.ch.len() * at, f.ch.len() * row) as u64,
            Kind::F32 => v.to_bits() as u64,
            _ => {
                let m = ((1u64 << ch.bits) - 1) as f32;
                // (x86 multiplies on the x87 at more than f32's precision and rounds to nearest,
                // x64 adds 1/2 to the f32 product and truncates: load-L6V5U5)
                match crate::model::arch() {
                    64 => (v.clamp(0.0, 1.0) * m + 0.5) as u64,
                    _ => round_even(v.clamp(0.0, 1.0) as f64 * m as f64) as u64,
                }
            }
        };
        w |= (raw as u128) << ch.shift;
    }
    w.to_le_bytes()[..f.bytes].to_vec()
}

/// a float as D3DXLoadSurfaceFromMemory writes it into a half channel, the channel k of n of
/// the row's pixels (at = n x + k of len = n x width) (MEASURED d3dx9load load-R32F, -G32R32F,
/// -A32B32G32R32F, -A16B16G16R16F): four halves a pixel as the row's array through
/// D3DXFloat32To16Array; two a pixel as an array of the pixel's two; one through
/// D3DXFloat32To16, as an array's rest on x86, as its bulk on x64
fn f16_load(x: u32, n: usize, at: usize, len: usize) -> u16 {
    match (n, crate::model::arch()) {
        (4, 64) => f16_x64(x, at < len - len % 4),
        (4, _) if at < len - len % 8 => f16_block(x),
        (1, 64) => f16_x64(x, true),
        (_, 64) => f16_x64(x, false),
        _ => f16_one(x),
    }
}

/// D3DXGetDeclVertexSize of stream 0: the end of the furthest element of the stream
pub fn decl_size(e: &[Elem]) -> u32 {
    hit(clause::D3DX9_DECLSIZE);
    e.iter().filter(|x| x.0 & 0xffff == 0).filter_map(|x| decl_type_size(x.1 & 255).map(|s| (x.0 >> 16) + s)).max().unwrap_or(0)
}

impl Api for D3dx9 {
    fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        match t[0].as_str() {
            "x9fvfsize" => out.push(format!("fvfsize {}", fvf_size(words(op, 1)?[0]))),
            "x9fvfdecl" => match fvf_decl(*words(op, 1)?.first().ok_or(Fail::Bad)?) {
                Ok(e) => out.push(format!("fvfdecl {}{}", e.len(), e.iter().map(|(a, b)| format!(" {a:08x} {b:08x}")).collect::<String>())),
                Err(hr) => out.push(format!("fvfdecl hr {hr:08x}")),
            },
            "x9declsize" => {
                let w = words(op, 1)?;
                let e: Vec<Elem> = w.chunks(2).map(|c| (c[0], c[1])).collect();
                hit(clause::D3DX9_DECLLENGTH);
                out.push(format!("declsize {} len {}", decl_size(&e), e.len()));
            }
            "x9cpu" => {
                // (the executor's processor, an input: crate::model::set_cpu)
                let Some(c) = crate::model::cpu() else {
                    return Err(Fail::Unmodeled(open::D3DX9_F16_CPU));
                };
                out.push(c);
            }
            "x9f16" => {
                hit(clause::D3DX9_F16);
                let w = words(op, 1)?;
                // (MEASURED, d3dx9 f16-*, 3688 values: x86 does the array 8 at a time and then
                // the rest one by one; x64 4 at a time and then the rest, conversions that differ,
                // or on another processor as x86 does: golden 465fd71)
                let x64 = crate::model::arch() == 64 && !cpu_as_x86()?;
                let bulk = if x64 { w.len() - w.len() % 4 } else { w.len() - w.len() % 8 };
                let f = |i: usize, x: u32| match (x64, i < bulk) {
                    (false, true) => f16_block(x),
                    (false, false) => f16_one(x),
                    (true, b) => f16_x64(x, b),
                };
                out.push(format!("f16{}", w.iter().enumerate().map(|(i, &x)| format!(" {:04x}", f(i, x))).collect::<String>()));
            }
            "x9f32" => {
                hit(clause::D3DX9_F32);
                out.push(format!("f32{}", words(op, 1)?.iter().map(|&x| format!(" {:08x}", f16_to_f32(x as u16).to_bits())).collect::<String>()));
            }
            "x9declfvf" => {
                let w = words(op, 1)?;
                let e: Vec<Elem> = w.chunks(2).map(|c| (c[0], c[1])).collect();
                match decl_fvf(&e) {
                    Ok(f) => out.push(format!("declfvf {f:08x}")),
                    Err(hr) => out.push(format!("declfvf hr {hr:08x}")),
                }
            }
            "x9load" => load(op, out)?,
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// D3DXFloat16To32Array of one value: exponent 31 is a number like any other, no infinities
/// or NaNs (MEASURED: d3dx9 f32-*, every half)
pub fn f16_to_f32(h: u16) -> f32 {
    let (s, e, m) = (if h & 0x8000 != 0 { -1.0 } else { 1.0 }, (h >> 10 & 31) as i32, (h & 1023) as f64);
    (s * if e == 0 { m * 2f64.powi(-24) } else { (1.0 + m / 1024.0) * 2f64.powi(e - 15) }) as f32
}

/// a float as a half the way D3DXFloat32To16Array does most of an array (8 at a time): IEEE,
/// ties toward zero; infinity past 65504 and for infinities and NaNs (MEASURED: d3dx9 f16-*)
pub fn f16_block(x: u32) -> u16 {
    let s = (x >> 16 & 0x8000) as u16;
    let i = x & 0x7fff_ffff;
    if i >= 0x4780_0000 {
        return s | 0x7c00;
    }
    s | ((half_bits(i) + 0xfff) >> 13) as u16
}

/// a float as a half the way D3DXFloat32To16Array does the rest of an array (fewer than 8):
/// exponent 31 a number (up to 131008, past it and for infinities and NaNs 0x7fff), ties to
/// even (MEASURED: d3dx9 f16-*)
pub fn f16_one(x: u32) -> u16 {
    let s = (x >> 16 & 0x8000) as u16;
    let i = x & 0x7fff_ffff;
    if i > 0x47ff_efff {
        return s | 0x7fff;
    }
    let b = half_bits(i);
    s | ((b + 0xfff + (b >> 13 & 1)) >> 13) as u16
}

/// a float as a half the way x64's D3DXFloat32To16Array does: exponent 31 a number, ties to
/// even, 0x7fff past 131008 and for infinities, a NaN's mantissa rounded with the quiet bit set;
/// denormals rounded 4 at a time, the rest of the array's truncated (MEASURED: d3dx9 f16-*)
/// whether x64's D3DXFloat32To16Array goes as x86's on the executor's processor (OPEN, guessed:
/// where the processor lacks AVX, since an AMD one with AVX and F16C went 4 at a time;
/// D3DX9.F16.CPU)
fn cpu_as_x86() -> Res<bool> {
    if crate::model::arch() != 64 {
        return Ok(false);
    }
    let c = crate::model::cpu().ok_or(Fail::Unmodeled(open::D3DX9_F16_CPU))?;
    let ecx = c.split(' ').nth(2).and_then(|v| u32::from_str_radix(v, 16).ok()).ok_or(Fail::Bad)?;
    Ok(ecx & 1 << 28 == 0)
}

pub fn f16_x64(x: u32, four: bool) -> u16 {
    let s = (x >> 16 & 0x8000) as u16;
    let (i, m) = (x & 0x7fff_ffff, x & 0x7f_ffff);
    if i > 0x7f80_0000 {
        return s | 0x7e00 | ((m + 0xfff + (m >> 13 & 1)) >> 13 & 0x3ff) as u16;
    }
    if i > 0x47ff_efff {
        return s | 0x7fff;
    }
    let b = half_bits(i);
    if i < 0x3880_0000 && !four {
        return s | (b >> 13) as u16;
    }
    s | ((b + 0xfff + (b >> 13 & 1)) >> 13) as u16
}

/// a float's magnitude (below 2^17) as a half shifted left 13: the exponent rebiased, or
/// the mantissa shifted down to a denormal
fn half_bits(i: u32) -> u32 {
    if i < 0x3880_0000 {
        let sh = 113 - (i >> 23) as i32;
        if sh > 24 {
            0
        } else {
            (0x80_0000 | i & 0x7f_ffff) >> sh
        }
    } else {
        i.wrapping_add(0xc800_0000)
    }
}
