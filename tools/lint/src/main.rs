//! kikizake-lint: the type-safety lint ratchet (src/ratchet.rs, a copy of masu's).
//!
//!   cargo run -q --release -p kikizake-lint [-- --update [--raise FILE...]]
//!   cargo run -q --release -p kikizake-lint -- --dup-list | --dup-update
//!
//! It also finds copied functions and tables (src/dup.rs, the same file as masu-lint's): the
//! copies kept on purpose are tools/lint/dup-known.tsv lines.
//!
//! `cargo test -p kikizake-lint` proves each counted lint fires on its regression.
// (Rust 1.97: a pub item nothing in the binary uses is dead; tests use more of it, so not under test)
#![cfg_attr(not(test), warn(dead_code_pub_in_binary))]
mod cases;
mod code;
mod dup;
mod escape;
mod formal;
mod guards;
mod modern;
mod ratchet;
mod tree;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::LazyLock;

use regex::Regex;
use tree::Rel;

/// the copies kept on purpose: kind, each copy as file#name, why
pub const DUP_KNOWN: &str = "tools/lint/dup-known.tsv";

/// the tracked Rust files dup reads (the lint's fixtures are made to be copies)
fn dup_files(root: &Path) -> Result<Vec<(String, String)>, String> {
    let out = std::process::Command::new("git").args(["ls-files", "-z", "*.rs"]).current_dir(root).output().map_err(|e| format!("git ls-files: {e}"))?;
    let mut files = Vec::new();
    for p in String::from_utf8_lossy(&out.stdout).split('\0').filter(|s| !s.is_empty() && !s.starts_with("tools/lint-fixtures/") && !s.starts_with("vendor/")) {
        let text = std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"))?;
        files.push((p.to_owned(), String::from_utf8_lossy(&text).into_owned()));
    }
    Ok(files)
}

/// `--dup-list` prints every group, `--dup-update` drops the gone lines of the known list;
/// otherwise a group not known is an error
fn dup_check(root: &Path, args: &[String]) -> Result<Option<String>, String> {
    let found = dup::find(&dup_files(root)?);
    let path = root.join(DUP_KNOWN);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    if args.iter().any(|a| a == "--dup-list") {
        print!("{}", dup::list(&found));
        return Ok(Some(format!("kikizake-lint dup: {} groups", found.len())));
    }
    if args.iter().any(|a| a == "--dup-update") {
        std::fs::write(&path, dup::updated(&text, &found)).map_err(|e| e.to_string())?;
        return Ok(Some(format!("kikizake-lint dup: wrote {DUP_KNOWN}")));
    }
    let new = dup::fresh(&found, &dup::load_known(&text));
    if !new.is_empty() {
        return Err(format!("{}kikizake-lint dup: {} groups of copied code (tools/lint/src/dup.rs)", dup::report(&new, DUP_KNOWN), new.len()));
    }
    Ok(None)
}

/// the repository this binary lints (it lives in it)
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static DISALLOWED: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r#"\{\s*path\s*=\s*"([^"]+)"\s*,\s*reason\s*=\s*"([^"]*)""#));

/// clippy.toml's disallowed-methods: path -> its masu-* name (None: the reason names none)
pub fn disallowed(clippy_toml: &str) -> BTreeMap<String, Option<String>> {
    let Some(r) = DISALLOWED.as_ref() else { return BTreeMap::new() };
    r.captures_iter(clippy_toml)
        .filter_map(|c| {
            let path = c.get(1)?.as_str().to_owned();
            let reason = c.get(2)?.as_str();
            let name = reason.split(':').next().filter(|n| n.starts_with("masu-") && !n.contains(' ')).map(str::to_owned);
            Some((path, name))
        })
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let update = args.iter().any(|a| a == "--update");
    let raise: BTreeSet<Rel> = args.iter().skip_while(|a| *a != "--raise").skip(1).map(|a| Rel::new(a)).collect();
    let root = root();
    if let Err(error) = cases::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = guards::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = modern::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = formal::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    if let Err(error) = escape::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    match dup_check(&root, &args) {
        Ok(Some(m)) => {
            println!("{m}");
            return ExitCode::SUCCESS;
        }
        Ok(None) => {}
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    }
    if args == ["--cases"] {
        return ExitCode::SUCCESS;
    }
    let repo = ratchet::Repo { root: root.clone(), dirs: vec!["src"], conf: root.clone(), target: None };
    match ratchet::run(&repo, &ratchet::repo_paths(&root), update, &raise) {
        Ok(m) => {
            println!("{m}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
