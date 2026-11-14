//! The xform suite: mapping modes, window / viewport extents and origins, the world transform,
//! LPtoDP / DPtoLP, and drawing through them.
use super::*;

/// a float with few bits (exact in decimal and in f32)
fn fl(r: &mut XorShift32, lo: i64, hi: i64) -> String {
    let v = r.range(lo * 8, hi * 8) as f64 / 8.0;
    format!("{v}")
}

fn grid() -> String {
    let mut p = Vec::new();
    for y in [-7, -1, 0, 1, 2, 3, 5, 13] {
        for x in [-9, -2, -1, 0, 1, 4, 7, 100] {
            p.push(format!("{x} {y}"));
        }
    }
    p.join(" ")
}

pub fn xform() -> String {
    let mut s = String::from("# kikizake suite xform: GDI mapping modes and world transforms (SPEC.md). Made by `kikizake gen xform`.\n");
    let mut r = XorShift32::new(0x7f0e_0000);
    // the defaults, and each mapping mode's extents
    writeln!(s, "case defaults\ndib 0 8 8 32\ngetmap 0\ngetxform 0\nlptodp 0 {}\nend", grid()).unwrap();
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
        writeln!(s, "getmap 0\nlptodp 0 {}\ndptolp 0 {}\nend", grid(), grid()).unwrap();
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
        writeln!(s, "getmap 0\nlptodp 0 {}\nend", grid()).unwrap();
    }
    // offsets and scales
    for i in 0..40 {
        writeln!(s, "case scale-{i}\ndib 0 8 8 32\nmapmode 0 8\nwinext 0 {} {}\nvpext 0 {} {}", r.range(1, 50), r.range(1, 50), r.range(1, 50), r.range(1, 50)).unwrap();
        writeln!(s, "scalewin 0 {} {} {} {}\nscalevp 0 {} {} {} {}", r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5), r.range(-3, 5), r.range(-1, 5)).unwrap();
        writeln!(s, "offsetwin 0 {} {}\noffsetvp 0 {} {}\ngetmap 0\nlptodp 0 {}\nend", r.range(-5, 5), r.range(-5, 5), r.range(-5, 5), r.range(-5, 5), grid()).unwrap();
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
        writeln!(s, "lptodp 0 {}\ndptolp 0 {}\ngmode 0 1\ngetxform 0\nend", grid(), grid()).unwrap();
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
    s
}
