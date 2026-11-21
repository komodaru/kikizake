//! Laws of the GDI drawing models beyond BitBlt (lines, regions, paths, transforms, blends,
//! DDBs): what the documentation states as properties, proptest and exhaustive over small
//! domains.
use proptest::prelude::*;
use kikizake::case::parse_cases;
use kikizake::model::gdiblend::{alpha, mix};
use kikizake::model::gdiline::{giq_pixels, line_pixels, rop2, Fix28};
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

    /// GIQ with whole ends is the integer line rule (both measured: the corners of the diamond
    /// it includes are the halfway ties' side); a 28.4 line never lights the pixel it ends in
    #[test]
    fn giq_of_whole_ends_is_the_integer_line(a in (-40i32..40, -40i32..40), b in (-40i32..40, -40i32..40), f in (-64i64..64, -64i64..64, -64i64..64, -64i64..64)) {
        let mut g = giq_pixels(Fix28(i64::from(a.0) * 16, i64::from(a.1) * 16), Fix28(i64::from(b.0) * 16, i64::from(b.1) * 16)).unwrap();
        let mut l = line_pixels(a, b);
        g.sort();
        l.sort();
        prop_assert_eq!(g, l);
        let (p, q) = (Fix28(i64::from(a.0) * 16 + f.0, i64::from(a.1) * 16 + f.1), Fix28(i64::from(b.0) * 16 + f.2, i64::from(b.1) * 16 + f.3));
        for t in giq_pixels(p, q).unwrap_or_default() {
            let (cx, cy) = (i64::from(t.0) * 16, i64::from(t.1) * 16);
            let ends_in = (q.0 - cx).abs() + (q.1 - cy).abs() < 8 || (q.0, q.1) == (cx + 8, cy) || (q.0, q.1) == (cx, cy + 8);
            prop_assert!(!ends_in, "the end's own pixel is left out");
        }
    }

    /// GIQ moves with whole pixels, diagonals along diamond edges included (their closed lower
    /// edges), and such a diagonal lights one pixel per pixel it advances, its end's left out
    #[test]
    fn giq_shifts_with_whole_pixels(a in (-64i64..64, -64i64..64), b in (-64i64..64, -64i64..64), s in (-5i64..6, -5i64..6), k in 1i64..60, dir in 0u8..4, f in 0i64..16) {
        let shift = |v: Vec<(i32, i32)>| { let mut v: Vec<(i32, i32)> = v.iter().map(|p| (p.0 + s.0 as i32, p.1 + s.1 as i32)).collect(); v.sort(); v };
        let moved = |p: Fix28| Fix28(p.0 + s.0 * 16, p.1 + s.1 * 16);
        // a diagonal along diamond edges: x - y or x + y is 8 modulo 16
        let (sx, sy) = [(1, 1), (-1, -1), (1, -1), (-1, 1)][dir as usize];
        let st = Fix28(a.0, if sx == sy { a.0 - 8 - 16 * f } else { 8 + 16 * f - a.0 });
        let en = Fix28(st.0 + sx * k, st.1 + sy * k);
        for (p, q) in [(Fix28(a.0, a.1), Fix28(b.0, b.1)), (st, en)] {
            let mut g = giq_pixels(moved(p), moved(q)).unwrap();
            g.sort();
            prop_assert_eq!(g, shift(giq_pixels(p, q).unwrap()));
        }
        let n = giq_pixels(st, en).unwrap().len() as i64;
        // (the pixels whose lower edge spans the run, less one if the end is on one)
        prop_assert!((n - (k / 16)).abs() <= 1, "{} pixels for a run of {}", n, k);
    }

    /// a whole turn of Arc from the right end of the middle row is Ellipse's figure: its points
    /// within a 1/16 pixel of Ellipse's (the first quarter is a part one, in float32; the middle
    /// two are Ellipse's own), then one Bézier of no length at the end, either way round
    #[test]
    fn arc_whole_turn_is_the_ellipse(l in -20i32..20, t in -20i32..20, w in 3i32..300, h in 2i32..150, cw in proptest::bool::ANY) {
        use kikizake::model::gdipath::ellipse_points;
        let (r, b) = (l + w, t + 2 * h);
        let at = (r + 5, t + h);
        let v = kikizake::model::gdiarc::arc((l, t, r, b), at, at, cw).unwrap();
        let c = ellipse_points(l, t, r, b, cw);
        prop_assert_eq!(v.beziers.len(), 5);
        prop_assert_eq!(v.start, c[0]);
        for (q, g) in v.beziers.iter().take(4).enumerate() {
            for (i, p) in g.iter().enumerate() {
                let e = c[3 * q + 1 + i];
                if q == 1 || q == 2 {
                    prop_assert_eq!(*p, e);
                } else {
                    prop_assert!((p.0 - e.0).abs() <= 1 && (p.1 - e.1).abs() <= 1, "quarter {}: {:?} vs {:?}", q, p, e);
                }
            }
        }
        prop_assert!(v.beziers[4].iter().all(|p| *p == c[12]), "the last Bézier has no length");
    }

    /// RoundRect with corners as large as the box (or larger) is Ellipse, its sides of no length
    #[test]
    fn round_rect_of_the_whole_box_is_the_ellipse(l in -20i32..20, t in -20i32..20, w in 3i32..30, h in 3i32..30, more in (0i32..40, 0i32..40), cw in proptest::bool::ANY) {
        use kikizake::model::gdipath::{ellipse_points, round_rect_points};
        let (r, b) = (l + w, t + h);
        let q = round_rect_points((l, t, r, b), (2 * w + more.0, 2 * h + more.1), cw);
        let e = ellipse_points(l, t, r, b, cw);
        for i in [4, 8, 12] {
            prop_assert_eq!(q[i], q[i - 1], "a side of no length");
        }
        let without: Vec<(i32, i32)> = q.iter().enumerate().filter(|(i, _)| ![4, 8, 12].contains(i)).map(|(_, p)| *p).collect();
        prop_assert_eq!(without, e.to_vec());
    }

    /// drawing through a viewport origin is drawing shifted by it (PatBlt, Rectangle, LineTo)
    #[test]
    fn viewport_origin_shifts_drawing(o in (-6i32..7, -6i32..7), a in (-4i32..14, -4i32..14), d in (-6i32..9, -6i32..9), kind in 0u8..3) {
        let draw = |dx: i32, dy: i32| match kind {
            0 => format!("patblt 0 {} {} {} {} 0xf00021", a.0 + dx, a.1 + dy, d.0, d.1),
            1 => format!("rectangle 0 {} {} {} {}", a.0 + dx, a.1 + dy, a.0 + d.0 + dx, a.1 + d.1 + dy),
            _ => format!("moveto 0 {} {}\nlineto 0 {} {}", a.0 + dx, a.1 + dy, a.0 + d.0 + dx, a.1 + d.1 + dy),
        };
        let head = "dib 0 16 16 8\npen 0 0 1 #ffffff\nbrush 0 solid #ff0000";
        let mapped = parse_cases(&format!("case m\n{head}\nvporg 0 {} {}\n{}\ndump 0\nend\n", o.0, o.1, draw(0, 0)));
        let plain = parse_cases(&format!("case m\n{head}\n{}\ndump 0\nend\n", draw(o.0, o.1)));
        let (m, p) = (run_case(&mapped[0]), run_case(&plain[0]));
        let (Ok(m), Ok(p)) = (m, p) else { panic!("{:?}", (run_case(&mapped[0]).err(), run_case(&plain[0]).err())) };
        // (the dump is the last lines; the mapped case answers one line more, for SetViewportOrgEx)
        prop_assert_eq!(&m[m.len() - 17..], &p[p.len() - 17..]);
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
    use kikizake::{case, compare, generators};
    let floor = [("lines", 443), ("region", 571), ("xform", 670), ("path", 679), ("text", 171), ("blend", 964), ("ddb", 59)];
    for (s, n) in floor {
        let cases = case::parse_cases(&generators::suite(s).unwrap());
        for arch in [32, 64] {
            model::set_arch(arch);
            let p = format!("{}/golden/{s}-{arch}/stdout.txt", env!("CARGO_MANIFEST_DIR"));
            let Ok(g) = std::fs::read_to_string(&p) else { continue };
            let r = compare::compare_text(&model::run(s, &cases), &g);
            // (differences are tests/windows.rs's: cases changed since Windows answered differ
            // until it answers again)
            assert!(r.same >= n, "{s}-{arch}: {} as Windows (at least {n})", r.same);
        }
    }
    model::set_arch(32);
}

proptest! {
    /// GDI's flattener (HFD) ends exactly at the Bézier's end point for curves within 256
    /// pixels, within 6/16 pixel beyond (its 18.14 steps lose bits there), and stays
    /// inside its control points' box (a Bézier lies in its control points' convex hull)
    #[test]
    fn flatten_ends_at_the_end(o in (-40000i32..40000, -40000i32..40000), p in prop::array::uniform4((0i32..0x4000, 0i32..0x4000))) {
        let p = p.map(|q| (q.0 + o.0, q.1 + o.1));
        let v = kikizake::model::bezier::flatten(p).expect("in the flattener's space");
        let e = v.last().copied().unwrap();
        let far = p.iter().flat_map(|q| [q.0 - o.0, q.1 - o.1]).any(|v| v >= 0x1000);
        let off = if far { 6 } else { 0 };
        prop_assert!((e.0 - p[3].0).abs() <= off && (e.1 - p[3].1).abs() <= off, "ends at {:?}, not {:?}", e, p[3]);
        let (l, r) = (p.iter().map(|q| q.0).min().unwrap(), p.iter().map(|q| q.0).max().unwrap());
        let (t, b) = (p.iter().map(|q| q.1).min().unwrap(), p.iter().map(|q| q.1).max().unwrap());
        for q in &v {
            let k = off.max(1);
            prop_assert!(l - k <= q.0 && q.0 <= r + k && t - k <= q.1 && q.1 <= b + k, "{q:?} outside {p:?}");
        }
    }

    /// a fixed point polygon fill: an axis-aligned rectangle of whole pixels is that rectangle,
    /// in either fill mode, and moving a polygon by whole pixels moves its pixels
    #[test]
    fn fix_fill_rectangles_and_moves(l in -20i32..20, t in -20i32..20, w in 1i32..20, h in 1i32..20, dx in -5i32..5, dy in -5i32..5, pts in prop::collection::vec((-400i32..400, -400i32..400), 3..7), winding: bool) {
        let s = 16;
        let rect = vec![(l * s, t * s), ((l + w) * s, t * s), ((l + w) * s, (t + h) * s), (l * s, (t + h) * s)];
        prop_assert_eq!(kikizake::model::rgn::fix_polygon_pixels(&[rect], winding), Rgn::rect(l, t, l + w, t + h));
        let moved: Vec<(i32, i32)> = pts.iter().map(|q| (q.0 + dx * s, q.1 + dy * s)).collect();
        let mut a = kikizake::model::rgn::fix_polygon_pixels(&[pts], winding);
        a.offset(dx, dy);
        prop_assert_eq!(a, kikizake::model::rgn::fix_polygon_pixels(&[moved], winding));
    }
}

proptest! {
    /// LPtoDP / DPtoLP through a whole world transform (integer scales or a swap, whole
    /// offsets) under MM_TEXT are exact, cold or warm
    #[test]
    fn world_whole_maps_are_exact_both_ways(k in (-4i32..5, -4i32..5), sw in any::<bool>(), d in (-50i32..50, -50i32..50), p in (-500i32..500, -500i32..500), warm in any::<bool>()) {
        // an integer world (scale or swap, whole offsets) under MM_TEXT: LPtoDP and DPtoLP exact
        prop_assume!(k.0 != 0 && k.1 != 0);
        let (a, b) = (k.0 as f32, k.1 as f32);
        let world = if sw { [0.0, b, a, 0.0, d.0 as f32, d.1 as f32] } else { [a, 0.0, 0.0, b, d.0 as f32, d.1 as f32] };
        let m = kikizake::model::gdixform::Map { mode: 1, gmode: 2, world, warm, ..Default::default() };
        let q = m.lp_to_dp(p).unwrap();
        let want = if sw { (p.1 * k.0 + d.0, p.0 * k.1 + d.1) } else { (p.0 * k.0 + d.0, p.1 * k.1 + d.1) };
        prop_assert_eq!(q, want);
        if k.0.abs() == 1 && k.1.abs() == 1 {
            prop_assert_eq!(m.dp_to_lp(q).unwrap(), p);
        }
    }
    /// MM_ISOTROPIC shrinks one axis to within half a unit of the other's ratio, and the stored
    /// extents read back unchanged
    #[test]
    fn isotropic_shrinks_one_axis_to_the_others_ratio(w in (1i32..3000, 1i32..3000), v in (-3000i32..3000, -3000i32..3000), sw in (any::<bool>(), any::<bool>())) {
        prop_assume!(v.0 != 0 && v.1 != 0);
        let w = (if sw.0 { -w.0 } else { w.0 }, if sw.1 { -w.1 } else { w.1 });
        let m = kikizake::model::gdixform::Map { mode: 7, wext: w, vext: v, unsettled: true, ..Default::default() };
        let e = m.ve();
        let (rx, ry) = ((f64::from(v.0) / f64::from(w.0)).abs(), (f64::from(v.1) / f64::from(w.1)).abs());
        if rx > ry {
            prop_assert_eq!(e.1, v.1);
            prop_assert!((f64::from(e.0.abs()) - ry * f64::from(w.0.abs())).abs() <= 0.5 + 1e-9);
        } else {
            prop_assert_eq!(e.0, v.0);
            prop_assert!((f64::from(e.1.abs()) - rx * f64::from(w.1.abs())).abs() <= 0.5 + 1e-9);
        }
        // once stored, read again unchanged (no drift: iso-0)
        let s = kikizake::model::gdixform::Map { vext: e, unsettled: false, ..m };
        prop_assert_eq!(s.ve(), e);
    }
    /// DPtoLP: with each window extent a whole multiple k of the viewport's, the inverse is
    /// exact, Lx = (Dx - xVO) k + xWO (the f32 inverse of a ratio 1/k is exact for small k)
    #[test]
    fn dptolp_whole_ratios_are_exact(kx in -64i32..64, ky in -64i32..64, vx in 1i32..64, vy in 1i32..64, wo in (-500i32..500, -500i32..500), vo in (-500i32..500, -500i32..500), d in (-2000i32..2000, -2000i32..2000)) {
        prop_assume!(kx != 0 && ky != 0);
        let m = kikizake::model::gdixform::Map { mode: 8, wext: (kx * vx, ky * vy), vext: (vx, vy), worg: wo, vorg: vo, ..Default::default() };
        let l = m.dp_to_lp(d).unwrap();
        prop_assert_eq!(l, ((d.0 - vo.0) * kx + wo.0, (d.1 - vo.1) * ky + wo.1));
    }
    /// a font the suite writes measures consistently: tmHeight is ascent + descent, A + B + C
    /// is the advance width, the extent is the widths' sum, GetTextExtentExPoint's running
    /// widths are the extent's prefix sums cut at the maximum, and bold adds its pixel to every
    /// advance that is not 0
    #[test]
    fn font_measures_add_up(seed in 0u32..4096, h in -40i64..40, bold in any::<bool>(), max in 0i64..200) {
        use kikizake::model::gdifont::{abc, extent, extent_ex, metrics, width, Slant, TextFont, Weight};
        let fnt = kikizake::generators::dwrite::font(0x5eed_0000 + seed);
        let p = kikizake::ttf::parse(&fnt.bytes()).unwrap();
        let f = TextFont { file: 0, height: h, weight: if bold { Weight::Bold } else { Weight::Normal }, slant: Slant::Upright };
        let Some(t) = metrics(&p, f) else { return Ok(()) };
        prop_assert_eq!(t.height, t.ascent + t.descent);
        prop_assert!(t.descent >= 0);
        let s: Vec<u32> = (0x20..0x7f).collect();
        let mut sum = 0;
        for &c in &s {
            let w = width(&p, f, c).unwrap();
            sum += w;
            if let Some(x) = abc(&p, f, c) {
                prop_assert_eq!(x.a + x.b + x.c, w);
                prop_assert!(x.b >= 1);
            }
            let n = width(&p, TextFont { weight: Weight::Normal, ..f }, c).unwrap();
            let adv = p.hmtx[kikizake::ttf::glyph_of(&p, c) as usize].0;
            prop_assert_eq!(w - n, if bold && adv > 0 { 1 } else { 0 });
        }
        prop_assert_eq!(extent(&p, f, &s, 0).unwrap(), (sum, t.height));
        let (fit, dx, e) = extent_ex(&p, f, &s, max, 0).unwrap();
        prop_assert_eq!(e, (sum, t.height));
        prop_assert_eq!(dx.len(), fit);
        let mut run = 0;
        for (i, &c) in s.iter().enumerate() {
            run += width(&p, f, c).unwrap();
            if i < fit {
                prop_assert_eq!(dx[i], run);
                prop_assert!(run <= max);
            } else {
                prop_assert!(i > fit || run > max);
            }
        }
    }
    /// a glyph's bitmaps agree with its measures: GGO_METRICS' box is GGO_BITMAP's, every
    /// pixel lit lies within GetCharABCWidths' black box columns, a GGO_GRAY8 value is at
    /// most 64, the buffer is its rows of whole DWORDs, and TextOut of the one character lights
    /// the bitmap's pixels within the box's rows
    #[test]
    fn glyph_bitmaps_agree(seed in 0u32..4096, h in -40i64..-1, bold in any::<bool>(), c in 0x20u32..0x7f) {
        use kikizake::model::gdifont::{abc, glyph, metrics, text_pixels, Format, Slant, TextFont, Weight};
        let fnt = kikizake::generators::dwrite::font(0x5eed_0000 + seed);
        let p = kikizake::ttf::parse(&fnt.bytes()).unwrap();
        let f = TextFont { file: 0, height: h, weight: if bold { Weight::Bold } else { Weight::Normal }, slant: Slant::Upright };
        let (Some(m), Some(b), Some(g)) = (glyph(&p, f, c, Format::Metrics), glyph(&p, f, c, Format::Mono), glyph(&p, f, c, Format::Gray8)) else { return Ok(()) };
        prop_assert_eq!((m.black, m.origin, m.inc), (b.black, b.origin, b.inc));
        prop_assert!(m.bytes.is_empty() && m.size == 1);
        prop_assert_eq!(b.size, b.bytes.len());
        prop_assert_eq!(g.size, g.bytes.len());
        prop_assert!(g.bytes.iter().all(|&v| v <= 64));
        if b.size == 0 {
            return Ok(());
        }
        let stride = (b.black.0 as usize + 31) / 32 * 4;
        prop_assert_eq!(b.size % stride, 0);
        prop_assert_eq!(g.size % ((g.black.0 as usize + 3) / 4 * 4), 0);
        let rows = b.size / stride;
        let mut lit = Vec::new();
        for r in 0..rows {
            for x in 0..b.black.0 as usize {
                if b.bytes[r * stride + x / 8] & (0x80 >> (x % 8)) != 0 {
                    lit.push((b.origin.0 + x as i64, r));
                }
            }
        }
        if let Some(w) = abc(&p, f, c) {
            for &(x, _) in &lit {
                prop_assert!(w.a <= x && x < w.a + w.b, "pixel {} outside the black box {:?}", x, w);
            }
        }
        // TextOut at (0, 0): its pixels, by row, the bitmap's rows within the box
        let t = text_pixels(&p, f, &[c], 0, 0).unwrap();
        let asc = metrics(&p, f).unwrap().ascent;
        for &(x, y) in &t {
            let j = asc - y - 1;
            prop_assert!(j >= b.origin.1 - b.black.1 && j < b.origin.1);
            prop_assert!(lit.iter().any(|&(lx, _)| lx == x));
        }
        prop_assert!(!t.is_empty());
    }
    /// a larger em (lfHeight below 0) never measures narrower or shorter
    #[test]
    fn font_measures_grow_with_the_em(seed in 0u32..4096, h in 1i64..60, c in 0x20u32..0x7f) {
        use kikizake::model::gdifont::{metrics, width, Slant, TextFont, Weight};
        let fnt = kikizake::generators::dwrite::font(0x5eed_0000 + seed);
        let p = kikizake::ttf::parse(&fnt.bytes()).unwrap();
        let f = |h: i64| TextFont { file: 0, height: -h, weight: Weight::Normal, slant: Slant::Upright };
        let (Some(a), Some(b)) = (metrics(&p, f(h)), metrics(&p, f(h + 1))) else { return Ok(()) };
        prop_assert!(b.height >= a.height && b.ave >= a.ave && b.max >= a.max);
        prop_assert!(width(&p, f(h + 1), c).unwrap() >= width(&p, f(h), c).unwrap());
    }
}

proptest! {
    /// an elliptic region lies in the box moved up and left by a pixel, its bound tight (no
    /// rows of cancelled edges), the same for the box's corners in any order, and left to
    /// right as well as top to bottom it is one run a row
    #[test]
    fn elliptic_region_is_one_run_a_row_in_its_box(l in -50i32..50, t in -50i32..50, w in 0i32..120, h in 0i32..120, swap in (proptest::bool::ANY, proptest::bool::ANY)) {
        use kikizake::model::gdirgn::elliptic;
        let (r, b) = (l + w, t + h);
        let g = elliptic((l, t, r, b)).unwrap();
        let (a, c) = (if swap.0 { (r, l) } else { (l, r) }, if swap.1 { (b, t) } else { (t, b) });
        prop_assert_eq!(&elliptic((a.0, c.0, a.1, c.1)).unwrap(), &g);
        prop_assert_eq!(g.loose, None);
        for (top, bottom, s) in &g.bands {
            prop_assert_eq!(s.len(), 1);
            prop_assert!(l - 1 <= s[0].0 && s[0].1 <= r && t - 1 <= *top && *bottom <= b, "{:?} outside {:?}", (top, bottom, s), (l, t, r, b));
        }
    }

    /// a region's loose bottom (GDI.RGN.LOOSE) moves with OffsetRgn and goes in CombineRgn
    #[test]
    fn loose_bound_moves_and_combines_away(x in 1i32..20, y in 1i32..20, dx in -9i32..9, dy in -9i32..9) {
        use kikizake::model::rgn::{polygon, SetOp};
        // (a triangle with a zigzag sliver of no width hanging below it, not y-monotone: the
        // general converter's bound reaches the sliver's end)
        let g = polygon(&[vec![(0, 0), (2 * x, 0), (x, y), (x, y + 5), (x, y + 2), (x, y + 6), (x, y)]], false).unwrap();
        let low = g.bounds().3;
        prop_assert_eq!(low, y + 6);
        prop_assert!(g.bands.last().unwrap().1 <= y);
        let mut o = g.clone();
        o.offset(dx, dy);
        prop_assert_eq!(o.bounds().3, low + dy);
        let c = g.combine(&g, SetOp::Or);
        prop_assert_eq!(c.loose, None);
        prop_assert_eq!(c.bands, g.bands);
    }
}

proptest! {
    /// the synthesized italic leaves the advances and the font's metrics but tmItalic as they
    /// are, its black boxes moved right for a glyph above the baseline and left for one below
    /// it, and its glyph bitmaps' widths what GetCharABCWidths says
    #[test]
    fn italic_shears_but_keeps_the_advances(seed in 0u32..4096, h in -40i64..40, bold in any::<bool>()) {
        use kikizake::model::gdifont::{abc, glyph, metrics, width, Format, Slant, TextFont, Weight};
        let fnt = kikizake::generators::dwrite::font(0x5eed_0000 + seed);
        let p = kikizake::ttf::parse(&fnt.bytes()).unwrap();
        let u = TextFont { file: 0, height: h, weight: if bold { Weight::Bold } else { Weight::Normal }, slant: Slant::Upright };
        let i = TextFont { slant: Slant::Italic, ..u };
        let (Some(tu), Some(ti)) = (metrics(&p, u), metrics(&p, i)) else { return Ok(()) };
        prop_assert_eq!(ti.italic, 255);
        prop_assert_eq!(kikizake::model::gdifont::Tm { italic: 0, ..ti }, tu);
        for c in 0x20..0x7fu32 {
            prop_assert_eq!(width(&p, i, c), width(&p, u, c));
            let (Some(a), Some(b)) = (abc(&p, u, c), abc(&p, i, c)) else { continue };
            if a.black != kikizake::model::gdifont::Black::Box {
                continue;
            }
            let (bx, contours) = p.glyphs[kikizake::ttf::glyph_of(&p, c) as usize].as_ref().unwrap();
            // (a glyph whose header box is not its points' (the writer's lenient fonts): the
            // upright box is the header's, the italic one the points')
            let pts: Vec<(i16, i16)> = contours.iter().flatten().copied().collect();
            if pts.iter().map(|q| q.0).min() != Some(bx.0) || pts.iter().map(|q| q.0).max() != Some(bx.2) {
                continue;
            }
            // (above the baseline the shear moves every point right, below it left)
            if pts.iter().all(|q| q.1 >= 0) {
                prop_assert!(b.a >= a.a && b.a + b.b >= a.a + a.b, "{:?} left of {:?}", b, a);
            }
            if pts.iter().all(|q| q.1 <= 0) {
                prop_assert!(b.a <= a.a && b.a + b.b <= a.a + a.b, "{:?} right of {:?}", b, a);
            }
            if let Some(g) = glyph(&p, i, c, Format::Mono) && g.size > 0 && !bold {
                prop_assert!(g.black.0 <= b.b + 1, "{:?} vs {:?}", g, b);
            }
        }
    }
}

proptest! {
    /// GDI's inverse color table holds only the color table's indices, and each entry's own
    /// cell holds an entry at most as far from the cell's centre as it (GDI.GRAD.INDEXED)
    #[test]
    fn inverse_table_holds_near_entries(pal in prop::collection::vec(0u32..0x100_0000, 1..40)) {
        use kikizake::model::gdiinvcmap::{table, Cell};
        let t = table(&pal);
        let d = |e: u32, c: [i64; 3]| -> i64 {
            let ch = [i64::from(e >> 16 & 0xff), i64::from(e >> 8 & 0xff), i64::from(e & 0xff)];
            (0..3).map(|i| (ch[i] - 8 * c[i] - 4).pow(2)).sum()
        };
        for r in 0..32u8 {
            for g in 0..32u8 {
                for b in 0..32u8 {
                    if let Some(i) = t.get(Cell::of(r, g, b)) {
                        prop_assert!(usize::from(i) < pal.len());
                    }
                }
            }
        }
        for &e in &pal {
            let c = [(e >> 19 & 31) as u8, (e >> 11 & 31) as u8, (e >> 3 & 31) as u8];
            let i = t.get(Cell::of(c[0], c[1], c[2]));
            prop_assert!(i.is_some(), "{:06x}'s own cell is not written", e);
            let cc = c.map(i64::from);
            prop_assert!(d(pal[usize::from(i.unwrap())], cc) <= d(e, cc));
        }
    }
}
