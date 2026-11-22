//! WARP's aliased line (the rasterizer behind Direct3D 9 to 11 on Windows' runner): which pixels a
//! line of two snapped ends lights. From the clean-room notes on WARP's line setup and per-pixel
//! line coverage (project notes kikizake-re/d3d11/lines.md, sections 1.1 and 1.2); every step is
//! one f32 operation rounded to nearest even, no fused multiply-add, true divisions.
//!
//! The ends are in 1/256 pixel in Direct3D 10's coordinates (pixel (i, j)'s center at
//! (i + 0.5, j + 0.5)). Setup moves an end that lies in its pixel's diamond to 1/256 short of
//! the pixel's center (the start backwards, the end forwards), then a pixel is lit when the
//! segment crosses its center column (or row) at a distance in (-0.5, 0.5000001) from the center
//! along the other axis (row: (-0.5, 0.5) for an x-major line).

/// 0.5000001, the bound above on the crossing (0x3F000001)
const HI: f32 = f32::from_bits(0x3F00_0001);
/// the setup's nudge, 1/256
const NUDGE: f32 = 1.0 / 256.0;

/// v as f32 (exact: positions and pixel numbers stay below 2^24)
fn float(v: i32) -> f32 {
    v as f32
}

/// a == b as f32 compares them (WARP's own tests)
fn same(a: f32, b: f32) -> bool {
    a.total_cmp(&b).is_eq() || (a.abs().total_cmp(&0.0).is_eq() && b.abs().total_cmp(&0.0).is_eq())
}

/// ceil(v) - 0.5: the center of the pixel whose diamond can hold v
fn owner(v: f32) -> f32 {
    let n = v.round_ties_even();
    let n = if n < v { n + 1.0 } else { n };
    n - 0.5
}

/// the point is in the diamond of the pixel centered at c: the bottom half with its edge, the
/// top half without; on the middle row a y-major line's right corner too
fn in_diamond(p: (f32, f32), c: (f32, f32), xmajor: bool) -> bool {
    let d = (p.0 - c.0).abs() + (p.1 - c.1).abs();
    let inclusive = p.1 > c.1 || (same(p.1, c.1) && !xmajor && p.0 > c.0);
    if inclusive { d <= 0.5 } else { d < 0.5 }
}

/// the line's setup: start, direction (None: nothing is lit)
pub fn setup(a: (i32, i32), b: (i32, i32)) -> Option<((f32, f32), (f32, f32), bool)> {
    let fx = |v: i32| float(v) * (1.0 / 256.0);
    let (x0, y0, x1, y1) = (fx(a.0), fx(a.1), fx(b.0), fx(b.1));
    if same(x0, x1) && same(y0, y1) {
        return None;
    }
    let xmajor = (x0 - x1).abs() >= (y0 - y1).abs();
    let (dx, dy) = (x1 - x0, y1 - y0);
    let (c0, c1) = ((owner(x0), owner(y0)), (owner(x1), owner(y1)));
    let (in_s, in_e) = (in_diamond((x0, y0), c0, xmajor), in_diamond((x1, y1), c1, xmajor));
    if in_s && in_e && same(c0.0, c1.0) && same(c0.1, c1.1) {
        return None;
    }
    let (mut x, mut y, mut ddx, mut ddy) = (x0, y0, dx, dy);
    if in_s {
        let (u, v) = if xmajor {
            let u = (x0 - c0.0) + if dx > 0.0 { NUDGE } else { -NUDGE };
            (u, (dy / dx) * u)
        } else {
            let v = (y0 - c0.1) + if dy > 0.0 { NUDGE } else { -NUDGE };
            ((dx / dy) * v, v)
        };
        x = x0 - u;
        y = y0 - v;
        ddx = dx + u;
        ddy = dy + v;
    }
    if in_e {
        let (u, v) = if xmajor {
            let u = ((x + ddx) - c1.0) + if ddx > 0.0 { NUDGE } else { -NUDGE };
            (u, (ddy / ddx) * u)
        } else {
            let v = ((y + ddy) - c1.1) + if ddy > 0.0 { NUDGE } else { -NUDGE };
            ((ddx / ddy) * v, v)
        };
        ddx -= u;
        ddy -= v;
    }
    Some(((x, y), (ddx, ddy), xmajor))
}

/// the pixels of a w x h target a line from `a` to `b` (1/256 pixel, Direct3D 10's coordinates)
/// lights, row by row
pub fn pixels(a: (i32, i32), b: (i32, i32), w: i32, h: i32) -> Vec<(i32, i32)> {
    coverage(a, b, w, h).0
}

/// `pixels`, and whether a crossing fell within 2^-12 of a bound: a tie f32's rounding decided
pub fn coverage(a: (i32, i32), b: (i32, i32), w: i32, h: i32) -> (Vec<(i32, i32)>, bool) {
    let Some(((x0, y0), (dx, dy), xmajor)) = setup(a, b) else { return (Vec::new(), false) };
    let mut tie = false;
    let mut near = |e: f32| {
        tie |= (e.abs() - 0.5).abs() < 1.0 / 4096.0;
        e
    };
    let hi_row = if xmajor { 0.5 } else { HI };
    let (inv_dx, inv_dy) = (-1.0 / dx, -1.0 / dy);
    let mut lit = Vec::new();
    for j in 0..h {
        for i in 0..w {
            let (px, py) = (float(i) + 0.5, float(j) + 0.5);
            let (ea, eb) = (x0 - px, y0 - py);
            let t = ea * inv_dx;
            let column = (0.0..=1.0).contains(&t) && {
                let e = near(t * dy + eb);
                e > -0.5 && e < HI
            };
            let t2 = eb * inv_dy;
            let row = (0.0..=1.0).contains(&t2) && {
                let e = near(t2 * dx + ea);
                e > -0.5 && e < hi_row
            };
            if column || row {
                lit.push((i, j));
            }
        }
    }
    (lit, tie)
}
