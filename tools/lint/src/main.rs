//! kikizake-lint: the type-safety lint ratchet (src/ratchet.rs, a copy of masu's).
//!
//!   cargo run -q --release -p kikizake-lint [-- --update [--raise FILE...]]
//!
//! `cargo test -p kikizake-lint` proves each counted lint fires on its regression.
// (Rust 1.97: a pub item nothing in the binary uses is dead; tests use more of it, so not under test)
#![cfg_attr(not(test), warn(dead_code_pub_in_binary))]
mod cases;
mod code;
mod escape;
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
    if let Err(error) = escape::check(&root) {
        eprintln!("{error}");
        return ExitCode::FAILURE;
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
