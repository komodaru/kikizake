//! Laws of the DirectWrite model (src/model/dwrite): the font writer and parser agree, and the
//! text analyses keep the invariants UAX #9 / #14 / #24 and the API's output shape promise,
//! whatever Windows' measured details are. Exhaustive over small alphabets, proptest elsewhere.
use proptest::prelude::*;
use kikizake::generators::dwrite::{font, BIDI_CHARS, BREAK_CHARS, CHARS, SCRIPT_CHARS};
use kikizake::model::dwrite::{bidi, gdi_round, linebreak, raster, script};
use kikizake::ttf::{glyph_of, parse};

proptest! {
    /// what the writer puts in a font is what the parser reads back
    #[test]
    fn ttf_round_trip(seed in any::<u32>()) {
        let f = font(seed);
        let p = parse(&f.bytes()).expect("parses");
        prop_assert_eq!(p.upm, f.upm);
        prop_assert_eq!(p.num_glyphs as usize, f.glyphs.len());
        prop_assert_eq!(&p.family, &f.family);
        prop_assert_eq!(p.os2.as_ref().map(|o| o.version), f.os2);
        prop_assert_eq!(p.hhea, Some(f.hhea));
        prop_assert_eq!(p.vmtx.is_some(), f.vertical);
        for (g, fg) in f.glyphs.iter().enumerate() {
            prop_assert_eq!(p.hmtx[g].0, fg.adv);
            let n = p.glyphs[g].as_ref().map_or(0, |x| x.1.len());
            prop_assert_eq!(n, fg.rects.len());
        }
        for &(c, g) in &f.cmap {
            prop_assert_eq!(glyph_of(&p, c), g);
        }
        // an unmapped character is glyph 0
        prop_assert_eq!(glyph_of(&p, 0x10fffd), 0);
    }

    /// translating a glyph by whole pixels translates its aliased pixels
    #[test]
    fn aliased_translates(x0 in -300i64..300, y0 in -300i64..300, w in 64i64..400, h in 64i64..400, dx in -5i64..5, dy in -5i64..5) {
        let r = [(x0, y0, x0 + w, y0 + h)];
        let t = [(x0 + dx * 64, y0 + dy * 64, x0 + w + dx * 64, y0 + h + dy * 64)];
        let a = raster::aliased(&r, raster::Dropout::On).unwrap();
        let b = raster::aliased(&t, raster::Dropout::On).unwrap();
        prop_assert_eq!(b, a.iter().map(|&(x, y)| (x + dx, y + dy)).collect());
    }

    /// a rectangle at least a pixel wide lights exactly the pixels whose centers it holds
    #[test]
    fn aliased_is_centers(x0 in -300i64..300, y0 in -300i64..300, w in 64i64..400, h in 64i64..400) {
        let a = raster::aliased(&[(x0, y0, x0 + w, y0 + h)], raster::Dropout::On).unwrap();
        let inside = |c: i64, lo: i64, hi: i64| lo <= c * 64 + 32 && c * 64 + 32 <= hi;
        for y in y0.div_euclid(64) - 1..=(y0 + h).div_euclid(64) + 1 {
            for x in x0.div_euclid(64) - 1..=(x0 + w).div_euclid(64) + 1 {
                prop_assert_eq!(a.contains(&(x, y)), inside(x, x0, x0 + w) && inside(y, y0, y0 + h));
            }
        }
    }

    /// GDI rounding lands on the pixel grid: rounding twice is rounding once, and it keeps order
    #[test]
    fn gdi_round_grid(upm in prop::sample::select(vec![16u16, 256, 500, 1000, 1024, 2000, 2048, 4096]), ppem in 1i64..240, a in -4096i32..4096, b in -4096i32..4096) {
        let r = gdi_round(upm, ppem);
        prop_assert!((r(a) - a).abs() as f32 <= upm as f32 / ppem as f32 / 2.0 + 1.0);
        if a <= b {
            prop_assert!(r(a) <= r(b));
        }
    }
}

fn bidi_class(c: u32) -> bidi::Class {
    bidi::class(c)
}

/// every string of up to 4 of the classes, both directions: one level per character; strong
/// characters at their own parity (I1, I2); levels within paragraph + 2; separators and
/// trailing whitespace at the paragraph level (L1)
#[test]
fn bidi_levels_exhaustive() {
    use bidi::Class::*;
    let cs: Vec<bidi::Class> = BIDI_CHARS.iter().map(|c| bidi_class(c.0)).collect();
    let k = cs.len();
    for n in 1..=4u32 {
        for idx in 0..(k as u32).pow(n) {
            let t: Vec<_> = (0..n).map(|j| cs[(idx / (k as u32).pow(j)) as usize % k]).collect();
            for para in [0u8, 1] {
                let lv = bidi::analyze(&t, para).1;
                assert_eq!(lv.len(), t.len());
                for (i, (&c, &l)) in t.iter().zip(&lv).enumerate() {
                    assert!(l >= para && l <= para + 2, "{t:?} {para} {lv:?}");
                    match c {
                        L => assert_eq!(l % 2, 0, "{t:?} {para} {lv:?}"),
                        R | AL => assert_eq!(l % 2, 1, "{t:?} {para} {lv:?}"),
                        S | B => assert_eq!(l, para, "{t:?} {para} {lv:?}"),
                        _ => {}
                    }
                    if t[i..].iter().all(|c| matches!(c, WS | S | B)) {
                        assert_eq!(l, para, "trailing {t:?} {para} {lv:?}");
                    }
                }
            }
        }
    }
}

/// every pair and triple of the classes: one entry per unit, the first unit MAY_NOT before,
/// the last CAN or MUST after, and each boundary told the same from both sides
#[test]
fn breaks_exhaustive() {
    let k = BREAK_CHARS.len();
    for n in 1..=3u32 {
        for idx in 0..(k as u32).pow(n) {
            let t: Vec<u32> = (0..n).map(|j| BREAK_CHARS[(idx / (k as u32).pow(j)) as usize % k]).collect();
            let b = linebreak::breaks(&t);
            assert_eq!(b.len(), t.len());
            assert_eq!(b[0].0, 2, "{t:x?} {b:?}");
            assert!(matches!(b[t.len() - 1].1, 1 | 3), "{t:x?} {b:?}");
            for i in 1..t.len() {
                assert_eq!(b[i - 1].1, b[i].0, "{t:x?} {b:?}");
            }
            for (i, &c) in t.iter().enumerate() {
                assert_eq!(b[i].2 == 1, matches!(c, 0x9..=0xd | 0x20 | 0x85 | 0xa0 | 0x2028 | 0x2029), "{c:#x}");
                // a mandatory break follows a hard line break and nothing else
                if i + 1 < t.len() {
                    assert_eq!(b[i].1 == 3, matches!(c, 0xa | 0xb | 0xc | 0xd | 0x85 | 0x2028 | 0x2029), "{t:x?} {b:?}");
                }
            }
        }
    }
}

/// every string of up to 3 of the characters: the runs tile the text in UTF-16 units, no two
/// neighbours share a script, and each strong character lies in a run of its script
#[test]
fn script_runs_exhaustive() {
    let k = SCRIPT_CHARS.len();
    for n in 1..=3u32 {
        for idx in 0..(k as u32).pow(n) {
            let t: Vec<u32> = (0..n).map(|j| SCRIPT_CHARS[(idx / (k as u32).pow(j)) as usize % k]).collect();
            let runs = script::runs(&t).unwrap();
            let units: usize = t.iter().map(|&c| if c >= 0x10000 { 2 } else { 1 }).sum();
            let mut pos = 0;
            for r in &runs {
                assert_eq!(r.0, pos, "{t:x?} {runs:?}");
                assert!(r.1 > 0);
                pos += r.1;
            }
            assert_eq!(pos, units, "{t:x?} {runs:?}");
            for w in runs.windows(2) {
                assert_ne!(w[0].2, w[1].2, "{t:x?} {runs:?}");
            }
            let mut u = 0;
            for &c in &t {
                if let script::Kind::Script(s) = script::kind(c).unwrap() {
                    let r = runs.iter().find(|r| r.0 <= u && u < r.0 + r.1).unwrap();
                    assert_eq!(r.2, s, "{t:x?} {runs:?}");
                }
                u += if c >= 0x10000 { 2 } else { 1 };
            }
        }
    }
}

/// the fonts map each of CHARS to its glyph
#[test]
fn fonts_map_chars() {
    for seed in 0..64 {
        let f = font(seed);
        let p = parse(&f.bytes()).unwrap();
        for (i, &c) in CHARS.iter().enumerate() {
            if c < 0x10000 || f.cmap.iter().any(|m| m.0 == c) {
                assert_eq!(glyph_of(&p, c) as usize, i + 1);
            }
        }
    }
}

proptest! {
    /// the parser takes any bytes (a cut or damaged font): an answer or None, never a panic
    #[test]
    fn parse_any_bytes(seed in any::<u32>(), cut in any::<prop::sample::Index>(), flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..8)) {
        let mut b = font(seed).bytes();
        for (i, x) in flips {
            let i = i.index(b.len());
            b[i] = x;
        }
        let n = cut.index(b.len() + 1);
        let _ = parse(&b[..n]);
        let _ = parse(&b);
    }
}

proptest! {
    /// any of the suite's ops with any small numbers after a damaged font: the model answers
    /// (lines or unmodeled), never panics
    #[test]
    fn model_takes_any_line(seed in any::<u32>(), flips in prop::collection::vec((any::<prop::sample::Index>(), any::<u8>()), 0..4), op in 0usize..10, nums in prop::collection::vec(-70i64..70, 0..8)) {
        let mut b = font(seed).bytes();
        for (i, x) in flips {
            let i = i.index(b.len());
            b[i] = x;
        }
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        let names = ["dwinfo", "dwmetrics", "dwgdimetrics", "dwcmap", "dwglyph", "dwgdiglyph", "dwshape", "dwrun aliased", "dwrun gray", "dwlayout"];
        let args: Vec<String> = nums.iter().map(|n| n.to_string()).collect();
        let text = format!("case x\ndwfont {hex}\n{} {}\nend\n", names[op], args.join(" "));
        let cases = kikizake::case::parse_cases(&text);
        let _ = kikizake::model::run("dwrite", &cases);
    }
}

/// one character of each value a UCD table lists (the first it lists)
fn reps(t: &kikizake::ucd::Table) -> Vec<u32> {
    let mut seen: Vec<&str> = Vec::new();
    let mut v = Vec::new();
    for &(lo, _, val) in t.ranges() {
        let c = lo.max(1);
        if !seen.contains(&val) && !(0xd800..=0xdfff).contains(&c) && t.get(c) == Some(val) {
            seen.push(val);
            v.push(c);
        }
    }
    v
}

/// every triple of the bidi classes (a character of each), both directions: one level per
/// character; the characters X9 removes at the level before them; strong characters outside
/// embeddings at their own parity; separators and trailing whitespace at the paragraph level
#[test]
fn bidi_every_class_triple() {
    use bidi::Class::*;
    let cs: Vec<bidi::Class> = reps(kikizake::ucd::bidi_class()).into_iter().map(bidi::class).collect();
    assert_eq!(cs.len(), 23);
    for &a in &cs {
        for &b in &cs {
            for &c in &cs {
                let t = [a, b, c];
                for para in [0u8, 1] {
                    let (ex, lv) = bidi::analyze(&t, para);
                    assert_eq!((ex.len(), lv.len()), (3, 3));
                    let embedded = t.iter().any(|c| matches!(c, LRE | RLE | LRO | RLO));
                    for i in 0..3 {
                        assert!(lv[i] <= para + 6 && ex[i] <= para + 6 && lv[i] >= para, "{t:?} {para} {lv:?}");
                        match t[i] {
                            BN | LRE | RLE | LRO | RLO | PDF => {
                                let before = if i == 0 { para } else { lv[i - 1] };
                                let trailing = t[i..].iter().all(|c| matches!(c, WS | BN | LRE | RLE | LRO | RLO | PDF | S | B));
                                assert!(lv[i] == before || trailing, "{t:?} {para} {lv:?}");
                            }
                            L if !embedded => assert_eq!(lv[i] % 2, 0, "{t:?} {para} {lv:?}"),
                            R | AL if !embedded => assert_eq!(lv[i] % 2, 1, "{t:?} {para} {lv:?}"),
                            S | B => assert_eq!(lv[i], para),
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

/// every pair of the line break classes (a character of each), next to each other and around
/// a space: one entry per UTF-16 unit, each boundary told the same from both sides, MAY_NOT
/// inside a supplementary character, MUST only after a hard line break
#[test]
fn breaks_every_class_pair() {
    let cs = reps(kikizake::ucd::line_break());
    assert!(cs.len() >= 40);
    let hard = |c: u32| matches!(c, 0xa..=0xd | 0x85 | 0x2028 | 0x2029);
    for &a in &cs {
        for &b in &cs {
            for t in [vec![a, b], vec![a, 0x20, b]] {
                let br = linebreak::breaks(&t);
                let units: Vec<u32> = t.iter().flat_map(|&c| if c >= 0x10000 { vec![c, c] } else { vec![c] }).collect();
                assert_eq!(br.len(), units.len(), "{t:x?}");
                assert_eq!(br[0].0, 2);
                for i in 1..br.len() {
                    assert_eq!(br[i - 1].1, br[i].0, "{t:x?} {br:?}");
                    if br[i - 1].1 == 3 {
                        assert!(hard(units[i - 1]), "{t:x?} {br:?}");
                    }
                }
                for (i, &c) in t.iter().enumerate() {
                    if c >= 0x10000 {
                        let u = t[..i].iter().map(|&c| if c >= 0x10000 { 2 } else { 1 }).sum::<usize>();
                        assert_eq!(br[u].1, 2, "{t:x?} {br:?}");
                    }
                }
            }
        }
    }
}

/// a layout of characters the font maps all needs no fallback: it is modeled, in both the
/// system fallback and the no fallback variants
#[test]
fn layout_of_mapped_chars_is_modeled() {
    for seed in 0..16 {
        let f = font(seed);
        let hex: String = f.bytes().iter().map(|x| format!("{x:02x}")).collect();
        let p = parse(&f.bytes()).unwrap();
        let cs: Vec<String> = CHARS.iter().filter(|&&c| c < 0x10000 && glyph_of(&p, c) != 0).take(6).map(|c| c.to_string()).collect();
        assert!(!cs.is_empty());
        for op in ["dwlayout", "dwlayoutnf"] {
            let text = format!("case x\ndwfont {hex}\n{op} 20 4000 1 {}\nend\n", cs.join(" "));
            let out = kikizake::model::run("dwrite", &kikizake::case::parse_cases(&text));
            assert!(!out.contains("unmodeled"), "{op} seed {seed}: {out}");
        }
    }
}

/// a layout without fallback of a character the font does not map is modeled (as glyph 0),
/// and so not as the system fallback, which is open
#[test]
fn layout_without_fallback_of_unmapped_is_modeled() {
    let f = font(0);
    let hex: String = f.bytes().iter().map(|x| format!("{x:02x}")).collect();
    let p = parse(&f.bytes()).unwrap();
    let un = (0x100u32..0x10000).find(|&c| glyph_of(&p, c) == 0).unwrap();
    let text = format!("case x\ndwfont {hex}\ndwlayoutnf 20 4000 1 {un}\nend\n");
    let out = kikizake::model::run("dwrite", &kikizake::case::parse_cases(&text));
    assert!(out.contains("dwlayout families 1 hr 00000000"), "{out}");
}

/// UAX #9 X5 to X7: past the deepest level an embedding is counted, not entered, and its PDF
/// takes that count back before it pops a level; at depth 61
#[test]
fn bidi_overflow_counts() {
    use bidi::Class::*;
    // LRE to 2, then RLE and LRE in turn up to 60
    let mut t = vec![LRE];
    while t.len() < 59 {
        t.push(if t.len() % 2 == 1 { RLE } else { LRE });
    }
    let (ex, _) = bidi::analyze(&[t.clone(), vec![L]].concat(), 0);
    assert_eq!(ex[t.len()], 60);
    // an LRE overflows (62); an RLE after it would reach 61 but is counted too
    let mut u = t.clone();
    u.extend([LRE, RLE, L, PDF, L, PDF, L, PDF, L]);
    let (ex, _) = bidi::analyze(&u, 0);
    let n = t.len();
    assert_eq!((ex[n + 2], ex[n + 4], ex[n + 6], ex[n + 8]), (60, 60, 60, 59));
}

/// X10: a level run's sos and eos are of the higher of its level and its neighbours': a
/// neutral between two embedded runs takes their direction
#[test]
fn bidi_run_sos_from_neighbour() {
    use bidi::Class::*;
    let (_, lv) = bidi::analyze(&[RLE, L, PDF, ON, RLE, L, PDF], 0);
    assert_eq!(lv[3], 1);
}

/// lines the executor refuses (no text to shape, a run without a glyph or an advance) are its
/// "bad line N" and the case goes on; a supplementary character to shape is no line the
/// generators write
#[test]
fn malformed_lines_are_bad_lines() {
    let hex: String = font(0).bytes().iter().map(|x| format!("{x:02x}")).collect();
    for (line, want) in [("dwshape 12", "bad line 3\ndwinfo"), ("dwrun aliased 12 0 0", "bad line 3\ndwinfo"), ("dwrun aliased 12 0 0 1", "bad line 3\ndwinfo"), ("dwrun aliased 12 0 0 1 0 2", "bad line 3\ndwinfo"), ("dwshape 12 0x41 0x1f600", "unmodeled bad line")] {
        let text = format!("case x\ndwfont {hex}\n{line}\ndwinfo\nend\n");
        let out = kikizake::model::run("dwrite", &kikizake::case::parse_cases(&text));
        assert!(out.contains(want), "{line}: {out}");
    }
}
