//! Cosmetic lines between 28.4 points (spec/gdiline.txt GDI.LINE.GIQ), exhaustive over every
//! pair of ends on a fine and a coarse grid, against the clause written in the rotated
//! coordinates u = x + y, v = x - y, where pixel (px, py)'s open diamond |x - cx| + |y - cy| <
//! 8 (28.4) is the open square |u - uc| < 8, |v - vc| < 8: a pixel is lit when some t in
//! [0, 1] puts the segment strictly inside that square (or the segment holds the diamond's
//! right or bottom corner), and the end is not in the diamond; a diagonal along diamond edges
//! takes the pixels whose lower (left for x - y, right for x + y) closed edge its u (v)
//! interval meets, less the one whose edge holds the end.
use kikizake::model::gdiline::{giq_pixels, Fix28};

/// a / b for b > 0, compared as cross products
#[derive(Clone, Copy)]
struct Q(i128, i128);

fn lt(a: Q, b: Q) -> bool {
    a.0 * b.1 < b.0 * a.1
}

/// whether some t in [0, 1] has lo < p0 + t dp < hi for both (p0, dp, lo, hi)
fn strictly_inside(axes: [(i128, i128, i128, i128); 2]) -> bool {
    // (the interval of t: its ends and whether each is open)
    let (mut lo, mut lo_open, mut hi, mut hi_open) = (Q(0, 1), false, Q(1, 1), false);
    for (p0, dp, a, b) in axes {
        if dp == 0 {
            if !(a < p0 && p0 < b) {
                return false;
            }
            continue;
        }
        // (t where the coordinate reaches a and b)
        let (ta, tb) = if dp > 0 { (Q(a - p0, dp), Q(b - p0, dp)) } else { (Q(p0 - a, -dp), Q(p0 - b, -dp)) };
        let (enter, leave) = if dp > 0 { (ta, tb) } else { (tb, ta) };
        if !lt(enter, lo) {
            (lo, lo_open) = (enter, true);
        }
        if !lt(hi, leave) {
            (hi, hi_open) = (leave, true);
        }
    }
    lt(lo, hi) || (!lt(hi, lo) && !lo_open && !hi_open)
}

/// whether the point lies on the closed segment a b
fn on_segment(a: (i128, i128), b: (i128, i128), p: (i128, i128)) -> bool {
    let (dx, dy, ux, uy) = (b.0 - a.0, b.1 - a.1, p.0 - a.0, p.1 - a.1);
    dx * uy == dy * ux && ux.min(0) >= dx.min(0) && ux.max(0) <= dx.max(0) && uy.min(0) >= dy.min(0) && uy.max(0) <= dy.max(0)
}

fn reference(a: (i64, i64), b: (i64, i64)) -> Vec<(i32, i32)> {
    let (a, b) = ((i128::from(a.0), i128::from(a.1)), (i128::from(b.0), i128::from(b.1)));
    let (ua, va, ub, vb) = (a.0 + a.1, a.0 - a.1, b.0 + b.1, b.0 - b.1);
    let px = |v: i128| v.div_euclid(16);
    let mut out = Vec::new();
    if a == b {
        return out;
    }
    // the diagonals along diamond edges: v (x - y) or u (x + y) constant and 8 off the centers'
    let along_v = va == vb && va.rem_euclid(16) == 8;
    let along_u = ua == ub && ua.rem_euclid(16) == 8;
    for y in px(a.1.min(b.1)) - 3..=px(a.1.max(b.1)) + 3 {
        for x in px(a.0.min(b.0)) - 3..=px(a.0.max(b.0)) + 3 {
            let (cx, cy) = (16 * x, 16 * y);
            let (uc, vc) = (cx + cy, cx - cy);
            let lit = if along_v {
                // (the lower left edge: v = vc - 8, u in [uc - 8, uc + 8])
                va == vc - 8 && ua.min(ub) <= uc + 8 && ua.max(ub) >= uc - 8 && !(uc - 8..=uc + 8).contains(&ub)
            } else if along_u {
                // (the lower right edge: u = uc + 8, v in [vc - 8, vc + 8])
                ua == uc + 8 && va.min(vb) <= vc + 8 && va.max(vb) >= vc - 8 && !(vc - 8..=vc + 8).contains(&vb)
            } else {
                let (right, bottom) = ((cx + 8, cy), (cx, cy + 8));
                let crosses = strictly_inside([(ua, ub - ua, uc - 8, uc + 8), (va, vb - va, vc - 8, vc + 8)]) || on_segment(a, b, right) || on_segment(a, b, bottom);
                let ends_in = ((ub - uc).abs() < 8 && (vb - vc).abs() < 8) || b == right || b == bottom;
                crosses && !ends_in
            };
            if lit {
                out.push((i32::try_from(x).unwrap(), i32::try_from(y).unwrap()));
            }
        }
    }
    out
}

fn model(a: (i64, i64), b: (i64, i64)) -> Vec<(i32, i32)> {
    let mut v = giq_pixels(Fix28(a.0, a.1), Fix28(b.0, b.1)).unwrap();
    v.sort_by_key(|p| (p.1, p.0));
    v
}

fn check(pts: &[(i64, i64)]) {
    for &a in pts {
        for &b in pts {
            assert_eq!(model(a, b), reference(a, b), "{a:?} -> {b:?}");
        }
    }
}

/// GDI.LINE.GIQ: every pair of ends in 0..24 (28.4, a pixel and a half each way)
#[test]
fn giq_is_the_diamond_rule_on_the_fine_grid() {
    let pts: Vec<(i64, i64)> = (0..=24).flat_map(|x| (0..=24).map(move |y| (x, y))).collect();
    check(&pts);
}

/// GDI.LINE.GIQ: every pair of ends in -20..60 in steps of 4 (whole and half pixels among
/// them, so the diagonals along diamond edges), lines up to five pixels long
#[test]
fn giq_is_the_diamond_rule_on_the_coarse_grid() {
    let pts: Vec<(i64, i64)> = (-5..=15).flat_map(|x| (-5..=15).map(move |y| (4 * x, 4 * y))).collect();
    check(&pts);
}
