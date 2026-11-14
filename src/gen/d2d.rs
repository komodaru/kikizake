//! The d2d suite's case generator: Direct2D's helpers, stroke styles, geometries and drawing
//! into a WIC bitmap. The inputs are mostly values whose arithmetic is exact in f32 (small
//! integers, quarters, powers of two), so that what a case asks is the API's rule, not
//! rounding; the rest (angles, odd fractions, NaN, infinities) is where the rounding is measured.
use super::*;

const NAN: &str = "0x7fc00000";
const INF: &str = "0x7f800000";
const NINF: &str = "0xff800000";

/// a quarter-step value in -range..=range
fn q(r: &mut XorShift32, range: i64) -> String {
    let v = r.range(-range * 4, range * 4) as f64 / 4.0;
    fmt(v)
}
fn fmt(v: f64) -> String {
    let s = format!("{v}");
    if s == "-0" { "0x80000000".into() } else { s }
}
fn int(r: &mut XorShift32, lo: i64, hi: i64) -> String {
    r.range(lo, hi).to_string()
}

pub fn d2d() -> String {
    let mut s = String::from("# kikizake suite d2d: Direct2D helpers, stroke styles, geometries, drawing into a WIC bitmap (SPEC.md). Made by `kikizake gen d2d`.\n");
    let mut r = XorShift32::new(0xd2d0_0001);
    math(&mut s, &mut r);
    stroke(&mut s, &mut r);
    geometry(&mut s, &mut r);
    sink(&mut s);
    draw(&mut s, &mut r);
    s
}

fn math(s: &mut String, r: &mut XorShift32) {
    // rotations: the right angles and their multiples, common angles, quarter degrees
    let mut angles: Vec<String> = ["0", "90", "180", "270", "360", "-90", "-180", "450", "720", "-720", "30", "45", "60", "120", "135", "1", "0.25", "-0.5", "1000", "3600"].iter().map(|v| v.to_string()).collect();
    for _ in 0..40 {
        angles.push(q(r, 400));
    }
    for (i, a) in angles.iter().enumerate() {
        let (cx, cy) = if i % 3 == 0 { ("0".into(), "0".into()) } else { (int(r, -8, 8), int(r, -8, 8)) };
        writeln!(s, "case rot-{i}\nd2rot {a} {cx} {cy}\nend").unwrap();
    }
    for a in [NAN, INF] {
        writeln!(s, "case rot-{a}\nd2rot {a} 0 0\nend").unwrap();
    }
    let skews = ["0 0", "45 0", "0 45", "45 45", "-45 0", "90 0", "30 60", "180 0", "135 -45"];
    for (i, k) in skews.iter().enumerate() {
        writeln!(s, "case skew-{i}\nd2skew {k} 0 0\nd2skew {k} 2 -3\nend").unwrap();
    }
    for i in 0..20 {
        writeln!(s, "case skew-r{i}\nd2skew {} {} {} {}\nend", q(r, 80), q(r, 80), int(r, -5, 5), int(r, -5, 5)).unwrap();
    }
    // inversion: exact (powers of two, unimodular integer matrices), singular, NaN, random
    let fixed = [
        "1 0 0 1 0 0", "2 0 0 4 1 -8", "0.5 0 0 0.25 3 5", "1 1 0 1 2 3", "2 1 1 1 -1 4", "0 1 -1 0 5 6", "1 2 2 4 0 0",
        "0 0 0 0 1 1", "1 0 0 0 0 0", "3 6 1 2 7 7", "1 0 0 1 0x7fc00000 0", "0x7f800000 0 0 1 0 0", "1e-20 0 0 1e-20 0 0",
        "1e20 0 0 1e20 0 0", "0x00000001 0 0 1 0 0",
    ];
    for (i, m) in fixed.iter().enumerate() {
        writeln!(s, "case inv-{i}\nd2inv {m}\nend").unwrap();
    }
    for i in 0..40 {
        let m: Vec<String> = (0..6).map(|k| if k < 4 { int(r, -3, 3) } else { q(r, 16) }).collect();
        writeln!(s, "case inv-r{i}\nd2inv {}\nend", m.join(" ")).unwrap();
    }
    // the trigonometry helpers (radians), vector length, color spaces
    let rad = ["0", "0.5", "1", "1.5707964", "3.1415927", "-1", "6.2831855", "100", "0.001", "-0.25", "10000", NAN, INF];
    for (i, a) in rad.iter().enumerate() {
        writeln!(s, "case trig-{i}\nd2sincos {a}\nd2tan {a}\nend").unwrap();
    }
    for i in 0..30 {
        let a = fmt(r.range(-4000, 4000) as f64 / 512.0);
        writeln!(s, "case trig-r{i}\nd2sincos {a}\nd2tan {a}\nend").unwrap();
    }
    let vecs = ["0 0 0", "3 4 0", "1 2 2", "2 3 6", "-1 -2 2", "1 1 1", "1e20 1e20 0", "0x7fc00000 0 0", "0x7f800000 1 1", "1e-30 0 0"];
    for (i, v) in vecs.iter().enumerate() {
        writeln!(s, "case vec3-{i}\nd2vec3 {v}\nend").unwrap();
    }
    for i in 0..20 {
        writeln!(s, "case vec3-r{i}\nd2vec3 {} {} {}\nend", q(r, 20), q(r, 20), q(r, 20)).unwrap();
    }
    // color spaces: every pair (and one past the enumeration), channel values over 0..1 by 1/16
    // and past it
    for from in 0..4 {
        for to in 0..4 {
            write!(s, "case cs-{from}-{to}\n").unwrap();
            for k in 0..=16 {
                let v = fmt(k as f64 / 16.0);
                writeln!(s, "d2cs {from} {to} {v} {v} {v} 0.5").unwrap();
            }
            writeln!(s, "d2cs {from} {to} -0.5 1.5 2 1\nd2cs {from} {to} {NAN} {INF} {NINF} 0\nend").unwrap();
        }
    }
    for i in 0..10 {
        let c: Vec<String> = (0..4).map(|_| fmt(r.range(0, 255) as f64 / 256.0)).collect();
        writeln!(s, "case cs-r{i}\nd2cs 1 2 {}\nd2cs 2 1 {}\nend", c.join(" "), c.join(" ")).unwrap();
    }
}

fn stroke(s: &mut String, r: &mut XorShift32) {
    // each field over its range and one past it, the rest valid
    let base = ["0", "0", "0", "0", "10", "0", "0"];
    let edge: &[(usize, &[&str])] = &[
        (0, &["1", "2", "3", "4", "-1"]),
        (1, &["1", "2", "3", "4"]),
        (2, &["1", "2", "3", "4"]),
        (3, &["1", "2", "3", "4"]),
        (4, &["0", "1", "0.5", "-1", "1000", NAN, INF]),
        (5, &["1", "2", "3", "4", "6", "-1"]),
        (6, &["1.5", "-2", NAN, INF]),
    ];
    for (f, vals) in edge {
        for v in *vals {
            let mut p = base.map(String::from);
            p[*f] = v.to_string();
            writeln!(s, "case stroke-{f}-{v}\nd2stroke {}\nend", p.join(" ")).unwrap();
        }
    }
    // dashes with each style: none, one, several, zero, negative, NaN
    let dashes = ["", "1", "1 2", "2 1 0.5 3", "0", "0 0", "-1 2", "1 0x7fc00000", "1 2 3"];
    for style in 0..6 {
        for (i, d) in dashes.iter().enumerate() {
            writeln!(s, "case dash-{style}-{i}\nd2stroke 0 0 0 0 10 {style} 0 {d}\nend").unwrap();
        }
    }
    for i in 0..30 {
        let n = r.range(0, 6);
        let d: Vec<String> = (0..n).map(|_| q(r, 4).replace('-', "")).collect();
        writeln!(s, "case stroke-r{i}\nd2stroke {} {} {} {} {} {} {} {}\nend", int(r, 0, 3), int(r, 0, 3), int(r, 0, 3), int(r, 0, 3), q(r, 8).replace('-', ""), int(r, 0, 5), q(r, 4), d.join(" ")).unwrap();
    }
}

/// an integer polygon: n points in 0..=size
fn polygon(r: &mut XorShift32, n: i64, size: i64) -> Vec<(i64, i64)> {
    (0..n).map(|_| (r.range(0, size), r.range(0, size))).collect()
}
fn figure(p: &[(i64, i64)], close: &str) -> String {
    let mut f = format!("m {} {}", p[0].0, p[0].1);
    for (x, y) in &p[1..] {
        write!(f, " {x} {y}").unwrap();
    }
    f + " " + close
}

fn geometry(s: &mut String, r: &mut XorShift32) {
    // shapes as given: ordinary, inverted, empty, NaN, negative radii, radii past half the size
    let rects = ["0 0 4 3", "1.5 -2 8 6.25", "4 3 0 0", "2 2 2 2", "0 0 0x7fc00000 4", "0 0 0x7f800000 4", "-8 -8 -2 -4"];
    for (i, g) in rects.iter().enumerate() {
        writeln!(s, "case rect-{i}\nd2rect 0 {g}\nd2bounds 0\nd2bounds 0 2 0 0 2 1 1\nd2bounds 0 0 1 -1 0 0 0\nd2area 0\nd2len 0\nend").unwrap();
    }
    let ells = ["0 0 2 1", "3 -1 0.5 4", "1 1 0 0", "1 1 -2 1", "0 0 0x7fc00000 1"];
    for (i, g) in ells.iter().enumerate() {
        writeln!(s, "case ell-{i}\nd2ell 0 {g}\nd2bounds 0\nd2bounds 0 2 0 0 2 1 1\nd2area 0\nd2len 0\nend").unwrap();
    }
    let rrects = ["0 0 8 4 1 1", "0 0 8 4 4 2", "0 0 8 4 10 10", "0 0 8 4 0 0", "0 0 8 4 -1 2", "8 4 0 0 1 1"];
    for (i, g) in rrects.iter().enumerate() {
        writeln!(s, "case rrect-{i}\nd2rrect 0 {g}\nd2bounds 0\nd2area 0\nd2len 0\nend").unwrap();
    }
    // transformed bounds of random rectangles under exact transforms
    for i in 0..40 {
        let (l, t) = (r.range(-8, 8), r.range(-8, 8));
        let (w, h) = (r.range(0, 8), r.range(0, 8));
        let m: Vec<String> = (0..6).map(|k| if k < 4 { q(r, 2) } else { q(r, 8) }).collect();
        writeln!(s, "case xbounds-{i}\nd2rect 0 {l} {t} {} {}\nd2bounds 0 {}\nd2xf 1 0 {}\nd2bounds 1\nd2area 1\nend", l + w, t + h, m.join(" "), m.join(" ")).unwrap();
    }
    // paths: figure and segment counts, bounds, area, length of integer polygons
    for i in 0..60 {
        let figs = r.range(1, 3);
        let mut line = format!("d2path 0 {}", if r.chance(2) { "wind" } else { "alt" });
        for _ in 0..figs {
            let n = r.range(1, 7);
            let p = polygon(r, n, 12);
            line += " ";
            line += &figure(&p, if r.chance(4) { "o" } else { "z" });
        }
        writeln!(s, "case path-{i}\n{line}\nd2count 0\nd2bounds 0\nd2bounds 0 1 0 0 -1 4 4\nd2area 0\nd2len 0\nend").unwrap();
    }
    // lengths that are exact: Pythagorean sides
    for (i, p) in ["m 0 0 3 4 z", "m 0 0 3 4 o", "m 1 1 4 5 4 1 z", "m 0 0 5 12 z", "m 0 0 8 6 8 0 o", "m 0 0 0 0 z"].iter().enumerate() {
        writeln!(s, "case plen-{i}\nd2path 0 alt {p}\nd2len 0\nd2area 0\nend").unwrap();
    }
    // hit tests: polygons, both fill modes, points at every half and quarter step around them
    let shapes = [
        ("square", "m 2 2 10 2 10 10 2 10 z"),
        ("tri", "m 1 1 11 3 4 11 z"),
        ("star", "m 6 0 9 12 0 4 12 4 3 12 z"),
        ("two", "m 0 0 6 0 6 6 0 6 z m 3 3 9 3 9 9 3 9 z"),
        ("ring", "m 0 0 12 0 12 12 0 12 z m 3 3 3 9 9 9 9 3 z"),
        ("ringsame", "m 0 0 12 0 12 12 0 12 z m 3 3 9 3 9 9 3 9 z"),
        ("open", "m 2 2 10 2 10 10 o"),
    ];
    for (name, f) in shapes {
        for mode in ["alt", "wind"] {
            write!(s, "case in-{name}-{mode}\nd2path 0 {mode} {f}\n").unwrap();
            for y in 0..=26 {
                for x in 0..=26 {
                    if (x + y * 3) % 4 == 0 {
                        writeln!(s, "d2in 0 {} {}", fmt(x as f64 / 2.0 - 0.5), fmt(y as f64 / 2.0 - 0.5)).unwrap();
                    }
                }
            }
            for _ in 0..30 {
                writeln!(s, "d2in 0 {} {}", fmt(r.range(-8, 104) as f64 / 8.0), fmt(r.range(-8, 104) as f64 / 8.0)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // the tolerance: points just outside a square's edge at 1/16 steps, tolerances 0.25 and others
    for tol in ["0.25", "0.5", "1", "0.01", "0"] {
        write!(s, "case intol-{tol}\nd2rect 0 0 0 8 8\n").unwrap();
        for k in 0..12 {
            let d = fmt(8.0 + k as f64 / 16.0);
            writeln!(s, "d2in 0 {d} 4 {tol}\nd2in 0 4 {} {tol}", fmt(-(k as f64) / 16.0)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    write!(s, "case in-ell\nd2ell 0 6 6 5 3\n").unwrap();
    for _ in 0..80 {
        writeln!(s, "d2in 0 {} {}", fmt(r.range(0, 96) as f64 / 8.0), fmt(r.range(0, 96) as f64 / 8.0)).unwrap();
    }
    writeln!(s, "end").unwrap();
    // relations: rectangles apart, overlapping, inside one another, the same, touching
    for i in 0..60 {
        let a = (r.range(0, 8), r.range(0, 8), r.range(1, 8), r.range(1, 8));
        let b = (r.range(0, 8), r.range(0, 8), r.range(1, 8), r.range(1, 8));
        writeln!(s, "case cmp-{i}\nd2rect 0 {} {} {} {}\nd2rect 1 {} {} {} {}\nd2cmp 0 1\nd2cmp 1 0\nend", a.0, a.1, a.0 + a.2, a.1 + a.3, b.0, b.1, b.0 + b.2, b.1 + b.3).unwrap();
    }
    for (i, (a, b)) in [("0 0 8 8", "2 2 4 4"), ("0 0 8 8", "0 0 8 8"), ("0 0 4 4", "4 0 8 4"), ("0 0 4 4", "5 0 8 4"), ("0 0 4 4", "4.125 0 8 4"), ("0 0 8 8", "0 0 4 4")].iter().enumerate() {
        writeln!(s, "case cmpf-{i}\nd2rect 0 {a}\nd2rect 1 {b}\nd2cmp 0 1\nd2cmp 1 0\nend").unwrap();
    }
    writeln!(s, "case cmp-path\nd2path 0 alt m 0 0 8 0 8 8 0 8 z\nd2rect 1 2 2 4 4\nd2ell 2 20 20 1 1\nd2cmp 0 1\nd2cmp 1 0\nd2cmp 0 2\nd2cmp 2 0\nd2cmp 0 0\nend").unwrap();
    writeln!(s, "case count-rect\nd2rect 0 0 0 1 1\nd2count 0\nend").unwrap();
}

fn sink(s: &mut String) {
    // the path geometry's states (each case one geometry, opened, filled, closed, asked)
    let seqs = [
        "count",
        "open count",
        "open close count",
        "open close open",
        "open open",
        "open close close",
        "open begin 0 0 line 1 0 line 1 1 end close count",
        "open begin 0 0 line 1 0 close count",
        "open line 1 0 close count",
        "open end close count",
        "open begin 0 0 begin 1 1 end close count",
        "open begin 0 0 end begin 2 2 line 3 3 end close count",
        "open begin 0 0 line 1 0 end close begin 5 5 end close count",
        "open fill wind begin 0 0 line 1 0 line 0 1 end fill alt close count",
        "open begin 0 0 line 1 0 line 0 1 end fill wind close count",
        "open begin 0x7fc00000 0 line 1 0 end close count",
    ];
    for (i, q) in seqs.iter().enumerate() {
        writeln!(s, "case sink-{i}\nd2sink {q}\nend").unwrap();
    }
}

fn color(r: &mut XorShift32, opaque: bool) -> String {
    let a = if opaque { 255 } else { *r.pick(&[0, 1, 64, 127, 128, 200, 254, 255]) };
    // (premultiplied targets: the color channels at most the alpha, and sometimes past it)
    let c = |r: &mut XorShift32| r.range(0, 255);
    format!("{} {} {} {a}", c(r), c(r), c(r))
}

fn draw(s: &mut String, r: &mut XorShift32) {
    writeln!(s, "case target\nd2target 4 2\nd2read\nend").unwrap();
    // brushes keep their color as given
    writeln!(s, "case brush\nd2target 1 1\nd2brush 0.5 1 0 1\nd2brush -1 2 0x7fc00000 0.5\nd2brush 0 0 0 0x7f800000\nend").unwrap();
    // Clear: every channel value (premultiplied by the alpha), opaque and not
    for a in [255, 254, 200, 128, 127, 64, 1, 0] {
        write!(s, "case clear-{a}\nd2target 16 1\n").unwrap();
        for v in (0..256).step_by(16) {
            writeln!(s, "d2clear {v} {} {} {a}\nd2read", 255 - v, (v * 7) & 255).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    write!(s, "case clear-all\nd2target 1 1\n").unwrap();
    for v in 0..256 {
        writeln!(s, "d2clear {v} {v} {v} 255\nd2read").unwrap();
    }
    writeln!(s, "end").unwrap();
    // aliased rectangles: integer, half and quarter edges, inverted, outside
    for i in 0..40 {
        let e = |r: &mut XorShift32| fmt(r.range(-8, 72) as f64 / 4.0);
        let aa = if i % 2 == 0 { "aliased" } else { "per" };
        let (a, b, c, d) = if i % 2 == 1 {
            // (antialiased: integer edges only, where coverage is all or nothing)
            (int(r, -2, 16), int(r, -2, 16), int(r, -2, 16), int(r, -2, 16))
        } else {
            (e(r), e(r), e(r), e(r))
        };
        writeln!(s, "case fill-{i}\nd2target 16 16\nd2aa {aa}\nd2clear 0 0 0 0\nd2fill {a} {b} {c} {d} {}\nd2read\nend", color(r, true)).unwrap();
    }
    // source-over: a translucent rectangle over a cleared target, many colors
    for i in 0..30 {
        writeln!(s, "case over-{i}\nd2target 8 4\nd2aa aliased\nd2clear {}\nd2fill 2 0 8 4 {}\nd2fill 0 2 6 4 {}\nd2read\nend", color(r, i % 3 != 0), color(r, false), color(r, false)).unwrap();
    }
    // antialiased edges: half and quarter pixel coverage
    for i in 0..12 {
        let e = |r: &mut XorShift32| fmt(r.range(0, 64) as f64 / 4.0);
        writeln!(s, "case aa-{i}\nd2target 16 4\nd2clear 0 0 0 0\nd2fill {} 0 {} 4 255 255 255 255\nd2read\nend", e(r), e(r)).unwrap();
    }
    // transforms: translate by quarters, scale by two
    for (i, m) in ["1 0 0 1 2 3", "2 0 0 2 0 0", "1 0 0 1 0.5 0.5", "-1 0 0 1 16 0", "0 1 1 0 0 0"].iter().enumerate() {
        writeln!(s, "case xform-{i}\nd2target 16 16\nd2aa aliased\nd2xform {m}\nd2fill 1 1 5 3 255 0 0 255\nd2read\nend").unwrap();
    }
    // polygons, aliased, both fill modes
    for i in 0..30 {
        let n = r.range(3, 6);
        let p = polygon(r, n, 16);
        let mode = if i % 2 == 0 { "alt" } else { "wind" };
        writeln!(s, "case fillg-{i}\nd2target 16 16\nd2aa aliased\nd2path 0 {mode} {}\nd2fillg 0 {}\nd2read\nend", figure(&p, "z"), color(r, true)).unwrap();
    }
    writeln!(s, "case fillg-star\nd2target 16 16\nd2aa aliased\nd2path 0 alt m 8 0 12 16 0 5 16 5 4 16 z\nd2fillg 0 255 255 255 255\nd2read\nd2path 1 wind m 8 0 12 16 0 5 16 5 4 16 z\nd2clear 0 0 0 0\nd2fillg 1 255 255 255 255\nd2read\nend").unwrap();
    writeln!(s, "case nort\nd2fill 0 0 1 1 0 0 0 255\nd2read\nend").unwrap();
}
