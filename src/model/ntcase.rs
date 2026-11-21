//! The ntcase suite's model: ntdll's case tables (spec/ntcase.txt): the measured table of
//! one-to-one pairs (ntcase_table.rs, a subset of Unicode's simple mappings) and towlower's
//! Latin-1 rule.
use super::{Api, Fail, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit};

fn pair(t: &[(u16, u16)], u: u16) -> u16 {
    t.binary_search_by_key(&u, |p| p.0).ok().and_then(|i| t.get(i)).map_or(u, |p| p.1)
}
pub fn upcase(u: u16) -> u16 {
    hit(clause::NT_CASE_UP);
    pair(&super::ntcase_table::BY_LOWER, u)
}
pub fn downcase(u: u16) -> u16 {
    hit(clause::NT_CASE_DOWN);
    pair(&super::ntcase_table::BY_UPPER, u)
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
