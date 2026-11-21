//! The clauses (spec/*.txt, spec/README.md): what each API does, numbered, with its status
//! and source. The models name each clause they apply (`hit`); a case that reaches an `open`
//! clause is unmodelled (Fail::Unmodeled(id)). The coverage of a run is the set of clauses hit.

use std::cell::RefCell;
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq)]
pub struct Clause {
    pub id: String,
    pub status: String,
    pub source: String,
    pub text: String,
    pub file: &'static str,
}

/// every clause file, one line each
pub const FILES: &[(&str, &str)] = &[
    ("gdi", include_str!("../spec/gdi.txt")),
    ("d3d8", include_str!("../spec/d3d8.txt")),
    ("d3d9", include_str!("../spec/d3d9.txt")),
    ("d3d9draw", include_str!("../spec/d3d9draw.txt")),
    ("d3dx9", include_str!("../spec/d3dx9.txt")),
    ("d3d11", include_str!("../spec/d3d11.txt")),
    ("wic", include_str!("../spec/wic.txt")),
    ("ddraw", include_str!("../spec/ddraw.txt")),
    ("opengl", include_str!("../spec/opengl.txt")),
    ("d3d7", include_str!("../spec/d3d7.txt")),
    ("gdiplus", include_str!("../spec/gdiplus.txt")),
    ("dwrite", include_str!("../spec/dwrite.txt")),
    ("d3d12", include_str!("../spec/d3d12.txt")),
    ("dxgi", include_str!("../spec/dxgi.txt")),
    ("d3dres", include_str!("../spec/d3dres.txt")),
    ("gdiline", include_str!("../spec/gdiline.txt")),
    ("gdirgn", include_str!("../spec/gdirgn.txt")),
    ("gdixform", include_str!("../spec/gdixform.txt")),
    ("gdipath", include_str!("../spec/gdipath.txt")),
    ("gditext", include_str!("../spec/gditext.txt")),
    ("gdiblend", include_str!("../spec/gdiblend.txt")),
    ("gdiddb", include_str!("../spec/gdiddb.txt")),
    ("d2d", include_str!("../spec/d2d.txt")),
    ("wicimage", include_str!("../spec/wicimage.txt")),
    ("ntstr", include_str!("../spec/ntstr.txt")),
    ("ntcase", include_str!("../spec/ntcase.txt")),
];

/// (excluded: deliberately not asked, e.g. calls that read past Windows' own arrays)
pub const STATUSES: &[&str] = &["doc", "measured", "assumed", "open", "excluded"];

pub fn clauses() -> Vec<Clause> {
    let mut v = Vec::new();
    for (file, text) in FILES {
        for l in text.lines() {
            if l.starts_with('#') || l.trim().is_empty() {
                continue;
            }
            let mut parts = l.splitn(3, " | ");
            let head = parts.next().unwrap_or("");
            let (id, status) = head.split_once(' ').unwrap_or((head, ""));
            v.push(Clause { id: id.into(), status: status.trim().into(), source: parts.next().unwrap_or("").into(), text: parts.next().unwrap_or("").into(), file });
        }
    }
    v
}

/// a clause's name: only the constants of `clause` (made from spec/ by build.rs) are ones
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClauseId(&'static str);

/// an open or excluded clause's name: only the constants of `open` are ones
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OpenId(&'static str);

impl ClauseId {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}
impl OpenId {
    pub fn as_str(self) -> &'static str {
        self.0
    }
}
impl From<OpenId> for ClauseId {
    fn from(o: OpenId) -> ClauseId {
        ClauseId(o.0)
    }
}
impl std::fmt::Display for ClauseId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::fmt::Display for OpenId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

include!(concat!(env!("OUT_DIR"), "/clauses.rs"));

thread_local! {
    static HITS: RefCell<BTreeSet<&'static str>> = const { RefCell::new(BTreeSet::new()) };
}

/// the model applies clause `id`
pub fn hit(id: ClauseId) {
    let id = id.0;
    // (Kani's compiler cannot take the thread-local set; the proofs need no coverage)
    #[cfg(not(kani))]
    HITS.with(|h| h.borrow_mut().insert(id));
    #[cfg(kani)]
    let _ = id;
}

/// the clauses hit since the last call
pub fn take() -> BTreeSet<&'static str> {
    HITS.with(|h| std::mem::take(&mut *h.borrow_mut()))
}

/// every suite's cases through the models: clause -> the cases ("suite:case") that reach it
pub fn coverage() -> std::collections::BTreeMap<String, Vec<String>> {
    let mut m: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (s, generate) in crate::generators::GENERATORS {
        for c in crate::case::parse_cases(&generate()) {
            for arch in [32, 64] {
                crate::model::set_arch(arch);
                take();
                let _ = crate::model::run_case(&c);
                for id in take() {
                    let v = m.entry(id.to_string()).or_default();
                    let n = format!("{s}:{}", c.name);
                    if v.last() != Some(&n) {
                        v.push(n);
                    }
                }
            }
        }
    }
    crate::model::set_arch(32);
    m
}
