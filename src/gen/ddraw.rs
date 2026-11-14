//! The ddraw suites' case generators.
use super::*;

pub fn ddraw() -> String {
    let mut s = String::from("# kikizake suite ddraw: DirectDraw 7 blits between system-memory surfaces (SPEC.md). Made by `kikizake gen ddraw`.\n");
    let fmts = ["8", "555", "565", "32"];
    let bytes = |f: &str| match f {
        "8" => 1,
        "32" => 4,
        _ => 2,
    };
    let mut r = XorShift32::new(0x00dd_7a00);
    // a key value: the pixel at (x, y) of a surface filled from `seed` (so that some match)
    let key = |f: &str, w: i64, seed: u32, x: i64, y: i64| -> u32 {
        let mut g = XorShift32::new(seed);
        let b = bytes(f);
        let mut v = 0;
        for _ in 0..((y * w + x) as u32 + 1) {
            v = 0;
            for k in 0..b {
                v |= (g.next() & 0xff) << (8 * k);
            }
        }
        v
    };
    for f in fmts {
        // copy, fill and the rectangle checks, one by one
        writeln!(s, "case basic-{f}\ndd\nddsurf 0 12 9 {f}\nddsurf 1 7 5 {f}\nddfill 0 11\nddfill 1 12").unwrap();
        for l in [
            "ddblt 0 0 0 7 5 1 all", "ddblt 0 all 0 all", "ddblt 0 5 4 12 9 1 all", "ddblt 0 2 1 5 3 1 3 2 6 4", "ddblt 0 6 4 13 9 1 all", "ddblt 0 -1 0 6 5 1 all",
            "ddblt 0 3 3 3 5 1 all", "ddblt 0 0 0 2 2 1 6 4 8 6", "ddblt 0 0 0 7 5 1 0 0 7 5 mirrorlr", "ddblt 0 1 2 8 7 1 all mirrorud", "ddblt 0 2 2 9 7 1 all mirrorlr mirrorud",
        ] {
            writeln!(s, "{l}").unwrap();
        }
        let m: u64 = if f == "32" { 0xffff_ffff } else { (1u64 << (8 * bytes(f))) - 1 };
        writeln!(s, "ddcolor 0 1 1 4 3 {:#x}\nddcolor 0 all {:#x}\nddcolor 0 10 0 13 2 5\nddblt 0 4 4 11 9 1 all\ndddump 0\nend", 0x1234_5678 & m, 0x9abc_def0u64 & m).unwrap();
        // the color keys: each kind, with a key that occurs in the source
        let k = key(f, 7, 12, 2, 1);
        let kd = key(f, 12, 11, 3, 3);
        for (name, setup, l) in [
            ("keysrc", format!("ddkey 1 src {k:#x}"), "ddblt 0 2 2 9 7 1 all keysrc"),
            ("keydest", format!("ddkey 0 dest {kd:#x}"), "ddblt 0 0 0 7 5 1 all keydest"),
            ("keysrcov", String::new(), &*format!("ddblt 0 3 1 10 6 1 all keysrcov {k:#x}")),
            ("keydestov", String::new(), &*format!("ddblt 0 0 2 7 7 1 all keydestov {kd:#x}")),
            ("keyboth", format!("ddkey 1 src {k:#x}\nddkey 0 dest {kd:#x}"), "ddblt 0 0 0 7 5 1 all keysrc keydest"),
            ("fast", String::new(), "ddfast 0 3 2 1 all\nddfast 0 8 6 1 all\nddfast 0 0 0 1 1 1 5 4"),
            ("fastsrckey", format!("ddkey 1 src {k:#x}"), "ddfast 0 1 1 1 all srckey\nddfast 0 5 4 1 0 0 7 5 srckey"),
            ("fastdestkey", format!("ddkey 0 dest {kd:#x}"), "ddfast 0 1 1 1 all destkey"),
            ("keynone", format!("ddkey 1 src {k:#x}\nddkey 1 src none\nddkey 1 src {k:#x}"), "ddblt 0 2 2 9 7 1 all keysrc"),
        ] {
            writeln!(s, "case {name}-{f}\ndd\nddsurf 0 12 9 {f}\nddsurf 1 7 5 {f}\nddfill 0 11\nddfill 1 12\n{setup}\n{l}\ndddump 0\nend").unwrap();
        }
        // within one surface, every direction of overlap
        writeln!(s, "case overlap-{f}\ndd\nddsurf 0 16 12 {f}\nddfill 0 21").unwrap();
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1), (2, 3), (-3, -2), (3, -1), (-2, 2)] {
            let (x, y) = (4 + dx, 4 + dy);
            writeln!(s, "ddblt 0 {x} {y} {} {} 0 4 4 12 8\nddfast 0 {} {} 0 5 5 11 9", x + 8, y + 4, 5 - dx, 5 - dy).unwrap();
        }
        writeln!(s, "dddump 0\nend").unwrap();
    }
    // random blits: rectangles at and past the edges, keys from the source, mirroring
    for i in 0..300 {
        let f = *r.pick(&fmts);
        let (dw, dh, sw, sh) = (r.range(1, 20), r.range(1, 14), r.range(1, 14), r.range(1, 10));
        let (ds, ss) = (r.next() | 1, r.next() | 1);
        writeln!(s, "case random-{i}\ndd\nddsurf 0 {dw} {dh} {f}\nddsurf 1 {sw} {sh} {f}\nddfill 0 {ds}\nddfill 1 {ss}").unwrap();
        let k = key(f, sw, ss, r.range(0, sw - 1), r.range(0, sh - 1));
        let kd = key(f, dw, ds, r.range(0, dw - 1), r.range(0, dh - 1));
        if r.chance(2) {
            writeln!(s, "ddkey 1 src {k:#x}").unwrap();
        }
        if r.chance(3) {
            writeln!(s, "ddkey 0 dest {kd:#x}").unwrap();
        }
        let src_key = s.lines().rev().take(3).any(|l| l.starts_with("ddkey 1 src"));
        let dest_key = s.lines().rev().take(3).any(|l| l.starts_with("ddkey 0 dest"));
        for _ in 0..r.range(1, 6) {
            let (w, h) = (r.range(0, sw.min(dw) + 1), r.range(0, sh.min(dh) + 1));
            let (sx, sy) = (r.range(-1, sw - w + 1), r.range(-1, sh - h + 1));
            let (dx, dy) = (r.range(-1, dw - w + 1), r.range(-1, dh - h + 1));
            let mut l = if r.chance(3) {
                format!("ddfast 0 {dx} {dy} 1 {sx} {sy} {} {}", sx + w, sy + h)
            } else {
                format!("ddblt 0 {dx} {dy} {} {} 1 {sx} {sy} {} {}", dx + w, dy + h, sx + w, sy + h)
            };
            let fast = l.starts_with("ddfast");
            match r.range(0, 7) {
                0 if src_key => l += if fast { " srckey" } else { " keysrc" },
                1 if dest_key => l += if fast { " destkey" } else { " keydest" },
                2 if !fast => l += &format!(" keysrcov {k:#x}"),
                3 if !fast => l += &format!(" keydestov {kd:#x}"),
                4 if !fast => l += *r.pick(&[" mirrorlr", " mirrorud", " mirrorlr mirrorud"]),
                _ => {}
            }
            writeln!(s, "{l}").unwrap();
        }
        writeln!(s, "dddump 0\nend").unwrap();
    }
    // what the documentation leaves open
    for (name, body) in [
        ("stretch", "ddsurf 0 12 9 32\nddsurf 1 5 3 32\nddfill 0 1\nddfill 1 2\nddblt 0 0 0 11 7 1 all\nddblt 0 0 0 2 2 1 all"),
        ("formats", "ddsurf 0 12 9 565\nddsurf 1 5 3 32\nddfill 0 1\nddfill 1 2\nddblt 0 0 0 5 3 1 all"),
        ("key-unset", "ddsurf 0 12 9 32\nddsurf 1 5 3 32\nddfill 0 1\nddfill 1 2\nddblt 0 0 0 5 3 1 all keysrc\nddfast 0 0 0 1 all srckey"),
        ("fill-highbits", "ddsurf 0 12 9 565\nddfill 0 1\nddcolor 0 all 0x12345"),
        ("fast-bothkeys", "ddsurf 0 12 9 32\nddsurf 1 5 3 32\nddfill 0 1\nddfill 1 2\nddkey 1 src 0\nddkey 0 dest 0\nddfast 0 0 0 1 all srckey destkey"),
        ("nosource", "ddsurf 0 12 9 32\nddfill 0 1\nddblt 0 all - all"),
    ] {
        writeln!(s, "case edge-{name}\ndd\n{body}\ndddump 0\nend").unwrap();
    }
    writeln!(s, "case nodev\nddsurf 0 4 4 32\nend").unwrap();
    s
}

/// a surface's format token and its bytes a pixel
const FMTS2: &[(&str, i64)] = &[("8", 1), ("555", 2), ("565", 2), ("24", 3), ("32", 4), ("a32", 4)];

/// clip lists: blits, fills and BltFast on a surface with a clipper
pub fn ddclip() -> String {
    let mut s = String::from("# kikizake suite ddclip: DirectDraw 7 clippers with clip lists on system-memory surfaces (SPEC.md). Made by `kikizake gen ddclip`.\n");
    let mut r = XorShift32::new(0x00dd_c11b);
    for f in ["8", "565", "32"] {
        let head = format!("dd\nddsurf 0 16 12 {f}\nddsurf 1 16 12 {f}\nddfill 0 3\nddfill 1 4");
        for (name, body) in [
            ("list", "ddclip 0 2 1 9 6 4 5 14 11\nddcliplist 0\nddblt 0 all 1 all"),
            ("one", "ddclip 0 3 2 10 9\nddblt 0 1 1 13 11 1 0 0 12 10"),
            ("past", "ddclip 0 0 0 16 12\nddblt 0 -3 -2 10 8 1 0 0 13 10\nddblt 0 10 8 19 15 1 0 0 9 7"),
            ("pastclip", "ddclip 0 -4 -4 20 20\nddcliplist 0\nddblt 0 -3 -2 10 8 1 0 0 13 10"),
            ("outside", "ddclip 0 20 20 30 30\nddcliplist 0\nddblt 0 all 1 all"),
            ("srcpast", "ddclip 0 0 0 16 12\nddblt 0 0 0 8 6 1 10 8 18 14"),
            ("fill", "ddclip 0 1 1 5 5 8 2 15 4\nddcolor 0 all 0x55\nddcolor 0 3 3 12 10 0x2a"),
            ("mirror", "ddclip 0 0 0 8 12\nddblt 0 2 1 14 9 1 0 0 12 8 mirrorlr\nddblt 0 0 0 16 12 1 all mirrorud"),
            ("stretch", "ddclip 0 0 0 8 6\nddblt 0 0 0 16 12 1 0 0 8 6"),
            ("fast", "ddclip 0 2 2 6 6\nddfast 0 0 0 1 0 0 8 8"),
            ("empty", "ddclip 0 empty\nddcliplist 0\nddblt 0 all 1 all\nddcolor 0 all 7"),
            ("none", "ddclip 0 2 2 6 6\nddclip 0 none\nddcliplist 0\nddblt 0 -1 0 5 4 1 0 0 6 4\nddblt 0 all 1 all"),
            ("srcclip", "ddclip 1 0 0 2 2\nddblt 0 all 1 all"),
            ("keysrc", "ddkey 1 src 0\nddclip 0 4 0 12 12\nddblt 0 all 1 all keysrc"),
            ("overlap", "ddclip 0 0 0 16 6\nddblt 0 1 1 13 11 0 0 0 12 10"),
            ("many", "ddclip 0 0 0 2 2 2 2 4 4 4 4 6 6 6 6 8 8 8 8 10 10 10 10 12 12 12 0 14 2 1 9 3 11\nddcliplist 0\nddblt 0 all 1 all"),
            ("overlapping", "ddclip 0 0 0 8 8 4 4 12 12\nddcliplist 0\nddblt 0 all 1 all"),
        ] {
            writeln!(s, "case {name}-{f}\n{head}\n{body}\ndddump 0\nend").unwrap();
        }
    }
    // random clip lists (inside, across and past the surface) and blits
    for i in 0..200 {
        let f = *r.pick(&["8", "555", "565", "24", "32"]);
        let (w, h) = (r.range(1, 20), r.range(1, 14));
        let (sw, sh) = (r.range(1, 14), r.range(1, 10));
        writeln!(s, "case random-{i}\ndd\nddsurf 0 {w} {h} {f}\nddsurf 1 {sw} {sh} {f}\nddfill 0 {}\nddfill 1 {}", r.next() | 1, r.next() | 1).unwrap();
        let n = r.range(1, 4);
        let mut l = String::from("ddclip 0");
        for _ in 0..n {
            let (x0, y0) = (r.range(-2, w), r.range(-2, h));
            let (x1, y1) = (r.range(x0 + 1, w + 3), r.range(y0 + 1, h + 3));
            write!(l, " {x0} {y0} {x1} {y1}").unwrap();
        }
        writeln!(s, "{l}").unwrap();
        for _ in 0..r.range(1, 4) {
            let (bw, bh) = (r.range(1, sw), r.range(1, sh));
            let (sx, sy) = (r.range(0, sw - bw), r.range(0, sh - bh));
            let (dx, dy) = (r.range(-3, w), r.range(-3, h));
            let l = match r.range(0, 5) {
                0 => format!("ddcolor 0 {dx} {dy} {} {} {:#x}", dx + bw, dy + bh, r.next() & 0xff),
                1 => format!("ddblt 0 {dx} {dy} {} {} 1 {sx} {sy} {} {} {}", dx + bw, dy + bh, sx + bw, sy + bh, r.pick(&["mirrorlr", "mirrorud"])),
                _ => format!("ddblt 0 {dx} {dy} {} {} 1 {sx} {sy} {} {}", dx + bw, dy + bh, sx + bw, sy + bh),
            };
            writeln!(s, "{l}").unwrap();
        }
        writeln!(s, "dddump 0\nend").unwrap();
    }
    s
}

/// palettes: CreatePalette's flags, entries, palettes on surfaces
pub fn ddpal() -> String {
    let mut s = String::from("# kikizake suite ddpal: DirectDraw palettes (SPEC.md). Made by `kikizake gen ddpal`.\n");
    let sizes = ["", "1bit", "2bit", "4bit", "8bit", "1bit 8bit", "2bit 4bit"];
    let mut k = 0;
    for size in sizes {
        for extra in ["", "allow256", "8bitentries", "initialize", "primary", "alpha", "allow256 8bitentries"] {
            let flags = format!("{size} {extra}");
            writeln!(s, "case create-{k}\ndd\nddpal 0 {} {}\nddpalcaps 0\nddpalget 0 0 256\nddpalget 0 0 16\nend", k + 1, flags.trim()).unwrap();
            k += 1;
        }
    }
    // entries: ranges at and past each size's end
    for (size, n) in [("1bit", 2), ("2bit", 4), ("4bit", 16), ("8bit", 256)] {
        for (st, cnt) in [(0, n), (0, 1), (n - 1, 1), (n - 1, 2), (n, 1), (0, (n + 1).min(256)), (1, n - 1), (0, 0), (n / 2, n / 2)] {
            writeln!(s, "case entries-{size}-{st}-{cnt}\ndd\nddpal 0 7 {size}\nddpalset 0 {st} {cnt} 9\nddpalget 0 {st} {cnt}\nddpalget 0 0 {n}\nend").unwrap();
        }
        writeln!(s, "case entries8-{size}\ndd\nddpal 0 7 {size} 8bitentries\nddpalget 0 0 {n}\nddpalset 0 0 {n} 3\nddpalget 0 0 {n}\nend").unwrap();
    }
    // palettes on surfaces of each format
    for (f, _) in FMTS2 {
        writeln!(s, "case surface-{f}\ndd\nddsurf 0 4 4 {f}\nddgetpal 0\nddpal 1 5 8bit\nddsetpal 0 1\nddgetpal 0\nddpal 2 6 4bit\nddsetpal 0 2\nddgetpal 0\nddsetpal 0 none\nddgetpal 0\nend").unwrap();
    }
    for (name, body) in [
        ("replace", "ddsurf 0 4 4 8\nddpal 0 1 8bit\nddpal 1 2 8bit\nddsetpal 0 0\nddsetpal 0 1\nddgetpal 0\nddsetpal 0 1\nddgetpal 0"),
        ("shared", "ddsurf 0 4 4 8\nddsurf 1 4 4 8\nddpal 0 1 8bit\nddsetpal 0 0\nddsetpal 1 0\nddgetpal 1\nddpalset 0 0 4 2\nddpalget 0 0 4"),
        ("blt", "ddsurf 0 8 4 8\nddsurf 1 8 4 8\nddfill 0 1\nddfill 1 2\nddpal 0 1 8bit\nddpal 1 2 8bit\nddsetpal 0 0\nddsetpal 1 1\nddblt 0 all 1 all\ndddump 0"),
        ("released", "ddsurf 0 4 4 8\nddpal 0 1 8bit\nddsetpal 0 0\nddpal 0 3 8bit\nddgetpal 0"),
        ("nodd", "ddpalget 0 0 4"),
    ] {
        writeln!(s, "case {name}\ndd\n{body}\nend").unwrap();
    }
    writeln!(s, "case nodev\nddpal 0 1 8bit\nend").unwrap();
    s
}

/// GetDC: GDI on surfaces, and what a held DC does to Lock, Blt and GetDC
pub fn dddc() -> String {
    let mut s = String::from("# kikizake suite dddc: GDI through IDirectDrawSurface7::GetDC (SPEC.md). Made by `kikizake gen dddc`.\n");
    let mut r = XorShift32::new(0x00dd_0dc0);
    for (f, _) in FMTS2 {
        // every value of each channel through SetPixel / GetPixel, and FillRect
        writeln!(s, "case bpp-{f}\ndd\nddsurf 0 8 4 {f}\nddgetdc 0\ndcbpp 0\nddreldc 0\nend").unwrap();
        for ch in 0..3 {
            writeln!(s, "case channel-{f}-{ch}\ndd\nddsurf 0 64 4 {f}\nddfill 0 1\nddgetdc 0").unwrap();
            for v in 0..256u32 {
                let c = v << (8 * (2 - ch));
                writeln!(s, "dcpix 0 {} {} #{c:06x}\ndcget 0 {} {}", v % 64, v / 64, v % 64, v / 64).unwrap();
            }
            writeln!(s, "ddreldc 0\ndddump 0\nend").unwrap();
        }
        writeln!(s, "case rects-{f}\ndd\nddsurf 0 16 8 {f}\nddfill 0 2\nddgetdc 0").unwrap();
        for _ in 0..12 {
            let (x0, y0) = (r.range(-2, 15), r.range(-2, 7));
            let (x1, y1) = (r.range(x0 - 1, 18), r.range(y0 - 1, 10));
            writeln!(s, "dcrect 0 {x0} {y0} {x1} {y1} #{:06x}", r.next() & 0xffffff).unwrap();
        }
        writeln!(s, "dcpix 0 -1 0 #ffffff\ndcpix 0 16 0 #ffffff\ndcget 0 0 8\nddreldc 0\ndddump 0\nend").unwrap();
        writeln!(s, "case random-{f}\ndd\nddsurf 0 16 8 {f}\nddfill 0 5\nddgetdc 0").unwrap();
        for _ in 0..64 {
            let (x, y) = (r.range(0, 15), r.range(0, 7));
            writeln!(s, "dcpix 0 {x} {y} #{:06x}\ndcget 0 {x} {y}", r.next() & 0xffffff).unwrap();
        }
        writeln!(s, "ddreldc 0\ndddump 0\nend").unwrap();
    }
    // 8 bpp: GetPixel through the palette, with and without one
    writeln!(s, "case pal8-none\ndd\nddsurf 0 16 16 8\nddfill 0 1\nddgetdc 0").unwrap();
    for i in 0..16 {
        writeln!(s, "dcget 0 {i} {i}").unwrap();
    }
    writeln!(s, "ddreldc 0\nend").unwrap();
    writeln!(s, "case pal8\ndd\nddsurf 0 16 16 8\nddpal 0 3 8bit allow256\nddsetpal 0 0\nddpalget 0 0 256\nddlock 0 all").unwrap();
    for i in 0..16 {
        writeln!(s, "ddpoke 0 0 {i} {}", (0..16).map(|k| format!("{:02x}", i * 16 + k)).collect::<String>()).unwrap();
    }
    writeln!(s, "ddunlock 0 all\nddgetdc 0").unwrap();
    for i in 0..256 {
        writeln!(s, "dcget 0 {} {}", i % 16, i / 16).unwrap();
    }
    writeln!(s, "ddreldc 0\nend").unwrap();
    writeln!(s, "case pal8-set\ndd\nddsurf 0 16 4 8\nddfill 0 1\nddpal 0 3 8bit allow256\nddsetpal 0 0\nddpalget 0 0 256\nddgetdc 0").unwrap();
    for i in 0..64u32 {
        let c = if i < 32 { [0x000000, 0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0x808080][i as usize % 6] } else { r.next() & 0xffffff };
        writeln!(s, "dcpix 0 {} {} #{c:06x}", i % 16, i / 16).unwrap();
    }
    writeln!(s, "ddreldc 0\ndddump 0\nend").unwrap();
    // a held DC and the other calls
    for (name, body) in [
        ("twice", "ddgetdc 0\nddgetdc 0\nddreldc 0\nddreldc 0"),
        ("lock", "ddgetdc 0\nddlock 0 all\nddreldc 0\nddlock 0 all\nddunlock 0 all"),
        ("locked", "ddlock 0 all\nddgetdc 0\nddunlock 0 all\nddgetdc 0\nddreldc 0"),
        ("blt", "ddgetdc 0\nddblt 1 all 0 all\nddblt 0 all 1 all\nddreldc 0\ndddump 1"),
        ("bltsrc", "ddgetdc 1\nddblt 0 all 1 all\nddcolor 1 all 3\nddreldc 1\ndddump 0"),
        ("reldc", "ddreldc 0"),
        ("visible", "ddgetdc 0\ndcpix 0 1 1 #ff00ff\nddreldc 0\ndddump 0\nddgetdc 0\ndcget 0 1 1\nddreldc 0"),
        ("lockpoke", "ddlock 0 all\nddpoke 0 4 1 ffffffff\nddunlock 0 all\nddgetdc 0\ndcget 0 1 1\nddreldc 0"),
        ("dump", "ddgetdc 0\ndddump 0\nddreldc 0"),
    ] {
        writeln!(s, "case dc-{name}\ndd\nddsurf 0 8 4 32\nddsurf 1 8 4 32\nddfill 0 1\nddfill 1 2\n{body}\nend").unwrap();
    }
    s
}

/// Lock: pitches, rectangles, flags, locks held twice, Unlock's rectangle
pub fn ddlock() -> String {
    let mut s = String::from("# kikizake suite ddlock: IDirectDrawSurface7::Lock and Unlock, GetSurfaceDesc (SPEC.md). Made by `kikizake gen ddlock`.\n");
    let mut r = XorShift32::new(0x00dd_10c4);
    // every width 1..=33 of each format: pitch and the description (an exhaustive small domain)
    for (f, _) in FMTS2 {
        writeln!(s, "case pitch-{f}\ndd").unwrap();
        for w in 1..=33 {
            writeln!(s, "ddsurf 0 {w} 2 {f}\nddlock 0 all\nddunlock 0 all\ndddesc 0").unwrap();
        }
        writeln!(s, "end").unwrap();
        writeln!(s, "case vid-{f}\ndd\nddsurf 0 17 3 {f} vid\ndddesc 0\nddlock 0 all\nddunlock 0 all\nend").unwrap();
    }
    for (f, b) in FMTS2 {
        // rectangles inside, at the edges and past them; the offsets of what Lock returns
        writeln!(s, "case rects-{f}\ndd\nddsurf 0 13 9 {f}").unwrap();
        for (x0, y0, x1, y1) in [(0, 0, 13, 9), (1, 2, 5, 7), (12, 8, 13, 9), (0, 0, 1, 1), (3, 3, 3, 5), (4, 4, 2, 6), (-1, 0, 4, 4), (0, -1, 4, 4), (10, 0, 14, 4), (0, 6, 4, 10), (5, 5, 5, 5), (13, 9, 14, 10)] {
            writeln!(s, "ddlock 0 {x0} {y0} {x1} {y1}\nddunlock 0 {x0} {y0} {x1} {y1}").unwrap();
        }
        writeln!(s, "end").unwrap();
        // writing through a rectangle's lock, then reading the whole surface
        writeln!(s, "case poke-{f}\ndd\nddsurf 0 13 9 {f}\nddfill 0 3\nddlock 0 2 3 9 7").unwrap();
        for y in 0..4 {
            writeln!(s, "ddpoke 0 {b} {y} {}", (0..*b * 3).map(|k| format!("{:02x}", (y * 40 + k) & 255)).collect::<String>()).unwrap();
        }
        writeln!(s, "ddunlock 0 2 3 9 7\ndddump 0\ndddump 0 2 3 9 7\nend").unwrap();
    }
    for (name, body) in [
        ("twice", "ddlock 0 all\nddlock 0 all\nddunlock 0 all\nddunlock 0 all"),
        ("tworects", "ddlock 0 0 0 4 4\nddlock 0 4 4 8 8\nddlock 0 2 2 6 6\nddunlock 0 0 0 4 4\nddunlock 0 4 4 8 8\nddunlock 0 2 2 6 6"),
        ("unlockwrong", "ddlock 0 1 1 4 4\nddunlock 0 0 0 4 4\nddunlock 0 all\nddunlock 0 1 1 4 4"),
        ("unlocknone", "ddunlock 0 all\nddunlock 0 none"),
        ("unlockall", "ddlock 0 1 1 4 4\nddunlock 0 all"),
        ("unlockrectall", "ddlock 0 all\nddunlock 0 0 0 8 8"),
        ("flags", "ddlock 0 all readonly\nddunlock 0 all\nddlock 0 all writeonly\nddunlock 0 all\nddlock 0 all nosyslock\nddunlock 0 all\nddlock 0 all nowait\nddunlock 0 all\nddlock 0 all readonly writeonly\nddunlock 0 all"),
        ("readonlywrite", "ddfill 0 1\nddlock 0 all readonly\nddpoke 0 0 0 01020304\nddunlock 0 all\ndddump 0"),
        ("lockedblt", "ddfill 0 1\nddfill 1 2\nddlock 0 all\nddblt 0 all 1 all\nddblt 1 all 0 all\nddcolor 0 all 5\nddunlock 0 all\ndddump 0"),
        ("lockeddesc", "ddlock 0 2 2 4 4\ndddesc 0\nddunlock 0 2 2 4 4"),
        ("nolock", "ddpoke 0 0 0 00"),
    ] {
        writeln!(s, "case {name}\ndd\nddsurf 0 8 8 32\nddsurf 1 8 8 32\n{body}\nend").unwrap();
    }
    // random rectangles on random sizes
    for i in 0..60 {
        let (f, _) = *r.pick(FMTS2);
        let (w, h) = (r.range(1, 40), r.range(1, 12));
        writeln!(s, "case random-{i}\ndd\nddsurf 0 {w} {h} {f}").unwrap();
        for _ in 0..4 {
            let (x0, y0) = (r.range(-1, w), r.range(-1, h));
            let (x1, y1) = (r.range(x0, w + 1), r.range(y0, h + 1));
            writeln!(s, "ddlock 0 {x0} {y0} {x1} {y1}\nddunlock 0 {x0} {y0} {x1} {y1}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    s
}

/// cooperative levels and flipping chains
pub fn ddflip() -> String {
    let mut s = String::from("# kikizake suite ddflip: DirectDraw 7 cooperative levels and flipping chains (SPEC.md). Made by `kikizake gen ddflip`.\n");
    let capss = ["offscreen", "offscreen sysmem", "offscreen vidmem", "texture", "texture sysmem", "3ddevice", "3ddevice sysmem", "offscreen 3ddevice", "offscreen 3ddevice sysmem", "texture 3ddevice", ""];
    // which chains each level makes
    for (lvl, coop) in [("normal", ""), ("exclusive", "ddcoop exclusive\n")] {
        for (k, caps) in capss.iter().enumerate() {
            for backs in [1, 2] {
                writeln!(s, "case chain-{lvl}-{k}-{backs}\ndd\n{coop}ddchain 0 {backs} 8 4 {caps}\ndddesc 0\ndddesc 1\nend").unwrap();
            }
        }
        writeln!(s, "case primary-{lvl}\ndd\n{coop}ddchain 0 0 8 4 primary\nddchain 1 1 8 4 primary\nend").unwrap();
    }
    // the flips: which buffer's pixels each surface shows after each one
    for (lvl, coop) in [("normal", ""), ("exclusive", "ddcoop exclusive\n")] {
        for (k, caps) in ["offscreen 3ddevice sysmem", "texture sysmem", "3ddevice sysmem"].iter().enumerate() {
            for backs in [1, 2, 3] {
                writeln!(s, "case flip-{lvl}-{k}-{backs}\ndd\n{coop}ddchain 0 {backs} 4 2 {caps}").unwrap();
                for i in 0..=backs {
                    writeln!(s, "ddfill {i} {}", 10 + i).unwrap();
                }
                for _ in 0..backs + 1 {
                    writeln!(s, "ddflip 0 - novsync").unwrap();
                    for i in 0..=backs {
                        writeln!(s, "dddump {i}").unwrap();
                    }
                }
                writeln!(s, "ddflip 0 {backs}\ndddump 0\nddflip 1 -\nddflip 0 0\nend").unwrap();
            }
        }
    }
    for (name, body) in [
        ("flags", "ddchain 0 1 4 2 3ddevice sysmem\nddflip 0 - nowait\nddflip 0 - odd\nddflip 0 - even\nddflip 0 - interval2\nddflip 0 - novsync interval2"),
        ("plain", "ddsurf 0 4 2 32\nddflip 0 -"),
        ("coop", "ddcoop exclusive\nddcoop normal\nddcoop exclusive\nddcoop exclusive"),
        ("primaryflip", "ddcoop exclusive\nddchain 0 1 8 4 primary\nddflip 0 - novsync\nddcoop normal\nddflip 0 - novsync"),
    ] {
        writeln!(s, "case {name}\ndd\n{body}\nend").unwrap();
    }
    s
}
