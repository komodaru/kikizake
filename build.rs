//! The clauses (spec/<api>.txt) as Rust constants, so a model can name only a clause that
//! exists (`clause::GDI_ROP3_TRUTH`), and `Fail::Unmodeled` only an open or excluded one
//! (`open::GDI_FMT_NEAREST`): a wrong name or status is a compile error, not a test failure.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // (the clause files src/spec.rs lists in FILES)
    println!("cargo:rerun-if-changed=src/spec.rs");
    let list = std::fs::read_to_string("src/spec.rs")?;
    let files: Vec<std::path::PathBuf> = list.split("include_str!(\"../").skip(1).map(|r| r.split_once('"').map_or(r, |(path, _)| path).into()).collect();
    let (mut all, mut open, mut every) = (String::new(), String::new(), String::new());
    let mut seen = std::collections::BTreeMap::new();
    for f in files {
        let name = f.file_stem().and_then(|s| s.to_str()).ok_or_else(|| format!("{}: no file name", f.display()))?.to_string();
        println!("cargo:rerun-if-changed={}", f.display());
        for l in std::fs::read_to_string(&f)?.lines() {
            if l.starts_with('#') || l.trim().is_empty() {
                continue;
            }
            let head = l.split_once(" | ").map_or(l, |(head, _)| head);
            let (id, status) = head.split_once(' ').ok_or_else(|| format!("{}: no status: {l}", f.display()))?;
            let status = status.trim();
            if !["doc", "measured", "assumed", "open", "excluded"].contains(&status) {
                return Err(format!("{}: {id}: status {status:?}", f.display()).into());
            }
            if !id.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '.' || c == '_') {
                return Err(format!("{}: clause id {id:?}", f.display()).into());
            }
            let ident = id.replace('.', "_");
            if let Some(other) = seen.insert(ident.clone(), id.to_string()) {
                return Err(format!("{}: clause {id} and {other} make the same name {ident}", f.display()).into());
            }
            all.push_str(&format!("    /// spec/{name}.txt ({status})\n    pub const {ident}: ClauseId = ClauseId(\"{id}\");\n"));
            every.push_str(&format!("{ident}, "));
            if status == "open" || status == "excluded" {
                open.push_str(&format!("    pub const {ident}: OpenId = OpenId(\"{id}\");\n"));
            }
        }
    }
    let out = format!("/// every clause, by name\npub mod clause {{\n    use super::ClauseId;\n{all}    /// all of them, in file order\n    pub const ALL: &[ClauseId] = &[{every}];\n}}\n/// the open and excluded clauses: what `Fail::Unmodeled` may name\npub mod open {{\n    use super::OpenId;\n{open}}}\n");
    std::fs::write(std::path::Path::new(&std::env::var("OUT_DIR")?).join("clauses.rs"), out)?;
    Ok(())
}
