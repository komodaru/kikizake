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

struct OutputView<'a, Line, Lines> {
    head: &'a [Line],
    cases: &'a [(Line, Lines)],
    done: bool,
    after_done: bool,
}

impl<'a> From<&'a Output> for OutputView<'a, String, Vec<String>> {
    fn from(output: &'a Output) -> Self {
        Self { head: &output.head, cases: &output.cases, done: output.done, after_done: output.after_done }
    }
}

#[derive(Default)]
struct Counts {
    cases: usize,
    same: usize,
    unmodeled: usize,
}

enum Difference<'a, Line> {
    AfterDone,
    NoExpectedCases,
    Header,
    Missing(&'a str),
    CaseName { position: usize, expected: &'a str, actual: &'a str },
    InvalidModel(&'a str),
    Line { name: &'a str, index: usize, lines: &'a [Line], actual: Option<&'a str> },
    Unexpected(&'a str),
    End,
}

fn describe_line(name: &str, index: usize, lines: &[String], actual: Option<&str>) -> String {
    let expected = lines.get(index).map(String::as_str);
    let number = index.saturating_add(1);
    let mut message = format!("{name}: line {number}: want {:?} got {:?}",
        expected.unwrap_or("(none)"), actual.unwrap_or("(none)"));
    if let (Some(a), Some(b)) = (expected, actual)
        && a.len() == b.len() && a.bytes().all(|c| c.is_ascii_hexdigit()) {
        let bpp = lines.iter().take(number).rev()
            .find_map(|line| line.strip_prefix("px "))
            .and_then(|px| px.split(' ').nth(3))
            .and_then(|depth| depth.parse::<u32>().ok()).unwrap_or(0);
        if bpp > 0 {
            let row = lines.iter().take(index).rev().take_while(|line| !line.starts_with("px ")).count();
            if let Some(x) = first_pixel(a, b, bpp) {
                message += &format!(" (row {row}, first pixel {x})");
            }
        }
    }
    message
}

pub fn compare(want: &Output, got: &Output) -> Report {
    let mut differ = Vec::new();
    let counts = compare_views(want.into(), got.into(), line_matches, |difference| differ.push(match difference {
        Difference::AfterDone => "output after done".into(),
        Difference::NoExpectedCases => "no expected cases".into(),
        Difference::Header => format!("output header: want {:?}, got {:?}", want.head, got.head),
        Difference::Missing(name) => format!("{name}: missing (the program stopped or skipped it)"),
        Difference::CaseName { position, expected, actual } => format!("case {position}: want {expected}, got {actual}"),
        Difference::InvalidModel(name) => format!("{name}: invalid model input"),
        Difference::Line { name, index, lines, actual } => describe_line(name, index, lines, actual),
        Difference::Unexpected(name) => format!("{name}: unexpected case"),
        Difference::End => if want.done {
            "program end: missing done marker".into()
        } else {
            "program end: unexpected done marker".into()
        },
    }));
    Report { cases: counts.cases, same: counts.same, unmodeled: counts.unmodeled, differ }
}

/// Walk borrowed output with a diagnostic sink. The verifier uses fixed arrays
/// and counts differences through the same traversal as the report.
fn compare_views<'a, Line, Lines>(
    want: OutputView<'a, Line, Lines>,
    got: OutputView<'a, Line, Lines>,
    mut matches: impl FnMut(Option<&str>, Option<&str>) -> bool,
    mut report: impl FnMut(Difference<'a, Line>),
) -> Counts
where
    Line: AsRef<str> + PartialEq + 'a,
    Lines: AsRef<[Line]> + 'a,
{
    let mut counts = Counts::default();
    if want.after_done || got.after_done { report(Difference::AfterDone); }
    if want.cases.is_empty() { report(Difference::NoExpectedCases); }
    if want.head != got.head { report(Difference::Header); }
    for (i, (name, wl)) in want.cases.iter().enumerate() {
        counts.cases += 1;
        let Some((actual, gl)) = got.cases.get(i) else {
            report(Difference::Missing(name.as_ref()));
            continue;
        };
        let (name, actual) = (name.as_ref(), actual.as_ref());
        if actual != name {
            report(Difference::CaseName { position: i + 1, expected: name, actual });
            continue;
        }
        let (wl, gl) = (wl.as_ref(), gl.as_ref());
        if wl.first().map(AsRef::as_ref).is_some_and(|l| l == "invalid case" || l == "unmodeled bad line") {
            report(Difference::InvalidModel(name));
            continue;
        }
        if wl.first().map(AsRef::as_ref).is_some_and(|l| l.starts_with("unmodeled ")) {
            counts.unmodeled += 1;
            continue;
        }
        let first_difference = (0..wl.len().max(gl.len())).find(|&index| {
            !matches(wl.get(index).map(AsRef::as_ref), gl.get(index).map(AsRef::as_ref))
        });
        if let Some(index) = first_difference {
            report(Difference::Line {
                name, index, lines: wl, actual: gl.get(index).map(AsRef::as_ref),
            });
        } else {
            counts.same += 1;
        }
    }
    for (name, _) in got.cases.iter().skip(want.cases.len()) {
        report(Difference::Unexpected(name.as_ref()));
    }
    if want.done != got.done { report(Difference::End); }
    counts
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
    fn comparator_report_structure() {
        let want_header: bool = kani::any();
        let got_header: bool = kani::any();
        let want_count: u8 = kani::any();
        let got_count: u8 = kani::any();
        kani::assume(want_count <= 2 && got_count <= 2);
        let names_match: [bool; 2] = kani::any();
        let want_done: bool = kani::any();
        let got_done: bool = kani::any();
        let want_extra: bool = kani::any();
        let got_extra: bool = kani::any();
        let [first_ok, second_ok] = names_match;
        let header = ["h"];
        let empty: &[&str] = &[];
        let expected = [("a", empty), ("b", empty)];
        let actual = [(if first_ok { "a" } else { "b" }, empty),
            (if second_ok { "b" } else { "a" }, empty)];
        let want = super::OutputView {
            head: if want_header { &header } else { empty },
            cases: &expected[..usize::from(want_count)], done: want_done, after_done: want_extra,
        };
        let got = super::OutputView {
            head: if got_header { &header } else { empty },
            cases: &actual[..usize::from(got_count)], done: got_done, after_done: got_extra,
        };
        let mut differences = 0;
        let report = super::compare_views(want, got, |_, _| unreachable!(), |_| differences += 1);
        let common = want_count.min(got_count);
        let same = usize::from(common >= 1 && first_ok) + usize::from(common >= 2 && second_ok);
        let case_errors = usize::from(want_count.max(got_count)) - same;
        let structure_errors = usize::from(want_header != got_header) + usize::from(want_count == 0)
            + usize::from(want_done != got_done) + usize::from(want_extra || got_extra);
        assert_eq!(differences, structure_errors + case_errors);
        assert_eq!(report.same, same);
        assert_eq!(report.cases, usize::from(want_count));
        assert_eq!(report.unmodeled, 0);
    }

    #[kani::proof]
    #[kani::unwind(20)]
    fn comparator_report_lines() {
        let kind: u8 = kani::any();
        let wanted = match kind {
            0 => "a", 1 => "unmodeled RULE", 2 => "invalid case", _ => "unmodeled bad line",
        };
        let present: bool = kani::any();
        let want_tail: bool = kani::any();
        let got_tail: bool = kani::any();
        let first_matches: bool = kani::any();
        let tail_matches: bool = kani::any();
        let expected_lines = [wanted, "c"];
        let actual_lines = [if present { "b" } else { "d" }, "d"];
        let expected = [("a", &expected_lines[..1 + usize::from(want_tail)])];
        let actual = [("a", &actual_lines[..usize::from(present) + usize::from(got_tail)])];
        let want = super::OutputView { head: &[], cases: &expected, done: true, after_done: false };
        let got = super::OutputView { head: &[], cases: &actual, done: true, after_done: false };
        // Give either row an arbitrary match result. The separate line proofs check
        // the production matcher's handling of wildcards and enumerated choices.
        let matches = |a: Option<&str>, b: Option<&str>| match (a, b) {
            (Some("a"), Some("b")) => first_matches,
            (Some("c"), Some("d")) => tail_matches,
            _ => false,
        };
        let mut differences = 0;
        let report = super::compare_views(want, got, matches, |_| differences += 1);
        let matches_body = kind == 0 && present && first_matches && want_tail == got_tail
            && (!want_tail || tail_matches);
        assert_eq!(differences, usize::from(!(matches_body || kind == 1)));
        assert_eq!(report.same, usize::from(matches_body));
        assert_eq!(report.unmodeled, usize::from(kind == 1));
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

#[test]
fn first_differing_pixel_is_relative_to_the_last_image_header() {
    let want = "case image\npx 0 8 1 1\n00\npx 0 2 2 32\n0000000000000000\n0000000000000000\ndone\n";
    let got = want.replace("0000000000000000\ndone", "0000000000110000\ndone");
    assert_eq!(compare_text(want, &got).differ, [
        "image: line 5: want \"0000000000000000\" got \"0000000000110000\" (row 1, first pixel 1)",
    ]);
}
