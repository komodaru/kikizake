//! `escape-comment` (user rule 2026-11-20, as masu-lint's): no comment may write a shortcut down
//! instead of the behavior Windows has: no to-do markers, "for now", simplifications, "not
//! implemented" / "not modelled", stubs that answer anything, approximations, best effort (in
//! English or Japanese), and in Rust outside tests no log string saying so. "not supported",
//! approximations and ignored flags pass only when the same comment gives Windows' evidence
//! (MEASURED, a golden, winref, the spec). The sites there already are listed in
//! tools/lint/escape-known.tsv (file, the comment, what to do): the list only goes down.
use std::sync::LazyLock;

use regex::Regex;

use std::collections::BTreeMap;
use std::path::Path;

use crate::code::code_only;

/// words that are an escape whatever else the comment says
static ALWAYS: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\b(?:TO", r"DO|FIX", r"ME|XX", r"X|HA", r"CK)\b",
        r"|(?i:\bfor\s+now\b|\bfor\s+the\s+moment\b|\bin\s+the\s+meantime\b)",
        r"|\bsimplif(?:y|ied|ies|ication|ications)\b|\bsimpler\s+(?:version|form|model)\b",
        r"|(?i:\bnot\s+(?:yet\s+)?(?:implemented|ported|modeled|modelled)\b|\bunimplemented\b|\bun-?implemented\b)",
        r"|(?i:\bbest[\s-]+effort\b|\bgood\s+enough\b|\bclose\s+enough\b|\bstubbed\b|\blogged\s+stub\b|\bstub\s+(?:returning|that\s+(?:logs|returns|fails|answers)))",
        r"|(?i:\bplaceholder\s+(?:value|answer|implementation|result)s?\b|\bhack(?:y|ish)?\b|\bworkaround\b|\bfake\s+(?:success|answer|result))",
        r"|\bonly\b[^.;:]{0,60}\bso\s+far\b",
        r"|とりあえず|簡略|未実装|未対応|暫定|ひとまず|手抜き|近似|仮実装|後で実装|あとで実装|ダミー実装",
    ))
    .ok()
});

/// words that are an escape unless the same comment gives Windows' evidence for the answer
static UNLESS_EVIDENCE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(concat!(
        r"\bnot\s+(?:supported|handled)\b|\bunsupported\b|\bunhandled\s+(?:case|form|kind)s?\b",
        r"|(?i:\bapproximat(?:e|ed|es|ion|ions|ely)\b)",
        r"|\b(?:flags?|offsets?|attributes?|arguments?|parameters?|options?|the\s+rest)\s+(?:are\s+|is\s+)?ignored\b",
        r"|\b(?:taken|accepted),?\s+(?:and\s+)?(?:ignored|no\s+effect)\b|\baccepted,?\s+(?:and\s+)?(?:does\s+)?nothing\b",
    ))
    .ok()
});

/// what makes "not supported" Windows' answer and not a gap
static EVIDENCE: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(r"Windows|winref|golden|MEASURED|[Mm]easured|Wine's test|\bas on\b|\bthe spec\b|functional spec|documented|MSDN|learn\.microsoft|\bx86\b|\bx87\b|\bfxc\b|\bCPUs?\b|vendor's|the hardware").ok()
});

const SLASH: [&str; 11] = [".rs", ".c", ".h", ".cpp", ".js", ".mjs", ".ts", ".tsx", ".wgsl", ".hlsl", ".fx"];
const HASH: [&str; 5] = [".py", ".sh", ".yml", ".yaml", ".toml"];

/// the comment text of each line of `text` (empty where there is none), by the file's kind
pub fn comments(path: &str, text: &str) -> Vec<String> {
    if SLASH.iter().any(|e| path.ends_with(e)) {
        let code = code_only(text, true);
        // (what code_only blanked and was not blank: comments and char literals)
        text.split('\n').zip(code.split('\n')).map(|(t, c)| t.chars().zip(c.chars().chain(std::iter::repeat(' '))).map(|(a, b)| if a != b { a } else { ' ' }).collect::<String>().trim().to_owned()).collect()
    } else if HASH.iter().any(|e| path.ends_with(e)) {
        text.split('\n').map(|l| hash_comment(l).unwrap_or_default().trim().to_owned()).collect()
    } else {
        vec![]
    }
}

/// a `#` comment outside quotes (not a shebang's `#!`, not a `${#x}` or `$#`)
fn hash_comment(l: &str) -> Option<&str> {
    let (mut q, mut prev) = (None::<char>, ' ');
    for (i, ch) in l.char_indices() {
        match q {
            Some(o) if ch == o && prev != '\\' => q = None,
            Some(_) => {}
            None if ch == '"' || ch == '\'' => q = Some(ch),
            None if ch == '#' && (prev.is_whitespace() || i == 0) && !l.get(i..).is_some_and(|r| r.starts_with("#!")) => return l.get(i..),
            None => {}
        }
        prev = ch;
    }
    None
}

/// the string literals of each line of a Rust file (empty where there is none)
fn rust_strings(text: &str) -> Vec<String> {
    let with = code_only(text, true);
    let without = code_only(text, false);
    with.split('\n').zip(without.split('\n')).map(|(a, b)| a.chars().zip(b.chars().chain(std::iter::repeat(' '))).map(|(x, y)| if x != y { x } else { ' ' }).collect::<String>().trim().to_owned()).collect()
}

fn hit(re: &LazyLock<Option<Regex>>, s: &str) -> bool {
    re.as_ref().is_some_and(|r| r.is_match(s))
}

/// whether comment `s` (one line, with the comment block it is in as `block`) is an escape
pub fn is_escape(s: &str, block: &str) -> bool {
    hit(&ALWAYS, s) || (hit(&UNLESS_EVIDENCE, s) && !hit(&EVIDENCE, block))
}


pub const KNOWN: &str = "tools/lint/escape-known.tsv";

/// the vocabulary itself and its tests
fn exempt(p: &str) -> bool {
    p.starts_with("tools/lint/") || p.starts_with("tools/lint-fixtures/") || p.starts_with("vendor/") || p.contains("/testdata/") || p.starts_with("golden/")
}

/// the escapes of one file: (line, the comment or string)
pub fn escapes(path: &str, text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let lines = comments(path, text);
    let mut k = 0;
    while k < lines.len() {
        let mut e = k;
        while lines.get(e).is_some_and(|l| !l.is_empty()) {
            e = e.saturating_add(1);
        }
        let block = lines.get(k..e).map(|b| b.join(" ")).unwrap_or_default();
        for (i, l) in lines.iter().enumerate().take(e).skip(k) {
            if is_escape(l, &block) {
                out.push((i.saturating_add(1), l.clone()));
            }
        }
        k = e.saturating_add(1);
    }
    let test = path.starts_with("tests/") || path.contains("/tests/") || path.ends_with("_tests.rs") || path.ends_with("tests.rs");
    if path.ends_with(".rs") && !test {
        for (i, l) in rust_strings(text).iter().enumerate() {
            let mut why = l.clone();
            for c in lines.iter().take(i.saturating_add(1)).skip(i.saturating_sub(12)) {
                why.push_str(c);
            }
            // (a string: only the words that are an escape whatever the evidence; "unsupported" is
            // also the name shirusu's call files give a value it did not capture)
            if !l.is_empty() && !l.starts_with("r\"") && !l.starts_with("r#") && hit(&ALWAYS, l) && is_escape(l, &why) {
                out.push((i.saturating_add(1), l.clone()));
            }
        }
    }
    out
}

fn key(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// the known sites: (file, comment) -> how many
pub fn known(text: &str) -> BTreeMap<(String, String), usize> {
    let mut m: BTreeMap<(String, String), usize> = BTreeMap::new();
    for l in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let mut c = l.splitn(3, '\t');
        if let (Some(f), Some(t)) = (c.next(), c.next()) {
            let e = m.entry((f.to_owned(), key(t))).or_default();
            *e = e.saturating_add(1);
        }
    }
    m
}

/// the escapes beyond the known sites, of `files` (path, text)
pub fn fresh(files: &[(String, String)], known_text: &str) -> Vec<String> {
    let mut left = known(known_text);
    let mut bad = Vec::new();
    for (p, t) in files {
        if exempt(p) {
            continue;
        }
        for (line, what) in escapes(p, t) {
            match left.get_mut(&(p.clone(), key(&what))) {
                Some(n) if *n > 0 => *n = n.saturating_sub(1),
                _ => bad.push(format!("{p}:{line}: escape-comment: {what}\n    (implement what Windows does instead of writing the shortcut down; \"not supported\" / an approximation / ignored flags only with Windows' evidence in the same comment: MEASURED, a golden, winref, the spec; tools/lint/src/escape.rs)")),
            }
        }
    }
    bad
}

pub fn check(root: &Path) -> Result<(), String> {
    let out = std::process::Command::new("git").args(["ls-files", "-z"]).current_dir(root).output().map_err(|e| format!("git ls-files: {e}"))?;
    let mut files = Vec::new();
    for p in String::from_utf8_lossy(&out.stdout).split('\0').filter(|s| !s.is_empty()) {
        if comments(p, "").is_empty() && !p.ends_with(".rs") {
            continue;
        }
        let text = std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"))?;
        files.push((p.to_owned(), String::from_utf8_lossy(&text).into_owned()));
    }
    let known_text = std::fs::read_to_string(root.join(KNOWN)).map_err(|e| format!("{KNOWN}: {e}"))?;
    let bad = fresh(&files, &known_text);
    if bad.is_empty() {
        return Ok(());
    }
    Err(bad.join("\n"))
}

#[cfg(test)]
#[path = "escape_tests.rs"]
mod tests;
