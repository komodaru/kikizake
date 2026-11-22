//! `lean-trust`: the Lean proofs (spec/lean/) rest on Lean's kernel alone. A proof left open
//! (`sorry`, `admit`), an assumed fact (`axiom`), a definition the kernel cannot see into
//! (`opaque`, `unsafe`, `@[implemented_by ..]`, `@[extern ..]`), a check handed to compiled code
//! (`native_decide`, `bv_decide`, `ofReduceBool`: the compiler trusted as the kernel) or the
//! kernel's check turned off (`debug.skipKernelTC`) is refused, in the code (comments do not
//! count). The lakefile keeps `warningAsError = true`, so a `sorry` cannot pass as a warning.
use std::path::Path;

/// (word, why)
pub const UNTRUSTED: &[(&str, &str)] = &[
    ("sorry", "a proof left open"),
    ("admit", "a proof left open"),
    ("axiom", "an assumed fact"),
    ("opaque", "a definition the kernel cannot unfold"),
    ("unsafe", "a definition the kernel does not check"),
    ("implemented_by", "the compiled code differs from what the kernel sees"),
    ("extern", "the compiled code differs from what the kernel sees"),
    ("native_decide", "the compiler trusted as the kernel"),
    ("bv_decide", "the compiler trusted as the kernel (its LRAT check runs compiled)"),
    ("ofReduceBool", "the compiler trusted as the kernel"),
    ("skipKernelTC", "the kernel's check turned off"),
];

/// a Lean file's code: comments (`-- ..`, nested `/- .. -/`) and strings blanked, the lines kept
pub fn lean_code(text: &str) -> String {
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    let mut depth = 0usize;
    let mut string = false;
    while let Some(c) = it.next() {
        let next = it.peek().copied();
        if depth > 0 {
            match (c, next) {
                ('-', Some('/')) => { it.next(); depth = depth.saturating_sub(1); out.push_str("  "); }
                ('/', Some('-')) => { it.next(); depth = depth.saturating_add(1); out.push_str("  "); }
                _ => out.push(blank(c)),
            }
            continue;
        }
        if string {
            if c == '\\' {
                out.push(' ');
                if let Some(e) = it.next() { out.push(blank(e)); }
                continue;
            }
            string = c != '"';
            out.push(blank(c));
            continue;
        }
        match (c, next) {
            ('/', Some('-')) => { it.next(); depth = 1; out.push_str("  "); }
            ('-', Some('-')) => {
                out.push(' ');
                while let Some(&e) = it.peek() {
                    if e == '\n' { break; }
                    out.push(' ');
                    it.next();
                }
            }
            ('"', _) => { string = true; out.push(' '); }
            _ => out.push(c),
        }
    }
    out
}

fn ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

/// the refusals in one Lean file (`rel`, its text)
pub fn lean(rel: &str, text: &str) -> Vec<String> {
    let code = lean_code(text);
    let mut bad = Vec::new();
    for (n, l) in (1usize..).zip(code.lines()) {
        for (word, why) in UNTRUSTED {
            for (at, _) in l.match_indices(word) {
                let before = l.get(..at).and_then(|b| b.chars().next_back());
                let after = l.get(at.saturating_add(word.len())..).and_then(|a| a.chars().next());
                // (a word of its own; `Foo.sorry` or `my_axiom` are other names, but a qualified
                // `Lean.ofReduceBool` is the same thing)
                let qualified = matches!(*word, "ofReduceBool" | "skipKernelTC" | "native_decide" | "bv_decide" | "implemented_by");
                let joined = before.is_some_and(|c| ident(c) || (c == '.' && !qualified));
                if !joined && !after.is_some_and(ident) {
                    bad.push(format!("{rel}:{n}: lean-trust: `{word}`: {why}"));
                }
            }
        }
    }
    bad
}

/// the lakefile keeps warnings errors
pub fn lakefile(rel: &str, text: &str) -> Vec<String> {
    if text.lines().any(|l| l.split('#').next().unwrap_or("").replace(' ', "") == "warningAsError=true") {
        Vec::new()
    } else {
        vec![format!("{rel}: lean-trust: [leanOptions] must keep warningAsError = true (a sorry is a warning)")]
    }
}

pub fn check(root: &Path) -> Result<(), String> {
    let base = root.join("spec/lean");
    let mut bad = Vec::new();
    let lake = base.join("lakefile.toml");
    let text = std::fs::read_to_string(&lake).map_err(|e| format!("spec/lean/lakefile.toml: {e}"))?;
    bad.extend(lakefile("spec/lean/lakefile.toml", &text));
    let mut stack = vec![base.clone()];
    let mut files = false;
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != ".lake") {
                    stack.push(p);
                }
                continue;
            }
            if p.extension().and_then(|x| x.to_str()) != Some("lean") {
                continue;
            }
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
            bad.extend(lean(&rel, &text));
            files = true;
        }
    }
    if !files {
        bad.push("spec/lean: no Lean files".into());
    }
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "formal_tests.rs"]
mod tests;
