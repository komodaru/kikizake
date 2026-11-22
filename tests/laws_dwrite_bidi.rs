//! AnalyzeBidi's separators (spec/dwrite.txt DW.BIDI.SEGMENT) over every text of up to 2
//! classes on each side of the separator, both paragraph directions (kind exhaustive).
use kikizake::model::dwrite::bidi::{self, Class, Class::*};

const ALL: [Class; 23] = [L, R, AL, EN, AN, ES, ET, CS, NSM, WS, ON, S, B, BN, LRE, LRO, RLE, RLO, PDF, LRI, RLI, FSI, PDI];
/// the classes of no explicit formatting and no separator
const PLAIN: [Class; 11] = [L, R, AL, EN, AN, ES, ET, CS, NSM, WS, ON];

/// every text of 0..=2 of the classes
fn texts(of: &[Class]) -> Vec<Vec<Class>> {
    let mut v = vec![Vec::new()];
    v.extend(of.iter().map(|&a| vec![a]));
    v.extend(of.iter().flat_map(|&a| of.iter().map(move |&b| vec![a, b])));
    v
}

fn cat(a: &[Class], b: &[Class], c: &[Class]) -> Vec<Class> {
    [a, b, c].concat()
}

/// DW.BIDI.SEGMENT: a paragraph separator ends the paragraph: the text after it has the
/// explicit and resolved levels it has alone (embeddings closed), the text up to it those it
/// has alone; every two texts of up to 2 of all 23 classes
#[test]
fn a_paragraph_separator_ends_the_paragraph() {
    let ts = texts(&ALL);
    for a in &ts {
        for b in &ts {
            for para in [0u8, 1] {
                let whole = bidi::analyze(&cat(a, &[B], b), para);
                let (x, y) = (bidi::analyze(&cat(a, &[B], &[]), para), bidi::analyze(b, para));
                assert_eq!(whole.0, [x.0, y.0].concat(), "{a:?} B {b:?} {para}");
                assert_eq!(whole.1, [x.1, y.1].concat(), "{a:?} B {b:?} {para}");
            }
        }
    }
}

/// DW.BIDI.SEGMENT: a segment separator ends the level run: between texts of no explicit
/// formatting (every two of up to 2 of the 11 such classes) the levels are those of the texts
/// on each side alone; after an embedding or override opened before (LRE, RLE, LRO, RLO) the
/// text after it has the levels it has right after the opener (sos the higher of its level and
/// the paragraph's), the embedding going on past it
#[test]
fn a_segment_separator_ends_the_level_run() {
    let ts = texts(&PLAIN);
    for a in &ts {
        for b in &ts {
            for para in [0u8, 1] {
                let whole = bidi::analyze(&cat(a, &[S], b), para);
                let (x, y) = (bidi::analyze(&cat(a, &[S], &[]), para), bidi::analyze(b, para));
                assert_eq!(whole.1, [x.1, y.1].concat(), "{a:?} S {b:?} {para}");
                for e in [LRE, RLE, LRO, RLO] {
                    let whole = bidi::analyze(&[&[e][..], a, &[S], b].concat(), para);
                    let alone = bidi::analyze(&cat(&[e], &[], b), para);
                    let n = whole.1.len() - b.len();
                    assert_eq!(whole.1[n..], alone.1[1..], "{e:?} {a:?} S {b:?} {para}");
                    assert_eq!(whole.0[n..], alone.0[1..], "{e:?} {a:?} S {b:?} {para}");
                }
            }
        }
    }
}
