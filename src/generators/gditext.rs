//! The text suite: fonts (metrics, faces, widths, glyphs) and text drawn into DIB sections.
use super::*;

/// a font the suite writes (rectangle glyphs, every table field chosen, as the dwrite suite's):
/// AddFontMemResourceEx of its file and CreateFontA by its family. Windows' own fonts are not
/// asked: what they measure and draw is their own data, not GDI's rules
fn mem_font(seed: u32, h: i64, wt: i64, it: i64) -> String {
    let f = super::dwrite::font(seed);
    let hex: String = f.bytes().iter().map(|b| format!("{b:02x}")).collect();
    format!("memfont {hex}\nfont 0 {h} 0 0 {wt} {it} 3 {}", f.family)
}

/// strings of the characters the suite's fonts map (and '_', a space)
const TEXTS: &[&str] = &["ABab", "Ab_01", "a-b", "E0D1", "BAD", "-_-", "e", "CAB_E"];

pub fn text() -> String {
    let mut s = String::from("# kikizake suite text: GDI fonts and text (SPEC.md). Made by `kikizake gen text`.\n");
    let mut r = XorShift32::new(0x7e47_0000);
    // the DC's defaults (the System font), text alignment and character extra
    writeln!(s, "case state\ndib 0 8 8 32\nalign 0 24\nalign 0 0\nalign 0 1\nextra 0 3\nextra 0 -2\nextra 0 0\nend");
    writeln!(s, "case defaults\ndib 0 8 8 32\nalign 0 24\nalign 0 0\nextra 0 3\nextra 0 0\nobjtype 0 font\nend");
    // text drawn: TextOut and ExtTextOut, opaque and transparent, every alignment, clipping,
    // in one of 8 fonts (italic: the last two)
    for i in 0..80 {
        writeln!(s, "case mdraw-{i}");
        let fi = r.range(0, 7);
        let bpp = *r.pick(&[1, 8, 32]);
        let (h, wt) = (r.range(-16, -8), *r.pick(&[400, 700]));
        writeln!(s, "dib 0 48 20 {bpp}\n{}", mem_font(0x7e47_3000 + fi as u32, h, wt, if fi >= 6 { 1 } else { 0 }));
        writeln!(s, "text 0 #ffffff\nbk 0 #{:06x}\nbkmode 0 {}", if bpp == 1 { 0 } else { r.next() & 0xffffff }, r.range(1, 2));
        if r.chance(2) {
            writeln!(s, "align 0 {}", *r.pick(&[0, 2, 6, 8, 24, 1, 9, 26]));
        }
        let t = *r.pick(&TEXTS[..6]);
        let (x, y) = (r.range(-4, 40), r.range(-4, 18));
        if r.chance(2) {
            writeln!(s, "textout 0 {x} {y} {t}");
        } else {
            let (a, b) = (r.range(-2, 30), r.range(-2, 14));
            writeln!(s, "exttext 0 {x} {y} {} {a} {b} {} {} {t}", *r.pick(&[0, 2, 4, 6]), a + r.range(0, 30), b + r.range(0, 14));
        }
        writeln!(s, "getpos 0\ndump 0\nend");
    }
    // ExtTextOut with no text: only the rectangle (ETO_OPAQUE fills it with the background color)
    for i in 0..24 {
        let (a, b) = (r.range(-3, 10), r.range(-3, 8));
        writeln!(s, "case opaque-{i}\ndib 0 12 8 32\nfill 0 {}\nbk 0 #{:06x}\nexttext 0 {} {} {} {a} {b} {} {}\ndump 0\nend", r.next(), r.next() & 0xffffff, r.range(0, 5), r.range(0, 5), *r.pick(&[0, 2, 4, 6]), a + r.range(-2, 10), b + r.range(-2, 8));
    }
    // TA_UPDATECP: the current position moves with the text
    writeln!(s, "case updatecp\ndib 0 64 16 1\n{}\nalign 0 1\nmoveto 0 2 2\ntextout 0 0 0 ab\ngetpos 0\ntextout 0 0 0 AB\ngetpos 0\nexttext 0 0 0 0 0 0 0 0 E-\ngetpos 0\ndump 0\nend", mem_font(0x7e47_3fff, -11, 400, 0));
    // fonts made for the suite (rectangle glyphs, every table field chosen, as the dwrite
    // suite's), added with AddFontMemResourceEx and taken by their family name: what GDI
    // measures and draws from a font's own tables (their own stream)
    let mut r = XorShift32::new(0x7e47_f0f0);
    let mut glyphs = String::new();
    for i in 0..48 {
        let f = super::dwrite::font(0x7e47_1000 + i);
        let fam = &f.family;
        let hex: String = f.bytes().iter().map(|b| format!("{b:02x}")).collect();
        let h = *r.pick(&[-8, -9, -11, -12, -13, -16, -20, -32, 10, 13, 16, 24, 0]);
        let wt = *r.pick(&[400, 400, 700]);
        let font = format!("memfont {hex}\nfont 0 {h} 0 0 {wt} 0 3 {fam}");
        writeln!(s, "case mfont-{i}\ndib 0 40 24 32\n{font}\nface 0\nmetrics 0\nwidths 0 32 98\nabc 0 48 69\nextent 0 ABab01\nextentex 0 30 ABCDE\nend");
        // (the same font drawn: its glyphs' outlines and bitmaps, and TextOut)
        writeln!(glyphs, "case mglyph-{i}\ndib 0 40 24 32\n{font}");
        for ch in [b'A', b'b', b'0', b' '] {
            for fmt in [0, 1, 6] {
                writeln!(glyphs, "glyph 0 {ch} {fmt}");
            }
        }
        writeln!(glyphs, "text 0 #ffffff\nbk 0 #000000\nbkmode 0 1\ntextout 0 {} {} ABab01-\ndump 0\nend", r.range(-2, 6), r.range(-2, 8));
    }
    s += &glyphs;
    // measuring with character extra and italic; the gray levels GGO_GRAY2 and GGO_GRAY4
    let mut r = XorShift32::new(0x7e47_e0e0);
    for i in 0..32 {
        let h = *r.pick(&[-8, -11, -13, -16, -20, 12, 16, 0]);
        let (wt, it) = (*r.pick(&[400, 400, 700]), if r.chance(2) { 1 } else { 0 });
        let extra = r.range(-3, 5);
        writeln!(s, "case mext-{i}\ndib 0 40 24 32\n{}\nmetrics 0\nwidths 0 32 98\nabc 0 48 69\nextent 0 ABab01\nextra 0 {extra}\nextent 0 ABab01\nextentex 0 30 ABCDE\nextent 0 _\nend", mem_font(0x7e47_2000 + i, h, wt, it));
        writeln!(s, "case mgray-{i}\ndib 0 40 24 32\n{}", mem_font(0x7e47_2000 + i, h, wt, it));
        for ch in [b'A', b'b', b'0', b' '] {
            for fmt in [1, 4, 5, 6] {
                writeln!(s, "glyph 0 {ch} {fmt}");
            }
        }
        writeln!(s, "end");
    }
    s
}
