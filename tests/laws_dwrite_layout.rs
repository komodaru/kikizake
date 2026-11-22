//! IDWriteTextLayout's lines and clusters without fallback (spec/dwrite.txt DW.LAYOUT.LINE,
//! WRAP, EMERGENCY, CLUSTERS, DEPTH) over every text of up to 5 characters of an alphabet of
//! letters, a space, a hyphen, a no-break space, a digit, Hebrew and a supplementary character,
//! at 4 line widths, wrapping and not, in 4 fonts (kind exhaustive). The breaks and levels are
//! the line break and bidi analyses' (laws_dwrite.rs), the face's metrics Metrics.lean's.
use kikizake::generators::dwrite::font;
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

/// a layout's lines as the clauses say: (the face, em, line width in quarters, wrapping, text)
fn layout(p: &kikizake::ttf::Parsed, em: u16, mw4: u16, wrap: bool, cps: &[u32]) -> Vec<String> {
    let m = metrics(p);
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
    let scale = f32::from(em) / f32::from(m.upm);
    let adv: Vec<f32> = cps.iter().map(|&c| f32::from(p.hmtx[usize::from(glyph_of(p, c))].0) * scale).collect();
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
    // DW.LAYOUT.LINE: baseline (ascent + lineGap) em / upm, height baseline + descent em / upm
    let base = (f32::from(m.asc) + f32::from(m.gap)) * f32::from(em) / f32::from(m.upm);
    let lh = base + f32::from(m.desc) * f32::from(em) / f32::from(m.upm);
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
    let height = lines.iter().fold(0.0f32, |a, _| a + lh);
    let mut out = vec![format!("metrics hr 00000000 left {} top 00000000 width {} wtrail {} height {} lines {} depth {depth}", b(-0.0), b(w), b(wt), b(height), lines.len())];
    for &(s, e) in &lines {
        let units: usize = cps[s..e].iter().map(|&c| len(c)).sum();
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
                    assert_eq!(got, layout(&p, em, mw4, wrap, t), "{seed} {em} {mw4} {wrap} {t:x?}");
                }
                assert!(out.iter().filter(|l| l.starts_with("dwlayout families")).all(|l| l == &format!("dwlayout families 1 hr 00000000 name{name}")));
            }
        }
    }
}
