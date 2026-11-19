//! kikizake command line (README.md):
//!   kikizake gen <suite>                  the suite's case file (cases/<suite>.txt)
//!   kikizake model <suite> [cases file]   what Windows prints for them, by the model
//!   kikizake compare <want> <got>         two outputs, case by case (exit 1 on a difference)
//!   kikizake spec                         each clause (spec/), its status and how many cases reach it
//!   kikizake learn pages|resolve|check    the documentation the clauses cite (src/learn.rs)
//!   kikizake tla <TLC dump>               a TLA+ spec's every state (spec/tla/) through the model
//!   kikizake trace [golden dir]           each clause's evidence (cases against Windows, laws) and gaps, per API
//!   kikizake calls import <suite>-<arch> <cases> <jsonl> <stdout> <src> <shirusu rev> [plain repeat ...]
//!                                         a shirusu recording of the executor as calls/<suite>-<arch>.txt (src/calls.rs)
//!   kikizake calls check [calls dir]      the recordings against the cases, x86 against x86-64
//!   kikizake calls usage <app.jsonl> [calls dir]  graphics calls and argument values an application (shirusu export) makes that no case does
use std::process::exit;
use kikizake::{case, compare, generators, model};

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| {
        eprintln!("{p}: {e}");
        exit(2)
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["gen", suite] => print!("{}", generators::suite(suite).unwrap_or_else(|| {
            eprintln!("suites: {:?}", generators::SUITES);
            exit(2)
        })),
        ["model", suite, rest @ ..] => {
            // (KIKIZAKE_ARCH=64: the answers of the x86-64 executor)
            if std::env::var("KIKIZAKE_ARCH").as_deref() == Ok("64") {
                model::set_arch(64);
            }
            // (KIKIZAKE_CPU="cpu ...": the processor line of the answer compared with)
            model::set_cpu(std::env::var("KIKIZAKE_CPU").ok());
            // (KIKIZAKE_FITTED=1: the fitted approximations of open clauses answer too)
            if std::env::var("KIKIZAKE_FITTED").as_deref() == Ok("1") {
                model::set_fitted(model::Fitted::Answered);
            }
            if !generators::SUITES.contains(suite) || rest.len() > 1 {
                eprintln!("expected a known suite and at most one case file: {suite}");
                exit(2);
            }
            let text = match rest {
                [f] => read(f),
                _ => generators::suite(suite).unwrap_or_else(|| exit(2)),
            };
            let cases = case::checked_cases(&text).unwrap_or_else(|e| {
                eprintln!("{suite}: {e}");
                exit(2)
            });
            if cases.is_empty() {
                eprintln!("no cases in {suite}");
                exit(2);
            }
            let (output, invalid) = model::run_report(suite, &cases);
            print!("{output}");
            if !invalid.is_empty() {
                eprintln!("invalid case input: {}", invalid.join(", "));
                exit(2);
            }
        }
        ["spec"] => {
            let cov = kikizake::spec::coverage();
            for c in kikizake::spec::clauses() {
                println!("{:24} {:9} {:5}  {}", c.id, c.status, cov.get(&c.id).map_or(0, |v| v.len()), c.text);
            }
        }
        ["learn", rest @ ..] => exit(kikizake::learn::main(rest)),
        ["tla", rest @ ..] => exit(kikizake::tla::main(rest)),
        ["trace", rest @ ..] => match kikizake::trace::table(std::path::Path::new(rest.first().copied().unwrap_or("golden"))) {
            Ok(table) => print!("{table}"),
            Err(e) => { eprintln!("{e}"); exit(2); }
        },
        ["calls", "import", name, cases, jsonl, stdout, src, rev, repeats @ ..] => {
            let repeats: Vec<_> = repeats.iter().map(|f| read(f)).collect();
            let mut r = kikizake::calls::from_jsonl_with_repeats(&read(jsonl), Some(&read(stdout)), &repeats).unwrap_or_else(|e| {
                eprintln!("{jsonl}: {e}");
                exit(2)
            });
            r.header.name = name.to_string();
            r.header.cases = kikizake::calls::CasesHash::of(&read(cases));
            r.header.src = src.to_string();
            r.header.shirusu = rev.to_string();
            print!("{}", kikizake::calls::format(&r));
        }
        ["calls", "usage", app, rest @ ..] => {
            let dir = std::path::Path::new(rest.first().copied().unwrap_or("calls"));
            let recs: Vec<_> = std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| Some(e.ok()?.path()).filter(|p| p.extension().is_some_and(|x| x == "txt")))
                .filter_map(|p| kikizake::calls::parse(&std::fs::read_to_string(p).ok()?).ok())
                .collect();
            match kikizake::calls::usage(&read(app), &recs) {
                Ok(lines) => lines.iter().for_each(|l| println!("{l}")),
                Err(e) => {
                    eprintln!("{app}: {e}");
                    exit(2)
                }
            }
        }
        ["calls", "check", rest @ ..] => {
            let r = kikizake::calls::check_dir(std::path::Path::new(rest.first().copied().unwrap_or("calls")), std::path::Path::new("."));
            print!("{}", r.report());
            exit(if r.problems() == 0 { 0 } else { 1 })
        }
        ["compare", want, got] => {
            let r = compare::compare_text(&read(want), &read(got));
            for d in &r.differ {
                println!("{d}");
            }
            println!("{} cases: {} same, {} differ, {} unmodeled", r.cases, r.same, r.differ.len(), r.unmodeled);
            exit(if r.differ.is_empty() { 0 } else { 1 })
        }
        _ => {
            eprintln!("usage: kikizake gen <suite> | model <suite> [cases] | compare <want> <got> | spec | trace [golden] | calls import|check|usage | learn pages|resolve|check | tla <dump>");
            exit(2)
        }
    }
}
