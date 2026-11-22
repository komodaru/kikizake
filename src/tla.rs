//! The TLA+ specifications (spec/tla/) against the models: TLC dumps every reachable state of a
//! spec (`-dump`), each with `log`, the behavior that reached it as case-language lines ("> " and
//! a call) and the lines the executor prints for them. `kikizake tla <dump>` runs each state's
//! calls through the model as one case and compares what it prints with the spec's lines
//! (spec/tla/README.md).

/// each state's log: its quoted strings in order
pub fn logs(dump: &str) -> Result<Vec<Vec<String>>, String> {
    let mut states = Vec::new();
    let dump = dump.trim();
    if !dump.starts_with("State ") {
        return Err("expected a TLC state dump".into());
    }
    for block in dump.split("\nState ") {
        let (header, body) = block.split_once('\n').ok_or("state missing its variables")?;
        let (_, rest) = body.split_once("/\\ log = ").ok_or_else(|| format!("{header}: missing log"))?;
        let tuple = rest.split("\n/\\ ").next().unwrap_or(rest).trim();
        let mut items = tuple.strip_prefix("<<").and_then(|t| t.strip_suffix(">>"))
            .ok_or_else(|| format!("{header}: malformed log tuple"))?.trim();
        let mut lines = Vec::new();
        while !items.is_empty() {
            let (line, tail) = items.strip_prefix('"').and_then(|t| t.split_once('"'))
                .ok_or_else(|| format!("{header}: expected a quoted log entry"))?;
            lines.push(line.to_owned());
            if tail.trim().is_empty() { break; }
            items = tail.trim_start().strip_prefix(',').ok_or("expected a log separator")?.trim_start();
            if items.is_empty() { return Err("missing final log entry".into()); }
        }
        states.push(lines);
    }
    Ok(states)
}

/// (states checked, how many differ, the first differences)
pub fn check(dump: &str) -> Result<(usize, usize, Vec<String>), String> {
    let mut bad = Vec::new();
    let mut differ = 0;
    let all = logs(dump)?;
    for log in &all {
        let mut text = String::from("case tla\n");
        let mut want = Vec::new();
        // (a log entry "# arch 64" (or 32) runs the state as that executor's model; x86 else)
        let mut arch = 32;
        for l in log {
            if let Some(a) = l.strip_prefix("# arch ") {
                arch = a.parse().map_err(|e| format!("bad arch entry {l:?}: {e}"))?;
                continue;
            }
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
        crate::model::set_arch(arch);
        let got = crate::model::run_case(&c[0]);
        crate::model::set_arch(32);
        if got.as_ref() == Ok(&want) {
            continue;
        }
        differ += 1;
        if bad.len() < 10 {
            bad.push(format!("{}\n  spec:  {:?}\n  model: {:?}", text.trim_end(), want, got));
        }
    }
    Ok((all.len(), differ, bad))
}

pub fn main(args: &[&str]) -> i32 {
    let [dump] = args else {
        eprintln!("usage: kikizake tla <TLC -dump file>");
        return 2;
    };
    let result = std::fs::read_to_string(dump).map_err(|e| e.to_string()).and_then(|text| check(&text));
    let (n, differ, bad) = match result {
        Ok(result) => result,
        Err(e) => { eprintln!("{dump}: {e}"); return 2; }
    };
    for b in &bad {
        println!("{b}");
    }
    println!("{n} states, {differ} differ from the model");
    (differ > 0) as i32
}
