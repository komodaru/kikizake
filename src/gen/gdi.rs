//! The gdi suites' case generators.
use super::*;

pub fn gdi() -> String {
    let mut s = String::from("# kikizake suite gdi: PatBlt and BitBlt into DIB sections (SPEC.md). Made by `kikizake gen gdi`.\n");
    // every ternary operation on every format, with a pattern brush so that P varies by pixel:
    // one 1x1 BitBlt per operation, its own pixel of a 16x16 picture
    for f in FORMATS {
        for brush in ["solid", "pattern"] {
            let mut r = XorShift32::new(0x5eed_0000 + f.0 * 7 + brush.len() as u32);
            writeln!(s, "case rop3-{}-{brush}", fmt_name(f)).unwrap();
            writeln!(s, "dib 0 16 16 {}{}", f.0, f.1).unwrap();
            writeln!(s, "fill 0 {}", r.next()).unwrap();
            writeln!(s, "dib 1 16 16 {}{}", f.0, f.1).unwrap();
            writeln!(s, "fill 1 {}", r.next()).unwrap();
            if brush == "solid" {
                writeln!(s, "brush 0 solid #{:06x}", r.next() & 0xffffff).unwrap();
            } else {
                writeln!(s, "dib 2 8 8 32").unwrap();
                writeln!(s, "fill 2 {}", r.next()).unwrap();
                writeln!(s, "brush 0 dib 2").unwrap();
            }
            for i in 0..256u32 {
                let (x, y) = (i % 16, i / 16);
                writeln!(s, "bitblt 0 {x} {y} 1 1 1 {x} {y} {:#x}", i << 16).unwrap();
            }
            writeln!(s, "dump 0\nend").unwrap();
        }
    }
    // named operations over whole pictures between every pair of formats
    let mut r = XorShift32::new(0x1234_5678);
    for d in FORMATS {
        for sf in FORMATS {
            if (d.0 == 1) != (sf.0 == 1) {
                continue; // (monochrome conversions: their own cases below)
            }
            writeln!(s, "case convert-{}-from-{}", fmt_name(d), fmt_name(sf)).unwrap();
            let (l, _) = dib_line(&mut r, 0, 7, 5, d, false);
            writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
            let (l, _) = dib_line(&mut r, 1, 7, 5, sf, false);
            writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
            writeln!(s, "bitblt 0 0 0 7 5 1 0 0 0xcc0020\ndump 0\nend").unwrap();
        }
    }
    // monochrome to and from color (the documented text/background color rules): unmodeled
    // until Windows' answers say how DIB sections with color tables follow them
    for (i, f) in FORMATS.iter().filter(|f| f.0 != 1).enumerate() {
        for dir in ["to", "from"] {
            writeln!(s, "case mono-{dir}-{}", fmt_name(f)).unwrap();
            let (a, b) = if dir == "to" { ((1, ""), *f) } else { (*f, (1, "")) };
            let (l, _) = dib_line(&mut r, 0, 9, 4, &a, i % 2 == 1);
            writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
            let (l, pal) = dib_line(&mut r, 1, 9, 4, &b, false);
            writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
            writeln!(s, "text 0 #{:06x}\nbk 0 #{:06x}", rcolor(&mut r, &[]), rcolor(&mut r, &[])).unwrap();
            writeln!(s, "text 1 #{:06x}\nbk 1 #{:06x}", rcolor(&mut r, &pal), rcolor(&mut r, &pal)).unwrap();
            // (a pixel equal to the background color, to see the color -> mono rule)
            if b.0 >= 8 {
                writeln!(s, "poke 1 2 1 {}", "ff".repeat(b.0 as usize / 8)).unwrap();
            }
            writeln!(s, "bitblt 0 0 0 9 4 1 0 0 0xcc0020\ndump 0\nend").unwrap();
        }
    }
    // random: formats, sizes, row orders, palettes, brushes, origins, clipping, borders, overlap
    let mut r = XorShift32::new(0xc0ff_ee00);
    for i in 0..400 {
        writeln!(s, "case random-{i}").unwrap();
        let df = *r.pick(FORMATS);
        let (w, h) = (r.range(1, 13), r.range(1, 7));
        let custom = r.chance(4);
        let (l, dpal) = dib_line(&mut r, 0, w, h, &df, custom);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        let same = r.chance(5);
        let (sw, sh) = (r.range(1, 16), r.range(1, 9));
        if !same {
            // (a monochrome source with a color destination half as often as the rest)
            let sf = if r.chance(2) { df } else { *r.pick(FORMATS) };
            let custom = r.chance(4);
            let (l, _) = dib_line(&mut r, 1, sw, sh, &sf, custom);
            writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
        }
        let src = if same { 0 } else { 1 };
        let (sw, sh) = if same { (w, h) } else { (sw, sh) };
        match r.range(0, 9) {
            0..=4 => writeln!(s, "brush 0 solid #{:06x}", rcolor(&mut r, &dpal)).unwrap(),
            5..=7 => {
                let pf = *r.pick(&[(32, ""), (24, ""), (8, ""), (16, ""), (1, "")]);
                let (pw, ph) = (r.range(1, 9), r.range(1, 9));
                let (l, _) = dib_line(&mut r, 2, pw, ph, &pf, false);
                writeln!(s, "{l}\nfill 2 {}\nbrush 0 dib 2", r.next()).unwrap();
            }
            8 => writeln!(s, "brush 0 hatch {} #{:06x}", r.range(0, 5), rcolor(&mut r, &dpal)).unwrap(),
            _ => writeln!(s, "brush 0 null").unwrap(),
        }
        if r.chance(3) {
            writeln!(s, "org 0 {} {}", r.range(-9, 9), r.range(-9, 9)).unwrap();
        }
        if r.chance(3) {
            writeln!(s, "text 0 #{:06x}\nbk 0 #{:06x}", rcolor(&mut r, &dpal), rcolor(&mut r, &dpal)).unwrap();
        }
        if r.chance(4) {
            let (x0, y0) = (r.range(-2, w), r.range(-2, h));
            writeln!(s, "clip 0 {x0} {y0} {} {}", x0 + r.range(0, w + 2), y0 + r.range(0, h + 2)).unwrap();
        }
        for _ in 0..r.range(1, 3) {
            let (x, y) = (r.range(-3, w + 1), r.range(-3, h + 1));
            let (bw, bh) = if r.chance(25) { (r.range(-4, -1), r.range(1, h)) } else { (r.range(0, w + 3), r.range(0, h + 3)) };
            let mut op = rop(&mut r);
            if r.chance(40) {
                op |= *r.pick(&[0x4000_0000u32, 0x8000_0000]);
            }
            if r.chance(5) && !crate::model::gdi::uses_s((op >> 16) as u8) {
                writeln!(s, "patblt 0 {x} {y} {bw} {bh} {op:#x}").unwrap();
            } else {
                writeln!(s, "bitblt 0 {x} {y} {bw} {bh} {src} {} {} {op:#x}", r.range(-3, sw), r.range(-3, sh)).unwrap();
            }
        }
        writeln!(s, "dump 0\nend").unwrap();
    }
    // what PatBlt does with an operation that needs a source, and the high flag bits
    for (i, op) in [0x00CC0020u32, 0x00660046, 0x40F00021, 0x80F00021, 0x405A0049].iter().enumerate() {
        writeln!(s, "case edge-patblt-{i}\ndib 0 6 3 32\nfill 0 {}\nbrush 0 solid #12ab34\npatblt 0 1 0 4 3 {op:#x}\ndump 0\nend", 77 + i).unwrap();
    }
    // IntersectClipRect with a rectangle not ordered; CreateDIBSection of a bit count GDI lacks
    writeln!(s, "case edge-clip-twice\ndib 0 6 4 32\nfill 0 5\nclip 0 1 1 5 3\nclip 0 2 0 4 4\nclip 0 0 0 1 1\nclip 0 0 0 6 4\nbrush 0 solid #00ff00\npatblt 0 0 0 6 4 0xf00021\ndump 0\nend").unwrap();
    writeln!(s, "case edge-clip-unordered\ndib 0 6 4 32\nfill 0 5\nclip 0 5 3 1 1\nbrush 0 solid #00ff00\npatblt 0 0 0 6 4 0xf00021\ndump 0\nend").unwrap();
    writeln!(s, "case edge-dib-2bpp\ndib 0 4 2 2\nend").unwrap();
    // negative extents (mirroring or normalizing?)
    for (i, (w, h)) in [(-3i32, 2i32), (3, -2), (-3, -2)].iter().enumerate() {
        writeln!(s, "case edge-negative-{i}\ndib 0 8 4 32\nfill 0 {}\ndib 1 8 4 32\nfill 1 {}\nbitblt 0 4 2 {w} {h} 1 4 2 0xcc0020\ndump 0\npatblt 0 4 2 {w} {h} 0x550009\ndump 0\nend", 90 + i, 95 + i).unwrap();
    }
    s
}

pub fn stretch() -> String {
    let mut s = String::from("# kikizake suite stretch: StretchBlt in each stretch mode (SPEC.md). Made by `kikizake gen stretch`.\n");
    let mut r = XorShift32::new(0x57e7_c400);
    // every enlargement and reduction of a row and of a column, 1..12 to 1..12, COLORONCOLOR
    for (dir, (a, b)) in [("x", (12, 1)), ("y", (1, 12))].iter().enumerate().map(|(i, d)| (i, d.1)) {
        let axis = if dir == 0 { "x" } else { "y" };
        writeln!(s, "case scale-{axis}\ndib 1 {a} {b} 32\nfill 1 {}", r.next()).unwrap();
        for dn in 1..=12 {
            // (a new DIB in the slot: a new DC, BLACKONWHITE again)
            writeln!(s, "dib 0 {a} {b} 32\nfill 0 7\nmode 0 3").unwrap();
            for sn in 1..=12 {
                let (dw, dh, sw, sh) = if dir == 0 { (dn, 1, sn, 1) } else { (1, dn, 1, sn) };
                writeln!(s, "stretchblt 0 0 0 {dw} {dh} 1 0 0 {sw} {sh} 0xcc0020").unwrap();
                writeln!(s, "dump 0").unwrap();
            }
        }
        writeln!(s, "end").unwrap();
    }
    for i in 0..250 {
        writeln!(s, "case random-{i}").unwrap();
        let df = *r.pick(FORMATS);
        let sf = if r.chance(2) { df } else { *r.pick(FORMATS) };
        let (w, h, sw, sh) = (r.range(1, 12), r.range(1, 8), r.range(1, 12), r.range(1, 8));
        let (l, _) = dib_line(&mut r, 0, w, h, &df, false);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        let (l, _) = dib_line(&mut r, 1, sw, sh, &sf, false);
        writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
        writeln!(s, "mode 0 {}", r.range(1, 4)).unwrap();
        if r.chance(3) {
            writeln!(s, "brush 0 solid #{:06x}", r.next() & 0xffffff).unwrap();
        }
        let ext = |r: &mut XorShift32, n: i64| if r.chance(12) { -r.range(1, n) } else { r.range(1, n + 2) };
        let (dw, dh) = (ext(&mut r, w), ext(&mut r, h));
        let (qw, qh) = (ext(&mut r, sw), ext(&mut r, sh));
        let op = if r.chance(3) { rop(&mut r) } else { 0xcc0020 };
        writeln!(s, "stretchblt 0 {} {} {dw} {dh} 1 {} {} {qw} {qh} {op:#x}\ndump 0\nend", r.range(-1, w - 1), r.range(-1, h - 1), r.range(0, sw - 1), r.range(0, sh - 1)).unwrap();
    }
    s
}

pub fn pixel() -> String {
    let mut s = String::from("# kikizake suite pixel: SetPixel, GetPixel, GetDIBits and SetDIBits between formats (SPEC.md). Made by `kikizake gen pixel`.\n");
    let mut r = XorShift32::new(0x9123_4567);
    for f in FORMATS {
        writeln!(s, "case pixel-{}", fmt_name(f)).unwrap();
        let (l, pal) = dib_line(&mut r, 0, 6, 3, f, false);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        for y in 0..3 {
            for x in 0..6 {
                writeln!(s, "getpixel 0 {x} {y}").unwrap();
            }
        }
        for _ in 0..24 {
            writeln!(s, "setpixel 0 {} {} #{:06x}", r.range(-1, 6), r.range(-1, 3), rcolor(&mut r, &pal)).unwrap();
        }
        writeln!(s, "clip 0 1 1 4 2\ngetpixel 0 0 0\ngetpixel 0 2 1\nsetpixel 0 0 0 #ffffff\nsetpixel 0 3 1 #000000\ndump 0\nend").unwrap();
    }
    for d in FORMATS {
        for o in FORMATS {
            let ob = format!("{}{}", o.0, o.1);
            writeln!(s, "case dibits-{}-{}", fmt_name(d), fmt_name(o)).unwrap();
            let (l, _) = dib_line(&mut r, 0, 7, 3, d, false);
            writeln!(s, "{l}\nfill 0 {}\ngetdibits 0 {ob}\nsetdibits 0 {ob} {}\ndump 0\nend", r.next(), r.next()).unwrap();
        }
    }
    s
}
