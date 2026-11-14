//! AnalyzeLineBreakpoints: the Unicode Line Breaking Algorithm (UAX #14) for the characters the
//! suite uses (their classes from LineBreak.txt), each boundary's condition given to the units
//! on either side of it.
use super::super::{Fail, Res};
use crate::spec::hit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lb {
    AL, SP, HY, NU, OP, CP, QU, ID, CL, GL, ZW, BA, LF, BK, CM, ZWJ, EX, SY, PR, PO, NS, IS,
}
use Lb::*;

/// DWRITE_BREAK_CONDITION
pub const NEUTRAL: u8 = 0;
pub const CAN: u8 = 1;
pub const MAY_NOT: u8 = 2;
pub const MUST: u8 = 3;

pub fn class(c: u32) -> Res<Lb> {
    Ok(match c {
        0x41..=0x5a | 0x61..=0x7a => AL,
        0x20 => SP,
        0x2d => HY,
        0x30..=0x39 => NU,
        0x28 | 0x5b | 0x7b => OP,
        0x29 | 0x5d => CP,
        0x7d | 0x3001 | 0x3002 => CL,
        0x22 | 0x27 => QU,
        0x4e00..=0x9fff | 0x3042 => ID,
        0x30a1 => NS, // (CJ, resolved as NS: LB1)
        0xa0 => GL,
        0x200b => ZW,
        0x9 | 0xad => BA,
        0xa => LF,
        0xc | 0x2028 => BK,
        0x300..=0x36f => CM,
        0x200d => ZWJ,
        0x21 | 0x3f => EX,
        0x2f => SY,
        0x24 => PR,
        0x25 => PO,
        0x2c | 0x2e | 0x3a | 0x3b => IS,
        _ => return Err(Fail::Unmodeled("DW.BREAK.CHAR")),
    })
}

/// the condition at the boundary before unit i (0 < i < n) of the classes `t`
fn boundary(t: &[Lb], attached: &[bool], i: usize) -> u8 {
    let (a, b) = (t[i - 1], t[i]);
    if attached[i] {
        return MAY_NOT;
    }
    // LB4, LB5
    if matches!(a, BK | LF) {
        hit("DW.BREAK.MUST");
        return MUST;
    }
    // LB6, LB7
    if matches!(b, BK | LF | SP | ZW) {
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
    // LB8a, LB9
    if a == ZWJ || (matches!(b, CM | ZWJ) && !matches!(a, BK | LF | SP | ZW)) {
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
    // LB18
    if a == SP {
        return CAN;
    }
    // LB19
    if a == QU || b == QU {
        return MAY_NOT;
    }
    // LB21
    if matches!(b, BA | HY | NS) {
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
    // LB31
    CAN
}

/// each unit's (before, after, whitespace, soft hyphen)
pub fn breaks(cps: &[u32]) -> Res<Vec<(u8, u8, u8, u8)>> {
    hit("DW.BREAK.UAX14");
    let mut t = Vec::new();
    for &c in cps {
        if c >= 0x10000 {
            return Err(Fail::Bad);
        }
        t.push(class(c)?);
    }
    // LB9, LB10: a combining mark takes its base's class, or is AL with no base
    let mut attached = vec![false; t.len()];
    for i in 0..t.len() {
        if matches!(t[i], CM | ZWJ) {
            let base = if i > 0 && !matches!(t[i - 1], BK | LF | SP | ZW) { Some(t[i - 1]) } else { None };
            attached[i] = base.is_some();
            if t[i] == CM || base.is_some() {
                t[i] = base.unwrap_or(AL);
            }
        }
    }
    let n = t.len();
    let mut v: Vec<(u8, u8, u8, u8)> = cps.iter().map(|&c| (NEUTRAL, NEUTRAL, matches!(c, 0x9..=0xd | 0x20 | 0x85 | 0xa0 | 0x2028 | 0x2029) as u8, (c == 0xad) as u8)).collect();
    v[0].0 = MAY_NOT;
    v[n - 1].1 = if matches!(t[n - 1], BK | LF) { MUST } else { CAN };
    for i in 1..n {
        let b = boundary(&t, &attached, i);
        v[i - 1].1 = b;
        v[i].0 = b;
    }
    Ok(v)
}
