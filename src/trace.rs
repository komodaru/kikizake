//! Traceability: for every clause (spec/), the evidence that the suite checks it.
//!
//! * cases: the cases that reach it (the model calls `spec::hit`), per executor, each with what
//!   Windows answered: the model's lines equal Windows' (`same`), differ, the model has none
//!   (`unmodeled`), or Windows has not answered that case yet (`unanswered`);
//! * laws: the properties (tests/, listed in spec/laws.txt) written from the clause's text that
//!   hold of the model whatever Windows answers, each `exhaustive` (every input of its domain:
//!   a proof for that domain) or `pbt` (property-based, random inputs);
//! * documentation: the pages its source names (spec/learn.txt: page -> URL; `kikizake learn`
//!   fetches them and checks the quoted words and the identifiers the source names are there).
//!
//! `cargo test` (tests/spec.rs) holds every clause to the evidence its status claims;
//! `kikizake trace` prints the table and the gaps per API.

use std::collections::BTreeMap;
use std::path::Path;

use crate::spec::Clause;

#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub same: usize,
    pub differ: usize,
    pub unmodeled: usize,
    pub unanswered: usize,
    /// "suite-arch:case", the first few of each kind
    pub cases: Vec<String>,
    pub laws: Vec<Law>,
    /// for the unmodeled cases: the open clause each stopped at, how many
    pub blocked: BTreeMap<String, usize>,
}

impl Evidence {
    pub fn reached(&self) -> usize {
        self.same + self.differ + self.unmodeled + self.unanswered
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Law {
    /// tests/<file>.rs
    pub file: String,
    pub test: String,
    pub kind: String,
    pub clauses: Vec<String>,
}

pub const LAW_KINDS: &[&str] = &["exhaustive", "pbt", "kani", "tla"];

pub const LAWS: &str = include_str!("../spec/laws.txt");
pub const LEARN: &str = include_str!("../spec/learn.txt");

/// spec/laws.txt: `<file>::<test> <kind> <clause> ...`
pub fn laws() -> Vec<Law> {
    let mut v = Vec::new();
    for l in LAWS.lines() {
        if l.starts_with('#') || l.trim().is_empty() {
            continue;
        }
        let t: Vec<&str> = l.split_whitespace().collect();
        let (file, test) = t[0].split_once("::").unwrap_or((t[0], ""));
        v.push(Law { file: file.into(), test: test.into(), kind: t.get(1).unwrap_or(&"").to_string(), clauses: t.iter().skip(2).map(|s| s.to_string()).collect() });
    }
    v
}

/// spec/learn.txt: page name -> URL
pub fn learn() -> BTreeMap<String, String> {
    LEARN.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).filter_map(|l| l.split_once(" | ")).map(|(p, u)| (p.trim().to_string(), u.trim().to_string())).collect()
}

/// the page names a clause's source cites: comma separated, notes in parentheses dropped,
/// `(none)` and the like are none
pub fn pages(source: &str) -> Vec<String> {
    let mut s = String::new();
    let mut depth = 0;
    for ch in source.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ if depth == 0 => s.push(ch),
            _ => {}
        }
    }
    let mut v: Vec<String> = Vec::new();
    for p in s.split([',', ';']) {
        // ("Page: section", "X::Method and the others")
        let p = p.split(": ").next().unwrap().trim().trim_end_matches(':').trim_end_matches(" and the others").trim();
        // (measured clauses name goldens and cases: no pages)
        if p.is_empty() || p.starts_with("runner/") || p == "SPEC.md" || p.starts_with("golden") || !p.starts_with(|c: char| c.is_ascii_uppercase()) {
            continue;
        }
        // ("IFoo::GetA, GetB": GetB is IFoo::GetB)
        let iface = v.last().and_then(|l| l.split_once("::")).map(|(i, _)| i.to_string());
        match iface {
            Some(i) if !p.contains("::") && !p.contains(' ') && !p.contains('_') && p.contains(|c: char| c.is_ascii_lowercase()) && !(p.starts_with('I') && p[1..].starts_with(|c: char| c.is_ascii_uppercase())) && p.starts_with(|c: char| c.is_ascii_uppercase()) => v.push(format!("{i}::{p}")),
            _ => v.push(p.to_string()),
        }
    }
    v
}

/// the notes in parentheses of a source (what of the page it means)
pub fn notes(source: &str) -> Vec<String> {
    let mut v = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for ch in source.chars() {
        match ch {
            '(' => {
                depth += 1;
                if depth > 1 {
                    cur.push(ch)
                }
            }
            ')' => {
                depth -= 1;
                if depth == 0 {
                    v.push(std::mem::take(&mut cur));
                } else {
                    cur.push(ch)
                }
            }
            _ if depth > 0 => cur.push(ch),
            _ => {}
        }
    }
    v
}

/// the words of a doc clause that must be on its pages: what it quotes ("..."), and the constant
/// names (all capitals, with an underscore) its source's notes name
pub fn checkable(c: &Clause) -> Vec<String> {
    let mut v = Vec::new();
    // (only a doc clause says the page says it)
    if c.status != "doc" {
        return v;
    }
    for (i, q) in c.text.split('"').enumerate() {
        if i % 2 == 1 && q.len() > 3 {
            v.push(q.trim_end_matches(['.', ',']).trim_start_matches("...").trim().trim_end_matches("...").trim().to_string());
        }
    }
    for n in notes(&c.source) {
        for w in n.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_')) {
            if w.contains('_') && w.len() > 4 && w.chars().all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_') {
                v.push(w.to_string());
            }
        }
    }
    v
}

/// a source that is no documentation at all: a doc clause must not have one
pub fn undocumented(source: &str) -> bool {
    let s = source.trim();
    s.is_empty() || s.starts_with("(none") || s.starts_with("(measured")
}

/// a source that is the executor's own contract (SPEC.md, runner/): not Windows' documentation,
/// so Windows' answers must agree with it
pub fn executor(source: &str) -> bool {
    let s = source.trim();
    s.starts_with("(the executor") || s.starts_with("runner/") || s.starts_with("SPEC.md")
}

fn golden(dir: &Path, suite: &str, arch: u32) -> Option<BTreeMap<String, Vec<String>>> {
    let t = std::fs::read_to_string(dir.join(format!("{suite}-{arch}/stdout.txt"))).ok()?;
    Some(crate::case::parse_output(&t).cases.into_iter().collect())
}

/// every suite's cases, each executor, through the models and against Windows' answers in `dir`
pub fn trace(dir: &Path) -> BTreeMap<String, Evidence> {
    let mut m: BTreeMap<String, Evidence> = BTreeMap::new();
    for s in crate::gen::SUITES {
        let text = crate::gen::suite(s).unwrap();
        let cases = crate::case::parse_cases(&text);
        for arch in [32, 64] {
            if arch == 64 && text.contains("\n# x86 only") {
                continue;
            }
            crate::model::set_arch(arch);
            let g = golden(dir, s, arch);
            for c in &cases {
                crate::spec::take();
                let r = crate::model::run_case(c);
                let hits = crate::spec::take();
                let want = g.as_ref().and_then(|g| g.get(&c.name));
                let kind = match (&r, want) {
                    (_, None) => 3,
                    (Err(_), _) => 2,
                    (Ok(l), Some(w)) if l == w => 0,
                    _ => 1,
                };
                for id in hits {
                    let e = m.entry(id.to_string()).or_default();
                    match kind {
                        0 => e.same += 1,
                        1 => e.differ += 1,
                        2 => {
                            e.unmodeled += 1;
                            if let Err(crate::model::Fail::Unmodeled(why)) = &r {
                                if why.as_str() != id {
                                    *e.blocked.entry(why.to_string()).or_default() += 1;
                                }
                            }
                        }
                        _ => e.unanswered += 1,
                    }
                    if e.cases.len() < 3 {
                        e.cases.push(format!("{s}-{arch}:{}", c.name));
                    }
                }
            }
        }
    }
    crate::model::set_arch(32);
    for l in laws() {
        for id in &l.clauses {
            m.entry(id.clone()).or_default().laws.push(l.clone());
        }
    }
    m
}

fn blocked(e: &Evidence) -> String {
    if e.blocked.is_empty() {
        return String::new();
    }
    let b: Vec<String> = e.blocked.iter().map(|(k, n)| format!("{k} {n}")).collect();
    format!(" (its cases stop at {})", b.join(", "))
}

/// what a clause lacks for its status (empty: nothing)
pub fn gaps(c: &Clause, e: &Evidence, learn: &BTreeMap<String, String>) -> Vec<String> {
    let mut v = Vec::new();
    let st = c.status.as_str();
    if st == "excluded" {
        return v;
    }
    if e.reached() == 0 {
        v.push("no case reaches it".into());
    }
    // (a case differing from Windows fails tests/windows.rs; a clause whose cases Windows has
    // not answered yet waits for the windows workflow)
    let pending = e.unanswered > 0;
    match st {
        "doc" => {
            if undocumented(&c.source) {
                v.push(format!("doc, but its source {:?} is no documentation", c.source));
            }
            if executor(&c.source) {
                if e.same == 0 && !pending {
                    v.push(format!("the executor's own rule, but no case reaching it agrees with a Windows answer{}", blocked(e)));
                }
                return v;
            }
            for p in pages(&c.source) {
                if !learn.contains_key(&p) {
                    v.push(format!("page {p:?} not in spec/learn.txt"));
                }
            }
            if e.laws.is_empty() && e.same == 0 && !pending {
                v.push(format!("no law and no case Windows agrees with{}", blocked(e)));
            }
        }
        "measured" => {
            if e.same == 0 && !pending {
                v.push(format!("measured, but no case reaching it agrees with a Windows answer{}", blocked(e)));
            }
        }
        "assumed" => {
            if e.laws.is_empty() && e.same == 0 && !pending {
                v.push(format!("no law and no case Windows agrees with{}", blocked(e)));
            }
        }
        _ => {}
    }
    v
}

/// one line per clause: id, status, cases same/differ/unmodeled/unanswered, laws, gaps
pub fn table(dir: &Path) -> String {
    let t = trace(dir);
    let learn = learn();
    let mut s = String::from("# clause status same differ unmodeled unanswered laws(exhaustive/pbt) | gaps\n");
    let mut per: BTreeMap<&str, [usize; 8]> = BTreeMap::new();
    let none = Evidence::default();
    for c in crate::spec::clauses() {
        let e = t.get(&c.id).unwrap_or(&none);
        let ex = e.laws.iter().filter(|l| l.kind == "exhaustive").count();
        let g = gaps(&c, e, &learn);
        s += &format!("{:28} {:9} {:6} {:3} {:5} {:5} {}/{} | {}\n", c.id, c.status, e.same, e.differ, e.unmodeled, e.unanswered, ex, e.laws.len() - ex, g.join("; "));
        let p = per.entry(c.file).or_default();
        p[0] += 1;
        p[match c.status.as_str() {
            "doc" => 1,
            "measured" => 2,
            "assumed" => 3,
            "open" => 4,
            _ => 5,
        }] += 1;
        if !e.laws.is_empty() {
            p[6] += 1;
        }
        if !g.is_empty() {
            p[7] += 1;
        }
    }
    s += "# per API: clauses doc measured assumed open excluded with-a-law with-gaps\n";
    for (f, p) in per {
        s += &format!("# {f:10} {:4} {:4} {:4} {:4} {:4} {:4} {:4} {:4}\n", p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
    }
    s
}
