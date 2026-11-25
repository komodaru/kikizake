//! The models against Windows' recorded answers, and the case files against their generators.
//!
//! Windows' answers: golden/<suite>-<arch>/stdout.txt, recorded by the windows workflow
//! (KIKIZAKE_GOLDEN=<dir> reads them from elsewhere), and golden/probes/: answers of masu's winref
//! probes (github.com/a-kaibu/masu, tests/winref) that check the models independently. The architecture declarations in each case file select the required recordings.
use std::path::PathBuf;
use kikizake::{case, compare, generators, model};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn case_files_are_what_the_generators_make() {
    for s in generators::SUITES {
        let f = std::fs::read_to_string(root().join(format!("cases/{s}.txt"))).unwrap();
        assert!(f == generators::suite(s).unwrap(), "cases/{s}.txt is stale: cargo run -- gen {s} > cases/{s}.txt");
    }
}

#[test]
fn models_match_windows() {
    let mut bad = Vec::new();
    // (KIKIZAKE_SUITES=a,b: only those, as cargo mutants of one model wants)
    let only = std::env::var("KIKIZAKE_SUITES").ok();
    let dir = std::env::var_os("KIKIZAKE_GOLDEN").map(PathBuf::from).unwrap_or_else(|| root().join("golden"));
    if let Some(filter) = &only {
        assert!(filter.split(',').all(|s| generators::SUITES.contains(&s)), "unknown suite in {filter}");
    }
    let selected: Vec<_> = only.as_deref().map_or_else(|| generators::SUITES.to_vec(), |s| s.split(',').collect());
    if let Err(e) = kikizake::trace::verify_sources(&dir, &selected) {
        bad.push(e.to_string());
    }
    for s in generators::SUITES {
        if only.as_ref().is_some_and(|o| !o.split(',').any(|x| x == *s)) {
            continue;
        }
        let input = generators::suite(s).unwrap();
        let cases = case::parse_cases(&input);
        for arch in generators::architectures(&input) {
            let reference = dir.join(format!("{s}-{arch}"));
            let (g, status) = match (std::fs::read_to_string(reference.join("stdout.txt")), std::fs::read_to_string(reference.join("exit"))) {
                (Ok(g), Ok(e)) => match e.trim().parse::<i64>() {
                    Ok(status) => (g, status),
                    Err(e) => {
                        bad.push(format!("{s}-{arch}: exit status: {e}"));
                        continue;
                    }
                },
                (Err(e), _) | (_, Err(e)) => {
                    bad.push(format!("{s}-{arch}: {e}"));
                    continue;
                }
            };
            let want = model::ModelContext::recorded(arch, &g).evaluate(|| model::run(s, &cases));
            // (every suite is compared to the end: a stop here would hide the differences after it)
            if case::parse_output(&want).done != (status == 0) {
                bad.push(format!("{s}-{arch}: the model's end (done or not) is not Windows' exit {status}"));
            }
            // These suites have deterministic, fully modeled output on both architectures.
            // A wildcard, missing result or newly unmodeled case must fail this comparison.
            if matches!(*s, "ntstr" | "ntcase" | "glstipple") && (status != 0 || !want.lines().eq(g.lines())) {
                bad.push(format!("{s}-{arch}: expected a successful recording and exact output for every operation"));
            }
            if *s == "opengl" {
                let recorded = case::parse_output(&g);
                let modeled = case::parse_output(&want);
                for name in [
                    "gap-light-0", "gap-eval-0",
                    "gap-select-0", "gap-select-1", "gap-select-2",
                    "gap-select-3", "gap-select-4", "gap-select-5",
                    "selection-depth-near", "selection-depth-interior", "selection-depth-far",
                    "selection-depth-partial-header", "selection-depth-partial-second", "selection-depth-exact",
                    "feedback-topology-triangles", "feedback-topology-triangle-strip",
                    "feedback-topology-triangle-fan", "feedback-topology-quads",
                    "feedback-topology-quad-strip", "feedback-topology-polygon",
                ] {
                    let expected = recorded.cases.iter().find(|(n, _)| n == name)
                        .unwrap_or_else(|| panic!("missing {name} Windows recording on {arch}"));
                    let actual = modeled.cases.iter().find(|(n, _)| n == name);
                    if name.starts_with("feedback-topology-") {
                        let (_, lines) = actual.expect("missing modeled polygon feedback");
                        let (ctx, feedback) = lines.split_first().expect("missing modeled context");
                        let (reference_ctx, reference_feedback) = expected.1.split_first().expect("missing reference context");
                        // Pixel format numbers depend on the installed display driver.
                        assert!(ctx.starts_with("glctx ok ") && reference_ctx.starts_with("glctx ok "));
                        assert_eq!(feedback, reference_feedback, "{name} on {arch}-bit Windows");
                    } else {
                        assert_eq!(actual, Some(expected), "{name} on {arch}-bit Windows");
                    }
                }
            }
            if *s == "d2dlimits" {
                let expected = case::parse_output(&g);
                let actual = case::parse_output(&want);
                for (name, lines) in expected.cases.iter().filter(|(name, _)| name.starts_with("bitmap-src-") || name.starts_with("gradient-") || name.starts_with("workspace-")) {
                    assert_eq!(actual.cases.iter().find(|(n, _)| n == name).map(|(_, out)| out), Some(lines),
                        "{s}-{arch} {name}");
                }
            }
            if *s == "d3d7wide" {
                let expected = case::parse_output(&g);
                let actual = case::parse_output(&want);
                for name in ["steep-rising", "steep-falling"] {
                    let reference = expected.cases.iter().find(|(n, _)| n == name)
                        .unwrap_or_else(|| panic!("missing Windows recording for {s}-{arch} {name}"));
                    let modeled = actual.cases.iter().find(|(n, _)| n == name);
                    assert_eq!(modeled, Some(reference), "{s}-{arch} {name}");
                }
            }
            if matches!(*s, "d3d10draw" | "d3d11draw") {
                let expected = case::parse_output(&g);
                let actual = case::parse_output(&want);
                for name in [
                    "probe-depth-alone", "probe-rt-1d", "probe-rt-1darray", "probe-rt-3d",
                    "probe-rt-3d-slice1", "probe-rt-buffer", "probe-rt-buffer-off",
                    "shader-bad", "probe-layout-refused", "probe-layout-otherkind",
                    "ld-89", "ld-26", "ld-table-29", "probe-ld-out", "hazard-load-mip1",
                    "probe-filter-view-mips", "probe-filter-view-mips-mag", "probe-filter-1d-mips",
                    "probe-filter-1darray-mips", "probe-filter-array-mips", "probe-filter-array1-mips",
                    "probe-filter-cube-mips", "probe-filter-cubearray-mips", "probe-filter-3d-mips",
                    "probe-rt-ms4-depth", "probe-rt-ms4-zplane", "probe-rt-ms4-depth1",
                    "probe-rt-ms4-depth1-prev", "probe-linesms-z",
                ] {
                    let reference = expected.cases.iter().find(|(n, _)| n == name)
                        .unwrap_or_else(|| panic!("missing Windows recording for {s}-{arch} {name}"));
                    let modeled = actual.cases.iter().find(|(n, _)| n == name);
                    assert_eq!(modeled, Some(reference), "{s}-{arch} {name}");
                }
            }
            if matches!(*s, "d3d10res" | "d3d11res") {
                let expected = case::parse_output(&g);
                let actual = case::parse_output(&want);
                for index in 0..=20 {
                    let name = format!("probe-box-{index}");
                    let reference = expected.cases.iter().find(|(n, _)| *n == name)
                        .unwrap_or_else(|| panic!("missing Windows recording for {s}-{arch} {name}"));
                    let modeled = actual.cases.iter().find(|(n, _)| *n == name);
                    assert_eq!(modeled, Some(reference), "{s}-{arch} {name}");
                }
            }
            let r = compare::compare_text(&want, &g);
            eprintln!("kikizake_{s}-{arch}: {} cases, {} as Windows, {} unmodeled, {} differ", r.cases, r.same, r.unmodeled, r.differ.len());
            bad.extend(r.differ.iter().map(|d| format!("{s}-{arch} {d}")));
        }
    }
    model::set_arch(32);
    assert!(bad.is_empty(), "the model differs from Windows:\n{}", bad.join("\n"));
}

/// the D3D9 defaults table (from the documentation) against an independent Windows run, masu's
/// d3d9state probe, when it is there
#[test]
fn d3d9_defaults_match_the_d3d9state_probe() {
    let p = root().join("golden/probes/d3d9state-32/stdout.txt");
    let Ok(g) = std::fs::read_to_string(p) else { return };
    use kikizake::model::d3d9::{all_states, Key};
    let mut n = 0;
    for (k, d, _) in all_states() {
        let want = match k {
            Key::Rs(i) => format!("rs {i} = {d:#x}"),
            Key::Tss(s, t) if [0, 1, 7].contains(&s) => format!("tss {s} {t}: 0 = {d:#x}"),
            Key::Ss(s, t) if [0, 15, 256, 257, 260].contains(&s) => format!("ss {s} {t}: 0 = {d:#x}"),
            _ => continue,
        };
        // (printf's %#x prints 0 as "0")
        let want = want.replace("= 0x0", "= 0");
        assert!(g.lines().any(|l| l == want), "Windows does not say {want:?}");
        n += 1;
    }
    assert!(n > 200);
}

/// the state block groups against masu's d3d9sb probe (which block types bring each state back)
#[test]
fn d3d9_groups_match_the_d3d9sb_probe() {
    let p = root().join("golden/probes/d3d9sb-32/stdout.txt");
    let Ok(g) = std::fs::read_to_string(p) else { return };
    use kikizake::model::d3d9::{all_states, Group, Key};
    let mut n = 0;
    for (k, _, grp) in all_states() {
        // (the probe's names: rs i, tss t (stage 0) / 700+t (stage 7), ss t (sampler 0) / 1500+t ...)
        let name = match k {
            Key::Rs(i) => format!("rs {i}"),
            Key::Tss(0, t) => format!("tss {t}"),
            Key::Tss(7, t) => format!("tss {}", 700 + t),
            Key::Ss(0, t) => format!("ss {t}"),
            Key::Ss(15, t) => format!("ss {}", 1500 + t),
            _ => continue,
        };
        let (a, p, v) = match grp {
            Group::Pixel => (1, 1, 0),
            Group::Vertex => (1, 0, 1),
            Group::Both => (1, 1, 1),
            Group::All => (1, 0, 0),
            Group::Nowhere => (0, 0, 0),
        };
        let want = format!("{name} all {a} pixel {p} vertex {v}");
        assert!(g.lines().any(|l| l == want), "Windows does not say {want:?}");
        n += 1;
    }
    assert!(n > 150);
}

/// the Direct3D 11 model against masu's d3d11desc probe: its tables of descriptions (read from
/// the probe's source) through the model, each line as Windows printed it
#[test]
fn d3d11_matches_the_d3d11desc_probe() {
    let Ok(src) = std::fs::read_to_string(root().join("golden/probes/d3d11desc.c")) else { return };
    let Ok(g) = std::fs::read_to_string(root().join("golden/probes/d3d11desc-32/stdout.txt")) else { return };
    use kikizake::model::d3d11::{normalize, Kind};
    let num = |t: &str| -> u32 {
        let t = t.trim();
        if let Some(v) = t.strip_prefix("(DWORD)-") {
            return (v.parse::<i64>().unwrap() as u32).wrapping_neg();
        }
        if let Some(h) = t.strip_prefix("0x") {
            return u32::from_str_radix(h, 16).unwrap();
        }
        t.parse().unwrap()
    };
    let at = |t: &str| -> usize {
        let t = t.trim();
        if let Some(a) = t.strip_prefix("RT(") {
            let (i, f) = a.trim_end_matches(')').split_once(',').unwrap();
            return 2 + 8 * i.trim().parse::<usize>().unwrap() + f.trim().parse::<usize>().unwrap();
        }
        t.parse().unwrap()
    };
    let mut n = 0;
    for (k, table) in [(Kind::Blend, "blends[]"), (Kind::Depth, "depths[]"), (Kind::Raster, "rasters[]"), (Kind::Sampler, "samplers[]")] {
        let body = &src[src.find(table).unwrap()..];
        let body = &body[..body.find("};").unwrap()];
        let mut made: Vec<(String, Vec<u32>)> = Vec::new();
        for l in body.lines().filter(|l| l.trim_start().starts_with("{ \"")) {
            let name = l.split('"').nth(1).unwrap().to_string();
            let mut d = kikizake::generators::d3d11_base(k);
            // { "name", { { at, v }, ... END } }, at maybe RT(i, f)
            for c in l.split("{ ").skip(3) {
                let c = c.split(" }").next().unwrap();
                if let Some((a, v)) = c.rsplit_once(", ") {
                    d[at(a)] = num(v);
                }
            }
            let want = match normalize(k, &d) {
                None => format!("{} {name} 0x80070057 same -", k.name()),
                Some(nd) => {
                    let same = made.iter().find(|(_, o)| *o == nd).map_or("-".to_string(), |(n, _)| n.clone());
                    let words: Vec<String> = nd.iter().map(|w| format!("{w:x}")).collect();
                    made.push((name.clone(), nd));
                    format!("{} {name} 0 {} same {same}", k.name(), words.join(" "))
                }
            };
            assert!(g.lines().any(|l| l == want), "Windows does not say {want:?}");
            n += 1;
        }
    }
    assert!(n > 100, "{n}");
}

/// the Direct3D 8 defaults against masu's d3d8state probe (device 0 software, 1 hardware
/// vertex processing)
#[test]
fn d3d8_defaults_match_the_d3d8state_probe() {
    let Ok(g) = std::fs::read_to_string(root().join("golden/probes/d3d8state-32/stdout.txt")) else { return };
    let text = format!("case a\ndevice8 swvp\n{0}end\ncase b\ndevice8 hwvp\n{0}end\n", {
        let mut s = String::new();
        for i in 0..256 {
            s += &format!("rs get {i}\n");
        }
        for st in [0, 7] {
            for t in 0..30 {
                s += &format!("tss get {st} {t}\n");
            }
        }
        s
    });
    let mut n = 0;
    for (dev, c) in kikizake::case::parse_cases(&text).iter().enumerate() {
        let out = kikizake::model::run_case(c).unwrap();
        for l in &out[1..] {
            // the probe: "0 rs 7 = 0x1", "0 rs 1: Get 0x8876086c", "0 tss 0 1: 0 = 0x4"
            let t: Vec<&str> = l.split(' ').collect();
            let v = |h: &str| match u32::from_str_radix(h, 16).unwrap() {
                0 => "0".to_string(),
                v => format!("{v:#x}"),
            };
            let want = match (t[0], t.len()) {
                ("rs", 4) if t[2] == "=" => format!("{dev} rs {} = {}", t[1], v(t[3])),
                ("rs", 4) => format!("{dev} rs {}: Get {}", t[1], v(t[3])),
                ("tss", 5) if t[3] == "=" => format!("{dev} tss {} {}: 0 = {}", t[1], t[2], v(t[4])),
                _ => panic!("{l}"),
            };
            assert!(g.lines().any(|l| l == want), "Windows does not say {want:?}");
            n += 1;
        }
    }
    assert_eq!(n, 2 * (256 + 60));
}

/// the WIC model against masu's wicconv probe: its conversions of 8x4 bitmaps (bytes from its
/// own formula), each one as Windows printed it; the pairs the model leaves open are skipped
#[test]
fn wic_matches_the_wicconv_probe() {
    let Ok(g) = std::fs::read_to_string(root().join("golden/probes/wicconv-32/stdout.txt")) else { return };
    use kikizake::model::wic::{convert, format, stride, FORMATS};
    const LONG: [&str; 23] = ["BlackWhite", "2bppGray", "4bppGray", "8bppGray", "16bppGray", "16bppBGR555", "16bppBGR565", "24bppBGR", "24bppRGB", "32bppBGR", "32bppBGRA", "32bppPBGRA", "32bppRGB", "32bppRGBA", "32bppPRGBA", "48bppRGB", "64bppRGBA", "32bppCMYK", "8bppAlpha", "1bppIndexed", "2bppIndexed", "4bppIndexed", "8bppIndexed"];
    assert_eq!(LONG.len(), FORMATS.len());
    let bytes = |f: usize, seed: u32| -> Vec<u8> { (0..stride(f, 8) * 4).map(|i| ((i as u32 * 73 + seed * 29 + 41) ^ (i as u32 >> 3) * 151) as u8).collect() };
    let mut jobs: Vec<(usize, usize, u32)> = (0..23).map(|f| (f, 10, f as u32)).collect();
    for seed in 0..2 {
        jobs.extend((0..23).map(|f| (10, f, 100 + seed)));
    }
    for (i, (a, b)) in [(7, 11), (3, 11), (11, 10), (14, 11), (16, 11), (15, 7), (4, 3), (6, 7), (13, 7), (22, 11)].into_iter().enumerate() {
        jobs.push((a, b, 200 + i as u32));
    }
    let (mut n, mut open, mut bad) = (0, 0, Vec::new());
    let mut seen = std::collections::HashMap::new();
    for (from, to, seed) in jobs {
        let label = format!("{}>{}: can 1 ", LONG[from], LONG[to]);
        // (the probe prints the same label for both 32bppBGRA seeds: take them in order)
        let k = seen.entry(label.clone()).or_insert(0);
        let Some(line) = g.lines().filter(|l| l.starts_with(&label)).nth(*k) else { panic!("no {label}") };
        *k += 1;
        let want = &line[label.len()..];
        match convert(from, to, 8, 4, &bytes(from, seed)) {
            Ok(o) => {
                let got: String = o.iter().map(|b| format!("{b:02x}")).collect();
                if got != want {
                    bad.push(format!("{label}\n  want {want}\n  got  {got}"));
                }
                n += 1;
            }
            Err(_) => open += 1,
        }
    }
    let _ = format;
    assert!(bad.is_empty(), "{} of {n} differ:\n{}", bad.len(), bad.join("\n"));
    assert!(n > 50, "{n} {open}");
}

/// cases known to have a line the executor cuts (to be fixed in their suites; only shrinks)
const LONG_LINES: &[(&str, &str)] = &[];

/// the generators write no line a model refuses ("invalid case" would pass over a case
/// unchecked: a model broken on its own cases must not look unmodeled), nor one longer than
/// the executor reads
#[test]
fn no_generated_line_is_bad() {
    let only = std::env::var("KIKIZAKE_SUITES").ok();
    let mut bad = Vec::new();
    for s in generators::SUITES {
        if only.as_ref().is_some_and(|o| !o.split(',').any(|x| x == *s)) {
            continue;
        }
        let cases = case::checked_cases(&generators::suite(s).unwrap()).unwrap_or_else(|e| panic!("{s}: {e}"));
        // (a line longer than the executor reads is cut there: the case asks something else)
        for c in &cases {
            if c.ops.iter().any(|o| o.tok.len() > case::MAX_TOKENS) && !LONG_LINES.contains(&(*s, c.name.as_str())) {
                bad.push(format!("{s} {}: a line of more than {} tokens", c.name, case::MAX_TOKENS));
            }
        }
        for arch in [32, 64] {
            model::set_arch(arch);
            let (_, invalid) = model::run_report(s, &cases);
            bad.extend(invalid.into_iter().map(|name| format!("{s}-{arch} {name}")));
        }
    }
    model::set_arch(32);
    assert!(bad.is_empty(), "cases a model calls bad lines:\n{}", bad.join("\n"));
}

/// the model's token limit is the executor's (a longer line is cut there, as region
/// loose-many was)
#[test]
fn token_limit_is_the_executors() {
    let h = include_str!("../runner/core.h");
    assert!(h.contains(&format!("#define MAXTOK {}\n", kikizake::case::MAX_TOKENS)), "runner/core.h MAXTOK is not case::MAX_TOKENS");
}
