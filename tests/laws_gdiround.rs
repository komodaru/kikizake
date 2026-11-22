//! CreateEllipticRgn and CreateRoundRectRgn (spec/gdirgn.txt GDI.RGN.ELLIPSE,
//! GDI.RGN.ROUNDRECT) against their clauses: the figure built as the clauses say in 28.4 (the
//! box moved to (16 l - 4, 16 t - 4) - (16 r - 28, 16 b - 28), half axes (R - L + 1) >> 1 and
//! (T - B + 1) >> 1, control points the half axis times 0x729D7775 / 2^32 in, rounded down),
//! flattened (GDI.PATH.FLATTENHFD) and its pixels taken as a path's between pixels, alternate
//! (GDI.PATH.FILLFIX); exhaustive over boxes, corners in any order, and corner sizes.
use kikizake::model::bezier::flatten;
use kikizake::model::gdirgn::{elliptic, round_rect};
use kikizake::model::rgn::{fix_polygon_pixels, Rgn};

/// v times 0x729D7775 / 2^32, rounded down
fn k(v: i32) -> i32 {
    i32::try_from((i64::from(v) * 0x729D_7775).div_euclid(1 << 32)).unwrap()
}

/// the moved box and its half axes: (L, T, R, B), a, b (b not above 0: T is above B)
fn moved(l: i32, t: i32, r: i32, b: i32) -> ((i32, i32, i32, i32), i32, i32) {
    let (l, r, t, b) = (l.min(r), l.max(r), t.min(b), t.max(b));
    let bx = (16 * l - 4, 16 * t - 4, 16 * r - 28, 16 * b - 28);
    (bx, (bx.2 - bx.0 + 1) >> 1, (bx.1 - bx.3 + 1) >> 1)
}

/// a figure of lines and Béziers (None: a line to the point; Some: a Bézier's two controls)
/// flattened from its start and filled
fn region(start: (i32, i32), segs: &[(Option<[(i32, i32); 2]>, (i32, i32))]) -> Rgn {
    let mut poly = vec![start];
    for &(c, to) in segs {
        let from = *poly.last().unwrap();
        match c {
            None => poly.push(to),
            Some([p, q]) => poly.extend(flatten([from, p, q, to]).unwrap()),
        }
    }
    fix_polygon_pixels(&[poly], false)
}

/// the corners' figure of half axes (a, c) on the moved box, from the right side's upper end
/// counterclockwise: each corner a quarter of Ellipse's, the sides lines
fn rounded(bx: (i32, i32, i32, i32), a: i32, c: i32) -> Rgn {
    let (l, t, r, b) = bx;
    let (ka, kc) = (k(a), k(c));
    region((r, t - c), &[
        (Some([(r, t - kc), (r - ka, t)]), (r - a, t)),
        (None, (l + a, t)),
        (Some([(l + ka, t), (l, t - kc)]), (l, t - c)),
        (None, (l, b + c)),
        (Some([(l, b + kc), (l + ka, b)]), (l + a, b)),
        (None, (r - a, b)),
        (Some([(r - ka, b), (r, b + kc)]), (r, b + c)),
    ])
}

/// GDI.RGN.ELLIPSE: Ellipse's four quarters of the moved box from the right end of the middle
/// row (B + b), counterclockwise, the middle columns and rows L + a and T - b, B + b
fn ellipse_reference(l: i32, t: i32, r: i32, b: i32) -> Rgn {
    let ((fl, ft, fr, fb), a, hb) = moved(l, t, r, b);
    let (ka, kb) = (k(a), k(hb));
    region((fr, fb + hb), &[
        (Some([(fr, ft - kb), (fr - ka, ft)]), (fr - a, ft)),
        (Some([(fl + ka, ft), (fl, ft - kb)]), (fl, ft - hb)),
        (Some([(fl, fb + kb), (fl + ka, fb)]), (fl + a, fb)),
        (Some([(fr - ka, fb), (fr, fb + kb)]), (fr, fb + hb)),
    ])
}

/// GDI.RGN.ELLIPSE: every box with corners in -2..12, given in any order (0 wide or high
/// included)
#[test]
fn elliptic_region_is_the_clause_figure() {
    for l in -2..12 {
        for t in -2..12 {
            for r in -2..12 {
                for b in -2..12 {
                    assert_eq!(elliptic((l, t, r, b)).unwrap(), ellipse_reference(l, t, r, b), "box {l} {t} {r} {b}");
                }
            }
        }
    }
}

/// GDI.RGN.ROUNDRECT: the corners' half axes the box's times min(|w| / W, 1) and min(|h| / H,
/// 1) in float32 (both 0 when W or H is 0), rounded halves away from 0
fn round_reference(bx: (i32, i32, i32, i32), w: i32, h: i32) -> Rgn {
    let (m, a, hb) = moved(bx.0, bx.1, bx.2, bx.3);
    let (ww, hh) = ((bx.2 - bx.0).abs(), (bx.3 - bx.1).abs());
    let (fx, fy) = if ww == 0 || hh == 0 { (0.0f32, 0.0f32) } else { ((w.abs() as f32 / ww as f32).min(1.0), (h.abs() as f32 / hh as f32).min(1.0)) };
    let away = |v: f32| v.round() as i32;
    rounded(m, away(a as f32 * fx), away(hb as f32 * fy))
}

/// GDI.RGN.ROUNDRECT: every box with corners in 0..9 in any order and corner sizes in -3..12;
/// a corner 0 wide or high gives the box's rectangle a pixel smaller than CreateRectRgn's
#[test]
fn round_rect_region_is_the_clause_figure() {
    for l in 0..9 {
        for t in 0..9 {
            for r in 0..9 {
                for b in 0..9 {
                    for w in -3..12 {
                        for h in [-2, 0, 1, 2, 5, 8, 11] {
                            let got = round_rect((l, t, r, b), w, h).unwrap();
                            assert_eq!(got, round_reference((l, t, r, b), w, h), "box {l} {t} {r} {b} corner {w} {h}");
                            if (w == 0 || h == 0) && (r - l).abs() > 1 && (b - t).abs() > 1 {
                                let q = (l.min(r), t.min(b), l.max(r) - 1, t.max(b) - 1);
                                assert_eq!(got.rects(), Rgn::rect(q.0, q.1, q.2, q.3).rects(), "box {l} {t} {r} {b} corner {w} {h}");
                            }
                        }
                    }
                }
            }
        }
    }
}
