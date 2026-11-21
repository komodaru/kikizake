//! GDI+ bitmaps and matrices: an executable specification of the flat API's bitmaps (made with
//! GdipCreateBitmapFromScan0 and no buffer), their pixels through LockBits in any pixel format,
//! GetPixel / SetPixel, palettes, a Graphics' Clear and FillRectangleI, RotateFlip,
//! CloneBitmapArea, and Matrix (CreateMatrix2, Multiply, Invert, TransformPoints).
//!
//! Sources (Microsoft Learn): "Image Pixel Format Constants" (each format's channels),
//! "Bitmap::LockBits", "Bitmap::GetPixel", "Bitmap::SetPixel", "RotateFlipType", "Matrix::Matrix",
//! "Matrix::Multiply", "Matrix::Invert", "CompositingMode", "CompositingQuality", "Status".

use crate::spec::{clause, open};
use super::{filled, filled_mut, Fail, Res};
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
    /// a value past the enum, which SetCompositingQuality takes as it is (GP.FILL.QUALITY.OTHER)
    Other,
}

impl Quality {
    pub fn from_value(v: i64) -> Option<Quality> {
        let named = [Quality::Default, Quality::HighSpeed, Quality::HighQuality, Quality::GammaCorrected, Quality::AssumeLinear];
        match usize::try_from(v).ok().and_then(|i| named.get(i)) {
            Some(&q) => Some(q),
            None if v == 5 => {
                hit(clause::GP_FILL_QUALITY_OTHER);
                Some(Quality::Other)
            }
            None => None,
        }
    }
    /// gamma corrected blending (HighQuality, GammaCorrected)
    pub const fn gamma(self) -> bool {
        match self {
            Quality::HighQuality | Quality::GammaCorrected => true,
            Quality::Default | Quality::HighSpeed | Quality::AssumeLinear | Quality::Other => false,
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
        assert!(b == 1 || b == 4 || b.is_multiple_of(8) && b <= 64);
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
                        p.push(0xff00_0000 | (r * 0x33) << 16 | (g * 0x33) << 8 | (b * 0x33));
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
    match (255u32 << 16).checked_div(a) {
        None => c,
        Some(r) => ((c * r) >> 16).min(255),
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
        // (MEASURED, golden gdiplusdraw copy-20: a premultiplied source whose color is past its
        // alpha: x86 saturates the channel at 255, x86-64 adds the channels packed, the carry
        // into the next one)
        let v = (s >> sh & 255) + ((d >> sh & 255) * (255 - sa) + 127) / 255;
        o = if super::arch() == 32 { o | v.min(255) << sh } else { o.wrapping_add(v << sh) };
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
        // (MEASURED, golden gdipluspaint hatch16-*: 4096 pixels: t / 255 as (t + (t >> 8)) >> 8)
        let t = (d >> sh & 255) * (255 - sa);
        o |= ((s >> sh & 255) + ((t + (t >> 8)) >> 8)) << sh;
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
        Depth::Four => ((0..8u32).map(|k| 0xff00_0000 | ((k >> 2) * 0xff_0000) | ((k >> 1 & 1) * 0xff00) | ((k & 1) * 0xff)).chain(more).collect(), 3 << 8),
        Depth::Eight => ((0..216u32).map(|k| 0xff00_0000 | ((k / 36) * 0x33_0000) | ((k / 6 % 6) * 0x3300) | ((k % 6) * 0x33)).chain(more).collect(), 7 << 8),
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

/// a brush color made ready for blending: premultiplied, and as 64bppPARGB words
struct Src {
    sp: u32,
    s64: u64,
    sa: u64,
    rest: f32,
}
impl Src {
    /// of a premultiplied color
    fn new(sp: u32) -> Res<Src> {
        let s64 = convert(Pf::Pargb32, Pf::Pargb64, sp as u64, &[])?;
        let sa = s64 >> 48 & 0xffff;
        let rd = rounding();
        Ok(Src { sp, s64, sa, rest: rd.sub(1.0, rd.div(sa as f32, 8192.0)) })
    }
    /// (gamma corrected, MEASURED: SourceOver of a pixel of a 32-bit format (ARGB, PARGB, RGB) in
    /// 64bppPARGB words: x86-64 in float as the executor rounds (GP.FILL.GAMMA.X64), x86 in
    /// 16-bit lanes (GP.FILL.GAMMA.X86))
    fn mix(&self, s: u64, d: u64) -> u64 {
        if super::arch() == 32 {
            hit(clause::GP_FILL_GAMMA_X86);
            (s + lanes(d, 8192 - self.sa)) & 0xffff
        } else {
            hit(clause::GP_FILL_GAMMA_X64);
            let rd = rounding();
            let (s, d) = (s as f32, d as f32);
            rd.add(rd.mul(rd.add(rd.div(s, 8192.0), rd.mul(rd.div(d, 8192.0), self.rest)), 8192.0), 0.5).floor() as i64 as u64 & 0xffff
        }
    }
    fn word(v: u64, k: u32) -> u64 {
        v >> (16 * k) & 0xffff
    }
    fn gam(&self, f: Pf, v: u64) -> Res<u64> {
        if f == Pf::Rgb32 && self.sa == 0 {
            // (MEASURED, golden gdipluspaint hatchblend-8: a clear source leaves a 32bppRGB pixel
            // as it is, its alpha byte too)
            hit(clause::GP_FILL_GAMMA_CLEAR);
            return Ok(v);
        }
        let p64 = Pf::Pargb64;
        let d = convert(f, p64, v, &[])?;
        let o = (0..4).fold(0, |o, k| o | self.mix(Self::word(self.s64, k), Self::word(d, k)) << (16 * k));
        let mut r = convert(p64, f, o, &[])?;
        if f == Pf::Rgb32 {
            // (32bppRGB: the colors as opaque, its alpha byte blended as an alpha)
            let a = self.mix(self.sa, widen_alpha(v >> 24));
            r = r & 0xff_ffff | (narrow_alpha(a) as u64) << 24;
        }
        Ok(r)
    }
    fn blend(&self, copy: bool, d: u32) -> u32 {
        if copy {
            hit(clause::GP_FILL_COPY);
            self.sp
        } else {
            over(self.sp, d)
        }
    }
}

/// a fill's brush: one color, or a texture of them (a hatch)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Source {
    Solid,
    /// a gradient: on x86 a pixel of it clear still drawn (MEASURED, golden gdiplusbrush
    /// linblend-27: 555's unused top bit cleared under it on x86, kept on x86-64)
    Gradient,
    /// a texture placed at the rendering origin (ox, oy)
    Texture { origin: (i64, i64) },
}

/// a fill of the pixels of a box (clipped to the bitmap; empty when x0 >= x1 or y0 >= y1), each
/// with the premultiplied brush color c(x, y) gives, none for a pixel the shape leaves out
fn fill_area(b: &mut Bitmap, (x0, y0, x1, y1): (i64, i64, i64, i64), c: &dyn Fn(u32, u32) -> Option<u32>, kind: Source, copy: bool, q: Quality) -> Res<()> {
    let gamma = !copy && q.gamma();
    let bw = b.w as i64;
    // (MEASURED, golden gdipluspaint hatchopaque16-*: gamma corrected, the dither's column counts
    // from the start of the pixel's run of opaque, or of translucent, source pixels in its row; a
    // run of one pixel takes column 2)
    let opaque = |x: u32, y: u32| c(x, y).map(|v| v >> 24 == 255);
    let texture = match kind {
        Source::Texture { origin } if gamma => Some(origin),
        Source::Texture { .. } | Source::Solid | Source::Gradient => None,
    };
    // (MEASURED, golden gdipluspaint hatchshift16-*, hatchrun16-*: the dither's row counts from
    // the rendering origin's y)
    let row = |y: u32| -> u32 {
        match texture {
            Some((_, oy)) => (i64::from(y) - oy).rem_euclid(4) as u32,
            None => y,
        }
    };
    let column = |x: u32, y: u32| -> u32 {
        let Some((ox, _)) = texture else { return x };
        hit(clause::GP_PAINT_GAMMA_RUNS);
        let o = opaque(x, y);
        let (mut a0, mut a1) = (x, x + 1);
        while i64::from(a0) > x0.max(0) && opaque(a0 - 1, y) == o {
            a0 -= 1;
        }
        while i64::from(a1) < x1.min(bw) && opaque(a1, y) == o {
            a1 += 1;
        }
        // (x86: column 2 for a run of one pixel, and for the last pixel of a translucent run of
        // odd length)
        let odd_end = (a1 - a0) % 2 == 1 && x == a1 - 1 && (o == Some(false) || a1 - a0 == 1 || i64::from(a1) == x1.min(bw));
        let at = if odd_end && super::arch() == 32 { 2 } else { x - a0 };
        // (MEASURED, golden gdipluspaint hatchshift16-*, hatchrun16-*: every run of the row
        // takes the phase of the fill's left edge from the rendering origin's x)
        let mut left = x0.max(0);
        while left < i64::from(x) && c(left as u32, y).is_none() {
            left += 1;
        }
        (i64::from(at) + left - ox).rem_euclid(4) as u32
    };
    let pargb = Pf::Pargb32;
    let argb = Pf::Argb32;
    let inbox = |x: u32, y: u32| (x0..x1).contains(&(x as i64)) && (y0..y1).contains(&(y as i64));
    let src = |x: u32, y: u32| -> Res<Option<Src>> {
        match c(x, y) {
            Some(c) if inbox(x, y) => Src::new(c).map(Some),
            _ => Ok(None),
        }
    };
    if b.f.wide().is_some() || indexed(b.f) {
        // (MEASURED: a Graphics of a 16-bit channel or indexed bitmap works on a copy of all of
        // it, 32bppARGB or 32bppPARGB, written back once it draws: the pixels it does not draw on
        // make the trip too)
        hit(clause::GP_FILL_BUFFER);
        let trip = x0 <= x1 && y0 <= y1 && x0 < b.w as i64 && y0 < b.h as i64;
        let bf = if indexed(b.f) { pargb } else { argb };
        for y in 0..b.h {
            for x in 0..b.w {
                let s = src(x, y)?;
                if !trip && s.is_none() {
                    continue;
                }
                let mut q = convert(b.f, bf, b.px(x, y), &[])?;
                match s {
                    Some(s) if gamma && indexed(b.f) => {
                        // (gamma corrected into an indexed bitmap: the premultiplied linear words
                        // narrowed as they are, as over black)
                        let d = convert(bf, Pf::Pargb64, q, &[])?;
                        let o = (0..3).fold(0xff00_0000u32, |o, k| o | narrow(s.mix(Src::word(s.s64, k), Src::word(d, k))) << (8 * k));
                        b.set_px(x, y, from_argb(b.f, o)?);
                        continue;
                    }
                    Some(s) if gamma => q = s.gam(bf, q)?,
                    Some(s) => {
                        let d = convert(bf, pargb, q, &[])? as u32;
                        q = convert(pargb, bf, s.blend(copy, d) as u64, &[])?;
                    }
                    None => {}
                }
                // (to an indexed bitmap the premultiplied color, as over black)
                let v = if indexed(b.f) { from_argb(b.f, q as u32)? } else { convert(bf, b.f, q, &[])? };
                b.set_px(x, y, v);
            }
        }
    } else if matches!(b.f, Pf::Rgb555 | Pf::Rgb565) {
        // (MEASURED, golden gdiplus fill-16bppRGB5*-over-*, blend-*: 477 channels: the destination
        // widened by a shift alone, the blend truncated, then dithered)
        let g6 = b.f == Pf::Rgb565;
        for y in y0.max(0)..y1 {
            for x in x0.max(0)..x1 {
                let (x, y) = (x as u32, y as u32);
                let Some(s) = src(x, y)? else { continue };
                // (a clear brush over draws nothing: 555's unused top bit stays; drawn, it is 0)
                if !copy && s.sp >> 24 == 0 && (kind != Source::Gradient || super::arch() == 64) {
                    continue;
                }
                let v = b.px(x, y) as u32;
                let d = if g6 { (v >> 11 & 31) << 19 | (v >> 5 & 63) << 10 | (v & 31) << 3 } else { (v >> 10 & 31) << 19 | (v >> 5 & 31) << 11 | (v & 31) << 3 };
                // (x86-64: the destination widened as LockBits does, the blend rounded)
                let p = if copy {
                    s.blend(copy, d)
                } else if gamma {
                    // (gamma corrected: the destination widened as LockBits does)
                    s.gam(argb, convert(b.f, argb, v as u64, &[])?)? as u32
                } else if super::arch() == 64 {
                    over(s.sp, convert(b.f, pargb, v as u64, &[])? as u32)
                } else {
                    over_truncated(s.sp, d)
                };
                let col = column(x, y);
                b.set_px(x, y, dither(b.f, p, col, row(y)));
            }
        }
    } else {
        for y in y0.max(0)..y1 {
            for x in x0.max(0)..x1 {
                let (x, y) = (x as u32, y as u32);
                let Some(s) = src(x, y)? else { continue };
                if gamma {
                    let v = s.gam(b.f, b.px(x, y))?;
                    b.set_px(x, y, v);
                    continue;
                }
                // (32bppRGB: its bytes as premultiplied, as they are)
                let rgb = b.f == Pf::Rgb32;
                let d = if rgb { b.px(x, y) } else { convert(b.f, pargb, b.px(x, y), &[])? };
                let p = s.blend(copy, d as u32) as u64;
                b.set_px(x, y, if rgb { p } else { convert(pargb, b.f, p, &[])? });
            }
        }
    }
    Ok(())
}

/// a HatchStyle GDI+ makes a brush of (0 ..= 52, HatchStyleMin ..= HatchStyleMax)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct HatchStyle(u8);
impl HatchStyle {
    pub const MAX: u8 = 52;
    pub fn new(v: i64) -> Option<HatchStyle> {
        u8::try_from(v).ok().filter(|&v| v <= Self::MAX).map(HatchStyle)
    }
    pub fn get(self) -> u8 {
        self.0
    }
}

/// WrapMode: how a texture (or gradient) repeats past its first tile
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Wrap {
    Tile,
    TileFlipX,
    TileFlipY,
    TileFlipXY,
    Clamp,
}
impl Wrap {
    pub fn from_value(v: i64) -> Option<Wrap> {
        Some(match v {
            0 => Wrap::Tile,
            1 => Wrap::TileFlipX,
            2 => Wrap::TileFlipY,
            3 => Wrap::TileFlipXY,
            4 => Wrap::Clamp,
            _ => return None,
        })
    }
    /// (flip across, flip down) of every other tile
    fn flips(self) -> (bool, bool) {
        match self {
            Wrap::Tile | Wrap::Clamp => (false, false),
            Wrap::TileFlipX => (true, false),
            Wrap::TileFlipY => (false, true),
            Wrap::TileFlipXY => (true, true),
        }
    }
}

/// a texture brush's texels: the bitmap as it was when the brush was made, premultiplied
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Texels {
    w: u32,
    h: u32,
    px: Vec<u32>,
}
impl Texels {
    /// a w x h tile of premultiplied texels, row by row (None unless there are w h of them)
    pub fn new(w: u32, h: u32, px: Vec<u32>) -> Option<Texels> {
        (w > 0 && h > 0 && px.len() == (w as usize) * (h as usize)).then_some(Texels { w, h, px })
    }
    /// the texel a device pixel takes, the tile anchored at the rendering origin; None past the
    /// one tile of Clamp
    pub fn at(&self, wrap: Wrap, x: i64, y: i64) -> Option<u32> {
        let (w, h) = (i64::from(self.w), i64::from(self.h));
        if wrap == Wrap::Clamp && !((0..w).contains(&x) && (0..h).contains(&y)) {
            return None;
        }
        let (fx, fy) = wrap.flips();
        let fold = |v: i64, n: i64, flip: bool| {
            let m = v.rem_euclid(n);
            if flip && v.div_euclid(n) % 2 != 0 { n - 1 - m } else { m }
        };
        let (tx, ty) = (fold(x, w, fx), fold(y, h, fy));
        Some(self.px[(ty * w + tx) as usize])
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Brush {
    Solid(u32),
    Hatch { style: HatchStyle, fore: u32, back: u32 },
    Texture { texels: Texels, wrap: Wrap },
    Linear(super::gplinear::Linear),
}

/// SmoothingMode as GDI+ 1.0 takes it (AntiAlias8x8 and past it: InvalidParameter)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Smoothing {
    Default,
    HighSpeed,
    HighQuality,
    None,
    AntiAlias,
    AntiAlias8x8,
}
impl Smoothing {
    pub fn from_value(v: i64) -> Option<Smoothing> {
        Some(match v {
            0 => Smoothing::Default,
            1 => Smoothing::HighSpeed,
            2 => Smoothing::HighQuality,
            3 => Smoothing::None,
            4 => Smoothing::AntiAlias,
            5 => Smoothing::AntiAlias8x8,
            _ => return None,
        })
    }
    /// the subsamples per pixel down (8 across); none when aliased
    pub fn rows(self) -> Option<u32> {
        match self {
            Smoothing::Default | Smoothing::HighSpeed | Smoothing::None => None,
            Smoothing::HighQuality | Smoothing::AntiAlias => Some(4),
            Smoothing::AntiAlias8x8 => Some(8),
        }
    }
}

/// PixelOffsetMode
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PixelOffset {
    Default,
    HighSpeed,
    HighQuality,
    None,
    Half,
}
impl PixelOffset {
    pub fn from_value(v: i64) -> Option<PixelOffset> {
        Some(match v {
            0 => PixelOffset::Default,
            1 => PixelOffset::HighSpeed,
            2 => PixelOffset::HighQuality,
            3 => PixelOffset::None,
            4 => PixelOffset::Half,
            _ => return None,
        })
    }
    pub fn half(self) -> bool {
        match self {
            PixelOffset::Default | PixelOffset::HighSpeed | PixelOffset::None => false,
            PixelOffset::HighQuality | PixelOffset::Half => true,
        }
    }
}

/// MEASURED (golden gdipluspaint hatch-*): each style's 8 x 8 pattern, the fore color's weight
/// 0..255 (255 - the gray of black on white), row by row
pub const HATCH: &[u8] = include_bytes!("data/gdiplus_hatch.bin");
const _: () = assert!(HATCH.len() == (HatchStyle::MAX as usize + 1) * 64);

/// an ARGB color premultiplied as 32bppPARGB holds it
fn premultiplied(c: u32) -> u32 {
    (0..3).fold(c & 0xff00_0000, |o, k| o | (premultiply((c >> (8 * k) & 255) as u64, (c >> 24) as u64) as u32) << (8 * k))
}

/// a hatch texel's weight of fore: data/gdiplus_hatch.bin holds only these four
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Weight {
    Back,
    Quarter,
    Most,
    Fore,
}
impl Weight {
    const fn known(v: u8) -> bool {
        matches!(v, 0 | 64 | 234 | 255)
    }
    fn of(v: u32) -> Weight {
        match v {
            0 => Weight::Back,
            64 => Weight::Quarter,
            234 => Weight::Most,
            255 => Weight::Fore,
            _ => unreachable!("a hatch weight past the table's four (the const assert below)"),
        }
    }
}
const _: () = {
    let mut i = 0;
    while i < HATCH.len() {
        assert!(Weight::known(HATCH[i]));
        i += 1;
    }
};

/// (MEASURED, golden gdipluspaint hatchedge16-*, hatchrun16-22, hatchbetween16-*, hatchcarry-*:
/// gamma corrected, the alpha of a texel of weight 234 is b + (f - b) c floored with c a float just
/// under 32 / 35 (the answers put c in [0.914163, 0.914286); 234 / 256 = 0.914063 is out), not
/// f 234 + b 22, >> 8 as without gamma (hatchpartial-*) and as its colors)
fn gamma_alpha_234(f: u32, b: u32) -> u32 {
    hit(clause::GP_PAINT_GAMMA_BETWEEN);
    if f == b {
        return b;
    }
    // (b + floor((32 (f - b) - 1) / 35), in unsigned steps: below f, b - ceil((32 (b - f) + 1) / 35),
    // at most b - f under b, never under 0)
    if f > b {
        b + (32 * (f - b) - 1) / 35
    } else {
        b - (32 * (b - f) / 35 + 1)
    }
}

/// the subsamples at p + k / n (k = 0 .. n) in [a, b)
fn samples(p: i64, a: f64, b: f64, n: u32) -> u32 {
    (0..n).filter(|&k| (a..b).contains(&(p as f64 + k as f64 / n as f64))).count() as u32
}

/// FillRectangle of a brush in float coordinates (MEASURED, golden gdipluspaint rectf-*,
/// rectfblend-*, hatch*-*)
#[allow(clippy::too_many_arguments)]
fn paint_rect(b: &mut Bitmap, brush: &Brush, (x, y, w, h): (f32, f32, f32, f32), copy: bool, q: Quality, sm: Smoothing, po: PixelOffset, origin: (i64, i64)) -> Res<()> {
    if w < 0.0 || h < 0.0 {
        hit(clause::GP_PAINT_NEGATIVE);
        return Ok(());
    }
    paint(b, brush, Area::Rect(x as f64, y as f64, w as f64, h as f64), copy, q, sm, po, origin)
}

/// a fill mode as FillPolygon takes it
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FillMode {
    Alternate,
    Winding,
}
impl FillMode {
    pub fn from_value(v: i64) -> Option<FillMode> {
        match v {
            0 => Some(FillMode::Alternate),
            1 => Some(FillMode::Winding),
            _ => None,
        }
    }
}

/// a polygon of points on the 1/16 grid (the device's 28.4), closed from the last to the first
#[derive(Clone, PartialEq, Debug)]
pub struct Polygon {
    pts: Vec<(f64, f64)>,
}
impl Polygon {
    /// none for a point off the 1/16 grid or past +-2^20 (not measured: GDI+ rounds them somehow)
    pub fn new(pts: &[(f32, f32)]) -> Option<Polygon> {
        let on = |v: f32| v.is_finite() && v.abs() < 1048576.0 && (v * 16.0).fract() == 0.0;
        pts.iter().all(|&(x, y)| on(x) && on(y)).then(|| Polygon { pts: pts.iter().map(|&(x, y)| (x as f64, y as f64)).collect() })
    }
    /// the winding number at (x, y): an edge counts where it crosses the sample's row, the row
    /// taken half open ([top, bottom)), an edge at or left of the sample counted
    pub fn winding(&self, x: f64, y: f64) -> i64 {
        let n = self.pts.len();
        (0..n)
            .map(|i| {
                let ((x0, y0), (x1, y1)) = (self.pts[i], self.pts[(i + 1) % n]);
                // (a horizontal edge crosses no row; one going down winds +1, up -1)
                let (up, (top, bottom)) = match y0.partial_cmp(&y1) {
                    Some(std::cmp::Ordering::Less) => (false, (y0, y1)),
                    Some(std::cmp::Ordering::Greater) => (true, (y1, y0)),
                    Some(std::cmp::Ordering::Equal) | None => return 0,
                };
                if !(top <= y && y < bottom) {
                    return 0;
                }
                // (the crossing x0 + (y - y0) (x1 - x0) / (y1 - y0) <= x, multiplied out: exact,
                // the coordinates on the 1/16 grid and the samples on the 1/64 grid)
                let (l, r) = ((x - x0) * (y1 - y0), (y - y0) * (x1 - x0));
                match (up, if up { r >= l } else { r <= l }) {
                    (false, true) => 1,
                    (true, true) => -1,
                    (_, false) => 0,
                }
            })
            .sum()
    }
    pub fn inside(&self, fm: FillMode, x: f64, y: f64) -> bool {
        let w = self.winding(x, y);
        match fm {
            FillMode::Alternate => w % 2 != 0,
            FillMode::Winding => w != 0,
        }
    }
}

/// what a brush fills
#[derive(Clone, Copy)]
enum Area<'a> {
    Rect(f64, f64, f64, f64),
    /// what the scan converter covered (GP.RASTER.FILL)
    Cover(&'a super::gppath::Coverage),
}

/// a brush filling an area (the rules of FillRectangle hold for a polygon: MEASURED, golden
/// gdiplusline fill-polygon-*)
#[allow(clippy::too_many_arguments)]
fn paint(b: &mut Bitmap, brush: &Brush, area: Area, copy: bool, q: Quality, sm: Smoothing, po: PixelOffset, (ox, oy): (i64, i64)) -> Res<()> {
    let pre = |c: u32| premultiplied(c);
    let kind = match *brush {
        Brush::Solid(_) => Source::Solid,
        Brush::Hatch { .. } => Source::Texture { origin: (ox, oy) },
        // (MEASURED, golden gdiplusbrush textile-*: a texture brush is anchored at the device's
        // (0, 0); the rendering origin moves hatches only)
        Brush::Texture { .. } => Source::Texture { origin: (0, 0) },
        Brush::Linear(_) => Source::Gradient,
    };
    let span = match brush {
        Brush::Linear(l) => super::gplinear::Span::new(rounding(), l, po.half()),
        Brush::Solid(_) | Brush::Hatch { .. } | Brush::Texture { .. } => None,
    };
    if matches!(brush, Brush::Linear(_)) && span.is_none() {
        return Ok(());
    }
    // (the brush's color at a pixel, premultiplied)
    let color = |px: u32, py: u32| -> Option<u32> {
        Some(match *brush {
            Brush::Solid(c) => pre(c),
            Brush::Linear(_) => span.as_ref()?.at(i64::from(px), i64::from(py)),
            Brush::Texture { ref texels, wrap } => {
                hit(clause::GP_BRUSH_TEXTURE_PAINT);
                // (past a Clamp tile the brush is a transparent texel, blended like any other: a
                // 64bpp destination comes back through premultiplication there, MEASURED golden
                // gdiplusbrush texblend-7, -20, -21)
                return Some(texels.at(wrap, i64::from(px), i64::from(py)).unwrap_or(0));
            }
            Brush::Hatch { style, fore, back } => {
                hit(clause::GP_PAINT_HATCH);
                let (hx, hy) = ((px as i64 - ox).rem_euclid(8), (py as i64 - oy).rem_euclid(8));
                let wt = HATCH[style.get() as usize * 64 + hy as usize * 8 + hx as usize] as u32;
                // (premultiplied; between them (antialiased styles) f w + b (256 - w), >> 8;
                // gamma corrected, the alpha of a texel of 234 as gamma_alpha_234 makes it)
                let (fore, back) = (pre(fore), pre(back));
                let gamma = q.gamma() && !copy;
                let lerp = |wt: u32| {
                    (0..4).fold(0, |o, k| {
                        let (f, g) = (fore >> (8 * k) & 255, back >> (8 * k) & 255);
                        let v = if k == 3 && wt == 234 && gamma { gamma_alpha_234(f, g) } else { (f * wt + g * (256 - wt)) >> 8 };
                        o | v << (8 * k)
                    })
                };
                match Weight::of(wt) {
                    Weight::Back => back,
                    Weight::Fore => fore,
                    Weight::Quarter => lerp(64),
                    Weight::Most => lerp(234),
                }
            }
        })
    };
    match (sm.rows(), area) {
        (None, Area::Rect(x, y, w, h)) => {
            hit(clause::GP_PAINT_ALIASED);
            let off = if po.half() { 0.5 } else { 0.0 };
            let edge = |v: f64| (v - off).ceil() as i64;
            let bx = (edge(x).max(0), edge(y).max(0), edge(x + w).min(b.w as i64), edge(y + h).min(b.h as i64));
            fill_area(b, bx, &color, kind, copy, q)
        }
        (None, Area::Cover(c)) => {
            let bx = (0, 0, b.w as i64, b.h as i64);
            fill_area(b, bx, &|px, py| color(px, py).filter(|_| c.count(px, py) > 0), kind, copy, q)
        }
        (Some(n), area) => {
            hit(clause::GP_PAINT_AA);
            let s = if po.half() { 0.0 } else { 0.5 };
            let cov = |px: u32, py: u32| match area {
                Area::Rect(x, y, w, h) => samples(px as i64, x + s, x + w + s, 8) * samples(py as i64, y + s, y + h + s, n),
                Area::Cover(c) => c.count(px, py),
            };
            let full = 8 * n;
            let bx = (0, 0, b.w as i64, b.h as i64);
            fill_area(b, bx, &|px, py| {
                let c = cov(px, py);
                // (each premultiplied channel times the share of subsamples, rounded, halves up)
                let c0 = color(px, py).filter(|_| c > 0)?;
                Some((0..4).fold(0, |o, k| o | ((2 * (c0 >> (8 * k) & 255) * c + full) / (2 * full)) << (8 * k)))
            }, kind, copy, q)
        }
    }
}

/// an Add* of a path, its arguments parsed
type Add = Box<dyn FnOnce(&mut super::gppath::Path) -> Result<(), super::gppath::Refused>>;

#[derive(Default)]
pub struct Gdiplus {
    pub bmp: [Option<Bitmap>; 2],
    pub mat: [Option<Matrix>; 2],
    pub brush: [Option<Brush>; 2],
    pub attr: [Option<Attr>; 2],
    pub path: [Option<super::gppath::Path>; 2],
    pub pen: [Option<super::gppath::Pen>; 2],
}

/// a 5 x 5 ColorMatrix, row by row: [r g b a 1] times it
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorMatrix(pub [f32; 25]);
impl ColorMatrix {
    /// the identity but for its translation row
    fn translation_only(&self) -> bool {
        (0..4).all(|i| (0..4).all(|j| self.0[i * 5 + j] == if i == j { 1.0 } else { 0.0 }))
    }
}

/// ColorMatrixFlags as SetColorMatrix takes them (AltGray only with a gray matrix)
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum MatrixFlags {
    Default,
    SkipGrays,
    AltGray(ColorMatrix),
}
impl MatrixFlags {
    /// (MEASURED, golden gdiplusdraw attr-flags*: past 0 ..= 2, or AltGray without a gray
    /// matrix: InvalidParameter)
    fn new(v: i64, gray: Option<ColorMatrix>) -> Option<MatrixFlags> {
        match (v, gray) {
            (0, _) => Some(MatrixFlags::Default),
            (1, _) => Some(MatrixFlags::SkipGrays),
            (2, Some(g)) => Some(MatrixFlags::AltGray(g)),
            _ => None,
        }
    }
}

/// an ImageAttributes: its color matrix (ColorAdjustTypeDefault), if one was set
#[derive(Clone, PartialEq, Debug)]
pub struct Attr {
    pub matrix: Option<(ColorMatrix, MatrixFlags)>,
}

/// a straight ARGB color through a ColorMatrix (GP.DRAW.MATRIX)
pub fn recolor(c: u32, m: &ColorMatrix, flags: MatrixFlags) -> u32 {
    let ch = |k: u32| (c >> (8 * k) & 255) as f32;
    let v = [ch(2), ch(1), ch(0), ch(3), 255.0];
    let gray = ch(2) == ch(1) && ch(1) == ch(0);
    let (m, v) = match flags {
        MatrixFlags::SkipGrays if gray => {
            hit(clause::GP_DRAW_SKIPGRAYS);
            return c;
        }
        // (MEASURED, golden gdiplusdraw altgray-*, gray-14, gray-29: under a color matrix that
        // only translates, the gray matrix takes a gray pixel as opaque)
        MatrixFlags::AltGray(g) if gray && m.translation_only() => {
            hit(clause::GP_DRAW_ALTGRAY_TRANSLATE);
            (g, [v[0], v[1], v[2], 255.0, 255.0])
        }
        MatrixFlags::AltGray(g) if gray => {
            hit(clause::GP_DRAW_ALTGRAY);
            (g, v)
        }
        MatrixFlags::Default | MatrixFlags::SkipGrays | MatrixFlags::AltGray(_) => (*m, v),
    };
    hit(clause::GP_DRAW_MATRIX);
    let out = |j: usize| -> u32 {
        let s: f32 = (0..5).map(|i| v[i] * m.0[i * 5 + j]).sum();
        (s.clamp(0.0, 255.0) + 0.5) as u32
    };
    out(3) << 24 | out(0) << 16 | out(1) << 8 | out(2)
}

impl Gdiplus {
    fn slot(t: &[String], i: usize) -> Res<usize> {
        let s = n(t, i)?;
        if !(0..2).contains(&s) {
            return Err(Fail::Bad);
        }
        Ok(s as usize)
    }

    /// gpshape with a pen: a straight line (GP.PEN.*)
    fn pen_shape(&mut self, t: &[String], out: &mut Vec<String>) -> Res<bool> {
        use super::gppath::{self as gpp, Stroke};
        let s = Self::slot(t, 1)?;
        let k = Self::slot(t, 3)?;
        let matrix = match t.get(4).map(String::as_str) {
            Some("-") => None,
            _ => Some(Self::slot(t, 4)?),
        };
        let copy = match t.get(5).map(String::as_str) {
            Some("copy") => true,
            Some("over") => false,
            _ => return Err(Fail::Bad),
        };
        let q = Quality::from_value(n(t, 6)?).ok_or(Fail::Bad)?;
        let (sm, po) = (n(t, 7)?, n(t, 8)?);
        let f = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
        let list = |at: usize| -> Res<Vec<(f32, f32)>> {
            let c = n(t, at)?;
            if !(0..=32).contains(&c) {
                return Err(Fail::Bad);
            }
            (0..c as usize).map(|j| Ok((f(at + 1 + 2 * j)?, f(at + 2 + 2 * j)?))).collect()
        };
        let r = rounding();
        let rect = || -> Res<(f32, f32, f32, f32)> { Ok((f(10)?, f(11)?, f(12)?, f(13)?)) };
        // (the shape a path, as the Draw* calls make it: a line on its own way)
        let mut path = gpp::Path::new(0);
        let (line, made) = match t.get(9).map(String::as_str) {
            Some("line") => (Some(((f(10)?, f(11)?), (f(12)?, f(13)?))), Ok(())),
            Some("rect") => (None, path.add_rectangle(r, rect()?)),
            Some("ellipse") => (None, path.add_ellipse(r, rect()?)),
            Some("arc") => (None, path.add_arc(r, rect()?, f(14)?, f(15)?)),
            Some("pie") => (None, path.add_pie(r, rect()?, f(14)?, f(15)?)),
            Some("bezier") => (None, path.add_beziers(r, &[(f(10)?, f(11)?), (f(12)?, f(13)?), (f(14)?, f(15)?), (f(16)?, f(17)?)])),
            // (DrawLines of fewer than two points: InvalidParameter, clean-room notes
            // geometry.md 13, both arches; AddLines takes one)
            Some("lines") => (None, match list(10)?.as_slice() {
                few @ ([] | [_]) => path.add_lines(r, few).and(Err(gpp::Refused::InvalidParameter)),
                more => path.add_lines(r, more),
            }),
            // (DrawPolygon takes two points, AddPolygon does not: one closed figure of them,
            // golden gdiplusline outline-polygon-34)
            Some("polygon") => (None, match list(10)?.as_slice() {
                two @ [_, _] => {
                    path.start_figure();
                    path.add_lines(r, two).map(|()| path.close_figure())
                }
                more => path.add_polygon(r, more),
            }),
            Some("curve") => (None, path.add_curve(r, &list(11)?, f(10)?)),
            Some("closedcurve") => (None, path.add_closed_curve(r, &list(11)?, f(10)?)),
            _ => return Err(Fail::Bad),
        };
        let Some(b) = self.bmp[s].as_mut() else {
            out.push("gpshape nobitmap".into());
            return Ok(true);
        };
        let world = match matrix {
            None => None,
            Some(m) => match self.mat[m] {
                Some(m) => Some(m),
                None => {
                    out.push("shape nomatrix".into());
                    return Ok(true);
                }
            },
        };
        let Some(pen) = self.pen[k].clone() else {
            out.push("shape nopen".into());
            return Ok(true);
        };
        if matches!(b.f, Pf::Gray16 | Pf::Argb1555) {
            hit(clause::GP_FILL_NOGRAPHICS);
            out.push(format!("shape {OUT_OF_MEMORY}"));
            return Ok(true);
        }
        let (Some(sm), Some(po)) = (Smoothing::from_value(sm), PixelOffset::from_value(po)) else {
            hit(clause::GP_PAINT_MODES);
            out.push(format!("shape 0 {INVALID_PARAMETER}"));
            return Ok(true);
        };
        // (a shape its Add* call refuses: InvalidParameter, nothing drawn, golden gdiplusline
        // outline-arc-28, outline-closedcurve-0)
        if made.is_err() {
            hit(clause::GP_PEN_EMPTY);
            out.push(format!("shape 0 {INVALID_PARAMETER}"));
            return Ok(true);
        }
        let dev = gpp::device(r, world.as_ref(), po);
        let wtd = gpp::world_to_device(r, world.as_ref(), po);
        // (DrawLine, DrawLines, DrawPolygon: points whose device box is under NEAR both ways draw
        // nothing, MEASURED on GDI+ itself, both arches: (3, 3) - (3.0001, 3))
        let whole: Vec<(f32, f32)> = match (line, t.get(9).map(String::as_str)) {
            (Some((p0, p1)), _) => vec![p0, p1],
            (None, Some("lines" | "polygon")) => path.pts.iter().map(|q| (q.x, q.y)).collect(),
            (None, _) => vec![],
        };
        if !whole.is_empty() {
            let d: Vec<(f32, f32)> = whole.iter().map(|&(x, y)| gpp::apply(r, &wtd, x, y)).collect();
            let size = |k: fn(&(f32, f32)) -> f32| {
                let (lo, hi) = d.iter().map(k).fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), v| (a.min(v), b.max(v)));
                r.sub(hi, lo)
            };
            let wide = |v: f32| v.partial_cmp(&gpp::NEAR) == Some(std::cmp::Ordering::Greater);
            if !wide(size(|q| q.0)) && !wide(size(|q| q.1)) {
                hit(clause::GP_PEN_EMPTY);
                out.push("shape 0 0".into());
                return Ok(true);
            }
        }
        let how = gpp::stroke(r, &pen, world.as_ref(), sm.rows().is_none(), super::gpwiden::pattern(pen.dash).is_some());
        // (MEASURED, golden gdiplusline pendash-5: DashStyleCustom with no dash array set is
        // OutOfMemory, nothing drawn)
        if pen.dash == 5 {
            hit(clause::GP_PEN_DASH);
            out.push(format!("shape 0 {OUT_OF_MEMORY}"));
            return Ok(true);
        }
        let cov = match (line, how) {
            (Some((p0, p1)), Stroke::OnePixel) => match sm.rows() {
                None => gpp::line_aliased(r, &dev, p0, p1, b.w, b.h)?,
                Some(n) => gpp::line_nominal(r, &dev, p0, p1, Some(n), b.w, b.h),
            },
            (Some((p0, p1)), Stroke::Diamond) => gpp::line_nominal(r, &dev, p0, p1, sm.rows(), b.w, b.h),
            (None, Stroke::OnePixel) if sm.rows().is_none() && !gpp::curved(&path) => {
                hit(clause::GP_PEN_OUTLINE);
                gpp::outline_aliased(r, &path, &dev, b.w, b.h)?
            }
            (None, Stroke::OnePixel | Stroke::Diamond) => {
                hit(clause::GP_PEN_OUTLINE);
                gpp::outline_nominal(r, &path, &dev, sm.rows(), b.w, b.h)?
            }
            (_, Stroke::DashedPixel) => {
                if let Some((p0, p1)) = line {
                    path.add_lines(r, &[p0, p1]).map_err(|_| Fail::Bad)?;
                }
                let dashes = super::gpwiden::dashed(r, &path, &pen)?.ok_or(Fail::Bad)?;
                gpp::outline_dda(r, &dashes, &dev, gpp::Last::Skipped, b.w, b.h)?
            }
            (_, Stroke::Wide) => {
                if let Some((p0, p1)) = line {
                    path.add_lines(r, &[p0, p1]).map_err(|_| Fail::Bad)?;
                }
                match super::gpwiden::widen(r, &path, &pen, &wtd, 0.25)? {
                    Ok(outline) => gpp::fill(r, &outline, None, FillMode::Winding, sm.rows(), PixelOffset::None, b.w, b.h)?,
                    // (DpDriver::StrokePath of no inverse: nothing drawn)
                    Err(super::gpwiden::Widened::NoInverse) => gpp::fill(r, &gpp::Path::new(1), None, FillMode::Winding, sm.rows(), PixelOffset::None, b.w, b.h)?,
                    // (golden penjoin-4, penjoin--1 on x86: OutOfMemory, nothing drawn)
                    Err(super::gpwiden::Widened::Failed) => {
                        out.push(format!("shape 0 {OUT_OF_MEMORY}"));
                        return Ok(true);
                    }
                }
            }
        };
        paint(b, &Brush::Solid(pen.color), Area::Cover(&cov), copy, q, sm, po, (0, 0))?;
        out.push("shape 0 0".into());
        Ok(true)
    }

    /// the path ops (GP.PATH.*)
    fn path_op(&mut self, t: &[String], out: &mut Vec<String>) -> Res<bool> {
        use super::gppath::{Path, Refused};
        let name = t[0].as_str();
        let f = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
        let r = rounding();
        match name {
            "gppath" => {
                let p = Self::slot(t, 1)?;
                self.path[p] = Some(Path::new(n(t, 2)?));
                out.push(format!("path {OK}"));
                return Ok(true);
            }
            "gpfillpath" => {
                let (s, k, p) = (Self::slot(t, 1)?, Self::slot(t, 2)?, Self::slot(t, 3)?);
                let copy = match t.get(4).map(String::as_str) {
                    Some("copy") => true,
                    Some("over") => false,
                    _ => return Err(Fail::Bad),
                };
                let q = Quality::from_value(n(t, 5)?).ok_or(Fail::Bad)?;
                let (sm, po) = (n(t, 6)?, n(t, 7)?);
                let Some(b) = self.bmp[s].as_mut() else {
                    out.push("gpfillpath nobitmap".into());
                    return Ok(true);
                };
                let Some(path) = &self.path[p] else {
                    out.push("gpfillpath nopath".into());
                    return Ok(true);
                };
                let Some(brush) = self.brush[k].clone() else {
                    out.push("fillpath nobrush".into());
                    return Ok(true);
                };
                if matches!(b.f, Pf::Gray16 | Pf::Argb1555) {
                    hit(clause::GP_FILL_NOGRAPHICS);
                    out.push(format!("fillpath {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                let (Some(sm), Some(po)) = (Smoothing::from_value(sm), PixelOffset::from_value(po)) else {
                    hit(clause::GP_PAINT_MODES);
                    out.push(format!("fillpath 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                };
                // (MEASURED, golden gdipluspath pathfillmode-*: a path's FillMode other than
                // Alternate (0) fills Winding, 2, 3 and -1 too)
                hit(clause::GP_PATH_FILLMODE);
                let fm = if path.fill == 0 { FillMode::Alternate } else { FillMode::Winding };
                let cov = super::gppath::fill(r, path, None, fm, sm.rows(), po, b.w, b.h)?;
                paint(b, &brush, Area::Cover(&cov), copy, q, sm, po, (0, 0))?;
                out.push(format!("fillpath 0 {OK}"));
                return Ok(true);
            }
            _ => {}
        }
        let p = Self::slot(t, 1)?;
        // (the arguments parsed before the slot is looked at, as the executor does)
        let parsed: Option<Add> = match name {
            "gpadd" => {
                let what = t.get(2).map(String::as_str).ok_or(Fail::Bad)?;
                let a = |k: usize| f(3 + k);
                let list = |at: usize| -> Res<Vec<(f32, f32)>> {
                    let c = n(t, at)?;
                    if !(0..=64).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    (0..c as usize).map(|j| Ok((f(at + 1 + 2 * j)?, f(at + 2 + 2 * j)?))).collect()
                };
                Some(match what {
                    "line" => {
                        let v = [a(0)?, a(1)?, a(2)?, a(3)?];
                        Box::new(move |q: &mut Path| q.add_lines(r, &[(v[0], v[1]), (v[2], v[3])]))
                    }
                    "rect" => {
                        let v = (a(0)?, a(1)?, a(2)?, a(3)?);
                        Box::new(move |q: &mut Path| q.add_rectangle(r, v))
                    }
                    "ellipse" => {
                        let v = (a(0)?, a(1)?, a(2)?, a(3)?);
                        Box::new(move |q: &mut Path| q.add_ellipse(r, v))
                    }
                    "arc" | "pie" => {
                        let (v, s, w) = ((a(0)?, a(1)?, a(2)?, a(3)?), a(4)?, a(5)?);
                        if what == "arc" { Box::new(move |q: &mut Path| q.add_arc(r, v, s, w)) } else { Box::new(move |q: &mut Path| q.add_pie(r, v, s, w)) }
                    }
                    "bezier" => {
                        let v = [a(0)?, a(1)?, a(2)?, a(3)?, a(4)?, a(5)?, a(6)?, a(7)?];
                        Box::new(move |q: &mut Path| q.add_beziers(r, &[(v[0], v[1]), (v[2], v[3]), (v[4], v[5]), (v[6], v[7])]))
                    }
                    "close" => Box::new(|q: &mut Path| {
                        q.close_figure();
                        Ok(())
                    }),
                    "start" => Box::new(|q: &mut Path| {
                        q.start_figure();
                        Ok(())
                    }),
                    "lines" | "beziers" | "polygon" => {
                        let v = list(3)?;
                        match what {
                            "lines" => Box::new(move |q: &mut Path| q.add_lines(r, &v)),
                            "beziers" => Box::new(move |q: &mut Path| q.add_beziers(r, &v)),
                            _ => Box::new(move |q: &mut Path| q.add_polygon(r, &v)),
                        }
                    }
                    "curve" | "closedcurve" => {
                        let tension = f(3)?;
                        let v = list(4)?;
                        if what == "curve" { Box::new(move |q: &mut Path| q.add_curve(r, &v, tension)) } else { Box::new(move |q: &mut Path| q.add_closed_curve(r, &v, tension)) }
                    }
                    _ => return Err(Fail::Bad),
                })
            }
            _ => None,
        };
        let matrix = |i: usize| -> Res<Option<usize>> {
            match t.get(i).map(String::as_str) {
                Some("-") => Ok(None),
                _ => Self::slot(t, i).map(Some),
            }
        };
        let (flat, widen) = match name {
            "gpflat" => (Some((matrix(2)?, f(3)?)), None),
            "gpwiden" => (None, Some((Self::slot(t, 2)?, matrix(3)?, f(4)?))),
            _ => (None, None),
        };
        let Some(path) = self.path[p].as_mut() else {
            out.push(format!("{name} nopath"));
            return Ok(true);
        };
        if let Some(add) = parsed {
            hit(clause::GP_PATH_BUILD);
            let st = match add(path) {
                Ok(()) => OK,
                Err(Refused::InvalidParameter) => INVALID_PARAMETER,
            };
            out.push(format!("add {st}"));
        } else if let Some((m, flatness)) = flat {
            let m = match m {
                None => None,
                Some(m) => match self.mat[m] {
                    Some(m) => Some(m),
                    None => {
                        out.push("flat nomatrix".into());
                        return Ok(true);
                    }
                },
            };
            path.flatten(r, m.as_ref(), flatness)?;
            out.push(format!("flat {OK}"));
        } else if let Some((k, m, flatness)) = widen {
            let Some(pen) = self.pen[k].clone() else {
                out.push("widen nopen".into());
                return Ok(true);
            };
            let m = match m {
                None => [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                Some(m) => match self.mat[m] {
                    Some(m) => m,
                    None => {
                        out.push("widen nomatrix".into());
                        return Ok(true);
                    }
                },
            };
            match super::gpwiden::widen(r, path, &pen, &m, flatness)? {
                Ok(p) => *path = p,
                // (the path left as it was)
                Err(super::gpwiden::Widened::Failed) => {
                    out.push(format!("widen {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                // (a matrix of no inverse: GdipWidenPath's status is GP.PEN)
                Err(super::gpwiden::Widened::NoInverse) => return Err(Fail::Unmodeled(open::GP_PEN)),
            }
            out.push(format!("widen {OK}"));
        } else {
            // gppathinfo
            out.push(format!("pathinfo {OK} {}", path.pts.len()));
            if !path.pts.is_empty() && path.pts.len() <= 4096 {
                let mut l = format!("pathpts {OK} {OK}");
                for q in &path.pts {
                    l += &format!(" {:02x} {:08x} {:08x}", q.type_byte(), q.x.to_bits(), q.y.to_bits());
                }
                out.push(l);
            }
            let (x, y, w, h) = path.bounds(r);
            out.push(format!("bounds {OK} {:08x} {:08x} {:08x} {:08x}", x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits()));
        }
        Ok(true)
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
                let b = filled_mut(&mut self.bmp, s)?;
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
                let b = filled(&self.bmp, s)?;
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
                let b = filled_mut(&mut self.bmp, s)?;
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
                let b = filled(&self.bmp, Self::slot(t, 1)?)?;
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
                let b = filled_mut(&mut self.bmp, Self::slot(t, 1)?)?;
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
                let sp = convert(Pf::Argb32, Pf::Pargb32, c as u64, &[])? as u32;
                fill_area(b, (x0, y0, x1, y1), &|_, _| Some(sp), Source::Solid, copy, q)?;
                out.push(format!("{w} 0 0"));
            }
            "gprotate" => {
                let b = filled_mut(&mut self.bmp, Self::slot(t, 1)?)?;
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
                let a = *filled(&self.mat, s)?;
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
                let m = *filled(&self.mat, s)?;
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
                let m = *filled(&self.mat, s)?;
                let rd = rounding();
                let k = t.len() - 2;
                if !k.is_multiple_of(2) || !(2..=32).contains(&k) {
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
            "gpbrush" => {
                let s = Self::slot(t, 1)?;
                self.brush[s] = None;
                hit(clause::GP_BRUSH);
                let b = match t.get(2).map(String::as_str) {
                    Some("solid") => Some(Brush::Solid(n(t, 3)? as u32)),
                    Some("hatch") => {
                        let (fore, back) = (n(t, 4)? as u32, n(t, 5)? as u32);
                        HatchStyle::new(n(t, 3)?).map(|style| Brush::Hatch { style, fore, back })
                    }
                    Some("linear") => {
                        use super::gplinear::Refused;
                        let v = (3..=9).map(|i| n(t, i)).collect::<Res<Vec<i64>>>()?;
                        let pt = |a: i64, b: i64| -> Res<(i32, i32)> { Ok((i32::try_from(a).map_err(|_| Fail::Bad)?, i32::try_from(b).map_err(|_| Fail::Bad)?)) };
                        let st = match super::gplinear::new(rounding(), pt(v[0], v[1])?, pt(v[2], v[3])?, (v[4] as u32, v[5] as u32), v[6]) {
                            Ok(l) => {
                                self.brush[s] = Some(Brush::Linear(l));
                                OK
                            }
                            Err(Refused::InvalidParameter) => INVALID_PARAMETER,
                            Err(Refused::OutOfMemory) => OUT_OF_MEMORY,
                        };
                        out.push(format!("brush {st}"));
                        return Ok(true);
                    }
                    Some("texture") => {
                        let src = Self::slot(t, 3)?;
                        let wrap = n(t, 4)?;
                        let Some(img) = &self.bmp[src] else {
                            out.push("gpbrush nobitmap".into());
                            return Ok(true);
                        };
                        // (MEASURED, golden gdiplusbrush texstatus-*, texfmt-16bppGrayScale: a
                        // WrapMode past 0 ..= 4, or a 16bppGrayScale bitmap: OutOfMemory)
                        match (Wrap::from_value(wrap), img.f) {
                            (Some(wrap), f) if f != Pf::Gray16 => {
                                hit(clause::GP_BRUSH_TEXTURE);
                                let px = (0..img.h).flat_map(|y| (0..img.w).map(move |x| (x, y))).map(|(x, y)| convert(img.f, Pf::Pargb32, img.px(x, y), &img.pal).map(|v| v as u32)).collect::<Res<Vec<u32>>>()?;
                                out.push("brush 0".into());
                                self.brush[s] = Some(Brush::Texture { texels: Texels::new(img.w, img.h, px).ok_or(Fail::Bad)?, wrap });
                            }
                            _ => {
                                hit(clause::GP_BRUSH_TEXTURE_STATUS);
                                out.push(format!("brush {OUT_OF_MEMORY}"));
                            }
                        }
                        return Ok(true);
                    }
                    _ => return Err(Fail::Bad),
                };
                match b {
                    Some(b) => {
                        out.push("brush 0".into());
                        self.brush[s] = Some(b);
                    }
                    None => {
                        hit(clause::GP_BRUSH_HATCH_RANGE);
                        out.push(format!("brush {INVALID_PARAMETER}"));
                    }
                }
            }
            "gpbrushinfo" => {
                let b = Self::slot(t, 1)?;
                // (MEASURED, golden gdiplusbrush texinfo: GetBrushType's BrushType, a texture's
                // transform the identity it was made with)
                let Some(brush) = &self.brush[b] else {
                    out.push("brushinfo nobrush".into());
                    return Ok(true);
                };
                hit(clause::GP_BRUSH_INFO);
                out.push(match brush {
                    Brush::Solid(_) => format!("brushinfo {OK} 0"),
                    Brush::Hatch { .. } => format!("brushinfo {OK} 1"),
                    Brush::Texture { .. } => {
                        let id = [1.0f32, 0.0, 0.0, 1.0, 0.0, 0.0].map(|e| format!(" {:08x}", e.to_bits())).concat();
                        format!("brushinfo {OK} 2 {OK}{id}")
                    }
                    Brush::Linear(l) => {
                        let (x, y, w, h) = l.rect;
                        let m = l.matrix.map(|e| format!(" {:08x}", e.to_bits())).concat();
                        format!("brushinfo {OK} 4 {OK} {:08x} {:08x} {:08x} {:08x} {OK}{m}", x.to_bits(), y.to_bits(), w.to_bits(), h.to_bits())
                    }
                });
            }
            "gppen" => {
                // (MEASURED, golden gdiplusline pen*-*: every setter takes any value, Ok)
                let p = Self::slot(t, 1)?;
                let fl = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
                let mut pen = super::gppath::Pen::new(n(t, 2)? as u32, fl(3)?);
                hit(clause::GP_PEN_MAKE);
                let mut l = format!("pen {OK}");
                let mut i = 4;
                while i < t.len() {
                    i += match t[i].as_str() {
                        "dash" => {
                            pen.dash = n(t, i + 1)?;
                            2
                        }
                        "cap" => {
                            (pen.start_cap, pen.end_cap, pen.dash_cap) = (n(t, i + 1)?, n(t, i + 2)?, n(t, i + 3)?);
                            4
                        }
                        "join" => {
                            pen.join = n(t, i + 1)?;
                            2
                        }
                        "align" => {
                            pen.align = n(t, i + 1)?;
                            2
                        }
                        "miter" => {
                            pen.miter = fl(i + 1)?;
                            2
                        }
                        _ => return Err(Fail::Bad),
                    };
                    l += &format!(" {OK}");
                }
                self.pen[p] = Some(pen);
                out.push(l);
            }
            "gppath" | "gpadd" | "gpflat" | "gpwiden" | "gppathinfo" | "gpfillpath" => return self.path_op(t, out),
            "gpshape" => {
                let s = Self::slot(t, 1)?;
                let pen = match t.get(2).map(String::as_str) {
                    Some("pen") => true,
                    Some("brush") => false,
                    _ => return Err(Fail::Bad),
                };
                if pen {
                    return self.pen_shape(t, out);
                }
                let k = Self::slot(t, 3)?;
                let matrix = match t.get(4).map(String::as_str) {
                    Some("-") => None,
                    _ => Some(Self::slot(t, 4)?),
                };
                let copy = match t.get(5).map(String::as_str) {
                    Some("copy") => true,
                    Some("over") => false,
                    _ => return Err(Fail::Bad),
                };
                let q = Quality::from_value(n(t, 6)?).ok_or(Fail::Bad)?;
                let (sm, po) = (n(t, 7)?, n(t, 8)?);
                let f = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
                let list = |at: usize| -> Res<Vec<(f32, f32)>> {
                    let c = n(t, at)?;
                    if !(0..=32).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    (0..c as usize).map(|j| Ok((f(at + 1 + 2 * j)?, f(at + 2 + 2 * j)?))).collect()
                };
                let r = rounding();
                // (every shape a path filled: GP.RASTER.FILL)
                let mut path = super::gppath::Path::new(0);
                let (fm, made) = match t.get(9).map(String::as_str) {
                    Some("ellipse") => (0, path.add_ellipse(r, (f(10)?, f(11)?, f(12)?, f(13)?))),
                    Some("pie") => (0, path.add_pie(r, (f(10)?, f(11)?, f(12)?, f(13)?), f(14)?, f(15)?)),
                    Some("polygon") => {
                        let fm = n(t, 10)?;
                        (fm, path.add_polygon(r, &list(11)?))
                    }
                    Some("closedcurve") => {
                        let tension = f(10)?;
                        let fm = n(t, 11)?;
                        (fm, path.add_closed_curve(r, &list(12)?, tension))
                    }
                    _ => return Err(Fail::Bad),
                };
                let Some(b) = self.bmp[s].as_mut() else {
                    out.push("gpshape nobitmap".into());
                    return Ok(true);
                };
                let world = match matrix {
                    None => None,
                    Some(m) => match self.mat[m] {
                        Some(m) => Some(m),
                        None => {
                            out.push("shape nomatrix".into());
                            return Ok(true);
                        }
                    },
                };
                let Some(brush) = self.brush[k].clone() else {
                    out.push("shape nobrush".into());
                    return Ok(true);
                };
                if matches!(b.f, Pf::Gray16 | Pf::Argb1555) {
                    hit(clause::GP_FILL_NOGRAPHICS);
                    out.push(format!("shape {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                let (Some(sm), Some(po)) = (Smoothing::from_value(sm), PixelOffset::from_value(po)) else {
                    hit(clause::GP_PAINT_MODES);
                    out.push(format!("shape 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                };
                // (MEASURED, golden gdiplusline fillprobe-poly-*: a FillMode past 0 ..= 1 is
                // InvalidParameter, nothing filled)
                let Some(fm) = FillMode::from_value(fm) else {
                    hit(clause::GP_SHAPE_POLYGON);
                    out.push(format!("shape 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                };
                if made.is_err() {
                    out.push(format!("shape 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                let cov = super::gppath::fill(r, &path, world.as_ref(), fm, sm.rows(), po, b.w, b.h)?;
                paint(b, &brush, Area::Cover(&cov), copy, q, sm, po, (0, 0))?;
                out.push("shape 0 0".into());
            }
            "gppaint" => {
                let s = Self::slot(t, 1)?;
                let k = Self::slot(t, 2)?;
                let Some(b) = self.bmp[s].as_mut() else {
                    out.push("gppaint nobitmap".into());
                    return Ok(true);
                };
                let Some(brush) = self.brush[k].clone() else {
                    hit(clause::GP_PAINT_NOBRUSH);
                    out.push("paint nobrush".into());
                    return Ok(true);
                };
                let f = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
                let (x, y, w, h) = (f(3)?, f(4)?, f(5)?, f(6)?);
                let copy = match t.get(7).map(String::as_str) {
                    Some("copy") => true,
                    Some("over") => false,
                    _ => return Err(Fail::Bad),
                };
                let q = Quality::from_value(n(t, 8)?).ok_or(Fail::Bad)?;
                let (sm, po) = (n(t, 9)?, n(t, 10)?);
                let origin = (n(t, 11)?, n(t, 12)?);
                hit(clause::GP_PAINT);
                if matches!(b.f, Pf::Gray16 | Pf::Argb1555) {
                    hit(clause::GP_FILL_NOGRAPHICS);
                    out.push(format!("paint {OUT_OF_MEMORY}"));
                    return Ok(true);
                }
                let (Some(sm), Some(po)) = (Smoothing::from_value(sm), PixelOffset::from_value(po)) else {
                    hit(clause::GP_PAINT_MODES);
                    out.push(format!("paint 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                };
                paint_rect(b, &brush, (x, y, w, h), copy, q, sm, po, origin)?;
                out.push("paint 0 0".into());
            }
            "gpattr" => {
                let a = Self::slot(t, 1)?;
                self.attr[a] = None;
                let f = |i: usize| -> Res<f32> { t.get(i).and_then(|v| v.parse::<f32>().ok()).ok_or(Fail::Bad) };
                let m = |at: usize| -> Res<ColorMatrix> {
                    let mut m = [0f32; 25];
                    for (k, v) in m.iter_mut().enumerate() {
                        *v = f(at + k)?;
                    }
                    Ok(ColorMatrix(m))
                };
                hit(clause::GP_ATTR);
                let st = match t.get(2).map(String::as_str) {
                    Some("none") => {
                        self.attr[a] = Some(Attr { matrix: None });
                        0
                    }
                    Some("matrix") => {
                        let main = m(4)?;
                        let gray = if t.get(29).map(String::as_str) == Some("gray") { Some(m(30)?) } else { None };
                        let flags = MatrixFlags::new(n(t, 3)?, gray);
                        self.attr[a] = Some(Attr { matrix: flags.map(|flags| (main, flags)) });
                        if flags.is_some() {
                            0
                        } else {
                            hit(clause::GP_ATTR_FLAGS);
                            INVALID_PARAMETER
                        }
                    }
                    _ => return Err(Fail::Bad),
                };
                out.push(format!("attr 0 {st}"));
            }
            "gpdraw" => {
                let (d, s) = (Self::slot(t, 1)?, Self::slot(t, 2)?);
                let attr = match t.get(3).map(String::as_str) {
                    Some("-") => None,
                    _ => Some(Self::slot(t, 3)?),
                };
                let v: Vec<i64> = (4..12).map(|i| n(t, i)).collect::<Res<_>>()?;
                let copy = match t.get(12).map(String::as_str) {
                    Some("copy") => true,
                    Some("over") => false,
                    _ => return Err(Fail::Bad),
                };
                let q = Quality::from_value(n(t, 13)?).ok_or(Fail::Bad)?;
                let (interp, po) = (n(t, 14)?, n(t, 15)?);
                if self.bmp[d].is_none() || self.bmp[s].is_none() {
                    out.push("gpdraw nobitmap".into());
                    return Ok(true);
                }
                let attr = match attr {
                    None => None,
                    Some(a) => match &self.attr[a] {
                        Some(a) => Some(a.clone()),
                        None => {
                            hit(clause::GP_DRAW_NOATTR);
                            out.push("draw noattr".into());
                            return Ok(true);
                        }
                    },
                };
                let one = v[2] == v[6] && v[3] == v[7];
                // (GdipSetInterpolationMode takes 0 ..= 7, GdipSetPixelOffsetMode 0 ..= 4, any other
                // value InvalidParameter; the runner then draws nothing, clean-room notes image.md
                // 11, golden drawmode-*)
                let (Some(interp), Some(po)) = (Interp::new(interp), PixelOffset::from_value(po)) else {
                    hit(clause::GP_PAINT_MODES);
                    out.push(format!("draw 0 {INVALID_PARAMETER}"));
                    return Ok(true);
                };
                // (a bitmap onto itself and flipped rectangles not asked)
                if d == s || v[2] <= 0 || v[3] <= 0 || v[6] <= 0 || v[7] <= 0 {
                    return Err(Fail::Unmodeled(open::GP_DRAW_SCALED));
                }
                hit(clause::GP_DRAW);
                let src = filled(&self.bmp, s)?.clone();
                let b = filled_mut(&mut self.bmp, d)?;
                let (dx, dy, w, h, sx, sy, sw, sh) = (v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7]);
                let matrix = attr.as_ref().and_then(|a| a.matrix);
                // a source pixel through the attributes, premultiplied; None past the bitmap
                let texel = |px: i64, py: i64| -> Res<Option<u32>> {
                    if px < 0 || py < 0 || px >= i64::from(src.w) || py >= i64::from(src.h) {
                        return Ok(None);
                    }
                    let p = src.px(px as u32, py as u32);
                    Ok(Some(match matrix {
                        None => convert(src.f, Pf::Pargb32, p, &[])? as u32,
                        Some((m, flags)) => {
                            let argb = convert(src.f, Pf::Argb32, p, &[])? as u32;
                            convert(Pf::Argb32, Pf::Pargb32, u64::from(recolor(argb, &m, flags)), &[])? as u32
                        }
                    }))
                };
                let mut colors = std::collections::HashMap::new();
                let high = match interp {
                    Interp::HighQualityBilinear => Some(super::gpstretch::Kernel::Tent),
                    Interp::HighQualityBicubic => Some(super::gpstretch::Kernel::Cubic),
                    Interp::NearestNeighbor | Interp::Bilinear | Interp::Bicubic => None,
                };
                if let Some(k) = high.filter(|_| !one) {
                    let draw = super::gpstretch::Draw { dst: (dx, dy, w, h), src: (sx, sy, sw, sh), half: po.half() };
                    let px = super::gpstretch::stretch(rounding(), k, &draw, (i64::from(src.w), i64::from(src.h)), (i64::from(b.w), i64::from(b.h)), super::arch() == 32, texel)?;
                    let (mut x0, mut y0, mut x1, mut y1) = (i64::MAX, i64::MAX, i64::MIN, i64::MIN);
                    for (x, y, c) in px {
                        (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1));
                        colors.insert((x as u32, y as u32), c);
                    }
                    if x0 < x1 {
                        fill_area(b, (x0, y0, x1, y1), &|x, y| colors.get(&(x, y)).copied(), Source::Solid, copy, q)?;
                    }
                    out.push("draw 0 0".into());
                    return Ok(true);
                }
                let mut colors = std::collections::HashMap::new();
                for y in dy.max(0)..(dy + h).min(b.h as i64) {
                    for x in dx.max(0)..(dx + w).min(b.w as i64) {
                        let c = match interp {
                            Interp::NearestNeighbor => {
                                if !one {
                                    hit(clause::GP_DRAW_NEAREST);
                                }
                                let (kx, ky) = (nearest_x(x - dx, sw, w, po), nearest_y(y - dy, sh, h, po));
                                // (a corner offset rounds the last rows and columns past the source
                                // rectangle: such rows are left as they are, such columns read the
                                // source pixel past the rectangle)
                                if ky >= sh {
                                    continue;
                                }
                                match texel(sx + kx, sy + ky)? {
                                    Some(c) => c,
                                    None => continue,
                                }
                            }
                            Interp::Bilinear => {
                                hit(clause::GP_DRAW_BILINEAR);
                                let (tx, ty) = (Tap::bilinear(x, dx, w, sx, sw, po), Tap::bilinear(y, dy, h, sy, sh, po));
                                let mut q4 = [[0u32; 2]; 2];
                                for (j, row) in q4.iter_mut().enumerate() {
                                    for (i, t) in row.iter_mut().enumerate() {
                                        *t = texel(tx.at + i as i64, ty.at + j as i64)?.unwrap_or(0);
                                    }
                                }
                                bilinear(q4, tx.w, ty.w)
                            }
                            Interp::Bicubic => {
                                hit(clause::GP_DRAW_BICUBIC);
                                let (tx, ty) = (Cubic::across(x, dx, w, sx, sw, po), Cubic::down(y, dy, h, sy, sh, po));
                                let mut q16 = [[0u32; 4]; 4];
                                for (j, row) in q16.iter_mut().enumerate() {
                                    for (i, t) in row.iter_mut().enumerate() {
                                        *t = texel(tx.at - 1 + i as i64, ty.at - 1 + j as i64)?.unwrap_or(0);
                                    }
                                }
                                bicubic(q16, tx.entry, ty.entry)
                            }
                            // (high quality 1:1 on both axes: the copy span)
                            Interp::HighQualityBilinear | Interp::HighQualityBicubic => match texel(x - dx + sx, y - dy + sy)? {
                                Some(c) => c,
                                None => continue,
                            },
                        };
                        colors.insert((x as u32, y as u32), c);
                    }
                }
                fill_area(b, (dx, dy, dx + w, dy + h), &|x, y| colors.get(&(x, y)).copied(), Source::Solid, copy, q)?;
                out.push("draw 0 0".into());
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// InterpolationMode: Default, LowQuality and Bilinear (0, 1, 3) draw alike, HighQuality as
/// HighQualityBicubic (2, 7) (golden gdiplusdraw filter*-*)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Interp {
    NearestNeighbor,
    Bilinear,
    Bicubic,
    HighQualityBilinear,
    HighQualityBicubic,
}

impl Interp {
    pub fn new(v: i64) -> Option<Interp> {
        match v {
            0 | 1 | 3 => Some(Interp::Bilinear),
            2 | 7 => Some(Interp::HighQualityBicubic),
            4 => Some(Interp::Bicubic),
            5 => Some(Interp::NearestNeighbor),
            6 => Some(Interp::HighQualityBilinear),
            _ => None,
        }
    }
}

/// a bilinear tap on one axis: the source pixel at and the weight of the one after it, out of
/// Tap::one() (x86 256, x86-64 2048)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tap {
    pub at: i64,
    pub w: u32,
}

impl Tap {
    pub fn one() -> u32 {
        if super::arch() == 32 { 256 } else { 2048 }
    }
    /// destination pixel p of a draw from d0 (length dn) of source s0 (length sn), in 16.16 fixed
    /// point (GP.DRAW.BILINEAR): x86 steps from destination pixel 0 (the origin there rounded
    /// once, the step s / d rounded), x86-64 rounds each pixel's place; pixel centers at +1/2
    /// for a half pixel offset
    pub fn bilinear(p: i64, d0: i64, dn: i64, s0: i64, sn: i64, po: PixelOffset) -> Tap {
        // places times 2 dn 65536, so that each is an integer
        let den = 2 * dn;
        let half = if po.half() { sn * 65536 - dn * 65536 } else { 0 };
        let u = if super::arch() == 32 {
            let step = (2 * sn * 65536 + dn).div_euclid(den);
            let origin = (2 * dn * s0 * 65536 - 2 * d0 * sn * 65536 + half + dn).div_euclid(den);
            origin + p * step
        } else {
            (2 * (p - d0) * sn * 65536 + 2 * dn * s0 * 65536 + half + dn).div_euclid(den)
        };
        let f = u & 0xffff;
        if super::arch() == 32 {
            Tap { at: u >> 16, w: (f >> 8) as u32 }
        } else {
            // (11 bits, rounded: a weight of 2048 takes the next pixel whole)
            let w = (f + 16) >> 5;
            Tap { at: (u >> 16) + (w >> 11), w: (w & 2047) as u32 }
        }
    }
}

/// a bicubic tap: the source pixel at (the taps at - 1 ..= at + 2) and the entry of the weights'
/// table, the place's fraction in 1/64 truncated (GP.DRAW.BICUBIC)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cubic {
    pub at: i64,
    pub entry: Entry,
}

/// an entry of the bicubic weights' table, 0 ..= 63 by construction
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Entry(u8);

impl Entry {
    pub fn of(fraction16: i64) -> Entry {
        Entry(((fraction16 & 0xffff) >> 10) as u8)
    }
    pub fn get(self) -> usize {
        usize::from(self.0)
    }
}

impl Cubic {
    /// across: a 16.16 step (s / d rounded) from the rectangle's first column, the first place
    /// s / 2d rounded less one half for a half pixel offset
    pub fn across(p: i64, d0: i64, dn: i64, s0: i64, sn: i64, po: PixelOffset) -> Cubic {
        let step = (2 * sn * 65536 + dn).div_euclid(2 * dn);
        let start = if po.half() { (sn * 65536 + dn).div_euclid(2 * dn) - 32768 } else { 0 };
        let u = s0 * 65536 + start + (p - d0) * step;
        Cubic { at: u >> 16, entry: Entry::of(u) }
    }
    /// down: each row's place on its own, rounded to 16.16
    pub fn down(p: i64, d0: i64, dn: i64, s0: i64, sn: i64, po: PixelOffset) -> Cubic {
        let half = if po.half() { sn * 65536 - dn * 65536 } else { 0 };
        let u = (2 * (p - d0) * sn * 65536 + 2 * dn * s0 * 65536 + half + dn).div_euclid(2 * dn);
        Cubic { at: u >> 16, entry: Entry::of(u) }
    }
}

/// x / 8 rounded, halves away from zero
const fn round8(x: i64) -> i64 {
    if x >= 0 { (x + 4) / 8 } else { -((-x + 4) / 8) }
}
/// 65536 times Keys' cubic (a = -1/2) at m / 64 for m in 0 ..= 64: (3 m^3 - 320 m^2 + 2^19) / 8
const fn inner16(m: i64) -> i64 {
    round8(3 * m * m * m - 320 * m * m + 524_288)
}
/// and at n / 64 for n in 64 ..= 128: (-n^3 + 320 n^2 - 32768 n + 2^20) / 8
const fn outer16(n: i64) -> i64 {
    round8(-n * n * n + 320 * n * n - 32768 * n + 1_048_576)
}

/// MEASURED (golden gdiplusdraw cubicq-*, filterx-*: every entry pinned on both arches): the
/// weights of taps at - 1 ..= at + 2 for each entry q, Keys' cubic (a = -1/2) at 1 + q/64, q/64,
/// 1 - q/64, 2 - q/64 in 1/65536 rounded, halves away from zero
pub const CUBIC16: [[i64; 4]; 64] = {
    let mut t = [[0; 4]; 64];
    let mut q = 0;
    while q < 64 {
        let m = q as i64;
        t[q] = [outer16(64 + m), inner16(m), inner16(64 - m), outer16(128 - m)];
        q += 1;
    }
    t
};
const _: () = assert!(CUBIC16[0][1] == 65536 && CUBIC16[0][0] == 0 && CUBIC16[0][2] == 0 && CUBIC16[32][1] == CUBIC16[32][2]);

/// x86's table: CUBIC16 / 4 rounded, halves away from zero (Q14)
pub const CUBIC14: [[i64; 4]; 64] = {
    let mut t = [[0; 4]; 64];
    let mut q = 0;
    while q < 64 {
        let mut j = 0;
        while j < 4 {
            let w = CUBIC16[q][j];
            t[q][j] = if w >= 0 { (w + 2) / 4 } else { -((-w + 2) / 4) };
            j += 1;
        }
        q += 1;
    }
    t
};

/// four premultiplied texel values weighted by one entry, clamped to a byte: x86 each product
/// in Q14 shifted down 11 (floored), the sum rounded from 1/8; x86-64 in Q16, the sum floored
fn cubic_line(v: [u32; 4], e: Entry) -> u32 {
    let s: i64 = if super::arch() == 32 {
        let w = CUBIC14[e.get()];
        let t: i64 = (0..4).map(|j| (i64::from(v[j]) * w[j]) >> 11).sum();
        (t + 4) >> 3
    } else {
        let w = CUBIC16[e.get()];
        (0..4).map(|j| i64::from(v[j]) * w[j]).sum::<i64>() >> 16
    };
    s.clamp(0, 255) as u32
}

/// sixteen premultiplied texels (rows top down) through the bicubic weights (GP.DRAW.BICUBIC):
/// down each column first, each clamped to a byte, then across
pub fn bicubic(q: [[u32; 4]; 4], ex: Entry, ey: Entry) -> u32 {
    // (each pass cuts a premultiplied channel past its alpha to it, golden gdiplusdraw
    // filterx-25, filter-67, filteralpha-*)
    let cut = |o: u32| {
        let a = o >> 24;
        (0..3).fold(o & 0xff00_0000, |c, k| c | (o >> (8 * k) & 255).min(a) << (8 * k))
    };
    let pass = |v: [u32; 4], e: Entry| cut([0, 8, 16, 24].iter().fold(0, |o, &sh| o | cubic_line(v.map(|p| p >> sh & 255), e) << sh));
    let mid: [u32; 4] = std::array::from_fn(|i| pass(std::array::from_fn(|j| q[j][i]), ey));
    pass(mid, ex)
}

/// four premultiplied texels [[top left, top right], [bottom left, bottom right]] weighted
/// (GP.DRAW.BILINEAR): x86 across then down, each rounded to 8 bits; x86-64 the products of the
/// weights at once, rounded
pub fn bilinear(q: [[u32; 2]; 2], wx: u32, wy: u32) -> u32 {
    let one = Tap::one();
    let mut o = 0;
    for sh in [0, 8, 16, 24] {
        let c = |i: usize, j: usize| q[j][i] >> sh & 255;
        let v = if super::arch() == 32 {
            let top = (c(0, 0) * (one - wx) + c(1, 0) * wx + 128) >> 8;
            let bottom = (c(0, 1) * (one - wx) + c(1, 1) * wx + 128) >> 8;
            (top * (one - wy) + bottom * wy + 128) >> 8
        } else {
            let s = u64::from(c(0, 0)) * u64::from((one - wx) * (one - wy))
                + u64::from(c(1, 0)) * u64::from(wx * (one - wy))
                + u64::from(c(0, 1)) * u64::from((one - wx) * wy)
                + u64::from(c(1, 1)) * u64::from(wx * wy);
            ((s + (1 << 21)) >> 22) as u32
        };
        o |= v << sh;
    }
    o
}

/// the source column of destination column k of a NearestNeighbor draw (GP.DRAW.NEAREST):
/// a corner offset rounds k*s/d half up; a half offset steps in 16.16 fixed point from half a step
pub fn nearest_x(k: i64, s: i64, d: i64, po: PixelOffset) -> i64 {
    if po.half() {
        let step = (2 * s * 65536 + d).div_euclid(2 * d);
        let start = (s * 65536 + d).div_euclid(2 * d);
        (start + k * step) >> 16
    } else {
        (2 * k * s + d).div_euclid(2 * d)
    }
}

/// the source row of destination row k: a half offset floors (k + 1/2)*s/d
pub fn nearest_y(k: i64, s: i64, d: i64, po: PixelOffset) -> i64 {
    if po.half() { ((2 * k + 1) * s).div_euclid(2 * d) } else { (2 * k * s + d).div_euclid(2 * d) }
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

#[cfg(test)]
mod gamma_tests {
    /// the unsigned steps of gamma_alpha_234 are b + floor((32 (f - b) - 1) / 35), for every pair
    #[test]
    fn gamma_alpha_234_is_the_floor() {
        for f in 0..=255u32 {
            for b in 0..=255u32 {
                let d = i64::from(f) - i64::from(b);
                let want = if f == b { i64::from(b) } else { i64::from(b) + (32 * d - 1).div_euclid(35) };
                assert_eq!(i64::from(super::gamma_alpha_234(f, b)), want, "f {f} b {b}");
            }
        }
    }
}
