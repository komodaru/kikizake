//! AnalyzeLineBreakpoints: the Unicode Line Breaking Algorithm (UAX #14) as Windows runs it,
//! classes from LineBreak.txt (Unicode 16.0) resolved as Windows does, each boundary's condition
//! given to the UTF-16 units on either side of it.
use crate::spec::clause;
use crate::spec::hit;

/// a LineBreak.txt class
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lb {
    BK, CR, LF, CM, NL, SG, WJ, ZW, GL, SP, ZWJ, B2, BA, BB, HY, CB, CL, CP, EX, IN, NS, OP, QU, IS,
    NU, PO, PR, SY, AI, AK, AL, AP, AS, CJ, EB, EM, H2, H3, HL, ID, JL, JT, JV, RI, SA, VF, VI, XX,
}
use Lb::*;

impl Lb {
    fn parse(v: &str) -> Option<Lb> {
        Some(match v {
            "BK" => BK, "CR" => CR, "LF" => LF, "CM" => CM, "NL" => NL, "SG" => SG, "WJ" => WJ,
            "ZW" => ZW, "GL" => GL, "SP" => SP, "ZWJ" => ZWJ, "B2" => B2, "BA" => BA, "BB" => BB,
            "HY" => HY, "CB" => CB, "CL" => CL, "CP" => CP, "EX" => EX, "IN" => IN, "NS" => NS,
            "OP" => OP, "QU" => QU, "IS" => IS, "NU" => NU, "PO" => PO, "PR" => PR, "SY" => SY,
            "AI" => AI, "AK" => AK, "AL" => AL, "AP" => AP, "AS" => AS, "CJ" => CJ, "EB" => EB,
            "EM" => EM, "H2" => H2, "H3" => H3, "HL" => HL, "ID" => ID, "JL" => JL, "JT" => JT,
            "JV" => JV, "RI" => RI, "SA" => SA, "VF" => VF, "VI" => VI, "XX" => XX,
            _ => return None,
        })
    }

    /// MEASURED (golden dwrite, breakall): the class Windows breaks a character of this class
    /// by: ambiguous, the Brahmic aksara classes, complex context, surrogates and unknown as
    /// alphabetic (LB1, and no LB28a), conditional Japanese starters as nonstarters, Hangul
    /// syllables and jamo and emoji bases as ideographs (no LB26, LB27), emoji modifiers as
    /// combining marks, Hebrew letters as alphabetic
    fn resolved(self) -> Lb {
        match self {
            AI | AK | AP | AS | VF | VI | SA | SG | XX | HL => AL,
            CJ => NS,
            H2 | H3 | JL | JT | JV | EB => ID,
            EM => CM,
            BK | CR | LF | CM | NL | WJ | ZW | GL | SP | ZWJ | B2 | BA | BB | HY | CB | CL | CP | EX | IN | NS | OP | QU
            | IS | NU | PO | PR | SY | AL | ID | RI => self,
        }
    }
}

/// a character's class (LineBreak.txt, its @missing default XX where unlisted)
pub fn class(c: u32) -> Lb {
    let v = crate::ucd::line_break().get(c).unwrap_or("XX");
    Lb::parse(v).unwrap_or_else(|| panic!("LineBreak.txt: no class {v}"))
}

/// DWRITE_BREAK_CONDITION
pub const NEUTRAL: u8 = 0;
pub const CAN: u8 = 1;
pub const MAY_NOT: u8 = 2;
pub const MUST: u8 = 3;

/// a hard line break (LB4, LB5)
fn hard(c: Lb) -> bool {
    matches!(c, BK | CR | LF | NL)
}

/// the condition at the boundary before character i (0 < i < n) of the resolved classes `t`
fn boundary(t: &[Lb], attached: &[bool], i: usize) -> u8 {
    let (a, b) = (t[i - 1], t[i]);
    // LB5: CR × LF
    if a == CR && b == LF {
        return MAY_NOT;
    }
    // LB4, LB5
    if hard(a) {
        hit(clause::DW_BREAK_MUST);
        return MUST;
    }
    if attached[i] {
        return MAY_NOT;
    }
    // LB6, LB7
    if hard(b) || matches!(b, SP | ZW) {
        return MAY_NOT;
    }
    // LB8: ZW SP* ÷
    let mut j = i;
    while j > 0 && t[j - 1] == SP {
        j -= 1;
    }
    let before_sp = if j > 0 { Some(t[j - 1]) } else { None };
    if before_sp == Some(ZW) {
        return CAN;
    }
    // LB8a (LB9: a mark on a base is attached above; one after a space is alphabetic, LB10)
    if a == ZWJ {
        return MAY_NOT;
    }
    // LB11
    if a == WJ || b == WJ {
        return MAY_NOT;
    }
    // LB12, LB12a
    if a == GL || (b == GL && !matches!(a, SP | BA | HY)) {
        return MAY_NOT;
    }
    // LB13
    if matches!(b, CL | CP | EX | IS | SY) {
        return MAY_NOT;
    }
    // LB14: OP SP* ×
    if before_sp == Some(OP) {
        return MAY_NOT;
    }
    // LB15: QU SP* × OP
    if before_sp == Some(QU) && b == OP {
        return MAY_NOT;
    }
    // LB16: (CL | CP) SP* × NS
    if matches!(before_sp, Some(CL | CP)) && b == NS {
        return MAY_NOT;
    }
    // LB17: B2 SP* × B2
    if before_sp == Some(B2) && b == B2 {
        return MAY_NOT;
    }
    // LB18
    if a == SP {
        return CAN;
    }
    // LB19
    if a == QU || b == QU {
        return MAY_NOT;
    }
    // LB20
    if a == CB || b == CB {
        return CAN;
    }
    // LB21
    if matches!(b, BA | HY | NS) || a == BB {
        return MAY_NOT;
    }
    // LB22
    if b == IN {
        return MAY_NOT;
    }
    // LB23, LB23a, LB24, LB25
    let pair = |x: &[Lb], y: &[Lb]| x.contains(&a) && y.contains(&b);
    if pair(&[AL], &[NU]) || pair(&[NU], &[AL]) || pair(&[PR], &[ID]) || pair(&[ID], &[PO]) || pair(&[PR, PO], &[AL]) {
        return MAY_NOT;
    }
    if pair(&[CL, CP, NU], &[PO, PR]) || pair(&[PO, PR], &[OP, NU]) || pair(&[HY, IS, NU, SY], &[NU]) {
        return MAY_NOT;
    }
    // LB28, LB29, LB30
    if pair(&[AL], &[AL]) || pair(&[IS], &[AL]) || pair(&[AL, NU], &[OP]) || pair(&[CP], &[AL, NU]) {
        return MAY_NOT;
    }
    // LB30a: regional indicators in pairs
    if a == RI && b == RI {
        let n = t[..i].iter().rev().take_while(|&&c| c == RI).count();
        if n % 2 == 1 {
            return MAY_NOT;
        }
    }
    // LB31
    CAN
}

/// White_Space (PropList.txt)
fn white(c: u32) -> bool {
    matches!(c, 0x9..=0xd | 0x20 | 0x85 | 0xa0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000)
}

/// each UTF-16 unit's (before, after, whitespace, soft hyphen)
pub fn breaks(cps: &[u32]) -> Vec<(u8, u8, u8, u8)> {
    hit(clause::DW_BREAK_UAX14);
    let mut t: Vec<Lb> = cps.iter().map(|&c| class(c).resolved()).collect();
    // LB9, LB10: a combining mark takes its base's class, or is AL with no base
    let mut attached = vec![false; t.len()];
    for i in 0..t.len() {
        if matches!(t[i], CM | ZWJ) {
            let base = if i > 0 && !matches!(t[i - 1], BK | CR | LF | NL | SP | ZW) { Some(t[i - 1]) } else { None };
            attached[i] = base.is_some();
            if t[i] == CM || base.is_some() {
                t[i] = base.unwrap_or(AL);
            }
        }
    }
    let n = t.len();
    let mut v: Vec<(u8, u8, u8, u8)> = cps.iter().map(|&c| (NEUTRAL, NEUTRAL, white(c) as u8, (c == 0xad) as u8)).collect();
    v[0].0 = MAY_NOT;
    v[n - 1].1 = if hard(t[n - 1]) { MUST } else { CAN };
    for i in 1..n {
        let b = boundary(&t, &attached, i);
        v[i - 1].1 = b;
        v[i].0 = b;
    }
    // (a supplementary character's two units: no break between them)
    let mut u = Vec::with_capacity(n * 2);
    for (&c, x) in cps.iter().zip(v) {
        if c >= 0x10000 {
            u.push((x.0, MAY_NOT, x.2, x.3));
            u.push((MAY_NOT, x.1, x.2, x.3));
        } else {
            u.push(x);
        }
    }
    u
}
