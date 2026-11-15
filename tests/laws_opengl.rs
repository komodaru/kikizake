//! Laws of the OpenGL model: what the GL 1.1 specification states as properties, whatever
//! Windows' measured details turn out to be.
use kikizake::compare::line_matches;
use kikizake::model::opengl::e::*;
use kikizake::model::opengl::tri::{self, Sub, Vertex};
use kikizake::model::opengl::{blend_bytes, blend_undefined, logic, logic_reads_dst, Fog, FogMode};
use proptest::prelude::*;

fn fog(mode: FogMode, density: f32, start: f32, end: f32) -> Fog {
    Fog { mode, density, start, end, index: 0.0, color: [0.0; 4] }
}

proptest! {
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
        prop_assert!(((sa.0 - o.0) as f64 - want).abs() <= 0.5 + 1e-6);
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
        let p = tri::clip(&t);
        if t.iter().all(|c| c[0].abs() <= 1.0 && c[1].abs() <= 1.0) {
            prop_assert_eq!(p, t.to_vec());
            return Ok(());
        }
        let area = (t[1][0] - t[0][0]) * (t[2][1] - t[0][1]) - (t[2][0] - t[0][0]) * (t[1][1] - t[0][1]);
        let side = |a: [f64; 2], b: [f64; 2], c: [f64; 2]| (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        for c in &p {
            prop_assert!(c[0].abs() <= 1.0 + 1e-9 && c[1].abs() <= 1.0 + 1e-9);
            for k in 0..3 {
                let s = side(t[k], t[(k + 1) % 3], *c) * area.signum();
                prop_assert!(s >= -1e-9 * (1.0 + area.abs()), "{c:?} off the triangle");
            }
        }
        if p.len() >= 3 {
            let turns: Vec<f64> = (0..p.len()).map(|k| side(p[k], p[(k + 1) % p.len()], p[(k + 2) % p.len()]) * area.signum()).collect();
            prop_assert!(turns.iter().all(|t| *t >= -1e-9), "not convex: {p:?}");
        }
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
}
