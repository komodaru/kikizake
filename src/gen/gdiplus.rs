//! The gdiplus suite's case generator.
use super::*;
use crate::model::gdiplus::FORMATS;

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
    let names: Vec<&str> = FORMATS.iter().map(|f| f.0).collect();
    let mut seed = 1u32;
    let mut next = || {
        seed += 1;
        seed
    };
    // a new bitmap of every format: what it holds, its palette
    for f in &names {
        writeln!(s, "case create-{f}\ngpbitmap 0 5 3 {f}\ngpinfo 0\ngpread 0 {f}\nend").unwrap();
    }
    for (i, (w, h)) in [(0, 1), (1, 0), (-1, 4), (4, -1), (1, 1), (1, 1000)].iter().enumerate() {
        writeln!(s, "case size-{i}\ngpbitmap 0 {w} {h} 32bppARGB\ngpinfo 0\nend").unwrap();
    }
    // every pair through LockBits: written in the bitmap's own format, read in another; written
    // in another, read in its own
    for from in &names {
        for to in &names {
            writeln!(s, "case read-{from}-{to}\ngpbitmap 0 9 3 {from}\ngpwrite 0 {from} seed {}\ngpread 0 {to}\nend", next()).unwrap();
            writeln!(s, "case write-{from}-{to}\ngpbitmap 0 9 3 {from}\ngpwrite 0 {to} seed {}\ngpread 0 {from}\nend", next()).unwrap();
        }
    }
    // every (color, alpha) pair between straight and premultiplied, and to 16-bit channels; every
    // 16-bit level back to 8 bits
    for (from, to) in [("32bppARGB", "32bppPARGB"), ("32bppPARGB", "32bppARGB"), ("32bppARGB", "64bppARGB"), ("32bppARGB", "64bppPARGB"), ("32bppPARGB", "64bppPARGB")] {
        writeln!(s, "case pairs-{from}-{to}\ngpbitmap 0 256 256 {from}\ngpwrite 0 {from} pairs\ngpread 0 {to}\nend").unwrap();
    }
    for f in ["48bppRGB", "64bppARGB", "64bppPARGB"] {
        writeln!(s, "case levels-{f}\ngpbitmap 0 256 33 {f}\ngpwrite 0 {f} levels\ngpread 0 32bppARGB\ngpread 0 {f}\nend").unwrap();
    }
    // every 5-bit color to the palettes (the nearest entry)
    for f in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
        writeln!(s, "case grid-{f}\ngpbitmap 0 256 128 24bppRGB\ngpwrite 0 24bppRGB grid\ngpread 0 {f}\nend").unwrap();
    }
    // the bytes of a format as GetPixel reads them, and SetPixel
    let mut r = XorShift32::new(0x6d1f_0001);
    for f in &names {
        for k in 0..3 {
            write!(s, "case pixel-{f}-{k}\ngpbitmap 0 4 3 {f}\ngpwrite 0 {f} seed {}\n", r.next()).unwrap();
            for _ in 0..4 {
                let (x, y) = (r.range(-1, 4), r.range(-1, 3));
                writeln!(s, "gpget 0 {x} {y}").unwrap();
            }
            for _ in 0..6 {
                let (x, y) = (r.range(-1, 4), r.range(-1, 3));
                writeln!(s, "gpset 0 {x} {y} {:#010x}\ngpget 0 {x} {y}", rargb(&mut r)).unwrap();
            }
            writeln!(s, "gpread 0 {f}\nend").unwrap();
        }
    }
    // locked rectangles inside, across and outside the bitmap
    for i in 0..40 {
        let f = *r.pick(&names);
        let (x, y, w, h) = (r.range(-2, 6), r.range(-2, 4), r.range(-1, 7), r.range(-1, 5));
        let g = *r.pick(&names);
        writeln!(s, "case lock-{i}\ngpbitmap 0 6 4 {f}\ngpwrite 0 {f} seed {}\ngpread 0 {g} at {x} {y} {w} {h}\ngpwrite 0 {g} seed {} at {x} {y} {w} {h}\ngpread 0 {f}\nend", r.next(), r.next()).unwrap();
    }
    // fills and clears: every format, both compositing modes, every quality
    for f in &names {
        for (m, mode) in ["over", "copy"].iter().enumerate() {
            for q in 0..5 {
                let (x, y, w, h) = (r.range(-3, 8), r.range(-2, 4), r.range(-2, 10), r.range(-2, 6));
                writeln!(s, "case fill-{f}-{mode}-{q}\ngpbitmap 0 10 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 {x} {y} {w} {h} {:#010x} {mode} {q}\ngpread 0 {f}\nend", r.next(), rargb(&mut r)).unwrap();
                if m == 0 && q == 0 {
                    writeln!(s, "case clear-{f}\ngpbitmap 0 3 2 {f}\ngpclear 0 {:#010x}\ngpread 0 {f}\nend", rargb(&mut r)).unwrap();
                }
            }
        }
    }
    // every gray level in an 8x8 block, on the 16-bit formats (where drawing dithers), and with
    // the blocks moved off the 8-pixel grid
    for (f, o) in [("16bppRGB555", 0), ("16bppRGB565", 0), ("16bppRGB565", 3)] {
        write!(s, "case dither-{f}-{o}\ngpbitmap 0 {} {} {f}\n", 128 + o, 128 + o).unwrap();
        for v in 0..256 {
            writeln!(s, "gpfill 0 {} {} 8 8 {:#010x} copy 0", o + v % 16 * 8, o + v / 16 * 8, 0xff00_0000u32 | v * 0x01_0101).unwrap();
        }
        writeln!(s, "gpread 0 {f}\nend").unwrap();
    }
    // source over every (color, alpha) of the destination, for a few sources
    for (i, c) in [0x80ff_4010u32, 0x40_20c0f0, 0xc0_7f8001, 0x01_ffffff, 0xfe_000000].iter().enumerate() {
        let q = if i == 4 { 3 } else { 0 };
        writeln!(s, "case over-{i}\ngpbitmap 0 256 256 32bppARGB\ngpwrite 0 32bppARGB pairs\ngpfill 0 0 0 256 256 {c:#010x} over {q}\ngpread 0 32bppARGB\nend").unwrap();
    }
    for i in 0..60 {
        let f = *r.pick(&["32bppARGB", "32bppPARGB", "32bppRGB", "24bppRGB", "16bppRGB565", "16bppARGB1555", "64bppARGB"]);
        let q = r.range(0, 4);
        writeln!(s, "case blend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 0 0 16 4 {:#010x} over {q}\ngpread 0 {f}\nend", r.next(), r.next()).unwrap();
    }
    // rotations and flips
    for f in &names {
        for t in 0..8 {
            writeln!(s, "case rotate-{f}-{t}\ngpbitmap 0 5 3 {f}\ngpwrite 0 {f} seed {}\ngprotate 0 {t}\ngpread 0 {f}\nend", r.next()).unwrap();
        }
    }
    // clones of an area into any format
    for i in 0..80 {
        let (f, g) = (*r.pick(&names), *r.pick(&names));
        let (x, y, w, h) = (r.range(-1, 6), r.range(-1, 4), r.range(-1, 7), r.range(-1, 5));
        writeln!(s, "case clone-{i}\ngpbitmap 0 7 4 {f}\ngpwrite 0 {f} seed {}\ngpclone 1 0 {x} {y} {w} {h} {g}\ngpinfo 1\ngpread 1 {g}\nend", r.next()).unwrap();
    }
    // clones into the indexed formats: of every other format, whole, and of every 5-bit color
    for f in &names {
        for g in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
            if *f == "16bppGrayScale" {
                continue;
            }
            writeln!(s, "case cloneidx-{f}-{g}\ngpbitmap 0 16 16 {f}\ngpwrite 0 {f} seed {}\ngpclone 1 0 0 0 16 16 {g}\ngpinfo 1\ngpread 1 {g}\nend", r.next()).unwrap();
        }
    }
    for g in ["1bppIndexed", "4bppIndexed", "8bppIndexed"] {
        writeln!(s, "case clonegrid-{g}\ngpbitmap 0 256 128 24bppRGB\ngpwrite 0 24bppRGB grid\ngpclone 1 0 0 0 256 128 {g}\ngpinfo 1\ngpread 1 {g}\nend").unwrap();
        writeln!(s, "case clonealpha-{g}\ngpbitmap 0 256 256 32bppARGB\ngpwrite 0 32bppARGB pairs\ngpclone 1 0 0 0 256 256 {g}\ngpread 1 {g}\nend").unwrap();
    }
    // the gamma-corrected blend over every (color, alpha) of the destination, for a few brushes
    for (i, (f, c, q)) in [("32bppARGB", 0xfe00_0000u32, 2), ("32bppARGB", 0x80ff_ffff, 3), ("32bppARGB", 0x8000_0000, 3), ("32bppARGB", 0x40ff_8000, 3), ("32bppARGB", 0xc010_80f0, 2), ("32bppARGB", 0x01ff_ffff, 3), ("32bppPARGB", 0x80ff_ffff, 3), ("32bppPARGB", 0xc010_80f0, 3)].iter().enumerate() {
        writeln!(s, "case gamma-{i}\ngpbitmap 0 256 256 {f}\ngpwrite 0 {f} pairs\ngpfill 0 0 0 256 256 {c:#010x} over {q}\ngpread 0 {f}\nend").unwrap();
    }
    for i in 0..60 {
        let f = *r.pick(&names);
        writeln!(s, "case gammablend-{i}\ngpbitmap 0 16 4 {f}\ngpwrite 0 {f} seed {}\ngpfill 0 0 0 16 4 {:#010x} over {}\ngpread 0 {f}\nend", r.next(), rargb(&mut r), r.range(2, 4)).unwrap();
    }
    // matrices: made, multiplied both ways, inverted, points through them (more in gdiplusmat)
    for i in 0..120 {
        let m: Vec<String> = (0..12).map(|k| {
            // (some singular ones: a row that is a multiple of the other)
            if i % 6 == 5 && k == 2 { "0".into() } else if i % 6 == 5 && k == 3 { "0".into() } else { format!("{:#x}", rfloat(&mut r)) }
        }).collect();
        let pts: Vec<String> = (0..6).map(|_| format!("{:#x}", rfloat(&mut r))).collect();
        writeln!(s, "case matrix-{i}\ngpmatrix 0 {}\ngpmatrix 1 {}\ngpmatpts 0 {}\ngpmatmul 0 1 {}\ngpmatpts 0 {}\ngpmatinv 0\ngpmatpts 0 {}\nend", m[..6].join(" "), m[6..].join(" "), pts.join(" "), i % 2, pts.join(" "), pts.join(" ")).unwrap();
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
        writeln!(s, "case matrix-{i}\ngpmatrix 0 {a}\ngpmatrix 1 {b}\ngpmatpts 0 {p}\ngpmatmul 0 1 {}\ngpmatpts 0 {p}\ngpmatinv 0\ngpmatpts 0 {p}\ngpmatinv 0\nend", i % 2).unwrap();
    }
    writeln!(s, "case matrix-order\ngpmatrix 0 0x3f800000 0 0 0x3f800000 0 0\ngpmatrix 1 0x40000000 0 0 0x40000000 0 0\ngpmatmul 0 1 2\ngpmatmul 0 1 -1\nend").unwrap();
    s
}
