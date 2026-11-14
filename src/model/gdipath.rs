//! The model of runner/gdipath.c's ops (spec/gdipath.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "beginpath" | "endpath" | "abortpath" | "closefig" | "flatten" | "widen" | "strokepath" | "fillpath" | "strokefill" | "selclippath" | "getpath" | "pathrgn" | "miter" => Err(Fail::Unmodeled("GDI.PATH.OPEN")),
        _ => Ok(false),
    }
}
