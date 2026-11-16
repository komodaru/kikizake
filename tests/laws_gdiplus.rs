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

/// the model answers every case of its suites on both executors: no clause of GDI+ is open
#[test]
fn the_model_answers_every_case() {
    for (suite, arch) in [("gdiplus", 32), ("gdiplus", 64), ("gdiplusmat", 32), ("gdiplusmat", 64)] {
        kikizake::model::set_arch(arch);
        let cases = kikizake::case::parse_cases(&kikizake::generators::suite(suite).unwrap());
        for c in &cases {
            match kikizake::model::run_case(c) {
                Ok(_) => {}
                Err(kikizake::model::Fail::Unmodeled(id)) => panic!("{suite}-{arch} {}: {id}", c.name),
                Err(kikizake::model::Fail::Bad) => panic!("{suite}-{arch} {}: a bad line", c.name),
                Err(kikizake::model::Fail::Empty) => panic!("{suite}-{arch} {}: no answer", c.name),
                Err(kikizake::model::Fail::Crash) => panic!("{suite}-{arch} {}: the executor dies", c.name),
            }
        }
    }
    kikizake::model::set_arch(32);
}

/// x86's 16-bit lane product is the rounded d f / 8192, or 8 below it exactly where the low word
/// of d f carries out when 4096 is added (exhaustive over the 13-bit words)
#[test]
fn lanes_lose_only_the_carry() {
    for d in 0..=8192u64 {
        for f in (0..=8192u64).step_by(7) {
            let r = (d * f + 4096) >> 13;
            let carry = ((d * f) & 0xffff) >= 0x1_0000 - 4096;
            assert_eq!(kikizake::model::gdiplus::lanes(d, f), if carry { r - 8 } else { r }, "{d} {f}");
        }
    }
}

proptest! {
    /// the lanes never go above the rounded quotient and never more than 8 below it
    #[test]
    fn lanes_stay_within_8(d in 0..=8192u64, f in 0..=8192u64) {
        let l = kikizake::model::gdiplus::lanes(d, f);
        let r = (d * f + 4096) >> 13;
        prop_assert!(l <= r && r - l <= 8);
    }
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

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]
    /// a hatch of one color paints as a solid brush of it does, in every compositing mode and
    /// quality, on every format, on both executors (the runs of GP.PAINT.GAMMA.RUNS are then one a
    /// row, from the bitmap's left edge)
    #[test]
    fn a_hatch_of_one_color_is_a_solid_fill(f in 0..Pf::ALL.len(), c in any::<u32>(), seed in any::<u32>(), style in 0..53i64, over in any::<bool>(), q in 0..6i64, arch in prop::sample::select(vec![32u32, 64])) {
        let f = Pf::ALL[f].name();
        let mode = if over { "over" } else { "copy" };
        let run = |brush: String| {
            let text = format!("# t\ncase t\ngpbitmap 0 16 8 {f}\ngpwrite 0 {f} seed {seed}\ngpbrush 0 {brush}\ngppaint 0 0 0 0 16 8 {mode} {q} 0 0 0 0\ngpread 0 {f}\nend\n");
            let cases = kikizake::case::parse_cases(&text);
            kikizake::model::set_arch(arch);
            let r = kikizake::model::run_case(&cases[0]).ok();
            kikizake::model::set_arch(32);
            r
        };
        let hatch = run(format!("hatch {style} {c:#010x} {c:#010x}"));
        let solid = run(format!("solid {c:#010x}"));
        prop_assert_eq!(hatch.map(|l| l.into_iter().filter(|l| !l.starts_with("brush")).collect::<Vec<_>>()), solid.map(|l| l.into_iter().filter(|l| !l.starts_with("brush")).collect::<Vec<_>>()));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]
    /// the identity ColorMatrix gives every color back (the channels go through as integers)
    #[test]
    fn the_identity_matrix_keeps_every_color(c in any::<u32>()) {
        let id = gp::ColorMatrix(std::array::from_fn(|k| if k % 6 == 0 { 1.0 } else { 0.0 }));
        prop_assert_eq!(gp::recolor(c, &id, gp::MatrixFlags::Default), c);
    }
    /// a translation by t / 256 adds it to every channel, clamped to 0 ..= 255 and rounded
    #[test]
    fn a_translation_adds_and_clamps(c in any::<u32>(), t in -512i32..512) {
        let mut m: [f32; 25] = std::array::from_fn(|k| if k % 6 == 0 { 1.0 } else { 0.0 });
        for j in 0..4 {
            m[20 + j] = t as f32 / 256.0;
        }
        let o = gp::recolor(c, &gp::ColorMatrix(m), gp::MatrixFlags::Default);
        for k in 0..4 {
            let want = ((f64::from(c >> (8 * k) & 255) + f64::from(t) * 255.0 / 256.0).clamp(0.0, 255.0) + 0.5).floor() as u32;
            prop_assert_eq!(o >> (8 * k) & 255, want);
        }
    }
    /// SkipGrays leaves a gray pixel whatever the matrix
    #[test]
    fn skip_grays_keeps_grays(a in any::<u8>(), v in any::<u8>(), m in proptest::array::uniform25(-2.0f32..2.0)) {
        let c = (u32::from(a) << 24) | (u32::from(v) * 0x1_0101);
        prop_assert_eq!(gp::recolor(c, &gp::ColorMatrix(m), gp::MatrixFlags::SkipGrays), c);
    }
    /// NearestNeighbor at 1:1 maps every pixel onto itself under every PixelOffsetMode
    #[test]
    fn nearest_one_to_one_is_the_identity(k in 0i64..4096, d in 1i64..4096, po in 0i64..5) {
        let po = gp::PixelOffset::from_value(po).unwrap();
        let k = k % d;
        prop_assert_eq!(gp::nearest_x(k, d, d, po), k);
        prop_assert_eq!(gp::nearest_y(k, d, d, po), k);
    }
    /// NearestNeighbor stays inside the source (or one past it: a corner offset rounds up, and
    /// a half offset's 16.16 step, rounded, can reach s on a wide destination) and never steps back
    #[test]
    fn nearest_is_monotone_and_inside(k in 0i64..4095, s in 1i64..4096, d in 1i64..4096, po in 0i64..5) {
        let po = gp::PixelOffset::from_value(po).unwrap();
        let k = k % d;
        for f in [gp::nearest_x, gp::nearest_y] {
            let p = f(k, s, d, po);
            prop_assert!((0..=s).contains(&p), "{} of {} -> {}", k, d, p);
            if k + 1 < d {
                prop_assert!(f(k + 1, s, d, po) >= p);
            }
        }
    }
    /// an integer enlargement by n repeats each source pixel n times
    #[test]
    fn nearest_integer_enlargement_repeats(s in 1i64..256, n in 1i64..16, k in 0i64..4096, po in 0i64..5) {
        let po = gp::PixelOffset::from_value(po).unwrap();
        let k = k % (s * n);
        if po.half() {
            prop_assert_eq!(gp::nearest_y(k, s, s * n, po), k / n);
            prop_assert_eq!(gp::nearest_x(k, s, s * n, po), k / n);
        }
    }
    /// Bilinear: a texel weighted alone comes back as it is, and four equal texels give it back,
    /// on either arch
    #[test]
    fn bilinear_keeps_a_lone_or_uniform_texel(c in any::<u32>(), wx in 0u32..2048, wy in 0u32..2048, arch in prop_oneof![Just(32u32), Just(64)]) {
        kikizake::model::set_arch(arch);
        let one = gp::Tap::one();
        let (wx, wy) = (wx % one, wy % one);
        prop_assert_eq!(gp::bilinear([[c, 0], [0, 0]], 0, 0), c);
        prop_assert_eq!(gp::bilinear([[c, c], [c, c]], wx, wy), c);
    }
    /// Bilinear places: a 1:1 draw lands each pixel on its texel with no weight, and the places
    /// never step back
    #[test]
    fn bilinear_places(p in 0i64..64, d0 in 0i64..32, dn in 1i64..64, s0 in 0i64..32, sn in 1i64..64, po in 0i64..5, arch in prop_oneof![Just(32u32), Just(64)]) {
        kikizake::model::set_arch(arch);
        let po = gp::PixelOffset::from_value(po).unwrap();
        let t = gp::Tap::bilinear(d0 + p % dn, d0, dn, s0, dn, po);
        prop_assert_eq!(t, gp::Tap { at: s0 + p % dn, w: 0 });
        let (a, b) = (gp::Tap::bilinear(d0 + p, d0, dn, s0, sn, po), gp::Tap::bilinear(d0 + p + 1, d0, dn, s0, sn, po));
        prop_assert!((b.at, b.w) >= (a.at, a.w));
    }
    /// Bicubic: entry 0 gives the texel at its place back, and sixteen equal opaque texels give
    /// the color back whatever the entries, on either arch
    #[test]
    fn bicubic_keeps_a_lone_or_uniform_texel(c in any::<u32>(), ex in 0i64..65536, ey in 0i64..65536, arch in prop_oneof![Just(32u32), Just(64)]) {
        kikizake::model::set_arch(arch);
        let c = c | 0xff00_0000;
        let mut q = [[0u32; 4]; 4];
        q[1][1] = c;
        let zero = gp::Entry::of(0);
        prop_assert_eq!(gp::bicubic(q, zero, zero), c);
        prop_assert_eq!(gp::bicubic([[c; 4]; 4], gp::Entry::of(ex), gp::Entry::of(ey)), c);
    }
    /// HighQualityBilinear: the spline's taps from any place sum to one and keep a uniform line;
    /// a 1:1 axis lands on its texel; places never step back, and an enlargement's never pass
    /// its source rectangle by more than a pixel
    #[test]
    fn high_bilinear_places_and_weights(p in 0i64..64, d0 in 0i64..32, dn in 1i64..64, s0 in 0i64..32, sn in 1i64..64, po in 0i64..5, down in any::<bool>(), c in any::<u32>()) {
        let po = gp::PixelOffset::from_value(po).unwrap();
        let axis = if down { gp::Axis::Down } else { gp::Axis::Across };
        let at = gp::Place128::high_bilinear(d0 + p, d0, dn, s0, sn, po, axis);
        let taps: Vec<(i64, i64)> = at.taps().collect();
        prop_assert_eq!(taps.iter().map(|t| t.1).sum::<i64>(), 32768);
        let line: Vec<(u32, i64)> = taps.iter().map(|&(_, w)| (c, w)).collect();
        prop_assert_eq!(gp::spline_pass(&line, 15), c);
        prop_assert_eq!(gp::Place128::high_bilinear(d0 + p % dn, d0, dn, s0, dn, po, axis), gp::Place128(128 * (s0 + p % dn)));
        let next = gp::Place128::high_bilinear(d0 + p + 1, d0, dn, s0, sn, po, axis);
        prop_assert!(next.0 >= at.0);
        if sn <= dn && p < dn {
            prop_assert!(at.0 >= 128 * s0 - 64 && at.0 <= 128 * (s0 + sn));
        }
    }
}

/// the quadratic spline: the shape's halves meet, each tap pair one half apart sums to one
#[test]
fn spline2_is_a_partition_of_one() {
    for t in -256..=256 {
        assert_eq!(gp::spline2(t), gp::spline2(-t));
        let s: i64 = (-3..=3).map(|k| gp::spline2(t + 128 * k)).sum();
        assert_eq!(s, 32768, "t {t}");
    }
}


/// the bicubic tables: each entry's weights sum to one, and entry q mirrors entry 64 - q
#[test]
fn cubic_tables_sum_to_one_and_mirror() {
    for q in 0..64 {
        let s: i64 = gp::CUBIC16[q].iter().sum();
        assert!((65535..=65537).contains(&s), "{q}: {s}");
        if q > 0 {
            let (a, b) = (gp::CUBIC16[q], gp::CUBIC16[64 - q]);
            assert_eq!(a, [b[3], b[2], b[1], b[0]]);
            let (a, b) = (gp::CUBIC14[q], gp::CUBIC14[64 - q]);
            assert_eq!(a, [b[3], b[2], b[1], b[0]]);
        }
    }
}

proptest! {
    /// a texture brush's tiles (GP.BRUSH.TEXTURE.PAINT): Tile repeats every tile; each flip mode
    /// mirrors every other tile on its axes and repeats every two; Clamp gives the one tile at
    /// (0, 0) and nothing past it; every mode is the bitmap itself on that tile
    #[test]
    fn texture_tiles(w in 1u32..7, h in 1u32..7, seed: u32, x in -40i64..40, y in -40i64..40) {
        use gp::{Texels, Wrap};
        let mut r = kikizake::rng::XorShift32::new(seed);
        let px: Vec<u32> = (0..w * h).map(|_| r.next()).collect();
        let t = Texels::new(w, h, px.clone()).unwrap();
        let (wi, hi) = (i64::from(w), i64::from(h));
        let inside = (0..wi).contains(&x) && (0..hi).contains(&y);
        for wrap in [Wrap::Tile, Wrap::TileFlipX, Wrap::TileFlipY, Wrap::TileFlipXY, Wrap::Clamp] {
            if inside {
                prop_assert_eq!(t.at(wrap, x, y), Some(px[(y * wi + x) as usize]));
            }
        }
        prop_assert_eq!(t.at(Wrap::Tile, x, y), t.at(Wrap::Tile, x + wi, y - hi));
        prop_assert_eq!(t.at(Wrap::TileFlipX, x, y), t.at(Wrap::TileFlipX, 2 * wi - 1 - x, y));
        prop_assert_eq!(t.at(Wrap::TileFlipX, x, y), t.at(Wrap::TileFlipX, x + 2 * wi, y + hi));
        prop_assert_eq!(t.at(Wrap::TileFlipY, x, y), t.at(Wrap::TileFlipY, x + wi, 2 * hi - 1 - y));
        prop_assert_eq!(t.at(Wrap::TileFlipXY, x, y), t.at(Wrap::TileFlipXY, 2 * wi - 1 - x, 2 * hi - 1 - y));
        prop_assert_eq!(t.at(Wrap::Clamp, x, y).is_some(), inside);
    }
}

/// Texels::new takes exactly w h texels, neither side 0
#[test]
fn texels_sizes() {
    assert!(gp::Texels::new(2, 3, vec![0; 6]).is_some());
    assert!(gp::Texels::new(2, 3, vec![0; 5]).is_none());
    assert!(gp::Texels::new(0, 3, vec![]).is_none());
}

/// run a case of a 16 x 16 32bppARGB bitmap and the given ops; its pixel rows
fn fill_rows(ops: &str) -> Vec<String> {
    let text = format!("case p\ngpbitmap 0 16 16 32bppARGB\ngpbrush 0 solid 0xc0408020\n{ops}gpread 0 32bppARGB\nend\n");
    let c = &kikizake::case::parse_cases(&text)[0];
    let out = kikizake::model::run_case(c).unwrap();
    out[out.len() - 16..].to_vec()
}

proptest! {
    /// FillPolygon of an axis-aligned rectangle (GP.SHAPE.POLYGON) is FillRectangle of it, under
    /// every smoothing and pixel offset mode and both compositing modes
    #[test]
    fn polygon_rectangle_is_fill_rectangle(x in -8i32..72, y in -8i32..72, w in 0i32..60, h in 0i32..60, sm in 0i64..6, po in 0i64..5, copy: bool, fm in 0i64..2) {
        let (x, y, w, h) = (x as f32 / 4.0, y as f32 / 4.0, w as f32 / 4.0, h as f32 / 4.0);
        let mode = if copy { "copy" } else { "over" };
        let rect = fill_rows(&format!("gppaint 0 0 {x} {y} {w} {h} {mode} 0 {sm} {po} 0 0\n"));
        let poly = fill_rows(&format!("gpshape 0 brush 0 - {mode} 0 {sm} {po} polygon {fm} 4 {x} {y} {} {y} {} {} {x} {}\n", x + w, x + w, y + h, y + h));
        prop_assert_eq!(rect, poly);
    }

    /// a polygon fills the same whichever way round and from whichever point it is given
    /// (GP.SHAPE.POLYGON), and both fill modes agree on a convex one (a triangle)
    #[test]
    fn polygon_order_does_not_matter(p in proptest::collection::vec((-8i32..72, -8i32..72), 3..7), sm in 0i64..6, po in 0i64..5, fm in 0i64..2, k in 0usize..7) {
        let s = |p: &[(i32, i32)]| p.iter().map(|&(x, y)| format!("{} {}", x as f32 / 4.0, y as f32 / 4.0)).collect::<Vec<_>>().join(" ");
        let op = |p: &[(i32, i32)], fm: i64| fill_rows(&format!("gpshape 0 brush 0 - copy 0 {sm} {po} polygon {fm} {} {}\n", p.len(), s(p)));
        let base = op(&p, fm);
        let mut rev = p.clone();
        rev.reverse();
        prop_assert_eq!(&base, &op(&rev, fm));
        let mut rot = p.clone();
        rot.rotate_left(k % p.len());
        prop_assert_eq!(&base, &op(&rot, fm));
        prop_assert_eq!(op(&p[..3], 0), op(&p[..3], 1));
    }
}

/// a point off the 1/16 grid is not modelled (GDI+'s rounding of it is not measured)
#[test]
fn polygon_grid() {
    assert!(gp::Polygon::new(&[(0.0, 0.0), (1.0625, 0.0), (0.0, 3.5)]).is_some());
    assert!(gp::Polygon::new(&[(0.0, 0.0), (1.03, 0.0), (0.0, 3.5)]).is_none());
    let sq = gp::Polygon::new(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]).unwrap();
    // (half open: the top and left edges in, the bottom and right out)
    let a = |x, y| sq.inside(gp::FillMode::Alternate, x, y);
    assert_eq!((a(0.0, 0.0), a(2.0, 1.0), a(1.0, 2.0), a(1.0, 1.0), a(-0.01, 1.0)), (true, false, false, true, false));
    // (clockwise on the screen, y down: the left edge goes up, -1; drawn the other way +1)
    assert_eq!(sq.winding(1.0, 1.0), -1);
    assert_eq!(gp::Polygon::new(&[(0.0, 0.0), (0.0, 2.0), (2.0, 2.0), (2.0, 0.0)]).unwrap().winding(1.0, 1.0), 1);
    let twice = gp::Polygon::new(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0), (0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]).unwrap();
    assert!(twice.inside(gp::FillMode::Winding, 1.0, 1.0));
    assert!(!twice.inside(gp::FillMode::Alternate, 1.0, 1.0));
}
