//! The ntcase suite's model: ntdll's case tables (spec/ntcase.txt). Unicode's simple mappings,
//! one BMP unit to one, until Windows' answers name the exceptions.
use super::{Api, Fail, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit};

/// a unit's simple case mapping by `f` where it is one BMP unit, else the unit
fn simple(u: u16, f: fn(char) -> Vec<char>) -> u16 {
    match char::from_u32(u32::from(u)).map(f).as_deref() {
        Some(&[c]) if (c as u32) < 0x10000 => c as u16,
        _ => u,
    }
}
pub fn upcase(u: u16) -> u16 {
    hit(clause::NT_CASE_UP);
    simple(u, |c| c.to_uppercase().collect())
}
pub fn downcase(u: u16) -> u16 {
    hit(clause::NT_CASE_DOWN);
    simple(u, |c| c.to_lowercase().collect())
}
pub fn towupper(u: u16) -> u16 {
    hit(clause::NT_CASE_TOWUPPER);
    upcase(u)
}
pub fn towlower(u: u16) -> u16 {
    hit(clause::NT_CASE_TOWLOWER);
    if (0x41..=0x5a).contains(&u) || ((0xc0..=0xde).contains(&u) && u != 0xd7) { u + 0x20 } else { u }
}

pub struct Ntcase;

impl Api for Ntcase {
    fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        if op.tok[0] != "tab" {
            return Ok(false);
        }
        let f: fn(u16) -> u16 = match op.tok.get(1).map(String::as_str) {
            Some("up") => upcase,
            Some("down") => downcase,
            Some("towupper") => towupper,
            Some("towlower") => towlower,
            _ => return Err(Fail::Bad),
        };
        let block = op.tok.get(2).and_then(|t| num(t)).filter(|b| (0..=255).contains(b)).ok_or(Fail::Bad)? as u16;
        let units: String = (0..256u16).map(|k| format!("{:04x}", f(block * 256 + k))).collect();
        out.push(format!("tab {} {units}", op.tok[1]));
        Ok(true)
    }
}
