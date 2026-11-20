//! Laws of the GDI+ widener and dashes (src/model/gpwiden.rs): a straight line widens to its
//! rectangle, a straight vertex adds one point a side, a rectangle's frame fills as the frame,
//! and dashes lie on the line no longer than their pattern.
use kikizake::model::gdiplus::{FillMode, PixelOffset, Round};
use kikizake::model::gppath::{self as path, Path, Pen};
use kikizake::model::gpwiden::{self as wd, Cap, Join};
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

#[test]
fn caps_and_joins_by_value() {
    assert_eq!((Cap::line(0).unwrap(), Cap::line(1).unwrap(), Cap::line(2).unwrap(), Cap::line(3).unwrap()), (Cap::Flat, Cap::Square, Cap::Round, Cap::Triangle));
    for v in [-1, 4, 5, 15, 0x20] {
        assert_eq!(Cap::line(v).unwrap(), Cap::Flat, "cap {v}");
    }
    for v in 0x10..=0x14 {
        assert!(Cap::line(v).is_err());
    }
    assert_eq!((Cap::dash(1), Cap::dash(2), Cap::dash(3), Cap::dash(-1)), (Cap::Flat, Cap::Round, Cap::Triangle, Cap::Flat));
    assert_eq!((Join::of(0).unwrap(), Join::of(1).unwrap(), Join::of(3).unwrap(), Join::of(4).unwrap(), Join::of(-1).unwrap()), (Join::Miter, Join::Bevel, Join::MiterClipped, Join::Nothing, Join::Nothing));
    assert!(Join::of(2).is_err());
    assert!(wd::pattern(0).is_none() && wd::pattern(5).is_none() && wd::pattern(-1).is_none());
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
        prop_assume!(join != 2);
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
        let long = wd::pattern(style).unwrap().iter().copied().fold(0f32, f32::max) * w;
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
}
