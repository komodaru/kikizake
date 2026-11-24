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

pub const LAW_KINDS: &[&str] = &["exhaustive", "pbt", "kani", "tla", "lean"];

/// the kinds that prove a clause for its whole stated domain, not on samples: an exhaustive
/// law, a Kani harness, a TLA+ spec (to its MaxOps), a Lean theorem (spec/lean, its definitions
/// held equal to the model's functions by `kikizake lean`)
pub const FORMAL_KINDS: &[&str] = &["exhaustive", "kani", "tla", "lean"];

/// the clause files whose every settled clause (doc, measured) has a formal law
/// (FORMAL_KINDS) or is listed in spec/unproven.txt, which only shrinks
pub const PROVEN_FILES: &[&str] = &{
    let mut names = [""; crate::spec::FILES.len()];
    let mut files = crate::spec::FILES;
    let mut slots = names.as_mut_slice();
    while let ([file, rest @ ..], [slot, tail @ ..]) = (files, slots) {
        *slot = file.0;
        files = rest;
        slots = tail;
    }
    names
};

/// How far a clause's formal laws reach the implementation (the models' Rust functions), kept
/// apart from whether the clause is proved of the specification: an exhaustive law, a Kani
/// harness and a TLA+ replay run the model's functions themselves (over their domain: `Every`);
/// a Lean theorem reaches them through its definitions' mirrors (lean::ledger). The best of a
/// clause's formal laws; None: no formal law.
pub fn implementation_reach(laws: &[Law], lean: &BTreeMap<String, crate::lean::ledger::Reach>) -> Option<crate::lean::ledger::Reach> {
    use crate::lean::ledger::Reach;
    laws.iter().filter(|l| FORMAL_KINDS.contains(&l.kind.as_str())).map(|l| if l.kind == "lean" { lean.get(&format!("{}::{}", l.file, l.test)).copied().unwrap_or(Reach::SpecOnly) } else { Reach::Every }).max()
}

pub const SAMPLED: &str = include_str!("../spec/sampled.txt");

/// spec/sampled.txt: the settled clauses of PROVEN_FILES proved of the specification whose
/// proof reaches the models only on samples or not at all (implementation_reach below
/// `Every`): `<clause> <sampled|spec-only>`
pub fn sampled() -> BTreeMap<String, String> {
    SAMPLED.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).filter_map(|l| l.split_once(' ')).map(|(c, r)| (c.trim().to_string(), r.trim().to_string())).collect()
}

/// what spec/sampled.txt should hold now: each settled clause of PROVEN_FILES with a formal
/// law whose best reach (implementation_reach) is below `Every`, the Lean files read from `root`
pub fn sampled_now(root: &Path) -> std::io::Result<BTreeMap<String, crate::lean::ledger::Reach>> {
    let lean = crate::lean::ledger::reaches(root)?;
    let mut by: BTreeMap<String, Vec<Law>> = BTreeMap::new();
    for l in laws() {
        for c in &l.clauses {
            by.entry(c.clone()).or_default().push(l.clone());
        }
    }
    let mut out = BTreeMap::new();
    for c in crate::spec::clauses() {
        if !PROVEN_FILES.contains(&c.file) || !["doc", "measured"].contains(&c.status.as_str()) {
            continue;
        }
        let r = by.get(&c.id).and_then(|ls| implementation_reach(ls, &lean));
        if let Some(r) = r.filter(|r| *r < crate::lean::ledger::Reach::Every) {
            out.insert(c.id, r);
        }
    }
    Ok(out)
}

/// spec/sampled.txt's text for `now` (its header kept)
pub fn sampled_text(now: &BTreeMap<String, crate::lean::ledger::Reach>) -> String {
    let mut s: String = SAMPLED.lines().take_while(|l| l.starts_with('#')).map(|l| format!("{l}\n")).collect();
    for (c, r) in now {
        s += &format!("{c} {}\n", r.name());
    }
    s
}

pub const UNPROVEN: &str = include_str!("../spec/unproven.txt");

/// spec/unproven.txt: the settled clauses of PROVEN_FILES no formal law proves yet
pub fn unproven() -> std::collections::BTreeSet<String> {
    UNPROVEN.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(|l| l.trim().to_string()).collect()
}

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
        let p = p.split_once(": ").map_or(p, |(page, _)| page).trim().trim_end_matches(':').trim_end_matches(" and the others").trim();
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

/// Check source digests before treating recorded output as evidence.
pub fn verify_sources(dir: &Path, suites: &[&str]) -> std::io::Result<()> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("runner/golden.py");
    let check = std::process::Command::new("python3").arg(script).arg("verify").arg(dir).args(suites).output()?;
    check.status.success().ok_or_else(|| std::io::Error::other(String::from_utf8_lossy(&check.stderr).into_owned()))
}

/// every suite's cases, each executor, through the models and against Windows' answers in `dir`
pub fn trace(dir: &Path) -> std::io::Result<BTreeMap<String, Evidence>> {
    verify_sources(dir, crate::generators::SUITES)?;
    let mut m: BTreeMap<String, Evidence> = BTreeMap::new();
    for (s, generate) in crate::generators::GENERATORS {
        let text = generate();
        let cases = crate::case::parse_cases(&text);
        for arch in crate::generators::architectures(&text) {
            let path = dir.join(format!("{s}-{arch}/stdout.txt"));
            let stdout = match std::fs::read_to_string(&path) {
                Ok(text) => text,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
                Err(e) => return Err(std::io::Error::new(e.kind(), format!("{}: {e}", path.display()))),
            };
            let reference = crate::case::parse_output(&stdout);
            crate::model::ModelContext::recorded(arch, &stdout).evaluate(|| {
                for (i, c) in cases.iter().enumerate() {
                    crate::spec::take();
                    let (r, ended) = crate::model::run_case_ended(c);
                    let hits = crate::spec::take();
                    let want = reference.cases.get(i).filter(|(name, _)| *name == c.name).map(|(_, lines)| lines);
                    let kind = match (&r, want) {
                        (_, None) => 3,
                        (Err(crate::model::Fail::Unmodeled(_) | crate::model::Fail::NoInput(_)), _) => 2,
                        (Ok(l), Some(w)) if l.len() == w.len() && l.iter().zip(w.iter()).all(|(a, b)| crate::compare::line_matches(Some(a.as_str()), Some(b.as_str()))) => 0,
                        _ => 1,
                    };
                    for id in hits {
                        let e = m.entry(id.to_string()).or_default();
                        match kind {
                            0 => e.same += 1,
                            1 => e.differ += 1,
                            2 => {
                                e.unmodeled += 1;
                                if let Err(crate::model::Fail::Unmodeled(why)) = &r && why.as_str() != id {
                                    *e.blocked.entry(why.to_string()).or_default() += 1;
                                }
                            }
                            _ => e.unanswered += 1,
                        }
                        if e.cases.len() < 3 {
                            e.cases.push(format!("{s}-{arch}:{}", c.name));
                        }
                    }
                    if ended == crate::model::Ended::Crash { break; }
                }
            });
        }
    }
    for l in laws() {
        for id in &l.clauses {
            m.entry(id.clone()).or_default().laws.push(l.clone());
        }
    }
    Ok(m)
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
    if e.differ > 0 {
        v.push(format!("{} cases differ from Windows", e.differ));
    }
    match st {
        "doc" => {
            if undocumented(&c.source) {
                v.push(format!("doc, but its source {:?} is no documentation", c.source));
            }
            if executor(&c.source) {
                if e.same == 0 {
                    v.push(format!("the executor's own rule, but no case reaching it agrees with a Windows answer{}", blocked(e)));
                }
                return v;
            }
            for p in pages(&c.source) {
                if !learn.contains_key(&p) {
                    v.push(format!("page {p:?} not in spec/learn.txt"));
                }
            }
            if e.laws.is_empty() && e.same == 0 {
                v.push(format!("no law and no case Windows agrees with{}", blocked(e)));
            }
        }
        "measured" => {
            if e.same == 0 {
                v.push(format!("measured, but no case reaching it agrees with a Windows answer{}", blocked(e)));
            }
        }
        "assumed" if e.laws.is_empty() && e.same == 0 => {
            v.push(format!("no law and no case Windows agrees with{}", blocked(e)));
        }
        _ => {}
    }
    v
}

/// what Windows says of a clause's cases: one of them differs, they agree, or none was answered
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Windows {
    Differs,
    Agrees,
    Unanswered,
}

impl Windows {
    pub fn of(e: &Evidence) -> Windows {
        match (e.differ, e.same) {
            (1.., _) => Windows::Differs,
            (0, 1..) => Windows::Agrees,
            (0, 0) => Windows::Unanswered,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Windows::Differs => "differs",
            Windows::Agrees => "agrees",
            Windows::Unanswered => "-",
        }
    }
}

/// a clause's formal proofs of the specification, by kind (FORMAL_KINDS: a Lean theorem, an
/// exhaustive law, a Kani harness, a TLA+ law), apart from how far they reach the Rust models
/// (implementation_reach) and from what Windows says (Windows): the review of 84b05c8, 6
pub fn proofs(laws: &[Law]) -> String {
    let kinds: Vec<&str> = FORMAL_KINDS.iter().copied().filter(|k| laws.iter().any(|l| l.kind == *k)).collect();
    if kinds.is_empty() { "-".to_string() } else { kinds.join("+") }
}

/// one line per clause: id, status, its three kinds of evidence apart (the proofs of the
/// specification, how far they reach the Rust models, what Windows says), the cases' counts,
/// the laws, the gaps
pub fn table(dir: &Path) -> std::io::Result<String> {
    let t = trace(dir)?;
    let learn = learn();
    let lean = crate::lean::ledger::reaches(Path::new(env!("CARGO_MANIFEST_DIR")))?;
    let mut s = String::from("# clause status proof impl windows | same differ unmodeled unanswered laws(exhaustive/pbt) | gaps
# proof: the formal laws proving the specification (lean, exhaustive, kani, tla; -: none)
# impl: how far they reach the models' Rust functions (every: over the domain; sampled: a Lean theorem whose definitions the models match on samples; spec-only: of the specification alone; -: no formal law)
# windows: what the Windows answers say of the clause's cases (agrees, differs; -: none answered)
");
    let mut per: BTreeMap<&str, [usize; 14]> = BTreeMap::new();
    let none = Evidence::default();
    for c in crate::spec::clauses() {
        let e = t.get(&c.id).unwrap_or(&none);
        let ex = e.laws.iter().filter(|l| l.kind == "exhaustive").count();
        let g = gaps(&c, e, &learn);
        let reach = implementation_reach(&e.laws, &lean);
        let win = Windows::of(e);
        s += &format!("{:28} {:9} {:22} {:9} {:7} | {:6} {:3} {:5} {:5} {}/{} | {}\n", c.id, c.status, proofs(&e.laws), reach.map_or("-", |r| r.name()), win.name(), e.same, e.differ, e.unmodeled, e.unanswered, ex, e.laws.len() - ex, g.join("; "));
        let p = per.entry(c.file).or_default();
        let [.., lean_proven, every, sampled, spec_only, agrees, differs] = p;
        if e.laws.iter().any(|l| l.kind == "lean") {
            *lean_proven = lean_proven.saturating_add(1);
        }
        match reach {
            Some(crate::lean::ledger::Reach::Every) => *every = every.saturating_add(1),
            Some(crate::lean::ledger::Reach::Sampled) => *sampled = sampled.saturating_add(1),
            Some(crate::lean::ledger::Reach::SpecOnly) => *spec_only = spec_only.saturating_add(1),
            None => {}
        }
        match win {
            Windows::Agrees => *agrees = agrees.saturating_add(1),
            Windows::Differs => *differs = differs.saturating_add(1),
            Windows::Unanswered => {}
        }
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
    s += "# per API: clauses doc measured assumed open excluded with-a-law with-gaps | lean-proven | impl-every impl-sampled spec-only | windows-agrees windows-differs\n";
    for (f, p) in per {
        s += &format!("# {f:10} {:4} {:4} {:4} {:4} {:4} {:4} {:4} {:4} | {:4} | {:4} {:4} {:4} | {:4} {:4}\n", p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[11], p[12], p[13]);
    }
    Ok(s)
}
