//! AnalyzeBidi: the Unicode Bidirectional Algorithm (UAX #9) for text without explicit
//! formatting characters or brackets: one isolating run sequence at the paragraph level, rules
//! W1-W7, N1-N2, I1-I2 and L1.
use crate::spec::hit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    L, R, AL, EN, AN, ES, ET, CS, NSM, WS, ON, S, B,
}
use Class::*;

/// the classes of the characters the suite uses (UnicodeData.txt); None: not modelled
pub fn class(c: u32) -> Option<Class> {
    Some(match c {
        0x9 | 0xb | 0x1f => S,
        0xa | 0xd | 0x1c..=0x1e | 0x85 | 0x2029 => B,
        0xc | 0x20 => WS,
        0x30..=0x39 => EN,
        0x2b | 0x2d => ES,
        0x23..=0x25 | 0xa2..=0xa5 => ET,
        0x2c | 0x2e | 0x2f | 0x3a => CS,
        0x21 | 0x22 | 0x26 | 0x27 | 0x2a | 0x3b..=0x3f | 0x40 | 0x5c | 0x5e | 0x5f | 0x60 | 0x7c | 0x7e => ON,
        0x41..=0x5a | 0x61..=0x7a => L,
        0x300..=0x36f => NSM,
        0x5d0..=0x5ea => R,
        0x627..=0x64a => AL,
        0x660..=0x669 => AN,
        _ => return None,
    })
}

/// the resolved level of each character at paragraph level `para` (0 or 1). MEASURED (golden
/// dwrite): a segment separator ends the run as a paragraph separator would; the text between
/// separators is resolved by itself, from and to the paragraph's direction.
pub fn levels(text: &[Class], para: u8) -> Vec<u8> {
    let mut lv = Vec::new();
    let mut s = 0;
    for (i, c) in text.iter().enumerate() {
        if matches!(c, S | B) {
            if i > s {
                hit("DW.BIDI.SEGMENT");
            }
            lv.extend(segment(&text[s..i], para));
            lv.push(para);
            s = i + 1;
        }
    }
    lv.extend(segment(&text[s..], para));
    // L1: separators, and whitespace before them or at the end, to the paragraph level
    let mut tail = true;
    for i in (0..text.len()).rev() {
        match text[i] {
            S | B => {
                hit("DW.BIDI.L1");
                tail = true;
            }
            WS if tail => {
                hit("DW.BIDI.L1");
                lv[i] = para;
            }
            _ => tail = false,
        }
    }
    lv
}

fn segment(text: &[Class], para: u8) -> Vec<u8> {
    let e = |l: u8| if l % 2 == 0 { L } else { R };
    let sos = e(para);
    let mut t: Vec<Class> = text.to_vec();
    hit("DW.BIDI.W");
    // W1
    let mut prev = sos;
    for c in t.iter_mut() {
        if *c == NSM {
            *c = prev;
        }
        prev = *c;
    }
    // W2, W3
    let mut strong = sos;
    for c in t.iter_mut() {
        match *c {
            L | R | AL => strong = *c,
            EN if strong == AL => *c = AN,
            _ => {}
        }
    }
    for c in t.iter_mut() {
        if *c == AL {
            *c = R;
        }
    }
    // W4
    for i in 1..t.len().saturating_sub(1) {
        if t[i - 1] == EN && t[i + 1] == EN && matches!(t[i], ES | CS) {
            t[i] = EN;
        } else if t[i - 1] == AN && t[i + 1] == AN && t[i] == CS {
            t[i] = AN;
        }
    }
    // W5
    let mut i = 0;
    while i < t.len() {
        if t[i] == ET {
            let s = i;
            while i < t.len() && t[i] == ET {
                i += 1;
            }
            if (s > 0 && t[s - 1] == EN) || (i < t.len() && t[i] == EN) {
                t[s..i].iter_mut().for_each(|c| *c = EN);
            }
        } else {
            i += 1;
        }
    }
    // W6
    for c in t.iter_mut() {
        if matches!(*c, ES | ET | CS) {
            *c = ON;
        }
    }
    // W7
    let mut strong = sos;
    for c in t.iter_mut() {
        match *c {
            L | R => strong = *c,
            EN if strong == L => *c = L,
            _ => {}
        }
    }
    // N1, N2
    hit("DW.BIDI.N");
    let ni = |c: Class| matches!(c, WS | ON | S | B);
    let dir = |c: Class| match c {
        L => Some(L),
        R | EN | AN => Some(R),
        _ => None,
    };
    let mut i = 0;
    while i < t.len() {
        if ni(t[i]) {
            let s = i;
            while i < t.len() && ni(t[i]) {
                i += 1;
            }
            let before = if s == 0 { sos } else { dir(t[s - 1]).unwrap_or(sos) };
            let after = if i == t.len() { sos } else { dir(t[i]).unwrap_or(sos) };
            let d = if before == after { before } else { e(para) };
            t[s..i].iter_mut().for_each(|c| *c = d);
        } else {
            i += 1;
        }
    }
    // I1, I2
    hit("DW.BIDI.I");
    let lv: Vec<u8> = t
        .iter()
        .map(|c| match (para % 2, c) {
            (0, R) => para + 1,
            (0, AN | EN) => para + 2,
            (1, L | EN | AN) => para + 1,
            _ => para,
        })
        .collect();
    lv
}
