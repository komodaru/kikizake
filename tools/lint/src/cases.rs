//! Validate the inputs that will be compiled into the Windows executors.
use std::path::Path;

use kikizake::{case::checked_cases, generators::GENERATORS};

/// the lines that record the executor's environment, an input of the model for the cases after
/// (the review of 8480456, 4: d3dx9load's cpu case came last, and a stop before it lost the
/// processor and with it 42 answered cases): only in a suite's first case
const ENVIRONMENT: &[&str] = &["x9cpu"];

fn blocks(label: &str, text: &str) -> Result<(), String> {
    let cases = checked_cases(text).map_err(|e| format!("{label}: {e}"))?;
    if cases.is_empty() {
        return Err(format!("{label}: no cases"));
    }
    for (i, c) in cases.iter().enumerate().skip(1) {
        if let Some(op) = c.ops.iter().find(|o| o.tok.first().is_some_and(|t| ENVIRONMENT.contains(&t.as_str()))) {
            return Err(format!("{label}: case {} ({}): {} records the environment: put it in the first case", i.saturating_add(1), c.name, op.tok.join(" ")));
        }
    }
    Ok(())
}

fn suite(name: &str, stored: &str, generated: &str) -> Result<(), String> {
    let path = format!("cases/{name}.txt");
    blocks(&path, stored)?;
    if stored != generated {
        blocks(&format!("generator for {name}"), generated)?;
        let line = stored.lines().zip(generated.lines()).take_while(|(a, b)| a == b).count().saturating_add(1);
        return Err(format!("{path}: line {line}: differs from generator for {name}"));
    }
    Ok(())
}

pub fn check(root: &Path) -> Result<(), String> {
    let mut errors = Vec::new();
    for &(name, generate) in GENERATORS {
        let path = root.join("cases").join(format!("{name}.txt"));
        let result = std::fs::read_to_string(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
            .and_then(|stored| suite(name, &stored, &generate()));
        if let Err(error) = result {
            errors.push(error);
        }
    }
    errors.is_empty().ok_or_else(|| errors.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_broken_stored_or_generated_blocks() {
        let valid = "case cpu\nx9cpu\nend\ncase cpu\nend\n";
        assert!(suite("sample", valid, valid).is_ok());
        for (bad, problem) in [
            ("case cpu\nx9cpu\ncase f16-0\nend", "line 3"),
            ("case factory\nend\nend", "line 3"),
            ("case factory\ndxgiwarp", "line 2"),
            ("device11", "line 1"),
            ("# comment", "no cases"),
        ] {
            assert!(suite("sample", bad, bad).unwrap_err().contains(&format!("cases/sample.txt: {problem}")));
            assert!(suite("sample", valid, bad).unwrap_err().contains(&format!("generator for sample: {problem}")));
        }
        assert!(suite("sample", valid, "case other\nend").unwrap_err().contains("line 1: differs"));
        // (the environment after another case)
        let late = "case f16-0\nx9f16 0\nend\ncase cpu\nx9cpu\nend\n";
        assert!(suite("sample", late, late).unwrap_err().contains("case 2 (cpu): x9cpu records the environment"));
    }
}
