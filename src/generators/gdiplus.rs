//! The gdiplus suite's case generator.
use super::*;
use crate::model::gdiplus::Pf;

fn rargb(r: &mut XorShift32) -> u32 {
    match r.range(0, 4) {
        0 => *r.pick(&[0, 0xff00_0000, 0xffff_ffff, 0x00ff_ffff, 0x80ff_0000, 0x7f00_ff00, 0x0100_00ff, 0xfe80_8080]),
        1 => 0xff00_0000 | r.next() & 0xff_ffff,
        _ => r.next(),
    }
}

/// floats the matrix cases use: small integers, halves, and random ones near 1
fn rfloat(r: &mut XorShift32) -> u32 {
    match r.range(0, 5) {
        0 => *r.pick(&[0, 0x8000_0000, 1.0f32.to_bits(), (-1.0f32).to_bits(), 2.0f32.to_bits(), 0.5f32.to_bits(), 3.0f32.to_bits(), 10.0f32.to_bits()]),
        1 => (r.range(-8, 8) as f32).to_bits(),
        2 => (r.range(-64, 64) as f32 / 4.0).to_bits(),
        _ => (((r.next() & 0xffff) as f32 / 65536.0 - 0.5) * 8.0).to_bits(),
    }
}

pub fn gdiplus() -> String {
    let mut s = String::from("# kikizake suite gdiplus: GDI+ bitmaps, pixel formats, fills, rotations, clones, matrices (SPEC.md). Made by `kikizake gen gdiplus`.\n");
    let names: Vec<&str> = Pf::ALL.iter().map(|f| f.name()).collect();
    let mut seed = 1u32;
    let mut next = || {
        seed += 1;
        seed
    };
    // a new bitmap of every format: what it holds, its palette
    for f in &names {
        writeln!(s, "case create-{f}\ngpbitmap 0 5 3 {f}\ngpinfo 0\ngpread 0 {f}\nend");
    }
    for (i, (w, h)) in [(0, 1), (1, 0), (-1, 4), (4, -1), (1, 1), (1, 1000)].iter().enumerate() {
        writeln!(s, "case size-{i}\ngpbitmap 0 {w} {h} 32bppARGB\ngpinfo 0\nend");
    }
    // every pair through LockBits: written in the bitmap's own format, read in another; written
    // in another, read in its own
    for from in &names {
        for to in &names {
            writeln!(s, "case read-{from}-{to}\ngpbitmap 0 9 3 {from}\ngpwrite 0 {from} seed {}\ngpread 0 {to}\nend", next());
            writeln!(s, "case write-{from}-{to}\ngpbitmap 0 9 3 {from}\ngpwrite 0 {to} seed {}\ngpread 0 {from}\nend", next());
        }
    }
    // every (color, alpha) pair between straight and premultiplied, and to 16-bit channels; every
    // 16-bit level back to 8 bits
    for (from, to) in [("32bppARGB", "32bppPARGB"), ("32bppPARGB", "32bppARGB"), ("32bppARGB", "64bppARGB"), ("32bppARGB", "64bppPARGB"), ("32bppPARGB", "64bppPARGB")] {
        writeln!(s, "case pairs-{from}-{to}\ngpbitmap 0 256 256 {from}\ngpwrite 0 {from} pairs\ngpread 0 {to}\nend");
    }
    for f in ["48bppRGB", "64bppARGB", "64bppPARGB"] {
        writeln!(s, "case levels-{f}\ngpbitmap 0 256 33 {f}\ngpwrite 0 {f} levels\ngpread 0 32bppARGB\ngpread 0 {f}\nend");
    }
    // every 5-bit color to the palettes (the nearest entry)
    for f in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
        writeln!(s, "case grid-{f}\ngpbitmap 0 256 128 24bppRGB\ngpwrite 0 24bppRGB grid\ngpread 0 {f}\nend");
    }
    // the bytes of a format as GetPixel reads them, and SetPixel
    let mut r = XorShift32::new(0x6d1f_0001);
    for f in &names {
        for k in 0..3 {
            write!(s, "case pixel-{f}-{k}\ngpbitmap 0 4 3 {f}\ngpwrite 0 {f} seed {}\n", r.next());
            for _ in 0..4 {
                let (x, y) = (r.range(-1, 4), r.range(-1, 3));
                writeln!(s, "gpget 0 {x} {y}");
            }
            for _ in 0..6 {
                let (x, y) = (r.range(-1, 4), r.range(-1, 3));
                writeln!(s, "gpset 0 {x} {y} {:#010x}\ngpget 0 {x} {y}", rargb(&mut r));
            }
            writeln!(s, "gpread 0 {f}\nend");
        }
    }
    // locked rectangles inside, across and outside the bitmap
    for i in 0..40 {
        let f = *r.pick(&names);
        let (x, y, w, h) = (r.range(-2, 6), r.range(-2, 4), r.range(-1, 7), r.range(-1, 5));
        let g = *r.pick(&names);
        writeln!(s, "case lock-{i}\ngpbitmap 0 6 4 {f}\ngpwrite 0 {f} seed {}\ngpread 0 {g} at {x} {y} {w} {h}\ngpwrite 0 {g} seed {} at {x} {y} {w} {h}\ngpread 0 {f}\nend", r.next(), r.next());
    }
    // fills and clears: every format, both compositing modes, every quality
    for f in &names {
        for (m, mode) in ["over", "copy"].iter().enumerate() {
            for q in 0..5 {
                let (x, y, w, h) = (r.range(-3, 8), r.range(-2, 4), r.range(-2, 10), r.range(-2, 6));
                writeln!(s, "case fill-{f}-{mode}-{q}\ngpbitmap 0 10 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 {x} {y} {w} {h} {:#010x} {mode} {q}\ngpread 0 {f}\nend", r.next(), rargb(&mut r));
                if m == 0 && q == 0 {
                    writeln!(s, "case clear-{f}\ngpbitmap 0 3 2 {f}\ngpclear 0 {:#010x}\ngpread 0 {f}\nend", rargb(&mut r));
                }
            }
        }
    }
    // every gray level in an 8x8 block, on the 16-bit formats (where drawing dithers), and with
    // the blocks moved off the 8-pixel grid
    for (f, o) in [("16bppRGB555", 0), ("16bppRGB565", 0), ("16bppRGB565", 3)] {
        write!(s, "case dither-{f}-{o}\ngpbitmap 0 {} {} {f}\n", 128 + o, 128 + o);
        for v in 0..256 {
            writeln!(s, "gpfill 0 {} {} 8 8 {:#010x} copy 0", o + v % 16 * 8, o + v / 16 * 8, 0xff00_0000u32 | (v * 0x01_0101));
        }
        writeln!(s, "gpread 0 {f}\nend");
    }
    // source over every (color, alpha) of the destination, for a few sources
    for (i, c) in [0x80ff_4010u32, 0x40_20c0f0, 0xc0_7f8001, 0x01_ffffff, 0xfe_000000].iter().enumerate() {
        let q = if i == 4 { 3 } else { 0 };
        writeln!(s, "case over-{i}\ngpbitmap 0 256 256 32bppARGB\ngpwrite 0 32bppARGB pairs\ngpfill 0 0 0 256 256 {c:#010x} over {q}\ngpread 0 32bppARGB\nend");
    }
    for i in 0..60 {
        let f = *r.pick(&["32bppARGB", "32bppPARGB", "32bppRGB", "24bppRGB", "16bppRGB565", "16bppARGB1555", "64bppARGB"]);
        let q = r.range(0, 4);
        writeln!(s, "case blend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 0 0 16 4 {:#010x} over {q}\ngpread 0 {f}\nend", r.next(), r.next());
    }
    // rotations and flips
    for f in &names {
        for t in 0..8 {
            writeln!(s, "case rotate-{f}-{t}\ngpbitmap 0 5 3 {f}\ngpwrite 0 {f} seed {}\ngprotate 0 {t}\ngpread 0 {f}\nend", r.next());
        }
    }
    // clones of an area into any format
    for i in 0..80 {
        let (f, g) = (*r.pick(&names), *r.pick(&names));
        let (x, y, w, h) = (r.range(-1, 6), r.range(-1, 4), r.range(-1, 7), r.range(-1, 5));
        writeln!(s, "case clone-{i}\ngpbitmap 0 7 4 {f}\ngpwrite 0 {f} seed {}\ngpclone 1 0 {x} {y} {w} {h} {g}\ngpinfo 1\ngpread 1 {g}\nend", r.next());
    }
    // clones into the indexed formats: of every other format, whole, and of every 5-bit color
    for f in &names {
        for g in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
            if *f == "16bppGrayScale" {
                continue;
            }
            writeln!(s, "case cloneidx-{f}-{g}\ngpbitmap 0 16 16 {f}\ngpwrite 0 {f} seed {}\ngpclone 1 0 0 0 16 16 {g}\ngpinfo 1\ngpread 1 {g}\nend", r.next());
        }
    }
    for g in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
        writeln!(s, "case clonegrid-{g}\ngpbitmap 0 256 128 24bppRGB\ngpwrite 0 24bppRGB grid\ngpclone 1 0 0 0 256 128 {g}\ngpinfo 1\ngpread 1 {g}\nend");
        writeln!(s, "case clonealpha-{g}\ngpbitmap 0 256 256 32bppARGB\ngpwrite 0 32bppARGB pairs\ngpclone 1 0 0 0 256 256 {g}\ngpread 1 {g}\nend");
    }
    // the gamma-corrected blend over every (color, alpha) of the destination, for a few brushes
    for (i, (f, c, q)) in [("32bppARGB", 0xfe00_0000u32, 2), ("32bppARGB", 0x80ff_ffff, 3), ("32bppARGB", 0x8000_0000, 3), ("32bppARGB", 0x40ff_8000, 3), ("32bppARGB", 0xc010_80f0, 2), ("32bppARGB", 0x01ff_ffff, 3), ("32bppPARGB", 0x80ff_ffff, 3), ("32bppPARGB", 0xc010_80f0, 3)].iter().enumerate() {
        writeln!(s, "case gamma-{i}\ngpbitmap 0 256 256 {f}\ngpwrite 0 {f} pairs\ngpfill 0 0 0 256 256 {c:#010x} over {q}\ngpread 0 {f}\nend");
    }
    for i in 0..60 {
        let f = *r.pick(&names);
        writeln!(s, "case gammablend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 0 0 16 4 {:#010x} over {}\ngpread 0 {f}\nend", r.next(), rargb(&mut r), r.range(2, 4));
    }
    // the gamma-corrected blend for every (brush alpha, destination alpha): column x drawn with
    // brush alpha x over the destination's alpha y, for a few brush colors
    for (i, c) in [0x00_0000u32, 0xff_ffff, 0x80_8080, 0x10_80f0].iter().enumerate() {
        writeln!(s, "case gammaalpha-{i}\ngpbitmap 0 256 256 32bppARGB\ngpwrite 0 32bppARGB pairs");
        for x in 0..256u32 {
            writeln!(s, "gpfill 0 {x} 0 1 256 {:#010x} over 2", x << 24 | c);
        }
        writeln!(s, "gpread 0 32bppARGB\nend");
    }
    // matrices: made, multiplied both ways, inverted, points through them (more in gdiplusmat)
    for i in 0..120 {
        let m: Vec<String> = (0..12).map(|k| {
            // (some singular ones: a row that is a multiple of the other)
            if i % 6 == 5 && (k == 2 || k == 3) { "0".into() } else { format!("{:#x}", rfloat(&mut r)) }
        }).collect();
        let pts: Vec<String> = (0..6).map(|_| format!("{:#x}", rfloat(&mut r))).collect();
        writeln!(s, "case matrix-{i}\ngpmatrix 0 {}\ngpmatrix 1 {}\ngpmatpts 0 {}\ngpmatmul 0 1 {}\ngpmatpts 0 {}\ngpmatinv 0\ngpmatpts 0 {}\nend", m[..6].join(" "), m[6..].join(" "), pts.join(" "), i % 2, pts.join(" "), pts.join(" "));
    }
    s
}

/// matrix arithmetic: products both ways, inverses, points (x86 rounds to nearest, x86-64 down)
pub fn gdiplusmat() -> String {
    let mut s = String::from("# kikizake suite gdiplusmat: GDI+ matrix arithmetic (SPEC.md). Made by `kikizake gen gdiplusmat`.\n");
    let mut r = XorShift32::new(0x1e7e_0001);
    for i in 0..300 {
        let m: Vec<String> = (0..12).map(|k| if i % 8 == 7 && (2..4).contains(&k) { "0".into() } else { format!("{:#x}", rfloat(&mut r)) }).collect();
        let pts: Vec<String> = (0..6).map(|_| format!("{:#x}", rfloat(&mut r))).collect();
        let (a, b, p) = (m[..6].join(" "), m[6..].join(" "), pts.join(" "));
        writeln!(s, "case matrix-{i}\ngpmatrix 0 {a}\ngpmatrix 1 {b}\ngpmatpts 0 {p}\ngpmatmul 0 1 {}\ngpmatpts 0 {p}\ngpmatinv 0\ngpmatpts 0 {p}\ngpmatinv 0\nend", i % 2);
    }
    writeln!(s, "case matrix-none\ngpmatpts 1 0 0\ngpmatinv 1\ngpmatmul 1 0 0\nend");
    writeln!(s, "case matrix-order\ngpmatrix 0 0x3f800000 0 0 0x3f800000 0 0\ngpmatrix 1 0x40000000 0 0 0x40000000 0 0\ngpmatmul 0 1 2\ngpmatmul 0 1 -1\nend");
    s
}

/// a coordinate on a 1/16 grid (exact in float and in decimal) around a 12-pixel bitmap
fn rcoord(r: &mut XorShift32) -> String {
    let v = r.range(-40, 12 * 16 + 40) as f32 / 16.0;
    format!("{v}")
}

/// a size on the same grid, now and then 0 or negative
fn rsize(r: &mut XorShift32) -> String {
    let v = r.range(-24, 10 * 16) as f32 / 16.0;
    format!("{v}")
}

pub fn gdipluspaint() -> String {
    let mut s = String::from("# kikizake suite gdipluspaint: GDI+ brushes and filled shapes: hatch brushes, rectangles in float coordinates under the smoothing and pixel offset modes (SPEC.md). Made by `kikizake gen gdipluspaint`.\n");
    let mut r = XorShift32::new(0x9a17_0001);
    let names: Vec<&str> = Pf::ALL.iter().map(|f| f.name()).collect();
    // every HatchStyle (and past both ends): black on white over 16 x 16
    for style in -1..=53 {
        writeln!(s, "case hatch-{style}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 hatch {style} 0xff000000 0xffffffff\ngppaint 0 0 0 0 16 16 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend");
    }
    // the pattern's anchor: the rendering origin, the rectangle's corner
    for i in 0..24 {
        let style = r.range(0, 53);
        let (ox, oy) = (r.range(-20, 20), r.range(-20, 20));
        let (x, y) = (r.range(0, 8), r.range(0, 8));
        writeln!(s, "case hatchorigin-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 hatch {style} 0xff000000 0xffffffff\ngppaint 0 0 {x} {y} {} {} copy 0 0 0 {ox} {oy}\ngpread 0 32bppARGB\nend", 16 - x, 16 - y);
    }
    // colors with alpha over every format, both compositing modes, every quality
    for i in 0..60 {
        let f = *r.pick(&names);
        let style = r.range(0, 53);
        let mode = *r.pick(&["over", "copy"]);
        writeln!(s, "case hatchblend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch {style} {:#010x} {:#010x}\ngppaint 0 0 0 0 16 4 {mode} {} 0 0 0 0\ngpread 0 {f}\nend", r.next(), rargb(&mut r), rargb(&mut r), r.range(0, 5));
    }
    // rectangles in float coordinates: which pixels each smoothing and pixel offset mode fills
    for i in 0..240 {
        let (x, y, w, h) = (rcoord(&mut r), rcoord(&mut r), rsize(&mut r), rsize(&mut r));
        let sm = r.range(0, 7);
        let po = r.range(0, 5);
        writeln!(s, "case rectf-{i}\ngpbitmap 0 12 12 32bppARGB\ngpbrush 0 solid 0xff0000ff\ngppaint 0 0 {x} {y} {w} {h} copy 0 {sm} {po} 0 0\ngpread 0 32bppARGB\nend");
    }
    // a solid brush with alpha, antialiased, over a destination (coverage times alpha)
    for i in 0..40 {
        let (x, y, w, h) = (rcoord(&mut r), rcoord(&mut r), rsize(&mut r), rsize(&mut r));
        let mode = *r.pick(&["over", "copy"]);
        writeln!(s, "case rectfblend-{i}\ngpbitmap 0 12 12 32bppARGB\ngpwrite 0 32bppARGB seed {}\ngpbrush 0 solid {:#010x}\ngppaint 0 0 {x} {y} {w} {h} {mode} {} {} {} 0 0\ngpread 0 32bppARGB\nend", r.next(), rargb(&mut r), r.range(0, 5), r.range(0, 7), r.range(0, 5));
    }
    // the antialiased styles between colors that are not opaque, read as they are (copy into
    // 32bppPARGB) and as straight colors
    for i in 0..32 {
        let style = *r.pick(&[2, 3, 5]);
        let f = *r.pick(&["32bppPARGB", "32bppARGB"]);
        writeln!(s, "case hatchpartial-{i}\ngpbitmap 0 8 8 {f}\ngpbrush 0 hatch {style} {:#010x} {:#010x}\ngppaint 0 0 0 0 8 8 copy 0 0 0 0 0\ngpread 0 {f}\nend", r.next(), r.next());
    }
    // hatches over 16bppRGB555 / 565, each beside a solid fill of its fore color
    for i in 0..32 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let (fore, back) = (r.next(), r.next());
        let seed = r.next();
        writeln!(s, "case hatch16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 0 hatch {} {fore:#010x} {back:#010x}\ngppaint 0 0 0 0 16 8 over {} 0 0 0 0\ngpread 0 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 1 solid {fore:#010x}\ngppaint 0 1 0 0 16 8 over 0 0 0 0 0\ngpread 0 {f}\nend", r.range(0, 52), r.range(0, 4));
    }
    // gamma corrected hatches over 16bppRGB555 / 565 of one color, and of two, each beside a
    // solid fill of the same color
    for i in 0..24 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let c = rargb(&mut r);
        let back = if i % 2 == 0 { c } else { rargb(&mut r) };
        let seed = r.next();
        writeln!(s, "case hatchgamma16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 0 hatch 0 {c:#010x} {back:#010x}\ngppaint 0 0 0 0 16 8 over 2 0 0 0 0\ngpread 0 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 1 solid {back:#010x}\ngppaint 0 1 0 0 16 8 over 2 0 0 0 0\ngpread 0 {f}\nend");
    }
    // opaque fore, translucent back, gamma corrected over 16bppRGB555 / 565, every few styles
    for i in 0..32 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let fore = 0xff00_0000 | r.next() & 0xff_ffff;
        let back = (r.range(1, 254) as u32) << 24 | r.next() & 0xff_ffff;
        let seed = r.next();
        writeln!(s, "case hatchopaque16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 0 hatch {} {fore:#010x} {back:#010x}\ngppaint 0 0 0 0 16 8 over {} 0 0 0 0\ngpread 0 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 1 solid {back:#010x}\ngppaint 0 1 0 0 16 8 over 2 0 0 0 0\ngpread 0 {f}\nend", r.range(0, 52), r.range(2, 3));
    }
    // the same, in rectangles off the bitmap's corner: where a run starts
    for i in 0..24 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let fore = 0xff00_0000 | r.next() & 0xff_ffff;
        let back = (r.range(1, 254) as u32) << 24 | r.next() & 0xff_ffff;
        let (x, y) = (r.range(1, 7), r.range(0, 3));
        writeln!(s, "case hatchrun16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch {} {fore:#010x} {back:#010x}\ngppaint 0 0 {x} {y} {} 5 over {} 0 0 {} {}\ngpread 0 {f}\nend", r.next(), r.range(0, 52), r.range(3, 16 - x), r.range(2, 3), r.range(-4, 4), r.range(-4, 4));
    }
    // one run per row (fore and back opaque), from each column 0..7 and each origin x 0..7: how
    // far the dither moves with the fill's left edge and with the rendering origin
    for i in 0..64 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let fore = 0xff00_0000 | r.next() & 0xff_ffff;
        let back = 0xff00_0000 | r.next() & 0xff_ffff;
        let (x, ox) = (i % 8, i / 8);
        writeln!(s, "case hatchshift16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch {} {fore:#010x} {back:#010x}\ngppaint 0 0 {x} 0 {} 8 over {} 0 0 {ox} {}\ngpread 0 {f}\nend", r.next(), r.range(0, 52), 16 - x, r.range(2, 3), r.range(-4, 4));
    }
    // horizontal lines (a row all fore, the rest translucent): one opaque run per line row, from
    // each column and origin, rows moved by the origin's y
    for i in 0..32 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let fore = 0xff00_0000 | r.next() & 0xff_ffff;
        let back = (r.range(1, 254) as u32) << 24 | r.next() & 0xff_ffff;
        let (x, ox, oy) = (i % 8, (i / 8) as i64 * 3 - 4, r.range(-4, 4));
        writeln!(s, "case hatchline16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch 0 {fore:#010x} {back:#010x}\ngppaint 0 0 {x} 0 {} 8 over {} 0 0 {ox} {oy}\ngpread 0 {f}\nend", r.next(), 16 - x, r.range(2, 3));
    }
    // the antialiased styles (texels between fore and back) over a translucent back, gamma
    // corrected over 555/565, from column 0 and origin 0 and off them
    for i in 0..48 {
        let f = *r.pick(&["16bppRGB555", "16bppRGB565"]);
        let fore = (r.range(1, 255) as u32) << 24 | r.next() & 0xff_ffff;
        let back = (r.range(1, 254) as u32) << 24 | r.next() & 0xff_ffff;
        let style = *r.pick(&[2, 3, 5]);
        let (x, ox, oy) = if i < 16 { (0, 0, 0) } else { (r.range(0, 7), r.range(-4, 4), r.range(-4, 4)) };
        writeln!(s, "case hatchbetween16-{i}\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch {style} {fore:#010x} {back:#010x}\ngppaint 0 0 {x} 0 {} 8 over {} 0 0 {ox} {oy}\ngpread 0 {f}\nend", r.next(), 16 - x, r.range(2, 3));
    }
    // hatchrun16-22's brush (the pixel GP.PAINT.GAMMA.BETWEEN is about): the same fill over
    // other destinations, the same destination with other origins, and whole-bitmap fills
    for i in 0..48 {
        let seed = if i == 0 || (16..32).contains(&i) { 564_120_086 } else { r.next() };
        let (ox, oy) = if (16..32).contains(&i) { (i % 4 - 1, i / 4 % 4 - 1) } else { (3, 3) };
        let (x, w) = if i < 32 { (7, 6) } else { (0, 16) };
        writeln!(s, "case hatchedge16-{i}\ngpbitmap 0 16 8 16bppRGB555\ngpwrite 0 16bppRGB555 seed {seed}\ngpbrush 0 hatch 5 0xfff7eb79 0x169c4d6e\ngppaint 0 0 {x} 0 {w} 5 over 2 0 0 {ox} {oy}\ngpread 0 16bppRGB555\nend");
    }
    // GP.PAINT.GAMMA.BETWEEN's carry: texels whose alpha mix fa 234 + ba 22 (always even) falls 4,
    // 6, 8 or 10 short of a multiple of 256, gamma corrected over 32bppARGB / PARGB / 555
    for i in 0..48 {
        let short = [4, 6, 8, 10][i / 12];
        let (fa, ba) = loop {
            let (fa, ba) = (r.range(1, 255) as u32, r.range(1, 254) as u32);
            if 256 - (fa * 234 + ba * 22) % 256 == short {
                break (fa, ba);
            }
        };
        let fore = fa << 24 | r.next() & 0xff_ffff;
        let back = ba << 24 | r.next() & 0xff_ffff;
        let f = ["32bppARGB", "32bppPARGB", "16bppRGB555"][i % 3];
        writeln!(s, "case hatchcarry-{i}\ngpbitmap 0 8 8 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 hatch 5 {fore:#010x} {back:#010x}\ngppaint 0 0 0 0 8 8 over {} 0 0 0 0\ngpread 0 {f}\nend", r.next(), r.range(2, 3));
    }
    writeln!(s, "case paint-nobrush\ngpbitmap 0 4 4 32bppARGB\ngppaint 0 1 0 0 4 4 copy 0 0 0 0 0\nend");
    writeln!(s, "case brush-solid\ngpbrush 1 solid 0x12345678\ngpbitmap 0 4 4 32bppARGB\ngppaint 0 1 1 1 2 2 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend");
    s
}

/// a ColorMatrix entry: exact in binary (k / 256) so strtof and Rust read the same float
fn rentry(r: &mut XorShift32) -> String {
    let v = match r.range(0, 5) {
        0 => *r.pick(&[0.0, 1.0, -1.0, 0.5, 2.0, 0.25]),
        1 => r.range(-4, 4) as f32 / 4.0,
        _ => r.range(-512, 512) as f32 / 256.0,
    };
    // (written out in full: k / 256 has at most 8 decimals)
    format!("{}", f64::from(v))
}

/// a 5 x 5 ColorMatrix: the identity, a scale, a translation, or all entries random
fn rmatrix(r: &mut XorShift32) -> Vec<String> {
    let kind = r.range(0, 3);
    (0..25)
        .map(|k| {
            let (row, col) = (k / 5, k % 5);
            match kind {
                0 if row == col => rentry(r),
                0 => "0".into(),
                1 if row == col => "1".into(),
                1 if row == 4 && col < 4 => rentry(r),
                1 => "0".into(),
                _ if col == 4 => if row == 4 { "1".into() } else { "0".into() },
                _ => rentry(r),
            }
        })
        .collect()
}

pub fn gdiplusdraw() -> String {
    let mut s = String::from("# kikizake suite gdiplusdraw: GDI+ DrawImage of one bitmap onto another, with image attributes (color matrices) (SPEC.md). Made by `kikizake gen gdiplusdraw`.\n");
    let mut r = XorShift32::new(0xd7a3_0001);
    let fmts = ["32bppARGB", "32bppPARGB", "32bppRGB", "24bppRGB"];
    let id = "1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1";
    writeln!(s, "case attr-none\ngpattr 0 none\nend");
    writeln!(s, "case attr-identity\ngpattr 0 matrix 0 {id}\nend");
    for flags in [1, 2, 3, -1] {
        writeln!(s, "case attr-flags{flags}\ngpattr 0 matrix {flags} {id}\nend");
    }
    writeln!(s, "case draw-noattr\ngpbitmap 0 4 4 32bppARGB\ngpbitmap 1 4 4 32bppARGB\ngpdraw 0 1 1 0 0 4 4 0 0 4 4 copy 0 5 4\nend");
    // a plain copy 1:1 (no attributes, identity attributes), each pair of formats, both modes
    for i in 0..32 {
        let (fs, fd) = (*r.pick(&fmts), *r.pick(&fmts));
        let mode = *r.pick(&["over", "copy"]);
        let a = if i % 2 == 0 { "-" } else { "0" };
        writeln!(s, "case copy-{i}\ngpbitmap 0 8 8 {fd}\ngpwrite 0 {fd} seed {}\ngpbitmap 1 8 8 {fs}\ngpwrite 1 {fs} seed {}\ngpattr 0 matrix 0 {id}\ngpdraw 0 1 {a} 0 0 8 8 0 0 8 8 {mode} 0 5 4\ngpread 0 {fd}\nend", r.next(), r.next());
    }
    // color matrices: scales, translations, full; copy and over, default and gamma corrected
    for i in 0..160 {
        let (fs, fd) = (*r.pick(&fmts), *r.pick(&fmts));
        let mode = *r.pick(&["over", "copy"]);
        let m = rmatrix(&mut r).join(" ");
        let q = *r.pick(&[0, 0, 2]);
        writeln!(s, "case matrix-{i}\ngpbitmap 0 8 8 {fd}\ngpwrite 0 {fd} seed {}\ngpbitmap 1 8 8 {fs}\ngpwrite 1 {fs} seed {}\ngpattr 0 matrix 0 {m}\ngpdraw 0 1 0 0 0 8 8 0 0 8 8 {mode} {q} 5 4\ngpread 0 {fd}\nend", r.next(), r.next());
    }
    // gray pixels (r = g = b) under SkipGrays and AltGray (the gray matrix)
    for i in 0..32 {
        let m = rmatrix(&mut r).join(" ");
        let g = rmatrix(&mut r).join(" ");
        let flags = [0, 1, 2][i % 3];
        let mut set = String::new();
        for k in 0..8 {
            let v = r.next() & 255;
            writeln!(set, "gpset 1 {k} {} {:#010x}", k % 4, (r.next() & 0xff00_0000) | (v * 0x1_0101));
        }
        writeln!(s, "case gray-{i}\ngpbitmap 0 8 4 32bppARGB\ngpwrite 0 32bppARGB seed {}\ngpbitmap 1 8 4 32bppARGB\ngpwrite 1 32bppARGB seed {}\n{set}gpattr 0 matrix {flags} {m} gray {g}\ngpdraw 0 1 0 0 0 8 4 0 0 8 4 copy 0 5 4\ngpread 0 32bppARGB\nend", r.next(), r.next());
    }
    // AltGray under a color matrix that only translates (GP.DRAW.ALTGRAY.TRANSLATE): every pixel
    // gray, the gray matrix the identity, a scale, a translation or full
    for i in 0..32 {
        let t: Vec<String> = (0..4).map(|_| rentry(&mut r)).collect();
        let m = format!("1 0 0 0 0 0 1 0 0 0 0 0 1 0 0 0 0 0 1 0 {} 1", t.join(" "));
        let g = match i % 4 {
            0 => id.to_string(),
            _ => rmatrix(&mut r).join(" "),
        };
        let mut set = String::new();
        for y in 0..4 {
            for x in 0..8 {
                let v = r.next() & 255;
                writeln!(set, "gpset 1 {x} {y} {:#010x}", (r.next() & 0xff00_0000) | (v * 0x1_0101));
            }
        }
        writeln!(s, "case altgray-{i}\ngpbitmap 0 8 4 32bppARGB\ngpbitmap 1 8 4 32bppARGB\n{set}gpattr 0 matrix 2 {m} gray {g}\ngpdraw 0 1 0 0 0 8 4 0 0 8 4 copy 0 5 4\ngpread 0 32bppARGB\nend");
    }
    // NearestNeighbor scaled draws of an opaque bitmap (so every pixel is told apart exactly):
    // which source pixel each destination pixel takes, under every pixel offset mode, up and down,
    // off the corners
    for i in 0..64 {
        let (sw, sh) = (r.range(2, 12), r.range(2, 12));
        let (w, h) = (r.range(1, 24), r.range(1, 24));
        let (sx, sy) = (r.range(0, 12 - sw), r.range(0, 12 - sh));
        let (dx, dy) = (r.range(0, 24 - w), r.range(0, 24 - h));
        let po = [0, 1, 2, 3, 4][i % 5];
        writeln!(s, "case nearest-{i}\ngpbitmap 0 24 24 32bppARGB\ngpbitmap 1 12 12 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {dx} {dy} {w} {h} {sx} {sy} {sw} {sh} copy 0 5 {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    // scaled draws, other interpolation and pixel offset modes (GP.DRAW.SCALED): asked, not fitted
    for i in 0..24 {
        let (w, h) = (r.range(1, 16), r.range(1, 16));
        let im = *r.pick(&[0, 1, 2, 3, 4, 5, 6, 7]);
        let po = *r.pick(&[0, 2, 3, 4]);
        writeln!(s, "case scale-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbitmap 1 8 8 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpdraw 0 1 - 0 0 {w} {h} 0 0 8 8 copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    // the filtering modes (Default, LowQuality, HighQuality, Bilinear, Bicubic, HighQualityBilinear,
    // HighQualityBicubic): one axis scaled at a time over an opaque bitmap (the kernel's weights
    // read off each column or row), then both, off the corners, then a translucent source
    const FILTERS: [i64; 7] = [0, 1, 2, 3, 4, 6, 7];
    const OFFSETS: [i64; 4] = [0, 4, 2, 3];
    for i in 0..112 {
        let (im, po) = (FILTERS[i % 7], OFFSETS[i / 7 % 4]);
        let w = r.range(1, 32);
        writeln!(s, "case filterx-{i}\ngpbitmap 0 32 8 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - 0 0 {w} 8 0 0 8 8 copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    for i in 0..112 {
        let (im, po) = (FILTERS[i % 7], OFFSETS[i / 7 % 4]);
        let h = r.range(1, 32);
        writeln!(s, "case filtery-{i}\ngpbitmap 0 8 32 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - 0 0 8 {h} 0 0 8 8 copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    for i in 0..112 {
        let (im, po) = (FILTERS[i % 7], OFFSETS[i / 7 % 4]);
        let (sw, sh) = (r.range(1, 8), r.range(1, 8));
        let (sx, sy) = (r.range(0, 8 - sw), r.range(0, 8 - sh));
        let (w, h) = (r.range(1, 24), r.range(1, 24));
        let (dx, dy) = (r.range(0, 32 - w), r.range(0, 32 - h));
        writeln!(s, "case filter-{i}\ngpbitmap 0 32 32 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {dx} {dy} {w} {h} {sx} {sy} {sw} {sh} copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    for i in 0..56 {
        let (im, po) = (FILTERS[i % 7], OFFSETS[i / 7 % 4]);
        let (w, h) = (r.range(1, 24), r.range(1, 24));
        writeln!(s, "case filteralpha-{i}\ngpbitmap 0 24 24 32bppARGB\ngpbitmap 1 8 8 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpdraw 0 1 - 0 0 {w} {h} 0 0 8 8 copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    // one source column across 64 destination columns, the step exactly 1/64: column k reads
    // the bicubic weights' table at entry k, against eight rows of three channels (the table of
    // each filtering mode pinned entry by entry); then the same down
    for i in 0..24 {
        let im = [4, 6, 7][i % 3];
        writeln!(s, "case cubicq-{i}\ngpbitmap 0 64 8 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - 0 0 64 8 3 0 1 8 copy 0 {im} 3\ngpread 0 32bppARGB\nend", r.next());
    }
    for i in 0..12 {
        let im = [4, 6, 7][i % 3];
        writeln!(s, "case cubicqy-{i}\ngpbitmap 0 8 64 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - 0 0 8 64 0 3 8 1 copy 0 {im} 3\ngpread 0 32bppARGB\nend", r.next());
    }
    // the high quality filters' weights read one by one: lone source columns (rows) of
    // ff/aa/55 on black, far enough apart that a destination pixel sees one of them, inside a
    // source rectangle 4 pixels in from each edge of the bitmap; enlargements then reductions,
    // corner and half pixel offsets
    for (axis, (dn, gap)) in ["x", "y"].iter().flat_map(|a| [(25, 6), (31, 6), (40, 6), (47, 6), (53, 6), (13, 10), (11, 10), (7, 10), (5, 10)].map(|d| (*a, d))) {
        for im in [6, 7] {
            for po in [0, 2] {
                for k in 0..gap {
                    let name = format!("hq{axis}-{im}-{po}-{dn}-{k}");
                    let mut c = format!("case {name}\n");
                    let line = |at: i64| if axis == "x" { format!("gpfill 1 {at} 0 1 4 0xffffaa55 copy 0\n") } else { format!("gpfill 1 0 {at} 4 1 0xffffaa55 copy 0\n") };
                    let (bw, bh, dw, dh) = if axis == "x" { (26, 4, 64, 4) } else { (4, 26, 4, 64) };
                    c += &format!("gpbitmap 0 {dw} {dh} 32bppARGB\ngpbitmap 1 {bw} {bh} 32bppRGB\ngpclear 1 0xff000000\n");
                    for at in (4 + k..22).step_by(gap as usize) {
                        c += &line(at);
                    }
                    let rect = if axis == "x" { format!("0 0 {dn} 4 4 0 18 4") } else { format!("0 0 4 {dn} 0 4 4 18") };
                    c += &format!("gpdraw 0 1 - {rect} copy 0 {im} {po}\ngpread 0 32bppARGB\nend");
                    writeln!(s, "{c}");
                }
            }
        }
    }
    // the same for HighQualityBicubic on a gray ground (its negative lobes show) eight pixels
    // across the other axis (its middle rows reach no edge)
    for (axis, (dn, gap)) in ["x", "y"].iter().flat_map(|a| [(25, 6), (31, 6), (40, 6), (47, 6), (53, 6), (13, 10), (11, 10), (7, 10), (5, 10)].map(|d| (*a, d))) {
        for po in [0, 2] {
            for k in 0..gap {
                let mut c = format!("case hqc{axis}-{po}-{dn}-{k}\n");
                let (bw, bh, dw, dh) = if axis == "x" { (26, 8, 64, 8) } else { (8, 26, 8, 64) };
                c += &format!("gpbitmap 0 {dw} {dh} 32bppARGB\ngpbitmap 1 {bw} {bh} 32bppRGB\ngpclear 1 0xff808080\n");
                for at in (4 + k..22).step_by(gap as usize) {
                    c += &if axis == "x" { format!("gpfill 1 {at} 0 1 8 0xffffaa55 copy 0\n") } else { format!("gpfill 1 0 {at} 8 1 0xffffaa55 copy 0\n") };
                }
                let rect = if axis == "x" { format!("0 0 {dn} 8 4 0 18 8") } else { format!("0 0 8 {dn} 0 4 8 18") };
                c += &format!("gpdraw 0 1 - {rect} copy 0 7 {po}\ngpread 0 32bppARGB\nend");
                writeln!(s, "{c}");
            }
        }
    }
    drawprobe(&mut s);
    s
}

/// probes of DrawImage past the first cases (a generator of its own, so the cases before keep
/// their numbers): a bitmap onto itself, flipped rectangles, HQ draws whose rows the bitmap
/// cuts, 2x HQ bicubic reductions of 16 columns and more, interpolation and pixel offset
/// values past the enums, scales of exactly -1
fn drawprobe(s: &mut String) {
    let mut r = XorShift32::new(0xd7a3_0b0b);
    for im in [0, 5, 6, 7] {
        writeln!(s, "case drawself-{im}\ngpbitmap 0 12 12 32bppARGB\ngpwrite 0 32bppARGB seed {}\ngpdraw 0 0 - 2 3 8 6 0 0 6 6 copy 0 {im} 0\ngpread 0 32bppARGB\nend", r.next());
    }
    for i in 0..16 {
        let sign = |r: &mut XorShift32| if r.range(0, 2) == 0 { 1 } else { -1 };
        let (dw, dh, sw, sh) = (sign(&mut r) * r.range(3, 12), sign(&mut r) * r.range(3, 12), sign(&mut r) * r.range(2, 8), sign(&mut r) * r.range(2, 8));
        let (dx, dy) = (if dw < 0 { 14 } else { 2 }, if dh < 0 { 14 } else { 2 });
        let (sx, sy) = (if sw < 0 { 8 } else { 0 }, if sh < 0 { 8 } else { 0 });
        writeln!(s, "case drawflip-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {dx} {dy} {dw} {dh} {sx} {sy} {sw} {sh} copy 0 {} {}\ngpread 0 32bppARGB\nend", r.next(), *r.pick(&[1, 2, 3, 4, 5, 6, 7]), r.range(0, 5));
    }
    for i in 0..12 {
        let (dy, dh) = (r.range(-10, 6), r.range(10, 24));
        writeln!(s, "case drawcut-{i}\ngpbitmap 0 16 12 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {} {dy} {} {dh} 0 0 8 8 copy 0 {} {}\ngpread 0 32bppARGB\nend", r.next(), r.range(-4, 4), r.range(8, 20), *r.pick(&[6, 7]), r.range(0, 5));
    }
    for i in 0..8 {
        let sw = r.range(32, 64);
        writeln!(s, "case drawhalf-{i}\ngpbitmap 0 40 8 32bppARGB\ngpbitmap 1 {sw} 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {} 0 {} 8 0 0 {sw} 8 copy 0 7 {}\ngpread 0 32bppARGB\nend", r.next(), r.range(0, 3), sw / 2 + r.range(-1, 2), r.range(0, 5));
    }
    for (im, po) in [(-1, 0), (8, 0), (9, 0), (100, 0), (5, -1), (5, 5)] {
        writeln!(s, "case drawmode-{im}-{po}\ngpbitmap 0 8 8 32bppARGB\ngpbitmap 1 4 4 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - 0 0 8 8 0 0 4 4 copy 0 {im} {po}\ngpread 0 32bppARGB\nend", r.next());
    }
    // (a scale of exactly -1 on an axis: AnalyzeRotateFlip's branch)
    for i in 0..8 {
        let (fx, fy) = [(true, false), (false, true), (true, true)][i % 3];
        let (dw, dh) = (if fx { -8 } else { 8 }, if fy { -6 } else { 6 });
        writeln!(s, "case drawrot-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {} {} {dw} {dh} 0 1 8 6 copy 0 {} {}\ngpread 0 32bppARGB\nend", r.next(), if fx { 12 } else { 3 }, if fy { 10 } else { 2 }, *r.pick(&[1, 4, 5, 6, 7]), r.range(0, 5));
    }
    // (wide mirrored HQ draws: on x86 the MMX columns of a negative step)
    for i in 0..12 {
        let sw = r.range(12, 40);
        let dw = -(sw * r.range(5, 30) / 10).max(20);
        let sx = r.range(0, 48 - sw);
        writeln!(s, "case drawmirror-{i}\ngpbitmap 0 64 6 32bppARGB\ngpbitmap 1 48 6 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {} 0 {dw} 6 {sx} 0 {sw} 6 copy 0 {} {}\ngpread 0 32bppARGB\nend", r.next(), 63 + r.range(-2, 1), *r.pick(&[2, 7, 7, 6]), r.range(0, 5));
    }
    // (the visible clip of the source rows through the plain filters)
    for i in 0..12 {
        let (dy, dh) = (r.range(-10, 6), r.range(10, 24));
        writeln!(s, "case drawcutlo-{i}\ngpbitmap 0 16 12 32bppARGB\ngpbitmap 1 8 8 32bppRGB\ngpwrite 1 32bppRGB seed {}\ngpdraw 0 1 - {} {dy} {} {dh} 0 0 8 8 copy 0 {} {}\ngpread 0 32bppARGB\nend", r.next(), r.range(-4, 4), r.range(8, 20), *r.pick(&[1, 3, 4, 5]), r.range(0, 5));
    }
}

/// the gdiplusbrush suite: LinearGradientBrush and TextureBrush filling rectangles
pub fn gdiplusbrush() -> String {
    let mut s = String::from("# kikizake suite gdiplusbrush: GDI+ linear gradient and texture brushes filling rectangles (SPEC.md). Made by `kikizake gen gdiplusbrush`.\n");
    let mut r = XorShift32::new(0x9a17_0b01);
    let names: Vec<&str> = Pf::ALL.iter().map(|f| f.name()).collect();
    // what CreateLineBrushI takes: every WrapMode and past it, equal points
    for wrap in -1..=5 {
        for (k, (x2, y2)) in [(8, 0), (0, 0), (3, 5)].iter().enumerate() {
            writeln!(s, "case linstatus-{wrap}-{k}\ngpbitmap 0 8 2 32bppARGB\ngpbrush 0 linear 0 0 {x2} {y2} 0xff000000 0xffffffff {wrap}\ngppaint 0 0 0 0 8 2 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend");
        }
    }
    // black to white across: each length 1..40, from columns off 0, every tiling
    for len in 1..=40 {
        let x1 = len % 5 - 2;
        let wrap = len % 4;
        writeln!(s, "case linx-{len}\ngpbitmap 0 48 1 32bppARGB\ngpbrush 0 linear {x1} 0 {} 0 0xff000000 0xffffffff {wrap}\ngppaint 0 0 0 0 48 1 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend", x1 + len);
    }
    // the same per channel: one channel each way, and the pixel offset modes
    for i in 0..20 {
        let len = r.range(2, 40);
        let po = r.range(0, 4);
        writeln!(s, "case linxc-{i}\ngpbitmap 0 48 1 32bppARGB\ngpbrush 0 linear 0 0 {len} 0 0xff00ff00 0xffff00ff {}\ngppaint 0 0 0 0 48 1 copy 0 0 {po} 0 0\ngpread 0 32bppARGB\nend", r.range(0, 3));
    }
    // down, and back to front
    for len in 1..=20 {
        writeln!(s, "case liny-{len}\ngpbitmap 0 1 32 32bppARGB\ngpbrush 0 linear 0 {len} 0 0 0xff000000 0xffffffff {}\ngppaint 0 0 0 0 1 32 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend", len % 4);
    }
    // any two points, any colors, read straight and premultiplied
    for i in 0..60 {
        let (x1, y1, x2, y2) = (r.range(-8, 24), r.range(-8, 24), r.range(-8, 24), r.range(-8, 24));
        let f = *r.pick(&["32bppARGB", "32bppPARGB"]);
        writeln!(s, "case lindiag-{i}\ngpbitmap 0 16 16 {f}\ngpbrush 0 linear {x1} {y1} {x2} {y2} {:#010x} {:#010x} {}\ngppaint 0 0 0 0 16 16 copy 0 0 0 0 0\ngpread 0 {f}\nend", rargb(&mut r), rargb(&mut r), r.range(0, 3));
    }
    // over a destination, every quality, every format
    for i in 0..40 {
        let f = *r.pick(&names);
        let (x2, y2) = (r.range(-16, 16), r.range(-4, 4));
        writeln!(s, "case linblend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 linear 0 0 {x2} {y2} {:#010x} {:#010x} {}\ngppaint 0 0 0 0 16 4 over {} 0 0 0 0\ngpread 0 {f}\nend", r.next(), rargb(&mut r), rargb(&mut r), r.range(0, 3), r.range(0, 4));
    }
    // rectangles in float coordinates under the smoothing and pixel offset modes
    for i in 0..40 {
        let (x, y, w, h) = (rcoord(&mut r), rcoord(&mut r), rsize(&mut r), rsize(&mut r));
        writeln!(s, "case linrect-{i}\ngpbitmap 0 12 12 32bppARGB\ngpbrush 0 linear 0 0 {} {} 0xff0000ff 0xffffff00 0\ngppaint 0 0 {x} {y} {w} {h} copy 0 {} {} {} {}\ngpread 0 32bppARGB\nend", r.range(1, 14), r.range(-6, 6), r.range(0, 6), r.range(0, 5), r.range(-4, 4), r.range(-4, 4));
    }
    // the brush's rectangle and transform beside what it paints (black to white, so that only
    // the place along the gradient matters), any two points, both pixel offsets
    for i in 0..120 {
        let (x1, y1) = (r.range(-8, 24), r.range(-8, 24));
        let (x2, y2) = if i < 16 { (x1 + [1, 0, -1, 0][i % 4] * (i as i64 + 1), y1 + [0, 1, 0, -1][i % 4] * (i as i64 + 1)) } else { (r.range(-8, 24), r.range(-8, 24)) };
        let (x2, y2) = if (x2, y2) == (x1, y1) { (x1 + 1, y1) } else { (x2, y2) };
        writeln!(s, "case lininfo-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 linear {x1} {y1} {x2} {y2} 0xff000000 0xffffffff 0\ngpbrushinfo 0\ngppaint 0 0 0 0 16 16 copy 0 0 {} 0 0\ngpread 0 32bppARGB\nend", if i % 3 == 2 { 2 } else { 0 });
    }
    // what CreateTexture takes: every WrapMode and past it, of each format
    for wrap in -1..=5 {
        writeln!(s, "case texstatus-{wrap}\ngpbitmap 1 3 2 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpbitmap 0 8 4 32bppARGB\ngpbrush 0 texture 1 {wrap}\ngppaint 0 0 0 0 8 4 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend", r.next());
    }
    // a texture of each format, tiled over a 32bppARGB bitmap
    for f in &names {
        writeln!(s, "case texfmt-{f}\ngpbitmap 1 5 3 {f}\ngpwrite 1 {f} seed {}\ngpbitmap 0 12 7 32bppARGB\ngpbrush 0 texture 1 0\ngppaint 0 0 0 0 12 7 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend", r.next());
    }
    // every tiling from rectangles and rendering origins off the corner
    for i in 0..40 {
        let (tw, th) = (r.range(1, 6), r.range(1, 6));
        let (x, y) = (r.range(0, 6), r.range(0, 6));
        writeln!(s, "case textile-{i}\ngpbitmap 1 {tw} {th} 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 texture 1 {}\ngppaint 0 0 {x} {y} {} {} copy 0 0 {} {} {}\ngpread 0 32bppARGB\nend", r.next(), r.range(0, 4), 16 - x, 16 - y, r.range(0, 4), r.range(-9, 9), r.range(-9, 9));
    }
    // a texture brush's transform
    writeln!(s, "case texinfo\ngpbitmap 1 3 2 32bppARGB\ngpbrush 0 texture 1 0\ngpbrushinfo 0\ngpbrush 1 solid 0xff000000\ngpbrushinfo 1\ngpbrush 1 hatch 3 0xff000000 0xffffffff\ngpbrushinfo 1\nend");
    // the texture is the bitmap as it was: the bitmap written after the brush is made
    for i in 0..4 {
        writeln!(s, "case texsnap-{i}\ngpbitmap 1 4 4 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpbitmap 0 8 8 32bppARGB\ngpbrush 0 texture 1 {i}\ngpwrite 1 32bppARGB seed {}\ngppaint 0 0 0 0 8 8 copy 0 0 0 0 0\ngpread 0 32bppARGB\nend", r.next(), r.next());
    }
    // over a destination of any format, every quality
    for i in 0..40 {
        let f = *r.pick(&names);
        writeln!(s, "case texblend-{i}\ngpbitmap 1 3 3 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpbitmap 0 9 4 {f}\ngpwrite 0 {f} seed {}\ngpbrush 0 texture 1 {}\ngppaint 0 0 0 0 9 4 over {} 0 0 0 0\ngpread 0 {f}\nend", r.next(), r.next(), r.range(0, 4), r.range(0, 4));
    }
    // rectangles in float coordinates under the smoothing and pixel offset modes
    for i in 0..40 {
        let (x, y, w, h) = (rcoord(&mut r), rcoord(&mut r), rsize(&mut r), rsize(&mut r));
        writeln!(s, "case texrect-{i}\ngpbitmap 1 3 2 32bppARGB\ngpwrite 1 32bppARGB seed {}\ngpbitmap 0 12 12 32bppARGB\ngpbrush 0 texture 1 {}\ngppaint 0 0 {x} {y} {w} {h} copy 0 {} {} {} {}\ngpread 0 32bppARGB\nend", r.next(), r.range(0, 4), r.range(0, 6), r.range(0, 5), r.range(-4, 4), r.range(-4, 4));
    }
    s
}

/// a coordinate in quarters over a 16 pixel bitmap, a little past it
fn rq(r: &mut XorShift32) -> String {
    format!("{}", r.range(-8, 72) as f32 / 4.0)
}

/// the gdiplusline suite: pens drawing lines and shapes, brushes filling ellipses, pies,
/// polygons and closed curves
pub fn gdiplusline() -> String {
    let mut s = String::from("# kikizake suite gdiplusline: GDI+ pens (widths, dashes, caps, joins, alignment) drawing lines and shapes, and brushes filling ellipses, pies, polygons, closed curves, under the smoothing and pixel offset modes and world transforms (SPEC.md). Made by `kikizake gen gdiplusline`.\n");
    let mut r = XorShift32::new(0x9a17_0c01);
    let head = "gpbitmap 0 16 16 32bppARGB";
    let pts = |r: &mut XorShift32, n: usize| -> String { (0..2 * n).map(|_| rq(r)).collect::<Vec<_>>().join(" ") };
    // what CreatePen1 and the setters take
    for (i, w) in ["1", "0", "-1", "2.5", "0.25"].iter().enumerate() {
        writeln!(s, "case penwidth-{i}\n{head}\ngppen 0 0xffff0000 {w}\ngpshape 0 pen 0 - copy 0 0 0 line 2 3 13 9\ngpread 0 32bppARGB\nend");
    }
    for v in -1..=6 {
        writeln!(s, "case pendash-{v}\n{head}\ngppen 0 0xffff0000 1 dash {v}\ngpshape 0 pen 0 - copy 0 0 0 line 0 4 16 4\ngpread 0 32bppARGB\nend");
    }
    for (i, c) in [0, 1, 2, 3, 0x10, 0x11, 0x12, 0x13, 0x14, 0xf0, 0xff, 4, -1].iter().enumerate() {
        writeln!(s, "case pencap-{i}\n{head}\ngppen 0 0xffff0000 4 cap {c} {c} 0\ngpshape 0 pen 0 - copy 0 0 0 line 4 8 12 8\ngpread 0 32bppARGB\nend");
    }
    for d in [-1, 0, 1, 2, 3, 4] {
        writeln!(s, "case pendashcap-{d}\n{head}\ngppen 0 0xffff0000 2 dash 1 cap 0 0 {d}\ngpshape 0 pen 0 - copy 0 0 0 line 0 8 16 8\ngpread 0 32bppARGB\nend");
    }
    for j in -1..=4 {
        writeln!(s, "case penjoin-{j}\n{head}\ngppen 0 0xffff0000 3 join {j}\ngpshape 0 pen 0 - copy 0 0 0 lines 3 2 12 8 3 14 12\ngpread 0 32bppARGB\nend");
    }
    for a in -1..=2 {
        writeln!(s, "case penalign-{a}\n{head}\ngppen 0 0xffff0000 3 align {a}\ngpshape 0 pen 0 - copy 0 0 0 rect 3 3 9 9\ngpread 0 32bppARGB\nend");
    }
    for (i, m) in ["0", "-1", "1", "4", "100"].iter().enumerate() {
        writeln!(s, "case penmiter-{i}\n{head}\ngppen 0 0xffff0000 3 join 0 miter {m}\ngpshape 0 pen 0 - copy 0 0 0 lines 3 1 14 8 3 1 14\ngpread 0 32bppARGB\nend");
    }
    // thin lines between points on a quarter grid: every smoothing and pixel offset mode
    for i in 0..120 {
        let (sm, po) = (r.range(0, 5), r.range(0, 4));
        writeln!(s, "case line-{i}\n{head}\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 {sm} {po} line {} {} {} {}\ngpread 0 32bppARGB\nend", rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r));
    }
    // wide lines, each cap
    for i in 0..60 {
        let w = r.range(2, 24) as f32 / 4.0;
        let c = *r.pick(&[0, 1, 2, 3, 0x10, 0x11, 0x12, 0x13, 0x14]);
        writeln!(s, "case linewide-{i}\n{head}\ngppen 0 0xff00ff00 {w} cap {c} {c} 0\ngpshape 0 pen 0 - copy 0 {} {} line {} {} {} {}\ngpread 0 32bppARGB\nend", r.range(0, 5), r.range(0, 4), rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r));
    }
    // dashed lines
    for i in 0..40 {
        let w = r.range(1, 3);
        writeln!(s, "case linedash-{i}\n{head}\ngppen 0 0xff0000ff {w} dash {} cap 0 0 {}\ngpshape 0 pen 0 - copy 0 {} 0 line {} {} {} {}\ngpread 0 32bppARGB\nend", r.range(1, 4), *r.pick(&[0, 2, 3]), r.range(0, 5), rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r));
    }
    // the other outlines
    let outline: &[(&str, usize)] = &[("rect", 4), ("ellipse", 4), ("bezier", 8), ("arc", 6), ("pie", 6)];
    for i in 0..100 {
        let (name, n) = outline[i % outline.len()];
        let args: Vec<String> = (0..n).map(|k| if n == 6 && k >= 4 { format!("{}", r.range(-400, 400)) } else { rq(&mut r) }).collect();
        writeln!(s, "case outline-{name}-{i}\n{head}\ngppen 0 0xffff0000 {}\ngpshape 0 pen 0 - copy 0 {} {} {name} {}\ngpread 0 32bppARGB\nend", r.range(1, 4), r.range(0, 5), r.range(0, 4), args.join(" "));
    }
    for i in 0..60 {
        let name = *r.pick(&["lines", "polygon", "curve", "closedcurve"]);
        let n = r.range(2, 6) as usize;
        let tension = if name.ends_with("curve") { format!("{} ", r.range(0, 8) as f32 / 4.0) } else { String::new() };
        writeln!(s, "case outline-{name}-{i}\n{head}\ngppen 0 0xffff0000 {} join {}\ngpshape 0 pen 0 - copy 0 {} {} {name} {tension}{n} {}\ngpread 0 32bppARGB\nend", r.range(1, 4), r.range(0, 3), r.range(0, 5), r.range(0, 4), pts(&mut r, n));
    }
    // filled shapes, both fill modes
    for i in 0..100 {
        let name = *r.pick(&["ellipse", "pie", "polygon", "closedcurve"]);
        let args = match name {
            "ellipse" => Vec::<_>::from_fn(4, |_| rq(&mut r)).join(" "),
            "pie" => format!("{} {} {} {} {} {}", rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r), r.range(-400, 400), r.range(-400, 400)),
            "polygon" => {
                let n = r.range(3, 7) as usize;
                format!("{} {n} {}", r.range(0, 1), pts(&mut r, n))
            }
            _ => {
                let n = r.range(3, 6) as usize;
                format!("{} {} {n} {}", r.range(0, 8) as f32 / 4.0, r.range(0, 1), pts(&mut r, n))
            }
        };
        writeln!(s, "case fill-{name}-{i}\n{head}\ngpbrush 0 solid 0xff00ff00\ngpshape 0 brush 0 - copy 0 {} {} {name} {args}\ngpread 0 32bppARGB\nend", r.range(0, 5), r.range(0, 4));
    }
    // through a world transform (scales, rotations, shears: the matrices' floats as bits)
    for i in 0..40 {
        let ang = r.range(0, 360) as f32 * std::f32::consts::PI / 180.0;
        let k = r.range(2, 8) as f32 / 4.0;
        let m = [ang.cos() * k, ang.sin() * k, -ang.sin(), ang.cos(), 8.0, 8.0].map(|v: f32| format!("{:#010x}", v.to_bits())).join(" ");
        let shape = if i % 2 == 0 { "pen 0 - copy 0 X 0 line -6 -3 6 3".to_string() } else { "brush 0 - copy 0 X 0 ellipse -5 -3 10 6".to_string() };
        let shape = shape.replace(" - ", " 0 ").replace('X', &r.range(0, 5).to_string());
        writeln!(s, "case world-{i}\n{head}\ngpmatrix 0 {m}\ngppen 0 0xffff0000 {}\ngpbrush 0 solid 0xff0000ff\ngpshape 0 {shape}\ngpread 0 32bppARGB\nend", r.range(1, 3));
    }
    // translucent pens and brushes over a destination, every quality
    for i in 0..40 {
        let shape = if i % 2 == 0 { format!("pen 0 - over {} {} 0 line {} {} {} {}", r.range(0, 4), r.range(0, 5), rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r)) } else { format!("brush 0 - over {} {} 0 ellipse {} {} {} {}", r.range(0, 4), r.range(0, 5), rq(&mut r), rq(&mut r), rq(&mut r), rq(&mut r)) };
        writeln!(s, "case shapeblend-{i}\n{head}\ngpwrite 0 32bppARGB seed {}\ngppen 0 {:#010x} {}\ngpbrush 0 solid {:#010x}\ngpshape 0 {shape}\ngpread 0 32bppARGB\nend", r.next(), rargb(&mut r), r.range(1, 4), rargb(&mut r));
    }
    lineprobe(&mut s);
    fillprobe(&mut s);
    penprobe(&mut s);
    s
}

/// a pen setter of the pen objects: a compound array, an adjustable arrow cap or a custom cap
/// (fill polygon or stroke lines), drawn from r
fn pen_object(r: &mut XorShift32, i: usize) -> String {
    match i % 4 {
        0 | 1 => {
            let n = *r.pick(&[2usize, 4, 4, 6]);
            let mut v: Vec<i64> = (0..n).map(|_| r.range(0, 9)).collect();
            v.sort();
            format!("compound {n} {}", v.iter().map(|x| format!("{}", *x as f32 / 8.0)).collect::<Vec<_>>().join(" "))
        }
        2 => format!("arrowcap {} {} {} {} {}", r.range(0, 2), r.range(1, 9) as f32 / 2.0, r.range(1, 9) as f32 / 2.0, r.range(0, 2), *r.pick(&["0", "0", "0.5", "1", "-1"])),
        _ => {
            let stroke = r.range(0, 2);
            let pts = if stroke == 1 { "3 -1 -1 0 0 1 -1" } else { *r.pick(&["3 -1 -1 0 0 1 -1", "4 -1 -2 0 0 1 -2 0 -1", "4 -0.5 -0.5 0.5 -0.5 0.5 0.5 -0.5 0.5"]) };
            format!("customcap {} {stroke} {} {} {} {} {} {} {pts}", r.range(0, 2), r.range(0, 4), *r.pick(&["0", "0", "1", "0.5"]), *r.pick(&["1", "1", "0.5", "2"]), r.range(0, 3), r.range(0, 3), r.range(0, 3))
        }
    }
}

/// probes of the pen past the first cases (a generator of its own, so the cases before keep
/// their numbers): Draw* calls of too few points, the round join on random polylines, anchor
/// caps on random lines of any width, Inset on rectangles off the pixel grid
fn penprobe(s: &mut String) {
    let mut r = XorShift32::new(0x7e4_d00d);
    let head = "gpbitmap 0 16 16 32bppARGB";
    let pts = |r: &mut XorShift32, n: usize| -> String { (0..2 * n).map(|_| rq(r)).collect::<Vec<_>>().join(" ") };
    for (name, tension) in [("lines", ""), ("polygon", ""), ("curve", "0.5 "), ("closedcurve", "0.5 ")] {
        for n in 0..=2 {
            for w in [1, 3] {
                writeln!(s, "case pencount-{name}-{n}-{w}\n{head}\ngppen 0 0xffff0000 {w}\ngpshape 0 pen 0 - copy 0 0 0 {name} {tension}{n} {}\ngpread 0 32bppARGB\nend", pts(&mut r, n));
            }
        }
    }
    writeln!(s, "case pencount-same\n{head}\ngppen 0 0xffff0000 3\ngpshape 0 pen 0 - copy 0 0 0 polygon 3 5 5 5 5 5 5\ngpread 0 32bppARGB\nend");
    for i in 0..30 {
        let n = r.range(3, 7) as usize;
        let name = *r.pick(&["lines", "polygon"]);
        writeln!(s, "case roundjoin-{i}\n{head}\ngppen 0 0xffff0000 {} join 2 cap {} {} 0\ngpshape 0 pen 0 - copy 0 {} {} {name} {n} {}\ngpread 0 32bppARGB\nend", r.range(3, 24) as f32 / 4.0, r.range(0, 4), r.range(0, 4), r.range(0, 5), r.range(0, 4), pts(&mut r, n));
    }
    for i in 0..30 {
        let caps = [0, 1, 2, 3, 0x11, 0x12, 0x13, 0x14];
        writeln!(s, "case anchor-{i}\n{head}\ngppen 0 0xff00ff00 {} cap {} {} 0 join {}\ngpshape 0 pen 0 - copy 0 {} {} lines 3 {}\ngpread 0 32bppARGB\nend", r.range(2, 20) as f32 / 4.0, *r.pick(&caps), *r.pick(&caps), r.range(0, 4), r.range(0, 5), r.range(0, 4), pts(&mut r, 3));
    }
    for i in 0..10 {
        writeln!(s, "case inset-{i}\n{head}\ngppen 0 0xffff0000 {} align 1\ngpshape 0 pen 0 - copy 0 {} 0 rect {} {} {} {}\ngpread 0 32bppARGB\nend", r.range(2, 16) as f32 / 4.0, r.range(0, 5), r.range(4, 24) as f32 / 4.0, r.range(4, 24) as f32 / 4.0, r.range(8, 40) as f32 / 4.0, r.range(8, 40) as f32 / 4.0);
    }
    // the pen objects: compound arrays, adjustable arrows, custom caps; and the setters' checks
    for i in 0..48 {
        let n = r.range(2, 5) as usize;
        let name = *r.pick(&["lines", "polygon", "lines"]);
        writeln!(s, "case penobject-{i}\n{head}\ngppen 0 0xffff0000 {} cap {} {} 0 join {} {}\ngpshape 0 pen 0 - copy 0 {} {} {name} {n} {}\ngpread 0 32bppARGB\nend", r.range(4, 36) as f32 / 4.0, r.range(0, 4), r.range(0, 4), r.range(0, 4), pen_object(&mut r, i), r.range(0, 5), r.range(0, 4), pts(&mut r, n));
    }
    for (i, set) in ["compound 1 0.5", "compound 3 0 0.5 1", "compound 2 0.5 0.25", "compound 2 -0.25 1", "compound 2 0 1.5", "compound 0", "compound 4 0 0.5 0.5 1", "compound 2 0 1 align 1", "compound 4 0 0.25 0.75 1 align 1", "arrowcap 1 0 2 1 0", "arrowcap 1 3 0 1 0", "arrowcap 1 3 2 1 3", "arrowcap 0 3 2 1 0", "arrowcap 1 -1 2 1 0", "customcap 1 0 0 0 1 0 0 0 2 0 0 1 1", "customcap 1 1 0 0 1 0 0 0 1 0 0"].iter().enumerate() {
        writeln!(s, "case penset-{i}\n{head}\ngppen 0 0xffff0000 3 {set}\ngpshape 0 pen 0 - copy 0 0 0 line 3 8 13 8\ngpread 0 32bppARGB\nend");
    }
    // (what the custom cap notes leave open: a scaling world matrix, a dash, a width scale of
    // 0, a round or triangle start cap whose last point lies on the first segment's line)
    let scale2 = "gpmatrix 0 0x40000000 0x00000000 0x00000000 0x40000000 0x00000000 0x00000000";
    for (i, (m, pen, shape)) in [
        (scale2, "2 customcap 1 0 0 0.5 1 0 0 0 3 -1 -1 0 0 1 -1", "line 1 4 7 4"),
        (scale2, "1.5 arrowcap 1 3 2 0 0", "line 1 2 7 5"),
        ("", "2 dash 1 arrowcap 1 3 2 1 0", "line 1 8 15 8"),
        ("", "2 dash 2 customcap 0 0 0 0 1 0 0 0 3 -1 -1 0 0 1 -1", "line 2 3 14 12"),
        ("", "2 customcap 1 1 0 0 0 0 0 0 3 -1 -1 0 0 1 -1", "line 2 8 14 8"),
        ("", "2 cap 2 0 0", "lines 3 8 8 12 8 4 8"),
        ("", "3 cap 3 1 0", "lines 3 6 6 10 10 2 2"),
    ]
    .iter()
    .enumerate()
    {
        let (m, k) = if m.is_empty() { (String::new(), "-") } else { (format!("{m}\n"), "0") };
        writeln!(s, "case penopen-{i}\n{head}\n{m}gppen 0 0xffff0000 {pen}\ngpshape 0 pen 0 {k} copy 0 0 0 {shape}\ngpread 0 32bppARGB\nend");
    }
    // (a world transform of no inverse: GdipSetWorldTransform refuses it)
    for (i, m) in ["0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000", "0x3f800000 0x40000000 0x40000000 0x40800000 0x40000000 0x40000000"].iter().enumerate() {
        writeln!(s, "case penworld-{i}\n{head}\ngpmatrix 0 {m}\ngppen 0 0xffff0000 2\ngpshape 0 pen 0 0 copy 0 0 0 line 2 3 12 9\ngpread 0 32bppARGB\nend");
        writeln!(s, "case brushworld-{i}\n{head}\ngpmatrix 0 {m}\ngpbrush 0 solid 0xff00ff00\ngpshape 0 brush 0 0 copy 0 0 0 ellipse 2 2 10 8\ngpread 0 32bppARGB\nend");
    }
    // (GdipSetPenDashArray and GdipSetPenDashOffset: the checks, a style after an array, offsets
    // past the pattern and negative, odd counts, thin aliased pens and wide ones, closed figures)
    for (i, set) in ["dasharray 0", "dasharray 2 2 0", "dasharray 2 2 -1", "dasharray 3 2 1 0.5", "dasharray 2 2 1 dash 1", "dash 1 dash 5", "dash 2 dasharray 2 4 1", "dasharray 2 4 1 dash 5", "dasharray 2 4 1 dash 0", "dasharray 2 4 1 dash 0 dash 5", "dashoff 1.5", "dashoff -2.25 dash 3"].iter().enumerate() {
        writeln!(s, "case pendashset-{i}\n{head}\ngppen 0 0xffff0000 1 {set}\ngpshape 0 pen 0 - copy 0 0 0 line 1 8 15 8\ngpread 0 32bppARGB\nend");
    }
    for i in 0..48 {
        let n = r.range(1, 6) as usize;
        let arr: Vec<String> = (0..n).map(|_| format!("{}", r.range(1, 17) as f32 / 4.0)).collect();
        let dash = if i % 4 == 0 { format!("dash {}", r.range(1, 5)) } else { format!("dasharray {n} {}", arr.join(" ")) };
        let off = r.range(-40, 41) as f32 / 4.0;
        let w = *r.pick(&[0.5, 1.0, 1.0, 2.0, 3.0, 4.5]);
        let shape = match r.range(0, 4) {
            0 => format!("lines 3 {}", pts(&mut r, 3)),
            1 => format!("rect {} {} {} {}", r.range(4, 16) as f32 / 4.0, r.range(4, 16) as f32 / 4.0, r.range(16, 40) as f32 / 4.0, r.range(16, 40) as f32 / 4.0),
            2 => format!("ellipse {} {} {} {}", r.range(4, 16) as f32 / 4.0, r.range(4, 16) as f32 / 4.0, r.range(16, 40) as f32 / 4.0, r.range(16, 40) as f32 / 4.0),
            _ => format!("polygon 4 {}", pts(&mut r, 4)),
        };
        writeln!(s, "case pendashoff-{i}\n{head}\ngppen 0 0xffff0000 {w} {dash} dashoff {off} cap {} {} {}\ngpshape 0 pen 0 - copy 0 {} 0 {shape}\ngpread 0 32bppARGB\nend", r.range(0, 4), r.range(0, 4), *r.pick(&[0, 2, 3]), *r.pick(&[0, 0, 2, 3]));
    }
}

/// probes of the aliased thin line (a generator of its own, so the cases before keep their
/// numbers): long lines whose every column is a decision, ends at each eighth of a pixel, short
/// lines with both ends fractional, ends past the bitmap, coordinates off the 1/16 grid
fn lineprobe(s: &mut String) {
    let mut r = XorShift32::new(0x11e_9b0b);
    let x16 = |r: &mut XorShift32, lo: i64, hi: i64| format!("{}", r.range(lo * 16, hi * 16) as f32 / 16.0);
    for i in 0..40 {
        let (sm, po) = if i < 32 { (3, 3) } else { (*r.pick(&[0, 1, 3]), *r.pick(&[0, 1, 3])) };
        let (a, b) = ((x16(&mut r, 0, 1), x16(&mut r, 0, 12)), (x16(&mut r, 62, 63), x16(&mut r, 0, 12)));
        let (a, b) = if i % 2 == 0 { (a, b) } else { (b, a) };
        writeln!(s, "case lineprobe-long-{i}\ngpbitmap 0 64 12 32bppARGB\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 {sm} {po} line {} {} {} {}\ngpread 0 32bppARGB\nend", a.0, a.1, b.0, b.1);
    }
    // (eight horizontal lines a case, one a row: start fraction i / 8, end fraction j / 8)
    for k in 0..16 {
        let (i, d) = (k % 8, if k < 8 { 1 } else { -1 });
        let mut ops = String::new();
        for j in 0..8 {
            let (x0, x1) = (2.0 + i as f32 / 8.0, 11.0 + j as f32 / 8.0);
            let (x0, x1) = if d > 0 { (x0, x1) } else { (x1, x0) };
            writeln!(ops, "gpshape 0 pen 0 - copy 0 3 3 line {x0} {j} {x1} {j}");
        }
        writeln!(s, "case lineprobe-end-{k}\ngpbitmap 0 16 8 32bppARGB\ngppen 0 0xffff0000 1\n{ops}gpread 0 32bppARGB\nend");
    }
    for i in 0..40 {
        let v: Vec<String> = (0..4).map(|_| x16(&mut r, 1, 15)).collect();
        writeln!(s, "case lineprobe-short-{i}\ngpbitmap 0 16 16 32bppARGB\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 3 3 line {}\ngpread 0 32bppARGB\nend", v.join(" "));
    }
    for i in 0..20 {
        let v: Vec<String> = (0..4).map(|_| x16(&mut r, -6, 22)).collect();
        writeln!(s, "case lineprobe-clip-{i}\ngpbitmap 0 16 16 32bppARGB\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 3 3 line {}\ngpread 0 32bppARGB\nend", v.join(" "));
    }
    for i in 0..20 {
        let v: Vec<String> = (0..4).map(|_| format!("{}", r.range(1000, 15000) as f32 / 1000.0)).collect();
        writeln!(s, "case lineprobe-grid-{i}\ngpbitmap 0 16 16 32bppARGB\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 3 3 line {}\ngpread 0 32bppARGB\nend", v.join(" "));
    }
    // (antialiased: the coverage of a long thin line, column by column)
    for i in 0..24 {
        let (sm, po) = (*r.pick(&[2, 4, 5]), *r.pick(&[3, 4]));
        let v = [x16(&mut r, 0, 1), x16(&mut r, 0, 12), x16(&mut r, 62, 63), x16(&mut r, 0, 12)];
        writeln!(s, "case lineprobe-aa-{i}\ngpbitmap 0 64 12 32bppARGB\ngppen 0 0xffff0000 1\ngpshape 0 pen 0 - copy 0 {sm} {po} line {}\ngpread 0 32bppARGB\nend", v.join(" "));
    }
}

/// probes of the fills: polygons in the Winding mode (self crossing, wound twice), and large
/// ellipses, pies and closed curves whose flattening shows in the pixels
fn fillprobe(s: &mut String) {
    let mut r = XorShift32::new(0xf111_9b0b);
    let q4 = |r: &mut XorShift32, lo: i64, hi: i64| format!("{}", r.range(lo * 4, hi * 4) as f32 / 4.0);
    for i in 0..30 {
        let (fm, sm, po) = (r.range(0, 2), *r.pick(&[3, 3, 4, 5]), *r.pick(&[3, 4]));
        let n = r.range(5, 10) as usize;
        let pts: Vec<String> = (0..2 * n).map(|_| q4(&mut r, -2, 18)).collect();
        writeln!(s, "case fillprobe-poly-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 solid 0xff00ff00\ngpshape 0 brush 0 - copy 0 {sm} {po} polygon {fm} {n} {}\ngpread 0 32bppARGB\nend", pts.join(" "));
    }
    for i in 0..30 {
        let (sm, po) = (*r.pick(&[3, 3, 4, 5]), *r.pick(&[3, 4]));
        let shape = match i % 3 {
            0 => format!("ellipse {} {} {} {}", q4(&mut r, 0, 8), q4(&mut r, 0, 8), q4(&mut r, 20, 56), q4(&mut r, 20, 56)),
            1 => format!("pie {} {} {} {} {} {}", q4(&mut r, 0, 8), q4(&mut r, 0, 8), q4(&mut r, 20, 56), q4(&mut r, 20, 56), r.range(-360, 360), r.range(-360, 360)),
            _ => {
                let n = r.range(3, 7) as usize;
                let pts: Vec<String> = (0..2 * n).map(|_| q4(&mut r, 2, 62)).collect();
                format!("closedcurve {} {} {n} {}", r.range(0, 8) as f32 / 4.0, r.range(0, 2), pts.join(" "))
            }
        };
        writeln!(s, "case fillprobe-curve-{i}\ngpbitmap 0 64 64 32bppARGB\ngpbrush 0 solid 0xff00ff00\ngpshape 0 brush 0 - copy 0 {sm} {po} {shape}\ngpread 0 32bppARGB\nend");
    }
}

/// the gdipluspath suite: GraphicsPath (the points each Add makes, FlattenPath's and WidenPath's
/// points, the bounds) and FillPath
pub fn gdipluspath() -> String {
    let mut s = String::from("# kikizake suite gdipluspath: GDI+ GraphicsPath: the points and types AddLine, AddBezier(s), AddRectangle, AddEllipse, AddArc, AddPie, AddPolygon, AddCurve, AddClosedCurve make, FlattenPath's and WidenPath's points, GetPathWorldBounds, FillPath (SPEC.md). Made by `kikizake gen gdipluspath`.\n");
    let mut r = XorShift32::new(0x9a7b_0c02);
    let q4 = |r: &mut XorShift32, lo: i64, hi: i64| format!("{}", r.range(lo * 4, hi * 4) as f32 / 4.0);
    let pts = |r: &mut XorShift32, n: usize| (0..2 * n).map(|_| q4(r, -2, 40)).collect::<Vec<_>>().join(" ");
    // statuses
    for (i, (fm, add)) in [(0, "line 1 2 3 4"), (1, "line 1 2 3 4"), (2, "line 1 2 3 4"), (-1, "line 1 2 3 4"), (0, "lines 1 1 1"), (0, "lines 0"), (0, "beziers 3 0 0 1 1 2 2"), (0, "beziers 5 0 0 1 1 2 2 3 3 4 4"), (0, "polygon 2 0 0 1 1"), (0, "curve 0.5 1 3 3"), (0, "curve -1 3 0 0 4 4 8 0"), (0, "closedcurve 0.5 2 0 0 4 4"), (0, "rect 0 0 -4 4"), (0, "ellipse 0 0 0 4"), (0, "arc 0 0 8 8 0 0"), (0, "pie 0 0 8 8 30 0"), (0, "close"), (0, "start")].iter().enumerate() {
        writeln!(s, "case pathstatus-{i}\ngppath 0 {fm}\ngpadd 0 {add}\ngppathinfo 0\nend");
    }
    for i in 0..40 {
        writeln!(s, "case pathellipse-{i}\ngppath 0 0\ngpadd 0 ellipse {} {} {} {}\ngppathinfo 0\nend", q4(&mut r, -4, 40), q4(&mut r, -4, 40), q4(&mut r, -8, 60), q4(&mut r, -8, 60));
    }
    for i in 0..60 {
        let what = if i % 2 == 0 { "arc" } else { "pie" };
        let ang = |r: &mut XorShift32| if r.chance(3) { format!("{}", 45 * r.range(-8, 8)) } else { format!("{}", r.range(-7200, 7200) as f32 / 10.0) };
        writeln!(s, "case path{what}-{i}\ngppath 0 0\ngpadd 0 {what} {} {} {} {} {} {}\ngppathinfo 0\nend", q4(&mut r, -4, 40), q4(&mut r, -4, 40), q4(&mut r, 1, 60), q4(&mut r, 1, 60), ang(&mut r), ang(&mut r));
    }
    for i in 0..60 {
        let add = match i % 6 {
            0 => format!("rect {} {} {} {}", q4(&mut r, -4, 40), q4(&mut r, -4, 40), q4(&mut r, 0, 40), q4(&mut r, 0, 40)),
            1 => format!("lines {n} {}", pts(&mut r, 5), n = 5),
            2 => format!("polygon {n} {}", pts(&mut r, 4), n = 4),
            3 => format!("beziers 7 {}", pts(&mut r, 7)),
            4 => format!("curve {} {n} {}", r.range(0, 8) as f32 / 4.0, pts(&mut r, 4), n = 4),
            _ => format!("closedcurve {} {n} {}", r.range(0, 8) as f32 / 4.0, pts(&mut r, 4), n = 4),
        };
        writeln!(s, "case pathadd-{i}\ngppath 0 0\ngpadd 0 {add}\ngpadd 0 line 1 2 3 4\ngpadd 0 close\ngpadd 0 line 5 6 7 8\ngppathinfo 0\nend");
    }
    // flattened: the flattener's points, at FlattenPath's default flatness 0.25 and others
    for i in 0..80 {
        let add = match i % 5 {
            0 => format!("ellipse {} {} {} {}", q4(&mut r, -4, 20), q4(&mut r, -4, 20), q4(&mut r, 1, 80), q4(&mut r, 1, 80)),
            1 => format!("arc {} {} {} {} {} {}", q4(&mut r, -4, 20), q4(&mut r, -4, 20), q4(&mut r, 1, 80), q4(&mut r, 1, 80), r.range(-360, 360), r.range(-360, 360)),
            2 => format!("bezier {}", pts(&mut r, 4)),
            3 => format!("curve {} 4 {}", r.range(0, 8) as f32 / 4.0, pts(&mut r, 4)),
            _ => format!("pie {} {} {} {} {} {}", q4(&mut r, -4, 20), q4(&mut r, -4, 20), q4(&mut r, 1, 80), q4(&mut r, 1, 80), r.range(-360, 360), r.range(-360, 360)),
        };
        let fl = *r.pick(&["0.25", "0.25", "0.25", "0.1", "1", "2", "0.01", "0"]);
        writeln!(s, "case pathflat-{i}\ngppath 0 0\ngpadd 0 {add}\ngpflat 0 - {fl}\ngppathinfo 0\nend");
    }
    // widened by pens of widths, caps and joins
    for i in 0..40 {
        let add = match i % 4 {
            0 => format!("line {} {} {} {}", q4(&mut r, 0, 30), q4(&mut r, 0, 30), q4(&mut r, 0, 30), q4(&mut r, 0, 30)),
            1 => format!("lines 3 {}", pts(&mut r, 3)),
            2 => format!("rect {} {} {} {}", q4(&mut r, 0, 20), q4(&mut r, 0, 20), q4(&mut r, 1, 20), q4(&mut r, 1, 20)),
            _ => format!("ellipse {} {} {} {}", q4(&mut r, 0, 20), q4(&mut r, 0, 20), q4(&mut r, 1, 20), q4(&mut r, 1, 20)),
        };
        let cap = *r.pick(&[0, 1, 2, 3]);
        writeln!(s, "case pathwiden-{i}\ngppen 0 0xff000000 {} cap {cap} {cap} 0 join {}\ngppath 0 0\ngpadd 0 {add}\ngpwiden 0 0 - 0.25\ngppathinfo 0\nend", r.range(1, 24) as f32 / 4.0, r.range(0, 3));
    }
    // filled: a path's ellipse against the same ellipse flattened first
    for i in 0..30 {
        let (sm, po) = (*r.pick(&[3, 4, 5]), *r.pick(&[3, 4]));
        let e = format!("ellipse {} {} {} {}", q4(&mut r, 0, 10), q4(&mut r, 0, 10), q4(&mut r, 8, 40), q4(&mut r, 8, 40));
        let flat = if i % 2 == 1 { "gpflat 0 - 0.25\n" } else { "" };
        writeln!(s, "case pathfill-{i}\ngpbitmap 0 48 48 32bppARGB\ngpbrush 0 solid 0xff00ff00\ngppath 0 0\ngpadd 0 {e}\n{flat}gpfillpath 0 0 0 copy 0 {sm} {po}\ngpread 0 32bppARGB\nend");
    }
    // (a path made with a FillMode past 0 ..= 1, filled; two crossing figures tell the modes
    // apart; and the ops' order of complaints)
    for (i, fm) in [2, -1, 3, 1, 0].into_iter().enumerate() {
        writeln!(s, "case pathfillmode-{i}\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 solid 0xff0000ff\ngppath 0 {fm}\ngpadd 0 rect 1 1 10 10\ngpadd 0 rect 4 4 10 10\ngpfillpath 0 0 0 copy 0 0 0\ngpread 0 32bppARGB\nend");
    }
    writeln!(s, "case pathfillwho-0\ngpfillpath 0 0 0 copy 0 0 0\ngpbitmap 0 4 4 32bppARGB\ngpfillpath 0 0 0 copy 0 0 0\ngppath 0 0\ngpfillpath 0 0 0 copy 0 0 0\ngpbrush 0 solid 0xff0000ff\ngpfillpath 0 0 0 copy 0 9 0\ngpfillpath 0 0 0 copy 0 0 9\ngpbitmap 1 4 4 16bppGrayScale\ngpfillpath 1 0 0 copy 0 0 0\nend");
    // widened by the pen objects (compound arrays, arrow and custom caps)
    for i in 0..40 {
        let add = match i % 3 {
            0 => format!("lines 3 {}", pts(&mut r, 3)),
            1 => format!("line {} {} {} {}", q4(&mut r, 0, 30), q4(&mut r, 0, 30), q4(&mut r, 0, 30), q4(&mut r, 0, 30)),
            _ => format!("rect {} {} {} {}", q4(&mut r, 0, 20), q4(&mut r, 0, 20), q4(&mut r, 1, 20), q4(&mut r, 1, 20)),
        };
        writeln!(s, "case pathwidenobj-{i}\ngppen 0 0xff000000 {} cap {} {} 0 join {} {}\ngppath 0 0\ngpadd 0 {add}\ngpwiden 0 0 - 0.25\ngppathinfo 0\nend", r.range(4, 36) as f32 / 4.0, r.range(0, 4), r.range(0, 4), r.range(0, 4), pen_object(&mut r, i));
    }
    // widened through matrices of no inverse (zero, rank 1, an infinite element)
    let singular = ["0x00000000 0x00000000 0x00000000 0x00000000 0x00000000 0x00000000", "0x3f800000 0x40000000 0x40000000 0x40800000 0x00000000 0x00000000", "0x00000000 0x00000000 0x00000000 0x3f800000 0x40400000 0x40800000", "0x7f800000 0x00000000 0x00000000 0x3f800000 0x00000000 0x00000000"];
    for (i, m) in singular.iter().enumerate() {
        for (j, add) in ["line 2 3 20 9", "rect 4 4 10 6", "bezier 1 1 8 20 16 -4 24 10"].iter().enumerate() {
            writeln!(s, "case pathwidensing-{i}-{j}\ngpmatrix 0 {m}\ngppen 0 0xff000000 {}\ngppath 0 0\ngpadd 0 {add}\ngpwiden 0 0 0 0.25\ngppathinfo 0\nend", 1 + j * 2);
        }
    }
    s
}
