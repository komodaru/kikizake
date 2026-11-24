//! Comparing two outputs case by case. A case whose wanted lines are "unmodeled ..." (a model's
//! output) is skipped. Differences are reported by case with the first differing line; a
//! differing picture row also names the first differing pixel.

use crate::case::{parse_output, Output};

#[derive(Debug, Default)]
pub struct Report {
    pub cases: usize,
    pub same: usize,
    pub unmodeled: usize,
    pub differ: Vec<String>,
}

fn first_pixel(want: &str, got: &str, bpp: u32) -> Option<usize> {
    let w = want.as_bytes();
    let g = got.as_bytes();
    let i = (0..w.len().min(g.len())).find(|&i| w[i] != g[i]).unwrap_or(w.len().min(g.len()));
    // (hex digits per pixel; below 8 bpp a byte holds several pixels)
    Some(if bpp >= 8 { i / (bpp as usize / 4) } else { i / 2 * (8 / bpp as usize) })
}

/// a wanted line against a got one: the same, or the same but where the wanted line has "?"
/// (a model's undefined byte, "??"), which any hex digit matches; "oneof a | b | ..." takes
/// exactly one of its alternatives (a "?" in one is no wildcard). On bytes, by plain loops: the
/// Kani proofs below check this code itself, and std's string searchers were more than CBMC
/// could hold in 15 GB for lines of 2 bytes.
pub fn line_matches(want: Option<&str>, got: Option<&str>) -> bool {
    match (want, got) {
        (Some(w), Some(g)) => match w.as_bytes().strip_prefix(b"oneof ") {
            Some(alternatives) => is_one_of(alternatives, g.as_bytes()),
            None => w.len() == g.len() && w.bytes().zip(g.bytes()).all(|(a, b)| if a == b'?' { b.is_ascii_hexdigit() } else { a == b }),
        },
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
}

/// `g` is one of the alternatives in `list`, separated by " | "
fn is_one_of(mut list: &[u8], g: &[u8]) -> bool {
    loop {
        match list.windows(3).position(|x| x == b" | ") {
            Some(at) => {
                if list.get(..at) == Some(g) {
                    return true;
                }
                list = list.get(at.saturating_add(3)..).unwrap_or_default();
            }
            None => return list == g,
        }
    }
}

pub fn compare(want: &Output, got: &Output) -> Report {
    let mut r = Report::default();
    if want.after_done || got.after_done { r.differ.push("output after done".into()); }
    if want.cases.is_empty() {
        r.differ.push("no expected cases".into());
    }
    if want.head != got.head {
        r.differ.push(format!("output header: want {:?}, got {:?}", want.head, got.head));
    }
    for (i, (name, wl)) in want.cases.iter().enumerate() {
        r.cases += 1;
        let Some((actual, gl)) = got.cases.get(i) else {
            r.differ.push(format!("{name}: missing (the program stopped or skipped it)"));
            continue;
        };
        if actual != name {
            r.differ.push(format!("case {}: want {name}, got {actual}", i + 1));
            continue;
        }
        if wl.first().is_some_and(|l| l == "invalid case" || l == "unmodeled bad line") {
            r.differ.push(format!("{name}: invalid model input"));
            continue;
        }
        if wl.first().is_some_and(|l| l.starts_with("unmodeled ")) {
            r.unmodeled += 1;
            continue;
        }
        let mut bpp = 0;
        let mut diff = None;
        for i in 0..wl.len().max(gl.len()) {
            let (a, b) = (wl.get(i).map(String::as_str), gl.get(i).map(String::as_str));
            if let Some(px) = a.and_then(|a| a.strip_prefix("px ")) {
                bpp = px.split(' ').nth(3).and_then(|v| v.parse().ok()).unwrap_or(0);
            }
            if !line_matches(a, b) {
                let mut d = format!("{name}: line {}: want {:?} got {:?}", i + 1, a.unwrap_or("(none)"), b.unwrap_or("(none)"));
                if let (Some(a), Some(b), true) = (a, b, bpp > 0) && a.len() == b.len() && a.bytes().all(|c| c.is_ascii_hexdigit()) {
                    let row = (0..i).rev().take_while(|&j| !wl[j].starts_with("px ")).count();
                    if let Some(x) = first_pixel(a, b, bpp) {
                        d += &format!(" (row {row}, first pixel {x})");
                    }
                }
                diff = Some(d);
                break;
            }
        }
        match diff {
            Some(d) => r.differ.push(d),
            None => r.same += 1,
        }
    }
    for (name, _) in got.cases.iter().skip(want.cases.len()) {
        r.differ.push(format!("{name}: unexpected case"));
    }
    if want.done != got.done {
        r.differ.push(format!("program end: want {} got {}", if want.done { "done" } else { "a crash" }, if got.done { "done" } else { "a crash" }));
    }
    r
}

pub fn compare_text(want: &str, got: &str) -> Report {
    compare(&parse_output(want), &parse_output(got))
}

#[cfg(kani)]
mod proofs {
    use super::line_matches;

    /// A fixed-size ASCII candidate keeps CBMC's input on the stack (#16).
    fn ascii() -> u8 {
        let byte = kani::any();
        kani::assume(byte <= 0x7f);
        byte
    }

    #[kani::proof]
    #[kani::unwind(12)]
    fn comparator_question_mark() {
        let candidate = [ascii(), ascii()];
        let got = std::str::from_utf8(&candidate).unwrap();
        let hex = matches!(candidate[1], b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F');
        assert_eq!(line_matches(Some("x?"), Some(got)), candidate[0] == b'x' && hex);
        assert!(!line_matches(Some("?"), Some(got)));
        assert!(!line_matches(None, Some(got)));
        assert!(!line_matches(Some(got), None));
        assert!(line_matches(None, None));
    }

    #[kani::proof]
    #[kani::unwind(16)]
    fn comparator_exact_alternatives() {
        let candidate = [ascii(), ascii()];
        let got = std::str::from_utf8(&candidate).unwrap();
        assert_eq!(line_matches(Some("oneof a? | ?b"), Some(got)),
                   candidate == *b"a?" || candidate == *b"?b");
        assert!(!line_matches(Some("oneof a | b"), Some(got)));
    }
}

#[test]
fn wildcard_requires_a_hex_digit_even_when_the_actual_byte_is_question_mark() {
    assert!(!line_matches(Some("??"), Some("??")));
    assert!(line_matches(Some("??"), Some("fA")));
    assert!(line_matches(Some("oneof ?? | AB"), Some("??")));
}
