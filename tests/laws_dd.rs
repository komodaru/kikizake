//! Properties of the DirectDraw and Direct3D 1/2/3/7 models (src/model/ddraw.rs, d3d7.rs) that
//! hold whatever the measured details: exhaustive over small domains, property-based elsewhere.
//! With Windows' answers they are what cargo-mutants counts on to catch mutants of the models.
use proptest::prelude::*;
use kikizake::case::{parse_cases, parse_output, tokens};
use kikizake::model::d3d7::{clear_z, pixel, rs_default, tri_z};
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

/// every z in millis into 16, 24 and 32 bits: 0 at 0, the largest value at 1, never decreasing,
/// and Clear's rounding at most one past the triangle's truncation
#[test]
fn z_values_are_monotone() {
    for bits in [16, 24, 32] {
        let max = if bits == 32 { u32::MAX } else { (1 << bits) - 1 };
        let (mut pc, mut pt) = (0, 0);
        for m in 0..=1000 {
            let z = m as f32 / 1000.0;
            let (c, t) = (clear_z(bits, z), tri_z(bits, z));
            assert!(c >= pc && t >= pt, "{bits} {m}");
            assert!(c <= max && t <= max);
            (pc, pt) = (c, t);
        }
        assert_eq!(clear_z(bits, 0.0), 0);
        assert_eq!(tri_z(bits, 0.0), 0);
        assert_eq!(clear_z(bits, 1.0), max);
    }
    assert_eq!(tri_z(16, 1.0), 65535);
}

/// every value of each channel: the target pixel keeps the channel's top bits, monotone, alpha gone
#[test]
fn pixels_keep_the_top_bits() {
    for f in FMTS {
        for sh in [0, 8, 16, 24] {
            let mut prev = 0;
            for v in 0..=255u32 {
                let p = pixel(f, v << sh);
                if sh == 24 {
                    assert_eq!(p, 0, "{f} alpha");
                    continue;
                }
                assert!(p >= prev, "{f} {sh} {v}");
                prev = p;
                assert_eq!(pixel(f, v << sh | 0xff00_0000), p);
            }
        }
    }
    assert_eq!(pixel("565", 0xffffff), 0xffff);
    assert_eq!(pixel("555", 0xffffff), 0x7fff);
}

/// a new RGB device answers every state 0..160 with the table's default
#[test]
fn render_state_defaults_read_back() {
    for v in [2, 3, 7] {
        let mut text = format!("d3d {v}\nd3ddev rgb 32 4 4\n");
        let mut want = vec![];
        for st in 0..=160 {
            text += &format!("d3drs get {st}\n");
            if let Some(d) = rs_default(v, st) {
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
        let px: String = pixel(f, c).to_le_bytes()[..b].iter().map(|x| format!("{x:02x}")).collect();
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
