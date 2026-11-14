//! shirusu's recordings of the executor on Windows (calls/, src/calls.rs) against the cases:
//! every case line spec/calls.txt names made its call with the case's arguments, x86 and x86-64
//! made the same calls, and recording did not change what the executor printed. A recording made
//! from other cases than the current ones waits for the shirusu workflow to record them again.
use std::path::PathBuf;

#[test]
fn recordings_match_the_cases() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let c = kikizake::calls::check_dir(&root.join("calls"), &root);
    eprint!("{}", c.report());
    assert!(c.problems() == 0, "{} problems (above)", c.problems());
}
