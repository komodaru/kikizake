//! shirusu's recordings of the executor on Windows (calls/, src/calls.rs) against the cases:
//! every case line spec/calls.txt names made its call with the case's arguments, x86 and x86-64
//! made the same calls, and recording did not change what the executor printed. A recording made
//! from other cases than the current ones waits for the shirusu workflow to record them again.
use std::path::PathBuf;

#[test]
fn recordings_match_the_cases() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // (the whole set: every suite of spec/calls-suites.txt on each architecture, current)
    let required = kikizake::calls::required(&root).unwrap_or_else(|e| panic!("{e}"));
    let c = kikizake::calls::check_complete(&root.join("calls"), &root, &required);
    eprint!("{}", c.report());
    assert!(c.problems() == 0, "{} problems (above)", c.problems());
}

/// the whole set's check fails where the review of 8480456 (5.3) found the partial one
/// passing: a directory of one recording, and that recording's src and shirusu made obsolete
#[test]
fn incomplete_or_stale_sets_fail() {
    use kikizake::calls::{check_complete, CasesHash, Wanted};
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tmp = std::env::temp_dir().join(format!("kikizake-calls-{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let rec = std::fs::read_to_string(root.join("calls/gdi-32.txt")).unwrap();
    let head = rec.lines().next().unwrap().to_string();
    let field = |k: &str| head.split(' ').skip_while(|w| *w != k).nth(1).unwrap().to_string();
    let cases = std::fs::read_to_string(root.join("cases/gdi.txt")).unwrap();
    let want = |n: &str| (n.to_string(), Wanted { cases: CasesHash::of(&cases), src: field("src"), shirusu: field("shirusu") });
    std::fs::write(tmp.join("gdi-32.txt"), &rec).unwrap();
    // (as recorded: no problem of its own; the other architecture missing is one)
    let ok = check_complete(&tmp, &root, &[want("gdi-32")]);
    assert!(ok.missing.is_empty(), "{}", ok.report());
    let one = check_complete(&tmp, &root, &[want("gdi-32"), want("gdi-64")]);
    assert_eq!(one.missing.len(), 1, "{}", one.report());
    assert!(one.missing[0].starts_with("gdi-64:"));
    // (another source and shirusu in the header: stale)
    let old = rec.replacen(&format!(" src {} ", field("src")), " src obsolete ", 1).replacen(&format!(" shirusu {} ", field("shirusu")), " shirusu obsolete ", 1);
    std::fs::write(tmp.join("gdi-32.txt"), old).unwrap();
    let stale = check_complete(&tmp, &root, &[want("gdi-32")]);
    assert_eq!(stale.missing.len(), 2, "{}", stale.report());
    assert!(stale.problems() >= 2);
    // (nothing required is no complete set)
    assert!(!check_complete(&tmp, &root, &[]).missing.is_empty());
    std::fs::remove_dir_all(&tmp).unwrap();
}
