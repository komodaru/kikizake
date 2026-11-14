//! GDI+ bitmaps and matrices: an executable specification of the flat API's bitmaps (made with
//! GdipCreateBitmapFromScan0 and no buffer), their pixels through LockBits in any pixel format,
//! GetPixel / SetPixel, palettes, a Graphics' Clear and FillRectangleI, RotateFlip,
//! CloneBitmapArea, and Matrix (CreateMatrix2, Multiply, Invert, TransformPoints).
//!
//! Sources (Microsoft Learn): "Image Pixel Format Constants" (each format's channels),
//! "Bitmap::LockBits", "Bitmap::GetPixel", "Bitmap::SetPixel", "RotateFlipType", "Matrix::Matrix",
//! "Matrix::Multiply", "Matrix::Invert", "CompositingMode", "CompositingQuality", "Status".

use crate::spec::clause;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

/// a pixel format (PixelFormat, "Image Pixel Format Constants")
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pf {
    I1,
    I4,
    I8,
    Gray16,
    Rgb555,
    Rgb565,
    Argb1555,
    Rgb24,
    Rgb32,
    Argb32,
    Pargb32,
    Rgb48,
    Argb64,
    Pargb64,
}

/// the formats of 16-bit channels
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wide {
    Rgb48,
    Argb64,
    Pargb64,
}

/// CompositingMode
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    SourceOver,
    SourceCopy,
}

/// CompositingQuality (0 Default .. 4 AssumeLinear)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Quality {
    Default,
    HighSpeed,
    HighQuality,
    GammaCorrected,
    AssumeLinear,
}

impl Quality {
    pub fn from_value(v: i64) -> Option<Quality> {
        [Quality::Default, Quality::HighSpeed, Quality::HighQuality, Quality::GammaCorrected, Quality::AssumeLinear].get(usize::try_from(v).ok()?).copied()
    }
    /// gamma corrected blending (HighQuality, GammaCorrected)
    pub const fn gamma(self) -> bool {
        match self {
            Quality::HighQuality | Quality::GammaCorrected => true,
            Quality::Default | Quality::HighSpeed | Quality::AssumeLinear => false,
        }
    }
}

/// the bits of an index of an indexed format
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth {
    One,
    Four,
    Eight,
}

impl Pf {
    pub const ALL: [Pf; 14] = [Pf::I1, Pf::I4, Pf::I8, Pf::Gray16, Pf::Rgb555, Pf::Rgb565, Pf::Argb1555, Pf::Rgb24, Pf::Rgb32, Pf::Argb32, Pf::Pargb32, Pf::Rgb48, Pf::Argb64, Pf::Pargb64];
    /// its name in the case language
    pub const fn name(self) -> &'static str {
        match self {
            Pf::I1 => "1bppIndexed",
            Pf::I4 => "4bppIndexed",
            Pf::I8 => "8bppIndexed",
            Pf::Gray16 => "16bppGrayScale",
            Pf::Rgb555 => "16bppRGB555",
            Pf::Rgb565 => "16bppRGB565",
            Pf::Argb1555 => "16bppARGB1555",
            Pf::Rgb24 => "24bppRGB",
            Pf::Rgb32 => "32bppRGB",
            Pf::Argb32 => "32bppARGB",
            Pf::Pargb32 => "32bppPARGB",
            Pf::Rgb48 => "48bppRGB",
            Pf::Argb64 => "64bppARGB",
            Pf::Pargb64 => "64bppPARGB",
        }
    }
    /// its PixelFormat value
    pub const fn code(self) -> u32 {
        match self {
            Pf::I1 => 0x0003_0101,
            Pf::I4 => 0x0003_0402,
            Pf::I8 => 0x0003_0803,
            Pf::Gray16 => 0x0010_1004,
            Pf::Rgb555 => 0x0002_1005,
            Pf::Rgb565 => 0x0002_1006,
            Pf::Argb1555 => 0x0006_1007,
            Pf::Rgb24 => 0x0002_1808,
            Pf::Rgb32 => 0x0002_2009,
            Pf::Argb32 => 0x0026_200a,
            Pf::Pargb32 => 0x000e_200b,
            Pf::Rgb48 => 0x0010_300c,
            Pf::Argb64 => 0x0034_400d,
            Pf::Pargb64 => 0x001c_400e,
        }
    }
    pub const fn bits(self) -> u32 {
        self.code() >> 8 & 255
    }
    pub const fn depth(self) -> Option<Depth> {
        match self {
            Pf::I1 => Some(Depth::One),
            Pf::I4 => Some(Depth::Four),
            Pf::I8 => Some(Depth::Eight),
            Pf::Gray16 | Pf::Rgb555 | Pf::Rgb565 | Pf::Argb1555 | Pf::Rgb24 | Pf::Rgb32 | Pf::Argb32 | Pf::Pargb32 | Pf::Rgb48 | Pf::Argb64 | Pf::Pargb64 => None,
        }
    }
    /// a format of 16-bit channels
    pub const fn wide(self) -> Option<Wide> {
        match self {
            Pf::Rgb48 => Some(Wide::Rgb48),
            Pf::Argb64 => Some(Wide::Argb64),
            Pf::Pargb64 => Some(Wide::Pargb64),
            Pf::I1 | Pf::I4 | Pf::I8 | Pf::Gray16 | Pf::Rgb555 | Pf::Rgb565 | Pf::Argb1555 | Pf::Rgb24 | Pf::Rgb32 | Pf::Argb32 | Pf::Pargb32 => None,
        }
    }
    pub fn parse(name: &str) -> Option<Pf> {
        Pf::ALL.into_iter().find(|f| f.name() == name)
    }
}

// (every format's bits are a whole number of bytes or below 8)
const _: () = {
    let mut i = 0;
    while i < Pf::ALL.len() {
        let b = Pf::ALL[i].bits();
        assert!(b == 1 || b == 4 || b % 8 == 0 && b <= 64);
        i += 1;
    }
};

pub const OK: u32 = 0;
pub const INVALID_PARAMETER: u32 = 2;
pub const OUT_OF_MEMORY: u32 = 3;

pub fn bits(f: Pf) -> u32 {
    f.bits()
}
fn gray16(f: Pf) -> bool {
    f == Pf::Gray16
}
pub fn indexed(f: Pf) -> bool {
    f.depth().is_some()
}
pub fn row_bytes(f: Pf, w: u32) -> usize {
    (w * bits(f)).div_ceil(8) as usize
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub f: Pf,
    pub w: u32,
    pub h: u32,
    pub rows: Vec<Vec<u8>>,
    pub pal: Vec<u32>,
    /// PaletteFlags; the palette type in bits 8.. (GP.CLONE.PALETTE)
    pub flags: u32,
}

/// a pixel's value (its bytes little-endian, or below 8 bits its index)
pub fn get_px(row: &[u8], x: u32, bits: u32) -> u64 {
    hit(clause::GP_LAYOUT);
    if bits < 8 {
        let bit = x * bits;
        return (row[(bit / 8) as usize] as u64 >> (8 - bits - bit % 8)) & ((1 << bits) - 1);
    }
    let n = (bits / 8) as usize;
    let o = x as usize * n;
    row[o..o + n].iter().rev().fold(0u64, |a, &b| a << 8 | b as u64)
}
pub fn put_px(row: &mut [u8], x: u32, bits: u32, v: u64) {
    if bits < 8 {
        let bit = x * bits;
        let shift = 8 - bits - bit % 8;
        let m = (((1u32 << bits) - 1) << shift) as u8;
        let b = &mut row[(bit / 8) as usize];
        *b = (*b & !m) | ((v << shift) as u8 & m);
        return;
    }
    let n = (bits / 8) as usize;
    let o = x as usize * n;
    for (i, b) in row[o..o + n].iter_mut().enumerate() {
        *b = (v >> (8 * i)) as u8;
    }
}

/// the default palettes of a new indexed bitmap (MEASURED gdiplus golden: 1 bpp black and white,
/// 4 bpp the 16 VGA colors, 8 bpp the halftone palette)
pub fn default_palette(f: Pf) -> Vec<u32> {
    let vga = [
        0xff00_0000u32, 0xff80_0000, 0xff00_8000, 0xff80_8000, 0xff00_0080, 0xff80_0080, 0xff00_8080, 0xff80_8080, 0xffc0_c0c0, 0xffff_0000, 0xff00_ff00, 0xffff_ff00,
        0xff00_00ff, 0xffff_00ff, 0xff00_ffff, 0xffff_ffff,
    ];
    match f.depth() {
        Some(Depth::One) => vec![0xff00_0000, 0xffff_ffff],
        Some(Depth::Four) => vga.to_vec(),
        Some(Depth::Eight) => {
            let mut p = vga.to_vec();
            p.resize(40, 0);
            for r in 0..6 {
                for g in 0..6 {
                    for b in 0..6 {
                        p.push(0xff00_0000 | (r * 0x33) << 16 | (g * 0x33) << 8 | b * 0x33);
                    }
                }
            }
            p
        }
        None => vec![],
    }
}

/// palette flags GetImagePalette reports (PaletteFlagsGrayScale 2, PaletteFlagsHalftone 4;
/// MEASURED: black and white is gray)
fn palette_flags(f: Pf) -> u32 {
    match f.depth() {
        Some(Depth::One) => 2,
        Some(Depth::Eight) => 4,
        Some(Depth::Four) | None => 0,
    }
}

fn x5(v: u64) -> u32 {
    ((v << 3) | (v >> 2)) as u32 & 255
}
fn x6(v: u64) -> u32 {
    ((v << 2) | (v >> 4)) as u32 & 255
}

/// MEASURED (golden gdiplus pairs-32bppARGB-32bppPARGB: every (color, alpha) pair)
fn premultiply(c: u64, a: u64) -> u64 {
    (c * a + 127) / 255
}

/// MEASURED (golden gdiplus pairs-32bppPARGB-32bppARGB: every pair): a 16.16 reciprocal of the
/// alpha, truncated, the product truncated, at most 255; alpha 0 keeps the color
fn unpremultiply(c: u32, a: u32) -> u32 {
    if a == 0 {
        c
    } else {
        ((c * ((255 << 16) / a)) >> 16).min(255)
    }
}

/// MEASURED (golden gdiplus pairs-32bppARGB-64bppARGB, levels-*): 16-bit channels hold linear
/// light, 0..8192 (GDI+'s sRGB64); 8-bit sRGB to it by the sRGB curve, rounded, and back
fn widen(c: u64) -> u64 {
    let c = c as f64 / 255.0;
    let l = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    (l * 8192.0 + 0.5) as u64
}
/// a 16-bit channel as a signed number, clamped to 0..8192 (MEASURED: words from 0x8000 are 0)
fn level(v: u64) -> u64 {
    (v as u16 as i16).clamp(0, 8192) as u64
}
fn narrow(v: u64) -> u32 {
    let l = level(v) as f64 / 8192.0;
    let s = if l <= 0.003_130_8 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 };
    (s * 255.0 + 0.5) as u32
}
/// alpha: a * 8192 / 255 truncated, and back
fn widen_alpha(a: u64) -> u64 {
    a * 8192 / 255
}
fn narrow_alpha(a: u64) -> u32 {
    (level(a) * 255 / 8192) as u32
}

/// SourceOver without gamma correction, both premultiplied (MEASURED, golden gdiplus
/// over-0..3: every (color, alpha) of the destination for 4 sources): s + d (255 - sa) / 255
/// rounded, alpha included
pub fn over(s: u32, d: u32) -> u32 {
    hit(clause::GP_FILL_OVER);
    let sa = s >> 24;
    let mut o = 0;
    for sh in [0, 8, 16, 24] {
        o |= ((s >> sh & 255) + ((d >> sh & 255) * (255 - sa) + 127) / 255) << sh;
    }
    o
}

/// MEASURED (golden gdiplus-32 gamma-*, gammaalpha-*: every (brush alpha, destination alpha)):
/// x86's d f / 8192 rounded, as 16-bit lanes make it: the product's high word times 8, plus its
/// low word with 4096 added and the carry out of the 16 bits lost, shifted down 13
pub fn lanes(d: u64, f: u64) -> u64 {
    let p = d * f;
    ((p >> 16) << 3) + ((((p & 0xffff) + 4096) & 0xffff) >> 13)
}

/// source over as drawing on 16bppRGB555/565 does it: the destination's part truncated
pub fn over_truncated(s: u32, d: u32) -> u32 {
    hit(clause::GP_FILL_OVER);
    let sa = s >> 24;
    let mut o = 0;
    for sh in [0, 8, 16] {
        o |= ((s >> sh & 255) + (d >> sh & 255) * (255 - sa) / 255) << sh;
    }
    o
}

/// MEASURED (golden gdiplus grid-1bppIndexed, grid-4bppIndexed, grid-8bppIndexed: every 5-bit
/// color): the index each default palette gives a color, by its high 5 bits a channel, packed as
/// the grid cases read it (1, 4, 8 bits an index, red, green, blue 5 bits each from the top).
/// Not the least RGB distance: that holds for 98.8% of the cells (1 bpp), 96.9% (4 bpp), 99.1%
/// (8 bpp), and no weighting, rounding or tie rule tried makes it all.
pub const NEAREST: &[u8] = include_bytes!("data/gdiplus_nearest.bin");

/// the index of a color in the default palette of an indexed format
pub fn nearest(d: Depth, c: u32) -> u32 {
    hit(clause::GP_CONV_TO_INDEXED);
    let k = ((c >> 19 & 31) << 10 | (c >> 11 & 31) << 5 | (c >> 3 & 31)) as usize;
    let (base, n) = match d {
        Depth::One => (0, 1),
        Depth::Four => (4096, 4),
        Depth::Eight => (4096 + 16384, 8),
    };
    get_px(&NEAREST[base..], k as u32, n) as u32
}

/// a pixel value of `f` as straight ARGB
pub fn to_argb(f: Pf, v: u64, pal: &[u32]) -> Res<u32> {
    let c = |r: u32, g: u32, b: u32| 0xff00_0000 | r << 16 | g << 8 | b;
    Ok(match f {
        Pf::I1 | Pf::I4 | Pf::I8 => {
            hit(clause::GP_CONV_INDEXED);
            if pal.is_empty() { default_palette(f)[v as usize] } else { pal[v as usize] }
        }
        // (no call converts a 16bppGrayScale pixel: GP.GRAY16 refuses them all first)
        Pf::Gray16 => return Err(Fail::Bad),
        Pf::Rgb555 => {
            hit(clause::GP_CONV_EXPAND);
            c(x5(v >> 10 & 31), x5(v >> 5 & 31), x5(v & 31))
        }
        Pf::Rgb565 => {
            hit(clause::GP_CONV_EXPAND);
            c(x5(v >> 11 & 31), x6(v >> 5 & 63), x5(v & 31))
        }
        Pf::Argb1555 => {
            hit(clause::GP_CONV_EXPAND);
            (if v & 0x8000 != 0 { 0xff00_0000 } else { 0 }) | (c(x5(v >> 10 & 31), x5(v >> 5 & 31), x5(v & 31)) & 0xff_ffff)
        }
        Pf::Rgb24 | Pf::Rgb32 => {
            hit(clause::GP_CONV_OPAQUE);
            0xff00_0000 | v as u32 & 0xff_ffff
        }
        Pf::Argb32 => v as u32,
        Pf::Pargb32 => {
            hit(clause::GP_CONV_UNPREMULTIPLY);
            let a = (v >> 24) as u32;
            let ch = |s: u32| unpremultiply((v >> s) as u32 & 255, a) << s;
            a << 24 | ch(16) | ch(8) | ch(0)
        }
        Pf::Rgb48 => {
            hit(clause::GP_CONV_NARROW);
            c(narrow(v >> 32), narrow(v >> 16), narrow(v))
        }
        Pf::Argb64 => {
            hit(clause::GP_CONV_NARROW);
            narrow_alpha(v >> 48) << 24 | (c(narrow(v >> 32), narrow(v >> 16), narrow(v)) & 0xff_ffff)
        }
        Pf::Pargb64 => {
            hit(clause::GP_CONV_NARROW_PREMULTIPLIED);
            let a = level(v >> 48) as i32;
            // (MEASURED: c * 8192 / a in 32 bits, kept to 16 bits, then a level)
            let un = |c: u64| narrow(if a == 0 { c } else { (word(c) * 8192 / a) as u16 as u64 });
            narrow_alpha(a as u64) << 24 | (c(un(v >> 32), un(v >> 16), un(v)) & 0xff_ffff)
        }
    })
}

/// straight ARGB as a pixel value of `f`
pub fn from_argb(f: Pf, c: u32) -> Res<u64> {
    let (a, r, g, b) = ((c >> 24) as u64, (c >> 16 & 255) as u64, (c >> 8 & 255) as u64, (c & 255) as u64);
    Ok(match f {
        Pf::I1 => nearest(Depth::One, c) as u64,
        Pf::I4 => nearest(Depth::Four, c) as u64,
        Pf::I8 => nearest(Depth::Eight, c) as u64,
        // (no call converts a 16bppGrayScale pixel: GP.GRAY16 refuses them all first)
        Pf::Gray16 => return Err(Fail::Bad),
        Pf::Rgb555 => {
            hit(clause::GP_CONV_TRUNCATE);
            (r >> 3) << 10 | (g >> 3) << 5 | b >> 3
        }
        Pf::Rgb565 => {
            hit(clause::GP_CONV_TRUNCATE);
            (r >> 3) << 11 | (g >> 2) << 5 | b >> 3
        }
        Pf::Argb1555 => {
            hit(clause::GP_CONV_TRUNCATE);
            (a >> 7) << 15 | (r >> 3) << 10 | (g >> 3) << 5 | b >> 3
        }
        Pf::Rgb24 => {
            hit(clause::GP_CONV_OPAQUE);
            (c & 0xff_ffff) as u64
        }
        Pf::Rgb32 => {
            hit(clause::GP_CONV_KEEP);
            c as u64
        }
        Pf::Argb32 => c as u64,
        Pf::Pargb32 => {
            hit(clause::GP_CONV_PREMULTIPLY);
            a << 24 | premultiply(r, a) << 16 | premultiply(g, a) << 8 | premultiply(b, a)
        }
        Pf::Rgb48 => {
            hit(clause::GP_CONV_WIDEN);
            widen(r) << 32 | widen(g) << 16 | widen(b)
        }
        Pf::Argb64 => {
            hit(clause::GP_CONV_WIDEN);
            widen_alpha(a) << 48 | widen(r) << 32 | widen(g) << 16 | widen(b)
        }
        Pf::Pargb64 => {
            hit(clause::GP_CONV_WIDEN_PREMULTIPLIED);
            let a16 = widen_alpha(a);
            let p = |c: u64| widen(c) * a16 / 8192;
            a16 << 48 | p(r) << 32 | p(g) << 16 | p(b)
        }
    })
}

/// a pixel of `from` as a pixel of `to` (LockBits, CloneBitmapArea)
pub fn convert(from: Pf, to: Pf, v: u64, pal: &[u32]) -> Res<u64> {
    if from == to {
        hit(clause::GP_LOCK_SAME);
        return Ok(v);
    }
    if let (Some(a), Some(b)) = (from.wide(), to.wide()) {
        hit(clause::GP_CONV_WIDE);
        return Ok(from_argb64(b, to_argb64(a, v)));
    }
    let opaque = |f: Pf| matches!(f, Pf::Rgb555 | Pf::Rgb565 | Pf::Rgb24 | Pf::Rgb32);
    if from == Pf::Pargb32 && opaque(to) {
        hit(clause::GP_CONV_PREMULTIPLIED_OPAQUE);
        return from_argb(to, v as u32);
    }
    from_argb(to, to_argb(from, v, pal)?)
}

/// a pixel of a 16-bit channel format as straight 16-bit ARGB, the words as they are
/// (MEASURED, golden gdiplus read-/write- between the 16-bit formats: signed words; a
/// premultiplied one with 0 < alpha < 8192 divided, c * 8192 / a in 32 bits truncated, kept to
/// 16 bits; others as they are)
fn to_argb64(f: Wide, v: u64) -> u64 {
    match f {
        Wide::Rgb48 => 8192 << 48 | v & 0xffff_ffff_ffff,
        Wide::Argb64 => v,
        Wide::Pargb64 => {
            let a = level(v >> 48) as i32;
            let un = |s: u32| (if a == 0 { v >> s & 0xffff } else { (word(v >> s) * 8192 / a) as u16 as u64 }) << s;
            (v & 0xffff << 48) | un(32) | un(16) | un(0)
        }
    }
}
/// and back: premultiplied (c * a) >> 13 in 32 bits, kept to 16 bits, the alpha word as it is
fn from_argb64(f: Wide, v: u64) -> u64 {
    match f {
        Wide::Rgb48 => v & 0xffff_ffff_ffff,
        Wide::Argb64 => v,
        Wide::Pargb64 => {
            let a = word(v >> 48);
            let p = |s: u32| (((word(v >> s) * a) >> 13) as u16 as u64) << s;
            (v & 0xffff << 48) | p(32) | p(16) | p(0)
        }
    }
}

/// the low 16 bits as a signed number
fn word(v: u64) -> i32 {
    v as u16 as i16 as i32
}

impl Bitmap {
    pub fn new(f: Pf, w: u32, h: u32) -> Bitmap {
        hit(clause::GP_CREATE_ZERO);
        Bitmap { f, w, h, rows: vec![vec![0; row_bytes(f, w)]; h as usize], pal: default_palette(f), flags: palette_flags(f) }
    }
    pub fn px(&self, x: u32, y: u32) -> u64 {
        get_px(&self.rows[y as usize], x, bits(self.f))
    }
    pub fn set_px(&mut self, x: u32, y: u32, v: u64) {
        put_px(&mut self.rows[y as usize], x, bits(self.f), v)
    }
}

type Rect = (i64, i64, i64, i64);

/// a locked or cloned rectangle: inside the bitmap and not empty, else InvalidParameter
fn rect_ok(b: &Bitmap, r: Rect) -> bool {
    hit(clause::GP_LOCK_RECT);
    let (x, y, w, h) = r;
    x >= 0 && y >= 0 && w > 0 && h > 0 && x + w <= b.w as i64 && y + h <= b.h as i64
}

/// the bytes a write fills (runner/gdiplus.c fill_bytes)
fn fill_bytes(t: &[String], i: usize, w: u32, h: u32, bits: u32) -> Res<Vec<Vec<u8>>> {
    let rb = (w * bits).div_ceil(8) as usize;
    let kind = t.get(i).map(String::as_str).ok_or(Fail::Bad)?;
    let mut x = match kind {
        "seed" => t.get(i + 1).and_then(|v| num(v)).ok_or(Fail::Bad)? as u32,
        "ramp" => 0,
        "pairs" if bits == 32 => 0,
        "levels" if bits == 48 || bits == 64 => 0,
        "grid" if bits == 24 => 0,
        _ => return Err(Fail::Bad),
    };
    if x == 0 {
        x = 0x9e37_79b9;
    }
    let mut r = crate::rng::XorShift32(x);
    let mut k = 0usize;
    let mut out = Vec::new();
    for y in 0..h as usize {
        let mut row = vec![0u8; rb];
        for (b, v) in row.iter_mut().enumerate() {
            *v = match kind {
                "seed" => r.next() as u8,
                "ramp" => k as u8,
                "pairs" => {
                    let px = b / 4;
                    [px, 255 - px, px * 7, y][b % 4] as u8
                }
                "grid" => {
                    let k = y * w as usize + b / 3;
                    let v = (k >> (5 * (b % 3))) & 31;
                    (v << 3 | v >> 2) as u8
                }
                _ => {
                    let n = (bits / 8) as usize;
                    let pix = y * w as usize + b / n;
                    let word = b % n / 2;
                    let l = if bits == 64 && word == 3 { 8192 } else { pix % 8193 };
                    (if b & 1 == 1 { l >> 8 } else { l }) as u8
                }
            };
            k += 1;
        }
        out.push(row);
    }
    Ok(out)
}

/// the rect after token `at` ("at x y w h"), else the whole bitmap
fn rect_after(t: &[String], b: &Bitmap) -> Res<Rect> {
    match t.iter().position(|s| s == "at") {
        Some(i) => Ok((n(t, i + 1)?, n(t, i + 2)?, n(t, i + 3)?, n(t, i + 4)?)),
        None => Ok((0, 0, b.w as i64, b.h as i64)),
    }
}

fn n(t: &[String], i: usize) -> Res<i64> {
    t.get(i).and_then(|v| num(v)).ok_or(Fail::Bad)
}

fn fmt_at(t: &[String], i: usize) -> Res<Pf> {
    t.get(i).and_then(|v| Pf::parse(v)).ok_or(Fail::Bad)
}

fn hex_row(row: &[u8], w: u32, bits: u32) -> String {
    let mut row = row.to_vec();
    let tail = w * bits % 8;
    if tail != 0 {
        let l = row.len() - 1;
        row[l] &= (0xff00u32 >> tail) as u8;
    }
    row.iter().map(|b| format!("{b:02x}")).collect()
}

/// a matrix: m11 m12 m21 m22 dx dy
pub type Matrix = [f32; 6];

/// the product a × b of two affine matrices (points as row vectors: p' = p × M)
/// MEASURED (golden gdiplus cloneidx-*, clonegrid-*, clone-68): the palette a clone into another
/// indexed format gets, its type (PaletteType) in the flags' bits 8..: 1 bpp FixedBW, 4 bpp
/// FixedHalftone8 (the 8 corners of the cube and 8 more), 8 bpp FixedHalftone216 (the 6x6x6 cube
/// and the same 8 more)
pub fn fixed_palette(d: Depth) -> (Vec<u32>, u32) {
    let more = [0xffc0_c0c0, 0xff80_8080, 0xff80_0000, 0xff00_8000, 0xff00_0080, 0xff80_8000, 0xff80_0080, 0xff00_8080];
    match d {
        Depth::One => (vec![0xff00_0000, 0xffff_ffff], 2 << 8),
        Depth::Four => ((0..8u32).map(|k| 0xff00_0000 | (k >> 2) * 0xff_0000 | (k >> 1 & 1) * 0xff00 | (k & 1) * 0xff).chain(more).collect(), 3 << 8),
        Depth::Eight => ((0..216u32).map(|k| 0xff00_0000 | (k / 36) * 0x33_0000 | (k / 6 % 6) * 0x3300 | (k % 6) * 0x33).chain(more).collect(), 7 << 8),
    }
}

/// how float arithmetic rounds: to nearest (x86), or toward minus infinity (MEASURED: x86-64's
/// GDI+ matrices, golden gdiplus-64 matrix-*: all 3390 numbers of 120 cases)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Round {
    Nearest,
    Down,
}

/// the f32 nearest x from below: the largest f32 at most the exact value c + e (e an f64 rest)
fn down(c: f64, e: f64) -> f32 {
    let f = c as f32;
    if (f as f64) > c || (f as f64) == c && e < 0.0 {
        f.next_down()
    } else {
        f
    }
}

impl Round {
    pub fn mul(self, a: f32, b: f32) -> f32 {
        let p = a as f64 * b as f64;
        if self == Round::Nearest || p == 0.0 || !p.is_finite() {
            return a * b;
        }
        down(p, 0.0)
    }
    pub fn add(self, a: f32, b: f32) -> f32 {
        let s = a as f64 + b as f64;
        if self == Round::Nearest || !s.is_finite() {
            return a + b;
        }
        // (two-sum: the rest of the f64 sum)
        let bb = s - a as f64;
        let e = (a as f64 - (s - bb)) + (b as f64 - bb);
        if s == 0.0 && e == 0.0 {
            // (an exact zero sum is -0 rounding down, but +0 + +0)
            return if a.is_sign_positive() && b.is_sign_positive() { 0.0 } else { -0.0 };
        }
        down(s, e)
    }
    pub fn sub(self, a: f32, b: f32) -> f32 {
        self.add(a, -b)
    }
    pub fn div(self, a: f32, b: f32) -> f32 {
        let q = a / b;
        if self == Round::Nearest || !q.is_finite() || q == 0.0 {
            return q;
        }
        // (q b against a, exactly in f64: above the true quotient, one step down)
        let r = q as f64 * b as f64 - a as f64;
        if r != 0.0 && (r > 0.0) == (b > 0.0) {
            q.next_down()
        } else {
            q
        }
    }
}

pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    mul_in(Round::Nearest, a, b)
}
pub fn mul_in(r: Round, a: &Matrix, b: &Matrix) -> Matrix {
    hit(clause::GP_MATRIX_MUL);
    let dot = |x: f32, y: f32, z: f32, w: f32| r.add(r.mul(x, y), r.mul(z, w));
    [
        dot(a[0], b[0], a[1], b[2]),
        dot(a[0], b[1], a[1], b[3]),
        dot(a[2], b[0], a[3], b[2]),
        dot(a[2], b[1], a[3], b[3]),
        r.add(dot(a[4], b[0], a[5], b[2]), b[4]),
        r.add(dot(a[4], b[1], a[5], b[3]), b[5]),
    ]
}

/// a point through a matrix
pub fn transform(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    transform_in(Round::Nearest, m, x, y)
}
pub fn transform_in(r: Round, m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    hit(clause::GP_MATRIX_POINT);
    (r.add(r.add(r.mul(m[0], x), r.mul(m[2], y)), m[4]), r.add(r.add(r.mul(m[1], x), r.mul(m[3], y)), m[5]))
}

/// the inverse (MEASURED, golden gdiplus-32, all 99 invertible cases, every element: the
/// determinant, its reciprocal and every product in float, the translation (m21 dy - m22 dx) r
/// and (m12 dx - m11 dy) r), or None when the determinant is 0
pub fn invert(m: &Matrix) -> Option<Matrix> {
    invert_in(Round::Nearest, m)
}
pub fn invert_in(r: Round, m: &Matrix) -> Option<Matrix> {
    hit(clause::GP_MATRIX_INVERT);
    let det = r.sub(r.mul(m[0], m[3]), r.mul(m[1], m[2]));
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    let q = r.div(1.0, det);
    Some([
        r.mul(m[3], q),
        r.mul(-m[1], q),
        r.mul(-m[2], q),
        r.mul(m[0], q),
        r.mul(r.sub(r.mul(m[2], m[5]), r.mul(m[3], m[4])), q),
        r.mul(r.sub(r.mul(m[1], m[4]), r.mul(m[0], m[5])), q),
    ])
}

fn elements(m: &Matrix) -> String {
    let mut s = "elements 0".to_string();
    for e in m {
        s += &format!(" {:08x}", e.to_bits());
    }
    s
}

/// MEASURED: drawing on 16bppRGB555/565 dithers each premultiplied channel with a 4x4 ordered
/// matrix on the bitmap's own coordinates: c' = min(max, (c + d) >> (8 - bits)), d from DITHER for
/// 5 bits, DITHER >> 1 for 6 (golden gdiplus dither-*: every gray level at every position)
pub const DITHER: [[u32; 4]; 4] = [[0, 4, 1, 5], [6, 2, 7, 3], [1, 5, 0, 4], [7, 3, 6, 2]];
pub fn dither(f: Pf, p: u32, x: u32, y: u32) -> u64 {
    hit(clause::GP_FILL_DITHER);
    let g6 = f == Pf::Rgb565;
    let d = DITHER[y as usize & 3][x as usize & 3];
    let ch = |c: u32, n: u32| ((c + (d >> (n - 5))) >> (8 - n)).min((1 << n) - 1) as u64;
    let (r, g, b) = (p >> 16 & 255, p >> 8 & 255, p & 255);
    if g6 { ch(r, 5) << 11 | ch(g, 6) << 5 | ch(b, 5) } else { ch(r, 5) << 10 | ch(g, 5) << 5 | ch(b, 5) }
}

/// matrix arithmetic: x86 rounds to nearest; x86-64 toward minus infinity (GP.MATRIX.X64)
fn rounding() -> Round {
    if super::arch() == 32 {
        hit(clause::GP_MATRIX_X86);
        Round::Nearest
    } else {
        hit(clause::GP_MATRIX_X64);
        Round::Down
    }
}

#[derive(Default)]
pub struct Gdiplus {
    pub bmp: [Option<Bitmap>; 2],
    pub mat: [Option<Matrix>; 2],
}

impl Gdiplus {
    fn slot(t: &[String], i: usize) -> Res<usize> {
        let s = n(t, i)?;
        if !(0..2).contains(&s) {
            return Err(Fail::Bad);
        }
        Ok(s as usize)
    }

    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        if !name.starts_with("gp") {
            return Ok(false);
        }
        hit(clause::GP_START);
        let needs_bitmap = matches!(name, "gpwrite" | "gpread" | "gpset" | "gpget" | "gpinfo" | "gpfill" | "gpclear" | "gprotate");
        match name {
            "gpbitmap" => {
                let s = Self::slot(t, 1)?;
                let (w, h, f) = (n(t, 2)?, n(t, 3)?, fmt_at(t, 4)?);
                self.bmp[s] = None;
                hit(clause::GP_CREATE);
                if w <= 0 || h <= 0 {
                    hit(clause::GP_CREATE_SIZE);
                    out.push(format!("bitmap {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if w * h > 65536 {
                    return Err(Fail::Bad);
                }
                out.push("bitmap 0".into());
                self.bmp[s] = Some(Bitmap::new(f, w as u32, h as u32));
            }
            _ if needs_bitmap && self.bmp[Self::slot(t, 1)?].is_none() => {
                // (only gpclone's slot 2 is a source; this is slot 1)
                out.push(format!("{name} nobitmap"));
            }
            "gpwrite" => {
                let s = Self::slot(t, 1)?;
                let f = fmt_at(t, 2)?;
                let b = self.bmp[s].as_mut().unwrap();
                let r = rect_after(t, b)?;
                if !rect_ok(b, r) {
                    out.push(format!("write {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if f != b.f && gray16(f) != gray16(b.f) {
                    hit(clause::GP_GRAY16);
                    fill_bytes(t, 3, r.2 as u32, r.3 as u32, bits(f))?;
                    out.push(format!("write 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                let data = fill_bytes(t, 3, r.2 as u32, r.3 as u32, bits(f))?;
                out.push("write 0 0".into());
                hit(clause::GP_LOCK_WRITE);
                for y in 0..r.3 as u32 {
                    for x in 0..r.2 as u32 {
                        let v = convert(f, b.f, get_px(&data[y as usize], x, bits(f)), &[])?;
                        b.set_px(r.0 as u32 + x, r.1 as u32 + y, v);
                    }
                }
            }
            "gpread" => {
                let s = Self::slot(t, 1)?;
                let f = fmt_at(t, 2)?;
                let b = self.bmp[s].as_ref().unwrap();
                let r = rect_after(t, b)?;
                if !rect_ok(b, r) {
                    out.push(format!("read {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if f != b.f && gray16(f) != gray16(b.f) {
                    hit(clause::GP_GRAY16);
                    out.push(format!("read {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                hit(clause::GP_LOCK_READ);
                out.push("read 0".into());
                out.push(format!("px {} {} {}", r.2, r.3, bits(f)));
                for y in 0..r.3 as u32 {
                    let mut row = vec![0u8; row_bytes(f, r.2 as u32)];
                    for x in 0..r.2 as u32 {
                        let v = convert(b.f, f, b.px(r.0 as u32 + x, r.1 as u32 + y), &b.pal)?;
                        put_px(&mut row, x, bits(f), v);
                    }
                    out.push(hex_row(&row, r.2 as u32, bits(f)));
                }
            }
            "gpget" | "gpset" => {
                let s = Self::slot(t, 1)?;
                let (x, y) = (n(t, 2)?, n(t, 3)?);
                let c = if name == "gpset" { n(t, 4)? as u32 } else { 0 };
                let b = self.bmp[s].as_mut().unwrap();
                let w = &name[2..];
                if gray16(b.f) {
                    hit(clause::GP_GRAY16);
                    out.push(format!("{w} {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                hit(clause::GP_PIXEL_BOUNDS);
                if x < 0 || y < 0 || x >= b.w as i64 || y >= b.h as i64 {
                    out.push(format!("{w} {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if name == "gpget" {
                    hit(clause::GP_PIXEL_GET);
                    out.push(format!("get 0 {:08x}", to_argb(b.f, b.px(x as u32, y as u32), &b.pal)?));
                } else {
                    hit(if indexed(b.f) { clause::GP_PIXEL_SET_INDEXED } else { clause::GP_PIXEL_SET });
                    let v = from_argb(b.f, c)?;
                    b.set_px(x as u32, y as u32, v);
                    out.push("set 0".into());
                }
            }
            "gpinfo" => {
                let b = self.bmp[Self::slot(t, 1)?].as_ref().unwrap();
                hit(clause::GP_INFO);
                if b.pal.is_empty() {
                    hit(clause::GP_INFO_NOPALETTE);
                }
                out.push(format!("info {:08x} {} {} palette 0 {} {}", b.f.code(), b.w, b.h, b.flags, b.pal.len()));
                for ch in b.pal.chunks(16) {
                    out.push(ch.iter().map(|c| format!("{c:08x}")).collect::<Vec<_>>().join(" "));
                }
            }
            "gpfill" | "gpclear" => {
                let b = self.bmp[Self::slot(t, 1)?].as_mut().unwrap();
                let w = &name[2..];
                let (r, c, mode, q) = if name == "gpfill" {
                    let mode = match t.get(7).map(String::as_str) {
                        Some("copy") => Mode::SourceCopy,
                        Some("over") => Mode::SourceOver,
                        _ => return Err(Fail::Bad),
                    };
                    ((n(t, 2)?, n(t, 3)?, n(t, 4)?, n(t, 5)?), n(t, 6)? as u32, mode, n(t, 8)?)
                } else {
                    ((0, 0, b.w as i64, b.h as i64), n(t, 2)? as u32, Mode::SourceCopy, 0)
                };
                let copy = match mode {
                    Mode::SourceCopy => true,
                    Mode::SourceOver => false,
                };
                hit(clause::GP_FILL);
                if matches!(b.f, Pf::Gray16 | Pf::Argb1555) {
                    hit(clause::GP_FILL_NOGRAPHICS);
                    out.push(format!("{w} {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                if indexed(b.f) {
                    hit(clause::GP_FILL_INDEXED);
                }
                let Some(q) = Quality::from_value(q) else {
                    return Err(Fail::Bad);
                };
                if r.2 < 0 || r.3 < 0 {
                    hit(clause::GP_FILL_NEGATIVE);
                }

                hit(clause::GP_FILL_RECT);
                let (x0, y0) = (r.0.max(0), r.1.max(0));
                let (x1, y1) = ((r.0 + r.2).min(b.w as i64), (r.1 + r.3).min(b.h as i64));
                let pargb = Pf::Pargb32;
                let argb = Pf::Argb32;
                // (the brush premultiplied)
                let sp = convert(argb, pargb, c as u64, &[])? as u32;
                let inside = |x: u32, y: u32| (x0..x1).contains(&(x as i64)) && (y0..y1).contains(&(y as i64));
                let blend = |d: u32| {
                    if copy {
                        hit(clause::GP_FILL_COPY);
                        sp
                    } else {
                        over(sp, d)
                    }
                };
                // (gamma corrected, MEASURED: SourceOver of a pixel of a 32-bit format (ARGB, PARGB, RGB)
                // in 64bppPARGB words: x86-64 in float as the executor rounds (GP.FILL.GAMMA.X64),
                // x86 in 16-bit lanes (GP.FILL.GAMMA.X86))
                let gamma = !copy && q.gamma();
                let p64 = Pf::Pargb64;
                let rd = rounding();
                let s64 = convert(pargb, p64, sp as u64, &[])?;
                let word = |v: u64, k: u32| v >> (16 * k) & 0xffff;
                let sa = word(s64, 3);
                let rest = rd.sub(1.0, rd.div(sa as f32, 8192.0));
                let x86 = super::arch() == 32;
                let mix = |s: u64, d: u64| -> u64 {
                    if x86 {
                        hit(clause::GP_FILL_GAMMA_X86);
                        s + lanes(d, 8192 - sa) & 0xffff
                    } else {
                        hit(clause::GP_FILL_GAMMA_X64);
                        let (s, d) = (s as f32, d as f32);
                        rd.add(rd.mul(rd.add(rd.div(s, 8192.0), rd.mul(rd.div(d, 8192.0), rest)), 8192.0), 0.5).floor() as i64 as u64 & 0xffff
                    }
                };
                let gam = |f: Pf, v: u64| -> Res<u64> {
                    let d = convert(f, p64, v, &[])?;
                    let o = (0..4).fold(0, |o, k| o | mix(word(s64, k), word(d, k)) << (16 * k));
                    let mut r = convert(p64, f, o, &[])?;
                    if f == Pf::Rgb32 {
                        // (32bppRGB: the colors as opaque, its alpha byte blended as an alpha)
                        let a = mix(sa, widen_alpha(v >> 24));
                        r = r & 0xff_ffff | (narrow_alpha(a) as u64) << 24;
                    }
                    Ok(r)
                };
                if b.f.wide().is_some() || indexed(b.f) {
                    // (MEASURED: a Graphics of a 16-bit channel or indexed bitmap works on a copy of
                    // all of it, 32bppARGB or 32bppPARGB, written back once it draws: the pixels it
                    // does not draw on make the trip too)
                    hit(clause::GP_FILL_BUFFER);
                    let trip = x0 <= x1 && y0 <= y1 && x0 < b.w as i64 && y0 < b.h as i64;
                    let bf = if indexed(b.f) { pargb } else { argb };
                    for y in 0..b.h {
                        for x in 0..b.w {
                            if !trip && !inside(x, y) {
                                continue;
                            }
                            let mut q = convert(b.f, bf, b.px(x, y), &[])?;
                            if inside(x, y) && gamma && indexed(b.f) {
                                // (gamma corrected into an indexed bitmap: the premultiplied linear
                                // words narrowed as they are, as over black)
                                let d = convert(bf, p64, q, &[])?;
                                let o = (0..3).fold(0xff00_0000u32, |o, k| o | narrow(mix(word(s64, k), word(d, k))) << (8 * k));
                                b.set_px(x, y, from_argb(b.f, o)?);
                                continue;
                            } else if inside(x, y) && gamma {
                                q = gam(bf, q)?;
                            } else if inside(x, y) {
                                let d = convert(bf, pargb, q, &[])? as u32;
                                q = convert(pargb, bf, blend(d) as u64, &[])?;
                            }
                            // (to an indexed bitmap the premultiplied color, as over black)
                            let v = if indexed(b.f) { from_argb(b.f, q as u32)? } else { convert(bf, b.f, q, &[])? };
                            b.set_px(x, y, v);
                        }
                    }
                } else if matches!(b.f, Pf::Rgb555 | Pf::Rgb565) {
                    // (MEASURED, golden gdiplus fill-16bppRGB5*-over-*, blend-*: 477 channels: the
                    // destination widened by a shift alone, the blend truncated, then dithered)
                    let g6 = b.f == Pf::Rgb565;
                    // (a clear brush over draws nothing: 555's unused top bit stays; drawn, it is 0)
                    let (y0, y1) = if !copy && sp >> 24 == 0 { (0, 0) } else { (y0, y1) };
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let (x, y) = (x as u32, y as u32);
                            let v = b.px(x, y) as u32;
                            let d = if g6 { (v >> 11 & 31) << 19 | (v >> 5 & 63) << 10 | (v & 31) << 3 } else { (v >> 10 & 31) << 19 | (v >> 5 & 31) << 11 | (v & 31) << 3 };
                            // (x86-64: the destination widened as LockBits does, the blend rounded)
                            let p = if copy {
                                blend(d)
                            } else if gamma {
                                // (gamma corrected: the destination widened as LockBits does)
                                gam(argb, convert(b.f, argb, v as u64, &[])?)? as u32
                            } else if super::arch() == 64 {
                                over(sp, convert(b.f, pargb, v as u64, &[])? as u32)
                            } else {
                                over_truncated(sp, d)
                            };
                            b.set_px(x, y, dither(b.f, p, x, y));
                        }
                    }
                } else {
                    for y in y0..y1 {
                        for x in x0..x1 {
                            let (x, y) = (x as u32, y as u32);
                            if gamma {
                                let v = gam(b.f, b.px(x, y))?;
                                b.set_px(x, y, v);
                                continue;
                            }
                            // (32bppRGB: its bytes as premultiplied, as they are)
                            let rgb = b.f == Pf::Rgb32;
                            let d = if rgb { b.px(x, y) } else { convert(b.f, pargb, b.px(x, y), &[])? };
                            let p = blend(d as u32) as u64;
                            b.set_px(x, y, if rgb { p } else { convert(pargb, b.f, p, &[])? });
                        }
                    }
                }
                out.push(format!("{w} 0 0"));
            }
            "gprotate" => {
                let b = self.bmp[Self::slot(t, 1)?].as_mut().unwrap();
                let k = n(t, 2)?;
                if !(0..8).contains(&k) {
                    return Err(Fail::Bad);
                }
                hit(clause::GP_ROTATE);
                *b = rotate(b, k as u32);
                out.push(format!("rotate 0 {} {}", b.w, b.h));
            }
            "gpclone" => {
                let (d, s) = (Self::slot(t, 1)?, Self::slot(t, 2)?);
                let r = (n(t, 3)?, n(t, 4)?, n(t, 5)?, n(t, 6)?);
                let f = fmt_at(t, 7)?;
                let Some(b) = self.bmp[s].as_ref() else {
                    out.push("gpclone nobitmap".into());
                    return Ok(true);
                };
                hit(clause::GP_CLONE);
                if !rect_ok(b, r) {
                    hit(clause::GP_CLONE_RECT);
                    out.push(format!("clone {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                if f != b.f && gray16(f) != gray16(b.f) {
                    hit(clause::GP_GRAY16);
                    out.push(format!("clone {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                if f != b.f {
                    hit(clause::GP_CLONE_FORMAT);
                }
                let mut c = Bitmap::new(f, r.2 as u32, r.3 as u32);
                let fixed = f.depth().filter(|_| f != b.f);
                if f == b.f {
                    (c.pal, c.flags) = (b.pal.clone(), b.flags);
                } else if let Some(d) = fixed {
                    hit(clause::GP_CLONE_PALETTE);
                    (c.pal, c.flags) = fixed_palette(d);
                }
                for y in 0..r.3 as u32 {
                    for x in 0..r.2 as u32 {
                        let v = b.px(r.0 as u32 + x, r.1 as u32 + y);
                        let v = match fixed {
                            // (the entry of the color LockBits' default palette gives, GP.CONV.TO.INDEXED;
                            // every default entry it picks is in the fixed palette: tests/laws_gdiplus.rs)
                            Some(d) => {
                                let q = default_palette(f)[nearest(d, to_argb(b.f, v, &b.pal)?) as usize];
                                c.pal.iter().position(|&e| e == q).ok_or(Fail::Bad)? as u64
                            }
                            None => convert(b.f, f, v, &b.pal)?,
                        };
                        c.set_px(x, y, v);
                    }
                }
                out.push("clone 0".into());
                self.bmp[d] = Some(c);
            }
            "gpmatrix" => {
                let s = Self::slot(t, 1)?;
                let mut m = [0f32; 6];
                for (i, e) in m.iter_mut().enumerate() {
                    *e = f32::from_bits(n(t, 2 + i)? as u32);
                }
                hit(clause::GP_MATRIX);
                out.push("matrix 0".into());
                out.push(elements(&m));
                self.mat[s] = Some(m);
            }
            "gpmatmul" | "gpmatinv" | "gpmatpts" if self.mat[Self::slot(t, 1)?].is_none() => out.push(format!("{name} nomatrix")),
            "gpmatmul" => {
                let (s, o) = (Self::slot(t, 1)?, Self::slot(t, 2)?);
                let order = n(t, 3)?;
                let rd = rounding();
                let Some(b) = self.mat[o] else {
                    out.push(format!("{name} nomatrix"));
                    return Ok(true);
                };
                let a = self.mat[s].unwrap();
                // (MatrixOrderPrepend 0: the passed matrix on the left)
                let m = match order {
                    0 => mul_in(rd, &b, &a),
                    1 => mul_in(rd, &a, &b),
                    _ => {
                        hit(clause::GP_MATRIX_ORDER);
                        out.push(format!("matmul {INVALID_PARAMETER}"));
                        out.push(elements(&a));
                        return Ok(true);
                    }
                };
                out.push("matmul 0".into());
                out.push(elements(&m));
                self.mat[s] = Some(m);
            }
            "gpmatinv" => {
                let s = Self::slot(t, 1)?;
                let m = self.mat[s].unwrap();
                let rd = rounding();
                match invert_in(rd, &m) {
                    Some(i) => {
                        out.push("matinv 0 1 0".into());
                        out.push(elements(&i));
                        self.mat[s] = Some(i);
                    }
                    None => {
                        out.push(format!("matinv 0 0 {INVALID_PARAMETER}"));
                        out.push(elements(&m));
                    }
                }
            }
            "gpmatpts" => {
                let s = Self::slot(t, 1)?;
                let m = self.mat[s].unwrap();
                let rd = rounding();
                let k = t.len() - 2;
                if k % 2 != 0 || k < 2 || k > 32 {
                    return Err(Fail::Bad);
                }
                let mut l = "pts 0".to_string();
                for i in 0..k / 2 {
                    let (x, y) = (f32::from_bits(n(t, 2 + 2 * i)? as u32), f32::from_bits(n(t, 3 + 2 * i)? as u32));
                    let (a, b) = transform_in(rd, &m, x, y);
                    l += &format!(" {:08x} {:08x}", a.to_bits(), b.to_bits());
                }
                out.push(l);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// RotateFlipType k: k & 3 quarter turns clockwise, then (k >= 4) a horizontal flip
pub fn rotate(b: &Bitmap, k: u32) -> Bitmap {
    let turns = k & 3;
    let (w, h) = if turns % 2 == 1 { (b.h, b.w) } else { (b.w, b.h) };
    let mut o = Bitmap { f: b.f, w, h, rows: vec![vec![0; row_bytes(b.f, w)]; h as usize], pal: b.pal.clone(), flags: b.flags };
    for y in 0..h {
        for x in 0..w {
            // (the source pixel of destination (x, y) after the turns, the flip after them)
            let (sx, sy) = match turns {
                0 => (x, y),
                1 => (y, b.h - 1 - x),
                2 => (b.w - 1 - x, b.h - 1 - y),
                _ => (b.w - 1 - y, x),
            };
            let dx = if k >= 4 { w - 1 - x } else { x };
            o.set_px(dx, y, b.px(sx, sy));
        }
    }
    o
}
