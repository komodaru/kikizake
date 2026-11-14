//! The model of runner/gdiddb.c's ops (spec/gdiddb.txt).
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::Op;

pub fn op(_g: &mut Gdi, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    match op.tok[0].as_str() {
        "ddb" | "bmobject" | "bmbits" | "devcaps" | "objtype" => Err(Fail::Unmodeled("GDI.DDB.OPEN")),
        _ => Ok(false),
    }
}
