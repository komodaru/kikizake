//! Checks that keep the 2026-11-20 review's mistakes from coming back (absolute: no baseline).
//!
//! * `model-uses-generators` (review 4.3): a model (src/model/) reads nothing of the generators
//!   (src/generators/): what both need lives in a neutral module (src/dxbc.rs, src/shaders.rs,
//!   src/resource_desc.rs), so the cases and the answers they are checked against do not share
//!   one implementation. The sites left are counted per file in `MODEL_USES_GENERATORS` and
//!   only go down.
//! * `io-error-as-empty` (review 2.2, 2.4): a file or directory that cannot be read is not an
//!   empty one: a statement that reads one (`fs::read_dir`, `fs::read_to_string`) must not turn
//!   the error into a default (`unwrap_or_default`, `unwrap_or`, `unwrap_or_else`, `.ok()`),
//!   unless a comment on it or the line before says `(optional`.
//! * `default-beside-new` (review 4.1): a type that derives Default and has a `fn new()` with no
//!   arguments makes `new` the default itself (`Self::default()`), so the two cannot disagree.
//! * `dib-without-flush` (review 3.4): an executor function that touches a DIB section's memory
//!   (`->bits`, `row(`) calls GdiFlush first (the function that creates the section excepted).
//! * `wait-unchecked` (review 3.5): a D3D12 queue signal, fence event or wait whose result is
//!   thrown away (the call is the whole statement).
//! * `confined` (review of 8480456, 1.2 and 1.3): what must go through one function is used only
//!   there (`CONFINED`): DirectDraw's pixels (`.px[`) only in Surf's accessors, so nothing reads
//!   a byte the model does not know (Px::Unknown) as a value; D3DX's half routines only in
//!   F16Array::half, so the array and the load codecs cannot pick different ones.
//! * `forbidden` (review of 8480456, 1.1): a construction Windows' answers refuted (`FORBIDDEN`):
//!   JPEG 4:4:0's chroma repeated instead of interpolated; device12 again dropping the swap
//!   chain (2.2); OptimizeRGB's index taken as 0 for a dot below 0 on x64 (4: Windows faults).
//! * `silent-cut` (review of 8480456, 3): an executor loop that copies only while a count is
//!   below a capacity (`for (...; ... && n < CAP; ...)`) drops the rest without a word, and the
//!   model reads it all: refuse past the capacity (`bad = 1`) instead. The line reader's MAXTOK
//!   (runner/main.c, mirrored by the model's rootsig::MAXTOK) is the one cut allowed.
//! * `kani-gates` (review of 8480456, 5.1): an API stable in the build's Rust but not in Kani's
//!   compiler (`KANI_GATES`) is used only with its feature turned on for Kani in src/lib.rs
//!   (`#![cfg_attr(kani, feature(..))]`), so the proofs compile whatever the generators use.
//! * `header-unflushed` (review of 8480456, 4): runner/main.c writes a case's header out at once
//!   (`oflush()` right after it), so a stop inside a case names that case.
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::code::{code_only, line_of};

/// (file, what (a regular expression), the functions it may appear in): `confined`
pub const CONFINED: &[(&str, &str, &[&str])] = &[
    ("src/model/ddraw.rs", r"\.px\[", &["get", "pixel", "set", "put"]),
    ("src/model/d3dx9.rs", r"\bf16_(sse|one|x64|x64_tail)\s*\(", &["half"]),
];

/// (file, what (a regular expression), why): `forbidden`
pub const FORBIDDEN: &[(&str, &str, &str)] = &[
    ("src/model/gdiplus.rs", r"\*\s*\(\s*256\s*-\s*wt\s*\)|\(\s*2\s*\*\s*\(\s*c0\b", "a hatch texel's mix and an antialiased pixel's share are the pure hatch_mix and coverage_share the Lean mirrors gp.hatch_mix, gp.coverage_share hold equal to spec/lean/Kikizake/GdiPlus/Blend.lean; a copy inlined in paint escapes them"),
    ("src/model/jpeg.rs", r"Chroma::H1V2\s*=>\s*repeat\s*\(", "4:4:0's chroma rows are interpolated 3:1 (opt-jpeg-subs-*-4), not repeated"),
    ("src/calls.rs", r"\[\s*(open|close)\s*\+|\[\s*\.\.\s*open\s*\]", "a call line is split (split_once, rsplit_once), not sliced at positions found apart: \") = (\" panicked (review of 8480456, 5.4)"),
    ("src/model/ntstr.rs", r"from_str_radix\s*\(\s*&\w+\s*\[", "hex bytes are read as the executor's hexbytes does, digit by digit of the bytes: a str slice through a character past ASCII panicked (review of 8480456, 5.4)"),
    ("tests/calls.rs", r"calls::check_dir\s*\(", "the whole set's condition is check_complete (every suite of spec/calls-suites.txt, current); check_dir checks only the recordings there are"),
    ("src/model/d3d12.rs", r#""device12"\s*=>\s*\{[^}]*self\.(chain|factory)\s*="#, "device12 again keeps the executor's device and what was made of it (dxgi device-again)"),
    ("src/model/d3d12.rs", r"\bat\s*\+\s*pitch\s*\*", "a subresource's bytes and its last row's end are 32-bit counts on Windows (Bytes32::of; golden d3d12-* fp-limit2-offsets-*, fp-limit2-total-*), not 64-bit sums"),
    ("src/model/d3d12.rs", r"kind\(\)\s*!=\s*Dim::Tex2D\s*&&\s*\(\s*d\.w\s*>", "GetCopyableFootprints checks a 2D texture's size as CreateCommittedResource does (size_max; golden d3d12-* fp-wide-*, fp-limit-*)"),
    ("src/lean/d3d12.rs", r"\bFP_SIZES\b|0x7fff_fff0", "the footprints mirror asks widths and heights up to 0xffffffff (SIZES): a narrower domain hid the model's 32-bit wrap"),
    ("src/model/d3draster.rs", r"d\.rast\[1\]\s*==\s*[23]\s*&&|p[xy]\s*>=\s*d\.scissor|\(s\.[xy]\s*-\s*1\)\.div_euclid|rem_euclid\(\s*2\s*\*\s*size|\(256\s*-\s*w[uv]?\)|2\s*=>\s*(a|reference)\s*<|let\s+top_left\s*=|Topo::TriStripAdj\s*=>\s*\(\[", "culling, the scissor test, a point's pixel, the address modes, the comparison functions, the 8-bit filters, the top-left rule and the topologies' vertices are the pure functions of d3drules.rs the Lean mirrors hold equal to spec/lean/Kikizake/D3d11/; a copy inlined in d3draster.rs escapes them"),
    ("src/model/d3d9draw.rs", r"if\s+self\.v8\s*\{\s*0xffff_ffff\s*\}|Self::blend_op\b|let\s+e\s*=\s*\(q\.0\s*-\s*p\.0\)", "the blend color, blend operation and top-left rule are the pure blend_color, blend_op and covers the Lean mirrors hold equal to spec/lean/Kikizake/D3d9/Raster.lean; a copy inlined elsewhere escapes them"),
    ("src/model/d3d9draw.rs", r"a1\[i\]\s*\*\s*a2\[i\]|a1\[i\]\s*\+\s*a2\[i\]", "the stage operations' formulas are stage_value's, which the Lean mirror d3d9.stage holds bit for bit to spec/lean/Kikizake/D3d9/Stage.lean; a copy inlined elsewhere escapes it"),
    ("src/model/d3d9draw.rs", r"\(-\(z\s*\*\s*density\)\)\.exp\(\)|\(1\.0\s*-\s*f\)\s*\*\s*fc\[", "the fog factor and blend are fog_factor, fog_exp_arg and fog_blend, which the Lean mirrors d3d9.fog* hold to spec/lean/Kikizake/D3d9/Fog.lean; a copy inlined elsewhere escapes them"),
];

/// (file, function, what (a regular expression), why): `forbidden` inside one function only
pub const FORBIDDEN_IN: &[(&str, &str, &str, &str)] = &[
    (
        "src/model/dxtenc.rs",
        "optimize_rgb",
        r"\bdot\s*<=?\s*F::of\s*\(\s*0(\.0)?\s*\)",
        "neither build's OptimizeRGB tests the dot below 0: x64 faults on the index -1 (zero-extended, 16 GiB past the table), x86 reads below it (Below; loadtex-dxt-negindex)",
    ),
    (
        "src/model/dxgiconv.rs",
        "decode_block",
        r"171\s*\*|\*\s*257\b|\*\s*32767\b|\+\s*[23]\s*\)\s*/\s*[57]",
        "BC1's colours, BC3's alpha and BC4's steps are the pure bc1_codes, bc3_alpha, bc4_code and bc4_snorm_code that tests/laws_conv.rs proves over every endpoint pair; a copy inlined in decode_block escapes them",
    ),
    (
        "src/model/dxgiconv.rs",
        "encode",
        r"\.trunc\s*\(|0x7f80_0000",
        "a 32-bit float's store and an integer clear are the pure f32_store and int_clear that tests/laws_conv.rs proves for every float; a copy inlined in encode escapes them",
    ),
    (
        "src/model/d3d12.rs",
        "clear_depth",
        r"\bto_unorm\s*\(|\.clamp\s*\(",
        "a UNORM depth clear is unorm_depth, which tests/laws_conv.rs proves for every float; a conversion inlined in clear_depth escapes it",
    ),
    (
        "src/model/bptc.rs",
        "bc6h",
        r"BC6H_MODES\s*\.\s*iter\s*\(\s*\)\s*\.\s*position",
        "BC6H's mode is bc6h_mode, which tests/laws_conv.rs proves for every mode value; a lookup inlined in bc6h escapes it",
    ),
    (
        "src/model/bptc.rs",
        "bc7",
        r"\.find\s*\(\s*\|_\|\s*s\.take",
        "BC7's mode is bc7_mode, which tests/laws_conv.rs proves for every first byte; a scan inlined in bc7 escapes it",
    ),
];

/// the name of the function `at` lies in (the last `fn name` before it)
fn enclosing_fn(code: &str, at: usize) -> &str {
    let before = code.get(..at).unwrap_or_default();
    let Some(k) = before.rfind("fn ") else { return "" };
    let name = before.get(k.saturating_add(3)..).unwrap_or_default();
    name.get(..name.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap_or(name.len())).unwrap_or_default()
}

/// `confined`: the sites of one file outside the functions allowed
fn confined(rel: &str, code: &str, text: &str) -> Vec<String> {
    let mut bad = Vec::new();
    for &(file, pat, allowed) in CONFINED.iter().filter(|c| c.0 == rel) {
        let Some(re) = rx(pat) else {
            bad.push(format!("{file}: confined: {pat} does not compile"));
            continue;
        };
        for m in re.find_iter(code) {
            // (the function's own definition is no use of it)
            if code.get(..m.start()).is_some_and(|b| b.trim_end().ends_with("fn")) {
                continue;
            }
            let f = enclosing_fn(code, m.start());
            if !allowed.contains(&f) {
                bad.push(format!("{rel}:{}: confined: {} in {f}, only in {}", line_of(text, m.start()), m.as_str().trim(), allowed.join(", ")));
            }
        }
    }
    bad
}

/// (file, sites): src/model/ files still reading the generators
pub const MODEL_USES_GENERATORS: &[(&str, usize)] = &[("src/model/wicimage.rs", 3)];

fn rx(s: &str) -> Option<Regex> {
    Regex::new(s).ok()
}

static GENERATORS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\b(crate|super)::generators\b|\buse\s+crate::generators"));
static FS_READ: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bfs::(read_dir|read_to_string)\s*\("));
static SWALLOW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\.(unwrap_or_default|unwrap_or|unwrap_or_else)\s*\(|\.ok\(\)|\.into_iter\(\)\s*\.flatten\(\)"));
/// (an `unwrap_or_else` that stops the program reports the error)
static STOPS: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"\bexit\s*\(|\bpanic!|\breturn\b"));
static DERIVE_DEFAULT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*pub(?:\([^)]*\))?\s+struct\s+(\w+)|#\[derive\([^\]]*\bDefault\b[^\]]*\)\]\s*(?:#\[[^\]]*\]\s*)*struct\s+(\w+)"));
static NEW: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"fn\s+new\s*\(\s*\)\s*->\s*(\w+)\s*\{"));
static IMPL: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"impl\s+(\w+)\s*\{"));
static C_FN: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^[A-Za-z_][\w \*]*\b(\w+)\s*\([^;{]*\)\s*\{"));
static DIB: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"->bits\b|\brow\s*\("));
/// (what only tests or sets the pointer: `!s->bits`, `s->bits ?`, `s->bits = ...`)
static DIB_POINTER: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"!\s*[\w>.-]*->bits\b|->bits\s*\?|->bits\s*=[^=]"));
static SILENT_CUT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"for\s*\([^;]*;[^;]*&&\s*\w+\s*<\s*([A-Z][A-Z0-9_]*)\s*;"));
/// (what the source writes, the feature Kani 0.68's compiler wants for it)
pub const KANI_GATES: &[(&str, &str)] = &[("Vec::<_>::from_fn", "vec_from_fn"), ("Vec::from_fn", "vec_from_fn")];

/// `kani-gates`: the features the sources need for Kani that src/lib.rs does not turn on
pub fn kani_gates(sources: &[(String, String)], lib: &str) -> Vec<String> {
    let lib = code_only(lib, false);
    let mut bad = Vec::new();
    for &(api, feature) in KANI_GATES {
        let Some((rel, _)) = sources.iter().find(|(_, t)| code_only(t, false).contains(api)) else { continue };
        let on = lib.lines().any(|l| l.contains("#![cfg_attr(kani") && l.contains(&format!("feature({feature})")));
        if !on && !bad.iter().any(|b: &String| b.contains(feature)) {
            bad.push(format!("src/lib.rs: kani-gates: {rel} uses {api}, unstable in Kani's compiler: #![cfg_attr(kani, feature({feature}))]"));
        }
    }
    bad
}

/// (file, capacity): the cuts allowed
pub const CUTS: &[(&str, &str)] = &[("runner/main.c", "MAXTOK")];
static WAIT: LazyLock<Option<Regex>> = LazyLock::new(|| rx(r"(?m)^\s*(ID3D12CommandQueue_Signal|ID3D12Fence_SetEventOnCompletion|WaitForSingleObject)\s*\("));

/// every pattern, compiled (None: one does not, an error of this file)
struct Pats {
    generators: &'static Regex,
    fs_read: &'static Regex,
    swallow: &'static Regex,
    stops: &'static Regex,
    derive_default: &'static Regex,
    new: &'static Regex,
    impls: &'static Regex,
    c_fn: &'static Regex,
    dib: &'static Regex,
    dib_pointer: &'static Regex,
    wait: &'static Regex,
    silent_cut: &'static Regex,
}

fn pats() -> Option<Pats> {
    Some(Pats { generators: GENERATORS.as_ref()?, fs_read: FS_READ.as_ref()?, swallow: SWALLOW.as_ref()?, stops: STOPS.as_ref()?, derive_default: DERIVE_DEFAULT.as_ref()?, new: NEW.as_ref()?, impls: IMPL.as_ref()?, c_fn: C_FN.as_ref()?, dib: DIB.as_ref()?, dib_pointer: DIB_POINTER.as_ref()?, wait: WAIT.as_ref()?, silent_cut: SILENT_CUT.as_ref()? })
}

const BROKEN: &str = "tools/lint/src/guards.rs: a pattern does not compile";

/// the statements of `code` (split at `;`, `{` and `}`) with their byte offsets
fn statements(code: &str) -> Vec<(usize, &str)> {
    let mut v = Vec::new();
    let mut from = 0;
    for (i, c) in code.char_indices() {
        if matches!(c, ';' | '{' | '}') {
            v.push((from, code.get(from..i).unwrap_or_default()));
            from = i.saturating_add(1);
        }
    }
    v.push((from, code.get(from..).unwrap_or_default()));
    v
}

/// the statement's only fallback is an `unwrap_or_else` whose closure stops the program
fn reports(p: &Pats, code: &str, at: usize, st: &str) -> bool {
    let Some(k) = st.find("unwrap_or_else(").filter(|_| p.swallow.find_iter(st).count() == 1) else { return false };
    let from = at.saturating_add(k).saturating_add("unwrap_or_else".len());
    let mut depth = 0usize;
    for (i, c) in code.get(from..).unwrap_or_default().char_indices() {
        match c {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return code.get(from..from.saturating_add(i)).is_some_and(|args| p.stops.is_match(args));
                }
            }
            _ => {}
        }
    }
    false
}

/// the problems of one Rust source at `rel`
pub fn rust(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = code_only(text, false);
    let mut bad = Vec::new();
    if rel.starts_with("src/model/") {
        let n = p.generators.find_iter(&code).count();
        let allowed = MODEL_USES_GENERATORS.iter().find(|(f, _)| *f == rel).map_or(0, |(_, n)| *n);
        if n > allowed {
            bad.push(format!("{rel}: model-uses-generators: {n} sites, at most {allowed}: move what both need to a neutral module"));
        }
    }
    bad.extend(confined(rel, &code, text));
    // (string literals kept: a match arm is known by its string)
    let with_strings = code_only(text, true);
    for &(file, pat, why) in FORBIDDEN.iter().filter(|f| f.0 == rel) {
        match rx(pat) {
            Some(re) => bad.extend(re.find_iter(&with_strings).map(|m| format!("{file}:{}: forbidden: {why}", line_of(text, m.start())))),
            None => bad.push(format!("{file}: forbidden: {pat} does not compile")),
        }
    }
    for &(file, func, pat, why) in FORBIDDEN_IN.iter().filter(|f| f.0 == rel) {
        match rx(pat) {
            Some(re) => bad.extend(re.find_iter(&with_strings).filter(|m| enclosing_fn(&with_strings, m.start()) == func).map(|m| format!("{file}:{}: forbidden in {func}: {why}", line_of(text, m.start())))),
            None => bad.push(format!("{file}: forbidden: {pat} does not compile")),
        }
    }
    let lines: Vec<&str> = text.split('\n').collect();
    for (at, st) in statements(&code) {
        if p.fs_read.is_match(st) && p.swallow.is_match(st) && !reports(&p, &code, at, st) {
            let (first, last) = (line_of(text, at), line_of(text, at.saturating_add(st.len())));
            let said = (first.saturating_sub(1)..=last).any(|l| l.checked_sub(1).and_then(|k| lines.get(k)).is_some_and(|t| t.contains("(optional")));
            if !said {
                bad.push(format!("{rel}:{}: io-error-as-empty: a read that fails is not an empty file; return the error (or say \"(optional\" why it may be missing)", line_of(text, at.saturating_add(st.len().saturating_sub(st.trim_start().len())))));
            }
        }
    }
    let defaults: Vec<&str> = p.derive_default.captures_iter(&code).filter_map(|c| c.get(1).or(c.get(2)).map(|m| m.as_str())).collect();
    for m in p.new.captures_iter(&code) {
        let (Some(all), Some(ret)) = (m.get(0), m.get(1)) else { continue };
        let ty = if ret.as_str() == "Self" {
            p.impls.captures_iter(code.get(..all.start()).unwrap_or_default()).last().and_then(|c| c.get(1)).map_or("", |t| t.as_str())
        } else {
            ret.as_str()
        };
        if !defaults.contains(&ty) {
            continue;
        }
        let body = code.get(all.end()..).unwrap_or_default();
        let body = body.get(..body.find('}').unwrap_or(body.len())).unwrap_or_default().trim();
        if body != "Self::default()" && body != format!("{ty}::default()") {
            bad.push(format!("{rel}:{}: default-beside-new: {ty} derives Default, so new() must be Self::default() (or drop the derive)", line_of(text, all.start())));
        }
    }
    bad
}

/// C with its comments blanked (string literals kept), positions kept
fn c_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let b: Vec<char> = text.chars().collect();
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        let next = b.get(i.saturating_add(1)).copied();
        if c == '/' && next == Some('*') {
            let mut j = i.saturating_add(2);
            while j < b.len() && !(b.get(j) == Some(&'*') && b.get(j.saturating_add(1)) == Some(&'/')) {
                j = j.saturating_add(1);
            }
            let end = j.saturating_add(2).min(b.len());
            for &x in b.get(i..end).unwrap_or_default() {
                out.push(if x == '\n' { '\n' } else { ' ' });
            }
            i = end;
        } else if c == '/' && next == Some('/') {
            while b.get(i).is_some_and(|&x| x != '\n') {
                out.push(' ');
                i = i.saturating_add(1);
            }
        } else {
            out.push(c);
            i = i.saturating_add(1);
        }
    }
    out
}

/// the problems of one executor source at `rel`
pub fn c(rel: &str, text: &str) -> Vec<String> {
    let Some(p) = pats() else { return vec![BROKEN.into()] };
    let code = c_code(text);
    let mut bad = Vec::new();
    for c in p.silent_cut.captures_iter(&code) {
        let (Some(all), Some(cap)) = (c.get(0), c.get(1)) else { continue };
        if !CUTS.contains(&(rel, cap.as_str())) {
            bad.push(format!("{rel}:{}: silent-cut: a copy stops at {} without refusing the rest; set bad past it", line_of(&code, all.start()), cap.as_str()));
        }
    }
    if rel == "runner/main.c" && code.contains("out(\"case %s") && !rx(r#"out\("case %s\\n"[^;]*\);\s*oflush\(\);"#).is_some_and(|h| h.is_match(&code)) {
        bad.push(format!("{rel}: header-unflushed: oflush() right after a case's header, so a stop inside the case names it"));
    }
    for m in p.wait.find_iter(&code) {
        bad.push(format!("{rel}:{}: wait-unchecked: the result of {} decides whether the GPU finished; check it", line_of(&code, m.start()), m.as_str().trim().trim_end_matches('(').trim()));
    }
    if Path::new(rel).file_name().and_then(|f| f.to_str()).is_some_and(|f| f.starts_with("gdi")) {
        let starts: Vec<(usize, &str)> = p.c_fn.captures_iter(&code).filter_map(|c| Some((c.get(0)?.start(), c.get(1)?.as_str()))).collect();
        for (k, &(at, name)) in starts.iter().enumerate() {
            let end = starts.get(k.saturating_add(1)).map_or(code.len(), |n| n.0);
            let body = code.get(at..end).unwrap_or_default();
            // (the body past the signature: `row` itself, and a function that makes the section)
            let open = body.find('{').map_or(0, |o| o.saturating_add(1));
            let inner = body.get(open..).unwrap_or_default();
            if name == "row" || inner.contains("CreateDIBSection") {
                continue;
            }
            let inner = p.dib_pointer.replace_all(inner, |c: &regex::Captures| " ".repeat(c.get(0).map_or(0, |m| m.as_str().len())));
            if let Some(first) = p.dib.find(&inner) {
                let flushed = inner.get(..first.start()).is_some_and(|p| p.contains("GdiFlush()"));
                if !flushed {
                    bad.push(format!("{rel}:{}: dib-without-flush: {name} touches the DIB section's memory before GdiFlush()", line_of(&code, at.saturating_add(open).saturating_add(first.start()))));
                }
            }
        }
    }
    bad
}

/// every problem in the repository at `root`
pub fn check(root: &Path) -> Result<(), String> {
    let mut bad = Vec::new();
    let mut seen = Vec::new();
    let mut rust_sources: Vec<(String, String)> = Vec::new();
    for (dir, ext) in [("src", "rs"), ("runner", "c")] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            let entries = std::fs::read_dir(&d).map_err(|e| format!("{}: {e}", d.display()))?;
            for e in entries {
                let p = e.map_err(|e| format!("{}: {e}", d.display()))?.path();
                if p.is_dir() {
                    stack.push(p);
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some(ext) {
                    continue;
                }
                let rel = p.strip_prefix(root).map_err(|e| e.to_string())?.to_string_lossy().replace('\\', "/");
                let text = std::fs::read_to_string(&p).map_err(|e| format!("{rel}: {e}"))?;
                bad.extend(if ext == "rs" { rust(&rel, &text) } else { c(&rel, &text) });
                if ext == "rs" {
                    rust_sources.push((rel.clone(), text));
                }
                seen.push(rel);
            }
        }
    }
    for f in MODEL_USES_GENERATORS.iter().map(|g| g.0).chain(CONFINED.iter().map(|c| c.0)).chain(FORBIDDEN.iter().map(|f| f.0).filter(|f| f.starts_with("src/"))).chain(FORBIDDEN_IN.iter().map(|f| f.0)) {
        if !seen.iter().any(|s| s == f) {
            bad.push(format!("{f}: listed in MODEL_USES_GENERATORS but gone: take its line out"));
        }
    }
    // (files outside src/ that a forbidden construction names)
    for f in FORBIDDEN.iter().map(|f| f.0).filter(|f| !f.starts_with("src/")) {
        match std::fs::read_to_string(root.join(f)) {
            Ok(t) => {
                bad.extend(rust(f, &t).into_iter().filter(|b| b.contains(": forbidden: ")));
                seen.push(f.to_string());
            }
            Err(e) => bad.push(format!("{f}: {e}")),
        }
    }
    let lib = std::fs::read_to_string(root.join("src/lib.rs")).map_err(|e| format!("src/lib.rs: {e}"))?;
    bad.extend(kani_gates(&rust_sources, &lib));
    if seen.is_empty() {
        bad.push("no sources found".into());
    }
    bad.sort();
    bad.is_empty().ok_or_else(|| bad.join("\n"))
}

#[cfg(test)]
#[path = "guards_tests.rs"]
mod tests;
