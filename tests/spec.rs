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
        let def = match l.kind.as_str() {
            "tla" => format!("\n{} ==", l.test),
            "lean" => format!("\ntheorem {} ", l.test),
            _ => format!("fn {}(", l.test),
        };
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
    for f in lean_files(&root.join("spec/lean/Kikizake")) {
        let rel = f.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
        let text = std::fs::read_to_string(&f).unwrap();
        for t in lean_theorems(&text) {
            assert!(listed.contains(&format!("{rel}::{t}")), "spec/laws.txt does not list the Lean theorem {rel}::{t}");
        }
    }
}

/// the .lean files under `dir`
fn lean_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut v = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            v.extend(lean_files(&p));
        } else if p.extension().is_some_and(|x| x == "lean") {
            v.push(p);
        }
    }
    v.sort();
    v
}

/// the names of a Lean file's public theorems (a `private theorem` is a step of another)
fn lean_theorems(text: &str) -> Vec<String> {
    text.lines().filter_map(|l| l.strip_prefix("theorem ")).map(|l| l.split_whitespace().next().unwrap().to_string()).collect()
}

/// every Lean file with a theorem has a mirror (src/lean/) that holds its definitions equal to
/// the model's functions, each mirror is defined in the file it names and run by Main.lean, and
/// Kikizake.lean imports every file, so `lake build` checks every theorem
#[test]
fn lean_theorems_are_about_the_models() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let ms = kikizake::lean::mirrors();
    let names: BTreeSet<&str> = ms.iter().map(|m| m.name).collect();
    assert_eq!(names.len(), ms.len(), "two mirrors share a name");
    let lib = std::fs::read_to_string(root.join("spec/lean/Kikizake.lean")).unwrap();
    let main = std::fs::read_to_string(root.join("spec/lean/Main.lean")).unwrap();
    for f in lean_files(&root.join("spec/lean/Kikizake")) {
        let rel = f.strip_prefix(root).unwrap().to_str().unwrap().replace('\\', "/");
        let module = rel.strip_prefix("spec/lean/").unwrap().strip_suffix(".lean").unwrap().replace('/', ".");
        let text = std::fs::read_to_string(&f).unwrap();
        if module == "Kikizake.Mirror" {
            continue;
        }
        assert!(lib.lines().any(|l| l == format!("import {module}")), "spec/lean/Kikizake.lean does not import {module}");
        let mine: Vec<_> = ms.iter().filter(|m| m.lean == rel).collect();
        assert!(lean_theorems(&text).is_empty() || !mine.is_empty(), "{rel} has theorems but no mirror (src/lean/) ties its definitions to the model");
        let ns = text.lines().find_map(|l| l.strip_prefix("namespace ")).unwrap_or_else(|| panic!("{rel}: no namespace"));
        let short = ns.strip_prefix("Kikizake.").unwrap();
        assert!(mine.is_empty() || main.contains(&format!("{short}.mirrors")), "spec/lean/Main.lean does not run {short}.mirrors");
        for m in mine {
            assert!(text.contains(&format!("⟨\"{}\",", m.name)), "{rel} does not define the mirror {}", m.name);
        }
    }
    for m in &ms {
        assert!(root.join(m.lean).exists(), "mirror {}: no file {}", m.name, m.lean);
    }
}

/// every settled clause of the proven APIs (trace::PROVEN_FILES) has a formal law
/// (trace::FORMAL_KINDS: exhaustive, kani, tla, lean) or is listed in spec/unproven.txt, which
/// only shrinks: a proven clause is taken out, a new clause comes proven
#[test]
fn unproven_only_shrinks() {
    let formal: BTreeSet<String> = kikizake::trace::laws().into_iter().filter(|l| kikizake::trace::FORMAL_KINDS.contains(&l.kind.as_str())).flat_map(|l| l.clauses).collect();
    let listed = kikizake::trace::unproven();
    let settled: BTreeSet<String> = spec::clauses().into_iter().filter(|c| kikizake::trace::PROVEN_FILES.contains(&c.file) && ["doc", "measured"].contains(&c.status.as_str())).map(|c| c.id).collect();
    let new: Vec<_> = settled.iter().filter(|c| !formal.contains(*c) && !listed.contains(*c)).collect();
    let gone: Vec<_> = listed.iter().filter(|c| formal.contains(*c) || !settled.contains(*c)).collect();
    assert!(new.is_empty(), "settled clauses with no formal law (prove them: spec/lean, spec/tla, src/proofs.rs, an exhaustive law):\n{}", new.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    assert!(gone.is_empty(), "clauses in spec/unproven.txt that are proven or not settled (take them out):\n{}", gone.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
}

/// every clause has the evidence its status claims (kikizake trace: the table, per API):
/// reached by a case, no case differing from Windows, a doc clause citing pages spec/learn.txt
/// knows and checked by a law or a case Windows agrees with, a measured one by a case Windows
/// agrees with
#[test]
fn clauses_have_their_evidence() {
    let dir = std::env::var_os("KIKIZAKE_GOLDEN").map(std::path::PathBuf::from).unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("golden"));
    let t = kikizake::trace::trace(&dir).unwrap();
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
