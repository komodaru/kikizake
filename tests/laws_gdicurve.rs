//! Ellipse's and RoundRect's figures in a path (spec/gdipath.txt GDI.PATH.ELLIPSE,
//! GDI.PATH.ROUNDRECT) against the clauses written in exact integers: a control point's inset
//! is the radius times 1 - 4/3 (sqrt 2 - 1) = (7 - 4 sqrt 2) / 3, irrational for any radius
//! but 0, so its floor is the largest m with 3m <= rad (7 - 4 sqrt 2), that is
//! 32 rad^2 <= (7 rad - 3m)^2 with 7 rad - 3m >= 0, and its ceiling the floor plus one.
use kikizake::model::gdipath::{ellipse_points, inset_ceil as model_inset_ceil, inset_floor as model_inset_floor, round_rect_points};

/// floor(v (7 - 4 sqrt 2) / q), v >= 0, q > 0
fn inset_floor(v: i64, q: i64) -> i64 {
    let (v, q) = (i128::from(v), i128::from(q));
    let fits = |m: i128| 7 * v - q * m >= 0 && 32 * v * v <= (7 * v - q * m) * (7 * v - q * m);
    // (an estimate from below, 1.3431 < 7 - 4 sqrt 2, then up to the last m that fits)
    let mut m = (v * 13431 / 10000) / q - 1;
    while fits(m + 1) {
        m += 1;
    }
    assert!(fits(m));
    i64::try_from(m).unwrap()
}

/// rounded up: the floor plus one unless the value is 0
fn inset_ceil(v: i64, q: i64) -> i64 {
    inset_floor(v, q) + i64::from(v > 0)
}

/// x taken to 32 bits, two's complement (the value in -2^31 .. 2^31 equal to x mod 2^32)
fn w32(x: i64) -> i64 {
    (x + (1 << 31)).rem_euclid(1 << 32) - (1 << 31)
}

/// the inset v (7 - 4 sqrt 2) / 6 of a side of either sign, rounded down or (up) up
fn signed_inset(v: i64, up: bool) -> i64 {
    let floor = if v >= 0 { inset_floor(v, 6) } else { -inset_floor(-v, 6) - 1 };
    floor + i64::from(up && v != 0)
}

/// one axis of the box from the clause: the ends times 16 and their width W = 16 hi - 16 lo
/// taken to 32 bits; (16 lo, 16 hi - 16) where W > 0, (16 lo - 16, 16 hi) where W < 0, no
/// figure where W is 0 or -2^31
fn axis(lo: i64, hi: i64) -> Option<(i64, i64)> {
    let (a, b) = (w32(16 * lo), w32(16 * hi));
    match w32(b - a) {
        0 => None,
        w if w == -(1 << 31) => None,
        w if w > 0 => Some((a, w32(b - 16))),
        _ => Some((w32(a - 16), b)),
    }
}

/// Ellipse's 13 points from the clause: around (l, t) - (r - 1, b - 1) in 28.4 (l <= r, t <=
/// b), from the right end of the middle row counterclockwise (cw: mirrored top to bottom), each
/// control point the half axis times 1 - 4/3 (sqrt 2 - 1) in from the box's edge, across
/// rounded down, down the box rounded up counterclockwise and down clockwise; the centre the
/// near side plus half the side, truncated; every value taken to 32 bits; None: no figure
fn ellipse(l: i64, t: i64, r: i64, b: i64, cw: bool) -> Option<Vec<(i64, i64)>> {
    let ((fl, fr), (ft, fb)) = (axis(l, r)?, axis(t, b)?);
    let (vx, vy) = (w32(fr - fl), w32(fb - ft));
    // (Rust's / truncates toward 0)
    let (cx, cy) = (w32(fl + vx / 2), w32(ft + vy / 2));
    // (half the axis times the factor: the whole axis over 6)
    let (dx, dy) = (signed_inset(vx, false), signed_inset(vy, !cw));
    let mut c = vec![
        (fr, cy), (fr, ft + dy), (fr - dx, ft), (cx, ft),
        (fl + dx, ft), (fl, ft + dy), (fl, cy),
        (fl, fb - dy), (fl + dx, fb), (cx, fb),
        (fr - dx, fb), (fr, fb - dy), (fr, cy),
    ];
    for q in &mut c {
        *q = (w32(q.0), w32(q.1));
    }
    if cw {
        for q in &mut c {
            q.1 = w32(ft + fb - q.1);
        }
    }
    Some(c)
}

/// a corner's radius in 28.4: d times 8, times (n - 1) / n to the nearest (halves up), at most
/// half the box's 28.4 side
fn radius(d: i64, n: i64) -> i64 {
    let v = 16 * d * (n - 1);
    // (v / 2n, halves up: floor((v + n) / 2n))
    ((v + n).div_euclid(2 * n)).min(8 * (n - 1))
}

/// RoundRect's 16 points from the clause
fn round_rect(bx: (i64, i64, i64, i64), wh: (i64, i64), cw: bool) -> Vec<(i64, i64)> {
    let (l, t, r, b) = bx;
    let (fl, fr, ft, fb) = (16 * l, 16 * (r - 1), 16 * t, 16 * (b - 1));
    let (rx, ry) = (radius(wh.0, r - l), radius(wh.1, b - t));
    let ix = inset_floor(rx, 3);
    let iy = if cw { inset_floor(ry, 3) } else { inset_ceil(ry, 3) };
    let mut c = vec![
        (fr, ft + ry), (fr, ft + iy), (fr - ix, ft), (fr - rx, ft),
        (fl + rx, ft), (fl + ix, ft), (fl, ft + iy), (fl, ft + ry),
        (fl, fb - ry), (fl, fb - iy), (fl + ix, fb), (fl + rx, fb),
        (fr - rx, fb), (fr - ix, fb), (fr, fb - iy), (fr, fb - ry),
    ];
    if cw {
        for q in &mut c {
            q.1 = ft + fb - q.1;
        }
    }
    c
}

fn wide(p: &[(i32, i32)]) -> Vec<(i64, i64)> {
    p.iter().map(|q| (i64::from(q.0), i64::from(q.1))).collect()
}

/// GDI.PATH.ELLIPSE: every width and every height from 1 to 65536 pixels, both directions,
/// at origins about 0
#[test]
fn ellipse_points_are_the_exact_inset_for_every_size() {
    const N: i32 = 65536;
    for n in 1..=N {
        let (l, t) = (n % 5 - 2, n % 3 - 1);
        let (r, b) = (l + n, t + (N + 1 - n));
        for cw in [false, true] {
            let got = ellipse_points(l, t, r, b, cw).map(|c| wide(&c));
            let want = ellipse(i64::from(l), i64::from(t), i64::from(r), i64::from(b), cw);
            assert_eq!(got, want, "box {l} {t} {r} {b} cw {cw}");
        }
    }
}

/// GDI.PATH.ELLIPSE, GDI.PATH.ROUNDRECT: the model's inset is the irrational's floor (the
/// largest m with q m <= v (7 - 4 sqrt 2), that is 7 v - q m >= 0 and 32 v^2 <= (7 v - q m)^2)
/// and its ceiling: over 6 for every side Ellipse's box can have in 28.4 (the multiples of 16
/// below 2^31, and the side 16 less), over 3 for every radius up to 2^27
#[test]
fn inset_is_the_irrationals_floor_for_every_28_4_side() {
    let fits = |v: i128, q: i128, m: i128| 7 * v - q * m >= 0 && 32 * v * v <= (7 * v - q * m) * (7 * v - q * m);
    let check = |v: i32, q: i32| {
        let m = model_inset_floor(v, q);
        let (vv, qq, mm) = (i128::from(v), i128::from(q), i128::from(m));
        assert!(fits(vv, qq, mm) && !fits(vv, qq, mm + 1), "v {v} q {q}: {m}");
        assert_eq!(i128::from(model_inset_ceil(v, q)), mm + i128::from(v > 0), "v {v} q {q}");
    };
    for k in 0..=(i32::MAX / 16) {
        check(16 * k, 6);
        // (a side whose box wrapped: W + 16, W a multiple of 16)
        check((16 * k).saturating_sub(16).max(0), 6);
    }
    for v in 0..=(1 << 27) {
        check(v, 3);
    }
}

/// GDI.PATH.ELLIPSE: Ellipse's figure past 65536 pixels and past the 28.4 range (32-bit
/// arithmetic: the box's ends and width wrap): every side k from 1 to 64 and within 64 of
/// 65536, 82800 (where 0.5522847498's inset floors apart from the irrational), 2^27 and 2^28
/// (the 32-bit width about 2^31, -2^31 and 0), the measured sides and 2^30, 2^31 - 2; at an
/// origin near 0 and at corners near +-2^26, +-2^27, 2^28 and 2^30
#[test]
fn ellipse_points_past_65536_and_the_28_4_range() {
    let mut sides: Vec<i64> = (1..=64).collect();
    for c in [65536, 82800, 1 << 27, 1 << 28] {
        sides.extend(c - 64..=c + 64);
    }
    sides.extend([131072, 160117, 165599, 171081, 237434, 392068, 430442, 612486, 1 << 20, 1 << 24, 1 << 30, (1 << 31) - 2]);
    let origins: [i64; 8] = [-2, 3, -(1 << 26) + 1, 1 << 26, -(1 << 27) - 5, (1 << 27) - 5, (1 << 28) + 3, (1 << 30) + 1];
    let fit = |v: i64| i32::try_from(v).ok();
    let mut n = 0u64;
    for &w in &sides {
        for (i, &x) in origins.iter().enumerate() {
            // (the other axis a side read from the other end of the list, at another origin)
            let h = sides[(sides.len() - 1 + i * 37) % sides.len()];
            let y = origins[(i + 3) % origins.len()];
            let (Some(l), Some(t), Some(r), Some(b)) = (fit(x), fit(y), fit(x + w), fit(y + h)) else { continue };
            for (l, t, r, b) in [(l, t, r, b), (t, l, b, r)] {
                for cw in [false, true] {
                    let got = ellipse_points(l, t, r, b, cw).map(|c| wide(&c));
                    let want = ellipse(i64::from(l), i64::from(t), i64::from(r), i64::from(b), cw);
                    assert_eq!(got, want, "box {l} {t} {r} {b} cw {cw}");
                    n += 1;
                }
            }
        }
    }
    assert!(n > 5000, "{n}");
}

/// GDI.PATH.ROUNDRECT: every box side from 1 to 300 pixels with every corner size from 1 to
/// two past the side (the radius capped at half the box), both directions
#[test]
fn round_rect_points_are_the_exact_inset_for_every_corner() {
    let pairs: Vec<(i32, i32)> = (1..=300).flat_map(|n| (1..=n + 2).map(move |d| (n, d))).collect();
    let k = pairs.len();
    for (i, &(w, cwid)) in pairs.iter().enumerate() {
        // (the other axis from the list read backwards, so every pair meets several)
        let (h, chgt) = pairs[k - 1 - i];
        let (l, t) = (i32::try_from(i % 7).unwrap() - 3, i32::try_from(i % 5).unwrap() - 2);
        let bx = (l, t, l + w, t + h);
        for cw in [false, true] {
            let got = wide(&round_rect_points(bx, (cwid, chgt), cw));
            let want = round_rect((i64::from(bx.0), i64::from(bx.1), i64::from(bx.2), i64::from(bx.3)), (i64::from(cwid), i64::from(chgt)), cw);
            assert_eq!(got, want, "box {bx:?} corner {cwid} {chgt} cw {cw}");
        }
    }
}

/// GetPath's lines for a path holding one figure: its points rounded to the nearest pixel
/// (halves up), the first PT_MOVETO (6), then `kind(i)`, the last closing the figure
fn get_path(c: &[(i64, i64)], kind: impl Fn(usize) -> u8) -> Vec<String> {
    let mut v = vec![format!("path {}", c.len())];
    for (i, q) in c.iter().enumerate() {
        let ty = if i == 0 { 6 } else { kind(i) | u8::from(i + 1 == c.len()) };
        v.push(format!("pt {} {} {ty}", (q.0 + 8).div_euclid(16), (q.1 + 8).div_euclid(16)));
    }
    v
}

/// the path lines a case prints for one shape recorded between BeginPath and EndPath
fn recorded(dir: u8, call: &str) -> Vec<String> {
    let text = format!("case curve\ndib 0 4 4 8\narcdir 0 {dir}\nbeginpath 0\n{call}\nendpath 0\ngetpath 0\nend\n");
    let c = kikizake::case::parse_cases(&text);
    let out = kikizake::model::run_case(&c[0]).unwrap_or_else(|e| panic!("{call}: {e:?}"));
    out.into_iter().filter(|l| l.starts_with("path ") || l.starts_with("pt ")).collect()
}

/// the whole output of a case recording one call in a path
fn all_lines(dir: u8, call: &str) -> Vec<String> {
    let text = format!("case curve\ndib 0 4 4 8\narcdir 0 {dir}\nbeginpath 0\n{call}\nendpath 0\ngetpath 0\nend\n");
    let c = kikizake::case::parse_cases(&text);
    kikizake::model::run_case(&c[0]).unwrap_or_else(|e| panic!("{call}: {e:?}"))
}

/// GDI.PATH.ROUNDFLAT: RoundRect with a corner 0 wide or high records Rectangle's figure, and
/// over an empty box with both corner sizes positive records nothing and succeeds; every box
/// with corners in -3..5 in either order, both arc directions
#[test]
fn round_rect_flat_is_rectangle_and_empty_is_nothing() {
    let k: Vec<i64> = (-3..5).collect();
    for &x0 in &k {
        for &x1 in &k {
            for &y0 in &k {
                for &y1 in &k {
                    for dir in [1u8, 2] {
                        let boxed = format!("{x0} {y0} {x1} {y1}");
                        if x0 != x1 && y0 != y1 {
                            let rect = all_lines(dir, &format!("rectangle 0 {boxed}"));
                            for (w, h) in [(0, 0), (0, 3), (5, 0), (0, 40)] {
                                assert_eq!(all_lines(dir, &format!("roundrect 0 {boxed} {w} {h}")), rect, "{boxed} corner {w} {h} dir {dir}");
                            }
                        } else {
                            for (w, h) in [(1, 1), (3, 7), (40, 2)] {
                                let got = all_lines(dir, &format!("roundrect 0 {boxed} {w} {h}"));
                                assert!(got.contains(&"ret 1".to_owned()), "{boxed}: {got:?}");
                                assert_eq!(got.last().map(String::as_str), Some("path 0"), "{boxed} corner {w} {h}: {got:?}");
                            }
                        }
                    }
                }
            }
        }
    }
}

/// GDI.PATH.ELLIPSE, GDI.PATH.ROUNDRECT: Ellipse and RoundRect recorded in a path, for every
/// box with corners in -3..6 given in either order and not empty, a range of corner sizes for
/// RoundRect, both arc directions, read back by GetPath as the clauses' points rounded halves up
#[test]
fn ellipse_and_round_rect_read_back_rounded() {
    let k: Vec<i64> = (-3..6).collect();
    for &x0 in &k {
        for &x1 in &k {
            for &y0 in &k {
                for &y1 in &k {
                    if x0 == x1 || y0 == y1 {
                        continue;
                    }
                    let bx = (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1));
                    for dir in [1u8, 2] {
                        let e = ellipse(bx.0, bx.1, bx.2, bx.3, dir == 2).expect("a box of these sides has its figure");
                        assert_eq!(recorded(dir, &format!("ellipse 0 {x0} {y0} {x1} {y1}")), get_path(&e, |_| 4), "ellipse {x0} {y0} {x1} {y1} dir {dir}");
                        for (w, h) in [(1, 1), (2, 5), (4, 4), (9, 3), (3, 9), (9, 9)] {
                            let q = round_rect(bx, (w, h), dir == 2);
                            let call = format!("roundrect 0 {x0} {y0} {x1} {y1} {w} {h}");
                            assert_eq!(recorded(dir, &call), get_path(&q, |i| if i % 4 == 0 { 2 } else { 4 }), "{call} dir {dir}");
                        }
                    }
                }
            }
        }
    }
}
