//! The text suite: fonts (metrics, faces, widths, glyphs) and text drawn into DIB sections.
use super::*;

const FACES: &[&str] = &["Arial", "Courier New", "Times New Roman", "Tahoma", "Segoe UI", "Consolas", "System", "Fixedsys", "Terminal", "MS Sans Serif", "Small Fonts", "No Such Face"];

pub fn text() -> String {
    let mut s = String::from("# kikizake suite text: GDI fonts and text (SPEC.md). Made by `kikizake gen text`.\n");
    let mut r = XorShift32::new(0x7e47_0000);
    // the DC's defaults (the System font), text alignment and character extra
    writeln!(s, "case state\ndib 0 8 8 32\nalign 0 24\nalign 0 0\nalign 0 1\nextra 0 3\nextra 0 -2\nextra 0 0\nend").unwrap();
    writeln!(s, "case defaults\ndib 0 8 8 32\nface 0\nmetrics 0\nalign 0 24\nalign 0 0\nextra 0 3\nextra 0 0\nobjtype 0 font\nend").unwrap();
    // each face at several heights: metrics, face, widths, ABC widths, extents
    for (fi, f) in FACES.iter().enumerate() {
        for h in [-8, -11, -13, -16, 12, 20, 0] {
            writeln!(s, "case font-{fi}-{h}\ndib 0 8 8 32\nfont 0 {h} 0 0 400 0 3 {f}\nface 0\nmetrics 0\nwidths 0 32 126\nabc 0 32 126").unwrap();
            writeln!(s, "extent 0 Hello,_world!\nextent 0 iiiWWW\nextent 0 _\nextentex 0 40 Hello,_world!\nextra 0 2\nextent 0 iiiWWW\nend").unwrap();
        }
        writeln!(s, "case bold-{fi}\ndib 0 8 8 32\nfont 0 -13 0 0 700 1 3 {f}\nface 0\nmetrics 0\nwidths 0 65 90\nextent 0 Bold_Italic\nend").unwrap();
    }
    // glyphs: metrics, 1 bpp and 8 level bitmaps, and outlines' sizes
    for f in ["Arial", "Courier New", "Times New Roman"] {
        for h in [-9, -16] {
            writeln!(s, "case glyph-{}-{h}\ndib 0 8 8 32\nfont 0 {h} 0 0 400 0 3 {f}", f.replace(' ', "")).unwrap();
            for ch in [b'A', b'g', b'.', b'W', b' ', b'@'] {
                for fmt in [0, 1, 4, 5, 6] {
                    writeln!(s, "glyph 0 {ch} {fmt}").unwrap();
                }
            }
            writeln!(s, "end").unwrap();
        }
    }
    // text drawn: TextOut and ExtTextOut, opaque and transparent, every alignment, clipping
    for i in 0..80 {
        writeln!(s, "case draw-{i}").unwrap();
        let f = FACES[r.range(0, 8) as usize];
        let bpp = *r.pick(&[1, 8, 32]);
        writeln!(s, "dib 0 48 20 {bpp}\nfont 0 {} 0 0 {} 0 3 {f}", r.range(-16, -8), *r.pick(&[400, 700])).unwrap();
        writeln!(s, "text 0 #ffffff\nbk 0 #{:06x}\nbkmode 0 {}", if bpp == 1 { 0 } else { r.next() & 0xffffff }, r.range(1, 2)).unwrap();
        if r.chance(2) {
            writeln!(s, "align 0 {}", *r.pick(&[0, 2, 6, 8, 24, 1, 9, 26])).unwrap();
        }
        let t = *r.pick(&["Ag", "Hello", "W.i", "kikizake", "|_|", "0123"]);
        let (x, y) = (r.range(-4, 40), r.range(-4, 18));
        if r.chance(2) {
            writeln!(s, "textout 0 {x} {y} {t}").unwrap();
        } else {
            let (a, b) = (r.range(-2, 30), r.range(-2, 14));
            writeln!(s, "exttext 0 {x} {y} {} {a} {b} {} {} {t}", *r.pick(&[0, 2, 4, 6]), a + r.range(0, 30), b + r.range(0, 14)).unwrap();
        }
        writeln!(s, "getpos 0\ndump 0\nend").unwrap();
    }
    // ExtTextOut with no text: only the rectangle (ETO_OPAQUE fills it with the background color)
    for i in 0..24 {
        let (a, b) = (r.range(-3, 10), r.range(-3, 8));
        writeln!(s, "case opaque-{i}\ndib 0 12 8 32\nfill 0 {}\nbk 0 #{:06x}\nexttext 0 {} {} {} {a} {b} {} {}\ndump 0\nend", r.next(), r.next() & 0xffffff, r.range(0, 5), r.range(0, 5), *r.pick(&[0, 2, 4, 6]), a + r.range(-2, 10), b + r.range(-2, 8)).unwrap();
    }
    // TA_UPDATECP: the current position moves with the text
    writeln!(s, "case updatecp\ndib 0 64 16 1\nfont 0 -11 0 0 400 0 3 Arial\nalign 0 1\nmoveto 0 2 2\ntextout 0 0 0 ab\ngetpos 0\ntextout 0 0 0 cd\ngetpos 0\nexttext 0 0 0 0 0 0 0 0 ef\ngetpos 0\ndump 0\nend").unwrap();
    s
}
