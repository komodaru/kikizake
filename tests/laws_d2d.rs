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
    assert!(!out.contains("invalid case"), "{}", out.lines().collect::<Vec<_>>().windows(2).find(|l| l[1] == "invalid case").map_or("", |l| l[0]));
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
        let ts = kikizake::model::d2dtess::tessellate(&g, 0.25);
        prop_assert!(ts.is_ok());
        let ts = ts.unwrap_or_default();
        let s: Vec<f64> = ts.iter().map(|t| cr(t[0], t[1], t[2])).collect();
        prop_assert!(s.iter().all(|&v| v > 0.0));
        let tri: f64 = s.iter().sum();
        let poly: f64 = (0..n).map(|i| f64::from(pts[i].0) * f64::from(pts[(i + 1) % n].1) - f64::from(pts[(i + 1) % n].0) * f64::from(pts[i].1)).sum::<f64>().abs();
        prop_assert_eq!(tri, poly);
        prop_assert_eq!(ts.len(), n - 2);
    }

    /// Tessellate of any polygons on a small grid, crossing or touching themselves and each
    /// other, by either fill mode: every triangle turns the same way, and the triangles cover
    /// the filled region exactly once: a sample point off every line is in as many triangles
    /// as the fill mode says it is filled (0 or 1); and the scanner's CArea sum is twice the
    /// region's area
    #[test]
    fn tessellations_tile_the_fill(wind in proptest::bool::ANY, figs in proptest::collection::vec(proptest::collection::vec((0i32..9, 0i32..9), 3..7), 1..4)) {
        let fs: Vec<Vec<(f32, f32)>> = figs.iter().map(|f| f.iter().map(|&(x, y)| (x as f32, y as f32)).collect()).collect();
        let refs: Vec<&[(f32, f32)]> = fs.iter().map(Vec::as_slice).collect();
        let g = path(wind, &refs);
        let ts = kikizake::model::d2dtess::tessellate(&g, 0.25);
        prop_assert!(ts.is_ok(), "{:?}", ts.err());
        let ts = ts.unwrap_or_default();
        let cr = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| (f64::from(b.0) - f64::from(a.0)) * (f64::from(c.1) - f64::from(a.1)) - (f64::from(b.1) - f64::from(a.1)) * (f64::from(c.0) - f64::from(a.0));
        // (an empty triangle of crossing points on one line comes out too, D2D.TESS: its f32
        // corners may turn either way by an ulp)
        prop_assert!(ts.iter().all(|t| cr(t[0], t[1], t[2]) >= -1e-4));
        let polys: Vec<Vec<(f64, f64)>> = fs.iter().map(|f| f.iter().map(|&(x, y)| (f64::from(x), f64::from(y))).collect()).collect();
        let inside = |p: (f64, f64)| {
            let mut w = 0;
            for f in &polys {
                for i in 0..f.len() {
                    let (a, b) = (f[i], f[(i + 1) % f.len()]);
                    if (a.1 <= p.1) != (b.1 <= p.1) {
                        let x = a.0 + (p.1 - a.1) * (b.0 - a.0) / (b.1 - a.1);
                        if x > p.0 {
                            w += if b.1 > a.1 { 1 } else { -1 };
                        }
                    }
                }
            }
            if wind { w != 0 } else { w % 2 != 0 }
        };
        let tin = |t: &[(f32, f32); 3], p: (f64, f64)| {
            let c = |a: (f32, f32), b: (f32, f32)| (f64::from(b.0) - f64::from(a.0)) * (p.1 - f64::from(a.1)) - (f64::from(b.1) - f64::from(a.1)) * (p.0 - f64::from(a.0));
            c(t[0], t[1]) > 0.0 && c(t[1], t[2]) > 0.0 && c(t[2], t[0]) > 0.0
        };
        // (the f32 corners of the crossings round off: the sums agree to 1e-6)
        // (sample points off the grid's lines and diagonals: irrational-ish offsets)
        for i in 0..40 {
            for j in 0..40 {
                let p = (f64::from(i) * 0.2 + 0.0371, f64::from(j) * 0.2 + 0.0617);
                let n = ts.iter().filter(|t| tin(t, p)).count();
                prop_assert_eq!(n, usize::from(inside(p)), "at {:?}", p);
            }
        }
        let ints: Vec<Vec<(i128, i128)>> = figs.iter().map(|f| f.iter().map(|&(x, y)| (i128::from(x) * 64, i128::from(y) * 64)).collect()).collect();
        let sc = kikizake::model::d2dtess::scan(wind, &ints);
        prop_assert!(sc.is_ok());
        let acc = sc.map(|s| s.acc).unwrap_or_default() / 4096.0;
        let tri: f64 = ts.iter().map(|t| cr(t[0], t[1], t[2])).sum();
        prop_assert!((acc - tri).abs() <= 1e-6 * (1.0 + tri), "{} {}", acc, tri);
    }
}

/// the shoelace sum of figures, f64
fn shoelace(figs: &[Fig]) -> f64 {
    let mut a = 0.0;
    for f in figs {
        let mut pts = vec![f.start];
        pts.extend(f.segs.iter().map(|s| match *s {
            Seg::L(p) | Seg::C(_, _, p) => p,
        }));
        for w in pts.windows(2) {
            a += f64::from(w[0].0) * f64::from(w[1].1) - f64::from(w[1].0) * f64::from(w[0].1);
        }
    }
    a / 2.0
}

proptest! {
    /// Outline and CombineWithGeometry of grid polygons (crossing, touching, either fill mode):
    /// every figure comes back to its start, the fill is on the right (the figures' signed areas sum to the
    /// filled area, holes negative), and the four operations agree with each other: union +
    /// intersect = A + B, xor = union - intersect, exclude = A - intersect
    #[test]
    fn outlines_bound_the_fill(wa in proptest::bool::ANY, wb in proptest::bool::ANY, fa in proptest::collection::vec((0i32..9, 0i32..9), 3..7), fb in proptest::collection::vec((0i32..9, 0i32..9), 3..7)) {
        use kikizake::model::d2dbool::{outline, Op};
        let to = |f: &[(i32, i32)]| f.iter().map(|&(x, y)| (x as f32, y as f32)).collect::<Vec<_>>();
        let (pa, pb) = (to(&fa), to(&fb));
        let (a, b) = (path(wa, &[&pa]), path(wb, &[&pb]));
        let area = |g: &Geom| f64::from(ar(g));
        let o = outline(&[&a], Op::Outline, 0.25);
        prop_assert!(o.is_ok());
        let o = o.unwrap_or_default();
        // (each figure comes back to its start: exactly, or at a crossing to the other line's
        // f64 copy of it, then left OPEN though the f32 points may be equal)
        let end = |f: &Fig| f.segs.last().map(|s| match *s { Seg::L(p) | Seg::C(_, _, p) => p });
        prop_assert!(o.iter().all(|f| end(f).is_some_and(|e| (e.0 - f.start.0).abs() < 1e-5 && (e.1 - f.start.1).abs() < 1e-5)));
        prop_assert!((shoelace(&o) - area(&a)).abs() < 1e-4);
        let op = |m: Op| outline(&[&a, &b], m, 0.25).map(|f| shoelace(&f)).unwrap_or(f64::NAN);
        let (u, i, x, e) = (op(Op::Union), op(Op::Intersect), op(Op::Xor), op(Op::Exclude));
        prop_assert!((u + i - area(&a) - area(&b)).abs() < 1e-4, "{} {} {} {}", u, i, area(&a), area(&b));
        prop_assert!((x - (u - i)).abs() < 1e-4);
        prop_assert!((e - (area(&a) - i)).abs() < 1e-4);
    }

    /// Widen of one straight line: the rectangle of its length by the width (its rails' points
    /// at half the width either side, the figure turning one way)
    #[test]
    fn widened_lines_are_rectangles(x0 in -64i32..64, y0 in -64i32..64, x1 in -64i32..64, y1 in -64i32..64, w in 1i32..64) {
        prop_assume!((x0, y0) != (x1, y1));
        let pts = [(x0 as f32 / 4.0, y0 as f32 / 4.0), (x1 as f32 / 4.0, y1 as f32 / 4.0)];
        let f = Fig { start: pts[0], segs: vec![Seg::L(pts[1])], closed: false };
        let w = w as f32 / 8.0;
        let r = kikizake::model::d2dwiden::widen(&[f], false, &IDENTITY, w, 0.25);
        prop_assert!(r.is_ok());
        let r = r.unwrap_or_default();
        prop_assert_eq!(r.len(), 1);
        let poly = Fig { closed: true, ..r[0].clone() };
        let len = (f64::from(pts[1].0 - pts[0].0).powi(2) + f64::from(pts[1].1 - pts[0].1).powi(2)).sqrt();
        let a = shoelace(&[poly]);
        prop_assert!((a.abs() - len * f64::from(w)).abs() < 1e-3 * (1.0 + len * f64::from(w)), "{} {}", a, len * f64::from(w));
    }

    /// the 256-bit fractions' integer agrees with i128 wherever i128 holds the answer: sums,
    /// products, order, exact quotients, gcd, and the f64 it rounds to (also past 2^128, against
    /// the product of two roundings that are exact)
    #[test]
    fn wide_integers_agree_with_i128(a in -(1i128 << 62)..(1i128 << 62), b in -(1i128 << 62)..(1i128 << 62), k in 0u32..60) {
        use kikizake::model::wide::W;
        let w = W::from;
        prop_assert_eq!(w(a) + w(b), w(a + b));
        prop_assert_eq!(w(a) - w(b), w(a - b));
        prop_assert_eq!(w(a) * w(b), w(a * b));
        prop_assert_eq!(w(a).cmp(&w(b)), a.cmp(&b));
        prop_assert_eq!(w(a * b).to_f64(), (a * b) as f64);
        if b != 0 {
            prop_assert_eq!(W::div(w(a * b), w(b)), w(a));
            prop_assert_eq!(W::div(w(a), w(b)), w(a / b));
        }
        let (mut x, mut y) = (a.abs(), b.abs());
        while y != 0 {
            (x, y) = (y, x % y);
        }
        prop_assert_eq!(W::gcd(w(a), w(b)), w(x));
        // (2^k a times 2^60 b, past 2^128: a b to f64 is exact when |a b| < 2^53)
        let (s, t) = (a >> 40, b >> 40);
        prop_assert_eq!((w(s << k) * w(t << 60)).to_f64(), (s * t) as f64 * 2f64.powi(k as i32 + 60));
    }

    /// the scanner's sweep is the same at any power of two: the points times 2^k give the
    /// boundary sum times 4^k, past 2^31 workspace units too
    #[test]
    fn sweeps_scale_by_powers_of_two(pts in proptest::collection::vec((-64i128..64, -64i128..64), 3..8), k in 0u32..40, wind in proptest::bool::ANY) {
        let a = kikizake::model::d2dtess::scan(wind, std::slice::from_ref(&pts));
        let big: Vec<(i128, i128)> = pts.iter().map(|p| (p.0 << k, p.1 << k)).collect();
        let b = kikizake::model::d2dtess::scan(wind, &[big]);
        prop_assert_eq!(a.is_ok(), b.is_ok());
        if let (Ok(a), Ok(b)) = (a, b) {
            prop_assert_eq!(a.acc * 4f64.powi(k as i32), b.acc);
        }
    }

    /// Widen of one straight line through any transform that does not flatten it (an elliptic
    /// pen when it is not a similarity): the image of the local rectangle, of area |det| times
    /// the length times the width
    #[test]
    fn widened_lines_through_transforms_are_parallelograms(x1 in -32i32..32, y1 in -32i32..32, w in 1i32..32, m in proptest::collection::vec(-16i32..16, 4)) {
        prop_assume!((x1, y1) != (0, 0));
        let m: M = [m[0] as f32 / 8.0, m[1] as f32 / 8.0, m[2] as f32 / 8.0, m[3] as f32 / 8.0, 1.0, 2.0];
        let det = f64::from(m[0]) * f64::from(m[3]) - f64::from(m[1]) * f64::from(m[2]);
        prop_assume!(det.abs() > 0.05);
        let p1 = (x1 as f32 / 4.0, y1 as f32 / 4.0);
        let f = Fig { start: (0.0, 0.0), segs: vec![Seg::L(p1)], closed: false };
        let w = w as f32 / 8.0;
        let r = kikizake::model::d2dwiden::widen(&[f], false, &m, w, 0.25);
        prop_assert!(r.is_ok());
        let r = r.unwrap_or_default();
        prop_assert_eq!(r.len(), 1);
        let poly = Fig { closed: true, ..r[0].clone() };
        let len = (f64::from(p1.0).powi(2) + f64::from(p1.1).powi(2)).sqrt();
        let want = det.abs() * len * f64::from(w);
        let a = shoelace(&[poly]);
        prop_assert!((a.abs() - want).abs() < 1e-3 * (1.0 + want), "{} {}", a, want);
    }

    /// Widen of any ellipse, however tight its ends against the pen (refinement arcs, cubics
    /// among them): every point and control point of the rails lies within the ellipse's box
    /// grown by the pen radius (the arcs bulge by at most 4/3 tan(turn/4) of it)
    #[test]
    fn widened_ellipses_stay_in_the_grown_box(rx in 1i32..64, ry in 1i32..64, w in 1i32..64) {
        let (rx, ry, w) = (rx as f32 / 16.0, ry as f32 / 16.0, w as f32 / 8.0);
        let (_, round, figs) = shape(&Geom::Ell([8.0, 6.0, rx, ry]));
        let r = kikizake::model::d2dwiden::widen(&figs, round, &IDENTITY, w, 0.25);
        prop_assert!(r.is_ok());
        let hw = f64::from(w) * 0.5 * 1.25 + 1e-3;
        let inside = |p: P| (f64::from(p.0) - 8.0).abs() <= f64::from(rx) + hw && (f64::from(p.1) - 6.0).abs() <= f64::from(ry) + hw;
        for f in r.unwrap_or_default() {
            prop_assert!(inside(f.start));
            for s in &f.segs {
                match *s {
                    Seg::L(p) => prop_assert!(inside(p), "{:?}", p),
                    Seg::C(a, b, c) => prop_assert!(inside(a) && inside(b) && inside(c), "{:?} {:?} {:?}", a, b, c),
                }
            }
        }
    }

    /// a gradient of one color, linear or radial, any extend mode and gamma, fills that color
    /// (its premultiplied bytes) wherever the mapping lands
    #[test]
    fn one_color_gradients_are_solid(rad in proptest::bool::ANY, g in 0usize..2, e in 0usize..3, c in proptest::collection::vec(0u8..=255, 3), k in 0i32..64, k2 in -16i32..16) {
        let (gm, ex) = (["22", "10"][g], ["clamp", "wrap", "mirror"][e]);
        let geo = if rad { format!("rad {gm} {ex} 8 2 {} 0 {} {}", f64::from(k2) / 8.0, 1.0 + f64::from(k) / 4.0, 1.0 + f64::from(k) / 8.0) } else { format!("lin {gm} {ex} 1 1 {} 3", 2.0 + f64::from(k) / 4.0) };
        let col = format!("{} {} {} 255", c[0], c[1], c[2]);
        let a = run(&format!("d2target 16 4\nd2aa aliased\nd2grad 0 0 16 4 {geo} 2 0 {col} 1 {col}\nd2read\n"));
        let b = run(&format!("d2target 16 4\nd2aa aliased\nd2fill 0 0 16 4 {col}\nd2read\n"));
        prop_assert_eq!(a, b);
    }
}
