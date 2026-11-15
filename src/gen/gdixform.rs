//! The xform suite: mapping modes, window / viewport extents and origins, the world transform,
//! LPtoDP / DPtoLP, and drawing through them.
use super::*;

/// a float with few bits (exact in decimal and in f32)
fn fl(r: &mut XorShift32, lo: i64, hi: i64) -> String {
    let v = r.range(lo * 8, hi * 8) as f64 / 8.0;
    format!("{v}")
}

/// 64 points as two calls of 32 (a case line holds at most 80 tokens, the runner 32 points)
fn grid(op: &str) -> String {
    let mut p = Vec::new();
    for y in [-7, -1, 0, 1, 2, 3, 5, 13] {
        for x in [-9, -2, -1, 0, 1, 4, 7, 100] {
            p.push(format!("{x} {y}"));
        }
    }
    format!("{op} 0 {}\n{op} 0 {}", p[..32].join(" "), p[32..].join(" "))
}

pub fn xform() -> String {
    let mut s = String::from("# kikizake suite xform: GDI mapping modes and world transforms (SPEC.md). Made by `kikizake gen xform`.\n");
    let mut r = XorShift32::new(0x7f0e_0000);
    // the defaults, and each mapping mode's extents
    writeln!(s, "case defaults\ndib 0 8 8 32\ngetmap 0\ngetxform 0\n{}\nend", grid("lptodp")).unwrap();
    for m in 0..=9 {
        writeln!(s, "case mapmode-{m}\ndib 0 8 8 32\nmapmode 0 {m}\ngetmap 0\nwinext 0 10 20\nvpext 0 30 -40\ngetmap 0\nmapmode 0 1\ngetmap 0\nend").unwrap();
    }
    // MM_ANISOTROPIC: the rounding of LPtoDP and DPtoLP over extents and origins
    for i in 0..120 {
        writeln!(s, "case aniso-{i}\ndib 0 8 8 32\nmapmode 0 8").unwrap();
        let ext = |r: &mut XorShift32| if r.chance(6) { r.range(-20, 20) } else { *r.pick(&[1, 2, 3, 5, 7, -1, -3, 10, 100, 1000, 65536]) };
        writeln!(s, "winext 0 {} {}\nvpext 0 {} {}", ext(&mut r), ext(&mut r), ext(&mut r), ext(&mut r)).unwrap();
        if r.chance(2) {
            writeln!(s, "winorg 0 {} {}\nvporg 0 {} {}", r.range(-9, 9), r.range(-9, 9), r.range(-9, 9), r.range(-9, 9)).unwrap();
        }
        writeln!(s, "getmap 0\n{}\n{}\nend", grid("lptodp"), grid("dptolp")).unwrap();
    }
    // MM_ISOTROPIC: the viewport adjusted to keep the aspect
    for i in 0..60 {
        writeln!(s, "case iso-{i}\ndib 0 8 8 32\nmapmode 0 7").unwrap();
        let mut ext = || { let v = r.range(1, 300); if r.chance(4) { -v } else { v } };
        let (a, b, c, d) = (ext(), ext(), ext(), ext());
        if i % 2 == 0 {
            writeln!(s, "winext 0 {a} {b}\nvpext 0 {c} {d}").unwrap();
        } else {
            writeln!(s, "vpext 0 {c} {d}\nwinext 0 {a} {b}").unwrap();
        }
        writeln!(s, "getmap 0\n{}\nend", grid("lptodp")).unwrap();
    }
    // offsets and scales
    for i in 0..40 {
        writeln!(s, "case scale-{i}\ndib 0 8 8 32\nmapmode 0 8\nwinext 0 {} {}\nvpext 0 {} {}", r.range(1, 50), r.range(1, 50), r.range(1, 50), r.range(1, 50)).unwrap();
        writeln!(s, "scalewin 0 {} {} {} {}\nscalevp 0 {} {} {} {}", r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5)).unwrap();
        writeln!(s, "offsetwin 0 {} {}\noffsetvp 0 {} {}\ngetmap 0\n{}\nend", r.range(-5, 5), r.range(-5, 5), r.range(-5, 5), r.range(-5, 5), grid("lptodp")).unwrap();
    }
    // world transforms: GM_ADVANCED, set / modify / get, CombineTransform
    for i in 0..80 {
        writeln!(s, "case world-{i}\ndib 0 8 8 32").unwrap();
        if i % 8 != 0 {
            writeln!(s, "gmode 0 2").unwrap();
        }
        let x = |r: &mut XorShift32| format!("{} {} {} {} {} {}", fl(r, -3, 3), fl(r, -3, 3), fl(r, -3, 3), fl(r, -3, 3), fl(r, -20, 20), fl(r, -20, 20));
        writeln!(s, "setxform 0 {}\ngetxform 0", x(&mut r)).unwrap();
        if r.chance(2) {
            writeln!(s, "modxform 0 {} {}\ngetxform 0", r.range(1, 3), x(&mut r)).unwrap();
        }
        if r.chance(3) {
            writeln!(s, "mapmode 0 8\nwinext 0 {} {}\nvpext 0 {} {}", r.range(1, 9), r.range(1, 9), r.range(1, 9), r.range(1, 9)).unwrap();
        }
        writeln!(s, "{}\n{}\ngmode 0 1\ngetxform 0\nend", grid("lptodp"), grid("dptolp")).unwrap();
    }
    for i in 0..60 {
        let x = |r: &mut XorShift32| format!("{} {} {} {} {} {}", fl(r, -4, 4), fl(r, -4, 4), fl(r, -4, 4), fl(r, -4, 4), fl(r, -99, 99), fl(r, -99, 99));
        writeln!(s, "case combinexform-{i}\ncombinexform {} {}\nend", x(&mut r), x(&mut r)).unwrap();
    }
    // drawing through a mapping: PatBlt, Rectangle and lines, scaled, mirrored, rotated
    for i in 0..60 {
        writeln!(s, "case draw-{i}\ndib 0 24 24 8\npen 0 0 1 #ffffff\nbrush 0 solid #ff0000").unwrap();
        match i % 3 {
            0 => writeln!(s, "mapmode 0 8\nwinext 0 {} {}\nvpext 0 {} {}\nvporg 0 {} {}", r.range(1, 4), r.range(1, 4), *r.pick(&[1, 2, 3, -1, -2]), *r.pick(&[1, 2, 3, -1, -2]), r.range(0, 24), r.range(0, 24)).unwrap(),
            1 => writeln!(s, "gmode 0 2\nsetxform 0 {} {} {} {} {} {}", *r.pick(&["1", "2", "0", "-1", "0.5", "1.5"]), *r.pick(&["0", "1", "-1", "0.5"]), *r.pick(&["0", "1", "-1", "0.5"]), *r.pick(&["1", "2", "0", "-1", "0.5", "1.5"]), r.range(0, 24), r.range(0, 24)).unwrap(),
            _ => writeln!(s, "winorg 0 {} {}\nvporg 0 {} {}", r.range(-5, 5), r.range(-5, 5), r.range(-5, 5), r.range(-5, 5)).unwrap(),
        }
        let (a, b) = (r.range(-6, 10), r.range(-6, 10));
        match r.range(0, 2) {
            0 => writeln!(s, "patblt 0 {a} {b} {} {} 0xf00021", r.range(-3, 8), r.range(-3, 8)).unwrap(),
            1 => writeln!(s, "rectangle 0 {a} {b} {} {}", a + r.range(-3, 8), b + r.range(-3, 8)).unwrap(),
            _ => writeln!(s, "moveto 0 {a} {b}\nlineto 0 {} {}", r.range(-6, 12), r.range(-6, 12)).unwrap(),
        }
        writeln!(s, "dump 0\nend").unwrap();
    }
    // more MM_ISOTROPIC mappings, origins too (their own stream)
    let mut r = XorShift32::new(0x150_7e0d);
    for i in 0..40 {
        writeln!(s, "case isomore-{i}\ndib 0 8 8 32\nmapmode 0 7\nvpext 0 {} {}\nwinext 0 {} {}\nwinorg 0 {} {}\nvporg 0 {} {}\ngetmap 0\n{}\n{}\nend", r.range(-300, 300), r.range(-300, 300), r.range(-300, 300), r.range(-300, 300), r.range(-9, 9), r.range(-9, 9), r.range(-9, 9), r.range(-9, 9), grid("lptodp"), grid("dptolp")).unwrap();
    }
    // ScaleWindowExtEx / ScaleViewportExtEx in MM_TEXT, a metric mode and MM_ISOTROPIC (their
    // own stream)
    let mut r = XorShift32::new(0x5ca1_e0de);
    for i in 0..24 {
        let mode = [1, 2, 7][i % 3];
        writeln!(s, "case scalemode-{i}\ndib 0 8 8 32\nmapmode 0 {mode}\nwinext 0 {} {}\nvpext 0 {} {}\ngetmap 0", r.range(1, 50), r.range(1, 50), r.range(1, 50), r.range(1, 50)).unwrap();
        writeln!(s, "scalewin 0 {} {} {} {}\ngetmap 0\nscalevp 0 {} {} {} {}\ngetmap 0\n{}\nend", r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), grid("lptodp")).unwrap();
    }
    // world transforms with window / viewport origins, a mapping too (their own stream)
    let mut r = XorShift32::new(0x0e1d_0c1a);
    for i in 0..40 {
        writeln!(s, "case worldorg-{i}\ndib 0 8 8 32\ngmode 0 2").unwrap();
        writeln!(s, "setxform 0 {} {} {} {} {} {}", fl(&mut r, -3, 3), fl(&mut r, -3, 3), fl(&mut r, -3, 3), fl(&mut r, -3, 3), fl(&mut r, -20, 20), fl(&mut r, -20, 20)).unwrap();
        if i % 2 == 1 {
            writeln!(s, "mapmode 0 8\nwinext 0 {} {}\nvpext 0 {} {}", r.range(1, 9), r.range(1, 9), *r.pick(&[1, 2, 3, 5, 7, -1, -3]), *r.pick(&[1, 2, 3, 5, 7, -1, -3])).unwrap();
        }
        writeln!(s, "winorg 0 {} {}\nvporg 0 {} {}\ngetmap 0\n{}\n{}\nend", r.range(-9, 9), r.range(-9, 9), r.range(-9, 9), r.range(-9, 9), grid("lptodp"), grid("dptolp")).unwrap();
    }
    // GM_COMPATIBLE asked over a world transform: is the mode still GM_ADVANCED afterwards
    for i in 0..4 {
        let w = ["2 0 0 1 3 -2", "1 0.5 0 1 0 0", "1 0 0 1 0.5 0.25", "-1 0 0 1 0 0"][i];
        writeln!(s, "case gmodeback-{i}\ndib 0 8 8 32\ngmode 0 2\nsetxform 0 {w}\ngmode 0 1\ngmode 0 2\ngmode 0 1\ngetxform 0\n{}\nmodxform 0 1 1 0 0 1 0 0\ngmode 0 1\ngmode 0 1\nend", grid("lptodp")).unwrap();
    }
    // lines whose ends fall between pixels: a 1/16 window over whole viewport units, so each
    // logical unit is 1/16 pixel (28.4 exactly), and a few coarser fractions (their own stream)
    let mut r = XorShift32::new(0x0f7a_c11e);
    for i in 0..48 {
        writeln!(s, "case fracline-{i}\ndib 0 24 24 8\npen 0 0 1 #ffffff\nmapmode 0 8").unwrap();
        let (we, ve) = [((16, 16), (1, 1)), ((16, 16), (-1, 1)), ((16, 16), (1, -1)), ((8, 4), (1, 1)), ((4, 2), (1, 1)), ((3, 5), (1, 1))][i % 6];
        writeln!(s, "winext 0 {} {}\nvpext 0 {} {}\nvporg 0 12 12", we.0, we.1, ve.0, ve.1).unwrap();
        let mut p = || (r.range(-10 * we.0 as i64, 10 * we.0 as i64), r.range(-10 * we.1 as i64, 10 * we.1 as i64));
        let (a, b, c) = (p(), p(), p());
        writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, b.0, b.1, c.0, c.1).unwrap();
    }
    // lines through a GM_ADVANCED world transform: 1/16 scales (ends in 28.4) with a cosmetic
    // pen, and whole scales with a pen of width 1 (does its width scale?); lines that leave the
    // surface with their ends between pixels (their own stream)
    let mut r = XorShift32::new(0x0adf_11e5);
    for i in 0..48 {
        writeln!(s, "case fraclineadv-{i}\ndib 0 24 24 8\npen 0 0 {} #ffffff\ngmode 0 2", if i < 32 { 0 } else { 1 }).unwrap();
        if i < 32 {
            let (sx, sy) = [(1, 1), (-1, 1), (1, -1), (-1, -1)][i % 4];
            let sh = if i % 8 < 4 { "0 0" } else { "0.03125 -0.015625" };
            writeln!(s, "setxform 0 {} {sh} {} 12 12", f64::from(sx) / 16.0, f64::from(sy) / 16.0).unwrap();
            let mut p = || (r.range(-160, 160), r.range(-160, 160));
            let (a, b, c) = (p(), p(), p());
            writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, b.0, b.1, c.0, c.1).unwrap();
        } else {
            let w = ["2 0 0 2 3 3", "1.5 0 0 1.5 2 2", "3 0 0 1 1 1", "1 0 0 3 1 1", "2 1 0 1 0 0", "1 0 0 0.5 4 4", "0.5 0 0 0.5 9 9", "1 1 -1 1 12 0"][i % 8];
            writeln!(s, "setxform 0 {w}").unwrap();
            let mut p = || (r.range(0, 9), r.range(0, 9));
            let (a, b) = (p(), p());
            writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, b.0, b.1).unwrap();
        }
    }
    for i in 0..24 {
        writeln!(s, "case fracclip-{i}\ndib 0 24 24 8\npen 0 0 1 #ffffff\nmapmode 0 8\nwinext 0 {} {}\nvpext 0 {} 1\nvporg 0 12 12", [16, 4, 1][i % 3], [16, 4, 4][i % 3], [1, 1, -2][i % 3]).unwrap();
        let k = [16, 4, 1][i % 3];
        let (a, b) = ((r.range(-30 * k, 30 * k), r.range(-10 * k, 10 * k)), (r.range(-10 * k, 10 * k), r.range(-10 * k, 10 * k)));
        writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, b.0, b.1).unwrap();
    }
    // 28.4 diagonals along diamond edges (x - y or x + y half a pixel off), through a 1/16
    // GM_ADVANCED world, cosmetic pen (their own stream)
    let mut r = XorShift32::new(0x0d1a_9011);
    for i in 0..24 {
        let (sx, sy) = [(1, 1), (-1, -1), (1, -1), (-1, 1)][i % 4];
        let a = (r.range(-120, 120), r.range(-120, 120));
        // (the start's x - y, or x + y for the other diagonal, is 8 mod 16)
        let a = if sx == sy { (a.0, a.0 - 8 - 16 * r.range(-7, 7)) } else { (a.0, -a.0 + 8 + 16 * r.range(-7, 7)) };
        let k = r.range(4, 60);
        writeln!(s, "case diagline-{i}\ndib 0 24 24 8\npen 0 0 0 #ffffff\ngmode 0 2\nsetxform 0 0.0625 0 0 0.0625 12 12\nmoveto 0 {} {}\nlineto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, a.0 + sx * k, a.1 + sy * k, a.0 + sx * k + 20, a.1 + sy * k - 9).unwrap();
    }
    // GM_COMPATIBLE lines with ends between pixels through a mapping that mirrors and
    // magnifies an axis, both ends on the surface, both ways (their own stream)
    let mut r = XorShift32::new(0x0f1a_e3a2);
    for i in 0..48 {
        // (the mirrored magnifying axis, the other one shrinking to fractions; the last pair
        // mirrors and magnifies both by 3/2)
        let ((wx, vx), (wy, vy)): ((i64, i64), (i64, i64)) = [((1, -2), (4, 1)), ((1, -3), (3, 2)), ((2, -3), (4, 1)), ((4, -5), (5, 2)), ((4, 1), (1, -2)), ((3, 2), (1, -3)), ((4, 1), (2, -3)), ((2, -3), (2, -3))][i % 8];
        writeln!(s, "case fracmirror-{i}\ndib 0 24 24 8\npen 0 0 1 #ffffff\nmapmode 0 8\nwinext 0 {wx} {wy}\nvpext 0 {vx} {vy}\nvporg 0 12 12").unwrap();
        // (logical ranges keeping the device point within 10 pixels of the origin)
        let (kx, ky) = (10 * wx / vx.abs(), 10 * wy / vy.abs());
        let mut p = || (r.range(-kx, kx), r.range(-ky, ky));
        let (a, b) = (p(), p());
        writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\ndump 0\nend", a.0, a.1, b.0, b.1).unwrap();
    }
    s
}
