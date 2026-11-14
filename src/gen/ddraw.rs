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
