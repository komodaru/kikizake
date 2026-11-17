use super::*;

#[test]
fn comments_and_strings_are_blanked_lines_kept() {
    let src = "let a = 1; // static mut\n/* outer /* inner */ still */ let b = \"vnvm-tree\";\nlet c = b\"x\"; let d = r#\"q\"#;";
    let code = code_only(src, false);
    assert_eq!(code.lines().count(), 3);
    assert_eq!(code.chars().count(), src.chars().count());
    assert!(!code.contains("static mut") && !code.contains("vnvm-tree") && !code.contains("inner") && !code.contains("still"));
    assert!(code.contains("let a = 1;") && code.contains("let b =") && code.contains("let d ="));
    let with = code_only(src, true);
    assert!(with.contains("\"vnvm-tree\"") && with.contains("r#\"q\"#") && !with.contains("static mut"));
}

#[test]
fn chars_and_lifetimes() {
    let src = "fn f<'a>(x: &'a str) -> char { if x == \"'\" { '\\'' } else { '\\u{1F600}' } }";
    let code = code_only(src, false);
    assert!(code.contains("<'a>") && code.contains("&'a str"));
    assert!(!code.contains("\\u{1F600}") && !code.contains("'\\''"));
}

#[test]
fn a_word_ending_in_r_or_b_is_no_raw_string() {
    let src = "let bar = br\"x\"; let r = for_r\"y\";";
    let code = code_only(src, false);
    assert!(code.contains("let bar ="));
    assert!(!code.contains("br\"x\""));
}

#[test]
fn line_text_collapses_spaces() {
    assert_eq!(line_text("a\n  b   c \nd", 2), "b c");
    assert_eq!(line_text("a", 5), "");
    assert_eq!(line_of("a\nb\nc", 4), 3);
}
