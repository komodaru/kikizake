//! A polygon region's bound when its last rows are empty (spec/gdirgn.txt GDI.RGN.LOOSE),
//! exhaustive over quadrilaterals, pairs of triangles and 28.4 triangles: the bound is the
//! pixels' own (top, left and right always), except that a polygon other than one figure of at
//! most 40 points whose y goes up once and down once round it has its bottom at the row after
//! its edges' lowest end (in 28.4 rounded up); CombineRgn's RGN_COPY keeps that bottom.
use kikizake::case::parse_cases;
use kikizake::model::rgn::{fix_polygon_pixels, polygon, polygon_pixels, Rgn};
use kikizake::model::run_case;

/// the pixels' own bound (0 0 0 0 for none), from the pixels themselves
fn tight(r: &Rgn, lo: i32, hi: i32) -> Option<(i32, i32, i32, i32)> {
    let px: Vec<(i32, i32)> = (lo..hi).flat_map(|y| (lo..hi).map(move |x| (x, y))).filter(|q| r.contains(q.0, q.1)).collect();
    let (l, r) = (px.iter().map(|q| q.0).min()?, px.iter().map(|q| q.0).max()? + 1);
    let (t, b) = (px.iter().map(|q| q.1).min()?, px.iter().map(|q| q.1).max()? + 1);
    Some((l, t, r, b))
}

/// one figure whose y, read round it with level steps left out, rises once and falls once
fn up_once_down_once(polys: &[Vec<(i32, i32)>]) -> bool {
    let [p] = polys else { return false };
    if p.len() > 40 {
        return false;
    }
    let steps: Vec<i32> = (0..p.len()).map(|i| (p[(i + 1) % p.len()].1 - p[i].1).signum()).filter(|s| *s != 0).collect();
    // (the places round the figure where the direction changes)
    let changes = (0..steps.len()).filter(|&i| steps[i] != steps[(i + 1) % steps.len()]).count();
    changes <= 2
}

fn want(polys: &[Vec<(i32, i32)>], pixels: &Rgn, lowest: i32, lo: i32, hi: i32) -> (i32, i32, i32, i32) {
    let Some((l, t, r, b)) = tight(pixels, lo, hi) else { return (0, 0, 0, 0) };
    (l, t, r, if up_once_down_once(polys) { b } else { b.max(lowest) })
}

fn check_int(polys: &[Vec<(i32, i32)>]) {
    let lowest = polys.iter().flatten().map(|q| q.1).max().unwrap();
    let w = want(polys, &polygon_pixels(polys, false), lowest, -1, 6);
    assert_eq!(polygon(polys, false).unwrap().bounds(), w, "{polys:?}");
}

/// GDI.RGN.LOOSE: every quadrilateral on the grid 0..4 (CreatePolygonRgn, ALTERNATE)
#[test]
fn polygon_bound_is_loose_only_off_the_fast_figures() {
    let g: Vec<(i32, i32)> = (0..4).flat_map(|x| (0..4).map(move |y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    check_int(&[vec![a, b, c, d]]);
                }
            }
        }
    }
}

/// GDI.RGN.LOOSE: every pair of triangles from a set (two figures: never the fast converter)
#[test]
fn two_figure_bound_reaches_the_lowest_end() {
    let tris = [
        vec![(0, 0), (3, 0), (0, 3)],
        vec![(1, 1), (4, 1), (1, 1)],
        vec![(0, 2), (2, 2), (1, 5)],
        vec![(2, 0), (2, 4), (3, 4)],
        vec![(0, 0), (1, 1), (0, 4)],
        vec![(1, 0), (4, 3), (4, 3)],
    ];
    for a in &tris {
        for b in &tris {
            check_int(&[a.clone(), b.clone()]);
        }
    }
}

/// GDI.RGN.LOOSE: every triangle of 28.4 corners on a grid (PathToRegion of a path between
/// pixels), the lowest end rounded up to a row
#[test]
fn fix_polygon_bound_rounds_the_lowest_end_up() {
    let v = [0, 5, 8, 16, 21, 33, 40];
    let g: Vec<(i32, i32)> = v.iter().flat_map(|&x| v.iter().map(move |&y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                let p = vec![vec![a, b, c]];
                let lowest = (a.1.max(b.1).max(c.1) + 15).div_euclid(16);
                let r = fix_polygon_pixels(&p, false);
                assert_eq!(r.bounds(), want(&p, &r, lowest, -1, 5), "{p:?}");
            }
        }
    }
    // (and quadrilaterals, the ones whose y turns four times never the fast converter's)
    let v = [0, 5, 16, 21, 40];
    let g: Vec<(i32, i32)> = v.iter().flat_map(|&x| v.iter().map(move |&y| (x, y))).collect();
    let mut loose = 0;
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    let p = vec![vec![a, b, c, d]];
                    let lowest = (a.1.max(b.1).max(c.1).max(d.1) + 15).div_euclid(16);
                    let r = fix_polygon_pixels(&p, false);
                    let w = want(&p, &r, lowest, -1, 5);
                    loose += usize::from(tight(&r, -1, 5).is_some_and(|t| t != w));
                    assert_eq!(r.bounds(), w, "{p:?}");
                }
            }
        }
    }
    assert!(loose > 0, "no loose bound among them");
}

/// GDI.RGN.LOOSE: RGN_COPY keeps the loose bottom, GetRgnBox answering it; every
/// quadrilateral on the grid 0..4
#[test]
fn copy_keeps_the_loose_bottom() {
    let g: Vec<(i32, i32)> = (0..4).flat_map(|x| (0..4).map(move |y| (x, y))).collect();
    let mut loose = 0;
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    let p = [vec![a, b, c, d]];
                    if tight(&polygon_pixels(&p, false), -1, 6).is_some_and(|t| t != polygon(&p, false).unwrap().bounds()) {
                        loose += 1;
                    }
                    let pts = format!("{} {} {} {} {} {} {} {}", a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1);
                    let t = format!("case loose\nrgn 1 poly 1 {pts}\nrgn 2 null\ncombine 2 1 1 5\nrgnbox 1\nrgnbox 2\nend\n");
                    let out = run_case(&parse_cases(&t)[0]).unwrap();
                    let n = out.len();
                    assert_eq!(out[n - 1], out[n - 2], "{pts}");
                    let bx = polygon(&[vec![a, b, c, d]], false).unwrap().bounds();
                    assert!(out[n - 1].ends_with(&format!("{} {} {} {}", bx.0, bx.1, bx.2, bx.3)), "{pts}: {:?}", out[n - 1]);
                }
            }
        }
    }
    assert!(loose > 0, "no loose bound among them");
}
