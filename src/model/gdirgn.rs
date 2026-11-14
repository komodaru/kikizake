//! The model of runner/gdirgn.c's ops (spec/gdirgn.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "rgn" | "setrect" | "combine" | "offsetrgn" | "rgnbox" | "ptin" | "rectin" | "equal" | "rgndata" | "fillrgn" | "framergn" | "invertrgn" | "paintrgn" | "selclip" | "clipbox" | "getcliprgn" | "exclude" | "offsetclip" | "ptvisible" | "rectvisible" => Err(Fail::Unmodeled("GDI.RGN.OPEN")),
        _ => Ok(false),
    }
}
