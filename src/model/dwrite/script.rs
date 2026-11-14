//! AnalyzeScript: script runs (Unicode 16.0 Scripts.txt, with Windows' own changes; ISO 15924
//! codes as GetScriptProperties gives them), common and inherited characters taking the script
//! of the run they are in.
use crate::spec::{clause, open};
use super::super::{Fail, Res};
use crate::spec::hit;

/// a script as AnalyzeScript names it: ISO 15924 code and number, and its shapes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Script {
    pub code: &'static str,
    pub num: u32,
    pub shapes: Shapes,
}

/// DWRITE_SCRIPT_SHAPES
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shapes {
    Default,
    NoVisual,
}

impl Shapes {
    pub fn value(self) -> u32 {
        match self {
            Shapes::Default => 0,
            Shapes::NoVisual => 1,
        }
    }
}

/// what a character is to AnalyzeScript
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// a script of its own
    Script(Script),
    /// Common or Inherited: the script of the run it is in
    Common,
    /// a control, format or separator character of no script: no glyph
    NoVisual,
    /// not assigned (Unknown)
    Unknown,
}

/// the run of a text of common characters only, and of an unassigned character alone
const ZZZZ: Script = Script { code: "Zzzz", num: 999, shapes: Shapes::Default };
/// the run of a no-visual character alone
const ZYYY: Script = Script { code: "Zyyy", num: 998, shapes: Shapes::NoVisual };

/// MEASURED (golden dwrite, scriptall): the scripts whose code and number Windows gives as
/// another's (its table is shifted for the scripts of Unicode 11 to 13, and Mro has 199)
const SHIFTED: [(&str, &str, u32); 14] = [
    ("Chorasmian", "Dogr", 328),
    ("Dives_Akuru", "Gong", 312),
    ("Dogra", "Rohg", 167),
    ("Elymaic", "Maka", 366),
    ("Gunjala_Gondi", "Medf", 265),
    ("Hanifi_Rohingya", "Sogo", 142),
    ("Khitan_Small_Script", "Sogd", 141),
    ("Makasar", "Elym", 128),
    ("Medefaidrin", "Hmnp", 451),
    ("Mro", "Mroo", 199),
    ("Nyiakeng_Puachue_Hmong", "Wcho", 283),
    ("Old_Sogdian", "Chrs", 109),
    ("Sogdian", "Diak", 342),
    ("Wancho", "Kits", 288),
];

/// MEASURED (golden dwrite, scriptall): common characters Windows puts in the Arabic script (the
/// Arabic number marks and ends of ayah, the Arabic semicolon, question mark and tatweel, the
/// Indic and Ottoman Siyaq numbers)
fn arabic(c: u32) -> bool {
    matches!(c, 0x605 | 0x61b | 0x61f | 0x640 | 0x6dd | 0x8e2 | 0x1ec71..=0x1ecb4 | 0x1ed01..=0x1ed3d)
}

pub fn kind(c: u32) -> Res<Kind> {
    hit(clause::DW_SCRIPT_UCD);
    let long = crate::ucd::scripts().get(c).unwrap_or("Unknown");
    let gc = crate::ucd::general_category().get(c).unwrap_or("Cn");
    Ok(match long {
        "Common" if arabic(c) => {
            hit(clause::DW_SCRIPT_WINDOWS);
            Kind::Script(iso("Arabic")?)
        }
        // MEASURED: controls, format characters and the line and paragraph separators of no
        // script, and U+FFFE, U+FFFF
        "Common" if matches!(gc, "Cc" | "Cf" | "Zl" | "Zp") => Kind::NoVisual,
        "Unknown" if matches!(c, 0xfffe | 0xffff) => Kind::NoVisual,
        "Common" | "Inherited" => Kind::Common,
        "Unknown" => Kind::Unknown,
        _ => Kind::Script(iso(long)?),
    })
}

/// a script's code and number by its Unicode long name
fn iso(long: &str) -> Res<Script> {
    if let Some(&(_, code, num)) = SHIFTED.iter().find(|s| s.0 == long) {
        hit(clause::DW_SCRIPT_WINDOWS);
        return Ok(Script { code, num, shapes: Shapes::Default });
    }
    // (every script of Scripts.txt is in iso15924.txt)
    let (code, num) = crate::ucd::iso15924(long).ok_or(Fail::Unmodeled(open::DW_SCRIPT_CHAR))?;
    Ok(Script { code, num, shapes: Shapes::Default })
}

/// the runs (UTF-16 position, length, script) of a text of code points
pub fn runs(cps: &[u32]) -> Res<Vec<(usize, usize, Script)>> {
    if let [c] = cps {
        let n = if *c >= 0x10000 { 2 } else { 1 };
        match kind(*c)? {
            Kind::NoVisual => return Ok(vec![(0, n, ZYYY)]),
            Kind::Unknown => return Ok(vec![(0, n, ZZZZ)]),
            Kind::Script(_) | Kind::Common => {}
        }
    }
    let mut v: Vec<(usize, usize, Script)> = Vec::new();
    let mut pending = 0; // (leading common characters' UTF-16 units)
    let mut pos = 0;
    for &c in cps {
        let n = if c >= 0x10000 { 2 } else { 1 };
        match kind(c)? {
            // (in a text of more than one character: not measured)
            Kind::NoVisual | Kind::Unknown => return Err(Fail::Unmodeled(open::DW_SCRIPT_CHAR)),
            Kind::Common => match v.last_mut() {
                Some(l) => {
                    hit(clause::DW_SCRIPT_COMMON);
                    l.1 += n
                }
                None => pending += n,
            },
            Kind::Script(s) => match v.last_mut() {
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
        v.push((0, pos, ZZZZ));
    }
    Ok(v)
}
