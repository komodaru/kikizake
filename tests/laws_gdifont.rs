//! What GDI measures of a TrueType font the suite writes (spec/gditext.txt GDI.TEXT.MEMFONT,
//! FACE, TM, AVE, WIDTH, ABC, ABCEMPTY, EXTENT, EXTRAEXTENT), exhaustive over a grid of fonts
//! (units per em, with and without OS/2, signed and unsigned win descents, fixed and
//! proportional), heights, weights and character extras, against the clauses computed from
//! the fields the writer is given (not from the file the model reads back), halves rounded up.
use kikizake::case::parse_cases;
use kikizake::model::run_case;
use kikizake::ttf::{Font, Glyph};

/// n / d rounded half up, d > 0
fn rnd(n: i64, d: i64) -> i64 {
    (2 * n + d).div_euclid(2 * d)
}

fn glyph(adv: i64, rects: &[(i64, i64, i64, i64)], u: i64) -> Glyph {
    let s = |v: i64| i16::try_from(v * u / 1000).unwrap();
    Glyph { adv: u16::try_from(adv * u / 1000).unwrap(), rects: rects.iter().map(|r| (s(r.0), s(r.1), s(r.2), s(r.3))).collect(), vert: (0, 0) }
}

/// the fonts: glyph 0 and 'A'..'D' with outlines (B of no advance, C wholly above the
/// ascent), the space without one
fn fonts() -> Vec<Font> {
    let mut v = Vec::new();
    for upm in [1000u16, 2048, 256] {
        let u = i64::from(upm);
        for os2 in [None, Some(4u16)] {
            for (win, fixed) in [((900u16, 300u16), false), ((1100, 65500), true), ((800, 250), true)] {
                let sc = |x: i64| i16::try_from(x * u / 1000).unwrap();
                let scu = |x: u16| if x > 32767 { x } else { u16::try_from(i64::from(x) * u / 1000).unwrap() };
                v.push(Font {
                    upm,
                    hhea: (sc(880), sc(-240), sc(if fixed { 0 } else { 90 })),
                    os2,
                    typo: (0, 0, 0),
                    win: (scu(win.0), scu(win.1)),
                    fs_selection: 0,
                    x_height: 0,
                    cap_height: 0,
                    strike: (0, 0),
                    underline: (0, 0),
                    script: [0; 8],
                    fixed_pitch: fixed,
                    symbol: false,
                    vertical: false,
                    family: format!("Kz Law {upm} {}", v.len()),
                    glyphs: vec![
                        glyph(500, &[(50, 0, 450, 700)], u),
                        glyph(600, &[(30, -100, 570, 650)], u),
                        glyph(0, &[(0, 0, 100, 100)], u),
                        glyph(450, &[(10, 1200, 200, 1300)], u),
                        glyph(777, &[(0, -200, 300, 0), (250, 100, 701, 900)], u),
                        glyph(250, &[], u),
                    ],
                    cmap: vec![(0x20, 5), (0x41, 1), (0x42, 2), (0x43, 3), (0x44, 4)],
                });
            }
        }
    }
    v
}

/// the clauses' numbers for one font and request
struct Want {
    /// the scale num / den
    num: i64,
    den: i64,
    ppem: i64,
    a: i64,
    d: i64,
    bold: i64,
}

fn bbox(g: &Glyph) -> Option<(i64, i64, i64, i64)> {
    let r = &g.rects;
    Some((
        i64::from(r.iter().map(|q| q.0).min()?),
        i64::from(r.iter().map(|q| q.1).min()?),
        i64::from(r.iter().map(|q| q.2).max()?),
        i64::from(r.iter().map(|q| q.3).max()?),
    ))
}

/// head's box: the union of the glyphs' boxes
fn head(f: &Font) -> (i64, i64, i64, i64) {
    let b: Vec<_> = f.glyphs.iter().filter_map(bbox).collect();
    (b.iter().map(|q| q.0).min().unwrap(), b.iter().map(|q| q.1).min().unwrap(), b.iter().map(|q| q.2).max().unwrap(), b.iter().map(|q| q.3).max().unwrap())
}

/// GDI.TEXT.TM: lfHeight 0 is -16; A and D usWinAscent and the signed usWinDescent (no OS/2:
/// head's yMax and -yMin), the cell A + D (no OS/2: hhea's ascender - descender); the scale
/// -lfHeight / upm below 0, lfHeight / cell above
fn want(f: &Font, height: i64, bold: bool) -> Want {
    let upm = i64::from(f.upm);
    let hb = head(f);
    let (a, d, cell) = match f.os2 {
        Some(_) => {
            let (a, d) = (i64::from(f.win.0), i64::from(f.win.1 as i16));
            (a, d, a + d)
        }
        None => (hb.3, -hb.1, i64::from(f.hhea.0) - i64::from(f.hhea.1)),
    };
    let h = if height == 0 { -16 } else { height };
    let (num, den) = if h < 0 { (-h, upm) } else { (h, cell) };
    Want { num, den, ppem: rnd(num * upm, den), a, d, bold: i64::from(bold) }
}

fn tm_line(f: &Font, w: &Want, italic: bool) -> String {
    let px = |v: i64| rnd(v * w.num, w.den);
    let (asc, desc) = (px(w.a), px(w.d).max(0));
    let hgt = asc + desc;
    let gap = i64::from(f.hhea.2);
    let e = match f.os2 {
        Some(_) => gap - (w.a + w.d - (i64::from(f.hhea.0) - i64::from(f.hhea.1))),
        None => gap,
    };
    let hb = head(f);
    let bw = hb.2 - hb.0;
    // GDI.TEXT.AVE: xAvgCharWidth (the writer's: the advances' mean, rounded down); 0: half of
    // head's width; no OS/2: two thirds of it
    let n = i64::try_from(f.glyphs.len()).unwrap();
    let avg = f.glyphs.iter().map(|g| i64::from(g.adv)).sum::<i64>() / n;
    let ave = match f.os2 {
        Some(_) if avg != 0 => px(avg),
        Some(_) => rnd(bw * w.num, 2 * w.den),
        None => rnd(2 * bw * w.num, 3 * w.den),
    } + w.bold;
    let pitch = match (f.fixed_pitch, f.os2.is_some()) {
        (false, _) => 7,
        (true, true) => 6,
        (true, false) => 0x36,
    };
    format!(
        "tm 1 h {hgt} asc {asc} desc {desc} int {} ext {} ave {ave} max {} weight {} over 0 dig 96 96 chars 30 255 31 32 it {} ul 0 st 0 pitch {pitch} charset 0",
        hgt - w.ppem,
        px(e.max(0)),
        px(bw) + w.bold,
        if w.bold == 1 { 700 } else { 400 },
        if italic { 255 } else { 0 }
    )
}

fn glyph_of(f: &Font, c: u32) -> &Glyph {
    let g = f.cmap.iter().find(|e| e.0 == c).map_or(0, |e| e.1);
    &f.glyphs[usize::from(g)]
}

/// GDI.TEXT.WIDTH: round(advance s), + 1 when bold and the advance is not 0
fn width(f: &Font, w: &Want, c: u32) -> i64 {
    let adv = i64::from(glyph_of(f, c).adv);
    rnd(adv * w.num, w.den) + if adv > 0 { w.bold } else { 0 }
}

/// GDI.TEXT.ABC, GDI.TEXT.ABCEMPTY
fn abc(f: &Font, w: &Want, c: u32) -> (i64, i64, i64) {
    let wd = width(f, w, c);
    let (x0, y0, x1, y1) = bbox(glyph_of(f, c)).unwrap();
    if y1 < -w.d || y0 > w.a {
        return (0, 1, wd - 1);
    }
    let upm = i64::from(f.upm);
    // (the edges on the rounded ppem in 1/64 pixel, halves up; the left rounded half down to a
    // pixel, the right half up)
    let (l, r) = (rnd(x0 * w.ppem * 64, upm), rnd(x1 * w.ppem * 64, upm));
    let a = -(-(2 * l - 64)).div_euclid(128);
    let b = ((r + 32).div_euclid(64) - a).max(1) + w.bold;
    (a, b, wd - a - b)
}

/// GDI.TEXT.EXTRAEXTENT: a character's width with the extra
fn extra_width(wd: i64, extra: i64) -> i64 {
    if extra >= 0 { wd + extra } else if wd == 0 { 0 } else { (wd + extra).max(1) }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// every font, height, weight and extra: the case's answers against the clauses'
#[test]
fn font_measures_are_the_clauses() {
    let texts = ["AB_C", "DDAx", "B", "A_D_C_B", "CCCC"];
    let mut cases = 0;
    for f in fonts() {
        let file = hex(&f.bytes());
        for height in [0, -12, -16, -33, 10, 25] {
            for bold in [false, true] {
                for extra in [0, 2, -3, -40] {
                    let w = want(&f, height, bold);
                    let mut calls = vec![format!("memfont {file}"), "dib 0 8 8 32".into(), format!("font 0 {height} 0 0 {} 0 3 {}", if bold { 700 } else { 400 }, f.family)];
                    // GDI.TEXT.MEMFONT, GDI.TEXT.FACE
                    let mut lines = vec!["memfont 1".to_string(), format!("face {} {}", f.family.chars().count(), f.family.replace(' ', "_"))];
                    calls.push("face 0".into());
                    calls.push("metrics 0".into());
                    lines.push(tm_line(&f, &w, false));
                    calls.push("widths 0 32 72".into());
                    lines.push(format!("widths 1 {}", (32..=72).map(|c| width(&f, &w, c).to_string()).collect::<Vec<_>>().join(" ")));
                    calls.push("abc 0 65 69".into());
                    lines.push(format!("abc 1 {}", (65..=69).map(|c| { let (a, b, c) = abc(&f, &w, c); format!("{a},{b},{c}") }).collect::<Vec<_>>().join(" ")));
                    calls.push(format!("extra 0 {extra}"));
                    lines.push("extra 0".into());
                    let hgt = rnd(w.a * w.num, w.den) + rnd(w.d * w.num, w.den).max(0);
                    for t in texts {
                        let cs: Vec<u32> = t.bytes().map(|b| if b == b'_' { 0x20 } else { u32::from(b) }).collect();
                        let run: Vec<i64> = cs.iter().scan(0, |x, &c| { *x += extra_width(width(&f, &w, c), extra); Some(*x) }).collect();
                        let total = *run.last().unwrap();
                        calls.push(format!("extent 0 {t}"));
                        lines.push(format!("extent 1 {total} {hgt}"));
                        for max in [0, total / 2, total] {
                            let fit: Vec<i64> = run.iter().copied().take_while(|v| *v <= max).collect();
                            calls.push(format!("extentex 0 {max} {t}"));
                            lines.push(format!("extentex 1 {} {total} {hgt}{}", fit.len(), fit.iter().map(|v| format!(" {v}")).collect::<String>()));
                        }
                    }
                    let text = format!("case font\n{}\nend\n", calls.join("\n"));
                    let out = run_case(&parse_cases(&text)[0]).unwrap_or_else(|e| panic!("{text}: {e:?}"));
                    assert_eq!(out, lines, "{} height {height} bold {bold} extra {extra}", f.family);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 18 * 6 * 2 * 4);
}
