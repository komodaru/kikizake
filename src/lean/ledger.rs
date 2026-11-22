//! How far a Lean theorem (a `lean` law of spec/laws.txt) reaches the Rust models. The theorem
//! is proved of Lean definitions; `kikizake lean` holds a definition equal to the model's
//! function through its mirror, over every argument of a domain (`Every`) or on samples
//! (`Sampled`). So a law proves its clause of the specification, and of the implementation only
//! as far as the mirrors of the definitions its statement names go:
//!
//! - [`Reach::Every`]: every mirrored definition the statement names is mirrored over its whole
//!   domain: the theorem holds of the model's functions there.
//! - [`Reach::Sampled`]: one of them is mirrored on samples: the theorem holds of the Lean
//!   definition; the model agrees with it on the samples.
//! - [`Reach::SpecOnly`]: the statement names no mirrored definition (a lemma about a helper,
//!   or a definition no mirror runs): a fact of the specification alone.
//!
//! The ledger keeps the two apart (trace::table's `impl` column, spec/sampled.txt).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::{Domain, Mirror};

/// how far a theorem reaches the model's functions (ordered weakest first)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Reach {
    SpecOnly,
    Sampled,
    Every,
}

impl Reach {
    pub fn name(self) -> &'static str {
        match self {
            Reach::SpecOnly => "spec-only",
            Reach::Sampled => "sampled",
            Reach::Every => "every",
        }
    }
}

/// a Lean file's text with its comments (`-- ..`, nested `/- .. -/`) blanked; strings kept
/// (the mirrors' names are strings)
fn without_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut it = text.chars().peekable();
    let mut depth = 0usize;
    let mut string = false;
    while let Some(c) = it.next() {
        let next = it.peek().copied();
        if string {
            out.push(c);
            if c == '\\' {
                if let Some(e) = it.next() {
                    out.push(e);
                }
            } else if c == '"' {
                string = false;
            }
            continue;
        }
        if depth > 0 {
            match (c, next) {
                ('-', Some('/')) => {
                    it.next();
                    depth = depth.saturating_sub(1);
                }
                ('/', Some('-')) => {
                    it.next();
                    depth = depth.saturating_add(1);
                }
                ('\n', _) => out.push('\n'),
                _ => {}
            }
            continue;
        }
        match (c, next) {
            ('/', Some('-')) => {
                it.next();
                depth = 1;
            }
            ('-', Some('-')) => {
                while it.peek().is_some_and(|&e| e != '\n') {
                    it.next();
                }
            }
            ('"', _) => {
                string = true;
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

fn ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\'' || c == '.'
}

/// the identifiers of a piece of Lean code, each by its last dotted part (`Blend.mix` -> `mix`)
fn idents(code: &str) -> BTreeSet<String> {
    code.split(|c: char| !ident_char(c)).filter(|w| w.starts_with(|c: char| c.is_alphabetic() || c == '_')).filter_map(|w| w.rsplit('.').next()).filter(|w| !w.is_empty()).map(str::to_owned).collect()
}

/// Lean's words in a mirror's body that are no definition of the file
const NOT_DEFS: &[&str] = &["fun", "do", "pure", "none", "some", "nat", "int", "bool", "if", "then", "else", "let", "match", "with", "true", "false"];

/// the names a Lean file defines (`def`, `private def`, `abbrev`, `noncomputable def` at a
/// line's start)
fn defined(code: &str) -> BTreeSet<String> {
    code.lines().filter_map(|l| {
        let l = l.trim_prefix("private ");
        let l = l.trim_prefix("noncomputable ");
        l.strip_prefix("def ").or_else(|| l.strip_prefix("abbrev "))
    }).filter_map(|r| r.split(|c: char| !ident_char(c)).next()).filter_map(|n| n.rsplit('.').next()).filter(|n| !n.is_empty() && *n != "mirrors").map(str::to_owned).collect()
}

/// the mirrors a Lean file lists (`def mirrors : List Mirror := [ ⟨"name", fun ..⟩, .. ]`):
/// name -> the definitions its body calls (the file's own, and qualified ones of other files
/// by their last part; its bound variables are neither)
pub fn file_mirrors(text: &str) -> BTreeMap<String, BTreeSet<String>> {
    let code = without_comments(text);
    let own = defined(&code);
    let mut out = BTreeMap::new();
    let Some((_, list)) = code.split_once("def mirrors") else { return out };
    let list = list.split_once("\n]").map_or(list, |(l, _)| l);
    for part in list.split("⟨\"").skip(1) {
        let Some((name, body)) = part.split_once('"') else { continue };
        let qualified: BTreeSet<String> = body.split(|c: char| !ident_char(c)).filter(|w| w.contains('.') && w.starts_with(|c: char| c.is_alphabetic())).filter_map(|w| w.rsplit('.').next()).map(str::to_owned).collect();
        let ids: BTreeSet<String> = idents(body).into_iter().filter(|w| !NOT_DEFS.contains(&w.as_str()) && (own.contains(w) || qualified.contains(w))).collect();
        out.insert(name.to_owned(), ids);
    }
    out
}

/// a theorem's statement: what follows `theorem <name>` up to its `:=`
pub fn statement<'a>(code: &'a str, theorem: &str) -> Option<&'a str> {
    code.split("theorem ").skip(1).find_map(|rest| {
        let rest = rest.trim_start();
        let after = rest.strip_prefix(theorem)?;
        // (the whole name: `mix` is not `mix_exact`)
        if after.starts_with(ident_char) {
            return None;
        }
        Some(after.split_once(":=").map_or(after, |(st, _)| st))
    })
}

/// how far theorem `theorem` of the Lean file (`text`) reaches the models, the Rust mirrors
/// (`mirrors`) giving each Lean mirror's domain; None: the file has no such theorem
pub fn reach(text: &str, theorem: &str, mirrors: &[&Mirror]) -> Option<Reach> {
    let code = without_comments(text);
    let names = idents(statement(&code, theorem)?);
    let used: Vec<String> = file_mirrors(text).into_iter().filter(|(_, ids)| ids.iter().any(|i| names.contains(i))).map(|(n, _)| n).collect();
    // (a Lean mirror no Rust mirror runs ties nothing to the model)
    let each = used.iter().map(|n| match mirrors.iter().find(|m| m.name == n.as_str()).map(|m| m.domain) {
        Some(Domain::Every(_)) => Reach::Every,
        Some(Domain::Sampled(_)) => Reach::Sampled,
        None => Reach::SpecOnly,
    });
    Some(each.min().unwrap_or(Reach::SpecOnly))
}

/// every `lean` law's reach (law's file::test -> reach), the Lean files read from `root`
pub fn reaches(root: &Path) -> std::io::Result<BTreeMap<String, Reach>> {
    let mirrors = super::mirrors();
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for l in crate::trace::laws().into_iter().filter(|l| l.kind == "lean") {
        if !texts.contains_key(&l.file) {
            let t = std::fs::read_to_string(root.join(&l.file)).map_err(|e| std::io::Error::new(e.kind(), format!("{}: {e}", l.file)))?;
            texts.insert(l.file.clone(), t);
        }
        let text = texts.get(&l.file).map_or("", String::as_str);
        let r = reach(text, &l.test, &mirrors).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, format!("{}: no theorem {}", l.file, l.test)))?;
        out.insert(format!("{}::{}", l.file, l.test), r);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "namespace Kikizake.T\n/-- the exact value -/\ndef exact (a : Nat) := a\ndef mix (a : Nat) := a + 0\ndef wide (a : Nat) := a\n\n-- theorem commented_out (a : Nat) : wide a = a := rfl\ntheorem mix_exact (a : Nat) : mix a = exact a := by simp [mix, exact]\ntheorem wide_id (a : Nat) :\n    wide a = a /- mix in a comment -/ := rfl\ntheorem both (a : Nat) : mix a + Kikizake.T.wide a = 2 * a := by simp [mix, wide]; omega\nprivate theorem exact_id (a : Nat) : exact a = a := rfl\n\ndef mirrors : List Mirror := [\n  ⟨\"t.mix\", fun\n    | [a] => do pure [mix (← nat a)]\n    | _ => none⟩,\n  ⟨\"t.wide\", fun\n    | [a] => do pure [wide (← nat a)]\n    | _ => none⟩\n]\nend Kikizake.T\n";

    fn m(name: &'static str, domain: Domain) -> Mirror {
        Mirror { name, lean: "spec/lean/Kikizake/T.lean", domain, inputs: |_| {}, model: |_| None }
    }

    /// a theorem reaches the model as far as the weakest mirror of the definitions it names; one
    /// naming no mirrored definition is of the specification alone; comments name nothing
    #[test]
    fn reach_is_the_weakest_mirror_named() {
        let (every, sampled) = (m("t.mix", Domain::Every("0..9")), m("t.wide", Domain::Sampled("a few")));
        let ms = [&every, &sampled];
        assert_eq!(reach(FILE, "mix_exact", &ms), Some(Reach::Every));
        assert_eq!(reach(FILE, "wide_id", &ms), Some(Reach::Sampled));
        assert_eq!(reach(FILE, "both", &ms), Some(Reach::Sampled), "a dotted name is its last part");
        assert_eq!(reach(FILE, "exact_id", &ms), Some(Reach::SpecOnly));
        assert_eq!(reach(FILE, "commented_out", &ms), None);
        // (a Lean mirror without a Rust one reaches nothing)
        assert_eq!(reach(FILE, "mix_exact", &[&sampled]), Some(Reach::SpecOnly));
        let fm = file_mirrors(FILE);
        assert_eq!(fm.keys().map(String::as_str).collect::<Vec<_>>(), ["t.mix", "t.wide"]);
        assert!(fm["t.mix"].contains("mix") && !fm["t.mix"].contains("pure") && !fm["t.mix"].contains("wide"));
    }
}
