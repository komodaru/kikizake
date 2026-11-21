use super::*;

#[test]
fn model_reading_the_generators_is_found() {
    assert!(rust("src/model/x.rs", "fn f() { crate::generators::d3ddraw::shaders(); }")[0].contains("model-uses-generators"));
    assert!(rust("src/model/x.rs", "use crate::generators::dxbc;")[0].contains("model-uses-generators"));
    // (comments and strings do not count; outside src/model/ is fine)
    assert!(rust("src/model/x.rs", "// crate::generators::x\nfn f() { let _ = \"crate::generators\"; }").is_empty());
    assert!(rust("src/trace.rs", "fn f() { crate::generators::suite(\"x\"); }").is_empty());
    let three = "fn f() { crate::generators::a(); crate::generators::b(); crate::generators::c(); }";
    assert!(rust("src/model/wicimage.rs", three).is_empty());
    assert!(!rust("src/model/wicimage.rs", &format!("{three} fn g() {{ crate::generators::d(); }}")).is_empty());
}

#[test]
fn reads_turned_into_empty_are_found() {
    // (review 2.4: calls check of a missing directory said 0 problems)
    let dir = "fn f(d: &Path) -> Vec<String> {\n    let names: Vec<String> = std::fs::read_dir(d)\n        .map(|d| d.filter_map(|e| e.ok()?.file_name().into_string().ok()).collect())\n        .unwrap_or_default();\n    names\n}";
    let r = rust("src/calls.rs", dir);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("src/calls.rs:2: io-error-as-empty"), "{r:?}");
    assert!(!rust("src/x.rs", "fn f() { let t = std::fs::read_to_string(p).unwrap_or_else(|_| String::new()); }").is_empty());
    assert!(!rust("src/x.rs", "fn f() { let t = std::fs::read_to_string(p).ok(); }").is_empty());
    assert!(!rust("src/main.rs", "fn f() { let r: Vec<_> = std::fs::read_dir(d).into_iter().flatten().collect(); }").is_empty());
    // (an error passed on, reported before stopping, or a file said to be optional, is fine)
    assert!(rust("src/main.rs", "fn read(p: &str) -> String {\n    std::fs::read_to_string(p).unwrap_or_else(|e| {\n        eprintln!(\"{p}: {e}\");\n        exit(2)\n    })\n}").is_empty());
    assert!(rust("src/x.rs", "fn f() -> R { let t = std::fs::read_to_string(p)?; let n = t.parse().unwrap_or(0); Ok(()) }").is_empty());
    assert!(rust("src/x.rs", "fn f() {\n    // (optional: a list of exemptions)\n    let t = std::fs::read_to_string(p).unwrap_or_default();\n}").is_empty());
}

#[test]
fn default_disagreeing_with_new_is_found() {
    // (review 4.1: Gdi::default() made no slots, Gdi::new() eight)
    let gdi = "#[derive(Default)]\npub struct Gdi { pub slots: Vec<Option<u8>> }\nimpl Gdi {\n    pub fn new() -> Gdi {\n        Gdi { slots: vec![None; 8] }\n    }\n}";
    assert!(rust("src/model/gdi.rs", gdi)[0].contains("src/model/gdi.rs:4: default-beside-new"));
    let selfy = "#[derive(Clone, Default)]\nstruct G { a: u8 }\nimpl G {\n    fn new() -> Self { G { a: 1 } }\n}";
    assert!(!rust("src/x.rs", selfy).is_empty());
    assert!(rust("src/x.rs", "#[derive(Default)]\npub struct Gdi { s: [u8; 8] }\nimpl Gdi {\n    pub fn new() -> Gdi {\n        Self::default()\n    }\n}").is_empty());
    // (no derive: new is the only way)
    assert!(rust("src/x.rs", "pub struct H { s: Vec<u8> }\nimpl H {\n    pub fn new() -> H { H { s: vec![0; 8] } }\n}").is_empty());
}

#[test]
fn dib_access_before_flush_is_found() {
    // (review 3.4: poke wrote the bits with GDI's batch still pending)
    let poke = "static void op_poke(void) {\n    struct slot *s = slot(1);\n    unsigned char *p = row(s, y) + x;\n    *p = 1;\n}\n";
    assert!(c("runner/gdi.c", poke)[0].starts_with("runner/gdi.c:3: dib-without-flush: op_poke"));
    let fixed = "static void op_poke(void) {\n    struct slot *s = slot(1);\n    GdiFlush();\n    unsigned char *p = row(s, y) + x;\n}\n";
    assert!(c("runner/gdi.c", fixed).is_empty());
    let brush = "static void op_brush(void) {\n    /* GdiFlush() in a comment is none */\n    memcpy(b, p->bits, n);\n}\n";
    assert!(!c("runner/gdiddb.c", brush).is_empty());
    // (making the section, row itself, and other APIs' modules are fine)
    assert!(c("runner/gdi.c", "static void op_dib(void) {\n    s->bmp = CreateDIBSection(s->dc, bi, 0, (void **)&s->bits, NULL, 0);\n    memset(s->bits, 0, n);\n}\nstatic unsigned char *row(struct slot *s, int y) { return s->bits + y; }\n").is_empty());
    assert!(c("runner/d3d9.c", poke).is_empty());
    // (testing or setting the pointer touches no pixel)
    assert!(c("runner/gdi.c", "void gdi_adopt(long n) {\n    if (!s->bits) return;\n    s->bits = ds.dsBm.bmBits;\n}\n").is_empty());
    assert!(!c("runner/gdi.c", "static void op_fill(void) {\n    unsigned char *b = s->bits ? s->bits : malloc(n);\n}\n").is_empty());
}

#[test]
fn unchecked_waits_are_found() {
    // (review 3.5: the readback was read whatever the wait answered)
    let old = "void kz_d3d12_wait(void) {\n    ID3D12CommandQueue_Signal(queue12, fence12, ++fenced);\n    if (x) {\n        ID3D12Fence_SetEventOnCompletion(fence12, fenced, event12);\n        WaitForSingleObject(event12, 10000);\n    }\n}\n";
    let r = c("runner/d3d12.c", old);
    assert_eq!(r.len(), 3, "{r:?}");
    assert!(r[0].starts_with("runner/d3d12.c:2: wait-unchecked: the result of ID3D12CommandQueue_Signal"));
    assert!(c("runner/d3d12.c", "    HRESULT hr = ID3D12CommandQueue_Signal(q, f, 1);\n    if (WaitForSingleObject(e, 10) != WAIT_OBJECT_0) fail();\n").is_empty());
}

#[test]
fn the_repository_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    if let Err(e) = check(&root) {
        panic!("{e}");
    }
}

#[test]
fn every_pattern_compiles() {
    assert!(pats().is_some(), "{BROKEN}");
}

#[test]
fn unconfined_uses_are_found() {
    // (review of 8480456, 1.3: a blit wrote the heap's bytes into px as plain values)
    let dd = "impl Surf {\n    fn get(&self) -> Res<u32> { match self.px[0] { _ => 0 } }\n}\nfn extra(s: &mut Surf) {\n    s.px[3] = Px::Known(0x90);\n}\n";
    let r = rust("src/model/ddraw.rs", dd);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("src/model/ddraw.rs:5: confined: .px[ in extra"), "{r:?}");
    // (1.2: the load codecs picked x86's bulk routine apart from the array's)
    let f16 = "fn f16_sse(x: u32) -> u16 { 0 }\nfn half(x: u32) -> u16 { f16_sse(x) }\nfn f16_load(x: u32) -> u16 { f16_one(x) }\n";
    let r = rust("src/model/d3dx9.rs", f16);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("confined: f16_one( in f16_load"), "{r:?}");
    // (other files, comments and the definition itself are fine)
    assert!(rust("src/model/gdi.rs", dd).is_empty());
    assert!(rust("src/model/d3dx9.rs", "// f16_one(x) in a comment\nfn f16_one(x: u32) -> u16 { 0 }\n").is_empty());
}

#[test]
fn refuted_constructions_are_found() {
    let old = "fn encode() {\n    match chroma {\n        Chroma::Full => d,\n        Chroma::H1V2 => repeat(&d),\n    }\n}\n";
    let r = rust("src/model/jpeg.rs", old);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("src/model/jpeg.rs:4: forbidden"), "{r:?}");
    assert!(rust("src/model/jpeg.rs", "fn encode() { match c { Chroma::H1V2 => upsample_h1v2(&d, w, h), _ => repeat(&d) } }").is_empty());
}

#[test]
fn device12_dropping_the_chain_is_found() {
    let old = "fn op() {\n    match name {\n        \"device12\" => {\n            self.dev = true;\n            self.chain = None;\n        }\n    }\n}\n";
    let r = rust("src/model/d3d12.rs", old);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("forbidden: device12 again"), "{r:?}");
    assert!(rust("src/model/d3d12.rs", "fn op() { match name { \"device12\" => { self.dev = true; } \"dxgifactory\" => { self.chain = None; } } }").is_empty());
}

#[test]
fn silent_cuts_are_found() {
    // (review of 8480456, 3: rootsig12 kept the first 1024 tokens of its rootsig12+ lines and its own)
    let old = "static void op_rootsig(void) {\n    rt = 0;\n    for (int k = 0; k < nrsx && rt < RSTOK; k++) rtk[rt++] = rsx[k];\n}\n";
    let r = c("runner/d3d12.c", old);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("runner/d3d12.c:3: silent-cut: a copy stops at RSTOK"), "{r:?}");
    assert!(c("runner/d3d12.c", "    if (nrsx > RSTOK) { bad = 1; return; }\n    for (int k = 0; k < nrsx; k++) rtk[rt++] = rsx[k];\n").is_empty());
    assert!(c("runner/main.c", "for (char *t = strtok(l, \" \"); t && ntok < MAXTOK; t = strtok(NULL, \" \")) tok[ntok++] = t;").is_empty());
}

#[test]
fn unflushed_case_headers_are_found() {
    let old = "        if (incase) { reset_all(); oflush(); }\n        out(\"case %s\\n\", ntok > 1 ? tok[1] : \"?\");\n        incase = 1;\n";
    assert!(c("runner/main.c", old)[0].contains("header-unflushed"));
    let new = "        out(\"case %s\\n\", ntok > 1 ? tok[1] : \"?\");\n        /* (at once) */\n        oflush();\n";
    assert!(c("runner/main.c", new).is_empty());
    assert!(c("runner/gdi.c", old).is_empty());
}
