//! The blend suite: AlphaBlend, TransparentBlt, GradientFill, MaskBlt, PlgBlt.
use super::*;

pub fn blend() -> String {
    let mut s = String::from("# kikizake suite blend: AlphaBlend, TransparentBlt, GradientFill, MaskBlt, PlgBlt (SPEC.md). Made by `kikizake gen blend`.\n");
    let mut r = XorShift32::new(0xb1e0_0000);
    // AlphaBlend with every constant alpha, 32 bpp onto 32 bpp, 64 random pixel pairs each
    for a in 0..256 {
        writeln!(s, "case constant-{a}\ndib 0 8 8 32\nfill 0 {}\ndib 1 8 8 32\nfill 1 {}\nalphablend 0 0 0 8 8 1 0 0 8 8 {a} 0\ndump 0\nend", r.next(), r.next()).unwrap();
    }
    // per pixel alpha (AC_SRC_ALPHA) with constant alphas
    for (i, a) in [255, 254, 128, 127, 64, 1, 0, 200].iter().enumerate() {
        for k in 0..4 {
            writeln!(s, "case perpixel-{i}-{k}\ndib 0 8 8 32\nfill 0 {}\ndib 1 8 8 32\nfill 1 {}\nalphablend 0 0 0 8 8 1 0 0 8 8 {a} 1\ndump 0\nend", r.next(), r.next()).unwrap();
        }
    }
    // premultiplied sources (color <= alpha), the documented case
    for k in 0..8 {
        writeln!(s, "case premul-{k}\ndib 0 8 8 32\nfill 0 {}\ndib 1 8 8 32", r.next()).unwrap();
        for y in 0..8 {
            for x in 0..8 {
                let a = r.range(0, 255) as u32;
                let c = |r: &mut XorShift32| r.range(0, a as i64) as u32;
                writeln!(s, "poke 1 {x} {y} {:02x}{:02x}{:02x}{a:02x}", c(&mut r), c(&mut r), c(&mut r)).unwrap();
            }
        }
        writeln!(s, "alphablend 0 0 0 8 8 1 0 0 8 8 {} 1\ndump 0\nend", *r.pick(&[255, 128, 77])).unwrap();
    }
    // between formats, stretched, clipped, out of bounds
    for i in 0..120 {
        writeln!(s, "case alpha-{i}").unwrap();
        let df = *r.pick(FORMATS);
        let sf = *r.pick(&[(32, ""), (32, ""), (24, ""), (16, ""), (8, "")]);
        let (w, h, sw, sh) = (r.range(1, 10), r.range(1, 8), r.range(1, 10), r.range(1, 8));
        let (l, _) = dib_line(&mut r, 0, w, h, &df, false);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        let (l, _) = dib_line(&mut r, 1, sw, sh, &sf, false);
        writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
        if r.chance(4) {
            writeln!(s, "clip 0 1 1 {} {}", w, h).unwrap();
        }
        let flat = r.chance(2);
        let (dw, dh) = if flat { (r.range(0, sw), r.range(0, sh)) } else { (r.range(-1, w + 2), r.range(-1, h + 2)) };
        let (qw, qh) = if flat { (dw, dh) } else { (r.range(-1, sw + 1), r.range(-1, sh + 1)) };
        writeln!(s, "alphablend 0 {} {} {dw} {dh} 1 {} {} {qw} {qh} {} {}\ndump 0\nend", r.range(-2, w), r.range(-2, h), r.range(-1, sw - 1), r.range(-1, sh - 1), *r.pick(&[255, 128, 30, 0, 254]), r.range(0, 1)).unwrap();
    }
    // TransparentBlt: the key color, between formats, stretched
    for i in 0..80 {
        writeln!(s, "case transparent-{i}").unwrap();
        let df = *r.pick(FORMATS);
        let sf = *r.pick(FORMATS);
        let (w, h, sw, sh) = (r.range(1, 10), r.range(1, 8), r.range(1, 10), r.range(1, 8));
        let (l, _) = dib_line(&mut r, 0, w, h, &df, false);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        let (l, spal) = dib_line(&mut r, 1, sw, sh, &sf, false);
        writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
        // (a key that some source pixels have)
        let key = if spal.is_empty() { r.next() & 0xffffff } else { *r.pick(&spal) };
        let key = if sf.0 == 32 && !r.chance(3) { 0 } else { key };
        if sf.0 >= 24 && key == 0 {
            writeln!(s, "poke 1 0 0 {}", "00".repeat(sf.0 as usize / 8)).unwrap();
        }
        let flat = r.chance(2);
        let (dw, dh) = if flat { (sw, sh) } else { (r.range(1, w + 2), r.range(1, h + 2)) };
        writeln!(s, "transparent 0 {} {} {dw} {dh} 1 0 0 {sw} {sh} #{key:06x}\ndump 0\nend", r.range(-2, w), r.range(-2, h)).unwrap();
    }
    // GradientFill: rectangles (both directions) of every width and height up to 12, triangles
    // (the color-table rectangles in cases of their own, after: gradrect8-*)
    let mut s8 = String::new();
    for mode in 0..2 {
        for n in 1..=12 {
            writeln!(s, "case gradrect-{mode}-{n}").unwrap();
            writeln!(s8, "case gradrect8-{mode}-{n}").unwrap();
            for f in [(32, ""), (24, ""), (16, ""), (16, " 565"), (8, "")] {
                let o = if f.0 == 8 { &mut s8 } else { &mut s };
                let (w, h) = if mode == 0 { (n, 2) } else { (2, n) };
                writeln!(o, "dib 0 {w} {h} {}{}", f.0, f.1).unwrap();
                let c = |r: &mut XorShift32| format!("{} {} {} {}", r.next() & 0xffff, r.next() & 0xffff, r.next() & 0xffff, r.next() & 0xffff);
                writeln!(o, "gradrect 0 {mode} 0 0 {} {w} {h} {}\ndump 0", c(&mut r), c(&mut r)).unwrap();
            }
            writeln!(s, "end").unwrap();
            writeln!(s8, "end").unwrap();
        }
    }
    s.push_str(&s8);
    // (a random stream of their own: the cases after keep theirs)
    let mut q = XorShift32::new(0x6ead_0001);
    let c16 = |q: &mut XorShift32, lo: u32| format!("{} {} {} {}", lo + q.next() % (0x1_0000 - lo), lo + q.next() % (0x1_0000 - lo), lo + q.next() % (0x1_0000 - lo), lo + q.next() % (0x1_0000 - lo));
    // triangles in every format
    for i in 0..36 {
        let f = [(16, ""), (16, " 565"), (24, ""), (8, ""), (4, ""), (1, "")][i % 6];
        writeln!(s, "case gradtri-fmt-{i}\ndib 0 16 16 {}{}", f.0, f.1).unwrap();
        let v = |q: &mut XorShift32| format!("{} {} {}", q.range(-4, 20), q.range(-4, 20), c16(q, 0));
        writeln!(s, "gradtri 0 {} {} {}\ndump 0\nend", v(&mut q), v(&mut q), v(&mut q)).unwrap();
    }
    // channels near the top (0xff00 and above): triangles and rectangles
    for i in 0..24 {
        let v = |q: &mut XorShift32| format!("{} {} {}", q.range(-4, 20), q.range(-4, 20), c16(q, 0xfe00));
        writeln!(s, "case gradtri-hi-{i}\ndib 0 16 16 32\ngradtri 0 {} {} {}\ndump 0\nend", v(&mut q), v(&mut q), v(&mut q)).unwrap();
        writeln!(s, "case gradrect-hi-{i}\ndib 0 16 4 32\ngradrect 0 {} 0 0 {} 16 4 {}\ndump 0\nend", i % 2, c16(&mut q, 0xfe00), c16(&mut q, 0xfe00)).unwrap();
    }
    // the color tables' dither: 16 by 16 ramps of one channel at a time, and random ones
    for (i, f) in [8, 4, 1].iter().enumerate() {
        for ch in 0..3 {
            let mut c0 = [0u32; 4];
            let mut c1 = [0u32; 4];
            c1[ch] = 0xffff;
            let cs = |c: [u32; 4]| format!("{} {} {} {}", c[0], c[1], c[2], c[3]);
            writeln!(s, "case gradrect-dither-{f}-{ch}\ndib 0 16 16 {f}\ngradrect 0 0 0 0 {} 16 16 {}\ndump 0\ngradrect 0 1 0 0 {} 16 16 {}\ndump 0\nend", cs(c0), cs(c1), cs(c0), cs(c1)).unwrap();
            c0[ch] = 0x8000;
            writeln!(s, "case gradrect-flat-{f}-{ch}\ndib 0 16 16 {f}\ngradrect 0 0 0 0 {} 16 16 {}\ndump 0\nend", cs(c0), cs(c0)).unwrap();
        }
        for j in 0..4 {
            writeln!(s, "case gradrect-dither-{f}-r{j}\ndib 0 16 16 {f}\ngradrect 0 {} 0 0 {} 16 16 {}\ndump 0\nend", (i + j) % 2, c16(&mut q, 0), c16(&mut q, 0)).unwrap();
        }
    }
    for i in 0..60 {
        writeln!(s, "case gradtri-{i}\ndib 0 16 16 32").unwrap();
        let v = |r: &mut XorShift32| format!("{} {} {} {} {} {}", r.range(-4, 20), r.range(-4, 20), r.next() & 0xffff, r.next() & 0xffff, r.next() & 0xffff, r.next() & 0xffff);
        writeln!(s, "gradtri 0 {} {} {}\ndump 0\nend", v(&mut r), v(&mut r), v(&mut r)).unwrap();
    }
    writeln!(s, "case gradrect-edges\ndib 0 6 4 32\ngradrect 0 0 4 0 65535 0 0 0 1 4 0 65535 0 0\ndump 0\ngradrect 0 1 -2 -2 0 0 65535 0 9 9 65535 65535 0 0\ndump 0\ngradrect 0 0 2 1 0 0 0 0 2 3 65535 65535 65535 0\ndump 0\nend").unwrap();
    // the edges: palette sources, a source rectangle outside the source, formats apart
    writeln!(s, "case alpha-palette\ndib 0 4 2 32\nfill 0 1\ndib 1 4 2 8\nfill 1 2\nalphablend 0 0 0 4 2 1 0 0 4 2 128 0\ndump 0\nend").unwrap();
    // palette sources of each depth onto 32 bpp, at several constant alphas (their alpha;
    // fixed fills: the random stream of the cases after stays as it was)
    for bpp in [1, 4, 8] {
        for sca in [0, 1, 127, 254, 255] {
            writeln!(s, "case alpha-palette-{bpp}-{sca}\ndib 0 4 2 32\nfill 0 {}\ndib 1 4 2 {bpp}\nfill 1 {}\nalphablend 0 0 0 4 2 1 0 0 4 2 {sca} 0\ndump 0\nend", 0x8040_2010u32 + sca, 0x0123_4567u32 * bpp).unwrap();
        }
    }
    // AlphaBlend stretched onto 32 bpp (a random stream of its own)
    let mut q = XorShift32::new(0xa1fa_0001);
    for i in 0..24 {
        let sf = [32, 24, 16, 8][i % 4];
        let (sw, sh) = (q.range(1, 7), q.range(1, 6));
        writeln!(s, "case alpha-stretch-{i}\ndib 0 10 8 32\nfill 0 {}\ndib 1 6 5 {sf}\nfill 1 {}", q.next(), q.next()).unwrap();
        writeln!(s, "alphablend 0 {} {} {} {} 1 {} {} {sw} {sh} {} {}\ndump 0\nend", q.range(-1, 4), q.range(-1, 4), q.range(1, 11), q.range(1, 9), q.range(0, 6 - sw + 1), q.range(0, 5 - sh + 1), q.range(0, 256), (sf == 32 && q.chance(2)) as u8).unwrap();
    }
    writeln!(s, "case transparent-outside\ndib 0 4 2 32\nfill 0 1\ndib 1 4 2 32\nfill 1 2\ntransparent 0 0 0 4 2 1 1 0 4 2 #000000\ndump 0\nend").unwrap();
    writeln!(s, "case maskblt-formats\ndib 0 4 2 32\nfill 0 1\ndib 1 4 2 24\nfill 1 2\ndib 2 4 2 1\nfill 2 3\nmaskblt 0 0 0 4 2 1 0 0 2 0 0 0xaacc0020\ndump 0\nend").unwrap();
    // MaskBlt: random quaternary operations, monochrome masks
    for i in 0..80 {
        writeln!(s, "case maskblt-{i}").unwrap();
        let df = *r.pick(&[(32, ""), (24, ""), (8, ""), (16, "")]);
        let (w, h) = (r.range(1, 10), r.range(1, 8));
        let (l, _) = dib_line(&mut r, 0, w, h, &df, false);
        writeln!(s, "{l}\nfill 0 {}", r.next()).unwrap();
        let (l, _) = dib_line(&mut r, 1, w, h, &df, false);
        writeln!(s, "{l}\nfill 1 {}", r.next()).unwrap();
        writeln!(s, "brush 0 solid #{:06x}", r.next() & 0xffffff).unwrap();
        let mask = if r.chance(5) { "none".to_string() } else if r.chance(2) { "2".into() } else { "3".into() };
        if mask == "2" {
            writeln!(s, "ddb 2 mono {} {}\nfill 2 {}", w + 2, h + 2, r.next()).unwrap();
        } else if mask == "3" {
            writeln!(s, "dib 3 {} {} 1\nfill 3 {}", w + 2, h + 2, r.next()).unwrap();
        }
        let rop4 = (r.next() & 0xff) << 24 | (r.next() & 0xff) << 16 | (r.next() & 0xffff);
        let rop4 = if r.chance(3) { 0xAACC0020 } else { rop4 };
        writeln!(s, "maskblt 0 {} {} {} {} 1 {} {} {mask} {} {} {rop4:#x}\ndump 0\nend", r.range(-1, 2), r.range(-1, 2), r.range(0, w + 1), r.range(0, h + 1), r.range(0, 2), r.range(0, 2), r.range(0, 2), r.range(0, 2)).unwrap();
    }
    // PlgBlt: rotations, mirrors, shears, scales
    for i in 0..60 {
        writeln!(s, "case plgblt-{i}\ndib 0 16 16 32\nfill 0 {}\ndib 1 6 5 32\nfill 1 {}", r.next(), r.next()).unwrap();
        let (x0, y0) = (r.range(0, 15), r.range(0, 15));
        let (ax, ay, bx, by) = match i % 4 {
            0 => (6, 0, 0, 5),
            1 => (0, 6, -5, 0),
            2 => (-6, 0, 0, -5),
            _ => (r.range(-8, 8), r.range(-8, 8), r.range(-8, 8), r.range(-8, 8)),
        };
        writeln!(s, "plgblt 0 {x0} {y0} {} {} {} {} 1 0 0 6 5", x0 + ax, y0 + ay, x0 + bx, y0 + by).unwrap();
        writeln!(s, "dump 0\nend").unwrap();
    }
    // PlgBlt: more shears and turns, scaled (a random stream of their own); a source of 8 by 8
    // distinct pixels
    let mut q = XorShift32::new(0x9161_0001);
    for i in 0..80 {
        writeln!(s, "case plgblt-more-{i}\ndib 0 24 24 32\nfill 0 {}\ndib 1 8 8 32\nfill 1 {}", q.next(), q.next()).unwrap();
        let (x0, y0) = (q.range(2, 22), q.range(2, 22));
        let (sw, sh) = (q.range(1, 9), q.range(1, 9));
        let (ax, ay, bx, by) = match i % 4 {
            0 => (0, q.range(-12, 13), q.range(-12, 13), 0),
            1 => (q.range(-12, 13), 0, 0, q.range(-12, 13)),
            _ => (q.range(-12, 13), q.range(-12, 13), q.range(-12, 13), q.range(-12, 13)),
        };
        writeln!(s, "plgblt 0 {x0} {y0} {} {} {} {} 1 {} {} {sw} {sh}\ndump 0\nend", x0 + ax, y0 + ay, x0 + bx, y0 + by, q.range(0, 9 - sw), q.range(0, 9 - sh)).unwrap();
    }
    // MaskBlt's rectangles: negative and 0 extents, the destination's edges, the mask's and the
    // source's bounds
    for i in 0..80 {
        writeln!(s, "case maskblt-bounds-{i}\ndib 0 6 5 32\nfill 0 {}\ndib 1 6 5 32\nfill 1 {}\nbrush 0 solid #{:06x}", r.next(), r.next(), r.next() & 0xffffff).unwrap();
        if r.chance(2) {
            writeln!(s, "ddb 2 mono 6 5\nfill 2 {}", r.next()).unwrap();
        } else {
            writeln!(s, "dib 2 6 5 1\nfill 2 {}", r.next()).unwrap();
        }
        let (x, y, w, h) = (r.range(-3, 6), r.range(-3, 5), r.range(-2, 8), r.range(-2, 7));
        writeln!(s, "maskblt 0 {x} {y} {w} {h} 1 {} {} 2 {} {} 0xaacc0020\ndump 0\nend", r.range(-2, 3), r.range(-2, 3), r.range(-2, 3), r.range(-2, 3)).unwrap();
    }
    // TransparentBlt's extents and source rectangle: 0, negative, partly outside the source
    for i in 0..60 {
        writeln!(s, "case transparent-ext-{i}\ndib 0 6 5 32\nfill 0 {}\ndib 1 5 4 32\nfill 1 {}", r.next(), r.next()).unwrap();
        let e = |r: &mut XorShift32, n: i64| if r.chance(3) { r.range(-1, 0) } else { r.range(1, n) };
        let (w, h) = (e(&mut r, 6), e(&mut r, 5));
        let (sw, sh) = if r.chance(2) { (w, h) } else { (e(&mut r, 5), e(&mut r, 4)) };
        writeln!(s, "transparent 0 {} {} {w} {h} 1 {} {} {sw} {sh} #000000\ndump 0\nend", r.range(-1, 3), r.range(-1, 3), r.range(-1, 2), r.range(-1, 2)).unwrap();
    }
    s
}
