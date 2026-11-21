//! WIC pixel format conversion: an executable specification of IWICFormatConverter between the
//! native pixel formats (Initialize without dithering, CopyPixels).
//!
//! Sources (Microsoft Learn): "Native Pixel Formats Overview" (each format's channels, their
//! order and size, little-endian 16-bit channels, sub-byte pixels from the high bits),
//! "IWICFormatConverter::Initialize". The arithmetic is not documented: MEASURED from Windows'
//! answers (masu's tests/winref/golden/wicconv-32: low-bit channels widened by repeating their
//! bits, 16-bit channels to 8 by their high byte, premultiplied alpha undone by a 16.16 reciprocal and
//! truncated, 16bppGray levels to 8 bits by a measured table, nearest opaque palette entries).

use crate::spec::clause;
use std::sync::OnceLock;
use super::{Fail, Res};
use crate::spec::hit;

/// (case-language name, bits a pixel)
pub const FORMATS: &[(&str, u32)] = &[
    ("bw", 1), ("gray2", 2), ("gray4", 4), ("gray8", 8), ("gray16", 16), ("bgr555", 16), ("bgr565", 16), ("bgr24", 24),
    ("rgb24", 24), ("bgr32", 32), ("bgra", 32), ("pbgra", 32), ("rgb32", 32), ("rgba", 32), ("prgba", 32), ("rgb48", 48),
    ("rgba64", 64), ("cmyk", 32), ("alpha8", 8), ("idx1", 1), ("idx2", 2), ("idx4", 4), ("idx8", 8),
];

pub fn format(name: &str) -> Option<usize> {
    FORMATS.iter().position(|f| f.0 == name)
}

/// The index of the format `name` of the table, None for a name it does not have: what
/// [`wic_fmt!`](crate::wic_fmt) evaluates when it is built.
pub const fn known(name: &str) -> Option<usize> {
    let mut rest = FORMATS;
    let mut i = 0usize;
    while let [(f, _), more @ ..] = rest {
        if same(f.as_bytes(), name.as_bytes()) {
            return Some(i);
        }
        rest = more;
        i = i.wrapping_add(1);
    }
    None
}

/// byte strings alike (for a const fn, where `==` on slices is not)
const fn same(mut a: &[u8], mut b: &[u8]) -> bool {
    loop {
        match (a, b) {
            ([x, ra @ ..], [y, rb @ ..]) => {
                if *x != *y {
                    return false;
                }
                (a, b) = (ra, rb);
            }
            ([], []) => return true,
            ([], [_, ..]) | ([_, ..], []) => return false,
        }
    }
}

/// the index of a WIC format of the table by a name written in the source, checked when it is built
#[macro_export]
macro_rules! wic_fmt {
    ($name:literal) => {{
        const INDEX: usize = match $crate::model::wic::known($name) {
            Some(i) => i,
            None => panic!("the name is not a format of the table"),
        };
        INDEX
    }};
}

pub fn bits(f: usize) -> u32 {
    FORMATS[f].1
}

pub fn indexed(f: usize) -> bool {
    FORMATS[f].0.starts_with("idx")
}

/// formats with 16-bit channels
fn wide(f: usize) -> bool {
    matches!(FORMATS[f].0, "gray16" | "rgb48" | "rgba64")
}

pub fn stride(f: usize, w: u32) -> usize {
    ((w * bits(f)).div_ceil(8)) as usize
}

/// the executor's palette (ARGB): 16 colors, entry 5 with alpha 0x40
pub fn palette() -> Vec<u32> {
    (0..16u32).map(|i| (if i == 5 { 0x4000_0000 } else { 0xff00_0000 }) | (i * 17) << 16 | (255 - i * 13) << 8 | ((i * 71) & 255)).collect()
}

/// FOUND clean-room (windowscodecs CHalftone::BuildNearestEntryTable, FindNearestColor and the
/// NearestColor op; confirmed by golden wic pair-*-idx*: every pixel): with no dithering a
/// pixel's index is a 16x16x16 table's entry at its colors' top 4 bits; each cell holds the
/// palette entry (the first 2^bits of it) nearest, by the sum of squares over B, G, R and A,
/// the color of levels 17 times the cell's (alpha 255), the first of equals; the pixel's own
/// alpha is not looked at
fn nearest_entry(bits: u32, [b, g, r]: [u8; 3]) -> u8 {
    static T: OnceLock<Vec<[u8; 4096]>> = OnceLock::new();
    let t = T.get_or_init(|| {
        let pal = palette();
        [1u32, 2, 4, 8]
            .iter()
            .map(|&bits| {
                let n = pal.len().min(1 << bits);
                std::array::from_fn(|cell| {
                    let c = [cell & 15, (cell >> 4) & 15, cell >> 8].map(|v| (v * 17) as i32);
                    let d = |e: u32| -> i32 {
                        let k = |s: u32| ((e >> s) & 0xff) as i32;
                        (c[0] - k(0)).pow(2) + (c[1] - k(8)).pow(2) + (c[2] - k(16)).pow(2) + (255 - k(24)).pow(2)
                    };
                    let mut best = (i32::MAX, 0usize);
                    for (i, &e) in pal.iter().take(n).enumerate() {
                        if d(e) < best.0 {
                            best = (d(e), i);
                        }
                    }
                    best.1 as u8
                })
            })
            .collect()
    });
    let k = match bits {
        1 => 0,
        2 => 1,
        4 => 2,
        _ => 3,
    };
    t[k][(usize::from(r >> 4) << 8) | usize::from(g & 0xf0) | usize::from(b >> 4)]
}

fn put_bits_or_byte(row: &mut [u8], x: usize, bits: u32, v: u32) {
    if bits == 8 {
        row[x] = v as u8;
    } else {
        put_bits(row, x as u32, bits, v);
    }
}

fn get_bits(row: &[u8], x: u32, bits: u32) -> u32 {
    let bit = x * bits;
    (row[(bit / 8) as usize] as u32 >> (8 - bits - bit % 8)) & ((1 << bits) - 1)
}

fn put_bits(row: &mut [u8], x: u32, bits: u32, v: u32) {
    let bit = x * bits;
    let shift = 8 - bits - bit % 8;
    let m = (((1u32 << bits) - 1) << shift) as u8;
    let b = &mut row[(bit / 8) as usize];
    *b = (*b & !m) | ((v << shift) as u8 & m);
}

fn srgb_to_linear(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

fn linear_to_srgb(l: f64) -> f64 {
    if l <= 0.003_130_8 { 12.92 * l } else { 1.055 * l.powf(1.0 / 2.4) - 0.055 }
}

/// MEASURED (golden wicencode encbw-*: 378 single colors, every one as this, 15 of them a level
/// off from rounding the luminance to 8 bits directly): the 8-bit gray of a color is its Rec. 709
/// luminance of the linear light back to sRGB rounded to 16 bits, then 16bppGray's measured
/// 8-bit levels
pub fn gray8(b: u8, g: u8, r: u8) -> u8 {
    let v16 = luminance(b, g, r, 65535.0).floor();
    u8::try_from(GRAY16_TO_8.partition_point(|&t| f64::from(t) <= v16)).unwrap_or(u8::MAX)
}

/// Rec. 709 luminance of the linear light, back to sRGB, at `max` levels (ASSUMED)
fn luminance(b: u8, g: u8, r: u8, max: f64) -> f64 {
    let y = 0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b);
    (linear_to_srgb(y) * max + 0.5).clamp(0.0, max)
}

/// MEASURED: the first 16bppGray level of each 8-bit gray 1..=255 (wicconv golden, every level)
const GRAY16_TO_8: [u16; 255] = [
    127, 379, 632, 884, 1137, 1389, 1641, 1894, 2146, 2399, 2649, 2892, 3122, 3551, 3752, 3946, 4315, 4490, 4827, 4988, 5298, 5595, 5737, 6015, 6281, 6538, 6786, 7026, 7373, 7596, 7812, 8127, 8330,
    8626, 8913, 9099, 9371, 9635, 9893, 10143, 10387, 10704, 10935, 11161, 11456, 11671, 11952, 12226, 12493, 12690, 12947, 13261, 13506, 13746, 13982, 14270, 14497, 14774, 15046, 15312, 15573,
    15829, 16081, 16328, 16571, 16810, 17091, 17368, 17594, 17862, 18124, 18383, 18637, 18887, 19133, 19416, 19655, 19929, 20160, 20426, 20688, 20946, 21201, 21452, 21734, 21977, 22218, 22489, 22755,
    22986, 23246, 23503, 23787, 24037, 24283, 24557, 24797, 25064, 25328, 25559, 25816, 26098, 26349, 26597, 26869, 27111, 27377, 27640, 27874, 28131, 28410, 28661, 28909, 29179, 29422, 29687, 29948,
    30207, 30462, 30716, 30966, 31237, 31482, 31747, 31988, 32248, 32505, 32759, 33032, 33282, 33529, 33795, 34057, 34317, 34575, 34830, 35083, 35334, 35601, 35847, 36110, 36370, 36627, 36883, 37136,
    37387, 37654, 37901, 38163, 38422, 38680, 38935, 39189, 39456, 39705, 39969, 40214, 40473, 40730, 40985, 41254, 41505, 41769, 42016, 42276, 42534, 42791, 43045, 43312, 43562, 43825, 44072, 44331,
    44588, 44844, 45097, 45363, 45612, 45874, 46134, 46392, 46648, 46902, 47154, 47418, 47680, 47927, 48185, 48442, 48697, 48962, 49214, 49476, 49724, 49982, 50239, 50506, 50760, 51011, 51273, 51534,
    51781, 52037, 52304, 52558, 52810, 53072, 53332, 53590, 53847, 54102, 54356, 54608, 54870, 55129, 55388, 55644, 55899, 56153, 56416, 56666, 56926, 57183, 57440, 57695, 57958, 58210, 58471, 58729,
    58977, 59243, 59497, 59751, 60012, 60262, 60520, 60777, 61042, 61297, 61549, 61810, 62069, 62327, 62583, 62838, 63092, 63353, 63604, 63862, 64119, 64375, 64638, 64891, 65152, 65411,
];

/// MEASURED (golden wic, 62296 of the 65280 pairs with alpha > 0, every one as this): a 16.16
/// reciprocal of the alpha, truncated, then the product truncated
fn unpremultiply(c: u8, a: u8) -> u8 {
    if a == 0 {
        0
    } else {
        ((c as u32 * ((255 << 16) / a as u32)) >> 16).min(255) as u8
    }
}

fn premultiply(c: u8, a: u8) -> u8 {
    ((c as u32 * a as u32 + 127) / 255) as u8
}

/// one pixel of `f` (its bytes, or below 8 bits its value) as straight BGRA
fn to_bgra(f: usize, s: &[u8], v: u32, pal: &[u32]) -> Res<[u8; 4]> {
    let le16 = |i: usize| u16::from_le_bytes([s[i], s[i + 1]]) as u32;
    let gray = |g: u8| [g, g, g, 255];
    let name = FORMATS[f].0;
    let id = match name {
        "bw" | "gray2" | "gray4" | "bgr555" | "bgr565" => clause::WIC_TO_EXPAND,
        "gray16" => clause::WIC_TO_GRAY16,
        "rgb48" | "rgba64" => clause::WIC_TO_HIGHBYTE,
        "pbgra" | "prgba" => clause::WIC_TO_UNPREMULTIPLY,
        "alpha8" => clause::WIC_TO_ALPHA,
        "cmyk" => clause::WIC_TO_CMYK,
        n if n.starts_with("idx") => clause::WIC_TO_INDEXED,
        _ => clause::WIC_LAYOUT,
    };
    hit(id);
    let x5 = |v: u32| ((v << 3) | (v >> 2)) as u8;
    let x6 = |v: u32| ((v << 2) | (v >> 4)) as u8;
    Ok(match name {
        "bw" | "gray2" | "gray4" => {
            let b = bits(f);
            gray((v * 255 / ((1 << b) - 1)) as u8)
        }
        "gray8" => gray(s[0]),
        "gray16" => gray(GRAY16_TO_8.partition_point(|&t| t as u32 <= le16(0)) as u8),
        "bgr555" => {
            let p = le16(0);
            [x5(p & 31), x5(p >> 5 & 31), x5(p >> 10 & 31), 255]
        }
        "bgr565" => {
            let p = le16(0);
            [x5(p & 31), x6(p >> 5 & 63), x5(p >> 11), 255]
        }
        "bgr24" | "bgr32" => [s[0], s[1], s[2], 255],
        "rgb24" | "rgb32" => [s[2], s[1], s[0], 255],
        "bgra" => [s[0], s[1], s[2], s[3]],
        "rgba" => [s[2], s[1], s[0], s[3]],
        "pbgra" => [unpremultiply(s[0], s[3]), unpremultiply(s[1], s[3]), unpremultiply(s[2], s[3]), s[3]],
        "prgba" => [unpremultiply(s[2], s[3]), unpremultiply(s[1], s[3]), unpremultiply(s[0], s[3]), s[3]],
        "rgb48" => [s[5], s[3], s[1], 255],
        "rgba64" => [s[5], s[3], s[1], s[7]],
        "alpha8" => [255, 255, 255, s[0]],
        "cmyk" => {
            let [b, g, r] = super::cmyk::to_rgb().ok_or(Fail::Bad)?.bgr8([s[0], s[1], s[2], s[3]]);
            [b, g, r, 255]
        }
        _ => {
            // (MEASURED: an index past the palette's colors is opaque black)
            let c = pal.get(v as usize).copied().unwrap_or_else(|| {
                hit(clause::WIC_TO_INDEX_OUTSIDE);
                0xff00_0000
            });
            c.to_le_bytes()
        }
    })
}

/// one straight BGRA pixel written as `f` at pixel x of `row`
fn from_bgra(f: usize, [b, g, r, a]: [u8; 4], row: &mut [u8], x: usize, gray_src: bool) -> Res<()> {
    let name = FORMATS[f].0;
    hit(match name {
        "bw" | "gray2" | "gray4" | "gray8" => clause::WIC_FROM_GRAY,
        "gray16" => clause::WIC_FROM_GRAY16,
        "bgr555" | "bgr565" => clause::WIC_FROM_TRUNCATE,
        "rgb48" | "rgba64" => clause::WIC_FROM_WIDEN,
        "pbgra" | "prgba" => clause::WIC_FROM_PREMULTIPLY,
        "bgr32" | "rgb32" => clause::WIC_FROM_X,
        "cmyk" => clause::WIC_FROM_CMYK,
        n if n.starts_with("idx") => clause::WIC_FROM_INDEXED,
        _ => clause::WIC_LAYOUT,
    });
    let n = (bits(f) / 8) as usize;
    let o = x * n;
    let w16 = |row: &mut [u8], o: usize, v: u16| row[o..o + 2].copy_from_slice(&v.to_le_bytes());
    let gray = || gray8(b, g, r);
    if matches!(name, "bw" | "gray2" | "gray4" | "gray8") && !gray_src {
        hit(clause::WIC_GRAY_ROUND);
    }
    match name {
        "bw" => put_bits(row, x as u32, 1, (gray() >= 128) as u32),
        // MEASURED: from a source that is not 8 bits or less of gray, 2bppGray is 4bppGray's
        // level rounded to 2 bits (thresholds 48, 128, 208 of the 8-bit gray)
        "gray2" if !gray_src => {
            hit(clause::WIC_FROM_GRAY2);
            put_bits(row, x as u32, 2, ((gray() as u32 >> 4) * 3 + 7) / 15)
        }
        "gray2" => put_bits(row, x as u32, 2, (gray() as u32 * 3 + 127) / 255),
        "gray4" => put_bits(row, x as u32, 4, gray() as u32 >> 4),
        "gray8" => row[o] = gray(),
        // (FOUND clean-room: MIL's GammaConvert_32bppBGRA_128bppRGBA then
        // GammaConvert_128bppRGBA_16bppGrayInt: each channel's linear float, the luma in f32,
        // back into 16 bits sRGB)
        "gray16" => {
            use super::wicimage::{float_srgb, gray_luma, linear16, FloatBack};
            let l = |c: u8| linear16(u16::from(c) * 257);
            w16(row, o, float_srgb(gray_luma(l(r), l(g), l(b)), FloatBack::Bits16));
        }
        "alpha8" => row[o] = a,
        "bgr555" => w16(row, o, ((r as u16 >> 3) << 10) | ((g as u16 >> 3) << 5) | (b as u16 >> 3)),
        "bgr565" => w16(row, o, ((r as u16 >> 3) << 11) | ((g as u16 >> 2) << 5) | (b as u16 >> 3)),
        "bgr24" => row[o..o + 3].copy_from_slice(&[b, g, r]),
        "rgb24" => row[o..o + 3].copy_from_slice(&[r, g, b]),
        // (MEASURED: 32bppBGR keeps the alpha byte, 32bppRGB writes 0xff)
        "bgr32" | "bgra" => row[o..o + 4].copy_from_slice(&[b, g, r, a]),
        "rgba" => row[o..o + 4].copy_from_slice(&[r, g, b, a]),
        "rgb32" => row[o..o + 4].copy_from_slice(&[r, g, b, 255]),
        "cmyk" => row[o..o + 4].copy_from_slice(&super::cmyk::from_rgb().ok_or(Fail::Bad)?.cmyk8([b, g, r])),
        "pbgra" => row[o..o + 4].copy_from_slice(&[premultiply(b, a), premultiply(g, a), premultiply(r, a), a]),
        "prgba" => row[o..o + 4].copy_from_slice(&[premultiply(r, a), premultiply(g, a), premultiply(b, a), a]),
        "rgb48" => {
            for (k, c) in [r, g, b].into_iter().enumerate() {
                w16(row, o + 2 * k, c as u16 * 257);
            }
        }
        "rgba64" => {
            for (k, c) in [r, g, b, a].into_iter().enumerate() {
                w16(row, o + 2 * k, c as u16 * 257);
            }
        }
        "idx1" | "idx2" | "idx4" | "idx8" => {
            let b2 = bits(f);
            put_bits_or_byte(row, x, b2, u32::from(nearest_entry(b2, [b, g, r])));
        }
        _ => return Err(Fail::Bad),
    }
    Ok(())
}

/// the converter's pixels: `w x h` of `from` (rows `stride(from, w)` bytes) as `to`, the bits
/// past each row's last pixel 0
pub fn convert(from: usize, to: usize, w: u32, h: u32, data: &[u8]) -> Res<Vec<u8>> {
    let pal = palette();
    let (ss, ds) = (stride(from, w), stride(to, w));
    if from == to {
        hit(clause::WIC_SAME);
        let mut o = data[..ss * h as usize].to_vec();
        let tail = (w * bits(to)) % 8;
        if tail != 0 {
            for y in 0..h as usize {
                o[y * ds + ds - 1] &= (0xff00u32 >> tail) as u8;
            }
        }
        return Ok(o);
    }
    let bgra = crate::wic_fmt!("bgra");
    if from != bgra && to != bgra {
        // (FOUND clean-room: a pair from or to 32bppCMYK goes through 32bppBGRA, windowscodecsext's
        // N-channel converter on the CMYK side: WIC.TO.CMYK, WIC.FROM.CMYK)
        // (between the 16-bit formats: MIL's direct ops, as wicimage has them)
        if wide(from) && wide(to) {
            hit(clause::WIC_PAIR);
            let mut img = super::wicimage::Img::new(w, h, FORMATS[from].0);
            img.px = data[..ss * h as usize].to_vec();
            return Ok(super::wicimage::wic_convert(&img, FORMATS[to].0)?.px);
        }
        hit(clause::WIC_PAIR);
    }
    let mut out = vec![0u8; ds * h as usize];
    for y in 0..h as usize {
        let src = &data[y * ss..(y + 1) * ss];
        let dst = &mut out[y * ds..(y + 1) * ds];
        for x in 0..w as usize {
            let b = bits(from);
            let (s, v) = if b < 8 { (&src[..0], get_bits(src, x as u32, b)) } else if b == 8 { (&src[x..x + 1], src[x] as u32) } else { let n = (b / 8) as usize; (&src[x * n..x * n + n], 0) };
            let p = to_bgra(from, s, v, &pal)?;
            from_bgra(to, p, dst, x, matches!(FORMATS[from].0, "bw" | "gray2" | "gray4" | "gray8"))?;
        }
    }
    Ok(out)
}

/// the executor's source bytes: xorshift32 from the seed, or byte i = i mod 256
pub fn source(from: usize, w: u32, h: u32, seed: Option<u32>) -> Vec<u8> {
    let n = stride(from, w) * h as usize;
    match seed {
        None => (0..n).map(|i| i as u8).collect(),
        Some(s) => {
            let mut r = crate::rng::XorShift32::new(s);
            (0..n).map(|_| r.next() as u8).collect()
        }
    }
}

/// `convert <from> <w> <h> <seed|ramp> <to>`
pub fn op(op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
    let t = op.tok.as_slice();
    if t[0] != "convert" {
        return Ok(false);
    }
    if t.len() != 6 {
        return Err(Fail::Bad);
    }
    let n = |i: usize| crate::case::num(&t[i]).ok_or(Fail::Bad);
    let from = format(&t[1]).ok_or(Fail::Bad)?;
    let to = format(&t[5]).ok_or(Fail::Bad)?;
    let (w, h) = (n(2)?, n(3)?);
    if !(1..=256).contains(&w) || !(1..=64).contains(&h) {
        return Err(Fail::Bad);
    }
    let (w, h) = (w as u32, h as u32);
    let seed = if t[4] == "ramp" { None } else { Some(n(4)? as u32) };
    let o = convert(from, to, w, h, &source(from, w, h, seed))?;
    hit(clause::WIC_CAN);
    out.push(format!("conv {w} {h} can 1"));
    for row in o.chunks(stride(to, w)) {
        out.push(row.iter().map(|b| format!("{b:02x}")).collect());
    }
    Ok(true)
}
