//! Ids and names written in the source that the tables do not have do not build: a DXGI format
//! by `known::<ID>()` / `fmts!`, a WIC format by `wic_fmt!`.
#[test]
fn unknown_table_entries_do_not_build() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/unknown_format.rs");
    t.compile_fail("tests/ui/unknown_wic_format.rs");
}
