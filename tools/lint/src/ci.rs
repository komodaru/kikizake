//! `apt-bundle` (2026-11-24, apt's mirror failed CI runs): CI installs apt packages only through
//! tools/ci/apt.sh, from the zstd tarball of their .debs on the release ci-bundles. A workflow or
//! script (tools/ci/apt.sh and tools/ci/apt-bundle.sh excepted) does not run apt-get, apt
//! install or apt update, nor keep .debs in a cache of its own; every set it gives apt.sh is a
//! line of tools/ci/apt-sets.txt, which .github/workflows/ci-bundles.yml uploads.
use std::collections::BTreeSet;
use std::path::Path;

/// what may run apt itself: the installer of the bundles and the script that makes them
pub const APT_OWN: [&str; 2] = ["tools/ci/apt.sh", "tools/ci/apt-bundle.sh"];
/// the sets ci-bundles.yml uploads, one per line
pub const APT_SETS: &str = "tools/ci/apt-sets.txt";
const APT_CALL: &str = "tools/ci/apt.sh";
/// what reads an apt mirror, or a cache of .debs that was filled from one
const APT_MIRROR: [&str; 6] = ["apt-get ", "apt install", "apt update", "dpkg -i", "path: ~/debs", ".apt-cache"];

/// the set a line gives tools/ci/apt.sh: its words up to the end of that command
pub fn apt_set(code: &str) -> Option<String> {
    let rest = code.match_indices(APT_CALL).filter_map(|(at, _)| code.get(at.saturating_add(APT_CALL.len())..)).find(|r| r.is_empty() || r.starts_with(char::is_whitespace) || r.starts_with(')'))?;
    let mut words = Vec::new();
    for w in rest.split_whitespace() {
        if matches!(w, "&&" | "||" | ";" | "|" | ")" | "\\") {
            break;
        }
        let word = w.trim_end_matches([';', ')']);
        if !word.is_empty() {
            words.push(word);
        }
        if word.len() < w.len() {
            break;
        }
    }
    Some(words.join(" "))
}

/// the sets of apt-sets.txt (`text`)
pub fn sets(text: &str) -> BTreeSet<String> {
    text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).map(str::to_owned).collect()
}

/// `apt-bundle`: what a workflow or script (`rel`, `text`) does against it
pub fn apt_bundle(rel: &str, text: &str, sets: &BTreeSet<String>) -> Vec<String> {
    if APT_OWN.contains(&rel) {
        return Vec::new();
    }
    let mut bad = Vec::new();
    for (k, l) in text.lines().enumerate() {
        if l.trim_start().starts_with('#') {
            continue;
        }
        let code = l.split(" #").next().unwrap_or(l);
        let line = k.saturating_add(1);
        if let Some(m) = APT_MIRROR.iter().find(|m| code.contains(*m)) {
            bad.push(format!("{rel}:{line}: apt-bundle: `{}` reads apt's mirror at CI time: tools/ci/apt.sh installs a set of {APT_SETS} from the release ci-bundles", m.trim()));
        }
        match apt_set(code) {
            Some(set) if set.is_empty() => bad.push(format!("{rel}:{line}: apt-bundle: tools/ci/apt.sh given no package")),
            Some(set) if !sets.contains(&set) => bad.push(format!("{rel}:{line}: apt-bundle: `{set}` is not a line of {APT_SETS}: ci-bundles has no tarball for it")),
            Some(_) | None => {}
        }
    }
    bad
}

pub fn check(root: &Path) -> Result<(), String> {
    let sets = sets(&std::fs::read_to_string(root.join(APT_SETS)).map_err(|e| format!("{APT_SETS}: {e}"))?);
    let mut bad = Vec::new();
    let mut n = 0usize;
    let mut stack = vec![root.join(".github/workflows"), root.join("tools"), root.join("runner"), root.join("tests"), root.join("spec")];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))? {
            let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|f| f != ".lake" && f != "target") {
                    stack.push(p);
                }
                continue;
            }
            if !matches!(p.extension().and_then(|x| x.to_str()), Some("sh" | "yml" | "yaml")) {
                continue;
            }
            let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
            let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
            bad.extend(apt_bundle(&rel, &text, &sets));
            n = n.saturating_add(1);
        }
    }
    if n == 0 {
        bad.push(".github/workflows: no workflow found".into());
    }
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "ci_tests.rs"]
mod tests;
