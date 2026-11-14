//! DirectWrite: an executable specification of font faces (from the font file a case gives),
//! shaping, glyph run rasterization, text analysis and text layout.
use super::{Fail, Res};
use crate::spec::hit;

#[derive(Default)]
pub struct Dwrite;

impl Dwrite {
    pub fn op(&mut self, op: &crate::case::Op, _out: &mut Vec<String>) -> Res<bool> {
        let id = match op.tok[0].as_str() {
            // (the font alone: the next line says what is unmodelled)
            "dwfont" => return Ok(true),
            "dwinfo" | "dwmetrics" | "dwgdimetrics" | "dwcmap" | "dwglyph" | "dwgdiglyph" => "DW.FACE",
            "dwshape" => "DW.SHAPE",
            "dwrun" => "DW.RUN",
            "dwlayout" => "DW.LAYOUT",
            "dwscript" => "DW.SCRIPT",
            "dwbidi" => "DW.BIDI",
            "dwbreak" => "DW.BREAK",
            _ => return Ok(false),
        };
        hit(id);
        Err(Fail::Unmodeled(id))
    }
}
