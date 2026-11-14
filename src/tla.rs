//! The TLA+ specifications (spec/tla/) against the models: TLC dumps every reachable state of a
//! spec (`-dump`), each with `log`, the behavior that reached it as case-language lines ("> " and
//! a call) and the lines the executor prints for them. `kikizake tla <dump>` runs each state's
//! calls through the model as one case and compares what it prints with the spec's lines
//! (spec/tla/README.md).

/// each state's log: its quoted strings in order
pub fn logs(dump: &str) -> Vec<Vec<String>> {
    let mut v = Vec::new();
    for block in format!("\n{dump}").split("\nState ").skip(1) {
        let Some(i) = block.find("/\\ log = ") else { continue };
        let rest = &block[i + 9..];
        let end = rest.find("\n/\\ ").unwrap_or(rest.len());
        v.push(rest[..end].split('"').skip(1).step_by(2).map(String::from).collect());
    }
    v
}

/// (states checked, how many differ, the first differences)
pub fn check(dump: &str) -> (usize, usize, Vec<String>) {
    let mut bad = Vec::new();
    let mut differ = 0;
    let all = logs(dump);
    for log in &all {
        let mut text = String::from("case tla\n");
        let mut want = Vec::new();
        for l in log {
            match l.strip_prefix("> ") {
                Some(call) => {
                    text += call;
                    text += "\n";
                }
                None => want.push(l.clone()),
            }
        }
        text += "end\n";
        let c = crate::case::parse_cases(&text);
        let got = crate::model::run_case(&c[0]);
        if got.as_ref() == Ok(&want) {
            continue;
        }
        differ += 1;
        if bad.len() < 10 {
            bad.push(format!("{}\n  spec:  {:?}\n  model: {:?}", text.trim_end(), want, got));
        }
    }
    (all.len(), differ, bad)
}

pub fn main(args: &[&str]) -> i32 {
    let [dump] = args else {
        eprintln!("usage: kikizake tla <TLC -dump file>");
        return 2;
    };
    let text = std::fs::read_to_string(dump).unwrap_or_else(|e| panic!("{dump}: {e}"));
    let (n, differ, bad) = check(&text);
    for b in &bad {
        println!("{b}");
    }
    println!("{n} states, {differ} differ from the model");
    (differ > 0) as i32
}
