//! Laws of the GDI+ widener and dashes (src/model/gpwiden.rs): a straight line widens to its
//! rectangle, a straight vertex adds one point a side, a rectangle's frame fills as the frame,
//! dashes lie on the line no longer than their pattern, every outline is well formed (Bezier
//! runs of three), the round join stays on the pen, and anchor caps sit on the line's end.
mod pairs;
use pairs::zip_eq;
use kikizake::model::gdiplus::{FillMode, PixelOffset, Round};
use kikizake::model::gppath::{self as path, Path, Pen};
use kikizake::model::gppath::Kind;
use kikizake::model::gpwiden::{self as wd, Anchor, Cap, Join};
use proptest::prelude::*;

const ID: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn round(down: bool) -> Round {
    if down { Round::Down } else { Round::Nearest }
}

fn widened(r: Round, pts: &[(f32, f32)], pen: &Pen) -> Path {
    let mut p = Path::new(0);
    p.add_lines(r, pts).unwrap();
    wd::widen(r, &p, pen, &ID, 0.25).unwrap().unwrap()
}

/// arrows at both ends leaving one point of the body fail the widening, none left draws the
/// caps alone (clean-room notes geometry.md 20, golden anchor-22)
#[test]
fn a_one_point_body_fails() {
    for r in [Round::Down, Round::Nearest] {
        let mut pen = Pen::new(0xff00_0000, 4.75);
        (pen.start_cap, pen.end_cap) = (0x14, 0x14);
        let widen = |pts: &[(f32, f32)]| {
            let mut p = Path::new(0);
            p.add_lines(r, pts).unwrap();
            wd::widen(r, &p, &pen, &ID, 0.25).unwrap()
        };
        assert_eq!(widen(&[(-0.25, 2.0), (1.75, 4.5), (6.25, 9.75)]).unwrap_err(), wd::Widened::Failed);
        assert_eq!(widen(&[(-0.25, 2.0), (6.25, 9.75)]).unwrap().figures().len(), 2);
        assert_eq!(widen(&[(-0.25, 2.0), (1.75, 4.5), (2.25, 5.0), (6.25, 9.75)]).unwrap().figures().len(), 2);
    }
}

#[test]
fn caps_and_joins_by_value() {
    assert_eq!((Cap::line(0), Cap::line(1), Cap::line(2), Cap::line(3)), (Cap::Flat, Cap::Square, Cap::Round, Cap::Triangle));
    for v in [-1, 4, 5, 15, 0x10, 0x15, 0x20, 0xf0, 0xff] {
        assert_eq!(Cap::line(v), Cap::Flat, "cap {v}");
    }
    assert_eq!([0x11, 0x12, 0x13, 0x14].map(Cap::line), [Anchor::Square, Anchor::Round, Anchor::Diamond, Anchor::Arrow].map(Cap::Anchor));
    assert_eq!((Cap::dash(1), Cap::dash(2), Cap::dash(3), Cap::dash(-1)), (Cap::Flat, Cap::Round, Cap::Triangle, Cap::Flat));
    assert_eq!([0, 1, 2, 3, 4, -1].map(Join::of), [Join::Miter, Join::Bevel, Join::Round, Join::MiterClipped, Join::Nothing, Join::Nothing]);
    assert!(wd::preset(0).is_none() && wd::preset(5).is_none() && wd::preset(-1).is_none());
    let pen = |dash: i64, arr: &[f32]| {
        let mut p = Pen::new(0xff00_0000, 1.0);
        (p.dash, p.dash_array) = (dash, arr.to_vec());
        p
    };
    assert!(wd::pattern(&pen(5, &[])).is_none() && wd::pattern(&pen(0, &[2.0])).is_none() && wd::pattern(&pen(6, &[2.0])).is_none());
    assert_eq!(wd::pattern(&pen(5, &[2.0, 0.5])), Some(&[2.0, 0.5][..]));
    assert_eq!(wd::pattern(&pen(2, &[2.0, 0.5])), Some(&[1.0, 1.0][..]));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    /// a flat-capped line widens to four points, both sides w / 2 off it: the rectangle
    #[test]
    fn a_line_widens_to_its_rectangle(a in (-50f32..50.0, -50f32..50.0), b in (-50f32..50.0, -50f32..50.0), w in 2f32..12.0, down: bool) {
        prop_assume!((a.0 - b.0).hypot(a.1 - b.1) > 0.5);
        let r = round(down);
        let p = widened(r, &[a, b], &Pen::new(0xff00_0000, w));
        prop_assert_eq!(p.pts.len(), 4);
        prop_assert!(p.pts[3].close);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy);
        for (i, q) in p.pts.iter().enumerate() {
            let d = ((q.x - a.0) * dy - (q.y - a.1) * dx) / l;
            prop_assert!((d.abs() - w / 2.0).abs() < 1e-3 * w.max(1.0), "point {} {}", i, d);
        }
    }
    /// a vertex on the straight line adds one point a side, whatever the join
    #[test]
    fn a_straight_vertex_adds_a_point(a in (-50f32..50.0, -50f32..50.0), d in (1f32..20.0, -20f32..20.0), w in 2f32..12.0, join in 0i64..4, down: bool) {
        let r = round(down);
        let b = (a.0 + d.0, a.1 + d.1);
        let c = (a.0 + 2.0 * d.0, a.1 + 2.0 * d.1);
        let mut pen = Pen::new(0xff00_0000, w);
        pen.join = join;
        prop_assert_eq!(widened(r, &[a, b, c], &pen).pts.len(), 6);
    }
    /// a rectangle with a miter join widens to its outer rectangle (a miter a corner) and its
    /// inner one (two bevel points a corner, crossing): filled Winding, the frame (integer
    /// corners, an even width)
    #[test]
    fn a_rectangle_frames(x in 2i32..6, y in 2i32..6, w in 4i32..9, h in 4i32..9, half in 1i32..2, down: bool) {
        let r = round(down);
        let mut p = Path::new(0);
        p.add_rectangle(r, (x as f32, y as f32, w as f32, h as f32)).unwrap();
        let o = wd::widen(r, &p, &Pen::new(0xff00_0000, 2.0 * half as f32), &ID, 0.25).unwrap().unwrap();
        prop_assert_eq!(o.pts.len(), 12);
        let cov = path::fill(r, &o, None, FillMode::Winding, None, PixelOffset::None, 20, 20).unwrap();
        for py in 0..20 {
            for px in 0..20 {
                let inside = |a: i32, b: i32, c: i32, d: i32| (a..c).contains(&(px as i32)) && (b..d).contains(&(py as i32));
                let want = inside(x - half, y - half, x + w + half, y + h + half) && !inside(x + half, y + half, x + w - half, y + h - half);
                prop_assert_eq!(cov.count(px, py) == 1, want, "pixel {} {}", px, py);
            }
        }
    }
    /// dashes of a line lie on it, in order, each at most its pattern's dash long; together they
    /// leave each gap
    #[test]
    fn dashes_lie_on_the_line(a in (-50f32..50.0, -50f32..50.0), b in (-50f32..50.0, -50f32..50.0), w in 2f32..6.0, style in 1i64..5, down: bool) {
        prop_assume!((a.0 - b.0).hypot(a.1 - b.1) > 1.0);
        let r = round(down);
        let mut pen = Pen::new(0xff00_0000, w);
        pen.dash = style;
        let p = widened(r, &[a, b], &pen);
        let long = wd::preset(style).unwrap().iter().copied().fold(0f32, f32::max) * w;
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy);
        for f in p.figures() {
            // (each dash's outline: within w / 2 of the line, within its dash along it)
            let along: Vec<f32> = f.iter().map(|q| ((q.x - a.0) * dx + (q.y - a.1) * dy) / l).collect();
            for q in f {
                let d = ((q.x - a.0) * dy - (q.y - a.1) * dx) / l;
                prop_assert!(d.abs() <= w / 2.0 + 1e-3 * w);
            }
            let (lo, hi) = along.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
            prop_assert!(hi - lo <= long + 1e-3 * l.max(1.0));
            prop_assert!(lo >= -1e-3 * l.max(1.0) && hi <= l + 1e-3 * l.max(1.0));
        }
    }
    /// every figure of every outline starts, closes, and its Bezier points come in threes
    #[test]
    fn outlines_are_well_formed(pts in proptest::collection::vec((-20f32..20.0, -20f32..20.0), 2..7), w in 0.5f32..8.0, join in -1i64..5, caps in (0i64..0x15, 0i64..0x15), closed: bool, down: bool) {
        let r = round(down);
        let mut p = Path::new(0);
        if closed && pts.len() > 2 { p.add_polygon(r, &pts).unwrap() } else { p.add_lines(r, &pts).unwrap() }
        let mut pen = Pen::new(0xff00_0000, w);
        (pen.join, pen.start_cap, pen.end_cap) = (join, caps.0, caps.1);
        let Ok(Ok(o)) = wd::widen(r, &p, &pen, &ID, 0.25) else { return Ok(()) };
        for f in o.figures() {
            prop_assert_eq!(f[0].kind, Kind::Start);
            prop_assert!(f.last().unwrap().close);
            let mut run = 0;
            for q in &f[1..] {
                match q.kind {
                    Kind::Bezier => run += 1,
                    Kind::Line | Kind::Start => {
                        prop_assert_eq!(run % 3, 0);
                        run = 0;
                    }
                }
            }
            prop_assert_eq!(run % 3, 0);
        }
        prop_assert!(path::fill(r, &o, None, FillMode::Winding, None, PixelOffset::None, 8, 8).is_ok());
    }
    /// a round join's points on the curve lie within the pen's half width of the polyline, its
    /// arcs' ends on the circle about the vertex
    #[test]
    fn a_round_join_stays_on_the_pen(a in (-20f32..20.0, -20f32..20.0), b in (-20f32..20.0, -20f32..20.0), c in (-20f32..20.0, -20f32..20.0), w in 2f32..8.0, down: bool) {
        prop_assume!((a.0 - b.0).hypot(a.1 - b.1) > 1.0 && (c.0 - b.0).hypot(c.1 - b.1) > 1.0);
        let r = round(down);
        let mut pen = Pen::new(0xff00_0000, w);
        pen.join = 2;
        let o = widened(r, &[a, b, c], &pen);
        let dist = |q: (f32, f32), s: (f32, f32), e: (f32, f32)| {
            let (dx, dy) = (e.0 - s.0, e.1 - s.1);
            let t = (((q.0 - s.0) * dx + (q.1 - s.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
            (q.0 - s.0 - t * dx).hypot(q.1 - s.1 - t * dy)
        };
        let mut run = 0;
        for q in &o.pts {
            run = match q.kind { Kind::Bezier => run + 1, Kind::Line | Kind::Start => 0 };
            if run % 3 == 0 {
                let d = dist((q.x, q.y), a, b).min(dist((q.x, q.y), b, c));
                prop_assert!(d < w / 2.0 + 1e-3 * w, "point {} {} at {}", q.x, q.y, d);
            }
            // (an arc's end points: on the circle about the vertex)
            if run > 0 && run % 3 == 0 {
                prop_assert!(((q.x - b.0).hypot(q.y - b.1) - w / 2.0).abs() < 1e-3 * w);
            }
        }
    }
    /// an arrow end cap: its tip on the line's end, the body pulled back by the pen width; a
    /// square anchor's corners max(w, 2) from the end
    #[test]
    fn anchors_sit_on_the_end(a in (-20f32..20.0, -20f32..20.0), d in (-20f32..20.0, -20f32..20.0), w in 1f32..3.0, down: bool) {
        let l = d.0.hypot(d.1);
        prop_assume!(l > 8.0 * w);
        let r = round(down);
        let b = (a.0 + d.0, a.1 + d.1);
        let mut pen = Pen::new(0xff00_0000, w);
        pen.end_cap = 0x14;
        let o = widened(r, &[a, b], &pen);
        let figs = o.figures();
        prop_assert_eq!(figs.len(), 2);
        prop_assert_eq!((figs[1][0].x, figs[1][0].y), b);
        prop_assert_eq!(figs[1].len(), 3);
        let far = figs[0].iter().map(|q| ((q.x - a.0) * d.0 + (q.y - a.1) * d.1) / l).fold(f32::MIN, f32::max);
        prop_assert!((far - (l - w)).abs() < 1e-3 * l, "body ends at {} of {}", far, l);
        pen.end_cap = 0x11;
        let o = widened(r, &[a, b], &pen);
        let figs = o.figures();
        prop_assert_eq!(figs[1].len(), 4);
        for q in figs[1] {
            prop_assert!(((q.x - b.0).hypot(q.y - b.1) - w.max(2.0)).abs() < 1e-3 * l);
        }
    }
}

/// SetFillPath / SetStrokePath's lengths of the notes' examples, both arches (geometry.md 21b)
#[test]
fn cap_lengths_as_measured() {
    for r in [Round::Down, Round::Nearest] {
        let v = [(-1.0, -1.0), (0.0, 0.0), (1.0, -1.0)];
        assert_eq!((wd::cap_length(r, &v, true), wd::cap_length(r, &v, false)), (1.0, 0.0));
        let v = [(-1.0, -2.0), (0.0, 0.0), (1.0, -2.0), (0.0, -1.0)];
        assert_eq!((wd::cap_length(r, &v, true), wd::cap_length(r, &v, false)), (1.0, 1.0));
        let v = [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)];
        assert_eq!((wd::cap_length(r, &v, true), wd::cap_length(r, &v, false)), (0.5, 0.5));
        assert_eq!(wd::cap_length(r, &[(-1.0, -3.0), (0.3, -0.7), (2.0, -5.0)], true), 3.666_666_7);
        // (no crossing below the x axis: NotImplemented for a fill path)
        let cap = |pts: Vec<(f32, f32)>| path::CustomCap::Path { stroke: false, pts, base_inset: 0.0, width_scale: 1.0, join: 0, caps: (0, 0) };
        assert_eq!(wd::cap_status(r, &cap(vec![(1.0, 1.0), (2.0, 2.0), (3.0, 1.0)])), 6);
        assert_eq!(wd::cap_status(r, &cap(vec![(-1.0, 1.0), (0.0, 0.0), (1.0, 1.0)])), 6);
        assert_eq!(wd::cap_status(r, &cap(vec![(-1.0, -1.0), (0.0, 0.0)])), 2);
        assert_eq!(wd::cap_status(r, &cap(vec![(-1.0, -1.0), (0.0, 0.0), (1.0, -1.0)])), 0);
    }
}

/// the open adjustable arrow H 3 W 2 on a width 2 pen: its V a stroke cap widened, the right
/// side's inner bevel points typed 0x41 (geometry.md 21g)
#[test]
fn an_open_arrow_as_measured() {
    for r in [Round::Down, Round::Nearest] {
        let mut pen = Pen::new(0xff00_0000, 2.0);
        pen.end_cap = 0xff;
        pen.custom[1] = Some(path::CustomCap::Arrow { height: 3.0, width: 2.0, filled: false, middle: 0.0 });
        let w = widened(r, &[(4.0, 8.0), (30.0, 8.0)], &pen);
        let types: Vec<u8> = w.pts.iter().map(|q| q.type_byte()).collect();
        assert_eq!(types, [0, 1, 1, 0x81, 0, 1, 1, 1, 0x41, 0x41, 0x81]);
        let want = [(24.31623, 5.05132), (33.16228, 8.0), (24.31623, 10.94868), (23.68377, 9.05132), (29.68377, 7.05132), (29.68377, 8.94868), (23.68377, 6.94868)];
        for (q, &(x, y)) in zip_eq(w.pts[4..].iter(), &want) {
            assert!((q.x - x).abs() < 1e-4 && (q.y - y).abs() < 1e-4, "{q:?} {x} {y}");
        }
        assert!((w.pts[1].x - 27.0).abs() < 1e-4);
        if r == Round::Nearest {
            assert_eq!(w.pts[1].x, 27.0);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    /// a custom fill cap's figure comes last, its fill path's types, within its scale of the end
    #[test]
    fn a_fill_cap_sits_on_the_end(a in (-20f32..20.0, -20f32..20.0), d in (-80f32..80.0, -80f32..80.0), w in 1f32..4.0, scale in 0.25f32..2.0, base in -0.5f32..2.0, k in 3usize..6, down: bool) {
        let l = d.0.hypot(d.1);
        let r = round(down);
        let s = (w * scale).max(2.0);
        prop_assume!(l > 4.0 * s);
        // (a fan of k points about the origin dipping below the x axis)
        let pts: Vec<(f32, f32)> = (0..k).map(|i| {
            let t = std::f32::consts::PI * (1.0 + i as f32) / (k as f32 + 1.0);
            (t.cos(), -t.sin() - 0.25)
        }).collect();
        let cap = path::CustomCap::Path { stroke: false, pts: pts.clone(), base_inset: base, width_scale: scale, join: 0, caps: (0, 0) };
        prop_assume!(wd::cap_status(r, &cap) == 0);
        let mut pen = Pen::new(0xff00_0000, w);
        pen.end_cap = 0xff;
        pen.custom[1] = Some(cap);
        let e = (a.0 + d.0, a.1 + d.1);
        let out = widened(r, &[a, e], &pen);
        let fig = &out.pts[out.pts.len() - k..];
        prop_assert_eq!(fig[0].kind, Kind::Start);
        prop_assert!(fig[k - 1].close);
        let reach = pts.iter().map(|q| q.0.hypot(q.1)).fold(0.0f32, f32::max) * s * 1.001 + 1e-3;
        for q in fig {
            prop_assert!((q.x - e.0).hypot(q.y - e.1) <= reach, "{:?} from {:?}", q, e);
        }
    }
    /// a dash offset a whole pattern further dashes the same, and an array set as Custom dashes
    /// as the style of the same array (geometry.md 6a, GpModF)
    #[test]
    fn a_pattern_further_dashes_the_same(n in 1usize..6, q in proptest::collection::vec(1i32..16, 6), o in -40i32..40, w in 0u8..3, style in 1i64..5, x1 in 8i32..60, y1 in -20i32..20, down: bool) {
        let r = round(down);
        let w = [1.0f32, 2.0, 4.0][w as usize];
        let arr: Vec<f32> = q[..n].iter().map(|&v| f32::from(i16::try_from(v).unwrap_or(1)) / 4.0).collect();
        let sum: f32 = arr.iter().sum();
        let quarter = |v: i32| f32::from(i16::try_from(v).unwrap_or(0)) / 4.0;
        let mut p = Path::new(0);
        prop_assert!(p.add_lines(r, &[(0.5, 0.5), (quarter(x1), quarter(y1))]).is_ok());
        let dashed = |dash: i64, arr: &[f32], off: f32| {
            let mut pen = Pen::new(0xff00_0000, w);
            pen.dash = dash;
            pen.dash_array = arr.to_vec();
            pen.dash_offset = off;
            wd::dashed(r, &p, &pen).ok().flatten().map(|d| d.figures().iter().map(|f| f.iter().map(|q| (q.x.to_bits(), q.y.to_bits())).collect::<Vec<_>>()).collect::<Vec<_>>())
        };
        // (as bits: a NaN coordinate equals itself)
        let a = dashed(5, &arr, quarter(o));
        prop_assert!(a.is_some());
        prop_assert_eq!(&a, &dashed(5, &arr, quarter(o) + sum));
        let preset = wd::preset(style).map(<[f32]>::to_vec).unwrap_or_default();
        prop_assert_eq!(dashed(style, &preset, quarter(o)), dashed(5, &preset, quarter(o)));
    }
}
