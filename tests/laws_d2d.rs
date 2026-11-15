//! Properties of the Direct2D model (src/model/d2d.rs) that hold whatever the measured details:
//! exhaustive over small domains, property-based elsewhere. With Windows' answers they are what
//! cargo-mutants counts on to catch every mutant of the model.
use proptest::prelude::*;
use kikizake::case::parse_cases;
use kikizake::model::d2d::*;
use kikizake::model::{self, run_case};

/// the laws are of the fitted approximations too: they answer here
fn fitted() {
    model::set_fitted(model::Fitted::Answered);
}

fn run(text: &str) -> Vec<String> {
    fitted();
    run_case(&parse_cases(&format!("case p\n{text}end\n"))[0]).unwrap()
}

fn ar(g: &Geom) -> f32 {
    fitted();
    match area(g, 0.25).unwrap() {
        Measure::Value(v) => v,
        Measure::BadNumber => f32::NAN,
    }
}

fn rect(l: f32, t: f32, r: f32, b: f32) -> Geom {
    Geom::Rect([l, t, r, b])
}

fn path(wind: bool, figs: &[&[(f32, f32)]]) -> Geom {
    Geom::Path { wind, figs: figs.iter().map(|p| Figure { pts: p.to_vec(), closed: true }).collect() }
}

/// unimodular integer matrices and power-of-two scales: their inverses are exact, and inverting
/// twice gives the matrix back
#[test]
fn exact_inverses_are_involutions() {
    let mut n = 0;
    for a in -2i32..=2 {
        for b in -2i32..=2 {
            for c in -2i32..=2 {
                for d in -2i32..=2 {
                    if a * d - b * c != 1 && a * d - b * c != -1 {
                        continue;
                    }
                    let m = [a as f32, b as f32, c as f32, d as f32, 3.0, -5.0];
                    let (ok, i) = invert(&m);
                    assert!(ok);
                    let p = mul(&m, &i);
                    assert_eq!(p.map(|v| v + 0.0), IDENTITY, "{m:?} times its inverse");
                    assert_eq!(invert(&i).1.map(|v| v + 0.0), m, "{m:?}");
                    n += 1;
                }
            }
        }
    }
    assert!(n > 50);
    for k in -8..8 {
        let s = (k as f32).exp2();
        assert_eq!(invert(&[s, 0.0, 0.0, s, 1.0, 1.0]).1, [1.0 / s, -0.0, -0.0, 1.0 / s, -1.0 / s, -1.0 / s]);
    }
}

/// singular matrices are reported and left alone; the executor's answer prints the same matrix
#[test]
fn singular_matrices_are_left_alone() {
    for m in [[1.0, 2.0, 2.0, 4.0, 5.0, 6.0], [0.0; 6], [1e-30, 0.0, 0.0, 1e-30, 0.0, 0.0]] {
        assert_eq!(invert(&m), (false, m));
    }
    let out = run("d2inv 1 2 2 4 5 6\n");
    assert_eq!(out, vec![format!("inv 0 0 {}", [1.0f32, 2.0, 2.0, 4.0, 5.0, 6.0].map(hex).join(" "))]);
}

/// every right angle in -3600..=3600 is an exact permutation matrix, and the center stays put
#[test]
fn right_angles_are_exact() {
    for k in -40i32..=40 {
        let m = rotate(k as f32 * 90.0, 3.0, -2.0);
        let (c, s) = [(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)][k.rem_euclid(4) as usize];
        assert_eq!([m[0], m[1], m[2], m[3]].map(|v| v + 0.0), [c, s, -s + 0.0, c], "{k} right angles");
        assert_eq!(apply(&m, 3.0, -2.0), (3.0, -2.0));
    }
}

proptest! {
    /// a rotation is a rotation: m11 = m22, m12 = -m21, and turning by a further 360 degrees (an
    /// exact f32 sum) changes nothing
    #[test]
    fn rotations_are_periodic(q in -4000i32..4000, cx in -16i32..16, cy in -16i32..16) {
        let a = q as f32 / 4.0;
        let m = rotate(a, cx as f32, cy as f32);
        prop_assert_eq!(m[0], m[3]);
        prop_assert_eq!(m[1], -m[2]);
        prop_assert_eq!(m, rotate(a + 360.0, cx as f32, cy as f32));
        prop_assert!((m[0] * m[0] + m[1] * m[1] - 1.0).abs() < 1e-6);
    }

    /// a skew keeps the diagonal and moves the center nowhere (up to rounding)
    #[test]
    fn skews_fix_the_center(ax in -80i32..80, ay in -80i32..80, cx in -8i32..8, cy in -8i32..8) {
        let m = skew(ax as f32, ay as f32, cx as f32, cy as f32);
        prop_assert_eq!((m[0], m[3]), (1.0, 1.0));
        let (x, y) = apply(&m, cx as f32, cy as f32);
        prop_assert!((x - cx as f32).abs() < 1e-4 && (y - cy as f32).abs() < 1e-4);
        prop_assert_eq!(m[1] >= 0.0, ay >= 0);
    }

    /// a rectangle's area does not depend on how it is given: as a rectangle, as either winding
    /// of a path, under either fill mode, moved by whole units
    #[test]
    fn rectangle_areas_agree(l in -8i32..8, t in -8i32..8, w in 0i32..9, h in 0i32..9, dx in -5i32..5) {
        let (l, t, r, b) = (l as f32, t as f32, (l + w) as f32, (t + h) as f32);
        let want = (w * h) as f32;
        prop_assert_eq!(ar(&rect(l, t, r, b)), want);
        prop_assert_eq!(ar(&rect(r, b, l, t)), want);
        for wind in [false, true] {
            let cw: &[(f32, f32)] = &[(l, t), (r, t), (r, b), (l, b)];
            let ccw: &[(f32, f32)] = &[(l, t), (l, b), (r, b), (r, t)];
            prop_assert_eq!(ar(&path(wind, &[cw])), want);
            prop_assert_eq!(ar(&path(wind, &[ccw])), want);
            let d = dx as f32;
            let moved: Vec<(f32, f32)> = cw.iter().map(|p| (p.0 + d, p.1)).collect();
            prop_assert_eq!(ar(&path(wind, &[&moved])), want);
        }
    }

    /// the winding fill holds everything the alternate fill holds: its area is at least as
    /// large, and equal for one convex figure; the area is never more than the bounds'
    #[test]
    fn winding_fills_at_least_alternate(pts in proptest::collection::vec((0i32..10, 0i32..10), 3..7), pts2 in proptest::collection::vec((0i32..10, 0i32..10), 3..6)) {
        let p: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
        let q: Vec<(f32, f32)> = pts2.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
        let alt = ar(&path(false, &[&p, &q]));
        let wind = ar(&path(true, &[&p, &q]));
        prop_assert!(wind >= alt - 1e-4, "winding {} alternate {}", wind, alt);
        let b = bounds(&path(true, &[&p, &q]), None).unwrap();
        prop_assert!(wind <= (b[2] - b[0]) * (b[3] - b[1]) + 1e-4);
        // (a figure that goes back over its own edges has no area: the f64 sum of its
        // trapezoids may leave a rounding residue either side of 0)
        prop_assert!(alt >= -1e-4, "alternate {}", alt);
    }

    /// FillContainsPoint: the winding fill holds every point the alternate one does; a point
    /// far from a rectangle is in exactly when it is inside
    #[test]
    fn containment(pts in proptest::collection::vec((0i32..10, 0i32..10), 3..7), x in -8i32..88, y in -8i32..88) {
        let p: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (x as f32, y as f32)).collect();
        let (px, py) = (x as f32 / 8.0, y as f32 / 8.0);
        if let (Ok(a), Ok(w)) = (contains(&path(false, &[&p]), px, py, 0.25), contains(&path(true, &[&p]), px, py, 0.25)) {
            prop_assert!(!a || w);
        }
        let r = rect(2.0, 2.0, 8.0, 6.0);
        let d = (2.0 - px).max(px - 8.0).max(2.0 - py).max(py - 6.0);
        if d.abs() > 0.3 {
            prop_assert_eq!(contains(&r, px, py, 0.25).unwrap(), d < 0.0);
        }
    }

    /// CompareWithGeometry both ways: disjoint and overlap are symmetric, contains is the other
    /// side of is contained, and a rectangle is contained in itself
    #[test]
    fn relations_are_converse(a in (0i32..8, 0i32..8, 1i32..8, 1i32..8), b in (0i32..8, 0i32..8, 1i32..8, 1i32..8)) {
        let g = |r: (i32, i32, i32, i32)| rect(r.0 as f32, r.1 as f32, (r.0 + r.2) as f32, (r.1 + r.3) as f32);
        let (ab, ba) = (relation(&g(a), &g(b)).unwrap(), relation(&g(b), &g(a)).unwrap());
        let conv = [0, 1, 3, 2, 4];
        if a == b {
            prop_assert_eq!((ab, ba), (2, 2));
        } else {
            prop_assert_eq!(ab, conv[ba as usize]);
        }
        prop_assert_eq!(relation(&g(a), &g(a)).unwrap(), 2);
    }

    /// aliased fills tile: two rectangles that share an edge cover each pixel once, as the one
    /// they make together (the top-left rule), whatever the quarter-pixel edges
    #[test]
    fn aliased_fills_tile(l in -8i32..72, m in -8i32..72, r in -8i32..72, t in -8i32..72, b in -8i32..72) {
        let e = |v: i32| format!("{}", v as f32 / 4.0);
        let base = "d2target 16 16\nd2aa aliased\nd2clear 0 0 0 0\n";
        let two = run(&format!("{base}d2fill {} {} {} {} 0 0 255 128\nd2fill {} {} {} {} 0 0 255 128\nd2read\n", e(l), e(t), e(m), e(b), e(m), e(t), e(r), e(b)));
        let one = run(&format!("{base}d2fill {} {} {} {} 0 0 255 128\nd2read\n", e(l), e(t), e(r), e(b)));
        if (l <= m) == (m <= r) {
            prop_assert_eq!(two, one);
        }
    }

    /// source over: an opaque source replaces, a transparent one leaves the target, and the
    /// result is a premultiplied color (no channel above its alpha)
    #[test]
    fn source_over(c in proptest::array::uniform4(0i64..256), d in proptest::array::uniform4(0i64..256)) {
        let (s, t) = (premultiply(c), premultiply(d));
        let o = over(s, t);
        prop_assert!(o[0] <= o[3] && o[1] <= o[3] && o[2] <= o[3], "{:?} over {:?} is {:?}", s, t, o);
        if c[3] == 255 {
            prop_assert_eq!(o, s);
        }
        if c[3] == 0 {
            prop_assert_eq!(o, t);
        }
    }

    /// any line of d2d ops and junk: the model answers or says why not, never panics
    #[test]
    fn never_panics(text in "(case [a-z]{1,3}\n|end\n|(d2rot|d2skew|d2inv|d2sincos|d2tan|d2vec3|d2cs|d2stroke|d2rect|d2ell|d2rrect|d2xf|d2path|d2sink|d2bounds|d2in|d2area|d2len|d2cmp|d2count|d2target|d2aa|d2xform|d2clear|d2fill|d2fillg|d2brush|d2read|d2info|d2dpi|d2tags|d2flush|d2state|d2clip|d2unclip|d2fillo)( (-?[0-9]{1,3}(\\.[0-9]{1,2})?|0x[0-9a-f]{1,8}|m|z|o|alt|wind|open|close|begin|line|end|count|fill|aliased|per|save|restore)){0,12}\n){0,30}") {
        let _ = model::run("p", &parse_cases(&text));
    }
}

/// premultiplying every (channel, alpha) pair: never past the alpha, exact at alpha 0 and 255,
/// and monotonic in both
#[test]
fn premultiply_exhaustive() {
    for a in 0..256i64 {
        let mut last = 0;
        for c in 0..256i64 {
            let p = premultiply([c, 0, 0, a]);
            assert_eq!(p[3] as i64, a);
            assert!(p[2] <= p[3], "{c} {a}");
            assert!(p[2] >= last);
            last = p[2];
            if a == 255 {
                assert_eq!(p[2] as i64, c);
            }
            if a == 0 {
                assert_eq!(p[2], 0);
            }
            if a > 0 {
                assert!(premultiply([c, 0, 0, a - 1])[2] <= p[2]);
            }
        }
    }
}

/// every valid stroke style is accepted and reads back; one past any enumeration is refused
#[test]
fn stroke_enums_exhaustive() {
    for start in 0..5 {
        for end in 0..5 {
            for dash in 0..5 {
                for join in 0..5 {
                    for style in 0..7 {
                        let dashes = if style == 5 { " 1 2" } else { "" };
                        let out = run(&format!("d2stroke {start} {end} {dash} {join} 10 {style} 0{dashes}\n"));
                        let ok = start < 4 && end < 4 && dash < 4 && join < 4 && style < 6;
                        assert_eq!(out[0].starts_with(&format!("stroke = {start} {end} {dash} {join} ")), ok, "{out:?}");
                        assert_eq!(out[0] == "stroke hr 80070057", !ok);
                    }
                }
            }
        }
    }
}

/// the sink: whatever the order of the calls, a geometry opens once, and counts are given
/// exactly when the sink closed without an error
#[test]
fn sink_states_exhaustive() {
    let steps = ["open", "close", "begin 0 0", "line 1 1", "end", "count"];
    let mut n = 0;
    for len in 1..=5u32 {
        for code in 0..6usize.pow(len) {
            let seq: Vec<&str> = (0..len).map(|i| steps[code / 6usize.pow(i) % 6]).collect();
            let out = run(&format!("d2sink {}\n", seq.join(" ")));
            let opens: Vec<&str> = out[0].match_indices(" open ").map(|(i, _)| &out[0][i + 6..i + 14]).collect();
            assert!(opens.iter().skip(1).all(|h| *h == "88990001") && opens.first().is_none_or(|h| *h == "00000000"), "{seq:?}: {}", out[0]);
            if !seq.contains(&"close") {
                assert!(!out[0].contains(" count 00000000"), "{seq:?}: {}", out[0]);
            }
            n += 1;
        }
    }
    assert!(n > 9000);
}

/// the generators write only lines the executor runs as written: the model answers no d2d case
/// with "bad line", and refuses lines the executor refuses
#[test]
fn no_bad_lines() {
    let out = model::run("d2d", &parse_cases(&kikizake::generators::suite("d2d").unwrap()));
    assert!(!out.contains("unmodeled bad line"), "{}", out.lines().zip(out.lines().skip(1)).find(|l| l.1 == "unmodeled bad line").map(|l| l.0).unwrap_or(""));
    let dashes = vec!["1"; 33].join(" ");
    for l in ["d2target 0 4", "d2target 4 65", "d2target 4 4 4", "d2target 4", "d2aa maybe", "d2aa per per", "d2xform 1 0 0 1 0", "d2bounds 0 1 0 0 1 0", "d2rect 8 0 0 1 1", "d2path 0 odd m 0 0 1 1 z", "d2path 0 alt m 0 0 1 1", "d2path 0 alt 1 1 z", &format!("d2stroke 0 0 0 0 10 5 0 {dashes}")] {
        let c = &parse_cases(&format!("case p\nd2target 2 2\n{l}\nend\n"))[0];
        assert_eq!(run_case(c), Err(model::Fail::Bad), "{l}");
    }
    let dashes = vec!["1"; 32].join(" ");
    assert!(run(&format!("d2stroke 0 0 0 0 10 5 0 {dashes}\n"))[0].starts_with("stroke = "));
    assert_eq!(run("d2target 64 1\nd2aa per\nd2aa aliased\nd2xform 1 0 0 1 0 0\n"), vec!["target 00000000"]);
}

proptest! {
    /// an ellipse's or rounded rectangle's area is worked out about its own center, rounding
    /// to nearest even: turning it half way round (every coordinate negated) does not change it;
    /// it lies between the inscribed rhombus and the ellipse, a rounded rectangle within its box
    #[test]
    fn curve_areas_mirror_and_bound(cx in -16i32..16, cy in -16i32..16, rx in 1i32..64, ry in 1i32..64, k in 0usize..3) {
        let (rx, ry) = (rx as f32 / 8.0, ry as f32 / 8.0);
        let tol = [0.25f32, 1.0, 0.05][k];
        let e = |x: i32, y: i32| Geom::Ell([x as f32, y as f32, rx, ry]);
        fitted();
        let a = |g: &Geom| match area(g, tol).unwrap() { Measure::Value(v) => v, Measure::BadNumber => f32::NAN };
        let here = a(&e(cx, cy));
        prop_assert_eq!(here, a(&e(-cx, -cy)));
        prop_assert!(here >= 2.0 * rx * ry * (1.0 - 1e-6) && here <= std::f32::consts::PI * rx * ry * (1.0 + 1e-6), "{} for {} x {}", here, rx, ry);
        let r = |x: f32, y: f32| Geom::RRect([x - rx, y - ry, x + rx, y + ry, rx / 2.0, ry / 2.0]);
        let ra = a(&r(cx as f32, cy as f32));
        prop_assert_eq!(ra, a(&r(-cx as f32, -cy as f32)));
        prop_assert!(ra <= 4.0 * rx * ry * (1.0 + 1e-6));
    }

    /// a geometry whose bounds are not finite has no area: D2DERR_BAD_NUMBER
    #[test]
    fn curve_area_of_nan_fails(k in 0usize..4) {
        let mut v = [0.0f32, 0.0, 2.0, 1.0];
        v[k] = f32::NAN;
        prop_assert_eq!(area(&Geom::Ell(v), 0.25).unwrap(), Measure::BadNumber);
    }
}

proptest! {
    /// a transformed rectangle's area is the rectangle's times |det|, and its length follows
    /// the sides (exact for these transforms)
    #[test]
    fn transformed_areas(l in -8i32..8, t in -8i32..8, w in 0i32..9, h in 0i32..9, k in 0usize..6) {
        let ms: [M; 6] = [[2.0, 0.0, 0.0, 2.0, 1.0, 1.0], [0.0, 1.0, -1.0, 0.0, 0.0, 0.0], [1.0, 0.0, 1.0, 1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 1.0, 3.0, 0.0], [0.5, 0.0, 0.0, 4.0, 0.0, 0.0], [1.0, 1.0, -1.0, 1.0, 0.0, 0.0]];
        let m = ms[k];
        let r = rect(l as f32, t as f32, (l + w) as f32, (t + h) as f32);
        let x = Geom::Xf(Box::new(r.clone()), m);
        let det = (m[0] * m[3] - m[1] * m[2]).abs();
        prop_assert_eq!(ar(&x), (w * h) as f32 * det);
        if m[1] == 0.0 && m[2] == 0.0 {
            prop_assert_eq!(length(&x, 0.25).unwrap(), 2.0 * (w as f32 * m[0].abs() + h as f32 * m[3].abs()));
        }
        prop_assert_eq!(length(&r, 0.25).unwrap(), (2 * (w + h)) as f32);
    }
}

proptest! {
    /// DrawBitmap's source rectangle is put in order first: naming its sides the other way round
    /// draws the same; one with no width or height inside the bitmap draws nothing (a side past an edge is moved onto
    /// it, so one wholly past an edge is turned round and repeats the edge texels instead)
    #[test]
    fn bitmap_sources_are_ordered(l in -12i32..20, t in -12i32..20, r in -12i32..20, b in -12i32..20, lin in proptest::bool::ANY, px in proptest::array::uniform12(0u8..=255)) {
        let q = |v: i32| v as f32 / 4.0;
        let hex: String = px.iter().enumerate().map(|(i, v)| format!("{:02x}", if i % 4 == 3 { 255 } else { *v })).collect::<String>().repeat(3);
        let mode = if lin { "linear" } else { "nearest" };
        let pixels = |out: Vec<String>| out.into_iter().skip_while(|l| !l.starts_with("px ")).collect::<Vec<_>>();
        let draw = |s: [f32; 4]| pixels(run(&format!("d2target 8 8\nd2aa aliased\nd2clear 1 2 3 255\nd2bmp 0 3 3 {hex}\nd2drawbmp 0 0.5 0.75 7.25 7.5 1 {mode} {} {} {} {}\nd2read\n", s[0], s[1], s[2], s[3])));
        let (l, t, r, b) = (q(l), q(t), q(r), q(b));
        let one = draw([l, t, r, b]);
        prop_assert_eq!(&one, &draw([r, t, l, b]));
        prop_assert_eq!(&one, &draw([l, b, r, t]));
        let empty = pixels(run("d2target 8 8\nd2aa aliased\nd2clear 1 2 3 255\nd2read\n"));
        let inside = |v: f32| (0.0..=3.0).contains(&v);
        if (l == r && inside(l)) || (t == b && inside(t)) {
            prop_assert_eq!(&one, &empty);
        }
    }
}

proptest! {
    /// DrawGeometry of a rectangle's figure (its four corners in either turn, from any corner)
    /// paints what DrawRectangle does, aliased or antialiased, under a transform too, whatever
    /// its sides against the width (strokewide-*)
    #[test]
    fn stroked_rectangles_are_drawrectangle(l in -8i32..40, t in -8i32..40, w in 1i32..40, h in 1i32..40, sw in 1i32..12, start in 0usize..4, back in proptest::bool::ANY, aa in proptest::bool::ANY, k in 0usize..3) {
        let q = |v: i32| v as f32 / 4.0;
        let (l, t, r, b, sw) = (q(l), q(t), q(l + w), q(t + h), q(sw));
        let mut c = vec![(l, t), (r, t), (r, b), (l, b)];
        if back {
            c.reverse();
        }
        c.rotate_left(start);
        let pts: Vec<String> = c.iter().map(|p| format!("{} {}", p.0, p.1)).collect();
        let xf = ["", "d2xform 2 0 0 2 1 0\n", "d2xform 0 1 -1 0 12 0\n"][k];
        let aa = if aa { "d2aa aliased\n" } else { "" };
        let head = format!("d2target 16 12\n{aa}{xf}d2clear 9 80 200 255\n");
        let mut a = run(&format!("{head}d2path 0 alt m {} z\nd2drawg 0 {sw} 250 120 30 255\nd2read\n", pts.join(" ")));
        let b = run(&format!("{head}d2drawrect {l} {t} {r} {b} {sw} 250 120 30 255\nd2read\n"));
        a.retain(|x| !x.starts_with("path"));
        prop_assert_eq!(a, b);
    }
}

proptest! {
    /// Tessellate of a convex polygon on the 1/64 grid (points on a circle in turn): every
    /// triangle turns the same way and none is empty, and they cover the polygon exactly once
    /// (their areas sum to its area) whichever corner it starts at and whichever way it turns
    #[test]
    fn convex_tessellations_tile(cx in -256i32..256, cy in -256i32..256, rad in 64i32..1024, n in 3usize..9, start in 0usize..9, back in proptest::bool::ANY, jit in proptest::collection::vec(0u32..1000, 9)) {
        let mut a: Vec<f64> = (0..n).map(|i| (i as f64 + f64::from(jit[i]) / 2000.0) / n as f64 * std::f64::consts::TAU).collect();
        a.sort_by(f64::total_cmp);
        let mut pts: Vec<(f32, f32)> = a.iter().map(|t| {
            let x = (f64::from(cx) + f64::from(rad) * t.cos()).round() / 64.0;
            let y = (f64::from(cy) + f64::from(rad) * t.sin()).round() / 64.0;
            (x as f32, y as f32)
        }).collect();
        pts.dedup();
        if back {
            pts.reverse();
        }
        let k = start % pts.len();
        pts.rotate_left(k);
        let n = pts.len();
        let cr = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (f64::from(b.0) - f64::from(a.0)) * (f64::from(c.1) - f64::from(a.1)) - (f64::from(b.1) - f64::from(a.1)) * (f64::from(c.0) - f64::from(a.0));
        // only strictly convex rounded polygons
        prop_assume!(n >= 3);
        let turns: Vec<f64> = (0..n).map(|i| cr(pts[i], pts[(i + 1) % n], pts[(i + 2) % n])).collect();
        prop_assume!(turns.iter().all(|&t| t > 0.0) || turns.iter().all(|&t| t < 0.0));
        let g = path(true, &[&pts]);
        let ts = tessellate(&g, 0.25);
        prop_assert!(ts.is_some());
        let ts = ts.unwrap_or_default();
        let s: Vec<f64> = ts.iter().map(|t| cr(t[0], t[1], t[2])).collect();
        prop_assert!(s.iter().all(|&v| v != 0.0));
        prop_assert!(s.iter().all(|&v| v > 0.0) || s.iter().all(|&v| v < 0.0));
        let tri: f64 = s.iter().map(|v| v.abs()).sum();
        let poly: f64 = (0..n).map(|i| f64::from(pts[i].0) * f64::from(pts[(i + 1) % n].1) - f64::from(pts[(i + 1) % n].0) * f64::from(pts[i].1)).sum::<f64>().abs();
        prop_assert_eq!(tri, poly);
        prop_assert_eq!(ts.len(), n - 2);
    }
}
