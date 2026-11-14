//! The model of runner/gdixform.c's ops (spec/gdixform.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "mapmode" | "gmode" | "winext" | "vpext" | "winorg" | "vporg" | "offsetwin" | "offsetvp" | "scalewin" | "scalevp" | "getmap" | "lptodp" | "dptolp" | "setxform" | "modxform" | "getxform" | "combinexform" => Err(Fail::Unmodeled("GDI.XFORM.OPEN")),
        _ => Ok(false),
    }
}
