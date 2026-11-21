//! A Rust source's code with its comments (and string literals, unless asked to keep them)
//! blanked: what a text rule reads and what a site is known by. Newlines and every other
//! character's position stay, so a match's line is the source's line. Not a parser: enough for
//! raw strings, nested block comments, byte strings, char literals and lifetimes.

/// `text` with its comments and char literals blanked, and its string literals too unless
/// `strings`.
pub fn code_only(text: &str, strings: bool) -> String {
    let t: Vec<char> = text.chars().collect();
    let n = t.len();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let at = |k: usize| t.get(k).copied();
    let word_before = |k: usize| k.checked_sub(1).and_then(|p| t.get(p)).is_some_and(|c| c.is_alphanumeric() || *c == '_');
    let starts = |k: usize, s: &str| s.chars().enumerate().all(|(d, c)| k.checked_add(d).and_then(|x| t.get(x)) == Some(&c));
    while i < n {
        let c = at(i).unwrap_or(' ');
        let (j, keep) = if starts(i, "//") {
            (find(&t, i, &['\n']).unwrap_or(n), false)
        } else if starts(i, "/*") {
            (block_comment_end(&t, i), false)
        } else if let Some(end) = (c == 'b' || c == 'r').then(|| raw_string_end(&t, i)).flatten().filter(|_| !word_before(i)) {
            (end, strings)
        } else if c == '"' || (c == 'b' && at(i.saturating_add(1)) == Some('"') && !word_before(i)) {
            (string_end(&t, if c == 'b' { i.saturating_add(2) } else { i.saturating_add(1) }), strings)
        } else if let Some(end) = (c == '\'').then(|| char_lit_end(&t, i)).flatten() {
            (end, false)
        } else {
            // (a run of plain code at once)
            let mut j = i.saturating_add(1);
            while at(j).is_some_and(|x| !matches!(x, '/' | '"' | '\'' | 'b' | 'r')) {
                j = j.saturating_add(1);
            }
            (j, true)
        };
        let j = j.min(n).max(i.saturating_add(1));
        for &x in t.get(i..j).unwrap_or_default() {
            out.push(if keep || x == '\n' { x } else { ' ' });
        }
        i = j;
    }
    out
}

fn find(t: &[char], from: usize, what: &[char]) -> Option<usize> {
    t.get(from..)?.iter().position(|c| what.contains(c)).and_then(|p| p.checked_add(from))
}

fn block_comment_end(t: &[char], i: usize) -> usize {
    let (mut depth, mut j) = (1u32, i.saturating_add(2));
    while depth > 0 && j < t.len() {
        let two = (t.get(j).copied(), t.get(j.saturating_add(1)).copied());
        if two == (Some('/'), Some('*')) {
            depth = depth.saturating_add(1);
            j = j.saturating_add(2);
        } else if two == (Some('*'), Some('/')) {
            depth = depth.saturating_sub(1);
            j = j.saturating_add(2);
        } else {
            j = j.saturating_add(1);
        }
    }
    j
}

/// `b?r#*"`: the end of the raw string starting at `i` (past its closing quote and hashes), or
/// None when `i` starts none
fn raw_string_end(t: &[char], i: usize) -> Option<usize> {
    let mut j = i;
    if t.get(j) == Some(&'b') {
        j = j.saturating_add(1);
    }
    if t.get(j) != Some(&'r') {
        return None;
    }
    j = j.saturating_add(1);
    let mut hashes = 0usize;
    while t.get(j) == Some(&'#') {
        hashes = hashes.saturating_add(1);
        j = j.saturating_add(1);
    }
    if t.get(j) != Some(&'"') {
        return None;
    }
    j = j.saturating_add(1);
    while j < t.len() {
        if t.get(j) == Some(&'"') && (1..=hashes).all(|h| t.get(j.saturating_add(h)) == Some(&'#')) {
            return Some(j.saturating_add(1).saturating_add(hashes));
        }
        j = j.saturating_add(1);
    }
    Some(t.len())
}

/// the end of a string whose text starts at `j` (past its closing quote)
fn string_end(t: &[char], mut j: usize) -> usize {
    while let Some(&c) = t.get(j) {
        if c == '"' {
            return j.saturating_add(1);
        }
        j = j.saturating_add(if c == '\\' { 2 } else { 1 });
    }
    t.len()
}

/// `'x'`, `'\n'`, `'\u{..}'`: the end of the char literal at `i`, None for a lifetime
fn char_lit_end(t: &[char], i: usize) -> Option<usize> {
    let a = *t.get(i.checked_add(1)?)?;
    if a == '\\' {
        let mut j = i.checked_add(2)?;
        if t.get(j) == Some(&'u') {
            // (`\u{hex}`, then anything up to the quote, as `\\(?:[^u]|u\{[0-9a-fA-F]+\})[^']*'`)
            j = j.checked_add(1)?;
            if t.get(j) != Some(&'{') {
                return None;
            }
            j = j.checked_add(1)?;
            let hex_from = j;
            while t.get(j).is_some_and(char::is_ascii_hexdigit) {
                j = j.checked_add(1)?;
            }
            if j == hex_from || t.get(j) != Some(&'}') {
                return None;
            }
            j = j.checked_add(1)?;
        } else {
            t.get(j)?;
            j = j.checked_add(1)?;
        }
        find(t, j, &['\'']).and_then(|q| q.checked_add(1))
    } else if a != '\'' && a != '\n' && t.get(i.checked_add(2)?) == Some(&'\'') {
        i.checked_add(3)
    } else {
        None
    }
}

/// line `n` (1-based) of `code`, its runs of whitespace one space: what a site is known by
pub fn line_text(code: &str, n: usize) -> String {
    n.checked_sub(1).and_then(|k| code.split('\n').nth(k)).map_or_default(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// the 1-based line of byte offset `at` in `s`
pub fn line_of(s: &str, at: usize) -> usize {
    s.get(..at).map_or(0, |p| p.matches('\n').count()).saturating_add(1)
}

#[cfg(test)]
#[path = "code_tests.rs"]
mod tests;
