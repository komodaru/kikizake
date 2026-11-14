//! GDI+ bitmaps and matrices: an executable specification of the flat API's bitmaps (made with
//! GdipCreateBitmapFromScan0 and no buffer), their pixels through LockBits in any pixel format,
//! GetPixel / SetPixel, palettes, a Graphics' Clear and FillRectangleI, RotateFlip,
//! CloneBitmapArea, and Matrix (CreateMatrix2, Multiply, Invert, TransformPoints).
//!
//! Sources (Microsoft Learn): "Image Pixel Format Constants" (each format's channels),
//! "Bitmap::LockBits", "Bitmap::GetPixel", "Bitmap::SetPixel", "RotateFlipType", "Matrix::Matrix",
//! "Matrix::Multiply", "Matrix::Invert", "CompositingMode", "CompositingQuality", "Status".

use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

/// (case-language name, PixelFormat value)
pub const FORMATS: &[(&str, u32)] = &[
    ("1bppIndexed", 0x0003_0101),
    ("4bppIndexed", 0x0003_0402),
    ("8bppIndexed", 0x0003_0803),
    ("16bppGrayScale", 0x0010_1004),
    ("16bppRGB555", 0x0002_1005),
    ("16bppRGB565", 0x0002_1006),
    ("16bppARGB1555", 0x0006_1007),
    ("24bppRGB", 0x0002_1808),
    ("32bppRGB", 0x0002_2009),
    ("32bppARGB", 0x0026_200a),
    ("32bppPARGB", 0x000e_200b),
    ("48bppRGB", 0x0010_300c),
    ("64bppARGB", 0x0034_400d),
    ("64bppPARGB", 0x001c_400e),
];

pub const OK: u32 = 0;
pub const INVALID_PARAMETER: u32 = 2;

pub fn format(name: &str) -> Option<usize> {
    FORMATS.iter().position(|f| f.0 == name)
}
pub fn bits(f: usize) -> u32 {
    FORMATS[f].1 >> 8 & 255
}
pub fn indexed(f: usize) -> bool {
    f < 3
}
fn name(f: usize) -> &'static str {
    FORMATS[f].0
}
pub fn row_bytes(f: usize, w: u32) -> usize {
    (w * bits(f)).div_ceil(8) as usize
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    pub f: usize,
    pub w: u32,
    pub h: u32,
    pub rows: Vec<Vec<u8>>,
    pub pal: Vec<u32>,
}

/// a pixel's value (its bytes little-endian, or below 8 bits its index)
fn get_px(row: &[u8], x: u32, bits: u32) -> u64 {
    hit("GP.LAYOUT");
    if bits < 8 {
        let bit = x * bits;
        return (row[(bit / 8) as usize] as u64 >> (8 - bits - bit % 8)) & ((1 << bits) - 1);
    }
    let n = (bits / 8) as usize;
    let o = x as usize * n;
    row[o..o + n].iter().rev().fold(0u64, |a, &b| a << 8 | b as u64)
}
fn put_px(row: &mut [u8], x: u32, bits: u32, v: u64) {
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
pub fn default_palette(f: usize) -> Vec<u32> {
    let vga = [
        0xff00_0000u32, 0xff80_0000, 0xff00_8000, 0xff80_8000, 0xff00_0080, 0xff80_0080, 0xff00_8080, 0xff80_8080, 0xffc0_c0c0, 0xffff_0000, 0xff00_ff00, 0xffff_ff00,
        0xff00_00ff, 0xffff_00ff, 0xff00_ffff, 0xffff_ffff,
    ];
    match f {
        0 => vec![0xff00_0000, 0xffff_ffff],
        1 => vga.to_vec(),
        2 => {
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
        _ => vec![],
    }
}

/// palette flags GetImagePalette reports (PaletteFlagsGrayScale 2, PaletteFlagsHalftone 4)
fn palette_flags(f: usize) -> u32 {
    match f {
        2 => 4,
        _ => 0,
    }
}

fn x5(v: u64) -> u32 {
    ((v << 3) | (v >> 2)) as u32 & 255
}
fn x6(v: u64) -> u32 {
    ((v << 2) | (v >> 4)) as u32 & 255
}

/// a pixel value of `f` as straight ARGB
fn to_argb(f: usize, v: u64, pal: &[u32]) -> Res<u32> {
    let c = |r: u32, g: u32, b: u32| 0xff00_0000 | r << 16 | g << 8 | b;
    Ok(match name(f) {
        n if n.ends_with("Indexed") => {
            hit("GP.CONV.INDEXED");
            // (the default palettes have an entry for every index)
            pal[v as usize]
        }
        "16bppGrayScale" => return Err(Fail::Unmodeled("GP.GRAY16")),
        "16bppRGB555" => {
            hit("GP.CONV.EXPAND");
            c(x5(v >> 10 & 31), x5(v >> 5 & 31), x5(v & 31))
        }
        "16bppRGB565" => {
            hit("GP.CONV.EXPAND");
            c(x5(v >> 11 & 31), x6(v >> 5 & 63), x5(v & 31))
        }
        "16bppARGB1555" => {
            hit("GP.CONV.EXPAND");
            (if v & 0x8000 != 0 { 0xff00_0000 } else { 0 }) | (c(x5(v >> 10 & 31), x5(v >> 5 & 31), x5(v & 31)) & 0xff_ffff)
        }
        "24bppRGB" | "32bppRGB" => {
            hit("GP.CONV.OPAQUE");
            0xff00_0000 | v as u32 & 0xff_ffff
        }
        "32bppARGB" => v as u32,
        "32bppPARGB" => return Err(Fail::Unmodeled("GP.CONV.UNPREMULTIPLY")),
        _ => return Err(Fail::Unmodeled("GP.CONV.WIDE")),
    })
}

/// straight ARGB as a pixel value of `f`
fn from_argb(f: usize, c: u32) -> Res<u64> {
    let (a, r, g, b) = ((c >> 24) as u64, (c >> 16 & 255) as u64, (c >> 8 & 255) as u64, (c & 255) as u64);
    Ok(match name(f) {
        n if n.ends_with("Indexed") => return Err(Fail::Unmodeled("GP.LOCK.INDEXED")),
        "16bppGrayScale" => return Err(Fail::Unmodeled("GP.GRAY16")),
        "16bppRGB555" => {
            hit("GP.CONV.TRUNCATE");
            (r >> 3) << 10 | (g >> 3) << 5 | b >> 3
        }
        "16bppRGB565" => {
            hit("GP.CONV.TRUNCATE");
            (r >> 3) << 11 | (g >> 2) << 5 | b >> 3
        }
        "16bppARGB1555" => {
            hit("GP.CONV.TRUNCATE");
            (a >> 7) << 15 | (r >> 3) << 10 | (g >> 3) << 5 | b >> 3
        }
        "24bppRGB" => {
            hit("GP.CONV.OPAQUE");
            (c & 0xff_ffff) as u64
        }
        "32bppRGB" => {
            hit("GP.CONV.OPAQUE");
            (c | 0xff00_0000) as u64
        }
        "32bppARGB" => c as u64,
        "32bppPARGB" => return Err(Fail::Unmodeled("GP.CONV.PREMULTIPLY")),
        _ => return Err(Fail::Unmodeled("GP.CONV.WIDE")),
    })
}

/// a pixel of `from` as a pixel of `to` (LockBits, CloneBitmapArea)
fn convert(from: usize, to: usize, v: u64, pal: &[u32]) -> Res<u64> {
    if from == to {
        hit("GP.LOCK.SAME");
        return Ok(v);
    }
    from_argb(to, to_argb(from, v, pal)?)
}

impl Bitmap {
    pub fn new(f: usize, w: u32, h: u32) -> Bitmap {
        hit("GP.CREATE.ZERO");
        Bitmap { f, w, h, rows: vec![vec![0; row_bytes(f, w)]; h as usize], pal: default_palette(f) }
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
    hit("GP.LOCK.RECT");
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

fn fmt_at(t: &[String], i: usize) -> Res<usize> {
    t.get(i).and_then(|v| format(v)).ok_or(Fail::Bad)
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
pub fn mul(a: &Matrix, b: &Matrix) -> Matrix {
    hit("GP.MATRIX.MUL");
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// a point through a matrix
pub fn transform(m: &Matrix, x: f32, y: f32) -> (f32, f32) {
    hit("GP.MATRIX.POINT");
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

/// the inverse, or None when the determinant is 0
pub fn invert(m: &Matrix) -> Option<Matrix> {
    hit("GP.MATRIX.INVERT");
    let det = m[0] * m[3] - m[1] * m[2];
    if det == 0.0 || !det.is_finite() {
        return None;
    }
    Some([
        m[3] / det,
        -m[1] / det,
        -m[2] / det,
        m[0] / det,
        (m[2] * m[5] - m[3] * m[4]) / det,
        (m[1] * m[4] - m[0] * m[5]) / det,
    ])
}

fn elements(m: &Matrix) -> String {
    let mut s = "elements 0".to_string();
    for e in m {
        s += &format!(" {:08x}", e.to_bits());
    }
    s
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
        hit("GP.START");
        let needs_bitmap = matches!(name, "gpwrite" | "gpread" | "gpset" | "gpget" | "gpinfo" | "gpfill" | "gpclear" | "gprotate");
        match name {
            "gpbitmap" => {
                let s = Self::slot(t, 1)?;
                let (w, h, f) = (n(t, 2)?, n(t, 3)?, fmt_at(t, 4)?);
                self.bmp[s] = None;
                hit("GP.CREATE");
                if w <= 0 || h <= 0 {
                    return Err(Fail::Unmodeled("GP.CREATE.SIZE"));
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
                if f != b.f && (indexed(f) || indexed(b.f)) {
                    return Err(Fail::Unmodeled("GP.LOCK.INDEXED"));
                }
                let data = fill_bytes(t, 3, r.2 as u32, r.3 as u32, bits(f))?;
                out.push("write 0 0".into());
                hit("GP.LOCK.WRITE");
                for y in 0..r.3 as u32 {
                    for x in 0..r.2 as u32 {
                        let v = convert(f, b.f, get_px(&data[y as usize], x, bits(f)), &b.pal)?;
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
                if f != b.f && (indexed(f) || indexed(b.f)) {
                    return Err(Fail::Unmodeled("GP.LOCK.INDEXED"));
                }
                hit("GP.LOCK.READ");
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
                hit("GP.PIXEL.BOUNDS");
                if x < 0 || y < 0 || x >= b.w as i64 || y >= b.h as i64 {
                    out.push(format!("{w} {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if name == "gpget" {
                    hit("GP.PIXEL.GET");
                    out.push(format!("get 0 {:08x}", to_argb(b.f, b.px(x as u32, y as u32), &b.pal)?));
                } else {
                    if indexed(b.f) {
                        return Err(Fail::Unmodeled("GP.PIXEL.SET.INDEXED"));
                    }
                    hit("GP.PIXEL.SET");
                    let v = from_argb(b.f, c)?;
                    b.set_px(x as u32, y as u32, v);
                    out.push("set 0".into());
                }
            }
            "gpinfo" => {
                let b = self.bmp[Self::slot(t, 1)?].as_ref().unwrap();
                hit("GP.INFO");
                if b.pal.is_empty() {
                    return Err(Fail::Unmodeled("GP.INFO.NOPALETTE"));
                }
                out.push(format!("info {:08x} {} {} palette 0 {} {}", FORMATS[b.f].1, b.w, b.h, palette_flags(b.f), b.pal.len()));
                for ch in b.pal.chunks(16) {
                    out.push(ch.iter().map(|c| format!("{c:08x}")).collect::<Vec<_>>().join(" "));
                }
            }
            "gpfill" | "gpclear" => return Err(Fail::Unmodeled("GP.FILL")),
            "gprotate" => {
                let b = self.bmp[Self::slot(t, 1)?].as_mut().unwrap();
                let k = n(t, 2)?;
                if !(0..8).contains(&k) {
                    return Err(Fail::Bad);
                }
                hit("GP.ROTATE");
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
                hit("GP.CLONE");
                if !rect_ok(b, r) {
                    out.push(format!("clone {INVALID_PARAMETER}"));
                    return Ok(true);
                }
                if f != b.f {
                    return Err(Fail::Unmodeled("GP.CLONE.FORMAT"));
                }
                let mut c = Bitmap::new(f, r.2 as u32, r.3 as u32);
                c.pal = b.pal.clone();
                for y in 0..r.3 as u32 {
                    for x in 0..r.2 as u32 {
                        c.set_px(x, y, b.px(r.0 as u32 + x, r.1 as u32 + y));
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
                hit("GP.MATRIX");
                out.push("matrix 0".into());
                out.push(elements(&m));
                self.mat[s] = Some(m);
            }
            "gpmatmul" | "gpmatinv" | "gpmatpts" if self.mat[Self::slot(t, 1)?].is_none() => out.push(format!("{name} nomatrix")),
            "gpmatmul" => {
                let (s, o) = (Self::slot(t, 1)?, Self::slot(t, 2)?);
                let order = n(t, 3)?;
                let Some(b) = self.mat[o] else {
                    out.push(format!("{name} nomatrix"));
                    return Ok(true);
                };
                let a = self.mat[s].unwrap();
                // (MatrixOrderPrepend 0: the passed matrix on the left)
                let m = match order {
                    0 => mul(&b, &a),
                    1 => mul(&a, &b),
                    _ => return Err(Fail::Unmodeled("GP.MATRIX.ORDER")),
                };
                out.push("matmul 0".into());
                out.push(elements(&m));
                self.mat[s] = Some(m);
            }
            "gpmatinv" => {
                let s = Self::slot(t, 1)?;
                let m = self.mat[s].unwrap();
                match invert(&m) {
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
                let k = t.len() - 2;
                if k % 2 != 0 || k < 2 || k > 32 {
                    return Err(Fail::Bad);
                }
                let mut l = "pts 0".to_string();
                for i in 0..k / 2 {
                    let (x, y) = (f32::from_bits(n(t, 2 + 2 * i)? as u32), f32::from_bits(n(t, 3 + 2 * i)? as u32));
                    let (a, b) = transform(&m, x, y);
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
    let mut o = Bitmap { f: b.f, w, h, rows: vec![vec![0; row_bytes(b.f, w)]; h as usize], pal: b.pal.clone() };
    for y in 0..h {
        for x in 0..w {
            let x = if k >= 4 { w - 1 - x } else { x };
            // (the source pixel of destination (x, y) after the turns)
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
