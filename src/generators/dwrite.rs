//! The dwrite suite's case generator: small TrueType fonts made here (src/ttf.rs, glyphs of
//! rectangles, every metric field varied), their faces' metrics, shaping, rasterization and
//! layout, and text analysis over small alphabets (exhaustive) and random strings.
use super::*;
use crate::ttf::{Font, Glyph};

/// the characters the fonts map, in glyph order from glyph 1 (glyph 0 is .notdef)
pub const CHARS: &[u32] = &[0x41, 0x42, 0x43, 0x44, 0x45, 0x61, 0x62, 0x20, 0x30, 0x31, 0x2d, 0xa0, 0x5d0, 0x1f600];

fn rect(r: &mut XorShift32, upm: i64) -> (i16, i16, i16, i16) {
    let x0 = r.range(-upm / 8, upm / 2);
    let y0 = r.range(-upm / 4, upm / 2);
    let x1 = x0 + r.range(1, upm / 2 + 1);
    let y1 = y0 + r.range(1, upm * 3 / 4 + 1);
    (x0 as i16, y0 as i16, x1 as i16, y1 as i16)
}

/// a font of the seed: every field drawn, its glyphs 1 to 3 rectangles (space and nbsp empty)
pub fn font(seed: u32) -> Font {
    let mut r = XorShift32::new(seed);
    let upm = *r.pick(&[1000u16, 2048, 1024, 256, 16, 4096, 2000, 1000, 2048, 500]);
    let u = upm as i64;
    let mut glyphs = Vec::new();
    for g in 0..=CHARS.len() {
        let empty = g > 0 && matches!(CHARS[g - 1], 0x20 | 0xa0);
        let rects = if empty { Vec::new() } else { (0..r.range(1, 3)).map(|_| rect(&mut r, u)).collect() };
        let adv = if r.chance(8) { 0 } else { r.range(u / 8, u) as u16 };
        let vert = (r.range(u / 2, u * 3 / 2) as u16, r.range(-u / 8, u / 4) as i16);
        glyphs.push(Glyph { adv, rects, vert });
    }
    let fixed = r.chance(4);
    if fixed {
        let a = glyphs[1].adv;
        glyphs.iter_mut().for_each(|g| g.adv = a);
    }
    let asc = r.range(u / 2, u) as i16;
    let desc = -(r.range(0, u / 2) as i16);
    let sel = if r.chance(2) { 0x80 } else { 0 } | 0x40;
    let mut cmap: Vec<(u32, u16)> = CHARS.iter().enumerate().map(|(i, &c)| (c, i as u16 + 1)).collect();
    if r.chance(4) {
        cmap.retain(|c| c.0 < 0x10000); // (no format 12 subtable)
    }
    Font {
        upm,
        hhea: (asc + r.range(-u / 8, u / 8) as i16, desc + r.range(-u / 8, u / 8) as i16, r.range(0, u / 4) as i16 * if r.chance(6) { -1 } else { 1 }),
        os2: if r.chance(10) { None } else { Some(*r.pick(&[0u16, 1, 2, 3, 4, 4, 4, 5])) },
        typo: (asc - r.range(0, u / 8) as i16, desc + r.range(0, u / 8) as i16, r.range(0, u / 5) as i16),
        win: ((asc as i64 + r.range(0, u / 4)) as u16, if r.chance(8) { (-(r.range(1, u / 4))) as u16 } else { (-desc as i64 + r.range(0, u / 4)) as u16 }),
        fs_selection: sel,
        x_height: if r.chance(5) { 0 } else { r.range(u / 4, u / 2) as i16 },
        cap_height: if r.chance(5) { 0 } else { r.range(u / 2, u) as i16 },
        strike: (if r.chance(5) { 0 } else { r.range(1, u / 10) as i16 }, r.range(0, u / 2) as i16),
        underline: (-(r.range(0, u / 5) as i16), if r.chance(5) { 0 } else { r.range(1, u / 10) as i16 }),
        script: std::array::from_fn(|_| r.range(-u / 2, u) as i16),
        fixed_pitch: fixed,
        symbol: false,
        vertical: r.chance(2),
        family: format!("Kikizake {seed:x}"),
        glyphs,
        cmap,
    }
}

fn gcd(a: i64, b: i64) -> i64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn font_line(f: &Font) -> String {
    format!("dwfont {}", hex(&f.bytes()))
}

/// representatives of the bidirectional classes (UAX #9), no explicit formatting characters
pub const BIDI_CHARS: &[(u32, &str)] = &[
    (0x61, "L"), (0x5d0, "R"), (0x627, "AL"), (0x31, "EN"), (0x661, "AN"), (0x2b, "ES"), (0x24, "ET"), (0x2c, "CS"), (0x300, "NSM"), (0x20, "WS"), (0x21, "ON"), (0x9, "S"),
];

/// characters of several scripts and the common and inherited ones
pub const SCRIPT_CHARS: &[u32] = &[0x41, 0x7a, 0x5d0, 0x627, 0x3b1, 0x430, 0x4e00, 0x3042, 0x30a2, 0xac00, 0xe01, 0x915, 0x20, 0x31, 0x2e, 0x300, 0x3001, 0x30fc, 0x200d, 0x1f600];

/// characters of several line breaking classes (UAX #14)
pub const BREAK_CHARS: &[u32] = &[0x61, 0x20, 0x2d, 0x31, 0x28, 0x29, 0x22, 0x4e00, 0x3002, 0x3001, 0xa0, 0x200b, 0xad, 0xa, 0x300, 0x21, 0x2f, 0x24, 0x25, 0x3042, 0x30a1, 0x9];

pub fn dwrite() -> String {
    let mut s = String::from("# kikizake suite dwrite: DirectWrite font faces, shaping, rasterization, text analysis and layout (SPEC.md). Made by `kikizake gen dwrite`.\n");
    let mut r = XorShift32::new(0xd_717e);
    let gids = (0..=CHARS.len()).map(|g| g.to_string()).collect::<Vec<_>>().join(" ");
    // a face's metrics: each font, its info, design and GDI compatible metrics
    for i in 0..120u32 {
        let f = font(0x0f00_0000 + i);
        writeln!(s, "case face-{i}\n{}\ndwinfo\ndwmetrics", font_line(&f)).unwrap();
        for _ in 0..3 {
            writeln!(s, "dwgdimetrics {} {}", r.range(1, 80), *r.pick(&[1, 1, 2, 3])).unwrap();
        }
        let cps: Vec<String> = CHARS.iter().chain(&[0x46, 0x5d1, 0x1f601, 0x10ffff, 0]).map(|c| format!("{c:#x}")).collect();
        writeln!(s, "dwcmap {}\ndwglyph 0 {gids}\ndwglyph 1 {gids}\ndwglyph 0 {}", cps.join(" "), CHARS.len() + 1).unwrap();
        writeln!(s, "end").unwrap();
        // (GDI compatible glyph metrics in a case of their own: what is open there leaves the
        // rest of the face's lines compared)
        writeln!(s, "case gdiglyph-{i}\n{}", font_line(&f)).unwrap();
        for _ in 0..3 {
            writeln!(s, "dwgdiglyph {} {} {} {} {gids}", r.range(1, 80), *r.pick(&[1, 1, 2]), r.range(0, 1), r.range(0, 1)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // shaping: the mapped characters (and some unmapped) at several sizes
    for i in 0..40u32 {
        let f = font(0x0f10_0000 + i);
        writeln!(s, "case shape-{i}\n{}", font_line(&f)).unwrap();
        for _ in 0..4 {
            let n = r.range(1, 12);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", *r.pick(&[0x41, 0x42, 0x43, 0x44, 0x45, 0x61, 0x62, 0x20, 0x30, 0x31, 0x2d, 0x46]))).collect();
            writeln!(s, "dwshape {} {}", r.range(1, 96), t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // rasterization: single glyphs at many sizes and subpixel origins, in every mode, then runs
    // (each line a case of its own: one mode left open leaves the others compared)
    for i in 0..60u32 {
        let f = font(0x0f20_0000 + i);
        for mode in ["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural"] {
            let g = r.range(0, 8);
            writeln!(s, "case run-{i}-{mode}\n{}", font_line(&f)).unwrap();
            writeln!(s, "dwrun {mode} {} {} {} {g} 0\nend", r.range(4, 40), r.range(-8, 40), r.range(16, 96)).unwrap();
        }
        let mode = *r.pick(&["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural"]);
        let n = r.range(2, 5);
        let run: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(0, 8), r.range(0, 60))).collect();
        writeln!(s, "case run-{i}-multi\n{}", font_line(&f)).unwrap();
        writeln!(s, "dwrun {mode} {} {} {} {}\nend", r.range(6, 24), r.range(0, 16), r.range(32, 64), run.join(" ")).unwrap();
    }
    // layout: a line of the font's characters, with and without wrapping
    for i in 0..30u32 {
        let f = font(0x0f30_0000 + i);
        writeln!(s, "case layout-{i}\n{}", font_line(&f)).unwrap();
        for _ in 0..3 {
            let n = r.range(1, 16);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", *r.pick(&[0x41, 0x42, 0x43, 0x61, 0x62, 0x20, 0x20, 0x30, 0x2d, 0xa0]))).collect();
            writeln!(s, "dwlayout {} {} {} {}", r.range(6, 48), r.range(40, 800), r.range(0, 1), t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // scripts: every pair of the characters, then random strings
    for (i, a) in SCRIPT_CHARS.iter().enumerate() {
        writeln!(s, "case script-pairs-{i}").unwrap();
        for b in SCRIPT_CHARS {
            writeln!(s, "dwscript {a:#x} {b:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for i in 0..40 {
        writeln!(s, "case script-{i}").unwrap();
        for _ in 0..8 {
            let n = r.range(1, 12);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", r.pick(SCRIPT_CHARS))).collect();
            writeln!(s, "dwscript {}", t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // bidi: every string of up to 3 of the classes, both paragraph directions, then random ones
    let k = BIDI_CHARS.len();
    for dir in ["ltr", "rtl"] {
        for a in 0..k {
            writeln!(s, "case bidi-{dir}-{}", BIDI_CHARS[a].1).unwrap();
            writeln!(s, "dwbidi {dir} {:#x}", BIDI_CHARS[a].0).unwrap();
            for b in 0..k {
                writeln!(s, "dwbidi {dir} {:#x} {:#x}", BIDI_CHARS[a].0, BIDI_CHARS[b].0).unwrap();
                for c in 0..k {
                    writeln!(s, "dwbidi {dir} {:#x} {:#x} {:#x}", BIDI_CHARS[a].0, BIDI_CHARS[b].0, BIDI_CHARS[c].0).unwrap();
                }
            }
            writeln!(s, "end").unwrap();
        }
    }
    for i in 0..60 {
        writeln!(s, "case bidi-random-{i}").unwrap();
        for _ in 0..8 {
            let n = r.range(4, 16);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", r.pick(BIDI_CHARS).0)).collect();
            writeln!(s, "dwbidi {} {}", r.pick(&["ltr", "rtl"]), t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // line breaks: every pair (with a letter either side), then random strings
    for (i, a) in BREAK_CHARS.iter().enumerate() {
        writeln!(s, "case break-pairs-{i}").unwrap();
        for b in BREAK_CHARS {
            writeln!(s, "dwbreak 0x61 {a:#x} {b:#x} 0x61").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for i in 0..40 {
        writeln!(s, "case break-{i}").unwrap();
        for _ in 0..8 {
            let n = r.range(1, 14);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", r.pick(BREAK_CHARS))).collect();
            writeln!(s, "dwbreak {}", t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    targeted(&mut s, &mut r);
    s
}

/// a font of the seed with the glyphs' advances and rectangles given (glyph i + 1 for entry i)
fn font_with(seed: u32, upm: u16, glyphs: &[(u16, Vec<(i16, i16, i16, i16)>)]) -> Font {
    let mut f = font(seed);
    f.upm = upm;
    f.vertical = false;
    for (i, (adv, rects)) in glyphs.iter().enumerate() {
        f.glyphs[i + 1].adv = *adv;
        f.glyphs[i + 1].rects = rects.clone();
    }
    f
}

/// cases aimed at what the random ones left open: the GDI natural rounding threshold, thin
/// stems (dropout), the pen positions of aliased runs
fn targeted(s: &mut String, r: &mut XorShift32) {
    // natural placements: advances whose em / upm multiple falls in 0.44..0.48 of a pixel
    let chars: Vec<String> = CHARS[..13].iter().map(|c| format!("{c:#x}")).collect();
    for em in 1..=4u32 {
        let lo = (0.44 * 2048.0 / em as f64).ceil() as u32;
        let hi = (0.48 * 2048.0 / em as f64).floor() as u32;
        let advs: Vec<u32> = (lo..=hi).flat_map(|a| [a, a + 2048 / em * (1 + a % 3)]).collect();
        for (k, ch) in advs.chunks(13).enumerate() {
            let gs: Vec<_> = ch.iter().map(|&a| (a as u16, vec![(0, 0, 100, 100)])).collect();
            let f = font_with(0x0f40_0000 + em * 64 + k as u32, 2048, &gs);
            writeln!(s, "case natplace-{em}-{k}\n{}\ndwshape {em} {}\nend", font_line(&f), chars[..ch.len()].join(" ")).unwrap();
        }
    }
    // thin stems: vertical and horizontal bars under a pixel wide at subpixel offsets
    for i in 0..40u32 {
        let upm = *r.pick(&[1000u16, 2048, 256]);
        let u = upm as i64;
        let em = r.range(6, 32);
        let px = u / em;
        let mut gs = Vec::new();
        for _ in 0..8 {
            let w = r.range(1, px.max(2)) as i16;
            let x = r.range(0, u / 2) as i16;
            let y = r.range(0, u / 4) as i16;
            let l = r.range(px * 2, u / 2) as i16;
            gs.push((r.range(u / 4, u) as u16, vec![if r.chance(2) { (x, y, x + w, y + l) } else { (x, y, x + l, y + w) }]));
        }
        let f = font_with(0x0f50_0000 + i, upm, &gs);
        writeln!(s, "case drop-{i}\n{}", font_line(&f)).unwrap();
        for _ in 0..8 {
            writeln!(s, "dwrun aliased {em} {} {} {} 0", r.range(0, 40), r.range(16, 96), r.range(1, 8)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // aliased runs of wide glyphs (no stem thinner than a pixel): where each glyph goes
    for i in 0..40u32 {
        let u = 1000i64;
        let gs: Vec<_> = (0..8).map(|_| (r.range(200, 1000) as u16, vec![(r.range(0, 100) as i16, 0, r.range(300, 600) as i16, r.range(300, 700) as i16)])).collect();
        let f = font_with(0x0f60_0000 + i, u as u16, &gs);
        writeln!(s, "case multi-{i}\n{}", font_line(&f)).unwrap();
        let n = r.range(2, 4);
        let run: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(1, 8), r.range(0, 200))).collect();
        writeln!(s, "dwrun aliased {} {} {} {}\nend", r.range(8, 40), r.range(0, 40), r.range(40, 96), run.join(" ")).unwrap();
    }
    // a thin stem (or bar) and one other box, beside, above or below it, either contour first:
    // which way the dropout pixel goes in a glyph of several contours
    for i in 0..60u32 {
        let mut gs = Vec::new();
        for _ in 0..8 {
            let (f, w) = (r.range(0, 99) as i16, r.range(5, 60) as i16);
            let stem = if r.chance(2) { (300 + f, 0, 300 + f + w, 500) } else { (0, 300 + f, 500, 300 + f + w) };
            let (dx, dy) = *r.pick(&[(-450i16, 0i16), (450, 0), (0, -450), (0, 450), (-450, -450), (450, 450), (-450, 450), (450, -450)]);
            let other = (300 + dx, 150 + dy, 500 + dx, 350 + dy);
            let rects = if r.chance(2) { vec![stem, other] } else { vec![other, stem] };
            gs.push((1000u16, rects));
        }
        let f = font_with(0x0f70_0000 + i, 1000, &gs);
        writeln!(s, "case drop2-{i}\n{}", font_line(&f)).unwrap();
        for g in 1..=8 {
            writeln!(s, "dwrun aliased 10 {} {} {g} 0", r.range(0, 3), r.range(40, 43)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // thin stems and bars, alone or crossing a box, at the larger sizes: up to which size the
    // rasterizer keeps dropout control on, in each mode
    for em in (26..=64i64).step_by(2) {
        let u = 1000i64;
        let mut gs = Vec::new();
        for k in 0..8i64 {
            // (a width of 0.2 .. 0.8 pixel, at a fraction of a pixel offset)
            let w = (u * (2 + r.range(0, 6)) / 10 / em).max(1) as i16;
            let f = r.range(0, u / em) as i16;
            let rects = match k % 4 {
                0 => vec![(300 + f, 0, 300 + f + w, 500)],
                1 => vec![(0, 300 + f, 500, 300 + f + w)],
                2 => vec![(0, 300 + f, 500, 300 + f + w), (250, 0, 450, 280 + f)],
                _ => vec![(300 + f, 0, 300 + f + w, 500), (320 + f + w, 100, 600, 300), (0, 200, 200, 420)],
            };
            gs.push((1000u16, rects));
        }
        let f = font_with(0x0f80_0000 + em as u32, u as u16, &gs);
        for mode in ["aliased", "natural", "gray"] {
            writeln!(s, "case dropem-{em}-{mode}\n{}", font_line(&f)).unwrap();
            for g in 1..=8 {
                writeln!(s, "dwrun {mode} {em} {} {} {g} 0", r.range(0, 4), r.range(160, 164)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // the grid fit mode asked for (enabled, disabled), in each mode, small sizes to large: single
    // glyphs and runs of two
    for mode in ["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural"] {
        for fit in ["fit", "nofit"] {
            let f = font_with(0x0f90_0000 + r.range(0, 1 << 16) as u32, 1000, &(0..8).map(|k| (r.range(300, 1000) as u16, if k % 2 == 0 { vec![(r.range(0, 300) as i16, r.range(-100, 100) as i16, r.range(400, 900) as i16, r.range(300, 800) as i16)] } else { vec![(300 + r.range(0, 99) as i16, 0, 300 + r.range(100, 160) as i16, 500), (0, 600, r.range(50, 500) as i16, 600 + r.range(5, 60) as i16)] })).collect::<Vec<_>>());
            writeln!(s, "case fit-{mode}-{fit}\n{}", font_line(&f)).unwrap();
            for em in [6, 9, 12, 16, 20, 28, 36, 48, 64, 96] {
                let run = if r.chance(2) { format!("{} 0", r.range(1, 8)) } else { format!("{} {} {} 0", r.range(1, 8), r.range(0, 200), r.range(1, 8)) };
                writeln!(s, "dwrun {mode}.{fit} {em} {} {} {run}", r.range(0, 40), r.range(160, 200)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // every face op before a font is loaded
    // a cmap lookup of one character
    writeln!(s, "case cmap-one\n{}\ndwcmap 0x41\ndwcmap 0x5d0\nend", font_line(&font(0x0f00_0001))).unwrap();
    writeln!(s, "case noface\ndwinfo\ndwmetrics\ndwgdimetrics 12 1\ndwcmap 0x41 0x42\ndwglyph 0 1 2\ndwgdiglyph 12 1 0 0 1\ndwshape 12 0x41 0x42\ndwrun aliased 12 0 0 1 0\ndwlayout 12 400 1 0x41 0x42\nend").unwrap();
    // a layout of a font without a family name (no name ID 1)
    let mut f = font(0x0fc1_0000);
    f.family = String::new();
    writeln!(s, "case noname\n{}\ndwinfo\ndwlayout 12 400 1 0x41 0x42\ndwlayout 20 30 1 0x41 0x20 0x42\nend", font_line(&f)).unwrap();
    // GDI compatible metrics at em sizes and pixels per dip of zero and below
    let f = font(0x0fc0_0000);
    writeln!(s, "case gdiinvalid\n{}", font_line(&f)).unwrap();
    for (em, ppd) in [(0, 1), (-1, 1), (-12, 1), (12, 0), (12, -1), (12, -2), (0, 0), (-12, -1)] {
        writeln!(s, "dwgdimetrics {em} {ppd}\ndwgdiglyph {em} {ppd} 0 0 1 2\ndwgdiglyph {em} {ppd} 1 1 1 2").unwrap();
    }
    writeln!(s, "end").unwrap();
    // every script: the first, middle and last character of each Scripts.txt range, and of each
    // gap between them (unassigned), and every common and inherited character, alone
    let mut cps = Vec::new();
    let mut next = 0u32;
    for &(lo, hi, v) in crate::ucd::scripts().ranges() {
        if lo > next {
            cps.extend([next, (next + lo - 1) / 2, lo - 1]);
        }
        match v {
            "Common" | "Inherited" => cps.extend(lo..=hi),
            _ => cps.extend([lo, (lo + hi) / 2, hi]),
        }
        next = hi + 1;
    }
    cps.extend([next, 0x10ffff]);
    cps.retain(|c| !(0xd800..=0xdfff).contains(c) && *c != 0);
    cps.dedup();
    for (i, ch) in cps.chunks(400).enumerate() {
        writeln!(s, "case scriptall-{i}").unwrap();
        for c in ch {
            writeln!(s, "dwscript {c:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // every line break class and bidi class: the first, middle and last character of each range
    // of LineBreak.txt and DerivedBidiClass.txt, between letters and alone
    let samples = |t: &crate::ucd::Table| {
        let mut v: Vec<u32> = Vec::new();
        let mut next = 0u32;
        for &(lo, hi, _) in t.ranges() {
            // (and the gaps, which take the file's @missing values)
            if lo > next {
                v.extend([next, (next + lo - 1) / 2, lo - 1]);
            }
            v.extend([lo, (lo + hi) / 2, hi]);
            next = hi + 1;
        }
        v.retain(|c| !(0xd800..=0xdfff).contains(c) && *c != 0);
        v.dedup();
        v
    };
    for (i, ch) in samples(crate::ucd::line_break()).chunks(300).enumerate() {
        writeln!(s, "case breakall-{i}").unwrap();
        for c in ch {
            writeln!(s, "dwbreak 0x61 {c:#x} 0x61\ndwbreak {c:#x} {c:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for (i, ch) in samples(crate::ucd::bidi_class()).chunks(300).enumerate() {
        writeln!(s, "case bidiall-{i}").unwrap();
        for c in ch {
            writeln!(s, "dwbidi ltr 0x5d0 {c:#x} 0x5d0\ndwbidi rtl 0x61 {c:#x} 0x61\ndwbidi ltr {c:#x} 0x31").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // a no-visual or unassigned character among others: every common control, format and
    // separator character and some unassigned ones, between, before and after letters
    let gc = crate::ucd::general_category();
    let mut nv: Vec<u32> = crate::ucd::scripts().ranges().iter().filter(|r| r.2 == "Common").flat_map(|r| r.0..=r.1).filter(|&c| matches!(gc.get(c), Some("Cc" | "Cf" | "Zl" | "Zp")) && c != 0).collect();
    nv.extend([0x378, 0x5ff, 0xfffe, 0xffff, 0x2fffe, 0xe0080, 0x10ffff]);
    for (i, ch) in nv.chunks(60).enumerate() {
        writeln!(s, "case scriptctx-{i}").unwrap();
        for c in ch {
            writeln!(s, "dwscript 0x41 {c:#x} 0x41\ndwscript {c:#x} 0x41\ndwscript 0x41 {c:#x}\ndwscript 0x5d0 {c:#x} 0x41\ndwscript {c:#x} {c:#x}\ndwscript 0x20 {c:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // a common character after a no-visual one
    writeln!(s, "case scriptctx-after").unwrap();
    for c in [0x1, 0xad, 0x200b, 0x2028, 0xfeff, 0xe0001, 0xfffe, 0x1bca0, 0x1d173] {
        writeln!(s, "dwscript {c:#x} 0x20\ndwscript {c:#x} 0x20 0x41\ndwscript 0x41 {c:#x} 0x20\ndwscript 0x5d0 {c:#x} 0x20 0x41\ndwscript {c:#x} 0x300\ndwscript {c:#x} 0x2e 0x5d0\ndwscript 0x378 0x20 {c:#x}").unwrap();
    }
    writeln!(s, "end").unwrap();
    // the system font fallback: which of the machine's fonts maps each character the font does
    // not (alone, and among the font's own), and layouts that take it
    const FB: [u32; 20] = [0x4e00, 0x3042, 0x30a2, 0xac00, 0x627, 0x628, 0x5d1, 0x915, 0xe01, 0x3b1, 0x430, 0x1f601, 0x2603, 0x2192, 0x378, 0xe000, 0x10fffd, 0x20ac, 0xe9, 0x7e];
    writeln!(s, "case fallback-chars\n{}", font_line(&font(0x0fe1_0000))).unwrap();
    for c in FB {
        writeln!(s, "dwfallback {c:#x}\ndwfallback 0x41 {c:#x} 0x42\ndwfallback {c:#x} {c:#x} 0x20 {c:#x}").unwrap();
    }
    writeln!(s, "dwfallback 0x41 0x42 0x20 0x61\nend").unwrap();
    for i in 0..12u32 {
        let f = font(0x0fe2_0000 + i);
        writeln!(s, "case fallback-{i}\n{}", font_line(&f)).unwrap();
        let mut rl = XorShift32::new(0x1a71_0000 + i);
        for _ in 0..4 {
            let n = rl.range(1, 14);
            let t: Vec<String> = (0..n)
                .map(|_| {
                    let (a, b) = (FB[rl.range(0, 19) as usize], FB[rl.range(0, 19) as usize]);
                    format!("{:#x}", *rl.pick(&[0x41, 0x61, 0x20, 0x20, 0x2d, a, b]))
                })
                .collect();
            writeln!(s, "dwfallback {}", t.join(" ")).unwrap();
            writeln!(s, "dwlayout {} {} {} {}", rl.range(6, 48), rl.range(40, 800), rl.range(0, 1), t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // where the fallback's pieces end: every two of the characters side by side (one font, or
    // two; one block, or two), and after and before the font's own
    writeln!(s, "case fallback-pairs\n{}", font_line(&font(0x0fe3_0000))).unwrap();
    for a in FB {
        let row: Vec<String> = FB.iter().map(|b| format!("{a:#x} {b:#x}")).collect();
        for r in row {
            writeln!(s, "dwfallback {r}").unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    // the fallback, and a layout, of a font without a family name; layouts without fallback of
    // supplementary characters
    let mut f = font(0x0fe3_0001);
    f.family = String::new();
    writeln!(s, "case fallback-noname\n{}\ndwfallback 0x41 0x4e00\ndwlayout 12 400 1 0x41 0x4e00\nend", font_line(&f)).unwrap();
    writeln!(s, "case fallback-nf\n{}\ndwlayoutnf 12 400 1 0x41 0x1f601 0x10fffd 0x42\ndwlayoutnf 20 80 1 0x1f601 0x20 0x1f601 0x20 0x41 0x1f601\nend", font_line(&font(0x0fe3_0002))).unwrap();
    // right-to-left runs at a line's start, middle and end, alone on a line and across lines
    // (the reordering depth)
    writeln!(s, "case fallback-bidi\n{}", font_line(&font(0x0fe3_0003))).unwrap();
    for t in ["0x627", "0x627 0x41", "0x41 0x627", "0x41 0x627 0x41 0x5d1 0x41", "0x627 0x20 0x628", "0x41 0x20 0x627 0x20 0x41 0x20 0x5d1 0x20 0x41", "0x627 0x628 0x20 0x41 0x41 0x20 0x5d1", "0x41 0x30 0x627 0x31 0x41"] {
        for (w, wrap) in [(800, 0), (40, 1), (100, 1)] {
            writeln!(s, "dwlayout 16 {w} {wrap} {t}").unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    // layouts without font fallback: characters the font does not map among those it does
    for i in 0..12u32 {
        let f = font(0x0fe0_0000 + i);
        writeln!(s, "case layoutnf-{i}\n{}", font_line(&f)).unwrap();
        let mut rl = XorShift32::new(0x1a70_0000 + i);
        for _ in 0..4 {
            let n = rl.range(1, 14);
            let t: Vec<String> = (0..n).map(|_| format!("{:#x}", *rl.pick(&[0x41, 0x61, 0x20, 0x20, 0x3b1, 0x4e00, 0x430, 0x2d, 0x7e, 0x26]))).collect();
            writeln!(s, "dwlayoutnf {} {} {} {}", rl.range(6, 48), rl.range(40, 800), rl.range(0, 1), t.join(" ")).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // every pair of classes: a character of each line break class (the first the file lists)
    // before one of each, next to each other and with a space between; of each bidi class, in
    // both directions, alone and between strong characters, and after each explicit formatting
    // character closed again
    let reps = |t: &crate::ucd::Table| {
        let mut seen: Vec<&str> = Vec::new();
        let mut v: Vec<u32> = Vec::new();
        for &(lo, _, val) in t.ranges() {
            let c = if lo == 0 { 1 } else { lo };
            if !seen.contains(&val) && !(0xd800..=0xdfff).contains(&c) && t.get(c) == Some(val) {
                seen.push(val);
                v.push(c);
            }
        }
        v
    };
    let lb = reps(crate::ucd::line_break());
    let pairs: Vec<(u32, u32)> = lb.iter().flat_map(|&a| lb.iter().map(move |&b| (a, b))).collect();
    for (i, ch) in pairs.chunks(200).enumerate() {
        writeln!(s, "case breakpair-{i}").unwrap();
        for (a, b) in ch {
            writeln!(s, "dwbreak {a:#x} {b:#x}\ndwbreak {a:#x} 0x20 {b:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    let bc = reps(crate::ucd::bidi_class());
    let pairs: Vec<(u32, u32)> = bc.iter().flat_map(|&a| bc.iter().map(move |&b| (a, b))).collect();
    for (i, ch) in pairs.chunks(100).enumerate() {
        writeln!(s, "case bidipair-{i}").unwrap();
        for (a, b) in ch {
            writeln!(s, "dwbidi ltr {a:#x} {b:#x}\ndwbidi rtl {a:#x} {b:#x}\ndwbidi ltr 0x5d0 {a:#x} {b:#x} 0x61\ndwbidi rtl 0x61 {a:#x} {b:#x} 0x5d0").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for (i, e) in [0x202a, 0x202b, 0x202d, 0x202e, 0x2066, 0x2067, 0x2068].into_iter().enumerate() {
        let close = if e >= 0x2066 { 0x2069 } else { 0x202c };
        writeln!(s, "case bidiembed-{i}").unwrap();
        for b in &bc {
            writeln!(s, "dwbidi ltr 0x61 {e:#x} {b:#x} {close:#x} 0x5d0\ndwbidi rtl 0x5d0 {e:#x} {b:#x} {close:#x} 0x61\ndwbidi ltr {e:#x} {b:#x} 0x31\ndwbidi rtl {e:#x} {e:#x} {b:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // files that are not one TrueType font: collections, fonts short of a table or cut short,
    // other signatures, bytes at random; each then asked what a face answers
    let probe = "dwinfo\ndwmetrics\ndwcmap 0x41 0x61\ndwglyph 0 0 1 2\ndwgdiglyph 12 1 0 0 1\ndwrun aliased 12 0 40 1 0";
    let base = font(0x0fd0_0000).bytes();
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for n in 1..=3u32 {
        files.push((format!("ttc{n}"), crate::ttf::ttc(&(0..n).map(|k| font(0x0fd1_0000 + k).bytes()).collect::<Vec<_>>())));
    }
    files.push(("ttc0".into(), crate::ttf::ttc(&[])));
    for (tag, _) in crate::ttf::tables_of(&base) {
        let t: Vec<_> = crate::ttf::tables_of(&base).into_iter().filter(|x| x.0 != tag).collect();
        files.push((format!("no-{}", String::from_utf8_lossy(&tag).trim().replace('/', "")), crate::ttf::assemble(t)));
    }
    for cut in [1usize, 4, 11, 12, 13, 40, 200, base.len() / 2, base.len() - 4, base.len() - 1] {
        files.push((format!("cut{cut}"), base[..cut].to_vec()));
    }
    for sig in [*b"true", *b"OTTO", *b"typ1", *b"\0\x01\0\x01", *b"wOFF"] {
        let mut b = base.clone();
        b[..4].copy_from_slice(&sig);
        files.push((format!("sig{}", hex(&sig)), b));
    }
    let mut rb = XorShift32::new(0xf11e);
    for i in 0..6usize {
        files.push((format!("rand{i}"), (0..[3usize, 16, 64, 300, 1000, 4000][i]).map(|_| rb.range(0, 255) as u8).collect()));
    }
    for (name, b) in files {
        writeln!(s, "case file-{name}\ndwfont {}\n{probe}\nend", hex(&b)).unwrap();
    }
    // grid fit disabled at 16 ppem, 2048 units per em (a 16.16 scale of 512): edges below and
    // left of zero on an exact half of a 1/64 pixel, which re/dwrite.md (Follow-up E) says round
    // away from zero (nofittie)
    {
        let gs: Vec<_> = (0..14i64).map(|j| (1000u16, vec![((-(2 * j + 1) * 3) as i16, (-(2 * j + 1) * 5) as i16, 300i16, 400i16)])).collect();
        let f = font_with(0x0fe1_0000, 2048, &gs);
        writeln!(s, "case nofittie\n{}", font_line(&f)).unwrap();
        for mode in ["gdi", "gdinatural"] {
            for g in 1..=14 {
                writeln!(s, "dwrun {mode}.nofit 16 160 160 {g} 0").unwrap();
            }
        }
        writeln!(s, "end").unwrap();
    }
    // GDI modes with grid fit disabled: single boxes at sizes 6..96, edges anywhere in a pixel
    for mode in ["gdi", "gdinatural"] {
        for i in 0..6u32 {
            let gs: Vec<_> = (0..8).map(|_| (r.range(300, 1000) as u16, vec![(r.range(0, 400) as i16, r.range(-200, 200) as i16, r.range(450, 990) as i16, r.range(300, 900) as i16)])).collect();
            let f = font_with(0x0fb0_0000 + i * 2 + (mode == "gdi") as u32, 1000, &gs);
            writeln!(s, "case fitgdi-{mode}-{i}\n{}", font_line(&f)).unwrap();
            for g in 1..=8 {
                writeln!(s, "dwrun {mode}.nofit {} {} {} {g} 0", r.range(6, 96), r.range(0, 40), r.range(160, 200)).unwrap();
                writeln!(s, "dwrun {mode}.fit {} {} {} {g} 0", r.range(6, 96), r.range(0, 40), r.range(160, 200)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // GDI compatible metrics turned back into design units: advances whose whole pixel count k
    // makes k upm / ppem fall within a fifth of a pixel of a half design unit, every upm and size
    for upm in [256u16, 1000, 1024, 2000, 2048, 4096] {
        for pp in (8..=104i64).step_by(4) {
            let u = upm as i64;
            let ks: Vec<i64> = (1..pp * 3).filter(|k| (2 * k * u).rem_euclid(2 * pp) * 10 >= 8 * pp && (2 * k * u).rem_euclid(2 * pp) * 10 <= 12 * pp).take(14).collect();
            if ks.is_empty() {
                continue;
            }
            let gs: Vec<_> = ks.iter().map(|k| (((2 * k * u + pp) / (2 * pp)) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 2) as i16)])).collect();
            let f = font_with(0x0fa0_0000 + u as u32 * 256 + pp as u32, upm, &gs);
            let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case gdiback-{upm}-{pp}\n{}\ndwgdiglyph {pp} 1 0 0 {}\ndwgdiglyph {} 2 0 0 {}\nend", font_line(&f), gids.join(" "), pp / 2, gids.join(" ")).unwrap();
        }
    }
    // sideways GDI compatible metrics: boxes whose bottoms step through the pixel (the bottom
    // side bearing, back in design units), every upm, sizes and both placements
    for upm in [1000u16, 2000, 2048] {
        for pp in (9..=63i64).step_by(6) {
            let u = upm as i64;
            let gs: Vec<_> = (0..14i64).map(|j| ((u / 2) as u16, vec![(0i16, (j * u / 27 - u / 4) as i16, (u / 4) as i16, (j * u / 27 + u / 4) as i16)])).collect();
            let f = font_with(0x0fc0_0000 + u as u32 * 256 + pp as u32, upm, &gs);
            let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case gdiside-{upm}-{pp}\n{}\ndwgdiglyph {pp} 1 0 1 {}\ndwgdiglyph {pp} 1 1 1 {}\nend", font_line(&f), gids.join(" "), gids.join(" ")).unwrap();
        }
    }
    // sideways bottoms a design unit apart through one sample row, at the sizes and vertical
    // origins of the gdiglyph cases whose bottom side bearing the model has a unit over or under
    // (does the result follow the outline's own bottom or only the sample row it ends in?)
    let rows: [(u16, i64, i16, i16, i64); 8] = [
        (2000, 13, 1486, -143, -346),
        (2000, 13, 1486, -143, -332),
        (2000, 13, 1486, -143, 12),
        (2000, 13, 1486, -143, 26),
        (2000, 13, 1460, -169, -346),
        (2000, 13, 1510, -119, -346),
        (2048, 27, 1398, -813, 440),
        (2048, 27, 1398, -813, 454),
    ];
    for (k, &(upm, pp, asc, desc, b0)) in rows.iter().enumerate() {
        let u = upm as i64;
        let gs: Vec<_> = (0..14i64).map(|j| ((u / 2) as u16, vec![(0i16, (b0 + j) as i16, (u / 4) as i16, (b0 + j + u / 4) as i16)])).collect();
        let mut f = font_with(0x0fd0_0000 + k as u32, upm, &gs);
        f.os2 = Some(4);
        f.typo = (asc, desc, 0);
        let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
        writeln!(s, "case gdisidebot-{k}\n{}\ndwgdiglyph {pp} 1 0 1 {}\ndwgdiglyph {pp} 1 1 1 {}\ndwglyph 1 {}\nend", font_line(&f), gids.join(" "), gids.join(" "), gids.join(" ")).unwrap();
    }
    // sideways bottoms one sample row apart (centred in it at 13 ppem, rows -8 to 19) under vertical origins
    // and advances of 9 to 11 and 0 to 3 pixels more, at 13, 26 and 39 ppem: where does the
    // bottom side bearing leave the em fraction differences?
    let mut k = 0;
    for asc in [1340i16, 1486, 1640] {
        for (d, half) in (0..4i64).flat_map(|d| [(d, 0i64), (d, 1)]) {
            let v = ((asc as i64 * 13 * 128 + 65 * 2000).div_euclid(128 * 2000)) as i64;
            let vadv = ((v + d) * 2000 + 6) / 13;
            let gs: Vec<_> = (0..14i64).map(|j| {
                let b = -((2 * (j + 14 * half - 8) + 1) * 2000).div_euclid(12 * 13);
                (1000u16, vec![(0i16, b as i16, 500i16, (b + 500) as i16)])
            }).collect();
            let mut f = font_with(0x0fd1_0000 + k, 2000, &gs);
            f.os2 = Some(4);
            f.typo = (asc, (asc as i64 - vadv) as i16, 0);
            let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
            write!(s, "case gdisideorg-{k}\n{}\n", font_line(&f)).unwrap();
            for pp in [13, 26, 39] {
                writeln!(s, "dwgdiglyph {pp} 1 0 1 {}\ndwgdiglyph {pp} 1 1 1 {}", gids.join(" "), gids.join(" ")).unwrap();
            }
            writeln!(s, "dwglyph 1 {}\nend", gids.join(" ")).unwrap();
            k += 1;
        }
    }
    // the sideways bottom side bearings the model has a unit off, again at units per em from
    // 4096 to 16384 (vertical origin, advance and bottoms scaled with it: the same pixels): a
    // bearing back in design units at so many scales pins the 16.16 em fraction Windows has
    // before it (which of advance, origin and bottom is off, and by how much)
    let gdo: Vec<i64> = (-8..20i64).map(|r| -((2 * r + 1) * 2000).div_euclid(12 * 13)).collect();
    let bot = |b0: i64| (0..14i64).map(|j| b0 + j).collect::<Vec<i64>>();
    // (pixels per em, units per em, vertical origin, advance height, bottoms: at those units)
    let rows: [(i64, i64, i64, i64, Vec<i64>); 6] = [
        (13, 2000, 1486, 1692, gdo.clone()),
        (13, 2000, 1340, 1692, gdo.clone()),
        (13, 2000, 1510, 1629, bot(-346)),
        (27, 2048, 1398, 2211, bot(440)),
        (13, 2000, 1538, 1692, gdo.clone()),
        (27, 2048, 1365, 2200, bot(440)),
    ];
    for (k, (pp, u0, vorg, vadv, bs)) in rows.iter().enumerate() {
        for (h, bs) in bs.chunks(14).enumerate() {
        for u in (4096..=16384i64).step_by(512) {
            let sc = |v: i64| (2 * v * u + u0).div_euclid(2 * u0);
            let gs: Vec<_> = bs.iter().map(|&b| ((u / 2) as u16, vec![(0i16, sc(b) as i16, (u / 4) as i16, (sc(b) + u / 4) as i16)])).collect();
            let mut f = font_with(0x0fd2_0000 + k as u32 * 128 + h as u32 * 64 + (u / 512) as u32, u as u16, &gs);
            f.os2 = Some(4);
            f.typo = (sc(*vorg) as i16, (sc(*vorg) - sc(*vadv)) as i16, 0);
            let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case gdisideupm-{k}{}-{u}\n{}\ndwgdiglyph {pp} 1 0 1 {}\ndwgdiglyph {pp} 1 1 1 {}\ndwglyph 1 {}\nend", ["", "b"][h], font_line(&f), gids.join(" "), gids.join(" "), gids.join(" ")).unwrap();
        }
        }
    }
    // the sideways advance height's and vertical origin's em fractions alone: one glyph, every
    // size from 8 to 120 pixels per em, the advance and origin a fixed share of the em at
    // units per em from 7001 to 16384 (the same pixels: the design units back pin the 16.16
    // fraction at each size)
    let shares: [(i64, i64); 12] = [(8460, 7430), (8145, 7550), (10796, 6826), (6206, 4500), (5000, 7000), (5003, 3001), (7777, 6667), (9231, 8462), (12345, 9876), (6667, 5000), (10000, 8000), (7500, 7499)];
    for (k, &(ra, ro)) in shares.iter().enumerate() {
        for u in [16384i64, 15013, 13331, 12007, 10007, 9001, 8191, 7001] {
            let gs = vec![((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])];
            let mut f = font_with(0x0fd3_0000 + k as u32 * 64 + (u % 61) as u32, u as u16, &gs);
            f.os2 = Some(4);
            let (vadv, vorg) = ((ra * u + 5000) / 10000, (ro * u + 5000) / 10000);
            f.typo = (vorg as i16, (vorg - vadv) as i16, 0);
            write!(s, "case gdivadv-{k}-{u}\n{}\n", font_line(&f)).unwrap();
            for pp in 8..=120 {
                writeln!(s, "dwgdiglyph {pp} 1 0 1 1").unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // the sideways advance heights at 10 and 22 ppem whose em fractions are lower than at
    // other sizes, against 18 and 26: every advance of 1 to 24 pixels, centred and 0.3 pixel
    // over, each at 4 units per em (DW.GDI.GLYPH)
    for pp in [10i64, 22, 18, 26] {
        for a in 1..=24i64 {
            for (h, off) in [(0, 0i64), (1, 3)] {
                for u in [16384i64, 14009, 12011, 10007] {
                    let vadv = ((10 * a + off) * u * 2 + 10 * pp).div_euclid(20 * pp);
                    let vorg = vadv * 3 / 5;
                    // (an advance past i16 has no typographic ascender and descender)
                    if vadv > i16::MAX as i64 {
                        continue;
                    }
                    let gs = vec![((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])];
                    let mut f = font_with(0x0fd4_0000 + (pp * 256 + a * 4 + h) as u32, u as u16, &gs);
                    f.os2 = Some(4);
                    f.typo = (vorg as i16, (vorg - vadv) as i16, 0);
                    writeln!(s, "case gdivadvp-{pp}-{a}-{h}-{u}\n{}\ndwgdiglyph {pp} 1 0 1 1\nend", font_line(&f)).unwrap();
                }
            }
        }
    }
    // glyph edges exactly on half a 1/64 pixel, above and below zero, at units per em that are
    // and are not powers of 2, where the tie decides a sample row or column of the black box
    // (sideways: y, 6 samples down; horizontal: x, 6 across): which way the scaled coordinate
    // rounds (DW.GDI.GLYPH)
    for (u, em) in [(6656i64, 13i64), (13312, 13), (4608, 27), (13824, 27), (3072, 12), (12288, 24), (2048, 27), (1024, 60), (256, 54), (4096, 26), (8192, 13)] {
        let tie = |v: i64| (2 * v * em * 64).rem_euclid(2 * u) == u;
        // (the two edges a tie gives, in 1/64 sample: does a sample centre lie between?)
        let matters = |v: i64| {
            let m = (v * em * 64).div_euclid(u) * 6;
            (m..=m + 6).any(|c| (c - 32).rem_euclid(64) == 0)
        };
        let off = |v: i64, d: i64| (d..d + 64).map(|h| v + h).find(|&w| !tie(w)).unwrap();
        for axis in 0..2 {
            let mut gs = Vec::new();
            for (neg, low) in [(true, true), (true, false), (false, true), (false, false)] {
                let vs: Vec<i64> = if neg { (-u..0).rev().collect() } else { (1..u).collect() };
                for v in vs.into_iter().filter(|&v| tie(v) && matters(v)).take(3) {
                    // (the tie is the low or the high edge; the other a third of the em away)
                    let (a, b) = if low { (v, off(v, u / 3)) } else { (-off(-v, u / 3), v) };
                    let r = if axis == 0 { (0, a, u / 4, b) } else { (a, 0, b, u / 4) };
                    gs.push(((u / 2) as u16, vec![(r.0 as i16, r.1 as i16, r.2 as i16, r.3 as i16)]));
                }
            }
            let mut f = font_with(0x0fd6_0000 + (u * 4 + axis) as u32, u as u16, &gs);
            f.os2 = Some(4);
            f.typo = ((u * 3 / 4) as i16, (-u / 4) as i16, 0);
            let gids: Vec<String> = (1..=gs.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case rastertie-{}-{u}-{em}\n{}\ndwgdiglyph {em} 1 0 0 {}\ndwgdiglyph {em} 1 0 1 {}\nend", ["y", "x"][axis as usize], font_line(&f), gids.join(" "), gids.join(" ")).unwrap();
        }
    }
    // the remainders that round both ways (DW.GDI.EDGE): every k of the remainder, each at the
    // units per em that read its fraction back, as the advance and as the origin
    for (pp, t) in [(66i64, 2i64), (26, -2), (50, -2), (62, -2), (97, 1)] {
        let fu = |f: i64, u: i64| (2 * f * u + 65536).div_euclid(131072);
        let us: Vec<i64> = (4096..=16384i64).rev().step_by(257).collect();
        let mut j = 0;
        for &u in &us {
            let ks: Vec<i64> = (1..=(i16::MAX as i64 - u / 4) * pp / u).filter(|&k| 2 * (k * 65536 % pp) - pp == t && fu(k * 65536 / pp, u) != fu(k * 65536 / pp + 1, u)).take(14).collect();
            if ks.is_empty() || j >= 24 {
                continue;
            }
            let gs: Vec<_> = ks.iter().map(|_| ((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])).collect();
            let mut f = font_with(0x0fd7_0000 + (pp * 64 + j) as u32, u as u16, &gs);
            f.vertical = true;
            let sc = |k: i64| (2 * k * u + pp).div_euclid(2 * pp);
            for (i, &k) in ks.iter().enumerate() {
                f.glyphs[i + 1].vert = (sc(k) as u16, (sc(k) - u / 4) as i16);
            }
            let gids: Vec<String> = (1..=ks.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case gdivmixed-{pp}-{j}\n{}\ndwgdiglyph {pp} 1 0 1 {}\ndwgdiglyph {pp} 1 0 0 {}\nend", font_line(&f), gids.join(" "), gids.join(" ")).unwrap();
            j += 1;
        }
    }
    // the one origin that rounds by units per em (DW.GDI.EDGE): 66 pixels at 97 ppem, sideways,
    // at a fourth of the units per em of 2048 to 16384 that read its fraction back, the glyph's box and
    // advance the same share of the em
    {
        let (pp, k) = (97i64, 66i64);
        let fu = |f: i64, u: i64| (2 * f * u + 65536).div_euclid(131072);
        let f0 = k * 65536 / pp;
        let us: Vec<i64> = (2048..=16384i64).filter(|&u| fu(f0, u) != fu(f0 + 1, u)).collect();
        // (every fourth of them)
        for &u in us.iter().step_by(4) {
            {
                let gs = vec![((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])];
                let mut f = font_with(0x0fd8_0000 + u as u32, u as u16, &gs);
                f.vertical = true;
                let v = (2 * k * u + pp).div_euclid(2 * pp);
                f.glyphs[1].vert = (v as u16, (v - u / 4) as i16);
                writeln!(s, "case gdiv97-{u}\n{}\ndwgdiglyph {pp} 1 0 1 1\nend", font_line(&f)).unwrap();
            }
        }
    }
    // the same origin, with the glyph's top moved: 14 tops a unit apart about a quarter em, and
    // 14 spread over the em, at 97 of those units per em (DW.GDI.EDGE: does the top decide it?)
    {
        let (pp, k) = (97i64, 66i64);
        let fu = |f: i64, u: i64| (2 * f * u + 65536).div_euclid(131072);
        let f0 = k * 65536 / pp;
        let us: Vec<i64> = (2048..=16384i64).filter(|&u| fu(f0, u) != fu(f0 + 1, u)).collect();
        let pick: Vec<i64> = us.iter().copied().filter(|&u| u <= 9300).step_by(8).chain(us.iter().copied().filter(|&u| u > 9300).step_by(80)).collect();
        for &u in &pick {
            for (h, tops) in [(0, (0..14).map(|j| u / 4 - 7 + j).collect::<Vec<i64>>()), (1, (0..14).map(|j| u * (j + 1) / 16).collect())] {
                let gs: Vec<_> = tops.iter().map(|&t| ((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, t as i16)])).collect();
                let mut f = font_with(0x0fd9_0000 + (u * 2 + h) as u32, u as u16, &gs);
                f.vertical = true;
                let v = (2 * k * u + pp).div_euclid(2 * pp);
                for (i, &t) in tops.iter().enumerate() {
                    f.glyphs[i + 1].vert = (v as u16, (v - t) as i16);
                }
                let gids: Vec<String> = (1..=tops.len()).map(|g| g.to_string()).collect();
                writeln!(s, "case gdiv97top-{h}-{u}\n{}\ndwgdiglyph {pp} 1 0 1 {}\nend", font_line(&f), gids.join(" ")).unwrap();
            }
        }
    }
    // the same origin by the glyph's black box at 2048 units per em (the top decides it, by the
    // sixth of a pixel its edge falls in): every top a sixth apart up to 100 pixels (row), and
    // at four tops the bottom (bot) and the right edge (wid) moved a sixth at a time
    {
        let (pp, k, u) = (97i64, 66i64, 2048i64);
        let v = (2 * k * u + pp).div_euclid(2 * pp);
        // (design units of the middle of sixth j of a pixel)
        let at = |j: i64| ((2 * j - 1) * u + 6 * pp).div_euclid(12 * pp);
        let mut put = |name: String, seed: u32, boxes: &[(i64, i64, i64)]| {
            let gs: Vec<_> = boxes.iter().map(|&(y0, x1, t)| ((u / 2) as u16, vec![(0i16, y0 as i16, x1 as i16, t as i16)])).collect();
            let mut f = font_with(seed, u as u16, &gs);
            f.vertical = true;
            for (i, &(_, _, t)) in boxes.iter().enumerate() {
                f.glyphs[i + 1].vert = (v as u16, (v - t) as i16);
            }
            let gids: Vec<String> = (1..=boxes.len()).map(|g| g.to_string()).collect();
            writeln!(s, "case {name}\n{}\ndwgdiglyph {pp} 1 0 1 {}\nend", font_line(&f), gids.join(" ")).unwrap();
        };
        let rows: Vec<i64> = (1..=600).collect();
        for (i, c) in rows.chunks(14).enumerate() {
            let b: Vec<_> = c.iter().map(|&j| (0, u / 4, at(j))).collect();
            put(format!("gdiv97row-{i}"), 0x0fda_0000 + i as u32, &b);
        }
        // (tops under the baseline, the box a pixel tall: rows 1 to 84 below)
        let rows: Vec<i64> = (1..=84).collect();
        for (i, c) in rows.chunks(14).enumerate() {
            let b: Vec<_> = c.iter().map(|&j| (-at(j) - u / 97, u / 4, -at(j))).collect();
            put(format!("gdiv97neg-{i}"), 0x0fdd_0000 + i as u32, &b);
        }
        for (i, &j) in [145i64, 146, 109, 36].iter().enumerate() {
            let b: Vec<_> = (0..14).map(|y| (at(y * 6 / 5), u / 4, at(j))).collect();
            put(format!("gdiv97bot-{j}"), 0x0fdb_0000 + i as u32, &b);
            let b: Vec<_> = (0..14).map(|x| (0, at(30 + x * 25), at(j))).collect();
            put(format!("gdiv97wid-{j}"), 0x0fdc_0000 + i as u32, &b);
        }
    }
    // GetGdiCompatibleGlyphMetrics through float (re/dwrite.md): axis aligned, k whole pixels'
    // em fraction is k times 1 / (p p) times p in float32, which rounds away from the exact
    // fraction at larger k (gdihf32: the first 14 such k of each of these sizes, as the advance
    // height and the origin, horizontal), and at ppem == upm the design metrics come back as
    // they are (gdiupm)
    {
        let fixed = |f: f32| (((f * 65536.0 * 65536.0) as i64) + 0x8000) >> 16;
        let fu = |f: i64, u: i64| (2 * f * u + 65536).div_euclid(131072);
        for pp in [31i64, 45, 51, 62, 97, 113, 127, 160, 200, 255] {
            let p = pp as f32;
            let m = (1.0 / (p * p)) * p;
            let flips: Vec<i64> = (1..4096i64).filter(|&k| fixed(k as f32 * m) != (2 * k * 65536 + pp).div_euclid(2 * pp)).collect();
            // (each at the units per em that reads both fractions back, the box small)
            let mut cs = Vec::new();
            for &k in &flips {
                let e = (2 * k * 65536 + pp).div_euclid(2 * pp);
                let ok = |u: i64| fu(e, u) != fu(fixed(k as f32 * m), u) && (2 * k * u + pp).div_euclid(2 * pp) + u / 4 <= i16::MAX as i64;
                if let Some(u) = (16..=16384i64).rev().find(|&u| ok(u)) {
                    cs.push((k, u));
                }
                if cs.len() == 14 {
                    break;
                }
            }
            for (i, &(k, u)) in cs.iter().enumerate() {
                let gs = vec![((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])];
                let mut f = font_with(0x0fde_0000 + (pp * 16 + i as i64) as u32, u as u16, &gs);
                f.vertical = true;
                let v = (2 * k * u + pp).div_euclid(2 * pp);
                f.glyphs[1].vert = (v as u16, (v - u / 4) as i16);
                writeln!(s, "case gdihf32-{pp}-{k}\n{}\ndwgdiglyph {pp} 1 0 0 1\ndwgdiglyph {pp} 1 1 0 1\nend", font_line(&f)).unwrap();
            }
        }
        for u in [16i64, 64, 100, 128, 200, 255] {
            let mut f = font(0x0fdf_0000 + u as u32);
            f.upm = u as u16;
            f.vertical = true;
            for (i, g) in f.glyphs.iter_mut().enumerate().skip(1) {
                let t = (i as i64 * 37) % u;
                g.rects = vec![((i as i64 % 5) as i16, (-(i as i64) % 7) as i16, (u / 3 + i as i64) as i16, t as i16)];
                g.adv = (u / 4 + i as i64 * 3) as u16;
                g.vert = ((u + i as i64) as u16, (u / 5 - t) as i16);
            }
            let gids: Vec<String> = (1..=14).map(|g| g.to_string()).collect();
            let gl = gids.join(" ");
            writeln!(s, "case gdiupm-{u}\n{}\ndwgdiglyph {u} 1 0 0 {gl}\ndwgdiglyph {u} 1 1 0 {gl}\ndwgdiglyph {u} 1 0 1 {gl}\nend", font_line(&f)).unwrap();
        }
    }
    // past 255 ppem (re/dwrite.md): the classic advance becomes the natural one, the scale of
    // design units to 26.6 multiplies by a 16.16 ratio (half away from zero) where ppem / 2^g
    // passes 255, and at ppem == upm the design metrics come back (gdibig: random glyphs, and
    // edges on an exact half of a 1/64 pixel below zero; shapebig: placements)
    for (i, &(pp, u)) in [(256i64, 2048i64), (257, 2048), (300, 1000), (384, 2048), (511, 4096), (640, 1000), (700, 1400), (1000, 1000), (1024, 2048), (2048, 2048)].iter().enumerate() {
        let mut f = font(0x0fe0_0000 + i as u32);
        f.upm = u as u16;
        f.vertical = true;
        // (a coordinate whose 26.6 value is an exact half: v pp 64 / u = n + 1/2)
        let w = 30 * u / pp;
        let half: Vec<i64> = (-w..0).rev().filter(|&v| (2 * v * pp * 64).rem_euclid(2 * u) == u).take(7).collect();
        for (j, g) in f.glyphs.iter_mut().enumerate().skip(1) {
            let a = j as i64;
            // (a box some 30 pixels across, so the black box stays quick to draw)
            let lo = half.get(j % 7).copied().unwrap_or(-w / 3);
            g.rects = vec![(((a * 13) % w) as i16, lo as i16, (w + a * 7) as i16, (w + a * 11) as i16)];
            g.adv = (u / 3 + a * 17 + (u / pp).max(1) * a / 3) as u16;
            g.vert = ((u + a * 29) as u16, (u / 7 - a * 3) as i16);
        }
        let gl = (1..=14).map(|g| g.to_string()).collect::<Vec<_>>().join(" ");
        // (sideways too where the box is small enough to draw)
        let side = if pp <= 1024 { format!("\ndwgdiglyph {pp} 1 0 1 {gl}") } else { String::new() };
        writeln!(s, "case gdibig-{pp}-{u}\n{}\ndwgdiglyph {pp} 1 0 0 {gl}\ndwgdiglyph {pp} 1 1 0 {gl}{side}\nend", font_line(&f)).unwrap();
        let chars: Vec<String> = CHARS[..13].iter().map(|c| format!("{c:#x}")).collect();
        writeln!(s, "case shapebig-{pp}-{u}\n{}\ndwshape {pp} {}\nend", font_line(&f), chars.join(" ")).unwrap();
    }
    // every size from 1 to 255 pixels per em (the GDI compatible sizes the cases ask for): the
    // advance heights and vertical origins (vmtx) whose em fraction k * 65536 / ppem is within
    // 9/2 of 1/ppem of a half, each at a units per em where the fraction and the one above it
    // come back as different design units (DW.GDI.GLYPH: where each size rounds the half)
    for pp in 1..=255i64 {
        let g = gcd(65536, pp);
        let rs: Vec<i64> = (g..pp).step_by(g as usize).filter(|r| (2 * r - pp).abs() <= 9).collect();
        // (advance or origin, the residue k * 65536 mod ppem)
        let mut left: Vec<(bool, i64)> = [false, true].iter().flat_map(|&o| rs.iter().map(move |&r| (o, r))).collect();
        let fu = |f: i64, u: i64| (2 * f * u + 65536).div_euclid(131072);
        // (the least k of that residue whose fraction reads back at u)
        // (k of one residue repeat every ppem / g)
        let k0: Vec<i64> = (0..pp).map(|r| (1..=pp).find(|&k| k * 65536 % pp == r).unwrap_or(0)).collect();
        let at = |r: i64, u: i64| (k0[r as usize]..=(i16::MAX as i64 - u / 4) * pp / u).step_by((pp / g) as usize).find(|&k| fu(k * 65536 / pp, u) != fu(k * 65536 / pp + 1, u));
        let mut j = 0;
        while !left.is_empty() {
            // (the largest units per em that reads back the most of them: the first that reads all)
            let mut best = (0, 0);
            for u in (2048..=16384i64).rev() {
                let n = left.iter().filter(|t| at(t.1, u).is_some()).count();
                if n > best.1 {
                    best = (u, n);
                }
                if n == left.len() {
                    break;
                }
            }
            if best.1 == 0 {
                // (a residue no units per em reads back: its fraction never shows)
                break;
            }
            let u = best.0;
            let (got, rest): (Vec<_>, Vec<_>) = left.iter().partition(|t| at(t.1, u).is_some());
            left = rest;
            let ka: Vec<i64> = got.iter().filter(|t| !t.0).map(|t| at(t.1, u).unwrap()).collect();
            let kv: Vec<i64> = got.iter().filter(|t| t.0).map(|t| at(t.1, u).unwrap()).collect();
            let n = ka.len().max(kv.len());
            let pairs: Vec<(i64, i64)> = (0..n).map(|i| (*ka.get(i).unwrap_or(&1), *kv.get(i).unwrap_or(&1))).collect();
            for c in pairs.chunks(14) {
                let gs: Vec<_> = c.iter().map(|_| ((u / 2) as u16, vec![(0i16, 0i16, (u / 4) as i16, (u / 4) as i16)])).collect();
                let mut f = font_with(0x0fd5_0000 + (pp * 64 + j) as u32, u as u16, &gs);
                f.vertical = true;
                let sc = |k: i64| (2 * k * u + pp).div_euclid(2 * pp);
                for (i, &(a, v)) in c.iter().enumerate() {
                    f.glyphs[i + 1].vert = (sc(a) as u16, (sc(v) - u / 4) as i16);
                }
                let gids: Vec<String> = (1..=c.len()).map(|g| g.to_string()).collect();
                writeln!(s, "case gdivtie-{pp}-{j}\n{}\ndwgdiglyph {pp} 1 0 1 {}\nend", font_line(&f), gids.join(" ")).unwrap();
                j += 1;
            }
        }
    }
}
