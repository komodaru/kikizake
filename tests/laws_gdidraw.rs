//! Laws of the GDI drawing models beyond BitBlt (lines, regions, paths, transforms, blends,
//! DDBs): what the documentation states as properties, proptest and exhaustive over small
//! domains.
use proptest::prelude::*;
use kikizake::case::parse_cases;
use kikizake::model::gdiblend::{alpha, mix};
use kikizake::model::gdiline::{line_pixels, rop2};
use kikizake::model::gdixform::{multiply, page, IDENTITY};
use kikizake::model::rgn::{Rgn, SetOp, COMPLEXREGION, NULLREGION, SIMPLEREGION};
use kikizake::model::{self, run_case};

/// SetROP2's codes: R2_BLACK 0, R2_NOTMERGEPEN, ..., R2_COPYPEN = P, R2_NOT = !D, R2_NOP = D,
/// R2_WHITE all ones, and each one a bitwise function of (P, D)
#[test]
fn rop2_is_the_truth_table() {
    let (p, d) = (0xF0F0_F0F0u32, 0xCCCC_CCCCu32);
    assert_eq!(rop2(1, p, d), 0);
    assert_eq!(rop2(16, p, d), !0);
    assert_eq!(rop2(13, p, d), p);
    assert_eq!(rop2(6, p, d), !d);
    assert_eq!(rop2(11, p, d), d);
    assert_eq!(rop2(7, p, d), p ^ d); // R2_XORPEN
    assert_eq!(rop2(9, p, d), p & d); // R2_MASKPEN
    assert_eq!(rop2(15, p, d), p | d); // R2_MERGEPEN
    assert_eq!(rop2(4, p, d), !p); // R2_NOTCOPYPEN
    for code in 1..=16i64 {
        for bit in 0..4u32 {
            let (pb, db) = (bit >> 1 & 1, bit & 1);
            let r = rop2(code, if pb == 1 { !0 } else { 0 }, if db == 1 { !0 } else { 0 });
            assert_eq!(r & 1, (code as u32 - 1) >> bit & 1, "code {code} P {pb} D {db}");
        }
    }
}

/// a line lights one pixel per step of its major axis, 8-connected, starting at its start,
/// ending next to its end; drawn the other way it lights the same pixels but for the two ends
/// (the halfway ties go to the same side whichever way a line runs)
#[test]
fn lines_are_connected_and_reversible() {
    for ax in -6..=6 {
        for ay in -6..=6 {
            for bx in -6..=6 {
                for by in -6..=6 {
                    let (a, b) = ((ax, ay), (bx, by));
                    let p = line_pixels(a, b);
                    assert_eq!(p.len() as i32, (bx - ax).abs().max((by - ay).abs()));
                    if p.is_empty() {
                        continue;
                    }
                    assert_eq!(p[0], a);
                    let last = *p.last().unwrap();
                    assert!((last.0 - b.0).abs() <= 1 && (last.1 - b.1).abs() <= 1 && last != b);
                    for w in p.windows(2) {
                        assert!((w[0].0 - w[1].0).abs() <= 1 && (w[0].1 - w[1].1).abs() <= 1);
                    }
                    let mut f: Vec<_> = p.clone();
                    f.push(b);
                    let mut r = line_pixels(b, a);
                    r.push(a);
                    f.sort();
                    r.sort();
                    assert_eq!(f, r, "{a:?} {b:?}");
                }
            }
        }
    }
}

fn arb_rect() -> impl Strategy<Value = (i32, i32, i32, i32)> {
    (-6i32..10, -6i32..10, -6i32..10, -6i32..10)
}

fn rgn_of(rs: &[(i32, i32, i32, i32)]) -> Rgn {
    rs.iter().fold(Rgn::empty(), |g, r| g.combine(&Rgn::rect(r.0, r.1, r.2, r.3), SetOp::Or))
}

/// a region's bands are its canonical form: bands apart or of different spans, spans apart
fn canonical(g: &Rgn) -> bool {
    g.bands.windows(2).all(|w| w[0].1 <= w[1].0 && !(w[0].1 == w[1].0 && w[0].2 == w[1].2))
        && g.bands.iter().all(|(t, b, s)| t < b && !s.is_empty() && s.iter().all(|x| x.0 < x.1) && s.windows(2).all(|w| w[0].1 < w[1].0))
}

proptest! {
    /// CombineRgn is the set operation on the pixels, and its result is canonical
    #[test]
    fn combine_is_the_set_operation(a in prop::collection::vec(arb_rect(), 0..4), b in prop::collection::vec(arb_rect(), 0..4), m in 1i64..5) {
        let (ra, rb) = (rgn_of(&a), rgn_of(&b));
        let op = SetOp::from_mode(m).unwrap();
        let r = ra.combine(&rb, op);
        prop_assert!(canonical(&ra) && canonical(&rb) && canonical(&r));
        for y in -7..11 {
            for x in -7..11 {
                let (pa, pb) = (ra.contains(x, y), rb.contains(x, y));
                let want = match m { 1 => pa && pb, 2 => pa || pb, 3 => pa != pb, _ => pa && !pb };
                prop_assert_eq!(r.contains(x, y), want);
            }
        }
        let n = r.rects().len();
        prop_assert_eq!(r.kind(), match n { 0 => NULLREGION, 1 => SIMPLEREGION, _ => COMPLEXREGION });
        // the same set has the same bands (EqualRgn on the data)
        let again = rb.combine(&ra, SetOp::Or).combine(&rb, SetOp::Diff);
        prop_assert_eq!(again, ra.combine(&rb, SetOp::Diff));
    }

    /// OffsetRgn moves every pixel; GetRgnBox bounds them all and touches each side
    #[test]
    fn offset_and_box(a in prop::collection::vec(arb_rect(), 1..4), dx in -9i32..9, dy in -9i32..9) {
        let g = rgn_of(&a);
        let o = g.offset_by(dx, dy);
        for y in -7..11 {
            for x in -7..11 {
                prop_assert_eq!(g.contains(x, y), o.contains(x + dx, y + dy));
            }
        }
        if g.kind() != NULLREGION {
            let b = g.bounds();
            for (l, t, r, bt) in g.rects() {
                prop_assert!(l >= b.0 && t >= b.1 && r <= b.2 && bt <= b.3);
            }
            prop_assert!(g.rects().iter().any(|r| r.0 == b.0) && g.rects().iter().any(|r| r.2 == b.2));
        } else {
            prop_assert_eq!(g.bounds(), (0, 0, 0, 0));
        }
    }

    /// AlphaBlend's constant alpha: 255 is the source, 0 the destination, and the result lies
    /// between them; per pixel alpha with an opaque source is the source
    #[test]
    fn alpha_bounds(s: u32, d: u32, a in 0u32..256) {
        for i in 0..4 {
            let (sc, dc) = (s >> (8 * i) & 255, d >> (8 * i) & 255);
            let m = mix(sc, dc, a);
            prop_assert!(m >= sc.min(dc) && m <= sc.max(dc));
        }
        prop_assert_eq!(alpha(s, d, 255, false), s);
        prop_assert_eq!(alpha(s, d, 0, false), d);
        prop_assert_eq!(alpha(s | 0xff00_0000, d, 255, true), s | 0xff00_0000);
    }

    /// CombineTransform with the identity on either side is the transform
    #[test]
    fn identity_combines(x in prop::array::uniform6(-64i32..64)) {
        let x = x.map(|v| v as f32 / 8.0);
        prop_assert_eq!(multiply(&IDENTITY, &x), x);
        prop_assert_eq!(multiply(&x, &IDENTITY), x);
    }

    /// LPtoDP's page transform: equal extents are a translation; otherwise each coordinate is
    /// within half a pixel and a sixteenth of the exact value, and monotone in the point
    #[test]
    fn page_rounding(a in -2000i32..2000, w in -300i32..300, v in -300i32..300, wo in -50i32..50, vo in -50i32..50) {
        prop_assume!(w != 0 && v != 0);
        prop_assert_eq!(page(a, w, w, wo, vo), a - wo + vo);
        let exact = (a - wo) as f64 * v as f64 / w as f64 + vo as f64;
        prop_assert!((page(a, w, v, wo, vo) as f64 - exact).abs() <= 0.5 + 1.0 / 16.0);
        let (p0, p1) = (page(a, w, v, wo, vo), page(a + 1, w, v, wo, vo));
        let up = (v > 0) == (w > 0);
        prop_assert!(if up { p1 >= p0 } else { p1 <= p0 }, "monotone");
    }

    /// whatever text comes in, the drawing models never panic
    #[test]
    fn never_panics(text in "(case [a-z]{1,3}\n|end\n|(dib|ddb|fill|dump|pen|extpen|moveto|lineto|polyline|polylineto|polygon|rectangle|ellipse|fillrect|framerect|invertrect|rop2|rgn|combine|rgndata|selclip|clipbox|exclude|offsetclip|mapmode|winext|vpext|winorg|lptodp|dptolp|gmode|setxform|modxform|getxform|beginpath|endpath|getpath|closefig|strokepath|alphablend|transparent|maskblt|align|extra|exttext|bmobject|bmbits|devcaps)( (-?[0-9]{1,3}|0x[0-9a-f]{1,8}|#[0-9a-f]{6}|solid|null|rect|mono|screen|compat|none|1.5)){0,13}\n){0,30}") {
        let cases = parse_cases(&text);
        for c in &cases {
            let _ = run_case(c);
        }
        let _ = model::run("x", &cases);
    }
}

/// what the models answer as Windows does: each suite at least this many cases, so a model that
/// stops answering (a case turned unmodeled) fails as surely as one that answers wrong
#[test]
fn models_keep_answering() {
    use kikizake::{case, compare, gen};
    let floor = [("lines", 202), ("region", 410), ("xform", 98), ("path", 34), ("text", 25), ("blend", 487), ("ddb", 14)];
    for (s, n) in floor {
        let cases = case::parse_cases(&gen::suite(s).unwrap());
        for arch in [32, 64] {
            model::set_arch(arch);
            let p = format!("{}/golden/{s}-{arch}/stdout.txt", env!("CARGO_MANIFEST_DIR"));
            let Ok(g) = std::fs::read_to_string(&p) else { continue };
            let r = compare::compare_text(&model::run(s, &cases), &g);
            // (cases Windows has not answered yet are left to tests/windows.rs)
            let differ = r.differ.iter().filter(|d| !d.contains("missing")).count();
            assert!(differ == 0 && r.same >= n, "{s}-{arch}: {} as Windows (at least {n}), {differ} differ", r.same);
        }
    }
    model::set_arch(32);
}
