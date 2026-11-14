//! AnalyzeScript: script runs (Unicode Scripts.txt; ISO 15924 codes as GetScriptProperties gives
//! them), common and inherited characters taking the script
//! of the run they are in.
use crate::spec::{clause, open};
use super::super::{Fail, Res};
use crate::spec::hit;

/// (ISO 15924 code, number, shapes): None for Common and Inherited (Scripts.txt)
pub fn script(c: u32) -> Res<Option<(&'static str, u32, u32)>> {
    match crate::ucd::scripts().get(c) {
        Some("Common" | "Inherited") => Ok(None),
        Some(long) => {
            hit(clause::DW_SCRIPT_UCD);
            let (code, num) = crate::ucd::iso15924(long).ok_or(Fail::Unmodeled(open::DW_SCRIPT_CHAR))?;
            Ok(Some((code, num, 0)))
        }
        None => Err(Fail::Unmodeled(open::DW_SCRIPT_CHAR)),
    }
}

/// the runs (UTF-16 position, length, script) of a text of code points
pub fn runs(cps: &[u32]) -> Res<Vec<(usize, usize, (&'static str, u32, u32))>> {
    let mut v: Vec<(usize, usize, (&'static str, u32, u32))> = Vec::new();
    let mut pending = 0; // (leading common characters' UTF-16 units)
    let mut pos = 0;
    for &c in cps {
        let n = if c >= 0x10000 { 2 } else { 1 };
        match script(c)? {
            None => match v.last_mut() {
                Some(l) => {
                    hit(clause::DW_SCRIPT_COMMON);
                    l.1 += n
                }
                None => pending += n,
            },
            Some(s) => match v.last_mut() {
                Some(l) if l.2 == s => l.1 += n,
                _ => {
                    hit(clause::DW_SCRIPT_RUN);
                    v.push((pos - pending, n + pending, s));
                    pending = 0;
                }
            },
        }
        pos += n;
    }
    if v.is_empty() {
        // MEASURED: text of common and inherited characters only is one run of Zzzz (unknown)
        hit(clause::DW_SCRIPT_ALLCOMMON);
        v.push((0, pos, ("Zzzz", 999, 0)));
    }
    Ok(v)
}
