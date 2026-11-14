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
    let gids = (0..=CHARS.len() + 1).map(|g| g.to_string()).collect::<Vec<_>>().join(" ");
    // a face's metrics: each font, its info, design and GDI compatible metrics
    for i in 0..120u32 {
        let f = font(0x0f00_0000 + i);
        writeln!(s, "case face-{i}\n{}\ndwinfo\ndwmetrics", font_line(&f)).unwrap();
        for _ in 0..3 {
            writeln!(s, "dwgdimetrics {} {}", r.range(1, 80), *r.pick(&[1, 1, 2, 3])).unwrap();
        }
        let cps: Vec<String> = CHARS.iter().chain(&[0x46, 0x5d1, 0x1f601, 0x10ffff, 0]).map(|c| format!("{c:#x}")).collect();
        writeln!(s, "dwcmap {}\ndwglyph 0 {gids}\ndwglyph 1 {gids}", cps.join(" ")).unwrap();
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
    for i in 0..60u32 {
        let f = font(0x0f20_0000 + i);
        writeln!(s, "case run-{i}\n{}", font_line(&f)).unwrap();
        for mode in ["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural"] {
            let g = r.range(0, 8);
            writeln!(s, "dwrun {mode} {} {} {} {g} 0", r.range(4, 40), r.range(-8, 40), r.range(16, 96)).unwrap();
        }
        let mode = *r.pick(&["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural"]);
        let n = r.range(2, 5);
        let run: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(0, 8), r.range(0, 60))).collect();
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
    s
}
