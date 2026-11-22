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
    // (a stage argument's modifiers only in stage_arg, which d3d9.arg holds to the Lean)
    let arg = "pub fn stage_arg(a: u32) -> V { if a & 0x20 != 0 { v = [v[3]; 4]; } v }\nfn arg(&self, a: u32) -> V { if a & 0x20 != 0 { v = [v[3]; 4]; } v }\n";
    let r = rust("src/model/d3d9draw.rs", arg);
    assert!(r.len() == 2 && r.iter().all(|e| e.contains("in arg")), "{r:?}");
    // (CheckDeviceFormat's codes only in check_hr, which d3d9.check holds to the Lean)
    let check = "pub fn check_hr(f: u32) -> u32 { 0x8876_086a }\nfn draw_op(&mut self) { out.push(format!(\"{:08x}\", 0x0876_086f)); }\n";
    let r = rust("src/model/d3d9draw.rs", check);
    assert!(r.len() == 1 && r[0].contains("in draw_op"), "{r:?}");
    // (the shading arithmetic only in grad9, plane10)
    let r = rust("src/model/d3d9draw.rs", "fn grad9(x: X) -> f32 { d1 * (y[2] - y[0]) }\nfn draw(&self) { let g = d1 * (y[2] - y[0]); }\n");
    assert!(r.len() == 1 && r[0].contains("in draw"), "{r:?}");
    // (a point's pixel only in point_pixel, a pixel's packing only in pack)
    let r = rust("src/model/d3d9draw.rs", "fn point_pixel(x: f32) -> i64 { (x - 0.5).ceil() as i64 }\nfn points(&self) { let px = (x - 0.5).ceil() as i64; }\n");
    assert!(r.len() == 1 && r[0].contains("in points"), "{r:?}");
    assert!(!rust("src/model/d3d9draw.rs", "fn encode_x(&self) { ch.iter().filter(|c| c.c != b'x' || x_alpha); }").is_empty());
    // (the perspective weights only in rhw_attrs, rhw_value; the divide by w only in transform_xyz,
    // xyz_vertex)
    let r = rust("src/model/d3d9draw.rs", "fn rhw_attrs(at: A) -> A { at[k] * at[11] }\nfn triangle(&mut self) { at[k] *= at[11]; }\n");
    assert!(r.len() == 1 && r[0].contains("in triangle"), "{r:?}");
    assert!(!rust("src/model/d3d9draw.rs", "fn triangle(&mut self) { let rhw = 1.0 / q[3]; }").is_empty());
    // (the footprint's root only in aniso_footprint, the sphere map only in tex_gen)
    let r = rust("src/model/d3d9draw.rs", "fn aniso_footprint(a: A) -> f32 { m2.sqrt() }\nfn sample(&self) { let m = m2.sqrt(); }\n");
    assert!(r.len() == 1 && r[0].contains("in sample"), "{r:?}");
    assert!(!rust("src/model/d3d9draw.rs", "fn vertices(&self) { let m = (r[2] - 1.0) * (r[2] - 1.0); }").is_empty());
    // (the FVF and the declaration only through set_fvf, set_decl)
    assert!(!rust("src/model/d3d9draw.rs", "fn op(&mut self) { self.draw.fvf = 0; }").is_empty());
    assert!(rust("src/model/d3d9draw.rs", "fn op(&mut self) { (self.draw.fvf, self.draw.decl, self.draw.decl_from) = set_decl(e); if self.draw.fvf == 0 {} }").is_empty());
    // (the draw's checks, filters, palette texels, stencil states, DXT, a level's bytes, the stage
    // plan and the bump arithmetic each in their pure function, which the Lean mirrors hold)
    let inlined = [
        ("fn draw_op(&mut self) { if x86 && !(1..=5).contains(&op) { return; } }", "draw_gate"),
        ("fn sample(&self) { let m = TexFilter::of(self.samp(st, 5), self.v8); }", "sample_filter"),
        ("fn pixel(&mut self) { let s = if ccw { [186, 187, 188, 189] } else { [53, 54, 55, 56] }; }", "palette_texel"),
        ("fn dxt_texel(n: u32) -> u32 { (171 * a + 85 * b) >> 8 }", "dxt_mix"),
        ("fn fill_bytes(w: usize) -> usize { w.div_ceil(4) * b }", "create_level"),
        ("fn stages(&self) { let r = self.tss(st, 2) & 0xf == 2; }", "stage_plan"),
        ("fn stages(&self) { t[k] = ((c * l * 255.0).round_ties_even() / 255.0) as f64; }", "bump_scale"),
        ("fn blend(&self) { let s = s.map(|x| x.clamp(0.0, 1.0) as f32); let f = s[3].min(1.0 - d[3]); }", "blend_float"),
        ("fn sample(&self) { let near64 = |x: f64| ((x * 64.0).round() / 64.0).floor() as i64; }", "point_at"),
        ("fn bilinear_sum(&self) { let fx = ((x - x0) * q as f64).floor() as i64; let w = (q - fx) * (q - fy); }", "bilinear_at"),
        ("fn triangle(&mut self) { if !slots.contains(&k) { slots.push(k); } }", "tci_slots"),
        ("fn triangle(&mut self) { let r = if set < ntex { t } else { [0.0, 0.0, 1.0, 0.0] }; }", "coord_row"),
        ("fn ttf(&self) -> [f64; 3] { if ttf & 0x100 != 0 { [t[0], t[1], t[count - 1]] } else { t } }", "tex_transform"),
    ];
    for (code, only) in inlined {
        let r = rust("src/model/d3d9draw.rs", code);
        assert!(!r.is_empty() && r.iter().all(|e| e.contains("confined") && e.contains(only)), "{code}: {r:?}");
    }
    let pure = "pub fn draw_gate(x86: bool, op: u32) -> bool { x86 && !(1..=5).contains(&op) }\npub fn dxt_mix(a: u32, b: u32) -> u32 { (171 * a + 85 * b) >> 8 }\npub fn stage_plan(a: u32) -> bool { a & 0xf == 2 }\npub fn bump_scale(c: f32) -> f32 { (c * 255.0).round_ties_even() }\n";
    assert!(rust("src/model/d3d9draw.rs", pure).is_empty());
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
fn decimal_ellipse_insets_are_found() {
    // (the model before ellipsebig-*: the inset from a decimal, the centre from the ends' sum)
    let old = "fn ellipse_points() {\n    let (cx, cy) = ((fl + fr) / 2, (ft + fb) / 2);\n    let inset = |rad2: i32| f64::from(rad2) * 0.5 * (1.0 - 0.552_284_749_8);\n}\n";
    let r = rust("src/model/gdipath.rs", old);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: Ellipse's and RoundRect's inset")).count(), 3, "{r:?}");
    assert!(rust("src/model/gdipath.rs", "fn ellipse_points() {\n    // (not 0.5522847498)\n    let (cx, cy) = (fl.wrapping_add(vx / 2), ft.wrapping_add(vy / 2));\n    let dx = inset_floor(vx, 6);\n}\n").is_empty());
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
fn footprint_sums_past_32_bits_are_found() {
    let old = "fn layout() {\n    for i in 0..n {\n        total = at + pitch * (rows - 1) + row;\n        at = align(at + pitch * rows, 512);\n    }\n}\n";
    let r = rust("src/model/d3d12.rs", old);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: a subresource's bytes")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3d12.rs", "fn layout() { total = at + Bytes32::of(pitch * (rows - 1) + row).get(); at = align(at + Bytes32::of(pitch * rows).get(), 512); }").is_empty());
    let unchecked = "fn validate() {\n    no(d.kind() != Dim::Tex2D && (d.w > wmax || d.h > hmax), SIZE)?;\n}\n";
    assert!(rust("src/model/d3d12.rs", unchecked).iter().any(|b| b.contains("forbidden: GetCopyableFootprints checks a 2D")));
    assert!(rust("src/model/d3d12.rs", "fn validate() { no(d.w > size_max(d) || d.h > size_max(d), SIZE)?; }").is_empty());
    assert!(rust("src/lean/d3d12.rs", "const FP_SIZES: [i64; 2] = [0, 1];").iter().any(|b| b.contains("forbidden: the footprints mirror")));
    assert!(rust("src/lean/d3d12.rs", "const SIZES: [i64; 2] = [0, 0xffff_ffff];").is_empty());
}

#[test]
fn d3d9_rules_stay_in_the_mirrored_functions() {
    let inlined = "fn blend(&self) {\n    let c = if self.v8 { 0xffff_ffff } else { self.rs(193) };\n    let op = Self::blend_op(op);\n}\n";
    let r = rust("src/model/d3d9draw.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: the blend color, blend operation")).count(), 2, "{r:?}");
    let raster = "fn cover() {\n    let e = (q.0 - p.0) * (sy - p.1) - (q.1 - p.1) * (sx - p.0);\n}\n";
    assert!(rust("src/model/d3d9draw.rs", raster).iter().any(|b| b.contains("forbidden: the blend color, blend operation")));
    assert!(rust("src/model/d3d9draw.rs", "fn blend(&self) { let c = blend_color(self.v8, self.rs(193)); let op = blend_op(op); }").is_empty());
}

#[test]
fn stage_formulas_stay_in_stage_value() {
    let inlined = "fn stage(&self) {\n    let v = match op { 4 => a1[i] * a2[i], 7 => a1[i] + a2[i], _ => 0.0 };\n}\n";
    let r = rust("src/model/d3d9draw.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: the stage operations' formulas")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3d9draw.rs", "fn stage_value(x: StageIn) -> f32 { a1 * a2 + a1a }").is_empty());
}

#[test]
fn fog_stays_in_its_functions() {
    let inlined = "fn pixel(&mut self) {\n    let f = (-(z * density)).exp();\n    c[k] = (f * c[k] as f32 + (1.0 - f) * fc[k] as f32) as f64;\n}\n";
    let r = rust("src/model/d3d9draw.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: the fog factor and blend")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3d9draw.rs", "fn pixel(&mut self) { let f = fog_factor(m, z, sa, (s, e, d))?; c[k] = f64::from(fog_blend(f, c, g)); }").is_empty());
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

#[test]
fn apis_kani_cannot_compile_are_found() {
    let src = vec![("src/generators/gdipath.rs".to_string(), "fn f() { let v = Vec::<_>::from_fn(3, |k| k); }".to_string())];
    let r = kani_gates(&src, "pub mod calls;\n");
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].contains("feature(vec_from_fn)"), "{r:?}");
    assert!(kani_gates(&src, "#![cfg_attr(kani, feature(vec_from_fn))]\npub mod calls;\n").is_empty());
    // (in a comment is no use, nor is the gate in a comment)
    assert!(kani_gates(&[("src/x.rs".into(), "// Vec::from_fn(3, f)".into())], "").is_empty());
    assert_eq!(kani_gates(&src, "// #![cfg_attr(kani, feature(vec_from_fn))]\n").len(), 1);
    // (one gate naming several features; a feature only a prefix of the one named is not on)
    let both = vec![src[0].clone(), ("src/lean/ledger.rs".to_string(), "fn f(l: &str) -> &str { l.trim_prefix(\"def \") }".to_string())];
    assert!(kani_gates(&both, "#![cfg_attr(kani, feature(vec_from_fn, trim_prefix_suffix))]\n").is_empty());
    let r = kani_gates(&both, "#![cfg_attr(kani, feature(vec_from_fn, trim_prefix))]\n");
    assert!(r.len() == 1 && r[0].contains("feature(trim_prefix_suffix)"), "{r:?}");
}

#[test]
fn the_partial_calls_check_as_ci_condition_is_found() {
    let old = "fn recordings_match_the_cases() {\n    let c = kikizake::calls::check_dir(&root.join(\"calls\"), &root);\n}\n";
    assert!(rust("tests/calls.rs", old).iter().any(|b| b.contains("forbidden: the whole set's condition")));
    assert!(rust("tests/calls.rs", "fn r() { let c = kikizake::calls::check_complete(&d, &root, &req); }").is_empty());
}

#[test]
fn parsers_slicing_strings_are_found() {
    let calls = "fn p(l: &str) {\n    let open = l.find('(')?;\n    let close = l.rfind(\") = \")?;\n    let a = &l[open + 1..close];\n    let n = &l[..open];\n}\n";
    assert_eq!(rust("src/calls.rs", calls).iter().filter(|b| b.contains("forbidden: a call line is split")).count(), 2);
    let nt = "fn hex(t: &str) -> u8 { u8::from_str_radix(&t[0..2], 16).unwrap_or(0) }";
    assert!(rust("src/model/ntstr.rs", nt).iter().any(|b| b.contains("forbidden: hex bytes")));
    assert!(rust("src/calls.rs", "fn p(l: &str) { let (name, rest) = l.split_once('(')?; }").is_empty());
}

#[test]
fn gdi_edges_inline_in_op_is_found() {
    assert_eq!(rust("src/model/dwrite.rs", "fn op() { let e = to.whole(advp); }").len(), 1);
    assert_eq!(rust("src/model/dwrite.rs", "fn op() { let t = ToEm::new(ppem); }").len(), 1);
    assert_eq!(rust("src/model/dwrite.rs", "fn op() { let v = down6(x); }").len(), 1);
    assert!(rust("src/model/dwrite.rs", "fn op() { *g = gdi_edges(ppem, upm, s, c, px); }").is_empty());
    assert!(rust("src/model/dwrite.rs", "fn gdi_edges() { let e = to.whole(advp); }").is_empty());
}

#[test]
fn load_channels_inline_are_found() {
    assert_eq!(rust("src/model/d3dx9.rs", "fn load_encode() { let q = round_even(v * m); }").len(), 1);
    assert_eq!(rust("src/model/d3dx9.rs", "fn load_encode() { let l = 0.2125 * r; }").len(), 1);
    assert_eq!(rust("src/model/d3dx9.rs", "fn load_encode() { let v = v.clamp(-1.0, 1.0); }").len(), 1);
    assert!(rust("src/model/d3dx9.rs", "fn load_encode() { let q = unorm_write(v, ch.bits, x64); }").is_empty());
    assert_eq!(rust("src/model/d3dx9.rs", "fn load_decode() { let v = raw as f32 * (1.0 / m); }").len(), 1);
    assert!(rust("src/model/d3dx9.rs", "fn load_decode() { let v = unorm_read(raw, c.bits); }").is_empty());
    assert!(rust("src/model/d3dx9.rs", "fn unorm_read() { let v = raw as f32 * (1.0 / m); }").is_empty());
}

#[test]
fn optimize_rgb_testing_the_dot_below_zero_is_found() {
    let old = "fn optimize_rgb() {\n    let i = if !F::X87 && dot <= F::of(0.0) {\n        0\n    } else {\n        t\n    };\n}\n";
    let r = rust("src/model/dxtenc.rs", old);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("src/model/dxtenc.rs:2: forbidden in optimize_rgb: neither build"), "{r:?}");
    assert_eq!(rust("src/model/dxtenc.rs", "fn optimize_rgb() { if dot < F::of(0) { } }").len(), 1);
    assert!(rust("src/model/dxtenc.rs", "fn optimize_rgb() { if !(fsteps > dot) { n - 1 } else { t } }").is_empty());
    assert_eq!(rust("src/model/dxtenc.rs", "fn optimize_rgb() { let i = (dot + h).store().trunc_i(); }").len(), 1);
    assert!(rust("src/model/dxtenc.rs", "fn optimize_rgb() { let i = step_index(dot, fsteps, n); }").is_empty());
    // (the alpha fit and the indices of encode_bc1 do test it)
    assert!(rust("src/model/dxtenc.rs", "fn optimize_alpha() { let i = if dot <= F::of(0.0) { 0 } else { t }; }").is_empty());
}

#[test]
fn d3d11_rules_stay_in_the_mirrored_functions() {
    let inlined = "fn tri() {\n    if d.rast[1] == 2 && front {}\n    let top_left = |a, b| a;\n    let pass = match f { 2 => a < s, _ => true };\n    let l = (a * (256 - wv) + c * wv) >> 1;\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: culling, the scissor test")).count(), 4, "{r:?}");
    let point = "fn point() {\n    let px = ((s.x - 1).div_euclid(256), 0);\n    if px >= d.scissor[0] {}\n}\n";
    assert_eq!(rust("src/model/d3draster.rs", point).iter().filter(|b| b.contains("forbidden: culling, the scissor test")).count(), 2);
    assert!(rust("src/model/d3draster.rs", "fn tri() { if rules::culls(d.rast[1], d.rast[2] != 0, cw) {} let px = (rules::point_pixel(s.x), 0); }").is_empty());
}

#[test]
fn conversions_inlined_past_their_proven_functions_are_found() {
    let blk = "fn decode_block() {\n    let t = (171 * a[k] + 85 * z[k]) >> 8;\n    let a = ((8 - k) * a0 + (k - 1) * a1 + 3) / 7;\n}\n";
    assert_eq!(rust("src/model/dxgiconv.rs", blk).iter().filter(|b| b.contains("forbidden in decode_block")).count(), 2);
    assert!(rust("src/model/dxgiconv.rs", "fn decode_block() { let c = bc1_codes(c0, c1, idx, four); }").is_empty());
    let enc = "fn encode() {\n    let t = (v as f64).trunc();\n}\n";
    assert_eq!(rust("src/model/dxgiconv.rs", enc).iter().filter(|b| b.contains("forbidden in encode")).count(), 1);
    assert!(rust("src/model/dxgiconv.rs", "fn int_clear() { let t = (v as f64).trunc(); }").is_empty());
    let dsv = "fn clear_depth() {\n    let d = to_unorm(raw.clamp(0.0, 1.0), 24);\n}\n";
    assert_eq!(rust("src/model/d3d12.rs", dsv).iter().filter(|b| b.contains("forbidden in clear_depth")).count(), 2);
    assert!(rust("src/model/d3d12.rs", "fn clear_depth() { let d = unorm_depth(depth, 24); }").is_empty());
    assert_eq!(rust("src/model/bptc.rs", "fn bc6h() { let m = BC6H_MODES.iter().position(|x| x.0 == m); }").len(), 1);
    assert_eq!(rust("src/model/bptc.rs", "fn bc7() { let m = (0..8).find(|_| s.take(1) == 1); }").len(), 1);
    assert!(rust("src/model/bptc.rs", "fn bc7() { let m = bc7_mode(b[0]); }").is_empty());
    assert_eq!(rust("src/model/d3dx9.rs", "fn yuy2_pair() { let y = 16.0 + q((65.481 * r + 128.553 * g) / 255.0); }").len(), 1);
    assert!(rust("src/model/d3dx9.rs", "fn yuy2_pair() { yuy2_bytes(c0, c1) }").is_empty());
    assert_eq!(rust("src/model/d3dx9.rs", "fn yuv_d3dx() {\n    let s = t.wrapping_sub(a);\n    let c = y * 0.00456621;\n}\n").len(), 2);
    assert!(rust("src/model/d3dx9.rs", "fn yuv_d3dx() { yuv_rgb(y, u, v) }").is_empty());
    assert_eq!(rust("src/model/d3dx9.rs", "fn dxt_d3dx() { let c = a + (b - a) * (1.0 / 3.0); }").len(), 1);
    assert!(rust("src/model/d3dx9.rs", "fn dxt_d3dx() { let c = dxt_colour(c0, c1, idx); }").is_empty());
}

#[test]
fn gdiplus_mixes_stay_in_the_mirrored_functions() {
    let inlined = "fn paint() {\n    let v = (f * wt + g * (256 - wt)) >> 8;\n    let s = (2 * (c0 >> (8 * k) & 255) * c + full) / (2 * full);\n}\n";
    let r = rust("src/model/gdiplus.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: a hatch texel's mix")).count(), 2, "{r:?}");
    assert!(rust("src/model/gdiplus.rs", "fn paint() { let v = hatch_mix(f, g, wt); let s = coverage_share(c0 >> (8 * k) & 255, c, full); }").is_empty());
}

#[test]
fn gdiplus_gamma64_stays_in_the_mirrored_function() {
    let inlined = "fn mix() {\n    let w = rd.add(rd.mul(rd.add(rd.div(s as f32, 8192.0), rd.mul(rd.div(d, 8192.0), self.rest)), 8192.0), 0.5);\n}\nfn new() {\n    let rest = rd.sub(1.0, rd.div(sa, 8192.0));\n}\n";
    let r = rust("src/model/gdiplus.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in mix: x86-64's gamma corrected word")).count(), 4, "{r:?}");
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in new: a gamma corrected source's rest")).count(), 1, "{r:?}");
    assert!(rust("src/model/gdiplus.rs", "fn mix() { let w = mix64(rounding(), s, d, self.rest); }\nfn new() { let rest = gamma_rest(rd, sa); }\n").is_empty());
}

#[test]
fn gdiplus_dither_place_stays_in_the_mirrored_functions() {
    let inlined = "fn fill_area() {\n    let odd_end = (a1 - a0) % 2 == 1;\n    let c = (at + left - ox).rem_euclid(4) as u32;\n}\n";
    let r = rust("src/model/gdiplus.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in fill_area: a gamma corrected texture's dither place")).count(), 2, "{r:?}");
    assert!(rust("src/model/gdiplus.rs", "fn fill_area() { let c = dither_column(x86, ox, &cells, x0, x1, x); let r = dither_row(oy, y); }\n").is_empty());
}

#[test]
fn check_with_reads_the_format_tables_only_through_its_facts() {
    let inlined = "fn check_with() {\n    let s = group_support(dev, d.fmt);\n    let q = support(dev, d.fmt).5[0];\n}\n";
    let r = rust("src/model/d3dres_check.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in check_with: check_with takes")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3dres_check.rs", "fn facts() { let s = group_support(dev, d.fmt); }\nfn check_with() { let s = fx.s; }\n").is_empty());
}

#[test]
fn check_view_with_reads_the_format_tables_only_through_its_facts() {
    let inlined = "fn check_view_with() {\n    let s = support(dev, f).2;\n    let t = super::dxgifmt::Num::Typeless;\n}\n";
    let r = rust("src/model/d3dres_check.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in check_view_with: check_view_with takes")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3dres_check.rs", "fn view_facts() { let s = support(dev, f).2; }\nfn check_view_with() { let s = fx.s; }\n").is_empty());
}

#[test]
fn box_rules_stay_in_the_mirrored_functions() {
    let inlined = "fn update() {\n    if sub >= res.nsubs() {\n        hit(clause::D3DRES_UPDATE_IGNORED);\n    }\n    let r = x[3].div_ceil(bw);\n}\nfn copy_region() {\n    hit(clause::D3DRES_COPYSUB_BAD);\n}\n";
    let r = rust("src/model/d3dres.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && (b.contains(" in update") || b.contains(" in copy_region"))).count(), 3, "{r:?}");
    assert!(rust("src/model/d3dres.rs", "fn update_plan() { hit(clause::D3DRES_UPDATE_IGNORED); }\nfn box_elems() { g(3).div_ceil(bw) }\nfn region_plan() { hit(clause::D3DRES_COPYSUB_BAD); }\n").is_empty());
    let resolve = "fn run() {\n    if int {\n        hit(clause::D3DRES_RESOLVE_IGNORED);\n    }\n    let r = b[3].min(whole[0]);\n}\n";
    let r = rust("src/model/d3dres.rs", resolve);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in run")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3dres.rs", "fn resolve_plan() { hit(clause::D3DRES_RESOLVE_SIZE); }\nfn box_clip() { b[3].min(whole[0]) }\n").is_empty());
}

#[test]
fn filter_arithmetic_stays_in_the_mirrored_functions() {
    let inlined = "fn sample() {\n    let s = c * n as f32 - 0.5;\n    let f = (lod.fract() * 256.0).round();\n    if index(dw) <= px {}\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in sample")).count(), 3, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn split() { let s = c * n as f32 - 0.5; }\nfn level() { let w = (lod.fract() * 256.0).round(); }\n").is_empty());
}

#[test]
fn layout_stays_in_the_mirrored_function() {
    let inlined = "fn draw() {\n    let v = match name { \"bad\" => return Some(LayoutVerdict::NoProgram), _ => 2 };\n    if usable.len() >= needs {}\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in draw")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn layout_verdict() { match name { \"bad\" => return Some(LayoutVerdict::NoProgram), _ => 2 }; if usable.len() >= needs {} }\n").is_empty());
}

#[test]
fn lod_stays_in_the_mirrored_functions() {
    let inlined = "fn shade() {\n    let s = (du * w).powi(2);\n    if rho <= 1.0 {}\n    let d = [0x15, 3, 3, 3, 1];\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in shade")).count(), 3, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn quad_span() { (du * w).powi(2) }\nfn filter_pick() { if rho <= 1.0 {} }\nfn sampler_or_default() { [0x15, 3, 3, 3, 1] }\n").is_empty());
}

#[test]
fn lines_stay_in_the_mirrored_functions() {
    let inlined = "fn lines() {\n    let k = QUAD_HALF / len;\n    let a = s * f32::from_bits(0x3EE6_6666);\n    let k = 2.0 / len;\n    let y = (pb.y - pa.y).abs() > dx;\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in lines")).count(), 4, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn quad_corners() { QUAD_HALF / len }\nfn alpha_coverage() { f32::from_bits(0x3EE6_6666) }\nfn alpha_corners() { 2.0 / len }\nfn major_of() { (b.y - a.y).abs() > dx }\n").is_empty());
}

#[test]
fn clipping_stays_in_the_mirrored_functions() {
    let inlined = "fn triangle() {\n    let t = da / (da - db);\n    if zs.iter().all(|z| *z < 0.0) || far {}\n    if q.iter().any(|x| !x.is_finite()) {}\n    q.windows(2).skip(1).all(f);\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in triangle")).count(), 4, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn clip_xy() { let t = da / (da - db); }\nfn depth_away() { zs.iter().all(|z| *z < 0.0) || far }\nfn not_finite() { q.iter().any(|x| !x.is_finite()) }\nfn clipped_away() { q.windows(2).skip(1).all(f) }\n").is_empty());
}

#[test]
fn interpolation_stays_in_the_mirrored_functions() {
    let inlined = "fn triangle() {\n    let z = p.plane(p.zs, cx, cy).max(lo).min(hi);\n    let d = minz + q[2] * (1.0 / q[3]) * (maxz - minz);\n}\n";
    let r = rust("src/model/d3draster.rs", inlined);
    assert_eq!(r.iter().filter(|b| b.contains("confined:") && b.contains(" in triangle")).count(), 2, "{r:?}");
    assert!(rust("src/model/d3draster.rs", "fn clamp_range() { z.max(lo).min(hi) }\nfn vertex_depth() { minz + z * (1.0 / w) * (maxz - minz) }\n").is_empty());
}

#[test]
fn mirror_samples_from_the_32_bit_generator_are_found() {
    let old = "fn values(each: &mut dyn FnMut(&[i64])) {\n    let mut rng = crate::rng::XorShift32::new(1);\n    each(&[rng.range(0, i64::MAX)]);\n}\n";
    let r = rust("src/lean/gdiplus.rs", old);
    assert!(r.iter().any(|m| m.starts_with("src/lean/gdiplus.rs:2: lean-sampler")), "{r:?}");
    assert!(rust("src/lean/gdiplus.rs", "fn f() { let mut rng = crate::rng::Sample64::new(1); }").is_empty());
    // (the generators keep XorShift32: the goldens were made with its sequence)
    assert!(rust("src/generators/gdi.rs", old).is_empty());
}

#[test]
fn zips_in_tests_are_found() {
    // (the review's test: with every place line gone, the zip compared nothing and passed)
    let old = "fn placements() {\n    let places: Vec<&String> = out.iter().filter(|l| l.starts_with(\"place \")).collect();\n    for (em, l) in (1..=200u16).zip(places) {\n        assert_eq!(l, &want(em));\n    }\n}\n";
    let r = test_rust("tests/laws_dwrite_face.rs", old);
    assert_eq!(r.len(), 1, "{r:?}");
    assert!(r[0].starts_with("tests/laws_dwrite_face.rs:3: zip-unchecked"), "{r:?}");
    assert!(!test_rust("tests/x.rs", "fn f() { assert!(a.iter().zip(&b).all(|(x, y)| x == y)); }").is_empty());
    assert!(test_rust("tests/x.rs", "fn f() { for (em, l) in zip_eq(1..=200u16, places) {} }\n// a.zip(b) in a comment\n").is_empty());
}

#[test]
fn recording_names_read_apart_are_found() {
    let old = "pub fn check_dir() {\n    let suite = n.rsplit_once('-').map_or(n.as_str(), |(s, _)| s);\n}\n";
    assert!(rust("src/calls.rs", old).iter().any(|m| m.contains("forbidden in check_dir")), "{:?}", rust("src/calls.rs", old));
    assert!(rust("src/calls.rs", "impl RecName {\n    pub fn parse(s: &str) { let (suite, arch) = s.rsplit_once('-'); }\n}\n").is_empty());
}

#[test]
fn view_levels_read_apart_are_found() {
    // (the review's Load: level 0 of the resource, whatever the view's most detailed level)
    let old = "fn shade(m: &D3dres) -> Res<[u32; 4]> {\n    let (w, h, _) = r.size(0);\n    texel(r, 0, v.format(), px as u32, py as u32)?\n}\n";
    let r = rust("src/model/d3draster.rs", old);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden in shade")).count(), 2, "{r:?}");
    assert!(!rust("src/model/d3draster.rs", "fn shade() { let b = v.words.get(2); }\n").is_empty());
    assert!(rust("src/model/d3draster.rs", "fn shade() {\n    let reach = view_reach(dev, v, r);\n    let (w, h, _) = r.size(reach.mip);\n    texel(r, sub, v.format(), x, y)?\n}\n").is_empty());
}

#[test]
fn attribute_groups_with_a_rest_are_found() {
    let old = "fn pop_attrib(&mut self, mask: u32, s: Ctx) {\n    let Ctx { color, normal, .. } = s;\n}\n";
    assert!(rust("src/model/opengl.rs", old).iter().any(|b| b.contains("forbidden in pop_attrib")));
    assert!(rust("src/model/opengl.rs", "fn pop_attrib(&mut self, mask: u32, s: Ctx) {\n    let Ctx { color, normal } = s;\n}\n").is_empty());
    assert!(rust("src/model/opengl_list.rs", "pub fn pop(&mut self, saved: &Lists) { let Lists { base, .. } = saved; }").iter().any(|b| b.contains("forbidden in pop")));
    // (the readbacks of the review: zoom 1 and raster coordinates 0 whatever was set)
    let get = "fn get(&self, p: u32) -> Vec<V> {\n    match p {\n        0xd16 | 0xd17 => vec![F(1.0)],\n        0xb06 => vec![F(0.0), F(0.0), F(0.0), F(1.0)],\n    }\n}\n";
    assert_eq!(rust("src/model/opengl.rs", get).iter().filter(|b| b.contains("forbidden: glGet")).count(), 2);
    let color = "fn get(&self, p: u32) -> Vec<V> {\n    match p {\n        0xb04 => c4([1.0; 4]),\n        0xb05 => vec![F(1.0)],\n    }\n}\n";
    assert_eq!(rust("src/model/opengl.rs", color).iter().filter(|b| b.contains("forbidden: glGet")).count(), 2);
}

#[test]
fn options_read_token_by_token_are_found() {
    let old = "fn op() {\n    for o in &t[first..] {\n        match option(cont, o)? {\n        }\n    }\n    let e = Encoded::Decodes(\"wmp\", img);\n}\n";
    assert_eq!(rust("src/model/wicimage.rs", old).iter().filter(|b| b.contains("Bag::write")).count(), 3);
    assert!(rust("src/model/wicimage.rs", "fn op() {\n    let bag = Bag::write(cont, &t[first..])?;\n    for (o, _) in bag.last() {}\n}\n").is_empty());
}

#[test]
fn list_cuts_and_proxies_compiled_are_found() {
    let cut = "fn run() {\n    for k in 0..u32::try_from(range.min(1 << 20)).unwrap_or(0) {}\n}\n";
    assert!(rust("src/model/opengl_list.rs", cut).iter().any(|b| b.contains("glDeleteLists")));
    let old = "fn kind(t: &[String]) -> Kind {\n    match op {\n        \"glmat\" | \"glfogv\" | \"gldraw\" | \"gltex\" => Kind::Compiled,\n        _ => Kind::Immediate,\n    }\n}\n";
    assert!(rust("src/model/opengl_list.rs", old).iter().any(|b| b.contains("proxy")));
    let new = "fn kind(t: &[String]) -> Kind {\n    match op {\n        \"gltex\" if proxy(t) => Kind::Immediate,\n        \"glmat\" | \"glfogv\" | \"gldraw\" | \"gltex\" | \"gltexsub\" => Kind::Compiled,\n        _ => Kind::Immediate,\n    }\n}\n";
    assert!(rust("src/model/opengl_list.rs", new).is_empty(), "{:?}", rust("src/model/opengl_list.rs", new));
    assert!(rust("src/model/opengl_list.rs", "fn run() { ctx.lists.lists.retain(|&k, _| u64::from(k.wrapping_sub(first)) >= range); }\n").is_empty());
}

#[test]
fn float_shifts_past_31_are_found() {
    assert!(rust("src/model/jxr.rs", "fn float2pixel() {\n    m >>= 1 - e1;\n}\n").iter().any(|b| b.contains("wrapping_shr")));
    assert!(rust("src/model/jxr.rs", "fn float2pixel() {\n    m = m.wrapping_shr(u32::try_from(1 - e1).unwrap_or(u32::MAX));\n}\n").is_empty());
}

#[test]
fn nodev_gates_apart_are_found() {
    let c = "static void op_a(void) {\n    if (!on12) { out(\"nodev\\n\"); return; }\n}\nstatic void op_more(void) {\n    keep();\n}\nstatic const struct kz_op ops[] = {\n    { \"a12\", op_a },\n    { \"more12\", op_more },\n    { 0 },\n};\n";
    let old = "fn op() {\n    match t0 {\n        \"a12\" | \"more12\" if !self.dev => {\n        }\n    }\n}\n";
    let b = nodev_gates("runner/x.c", c, "ops", "src/model/x.rs", old, "!self.dev");
    assert_eq!(b.len(), 1, "{b:?}");
    assert!(b[0].contains("more12"));
    let new = old.replace(" | \"more12\"", "");
    assert!(nodev_gates("runner/x.c", c, "ops", "src/model/x.rs", &new, "!self.dev").is_empty());
    let less = old.replace("\"a12\" | ", "");
    assert_eq!(nodev_gates("runner/x.c", c, "ops", "src/model/x.rs", &less, "!self.dev").len(), 2);
}

#[test]
fn view_slots_and_removed_views_keep_the_executors_order() {
    let old = "fn view() {\n    if kind == 3 && !dev.d3d11() {\n        out.push(\"uav none\".into());\n        return Ok(());\n    }\n    if !check {\n        self.views.empty(v);\n    }\n}\nfn op() {\n    match name {\n        \"read\" => {\n            return Ok(true);\n        }\n        \"tex\" | \"buf\" | \"srv\" | \"rtv\" if dev.d3d11() => {}\n    }\n}\n";
    let r = rust("src/model/d3dres.rs", old);
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: the executor empties the view slot")).count(), 1, "{r:?}");
    assert_eq!(r.iter().filter(|b| b.contains("forbidden: on a removed device a view of an empty slot")).count(), 1, "{r:?}");
    let new = "fn view() {\n    if !check {\n        self.views.empty(v);\n    }\n    if kind == 3 && !dev.d3d11() {\n        out.push(\"uav none\".into());\n        return Ok(());\n    }\n}\nfn op() {\n    match name {\n        \"read\" => {\n            return Ok(true);\n        }\n        \"srv\" | \"rtv\" if self.res.at(slot(t, 2)?).is_none() => {\n            return Ok(true);\n        }\n        \"tex\" | \"buf\" if dev.d3d11() => {}\n        \"srv\" | \"rtv\" if dev.d3d11() => {}\n    }\n}\n";
    assert!(rust("src/model/d3dres.rs", new).is_empty(), "{:?}", rust("src/model/d3dres.rs", new));
}

#[test]
fn current_sources_set_apart_are_found() {
    // (a source set without set_cur keeps the decoder's frame it replaced as its own)
    let old = "fn flipper(&mut self) {\n    self.cur = Some(flip(c, o));\n}\n";
    assert!(rust("src/model/wicimage.rs", old).iter().any(|b| b.contains("confined")), "{:?}", rust("src/model/wicimage.rs", old));
    assert!(rust("src/model/wicimage.rs", "fn flipper(&mut self) {\n    self.set_cur(flip(c, o));\n}\n").is_empty());
}

#[test]
fn wmp_frames_by_their_options_are_found() {
    let old = "fn op() {\n    match cont {\n        \"wmp\" if self.from == Some(\"wmp\") && bag.last().next().is_none() && nf == c.fmt => {}\n    }\n    let o = if rcw && i.fmt == \"gray8\" { a } else { b };\n}\n";
    assert_eq!(rust("src/model/wicimage.rs", old).iter().filter(|b| b.contains("turned()")).count(), 2);
    assert!(rust("src/model/wicimage.rs", "fn op() {\n    match cont {\n        \"wmp\" if self.from == Some(\"wmp\") && nf == c.fmt => {}\n    }\n}\n").is_empty());
}
