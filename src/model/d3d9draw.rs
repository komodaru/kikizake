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
    hit("D3D9.RT.F16");
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
    hit("D3D9.TEX.MISSING");
    let miss_rgb = if c.iter().take(3).all(Option::is_none) { 0.0 } else { 1.0 };
    [c[0].unwrap_or(miss_rgb), c[1].unwrap_or(miss_rgb), c[2].unwrap_or(miss_rgb), c[3].unwrap_or(1.0)]
}

/// rgba -> one pixel's bytes of a render target format
pub fn encode(f: &Fmt, rgba: [f64; 4]) -> Vec<u8> {
    let mut w: u128 = 0;
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

/// float -> n-bit unsigned normalized: clamped, scaled, rounded to nearest (D3D9.RT.UNORM)
pub fn unorm(v: f64, bits: u32) -> u64 {
    hit("D3D9.RT.UNORM");
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

#[derive(Clone, Debug, Default)]
pub struct Draw {
    pub rt: Option<Surface>,
    /// the depth (24 bits) and stencil of each pixel
    pub depth: Vec<u32>,
    pub stencil: Vec<u8>,
    pub texs: [Option<Surface>; 8],
    pub stage_tex: [Option<usize>; 8],
    pub fvf: u32,
    pub scissor: Option<(i32, i32, i32, i32)>,
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
    hit("D3D9.FORMATS");
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
}

/// a D3DCOLOR's channels (r, g, b, a) as f32 c / 255 (MEASURED: so the pipeline reads them)
fn argb(c: u32) -> [f64; 4] {
    [c >> 16 & 255, c >> 8 & 255, c & 255, c >> 24].map(|v| (v as f32 / 255.0) as f64)
}

fn cross(p: [(f64, f64); 3]) -> f64 {
    (p[1].0 - p[0].0) * (p[2].1 - p[0].1) - (p[1].1 - p[0].1) * (p[2].0 - p[0].0)
}

/// Sutherland-Hodgman against x0 <= x <= x1, y0 <= y <= y1, in f32: a new vertex is
/// p + t (q - p), t = dp / (dp - dq) (MEASURED: D3D9.RASTER.CLIP)
pub fn clip(pts: &[(f32, f32)], x0: f32, x1: f32, y0: f32, y1: f32) -> Vec<(f32, f32)> {
    let mut poly = pts.to_vec();
    for plane in 0..4 {
        let d = |v: &(f32, f32)| match plane {
            0 => v.0 - x0,
            1 => x1 - v.0,
            2 => v.1 - y0,
            _ => y1 - v.1,
        };
        let mut out = Vec::new();
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let (dp, dq) = (d(&p), d(&q));
            if dp >= 0.0 {
                out.push(p);
            }
            if (dp >= 0.0) != (dq >= 0.0) {
                let t = dp / (dp - dq);
                out.push((p.0 + t * (q.0 - p.0), p.1 + t * (q.1 - p.1)));
            }
        }
        poly = out;
        if poly.is_empty() {
            break;
        }
    }
    poly
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

/// the fill convention on a triangle of f32 vertices, unsnapped (f64 edge functions: exact for
/// these magnitudes)
fn cover_f(t: &[(f32, f32); 3], w: i32, h: i32, covered: &mut [bool]) {
    let t: Vec<(f64, f64)> = t.iter().map(|p| (p.0 as f64, p.1 as f64)).collect();
    let area = cross([t[0], t[1], t[2]]);
    if area == 0.0 {
        return;
    }
    let t = if area < 0.0 { [t[0], t[2], t[1]] } else { [t[0], t[1], t[2]] };
    for py in 0..h {
        for px in 0..w {
            let (sx, sy) = (px as f64, py as f64);
            let ok = (0..3).all(|k| {
                let (p, q) = (t[k], t[(k + 1) % 3]);
                let e = (q.0 - p.0) * (sy - p.1) - (q.1 - p.1) * (sx - p.0);
                e > 0.0 || (e == 0.0 && (q.1 - p.1 < 0.0 || (q.1 == p.1 && q.0 - p.0 > 0.0)))
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
                hit("D3D9.TARGET");
                if !supported(f, true, self.v8) {
                    // (and the device's own back buffer stays the target: D3D9.CLEAR.BACKBUFFER)
                    self.draw.rt = None;
                    out.push(format!("target failed {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let fm = format(f).ok_or(Fail::Unmodeled("D3D9.FORMATS"))?;
                self.draw.rt = Some(Surface { w, h, fmt: f, bytes: vec![0; (w * h) as usize * fm.bytes] });
                self.draw.depth = vec![0; (w * h) as usize];
                self.draw.stencil = vec![0; (w * h) as usize];
                // (Direct3D 9: SetRenderTarget resets the viewport and the scissor rectangle to the target)
                self.draw.scissor = None;
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
                hit("D3D9.CLEAR");
                if flags & 7 == 0 {
                    hit("D3D9.CLEAR.FLAGS");
                    out.push(format!("clear hr {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let Some(rt) = &mut self.draw.rt else {
                    hit("D3D9.CLEAR.BACKBUFFER");
                    return Ok(true);
                };
                let (w, h) = (rt.w, rt.h);
                // (MEASURED: with the scissor test on, Clear keeps to the scissor rectangle too)
                let (sx0, sy0, sx1, sy1) = match self.draw.scissor {
                    Some(r) if !self.v8 && self.state.get(&Key::Rs(174)).copied().unwrap_or(0) != 0 => {
                        hit("D3D9.SCISSOR");
                        r
                    }
                    _ => (0, 0, w, h),
                };
                let all = [(0, 0, w, h)];
                let list: &[(i32, i32, i32, i32)] = if rects.is_empty() { &all } else { &rects };
                let fm = format(rt.fmt).unwrap();
                hit("D3D9.CLEAR.COLOR");
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
                hit("D3D9.RT.READ");
                let b = format(rt.fmt).unwrap().bytes;
                out.push(format!("px rt {} {} {}", rt.w, rt.h, b * 8));
                for row in rt.bytes.chunks(rt.w as usize * b) {
                    out.push(row.iter().map(|x| format!("{x:02x}")).collect());
                }
            }
            "d9fvf" => {
                self.draw.fvf = n(op, 1)? as u32;
                hit("D3D9.FVF.SET");
            }
            "d9tex" => {
                let (i, w, h, f) = (n(op, 1)? as usize, n(op, 2)? as i32, n(op, 3)? as i32, n(op, 4)? as u32);
                hit("D3D9.TEX.CREATE");
                if !supported(f, false, self.v8) {
                    self.draw.texs[i] = None;
                    out.push(format!("tex failed {INVALIDCALL:08x}"));
                    return Ok(true);
                }
                let fm = format(f).ok_or(Fail::Unmodeled("D3D9.FORMATS"))?;
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
                self.draw_prims(ty, nv, &words)?;
            }
            "d9getfvf" | "d9getdecl" | "d9decl" | "d9check" | "d9viewport" | "d9xform" => return Err(Fail::Unmodeled("D3D9.DRAW.PENDING")),
            _ => return Err(Fail::Bad),
        }
        Ok(true)
    }
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
    hit("D3D9.TEX.ADDRESS");
    Ok(match mode {
        1 => Some(i.rem_euclid(n)),
        2 => {
            let k = i.rem_euclid(2 * n);
            Some(if k < n { k } else { 2 * n - 1 - k })
        }
        3 => Some(i.clamp(0, n - 1)),
        4 => (0..n).contains(&i).then_some(i),
        5 => Some(if i < 0 { -i - 1 } else { i }.clamp(0, n - 1)),
        _ => return Err(Fail::Unmodeled("D3D9.TEX.ADDRESS")),
    })
}

impl D3d9 {
    fn texel(&self, tx: &Surface, x: i64, y: i64, mu: u32, mv: u32) -> Res<[f64; 4]> {
        let (Some(x), Some(y)) = (address(mu, x, tx.w as i64)?, address(mv, y, tx.h as i64)?) else {
            hit("D3D9.TEX.BORDER");
            return Ok(argb(self.samp(0, 4)));
        };
        let fm = format(tx.fmt).unwrap();
        match fm.kind {
            Kind::Int | Kind::F16 | Kind::F32 if !fm.ch.iter().any(|c| c.c == b'p') => {}
            _ => return Err(Fail::Unmodeled("D3D9.TEX.FORMAT")),
        }
        hit("D3D9.TEX.DECODE");
        Ok(to_rgba(tx.fmt, &decode(&fm, tx.px(x as i32, y as i32))))
    }

    /// stage 0's texture at (u, v)
    fn sample(&self, tx: &Surface, u: f64, v: f64) -> Res<[f64; 4]> {
        let (mu, mv) = (self.samp(0, 1), self.samp(0, 2));
        let (mag, min) = (self.samp(0, 5), self.samp(0, 6));
        if mag != min {
            return Err(Fail::Unmodeled("D3D9.TEX.FILTER"));
        }
        let (x, y) = (u * tx.w as f64, v * tx.h as f64);
        match mag {
            1 => {
                hit("D3D9.TEX.POINT");
                self.texel(tx, x.floor() as i64, y.floor() as i64, mu, mv)
            }
            2 => {
                hit("D3D9.TEX.LINEAR");
                // (MEASURED, filter-1 and filter-3: weights in 16ths of a texel, the weighted sum of
                // 8-bit texels truncated; the 16ths taken down (ASSUMED))
                let (x, y) = (x - 0.5, y - 0.5);
                let (x0, y0) = (x.floor(), y.floor());
                let (fx, fy) = (((x - x0) * 16.0).floor() as i64, ((y - y0) * 16.0).floor() as i64);
                let (x0, y0) = (x0 as i64, y0 as i64);
                let t = [self.texel(tx, x0, y0, mu, mv)?, self.texel(tx, x0 + 1, y0, mu, mv)?, self.texel(tx, x0, y0 + 1, mu, mv)?, self.texel(tx, x0 + 1, y0 + 1, mu, mv)?];
                let w = [(16 - fx) * (16 - fy), fx * (16 - fy), (16 - fx) * fy, fx * fy];
                let fm = format(tx.fmt).unwrap();
                let bytes = fm.kind == Kind::Int && fm.ch.iter().all(|c| c.bits == 8 && !matches!(c.c, b'u' | b'v' | b'w' | b'q'));
                let mut c = [0.0; 4];
                for k in 0..4 {
                    c[k] = if bytes {
                        let v: i64 = (0..4).map(|i| unorm(t[i][k], 8) as i64 * w[i]).sum();
                        ((v >> 8) as f32 / 255.0) as f64
                    } else {
                        (0..4).map(|i| t[i][k] * w[i] as f64).sum::<f64>() / 256.0
                    };
                }
                Ok(c)
            }
            _ => Err(Fail::Unmodeled("D3D9.TEX.FILTER")),
        }
    }

    /// a stage argument (D3DTA) as rgba
    fn arg(&self, a: u32, diffuse: [f64; 4], specular: [f64; 4], tex: Option<[f64; 4]>) -> Res<[f64; 4]> {
        let mut v = match a & 0xf {
            0 | 1 => diffuse, // (stage 0: CURRENT is the diffuse color)
            // (MEASURED: with no texture, color 1 and the current alpha)
            2 => tex.unwrap_or_else(|| {
                hit("D3D9.STAGE.NOTEX");
                [1.0, 1.0, 1.0, diffuse[3]]
            }),
            3 => argb(self.rs(60)),
            4 => specular,
            _ => return Err(Fail::Unmodeled("D3D9.STAGE.ARG")),
        };
        if a & 0x20 != 0 {
            v = [v[3]; 4];
        }
        if a & 0x10 != 0 {
            v = v.map(|x| 1.0 - x);
        }
        if a & !0x3f != 0 {
            return Err(Fail::Unmodeled("D3D9.STAGE.ARG"));
        }
        Ok(v)
    }

    /// stage 0's color and alpha operations (D3DTEXTUREOP), stage 1 disabled
    fn stages(&self, diffuse: [f64; 4], specular: [f64; 4], tex: Option<[f64; 4]>) -> Res<[f64; 4]> {
        let (cop, aop) = (self.tss(0, 1), self.tss(0, 4));
        if cop == 1 {
            hit("D3D9.STAGE.DISABLE");
            return Ok(diffuse);
        }
        if self.tss(1, 1) != 1 || self.tss(0, 28) != 1 {
            return Err(Fail::Unmodeled("D3D9.STAGE.MULTI"));
        }
        if aop == 1 {
            return Err(Fail::Unmodeled("D3D9.STAGE.ALPHADISABLE"));
        }
        let mut out = [0.0; 4];
        for (k, (op, a0, a1, a2)) in [(cop, 26, 2, 3), (aop, 27, 5, 6)].into_iter().enumerate() {
            // (only the arguments the operation reads: SELECTARG1 reads no second one)
            let get = |used: bool, a: u32| if used { self.arg(self.tss(0, a), diffuse, specular, tex) } else { Ok([0.0; 4]) };
            let a0 = get(matches!(op, 25 | 26), a0)?;
            let a1 = get(op != 3, a1)?;
            let a2 = get(op != 2, a2)?;
            let chans: &[usize] = if k == 0 { &[0, 1, 2] } else { &[3] };
            if k == 0 && op == 24 {
                // DOTPRODUCT3: into every channel, alpha too
                hit("D3D9.STAGE.OP");
                let d = 4.0 * (0..3).map(|i| (a1[i] - 0.5) * (a2[i] - 0.5)).sum::<f64>();
                return Ok([d.clamp(0.0, 1.0); 4]);
            }
            // (MEASURED: in f32)
            let (a0, a1, a2) = (a0.map(|x| x as f32), a1.map(|x| x as f32), a2.map(|x| x as f32));
            let da = diffuse[3] as f32;
            for &i in chans {
                let alpha = k == 1;
                let r: f32 = match op {
                    2 => a1[i],
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
                    16 => a1[i] * da + a2[i] * (1.0 - da),
                    18 if !alpha => a1[i] + a1[3] * a2[i],
                    19 if !alpha => a1[i] * a2[i] + a1[3],
                    20 if !alpha => (1.0 - a1[3]) * a2[i] + a1[i],
                    21 if !alpha => (1.0 - a1[i]) * a2[i] + a1[3],
                    25 => a0[i] + a1[i] * a2[i],
                    26 => a0[i] * a1[i] + (1.0 - a0[i]) * a2[i],
                    _ => return Err(Fail::Unmodeled("D3D9.STAGE.OPEN")),
                };
                hit("D3D9.STAGE.OP");
                out[i] = r.clamp(0.0, 1.0) as f64;
            }
        }
        Ok(out)
    }

    fn draw_prims(&mut self, ty: u32, nv: usize, words: &[u32]) -> Res<()> {
        let fvf = self.draw.fvf;
        if fvf & 0x400e != 4 {
            return Err(Fail::Unmodeled("D3D9.DRAW.XYZ"));
        }
        let ntex = (fvf >> 8 & 0xf) as usize;
        let tsize = |i: usize| [2, 3, 4, 1][(fvf >> (16 + 2 * i) & 3) as usize];
        let size = 4 + (fvf >> 4 & 1) as usize * 3 + (fvf >> 5 & 1) as usize + (fvf >> 6 & 1) as usize + (fvf >> 7 & 1) as usize + (0..ntex).map(tsize).sum::<usize>();
        if words.len() / nv != size {
            return Err(Fail::Unmodeled("D3D9.DRAW.STRIDE"));
        }
        let vs: Vec<Vtx> = words
            .chunks(size)
            .map(|w| {
                let mut i = 4 + (fvf >> 4 & 1) as usize * 3 + (fvf >> 5 & 1) as usize;
                let mut take = |on: bool| {
                    let v = if on { Some(w[i]) } else { None };
                    i += on as usize;
                    v
                };
                let diffuse = argb(take(fvf & 0x40 != 0).unwrap_or(0xffff_ffff));
                let specular = argb(take(fvf & 0x80 != 0).unwrap_or(0));
                let uv = if ntex > 0 && tsize(0) >= 2 { [fl(w[i]), fl(w[i + 1])] } else { [0.0; 2] };
                Vtx { x: f32::from_bits(w[0]), y: f32::from_bits(w[1]), z: fl(w[2]), rhw: fl(w[3]), diffuse, specular, uv }
            })
            .collect();
        // the triangles, each with its first (flat shading) vertex first
        let tris: Vec<[usize; 3]> = match ty {
            4 => (0..nv / 3).map(|i| [3 * i, 3 * i + 1, 3 * i + 2]).collect(),
            5 => (0..nv.saturating_sub(2)).map(|i| if i % 2 == 0 { [i, i + 1, i + 2] } else { [i, i + 2, i + 1] }).collect(),
            6 => (0..nv.saturating_sub(2)).map(|i| [i + 1, i + 2, 0]).collect(),
            _ => return Err(Fail::Unmodeled("D3D9.DRAW.POINTSLINES")),
        };
        hit("D3D9.DRAW.ASSEMBLY");
        for t in tris {
            self.triangle(vs[t[0]], vs[t[1]], vs[t[2]])?;
        }
        Ok(())
    }

    fn triangle(&mut self, a: Vtx, b: Vtx, c: Vtx) -> Res<()> {
        let Some(rt) = &self.draw.rt else {
            hit("D3D9.CLEAR.BACKBUFFER");
            return Ok(());
        };
        let (w, h) = (rt.w, rt.h);
        if [a, b, c].iter().any(|v| v.rhw != 1.0) {
            return Err(Fail::Unmodeled("D3D9.DRAW.RHW"));
        }
        let vs = [a, b, c];
        // the winding (culling) of the vertices as given
        let area = cross([(a.x as f64, a.y as f64), (b.x as f64, b.y as f64), (c.x as f64, c.y as f64)]);
        hit("D3D9.CULL");
        match self.rs(22) {
            1 => {}
            2 if area > 0.0 => return Ok(()),
            3 if area < 0.0 => return Ok(()),
            2 | 3 => {}
            _ => return Err(Fail::Unmodeled("D3D9.CULL")),
        }
        if area == 0.0 {
            return Ok(());
        }
        // coverage: the triangle clipped to the target, the vertices snapped to 1/256 pixel
        let inside = |v: &(f32, f32)| (-0.5..=w as f32 - 0.5).contains(&v.0) && (-0.5..=h as f32 - 0.5).contains(&v.1);
        let pts = [(a.x, a.y), (b.x, b.y), (c.x, c.y)];
        let poly = if pts.iter().all(inside) {
            pts.to_vec()
        } else {
            hit("D3D9.RASTER.CLIP");
            clip(&pts, -0.5, w as f32 - 0.5, -0.5, h as f32 - 0.5)
        };
        let snapped: Vec<(i64, i64)> = poly.iter().map(|p| (round_even(p.0 as f64 * 256.0) as i64, round_even(p.1 as f64 * 256.0) as i64)).collect();
        let mut covered = vec![false; (w * h) as usize];
        for i in 1..snapped.len().saturating_sub(1) {
            cover([snapped[0], snapped[i], snapped[i + 1]], w, h, &mut covered);
        }
        if !pts.iter().all(inside) {
            // (open: a pixel that the rounding of the new vertices decides: covered once snapped
            // and not by the clipped polygon itself, or the other way round)
            let mut exact = vec![false; (w * h) as usize];
            for i in 1..poly.len().saturating_sub(1) {
                cover_f(&[poly[0], poly[i], poly[i + 1]], w, h, &mut exact);
            }
            if exact != covered {
                return Err(Fail::Unmodeled("D3D9.RASTER.CLIPTIE"));
            }
        }
        hit("D3D9.RASTER.TOPLEFT");
        let flat = self.rs(9) == 1;
        // (open: Direct3D 8 shades a clipped triangle a little differently, d3d8draw prims-3 and
        // shade-9)
        let varies = !flat && vs.iter().any(|v| v.diffuse != a.diffuse || v.specular != a.specular);
        if self.v8 && varies && !pts.iter().all(inside) {
            return Err(Fail::Unmodeled("D3D9.RASTER.CLIP8"));
        }
        let mut clipr = (0, 0, w, h);
        if !self.v8 && self.rs(174) != 0 {
            hit("D3D9.SCISSOR");
            if let Some((x0, y0, x1, y1)) = self.draw.scissor {
                clipr = (x0.max(0), y0.max(0), x1.min(w), y1.min(h));
            }
        }
        // the attributes: MEASURED (Gouraud shading on 15 triangles, 6072 channel values, every
        // one): in f32, positions with the half pixel added (D3D10's pixel centers), the
        // barycentric weights each an edge function over the area, v0 + w2 (v2 - v0) + w1 (v1 - v0)
        // (of the whole triangle, clipped or not: interpolating over the clipped polygon's pieces
        // changes none of the pixels that differ, d3d8draw prims-3 and shade-9)
        let p32: Vec<(f32, f32)> = vs.iter().map(|v| (v.x + 0.5, v.y + 0.5)).collect();
        let area32 = (p32[1].0 - p32[0].0) * (p32[2].1 - p32[0].1) - (p32[1].1 - p32[0].1) * (p32[2].0 - p32[0].0);
        let e32 = |i: usize, j: usize, sx: f32, sy: f32| (p32[j].0 - p32[i].0) * (sy - p32[i].1) - (p32[j].1 - p32[i].1) * (sx - p32[i].0);
        for py in clipr.1..clipr.3 {
            for px in clipr.0..clipr.2 {
                if !covered[(py * w + px) as usize] {
                    continue;
                }
                let (sx, sy) = (px as f32 + 0.5, py as f32 + 0.5);
                let (w1, w2) = (e32(2, 0, sx, sy) / area32, e32(0, 1, sx, sy) / area32);
                let lerp = |f: &dyn Fn(&Vtx) -> f32| {
                    let (a0, a1, a2) = (f(&vs[0]), f(&vs[1]), f(&vs[2]));
                    ((a0 + w2 * (a2 - a0)) + w1 * (a1 - a0)) as f64
                };
                let z = lerp(&|v| v.z as f32);
                let (diffuse, specular) = if flat {
                    hit("D3D9.SHADE.FLAT");
                    (a.diffuse, a.specular)
                } else {
                    hit("D3D9.SHADE.GOURAUD");
                    (std::array::from_fn(|k| lerp(&|v| v.diffuse[k] as f32)), std::array::from_fn(|k| lerp(&|v| v.specular[k] as f32)))
                };
                let uv = [lerp(&|v| v.uv[0] as f32), lerp(&|v| v.uv[1] as f32)];
                self.pixel(px, py, z, diffuse, specular, uv)?;
            }
        }
        Ok(())
    }

    fn pixel(&mut self, x: i32, y: i32, z: f64, diffuse: [f64; 4], specular: [f64; 4], uv: [f64; 2]) -> Res<()> {
        let tex = match self.draw.stage_tex[0] {
            Some(i) if self.tss(0, 1) != 1 => Some(self.sample(self.draw.texs[i].as_ref().unwrap(), uv[0], uv[1])?),
            _ => None,
        };
        let mut c = self.stages(diffuse, specular, tex)?;
        if self.rs(29) != 0 {
            hit("D3D9.SPECULAR");
            for k in 0..3 {
                c[k] = (c[k] + specular[k]).min(1.0);
            }
        }
        if self.rs(28) != 0 {
            return Err(Fail::Unmodeled("D3D9.FOG"));
        }
        if self.rs(15) != 0 {
            hit("D3D9.ALPHATEST");
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
        if self.rs(185) != 0 {
            return Err(Fail::Unmodeled("D3D9.STENCIL.TWOSIDED"));
        }
        if self.rs(52) != 0 {
            hit("D3D9.STENCIL");
            let s = self.draw.stencil[k];
            let (refv, mask, wmask) = (self.rs(57) as u8, self.rs(58) as u8, self.rs(59) as u8);
            let spass = cmp(self.rs(56), (refv & mask) as u64, (s & mask) as u64);
            let op = if !spass { self.rs(53) } else if !zpass { self.rs(54) } else { self.rs(55) };
            let ns = stencil_op(op, s, refv);
            self.draw.stencil[k] = (s & !wmask) | (ns & wmask);
            if !spass {
                return Ok(());
            }
        }
        if !zpass {
            hit("D3D9.DEPTH");
            return Ok(());
        }
        if ztest {
            hit("D3D9.DEPTH");
            if self.rs(14) != 0 {
                self.draw.depth[k] = zq as u32;
            }
        }
        let rt = self.draw.rt.as_ref().unwrap();
        let fm = format(rt.fmt).unwrap();
        let old = to_rgba(rt.fmt, &decode(&fm, rt.px(x, y)));
        let mut new = c;
        if self.rs(27) != 0 {
            hit("D3D9.BLEND");
            new = self.blend(rt.fmt, c, old)?;
        }
        let mask = self.rs(168);
        if mask & 0xf != 0xf {
            hit("D3D9.COLORWRITE");
            // (COLORWRITEENABLE: bit 0 red, 1 green, 2 blue, 3 alpha)
            for (k, bit) in [(0, 1), (1, 2), (2, 4), (3, 8)] {
                if mask & bit == 0 {
                    new[k] = old[k];
                }
            }
        }
        let px = encode(&fm, new);
        self.draw.rt.as_mut().unwrap().set(x, y, &px);
        Ok(())
    }

    /// blending on a target of 8-bit unorm channels (MEASURED, d3d9draw blend-*: each product
    /// x * f of two 8-bit numbers as x * f * 257 >> 8, a number in 1/256ths that is a little
    /// under x * f / 255 (but x * ONE as x exactly); the two added or subtracted, then rounded to 8 bits half up)
    fn blend(&self, fmt: u32, s: [f64; 4], d: [f64; 4]) -> Res<[f64; 4]> {
        let fm = format(fmt).unwrap();
        if fm.kind != Kind::Int || fm.ch.iter().any(|c| c.c != b'x' && (c.bits != 8 || !matches!(c.c, b'r' | b'g' | b'b' | b'a'))) {
            return Err(Fail::Unmodeled("D3D9.BLEND.FORMAT"));
        }
        let (s, d) = (s.map(|x| unorm(x, 8) as i64), d.map(|x| unorm(x, 8) as i64));
        let c = self.rs(193);
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
                14 if !self.v8 => bf[k],
                15 if !self.v8 => 255 - bf[k],
                _ => return Err(Fail::Unmodeled("D3D9.BLEND.FACTOR")),
            })
        };
        let separate = !self.v8 && self.rs(206) != 0;
        let mut out = [0.0; 4];
        for k in 0..4 {
            let alpha = separate && k == 3;
            let (mut sf, mut df, op) = if alpha { (self.rs(207), self.rs(208), self.rs(209)) } else { (self.rs(19), self.rs(20), self.rs(171)) };
            // BOTHSRCALPHA, BOTHINVSRCALPHA as either factor: both factors from the source's alpha
            // (MEASURED: blend-dest-12, blend-dest-13)
            if [sf, df].iter().any(|f| matches!(f, 12 | 13)) {
                if alpha {
                    return Err(Fail::Unmodeled("D3D9.BLEND.BOTHALPHA"));
                }
                (sf, df) = if sf == 12 || sf != 13 && df == 12 { (5, 6) } else { (6, 5) };
            }
            let (fs, fd) = (factor(sf, k)?, factor(df, k)?);
            // (ONE exactly: MEASURED, d3d8draw blend-random-2)
            let prod = |x: i64, f: u32, v: i64| if f == 2 { x << 8 } else { x * v * 257 >> 8 };
            let (a, b) = (prod(s[k], sf, fs), prod(d[k], df, fd));
            let v = match op {
                1 => a + b,
                2 => a - b,
                3 => b - a,
                4 => s[k].min(d[k]) << 8,
                5 => s[k].max(d[k]) << 8,
                _ => return Err(Fail::Unmodeled("D3D9.BLEND.OP")),
            };
            let r = (v.max(0) + 128 >> 8).min(255);
            if op <= 3 {
                // (open: where the 1/256ths and the exact sum round apart, 23 of 43624 values,
                // Windows takes either)
                let n = match op {
                    1 => s[k] * fs + d[k] * fd,
                    2 => s[k] * fs - d[k] * fd,
                    _ => d[k] * fd - s[k] * fs,
                };
                if (2 * n.max(0) + 255) / 510 != r {
                    return Err(Fail::Unmodeled("D3D9.BLEND.TIE"));
                }
            }
            out[k] = (r as f32 / 255.0) as f64;
        }
        Ok(out)
    }
}
