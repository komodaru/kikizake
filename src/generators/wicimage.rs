//! The WIC bitmap source suites (runner/wicimage.c): predefined palettes, scalers, flip/rotate,
//! clippers and CopyPixels, color contexts and transforms, the decoders (files written here, byte
//! by byte, from the formats' own specifications) and the encoders.
use super::*;
use crate::model::wic::FORMATS;

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

fn bytes(r: &mut XorShift32, n: usize) -> Vec<u8> {
    (0..n).map(|_| r.next() as u8).collect()
}

const HEAD: &str = "Made by `kikizake gen`; runner/wicimage.c says what each line does.";

pub fn wicpalette() -> String {
    let mut s = format!("# kikizake suite wicpalette: WIC predefined palettes. {HEAD}\n");
    for t in 0..=14u32 {
        for a in 0..2 {
            writeln!(s, "case predefined-{t}-{a}\nwpalette {t} {a}\nend");
        }
    }
    writeln!(s, "case predefined-large\nwpalette 0x7fffffff 0\nend");
    // the palette of an indexed bitmap, and of one that has none
    for f in ["idx1", "idx2", "idx4", "idx8", "bgra", "gray8", "bw"] {
        writeln!(s, "case srcpal-{f}\nwbits {f} 4 2 1\nwsrcpal\nend");
    }
    s
}

const MODES: &[&str] = &["nn", "linear", "cubic", "fant", "hqcubic"];
/// formats a scaler, flip/rotator or clipper is asked of
const XFORMATS: &[&str] = &["bw", "gray4", "gray8", "gray16", "bgr565", "bgr24", "bgra", "pbgra", "rgba", "rgb48", "rgba64", "cmyk", "idx4", "idx8", "alpha8"];

pub fn wicscale() -> String {
    let mut s = format!("# kikizake suite wicscale: WIC bitmap scalers. {HEAD}\n");
    // one row of 8-bit gray, every width 1..=8 to every width 1..=17, each mode (the sampling
    // positions and weights of each mode)
    for m in MODES {
        for sw in 1..=8 {
            for dw in 1..=17 {
                writeln!(s, "case row-{m}-{sw}-{dw}\nwbits gray8 {sw} 1 ramp\nwscale {dw} 1 {m}\nwinfo\nwpixels\nend");
            }
        }
    }
    // the same of random levels (the weights and their rounding), and two levels spread over
    // many output pixels (the positions' precision)
    for (k, m) in MODES.iter().enumerate() {
        for sw in 1..=8 {
            for dw in 1..=17 {
                writeln!(s, "case rrow-{m}-{sw}-{dw}\nwbits gray8 {sw} 1 {}\nwscale {dw} 1 {m}\nwpixels\nend", 1 + k * 1000 + sw * 20 + dw);
            }
        }
        for (i, (sw, dw)) in [(2, 251), (3, 250), (2, 255), (5, 241), (251, 2), (250, 3), (64, 17), (17, 64)].into_iter().enumerate() {
            for v in 0..3 {
                writeln!(s, "case fine-{m}-{sw}-{dw}-{v}\nwbits gray8 {sw} 1 {}\nwscale {dw} 1 {m}\nwpixels\nend", 50_000 + k * 100 + i * 10 + v);
            }
        }
    }
    // random levels along columns (the vertical pass), and random pictures scaled both ways at
    // once (its order and rounding between the passes, and the channels of color formats)
    for (k, m) in MODES.iter().enumerate() {
        for sh in 1..=8 {
            for dh in 1..=17 {
                writeln!(s, "case rcol-{m}-{sh}-{dh}\nwbits gray8 1 {sh} {}\nwscale 1 {dh} {m}\nwpixels\nend", 60_000 + k * 1000 + sh * 20 + dh);
            }
        }
        // (64 random columns at once: each output row's weights solve from them)
        if *m == "cubic" {
            for sh in [2u32, 3, 4, 5, 7] {
                for dh in [1u32, 2, 3, 5, 8, 11, 16, 17] {
                    if sh != dh {
                        writeln!(s, "case vcols-{m}-{sh}-{dh}\nwbits gray8 64 {sh} {}\nwscale 64 {dh} {m}\nwpixels\nend", 80_000 + sh * 20 + dh);
                    }
                }
            }
        }
        for f in ["gray8", "bgr24", "bgra", "pbgra", "gray16", "rgb48", "rgba64", "rgba", "alpha8"] {
            for (i, (sw, sh, dw, dh)) in [(3, 4, 7, 5), (5, 3, 2, 8), (4, 4, 9, 9), (8, 6, 3, 13), (6, 2, 6, 7), (7, 7, 3, 3)].into_iter().enumerate() {
                writeln!(s, "case rgrid-{m}-{f}-{sw}x{sh}-{dw}x{dh}\nwbits {f} {sw} {sh} {}\nwscale {dw} {dh} {m}\nwpixels\nend", 70_000 + k * 100 + i);
            }
        }
    }
    // the same along columns, and in both directions at once
    for m in MODES {
        for (sh, dh) in [(3, 7), (7, 3), (5, 2), (2, 5), (4, 9)] {
            writeln!(s, "case col-{m}-{sh}-{dh}\nwbits gray8 1 {sh} ramp\nwscale 1 {dh} {m}\nwpixels\nend");
            writeln!(s, "case both-{m}-{sh}-{dh}\nwbits bgra {sh} {dh} {}\nwscale {dh} {sh} {m}\nwpixels\nend", sh * 10 + dh);
        }
    }
    // every format through every mode: the output format, and the pixels
    for (i, f) in XFORMATS.iter().enumerate() {
        for (k, m) in MODES.iter().enumerate() {
            writeln!(s, "case fmt-{f}-{m}\nwbits {f} 7 5 {}\nwscale 11 3 {m}\nwinfo\nwpixels\nend", 100 + i * 5 + k);
        }
    }
    // large ratios (Fant and high quality cubic average many pixels)
    for m in MODES {
        writeln!(s, "case down-{m}\nwbits gray8 64 1 ramp\nwscale 5 1 {m}\nwpixels\nend");
        writeln!(s, "case down2-{m}\nwbits bgra 40 30 77\nwscale 3 4 {m}\nwpixels\nend");
        writeln!(s, "case up-{m}\nwbits bgra 2 2 78\nwscale 9 9 {m}\nwpixels\nend");
        writeln!(s, "case alpha-{m}\nwbits bgra 6 6 79\nwscale 4 9 {m}\nwpixels\nend");
    }
    // sizes Windows refuses, and rectangles of a scaled picture
    for (w, h) in [(0, 4), (4, 0), (0, 0), (0x10000, 1), (1, 0x10000)] {
        writeln!(s, "case size-{w}-{h}\nwbits gray8 4 4 ramp\nwscale {w} {h} nn\nend");
    }
    // (how large a scaler Windows makes)
    for (w, h) in [(0x10000, 1), (1, 0x10000), (100_000, 1), (1, 100_000), (0x10000, 0x10000), (65535, 65535), (0x7fff_ffff, 1), (1, 0x7fff_ffff), (0xffff_ffffu32, 1)] {
        writeln!(s, "case bigsize-{w}-{h}\nwbits gray8 4 4 ramp\nwscale {w} {h} nn\nwinfo\nend");
    }
    let mut r = XorShift32::new(0x5ca1e);
    for i in 0..60 {
        let m = r.pick(MODES);
        let (sw, sh, dw, dh) = (r.range(1, 12), r.range(1, 12), r.range(1, 20), r.range(1, 20));
        let (x, y) = (r.range(0, dw - 1), r.range(0, dh - 1));
        let (w, h) = (r.range(1, dw - x), r.range(1, dh - y));
        writeln!(s, "case rect-{i}\nwbits bgr24 {sw} {sh} {}\nwscale {dw} {dh} {m}\nwpixels {x} {y} {w} {h}\nend", r.next());
    }
    // a large scaler of a mode that works in another format, and one scaled again
    for m in ["hqcubic", "fant", "linear"] {
        writeln!(s, "case bigsize-fmt-{m}\nwbits bgra 4 4 77\nwscale 4096 2048 {m}\nwinfo\nwscale 3 2 nn\nwinfo\nend");
    }
    // (HighQualityCubic of 16 bits one pass at a time: 64 random rows scaled across only, 64
    // random columns scaled down only: each pass's f32 order apart)
    for (i, (a, b)) in [(8u32, 3u32), (3, 8), (4, 9), (7, 3), (6, 7), (5, 2), (13, 4), (2, 5)].into_iter().enumerate() {
        writeln!(s, "case hq16row-{a}-{b}\nwbits gray16 {a} 64 {}\nwscale {b} 64 hqcubic\nwpixels\nend", 90_000 + i);
        writeln!(s, "case hq16col-{a}-{b}\nwbits gray16 64 {a} {}\nwscale 64 {b} hqcubic\nwpixels\nend", 90_100 + i);
    }
    s
}

pub fn wictransform() -> String {
    let mut s = format!("# kikizake suite wictransform: WIC flip/rotators, clippers, CopyPixels. {HEAD}\n");
    // every transform of every format, an odd-sized picture
    let opts = [0u32, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19, 24, 25, 26, 27];
    for (i, f) in XFORMATS.iter().enumerate() {
        for o in opts {
            writeln!(s, "case flip-{f}-{o}\nwbits {f} 5 3 {}\nwflip {o}\nwinfo\nwpixels\nend", 300 + i * 32 + o as usize);
        }
    }
    for o in [4u32, 5, 7, 32, 0x7fffffff] {
        writeln!(s, "case flip-bad-{o}\nwbits gray8 3 2 ramp\nwflip {o}\nwinfo\nend");
    }
    // (what the options outside the documented ones do to the pixels)
    for o in [4u32, 5, 6, 7, 12, 13, 20, 28, 31, 32, 33, 64, 0x80, 0x100, 0x8000_0000, 0x7fff_ffff, 0xffff_ffff] {
        writeln!(s, "case flipbad-{o}\nwbits gray8 3 2 ramp\nwflip {o}\nwinfo\nwpixels\nend");
    }
    // rectangles of a flipped picture (the rotator reads its source by rows or columns)
    let mut r = XorShift32::new(0xf11b);
    for i in 0..40 {
        let o = *r.pick(&opts);
        let f = r.pick(XFORMATS);
        let (w, h) = (r.range(1, 9), r.range(1, 9));
        let (dw, dh) = if o & 1 != 0 { (h, w) } else { (w, h) };
        let (x, y) = (r.range(0, dw - 1), r.range(0, dh - 1));
        writeln!(s, "case fliprect-{i}\nwbits {f} {w} {h} {}\nwflip {o}\nwpixels {x} {y} {} {}\nend", r.next(), r.range(1, dw - x), r.range(1, dh - y));
    }
    // clippers: inside, at the edges, outside, empty
    for (i, f) in XFORMATS.iter().enumerate() {
        writeln!(s, "case clip-{f}\nwbits {f} 9 7 {}\nwclip 1 2 5 4\nwinfo\nwpixels\nwpixels 1 1 3 2\nend", 900 + i);
    }
    let rects: &[(i64, i64, i64, i64)] = &[(0, 0, 6, 5), (5, 4, 1, 1), (0, 0, 0, 0), (0, 0, 7, 5), (0, 0, 6, 6), (-1, 0, 2, 2), (0, -1, 2, 2), (6, 0, 1, 1), (2, 2, 0, 3), (3, 3, 4, 4), (0, 0, 0x7fffffff, 1)];
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        writeln!(s, "case cliprect-{i}\nwbits bgra 6 5 7\nwclip {x} {y} {w} {h}\nwinfo\nwpixels\nend");
    }
    // CopyPixels: rectangles, strides and buffers Windows takes or refuses
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        writeln!(s, "case copyrect-{i}\nwbits bgr24 6 5 8\nwpixels {x} {y} {w} {h}\nend");
    }
    for (f, fi) in [("bw", crate::wic_fmt!("bw")), ("gray4", crate::wic_fmt!("gray4")), ("bgr24", crate::wic_fmt!("bgr24")), ("rgba64", crate::wic_fmt!("rgba64"))] {
        let st = crate::model::wic::stride(fi, 5) as i64;
        for (k, (stride, size)) in [(st, st * 3), (st - 1, st * 3), (st + 3, (st + 3) * 3), (st + 3, (st + 3) * 2 + st), (st + 3, (st + 3) * 2 + st - 1), (st, st * 3 - 1), (0, 1000), (st, 0)].into_iter().enumerate() {
            writeln!(s, "case stride-{f}-{k}\nwbits {f} 5 3 3\nwpixels 0 0 5 3 {stride} {size}\nend");
        }
    }
    // chains: scale, flip, clip, convert in turn
    let mut r = XorShift32::new(0xc4a1);
    for i in 0..40 {
        let f = r.pick(&["bgra", "bgr24", "gray8", "idx8", "rgba64"]);
        let (w, h) = (r.range(2, 9), r.range(2, 9));
        let mut l = format!("case chain-{i}\nwbits {f} {w} {h} {}\n", r.next());
        let (mut cw, mut ch) = (w, h);
        for _ in 0..3 {
            match r.range(0, 3) {
                0 => {
                    let o = *r.pick(&opts);
                    l += &format!("wflip {o}\n");
                    if o & 1 != 0 {
                        (cw, ch) = (ch, cw);
                    }
                }
                1 => {
                    let (x, y) = (r.range(0, cw - 1), r.range(0, ch - 1));
                    let (nw, nh) = (r.range(1, cw - x), r.range(1, ch - y));
                    l += &format!("wclip {x} {y} {nw} {nh}\n");
                    (cw, ch) = (nw, nh);
                }
                2 => {
                    let (nw, nh) = (r.range(1, 12), r.range(1, 12));
                    l += &format!("wscale {nw} {nh} nn\n");
                    (cw, ch) = (nw, nh);
                }
                _ => l += &format!("wconv {}\n", r.pick(&["bgra", "bgr24", "gray8", "rgba64", "pbgra"])),
            }
        }
        s += &l;
        s += "winfo\nwpixels\nend\n";
    }
    // a decoded picture of unequal resolutions turned: which resolution goes with which side
    let f = bmp(40, 3, 2, 24, 0, [0; 4], 0, &[], &[1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 0, 0, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 0], false, (2835, 5906));
    for o in [0, 1, 2, 3, 9, 11] {
        writeln!(s, "case flip-dpi-{o}\nwdecode {}\nwframe 0\nwflip {o}\nwinfo\nwpixels\nend", hex(&f));
    }
    s
}

pub fn wiccolor() -> String {
    let mut s = format!("# kikizake suite wiccolor: WIC color contexts and transforms. {HEAD}\n");
    for n in [0u32, 1, 2, 3, 4, 5, 100, 65535, 2147483647] {
        writeln!(s, "case ctx-{n}\nwctx {n}\nend");
    }
    let srcs = ["bgra", "bgr24", "rgb24", "gray8", "gray16", "pbgra", "rgba", "bgr565", "rgba64", "cmyk", "idx8", "bw"];
    let dsts = ["bgra", "bgr24", "gray8", "pbgra", "rgba64", "cmyk", "rgb48", "idx8"];
    for (i, f) in srcs.iter().enumerate() {
        for (k, t) in dsts.iter().enumerate() {
            for (a, b) in [(1, 1), (1, 2), (2, 1)] {
                writeln!(s, "case xf-{f}-{t}-{a}{b}\nwbits {f} 8 2 {}\nwcolor {a} {b} {t}\nwinfo\nwpixels\nend", 1 + i * 64 + k * 4 + a * 2 + b);
            }
        }
    }
    for (a, b) in [(0, 1), (1, 0), (3, 1), (1, 3)] {
        writeln!(s, "case xf-bad-{a}{b}\nwbits bgra 2 2 1\nwcolor {a} {b} bgra\nend");
    }
    // the transforms' curves and mixing: ramps of each primary, gray, and random colors, from a
    // 24-bit BMP, into 24bppBGR and 64bppRGBA (16 bits show the arithmetic), between every pair
    // of contexts and the values InitializeFromExifColorSpace takes besides 1 and 2
    let mut r = XorShift32::new(0xc01a);
    let mut ramps: Vec<(&str, Vec<[u8; 3]>)> = vec![];
    for (k, name) in ["b", "g", "r", "gray"].iter().enumerate() {
        ramps.push((name, (0..=255u8).map(|v| if k == 3 { [v; 3] } else { let mut c = [0; 3]; c[k] = v; c }).collect()));
    }
    ramps.push(("rand", (0..1024).map(|_| [r.next() as u8, r.next() as u8, r.next() as u8]).collect()));
    for (name, px) in &ramps {
        let w = 256u32;
        let h = px.len() as u32 / w;
        let rows: Vec<u8> = (0..h as usize).rev().flat_map(|y| px[y * 256..y * 256 + 256].iter().flat_map(|c| c.iter().copied()).collect::<Vec<_>>()).collect();
        let f = bmp(40, w as i32, h as i32, 24, 0, [0; 4], 0, &[], &rows, false, (3780, 3780));
        for (a, b) in [(1, 1), (1, 2), (2, 1), (2, 2), (0, 1), (1, 0), (3, 2), (65535, 1)] {
            for t in ["bgr24", "rgba64"] {
                writeln!(s, "case cm-{name}-{a}{b}-{t}\nwdecode {}\nwframe 0\nwcolor {a} {b} {t}\nwpixels\nend", hex(&f));
            }
        }
    }
    // every 8-bit level through sRGB to Adobe RGB and back
    writeln!(s, "case levels-12\nwbits bgr24 256 1 ramp\nwcolor 1 2 bgr24\nwpixels\nend");
    writeln!(s, "case levels-21\nwbits bgr24 256 1 ramp\nwcolor 2 1 bgr24\nwpixels\nend");
    // the profiles the transforms are made of (sRGB's, and the CMYK one that 32bppCMYK goes through)
    writeln!(s, "case profile-srgb\nwprofile srgb\nend");
    writeln!(s, "case profile-swop\nwprofile swop\nend");
    // (the tables of RSWOP.icm the CMYK transforms go through: read into the suite by the
    // user's choice, 2026-11-21)
    for t in ["A2B0", "A2B1", "A2B2", "B2A0", "B2A1", "B2A2", "gamt"] {
        writeln!(s, "case profile-swop-{t}\nwprofile swop {t}\nend");
    }
    s
}

/* ---------------------------------------------------------------------------------------- */
/* files, written from their formats' specifications */

fn le16(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&(x as u16).to_le_bytes());
}
fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}

/// a BMP: `hdr` the header size (12 core, 40 info, 108 v4, 124 v5), `bpp`, `comp` (0 RGB, 1 RLE8,
/// 2 RLE4, 3 bitfields), masks for bitfields (and alpha for v4/v5), `top` for a top-down
/// picture, `colors` palette entries written (biClrUsed), the pixel bytes as they are stored
#[allow(clippy::too_many_arguments)]
pub fn bmp(hdr: u32, w: i32, h: i32, bpp: u32, comp: u32, masks: [u32; 4], colors: u32, pal: &[u32], px: &[u8], top: bool, ppm: (u32, u32)) -> Vec<u8> {
    let entry = if hdr == 12 { 3 } else { 4 };
    let masks_extra = if hdr == 40 && comp == 3 { 12 } else { 0 };
    let off = 14 + hdr + masks_extra + entry * pal.len() as u32;
    let mut v = Vec::new();
    v.extend_from_slice(b"BM");
    le32(&mut v, off + px.len() as u32);
    le32(&mut v, 0);
    le32(&mut v, off);
    le32(&mut v, hdr);
    if hdr == 12 {
        le16(&mut v, w as u32);
        le16(&mut v, h as u32);
        le16(&mut v, 1);
        le16(&mut v, bpp);
    } else {
        le32(&mut v, w as u32);
        le32(&mut v, if top { (-h) as u32 } else { h as u32 });
        le16(&mut v, 1);
        le16(&mut v, bpp);
        le32(&mut v, comp);
        le32(&mut v, px.len() as u32);
        le32(&mut v, ppm.0);
        le32(&mut v, ppm.1);
        le32(&mut v, colors);
        le32(&mut v, 0);
        if hdr == 40 && comp == 3 {
            for m in &masks[..3] {
                le32(&mut v, *m);
            }
        }
        if hdr >= 108 {
            for m in masks {
                le32(&mut v, m);
            }
            le32(&mut v, 0x7352_4742); // LCS_sRGB
            v.resize(v.len() + 36 + 12, 0);
            if hdr == 124 {
                le32(&mut v, 4); // LCS_GM_IMAGES
                le32(&mut v, 0);
                le32(&mut v, 0);
                le32(&mut v, 0);
            }
        }
    }
    for c in pal {
        v.extend_from_slice(&c.to_le_bytes()[..entry as usize]);
    }
    v.extend_from_slice(px);
    v
}

use crate::model::wicimage::{adler32, crc32};

/// zlib, deflate with stored blocks of at most `block` bytes (RFC 1950, 1951 3.2.4)
pub fn zlib_stored(data: &[u8], block: usize) -> Vec<u8> {
    let mut v = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() { vec![&[]] } else { data.chunks(block).collect() };
    for (i, c) in chunks.iter().enumerate() {
        v.push((i + 1 == chunks.len()) as u8);
        le16(&mut v, c.len() as u32);
        le16(&mut v, !(c.len() as u32) & 0xffff);
        v.extend_from_slice(c);
    }
    v.extend_from_slice(&adler32(data).to_be_bytes());
    v
}

fn chunk(v: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    v.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let mut c = name.to_vec();
    c.extend_from_slice(data);
    v.extend_from_slice(&c);
    v.extend_from_slice(&crc32(&c).to_be_bytes());
}

/// a PNG (ISO/IEC 15948): `rows` the filtered scanlines (each with its filter byte, Adam7's
/// passes in turn when interlaced), the extra chunks before IDAT
pub fn png(w: u32, h: u32, depth: u8, ctype: u8, interlace: bool, extra: &[(&[u8; 4], Vec<u8>)], filtered: &[u8], block: usize) -> Vec<u8> {
    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[depth, ctype, 0, 0, interlace as u8]);
    chunk(&mut v, b"IHDR", &ihdr);
    for (n, d) in extra {
        chunk(&mut v, n, d);
    }
    let z = zlib_stored(filtered, block);
    // (IDAT split in two: a decoder joins them)
    let half = z.len() / 2;
    chunk(&mut v, b"IDAT", &z[..half]);
    chunk(&mut v, b"IDAT", &z[half..]);
    chunk(&mut v, b"IEND", &[]);
    v
}

pub fn png_channels(ctype: u8) -> u32 {
    match ctype {
        0 | 3 => 1,
        2 => 3,
        4 => 2,
        _ => 4,
    }
}

/// Adam7 passes: (x0, y0, dx, dy)
pub const ADAM7: [(u32, u32, u32, u32); 7] = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i32 + b as i32 - c as i32;
    let (pa, pb, pc) = ((p - a as i32).abs(), (p - b as i32).abs(), (p - c as i32).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

/// raw scanlines (no filter bytes, `bpr` bytes each) filtered with the given filter types
pub fn png_filter(raw: &[Vec<u8>], bpp: usize, filters: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut prev: Vec<u8> = vec![0; raw.first().map_or(0, |r| r.len())];
    for (y, row) in raw.iter().enumerate() {
        let f = filters[y % filters.len()];
        out.push(f);
        for i in 0..row.len() {
            let a = if i >= bpp { row[i - bpp] } else { 0 };
            let b = prev[i];
            let c = if i >= bpp { prev[i - bpp] } else { 0 };
            out.push(row[i].wrapping_sub(match f {
                0 => 0,
                1 => a,
                2 => b,
                3 => ((a as u32 + b as u32) / 2) as u8,
                _ => paeth(a, b, c),
            }));
        }
        prev = row.clone();
    }
    out
}

/// GIF's variable-length LZW (GIF89a appendix F, as giflib writes it), as data sub-blocks
pub fn gif_lzw(min: u32, idx: &[u8]) -> Vec<u8> {
    let clear = 1u32 << min;
    let mut bits = Vec::<u8>::new();
    let (mut acc, mut nacc) = (0u32, 0u32);
    let mut emit = |code: u32, size: u32, bits: &mut Vec<u8>| {
        acc |= code << nacc;
        nacc += size;
        while nacc >= 8 {
            bits.push(acc as u8);
            acc >>= 8;
            nacc -= 8;
        }
    };
    let mut dict: std::collections::HashMap<(u32, u8), u32> = Default::default();
    let mut next = clear + 2;
    let mut size = min + 1;
    emit(clear, size, &mut bits);
    let mut cur: Option<u32> = None;
    for &p in idx {
        let Some(c) = cur else {
            cur = Some(p as u32);
            continue;
        };
        if let Some(&n) = dict.get(&(c, p)) {
            cur = Some(n);
            continue;
        }
        emit(c, size, &mut bits);
        // (giflib's order: the code goes out, the size grows once the next code needs it, then
        // the new string gets its code)
        if next >= 1 << size && size < 12 {
            size += 1;
        }
        if next >= 4095 {
            emit(clear, size, &mut bits);
            dict.clear();
            next = clear + 2;
            size = min + 1;
        } else {
            dict.insert((c, p), next);
            next += 1;
        }
        cur = Some(p as u32);
    }
    if let Some(c) = cur {
        emit(c, size, &mut bits);
    }
    emit(clear + 1, size, &mut bits);
    if nacc > 0 {
        bits.push(acc as u8);
    }
    let mut v = vec![min as u8];
    for c in bits.chunks(255) {
        v.push(c.len() as u8);
        v.extend_from_slice(c);
    }
    v.push(0);
    v
}

pub struct GifFrame {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub local: Option<Vec<u32>>,
    pub transparent: Option<u8>,
    pub interlace: bool,
    pub idx: Vec<u8>,
    pub min: u32,
}

pub fn gif(w: u32, h: u32, global: Option<&[u32]>, bg: u8, frames: &[GifFrame]) -> Vec<u8> {
    let table_bits = |n: usize| (1..=8).find(|b| 1usize << b >= n).unwrap_or(8);
    let put_table = |v: &mut Vec<u8>, t: &[u32]| {
        let n = 1usize << table_bits(t.len());
        for i in 0..n {
            let c = t.get(i).copied().unwrap_or(0);
            v.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]);
        }
    };
    let mut v = b"GIF89a".to_vec();
    le16(&mut v, w);
    le16(&mut v, h);
    v.push(global.map_or(0, |g| 0x80 | 0x70 | (table_bits(g.len()) as u8 - 1)));
    v.push(bg);
    v.push(0);
    if let Some(g) = global {
        put_table(&mut v, g);
    }
    for f in frames {
        if let Some(t) = f.transparent {
            v.extend_from_slice(&[0x21, 0xf9, 4, 1 | 4, 10, 0, t, 0]);
        }
        v.push(0x2c);
        le16(&mut v, f.x);
        le16(&mut v, f.y);
        le16(&mut v, f.w);
        le16(&mut v, f.h);
        v.push(f.local.as_ref().map_or(0, |l| 0x80 | (table_bits(l.len()) as u8 - 1)) | if f.interlace { 0x40 } else { 0 });
        if let Some(l) = &f.local {
            put_table(&mut v, l);
        }
        // (interlaced: rows stored in pass order 0/8, 4/8, 2/4, 1/2)
        let rows: Vec<u32> = if f.interlace { [(0, 8), (4, 8), (2, 4), (1, 2)].iter().flat_map(|&(s, d)| (s..f.h).step_by(d)).collect() } else { (0..f.h).collect() };
        let stored: Vec<u8> = rows.iter().flat_map(|&y| f.idx[(y * f.w) as usize..((y + 1) * f.w) as usize].to_vec()).collect();
        v.extend(gif_lzw(f.min, &stored));
    }
    v.push(0x3b);
    v
}

/// a baseline TIFF (TIFF 6.0): one IFD per image, uncompressed strips of `rps` rows.
/// Each image: (w, h, photometric, bits per sample, samples, extra samples, planar, pixel bytes
/// as stored (planar 2: each plane in turn), palette (16-bit RGB) for photometric 3)
#[derive(Clone)]
pub struct TiffImage {
    pub w: u32,
    pub h: u32,
    pub photometric: u16,
    pub bits: u16,
    pub samples: u16,
    pub extra: Option<u16>,
    pub planar: u16,
    pub rps: u32,
    pub px: Vec<u8>,
    pub map: Vec<u16>,
    pub res: Option<(u32, u32, u16)>,
    pub format: u16,
}

pub fn tiff(big: bool, images: &[TiffImage]) -> Vec<u8> {
    let w16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&if big { x.to_be_bytes() } else { x.to_le_bytes() });
    let w32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&if big { x.to_be_bytes() } else { x.to_le_bytes() });
    let mut v: Vec<u8> = if big { b"MM\0\x2a".to_vec() } else { b"II\x2a\0".to_vec() };
    w32(&mut v, 0); // first IFD, patched
    let mut link = 4usize;
    for im in images {
        // the data first: pixels, then the arrays the tags point to
        let row = (im.w * im.bits as u32 * if im.planar == 2 { 1 } else { im.samples as u32 }).div_ceil(8) as usize;
        let planes = if im.planar == 2 { im.samples as usize } else { 1 };
        let mut offs = Vec::new();
        let mut counts = Vec::new();
        for p in 0..planes {
            for y0 in (0..im.h).step_by(im.rps.max(1) as usize) {
                let n = (im.rps.min(im.h - y0)) as usize * row;
                let start = p * row * im.h as usize + y0 as usize * row;
                if v.len() % 2 == 1 {
                    v.push(0);
                }
                offs.push(v.len() as u32);
                counts.push(n as u32);
                v.extend_from_slice(&im.px[start..start + n]);
            }
        }
        let arr = |v: &mut Vec<u8>, vals: &[u32], short: bool| -> u32 {
            if v.len() % 2 == 1 {
                v.push(0);
            }
            let o = v.len() as u32;
            for &x in vals {
                if short {
                    w16(v, x as u16)
                } else {
                    w32(v, x)
                }
            }
            o
        };
        // tags: (tag, type, count, values)
        let mut tags: Vec<(u16, u16, Vec<u32>)> = vec![
            (256, 4, vec![im.w]),
            (257, 4, vec![im.h]),
            (258, 3, vec![im.bits as u32; im.samples as usize]),
            (259, 3, vec![1]),
            (262, 3, vec![im.photometric as u32]),
            (273, 4, offs.clone()),
            (277, 3, vec![im.samples as u32]),
            (278, 4, vec![im.rps]),
            (279, 4, counts.clone()),
            (284, 3, vec![im.planar as u32]),
        ];
        if let Some((x, y, unit)) = im.res {
            let rx = arr(&mut v, &[x, 1], false);
            let ry = arr(&mut v, &[y, 1], false);
            tags.push((282, 5, vec![rx]));
            tags.push((283, 5, vec![ry]));
            tags.push((296, 3, vec![unit as u32]));
        }
        if !im.map.is_empty() {
            tags.push((320, 3, im.map.iter().map(|&x| x as u32).collect()));
        }
        if let Some(e) = im.extra {
            tags.push((338, 3, vec![e as u32]));
        }
        if im.format != 1 {
            tags.push((339, 3, vec![im.format as u32; im.samples as usize]));
        }
        tags.sort_by_key(|t| t.0);
        // values that do not fit in 4 bytes go out of line
        let mut entries = Vec::new();
        for (tag, ty, vals) in &tags {
            let size = if *ty == 3 { 2 } else { 4 } * vals.len();
            if *ty == 5 {
                entries.push((*tag, *ty, 1u32, vals[0], false));
            } else if size > 4 {
                let o = arr(&mut v, vals, *ty == 3);
                entries.push((*tag, *ty, vals.len() as u32, o, false));
            } else {
                entries.push((*tag, *ty, vals.len() as u32, 0, true));
            }
        }
        if v.len() % 2 == 1 {
            v.push(0);
        }
        let ifd = v.len() as u32;
        let patch = if big { ifd.to_be_bytes() } else { ifd.to_le_bytes() };
        v[link..link + 4].copy_from_slice(&patch);
        w16(&mut v, entries.len() as u16);
        for (k, (tag, ty, count, off, inline)) in entries.iter().enumerate() {
            w16(&mut v, *tag);
            w16(&mut v, *ty);
            w32(&mut v, *count);
            if *inline {
                let vals = &tags[k].2;
                let mut b = Vec::new();
                for &x in vals {
                    if *ty == 3 {
                        w16(&mut b, x as u16)
                    } else {
                        w32(&mut b, x)
                    }
                }
                b.resize(4, 0);
                v.extend_from_slice(&b);
            } else {
                w32(&mut v, *off);
            }
        }
        link = v.len();
        w32(&mut v, 0);
    }
    v
}

/// an icon file: entries (width, height (0 = 256), the image: a BMP without its file header
/// (height doubled, XOR then AND mask) or a whole PNG)
pub fn ico(entries: &[(u32, u32, u32, Vec<u8>)]) -> Vec<u8> {
    let mut v = vec![0, 0, 1, 0];
    le16(&mut v, entries.len() as u32);
    let mut off = 6 + 16 * entries.len() as u32;
    for (w, h, bpp, d) in entries {
        v.extend_from_slice(&[*w as u8, *h as u8, 0, 0]);
        le16(&mut v, 1);
        le16(&mut v, *bpp);
        le32(&mut v, d.len() as u32);
        le32(&mut v, off);
        off += d.len() as u32;
    }
    for e in entries {
        v.extend_from_slice(&e.3);
    }
    v
}

/// a DDS file: the header (DDS_HEADER) for a FourCC (DXT1/3/5) or an uncompressed 32-bit format,
/// `mips` levels, the data as given
pub fn dds(w: u32, h: u32, mips: u32, fourcc: Option<&[u8; 4]>, data: &[u8]) -> Vec<u8> {
    let mut v = b"DDS ".to_vec();
    le32(&mut v, 124);
    le32(&mut v, 0x1 | 0x2 | 0x4 | 0x1000 | if mips > 1 { 0x20000 } else { 0 } | if fourcc.is_some() { 0x80000 } else { 0x8 });
    le32(&mut v, h);
    le32(&mut v, w);
    let pitch = match fourcc {
        Some(b"DXT1") => w.div_ceil(4).max(1) * 8 * h.div_ceil(4).max(1),
        Some(_) => w.div_ceil(4).max(1) * 16 * h.div_ceil(4).max(1),
        None => w * 4,
    };
    le32(&mut v, pitch);
    le32(&mut v, 0);
    le32(&mut v, mips);
    v.resize(v.len() + 44, 0);
    // DDS_PIXELFORMAT
    le32(&mut v, 32);
    match fourcc {
        Some(f) => {
            le32(&mut v, 0x4);
            v.extend_from_slice(f);
            v.resize(v.len() + 20, 0);
        }
        None => {
            le32(&mut v, 0x41);
            le32(&mut v, 0);
            le32(&mut v, 32);
            le32(&mut v, 0x00ff_0000);
            le32(&mut v, 0x0000_ff00);
            le32(&mut v, 0x0000_00ff);
            le32(&mut v, 0xff00_0000);
        }
    }
    le32(&mut v, 0x1000 | if mips > 1 { 0x400008 } else { 0 });
    v.resize(v.len() + 16, 0);
    v.extend_from_slice(data);
    v
}

fn rand_pal(r: &mut XorShift32, n: usize) -> Vec<u32> {
    (0..n).map(|_| r.next() & 0xffffff).collect()
}

/// the decoder cases: each a file and what is asked of it
fn decode_case(s: &mut String, name: &str, file: &[u8], frames: u32, pal: bool) {
    writeln!(s, "case {name}\nwdecode {}", hex(file));
    for f in 0..frames {
        writeln!(s, "wframe {f}\nwinfo\nwpixels");
        if pal {
            s.push_str("wsrcpal\n");
        }
    }
    s.push_str("end\n");
}

pub fn wicdecode() -> String {
    let mut s = format!("# kikizake suite wicdecode: the WIC decoders, on files written byte by byte. {HEAD}\n");
    let mut r = XorShift32::new(0xdec0de);
    // BMP: every header, depth, compression and row order
    for hdr in [12u32, 40, 108, 124] {
        for bpp in [1u32, 4, 8, 16, 24, 32] {
            if hdr == 12 && bpp == 16 || hdr == 12 && bpp == 32 {
                continue;
            }
            for top in [false, true] {
                if top && hdr == 12 {
                    continue;
                }
                let (w, h) = (r.range(1, 9) as i32, r.range(1, 5) as i32);
                let stride = (w as u32 * bpp).div_ceil(32) * 4;
                let px = bytes(&mut r, (stride * h as u32) as usize);
                let n = if bpp <= 8 { 1usize << bpp } else { 0 };
                let pal = rand_pal(&mut r, n);
                let f = bmp(hdr, w, h, bpp, 0, [0; 4], 0, &pal, &px, top, (3780, 2835));
                decode_case(&mut s, &format!("bmp-{hdr}-{bpp}-{}", if top { "td" } else { "bu" }), &f, 1, bpp <= 8);
            }
        }
    }
    // BMP bit fields: 16 and 32 bits, the usual masks and others
    let fields: &[(u32, [u32; 4])] = &[
        (16, [0x7c00, 0x03e0, 0x001f, 0]),
        (16, [0xf800, 0x07e0, 0x001f, 0]),
        (16, [0x0f00, 0x00f0, 0x000f, 0xf000]),
        (16, [0x001f, 0x07e0, 0xf800, 0]),
        (32, [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0]),
        (32, [0x00ff_0000, 0x0000_ff00, 0x0000_00ff, 0xff00_0000]),
        (32, [0x3ff0_0000, 0x000f_fc00, 0x0000_03ff, 0]),
        (32, [0x0000_00ff, 0x0000_ff00, 0x00ff_0000, 0xff00_0000]),
    ];
    for hdr in [40u32, 108, 124] {
        for (k, (bpp, m)) in fields.iter().enumerate() {
            let (w, h) = (r.range(1, 7) as i32, r.range(1, 4) as i32);
            let stride = (w as u32 * bpp).div_ceil(32) * 4;
            let px = bytes(&mut r, (stride * h as u32) as usize);
            let f = bmp(hdr, w, h, *bpp, 3, *m, 0, &[], &px, false, (0, 0));
            decode_case(&mut s, &format!("bmpfields-{hdr}-{k}"), &f, 1, false);
        }
    }
    // BMP: a short palette (biClrUsed), 32 bits with alpha in a v5 header with no mask, RLE
    for colors in [1u32, 3, 7] {
        let px = bytes(&mut r, 8);
        let pal = rand_pal(&mut r, colors as usize);
        let f = bmp(40, 8, 2, 4, 0, [0; 4], colors, &pal, &px, false, (0, 0));
        decode_case(&mut s, &format!("bmppal-{colors}"), &f, 1, true);
    }
    for (k, rle) in [
        vec![3u8, 1, 2, 7, 0, 0, 0, 3, 9, 8, 7, 0, 0, 1],                          // runs, an absolute run, end of line, end of bitmap
        vec![2, 5, 0, 2, 2, 1, 3, 6, 0, 1],                                        // a delta
        vec![8, 4, 0, 1],                                                          // a short picture
        vec![0, 4, 1, 2, 3, 4, 0, 0, 4, 9, 0, 0, 0, 1],                            // absolute, then runs
    ]
    .into_iter()
    .enumerate()
    {
        let pal = rand_pal(&mut r, 16);
        decode_case(&mut s, &format!("bmprle8-{k}"), &bmp(40, 8, 3, 8, 1, [0; 4], 16, &pal, &rle, false, (0, 0)), 1, false);
        decode_case(&mut s, &format!("bmprle4-{k}"), &bmp(40, 8, 3, 4, 2, [0; 4], 16, &pal, &rle, false, (0, 0)), 1, false);
    }
    // broken BMPs
    let good = bmp(40, 4, 2, 24, 0, [0; 4], 0, &[], &bytes(&mut r, 24), false, (0, 0));
    for (k, cut) in [10usize, 30, 54, 60].into_iter().enumerate() {
        decode_case(&mut s, &format!("bmpshort-{k}"), &good[..cut], 1, false);
    }
    // PNG: every color type and depth, both filters' worth of rows, with and without Adam7
    let types: &[(u8, &[u8])] = &[(0, &[1, 2, 4, 8, 16]), (2, &[8, 16]), (3, &[1, 2, 4, 8]), (4, &[8, 16]), (6, &[8, 16])];
    for &(ct, depths) in types {
        for &d in depths {
            for il in [false, true] {
                let (w, h) = (r.range(1, 11) as u32, r.range(1, 9) as u32);
                let bpp_bits = d as u32 * png_channels(ct);
                let filters: Vec<u8> = (0..5).map(|_| r.range(0, 4) as u8).collect();
                let passes: Vec<(u32, u32)> = if il {
                    ADAM7.iter().map(|&(x0, y0, dx, dy)| ((w + dx - 1 - x0.min(w)) / dx * (x0 < w) as u32, (h + dy - 1 - y0.min(h)) / dy * (y0 < h) as u32)).collect()
                } else {
                    vec![(w, h)]
                };
                let mut filtered = Vec::new();
                for (pw, ph) in passes {
                    if pw == 0 || ph == 0 {
                        continue;
                    }
                    let bpr = (pw * bpp_bits).div_ceil(8) as usize;
                    let raw: Vec<Vec<u8>> = (0..ph).map(|_| bytes(&mut r, bpr)).collect();
                    filtered.extend(png_filter(&raw, (bpp_bits as usize).div_ceil(8), &filters));
                }
                let mut extra: Vec<(&[u8; 4], Vec<u8>)> = Vec::new();
                if ct == 3 {
                    let n = r.range(1, 1 << d.min(8)) as usize;
                    extra.push((b"PLTE", bytes(&mut r, n * 3)));
                    if r.chance(2) {
                        let k = r.range(1, n as i64) as usize;
                        extra.push((b"tRNS", bytes(&mut r, k)));
                    }
                }
                if (ct == 0 || ct == 2) && r.chance(2) {
                    extra.push((b"tRNS", bytes(&mut r, if ct == 0 { 2 } else { 6 })));
                }
                if r.chance(2) {
                    let mut p = Vec::new();
                    p.extend_from_slice(&(r.range(1, 20000) as u32).to_be_bytes());
                    p.extend_from_slice(&(r.range(1, 20000) as u32).to_be_bytes());
                    p.push(r.range(0, 1) as u8);
                    extra.push((b"pHYs", p));
                }
                let f = png(w, h, d, ct, il, &extra, &filtered, r.range(7, 300) as usize);
                decode_case(&mut s, &format!("png-{ct}-{d}-{}", if il { "adam7" } else { "plain" }), &f, 1, ct == 3);
            }
        }
    }
    // PNG with a palette index past the palette, a bad CRC, a cut file
    {
        let filtered = png_filter(&[vec![0, 1, 2, 3, 200, 255]], 1, &[0]);
        let f = png(6, 1, 8, 3, false, &[(b"PLTE", vec![1, 2, 3, 4, 5, 6, 7, 8, 9])], &filtered, 64);
        decode_case(&mut s, "png-index-outside", &f, 1, true);
        let mut bad = f.clone();
        let n = bad.len();
        bad[n - 20] ^= 1;
        decode_case(&mut s, "png-bad-crc", &bad, 1, false);
        decode_case(&mut s, "png-short", &f[..f.len() - 30], 1, false);
    }
    // GIF: global and local tables, transparency, interlace, frames placed inside the screen
    for k in 0..12 {
        let (w, h) = (r.range(2, 12) as u32, r.range(2, 8) as u32);
        let gl = if k % 3 != 2 { { let n = r.range(2, 256) as usize; Some(rand_pal(&mut r, n)) } } else { None };
        let nf = if k % 2 == 0 { 1 } else { r.range(2, 3) as u32 };
        let mut frames = Vec::new();
        for _ in 0..nf {
            let (fw, fh) = (r.range(1, w as i64) as u32, r.range(1, h as i64) as u32);
            // (a local table: always without a global one, else one frame in three)
            let (local, n) = match &gl {
                Some(g) if !r.chance(3) => (None, g.len()),
                _ => {
                    let n = r.range(2, 64) as usize;
                    let p = rand_pal(&mut r, n);
                    let len = p.len();
                    (Some(p), len)
                }
            };
            // (the bits of an index: the least of 1..=8 whose 2^bits colors hold the table's n)
            let bits = n.next_power_of_two().trailing_zeros().max(1);
            let min = bits.max(2);
            let idx: Vec<u8> = (0..fw * fh).map(|_| (r.next() as usize % (1 << bits)) as u8).collect();
            frames.push(GifFrame { x: r.range(0, (w - fw) as i64) as u32, y: r.range(0, (h - fh) as i64) as u32, w: fw, h: fh, local, transparent: if r.chance(2) { Some(r.range(0, n as i64 - 1) as u8) } else { None }, interlace: r.chance(3), idx, min });
        }
        let f = gif(w, h, gl.as_deref(), 0, &frames);
        decode_case(&mut s, &format!("gif-{k}"), &f, nf, true);
    }
    // TIFF: bilevel, gray, palette, RGB, RGBA (both alphas), CMYK, 16 bits, planar, both orders
    let timages: &[(u16, u16, u16, Option<u16>, u16)] = &[
        (0, 1, 1, None, 1), (1, 1, 1, None, 1), (1, 4, 1, None, 1), (1, 8, 1, None, 1), (0, 8, 1, None, 1), (1, 16, 1, None, 1),
        (3, 1, 1, None, 1), (3, 4, 1, None, 1), (3, 8, 1, None, 1), (2, 8, 3, None, 1), (2, 8, 3, None, 2), (2, 16, 3, None, 1),
        (2, 8, 4, Some(1), 1), (2, 8, 4, Some(2), 1), (2, 16, 4, Some(2), 1), (2, 16, 4, Some(1), 1), (5, 8, 4, None, 1), (5, 16, 4, None, 1),
        (1, 8, 2, Some(2), 1), (2, 8, 4, Some(0), 1), (5, 8, 5, Some(2), 1),
    ];
    for (k, &(ph, bits, samples, extra, planar)) in timages.iter().enumerate() {
        for big in [false, true] {
            let (w, h) = (r.range(1, 9) as u32, r.range(1, 6) as u32);
            let row = if planar == 2 { (w * bits as u32).div_ceil(8) } else { (w * bits as u32 * samples as u32).div_ceil(8) } as usize;
            let total = row * h as usize * if planar == 2 { samples as usize } else { 1 };
            let map: Vec<u16> = if ph == 3 { (0..3 << bits).map(|_| r.next() as u16).collect() } else { vec![] };
            let im = TiffImage { w, h, photometric: ph, bits, samples, extra, planar, rps: r.range(1, h as i64) as u32, px: bytes(&mut r, total), map, res: if r.chance(2) { Some((r.range(1, 600) as u32, r.range(1, 600) as u32, r.range(1, 3) as u16)) } else { None }, format: 1 };
            let f = tiff(big, &[im]);
            decode_case(&mut s, &format!("tiff-{k}-{}", if big { "mm" } else { "ii" }), &f, 1, ph == 3);
        }
    }
    // TIFF: several pages, float samples
    {
        let mk = |r: &mut XorShift32, w: u32, h: u32| TiffImage { w, h, photometric: 2, bits: 8, samples: 3, extra: None, planar: 1, rps: h, px: bytes(r, (w * h * 3) as usize), map: vec![], res: None, format: 1 };
        let pages = [mk(&mut r, 3, 2), mk(&mut r, 5, 4), mk(&mut r, 1, 1)];
        decode_case(&mut s, "tiff-pages", &tiff(false, &pages), 3, false);
        let fl: Vec<u8> = [0.0f32, 0.25, 0.5, 1.0, 2.0, -1.0].iter().flat_map(|f| f.to_le_bytes()).collect();
        let im = TiffImage { w: 6, h: 1, photometric: 1, bits: 32, samples: 1, extra: None, planar: 1, rps: 1, px: fl, map: vec![], res: None, format: 3 };
        decode_case(&mut s, "tiff-float-gray", &tiff(false, &[im]), 1, false);
    }
    // ICO: BMP entries of each depth (XOR and AND masks), PNG entries, several sizes in one file
    let icon_bmp = |r: &mut XorShift32, w: u32, h: u32, bpp: u32| -> Vec<u8> {
        let stride = (w * bpp).div_ceil(32) * 4;
        let mstride = w.div_ceil(32) * 4;
        let n = if bpp <= 8 { 1usize << bpp } else { 0 };
        let pal = rand_pal(r, n);
        let mut px = bytes(r, (stride * h) as usize);
        px.extend(bytes(r, (mstride * h) as usize));
        bmp(40, w as i32, (2 * h) as i32, bpp, 0, [0; 4], 0, &pal, &px, false, (0, 0))[14..].to_vec()
    };
    for bpp in [1u32, 4, 8, 16, 24, 32] {
        let (w, h) = (r.range(1, 8) as u32, r.range(1, 8) as u32);
        let e = icon_bmp(&mut r, w, h, bpp);
        decode_case(&mut s, &format!("ico-bmp-{bpp}"), &ico(&[(w, h, bpp, e)]), 1, false);
    }
    {
        let e1 = icon_bmp(&mut r, 4, 4, 32);
        let e2 = icon_bmp(&mut r, 2, 3, 8);
        let raw: Vec<Vec<u8>> = (0..3).map(|_| bytes(&mut r, 3 * 4)).collect();
        let p = png(3, 3, 8, 6, false, &[], &png_filter(&raw, 4, &[0, 1, 4]), 100);
        decode_case(&mut s, "ico-three", &ico(&[(4, 4, 32, e1), (2, 3, 8, e2), (3, 3, 32, p)]), 3, false);
    }
    // the order of an icon's frames: sizes and depths in several orders
    {
        let a = icon_bmp(&mut r, 2, 2, 32);
        let b = icon_bmp(&mut r, 4, 4, 32);
        let c = icon_bmp(&mut r, 3, 3, 32);
        decode_case(&mut s, "ico-order-size", &ico(&[(2, 2, 32, a), (4, 4, 32, b), (3, 3, 32, c)]), 3, false);
        let a = icon_bmp(&mut r, 3, 3, 8);
        let b = icon_bmp(&mut r, 3, 3, 32);
        let c = icon_bmp(&mut r, 3, 3, 4);
        decode_case(&mut s, "ico-order-depth", &ico(&[(3, 3, 8, a), (3, 3, 32, b), (3, 3, 4, c)]), 3, false);
        let a = icon_bmp(&mut r, 5, 5, 24);
        let raw: Vec<Vec<u8>> = (0..2).map(|_| bytes(&mut r, 2 * 4)).collect();
        let p = png(2, 2, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 100);
        let c = icon_bmp(&mut r, 3, 3, 32);
        let d = icon_bmp(&mut r, 6, 2, 1);
        decode_case(&mut s, "ico-order-mixed", &ico(&[(5, 5, 24, a), (2, 2, 32, p), (3, 3, 32, c), (6, 2, 1, d)]), 4, false);
        // (PNG entries of small sizes alone, with filters none and up, and beside a BMP entry)
        for (w, h) in [(1, 1), (2, 2), (2, 3), (3, 2), (4, 4), (1, 5)] {
            for f in [0u8, 2] {
                let raw: Vec<Vec<u8>> = (0..h).map(|_| bytes(&mut r, w as usize * 4)).collect();
                let p = png(w, h, 8, 6, false, &[], &png_filter(&raw, 4, &[f]), 100);
                decode_case(&mut s, &format!("ico-png-{w}x{h}-{f}"), &ico(&[(w, h, 32, p.clone())]), 1, false);
                let b = icon_bmp(&mut r, 3, 3, 32);
                decode_case(&mut s, &format!("ico-png-{w}x{h}-{f}-bmp"), &ico(&[(w, h, 32, p), (3, 3, 32, b)]), 2, false);
            }
        }
        // (small PNG entries: every size from 1 to 8 pixels and two larger, beside BMP entries
        // of 1x1, 3x3 and 8x8 (32 bits) and 3x3 (8 bits), before and after them, the PNG
        // padded with an ancillary chunk or not)
        let mut r2 = XorShift32::new(0x1c0e);
        for (w, h) in [(1u32, 1u32), (2, 1), (1, 2), (3, 1), (1, 3), (2, 2), (4, 1), (1, 4), (5, 1), (1, 5), (6, 1), (1, 6), (2, 3), (3, 2), (7, 1), (8, 1), (2, 4), (3, 3), (4, 4)] {
            for (bw, bb) in [(1u32, 32u32), (3, 32), (8, 32), (3, 8)] {
                for pad in [false, true] {
                    let raw: Vec<Vec<u8>> = (0..h).map(|_| bytes(&mut r2, w as usize * 4)).collect();
                    let extra: Vec<(&[u8; 4], Vec<u8>)> = if pad { vec![(b"tEXt", vec![b'k', 0].into_iter().chain((0..200).map(|i| b'a' + (i % 26) as u8)).collect())] } else { vec![] };
                    let p = png(w, h, 8, 6, false, &extra, &png_filter(&raw, 4, &[0]), 100);
                    let b = icon_bmp(&mut r2, bw, bw, bb);
                    let pd = if pad { "-pad" } else { "" };
                    decode_case(&mut s, &format!("icosmall-{w}x{h}-b{bw}x{bb}{pd}"), &ico(&[(w, h, 32, p.clone()), (bw, bw, bb, b.clone())]), 2, false);
                    decode_case(&mut s, &format!("icosmall-b{bw}x{bb}-{w}x{h}{pd}"), &ico(&[(bw, bw, bb, b), (w, h, 32, p)]), 2, false);
                }
            }
        }
        // (the order of a PNG and a BMP entry of every small size: PNG 1..5 x 1..5, BMP 1..4 x
        // 1..4 of 32 bits, both file orders)
        let mut r3 = XorShift32::new(0x1c0f);
        for (pw, ph) in (1..=5u32).flat_map(|w| (1..=5u32).map(move |h| (w, h))) {
            let raw: Vec<Vec<u8>> = (0..ph).map(|_| bytes(&mut r3, pw as usize * 4)).collect();
            let p = png(pw, ph, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 100);
            for (bw, bh) in (1..=4u32).flat_map(|w| (1..=4u32).map(move |h| (w, h))) {
                let b = icon_bmp(&mut r3, bw, bh, 32);
                decode_case(&mut s, &format!("icopair-p{pw}x{ph}-b{bw}x{bh}"), &ico(&[(pw, ph, 32, p.clone()), (bw, bh, 32, b.clone())]), 2, false);
                decode_case(&mut s, &format!("icopair-b{bw}x{bh}-p{pw}x{ph}"), &ico(&[(bw, bh, 32, b), (pw, ph, 32, p.clone())]), 2, false);
            }
        }
        // (does the order hang on the BMP's pixels? the same sizes, other pixels, alpha all 0
        // or all 255, the mask all 0 or all 1)
        let mut r4 = XorShift32::new(0x1c0e);
        for (pw, ph, bw, bh) in [(2u32, 3u32, 1u32, 3u32), (2, 3, 1, 1), (3, 3, 2, 3), (4, 4, 4, 1), (4, 4, 3, 3)] {
            let raw: Vec<Vec<u8>> = (0..ph).map(|_| bytes(&mut r4, pw as usize * 4)).collect();
            let p = png(pw, ph, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 100);
            for k in 0..12 {
                let mut b = icon_bmp(&mut r4, bw, bh, 32);
                let px = 40..40 + (4 * bw * bh) as usize;
                let mask = 40 + (4 * bw * bh) as usize..b.len();
                match k {
                    8 => px.step_by(4).for_each(|i| b[i + 3] = 0),
                    9 => px.step_by(4).for_each(|i| b[i + 3] = 255),
                    10 => mask.for_each(|i| b[i] = 0),
                    11 => mask.for_each(|i| b[i] = 255),
                    _ => {}
                }
                decode_case(&mut s, &format!("icoseed-p{pw}x{ph}-b{bw}x{bh}-{k}"), &ico(&[(pw, ph, 32, p.clone()), (bw, bh, 32, b)]), 2, false);
            }
        }
        // (a directory that says 0 bits)
        let a = icon_bmp(&mut r, 3, 3, 8);
        let b = icon_bmp(&mut r, 2, 2, 4);
        decode_case(&mut s, "ico-order-zero", &ico(&[(3, 3, 0, a), (2, 2, 0, b)]), 2, false);
    }
    // DDS: DXT1/3/5 blocks with and without mips, an uncompressed A8R8G8B8 file
    for (k, (fc, bs)) in [(b"DXT1", 8usize), (b"DXT3", 16), (b"DXT5", 16)].into_iter().enumerate() {
        for (w, h, mips) in [(4u32, 4u32, 1u32), (8, 4, 1), (5, 3, 1), (8, 8, 4)] {
            let mut n = 0;
            for m in 0..mips {
                n += (w >> m).max(1).div_ceil(4) as usize * (h >> m).max(1).div_ceil(4) as usize * bs;
            }
            let mut data = bytes(&mut r, n);
            if k == 0 && w == 4 {
                // (a block whose first color is not above its second: the 3-color mode)
                data[0..4].copy_from_slice(&[0x10, 0x00, 0x20, 0x00]);
            }
            decode_case(&mut s, &format!("dds-{}-{w}x{h}-{mips}", fc.iter().map(|&c| char::from(c.to_ascii_lowercase())).collect::<String>()), &dds(w, h, mips, Some(fc), &data), mips, false);
        }
    }
    decode_case(&mut s, "dds-argb", &dds(3, 2, 1, None, &bytes(&mut r, 24)), 1, false);
    // not an image
    decode_case(&mut s, "garbage", &bytes(&mut r, 40), 1, false);
    // files cut at every length (the header, the tables, the pixels), and broken in other ways
    let mut r = XorShift32::new(0xb20e);
    let cuts = |s: &mut String, name: &str, f: &[u8], frames: u32| {
        let n = f.len();
        // (every length: where each decoder stops reading is measured, not guessed)
        for k in 1..n {
            decode_case(s, &format!("cut-{name}-{k}"), &f[..k], frames, false);
        }
    };
    // (a TIFF whose IFD comes first and its strips last, a GIF whose data spans many
    // sub-blocks, an interlaced PNG: cut every few bytes)
    let mut rc = XorShift32::new(0xc07);
    {
        let (w, h, rps) = (5u32, 3u32, 2u32);
        let px = bytes(&mut rc, (w * h * 3) as usize);
        let mut v = b"II\x2a\0".to_vec();
        v.extend_from_slice(&8u32.to_le_bytes());
        let n = 10u16;
        let after = 8 + 2 + 12 * n as u32 + 4;
        let (bits_at, offs_at, counts_at) = (after, after + 6, after + 14);
        let strips = after + 22;
        let row = w * 3;
        let tags: [(u16, u16, u32, u32); 10] = [(256, 4, 1, w), (257, 4, 1, h), (258, 3, 3, bits_at), (259, 3, 1, 1), (262, 3, 1, 2), (273, 4, 2, offs_at), (277, 3, 1, 3), (278, 4, 1, rps), (279, 4, 2, counts_at), (284, 3, 1, 1)];
        v.extend_from_slice(&n.to_le_bytes());
        for (t, ty, c, val) in tags {
            v.extend_from_slice(&t.to_le_bytes());
            v.extend_from_slice(&ty.to_le_bytes());
            v.extend_from_slice(&c.to_le_bytes());
            v.extend_from_slice(&if ty == 3 && c == 1 { (val as u16 as u32).to_le_bytes() } else { val.to_le_bytes() });
        }
        v.extend_from_slice(&0u32.to_le_bytes());
        for b in [8u16, 8, 8] {
            v.extend_from_slice(&b.to_le_bytes());
        }
        for o in [strips, strips + rps * row] {
            v.extend_from_slice(&o.to_le_bytes());
        }
        for c in [rps * row, (h - rps) * row] {
            v.extend_from_slice(&c.to_le_bytes());
        }
        assert_eq!(v.len() as u32, strips);
        v.extend_from_slice(&px);
        for k in 1..v.len() {
            decode_case(&mut s, &format!("cuta-tiff-{k}"), &v[..k], 1, false);
        }
        decode_case(&mut s, "cuta-tiff-whole", &v, 1, false);
    }
    {
        let idx: Vec<u8> = (0..32 * 16).map(|_| rc.next() as u8).collect();
        let g = gif(32, 16, Some(&rand_pal(&mut rc, 256)), 0, &[GifFrame { x: 0, y: 0, w: 32, h: 16, local: None, transparent: None, interlace: false, idx, min: 8 }]);
        for k in (790..g.len()).step_by(3) {
            decode_case(&mut s, &format!("cuta-gif-{k}"), &g[..k], 1, false);
        }
    }
    {
        let mut filtered = Vec::new();
        for &(x0, y0, dx, dy) in &ADAM7 {
            let (pw, ph) = ((9 + dx - 1 - x0) / dx, (9 + dy - 1 - y0) / dy);
            let raw: Vec<Vec<u8>> = (0..ph).map(|_| bytes(&mut rc, pw as usize * 3)).collect();
            filtered.extend(png_filter(&raw, 3, &[0]));
        }
        let p = png(9, 9, 8, 2, true, &[], &filtered, 60);
        for k in 41..p.len() {
            decode_case(&mut s, &format!("cuta-png-adam7-{k}"), &p[..k], 1, false);
        }
    }
    let gf = gif(6, 4, Some(&rand_pal(&mut r, 16)), 0, &[GifFrame { x: 0, y: 0, w: 6, h: 4, local: None, transparent: None, interlace: false, idx: bytes(&mut r, 24).iter().map(|v| v & 15).collect(), min: 4 }]);
    cuts(&mut s, "gif", &gf, 1);
    let tf = tiff(false, &[TiffImage { w: 5, h: 3, photometric: 2, bits: 8, samples: 3, extra: None, planar: 1, rps: 2, px: bytes(&mut r, 45), map: vec![], res: None, format: 1 }]);
    cuts(&mut s, "tiff", &tf, 1);
    let e = icon_bmp(&mut r, 4, 4, 32);
    let raw: Vec<Vec<u8>> = (0..3).map(|_| bytes(&mut r, 12)).collect();
    let pe = png(3, 3, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 100);
    cuts(&mut s, "ico", &ico(&[(4, 4, 32, e), (3, 3, 32, pe)]), 2);
    cuts(&mut s, "dds", &dds(8, 8, 2, Some(b"DXT1"), &bytes(&mut r, 40)), 2);
    let raw: Vec<Vec<u8>> = (0..4).map(|_| bytes(&mut r, 15)).collect();
    cuts(&mut s, "png", &png(5, 4, 8, 2, false, &[], &png_filter(&raw, 3, &[0, 1, 2, 3]), 100), 1);
    // RLE that runs past the picture: a run past the row, past the last row, a delta past the end,
    // an absolute run past the data
    let pal: Vec<u32> = (0..16).map(|i| 0xff00_0000 | (i * 0x0f0f0f)).collect();
    for (k, rle) in [vec![12u8, 3, 0, 1], vec![8, 1, 0, 0, 8, 2, 0, 0, 8, 3, 0, 0, 8, 4, 0, 1], vec![0, 2, 9, 9, 4, 5, 0, 1], vec![0, 6, 1, 2], vec![2, 7]].into_iter().enumerate() {
        decode_case(&mut s, &format!("bmprlebad8-{k}"), &bmp(40, 8, 3, 8, 1, [0; 4], 16, &pal, &rle, false, (0, 0)), 1, false);
        decode_case(&mut s, &format!("bmprlebad4-{k}"), &bmp(40, 8, 3, 4, 2, [0; 4], 16, &pal, &rle, false, (0, 0)), 1, false);
    }
    // PNG tRNS on 8 and 16-bit gray: the key level, another, a short chunk
    for (d, key) in [(8u8, vec![0u8, 0x40]), (16, vec![0x12, 0x34]), (8, vec![0x40]), (4, vec![0, 5]), (2, vec![0, 1]), (1, vec![0, 1])] {
        let n = if d == 16 { 2 } else { 1 };
        let mut raw: Vec<Vec<u8>> = (0..3).map(|_| bytes(&mut r, (4 * d as usize).div_ceil(8))).collect();
        if d >= 8 {
            raw[1][..n].copy_from_slice(&if d == 8 { vec![0x40] } else { vec![0x12, 0x34] });
        }
        // (the palette too: a gray picture with a key comes out indexed)
        decode_case(&mut s, &format!("png-trns-gray{d}-{}", key.len()), &png(4, 3, d, 0, false, &[(b"tRNS", key)], &png_filter(&raw, n, &[0]), 100), 1, d < 16);
    }
    // IHDRs the format does not allow: every color type with depths it does not take, color
    // types past 6, compression, filter and interlace methods other than 0 (and Adam7's 1)
    let valid = |ct: u8, d: u8| matches!((ct, d), (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) | (2 | 4 | 6, 8 | 16));
    let rows: Vec<u8> = (0..3).flat_map(|_| { let mut v = vec![0u8]; v.extend(bytes(&mut r, 24)); v }).collect();
    let plte: Vec<u8> = bytes(&mut r, 3 * 256);
    for ct in 0u8..=8 {
        for d in [0u8, 1, 2, 3, 4, 5, 7, 8, 9, 12, 16, 32, 255] {
            if valid(ct, d) {
                continue;
            }
            decode_case(&mut s, &format!("png-ihdr-{ct}-{d}"), &png(4, 3, d, ct, false, &[(b"PLTE", plte.clone())], &rows, 100), 1, false);
        }
    }
    for (k, at) in [(1u8, 10usize), (1, 11), (2, 12), (255, 12), (255, 10), (255, 11)] {
        let rows8: Vec<u8> = rows.chunks(25).flat_map(|c| c[..5].to_vec()).collect();
        let mut f = png(4, 3, 8, 0, false, &[], &rows8, 100);
        // (IHDR's data starts at 16; its CRC written again)
        f[16 + at] = k;
        let crc = crc32(&f[12..29]);
        f[29..33].copy_from_slice(&crc.to_be_bytes());
        decode_case(&mut s, &format!("png-ihdr-method-{at}-{k}"), &f, 1, false);
    }
    // TIFF layouts beyond the usual: 2-bit gray and palette, 16-bit gray+alpha, 16-bit RGB with
    // no alpha flag, 8-bit RGB+alpha unspecified premultiplied, YCbCr, a mask, 32-bit RGB, 4 extra
    for (k, (ph, bits, spp, extra)) in [(1u16, 2u16, 1u16, None), (3, 2, 1, None), (1, 16, 2, Some(2)), (2, 16, 4, Some(0)), (2, 8, 5, Some(2)), (6, 8, 3, None), (4, 1, 1, None), (2, 32, 3, None), (1, 8, 2, Some(0)), (1, 8, 2, Some(1)), (5, 8, 5, Some(1)), (2, 4, 3, None)].into_iter().enumerate() {
        let (w, h) = (5u32, 3u32);
        let total = ((w * bits as u32 * spp as u32).div_ceil(8) * h) as usize;
        let map = if ph == 3 { (0..3 * (1usize << bits)).map(|_| r.next() as u16).collect() } else { vec![] };
        let im = TiffImage { w, h, photometric: ph, bits, samples: spp, extra, planar: 1, rps: h, px: bytes(&mut r, total), map, res: None, format: 1 };
        decode_case(&mut s, &format!("tiff-other-{k}"), &tiff(false, &[im]), 1, ph == 3);
    }
    // CMYK with an associated alpha: alphas of 0, 1, 2, 128, 255 against every level
    let px: Vec<u8> = [0u8, 1, 2, 128, 255].iter().flat_map(|&a| [0u8, 1, 2, 127, 128, 254, 255, 77].chunks(4).flat_map(move |c| [c[0], c[1], c[2], c[3], a]).collect::<Vec<u8>>()).collect();
    let im = TiffImage { w: 2, h: 5, photometric: 5, bits: 8, samples: 5, extra: Some(1), planar: 1, rps: 5, px, map: vec![], res: None, format: 1 };
    decode_case(&mut s, "tiff-cmyka-assoc", &tiff(false, &[im]), 1, false);
    // a TIFF whose second page is YCbCr, and one whose first page is
    for first in [false, true] {
        let rgb = TiffImage { w: 2, h: 2, photometric: 2, bits: 8, samples: 3, extra: None, planar: 1, rps: 2, px: bytes(&mut r, 12), map: vec![], res: None, format: 1 };
        let ycc = TiffImage { photometric: 6, px: bytes(&mut r, 12), ..rgb.clone() };
        let pages = if first { [ycc, rgb] } else { [rgb, ycc] };
        decode_case(&mut s, &format!("tiff-ycbcr-page-{}", if first { 0 } else { 1 }), &tiff(false, &pages), 2, false);
    }
    // icon BMP entries of 2 bits and of 0 (a directory and header that agree), PNG entries of
    // gray, palette, 16 bits
    for bpp in [2u32, 0, 3, 5, 48, 64] {
        let e = icon_bmp(&mut r, 4, 4, bpp.max(1));
        let mut e2 = e.clone();
        e2[14..16].copy_from_slice(&(bpp as u16).to_le_bytes());
        decode_case(&mut s, &format!("ico-bmp-depth-{bpp}"), &ico(&[(4, 4, bpp, e2)]), 1, false);
    }
    for (ct, d, n) in [(0u8, 8u8, 1usize), (3, 8, 1), (2, 16, 6), (4, 8, 2), (0, 1, 1), (2, 8, 3), (6, 16, 8), (4, 16, 4), (0, 16, 2), (3, 4, 1), (3, 1, 1)] {
        let row = (4 * n * d as usize).div_ceil(8);
        let raw: Vec<Vec<u8>> = (0..4).map(|_| bytes(&mut r, row.max(1))).collect();
        let extra: Vec<(&[u8; 4], Vec<u8>)> = if ct == 3 { vec![(b"PLTE", bytes(&mut r, 3 << d))] } else { vec![] };
        let p = png(4, 4, d, ct, false, &extra, &png_filter(&raw, n.max(1), &[0]), 100);
        decode_case(&mut s, &format!("ico-png-ct{ct}-{d}"), &ico(&[(4, 4, 32, p)]), 1, false);
    }
    // DDS of other FourCCs and of a DX10 header
    for fc in [b"DXT2", b"DXT4", b"ATI1", b"ATI2", b"BC4U", b"BC5U", b"DX10"] {
        decode_case(&mut s, &format!("dds-{}", fc.iter().map(|&c| char::from(c.to_ascii_lowercase())).collect::<String>()), &dds(4, 4, 1, Some(fc), &bytes(&mut r, 32)), 1, false);
    }
    // DX10 headers: the BC formats (typeless, unorm, sRGB), BC4-BC7, 8-bit RGBA and BGRA, and an
    // array of two
    for (fmt, array) in [(70u32, 1u32), (71, 1), (72, 1), (73, 1), (74, 1), (75, 1), (76, 1), (77, 1), (78, 1), (80, 1), (83, 1), (95, 1), (98, 1), (28, 1), (29, 1), (87, 1), (91, 1), (71, 2)] {
        let mut f = dds(4, 4, 1, Some(b"DX10"), &[]);
        for v in [fmt, 3, 0, array, 0] {
            le32(&mut f, v);
        }
        f.extend_from_slice(&bytes(&mut r, 64 * array as usize));
        decode_case(&mut s, &format!("dds-dx10-{fmt}-{array}"), &f, array, false);
    }
    // an ICO's PNG entry with a resolution of its own (pHYs 150 by 72 dots an inch)
    {
        let mut r = XorShift32::new(0x1c0d);
        let raw: Vec<Vec<u8>> = (0..3).map(|_| bytes(&mut r, 3 * 4)).collect();
        let mut phys = 5906u32.to_be_bytes().to_vec();
        phys.extend_from_slice(&2835u32.to_be_bytes());
        phys.push(1);
        let p = png(3, 3, 8, 6, false, &[(b"pHYs", phys)], &png_filter(&raw, 4, &[0, 0, 0]), 100);
        decode_case(&mut s, "ico-png-phys", &ico(&[(3, 3, 32, p)]), 1, false);
    }
    // (does the order of a small PNG entry and a BMP entry hang on the PNG's pixels? one 3x3
    // PNG of other pixels, alpha all 255 or all 0, and the PNG's stored block size, beside BMPs
    // that came first and second in icopair-*)
    {
        let mut r = XorShift32::new(0x1c10);
        for k in 0..8u32 {
            let raw: Vec<Vec<u8>> = (0..3).map(|_| {
                let mut row = bytes(&mut r, 12);
                if k >= 4 {
                    for a in row.iter_mut().skip(3).step_by(4) { *a = if k % 2 == 0 { 255 } else { 0 }; }
                }
                row
            }).collect();
            let p = png(3, 3, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), if k == 3 { 7 } else { 100 });
            for (bw, bh) in [(2u32, 2u32), (1, 3), (2, 3), (4, 4)] {
                let b = icon_bmp(&mut r, bw, bh, 32);
                decode_case(&mut s, &format!("icopng-{k}-p-b{bw}x{bh}"), &ico(&[(3, 3, 32, p.clone()), (bw, bh, 32, b.clone())]), 2, false);
                decode_case(&mut s, &format!("icopng-{k}-b{bw}x{bh}-p"), &ico(&[(bw, bh, 32, b), (3, 3, 32, p.clone())]), 2, false);
            }
        }
    }
    // (three entries of one depth, a PNG among them, in every file order: which order does a
    // sort make of the PNG rule, which is no order; then a PNG beside BMPs of fewer bits, larger
    // PNGs beside 32-bit BMPs, and a PNG among BMPs of two other depths)
    {
        let mut r = XorShift32::new(0x1c11);
        let pngp = |r: &mut XorShift32, w: u32, h: u32| {
            let raw: Vec<Vec<u8>> = (0..h).map(|_| bytes(r, w as usize * 4)).collect();
            png(w, h, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 100)
        };
        const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        // (each entry: is it a PNG, its size, its depth)
        let trios: [[(bool, u32, u32, u32); 3]; 10] = [
            [(true, 2, 2, 32), (false, 1, 2, 32), (false, 2, 1, 32)],
            [(true, 3, 3, 32), (false, 2, 3, 32), (false, 3, 1, 32)],
            [(true, 3, 3, 32), (false, 4, 4, 32), (false, 1, 1, 32)],
            [(true, 3, 3, 32), (false, 3, 4, 32), (false, 2, 2, 32)],
            [(true, 4, 4, 32), (false, 3, 3, 32), (false, 4, 3, 32)],
            [(true, 2, 2, 32), (false, 4, 4, 32), (false, 1, 1, 32)],
            [(true, 3, 3, 32), (true, 2, 2, 32), (false, 3, 2, 32)],
            [(true, 2, 2, 32), (true, 3, 3, 32), (false, 4, 4, 32)],
            [(true, 3, 3, 32), (false, 3, 3, 8), (false, 3, 3, 24)],
            [(true, 3, 3, 32), (false, 2, 3, 8), (false, 4, 4, 24)],
        ];
        for (t, trio) in trios.iter().enumerate() {
            let files: Vec<Vec<u8>> = trio.iter().map(|&(p, w, h, bpp)| if p { pngp(&mut r, w, h) } else { icon_bmp(&mut r, w, h, bpp) }).collect();
            for (k, perm) in PERMS.iter().enumerate() {
                let es: Vec<(u32, u32, u32, Vec<u8>)> = perm.iter().map(|&i| (trio[i].1, trio[i].2, trio[i].3, files[i].clone())).collect();
                decode_case(&mut s, &format!("icotri-{t}-{k}"), &ico(&es), 3, false);
            }
        }
        for (pw, ph) in [(3u32, 3u32), (8, 8), (16, 16)] {
            let p = pngp(&mut r, pw, ph);
            for bw in [3u32, 16] {
                for bpp in [1u32, 4, 8, 24] {
                    let b = icon_bmp(&mut r, bw, bw, bpp);
                    decode_case(&mut s, &format!("icodepth-p{pw}x{ph}-b{bw}x{bpp}"), &ico(&[(pw, ph, 32, p.clone()), (bw, bw, bpp, b.clone())]), 2, false);
                    decode_case(&mut s, &format!("icodepth-b{bw}x{bpp}-p{pw}x{ph}"), &ico(&[(bw, bw, bpp, b), (pw, ph, 32, p.clone())]), 2, false);
                }
            }
        }
        for (pw, ph) in [(8u32, 8u32), (16, 16), (12, 12), (16, 8)] {
            let p = pngp(&mut r, pw, ph);
            for (bw, bh) in [(8u32, 8u32), (16, 16), (10, 10), (6, 6), (16, 12)] {
                let b = icon_bmp(&mut r, bw, bh, 32);
                decode_case(&mut s, &format!("icobig-p{pw}x{ph}-b{bw}x{bh}"), &ico(&[(pw, ph, 32, p.clone()), (bw, bh, 32, b.clone())]), 2, false);
                decode_case(&mut s, &format!("icobig-b{bw}x{bh}-p{pw}x{ph}"), &ico(&[(bw, bh, 32, b), (pw, ph, 32, p.clone())]), 2, false);
            }
        }
    }
    // (probes for WICIMAGE.DECODE.BROKEN: a GIF image with no color table at all, PNG filter
    // types past 4, a TIFF whose strip list is shorter than its picture, pictures past 2^20
    // pixels read by rectangles)
    let gw = [GifFrame { x: 0, y: 0, w: 3, h: 2, local: None, transparent: None, interlace: false, idx: vec![0, 1, 2, 3, 1, 0], min: 2 }];
    decode_case(&mut s, "broken-gif-notable", &gif(3, 2, None, 0, &gw), 1, true);
    for f in [5u8, 6, 255] {
        let raw = [0u8, 10, 20, 30, f, 40, 50, 60, 1, 1, 2, 3];
        decode_case(&mut s, &format!("broken-png-filter-{f}"), &png(3, 3, 8, 0, false, &[], &raw, 1 << 16), 1, false);
    }
    for k in 0..2 {
        let im = TiffImage { w: 4, h: 4, photometric: 1, bits: 8, samples: 1, extra: None, planar: 1, rps: 2, px: (0..16).map(|v| v * 13).collect(), map: Vec::new(), res: None, format: 1 };
        let mut f = tiff(false, &[im]);
        // (two strips listed: RowsPerStrip written 1, or the height 6, so the picture wants more)
        if k == 0 {
            tiff_patch(&mut f, 278, 1);
        } else {
            tiff_patch(&mut f, 257, 6);
        }
        decode_case(&mut s, &format!("broken-tiff-strips-{k}"), &f, 1, false);
    }
    for (w, h) in [(1200u32, 1000u32), (40000, 40000)] {
        let rects = [(0, 0, 4, 2), (w - 4, h - 2, 4, 2), (w / 2, h / 2, 3, 1)];
        let big = |s: &mut String, name: &str, file: &[u8]| {
            writeln!(s, "case {name}\nwdecode {}\nwframe 0\nwinfo", hex(file));
            for (x, y, rw, rh) in rects {
                writeln!(s, "wpixels {x} {y} {rw} {rh}");
            }
            s.push_str("end\n");
        };
        let pal = [0x102030u32, 0x405060];
        big(&mut s, &format!("broken-big-bmp-{w}"), &bmp(40, w as i32, h as i32, 8, 1, [0; 4], 2, &pal, &[0, 0, 0, 1], false, (0, 0)));
        big(&mut s, &format!("broken-big-png-{w}"), &png(w, h, 8, 0, false, &[], &[0, 1, 2], 1 << 16));
        let mut g = b"GIF89a".to_vec();
        le16(&mut g, w);
        le16(&mut g, h);
        g.extend_from_slice(&[0, 0, 0, 0x2c, 0, 0, 0, 0]);
        le16(&mut g, w);
        le16(&mut g, h);
        // (a local table of 2 colors; LZW size 2: a clear code, then the end code)
        g.extend_from_slice(&[0x80, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 2, 2, 0x2c, 0, 0, 0x3b]);
        big(&mut s, &format!("broken-big-gif-{w}"), &g);
    }
    // (round 2: which size GIF's limit reads, a row the data reaches in part under a filter,
    // BMP's limits at their edge, big TIFFs)
    let gifsz = |s: &mut String, name: &str, (sw, sh): (u32, u32), (fw, fh): (u32, u32)| {
        let mut g = b"GIF89a".to_vec();
        le16(&mut g, sw);
        le16(&mut g, sh);
        g.extend_from_slice(&[0, 0, 0, 0x2c, 0, 0, 0, 0]);
        le16(&mut g, fw);
        le16(&mut g, fh);
        g.extend_from_slice(&[0x80, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 2, 2, 0x2c, 0, 0, 0x3b]);
        writeln!(s, "case {name}\nwdecode {}\nwframe 0\nwinfo\nwpixels 0 0 1 1\nend", hex(&g));
    };
    gifsz(&mut s, "broken-gifsize-screen", (40000, 40000), (2, 2));
    gifsz(&mut s, "broken-gifsize-frame", (2, 2), (40000, 40000));
    gifsz(&mut s, "broken-gifsize-zero", (0, 5), (2, 2));
    gifsz(&mut s, "broken-gifsize-edge-0", (32769, 16383), (2, 2));
    gifsz(&mut s, "broken-gifsize-edge-1", (32770, 16383), (2, 2));
    for (k, (w, h, data)) in [(1200u32, 1000u32, vec![1u8, 5, 3]), (4, 3, vec![0, 1, 2, 3, 4, 2, 9, 9]), (4, 3, vec![0, 1, 2, 3, 4, 1, 9, 9]), (4, 3, vec![0, 1, 2, 3, 4, 4, 9, 9])].into_iter().enumerate() {
        writeln!(s, "case broken-png-part-{k}\nwdecode {}\nwframe 0\nwinfo\nwpixels 0 0 4 2\nend", hex(&png(w, h, 8, 0, false, &[], &data, 1 << 16)));
    }
    decode_case(&mut s, "broken-png-part-adam7", &png(4, 4, 8, 0, true, &[], &[0, 7, 0, 8, 0, 9, 1], 1 << 16), 1, false);
    let pal = [0x102030u32, 0x405060];
    for (k, (w, h)) in [(0i32, 4i32), (4, 0), (-4, 4), (4096, 131071), (4096, -131071), (0x0400_0000, 1)].into_iter().enumerate() {
        writeln!(s, "case broken-bmpsize-{k}\nwdecode {}\nwframe 0\nwinfo\nwpixels 0 0 1 1\nend", hex(&bmp(40, w, h, 8, 1, [0; 4], 2, &pal, &[0, 1], false, (0, 0))));
    }
    for (w, h) in [(1200u32, 1000u32), (40000, 40000)] {
        let im = TiffImage { w: 4, h: 4, photometric: 1, bits: 8, samples: 1, extra: None, planar: 1, rps: 2, px: (0..16).map(|v| v * 13).collect(), map: Vec::new(), res: None, format: 1 };
        let mut f = tiff(false, &[im]);
        tiff_patch(&mut f, 256, w);
        tiff_patch(&mut f, 257, h);
        tiff_patch(&mut f, 278, h);
        writeln!(s, "case broken-big-tiff-{w}\nwdecode {}\nwframe 0\nwinfo\nwpixels 0 0 4 2\nwpixels {} {} 3 1\nend", hex(&f), w / 2, h / 2);
    }
    // (WICIMAGE.DECODE.BROKEN) an 8x8 interlaced PNG whose data ends after each pass and
    // inside each: which passes are kept; every byte its own value
    let all: Vec<u8> = {
        let mut v = Vec::new();
        let mut n = 1u8;
        for (cols, rows) in [(1usize, 1usize), (1, 1), (2, 1), (2, 2), (4, 2), (4, 4), (8, 4)] {
            for _ in 0..rows {
                v.push(0);
                for _ in 0..cols {
                    v.push(n);
                    n = n.wrapping_add(3);
                }
            }
        }
        v
    };
    for cut in [1usize, 2, 3, 4, 6, 7, 10, 13, 18, 23, 30, 43, 60, 78] {
        decode_case(&mut s, &format!("broken-adam7-{cut}"), &png(8, 8, 8, 0, true, &[], &all[..cut], 1 << 16), 1, false);
    }
    // a TIFF of one strip with fewer bytes than its rows: one pixel of each row asked in turn
    for (w, h) in [(64u32, 64u32), (256, 256), (1200, 1000), (4096, 4096), (100, 10000), (8192, 8192)] {
        let im = TiffImage { w: 4, h: 4, photometric: 1, bits: 8, samples: 1, extra: None, planar: 1, rps: 2, px: (0..16).map(|v| v * 13).collect(), map: Vec::new(), res: None, format: 1 };
        let mut f = tiff(false, &[im]);
        tiff_patch(&mut f, 256, w);
        tiff_patch(&mut f, 257, h);
        tiff_patch(&mut f, 278, h);
        write!(s, "case broken-short-tiff-{w}-{h}\nwdecode {}\nwframe 0\nwinfo\n", hex(&f));
        for y in [0u32, 1, 2, 3, 4, 7, 8, 15, 16, 31, 32, 63, 64, 127, 128, 255, 256, 511, 512, 999, 1023, 2047, 4095, 8191, 9999] {
            if y < h {
                writeln!(s, "wpixels {} {y} 1 1", w - 1);
            }
        }
        s.push_str("end\n");
    }
    // GIF frames about 2^26 pixels
    gifsz(&mut s, "broken-gifsize-big-0", (8192, 8192), (8192, 8192));
    gifsz(&mut s, "broken-gifsize-big-1", (8193, 8192), (8193, 8192));
    s
}

/// a little-endian TIFF's first IFD: the entry for `tag` given the value `v` (SHORT or LONG)
fn tiff_patch(f: &mut [u8], tag: u16, v: u32) {
    if let Some(at) = tiff_entry(f, tag) {
        let short = f[at + 2] == 3;
        if short {
            f[at + 8..at + 10].copy_from_slice(&(v as u16).to_le_bytes());
        } else {
            f[at + 8..at + 12].copy_from_slice(&v.to_le_bytes());
        }
    }
}

fn tiff_entry(f: &[u8], tag: u16) -> Option<usize> {
    let ifd = u32::from_le_bytes(f.get(4..8)?.try_into().ok()?) as usize;
    let n = usize::from(u16::from_le_bytes(f.get(ifd..ifd + 2)?.try_into().ok()?));
    (0..n).map(|i| ifd + 2 + 12 * i).find(|&at| f.get(at..at + 2) == Some(&tag.to_le_bytes()[..]))
}

/// the containers WIC has encoders for
const ENCODERS: &[&str] = &["bmp", "png", "tiff", "gif", "jpeg", "wmp", "dds", "heif", "webp"];

pub fn wicencode() -> String {
    let mut s = format!("# kikizake suite wicencode: the WIC encoders, each file decoded again. {HEAD}\n");
    let names: Vec<&str> = FORMATS.iter().map(|f| f.0).collect();
    // every encoder with every format: what SetPixelFormat answers, the file (BMP: every byte),
    // and what decodes from it
    let mut seed = 1;
    for c in ENCODERS {
        for f in &names {
            // (the file itself too where the model writes it byte by byte: BMP, and the lossy
            // encoders and GIF, whose bytes are what to fit)
            let bytes = if ["bmp", "gif", "jpeg", "wmp", "dds"].contains(c) { "wbytes\n" } else { "" };
            // (GIF: the palette it writes too)
            let pal = if *c == "gif" { "wsrcpal\n" } else { "" };
            writeln!(s, "case enc-{c}-{f}\nwbits {f} 5 3 {seed}\nwencode {c}\n{bytes}wredecode\nwframe 0\nwinfo\nwpixels\n{pal}end");
            seed += 1;
        }
    }
    // a format other than the source's asked of SetPixelFormat
    for c in ["bmp", "png", "tiff"] {
        for f in ["bgra", "gray8", "idx8", "rgba64", "bw"] {
            writeln!(s, "case encas-{c}-{f}\nwbits bgr24 4 2 {seed}\nwencode {c} {f}\nwredecode\nwframe 0\nwinfo\nwpixels\nend");
            seed += 1;
        }
    }
    // color into BlackWhite: 16x16 of one gray at every level, of one primary every 5 levels
    let one = |c: [u8; 3]| bmp(40, 16, 16, 24, 0, [0; 4], 0, &[], &c.repeat(256), false, (0, 0));
    for v in 0..=255u8 {
        writeln!(s, "case encbw-gray-{v}\nwdecode {}\nwframe 0\nwencode png bw\nwredecode\nwframe 0\nwpixels\nend", hex(&one([v; 3])));
    }
    for (k, ch) in ["b", "g", "r"].iter().enumerate() {
        for v in (0..=255u8).step_by(5) {
            let mut c = [0u8; 3];
            c[k] = v;
            writeln!(s, "case encbw-{ch}-{v}\nwdecode {}\nwframe 0\nwencode png bw\nwredecode\nwframe 0\nwpixels\nend", hex(&one(c)));
        }
    }
    // 16-bit sources into 64bppRGBAFixedPoint (what the BMP encoder takes them as): every 16th
    // level and every level below 1024, gray, RGB and RGBA
    let lv = |k: usize, lo: bool| if lo { k as u16 } else { (k * 16 + k % 16) as u16 };
    for lo in [false, true] {
        let n = if lo { 1024usize } else { 4096 };
        let w = 64u32;
        let h = (n / 64) as u32;
        let mk = |ch: usize, f: &dyn Fn(usize) -> Vec<u16>| -> Vec<Vec<u8>> { (0..h as usize).map(|y| (0..64).flat_map(|x| f(y * 64 + x)).flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>()).map(|r| { assert_eq!(r.len(), 64 * 2 * ch); r }).collect() };
        let tag = if lo { "lo" } else { "all" };
        let g = mk(1, &|k| vec![lv(k, lo)]);
        let c = mk(3, &|k| vec![lv(k, lo), 65535 - lv(k, lo), (k as u16).wrapping_mul(37)]);
        let a = mk(4, &|k| vec![lv(k, lo), (k as u16).wrapping_mul(37), 65535 - lv(k, lo), (k as u16).wrapping_mul(7919)]);
        for (name, ct, rows, bpp) in [("gray16", 0u8, g, 2usize), ("rgb48", 2, c, 6), ("rgba64", 6, a, 8)] {
            let f = png(w, h, 16, ct, false, &[], &png_filter(&rows, bpp, &[0]), 4096);
            writeln!(s, "case fixed-{name}-{tag}\nwdecode {}\nwframe 0\nwinfo\nwconv rgba64fixed\nwpixels\nend", hex(&f));
            // (and into the float formats the Fant scaler works in)
            for to in ["grayfloat", "rgbafloat", "prgbafloat", "rgbfloat"] {
                writeln!(s, "case float-{name}-{tag}-{to}\nwdecode {}\nwframe 0\nwconv {to}\nwpixels\nend", hex(&f));
            }
        }
    }
    // a gray ramp and random pictures of 2 to 300 colors into 8bppIndexed: the palette chosen
    let mut r = XorShift32::new(0x9a1e);
    let ramp: Vec<u8> = (0..=255u8).flat_map(|v| [v; 3]).collect();
    writeln!(s, "case encq-ramp\nwdecode {}\nwframe 0\nwencode png idx8\nwredecode\nwframe 0\nwpixels\nwsrcpal\nend", hex(&bmp(40, 16, 16, 24, 0, [0; 4], 0, &[], &ramp, false, (0, 0))));
    for n in [2usize, 3, 16, 17, 100, 255, 256, 257, 300] {
        let cols: Vec<[u8; 3]> = (0..n).map(|_| [r.next() as u8, r.next() as u8, r.next() as u8]).collect();
        let px: Vec<u8> = (0..400).flat_map(|i| cols[if i < n { i } else { r.next() as usize % n }]).collect();
        writeln!(s, "case encq-{n}\nwdecode {}\nwframe 0\nwencode png idx8\nwredecode\nwframe 0\nwpixels\nwsrcpal\nend", hex(&bmp(40, 20, 20, 24, 0, [0; 4], 0, &[], &px, false, (0, 0))));
    }
    // options
    let opts: &[(&str, &str)] = &[
        ("bmp", "EnableV5Header32bppBGRA=1"),
        ("bmp", "EnableV5Header32bppBGRA=0"),
        ("png", "InterlaceOption=1"),
        ("png", "FilterOption=1"),
        ("png", "FilterOption=2"),
        ("png", "FilterOption=3"),
        ("png", "FilterOption=4"),
        ("png", "FilterOption=5"),
        ("png", "FilterOption=6"),
        ("tiff", "TiffCompressionMethod=1"),
        ("tiff", "TiffCompressionMethod=2"),
        ("tiff", "TiffCompressionMethod=6"),
        ("tiff", "TiffCompressionMethod=7"),
        ("jpeg", "ImageQuality=1.0"),
        ("jpeg", "ImageQuality=0.0"),
        ("jpeg", "JpegYCrCbSubsampling=3"),
        ("jpeg", "BitmapTransform=1"),
        ("jpeg", "SuppressApp0=1"),
        ("wmp", "Lossless=1"),
        ("wmp", "ImageQuality=1.0"),
        ("png", "ImageQuality=0.5"),
    ];
    for (k, (c, o)) in opts.iter().enumerate() {
        for f in ["bgra", "bgr24", "gray8"] {
            writeln!(s, "case opt-{k}-{f}\nwbits {f} 6 4 {seed}\nwencode {c} {o}\nwredecode\nwframe 0\nwinfo\nwpixels\nend");
            seed += 1;
        }
    }
    // a larger picture through each lossless encoder (PNG and TIFF strips, GIF's LZW past its
    // first codes)
    for c in ["bmp", "png", "tiff", "gif"] {
        let f = if c == "gif" { "idx8" } else { "bgra" };
        writeln!(s, "case big-{c}\nwbits {f} 40 30 {seed}\nwencode {c}\nwredecode\nwframe 0\nwinfo\nwpixels\nend");
        seed += 1;
    }
    // GIF from RGBA: every alpha (which go transparent), and a palette that fills 2^b or 256
    // with a transparent entry still to come
    let rgba = |w: u32, px: &dyn Fn(usize) -> [u8; 4]| {
        let rows: Vec<Vec<u8>> = (0..16usize).map(|y| (0..w as usize).flat_map(|x| px(y * w as usize + x)).collect()).collect();
        png(w, 16, 8, 6, false, &[], &png_filter(&rows, 4, &[0]), 4096)
    };
    let cases: [(&str, Vec<u8>); 4] = [
        ("alpha", rgba(16, &|i| [(i % 8 * 32) as u8, 255 - (i % 8 * 32) as u8, 77, i as u8])),
        ("full16", rgba(16, &|i| [(i % 16 * 16) as u8, 9, 200, if i % 5 == 0 { 0 } else { 255 }])),
        ("full255", rgba(16, &|i| [(i % 255) as u8, (i % 255 * 7) as u8, 3, if i == 255 { 0 } else { 255 }])),
        ("full256", rgba(16, &|i| [i as u8, (i * 7) as u8, 3, if i == 17 { 0 } else { 255 }])),
    ];
    // every 16-bit level into 128bppRGBFloat (the table of the linear values the float formats
    // and the Fant scaler start from)
    let every: Vec<Vec<u8>> = (0..256u32).map(|y| (0..256u32).flat_map(|x| ((y * 256 + x) as u16).to_be_bytes()).collect()).collect();
    let f = png(256, 256, 16, 0, false, &[], &png_filter(&every, 2, &[0]), 4096);
    writeln!(s, "case float-gray16-every-rgbfloat\nwdecode {}\nwframe 0\nwconv rgbfloat\nwpixels\nend", hex(&f));
    // more colors than an indexed format holds: the palette the encoder makes, and the pixels
    for f in ["bw", "idx4", "idx8"] {
        for (k, (w, h, seed)) in [(4u32, 2u32, 212u32), (16, 16, 0x51), (16, 16, 0x52), (32, 32, 0x53)].iter().enumerate() {
            writeln!(s, "case encpal-{f}-{k}\nwbits bgr24 {w} {h} {seed}\nwencode bmp {f}\nwredecode\nwframe 0\nwinfo\nwpixels\nwsrcpal\nend");
        }
    }
    for (n, f) in cases {
        writeln!(s, "case gifalpha-{n}\nwdecode {}\nwframe 0\nwencode gif\nwbytes\nwredecode\nwframe 0\nwinfo\nwpixels\nwsrcpal\nend", hex(&f));
    }
    gridtie(&mut s);
    // DDS pictures of more than one row of blocks
    for (f, w, h, seed) in [("gray8", 9u32, 9u32, 0x0dd5_0001u32), ("gray8", 6, 10, 0x0dd5_0002), ("gray16", 8, 12, 0x0dd5_0003), ("bgra", 9, 9, 0x0dd5_0004), ("bgr24", 4, 8, 0x0dd5_0005)] {
        writeln!(s, "case encdds-{f}-{w}x{h}\nwbits {f} {w} {h} {seed}\nwencode dds\nwbytes\nwredecode\nwframe 0\nwinfo\nwpixels\nend");
    }
    // the TIFF compression methods the options above leave out (0 don't care, 3 CCITT4, 4 LZW,
    // 5 RLE), on their own seeds
    for (k, m) in [0u32, 3, 4, 5].iter().enumerate() {
        for (j, f) in ["bgra", "bgr24", "gray8", "bw"].iter().enumerate() {
            writeln!(s, "case opt-tiff-{m}-{f}\nwbits {f} 6 4 {}\nwencode tiff TiffCompressionMethod={m}\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x7177_0000u32 + (k * 4 + j) as u32);
        }
    }
    // every 16-bit gray level into 48bppRGB (16bppGray's own path into 64bppRGBAFixedPoint goes
    // through it, fixpath-gray16-rgb48)
    {
        let every: Vec<Vec<u8>> = (0..256u32).map(|y| (0..256u32).flat_map(|x| ((y * 256 + x) as u16).to_be_bytes()).collect()).collect();
        let f = png(256, 256, 16, 0, false, &[], &png_filter(&every, 2, &[0]), 4096);
        writeln!(s, "case gray16-every-rgb48\nwdecode {}\nwframe 0\nwconv rgb48\nwpixels\nend", hex(&f));
    }
    // many single DDS blocks (one 4x4 picture each, its own seed): the start and the stop of the
    // color fit, and the alpha fit
    for (f, n) in [("bgr24", 48u32), ("bgra", 32), ("gray8", 16)] {
        for k in 0..n {
            writeln!(s, "case ddsblock-{f}-{k}\nwbits {f} 4 4 {}\nwencode dds\nwbytes\nend", 0x0dd6_0000u32 + k * 7919 + n);
        }
    }
    // 16bppGray into 64bppRGBAFixedPoint through each other format: which path Windows takes
    {
        let rows: Vec<Vec<u8>> = (0..64usize).map(|y| (0..64usize).flat_map(|x| { let k = y * 64 + x; ((k * 16 + k % 16) as u16).to_be_bytes() }).collect()).collect();
        let f = png(64, 64, 16, 0, false, &[], &png_filter(&rows, 2, &[0]), 4096);
        for mid in ["grayfloat", "rgbafloat", "prgbafloat", "rgbfloat", "rgb96float", "grayfix16", "grayfix32", "grayhalf", "rgba64half", "rgb64half", "rgb48half", "rgb48", "rgba64", "rgb48fixed", "rgb64fixed", "rgb96fixed", "rgba128fixed", "rgb128fixed", "bgra64fixed"] {
            writeln!(s, "case fixpath-gray16-{mid}\nwdecode {}\nwframe 0\nwconv {mid}\nwinfo\nwpixels\nwconv rgba64fixed\nwpixels\nend", hex(&f));
        }
    }
    // every 8-bit level of 24bppBGR, 32bppBGRA (and every alpha) and 8bppGray into
    // 64bppRGBAFixedPoint
    {
        let ramp: Vec<u8> = (0..256u32).flat_map(|v| [v as u8, (255 - v) as u8, (v * 7 % 256) as u8]).collect();
        let b = bmp(40, 256, 1, 24, 0, [0; 4], 0, &[], &ramp, true, (2835, 2835));
        writeln!(s, "case rgba64fixed-every-bgr24\nwdecode {}\nwframe 0\nwconv rgba64fixed\nwpixels\nend", hex(&b));
        let ramp: Vec<u8> = (0..256u32).flat_map(|v| [v as u8, (255 - v) as u8, (v * 7 % 256) as u8, (v * 13 % 256) as u8]).collect();
        let raw: Vec<Vec<u8>> = vec![ramp.chunks(4).flat_map(|c| [c[2], c[1], c[0], c[3]]).collect()];
        let p = png(256, 1, 8, 6, false, &[], &png_filter(&raw, 4, &[0]), 4096);
        writeln!(s, "case rgba64fixed-every-bgra\nwdecode {}\nwframe 0\nwconv rgba64fixed\nwpixels\nend", hex(&p));
        let raw: Vec<Vec<u8>> = vec![(0..=255u8).collect()];
        let p = png(256, 1, 8, 0, false, &[], &png_filter(&raw, 1, &[0]), 4096);
        writeln!(s, "case rgba64fixed-every-gray8\nwdecode {}\nwframe 0\nwconv rgba64fixed\nwpixels\nend", hex(&p));
    }
    // (not measured yet: premultiplied and RGBA order into 64bppRGBAFixedPoint, and a
    // TiffCompressionMethod past 7)
    for (k, f) in ["pbgra", "rgba", "prgba"].iter().enumerate() {
        writeln!(s, "case rgba64fixed-from-{f}\nwbits {f} 16 16 {}\nwconv rgba64fixed\nwpixels\nend", 0x64f1_0000u32 + k as u32);
    }
    writeln!(s, "case opt-tiff-8-bgra\nwbits bgra 6 4 {}\nwencode tiff TiffCompressionMethod=8\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x7177_0100u32);
    // (not measured yet: the fixed-point and float formats back into 8 bits, and an option name
    // no encoder has)
    for (k, (f, t)) in [("rgba64fixed", "bgra"), ("rgbafloat", "bgra"), ("rgb48fixed", "rgb48"), ("grayfix16", "gray8"), ("rgba64half", "bgra")].into_iter().enumerate() {
        writeln!(s, "case back-{f}-{t}\nwbits bgra 16 16 {}\nwconv {f}\nwconv {t}\nwpixels\nend", 0xbac0_0000u32 + k as u32);
    }
    // GIF from RGBA of more colors than 256 with transparent pixels (the grid a box short):
    // random colors, random colors of a few levels (counts differ), one transparent pixel or many
    let mut r = crate::rng::XorShift32::new(0x61f6_0000);
    for (k, (w, levels, every)) in [(32u32, 256u32, 7usize), (32, 256, 1000), (32, 12, 5), (48, 40, 3)].into_iter().enumerate() {
        let px: Vec<[u8; 4]> = (0..(w * 16) as usize).map(|i| {
            let mut c = || (r.next() % levels * (255 / (levels - 1).max(1))).min(255) as u8;
            [c(), c(), c(), if i % every == every / 2 { 0 } else { 255 }]
        }).collect();
        writeln!(s, "case gifgrid-{k}\nwdecode {}\nwframe 0\nwencode gif\nwbytes\nwredecode\nwframe 0\nwinfo\nwpixels\nwsrcpal\nend", hex(&rgba(w, &|i| px[i])));
    }
    // (64bppRGBAFixedPoint's alpha back into 16 bits, and the float and half formats' alpha:
    // random alphas)
    for (k, (f, t)) in [("rgba64fixed", "rgba64"), ("rgbafloat", "rgba64"), ("rgba64half", "rgba64")].into_iter().enumerate() {
        writeln!(s, "case backalpha-{f}-{t}\nwbits bgra 16 16 {}\nwconv {f}\nwconv {t}\nwpixels\nend", 0xba1f_0000u32 + k as u32);
    }
    // 8-bit color into 16bppGrayFixedPoint as it is (the luma's sums and rounding): random
    // pixels, and every level of each primary and of gray
    for (k, f) in ["bgra", "bgr24", "rgba", "pbgra", "gray8"].into_iter().enumerate() {
        writeln!(s, "case grayfix-from-{f}\nwbits {f} 64 64 {}\nwconv grayfix16\nwpixels\nend", 0x6f10_0000u32 + k as u32);
    }
    for (name, px) in [("red", [0u8, 0, 1]), ("green", [0, 1, 0]), ("blue", [1, 0, 0]), ("gray", [1, 1, 1])] {
        let rows: Vec<Vec<u8>> = (0..16usize).map(|y| (0..16usize).flat_map(|x| { let c = (y * 16 + x) as u8; [px[2] * c, px[1] * c, px[0] * c] }).collect()).collect();
        writeln!(s, "case grayfix-every-{name}\nwdecode {}\nwframe 0\nwconv bgr24\nwconv grayfix16\nwpixels\nend", hex(&png(16, 16, 8, 2, false, &[], &png_filter(&rows, 3, &[0]), 4096)));
    }
    // (not measured yet: color wide formats into gray, gray wide formats into color)
    for (k, (f, t)) in [("rgba64fixed", "gray16"), ("rgbafloat", "gray16"), ("grayfix16", "bgra"), ("grayfloat", "rgba64")].into_iter().enumerate() {
        writeln!(s, "case cross-{f}-{t}\nwbits bgra 16 16 {}\nwconv {f}\nwconv {t}\nwpixels\nend", 0xc055_0000u32 + k as u32);
    }
    // (color into gray from the wide formats: every 16-bit level as RGBA float, every s2.13
    // level as RGBA fixed point, gray all three channels)
    {
        let lin = crate::model::wicimage::linear16;
        let g48 = crate::model::wicimage::gray16_rgb48;
        let mut pick = vec![None; 8193];
        for v in 0..=65535u16 {
            let k = (f64::from(lin(g48(v))) * 8192.0 + 0.5).floor() as usize;
            if k <= 8192 && pick[k].is_none() {
                pick[k] = Some(v);
            }
        }
        let gray = |lv: &[u16]| -> Vec<u8> {
            let h = lv.len().div_ceil(128);
            let rows: Vec<Vec<u8>> = (0..h).map(|y| (0..128).flat_map(|x| lv[(y * 128 + x).min(lv.len() - 1)].to_be_bytes()).collect()).collect();
            png(128, h as u32, 16, 0, false, &[], &png_filter(&rows, 2, &[0]), 4096)
        };
        let fixlv: Vec<u16> = pick.iter().flatten().copied().collect();
        writeln!(s, "case graysweep-rgba64fixed-gray16\nwdecode {}\nwframe 0\nwconv rgba64fixed\nwconv gray16\nwpixels\nend", hex(&gray(&fixlv)));
        for j in 0..4u32 {
            let lv: Vec<u16> = (0..16384u32).map(|k| (k * 4 + (k + j) % 4) as u16).collect();
            writeln!(s, "case graysweep{j}-rgbafloat-gray16\nwdecode {}\nwframe 0\nwconv rgbafloat\nwconv gray16\nwpixels\nend", hex(&gray(&lv)));
        }
    }
    // (where inside the gaps between neighbouring gray levels the steps into 16 bits sit: red
    // and green at a 16-bit level v, blue from v to v + 15, so that the luma creeps across the
    // gap above v in 16 steps; three stretches of 256 levels)
    for (k, base) in [2560u32, 25600, 51200].into_iter().enumerate() {
        let rows: Vec<Vec<u8>> = (0..32u32).map(|y| (0..128u32).flat_map(|x| {
            let v = base + y * 8 + x / 16;
            let b = (v + x % 16).min(65535);
            [v as u16, v as u16, b as u16].into_iter().flat_map(|c| c.to_be_bytes())
        }).collect()).collect();
        let f = png(128, 32, 16, 2, false, &[], &png_filter(&rows, 6, &[0]), 4096);
        writeln!(s, "case gapsweep-{k}-rgbafloat-gray16\nwdecode {}\nwframe 0\nwconv rgbafloat\nwconv gray16\nwpixels\nend", hex(&f));
        writeln!(s, "case gapsweep-{k}-rgbafloat-rgba64\nwdecode {}\nwframe 0\nwconv rgbafloat\nwconv rgba64\nwpixels\nend", hex(&f));
    }
    writeln!(s, "case opt-jpeg-sub1-bgra\nwbits bgra 6 4 {}\nwencode jpeg JpegYCrCbSubsampling=1\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x7177_0101u32);
    // the wide formats back into 8 and 16 bits: every s2.13 value 0..=8192 (a 16bppGray level
    // whose 48bppRGB level lands on it), and every 4th 16-bit level as float and half (wbits
    // makes no wide format, so no values out of 0..=1 here)
    let lin = crate::model::wicimage::linear16;
    let g48 = crate::model::wicimage::gray16_rgb48;
    let mut pick = vec![None; 8193];
    for v in 0..=65535u16 {
        let k = (f64::from(lin(g48(v))) * 8192.0 + 0.5).floor() as usize;
        if k <= 8192 && pick[k].is_none() {
            pick[k] = Some(v);
        }
    }
    let fixlv: Vec<u16> = pick.iter().flatten().copied().collect();
    let sweep = |lv: &[u16], w: usize| -> Vec<u8> {
        let h = lv.len().div_ceil(w);
        let rows: Vec<Vec<u8>> = (0..h).map(|y| (0..w).flat_map(|x| lv[(y * w + x).min(lv.len() - 1)].to_be_bytes()).collect()).collect();
        png(w as u32, h as u32, 16, 0, false, &[], &png_filter(&rows, 2, &[0]), 4096)
    };
    let fixpng = sweep(&fixlv, 128);
    let quarter: Vec<u16> = (0..16384u32).map(|k| (k * 4 + k % 4) as u16).collect();
    let flpng = sweep(&quarter, 128);
    for (via, png, tos) in [("rgba64fixed", &fixpng, &["rgba64", "bgra", "rgb48", "bgr24"][..]), ("grayfix16", &fixpng, &["gray16", "gray8"]), ("rgbafloat", &flpng, &["rgba64", "bgra"]), ("rgba64half", &flpng, &["rgba64", "bgra"]), ("grayfloat", &flpng, &["gray16", "gray8"])] {
        for to in tos {
            writeln!(s, "case backsweep-{via}-{to}\nwdecode {}\nwframe 0\nwconv {via}\nwconv {to}\nwpixels\nend", hex(png));
        }
    }
    // (the other three quarters of the 16-bit levels as float and half: every level)
    for j in 1..4u32 {
        let lv: Vec<u16> = (0..16384u32).map(|k| (k * 4 + (k + j) % 4) as u16).collect();
        let png = sweep(&lv, 128);
        for (via, tos) in [("rgbafloat", ["rgba64", "bgra"]), ("rgba64half", ["rgba64", "bgra"]), ("grayfloat", ["gray16", "gray8"])] {
            for to in tos {
                writeln!(s, "case backsweep{j}-{via}-{to}\nwdecode {}\nwframe 0\nwconv {via}\nwconv {to}\nwpixels\nend", hex(&png));
            }
        }
    }
    // (where each 16-bit and 8-bit level starts as a float: the executor bisects the bits of
    // 0..1 per level, WICIMAGE.CONVERT.FLOAT.NARROW)
    for (from, to) in [("grayfloat", "gray16"), ("grayfloat", "gray8"), ("rgbafloat", "rgba64"), ("rgbafloat", "bgra")] {
        writeln!(s, "case steps-{from}-{to}\nwsteps {from} {to}\nend");
    }
    // (floats outside 0..1 back into 8 and 16 bits: a 32bppGrayFloat TIFF of them)
    let odd: Vec<u8> = [-1.0f32, -0.0, -1e-30, f32::from_bits(1), 1.0000001, 1.5, 2.0, 1e30, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, f32::from_bits(0xffc0_0000), 0.5, 1.0, 0.0, -0.5]
        .iter().flat_map(|f| f.to_le_bytes()).collect();
    let tf = tiff(false, &[TiffImage { w: 16, h: 1, photometric: 1, bits: 32, samples: 1, extra: None, planar: 1, rps: 1, px: odd, map: vec![], res: None, format: 3 }]);
    for (via, to) in [("", "gray16"), ("", "gray8"), ("", "rgba64"), ("rgbafloat", "rgba64"), ("rgbafloat", "bgra"), ("rgbafloat", "gray16"), ("rgba64half", "rgba64"), ("rgba64fixed", "rgba64")] {
        let conv = if via.is_empty() { String::new() } else { format!("wconv {via}\n") };
        writeln!(s, "case floatout-{}{to}\nwdecode {}\nwframe 0\n{conv}wconv {to}\nwpixels\nend", if via.is_empty() { String::new() } else { format!("{via}-") }, hex(&tf));
    }
    // (alpha outside 0..1: a 128bppRGBAFloat TIFF of gray 0.5 under those floats as alpha; and
    // 16bppGrayFixedPoint outside 0..1, from those gray floats)
    let vals = [-1.0f32, -0.0, -1e-30, f32::from_bits(1), 1.0000001, 1.5, 2.0, 1e30, f32::INFINITY, f32::NEG_INFINITY, f32::NAN, f32::from_bits(0xffc0_0000), 0.5, 1.0, 0.0, -0.5];
    let px: Vec<u8> = vals.iter().flat_map(|&a| [0.5f32, 0.5, 0.5, a]).flat_map(|f| f.to_le_bytes()).collect();
    let ta = tiff(false, &[TiffImage { w: 16, h: 1, photometric: 2, bits: 32, samples: 4, extra: Some(2), planar: 1, rps: 1, px, map: vec![], res: None, format: 3 }]);
    for to in ["rgba64", "bgra"] {
        writeln!(s, "case floatalpha-{to}\nwdecode {}\nwframe 0\nwinfo\nwconv {to}\nwpixels\nend", hex(&ta));
    }
    for to in ["gray16", "gray8", "bgra"] {
        writeln!(s, "case grayfixout-{to}\nwdecode {}\nwframe 0\nwconv grayfix16\nwpixels\nwconv {to}\nwpixels\nend", hex(&tf));
    }
    // every JpegYCrCbSubsampling on pictures of odd and even sizes past one block (the MCU's
    // edges, fancy upsampling's ends, gray ignoring it)
    for (k, (sub, f, w, h)) in [(0, "bgra", 6, 4), (2, "bgra", 6, 4), (4, "bgra", 6, 4), (2, "bgr24", 21, 13), (4, "bgr24", 21, 13), (1, "bgr24", 21, 13), (3, "bgr24", 21, 13), (2, "gray8", 9, 7), (4, "bgra", 2, 17), (2, "bgra", 2, 17)].into_iter().enumerate() {
        writeln!(s, "case opt-jpeg-subs-{k}-{sub}-{f}\nwbits {f} {w} {h} {}\nwencode jpeg JpegYCrCbSubsampling={sub}\nwbytes\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x7177_0200u32 + k as u32);
    }
    // option values past the enumerations and names an encoder does not take
    for (k, (c, o)) in [("jpeg", "JpegYCrCbSubsampling=5"), ("wmp", "ImageQuality=0.5"), ("png", "FilterOption=0"), ("gif", "InterlaceOption=1"), ("png", "FilterOption=7"), ("jpeg", "SuppressApp0=2")].into_iter().enumerate() {
        // (PNG's bytes are its zlib's, which the model does not write: only what decodes)
        let bytes = if c == "png" { "" } else { "wbytes\n" };
        writeln!(s, "case optother-{k}-bgra\nwbits bgra 6 4 {}\nwencode {c} {o}\n{bytes}wredecode\nwframe 0\nwinfo\nwpixels\nend", 0x0b7e_0000u32 + k as u32);
    }
    // the pairs of 16-bit formats (WIC.PAIR.WIDE): 64bppRGBA and 48bppRGB PNGs of random levels
    // into each other, 16bppGray and 64bppRGBAFixedPoint
    let mut rw = XorShift32::new(0x16b1);
    for (ct, n, name) in [(6u8, 4usize, "rgba64"), (2, 3, "rgb48")] {
        let raw: Vec<Vec<u8>> = (0..4).map(|_| bytes(&mut rw, 8 * n * 2)).collect();
        let f = png(8, 4, 16, ct, false, &[], &png_filter(&raw, n * 2, &[0]), 100);
        for to in ["rgba64", "rgb48", "gray16", "rgba64fixed"] {
            if to != name {
                writeln!(s, "case widepair-{name}-{to}\nwdecode {}\nwframe 0\nwinfo\nwconv {to}\nwpixels\nend", hex(&f));
            }
        }
    }
    // JPEG XR's lossy encoder on the formats past FORMATS (WICIMAGE.ENCODE.LOSSY): floats,
    // halves, fixed points, 101010, CMYK of 16 bits and with alpha (made from a 64bppRGBA bitmap by
    // the converter); what SetPixelFormat answers, the file and what decodes from it
    for (k, f) in ["bgr48", "bgra64", "prgba64", "pbgra64", "bgr101010", "grayfloat", "rgbafloat", "prgbafloat", "rgbfloat", "rgba64fixed", "bgra64fixed", "rgba64half", "rgb96float", "cmyk64", "cmyka40", "cmyka80", "grayfix16", "grayhalf", "grayfix32", "rgb48fixed", "rgb48half", "rgb64fixed", "rgb64half", "rgba128fixed", "rgb128fixed", "rgb96fixed"].into_iter().enumerate() {
        writeln!(s, "case encwide-wmp-{f}\nwbits rgba64 5 3 {}\nwconv {f}\nwencode wmp\nwbytes\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x3a8_0000u32 + k as u32);
    }
    // every option name the runner knows written to every encoder's bag (WICIMAGE.ENCODE.OPTIONS):
    // which bags take which names, with a value each takes where it does
    let names = [("ImageQuality", "1"), ("CompressionQuality", "0.5"), ("Lossless", "1"), ("BitmapTransform", "0"), ("InterlaceOption", "0"), ("FilterOption", "1"), ("TiffCompressionMethod", "1"), ("JpegYCrCbSubsampling", "3"), ("SuppressApp0", "0"), ("EnableV5Header32bppBGRA", "0"), ("Quality", "1"), ("Overlap", "1"), ("Subsampling", "3"), ("AlphaQuality", "1"), ("HeifCompressionMethod", "0")];
    for (k, c) in ENCODERS.iter().enumerate() {
        for (j, (n, v)) in names.iter().enumerate() {
            writeln!(s, "case optname-{c}-{n}\nwbits bgra 6 4 {}\nwencode {c} {n}={v}\nwredecode\nwframe 0\nwinfo\nwpixels\nend", 0x0b7f_0000u32 + (k * 16 + j) as u32);
        }
    }
    // the values of the names each bag has past those (WICIMAGE.ENCODE.OPTIONS): out of range,
    // the other transforms, JPEG XR's codec options at their other values
    let vals = [
        ("jpeg", "BitmapTransform=2"), ("jpeg", "BitmapTransform=3"), ("jpeg", "BitmapTransform=8"), ("jpeg", "BitmapTransform=16"), ("jpeg", "BitmapTransform=9"), ("jpeg", "BitmapTransform=5"),
        ("jpeg", "ImageQuality=0"), ("jpeg", "ImageQuality=1.5"), ("jpeg", "ImageQuality=-0.5"),
        ("tiff", "CompressionQuality=1.5"), ("tiff", "TiffCompressionMethod=6 CompressionQuality=0"), ("tiff", "TiffCompressionMethod=6 CompressionQuality=1"),
        ("wmp", "ImageQuality=1.5"), ("wmp", "ImageQuality=-0.5"), ("wmp", "Lossless=0"), ("wmp", "BitmapTransform=1"), ("wmp", "BitmapTransform=2"), ("wmp", "BitmapTransform=8"), ("wmp", "BitmapTransform=16"), ("wmp", "BitmapTransform=9"),
        ("wmp", "Quality=0"), ("wmp", "Quality=255"), ("wmp", "Overlap=0"), ("wmp", "Overlap=2"), ("wmp", "Overlap=3"), ("wmp", "Subsampling=0"), ("wmp", "Subsampling=1"), ("wmp", "Subsampling=2"), ("wmp", "Subsampling=4"), ("wmp", "AlphaQuality=0"), ("wmp", "AlphaQuality=255"),
    ];
    for (k, (c, o)) in vals.into_iter().enumerate() {
        let bytes = if c == "png" || c == "tiff" { "" } else { "wbytes\n" };
        writeln!(s, "case optval-{k}-{c}\nwbits bgra 6 4 {}\nwencode {c} {o}\n{bytes}wredecode\nwframe 0\nwinfo\nwpixels\nend", 0x0b80_0000u32 + k as u32);
    }
    s
}

/// the order of the encoder's grid cells of equal pixel count (WICIMAGE.ENCODE.GRID): one-row
/// pictures with chosen counts per cell, every pixel its own color so that the grid is taken;
/// a cell's pixels differ in the 3 bits below the 5 the grid looks at
fn gridtie(s: &mut String) {
    // (bits, the halvings per axis as the grid makes them: red, green, blue in turn)
    let grids = [("bw", [1u32, 0, 0]), ("idx4", [2, 1, 1]), ("idx8", [3, 3, 2])];
    // a cell's j-th color, BGR
    let color = |sp: [u32; 3], cell: usize, j: usize| -> [u8; 3] {
        let mut v = [0u8; 3];
        let mut c = cell;
        for k in (0..3).rev() {
            let at = c & ((1 << sp[k]) - 1);
            c >>= sp[k];
            let low = (j >> (3 * (2 - k))) & 7;
            v[k] = ((at << (8 - sp[k])) | low) as u8;
        }
        [v[2], v[1], v[0]]
    };
    let file = |px: &[[u8; 3]]| {
        let mut row = px.concat();
        row.resize(row.len().div_ceil(4) * 4, 0);
        bmp(40, px.len() as i32, 1, 24, 0, [0; 4], 0, &[], &row, false, (0, 0))
    };
    let case = |s: &mut String, name: &str, f: &str, px: &[[u8; 3]]| {
        writeln!(s, "case gridtie-{name}\nwdecode {}\nwframe 0\nwencode bmp {f}\nwredecode\nwframe 0\nwpixels\nwsrcpal\nend", hex(&file(px)));
    };
    let mut r = crate::rng::XorShift32::new(0x6e1d_7e00);
    for (f, sp) in grids {
        let n = 1usize << (sp[0] + sp[1] + sp[2]);
        // every cell the same count, the cells in an order: as numbered, backwards, shuffled
        let per = if n == 256 { 1 } else { 2 };
        let extra = usize::from(n == 256);
        for k in 0..6 {
            let mut order: Vec<usize> = (0..n).collect();
            match k {
                0 => {}
                1 => order.reverse(),
                _ => {
                    for i in (1..n).rev() {
                        order.swap(i, r.next() as usize % (i + 1));
                    }
                }
            }
            // (256 cells of one pixel each are 256 colors: one more pixel, in a cell of its own count 2)
            let mut px: Vec<[u8; 3]> = (0..per).flat_map(|j| order.iter().map(move |&c| (c, j))).map(|(c, j)| color(sp, c, j)).collect();
            if extra == 1 {
                px.push(color(sp, order[0], 1));
            }
            case(s, &format!("{f}-even-{k}"), f, &px);
        }
        // three cells of counts 3 c, 2 c, c and the rest empty: the order of the empty cells
        let c = if n == 256 { 50 } else if n == 16 { 3 } else { 1 };
        for k in 0..4 {
            let cells: Vec<usize> = if n == 2 { vec![k % 2, 1 - k % 2] } else {
                let mut v = vec![];
                while v.len() < 3 {
                    let x = r.next() as usize % n;
                    if !v.contains(&x) { v.push(x); }
                }
                v
            };
            let mut px = vec![];
            for (m, &cl) in cells.iter().enumerate() {
                for j in 0..c * (3 - m) {
                    px.push(color(sp, cl, j));
                }
            }
            case(s, &format!("{f}-few-{k}"), f, &px);
        }
    }
}
