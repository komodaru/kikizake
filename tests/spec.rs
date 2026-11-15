//! The clauses (spec/) against the models and the suites: every clause is reached by some case
//! (a documented rule no case exercises is a hole in the suite), the models name no clause the
//! files lack, statuses are known, ids are unique.
use std::collections::BTreeSet;
use kikizake::spec;

#[test]
fn every_clause_is_reached_and_known() {
    let cl = spec::clauses();
    let ids: BTreeSet<_> = cl.iter().map(|c| c.id.clone()).collect();
    assert_eq!(ids.len(), cl.len(), "duplicate clause ids");
    for c in &cl {
        assert!(spec::STATUSES.contains(&c.status.as_str()), "{}: status {:?}", c.id, c.status);
        assert!(!c.source.is_empty() && !c.text.is_empty(), "{}: source and text", c.id);
    }
    let cov = spec::coverage();
    let unknown: Vec<_> = cov.keys().filter(|k| !ids.contains(*k)).collect();
    assert!(unknown.is_empty(), "the models name clauses spec/ lacks: {unknown:?}");
    let unreached: Vec<_> = cl.iter().filter(|c| !cov.contains_key(&c.id) && c.status != "excluded").map(|c| &c.id).collect();
    assert!(unreached.is_empty(), "clauses no case reaches: {unreached:?}");
    for c in &cl {
        eprintln!("{:24} {:8} {:5} cases", c.id, c.status, cov.get(&c.id).map_or(0, |v| v.len()));
    }
}

/// a case is named once in its suite: the answers are matched to the cases by name
#[test]
fn case_names_are_unique() {
    for s in kikizake::generators::SUITES {
        let mut seen = BTreeSet::new();
        for c in kikizake::case::parse_cases(&kikizake::generators::suite(s).unwrap()) {
            assert!(seen.insert(c.name.clone()), "{s}: case {} twice", c.name);
        }
    }
}

/// the constants build.rs makes (`clause::X`, `open::X`) are the clauses, so naming a clause in
/// a model is checked by the compiler; a clause that leaves open or excluded leaves `open::`
#[test]
fn clause_constants_are_the_clauses() {
    let made: Vec<String> = spec::clause::ALL.iter().map(|c| c.to_string()).collect();
    let ids: Vec<String> = spec::clauses().into_iter().map(|c| c.id).collect();
    assert_eq!(made, ids);
}

/// spec/laws.txt names tests that exist and clauses that exist, and lists every Kani harness and
/// every TLA+ spec
#[test]
fn laws_name_tests_and_clauses() {
    let ids: BTreeSet<_> = spec::clauses().into_iter().map(|c| c.id).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for l in kikizake::trace::laws() {
        let t = std::fs::read_to_string(root.join(&l.file)).unwrap_or_else(|_| panic!("spec/laws.txt: no file {}", l.file));
        // (a test or Kani harness is a fn; a TLA+ law is the spec's definition, checked by TLC
        // and replayed against the models: kikizake tla)
        let def = if l.kind == "tla" { format!("\n{} ==", l.test) } else { format!("fn {}(", l.test) };
        assert!(t.contains(&def), "spec/laws.txt: {} has no {} {}", l.file, l.kind, l.test);
        assert!(kikizake::trace::LAW_KINDS.contains(&l.kind.as_str()), "spec/laws.txt: {}::{} kind {:?}", l.file, l.test, l.kind);
        assert!(!l.clauses.is_empty(), "spec/laws.txt: {}::{} checks no clause", l.file, l.test);
        for c in &l.clauses {
            assert!(ids.contains(c), "spec/laws.txt: {}::{} names {c}, no clause", l.file, l.test);
        }
    }
    let listed: BTreeSet<String> = kikizake::trace::laws().iter().map(|l| format!("{}::{}", l.file, l.test)).collect();
    let proofs = std::fs::read_to_string(root.join("src/proofs.rs")).unwrap();
    for h in proofs.split("#[kani::proof]\nfn ").skip(1) {
        let h = h.split('(').next().unwrap();
        assert!(listed.contains(&format!("src/proofs.rs::{h}")), "spec/laws.txt does not list the Kani harness {h}");
    }
    for e in std::fs::read_dir(root.join("spec/tla")).unwrap() {
        let f = e.unwrap().file_name().into_string().unwrap();
        if let Some(m) = f.strip_suffix(".tla") {
            assert!(listed.iter().any(|l| l.starts_with(&format!("spec/tla/{f}::"))), "spec/laws.txt does not list spec/tla/{f}");
            assert!(root.join(format!("spec/tla/{m}.cfg")).exists(), "spec/tla/{f} has no {m}.cfg");
        }
    }
}

/// every clause has the evidence its status claims (kikizake trace: the table, per API):
/// reached by a case, no case differing from Windows, a doc clause citing pages spec/learn.txt
/// knows and checked by a law or a case Windows agrees with, a measured one by a case Windows
/// agrees with
#[test]
fn clauses_have_their_evidence() {
    let dir = std::env::var_os("KIKIZAKE_GOLDEN").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden"));
    let t = kikizake::trace::trace(&dir);
    let learn = kikizake::trace::learn();
    let none = kikizake::trace::Evidence::default();
    let mut bad = Vec::new();
    for c in spec::clauses() {
        let g = kikizake::trace::gaps(&c, t.get(&c.id).unwrap_or(&none), &learn);
        if !g.is_empty() {
            bad.push(format!("{} ({}, spec/{}.txt): {}", c.id, c.status, c.file, g.join("; ")));
        }
    }
    assert!(bad.is_empty(), "{} clauses lack evidence:\n{}", bad.len(), bad.join("\n"));
}

/// nothing is left unresolved: every clause open (not modelled), assumed (not settled) or
/// excluded (not asked) is listed in spec/unresolved.txt, which only shrinks. A new clause comes
/// settled (doc or measured), and one settled leaves the list (the goal is an empty list).
#[test]
fn unresolved_only_shrinks() {
    let listed: BTreeSet<String> = include_str!("../spec/unresolved.txt").lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()).map(|l| l.trim().to_string()).collect();
    let now: BTreeSet<String> = spec::clauses().into_iter().filter(|c| ["open", "assumed", "excluded"].contains(&c.status.as_str())).map(|c| format!("{} {}", c.id, c.status)).collect();
    let new: Vec<_> = now.difference(&listed).collect();
    let gone: Vec<_> = listed.difference(&now).collect();
    assert!(new.is_empty(), "clauses left unresolved (settle them: doc or measured, by Windows' answers):\n{}", new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(gone.is_empty(), "settled clauses still listed in spec/unresolved.txt (take them out):\n{}", gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}
