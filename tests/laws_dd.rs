//! Properties of the DirectDraw and Direct3D 1/2/3/7 models (src/model/ddraw.rs, d3d7.rs) that
//! hold whatever the measured details: exhaustive over small domains, property-based elsewhere.
//! With Windows' answers they are what cargo-mutants counts on to catch mutants of the models.
use proptest::prelude::*;
use kikizake::case::{parse_cases, parse_output, tokens};
use kikizake::model::d3d7::{clear_z, f32_past, SlopeRound, gouraud_channel, line, pixel, rs_default, tri_z, tri_z_write, Depth, Family, Fmt, Ver};
use kikizake::model::ddraw::{pal_entries, pitch};
use kikizake::model::{self, run_case, Fail};

fn try_run(text: &str) -> Result<Vec<String>, Fail> {
    run_case(&parse_cases(&format!("case p\n{text}end\n"))[0])
}

fn run(text: &str) -> Vec<String> {
    try_run(text).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

/// the rows a dump printed (after its header line)
fn rows(out: &[String], header: &str) -> Vec<String> {
    let i = out.iter().position(|l| l.starts_with(header)).expect(header);
    let h: usize = out[i].split(' ').nth(3).unwrap().parse().unwrap();
    out[i + 1..i + 1 + h].to_vec()
}

const FMTS: [&str; 4] = ["565", "555", "24", "32"];

/// every z in millis into 16 and 32 bits: 0 at 0, never decreasing; a triangle's z below
/// z * 2^16 (2^32) by less than one step (a float's step, 32 bits), the 16-bit value the top of
/// the 32-bit one; a write over an old z keeps the old low byte's set bits and nothing else
#[test]
fn z_values_are_monotone() {
    for (bits, dp) in [(16, Depth::Z16), (32, Depth::Z32)] {
        let max = dp.max();
        let (mut pc, mut pt) = (0, 0);
        for m in 0..=1000 {
            let z = m as f32 / 1000.0;
            let (c, t) = (clear_z(dp, z), tri_z(dp, z));
            assert!(c >= pc && t >= pt, "{bits} {m}");
            assert!(c <= max && t <= max);
            let x = z as f64 * if bits == 32 { 4294967296.0 } else { 65536.0 };
            let step = if bits == 32 { (x / 8388608.0).max(1.0) + 1.0 } else { 1.0 };
            assert!(m == 0 || (f64::from(t) < x && x <= f64::from(t) + step), "{bits} {m}");
            assert_eq!(tri_z(Depth::Z16, z), tri_z(Depth::Z32, z) >> 16, "{m}");
            for old in [0, 0x80, 0xff, 0x1234_5678, u32::MAX] {
                let w = tri_z_write(dp, z, old);
                assert_eq!(w & !0xff, t & !0xff);
                assert_eq!(w & 0xff, if bits == 32 { (t | old) & 0xff } else { t & 0xff });
            }
            (pc, pt) = (c, t);
        }
        assert_eq!(clear_z(dp, 0.0), 0);
        assert_eq!(tri_z(dp, 0.0), 0);
        assert_eq!(clear_z(dp, 1.0), max);
    }
    // (z 1 is not drawn by the software rasterizers, D3D7.ZRANGE)
    assert_eq!(tri_z(Depth::Z16, 1.0), 65535);
    assert_eq!(tri_z(Depth::Z32, 1.0), 0xffff_ff00);
    // (z · 2^32 less 1 a float exactly: kept, not chopped further)
    assert_eq!(tri_z(Depth::Z32, 1.0 / 256.0), 0x00ff_ffff);
    // d3d7dev zrange (over 0x80000000) and zhist (over 0xffffffff)
    for (m, a, b) in [(1, 0x0041_8936, 0x0041_89ff), (500, 0x7fff_ff80, 0x7fff_ffff), (990, 0xfd70_a300, 0xfd70_a3ff), (999, 0xffbe_7600, 0xffbe_76ff)] {
        let z = m as f32 / 1000.0;
        assert_eq!((tri_z_write(Depth::Z32, z, 0x8000_0000), tri_z_write(Depth::Z32, z, u32::MAX)), (a, b), "{m}");
    }
}

/// every value of each channel: the target pixel keeps the channel's top bits, monotone, alpha gone
#[test]
fn pixels_keep_the_top_bits() {
    for f in FMTS {
        for sh in [0, 8, 16, 24] {
            let mut prev = 0;
            for v in 0..=255u32 {
                let fm = Fmt::parse(f).unwrap();
                let p = pixel(fm, v << sh);
                if sh == 24 {
                    assert_eq!(p, 0, "{f} alpha");
                    continue;
                }
                assert!(p >= prev, "{f} {sh} {v}");
                prev = p;
                assert_eq!(pixel(fm, v << sh | 0xff00_0000), p);
            }
        }
    }
    assert_eq!(pixel(Fmt::R565, 0xffffff), 0xffff);
    assert_eq!(pixel(Fmt::R555, 0xffffff), 0x7fff);
}

/// a new RGB device answers every state 0..160 with the table's default
#[test]
fn render_state_defaults_read_back() {
    for v in [2, 3, 7] {
        let mut text = format!("d3d {v}\nd3ddev rgb 32 4 4\n");
        let mut want = vec![];
        for st in 0..=160 {
            text += &format!("d3drs get {st}\n");
            if let Some(d) = rs_default(Ver::parse(v).unwrap(), st) {
                want.push((st, d));
            }
        }
        let out = run(&text);
        assert!(!want.is_empty());
        for (st, d) in want {
            assert!(out.contains(&format!("rs {st} = {d:08x}")), "{v} {st}");
        }
    }
}

/// surface pitches: at least the row's bytes, aligned, and growing with the width
#[test]
fn pitches_hold_a_row() {
    for arch in [32, 64] {
        model::set_arch(arch);
        for bpp in [8, 16, 24, 32] {
            let mut prev = 0;
            for w in 1..300 {
                let p = pitch(w, bpp);
                assert!(p >= w * bpp as i64 / 8 && p % 4 == 0 && p >= prev, "{arch} {bpp} {w}");
                prev = p;
            }
        }
    }
    model::set_arch(32);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    /// Clear of the whole target: every pixel the color as the target holds it
    #[test]
    fn clear_fills_the_target(v in prop::sample::select(vec![2u32, 3, 7]), f in prop::sample::select(FMTS.to_vec()), w in 1u32..12, h in 1u32..9, c: u32) {
        let out = run(&format!("d3d {v}\nd3ddev rgb {f} {w} {h}\nd3dclear {c:#x}\nd3ddump\n"));
        let b = if f == "24" { 3 } else if f == "32" { 4 } else { 2 };
        let px: String = pixel(Fmt::parse(f).unwrap(), c).to_le_bytes()[..b].iter().map(|x| format!("{x:02x}")).collect();
        for r in rows(&out, "dd rt") {
            prop_assert_eq!(r, px.repeat(w as usize));
        }
    }

    /// with culling off a triangle's vertex order does not matter, and two triangles of one color
    /// drawn twice change nothing more
    #[test]
    fn vertex_order_and_redraw(x: [i64; 3], y: [i64; 3], c in 0u32..0x100_0000) {
        let x = x.map(|v| v.rem_euclid(200) - 20);
        let y = y.map(|v| v.rem_euclid(150) - 20);
        let t = |i: [usize; 3]| format!("d3dtri {:#x} {} {} {} {} {} {}\n", c | 0xff00_0000, x[i[0]], y[i[0]], x[i[1]], y[i[1]], x[i[2]], y[i[2]]);
        let base = "d3d 7\nd3ddev rgb 32 10 8\nd3drs set 22 1\nd3dclear 0xff000000\n";
        let a = try_run(&format!("{base}{}d3ddump\n", t([0, 1, 2])));
        if let Ok(a) = a {
            for o in [[0, 2, 1], [1, 2, 0], [2, 1, 0]] {
                prop_assert_eq!(&a, &run(&format!("{base}{}d3ddump\n", t(o))));
            }
            prop_assert_eq!(&a, &run(&format!("{base}{}{}d3ddump\n", t([0, 1, 2]), t([0, 1, 2]))));
        }
    }

    /// the software rasterizers' Gouraud spans (crate::model::d3dim, D3D7.GOURAUD): each pixel's
    /// 8.8 colour, inside or across the target, lies within the f32 error and the truncated
    /// per-pixel step of the exact colour plane at that pixel (a level and an eighth, plus the
    /// span's length times 1/256), and the pixels are those of the walk alone
    #[test]
    fn gouraud_spans_follow_the_plane(v in prop::array::uniform6(-640i64..1280), c in prop::array::uniform3(any::<u32>())) {
        use kikizake::model::d3dim::{raster, Clip, Vert};
        let p = |i: usize| (v[2 * i], v[2 * i + 1]);
        let vert = |i: usize| Vert { x: v[2 * i] as f32 / 16.0, y: v[2 * i + 1] as f32 / 16.0, c: c[i].to_le_bytes() };
        let clip = Clip { l: 0, t: 0, r: 48, b: 40 };
        let den = ((p(1).0 - p(0).0) * (p(2).1 - p(0).1) - (p(2).0 - p(0).0) * (p(1).1 - p(0).1)) as f64;
        prop_assume!(den.abs() > 256.0 * 4.0);
        let Some(r) = raster([vert(0), vert(1), vert(2)], clip, true).unwrap() else { return Ok(()) };
        let g = raster([vert(0), vert(1), vert(2)], clip, false).unwrap().unwrap();
        prop_assert_eq!(r.px.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>(), g.px.iter().map(|q| (q.x, q.y)).collect::<Vec<_>>());
        let width = r.px.iter().map(|q| q.x).max().unwrap_or(0) - r.px.iter().map(|q| q.x).min().unwrap_or(0) + 1;
        for q in &r.px {
            for k in 0..4 {
                let ch = |i: usize| f64::from(c[i].to_le_bytes()[k]);
                let (x, y) = (q.x as f64 * 16.0, q.y as f64 * 16.0);
                let l1 = ((x - p(0).0 as f64) * (p(2).1 - p(0).1) as f64 - (p(2).0 - p(0).0) as f64 * (y - p(0).1 as f64)) / den;
                let l2 = ((p(1).0 - p(0).0) as f64 * (y - p(0).1 as f64) - (x - p(0).0 as f64) * (p(1).1 - p(0).1) as f64) / den;
                let exact = 256.0 * (ch(0) + l1 * (ch(1) - ch(0)) + l2 * (ch(2) - ch(0)));
                // (a colour plane leaving 0..255 between the pixel centres wraps: left out)
                prop_assume!((0.0..=65280.0).contains(&exact));
                let got = f64::from(q.c[k]);
                prop_assert!((got - exact).abs() <= 288.0 + width as f64 + exact.abs() * 1e-5, "{:?} channel {k}: {got} against {exact}", (q.x, q.y));
            }
        }
    }

    /// the software rasterizers' walk of a triangle inside or across the target: every pixel more than
    /// 1.5 px inside its three edges is drawn, and none more than 1.5 px outside one (the walk's
    /// shifts and ties move edges by at most a pixel; a pixel past the right side spills into
    /// the next row, so the target is as wide as the triangle can reach)
    #[test]
    fn walk_stays_by_the_edges(x: [i64; 3], y: [i64; 3], dev in prop::sample::select(vec!["rgb", "mmx", "ramp"])) {
        // (inside the target, or crossing it: walked, cut at its sides)
        let x = x.map(|v| v.rem_euclid(241) - 40);
        let y = y.map(|v| v.rem_euclid(241) - 40);
        let inside = x.iter().all(|&v| (0..=160).contains(&v)) && y.iter().all(|&v| (0..=176).contains(&v));
        prop_assume!(inside || x.iter().any(|&v| !(0..=192).contains(&v)) || y.iter().any(|&v| !(0..=176).contains(&v)));
        let base = format!("d3d 2\nd3ddev {dev} 32 12 11\nd3drs set 22 1\nd3dclear 0xff000000\n");
        let out = try_run(&format!("{base}d3dtri 0xffffffff {} {} {} {} {} {}\nd3ddump\n", x[0], y[0], x[1], y[1], x[2], y[2]));
        let area = (x[1] - x[0]) * (y[2] - y[0]) - (x[2] - x[0]) * (y[1] - y[0]);
        if let (Ok(out), true) = (out, area != 0) {
            let r = rows(&out, "dd rt");
            let s = area.signum();
            for (py, row) in r.iter().enumerate() {
                for px in 0..12 {
                    let on = &row[8 * px..8 * px + 8] != "00000000";
                    // the least signed distance to an edge, in pixels, inside positive
                    let d = (0..3).map(|i| {
                        let (ax, ay, bx, by) = (x[i], y[i], x[(i + 1) % 3], y[(i + 1) % 3]);
                        let e = (bx - ax) * (py as i64 * 16 - ay) - (by - ay) * (px as i64 * 16 - ax);
                        (s * e) as f64 / (((bx - ax).pow(2) + (by - ay).pow(2)) as f64).sqrt() / 16.0
                    }).fold(f64::INFINITY, f64::min);
                    prop_assert!(!(d > 1.5 && !on) && !(d < -1.5 && on), "({px},{py}) d {d} on {on}");
                }
            }
        }
    }

    /// blending: alpha 255 gives the source's colors, alpha 0 the destination's, each scaled by 255/256
    #[test]
    fn blend_ends(seed in 1u32..1000, c in 0u32..0x100_0000, f in prop::sample::select(vec!["32", "24"])) {
        let base = format!("d3d 7\nd3ddev rgb {f} 6 3\nd3drs set 22 1\nd3dfill {seed}\n");
        let before = run(&format!("{base}d3ddump\n"));
        let tri = "-16 -16 600 -16 -16 600";
        let st = "d3drs set 27 1\nd3drs set 19 5\nd3drs set 20 6\n";
        let full = run(&format!("{base}{st}d3dtri {:#x} {tri}\nd3ddump\n", 0xff00_0000 | c));
        let none = run(&format!("{base}{st}d3dtri {c:#x} {tri}\nd3ddump\n"));
        let plain = run(&format!("{base}d3dtri {:#x} {tri}\nd3ddump\n", 0xff00_0000 | c));
        // (each end scaled by 255/256: one less for every value but 0, as Windows measures)
        let b = if f == "32" { 4 } else { 3 };
        let strip = |r: &String| (0..r.len() / 2).map(|i| u32::from_str_radix(&r[2 * i..2 * i + 2], 16).unwrap()).enumerate().map(|(i, d)| format!("{:02x}", if i % b == 3 { 0 } else { d * 255 >> 8 })).collect::<String>();
        prop_assert_eq!(rows(&full, "dd rt"), rows(&plain, "dd rt").iter().map(strip).collect::<Vec<String>>());
        prop_assert_eq!(rows(&none, "dd rt"), rows(&before, "dd rt").iter().map(strip).collect::<Vec<String>>());
    }

    /// a byte poked through Lock is what a dump then prints, and nothing else changes
    #[test]
    fn poke_reads_back(f in prop::sample::select(vec!["8", "565", "24", "32"]), w in 1u32..9, h in 1u32..6, seed in 1u32..999, y in 0u32..6, x in 0u32..36, v: u8) {
        prop_assume!(y < h);
        let b = match f { "8" => 1, "565" => 2, "24" => 3, _ => 4 };
        prop_assume!(x < w * b);
        let base = format!("dd\nddsurf 0 {w} {h} {f}\nddfill 0 {seed}\n");
        let before = run(&format!("{base}dddump 0\n"));
        let after = run(&format!("{base}ddlock 0 all\nddpoke 0 {x} {y} {v:02x}\nddunlock 0 none\ndddump 0\n"));
        let mut want = rows(&before, "dd 0");
        let r = &mut want[y as usize];
        r.replace_range(2 * x as usize..2 * x as usize + 2, &format!("{v:02x}"));
        prop_assert_eq!(rows(&after, "dd 0"), want);
    }

    /// bytes poked into a row's padding are what the 8 bpp backward copy's extra pixels take
    /// from there (DDRAW.LOCK.PADDING, DDRAW.OVERLAP8): the first extra pixel of row y is the
    /// padding byte dx before the end of row y - 1
    #[test]
    fn padding_reads_back(w in 29i64..32, dx in 1i64..4, v in prop::collection::vec(any::<u8>(), 18)) {
        let pokes: String = (0..6).map(|y| format!("ddpoke 0 {w} {y} {}\n", (w..32).map(|c| format!("{:02x}", v[(y * 3 + c - w) as usize])).collect::<String>())).collect();
        let out = run(&format!("dd\nddsurf 0 {w} 6 8\nddfill 0 7\nddlock 0 all\n{pokes}ddunlock 0 none\nddblt 0 {dx} 2 {} 4 0 0 2 4 4\ndddump 0\n", dx + 4));
        let r = rows(&out, "dd 0");
        for y in 2..4usize {
            // (the byte at address y·32 − dx: row y − 1, column 32 − dx)
            let c = 32 - dx;
            let want = if c >= w { v[((y as i64 - 1) * 3 + c - w) as usize] } else { u8::from_str_radix(&rows(&run(&format!("dd\nddsurf 0 {w} 6 8\nddfill 0 7\ndddump 0\n")), "dd 0")[y - 1][2 * c as usize..2 * c as usize + 2], 16).unwrap() };
            prop_assert_eq!(&r[y][0..2], format!("{want:02x}"));
        }
    }

    /// flipping a chain of n buffers n times brings every buffer's contents back
    #[test]
    fn flips_rotate(n in 1usize..4, seed in 1u32..999) {
        let mut text = format!("dd\nddchain 0 {n} 4 2 offscreen 3ddevice sysmem\n");
        for i in 0..=n {
            text += &format!("ddfill {i} {}\n", seed + i as u32);
        }
        let dumps: String = (0..=n).map(|i| format!("dddump {i}\n")).collect();
        let before = run(&format!("{text}{dumps}"));
        let flips = "ddflip 0 - novsync\n".repeat(n + 1);
        let after = run(&format!("{text}{flips}{dumps}"));
        prop_assert_eq!(before.iter().filter(|l| !l.starts_with("flip")).collect::<Vec<_>>(), after.iter().filter(|l| !l.starts_with("flip")).collect::<Vec<_>>());
        let one = run(&format!("{text}ddflip 0 - novsync\n{dumps}"));
        prop_assert_ne!(&before, &one);
    }

    /// a clip list reads back as given, with its bounding box
    #[test]
    fn clip_lists_round_trip(r in prop::collection::vec((-9i32..20, -9i32..20, 1i32..9, 1i32..9), 1..6)) {
        let rects: Vec<[i32; 4]> = r.iter().map(|&(x, y, w, h)| [x, y, x + w, y + h]).collect();
        let flat = rects.iter().flatten().map(|v| v.to_string()).collect::<Vec<_>>().join(" ");
        let out = run(&format!("dd\nddsurf 0 8 8 32\nddclip 0 {flat}\nddcliplist 0\n"));
        let b = [rects.iter().map(|r| r[0]).min().unwrap(), rects.iter().map(|r| r[1]).min().unwrap(), rects.iter().map(|r| r[2]).max().unwrap(), rects.iter().map(|r| r[3]).max().unwrap()];
        let bound = b.map(|v| v.to_string()).join(" ");
        prop_assert!(out.contains(&format!("cliplist {} {bound} : {flat}", rects.len())), "{:?}", out);
    }

    /// palette entries set are read back (8 bits, ALLOW256: no reserved entries)
    #[test]
    fn palette_entries_round_trip(start in 0u32..256, n in 1u32..64, seed in 1u32..999) {
        let n = n.min(256 - start);
        let out = run(&format!("dd\nddpal 0 7 8bit allow256\nddpalset 0 {start} {n} {seed}\nddpalget 0 {start} {n}\n"));
        let e = pal_entries(seed);
        let want: String = e[..n as usize].iter().map(|p| p.iter().map(|x| format!("{x:02x}")).collect::<String>()).collect();
        prop_assert!(out.iter().any(|l| l.split(' ').next_back() == Some(want.as_str())), "{:?}", out);
    }

    /// whatever text comes in, parsing and the models never panic
    #[test]
    fn never_panics(text in "(case [a-z]{1,3}\n|end\n|(dd|ddsurf|ddfill|dddump|ddclip|ddcliplist|ddpal|ddpalget|ddpalset|ddpalcaps|ddsetpal|ddgetpal|ddgetdc|ddreldc|dcpix|dcget|dcrect|dcbpp|ddlock|ddpoke|ddunlock|dddesc|ddcoop|ddchain|ddflip|ddblt|ddcolor|ddfast|d3d|d3denum|d3ddev|d3drs|d3dclear|d3dtri|d3dfill|d3ddump|d3dzdump)( (-?[0-9]{1,4}|0x[0-9a-f]{1,8}|#[0-9a-f]{6}|all|none|empty|get|set|z|rgb|hal|ref|565|32|24|8|z16|z32|primary|offscreen|3ddevice|sysmem|vid|8bit|allow256|readonly|-)){0,9}\n){0,40}") {
        let cases = parse_cases(&text);
        for c in &cases {
            let _ = run_case(c);
        }
        let out = model::run("x", &cases);
        prop_assert_eq!(parse_output(&out).cases.len(), cases.len());
        for l in text.lines() {
            let _ = tokens(l);
        }
    }
}

/// the models answer every case of these suites but the few the open clauses leave (as the
/// Windows comparison skips unmodeled cases, a lost answer would hide there)
#[test]
fn suites_stay_modeled() {
    use kikizake::generators;
    for (s, n32, n64) in [("ddraw", 0, 0), ("ddclip", 0, 0), ("ddpal", 0, 0), ("dddc", 0, 0), ("dd8", 0, 0), ("ddlock", 0, 0), ("ddflip", 0, 0), ("d3d7", 0, 0), ("d3d7dev", 44, 0), ("d3d7shade", 17, 0)] {
        let cases = parse_cases(&generators::suite(s).unwrap());
        for (arch, n) in [(32, n32), (64, n64)] {
            model::set_arch(arch);
            let res: Vec<_> = cases.iter().map(run_case).collect();
            let un = res.iter().filter(|r| matches!(r, Err(Fail::Unmodeled(_)))).count();
            assert_eq!(un, n, "{s}-{arch}");
            // (a generated line the model calls bad would be skipped as well)
            assert!(res.iter().all(|r| !matches!(r, Err(Fail::Bad | Fail::Empty))), "{s}-{arch}: a bad line");
        }
    }
    model::set_arch(32);
}

proptest! {
    /// a stretch's source index stays in the source, never goes back, takes every source index
    /// when it grows, and is the identity at equal counts, on every format and mirroring
    #[test]
    fn stretch_indices_are_monotone_and_cover(bpp in prop::sample::select(vec![8u32, 16, 24, 32]), mirrored: bool, rows: bool, s in 1i64..40, d in 1i64..60) {
        use kikizake::model::ddraw::{Axis, Stretch};
        let rule = Stretch::of(bpp, mirrored);
        let axis = if rows { Axis::Y } else { Axis::X };
        let ix: Vec<i64> = (0..d).map(|x| rule.source(axis, x, s, d)).collect();
        prop_assert!(ix.iter().all(|&i| (0..s).contains(&i)));
        prop_assert!(ix.windows(2).all(|w| w[0] <= w[1] && w[1] - w[0] <= (s + d - 1) / d));
        if d >= s {
            prop_assert_eq!(ix.iter().copied().collect::<std::collections::BTreeSet<_>>().len() as i64, s);
        }
        if d == s {
            prop_assert!(ix.iter().enumerate().all(|(x, &i)| i == x as i64));
        }
    }
}

proptest! {
    /// the entry a color goes to on an 8 bpp DC is no farther than any other, a color the table
    /// holds goes to the first entry of it, and the system palette maps each of its own colors
    /// back to itself
    #[test]
    fn nearest_entry_is_nearest(seed: u32, c in 0u32..0x100_0000, i in 0usize..256) {
        use kikizake::model::ddraw::{nearest_entry, SYSTEM_PALETTE};
        let tab: Vec<u32> = pal_entries(seed).iter().map(|e| u32::from(e[0]) << 16 | u32::from(e[1]) << 8 | u32::from(e[2])).collect();
        let d = |e: u32| -> u32 { [16u32, 8, 0].iter().map(|&s| (e >> s & 255).abs_diff(c >> s & 255).pow(2)).sum() };
        let n = usize::from(nearest_entry(&tab, c));
        prop_assert!(tab.iter().all(|&e| d(tab[n]) <= d(e)));
        let first = tab.iter().position(|&e| e == tab[i]).unwrap();
        prop_assert_eq!(usize::from(nearest_entry(&tab, tab[i])), first);
        prop_assert_eq!(usize::from(nearest_entry(&SYSTEM_PALETTE, SYSTEM_PALETTE[i])), i);
    }
}

/// the eight compares: each the negation of its opposite, Never and Always constant, every pair
/// of 0..=40 values
#[test]
fn compares_are_consistent() {
    use kikizake::model::d3d7::Cmp;
    for a in 0..=40u32 {
        for b in 0..=40u32 {
            let p = |c: Cmp| c.passes(a, b);
            assert!(!p(Cmp::Never) && p(Cmp::Always));
            assert_eq!(p(Cmp::Less), !p(Cmp::GreaterEqual));
            assert_eq!(p(Cmp::Greater), !p(Cmp::LessEqual));
            assert_eq!(p(Cmp::Equal), !p(Cmp::NotEqual));
            assert_eq!(p(Cmp::LessEqual), p(Cmp::Less) || p(Cmp::Equal));
        }
    }
    for v in 0..=9u32 {
        assert_eq!(Cmp::parse(v).is_some(), (1..=8).contains(&v));
    }
}

/// dithering: never below the truncated value nor more than one step above it, never past the
/// largest value, monotone in the value at each position, and 0 and 255 kept, for every value,
/// position and width
#[test]
fn dither_stays_within_a_step() {
    use kikizake::model::d3d7::dither;
    for bits in [5u32, 6] {
        for y in 0..4 {
            for x in 0..4 {
                let mut prev = 0;
                for v in 0..=255u32 {
                    let q = dither(v, bits, x, y);
                    let t = v >> (8 - bits);
                    assert!(q >= t && q <= t + 1 && q < 1 << bits, "{bits} {x} {y} {v}");
                    assert!(q >= prev);
                    prev = q;
                }
                assert_eq!(dither(0, bits, x, y), 0);
                assert_eq!(dither(255, bits, x, y), (1 << bits) - 1);
            }
        }
    }
}

proptest! {
    /// blend factors: ZERO/ONE pick a side exactly (software ONE is 256ths, the HAL's 1), the BOTH
    /// factors as the source sum to one whole, and every software factor stays in 0..=256
    #[test]
    fn blend_factors_bound(sf in 1u32..=13, df in 1u32..=13, s in 0u32..256, d in 0u32..256, a in 0u32..256) {
        use kikizake::model::d3d7::{factors_hw, factors_soft, Factor};
        let f = (Factor::parse(sf).unwrap(), Factor::parse(df).unwrap());
        let (fs, fd) = factors_soft(f, s, d, a);
        prop_assert!(fs <= 256 && fd <= 256);
        if matches!(f.0, Factor::BothSrcAlpha | Factor::BothInvSrcAlpha) {
            prop_assert_eq!(fs + fd, 255);
        }
        prop_assert_eq!(factors_soft((Factor::One, Factor::Zero), s, d, a), (256, 0));
        let (hs, hd) = factors_hw(f, f64::from(s) / 255.0, f64::from(d) / 255.0, f64::from(a) / 255.0);
        prop_assert!((0.0..=1.0).contains(&hs) && (0.0..=1.0).contains(&hd));
    }
}

/// a short edge's tie (D3D7.TRI.TIE) by where its f32 walk lies: f32 rounds to nearest, so the
/// edge mirrored about x = 0 lies on the other side by as much, and the two never both move a
/// pixel; a slope and first-row x exact in f32 (a rise of a power of two pixels and a top on
/// whole sixteenths) leave the edge on the pixel; for every edge of a small domain, rows 1 to 3
/// after its first
#[test]
fn ties_round_symmetrically() {
    let mut moved = 0;
    for tx in (-24..=24).step_by(5) {
        for ty in 0..20 {
            let first = (ty + 15) / 16;
            for dy in 1..48 {
                for dx in -48..=48 {
                    let (t, b) = ((tx, ty), (tx + dx, ty + dy));
                    let (tm, bm) = ((-tx, ty), (-tx - dx, ty + dy));
                    for k in 1..4 {
                        let (Some(p), Some(q)) = (f32_past(t, b, first, k, SlopeRound::Nearest), f32_past(tm, bm, first, k, SlopeRound::Nearest)) else { continue };
                        assert!(!(p && q), "{t:?} {b:?} row {k}");
                        if [16, 32].contains(&dy) {
                            assert!(!p && !q, "{t:?} {b:?} row {k}: exact in f32");
                        }
                        moved += usize::from(p || q);
                    }
                }
            }
        }
    }
    // (most slopes are not exact in f32: some edges move)
    assert!(moved > 1000, "{moved}");
}

/// a triangle with its bottom vertex on a pixel of row 1 (D3D7.TRI.EDGE): its top on row 0 off a
/// pixel, its middle on either side; row 1, where both edges meet at the bottom vertex, is empty
/// on both soft drawers
#[test]
fn one_row_tops_leave_row_1_empty() {
    let mut n = 0;
    for dev in ["rgb", "mmx"] {
        for tx in (1..176).step_by(7).filter(|x| x % 16 != 0) {
            for px in 0..=10 {
                for my in [3, 8, 13] {
                    for off in [-40, 40] {
                        let bx = 16 * px;
                        let mx = tx + (bx - tx) * my / 16 + off;
                        if !(0..=176).contains(&mx) {
                            continue;
                        }
                        let out = try_run(&format!("d3d 7\nd3ddev {dev} 565 12 4\nd3drs set 22 1\nd3dclear 0xff000000\nd3dtri 0xffffffff {tx} 0 {bx} 16 {mx} {my}\nd3ddump\n")).unwrap();
                        let r = rows(&out, "dd rt");
                        assert!(r[1].chars().all(|c| c == '0'), "{dev} ({tx},0) ({bx},16) ({mx},{my}): {}", r[1]);
                        n += 1;
                    }
                }
            }
        }
    }
    assert!(n > 1000, "{n}");
}

/// the long edge's tie (D3D7.TRI.TIE, D3D7.TRI.EDGE): its f32 slope rounds toward zero, so an
/// edge whose first-row x is exact (a top on a row) and whose slope is positive never lies past
/// the pixel, the edge mirrored about x = 0 never past when the edge is, and a slope exact in f32
/// leaves both on it; for every edge of a small domain, rows 1 to 3 after its first
#[test]
fn long_ties_round_toward_zero() {
    let (mut moved, mut checked) = (0, 0);
    for tx in (-24..=24).step_by(5) {
        for ty in 0..20 {
            let first = (ty + 15) / 16;
            for dy in 1..48 {
                for dx in -48..=48 {
                    let (t, b) = ((tx, ty), (tx + dx, ty + dy));
                    let (tm, bm) = ((-tx, ty), (-tx - dx, ty + dy));
                    for k in 1..4 {
                        let (Some(p), Some(q)) = (f32_past(t, b, first, k, SlopeRound::TowardZero), f32_past(tm, bm, first, k, SlopeRound::TowardZero)) else { continue };
                        checked += 1;
                        assert!(!(p && q), "{t:?} {b:?} row {k}");
                        if ty == first * 16 && dx > 0 {
                            assert!(!p, "{t:?} {b:?} row {k}: a slope cut toward zero");
                        }
                        if [16, 32].contains(&dy) {
                            assert!(!p && !q, "{t:?} {b:?} row {k}: exact in f32");
                        }
                        moved += usize::from(p || q);
                    }
                }
            }
        }
    }
    // (a negative slope cut toward zero lies past: some edges move)
    assert!(moved > 1000 && checked > 100_000, "{moved} {checked}");
}

/// D3D7.LINE: a line (|dx| >= |dy|, or the other way) draws exactly one pixel in each whole
/// column of its major axis strictly between its ends, the same one walked either way but at an
/// exact tie, where the two ways' f32 walks may lie side by side; LASTPIXEL off draws the same
/// pixels but at most the last one's; for every line between two points of a grid of fifths of a
/// pixel up to (5, 5) on a 6 x 6 target, on the RGB and ramp rasterizers
#[test]
fn lines_cross_each_column_once() {
    let (w, h) = (6i64, 6i64);
    let pts: Vec<(i64, i64)> = (0..17).flat_map(|i| (0..17).map(move |j| (i * 5, j * 5))).collect();
    let (mut drawn, mut ties) = (0, 0);
    for family in [Family::Soft { mmx: false }, Family::Ramp] {
        for &a in &pts {
            for &b in &pts {
                let (dx, dy) = (b.0 - a.0, b.1 - a.1);
                let (Ok(on), Ok(off), Ok(back)) = (line(family, [a, b], true, w, h), line(family, [a, b], false, w, h), line(family, [b, a], true, w, h)) else { continue };
                drawn += 1;
                // off draws a subset of on, missing at most one pixel
                assert!(on.iter().zip(&off).all(|(&n, &f)| n || !f), "{a:?} {b:?}");
                assert!(on.iter().zip(&off).filter(|(n, f)| n != f).count() <= 1, "{a:?} {b:?}");
                if dx == 0 && dy == 0 {
                    assert!(on.iter().all(|&c| !c));
                    continue;
                }
                let xmajor = dx.abs() >= dy.abs();
                let (lo, hi) = if xmajor { (a.0.min(b.0), a.0.max(b.0)) } else { (a.1.min(b.1), a.1.max(b.1)) };
                for m in 0..if xmajor { w } else { h } {
                    if !(lo < m * 16 && m * 16 < hi) {
                        continue;
                    }
                    let cells = |c: &[bool]| -> Vec<i64> {
                        (0..if xmajor { h } else { w })
                            .filter(|&n| {
                                let (x, y) = if xmajor { (m, n) } else { (n, m) };
                                c[(y * w + x) as usize]
                            })
                            .collect()
                    };
                    // (an end's pixel may share the column with the line's); the two ways meet
                    // but at an exact tie (v + 7/16 a whole pixel), where they lie side by side
                    let (one, two) = (cells(&on), cells(&back));
                    let mid: Vec<i64> = one.iter().copied().filter(|n| two.contains(n)).collect();
                    let ((a0, b0), (a1, b1)) = if xmajor { (a, b) } else { ((a.1, a.0), (b.1, b.0)) };
                    let tie = ((b0 + 7) * (a1 - a0) + (m * 16 - a0) * (b1 - b0)).rem_euclid(16 * (a1 - a0)) == 0;
                    let near = one.iter().any(|p| two.iter().any(|q| (p - q).abs() == 1));
                    assert!(one.len() <= 2 && (!mid.is_empty() || (tie && near)), "{a:?} {b:?} column {m}: {one:?} {two:?}");
                    ties += usize::from(tie && mid.is_empty());
                }
            }
        }
    }
    // (exact ties are walked too, and some go apart)
    assert!(drawn > 100_000 && ties > 100, "{drawn} {ties}");
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 2048, ..ProptestConfig::default() })]

    /// D3D7.GOURAUD (software): at the span's anchor the 8.8 color is the floor of 256 · the
    /// exact color; each step across strays from the exact color by at most 1/256 more (the
    /// step is the exact one cut toward zero); one color at every vertex is that color, whole,
    /// everywhere
    #[test]
    fn gouraud_steps_from_the_anchor(v in prop::array::uniform3((0i64..1024, 0i64..256)), col: [u8; 3], py in 0i64..16, a in 0i64..64, px in 0i64..64) {
        let den = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
        prop_assume!(den != 0);
        let exact = |x: i64| -> f64 {
            let (x, y) = (x as f64 * 16.0, py as f64 * 16.0);
            let p = |i: usize| (v[i].0 as f64, v[i].1 as f64);
            let ((x0, y0), (x1, y1), (x2, y2)) = (p(0), p(1), p(2));
            let d = den as f64;
            let l1 = ((x - x0) * (y2 - y0) - (x2 - x0) * (y - y0)) / d;
            let l2 = ((x1 - x0) * (y - y0) - (x - x0) * (y1 - y0)) / d;
            let c = |i: usize| f64::from(col[i]);
            c(0) + l1 * (c(1) - c(0)) + l2 * (c(2) - c(0))
        };
        let at = |x: i64| f64::from(gouraud_channel(v, col, x, py, a).raw());
        let e = 256.0 * exact(a);
        if (0.0..65535.0).contains(&e) {
            prop_assert!((at(a) - e.floor()).abs() <= 1.0, "{} {}", at(a), e);
        }
        let (ex, k) = (256.0 * exact(px), (px - a).abs() as f64);
        if (0.0..65535.0).contains(&ex) && (0.0..65535.0).contains(&e) {
            prop_assert!((at(px) - ex).abs() <= k + 2.0, "{} {} {}", at(px), ex, k);
        }
        let one = gouraud_channel(v, [col[0]; 3], px, py, a);
        prop_assert_eq!(one.raw(), u32::from(col[0]) << 8);
    }
}
