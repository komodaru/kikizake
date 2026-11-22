//! IDWriteTextLayout's lines and clusters without fallback (spec/dwrite.txt DW.LAYOUT.LINE,
//! WRAP, EMERGENCY, CLUSTERS, DEPTH) over every text of up to 5 characters of an alphabet of
//! letters, a space, a hyphen, a no-break space, a digit, Hebrew and a supplementary character,
//! at 4 line widths, wrapping and not, in 4 fonts (kind exhaustive). The breaks and levels are
//! the line break and bidi analyses' (laws_dwrite.rs), the face's metrics Metrics.lean's.
use kikizake::generators::dwrite::font;
use kikizake::model::dwrite::fallback::{self, LineMetrics, Pick, Range};
use kikizake::model::dwrite::{bidi, linebreak, metrics};
use kikizake::ttf::{glyph_of, parse};

/// the characters the texts are of
const ALPHABET: [u32; 7] = [0x61, 0x20, 0x2d, 0xa0, 0x30, 0x5d0, 0x1f600];

/// every text of 1..=n of the characters
fn texts(n: u32) -> Vec<Vec<u32>> {
    let k = u32::try_from(ALPHABET.len()).unwrap();
    let mut v = Vec::new();
    for len in 1..=n {
        for idx in 0..k.pow(len) {
            v.push((0..len).map(|j| ALPHABET[usize::try_from(idx / k.pow(j) % k).unwrap()]).collect());
        }
    }
    v
}

fn b(v: f32) -> String {
    format!("{:08x}", v.to_bits())
}

/// each character's design advance and its font's metrics without fallback: the face's own
fn own(p: &kikizake::ttf::Parsed, cps: &[u32]) -> Vec<(u16, LineMetrics)> {
    let m = metrics(p);
    let lm = LineMetrics { upm: m.upm, asc: m.asc, desc: m.desc, gap: m.gap };
    cps.iter().map(|&c| (p.hmtx[usize::from(glyph_of(p, c))].0, lm)).collect()
}

/// a layout's lines as the clauses say: (em, line width in quarters, wrapping, text, each
/// character's design advance and font metrics)
fn layout(em: u16, mw4: u16, wrap: bool, cps: &[u32], fonts: &[(u16, LineMetrics)]) -> Vec<String> {
    let n = cps.len();
    let len = |c: u32| if c >= 0x10000 { 2usize } else { 1 };
    // (each character's break analysis: its first unit's, the break after it its last unit's)
    let bu = linebreak::breaks(cps);
    let mut br = Vec::new();
    let mut u = 0;
    for &c in cps {
        br.push((bu[u].1, bu[u + len(c) - 1].1, bu[u].2, bu[u].3));
        u += len(c);
    }
    // (DW.LAYOUT.FALLBACK: each advance by its own font's em / upm)
    let adv: Vec<f32> = fonts.iter().map(|&(a, m)| f32::from(a) * (f32::from(em) / f32::from(m.upm))).collect();
    let ws = |i: usize| br[i].2 != 0;
    // [s, e) less its trailing whitespace
    let trim = |s: usize, e: usize| (s..e).rev().find(|&k| !ws(k)).map_or(s, |k| k + 1);
    let width = |s: usize, e: usize| adv[s..trim(s, e)].iter().fold(0.0f32, |a, x| a + x);
    let maxw = f32::from(mw4) / 4.0;
    // DW.LAYOUT.WRAP: greedy, each line to the last CAN opportunity (or the end) whose width
    // without trailing whitespace fits; DW.LAYOUT.EMERGENCY: none fits: the most clusters that
    // fit, at least one, and the whitespace after them
    let mut lines = Vec::new();
    let mut s = 0;
    while s < n {
        let e = if !wrap {
            n
        } else if let Some(e) = (s + 1..=n).rev().find(|&e| (e == n || br[e - 1].1 == linebreak::CAN) && width(s, e) <= maxw) {
            e
        } else {
            let mut f = (s + 1..=n).rev().find(|&f| width(s, f) <= maxw).unwrap_or(s + 1).max(s + 1);
            while f < n && ws(f) {
                f += 1;
            }
            f
        };
        lines.push((s, e));
        s = e;
    }
    // DW.LAYOUT.LINE: baseline (ascent + lineGap) em / upm, height baseline + descent em / upm;
    // DW.LAYOUT.FALLBACK: a line the greatest baseline and descent of its characters' fonts
    let line = |s: usize, e: usize| {
        let base = fonts[s..e].iter().map(|&(_, m)| (f32::from(m.asc) + f32::from(m.gap)) * f32::from(em) / f32::from(m.upm)).fold(0.0f32, f32::max);
        let desc = fonts[s..e].iter().map(|&(_, m)| f32::from(m.desc) * f32::from(em) / f32::from(m.upm)).fold(0.0f32, f32::max);
        (base, base + desc)
    };
    let classes: Vec<bidi::Class> = cps.iter().map(|&c| bidi::class(c)).collect();
    let (_, mut lv) = bidi::analyze(&classes, 0);
    for &(s, e) in &lines {
        for k in trim(s, e)..e {
            lv[k] = 0;
        }
    }
    // DW.LAYOUT.DEPTH: the most runs of one level on a line
    let depth = lines.iter().map(|&(s, e)| 1 + lv[s..e].windows(2).filter(|w| w[0] != w[1]).count()).max().unwrap_or(1);
    let w = lines.iter().map(|&(s, e)| width(s, e)).fold(0.0f32, f32::max);
    let wt = lines.iter().map(|&(s, e)| adv[s..e].iter().fold(0.0f32, |a, x| a + x)).fold(0.0f32, f32::max);
    let height = lines.iter().fold(0.0f32, |a, &(s, e)| a + line(s, e).1);
    let mut out = vec![format!("metrics hr 00000000 left {} top 00000000 width {} wtrail {} height {} lines {} depth {depth}", b(-0.0), b(w), b(wt), b(height), lines.len())];
    for &(s, e) in &lines {
        let units: usize = cps[s..e].iter().map(|&c| len(c)).sum();
        let (base, lh) = line(s, e);
        out.push(format!("line {units} trail {} nl 0 height {} base {} trimmed 0", e - trim(s, e), b(lh), b(base)));
    }
    // DW.LAYOUT.CLUSTERS: width, UTF-16 length, canWrapLineAfter, isWhitespace, isNewline 0,
    // isSoftHyphen, isRightToLeft for an odd level
    let cl: String = (0..n).map(|i| format!(" {}/{}/{}{}0{}{}", b(adv[i]), len(cps[i]), u8::from(br[i].1 == linebreak::CAN), br[i].2, br[i].3, lv[i] % 2)).collect();
    out.push(format!("clusters hr 00000000 n {n}{cl}"));
    out
}

#[test]
fn layouts_are_the_lines_and_clusters() {
    let all = texts(5);
    for seed in [0u32, 3, 5, 9] {
        let f = font(seed);
        let p = parse(&f.bytes()).unwrap();
        let ok: Vec<&Vec<u32>> = all.iter().filter(|t| t.iter().all(|&c| glyph_of(&p, c) != 0)).collect();
        assert!(ok.len() > 1000, "{seed}");
        let hex: String = f.bytes().iter().map(|x| format!("{x:02x}")).collect();
        let name: String = p.family.encode_utf16().map(|u| format!(" {u}")).collect();
        for (em, mw4) in [(20u16, 1u16), (20, 90), (20, 300), (13, 4000)] {
            for wrap in [false, true] {
                let mut lines = format!("case c\ndwfont {hex}");
                for t in &ok {
                    lines += &format!("\ndwlayoutnf {em} {mw4} {}{}", u8::from(wrap), t.iter().map(|c| format!(" {c}")).collect::<String>());
                }
                lines += "\nend\n";
                let out = kikizake::model::run_case(&kikizake::case::parse_cases(&lines)[0]).unwrap();
                let mut it = out[2..].split(|l| l.starts_with("dwlayout families")).skip(1);
                assert!(out[2].starts_with("dwlayout families 1 hr 00000000"));
                for t in &ok {
                    let got = it.next().unwrap();
                    assert_eq!(got, layout(em, mw4, wrap, t, &own(&p, t)), "{seed} {em} {mw4} {wrap} {t:x?}");
                }
                assert!(out.iter().filter(|l| l.starts_with("dwlayout families")).all(|l| l == &format!("dwlayout families 1 hr 00000000 name{name}")));
            }
        }
    }
}

/// the characters of the fallback's table (DW.FALLBACK.FONTS): every one the cases name
const TABLE: [u32; 20] = [0x4e00, 0x3042, 0x30a2, 0xac00, 0x627, 0x628, 0x5d1, 0x915, 0xe01, 0x3b1, 0x430, 0x1f601, 0x2603, 0x2192, 0x378, 0xe000, 0x10fffd, 0x20ac, 0xe9, 0x7e];

/// DW.FALLBACK.FONTS: the table is the fallback's whole answer: for every code point the
/// lookup answers exactly the table's characters, each with its font's glyph and design advance
/// (or none), the fonts' names and metrics the machine's
#[test]
fn the_fallback_table_is_the_lookup() {
    use fallback::Sys::*;
    let want: [(u32, Option<(fallback::Sys, u16, u16)>); 20] = [
        (0x4e00, Some((YuGothicUi, 1202, 2048))),
        (0x3042, Some((YuGothicUi, 23644, 1671))),
        (0x30a2, Some((YuGothicUi, 23727, 1589))),
        (0xac00, Some((MalgunGothic, 636, 2048))),
        (0x627, Some((SegoeUi, 2317, 513))),
        (0x628, Some((SegoeUi, 2318, 1881))),
        (0x5d1, Some((SegoeUi, 2921, 1168))),
        (0x915, Some((NirmalaUi, 248, 1768))),
        (0xe01, Some((LeelawadeeUi, 251, 1175))),
        (0x3b1, Some((SegoeUi, 303, 1258))),
        (0x430, Some((SegoeUi, 519, 1042))),
        (0x1f601, Some((SegoeUiEmoji, 2267, 2812))),
        (0x2603, Some((SegoeUiEmoji, 3869, 2812))),
        (0x2192, Some((SegoeUi, 2862, 1767))),
        (0x378, None),
        (0xe000, None),
        (0x10fffd, None),
        (0x20ac, Some((SegoeUi, 189, 1104))),
        (0xe9, Some((SegoeUi, 112, 1071))),
        (0x7e, Some((SegoeUi, 97, 1401))),
    ];
    for c in 0..0x11_0000u32 {
        let got = fallback::sys(c).map(|x| x.1);
        let w = want.iter().find(|w| w.0 == c).map(|w| w.1);
        assert_eq!(got, w, "{c:x}");
    }
    let names = [(SegoeUi, "Segoe UI"), (SegoeUiEmoji, "Segoe UI Emoji"), (YuGothicUi, "Yu Gothic UI"), (NirmalaUi, "Nirmala UI"), (MalgunGothic, "Malgun Gothic"), (LeelawadeeUi, "Leelawadee UI")];
    for (s, n) in names {
        assert_eq!(s.name(), n);
        let m = s.metrics();
        let ad = if s == MalgunGothic { (2229, 495) } else { (2210, 514) };
        assert_eq!((m.upm, m.asc, m.desc, m.gap), (2048, ad.0, ad.1, 0), "{n}");
    }
}

/// a character's piece key: the own font's, or the table's font and range
fn key(p: &kikizake::ttf::Parsed, c: u32) -> (Pick, Option<Range>) {
    if glyph_of(p, c) != 0 {
        return (Pick::Own, None);
    }
    match fallback::sys(c).unwrap() {
        (r, Some((s, _, _))) => (Pick::Sys(s), Some(r)),
        (r, None) => (Pick::None, Some(r)),
    }
}

/// DW.FALLBACK.MAP: every text of up to 3 of the table's characters and 3 the font maps (a
/// space, A and a): a mapped character is the own font's; another the table's font, or none; a
/// piece ends where the font changes and, among the machine's fonts and none, where the
/// table's range changes; each piece's glyphs the font's
#[test]
fn the_fallback_pieces_for_every_short_text() {
    let f = font(0x0fe1_0000);
    let p = parse(&f.bytes()).unwrap();
    let chars: Vec<u32> = TABLE.iter().copied().chain([0x20, 0x41, 0x61]).collect();
    let k = u32::try_from(chars.len()).unwrap();
    for len in 1..=3u32 {
        for idx in 0..k.pow(len) {
            let t: Vec<u32> = (0..len).map(|j| chars[usize::try_from(idx / k.pow(j) % k).unwrap()]).collect();
            let got = fallback::pieces(&p, &t).unwrap();
            let mut want: Vec<(Pick, Vec<u32>)> = Vec::new();
            let mut last = None;
            for &c in &t {
                let kc = key(&p, c);
                match want.last_mut() {
                    Some(w) if last == Some(kc) => w.1.push(c),
                    _ => want.push((kc.0, vec![c])),
                }
                last = Some(kc);
            }
            assert_eq!(got.iter().map(|x| (x.pick, x.cps.clone())).collect::<Vec<_>>(), want, "{t:x?}");
            for pc in &got {
                for (&c, &(g, a)) in pc.cps.iter().zip(&pc.glyphs) {
                    let w = match pc.pick {
                        Pick::Own => (glyph_of(&p, c), p.hmtx[usize::from(glyph_of(&p, c))].0),
                        Pick::Sys(_) => fallback::sys(c).unwrap().1.map(|x| (x.1, x.2)).unwrap(),
                        Pick::None => (0, p.hmtx[0].0),
                    };
                    assert_eq!((g, a), w, "{t:x?} {c:x}");
                }
            }
        }
    }
}

/// DW.LAYOUT.FALLBACK: every text of up to 3 of the table's characters, a space and A, at 2
/// line widths, wrapping and not: each character in the font the fallback picks (none: the
/// own font's glyph 0), its advance by that font's em / upm, a line the greatest baseline and
/// descent of its characters' fonts
#[test]
fn fallback_layouts_take_each_fonts_metrics() {
    let f = font(0x0fe1_0000);
    let p = parse(&f.bytes()).unwrap();
    let m = metrics(&p);
    let ownm = LineMetrics { upm: m.upm, asc: m.asc, desc: m.desc, gap: m.gap };
    let hex: String = f.bytes().iter().map(|x| format!("{x:02x}")).collect();
    let chars: Vec<u32> = TABLE.iter().copied().chain([0x20, 0x41]).collect();
    let k = u32::try_from(chars.len()).unwrap();
    let mut texts = Vec::new();
    for len in 1..=3u32 {
        for idx in 0..k.pow(len) {
            texts.push((0..len).map(|j| chars[usize::try_from(idx / k.pow(j) % k).unwrap()]).collect::<Vec<u32>>());
        }
    }
    for (em, mw4) in [(20u16, 90u16), (13, 4000)] {
        for wrap in [false, true] {
            let mut lines = format!("case c\ndwfont {hex}");
            for t in &texts {
                lines += &format!("\ndwlayout {em} {mw4} {}{}", u8::from(wrap), t.iter().map(|c| format!(" {c}")).collect::<String>());
            }
            lines += "\nend\n";
            let out = kikizake::model::run_case(&kikizake::case::parse_cases(&lines)[0]).unwrap();
            let mut it = out[2..].split(|l| l.starts_with("dwlayout families")).skip(1);
            for t in &texts {
                let fonts: Vec<(u16, LineMetrics)> = t
                    .iter()
                    .map(|&c| match key(&p, c).0 {
                        Pick::Own => (p.hmtx[usize::from(glyph_of(&p, c))].0, ownm),
                        Pick::Sys(s) => (fallback::sys(c).unwrap().1.unwrap().2, s.metrics()),
                        Pick::None => (p.hmtx[0].0, ownm),
                    })
                    .collect();
                assert_eq!(it.next().unwrap(), layout(em, mw4, wrap, t, &fonts), "{em} {mw4} {wrap} {t:x?}");
            }
        }
    }
}
