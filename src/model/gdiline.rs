//! The model of runner/gdiline.c's ops (spec/gdiline.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "pen" | "extpen" | "moveto" | "getpos" | "lineto" | "polyline" | "polylineto" | "polygon" | "polybezier" | "polybezierto" | "rectangle" | "ellipse" | "roundrect" | "arc" | "arcto" | "pie" | "chord" | "fillrect" | "framerect" | "invertrect" | "rop2" | "polyfill" | "arcdir" => Err(Fail::Unmodeled("GDI.LINE.OPEN")),
        _ => Ok(false),
    }
}
