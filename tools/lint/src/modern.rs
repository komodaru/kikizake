//! Rust 1.97..1.100 の機能で型や設定に移した決まり (2026-11-21、こた「Rust 1.100 の新機能を全部
//! 入れて、変更箇所の退行がないように自作リンタも」)。基準値なし、絶対の決まり。
//!
//! * `wildcard-unreachable`: `_ => unreachable!(..)` の腕を書かない。来ない値は型で消す
//!   (文字列の op は `words!` の enum、深さや重みは enum、表は const で作って型にする)。
//! * `hand-bit-width`: `32 - x.leading_zeros()` を手で書かない: `x.bit_width()` (1.100)。
//! * `bin-dead-pub`: バイナリ (src/main.rs と tools/lint/src/main.rs) は
//!   `#![cfg_attr(not(test), warn(dead_code_pub_in_binary))]` (1.97) を持つ: crate の階層でしか
//!   効かないので Cargo.toml でなく main に書く。
//! * `rust-lints`: crate の Cargo.toml (fixture を除く) はどれも `raw_borrows_via_references`
//!   (1.99) を持つ。
//! * `hand-std`: 1.98..1.100 が名前をつけた形を手で書かない: `x.toggle()`、`s.trim_prefix(p)`、
//!   `o.map_or_default(f)`、`c.ok_or(e)`、`Vec::from_fn(v.len(), f)`。
//! * `publish-age`: .cargo/config.toml が `global-min-publish-age` (1.100) を持つ: 公開されて
//!   すぐの版 (乗っ取られた版はたいてい数日で消される) を lock の更新で拾わない。
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::code::{code_only, line_of};

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static WILD: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?:^|[\s,|{(])_\s*=>\s*\{?\s*(?:(?:::)?(?:core|std)::)?unreachable!"));
static BITS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\b(?:8|16|32|64|128)\s*-\s*[\w.()\[\]&*| ]*?\.leading_zeros\(\)"));

static TOGGLE: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?:^|[^\w.])([\w.]+(?:\[[^\]\n]+\])?)\s*=\s*!\s*([\w.]+(?:\[[^\]\n]+\])?)\s*;"));
static TRIM: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?:^|[^\w.])([\w.]+)\.strip_prefix\([^()]*\)\s*\.unwrap_or\(\s*([\w.]+)\s*\)"));
static MAP_OR_DEFAULT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\.map_or\(\s*(?:Default::default\(\)|String::new\(\)|Vec::new\(\))\s*,"));
static OK_OR: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bif [^{};\n]+ \{\s*Ok\(\(\)\)\s*\}\s*else\s*\{\s*Err\("));
static FROM_FN: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\(0\.\.[\w.()]*\.len\(\)\)\.map\(\|\w+\|"));

/// the byte of the `)` closing the `(` at `open` (None: unbalanced)
fn closing(code: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in code.get(open..)?.char_indices() {
        match c {
            '(' | '[' | '{' => depth = depth.saturating_add(1),
            ')' | ']' | '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open.saturating_add(i));
                }
            }
            _ => {}
        }
    }
    None
}

/// the hand-written shapes of one file's code: (byte, what to write)
fn hand_std(code: &str) -> Vec<(usize, &'static str)> {
    let mut out = Vec::new();
    let same = |c: &regex::Captures| c.get(1).map(|m| m.as_str()) == c.get(2).map(|m| m.as_str());
    if let Some(r) = TOGGLE.as_ref() {
        out.extend(r.captures_iter(code).filter(same).filter_map(|c| c.get(1)).map(|m| (m.start(), "`x = !x` is `x.toggle()` (Rust 1.100)")));
    }
    if let Some(r) = TRIM.as_ref() {
        out.extend(r.captures_iter(code).filter(same).filter_map(|c| c.get(1)).map(|m| (m.start(), "`s.strip_prefix(p).unwrap_or(s)` is `s.trim_prefix(p)` (Rust 1.100)")));
    }
    if let Some(r) = MAP_OR_DEFAULT.as_ref() {
        out.extend(r.find_iter(code).map(|m| (m.start(), "`.map_or(Default::default(), f)` is `.map_or_default(f)` (Rust 1.98)")));
    }
    // (`.map(f).unwrap_or_default()`: the map's parentheses balanced by hand, a regex cannot)
    for (at, _) in code.match_indices(".map(") {
        let Some(close) = closing(code, at.saturating_add(4)) else { continue };
        if code.get(close.saturating_add(1)..).is_some_and(|rest| rest.trim_start().starts_with(".unwrap_or_default()")) {
            out.push((at, "`.map(f).unwrap_or_default()` is `.map_or_default(f)` (Rust 1.98)"));
        }
    }
    if let Some(r) = OK_OR.as_ref() {
        out.extend(r.find_iter(code).map(|m| (m.start(), "`if c { Ok(()) } else { Err(e) }` is `c.ok_or(e)` / `c.ok_or_else(|| e)` (Rust 1.98)")));
    }
    if let Some(r) = FROM_FN.as_ref() {
        for m in r.find_iter(code) {
            // (the `(` of the `.map(` the match ends in)
            let Some(open) = code.get(..m.end()).and_then(|s| s.rfind(".map(")).map(|k| k.saturating_add(4)) else { continue };
            let Some(close) = closing(code, open) else { continue };
            if code.get(close.saturating_add(1)..).is_some_and(|rest| rest.trim_start().starts_with(".collect::<Vec<")) {
                out.push((m.start(), "`(0..v.len()).map(f).collect::<Vec<_>>()` is `Vec::from_fn(v.len(), f)` (Rust 1.100)"));
            }
        }
    }
    out
}

const BROKEN: &str = "tools/lint/src/modern.rs: a pattern does not compile";

/// the binaries' roots
pub const BINARIES: &[&str] = &["src/main.rs", "tools/lint/src/main.rs"];
/// the crates' manifests (the lint's own fixture crate is not one of kikizake's)
pub const MANIFESTS: &[&str] = &["Cargo.toml", "fuzz/Cargo.toml", "tools/lint/Cargo.toml"];
pub const CARGO_CONFIG: &str = ".cargo/config.toml";

/// the problems of one Rust source at `rel`
pub fn rust(rel: &str, text: &str) -> Vec<String> {
    let (Some(wild), Some(bits)) = (WILD.as_ref(), BITS.as_ref()) else { return vec![BROKEN.into()] };
    let code = code_only(text, false);
    let mut bad = Vec::new();
    for m in wild.find_iter(&code) {
        let at = m.start().saturating_add(m.as_str().find('_').unwrap_or(0));
        bad.push(format!("{rel}:{}: wildcard-unreachable: a value that cannot come is taken out by a type (an enum, a const-built table), not by `_ => unreachable!()`", line_of(text, at)));
    }
    for m in bits.find_iter(&code) {
        bad.push(format!("{rel}:{}: hand-bit-width: `{}` is `.bit_width()` (Rust 1.100)", line_of(text, m.start()), m.as_str()));
    }
    if [&*TOGGLE, &*TRIM, &*MAP_OR_DEFAULT, &*OK_OR, &*FROM_FN].iter().any(|r| r.is_none()) {
        return vec![BROKEN.into()];
    }
    for (at, what) in hand_std(&code) {
        bad.push(format!("{rel}:{}: hand-std: {what}", line_of(text, at)));
    }
    if BINARIES.contains(&rel) && !enables(&code) {
        bad.push(format!("{rel}:1: bin-dead-pub: a binary warns of pub items it does not use: `#![cfg_attr(not(test), warn(dead_code_pub_in_binary))]`"));
    }
    bad
}

/// an inner attribute that warns (or denies) dead_code_pub_in_binary
fn enables(code: &str) -> bool {
    code.split("#![").skip(1).any(|a| {
        let a = a.get(..a.find(']').unwrap_or(a.len())).unwrap_or_default();
        a.contains("dead_code_pub_in_binary") && (a.contains("warn(") || a.contains("deny(") || a.contains("forbid(")) && !a.contains("allow(") && !a.contains("expect(")
    })
}

/// a manifest's problems: the raw-borrow lint set (not to allow)
pub fn manifest(rel: &str, text: &str) -> Vec<String> {
    let on = text.lines().map(|l| l.split('#').next().unwrap_or_default()).any(|l| {
        let l = l.replace(' ', "");
        l.starts_with("raw_borrows_via_references=") && !l.contains("\"allow\"")
    });
    if on { Vec::new() } else { vec![format!("{rel}: rust-lints: [lints.rust] raw_borrows_via_references = \"warn\" (Rust 1.99) is missing")] }
}

/// .cargo/config.toml's problems
pub fn cargo_config(text: &str) -> Vec<String> {
    let set = text.lines().map(|l| l.split('#').next().unwrap_or_default().trim()).any(|l| l.starts_with("global-min-publish-age"));
    if set { Vec::new() } else { vec![format!("{CARGO_CONFIG}: publish-age: [registry] global-min-publish-age is missing")] }
}

/// every problem of the repository at `root`
pub fn check(root: &Path) -> Result<(), String> {
    let mut bad = Vec::new();
    let read = |rel: &str| std::fs::read_to_string(root.join(rel)).map_err(|e| format!("{rel}: {e}"));
    let mut stack = vec![root.join("src"), root.join("tools/lint/src"), root.join("fuzz")];
    while let Some(d) = stack.pop() {
        let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
        for e in entries {
            let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != "target" && n != "corpus" && n != "artifacts") {
                    stack.push(p);
                }
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            // (the lint's own tests spell what they look for)
            if rel.starts_with("tools/lint/src/") && rel.ends_with("_tests.rs") {
                continue;
            }
            bad.extend(rust(&rel, &read(&rel)?));
        }
    }
    for b in BINARIES {
        if !root.join(b).is_file() {
            bad.push(format!("{b}: listed in BINARIES but gone"));
        }
    }
    for m in MANIFESTS {
        bad.extend(manifest(m, &read(m)?));
    }
    bad.extend(cargo_config(&read(CARGO_CONFIG)?));
    bad.sort();
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "modern_tests.rs"]
mod tests;
