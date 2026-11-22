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
            None => w.len() == g.len() && w.bytes().zip(g.bytes()).all(|(a, b)| a == b || (a == b'?' && b.is_ascii_hexdigit())),
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

/// Kani: the judge accepts what the expected output allows and nothing else (formal.yml's kani
/// job). Bounded: lines of up to 4 bytes over an alphabet with hex and non-hex digits, `?`,
/// spaces and `|`; outputs of up to 2 cases of up to 2 lines from a table of the line kinds.
#[cfg(kani)]
mod proofs {
    use super::*;

    const ALPHABET: [u8; 6] = *b"0fg?x ";

    /// (bytes of the alphabet, all ASCII: the String is made from them whole, not char by char)
    fn any_text(n: usize) -> String {
        let mut b = Vec::with_capacity(n);
        for _ in 0..n {
            let i: usize = kani::any();
            kani::assume(i < ALPHABET.len());
            b.push(ALPHABET[i]);
        }
        String::from_utf8(b).unwrap_or_default()
    }

    fn hex(c: u8) -> bool {
        c.is_ascii_digit() || (b'a'..=b'f').contains(&c) || (b'A'..=b'F').contains(&c)
    }

    /// Without `oneof`: a wanted line takes exactly the got line of its length whose every byte
    /// is its own or, where it has `?`, a hex digit; one of another length only if equal (never)
    #[kani::proof]
    #[kani::unwind(6)]
    fn a_question_mark_takes_one_hex_digit_and_nothing_else() {
        let (n, m): (usize, usize) = (kani::any(), kani::any());
        kani::assume(n <= 4 && m <= 4);
        let (w, g) = (any_text(n), any_text(m));
        let want = n == m && w.bytes().zip(g.bytes()).all(|(a, b)| a == b || (a == b'?' && hex(b)));
        assert_eq!(line_matches(Some(&w), Some(&g)), want);
        // (a line against no line: only no line against no line)
        assert!(!line_matches(Some(&w), None) && !line_matches(None, Some(&g)) && line_matches(None, None));
        kani::cover!(want && w.bytes().any(|c| c == b'?') && w != g, "a ? taking a hex digit");
        kani::cover!(!want && n == m && w.bytes().zip(g.bytes()).any(|(a, b)| a == b'?' && b == b'g'), "a ? refusing a non-hex byte");
    }

    /// `oneof a | b`: the got line is one of the alternatives listed, exactly (a `?` in one is
    /// no wildcard), nothing else
    #[kani::proof]
    #[kani::unwind(6)]
    fn oneof_takes_only_its_alternatives() {
        let (n1, n2, m): (usize, usize, usize) = (kani::any(), kani::any(), kani::any());
        kani::assume(n1 <= 3 && n2 <= 3 && m <= 3);
        let (a, b, g) = (any_text(n1), any_text(n2), any_text(m));
        // (the alternatives as the generators write them: no " | " inside one)
        kani::assume(!a.bytes().any(|c| c == b' ') && !b.bytes().any(|c| c == b' '));
        let mut w = String::from("oneof ");
        w.push_str(&a);
        w.push_str(" | ");
        w.push_str(&b);
        assert_eq!(line_matches(Some(&w), Some(&g)), g == a || g == b);
        kani::cover!(g == b && g != a, "the second alternative");
        kani::cover!(g != a && g != b && a.bytes().any(|c| c == b'?') && g.len() == a.len(), "a ? in an alternative is no wildcard");
    }

    /// the line kinds of an expected or a got output
    const LINES: [&str; 6] = ["0a", "0?", "oneof 0a | 1b", "unmodeled stops at X", "invalid case", "1b"];

    /// (the kinds a case's lines are checked with one against another: lines that match, take a
    /// `?`, or differ; the special lines are the other harness's)
    const PLAIN: [&str; 3] = ["0a", "0?", "1b"];

    fn any_case(max_lines: usize, kinds: &[&str]) -> (String, Vec<String>) {
        let name = if kani::any() { "c1" } else { "c2" };
        let n: usize = kani::any();
        kani::assume(n <= max_lines);
        let mut lines = Vec::new();
        for _ in 0..n {
            let i: usize = kani::any();
            kani::assume(i < kinds.len());
            lines.push(kinds[i].to_string());
        }
        (name.to_string(), lines)
    }

    fn any_output(max_cases: usize, max_lines: usize, kinds: &[&str]) -> Output {
        let n: usize = kani::any();
        kani::assume(n <= max_cases);
        let mut cases = Vec::new();
        for _ in 0..n {
            cases.push(any_case(max_lines, kinds));
        }
        Output { head: if kani::any() { vec!["h".into()] } else { Vec::new() }, cases, done: kani::any(), after_done: kani::any() }
    }

    /// a case's lines agree: as many, each taken by its wanted line
    fn lines_agree(w: &[String], g: &[String]) -> bool {
        w.len() == g.len() && w.iter().zip(g).all(|(a, b)| line_matches(Some(a), Some(b)))
    }

    /// The report has no difference exactly when the outputs agree: the same header, each wanted
    /// case answered in its place under its name with lines its wanted lines take (an
    /// `unmodeled` one counted as such, never as the same), no case more, the same end (done or
    /// a crash), nothing after done, and some case wanted. Counts: every wanted case is one
    /// case, the same and the unmodeled never more than there are.
    ///
    /// Two harnesses of the one check (one at 2 cases of 2 lines ran CBMC out of 15 GB): cases
    /// of one line each, up to 2 of them (the order, names, counts, head, end); one case each,
    /// of up to 2 lines of the PLAIN kinds (a case's lines, as many of them; with every kind it ran
    /// out of 15 GB too, and the special lines only ever count as a case's first line).
    fn report_is_clean_exactly_when_the_outputs_agree(want: &Output, got: &Output) -> (bool, Report) {
        let r = compare(want, got);
        assert_eq!(r.cases, want.cases.len());
        assert!(r.same.saturating_add(r.unmodeled) <= r.cases);
        let bad_model = |l: &[String]| l.first().is_some_and(|l| l == "invalid case" || l == "unmodeled bad line");
        let unmodeled = |l: &[String]| !bad_model(l) && l.first().is_some_and(|l| l.starts_with("unmodeled "));
        let agree = !want.cases.is_empty()
            && !want.after_done
            && !got.after_done
            && want.head == got.head
            && want.done == got.done
            && want.cases.len() == got.cases.len()
            && want.cases.iter().zip(&got.cases).all(|((wn, wl), (gn, gl))| wn == gn && !bad_model(wl) && (unmodeled(wl) || lines_agree(wl, gl)));
        assert_eq!(r.differ.is_empty(), agree);
        if agree {
            let um = want.cases.iter().filter(|(_, l)| unmodeled(l)).count();
            assert_eq!(r.unmodeled, um);
            assert_eq!(r.same, r.cases - um);
        }
        (agree, r)
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn the_report_is_clean_exactly_when_the_outputs_agree() {
        let (want, got) = (any_output(2, 1, &LINES), any_output(2, 1, &LINES));
        let (agree, r) = report_is_clean_exactly_when_the_outputs_agree(&want, &got);
        kani::cover!(agree && r.same == 2, "two cases the same");
        kani::cover!(agree && r.unmodeled == 1 && r.same == 1, "one unmodeled, one the same");
        kani::cover!(!agree && want.cases.len() == got.cases.len() && want.done == got.done && want.head == got.head, "a case differs");
        kani::cover!(got.cases.len() > want.cases.len() && !r.differ.is_empty(), "an unexpected case");
    }

    #[kani::proof]
    #[kani::unwind(4)]
    fn a_case_is_clean_exactly_when_its_lines_agree() {
        let (want, got) = (any_output(1, 2, &PLAIN), any_output(1, 2, &PLAIN));
        let (agree, r) = report_is_clean_exactly_when_the_outputs_agree(&want, &got);
        let lines = |o: &Output| o.cases.first().map_or(0, |(_, l)| l.len());
        kani::cover!(agree && r.same == 1 && lines(&want) == 2, "two lines taken");
        kani::cover!(!agree && lines(&want) == 2 && lines(&got) == 1 && want.done == got.done && want.head == got.head, "a line missing");
        kani::cover!(!agree && lines(&want) == 2 && lines(&got) == 2 && want.cases.first().map(|c| &c.0) == got.cases.first().map(|c| &c.0) && want.done == got.done && want.head == got.head && !want.after_done && !got.after_done, "two lines against two that differ");
    }
}
