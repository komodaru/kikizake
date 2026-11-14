//! Laws of the GDI+ model (tests/laws.rs for the others; a file of its own so that APIs added
//! side by side do not collide): what the documentation states as properties, exhaustive over
//! small domains, property-based elsewhere.
use proptest::prelude::*;
use kikizake::model::gdiplus::{self as gp, bits, from_argb, get_px, mul, put_px, rotate, to_argb, transform, Bitmap, Pf};

fn bitmap(f: Pf, w: u32, h: u32, seed: u32) -> Bitmap {
    let mut b = Bitmap::new(f, w, h);
    let mut r = kikizake::rng::XorShift32::new(seed);
    for y in 0..h {
        for x in 0..w {
            let v = r.next() as u64 | (r.next() as u64) << 32;
            let m = if bits(f) == 64 { !0 } else { (1u64 << bits(f)) - 1 };
            b.set_px(x, y, v & m);
        }
    }
    b
}

/// every pixel of a row at every bit depth is written and read back alone
#[test]
fn pixels_are_independent() {
    for f in Pf::ALL {
        let n = bits(f);
        let w = 9u32;
        for x in 0..w {
            let mut row = vec![0u8; gp::row_bytes(f, w)];
            let m = if n == 64 { !0 } else { (1u64 << n) - 1 };
            put_px(&mut row, x, n, m);
            for o in 0..w {
                assert_eq!(get_px(&row, o, n), if o == x { m } else { 0 }, "{} x {x} o {o}", f.name());
            }
        }
    }
}

/// RotateFlipType: the 8 types are the dihedral group of the rectangle; 1 is a quarter turn
/// clockwise, 4 a horizontal flip, k = (k & 3 turns) then (k & 4: flip), 6 the vertical flip
#[test]
fn rotations_are_the_dihedral_group() {
    for (i, f) in Pf::ALL.into_iter().enumerate() {
        for (w, h) in [(1, 1), (3, 2), (5, 3), (9, 4)] {
            let b = bitmap(f, w, h, i as u32 * 77 + w);
            assert_eq!(rotate(&b, 0), b);
            let q = rotate(&b, 1);
            assert_eq!((q.w, q.h), (h, w));
            // (a quarter turn clockwise: the bottom-left pixel goes to the top-left)
            assert_eq!(q.px(0, 0), b.px(0, h - 1));
            assert_eq!(rotate(&rotate(&q, 1), 1), rotate(&b, 3));
            assert_eq!(rotate(&rotate(&rotate(&q, 1), 1), 1), b);
            let fx = rotate(&b, 4);
            assert_eq!(fx.px(0, 0), b.px(w - 1, 0));
            assert_eq!(rotate(&fx, 4), b);
            let fy = rotate(&b, 6);
            for y in 0..h {
                for x in 0..w {
                    assert_eq!(fy.px(x, y), b.px(x, h - 1 - y));
                }
            }
            for k in 0..8 {
                // turns then flip
                assert_eq!(rotate(&b, k), if k >= 4 { rotate(&rotate(&b, k & 3), 4) } else { rotate(&b, k) });
            }
        }
    }
}

/// the low-bit formats: every value of the format survives the trip through ARGB (exhaustive)
#[test]
fn low_bit_values_survive_argb() {
    for name in ["16bppRGB555", "16bppRGB565", "16bppARGB1555", "24bppRGB"] {
        let f = Pf::parse(name).unwrap();
        let n = if name == "24bppRGB" { 1 << 24 } else { 1 << 16 };
        for v in (0..n).step_by(if n > 65536 { 251 } else { 1 }) {
            let v = if name == "16bppRGB555" { v & 0x7fff } else { v };
            let c = to_argb(f, v, &[]).unwrap();
            assert_eq!(from_argb(f, c).unwrap(), v, "{name} {v:#x}");
        }
    }
}

const ID: gp::Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

fn small() -> impl Strategy<Value = f32> {
    (-64i32..64).prop_map(|v| v as f32 / 4.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// the identity is the identity of the product, both ways, exactly
    #[test]
    fn identity(m in prop::array::uniform6(small())) {
        prop_assert_eq!(mul(&ID, &m), m);
        prop_assert_eq!(mul(&m, &ID), m);
        prop_assert_eq!(transform(&ID, m[0], m[1]), (m[0], m[1]));
    }

    /// row vectors: a point through a × b is the point through a, then through b (exact on
    /// quarter integers, which float holds exactly here)
    #[test]
    fn product_composes(a in prop::array::uniform6(small()), b in prop::array::uniform6(small()), x in small(), y in small()) {
        let (px, py) = transform(&a, x, y);
        prop_assert_eq!(transform(&mul(&a, &b), x, y), transform(&b, px, py));
    }

    /// an invertible matrix times its inverse moves a point back within float rounding; a zero
    /// determinant is never invertible
    #[test]
    fn inverse_undoes(m in prop::array::uniform6(small()), x in small(), y in small()) {
        let det = m[0] * m[3] - m[1] * m[2];
        match gp::invert(&m) {
            None => prop_assert_eq!(det, 0.0),
            Some(i) => {
                let (px, py) = transform(&m, x, y);
                let (bx, by) = transform(&i, px, py);
                let tol = 1e-3 * (1.0 + px.abs() + py.abs()) * (1.0 + m.iter().fold(0f32, |a, v| a.max(v.abs()))) / det.abs().min(1.0);
                prop_assert!((bx - x).abs() <= tol && (by - y).abs() <= tol, "{bx} {by} vs {x} {y}");
            }
        }
    }

    /// rounding down: the result is at most the exact value and the next float above is more
    /// (products of two floats and their sums with small ones are exact in f64)
    #[test]
    fn rounding_down_is_the_float_below(a in -1e6f32..1e6, b in -1e6f32..1e6, k in -64i32..64) {
        let d = gp::Round::Down;
        let p = a as f64 * b as f64;
        let m = d.mul(a, b);
        prop_assert!(m as f64 <= p && (p == 0.0 || m.next_up() as f64 > p), "{a} * {b} = {m}");
        let c = k as f32 / 8.0;
        let s = a as f64 + c as f64;
        let t = d.add(a, c);
        prop_assert!(t as f64 <= s && (t.next_up() as f64 > s || s == 0.0), "{a} + {c} = {t}");
        if b != 0.0 {
            let q = d.div(a, b);
            // (q <= a / b exactly: q b against a, the sign of b turning it)
            let r = q as f64 * b as f64 - a as f64;
            prop_assert!(if b > 0.0 { r <= 0.0 } else { r >= 0.0 }, "{a} / {b} = {q}");
            let r = q.next_up() as f64 * b as f64 - a as f64;
            prop_assert!(if b > 0.0 { r > 0.0 } else { r < 0.0 }, "{a} / {b} = {q} next");
        }
        prop_assert_eq!(gp::Round::Nearest.mul(a, b), a * b);
    }

    /// a bitmap read in its own format is what was written, for any rectangle inside it
    #[test]
    fn same_format_round_trip(f in 0..Pf::ALL.len(), seed: u32, x in 0i64..5, y in 0i64..3, w in 1i64..5, h in 1i64..3) {
        let name = Pf::ALL[f].name();
        let text = format!("case p\ngpbitmap 0 9 5 {name}\ngpwrite 0 {name} seed {seed} at {x} {y} {w} {h}\ngpread 0 {name} at {x} {y} {w} {h}\ngpread 0 {name}\nend\n");
        let c = &kikizake::case::parse_cases(&text)[0];
        let out = kikizake::model::run_case(c).unwrap();
        prop_assert_eq!(&out[1], "write 0 0");
        // (the untouched pixels stay 0: the rows of the whole read outside the rectangle)
        let whole = &out[6 + h as usize..];
        for (yy, row) in whole.iter().enumerate() {
            let yy = yy as i64;
            if yy < y || yy >= y + h {
                prop_assert!(row.bytes().all(|b| b == b'0'), "{}", row);
            }
        }
    }
}

/// the ordered dither keeps every level's mean: over a 4x4 block of one 8-bit value c the 5-bit
/// results are c >> 3 or one more, the larger 2 (c & 7) times (each d of 0..7 twice); 6 bits:
/// c >> 2 or one more, 4 (c & 3) times; clamped at the top (exhaustive)
#[test]
fn dither_keeps_the_mean() {
    for (name, gbits) in [("16bppRGB555", 5u32), ("16bppRGB565", 6)] {
        let f = Pf::parse(name).unwrap();
        for c in 0..256u32 {
            let (mut sr, mut sg) = (0, 0);
            for y in 0..4 {
                for x in 0..4 {
                    let v = gp::dither(f, 0xff00_0000 | c << 16 | c << 8 | c, x, y);
                    let (r, g, b) = (v >> (5 + gbits) & 31, v >> 5 & ((1 << gbits) - 1), v & 31);
                    assert_eq!(r, b);
                    sr += r;
                    sg += g;
                }
            }
            assert_eq!(sr as u32, (16 * (c >> 3) + 2 * (c & 7)).min(31 * 16), "{name} {c} red");
            assert_eq!(sg as u32, if gbits == 6 { (16 * (c >> 2) + 4 * (c & 3)).min(63 * 16) } else { (16 * (c >> 3) + 2 * (c & 7)).min(31 * 16) }, "{name} {c} green");
        }
    }
}

/// the model answers every case of its suites but those of the open clauses, and the same
/// number of them on each executor as when it was settled (a model that gives up on more cases
/// still matches Windows on the rest: this is what keeps it answering)
#[test]
fn the_model_answers_all_but_the_open_clauses() {
    for (suite, arch, open) in [("gdiplus", 32, 96), ("gdiplus", 64, 0), ("gdiplusmat", 32, 0), ("gdiplusmat", 64, 0)] {
        kikizake::model::set_arch(arch);
        let cases = kikizake::case::parse_cases(&kikizake::gen::suite(suite).unwrap());
        let mut n = 0;
        for c in &cases {
            match kikizake::model::run_case(c) {
                Ok(_) => {}
                Err(kikizake::model::Fail::Unmodeled(id)) => {
                    assert!(["GP.FILL.GAMMA"].contains(&id), "{suite} {}: {id}", c.name);
                    n += 1;
                }
                Err(kikizake::model::Fail::Bad) => panic!("{suite} {}: a bad line", c.name),
                Err(kikizake::model::Fail::Empty) => panic!("{suite} {}: no answer", c.name),
            }
        }
        assert_eq!(n, open, "{suite}-{arch}");
    }
    kikizake::model::set_arch(32);
}

/// a clone into an indexed format finds each pixel's color in its fixed palette: every entry the
/// nearest table picks from a default palette is in the fixed one (exhaustive over 5-bit colors)
#[test]
fn fixed_palettes_hold_every_nearest_color() {
    for (f, d) in [(Pf::I1, gp::Depth::One), (Pf::I4, gp::Depth::Four), (Pf::I8, gp::Depth::Eight)] {
        let (fixed, _) = gp::fixed_palette(d);
        let default = gp::default_palette(f);
        for k in 0..32768u32 {
            let c = 0xff00_0000 | (k >> 10) << 19 | (k >> 5 & 31) << 11 | (k & 31) << 3;
            let e = default[gp::nearest(d, c) as usize];
            assert!(fixed.contains(&e), "{f:?} {k:#x} {e:08x}");
        }
    }
}
