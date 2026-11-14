//! kikizake command line (README.md):
//!   kikizake gen <suite>                  the suite's case file (cases/<suite>.txt)
//!   kikizake model <suite> [cases file]   what Windows prints for them, by the model
//!   kikizake compare <want> <got>         two outputs, case by case (exit 1 on a difference)
//!   kikizake spec                         each clause (spec/), its status and how many cases reach it
//!   kikizake learn pages|resolve|check    the documentation the clauses cite (src/learn.rs)
//!   kikizake trace [golden dir]           each clause's evidence (cases against Windows, laws) and gaps, per API
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
        ["compare", want, got] => {
            let r = compare::compare_text(&read(want), &read(got));
            for d in &r.differ {
                println!("{d}");
            }
            println!("{} cases: {} same, {} differ, {} unmodeled", r.cases, r.same, r.differ.len(), r.unmodeled);
            exit(if r.differ.is_empty() { 0 } else { 1 })
        }
        _ => {
            eprintln!("usage: kikizake gen <suite> | model <suite> [cases] | compare <want> <got> | spec | trace [golden] | learn pages|resolve|check");
            exit(2)
        }
    }
}
