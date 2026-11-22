//! Filling and outlining figures whose points fall between pixels (spec/gdipath.txt
//! GDI.PATH.FILLFIX, GDI.PATH.STROKEFILLFIX) and Ellipse / RoundRect outside a path
//! (spec/gdiline.txt GDI.SHAPE.ELLIPSEPEN, GDI.SHAPE.ROUNDPEN).
use kikizake::case::parse_cases;
use kikizake::model::rgn::fix_polygon_pixels;
use kikizake::model::run_case;

/// GDI.PATH.FILLFIX from the clause: on row y the crossings of the edges whose [upper, lower)
/// 28.4 rows hold 16y, at their exact x there; pixel x is inside when the crossings with x_e
/// <= 16x (its left edge in [left, right)) are odd in number (ALTERNATE) or sum to a nonzero
/// winding (WINDING)
fn reference(p: &[(i32, i32)], winding: bool, lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for y in lo..hi {
        let yc = i64::from(y) * 16;
        // (each crossing as n / d, d > 0, and its direction)
        let mut xs: Vec<(i64, i64, i32)> = Vec::new();
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if a.1 == b.1 {
                continue;
            }
            let (top, bot, dir) = if a.1 < b.1 { (a, b, 1) } else { (b, a, -1) };
            let (t, bt) = (i64::from(top.1), i64::from(bot.1));
            if yc < t || yc >= bt {
                continue;
            }
            let n = i64::from(top.0) * (bt - t) + i64::from(bot.0 - top.0) * (yc - t);
            xs.push((n, bt - t, dir));
        }
        for x in lo..hi {
            let left = i64::from(x) * 16;
            let at_or_left: Vec<i32> = xs.iter().filter(|e| e.0 <= left * e.1).map(|e| e.2).collect();
            let inside = if winding { at_or_left.iter().sum::<i32>() != 0 } else { at_or_left.len() % 2 == 1 };
            if inside {
                out.push((x, y));
            }
        }
    }
    out
}

fn model(p: &[(i32, i32)], winding: bool, lo: i32, hi: i32) -> Vec<(i32, i32)> {
    let r = fix_polygon_pixels(&[p.to_vec()], winding);
    (lo..hi).flat_map(|y| (lo..hi).map(move |x| (x, y))).filter(|q| r.contains(q.0, q.1)).collect()
}

/// GDI.PATH.FILLFIX: every triangle with corners on a grid of 28.4 values (whole pixels,
/// halves and odd sixteenths), both fill modes
#[test]
fn fix_fill_is_the_crossing_rule_for_every_triangle() {
    let v = [0, 5, 8, 13, 16, 24, 31, 40];
    let g: Vec<(i32, i32)> = v.iter().flat_map(|&x| v.iter().map(move |&y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for w in [false, true] {
                    let p = [a, b, c];
                    assert_eq!(model(&p, w, -1, 4), reference(&p, w, -1, 4), "{p:?} winding {w}");
                }
            }
        }
    }
}

/// GDI.PATH.FILLFIX: every quadrilateral (self-crossing ones too) on a coarser 28.4 grid
#[test]
fn fix_fill_is_the_crossing_rule_for_every_quadrilateral() {
    let v = [0, 7, 16, 25, 33];
    let g: Vec<(i32, i32)> = v.iter().flat_map(|&x| v.iter().map(move |&y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    for w in [false, true] {
                        let p = [a, b, c, d];
                        assert_eq!(model(&p, w, -1, 4), reference(&p, w, -1, 4), "{p:?} winding {w}");
                    }
                }
            }
        }
    }
}

fn dump(body: &str) -> Vec<String> {
    let t = format!("case fill\ndib 0 12 12 8\npen 0 0 1 #ff0000\n{body}\ndump 0\nend\n");
    let c = parse_cases(&t);
    let out = run_case(&c[0]).unwrap_or_else(|e| panic!("{body}: {e:?}"));
    let at = out.iter().rposition(|l| l.starts_with("px ")).unwrap();
    out[at..].to_vec()
}

/// GDI.PATH.STROKEFILLFIX: StrokeAndFillPath of figures between pixels (an ellipse and a
/// closed Bézier figure) is FillPath of the path, then each figure's closed outline in 28.4 as
/// StrokePath draws the same path with its figures closed; both fill modes
#[test]
fn stroke_and_fill_is_the_fill_then_the_closed_outline() {
    let shapes: Vec<String> = (1..=6)
        .flat_map(|w| (1..=6).map(move |h| (w, h)))
        .flat_map(|(w, h)| {
            [format!("ellipse 0 1 2 {} {}", 1 + w, 2 + h), format!("moveto 0 1 1\npolybezierto 0 {w} 0 {} {h} 1 {}\nclosefig 0", w + 3, h + 2)]
        })
        .collect();
    for s in &shapes {
        for mode in [1, 2] {
            let head = format!("brush 0 solid #00ff00\npolyfill 0 {mode}");
            let both = dump(&format!("{head}\nbeginpath 0\n{s}\nendpath 0\nstrokefill 0"));
            let apart = dump(&format!("{head}\nbeginpath 0\n{s}\nendpath 0\nfillpath 0\nbeginpath 0\n{s}\nclosefig 0\nendpath 0\nstrokepath 0"));
            assert_eq!(both, apart, "{s} mode {mode}");
        }
    }
}

/// GDI.SHAPE.ELLIPSEPEN, GDI.SHAPE.ROUNDPEN: Ellipse and RoundRect with a pen draw the path's
/// figure (counterclockwise) as StrokePath does with the null brush, as StrokeAndFillPath does
/// with a brush; every box in 0..10 given left to right and top to bottom, more than a pixel
/// each way, RoundRect corners 1..6
#[test]
fn ellipse_and_round_rect_draw_their_path() {
    for l in 0..10 {
        for t in 0..10 {
            for r in l + 1..=10 {
                for b in t + 1..=10 {
                    let bx = format!("{l} {t} {r} {b}");
                    let mut calls = vec![format!("ellipse 0 {bx}")];
                    if (l + t + r + b) % 2 == 0 {
                        calls.extend([(1, 1), (3, 2), (6, 6), (2, 5)].iter().map(|(w, h)| format!("roundrect 0 {bx} {w} {h}")));
                    }
                    for c in &calls {
                        for (brush, path_op) in [("null", "strokepath"), ("solid #00ff00", "strokefill")] {
                            let head = format!("brush 0 {brush}");
                            let drawn = dump(&format!("{head}\n{c}"));
                            let pathed = dump(&format!("{head}\nbeginpath 0\n{c}\nendpath 0\n{path_op} 0"));
                            assert_eq!(drawn, pathed, "{c} brush {brush}");
                        }
                    }
                }
            }
        }
    }
}
