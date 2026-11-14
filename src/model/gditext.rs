//! The model of runner/gditext.c's ops (spec/gditext.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "font" | "metrics" | "face" | "extent" | "extentex" | "widths" | "abc" | "textout" | "exttext" | "align" | "extra" | "glyph" => Err(Fail::Unmodeled("GDI.TEXT.OPEN")),
        _ => Ok(false),
    }
}
