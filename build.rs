//! The clauses (spec/<api>.txt) as Rust constants, so a model can name only a clause that
//! exists (`clause::GDI_ROP3_TRUTH`), and `Fail::Unmodeled` only an open or excluded one
//! (`open::GDI_FMT_NEAREST`): a wrong name or status is a compile error, not a test failure.
use std::fmt::Write;

fn main() {
    // (the clause files src/spec.rs lists in FILES)
    println!("cargo:rerun-if-changed=src/spec.rs");
    let list = std::fs::read_to_string("src/spec.rs").unwrap();
    let files: Vec<std::path::PathBuf> = list.split("include_str!(\"../").skip(1).map(|r| r.split('"').next().unwrap().into()).collect();
    let (mut all, mut open, mut every) = (String::new(), String::new(), String::new());
    let mut seen = std::collections::BTreeMap::new();
    for f in files {
        let name = f.file_stem().unwrap().to_str().unwrap().to_string();
        println!("cargo:rerun-if-changed={}", f.display());
        for l in std::fs::read_to_string(&f).unwrap().lines() {
            if l.starts_with('#') || l.trim().is_empty() {
                continue;
            }
            let head = l.split(" | ").next().unwrap();
            let (id, status) = head.split_once(' ').unwrap_or_else(|| panic!("{}: no status: {l}", f.display()));
            let status = status.trim();
            assert!(["doc", "measured", "assumed", "open", "excluded"].contains(&status), "{}: {id}: status {status:?}", f.display());
            assert!(id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '_'), "{}: clause id {id:?}", f.display());
            let ident = id.replace('.', "_");
            if let Some(other) = seen.insert(ident.clone(), id.to_string()) {
                panic!("{}: clause {id} and {other} make the same name {ident}", f.display());
            }
            writeln!(all, "    /// spec/{name}.txt ({status})\n    pub const {ident}: ClauseId = ClauseId(\"{id}\");").unwrap();
            write!(every, "{ident}, ").unwrap();
            if status == "open" || status == "excluded" {
                writeln!(open, "    pub const {ident}: OpenId = OpenId(\"{id}\");").unwrap();
            }
        }
    }
    let out = format!("/// every clause, by name\npub mod clause {{\n    use super::ClauseId;\n{all}    /// all of them, in file order\n    pub const ALL: &[ClauseId] = &[{every}];\n}}\n/// the open and excluded clauses: what `Fail::Unmodeled` may name\npub mod open {{\n    use super::OpenId;\n{open}}}\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("clauses.rs"), out).unwrap();
}
