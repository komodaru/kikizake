//! (masu's crates/kikizake-lint (a-kaibu/masu), copied: the same lints and rules, the code
//! in src/ instead of crates/; kikizake stays a repository of its own, so it keeps its own copy.)
//!
//! The type-safety lint ratchet: clippy's lints for what the compiler could have caught (an `as`
//! that truncates or changes sign, a `_` arm that swallows a new enum variant, an unwrap), counted
//! per file against tools/lint-ratchet.tsv. A file may only go down: a push that adds one fails
//! CI (write it with From/TryFrom, an exhaustive match, `?`/a typed error instead).
//!
//!   kikizake-lint                        check (CI)
//!   kikizake-lint --update               write the counts down after fixing some (never up)
//!   kikizake-lint --update --raise FILE...
//!                                            those files' lines to what they have now: only for
//!                                            code moved in from another file (say so in the commit)
//!
//! Per file, not per crate: threads that each lower a crate's total merge their tsv lines without
//! a conflict and without one's total hiding the other's additions. `allow`/`expect` attributes
//! naming a tracked lint count as one of their own ("allow-attr"), so silencing is not a way past
//! it. Code under #[cfg(test)] may unwrap (clippy.toml), and test files (tests/, *_tests.rs,
//! tests.rs) are not counted.
//!
//! ROBUST: the lints for code that can panic, overflow, misread memory or drop an error, counted
//! in a second clippy run over the libraries and binaries only. GROUPS: clippy's correctness,
//! suspicious and perf groups. rustc's unsafe_op_in_unsafe_fn (E0133 as a warning) is counted as
//! "unsafe_op_in_unsafe_fn".
//!
//! CUSTOM ("masu-*"): what clippy has no lint of its own for. The unsafe functions are clippy's
//! disallowed_methods, resolved by type: clippy.toml lists them, each reason starting with the
//! masu-* name it is counted under (masu-set_len, masu-from_raw_parts, masu-transmute,
//! masu-uninit; masu-relay, masu's Storage::get: each place the server reads an object's bytes, no
//! relay 2026-11-16). What is syntax, not a call, is found in each file's code with its comments
//! and strings taken out, across lines: `static mut`, `unsafe impl ... Send/Sync for`; and the
//! store's tree format ("vnvm-tree", a string) spelled outside crates/kura/src/tree_list.rs.
//!
//! PINNED: the unsafe kinds (masu-*, undocumented unsafe, unsafe ops in unsafe fns, pointer casts,
//! allow-attr) are also kept site by site in tools/lint-ratchet-sites.tsv: file, lint, the
//! text of its line (spaces collapsed). Every site not in the list fails, even when the count
//! stays (a from_raw_parts removed here, a transmute added there). An edited line is a new site:
//! fix it, or for code moved as it is, --raise its file.
//!
//! ratchet_tests.rs runs every counted lint against tools/lint-fixtures/ratchet (one marked line
//! per regression): a lint a clippy upgrade renamed, or a clippy.toml path that stops resolving,
//! fails there instead of counting 0.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::code::{code_only, line_text};
use crate::tree::Rel;

pub const LINTS: [&str; 10] = [
    "as_conversions",
    "cast_possible_truncation",
    "cast_sign_loss",
    "cast_possible_wrap",
    "wildcard_enum_match_arm",
    "match_wildcard_for_single_variants",
    "unwrap_used",
    "expect_used",
    "fn_to_numeric_cast_any",
    "ptr_as_ptr",
];
pub const ROBUST: [&str; 49] = [
    "indexing_slicing",
    "arithmetic_side_effects",
    "string_slice",
    "get_unwrap",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "panic_in_result_fn",
    "unwrap_in_result",
    "exit",
    "dbg_macro",
    "undocumented_unsafe_blocks",
    "multiple_unsafe_ops_per_block",
    "missing_safety_doc",
    "cast_precision_loss",
    "cast_ptr_alignment",
    "ptr_cast_constness",
    "borrow_as_ptr",
    "as_underscore",
    "char_lit_as_u8",
    "transmute_ptr_to_ptr",
    "transmute_undefined_repr",
    "cast_slice_different_sizes",
    "cast_enum_truncation",
    "fn_to_numeric_cast_with_truncation",
    "invalid_upcast_comparisons",
    "checked_conversions",
    "let_underscore_must_use",
    "unused_result_ok",
    "map_err_ignore",
    "float_cmp",
    "lossy_float_literal",
    "iter_over_hash_type",
    "unchecked_time_subtraction",
    "mem_forget",
    "large_stack_arrays",
    "maybe_infinite_iter",
    "mixed_read_write_in_expression",
    "debug_assert_with_mut_call",
    "fallible_impl_from",
    "empty_drop",
    "same_name_method",
    "rest_pat_in_fully_bound_structs",
    "wildcard_in_or_patterns",
    "suspicious_xor_used_as_pow",
    "unnecessary_safety_comment",
    "try_err",
    "mut_mut",
];
pub const GROUPS: [&str; 3] = ["correctness", "suspicious", "perf"];
/// rustc's code for unsafe_op_in_unsafe_fn
pub const UNSAFE_OP: (&str, &str) = ("E0133", "unsafe_op_in_unsafe_fn");
pub const PINNED: [&str; 10] = [
    "undocumented_unsafe_blocks",
    "multiple_unsafe_ops_per_block",
    "missing_safety_doc",
    "not_unsafe_ptr_arg_deref",
    "unsafe_op_in_unsafe_fn",
    "cast_ptr_alignment",
    "ptr_as_ptr",
    "borrow_as_ptr",
    "missing_transmute_annotations",
    "allow-attr",
];
/// (code with comments and strings out: across lines, `[^{;]*?` stops at the impl's body)
pub const CUSTOM: [(&str, &str); 2] = [("masu-static_mut", r"\bstatic\s+mut\b"), ("masu-unsafe_send_sync", r"\bunsafe\s+impl\b[^{;]*?\b(?:Send|Sync)\s+for\b")];
/// (code with its strings: it is one)
pub const RAW: [(&str, &str); 1] = [("masu-wire_tree", concat!("vnvm", "-tree"))];
pub const CUSTOM_HOME: [(&str, &str); 1] = [("masu-wire_tree", "crates/kura/src/tree_list.rs")];

/// Where a lint is counted: a file, a 1-based line and column.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hit {
    pub file: Rel,
    pub line: usize,
    pub col: usize,
    pub lint: String,
}

pub fn pinned(lint: &str) -> bool {
    PINNED.contains(&lint) || lint.starts_with("masu-")
}

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static TRACKED: LazyLock<Option<Regex>> = LazyLock::new(|| {
    let names: Vec<&str> = LINTS.iter().chain(ROBUST.iter()).chain(GROUPS.iter()).copied().collect();
    rx(&format!(r"\b(?:clippy::(?:{})|{})\b", names.join("|"), UNSAFE_OP.1))
});
static ALLOW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"#!?\[\s*(?:allow|expect)\s*\(([^\]]*)\)\s*\]"));
static BACKTICK: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"`([^`]+)`"));

/// Normalize `p` with `.` and `..` resolved (a #[path] include's `src/../../kura/x.rs`)
fn normalize(p: &str) -> String {
    let mut out: Vec<&str> = vec![];
    for s in p.split(['/', '\\']) {
        match s {
            "" | "." => {}
            ".." => {
                if out.pop().is_none() {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    let joined = out.join("/");
    if p.starts_with(['/', '\\']) { format!("/{joined}") } else { joined }
}

/// The repository (or a fixture) whose lints are counted.
pub struct Repo {
    pub root: PathBuf,
    /// where its Rust code is (allow-attr and the custom lints walk these)
    pub dirs: Vec<&'static str>,
    /// the clippy.toml it is checked with, and where clippy builds
    pub conf: PathBuf,
    pub target: Option<PathBuf>,
}

fn clippy_lints(first: bool) -> Vec<String> {
    if first {
        LINTS.iter().map(|l| format!("clippy::{l}")).chain(GROUPS.iter().map(|g| format!("clippy::{g}"))).collect()
    } else {
        ROBUST.iter().map(|l| format!("clippy::{l}")).chain([UNSAFE_OP.1.to_owned(), "clippy::disallowed_methods".to_owned()]).collect()
    }
}

/// clippy's two runs: (LINTS + GROUPS, --all-targets), (ROBUST, libraries and binaries)
pub fn clippy(repo: &Repo, disallowed: &BTreeMap<String, Option<String>>) -> Result<BTreeSet<Hit>, String> {
    let mut seen = BTreeSet::new();
    for (targets, first) in [(&["--all-targets"][..], true), (&["--lib", "--bins"][..], false)] {
        let mut c = std::process::Command::new("cargo");
        c.args(["clippy", "--workspace"]).args(targets).args(["--keep-going", "--message-format=json", "--", "--cap-lints", "warn", "-A", "clippy::all"]);
        for l in clippy_lints(first) {
            // (forced: an #[allow] in the code does not hide a counted lint from the count)
            c.args(["--force-warn", &l]);
        }
        // (the warnings are what it counts: CI's CARGO_BUILD_WARNINGS=deny would fail the run)
        c.current_dir(&repo.root).env("CARGO_BUILD_WARNINGS", "warn").env("CLIPPY_CONF_DIR", &repo.conf).stderr(std::process::Stdio::inherit());
        if let Some(t) = &repo.target {
            c.env("CARGO_TARGET_DIR", t);
        }
        let out = c.output().map_err(|e| format!("cargo clippy: {e}"))?;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            if let Some(h) = message(line, disallowed)? {
                seen.insert(h);
            }
        }
        if !out.status.success() {
            return Err("clippy failed (above)".into());
        }
    }
    Ok(seen)
}

/// one line of cargo's JSON: the counted lint it reports, if any
fn message(line: &str, disallowed: &BTreeMap<String, Option<String>>) -> Result<Option<Hit>, String> {
    let Ok(m) = serde_json::from_str::<serde_json::Value>(line) else { return Ok(None) };
    if m.get("reason").and_then(|r| r.as_str()) != Some("compiler-message") {
        return Ok(None);
    }
    let Some(msg) = m.get("message") else { return Ok(None) };
    let level = msg.get("level").and_then(|l| l.as_str()).unwrap_or_default();
    let text = msg.get("message").and_then(|t| t.as_str()).unwrap_or_default();
    if level == "error" {
        eprint!("{}", msg.get("rendered").and_then(|r| r.as_str()).unwrap_or(text));
    }
    let code = msg.get("code").and_then(|c| c.get("code")).and_then(|c| c.as_str()).unwrap_or_default();
    let lint = if level == "warning" && code == UNSAFE_OP.0 {
        UNSAFE_OP.1.to_owned()
    } else if let Some(l) = code.strip_prefix("clippy::") {
        if l == "disallowed_methods" {
            let path = BACKTICK.as_ref().and_then(|r| r.captures(text)).and_then(|c| c.get(1)).map_or_default(|p| p.as_str());
            match disallowed.get(path) {
                Some(Some(name)) => name.clone(),
                Some(None) | None => return Err(format!("a disallowed method clippy.toml names no masu-* name for: {text}")),
            }
        } else {
            l.to_owned()
        }
    } else {
        return Ok(None);
    };
    let spans = msg.get("spans").and_then(|s| s.as_array()).cloned().unwrap_or_default();
    let Some(sp) = spans.iter().find(|s| s.get("is_primary").and_then(|p| p.as_bool()) == Some(true)) else { return Ok(None) };
    let file = sp.get("file_name").and_then(|f| f.as_str()).unwrap_or_default();
    let num = |k: &str| sp.get(k).and_then(|v| v.as_u64()).and_then(|v| usize::try_from(v).ok()).unwrap_or(0);
    // (a lib's code is checked twice with --all-targets: as the lib and as its tests; a #[path]
    // include once under each crate's spelling of its path)
    Ok(Some(Hit { file: Rel(normalize(file)), line: num("line_start"), col: num("column_start"), lint }))
}

/// every .rs file under the repo's dirs: (relative path, text)
fn sources(repo: &Repo) -> Vec<(Rel, String)> {
    let mut out = vec![];
    let mut stack: Vec<PathBuf> = repo.dirs.iter().map(|d| repo.root.join(d)).collect();
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs")
                && let (Ok(rel), Ok(b)) = (p.strip_prefix(&repo.root), std::fs::read(&p))
            {
                out.push((Rel(rel.to_string_lossy().replace('\\', "/")), String::from_utf8_lossy(&b).into_owned()));
            }
        }
    }
    out.sort();
    out
}

/// `allow`/`expect` attributes naming a tracked lint, read as tokens: outer and inner, over several
/// lines, inside `cfg_attr(...)` and inside macro bodies (comments are not tokens). A file that does
/// not lex falls back to the raw text, line by line.
pub fn allow_attrs(files: &[(Rel, String)]) -> Vec<Hit> {
    let Some(tracked) = TRACKED.as_ref() else { return vec![] };
    let mut out = vec![];
    for (rel, text) in files {
        match text.parse::<proc_macro2::TokenStream>() {
            Ok(ts) => attrs_in(ts, &mut |line, col, body| {
                if tracked.is_match(&body) {
                    out.push(Hit { file: rel.clone(), line, col, lint: "allow-attr".into() });
                }
            }),
            Err(_) => {
                let Some(allow) = ALLOW.as_ref() else { continue };
                for (k, line) in text.split('\n').enumerate() {
                    for c in allow.captures_iter(line) {
                        if let (Some(all), Some(inner)) = (c.get(0), c.get(1))
                            && tracked.is_match(inner.as_str())
                        {
                            let col = line.get(..all.start()).map_or(0, |p| p.chars().count()).saturating_add(1);
                            out.push(Hit { file: rel.clone(), line: k.saturating_add(1), col, lint: "allow-attr".into() });
                        }
                    }
                }
            }
        }
    }
    out
}

/// each `#[allow(..)]`, `#![expect(..)]`, `#[cfg_attr(.., allow(..))]` in `ts`: (line, column, the
/// lint list with whitespace removed)
fn attrs_in(ts: proc_macro2::TokenStream, f: &mut impl FnMut(usize, usize, String)) {
    use proc_macro2::{Delimiter, TokenTree};
    let toks: Vec<TokenTree> = ts.into_iter().collect();
    for (i, t) in toks.iter().enumerate() {
        match t {
            TokenTree::Punct(p) if p.as_char() == '#' => {
                let mut j = i.saturating_add(1);
                if matches!(toks.get(j), Some(TokenTree::Punct(b)) if b.as_char() == '!') {
                    j = j.saturating_add(1);
                }
                if let Some(TokenTree::Group(g)) = toks.get(j)
                    && g.delimiter() == Delimiter::Bracket
                    && let Some(lints) = allowed(g.stream(), true)
                {
                    let s = p.span().start();
                    f(s.line, s.column.saturating_add(1), lints);
                }
            }
            TokenTree::Group(g) => attrs_in(g.stream(), f),
            TokenTree::Punct(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => {}
        }
    }
}

/// an attribute's inside: the lints it allows (`allow(..)`, `expect(..)`, or either nested in
/// `cfg_attr(cond, ..)`), whitespace removed; None when it allows nothing
fn allowed(inside: proc_macro2::TokenStream, top: bool) -> Option<String> {
    use proc_macro2::{Delimiter, TokenTree};
    let toks: Vec<TokenTree> = inside.into_iter().collect();
    let mut found: Option<String> = None;
    for (k, t) in toks.iter().enumerate() {
        let TokenTree::Ident(id) = t else { continue };
        let Some(TokenTree::Group(g)) = toks.get(k.saturating_add(1)) else { continue };
        if g.delimiter() != Delimiter::Parenthesis {
            continue;
        }
        let name = id.to_string();
        let got = if name == "allow" || name == "expect" {
            Some(g.stream().to_string().split_whitespace().collect::<String>())
        } else if name == "cfg_attr" {
            allowed(g.stream(), false)
        } else {
            None
        };
        if let Some(g) = got {
            found = Some(match found {
                Some(f) => format!("{f},{g}"),
                None => g,
            });
        }
        // (at the top only the attribute's own name counts: `path(..)` in `#[x(path(allow(..)))]` does
        // not; inside cfg_attr every attribute after the condition does)
        if top {
            break;
        }
    }
    found
}

/// the masu-* lints found by text: static mut, unsafe impl Send/Sync, the wire format's name
pub fn custom(files: &[(Rel, String)]) -> Vec<Hit> {
    let rules: Vec<(&str, Regex, bool)> = CUSTOM.iter().map(|(n, r)| (*n, r, false)).chain(RAW.iter().map(|(n, r)| (*n, r, true))).filter_map(|(n, r, s)| rx(r).map(|r| (n, r, s))).collect();
    let mut out = vec![];
    for (rel, text) in files {
        let (code, with_strings) = (code_only(text, false), code_only(text, true));
        for (name, r, strings) in &rules {
            if CUSTOM_HOME.iter().any(|(n, h)| n == name && *h == rel.as_str()) {
                continue;
            }
            let src = if *strings { &with_strings } else { &code };
            for m in r.find_iter(src) {
                let line = crate::code::line_of(src, m.start());
                out.push(Hit { file: rel.clone(), line, col: m.start().saturating_add(1), lint: (*name).to_owned() });
            }
        }
    }
    out
}

/// Counts per (file, lint) and per pinned site (file, lint, line text), test files left out.
pub struct Counts {
    pub files: BTreeMap<(Rel, String), usize>,
    pub sites: BTreeMap<(Rel, String, String), usize>,
    pub where_: BTreeMap<(Rel, String, String), usize>,
}

pub fn count(repo: &Repo, hits: &BTreeSet<Hit>) -> Counts {
    let srcs = sources(repo);
    let mut all: Vec<Hit> = hits.iter().cloned().collect();
    all.extend(allow_attrs(&srcs));
    all.extend(custom(&srcs));
    let mut c = Counts { files: BTreeMap::new(), sites: BTreeMap::new(), where_: BTreeMap::new() };
    let mut lines: BTreeMap<Rel, String> = BTreeMap::new();
    for h in all {
        if h.file.is_test_file() {
            continue;
        }
        bump(&mut c.files, (h.file.clone(), h.lint.clone()));
        if pinned(&h.lint) {
            let code = lines.entry(h.file.clone()).or_insert_with(|| std::fs::read(repo.root.join(h.file.as_str())).map_or_default(|b| code_only(&String::from_utf8_lossy(&b), true)));
            let k = (h.file.clone(), h.lint.clone(), line_text(code, h.line));
            bump(&mut c.sites, k.clone());
            c.where_.entry(k).or_insert(h.line);
        }
    }
    c
}

fn bump<K: Ord>(m: &mut BTreeMap<K, usize>, k: K) {
    let e = m.entry(k).or_default();
    *e = e.saturating_add(1);
}

pub fn load_counts(text: &str) -> BTreeMap<(Rel, String), usize> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| {
            let v: Vec<&str> = l.split('\t').collect();
            // (crate lines: what the ratchet enforced before 2026-11-14 18:40Z; ignored)
            match v.as_slice() {
                ["file", f, lint, n] => Some(((Rel::new(f), (*lint).to_owned()), n.parse().ok()?)),
                _ => None,
            }
        })
        .collect()
}

pub fn save_counts(m: &BTreeMap<(Rel, String), usize>) -> String {
    let mut s = String::from("# kikizake-lint: type-safety, robustness and masu-* lints per file (enforced: never up)\n");
    for ((f, l), n) in m {
        if *n > 0 {
            s.push_str(&format!("file\t{f}\t{l}\t{n}\n"));
        }
    }
    s
}

pub fn load_sites(text: &str) -> BTreeMap<(Rel, String, String), usize> {
    let mut m = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut c = l.splitn(3, '\t');
        if let (Some(f), Some(lint), Some(t)) = (c.next(), c.next(), c.next()) {
            bump(&mut m, (Rel::new(f), lint.to_owned(), t.to_owned()));
        }
    }
    m
}

pub fn save_sites(m: &BTreeMap<(Rel, String, String), usize>) -> String {
    let mut s = String::from("# kikizake-lint: every unsafe-kind site (file, lint, its line's text); a site not here fails CI\n");
    for ((f, l, t), n) in m {
        for _ in 0..*n {
            s.push_str(&format!("{f}\t{l}\t{t}\n"));
        }
    }
    s
}

/// the pinned sites more often now than in the list: a count that stays the same does not hide
/// one site traded for another
pub fn fresh_sites<'a>(now: &'a BTreeMap<(Rel, String, String), usize>, known: &BTreeMap<(Rel, String, String), usize>, raise: &BTreeSet<Rel>) -> Vec<&'a (Rel, String, String)> {
    now.iter().filter(|(k, n)| **n > known.get(*k).copied().unwrap_or(0) && !raise.contains(&k.0)).map(|(k, _)| k).collect()
}

pub struct Paths {
    pub counts: PathBuf,
    pub sites: PathBuf,
}

/// the check, or with `update` the lists written down (only down, but `raise` files as they are)
pub fn run(repo: &Repo, paths: &Paths, update: bool, raise: &BTreeSet<Rel>) -> Result<String, String> {
    let disallowed = crate::disallowed(&std::fs::read_to_string(repo.conf.join("clippy.toml")).unwrap_or_default());
    let hits = clippy(repo, &disallowed)?;
    let now = count(repo, &hits);
    let old = load_counts(&std::fs::read_to_string(&paths.counts).unwrap_or_default());
    let first = !paths.sites.exists();
    let old_sites = load_sites(&std::fs::read_to_string(&paths.sites).unwrap_or_default());
    let mut bad = vec![];
    for (k, n) in &now.files {
        let was = old.get(k).copied().unwrap_or(0);
        if *n > was && !raise.contains(&k.0) {
            bad.push(format!("kikizake-lint: {}: clippy::{} {was} -> {n}", k.0, k.1));
        }
    }
    if !first {
        for k in fresh_sites(&now.sites, &old_sites, raise) {
            let at = now.where_.get(k).copied().unwrap_or(0);
            bad.push(format!("kikizake-lint: {}:{at}: {}: a new site (not in {}): {}", k.0, k.1, shown(&paths.sites, &repo.root), k.2));
        }
    }
    for b in &bad {
        println!("{b}");
    }
    let down: usize = old.iter().map(|(k, n)| n.saturating_sub(now.files.get(k).copied().unwrap_or(0))).sum();
    if update {
        if !bad.is_empty() {
            return Err("counts above went up or sites are new: fix them (the baseline only goes down)".into());
        }
        let counts: BTreeMap<_, _> = now.files.iter().map(|(k, n)| (k.clone(), if raise.contains(&k.0) { *n } else { (*n).min(old.get(k).copied().unwrap_or(*n)) })).collect();
        let sites: BTreeMap<_, _> =
            now.sites.iter().map(|(k, n)| (k.clone(), if first || raise.contains(&k.0) { *n } else { (*n).min(old_sites.get(k).copied().unwrap_or(0)) })).filter(|(_, n)| *n > 0).collect();
        std::fs::write(&paths.counts, save_counts(&counts)).map_err(|e| e.to_string())?;
        std::fs::write(&paths.sites, save_sites(&sites)).map_err(|e| e.to_string())?;
        return Ok(format!("kikizake-lint: wrote {} ({} in all, {down} fewer)", shown(&paths.counts, &repo.root), counts.values().sum::<usize>()));
    }
    if down > 0 {
        println!("kikizake-lint: {down} fewer than the baseline (kikizake-lint --update writes it down)");
    }
    if !bad.is_empty() {
        return Err("lints went up or unsafe sites are new (above): From/TryFrom instead of `as`, every variant named instead of `_`, `?` or a typed error instead of unwrap/panic, get()/checked_*/wrapping_* instead of a[i] and bare arithmetic, // SAFETY: on unsafe".into());
    }
    Ok(format!("kikizake-lint: ok ({} left)", now.files.values().sum::<usize>()))
}

/// `p` as the repository names it
fn shown(p: &Path, root: &Path) -> String {
    p.trim_prefix(root).display().to_string()
}

pub fn repo_paths(root: &Path) -> Paths {
    Paths { counts: root.join("tools/lint-ratchet.tsv"), sites: root.join("tools/lint-ratchet-sites.tsv") }
}

#[cfg(test)]
#[path = "ratchet_tests.rs"]
mod tests;
