//! Laws of the OpenGL model: what the GL 1.1 specification states as properties, whatever
//! Windows' measured details turn out to be.
use kikizake::compare::line_matches;
use kikizake::model::opengl::e::*;
use kikizake::model::opengl::tri::{self, Sub, Vertex};
use kikizake::model::opengl::{blend_bytes, color_material_lit, Given, blend_undefined, logic, logic_reads_dst, Fog, FogMode};
use proptest::prelude::*;

fn fog(mode: FogMode, density: f32, start: f32, end: f32) -> Fog {
    Fog { mode, density, start, end, index: 0.0, color: [0.0; 4] }
}

proptest! {
    /// GL 1.1 2.6.1: a polygon's area flips its sign when its corners turn the other way, and
    /// does not change when they start elsewhere (what GL_CULL_FACE reads)
    #[test]
    fn signed_areas_flip_with_the_turn(w in proptest::collection::vec(proptest::array::uniform2(-64i32..64), 3..8), k in 0usize..8) {
        let w: Vec<[f64; 2]> = w.iter().map(|p| p.map(f64::from)).collect();
        let a = tri::signed_area(&w);
        let reversed: Vec<[f64; 2]> = w.iter().rev().copied().collect();
        prop_assert_eq!(tri::signed_area(&reversed), -a);
        let mut rotated = w.clone();
        rotated.rotate_left(k % w.len());
        prop_assert_eq!(tri::signed_area(&rotated), a);
    }

    /// GL 1.1 2.13.8: a lit color is clamped to 0..1 at the vertex; under GL_COLOR_MATERIAL
    /// alpha is the current color's, and more light never darkens a channel
    #[test]
    fn color_material_lights_hold_to_one(ambient in 0f32..1.0, nl in 0f32..1.0, more in 0f32..1.0, c in proptest::array::uniform4(0f32..=1.0)) {
        let a = color_material_lit(ambient, nl, c);
        let b = color_material_lit(ambient, nl + more, c);
        prop_assert_eq!(a[3], c[3]);
        for k in 0..3 {
            prop_assert!((0.0..=1.0).contains(&a[k]));
            prop_assert!(b[k] >= a[k]);
        }
    }

    /// GL 1.1 3.9: the factor is held to 0..1, is 1 at the eye for the exponential modes, 1 at
    /// the start and 0 at the end for the linear one, never grows with the distance, and depends
    /// on the distance only (|z|)
    #[test]
    fn fog_factor_follows_the_modes(density in 0f32..8.0, start in 0f32..4.0, len in 0.01f32..4.0, z0 in 0f64..8.0, dz in 0f64..8.0) {
        let end = start + len;
        for mode in [FogMode::Exp, FogMode::Exp2, FogMode::Linear] {
            let f = fog(mode, density, start, end);
            let (a, b) = (f.factor(z0), f.factor(z0 + dz));
            prop_assert!((0.0..=1.0).contains(&a) && (0.0..=1.0).contains(&b));
            prop_assert!(b <= a, "{mode:?}: {a} at {z0}, {b} at {}", z0 + dz);
            prop_assert_eq!(f.factor(-z0), a);
            if mode != FogMode::Linear {
                prop_assert_eq!(f.factor(0.0), 1.0);
            }
        }
        let f = fog(FogMode::Linear, density, start, end);
        prop_assert_eq!(f.factor(f64::from(start)), 1.0);
        prop_assert!(f.factor(f64::from(end)).abs() < 1e-6);
    }

    /// GL 1.1 3.9: a mode's name round-trips and no other name is a mode
    #[test]
    fn fog_modes_round_trip(m in 0u32..0x3000) {
        match FogMode::try_from(m) {
            Ok(mode) => prop_assert_eq!(mode.name(), m),
            Err(()) => prop_assert!(![0x800, 0x801, 0x2601].contains(&m)),
        }
    }

    /// GL.UNDEFINED: what blend_undefined calls defined does not depend on the undefined
    /// destination channels: any bytes in them leave those result channels as they are
    #[test]
    fn blend_undefined_is_sound(sf in 0usize..11, df in 0usize..11, s in any::<[u8; 4]>(), d in any::<[u8; 4]>(), junk in any::<[u8; 4]>(), und in any::<[bool; 4]>()) {
        let names = [ZERO, ONE, SRC_COLOR, ONE_MINUS_SRC_COLOR, SRC_ALPHA, ONE_MINUS_SRC_ALPHA, DST_ALPHA, ONE_MINUS_DST_ALPHA, DST_COLOR, ONE_MINUS_DST_COLOR, SRC_ALPHA_SATURATE];
        let f = (names[sf], names[df]);
        let s = s.map(u32::from);
        let d0 = d.map(u32::from);
        let d1: [u32; 4] = [0, 1, 2, 3].map(|k| if und[k] { u32::from(junk[k]) } else { d0[k] });
        let (a, b) = (blend_bytes(f, s, d0, Some(d0[3])), blend_bytes(f, s, d1, Some(d1[3])));
        let out = blend_undefined(f, und);
        for k in 0..4 {
            if !out[k] {
                prop_assert_eq!(a[k], b[k], "{:x?} channel {}", f, k);
            }
        }
        prop_assert_eq!(blend_undefined(f, [false; 4]), [false; 4]);
    }

    /// GL.UNDEFINED: the same for the logic operations
    #[test]
    fn logic_reads_dst_is_sound(op in 0x1500u32..0x1510, s in any::<u8>(), d0 in any::<u8>(), d1 in any::<u8>()) {
        if !logic_reads_dst(op) {
            prop_assert_eq!(logic(op, u32::from(s), u32::from(d0)) & 0xff, logic(op, u32::from(s), u32::from(d1)) & 0xff);
        }
    }

    /// GL.UNDEFINED: a line with some bytes given as "??" matches the line with any bytes there,
    /// and a defined byte still has to be the same
    #[test]
    fn undefined_bytes_match_any(b in proptest::collection::vec(any::<u8>(), 1..64), other in any::<u8>(), mask in any::<u64>(), at in any::<prop::sample::Index>()) {
        let got: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let want: String = b.iter().enumerate().map(|(i, x)| if mask >> (i % 64) & 1 != 0 { "??".into() } else { format!("{x:02x}") }).collect();
        let (w, g) = (format!("read {want}"), format!("read {got}"));
        prop_assert!(line_matches(Some(&w), Some(&g)));
        let i = at.index(b.len());
        if mask >> (i % 64) & 1 == 0 && other != b[i] {
            let mut c = b.clone();
            c[i] = other;
            let got: String = c.iter().map(|x| format!("{x:02x}")).collect();
            let g = format!("read {got}");
            prop_assert!(!line_matches(Some(&w), Some(&g)));
        }
    }

    /// GL 1.1 4.1.6 (GL.BLEND.DSTALPHA): with destination alpha (ONE, ZERO) gives the source and
    /// (ZERO, ONE) the destination
    #[test]
    fn blend_dstalpha_identities(s in any::<[u8; 4]>(), d in any::<[u8; 4]>()) {
        let (s, d) = (s.map(u32::from), d.map(u32::from));
        prop_assert_eq!(blend_bytes((ONE, ZERO), s, d, Some(d[3])), s);
        prop_assert_eq!(blend_bytes((ZERO, ONE), s, d, Some(d[3])), d);
    }

    /// GL.RASTER.TRIANGLE: the rounding is the same either side of the viewport's center, stays
    /// within half a step of NDC times half the size less 1/256, and never moves a coordinate
    /// past another
    #[test]
    fn snap_is_symmetric_and_monotone(o in -4096i64..4096, size in 1u32..4096, a in -1f64..1.0, b in -1f64..1.0) {
        let (o, size) = (Sub(o), f64::from(size));
        let (sa, sm) = (Sub::snap(a, size, o), Sub::snap(-a, size, o));
        prop_assert_eq!(sa.0 - o.0, o.0 - sm.0);
        let want = a * (size * 128.0 - 1.0);
        // (snap rounds to 1/65536 before the half to even, so a value up to 2^-17 under a half
        // rounds as the half: 0.5 + 2^-17 at most, as o 0, size 583, a 0.24502499231386463 found)
        prop_assert!(((sa.0 - o.0) as f64 - want).abs() <= 0.5 + 1.0 / 131072.0 + 1e-12);
        if a <= b {
            prop_assert!(sa <= Sub::snap(b, size, o));
        }
    }

    /// GL.RASTER.TRIANGLE: the view volume's sides land 1/256 inside the viewport's
    #[test]
    fn snap_keeps_the_sides_inside(o in -4096i64..4096, size in 1i64..4096) {
        let o = Sub(o);
        prop_assert_eq!(Sub::snap(1.0, size as f64, o), Sub(o.0 + size * 128 - 1));
        prop_assert_eq!(Sub::snap(-1.0, size as f64, o), Sub(o.0 - size * 128 + 1));
    }

    /// GL.RASTER.CLIP.XY: a triangle inside the view volume is left as it is; any other is cut to
    /// a convex polygon inside it whose every vertex lies on the triangle
    #[test]
    fn clip_cuts_to_the_volume(v in any::<[(i16, i16); 3]>(), scale in 1f64..8.0) {
        let t = v.map(|(x, y)| [f64::from(x) / 32768.0 * scale, f64::from(y) / 32768.0 * scale]);
        let h = t.map(|c| [c[0] as f32, c[1] as f32, 0.0, 1.0]);
        let t = h.map(|c| [f64::from(c[0]), f64::from(c[1])]);
        let p: Vec<[f64; 2]> = tri::clip(&h).iter().map(|c| {
            assert!(c[2] == 0.0 && c[3] == 1.0);
            [f64::from(c[0]), f64::from(c[1])]
        }).collect();
        if t.iter().all(|c| c[0].abs() <= 1.0 && c[1].abs() <= 1.0) {
            prop_assert_eq!(p, t.to_vec());
            return Ok(());
        }
        let area = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
        let side = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        for c in &p {
            prop_assert!(c[0].abs() <= 1.0 + 1e-6 && c[1].abs() <= 1.0 + 1e-6);
            for k in 0..3 {
                let s = side(t[k], t[(k + 1) % 3], *c) * area.signum();
                prop_assert!(s >= -1e-5 * (1.0 + area.abs()), "{c:?} off the triangle");
            }
        }
        if p.len() >= 3 {
            let turns: Vec<f64> = (0..p.len()).map(|k| side(p[k], p[(k + 1) % p.len()], p[(k + 2) % p.len()]) * area.signum()).collect();
            prop_assert!(turns.iter().all(|t| *t >= -1e-5), "not convex: {p:?}");
        }
    }

    /// GL 1.1 3.4.1: a 1-pixel line has at most one pixel per column when x-major (per row
    /// when y-major), each within a pixel of the segment, as many as it spans along that axis
    /// give or take two, and moving both ends and the viewport by whole pixels moves the
    /// pixels with them (the ends snap toward the viewport's center, GL.RASTER.LINE)
    #[test]
    fn line_is_thin_and_moves_with_its_ends(v in any::<[(u8, u8); 2]>(), dx in -3i64..4, dy in -3i64..4) {
        let end = |(x, y): (u8, u8)| [f64::from(x) / 32.0 + 4.0, f64::from(y) / 32.0 + 4.0];
        let (a, b) = (end(v[0]), end(v[1]));
        let p = tri::line(a, b, [0, 0, 16, 16], tri::Stroke::Aliased(tri::LineWidth::ONE));
        let xmajor = (b[0] - a[0]).abs() > (b[1] - a[1]).abs();
        let mut seen = std::collections::BTreeSet::new();
        for &(x, y) in &p {
            prop_assert!(seen.insert(if xmajor { x } else { y }), "two pixels in one column or row: {p:?}");
            let (xc, yc) = (x as f64 + 0.5, y as f64 + 0.5);
            let t = (0..=256).map(|k| f64::from(k) / 256.0).map(|t| (a[0] + t * (b[0] - a[0]) - xc).hypot(a[1] + t * (b[1] - a[1]) - yc)).fold(f64::INFINITY, f64::min);
            prop_assert!(t <= 1.5, "({x}, {y}) {t} from the line");
        }
        let span = if xmajor { (b[0] - a[0]).abs() } else { (b[1] - a[1]).abs() };
        prop_assert!((p.len() as f64 - span).abs() <= 2.0, "{} pixels for {span}", p.len());
        let shift = |c: [f64; 2]| [c[0] + dx as f64, c[1] + dy as f64];
        let q = tri::line(shift(a), shift(b), [dx, dy, 16, 16], tri::Stroke::Aliased(tri::LineWidth::ONE));
        let moved: Vec<(i64, i64)> = p.iter().map(|(x, y)| (x + dx, y + dy)).collect();
        prop_assert_eq!(q, moved);
    }

    /// GL.RASTER.LINE.WIDE: a line of width n (not a diagonal, inside the viewport) has n
    /// pixels in a run across each of its 1-pixel line's steps, the 1-pixel line's pixel among
    /// them
    #[test]
    fn wide_lines_hold_the_thin_line(v in any::<[(u8, u8); 2]>(), w in 1f32..10.4) {
        let end = |(x, y): (u8, u8)| [f64::from(x) / 64.0 + 6.0, f64::from(y) / 64.0 + 6.0];
        let (a, b) = (end(v[0]), end(v[1]));
        let thin = tri::line(a, b, [0, 0, 16, 16], tri::Stroke::Aliased(tri::LineWidth::ONE));
        let n = tri::LineWidth::of(w);
        let wide = tri::line(a, b, [0, 0, 16, 16], tri::Stroke::Aliased(n));
        let (dx, dy) = ((b[0] - a[0]).abs(), (b[1] - a[1]).abs());
        // (not within the snapping of a diagonal)
        prop_assume!((dx - dy).abs() > 1.0 / 32.0);
        let xmajor = dx > dy;
        prop_assert_eq!(wide.len() as i64, thin.len() as i64 * n.pixels());
        for (x, y) in &thin {
            let run: Vec<i64> = wide.iter().filter(|p| if xmajor { p.0 == *x } else { p.1 == *y }).map(|p| if xmajor { p.1 } else { p.0 }).collect();
            prop_assert_eq!(run.len() as i64, n.pixels());
            prop_assert!(run.windows(2).all(|r| r[1] == r[0] + 1), "not one run: {run:?}");
            prop_assert!(run.contains(if xmajor { y } else { x }), "({x}, {y}) not in {run:?}");
        }
    }

    /// A window position's DIB pixel only moves on as the position does: a 32nd further right
    /// (or down) never lands left (or up) of where it was, nor more than a pixel on; a position
    /// exactly between two 16ths lands on one of its neighbors' pixels, whichever its
    /// normalized coordinate; off the window, the pixel goes on past its edge
    #[test]
    fn device_pixels_move_on_with_the_position(x in -800i32..800, y in -800i32..800, n in any::<[i8; 2]>()) {
        let at = |x: i32, y: i32| tri::DevicePixel::of([f64::from(x) / 32.0, f64::from(y) / 32.0], [f64::from(n[0]) / 64.0, f64::from(n[1]) / 64.0], 16);
        let (p, right, down) = (at(x, y), at(x + 1, y), at(x, y - 1));
        prop_assert!(right.x - p.x == 0 || right.x - p.x == 1, "{p:?} {right:?}");
        prop_assert!(down.row - p.row == 0 || down.row - p.row == 1, "{p:?} {down:?}");
        prop_assert_eq!((right.row, down.x), (p.row, p.x));
        if x.rem_euclid(2) == 1 {
            prop_assert!(p.x == at(x - 1, y).x || p.x == right.x);
        }
    }

    /// GL.RASTER.LINE.FLAT: a flat-shaded line is one pixel per major step from its first end's,
    /// each next to the last, the second end's not drawn; drawn the other way it lights the same
    /// pixels between the ends (the ties go the same way whichever way it runs), each within half
    /// a pixel across of the line between the ends' pixels
    #[test]
    fn flat_lines_run_alike_both_ways(v in any::<[(i8, i8); 2]>()) {
        let p = |(x, row): (i8, i8)| tri::DevicePixel { x: i64::from(x), row: i64::from(row) };
        let (a, b) = (p(v[0]), p(v[1]));
        let fwd = tri::flat_line(a, b);
        let (dx, dy) = (b.x - a.x, b.row - a.row);
        prop_assert_eq!(fwd.len() as i64, dx.abs().max(dy.abs()));
        if let Some(first) = fwd.first() {
            prop_assert_eq!(*first, a);
        }
        prop_assert!(!fwd.contains(&b));
        for w in fwd.windows(2) {
            prop_assert!((w[1].x - w[0].x).abs() <= 1 && (w[1].row - w[0].row).abs() <= 1);
        }
        for q in &fwd {
            // twice the distance across, in units of the major length
            let across = (dx * (q.row - a.row) - dy * (q.x - a.x)).abs() * 2;
            prop_assert!(across <= dx.abs().max(dy.abs()), "{q:?} off the line {a:?} {b:?}");
        }
        let mut one: Vec<_> = fwd.iter().copied().chain([b]).collect();
        let mut other: Vec<_> = tri::flat_line(b, a).into_iter().chain([a]).collect();
        one.sort_by_key(|q| (q.x, q.row));
        other.sort_by_key(|q| (q.x, q.row));
        one.dedup();
        other.dedup();
        prop_assert_eq!(one, other);
    }

    /// GL.RASTER.LINE.WIDE: a wide y-major line entering through the left edge has the
    /// window's first width columns at its first row, whatever its slope
    #[test]
    fn entering_lines_start_at_the_edge(y in 256u16..3584, b in any::<(u16, u16)>(), w in 2i64..10) {
        let a = [0.0, f64::from(y) / 256.0];
        let b = [f64::from(b.0 % 1024) / 256.0 + 1.0, f64::from(b.1 % 2048) / 256.0 + if b.1 % 2 == 0 { 8.0 } else { -6.0 } + 4.0];
        prop_assume!((b[1] - a[1]).abs() > (b[0] - a[0]).abs() + 0.1);
        let p = tri::line(a, b, [0, 0, 16, 16], tri::Stroke::Entering(tri::LineWidth::of(w as f32), tri::WindowEdge(0)));
        let Some(&(_, row)) = p.first() else { return Ok(()) };
        let mut first: Vec<i64> = p.iter().filter(|q| q.1 == row).map(|q| q.0).collect();
        first.sort();
        prop_assert_eq!(first, (0..w).collect::<Vec<i64>>());
    }

    /// GL.RASTER.INTERPOLATE.LINECOLOR: a smooth line's first pixel has its first end's color,
    /// and the color moves by the same amount at each pixel, reaching the other end's color one
    /// major length on, whichever way the line runs
    #[test]
    fn line_colors_run_between_the_ends(a in 0i64..16, len in 1i64..16, down in any::<bool>(), c in any::<[[u8; 4]; 2]>()) {
        let c = c.map(|e| e.map(|v| f32::from(v) / 255.0));
        let (m0, m1) = if down { (a + len, a) } else { (a, a + len) };
        let major = [Sub(m0 * 256 + 128), Sub(m1 * 256 + 128)];
        let at = |j| tri::line_color(major, c, m0, j);
        prop_assert_eq!(at(m0), c[0]);
        let end = at(m1);
        for k in 0..4 {
            prop_assert!((end[k] - c[1][k]).abs() < 1e-5, "{k}: {} for {}", end[k], c[1][k]);
        }
    }

    /// GL.DRAW.BUFFER: the bytes glDrawPixels reads are the given ones, then the runner's
    /// zeros to the end of its buffer; past the buffer is no answer
    #[test]
    fn draw_reads_the_given_bytes_then_zeros(bytes in proptest::collection::vec(any::<u8>(), 1..512), at in 0usize..1024, n in 1usize..16) {
        let g = Given::new(bytes.clone());
        let want: Vec<u8> = (at..at + n).map(|i| bytes.get(i).copied().unwrap_or(0)).collect();
        prop_assert_eq!(g.bytes(at, n).ok().map(<[u8]>::to_vec), Some(want));
        prop_assert!(g.bytes(Given::BUFFER - n + 1, n).is_err());
        prop_assert!(g.bytes(Given::BUFFER - n, n).is_ok());
    }

    /// GL.RASTER.LINE.SMOOTH, GL.RASTER.LINE.SMOOTH.WIDE: a smooth line by the general path
    /// covers each pixel by more than 0 and at most 1, once per row it walks, and a line of one
    /// color keeps it (only alpha takes the coverage)
    #[test]
    fn aa_lines_cover_at_most_one(v in any::<[(u16, u16); 2]>(), w in 1u16..80, c in any::<[u8; 4]>()) {
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + f32::from(x % 1024) / 64.0, tri::RASTER_BIAS + f32::from(y % 1024) / 64.0]));
        let c = c.map(f32::from);
        let p = tri::aa_line(r[0], r[1], [c, c], tri::AaWidth::of(f32::from(w) / 8.0), None);
        let mut seen = std::collections::BTreeSet::new();
        for f in &p {
            prop_assert!(f.cover > 0.0 && f.cover <= 1.0);
            prop_assert_eq!(&f.c[..3], &c[..3]);
            prop_assert_eq!(f.c[3], c[3] * f.cover);
            prop_assert!(seen.insert((f.x, f.row)), "{:?} twice", (f.x, f.row));
        }
    }

    /// GL.RASTER.LINE.STIPPLE: a smooth line stippled by all ones is the line unstippled, by
    /// none nothing, and by any pattern no pixel covered more than unstippled
    #[test]
    fn aa_stipples_only_take_coverage(v in any::<[(u16, u16); 2]>(), pattern in any::<u16>(), repeat in 1u8..=255) {
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + f32::from(x % 1024) / 64.0, tri::RASTER_BIAS + f32::from(y % 1024) / 64.0]));
        let (c, w) = ([[255.0; 4]; 2], tri::AaWidth::of(1.0));
        let plain = tri::aa_line(r[0], r[1], c, w, None);
        prop_assert_eq!(&tri::aa_line(r[0], r[1], c, w, Some(tri::AaStipple { pattern: 0xffff, repeat, at: (0, 0) })), &plain);
        let none = tri::aa_line(r[0], r[1], c, w, Some(tri::AaStipple { pattern: 0, repeat, at: (0, 0) }));
        prop_assert!(none.is_empty());
        for f in tri::aa_line(r[0], r[1], c, w, Some(tri::AaStipple { pattern, repeat, at: (0, 0) })) {
            let p = plain.iter().find(|p| (p.x, p.row) == (f.x, f.row));
            prop_assert!(p.is_some_and(|p| f.cover <= p.cover));
        }
    }

    /// GL.RASTER.LINE.SMOOTH.WIDE: the width held to 0.5 ..= 10 and a multiple of 1/8, the
    /// nearest one
    #[test]
    fn aa_widths_round_to_eighths(w in 0.01f32..20.0) {
        let g = tri::AaWidth::of(w).get();
        prop_assert!((0.5..=10.0).contains(&g));
        prop_assert_eq!((g * 8.0).fract(), 0.0);
        prop_assert!((0.5..10.0).contains(&w) == false || (g - w).abs() <= 1.0 / 16.0 + 1e-6);
    }

    /// GL.RASTER.LINE.SMOOTH.BLEND: a fast smooth line's two pixels at each step are next to
    /// each other across it, weighing 255 between them, one step along apart from the last
    #[test]
    fn fast_aa_line_pairs_weigh_one_pixel(v in any::<[(u16, u16); 2]>()) {
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + 2.0 + f32::from(x % 768) / 64.0, tri::RASTER_BIAS + 2.0 + f32::from(y % 768) / 64.0]));
        let b = tri::RASTER_BIAS as i64;
        let p = tri::aa_line_fast(r[0], r[1], [[255.0; 4]; 2], 255.0, [[b, b + 16], [b, b + 16]]);
        prop_assert_eq!(p.len() % 2, 0);
        for pair in p.chunks(2) {
            prop_assert_eq!(u16::from(pair[0].weight) + u16::from(pair[1].weight), 255);
            prop_assert_eq!((pair[0].x - pair[1].x).abs() + (pair[0].row - pair[1].row).abs(), 1);
        }
        for two in p.chunks(2).collect::<Vec<_>>().windows(2) {
            let (s, t) = (two[0][0], two[1][0]);
            prop_assert!((s.x - t.x).abs() <= 1 && (s.row - t.row).abs() <= 1);
        }
    }

    /// GL.RASTER.LINE.SMOOTH: a fast smooth line's step colors in channel units (5-bit ones,
    /// raster-open-32) stay between its ends' for its first |major| steps (later ones go on
    /// past the second end's, wrapping), the two pixels of a step in one color
    #[test]
    fn fast_aa_line_colors_stay_between_the_ends(v in any::<[(u16, u16); 2]>(), c in any::<[[u8; 3]; 2]>()) {
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + 2.0 + f32::from(x % 768) / 64.0, tri::RASTER_BIAS + 2.0 + f32::from(y % 768) / 64.0]));
        let b = tri::RASTER_BIAS as i64;
        let unit = |k: [u8; 3]| [f32::from(k[0] % 32), f32::from(k[1] % 32), f32::from(k[2] % 32), 0.0];
        let ends = [unit(c[0]), unit(c[1])];
        let p = tri::aa_line_fast(r[0], r[1], ends, 255.0, [[b, b + 16], [b, b + 16]]);
        let major = (r[1].0[0] - r[0].0[0]).abs().max((r[1].0[1] - r[0].0[1]).abs());
        for (step, pair) in p.chunks(2).enumerate() {
            prop_assert_eq!(pair[0].c, pair[pair.len() - 1].c);
            if step as f32 > major {
                continue;
            }
            for k in 0..3 {
                let (lo, hi) = (ends[0][k].min(ends[1][k]), ends[0][k].max(ends[1][k]));
                prop_assert!((lo..=hi).contains(&f32::from(pair[0].c[k])), "{:?} {:?}", pair[0].c, ends);
            }
        }
    }

    /// GL.RASTER.INTERPOLATE.LINE: a line inside the view volume is not cut; one with both ends
    /// past the same plane is gone; a cut end lies on the plane it was cut at, within rounding,
    /// and its color between the ends'
    #[test]
    fn cut_lines_end_on_the_plane(v in any::<[(i16, i16); 2]>(), c in any::<[u8; 2]>()) {
        let end = |(x, y): (i16, i16), c: u8| tri::ClipEnd { clip: [f32::from(x) / 8192.0, f32::from(y) / 8192.0, 0.0, 1.0], c: [f32::from(c); 4] };
        let (a, b) = (end(v[0], c[0]), end(v[1], c[1]));
        let inside = |e: &tri::ClipEnd| e.clip[..2].iter().all(|v| v.abs() <= 1.0);
        match tri::cut_line(a, b, true) {
            None => prop_assert!(!(inside(&a) && inside(&b))),
            Some((e, cut)) => {
                prop_assert_eq!(cut, [!inside(&a), !inside(&b)]);
                let (lo, hi) = (f32::from(c[0].min(c[1])), f32::from(c[0].max(c[1])));
                for (k, e) in e.iter().enumerate() {
                    prop_assert!(e.clip[..2].iter().all(|v| v.abs() <= 1.0 + 1e-5), "{:?}", e);
                    if cut[k] {
                        prop_assert!(e.clip[..2].iter().any(|v| (v.abs() - 1.0).abs() <= 1e-5));
                    }
                    prop_assert!(e.c[0] >= lo - 1e-3 && e.c[0] <= hi + 1e-3);
                }
            }
        }
    }

    /// GL.POLYGON.SMOOTH: a smooth triangle covers the same samples, with the same color
    /// points, whichever order its corners are listed in (each edge's line is built from its
    /// end of smaller y, its side from the third corner)
    #[test]
    fn aa_triangles_ignore_the_corner_order(v in any::<[(u16, u16); 3]>(), nicest in any::<bool>()) {
        let grid = if nicest { tri::AaGrid::Eight } else { tri::AaGrid::Four };
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + f32::from(x % 1024) / 64.0, tri::RASTER_BIAS + f32::from(y % 1024) / 64.0]));
        let a = tri::aa_triangle(r, grid);
        for p in [[1, 2, 0], [2, 0, 1], [0, 2, 1], [2, 1, 0], [1, 0, 2]] {
            prop_assert_eq!(&a, &tri::aa_triangle(p.map(|i| r[i]), grid));
        }
        let n = grid.side() * grid.side();
        prop_assert!(a.iter().all(|p| (1..=n).contains(&p.cover.hits()) && p.cover.fraction() > 0.0 && p.cover.fraction() <= 1.0));
    }

    /// GL.POLYGON.SMOOTH: the two triangles of a convex quad split along a diagonal take each
    /// sample at most once between them (a sample on the diagonal goes to one side only), and
    /// a pixel wholly inside one is wholly covered
    #[test]
    fn aa_triangles_share_a_diagonal(v in any::<[(u16, u16); 2]>(), lean in 0u16..256) {
        let at = |c: u16| f32::from(c % 512) / 32.0;
        let (x0, x1) = (at(v[0].0).min(at(v[1].0)), at(v[0].0).max(at(v[1].0)));
        let (y0, y1) = (at(v[0].1).min(at(v[1].1)), at(v[0].1).max(at(v[1].1)));
        let d = f32::from(lean) / 256.0 * (x1 - x0);
        let q = [(x0, y0), (x1, y0), (x1 + d, y1), (x0 + d, y1)].map(|(x, y)| tri::Raster([tri::RASTER_BIAS + x, tri::RASTER_BIAS + y]));
        let mut got = std::collections::BTreeMap::new();
        for t in [[0, 1, 2], [0, 2, 3]] {
            for p in tri::aa_triangle(t.map(|i| q[i]), tri::AaGrid::Four) {
                *got.entry((p.x, p.row)).or_insert(0u16) += p.cover.hits();
            }
        }
        prop_assert!(got.values().all(|n| *n <= 16));
    }

    /// GL.POLYGON.SMOOTH: a pixel whose every sample a triangle covers takes its colors at the
    /// pixel's center, and a plane through equal values gives that value back within a step
    #[test]
    fn aa_planes_of_one_value_hold_it(v in any::<[(u16, u16); 3]>(), c in 0u8..=255) {
        let r = v.map(|(x, y)| tri::Raster([tri::RASTER_BIAS + f32::from(x % 1024) / 64.0, tri::RASTER_BIAS + f32::from(y % 1024) / 64.0]));
        let c = f32::from(c);
        for p in tri::aa_triangle(r, tri::AaGrid::Four) {
            if p.cover.hits() == 16 {
                prop_assert_eq!(p.at, [(p.x + 40960) as f32 + 0.5, (p.row + 40960) as f32 + 0.5]);
            }
            let got = tri::aa_plane(r, [c; 3], p.at);
            prop_assert!((got - c).abs() <= c.max(1.0) * 4.0 * f32::EPSILON, "{} {}", got, c);
        }
    }

    /// A triangle (convex) crosses each plane of the view volume at most twice; a bow tie
    /// moved past one plane by more than its middle crosses it four times
    #[test]
    fn convex_outlines_cross_a_plane_at_most_twice(v in any::<[(i16, i16); 3]>(), scale in 1f32..8.0, shift in 1.3f32..1.7) {
        let t = v.map(|(x, y)| [f32::from(x) / 32768.0 * scale, f32::from(y) / 32768.0 * scale, 0.0, 1.0]);
        prop_assert!(tri::clip_crossing(&t).1 <= 2);
        let tie = [[-1.0, -1.0], [1.0, 1.0], [1.0, -1.0], [-1.0, 1.0]].map(|[x, y]: [f32; 2]| [x * 0.75, y * 0.75 + shift, 0.0, 1.0]);
        let (p, n) = tri::clip_crossing(&tie);
        prop_assert_eq!(n, 4, "{:?}", p);
    }

    /// GL 1.1 3.5.1: which pixels a triangle covers does not depend on the order of its vertices
    #[test]
    fn triangle_ignores_vertex_order(v in any::<[(i16, i16); 3]>()) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8), y: Sub(i64::from(y) / 8) });
        let sorted = |mut p: Vec<(i64, i64)>| { p.sort(); p };
        let a = sorted(tri::pixels(&v, 16, 16));
        for p in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            prop_assert_eq!(&a, &sorted(tri::pixels(&p.map(|k| v[k]), 16, 16)));
        }
    }

    /// GL 1.1 3.5.1: two triangles sharing an edge (a convex quad split along a diagonal) draw
    /// no pixel twice and leave no center 1/128 pixel inside the quad undrawn
    #[test]
    fn shared_edges_tile(c in (-2000i64..6000, -2000i64..6000), r in (256f64..3000.0, 256f64..3000.0), t in 0f64..1.0, j in any::<[u8; 4]>()) {
        // four points on an ellipse around c at increasing angles: a convex quad
        let q: Vec<Vertex> = (0..4).map(|k| {
            let a = t * std::f64::consts::TAU + k as f64 * std::f64::consts::FRAC_PI_2 + f64::from(j[k]) / 256.0 * 1.2;
            Vertex { x: Sub(c.0 + (r.0 * a.cos()) as i64), y: Sub(c.1 + (r.1 * a.sin()) as i64) }
        }).collect();
        let turn = |a: Vertex, b: Vertex, c: Vertex| (b.x.0 - a.x.0) * (c.y.0 - b.y.0) - (b.y.0 - a.y.0) * (c.x.0 - b.x.0);
        // (rounding to whole units can flatten a corner)
        if !(0..4).all(|k| turn(q[k], q[(k + 1) % 4], q[(k + 2) % 4]) > 0) {
            return Ok(());
        }
        let (p, s) = (tri::pixels(&[q[0], q[1], q[2]], 32, 32), tri::pixels(&[q[0], q[2], q[3]], 32, 32));
        let mut both: Vec<(i64, i64)> = p.iter().chain(&s).copied().collect();
        both.sort();
        let n = both.len();
        both.dedup();
        prop_assert_eq!(both.len(), n);
        let cross = |a: Vertex, b: Vertex, x: i64, y: i64| (b.x.0 - a.x.0) as i128 * (y - a.y.0) as i128 - (b.y.0 - a.y.0) as i128 * (x - a.x.0) as i128;
        for y in 0..32i64 {
            for x in 0..32i64 {
                let (cx, cy) = (x * 256 + 128, y * 256 + 128);
                let e: Vec<i128> = (0..4).map(|k| cross(q[k], q[(k + 1) % 4], cx, cy)).collect();
                // (the walk strays from the line by its first x's rounding, at most 1/512 pixel,
                // and its step's, 2^-23 a row: a center 1/128 pixel inside along x is drawn)
                let clear = (0..4).all(|k| {
                    let dy = i128::from((q[(k + 1) % 4].y.0 - q[k].y.0).abs());
                    dy == 0 || e[k].abs() >= 2 * dy
                });
                if clear && (e.iter().all(|v| *v > 0) || e.iter().all(|v| *v < 0)) {
                    prop_assert!(both.binary_search(&(x, y)).is_ok(), "center ({x}, {y}) inside the quad undrawn");
                }
            }
        }
    }

    /// GL.RASTER.TRIANGLE: a convex polygon walked as a whole draws what its fan's triangles
    /// draw, except at centers within 1/128 pixel of a diagonal, and any rotation of its
    /// vertices draws the same
    #[test]
    fn whole_polygon_is_its_fan_off_the_diagonals(c in (-2000i64..6000, -2000i64..6000), r in (256f64..3000.0, 256f64..3000.0), t in 0f64..1.0, j in any::<[u8; 5]>(), back in any::<bool>()) {
        let mut q: Vec<Vertex> = (0..5).map(|k| {
            let a = t * std::f64::consts::TAU + k as f64 * (std::f64::consts::TAU / 5.0) + f64::from(j[k]) / 256.0;
            Vertex { x: Sub(c.0 + (r.0 * a.cos()) as i64), y: Sub(c.1 + (r.1 * a.sin()) as i64) }
        }).collect();
        if back {
            q.reverse();
        }
        let turn = |a: Vertex, b: Vertex, c: Vertex| (b.x.0 - a.x.0) * (c.y.0 - b.y.0) - (b.y.0 - a.y.0) * (c.x.0 - b.x.0);
        let sign = turn(q[0], q[1], q[2]).signum();
        if sign == 0 || !(0..5).all(|k| turn(q[k], q[(k + 1) % 5], q[(k + 2) % 5]).signum() == sign) {
            return Ok(());
        }
        let sorted = |mut p: Vec<(i64, i64)>| { p.sort(); p };
        let whole = sorted(tri::pixels(&q, 32, 32));
        for k in 1..5 {
            let rot: Vec<Vertex> = (0..5).map(|i| q[(i + k) % 5]).collect();
            prop_assert_eq!(&whole, &sorted(tri::pixels(&rot, 32, 32)));
        }
        let fan = sorted((1..4).flat_map(|k| tri::pixels(&[q[0], q[k], q[k + 1]], 32, 32)).collect());
        let near = |x: i64, y: i64| (2..4).any(|k| {
            let (a, b) = (q[0], q[k]);
            let dy = i128::from((b.y.0 - a.y.0).abs()).max(1);
            let e = i128::from(b.x.0 - a.x.0) * i128::from(y * 256 + 128 - a.y.0) - i128::from(b.y.0 - a.y.0) * i128::from(x * 256 + 128 - a.x.0);
            e.abs() < 2 * dy
        });
        let off = |p: &Vec<(i64, i64)>| p.iter().copied().filter(|(x, y)| !near(*x, *y)).collect::<Vec<_>>();
        prop_assert_eq!(off(&whole), off(&fan));
    }

    /// GL.RASTER.TRIANGLE: moving a triangle by whole pixels moves its pixels the same
    #[test]
    fn triangle_moves_with_whole_pixels(v in any::<[(i16, i16); 3]>(), dx in -4i64..4, dy in -4i64..4) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 16 + 4096), y: Sub(i64::from(y) / 16 + 4096) });
        let m = v.map(|p| Vertex { x: Sub(p.x.0 + dx * 256), y: Sub(p.y.0 + dy * 256) });
        let (a, mut b) = (tri::pixels(&v, 40, 40), tri::pixels(&m, 40, 40));
        let mut a: Vec<(i64, i64)> = a.into_iter().map(|(x, y)| (x + dx, y + dy)).filter(|(x, y)| (0..40).contains(x) && (0..40).contains(y)).collect();
        a.sort();
        b.retain(|(x, y)| (x - dx) >= 0 && (x - dx) < 40 && (y - dy) >= 0 && (y - dy) < 40);
        b.sort();
        prop_assert_eq!(a, b);
    }
    /// GL 1.1 3.5.1 (smooth shading): a fan triangle's color plane does not depend on the order
    /// of its corners, gives each corner its own color, and keeps every covered pixel's color
    /// between the corners' colors
    #[test]
    fn color_plane_is_barycentric(v in any::<[(i16, i16); 3]>(), c in any::<[[u8; 4]; 3]>()) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8), y: Sub(i64::from(y) / 8) });
        let c = c.map(|q| q.map(|b| f64::from(b) / 255.0));
        let area = (v[1].x.0 - v[0].x.0) * (v[2].y.0 - v[0].y.0) - (v[2].x.0 - v[0].x.0) * (v[1].y.0 - v[0].y.0);
        prop_assume!(area != 0);
        // (no slivers: a covered pixel's center may lie a rasterizer's rounding outside the
        // corners, its color the farther past theirs the thinner the triangle; every height at
        // least a pixel)
        let len = |i: usize, j: usize| ((v[j].x.0 - v[i].x.0) as f64).hypot((v[j].y.0 - v[i].y.0) as f64);
        prop_assume!([(0, 1), (1, 2), (2, 0)].iter().all(|&(i, j)| (area.abs() as f64) / len(i, j) >= 256.0));
        let near = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
        for (x, y) in tri::pixels(&v, 16, 16) {
            let at = tri::plane(v, c, x, y);
            for p in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                prop_assert!(near(at, tri::plane(p.map(|k| v[k]), p.map(|k| c[k]), x, y)));
            }
            for k in 0..4 {
                let (lo, hi) = (c.iter().map(|q| q[k]).fold(f64::INFINITY, f64::min), c.iter().map(|q| q[k]).fold(f64::NEG_INFINITY, f64::max));
                prop_assert!(at[k] >= lo - 1e-9 && at[k] <= hi + 1e-9, "{at:?} out of {c:?}");
            }
        }
        for k in 0..3 {
            // a corner on a pixel center gets its own color
            let w = v.map(|p| Vertex { x: Sub(p.x.0 * 256 + 128), y: Sub(p.y.0 * 256 + 128) });
            prop_assert!(near(tri::plane(w, c, v[k].x.0, v[k].y.0), c[k]));
        }
    }
    /// GL.RASTER.INTERPOLATE.DEPTH: a triangle's depths cover exactly its pixels, one depth
    /// everywhere when its corners share it, and do not depend on the order of corners at
    /// different heights
    #[test]
    fn depths_cover_the_pixels_and_ignore_order(v in any::<[(i16, i16); 3]>(), z in any::<[u16; 3]>(), wide in any::<bool>()) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8), y: Sub(i64::from(y) / 8) });
        let z = z.map(|q| f32::from(q) / 65535.0);
        let bits = if wide { tri::DepthBits::D32 } else { tri::DepthBits::D16 };
        let sorted = |mut p: Vec<(i64, i64, tri::Fixed)>| { p.sort_by_key(|q| (q.0, q.1)); p };
        let d = sorted(tri::depths(v, z, 16, 16, bits));
        let mut at: Vec<(i64, i64)> = d.iter().map(|q| (q.0, q.1)).collect();
        at.sort();
        let mut px = tri::pixels(&v, 16, 16);
        px.sort();
        prop_assert_eq!(&at, &px);
        let flat = sorted(tri::depths(v, [z[0]; 3], 16, 16, bits));
        prop_assert!(flat.windows(2).all(|w| w[0].2 == w[1].2));
        if v[0].y != v[1].y && v[1].y != v[2].y && v[0].y != v[2].y {
            for p in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
                prop_assert_eq!(&d, &sorted(tri::depths(p.map(|k| v[k]), p.map(|k| z[k]), 16, 16, bits)));
            }
        }
    }

    /// GL.RASTER.INTERPOLATE.DEPTH: where the depth changes by less than 1/64 per pixel, each
    /// pixel's depth is within 2^-16 of the plane through the corners
    #[test]
    fn depths_follow_the_plane(v in any::<[(i16, i16); 3]>(), z in any::<[u16; 3]>()) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8 + 2048), y: Sub(i64::from(y) / 8 + 2048) });
        let z = z.map(|q| f32::from(q) / 65535.0 * 0.5 + 0.25);
        let c = z.map(f64::from);
        for (x, y, d) in tri::depths(v, z, 16, 16, tri::DepthBits::D32) {
            let plane = tri::plane(v, [[c[0]; 4], [c[1]; 4], [c[2]; 4]], x, y)[0];
            let near = |k: usize| tri::plane(v, [[c[0]; 4], [c[1]; 4], [c[2]; 4]], x + [1, 0][k], y + [0, 1][k])[0];
            prop_assume!((near(0) - plane).abs() < 1.0 / 64.0 && (near(1) - plane).abs() < 1.0 / 64.0);
            let got = f64::from(d.0) / f64::from(1u32 << 31);
            prop_assert!((got - plane).abs() < 1.0 / 65536.0, "{got} vs {plane}");
        }
    }

    /// GL.RASTER.INTERPOLATE.LINE: a line cut to the view volume keeps its ends inside it (to
    /// float32 rounding) and on the line; one inside it is not cut; the cut is the same whichever
    /// end comes first (to rounding)
    #[test]
    fn cut_lines_stay_on_the_line_inside(a in any::<[i16; 4]>(), b in any::<[i16; 4]>()) {
        let c = |v: [i16; 4]| [f32::from(v[0]) / 4096.0, f32::from(v[1]) / 4096.0, f32::from(v[2]) / 4096.0, f32::from(v[3]) / 8192.0 + 4.5];
        let (a, b) = (c(a), c(b));
        let Some((p, q)) = tri::clip_segment(a, b) else {
            return Ok(());
        };
        let inside = |v: [f32; 4]| (0..3).all(|k| v[k].abs() <= v[3] * (1.0 + 1e-5));
        prop_assert!(inside(p) && inside(q), "{p:?} {q:?}");
        // (on the line: each a + t (b - a), one t for every coordinate)
        for v in [p, q] {
            let k = (0..4).max_by(|i, j| (b[*i] - a[*i]).abs().total_cmp(&(b[*j] - a[*j]).abs())).unwrap_or(0);
            let t = if b[k] == a[k] { 0.0 } else { (v[k] - a[k]) / (b[k] - a[k]) };
            for j in 0..4 {
                prop_assert!((a[j] + t * (b[j] - a[j]) - v[j]).abs() < 1e-3, "{v:?} off the line");
            }
        }
        if inside(a) && inside(b) {
            prop_assert_eq!((p, q), (a, b));
        }
        if let Some((q2, p2)) = tri::clip_segment(b, a) {
            for (u, v) in [(p, p2), (q, q2)] {
                prop_assert!((0..4).all(|k| (u[k] - v[k]).abs() < 1e-3), "{u:?} {v:?}");
            }
        }
    }

    /// GL.RASTER.INTERPOLATE.LINE: a line's depths change by one fixed step per pixel, and stay
    /// within 2^-16 of the line between its ends where the step is under 1/64
    #[test]
    fn line_depths_step_evenly(a in 0i64..4096, len in 256i64..4096, back in any::<bool>(), z in any::<[u16; 2]>(), first in 0i64..4) {
        let (a, b) = if back { (Sub(a + len), Sub(a)) } else { (Sub(a), Sub(a + len)) };
        let z = z.map(|q| f32::from(q) / 65535.0 * 0.5 + 0.25);
        let step = (b.0 - a.0).signum();
        let j0 = (a.0 - 128).div_euclid(256) + first * step;
        let d: Vec<i32> = (0..8).map(|n| tri::line_depth([a, b], z, j0, j0 + n * step, tri::DepthBits::D32).0).collect();
        // (as 32-bit integers, wrapping)
        prop_assert!(d.windows(3).all(|w| w[1].wrapping_sub(w[0]) == w[2].wrapping_sub(w[1])));
        let per = f64::from(z[1] - z[0]) / ((b.0 - a.0).abs() as f64 / 256.0);
        prop_assume!(per.abs() < 1.0 / 64.0);
        for (n, got) in d.iter().enumerate() {
            let j = j0 + n as i64 * step;
            let t = ((j * 256 + 128 - a.0) as f64) / ((b.0 - a.0) as f64);
            let want = f64::from(z[0]) + t * f64::from(z[1] - z[0]);
            prop_assume!((0.0..1.0).contains(&want));
            prop_assert!((f64::from(*got) / f64::from(1u32 << 31) - want).abs() < 1.0 / 65536.0, "{got} vs {want}");
        }
    }
}

/// (the case proptest found, CI 2026-11-20) 0.245025 of 583 pixels scales to 18284.499998...:
/// the 1/65536 pre-rounding makes it a half, which goes to even, 2^-17 more than half a step
#[test]
fn snap_pre_rounding_bound() {
    let a = 0.24502499231386463f64;
    let s = Sub::snap(a, 583.0, Sub(0));
    let want = a * (583.0 * 128.0 - 1.0);
    let err = (s.0 as f64 - want).abs();
    assert!(err > 0.5 && err <= 0.5 + 1.0 / 131072.0, "{err}");
}
