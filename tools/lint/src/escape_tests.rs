//! escape-comment word by word: each escape alone is found, each Windows answer with its
//! evidence lets through, a known site covers as many as it is listed.
use super::*;

#[test]
fn each_escape_word_is_found_alone() {
    let words = [
        concat!("TO", "DO: later"), concat!("FIX", "ME"), concat!("HA", "CK around"), "zero for now", "a simplified model",
        "not implemented", "curves are not modelled yet", "not ported", "best effort", "good enough", "stubbed out",
        "a stub returning 0", "a workaround", "compared with Windows only for now", "とりあえず", "簡略化", "未実装", "近似",
        "not supported", "the fitted approximations of open clauses", "(flags ignored)",
    ];
    for w in words {
        assert!(is_escape(w, w), "{w:?} is not found");
    }
}

#[test]
fn windows_answers_with_evidence_pass() {
    for w in [
        "binary labels: not supported, as on Windows",
        "(MEASURED: flags other than DXGI_CREATE_FACTORY_DEBUG are ignored)",
        "D2D1_GEOMETRY_SIMPLIFICATION_OPTION_LINES",
        "Simplify (no transform; tolerance 0.25 when not given)",
        "GetSize fails WINCODEC_ERR_UNSUPPORTEDPIXELFORMAT (0x88982f80)",
    ] {
        assert!(!is_escape(w, w), "{w:?} is flagged");
    }
}

#[test]
fn comments_strings_and_known_sites() {
    let files = vec![("src/model/x.rs".to_owned(), "// for now\nfn f() { eprintln!(\"not implemented\"); }\n".to_owned()), ("runner/x.c".to_owned(), "/* stubbed */\n".to_owned())];
    assert_eq!(fresh(&files, "").len(), 3);
    assert_eq!(fresh(&files, "src/model/x.rs\t// for now\twhy\n").len(), 2);
    // (one listed line covers one site, not two)
    let two = vec![("src/a.rs".to_owned(), "// for now\n// for now\n".to_owned())];
    assert_eq!(fresh(&two, "src/a.rs\t// for now\twhy\n").len(), 1);
}

#[test]
fn the_repository_passes() {
    assert_eq!(check(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")), Ok(()));
}
