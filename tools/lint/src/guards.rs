//! Checks that keep the 2026-11-20 review's mistakes from coming back (absolute: no baseline).
//!
//! * `model-uses-generators` (review 4.3): a model (src/model/) reads nothing of the generators
//!   (src/generators/): what both need lives in a neutral module (src/dxbc.rs, src/shaders.rs,
//!   src/resource_desc.rs), so the cases and the answers they are checked against do not share
//!   one implementation. The sites left are counted per file in `MODEL_USES_GENERATORS` and
//!   only go down.
//! * `io-error-as-empty` (review 2.2, 2.4): a file or directory that cannot be read is not an
//!   empty one: a statement that reads one (`fs::read_dir`, `fs::read_to_string`) must not turn
//!   the error into a default (`unwrap_or_default`, `unwrap_or`, `unwrap_or_else`, `.ok()`),
//!   unless a comment on it or the line before says `(optional`.
//! * `default-beside-new` (review 4.1): a type that derives Default and has a `fn new()` with no
//!   arguments makes `new` the default itself (`Self::default()`), so the two cannot disagree.
//! * `dib-without-flush` (review 3.4): an executor function that touches a DIB section's memory
//!   (`->bits`, `row(`) calls GdiFlush first (the function that creates the section excepted).
//! * `wait-unchecked` (review 3.5): a D3D12 queue signal, fence event or wait whose result is
//!   thrown away (the call is the whole statement).
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::code::{code_only, line_of};

/// (file, sites): src/model/ files still reading the generators
pub const MODEL_USES_GENERATORS: &[(&str, usize)] = &[("src/model/wicimage.rs", 3)];

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static GENERATORS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\b(crate|super)::generators\b|\buse\s+crate::generators"));
static FS_READ: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bfs::(read_dir|read_to_string)\s*\("));
static SWALLOW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\.(unwrap_or_default|unwrap_or|unwrap_or_else)\s*\(|\.ok\(\)|\.into_iter\(\)\s*\.flatten\(\)"));
/// (an `unwrap_or_else` that stops the program reports the error)
static STOPS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bexit\s*\(|\bpanic!|\breturn\b"));
static DERIVE_DEFAULT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*pub(?:\([^)]*\))?\s+struct\s+(\w+)|#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*struct\s+(\w+)"));
static NEW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"fn\s+new\s*\(\s*\)\s*->\s*(\w+)\s*\{"));
static IMPL: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"impl\s+(\w+)\s*\{"));
static C_FN: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^[A-Za-z_][\w \*]*\b(\w+)\s*\([^;{]*\)\s*\{"));
static DIB: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"->bits\b|\brow\s*\("));
/// (what only tests or sets the pointer: `!s->bits`, `s->bits ?`, `s->bits = ...`)
static DIB_POINTER: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"!\s*[\w>.-]*->bits\b|->bits\s*\?|->bits\s*=[^=]"));
static WAIT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^\s*(ID3D12CommandQueue_Signal|ID3D12Fence_SetEventOnCompletion|WaitForSingleObject)\s*\("));

/// every pattern, compiled (None: one does not, an error of this file)
struct Pats {
    generators: &'static Regex,
    fs_read: &'static Regex,
    swallow: &'static Regex,
    stops: &'static Regex,
    derive_default: &'static Regex,
    new: &'static Regex,
    impls: &'static Regex,
    c_fn: &'static Regex,
    dib: &'static Regex,
    dib_pointer: &'static Regex,
    wait: &'static Regex,
}

fn pats() -> Option<Pats> {
    Some(Pats { generators: GENERATORS.as_ref()?, fs_read: FS_READ.as_ref()?, swallow: SWALLOW.as_ref()?, stops: STOPS.as_ref()?, derive_default: DERIVE_DEFAULT.as_ref()?, new: NEW.as_ref()?, impls: IMPL.as_ref()?, c_fn: C_FN.as_ref()?, dib: DIB.as_ref()?, dib_pointer: DIB_POINTER.as_ref()?, wait: WAIT.as_ref()? })
}

const BROKEN: &str = "tools/lint/src/guards.rs: a pattern does not compile";

/// the statements of `code` (split at `;`, `{` and `}`) with their byte offsets
fn statements(code: &str) -> Vec<(usize, &str)> {
    let mut v = Vec::new();
    let mut from = 0;
    for (i, c) in code.char_indices() {
        if matches!(c, ';' | '{' | '}') {
            v.push((from, code.get(from..i).unwrap_or_default()));
            from = i.saturating_add(1);
        }
    }
    v.push((from, code.get(from..).unwrap_or_default()));
    v
}

/// the statement's only fallback is an `unwrap_or_else` whose closure stops the program
fn reports(p: &Pats, code: &str, at: usize, st: &str) -> bool {
    let Some(k) = st.find("unwrap_or_else(").filter(|_| p.swallow.find_iter(st).count() == 1) else { return false };
    let from = at.saturating_add(k).saturating_add("unwrap_or_else".len());
    let mut depth = 0usize;
    for (i, c) in code.get(from..).unwrap_or_default().char_indices() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return code.get(from..from.saturating_add(i)).is_some_and(|args| p.stops.is_match(args));
                }
            }
            _ => {}
        }
    }
    false
}

/// the problems of one Rust source at `rel`
pub fn rust(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = code_only(text, false);
    let mut bad = Vec::new();
    if rel.starts_with("src/model/") {
        let n = p.generators.find_iter(&code).count();
        let allowed = MODEL_USES_GENERATORS.iter().find(|(f, _)| *f == rel).map_or(0, |(_, n)| *n);
        if n > allowed {
            bad.push(format!("{rel}: model-uses-generators: {n} sites, at most {allowed}: move what both need to a neutral module"));
        }
    }
    let lines: Vec<&str> = text.split('\n').collect();
    for (at, st) in statements(&code) {
        if p.fs_read.is_match(st) && p.swallow.is_match(st) && !reports(&p, &code, at, st) {
            let (first, last) = (line_of(text, at), line_of(text, at.saturating_add(st.len())));
            let said = (first.saturating_sub(1)..=last).any(|l| l.checked_sub(1).and_then(|k| lines.get(k)).is_some_and(|t| t.contains("(optional")));
            if !said {
                bad.push(format!("{rel}:{}: io-error-as-empty: a read that fails is not an empty file; return the error (or say \"(optional\" why it may be missing)", line_of(text, at.saturating_add(st.len().saturating_sub(st.trim_start().len())))));
            }
        }
    }
    let defaults: Vec<&str> = p.derive_default.captures_iter(&code).filter_map(|c| c.get(1).or(c.get(2)).map(|m| m.as_str())).collect();
    for m in p.new.captures_iter(&code) {
        let (Some(all), Some(ret)) = (m.get(0), m.get(1)) else { continue };
        let ty = if ret.as_str() == "Self" {
            p.impls.captures_iter(code.get(..all.start()).unwrap_or_default()).last().and_then(|c| c.get(1)).map_or("", |t| t.as_str())
        } else {
            ret.as_str()
        };
        if !defaults.contains(&ty) {
            continue;
        }
        let body = code.get(all.end()..).unwrap_or_default();
        let body = body.get(..body.find('}').unwrap_or(body.len())).unwrap_or_default().trim();
        if body != "Self::default()" && body != format!("{ty}::default()") {
            bad.push(format!("{rel}:{}: default-beside-new: {ty} derives Default, so new() must be Self::default() (or drop the derive)", line_of(text, all.start())));
        }
    }
    bad
}

/// C with its comments blanked (string literals kept), positions kept
fn c_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let next = b.get(i.saturating_add(1)).copied();
        if c == '/' && next == Some('*') {
            let mut j = i.saturating_add(2);
            while j < b.len() && !(b.get(j) == Some(&'*') && b.get(j.saturating_add(1)) == Some(&'/')) {
                j = j.saturating_add(1);
            }
            let end = j.saturating_add(2).min(b.len());
            for &x in b.get(i..end).unwrap_or_default() {
                out.push(if x == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else if c == '/' && next == Some('/') {
            while b.get(i).is_some_and(|&x| x != '\n') {
                out.push(' ');
                i = i.saturating_add(1);
            }
        } else {
            out.push(c);
            i = i.saturating_add(1);
        }
    }
    out
}

/// the problems of one executor source at `rel`
pub fn c(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = c_code(text);
    let mut bad = Vec::new();
    for m in p.wait.find_iter(&code) {
        bad.push(format!("{rel}:{}: wait-unchecked: the result of {} decides whether the GPU finished; check it", line_of(&code, m.start()), m.as_str().trim().trim_end_matches('(').trim()));
    }
    if Path::new(rel).file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with("gdi")) {
        let starts: Vec<(usize, &str)> = p.c_fn.captures_iter(&code).filter_map(|c| Some((c.get(0)?.start(), c.get(1)?.as_str()))).collect();
        for (k, &(at, name)) in starts.iter().enumerate() {
            let end = starts.get(k.saturating_add(1)).map_or(code.len(), |n| n.0);
            let body = code.get(at..end).unwrap_or_default();
            // (the body past the signature: `row` itself, and a function that makes the section)
            let open = body.find('{').map_or(0, |o| o.saturating_add(1));
            let inner = body.get(open..).unwrap_or_default();
            if name == "row" || inner.contains("CreateDIBSection") {
                continue;
            }
            let inner = p.dib_pointer.replace_all(inner, |c: &regex::Captures| " ".repeat(c.get(0).map_or(0, |m| m.as_str().len())));
            if let Some(first) = p.dib.find(&inner) {
                let flushed = inner.get(..first.start()).is_some_and(|p| p.contains("GdiFlush()"));
                if !flushed {
                    bad.push(format!("{rel}:{}: dib-without-flush: {name} touches the DIB section's memory before GdiFlush()", line_of(&code, at.saturating_add(open).saturating_add(first.start()))));
                }
            }
        }
    }
    bad
}

/// every problem in the repository at `root`
pub fn check(root: &Path) -> Result<(), String> {
    let mut bad = Vec::new();
    let mut seen = Vec::new();
    for (dir, ext) in [("src", "rs"), ("runner", "c")] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
            for e in entries {
                let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some(ext) {
                    continue;
                }
                let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
                bad.extend(if ext == "rs" { rust(&rel, &text) } else { c(&rel, &text) });
                seen.push(rel);
            }
        }
    }
    for (f, _) in MODEL_USES_GENERATORS {
        if !seen.iter().any(|s| s == f) {
            bad.push(format!("{f}: listed in MODEL_USES_GENERATORS but gone: take its line out"));
        }
    }
    if seen.is_empty() {
        bad.push("no sources found".into());
    }
    bad.sort();
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "guards_tests.rs"]
mod tests;
