//! kikizake command line (README.md):
//!   kikizake gen <suite>                  the suite's case file (cases/<suite>.txt)
//!   kikizake model <suite> [cases file]   what Windows prints for them, by the model
//!   kikizake compare <want> <got>         two outputs, case by case (exit 1 on a difference)
//!   kikizake spec                         each clause (spec/), its status and how many cases reach it
//!   kikizake learn pages|resolve|check    the documentation the clauses cite (src/learn.rs)
//!   kikizake trace [golden dir]           each clause's evidence (cases against Windows, laws) and gaps, per API
//!   kikizake calls import <suite>-<arch> <cases> <jsonl> <stdout> <src> <shirusu rev>
//!                                         a shirusu recording of the executor as calls/<suite>-<arch>.txt (src/calls.rs)
//!   kikizake calls check [calls dir]      the recordings against the cases, x86 against x86-64
use std::process::exit;
use kikizake::{case, compare, gen, model};

fn read(p: &str) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| {
        eprintln!("{p}: {e}");
        exit(2)
    })
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["gen", suite] => print!("{}", gen::suite(suite).unwrap_or_else(|| {
            eprintln!("suites: {:?}", gen::SUITES);
            exit(2)
        })),
        ["model", suite, rest @ ..] => {
            // (KIKIZAKE_ARCH=64: the answers of the x86-64 executor)
            if std::env::var("KIKIZAKE_ARCH").as_deref() == Ok("64") {
                model::set_arch(64);
            }
            let text = match rest {
                [f] => read(f),
                _ => gen::suite(suite).unwrap_or_default(),
            };
            print!("{}", model::run(suite, &case::parse_cases(&text)));
        }
        ["spec"] => {
            let cov = kikizake::spec::coverage();
            for c in kikizake::spec::clauses() {
                println!("{:24} {:9} {:5}  {}", c.id, c.status, cov.get(&c.id).map_or(0, |v| v.len()), c.text);
            }
        }
        ["learn", rest @ ..] => exit(kikizake::learn::main(rest)),
        ["trace", rest @ ..] => print!("{}", kikizake::trace::table(std::path::Path::new(rest.first().copied().unwrap_or("golden")))),
        ["calls", "import", name, cases, jsonl, stdout, src, rev] => {
            let (cs, same, events, other) = kikizake::calls::from_jsonl(&read(jsonl), std::fs::read_to_string(stdout).ok().as_deref()).unwrap_or_else(|e| {
                eprintln!("{jsonl}: {e}");
                exit(2)
            });
            let header = format!("kikizake calls {name} cases {} src {src} shirusu {rev} stdout {same} events {events} other {other}", kikizake::calls::fnv(&read(cases)));
            print!("{}", kikizake::calls::format(&kikizake::calls::Recording { header, cases: cs }));
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
            eprintln!("usage: kikizake gen <suite> | model <suite> [cases] | compare <want> <got> | spec | trace [golden] | calls import|check | learn pages|resolve|check");
            exit(2)
        }
    }
}
