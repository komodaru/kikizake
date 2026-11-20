//! The model of runner/gditext.c's ops (spec/gditext.txt): the text state of a DC (alignment,
//! character extra) and ExtTextOut's rectangle. What a font looks like and measures is
//! Windows' fonts' own: those cases are compared with Windows only.
//!
//! Sources (Microsoft Learn): "SetTextAlign", "SetTextCharacterExtra", "ExtTextOutA",
//! "TextOutA", "CreateFontA", "GetTextMetrics", "GetTextExtentPoint32", "GetCharWidth32",
//! "GetCharABCWidths", "GetGlyphOutline", "GetTextFace".

use crate::spec::{clause, open};
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;
use super::gdifont::{self, Slant, TextFont, Weight};

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// a text token's characters ('_' a space); None past ASCII (the code page is not modelled)
fn chars(t: &str) -> Option<Vec<u32>> {
    t.bytes().map(|b| if b == b'_' { Some(0x20) } else if b.is_ascii() { Some(b as u32) } else { None }).collect()
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if t0 == "memfont" {
        // (MEASURED: a font file the suite writes adds one font)
        let b = op.tok.get(1).and_then(|h| super::dwrite::hex(h)).ok_or(Fail::Bad)?;
        let p = crate::ttf::parse(&b).ok_or(Fail::Unmodeled(open::GDI_TEXT_FONT))?;
        hit(clause::GDI_TEXT_MEMFONT);
        g.fonts.push(p);
        out.push("memfont 1".into());
        return Ok(true);
    }
    if !matches!(t0, "font" | "metrics" | "face" | "extent" | "extentex" | "widths" | "abc" | "textout" | "exttext" | "align" | "extra" | "glyph") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    let fonts = &g.fonts;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
    let sel = |sl: &super::gdi::Slot| -> Res<(&crate::ttf::Parsed, TextFont)> {
        let f = sl.ext.font.ok_or(Fail::Unmodeled(open::GDI_TEXT_FONT))?;
        if sl.ext.mapped() {
            return Err(Fail::Unmodeled(open::GDI_TEXT_FONT));
        }
        Ok((fonts.get(f.file).ok_or(Fail::Bad)?, f))
    };
    let unm = || Fail::Unmodeled(open::GDI_TEXT_FONT);
    match t0 {
        "align" => {
            let v = n(op, 2)?;
            hit(clause::GDI_TEXT_ALIGN);
            out.push(format!("align {}", std::mem::replace(&mut sl.ext.align, v)));
        }
        "extra" => {
            let v = n(op, 2)?;
            hit(clause::GDI_TEXT_EXTRA);
            out.push(format!("extra {}", std::mem::replace(&mut sl.ext.extra, v)));
        }
        "exttext" if op.tok.len() == 9 => {
            let (o, l, t, r, b) = (n(op, 4)?, n(op, 5)? as i32, n(op, 6)? as i32, n(op, 7)? as i32, n(op, 8)? as i32);
            if sl.ext.mapped() || sl.ext.align & 1 != 0 {
                return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
            }
            hit(clause::GDI_TEXT_NOTEXT);
            if o & 2 != 0 {
                hit(clause::GDI_TEXT_OPAQUE);
                if l > r || t > b {
                    // (MEASURED: succeeds; the rectangle's corners in either order)
                    hit(clause::GDI_TEXT_OPAQUEORDER);
                }
                let (l, r, t, b) = (l.min(r), l.max(r), t.min(b), t.max(b));
                let v = sl.s.from_rgb(sl.bk);
                for y in t.max(0)..b.min(sl.s.h) {
                    for x in l.max(0)..r.min(sl.s.w) {
                        if sl.visible(x, y) {
                            sl.s.set(x, y, v);
                        }
                    }
                }
            }
            out.push("ret 1".into());
        }
        "font" if op.tok.len() >= 9 => {
            let (h, w, e, wt, it, q) = (n(op, 2)?, n(op, 3)?, n(op, 4)?, n(op, 5)?, n(op, 6)?, n(op, 7)?);
            let face = op.tok[8..].join(" ");
            let file = fonts.iter().position(|p| p.family == face).ok_or(unm())?;
            let weight = Weight::of(wt).ok_or(unm())?;
            let slant = Slant::of(it).ok_or(unm())?;
            // (the suite's requests: no width or escapement, CLEARTYPE_QUALITY)
            if (w, e, q) != (0, 0, 3) {
                return Err(unm());
            }
            hit(clause::GDI_TEXT_MEMFONT);
            sl.ext.font = Some(TextFont { file, height: h, weight, slant });
        }
        "face" => {
            let (p, _) = sel(sl)?;
            hit(clause::GDI_TEXT_FACE);
            out.push(format!("face {} {}", p.family.chars().count(), p.family.replace(' ', "_")));
        }
        "metrics" => {
            let (p, f) = sel(sl)?;
            let t = gdifont::metrics(p, f).ok_or(unm())?;
            hit(clause::GDI_TEXT_TM);
            hit(clause::GDI_TEXT_AVE);
            out.push(format!("tm 1 h {} asc {} desc {} int {} ext {} ave {} max {} weight {} over 0 dig 96 96 chars 30 255 31 32 it {} ul 0 st 0 pitch {} charset 0", t.height, t.ascent, t.descent, t.internal, t.external, t.ave, t.max, t.weight, t.italic, t.pitch));
        }
        "widths" | "abc" => {
            let (p, f) = sel(sl)?;
            if f.slant == Slant::Italic {
                hit(clause::GDI_TEXT_ITALIC);
            }
            let (a, b) = (n(op, 2)?, n(op, 3)?);
            if b < a || b - a > 255 || a < 0 || b > 0x7f {
                return Err(unm());
            }
            let mut s = format!("{t0} 1");
            for c in a as u32..=b as u32 {
                if t0 == "widths" {
                    hit(clause::GDI_TEXT_WIDTH);
                    s += &format!(" {}", gdifont::width(p, f, c).ok_or(unm())?);
                } else {
                    let w = gdifont::abc(p, f, c).ok_or(unm())?;
                    hit(match w.black {
                        gdifont::Black::Box => clause::GDI_TEXT_ABC,
                        gdifont::Black::Outside => clause::GDI_TEXT_ABCEMPTY,
                    });
                    s += &format!(" {},{},{}", w.a, w.b, w.c);
                }
            }
            out.push(s);
        }
        "extent" => {
            let (p, f) = sel(sl)?;
            let t = chars(op.tok.get(2).ok_or(Fail::Bad)?).ok_or(unm())?;
            let (x, y) = gdifont::extent(p, f, &t, sl.ext.extra).ok_or(unm())?;
            if sl.ext.extra != 0 {
                if f.slant == Slant::Italic {
                    // (italic with a character extra: a character wider and the height a pixel
                    // less than the model's in mext-11, mext-22, mext-27, open)
                    return Err(unm());
                }
                hit(clause::GDI_TEXT_EXTRAEXTENT);
            }
            hit(clause::GDI_TEXT_EXTENT);
            out.push(format!("extent 1 {x} {y}"));
        }
        "extentex" => {
            let (p, f) = sel(sl)?;
            let m = n(op, 2)?;
            let t = chars(op.tok.get(3).ok_or(Fail::Bad)?).ok_or(unm())?;
            let (fit, dx, (x, y)) = gdifont::extent_ex(p, f, &t, m, sl.ext.extra).ok_or(unm())?;
            if sl.ext.extra != 0 {
                if f.slant == Slant::Italic {
                    // (italic with a character extra: a character wider and the height a pixel
                    // less than the model's in mext-11, mext-22, mext-27, open)
                    return Err(unm());
                }
                hit(clause::GDI_TEXT_EXTRAEXTENT);
            }
            hit(clause::GDI_TEXT_EXTENT);
            let mut s = format!("extentex 1 {fit} {x} {y}");
            for v in dx {
                s += &format!(" {v}");
            }
            out.push(s);
        }
        "glyph" => {
            let (p, f) = sel(sl)?;
            if f.slant == Slant::Italic {
                hit(clause::GDI_TEXT_ITALIC);
            }
            let (c, fmt) = (n(op, 2)?, n(op, 3)?);
            let fmt = gdifont::Format::of(fmt).ok_or(unm())?;
            if !(0..0x80).contains(&c) {
                return Err(unm());
            }
            let b = gdifont::glyph(p, f, c as u32, fmt).ok_or(unm())?;
            hit(match fmt {
                gdifont::Format::Metrics | gdifont::Format::Mono => clause::GDI_TEXT_BITMAP,
                gdifont::Format::Gray2 | gdifont::Format::Gray4 | gdifont::Format::Gray8 => clause::GDI_TEXT_GRAY,
            });
            out.push(format!("glyph {} {} {} {} {} {} 0", b.size, b.black.0, b.black.1, b.origin.0, b.origin.1, b.inc));
            if !b.bytes.is_empty() {
                out.push(b.bytes.iter().map(|x| format!("{x:02x}")).collect());
            }
        }
        "textout" if sl.ext.font.is_some() => {
            let (p, f) = sel(sl)?;
            let (x, y) = (n(op, 2)?, n(op, 3)?);
            let t = chars(op.tok.get(4).ok_or(Fail::Bad)?).ok_or(unm())?;
            // (the suite's state: TA_TOP | TA_LEFT, TRANSPARENT)
            if sl.ext.align != 0 || sl.bkmode != 1 || sl.ext.extra != 0 {
                return Err(unm());
            }
            let px = gdifont::text_pixels(p, f, &t, x, y).ok_or(unm())?;
            hit(clause::GDI_TEXT_DRAW);
            let v = sl.s.from_rgb(sl.text);
            for (x, y) in px {
                let (x, y) = (x as i32, y as i32);
                if sl.visible(x, y) {
                    sl.s.set(x, y, v);
                }
            }
            out.push("ret 1".into());
        }
        _ => return Err(unm()),
    }
    Ok(true)
}
