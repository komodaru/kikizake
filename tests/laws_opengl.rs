//! Laws of the OpenGL model: what the GL 1.1 specification states as properties, whatever
//! Windows' measured details turn out to be.
mod pairs;
use pairs::zip_eq;
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
    /// between the corners' colors, but for the rasterizer's rounding of its edges
    #[test]
    fn color_plane_is_barycentric(v in any::<[(i16, i16); 3]>(), c in any::<[[u8; 4]; 3]>()) {
        color_plane_holds(v, c)?;
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

    /// GL.POLYGON.SMOOTH: in color index mode an antialiased fragment keeps its index's whole
    /// part above the low 4 bits and carries the coverage, rounded to fifteenths, in them
    /// (raster-open-45, 46)
    #[test]
    fn index_coverage_fills_the_low_four_bits(index in 0u16..4096, frac in 0.0f32..1.0, cover in 0.0f32..=1.0) {
        let v = tri::index_coverage(f32::from(index) + frac * 0.99, cover);
        prop_assert_eq!(v.fract(), 0.0);
        let k = v as i64;
        prop_assert_eq!(k & !15, i64::from(index) & !15);
        prop_assert!(((k & 15) as f32 - cover * 15.0).abs() <= 0.5 + 1e-4);
    }

    /// GL.POLYGON.OFFSET: an offset fill covers the same pixels, no offset is `depths`, and each
    /// pixel moves by the offset (max |slope| times the factor plus the units) to the start's
    /// rounding (raster-open-29, 31)
    #[test]
    fn offset_depths_move_by_the_offset(v in any::<[(i16, i16); 3]>(), z in any::<[u16; 3]>(), factor in -4.0f32..4.0, units in -64i8..64) {
        let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8 + 2048), y: Sub(i64::from(y) / 8 + 2048) });
        // (depths a little apart, so most planes are gentle)
        let z = z.map(|q| f32::from(q) / 65535.0 * 2f32.powi(-12) + 0.5);
        let bits = tri::DepthBits::D32;
        // (planes gentle enough that every row's start, seen or not, stays inside the 32-bit
        // range with the offset: a start past it is i32::MIN, and the rows below step from it)
        let (a, b) = tri::depth_slopes(v, z);
        prop_assume!(a.abs().max(b.abs()) < 1.0 / 256.0);
        let plain = tri::depths(v, z, 16, 16, bits);
        prop_assert_eq!(&plain, &tri::depths_offset(v, z, 16, 16, bits, tri::Offset::NONE));
        let offset = tri::Offset { factor, units: f32::from(units) * 2f32.powi(-23) };
        let moved = tri::depths_offset(v, z, 16, 16, bits, offset);
        prop_assert_eq!(plain.len(), moved.len());
        // (the plane's slopes, as depths has them, bounded by the moves seen)
        let scale = f64::from(1u32 << 31);
        for (p, q) in zip_eq(plain.iter(), &moved) {
            prop_assert_eq!((p.0, p.1), (q.0, q.1));
            let shift = f64::from(q.2 .0.wrapping_sub(p.2 .0));
            let first = f64::from(moved[0].2 .0.wrapping_sub(plain[0].2 .0));
            // (every pixel alike to the rounding of each left edge's start: a float32 ulp of
            // the start, the depth plus the offset)
            prop_assume!(first.abs() < scale / 4.0);
            // (a start past the 32-bit range is the buffer's i32::MIN, not moved by the offset)
            prop_assume!(q.2 .0 > 0 && p.2 .0 > 0 && moved[0].2 .0 > 0 && plain[0].2 .0 > 0);
            prop_assert!((shift - first).abs() <= 512.0 + first.abs() * 2f64.powi(-21), "{shift} {first}");
            prop_assert!(first.abs() <= f64::from(factor.abs()) * 4.0 * scale + 64.0 * 256.0 + 512.0);
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

proptest! {
    /// GL 1.1 3.8.12: glGenTextures gives names no texture and no other glGenTextures holds; the
    /// generic implementation's run is the lowest run of free names from 1 that fits
    #[test]
    fn texture_names_are_fresh_runs(steps in proptest::collection::vec((0u8..3, 1u32..6, 1u32..24), 1..24)) {
        use kikizake::model::opengl::tex::Tex;
        let mut tex = Tex::new();
        let mut held: std::collections::BTreeSet<u32> = std::collections::BTreeSet::new();
        for (op, n, name) in steps {
            match op {
                0 => {
                    let names = tex.gen_names(n);
                    prop_assert_eq!(names.len(), usize::try_from(n).unwrap());
                    let first = names[0];
                    for (k, v) in names.iter().enumerate() {
                        prop_assert_eq!(*v, first + u32::try_from(k).unwrap());
                        prop_assert!(!held.contains(v));
                    }
                    // (no lower run of n free names)
                    for start in 1..first {
                        prop_assert!((start..start + n).any(|v| held.contains(&v)));
                    }
                    held.extend(names);
                }
                1 => {
                    tex.delete(&[name]);
                    held.remove(&name);
                    prop_assert!(!tex.is_texture(name));
                }
                _ => {
                    prop_assert!(tex.bind(0xde1, name).is_ok() || tex.objects.get(&name).is_some_and(|o| o.target != kikizake::model::opengl::tex::Target::Two));
                    held.insert(name);
                    prop_assert!(tex.is_texture(name));
                }
            }
        }
    }

    /// GL 1.1 3.8.1: a texel kept as a byte is the nearest of 255 steps, so keeping it again
    /// changes nothing, and no stored component leaves [0, 1]
    #[test]
    fn byte_texels_keep_their_steps(units in 0f32..=255.0) {
        use kikizake::model::opengl::tex::{kept, Store};
        let b = kept(Store::Bytes, units);
        prop_assert!((0.0..=1.0).contains(&b));
        prop_assert_eq!(kept(Store::Bytes, b * 255.0).to_bits(), b.to_bits());
        prop_assert!((b * 255.0 - units).abs() <= 0.5 + 1e-3);
        let f = kept(Store::Floats, units);
        prop_assert!((f * 255.0 - units).abs() <= 1e-3 * units.max(1.0));
    }

    /// C's %.9g, as the executor prints float queries: nine significant digits, so a float reads
    /// back to itself
    #[test]
    fn nine_digits_read_back_to_the_float(bits in any::<u32>()) {
        use kikizake::model::opengl::tex::g9;
        let v = f32::from_bits(bits);
        prop_assume!(v.is_finite());
        let s = g9(f64::from(v));
        let back: f64 = s.parse().unwrap();
        prop_assert_eq!(back as f32, v, "{}", s);
    }

    /// GL 1.1 3.8.5: NEAREST's texel is inside the level for any s, REPEAT's repeats with
    /// period 1 off texel boundaries and CLAMP's never goes back as s grows (GL.RASTER.TEXTURE.FILTER)
    #[test]
    fn nearest_texels_stay_in_the_level(q in -4096i32..4096, d in 1i32..64, log in 0u32..11) {
        use kikizake::model::opengl::texdraw::{index, Wrap};
        let n = 1u32 << log;
        let s = q as f32 / 256.0;
        for wrap in [Wrap::Repeat, Wrap::Clamp] {
            let i = index(s, n, wrap);
            prop_assert!((0..i64::from(n)).contains(&i));
        }
        // (off texel boundaries: on one a negative s is taken from the top, texture.md 4.3)
        if (s * n as f32).fract() != 0.0 {
            prop_assert_eq!(index(s, n, Wrap::Repeat), index(s + 1.0, n, Wrap::Repeat));
        }
        prop_assert!(index(s, n, Wrap::Clamp) <= index(s + d as f32 / 256.0, n, Wrap::Clamp));
    }

    /// GL 1.1 3.8.6: LINEAR's two texels are neighbours (REPEAT wrapping round), its weight in
    /// [0, 1) (GL.RASTER.TEXTURE.FILTER)
    #[test]
    fn linear_texels_are_neighbours(q in -4096i32..4096, log in 0u32..11) {
        use kikizake::model::opengl::texdraw::{pair, Wrap};
        let n = 1u32 << log;
        let s = q as f32 / 256.0;
        let (i0, i1, a) = pair(s, n, Wrap::Repeat);
        prop_assert!((0..i64::from(n)).contains(&i0));
        prop_assert_eq!(i1, (i0 + 1) % i64::from(n));
        prop_assert!((0.0..1.0).contains(&a));
        let (c0, c1, b) = pair(s, n, Wrap::Clamp);
        prop_assert_eq!(c1, c0 + 1);
        prop_assert!((-1..=i64::from(n)).contains(&c0));
        prop_assert!((0.0..1.0).contains(&b));
    }

    /// GL 1.1 3.8.7: lambda is log2 of rho; the piecewise-linear log2 of rho squared, halved,
    /// is within 0.05 of it (GL.RASTER.TEXTURE.FILTER)
    #[test]
    fn lambda_follows_log2_of_rho(r in 1e-6f32..1e6) {
        use kikizake::model::opengl::texdraw::log2_half;
        prop_assert!((log2_half(r) - 0.5 * r.log2()).abs() <= 0.05);
    }

    /// GL 1.1 3.8.9 table 3.18: REPLACE on RGBA gives the texel whatever the fragment, MODULATE
    /// by a white texel leaves the fragment as it is (GL.RASTER.TEXTURE)
    #[test]
    fn replace_takes_the_texel_and_white_modulates_nothing(c in proptest::array::uniform4(0f32..=255.0), t in proptest::array::uniform4(0f32..=1.0)) {
        use kikizake::model::opengl::tex::Base;
        use kikizake::model::opengl::texdraw::{env, Env};
        let scale = [255.0; 4];
        let r = env(Env::Replace, Base::Rgba, c, t, [0.0; 4], scale);
        prop_assert_eq!(r, t.map(|v| v * 255.0));
        let m = env(Env::Modulate, Base::Rgba, c, [1.0; 4], [0.0; 4], scale);
        prop_assert_eq!(m, c);
    }

    /// GL 1.1 5.4: glGenLists gives n names in one run none of which names a list, the lowest
    /// such run from 1, each then a list (GL.LIST)
    #[test]
    fn list_names_are_fresh_runs(held in proptest::collection::btree_set(1u32..40, 0..20), n in 1u32..6) {
        use kikizake::model::opengl::list::Lists;
        let mut l = Lists::default();
        for k in &held {
            l.lists.insert(*k, Vec::new());
        }
        let first = l.gen_names(n);
        prop_assert!(first >= 1);
        for v in first..first + n {
            prop_assert!(!held.contains(&v));
            prop_assert!(l.lists.contains_key(&v));
        }
        for start in 1..first {
            prop_assert!((start..start + n).any(|v| held.contains(&v)));
        }
    }

    // GL.TEX.GEN: the eye plane is kept times the inverse modelview, so the inverse undoes
    // the matrix to float32's precision: M^-1 M is the identity for a matrix of rotation, scale
    // and translation (the affine path) and with a last row other than 0 0 0 1 (the general one)
    #[test]
    fn eye_plane_inverse_undoes_the_modelview(angle in -3.0f32..3.0, sx in 0.25f32..4.0, sy in 0.25f32..4.0, sz in 0.25f32..4.0, t in proptest::array::uniform3(-8.0f32..8.0), w in proptest::array::uniform3(-0.25f32..0.25)) {
        let (c, s) = (angle.cos(), angle.sin());
        for last in [[0.0, 0.0, 0.0], w] {
            let m: [f32; 16] = [c * sx, s * sx, 0.0, last[0], -s * sy, c * sy, 0.0, last[1], 0.0, 0.0, sz, last[2], t[0], t[1], t[2], 1.0];
            let inv = kikizake::model::opengl::tex::invert(&m).ok_or_else(|| TestCaseError::fail("singular"))?;
            // The normwise residual bound scales with the matrix condition number.
            let norm = |matrix: &[f32; 16]| (0..4).map(|r| (0..4)
                .map(|c| f64::from(matrix[4 * c + r]).abs()).sum::<f64>()).fold(0.0, f64::max);
            let scale = norm(&m) * norm(&inv);
            for r in 0..4 {
                for col in 0..4 {
                    let v: f64 = (0..4).map(|k| f64::from(inv[k * 4 + r]) * f64::from(m[col * 4 + k])).sum();
                    let want = if r == col { 1.0 } else { 0.0 };
                    prop_assert!((v - want).abs() <= 32.0 * f64::from(f32::EPSILON) * scale, "({r}, {col}) {v}, scale {scale}");
                }
            }
        }
    }

    // GL.TEX.GEN: a singular modelview (a zero scale) has no inverse, and the plane is kept
    #[test]
    fn singular_modelviews_have_no_inverse(k in 0usize..3, s in -4.0f32..4.0) {
        let mut m = [1.0f32, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
        m[k * 5] = 0.0;
        m[12] = s;
        prop_assert!(kikizake::model::opengl::tex::invert(&m).is_none());
    }

    // GL.RASTER.TEXTURE.PRIM: an antialiased line's rows each count their pixels from 0 and its
    // colors are the first end's plus the count times the step, so texture coordinates stepped
    // by the same count follow its colors exactly
    #[test]
    fn aa_line_rows_count_as_their_colors_step(x0 in 1.0f32..30.0, y0 in 1.0f32..30.0, x1 in 1.0f32..30.0, y1 in 1.0f32..30.0, w in 1.0f32..4.0) {
        let (a, b) = (tri::Raster([x0, y0]), tri::Raster([x1, y1]));
        let c = [[0.0, 10.0, 20.0, 1.0], [100.0, 50.0, 0.0, 1.0]];
        let length = (x1 - x0).abs().max((y1 - y0).abs());
        prop_assume!(length > 0.5);
        let dc: Vec<f32> = (0..3).map(|k| (c[1][k] - c[0][k]) * (1.0 / length)).collect();
        for p in tri::aa_line(a, b, c, tri::AaWidth::of(w), None) {
            prop_assert!(p.k >= 0);
            let mut col: [f32; 3] = [c[0][0], c[0][1], c[0][2]];
            for _ in 0..p.k {
                for k in 0..3 {
                    col[k] += dc[k];
                }
            }
            for k in 0..3 {
                prop_assert_eq!(p.c[k].to_bits(), col[k].to_bits());
            }
        }
    }

    // GL.ARRAY: an integer color or normal component maps to [0, 1] unsigned, [-1, 1] signed,
    // the extremes onto the ends, in order (GL 1.1 table 2.6)
    #[test]
    fn array_components_map_in_order(a in any::<i16>(), b in any::<i16>()) {
        use kikizake::model::opengl::array::normalized;
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert!(normalized(SHORT, f64::from(lo)) <= normalized(SHORT, f64::from(hi)));
        prop_assert!(normalized(0x1403, f64::from(lo.unsigned_abs())) <= 1.0);
        prop_assert_eq!(normalized(SHORT, -32768.0), -1.0);
        prop_assert_eq!(normalized(SHORT, 32767.0), 1.0);
        prop_assert_eq!(normalized(UNSIGNED_BYTE, 255.0), 1.0);
        prop_assert_eq!(normalized(UNSIGNED_BYTE, 0.0), 0.0);
        prop_assert_eq!(normalized(BYTE, -128.0), -1.0);
    }

    // GL.ARRAY: a color array's integer component in order, within a few float32 ulps of
    // GL 1.1 table 2.6 (the rounded reciprocals: 1 + 4 ulps at most past it), and a normal's
    // INT component the color's
    #[test]
    fn array_colors_stay_in_order_near_the_table(a in any::<i32>(), b in any::<i32>(), u in any::<u32>()) {
        use kikizake::model::opengl::array::{color_component, normal_component};
        const INT: u32 = 0x1404;
        const UNSIGNED_INT: u32 = 0x1405;
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert!(color_component(INT, f64::from(lo)) <= color_component(INT, f64::from(hi)));
        prop_assert_eq!(normal_component(INT, f64::from(a)).to_bits(), color_component(INT, f64::from(a)).to_bits());
        let exact = f64::from(u) / 4_294_967_295.0;
        let got = f64::from(color_component(UNSIGNED_INT, f64::from(u)));
        prop_assert!((got - exact).abs() <= exact * 1e-6 + 1e-9, "{} {}", got, exact);
        let s = (a & 0xFFFF) as i16;
        let exact = (2.0 * f64::from(s) + 1.0) / 65535.0;
        let got = f64::from(color_component(SHORT, f64::from(s)));
        prop_assert!((got - exact).abs() <= exact.abs() * 1e-6 + 1e-9, "{} {}", got, exact);
    }

    // GL.ARRAY: a glp line's values read back as their type from the executor's layout
    #[test]
    fn glp_values_read_back(v in proptest::collection::vec(any::<i16>(), 1..64)) {
        use kikizake::model::opengl::array::{glp_bytes, read};
        let mut t: Vec<String> = ["glp", "glVertexPointer", "iii", "2", "0x1402", "0", "s"].iter().map(|x| x.to_string()).collect();
        t.extend(v.iter().map(|x| x.to_string()));
        let bytes = glp_bytes(&t).map_err(|_| TestCaseError::fail("bytes"))?.ok_or_else(|| TestCaseError::fail("none"))?;
        prop_assert_eq!(bytes.len(), v.len() * 2);
        for (k, x) in v.iter().enumerate() {
            let got = read(SHORT, &bytes[k * 2..k * 2 + 2]).map_err(|_| TestCaseError::fail("read"))?;
            prop_assert_eq!(got, f64::from(*x));
        }
    }
}


/// one OpenGL case's lines in an ortho 0..16 window, after `body`
fn gl_lines(body: &str) -> Vec<String> {
    let text = format!("case p\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n{body}\nend\n");
    let c = kikizake::case::parse_cases(&text);
    c.first().map_or_else(Vec::new, |c| kikizake::model::run_case(c).unwrap_or_else(|e| vec![format!("{e:?}")]))
}

proptest! {
    // GL.VARIANTS: a short or int color, by value or by pointer, is the float color its values
    // map to as the color arrays map them, alpha 1
    #[test]
    fn color_forms_act_as_their_float_form(s in proptest::array::uniform3(any::<i16>()), i in proptest::array::uniform3(any::<i32>())) {
        use kikizake::model::opengl::array::color_component;
        const INT: u32 = 0x1404;
        let get = "glget f 0xb00 4\nglget i 0xb00 4";
        let float = |ty: u32, v: [f64; 3]| {
            let [r, g, b] = v.map(|x| color_component(ty, x));
            gl_lines(&format!("gl glColor4f {r} {g} {b} 1\n{get}"))
        };
        let [s0, s1, s2] = s;
        let want = float(SHORT, s.map(f64::from));
        prop_assert_eq!(&gl_lines(&format!("gl glColor3s {s0} {s1} {s2}\n{get}")), &want);
        prop_assert_eq!(&gl_lines(&format!("glp glColor3sv - s {s0} {s1} {s2}\n{get}")), &want);
        let [i0, i1, i2] = i;
        let want = float(INT, i.map(f64::from));
        prop_assert_eq!(&gl_lines(&format!("gl glColor3i {i0} {i1} {i2}\n{get}")), &want);
        prop_assert_eq!(&gl_lines(&format!("glp glColor3iv - i {i0} {i1} {i2}\n{get}")), &want);
    }

    // GL.VARIANTS, GL.RASTER.POS: every form of a raster position reads back within the
    // rasterizer's 1/256 steps of where it was given, valid, at the middle depth, w 1
    #[test]
    fn raster_positions_read_back_near_where_given(x in 0i32..16, y in 0i32..16) {
        let get = "glget f 0xb07 4\nglget i 0xb08 1";
        let want = gl_lines(&format!("gl glRasterPos2f {x} {y}\n{get}"));
        prop_assert_eq!(&gl_lines(&format!("gl glRasterPos2s {x} {y}\n{get}")), &want);
        prop_assert_eq!(&gl_lines(&format!("gl glRasterPos3d {x} {y} 0\n{get}")), &want);
        prop_assert_eq!(&gl_lines(&format!("glp glRasterPos2iv - i {x} {y}\n{get}")), &want);
        let line = want.iter().find(|l| l.starts_with("get b07")).ok_or_else(|| TestCaseError::fail("no b07"))?;
        let v: Vec<f32> = line.split_whitespace().skip(3).filter_map(|h| u32::from_str_radix(h, 16).ok().map(f32::from_bits)).collect();
        prop_assert_eq!(v.len(), 4);
        let near = |got: f32, want: i32| (f64::from(got) - f64::from(want)).abs() <= 1.0 / 128.0;
        prop_assert!(v.first().is_some_and(|g| near(*g, x)) && v.get(1).is_some_and(|g| near(*g, y)), "{:?}", v);
        prop_assert_eq!(v.get(2..), Some(&[0.5f32, 1.0][..]));
        prop_assert!(want.iter().any(|l| l == "get b08 = 1"), "{:?}", want);
    }
}

proptest! {
    // GL.POLYGON.SMOOTH, GL 1.1 2.6.1: a smooth triangle with a corner behind the eye is culled
    // as front or as back facing, never both; culling the other face draws it as with no culling
    #[test]
    fn smooth_triangles_behind_the_eye_cull_one_face(x in -4i32..=4, y in -4i32..=4, z in 1i32..=4, cw in any::<bool>()) {
        let tri = |cull: &str| {
            let face = if cw { "gl glFrontFace 0x900\n" } else { "" };
            let text = format!("case p\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glFrustum -1 1 -1 1 1 10\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\ngl glClear 0x4000\ngl glEnable 0xb41\n{face}{cull}gl glBegin 4\ngl glColor4ub 255 0 0 255\ngl glVertex3f -1 -1 -2\ngl glColor4ub 0 255 0 255\ngl glVertex3f 1 -0.5 -2\ngl glColor4ub 0 0 255 255\ngl glVertex3f {} {} {z}\ngl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nend\n", f64::from(x) / 4.0, f64::from(y) / 4.0);
            let c = kikizake::case::parse_cases(&text);
            c.first().map(|c| kikizake::model::run_case(c).map_err(|e| format!("{e:?}")))
        };
        let none = tri("").ok_or_else(|| TestCaseError::fail("no case"))?.map_err(TestCaseError::fail)?;
        let back = tri("gl glEnable 0xb44\ngl glCullFace 0x405\n").ok_or_else(|| TestCaseError::fail("no case"))?.map_err(TestCaseError::fail)?;
        let front = tri("gl glEnable 0xb44\ngl glCullFace 0x404\n").ok_or_else(|| TestCaseError::fail("no case"))?.map_err(TestCaseError::fail)?;
        let clear = tri("gl glEnable 0xb44\ngl glCullFace 0x408\n").ok_or_else(|| TestCaseError::fail("no case"))?.map_err(TestCaseError::fail)?;
        prop_assert!(back == none || front == none, "drawn neither way");
        prop_assert!(back == clear || front == clear, "culled neither way");
    }
}

/// color_plane_is_barycentric's law for one triangle (corners in 1/2048 pixels, colors in bytes)
fn color_plane_holds(v: [(i16, i16); 3], c: [[u8; 4]; 3]) -> Result<(), TestCaseError> {
    let v = v.map(|(x, y)| Vertex { x: Sub(i64::from(x) / 8), y: Sub(i64::from(y) / 8) });
    let c = c.map(|q| q.map(|b| f64::from(b) / 255.0));
    let [p0, p1, p2] = v.map(|p| (p.x.0 as f64, p.y.0 as f64));
    let area = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
    prop_assume!(area != 0.0);
    // (each corner's height over the edge across from it, in 1/256 pixels; no slivers: every
    // height at least a pixel)
    let len = |a: (f64, f64), b: (f64, f64)| (b.0 - a.0).hypot(b.1 - a.1);
    let heights = [area.abs() / len(p1, p2), area.abs() / len(p2, p0), area.abs() / len(p0, p1)];
    prop_assume!(heights.iter().all(|h| *h >= 256.0));
    // (the rasterizer walks each edge in fixed point: a covered pixel's center lies at most one
    // 1/256 step outside the triangle, each corner's weight at least -1 / its height, so a color
    // at most its spread over the least height past the corners')
    let slack = heights.iter().map(|h| 1.0 / h).fold(0.0, f64::max);
    let near = |a: [f64; 4], b: [f64; 4]| zip_eq(a.iter(), b).all(|(x, y)| (x - y).abs() < 1e-6);
    for (x, y) in tri::pixels(&v, 16, 16) {
        let (cx, cy) = ((x * 256 + 128) as f64, (y * 256 + 128) as f64);
        let l1 = ((cx - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (cy - p0.1)) / area;
        let l2 = ((p1.0 - p0.0) * (cy - p0.1) - (cx - p0.0) * (p1.1 - p0.1)) / area;
        for (l, h) in zip_eq([1.0 - l1 - l2, l1, l2].iter(), heights) {
            prop_assert!(*l >= -1.0 / h - 1e-12, "pixel {:?} {} past an edge", (x, y), -l * h);
        }
        let at = tri::plane(v, c, x, y);
        for p in [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]] {
            prop_assert!(near(at, tri::plane(p.map(|k| v[k]), p.map(|k| c[k]), x, y)));
        }
        for k in 0..4 {
            let (lo, hi) = (c.iter().map(|q| q[k]).fold(f64::INFINITY, f64::min), c.iter().map(|q| q[k]).fold(f64::NEG_INFINITY, f64::max));
            let past = (hi - lo) * slack + 1e-9;
            prop_assert!(at[k] >= lo - past && at[k] <= hi + past, "{at:?} out of {c:?}");
        }
    }
    for k in 0..3 {
        // a corner on a pixel center gets its own color
        let w = v.map(|p| Vertex { x: Sub(p.x.0 * 256 + 128), y: Sub(p.y.0 * 256 + 128) });
        prop_assert!(near(tri::plane(w, c, v[k].x.0, v[k].y.0), c[k]));
    }
    Ok(())
}

/// color_plane_is_barycentric's failing input: pixel (2, 12)'s center 0.026 of a 1/256 step past
/// the edge from corner 0 to 2, covered by the rasterizer's rounding, green 0.2156909 over the
/// corners' 0.2156863
#[test]
fn color_plane_allows_the_edge_rounding() {
    let r = color_plane_holds([(-15288, -30920), (5968, 0), (7616, 32512)], [[0, 55, 0, 0], [0, 0, 0, 0], [0, 55, 0, 0]]);
    assert!(r.is_ok(), "{r:?}");
}

/// GL.ATTRIB.POP: glPopAttrib gives back the groups pushed and leaves the others as changed:
/// the current texture coordinates, raster position and index (CURRENT_BIT), the line stipple
/// and width (LINE_BIT), the list base (LIST_BIT), the clear index and index mask
/// (COLOR_BUFFER_BIT), the pixel zoom, index shift and depth scale (PIXEL_MODE_BIT), every mask
/// of the five, random values (the review of 84b05c8, 3.3)
mod attrib_pop {
    use super::*;
    /// (group bit, the state set from value v, what glGet reads of it)
    fn groups(v: u32) -> [(u32, String, &'static str); 5] {
        let f = |k: u32| f64::from(v.wrapping_add(k) % 61) / 8.0;
        [
            (0x1, format!("gl glTexCoord4f {} {} {} {}\ngl glRasterPos2f {} {}\ngl glIndexf {}", f(1), f(2), f(3), f(4), f(5), f(6), f(7)), "glget f 0xb03 4\nglget f 0xb07 4\nglget f 0xb06 4\nglget f 0xb01 1"),
            (0x4, format!("gl glLineStipple {} {:#x}\ngl glLineWidth {}", v % 255 + 1, v % 65536, f(8) + 1.0), "glget i 0xb25 1\nglget i 0xb26 1\nglget f 0xb21 1"),
            (0x20000, format!("gl glListBase {}", v % 1000), "glget i 0xb32 1"),
            (0x4000, format!("gl glClearIndex {}\ngl glIndexMask {}", f(9), v % 256), "glget f 0xc20 1\nglget i 0xc21 1"),
            (0x20, format!("gl glPixelZoom {} {}\ngl glPixelTransferi 0xd12 {}\ngl glPixelTransferf 0xd1e {}", f(10), f(11), v % 7, f(12)), "glget f 0xd16 1\nglget f 0xd17 1\nglget i 0xd12 1\nglget f 0xd1e 1"),
        ]
    }
    fn reads(set: &str) -> Vec<String> {
        let gets: Vec<&str> = groups(0).iter().map(|g| g.2).collect();
        let text = format!("case p\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n{set}\n{}\nglerr\nend\n", gets.join("\n"));
        let c = kikizake::case::parse_cases(&text);
        c.first().map_or_else(Vec::new, |c| kikizake::model::run_case(c).unwrap_or_else(|e| vec![format!("{e:?}")]))
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn popped_groups_come_back(mask in 0u32..32, a: u32, b: u32) {
            prop_assume!(a % 61 != b % 61 && a % 1000 != b % 1000);
            let (ga, gb) = (groups(a), groups(b));
            let bits: u32 = ga.iter().enumerate().filter(|(i, _)| mask >> i & 1 == 1).map(|(_, g)| g.0).sum();
            let set = |g: &[(u32, String, &str); 5]| g.iter().map(|x| x.1.as_str()).collect::<Vec<_>>().join("\n");
            let popped = reads(&format!("{}\ngl glPushAttrib {bits:#x}\n{}\ngl glPopAttrib", set(&ga), set(&gb)));
            // (what each group reads when it holds a's values, or b's)
            let want: String = (0..5).map(|i| if mask >> i & 1 == 1 { ga[i].1.clone() } else { gb[i].1.clone() }).collect::<Vec<_>>().join("\n");
            prop_assert_eq!(popped, reads(&want), "mask {:#x}", bits);
        }
    }
}

/// glDeleteLists of a range however wide (the review of 84b05c8, 5: lists past 2^20 names of
/// the first stayed) and a proxy glTexImage, which runs at once and is not kept in a list
/// (GL 1.1 3.8.3; the review's 3.5): the model's answers against the rules
mod lists_far {
    use super::*;
    fn run(body: &str) -> Vec<String> {
        let text = format!("case p\nglctx 32 choose 0x38 0 32 0 0 32 8\n{body}\nglerr\nend\n");
        let c = kikizake::case::parse_cases(&text);
        c.first().map_or_else(Vec::new, |c| kikizake::model::run_case(c).unwrap_or_else(|e| vec![format!("{e:?}")]))
    }
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn deleted_lists_are_their_range(first in 1u32.., range in prop_oneof![0i64..4, 1_048_570i64..1_048_600, 0i64..=0x7fff_ffff], offs in proptest::collection::vec(prop_oneof![0u32..8, 1_048_570u32..1_048_600, any::<u32>()], 1..6)) {
            // (lists named first + off, list 0 not being one)
            let ids: Vec<u32> = offs.iter().map(|&o| first.wrapping_add(o)).filter(|&k| k != 0).collect();
            prop_assume!(!ids.is_empty());
            let make: String = ids.iter().map(|k| format!("gl glNewList {k} 0x1300\ngl glColor4ub 1 2 3 4\ngl glEndList\n")).collect();
            let ask: String = ids.iter().map(|k| format!("glr glIsList b i {k}\n")).collect();
            let got = run(&format!("{make}gl glDeleteLists {first} {range}\n{ask}"));
            let want: Vec<String> = std::iter::once("glctx ok 1".to_string())
                .chain(ids.iter().map(|&k| format!("ret {}", u8::from(u64::from(k.wrapping_sub(first)) >= u64::try_from(range).unwrap_or(0)))))
                .chain(std::iter::once("glerr 0".to_string()))
                .collect();
            prop_assert_eq!(got, want);
        }

        #[test]
        fn proxy_images_run_outside_lists(a in 0u32..4, b in 0u32..4, execute: bool) {
            // (PROXY_TEXTURE_2D: the executor's gltex of any target but TEXTURE_1D is glTexImage2D)
            let target = "0x8064";
            let image = |n: u32| {
                let side = 1u32 << n;
                format!("gltex {target} 0 4 {side} {side} 0 0x1908 0x1401 21 {}", side * side * 4)
            };
            let q = format!("gltexq level {target} 0 0x1000 i 1");
            let mode = if execute { "0x1301" } else { "0x1300" };
            let got = run(&format!("gl glNewList 1 {mode}\n{}\n{q}\ngl glEndList\n{q}\n{}\n{q}\ngl glCallList 1\n{q}", image(a), image(b)));
            // (the proxy holds the size of the last image given it, a list's call giving none)
            let (sa, sb) = (1u32 << a, 1u32 << b);
            let want = vec!["glctx ok 1".to_string(), format!("texq {sa}"), format!("texq {sa}"), format!("texq {sb}"), format!("texq {sb}"), "glerr 0".to_string()];
            prop_assert_eq!(got, want);
        }
    }
}
#[test]
fn stipple_display_lists_capture_unpack_and_attribute_stack_restores_pattern() {
    let text = "case state
glctx 32 choose 0x38 0 32 0 0 32 8
gl glPixelStorei 0xcf1 1
gl glNewList 17 0x1300
glp glPolygonStipple - ub 1
gl glEndList
gl glPixelStorei 0xcf1 0
glq glGetPolygonStipple - ub 2
gl glCallList 17
glq glGetPolygonStipple - ub 2
gl glPushAttrib 0x10
glp glPolygonStipple - ub 127
glq glGetPolygonStipple - ub 2
gl glPopAttrib
glq glGetPolygonStipple - ub 2
glerr
end";
    let output = kikizake::model::run_case(&kikizake::case::parse_cases(text)[0]).unwrap();
    assert_eq!(&output[1..], ["q 255 255", "q 128 0", "q 127 0", "q 128 0", "glerr 0"]);
}

proptest! {
    // glq starts with 0xcc, so partial destination bytes test preservation too.
    #[test]
    fn polygon_stipple_pack_unpack(words in prop::collection::vec(any::<u32>(), 64),
        layouts in prop::collection::vec((any::<bool>(), 0u32..4, 0usize..49, 0usize..4, 0usize..12), 2)) {
        use std::fmt::Write;
        let mut text = String::from("case law\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
        let mut src = words.iter().flat_map(|w| w.to_le_bytes()).collect::<Vec<_>>();
        src.resize(4096, 0);
        for (k, &(lsb, shift, row, skip_y, skip_x)) in layouts.iter().enumerate() {
            let base = if k == 0 { 0xcf0 } else { 0xd00 };
            for (i, v) in [usize::from(lsb), row, skip_y, skip_x, 1 << shift].into_iter().enumerate() {
                writeln!(text, "gl glPixelStorei {} {v}", base + i + 1).unwrap();
            }
            if k == 0 {
                text.push_str("glp glPolygonStipple - ui");
                for word in &words { write!(text, " {word}").unwrap(); }
                text.push('\n');
            }
        }
        text.push_str("glq glGetPolygonStipple - ub 400\nend\n");
        let address = |k: usize, x: usize, y: usize| {
            let (lsb, shift, row, skip_y, skip_x) = layouts[k];
            let align = 1usize << shift;
            let bytes = (if row == 0 { 32 } else { row }).div_ceil(8);
            let stride = bytes.next_multiple_of(align);
            let pos = skip_x + x;
            ((skip_y + y) * stride + pos / 8, 1u8 << if lsb { pos % 8 } else { 7 - pos % 8 })
        };
        let mut expected = [0xcc; 400];
        for y in 0..32 {
            for x in 0..32 {
                let (from, source_mask) = address(0, x, y);
                let (to, mask) = address(1, x, y);
                if src[from] & source_mask != 0 { expected[to] |= mask; } else { expected[to] &= !mask; }
            }
        }
        let case = kikizake::case::parse_cases(&text);
        let actual = kikizake::model::run_case(&case[0]).unwrap();
        let query = format!("q {}", expected.map(|b| b.to_string()).join(" "));
        prop_assert_eq!(actual.last(), Some(&query));
    }
}
proptest! {
    #[test]
    fn pixel_map_queries_preserve_entries_and_query_tail(
        entries in prop::collection::vec(any::<u16>(), 8),
        slot in 0xc70u32..=0xc79,
    ) {
        let input = entries.iter().map(u16::to_string).collect::<Vec<_>>().join(" ");
        let text = format!("case map\nglctx 32 choose 0x38 0 32 0 0 32 8\n\
            glp glPixelMapusv ii {slot} 8 us {input}\n\
            glget i {} 1\nglq glGetPixelMapusv i {slot} us 9\nglerr\nend", slot + 0x40);
        let actual = kikizake::model::run_case(&kikizake::case::parse_cases(&text)[0]).unwrap();
        let size = format!("get {:x} = 8", slot + 0x40);
        let query = format!("q {input} 52428");
        prop_assert_eq!(&actual[1..], &[size, query, "glerr 0".into()]);
    }
}

#[test]
fn pixel_map_native_integer_scales_and_zero_reset() {
    let text = "case map
glctx 32 choose 0x38 0 32 0 0 32 8
glp glPixelMapfv ii 0xc70 4 f -1.5 -0.5 0.5 1.5
glq glGetPixelMapfv i 0xc70 f 4
glp glPixelMapuiv ii 0xc76 4 ui 1 2147483647 2147483648 4294967295
glq glGetPixelMapuiv i 0xc76 ui 5
glq glGetPixelMapusv i 0xc76 us 5
glp glPixelMapfv ii 0xc76 0 f 0.75
glget i 0xcb6 1
glq glGetPixelMapfv i 0xc76 f 3
glerr
end";
    let output = kikizake::model::run_case(&kikizake::case::parse_cases(text)[0]).unwrap();
    assert_eq!(&output[1..], ["q -2 -1 1 2", "q 1 2147483520 2147483520 4294967040 2125315823",
        "q 0 32768 32768 65535 52428", "get cb6 = 1", "q 0 -12345 -12345", "glerr 0"]);
}
#[test]
fn index_and_stencil_pixel_maps_match_windows() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_pixelmap.txt"));
    // RGBA bytes or stencil indices, with mapping off/on and index shift 0/1.
    let expected = [
        ["7f7fffff7f7fffff3fff00ff3fff00ffff0000ffff0000ffbf3f7fffff0000ff",
         "bf3f7fffbf3f7fff3fff00ff3fff00ffbf3f7fffbf3f7fff3fff00ffbf3f7fff",
         "808000ff808000ff400000ff400000ffffff00ffffff00ffbfffffffffff00ff",
         "bfffffffbfffffff400000ff400000ffbfffffffbfffffff400000ffbfffffff"],
        ["6941b0f08f7f122f", "a404c0c03cfc48bc", "0101000003030203", "0303020203030203"],
    ];
    for (case, reads) in zip_eq(&cases, expected) {
        let output = kikizake::model::run_case(case).unwrap();
        let mut want = vec!["glctx ok 1".to_string()];
        for bytes in reads {
            want.extend([format!("read {bytes}"), "glerr 0".to_string()]);
        }
        assert_eq!(output, want, "{}", case.name);
    }
}

fn bitmap_row(byte: u8, lsb: bool, skip: u8, width: u8) -> (String, String) {
    let text = format!("case bitmap\nglctx 32 choose 0x38 0 32 0 0 32 8\n\
        gl glMatrixMode 0x1701\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n\
        gl glDisable 0xbd0\ngl glClear 0x4000\ngl glColor3ub 255 0 0\n\
        gl glRasterPos2f 2.25 2.25\ngl glColor3ub 0 0 255\n\
        gl glPixelStorei 0xcf1 {}\ngl glPixelStorei 0xcf4 {skip}\n\
        glp glBitmap iiffff {width} 1 0 0 1 2 ub {byte}\n\
        glread 2 2 8 1 0x1908 0x1401 32\nglerr\nend", u8::from(lsb));
    let output = kikizake::model::run_case(&kikizake::case::parse_cases(&text)[0]).unwrap();
    assert_eq!(output.last().unwrap(), "glerr 0");
    let mut expected = String::from("read ");
    for x in 0..8 {
        let bit = x + skip;
        let set = x < width && bit < 8 && byte & (1 << if lsb { bit } else { 7 - bit }) != 0;
        expected.push_str(if set { "ff0000ff" } else { "000000ff" });
    }
    (output[1].clone(), expected)
}

#[test]
fn bitmap_every_byte_in_both_orders() {
    for byte in u8::MIN..=u8::MAX {
        for lsb in [false, true] {
            let (pixels, expected) = bitmap_row(byte, lsb, 0, 8);
            assert_eq!(pixels, expected, "byte {byte}, lsb {lsb}");
        }
    }
}

proptest! {
    #[test]
    fn bitmap_unpack_skip_and_width(byte in any::<u8>(), lsb in any::<bool>(),
        skip in 0u8..8, width in 0u8..=8) {
        let (pixels, expected) = bitmap_row(byte, lsb, skip, width);
        prop_assert_eq!(pixels, expected);
    }
}

#[test]
fn bitmap_fractional_moves_match_windows() {
    let case = kikizake::case::parse_cases(include_str!("fixtures/opengl_bitmap_moves.txt"));
    let output = kikizake::model::run_case(&case[0]).unwrap();
    let mut expected = vec![String::from("glctx ok 1")];
    for xy in [
        "40004000 40000000",
        "40004000 40000000",
        "40004000 40000000",
        "40008000 3fff8000",
        "4000c000 3fff0000",
        "40008000 3fff8000",
        "40088000 3fef8000",
        "42fb2e00 c2f32c00",
        "40088000 3fef8000",
    ] {
        expected.push(format!("get b07 = {xy} 3f000000 3f800000"));
    }
    expected.push(String::from("glerr 0"));
    assert_eq!(output, expected);
}

#[test]
fn copy_pixels_matches_initialized_windows_buffers() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_copypixels.txt"));
    let reads: Vec<_> = include_str!("fixtures/opengl_copypixels_reads.txt").lines().collect();
    for (case, read) in zip_eq(&cases, reads) {
        let output = kikizake::model::run_case(case).unwrap();
        assert_eq!(output.len(), 3, "{}", case.name);
        assert_eq!(output[0], "glctx ok 1");
        assert_eq!(output[2], "glerr 0");
        let bytes = |line: &str| {
            let hex = line.strip_prefix("read ").unwrap();
            (0..hex.len()).step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect::<Vec<_>>()
        };
        let (actual, expected) = (bytes(&output[1]), bytes(read));
        assert_eq!(actual.len(), expected.len());
        for (i, (got, want)) in zip_eq(&actual, &expected).enumerate() {
            assert_eq!(got, want, "{} at byte {i}", case.name);
        }
    }
}

fn copy_bitmap_row(pattern: u8, src: usize, dst: usize, width: usize) -> (String, String) {
    let text = format!("case overlap\nglctx 32 choose 0x38 0 32 0 0 32 8\n\
        gl glMatrixMode 0x1701\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n\
        gl glDisable 0xbd0\ngl glClear 0x4000\ngl glRasterPos2i 0 0\n\
        glp glBitmap iiffff 8 1 0 0 0 0 ub {pattern}\ngl glRasterPos2i {dst} 0\n\
        gl glCopyPixels {src} 0 {width} 1 0x1800\nglread 0 0 8 1 0x1908 0x1401 32\nglerr\nend");
    let result = kikizake::model::run_case(&kikizake::case::parse_cases(&text)[0]).unwrap();
    assert_eq!(result.last().unwrap(), "glerr 0");
    let mut expected = String::from("read ");
    for x in 0..8 {
        let from = if (dst..dst + width).contains(&x) { src + x - dst } else { x };
        expected.push_str(if from < 8 && pattern & (128 >> from) != 0 { "ffffffff" } else { "000000ff" });
    }
    (result[1].clone(), expected)
}

#[test]
fn copy_pixels_every_overlapping_byte() {
    for pattern in u8::MIN..=u8::MAX {
        for (src, dst) in [(0, 1), (1, 0)] {
            let (actual, expected) = copy_bitmap_row(pattern, src, dst, 7);
            assert_eq!(actual, expected);
        }
    }
}

proptest! {
    #[test]
    fn copy_pixels_region_uses_original_source(pattern in any::<u8>(), src in 0usize..8,
        dst in 0usize..8, width in 0usize..=8) {
        let (actual, expected) = copy_bitmap_row(pattern, src, dst, width);
        prop_assert_eq!(actual, expected);
    }
}

#[test]
fn clip_plane_equations_match_windows() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_clip_state.txt"));
    assert_eq!(cases.len(), 18);
    for case in cases {
        let result = kikizake::model::run_case(&case).unwrap();
        let equation = if case.name.ends_with("-scaled") {
            "q 0.5 2 -2 3 -12345"
        } else {
            "q 1 8 -16 3 -12345"
        };
        assert_eq!(&result[1..], ["q 0 0 0 0 -12345", equation, "glerr 0"], "{}", case.name);
    }
}

proptest! {
    #[test]
    fn filled_rectangle_obeys_enabled_clip_planes(slot in 0u32..6, xcut in 0i32..=8, ycut in 0i32..=8, mask in 0u8..4) {
        let xplane = 0x3000 + slot;
        let yplane = 0x3000 + (slot + 1) % 6;
        let enable_x = if mask & 1 != 0 { format!("gl glEnable {xplane}\n") } else { String::new() };
        let enable_y = if mask & 2 != 0 { format!("gl glEnable {yplane}\n") } else { String::new() };
        let text = format!("case clip\nglctx 32 choose 0x38 0 32 0 0 32 8\n\
            gl glMatrixMode 0x1701\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\n\
            glp glClipPlane i {xplane} d 1 0 0 {}\nglp glClipPlane i {yplane} d 0 1 0 {}\n\
            {enable_x}{enable_y}gl glClear 0x4000\ngl glRectf 0 0 8 8\n\
            glread 0 0 8 8 0x1908 0x1401 256\nglerr\nend\n", -xcut, -ycut);
        let cases = kikizake::case::parse_cases(&text);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        let mut read = String::from("read ");
        for y in 0..8 {
            for x in 0..8 {
                let inside = (mask & 1 == 0 || x >= xcut) && (mask & 2 == 0 || y >= ycut);
                read.push_str(if inside { "ffffffff" } else { "000000ff" });
            }
        }
        prop_assert_eq!(&output[1..], [read.as_str(), "glerr 0"]);
    }
}

#[test]
fn point_and_raster_position_include_the_clip_boundary() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_clip_point.txt"));
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    assert_eq!(&output[1..], ["get b08 = 0", "get b08 = 1", "get b08 = 1", "read 000000ffffffffffffffffff", "glerr 0"]);
}

#[test]
fn crossing_lines_match_windows_in_both_directions() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_clip_line.txt"));
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    let pixels = concat!("read ", "000000ff000000ffbf003fff9f005fff7f007fff5f009fff000000ff000000ff000000ff",
        "000000ff000000ff000000ff9f005fff7f007fff5f009fff3f00bfff000000ff000000ff",
        "000000ff000000ff000000ff000000ff000000ff000000ff000000ff000000ff000000ff");
    assert_eq!(&output[1..], [pixels, "glerr 0"]);
}

#[test]
fn clipped_polygon_draws_new_corners_and_edge() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_clip_outline.txt"));
    let reads = ["read ffffffff000000ff000000ff000000ffffffffff000000ff", "read ffffffffffffffffffffffffffffffffffffffff000000ff"];
    for (case, read) in zip_eq(&cases, reads) {
        let output = kikizake::model::run_case(case).unwrap();
        assert_eq!(&output[1..], [read, "glerr 0"], "{}", case.name);
    }
}

/// All eight triangle flags, with one corner inside or outside the left plane,
/// in point, thin line, wide line and antialiased line modes on Windows.
#[test]
fn every_outline_edge_mask_matches_windows() {
    use std::fmt::Write;
    let mut golden = include_str!("fixtures/opengl_edge_masks.txt").lines();
    for textured in [false, true] {
        for first in [-4, 2] {
            for (mode, extra) in [(0x1b00, ""), (0x1b01, ""), (0x1b01, "gl glLineWidth 3\n"), (0x1b01, "gl glEnable 0xb20\n")] {
                for mask in 0..8 {
                    let setup = if textured { "gltex 3553 0 4 4 4 0 0x1908 0x1401 19 64\ngl glTexParameteri 3553 10241 9728\ngl glTexEnvi 8960 8704 8448\ngl glEnable 3553\n" } else { "" };
                    let head = format!("case edge\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\ngl glDisable 0xbd0\n{setup}gl glClear 0x4000\ngl glPolygonMode 0x408 {mode}\n{extra}");
                    let mut input = format!("{head}gl glBegin 4\n");
                    for (i, (x, y)) in [(first, 2), (13, 4), (6, 13)].into_iter().enumerate() {
                        if textured { writeln!(input, "gl glTexCoord2f {}", ["0 0", "1 0", "0.5 1"][i]).unwrap(); }
                        writeln!(input, "gl glEdgeFlag {}\ngl glVertex2f {x} {y}", (mask >> i) & 1).unwrap();
                    }
                    input.push_str("gl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr\nend\n");
                    let cases = kikizake::case::parse_cases(&input);
                    let output = kikizake::model::run_case(&cases[0]).unwrap();
                    let mut read = String::from("read ");
                    for run in golden.next().unwrap().split_whitespace() {
                        let (count, rgba) = run.split_once('*').unwrap();
                        read.push_str(&rgba.repeat(count.parse().unwrap()));
                    }
                    assert_eq!(&output[1..], [read.as_str(), "glerr 0"], "{first} {mode} {extra} {mask}");
                    if !textured {
                        let flags = [mask & 1, (mask >> 1) & 1, (mask >> 2) & 1];
                        let [a, b, c] = flags;
                        let array = format!("{head}glp glVertexPointer iii 2 0x1406 0 f {first} 2 13 4 6 13\ngl glEnableClientState 0x8074\nglp glEdgeFlagPointer i 2 ub {a} 99 {b} 99 {c} 99\ngl glEnableClientState 0x8079\ngl glDrawArrays 4 0 3\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr\nend\n");
                        let cases = kikizake::case::parse_cases(&array);
                        assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), output);
                    }
                }
            }
        }
    }
    assert!(golden.next().is_none());
}

proptest! {
    #[test]
    fn edge_flags_select_only_their_polygon_points(x in 1u8..5, y in 1u8..5, mask in 0u8..8) {
        use std::fmt::Write;
        let points = [(x, y), (x + 6, y + 1), (x + 1, y + 6)];
        let mut input = String::from("case points\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\ngl glClear 0x4000\ngl glPolygonMode 0x408 0x1b00\ngl glBegin 4\n");
        for (i, (px, py)) in points.iter().enumerate() {
            writeln!(input, "gl glEdgeFlag {}\ngl glVertex2f {px}.5 {py}.5", (mask >> i) & 1).unwrap();
        }
        input.push_str("gl glEnd\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let result = kikizake::model::run_case(&cases[0]).unwrap();
        let mut read = String::from("read ");
        for py in 0..16 {
            for px in 0..16 {
                let drawn = points.iter().enumerate().any(|(i, p)| *p == (px, py) && mask & (1 << i) != 0);
                read.push_str(if drawn { "ffffffff" } else { "000000ff" });
            }
        }
        prop_assert_eq!(&result[1..], [read.as_str(), "glerr 0"]);
    }
}

// Windows RGBA reads from the centers of the 4x4 texture in the fixture.
const RASTER_TEXELS: [[&str; 4]; 4] = [
    ["525093ff", "aef7c5ff", "3f6b72ff", "ea29b9ff"],
    ["2f9661ff", "56155aff", "f7f618ff", "3d7135ff"],
    ["291a21ff", "8c7678ff", "829163ff", "082fc0ff"],
    ["ef751fff", "b89cb1ff", "538de5ff", "be64e9ff"],
];

fn check_raster_texel(x: usize, y: usize, s: f32, t: f32) {
    let input = include_str!("fixtures/opengl_raster_texel.txt").replace("{s}", &s.to_string()).replace("{t}", &t.to_string());
    let cases = kikizake::case::parse_cases(&input);
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    let pixel = format!("read {}", RASTER_TEXELS[y][x]);
    assert_eq!(&output[1..], [pixel.as_str(), pixel.as_str(), pixel.as_str(), "glerr 0"]);
}

#[test]
fn pixel_images_use_the_latched_raster_texel() {
    for y in 0u8..4 {
        for x in 0u8..4 {
            check_raster_texel(usize::from(x), usize::from(y), (f32::from(x) + 0.5) / 4.0, (f32::from(y) + 0.5) / 4.0);
        }
    }
}

proptest! {
    #[test]
    fn pixel_images_agree_inside_each_nearest_texel(x in 0u8..4, y in 0u8..4, ds in 1u8..64, dt in 1u8..64) {
        let coord = |cell, offset| (f32::from(cell) + f32::from(offset) / 64.0) / 4.0;
        check_raster_texel(usize::from(x), usize::from(y), coord(x, ds), coord(y, dt));
    }
}

#[test]
fn clipped_outlines_select_the_minification_filter() {
    let cases = kikizake::case::parse_cases(include_str!("fixtures/opengl_outline_min_filter.txt"));
    let minified = format!("read {}{}", "88914dff".repeat(6), "313e2cff".repeat(7));
    let magnified = "read 8a6e44ff857248ff746947ff625f46ff515645ff3f4c43ff474445ff5e3e48ff75374bff8c304effa32a51ff9e2c49ff962f3fff";
    for (case, expected) in zip_eq(&cases, [minified.as_str(), minified.as_str(), magnified]) {
        let output = kikizake::model::run_case(case).unwrap();
        assert_eq!(&output[1..], [expected, "glerr 0"], "{}", case.name);
    }
}

#[test]
fn clipped_outline_fog_matches_windows() {
    let mut expected = include_str!("fixtures/opengl_outline_fog_reads.txt").lines();
    for fog in ["0 0 0", "0.25 0.5 0.75"] {
        for mode in ["gl glPolygonMode 0x408 0x1b01", "gl glPolygonMode 0x408 0x1b00",
                     "gl glEnable 0xb20\ngl glPolygonMode 0x408 0x1b01"] {
            for [a, b, c] in [["0"; 3], ["0.5"; 3], ["0.25", "0.75", "-0.5"]] {
                let input = include_str!("fixtures/opengl_outline_fog.txt").replace("{fog}", fog)
                    .replace("{mode}", mode).replace("{a}", a).replace("{b}", b).replace("{c}", c);
                let cases = kikizake::case::parse_cases(&input);
                let output = kikizake::model::run_case(&cases[0]).unwrap();
                let mut read = String::from("read ");
                for run in expected.next().unwrap().split_whitespace() {
                    let (count, pixel) = run.split_once('*').unwrap();
                    read.push_str(&pixel.repeat(count.parse().unwrap()));
                }
                assert_eq!(&output[1..], [read.as_str(), "glerr 0"], "{fog} {mode} {a} {b} {c}");
            }
        }
    }
    assert!(expected.next().is_none());
}

#[test]
fn wgl_dib_swap_counts_and_pixel_format_results() {
    let mut input = String::from("case wgl-dib\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
    input.push_str("wglx getpf\nwglx choose 0x38 0 32 0 0 32 8\nwglx setpf 1\nwglx setpf 2\nwglx setpf 0\nwglx descpf 99\nwglx swap\n");
    let mut expected = ["glctx ok 1", "wglx getpf 1", "wglx choose 1", "wglx setpf 1 0",
        "wglx setpf 0 2000", "wglx setpf 0 87", "wglx descpf 0", "wglx swap 1 0"]
        .map(String::from).to_vec();
    for mask in [-1, 0, 1, 2, 3, 65536] {
        input.push_str(&format!("wglx swaplayer {mask}\n"));
        expected.push("wglx swaplayer 1 0".into());
    }
    for count in 0..=16 {
        input.push_str(&format!("wglx swapmultiple {count}\n"));
        expected.push(format!("wglx swapmultiple {} 0", (1u32 << count) - 1));
    }
    input.push_str("glerr\nend\n");
    expected.push("glerr 0".into());
    let cases = kikizake::case::parse_cases(&input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), expected);
}

#[test]
fn wgl_layer_planes_and_temporary_contexts() {
    let operations = [
        ("layer 1 0", "layer 0 0"), ("layer 0 0", "layer 0 87"),
        ("layer 1 -1", "layer 0 87"), ("layer 36 1", "layer 0 87"),
        ("getlayerpal -1 -1 0", "getlayerpal 0 87"),
        ("setlayerpal 1 0 256", "setlayerpal 0 87"), ("realize 1 1", "realize 0 87"),
        ("layerctx -1", "layerctx 0 5"), ("layerctx 0", "layerctx 1 0"),
        ("layerctx 1", "layerctx 0 87"), ("share", "share 1 0"),
        ("copy -1", "copy 1 0"), ("copy 0", "copy 1 0"),
    ];
    let mut input = String::from("case layers\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
    let mut expected = vec![String::from("glctx ok 1")];
    for (command, result) in operations {
        input.push_str(&format!("wglx {command}\n"));
        expected.push(format!("wglx {result}"));
    }
    input.push_str("wglx getpf\nglerr\nend\n");
    expected.extend(["wglx getpf 1", "glerr 0"].map(String::from));
    let cases = kikizake::case::parse_cases(&input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), expected);
}

#[test]
fn wgl_generic_extension_lookup_matches_windows() {
    let mut input = String::from("case extensions\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
    let mut expected = vec![String::from("glctx ok 1")];
    for (names, available) in [
        ("glAddSwapHintRectWIN glColorTableEXT glColorSubTableEXT glGetColorTableEXT glGetColorTableParameterivEXT glGetColorTableParameterfvEXT glDrawRangeElementsWIN", 1),
        ("glDrawRangeElementsEXT glBegin glcolortableEXT wglSwapIntervalEXT unknown", 0),
    ] {
        for name in names.split_whitespace() {
            for query in ["proc", "defproc"] {
                input.push_str(&format!("wglx {query} {name}\n"));
                expected.push(format!("wglx {query} {available}"));
            }
        }
    }
    input.push_str("glerr\nend\n");
    expected.push(String::from("glerr 0"));
    let cases = kikizake::case::parse_cases(&input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), expected);
}

#[test]
fn wgl_empty_font_ranges_and_invalid_outline_formats_preserve_lists() {
    for encoding in ["a", "w"] {
        let mut input = String::from("case fonts\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glNewList 100 0x1300\ngl glEndList\n");
        let mut expected = vec![String::from("glctx ok 1")];
        for base in [100, 101] {
            for (cmd, suffix, result) in [
                ("fontbmp", "0", "0 87"), ("fontoutline", "0", "1 0"),
                ("fontoutline", "1", "1 0"), ("fontoutline", "2", "0 8"),
            ] {
                let format = if cmd == "fontbmp" { String::new() } else { format!(" {suffix}") };
                input.push_str(&format!("wglx {cmd} {encoding} 65 0 {base}{format}\n"));
                expected.push(format!("wglx {cmd} {result}"));
            }
            input.push_str(&format!("wglx fontoutline {encoding} 65 1 {base} 2\nglr glIsList b i {base}\nglerr\n"));
            expected.extend(["wglx fontoutline 0 8", if base == 100 { "ret 1" } else { "ret 0" }, "glerr 0"].map(String::from));
        }
        input.push_str("end\n");
        let cases = kikizake::case::parse_cases(&input);
        assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), expected);
    }
}

#[test]
fn wgl_ansi_bitmap_range_failure_preserves_the_whole_list_range() {
    for (first, count) in [(255, 2), (256, 1), (257, 2), (12354, 1), (65535, 2)] {
        let input = format!("case ansi-range\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glNewList 101 0x1300\ngl glEndList\nwglx fontbmp a {first} {count} 100\nglr glIsList b i 100\nglr glIsList b i 101\nglerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(),
            ["glctx ok 1", "wglx fontbmp 0 87", "ret 0", "ret 1", "glerr 0"]);
    }
}

#[test]
fn wgl_bitmap_lists_draw_and_advance_like_windows() {
    let input = include_str!("../cases/opengl.txt").lines().skip_while(|line| *line != "case gap-wgl-1")
        .skip(1).take_while(|line| !line.starts_with("wglx fontbmp w")).collect::<Vec<_>>().join("\n");
    let cases = kikizake::case::parse_cases(&format!("case bitmap\n{input}\nend\n"));
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    for recording in [include_str!("../golden/opengl-32/stdout.txt"), include_str!("../golden/opengl-64/stdout.txt")] {
        let expected: Vec<_> = recording.lines().skip_while(|line| *line != "case gap-wgl-1")
            .skip(1).take_while(|line| !line.starts_with("read ")).collect();
        assert_eq!(&output[..expected.len()], expected);
        let pixels = recording.lines().skip_while(|line| *line != "case gap-wgl-1")
            .find(|line| line.starts_with("read ")).unwrap();
        assert_eq!(output.last().unwrap(), pixels);
    }
}

#[test]
fn wgl_byte_range_bitmaps_match_native_pixels_and_positions() {
    // FNV-1a of the raster-position and RGBA readback lines from both Windows runners.
    let mut digest = 2166136261u32;
    for encoding in ["a", "w"] {
        for code in (0..256).chain((12354..=12355).filter(|_| encoding == "w")) {
            let input = format!("case glyph\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMatrixMode 0x1701\ngl glLoadIdentity\ngl glOrtho 0 16 0 16 -1 1\ngl glMatrixMode 0x1700\ngl glRasterPos2i 2 4\nwglx fontbmp {encoding} {code} 1 100\ngl glCallList 100\nglget f 0xb07 4\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            assert_eq!(output[1], "wglx fontbmp 1 0");
            assert_eq!(output[4], "glerr 0");
            for line in &output[2..4] {
                for byte in line.bytes().chain([b'\n']) {
                    digest = (digest ^ u32::from(byte)).wrapping_mul(16777619);
                }
            }
        }
    }
    assert_eq!(digest, 0xb452b9f8);
}

#[test]
fn wgl_outline_spaces_apply_state_only_when_called() {
    for encoding in ["a", "w"] {
        for code in [32, 160] {
            for format in [0, 1] {
                let input = format!("case space\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glNormal3f 1 0 0\ngl glFrontFace 0x900\nwglx fontoutline {encoding} {code} 1 100 {format}\nglget f 0xb02 3\nglget i 0xb46 1\nglr glIsList b i 100\ngl glCallList 100\nglget f 0xb02 3\nglget i 0xb46 1\nglget f 0xba6 16\nglerr\nend\n");
                let cases = kikizake::case::parse_cases(&input);
                assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), [
                    "glctx ok 1", "wglx fontoutline 1 0",
                    "glyph 0.00048828125 0.00048828125 0 0 0.248535156 0",
                    "get b02 = 3f800000 00000000 00000000", "get b46 = 2304", "ret 1",
                    "get b02 = 00000000 00000000 3f800000", "get b46 = 2305",
                    "get ba6 = 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 00000000 00000000 00000000 3f800000 00000000 3e7e8000 00000000 00000000 3f800000",
                    "glerr 0",
                ]);
            }
        }
    }
}

#[test]
fn wgl_outline_lists_draw_native_lines_and_faces() {
    let mut masks = include_str!("fixtures/opengl_outline_glyph_pixels.txt").lines();
    for format in [0, 1] {
        for code in [65, 66] {
            let input = format!("case outline\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glTranslatef -0.75 -0.75 0\ngl glScalef 2 2 1\nwglx fontoutline a {code} 1 100 {format}\ngl glCallList 100\nglread 0 0 16 16 0x1908 0x1401 1024\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            let mut expected = String::from("read ");
            for pixel in masks.next().unwrap().bytes() {
                expected.push_str(if pixel == b'#' { "ffffffff" } else { "000000ff" });
            }
            assert_eq!(output[3], expected, "format {format} code {code}");
            assert_eq!(output[4], "glerr 0");
        }
    }
    assert!(masks.next().is_none());
}

proptest! {
    #[test]
    fn wgl_font_batches_replace_only_the_requested_lists(
        base in 2u32..4096, length in 1u32..=64, selector in any::<u8>(),
        wide in any::<bool>(), outline in any::<bool>(), filled in any::<bool>(),
    ) {
        use std::fmt::Write;
        let encoding = if wide { "w" } else { "a" };
        let count = if outline { 1 + length % 2 } else { length };
        let first = if outline { 65 } else { u32::from(selector) % (257 - count) };
        let end = base + count;
        let mut input = String::from("case replace\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
        // A neighboring list changes the list base when called. The overwritten
        // list had the same observable effect, which its replacement must erase.
        for name in [base - 1, base, end] {
            writeln!(input, "gl glNewList {name} 0x1300\ngl glListBase {name}\ngl glEndList").unwrap();
        }
        if outline {
            writeln!(input, "wglx fontoutline {encoding} {first} {count} {base} {}", u8::from(filled)).unwrap();
        } else {
            writeln!(input, "wglx fontbmp {encoding} {first} {count} {base}").unwrap();
        }
        for name in base..end {
            writeln!(input, "glr glIsList b i {name}").unwrap();
        }
        writeln!(input, "gl glCallList {base}\nglget i 0xb32 1\ngl glDeleteLists {base} {count}").unwrap();
        for name in base..end {
            writeln!(input, "glr glIsList b i {name}").unwrap();
        }
        for name in [base - 1, end] {
            writeln!(input, "gl glCallList {name}\nglget i 0xb32 1").unwrap();
        }
        input.push_str("glerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        let mut actual = output.iter().map(String::as_str).filter(|s| !s.starts_with("glyph "));
        prop_assert_eq!(actual.next(), Some("glctx ok 1"));
        prop_assert_eq!(actual.next(), Some(if outline { "wglx fontoutline 1 0" } else { "wglx fontbmp 1 0" }));
        for _ in base..end { prop_assert_eq!(actual.next(), Some("ret 1")); }
        prop_assert_eq!(actual.next(), Some("get b32 = 0"));
        for _ in base..end { prop_assert_eq!(actual.next(), Some("ret 0")); }
        for name in [base - 1, end] {
            let expected = format!("get b32 = {name}");
            prop_assert_eq!(actual.next(), Some(expected.as_str()));
        }
        prop_assert_eq!(actual.next(), Some("glerr 0"));
        prop_assert_eq!(actual.next(), None);
    }
}

proptest! {
    #[test]
    fn debug_entry_preserves_pending_gl_errors(a in any::<i32>(), b in any::<i32>()) {
        let input = format!("case debug\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glEnable -1\nglr glDebugEntry i ii {a} {b}\nglerr\ngl glBegin 0\nglr glDebugEntry i ii {b} {a}\ngl glEnd\nglerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        prop_assert_eq!(output, ["glctx ok 1", "ret 0", "glerr 500", "ret 0", "glerr 0"]);
    }
}

#[test]
fn wgl_format_exports_cover_every_generic_dib_descriptor() {
    use std::fmt::Write;
    for recording in [include_str!("../golden/opengl-32/stdout.txt"), include_str!("../golden/opengl-64/stdout.txt")] {
        let descriptors: Vec<_> = recording.lines().skip_while(|line| *line != "glpf 32 n 36")
            .skip(1).take_while(|line| line.starts_with("pf ")).collect();
        assert_eq!(descriptors.len(), 36);
        for descriptor in &descriptors {
            let fields: Vec<_> = descriptor.split_whitespace().collect();
            let index = fields[1].parse::<usize>().unwrap();
            let flags = u32::from_str_radix(fields[2], 16).unwrap();
            if flags & 8 == 0 { continue; } // PFD_DRAW_TO_BITMAP
            let mut input = format!("case descriptors\nglctx {} {index}\n", fields[4]);
            let mut expected = vec![format!("glctx ok {index}")];
            for (i, description) in descriptors.iter().enumerate() {
                let requested = i + 1;
                writeln!(input, "wglx descpf {requested}\nwglx setpf {requested}\nwglx getpf").unwrap();
                expected.extend([String::from("wglx descpf 36"), description.to_string(),
                    String::from(if requested == index { "wglx setpf 1 0" } else { "wglx setpf 0 2000" }),
                    format!("wglx getpf {index}")]);
            }
            input.push_str("glerr\nend\n");
            expected.push(String::from("glerr 0"));
            let cases = kikizake::case::parse_cases(&input);
            assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), expected);
        }
    }
}

#[test]
fn pixel_format_requests_use_windows_field_widths_at_every_entry_point() {
    for request in ["0x38 0 32 0 0 32 8", "070 256 288 -256 512 040 264",
                    "+0x38 -256 -224 256 0x100 0x120 -248"] {
        let input = format!("case pfd\nglchoose 32 {request}\nglctx 32 choose {request}\nwglx choose {request}\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(),
            ["glchoose 1", "glctx ok 1", "wglx choose 1"], "{request}");
    }
}

#[test]
fn light_defaults_match_native_queries_and_write_widths() {
    use std::fmt::Write;
    let recorded: Vec<_> = include_str!("fixtures/opengl_light_defaults.txt").lines().collect();
    assert_eq!(recorded.len(), 40);
    for light in 0x4000..=0x4007 {
        let mut input = String::from("case defaults\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
        for parameter in 0x1200..=0x1209 {
            for ty in ["f", "i"] {
                writeln!(input, "glq glGetLight{ty}v ii {light} {parameter} {ty} 4").unwrap();
            }
        }
        input.push_str("glerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        let offset = if light == 0x4000 { 0 } else { 20 };
        assert_eq!(&output[1..21], &recorded[offset..offset + 20]);
        assert_eq!(output[21], "glerr 0");
    }
}

#[test]
fn invalid_light_queries_preserve_output_and_prioritize_begin_errors() {
    use std::fmt::Write;
    for inside in [false, true] {
        for ty in ["i", "f"] {
            for (light, pname) in [(0x4008, 0x1200), (0x4000, 0x120a), (0x4008, 0x120a), (0x4000, 0x1200)] {
                let mut input = String::from("case invalid-light\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
                if inside { input.push_str("gl glBegin 0\n"); }
                writeln!(input, "glq glGetLight{ty}v ii {light} {pname} {ty} 4").unwrap();
                if inside { input.push_str("gl glEnd\n"); }
                input.push_str("glerr\nglerr\nend\n");
                let error = if inside { "glerr 502" } else if light == 0x4000 && pname == 0x1200 {
                    "glerr 0"
                } else { "glerr 500" };
                let query = match (error == "glerr 0", ty) {
                    (true, "i") => "q 0 0 0 2147482496",
                    (true, _) => "q 0 0 0 1",
                    (false, "i") => "q 2125315823 2125315823 2125315823 2125315823",
                    (false, _) => "q -12345 -12345 -12345 -12345",
                };
                let cases = kikizake::case::parse_cases(&input);
                assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1", query, error, "glerr 0"]);
            }
        }
    }
}

#[test]
fn material_shininess_tracks_faces_and_the_lighting_stack() {
    let input = "case shine\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glMateriali 0x408 0x1601 20\ngl glPushAttrib 0x40\ngl glMaterialf 0x404 0x1601 2.5\ngl glMateriali 0x405 0x1601 129\nglerr\nglq glGetMaterialiv ii 0x404 0x1601 i 1\nglq glGetMaterialfv ii 0x404 0x1601 f 2\nglq glGetMaterialfv ii 0x405 0x1601 f 1\ngl glPopAttrib\nglq glGetMaterialfv ii 0x404 0x1601 f 1\nglq glGetMaterialiv ii 0x405 0x1603 i 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1", "glerr 501",
        "q 3", "q 2.5 -12345", "q 20", "q 20", "q 0 1 1 2125315823", "glerr 0"]);
}

#[test]
fn material_vectors_preserve_face_selection_and_round_indices() {
    let input = "case vectors\nglctx 32 choose 0x38 0 32 0 0 32 8\nglp glMaterialfv ii 0x408 0x1603 f -1.5 2.5 4.75\nglq glGetMaterialfv ii 0x404 0x1603 f 4\nglq glGetMaterialiv ii 0x405 0x1603 i 4\ngl glPushAttrib 0x40\nglp glMaterialiv ii 0x405 0x1603 i -3 4 7\nglq glGetMaterialfv ii 0x405 0x1603 f 3\ngl glPopAttrib\nglq glGetMaterialfv ii 0x405 0x1603 f 3\nglp glMaterialfv ii 0x408 0x1601 f 2.5\nglq glGetMaterialiv ii 0x405 0x1601 i 1\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1", "q -1.5 2.5 4.75 -12345",
        "q -2 3 5 2125315823", "q -3 4 7", "q -1.5 2.5 4.75", "q 3", "glerr 0"]);
}

proptest! {
    #[test]
    fn wgl_layer_palette_results_depend_on_plane(
        plane in prop_oneof![Just(0i32), any::<i32>()], start in any::<i32>(),
        count in 0u16..=256, realize in any::<i32>(),
    ) {
        let input = format!("case palette\nglctx 32 choose 0x38 0 32 0 0 32 8\nwglx getlayerpal {plane} {start} {count}\nwglx setlayerpal {plane} {start} {count}\nwglx realize {plane} {realize}\nglerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        let error = if plane == 0 { 0 } else { 87 };
        prop_assert_eq!(output, ["glctx ok 1".into(), format!("wglx getlayerpal 0 {error}"),
            format!("wglx setlayerpal 0 {error}"), format!("wglx realize 0 {error}"), "glerr 0".into()]);
    }
}

proptest! {
    #[test]
    fn single_buffer_swaps_preserve_pixels_and_gl_error(
        color in proptest::array::uniform3(any::<u8>()), mask in any::<i32>(),
    ) {
        let [r, g, b] = color.map(|v| f32::from(v) / 255.0);
        let input = format!("case swap\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glClearColor {r} {g} {b} 1\ngl glClear 0x4000\nglread 0 0 1 1 0x1908 0x1401 4\ngl glEnable -1\nwglx swap\nwglx swaplayer {mask}\nglerr\nglread 0 0 1 1 0x1908 0x1401 4\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        prop_assert_eq!(&output[2..5], ["wglx swap 1 0", "wglx swaplayer 1 0", "glerr 500"]);
        prop_assert_eq!(&output[1], &output[5]);
    }
}

#[test]
fn bitmap_font_creation_inside_a_primitive_preserves_existing_lists() {
    for encoding in ["a", "w"] {
        for count in [0, 1, 3] {
            let input = format!("case bmp-begin\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glNewList 100 0x1300\ngl glListBase 12\ngl glEndList\ngl glBegin 0\nwglx fontbmp {encoding} 65 {count} 100\ngl glVertex2i 0 0\ngl glEnd\nglerr\ngl glCallList 100\nglget i 0xb32 1\nglr glIsList b i 101\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let (call, error) = if count == 0 { ("wglx fontbmp 0 87", "glerr 0") }
                else { ("wglx fontbmp 0 0", "glerr 502") };
            assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(),
                ["glctx ok 1", call, error, "get b32 = 12", "ret 0", "glerr 0"]);
        }
    }
}

#[test]
fn light_scalar_validation_preserves_the_previous_value() {
    use std::fmt::Write;
    let mut input = String::from("case scalars\nglctx 32 choose 0x38 0 32 0 0 32 8\n");
    for pname in 0x1205..=0x1209 {
        for value in ["2.5", "-1", "91", "129", "180"] {
            writeln!(input, "gl glLightf 0x4007 {pname} {value}\nglq glGetLightiv ii 0x4007 {pname} i 1\nglerr").unwrap();
        }
    }
    input.push_str("end\n");
    let cases = kikizake::case::parse_cases(&input);
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    assert_eq!(output[1..], include_str!("fixtures/opengl_light_scalars.txt").lines().collect::<Vec<_>>());
}

proptest! {
    #[test]
    fn scalar_and_vector_light_calls_share_per_light_state(
        light in 0x4000u32..=0x4007, pname in 0x1205u32..=0x1209,
        value in -2i32..=182, use_float in any::<bool>(),
    ) {
        let ty = if use_float { "f" } else { "i" };
        let run = |set: &str| {
            let input = format!("case light\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glPushAttrib 0x40\n{set}\nglerr\nglq glGetLightfv ii {light} {pname} f 1\ngl glPopAttrib\nglq glGetLightfv ii {light} {pname} f 1\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            kikizake::model::run_case(&cases[0]).unwrap()
        };
        let scalar = run(&format!("gl glLight{ty} {light} {pname} {value}"));
        let vector = run(&format!("glp glLight{ty}v ii {light} {pname} {ty} {value} 93"));
        prop_assert_eq!(scalar, vector);
    }
}

#[test]
fn light_position_is_transformed_when_specified() {
    let input = "case position\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glTranslatef 3 4 5\ngl glScalef 2 3 4\nglp glLightfv ii 0x4002 0x1203 f 1 2 -1 0\nglp glLightiv ii 0x4003 0x1203 i 1 2 -1 1\ngl glLoadIdentity\nglq glGetLightfv ii 0x4002 0x1203 f 4\nglq glGetLightfv ii 0x4003 0x1203 f 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(),
        ["glctx ok 1", "q 2 6 -4 0", "q 5 10 1 1", "glerr 0"]);
}

#[test]
fn spot_direction_is_transformed_without_translation_or_normalization() {
    let input = "case dir\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glTranslatef 3 4 5\ngl glScalef 2 3 4\nglp glLightfv ii 0x4002 0x1204 f 4 6 -8\nglp glLightiv ii 0x4003 0x1204 i 1 2 -1\ngl glLoadIdentity\nglq glGetLightfv ii 0x4002 0x1204 f 4\nglq glGetLightfv ii 0x4003 0x1204 f 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(),
        ["glctx ok 1", "q 2 2 -2 -12345", "q 0.5 0.666666687 -0.25 -12345", "glerr 0"]);
}

#[test]
fn light_color_conversions_match_windows_outside_the_unit_interval() {
    let input = "case lightcolor\nglctx 32 choose 0x38 0 32 0 0 32 8\nglp glLightfv ii 0x4003 0x1201 f -0.5 0.25 1 2\nglq glGetLightfv ii 0x4003 0x1201 f 4\nglq glGetLightiv ii 0x4003 0x1201 i 4\nglp glLightiv ii 0x4003 0x1202 i -2147483648 -1 0 2147483647\nglq glGetLightfv ii 0x4003 0x1202 f 4\nglq glGetLightiv ii 0x4003 0x1202 i 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1", "q -0.5 0.25 1 2",
        "q -1073741248 536870624 2147482496 -2147483648",
        "q -1.0000006 -2.32830782e-10 2.32830782e-10 1.0000006",
        "q -2147483648 -1 0 -2147483648", "glerr 0"]);
}

#[test]
fn scene_light_model_preserves_per_channel_ambient_and_flags() {
    let input = "case scene\nglctx 32 choose 0x38 0 32 0 0 32 8\nglp glLightModelfv i 0xb53 f -0.5 0.1 0.3 2\nglget f 0xb53 4\ngl glPushAttrib 0x40\nglp glLightModeliv i 0xb53 i -2147483648 -1 0 2147483647\nglget f 0xb53 4\ngl glLightModelf 0xb51 -0.25\nglget i 0xb51 1\ngl glLightModeli 0xb52 -5\nglget i 0xb52 1\ngl glPopAttrib\nglget f 0xb53 4\nglget i 0xb51 1\nglget i 0xb52 1\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1",
        "get b53 = bf000000 3dccccce 3e99999a 40000000", "get b53 = bf800005 af800005 2f800005 3f800005",
        "get b51 = 1", "get b52 = 1", "get b53 = bf000000 3dccccce 3e99999a 40000000",
        "get b51 = 0", "get b52 = 0", "glerr 0"]);
}

#[test]
fn scene_ambient_channels_match_native_point_pixels() {
    for (ambient, default, tracked) in [
        ("1 0 0", "330000ff", "3f0000ff"),
        ("0 1 0", "003300ff", "007f00ff"),
        ("0 0 1", "000033ff", "0000ffff"),
        ("1 0.5 0.25", "33190cff", "3f3f3fff"),
    ] {
        for (material, pixel) in [("", default), ("gl glEnable 0xb57\ngl glColor4f 0.25 0.5 1 1\n", tracked)] {
            let input = format!("case ambient\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\n{material}glp glLightModelfv i 0xb53 f {ambient} 1\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 7 7 2 2 0x1908 0x1401 16\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            let expected = format!("read 000000ff{pixel}000000ff000000ff");
            assert_eq!(&output[1..], [expected.as_str(), "glerr 0"], "{ambient} {material}");
        }
    }
}

#[test]
fn directional_light_colors_match_native_point_pixels() {
    for (z, pixel) in [(-1, "001900ff"), (0, "001900ff"), (1, "cc1966ff")] {
        let input = format!("case directional\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\ngl glEnable 0x4003\nglp glLightModelfv i 0xb53 f 0 0 0 1\nglp glLightfv ii 0x4003 0x1200 f 0 0.5 0 1\nglp glLightfv ii 0x4003 0x1201 f 1 0 0.5 1\nglp glLightfv ii 0x4003 0x1203 f 0 0 {z} 0\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 7 7 2 2 0x1908 0x1401 16\nglerr\nend\n");
        let cases = kikizake::case::parse_cases(&input);
        let output = kikizake::model::run_case(&cases[0]).unwrap();
        let expected = format!("read 000000ff{pixel}000000ff000000ff");
        assert_eq!(&output[1..], [expected.as_str(), "glerr 0"]);
    }
}

#[test]
fn positional_light_attenuation_matches_native_point_pixels() {
    for (z, pixels) in [(1, ["cc", "66", "66", "66"]), (2, ["cc", "66", "44", "28"])] {
        for ([c, l, q], pixel) in zip_eq([[1,0,0], [2,0,0], [1,1,0], [1,0,1]], pixels) {
            let input = format!("case positional\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\ngl glEnable 0x4000\nglp glLightModelfv i 0xb53 f 0 0 0 1\nglp glLightfv ii 0x4000 0x1203 f 0 0 {z} 1\ngl glLightf 0x4000 0x1207 {c}\ngl glLightf 0x4000 0x1208 {l}\ngl glLightf 0x4000 0x1209 {q}\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 8 7 1 1 0x1908 0x1401 4\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            let expected = format!("read {pixel}{pixel}{pixel}ff");
            assert_eq!(&output[1..], [expected.as_str(), "glerr 0"]);
        }
    }
}

#[test]
fn spot_cutoff_and_exponent_match_native_point_pixels() {
    for w in [0, 1] {
        for (direction, pixels) in [
            ("0 0 -1", ["cc"; 6]), ("0.6 0 -0.8", ["00","00","cc","82","cc","cc"]),
            ("0 0 1", ["00","00","00","00","cc","cc"]),
        ] {
            for (i, (angle, exponent)) in [(30,0),(30,2),(60,0),(60,2),(180,0),(180,2)].into_iter().enumerate() {
                let input = format!("case spot\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\ngl glEnable 0x4000\nglp glLightModelfv i 0xb53 f 0 0 0 1\nglp glLightfv ii 0x4000 0x1203 f 0 0 1 {w}\nglp glLightfv ii 0x4000 0x1204 f {direction}\ngl glLightf 0x4000 0x1206 {angle}\ngl glLightf 0x4000 0x1205 {exponent}\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 8 7 1 1 0x1908 0x1401 4\nglerr\nend\n");
                let cases = kikizake::case::parse_cases(&input);
                let output = kikizake::model::run_case(&cases[0]).unwrap();
                let expected = format!("read {}ff", pixels[i].repeat(3));
                assert_eq!(&output[1..], [expected.as_str(), "glerr 0"]);
            }
        }
    }
}

#[test]
fn emission_draws_with_lights_off_and_specular_is_per_face() {
    let input = "case mat\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\nglp glLightModelfv i 0xb53 f 0 0 0 1\nglp glMaterialfv ii 0x404 0x1600 f 0.25 0.5 0.75 1\nglp glMaterialiv ii 0x405 0x1202 i -2147483648 -1 0 2147483647\nglq glGetMaterialfv ii 0x405 0x1202 f 4\nglq glGetMaterialiv ii 0x404 0x1600 i 4\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 8 7 1 1 0x1908 0x1401 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    assert_eq!(kikizake::model::run_case(&cases[0]).unwrap(), ["glctx ok 1",
        "q -1.00000048 -2.32830755e-10 2.32830755e-10 1.00000048",
        "q 536870624 1073741248 1610611968 2147482496", "read 3f7fbfff", "glerr 0"]);
}

#[test]
fn specular_highlights_follow_light_viewer_and_shininess() {
    for (local, dir, native) in [
        (0, "0 0 1", [255, 255, 255]), (0, "0.6 0 0.8", [255, 207, 108]),
        (0, "0 0 -1", [0; 3]), (1, "0 0 1", [255, 0, 0]),
        (1, "0.6 0 0.8", [0; 3]), (1, "0 0 -1", [0; 3]),
    ] {
        for (shine, red) in zip_eq([0, 4, 16], native) {
            let input = format!("case spec\nglctx 32 choose 0x38 0 32 0 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\ngl glEnable 0x4000\nglp glLightModelfv i 0xb53 f 0 0 0 1\nglp glLightfv ii 0x4000 0x1201 f 0 0 0 1\nglp glLightfv ii 0x4000 0x1203 f {dir} 0\nglp glMaterialfv ii 0x404 0x1202 f 1 0.5 0.25 1\ngl glMateriali 0x404 0x1601 {shine}\ngl glLightModeli 0xb51 {local}\ngl glBegin 0\ngl glVertex3f 0 0 0.5\ngl glEnd\nglread 8 7 1 1 0x1908 0x1401 4\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            let pixel = output[1].strip_prefix("read ").unwrap();
            for (k, expected) in [red, red / 2, red / 4, 255u8].into_iter().enumerate() {
                let channel = u8::from_str_radix(&pixel[2*k..2*k+2], 16).unwrap();
                let oblique = local == 0 && dir == "0.6 0 0.8" && shine > 0 && k < 3;
                assert!(channel.abs_diff(expected) <= u8::from(oblique), "{dir} {shine} {pixel}");
            }
            assert_eq!(output[2], "glerr 0");
        }
    }
}

#[test]
fn combined_material_update_keeps_face_selection_and_diffuse_alpha() {
    let input = "case materials\nglctx 32 choose 0x38 0 32 8 0 32 8\ngl glDisable 0xbd0\ngl glEnable 0xb50\nglp glLightModelfv i 0xb53 f 1 1 1 1\nglp glMaterialfv ii 0x408 0x1602 f 0.1 0.25 0.75 0.5\nglp glMaterialfv ii 0x405 0x1200 f 0 1 0 1\nglq glGetMaterialfv ii 0x404 0x1200 f 4\nglq glGetMaterialfv ii 0x405 0x1200 f 4\nglq glGetMaterialiv ii 0x405 0x1201 i 4\ngl glBegin 0\ngl glVertex2i 0 0\ngl glEnd\nglread 8 7 1 1 0x1908 0x1401 4\nglerr\nend\n";
    let cases = kikizake::case::parse_cases(input);
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    assert_eq!(&output[1..], ["q 0.100000001 0.25 0.75 0.5", "q 0 1 0 1",
        "q 214748256 536870624 1610611840 1073741248", "read 193fbf7f", "glerr 0"]);
}

#[test]
fn color_material_protects_only_the_tracked_face_and_property() {
    let input = "case material-switch
glctx 32 choose 0x38 0 32 8 0 32 8
gl glColor4f 0.25 0.5 0.75 1
gl glColorMaterial 0x404 0x1201
gl glEnable 0xb57
glq glGetMaterialfv ii 0x404 0x1201 f 4
glp glMaterialfv ii 0x408 0x1602 f 1 0 0 1
glq glGetMaterialfv ii 0x404 0x1201 f 4
glq glGetMaterialfv ii 0x404 0x1200 f 4
glq glGetMaterialfv ii 0x405 0x1201 f 4
gl glColorMaterial 0x405 0x1202
glq glGetMaterialfv ii 0x405 0x1202 f 4
gl glColor4f 0 1 0 1
gl glDisable 0xb57
gl glColor4f 1 1 1 1
glq glGetMaterialfv ii 0x405 0x1202 f 4
glerr
end
";
    let cases = kikizake::case::parse_cases(input);
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    assert_eq!(&output[1..], ["q 0.25 0.5 0.75 1", "q 0.25 0.5 0.75 1", "q 1 0 0 1",
        "q 1 0 0 1", "q 0.25 0.5 0.75 1", "q 0 1 0 1", "glerr 0"]);
}

#[test]
fn every_color_material_selector_protects_exactly_its_components() {
    use std::fmt::Write;
    let faces = [(FRONT, 1), (BACK, 2), (FRONT_AND_BACK, 3)];
    // Bit positions represent ambient, diffuse, specular and emission respectively.
    let modes = [(0x1200, 1), (0x1201, 2), (0x1202, 4), (0x1600, 8), (0x1602, 3)];
    for (track_face, tracked_faces) in faces {
        for (track_mode, tracked_properties) in modes {
            for (write_face, written_faces) in faces {
                for (write_mode, written_properties) in modes {
                    let mut input = String::from("case selectors\nglctx 32 choose 0x38 0 32 8 0 32 8\n");
                    for &(property, _) in &modes[..4] {
                        writeln!(input, "glp glMaterialfv ii {FRONT_AND_BACK} {property} f 0 0 0 1").unwrap();
                    }
                    writeln!(input, "gl glColor4f 1 0 0 1\ngl glColorMaterial {track_face} {track_mode}\ngl glEnable 0xb57").unwrap();
                    writeln!(input, "glp glMaterialfv ii {write_face} {write_mode} f 0 1 0 1").unwrap();
                    let mut expected = Vec::new();
                    for &(face, side) in &faces[..2] {
                        for &(property, channel) in &modes[..4] {
                            writeln!(input, "glq glGetMaterialfv ii {face} {property} f 4").unwrap();
                            expected.push(if tracked_faces & side != 0 && tracked_properties & channel != 0 {
                                "q 1 0 0 1"
                            } else if written_faces & side != 0 && written_properties & channel != 0 {
                                "q 0 1 0 1"
                            } else {
                                "q 0 0 0 1"
                            });
                        }
                    }
                    input.push_str("glerr\nend\n");
                    expected.push("glerr 0");
                    let cases = kikizake::case::parse_cases(&input);
                    let output = kikizake::model::run_case(&cases[0]).unwrap();
                    assert_eq!(&output[1..], expected, "{input}");
                }
            }
        }
    }
}

#[test]
fn two_sided_lighting_selects_material_and_reverses_back_normals() {
    let input = "case sides
glctx 32 choose 0x38 0 32 8 0 32 8
gl glDisable 0xbd0
gl glEnable 0xb50
gl glEnable 0x4000
gl glLightModeli 0xb52 1
glp glLightModelfv i 0xb53 f 0 0 0 1
glp glMaterialfv ii 0x404 0x1201 f 1 0 0 1
glp glMaterialfv ii 0x405 0x1201 f 0 1 0 0.5
gl glFrontFace 2305
gl glNormal3f 0 0 1
gl glRectf -1 -1 1 1
glread 8 8 1 1 0x1908 0x1401 4
gl glFrontFace 2304
gl glNormal3f 0 0 -1
gl glRectf -1 -1 1 1
glread 8 8 1 1 0x1908 0x1401 4
gl glFrontFace 2304
gl glNormal3f 0 0 1
gl glRectf -1 -1 1 1
glread 8 8 1 1 0x1908 0x1401 4
glerr
end
";
    let cases = kikizake::case::parse_cases(input);
    let output = kikizake::model::run_case(&cases[0]).unwrap();
    assert_eq!(&output[1..], ["read ff0000ff", "read 00ff007f", "read 0000007f", "glerr 0"]);
}

#[test]
fn debug_entry_preserves_each_gl_error_inside_and_outside_primitives() {
    let overflow = "gl glPushMatrix\n".repeat(64);
    for (setup, error) in [("", "0"), ("gl glEnable -1\n", "500"), ("gl glLineWidth -1\n", "501"),
                           ("gl glEnd\n", "502"), (overflow.as_str(), "503"), ("gl glPopMatrix\n", "504")] {
        for inside in [false, true] {
            let (begin, end) = if inside { ("gl glBegin 0\n", "gl glEnd\n") } else { ("", "") };
            let input = format!("case debug\nglctx 32 choose 0x38 0 32 0 0 32 8\n{setup}{begin}glr glDebugEntry i ii -2147483648 2147483647\n{end}glerr\nglerr\nend\n");
            let cases = kikizake::case::parse_cases(&input);
            let output = kikizake::model::run_case(&cases[0]).unwrap();
            assert_eq!(&output[1..], ["ret 0".to_string(), format!("glerr {error}"), "glerr 0".to_string()], "{input}");
        }
    }
}
