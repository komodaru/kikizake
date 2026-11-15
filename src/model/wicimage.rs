//! WIC bitmap sources other than the format converter (runner/wicimage.c): bitmaps in memory,
//! scalers, flip/rotators, clippers, CopyPixels, predefined palettes, color contexts and
//! transforms, the decoders (BMP, PNG, GIF, TIFF, ICO, DDS) and the encoders.
//!
//! Sources (Microsoft Learn): the WIC interface pages (IWICBitmapScaler, IWICBitmapFlipRotator,
//! IWICBitmapClipper, IWICBitmapSource::CopyPixels, IWICPalette::InitializePredefined,
//! IWICColorContext, IWICColorTransform), the enumerations (WICBitmapPaletteType,
//! WICBitmapInterpolationMode, WICBitmapTransformOptions), "Native Pixel Formats Overview" (each
//! codec's pixel formats) and the codecs' overview pages; the file formats from their own
//! specifications (BMP: the BITMAPINFOHEADER pages; PNG: ISO/IEC 15948; GIF89a; TIFF 6.0; DDS:
//! "Programming Guide for DDS" and the block compression page). spec/wicimage.txt has the clauses.

use crate::spec::{clause, open};
use super::{Fail, Res};
use crate::spec::hit;

pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const E_NOTIMPL: u32 = 0x8000_4001;
pub const PALETTE_UNAVAILABLE: u32 = 0x8898_2f45;
pub const INSUFFICIENT_BUFFER: u32 = 0x8898_2f8c;
pub const ARITHMETIC_OVERFLOW: u32 = 0x8007_0216;
pub const STREAM_READ: u32 = 0x8898_2f72;
pub const BAD_IMAGE: u32 = 0x8898_2f60;
pub const UNKNOWN_IMAGE_FORMAT: u32 = 0x8898_2f07;
pub const COMPONENT_NOT_FOUND: u32 = 0x8898_2f50;

/// every pixel format the executor names: (name, bits a pixel); the first 23 are model::wic's
pub const FORMATS: &[(&str, u32)] = &[
    ("bw", 1), ("gray2", 2), ("gray4", 4), ("gray8", 8), ("gray16", 16), ("bgr555", 16), ("bgr565", 16), ("bgr24", 24),
    ("rgb24", 24), ("bgr32", 32), ("bgra", 32), ("pbgra", 32), ("rgb32", 32), ("rgba", 32), ("prgba", 32), ("rgb48", 48),
    ("rgba64", 64), ("cmyk", 32), ("alpha8", 8), ("idx1", 1), ("idx2", 2), ("idx4", 4), ("idx8", 8),
    ("bgr48", 48), ("bgra64", 64), ("prgba64", 64), ("pbgra64", 64), ("bgr101010", 32), ("grayfloat", 32), ("rgbafloat", 128),
    ("prgbafloat", 128), ("rgbfloat", 128), ("rgba64fixed", 64), ("bgra64fixed", 64), ("rgba64half", 64), ("rgb96float", 96),
    ("cmyk64", 64), ("cmyka40", 40), ("cmyka80", 80), ("grayfix16", 16), ("grayhalf", 16), ("grayfix32", 32),
    ("rgb48fixed", 48), ("rgb48half", 48), ("rgb64fixed", 64), ("rgb64half", 64), ("rgba128fixed", 128), ("rgb128fixed", 128),
    ("rgb96fixed", 96),
];

pub fn bits_of(f: &str) -> u32 {
    FORMATS.iter().find(|x| x.0 == f).map_or(0, |x| x.1)
}

pub fn stride(f: &str, w: u32) -> usize {
    (w as usize * bits_of(f) as usize).div_ceil(8)
}

/// a bitmap source as the model knows it: its pixels in its format, rows `stride` bytes
#[derive(Clone, Debug, PartialEq)]
pub struct Img {
    pub w: u32,
    pub h: u32,
    pub fmt: &'static str,
    pub dpi: (f64, f64),
    pub px: Vec<u8>,
    pub pal: Option<Vec<u32>>,
}

fn fmt_static(f: &str) -> &'static str {
    FORMATS.iter().find(|x| x.0 == f).map(|x| x.0).unwrap_or("?")
}

impl Img {
    pub fn new(w: u32, h: u32, fmt: &str) -> Img {
        let fmt = fmt_static(fmt);
        Img { w, h, fmt, dpi: (96.0, 96.0), px: vec![0; stride(fmt, w) * h as usize], pal: None }
    }
    pub fn stride(&self) -> usize {
        stride(self.fmt, self.w)
    }
    fn bits(&self) -> u32 {
        bits_of(self.fmt)
    }
    /// pixel (x, y): its bytes, or below 8 bits its value in the low byte
    pub fn get(&self, x: u32, y: u32) -> Vec<u8> {
        let b = self.bits();
        let row = &self.px[y as usize * self.stride()..];
        if b % 8 != 0 {
            let bit = x * b;
            vec![(row[(bit / 8) as usize] >> (8 - b - bit % 8)) & ((1u16 << b) - 1) as u8]
        } else {
            let n = (b / 8) as usize;
            row[x as usize * n..x as usize * n + n].to_vec()
        }
    }
    pub fn put(&mut self, x: u32, y: u32, v: &[u8]) {
        let b = self.bits();
        let st = self.stride();
        let row = &mut self.px[y as usize * st..];
        if b % 8 != 0 {
            let bit = x * b;
            let shift = 8 - b - bit % 8;
            let m = ((((1u16 << b) - 1) as u8) as u32) << shift;
            let byte = &mut row[(bit / 8) as usize];
            // (the field cleared, then the value added in: no carry can leave it)
            *byte = (*byte & !(m as u8)) + ((v[0] as u32) << shift & m) as u8;
        } else {
            let n = (b / 8) as usize;
            row[x as usize * n..x as usize * n + n].copy_from_slice(v);
        }
    }
}

/* ---------------------------------------------------------------------------------------- */
/* bitmaps, transforms, CopyPixels */

/// `wbits`: model::wic's source bytes, the executor's palette for indexed formats
pub fn bitmap(fmt: &str, w: u32, h: u32, seed: Option<u32>) -> Img {
    let f = crate::model::wic::format(fmt).unwrap();
    let mut i = Img::new(w, h, fmt);
    i.px = crate::model::wic::source(f, w, h, seed);
    // (MEASURED: a bitmap from memory has no resolution: 0 dpi)
    i.dpi = (0.0, 0.0);
    if crate::model::wic::indexed(f) {
        i.pal = Some(crate::model::wic::palette());
    }
    hit(clause::WICIMAGE_BITMAP);
    if fmt == "alpha8" {
        hit(clause::WICIMAGE_BITMAP_ALPHA8);
    }
    i
}

/// MEASURED (golden wicscale: every width 1..=8 to every width 1..=17, both directions):
/// nearest neighbor: output pixel d takes source pixel ceil((d + 1) * src / dst) - 1, the one
/// under the output pixel's right (bottom) edge, not its center
pub fn nearest(d: u32, src: u32, dst: u32) -> u32 {
    (((d as u64 + 1) * src as u64 - 1) / dst as u64) as u32
}

/// MEASURED (golden wicscale fmt-*: 15 formats, every mode): the format a scaler works in and
/// the one it answers with. NearestNeighbor, Linear and Cubic (and Fant but for 16-bit gray and
/// RGBA): gray of 8 bits or less in 8bppGray, 24bppBGR as it is, 32bppBGRA (and 32bppRGBA,
/// 8bppAlpha) premultiplied and answered as 32bppBGRA, every other format as 32bppPBGRA
pub fn scale_formats(f: &str, mode: &str) -> (&'static str, &'static str) {
    match (f, mode) {
        ("gray16", "fant") => ("grayfloat", "grayfloat"),
        ("rgba64", "fant") => ("prgbafloat", "prgbafloat"),
        ("gray16" | "rgb48" | "rgba64" | "rgba" | "alpha8", "hqcubic") => (fmt_static(f), fmt_static(f)),
        ("bw" | "gray2" | "gray4" | "gray8", _) => ("gray8", "gray8"),
        ("bgr24", _) => ("bgr24", "bgr24"),
        ("bgra" | "rgba" | "alpha8", _) => ("pbgra", "bgra"),
        _ => ("pbgra", "pbgra"),
    }
}

/// MEASURED (golden wicencode float-gray16-every-rgbfloat: every 16-bit level): the linear value
/// Windows gives each 16-bit sRGB level, as little-endian f32 (neither sRGB's curve in f32,
/// 0.4% of the levels, nor the 8-bit table interpolated at v / 257, 43%, makes it)
static LINEAR16: &[u8; 65536 * 4] = include_bytes!("data/wic_linear16.bin");

/// MEASURED (golden wicencode gray16-every-rgb48: every 16-bit level): the 48bppRGB level
/// Windows gives each 16bppGray level (not the level itself: 16510 of 65536 levels are one off,
/// no 256-multiple but 0 comes out; no rule of the 8 bits and the rest found), little-endian
static GRAY16_RGB48: &[u8; 65536 * 2] = include_bytes!("data/wic_gray16_rgb48.bin");

pub fn gray16_rgb48(v: u16) -> u16 {
    let k = usize::from(v) * 2;
    u16::from_le_bytes([GRAY16_RGB48[k], GRAY16_RGB48[k + 1]])
}

pub fn linear16(v: u16) -> f32 {
    let k = usize::from(v) * 4;
    f32::from_le_bytes([LINEAR16[k], LINEAR16[k + 1], LINEAR16[k + 2], LINEAR16[k + 3]])
}

/// MEASURED (golden wicencode float-*, fixed-*: 4096 and 1024 levels of 16bppGray, 48bppRGB and
/// 64bppRGBA, every value as this): 16 bits a channel into the float formats: each color channel
/// its LINEAR16 value, alpha v / 65535 in f32; 128bppPRGBAFloat multiplies in f32; 32bppGrayFloat
/// is (r * 0.2126 + g * 0.7152) + b * 0.0722 in f32; 64bppRGBAFixedPoint rounds the value times
/// 8192 (16bppGray's own path there first through 48bppRGB: GRAY16_RGB48)
fn wide_to_float(s: &Img, to: &'static str) -> Option<Res<Img>> {
    let n: usize = match s.fmt {
        "gray16" => 1,
        "rgb48" => 3,
        "rgba64" => 4,
        _ => return None,
    };
    let out: usize = match to {
        "grayfloat" => 4,
        "rgbafloat" | "prgbafloat" | "rgbfloat" => 16,
        "rgba64fixed" => 8,
        _ => return None,
    };
    hit(clause::WICIMAGE_CONVERT_WIDE_FLOAT);
    let mut o = Img::new(s.w, s.h, to);
    o.dpi = s.dpi;
    let (w, st, ost) = (s.w as usize, s.stride(), o.stride());
    for y in 0..s.h as usize {
        for x in 0..w {
            let at = |k: usize| {
                let i = y * st + (x * n + k) * 2;
                u16::from_le_bytes([s.px[i], s.px[i + 1]])
            };
            // (MEASURED: golden fixpath-gray16-*: 16bppGray into 64bppRGBAFixedPoint goes through
            // 48bppRGB, into the float formats it does not)
            let c: [u16; 3] = match (n, to) {
                (1, "rgba64fixed") => [gray16_rgb48(at(0)); 3],
                (1, _) => [at(0); 3],
                _ => [at(0), at(1), at(2)],
            };
            let [r, g, b] = c.map(linear16);
            let a16 = if n == 4 { Some(at(3)) } else { None };
            let a = a16.map_or(1.0, |a| f32::from(a) / 65535.0);
            let d = &mut o.px[y * ost + x * out..y * ost + (x + 1) * out];
            let put = |d: &mut [u8], v: &[f32]| {
                for (k, f) in v.iter().enumerate() {
                    d[k * 4..k * 4 + 4].copy_from_slice(&f.to_le_bytes());
                }
            };
            match to {
                "grayfloat" => put(d, &[(r * 0.2126 + g * 0.7152) + b * 0.0722]),
                // (128bppRGBFloat's fourth float keeps the alpha)
                "rgbafloat" | "rgbfloat" => put(d, &[r, g, b, a]),
                "prgbafloat" => put(d, &[r * a, g * a, b * a, a]),
                _ => {
                    let fix = |v: f64| ((v * 8192.0 + 0.5).floor() as i16).to_le_bytes();
                    let fa = a16.map_or(1.0, |a| f64::from(a) / 65535.0);
                    for (k, v) in [f64::from(r), f64::from(g), f64::from(b), fa].into_iter().enumerate() {
                        d[k * 2..k * 2 + 2].copy_from_slice(&fix(v));
                    }
                }
            }
        }
    }
    Some(Ok(o))
}

/// the channels of the fixed-point, half and three-float formats: how many, their kind, the
/// alpha's index when one is there (the colors are in r, g, b order but 64bppBGRAFixedPoint's)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WideKind {
    /// s2.13 in 16 bits
    Fix16,
    /// s7.24 in 32 bits
    Fix32,
    /// IEEE binary16
    Half,
    /// IEEE binary32
    Float,
}

/// (channels, kind, whether the fourth channel is there)
fn wide_layout(f: &str) -> Option<(usize, WideKind)> {
    Some(match f {
        "grayfix16" => (1, WideKind::Fix16),
        "grayfix32" => (1, WideKind::Fix32),
        "grayhalf" => (1, WideKind::Half),
        "rgb48fixed" => (3, WideKind::Fix16),
        "rgb48half" => (3, WideKind::Half),
        "rgb64fixed" | "bgra64fixed" => (4, WideKind::Fix16),
        "rgb64half" | "rgba64half" => (4, WideKind::Half),
        "rgb96fixed" => (3, WideKind::Fix32),
        "rgb128fixed" | "rgba128fixed" => (4, WideKind::Fix32),
        "rgb96float" => (3, WideKind::Float),
        "grayfloat" => (1, WideKind::Float),
        "rgbafloat" | "prgbafloat" | "rgbfloat" => (4, WideKind::Float),
        _ => return None,
    })
}

/// MEASURED (golden wicencode fixpath-gray16-*: 4096 levels of 16bppGray into each format, every
/// value as this): into a half format each channel the LINEAR16 value rounded to the nearest
/// half (even on a tie); into an s2.13 format the 48bppRGB level's (GRAY16_RGB48) LINEAR16 value
/// times 8192 rounded; into an s7.24 format the 48bppRGB level's LINEAR16 value times 2^24 plus
/// 0.5 in f32, truncated, and into 32bppGrayFixedPoint that value back in f32 (times 2^-24) as
/// 32bppGrayFloat's sum of the three; 96bppRGBFloat the LINEAR16 value; the fourth channel 1
fn gray16_to_wide(s: &Img, to: &'static str) -> Option<Res<Img>> {
    if s.fmt != "gray16" || matches!(to, "grayfloat" | "rgbafloat" | "prgbafloat" | "rgbfloat") {
        return None;
    }
    let (n, kind) = wide_layout(to)?;
    hit(clause::WICIMAGE_CONVERT_GRAY16_WIDE);
    let mut o = Img::new(s.w, s.h, to);
    o.dpi = s.dpi;
    let (st, ost) = (s.stride(), o.stride());
    let size = match kind {
        WideKind::Fix16 | WideKind::Half => 2,
        WideKind::Fix32 | WideKind::Float => 4,
    };
    for y in 0..s.h as usize {
        for x in 0..s.w as usize {
            let i = y * st + x * 2;
            let v = u16::from_le_bytes([s.px[i], s.px[i + 1]]);
            let lin = linear16(v);
            let lin48 = linear16(gray16_rgb48(v));
            let fix32 = |f: f32| (f * 16_777_216.0f32 + 0.5f32) as i32;
            let (c, one): (Vec<u8>, Vec<u8>) = match kind {
                WideKind::Half => (crate::model::d3d9draw::f32_to_f16(lin).to_le_bytes().to_vec(), crate::model::d3d9draw::f32_to_f16(1.0).to_le_bytes().to_vec()),
                WideKind::Fix16 => ((((f64::from(lin48) * 8192.0) + 0.5).floor() as i16).to_le_bytes().to_vec(), 8192i16.to_le_bytes().to_vec()),
                WideKind::Fix32 if n == 1 => {
                    let f = fix32(lin48) as f32 * (1.0 / 16_777_216.0f32);
                    (fix32((f * 0.2126 + f * 0.7152) + f * 0.0722).to_le_bytes().to_vec(), vec![])
                }
                WideKind::Fix32 => (fix32(lin48).to_le_bytes().to_vec(), (1i32 << 24).to_le_bytes().to_vec()),
                WideKind::Float => (lin.to_le_bytes().to_vec(), 1.0f32.to_le_bytes().to_vec()),
            };
            let d = &mut o.px[y * ost + x * n * size..y * ost + (x + 1) * n * size];
            for k in 0..n {
                d[k * size..(k + 1) * size].copy_from_slice(if k == 3 { &one } else { &c });
            }
        }
    }
    Some(Ok(o))
}

/// MEASURED (golden wicencode fixpath-gray16-*: from each format into 64bppRGBAFixedPoint, every
/// value as this): a float or half channel times 8192 rounded; an s2.13 one as it is; an s7.24
/// one shifted right 11 bits; a gray channel into each color; no fourth channel: alpha 1 (8192)
fn wide_to_rgba64fixed(s: &Img, to: &'static str) -> Option<Res<Img>> {
    if to != "rgba64fixed" || s.fmt == "rgba64fixed" {
        return None;
    }
    let (n, kind) = wide_layout(s.fmt)?;
    hit(clause::WICIMAGE_CONVERT_WIDE_FIXED);
    let mut o = Img::new(s.w, s.h, to);
    o.dpi = s.dpi;
    let (st, ost) = (s.stride(), o.stride());
    let size = match kind {
        WideKind::Fix16 | WideKind::Half => 2,
        WideKind::Fix32 | WideKind::Float => 4,
    };
    for y in 0..s.h as usize {
        for x in 0..s.w as usize {
            let at = |k: usize| -> i16 {
                let i = y * st + (x * n + k) * size;
                let b = &s.px[i..i + size];
                let round = |v: f64| (v * 8192.0 + 0.5).floor() as i16;
                match kind {
                    WideKind::Fix16 => i16::from_le_bytes([b[0], b[1]]),
                    WideKind::Fix32 => (i32::from_le_bytes([b[0], b[1], b[2], b[3]]) >> 11) as i16,
                    WideKind::Half => round(crate::model::d3d9draw::f16_to_f64(u16::from_le_bytes([b[0], b[1]]))),
                    WideKind::Float => round(f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))),
                }
            };
            let v: [i16; 4] = match (n, s.fmt) {
                (1, _) => [at(0), at(0), at(0), 8192],
                (3, _) => [at(0), at(1), at(2), 8192],
                (_, "bgra64fixed") => [at(2), at(1), at(0), at(3)],
                _ => [at(0), at(1), at(2), at(3)],
            };
            for (k, c) in v.iter().enumerate() {
                o.px[y * ost + x * 8 + k * 2..y * ost + x * 8 + k * 2 + 2].copy_from_slice(&c.to_le_bytes());
            }
        }
    }
    Some(Ok(o))
}

pub fn wic_convert(s: &Img, to: &'static str) -> Res<Img> {
    if s.fmt == to {
        return Ok(s.clone());
    }
    if let Some(r) = wide_to_float(s, to) {
        return r;
    }
    if let Some(r) = gray16_to_wide(s, to) {
        return r;
    }
    if let Some(r) = wide_to_rgba64fixed(s, to) {
        return r;
    }
    if matches!(s.fmt, "bgr24" | "bgra" | "gray8" | "pbgra" | "rgba" | "prgba") && to == "rgba64fixed" {
        // MEASURED (golden wicencode encas-bmp-rgba64, rgba64fixed-every-bgr24, -bgra, -gray8:
        // every level; rgba64fixed-from-pbgra, -rgba, -prgba: 256 random pixels each): each 8-bit
        // color channel the LINEAR16 value of its 16-bit level (c * 257) times 8192 rounded, not
        // premultiplied; a premultiplied one first divided by alpha through a reciprocal table,
        // (c * ((255 << 16) / a)) >> 16 at most 255, 0 where alpha is 0; alpha a / 255 times 8192
        // rounded, 1 where none
        hit(clause::WICIMAGE_CONVERT_BYTE_FIXED);
        let mut o = Img::new(s.w, s.h, to);
        o.dpi = s.dpi;
        let ost = o.stride();
        let fix = |c: u8| (f64::from(linear16(u16::from(c) * 257)) * 8192.0 + 0.5).floor() as i16;
        let unpremultiply = |c: u8, a: u8| -> u8 {
            match a {
                0 => 0,
                _ => u8::try_from(((u32::from(c) * ((255 << 16) / u32::from(a))) >> 16).min(255)).unwrap_or(u8::MAX),
            }
        };
        for y in 0..s.h {
            for x in 0..s.w {
                let p = s.get(x, y);
                let alpha = |a: u8| (f64::from(a) / 255.0 * 8192.0 + 0.5).floor() as i16;
                let v: [i16; 4] = match s.fmt {
                    "gray8" => [fix(p[0]), fix(p[0]), fix(p[0]), 8192],
                    "bgr24" => [fix(p[2]), fix(p[1]), fix(p[0]), 8192],
                    "bgra" => [fix(p[2]), fix(p[1]), fix(p[0]), alpha(p[3])],
                    "rgba" => [fix(p[0]), fix(p[1]), fix(p[2]), alpha(p[3])],
                    "pbgra" => [2, 1, 0].map(|k| fix(unpremultiply(p[k], p[3]))).into_iter().chain([alpha(p[3])]).collect::<Vec<_>>().try_into().map_err(|_| Fail::Bad)?,
                    _ => [0, 1, 2].map(|k| fix(unpremultiply(p[k], p[3]))).into_iter().chain([alpha(p[3])]).collect::<Vec<_>>().try_into().map_err(|_| Fail::Bad)?,
                };
                let at = y as usize * ost + x as usize * 8;
                for (k, c) in v.iter().enumerate() {
                    o.px[at + k * 2..at + k * 2 + 2].copy_from_slice(&c.to_le_bytes());
                }
            }
        }
        return Ok(o);
    }
    if (s.fmt, to) == ("gray16", "rgb48") {
        hit(clause::WICIMAGE_CONVERT_GRAY16_RGB48);
        let mut o = Img::new(s.w, s.h, to);
        o.dpi = s.dpi;
        let (st, ost) = (s.stride(), o.stride());
        for y in 0..s.h as usize {
            for x in 0..s.w as usize {
                let i = y * st + x * 2;
                let v = gray16_rgb48(u16::from_le_bytes([s.px[i], s.px[i + 1]])).to_le_bytes();
                for k in 0..3 {
                    o.px[y * ost + x * 6 + k * 2..y * ost + x * 6 + k * 2 + 2].copy_from_slice(&v);
                }
            }
        }
        return Ok(o);
    }
    let (Some(fi), Some(ti)) = (crate::model::wic::format(s.fmt), crate::model::wic::format(to)) else { return Err(Fail::Unmodeled(open::WICIMAGE_CONVERT_OTHER)) };
    if s.pal.is_some() && s.pal != Some(crate::model::wic::palette()) {
        return Err(Fail::Unmodeled(open::WICIMAGE_CONVERT_OTHER));
    }
    let px = crate::model::wic::convert(fi, ti, s.w, s.h, &s.px)?;
    let pal = if crate::model::wic::indexed(ti) { Some(crate::model::wic::palette()) } else { None };
    Ok(Img { px, fmt: to, pal, ..s.clone() })
}

/// Catmull-Rom's cubic (a = -1/2)
pub fn catmull_rom(x: f64) -> f64 {
    let x = x.abs();
    if x < 1.0 {
        1.5 * x * x * x - 2.5 * x * x + 1.0
    } else if x < 2.0 {
        -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0
    } else {
        0.0
    }
}

/// output pixel i's center in source pixels, 16.16 fixed point: the step truncated, the
/// centers aligned (the Linear and Cubic scalers)
fn fixed_center(i: u32, n: u32, d: u32) -> i64 {
    let step = ((n as i64) << 16) / d as i64;
    (2 * i as i64 + 1) * step / 2 - 32768
}

/// one axis of a scaler: each output pixel's source pixels and integer weights over `den`
pub struct Taps {
    pub den: i64,
    pub taps: Vec<Vec<(usize, i64)>>,
}

/// MEASURED (golden wicscale row-*/rrow-*/fine-*/col-*: gray rows and columns of every width
/// 1..=8 to every width 1..=17 and 2..5 <-> 241..255, 303 cases each, all as Windows):
/// - Linear: the center in 16.16 (step truncated), not before the first pixel, weights of 8 bits
///   (the fraction's high byte) between it and the next pixel, the last pixel alone
/// - Cubic: Catmull-Rom at the center's fraction cut to 6 bits, weights of 16 bits rounded, the
///   third tap taking what the four miss of 1, the taps outside the picture its edge pixels
/// - Fant: Fant's resampling (each output pixel the average over its span of the source
///   interpolated linearly from each pixel's start), exact, so weights over n * d
pub fn taps(mode: &str, n: u32, d: u32) -> Taps {
    let last = n as i64 - 1;
    let at = |k: i64| k.clamp(0, last) as usize;
    match mode {
        "linear" => Taps {
            den: 256,
            taps: (0..d)
                .map(|i| {
                    let p = fixed_center(i, n, d).max(0);
                    let (k, f) = (p >> 16, (p & 0xffff) >> 8);
                    if k >= last { vec![(last as usize, 256)] } else { vec![(at(k), 256 - f), (at(k + 1), f)] }
                })
                .collect(),
        },
        "cubic" => Taps {
            den: 1 << 16,
            taps: (0..d)
                .map(|i| {
                    let p = fixed_center(i, n, d);
                    let (k, t) = (p >> 16, ((p & 0xffff) >> 10) as f64 / 64.0);
                    let mut w: Vec<i64> = (-1..3).map(|j| (catmull_rom(t - j as f64) * 65536.0 + 0.5).floor() as i64).collect();
                    w[2] += 65536 - w.iter().sum::<i64>();
                    (-1..3).zip(w).map(|(j, w)| (at(k + j), w)).collect()
                })
                .collect(),
        },
        _ => {
            // (in units of 1/d of a source pixel: a source pixel is d, an output pixel n)
            let (n6, d6) = (n as i64, d as i64);
            let mut taps = vec![];
            let mut cur: Vec<(usize, i64)> = vec![];
            let (mut u, mut inseg, mut outseg) = (0i64, d6, n6);
            while taps.len() < d as usize {
                let seg = inseg.min(outseg);
                // (the source at the segment's start: inseg of pixel u, the rest of the next)
                for (k, w) in [(at(u), inseg * seg), (at(u + 1), (d6 - inseg) * seg)] {
                    match cur.iter_mut().find(|t| t.0 == k) {
                        Some(t) => t.1 += w,
                        None => cur.push((k, w)),
                    }
                }
                if inseg < outseg {
                    outseg -= inseg;
                    inseg = d6;
                    u += 1;
                } else {
                    inseg -= outseg;
                    outseg = n6;
                    taps.push(std::mem::take(&mut cur));
                }
            }
            Taps { den: d6 * n6, taps }
        }
    }
}

/// MEASURED (golden wicscale, as `taps`): HighQualityCubic: Catmull-Rom about the exact
/// center, widened by the ratio when shrinking, the weights normalized, the taps outside the
/// picture its edge pixels
pub fn hq_taps(n: u32, d: u32) -> Vec<Vec<(usize, f64)>> {
    let r = (n as f64 / d as f64).max(1.0);
    (0..d)
        .map(|i| {
            let p = (i as f64 + 0.5) * n as f64 / d as f64 - 0.5;
            let (lo, hi) = ((p - 2.0 * r).floor() as i64, (p + 2.0 * r).ceil() as i64);
            let w: Vec<(usize, f64)> = (lo..=hi).map(|j| (j.clamp(0, n as i64 - 1) as usize, catmull_rom((j as f64 - p) / r))).collect();
            let sum: f64 = w.iter().map(|t| t.1).sum();
            w.into_iter().map(|(k, x)| (k, x / sum)).collect()
        })
        .collect()
}

/// a filter over a picture of 8-bit channels (8bppGray, 24bppBGR, 32bppPBGRA)
fn filter(s: &Img, w: u32, h: u32, mode: &str) -> Res<Img> {
    let c = (bits_of(s.fmt) / 8) as usize;
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    let src = |x: usize, y: usize, k: usize| s.px[y * s.stride() + x * c + k] as i64;
    let ow = w as usize;
    let ost = o.stride();
    match mode {
        "linear" | "cubic" => {
            // (MEASURED: across, then down, each pass truncated to 8 bits)
            let (tx, ty) = (taps(mode, s.w, w), taps(mode, s.h, h));
            let mut mid = vec![0i64; ow * s.h as usize * c];
            for y in 0..s.h as usize {
                for (x, t) in tx.taps.iter().enumerate() {
                    for k in 0..c {
                        let v: i64 = t.iter().map(|&(j, wt)| wt * src(j, y, k)).sum();
                        mid[(y * ow + x) * c + k] = v.div_euclid(tx.den).clamp(0, 255);
                    }
                }
            }
            for (y, t) in ty.taps.iter().enumerate() {
                for x in 0..ow {
                    for k in 0..c {
                        let m = |j: usize| mid[(j * ow + x) * c + k];
                        let v: i64 = match mode {
                            // MEASURED (golden wicscale vcols-cubic-*, 19520 samples as Windows): down
                            // on x86, Cubic's weights halved to 15 bits, each product floored to 1/4,
                            // the sum floored; on x64 as across (the sum of 16-bit products floored)
                            "cubic" if crate::model::arch() == 32 => {
                                hit(clause::WICIMAGE_SCALE_CUBIC_VERTICAL);
                                t.iter().map(|&(j, wt)| ((wt >> 1) * m(j)) >> 13).sum::<i64>() >> 2
                            }
                            _ => {
                                if mode == "cubic" {
                                    hit(clause::WICIMAGE_SCALE_CUBIC_VERTICAL);
                                }
                                t.iter().map(|&(j, wt)| wt * m(j)).sum::<i64>().div_euclid(ty.den)
                            }
                        };
                        o.px[y * ost + x * c + k] = v.clamp(0, 255) as u8;
                    }
                }
            }
        }
        "fant" => {
            // (MEASURED: exact in both directions, rounded once, halves up)
            let (tx, ty) = (taps(mode, s.w, w), taps(mode, s.h, h));
            let den = tx.den as i128 * ty.den as i128;
            for (y, a) in ty.taps.iter().enumerate() {
                for (x, b) in tx.taps.iter().enumerate() {
                    for k in 0..c {
                        let v: i128 = a.iter().map(|&(jy, wy)| b.iter().map(|&(jx, wx)| wy as i128 * wx as i128 * src(jx, jy, k) as i128).sum::<i128>()).sum();
                        o.px[y * ost + x * c + k] = ((2 * v + den).div_euclid(2 * den)).clamp(0, 255) as u8;
                    }
                }
            }
        }
        _ => {
            // (MEASURED: without rounding between the passes, rounded once)
            let (tx, ty) = (hq_taps(s.w, w), hq_taps(s.h, h));
            for (y, a) in ty.iter().enumerate() {
                for (x, b) in tx.iter().enumerate() {
                    for k in 0..c {
                        let v: f64 = a.iter().map(|&(jy, wy)| wy * b.iter().map(|&(jx, wx)| wx * src(jx, jy, k) as f64).sum::<f64>()).sum();
                        o.px[y * ost + x * c + k] = (v + 0.5).floor().clamp(0.0, 255.0) as u8;
                    }
                }
            }
        }
    }
    Ok(o)
}

/// HighQualityCubic in a format of its own (16 bits a channel, 32bppRGBA, 8bppAlpha): as over 8
/// bits, every channel alike
fn hq_wide(s: &Img, w: u32, h: u32) -> Res<Img> {
    // (MEASURED: golden wicscale rgrid-hqcubic-rgba-*, fmt-rgba-hqcubic: straight alpha filtered
    // premultiplied, as 32bppPRGBA, and back)
    if s.fmt == "rgba" {
        let p = hq_plain(&wic_convert(s, "prgba")?, w, h)?;
        return wic_convert(&p, "rgba");
    }
    // (MEASURED: golden rgrid-hqcubic-rgba64-*, fmt-rgba64-hqcubic: 64bppRGBA is premultiplied
    // too, color = (c*a)>>16, and back = floor(c*65536/a) clamped, 0 where alpha is 0)
    if s.fmt == "rgba64" {
        let mut p = s.clone();
        for px in p.px.chunks_exact_mut(8) {
            let a = u32::from(u16::from_le_bytes([px[6], px[7]]));
            for k in 0..3 {
                let v = u32::from(u16::from_le_bytes([px[2 * k], px[2 * k + 1]]));
                let v = u16::try_from(v * a >> 16).expect("c*a>>16 < 65536");
                px[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
        let mut q = hq_plain(&p, w, h)?;
        for px in q.px.chunks_exact_mut(8) {
            let a = u32::from(u16::from_le_bytes([px[6], px[7]]));
            for k in 0..3 {
                let v = u32::from(u16::from_le_bytes([px[2 * k], px[2 * k + 1]]));
                let v = if a == 0 { 0 } else { (v * 65536 / a).min(65535) };
                let v = u16::try_from(v).expect("clamped to 65535");
                px[2 * k..2 * k + 2].copy_from_slice(&v.to_le_bytes());
            }
        }
        return Ok(q);
    }
    hq_plain(s, w, h)
}

fn hq_plain(s: &Img, w: u32, h: u32) -> Res<Img> {
    let wide = matches!(s.fmt, "gray16" | "rgb48" | "rgba64");
    let size = if wide { 2 } else { 1 };
    let c = bits_of(s.fmt) as usize / (8 * size);
    let max = if wide { 65535.0 } else { 255.0 };
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    let src = |x: usize, y: usize, k: usize| {
        let i = y * s.stride() + (x * c + k) * size;
        if wide { f64::from(u16::from_le_bytes([s.px[i], s.px[i + 1]])) } else { f64::from(s.px[i]) }
    };
    let ost = o.stride();
    let (tx, ty) = (hq_taps(s.w, w), hq_taps(s.h, h));
    for (y, a) in ty.iter().enumerate() {
        for (x, b) in tx.iter().enumerate() {
            let sum = |f: &dyn Fn(usize, usize) -> f64| -> f64 { a.iter().map(|&(jy, wy)| wy * b.iter().map(|&(jx, wx)| wx * f(jx, jy)).sum::<f64>()).sum() };
            for k in 0..c {
                let v = sum(&|x, y| src(x, y, k));
                // (MEASURED: golden rgrid-hqcubic-gray16-7x7-3x3, -rgb48-4x4-9x9, -8x6-3x13,
                // -rgba64-4x4-9x9, -7x7-3x3: 16-bit sums within 0.005 of a half round either way
                // (one at 0.002..0.005), its f32 order not found)
                if wide && (v - v.floor() - 0.5).abs() < 0.005 {
                    return Err(Fail::Unmodeled(open::WICIMAGE_SCALE_WIDE));
                }
                let v = (v + 0.5).floor().clamp(0.0, max) as u16;
                let i = y * ost + (x * c + k) * size;
                if wide {
                    o.px[i..i + 2].copy_from_slice(&v.to_le_bytes());
                } else {
                    o.px[i] = v as u8;
                }
            }
        }
    }
    Ok(o)
}

/// a filter over a picture of f32 channels (32bppGrayFloat, 128bppPRGBAFloat)
fn filter_float(s: &Img, w: u32, h: u32, mode: &str) -> Res<Img> {
    let c = (bits_of(s.fmt) / 32) as usize;
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    let src = |x: usize, y: usize, k: usize| {
        let i = y * s.stride() + (x * c + k) * 4;
        f64::from(f32::from_le_bytes([s.px[i], s.px[i + 1], s.px[i + 2], s.px[i + 3]]))
    };
    let ost = o.stride();
    let mut put = |x: usize, y: usize, k: usize, v: f64| {
        let i = y * ost + (x * c + k) * 4;
        o.px[i..i + 4].copy_from_slice(&(v as f32).to_le_bytes());
    };
    match mode {
        "fant" => {
            // MEASURED (golden wicscale rgrid-fant-gray16-*, -rgba64-*, fmt-*-fant: 1275 floats,
            // every one as this): across, then down, in f32: each tap's weight its integer
            // weight times the f32 reciprocal of the pass's denominator, each product rounded, then
            // added in the order of the taps
            let (tx, ty) = (taps(mode, s.w, w), taps(mode, s.h, h));
            let (rx, ry) = (1.0 / tx.den as f32, 1.0 / ty.den as f32);
            let ow = w as usize;
            let mut mid = vec![0f32; ow * s.h as usize * c];
            for y in 0..s.h as usize {
                for (x, t) in tx.taps.iter().enumerate() {
                    for k in 0..c {
                        mid[(y * ow + x) * c + k] = t.iter().fold(0.0f32, |acc, &(j, wt)| acc + (wt as f32 * rx) * src(j, y, k) as f32);
                    }
                }
            }
            for (y, t) in ty.taps.iter().enumerate() {
                for x in 0..ow {
                    for k in 0..c {
                        put(x, y, k, f64::from(t.iter().fold(0.0f32, |acc, &(j, wt)| acc + (wt as f32 * ry) * mid[(j * ow + x) * c + k])));
                    }
                }
            }
        }
        _ => return Err(Fail::Unmodeled(open::WICIMAGE_SCALE_WIDE)),
    }
    Ok(o)
}

pub fn scale(s: &Img, w: u32, h: u32, mode: &str) -> Res<Img> {
    let (work, out) = scale_formats(s.fmt, mode);
    hit(clause::WICIMAGE_SCALE_FORMAT);
    if mode == "hqcubic" && ["gray16", "rgb48", "rgba64", "rgba", "alpha8"].contains(&work) {
        hit(clause::WICIMAGE_SCALE_HQCUBIC);
        return hq_wide(&wic_convert(s, work)?, w, h);
    }
    if mode == "fant" && ["grayfloat", "prgbafloat"].contains(&work) {
        hit(clause::WICIMAGE_SCALE_FANT);
        let o = filter_float(&wic_convert(s, work)?, w, h, mode)?;
        return wic_convert(&o, out);
    }
    if mode != "nn" {
        if !["gray8", "bgr24", "pbgra"].contains(&work) {
            return Err(Fail::Unmodeled(open::WICIMAGE_SCALE_WIDE));
        }
        let s = wic_convert(s, work)?;
        let o = filter(&s, w, h, mode)?;
        hit(match mode {
            "linear" => clause::WICIMAGE_SCALE_LINEAR,
            "cubic" => clause::WICIMAGE_SCALE_CUBIC,
            "fant" => clause::WICIMAGE_SCALE_FANT,
            _ => clause::WICIMAGE_SCALE_HQCUBIC,
        });
        return wic_convert(&o, out);
    }
    let s = wic_convert(s, work)?;
    hit(clause::WICIMAGE_SCALE_NEAREST);
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    for y in 0..h {
        for x in 0..w {
            o.put(x, y, &s.get(nearest(x, s.w, w), nearest(y, s.h, h)));
        }
    }
    wic_convert(&o, out)
}

/// WICBitmapTransformOptions: the flips of the source, then the rotation clockwise (low two
/// bits) (MEASURED: golden wictransform, every combination)
pub fn flip(s: &Img, opt: u32) -> Img {
    if opt & 3 != 0 && opt & 24 != 0 {
        return rotate_flip(&rotate_flip(s, opt & 24), opt & 3);
    }
    rotate_flip(s, opt)
}

fn rotate_flip(s: &Img, opt: u32) -> Img {
    let rot = opt & 3;
    let (w, h) = if rot & 1 != 0 { (s.h, s.w) } else { (s.w, s.h) };
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], dpi: if rot & 1 != 0 { (s.dpi.1, s.dpi.0) } else { s.dpi }, ..s.clone() };
    for y in 0..h {
        for x in 0..w {
            let (mut dx, mut dy) = (x, y);
            if opt & 8 != 0 {
                dx = w - 1 - dx;
            }
            if opt & 16 != 0 {
                dy = h - 1 - dy;
            }
            // (the source pixel the rotated picture shows at (dx, dy))
            let (sx, sy) = match rot {
                0 => (dx, dy),
                1 => (dy, s.h - 1 - dx),
                2 => (s.w - 1 - dx, s.h - 1 - dy),
                _ => (s.w - 1 - dy, dx),
            };
            o.put(x, y, &s.get(sx, sy));
        }
    }
    o
}

/// a rectangle inside the picture (ASSUMED: one that is not, or is empty, is E_INVALIDARG)
fn inside(s: &Img, x: i64, y: i64, w: i64, h: i64) -> bool {
    x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= s.w as i64 && y + h <= s.h as i64
}

pub fn clip(s: &Img, x: u32, y: u32, w: u32, h: u32) -> Img {
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    for j in 0..h {
        for i in 0..w {
            o.put(i, j, &s.get(x + i, y + j));
        }
    }
    o
}

/// CopyPixels of a rectangle into rows of `stride` bytes in a buffer of `size`: the hr and the rows
pub fn copy(s: &Img, rect: Option<(i64, i64, i64, i64)>, bufs: Option<(i64, i64)>) -> (u32, Vec<Vec<u8>>) {
    let (x, y, w, h) = rect.unwrap_or((0, 0, s.w as i64, s.h as i64));
    if x < 0 || y < 0 {
        hit(clause::WICIMAGE_COPY_NEGATIVE);
        return (ARITHMETIC_OVERFLOW, vec![]);
    }
    if !inside(s, x, y, w, h) {
        hit(clause::WICIMAGE_COPY_RECT);
        return (E_INVALIDARG, vec![]);
    }
    let n = stride(s.fmt, w as u32) as i64;
    if let Some((st, size)) = bufs {
        if st < n {
            hit(clause::WICIMAGE_COPY_STRIDE);
            return (E_INVALIDARG, vec![]);
        }
        if size < st * (h - 1) + n {
            hit(clause::WICIMAGE_COPY_BUFFER);
            return (INSUFFICIENT_BUFFER, vec![]);
        }
    }
    hit(clause::WICIMAGE_COPY);
    let c = clip(s, x as u32, y as u32, w as u32, h as u32);
    (0, c.px.chunks(n as usize).map(<[u8]>::to_vec).collect())
}

/* ---------------------------------------------------------------------------------------- */
/* palettes */

/// the 16 system colors that are not the 8 on-off primaries, in the order the halftone
/// palettes append them
const SYSTEM8: [u32; 8] = [0xc0c0c0, 0x808080, 0x800000, 0x008000, 0x000080, 0x808000, 0x800080, 0x008080];

/// MEASURED (golden wicpalette, every entry of every type): a halftone palette is the cube of
/// `levels` evenly spaced values of each primary (blue fastest, then green, then red), then the
/// system colors not in it (none for 252 and 256)
pub fn halftone(t: u32) -> Vec<u32> {
    let (r, g, b) = match t {
        3 => (2, 2, 2),
        4 => (3, 3, 3),
        5 => (4, 4, 4),
        6 => (5, 5, 5),
        7 => (6, 6, 6),
        8 => (6, 7, 6),
        _ => (8, 8, 4),
    };
    // (levels rounded to the nearest: 0x2b, 0x40, 0xbf, 0xd5, 0x24, 0x49 ...)
    let lv = |i: u32, n: u32| (i * 255 * 2 + (n - 1)) / (2 * (n - 1));
    let mut c = Vec::new();
    for ri in 0..r {
        for gi in 0..g {
            for bi in 0..b {
                c.push(0xff00_0000 | lv(ri, r) << 16 | lv(gi, g) << 8 | lv(bi, b));
            }
        }
    }
    if t < 8 {
        for e in SYSTEM8 {
            if !c.contains(&(0xff00_0000 | e)) {
                c.push(0xff00_0000 | e);
            }
        }
    }
    c
}

/// InitializePredefined: (hr, type, colors)
pub fn predefined(t: u32, transparent: bool) -> Res<(u32, u32, Vec<u32>)> {
    let gray = |n: u32| -> Vec<u32> { (0..n).map(|i| 0xff00_0000 | (i * 255 / (n - 1)) * 0x10101).collect() };
    let mut c = match t {
        2 => {
            hit(clause::WICIMAGE_PALETTE_BW);
            vec![0xff00_0000, 0xffff_ffff]
        }
        3..=9 => {
            hit(clause::WICIMAGE_PALETTE_HALFTONE);
            halftone(t)
        }
        10 => gray(4),
        11 => gray(16),
        12 => gray(256),
        _ => {
            hit(clause::WICIMAGE_PALETTE_TYPE);
            return Ok((E_INVALIDARG, 0, vec![]));
        }
    };
    if t >= 10 {
        hit(clause::WICIMAGE_PALETTE_GRAY);
    }
    if transparent {
        // MEASURED: the black and white and gray palettes ignore it (the documentation says
        // to pass FALSE for them); the type stays what it was (not Custom as documented)
        if t == 2 || t >= 10 {
            hit(clause::WICIMAGE_PALETTE_TRANSPARENT_GRAY);
            return Ok((0, t, c));
        }
        hit(clause::WICIMAGE_PALETTE_TRANSPARENT);
        if c.len() == 256 {
            c[255] = 0;
        } else {
            c.push(0);
        }
    }
    Ok((0, t, c))
}

fn palette_line(op: &str, hr: u32, t: u32, c: &[u32], out: &mut Vec<String>) {
    let bw = c.len() == 2 && c.contains(&0xff00_0000) && c.contains(&0xffff_ffff);
    // (MEASURED, golden wicdecode png-trns-gray*: a gray palette with a transparent entry is not gray)
    let gray = c.iter().all(|&v| (v >> 16 & 255) == (v >> 8 & 255) && (v >> 8 & 255) == (v & 255) && v >> 24 == 255);
    let alpha = c.iter().any(|&v| v >> 24 != 255);
    out.push(format!("{op} {hr:08x} type {t} count {} bw {} gray {} alpha {}", c.len(), bw as u8, (gray && !c.is_empty()) as u8, alpha as u8));
    if hr == 0 {
        out.push(c.iter().map(|v| format!("{v:08x}")).collect());
    }
}

/* ---------------------------------------------------------------------------------------- */
/* decoders */

pub struct Decoded {
    pub container: &'static str,
    pub frames: Vec<Res<Frame>>,
}

/// a decoder's frame: its picture, or one whose GetSize and GetPixelFormat fail with an HRESULT
#[derive(Clone)]
pub enum Frame {
    Pixels(Img),
    NoInfo(u32),
    /// its size and format there, its CopyPixels failing with the HRESULT
    NoPixels(Img, u32),
}

impl Frame {
    /// the picture, when the frame has one to copy
    pub fn pixels(self) -> Option<Img> {
        match self {
            Frame::Pixels(i) => Some(i),
            Frame::NoInfo(_) | Frame::NoPixels(..) => None,
        }
    }
}

#[derive(Clone, Copy)]
enum FrameFail {
    NoInfo(u32),
    NoPixels(u32),
}

fn rd16(b: &[u8], o: usize) -> Option<u32> {
    Some(u16::from_le_bytes(b.get(o..o + 2)?.try_into().ok()?) as u32)
}
fn rd32(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o + 4)?.try_into().ok()?))
}

/// which decoder takes a file (by its first bytes)
pub fn decode(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let d = if b.starts_with(b"BM") {
        bmp(b).map(|i| i.map(|i| Decoded { container: "bmp", frames: vec![Ok(Frame::Pixels(i))] }))
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(b).map(|i| i.map(|i| Decoded { container: "png", frames: vec![Ok(Frame::Pixels(i))] }))
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        gif(b)
    } else if b.starts_with(b"II\x2a\0") || b.starts_with(b"MM\0\x2a") {
        tiff(b)
    } else if b.starts_with(&[0, 0, 1, 0]) {
        ico(b)
    } else if b.starts_with(b"DDS ") {
        dds(b)
    } else {
        hit(clause::WICIMAGE_DECODE_UNKNOWN);
        return Ok(Err(COMPONENT_NOT_FOUND));
    };
    d
}

fn ppm_dpi(ppm: u32) -> f64 {
    if ppm == 0 {
        96.0
    } else {
        ppm as f64 * 0.0254
    }
}

/// BMP (BITMAPCOREHEADER, BITMAPINFOHEADER, BITMAPV4HEADER, BITMAPV5HEADER)
fn bmp(b: &[u8]) -> Res<Result<Img, u32>> {
    let open = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    // (MEASURED: headers cut short: WINCODEC_ERR_STREAMREAD; pixels cut short: WINCODEC_ERR_BADIMAGE)
    let (Some(off), Some(hdr)) = (rd32(b, 10), rd32(b, 14)) else {
        hit(clause::WICIMAGE_BMP_SHORT);
        return Ok(Err(STREAM_READ));
    };
    if b.len() < 14 + hdr as usize {
        hit(clause::WICIMAGE_BMP_SHORT);
        return Ok(Err(STREAM_READ));
    }
    let (w, h, bpp, comp, used, ppm) = if hdr == 12 {
        let (Some(w), Some(h), Some(bpp)) = (rd16(b, 18), rd16(b, 20), rd16(b, 24)) else { return open };
        (w as i32, h as i32, bpp, 0, 0, (0, 0))
    } else {
        let v: Vec<Option<u32>> = [18, 22, 28, 30, 38, 42, 46].iter().map(|&o| rd32(b, o).or_else(|| rd16(b, o))).collect();
        if v.iter().any(Option::is_none) || b.len() < 14 + hdr as usize {
            return open;
        }
        let v: Vec<u32> = v.into_iter().map(Option::unwrap).collect();
        (v[0] as i32, v[1] as i32, rd16(b, 28).unwrap(), v[3], v[6], (v[4], v[5]))
    };
    hit(clause::WICIMAGE_BMP_HEADER);
    let top = h < 0;
    let (w, h) = (w as u32, h.unsigned_abs());
    // masks: bit fields after an info header, inside a v4/v5 header
    let masks: Option<[u32; 4]> = if comp == 3 {
        let at = if hdr == 40 { 54 } else { 54 };
        let m = [rd32(b, at), rd32(b, at + 4), rd32(b, at + 8), if hdr >= 108 { rd32(b, at + 12) } else { Some(0) }];
        if m.iter().any(Option::is_none) {
            return open;
        }
        Some(m.map(Option::unwrap))
    } else {
        None
    };
    let entry = if hdr == 12 { 3 } else { 4 };
    let pal_at = 14 + hdr as usize + if hdr == 40 && comp == 3 { 12 } else { 0 };
    let colors = if bpp <= 8 { if used == 0 || used > 1 << bpp { 1 << bpp } else { used } } else { 0 } as usize;
    let mut pal = Vec::new();
    for i in 0..colors {
        let Some(c) = b.get(pal_at + i * entry..pal_at + i * entry + 3) else { return open };
        pal.push(0xff00_0000 | (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32);
    }
    let dpi = (ppm_dpi(ppm.0), ppm_dpi(ppm.1));
    let src_stride = (w as usize * bpp as usize).div_ceil(32) * 4;
    let data = &b[(off as usize).min(b.len())..];
    if comp == 1 || comp == 2 {
        // (the model does not hold pictures past 2^20 pixels)
        if w as u64 * h as u64 > 1 << 20 {
            return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
        }
        let px = rle(data, w, h, comp == 2, &pal);
        hit(clause::WICIMAGE_BMP_RLE);
        let mut i = Img::new(w, h, "bgr32");
        i.dpi = dpi;
        i.px = px;
        return Ok(Ok(i));
    }
    if src_stride.checked_mul(h as usize).is_none_or(|n| data.len() < n) {
        hit(clause::WICIMAGE_BMP_SHORT);
        return Ok(Err(BAD_IMAGE));
    }
    let fmt = match (bpp, masks) {
        (1, _) => "idx1",
        (4, _) => "idx4",
        (8, _) => "idx8",
        (16, None) | (16, Some([0x7c00, 0x03e0, 0x001f, 0])) => "bgr555",
        (16, Some([0xf800, 0x07e0, 0x001f, 0])) => "bgr565",
        (24, None) => "bgr24",
        (32, None) | (32, Some([0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0])) => "bgr32",
        (32, Some([0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000])) => "bgra",
        // (MEASURED golden wicencode enc-bmp-gray16, -rgb48, -rgba64: what the encoder writes of
        // 64bppRGBAFixedPoint, 64 bits a pixel, decodes as it)
        (64, None) => "rgba64fixed",
        (16 | 32, Some(m)) => {
            hit(clause::WICIMAGE_BMP_FIELDS);
            let mut i = Img::new(w, h, "bgr32");
            i.dpi = dpi;
            for y in 0..h {
                let sy = if top { y } else { h - 1 - y } as usize;
                for x in 0..w as usize {
                    let o = sy * src_stride + x * bpp as usize / 8;
                    let v = if bpp == 16 { rd16(data, o).unwrap() } else { rd32(data, o).unwrap() };
                    let c = |k: usize| field(v, m[k]);
                    i.put(x as u32, y, &[c(2), c(1), c(0), 0]);
                }
            }
            return Ok(Ok(i));
        }
        _ => return open,
    };
    hit(clause::WICIMAGE_BMP_FORMAT);
    let mut i = Img::new(w, h, fmt);
    i.dpi = dpi;
    let n = i.stride();
    for y in 0..h as usize {
        let sy = if top { y } else { h as usize - 1 - y };
        i.px[y * n..y * n + n].copy_from_slice(&data[sy * src_stride..sy * src_stride + n]);
        if bpp == 64 {
            for p in i.px[y * n..y * n + n].chunks_exact_mut(8) {
                swap_red_blue16(p);
            }
        }
    }
    if bpp <= 8 {
        hit(clause::WICIMAGE_BMP_PALETTE);
        i.pal = Some(pal);
    }
    Ok(Ok(i))
}

/// MEASURED (golden wicdecode bmpfields-*): a bit field's value widened to 8 bits by repeating
/// its bits, a wider one narrowed to its high 8 bits
pub fn field(v: u32, m: u32) -> u8 {
    if m == 0 {
        return 0;
    }
    let n = m.count_ones();
    let x = (v & m) >> m.trailing_zeros();
    if n >= 8 {
        return (x >> (n - 8)) as u8;
    }
    let mut o = 0u32;
    let mut have = 0;
    while have < 8 {
        o = o << n | x;
        have += n;
    }
    (o >> (have - 8)) as u8
}

/// RLE8 / RLE4 (BITMAPINFOHEADER's compression): runs, absolute runs padded to a word, end of
/// line, end of bitmap, deltas; bottom-up. MEASURED (golden wicdecode bmprle*): 32bppBGR, the
/// palette's colors with the fourth byte 0, pixels no run reaches 0
fn rle(d: &[u8], w: u32, h: u32, four: bool, pal: &[u32]) -> Vec<u8> {
    // MEASURED (golden wicdecode bmprlebad*): data that ends (inside a pair, a delta, an
    // absolute run) ends the picture there, an absolute run cut short written not at all; runs
    // past the row are cut off at its end
    let mut px = vec![0u8; 4 * (w * h) as usize];
    let (mut x, mut y) = (0u32, 0u32);
    let mut put = |x: u32, y: u32, v: u8| {
        if x < w && y < h {
            let c = pal.get(v as usize).copied().unwrap_or(0);
            let o = 4 * ((h - 1 - y) * w + x) as usize;
            px[o..o + 4].copy_from_slice(&[c as u8, (c >> 8) as u8, (c >> 16) as u8, 0]);
        }
    };
    let mut p = 0;
    loop {
        let (Some(&n), Some(&v)) = (d.get(p), d.get(p + 1)) else {
            hit(clause::WICIMAGE_BMP_RLE_BROKEN);
            break;
        };
        p += 2;
        if n > 0 {
            for k in 0..n as u32 {
                let i = if four { if k % 2 == 0 { v >> 4 } else { v & 15 } } else { v };
                put(x, y, i);
                x += 1;
            }
            continue;
        }
        match v {
            0 => {
                x = 0;
                y += 1;
            }
            1 => break,
            2 => {
                let (Some(&dx), Some(&dy)) = (d.get(p), d.get(p + 1)) else {
            hit(clause::WICIMAGE_BMP_RLE_BROKEN);
            break;
        };
                x += dx as u32;
                y += dy as u32;
                p += 2;
            }
            n => {
                let bytes = if four { (n as usize).div_ceil(2) } else { n as usize };
                let Some(a) = d.get(p..p + bytes) else {
            hit(clause::WICIMAGE_BMP_RLE_BROKEN);
            break;
        };
                for k in 0..n as usize {
                    let i = if four { if k % 2 == 0 { a[k / 2] >> 4 } else { a[k / 2] & 15 } } else { a[k] };
                    put(x, y, i);
                    x += 1;
                }
                p += bytes + bytes % 2;
            }
        }
        if y >= h {
            break;
        }
    }
    px
}

/// zlib (RFC 1950), stored blocks only: anything else is left to Windows
/// zlib stored blocks as far as the bytes go: the data, and whether the final block ended;
/// None for a compressed block (left to Windows)
fn inflate_stored(z: &[u8]) -> Option<(Vec<u8>, bool)> {
    let mut o = Vec::new();
    let mut p = 2;
    loop {
        let Some(&hdr) = z.get(p) else { return Some((o, false)) };
        if hdr >> 1 & 3 != 0 {
            return None;
        }
        let Some(len) = rd16(z, p + 1) else { return Some((o, false)) };
        let len = len as usize;
        let end = (p + 5 + len).min(z.len());
        o.extend_from_slice(z.get(p + 5..end).unwrap_or_default());
        if end < p + 5 + len {
            return Some((o, false));
        }
        p += 5 + len;
        if hdr & 1 != 0 {
            return Some((o, true));
        }
    }
}

fn adler32(b: &[u8]) -> u32 {
    let (mut a, mut s) = (1u32, 0u32);
    for &x in b {
        a = (a + x as u32) % 65521;
        s = (s + a) % 65521;
    }
    s << 16 | a
}

fn crc32(b: &[u8]) -> u32 {
    let mut c = !0u32;
    for &x in b {
        c ^= x as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

fn unfilter(data: &[u8], rows: usize, bpr: usize, bpp: usize) -> Option<Vec<Vec<u8>>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut prev = vec![0u8; bpr];
    let mut p = 0;
    for _ in 0..rows {
        // (a row the data does not reach whole: the rows so far)
        let (Some(&f), Some(row)) = (data.get(p), data.get(p + 1..p + 1 + bpr)) else { break };
        let mut row = row.to_vec();
        p += 1 + bpr;
        for i in 0..bpr {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            row[i] = row[i].wrapping_add(match f {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u32 + b as u32) / 2) as u8,
                4 => {
                    let pp = a as i32 + b as i32 - c as i32;
                    let (pa, pb, pc) = ((pp - a as i32).abs(), (pp - b as i32).abs(), (pp - c as i32).abs());
                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
                _ => return None,
            });
        }
        prev = row.clone();
        out.push(row);
    }
    Some(out)
}

/// PNG (ISO/IEC 15948)
fn png(b: &[u8]) -> Res<Result<Img, u32>> {
    let open = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    let mut p = 8;
    let (mut ihdr, mut plte, mut trns, mut phys, mut idat) = (None, None, None, None, Vec::new());
    // (MEASURED, golden wicdecode png-bad-crc, png-short, cut-png-*: the CRCs are not checked;
    // a file that ends early ends its chunks there: of the first IDAT the bytes there are, of
    // any other chunk nothing unless it is whole with its CRC)
    let mut broken = false;
    loop {
        let Some(len) = b.get(p..p + 4).map(|x| u32::from_be_bytes(x.try_into().unwrap()) as usize) else {
            broken = true;
            break;
        };
        if b.len() < p + 12 + len {
            broken = true;
            if idat.is_empty() && b.get(p + 4..p + 8) == Some(b"IDAT") {
                idat.extend_from_slice(b.get(p + 8..(p + 8 + len).min(b.len())).unwrap_or_default());
            }
            break;
        }
        let body = &b[p + 4..p + 8 + len];
        let _ = crc32;
        let d = &body[4..];
        match &body[..4] {
            b"IHDR" => ihdr = Some(d.to_vec()),
            b"PLTE" => plte = Some(d.to_vec()),
            b"tRNS" => trns = Some(d.to_vec()),
            b"pHYs" => phys = Some(d.to_vec()),
            b"IDAT" => idat.extend_from_slice(d),
            b"IEND" => break,
            _ => {}
        }
        p += 12 + len;
    }
    let Some(ih) = ihdr.filter(|h| h.len() == 13) else { return open };
    let be = |o: usize| u32::from_be_bytes(ih[o..o + 4].try_into().unwrap());
    let (w, h, depth, ct, il) = (be(0), be(4), ih[8] as u32, ih[9], ih[12] == 1);
    // (an IHDR the format does not allow: cases png-ihdr-* to measure; found by fuzzing, a
    // depth of 0 read no samples)
    let allowed = matches!((ct, depth), (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) | (2 | 4 | 6, 8 | 16));
    if !allowed || ih[10] != 0 || ih[11] != 0 || ih[12] > 1 {
        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    }
    // (the model does not hold pictures past 2^20 pixels)
    if w as u64 * h as u64 > 1 << 20 {
        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    }
    hit(clause::WICIMAGE_PNG_CHUNKS);
    // MEASURED: image data that does not inflate whole (cut, or its Adler-32 wrong): the frame
    // is there, its pixels all 0
    // MEASURED (golden wicdecode cut-png-*): a file that ends before the first IDAT has a
    // byte: no decoder knows it (0x88982f07)
    if broken && idat.is_empty() {
        hit(clause::WICIMAGE_PNG_SHORT);
        return Ok(Err(UNKNOWN_IMAGE_FORMAT));
    }
    // MEASURED: image data that inflates whole but whose Adler-32 is wrong: the frame is
    // there, its pixels all 0; data cut short: the rows it holds whole, the rest 0
    let Some((raw, whole)) = inflate_stored(&idat) else { return open };
    let raw = if whole { Some(raw).filter(|r| idat.len() >= 4 && adler32(r) == u32::from_be_bytes(idat[idat.len() - 4..].try_into().unwrap())) } else { Some(raw) };
    if !whole && il {
        // (interlaced pictures cut short: cases png-cut-adam7-* to measure)
        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    }
    let zero = raw.is_none();
    if zero {
        hit(clause::WICIMAGE_PNG_ZERO);
    }
    let raw = raw.unwrap_or_default();
    let ch = match ct {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        6 => 4,
        _ => return open,
    };
    let pbits = depth * ch;
    // the picture's samples, row by row (Adam7's passes put back in place)
    let mut full: Vec<Vec<u8>> = vec![vec![0; (w as usize * pbits as usize).div_ceil(8)]; h as usize];
    let passes: Vec<(u32, u32, u32, u32)> = if il { crate::generators::WICIMAGE_ADAM7.to_vec() } else { vec![(0, 0, 1, 1)] };
    let mut at = 0;
    for (x0, y0, dx, dy) in if zero { vec![] } else { passes } {
        let pw = if x0 < w { (w - x0).div_ceil(dx) } else { 0 };
        let ph = if y0 < h { (h - y0).div_ceil(dy) } else { 0 };
        if pw == 0 || ph == 0 {
            continue;
        }
        let bpr = (pw as usize * pbits as usize).div_ceil(8);
        let Some(rows) = unfilter(&raw[at.min(raw.len())..], ph as usize, bpr, (pbits as usize).div_ceil(8)) else { return open };
        at += ph as usize * (bpr + 1);
        for (j, row) in rows.iter().enumerate() {
            for i in 0..pw {
                let (x, y) = (x0 + i * dx, y0 + j as u32 * dy);
                let v = get_sample_bits(row, i, pbits);
                put_sample_bits(&mut full[y as usize], x, pbits, &v);
            }
        }
    }
    if il {
        hit(clause::WICIMAGE_PNG_ADAM7);
    }
    let dpi = match phys {
        Some(ph) if ph.len() == 9 && ph[8] == 1 => {
            let px = u32::from_be_bytes(ph[0..4].try_into().unwrap());
            let py = u32::from_be_bytes(ph[4..8].try_into().unwrap());
            hit(clause::WICIMAGE_PNG_PHYS);
            (px as f64 * 0.0254, py as f64 * 0.0254)
        }
        _ => (96.0, 96.0),
    };
    // MEASURED (golden wicdecode png-0-*, png-2-*): with tRNS, gray below 8 bits decodes as
    // indexed (its levels the palette), RGB as 32bppBGRA / 64bppRGBA, the color tRNS names
    // transparent
    let key: Option<Vec<u8>> = match (&trns, ct) {
        (Some(_), 0) => {
            hit(clause::WICIMAGE_PNG_TRNS_GRAY);
            None
        }
        (Some(t), 2) if t.len() >= 6 => {
            hit(clause::WICIMAGE_PNG_TRNS);
            Some(t[..6].to_vec())
        }
        _ => None,
    };
    let fmt = match (ct, depth) {
        (0, 1) if trns.is_some() => "idx1",
        (0, 2) if trns.is_some() => "idx2",
        (0, 4) if trns.is_some() => "idx4",
        (0, 8) if trns.is_some() => "idx8",
        (0, 16) if trns.is_some() => "rgba64",
        (2, 8) if trns.is_some() => "bgra",
        (2, 16) if trns.is_some() => "rgba64",
        (0, 1) => "bw",
        (0, 2) => "gray2",
        (0, 4) => "gray4",
        (0, 8) => "gray8",
        (0, 16) => "gray16",
        (2, 8) => "bgr24",
        (2, 16) => "rgb48",
        (3, 1) => "idx1",
        (3, 2) => "idx2",
        (3, 4) => "idx4",
        (3, 8) => "idx8",
        (4, 8) | (6, 8) => "bgra",
        (4, 16) | (6, 16) => "rgba64",
        _ => return open,
    };
    hit(clause::WICIMAGE_PNG_FORMAT);
    // MEASURED (golden wicdecode png-trns-gray*): a gray key is the chunk's 16 bits (bytes it
    // lacks 0); at up to 8 bits its palette entry is transparent black, at 16 the pixels of
    // that level get alpha 0 and keep their level
    let gray_key = match (&trns, ct) {
        (Some(t), 0) => Some(u16::from_be_bytes([t.first().copied().unwrap_or(0), t.get(1).copied().unwrap_or(0)]) as u32),
        _ => None,
    };
    let mut img = Img::new(w, h, fmt);
    img.dpi = dpi;
    for y in 0..h {
        for x in 0..w {
            let s = get_sample_bits(&full[y as usize], x, pbits);
            let be16 = |k: usize| [s[2 * k + 1], s[2 * k]];
            let hide = key.as_ref().is_some_and(|k| if depth == 8 { (0..3).all(|c| k[2 * c] == 0 && k[2 * c + 1] == s[c]) } else { k[..] == s[..6] });
            let v: Vec<u8> = match (ct, depth) {
                (2, 8) if key.is_some() => vec![s[2], s[1], s[0], if hide { 0 } else { 255 }],
                (2, 16) if key.is_some() => [be16(0), be16(1), be16(2), if hide { [0, 0] } else { [255, 255] }].concat(),
                (0, 16) if trns.is_some() => {
                    let a = if gray_key == Some(u16::from_be_bytes([s[0], s[1]]) as u32) { [0, 0] } else { [255, 255] };
                    [be16(0), be16(0), be16(0), a].concat()
                }
                (0, 16) => be16(0).to_vec(),
                (2, 8) => vec![s[2], s[1], s[0]],
                (2, 16) => [be16(0), be16(1), be16(2)].concat(),
                (4, 8) => vec![s[0], s[0], s[0], s[1]],
                (4, 16) => [be16(0), be16(0), be16(0), be16(1)].concat(),
                (6, 8) => vec![s[2], s[1], s[0], s[3]],
                (6, 16) => [be16(0), be16(1), be16(2), be16(3)].concat(),
                _ => s,
            };
            img.put(x, y, &v);
        }
    }
    if let (Some(k), true) = (gray_key, depth <= 8) {
        let n = 1u32 << depth;
        img.pal = Some((0..n).map(|v| if v == k & (n - 1) { 0 } else { 0xff00_0000 | (v * 255 / (n - 1)) * 0x10101 }).collect());
    }
    if ct == 3 {
        let Some(pl) = plte else { return open };
        let t = trns.unwrap_or_default();
        hit(clause::WICIMAGE_PNG_PALETTE);
        img.pal = Some(pl.chunks(3).enumerate().map(|(i, c)| (*t.get(i).unwrap_or(&255) as u32) << 24 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect());
    }
    Ok(Ok(img))
}

/// a sample of `bits` at pixel x of a row: below 8 bits its value, else its bytes
fn get_sample_bits(row: &[u8], x: u32, bits: u32) -> Vec<u8> {
    if bits < 8 {
        let bit = x * bits;
        vec![(row[(bit / 8) as usize] >> (8 - bits - bit % 8)) & ((1u16 << bits) - 1) as u8]
    } else {
        let n = (bits / 8) as usize;
        row[x as usize * n..x as usize * n + n].to_vec()
    }
}

fn put_sample_bits(row: &mut [u8], x: u32, bits: u32, v: &[u8]) {
    if bits < 8 {
        let bit = x * bits;
        let shift = 8 - bits - bit % 8;
        let m = ((((1u16 << bits) - 1) as u8) as u32) << shift;
        let b = &mut row[(bit / 8) as usize];
        *b = (*b & !(m as u8)) | ((v[0] as u32) << shift & m) as u8;
    } else {
        let n = (bits / 8) as usize;
        row[x as usize * n..x as usize * n + n].copy_from_slice(v);
    }
}

/// GIF's LZW (GIF89a appendix F)
fn lzw(min: u32, data: &[u8], n: usize, cut: bool) -> Option<Vec<u8>> {
    let clear = 1u32 << min;
    let mut dict: Vec<Vec<u8>> = (0..clear).map(|i| vec![i as u8]).collect();
    dict.push(vec![]);
    dict.push(vec![]);
    let mut size = min + 1;
    let (mut acc, mut nacc, mut p) = (0u32, 0u32, 0usize);
    let mut out = Vec::new();
    let mut prev: Option<u32> = None;
    while out.len() < n {
        while nacc < size {
            // (data cut short: the pixels so far)
            let Some(&x) = data.get(p) else { return if cut { Some(out) } else { None } };
            acc |= (x as u32) << nacc;
            p += 1;
            nacc += 8;
        }
        let code = acc & ((1 << size) - 1);
        acc >>= size;
        nacc -= size;
        if code == clear {
            dict.truncate(clear as usize + 2);
            size = min + 1;
            prev = None;
            continue;
        }
        if code == clear + 1 {
            break;
        }
        if code as usize > dict.len() {
            return None;
        }
        let entry = if (code as usize) < dict.len() {
            dict[code as usize].clone()
        } else {
            let mut e = dict[prev? as usize].clone();
            e.push(e[0]);
            e
        };
        if let Some(pc) = prev && dict.len() < 4096 {
            let mut e = dict[pc as usize].clone();
            e.push(entry[0]);
            dict.push(e);
        }
        out.extend_from_slice(&entry);
        if dict.len() >= 1 << size && size < 12 {
            size += 1;
        }
        prev = Some(code);
    }
    out.truncate(n);
    Some(out)
}

/// GIF87a/89a: each image a frame of its own size, 8bppIndexed, its table (local or global)
fn gif(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let open = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    // MEASURED (golden wicdecode cut-gif-*): a file that ends inside the screen descriptor,
    // a color table or an image descriptor: WINCODEC_ERR_STREAMREAD; right after an image
    // descriptor (no LZW code size): E_INVALIDARG; inside the image data: the frame is there, of
    // its data only the whole sub-blocks count, the pixels they do not reach 0
    let short = Ok(Err(STREAM_READ));
    let Some(flags) = b.get(10) else { return short };
    if b.len() < 13 {
        return short;
    }
    let mut p = 13;
    let table = |p: usize, flags: u8| -> Option<Vec<u32>> {
        let n = 2usize << (flags & 7);
        let t = b.get(p..p + 3 * n)?;
        Some(t.chunks(3).map(|c| 0xff00_0000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect())
    };
    let global = if flags & 0x80 != 0 {
        let Some(t) = table(p, *flags) else { return short };
        p += 3 * t.len();
        Some(t)
    } else {
        None
    };
    let mut frames = Vec::new();
    let mut transparent: Option<u8> = None;
    loop {
        match b.get(p) {
            Some(0x21) => {
                let Some(&label) = b.get(p + 1) else { return open };
                p += 2;
                if label == 0xf9 && b.get(p) == Some(&4) {
                    let Some(g) = b.get(p + 1..p + 5) else { return open };
                    transparent = if g[0] & 1 != 0 { Some(g[3]) } else { None };
                }
                while let Some(&n) = b.get(p) {
                    p += 1 + n as usize;
                    if n == 0 {
                        break;
                    }
                }
            }
            Some(0x2c) => {
                let (Some(w), Some(h), Some(&f)) = (rd16(b, p + 5), rd16(b, p + 7), b.get(p + 9)) else { return short };
                p += 10;
                let local = if f & 0x80 != 0 {
                    let Some(t) = table(p, f) else { return short };
                    p += 3 * t.len();
                    Some(t)
                } else {
                    None
                };
                let Some(&min) = b.get(p) else {
                    hit(clause::WICIMAGE_GIF_SHORT);
                    return Ok(Err(E_INVALIDARG));
                };
                p += 1;
                let mut data = Vec::new();
                let mut cut = false;
                // (the bytes of a sub-block the file ends inside, after whole ones)
                let mut inside: Option<usize> = None;
                while let Some(&n) = b.get(p) {
                    let Some(d) = b.get(p + 1..p + 1 + n as usize) else {
                        cut = true;
                        if !data.is_empty() {
                            inside = Some(b.len() - p - 1);
                        }
                        p = b.len();
                        break;
                    };
                    data.extend_from_slice(d);
                    p += 1 + n as usize;
                    if n == 0 {
                        break;
                    }
                }
                if b.get(p).is_none() {
                    cut = true;
                }
                let Some(mut idx) = lzw(min as u32, &data, (w * h) as usize, cut) else { return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)) };
                if idx.len() < (w * h) as usize {
                    if !cut {
                        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
                    }
                    hit(clause::WICIMAGE_GIF_SHORT);
                    idx.resize((w * h) as usize, 0);
                }
                // MEASURED (golden wicdecode cuta-gif-*: a 32 x 16 picture in sub-blocks of 255,
                // 255, 98, cut at every third byte): a file that ends inside a sub-block after
                // whole ones: the picture one row down (the first row 0, the last lost); past 32
                // bytes of the second sub-block Windows writes pixels of no rule (cases
                // cuta-gif-1084..1303)
                if let Some(m) = inside {
                    if m > 32 || f & 0x40 != 0 {
                        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
                    }
                    let n = idx.len();
                    idx.splice(0..0, std::iter::repeat_n(0, (w as usize).min(n)));
                    idx.truncate(n);
                }
                hit(clause::WICIMAGE_GIF_LZW);
                let rows: Vec<u32> = if f & 0x40 != 0 {
                    hit(clause::WICIMAGE_GIF_INTERLACE);
                    [(0, 8), (4, 8), (2, 4), (1, 2)].iter().flat_map(|&(s, d)| (s..h).step_by(d)).collect()
                } else {
                    (0..h).collect()
                };
                let mut img = Img::new(w, h, "idx8");
                for (k, &y) in rows.iter().enumerate() {
                    img.px[(y * w) as usize..((y + 1) * w) as usize].copy_from_slice(&idx[k * w as usize..(k + 1) * w as usize]);
                }
                let Some(mut pal) = local.or_else(|| global.clone()) else { return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)) };
                if let Some(t) = transparent {
                    hit(clause::WICIMAGE_GIF_TRANSPARENT);
                    if let Some(c) = pal.get_mut(t as usize) {
                        *c &= 0x00ff_ffff;
                    }
                }
                hit(clause::WICIMAGE_GIF_FRAME);
                img.pal = Some(pal);
                frames.push(Ok(Frame::Pixels(img)));
                transparent = None;
            }
            Some(0x3b) => break,
            // (a file that ends after a frame: the frames so far)
            None if !frames.is_empty() => break,
            _ => return open,
        }
    }
    Ok(Ok(Decoded { container: "gif", frames }))
}

/// TIFF 6.0 baseline, uncompressed
fn tiff(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let open = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    let big = b[0] == b'M';
    let r16 = |o: usize| -> Option<u32> { let x: [u8; 2] = b.get(o..o + 2)?.try_into().ok()?; Some(if big { u16::from_be_bytes(x) } else { u16::from_le_bytes(x) } as u32) };
    let r32 = |o: usize| -> Option<u32> { let x: [u8; 4] = b.get(o..o + 4)?.try_into().ok()?; Some(if big { u32::from_be_bytes(x) } else { u32::from_le_bytes(x) }) };
    let Some(mut ifd) = r32(4) else { return open };
    let mut frames = Vec::new();
    while ifd != 0 {
        let Some(n) = r16(ifd as usize) else { return open };
        let mut tags: std::collections::BTreeMap<u32, Vec<u32>> = Default::default();
        for k in 0..n as usize {
            let e = ifd as usize + 2 + 12 * k;
            let (Some(tag), Some(ty), Some(count)) = (r16(e), r16(e + 2), r32(e + 4)) else { return open };
            let size = match ty {
                3 => 2,
                4 => 4,
                5 => 8,
                _ => 1,
            };
            let at = if size * count as usize > 4 { r32(e + 8).unwrap_or(0) as usize } else { e + 8 };
            let mut vals = Vec::new();
            for i in 0..count as usize {
                let v = match ty {
                    3 => r16(at + 2 * i),
                    4 => r32(at + 4 * i),
                    5 => r32(at + 8 * i).zip(r32(at + 8 * i + 4)).map(|(a, b)| if b == 0 { 0 } else { a / b }),
                    _ => b.get(at + i).map(|&x| x as u32),
                };
                let Some(v) = v else { return open };
                vals.push(v);
            }
            tags.insert(tag, vals);
        }
        let Some(next) = r32(ifd as usize + 2 + 12 * n as usize) else { return open };
        ifd = next;
        // MEASURED (golden wicdecode tiff-other-5, -6): a first page of YCbCr or a mask: no
        // decoder takes the file
        if matches!(tags.get(&262).and_then(|v| v.first()), Some(4 | 6)) {
            if frames.is_empty() {
                hit(clause::WICIMAGE_TIFF_REFUSED);
                return Ok(Err(COMPONENT_NOT_FOUND));
            }
            // MEASURED (golden wicdecode tiff-ycbcr-page-1): such a later page is a frame whose
            // GetSize fails WINCODEC_ERR_UNSUPPORTEDPIXELFORMAT (0x88982f80)
            hit(clause::WICIMAGE_TIFF_REFUSED);
            frames.push(Ok(Frame::NoInfo(0x8898_2f80)));
            continue;
        }
        frames.push(tiff_image(b, &tags).map(Frame::Pixels));
    }
    hit(clause::WICIMAGE_TIFF_IFD);
    Ok(Ok(Decoded { container: "tiff", frames }))
}

fn tiff_image(b: &[u8], t: &std::collections::BTreeMap<u32, Vec<u32>>) -> Res<Img> {
    let one = |k: u32, d: u32| t.get(&k).and_then(|v| v.first().copied()).unwrap_or(d);
    let (w, h) = (one(256, 0), one(257, 0));
    let (bits, ph, spp, planar) = (one(258, 1), one(262, 1), one(277, 1), one(284, 1));
    // (the model does not hold pictures past 2^20 pixels)
    if w as u64 * h as u64 > 1 << 20 || bits > 64 || spp > 16 {
        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    }
    let extra = t.get(&338).and_then(|v| v.first().copied());
    let sfmt = one(339, 1);
    // MEASURED (golden wicdecode tiff-*: every photometric, depth, sample count and extra
    // sample of the suite, both byte orders)
    let fmt = match (ph, bits, spp, extra) {
        (0 | 1, 1, 1, None) => "bw",
        (0 | 1, 4, 1, None) => "gray4",
        (0 | 1, 8, 1, None) => "gray8",
        (0 | 1, 16, 1, None) => "gray16",
        (1, 32, 1, None) if sfmt == 3 => "grayfloat",
        (1, 8, 2, Some(2)) => "bgra",
        (3, 1, 1, None) => "idx1",
        (3, 4, 1, None) => "idx4",
        (3, 8, 1, None) => "idx8",
        (2, 8, 3, None) => "bgr24",
        (2, 16, 3, None) => "rgb48",
        (2, 8, 4, Some(0 | 2)) => "bgra",
        (2, 8, 4, Some(1)) => "pbgra",
        (2, 16, 4, Some(2)) => "rgba64",
        (2, 16, 4, Some(1)) => "prgba64",
        (5, 8, 4, None) => "cmyk",
        (5, 16, 4, None) => "cmyk64",
        (5, 8, 5, Some(2)) => "cmyka40",
        // MEASURED (golden wicdecode tiff-other-*): 2-bit gray widened to 4 bits, 2-bit indices
        // to 4, gray with an alpha of 16 bits to RGBA, an unspecified extra kept as alpha at 16
        // bits and dropped at 8, a fifth sample past RGBA dropped, 32-bit RGB as float, 4-bit RGB
        // widened to 8 bits; YCbCr and masks no decoder takes
        (0 | 1, 2, 1, None) => "gray4",
        (3, 2, 1, None) => "idx4",
        (1, 16, 2, Some(2)) => "rgba64",
        (2, 16, 4, Some(0)) => "rgba64",
        (2, 8, 5, Some(_)) => "bgra",
        (2, 32, 3, None) => "rgb96float",
        (1, 8, 2, Some(0)) => "gray8",
        (1, 8, 2, Some(1)) => "pbgra",
        (5, 8, 5, Some(1)) => "cmyka40",
        (2, 4, 3, None) => "bgr24",
        _ => return Err(Fail::Unmodeled(open::WICIMAGE_TIFF_OTHER)),
    };
    hit(clause::WICIMAGE_TIFF_FORMAT);
    let (Some(offs), Some(counts)) = (t.get(&273), t.get(&279)) else { return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)) };
    let mut data = Vec::new();
    for (o, c) in offs.iter().zip(counts) {
        let Some(d) = b.get(*o as usize..*o as usize + *c as usize) else { return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)) };
        data.extend_from_slice(d);
    }
    // (planar 2: each sample's plane in turn; made chunky here)
    if planar == 2 {
        hit(clause::WICIMAGE_TIFF_PLANAR);
        let pr = (w as usize * bits as usize).div_ceil(8);
        let plane = pr * h as usize;
        if bits % 8 != 0 || data.len() < plane * spp as usize {
            return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
        }
        let n = bits as usize / 8;
        let mut c = Vec::new();
        for y in 0..h as usize {
            for x in 0..w as usize {
                for k in 0..spp as usize {
                    c.extend_from_slice(&data[k * plane + y * pr + x * n..k * plane + y * pr + x * n + n]);
                }
            }
        }
        data = c;
    }
    let row = (w as usize * bits as usize * spp as usize).div_ceil(8);
    if data.len() < row * h as usize {
        return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    }
    let mut img = Img::new(w, h, fmt);
    let big = b[0] == b'M';
    if ph == 0 {
        hit(clause::WICIMAGE_TIFF_WHITEISZERO);
    }
    for y in 0..h {
        let r = &data[y as usize * row..(y as usize + 1) * row];
        for x in 0..w {
            let mut s = if bits < 8 {
                (0..spp).map(|k| get_sample_bits(r, x * spp + k, bits)[0]).collect()
            } else {
                get_sample_bits(r, x, bits * spp)
            };
            if ph == 0 {
                // (WhiteIsZero: the levels inverted)
                for v in s.iter_mut() {
                    *v = if bits < 8 { !*v & ((1u16 << bits) - 1) as u8 } else { !*v };
                }
            }
            let w16 = |k: usize| if big { [s[2 * k + 1], s[2 * k]] } else { [s[2 * k], s[2 * k + 1]] };
            let w32 = |k: usize| if big { [s[4 * k + 3], s[4 * k + 2], s[4 * k + 1], s[4 * k]] } else { [s[4 * k], s[4 * k + 1], s[4 * k + 2], s[4 * k + 3]] };
            let v: Vec<u8> = match (fmt, spp) {
                ("gray4", _) if bits == 2 => vec![s[0] * 5],
                ("bgra" | "pbgra", 2) => vec![s[0], s[0], s[0], s[1]],
                ("gray8", 2) => vec![s[0]],
                ("rgba64", 2) => [w16(0), w16(0), w16(0), w16(1)].concat(),
                ("bgr24", _) if bits == 4 => vec![s[2] * 17, s[1] * 17, s[0] * 17],
                ("bgr24", _) => vec![s[2], s[1], s[0]],
                ("rgb96float", _) => [w32(0), w32(1), w32(2)].concat(),
                // (MEASURED, golden wicdecode tiff-other-10, tiff-cmyka-assoc: CMYK with an
                // associated alpha divided by it, truncated, at most 255)
                ("cmyka40", _) if extra == Some(1) => {
                    let a = s[4] as u32;
                    (0..4).map(|k| if a == 0 { 0 } else { (s[k] as u32 * 255 / a).min(255) as u8 }).chain([s[4]]).collect()
                }
                ("bgra" | "pbgra", _) => vec![s[2], s[1], s[0], s[3]],
                ("gray16", _) => w16(0).to_vec(),
                ("grayfloat", _) => w32(0).to_vec(),
                ("rgb48", _) => [w16(0), w16(1), w16(2)].concat(),
                ("rgba64" | "prgba64" | "cmyk64", _) => [w16(0), w16(1), w16(2), w16(3)].concat(),
                _ => s,
            };
            img.put(x, y, &v);
        }
    }
    // resolution: inches (2) as they are, centimeters (3) times 2.54, none (1) as they are
    if let (Some(x), Some(y)) = (t.get(&282).and_then(|v| v.first()), t.get(&283).and_then(|v| v.first())) {
        let unit = one(296, 2);
        let k = match unit {
            3 => 2.54,
            _ => 1.0,
        };
        hit(clause::WICIMAGE_TIFF_RESOLUTION);
        img.dpi = (*x as f64 * k, *y as f64 * k);
    }
    if ph == 3 {
        let Some(m) = t.get(&320) else { return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)) };
        let n = 1usize << bits;
        if m.len() < 3 * n {
            return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
        }
        hit(clause::WICIMAGE_TIFF_PALETTE);
        let c = |v: u32| v * 255 / 65535;
        img.pal = Some((0..n).map(|i| 0xff00_0000 | c(m[i]) << 16 | c(m[n + i]) << 8 | c(m[2 * n + i])).collect());
    }
    Ok(img)
}

/// ICO: each entry a frame of 32bppBGRA ("ICO Format Overview"): a PNG, or a BMP without its
/// file header, its height doubled, the colors then the AND mask
fn ico(b: &[u8]) -> Res<Result<Decoded, u32>> {
    // MEASURED (golden wicdecode cut-ico-*): a file that ends inside the count:
    // WINCODEC_ERR_STREAMREAD; inside the directory or a BMP entry: WINCODEC_ERR_BADIMAGE; inside
    // a PNG entry: that entry as a PNG file cut there (no frame when no decoder knows it)
    let Some(n) = rd16(b, 4) else {
        hit(clause::WICIMAGE_ICO_SHORT);
        return Ok(Err(STREAM_READ));
    };
    let mut entries = Vec::new();
    for k in 0..n as usize {
        let (Some(size), Some(off)) = (rd32(b, 6 + 16 * k + 8), rd32(b, 6 + 16 * k + 12)) else {
            hit(clause::WICIMAGE_ICO_SHORT);
            return Ok(Err(BAD_IMAGE));
        };
        let (off, size) = (off as usize, size as usize);
        let d = match b.get(off..off.saturating_add(size)) {
            Some(d) => d,
            None if b.get(off..).is_some_and(|d| d.starts_with(b"\x89PNG\r\n\x1a\n") || b"\x89PNG\r\n\x1a\n".starts_with(d)) => &b[off..],
            None if off >= b.len() => &[],
            None => {
                hit(clause::WICIMAGE_ICO_SHORT);
                return Ok(Err(BAD_IMAGE));
            }
        };
        if d.len() < 8 && b"\x89PNG\r\n\x1a\n".starts_with(d) || d.starts_with(b"\x89PNG\r\n\x1a\n") && matches!(self::png(d)?, Err(_)) {
            hit(clause::WICIMAGE_ICO_SHORT);
            continue;
        }
        // (the entry's own depth and size: a PNG's IHDR, a BMP's header)
        let (w, h, bpp) = if d.starts_with(b"\x89PNG") {
            let be = |o: usize| d.get(o..o + 4).map_or(0, |x| u32::from_be_bytes(x.try_into().unwrap()));
            (be(16), be(20), d.get(24).map_or(0, |&x| x as u32) * d.get(25).map_or(1, |&c| crate::generators::wicimage_png_channels(c)))
        } else {
            (rd32(d, 4).unwrap_or(0), rd32(d, 8).unwrap_or(0) / 2, rd16(d, 14).unwrap_or(0))
        };
        let png = d.starts_with(b"\x89PNG");
        // MEASURED (golden wicdecode ico-bmp-depth-*, ico-png-ct*): a BMP entry of a depth but 1, 4,
        // 8, 16, 24 and 32 (2, 3, 5, 48, 64): no decoder for the file (0x88982f07), of 0 bits
        // WINCODEC_ERR_BADIMAGE; a PNG entry
        // that does not decode to 32bppBGRA (8-bit RGBA or gray+alpha): WINCODEC_ERR_BADIMAGE
        if !png && !matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32) {
            hit(clause::WICIMAGE_ICO_DEPTH_BAD);
            return Ok(Err(if bpp == 0 { BAD_IMAGE } else { UNKNOWN_IMAGE_FORMAT }));
        }
        if png && self::png(d)?.map_or(true, |i| i.fmt != "bgra") {
            hit(clause::WICIMAGE_ICO_PNG_BAD);
            return Ok(Err(BAD_IMAGE));
        }
        entries.push(IcoEntry { bpp, png, w, h, img: ico_entry(d) });
    }
    // MEASURED (golden wicdecode ico-three, ico-order-*, ico-png-*-bmp: the same sizes in every
    // depth, the same depth in every size, PNG and BMP entries, directories that say 0 bits): the
    // frames by the entries' own depth, deepest first, of one depth PNG entries first, then the
    // last in the file first (not by size)
    if entries.is_empty() {
        hit(clause::WICIMAGE_ICO_SHORT);
        return Ok(Err(BAD_IMAGE));
    }
    let entries = ico_order(entries)?;
    let mut wider = 0;
    let mut frames = Vec::new();
    for e in entries {
        // MEASURED (golden wicdecode icopair-*, icosmall-*, ico-order-mixed): a PNG frame after
        // a wider frame: its CopyPixels fails WINCODEC_ERR_INSUFFICIENTBUFFER (0x88982f8c)
        let starved = e.png && wider > e.w;
        wider = wider.max(e.w);
        frames.push(e.img.map(|i| if starved {
            hit(clause::WICIMAGE_ICO_PNG_AFTER_WIDER);
            Frame::NoPixels(i, INSUFFICIENT_BUFFER)
        } else {
            Frame::Pixels(i)
        }));
    }
    hit(clause::WICIMAGE_ICO);
    Ok(Ok(Decoded { container: "ico", frames }))
}

struct IcoEntry {
    /// the entry's own depth (a BMP header's, a PNG's IHDR), not the directory's
    bpp: u32,
    png: bool,
    w: u32,
    h: u32,
    img: Res<Img>,
}

/// MEASURED (golden wicdecode icopair-*: every 32-bit PNG of 1..5 x 1..5 beside every 32-bit
/// BMP of 1..4 x 1..4 in both file orders, 384 pairs; icodepth-*: PNGs of 3, 8, 16 beside BMPs of
/// 3 and 16 of 1, 4, 8, 24 bits; icosmall-*, icoseed-*, icopng-*): a file of one PNG entry and one
/// BMP entry: the PNG first when it is near the BMP, the BMP first when far, the later in the file
/// first when just between. The distance: the heights' difference, plus the widths' difference
/// when the PNG is the wider or twice it when the narrower, plus twice the bits the BMP has fewer;
/// near: below the lesser height, just between: equal to it
fn ico_png_vs_bmp(p: (u32, u32), b: (u32, u32, u32)) -> std::cmp::Ordering {
    let dw = if p.0 >= b.0 { p.0 - b.0 } else { 2 * (b.0 - p.0) };
    let d = p.1.abs_diff(b.1) + dw + 2 * 32u32.saturating_sub(b.2);
    d.cmp(&p.1.min(b.1))
}

/// the frames' order: MEASURED (golden wicdecode ico-order-size/depth/zero, ico-png-*-bmp,
/// icopair-*, icodepth-*, icosmall-*): BMP entries by their own depth, deepest first, of one depth
/// the last in the file first; one PNG entry beside one BMP entry by `ico_png_vs_bmp`
fn ico_order(mut entries: Vec<IcoEntry>) -> Res<Vec<IcoEntry>> {
    use std::cmp::Ordering;
    if let [a, b] = &entries[..]
        && a.png != b.png
    {
        let ((p, m), png_first_in_file) = if a.png { ((a, b), true) } else { ((b, a), false) };
        let png_first = match ico_png_vs_bmp((p.w, p.h), (m.w, m.h, m.bpp)) {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => !png_first_in_file,
        };
        if png_first != png_first_in_file {
            entries.reverse();
        }
        hit(clause::WICIMAGE_ICO_ORDER_PNG_PAIR);
        return Ok(entries);
    }
    // (OPEN: a PNG among three or more entries, or two PNGs: the frames are no sort of the pair
    // rule (golden icotri-*: the BMPs then come by size, a PNG before a BMP the pair rule puts
    // first), WICIMAGE.ICO.PNG.SMALL)
    if entries.iter().any(|e| e.png) {
        return Err(Fail::Unmodeled(open::WICIMAGE_ICO_PNG_SMALL));
    }
    entries.reverse();
    entries.sort_by_key(|e| std::cmp::Reverse(e.bpp));
    hit(clause::WICIMAGE_ICO_ORDER);
    Ok(entries)
}

fn ico_entry(d: &[u8]) -> Res<Img> {
    if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        let p = png(d)?.map_err(|_| Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN))?;
        hit(clause::WICIMAGE_ICO_PNG);
        // (only 32bppBGRA entries get here: ico() refuses the others)
        // (a 32bppBGRA PNG has no palette)
        return Ok(Img { dpi: (96.0, 96.0), ..p });
    }
    let broken = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    let (Some(hdr), Some(w), Some(h2), Some(bpp)) = (rd32(d, 0), rd32(d, 4), rd32(d, 8), rd16(d, 14)) else { return broken };
    let h = h2 / 2;
    // (the model does not hold pictures past 2^20 pixels)
    if w as u64 * h as u64 > 1 << 20 || bpp > 32 {
        return broken;
    }
    let colors = if bpp <= 8 { 1usize << bpp } else { 0 };
    let used = rd32(d, 32).unwrap_or(0) as usize;
    let colors = if used > 0 && used < colors { used } else { colors };
    let pal: Vec<u32> = (0..colors).filter_map(|i| d.get(hdr as usize + 4 * i..hdr as usize + 4 * i + 4).map(|c| (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32)).collect();
    let xs = (w as usize * bpp as usize).div_ceil(32) * 4;
    let ms = (w as usize).div_ceil(32) * 4;
    let at = hdr as usize + 4 * colors;
    let Some(xor) = d.get(at..at + xs * h as usize) else { return broken };
    let Some(and) = d.get(at + xs * h as usize..at + xs * h as usize + ms * h as usize) else { return broken };
    hit(clause::WICIMAGE_ICO_BMP);
    let mut img = Img::new(w, h, "bgra");
    let any_alpha = bpp == 32 && (0..h as usize).any(|y| (0..w as usize).any(|x| xor[y * xs + 4 * x + 3] != 0));
    for y in 0..h {
        let sy = (h - 1 - y) as usize;
        let row = &xor[sy * xs..];
        for x in 0..w {
            let m = (and[sy * ms + (x / 8) as usize] >> (7 - x % 8)) & 1;
            let c: [u8; 4] = match bpp {
                1 | 4 | 8 => {
                    let v = get_sample_bits(row, x, bpp as u32)[0] as usize;
                    let c = *pal.get(v).unwrap_or(&0);
                    [c as u8, (c >> 8) as u8, (c >> 16) as u8, 255]
                }
                16 => {
                    // (MEASURED: 555, widened by repeating bits)
                    hit(clause::WICIMAGE_ICO_DEPTH);
                    let v = rd16(row, 2 * x as usize).unwrap();
                    let x5 = |v: u32| ((v << 3) | (v >> 2)) as u8;
                    [x5(v & 31), x5(v >> 5 & 31), x5(v >> 10 & 31), 255]
                }
                24 => [row[3 * x as usize], row[3 * x as usize + 1], row[3 * x as usize + 2], 255],
                32 => {
                    let p = &row[4 * x as usize..4 * x as usize + 4];
                    [p[0], p[1], p[2], p[3]]
                }
                _ => unreachable!("ico() refuses other depths"),
            };
            // (ASSUMED: alpha from the colors when any is set, else from the mask: 1 transparent)
            if !any_alpha && m != 0 {
                img.put(x, y, &[0, 0, 0, 0]);
            } else {
                img.put(x, y, &[c[0], c[1], c[2], if any_alpha { c[3] } else { 255 }]);
            }
        }
    }
    Ok(img)
}

/// a 565 color's channels (blue, green, red) and their maximums
fn rgb565(v: u32) -> ([u32; 3], [u32; 3]) {
    ([v & 31, v >> 5 & 63, v >> 11 & 31], [31, 63, 31])
}

/// one 4x4 block of BC1 (DXT1), BC2 (DXT3) or BC3 (DXT5): 16 straight BGRA pixels, row by row
/// ("Texture Block Compression in Direct3D 11"). MEASURED (golden wicdecode dds-*: 522 samples
/// of the interpolated colors): the thirds and halves are taken of the 5 and 6-bit values, then
/// scaled to 8 bits and rounded, halves down (one half in 3-color mode: 10.5 of 63 is 42), round((p * c0 + q * c1) / d * 255 / max)
pub fn bc_block(kind: u8, blk: &[u8]) -> [[u8; 4]; 16] {
    let (alpha, color) = if kind == 1 { (&blk[..0], blk) } else { (&blk[..8], &blk[8..]) };
    let c0 = rd16(color, 0).unwrap();
    let c1 = rd16(color, 2).unwrap();
    let ((a, m), (b, _)) = (rgb565(c0), rgb565(c1));
    let mix = |p: u32, q: u32, d: u32| -> [u32; 3] { [0, 1, 2].map(|i| (2 * (p * a[i] + q * b[i]) * 255 + d * m[i] - 1) / (2 * d * m[i])) };
    let four = kind != 1 || c0 > c1;
    let (e0, e1) = (mix(1, 0, 1), mix(0, 1, 1));
    let pal: [[u32; 4]; 4] = if four {
        let (x, y) = (mix(2, 1, 3), mix(1, 2, 3));
        [[e0[0], e0[1], e0[2], 255], [e1[0], e1[1], e1[2], 255], [x[0], x[1], x[2], 255], [y[0], y[1], y[2], 255]]
    } else {
        let x = mix(1, 1, 2);
        [[e0[0], e0[1], e0[2], 255], [e1[0], e1[1], e1[2], 255], [x[0], x[1], x[2], 255], [0, 0, 0, 0]]
    };
    let idx = rd32(color, 4).unwrap();
    let mut out = [[0u8; 4]; 16];
    // (MEASURED: BC3's alphas rounded, (p * a0 + q * a1 + d / 2) / d)
    let alphas: [u32; 8] = if kind == 3 {
        let (a0, a1) = (alpha[0] as u32, alpha[1] as u32);
        let m = |p: u32, q: u32, d: u32| (p * a0 + q * a1 + d / 2) / d;
        if a0 > a1 {
            [a0, a1, m(6, 1, 7), m(5, 2, 7), m(4, 3, 7), m(3, 4, 7), m(2, 5, 7), m(1, 6, 7)]
        } else {
            [a0, a1, m(4, 1, 5), m(3, 2, 5), m(2, 3, 5), m(1, 4, 5), 0, 255]
        }
    } else {
        [0; 8]
    };
    let abits = if kind == 3 { alpha[2..8].iter().rev().fold(0u64, |v, &x| v << 8 | x as u64) } else { 0 };
    for (i, o) in out.iter_mut().enumerate() {
        let c = pal[(idx >> (2 * i) & 3) as usize];
        let al = match kind {
            1 => c[3],
            2 => ((alpha[i / 2] >> (4 * (i % 2))) & 15) as u32 * 17,
            _ => alphas[(abits >> (3 * i) & 7) as usize],
        };
        *o = [c[0] as u8, c[1] as u8, c[2] as u8, al as u8];
    }
    out
}

/// DDS: BC1, BC2, BC3 ("DDS Format Overview"), each mip level a frame (ASSUMED), 32bppBGRA
fn dds(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let broken = Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN));
    // MEASURED (golden wicdecode cut-dds-*): a header cut short: WINCODEC_ERR_STREAMREAD
    if b.len() < 128 {
        hit(clause::WICIMAGE_DDS_SHORT);
        return Ok(Err(STREAM_READ));
    }
    let (Some(h), Some(w), Some(mips), Some(pf)) = (rd32(b, 12), rd32(b, 16), rd32(b, 28), rd32(b, 80)) else { return broken };
    if pf & 4 == 0 {
        // (MEASURED: an uncompressed file: WINCODEC_ERR_BADHEADER from the decoder)
        hit(clause::WICIMAGE_DDS_OTHER);
        return Ok(Err(0x8898_2f61));
    }
    // MEASURED (golden wicdecode dds-dxt2, -dxt4, -ati1, ...): DXT2 and DXT4 decode as DXT3 and
    // DXT5 into 32bppPBGRA (the colors as stored); BC4/BC5 FourCCs WINCODEC_ERR_BADHEADER
    let (kind, pre, start, slices) = match b.get(84..88) {
        Some(b"DXT1") => (1u8, true, 128, 1),
        Some(b"DXT2") => (2, true, 128, 1),
        Some(b"DXT3") => (2, false, 128, 1),
        Some(b"DXT4") => (3, true, 128, 1),
        Some(b"DXT5") => (3, false, 128, 1),
        Some(b"DX10") => {
            // MEASURED (golden wicdecode dds-dx10-*): of a DX10 header only BC1, BC2 and BC3
            // UNORM decode, into 32bppBGRA, each array slice a frame; the others (typeless,
            // sRGB, BC4-7, RGBA and BGRA) 0x88982f61
            let (Some(fmt), Some(array)) = (rd32(b, 128), rd32(b, 140)) else { return broken };
            let kind = match fmt {
                71 => 1u8,
                74 => 2,
                77 => 3,
                _ => {
                    hit(clause::WICIMAGE_DDS_DX10);
                    return Ok(Err(0x8898_2f61));
                }
            };
            if array > 1 && mips > 1 || array > 64 {
                return Err(Fail::Unmodeled(open::WICIMAGE_DDS_FOURCC));
            }
            hit(clause::WICIMAGE_DDS_DX10);
            (kind, false, 148usize, array.max(1))
        }
        _ => {
            hit(clause::WICIMAGE_DDS_OTHER);
            return Ok(Err(0x8898_2f61));
        }
    };
    // (the model does not hold pictures past 2^20 pixels, nor mips past 32)
    if w as u64 * h as u64 > 1 << 20 || mips > 32 {
        return broken;
    }
    let bs = if kind == 1 { 8 } else { 16 };
    let mut at = start;
    let mut frames = Vec::new();
    for m in (0..slices).flat_map(|_| 0..mips.max(1)) {
        let (mw, mh) = ((w >> m).max(1), (h >> m).max(1));
        let (bw, bh) = (mw.div_ceil(4), mh.div_ceil(4));
        // (ASSUMED: BC1 as premultiplied, its alpha all 0 or 255; BC2 and BC3 straight)
        let mut img = Img::new(mw, mh, if pre { "pbgra" } else { "bgra" });
        // MEASURED (golden wicdecode cut-dds-*): a level the file ends inside: CopyPixels
        // WINCODEC_ERR_STREAMREAD; one that starts past the end: E_INVALIDARG
        let need = (bw * bh) as usize * bs;
        if b.len() < at + need {
            hit(clause::WICIMAGE_DDS_LEVEL_SHORT);
            frames.push(Ok(Frame::NoPixels(img, if at > b.len() { E_INVALIDARG } else { STREAM_READ })));
            at += need;
            continue;
        }
        for by in 0..bh {
            for bx in 0..bw {
                let Some(blk) = b.get(at..at + bs) else { return broken };
                at += bs;
                let px = bc_block(kind, blk);
                for (i, p) in px.iter().enumerate() {
                    let (x, y) = (bx * 4 + i as u32 % 4, by * 4 + i as u32 / 4);
                    if x < mw && y < mh {
                        img.put(x, y, &if kind == 1 && p[3] == 0 { [0; 4] } else { *p });
                    }
                }
            }
        }
        frames.push(Ok(Frame::Pixels(img)));
    }
    hit(clause::WICIMAGE_DDS);
    Ok(Ok(Decoded { container: "dds", frames }))
}

/* ---------------------------------------------------------------------------------------- */
/* encoders */

/// the pixel formats each encoder takes as they are ("Native Pixel Formats Overview")
pub fn encoder_formats(c: &str) -> &'static [&'static str] {
    match c {
        "bmp" => &["idx1", "idx4", "idx8", "bgr555", "bgr565", "bgr24", "bgr32", "bgra", "pbgra", "rgba64fixed", "bgra64fixed"],
        "png" => &["idx1", "idx2", "idx4", "idx8", "bw", "gray2", "gray4", "gray8", "gray16", "bgr24", "bgra", "rgb48", "bgr48", "rgba64", "bgra64"],
        "tiff" => &["idx1", "idx4", "idx8", "bw", "gray4", "gray8", "gray16", "bgr24", "bgra", "cmyk", "rgb48", "rgba64"],
        "gif" => &["idx8"],
        "jpeg" => &["gray8", "bgr24", "cmyk"],
        "wmp" => &["bw", "gray8", "bgr555", "gray16", "bgr24", "rgb24", "bgr32", "bgra", "bgr101010", "rgb48", "rgba64", "cmyk", "rgbafloat", "rgbfloat", "cmyk64", "cmyka40", "cmyka80"],
        "dds" => &["bgra", "pbgra"],
        _ => &[],
    }
}

/// what an encoder option does, as far as measured
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodeOption {
    /// changes the file, not the pixels it decodes to
    Lossless,
    /// BMP's EnableV5Header32bppBGRA
    V5Header(bool),
    /// TIFF's CCITT compression: the frame takes BlackWhite
    BlackWhite,
    /// an option of another encoder: IPropertyBag2::Write fails (E_FAIL)
    Rejected,
    /// a value past the option's enumeration: IWICBitmapFrameEncode::Initialize fails
    /// WINCODEC_ERR_VALUEOUTOFRANGE
    OutOfRange,
    /// WMP's Lossless
    WmpLossless,
    /// JPEG's ImageQuality, as IJG's quality 0..=100
    JpegQuality(i64),
    /// JPEG's SuppressApp0
    JpegNoApp0(bool),
    /// JPEG's JpegYCrCbSubsampling 4:4:4
    Jpeg444,
    /// JPEG's BitmapTransform (a WICBitmapTransformOptions)
    JpegTransform(u32),
}

/// MEASURED (golden wicencode enc-jpeg-*: every byte and pixel, see model::jpeg): WIC's JPEG
/// encoder is IJG libjpeg 6b's at quality 90 (ImageQuality 0.9); the file decodes at 0 dpi
/// (JFIF's density 0), a CMYK one (Adobe, no JFIF) at 96
/// MEASURED (golden wicencode enc-dds-*: every byte): a DDS file of one BC3 (DXT5) surface:
/// the 124-byte header with caps, height, width, pixel format and linear size (the blocks'
/// bytes), depth 1 and 1 mip level, the FourCC DXT5 and caps TEXTURE; then the blocks
/// (src/model/bc.rs) left to right, top to bottom, a block past the picture repeating its last
/// column and row
fn encode_dds(i: &Img) -> Res<Vec<u8>> {
    if i.fmt != "bgra" {
        return Err(Fail::Bad);
    }
    let (bw, bh) = (i.w.div_ceil(4), i.h.div_ceil(4));
    let mut blocks = Vec::new();
    for by in 0..bh {
        for bx in 0..bw {
            let px: [crate::model::bc::Px; 16] = std::array::from_fn(|k| {
                let k = u32::try_from(k).unwrap_or(0);
                let (x, y) = ((bx * 4 + k % 4).min(i.w - 1), (by * 4 + k / 4).min(i.h - 1));
                let p = i.get(x, y);
                [p[2], p[1], p[0], p[3]].map(|v| f32::from(v) * (1.0 / 255.0))
            });
            let b = crate::model::bc::encode_bc3(&px).ok_or(Fail::Unmodeled(open::WICIMAGE_ENCODE_LOSSY))?;
            blocks.extend_from_slice(&b);
        }
    }
    hit(clause::WICIMAGE_ENCODE_DDS);
    let mut v = Vec::with_capacity(128 + blocks.len());
    v.extend_from_slice(b"DDS ");
    let le = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    let linear = u32::try_from(blocks.len()).map_err(|_| Fail::Bad)?;
    for x in [124, 0x000a_1007, i.h, i.w, linear, 1, 1] {
        le(&mut v, x);
    }
    v.resize(v.len() + 44, 0);
    for x in [32, 4] {
        le(&mut v, x);
    }
    v.extend_from_slice(b"DXT5");
    v.resize(v.len() + 20, 0);
    le(&mut v, 0x1000);
    v.resize(128, 0);
    v.extend_from_slice(&blocks);
    Ok(v)
}

fn encode_jpeg(i: &Img, opts: EncodeOptions) -> Res<Encoded> {
    use crate::model::jpeg::{self, Kind};

    let kind = match i.fmt {
        "gray8" => Kind::Gray,
        "bgr24" => Kind::Color,
        "cmyk" => Kind::Cmyk,
        _ => return Err(Fail::Bad),
    };
    let q = jpeg::Quality::new(opts.jpeg_quality.unwrap_or(90));
    let turned;
    let i = if opts.jpeg_transform == 0 { i } else { turned = flip(i, opts.jpeg_transform); &turned };
    let (w, h) = (i.w as usize, i.h as usize);
    let n = (bits_of(i.fmt) / 8) as usize;
    let px: Vec<u8> = (0..h).flat_map(|y| i.px[y * i.stride()..y * i.stride() + w * n].to_vec()).collect();
    let j = jpeg::encode(kind, w, h, &px, q, !opts.jpeg_no_app0, opts.jpeg_444);
    hit(clause::WICIMAGE_ENCODE_JPEG);
    let mut o = Img::new(w as u32, h as u32, i.fmt);
    o.dpi = if kind == Kind::Cmyk || opts.jpeg_no_app0 { (96.0, 96.0) } else { (0.0, 0.0) };
    let st = o.stride();
    for y in 0..h {
        o.px[y * st..y * st + w * n].copy_from_slice(&j.px[y * w * n..(y + 1) * w * n]);
    }
    Ok(Encoded::Lossy(j.bytes, "jpeg", o))
}

#[derive(Default, Clone, Copy)]
struct EncodeOptions {
    jpeg_quality: Option<i64>,
    jpeg_no_app0: bool,
    jpeg_444: bool,
    jpeg_transform: u32,
    v5: bool,
    bw: bool,
    wmp_lossless: bool,
}

/// MEASURED (golden wicencode opt-*: every option the cases set, on BGRA, BGR and gray sources):
/// PNG's InterlaceOption and FilterOption (1 to 6), TIFF's TiffCompressionMethod 0 (don't care),
/// 1 (none), 4 (LZW), 5 (RLE), 6 (ZIP) and 7 (LZW with differencing) change only the file;
/// TiffCompressionMethod 2 (CCITT3) and 3 (CCITT4) make the frame BlackWhite
/// (golden opt-tiff-*); WMP's Lossless and ImageQuality 1.0 keep every pixel; PNG takes no ImageQuality. Other names and values are not measured.
pub fn option(cont: &str, o: &str) -> Res<EncodeOption> {
    let (name, val) = o.split_once('=').ok_or(Fail::Bad)?;
    Ok(match (cont, name, val) {
        ("png", "InterlaceOption", "0" | "1") | ("png", "FilterOption", "1" | "2" | "3" | "4" | "5" | "6") | ("tiff", "TiffCompressionMethod", "0" | "1" | "4" | "5" | "6" | "7") => EncodeOption::Lossless,
        ("bmp", "EnableV5Header32bppBGRA", "0") => EncodeOption::V5Header(false),
        ("bmp", "EnableV5Header32bppBGRA", "1") => EncodeOption::V5Header(true),
        ("tiff", "TiffCompressionMethod", "2" | "3") => EncodeOption::BlackWhite,
        // (WICTiffCompressionOption ends at 7, WICTiffCompressionLZWHDifferencing; MEASURED golden
        // opt-tiff-8-bgra: 8 fails the frame's Initialize 0x88982f05)
        ("tiff", "TiffCompressionMethod", v) if v.parse::<u8>().is_ok_and(|v| v > 7) => EncodeOption::OutOfRange,
        ("png", "ImageQuality", _) => EncodeOption::Rejected,
        // (Encoder options: JPEG XR's ImageQuality 1.0 is lossless; MEASURED golden opt-19-*: every
        // pixel kept, as Lossless=1)
        ("wmp", "Lossless", "1") | ("wmp", "ImageQuality", "1.0" | "1") => EncodeOption::WmpLossless,
        ("jpeg", "ImageQuality", v) => EncodeOption::JpegQuality((v.parse::<f64>().map_err(|_| Fail::Bad)? * 100.0).round() as i64),
        ("jpeg", "SuppressApp0", v) => EncodeOption::JpegNoApp0(v != "0"),
        ("jpeg", "JpegYCrCbSubsampling", "3") => EncodeOption::Jpeg444,
        ("jpeg", "BitmapTransform", "1") => EncodeOption::JpegTransform(1),
        _ => return Err(Fail::Unmodeled(open::WICIMAGE_ENCODE_OPTIONS)),
    })
}

pub enum Encoded {
    Bytes(Vec<u8>),
    /// a lossless file the model has no bytes of: what decodes from it
    Decodes(&'static str, Img),
    /// a file the model has the bytes of and, since its decoder is not modelled apart, what
    /// Windows' decoder gives from it
    Lossy(Vec<u8>, &'static str, Img),
}

/* ---------------------------------------------------------------------------------------- */

#[derive(Default)]
pub struct WicImage {
    cur: Option<Img>,
    fail: Option<FrameFail>,
    dec: Option<Decoded>,
    enc: Option<Encoded>,
}

const OPS: &[&str] = &["wbits", "wscale", "wflip", "wclip", "wconv", "wcolor", "wctx", "winfo", "wpixels", "wpalette", "wsrcpal", "wdecode", "wframe", "wencode", "wbytes", "wredecode"];

fn hexs(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

impl WicImage {
    pub fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        let t: Vec<&str> = op.tok.iter().map(String::as_str).collect();
        if !OPS.contains(&t[0]) {
            return Ok(false);
        }
        let n = |i: usize| t.get(i).and_then(|v| crate::case::num(v)).ok_or(Fail::Bad);
        let needs_cur = !matches!(t[0], "wbits" | "wctx" | "wpalette" | "wdecode" | "wframe" | "wbytes" | "wredecode");
        if needs_cur {
            match (self.fail, t[0]) {
                // (a frame without size or format: the executor prints its failure and knows no
                // bits a pixel to copy with)
                (Some(FrameFail::NoInfo(hr)), "winfo") => {
                    out.push(format!("winfo {hr:08x} 0 0 {{00000000-0000-0000-0000-000000000000}} 0.000 0.000"));
                    return Ok(true);
                }
                (Some(FrameFail::NoInfo(_)), "wpixels") => {
                    out.push("wpixels unknown".into());
                    return Ok(true);
                }
                (Some(FrameFail::NoPixels(hr)), "wpixels") if t.len() == 1 => {
                    out.push(format!("wpixels {hr:08x} {}", bits_of(self.cur.as_ref().map_or("bgra", |c| c.fmt))));
                    return Ok(true);
                }
                (Some(FrameFail::NoPixels(_)), "winfo") | (None, _) => {}
                (Some(_), _) => return Err(Fail::Unmodeled(open::WICIMAGE_DECODE_BROKEN)),
            }
        } else if t[0] == "wbits" || t[0] == "wdecode" {
            self.fail = None;
        }
        if needs_cur && self.cur.as_ref().is_some_and(|c| c.px.is_empty()) && !matches!(t[0], "winfo" | "wscale") {
            return Err(Fail::Unmodeled(open::WICIMAGE_SCALE_BIG));
        }
        if needs_cur && self.cur.is_none() {
            if t[0] == "wpixels" && ![1, 5, 7].contains(&t.len()) {
                return Err(Fail::Bad);
            }
            hit(clause::WICIMAGE_NOSOURCE);
            out.push(format!("{} -", t[0]));
            return Ok(true);
        }
        match t[0] {
            "wbits" => {
                let f = crate::model::wic::format(t.get(1).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?;
                let (w, h) = (n(2)?, n(3)?);
                if t.len() != 5 || !(1..=1024).contains(&w) || !(1..=1024).contains(&h) {
                    return Err(Fail::Bad);
                }
                let seed = if t[4] == "ramp" { None } else { Some(n(4)? as u32) };
                out.push("wbits 00000000".into());
                self.cur = Some(bitmap(crate::model::wic::FORMATS[f].0, w as u32, h as u32, seed));
            }
            "wscale" => {
                let (w, h) = (n(1)?, n(2)?);
                let m = *t.get(3).ok_or(Fail::Bad)?;
                if !["nn", "linear", "cubic", "fant", "hqcubic"].contains(&m) {
                    return Err(Fail::Bad);
                }
                let (w, h) = (w as u32 as u64, h as u32 as u64);
                let c = self.cur.clone().unwrap(); let c = &c;
                // MEASURED (golden wicscale size-*, bigsize-*): a size of 0 E_INVALIDARG, one
                // whose rows (in bits) or picture (in bytes, rows padded to 4 bytes) pass 32 bits arithmetic overflow, the
                // source kept; others are made, however large
                let bits = bits_of(scale_formats(c.fmt, m).0) as u64;
                let hr = if w == 0 || h == 0 {
                    E_INVALIDARG
                } else if w * bits >= 1 << 32 || (w * bits).div_ceil(32) * 4 * h >= 1 << 32 {
                    ARITHMETIC_OVERFLOW
                } else {
                    0
                };
                hit(clause::WICIMAGE_SCALE_SIZE);
                out.push(format!("wscale {hr:08x}"));
                if hr != 0 {
                    return Ok(true);
                }
                if w * h > 1 << 22 || c.px.is_empty() {
                    // (a picture the model does not hold: its size only)
                    let (wf, _) = scale_formats(c.fmt, m);
                    self.cur = Some(Img { w: w as u32, h: h as u32, px: vec![], fmt: scale_formats(c.fmt, m).1, ..Img::new(1, 1, wf) });
                    self.cur.as_mut().unwrap().dpi = c.dpi;
                    return Ok(true);
                }
                let s = scale(c, w as u32, h as u32, m)?;
                self.cur = Some(s);
            }
            "wflip" => {
                // MEASURED (golden wictransform flipbad-*: 17 values): the bits besides the
                // rotation and the two flips are ignored
                let o = n(1)? as u32;
                if o & !0x1b != 0 {
                    hit(clause::WICIMAGE_FLIP_BAD);
                }
                let o = o & 0x1b;
                hit(clause::WICIMAGE_FLIP);
                out.push("wflip 00000000".into());
                self.cur = Some(flip(self.cur.as_ref().unwrap(), o));
            }
            "wclip" => {
                let (x, y, w, h) = (n(1)?, n(2)?, n(3)?, n(4)?);
                let c = self.cur.as_ref().unwrap();
                if !inside(c, x, y, w, h) {
                    hit(clause::WICIMAGE_CLIP_RECT);
                    out.push(format!("wclip {E_INVALIDARG:08x}"));
                } else {
                    hit(clause::WICIMAGE_CLIP);
                    out.push("wclip 00000000".into());
                    self.cur = Some(clip(c, x as u32, y as u32, w as u32, h as u32));
                }
            }
            "wconv" => {
                let to = fmt_static(t.get(1).ok_or(Fail::Bad)?);
                hit(clause::WICIMAGE_CONVERT);
                let c = wic_convert(self.cur.as_ref().unwrap(), to)?;
                out.push("wconv 00000000".into());
                self.cur = Some(c);
            }
            "wcolor" => {
                let (a, b, f) = (n(1)?, n(2)?, fmt_static(t.get(3).ok_or(Fail::Bad)?));
                // MEASURED (golden wiccolor cm-*-01, -10, -32, -655351-*: every pixel as from 1):
                // a context of any value but 2 is sRGB
                let srgb = |v: i64| if v == 2 { 2 } else { 1 };
                if a != 1 && a != 2 || b != 1 && b != 2 {
                    hit(clause::WICIMAGE_COLOR_EXIF_BAD);
                }
                let (a, b) = (srgb(a), srgb(b));
                let Some(c) = &self.cur else {
                    out.push("wcolor -".into());
                    return Ok(true);
                };
                let hr = color_transform(c.fmt, f, a == b);
                hit(clause::WICIMAGE_COLOR_INIT);
                out.push(format!("wcolor 00000000 00000000 {hr:08x}"));
                if hr == 0 {
                    if !(a == b && c.fmt == f) || COLOR_RGB.contains(&c.fmt) {
                        return Err(Fail::Unmodeled(open::WICIMAGE_COLOR_TRANSFORM));
                    }
                    // (MEASURED: golden wiccolor xf-idx8-idx8-11, xf-cmyk-cmyk-11, xf-gray8-gray8-11:
                    // the source's pixels as they are)
                    hit(clause::WICIMAGE_COLOR_SAME);
                }
            }
            "wctx" => {
                let v = n(1)?;
                // (MEASURED, golden wiccolor ctx-*: any value taken and given back)
                hit(if v == 1 || v == 2 { clause::WICIMAGE_COLOR_EXIF } else { clause::WICIMAGE_COLOR_EXIF_BAD });
                out.push(format!("wctx 00000000 type 2 exif 00000000 {}", v as u32));
            }
            "winfo" => {
                let c = self.cur.as_ref().unwrap();
                hit(clause::WICIMAGE_INFO);
                out.push(format!("winfo 00000000 {} {} {} {:.3} {:.3}", c.w, c.h, c.fmt, c.dpi.0, c.dpi.1));
            }
            "wpixels" => {
                let c = self.cur.as_ref().unwrap();
                let rect = match t.len() {
                    1 => None,
                    5 | 7 => Some((n(1)?, n(2)?, n(3)?, n(4)?)),
                    _ => return Err(Fail::Bad),
                };
                let bufs = if t.len() == 7 { Some((n(5)?, n(6)?)) } else { None };
                let (hr, rows) = copy(c, rect, bufs);
                out.push(format!("wpixels {hr:08x} {}", bits_of(c.fmt)));
                let (w, b) = (rect.map_or(c.w as i64, |r| r.2), bits_of(c.fmt) as i64);
                for mut r in rows {
                    let tail = w * b % 8;
                    if tail != 0 {
                        let l = r.len() - 1;
                        r[l] &= (0xff00u32 >> tail) as u8;
                    }
                    out.push(hexs(&r));
                }
            }
            "wpalette" => {
                let (ty, a) = (n(1)?, n(2)?);
                let (hr, t, c) = predefined(ty as u32, a != 0)?;
                palette_line("wpalette", hr, t, &c, out);
            }
            "wsrcpal" => {
                let c = self.cur.as_ref().unwrap();
                match &c.pal {
                    Some(p) => {
                        hit(clause::WICIMAGE_SRCPAL);
                        palette_line("wsrcpal", 0, 0, p, out)
                    }
                    None => {
                        hit(clause::WICIMAGE_SRCPAL_NONE);
                        out.push(format!("wsrcpal {PALETTE_UNAVAILABLE:08x} type 0 count 0 bw 0 gray 0 alpha 0"));
                    }
                }
            }
            "wdecode" => {
                let h = *t.get(1).ok_or(Fail::Bad)?;
                if t.len() != 2 || h.len() % 2 != 0 || !h.is_ascii() {
                    return Err(Fail::Bad);
                }
                let b: Option<Vec<u8>> = (0..h.len() / 2).map(|i| u8::from_str_radix(&h[2 * i..2 * i + 2], 16).ok()).collect();
                let b = b.ok_or(Fail::Bad)?;
                self.decode_into("wdecode", &b, out)?;
            }
            "wframe" => {
                let i = n(1)? as usize;
                let Some(d) = &self.dec else {
                    out.push("wframe -".into());
                    return Ok(true);
                };
                let Some(f) = d.frames.get(i) else {
                    // (MEASURED, golden wicdecode cut-ico-*: past the frames
                    // WINCODEC_ERR_FRAMEMISSING, the source as it was)
                    hit(clause::WICIMAGE_FRAME_MISSING);
                    out.push("wframe 88982f62".into());
                    return Ok(true);
                };
                let f = f.clone()?;
                hit(clause::WICIMAGE_FRAME);
                out.push("wframe 00000000".into());
                match f {
                    Frame::Pixels(f) => {
                        self.cur = Some(f);
                        self.fail = None;
                    }
                    Frame::NoInfo(hr) => {
                        self.cur = Some(Img::new(0, 0, "bgra"));
                        self.fail = Some(FrameFail::NoInfo(hr));
                    }
                    Frame::NoPixels(f, hr) => {
                        self.cur = Some(f);
                        self.fail = Some(FrameFail::NoPixels(hr));
                    }
                }
            }
            "wencode" => {
                let Some(c) = self.cur.clone() else {
                    out.push("wencode -".into());
                    return Ok(true);
                };
                let cont = *t.get(1).ok_or(Fail::Bad)?;
                let mut first = 2;
                let mut fmt = c.fmt;
                if t.len() > 2 && !t[2].contains('=') {
                    fmt = fmt_static(t[2]);
                    first = 3;
                }
                self.enc = None;
                // (MEASURED: HEIF's and WebP's encoders are not there on windows-2025)
                if let Some(hr) = match cont {
                    "heif" => Some(0x88982f8bu32),
                    "webp" => Some(COMPONENT_NOT_FOUND),
                    _ => None,
                } {
                    hit(clause::WICIMAGE_ENCODE_MISSING);
                    out.push(format!("wencode {hr:08x} {fmt} create"));
                    return Ok(true);
                }
                let mut opts = EncodeOptions::default();
                for o in &t[first..] {
                    match option(cont, o)? {
                        EncodeOption::Lossless => hit(clause::WICIMAGE_ENCODE_OPTION_LOSSLESS),
                        EncodeOption::V5Header(v5) => {
                            hit(clause::WICIMAGE_ENCODE_OPTION_LOSSLESS);
                            opts.v5 = v5;
                        }
                        EncodeOption::BlackWhite => {
                            hit(clause::WICIMAGE_ENCODE_OPTION_CCITT);
                            opts.bw = true;
                        }
                        EncodeOption::WmpLossless => {
                            hit(clause::WICIMAGE_ENCODE_OPTION_LOSSLESS);
                            opts.wmp_lossless = true;
                        }
                        EncodeOption::JpegQuality(q) => {
                            hit(clause::WICIMAGE_ENCODE_JPEG);
                            opts.jpeg_quality = Some(q);
                        }
                        EncodeOption::JpegNoApp0(n) => {
                            hit(clause::WICIMAGE_ENCODE_JPEG);
                            opts.jpeg_no_app0 = n;
                        }
                        EncodeOption::JpegTransform(t) => {
                            hit(clause::WICIMAGE_ENCODE_JPEG);
                            opts.jpeg_transform = t;
                        }
                        EncodeOption::Jpeg444 => {
                            hit(clause::WICIMAGE_ENCODE_JPEG);
                            opts.jpeg_444 = true;
                        }
                        EncodeOption::Rejected => {
                            hit(clause::WICIMAGE_ENCODE_OPTION_FOREIGN);
                            out.push(format!("wencode 80004005 {fmt} option"));
                            return Ok(true);
                        }
                        EncodeOption::OutOfRange => {
                            hit(clause::WICIMAGE_ENCODE_OPTION_RANGE);
                            out.push(format!("wencode 88982f05 {fmt} frameinit"));
                            return Ok(true);
                        }
                    }
                }
                let nf = if opts.bw { "bw" } else { negotiate(cont, fmt).ok_or(Fail::Bad)? };
                hit(if nf == fmt { clause::WICIMAGE_ENCODE_FORMAT } else { clause::WICIMAGE_ENCODE_NEGOTIATE });
                let img = encode_source(&c, nf)?;
                let enc = match cont {
                    // (MEASURED: golden opt-0-bgra: a v5 header keeps 32bppBGRA's alpha, it decodes as
                    // 32bppBGRA; without, as 32bppBGR)
                    "bmp" if opts.v5 && img.fmt == "bgra" => Encoded::Decodes("bmp", img),
                    "bmp" => Encoded::Bytes(encode_bmp(&img)?),
                    "png" | "tiff" => {
                        hit(clause::WICIMAGE_ENCODE_LOSSLESS);
                        Encoded::Decodes(if cont == "png" { "png" } else { "tiff" }, img)
                    }
                    "gif" => Encoded::Bytes(encode_gif(&img)?),
                    "wmp" if opts.wmp_lossless => Encoded::Decodes("wmp", img),
                    "jpeg" => encode_jpeg(&img, opts)?,
                    "dds" => Encoded::Bytes(encode_dds(&img)?),
                    _ => return Err(Fail::Unmodeled(open::WICIMAGE_ENCODE_LOSSY)),
                };
                out.push(format!("wencode 00000000 {nf} ok"));
                self.enc = Some(enc);
            }
            "wbytes" => match &self.enc {
                Some(Encoded::Bytes(b) | Encoded::Lossy(b, _, _)) => {
                    let (from, len) = if t.len() == 3 { (n(1)? as usize, n(2)? as usize) } else { (0, b.len()) };
                    let from = from.min(b.len());
                    out.push(format!("wbytes {}", b.len()));
                    out.push(hexs(&b[from..(from + len).min(b.len())]));
                }
                Some(_) => return Err(Fail::Bad),
                None => {
                    out.push("wbytes 0".into());
                    out.push(String::new());
                }
            },
            "wredecode" => match self.enc.take() {
                None => out.push("wredecode -".into()),
                Some(Encoded::Bytes(b)) => {
                    self.decode_into("wredecode", &b, out)?;
                    self.enc = Some(Encoded::Bytes(b));
                }
                Some(Encoded::Decodes(cont, img)) => {
                    out.push(format!("wredecode 00000000 {cont} frames 1"));
                    let mut i = img.clone();
                    // (MEASURED: written at 96 dpi when the source has none, but by WMP's
                    // encoder (golden opt-18-*: 0 dpi))
                    if i.dpi == (0.0, 0.0) && cont != "wmp" {
                        i.dpi = (96.0, 96.0);
                    }
                    i.pal = img.pal.clone();
                    self.dec = Some(Decoded { container: cont, frames: vec![Ok(Frame::Pixels(i))] });
                    self.enc = Some(Encoded::Decodes(cont, img));
                }
                Some(Encoded::Lossy(b, cont, img)) => {
                    out.push(format!("wredecode 00000000 {cont} frames 1"));
                    self.dec = Some(Decoded { container: cont, frames: vec![Ok(Frame::Pixels(img.clone()))] });
                    self.enc = Some(Encoded::Lossy(b, cont, img));
                }
            },
            _ => unreachable!(),
        }
        Ok(true)
    }

    fn decode_into(&mut self, op: &str, b: &[u8], out: &mut Vec<String>) -> Res<()> {
        match decode(b)? {
            Ok(d) => {
                out.push(format!("{op} 00000000 {} frames {}", d.container, d.frames.len()));
                self.dec = Some(d);
            }
            Err(hr) => {
                out.push(format!("{op} {hr:08x} - frames 0"));
                self.dec = None;
            }
        }
        Ok(())
    }
}

/// the sources IWICColorTransform takes between RGB contexts
const COLOR_RGB: [&str; 7] = ["bgr24", "bgr565", "bgra", "pbgra", "rgb24", "rgba", "rgba64"];

/// MEASURED (golden wiccolor xf-*: 12 sources into 8 formats between every pair of the sRGB and
/// Adobe RGB contexts): IWICColorTransform::Initialize's answer. A source in the same context and
/// format is taken as it is; else an RGB source into 24bppBGR, 32bppBGRA, 32bppPBGRA or 64bppRGBA,
/// and nothing else: indexed and BlackWhite sources and the indexed and 48bppRGB targets
/// WINCODEC_ERR_UNSUPPORTEDPIXELFORMAT, gray and CMYK ones 0x800707db (ERROR_INVALID_TRANSFORM)
pub fn color_transform(from: &str, to: &str, same_context: bool) -> u32 {
    let rgb = COLOR_RGB.contains(&from);
    if same_context && from == to {
        0
    } else if from == "bw" || from.starts_with("idx") || (rgb && (to.starts_with("idx") || to == "rgb48")) {
        0x88982f80
    } else if !rgb || !["bgr24", "bgra", "pbgra", "rgba64"].contains(&to) {
        0x800707db
    } else {
        0
    }
}

/// MEASURED (golden wicencode enc-*: 23 source formats into each encoder): the format
/// SetPixelFormat answers for one the encoder does not take
pub fn negotiate(cont: &str, f: &'static str) -> Option<&'static str> {
    let gray = ["bw", "gray2", "gray4", "gray8", "gray16"].contains(&f);
    let wide = ["gray16", "rgb48", "rgba64"].contains(&f);
    let straight = ["rgba", "prgba", "alpha8"].contains(&f);
    Some(match cont {
        "bmp" => match f {
            "bw" | "idx1" => "idx1",
            "gray2" | "gray4" | "idx2" | "idx4" => "idx4",
            "gray8" | "idx8" => "idx8",
            _ if wide => "rgba64fixed",
            "bgr555" | "bgr565" | "bgr24" | "bgr32" | "bgra" | "pbgra" => f,
            _ if straight => "bgra",
            _ => "bgr24",
        },
        "png" | "tiff" => match f {
            "gray2" if cont == "tiff" => "gray4",
            "idx2" if cont == "tiff" => "idx4",
            "cmyk" if cont == "tiff" => "cmyk",
            _ if gray || wide => f,
            "bgr24" | "bgra" | "idx1" | "idx2" | "idx4" | "idx8" => f,
            "pbgra" => "bgra",
            _ if straight => "bgra",
            _ => "bgr24",
        },
        "gif" => "idx8",
        "jpeg" => match f {
            _ if gray || f == "alpha8" => "gray8",
            "cmyk" => "cmyk",
            _ => "bgr24",
        },
        "wmp" => match f {
            "gray2" | "gray4" | "alpha8" => "gray8",
            "rgb32" => "rgb24",
            "rgba" => "bgra",
            "prgba" => "pbgra",
            _ if f.starts_with("idx") => "rgb24",
            _ => f,
        },
        "dds" => "bgra",
        _ => return None,
    })
}

/// MEASURED (golden wicencode encbw-*: 16 x 16 of every gray level and of the primaries every 5
/// levels, all as Windows): a color source written as BlackWhite: its gray (as 8bppGray) dithered
/// by Floyd and Steinberg's error diffusion (7, 3, 5, 1 sixteenths), serpentine with the first
/// row right to left, the errors kept in sixteenths and read back floored, each value clamped
/// to 0..=255, white from 128
pub fn dither_bw(g: &Img) -> Img {
    let mut o = Img::new(g.w, g.h, "bw");
    o.dpi = g.dpi;
    let gray = |x: usize, y: usize| [i32::from(g.px[y * g.stride() + x]); 3];
    // (black and white: the nearest to a cell's center is white from cell 8, so white from 128)
    let ix = dither(g.w as usize, g.h as usize, &gray, &[[0; 3], [255; 3]], &|_, _| false);
    let ost = o.stride();
    for (k, i) in ix.into_iter().enumerate() {
        let (x, y) = (k % g.w as usize, k / g.w as usize);
        if i == Some(1) {
            o.px[y * ost + x / 8] |= 0x80 >> (x % 8);
        }
    }
    o
}

/// MEASURED (golden wicencode encq-*, enc-gif-*, encbw-*: every pixel as Windows): pixels into a
/// palette by Floyd and Steinberg's error diffusion (7, 3, 5, 1 sixteenths), serpentine with the
/// first row right to left, errors kept in sixteenths and read back floored, each channel
/// clamped to 0..=255; a pixel takes the entry nearest (squared distance, the first on a tie) to
/// the center 17*(v>>4) of its 4-bit cell, and its error is against that entry. `clear` pixels
/// get None, their error spent all the same (golden gifalpha-full16: 6 pixels otherwise).
pub fn dither(w: usize, h: usize, px: &dyn Fn(usize, usize) -> [i32; 3], pal: &[[i32; 3]], clear: &dyn Fn(usize, usize) -> bool) -> Vec<Option<usize>> {
    let mut cells: std::collections::HashMap<[i32; 3], usize> = std::collections::HashMap::new();
    let mut near = |v: [i32; 3]| -> usize {
        let cell = v.map(|c| c >> 4);
        *cells.entry(cell).or_insert_with(|| {
            let ctr = cell.map(|c| 17 * c);
            let d = |p: &[i32; 3]| (0..3).map(|k| (p[k] - ctr[k]).pow(2)).sum::<i32>();
            (0..pal.len()).min_by_key(|&i| (d(&pal[i]), i)).unwrap_or(0)
        })
    };
    let mut out = vec![None; w * h];
    let mut cur = vec![[0i32; 3]; w + 2];
    let mut next = vec![[0i32; 3]; w + 2];
    for y in 0..h {
        let rtl = y % 2 == 0;
        for i in 0..w {
            let x = if rtl { w - 1 - i } else { i };
            let s = px(x, y);
            let v: [i32; 3] = std::array::from_fn(|k| (s[k] + (cur[x + 1][k] >> 4)).clamp(0, 255));
            let n = near(v);
            if !clear(x, y) {
                out[y * w + x] = Some(n);
            }
            let (ahead, behind) = if rtl { (x, x + 2) } else { (x + 2, x) };
            for k in 0..3 {
                let e = v[k] - pal.get(n).map_or(0, |p| p[k]);
                cur[ahead][k] += 7 * e;
                next[behind][k] += 3 * e;
                next[x + 1][k] += 5 * e;
                next[ahead][k] += e;
            }
        }
        cur = std::mem::replace(&mut next, vec![[0; 3]; w + 2]);
    }
    out
}

/// what WriteSource gives the encoder: an indexed source keeps its indices and palette in a
/// wider index format; any other source written as indices has the palette of its colors in
/// the order they first appear (MEASURED: golden wicencode enc-bmp-bw/gray*, encas-*-idx8: the
/// sources' colors, all fewer than the palette holds); else the source converted
fn encode_source(c: &Img, nf: &'static str) -> Res<Img> {
    if c.fmt == nf {
        return Ok(c.clone());
    }
    // (MEASURED: golden opt-10-gray8: an 8bppGray source dithers as a color one's gray)
    if nf == "bw" && c.fmt == "gray8" {
        hit(clause::WICIMAGE_ENCODE_BW);
        return Ok(dither_bw(c));
    }
    if nf == "bw" && !c.fmt.starts_with("gray") {
        hit(clause::WICIMAGE_ENCODE_BW);
        return Ok(dither_bw(&wic_convert(c, "gray8")?));
    }
    if !nf.starts_with("idx") {
        hit(clause::WICIMAGE_ENCODE_CONVERT);
        return wic_convert(c, nf);
    }
    let mut o = Img::new(c.w, c.h, nf);
    o.dpi = c.dpi;
    if c.fmt == "bw" {
        // (MEASURED: golden enc-bmp-bw: the rows as they are, even the bits past their end)
        hit(clause::WICIMAGE_ENCODE_BWINDEX);
        o.pal = Some(vec![0xff00_0000, 0xffff_ffff]);
        if nf == "idx1" {
            o.px = c.px.clone();
            return Ok(o);
        }
        for y in 0..c.h {
            for x in 0..c.w {
                o.put(x, y, &[u8::from(c.get(x, y)[0] != 0)]);
            }
        }
        return Ok(o);
    }
    if c.fmt.starts_with("idx") {
        hit(clause::WICIMAGE_ENCODE_WIDEN);
        o.pal = c.pal.clone();
        for y in 0..c.h {
            for x in 0..c.w {
                o.put(x, y, &c.get(x, y));
            }
        }
        return Ok(o);
    }
    let b = wic_convert(c, "bgra")?;
    // MEASURED (golden wicencode gifalpha-alpha: every alpha, 0..=0x7f transparent; enc-gif-bgra,
    // -pbgra, -rgba, -prgba, -rgba64, -alpha8): the palette is the colors with their alpha dropped,
    // in the order they first come, and one transparent entry after them takes every pixel of
    // alpha below 0x80
    let alpha = |x: u32, y: u32| b.get(x, y)[3];
    let clear = |x: usize, y: usize| alpha(x as u32, y as u32) < 0x80;
    let mut pal: Vec<u32> = vec![];
    for y in 0..c.h {
        for x in 0..c.w {
            let v = u32::from_le_bytes(b.get(x, y).try_into().map_err(|_| Fail::Bad)?) | 0xff00_0000;
            if !pal.contains(&v) {
                pal.push(v);
            }
        }
    }
    // past the palette's size: boxes of the cube (grid_palette); with a transparent entry beside
    // them open (gifalpha-full256)
    if pal.len() > 1usize << bits_of(nf) {
        if (0..c.h).any(|y| (0..c.w).any(|x| clear(x as usize, y as usize))) {
            return Err(Fail::Unmodeled(open::WICIMAGE_ENCODE_QUANTIZE));
        }
        pal = grid_palette(&b, bits_of(nf))?;
    }
    let rgb: Vec<[i32; 3]> = pal.iter().map(|&p| [16, 8, 0].map(|s| ((p >> s) & 0xff) as i32)).collect();
    let src = |x: usize, y: usize| {
        let p = b.get(x as u32, y as u32);
        [i32::from(p[2]), i32::from(p[1]), i32::from(p[0])]
    };
    let (w, h) = (c.w as usize, c.h as usize);
    let ix = dither(w, h, &src, &rgb, &clear);
    let any_clear = (0..h).any(|y| (0..w).any(|x| clear(x, y)));
    let t = pal.len();
    if any_clear {
        if t >> bits_of(nf) != 0 {
            return Err(Fail::Unmodeled(open::WICIMAGE_ENCODE_QUANTIZE));
        }
        hit(clause::WICIMAGE_ENCODE_TRANSPARENT);
        pal.push(0);
    }
    for (k, i) in ix.into_iter().enumerate() {
        let v = u8::try_from(i.unwrap_or(t)).map_err(|_| Fail::Bad)?;
        o.put((k % w) as u32, (k / w) as u32, &[v]);
    }
    hit(clause::WICIMAGE_ENCODE_PALETTE);
    o.pal = Some(pal);
    Ok(o)
}

/// a box of the 5-bit RGB cube (both ends in), how many halvings made it (its volume is the
/// cube's halved that often) and how many pixels fall in it
#[derive(Clone, Copy)]
struct Box5 {
    lo: [usize; 3],
    hi: [usize; 3],
    depth: u32,
    count: u32,
}

/// MEASURED (golden wicencode encpal-*, encq-*, gridtie-*: every palette, including the order
/// of equal counts): more colors than the palette holds take boxes of the 5-bit RGB cube: from
/// the whole cube, the box of most volume (then most pixels, then the first) is halved across
/// its longest side (red, then green, then blue first) at the middle, the upper half keeping its
/// place and the lower put last, until there are as many boxes as entries; then the boxes ordered
/// by their pixels, most first, equal counts as they stand; each entry the middle of its box,
/// widened by 255 / 31 rounded (so a picture whose colors fill the cube takes the 2x1x1, 4x2x2,
/// 8x8x4 grid). OPEN (ENCODE.QUANTIZE): 256 entries where most boxes would be empty
/// (gridtie-idx8-few-* split the full boxes finer)
fn grid_palette(b: &Img, bits: u32) -> Res<Vec<u32>> {
    let at = |r: usize, g: usize, bl: usize| (r * 32 + g) * 32 + bl;
    let mut hist = vec![0u32; 32 * 32 * 32];
    for y in 0..b.h {
        for x in 0..b.w {
            let p = b.get(x, y);
            hist[at(usize::from(p[2] >> 3), usize::from(p[1] >> 3), usize::from(p[0] >> 3))] += 1;
        }
    }
    let count = |lo: [usize; 3], hi: [usize; 3]| -> u32 {
        let mut n = 0;
        for r in lo[0]..=hi[0] {
            for g in lo[1]..=hi[1] {
                for bl in lo[2]..=hi[2] {
                    n += hist[at(r, g, bl)];
                }
            }
        }
        n
    };
    let total = count([0; 3], [31; 3]);
    let mut boxes = vec![Box5 { lo: [0; 3], hi: [31; 3], depth: 0, count: total }];
    while boxes.len() < 1 << bits {
        let best = (0..boxes.len()).max_by_key(|&i| (std::cmp::Reverse(boxes[i].depth), boxes[i].count, std::cmp::Reverse(i))).ok_or(Fail::Bad)?;
        let bx = boxes[best];
        let axis = (0..3).max_by_key(|&k| (bx.hi[k] - bx.lo[k], std::cmp::Reverse(k))).ok_or(Fail::Bad)?;
        if bx.hi[axis] == bx.lo[axis] {
            return Err(Fail::Bad);
        }
        let mid = (bx.lo[axis] + bx.hi[axis]) / 2;
        let mut lower = Box5 { depth: bx.depth + 1, ..bx };
        lower.hi[axis] = mid;
        lower.count = count(lower.lo, lower.hi);
        let mut upper = lower;
        upper.lo[axis] = mid + 1;
        upper.hi[axis] = bx.hi[axis];
        upper.count = bx.count - lower.count;
        boxes[best] = upper;
        boxes.push(lower);
    }
    // (the goldens with 256 entries that agree have at most 94 boxes empty, those that do not 253)
    if bits > 4 && boxes.iter().filter(|bx| bx.count == 0).count() > boxes.len() / 2 {
        return Err(Fail::Unmodeled(open::WICIMAGE_ENCODE_QUANTIZE));
    }
    boxes.sort_by_key(|bx| std::cmp::Reverse(bx.count));
    let level = |bx: &Box5, k: usize| u8::try_from(((bx.lo[k] + bx.hi[k]) / 2 * 255 * 2 + 31) / 62).map_err(|_| Fail::Bad);
    hit(clause::WICIMAGE_ENCODE_GRID);
    boxes.iter().map(|bx| Ok(u32::from_be_bytes([0xff, level(bx, 0)?, level(bx, 1)?, level(bx, 2)?]))).collect()
}

/// ASSUMED: the BMP encoder writes a BITMAPINFOHEADER file, bottom-up, rows padded to 4 bytes,
/// the palette in full for indexed formats, the resolution in pixels a meter
/// MEASURED (golden wicencode enc-gif-*: every source format, the bytes as Windows): GIF89a, no
/// global color table (flags 0x70), a graphic control extension naming the first transparent
/// palette entry when there is one, one image of a local table of 2^b entries (b at least 2,
/// the colors past the palette's 0), LZW of minimum code size b, a clear code first, sub-blocks
/// of up to 255 bytes
pub fn encode_gif(i: &Img) -> Res<Vec<u8>> {
    let Some(pal) = i.pal.as_ref().filter(|_| i.fmt == "idx8") else { return Err(Fail::Bad) };
    let bits = (2..=8u32).find(|&b| 1usize << b >= pal.len()).ok_or(Fail::Bad)?;
    let (w, h) = (u16::try_from(i.w).map_err(|_| Fail::Bad)?, u16::try_from(i.h).map_err(|_| Fail::Bad)?);
    let mut v = b"GIF89a".to_vec();
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&[0x70, 0, 0]);
    if let Some(t) = pal.iter().position(|&c| c >> 24 == 0) {
        v.extend_from_slice(&[0x21, 0xf9, 4, 1, 0, 0, u8::try_from(t).map_err(|_| Fail::Bad)?, 0]);
    }
    v.push(0x2c);
    v.extend_from_slice(&[0, 0, 0, 0]);
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.push(0x80 | (bits - 1) as u8);
    for k in 0..1usize << bits {
        let c = pal.get(k).copied().unwrap_or(0);
        v.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
    }
    v.push(bits as u8);
    // (MEASURED: golden enc-gif-idx8: indices past the palette keep their low b bits)
    let mask = u8::try_from((1u32 << bits) - 1).map_err(|_| Fail::Bad)?;
    let px: Vec<u8> = (0..i.h as usize).flat_map(|y| i.px[y * i.stride()..y * i.stride() + i.w as usize].iter().map(move |&p| p & mask)).collect();
    let data = lzw_encode(bits, &px);
    for c in data.chunks(255) {
        v.push(c.len() as u8);
        v.extend_from_slice(c);
    }
    v.extend_from_slice(&[0, 0x3b]);
    hit(clause::WICIMAGE_ENCODE_GIF);
    Ok(v)
}

/// GIF's LZW the encoding way: a clear code first, codes one bit wider once the next code needs
/// it, a clear code when the table is full, the end code last
fn lzw_encode(min: u32, px: &[u8]) -> Vec<u8> {
    let clear = 1u32 << min;
    let mut out = Vec::new();
    let (mut acc, mut nacc) = (0u64, 0u32);
    let mut emit = |code: u32, size: u32, out: &mut Vec<u8>| {
        acc |= (code as u64) << nacc;
        nacc += size;
        while nacc >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            nacc -= 8;
        }
    };
    let mut dict: std::collections::HashMap<(u32, u8), u32> = std::collections::HashMap::new();
    let mut size = min + 1;
    let mut next = clear + 2;
    emit(clear, size, &mut out);
    let mut cur: Option<u32> = None;
    for &p in px {
        let Some(c) = cur else {
            cur = Some(p as u32);
            continue;
        };
        if let Some(&n) = dict.get(&(c, p)) {
            cur = Some(n);
            continue;
        }
        emit(c, size, &mut out);
        if next < 4096 {
            dict.insert((c, p), next);
            next += 1;
            if next > 1 << size && size < 12 {
                size += 1;
            }
        } else {
            emit(clear, size, &mut out);
            dict.clear();
            size = min + 1;
            next = clear + 2;
        }
        cur = Some(p as u32);
    }
    if let Some(c) = cur {
        emit(c, size, &mut out);
    }
    emit(clear + 1, size, &mut out);
    if nacc > 0 {
        out.push(acc as u8);
    }
    out
}

/// a 64-bit pixel's first and third 16-bit samples swapped (RGBA as BMP's BGRA and back)
fn swap_red_blue16(p: &mut [u8]) {
    p.swap(0, 4);
    p.swap(1, 5);
}

pub fn encode_bmp(i: &Img) -> Res<Vec<u8>> {
    if !["idx1", "idx4", "idx8", "bgr555", "bgr565", "bgr24", "bgr32", "bgra", "pbgra", "rgba64fixed"].contains(&i.fmt) {
        return Err(Fail::Unmodeled(open::WICIMAGE_CONVERT_OTHER));
    }
    hit(clause::WICIMAGE_ENCODE_BMP);
    let bpp = bits_of(i.fmt);
    let st = (i.w as usize * bpp as usize).div_ceil(32) * 4;
    let mut px = vec![0u8; st * i.h as usize];
    for y in 0..i.h as usize {
        let src = &i.px[y * i.stride()..(y + 1) * i.stride()];
        let d = (i.h as usize - 1 - y) * st;
        px[d..d + src.len()].copy_from_slice(src);
        // (MEASURED golden enc-bmp-rgb48, -rgba64: 64 bits a pixel are written blue first)
        if i.fmt == "rgba64fixed" {
            for p in px[d..d + src.len()].chunks_exact_mut(8) {
                swap_red_blue16(p);
            }
        }
    }
    let pal: Vec<u32> = i.pal.clone().unwrap_or_default();
    let ppm = (i.dpi.0 / 0.0254 + 0.5) as u32;
    let mut f = crate::generators::wicimage_bmp(40, i.w as i32, i.h as i32, bpp, if i.fmt == "bgr565" { 3 } else { 0 }, [0xf800, 0x07e0, 0x001f, 0], pal.len() as u32, &pal, &px, false, (ppm, ppm));
    // (MEASURED: biSizeImage 0, biClrImportant the palette's size, its entries' alpha bytes kept)
    f[34..38].fill(0);
    f[50..54].copy_from_slice(&(pal.len() as u32).to_le_bytes());
    Ok(f)
}
