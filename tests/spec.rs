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
    for s in kikizake::gen::SUITES {
        let mut seen = BTreeSet::new();
        for c in kikizake::case::parse_cases(&kikizake::gen::suite(s).unwrap()) {
            assert!(seen.insert(c.name.clone()), "{s}: case {} twice", c.name);
        }
    }
}

fn model_sources(dir: &std::path::Path, v: &mut Vec<(String, String)>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            model_sources(&p, v);
        } else if p.extension().is_some_and(|x| x == "rs") {
            v.push((p.display().to_string(), std::fs::read_to_string(&p).unwrap()));
        }
    }
}

/// Fail::Unmodeled names the clause that leaves the case open: an `open` one (or `excluded`, not
/// asked), never one the model claims to do
#[test]
fn unmodeled_names_an_open_clause() {
    let st: std::collections::BTreeMap<_, _> = spec::clauses().into_iter().map(|c| (c.id, c.status)).collect();
    let mut src = Vec::new();
    model_sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/model"), &mut src);
    let mut bad = Vec::new();
    for (f, text) in &src {
        for (i, l) in text.lines().enumerate() {
            if !l.contains("Unmodeled(") {
                continue;
            }
            for (k, w) in l.split('"').enumerate() {
                if k % 2 == 1 && w.contains('.') && w.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '_') {
                    match st.get(w).map(String::as_str) {
                        Some("open") | Some("excluded") => {}
                        s => bad.push(format!("{f}:{}: {w} is {}", i + 1, s.unwrap_or("no clause"))),
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "Unmodeled names a clause that is not open:\n{}", bad.join("\n"));
}

/// spec/laws.txt names tests that exist and clauses that exist
#[test]
fn laws_name_tests_and_clauses() {
    let ids: BTreeSet<_> = spec::clauses().into_iter().map(|c| c.id).collect();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for l in kikizake::trace::laws() {
        let t = std::fs::read_to_string(root.join(&l.file)).unwrap_or_else(|_| panic!("spec/laws.txt: no file {}", l.file));
        assert!(t.contains(&format!("fn {}(", l.test)), "spec/laws.txt: {} has no test {}", l.file, l.test);
        assert!(kikizake::trace::LAW_KINDS.contains(&l.kind.as_str()), "spec/laws.txt: {}::{} kind {:?}", l.file, l.test, l.kind);
        assert!(!l.clauses.is_empty(), "spec/laws.txt: {}::{} checks no clause", l.file, l.test);
        for c in &l.clauses {
            assert!(ids.contains(c), "spec/laws.txt: {}::{} names {c}, no clause", l.file, l.test);
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
