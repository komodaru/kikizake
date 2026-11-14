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
        let bits: u32 = layout[i + 1..j].parse().unwrap();
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
    let mut w: u128 = 0;
    for c in &f.ch {
        if c.c == b'x' && !x_alpha {
            continue;
        }
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
        w |= (raw as u128) << c.shift;
    }
    w.to_le_bytes()[..f.bytes].to_vec()
}

pub fn round_even(x: f64) -> f64 {
    let r = x.round();
    if (x - x.trunc()).abs() == 0.5 && r % 2.0 != 0.0 {
        r - x.signum()
    } else {
        r
    }
}

/// one texel of a DXT1..DXT5 texture as rgba
fn dxt_texel(n: u32, bytes: &[u8], w: i32, x: i32, y: i32) -> [f64; 4] {
    hit(clause::D3D9_TEX_DXT);
    let bsz = if n == 1 { 8 } else { 16 };
    let bi = ((y / 4) * ((w + 3) / 4) + x / 4) as usize * bsz;
    let b = &bytes[bi..bi + bsz];
    let t = ((y % 4) * 4 + x % 4) as u32;
    let cb = if n == 1 { &b[0..8] } else { &b[8..16] };
    let (c0, c1) = (u16::from_le_bytes([cb[0], cb[1]]) as u32, u16::from_le_bytes([cb[2], cb[3]]) as u32);
    let idx = u32::from_le_bytes([cb[4], cb[5], cb[6], cb[7]]) >> (2 * t) & 3;
    // (the 565 endpoints widened to 8 bits by repeating their top bits)
    let ex = |c: u32| [(c >> 11 & 31) << 3 | (c >> 13 & 7), (c >> 5 & 63) << 2 | (c >> 9 & 3), (c & 31) << 3 | (c >> 2 & 7)];
    let (e0, e1) = (ex(c0), ex(c1));
    let four = n != 1 || c0 > c1;
    let mut c = [0.0, 0.0, 0.0, 1.0];
    for k in 0..3 {
        let (a, b) = (e0[k], e1[k]);
        // (MEASURED: thirds as 171/256 and 85/256, taken down; the half taken down)
        let v = match (idx, four) {
            (0, _) => a,
            (1, _) => b,
            (2, true) => (171 * a + 85 * b) >> 8,
            (3, true) => (85 * a + 171 * b) >> 8,
            (2, false) => (a + b) >> 1,
            _ => 0,
        };
        c[k] = v as f64 / 255.0;
    }
    if n == 1 && !four && idx == 3 {
        c[3] = 0.0;
    }
    if n == 2 || n == 3 {
        let a = u64::from_le_bytes(b[0..8].try_into().unwrap()) >> (4 * t) & 15;
        c[3] = (a * 17) as f64 / 255.0;
    } else if n >= 4 {
        let (a0, a1) = (b[0] as u32, b[1] as u32);
        let ai = (u64::from_le_bytes([b[2], b[3], b[4], b[5], b[6], b[7], 0, 0]) >> (3 * t) & 7) as u32;
        // (MEASURED: sevenths and fifths rounded to nearest)
        let mix = |wa: u32, wb: u32| (wa * a0 + wb * a1 + (wa + wb) / 2) / (wa + wb);
        let v = match ai {
            0 => a0,
            1 => a1,
            i if a0 > a1 => mix(8 - i, i - 1),
            6 => 0,
            7 => 255,
            i => mix(6 - i, i - 1),
        };
        c[3] = v as f64 / 255.0;
    }
    c
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
    fn px(&self, x: i32, y: i32) -> &[u8] {
        let b = format(self.fmt).unwrap().bytes;
        let i = (y * self.w + x) as usize * b;
        &self.bytes[i..i + b]
    }
    fn set(&mut self, x: i32, y: i32, v: &[u8]) {
        let b = v.len();
        let i = (y * self.w + x) as usize * b;
        self.bytes[i..i + b].copy_from_slice(v);
    }
}

/// D3DBLENDOP
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlendOp {
    Add,
    Sub,
    RevSub,
    Min,
    Max,
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
    pub duv: ((f64, f64), (f64, f64)),
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// xorshift32 bytes (SPEC.md), or byte i = i mod 256 for seed -1, for w x h pixels of a format
/// (block rows for DXT)
pub fn fill_bytes(f: &Fmt, w: i32, h: i32, seed: i64) -> Vec<u8> {
    let n = match f.kind {
        Kind::Dxt(_) => ((w + 3) / 4 * f.bytes as i32 * ((h + 3) / 4)) as usize,
        _ => (w * h) as usize * f.bytes,
    };
    if seed == -1 {
        return (0..n).map(|i| i as u8).collect();
    }
    let mut r = crate::rng::XorShift32::new(seed as u32);
    (0..n).map(|_| r.next() as u8).collect()
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

/// one vertex as the pipeline sees it: screen position (1/256 pixel), rhw, colors, coordinates
#[derive(Clone, Copy, Debug)]
struct Vtx {
    x: f32,
    y: f32,
    z: f64,
    rhw: f64,
    diffuse: [f64; 4],
    specular: [f64; 4],
    uv: [f64; 2],
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
            let l = *e.last().unwrap();
            e.push(elem((l.0 >> 16) + decl_type_size(l.1 & 255).unwrap(), 1, 5, k));
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
        let off = e.get(at).map_or_else(|| e.last().map_or(0, |l| (l.0 >> 16) + decl_type_size(l.1 & 255).unwrap()), |x| x.0 >> 16);
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
/// z, diffuse, specular, (u, v), rhw
const ATTRS: usize = 12;
/// the attributes perspective divides (all but z and rhw)
const PERSP: std::ops::Range<usize> = 1..11;

/// a vertex of a clipped polygon: its position and attributes, in f32
#[derive(Clone, Copy)]
struct PolyVtx {
    x: f32,
    y: f32,
    at: [f32; ATTRS],
}

impl Vtx {
    fn attrs(&self) -> [f32; ATTRS] {
        let d = self.diffuse.map(|c| c as f32);
        let s = self.specular.map(|c| c as f32);
        [self.z as f32, d[0], d[1], d[2], d[3], s[0], s[1], s[2], s[3], self.uv[0] as f32, self.uv[1] as f32, self.rhw as f32]
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
        let ([x0, x1, x2], [y0, y1, y2]) = (self.xs, self.ys);
        let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        let r = 1.0 / area;
        let (d1, d2) = (a[1] - a[0], a[2] - a[0]);
        let gx = (d1 * (y2 - y0) - d2 * (y1 - y0)) * r;
        let gy = (d2 * (x1 - x0) - d1 * (x2 - x0)) * r;
        let c = (a[0] - gx * x0) - gy * y0;
        (c + gy * cy) + gx * cx
    }

    fn at(&self, cx: f32, cy: f32) -> [f64; ATTRS] {
        let q = 1.0 / self.plane(self.iw, cx, cy);
        std::array::from_fn(|k| {
            let v = self.plane(self.at.map(|a| a[k]), cx, cy);
            (if PERSP.contains(&k) { v * q } else { v }) as f64
        })
    }
}

/// the triangle's vertex of least x, then least y (the first of equals)
fn least_x(c: &[PolyVtx; 3]) -> usize {
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

/// a sampler's D3DTEXTUREFILTERTYPE
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum TexFilter {
    Point,
    Linear,
    Anisotropic,
    /// NONE, PYRAMIDALQUAD, GAUSSIANQUAD, CONVOLUTIONMONO, or past them (cases filter-other-*)
    Other(u32),
}

impl TexFilter {
    fn of(v: u32) -> TexFilter {
        match v {
            1 => TexFilter::Point,
            2 => TexFilter::Linear,
            3 => TexFilter::Anisotropic,
            v => TexFilter::Other(v),
        }
    }
}

/// a line's snapped ends (1/256 pixel, Direct3D 9's pixel centers at whole pixels)
struct Seg9 {
    a: (i128, i128),
    b: (i128, i128),
}

/// how a line meets a pixel's diamond
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Meet {
    No,
    Yes,
    /// along one of the diamond's edges (open, D3D9.DRAW.POINTSLINES)
    AlongEdge,
}

const DIAMOND: i128 = 128;

impl Seg9 {
    /// p, scaled by s, in the diamond of the pixel whose center is c: its inside, its lower-left
    /// and lower-right edges and bottom corner, and for a y-major line also its right corner
    fn in_diamond(&self, p: (i128, i128), c: (i128, i128), s: i128, ymaj: bool) -> bool {
        let (dx, dy) = (p.0 - c.0 * s, p.1 - c.1 * s);
        let (d, r) = (dx.abs() + dy.abs(), DIAMOND * s);
        if d != r {
            return d < r;
        }
        dx == 0 && dy == r || dx == r && dy == 0 && ymaj || dy > 0 && dx != 0
    }

    fn meets(&self, c: (i128, i128), ymaj: bool) -> Meet {
        let (a, b) = (self.a, self.b);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let less = |p: (i128, i128), q: (i128, i128)| p.0 * q.1 < q.0 * p.1;
        let (mut t0, mut t1) = ((0i128, 1i128), (1i128, 1i128));
        for (u, v) in [(1i128, 1i128), (1, -1), (-1, 1), (-1, -1)] {
            let num = DIAMOND - (u * (a.0 - c.0) + v * (a.1 - c.1));
            let den = u * dx + v * dy;
            match den.signum() {
                0 if num < 0 => return Meet::No,
                0 if num == 0 && dx != 0 => {
                    // (the line runs on this edge's line of the diamond)
                    return if self.along(c) { Meet::AlongEdge } else { Meet::No };
                }
                0 => {}
                1 => {
                    if less((num, den), t1) {
                        t1 = (num, den);
                    }
                }
                _ => {
                    if less(t0, (-num, -den)) {
                        t0 = (-num, -den);
                    }
                }
            }
        }
        if less(t1, t0) {
            return Meet::No;
        }
        let mid = (t0.0 * t1.1 + t1.0 * t0.1, 2 * t0.1 * t1.1);
        if [t0, t1, mid].iter().any(|&(n, s)| self.in_diamond((a.0 * s + dx * n, a.1 * s + dy * n), c, s, ymaj)) {
            Meet::Yes
        } else {
            Meet::No
        }
    }

    /// some stretch of the segment, of length, lies on the diamond's boundary
    fn along(&self, c: (i128, i128)) -> bool {
        let (a, b) = (self.a, self.b);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        // the parameters inside the closed diamond
        let (mut lo, mut hi) = ((0i128, 1i128), (1i128, 1i128));
        let less = |p: (i128, i128), q: (i128, i128)| p.0 * q.1 < q.0 * p.1;
        for (u, v) in [(1i128, 1i128), (1, -1), (-1, 1), (-1, -1)] {
            let num = DIAMOND - (u * (a.0 - c.0) + v * (a.1 - c.1));
            let den = u * dx + v * dy;
            match den.signum() {
                0 if num < 0 => return false,
                0 => {}
                1 => {
                    if less((num, den), hi) {
                        hi = (num, den);
                    }
                }
                _ => {
                    if less(lo, (-num, -den)) {
                        lo = (-num, -den);
                    }
                }
            }
        }
        less(lo, hi)
    }
}

/// the sides a triangle is clipped against, in the order it is
#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// a polygon clipped to -0.5 ..= x1 and -0.5 ..= y1, a side at a time (Sutherland-Hodgman, each
/// edge from its first vertex: p + t (q - p), t = dp / (dp - dq), in f32, the attributes alike)
fn clip_polygon(poly: &[PolyVtx], x1: f32, y1: f32) -> Vec<PolyVtx> {
    let mut poly = poly.to_vec();
    for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
        let d = |v: &PolyVtx| match side {
            Side::Left => v.x + 0.5,
            Side::Right => x1 - v.x,
            Side::Top => v.y + 0.5,
            Side::Bottom => y1 - v.y,
        };
        let mut out = Vec::new();
        for (i, p) in poly.iter().enumerate() {
            let q = &poly[(i + 1) % poly.len()];
            let (dp, dq) = (d(p), d(q));
            if dp >= 0.0 {
                out.push(*p);
            }
            if (dp >= 0.0) != (dq >= 0.0) {
                let t = dp / (dp - dq);
                out.push(PolyVtx { x: p.x + t * (q.x - p.x), y: p.y + t * (q.y - p.y), at: std::array::from_fn(|k| p.at[k] + t * (q.at[k] - p.at[k])) });
            }
        }
        poly = out;
    }
    poly
}

/// a convex polygon of n vertices cut into the strip v1 v2 v0 v3 v(n-1) v4 v(n-2) ... (MEASURED:
/// d3d9draw shade-clip-*, no other order fits all 16)
fn strip(n: usize) -> Vec<[usize; 3]> {
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
    let area = (t[1].0 - t[0].0) * (t[2].1 - t[0].1) - (t[1].1 - t[0].1) * (t[2].0 - t[0].0);
    if area == 0 {
        return;
    }
    let t = if area < 0 { [t[0], t[2], t[1]] } else { t };
    for py in 0..h {
        for px in 0..w {
            let (sx, sy) = (px as i64 * 256, py as i64 * 256);
            let ok = (0..3).all(|k| {
                let (p, q) = (t[k], t[(k + 1) % 3]);
                let e = (q.0 - p.0) * (sy - p.1) - (q.1 - p.1) * (sx - p.0);
                e > 0 || (e == 0 && (q.1 - p.1 < 0 || (q.1 == p.1 && q.0 - p.0 > 0)))
            });
            if ok {
                covered[(py * w + px) as usize] = true;
            }
        }
    }
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
    /// a sampler state (Direct3D 8: the stage state of the same meaning)
    fn samp(&self, s: u32, t: u32) -> u32 {
        if self.v8 {
            // ADDRESSU V W, BORDERCOLOR, MAG MIN MIP filters: stage states 13 14 25 15 16 17 18
            let t8 = [0, 13, 14, 25, 15, 16, 17, 18][t as usize];
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
                let all = [(0, 0, w, h)];
                let list: &[(i32, i32, i32, i32)] = if rects.is_empty() { &all } else { &rects };
                let fm = format(rt.fmt).unwrap();
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
                let b = format(rt.fmt).unwrap().bytes;
                out.push(format!("px rt {} {} {}", rt.w, rt.h, b * 8));
                for row in rt.bytes.chunks(rt.w as usize * b) {
                    out.push(row.iter().map(|x| format!("{x:02x}")).collect());
                }
            }
            "d9fvf" => {
                self.draw.fvf = n(op, 1)? as u32;
                hit(clause::D3D9_FVF_SET);
                self.draw.decl = fvf_runtime_decl(self.draw.fvf)?;
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
                // (MEASURED, check: what CreateTexture / CreateRenderTarget take, Direct3D 8's
                // CheckDeviceFormat too, though its CreateTexture refuses three more)
                hit(clause::D3D9_FORMATS_CHECK);
                let f = f as u32;
                let tex = supported(f, false, false);
                const NOTAVAILABLE: u32 = 0x8876_086a;
                let ok = |b: bool| if b { 0 } else { NOTAVAILABLE };
                let dxt = format(f).is_some_and(|m| matches!(m.kind, Kind::Dxt(_)));
                let hr = match (u, rtype) {
                    // (Direct3D 8, MEASURED d3d8draw check-usage: a plain surface INVALIDCALL; a
                    // render target volume as a texture, not the palette nor the DXT formats;
                    // A2R10G10B10, A16B16G16R16 and L16 depth stencil too; no AUTOGENMIPMAP)
                    (0, 1) | (0x400, 3) if self.v8 => INVALIDCALL,
                    (1, 4) if self.v8 => ok(tex && !matches!(f, 40 | 41) && !dxt),
                    (2, 1) if self.v8 && matches!(f, 35 | 36 | 81) => 0,
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
                };
                if !matches!((u, rtype), (0, 3) | (1, 1) | (1, 3)) {
                    hit(clause::D3D9_FORMATS_USAGE);
                }
                out.push(format!("check {f} {u:x} {rtype} hr {hr:08x}"));
            }
            "d9tex" => {
                let (i, w, h, f) = (n(op, 1)? as usize, n(op, 2)? as i32, n(op, 3)? as i32, n(op, 4)? as u32);
                hit(clause::D3D9_TEX_CREATE);
                if !supported(f, false, self.v8) {
                    self.draw.texs[i] = None;
                    out.push(format!("tex failed {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let fm = format(f).ok_or(Fail::Bad)?;
                self.draw.texs[i] = Some(Surface { w, h, fmt: f, bytes: fill_bytes(&fm, w, h, 0).iter().map(|_| 0).collect() });
            }
            "d9texfill" => {
                let i = n(op, 1)? as usize;
                let seed = if t.get(2).is_some_and(|s| s == "ramp") { -1 } else { n(op, 2)? };
                let Some(tx) = &mut self.draw.texs[i] else {
                    out.push(format!("tex {i} none"));
                    return Ok(true);
                };
                tx.bytes = fill_bytes(&format(tx.fmt).unwrap(), tx.w, tx.h, seed);
            }
            "d9palette" => {
                let b = fill_bytes(&format(50).unwrap(), 1024, 1, n(op, 1)?);
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
                let op = self.rs(171);
                if crate::model::arch() == 32 && !(1..=5).contains(&op) {
                    // (the executor does not draw: the x86 runtime dies on it, MEASURED)
                    hit(clause::D3D9_BLEND_OPX86);
                    out.push(format!("draw x86 blendop {op}"));
                } else if let Some(a) = (0..8u32).filter(|&st| crate::model::arch() == 32 && self.draw.stage_tex[st as usize].is_some()).flat_map(|st| [self.samp(st, 1), self.samp(st, 2)]).find(|a| !(1..=5).contains(a)) {
                    // (nor with a texture's address mode outside WRAP..MIRRORONCE, MEASURED)
                    hit(clause::D3D9_TEX_ADDRESSX86);
                    out.push(format!("draw x86 address {a}"));
                } else if self.draw.palette.is_none() && self.draw.stage_tex.iter().flatten().any(|&i| self.draw.texs[i].as_ref().is_some_and(|t| matches!(t.fmt, 40 | 41))) {
                    // (MEASURED tex-*-P8, tex-*-A8P8: no palette, no drawing)
                    hit(clause::D3D9_TEX_PALETTE_NONE);
                    out.push(format!("draw hr {INVALIDCALL:08x}"));
                } else {
                    self.draw_prims(ty, nv, &words)?;
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
                    // (SetVertexDeclaration: GetFVF answers 0 after it, MEASURED: decl-*)
                    self.draw.fvf = 0;
                    self.draw.decl = Some(e);
                } else {
                    // (the declaration set before stays)
                    out.push("decl hr 80004005".into());
                }
            }
            "d9xform" => {
                let ty = n(op, 1)? as u32;
                let m: Vec<f32> = (2..18).map(|i| n(op, i).map(|v| f32::from_bits(v as u32))).collect::<Res<_>>()?;
                if !matches!(ty, 2 | 3 | 256) {
                    // (the generators set only VIEW, PROJECTION, WORLD)
                    return Err(Fail::Bad);
                }
                hit(clause::D3D9_DRAW_XYZ);
                self.draw.xform.insert(ty, m.try_into().unwrap());
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
        let fm = format(tx.fmt).unwrap();
        hit(clause::D3D9_TEX_FORMAT);
        match fm.kind {
            Kind::Int | Kind::F16 | Kind::F32 if !fm.ch.iter().any(|c| c.c == b'p') => {}
            Kind::Dxt(n) => return Ok(dxt_texel(n, &tx.bytes, tx.w, x as i32, y as i32)),
            Kind::Int if matches!(tx.fmt, 40 | 41) => {
                // (MEASURED palette-*: the entry's color; P8's alpha the entry's flags, A8P8's its own)
                hit(clause::D3D9_TEX_PALETTE);
                let p = tx.px(x as i32, y as i32);
                let e = self.draw.palette.as_ref().ok_or(Fail::Bad)?[p[0] as usize];
                let a = if tx.fmt == 40 { p[1] } else { e[3] };
                return Ok([e[0] as f64 / 255.0, e[1] as f64 / 255.0, e[2] as f64 / 255.0, a as f64 / 255.0]);
            }
            // (none taken: every format WARP takes is decoded above, law every_taken_format_is_decoded)
            _ => return Err(Fail::Bad),
        }
        hit(clause::D3D9_TEX_DECODE);
        Ok(to_rgba(tx.fmt, &decode(&fm, tx.px(x as i32, y as i32))))
    }

    /// a stage's texture at (u, v)
    fn sample(&self, st: u32, tx: &Surface, u: f64, v: f64) -> Res<[f64; 4]> {
        let (mu, mv) = (self.samp(st, 1), self.samp(st, 2));
        // (MEASURED filter2-0-32, filter2-1-32: a texture of one level magnifies everywhere,
        // minified or not: the level of detail is clamped to the texture's levels, 0, before it
        // picks the filter, so MINFILTER takes no part)
        hit(clause::D3D9_TEX_MAGONLY);
        let filter = match TexFilter::of(self.samp(st, 5)) {
            TexFilter::Anisotropic => {
                // (MEASURED filter2-5-4, d3d8draw filter2-2-4: linear while no axis of the
                // footprint is longer than a texel; past that, its own taps: open)
                let ((dudx, dudy), (dvdx, dvdy)) = self.draw.duv;
                let (w, h) = (tx.w as f64, tx.h as f64);
                let major = ((dudx * w).hypot(dvdx * h)).max((dudy * w).hypot(dvdy * h));
                if major > 1.0 {
                    return Err(Fail::Unmodeled(open::D3D9_TEX_FILTER));
                }
                TexFilter::Linear
            }
            f => f,
        };
        let (x, y) = (u * tx.w as f64, v * tx.h as f64);
        match filter {
            TexFilter::Point => {
                hit(clause::D3D9_TEX_POINT);
                // (MEASURED: the texel coordinate rounded to the nearest 64th, then taken down:
                // d3d8draw filter2-3-4 takes texel 2 at 1.99375; 1/16, 1/32 break d3d9draw filter2-3-*,
                // 1/128 misses it)
                let near64 = |x: f64| ((x * 64.0).round() / 64.0).floor() as i64;
                self.texel(st, tx, near64(x), near64(y), mu, mv)
            }
            TexFilter::Linear => {
                hit(clause::D3D9_TEX_LINEAR);
                // (MEASURED, filter-*, filter2-*, filter-uv-*: weights in 32nds of a texel taken
                // down, the weighted sum of 8-bit texels truncated)
                let (x, y) = (x - 0.5, y - 0.5);
                let (x0, y0) = (x.floor(), y.floor());
                const Q: i64 = 32;
                let q = Q;
                let (fx, fy) = (((x - x0) * q as f64).floor() as i64, ((y - y0) * q as f64).floor() as i64);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let t = [self.texel(st, tx, x0, y0, mu, mv)?, self.texel(st, tx, x0 + 1, y0, mu, mv)?, self.texel(st, tx, x0, y0 + 1, mu, mv)?, self.texel(st, tx, x0 + 1, y0 + 1, mu, mv)?];
                let w = [(q - fx) * (q - fy), fx * (q - fy), (q - fx) * fy, fx * fy];
                let fm = format(tx.fmt).unwrap();
                let bytes = fm.kind == Kind::Int && fm.ch.iter().all(|c| c.bits == 8 && !matches!(c.c, b'u' | b'v' | b'w' | b'q'));
                let mut c = [0.0; 4];
                for k in 0..4 {
                    c[k] = if bytes {
                        let v: i64 = (0..4).map(|i| unorm(t[i][k], 8) as i64 * w[i]).sum();
                        ((v / (q * q)) as f32 / 255.0) as f64
                    } else {
                        (0..4).map(|i| t[i][k] * w[i] as f64).sum::<f64>() / (q * q) as f64
                    };
                }
                Ok(c)
            }
            TexFilter::Anisotropic | TexFilter::Other(_) => Err(Fail::Unmodeled(open::D3D9_TEX_FILTER)),
        }
    }

    /// a stage argument (D3DTA) as rgba
    fn arg(&self, st: u32, a: u32, r: &Regs, tex: Option<[f64; 4]>) -> Res<[f64; 4]> {
        let mut v = match a & 0xf {
            0 => r.diffuse,
            // (stage 0: CURRENT is the diffuse color)
            1 => r.current,
            // (MEASURED: with no texture, color 1 and the current alpha)
            2 => tex.unwrap_or_else(|| {
                hit(clause::D3D9_STAGE_NOTEX);
                [1.0, 1.0, 1.0, r.current[3]]
            }),
            3 => argb(self.rs(60)),
            4 => r.specular,
            5 if !self.v8 => {
                hit(clause::D3D9_STAGE_ARG);
                r.temp
            }
            6 if !self.v8 => {
                hit(clause::D3D9_STAGE_ARG);
                argb(self.tss(st, 32))
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

    /// the texture stages (D3DTEXTUREOP) in turn, to the first whose COLOROP is DISABLE
    fn stages(&self, diffuse: [f64; 4], specular: [f64; 4], uv: [f64; 2]) -> Res<[f64; 4]> {
        let mut r = Regs { diffuse, specular, current: diffuse, temp: [0.0; 4] };
        // (a bump mapping stage's texel, its kind and the stage, for the stage after it)
        let mut bump: Option<([f64; 4], Bump, u32)> = None;
        // (the texture the stage before sampled)
        let mut last_tex: Option<[f64; 4]> = None;
        for st in 0..8u32 {
            let (cop, aop) = (self.tss(st, 1), self.tss(st, 4));
            if cop == 1 {
                if st == 0 {
                    hit(clause::D3D9_STAGE_DISABLE);
                }
                break;
            }
            if st > 0 {
                hit(clause::D3D9_STAGE_MULTI);
            }
            let tex = match self.draw.stage_tex[st as usize] {
                Some(i) => {
                    if self.tss(st, 11) != 0 {
                        // (TEXCOORDINDEX: another set of coordinates, or a generated one)
                        return Err(Fail::Unmodeled(open::D3D9_STAGE_OPEN));
                    }
                    let (mut u, mut v) = (uv[0], uv[1]);
                    let mut lum = None;
                    if let Some((t, kind, bs)) = bump {
                        // (MEASURED bump-*: the coordinates moved by BUMPENVMAT times (du, dv);
                        // LUMINANCE: the texture's color times L LSCALE + LOFFSET, clamped to 0..1,
                        // L 1 where the format has none)
                        hit(clause::D3D9_STAGE_BUMP);
                        let m = |k: u32| f32::from_bits(self.tss(bs, k)) as f64;
                        let (du, dv) = (t[0], t[1]);
                        u += m(7) * du + m(9) * dv;
                        v += m(8) * du + m(10) * dv;
                        if kind == Bump::Luminance {
                            lum = Some((t[2] * m(22) + m(23)).clamp(0.0, 1.0));
                        }
                    }
                    let mut t = self.sample(st, self.draw.texs[i].as_ref().unwrap(), u, v)?;
                    if let Some(l) = lum {
                        for k in 0..3 {
                            t[k] *= l;
                        }
                    }
                    Some(t)
                }
                None => None,
            };
            let before = std::mem::replace(&mut last_tex, tex);
            bump = None;
            if let Some(kind) = Bump::of(cop) {
                // (MEASURED top-color-BUMPENVMAP*, stage1-BUMPENVMAP*, bump-*: the stage gives
                // the texture the stage before it sampled, 0 where none did; with no texture of
                // its own, (du, dv) and L are 0, bump-2, bump-3 on both)
                hit(clause::D3D9_STAGE_BUMP);
                bump = Some((tex.unwrap_or([0.0; 4]), kind, st));
                let out = before.unwrap_or([0.0; 4]);
                match self.tss(st, 28) {
                    1 => r.current = out,
                    5 if !self.v8 => r.temp = out,
                    _ => return Err(Fail::Bad),
                }
                continue;
            }
            let out = if aop == 1 {
                // ALPHAOP DISABLE under an enabled COLOROP (MEASURED, alphadisable-*, stage 0):
                // the alpha the stage is given where its color operation reads the diffuse color
                // or CURRENT, else 1 (DOTPRODUCT3 writes its own)
                if st == 0 {
                    hit(clause::D3D9_STAGE_ALPHADISABLE_FIRST);
                }
                let mut out = self.stage(st, cop, 2, &r, tex)?;
                if cop != 24 {
                    out[3] = if st > 0 {
                        // (MEASURED alphadisable-stage1-*: a later stage leaves CURRENT's alpha)
                        hit(clause::D3D9_STAGE_ALPHADISABLE);
                        r.current[3]
                    } else if self.reads_current(st, cop) {
                        r.current[3]
                    } else {
                        1.0
                    };
                }
                out
            } else {
                self.stage(st, cop, aop, &r, tex)?
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

    /// whether a stage's color operation reads the diffuse color or CURRENT: through an argument
    /// it uses, or BLENDDIFFUSEALPHA, BLENDCURRENTALPHA
    fn reads_current(&self, st: u32, cop: u32) -> bool {
        let sel = |a: u32| matches!(self.tss(st, a) & 0xf, 0 | 1);
        let (a1, a2, a0) = (cop != 3 && sel(2), cop != 2 && sel(3), matches!(cop, 25 | 26) && sel(26));
        a1 || a2 || a0 || matches!(cop, 12 | 16)
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
                let v: f32 = match op {
                    // (PREMODULATE: MEASURED top-*-PREMODULATE, stage1-PREMODULATE: as SELECTARG1)
                    2 | 17 => a1[i],
                    3 => a2[i],
                    4 => a1[i] * a2[i],
                    5 => a1[i] * a2[i] * 2.0,
                    6 => a1[i] * a2[i] * 4.0,
                    7 => a1[i] + a2[i],
                    8 => a1[i] + a2[i] - 0.5,
                    9 => (a1[i] + a2[i] - 0.5) * 2.0,
                    10 => a1[i] - a2[i],
                    11 => a1[i] + a2[i] - a1[i] * a2[i],
                    12 => a1[i] * da + a2[i] * (1.0 - da),
                    13 => {
                        let ta = tex.unwrap_or([1.0; 4])[3] as f32;
                        a1[i] * ta + a2[i] * (1.0 - ta)
                    }
                    14 => {
                        let fa = argb(self.rs(60))[3] as f32;
                        a1[i] * fa + a2[i] * (1.0 - fa)
                    }
                    15 => a1[i] + a2[i] * (1.0 - tex.unwrap_or([1.0; 4])[3] as f32),
                    16 => a1[i] * ca + a2[i] * (1.0 - ca),
                    // (MEASURED, top-alpha-*, stage1-*: the color-only operations as alpha
                    // operations give 1, the alpha before on a later stage; the bump mapping ones
                    // and DOTPRODUCT3 too, top-alpha-BUMPENVMAP*, top-alpha-DOTPRODUCT3)
                    18..=24 if alpha => if st == 0 { 1.0 } else { ca },
                    18 => a1[i] + a1[3] * a2[i],
                    19 => a1[i] * a2[i] + a1[3],
                    20 => (1.0 - a1[3]) * a2[i] + a1[i],
                    21 => (1.0 - a1[i]) * a2[i] + a1[3],
                    25 => a0[i] + a1[i] * a2[i],
                    26 => a0[i] * a1[i] + (1.0 - a0[i]) * a2[i],
                    _ => return Err(Fail::Unmodeled(open::D3D9_STAGE_OPEN)),
                };
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
        let stride = words.len() / nv;
        if stride < size {
            // (the generators write no stride short of the FVF's size)
            return Err(Fail::Bad);
        }
        if stride > size {
            // (MEASURED stride-*: the vertices that far apart, the words past the FVF's unread)
            hit(clause::D3D9_DRAW_STRIDE);
        }
        let vs: Vec<Vtx> = words
            .chunks(stride)
            .map(|w| {
                let mut i = pos + (fvf >> 4 & 1) as usize * 3 + (fvf >> 5 & 1) as usize;
                let mut take = |on: bool| {
                    let v = if on { Some(w[i]) } else { None };
                    i += on as usize;
                    v
                };
                let diffuse = argb(take(fvf & 0x40 != 0).unwrap_or(0xffff_ffff));
                let specular = argb(take(fvf & 0x80 != 0).unwrap_or(0));
                let uv = if ntex > 0 && tsize(0) >= 2 { [fl(w[i]), fl(w[i + 1])] } else { [0.0; 2] };
                if pos == 3 {
                    let (x, y, z, rhw, clip) = self.transform([w[0], w[1], w[2]].map(f32::from_bits));
                    return Vtx { x, y, z: z as f64, rhw: rhw as f64, diffuse, specular, uv, clip: Some(clip) };
                }
                Vtx { x: f32::from_bits(w[0]), y: f32::from_bits(w[1]), z: fl(w[2]), rhw: fl(w[3]), diffuse, specular, uv, clip: None }
            })
            .collect();
        if ty == 1 {
            return self.points(&vs);
        }
        // the triangles, each with its first (flat shading) vertex first
        let tris: Vec<[usize; 3]> = match ty {
            4 => (0..nv / 3).map(|i| [3 * i, 3 * i + 1, 3 * i + 2]).collect(),
            5 => (0..nv.saturating_sub(2)).map(|i| if i % 2 == 0 { [i, i + 1, i + 2] } else { [i, i + 2, i + 1] }).collect(),
            6 => (0..nv.saturating_sub(2)).map(|i| [i + 1, i + 2, 0]).collect(),
            2 | 3 => {
                let segs: Vec<(usize, usize)> = if ty == 2 { (0..nv / 2).map(|i| (2 * i, 2 * i + 1)).collect() } else { (0..nv.saturating_sub(1)).map(|i| (i, i + 1)).collect() };
                for (i, j) in segs {
                    self.line(vs[i], vs[j])?;
                }
                return Ok(());
            }
            _ => return Err(Fail::Bad),
        };
        hit(clause::D3D9_DRAW_ASSEMBLY);
        for t in tris {
            self.triangle(vs[t[0]], vs[t[1]], vs[t[2]])?;
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

    fn transform(&self, p: [f32; 3]) -> (f32, f32, f32, f32, [f32; 4]) {
        let id: [f32; 16] = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let m = |t: u32| *self.draw.xform.get(&t).unwrap_or(&id);
        let mul = |a: [f32; 16], b: [f32; 16]| -> [f32; 16] { std::array::from_fn(|i| (0..4).fold(0.0f32, |s, k| s + a[i / 4 * 4 + k] * b[k * 4 + i % 4])) };
        let xv = |v: [f32; 4], a: [f32; 16]| -> [f32; 4] { std::array::from_fn(|j| (0..4).fold(0.0f32, |s, k| s + v[k] * a[k * 4 + j])) };
        let c = xv([p[0], p[1], p[2], 1.0], mul(mul(m(256), m(2)), m(3)));
        let (vx, vy, vw, vh, z0, z1) = self.viewport();
        let rhw = 1.0 / c[3];
        let (x, y, z) = (c[0] * rhw, c[1] * rhw, c[2] * rhw);
        let (hw, hh) = (vw as f32 / 2.0, vh as f32 / 2.0);
        (vx as f32 + (1.0 + x) * hw, vy as f32 + (1.0 - y) * hh, z0 + z * (z1 - z0), rhw, c)
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
        let inside = |v: &Vtx| (-0.5..=w as f32 - 0.5).contains(&v.x) && (-0.5..=h as f32 - 0.5).contains(&v.y);
        if !inside(&a) || !inside(&b) {
            // (open: a line clipped lights the pixel its end is in for some lines and not others,
            // and its start's pixel moves, line-random-*)
            return Err(Fail::Unmodeled(open::D3D9_DRAW_POINTSLINES));
        }
        let snap = |v: f32| round_even(v as f64 * 256.0) as i128;
        let l = Seg9 { a: (snap(a.x), snap(a.y)), b: (snap(b.x), snap(b.y)) };
        if l.a == l.b {
            return Ok(());
        }
        hit(clause::D3D9_DRAW_LINES);
        let ymaj = (l.b.1 - l.a.1).abs() > (l.b.0 - l.a.0).abs();
        let mut lit = Vec::new();
        for py in 0..h {
            for px in 0..w {
                let c = (px as i128 * 256, py as i128 * 256);
                match l.meets(c, ymaj) {
                    Meet::No => {}
                    // (open: a line running along diamonds' edges lights some of them and not
                    // others, line-diag-0, -4, line-anti-0, -4)
                    Meet::AlongEdge => return Err(Fail::Unmodeled(open::D3D9_DRAW_POINTSLINES)),
                    Meet::Yes => {
                        if !l.in_diamond(l.b, c, 1, ymaj) {
                            lit.push((px, py));
                        }
                    }
                }
            }
        }
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
            self.pixel(px, py, v[0], diffuse, specular, [v[9], v[10]])?;
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
            let (x, y) = ((v.x - 0.5).ceil() as i64, (v.y - 0.5).ceil() as i64);
            if (0..w as i64).contains(&x) && (0..h as i64).contains(&y) {
                self.pixel(x as i32, y as i32, v.z, v.diffuse, v.specular, v.uv)?;
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
        // (MEASURED, cull-0, cull-4: a mode outside the enumeration culls as CW does on x64,
        // as CCW does on x86)
        let mode = match self.rs(22) {
            m @ (1..=3) => m,
            _ if crate::model::arch() == 32 => {
                hit(clause::D3D9_CULL_BAD);
                3
            }
            m => m,
        };
        match mode {
            1 => {}
            3 if area < 0.0 => return Ok(()),
            3 => {}
            m if area > 0.0 => {
                if m != 2 {
                    hit(clause::D3D9_CULL_BAD);
                }
                return Ok(());
            }
            m => {
                if m != 2 {
                    hit(clause::D3D9_CULL_BAD);
                }
            }
        }
        if area == 0.0 {
            return Ok(());
        }
        self.draw.ccw = area < 0.0;
        let xyz = vs.iter().all(|v| v.clip.is_some());
        if xyz && !self.draw.in_piece {
            // (untransformed vertices: clipped in clip space to the viewport's pixels, the planes
            // half a pixel in, x = vx - 0.5 .. vx + w - 0.5 (as pretransformed vertices are,
            // D3D9.RASTER.CLIP), each new vertex a + (b - a) t; the polygon cut into a fan)
            let (vx, vy, vw, vh, ..) = self.viewport();
            let sh = |v: &Vtx| {
                let c = v.clip.unwrap();
                [c[0] + c[3] / vw as f32, c[1] - c[3] / vh as f32, c[2], c[3]]
            };
            let dist = |q: &[f32; 4], plane: usize| match plane {
                0 => q[3] + q[0],
                1 => q[3] - q[0],
                2 => q[3] + q[1],
                _ => q[3] - q[1],
            };
            let mut poly: Vec<([f32; 4], [f32; ATTRS])> = vs.iter().map(|v| (sh(v), v.attrs())).collect();
            if poly.iter().any(|(q, _)| (0..4).any(|p| dist(q, p) < 0.0)) {
                hit(clause::D3D9_DRAW_XYZ_CLIP);
                for plane in 0..4 {
                    let mut out = Vec::new();
                    for i in 0..poly.len() {
                        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
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
                let vtx = |(q, at): ([f32; 4], [f32; ATTRS])| {
                    let rhw = 1.0 / q[3];
                    Vtx {
                        x: vx as f32 - 0.5 + (1.0 + q[0] * rhw) * (vw as f32 / 2.0),
                        y: vy as f32 - 0.5 + (1.0 - q[1] * rhw) * (vh as f32 / 2.0),
                        z: at[0] as f64,
                        rhw: rhw as f64,
                        diffuse: std::array::from_fn(|k| at[1 + k] as f64),
                        specular: std::array::from_fn(|k| at[5 + k] as f64),
                        uv: [at[9] as f64, at[10] as f64],
                        clip: Some([q[0] - q[3] / vw as f32, q[1] + q[3] / vh as f32, q[2], q[3]]),
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
        // (open: Direct3D 8 leaves some triangles a little past the target unclipped, as a guard
        // band would, and not others: d3d8draw raster-3, -77, -96, shade-13, clip-decide-*)
        if self.v8 && clipped && pts.iter().all(|v| within(v, -1.0, 3.0)) {
            return Err(Fail::Unmodeled(open::D3D9_RASTER_CLIP8));
        }
        let flat = self.rs(9) == 1;
        let mut clipr = (0, 0, w, h);
        if !self.v8 && self.rs(174) != 0 {
            hit(clause::D3D9_SCISSOR);
            if let Some((x0, y0, x1, y1)) = self.draw.scissor {
                clipr = (x0.max(0), y0.max(0), x1.min(w), y1.min(h));
            }
        }
        let corners = vs.map(|v| {
            let mut at = v.attrs();
            if persp {
                // (the attributes times rhw, interpolated in screen space, then divided by the
                // interpolated rhw)
                hit(clause::D3D9_DRAW_RHW);
                for k in PERSP {
                    at[k] *= at[11];
                }
            }
            PolyVtx { x: v.x, y: v.y, at }
        });
        let snap = |v: &PolyVtx| (round_even(v.x as f64 * 256.0) as i64, round_even(v.y as f64 * 256.0) as i64);
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
            // (MEASURED rhw-6: with perspective, the right and bottom sides at the size itself)
            let edge = if persp { 0.0 } else { 0.5 };
            let poly: Vec<PolyVtx> = clip_polygon(&turned, w as f32 - edge, h as f32 - edge)
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
            let area32 = (x[1] - x[0]) * (y[2] - y[0]) - (y[1] - y[0]) * (x[2] - x[0]);
            let grads: [(f32, f32); ATTRS] = std::array::from_fn(|k| {
                let (d1, d2) = (piece[1].at[k] - piece[0].at[k], piece[2].at[k] - piece[0].at[k]);
                ((d1 * (y[2] - y[0]) - d2 * (y[1] - y[0])) / area32, (d2 * (x[1] - x[0]) - d1 * (x[2] - x[0])) / area32)
            });
            self.draw.duv = ((grads[9].0 as f64, grads[9].1 as f64), (grads[10].0 as f64, grads[10].1 as f64));
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
                    let at32: [f32; ATTRS] = std::array::from_fn(|k| {
                        let (dx, dy) = grads[k];
                        piece[0].at[k] + dy * (sy - y[0]) + dx * (sx - x[0])
                    });
                    let at: [f64; ATTRS] = match &planes {
                        Some(p) => p.at(sx, sy),
                        None => std::array::from_fn(|k| if persp && PERSP.contains(&k) { (at32[k] / at32[11]) as f64 } else { at32[k] as f64 }),
                    };
                    let (diffuse, specular) = if flat {
                        hit(clause::D3D9_SHADE_FLAT);
                        (a.diffuse, a.specular)
                    } else {
                        hit(clause::D3D9_SHADE_GOURAUD);
                        (std::array::from_fn(|k| at[1 + k]), std::array::from_fn(|k| at[5 + k]))
                    };
                    self.pixel(px, py, at[0], diffuse, specular, [at[9], at[10]])?;
                }
            }
        }
        Ok(())
    }

    fn pixel(&mut self, x: i32, y: i32, z: f64, diffuse: [f64; 4], specular: [f64; 4], uv: [f64; 2]) -> Res<()> {
        let mut c = self.stages(diffuse, specular, uv)?;
        if self.rs(29) != 0 {
            hit(clause::D3D9_SPECULAR);
            for k in 0..3 {
                c[k] = (c[k] + specular[k]).min(1.0);
            }
        }
        if self.rs(28) != 0 {
            // (MEASURED fog-*: the factor from the iterated specular alpha (FOGTABLEMODE NONE) or
            // EXP, EXP2, LINEAR of the pixel's z; the color f c + (1 - f) fog, in f32)
            hit(clause::D3D9_FOG);
            let fl = |k: u32| f32::from_bits(self.rs(k)) as f64;
            let (start, end, density) = (fl(36), fl(37), fl(38));
            let f = match self.rs(35) {
                0 => specular[3],
                1 => (-(z * density)).exp(),
                2 => (-(z * density) * (z * density)).exp(),
                3 => (end - z) / (end - start),
                // (the generators set no other table mode)
                _ => return Err(Fail::Bad),
            }
            .clamp(0.0, 1.0) as f32;
            let fc = argb(self.rs(34));
            for k in 0..3 {
                c[k] = (f * c[k] as f32 + (1.0 - f) * fc[k] as f32) as f64;
            }
        }
        if self.rs(15) != 0 {
            hit(clause::D3D9_ALPHATEST);
            if !cmp(self.rs(25), unorm(c[3], 8), (self.rs(24) & 0xff) as u64) {
                return Ok(());
            }
        }
        let rt = self.draw.rt.as_ref().unwrap();
        let k = (y * rt.w + x) as usize;
        // stencil, then depth
        let zq = unorm(z, 24);
        let ztest = self.rs(7) != 0;
        let zpass = !ztest || cmp(self.rs(23), zq, self.draw.depth[k] as u64);
        if self.rs(52) != 0 {
            hit(clause::D3D9_STENCIL);
            // (two-sided: a counterclockwise triangle takes the CCW states)
            let (fail, zfail, pass, func) = if !self.v8 && self.rs(185) != 0 && self.draw.ccw {
                hit(clause::D3D9_STENCIL_TWOSIDED);
                (186, 187, 188, 189)
            } else {
                (53, 54, 55, 56)
            };
            let s = self.draw.stencil[k];
            let (refv, mask, wmask) = (self.rs(57) as u8, self.rs(58) as u8, self.rs(59) as u8);
            let spass = cmp(self.rs(func), (refv & mask) as u64, (s & mask) as u64);
            let op = if !spass { self.rs(fail) } else if !zpass { self.rs(zfail) } else { self.rs(pass) };
            let ns = stencil_op(op, s, refv);
            self.draw.stencil[k] = (s & !wmask) | (ns & wmask);
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
        let rt = self.draw.rt.as_ref().unwrap();
        let fm = format(rt.fmt).unwrap();
        let old = to_rgba(rt.fmt, &decode(&fm, rt.px(x, y)));
        let mut new = c;
        if self.rs(27) != 0 {
            hit(clause::D3D9_BLEND);
            new = self.blend(rt.fmt, c, old)?;
        }
        let mask = self.rs(168);
        if mask & 0xf != 0xf {
            hit(clause::D3D9_COLORWRITE);
            // (COLORWRITEENABLE: bit 0 red, 1 green, 2 blue, 3 alpha)
            for (k, bit) in [(0, 1), (1, 2), (2, 4), (3, 8)] {
                if mask & bit == 0 {
                    new[k] = old[k];
                }
            }
        }
        if fm.ch.iter().any(|c| c.c == b'x') {
            hit(clause::D3D9_DRAW_XZERO);
        }
        let px = encode_x(&fm, new, false);
        self.draw.rt.as_mut().unwrap().set(x, y, &px);
        Ok(())
    }

    /// blending on a target of 8-bit unorm channels (MEASURED, d3d9draw blend-*: each product
    /// x * f of two 8-bit numbers as x * f * 257 >> 8, a number in 1/256ths that is a little
    /// under x * f / 255 (but x * ONE as x exactly); the two added or subtracted, then rounded to 8 bits half up)
    /// blending on a target of any other format (MEASURED, d3d9draw blend-fmt-*: 95 cases, every
    /// format the target takes): in f32, the source clamped to 0..1, the destination as read
    /// a blend operation outside ADD..MAX is MAX (MEASURED x64 blend-badop-*; the x86 runtime
    /// dies drawing with one: D3D9.BLEND.OPX86, so it does not get here)
    fn blend_op(op: u32) -> BlendOp {
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

    fn blend_float(&self, s: [f64; 4], d: [f64; 4]) -> Res<[f64; 4]> {
        let s = s.map(|x| x.clamp(0.0, 1.0) as f32);
        let d = d.map(|x| x as f32);
        // (Direct3D 8 has no BLENDFACTOR state, yet BLENDFACTOR and INVBLENDFACTOR blend with
        // white there: MEASURED d3d8draw blend-factor8)
        let c = if self.v8 { 0xffff_ffff } else { self.rs(193) };
        if self.v8 && [19, 20].iter().any(|&i| matches!(self.rs(i), 14 | 15)) {
            hit(clause::D3D9_BLEND_FACTOR);
        }
        let bf = argb(c).map(|v| v as f32);
        let factor = |f: u32, k: usize| -> Res<f32> {
            Ok(match f {
                1 => 0.0,
                2 => 1.0,
                3 => s[k],
                4 => 1.0 - s[k],
                5 => s[3],
                6 => 1.0 - s[3],
                7 => d[3],
                8 => 1.0 - d[3],
                9 => d[k],
                10 => 1.0 - d[k],
                11 if k < 3 => s[3].min(1.0 - d[3]),
                11 => 1.0,
                14 => bf[k],
                15 => 1.0 - bf[k],
                _ => return Err(Fail::Bad),
            })
        };
        let separate = !self.v8 && self.rs(206) != 0;
        let mut out = [0.0; 4];
        for k in 0..4 {
            let alpha = separate && k == 3;
            let (mut sf, mut df, op) = if alpha { (self.rs(207), self.rs(208), self.rs(209)) } else { (self.rs(19), self.rs(20), self.rs(171)) };
            if [sf, df].iter().any(|f| matches!(f, 12 | 13)) {
                if alpha {
                    hit(clause::D3D9_BLEND_BOTHALPHA);
                    let zero = |f: u32| if matches!(f, 12 | 13) { 1 } else { f };
                    (sf, df) = (zero(sf), zero(df));
                } else {
                    (sf, df) = if sf == 12 || sf != 13 && df == 12 { (5, 6) } else { (6, 5) };
                }
            }
            let (a, b) = (s[k] * factor(sf, k)?, d[k] * factor(df, k)?);
            out[k] = match Self::blend_op(op) {
                BlendOp::Add => a + b,
                BlendOp::Sub => a - b,
                BlendOp::RevSub => b - a,
                BlendOp::Min => s[k].min(d[k]),
                BlendOp::Max => s[k].max(d[k]),
            } as f64;
        }
        Ok(out)
    }

    fn blend(&self, fmt: u32, s: [f64; 4], d: [f64; 4]) -> Res<[f64; 4]> {
        let fm = format(fmt).unwrap();
        if fm.kind != Kind::Int || fm.ch.iter().any(|c| c.c != b'x' && (c.bits != 8 || !matches!(c.c, b'r' | b'g' | b'b' | b'a'))) {
            hit(clause::D3D9_BLEND_FORMAT);
            return self.blend_float(s, d);
        }
        let (s, d) = (s.map(|x| unorm(x, 8) as i64), d.map(|x| unorm(x, 8) as i64));
        // (Direct3D 8 has no BLENDFACTOR state, yet BLENDFACTOR and INVBLENDFACTOR blend with
        // white there: MEASURED d3d8draw blend-factor8)
        let c = if self.v8 { 0xffff_ffff } else { self.rs(193) };
        if self.v8 && [19, 20].iter().any(|&i| matches!(self.rs(i), 14 | 15)) {
            hit(clause::D3D9_BLEND_FACTOR);
        }
        let bf = [c >> 16 & 255, c >> 8 & 255, c & 255, c >> 24].map(|v| v as i64);
        let factor = |f: u32, k: usize| -> Res<i64> {
            Ok(match f {
                1 => 0,
                2 => 255,
                3 => s[k],
                4 => 255 - s[k],
                5 => s[3],
                6 => 255 - s[3],
                7 => d[3],
                8 => 255 - d[3],
                9 => d[k],
                10 => 255 - d[k],
                11 if k < 3 => s[3].min(255 - d[3]),
                11 => 255,
                14 => bf[k],
                15 => 255 - bf[k],
                _ => return Err(Fail::Bad),
            })
        };
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
            // BOTHSRCALPHA, BOTHINVSRCALPHA as either factor: both factors from the source's alpha
            // (MEASURED: blend-dest-12, blend-dest-13)
            if [sf, df].iter().any(|f| matches!(f, 12 | 13)) {
                if alpha {
                    // (as a separate alpha factor, either is ZERO: MEASURED blend-bothalpha-*,
                    // blend-random-5, -31)
                    hit(clause::D3D9_BLEND_BOTHALPHA);
                    let zero = |f: u32| if matches!(f, 12 | 13) { 1 } else { f };
                    (sf, df) = (zero(sf), zero(df));
                } else {
                    (sf, df) = if sf == 12 || sf != 13 && df == 12 { (5, 6) } else { (6, 5) };
                }
            }
            let op = Self::blend_op(op);
            let (fs, fd) = (factor(sf, k)?, factor(df, k)?);
            // (ONE exactly: MEASURED, d3d8draw blend-random-2)
            let prod = |x: i64, f: u32, v: i64| if f == 2 { x << 8 } else { x * v * 257 >> 8 };
            let (a, b) = (prod(s[k], sf, fs), prod(d[k], df, fd));
            let v = match op {
                BlendOp::Add => a + b,
                BlendOp::Sub => a - b,
                BlendOp::RevSub => b - a,
                BlendOp::Min => s[k].min(d[k]) << 8,
                BlendOp::Max => s[k].max(d[k]) << 8,
            };
            let r = (v.max(0) + 128 >> 8).min(255);
            let n = match op {
                BlendOp::Add => Some(s[k] * fs + d[k] * fd),
                BlendOp::Sub => Some(s[k] * fs - d[k] * fd),
                BlendOp::RevSub => Some(d[k] * fd - s[k] * fs),
                BlendOp::Min | BlendOp::Max => None,
            };
            let r = match n {
                Some(n) if exact => {
                    hit(clause::D3D9_BLEND_TIE);
                    ((2 * n.max(0) + 255) / 510).min(255)
                }
                _ => r,
            };
            out[k] = (r as f32 / 255.0) as f64;
        }
        Ok(out)
    }
}
