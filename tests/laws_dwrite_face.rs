//! The DirectWrite face's answers (spec/dwrite.txt DW.FILE.*, DW.NOFACE, DW.INFO, DW.GLYPH.*,
//! DW.GDI.INVALID, DW.SHAPE.*, DW.LAYOUT.NAME) over the whole domain each law names: every
//! signature, every glyph index, every BMP character, every table and pair of tables removed
//! from the generated fonts (kind exhaustive).
mod pairs;
use pairs::zip_eq;
use kikizake::generators::dwrite::{font, CHARS};
use kikizake::ttf::{analyze, assemble, glyph_of, parse, tables_of, FileType};

/// the lines the model answers for a case of these lines
fn run(lines: &str) -> Vec<String> {
    let cases = kikizake::case::parse_cases(&format!("case c\n{lines}\nend\n"));
    kikizake::model::run_case(&cases[0]).unwrap()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// the line of `out` that starts with `p`
fn line<'a>(out: &'a [String], p: &str) -> &'a str {
    out.iter().find(|l| l.starts_with(p)).map_or("", |l| l.as_str())
}

/// DW.FILE.ANALYZE, DW.FILE.OTHER: Analyze by the signature, for every first 32 bits of a
/// 28-byte file (count fields 1): 00010000 and "true" TrueType, "OTTO" CFF, "ttcf" a
/// collection of 1, any other Unknown (the spec, DW.FILE.OTHER)
#[test]
fn analyze_is_the_signature_for_every_word() {
    let n = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16);
    let n = u64::try_from(n).unwrap_or(4);
    let chunk = (1u64 << 32) / n;
    let bad = std::thread::scope(|s| {
        let hs: Vec<_> = (0..n)
            .map(|k| {
                s.spawn(move || {
                    let hi = if k + 1 == n { 1u64 << 32 } else { (k + 1) * chunk };
                    (k * chunk..hi).filter_map(|v| u32::try_from(v).ok()).find(|&w| {
                        let mut f = [0u8; 28];
                        f[..4].copy_from_slice(&w.to_be_bytes());
                        f[5] = 1;
                        f[11] = 1;
                        let want = match &w.to_be_bytes() {
                            [0, 1, 0, 0] | b"true" => FileType::TrueType,
                            b"OTTO" => FileType::Cff,
                            b"ttcf" => FileType::Collection(1),
                            _ => FileType::Unknown,
                        };
                        analyze(&f) != want
                    })
                })
            })
            .collect();
        hs.into_iter().filter_map(|h| h.join().unwrap()).min()
    });
    assert_eq!(bad, None);
}

/// DW.FILE.OTHER: each signature needs its header and table directory whole (12 + 16 n bytes
/// for n tables, a collection 12 + 4 n for n fonts, n at least 1), for every count 0..65535
/// at the lengths either side of whole; then a face's analyze line is the type's numbers
#[test]
fn analyze_needs_the_directory_for_every_count() {
    for sig in [[0u8, 1, 0, 0], *b"true", *b"OTTO", *b"ttcf"] {
        for n in 0..=65535u32 {
            let ttc = &sig == b"ttcf";
            let whole = if ttc { 12 + 4 * n } else { 12 + 16 * n };
            for len in [whole.saturating_sub(1), whole] {
                let mut f = vec![0u8; usize::try_from(len.max(12)).unwrap()];
                f[..4].copy_from_slice(&sig);
                if ttc {
                    f[8..12].copy_from_slice(&n.to_be_bytes());
                } else {
                    f[4..6].copy_from_slice(&u16::try_from(n).unwrap().to_be_bytes());
                }
                let ok = f.len() >= usize::try_from(whole).unwrap() && (!ttc || n >= 1);
                let want = match (ok, &sig) {
                    (false, _) => FileType::Unknown,
                    (true, b"OTTO") => FileType::Cff,
                    (true, b"ttcf") => FileType::Collection(n),
                    (true, _) => FileType::TrueType,
                };
                assert_eq!(analyze(&f), want, "{sig:?} {n} {}", f.len());
            }
        }
    }
    let names = [(FileType::TrueType, "supported 1 file 2 face 1 faces 1"), (FileType::Cff, "supported 1 file 1 face 0 faces 1"), (FileType::Unknown, "supported 0 file 0 face 6 faces 0")];
    let base = font(0).bytes();
    for (sig, (_, want)) in zip_eq([[0u8, 1, 0, 0], *b"OTTO", *b"wOFF"].iter(), names) {
        let mut b = base.clone();
        b[..4].copy_from_slice(sig);
        let out = run(&format!("dwfont {}", hex(&b)));
        assert_eq!(out[0], format!("dwfont analyze hr 00000000 {want}"));
    }
    for k in 1..=3u32 {
        let fonts: Vec<Vec<u8>> = (0..k).map(|i| font(i).bytes()).collect();
        let out = run(&format!("dwfont {}", hex(&kikizake::ttf::ttc(&fonts))));
        assert_eq!(out[0], format!("dwfont analyze hr 00000000 supported 1 file 3 face 2 faces {k}"));
    }
}

/// DW.FILE.ANALYZE: every generated font is a supported TrueType file of one face
#[test]
fn fonts_are_one_truetype_face() {
    for seed in 0..64 {
        let out = run(&format!("dwfont {}", hex(&font(seed).bytes())));
        assert_eq!(out, ["dwfont analyze hr 00000000 supported 1 file 2 face 1 faces 1", "dwfont face hr 00000000"], "{seed}");
    }
}

/// the face ops, each with arguments it takes
const FACE_OPS: [&str; 10] = [
    "dwinfo",
    "dwmetrics",
    "dwgdimetrics 12 1",
    "dwcmap 65",
    "dwglyph 0 0",
    "dwgdiglyph 12 1 0 0 0",
    "dwshape 12 65",
    "dwrun aliased 12 0 40 1 0",
    "dwfallback 65",
    "dwlayout 20 4000 1 65",
];

/// DW.NOFACE: every face op before a font is loaded, and after a file that is no font, prints
/// "<op> noface" and nothing else
#[test]
fn ops_without_a_face_say_noface() {
    for op in FACE_OPS {
        let name = op.split(' ').next().unwrap();
        assert_eq!(run(op), [format!("{name} noface")], "{op}");
        let out = run(&format!("dwfont 00000000\n{op}"));
        assert_eq!(out.last().unwrap(), &format!("{name} noface"), "{op}");
        let mut b = font(0).bytes();
        b.truncate(40);
        let out = run(&format!("dwfont {}\n{op}", hex(&b)));
        assert_eq!(out.last().unwrap(), &format!("{name} noface"), "{op}");
    }
}

/// DW.INFO: glyph count maxp's, type TRUETYPE, index 0, no simulations, symbol where the cmap
/// read is (3,0) (a font with (3,10) or (3,1) as well is not symbol), monospaced post's isFixedPitch, no vertical variants: every generated font, symbol
/// and not
#[test]
fn info_is_the_tables() {
    for seed in 0..64 {
        for symbol in [false, true] {
            let mut f = font(seed);
            f.symbol = symbol;
            let p = parse(&f.bytes()).unwrap();
            let out = run(&format!("dwfont {}\ndwinfo", hex(&f.bytes())));
            // (the (3, 0) subtable is the one read only where there is no (3, 10) one)
            let read = symbol && !f.cmap.iter().any(|c| c.0 > 0xffff);
            let want = format!("dwinfo glyphs {} type 1 index 0 sim 0 symbol {} mono {} vert 0", f.glyphs.len(), u8::from(read), u8::from(f.fixed_pitch));
            assert_eq!(out[2], want, "{seed}");
            assert_eq!(usize::from(p.num_glyphs), f.glyphs.len());
        }
    }
}

/// DW.GLYPH.OUTSIDE, DW.GLYPH.SIDEWAYS: for every glyph index 0..65535 on 16 fonts, an index
/// past the font's glyphs has glyph 0's metrics (S_OK), and isSideways changes nothing
#[test]
fn glyphs_outside_are_glyph_zero_and_sideways_the_same() {
    let all: String = (0..=65535u32).map(|g| format!(" {g}")).collect();
    for seed in 0..16 {
        let f = font(seed);
        let h = hex(&f.bytes());
        let out = run(&format!("dwfont {h}\ndwglyph 0{all}\ndwglyph 1{all}"));
        let (up, side) = (&out[3..3 + 65536], &out[3 + 65536 + 1..]);
        assert_eq!(out[2], "dwglyph hr 00000000");
        assert_eq!(up, side, "{seed}");
        for (g, l) in up.iter().enumerate().skip(f.glyphs.len()) {
            assert_eq!(l, &up[0], "{seed} {g}");
        }
    }
}

/// DW.GDI.INVALID: an em size or pixels per dip at most 0 (every pair -4..4 with either so):
/// E_INVALIDARG and nothing else, from both calls
#[test]
fn gdi_sizes_at_most_zero_are_invalid() {
    let h = hex(&font(0).bytes());
    for em in -4..=4 {
        for ppd in -4..=4 {
            if em > 0 && ppd > 0 {
                continue;
            }
            let out = run(&format!("dwfont {h}\ndwgdimetrics {em} {ppd}\ndwgdiglyph {em} {ppd} 0 0 0 1"));
            assert_eq!(out[2..], ["dwgdimetrics hr 80070057", "dwgdiglyph hr 80070057"], "{em} {ppd}");
        }
    }
}

/// DW.SHAPE.PROPS: for every BMP character, text properties 4 (5 for a space), glyph
/// properties 112 (114 for a space and a no-break space)
#[test]
fn shaping_properties_for_every_bmp_character() {
    let h = hex(&font(0).bytes());
    for block in (0..=0xffffu32).collect::<Vec<_>>().chunks(4096) {
        let text: String = block.iter().map(|c| format!(" {c}")).collect();
        let out = run(&format!("dwfont {h}\ndwshape 12{text}"));
        let text_props: String = block.iter().map(|&c| if c == 0x20 { " 5" } else { " 4" }).collect();
        let glyph_props: String = block.iter().map(|&c| if matches!(c, 0x20 | 0xa0) { " 114" } else { " 112" }).collect();
        assert_eq!(line(&out, "textprops"), format!("textprops{text_props}"));
        assert_eq!(line(&out, "glyphprops"), format!("glyphprops{glyph_props}"));
    }
}

/// DW.SHAPE.PLACE: the advance hmtx's times em / upm in f32, offsets 0, for every character the
/// fonts map, 16 fonts, every em 1..200
#[test]
fn placements_are_the_scaled_advance() {
    for seed in 0..16 {
        let f = font(seed);
        let p = parse(&f.bytes()).unwrap();
        let h = hex(&f.bytes());
        let cs: Vec<u32> = CHARS.iter().copied().filter(|&c| c < 0x10000).collect();
        let text: String = cs.iter().map(|c| format!(" {c}")).collect();
        let mut lines = format!("dwfont {h}");
        for em in 1..=200 {
            lines += &format!("\ndwshape {em}{text}");
        }
        let out = run(&lines);
        let places: Vec<&String> = out.iter().filter(|l| l.starts_with("place ")).collect();
        for (em, l) in zip_eq(1..=200u16, places) {
            let s = f32::from(em) / f32::from(p.upm);
            let want: String = cs.iter().map(|&c| format!(" {:08x}/00000000/00000000", (f32::from(p.hmtx[usize::from(glyph_of(&p, c))].0) * s).to_bits())).collect();
            assert_eq!(l, &format!("place hr 00000000{want}"), "{seed} {em}");
        }
    }
}

/// DW.LAYOUT.NAME: a font without a family name (its name table removed, 16 fonts): the
/// collection has 1 family, the layout fails with E_FAIL, and so does the fallback
#[test]
fn a_font_without_a_family_name_has_no_layout() {
    for seed in 0..16 {
        let t: Vec<_> = tables_of(&font(seed).bytes()).into_iter().filter(|x| &x.0 != b"name").collect();
        let h = hex(&assemble(t));
        let out = run(&format!("dwfont {h}\ndwlayout 20 4000 1 65\ndwlayoutnf 20 4000 1 65\ndwfallback 65"));
        assert_eq!(out[2..], ["dwlayout families 1 hr 80004005 name", "dwlayout families 1 hr 80004005 name", "dwfallback hr 80004005"], "{seed}");
    }
}

/// DW.FILE.OTHER: every table and every pair of tables removed from 8 fonts (but vhea and
/// vmtx, of which the clause says nothing): without maxp no
/// face (DWRITE_E_FILEFORMAT, then noface); without head 2048 units per em and an empty box;
/// without hhea or hmtx no glyphs (glyph metrics and runs E_INVALIDARG, every character glyph
/// 0); without head, loca or glyf no outlines (design boxes empty at the left side bearing,
/// GDI compatible metrics all 0); without cmap every character glyph 0; without post the
/// underline a tenth of the em below
#[test]
fn tables_removed_are_what_the_face_lacks() {
    let probe = "dwmetrics\ndwcmap 65 97\ndwglyph 0 1 2\ndwgdiglyph 12 1 0 0 1\ndwrun aliased 12 0 40 1 0";
    for seed in 0..8 {
        let f = font(seed);
        let full = parse(&f.bytes()).unwrap();
        // (the tables the clause names; vhea without vmtx is no case's)
        let tags: Vec<[u8; 4]> = tables_of(&f.bytes()).into_iter().map(|x| x.0).filter(|t| !matches!(t, b"vhea" | b"vmtx")).collect();
        let mut sets: Vec<Vec<[u8; 4]>> = tags.iter().map(|&t| vec![t]).collect();
        for (i, &a) in tags.iter().enumerate() {
            sets.extend(tags[i + 1..].iter().map(|&b| vec![a, b]));
        }
        for gone in sets {
            let has = |t: &[u8; 4]| !gone.contains(t);
            let t: Vec<_> = tables_of(&f.bytes()).into_iter().filter(|x| has(&x.0)).collect();
            let out = run(&format!("dwfont {}\n{probe}", hex(&assemble(t))));
            let what = format!("{seed} without {:?}", gone.iter().map(|t| String::from_utf8_lossy(t).into_owned()).collect::<Vec<_>>());
            if !has(b"maxp") {
                assert_eq!(out[1], "dwfont face hr 88985000", "{what}");
                assert!(out[2..].iter().all(|l| l.ends_with(" noface")), "{what}");
                continue;
            }
            assert_eq!(out[1], "dwfont face hr 00000000", "{what}");
            let upm = if has(b"head") { full.upm } else { 2048 };
            let m = line(&out, "dwmetrics upm");
            assert!(m.starts_with(&format!("dwmetrics upm {upm} ")), "{what}: {m}");
            if !has(b"head") {
                assert!(line(&out, "dwmetrics box").starts_with("dwmetrics box 0 0 0 0 "), "{what}");
            }
            if !has(b"post") {
                assert!(m.contains(&format!(" ulpos {} ", -i32::from(upm / 10))), "{what}: {m}");
            }
            let glyphs = has(b"hhea") && has(b"hmtx");
            if !glyphs || !has(b"cmap") {
                assert_eq!(line(&out, "dwcmap"), "dwcmap 0 0", "{what}");
            }
            if !glyphs {
                assert_eq!(line(&out, "dwglyph"), "dwglyph hr 80070057", "{what}");
                assert_eq!(line(&out, "dwrun"), "dwrun hr 80070057", "{what}");
                continue;
            }
            if !(has(b"head") && has(b"loca") && has(b"glyf")) {
                let at = out.iter().position(|l| l == "dwgdiglyph hr 00000000").expect(&what);
                assert!(out[at + 1..at + 2].iter().all(|l| l == "glyph adv 0 lsb 0 rsb 0 tsb 0 advh 0 bsb 0 voy 0"), "{what}");
                let at = out.iter().position(|l| l == "dwglyph hr 00000000").expect(&what);
                for (k, l) in out[at + 1..at + 3].iter().enumerate() {
                    let (adv, lsb) = full.hmtx[k + 1];
                    let want = format!("glyph adv {adv} lsb {lsb} rsb {} ", i32::from(adv) - i32::from(lsb));
                    assert!(l.starts_with(&want), "{what}: {l}");
                }
            }
        }
    }
}
