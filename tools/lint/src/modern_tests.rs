use super::*;

#[test]
fn wildcard_unreachable_is_found() {
    // (2026-11-21: the string routers of d3draster/d3dres/opengl/gdipath/wicimage ended so)
    let r = rust("src/model/x.rs", "fn f(s: &str) -> u8 {\n    match s {\n        \"a\" => 1,\n        _ => unreachable!(),\n    }\n}");
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("src/model/x.rs:4: wildcard-unreachable"), "{r:?}");
    assert!(!rust("src/x.rs", "match v { 0 => 1, _ => { unreachable!(\"no\") } }").is_empty());
    assert!(!rust("src/x.rs", "match v { 0 => 1, _ => std::unreachable!() }").is_empty());
    // (a named arm, a guard, a comment or a string is not one; nor a const-time panic)
    assert!(rust("src/x.rs", "match v { Kind::Never => unreachable!(), }").is_empty());
    assert!(rust("src/x.rs", "// `_ => unreachable!()` is gone\nlet s = \"_ => unreachable!()\";").is_empty());
    assert!(rust("src/x.rs", "const fn of(v: u8) -> W { match v { 0 => W::A, _ => panic!(\"past\") } }").is_empty());
    assert!(rust("src/x.rs", "let x_ => 1;").is_empty());
}

#[test]
fn hand_bit_width_is_found() {
    assert!(rust("src/x.rs", "let n = 32 - v.leading_zeros();")[0].contains("hand-bit-width"));
    assert!(!rust("src/x.rs", "let n = 64 - (a | b).leading_zeros() as usize;").is_empty());
    assert!(!rust("src/x.rs", "let n = 16 - self.w[3].leading_zeros();").is_empty());
    assert!(rust("src/x.rs", "let n = v.bit_width(); let z = v.leading_zeros(); let k = 31 - v.leading_zeros();").is_empty());
}

#[test]
fn hand_std_shapes_are_found() {
    // (2026-11-21: 1 trim_prefix, 7 map_or_default, 5 ok_or and 14 Vec::from_fn replaced)
    for bad in [
        "fn f(s: &mut S) { s.on = !s.on; }",
        "fn f(n: &str) -> &str { n.strip_prefix(\"x\").unwrap_or(n) }",
        "fn f(o: Option<u32>) -> String { o.map(|v| v.to_string()).unwrap_or_default() }",
        "fn f(o: Option<&str>) -> String { o.map_or(String::new(), |v| v.to_owned()) }",
        "fn f(ok: bool) -> Res<()> { if ok { Ok(()) } else { Err(Fail::Bad) } }",
        "fn f(v: &[u8]) -> Vec<u32> { (0..v.len()).map(|i| u32::from(v[i])).collect::<Vec<u32>>() }",
    ] {
        let r = rust("src/model/x.rs", bad);
        assert!(r.len() == 1 && r[0].contains("hand-std"), "{bad}: {r:?}");
    }
    for good in [
        "fn f(s: &mut S) { s.on.toggle(); s.a = !s.b; }",
        "fn f(n: &str, d: &str) -> &str { n.strip_prefix(\"x\").unwrap_or(d) }",
        "fn f(o: Option<u32>) -> String { o.map_or_default(|v| v.to_string()) }",
        "fn g(v: &[Option<u8>]) -> Vec<u8> { v.iter().map(|x| x.unwrap_or_default()).collect() }",
        "fn f(ok: bool) -> Res<u8> { if ok { Ok(1) } else { Err(Fail::Bad) } }",
        "fn f(n: u32) -> Vec<u32> { (0..n).map(|i| i * 2).collect::<Vec<u32>>() }",
    ] {
        assert!(rust("src/model/x.rs", good).is_empty(), "{good}");
    }
}

#[test]
fn binaries_warn_of_dead_pub() {
    let on = "//! x\n#![cfg_attr(not(test), warn(dead_code_pub_in_binary))]\nfn main() {}";
    assert!(rust("src/main.rs", on).is_empty());
    assert!(rust("src/main.rs", "#![deny(dead_code_pub_in_binary)]\nfn main() {}").is_empty());
    assert!(rust("src/main.rs", "fn main() {}")[0].contains("bin-dead-pub"));
    assert!(!rust("src/main.rs", "#![allow(dead_code_pub_in_binary)]\nfn main() {}").is_empty());
    assert!(!rust("src/main.rs", "// #![warn(dead_code_pub_in_binary)]\nfn main() {}").is_empty());
    // (a library module needs none)
    assert!(rust("src/lib.rs", "pub fn f() {}").is_empty());
}

#[test]
fn manifests_and_config_are_checked() {
    assert!(manifest("Cargo.toml", "[lints.rust]\nraw_borrows_via_references = \"warn\"\n").is_empty());
    assert!(manifest("Cargo.toml", "[lints.rust]\nraw_borrows_via_references = \"deny\" # x\n").is_empty());
    assert!(!manifest("Cargo.toml", "[lints.rust]\nraw_borrows_via_references = \"allow\"\n").is_empty());
    assert!(!manifest("Cargo.toml", "[lints.rust]\n# raw_borrows_via_references = \"warn\"\n").is_empty());
    assert!(cargo_config("[registry]\nglobal-min-publish-age = \"7 days\"\n").is_empty());
    assert!(!cargo_config("[registry]\n# global-min-publish-age = \"7 days\"\n").is_empty());
}

#[test]
fn the_repository_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Err(e) = check(&root) {
        panic!("{e}");
    }
}
