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
    let mut add = |ty: u32, usage: u32, index: u32| -> Result<(), u32> {
        v.push(elem(off, ty, usage, index));
        off += decl_type_size(ty).ok_or(INVALIDCALL)?;
        Ok(())
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
            add(3, 9, 0)?;
        } else {
            add(2, 0, 0)?;
        }
        if blend {
            if count > 0 {
                add(count - 1, 1, 0)?;
            }
            if fvf & 0x1000 != 0 {
                add(5, 2, 0)?;
            } else if fvf & 0x8000 != 0 {
                add(4, 2, 0)?;
            }
        }
    }
    for (bit, ty, usage, index) in [(0x10, 2, 3, 0), (0x20, 0, 4, 0), (0x40, 4, 10, 0), (0x80, 4, 10, 1)] {
        if fvf & bit != 0 {
            add(ty, usage, index)?;
        }
    }
    for i in 0..(fvf >> 8 & 0xf) {
        add(tex_floats(fvf, i) - 1, 5, i)?;
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

/// where x9load (a plain system memory surface) and x9loadtex (level 0 of a system memory texture)
/// load
#[derive(Clone, Copy, PartialEq)]
enum Dest {
    Plain,
    Texture,
}

/// an x9load source picture: the ramp (bytes 0, 1, 2, ..), a seed's random bytes, or the bytes
/// given (b:<hex>, the rest 0)
enum Picture {
    Ramp,
    Seed(i64),
    Given(Vec<u8>),
}

fn hex_bytes(h: &str) -> Option<Vec<u8>> {
    (0..h.len() / 2).map(|i| h.get(2 * i..2 * i + 2).and_then(|b| u8::from_str_radix(b, 16).ok())).collect()
}

/// x9load: D3DXLoadSurfaceFromMemory of a filled picture into a system memory surface
fn load(op: &Op, out: &mut Vec<String>, into: Dest) -> Res<()> {
    use super::d3d9draw::{fill_bytes, format, supported, Kind};
    let t = op.tok.as_slice();
    let w: Vec<u32> = (1..5).map(|i| t.get(i).and_then(|x| num(x)).map(|v| v as u32).ok_or(Fail::Bad)).collect::<Res<_>>()?;
    let (df, sf, wd, ht) = (w[0], w[1], w[2] as i32, w[3] as i32);
    let picture = match t.get(5).map(String::as_str) {
        Some("ramp") => Picture::Ramp,
        Some(b) if b.starts_with("b:") => Picture::Given(hex_bytes(&b[2..]).ok_or(Fail::Bad)?),
        Some(n) => Picture::Seed(num(n).ok_or(Fail::Bad)?),
        None => return Err(Fail::Bad),
    };
    hit(clause::D3DX9_LOAD);
    let dxt = format(df).is_some_and(|m| matches!(m.kind, Kind::Dxt(_)));
    if into == Dest::Texture && !supported(df, false, false) {
        // (CreateTexture as d3d9draw's textures, D3D9.FORMATS)
        out.push(format!("load texture hr {INVALIDCALL:08x}"));
        return Ok(());
    }
    if into == Dest::Plain && df != super::d3d9draw::fourcc(b"YUY2") && (matches!(df, 20 | 27 | 29 | 33) || dxt || !supported(df, false, false)) {
        // (MEASURED: CreateOffscreenPlainSurface takes none of the formats textures do not, nor
        // R8G8B8, X8B8G8R8, nor the DXT ones; of the YUV ones YUY2 but not UYVY: d3dx9load,
        // load-into-*)
        out.push(format!("load surface hr {INVALIDCALL:08x}"));
        return Ok(());
    }
    let (sm, dm) = (format(sf).ok_or(Fail::Bad)?, format(df).ok_or(Fail::Bad)?);
    let yuy2 = df == super::d3d9draw::fourcc(b"YUY2");
    let into_dxt = match dm.kind {
        Kind::Dxt(n) => DxtAlpha::of(n),
        Kind::Int | Kind::F16 | Kind::F32 | Kind::Yuv => None,
    };
    if !(matches!(dm.kind, Kind::Int | Kind::F16 | Kind::F32) || into_dxt.is_some() && wd % 4 == 0 && ht % 4 == 0 || yuy2 && matches!(sm.kind, Kind::Int | Kind::F16 | Kind::F32)) {
        return Err(Fail::Unmodeled(open::D3DX9_LOAD_FORMAT));
    }
    if sm.ch.iter().chain(&dm.ch).any(|c| is_bump(c.c)) {
        hit(clause::D3DX9_LOAD_SIGNED);
    }
    let src = match picture {
        Picture::Ramp => fill_bytes(&sm, wd, ht, -1),
        Picture::Seed(seed) => fill_bytes(&sm, wd, ht, seed),
        Picture::Given(mut b) => {
            b.resize(fill_bytes(&sm, wd, ht, -1).len(), 0);
            b
        }
    };
    if into_dxt.is_none() {
        out.push(format!("px rt {wd} {ht} {}", dm.bytes * 8));
    }
    // (MEASURED load-R16F .. load-A32B32G32R32F: halves of exponent 31 read as numbers and NaNs,
    // numbers past the half range written as f16_load has them, D3DX9.LOAD.F16RANGE)
    let pixels: Vec<[Sample; 4]> = match sm.kind {
        Kind::Dxt(n) => {
            let alpha = DxtAlpha::of(n).ok_or(Fail::Bad)?;
            (0..ht).flat_map(|y| (0..wd).map(move |x| (x, y))).map(|(x, y)| dxt_d3dx(alpha, &src, wd, x, y).map(Sample::Unsigned)).collect()
        }
        Kind::Yuv => {
            hit(clause::D3DX9_LOAD_YUV);
            (0..wd as usize * ht as usize).map(|i| yuv_d3dx(sf, &src, wd as usize, i).map(Sample::Unsigned)).collect()
        }
        Kind::Int | Kind::F16 | Kind::F32 => src.chunks(sm.bytes).map(|p| load_decode(&sm, sf, p)).collect(),
    };
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
    if let Some(alpha) = into_dxt {
        hit(clause::D3DX9_LOAD_DXTENC);
        let bsz = if alpha == DxtAlpha::Punch { 8 } else { 16 };
        out.push(format!("px rt {wd} {ht} -{bsz}"));
        // (MEASURED loadtex-into-DXT*: floats clamped to 0..1 first, a NaN as 1)
        let unit = |x: Sample| {
            let v = match (x, sm.kind) {
                (Sample::Unsigned(v), _) | (Sample::Signed(v), Kind::F16 | Kind::F32) => v,
                (Sample::Signed(v), _) => (v + 1.0) / 2.0,
            };
            if v.is_nan() { 1.0 } else { v.clamp(0.0, 1.0) }
        };
        for by in 0..ht / 4 {
            let mut row = String::new();
            for bx in 0..wd / 4 {
                let at = |i: usize| pixels[usize::try_from((by * 4 + i as i32 / 4) * wd + bx * 4 + i as i32 % 4).unwrap_or(0)];
                // (MEASURED loadtex-into-DXT*: x64 in f32, x86 on the x87 with its rounding
                // control, the clean-room notes)
                let px = |i: usize| -> [f32; 4] { let p = at(i); std::array::from_fn(|k| unit(p[k])) };
                let block = match crate::model::arch() {
                    64 => dxt_block::<f32>(alpha, std::array::from_fn(px)),
                    _ => dxt_block::<super::dxtenc::X87>(alpha, std::array::from_fn(|i| px(i).map(<super::dxtenc::X87 as super::dxtenc::Real>::of))),
                };
                let block = match block {
                    Ok(block) => block,
                    Err(super::dxtenc::Below::Unknown) => return Err(Fail::Unmodeled(open::D3DX9_LOAD_FORMAT)),
                    Err(super::dxtenc::Below::Fault) => {
                        hit(clause::D3DX9_LOAD_NEGFAULT);
                        return Err(Fail::Crash);
                    }
                };
                row.extend(block.iter().map(|x| format!("{x:02x}")));
            }
            out.push(row);
        }
        return Ok(());
    }
    if yuy2 {
        hit(clause::D3DX9_LOAD_YUY2);
        for row in pixels.chunks(wd as usize) {
            let bytes: Vec<u8> = row.chunks(2).flat_map(yuy2_pair).collect();
            out.push(bytes[..row.len() * 2].iter().map(|x| format!("{x:02x}")).collect());
        }
        return Ok(());
    }
    for row in pixels.chunks(wd as usize) {
        let bytes: Vec<Vec<u8>> = row.iter().enumerate().map(|(i, &p)| load_encode(&dm, sm.kind, p, (i, wd as usize))).collect::<Res<_>>()?;
        out.push(bytes.iter().flatten().map(|x| format!("{x:02x}")).collect());
    }
    Ok(())
}

/// a 4 x 4 block into DXT1 .. DXT5 as D3DX encodes it (D3DX9.LOAD.DXTENC)
fn dxt_block<F: super::dxtenc::Real>(alpha: DxtAlpha, px: [[F; 4]; 16]) -> Result<Vec<u8>, super::dxtenc::Below> {
    use super::dxtenc::{encode_alpha4, encode_alpha8, encode_bc1, Steps};
    let premul = |px: [[F; 4]; 16]| px.map(|c| [c[0] * c[3], c[1] * c[3], c[2] * c[3], c[3]]);
    match alpha {
        DxtAlpha::Punch => {
            let keyed = px.iter().filter(|c| c[3] < F::of(0.5)).count();
            if keyed == 16 {
                return Ok(vec![0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
            }
            Ok(encode_bc1(&px, if keyed > 0 { Steps::Three } else { Steps::Four })?.to_vec())
        }
        DxtAlpha::Explicit { premultiplied } => {
            let px = if premultiplied { premul(px) } else { px };
            Ok([encode_alpha4(&px), encode_bc1(&px, Steps::Four)?].concat())
        }
        DxtAlpha::Interpolated { premultiplied } => {
            let px = if premultiplied { premul(px) } else { px };
            Ok([encode_alpha8(&px), encode_bc1(&px, Steps::Four)?].concat())
        }
    }
}

/// a pair of pixels into YUY2 as D3DX writes it (MEASURED load-into-YUY2, load-yuy2-probe):
/// R, G, B in 0 .. 255 (a signed one as (s + 1) / 2; load-yuy2-from-*) into yuy2_bytes
fn yuy2_pair(p: &[[Sample; 4]]) -> [u8; 4] {
    let rgb = |s: &[Sample; 4]| -> [f64; 3] {
        std::array::from_fn(|k| match s[k] {
            Sample::Unsigned(v) => f64::from(v.clamp(0.0, 1.0)) * 255.0,
            Sample::Signed(v) => f64::from(((v + 1.0) / 2.0).clamp(0.0, 1.0)) * 255.0,
        })
    };
    let c0 = rgb(&p[0]);
    yuy2_bytes(c0, p.get(1).map_or(c0, rgb))
}

/// two pixels' R, G, B (0 .. 255) as YUY2's Y0 U Y1 V (D3DX9.LOAD.YUY2): each Y
/// 16 + (65.481 R + 128.553 G + 24.966 B) / 255, U and V the first pixel's
/// 128 + (-37.797 R - 74.203 G + 112 B) / 255 and 128 + (112 R - 93.786 G - 18.214 B) / 255,
/// each offset plus 1/2 truncated toward 0, in f64
pub fn yuy2_bytes(c0: [f64; 3], c1: [f64; 3]) -> [u8; 4] {
    let q = |v: f64| (v + 0.5).trunc();
    let byte = |v: f64| u8::try_from(v as i64).unwrap_or(if v < 0.0 { 0 } else { 255 });
    let y = |c: [f64; 3]| byte(16.0 + q((65.481 * c[0] + 128.553 * c[1] + 24.966 * c[2]) / 255.0));
    let u = byte(128.0 + q((-37.797 * c0[0] - 74.203 * c0[1] + 112.0 * c0[2]) / 255.0));
    let v = byte(128.0 + q((112.0 * c0[0] - 93.786 * c0[1] - 18.214 * c0[2]) / 255.0));
    [y(c0), u, y(c1), v]
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
                _ if is_bump(c.c) => snorm_read(raw, c.bits),
                _ => unorm_read(raw, c.bits),
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

/// how a DXT format keeps its alpha
#[derive(Clone, Copy, PartialEq)]
enum DxtAlpha {
    /// DXT1: the colour block's three colour mode index 3
    Punch,
    /// DXT2 (premultiplied) and DXT3: 4 bits a texel
    Explicit { premultiplied: bool },
    /// DXT4 (premultiplied) and DXT5: two endpoints and 3 bit indices
    Interpolated { premultiplied: bool },
}

impl DxtAlpha {
    fn of(n: u32) -> Option<DxtAlpha> {
        match n {
            1 => Some(DxtAlpha::Punch),
            2 | 3 => Some(DxtAlpha::Explicit { premultiplied: n == 2 }),
            4 | 5 => Some(DxtAlpha::Interpolated { premultiplied: n == 4 }),
            _ => None,
        }
    }
}

/// a DXT texel as D3DX reads it, (r, g, b, a), in f32 (MEASURED: d3dx9load load-DXT*):
/// endpoints c * (1 / 31) and c * (1 / 63); the three colour mode whenever c0 <= c1, in every
/// DXT format; index 2 a + (b - a) / 3 (four colours) or a + (b - a) * 0.5, index 3
/// a + (b - a) * (2 / 3) or black; DXT3's alpha a4 * (1 / 15), DXT5's ((n - k) a0 + k a1) * (1 / n)
/// of a0, a1 as b * (1 / 255); DXT2 and DXT4 divided back, colour * (1 / a) at most 1, 0 where a is 0
fn dxt_d3dx(alpha: DxtAlpha, src: &[u8], w: i32, x: i32, y: i32) -> [f32; 4] {
    hit(clause::D3DX9_LOAD_DXT);
    let bsz = if alpha == DxtAlpha::Punch { 8 } else { 16 };
    let bi = usize::try_from((y / 4) * ((w + 3) / 4) + x / 4).unwrap_or(0) * bsz;
    let Some(b) = src.get(bi..bi + bsz) else { return [0.0; 4] };
    let t = u32::try_from((y % 4) * 4 + x % 4).unwrap_or(0);
    let cb = &b[bsz - 8..];
    let (c0, c1) = (u16::from_le_bytes([cb[0], cb[1]]), u16::from_le_bytes([cb[2], cb[3]]));
    let idx = u32::from_le_bytes([cb[4], cb[5], cb[6], cb[7]]) >> (2 * t) & 3;
    let [r, g, bl] = dxt_colour(c0, c1, idx);
    let mut c = [r, g, bl, 1.0];
    let premultiplied = match alpha {
        DxtAlpha::Punch => {
            c[3] = if c0 <= c1 && idx == 3 { 0.0 } else { 1.0 };
            false
        }
        DxtAlpha::Explicit { premultiplied } => {
            let a4 = u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]) >> (4 * t) & 15;
            c[3] = dxt3_alpha(u8::try_from(a4).unwrap_or(0));
            premultiplied
        }
        DxtAlpha::Interpolated { premultiplied } => {
            let ai = u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]) >> (3 * t) & 7;
            c[3] = dxt5_alpha(b[0], b[1], u8::try_from(ai).unwrap_or(0));
            premultiplied
        }
    };
    if premultiplied {
        for k in 0..3 {
            c[k] = if c[3] == 0.0 { 0.0 } else { (c[k] * (1.0 / c[3])).min(1.0) };
        }
    }
    c
}

/// a DXT colour block's texel of index idx (0..3) as D3DX reads it, (r, g, b) in f32
/// (D3DX9.LOAD.DXT): endpoints c * (1/31) and c * (1/63); four colours where c0 > c1, index 2
/// a + (b - a) * (1/3), index 3 a + (b - a) * (2/3); else three, index 2 a + (b - a) * 0.5,
/// index 3 black
pub fn dxt_colour(c0: u16, c1: u16, idx: u32) -> [f32; 3] {
    let unit = |c: u16, bits: u32| -> f32 {
        let m = (1u16 << bits) - 1;
        f32::from(c & m) * (1.0 / f32::from(m))
    };
    let ex = |c: u16| [unit(c >> 11, 5), unit(c >> 5, 6), unit(c, 5)];
    let (e0, e1) = (ex(c0), ex(c1));
    let four = c0 > c1;
    std::array::from_fn(|k| {
        let (a, b) = (e0[k], e1[k]);
        match (idx, four) {
            (0, _) => a,
            (1, _) => b,
            (2, true) => a + (b - a) * (1.0 / 3.0),
            (3, true) => a + (b - a) * (2.0 / 3.0),
            (2, false) => a + (b - a) * 0.5,
            (_, false) | (_, true) => 0.0,
        }
    })
}

/// DXT3's alpha of 4 bits a4 as D3DX reads it: a4 * (1/15) in f32 (D3DX9.LOAD.DXT)
pub fn dxt3_alpha(a4: u8) -> f32 {
    f32::from(a4 & 15) * (1.0 / 15.0)
}

/// DXT5's alpha of index ai (0..7) between endpoints b0, b1 as D3DX reads it, in f32
/// (D3DX9.LOAD.DXT): the endpoints b * (1/255); between them ((n - k) a0 + k a1) * (1/n), n 7
/// where b0 > b1, else 5 with indices 6 and 7 for 0 and 1
pub fn dxt5_alpha(b0: u8, b1: u8, ai: u8) -> f32 {
    let (a0, a1) = (f32::from(b0) * (1.0 / 255.0), f32::from(b1) * (1.0 / 255.0));
    let mix = |k: u8, n: u8| (f32::from(n.saturating_sub(k)) * a0 + f32::from(k) * a1) * (1.0 / f32::from(n));
    match ai & 7 {
        0 => a0,
        1 => a1,
        i if b0 > b1 => mix(i.saturating_sub(1), 7),
        6 => 0.0,
        7 => 1.0,
        i => mix(i.saturating_sub(1), 5),
    }
}

/// a UYVY or YUY2 pixel as D3DX reads it, (r, g, b, a): an even pixel its pair's U and V, an
/// odd one theirs between its pair and the next by the taps (-1, 9, 9, -1) / 16 over the
/// pairs before and after (the row's ends repeated), rounded; BT.601's studio range
fn yuv_d3dx(f: u32, src: &[u8], w: usize, i: usize) -> [f32; 4] {
    let (row, x) = (i / w, i % w);
    let pairs = w.div_ceil(2);
    // (UYVY: U Y0 V Y1; YUY2: Y0 U Y1 V)
    let uyvy = f == super::d3d9draw::fourcc(b"UYVY");
    let pair = |q: isize| {
        let q = q.clamp(0, pairs as isize - 1) as usize;
        let at = (row * pairs + q) * 4;
        let b = src.get(at..at + 4).unwrap_or(&[0; 4]);
        if uyvy { ([b[1], b[3]], b[0], b[2]) } else { ([b[0], b[2]], b[1], b[3]) }
    };
    let p = (x / 2) as isize;
    let (ys, u0, v0) = pair(p);
    let y = ys[x % 2];
    // (an odd pixel's chroma from the four nearest pairs, -1 9 9 -1 / 16 rounded in unsigned 32
    // bit arithmetic, not clamped: 256 seen, and a negative sum wraps to a huge one, the load-YUY2
    // magenta; a source row's ends repeat)
    let (u, v) = if x % 2 == 1 {
        let tap = |k: fn(([u8; 2], u8, u8)) -> u8| chroma_tap(k(pair(p - 1)), k(pair(p)), k(pair(p + 1)), k(pair(p + 2)));
        (tap(|t| t.1), tap(|t| t.2))
    } else {
        (i64::from(u0), i64::from(v0))
    };
    yuv_rgb(y, u, v)
}

/// an odd pixel's chroma from the pairs' a, b, c, d around it (D3DX9.LOAD.YUV):
/// (-a + 9 b + 9 c - d + 8) >> 4 in unsigned 32-bit arithmetic, neither clamped nor saturated
pub fn chroma_tap(a: u8, b: u8, c: u8, d: u8) -> i64 {
    let s = u32::from(b).wrapping_add(u32::from(c)).wrapping_mul(9).wrapping_add(8);
    i64::from(s.wrapping_sub(u32::from(a).wrapping_add(u32::from(d))) >> 4)
}

/// a pixel's Y, U, V as (r, g, b, a) (D3DX9.LOAD.YUV): studio range BT.601 with D3DX's six digit
/// constants, in f32, each clamped to 0..1, alpha 1
pub fn yuv_rgb(y: u8, u: i64, v: i64) -> [f32; 4] {
    let c = f32::from(i16::from(y).wrapping_sub(16)) * 0.00456621f32;
    let (d, e) = (u.wrapping_sub(128) as f32, v.wrapping_sub(128) as f32);
    let cl = |x: f32| x.clamp(0.0, 1.0);
    [cl(c + e * 0.00625893), cl((c - d * 0.00153632) - e * 0.00318811), cl(c + d * 0.00791071), 1.0]
}

/// a signed (bump) channel: U, V, W, Q
fn is_bump(c: u8) -> bool {
    matches!(c, b'u' | b'v' | b'w' | b'q')
}

/// an n-bit unsigned channel as D3DX reads it (D3DX9.LOAD.CONVERT): v times the f32 nearest
/// 1 / (2^n - 1), in f32 (n 1..=16)
pub fn unorm_read(raw: u64, bits: u32) -> f32 {
    raw as f32 * (1.0 / ((1u64 << bits) - 1) as f32)
}

/// an n-bit signed channel as D3DX reads it (D3DX9.LOAD.SIGNED): its two's complement times
/// the f32 nearest 1 / (2^(n-1) - 1), in f32, at least -1 (n 2..=16)
pub fn snorm_read(raw: u64, bits: u32) -> f32 {
    let half = 1i64 << (bits - 1);
    let sv = raw as i64 - if raw as i64 >= half { 2 * half } else { 0 };
    (sv as f32 * (1.0 / (half - 1) as f32)).max(-1.0)
}

/// a value into an n-bit unsigned channel (D3DX9.LOAD.CONVERT): clamped to 0..1 (NaN 1) and
/// times 2^n - 1; on x86 at more than f32's precision rounded to nearest, on x64 the f32
/// product plus 1/2 truncated
pub fn unorm_write(v: f32, bits: u32, x64: bool) -> u64 {
    use super::d3d9draw::round_even;
    let v = if v.is_nan() { 1.0 } else { v.clamp(0.0, 1.0) };
    let m = ((1u64 << bits) - 1) as f32;
    if x64 { (v * m + 0.5) as u64 } else { round_even(v as f64 * m as f64) as u64 }
}

/// a value into an n-bit signed channel (D3DX9.LOAD.SIGNED, MEASURED load-*-signed): clamped
/// to -1..1 (a NaN 1, as into unorm channels: load-R32F-signed), times 2^(n-1) - 1 plus 1/2
/// truncated toward 0, the channel's two's complement
pub fn snorm_write(v: f32, bits: u32) -> u64 {
    let half = 1i64 << (bits - 1);
    let v = if v.is_nan() { 1.0 } else { v.clamp(-1.0, 1.0) };
    let q = (v * (half - 1) as f32 + 0.5) as i64;
    (q & (2 * half - 1)) as u64
}

/// luminance from (r, g, b) (D3DX9.LOAD.CONVERT): Rec. 709's 0.2125 r + 0.7154 g + 0.0721 b in
/// f32, left to right
pub fn luminance(r: f32, g: f32, b: f32) -> f32 {
    0.2125 * r + 0.7154 * g + 0.0721 * b
}

/// (r, g, b, a) into a pixel as D3DX writes it: unorm channels rounded to nearest, X bits 0,
/// luminance Rec. 709's 0.2125 r + 0.7154 g + 0.0721 b, halves rounded to nearest (MEASURED:
/// d3dx9load)
fn load_encode(f: &super::d3d9draw::Fmt, src: super::d3d9draw::Kind, px: [Sample; 4], (at, row): (usize, usize)) -> Res<Vec<u8>> {
    use super::d3d9draw::Kind;
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
                (Sample::Unsigned(x), Kind::Int | Kind::Dxt(_) | Kind::Yuv) => 2.0 * x - 1.0,
                (Sample::Unsigned(x), _) | (Sample::Signed(x), _) => x,
            };
            w |= u128::from(snorm_write(v, ch.bits)) << ch.shift;
            continue;
        }
        // (into unorm channels each clamped to 0..1 first, luminance's too, a NaN to 1)
        let k = |i: usize| if f.kind != Kind::Int { c[i] } else if c[i].is_nan() { 1.0 } else { c[i].clamp(0.0, 1.0) };
        let v = match ch.c {
            b'r' => k(0),
            b'g' => k(1),
            b'b' => k(2),
            b'a' => k(3),
            b'l' => luminance(k(0), k(1), k(2)),
            _ => continue,
        };
        let raw = match f.kind {
            Kind::F16 => u64::from(f16_load(v.to_bits(), f.ch.len(), ci + f.ch.len() * at, f.ch.len() * row)?),
            Kind::F32 => v.to_bits() as u64,
            _ => unorm_write(v, ch.bits, crate::model::arch() == 64),
        };
        w |= (raw as u128) << ch.shift;
    }
    Ok(w.to_le_bytes()[..f.bytes].to_vec())
}

/// a float as D3DXLoadSurfaceFromMemory writes it into a half channel, the channel k of n of
/// the row's pixels (at = n x + k of len = n x width) (MEASURED d3dx9load load-R32F, -G32R32F,
/// -A32B32G32R32F, -A16B16G16R16F): four halves a pixel as the row's array through
/// D3DXFloat32To16Array; one or two a pixel as an array of the pixel's own, so as an array's
/// rest (on x64 f16_x64_tail)
fn f16_load(x: u32, n: usize, at: usize, len: usize) -> Res<u16> {
    // (the clean-room notes: A16B16G16R16F's codec converts a row in one D3DXFloat32To16Array
    // call, R16F's and G16R16F's a pixel at a time; MEASURED load-A32B32G32R32F)
    hit(clause::D3DX9_LOAD_F16RANGE);
    let conv = F16Array::of_executor()?;
    // (the value's place in the array it goes through: the row's, or a lone one's)
    let (at, len) = if n == 4 { (at, len) } else { (0, 1) };
    let bulk = len - len % if conv == F16Array::Amd64 { 4 } else { 8 };
    Ok(conv.half(x, at < bulk))
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
                    return Err(Fail::NoInput(crate::model::Input::Cpu));
                };
                out.push(c);
            }
            "x9f16" => {
                hit(clause::D3DX9_F16);
                let w = words(op, 1)?;
                // (MEASURED, d3dx9 f16-*, 3688 values: x86 does the array 8 at a time and then
                // the rest one by one; x64 4 at a time and then the rest, conversions that differ,
                // or on GenuineIntel as x86 does: golden 465fd71)
                out.push(format!("f16{}", F16Array::of_executor()?.convert(&w).iter().map(|h| format!(" {h:04x}")).collect::<String>()));
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
            "x9load" => load(op, out, Dest::Plain)?,
            "x9loadtex" => load(op, out, Dest::Texture)?,
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

/// a float as a half the way D3DXFloat32To16Array does most of an array, 8 at a time: x86's
/// intelsse2 routine (every SSE2 processor without 3DNow, Intel's and AMD's alike) and x64's
/// intelsse3 one (GenuineIntel) are the same rule (the clean-room notes of both builds; MEASURED
/// d3dx9-32 f16-*, 56/56 lines, f16-ties' 0x33000401 -> 0001 where a shift gives 0000): IEEE,
/// ties toward zero, 0x7c00 past 65520 and for infinities and NaNs; below 2^-14 the magnitude plus 2^-14 in f32 (rounding to 2^-37), its
/// bits less 2^23; the bits plus 0xc8000fff shifted down 13, at most 0x7c00
pub fn f16_sse(x: u32) -> u16 {
    let a = f32::from_bits(x & 0x7fff_ffff);
    let tiny = a < f32::from_bits(0x3880_0000);
    let b = if tiny { (a + f32::from_bits(0x3880_0000)).to_bits() - 0x80_0000 } else { a.to_bits() };
    let r = b.wrapping_add(0xc800_0fff) >> 13;
    low16(x >> 16 & 0x8000 | r.min(0x7c00))
}

/// the low 16 bits
fn low16(v: u32) -> u16 {
    let [a, b, _, _] = v.to_le_bytes();
    u16::from_le_bytes([a, b])
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

/// the D3DXFloat32To16Array the executor's D3DX runs (D3DX9.F16.CPU): x86's; on x64 D3DX's
/// fast table takes intelsse3's where cpuid's vendor is GenuineIntel, amdx64's otherwise (the
/// clean-room notes; MEASURED golden d3dx9 of 4abddab, AuthenticAMD as amdx64's)
#[derive(Clone, Copy, PartialEq, Eq)]
enum F16Array {
    /// 8 at a time as f16_sse (intelsse2_D3DXFloat32To16Array), the rest one by one as
    /// f16_one (c_D3DXFloat32To16Array, called for the last n % 8 even when none are left)
    X86,
    /// 8 at a time as f16_sse, the rest one by one as f16_one (c_D3DXFloat32To16Array)
    IntelSse3,
    /// 4 at a time as f16_x64, the rest one by one as f16_x64_tail
    Amd64,
}

impl F16Array {
    fn of_executor() -> Res<Self> {
        if crate::model::arch() != 64 {
            return Ok(Self::X86);
        }
        hit(clause::D3DX9_F16_CPU);
        let c = crate::model::cpu().ok_or(Fail::NoInput(crate::model::Input::Cpu))?;
        Ok(if c.split(' ').nth(1).ok_or(Fail::Bad)? == "GenuineIntel" { Self::IntelSse3 } else { Self::Amd64 })
    }

    /// an array of floats as halves
    fn convert(self, w: &[u32]) -> Vec<u16> {
        let step = if self == Self::Amd64 { 4 } else { 8 };
        let bulk = w.len() - w.len() % step;
        w.iter().enumerate().map(|(i, &x)| self.half(x, i < bulk)).collect()
    }

    /// one value as a half, in the array's blocks (`bulk`) or its rest: the one place that
    /// picks a routine (D3DXFloat32To16Array and the load codecs both come here)
    fn half(self, x: u32, bulk: bool) -> u16 {
        match (self, bulk) {
            (Self::X86 | Self::IntelSse3, true) => f16_sse(x),
            (Self::X86 | Self::IntelSse3, false) => f16_one(x),
            (Self::Amd64, true) => f16_x64(x),
            (Self::Amd64, false) => f16_x64_tail(x),
        }
    }
}

/// x64's D3DXFloat32To16Array past its last 4 (the clean-room notes): clamped to +-131008
/// (a NaN to -131008), rounded to a half's 11 bits by adding and taking off 8192 times
/// itself, times 2^-112, the bits shifted down 13 (a denormal truncated)
pub fn f16_x64_tail(x: u32) -> u16 {
    let v = f32::from_bits(x);
    let (lo, hi) = (-131008f32, 131008f32);
    let m = if v > hi { hi } else { v };
    let v = if !(m > lo) { lo } else if v > hi { hi } else { v };
    let c = v * 8192.0;
    let t = ((c + v) - c) * f32::from_bits(0x0780_0000);
    let b = t.to_bits();
    low16(v.to_bits() >> 16 & 0x8000 | b >> 13 & 0x7fff)
}

/// a float as a half the way x64's amdx64 D3DXFloat32To16Array does 4 at a time: exponent 31
/// a number, ties to even, 0x7fff past 131008 and for infinities, a NaN's mantissa rounded
/// with the quiet bit set (MEASURED: d3dx9 f16-*; the clean-room notes: clamped by minps and
/// maxps, times 2^-112, rounded at bit 13)
pub fn f16_x64(x: u32) -> u16 {
    let (lo, hi) = (-131008f32, 131008f32);
    // (minps and maxps: a NaN goes through)
    let v = f32::from_bits(x);
    let v = if hi < v { hi } else { v };
    let v = if lo > v { lo } else { v };
    let t = (v * f32::from_bits(0x0780_0000)).to_bits();
    let r = t.wrapping_add(t >> 13 & 1).wrapping_add(0xfff) >> 13 & 0x7fff;
    low16(v.to_bits() >> 16 & 0x8000 | r)
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
