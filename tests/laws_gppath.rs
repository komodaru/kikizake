//! Laws of the GDI+ path model (src/model/gppath.rs): the scan converter against an exact
//! point-in-polygon oracle, the fixed-point flattener's ends, bounds, and filling a path as its
//! own flattening.
use kikizake::model::gdiplus::{self as gp, FillMode, PixelOffset, Round};
use kikizake::model::gppath::{self as path, Fig, Path};
use proptest::prelude::*;

fn fm(v: bool) -> FillMode {
    if v { FillMode::Winding } else { FillMode::Alternate }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    /// the scan converter covers exactly the samples an exact point-in-polygon test holds:
    /// aliased the pixel's own corner (i, j), antialiased (i - 1/2 + a / 8, j - 1/2 + b / n)
    #[test]
    fn scan_matches_the_oracle(p in proptest::collection::vec((-64i32..320, -64i32..320), 3..8), wind: bool, aa in 0u32..3) {
        let rows = [None, Some(4), Some(8)][aa as usize];
        let (w, h) = (16, 16);
        let fig = Fig { pts: p.clone(), closed: true };
        let cov = path::rasterize(&[fig], fm(wind), rows, w, h);
        let poly = gp::Polygon::new(&p.iter().map(|&(x, y)| (x as f32 / 16.0, y as f32 / 16.0)).collect::<Vec<_>>()).unwrap();
        for y in 0..h {
            for x in 0..w {
                let want = match rows {
                    None => u32::from(poly.inside(fm(wind), x as f64, y as f64)),
                    Some(n) => (0..8).flat_map(|a| (0..n).map(move |b| (a, b))).filter(|&(a, b)| poly.inside(fm(wind), x as f64 - 0.5 + a as f64 / 8.0, y as f64 - 0.5 + b as f64 / n as f64)).count() as u32,
                };
                prop_assert_eq!(cov.count(x, y), want, "pixel {} {}", x, y);
            }
        }
    }
    /// GDI's fixed-point flattener starts after P0 and ends exactly on P3, and never leaves the
    /// box of the control points (16 integers of slack)
    #[test]
    fn flattener_ends_on_the_end(q in proptest::array::uniform4((-300i32..300, -300i32..300))) {
        let pts = path::bezier32(q).unwrap();
        prop_assert_eq!(*pts.last().unwrap(), q[3]);
        let (x0, x1) = (q.iter().map(|p| p.0).min().unwrap(), q.iter().map(|p| p.0).max().unwrap());
        let (y0, y1) = (q.iter().map(|p| p.1).min().unwrap(), q.iter().map(|p| p.1).max().unwrap());
        for &(x, y) in &pts {
            prop_assert!(x >= x0 - 1 && x <= x1 + 1 && y >= y0 - 1 && y <= y1 + 1);
        }
    }
    /// past 14 bits the 32-bit flattener refuses
    #[test]
    fn flattener_refuses_past_its_range(a in 0i32..100, d in 0x3ff0i32..0x10000) {
        prop_assert!(path::bezier32([(a, 0), (a + d, 0), (a, 1), (a, 2)]).is_none());
    }
    /// every point lies in the bounds, sizes not negative
    #[test]
    fn bounds_hold_every_point(rect in (-50f32..50.0, -50f32..50.0, 0.5f32..60.0, 0.5f32..60.0), start in -720f32..720.0, sweep in -400f32..400.0, down: bool) {
        let r = if down { Round::Down } else { Round::Nearest };
        let mut p = Path::new(0);
        p.add_pie(r, rect, start, sweep).unwrap();
        let (x, y, w, h) = p.bounds(r);
        prop_assert!(w >= 0.0 && h >= 0.0);
        for q in &p.pts {
            prop_assert!(q.x >= x && q.y >= y && q.x <= x + w + 1e-3 && q.y <= y + h + 1e-3);
        }
    }
    /// a fill flattens at a flatness of 1/4: a path fills as its own FlattenPath(0.25) does
    #[test]
    fn fill_is_fill_of_the_flattened(rect in (-4f32..20.0, -4f32..20.0, 0.5f32..20.0, 0.5f32..20.0), start in -360f32..360.0, sweep in -400f32..400.0, down: bool, aa in 0u32..3, wind: bool) {
        let r = if down { Round::Down } else { Round::Nearest };
        let rows = [None, Some(4), Some(8)][aa as usize];
        let mut p = Path::new(0);
        p.add_pie(r, rect, start, sweep).unwrap();
        p.add_ellipse(r, (rect.1, rect.0, rect.3, rect.2)).unwrap();
        let a = path::fill(r, &p, None, fm(wind), rows, PixelOffset::None, 16, 16).unwrap();
        p.flatten(r, None, 0.25).unwrap();
        let b = path::fill(r, &p, None, fm(wind), rows, PixelOffset::None, 16, 16).unwrap();
        prop_assert_eq!(a, b);
    }
    /// the diamond sweep of a polyline is one closed loop: it starts and ends on the first
    /// point's inner side, P0 - O[k]
    #[test]
    fn sweep_is_a_closed_loop(p in proptest::collection::vec((-200i32..200, -200i32..200), 1..8)) {
        let mut q = p.clone();
        q.dedup();
        match path::sweep(&p) {
            None => prop_assert_eq!(q.len(), 1),
            Some(o) => {
                prop_assert!(q.len() >= 2);
                prop_assert_eq!(o.first(), o.last());
                prop_assert!(o.len() >= 4 * q.len());
            }
        }
    }
    /// a two point path stroked one pixel wide is the line drawn: aliased by the DDA,
    /// antialiased or aliased-translucent by the sweep
    #[test]
    fn a_two_point_outline_is_the_line(a in (-4f32..20.0, -4f32..20.0), b in (-4f32..20.0, -4f32..20.0), down: bool, aa in 0u32..3, half: bool) {
        let r = if down { Round::Down } else { Round::Nearest };
        let rows = [None, Some(4), Some(8)][aa as usize];
        let po = if half { PixelOffset::Half } else { PixelOffset::None };
        let m = path::device(r, None, po);
        let mut p = Path::new(0);
        p.add_lines(r, &[a, b]).unwrap();
        prop_assert_eq!(path::outline_nominal(r, &p, &m, rows, 16, 16).unwrap(), path::line_nominal(r, &m, a, b, rows, 16, 16));
        prop_assert_eq!(path::outline_aliased(r, &p, &m, 16, 16), path::line_aliased(r, &m, a, b, 16, 16));
    }
    /// a closed figure stroked by the DDA is each of its segments drawn, the closing one too
    #[test]
    fn a_polygon_outline_is_its_segments(v in proptest::collection::vec((-4f32..20.0, -4f32..20.0), 3..7), down: bool) {
        let r = if down { Round::Down } else { Round::Nearest };
        let m = path::device(r, None, PixelOffset::None);
        let mut p = Path::new(0);
        p.add_polygon(r, &v).unwrap();
        let pts: Vec<(f32, f32)> = p.pts.iter().map(|q| (q.x, q.y)).collect();
        let Ok(got) = path::outline_aliased(r, &p, &m, 16, 16) else { return Ok(()) };
        for y in 0..16 {
            for x in 0..16 {
                let any = (0..pts.len()).any(|i| path::line_aliased(r, &m, pts[i], pts[(i + 1) % pts.len()], 16, 16).is_ok_and(|c| c.count(x, y) == 1));
                prop_assert_eq!(got.count(x, y) == 1, any, "pixel {} {}", x, y);
            }
        }
    }
}

/// a path's types: AddLines joins the open figure, a point on its last one dropped
#[test]
fn lines_join_the_open_figure() {
    let r = Round::Nearest;
    let mut p = Path::new(0);
    p.add_lines(r, &[(0.0, 0.0), (1.0, 1.0)]).unwrap();
    p.add_lines(r, &[(1.0, 1.0), (2.0, 0.0)]).unwrap();
    p.add_lines(r, &[(3.0, 3.0)]).unwrap();
    assert_eq!(p.pts.iter().map(|q| q.type_byte()).collect::<Vec<_>>(), [0, 1, 1, 1]);
    p.close_figure();
    p.add_lines(r, &[(3.0, 3.0)]).unwrap();
    assert_eq!(p.pts.iter().map(|q| q.type_byte()).collect::<Vec<_>>(), [0, 1, 1, 0x81, 0]);
    assert!(p.add_lines(r, &[]).is_err());
    assert!(p.add_beziers(r, &[(0.0, 0.0); 5]).is_err());
    assert!(p.add_polygon(r, &[(0.0, 0.0); 2]).is_err());
}

/// RasterizerCeiling: 1/16 rounded half up, then ceiled
#[test]
fn ceiling_rounds_then_ceils() {
    for (v, want) in [(85.5f32, 86), (20.5, 21), (78.0, 78), (-0.5, 0), (-0.53125, 0), (0.03, 0), (0.031, 0), (0.03125, 1), (-0.03125, 0), (0.0, 0)] {
        assert_eq!(path::ceil16(Round::Nearest, v), want, "{v}");
    }
    assert_eq!(path::ceil16(Round::Nearest, f32::INFINITY), i32::MIN.wrapping_add(15) >> 4);
}
