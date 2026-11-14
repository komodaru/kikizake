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

use super::{Fail, Res};
use crate::spec::hit;

pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const E_NOTIMPL: u32 = 0x8000_4001;
pub const PALETTE_UNAVAILABLE: u32 = 0x8898_2f45;
pub const INSUFFICIENT_BUFFER: u32 = 0x8898_2f8c;
pub const COMPONENT_NOT_FOUND: u32 = 0x8898_2f50;

/// every pixel format the executor names: (name, bits a pixel); the first 23 are model::wic's
pub const FORMATS: &[(&str, u32)] = &[
    ("bw", 1), ("gray2", 2), ("gray4", 4), ("gray8", 8), ("gray16", 16), ("bgr555", 16), ("bgr565", 16), ("bgr24", 24),
    ("rgb24", 24), ("bgr32", 32), ("bgra", 32), ("pbgra", 32), ("rgb32", 32), ("rgba", 32), ("prgba", 32), ("rgb48", 48),
    ("rgba64", 64), ("cmyk", 32), ("alpha8", 8), ("idx1", 1), ("idx2", 2), ("idx4", 4), ("idx8", 8),
    ("bgr48", 48), ("bgra64", 64), ("prgba64", 64), ("pbgra64", 64), ("bgr101010", 32), ("grayfloat", 32), ("rgbafloat", 128),
    ("prgbafloat", 128), ("rgbfloat", 128), ("rgba64fixed", 64), ("bgra64fixed", 64), ("rgba64half", 64), ("rgb96float", 96),
    ("cmyk64", 64), ("cmyka40", 40), ("cmyka80", 80),
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
        if b < 8 {
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
        if b < 8 {
            let bit = x * b;
            let shift = 8 - b - bit % 8;
            let m = ((((1u16 << b) - 1) as u8) as u32) << shift;
            let byte = &mut row[(bit / 8) as usize];
            *byte = (*byte & !(m as u8)) | ((v[0] as u32) << shift & m) as u8;
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
    if crate::model::wic::indexed(f) {
        i.pal = Some(crate::model::wic::palette());
    }
    hit("WICIMAGE.BITMAP");
    i
}

/// MEASURED/ASSUMED: nearest neighbor: destination pixel d takes source pixel
/// floor((d + 0.5) * src / dst)
pub fn nearest(d: u32, src: u32, dst: u32) -> u32 {
    (((2 * d as u64 + 1) * src as u64) / (2 * dst as u64)) as u32
}

pub fn scale(s: &Img, w: u32, h: u32, mode: &str) -> Res<Img> {
    if mode != "nn" {
        return Err(Fail::Unmodeled("WICIMAGE.SCALE.FILTER"));
    }
    hit("WICIMAGE.SCALE.NEAREST");
    let mut o = Img { w, h, px: vec![0; stride(s.fmt, w) * h as usize], ..s.clone() };
    for y in 0..h {
        for x in 0..w {
            o.put(x, y, &s.get(nearest(x, s.w, w), nearest(y, s.h, h)));
        }
    }
    Ok(o)
}

/// WICBitmapTransformOptions: rotation clockwise (low two bits), then the flips
pub fn flip(s: &Img, opt: u32) -> Img {
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
    if !inside(s, x, y, w, h) {
        hit("WICIMAGE.COPY.RECT");
        return (E_INVALIDARG, vec![]);
    }
    let n = stride(s.fmt, w as u32) as i64;
    if let Some((st, size)) = bufs {
        if st < n {
            hit("WICIMAGE.COPY.STRIDE");
            return (E_INVALIDARG, vec![]);
        }
        if size < st * (h - 1) + n {
            hit("WICIMAGE.COPY.BUFFER");
            return (INSUFFICIENT_BUFFER, vec![]);
        }
    }
    hit("WICIMAGE.COPY");
    let c = clip(s, x as u32, y as u32, w as u32, h as u32);
    (0, c.px.chunks(n as usize).map(<[u8]>::to_vec).collect())
}

/* ---------------------------------------------------------------------------------------- */
/* palettes */

/// InitializePredefined: (hr, type, colors)
pub fn predefined(t: u32, transparent: bool) -> Res<(u32, u32, Vec<u32>)> {
    let gray = |n: u32| -> Vec<u32> { (0..n).map(|i| 0xff00_0000 | (i * 255 / (n - 1)) * 0x10101).collect() };
    let mut c = match t {
        2 => {
            hit("WICIMAGE.PALETTE.BW");
            vec![0xff00_0000, 0xffff_ffff]
        }
        3..=9 => return Err(Fail::Unmodeled("WICIMAGE.PALETTE.HALFTONE")),
        10 => gray(4),
        11 => gray(16),
        12 => gray(256),
        _ => {
            hit("WICIMAGE.PALETTE.TYPE");
            return Ok((E_INVALIDARG, 0, vec![]));
        }
    };
    if t >= 10 {
        hit("WICIMAGE.PALETTE.GRAY");
    }
    if transparent {
        hit("WICIMAGE.PALETTE.TRANSPARENT");
        if c.len() == 256 {
            c[255] = 0;
        } else {
            c.push(0);
        }
        return Ok((0, 0, c));
    }
    Ok((0, t, c))
}

fn palette_line(op: &str, hr: u32, t: u32, c: &[u32], out: &mut Vec<String>) {
    let bw = c.len() == 2 && c.contains(&0xff00_0000) && c.contains(&0xffff_ffff);
    let gray = c.iter().all(|&v| (v >> 16 & 255) == (v >> 8 & 255) && (v >> 8 & 255) == (v & 255));
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
    pub frames: Vec<Res<Img>>,
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
        bmp(b).map(|i| Ok(Decoded { container: "bmp", frames: vec![Ok(i)] }))
    } else if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(b).map(|i| Ok(Decoded { container: "png", frames: vec![Ok(i)] }))
    } else if b.starts_with(b"GIF8") {
        gif(b)
    } else if b.starts_with(b"II\x2a\0") || b.starts_with(b"MM\0\x2a") {
        tiff(b)
    } else if b.starts_with(&[0, 0, 1, 0]) {
        ico(b)
    } else if b.starts_with(b"DDS ") {
        dds(b)
    } else {
        hit("WICIMAGE.DECODE.UNKNOWN");
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
fn bmp(b: &[u8]) -> Res<Img> {
    let open = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let (Some(off), Some(hdr)) = (rd32(b, 10), rd32(b, 14)) else { return open };
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
    hit("WICIMAGE.BMP.HEADER");
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
        return Err(Fail::Unmodeled("WICIMAGE.BMP.RLE"));
    }
    if data.len() < src_stride * h as usize {
        return open;
    }
    let fmt = match (bpp, masks) {
        (1, _) => "idx1",
        (4, _) => "idx4",
        (8, _) => "idx8",
        (16, None) | (16, Some([0x7c00, 0x03e0, 0x001f, 0])) => "bgr555",
        (16, Some([0xf800, 0x07e0, 0x001f, 0])) => "bgr565",
        (24, None) => "bgr24",
        (32, None) | (32, Some([0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0])) => if hdr == 124 && masks.is_none() { return Err(Fail::Unmodeled("WICIMAGE.BMP.V5ALPHA")) } else { "bgr32" },
        (32, Some([0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000])) => "bgra",
        _ => return Err(Fail::Unmodeled("WICIMAGE.BMP.FIELDS")),
    };
    hit("WICIMAGE.BMP.FORMAT");
    let mut i = Img::new(w, h, fmt);
    i.dpi = dpi;
    let n = i.stride();
    for y in 0..h as usize {
        let sy = if top { y } else { h as usize - 1 - y };
        i.px[y * n..y * n + n].copy_from_slice(&data[sy * src_stride..sy * src_stride + n]);
    }
    if bpp < 8 && w as usize * bpp as usize % 8 != 0 {
        // (the bits past the last pixel of a row are the file's: the executor zeroes them)
    }
    if bpp <= 8 {
        hit("WICIMAGE.BMP.PALETTE");
        i.pal = Some(pal);
    }
    Ok(i)
}

/// zlib (RFC 1950), stored blocks only: anything else is left to Windows
fn inflate_stored(z: &[u8]) -> Option<Vec<u8>> {
    let mut o = Vec::new();
    let mut p = 2;
    loop {
        let hdr = *z.get(p)?;
        if hdr >> 1 & 3 != 0 {
            return None;
        }
        let len = rd16(z, p + 1)? as usize;
        o.extend_from_slice(z.get(p + 5..p + 5 + len)?);
        p += 5 + len;
        if hdr & 1 != 0 {
            return Some(o);
        }
    }
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
        let f = *data.get(p)?;
        let mut row = data.get(p + 1..p + 1 + bpr)?.to_vec();
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
fn png(b: &[u8]) -> Res<Img> {
    let open = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let mut p = 8;
    let (mut ihdr, mut plte, mut trns, mut phys, mut idat) = (None, None, None, None, Vec::new());
    loop {
        let Some(len) = b.get(p..p + 4).map(|x| u32::from_be_bytes(x.try_into().unwrap()) as usize) else { return open };
        let Some(body) = b.get(p + 4..p + 8 + len) else { return open };
        let Some(crc) = b.get(p + 8 + len..p + 12 + len) else { return open };
        if crc32(body) != u32::from_be_bytes(crc.try_into().unwrap()) {
            return Err(Fail::Unmodeled("WICIMAGE.PNG.CRC"));
        }
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
    let Some(raw) = inflate_stored(&idat) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
    hit("WICIMAGE.PNG.CHUNKS");
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
    let passes: Vec<(u32, u32, u32, u32)> = if il { crate::gen::WICIMAGE_ADAM7.to_vec() } else { vec![(0, 0, 1, 1)] };
    let mut at = 0;
    for (x0, y0, dx, dy) in passes {
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
        hit("WICIMAGE.PNG.ADAM7");
    }
    let dpi = match phys {
        Some(ph) if ph.len() == 9 && ph[8] == 1 => {
            let px = u32::from_be_bytes(ph[0..4].try_into().unwrap());
            let py = u32::from_be_bytes(ph[4..8].try_into().unwrap());
            hit("WICIMAGE.PNG.PHYS");
            (px as f64 * 0.0254, py as f64 * 0.0254)
        }
        _ => (96.0, 96.0),
    };
    if trns.is_some() && ct != 3 {
        return Err(Fail::Unmodeled("WICIMAGE.PNG.TRNS"));
    }
    let fmt = match (ct, depth) {
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
    hit("WICIMAGE.PNG.FORMAT");
    let mut img = Img::new(w, h, fmt);
    img.dpi = dpi;
    for y in 0..h {
        for x in 0..w {
            let s = get_sample_bits(&full[y as usize], x, pbits);
            let be16 = |k: usize| [s[2 * k + 1], s[2 * k]];
            let v: Vec<u8> = match (ct, depth) {
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
    if ct == 3 {
        let Some(pl) = plte else { return open };
        let t = trns.unwrap_or_default();
        hit("WICIMAGE.PNG.PALETTE");
        img.pal = Some(pl.chunks(3).enumerate().map(|(i, c)| (*t.get(i).unwrap_or(&255) as u32) << 24 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect());
    }
    Ok(img)
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
fn lzw(min: u32, data: &[u8], n: usize) -> Option<Vec<u8>> {
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
            acc |= (*data.get(p)? as u32) << nacc;
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
        if let Some(pc) = prev {
            if dict.len() < 4096 {
                let mut e = dict[pc as usize].clone();
                e.push(entry[0]);
                dict.push(e);
            }
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
    let open = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let Some(flags) = b.get(10) else { return open };
    let mut p = 13;
    let table = |p: usize, flags: u8| -> Option<Vec<u32>> {
        let n = 2usize << (flags & 7);
        let t = b.get(p..p + 3 * n)?;
        Some(t.chunks(3).map(|c| 0xff00_0000 | (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect())
    };
    let global = if flags & 0x80 != 0 {
        let Some(t) = table(p, *flags) else { return open };
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
                let (Some(w), Some(h), Some(&f)) = (rd16(b, p + 5), rd16(b, p + 7), b.get(p + 9)) else { return open };
                p += 10;
                let local = if f & 0x80 != 0 {
                    let Some(t) = table(p, f) else { return open };
                    p += 3 * t.len();
                    Some(t)
                } else {
                    None
                };
                let Some(&min) = b.get(p) else { return open };
                p += 1;
                let mut data = Vec::new();
                while let Some(&n) = b.get(p) {
                    let Some(d) = b.get(p + 1..p + 1 + n as usize) else { return open };
                    data.extend_from_slice(d);
                    p += 1 + n as usize;
                    if n == 0 {
                        break;
                    }
                }
                let Some(idx) = lzw(min as u32, &data, (w * h) as usize) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
                if idx.len() < (w * h) as usize {
                    return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
                }
                hit("WICIMAGE.GIF.LZW");
                let rows: Vec<u32> = if f & 0x40 != 0 {
                    hit("WICIMAGE.GIF.INTERLACE");
                    [(0, 8), (4, 8), (2, 4), (1, 2)].iter().flat_map(|&(s, d)| (s..h).step_by(d)).collect()
                } else {
                    (0..h).collect()
                };
                let mut img = Img::new(w, h, "idx8");
                for (k, &y) in rows.iter().enumerate() {
                    img.px[(y * w) as usize..((y + 1) * w) as usize].copy_from_slice(&idx[k * w as usize..(k + 1) * w as usize]);
                }
                let Some(mut pal) = local.or_else(|| global.clone()) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
                if let Some(t) = transparent {
                    hit("WICIMAGE.GIF.TRANSPARENT");
                    if let Some(c) = pal.get_mut(t as usize) {
                        *c &= 0x00ff_ffff;
                    }
                }
                hit("WICIMAGE.GIF.FRAME");
                img.pal = Some(pal);
                frames.push(Ok(img));
                transparent = None;
            }
            Some(0x3b) => break,
            _ => return open,
        }
    }
    Ok(Ok(Decoded { container: "gif", frames }))
}

/// TIFF 6.0 baseline, uncompressed
fn tiff(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let open = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
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
        frames.push(tiff_image(b, &tags));
    }
    hit("WICIMAGE.TIFF.IFD");
    Ok(Ok(Decoded { container: "tiff", frames }))
}

fn tiff_image(b: &[u8], t: &std::collections::BTreeMap<u32, Vec<u32>>) -> Res<Img> {
    let one = |k: u32, d: u32| t.get(&k).and_then(|v| v.first().copied()).unwrap_or(d);
    let (w, h) = (one(256, 0), one(257, 0));
    let (bits, ph, spp, planar) = (one(258, 1), one(262, 1), one(277, 1), one(284, 1));
    let extra = t.get(&338).and_then(|v| v.first().copied());
    let sfmt = one(339, 1);
    let fmt = match (ph, bits, spp, extra) {
        (0 | 1, 1, 1, None) => "bw",
        (0 | 1, 4, 1, None) => "gray4",
        (0 | 1, 8, 1, None) => "gray8",
        (0 | 1, 16, 1, None) => "gray16",
        (1, 32, 1, None) if sfmt == 3 => "grayfloat",
        (3, 1, 1, None) => "idx1",
        (3, 4, 1, None) => "idx4",
        (3, 8, 1, None) => "idx8",
        (2, 8, 3, None) => "bgr24",
        (2, 16, 3, None) => "rgb48",
        (2, 8, 4, Some(2)) => "bgra",
        (2, 8, 4, Some(1)) => "pbgra",
        (2, 16, 4, Some(2)) => "rgba64",
        (2, 16, 4, Some(1)) => "prgba64",
        (5, 8, 4, None) => "cmyk",
        (5, 16, 4, None) => "cmyk64",
        _ => return Err(Fail::Unmodeled("WICIMAGE.TIFF.OTHER")),
    };
    if ph == 0 || planar == 2 || fmt == "grayfloat" || fmt == "prgba64" {
        return Err(Fail::Unmodeled("WICIMAGE.TIFF.OTHER"));
    }
    hit("WICIMAGE.TIFF.FORMAT");
    let (Some(offs), Some(counts)) = (t.get(&273), t.get(&279)) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
    let mut data = Vec::new();
    for (o, c) in offs.iter().zip(counts) {
        let Some(d) = b.get(*o as usize..(*o + *c) as usize) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
        data.extend_from_slice(d);
    }
    let row = (w as usize * bits as usize * spp as usize).div_ceil(8);
    if data.len() < row * h as usize {
        return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    }
    let mut img = Img::new(w, h, fmt);
    let big = b[0] == b'M';
    for y in 0..h {
        let r = &data[y as usize * row..(y as usize + 1) * row];
        for x in 0..w {
            let s = get_sample_bits(r, x, bits * spp);
            let w16 = |k: usize| if big { [s[2 * k + 1], s[2 * k]] } else { [s[2 * k], s[2 * k + 1]] };
            let v: Vec<u8> = match fmt {
                "bgr24" => vec![s[2], s[1], s[0]],
                "bgra" | "pbgra" => vec![s[2], s[1], s[0], s[3]],
                "gray16" => w16(0).to_vec(),
                "rgb48" => [w16(0), w16(1), w16(2)].concat(),
                "rgba64" => [w16(0), w16(1), w16(2), w16(3)].concat(),
                "cmyk64" => [w16(0), w16(1), w16(2), w16(3)].concat(),
                _ => s,
            };
            img.put(x, y, &v);
        }
    }
    // resolution: inches (2) as they are, centimeters (3) times 2.54
    if let (Some(x), Some(y)) = (t.get(&282), t.get(&283)) {
        let unit = one(296, 2);
        let k = match unit {
            2 => 1.0,
            3 => 2.54,
            _ => return Err(Fail::Unmodeled("WICIMAGE.TIFF.UNIT")),
        };
        hit("WICIMAGE.TIFF.RESOLUTION");
        img.dpi = (x[0] as f64 * k, y[0] as f64 * k);
    }
    if ph == 3 {
        let Some(m) = t.get(&320) else { return Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN")) };
        let n = 1usize << bits;
        hit("WICIMAGE.TIFF.PALETTE");
        img.pal = Some((0..n).map(|i| 0xff00_0000 | (m[i] >> 8) << 16 | (m[n + i] >> 8) << 8 | m[2 * n + i] >> 8).collect());
    }
    Ok(img)
}

/// ICO: each entry a frame of 32bppBGRA ("ICO Format Overview"): a PNG, or a BMP without its
/// file header, its height doubled, the colors then the AND mask
fn ico(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let broken = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let Some(n) = rd16(b, 4) else { return broken };
    let mut frames = Vec::new();
    for k in 0..n as usize {
        let (Some(size), Some(off)) = (rd32(b, 6 + 16 * k + 8), rd32(b, 6 + 16 * k + 12)) else { return broken };
        let Some(d) = b.get(off as usize..(off + size) as usize) else { return broken };
        frames.push(ico_entry(d));
    }
    hit("WICIMAGE.ICO");
    Ok(Ok(Decoded { container: "ico", frames }))
}

fn ico_entry(d: &[u8]) -> Res<Img> {
    let bgra = crate::model::wic::format("bgra").unwrap();
    if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        let p = png(d)?;
        hit("WICIMAGE.ICO.PNG");
        let from = crate::model::wic::format(p.fmt).ok_or(Fail::Unmodeled("WICIMAGE.ICO.PNG"))?;
        let px = crate::model::wic::convert(from, bgra, p.w, p.h, &p.px)?;
        return Ok(Img { px, fmt: "bgra", pal: None, dpi: (96.0, 96.0), ..p });
    }
    let broken = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let (Some(hdr), Some(w), Some(h2), Some(bpp)) = (rd32(d, 0), rd32(d, 4), rd32(d, 8), rd16(d, 14)) else { return broken };
    let h = h2 / 2;
    let colors = if bpp <= 8 { 1usize << bpp } else { 0 };
    let used = rd32(d, 32).unwrap_or(0) as usize;
    let colors = if used > 0 && used < colors { used } else { colors };
    let pal: Vec<u32> = (0..colors).filter_map(|i| d.get(hdr as usize + 4 * i..hdr as usize + 4 * i + 4).map(|c| (c[2] as u32) << 16 | (c[1] as u32) << 8 | c[0] as u32)).collect();
    let xs = (w as usize * bpp as usize).div_ceil(32) * 4;
    let ms = (w as usize).div_ceil(32) * 4;
    let at = hdr as usize + 4 * colors;
    let Some(xor) = d.get(at..at + xs * h as usize) else { return broken };
    let Some(and) = d.get(at + xs * h as usize..at + xs * h as usize + ms * h as usize) else { return broken };
    hit("WICIMAGE.ICO.BMP");
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
                24 => [row[3 * x as usize], row[3 * x as usize + 1], row[3 * x as usize + 2], 255],
                32 => {
                    let p = &row[4 * x as usize..4 * x as usize + 4];
                    [p[0], p[1], p[2], p[3]]
                }
                _ => return Err(Fail::Unmodeled("WICIMAGE.ICO.DEPTH")),
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

fn rgb565(v: u32) -> [u32; 3] {
    let x5 = |v: u32| (v << 3) | (v >> 2);
    let x6 = |v: u32| (v << 2) | (v >> 4);
    [x5(v & 31), x6(v >> 5 & 63), x5(v >> 11 & 31)]
}

/// one 4x4 block of BC1 (DXT1), BC2 (DXT3) or BC3 (DXT5): 16 straight BGRA pixels, row by row
/// ("Texture Block Compression in Direct3D 11"; ASSUMED: thirds and halves truncated)
pub fn bc_block(kind: u8, blk: &[u8]) -> [[u8; 4]; 16] {
    let (alpha, color) = if kind == 1 { (&blk[..0], blk) } else { (&blk[..8], &blk[8..]) };
    let c0 = rd16(color, 0).unwrap();
    let c1 = rd16(color, 2).unwrap();
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |p: u32, q: u32, d: u32| -> [u32; 3] { [0, 1, 2].map(|i| (p * a[i] + q * b[i]) / d) };
    let four = kind != 1 || c0 > c1;
    let pal: [[u32; 4]; 4] = if four {
        let (m, n) = (mix(2, 1, 3), mix(1, 2, 3));
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [m[0], m[1], m[2], 255], [n[0], n[1], n[2], 255]]
    } else {
        let m = mix(1, 1, 2);
        [[a[0], a[1], a[2], 255], [b[0], b[1], b[2], 255], [m[0], m[1], m[2], 255], [0, 0, 0, 0]]
    };
    let idx = rd32(color, 4).unwrap();
    let mut out = [[0u8; 4]; 16];
    let alphas: [u32; 8] = if kind == 3 {
        let (a0, a1) = (alpha[0] as u32, alpha[1] as u32);
        if a0 > a1 {
            [a0, a1, (6 * a0 + a1) / 7, (5 * a0 + 2 * a1) / 7, (4 * a0 + 3 * a1) / 7, (3 * a0 + 4 * a1) / 7, (2 * a0 + 5 * a1) / 7, (a0 + 6 * a1) / 7]
        } else {
            [a0, a1, (4 * a0 + a1) / 5, (3 * a0 + 2 * a1) / 5, (2 * a0 + 3 * a1) / 5, (a0 + 4 * a1) / 5, 0, 255]
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
        *o = [c[2] as u8, c[1] as u8, c[0] as u8, al as u8];
    }
    out
}

/// DDS: BC1, BC2, BC3 ("DDS Format Overview"), each mip level a frame (ASSUMED), 32bppBGRA
fn dds(b: &[u8]) -> Res<Result<Decoded, u32>> {
    let broken = Err(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"));
    let (Some(h), Some(w), Some(mips), Some(pf)) = (rd32(b, 12), rd32(b, 16), rd32(b, 28), rd32(b, 80)) else { return broken };
    if pf & 4 == 0 {
        return Err(Fail::Unmodeled("WICIMAGE.DDS.OTHER"));
    }
    let kind = match b.get(84..88) {
        Some(b"DXT1") => 1u8,
        Some(b"DXT3") => 2,
        Some(b"DXT5") => 3,
        _ => return Err(Fail::Unmodeled("WICIMAGE.DDS.OTHER")),
    };
    let bs = if kind == 1 { 8 } else { 16 };
    let mut at = 128;
    let mut frames = Vec::new();
    for m in 0..mips.max(1) {
        let (mw, mh) = ((w >> m).max(1), (h >> m).max(1));
        let (bw, bh) = (mw.div_ceil(4), mh.div_ceil(4));
        // (ASSUMED: BC1 as premultiplied, its alpha all 0 or 255; BC2 and BC3 straight)
        let mut img = Img::new(mw, mh, if kind == 1 { "pbgra" } else { "bgra" });
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
        frames.push(Ok(img));
    }
    hit("WICIMAGE.DDS");
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

pub enum Encoded {
    Bytes(Vec<u8>),
    /// a lossless file the model has no bytes of: what decodes from it
    Decodes(&'static str, Img),
}

/* ---------------------------------------------------------------------------------------- */

#[derive(Default)]
pub struct WicImage {
    cur: Option<Img>,
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
        if needs_cur && self.cur.is_none() {
            if t[0] == "wpixels" && ![1, 5, 7].contains(&t.len()) {
                return Err(Fail::Bad);
            }
            hit("WICIMAGE.NOSOURCE");
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
                if crate::model::wic::FORMATS[f].0 == "alpha8" {
                    return Err(Fail::Unmodeled("WICIMAGE.BITMAP.ALPHA8"));
                }
                out.push("wbits 00000000".into());
                self.cur = Some(bitmap(crate::model::wic::FORMATS[f].0, w as u32, h as u32, seed));
            }
            "wscale" => {
                let (w, h) = (n(1)?, n(2)?);
                let m = *t.get(3).ok_or(Fail::Bad)?;
                if !["nn", "linear", "cubic", "fant", "hqcubic"].contains(&m) {
                    return Err(Fail::Bad);
                }
                if w <= 0 || h <= 0 || w > 0xffff || h > 0xffff {
                    return Err(Fail::Unmodeled("WICIMAGE.SCALE.SIZE"));
                }
                let s = scale(self.cur.as_ref().unwrap(), w as u32, h as u32, m)?;
                out.push("wscale 00000000".into());
                self.cur = Some(s);
            }
            "wflip" => {
                let o = n(1)? as u32;
                if o & !0x1b != 0 {
                    return Err(Fail::Unmodeled("WICIMAGE.FLIP.BAD"));
                }
                hit("WICIMAGE.FLIP");
                out.push("wflip 00000000".into());
                self.cur = Some(flip(self.cur.as_ref().unwrap(), o));
            }
            "wclip" => {
                let (x, y, w, h) = (n(1)?, n(2)?, n(3)?, n(4)?);
                let c = self.cur.as_ref().unwrap();
                if !inside(c, x, y, w, h) {
                    hit("WICIMAGE.CLIP.RECT");
                    out.push(format!("wclip {E_INVALIDARG:08x}"));
                    self.cur = None;
                } else {
                    hit("WICIMAGE.CLIP");
                    out.push("wclip 00000000".into());
                    self.cur = Some(clip(c, x as u32, y as u32, w as u32, h as u32));
                }
            }
            "wconv" => {
                let to = *t.get(1).ok_or(Fail::Bad)?;
                let c = self.cur.as_ref().unwrap();
                let (Some(fi), Some(ti)) = (crate::model::wic::format(c.fmt), crate::model::wic::format(to)) else { return Err(Fail::Unmodeled("WICIMAGE.CONVERT")) };
                if c.pal.is_some() && c.pal != Some(crate::model::wic::palette()) {
                    return Err(Fail::Unmodeled("WICIMAGE.CONVERT"));
                }
                hit("WICIMAGE.CONVERT");
                let px = crate::model::wic::convert(fi, ti, c.w, c.h, &c.px)?;
                out.push("wconv 00000000".into());
                let pal = if crate::model::wic::indexed(ti) { Some(crate::model::wic::palette()) } else { None };
                self.cur = Some(Img { px, fmt: fmt_static(to), pal, ..c.clone() });
            }
            "wcolor" => return Err(Fail::Unmodeled("WICIMAGE.COLOR.TRANSFORM")),
            "wctx" => {
                let v = n(1)?;
                if v == 1 || v == 2 {
                    hit("WICIMAGE.COLOR.EXIF");
                    out.push(format!("wctx 00000000 type 2 exif 00000000 {v}"));
                } else {
                    return Err(Fail::Unmodeled("WICIMAGE.COLOR.EXIF.BAD"));
                }
            }
            "winfo" => {
                let c = self.cur.as_ref().unwrap();
                hit("WICIMAGE.INFO");
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
                        hit("WICIMAGE.SRCPAL");
                        palette_line("wsrcpal", 0, 0, p, out)
                    }
                    None => {
                        hit("WICIMAGE.SRCPAL.NONE");
                        out.push(format!("wsrcpal {PALETTE_UNAVAILABLE:08x} type 0 count 0 bw 0 gray 0 alpha 0"));
                    }
                }
            }
            "wdecode" => {
                let h = *t.get(1).ok_or(Fail::Bad)?;
                if t.len() != 2 || h.len() % 2 != 0 {
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
                let f = d.frames.get(i).ok_or(Fail::Unmodeled("WICIMAGE.DECODE.BROKEN"))?.clone()?;
                hit("WICIMAGE.FRAME");
                out.push("wframe 00000000".into());
                self.cur = Some(f);
            }
            "wencode" => {
                let c = self.cur.as_ref().unwrap().clone();
                let cont = *t.get(1).ok_or(Fail::Bad)?;
                let mut first = 2;
                let mut fmt = c.fmt;
                if t.len() > 2 && !t[2].contains('=') {
                    fmt = fmt_static(t[2]);
                    first = 3;
                }
                if t.len() > first {
                    return Err(Fail::Unmodeled("WICIMAGE.ENCODE.OPTIONS"));
                }
                if !encoder_formats(cont).contains(&fmt) {
                    return Err(Fail::Unmodeled("WICIMAGE.ENCODE.NEGOTIATE"));
                }
                if fmt != c.fmt {
                    return Err(Fail::Unmodeled("WICIMAGE.ENCODE.CONVERT"));
                }
                hit("WICIMAGE.ENCODE.FORMAT");
                out.push(format!("wencode 00000000 {fmt} ok"));
                self.enc = Some(match cont {
                    "bmp" => Encoded::Bytes(encode_bmp(&c)?),
                    "png" | "tiff" | "gif" => {
                        hit("WICIMAGE.ENCODE.LOSSLESS");
                        Encoded::Decodes(if cont == "png" { "png" } else if cont == "tiff" { "tiff" } else { "gif" }, c)
                    }
                    _ => return Err(Fail::Unmodeled("WICIMAGE.ENCODE.LOSSY")),
                });
            }
            "wbytes" => match &self.enc {
                Some(Encoded::Bytes(b)) => {
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
                    if cont == "gif" {
                        return Err(Fail::Unmodeled("WICIMAGE.ENCODE.GIF"));
                    }
                    i.pal = img.pal.clone();
                    self.dec = Some(Decoded { container: cont, frames: vec![Ok(i)] });
                    self.enc = Some(Encoded::Decodes(cont, img));
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

/// ASSUMED: the BMP encoder writes a BITMAPINFOHEADER file, bottom-up, rows padded to 4 bytes,
/// the palette in full for indexed formats, the resolution in pixels a meter
pub fn encode_bmp(i: &Img) -> Res<Vec<u8>> {
    if !["idx1", "idx4", "idx8", "bgr555", "bgr565", "bgr24", "bgr32"].contains(&i.fmt) {
        return Err(Fail::Unmodeled("WICIMAGE.ENCODE.BMP"));
    }
    hit("WICIMAGE.ENCODE.BMP");
    let bpp = bits_of(i.fmt);
    let st = (i.w as usize * bpp as usize).div_ceil(32) * 4;
    let mut px = vec![0u8; st * i.h as usize];
    for y in 0..i.h as usize {
        let src = &i.px[y * i.stride()..(y + 1) * i.stride()];
        let d = (i.h as usize - 1 - y) * st;
        px[d..d + src.len()].copy_from_slice(src);
    }
    let pal: Vec<u32> = i.pal.clone().unwrap_or_default();
    let ppm = (i.dpi.0 / 0.0254 + 0.5) as u32;
    Ok(crate::gen::wicimage_bmp(40, i.w as i32, i.h as i32, bpp, if i.fmt == "bgr565" { 3 } else { 0 }, [0xf800, 0x07e0, 0x001f, 0], pal.len() as u32, &pal, &px, false, (ppm, ppm)))
}
