//! DPtoLP through the page transform and LPtoDP / DPtoLP through a world transform
//! (spec/gdixform.txt GDI.MAP.INVERSE, GDI.MAP.DPROUND, GDI.XFORM.WORLDPOINT,
//! GDI.XFORM.WORLDINV, GDI.XFORM.ROUNDWORLD) against the clauses: binary32 steps in f32 as
//! the clauses order them, and LPtoDP's exact sum taken in integers from the f32 values' bits
//! (so it does not lean on f64 holding the sum exactly).
use kikizake::model::gdixform::{page, Map, IDENTITY};

fn map(wext: (i32, i32), vext: (i32, i32), worg: (i32, i32), vorg: (i32, i32), world: [f32; 6]) -> Map {
    // (MM_ANISOTROPIC: the extents as stored, no isotropic shrinking)
    Map { mode: 8, gmode: if world == IDENTITY { 1 } else { 2 }, wext, vext, worg, vorg, world, ..Map::default() }
}

/// f32 to i32 as the clauses round: halves away from zero
fn away(v: f32) -> i64 {
    // (f32::round rounds halves away from zero; whole f32 values past 2^24 are their own)
    let r = v.round();
    assert!(r.abs() < 1.0e18, "{v}");
    r as i64
}

/// GDI.MAP.DPROUND, one axis: M11 = vext / wext in f32, the inverse M22 / (M11 M22) (y: M11 /
/// (M11 M22)), the point times it and wO - vO times it each rounded halves away, added
fn dp_axis(d: i32, mine: f32, other: f32, wo: i32, vo: i32) -> i64 {
    let det = mine * other;
    let inv = other / det;
    away(d as f32 * inv) + away(wo as f32 - vo as f32 * inv)
}

/// the extents and origins the page laws run over: every pair of extents in -6..6 without 0
/// and larger ones that make f32 quotients inexact, origins in -3..3
fn exts() -> Vec<i32> {
    let mut v: Vec<i32> = (-6..=6).filter(|e| *e != 0).collect();
    v.extend([7, -9, 10, 13, 100, -333, 1000, 4096, 65535, -100_003, 1 << 20]);
    v
}

/// GDI.MAP.DPROUND: DPtoLP is the f32 inverse of the f32 matrix, both axes
#[test]
fn dp_to_lp_is_the_f32_inverse() {
    let (e, o) = (exts(), [-3, -1, 0, 2, 3, 1000]);
    let pts = [-7, -2, -1, 0, 1, 3, 8, 77, 1023, -40000, 1 << 22];
    for &wx in &e {
        for &vx in &e {
            for (k, &wo) in o.iter().enumerate() {
                let vo = o[(k + 2) % o.len()];
                // (the y axis takes the extents the other way round)
                let m = map((wx, vx), (vx, wx), (wo, vo), (vo, wo), IDENTITY);
                let (sx, sy) = (vx as f32 / wx as f32, wx as f32 / vx as f32);
                for &p in &pts {
                    let got = m.dp_to_lp((p, -p)).unwrap();
                    let want = (dp_axis(p, sx, sy, wo, vo), dp_axis(-p, sy, sx, vo, wo));
                    if i32::try_from(want.0).is_err() || i32::try_from(want.1).is_err() {
                        // (past 32 bits: an answer the clause does not give)
                        continue;
                    }
                    assert_eq!((i64::from(got.0), i64::from(got.1)), want, "ext {wx} {vx} org {wo} {vo} point {p}");
                }
            }
        }
    }
}

/// GDI.MAP.INVERSE: within the small extents, origins and points DPtoLP is the documented
/// (Dx - xVO) * xWE / xVE + xWO to within one logical unit (the point's and the offset's roundings each half a unit)
#[test]
fn dp_to_lp_is_near_the_documented_inverse() {
    let e: Vec<i32> = (-6..=6).filter(|e| *e != 0).collect();
    for &wx in &e {
        for &vx in &e {
            for wo in -3..=3 {
                for vo in -3..=3 {
                    let m = map((wx, 1), (vx, 1), (wo, 0), (vo, 0), IDENTITY);
                    for d in -12..=12 {
                        let got = i64::from(m.dp_to_lp((d, 0)).unwrap().0);
                        // (exact: n / vx with n = (d - vo) wx + wo vx)
                        let (n, q) = (i64::from(d - vo) * i64::from(wx) + i64::from(wo) * i64::from(vx), i64::from(vx));
                        let (n, q) = if q < 0 { (-n, -q) } else { (n, q) };
                        let dist = (got * q - n).abs();
                        assert!(dist <= q, "ext {wx} {vx} org {wo} {vo} point {d}: {got} vs {n}/{q}");
                    }
                }
            }
        }
    }
}

/// an f32's value as m 2^e, m an integer
fn parts(v: f32) -> (i128, i32) {
    let b = v.to_bits();
    let (s, ex, fr) = (b >> 31, i32::try_from((b >> 23) & 0xff).unwrap(), i128::from(b & 0x7f_ffff));
    let (m, e) = if ex == 0 { (fr, -149) } else { (fr + (1 << 23), ex - 150) };
    (if s == 1 { -m } else { m }, e)
}

/// n / 2^k rounded halves away from zero, k >= 0
fn shift_away(n: i128, k: u32) -> i128 {
    if k == 0 {
        return n;
    }
    let h = 1i128 << (k - 1);
    if n >= 0 { (n + h) >> k } else { -((-n + h) >> k) }
}

/// 16 (x a + y b) exactly, rounded halves away
fn exact16(x: i32, a: f32, y: i32, b: f32) -> i64 {
    let ((ma, ea), (mb, eb)) = (parts(a), parts(b));
    // (a zero takes the other's exponent)
    let (ea, eb) = (if ma == 0 { eb } else { ea }, if mb == 0 { ea } else { eb });
    let lo = ea.min(eb);
    let (sa, sb) = (u32::try_from(ea - lo).unwrap(), u32::try_from(eb - lo).unwrap());
    assert!(sa < 64 && sb < 64, "exponents {ea} {eb}");
    // (16 (x ma 2^ea + y mb 2^eb) = (x ma 2^sa + y mb 2^sb) 2^(lo + 4))
    let n = i128::from(x) * (ma << sa) + i128::from(y) * (mb << sb);
    let k = lo + 4;
    let v = if k >= 0 { n << u32::try_from(k).unwrap() } else { shift_away(n, u32::try_from(-k).unwrap()) };
    i64::try_from(v).unwrap()
}

/// GDI.XFORM.WORLDPOINT, GDI.XFORM.ROUNDWORLD: the device matrix of the clauses
fn device(world: [f32; 6], wext: (i32, i32), vext: (i32, i32), worg: (i32, i32), vorg: (i32, i32)) -> [f32; 6] {
    let (sx, sy) = (vext.0 as f32 / wext.0 as f32, vext.1 as f32 / wext.1 as f32);
    // (eDx * vext / wext + (vO - wO * vext / wext), each step in f32)
    let ox = world[4] * sx + (vorg.0 as f32 - worg.0 as f32 * sx);
    let oy = world[5] * sy + (vorg.1 as f32 - worg.1 as f32 * sy);
    [world[0] * sx, world[1] * sy, world[2] * sx, world[3] * sy, ox, oy]
}

fn worlds() -> Vec<[f32; 6]> {
    vec![
        [0.0, 1.0, -1.0, 0.0, 3.0, 0.0],
        [1.0, 1.0, -1.0, 1.0, 2.0, 0.0],
        [2.0, 0.0, 0.0, 0.5, -1.5, 2.25],
        [0.1, 0.7, -0.3, 1.9, 0.5, -0.5],
        [1.5, -0.25, 0.75, 1.25, 10.0, -7.5],
        [-1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        [0.333, 0.0, 0.0, 3.0, 1.0 / 3.0, 2.0 / 3.0],
        [1.0, 0.001, 0.0, 1.0, 0.0, 0.0],
        [3.7, -2.2, 1.1, 0.9, -12.3, 4.4],
        [1.1, 0.0, 0.0, 0.9, 12345.678, -9876.54],
    ]
}

/// the extents and origins the world laws run over
fn settings() -> Vec<((i32, i32), (i32, i32), (i32, i32), (i32, i32))> {
    let exts = [((1, 1), (1, 1)), ((2, 1), (1, 1)), ((3, -7), (5, 2)), ((10, 10), (3, 7)), ((-4, 9), (13, -1)), ((1000, 3), (17, 999))];
    let orgs = [((0, 0), (0, 0)), ((1, -2), (0, 0)), ((0, 0), (3, 5)), ((-7, 4), (2, -9)), ((100_003, -77_777), (-54_321, 31_415)), ((3_000_017, -1_234_567), (-2_999_999, 2_222_221))];
    exts.iter().flat_map(|e| orgs.iter().map(move |o| (e.0, e.1, o.0, o.1))).collect()
}

/// GDI.XFORM.WORLDPOINT, GDI.XFORM.ROUNDWORLD: LPtoDP through a world transform is the exact
/// sum of the point times the f32 matrix, to 28.4 halves away, plus the 28.4 offset, then
/// (v + 8) >> 4
#[test]
fn world_lp_to_dp_is_the_exact_f32_matrix() {
    let pts = [-1000, -37, -5, -1, 0, 1, 2, 7, 64, 999, 123_457];
    // (a world equal to the identity takes the page rule, GDI.MAP.ROUND)
    for (we, ve, wo, vo) in settings() {
        let m = map(we, ve, wo, vo, IDENTITY);
        for &x in &pts {
            let want = (page(x, we.0, ve.0, wo.0, vo.0), page(-x, we.1, ve.1, wo.1, vo.1));
            assert_eq!(m.lp_to_dp((x, -x)).unwrap(), want, "identity {we:?} {ve:?} {wo:?} {vo:?} point {x}");
        }
    }
    for w in worlds() {
        for (we, ve, wo, vo) in settings() {
            let m = map(we, ve, wo, vo, w);
            let d = device(w, we, ve, wo, vo);
            for &x in &pts {
                for &y in &pts {
                    let fx = exact16(x, d[0], y, d[2]) + away(d[4] * 16.0);
                    let fy = exact16(x, d[1], y, d[3]) + away(d[5] * 16.0);
                    let want = ((fx + 8).div_euclid(16), (fy + 8).div_euclid(16));
                    let got = m.lp_to_dp((x, y)).unwrap();
                    assert_eq!((i64::from(got.0), i64::from(got.1)), want, "world {w:?} {we:?} {ve:?} {wo:?} {vo:?} point {x} {y}");
                }
            }
        }
    }
}

/// GDI.XFORM.WORLDINV, GDI.XFORM.ROUNDWORLD: DPtoLP through a world transform inverts the f32
/// matrix through its f32 determinant, the offset -(dx i11 + dy i21), each axis the f32 sum of
/// the point's two products rounded halves away plus the offset rounded halves away
#[test]
fn world_dp_to_lp_is_the_f32_inverse() {
    let pts = [-1000, -37, -5, -1, 0, 1, 2, 7, 64, 999, 123_457];
    for w in worlds() {
        for (we, ve, wo, vo) in settings() {
            let m = map(we, ve, wo, vo, w);
            let [m11, m12, m21, m22, dx, dy] = device(w, we, ve, wo, vo);
            let det = m11 * m22 - m12 * m21;
            let (i11, i12, i21, i22) = (m22 / det, -m12 / det, -m21 / det, m11 / det);
            let (ox, oy) = (-(dx * i11 + dy * i21), -(dx * i12 + dy * i22));
            for &x in &pts {
                for &y in &pts {
                    let (fx, fy) = (x as f32, y as f32);
                    let want = (away(fx * i11 + fy * i21) + away(ox), away(fx * i12 + fy * i22) + away(oy));
                    let got = m.dp_to_lp((x, y)).unwrap();
                    assert_eq!((i64::from(got.0), i64::from(got.1)), want, "world {w:?} {we:?} {ve:?} {wo:?} {vo:?} point {x} {y}");
                }
            }
        }
    }
}

/// GDI.MAP.ISOTROPIC's rule on stored extents: one viewport extent scaled down by the ratio
/// of the smaller to the larger of |vx / wx| and |vy / wy|, floor(v |r| + 0.5)
fn iso_shrink(w: (i32, i32), v: (i32, i32)) -> (i32, i32) {
    let (xd, yd) = (f64::from(v.0) / f64::from(w.0), f64::from(v.1) / f64::from(w.1));
    let near = |r: f64| (r + 0.5).floor() as i32;
    if xd.abs() > yd.abs() { (near(f64::from(v.0) * (yd / xd).abs()), v.1) } else { (v.0, near(f64::from(v.1) * (xd / yd).abs())) }
}

/// GDI.MAP.ISOPOINTS: MM_ISOTROPIC with its extents stored (read once since they changed):
/// LPtoDP is the page rule and DPtoLP GDI.MAP.DPROUND's through the stored extents shrunk once
/// more when the rule shrinks y, the stored ones when it would shrink x, DPtoLP's offset
/// wO - vO * inverse in f64; every window extent in -4..4 and viewport extent in -6..6 (0
/// out), a few origins and points
#[test]
fn isotropic_points_take_the_extents_shrunk_once_more() {
    let (we, ve): (Vec<i32>, Vec<i32>) = ((-4..=4).filter(|e| *e != 0).collect(), (-6..=6).filter(|e| *e != 0).collect());
    let orgs = [((0, 0), (0, 0)), ((8, -3), (2, 1)), ((-5, 7), (-1, 4))];
    let pts = [(-9, 4), (0, 0), (1, -1), (5, 11), (-13, -7)];
    for &wx in &we {
        for &wy in &we {
            for &vx in &ve {
                for &vy in &ve {
                    let (w, v) = ((wx, wy), (vx, vy));
                    let s = iso_shrink(w, v);
                    let e = if s.0 == v.0 { s } else { v };
                    for &(wo, vo) in &orgs {
                        let m = Map { mode: 7, wext: w, vext: v, worg: wo, vorg: vo, unsettled: false, ..Map::default() };
                        for &(x, y) in &pts {
                            let want = (page(x, w.0, e.0, wo.0, vo.0), page(y, w.1, e.1, wo.1, vo.1));
                            assert_eq!(m.lp_to_dp((x, y)).unwrap(), want, "lp {w:?} {v:?} {wo:?} {vo:?} ({x}, {y})");
                            if e.0 == 0 || e.1 == 0 {
                                continue;
                            }
                            let (m11, m22) = (e.0 as f32 / w.0 as f32, e.1 as f32 / w.1 as f32);
                            let det = m11 * m22;
                            let (i11, i22) = (m22 / det, m11 / det);
                            let axis = |d: i32, inv: f32, o: i32, p: i32| -> i64 { away(d as f32 * inv) + (f64::from(o) - f64::from(p) * f64::from(inv)).round() as i64 };
                            let want = (axis(x, i11, wo.0, vo.0), axis(y, i22, wo.1, vo.1));
                            let got = m.dp_to_lp((x, y)).unwrap();
                            assert_eq!((i64::from(got.0), i64::from(got.1)), want, "dp {w:?} {v:?} {wo:?} {vo:?} ({x}, {y})");
                        }
                    }
                }
            }
        }
    }
}
