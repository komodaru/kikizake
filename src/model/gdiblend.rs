//! The model of runner/gdiblend.c's ops (spec/gdiblend.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "alphablend" | "transparent" | "gradrect" | "gradtri" | "maskblt" | "plgblt" => Err(Fail::Unmodeled("GDI.BLEND.OPEN")),
        _ => Ok(false),
    }
}
