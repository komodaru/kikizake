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
    curves(&mut s, &mut r);
    arith(&mut s, &mut r);
    target(&mut s, &mut r);
    target_more(&mut s, &mut r);
    gradfit(&mut s);
    bitmaps_more(&mut s, &mut r);
    layerfit(&mut s);
    bitmapfit(&mut s, &mut r);
    clipfit(&mut s);
    huge(&mut s);
    brushworld(&mut s, &mut r);
    strokefit(&mut s, &mut r);
    xfbounds(&mut s, &mut r);
    bmpsrcfit(&mut s, &mut r);
    xfbounds_more(&mut s);
    strokemore(&mut s, &mut r);
    bmpsrcmore(&mut s, &mut r);
    strokewide(&mut s, &mut r);
    gradscan(&mut s);
    strokelobe(&mut s, &mut r);
    xfell(&mut s, &mut r);
    probes(&mut s, &mut r);
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
        writeln!(s, "case rot-{i}\nd2rot {a} {cx} {cy}\nend");
    }
    for a in [NAN, INF] {
        writeln!(s, "case rot-{a}\nd2rot {a} 0 0\nend");
    }
    let skews = ["0 0", "45 0", "0 45", "45 45", "-45 0", "90 0", "30 60", "180 0", "135 -45"];
    for (i, k) in skews.iter().enumerate() {
        writeln!(s, "case skew-{i}\nd2skew {k} 0 0\nd2skew {k} 2 -3\nend");
    }
    for i in 0..20 {
        writeln!(s, "case skew-r{i}\nd2skew {} {} {} {}\nend", q(r, 80), q(r, 80), int(r, -5, 5), int(r, -5, 5));
    }
    // inversion: exact (powers of two, unimodular integer matrices), singular, NaN, random
    let fixed = [
        "1 0 0 1 0 0", "2 0 0 4 1 -8", "0.5 0 0 0.25 3 5", "1 1 0 1 2 3", "2 1 1 1 -1 4", "0 1 -1 0 5 6", "1 2 2 4 0 0",
        "0 0 0 0 1 1", "1 0 0 0 0 0", "3 6 1 2 7 7", "1 0 0 1 0x7fc00000 0", "0x7f800000 0 0 1 0 0", "1e-20 0 0 1e-20 0 0",
        "1e20 0 0 1e20 0 0", "0x00000001 0 0 1 0 0",
    ];
    for (i, m) in fixed.iter().enumerate() {
        writeln!(s, "case inv-{i}\nd2inv {m}\nend");
    }
    for i in 0..40 {
        let m: Vec<String> = (0..6).map(|k| if k < 4 { int(r, -3, 3) } else { q(r, 16) }).collect();
        writeln!(s, "case inv-r{i}\nd2inv {}\nend", m.join(" "));
    }
    // the trigonometry helpers (radians), vector length, color spaces
    let rad = ["0", "0.5", "1", "1.5707964", "3.1415927", "-1", "6.2831855", "100", "0.001", "-0.25", "10000", NAN, INF];
    for (i, a) in rad.iter().enumerate() {
        writeln!(s, "case trig-{i}\nd2sincos {a}\nd2tan {a}\nend");
    }
    for i in 0..30 {
        let a = fmt(r.range(-4000, 4000) as f64 / 512.0);
        writeln!(s, "case trig-r{i}\nd2sincos {a}\nd2tan {a}\nend");
    }
    let vecs = ["0 0 0", "3 4 0", "1 2 2", "2 3 6", "-1 -2 2", "1 1 1", "1e20 1e20 0", "0x7fc00000 0 0", "0x7f800000 1 1", "1e-30 0 0"];
    for (i, v) in vecs.iter().enumerate() {
        writeln!(s, "case vec3-{i}\nd2vec3 {v}\nend");
    }
    for i in 0..20 {
        writeln!(s, "case vec3-r{i}\nd2vec3 {} {} {}\nend", q(r, 20), q(r, 20), q(r, 20));
    }
    // color spaces: every pair (and one past the enumeration), channel values over 0..1 by 1/16
    // and past it
    for from in 0..4 {
        for to in 0..4 {
            // (the curve between 0 and 1 apart from the ends, the clamping and the alpha)
            write!(s, "case cs-{from}-{to}\n");
            for k in 1..16 {
                let v = fmt(k as f64 / 16.0);
                writeln!(s, "d2cs {from} {to} {v} {v} {v} 0.5");
            }
            writeln!(s, "end\ncase cs-ends-{from}-{to}\nd2cs {from} {to} 0 0 0 0.5\nd2cs {from} {to} 1 1 1 0.5\nd2cs {from} {to} 0 1 0 1");
            writeln!(s, "d2cs {from} {to} -0.5 1.5 2 1\nd2cs {from} {to} {NAN} {INF} {NINF} 0\nd2cs {from} {to} 0 1 0 1.5\nd2cs {from} {to} 1 0 1 -1\nd2cs {from} {to} 0 0 1 {NAN}\nd2cs {from} {to} 0x80000000 1 0 {INF}\nend");
        }
    }
    for i in 0..10 {
        let c: Vec<String> = (0..4).map(|_| fmt(r.range(0, 255) as f64 / 256.0)).collect();
        writeln!(s, "case cs-r{i}\nd2cs 1 2 {}\nd2cs 2 1 {}\nend", c.join(" "), c.join(" "));
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
            writeln!(s, "case stroke-{f}-{v}\nd2stroke {}\nend", p.join(" "));
        }
    }
    // dashes with each style: none, one, several, zero, negative, NaN
    let dashes = ["", "1", "1 2", "2 1 0.5 3", "0", "0 0", "-1 2", "1 0x7fc00000", "1 2 3"];
    for style in 0..6 {
        for (i, d) in dashes.iter().enumerate() {
            writeln!(s, "case dash-{style}-{i}\nd2stroke 0 0 0 0 10 {style} 0 {d}\nend");
        }
    }
    for i in 0..30 {
        let n = r.range(0, 6);
        let d: Vec<String> = (0..n).map(|_| q(r, 4).replace('-', "")).collect();
        writeln!(s, "case stroke-r{i}\nd2stroke {} {} {} {} {} {} {} {}\nend", int(r, 0, 3), int(r, 0, 3), int(r, 0, 3), int(r, 0, 3), q(r, 8).replace('-', ""), int(r, 0, 5), q(r, 4), d.join(" "));
    }
}

/// an integer polygon: n points in 0..=size
fn polygon(r: &mut XorShift32, n: i64, size: i64) -> Vec<(i64, i64)> {
    (0..n).map(|_| (r.range(0, size), r.range(0, size))).collect()
}
fn figure(p: &[(i64, i64)], close: &str) -> String {
    let mut f = format!("m {} {}", p[0].0, p[0].1);
    for (x, y) in &p[1..] {
        write!(f, " {x} {y}");
    }
    f + " " + close
}

fn geometry(s: &mut String, r: &mut XorShift32) {
    // shapes as given: ordinary, inverted, empty, NaN, negative radii, radii past half the size
    let rects = ["0 0 4 3", "1.5 -2 8 6.25", "4 3 0 0", "2 2 2 2", "0 0 0x7fc00000 4", "0 0 0x7f800000 4", "-8 -8 -2 -4"];
    for (i, g) in rects.iter().enumerate() {
        writeln!(s, "case rect-{i}\nd2rect 0 {g}\nd2bounds 0\nd2bounds 0 2 0 0 2 1 1\nd2bounds 0 0 1 -1 0 0 0\nd2area 0\nd2len 0\nend");
    }
    let ells = ["0 0 2 1", "3 -1 0.5 4", "1 1 0 0", "1 1 -2 1", "0 0 0x7fc00000 1"];
    for (i, g) in ells.iter().enumerate() {
        writeln!(s, "case ell-{i}\nd2ell 0 {g}\nd2bounds 0\nd2bounds 0 2 0 0 2 1 1\nend");
        // (the flattened curve's area and length, each by itself)
        writeln!(s, "case ell-area-{i}\nd2ell 0 {g}\nd2area 0\nend\ncase ell-len-{i}\nd2ell 0 {g}\nd2len 0\nend");
    }
    let rrects = ["0 0 8 4 1 1", "0 0 8 4 4 2", "0 0 8 4 10 10", "0 0 8 4 0 0", "0 0 8 4 -1 2", "8 4 0 0 1 1"];
    for (i, g) in rrects.iter().enumerate() {
        writeln!(s, "case rrect-{i}\nd2rrect 0 {g}\nd2bounds 0\nend");
        writeln!(s, "case rrect-area-{i}\nd2rrect 0 {g}\nd2area 0\nend\ncase rrect-len-{i}\nd2rrect 0 {g}\nd2len 0\nend");
        writeln!(s, "case rrect-in-{i}\nd2rrect 0 {g}\nd2in 0 4 2\nd2in 0 0.25 0.25\nd2in 0 7.5 3.75\nend");
    }
    // transformed bounds of random rectangles under exact transforms
    for i in 0..40 {
        let (l, t) = (r.range(-8, 8), r.range(-8, 8));
        let (w, h) = (r.range(0, 8), r.range(0, 8));
        let m: Vec<String> = (0..6).map(|k| if k < 4 { q(r, 2) } else { q(r, 8) }).collect();
        writeln!(s, "case xbounds-{i}\nd2rect 0 {l} {t} {} {}\nd2bounds 0 {}\nd2xf 1 0 {}\nd2bounds 1\nd2area 1\nend", l + w, t + h, m.join(" "), m.join(" "));
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
        writeln!(s, "case path-{i}\n{line}\nd2count 0\nd2bounds 0\nd2bounds 0 1 0 0 -1 4 4\nd2area 0\nd2len 0\nend");
    }
    // lengths that are exact: Pythagorean sides
    for (i, p) in ["m 0 0 3 4 z", "m 0 0 3 4 o", "m 1 1 4 5 4 1 z", "m 0 0 5 12 z", "m 0 0 8 6 8 0 o", "m 0 0 0 0 z"].iter().enumerate() {
        writeln!(s, "case plen-{i}\nd2path 0 alt {p}\nd2len 0\nd2area 0\nend");
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
            write!(s, "case in-{name}-{mode}\nd2path 0 {mode} {f}\n");
            for y in 0..=26 {
                for x in 0..=26 {
                    if (x + y * 3) % 4 == 0 {
                        writeln!(s, "d2in 0 {} {}", fmt(x as f64 / 2.0 - 0.5), fmt(y as f64 / 2.0 - 0.5));
                    }
                }
            }
            for _ in 0..30 {
                writeln!(s, "d2in 0 {} {}", fmt(r.range(-8, 104) as f64 / 8.0), fmt(r.range(-8, 104) as f64 / 8.0));
            }
            writeln!(s, "end");
        }
    }
    // the tolerance: points just outside a square's edge at 1/16 steps, tolerances 0.25 and others
    for tol in ["0.25", "0.5", "1", "0.01", "0"] {
        write!(s, "case intol-{tol}\nd2rect 0 0 0 8 8\n");
        for k in 0..12 {
            let d = fmt(8.0 + k as f64 / 16.0);
            writeln!(s, "d2in 0 {d} 4 {tol}\nd2in 0 4 {} {tol}", fmt(-(k as f64) / 16.0));
        }
        writeln!(s, "end");
    }
    // ellipses: points well inside or outside, and (by themselves) points near the curve
    for (k, (cx, cy, rx, ry)) in [(6.0, 6.0, 5.0, 3.0), (4.0, 5.0, 2.0, 4.5), (8.0, 8.0, 6.0, 6.0)].into_iter().enumerate() {
        let (mut far, mut near) = (String::new(), String::new());
        for _ in 0..120 {
            let (x, y) = (r.range(0, 128) as f64 / 8.0, r.range(0, 128) as f64 / 8.0);
            let v: f64 = ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2);
            let l = format!("d2in 0 {} {}\n", fmt(x), fmt(y));
            if (v.sqrt() - 1.0).abs() * rx.min(ry) < 0.75 { near += &l } else { far += &l }
        }
        writeln!(s, "case in-ell-{k}\nd2ell 0 {cx} {cy} {rx} {ry}\n{far}end\ncase in-ellnear-{k}\nd2ell 0 {cx} {cy} {rx} {ry}\n{near}end");
    }
    // relations: rectangles apart, overlapping, inside one another, the same, touching
    for i in 0..60 {
        let a = (r.range(0, 8), r.range(0, 8), r.range(1, 8), r.range(1, 8));
        let b = (r.range(0, 8), r.range(0, 8), r.range(1, 8), r.range(1, 8));
        writeln!(s, "case cmp-{i}\nd2rect 0 {} {} {} {}\nd2rect 1 {} {} {} {}\nd2cmp 0 1\nd2cmp 1 0\nend", a.0, a.1, a.0 + a.2, a.1 + a.3, b.0, b.1, b.0 + b.2, b.1 + b.3);
    }
    for (i, (a, b)) in [("0 0 8 8", "2 2 4 4"), ("0 0 8 8", "0 0 8 8"), ("0 0 4 4", "4 0 8 4"), ("0 0 4 4", "5 0 8 4"), ("0 0 4 4", "4.125 0 8 4"), ("0 0 8 8", "0 0 4 4")].iter().enumerate() {
        writeln!(s, "case cmpf-{i}\nd2rect 0 {a}\nd2rect 1 {b}\nd2cmp 0 1\nd2cmp 1 0\nend");
    }
    // (compared with Windows only for now: transformed curves, NaN in a path, just past the tolerance)
    writeln!(s, "case rrect-xf\nd2rrect 0 0 0 8 4 1 1\nd2bounds 0 0 1 -1 0 0 0\nend\ncase ell-xf\nd2ell 0 0 0 2 1\nd2bounds 0 0 1 -1 0 0 0\nend");
    for (i, m) in ["1 1 -1 1 0 0", "1 0 1 1 0 0", "2 1 0.5 -1 3 4", "0.75 -0.5 0.25 1.5 -2 1"].iter().enumerate() {
        writeln!(s, "case rrect-xf{i}\nd2rrect 0 0 0 8 4 1 1\nd2bounds 0 {m}\nd2rrect 1 1 1 7 9 3 2\nd2bounds 1 {m}\nend\ncase ell-xf{i}\nd2ell 0 0 0 2 1\nd2bounds 0 {m}\nd2ell 1 3 -2 4 0.5\nd2bounds 1 {m}\nend");
    }
    writeln!(s, "case path-nan2\nd2path 0 alt m 0 0 4 {NAN} 4 4 z\nd2bounds 0\nd2bounds 0 2 0 0 2 1 1\nd2path 1 alt m 0 0 4 0 4 4 z m {NAN} {NAN} 1 1 o\nd2bounds 1\nend");
    writeln!(s, "case path-nan\nd2path 0 alt m 0 0 {NAN} 4 4 4 z\nd2bounds 0\nend\ncase in-edge\nd2rect 0 0 0 8 8\nd2in 0 8.2505 4\nend");
    writeln!(s, "case cmp-same\nd2rect 0 1 1 3 3\nd2cmp 0 0\nend");
    writeln!(s, "case cmp-path\nd2path 0 alt m 0 0 8 0 8 8 0 8 z\nd2rect 1 2 2 4 4\nd2ell 2 20 20 1 1\nd2cmp 0 1\nd2cmp 1 0\nd2cmp 0 2\nd2cmp 2 0\nd2cmp 0 0\nend");
    writeln!(s, "case count-rect\nd2rect 0 0 0 1 1\nd2count 0\nend");
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
        writeln!(s, "case sink-{i}\nd2sink {q}\nend");
    }
}

fn color(r: &mut XorShift32, opaque: bool) -> String {
    let a = if opaque { 255 } else { *r.pick(&[0, 1, 64, 127, 128, 200, 254, 255]) };
    // (premultiplied targets: the color channels at most the alpha, and sometimes past it)
    let c = |r: &mut XorShift32| r.range(0, 255);
    format!("{} {} {} {a}", c(r), c(r), c(r))
}

fn draw(s: &mut String, r: &mut XorShift32) {
    writeln!(s, "case target\nd2target 4 2\nd2read\nend");
    // brushes keep their color as given
    writeln!(s, "case brush\nd2target 1 1\nd2brush 0.5 1 0 1\nd2brush -1 2 0x7fc00000 0.5\nd2brush 0 0 0 0x7f800000\nend");
    // Clear: every channel value (premultiplied by the alpha), opaque and not
    for a in [255, 254, 200, 128, 127, 64, 1, 0] {
        write!(s, "case clear-{a}\nd2target 16 1\n");
        for v in (0..256).step_by(16) {
            writeln!(s, "d2clear {v} {} {} {a}\nd2read", 255 - v, (v * 7) & 255);
        }
        writeln!(s, "end");
    }
    write!(s, "case clear-all\nd2target 1 1\n");
    for v in 0..256 {
        writeln!(s, "d2clear {v} {v} {v} 255\nd2read");
    }
    writeln!(s, "end");
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
        writeln!(s, "case fill-{i}\nd2target 16 16\nd2aa {aa}\nd2clear 0 0 0 0\nd2fill {a} {b} {c} {d} {}\nd2read\nend", color(r, true));
    }
    // source-over: a translucent rectangle over a cleared target, many colors
    for i in 0..30 {
        writeln!(s, "case over-{i}\nd2target 8 4\nd2aa aliased\nd2clear {}\nd2fill 2 0 8 4 {}\nd2fill 0 2 6 4 {}\nd2read\nend", color(r, i % 3 != 0), color(r, false), color(r, false));
    }
    // antialiased edges: half and quarter pixel coverage
    for i in 0..12 {
        let e = |r: &mut XorShift32| fmt(r.range(0, 64) as f64 / 4.0);
        writeln!(s, "case aa-{i}\nd2target 16 4\nd2clear 0 0 0 0\nd2fill {} 0 {} 4 255 255 255 255\nd2read\nend", e(r), e(r));
    }
    // transforms: translate by quarters, scale by two
    for (i, m) in ["1 0 0 1 2 3", "2 0 0 2 0 0", "1 0 0 1 0.5 0.5", "-1 0 0 1 16 0", "0 1 1 0 0 0"].iter().enumerate() {
        writeln!(s, "case xform-{i}\nd2target 16 16\nd2aa aliased\nd2xform {m}\nd2fill 1 1 5 3 255 0 0 255\nd2read\nend");
    }
    // polygons, aliased, both fill modes
    for i in 0..30 {
        let n = r.range(3, 6);
        let p = polygon(r, n, 16);
        let mode = if i % 2 == 0 { "alt" } else { "wind" };
        writeln!(s, "case fillg-{i}\nd2target 16 16\nd2aa aliased\nd2path 0 {mode} {}\nd2fillg 0 {}\nd2read\nend", figure(&p, "z"), color(r, true));
    }
    writeln!(s, "case fillg-star\nd2target 16 16\nd2aa aliased\nd2path 0 alt m 8 0 12 16 0 5 16 5 4 16 z\nd2fillg 0 255 255 255 255\nd2read\nd2path 1 wind m 8 0 12 16 0 5 16 5 4 16 z\nd2clear 0 0 0 0\nd2fillg 1 255 255 255 255\nd2read\nend");
    // curves and antialiased colors over a background (compared with Windows only for now)
    writeln!(s, "case fill-ell\nd2target 16 16\nd2aa aliased\nd2ell 0 8 8 5 3\nd2fillg 0 255 255 255 255\nd2read\nend");
    writeln!(s, "case fill-ell-aa\nd2target 16 16\nd2ell 0 8 8 5 3\nd2fillg 0 255 255 255 255\nd2read\nend");
    for i in 0..12 {
        let e = |r: &mut XorShift32| fmt(r.range(0, 64) as f64 / 4.0);
        writeln!(s, "case aa-c{i}\nd2target 16 2\nd2clear {}\nd2fill {} 0 {} 2 {}\nd2read\nend", color(r, i % 2 == 0), e(r), e(r), color(r, i % 3 == 0));
    }
    writeln!(s, "case nort\nd2fill 0 0 1 1 0 0 0 255\nd2read\nend");
}

/// what the geometries give a sink (Simplify, Outline, Widen, CombineWithGeometry, Tessellate),
/// dense color space curves, antialiased coverage, relations between any shapes
fn curves(s: &mut String, r: &mut XorShift32) {
    // ellipses and rounded rectangles as cubics, and flattened at several tolerances
    let ells = ["0 0 1 1", "0 0 2 1", "3 -1 0.5 4", "8 8 6 6", "0 0 20 10", "0 0 100 100", "1 1 0 0", "1 1 -2 1", "0 0 0.125 0.125", "5 5 3 0"];
    for (i, e) in ells.iter().enumerate() {
        writeln!(s, "case simp-ell-{i}\nd2ell 0 {e}\nd2simp 0 cubics\nend");
        for tol in ["0.25", "1", "0.05", "0", "4"] {
            writeln!(s, "case flat-ell-{i}-{tol}\nd2ell 0 {e}\nd2simp 0 lines {tol}\nend");
        }
    }
    let rrs = ["0 0 8 4 1 1", "0 0 8 4 4 2", "0 0 8 4 10 10", "0 0 8 4 0 0", "0 0 8 4 -1 2", "8 4 0 0 1 1", "0 0 40 30 7 3", "0 0 8 4 0 2"];
    for (i, g) in rrs.iter().enumerate() {
        writeln!(s, "case simp-rrect-{i}\nd2rrect 0 {g}\nd2simp 0 cubics\nend");
        for tol in ["0.25", "1", "0.05"] {
            writeln!(s, "case flat-rrect-{i}-{tol}\nd2rrect 0 {g}\nd2simp 0 lines {tol}\nend");
        }
    }
    // paths and rectangles through a sink as they are
    for i in 0..10 {
        let n = r.range(2, 6);
        let p = polygon(r, n, 12);
        let mode = if i % 2 == 0 { "alt" } else { "wind" };
        writeln!(s, "case simp-path-{i}\nd2path 0 {mode} {} {}\nd2simp 0 cubics\nd2simp 0 lines\nend", figure(&p, "z"), figure(&polygon(r, 3, 12), "o"));
    }
    writeln!(s, "case simp-rect\nd2rect 0 1 2 5 7\nd2simp 0 cubics\nd2rect 1 5 7 1 2\nd2simp 1 lines\nend");
    // outlines, widenings, combinations, tessellations
    let shapes = ["d2rect 0 1 1 7 5", "d2path 0 alt m 6 0 9 12 0 4 12 4 3 12 z", "d2path 0 wind m 6 0 9 12 0 4 12 4 3 12 z", "d2path 0 alt m 0 0 6 0 6 6 0 6 z m 3 3 9 3 9 9 3 9 z", "d2ell 0 4 4 3 2", "d2path 0 alt m 1 1 5 1 5 5 o"];
    for (i, g) in shapes.iter().enumerate() {
        writeln!(s, "case outline-{i}\n{g}\nd2outline 0\nend\ncase tess-{i}\n{g}\nd2tess 0\nend");
        for w in ["1", "2.5", "0"] {
            writeln!(s, "case widen-{i}-{w}\n{g}\nd2widen 0 {w}\nend");
        }
    }
    let pairs = [("d2rect 0 0 0 4 4", "d2rect 1 2 2 6 6"), ("d2rect 0 0 0 4 4", "d2rect 1 5 5 6 6"), ("d2rect 0 0 0 8 8", "d2rect 1 2 2 4 4"), ("d2rect 0 0 0 4 4", "d2path 1 alt m 2 -2 6 2 2 6 -2 2 z"), ("d2ell 0 4 4 3 3", "d2rect 1 4 4 8 8"), ("d2path 0 alt m 0 0 6 0 6 6 0 6 z", "d2path 1 wind m 3 3 9 3 9 9 3 9 z")];
    for (i, (a, b)) in pairs.iter().enumerate() {
        for mode in ["union", "intersect", "xor", "exclude"] {
            writeln!(s, "case combine-{i}-{mode}\n{a}\n{b}\nd2combine 0 1 {mode}\nend");
        }
    }
    // relations between any two shapes: paths, ellipses, rounded rectangles
    for i in 0..40 {
        let mk = |r: &mut XorShift32, slot: u32| -> String {
            match r.range(0, 3) {
                0 => format!("d2ell {slot} {} {} {} {}", r.range(2, 10), r.range(2, 10), r.range(1, 5), r.range(1, 5)),
                1 => format!("d2rrect {slot} {} {} {} {} 1 1", r.range(0, 5), r.range(0, 5), r.range(6, 12), r.range(6, 12)),
                _ => {
                    let n = r.range(3, 6);
                    let p = polygon(r, n, 12);
                    format!("d2path {slot} {} {}", if r.chance(2) { "alt" } else { "wind" }, figure(&p, "z"))
                }
            }
        };
        let (a, b) = (mk(r, 0), mk(r, 1));
        writeln!(s, "case cmpg-{i}\n{a}\n{b}\nd2cmp 0 1\nd2cmp 1 0\nend");
    }
    // the color space curve: every 1/256, and random floats in 0..1
    for (from, to) in [(1, 2), (2, 1)] {
        write!(s, "case csd-{from}-{to}\n");
        for k in 1..256 {
            let v = fmt(k as f64 / 256.0);
            writeln!(s, "d2cs {from} {to} {v} {v} {v} 1");
        }
        writeln!(s, "end");
        write!(s, "case csr-{from}-{to}\n");
        for _ in 0..200 {
            let b: Vec<String> = (0..3).map(|_| format!("0x{:08x}", 0x3000_0000 + r.next() % 0x0f80_0000)).collect();
            writeln!(s, "d2cs {from} {to} {} 1", b.join(" "));
        }
        writeln!(s, "end");
    }
    // antialiased coverage: edges at sixteenths, white on an empty target, colors over colors
    for i in 0..24 {
        let e = |r: &mut XorShift32| fmt(r.range(0, 256) as f64 / 16.0);
        writeln!(s, "case aa16-{i}\nd2target 16 2\nd2fill {} 0 {} 1 255 255 255 255\nd2fill 0 {} 16 {} 255 255 255 255\nd2read\nend", e(r), e(r), fmt(1.0 + r.range(0, 16) as f64 / 16.0), fmt(2.0));
    }
    for i in 0..30 {
        let e = |r: &mut XorShift32| fmt(r.range(0, 64) as f64 / 4.0);
        writeln!(s, "case aac-{i}\nd2target 16 2\nd2clear {}\nd2fill {} 0 {} 2 {}\nd2read\nend", color(r, i % 2 == 0), e(r), e(r), color(r, i % 3 == 0));
    }
    for i in 0..12 {
        let p = polygon(r, 3, 16);
        writeln!(s, "case aapoly-{i}\nd2target 16 16\nd2path 0 alt {}\nd2fillg 0 255 255 255 255\nd2read\nend", figure(&p, "z"));
    }
    for (i, e) in ["8 8 5 3", "8 8 7.5 7.5", "4.5 4.5 2 2"].iter().enumerate() {
        writeln!(s, "case fillell-{i}\nd2target 16 16\nd2aa aliased\nd2ell 0 {e}\nd2fillg 0 255 255 255 255\nd2read\nend\ncase fillell-aa-{i}\nd2target 16 16\nd2ell 0 {e}\nd2fillg 0 255 255 255 255\nd2read\nend");
    }
    writeln!(s, "case fillrrect\nd2target 16 16\nd2aa aliased\nd2rrect 0 1 1 15 11 4 3\nd2fillg 0 255 255 255 255\nd2read\nend");
}

/// a float with a fractional part: a quarter step plus random low bits
fn frac(r: &mut XorShift32, lo: i64, hi: i64) -> String {
    fmt(r.range(lo * 64, hi * 64) as f64 / 64.0 + (r.next() % 1000) as f64 / 64000.0)
}

fn arith(s: &mut String, r: &mut XorShift32) {
    // the arithmetic of areas: polygons with fractional corners, one figure, either fill mode
    for i in 0..40 {
        let n = r.range(3, 7);
        let pts: Vec<String> = (0..n).map(|_| format!("{} {}", frac(r, -8, 8), frac(r, -8, 8))).collect();
        let mode = if i % 2 == 0 { "alt" } else { "wind" };
        writeln!(s, "case parea-{i}\nd2path 0 {mode} m {} z\nd2area 0\nd2tess 0\nend", pts.join(" "));
    }
    // curved areas at several tolerances, and their tessellation; lengths at other tolerances
    for i in 0..30 {
        let g = if i % 2 == 0 {
            format!("d2ell 0 {} {} {} {}", frac(r, -10, 10), frac(r, -10, 10), frac(r, 0, 8), frac(r, 0, 8))
        } else {
            format!("d2rrect 0 {} {} {} {} {} {}", frac(r, -10, 0), frac(r, -10, 0), frac(r, 1, 10), frac(r, 1, 10), frac(r, 0, 4), frac(r, 0, 4))
        };
        writeln!(s, "case carea-{i}\n{g}\nd2area 0\nd2area 0 1\nd2area 0 0.05\nd2tess 0\nd2len 0 1\nd2len 0 0\nend");
    }
    // bounds of curves through a transform
    for i in 0..30 {
        let g = if i % 2 == 0 {
            format!("d2ell 0 {} {} {} {}", frac(r, -10, 10), frac(r, -10, 10), frac(r, 0, 8), frac(r, 0, 8))
        } else {
            format!("d2rrect 0 {} {} {} {} {} {}", frac(r, -10, 0), frac(r, -10, 0), frac(r, 1, 10), frac(r, 1, 10), frac(r, 0, 4), frac(r, 0, 4))
        };
        let m: Vec<String> = (0..6).map(|_| frac(r, -3, 3)).collect();
        writeln!(s, "case cbounds-{i}\n{g}\nd2bounds 0 {}\nd2bounds 0 1 0 0 1 {} {}\nd2bounds 0 {} 0 0 {} 0 0\nend", m.join(" "), m[4], m[5], m[0], m[3]);
    }
    // curves filled through a transform (aliased)
    for (i, m) in ["0.7071 0.7071 -0.7071 0.7071 8 0", "1.5 0 0 0.5 1 2", "1 0.5 0 1 0 -2"].iter().enumerate() {
        writeln!(s, "case fillell-xf-{i}\nd2target 16 16\nd2aa aliased\nd2xform {m}\nd2ell 0 6 6 5 3\nd2fillg 0 255 255 255 255\nd2rrect 1 1 1 12 9 3 2\nd2fillg 1 0 0 255 128\nd2read\nend");
    }
}

/// a premultiplied B G R A bitmap of w x h as hex
fn bitmap(r: &mut XorShift32, w: i64, h: i64) -> String {
    let mut o = String::new();
    for _ in 0..w * h {
        let any = r.range(1, 255);
        let a = *r.pick(&[0, 255, 255, 128, any]);
        for _ in 0..3 {
            write!(o, "{:02x}", r.range(0, a + 1).min(255));
        }
        write!(o, "{a:02x}");
    }
    o
}

fn target(s: &mut String, r: &mut XorShift32) {
    // the target's state: size, dpi, format, mode, transform, tags; a state block; flush
    writeln!(s, "case info-0\nd2target 8 4\nd2info\nd2dpi 192 48\nd2info\nd2dpi 0 0\nd2info\nend");
    writeln!(s, "case info-1\nd2target 8 4\nd2aa aliased\nd2xform 1 2 3 4 5 6\nd2tags 5 6\nd2info\nd2flush\nd2info\nend");
    writeln!(s, "case state-0\nd2target 8 4\nd2xform 2 0 0 2 1 1\nd2aa aliased\nd2tags 3 4\nd2state save\nd2xform 1 0 0 1 0 0\nd2aa per\nd2tags 0 0\nd2info\nd2state restore\nd2info\nend");
    writeln!(s, "case state-1\nd2target 8 4\nd2state restore\nd2info\nend");
    // axis-aligned clips on fractions, both modes
    for i in 0..12 {
        let e = |r: &mut XorShift32, lo: i64, hi: i64| fmt(r.range(lo * 8, hi * 8) as f64 / 8.0);
        let mode = if i % 2 == 0 { "aliased" } else { "per" };
        writeln!(s, "case clip-{i}\nd2target 16 4\nd2clear {}\nd2clip {} {} {} {} {mode}\nd2fill 0 0 16 4 {}\nd2unclip\nd2read\nend", color(r, true), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), color(r, i % 3 == 0));
    }
    writeln!(s, "case clip-xf\nd2target 16 8\nd2xform 0.7071 0.7071 -0.7071 0.7071 8 0\nd2clip 1 1 6 6 aliased\nd2fill 0 0 16 16 255 255 255 255\nd2unclip\nd2read\nend");
    // brush opacity
    for i in 0..12 {
        let o = *r.pick(&["0.5", "0.25", "0.7", "0.3333", "0", "1", "0.01", "0.99"]);
        let aa = if i % 2 == 0 { "d2aa aliased\n" } else { "" };
        writeln!(s, "case fillo-{i}\nd2target 16 2\n{aa}d2clear {}\nd2fillo {} 0 {} 2 {} {o}\nd2read\nend", color(r, i % 4 != 3), fmt(r.range(0, 32) as f64 / 4.0), fmt(8.0 + r.range(0, 32) as f64 / 4.0), color(r, i % 3 == 0));
    }
    // gradients: linear and radial, either gamma, every extend mode, 1 to 4 stops in any order
    for i in 0..24 {
        let lin = i < 14;
        let gamma = if i % 2 == 0 { "22" } else { "10" };
        let ext = ["clamp", "wrap", "mirror"][i % 3];
        let n = r.range(1, 5);
        let stops: Vec<String> = (0..n).map(|_| format!("{} {}", fmt(r.range(-2, 10) as f64 / 8.0), { let o = r.chance(2); color(r, o) })).collect();
        let g = if lin {
            format!("lin {gamma} {ext} {} {} {} {}", q(r, 16), q(r, 4), q(r, 16), q(r, 4))
        } else {
            format!("rad {gamma} {ext} {} {} {} {} {} {}", q(r, 16), q(r, 4), fmt(r.range(-8, 8) as f64 / 4.0), fmt(r.range(-4, 4) as f64 / 4.0), fmt(r.range(1, 40) as f64 / 4.0), fmt(r.range(1, 16) as f64 / 4.0))
        };
        writeln!(s, "case grad-{i}\nd2target 16 4\nd2aa aliased\nd2grad 0 0 16 4 {g} {n} {}\nd2read\nend", stops.join(" "));
    }
    // bitmaps drawn and as brushes
    for i in 0..12 {
        let (w, h) = (r.range(1, 5), r.range(1, 5));
        let mode = if i % 2 == 0 { "nearest" } else { "linear" };
        let o = *r.pick(&["1", "0.5", "0.25"]);
        let src = if i % 3 == 0 { format!(" 0.5 0 {} {}", w, fmt(h as f64 - 0.5)) } else { String::new() };
        writeln!(s, "case bmp-{i}\nd2target 16 8\nd2clear {}\nd2bmp 0 {w} {h} {}\nd2drawbmp 0 {} {} {} {} {o} {mode}{src}\nd2read\nend", color(r, true), bitmap(r, w, h), q(r, 4), q(r, 2), fmt(4.0 + r.range(0, 48) as f64 / 4.0), fmt(2.0 + r.range(0, 24) as f64 / 4.0));
    }
    for i in 0..9 {
        let (w, h) = (r.range(1, 4), r.range(1, 4));
        let ex = ["clamp", "wrap", "mirror"];
        let m = if i % 2 == 1 { " 1.5 0 0 1.5 0.5 0" } else { "" };
        writeln!(s, "case bbrush-{i}\nd2target 16 8\nd2aa aliased\nd2bmp 0 {w} {h} {}\nd2bbrush 0 0 0 16 8 {} {} {}{m}\nd2read\nend", bitmap(r, w, h), ex[i % 3], ex[(i / 3) % 3], if i % 4 < 2 { "nearest" } else { "linear" });
    }
    // strokes: lines, rectangles and geometries, aliased and antialiased
    for i in 0..16 {
        let aa = if i % 2 == 0 { "d2aa aliased\n" } else { "" };
        let w = *r.pick(&["1", "2", "0.5", "3", "1.5"]);
        let op = match i % 4 {
            0 | 1 => format!("d2line {} {} {} {} {w} 255 255 255 255", q(r, 16), q(r, 12), q(r, 16), q(r, 12)),
            2 => format!("d2drawrect {} {} {} {} {w} 255 255 255 255", q(r, 6), q(r, 4), fmt(8.0 + r.range(0, 32) as f64 / 4.0), fmt(6.0 + r.range(0, 20) as f64 / 4.0)),
            _ => format!("d2ell 0 8 6 {} {}\nd2drawg 0 {w} 255 255 255 255", fmt(2.0 + r.range(0, 20) as f64 / 4.0), fmt(2.0 + r.range(0, 12) as f64 / 4.0)),
        };
        writeln!(s, "case stroke-{i}\nd2target 16 12\n{aa}{op}\nd2read\nend");
    }
    // layers: opacity, content bounds, geometric masks
    for i in 0..6 {
        let mask = if i % 2 == 1 { " 0" } else { "" };
        writeln!(s, "case layer-{i}\nd2target 16 4\nd2aa aliased\nd2path 0 alt m 0 0 12 0 4 4 z\nd2clear {}\nd2layer {} 0 {} 4 {}{mask}\nd2fill 0 0 16 4 {}\nd2poplayer\nd2read\nend", color(r, true), r.range(0, 6), r.range(8, 17), *r.pick(&["1", "0.5", "0.25"]), color(r, i % 3 == 0));
    }
}

/// more of the target's state, clips and opacity: odd dpi, state blocks used twice, clips that
/// Clear goes through, two clips at once, a turned antialiased clip, opacities past 0..1
fn target_more(s: &mut String, r: &mut XorShift32) {
    for (i, d) in ["72 100", "0 96", "96 0", "-96 96", "1 1", "300 7", "96 -1", "-1 -1"].iter().enumerate() {
        writeln!(s, "case dpi-{i}\nd2target 8 4\nd2dpi 120 144\nd2dpi {d}\nd2info\nend");
    }
    writeln!(s, "case state-2\nd2target 8 4\nd2tags 1 2\nd2state save\nd2tags 3 4\nd2xform 1 0 0 1 2 3\nd2state save\nd2tags 5 6\nd2aa aliased\nd2state restore\nd2info\nd2state restore\nd2info\nend");
    let e = |r: &mut XorShift32, lo: i64, hi: i64| fmt(r.range(lo * 8, hi * 8) as f64 / 8.0);
    for i in 0..4 {
        let mode = if i % 2 == 0 { "aliased" } else { "per" };
        writeln!(s, "case clipclear-{i}\nd2target 16 4\nd2clear {}\nd2clip {} {} {} {} {mode}\nd2clear {}\nd2unclip\nd2read\nend", color(r, true), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), color(r, i < 2));
    }
    for i in 0..8 {
        let m1 = if i % 2 == 0 { "aliased" } else { "per" };
        let m2 = if i % 4 < 2 { "per" } else { "aliased" };
        writeln!(s, "case clipnest-{i}\nd2target 16 4\nd2clear {}\nd2clip {} {} {} {} {m1}\nd2clip {} {} {} {} {m2}\nd2fill 0 0 16 4 {}\nd2unclip\nd2unclip\nd2read\nend", color(r, true), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), color(r, i % 3 == 0));
    }
    for i in 0..6 {
        writeln!(s, "case clipaa-{i}\nd2target 16 4\nd2clear {}\nd2clip {} {} {} {} per\nd2fill {} {} {} {} {}\nd2unclip\nd2read\nend", color(r, true), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), e(r, 0, 8), e(r, 0, 2), e(r, 8, 16), e(r, 2, 4), color(r, i % 2 == 0));
    }
    writeln!(s, "case clip-unbal\nd2target 8 2\nd2clip 0 0 4 2 aliased\nd2fill 0 0 8 2 255 0 0 255\nd2read\nd2fill 0 0 8 2 0 255 0 255\nd2read\nend");
    writeln!(s, "case clip-pop0\nd2target 8 2\nd2fill 0 0 8 2 255 0 0 255\nd2unclip\nd2fill 0 0 8 2 0 255 0 255\nd2read\nd2fill 0 0 4 2 0 0 255 255\nd2read\nend");
    writeln!(s, "case clip-xfper\nd2target 16 8\nd2xform 0.7071 0.7071 -0.7071 0.7071 8 0\nd2clip 1.3 1 6 6 per\nd2fill 0 0 16 16 255 255 255 255\nd2unclip\nd2read\nend");
    writeln!(s, "case clip-xfscale\nd2target 16 8\nd2xform 1.5 0 0 0.75 0.25 0.5\nd2clip 1 1 6 6 per\nd2fill 0 0 16 16 200 100 50 255\nd2unclip\nd2read\nend");
    for (i, o) in ["1.5", "-0.5", "2", "0.5", "0.7", "0.25"].iter().enumerate() {
        writeln!(s, "case fillo-x{i}\nd2target 16 2\nd2clear {}\nd2fillo {} 0.25 {} 1.625 {} {o}\nd2read\nend", color(r, true), frac(r, 0, 4), frac(r, 8, 16), color(r, i % 2 == 0));
    }
    // layers: fractional content bounds, masks with any coverage, nested, under a clip, through
    // a transform, opacities past 0..1, left pushed, popped on the wrong kind
    for i in 0..10 {
        let mask = if i % 2 == 0 { format!("d2path 0 alt m {} {} {} {} {} {} z\n", frac(r, 0, 6), frac(r, 0, 2), frac(r, 10, 16), frac(r, 0, 2), frac(r, 4, 12), frac(r, 2, 4)) } else { "d2rect 0 2.5 0.5 12.5 3.5\n".into() };
        let o = *r.pick(&["1", "0.5", "0.3", "0.75", "1.5", "-0.5", "0.01"]);
        writeln!(s, "case layerx-{i}\nd2target 16 4\n{mask}d2clear {}\nd2layer {} {} {} {} {o} 0\nd2fill {} 0 {} 4 {}\nd2poplayer\nd2read\nend", color(r, true), frac(r, 0, 6), frac(r, 0, 2), frac(r, 8, 16), frac(r, 2, 4), frac(r, 0, 4), frac(r, 10, 16), color(r, i % 3 == 0));
    }
    writeln!(s, "case layer-nest\nd2target 16 4\nd2clear 10 200 30 255\nd2layer 0 0 12 4 0.5\nd2fill 0 0 16 4 200 10 30 255\nd2layer 4 0 16 4 0.5\nd2fill 0 0 16 4 30 10 200 200\nd2poplayer\nd2poplayer\nd2read\nend");
    writeln!(s, "case layer-clip\nd2target 16 4\nd2clear 10 200 30 255\nd2clip 2.5 0.25 13.375 3.5 per\nd2layer 0 0 12 4 0.75\nd2fill 0 0 16 4 200 10 30 255\nd2poplayer\nd2unclip\nd2read\nend");
    writeln!(s, "case layer-inclip\nd2target 16 4\nd2clear 10 200 30 255\nd2layer 0 0 12 4 0.75\nd2clip 2.5 0.25 13.375 3.5 per\nd2fill 0 0 16 4 200 10 30 255\nd2unclip\nd2poplayer\nd2read\nend");
    writeln!(s, "case layer-xf\nd2target 16 8\nd2path 0 alt m 0 0 6 0 2 3 z\nd2xform 2 0 0 2 0.5 0.25\nd2clear 10 200 30 255\nd2layer 0.5 0 6.25 3 0.5 0\nd2fill 0 0 8 4 200 10 30 255\nd2poplayer\nd2read\nend");
    writeln!(s, "case layer-unbal\nd2target 8 2\nd2clear 10 200 30 255\nd2layer 0 0 4 2 1\nd2fill 0 0 8 2 255 0 0 255\nd2read\nd2fill 0 0 8 2 0 0 255 255\nd2read\nend");
    writeln!(s, "case layer-pop0\nd2target 8 2\nd2poplayer\nd2fill 0 0 8 2 255 0 0 255\nd2read\nend");
    writeln!(s, "case layer-pop1\nd2target 8 2\nd2clip 0 0 4 2 aliased\nd2poplayer\nd2fill 0 0 8 2 255 0 0 255\nd2read\nend");
    writeln!(s, "case layer-pop2\nd2target 8 2\nd2layer 0 0 4 2 1\nd2unclip\nd2fill 0 0 8 2 255 0 0 255\nd2read\nend");
    writeln!(s, "case layer-pop3\nd2target 8 2\nd2fill 0 0 8 2 0 255 0 255\nd2clip 0 0 4 2 aliased\nd2poplayer\nd2fill 0 0 8 2 255 0 0 255\nd2read\nd2fill 0 0 2 2 0 0 255 255\nd2read\nend");
    writeln!(s, "case layer-pop4\nd2target 8 2\nd2fill 0 0 8 2 0 255 0 255\nd2poplayer\nd2fill 0 0 8 2 255 0 0 255\nd2read\nend");
    writeln!(s, "case layer-pop5\nd2target 8 2\nd2fill 0 0 8 2 0 255 0 255\nd2layer 0 0 4 2 1\nd2unclip\nd2read\nd2fill 0 0 2 2 0 0 255 255\nd2read\nend");
    writeln!(s, "case layer-nest2\nd2target 8 2\nd2fill 0 0 8 2 0 255 0 255\nd2layer 0 0 4 2 1\nd2fill 0 0 8 2 255 0 0 255\nd2poplayer\nd2layer 4 0 8 2 0.5\nd2fill 0 0 8 2 0 0 255 255\nd2poplayer\nd2read\nend");
}

/// gradients made to read the interpolation off: one row of 64 pixels, two stops at 0 and 1,
/// spans from 16 to 16384 pixels and starts far before the row, both gammas, a radial one
fn gradfit(s: &mut String) {
    let pairs = ["0 0 0 0 255 1 255 255 255 255", "0 255 0 0 255 1 0 0 255 255", "0 0 0 0 0 1 255 255 255 255", "0 10 200 90 255 1 240 30 160 128"];
    let spans = [(0.0, 64.0), (0.0, 16.0), (0.0, 256.0), (0.0, 1024.0), (0.0, 16384.0), (-192.0, 64.0), (32.0, 0.0), (0.25, 63.5)];
    let mut i = 0;
    for gamma in ["22", "10"] {
        for (pi, pair) in pairs.iter().enumerate() {
            for (si, &(a, b)) in spans.iter().enumerate() {
                if pi > 1 && si > 2 {
                    continue;
                }
                writeln!(s, "case gradfit-{i}\nd2target 64 1\nd2aa aliased\nd2grad 0 0 64 1 lin {gamma} clamp {} 0 {} 0 2 {pair}\nd2read\nend", fmt(a), fmt(b));
                i += 1;
            }
        }
        for ext in ["clamp", "wrap", "mirror"] {
            writeln!(s, "case gradfit-{i}\nd2target 64 1\nd2aa aliased\nd2grad 0 0 64 1 lin {gamma} {ext} 8 0 24 0 2 {}\nd2read\nend", pairs[0]);
            i += 1;
            writeln!(s, "case gradfit-{i}\nd2target 64 1\nd2aa aliased\nd2grad 0 0 64 1 rad {gamma} {ext} 0 0 0 0 48 0.5 2 {}\nd2read\nend", pairs[1]);
            i += 1;
        }
    }
}

/// DrawBitmap aliased, through world transforms, past the source's edges; bitmap brushes
/// through world transforms and with odd brush transforms
fn bitmaps_more(s: &mut String, r: &mut XorShift32) {
    for i in 0..12 {
        let (w, h) = (r.range(1, 5), r.range(1, 5));
        let mode = if i % 2 == 0 { "nearest" } else { "linear" };
        let aa = if i % 4 < 2 { "d2aa aliased\n" } else { "" };
        let xf = ["", "d2xform 1.5 0 0 0.75 1.25 0.5\n", "d2xform 0.8 0.6 -0.6 0.8 6 0\n", "d2xform -1 0 0 1 16 0\n"][i % 4];
        let src = if i % 3 == 0 { format!(" -0.5 0.25 {} {}", fmt(w as f64 + 0.75), fmt(h as f64 - 0.25)) } else { String::new() };
        writeln!(s, "case bmpx-{i}\nd2target 16 8\n{aa}{xf}d2clear {}\nd2bmp 0 {w} {h} {}\nd2drawbmp 0 {} {} {} {} 1 {mode}{src}\nd2read\nend", color(r, true), bitmap(r, w, h), frac(r, 0, 4), frac(r, 0, 2), frac(r, 6, 14), frac(r, 4, 8));
    }
    for i in 0..8 {
        let (w, h) = (r.range(1, 5), r.range(1, 5));
        let mode = if i % 2 == 0 { "nearest" } else { "linear" };
        let ex = ["clamp", "wrap", "mirror"][i % 3];
        let xf = ["", "d2xform 1 0 0 1 0.5 0.25\n", "d2xform 0.8 0.6 -0.6 0.8 6 0\n", "d2xform 2 0 0 2 0 0\n"][i % 4];
        let m = ["0.7 0.2 -0.3 1.1 0.25 0.5", "-1 0 0 1 3 0", "2.5 0 0 0.5 0 0", "1 0.5 0 1 -2 1"][(i / 2) % 4];
        writeln!(s, "case bbrushx-{i}\nd2target 16 8\nd2aa aliased\n{xf}d2bmp 0 {w} {h} {}\nd2bbrush 0 0 0 16 8 {ex} {ex} {mode} {m}\nd2read\nend", bitmap(r, w, h));
    }
}

/// layers made to read the composition off: one row of 64 pixels, a mask whose coverage runs
/// 0 to 64 samples along the row, opaque and translucent content, opacities across 0..1
fn layerfit(s: &mut String) {
    let mut i = 0;
    for o in ["0.1", "0.2", "0.3", "0.4", "0.5", "0.6", "0.7", "0.8", "0.9", "1", "0.3333", "0.05"] {
        for c in ["200 10 30 255", "60 220 140 128", "255 255 255 64"] {
            writeln!(s, "case layerfit-{i}\nd2target 64 2\nd2path 0 alt m 0 0 64 0 64 2 0 2 z\nd2path 1 alt m 0 0 64 2 0 2 z\nd2clear 30 120 200 255\nd2layer 0 0 64 2 {o} 1\nd2fill 0 0 64 2 {c}\nd2poplayer\nd2read\nend");
            i += 1;
        }
    }
    for o in ["0.3", "0.7"] {
        writeln!(s, "case layerfit-{i}\nd2target 64 2\nd2clear 30 120 200 255\nd2layer 0.25 0.125 63.875 1.875 {o}\nd2fill 0 0 64 2 60 220 140 128\nd2poplayer\nd2read\nend");
        i += 1;
    }
}

/// DrawBitmap source rectangles past or outside the bitmap, and bitmap brush mappings that
/// tell which axis a linear sample lerps along first
fn bitmapfit(s: &mut String, r: &mut XorShift32) {
    let srcs = ["-1.5 0.5 2.5 2.5", "0.5 -1.25 2.5 2.75", "0.25 0.5 4.5 2.5", "0.5 0.25 2.5 5.25", "-1 -1 4 4", "-0.75 0.5 2.25 2.5", "5 5 7 7", "2.5 0.5 1 2.5", "0 0 3 -1"];
    for (i, src) in srcs.iter().enumerate() {
        let mode = if i % 2 == 0 { "nearest" } else { "linear" };
        writeln!(s, "case bmpsrc-{i}\nd2target 16 8\nd2aa aliased\nd2clear {}\nd2bmp 0 3 3 {}\nd2drawbmp 0 0.5 0.75 13.25 7.5 1 {mode} {src}\nd2read\nend", color(r, true), bitmap(r, 3, 3));
    }
    let ms = ["0 1 -1 0 8 0", "0 -1 1 0 0 8", "1 0.3 0 1 0 0", "1 0 0.3 1 0 0", "1.3 0 0 0.7 0.1 0.2", "-1.3 0 0 0.7 9 0.2", "1.3 0.001 0 0.7 0.1 0.2", "0.6 0.8 -0.8 0.6 4 1", "1.25 0 0 -1.5 0 7", "0 1.3 0.7 0 0.25 0.5"];
    for (i, m) in ms.iter().enumerate() {
        let ex = ["wrap", "mirror", "clamp"][i % 3];
        writeln!(s, "case bbrushfit-{i}\nd2target 16 8\nd2aa aliased\nd2bmp 0 3 3 {}\nd2bbrush 0 0 0 16 8 {ex} {ex} linear {m}\nd2read\nend", bitmap(r, 3, 3));
    }
    for (i, xf) in ["0 1 -1 0 16 0", "1 0.25 0 1 0 0", "1.5 0 0 1.25 0 0"].iter().enumerate() {
        writeln!(s, "case bbrushfit-xf{i}\nd2target 16 8\nd2aa aliased\nd2xform {xf}\nd2bmp 0 3 3 {}\nd2bbrush 0 -8 -8 24 24 wrap wrap linear 1.3 0 0 0.7 0.1 0.2\nd2read\nend", bitmap(r, 3, 3));
    }
}

/// antialiased clips made to read the blend off: pixel (2a + 1, 2b + 1) under a clip whose
/// right and bottom edges cut a/8 and b/8 into it (a b of 64 samples), translucent and opaque
/// fills; rotated clips (their bounding box) with the box's edges sweeping across a pixel
fn clipfit(s: &mut String) {
    for (i, c) in ["44 107 187 64", "200 30 90 128", "20 240 120 200", "250 250 250 255", "90 60 30 10"].iter().enumerate() {
        writeln!(s, "case clipfit-{i}\nd2target 18 18\nd2clear 37 125 30 255");
        for a in 0..9 {
            for b in 0..9 {
                let (x, y) = (2 * a, 2 * b);
                writeln!(s, "d2clip {x}.5 {y}.5 {} {} per\nd2fill 0 0 18 18 {c}\nd2unclip", fmt(x as f64 + 1.0 + a as f64 / 8.0), fmt(y as f64 + 1.0 + b as f64 / 8.0));
            }
        }
        writeln!(s, "d2read\nend");
    }
    for i in 0..32 {
        let tx = fmt(8.0 + i as f64 / 32.0);
        let (m, name) = if i % 2 == 0 { ("0.7071 0.7071 -0.7071 0.7071", "45") } else { ("0.866 0.5 -0.5 0.866", "30") };
        writeln!(s, "case clipxf-{i}\nd2target 16 8\nd2xform {m} {tx} 0\nd2clip 1.3 1 6 6 per\nd2xform 1 0 0 1 0 0\nd2fill 0 0 16 8 255 255 255 255\nd2unclip\nd2read\nend");
        let _ = name;
    }
}

/// fills with a vertex past 2^23 pixels, where the 28.4 rasterizer has no room (D2D.FILL.HUGE)
fn huge(s: &mut String) {
    for (i, v) in ["9000000", "-9000000", "16777216", "1e9"].iter().enumerate() {
        let aa = if i % 2 == 0 { "" } else { "d2aa aliased\n" };
        writeln!(s, "case fillhuge-{i}\nd2target 8 4\n{aa}d2clear 10 20 30 255\nd2path 0 alt m 1 1 {v} 2 1 3 z\nd2fillg 0 200 100 50 255\nd2fill 0 0 {v} 2 50 60 70 255\nd2read\nend");
    }
}

/// linear bitmap brushes under world transforms that shear, turn or swap the axes, with brush
/// matrices that keep or mix them: which way the four texels are weighed
fn brushworld(s: &mut String, r: &mut XorShift32) {
    let worlds = ["1 0.25 0 1 0 0", "1 0 0.25 1 0 0", "0.866 0.5 -0.5 0.866 4 -2", "0 1.5 -1 0 16 0", "1.2 0.3 0.1 0.9 0 0", "-1 0 0 1 16 0", "1 0.001 0 1 0 0"];
    let brushes = ["1.3 0 0 0.7 0.1 0.2", "0.9 0.4 -0.2 1.1 0.3 0"];
    let mut i = 0;
    for w in worlds {
        for b in brushes {
            writeln!(s, "case bbrushwx-{i}\nd2target 16 8\nd2aa aliased\nd2xform {w}\nd2bmp 0 3 3 {}\nd2bbrush 0 -16 -16 32 32 wrap wrap linear {b}\nd2read\nend", bitmap(r, 3, 3));
            i += 1;
        }
    }
}

/// DrawGeometry on straight closed figures (miter joins, sharp corners past the miter limit),
/// open figures, aliased and antialiased, through transforms
fn strokefit(s: &mut String, r: &mut XorShift32) {
    let paths = ["m 2.3 1.7 13.6 2.2 7.9 10.4 z", "m 1.5 1.5 14.5 1.5 14.5 10.5 1.5 10.5 z", "m 2 6 14 5 2 7 z", "m 3.25 2.5 12.75 9.5 12.75 2.5 3.25 9.5 z", "m 2.1 2.2 13.7 3.1 9.2 9.8 5.4 6.1 z m 6 4 9 4 8 7 z", "m 2.5 9 8 2 13.5 9 o", "m 2 2 14 2 2 10 14 10 o"];
    let mut i = 0;
    for p in paths {
        for (w, aa, xf) in [("1", "", ""), ("2.5", "d2aa aliased\n", ""), ("0.75", "", "d2xform 0.9 0.3 -0.3 0.9 2 -1\n"), ("4", "", "")] {
            writeln!(s, "case strokefit-{i}\nd2target 16 12\n{aa}{xf}d2clear {}\nd2path 0 alt {p}\nd2drawg 0 {w} {}\nd2read\nend", color(r, true), color(r, false));
            i += 1;
        }
    }
}

/// bounds of curves through a transform, pulled apart: ellipses at the origin turned only,
/// moved off it, the transform moving them, and general ones with several transforms each
fn xfbounds(s: &mut String, r: &mut XorShift32) {
    let rot = |r: &mut XorShift32| {
        let a = r.range(0, 6283) as f64 / 1000.0;
        let k = r.range(32, 160) as f64 / 64.0;
        // The native matrix stores FLOATs. Drop host libm's low f64 bits before
        // printing, so Windows and Linux generate the same case file.
        let coeff = |v| f64::from(crate::model::gppath::to32(crate::model::gdiplus::Round::Nearest, v));
        let c = coeff(a.cos() * k);
        let s = coeff(a.sin() * k);
        [c, s, -s, c]
    };
    let dy = |r: &mut XorShift32, lo: i64, hi: i64| fmt(r.range(lo * 16, hi * 16) as f64 / 16.0);
    for i in 0..24 {
        let g = if i % 2 == 0 { format!("d2ell 0 0 0 {} {}", dy(r, 1, 6), dy(r, 1, 6)) } else { format!("d2rrect 0 {} {} {} {} {} {}", dy(r, -6, -1), dy(r, -6, -1), dy(r, 1, 6), dy(r, 1, 6), dy(r, 0, 3), dy(r, 0, 3)) };
        let mut o = format!("case xfb-rot-{i}\n{g}\n");
        for _ in 0..4 {
            let m = rot(r);
            o += &format!("d2bounds 0 {} {} {} {} 0 0\n", fmt(m[0]), fmt(m[1]), fmt(m[2]), fmt(m[3]));
        }
        for _ in 0..4 {
            let m = rot(r);
            o += &format!("d2bounds 0 {} {} {} {} {} {}\n", fmt(m[0]), fmt(m[1]), fmt(m[2]), fmt(m[3]), frac(r, -6, 6), frac(r, -6, 6));
        }
        writeln!(s, "{o}end");
    }
    for i in 0..24 {
        let g = if i % 2 == 0 { format!("d2ell 0 {} {} {} {}", frac(r, -10, 10), frac(r, -10, 10), frac(r, 0, 8), frac(r, 0, 8)) } else { format!("d2rrect 0 {} {} {} {} {} {}", frac(r, -10, 0), frac(r, -10, 0), frac(r, 1, 10), frac(r, 1, 10), frac(r, 0, 4), frac(r, 0, 4)) };
        let mut o = format!("case xfb-ctr-{i}\n{g}\n");
        for _ in 0..4 {
            let m = rot(r);
            o += &format!("d2bounds 0 {} {} {} {} 0 0\n", fmt(m[0]), fmt(m[1]), fmt(m[2]), fmt(m[3]));
        }
        for _ in 0..4 {
            let m: Vec<String> = (0..6).map(|_| frac(r, -3, 3)).collect();
            o += &format!("d2bounds 0 {}\n", m.join(" "));
        }
        writeln!(s, "{o}end");
    }
}

/// DrawBitmap source rectangles that are empty, turned round, or outside the bitmap, one way
/// at a time, with both interpolations; a 4 x 3 bitmap of opaque texels
fn bmpsrcfit(s: &mut String, r: &mut XorShift32) {
    let srcs = [
        "5 5 7 7", "-3 -3 -1 -1", "5 1 7 2", "-3 1 -1 2", "1 5 2 7", "1 -3 2 -1", "3 1 1 2", "1 2 3 1", "3 2 1 1", "1 1 1 2", "1 1 3 1", "2 2 2 2",
        "4 1 6 2", "1 3 2 5", "0 0 4 0", "0 3 4 3", "4 0 4 3", "0 0 0 3", "4.5 0 5 3", "0 -2 4 -0.5", "-1 0.5 0.25 2.5", "3.75 0.5 6 2.5", "0 0 4 -1", "0 0 -1 3",
    ];
    let mut i = 0;
    for mode in ["nearest", "linear"] {
        for src in srcs {
            let px: String = (0..12).map(|_| format!("{:02x}{:02x}{:02x}ff", r.byte(), r.byte(), r.byte())).collect();
            writeln!(s, "case bmpsrcfit-{i}\nd2target 16 8\nd2aa aliased\nd2clear {}\nd2bmp 0 4 3 {px}\nd2drawbmp 0 0.5 0.75 13.25 7.5 1 {mode} {src}\nd2read\nend", color(r, true));
            i += 1;
        }
    }
}

/// the geometries xfb-ctr-* miss, through transforms of few rounding steps: shears and
/// turns by dyadic factors, the same with a translation
fn xfbounds_more(s: &mut String) {
    let geoms = [
        "d2ell 0 0.4874375 -3.896546875 7.037171875 4.902140625",
        "d2ell 0 4.292125 -5.831296875 5.793234375 7.86309375",
        "d2ell 0 -3.222703125 4.936015625 5.97175 5.959046875",
        "d2ell 0 -0.48653125 -5.624578125 6.990609375 6.106734375",
        "d2ell 0 8.78415625 -1.774265625 4.11884375 7.5035",
        "d2ell 0 0.5 -3.75 7 5",
    ];
    let ms = ["1 1 -1 1 0 0", "1 0.5 -0.5 1 0 0", "0.5 -1 1 0.5 0 0", "1 0 0.5 1 0 0", "1 0.25 0 1 0 0", "1 1 -1 1 3 -2", "2 1 -1 2 0.5 0.25", "0.75 -0.5 0.25 1.5 0 0"];
    for (i, g) in geoms.iter().enumerate() {
        let mut o = format!("case xfbm-{i}\n{g}\n");
        for m in ms {
            o += &format!("d2bounds 0 {m}\n");
        }
        writeln!(s, "{o}end");
    }
}

/// DrawGeometry of random straight paths: three to five corners, closed or open, sharp turns
/// and thin figures among them, widths from a quarter pixel to five, either antialiasing,
/// sometimes through a world transform
fn strokemore(s: &mut String, r: &mut XorShift32) {
    for i in 0..48 {
        let n = r.range(3, 6);
        let pts: Vec<String> = (0..n).map(|_| format!("{} {}", frac(r, 1, 15), frac(r, 1, 11))).collect();
        let end = if i % 3 == 2 { "o" } else { "z" };
        let w = fmt(r.range(1, 21) as f64 / 4.0);
        let aa = if i % 2 == 1 { "d2aa aliased\n" } else { "" };
        let xf = if i % 5 == 4 { "d2xform 0.8 0.6 -0.6 0.8 4 -2\n" } else { "" };
        writeln!(s, "case strokemore-{i}\nd2target 16 12\n{aa}{xf}d2clear {}\nd2path 0 alt m {} {end}\nd2drawg 0 {w} {}\nd2read\nend", color(r, true), pts.join(" "), color(r, false));
    }
}

/// more source rectangles turned round by the cut, on each side, both interpolations, to pull
/// apart which ones linear interpolation drops
fn bmpsrcmore(s: &mut String, r: &mut XorShift32) {
    let srcs = ["-3 1 -0.5 2", "-0.5 1 -0.25 2", "-3 -3 -0.5 2", "4.25 1 5 2", "4.5 4 6 5", "1 -3 2 -0.25", "1 -0.5 2 -0.25", "1 3.25 2 4", "-2 -2 -1 5", "-1 -1 6 -0.5", "6 -1 7 4", "0.5 4 3.5 5"];
    let mut i = 0;
    for mode in ["linear", "nearest"] {
        for src in srcs {
            let px: String = (0..12).map(|_| format!("{:02x}{:02x}{:02x}ff", r.byte(), r.byte(), r.byte())).collect();
            writeln!(s, "case bmpsrcmore-{i}\nd2target 16 8\nd2aa aliased\nd2clear {}\nd2bmp 0 4 3 {px}\nd2drawbmp 0 0.5 0.75 13.25 7.5 1 {mode} {src}\nd2read\nend", color(r, true));
            i += 1;
        }
    }
}

/// strokes wider than twice a side: small rectangles both ways round, thin triangles and open
/// figures with short segments, to show what the widener does where a join's inner side
/// passes the segment before it
fn strokewide(s: &mut String, r: &mut XorShift32) {
    for i in 0..36 {
        let (x, y) = (r.range(32, 72) as f64 / 8.0 + 1.0 / 64.0, r.range(24, 56) as f64 / 8.0 + 3.0 / 128.0);
        let pts = match i % 4 {
            0 | 1 => {
                let (w, h) = (r.range(1, 12) as f64 / 8.0, r.range(1, 12) as f64 / 8.0);
                let c = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
                let c: Vec<_> = if i % 4 == 0 { c.to_vec() } else { c.iter().rev().copied().collect() };
                c.iter().map(|p| format!("{} {}", fmt(p.0), fmt(p.1))).collect::<Vec<_>>().join(" ")
            }
            _ => (0..r.range(3, 4)).map(|_| format!("{} {}", frac(r, 3, 11), frac(r, 2, 8))).collect::<Vec<_>>().join(" "),
        };
        let end = if i % 4 == 3 { "o" } else { "z" };
        let w = fmt(r.range(4, 24) as f64 / 4.0);
        let aa = if i % 2 == 1 { "d2aa aliased\n" } else { "" };
        writeln!(s, "case strokewide-{i}\nd2target 16 12\n{aa}d2clear {}\nd2path 0 alt m {pts} {end}\nd2drawg 0 {w} {}\nd2read\nend", color(r, true), color(r, false));
    }
}

/// linear gradients moved by 1/32 of a pixel at a time: over 256 pixels every texel's
/// fraction in steps of 1/32, over 64 pixels the ties at texel halves one by one; red to blue
/// and black to white, both gammas, to map the sampled value against the position exactly
fn gradscan(s: &mut String) {
    let pairs = ["0 255 0 0 255 1 0 0 255 255", "0 0 0 0 255 1 255 255 255 255"];
    let mut i = 0;
    for gamma in ["22", "10"] {
        for pair in pairs {
            for span in [256.0, 64.0] {
                for m in 0..32 {
                    let d = f64::from(m) / 32.0;
                    writeln!(s, "case gradscan-{i}\nd2target 64 1\nd2aa aliased\nd2grad 0 0 64 1 lin {gamma} clamp {} 0 {} 0 2 {pair}\nd2read\nend", fmt(-d), fmt(span - d));
                    i += 1;
                }
            }
        }
    }
}

/// a figure with one side short against the width, turning through angles from sharp to
/// shallow at its ends: whether a join's inner lobe takes away where it passes the quads
fn strokelobe(s: &mut String, r: &mut XorShift32) {
    for i in 0..48 {
        let (x, y) = (7.0 + (r.range(0, 15) as f64) / 16.0, 5.0 + (r.range(0, 15) as f64) / 16.0);
        let short = (r.range(2, 8) as f64) / 8.0;
        let ang = |r: &mut XorShift32| (r.range(10, 170) as f64).to_radians();
        let (a1, a2) = (ang(r), ang(r));
        let (l1, l2) = ((r.range(20, 40) as f64) / 8.0, (r.range(20, 40) as f64) / 8.0);
        let p0 = (x - l1 * a1.cos(), y - l1 * a1.sin());
        let p1 = (x, y);
        let p2 = (x + short, y);
        let p3 = (x + short + l2 * a2.cos(), y - l2 * a2.sin());
        let pts = [p0, p1, p2, p3].iter().map(|p| format!("{} {}", fmt((p.0 * 64.0).round() / 64.0), fmt((p.1 * 64.0).round() / 64.0))).collect::<Vec<_>>().join(" ");
        let end = if i % 3 == 2 { "z" } else { "o" };
        let w = fmt((r.range(8, 24) as f64) / 4.0);
        let aa = if i % 2 == 1 { "d2aa aliased\n" } else { "" };
        writeln!(s, "case strokelobe-{i}\nd2target 16 12\n{aa}d2clear {}\nd2path 0 alt m {pts} {end}\nd2drawg 0 {w} {}\nd2read\nend", color(r, true), color(r, false));
    }
}

/// ellipses and rounded rectangles through CreateTransformedGeometry, bounds with and without a
/// world transform: the inner geometry's bounds through both transforms (D2D.XF, D2D.BOUNDS.ELLXF)
fn xfell(s: &mut String, r: &mut XorShift32) {
    for i in 0..16 {
        let q = |r: &mut XorShift32, a: i64, b: i64| fmt(r.range(a, b) as f64 / 8.0);
        let g = if i % 2 == 0 {
            format!("d2ell 0 {} {} {} {}", q(r, -64, 64), q(r, -64, 64), q(r, 4, 64), q(r, 4, 64))
        } else {
            format!("d2rrect 0 {} {} {} {} {} {}", q(r, -64, 0), q(r, -64, 0), q(r, 8, 64), q(r, 8, 64), q(r, 0, 24), q(r, 0, 24))
        };
        let m = |r: &mut XorShift32| Vec::<_>::from_fn(6, |_| q(r, -16, 16)).join(" ");
        let (m1, m2) = (m(r), m(r));
        writeln!(s, "case xfell-{i}\n{g}\nd2xf 1 0 {m1}\nd2bounds 1\nd2bounds 1 {m2}\nend");
    }
}

/// what the clean-room notes leave open, asked: crossing polygons past 16 DIP (ComputeArea's
/// 2^20 workspace no longer exact; Tessellate's 1/64 one still is), a stroke turning back on
/// itself (180 degrees), tight curves under a wide pen (the widener's refinement arcs, as lines and as cubics), and the special case of equal radii
fn probes(s: &mut String, r: &mut XorShift32) {
    for i in 0..8 {
        let n = r.range(4, 8);
        let pts: Vec<String> = (0..n).map(|_| format!("{} {}", frac(r, -400, 400), frac(r, -400, 400))).collect();
        let mode = if i % 2 == 0 { "alt" } else { "wind" };
        writeln!(s, "case pareabig-{i}\nd2path 0 {mode} m {} z\nd2area 0\nd2tess 0\nend", pts.join(" "));
    }
    for (i, w) in ["1", "2.5"].iter().enumerate() {
        let g = "d2path 0 alt m 2 6 12 6 5 6 o";
        writeln!(s, "case widenturn-{i}\n{g}\nd2widen 0 {w}\nend\ncase strketurn-{i}\nd2target 16 12\n{g}\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    for (i, (e, w)) in [("8 6 1.5 1", "4"), ("8 6 3 0.5", "3"), ("8 6 0.75 0.75", "2.5")].iter().enumerate() {
        writeln!(s, "case widentight-{i}\nd2ell 0 {e}\nd2widen 0 {w}\nend\ncase strketight-{i}\nd2target 16 12\nd2ell 0 {e}\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    // the refinement arcs as cubics (turns past the refinement, past 90 degrees too), asked
    for (i, (e, w)) in [("8 6 4 0.25", "6"), ("8 6 5 0.5", "8"), ("8 6 3 1", "5"), ("6 6 4 0.1", "1")].iter().enumerate() {
        writeln!(s, "case widenarc-{i}\nd2ell 0 {e}\nd2widen 0 {w}\nend\ncase strkarc-{i}\nd2target 16 12\nd2ell 0 {e}\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    // CCompactShape's special case of equal radii: an inner circle, a sharp inner rectangle
    for (i, (g, w)) in [("d2ell 0 8 6 3 3", "2"), ("d2ell 0 8 6 1 1", "1.5"), ("d2rrect 0 2 2 14 10 1 1", "3"), ("d2rrect 0 2 2 14 10 2 2", "1"), ("d2rrect 0 2 2 14 10 9 9", "2")].iter().enumerate() {
        writeln!(s, "case widenround-{i}\n{g}\nd2widen 0 {w}\nend");
    }
    // what is still open, asked: crossing figures past 2^31 area workspace units (D2D.AREA.CURVE),
    // an Outline past 16 DIP (D2D.SIMP), a stroke under a skew (an elliptic pen) and a rounded
    // rectangle's own stroke (D2D.RT)
    for (i, k) in ["3000", "5000"].iter().enumerate() {
        writeln!(s, "case pareahuge-{i}\nd2path 0 alt m -{k} -{k} {k} {k} {k} -{k} -{k} {k} z\nd2area 0\nend");
    }
    writeln!(s, "case outlinebig-0\nd2path 0 alt m -40 -40 40 40 40 -40 -40 40 z\nd2outline 0\nend");
    writeln!(s, "case strkskew-0\nd2target 16 12\nd2xform 1 0 0.5 1 0 0\nd2path 0 alt m 2 2 10 8 3 9 z\nd2drawg 0 1.5 255 255 255 255\nd2read\nend");
    writeln!(s, "case strkrrect-0\nd2target 16 12\nd2rrect 0 2 2 14 10 2 1\nd2drawg 0 1.5 255 255 255 255\nd2read\nend");
    for (i, (e, w)) in [("8 6 3 3", "2"), ("8 6 1 1", "1.5")].iter().enumerate() {
        writeln!(s, "case strkround-{i}\nd2target 16 12\nd2ell 0 {e}\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    // the elliptic pen (a skew, a non-uniform scale, a reflection; curves and sharp corners
    // under it), DrawGeometry of rectangles (antialiased, under a skew), and a layer's mask with
    // curves (D2D.RT, still open)
    for (i, (x, g, w)) in [
        ("1 0 0.5 1 0 0", "d2ell 0 7 6 4 3", "2"),
        ("2 0 0 0.75 0 0", "d2path 0 wind m 1 2 7 3 2 12 z", "1.5"),
        ("-1 0 0 1 16 0", "d2path 0 alt m 2 2 12 4 4 10 o", "2.5"),
        ("1 0.25 -0.5 1 3 0", "d2path 0 alt m 1 1 12 2 3 3 o", "1"),
        ("0.75 0 0.25 1.5 0 0", "d2rrect 0 2 2 14 6 2 1", "1.25"),
    ].iter().enumerate() {
        writeln!(s, "case strkpen-{i}\nd2target 16 12\nd2xform {x}\n{g}\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    for (i, (x, aa, w)) in [("1 0 0 1 0 0", "per", "1.5"), ("1 0 0.5 1 0 0", "per", "1"), ("1 0 0.5 1 0 0", "aliased", "2"), ("1 0 0 1 0.25 0.5", "per", "9")].iter().enumerate() {
        writeln!(s, "case strkrect-{i}\nd2target 16 12\nd2aa {aa}\nd2xform {x}\nd2rect 0 2.5 2 12 9.5\nd2drawg 0 {w} 255 255 255 255\nd2read\nend");
    }
    writeln!(s, "case layercurve-0\nd2target 16 12\nd2ell 0 8 6 5 4\nd2layer 0 0 16 12 1 0\nd2fill 0 0 16 12 238 191 70 255\nd2poplayer\nd2read\nend");
    writeln!(s, "case pareahuge-2\nd2path 0 alt m -3000 -3000 3000 3000 3000 -3000 0.0001 3000 z\nd2area 0\nend");
    writeln!(s, "case outlinehuge-0\nd2path 0 alt m -3000 -3000 3000 3000 3000 -3000 -3000 3000 z\nd2outline 0\nend");
    writeln!(s, "case layercurve-1\nd2target 16 12\nd2ell 0 8 6 7 5\nd2layer 2 1 13 10 1 0\nd2fill 0 0 16 12 238 191 70 255\nd2poplayer\nd2read\nend");
    writeln!(s, "case pareahuge-3\nd2path 0 alt m -200000000 -200000000 200000000 200000000 200000000 -200000000 -200000000 200000000 z\nd2area 0\nend");
    writeln!(s, "case tesshuge-0\nd2path 0 alt m -300000 -300000 300000 -300000 0 300000 z\nd2tess 0\nend");
    writeln!(s, "case strkinf-0\nd2target 16 12\nd2path 0 alt m 2 2 12 4 4 10 z\nd2drawg 0 inf 255 255 255 255\nd2read\nend");
    writeln!(s, "case pareahuge-4\nd2ell 0 200000000 100000000 300000000 150000000\nd2area 0\nend");
    writeln!(s, "case tesshuge-1\nd2path 0 alt m -1e13 -1e13 1e13 -1e13 0 1e13 z\nd2tess 0\nend");
    writeln!(s, "case strknan-0\nd2target 16 12\nd2path 0 alt m 2 2 12 4 4 10 z\nd2drawg 0 NaN 255 255 255 255\nd2read\nend");
}

/// Focused native measurements for the remaining render-target contracts.
pub fn d2dlimits() -> String {
    let mut s = String::from("# kikizake suite d2dlimits: Direct2D render-target boundary cases.\n");
    let bitmap = "d2bmp 0 2 2 0000ffff00ff00ffff0000ffffffffff";
    for (name, op) in [
        ("bitmap-src-nan", format!("{bitmap}\nd2drawbmp 0 0 0 4 4 1 nearest NaN 0 2 2")),
        ("bitmap-src-infinity", format!("{bitmap}\nd2drawbmp 0 0 0 4 4 1 linear 0 0 inf 2")),
        ("gradient-empty", "d2grad 0 0 4 4 lin 22 clamp 0 0 4 4 0".into()),
        ("gradient-singular", "d2xform 1 1 1 1 0 0\nd2grad 0 0 4 4 lin 22 clamp 0 0 4 4 2 0 255 0 0 255 1 0 0 255 255".into()),
        ("gradient-coincident", "d2grad 0 0 4 4 lin 22 clamp 2 2 2 2 2 0 255 0 0 255 1 0 0 255 255".into()),
        ("curve-outside-fixed-range", "d2ell 0 8388608 2 8388608 2\nd2fillg 0 255 255 255 255".into()),
        ("stroke-nan", "d2ell 0 2 2 1 1\nd2drawg 0 NaN 255 255 255 255".into()),
    ] {
        writeln!(s, "case {name}\nd2target 4 4\nd2clear 39 70 111 255\n{op}\nd2read\nend");
    }
    for (end, limit) in [("positive", "inf"), ("negative", "-inf")] {
        for mode in ["nearest", "linear"] {
            for (axis, src) in [
                ("x", format!("{limit} 0 {limit} 2")),
                ("y", format!("0 {limit} 2 {limit}")),
                ("turned", format!("{limit} 0 {limit} 2")),
            ] {
                let xf = if axis == "turned" { "d2xform 0 1 -1 0 4 0\n" } else { "" };
                writeln!(s, "case bitmap-src-infinity-{end}-{mode}-{axis}\nd2target 4 4\nd2aa aliased\nd2clear 39 70 111 255\n{bitmap}\n{xf}d2drawbmp 0 0 0 4 4 1 {mode} {src}\nd2read\nend");
            }
        }
    }
    let colors = "3 0 255 0 0 255 0.5 0 255 0 255 1 0 0 255 255";
    for ext in ["clamp", "wrap", "mirror"] {
        for (name, transform, kind, args) in [
            ("point-linear", "1 0 0 1 0 0", "lin", "2 2 2 2"),
            ("flat-radial-x", "1 0 0 1 0 0", "rad", "2 2 0 0 0 2"),
            ("flat-radial-y", "1 0 0 1 0 0", "rad", "2 2 0 0 2 0"),
            ("flat-radial-skew", "1 1 0 1 0 0", "rad", "2 2 0 0 0 0"),
            ("zero-linear", "0 0 0 0 2 2", "lin", "0 0 4 4"),
            ("line-radial", "1 0 0 0 0 2", "rad", "2 2 0 0 2 2"),
        ] {
            writeln!(s, "case gradient-{name}-{ext}\nd2target 4 4\nd2clear 39 70 111 255\nd2xform {transform}\nd2grad 0 0 4 4 {kind} 22 {ext} {args} {colors}\nd2read\nend");
        }
    }
    s
}
