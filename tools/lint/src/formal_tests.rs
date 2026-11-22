use super::*;

#[test]
fn untrusted_proofs_are_found() {
    for (code, word) in [
        ("theorem t : 1 = 2 := by sorry", "sorry"),
        ("theorem t : 1 = 2 := by\n  admit", "admit"),
        ("axiom magic : False", "axiom"),
        ("opaque f : Nat → Nat", "opaque"),
        ("unsafe def f := 1", "unsafe"),
        ("@[implemented_by g] def f := 1", "implemented_by"),
        ("@[extern \"c_f\"] def f := 1", "extern"),
        ("theorem t : 2 + 2 = 4 := by native_decide", "native_decide"),
        ("theorem t (x : BitVec 8) : x &&& x = x := by bv_decide", "bv_decide"),
        ("theorem t : True := Lean.ofReduceBool _ _ rfl", "ofReduceBool"),
        ("set_option debug.skipKernelTC true", "skipKernelTC"),
    ] {
        let r = lean("spec/lean/X.lean", code);
        assert!(r.iter().any(|b| b.contains(&format!("`{word}`"))), "{code}: {r:?}");
        assert!(r[0].starts_with("spec/lean/X.lean:"), "{r:?}");
    }
}

#[test]
fn words_in_comments_strings_and_other_names_are_not() {
    let ok = "/-! no sorry here, nor an axiom /- nested admit -/ still a comment -/\n-- native_decide is not used\ndef s := \"sorry\"\ntheorem my_axiom : True := trivial\ndef Foo.sorry' := 1\ntheorem t : 1 + 1 = 2 := by decide";
    assert!(lean("spec/lean/X.lean", ok).is_empty(), "{:?}", lean("spec/lean/X.lean", ok));
    // (the line numbers stay those of the file)
    let r = lean("spec/lean/X.lean", "/- a\nb -/\n\ntheorem t : False := by sorry");
    assert_eq!(r, vec!["spec/lean/X.lean:4: lean-trust: `sorry`: a proof left open".to_string()]);
}

#[test]
fn lakefile_keeps_warnings_errors() {
    assert!(lakefile("l", "[leanOptions]\nwarningAsError = true\n").is_empty());
    assert!(!lakefile("l", "[leanOptions]\nwarningAsError = false\n").is_empty());
    assert!(!lakefile("l", "[leanOptions]\n# warningAsError = true\n").is_empty());
    assert!(!lakefile("l", "[leanOptions]\nautoImplicit = false\n").is_empty());
}
