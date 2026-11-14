//! AnalyzeScript: script runs (Unicode Scripts.txt for the characters the suite uses; ISO 15924
//! codes as GetScriptProperties gives them), common and inherited characters taking the script
//! of the run they are in.
use crate::spec::{clause, open};
use super::super::{Fail, Res};
use crate::spec::hit;

/// (ISO 15924 code, number, shapes): None for Common and Inherited
pub fn script(c: u32) -> Res<Option<(&'static str, u32, u32)>> {
    Ok(Some(match c {
        0x41..=0x5a | 0x61..=0x7a => ("Latn", 215, 0),
        0x5d0..=0x5ea => ("Hebr", 125, 0),
        0x620..=0x64a => ("Arab", 160, 0),
        0x391..=0x3c9 => ("Grek", 200, 0),
        0x410..=0x44f => ("Cyrl", 220, 0),
        0x4e00..=0x9fff => ("Hani", 500, 0),
        0x3041..=0x3096 => ("Hira", 410, 0),
        0x30a1..=0x30fa => ("Kana", 411, 0),
        0xac00..=0xd7a3 => ("Hang", 286, 0),
        0xe01..=0xe3a => ("Thai", 352, 0),
        0x915..=0x939 => ("Deva", 315, 0),
        0x20 | 0x2e | 0x30..=0x39 | 0x3001 | 0x30fc | 0x1f600 | 0x300..=0x36f | 0x200d => return Ok(None),
        _ => return Err(Fail::Unmodeled(open::DW_SCRIPT_CHAR)),
    }))
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
