//! CreatePolygonRgn's scan conversion (spec/gdirgn.txt GDI.RGN.POLYSCAN), exhaustive over every
//! triangle with corners on a 6 by 6 grid and every quadrilateral (self-crossing ones included)
//! on a 4 by 4 grid, both fill modes, against the rule written from the clause: on scanline y
//! (whole, the pixels' top) each edge whose [upper, lower) rows hold y crosses at the exact
//! x; X11's integer stepping takes the first whole x at or right of it, so a pixel x lies in a
//! span when ALTERNATE counts an odd number of crossings x_e with ceil(x_e) <= x, WINDING a
//! nonzero sum of their directions.
use kikizake::model::rgn::polygon_pixels;

/// ceil(a / b), b > 0
fn ceil_div(a: i64, b: i64) -> i64 {
    -((-a).div_euclid(b))
}

/// the clause's pixels of the polygon on rows and columns lo..hi
fn reference(p: &[(i32, i32)], winding: bool, lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in lo..hi {
        // (each edge crossing row y: the whole x X11 steps it to, and +1 going down, -1 up)
        let mut xs: Vec<(i64, i32)> = Vec::new();
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if a.1 == b.1 {
                continue;
            }
            let (top, bot, dir) = if a.1 < b.1 { (a, b, 1) } else { (b, a, -1) };
            if y < top.1 || y >= bot.1 {
                continue;
            }
            let (dx, dy) = (i64::from(bot.0 - top.0), i64::from(bot.1 - top.1));
            xs.push((i64::from(top.0) + ceil_div(i64::from(y - top.1) * dx, dy), dir));
        }
        for x in lo..hi {
            let left: Vec<i32> = xs.iter().filter(|e| e.0 <= i64::from(x)).map(|e| e.1).collect();
            let inside = if winding { left.iter().sum::<i32>() != 0 } else { left.len() % 2 == 1 };
            if inside {
                out.push((x, y));
            }
        }
    }
    out
}

fn model(p: &[(i32, i32)], winding: bool, lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let r = polygon_pixels(&[p.to_vec()], winding);
    let mut out = Vec::new();
    for y in lo..hi {
        for x in lo..hi {
            if r.contains(x, y) {
                out.push((x, y));
            }
        }
    }
    out
}

/// GDI.RGN.POLYSCAN: every triangle on the grid 0..6, both modes
#[test]
fn polygon_region_is_the_x11_rule_for_every_triangle() {
    let g: Vec<(i32, i32)> = (0..6).flat_map(|x| (0..6).map(move |y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                let p = [a, b, c];
                for w in [false, true] {
                    assert_eq!(model(&p, w, -1, 7), reference(&p, w, -1, 7), "{p:?} winding {w}");
                }
            }
        }
    }
}

/// GDI.RGN.POLYSCAN: every quadrilateral on the grid 0..4 (rectangles, bow ties, figures
/// doubling back), both modes
#[test]
fn polygon_region_is_the_x11_rule_for_every_quadrilateral() {
    let g: Vec<(i32, i32)> = (0..4).flat_map(|x| (0..4).map(move |y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    let p = [a, b, c, d];
                    for w in [false, true] {
                        assert_eq!(model(&p, w, -1, 5), reference(&p, w, -1, 5), "{p:?} winding {w}");
                    }
                }
            }
        }
    }
}
