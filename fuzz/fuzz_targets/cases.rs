//! Any bytes as a case file and as executor output: parsing, the models and the comparison
//! never panic, the model answers through the first process stop, and a nonempty valid
//! output compares equal to itself.
#![no_main]
use libfuzzer_sys::fuzz_target;
use kikizake::{case, compare, model};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let cases = case::parse_cases(&text);
    let out = model::run("fuzz", &cases);
    let parsed = case::parse_output(&out);
    let evaluated: Vec<_> = model::evaluate_cases(&cases).collect();
    assert_eq!(parsed.cases.len(), evaluated.len());
    assert!(parsed.cases.iter().zip(&evaluated).all(|((name, _), (case, _, _))| name == &case.name));
    assert_eq!(parsed.done, evaluated.last().is_none_or(|(_, _, ended)| *ended == model::Ended::Next));
    if parsed.done { assert_eq!(parsed.cases.len(), cases.len()); }
    let concrete = out.lines().map(|line| line.strip_prefix("oneof ")
        .and_then(|choices| choices.split(" | ").next()).unwrap_or(line)).collect::<Vec<_>>().join("\n");
    let r = compare::compare_text(&out, &concrete);
    let valid = !cases.is_empty() && evaluated.iter().all(|(_, result, _)| !matches!(result, Err(model::Fail::Bad | model::Fail::Empty)));
    assert_eq!(r.differ.is_empty(), valid, "{:?}", r.differ);
    let _ = compare::compare_text(&text, &out);
});
