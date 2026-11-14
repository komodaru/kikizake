//! D3DX 9 (runner/d3dx9.c).
use super::{Api, Fail, Res};
use crate::case::Op;

#[derive(Default)]
pub struct D3dx9;

impl Api for D3dx9 {
    fn op(&mut self, op: &Op, _out: &mut Vec<String>) -> Res<bool> {
        if op.tok[0].starts_with("x9") {
            return Err(Fail::Unmodeled("D3DX9.PENDING"));
        }
        Ok(false)
    }
}
