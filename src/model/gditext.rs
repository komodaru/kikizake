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

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if t0 == "memfont" {
        // (a font file the case makes, added for the case: what it measures and draws is open)
        return Err(Fail::Unmodeled(open::GDI_TEXT_FONT));
    }
    if !matches!(t0, "font" | "metrics" | "face" | "extent" | "extentex" | "widths" | "abc" | "textout" | "exttext" | "align" | "extra" | "glyph") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
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
        _ => return Err(Fail::Unmodeled(open::GDI_TEXT_FONT)),
    }
    Ok(true)
}
