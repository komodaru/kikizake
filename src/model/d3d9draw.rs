//! Direct3D 9 and 8 drawing (runner/d3d9draw.c): the tables the generators and the model share.

/// render target formats asked (D3DFORMAT, name)
pub const TARGET_FORMATS: &[(u32, &str)] = &[
    (20, "R8G8B8"), (21, "A8R8G8B8"), (22, "X8R8G8B8"), (23, "R5G6B5"), (24, "X1R5G5B5"), (25, "A1R5G5B5"), (26, "A4R4G4B4"), (27, "R3G3B2"), (28, "A8"), (29, "A8R3G3B2"),
    (30, "X4R4G4B4"), (31, "A2B10G10R10"), (32, "A8B8G8R8"), (33, "X8B8G8R8"), (34, "G16R16"), (35, "A2R10G10B10"), (36, "A16B16G16R16"), (50, "L8"), (51, "A8L8"), (60, "V8U8"),
    (81, "L16"), (111, "R16F"), (112, "G16R16F"), (113, "A16B16G16R16F"), (114, "R32F"), (115, "G32R32F"), (116, "A32B32G32R32F"),
];

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    s[0] as u32 | (s[1] as u32) << 8 | (s[2] as u32) << 16 | (s[3] as u32) << 24
}

/// texture formats asked: the target formats and those only textures have
pub const TEXTURE_FORMATS: &[(u32, &str)] = &[
    (20, "R8G8B8"), (21, "A8R8G8B8"), (22, "X8R8G8B8"), (23, "R5G6B5"), (24, "X1R5G5B5"), (25, "A1R5G5B5"), (26, "A4R4G4B4"), (27, "R3G3B2"), (28, "A8"), (29, "A8R3G3B2"),
    (30, "X4R4G4B4"), (31, "A2B10G10R10"), (32, "A8B8G8R8"), (33, "X8B8G8R8"), (34, "G16R16"), (35, "A2R10G10B10"), (36, "A16B16G16R16"), (40, "A8P8"), (41, "P8"), (50, "L8"),
    (51, "A8L8"), (52, "A4L4"), (60, "V8U8"), (61, "L6V5U5"), (62, "X8L8V8U8"), (63, "Q8W8V8U8"), (64, "V16U16"), (67, "A2W10V10U10"), (81, "L16"), (110, "Q16W16V16U16"),
    (111, "R16F"), (112, "G16R16F"), (113, "A16B16G16R16F"), (114, "R32F"), (115, "G32R32F"), (116, "A32B32G32R32F"), (117, "CxV8U8"),
    (fourcc(b"DXT1"), "DXT1"), (fourcc(b"DXT2"), "DXT2"), (fourcc(b"DXT3"), "DXT3"), (fourcc(b"DXT4"), "DXT4"), (fourcc(b"DXT5"), "DXT5"), (fourcc(b"UYVY"), "UYVY"), (fourcc(b"YUY2"), "YUY2"),
];

/// D3DTEXTUREOP
pub const TOPS: &[(u32, &str)] = &[
    (1, "DISABLE"), (2, "SELECTARG1"), (3, "SELECTARG2"), (4, "MODULATE"), (5, "MODULATE2X"), (6, "MODULATE4X"), (7, "ADD"), (8, "ADDSIGNED"), (9, "ADDSIGNED2X"), (10, "SUBTRACT"),
    (11, "ADDSMOOTH"), (12, "BLENDDIFFUSEALPHA"), (13, "BLENDTEXTUREALPHA"), (14, "BLENDFACTORALPHA"), (15, "BLENDTEXTUREALPHAPM"), (16, "BLENDCURRENTALPHA"), (17, "PREMODULATE"),
    (18, "MODULATEALPHA_ADDCOLOR"), (19, "MODULATECOLOR_ADDALPHA"), (20, "MODULATEINVALPHA_ADDCOLOR"), (21, "MODULATEINVCOLOR_ADDALPHA"), (22, "BUMPENVMAP"), (23, "BUMPENVMAPLUMINANCE"),
    (24, "DOTPRODUCT3"), (25, "MULTIPLYADD"), (26, "LERP"),
];

/// D3DBLENDOP
pub const BLEND_OPS: &[(u32, &str)] = &[(1, "ADD"), (2, "SUBTRACT"), (3, "REVSUBTRACT"), (4, "MIN"), (5, "MAX")];
/// D3DCMPFUNC
pub const CMP: &[(u32, &str)] = &[(1, "NEVER"), (2, "LESS"), (3, "EQUAL"), (4, "LESSEQUAL"), (5, "GREATER"), (6, "NOTEQUAL"), (7, "GREATEREQUAL"), (8, "ALWAYS")];
/// D3DSTENCILOP
pub const STENCIL_OPS: &[(u32, &str)] = &[(1, "KEEP"), (2, "ZERO"), (3, "REPLACE"), (4, "INCRSAT"), (5, "DECRSAT"), (6, "INVERT"), (7, "INCR"), (8, "DECR")];

/// D3DDECLTYPE's size in bytes (D3DDECLTYPE): None past UNUSED
pub fn decl_type_size(t: u32) -> Option<u32> {
    Some(match t {
        0 | 4 | 5 | 6 | 8 | 9 | 11 | 13 | 14 | 15 => 4,
        1 | 7 | 10 | 12 | 16 => 8,
        2 => 12,
        3 => 16,
        17 => 0,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// The model: what the drawing lines print, from the clauses in spec/d3d9draw.txt.

use crate::spec::{clause, open};
use super::d3d9::{D3d9, Key};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const INVALIDCALL: u32 = 0x8876_086c;

/// one channel of a pixel format: its letter (r g b a l x p; u v w q are signed), lowest bit, bits
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ch {
    pub c: u8,
    pub shift: u32,
    pub bits: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Int,
    F16,
    F32,
    /// DXT1..5
    Dxt(u32),
    Yuv,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fmt {
    pub bytes: usize,
    pub kind: Kind,
    pub ch: Vec<Ch>,
}

/// how a texture's texels are read: by their channels (D3D9.TEX.DECODE, TEX.MISSING), as DXT
/// blocks (D3D9.TEX.DXT) or through the palette (D3D9.TEX.PALETTE)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TexelKind {
    Channels,
    Dxt(u32),
    Palette,
}

/// the way a texture of format f (of layout fm) is sampled; none for a format of no such way (the
/// YUV ones, which WARP does not take: law every_taken_format_is_decoded, D3D9.TEX.FORMAT)
pub fn texel_kind(f: u32, fm: &Fmt) -> Option<TexelKind> {
    hit(clause::D3D9_TEX_FORMAT);
    let p = fm.ch.iter().any(|c| c.c == b'p');
    match fm.kind {
        Kind::Int | Kind::F16 | Kind::F32 if !p => Some(TexelKind::Channels),
        Kind::Dxt(n) => Some(TexelKind::Dxt(n)),
        Kind::Int if matches!(f, 40 | 41) => Some(TexelKind::Palette),
        Kind::Int | Kind::F16 | Kind::F32 | Kind::Yuv => None,
    }
}

/// a format's layout (D3DFORMAT: channels named from the most significant bit down, so A8R8G8B8
/// keeps blue in bits 0..7); None for formats the model does not know
pub fn format(f: u32) -> Option<Fmt> {
    let (kind, layout) = match f {
        20 => (Kind::Int, "b8g8r8"),
        21 => (Kind::Int, "b8g8r8a8"),
        22 => (Kind::Int, "b8g8r8x8"),
        23 => (Kind::Int, "b5g6r5"),
        24 => (Kind::Int, "b5g5r5x1"),
        25 => (Kind::Int, "b5g5r5a1"),
        26 => (Kind::Int, "b4g4r4a4"),
        27 => (Kind::Int, "b2g3r3"),
        28 => (Kind::Int, "a8"),
        29 => (Kind::Int, "b2g3r3a8"),
        30 => (Kind::Int, "b4g4r4x4"),
        31 => (Kind::Int, "r10g10b10a2"),
        32 => (Kind::Int, "r8g8b8a8"),
        33 => (Kind::Int, "r8g8b8x8"),
        34 => (Kind::Int, "r16g16"),
        35 => (Kind::Int, "b10g10r10a2"),
        36 => (Kind::Int, "r16g16b16a16"),
        40 => (Kind::Int, "p8a8"),
        41 => (Kind::Int, "p8"),
        50 => (Kind::Int, "l8"),
        51 => (Kind::Int, "l8a8"),
        52 => (Kind::Int, "l4a4"),
        60 | 117 => (Kind::Int, "u8v8"),
        61 => (Kind::Int, "u5v5l6"),
        62 => (Kind::Int, "u8v8l8x8"),
        63 => (Kind::Int, "u8v8w8q8"),
        64 => (Kind::Int, "u16v16"),
        67 => (Kind::Int, "u10v10w10a2"),
        81 => (Kind::Int, "l16"),
        110 => (Kind::Int, "u16v16w16q16"),
        111 => (Kind::F16, "r16"),
        112 => (Kind::F16, "r16g16"),
        113 => (Kind::F16, "r16g16b16a16"),
        114 => (Kind::F32, "r32"),
        115 => (Kind::F32, "r32g32"),
        116 => (Kind::F32, "r32g32b32a32"),
        _ if f == fourcc(b"DXT1") => return Some(Fmt { bytes: 8, kind: Kind::Dxt(1), ch: vec![] }),
        _ if (fourcc(b"DXT2")..=fourcc(b"DXT5")).contains(&f) && f & 0xff_ffff == fourcc(b"DXT1") & 0xff_ffff => {
            return Some(Fmt { bytes: 16, kind: Kind::Dxt((f >> 24) - b'0' as u32), ch: vec![] })
        }
        _ if f == fourcc(b"UYVY") || f == fourcc(b"YUY2") => return Some(Fmt { bytes: 2, kind: Kind::Yuv, ch: vec![] }),
        _ => return None,
    };
    let mut ch = Vec::new();
    let mut shift = 0;
    let b = layout.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        let bits: u32 = layout.get(i + 1..j)?.parse().ok()?;
        ch.push(Ch { c, shift, bits });
        shift += bits;
        i = j;
    }
    Some(Fmt { bytes: shift as usize / 8, kind, ch })
}

fn signed(c: u8) -> bool {
    matches!(c, b'u' | b'v' | b'w' | b'q')
}

/// the channels of one pixel's bytes as numbers: unsigned normalized v / (2^n - 1), signed
/// normalized v / (2^(n-1) - 1) (the most negative value -1 too), floats as they are
pub fn decode(f: &Fmt, p: &[u8]) -> Vec<(u8, f64)> {
    let mut word = [0u8; 16];
    word[..p.len()].copy_from_slice(p);
    let w = u128::from_le_bytes(word);
    f.ch.iter()
        .map(|c| {
            let raw = (w >> c.shift) as u64 & ((1u64 << c.bits) - 1);
            let v = match f.kind {
                // (MEASURED: a NaN is read as 0: D3D9.TEX.DECODE)
                Kind::F16 => nan0(f16_to_f64(raw as u16)),
                Kind::F32 => nan0(f32::from_bits(raw as u32) as f64),
                _ if signed(c.c) => {
                    // (MEASURED: v times the f32 nearest 1 / (2^(n-1) - 1), in f32)
                    let m = ((1i64 << (c.bits - 1)) - 1) as f32;
                    let s = if raw >> (c.bits - 1) != 0 { raw as i64 - (1i64 << c.bits) } else { raw as i64 };
                    ((s as f32 * (1.0 / m)) as f64).max(-1.0)
                }
                _ => raw as f64 / ((1u64 << c.bits) - 1) as f64,
            };
            (c.c, v)
        })
        .collect()
}

fn nan0(v: f64) -> f64 {
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

/// IEEE binary16
pub fn f16_to_f64(h: u16) -> f64 {
    let (s, e, m) = (h >> 15, (h >> 10 & 31) as i32, (h & 0x3ff) as f64);
    let v = match e {
        0 => m * 2f64.powi(-24),
        31 if m == 0.0 => f64::INFINITY,
        31 => f64::NAN,
        _ => (1.0 + m / 1024.0) * 2f64.powi(e - 15),
    };
    if s != 0 {
        -v
    } else {
        v
    }
}

/// f32 to binary16 rounded toward zero (MEASURED: WARP writes half float targets so, D3D9.RT.F16)
pub fn f32_to_f16_rtz(x: f32) -> u16 {
    hit(clause::D3D9_RT_F16);
    let h = f32_to_f16(x);
    // (the nearest half, one step back toward zero when it lies beyond x)
    if x.is_finite() && f16_to_f64(h).abs() > (x as f64).abs() {
        h - 1
    } else {
        h
    }
}

/// f32 to IEEE binary16, round to nearest even
pub fn f32_to_f16(x: f32) -> u16 {
    let b = x.to_bits();
    let s = (b >> 16 & 0x8000) as u16;
    let e = (b >> 23 & 255) as i32;
    let m = b & 0x7f_ffff;
    if e == 255 {
        return s | 0x7c00 | if m != 0 { 0x200 } else { 0 };
    }
    let e2 = e - 127 + 15;
    if e2 >= 31 {
        return s | 0x7c00;
    }
    // (the 24-bit significand, shifted so that the half's last bit is bit 0, rounded to even)
    let (sig, sh) = if e2 <= 0 { ((m | 0x80_0000) as u64, (14 - e2) as u32) } else { ((m | 0x80_0000) as u64, 13) };
    if sh > 24 {
        return s;
    }
    let mut v = sig >> sh;
    let rem = sig & ((1 << sh) - 1);
    let half = 1 << (sh - 1);
    if rem > half || (rem == half && v & 1 == 1) {
        v += 1;
    }
    if e2 <= 0 {
        s | v as u16
    } else {
        let r = ((e2 as u32) << 10) + (v as u32 - 0x400);
        if r >= 0x7c00 {
            s | 0x7c00
        } else {
            s | r as u16
        }
    }
}

/// sampled texel (r, g, b, a) of a decoded pixel: the channels the format has, the others as the
/// clauses say (D3D9.TEX.MISSING)
pub fn to_rgba(f: u32, chans: &[(u8, f64)]) -> [f64; 4] {
    let mut c = [None::<f64>; 4];
    let mut l = None;
    for &(n, v) in chans {
        match n {
            b'r' | b'u' => c[0] = Some(v),
            b'g' | b'v' => c[1] = Some(v),
            b'b' | b'w' => c[2] = Some(v),
            b'a' | b'q' => c[3] = Some(v),
            b'l' => l = Some(v),
            _ => {}
        }
    }
    if let Some(l) = l {
        // (luminance: in the color channels, after the bump channels where both are there)
        for k in 0..3 {
            if c[k].is_none() {
                c[k] = Some(l);
            }
        }
    }
    let _ = f;
    hit(clause::D3D9_TEX_MISSING);
    let miss_rgb = if c.iter().take(3).all(Option::is_none) { 0.0 } else { 1.0 };
    [c[0].unwrap_or(miss_rgb), c[1].unwrap_or(miss_rgb), c[2].unwrap_or(miss_rgb), c[3].unwrap_or(1.0)]
}

/// rgba -> one pixel's bytes of a render target format
pub fn encode(f: &Fmt, rgba: [f64; 4]) -> Vec<u8> {
    encode_x(f, rgba, true)
}

/// a pixel's bytes, the unused X bits the alpha (Clear) or 0 (a draw: MEASURED, d3d9draw
/// blend-fmt-X8R8G8B8-*)
pub fn encode_x(f: &Fmt, rgba: [f64; 4], x_alpha: bool) -> Vec<u8> {
    let mut raws = Vec::with_capacity(f.ch.len());
    for c in &f.ch {
        let v = match c.c {
            b'r' | b'l' | b'u' => rgba[0],
            b'g' | b'v' => rgba[1],
            b'b' | b'w' => rgba[2],
            // (MEASURED: the unused X bits take the alpha, as an A channel would: D3D9.RT.UNORM)
            _ => rgba[3],
        };
        let raw: u64 = match f.kind {
            Kind::F16 => f32_to_f16_rtz(v as f32) as u64,
            Kind::F32 => (v as f32).to_bits() as u64,
            _ if signed(c.c) => {
                let m = ((1i64 << (c.bits - 1)) - 1) as f64;
                let q = round_even(v.clamp(-1.0, 1.0) * m) as i64;
                (q as u64) & ((1 << c.bits) - 1)
            }
            _ => unorm(v, c.bits),
        };
        raws.push(raw);
    }
    pack(&f.ch, &raws, x_alpha).to_le_bytes()[..f.bytes].to_vec()
}

/// a pixel's channels' raw values put together at their shifts; the unused X bits 0 unless
/// `x_alpha` (D3D9.DRAW.XZERO: a draw writes 0 there, Clear the alpha, D3D9.RT.UNORM)
pub fn pack(ch: &[Ch], raws: &[u64], x_alpha: bool) -> u128 {
    ch.iter().zip(raws).filter(|(c, _)| c.c != b'x' || x_alpha).fold(0, |w, (c, &r)| w | u128::from(r) << c.shift)
}

/// D3D9.RASTER.CLIP, RASTER.TOPLEFT: a screen coordinate snapped to 1/256 pixel, to nearest even
/// (the value in 1/256ths)
pub fn snap256(v: f32) -> i64 {
    round_even(f64::from(v) * 256.0) as i64
}

pub fn round_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 {
        r - x.signum()
    } else {
        r
    }
}

/// one texel of a DXT1..DXT5 texture as rgba: its block's texel (dxt_block_texel)
fn dxt_texel(n: u32, bytes: &[u8], w: i32, x: i32, y: i32) -> Res<[f64; 4]> {
    let bsz = if n == 1 { 8 } else { 16 };
    let bi = ((y / 4) * ((w + 3) / 4) + x / 4) as usize * bsz;
    let t = u32::try_from((y % 4) * 4 + x % 4).ok().ok_or(Fail::Bad)?;
    let c = dxt_block_texel(n, bytes.get(bi..bi + bsz).ok_or(Fail::Bad)?, t).ok_or(Fail::Bad)?;
    Ok(c.map(|v| f64::from(v) / 255.0))
}

/// a 5-bit (6-bit) endpoint channel widened to 8 bits by repeating its top bits
pub fn dxt_widen(c: u32, bits: u32) -> u32 {
    if bits == 5 { c << 3 | c >> 2 } else { c << 2 | c >> 4 }
}

/// a DXT color of the endpoint channels a and b by its index: a, b, then the thirds as 171/256
/// and 85/256 taken down (four colors), or the half taken down and 0 (DXT1's three)
pub fn dxt_mix(idx: u32, four: bool, a: u32, b: u32) -> u32 {
    match (idx, four) {
        (0, _) => a,
        (1, _) => b,
        (2, true) => (171 * a + 85 * b) >> 8,
        (3, true) => (85 * a + 171 * b) >> 8,
        (2, false) => (a + b) >> 1,
        _ => 0,
    }
}

/// a DXT4/5 alpha of the endpoints a0, a1 by its 3-bit index: sevenths (a0 > a1) or fifths
/// and 0, 255, each rounded to nearest
pub fn dxt_alpha(ai: u32, a0: u32, a1: u32) -> u32 {
    let mix = |wa: u32, wb: u32| (wa * a0 + wb * a1 + (wa + wb) / 2) / (wa + wb);
    match ai {
        0 => a0,
        1 => a1,
        i if a0 > a1 => mix(8 - i, i - 1),
        6 => 0,
        7 => 255,
        i => mix(6 - i, i - 1),
    }
}

/// texel t (0..15, row by row) of a DXT1..DXT5 block (8 bytes for DXT1, else 16: the alpha
/// block, then the color one) as 8-bit r, g, b, a (MEASURED tex-*-DXT*, D3D9.TEX.DXT);
/// spec/lean/Kikizake/D3d9/Texel.lean `texel`
pub fn dxt_block_texel(n: u32, b: &[u8], t: u32) -> Option<[u32; 4]> {
    hit(clause::D3D9_TEX_DXT);
    if t > 15 {
        return None;
    }
    let cb = b.get(if n == 1 { 0..8 } else { 8..16 })?;
    let word = |i: usize| -> Option<u32> { Some(u32::from(u16::from_le_bytes(*cb.get(i..)?.first_chunk()?))) };
    let (c0, c1) = (word(0)?, word(2)?);
    let idx = u32::from_le_bytes(*cb.get(4..)?.first_chunk()?) >> (2 * t) & 3;
    let ex = |c: u32| [dxt_widen(c >> 11 & 31, 5), dxt_widen(c >> 5 & 63, 6), dxt_widen(c & 31, 5)];
    let (e0, e1) = (ex(c0), ex(c1));
    let four = n != 1 || c0 > c1;
    let [r, g, bl] = [0, 1, 2].map(|k| dxt_mix(idx, four, e0[k], e1[k]));
    let alpha = if n == 1 {
        if !four && idx == 3 { 0 } else { 255 }
    } else if n == 2 || n == 3 {
        u32::try_from(u64::from_le_bytes(*b.first_chunk()?) >> (4 * t) & 15).ok()? * 17
    } else {
        let (a0, a1) = (u32::from(*b.first()?), u32::from(*b.get(1)?));
        let bits = u64::from_le_bytes([*b.get(2)?, *b.get(3)?, *b.get(4)?, *b.get(5)?, *b.get(6)?, *b.get(7)?, 0, 0]);
        dxt_alpha(u32::try_from(bits >> (3 * t) & 7).ok()?, a0, a1)
    };
    Some([r, g, bl, alpha])
}

/// float -> n-bit unsigned normalized: clamped, scaled, rounded to nearest (D3D9.RT.UNORM)
pub fn unorm(v: f64, bits: u32) -> u64 {
    hit(clause::D3D9_RT_UNORM);
    let m = ((1u64 << bits) - 1) as f64;
    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
    // (MEASURED: the product in f32, rounded to nearest even; depth's 24 bits in f64)
    if bits <= 16 {
        round_even((v as f32 * m as f32) as f64) as u64
    } else {
        round_even(v * m) as u64
    }
}

#[derive(Clone, Debug)]
pub struct Surface {
    pub w: i32,
    pub h: i32,
    pub fmt: u32,
    pub bytes: Vec<u8>,
}

impl Surface {
    fn px(&self, x: i32, y: i32) -> Res<&[u8]> {
        let b = format(self.fmt).ok_or(Fail::Bad)?.bytes;
        let i = (y * self.w + x) as usize * b;
        Ok(&self.bytes[i..i + b])
    }
    fn set(&mut self, x: i32, y: i32, v: &[u8]) {
        let b = v.len();
        let i = (y * self.w + x) as usize * b;
        self.bytes[i..i + b].copy_from_slice(v);
    }
}

/// D3DBLENDOP
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlendOp {
    Add,
    Sub,
    RevSub,
    Min,
    Max,
}

/// the call that set the vertex declaration
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DeclFrom {
    /// SetFVF, or none set yet
    #[default]
    Fvf,
    /// SetVertexDeclaration
    Declaration,
}

/// D3D9.FVF.SET, FVF.GET, DECL.GET: SetFVF(f) as GetFVF and GetVertexDeclaration then answer
/// (fvf, declaration, its origin), from what they answered before. MEASURED fvf-decl-order-*:
/// an FVF of 0 changes nothing with no declaration or one SetVertexDeclaration set, and else sets
/// a declaration of no elements; fvf-*: any other FVF its runtime declaration
pub fn set_fvf(f: u32, now: (u32, Option<Vec<(u32, u32)>>, DeclFrom)) -> Res<(u32, Option<Vec<(u32, u32)>>, DeclFrom)> {
    hit(clause::D3D9_FVF_SET);
    Ok(match (f, now) {
        (0, (fvf, None, from)) => (fvf, None, from),
        (0, (fvf, Some(d), DeclFrom::Declaration)) => (fvf, Some(d), DeclFrom::Declaration),
        (0, (_, Some(_), DeclFrom::Fvf)) => (0, Some(Vec::new()), DeclFrom::Fvf),
        (f, _) => (f, fvf_runtime_decl(f)?, DeclFrom::Fvf),
    })
}

/// D3D9.FVF.GET, DECL.GET: SetVertexDeclaration of a declaration CreateVertexDeclaration took:
/// GetFVF then answers the FVF it converts to, 0 when none (MEASURED fvf-decl-order-*, decl-*)
pub fn set_decl(e: Vec<(u32, u32)>) -> (u32, Option<Vec<(u32, u32)>>, DeclFrom) {
    hit(clause::D3D9_FVF_GET);
    (super::d3dx9::decl_fvf(&e).unwrap_or(0), Some(e), DeclFrom::Declaration)
}

#[derive(Clone, Debug, Default)]
pub struct Draw {
    pub rt: Option<Surface>,
    /// the depth (24 bits) and stencil of each pixel
    pub depth: Vec<u32>,
    pub stencil: Vec<u8>,
    pub texs: [Option<Surface>; 8],
    pub stage_tex: [Option<usize>; 8],
    pub fvf: u32,
    /// the vertex declaration GetVertexDeclaration answers (None: no declaration)
    pub decl: Option<Vec<(u32, u32)>>,
    /// which call set the declaration (D3D9.FVF.SET: SetFVF(0) keeps one SetVertexDeclaration set)
    pub decl_from: DeclFrom,
    pub scissor: Option<(i32, i32, i32, i32)>,
    /// the triangle being drawn is counterclockwise on screen (two-sided stencil)
    pub ccw: bool,
    /// the current texture palette's entries (r, g, b, flags), None before d9palette
    pub palette: Option<Box<[[u8; 4]; 256]>>,
    /// SetTransform's matrices by D3DTRANSFORMSTATETYPE (none: the identity)
    pub xform: std::collections::BTreeMap<u32, [f32; 16]>,
    /// SetViewport's (x, y, width, height, MinZ, MaxZ); None: the whole target, depth 0..1
    pub viewport: Option<(i32, i32, i32, i32, f32, f32)>,
    /// drawing a piece of an untransformed triangle clipped in clip space
    pub in_piece: bool,
    /// the drawn triangle's texture coordinate derivatives ((du/dx, du/dy), (dv/dx, dv/dy)): its
    /// footprint in the texture, by which anisotropic filtering differs from linear
    /// the texture coordinates' derivatives, of each set the stages read (TexSet)
    pub duv: [((f64, f64), (f64, f64)); NSET],
    /// the sets of texture coordinates the enabled stages read, stage 0's first (TexSet)
    pub slots: Vec<SetKey>,
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// xorshift32 bytes (SPEC.md), or byte i = i mod 256 for seed -1, for w x h pixels of a format
/// (block rows for DXT)
pub fn fill_bytes(f: &Fmt, w: i32, h: i32, seed: i64) -> Vec<u8> {
    let n = level_bytes(matches!(f.kind, Kind::Dxt(_)), f.bytes, w, h).unwrap_or(0);
    if seed == -1 {
        return (0..n).map(|i| i as u8).collect();
    }
    let mut r = crate::rng::XorShift32::new(seed as u32);
    (0..n).map(|_| r.next() as u8).collect()
}

/// the bytes of a texture's one level (D3D9.TEX.CREATE): its rows of w pixels of bpp bytes, or
/// of a DXT format the rows of 4 x 4 blocks of bpp bytes each, a part block whole;
/// spec/lean/Kikizake/D3d9/Texel.lean `levelBytes`
pub fn level_bytes(dxt: bool, bpp: usize, w: i32, h: i32) -> Option<usize> {
    let (w, h) = (usize::try_from(w).ok()?, usize::try_from(h).ok()?);
    if dxt {
        w.div_ceil(4).checked_mul(bpp)?.checked_mul(h.div_ceil(4))
    } else {
        h.checked_mul(w.checked_mul(bpp)?)
    }
}

/// CreateTexture's level of a format (D3D9.TEX.CREATE): refused (INVALIDCALL) for a format WARP
/// does not take as a texture (taken: supported), else the level's bytes, all 0
pub fn create_level(taken: bool, dxt: bool, bpp: usize, w: i32, h: i32) -> Result<Vec<u8>, u32> {
    hit(clause::D3D9_TEX_CREATE);
    if !taken {
        return Err(INVALIDCALL);
    }
    Ok(vec![0; level_bytes(dxt, bpp, w, h).ok_or(INVALIDCALL)?])
}

/// MEASURED (check case, d3d9draw and d3d8draw goldens, x86 and x86-64 alike): the formats
/// Windows' WARP device takes for textures (CheckDeviceFormat usage 0) and render targets
/// (RENDERTARGET, as a surface or a texture); CreateTexture and CreateRenderTarget agree
pub fn supported(f: u32, rt: bool, v8: bool) -> bool {
    hit(clause::D3D9_FORMATS);
    let no_tex = [20, 27, 29, 33, 52, 61, 62, 67, 117, fourcc(b"UYVY"), fourcc(b"YUY2")];
    // (Direct3D 8 takes A2R10G10B10, A16B16G16R16, L16 as targets but not as textures: d3d8draw
    // tex-*, clear-*)
    if no_tex.contains(&f) || v8 && !rt && matches!(f, 35 | 36 | 81) || !TEXTURE_FORMATS.iter().any(|t| t.0 == f) {
        return false;
    }
    !rt || !(matches!(f, 40 | 41) || format(f).is_some_and(|m| matches!(m.kind, Kind::Dxt(_))))
}

/// CheckDeviceFormat's answer for a format, a usage and a resource type (D3D9.FORMATS.CHECK,
/// FORMATS.USAGE): what CreateTexture / CreateRenderTarget take, Direct3D 8's CheckDeviceFormat
/// too, though its CreateTexture refuses three more (MEASURED, check, check-usage)
pub fn check_hr(f: u32, u: i64, rtype: i64, v8: bool) -> Res<u32> {
    hit(clause::D3D9_FORMATS_CHECK);
    let tex = supported(f, false, false);
    const NOTAVAILABLE: u32 = 0x8876_086a;
    let ok = |b: bool| if b { 0 } else { NOTAVAILABLE };
    let dxt = format(f).is_some_and(|m| matches!(m.kind, Kind::Dxt(_)));
    Ok(match (u, rtype) {
        // (Direct3D 8, MEASURED d3d8draw check-usage: a plain surface INVALIDCALL; a
        // render target volume as a texture, not the palette nor the DXT formats;
        // A2R10G10B10, A16B16G16R16 and L16 depth stencil too; no AUTOGENMIPMAP)
        (0, 1) | (0x400, 3) if v8 => INVALIDCALL,
        (1, 4) if v8 => ok(tex && !matches!(f, 40 | 41) && !dxt),
        (2, 1) if v8 && matches!(f, 35 | 36 | 81) => 0,
        (0, 3) => ok(tex),
        (1, 1) | (1, 3) => ok(supported(f, true, false)),
        // (DYNAMIC; volume and cube textures: as textures)
        (0x200, 3) | (0, 4) | (0, 5) => ok(tex),
        // (a plain surface: not A8P8 nor the DXT ones, but YUY2)
        (0, 1) => ok(tex && f != 40 && !dxt || f == fourcc(b"YUY2")),
        // (a render target volume: INVALIDCALL)
        (1, 4) => INVALIDCALL,
        // (a depth stencil surface: D16, D24S8, D24X8, D32F_LOCKABLE; other depth
        // formats NOTAVAILABLE, the rest INVALIDCALL)
        (2, 1) => match f {
            75 | 77 | 80 | 82 => 0,
            70 | 71 | 73 | 79 | 83 | 84 => NOTAVAILABLE,
            _ => INVALIDCALL,
        },
        // (AUTOGENMIPMAP: D3DOK_NOAUTOGEN for X8R8G8B8, R5G6B5, the palette, signed
        // and DXT formats)
        (0x400, 3) if tex => {
            if dxt || matches!(f, 22 | 23 | 40 | 41 | 60 | 63 | 64 | 110) { 0x0876_086f } else { 0 }
        }
        (0x400, 3) => NOTAVAILABLE,
        // (the generators ask no other pair)
        _ => return Err(Fail::Bad),
    })
}

/// one vertex as the pipeline sees it: screen position (1/256 pixel), rhw, colors, coordinates
#[derive(Clone, Copy, Debug)]
struct Vtx {
    x: f32,
    y: f32,
    z: f64,
    rhw: f64,
    diffuse: [f64; 4],
    specular: [f64; 4],
    /// the texture coordinates (s, t, q) of the sets the stages read (TexSet)
    uv: [[f64; 3]; NSET],
    /// an untransformed vertex's clip position (D3D9.DRAW.XYZ); None: pretransformed
    clip: Option<[f32; 4]>,
}

/// a D3DCOLOR's channels (r, g, b, a) as f32 c / 255 (MEASURED: so the pipeline reads them)
fn argb(c: u32) -> [f64; 4] {
    [c >> 16 & 255, c >> 8 & 255, c & 255, c >> 24].map(|v| (v as f32 / 255.0) as f64)
}

fn cross(p: [(f64, f64); 3]) -> f64 {
    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0)
}

/// the declaration the runtime makes of an FVF (SetFVF, then GetVertexDeclaration): as
/// D3DXDeclaratorFromFVF's, but XYZW a FLOAT4 POSITION, the reserved bits ignored, 0x2000 a FLOAT1
/// FOG before the texture coordinates; FVF 0 none (MEASURED: d3d9draw fvf-*)
pub fn fvf_runtime_decl(fvf: u32) -> Res<Option<Vec<(u32, u32)>>> {
    use super::d3dx9::{elem, fvf_decl};
    if fvf == 0 {
        return Ok(None);
    }
    let sets = fvf >> 8 & 0xf;
    let xyzw = fvf & 0x400e == 0x4002;
    // (MEASURED fvf-bits: 0x4000 with other position bits than XYZW's is no position at all)
    let base = fvf & !0x6001 & !if fvf & 0x4000 != 0 { 0x400e } else { 0 } | if xyzw { 0x002 } else { 0 };
    // (MEASURED, fvf-texcount: past 8 texture coordinate sets, FLOAT2 ones of the next indices)
    let base = if sets > 8 { base & !0xf00 | 8 << 8 } else { base };
    // (MEASURED, fvf-6, fvf-blend5, fvf-bits: XYZB5 without an index flag, which D3DX refuses, as
    // four weights and the fifth a FLOAT1 BLENDINDICES)
    let b5 = base & 0x400e == 0xe && base & 0x9000 == 0;
    let mut e = fvf_decl(if b5 { base | 0x1000 } else { base }).map_err(|_| Fail::Bad)?;
    if b5 {
        hit(clause::D3D9_FVF_BLEND5);
        e[2].1 &= !255;
    }
    if sets > 8 {
        hit(clause::D3D9_FVF_TEXCOUNT);
        for k in 8..sets {
            let Some(&l) = e.last() else {
                return Err(Fail::Bad);
            };
            e.push(elem((l.0 >> 16) + decl_type_size(l.1 & 255).ok_or(Fail::Bad)?, 1, 5, k));
        }
    }
    if xyzw {
        // (a FLOAT4 in place of the FLOAT3: what follows moves 4 on)
        e[0].1 = e[0].1 & !255 | 3;
        for x in e.iter_mut().skip(1) {
            x.0 += 4 << 16;
        }
    }
    if fvf & 0x2000 != 0 {
        let at = e.iter().take_while(|x| x.1 >> 16 & 255 != 5).count();
        let off = match (e.get(at), e.last()) {
            (Some(x), _) => x.0 >> 16,
            (None, Some(l)) => (l.0 >> 16) + decl_type_size(l.1 & 255).ok_or(Fail::Bad)?,
            (None, None) => 0,
        };
        for x in e.iter_mut().skip(at) {
            x.0 += 4 << 16;
        }
        e.insert(at, elem(off, 0, 11, 0));
    }
    hit(clause::D3D9_FVF_DECL);
    Ok(Some(e))
}

/// the texture stages' registers: the iterated colors, CURRENT and (9) TEMP
struct Regs {
    diffuse: [f64; 4],
    specular: [f64; 4],
    current: [f64; 4],
    temp: [f64; 4],
}

/// the attributes a vertex carries through clipping: z, diffuse (r, g, b, a), specular, u, v
/// z, diffuse, specular, (u, v), rhw, a second (u, v)
pub const ATTRS: usize = 19;
/// the attributes perspective divides (all but z and rhw)
const PERSP: [usize; 17] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18];
/// how many sets of texture coordinates a vertex carries for the stages (TexSet)
const NSET: usize = 3;
/// where each set of texture coordinates (TexSet) is among the attributes: s and t, and q
const UV_AT: [usize; NSET] = [9, 12, 14];
const Q_AT: [usize; NSET] = [16, 17, 18];

/// a set's interpolated attributes as the stages read it: (s / q, t / q)
fn st_of(at: &[f64; ATTRS]) -> [[f64; 2]; NSET] {
    std::array::from_fn(|i| [at[UV_AT[i]] / at[Q_AT[i]], at[UV_AT[i] + 1] / at[Q_AT[i]]])
}

/// what tells sets apart: the TEXCOORDINDEX, and the stage whose texture matrix transforms it
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct SetKey {
    pub tci: u32,
    /// (TEXTURETRANSFORMFLAGS, the stage), when not 0
    pub xform: Option<(u32, u32)>,
}

/// a stage's set through its texture matrix a (TEXTURETRANSFORMFLAGS ttf on untransformed
/// vertices; MEASURED tci-gen2-4, -5, tci-ttf-*; D3D9.STAGE.TEXXFORM): the row v times a in f32,
/// each sum from 0 in the order of v; the first two of it, and the count's last where PROJECTED
/// divides by it (1 where not), but not COUNT2 on Direct3D 8 (tci-ttf-5); COUNT1 leaves v as it
/// is on Direct3D 9 and takes (the first, 0) on Direct3D 8; none for a count but 1 .. 4;
/// spec/lean/Kikizake/D3d9/Coord.lean `texTransform`
pub fn tex_transform(ttf: u32, v8: bool, v: [f32; 4], a: [f32; 16]) -> Option<[f64; 3]> {
    let t: [f32; 4] = std::array::from_fn(|j| v.iter().zip(a.iter().skip(j).step_by(4)).fold(0.0f32, |s, (x, m)| s + x * m));
    let [t0, t1, ..] = t;
    let [v0, v1, ..] = v;
    let count = ttf & 0xff;
    Some(match count {
        1 if v8 => [t0 as f64, 0.0, 1.0],
        1 => [v0 as f64, v1 as f64, 1.0],
        2 if v8 => [t0 as f64, t1 as f64, 1.0],
        2..=4 if ttf & 0x100 != 0 => [t0 as f64, t1 as f64, *t.get(usize::try_from(count - 1).ok()?)? as f64],
        2..=4 => [t0 as f64, t1 as f64, 1.0],
        _ => return None,
    })
}

/// the sets of coordinates a draw's stages read (MEASURED tci-multi-*, tci-three-*: each enabled
/// stage reads its own set, those of the same TEXCOORDINDEX and texture matrix one;
/// D3D9.STAGE.TCI): stage 0's, then each next stage's up to the first COLOROP DISABLE, where
/// new; none past NSET (the generators read at most three sets);
/// spec/lean/Kikizake/D3d9/Coord.lean `slots`
pub fn tci_slots(keys: [SetKey; 8], cops: [u32; 8]) -> Option<Vec<SetKey>> {
    let [first, ..] = keys;
    let mut slots = vec![first];
    for (k, _) in keys.iter().zip(cops).skip(1).take_while(|&(_, c)| c != 1) {
        if !slots.contains(k) {
            slots.push(*k);
        }
    }
    (slots.len() <= NSET).then_some(slots)
}

/// a vertex's set of coordinates `set` among its sets (tex: the words from its first set on;
/// sizes: each set's count), as a texture matrix's row vector: its coordinates, then 1, then 0,
/// past the sets (0, 0, 1, 0); and as passed through, (0, 0) for a set past them or of one
/// coordinate (MEASURED texcoordindex-*; D3D9.STAGE.TCI); none where tex is short of the sets;
/// Coord.lean `row`
pub fn coord_row(tex: &[u32], sizes: &[usize], set: usize) -> Option<([f32; 4], [f32; 4])> {
    let Some(&n) = sizes.get(set) else {
        return Some(([0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 1.0, 0.0]));
    };
    let o: usize = sizes.get(..set)?.iter().sum();
    let mut r = [0.0f32; 4];
    for (k, x) in r.iter_mut().enumerate() {
        *x = match k.cmp(&n) {
            std::cmp::Ordering::Less => f32::from_bits(*tex.get(o.checked_add(k)?)?),
            std::cmp::Ordering::Equal => 1.0,
            std::cmp::Ordering::Greater => 0.0,
        };
    }
    let [_, _, z, q] = r;
    Some((r, if n >= 2 { r } else { [0.0, 0.0, z, q] }))
}

/// TEXCOORDINDEX's generation mode (its high word)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TexGen {
    Passthru,
    Normal,
    Position,
    Reflection,
    SphereMap,
}

impl TexGen {
    /// (4 SPHEREMAP; the generators write no mode past it; MEASURED d3d8draw tci-xyz-3:
    /// Direct3D 8, which has no SPHEREMAP, passes the set through for it)
    pub fn of(tci: u32, v8: bool) -> TexGen {
        match tci >> 16 {
            0 => TexGen::Passthru,
            4 if v8 => TexGen::Passthru,
            1 => TexGen::Normal,
            2 => TexGen::Position,
            3 => TexGen::Reflection,
            _ => TexGen::SphereMap,
        }
    }
}

/// which of a vertex's two carried sets of texture coordinates a stage reads: the set stage 0's
/// TEXCOORDINDEX names, or the one other set a later stage's names
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TexSet {
    First,
    Second,
    Third,
}

impl TexSet {
    const ALL: [TexSet; NSET] = [TexSet::First, TexSet::Second, TexSet::Third];

    fn at(self) -> usize {
        match self {
            TexSet::First => 0,
            TexSet::Second => 1,
            TexSet::Third => 2,
        }
    }
}

/// a vertex of a clipped polygon: its position and attributes, in f32
#[derive(Clone, Copy)]
pub struct PolyVtx {
    pub x: f32,
    pub y: f32,
    pub at: [f32; ATTRS],
}

impl Vtx {
    fn attrs(&self) -> [f32; ATTRS] {
        let d = self.diffuse.map(|c| c as f32);
        let s = self.specular.map(|c| c as f32);
        let u = self.uv.map(|t| t.map(|c| c as f32));
        [self.z as f32, d[0], d[1], d[2], d[3], s[0], s[1], s[2], s[3], u[0][0], u[0][1], self.rhw as f32, u[1][0], u[1][1], u[2][0], u[2][1], u[0][2], u[1][2], u[2][2]]
    }
}

/// (MEASURED, as WARP's Direct3D 10 rasterizer: d3draster's Planes; D3D9.DRAW.XYZ, xyz-*) an
/// untransformed triangle's attributes: the vertices turned so the least clip x comes first, at
/// their snapped screen positions (D3D10 pixel centers); a plane per attribute,
/// A = (x1 - x0)(y2 - y0) - (y1 - y0)(x2 - x0), gx = (d1 (y2 - y0) - d2 (y1 - y0)) (1 / A),
/// gy = (d2 (x1 - x0) - d1 (x2 - x0)) (1 / A), C = (a0 - gx x0) - gy y0, the value at a pixel
/// center (C + gy cy) + gx cx, in f32; the attributes perspective-correct (planes of 1/w and a/w),
/// z on its own plane
struct Planes {
    xs: [f32; 3],
    ys: [f32; 3],
    iw: [f32; 3],
    at: [[f32; ATTRS]; 3],
}

impl Planes {
    fn new(vs: &[Vtx; 3], snap: &dyn Fn(&PolyVtx) -> (i64, i64)) -> Planes {
        let cx = |v: &Vtx| v.clip.map_or(0.0, |c| c[0]);
        let k0 = (0..3).fold(0, |k, i| if cx(&vs[i]) < cx(&vs[k]) { i } else { k });
        let t: [&Vtx; 3] = [0, 1, 2].map(|i| &vs[(k0 + i) % 3]);
        let s = t.map(|v| snap(&PolyVtx { x: v.x, y: v.y, at: [0.0; ATTRS] }));
        let iw = t.map(|v| v.rhw as f32);
        let at = [0, 1, 2].map(|i| {
            let mut a = t[i].attrs();
            for k in PERSP {
                a[k] *= iw[i];
            }
            a
        });
        Planes { xs: s.map(|p| p.0 as f32 / 256.0 + 0.5), ys: s.map(|p| p.1 as f32 / 256.0 + 0.5), iw, at }
    }

    fn plane(&self, a: [f32; 3], cx: f32, cy: f32) -> f32 {
        plane10(self.xs, self.ys, a, cx, cy)
    }

    fn at(&self, cx: f32, cy: f32) -> [f64; ATTRS] {
        let q = 1.0 / self.plane(self.iw, cx, cy);
        std::array::from_fn(|k| {
            let v = self.plane(self.at.map(|a| a[k]), cx, cy);
            (if PERSP.contains(&k) { v * q } else { v }) as f64
        })
    }
}

/// D3D9.SHADE.GOURAUD: an attribute's gradients (d/dx, d/dy) over a triangle at the pixel
/// centers x, y (positions plus half a pixel), in f32: d/dx = ((a1 - a0)(y2 - y0) - (a2 - a0)(y1 -
/// y0)) / area, d/dy = ((a2 - a0)(x1 - x0) - (a1 - a0)(x2 - x0)) / area
pub fn grad9(x: [f32; 3], y: [f32; 3], a: [f32; 3]) -> (f32, f32) {
    let area = (x[1] - x[0]) * (y[2] - y[0]) - (y[1] - y[0]) * (x[2] - x[0]);
    let (d1, d2) = (a[1] - a[0], a[2] - a[0]);
    ((d1 * (y[2] - y[0]) - d2 * (y[1] - y[0])) / area, (d2 * (x[1] - x[0]) - d1 * (x[2] - x[0])) / area)
}

/// D3D9.SHADE.GOURAUD: the attribute at the pixel center (sx, sy), v0 + d/dy (y - y0) + d/dx
/// (x - x0) in that order, in f32
pub fn at9(a0: f32, (dx, dy): (f32, f32), x0: f32, y0: f32, sx: f32, sy: f32) -> f32 {
    a0 + dy * (sy - y0) + dx * (sx - x0)
}

/// what a primitive type draws of nv vertices: lines (2 LINELIST, 3 LINESTRIP) or triangles,
/// each with its first (flat shading) vertex first (D3D9.DRAW.ASSEMBLY: 4 TRIANGLELIST three
/// vertices each, 5 TRIANGLESTRIP each next vertex with the two before, every other one turned to
/// keep the winding, 6 TRIANGLEFAN each next pair with the first); None for the others
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prims {
    Segs(Vec<(usize, usize)>),
    Tris(Vec<[usize; 3]>),
}

pub fn assemble(ty: u32, nv: usize) -> Option<Prims> {
    Some(match ty {
        2 => Prims::Segs((0..nv / 2).map(|i| (2 * i, 2 * i + 1)).collect()),
        3 => Prims::Segs((0..nv.saturating_sub(1)).map(|i| (i, i + 1)).collect()),
        4 | 5 | 6 => {
            hit(clause::D3D9_DRAW_ASSEMBLY);
            Prims::Tris(match ty {
                4 => (0..nv / 3).map(|i| [3 * i, 3 * i + 1, 3 * i + 2]).collect(),
                5 => (0..nv.saturating_sub(2)).map(|i| if i % 2 == 0 { [i, i + 1, i + 2] } else { [i, i + 2, i + 1] }).collect(),
                _ => (0..nv.saturating_sub(2)).map(|i| [i + 1, i + 2, 0]).collect(),
            })
        }
        _ => return None,
    })
}

/// D3D9.DRAW.STRIDE: the words each of nv vertices is read from, out of DrawPrimitiveUP's: the
/// stride the words' count over nv, each vertex's first `size` (the FVF's) words (MEASURED
/// stride-*: the words past them unread); None for a stride short of the size, which the
/// generators write none of
pub fn vertex_words(words: &[u32], nv: usize, size: usize) -> Option<Vec<&[u32]>> {
    let stride = words.len().checked_div(nv)?;
    if stride < size || stride == 0 || words.len() % nv != 0 {
        return None;
    }
    if stride > size {
        hit(clause::D3D9_DRAW_STRIDE);
    }
    words.chunks(stride).map(|c| c.get(..size)).collect()
}

/// D3D9.DRAW.POINTS: the pixel a point lights, x and y each to the nearest pixel center, halves
/// down: ceil(x - 0.5) in f32
pub fn point_pixel(x: f32, y: f32) -> (i64, i64) {
    ((x - 0.5).ceil() as i64, (y - 0.5).ceil() as i64)
}

/// D3D9.SHADE.FLAT, SHADE.GOURAUD: the colors a pixel takes, the first vertex's with flat
/// shading, else the interpolated ones
pub fn shaded<T>(flat: bool, first: T, interpolated: T) -> T {
    if flat {
        hit(clause::D3D9_SHADE_FLAT);
        first
    } else {
        hit(clause::D3D9_SHADE_GOURAUD);
        interpolated
    }
}

/// Direct3D 10's attribute plane (D3DRASTER.INTERP, D3D9.TEX.QUAD, D3D9.DRAW.XYZ):
/// gx = (d1 (y2 - y0) - d2 (y1 - y0)) (1 / A), gy = (d2 (x1 - x0) - d1 (x2 - x0)) (1 / A),
/// C = (a0 - gx x0) - gy y0, the value (C + gy cy) + gx cx, in f32
pub fn plane10(x: [f32; 3], y: [f32; 3], a: [f32; 3], cx: f32, cy: f32) -> f32 {
    let area = (x[1] - x[0]) * (y[2] - y[0]) - (y[1] - y[0]) * (x[2] - x[0]);
    let r = 1.0 / area;
    let (d1, d2) = (a[1] - a[0], a[2] - a[0]);
    let gx = (d1 * (y[2] - y[0]) - d2 * (y[1] - y[0])) * r;
    let gy = (d2 * (x[1] - x[0]) - d1 * (x[2] - x[0])) * r;
    let c = (a[0] - gx * x[0]) - gy * y[0];
    (c + gy * cy) + gx * cx
}

/// the triangle's vertex of least x, then least y (the first of equals)
pub fn least_x(c: &[PolyVtx; 3]) -> usize {
    (0..3).min_by(|&i, &j| (c[i].x, c[i].y).partial_cmp(&(c[j].x, c[j].y)).unwrap_or(std::cmp::Ordering::Equal)).unwrap_or(0)
}

/// a bump mapping stage's color operation
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bump {
    /// BUMPENVMAP
    Plain,
    /// BUMPENVMAPLUMINANCE
    Luminance,
}

impl Bump {
    fn of(cop: u32) -> Option<Bump> {
        match cop {
            22 => Some(Bump::Plain),
            23 => Some(Bump::Luminance),
            _ => None,
        }
    }
}

/// what stage_plan reads of a texture stage: COLOROP, ALPHAOP, whether it has a texture, whether
/// its operations read it (reads_texture) and whether its color operation reads the diffuse
/// color or CURRENT (reads_current)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StageDesc {
    pub cop: u32,
    pub aop: u32,
    pub has_tex: bool,
    pub reads_tex: bool,
    pub reads_cur: bool,
}

/// how a running stage's alpha is made: by an alpha operation, or under ALPHAOP DISABLE the
/// alpha it is given (CURRENT's) or 1
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlphaRule {
    Op(u32),
    Keep,
    One,
}

/// one stage of a draw's texture stages (stage_plan); moved: its texture coordinates moved by the
/// bump mapping stage before it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// the stage's operations, the alpha by the rule; premod: PREMODULATE times the next stage's
    /// texture
    Run { st: u32, moved: bool, alpha: AlphaRule, premod: bool },
    /// a bump mapping stage followed by an enabled stage: it writes nothing
    Defer { st: u32, moved: bool },
    /// a bump mapping stage that is the last: it writes the texture the stage before (from)
    /// sampled, 0 where none did
    BumpLast { st: u32, moved: bool, from: Option<u32> },
    /// a bump mapping stage followed by one, with stage s before it reading the texture it lacks:
    /// the stages off from s, CURRENT as s found it
    Off { st: u32, moved: bool, s: u32 },
}

/// whether a stage's color or alpha operation reads its texture: an argument it uses of
/// D3DTA_TEXTURE (args: COLORARG1, 2, 0, ALPHAARG1, 2, 0), or BLENDTEXTUREALPHA(PM); the alpha
/// operation not under ALPHAOP DISABLE; spec/lean/Kikizake/D3d9/Chain.lean `readsTexture`
pub fn reads_texture(cop: u32, aop: u32, args: [u32; 6]) -> bool {
    let [c1, c2, c0, a1, a2, a0] = args.map(|a| a & 0xf == 2);
    let reads = |op: u32, s1: bool, s2: bool, s0: bool| (op != 3 && s1) || (op != 2 && s2) || (matches!(op, 25 | 26) && s0) || matches!(op, 13 | 14);
    reads(cop, c1, c2, c0) || aop != 1 && reads(aop, a1, a2, a0)
}

/// whether a stage's color operation reads the diffuse color or CURRENT: through an argument it
/// uses (COLORARG1, 2, 0), or BLENDDIFFUSEALPHA, BLENDCURRENTALPHA;
/// spec/lean/Kikizake/D3d9/Chain.lean `readsCurrent`
pub fn reads_current(cop: u32, args: [u32; 3]) -> bool {
    let [s1, s2, s0] = args.map(|a| matches!(a & 0xf, 0 | 1));
    (cop != 3 && s1) || (cop != 2 && s2) || (matches!(cop, 25 | 26) && s0) || matches!(cop, 12 | 16)
}

/// the texture stages a draw runs, in turn to the first whose COLOROP is DISABLE (D3D9.STAGE.MULTI,
/// STAGE.DISABLE: none where stage 0's is, the diffuse color going through), a bump mapping stage
/// followed by an enabled one writing nothing, or turning the stages off from the first before it
/// that reads the texture it lacks (D3D9.STAGE.BUMPCHAIN), ALPHAOP DISABLE keeping the alpha
/// given where the color operation reads it, 1 on stage 0 where not (D3D9.STAGE.ALPHADISABLE.FIRST,
/// STAGE.ALPHADISABLE); spec/lean/Kikizake/D3d9/Chain.lean `plan`
pub fn stage_plan(d: &[StageDesc]) -> Vec<Step> {
    let mut plan = Vec::new();
    // (the first stage so far that reads the texture it lacks)
    let mut lack: Option<u32> = None;
    let mut moved = false;
    for (st, (x, next)) in (0u32..).zip(d.iter().zip(d.iter().skip(1).map(Some).chain([None]))) {
        if x.cop == 1 {
            if st == 0 {
                hit(clause::D3D9_STAGE_DISABLE);
            }
            break;
        }
        if st > 0 {
            hit(clause::D3D9_STAGE_MULTI);
        }
        let next_on = next.is_some_and(|n| n.cop != 1);
        let step = if Bump::of(x.cop).is_some() {
            if next_on {
                hit(clause::D3D9_STAGE_BUMPCHAIN);
                if let Some(s) = lack {
                    plan.push(Step::Off { st, moved, s });
                    return plan;
                }
                Step::Defer { st, moved }
            } else {
                Step::BumpLast { st, moved, from: st.checked_sub(1) }
            }
        } else {
            let alpha = if x.aop != 1 {
                AlphaRule::Op(x.aop)
            } else if x.cop == 24 {
                // (DOTPRODUCT3 writes its own alpha)
                AlphaRule::Op(2)
            } else if st > 0 {
                // (MEASURED alphadisable-stage1-*: a later stage leaves CURRENT's alpha)
                hit(clause::D3D9_STAGE_ALPHADISABLE);
                AlphaRule::Keep
            } else {
                // (MEASURED alphadisable-*: stage 0 the alpha it is given where the color
                // operation reads the diffuse color or CURRENT, else 1)
                hit(clause::D3D9_STAGE_ALPHADISABLE_FIRST);
                if x.reads_cur { AlphaRule::Keep } else { AlphaRule::One }
            };
            let premod = (x.cop == 17 || x.aop == 17) && next_on && next.is_some_and(|n| n.has_tex);
            Step::Run { st, moved, alpha, premod }
        };
        if lack.is_none() && !x.has_tex && x.reads_tex {
            lack = Some(st);
        }
        moved = matches!(step, Step::Defer { .. } | Step::BumpLast { .. });
        plan.push(step);
    }
    plan
}

/// a bump mapped stage's texture coordinates (MEASURED bump-*, D3D9.STAGE.BUMP): (u + (M00 du +
/// M10 dv), v + (M01 du + M11 dv)) in f64, m = (M00, M01, M10, M11);
/// spec/lean/Kikizake/D3d9/Chain.lean `bumpUv`
pub fn bump_uv((u, v): (f64, f64), [m00, m01, m10, m11]: [f64; 4], (du, dv): (f64, f64)) -> (f64, f64) {
    (u + (m00 * du + m10 * dv), v + (m01 * du + m11 * dv))
}

/// BUMPENVMAPLUMINANCE's factor: L LSCALE + LOFFSET clamped to 0..1, in f64 (D3D9.STAGE.BUMP);
/// spec/lean/Kikizake/D3d9/Chain.lean `bumpLuminance`
pub fn bump_luminance(l: f64, scale: f64, offset: f64) -> f64 {
    (l * scale + offset).clamp(0.0, 1.0)
}

/// a color channel times the luminance factor in f32, rounded to the nearest of 255, halves to
/// even (MEASURED bump-2, bump-chain-3; D3D9.STAGE.BUMP); spec/lean/Kikizake/D3d9/Chain.lean
/// `bumpScale`
pub fn bump_scale(c: f64, l: f64) -> f64 {
    f64::from(((c as f32) * (l as f32) * 255.0).round_ties_even() / 255.0)
}

/// PREMODULATE's channel times the next stage's texture, in f32 (MEASURED premod-chain-*;
/// D3D9.STAGE.BUMP); spec/lean/Kikizake/D3d9/Chain.lean `premodulate`
pub fn premodulate(a: f64, t: f64) -> f64 {
    f64::from((a as f32) * (t as f32))
}

/// how a triangle's attributes are set up for its pixels
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Setup {
    /// v0 + d/dy (y - y0) + d/dx (x - x0) (D3D9.SHADE.GOURAUD)
    Warp9,
    /// (C + gy y) + gx x, as for untransformed vertices (D3D9.TEX.QUAD)
    D3d10,
}

/// one in the bilinear filter's weights (32nds of a texel each way)
const BILERP_ONE: i64 = 1024;

/// point filtering's texel of the texel coordinate x (u times the width): x rounded to the
/// nearest 64th, then taken down (MEASURED d3d8draw filter2-3-4 takes texel 2 at 1.99375; 1/16,
/// 1/32 break d3d9draw filter2-3-*, 1/128 misses it; D3D9.TEX.POINT);
/// spec/lean/Kikizake/D3d9/Filter.lean `pointAt`
pub fn point_at(x: f64) -> i64 {
    ((x * 64.0).round() / 64.0).floor() as i64
}

/// linear filtering's first texel of the texel coordinate x and the next one's weight in 32nds of
/// a texel: x - 0.5 taken down, its fraction times 32 taken down (MEASURED filter-*, filter2-*,
/// filter-uv-*; D3D9.TEX.LINEAR); Filter.lean `bilinearAt`
pub fn bilinear_at(x: f64) -> (i64, i64) {
    let x = x - 0.5;
    let x0 = x.floor();
    (x0 as i64, ((x - x0) * 32.0).floor() as i64)
}

/// the weights of the texels (x0, y0), (x0 + 1, y0), (x0, y0 + 1), (x0 + 1, y0 + 1) of the next
/// ones' weights fx, fy (in 32nds, BILERP_ONE in all); Filter.lean `weights`
pub fn bilinear_weights(fx: i64, fy: i64) -> [i64; 4] {
    const Q: i64 = 32;
    const _: () = assert!(Q * Q == BILERP_ONE);
    [(Q - fx) * (Q - fy), fx * (Q - fy), (Q - fx) * fy, fx * fy]
}

/// 8-bit texels' (0..255) weighted sum, in 1/BILERP_ONE of 255; Filter.lean `bytesSum`
pub fn bilinear_bytes(t: [u64; 4], w: [i64; 4]) -> i64 {
    t.iter().zip(w).map(|(&t, w)| t as i64 * w).sum()
}

/// an 8-bit weighted sum truncated to a whole 255th (MEASURED filter-*: the sum truncated), f32;
/// Filter.lean `unit`
pub fn bilinear_unit(v: i64) -> f64 {
    ((v / BILERP_ONE) as f32 / 255.0) as f64
}

/// other texels' weighted sum, in f64; Filter.lean `floatSum`
pub fn bilinear_float(t: [f64; 4], w: [i64; 4]) -> f64 {
    t.iter().zip(w).map(|(&t, w)| t * w as f64).sum::<f64>() / BILERP_ONE as f64
}

/// a bilinear filter's weighted sum
enum Weighted {
    /// of 8-bit texels, in 1/BILERP_ONE of 255
    Bytes([i64; 4]),
    /// of other texels, their values
    Float([f64; 4]),
}

/// a sampler's D3DTEXTUREFILTERTYPE, as WARP filters with it
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TexFilter {
    Point,
    Linear,
    Anisotropic,
    /// Direct3D 9's PYRAMIDALQUAD, GAUSSIANQUAD
    Quad,
    /// NONE: as MAGFILTER POINT (MEASURED filter-other-0-*); as Direct3D 9's MINFILTER it
    /// leaves MAGFILTER's filter, with Direct3D 10's attribute setup (filter-min-other-0)
    None,
    /// CONVOLUTIONMONO, values past them, Direct3D 8's FLATCUBIC, GAUSSIANCUBIC (4, 5):
    /// as MAGFILTER POINT (MEASURED filter-other-*)
    Other,
}

impl TexFilter {
    fn of(v: u32, v8: bool) -> TexFilter {
        match v {
            0 => TexFilter::None,
            1 => TexFilter::Point,
            2 => TexFilter::Linear,
            3 => TexFilter::Anisotropic,
            4 | 5 if !v8 => TexFilter::Quad,
            _ => TexFilter::Other,
        }
    }
}

/// the filter WARP samples a texture of one level with, of a sampler's MAGFILTER and MINFILTER
/// (Point, Linear or Anisotropic, which is still linear for a footprint of no axis past a
/// texel): the level of detail clamped to 0 before it picks, MAGFILTER alone filters
/// (D3D9.TEX.MAGONLY), but for a MINFILTER that makes the whole of it POINT or ANISOTROPIC
/// (D3D9.TEX.FILTEROTHER, D3D9.TEX.QUAD); spec/lean/Kikizake/D3d9/Tex.lean `sampleFilter`
pub fn sample_filter(mag: u32, min: u32, v8: bool) -> TexFilter {
    hit(clause::D3D9_TEX_MAGONLY);
    let (mag, min) = (TexFilter::of(mag, v8), TexFilter::of(min, v8));
    let mag = match min {
        // (MEASURED d3d8draw filter-min-quad-*, filter-min-other-0: Direct3D 8's FLATCUBIC,
        // GAUSSIANCUBIC and NONE as MINFILTER make the whole of it POINT)
        TexFilter::Other | TexFilter::None if v8 => TexFilter::Point,
        // (MEASURED filter-min-other-6, -7, -ffffffff: Direct3D 9's CONVOLUTIONMONO and values
        // past it as MINFILTER make the whole of it POINT too, D3D9.TEX.FILTEROTHER)
        TexFilter::Other => TexFilter::Point,
        // (MEASURED aniso-*: ANISOTROPIC as either filter makes both anisotropic, Direct3D 10's
        // one ANISOTROPIC filter)
        TexFilter::Anisotropic => TexFilter::Anisotropic,
        // (MEASURED filter-min-other-0, filter-min-quad-*: Direct3D 9's NONE, PYRAMIDALQUAD,
        // GAUSSIANQUAD as MINFILTER leave MAGFILTER's filter, with Direct3D 10's attribute
        // setup: d3d10_setup)
        TexFilter::None | TexFilter::Quad | TexFilter::Point | TexFilter::Linear => mag,
    };
    match mag {
        // (MEASURED filter-other-4-*, -5-*: linear, the attributes set up as Direct3D 10's
        // rasterizer does, D3D9.TEX.QUAD)
        TexFilter::Quad => TexFilter::Linear,
        // (MEASURED filter-other-*: NONE, CONVOLUTIONMONO, values past them and Direct3D 8's
        // FLATCUBIC, GAUSSIANCUBIC as MAGFILTER: POINT)
        TexFilter::None | TexFilter::Other => {
            hit(clause::D3D9_TEX_FILTEROTHER);
            TexFilter::Point
        }
        f @ (TexFilter::Point | TexFilter::Linear | TexFilter::Anisotropic) => f,
    }
}

/// whether a textured stage's filters set the draw's attributes up as Direct3D 10's rasterizer
/// does (Setup::D3d10): Direct3D 9's PYRAMIDALQUAD or GAUSSIANQUAD as MAGFILTER, or those or NONE
/// as MINFILTER (MEASURED filter-other-4-*, -5-*, filter-min-quad-*, filter-min-other-0;
/// D3D9.TEX.QUAD, D3D9.TEX.FILTEROTHER); spec/lean/Kikizake/D3d9/Tex.lean `d3d10Setup`
pub fn d3d10_setup(mag: u32, min: u32, v8: bool) -> bool {
    TexFilter::of(mag, v8) == TexFilter::Quad || (!v8 && matches!(TexFilter::of(min, v8), TexFilter::Quad | TexFilter::None))
}

/// what d9draw does before it draws: on x86 a BLENDOP outside ADD..MAX, or an address mode
/// outside WRAP..MIRRORONCE of a stage with a texture, kills the runtime, so the executor reads
/// the state back and does not draw (MEASURED d3d9draw-32, d3d8draw-32; D3D9.BLEND.OPX86,
/// D3D9.TEX.ADDRESSX86); a palette texture bound before any palette is set fails the draw
/// with INVALIDCALL (MEASURED tex-*-P8, tex-*-A8P8; D3D9.TEX.PALETTE.NONE)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawGate {
    Draw,
    BlendOpX86(u32),
    AddressX86(u32),
    NoPalette,
}

/// DrawGate of x86 or x64, BLENDOP, the ADDRESSU and ADDRESSV of each stage with a texture in
/// turn, whether a palette is set and whether a palette texture is bound;
/// spec/lean/Kikizake/D3d9/Tex.lean `gate`
pub fn draw_gate(x86: bool, blend_op: u32, addresses: &[u32], palette_set: bool, palette_tex: bool) -> DrawGate {
    let named = |v: u32| (1..=5).contains(&v);
    if x86 && !named(blend_op) {
        hit(clause::D3D9_BLEND_OPX86);
        return DrawGate::BlendOpX86(blend_op);
    }
    if let Some(&a) = addresses.iter().find(|&&a| x86 && !named(a)) {
        hit(clause::D3D9_TEX_ADDRESSX86);
        return DrawGate::AddressX86(a);
    }
    if !palette_set && palette_tex {
        hit(clause::D3D9_TEX_PALETTE_NONE);
        return DrawGate::NoPalette;
    }
    DrawGate::Draw
}

/// a palette texel (MEASURED palette-*, D3D9.TEX.PALETTE): its entry's color (r, g, b, flags);
/// P8's alpha the entry's flags, A8P8's (40) its own byte a8; spec/lean/Kikizake/D3d9/Tex.lean
/// `paletteTexel`
pub fn palette_texel(fmt: u32, a8: u8, e: [u8; 4]) -> [u8; 4] {
    hit(clause::D3D9_TEX_PALETTE);
    let [r, g, b, flags] = e;
    [r, g, b, if fmt == 40 { a8 } else { flags }]
}

/// the stencil states a pixel is tested and written with: FAIL, ZFAIL, PASS, FUNC, or (9) the
/// CCW ones for a counterclockwise triangle under TWOSIDEDSTENCILMODE (MEASURED stencil2-*,
/// D3D9.STENCIL.TWOSIDED); spec/lean/Kikizake/D3d9/Tex.lean `stencilStates`
pub fn stencil_states(v8: bool, two_sided: u32, ccw: bool) -> [u32; 4] {
    if !v8 && two_sided != 0 && ccw {
        hit(clause::D3D9_STENCIL_TWOSIDED);
        [186, 187, 188, 189]
    } else {
        [53, 54, 55, 56]
    }
}

/// the stretch of a..b inside [-0.5, max.0] x [-0.5, max.1] in f32 (Liang-Barsky): t = (edge - a)
/// / (b - a) per edge, the ends a + t (b - a)
fn clip_line(a: (f32, f32), b: (f32, f32), max: (f32, f32)) -> Option<((f32, f32), (f32, f32))> {
    let (mut t0, mut t1) = (0f32, 1f32);
    for (pa, pb, hi) in [(a.0, b.0, max.0), (a.1, b.1, max.1)] {
        let d = pb - pa;
        if d == 0.0 {
            if pa < -0.5 || pa > hi {
                return None;
            }
            continue;
        }
        let (tl, th) = ((-0.5 - pa) / d, (hi - pa) / d);
        let (enter, leave) = if d > 0.0 { (tl, th) } else { (th, tl) };
        t0 = t0.max(enter);
        t1 = t1.min(leave);
    }
    if t0 > t1 {
        return None;
    }
    let at = |t: f32| if t == 0.0 { a } else if t == 1.0 { b } else { (a.0 + t * (b.0 - a.0), a.1 + t * (b.1 - a.1)) };
    Some((at(t0), at(t1)))
}

/// D3D9.DRAW.LINECLIP, LINECLIP_NEAR: the line of pretransformed ends a, b on a w x h target as
/// it is rasterized: (the ends snapped, the segment drawn: past the target cut to it in f32 and
/// snapped again) in 1/256 pixel; None: nothing drawn
pub fn line_cut(a: (f32, f32), b: (f32, f32), w: i32, h: i32, v8: bool) -> Option<((i128, i128), (i128, i128), (i128, i128), (i128, i128))> {
    let inside = |v: &(f32, f32)| (-0.5..=w as f32 - 0.5).contains(&v.0) && (-0.5..=h as f32 - 0.5).contains(&v.1);
    let snap = |v: f32| i128::from(snap256(v));
    let l = Seg9 { a: (snap(a.0), snap(a.1)), b: (snap(b.0), snap(b.1)) };
    if l.a == l.b {
        return None;
    }
    // (the segment rasterized: a line past the target cut to it in f32 and snapped again)
    let r = match (inside(&a), inside(&b)) {
        (true, true) => {
            Seg9 { a: l.a, b: l.b }
        }
        (ia, ib) => {
            let unsnap = |p: (i128, i128)| (p.0 as f32 / 256.0, p.1 as f32 / 256.0);
            let (mut pa, mut pb) = (unsnap(l.a), unsnap(l.b));
            if !ia && !ib {
                // (MEASURED line-both-*: past the target at both ends, an end past it only
                // on the right or bottom side and by at most half a pixel is moved onto it)
                hit(clause::D3D9_DRAW_LINECLIP_NEAR);
                let (rx, by) = (w as f32 - 0.5, h as f32 - 0.5);
                for p in [&mut pa, &mut pb] {
                    let past = p.0 > rx || p.1 > by;
                    let near = p.0 >= -0.5 && p.1 >= -0.5 && p.0 <= rx + 0.5 && p.1 <= by + 0.5;
                    if past && near {
                        *p = (p.0.min(rx), p.1.min(by));
                    }
                }
            }
            // (MEASURED points-lines-2 of d3d8draw: Direct3D 8's right (bottom) edge at the
            // size itself where an end is at the width (height), as D3D9.RASTER.CLIP8SIDE)
            let side = |size: i32, at: bool| if v8 && at { size as f32 } else { size as f32 - 0.5 };
            let (wf, hf) = (w as f32, h as f32);
            let max = (side(w, pa.0 == wf || pb.0 == wf), side(h, pa.1 == hf || pb.1 == hf));
            if max != (wf - 0.5, hf - 0.5) {
                hit(clause::D3D9_RASTER_CLIP8SIDE);
            }
            let Some((ca, cb)) = clip_line(pa, pb, max) else {
                hit(clause::D3D9_DRAW_LINECLIP);
                return None;
            };
            let snap = |v: f32| i128::from(snap256(v));
            let r = Seg9 { a: (snap(ca.0), snap(ca.1)), b: (snap(cb.0), snap(cb.1)) };
            // (MEASURED line-clip-*, line-random-*, line-end-*-clip, line-corner-*,
            // line-both-*: a line past the target cut to it in f32, t = (edge - a) / (b - a),
            // a + t (b - a), D3D9.DRAW.LINECLIP)
            hit(clause::D3D9_DRAW_LINECLIP);
            r
        }
    };
    if r.a == r.b {
        return None;
    }
    Some((l.a, l.b, r.a, r.b))
}

/// D3D9.DRAW.XYZ: an untransformed position through WORLD, VIEW, PROJECTION (m: the three, the
/// product WORLD VIEW first, then times PROJECTION, each entry a sum from 0 in f32) as a row vector,
/// divided by w, through the viewport (vx, vy, vw, vh, z0, z1): (x, y, z, rhw, the clip-space
/// position)
pub fn transform_xyz(p: [f32; 3], m: [[f32; 16]; 3], vp: (i32, i32, i32, i32, f32, f32)) -> (f32, f32, f32, f32, [f32; 4]) {
    let mul = |a: [f32; 16], b: [f32; 16]| -> [f32; 16] { std::array::from_fn(|i| (0..4).fold(0.0f32, |s, k| s + a[i / 4 * 4 + k] * b[k * 4 + i % 4])) };
    let xv = |v: [f32; 4], a: [f32; 16]| -> [f32; 4] { std::array::from_fn(|j| (0..4).fold(0.0f32, |s, k| s + v[k] * a[k * 4 + j])) };
    let c = xv([p[0], p[1], p[2], 1.0], mul(mul(m[0], m[1]), m[2]));
    let (vx, vy, vw, vh, z0, z1) = vp;
    let rhw = 1.0 / c[3];
    let (x, y, z) = (c[0] * rhw, c[1] * rhw, c[2] * rhw);
    let (hw, hh) = (vw as f32 / 2.0, vh as f32 / 2.0);
    (vx as f32 + (1.0 + x) * hw, vy as f32 + (1.0 - y) * hh, z0 + z * (z1 - z0), rhw, c)
}

/// D3D9.DRAW.XYZ.CLIP: an untransformed triangle (clip-space positions, attributes) clipped in clip
/// space to the planes half a pixel in from a vw x vh viewport's edges (the positions shifted by
/// (w / vw, -w / vh), then -w <= x <= w, -w <= y <= w in turn, Sutherland-Hodgman, each new vertex
/// a + (b - a) t, t = da / (da - db), in f32); None: no vertex outside, nothing clipped
pub fn clip_xyz(poly: &[([f32; 4], [f32; ATTRS])], vw: i32, vh: i32) -> Option<Vec<([f32; 4], [f32; ATTRS])>> {
    let sh = |c: &[f32; 4]| [c[0] + c[3] / vw as f32, c[1] - c[3] / vh as f32, c[2], c[3]];
    let dist = |q: &[f32; 4], plane: usize| match plane {
        0 => q[3] + q[0],
        1 => q[3] - q[0],
        2 => q[3] + q[1],
        _ => q[3] - q[1],
    };
    let mut poly: Vec<([f32; 4], [f32; ATTRS])> = poly.iter().map(|(c, at)| (sh(c), *at)).collect();
    if !poly.iter().any(|(q, _)| (0..4).any(|p| dist(q, p) < 0.0)) {
        return None;
    }
    hit(clause::D3D9_DRAW_XYZ_CLIP);
    for plane in 0..4 {
        let mut out = Vec::new();
        for (i, &p) in poly.iter().enumerate() {
            let Some(&q) = poly.get((i + 1) % poly.len()) else { continue };
            let (dp, dq) = (dist(&p.0, plane), dist(&q.0, plane));
            if dp >= 0.0 {
                out.push(p);
            }
            if (dp >= 0.0) != (dq >= 0.0) {
                let t = dp / (dp - dq);
                out.push((std::array::from_fn(|k| p.0[k] + (q.0[k] - p.0[k]) * t), std::array::from_fn(|k| p.1[k] + (q.1[k] - p.1[k]) * t)));
            }
        }
        poly = out;
    }
    Some(poly)
}

/// D3D9.DRAW.XYZ.CLIP: a clipped vertex (shifted clip-space position q) on the screen of viewport
/// (vx, vy, vw, vh): x, y, rhw = 1 / w, and its clip-space position unshifted
pub fn xyz_vertex(q: [f32; 4], vp: (i32, i32, i32, i32)) -> (f32, f32, f32, [f32; 4]) {
    let (vx, vy, vw, vh) = vp;
    let rhw = 1.0 / q[3];
    (
        vx as f32 - 0.5 + (1.0 + q[0] * rhw) * (vw as f32 / 2.0),
        vy as f32 - 0.5 + (1.0 - q[1] * rhw) * (vh as f32 / 2.0),
        rhw,
        [q[0] - q[3] / vw as f32, q[1] + q[3] / vh as f32, q[2], q[3]],
    )
}

/// D3D9.DRAW.RHW: a pretransformed vertex's attributes for perspective-correct interpolation:
/// every one but z (0) and rhw itself (11) times rhw, in f32
pub fn rhw_attrs(at: [f32; ATTRS]) -> [f32; ATTRS] {
    hit(clause::D3D9_DRAW_RHW);
    std::array::from_fn(|k| if PERSP.contains(&k) { at[k] * at[11] } else { at[k] })
}

/// D3D9.DRAW.RHW: the value at a pixel of the attributes interpolated so: each but z and rhw divided
/// by the interpolated rhw, in f32
pub fn rhw_value(at32: [f32; ATTRS]) -> [f64; ATTRS] {
    std::array::from_fn(|k| f64::from(if PERSP.contains(&k) { at32[k] / at32[11] } else { at32[k] }))
}

/// D3D9.TEX.ANISO2: the footprint of derivatives a (ddx) and b (ddy) in texture coordinates on a
/// w x h texture, in f32 one rounding an operation: (r, the major axis's direction, the tap count
/// n of MAXANISOTROPY `limit`); r <= 1 filters bilinearly
pub fn aniso_footprint(a: [f32; 2], b: [f32; 2], (w, h): (f32, f32), limit: u32) -> (f32, [f32; 2], u32) {
    let (a, b) = ([a[0] * w, a[1] * h], [b[0] * w, b[1] * h]);
    let x2 = a[0] * a[0] + a[1] * a[1];
    let y2 = b[0] * b[0] + b[1] * b[1];
    let area = (a[0] * b[1] - a[1] * b[0]).abs();
    // (a tie takes ddy as the major axis)
    let (m2, dir) = if x2 > y2 { (x2, a) } else { (y2, b) };
    let m = m2.sqrt();
    let inv = 1.0 / m;
    let axis = dir.map(|c| c * inv);
    // (the tap count: MAXANISOTROPY taken down to a power of 2, at most 16)
    let n: u32 = match limit {
        0..=1 => 1,
        2..=3 => 2,
        4..=7 => 4,
        8..=15 => 8,
        _ => 16,
    };
    let nf: f32 = n.to_string().parse().unwrap_or(1.0);
    let mut r = m2 / area;
    let clamped = r > nf;
    if clamped {
        r = nf;
    }
    let minor = if clamped { m / r } else { area / m };
    if minor < 1.0 {
        r = (r * minor).max(1.0);
    }
    let nan0 = |x: f32| if x.is_nan() { 0.0 } else { x };
    (nan0(r), axis.map(nan0), n)
}

/// D3D9.TEX.ANISO2, ANISON: where the taps of footprint (r, axis, n) around (u, v) fall, in f32:
/// r <= 1 none; r <= 2 two at the center plus and less 0.5 (r - 1) axis; past 2 n taps, step =
/// f32(1/(n + 1)) r, the first at the center plus (-0.5 r + step) axis, each next a step on
pub fn aniso_taps((u, v): (f32, f32), r: f32, axis: [f32; 2], n: u32, (w, h): (f32, f32)) -> Vec<(f32, f32)> {
    if r.is_nan() || r <= 1.0 {
        return Vec::new();
    }
    if r <= 2.0 {
        // (MEASURED aniso-*, aniso-axis-*: two taps, 0.5 (r - 1) along the axis each way)
        hit(clause::D3D9_TEX_ANISO2);
        let hh = 0.5 * (r - 1.0);
        let off = [hh * axis[0] / w, hh * axis[1] / h];
        return vec![(u + off[0], v + off[1]), (u - off[0], v - off[1])];
    }
    // (MEASURED aniso-axis-*, aniso-*: n taps, the step 1/(n + 1) in f32 times r, the first at
    // -r/2 plus a step, a step added for each)
    hit(clause::D3D9_TEX_ANISON);
    let k = 1.0 / n.saturating_add(1).to_string().parse::<f32>().unwrap_or(1.0);
    let step = k * r;
    let start = -0.5 * r + step;
    let d = [axis[0] / w, axis[1] / h];
    let sv = [d[0] * step, d[1] * step];
    let (mut qu, mut qv) = (u + d[0] * start, v + d[1] * start);
    let mut taps = Vec::new();
    for _ in 0..n {
        taps.push((qu, qv));
        (qu, qv) = (qu + sv[0], qv + sv[1]);
    }
    taps
}

/// D3D9.TEX.ANISON: the taps' sums averaged in pairs of neighbors (truncating) until one is left
/// (MEASURED aniso-axis-*: summing them all misses pixels); None for no taps or a count not a
/// power of two
pub fn pair_average(mut vals: Vec<[i64; 4]>) -> Option<[i64; 4]> {
    while vals.len() > 1 {
        vals = vals.chunks(2).map(|p| match p {
            [a, b] => {
                let mut o = [0i64; 4];
                for ((o, x), y) in o.iter_mut().zip(a).zip(b) {
                    *o = (x + y) / 2;
                }
                Some(o)
            }
            _ => None,
        }).collect::<Option<_>>()?;
    }
    vals.first().copied()
}

/// D3D9.STAGE.TCIGEN: a position in camera space: the row vector (p, 1) times WORLD, then times
/// VIEW, each entry a sum from 0 in f32
pub fn camera_space(p: [f32; 3], world: [f32; 16], view: [f32; 16]) -> [f32; 4] {
    let xv = |v: [f32; 4], a: [f32; 16]| -> [f32; 4] { std::array::from_fn(|j| (0..4).fold(0.0f32, |s, k| s + v[k] * a[k * 4 + j])) };
    xv(xv([p[0], p[1], p[2], 1.0], world), view)
}

/// D3D9.STAGE.TCIGEN: a normal in camera space: through the inverse transpose of WORLD VIEW's upper
/// 3 x 3 (its cofactors over the determinant), in f32, normalized when `normalize`
pub fn camera_normal(n: [f32; 3], world: [f32; 16], view: [f32; 16], normalize: bool) -> [f32; 3] {
    let (a, b) = (world, view);
    let wv: [f32; 9] = std::array::from_fn(|i| (0..3).fold(0.0f32, |s, k| s + a[i / 3 * 4 + k] * b[k * 4 + i % 3]));
    // (the cofactors: the inverse transpose up to 1 / det)
    let e = |r: usize, c: usize| wv[r * 3 + c];
    let cof: [f32; 9] = std::array::from_fn(|i| {
        let (r, c) = (i / 3, i % 3);
        let (r0, r1, c0, c1) = ((r + 1) % 3, (r + 2) % 3, (c + 1) % 3, (c + 2) % 3);
        e(r0, c0) * e(r1, c1) - e(r0, c1) * e(r1, c0)
    });
    let det = e(0, 0) * cof[0] + e(0, 1) * cof[1] + e(0, 2) * cof[2];
    // n' = n (M^-1)^T: (M^-1)^T = cof / det, row vector times it
    let t: [f32; 3] = std::array::from_fn(|j| (0..3).fold(0.0f32, |s, k| s + n[k] * cof[k * 3 + j]) / det);
    if normalize {
        let l = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        t.map(|x| x / l)
    } else {
        t
    }
}

/// D3D9.STAGE.TCIGEN: the coordinates generated from camera-space position c and normal nn:
/// CAMERASPACENORMAL (Nx, Ny); CAMERASPACEPOSITION (Px, Py); CAMERASPACEREFLECTIONVECTOR R = 2 (E.N)
/// N - E, E = -P / |P| under LOCALVIEWER, else (0, 0, 1); SPHEREMAP (Rx / m + 1/2, Ry / m + 1/2), m
/// = 2 sqrt(Rx^2 + Ry^2 + (Rz - 1)^2); as the row (g, 1)
pub fn tex_gen(texgen: TexGen, c: [f32; 4], nn: [f32; 3], local: bool) -> [f32; 4] {
    let pl = (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    // (MEASURED tci-gen-4, -5: without LOCALVIEWER E = (0, 0, 1))
    let e = if local { [-c[0] / pl, -c[1] / pl, -c[2] / pl] } else { [0.0, 0.0, 1.0] };
    let en = e[0] * nn[0] + e[1] * nn[1] + e[2] * nn[2];
    let r = [2.0 * en * nn[0] - e[0], 2.0 * en * nn[1] - e[1], 2.0 * en * nn[2] - e[2]];
    let g: [f32; 3] = match texgen {
        TexGen::Passthru => [0.0; 3],
        TexGen::Normal => nn,
        TexGen::Position => [c[0], c[1], c[2]],
        TexGen::Reflection => r,
        TexGen::SphereMap => {
            let m = 2.0 * (r[0] * r[0] + r[1] * r[1] + (r[2] - 1.0) * (r[2] - 1.0)).sqrt();
            [r[0] / m + 0.5, r[1] / m + 0.5, 0.0]
        }
    };
    [g[0], g[1], g[2], 1.0]
}

/// D3D9.STAGE.PROJFOOT: the offset from pixel (px, py) to its 2 x 2 quad's top-left pixel
pub fn quad_origin(px: i32, py: i32) -> (f32, f32) {
    let bit = |v: i32| if v & 1 == 0 { 0.0 } else { -1.0 };
    (bit(px), bit(py))
}

/// D3D9.STAGE.PROJFOOT: each set's footprint from the quad's top-left pixel's attributes c against
/// its right (r) and lower (d) neighbours': Direct3D 10's coarse derivatives of s/q and t/q, in f64
/// ((ds/dx, ds/dy), (dt/dx, dt/dy))
pub fn quad_duv(c: &[f64; ATTRS], r: &[f64; ATTRS], d: &[f64; ATTRS]) -> [((f64, f64), (f64, f64)); NSET] {
    let mut out = [((0.0, 0.0), (0.0, 0.0)); NSET];
    for (o, ((c, r), d)) in out.iter_mut().zip(st_of(c).iter().zip(&st_of(r)).zip(&st_of(d))) {
        *o = ((r[0] - c[0], d[0] - c[0]), (r[1] - c[1], d[1] - c[1]));
    }
    out
}

/// a line's snapped ends (1/256 pixel, Direct3D 9's pixel centers at whole pixels)
struct Seg9 {
    a: (i128, i128),
    b: (i128, i128),
}

/// half a pixel in 1/256: Direct3D 9's pixel centers are Direct3D 10's corners
const HALF: i128 = 128;

/// the sides a triangle is clipped against, in the order it is
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// a polygon clipped to -0.5 ..= x1 and -0.5 ..= y1, a side at a time (Sutherland-Hodgman, each
/// edge from its first vertex: p + t (q - p), t = dp / (dp - dq), in f32, the attributes alike),
/// on the sides (left, right, top, bottom) some vertex of the triangle is outside of; the new
/// vertices then clamped into the viewport (the clean-room notes: WARP clamps them to its clip
/// space's -w ..= w, so a new vertex past a side not clipped lands on it; MEASURED clip8-*,
/// raster-3, -54, -90 of d3d8draw: on a side clipped at the size itself, Direct3D 8's, there)
pub fn clip_polygon(poly: &[PolyVtx], x1: f32, y1: f32, sides: [bool; 4]) -> Vec<PolyVtx> {
    let mut poly: Vec<(PolyVtx, bool)> = poly.iter().map(|v| (*v, false)).collect();
    for (si, side) in [Side::Left, Side::Right, Side::Top, Side::Bottom].into_iter().enumerate() {
        if !sides[si] {
            continue;
        }
        let d = |v: &PolyVtx| match side {
            Side::Left => v.x + 0.5,
            Side::Right => x1 - v.x,
            Side::Top => v.y + 0.5,
            Side::Bottom => y1 - v.y,
        };
        let mut out = Vec::new();
        for (i, (p, pn)) in poly.iter().enumerate() {
            let (q, _) = &poly[(i + 1) % poly.len()];
            let (dp, dq) = (d(p), d(q));
            if dp >= 0.0 {
                out.push((*p, *pn));
            }
            if (dp >= 0.0) != (dq >= 0.0) {
                let t = dp / (dp - dq);
                out.push((PolyVtx { x: p.x + t * (q.x - p.x), y: p.y + t * (q.y - p.y), at: std::array::from_fn(|k| p.at[k] + t * (q.at[k] - p.at[k])) }, true));
            }
        }
        poly = out;
    }
    let (cx, cy) = (if sides[1] { x1 } else { x1.min(x1.ceil() - 0.5) }, if sides[3] { y1 } else { y1.min(y1.ceil() - 0.5) });
    poly.into_iter().map(|(mut v, new)| {
        if new {
            v.x = v.x.clamp(-0.5, cx);
            v.y = v.y.clamp(-0.5, cy);
        }
        v
    }).collect()
}

/// D3D9.RASTER.CLIP, CLIP8, CLIP8SIDE: where a triangle is clipped, (the right side, the bottom
/// side, the sides (left, right, top, bottom) some vertex is strictly outside of: x < 0, x > the
/// width, y < 0, y > the height; the clean-room notes, MEASURED clip8-*). The right and bottom
/// sides half a pixel in, at the size itself with perspective (MEASURED rhw-6) and on Direct3D 8
/// with a vertex at the width (height) itself (d3d8draw raster-77, -96 and 17 more)
pub fn clip_bounds(pts: [(f32, f32); 3], w: i32, h: i32, v8: bool, persp: bool) -> (f32, f32, [bool; 4]) {
    let (wf, hf): (f32, f32) = (w.to_string().parse().unwrap_or(f32::NAN), h.to_string().parse().unwrap_or(f32::NAN));
    let at_w = v8 && pts.iter().any(|v| v.0 == wf);
    let at_h = v8 && pts.iter().any(|v| v.1 == hf);
    if at_w || at_h {
        hit(clause::D3D9_RASTER_CLIP8SIDE);
    }
    hit(clause::D3D9_RASTER_CLIP8);
    let edge = if persp { 0.0 } else { 0.5 };
    let (ex, ey) = (if at_w { 0.0 } else { edge }, if at_h { 0.0 } else { edge });
    (wf - ex, hf - ey, [pts.iter().any(|v| v.0 < 0.0), pts.iter().any(|v| v.0 > wf), pts.iter().any(|v| v.1 < 0.0), pts.iter().any(|v| v.1 > hf)])
}

/// a convex polygon of n vertices cut into the strip v1 v2 v0 v3 v(n-1) v4 v(n-2) ... (MEASURED:
/// d3d9draw shade-clip-*, no other order fits all 16)
pub fn strip(n: usize) -> Vec<[usize; 3]> {
    let mut order = vec![0];
    let (mut lo, mut hi) = (1, n.saturating_sub(1));
    while lo <= hi {
        order.push(lo);
        if lo != hi {
            order.push(hi);
        }
        lo += 1;
        hi -= 1;
    }
    let order: Vec<usize> = order.iter().map(|&k| (k + 1) % n.max(1)).collect();
    order.windows(3).map(|t| [t[0], t[1], t[2]]).collect()
}

/// the top-left fill convention on a triangle in 1/256 pixel units: pixel centers (integer
/// coordinates) inside, or on a top or a left edge
fn cover(t: [(i64, i64); 3], w: i32, h: i32, covered: &mut [bool]) {
    for py in 0..h {
        for px in 0..w {
            if covers(t, i64::from(px) * 256, i64::from(py) * 256) {
                covered[(py * w + px) as usize] = true;
            }
        }
    }
}

/// one channel's inputs to a texture stage operation (D3DTEXTUREOP), in f32: the arguments
/// (D3DTA, after COMPLEMENT and ALPHAREPLICATE), the first argument's alpha, the diffuse and
/// current alphas, the texture's alpha (1 without one), the texture factor's alpha and the
/// channel's current value
#[derive(Clone, Copy)]
pub struct StageIn {
    pub op: u32,
    pub st: u32,
    pub alpha: bool,
    pub a0: f32,
    pub a1: f32,
    pub a2: f32,
    pub a1a: f32,
    pub da: f32,
    pub ca: f32,
    pub ta: f32,
    pub fa: f32,
    pub cur: f32,
}

/// a texture stage operation's value of one channel, before it is saturated (D3D9.STAGE.OP,
/// STAGE.PASTOP): each operation's formula in f32, in the order the operations are written
pub fn stage_value(x: StageIn) -> Res<f32> {
    let StageIn { op, st, alpha, a0, a1, a2, a1a, da, ca, ta, fa, cur } = x;
    Ok(match op {
        // (PREMODULATE: MEASURED top-*-PREMODULATE, stage1-PREMODULATE: as SELECTARG1)
        2 | 17 => a1,
        3 => a2,
        4 => a1 * a2,
        5 => a1 * a2 * 2.0,
        6 => a1 * a2 * 4.0,
        7 => a1 + a2,
        8 => a1 + a2 - 0.5,
        9 => (a1 + a2 - 0.5) * 2.0,
        10 => a1 - a2,
        11 => a1 + a2 - a1 * a2,
        12 => a1 * da + a2 * (1.0 - da),
        13 => a1 * ta + a2 * (1.0 - ta),
        14 => a1 * fa + a2 * (1.0 - fa),
        15 => a1 + a2 * (1.0 - ta),
        16 => a1 * ca + a2 * (1.0 - ca),
        // (MEASURED, top-alpha-*, stage1-*: the color-only operations as alpha
        // operations give 1, the alpha before on a later stage; the bump mapping ones
        // and DOTPRODUCT3 too, top-alpha-BUMPENVMAP*, top-alpha-DOTPRODUCT3)
        18..=24 if alpha => if st == 0 { 1.0 } else { ca },
        18 => a1 + a1a * a2,
        19 => a1 * a2 + a1a,
        20 => (1.0 - a1a) * a2 + a1,
        21 => (1.0 - a1) * a2 + a1a,
        25 => a0 + a1 * a2,
        26 => a0 * a1 + (1.0 - a0) * a2,
        // (MEASURED top-past-*, top-past-later-*, both runtimes, stage 0: a COLOROP
        // past LERP makes black; as ALPHAOP 32 makes 1, the others leave the diffuse
        // alpha, D3D9.STAGE.PASTOP)
        27.. if st == 0 && !alpha => {
            hit(clause::D3D9_STAGE_PASTOP);
            0.0
        }
        32 if st == 0 => {
            hit(clause::D3D9_STAGE_PASTOP);
            1.0
        }
        27.. if st == 0 => {
            hit(clause::D3D9_STAGE_PASTOP);
            da
        }
        // (MEASURED top-past-later-*, top-past-chain-*: on a later stage any
        // operation past LERP leaves CURRENT, as COLOROP and as ALPHAOP, and the
        // stages after it go on)
        27.. if !alpha => {
            hit(clause::D3D9_STAGE_PASTOP);
            cur
        }
        27.. => {
            hit(clause::D3D9_STAGE_PASTOP);
            ca
        }
        // (never here: a COLOROP DISABLE ends the stages and an ALPHAOP DISABLE
        // runs as SELECTARG1; the bump mapping COLOROPs and DOTPRODUCT3 are run
        // before)
        1 | 22..=24 => return Err(Fail::Bad),
        // (open: an operation of 0, cases top-zero-*)
        0 => return Err(Fail::Unmodeled(open::D3D9_STAGE_OPEN)),
    })
}

/// SPECULARENABLE: the specular color added to the color, to 1 at most (D3D9.SPECULAR)
pub fn specular_add(c: f64, s: f64) -> f64 {
    (c + s).min(1.0)
}

/// the pixel fog factor (MEASURED fog-*, D3D9.FOG): FOGTABLEMODE NONE the iterated specular alpha,
/// EXP, EXP2 and LINEAR of the pixel's z with FOGSTART, FOGEND, FOGDENSITY, in f64, clamped to
/// 0..1 and taken to f32
pub fn fog_factor(mode: u32, z: f64, specular_alpha: f64, (start, end, density): (f64, f64, f64)) -> Res<f32> {
    let f = match mode {
        0 => specular_alpha,
        1 | 2 => fog_exp_arg(mode, z, density).exp(),
        3 => (end - z) / (end - start),
        // (the generators set no other table mode)
        _ => return Err(Fail::Bad),
    };
    Ok(f.clamp(0.0, 1.0) as f32)
}

/// EXP's and EXP2's exponent: -(z d), -(z d) (z d)
pub fn fog_exp_arg(mode: u32, z: f64, density: f64) -> f64 {
    let n = -(z * density);
    if mode == 1 { n } else { n * (z * density) }
}

/// a channel fogged: f c + (1 - f) fog, in f32 (D3D9.FOG)
pub fn fog_blend(f: f32, c: f32, fog: f32) -> f32 {
    f * c + (1.0 - f) * fog
}

/// what a stage's arguments (D3DTA) read: the registers, the stage's texel (none without a
/// texture), TEXTUREFACTOR and the stage's CONSTANT
pub struct ArgRegs {
    pub diffuse: [f64; 4],
    pub current: [f64; 4],
    pub specular: [f64; 4],
    pub temp: [f64; 4],
    pub tex: Option<[f64; 4]>,
    pub factor: u32,
    pub constant: u32,
}

/// a stage argument (D3DTA): the register the selector names, then ALPHAREPLICATE (0x20) and
/// COMPLEMENT (0x10) (D3D9.STAGE.ARG, STAGE.NOTEX: with no texture, color 1 and the current
/// alpha); on Direct3D 8 no TEMP or CONSTANT
pub fn stage_arg(a: u32, r: &ArgRegs, v8: bool) -> Res<[f64; 4]> {
    let mut v = match a & 0xf {
        0 => r.diffuse,
        // (stage 0: CURRENT is the diffuse color)
        1 => r.current,
        // (MEASURED: with no texture, color 1 and the current alpha)
        2 => r.tex.unwrap_or_else(|| {
            hit(clause::D3D9_STAGE_NOTEX);
            [1.0, 1.0, 1.0, r.current[3]]
        }),
        3 => argb(r.factor),
        4 => r.specular,
        5 if !v8 => {
            hit(clause::D3D9_STAGE_ARG);
            r.temp
        }
        6 if !v8 => {
            hit(clause::D3D9_STAGE_ARG);
            argb(r.constant)
        }
        // (D3DTA has no other selector, and Direct3D 8 no TEMP or CONSTANT: the generators write none)
        _ => return Err(Fail::Bad),
    };
    if a & 0x20 != 0 {
        v = [v[3]; 4];
    }
    if a & 0x10 != 0 {
        v = v.map(|x| 1.0 - x);
    }
    if a & !0x3f != 0 {
        // (no D3DTA modifier beyond COMPLEMENT and ALPHAREPLICATE: the generators write none)
        return Err(Fail::Bad);
    }
    Ok(v)
}

/// how the vertices of a triangle wind on the screen (y down): the sign of its area
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Winding {
    /// a positive area: clockwise on the screen
    Cw,
    /// a negative area
    Ccw,
    /// no area
    Flat,
}

/// whether CULLMODE (D3DCULL: NONE 1, CW 2, CCW 3) culls a triangle of this winding (D3D9.CULL);
/// a mode outside the enumeration culls as CW on x64 and as CCW on x86 (MEASURED cull-0, cull-4,
/// D3D9.CULL.BAD)
pub fn culls(mode: u32, x86: bool, winding: Winding) -> bool {
    let mode = match mode {
        1..=3 => mode,
        _ => {
            hit(clause::D3D9_CULL_BAD);
            if x86 { 3 } else { 2 }
        }
    };
    match (mode, winding) {
        (1, _) | (_, Winding::Flat) => false,
        (3, w) => w == Winding::Ccw,
        (_, w) => w == Winding::Cw,
    }
}

/// whether the point (sx, sy) is drawn by the triangle t, all in 1/256 pixel units: strictly
/// inside each edge, or on an edge that is a top one (going left, the triangle turned to wind
/// with a positive area) or a left one (going up); a triangle of no area draws nothing
/// (D3D9.RASTER.TOPLEFT)
pub fn covers(t: [(i64, i64); 3], sx: i64, sy: i64) -> bool {
    let area = (t[1].0 - t[0].0) * (t[2].1 - t[0].1) - (t[1].1 - t[0].1) * (t[2].0 - t[0].0);
    if area == 0 {
        return false;
    }
    let t = if area < 0 { [t[0], t[2], t[1]] } else { t };
    (0..3).all(|k| {
        let (p, q) = (t[k], t[(k + 1) % 3]);
        let side = (q.0 - p.0) * (sy - p.1) - (q.1 - p.1) * (sx - p.0);
        side > 0 || (side == 0 && (q.1 - p.1 < 0 || (q.1 == p.1 && q.0 - p.0 > 0)))
    })
}

fn fl(w: u32) -> f64 {
    f32::from_bits(w) as f64
}

impl D3d9 {
    fn rs(&self, i: u32) -> u32 {
        self.state.get(&Key::Rs(i)).copied().unwrap_or(0)
    }
    fn tss(&self, s: u32, t: u32) -> u32 {
        self.state.get(&Key::Tss(s, t)).copied().unwrap_or(0)
    }
    /// whether an enabled stage with a texture sets the attributes up as Direct3D 10 does
    /// (d3d10_setup)
    fn quad_filtered(&self) -> bool {
        (0..8u32).take_while(|&st| self.tss(st, 1) != 1).any(|st| self.draw.stage_tex[st as usize].is_some() && d3d10_setup(self.samp(st, 5), self.samp(st, 6), self.v8))
    }
    /// a sampler state (Direct3D 8: the stage state of the same meaning)
    fn samp(&self, s: u32, t: u32) -> u32 {
        if self.v8 {
            // ADDRESSU V W, BORDERCOLOR, MAG MIN MIP filters, MIPMAPLODBIAS, MAXMIPLEVEL,
            // MAXANISOTROPY: stage states 13 14 25 15 16 17 18 19 20 21
            const T8: [u32; 11] = [0, 13, 14, 25, 15, 16, 17, 18, 19, 20, 21];
            let t8 = T8[t as usize];
            self.tss(s, t8)
        } else {
            self.state.get(&Key::Ss(s, t)).copied().unwrap_or(0)
        }
    }

    pub(super) fn draw_op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let v8_only_9 = matches!(t[0].as_str(), "d9decl" | "d9getdecl" | "d9scissor");
        if self.v8 && v8_only_9 {
            return Err(Fail::Bad);
        }
        if !self.dev {
            out.push("nodev".into());
            return Ok(true);
        }
        match t[0].as_str() {
            "d9target" => {
                let (w, h, f) = (n(op, 1)? as i32, n(op, 2)? as i32, n(op, 3)? as u32);
                hit(clause::D3D9_TARGET);
                if !supported(f, true, self.v8) {
                    // (and the device's own back buffer stays the target: D3D9.CLEAR.BACKBUFFER)
                    self.draw.rt = None;
                    out.push(format!("target failed {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let fm = format(f).ok_or(Fail::Bad)?;
                self.draw.rt = Some(Surface { w, h, fmt: f, bytes: vec![0; (w * h) as usize * fm.bytes] });
                self.draw.depth = vec![0; (w * h) as usize];
                self.draw.stencil = vec![0; (w * h) as usize];
                // (Direct3D 9: SetRenderTarget resets the viewport and the scissor rectangle to the target)
                self.draw.scissor = None;
                self.draw.viewport = None;
                // (what a new target holds is not defined: every case clears it before drawing)
            }
            "d9clear" => {
                let flags = n(op, 1)? as u32;
                let c = n(op, 2)? as u32;
                let z = fl(n(op, 3)? as u32);
                let st = n(op, 4)? as u32;
                let mut rects = Vec::new();
                let mut i = 5;
                while i + 3 < t.len() {
                    rects.push((n(op, i)? as i32, n(op, i + 1)? as i32, n(op, i + 2)? as i32, n(op, i + 3)? as i32));
                    i += 4;
                }
                hit(clause::D3D9_CLEAR);
                if flags & 7 == 0 {
                    hit(clause::D3D9_CLEAR_FLAGS);
                    out.push(format!("clear hr {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let Some(rt) = &mut self.draw.rt else {
                    hit(clause::D3D9_CLEAR_BACKBUFFER);
                    return Ok(true);
                };
                let (w, h) = (rt.w, rt.h);
                // (MEASURED: with the scissor test on, Clear keeps to the scissor rectangle too)
                let (sx0, sy0, sx1, sy1) = match self.draw.scissor {
                    Some(r) if !self.v8 && self.state.get(&Key::Rs(174)).copied().unwrap_or(0) != 0 => {
                        hit(clause::D3D9_SCISSOR);
                        r
                    }
                    _ => (0, 0, w, h),
                };
                // (D3D9.CLEAR: the rectangles clipped to the viewport, NULL the whole viewport)
                let (vx0, vy0, vx1, vy1) = match self.draw.viewport {
                    Some((x, y, vw, vh, _, _)) => (x, y, x.saturating_add(vw), y.saturating_add(vh)),
                    None => (0, 0, w, h),
                };
                let (sx0, sy0, sx1, sy1) = (sx0.max(vx0), sy0.max(vy0), sx1.min(vx1), sy1.min(vy1));
                let all = [(0, 0, w, h)];
                let list: &[(i32, i32, i32, i32)] = if rects.is_empty() { &all } else { &rects };
                let fm = format(rt.fmt).ok_or(Fail::Bad)?;
                hit(clause::D3D9_CLEAR_COLOR);
                let px = encode(&fm, argb(c));
                let zq = unorm(z, 24) as u32;
                for &(x0, y0, x1, y1) in list {
                    for y in y0.max(0).max(sy0)..y1.min(h).min(sy1) {
                        for x in x0.max(0).max(sx0)..x1.min(w).min(sx1) {
                            if flags & 1 != 0 {
                                rt.set(x, y, &px);
                            }
                            let k = (y * w + x) as usize;
                            if flags & 2 != 0 {
                                self.draw.depth[k] = zq;
                            }
                            if flags & 4 != 0 {
                                self.draw.stencil[k] = st as u8;
                            }
                        }
                    }
                }
            }
            "d9rt" => {
                let Some(rt) = &self.draw.rt else {
                    out.push("rt none".into());
                    return Ok(true);
                };
                hit(clause::D3D9_RT_READ);
                let b = format(rt.fmt).ok_or(Fail::Bad)?.bytes;
                out.push(format!("px rt {} {} {}", rt.w, rt.h, b * 8));
                for row in rt.bytes.chunks(rt.w as usize * b) {
                    out.push(row.iter().map(|x| format!("{x:02x}")).collect());
                }
            }
            "d9fvf" => {
                let now = (self.draw.fvf, self.draw.decl.take(), self.draw.decl_from);
                (self.draw.fvf, self.draw.decl, self.draw.decl_from) = set_fvf(n(op, 1)? as u32, now)?;
            }
            "d9getfvf" => {
                hit(clause::D3D9_FVF_GET);
                out.push(format!("getfvf {:08x}", self.draw.fvf));
            }
            "d9getdecl" => {
                hit(clause::D3D9_DECL_GET);
                match &self.draw.decl {
                    None => out.push("getdecl null".into()),
                    Some(e) => out.push(format!("getdecl {}{}", e.len(), e.iter().map(|(a, b)| format!(" {a:08x} {b:08x}")).collect::<String>())),
                }
            }
            "d9check" => {
                let (f, u, rtype) = (n(op, 1)?, n(op, 2)?, n(op, 3)?);
                let f = f as u32;
                let hr = check_hr(f, u, rtype, self.v8)?;
                if !matches!((u, rtype), (0, 3) | (1, 1) | (1, 3)) {
                    hit(clause::D3D9_FORMATS_USAGE);
                }
                out.push(format!("check {f} {u:x} {rtype} hr {hr:08x}"));
            }
            "d9tex" => {
                let (i, w, h, f) = (n(op, 1)? as usize, n(op, 2)? as i32, n(op, 3)? as i32, n(op, 4)? as u32);
                let taken = supported(f, false, self.v8);
                let fm = format(f).filter(|_| taken);
                match create_level(taken, fm.as_ref().is_some_and(|m| matches!(m.kind, Kind::Dxt(_))), fm.as_ref().map_or(0, |m| m.bytes), w, h) {
                    Ok(bytes) => self.draw.texs[i] = Some(Surface { w, h, fmt: f, bytes }),
                    Err(hr) => {
                        self.draw.texs[i] = None;
                        out.push(format!("tex failed {hr:08x}"));
                    }
                }
            }
            "d9texfill" => {
                let i = n(op, 1)? as usize;
                let seed = if t.get(2).is_some_and(|s| s == "ramp") { -1 } else { n(op, 2)? };
                let Some(tx) = &mut self.draw.texs[i] else {
                    out.push(format!("tex {i} none"));
                    return Ok(true);
                };
                tx.bytes = fill_bytes(&format(tx.fmt).ok_or(Fail::Bad)?, tx.w, tx.h, seed);
            }
            "d9palette" => {
                let b = fill_bytes(&format(50).ok_or(Fail::Bad)?, 1024, 1, n(op, 1)?);
                let mut e = Box::new([[0u8; 4]; 256]);
                for (i, c) in b.chunks(4).enumerate() {
                    e[i].copy_from_slice(c);
                }
                self.draw.palette = Some(e);
            }
            "d9settex" => {
                let s = n(op, 1)? as usize;
                let i = if t.get(2).is_some_and(|s| s == "none") { None } else { Some(n(op, 2)? as usize) };
                self.draw.stage_tex[s] = i.filter(|&i| self.draw.texs[i].is_some());
            }
            "d9scissor" => {
                self.draw.scissor = Some((n(op, 1)? as i32, n(op, 2)? as i32, n(op, 3)? as i32, n(op, 4)? as i32));
            }
            "d9draw" => {
                let (ty, nv) = (n(op, 1)? as u32, n(op, 2)? as usize);
                let words: Vec<u32> = (3..t.len()).map(|i| n(op, i).map(|v| v as u32)).collect::<Res<_>>()?;
                if nv == 0 || words.len() % nv != 0 {
                    return Err(Fail::Bad);
                }
                let addresses: Vec<u32> = (0..8u32).filter(|&st| self.draw.stage_tex[st as usize].is_some()).flat_map(|st| [self.samp(st, 1), self.samp(st, 2)]).collect();
                let palette_tex = self.draw.stage_tex.iter().flatten().any(|&i| self.draw.texs[i].as_ref().is_some_and(|t| matches!(t.fmt, 40 | 41)));
                match draw_gate(crate::model::arch() == 32, self.rs(171), &addresses, self.draw.palette.is_some(), palette_tex) {
                    DrawGate::BlendOpX86(op) => out.push(format!("draw x86 blendop {op}")),
                    DrawGate::AddressX86(a) => out.push(format!("draw x86 address {a}")),
                    DrawGate::NoPalette => out.push(format!("draw hr {INVALIDCALL:08x}")),
                    DrawGate::Draw => self.draw_prims(ty, nv, &words)?,
                }
            }
            "d9decl" => {
                let w: Vec<u32> = (1..t.len()).map(|i| n(op, i).map(|v| v as u32)).collect::<Res<_>>()?;
                if w.len() % 2 != 0 {
                    return Err(Fail::Bad);
                }
                let e: Vec<(u32, u32)> = w.chunks(2).map(|p| (p[0], p[1])).collect();
                hit(clause::D3D9_DECL_CREATE);
                if decl_ok(&e)? {
                    (self.draw.fvf, self.draw.decl, self.draw.decl_from) = set_decl(e);
                } else {
                    // (the declaration set before stays)
                    out.push("decl hr 80004005".into());
                }
            }
            "d9xform" => {
                let ty = n(op, 1)? as u32;
                let m: Vec<f32> = (2..18).map(|i| n(op, i).map(|v| f32::from_bits(v as u32))).collect::<Res<_>>()?;
                if !matches!(ty, 2 | 3 | 16..=23 | 256) {
                    // (the generators set only VIEW, PROJECTION, TEXTURE0..7, WORLD)
                    return Err(Fail::Bad);
                }
                hit(clause::D3D9_DRAW_XYZ);
                self.draw.xform.insert(ty, std::array::from_fn(|i| m[i]));
            }
            "d9viewport" => {
                let v: Vec<i64> = (1..5).map(|i| n(op, i)).collect::<Res<_>>()?;
                let (z0, z1) = (f32::from_bits(n(op, 5)? as u32), f32::from_bits(n(op, 6)? as u32));
                let Some(rt) = &self.draw.rt else { return Err(Fail::Bad) };
                if v[0] < 0 || v[1] < 0 || v[0] + v[2] > rt.w as i64 || v[1] + v[3] > rt.h as i64 {
                    // (the generators set no viewport past the target)
                    return Err(Fail::Bad);
                }
                self.draw.viewport = Some((v[0] as i32, v[1] as i32, v[2] as i32, v[3] as i32, z0, z1));
            }
            _ => return Err(Fail::Bad),
        }
        Ok(true)
    }
}

/// CreateVertexDeclaration's checks of elements (Stream | Offset << 16, Type | Method << 8 | Usage
/// << 16 | UsageIndex << 24): MEASURED (decl-one-*, decl-pair-*): an element of a type up to
/// FLOAT16_4 (16), method DEFAULT, usage up to DEPTH (12) and usage index under 16; TESSFACTOR only
/// a FLOAT1 of index 0; offsets multiples of 4
pub fn decl_ok(e: &[(u32, u32)]) -> Res<bool> {
    for &(a, b) in e {
        let (ty, method, usage, index) = (b & 255, b >> 8 & 255, b >> 16 & 255, b >> 24);
        if (a >> 16) % 4 != 0 || ty > 16 || method != 0 || usage > 12 || index >= 16 || usage == 8 && (ty != 0 || index != 0) {
            return Ok(false);
        }
    }
    // no usage and index twice; each stream's elements together; a POSITIONT of index 0 neither
    // with a POSITION of index 0 nor with more than one stream
    let keys: Vec<(u32, u32)> = e.iter().map(|&(_, b)| (b >> 16 & 255, b >> 24)).collect();
    let streams: Vec<u32> = e.iter().map(|&(a, _)| a & 0xffff).collect();
    let twice = keys.iter().enumerate().any(|(i, k)| keys[..i].contains(k));
    let apart = (1..streams.len()).any(|i| streams[i] != streams[i - 1] && streams[..i].contains(&streams[i]));
    let one_stream = streams.iter().all(|&s| s == streams[0]);
    let transformed = keys.contains(&(9, 0)) && (keys.contains(&(0, 0)) || !one_stream);
    Ok(!twice && !apart && !transformed)
}

/// one 8-bit channel blended (MEASURED, d3d9draw blend-*): the source and destination each
/// (value, whether its factor is ONE, the factor in 0..255). Each product x * f of two 8-bit
/// numbers is x * f * 257 >> 8 in 1/256ths, a little under x * f / 255 (x * ONE is x exactly,
/// d3d8draw blend-random-2); the two added or subtracted, rounded to 8 bits half up. With `exact`
/// (D3D9.BLEND.TIE: SRCALPHASAT, separate alpha, a target without alpha) the exact sum in
/// 255ths rounded to nearest. MIN and MAX ignore the factors
pub fn blend8(op: BlendOp, (x, xone, fs): (i64, bool, i64), (y, yone, fd): (i64, bool, i64), exact: bool) -> i64 {
    let prod = |v: i64, one: bool, f: i64| if one { v << 8 } else { v * f * 257 >> 8 };
    let (a, b) = (prod(x, xone, fs), prod(y, yone, fd));
    let n = match op {
        BlendOp::Add => (a + b, x * fs + y * fd),
        BlendOp::Sub => (a - b, x * fs - y * fd),
        BlendOp::RevSub => (b - a, y * fd - x * fs),
        BlendOp::Min => return x.min(y),
        BlendOp::Max => return x.max(y),
    };
    if exact { ((2 * n.1.max(0) + 255) / 510).min(255) } else { (n.0.max(0) + 128 >> 8).min(255) }
}

/// D3DCMPFUNC
pub fn cmp(f: u32, a: u64, b: u64) -> bool {
    match f {
        1 => false,
        2 => a < b,
        3 => a == b,
        4 => a <= b,
        5 => a > b,
        6 => a != b,
        7 => a >= b,
        _ => true,
    }
}

/// D3DSTENCILOP on an 8-bit stencil value
pub fn stencil_op(op: u32, s: u8, refv: u8) -> u8 {
    match op {
        2 => 0,
        3 => refv,
        4 => s.saturating_add(1),
        5 => s.saturating_sub(1),
        6 => !s,
        7 => s.wrapping_add(1),
        8 => s.wrapping_sub(1),
        _ => s,
    }
}

/// D3D9.STENCIL: the new stencil value stored through the write mask (the other bits kept)
pub fn stencil_write(s: u8, ns: u8, wmask: u8) -> u8 {
    (s & !wmask) | (ns & wmask)
}

/// D3DTEXTUREADDRESS: a texel index (any integer) into 0..n, None for the border
pub fn address(mode: u32, i: i64, n: i64) -> Res<Option<i64>> {
    hit(clause::D3D9_TEX_ADDRESS);
    Ok(match mode {
        1 => Some(i.rem_euclid(n)),
        2 => {
            let k = i.rem_euclid(2 * n);
            Some(if k < n { k } else { 2 * n - 1 - k })
        }
        3 => Some(i.clamp(0, n - 1)),
        4 => (0..n).contains(&i).then_some(i),
        5 => Some(if i < 0 { -i - 1 } else { i }.clamp(0, n - 1)),
        // (MEASURED x64, address-0, address-6: a mode outside the enumeration as MIRRORONCE)
        _ => {
            hit(clause::D3D9_TEX_ADDRESS_BAD);
            Some(if i < 0 { -i - 1 } else { i }.clamp(0, n - 1))
        }
    })
}

impl D3d9 {
    fn texel(&self, st: u32, tx: &Surface, x: i64, y: i64, mu: u32, mv: u32) -> Res<[f64; 4]> {
        let (Some(x), Some(y)) = (address(mu, x, tx.w as i64)?, address(mv, y, tx.h as i64)?) else {
            hit(clause::D3D9_TEX_BORDER);
            return Ok(argb(self.samp(st, 4)));
        };
        let fm = format(tx.fmt).ok_or(Fail::Bad)?;
        match texel_kind(tx.fmt, &fm) {
            Some(TexelKind::Channels) => {}
            Some(TexelKind::Dxt(n)) => return dxt_texel(n, &tx.bytes, tx.w, x as i32, y as i32),
            Some(TexelKind::Palette) => {
                // (MEASURED palette-*: the entry's color; P8's alpha the entry's flags, A8P8's its own)
                let p = tx.px(x as i32, y as i32)?;
                let e = self.draw.palette.as_ref().ok_or(Fail::Bad)?[p[0] as usize];
                return Ok(palette_texel(tx.fmt, p.get(1).copied().unwrap_or(0), e).map(|c| f64::from(c) / 255.0));
            }
            // (none taken: every format WARP takes is decoded above, law every_taken_format_is_decoded)
            _ => return Err(Fail::Bad),
        }
        hit(clause::D3D9_TEX_DECODE);
        Ok(to_rgba(tx.fmt, &decode(&fm, tx.px(x as i32, y as i32)?)))
    }

    /// the bilinear filter at texel coordinates (x, y)
    fn bilinear(&self, st: u32, tx: &Surface, x: f64, y: f64, mu: u32, mv: u32) -> Res<[f64; 4]> {
        // (MEASURED, filter-*, filter2-*, filter-uv-*: weights in 32nds of a texel taken
        // down, the weighted sum of 8-bit texels truncated)
        Ok(match self.bilinear_sum(st, tx, x, y, mu, mv)? {
            Weighted::Bytes(v) => v.map(bilinear_unit),
            Weighted::Float(c) => c,
        })
    }

    /// the bilinear filter's weighted sum at texel coordinates (x, y): of 8-bit texels in
    /// 1/BILERP_ONE, before it is truncated, or of other texels' values
    fn bilinear_sum(&self, st: u32, tx: &Surface, x: f64, y: f64, mu: u32, mv: u32) -> Res<Weighted> {
        let ((x0, fx), (y0, fy)) = (bilinear_at(x), bilinear_at(y));
        let t = [self.texel(st, tx, x0, y0, mu, mv)?, self.texel(st, tx, x0 + 1, y0, mu, mv)?, self.texel(st, tx, x0, y0 + 1, mu, mv)?, self.texel(st, tx, x0 + 1, y0 + 1, mu, mv)?];
        let w = bilinear_weights(fx, fy);
        let fm = format(tx.fmt).ok_or(Fail::Bad)?;
        let bytes = fm.kind == Kind::Int && fm.ch.iter().all(|c| c.bits == 8 && !matches!(c.c, b'u' | b'v' | b'w' | b'q'));
        Ok(if bytes {
            Weighted::Bytes(std::array::from_fn(|k| bilinear_bytes(t.map(|c| unorm(c[k], 8)), w)))
        } else {
            Weighted::Float(std::array::from_fn(|k| bilinear_float(t.map(|c| c[k]), w)))
        })
    }

    /// a stage's texture at (u, v)
    fn sample(&self, st: u32, tx: &Surface, u: f64, v: f64) -> Res<[f64; 4]> {
        let (mu, mv) = (self.samp(st, 1), self.samp(st, 2));
        // (MEASURED filter2-0-32, filter2-1-32: a texture of one level magnifies everywhere,
        // minified or not: the level of detail is clamped to the texture's levels, 0, before it
        // picks the filter, so MINFILTER takes no part)
        let filter = match sample_filter(self.samp(st, 5), self.samp(st, 6), self.v8) {
            TexFilter::Anisotropic => {
                // (MEASURED filter2-5-4, d3d8draw filter2-2-4: linear while no axis of the
                // footprint is longer than a texel, aniso-3, -11, -18, -21, -23, -31: and with
                // MAXANISOTROPY 1, aniso-8: and with axes of one length)
                // (the clean-room notes: WARP's footprint in f32, one rounding an operation)
                let ((dudx, dudy), (dvdx, dvdy)) = self.draw.duv[self.tex_set(st).at()];
                let f = |a: f64| a as f32;
                let (w, h) = (tx.w as f32, tx.h as f32);
                let (r, axis, n) = aniso_footprint([f(dudx), f(dvdx)], [f(dudy), f(dvdy)], (w, h), self.samp(st, 10));
                if r > 1.0 {
                    let (mu, mv) = (self.samp(st, 1), self.samp(st, 2));
                    // (each tap's weighted sum truncated to a 64th of 1/255; to 32nds or 128ths,
                    // or rounded, misses pixels)
                    const TAP: i64 = BILERP_ONE / 64;
                    let tap = |(tu, tv): (f32, f32)| -> Res<[i64; 4]> {
                        let Weighted::Bytes(t) = self.bilinear_sum(st, tx, f64::from(tu) * f64::from(w), f64::from(tv) * f64::from(h), mu, mv)? else {
                            // (texels other than 8-bit ones: no case)
                            return Err(Fail::Unmodeled(open::D3D9_TEX_FILTER));
                        };
                        Ok(t.map(|x| x / TAP))
                    };
                    let vals: Vec<[i64; 4]> = aniso_taps((f(u), f(v)), r, axis, n, (w, h)).into_iter().map(tap).collect::<Res<_>>()?;
                    let s = pair_average(vals).ok_or(Fail::Bad)?;
                    return Ok(s.map(|s| ((s / 64) as f32 / 255.0) as f64));
                }
                TexFilter::Linear
            }
            f => f,
        };
        let (x, y) = (u * tx.w as f64, v * tx.h as f64);
        match filter {
            TexFilter::Point => {
                hit(clause::D3D9_TEX_POINT);
                self.texel(st, tx, point_at(x), point_at(y), mu, mv)
            }
            TexFilter::Linear => {
                hit(clause::D3D9_TEX_LINEAR);
                self.bilinear(st, tx, x, y, mu, mv)
            }
            TexFilter::Anisotropic | TexFilter::Quad | TexFilter::None | TexFilter::Other => unreachable!("taken as linear or point above"),
        }
    }

    /// a stage argument (D3DTA) as rgba
    fn arg(&self, st: u32, a: u32, r: &Regs, tex: Option<[f64; 4]>) -> Res<[f64; 4]> {
        let regs = ArgRegs { diffuse: r.diffuse, current: r.current, specular: r.specular, temp: r.temp, tex, factor: self.rs(60), constant: self.tss(st, 32) };
        stage_arg(a, &regs, self.v8)
    }

    /// a stage's states as stage_plan reads them
    fn stage_desc(&self, st: u32) -> StageDesc {
        let a = |t: u32| self.tss(st, t);
        StageDesc {
            cop: a(1),
            aop: a(4),
            has_tex: self.draw.stage_tex.get(st as usize).is_some_and(Option::is_some),
            reads_tex: reads_texture(a(1), a(4), [a(2), a(3), a(26), a(5), a(6), a(27)]),
            reads_cur: reads_current(a(1), [a(2), a(3), a(26)]),
        }
    }

    /// the texture stages (D3DTEXTUREOP) as stage_plan orders them
    fn stages(&self, diffuse: [f64; 4], specular: [f64; 4], uv: [[f64; 2]; NSET]) -> Res<[f64; 4]> {
        let mut r = Regs { diffuse, specular, current: diffuse, temp: [0.0; 4] };
        let desc: [StageDesc; 8] = std::array::from_fn(|st| self.stage_desc(st as u32));
        // (the texture each stage sampled, and CURRENT as each stage found it)
        let mut texs: [Option<[f64; 4]>; 8] = [None; 8];
        let mut found = [diffuse; 8];
        for step in stage_plan(&desc) {
            let (Step::Run { st, moved, .. } | Step::Defer { st, moved } | Step::BumpLast { st, moved, .. } | Step::Off { st, moved, .. }) = step;
            let k = st as usize;
            found[k] = r.current;
            let tex = match self.draw.stage_tex[k] {
                Some(i) => {
                    let [mut u, mut v] = uv[self.tex_set(st).at()];
                    let mut lum = None;
                    // (the bump mapping stage before: its texel, 0 without a texture (bump-2), and kind)
                    if let Some(kind) = moved.then(|| Bump::of(desc[k - 1].cop)).flatten() {
                        // (MEASURED bump-*: the coordinates moved by BUMPENVMAT times (du, dv);
                        // LUMINANCE: the texture's color times L LSCALE + LOFFSET, clamped to 0..1,
                        // L 1 where the format has none)
                        hit(clause::D3D9_STAGE_BUMP);
                        let t = texs[k - 1].unwrap_or([0.0; 4]);
                        let bs = st - 1;
                        let m = |k: u32| f32::from_bits(self.tss(bs, k)) as f64;
                        (u, v) = bump_uv((u, v), [m(7), m(8), m(9), m(10)], (t[0], t[1]));
                        if kind == Bump::Luminance {
                            lum = Some(bump_luminance(t[2], m(22), m(23)));
                        }
                    }
                    let mut t = self.sample(st, self.draw.texs[i].as_ref().ok_or(Fail::Empty)?, u, v)?;
                    if let Some(l) = lum {
                        for c in t.iter_mut().take(3) {
                            *c = bump_scale(*c, l);
                        }
                    }
                    Some(t)
                }
                None => None,
            };
            texs[k] = tex;
            let (cop, aop) = (desc[k].cop, desc[k].aop);
            let out = match step {
                // (MEASURED bump-chain-2, -8, -11, bump-late-*: the stages off from s, CURRENT as
                // it found it)
                Step::Off { s, .. } => return Ok(found[s as usize]),
                // (MEASURED bump-chain-4, -5, -6, -9, -10: a bump mapping stage followed by a
                // stage writes nothing)
                Step::Defer { .. } => continue,
                // (MEASURED top-color-BUMPENVMAP*, stage1-BUMPENVMAP*, bump-*: the last stage
                // gives the texture the stage before it sampled, 0 where none did)
                Step::BumpLast { from, .. } => {
                    hit(clause::D3D9_STAGE_BUMP);
                    from.and_then(|p| texs.get(p as usize).copied().flatten()).unwrap_or([0.0; 4])
                }
                Step::Run { alpha, premod, .. } => {
                    let mut out = match alpha {
                        AlphaRule::Op(op) => self.stage(st, cop, op, &r, tex)?,
                        // (ALPHAOP DISABLE: the color operation's stage, its alpha the one given
                        // or 1, alphadisable-*)
                        AlphaRule::Keep => {
                            let mut out = self.stage(st, cop, 2, &r, tex)?;
                            out[3] = r.current[3];
                            out
                        }
                        AlphaRule::One => {
                            let mut out = self.stage(st, cop, 2, &r, tex)?;
                            out[3] = 1.0;
                            out
                        }
                    };
                    if premod {
                        // (MEASURED premod-chain-*: PREMODULATE gives its first argument times
                        // the next stage's texture, in f32, where the next stage is enabled and
                        // has one)
                        hit(clause::D3D9_STAGE_BUMP);
                        let i = self.draw.stage_tex[k + 1].ok_or(Fail::Bad)?;
                        let [nu, nv] = uv[self.tex_set(st + 1).at()];
                        let nt = self.sample(st + 1, self.draw.texs[i].as_ref().ok_or(Fail::Empty)?, nu, nv)?;
                        for c in 0..4 {
                            if (c < 3 && cop == 17) || (c == 3 && aop == 17) {
                                out[c] = premodulate(out[c], nt[c]);
                            }
                        }
                    }
                    out
                }
            };
            match self.tss(st, 28) {
                1 => r.current = out,
                5 if !self.v8 => {
                    hit(clause::D3D9_STAGE_ARG);
                    r.temp = out;
                }
                // (RESULTARG is CURRENT or TEMP, Direct3D 9 only: the generators write no other)
                _ => return Err(Fail::Bad),
            }
        }
        Ok(r.current)
    }

    /// the set of texture coordinates a stage reads (MEASURED tci-multi-*: each stage its own
    /// TEXCOORDINDEX's, D3D9.STAGE.TCI)
    fn tex_set(&self, st: u32) -> TexSet {
        let key = self.set_key(st);
        self.draw.slots.iter().position(|k| *k == key).and_then(|i| TexSet::ALL.get(i).copied()).unwrap_or(TexSet::First)
    }

    /// what tells apart the set a stage reads (SetKey)
    fn set_key(&self, st: u32) -> SetKey {
        SetKey { tci: self.tss(st, 11), xform: match self.tss(st, 24) { 0 => None, t => Some((t, st)) } }
    }

    /// one stage's color and alpha operations
    fn stage(&self, st: u32, cop: u32, aop: u32, r: &Regs, tex: Option<[f64; 4]>) -> Res<[f64; 4]> {
        let mut out = [0.0; 4];
        for (k, (op, a0, a1, a2)) in [(cop, 26, 2, 3), (aop, 27, 5, 6)].into_iter().enumerate() {
            // (only the arguments the operation reads: SELECTARG1 reads no second one)
            let get = |used: bool, a: u32| if used { self.arg(st, self.tss(st, a), r, tex) } else { Ok([0.0; 4]) };
            let a0 = get(matches!(op, 25 | 26), a0)?;
            let a1 = get(op != 3, a1)?;
            let a2 = get(op != 2, a2)?;
            let chans: &[usize] = if k == 0 { &[0, 1, 2] } else { &[3] };
            if k == 0 && op == 24 {
                // DOTPRODUCT3: into every channel, alpha too
                hit(clause::D3D9_STAGE_OP);
                let d = 4.0 * (0..3).map(|i| (a1[i] - 0.5) * (a2[i] - 0.5)).sum::<f64>();
                return Ok([d.clamp(0.0, 1.0); 4]);
            }
            // (MEASURED: in f32)
            let (a0, a1, a2) = (a0.map(|x| x as f32), a1.map(|x| x as f32), a2.map(|x| x as f32));
            let da = r.diffuse[3] as f32;
            let ca = r.current[3] as f32;
            for &i in chans {
                let alpha = k == 1;
                let ta = tex.unwrap_or([1.0; 4])[3] as f32;
                let fa = argb(self.rs(60))[3] as f32;
                let v = stage_value(StageIn { op, st, alpha, a0: a0[i], a1: a1[i], a2: a2[i], a1a: a1[3], da, ca, ta, fa, cur: r.current[i] as f32 })?;
                hit(clause::D3D9_STAGE_OP);
                out[i] = v.clamp(0.0, 1.0) as f64;
            }
        }
        Ok(out)
    }

    fn draw_prims(&mut self, ty: u32, nv: usize, words: &[u32]) -> Res<()> {
        let fvf = self.draw.fvf;
        // XYZRHW (pretransformed) or XYZ (transformed by the device)
        let pos = match fvf & 0x400e {
            4 => 4,
            2 => 3,
            // (the generators draw with XYZ and XYZRHW positions only)
            _ => return Err(Fail::Bad),
        };
        let ntex = (fvf >> 8 & 0xf) as usize;
        let tsize = |i: usize| [2, 3, 4, 1][(fvf >> (16 + 2 * i) & 3) as usize];
        let size = pos + (fvf >> 4 & 1) as usize * 3 + (fvf >> 5 & 1) as usize + (fvf >> 6 & 1) as usize + (fvf >> 7 & 1) as usize + (0..ntex).map(tsize).sum::<usize>();
        // (MEASURED texcoordindex-*: a stage's TEXCOORDINDEX picks the vertex's set by its low
        // word, (0, 0) past the sets; with XYZRHW vertices the generation mode in its high word
        // is ignored)
        // (MEASURED tci-multi-*, tci-three-*: each enabled stage reads its own set, those of
        // the same TEXCOORDINDEX and texture matrix one; D3D9.STAGE.TCI)
        const STAGES: [u32; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
        let slots = tci_slots(STAGES.map(|st| self.set_key(st)), STAGES.map(|st| self.tss(st, 1))).ok_or(Fail::Bad)?;
        let keys: [SetKey; NSET] = std::array::from_fn(|i| slots.get(i).copied().unwrap_or(slots[0]));
        self.draw.slots = slots;
        let tcis = keys.map(|k| k.tci);
        let sets = tcis.map(|t| (t & 0xffff) as usize);
        if tcis.iter().any(|&t| t != 0) {
            hit(clause::D3D9_STAGE_TCI);
        }
        // (MEASURED tci-gen2-4, -5, tci-ttf-*: TEXTURETRANSFORMFLAGS takes a stage's set of
        // untransformed vertices through its texture matrix, D3D9.STAGE.TEXXFORM; PROJECTED
        // divides by the count's last; pretransformed vertices' sets as they are)
        if pos == 3 && keys.iter().any(|k| k.xform.is_some_and(|(t, _)| !matches!(t & 0xff, 1..=4) || t & !0x1ff != 0)) {
            // (the generators set counts 1 .. 4 and PROJECTED only)
            return Err(Fail::Bad);
        }
        let projected = pos == 3 && keys.iter().any(|k| k.xform.is_some_and(|(t, _)| t & 0x100 != 0 && t & 0xff != 1));
        if projected {
            // (MEASURED tci-proj-*: projected coordinates under LINEAR and ANISOTROPIC filters,
            // their footprint from the 2 x 2 quad, D3D9.STAGE.PROJFOOT)
            hit(clause::D3D9_STAGE_PROJFOOT);
        }
        let sizes: Vec<usize> = (0..ntex).map(tsize).collect();
        let vs: Vec<Vtx> = vertex_words(words, nv, size)
            .ok_or(Fail::Bad)?
            .into_iter()
            .map(|w| -> Res<Vtx> {
                let mut i = pos + (fvf >> 4 & 1) as usize * 3 + (fvf >> 5 & 1) as usize;
                let mut take = |on: bool| {
                    let v = if on { Some(w[i]) } else { None };
                    i += on as usize;
                    v
                };
                let diffuse = argb(take(fvf & 0x40 != 0).unwrap_or(0xffff_ffff));
                let specular = argb(take(fvf & 0x80 != 0).unwrap_or(0));
                // (the sets the stages' TEXCOORDINDEX name, D3D9.STAGE.TCI: (0, 0) past the
                // vertex's sets; as a row vector for a texture matrix its coordinates, then 1,
                // then 0)
                let tex = w.get(i..).ok_or(Fail::Bad)?;
                let [r0, r1, r2] = sets.map(|set| coord_row(tex, &sizes, set));
                let rows = [r0.ok_or(Fail::Bad)?, r1.ok_or(Fail::Bad)?, r2.ok_or(Fail::Bad)?];
                let passed: [[f32; 4]; NSET] = rows.map(|r| r.1);
                if pos == 3 {
                    let p = [w[0], w[1], w[2]].map(f32::from_bits);
                    // (MEASURED tci-xyz-*, tci-gen-*: coordinates generated from the camera-space
                    // position P (world then view) and normal N (0 where the vertex has none):
                    // CAMERASPACENORMAL (Nx, Ny); CAMERASPACEPOSITION (Px, Py);
                    // CAMERASPACEREFLECTIONVECTOR R = 2 (E.N) N - E, E = -P / |P| (LOCALVIEWER);
                    // SPHEREMAP from R, m = 2 sqrt(Rx^2 + Ry^2 + (Rz - 1)^2), (Rx / m + 1/2,
                    // Ry / m + 1/2); D3D9.STAGE.TCIGEN)
                    let uv: [Res<[f64; 3]>; NSET] = std::array::from_fn(|k| {
                        let v: [f32; 4] = match TexGen::of(tcis[k], self.v8) {
                            TexGen::Passthru => passed[k],
                            texgen => {
                                hit(clause::D3D9_STAGE_TCIGEN);
                                let c = self.camera_space(p);
                                // (the vertex's normal through the inverse transpose of world
                                // then view, normalized under NORMALIZENORMALS; none: 0)
                                let nn = if fvf & 0x10 != 0 { self.camera_normal([w[pos], w[pos + 1], w[pos + 2]].map(f32::from_bits)) } else { [0.0f32; 3] };
                                tex_gen(texgen, c, nn, self.rs(142) != 0)
                            }
                        };
                        match keys[k].xform {
                            Some((t, st)) => self.ttf(t, st, v),
                            None => Ok([v[0] as f64, v[1] as f64, 1.0]),
                        }
                    });
                    let [u0, u1, u2] = uv;
                    let uv = [u0?, u1?, u2?];
                    let (x, y, z, rhw, clip) = self.transform(p);
                    return Ok(Vtx { x, y, z: z as f64, rhw: rhw as f64, diffuse, specular, uv, clip: Some(clip) });
                }
                let uv = passed.map(|v| [v[0] as f64, v[1] as f64, 1.0]);
                Ok(Vtx { x: f32::from_bits(w[0]), y: f32::from_bits(w[1]), z: fl(w[2]), rhw: fl(w[3]), diffuse, specular, uv, clip: None })
            })
            .collect::<Res<_>>()?;
        if ty == 1 {
            return self.points(&vs);
        }
        match assemble(ty, nv).ok_or(Fail::Bad)? {
            Prims::Segs(segs) => {
                for (i, j) in segs {
                    self.line(vs[i], vs[j])?;
                }
            }
            Prims::Tris(tris) => {
                for t in tris {
                    self.triangle(vs[t[0]], vs[t[1]], vs[t[2]])?;
                }
            }
        }
        Ok(())
    }

    /// an untransformed position through WORLD, VIEW, PROJECTION and the viewport: screen x, y,
    /// z and rhw (D3D9.DRAW.XYZ)
    /// the viewport: SetViewport's, or the whole target
    fn viewport(&self) -> (i32, i32, i32, i32, f32, f32) {
        let (rw, rh) = self.draw.rt.as_ref().map_or((0, 0), |r| (r.w, r.h));
        self.draw.viewport.unwrap_or((0, 0, rw, rh, 0.0, 1.0))
    }

    /// a position in camera space: times the world matrix, then the view one, in f32
    fn camera_space(&self, p: [f32; 3]) -> [f32; 4] {
        let id: [f32; 16] = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let m = |t: u32| *self.draw.xform.get(&t).unwrap_or(&id);
        camera_space(p, m(256), m(2))
    }

    /// a normal in camera space (camera_normal), normalized under NORMALIZENORMALS
    fn camera_normal(&self, n: [f32; 3]) -> [f32; 3] {
        let id: [f32; 16] = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let m = |t: u32| *self.draw.xform.get(&t).unwrap_or(&id);
        camera_normal(n, m(256), m(2), self.rs(143) != 0)
    }

    /// a set through a stage's TEXTURETRANSFORMFLAGS (MEASURED tci-gen2-4, -5, tci-ttf-*):
    /// COUNT2 .. 4 the row vector times the stage's texture matrix (TEXTUREn, 16 + n), the
    /// first two taken, PROJECTED their q the count's last (but COUNT2's on Direct3D 8);
    /// COUNT1 the set as it is on Direct3D 9, (the matrix's first, 0) on Direct3D 8; as
    /// (s, t, q), q divided per pixel
    fn ttf(&self, ttf: u32, st: u32, v: [f32; 4]) -> Res<[f64; 3]> {
        hit(clause::D3D9_STAGE_TEXXFORM);
        let id: [f32; 16] = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let a = *self.draw.xform.get(&(16 + st)).unwrap_or(&id);
        tex_transform(ttf, self.v8, v, a).ok_or(Fail::Bad)
    }

    fn transform(&self, p: [f32; 3]) -> (f32, f32, f32, f32, [f32; 4]) {
        let id: [f32; 16] = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let m = |t: u32| *self.draw.xform.get(&t).unwrap_or(&id);
        transform_xyz(p, [m(256), m(2), m(3)], self.viewport())
    }

    /// a line of pretransformed vertices: as WARP's Direct3D 10 rasterizer draws them
    /// (D3DRASTER.LINE: the diamond-exit rule over the snapped ends, the diamond's inside, lower
    /// edges and bottom corner, a y-major line's right corner too; the colors a plane over the
    /// major axis), in Direct3D 9's pixel centers (MEASURED d3d9draw, d3d8draw line-*,
    /// points-lines-*)
    fn line(&mut self, a: Vtx, b: Vtx) -> Res<()> {
        let Some(rt) = &self.draw.rt else {
            hit(clause::D3D9_CLEAR_BACKBUFFER);
            return Ok(());
        };
        let (w, h) = (rt.w, rt.h);
        if a.clip.is_some() || a.rhw != 1.0 || b.rhw != 1.0 {
            // (the generators draw lines of pretransformed vertices of rhw 1 only)
            return Err(Fail::Bad);
        }
        let Some((la, lb, ra, rb)) = line_cut((a.x, a.y), (b.x, b.y), w, h, self.v8) else { return Ok(()) };
        let (l, r) = (Seg9 { a: la, b: lb }, Seg9 { a: ra, b: rb });
        hit(clause::D3D9_DRAW_LINES);
        // (WARP's aliased line in Direct3D 10's coordinates, half a pixel on: D3D9.DRAW.LINES,
        // D3D9.DRAW.LINEEND, its ties D3D9.DRAW.POINTSLINES)
        hit(clause::D3D9_DRAW_LINEEND);
        let d10 = |p: (i128, i128)| i32::try_from(p.0 + HALF).and_then(|x| Ok((x, i32::try_from(p.1 + HALF)?)));
        let (Ok(pa), Ok(pb)) = (d10(r.a), d10(r.b)) else { return Err(Fail::Bad) };
        let (lit, tie) = super::warpline::coverage(pa, pb, w, h);
        if tie {
            hit(clause::D3D9_DRAW_POINTSLINES);
        }
        let ymaj = (r.b.1 - r.a.1).abs() > (r.b.0 - r.a.0).abs();
        let flat = self.rs(9) == 1;
        let (at, bt) = (a.attrs(), b.attrs());
        for (px, py) in lit {
            // (the plane over the major axis in Direct3D 10's coordinates, in f32)
            let (pa, pb, c) = if ymaj { (l.a.1, l.b.1, py as f32 + 0.5) } else { (l.a.0, l.b.0, px as f32 + 0.5) };
            let (pa, pb) = (pa as f32 / 256.0 + 0.5, pb as f32 / 256.0 + 0.5);
            let g = 1.0 / (pb - pa);
            let v: [f64; ATTRS] = std::array::from_fn(|k| {
                let grad = (bt[k] - at[k]) * g;
                ((at[k] - grad * pa) + grad * c) as f64
            });
            let (diffuse, specular) = if flat { (a.diffuse, a.specular) } else { (std::array::from_fn(|k| v[1 + k]), std::array::from_fn(|k| v[5 + k])) };
            self.pixel(px, py, v[0], diffuse, specular, st_of(&v))?;
        }
        Ok(())
    }

    /// a point list: each point the one pixel whose area holds it, (x, y) taken to the pixel
    /// whose center is nearest, halves down (MEASURED points-lines-0, -3)
    fn points(&mut self, vs: &[Vtx]) -> Res<()> {
        let Some(rt) = &self.draw.rt else {
            hit(clause::D3D9_CLEAR_BACKBUFFER);
            return Ok(());
        };
        let (w, h) = (rt.w, rt.h);
        hit(clause::D3D9_DRAW_POINTS);
        for v in vs {
            // (a point's attributes are its vertex's: nothing to divide by rhw)
            let (x, y) = point_pixel(v.x, v.y);
            if (0..w as i64).contains(&x) && (0..h as i64).contains(&y) {
                self.pixel(x as i32, y as i32, v.z, v.diffuse, v.specular, v.uv.map(|t| [t[0] / t[2], t[1] / t[2]]))?;
            }
        }
        Ok(())
    }

    fn triangle(&mut self, a: Vtx, b: Vtx, c: Vtx) -> Res<()> {
        let Some(rt) = &self.draw.rt else {
            hit(clause::D3D9_CLEAR_BACKBUFFER);
            return Ok(());
        };
        let (w, h) = (rt.w, rt.h);
        // (MEASURED rhw-*: perspective-correct attributes where an rhw is not 1)
        let persp = [a, b, c].iter().any(|v| v.rhw != 1.0);
        let vs = [a, b, c];
        // the winding (culling) of the vertices as given
        let area = cross([(a.x as f64, a.y as f64), (b.x as f64, b.y as f64), (c.x as f64, c.y as f64)]);
        hit(clause::D3D9_CULL);
        let winding = if area > 0.0 { Winding::Cw } else if area < 0.0 { Winding::Ccw } else { Winding::Flat };
        if culls(self.rs(22), crate::model::arch() == 32, winding) {
            return Ok(());
        }
        if area == 0.0 {
            return Ok(());
        }
        self.draw.ccw = area < 0.0;
        // (every vertex's clip space position: untransformed vertices)
        let clips: Option<Vec<[f32; 4]>> = vs.iter().map(|v| v.clip).collect();
        let xyz = clips.is_some();
        if let Some(clips) = clips.filter(|_| !self.draw.in_piece) {
            // (untransformed vertices: clipped in clip space to the viewport's pixels, the planes
            // half a pixel in, x = vx - 0.5 .. vx + w - 0.5 (as pretransformed vertices are,
            // D3D9.RASTER.CLIP), each new vertex a + (b - a) t; the polygon cut into a fan)
            let (vx, vy, vw, vh, ..) = self.viewport();
            let poly: Vec<([f32; 4], [f32; ATTRS])> = vs.iter().zip(&clips).map(|(v, c)| (*c, v.attrs())).collect();
            if let Some(poly) = clip_xyz(&poly, vw, vh) {
                let vtx = |(q, at): ([f32; 4], [f32; ATTRS])| {
                    let (x, y, rhw, clip) = xyz_vertex(q, (vx, vy, vw, vh));
                    Vtx {
                        x,
                        y,
                        z: at[0] as f64,
                        rhw: rhw as f64,
                        diffuse: std::array::from_fn(|k| at[1 + k] as f64),
                        specular: std::array::from_fn(|k| at[5 + k] as f64),
                        uv: std::array::from_fn(|i| [at[UV_AT[i]] as f64, at[UV_AT[i] + 1] as f64, at[Q_AT[i]] as f64]),
                        clip: Some(clip),
                    }
                };
                let flat = (a.diffuse, a.specular);
                self.draw.in_piece = true;
                let mut r = Ok(());
                for i in 1..poly.len().saturating_sub(1) {
                    let (mut p0, p1, p2) = (vtx(poly[0]), vtx(poly[i]), vtx(poly[i + 1]));
                    if self.rs(9) == 1 {
                        // (flat shading: the first vertex's colors still)
                        (p0.diffuse, p0.specular) = flat;
                    }
                    r = r.and(self.triangle(p0, p1, p2));
                }
                self.draw.in_piece = false;
                return r;
            }
        }
        // which triangles are clipped: MEASURED (d3d9draw raster-17: a vertex at y = 16 is not;
        // raster-*, shade-clip-*: one past -0.5 or past the size is)
        let pts = [(a.x, a.y), (b.x, b.y), (c.x, c.y)];
        let within = |v: &(f32, f32), lo: f32, hi: f32| (lo..=w as f32 + hi).contains(&v.0) && (lo..=h as f32 + hi).contains(&v.1);
        let clipped = !xyz && !pts.iter().all(|v| within(v, -0.5, 0.0));
        // (MEASURED d3d8draw raster-*, clip-decide-*, shade-clip-*: Direct3D 8 clips as Direct3D
        // 9 does, but with a vertex at the target's width (height) itself it clips the right
        // (bottom) side at the size, not half a pixel in: raster-77, -96 and 17 more,
        // D3D9.RASTER.CLIP8SIDE)
        let (x1, y1, sides) = clip_bounds(pts, w, h, self.v8, persp);
        let flat = self.rs(9) == 1;
        let mut clipr = (0, 0, w, h);
        if !self.v8 && self.rs(174) != 0 {
            hit(clause::D3D9_SCISSOR);
            if let Some((x0, y0, x1, y1)) = self.draw.scissor {
                clipr = (x0.max(0), y0.max(0), x1.min(w), y1.min(h));
            }
        }
        let corners = vs.map(|v| {
            // (the attributes times rhw, interpolated in screen space, then divided by the
            // interpolated rhw)
            PolyVtx { x: v.x, y: v.y, at: if persp { rhw_attrs(v.attrs()) } else { v.attrs() } }
        });
        let snap = |v: &PolyVtx| (snap256(v.x), snap256(v.y));
        // the pieces drawn: the triangle itself, or the clipped polygon's vertices snapped to
        // 1/256 pixel and cut into a strip (MEASURED: d3d9draw shade-clip-*, d3d8draw prims-3,
        // shade-9: the polygon as clipped to x = -0.5, size - 0.5, y = -0.5, size - 0.5 in turn,
        // its vertices v1 v2 v0 v3 v(n-1) v4 ..., each piece interpolated over its own snapped
        // vertices)
        let pieces: Vec<[PolyVtx; 3]> = if clipped {
            hit(clause::D3D9_RASTER_CLIP);
            // (the triangle turned to start at its vertex of least x, then least y: MEASURED,
            // clip-decide-*, which diagonal cuts a quadrilateral)
            let first = least_x(&corners);
            let turned: [PolyVtx; 3] = std::array::from_fn(|k| corners[(first + k) % 3]);
            let poly: Vec<PolyVtx> = clip_polygon(&turned, x1, y1, sides)
                .iter()
                .map(|v| {
                    let (x, y) = snap(v);
                    PolyVtx { x: x as f32 / 256.0, y: y as f32 / 256.0, at: v.at }
                })
                .collect();
            strip(poly.len()).into_iter().map(|t| t.map(|k| poly[k])).collect()
        } else {
            // (MEASURED filter2-1-32 on both executors: an unclipped triangle is interpolated from
            // its vertex of least x, then least y, too; the f32 texture coordinates at a 32nd's edge
            // tell which)
            vec![std::array::from_fn(|k| corners[(least_x(&corners) + k) % 3])]
        };
        // (untransformed vertices: the attributes as WARP's Direct3D 10 rasterizer sets them up,
        // D3DRASTER.INTERP)
        let planes = match vs.map(|v| v.clip) {
            [Some(_), Some(_), Some(_)] => Some(Planes::new(&vs, &snap)),
            _ => None,
        };
        hit(clause::D3D9_RASTER_TOPLEFT);
        for piece in pieces {
            let mut covered = vec![false; (w * h) as usize];
            cover(piece.map(|v| snap(&v)), w, h, &mut covered);
            // the attributes: MEASURED (Gouraud shading, d3d9draw shade-*, clip-decide-*, filter2-*
            // on both executors, every channel value): in f32, positions with the half pixel added
            // (D3D10's pixel centers), each attribute's gradients over the area and then
            // v0 + d/dy (y - y0) + d/dx (x - x0), in that order
            let (x, y) = (piece.map(|v| v.x + 0.5), piece.map(|v| v.y + 0.5));
            let grads: [(f32, f32); ATTRS] = std::array::from_fn(|k| grad9(x, y, piece.map(|v| v.at[k])));
            self.draw.duv = UV_AT.map(|k| ((grads[k].0 as f64, grads[k].1 as f64), (grads[k + 1].0 as f64, grads[k + 1].1 as f64)));
            // (MEASURED filter-other-4-*, -5-*: a draw that filters with PYRAMIDALQUAD or
            // GAUSSIANQUAD has its attributes set up as Direct3D 10's rasterizer does, the planes
            // C + gy y + gx x with the area's reciprocal; four pixels a 32nd's edge apart tell)
            let setup = if self.quad_filtered() {
                hit(clause::D3D9_TEX_QUAD);
                Setup::D3d10
            } else {
                Setup::Warp9
            };
            for py in clipr.1..clipr.3 {
                for px in clipr.0..clipr.2 {
                    if !covered[(py * w + px) as usize] {
                        continue;
                    }
                    if xyz {
                        // (untransformed: only the viewport's pixels, D3D9.DRAW.XYZ)
                        let (vx, vy, vw, vh, ..) = self.viewport();
                        if px < vx || px >= vx + vw || py < vy || py >= vy + vh {
                            continue;
                        }
                    }
                    let (sx, sy) = (px as f32 + 0.5, py as f32 + 0.5);
                    let at32: [f32; ATTRS] = std::array::from_fn(|k| match setup {
                        Setup::Warp9 => at9(piece[0].at[k], grads[k], x[0], y[0], sx, sy),
                        Setup::D3d10 => plane10(x, y, piece.map(|v| v.at[k]), sx, sy),
                    });
                    let at: [f64; ATTRS] = match &planes {
                        Some(p) => p.at(sx, sy),
                        None if persp => rhw_value(at32),
                        None => at32.map(f64::from),
                    };
                    let (diffuse, specular) = shaded(flat, (a.diffuse, a.specular), (std::array::from_fn(|k| at[1 + k]), std::array::from_fn(|k| at[5 + k])));
                    if !persp {
                        // (the footprint as Direct3D 10's coarse derivatives, the quad's top-left
                        // pixel's s/q, t/q against its right and lower neighbours': projected
                        // coordinates, D3D9.STAGE.PROJFOOT, and every other, the clean-room notes)
                        let attr = |ox: f32, oy: f32| -> [f64; ATTRS] {
                            match &planes {
                                Some(p) => p.at(sx + ox, sy + oy),
                                // (the clean-room notes and MEASURED aniso-4, -5, -17, -20, -27,
                                // aniso-axis-11: the quad's values as the pixel shader's planes
                                // give them, C + gy y + gx x, Direct3D 10's setup)
                                None => std::array::from_fn(|k| f64::from(plane10(x, y, piece.map(|v| v.at[k]), sx + ox, sy + oy))),

                            }
                        };
                        let (ox, oy) = quad_origin(px, py);
                        self.draw.duv = quad_duv(&attr(ox, oy), &attr(ox + 1.0, oy), &attr(ox, oy + 1.0));
                    }
                    self.pixel(px, py, at[0], diffuse, specular, st_of(&at))?;
                }
            }
        }
        Ok(())
    }

    fn pixel(&mut self, x: i32, y: i32, z: f64, diffuse: [f64; 4], specular: [f64; 4], uv: [[f64; 2]; NSET]) -> Res<()> {
        let mut c = self.stages(diffuse, specular, uv)?;
        if self.rs(29) != 0 {
            hit(clause::D3D9_SPECULAR);
            for k in 0..3 {
                c[k] = specular_add(c[k], specular[k]);
            }
        }
        if self.rs(28) != 0 {
            // (MEASURED fog-*: the factor from the iterated specular alpha (FOGTABLEMODE NONE) or
            // EXP, EXP2, LINEAR of the pixel's z; the color f c + (1 - f) fog, in f32)
            hit(clause::D3D9_FOG);
            let fl = |k: u32| f32::from_bits(self.rs(k)) as f64;
            let (start, end, density) = (fl(36), fl(37), fl(38));
            let f = fog_factor(self.rs(35), z, specular[3], (start, end, density))?;
            let fc = argb(self.rs(34));
            for k in 0..3 {
                c[k] = f64::from(fog_blend(f, c[k] as f32, fc[k] as f32));
            }
        }
        if self.rs(15) != 0 {
            hit(clause::D3D9_ALPHATEST);
            if !cmp(self.rs(25), unorm(c[3], 8), (self.rs(24) & 0xff) as u64) {
                return Ok(());
            }
        }
        let rt = self.draw.rt.as_ref().ok_or(Fail::Empty)?;
        let k = (y * rt.w + x) as usize;
        // stencil, then depth
        let zq = unorm(z, 24);
        let ztest = self.rs(7) != 0;
        let zpass = !ztest || cmp(self.rs(23), zq, self.draw.depth[k] as u64);
        if self.rs(52) != 0 {
            hit(clause::D3D9_STENCIL);
            // (two-sided: a counterclockwise triangle takes the CCW states)
            let [fail, zfail, pass, func] = stencil_states(self.v8, self.rs(185), self.draw.ccw);
            let s = self.draw.stencil[k];
            let (refv, mask, wmask) = (self.rs(57) as u8, self.rs(58) as u8, self.rs(59) as u8);
            let spass = cmp(self.rs(func), (refv & mask) as u64, (s & mask) as u64);
            let op = if !spass { self.rs(fail) } else if !zpass { self.rs(zfail) } else { self.rs(pass) };
            let ns = stencil_op(op, s, refv);
            self.draw.stencil[k] = stencil_write(s, ns, wmask);
            if !spass {
                return Ok(());
            }
        }
        if !zpass {
            hit(clause::D3D9_DEPTH);
            return Ok(());
        }
        if ztest {
            hit(clause::D3D9_DEPTH);
            if self.rs(14) != 0 {
                self.draw.depth[k] = zq as u32;
            }
        }
        let rt = self.draw.rt.as_ref().ok_or(Fail::Empty)?;
        let fm = format(rt.fmt).ok_or(Fail::Bad)?;
        let old = to_rgba(rt.fmt, &decode(&fm, rt.px(x, y)?));
        let mut new = c;
        if self.rs(27) != 0 {
            hit(clause::D3D9_BLEND);
            new = self.blend(rt.fmt, c, old)?;
        }
        let mask = self.rs(168);
        if mask & 0xf != 0xf {
            hit(clause::D3D9_COLORWRITE);
            new = color_write(mask, new, old);
        }
        if fm.ch.iter().any(|c| c.c == b'x') {
            hit(clause::D3D9_DRAW_XZERO);
        }

        let px = encode_x(&fm, new, false);
        self.draw.rt.as_mut().ok_or(Fail::Empty)?.set(x, y, &px);
        Ok(())
    }

    /// the blend states (blend_float's)
    fn blend_states(&self) -> BlendStates {
        if self.v8 && [19, 20].iter().any(|&i| matches!(self.rs(i), 14 | 15)) {
            hit(clause::D3D9_BLEND_FACTOR);
        }
        BlendStates {
            src: self.rs(19),
            dst: self.rs(20),
            op: self.rs(171),
            separate: !self.v8 && self.rs(206) != 0,
            src_alpha: self.rs(207),
            dst_alpha: self.rs(208),
            op_alpha: self.rs(209),
            factor: blend_color(self.v8, self.rs(193)),
        }
    }

    fn blend(&self, fmt: u32, s: [f64; 4], d: [f64; 4]) -> Res<[f64; 4]> {
        let fm = format(fmt).ok_or(Fail::Bad)?;
        if fm.kind != Kind::Int || fm.ch.iter().any(|c| c.c != b'x' && (c.bits != 8 || !matches!(c.c, b'r' | b'g' | b'b' | b'a'))) {
            return blend_float(s, d, &self.blend_states()).ok_or(Fail::Bad);
        }
        let (s, d) = (s.map(|x| unorm(x, 8) as i64), d.map(|x| unorm(x, 8) as i64));
        let c = blend_color(self.v8, self.rs(193));
        if self.v8 && [19, 20].iter().any(|&i| matches!(self.rs(i), 14 | 15)) {
            hit(clause::D3D9_BLEND_FACTOR);
        }
        let factor = |f: u32, k: usize| -> Res<i64> { factor8(f, k, s, d, c).ok_or(Fail::Bad) };
        let separate = !self.v8 && self.rs(206) != 0;
        // (MEASURED, blend-*: two ways of rounding. With SRCALPHASAT among the factors, the
        // separate alpha factors on, or a target without alpha (X bits), each channel is the
        // exact sum rounded to nearest; otherwise the products are taken in 1/256ths below)
        let sat = [19, 20].iter().chain(if separate { [207, 208].iter() } else { [].iter() }).any(|&i| self.rs(i) == 11);
        let exact = sat || separate || fm.ch.iter().any(|c| c.c == b'x');
        let mut out = [0.0; 4];
        for k in 0..4 {
            let alpha = separate && k == 3;
            let (mut sf, mut df, op) = if alpha { (self.rs(207), self.rs(208), self.rs(209)) } else { (self.rs(19), self.rs(20), self.rs(171)) };
            (sf, df) = both_alpha(sf, df, alpha);
            let op = blend_op(op);
            let (fs, fd) = (factor(sf, k)?, factor(df, k)?);
            if exact && !matches!(op, BlendOp::Min | BlendOp::Max) {
                hit(clause::D3D9_BLEND_TIE);
            }
            let r = blend8(op, (s[k], sf == 2, fs), (d[k], df == 2, fd), exact);
            out[k] = (r as f32 / 255.0) as f64;
        }
        Ok(out)
    }
}

/// blending on a target of 8-bit unorm channels (MEASURED, d3d9draw blend-*: each product
/// x * f of two 8-bit numbers as x * f * 257 >> 8, a number in 1/256ths that is a little
/// under x * f / 255 (but x * ONE as x exactly); the two added or subtracted, then rounded to 8 bits half up)
/// blending on a target of any other format (MEASURED, d3d9draw blend-fmt-*: 95 cases, every
/// format the target takes): in f32, the source clamped to 0..1, the destination as read
/// a blend operation outside ADD..MAX is MAX (MEASURED x64 blend-badop-*; the x86 runtime
/// dies drawing with one: D3D9.BLEND.OPX86, so it does not get here)
pub fn blend_op(op: u32) -> BlendOp {
    match op {
        1 => BlendOp::Add,
        2 => BlendOp::Sub,
        3 => BlendOp::RevSub,
        4 => BlendOp::Min,
        5 => BlendOp::Max,
        _ => {
            hit(clause::D3D9_BLEND_OP);
            BlendOp::Max
        }
    }
}

/// what blending reads of the render states: SRCBLEND, DESTBLEND, BLENDOP, (9)
/// SEPARATEALPHABLENDENABLE and the alpha's SRCBLENDALPHA, DESTBLENDALPHA, BLENDOPALPHA, and
/// the BLENDFACTOR color (blend_color)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlendStates {
    pub src: u32,
    pub dst: u32,
    pub op: u32,
    pub separate: bool,
    pub src_alpha: u32,
    pub dst_alpha: u32,
    pub op_alpha: u32,
    pub factor: u32,
}

/// a blend factor (D3DBLEND) of channel k in f32, of the clamped source s, the destination d and
/// the BLENDFACTOR color's channels bf; none outside the enumeration and for the BOTH ones
/// (both_alpha turns them first)
fn factor_f32(f: u32, k: usize, s: [f32; 4], d: [f32; 4], bf: [f32; 4]) -> Option<f32> {
    let (sk, dk, bk) = (*s.get(k)?, *d.get(k)?, *bf.get(k)?);
    Some(match f {
        1 => 0.0,
        2 => 1.0,
        3 => sk,
        4 => 1.0 - sk,
        5 => s[3],
        6 => 1.0 - s[3],
        7 => d[3],
        8 => 1.0 - d[3],
        9 => dk,
        10 => 1.0 - dk,
        11 if k < 3 => s[3].min(1.0 - d[3]),
        11 => 1.0,
        14 => bk,
        15 => 1.0 - bk,
        _ => return None,
    })
}

/// blending on a target of any format but 8-bit unsigned ones (MEASURED d3d9draw blend-fmt-*,
/// 95 cases; D3D9.BLEND.FORMAT): in f32, the source clamped to 0..1, the destination as read,
/// source * its factor OP destination * its factor, MIN and MAX of the two alone; the alpha by
/// the separate states where they are on; the result written as the format writes; none for a
/// factor outside the enumeration; spec/lean/Kikizake/D3d9/Blend.lean `blendFloat`
pub fn blend_float(s: [f64; 4], d: [f64; 4], b: &BlendStates) -> Option<[f64; 4]> {
    hit(clause::D3D9_BLEND_FORMAT);
    let s = s.map(|x| x.clamp(0.0, 1.0) as f32);
    let d = d.map(|x| x as f32);
    let bf = argb(b.factor).map(|v| v as f32);
    let mut out = [0.0; 4];
    for (k, o) in out.iter_mut().enumerate() {
        let alpha = b.separate && k == 3;
        let (sf, df, op) = if alpha { (b.src_alpha, b.dst_alpha, b.op_alpha) } else { (b.src, b.dst, b.op) };
        let (sf, df) = both_alpha(sf, df, alpha);
        let (sk, dk) = (*s.get(k)?, *d.get(k)?);
        let (x, y) = (sk * factor_f32(sf, k, s, d, bf)?, dk * factor_f32(df, k, s, d, bf)?);
        *o = f64::from(match blend_op(op) {
            BlendOp::Add => x + y,
            BlendOp::Sub => x - y,
            BlendOp::RevSub => y - x,
            BlendOp::Min => sk.min(dk),
            BlendOp::Max => sk.max(dk),
        });
    }
    Some(out)
}

/// the BLENDFACTOR color blends take: Direct3D 8 has no BLENDFACTOR state, yet BLENDFACTOR and
/// INVBLENDFACTOR blend with white there (MEASURED d3d8draw blend-factor8, D3D9.BLEND.FACTOR)
pub fn blend_color(v8: bool, state: u32) -> u32 {
    if v8 { 0xffff_ffff } else { state }
}

/// a blend factor (D3DBLEND) of one 8-bit channel k (0 red .. 3 alpha) of the source s and the
/// destination d, BLENDFACTOR c (ARGB); none for a factor outside the enumeration and the
/// BOTH ones (both_alpha turns them first)
pub fn factor8(f: u32, k: usize, s: [i64; 4], d: [i64; 4], c: u32) -> Option<i64> {
    let bf = [c >> 16 & 255, c >> 8 & 255, c & 255, c >> 24].map(i64::from);
    Some(match f {
        1 => 0,
        2 => 255,
        3 => *s.get(k)?,
        4 => 255 - s.get(k)?,
        5 => s[3],
        6 => 255 - s[3],
        7 => d[3],
        8 => 255 - d[3],
        9 => *d.get(k)?,
        10 => 255 - d.get(k)?,
        11 if k < 3 => s[3].min(255 - d[3]),
        11 => 255,
        14 => *bf.get(k)?,
        15 => 255 - bf.get(k)?,
        _ => return None,
    })
}

/// the source and destination factors after BOTHSRCALPHA (12) and BOTHINVSRCALPHA (13): as either
/// factor, both factors from the source's alpha, the first named winning (SRCALPHA and
/// INVSRCALPHA, or the reverse: MEASURED blend-dest-12, blend-dest-13); as a separate alpha
/// factor either is ZERO (MEASURED blend-bothalpha-*, blend-random-5, -31, D3D9.BLEND.BOTHALPHA)
pub fn both_alpha(sf: u32, df: u32, alpha: bool) -> (u32, u32) {
    if ![sf, df].iter().any(|f| matches!(f, 12 | 13)) {
        return (sf, df);
    }
    if alpha {
        hit(clause::D3D9_BLEND_BOTHALPHA);
        let zero = |f: u32| if matches!(f, 12 | 13) { 1 } else { f };
        (zero(sf), zero(df))
    } else if sf == 12 || sf != 13 && df == 12 {
        (5, 6)
    } else {
        (6, 5)
    }
}

/// the color written through COLORWRITEENABLE (bit 0 red, 1 green, 2 blue, 3 alpha): each
/// channel the new one where its bit is set, the old one where not (D3D9.COLORWRITE)
pub fn color_write<T: Copy>(mask: u32, new: [T; 4], old: [T; 4]) -> [T; 4] {
    let mut out = new;
    for (k, bit) in [(0, 1), (1, 2), (2, 4), (3, 8)] {
        if mask & bit == 0 {
            out[k] = old[k];
        }
    }
    out
}
