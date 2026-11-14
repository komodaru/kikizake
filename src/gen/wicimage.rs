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
            writeln!(s, "case predefined-{t}-{a}\nwpalette {t} {a}\nend").unwrap();
        }
    }
    writeln!(s, "case predefined-large\nwpalette 0x7fffffff 0\nend").unwrap();
    // the palette of an indexed bitmap, and of one that has none
    for f in ["idx1", "idx2", "idx4", "idx8", "bgra", "gray8", "bw"] {
        writeln!(s, "case srcpal-{f}\nwbits {f} 4 2 1\nwsrcpal\nend").unwrap();
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
                writeln!(s, "case row-{m}-{sw}-{dw}\nwbits gray8 {sw} 1 ramp\nwscale {dw} 1 {m}\nwinfo\nwpixels\nend").unwrap();
            }
        }
    }
    // the same of random levels (the weights and their rounding), and two levels spread over
    // many output pixels (the positions' precision)
    for (k, m) in MODES.iter().enumerate() {
        for sw in 1..=8 {
            for dw in 1..=17 {
                writeln!(s, "case rrow-{m}-{sw}-{dw}\nwbits gray8 {sw} 1 {}\nwscale {dw} 1 {m}\nwpixels\nend", 1 + k * 1000 + sw * 20 + dw).unwrap();
            }
        }
        for (i, (sw, dw)) in [(2, 251), (3, 250), (2, 255), (5, 241), (251, 2), (250, 3), (64, 17), (17, 64)].into_iter().enumerate() {
            for v in 0..3 {
                writeln!(s, "case fine-{m}-{sw}-{dw}-{v}\nwbits gray8 {sw} 1 {}\nwscale {dw} 1 {m}\nwpixels\nend", 50_000 + k * 100 + i * 10 + v).unwrap();
            }
        }
    }
    // random levels along columns (the vertical pass), and random pictures scaled both ways at
    // once (its order and rounding between the passes, and the channels of color formats)
    for (k, m) in MODES.iter().enumerate() {
        for sh in 1..=8 {
            for dh in 1..=17 {
                writeln!(s, "case rcol-{m}-{sh}-{dh}\nwbits gray8 1 {sh} {}\nwscale 1 {dh} {m}\nwpixels\nend", 60_000 + k * 1000 + sh * 20 + dh).unwrap();
            }
        }
        // (64 random columns at once: each output row's weights solve from them)
        if *m == "cubic" {
            for sh in [2u32, 3, 4, 5, 7] {
                for dh in [1u32, 2, 3, 5, 8, 11, 16, 17] {
                    if sh != dh {
                        writeln!(s, "case vcols-{m}-{sh}-{dh}\nwbits gray8 64 {sh} {}\nwscale 64 {dh} {m}\nwpixels\nend", 80_000 + sh * 20 + dh).unwrap();
                    }
                }
            }
        }
        for f in ["gray8", "bgr24", "bgra", "pbgra"] {
            for (i, (sw, sh, dw, dh)) in [(3, 4, 7, 5), (5, 3, 2, 8), (4, 4, 9, 9), (8, 6, 3, 13), (6, 2, 6, 7), (7, 7, 3, 3)].into_iter().enumerate() {
                writeln!(s, "case rgrid-{m}-{f}-{sw}x{sh}-{dw}x{dh}\nwbits {f} {sw} {sh} {}\nwscale {dw} {dh} {m}\nwpixels\nend", 70_000 + k * 100 + i).unwrap();
            }
        }
    }
    // the same along columns, and in both directions at once
    for m in MODES {
        for (sh, dh) in [(3, 7), (7, 3), (5, 2), (2, 5), (4, 9)] {
            writeln!(s, "case col-{m}-{sh}-{dh}\nwbits gray8 1 {sh} ramp\nwscale 1 {dh} {m}\nwpixels\nend").unwrap();
            writeln!(s, "case both-{m}-{sh}-{dh}\nwbits bgra {sh} {dh} {}\nwscale {dh} {sh} {m}\nwpixels\nend", sh * 10 + dh).unwrap();
        }
    }
    // every format through every mode: the output format, and the pixels
    for (i, f) in XFORMATS.iter().enumerate() {
        for (k, m) in MODES.iter().enumerate() {
            writeln!(s, "case fmt-{f}-{m}\nwbits {f} 7 5 {}\nwscale 11 3 {m}\nwinfo\nwpixels\nend", 100 + i * 5 + k).unwrap();
        }
    }
    // large ratios (Fant and high quality cubic average many pixels)
    for m in MODES {
        writeln!(s, "case down-{m}\nwbits gray8 64 1 ramp\nwscale 5 1 {m}\nwpixels\nend").unwrap();
        writeln!(s, "case down2-{m}\nwbits bgra 40 30 77\nwscale 3 4 {m}\nwpixels\nend").unwrap();
        writeln!(s, "case up-{m}\nwbits bgra 2 2 78\nwscale 9 9 {m}\nwpixels\nend").unwrap();
        writeln!(s, "case alpha-{m}\nwbits bgra 6 6 79\nwscale 4 9 {m}\nwpixels\nend").unwrap();
    }
    // sizes Windows refuses, and rectangles of a scaled picture
    for (w, h) in [(0, 4), (4, 0), (0, 0), (0x10000, 1), (1, 0x10000)] {
        writeln!(s, "case size-{w}-{h}\nwbits gray8 4 4 ramp\nwscale {w} {h} nn\nend").unwrap();
    }
    // (how large a scaler Windows makes)
    for (w, h) in [(0x10000, 1), (1, 0x10000), (100_000, 1), (1, 100_000), (0x10000, 0x10000), (65535, 65535), (0x7fff_ffff, 1), (1, 0x7fff_ffff), (0xffff_ffffu32, 1)] {
        writeln!(s, "case bigsize-{w}-{h}\nwbits gray8 4 4 ramp\nwscale {w} {h} nn\nwinfo\nend").unwrap();
    }
    let mut r = XorShift32::new(0x5ca1e);
    for i in 0..60 {
        let m = r.pick(MODES);
        let (sw, sh, dw, dh) = (r.range(1, 12), r.range(1, 12), r.range(1, 20), r.range(1, 20));
        let (x, y) = (r.range(0, dw - 1), r.range(0, dh - 1));
        let (w, h) = (r.range(1, dw - x), r.range(1, dh - y));
        writeln!(s, "case rect-{i}\nwbits bgr24 {sw} {sh} {}\nwscale {dw} {dh} {m}\nwpixels {x} {y} {w} {h}\nend", r.next()).unwrap();
    }
    s
}

pub fn wictransform() -> String {
    let mut s = format!("# kikizake suite wictransform: WIC flip/rotators, clippers, CopyPixels. {HEAD}\n");
    // every transform of every format, an odd-sized picture
    let opts = [0u32, 1, 2, 3, 8, 9, 10, 11, 16, 17, 18, 19, 24, 25, 26, 27];
    for (i, f) in XFORMATS.iter().enumerate() {
        for o in opts {
            writeln!(s, "case flip-{f}-{o}\nwbits {f} 5 3 {}\nwflip {o}\nwinfo\nwpixels\nend", 300 + i * 32 + o as usize).unwrap();
        }
    }
    for o in [4u32, 5, 7, 32, 0x7fffffff] {
        writeln!(s, "case flip-bad-{o}\nwbits gray8 3 2 ramp\nwflip {o}\nwinfo\nend").unwrap();
    }
    // (what the options outside the documented ones do to the pixels)
    for o in [4u32, 5, 6, 7, 12, 13, 20, 28, 31, 32, 33, 64, 0x80, 0x100, 0x8000_0000, 0x7fff_ffff, 0xffff_ffff] {
        writeln!(s, "case flipbad-{o}\nwbits gray8 3 2 ramp\nwflip {o}\nwinfo\nwpixels\nend").unwrap();
    }
    // rectangles of a flipped picture (the rotator reads its source by rows or columns)
    let mut r = XorShift32::new(0xf11b);
    for i in 0..40 {
        let o = *r.pick(&opts);
        let f = r.pick(XFORMATS);
        let (w, h) = (r.range(1, 9), r.range(1, 9));
        let (dw, dh) = if o & 1 != 0 { (h, w) } else { (w, h) };
        let (x, y) = (r.range(0, dw - 1), r.range(0, dh - 1));
        writeln!(s, "case fliprect-{i}\nwbits {f} {w} {h} {}\nwflip {o}\nwpixels {x} {y} {} {}\nend", r.next(), r.range(1, dw - x), r.range(1, dh - y)).unwrap();
    }
    // clippers: inside, at the edges, outside, empty
    for (i, f) in XFORMATS.iter().enumerate() {
        writeln!(s, "case clip-{f}\nwbits {f} 9 7 {}\nwclip 1 2 5 4\nwinfo\nwpixels\nwpixels 1 1 3 2\nend", 900 + i).unwrap();
    }
    let rects: &[(i64, i64, i64, i64)] = &[(0, 0, 6, 5), (5, 4, 1, 1), (0, 0, 0, 0), (0, 0, 7, 5), (0, 0, 6, 6), (-1, 0, 2, 2), (0, -1, 2, 2), (6, 0, 1, 1), (2, 2, 0, 3), (3, 3, 4, 4), (0, 0, 0x7fffffff, 1)];
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        writeln!(s, "case cliprect-{i}\nwbits bgra 6 5 7\nwclip {x} {y} {w} {h}\nwinfo\nwpixels\nend").unwrap();
    }
    // CopyPixels: rectangles, strides and buffers Windows takes or refuses
    for (i, (x, y, w, h)) in rects.iter().enumerate() {
        writeln!(s, "case copyrect-{i}\nwbits bgr24 6 5 8\nwpixels {x} {y} {w} {h}\nend").unwrap();
    }
    for f in ["bw", "gray4", "bgr24", "rgba64"] {
        let st = crate::model::wic::stride(crate::model::wic::format(f).unwrap(), 5) as i64;
        for (k, (stride, size)) in [(st, st * 3), (st - 1, st * 3), (st + 3, (st + 3) * 3), (st + 3, (st + 3) * 2 + st), (st + 3, (st + 3) * 2 + st - 1), (st, st * 3 - 1), (0, 1000), (st, 0)].into_iter().enumerate() {
            writeln!(s, "case stride-{f}-{k}\nwbits {f} 5 3 3\nwpixels 0 0 5 3 {stride} {size}\nend").unwrap();
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
    s
}

pub fn wiccolor() -> String {
    let mut s = format!("# kikizake suite wiccolor: WIC color contexts and transforms. {HEAD}\n");
    for n in [0u32, 1, 2, 3, 4, 5, 100, 65535, 2147483647] {
        writeln!(s, "case ctx-{n}\nwctx {n}\nend").unwrap();
    }
    let srcs = ["bgra", "bgr24", "rgb24", "gray8", "gray16", "pbgra", "rgba", "bgr565", "rgba64", "cmyk", "idx8", "bw"];
    let dsts = ["bgra", "bgr24", "gray8", "pbgra", "rgba64", "cmyk", "rgb48", "idx8"];
    for (i, f) in srcs.iter().enumerate() {
        for (k, t) in dsts.iter().enumerate() {
            for (a, b) in [(1, 1), (1, 2), (2, 1)] {
                writeln!(s, "case xf-{f}-{t}-{a}{b}\nwbits {f} 8 2 {}\nwcolor {a} {b} {t}\nwinfo\nwpixels\nend", 1 + i * 64 + k * 4 + a * 2 + b).unwrap();
            }
        }
    }
    for (a, b) in [(0, 1), (1, 0), (3, 1), (1, 3)] {
        writeln!(s, "case xf-bad-{a}{b}\nwbits bgra 2 2 1\nwcolor {a} {b} bgra\nend").unwrap();
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
                writeln!(s, "case cm-{name}-{a}{b}-{t}\nwdecode {}\nwframe 0\nwcolor {a} {b} {t}\nwpixels\nend", hex(&f)).unwrap();
            }
        }
    }
    // every 8-bit level through sRGB to Adobe RGB and back
    writeln!(s, "case levels-12\nwbits bgr24 256 1 ramp\nwcolor 1 2 bgr24\nwpixels\nend").unwrap();
    writeln!(s, "case levels-21\nwbits bgr24 256 1 ramp\nwcolor 2 1 bgr24\nwpixels\nend").unwrap();
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

fn adler32(b: &[u8]) -> u32 {
    let (mut a, mut s) = (1u32, 0u32);
    for &x in b {
        a = (a + x as u32) % 65521;
        s = (s + a) % 65521;
    }
    s << 16 | a
}

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
    writeln!(s, "case {name}\nwdecode {}", hex(file)).unwrap();
    for f in 0..frames {
        writeln!(s, "wframe {f}\nwinfo\nwpixels").unwrap();
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
            let local = if gl.is_none() || r.chance(3) { { let n = r.range(2, 64) as usize; Some(rand_pal(&mut r, n)) } } else { None };
            let n = local.as_ref().or(gl.as_ref()).unwrap().len();
            let bits = (1..=8).find(|b| 1usize << b >= n).unwrap() as u32;
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
            decode_case(&mut s, &format!("dds-{}-{w}x{h}-{mips}", std::str::from_utf8(fc).unwrap().to_lowercase()), &dds(w, h, mips, Some(fc), &data), mips, false);
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
        decode_case(&mut s, &format!("dds-{}", std::str::from_utf8(fc).unwrap().to_lowercase()), &dds(4, 4, 1, Some(fc), &bytes(&mut r, 32)), 1, false);
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
    s
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
            let bytes = if *c == "bmp" { "wbytes\n" } else { "" };
            writeln!(s, "case enc-{c}-{f}\nwbits {f} 5 3 {seed}\nwencode {c}\n{bytes}wredecode\nwframe 0\nwinfo\nwpixels\nend").unwrap();
            seed += 1;
        }
    }
    // a format other than the source's asked of SetPixelFormat
    for c in ["bmp", "png", "tiff"] {
        for f in ["bgra", "gray8", "idx8", "rgba64", "bw"] {
            writeln!(s, "case encas-{c}-{f}\nwbits bgr24 4 2 {seed}\nwencode {c} {f}\nwredecode\nwframe 0\nwinfo\nwpixels\nend").unwrap();
            seed += 1;
        }
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
            writeln!(s, "case opt-{k}-{f}\nwbits {f} 6 4 {seed}\nwencode {c} {o}\nwredecode\nwframe 0\nwinfo\nwpixels\nend").unwrap();
            seed += 1;
        }
    }
    // a larger picture through each lossless encoder (PNG and TIFF strips, GIF's LZW past its
    // first codes)
    for c in ["bmp", "png", "tiff", "gif"] {
        let f = if c == "gif" { "idx8" } else { "bgra" };
        writeln!(s, "case big-{c}\nwbits {f} 40 30 {seed}\nwencode {c}\nwredecode\nwframe 0\nwinfo\nwpixels\nend").unwrap();
        seed += 1;
    }
    s
}
