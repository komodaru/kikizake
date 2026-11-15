//! AnalyzeBidi: the Unicode Bidirectional Algorithm (UAX #9) as Windows runs it: explicit
//! embeddings and overrides (X1-X9, depth 61), isolates as other neutrals, level runs (X10),
//! rules W1-W7, N1-N2, I1-I2 and L1; classes from DerivedBidiClass.txt (Unicode 16.0).
use crate::spec::clause;
use crate::spec::hit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    L, R, AL, EN, AN, ES, ET, CS, NSM, WS, ON, S, B, BN, LRE, LRO, RLE, RLO, PDF, LRI, RLI, FSI, PDI,
}
use Class::*;

impl Class {
    /// a DerivedBidiClass.txt value: the short name on its lines, the long one on @missing lines
    fn parse(v: &str) -> Option<Class> {
        Some(match v {
            "L" | "Left_To_Right" => L,
            "R" | "Right_To_Left" => R,
            "AL" | "Arabic_Letter" => AL,
            "EN" | "European_Number" => EN,
            "AN" | "Arabic_Number" => AN,
            "ES" | "European_Separator" => ES,
            "ET" | "European_Terminator" => ET,
            "CS" | "Common_Separator" => CS,
            "NSM" | "Nonspacing_Mark" => NSM,
            "WS" | "White_Space" => WS,
            "ON" | "Other_Neutral" => ON,
            "S" | "Segment_Separator" => S,
            "B" | "Paragraph_Separator" => B,
            "BN" | "Boundary_Neutral" => BN,
            "LRE" | "Left_To_Right_Embedding" => LRE,
            "LRO" | "Left_To_Right_Override" => LRO,
            "RLE" | "Right_To_Left_Embedding" => RLE,
            "RLO" | "Right_To_Left_Override" => RLO,
            "PDF" | "Pop_Directional_Format" => PDF,
            "LRI" | "Left_To_Right_Isolate" => LRI,
            "RLI" | "Right_To_Left_Isolate" => RLI,
            "FSI" | "First_Strong_Isolate" => FSI,
            "PDI" | "Pop_Directional_Isolate" => PDI,
            _ => return None,
        })
    }
}

/// a character's class (DerivedBidiClass.txt). MEASURED (golden dwrite, bidiall): a character
/// it does not list is L, not its @missing default (R, AL or ET in their blocks)
pub fn class(c: u32) -> Class {
    hit(clause::DW_BIDI_UCD);
    let v = crate::ucd::bidi_class().listed(c).unwrap_or("L");
    Class::parse(v).unwrap_or_else(|| panic!("DerivedBidiClass.txt: no class {v}"))
}

/// X9: the characters the explicit rules remove (they take the level of the one before).
/// MEASURED (golden dwrite, bidipair): the isolates too, as boundary neutrals
fn removed(c: Class) -> bool {
    matches!(c, BN | LRE | LRO | RLE | RLO | PDF | LRI | RLI | FSI | PDI)
}

/// UAX #9 6.2's deepest embedding level
const MAX_DEPTH: u8 = 61;

/// each character's explicit (embedding) level and resolved level at paragraph level `para`
/// (0 or 1). MEASURED (golden dwrite): a paragraph separator ends the paragraph, the text after
/// it resolved by itself, embeddings and all; a segment separator ends the level run (the text
/// between separators resolved apart) but not the embeddings
pub fn analyze(text: &[Class], para: u8) -> (Vec<u8>, Vec<u8>) {
    let (mut ex, mut lv) = (Vec::new(), Vec::new());
    let mut s = 0;
    for (i, c) in text.iter().enumerate() {
        if *c == B {
            let (e, l) = paragraph(&text[s..i], para);
            ex.extend(e);
            lv.extend(l);
            ex.push(para);
            lv.push(para);
            s = i + 1;
        }
    }
    let (e, l) = paragraph(&text[s..], para);
    ex.extend(e);
    lv.extend(l);
    (ex, lv)
}

/// the levels of a text without paragraph separators
fn paragraph(text: &[Class], para: u8) -> (Vec<u8>, Vec<u8>) {
    // X1-X8: the embedding level of each character, and its class under an override
    let mut stack: Vec<(u8, Option<Class>)> = vec![(para, None)];
    let mut over_count = 0u32;
    let mut ex = Vec::with_capacity(text.len());
    let mut t = Vec::with_capacity(text.len());
    for &c in text {
        let (level, dir) = *stack.last().unwrap_or(&(para, None));
        match c {
            RLE | LRE | RLO | LRO => {
                hit(clause::DW_BIDI_X);
                let next = match c {
                    RLE | RLO => (level + 1) | 1,
                    _ => (level + 2) & !1,
                };
                let o = match c {
                    RLO => Some(R),
                    LRO => Some(L),
                    _ => None,
                };
                if next <= MAX_DEPTH && over_count == 0 {
                    stack.push((next, o));
                    ex.push(next);
                } else {
                    // (too deep: counted, so that its PDF is matched)
                    over_count += 1;
                    ex.push(level);
                }
                t.push(c);
            }
            PDF => {
                hit(clause::DW_BIDI_X);
                ex.push(level);
                if over_count > 0 {
                    over_count -= 1;
                } else if stack.len() > 1 {
                    stack.pop();
                }
                t.push(c);
            }
            // (L1: a segment separator at the paragraph level, its explicit level too)
            S => {
                hit(clause::DW_BIDI_L1);
                ex.push(para);
                t.push(S);
            }
            _ => {
                ex.push(level);
                t.push(match (dir, removed(c)) {
                    (Some(d), false) => d,
                    _ => c,
                });
            }
        }
    }
    // X9, X10: the level runs of the characters left, each resolved with its sos and eos
    let kept: Vec<usize> = (0..t.len()).filter(|&i| !removed(t[i])).collect();
    let mut lv = vec![para; t.len()];
    let mut k = 0;
    while k < kept.len() {
        let level = ex[kept[k]];
        if t[kept[k]] == S {
            // (a segment separator: the level runs on either side are resolved apart)
            hit(clause::DW_BIDI_SEGMENT);
            k += 1;
            continue;
        }
        let mut j = k;
        while j < kept.len() && ex[kept[j]] == level && t[kept[j]] != S {
            j += 1;
        }
        let before = if k == 0 { para } else { ex[kept[k - 1]] };
        let after = if j == kept.len() { para } else { ex[kept[j]] };
        let e = |l: u8| if l % 2 == 0 { L } else { R };
        let run: Vec<Class> = kept[k..j].iter().map(|&i| t[i]).collect();
        let r = resolve(&run, level, e(level.max(before)), e(level.max(after)));
        for (&i, l) in kept[k..j].iter().zip(r) {
            lv[i] = l;
        }
        k = j;
    }
    // MEASURED: a removed character takes the resolved level of the one before it (the
    // paragraph's at the start)
    let mut prev = para;
    for i in 0..t.len() {
        if removed(t[i]) {
            lv[i] = prev;
        }
        prev = lv[i];
    }
    (ex, lv)
}

/// W1-W7, N1-N2, I1-I2 over one level run
fn resolve(run: &[Class], level: u8, sos: Class, eos: Class) -> Vec<u8> {
    let mut t: Vec<Class> = run.to_vec();
    hit(clause::DW_BIDI_W);
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
    hit(clause::DW_BIDI_N);
    let ni = |c: Class| matches!(c, WS | ON | S | B);
    let dir = |c: Class| match c {
        L => Some(L),
        R | EN | AN => Some(R),
        _ => None,
    };
    let e = if level % 2 == 0 { L } else { R };
    let mut i = 0;
    while i < t.len() {
        if ni(t[i]) {
            let s = i;
            while i < t.len() && ni(t[i]) {
                i += 1;
            }
            let before = if s == 0 { sos } else { dir(t[s - 1]).unwrap_or(sos) };
            let after = if i == t.len() { eos } else { dir(t[i]).unwrap_or(eos) };
            let d = if before == after { before } else { e };
            t[s..i].iter_mut().for_each(|c| *c = d);
        } else {
            i += 1;
        }
    }
    // I1, I2
    hit(clause::DW_BIDI_I);
    t.iter()
        .map(|c| match (level % 2, c) {
            (0, R) => level + 1,
            (0, AN | EN) => level + 2,
            (1, L | EN | AN) => level + 1,
            _ => level,
        })
        .collect()
}
