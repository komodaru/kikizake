mod pairs;
use pairs::zip_eq;
use kikizake::{case, compare, model, tla};
use std::process::Command;

#[test]
fn comparison_requires_the_expected_sequence_and_end() {
    let want = "kikizake a\ncase x\nunmodeled example\ncase x\n42\ndone\n";
    assert!(compare::compare_text(want, want).differ.is_empty());
    for got in [want.replace("kikizake a", "kikizake b"), format!("extra header\n{want}"), want.replace("done", "case y\ndone"),
        want.replace("case x\n42\n", ""), want.replace("done", "done\ndone")] {
        assert!(!compare::compare_text(want, &got).differ.is_empty());
    }
    let prefix = "case x\n42\n";
    let complete = "case x\n42\ndone\n";
    assert!(compare::compare_text(prefix, prefix).differ.is_empty());
    assert!(!compare::compare_text(prefix, "case x\n42\ncase y\n").differ.is_empty());
    assert_eq!(compare::compare_text(complete, prefix).differ, ["program end: missing done marker"]);
    assert_eq!(compare::compare_text(prefix, complete).differ, ["program end: unexpected done marker"]);
    assert!(!compare::compare_text("done", "done").differ.is_empty());
}

#[test]
fn malformed_or_empty_inputs_fail() {
    let exe = env!("CARGO_BIN_EXE_kikizake");
    for args in [vec!["model", "gdi_typo"], vec!["model", "gdi", "/dev/null"], vec!["calls", "check", "no-such-recording-directory"]] {
        assert!(!Command::new(exe).args(args).output().unwrap().status.success());
    }
    for dump in ["", "State 1:\n/\\ log = invalid", "State 1:\n/\\ log = << >>\nState 2:\n/\\ x = 1"] {
        assert!(tla::check(dump).is_err());
    }
    assert_eq!(tla::check("State 1:\n/\\ log = << >>").unwrap().0, 1);
}

#[test]
fn footprint_inputs_are_narrowed_at_the_abi_boundary() {
    let output = |fields| {
        let cases = case::parse_cases(&format!("case fp\ndevice12\nfootprint12 0 1 0 1 0 256 1 {fields} 0 1 0 1 0\nend"));
        model::run_case(&cases[0]).unwrap()
    };
    assert_eq!(output("1 1"), output("65537 65537"));
}

#[test]
fn cases_have_independent_device_state() {
    let two = case::parse_cases("case a\ndevice11\nend\ncase b\nfmt 28\nend");
    let alone = model::run("test", &two[1..]);
    let both = model::run("test", &two);
    assert_eq!(case::parse_output(&alone).cases[0], case::parse_output(&both).cases[1]);
}

/// x64's D3DXFloat32To16Array goes by cpuid's vendor (D3DX9.F16.CPU): any vendor but
/// GenuineIntel as AuthenticAMD, and a recording without its cpu line has no answer
#[test]
fn cpu_dispatch_goes_by_the_vendor_alone() {
    let cases = case::parse_cases("case x\nx9f16 0x7fc00000 0x33000401 0x33000401 0x33000401 0x33000401 0x33000401 0x33000401 0x33000401 0x3f801000\nend");
    let run = |cpu: &str| model::ModelContext::recorded(64, cpu).evaluate(|| model::run_case(&cases[0])).ok();
    let amd = run("cpu AuthenticAMD feda3203 178bfbff f1bf07a9");
    assert!(amd.is_some());
    assert_eq!(run("cpu Unknown 0 0 0"), amd);
    assert_eq!(run("cpu Unknown 10000000 0 0"), amd);
    let intel = run("cpu GenuineIntel 0 0 0");
    assert!(intel.is_some() && intel != amd);
    let none = model::ModelContext::recorded(64, "").evaluate(|| model::run_case(&cases[0]));
    assert!(matches!(none, Err(model::Fail::NoInput(model::Input::Cpu))));
}

#[test]
fn incomplete_case_blocks_are_rejected() {
    for text in ["case", "case x", "case x\ncase y\nend", "end", "device11"] {
        assert!(case::checked_cases(text).is_err());
    }
}

#[test]
fn unanswered_is_insufficient_for_a_measured_clause() {
    let clause = kikizake::spec::clauses().into_iter().find(|c| c.status == "measured").unwrap();
    let evidence = kikizake::trace::Evidence { unanswered: 1, ..Default::default() };
    assert!(!kikizake::trace::gaps(&clause, &evidence, &Default::default()).is_empty());
}

#[test]
fn unknown_shader_with_an_intact_container_is_unmodeled() {
    let mut shader = kikizake::generators::d3ddraw::shader(kikizake::generators::d3ddraw::Prog::Ps);
    // The DXBC writer accepts a program with a different signature name.
    let index = shader.windows(9).position(|w| w == b"SV_Target").unwrap();
    shader[index + 3] = b't';
    let sum = kikizake::generators::dxbc::checksum(&shader[20..]);
    for (dest, word) in zip_eq(shader[4..20].chunks_exact_mut(4), sum) { dest.copy_from_slice(&word.to_le_bytes()); }
    let text = format!("case x\ndevice11\nshader 0 ps {}\nend", kikizake::generators::dxbc::hex(&shader));
    assert!(matches!(model::run_case(&case::parse_cases(&text)[0]), Err(model::Fail::Unmodeled(_))));
}

#[test]
fn a_modeled_stop_omits_following_cases() {
    let cases = case::parse_cases(include_str!("../cases/d3d11res.txt"));
    let stopped = cases.into_iter().find(|c| c.name == "x86-usageff-0").unwrap();
    let mut stream = vec![stopped];
    stream.extend(case::parse_cases("case later\ndevice11\nend"));
    let out = model::ModelContext::recorded(32, "").evaluate(|| model::run("test", &stream));
    let parsed = case::parse_output(&out);
    assert!(!parsed.done);
    assert_eq!(parsed.cases.len(), 1);
    assert_eq!(parsed.cases[0].0, stream[0].name);
}

#[test]
fn x86_query_heap_boundary_accepts_only_recorded_results() {
    let input = case::parse_cases("case q\ndevice12\nqheap12 0 8126464\nend");
    let want = model::ModelContext::recorded(32, "").evaluate(|| model::run("s", &input));
    for (hr, valid) in [("00000000", true), ("8007000e", true), ("80070057", false)] {
        let actual = format!("kikizake s\ncase q\ndevice12 ok\nqheap12 hr {hr}\ndone\n");
        assert_eq!(compare::compare_text(&want, &actual).differ.is_empty(), valid);
    }
}

/// `kikizake lean` fails a Lean mirror that answers differently from the model, or stops
/// answering (src/lean/): here a stand-in that answers 0 to everything, and one that quits
#[cfg(unix)]
#[test]
fn lean_mirror_differences_are_found() {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("kikizake-lean-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = |name: &str, body: &str| {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_str().unwrap().to_string()
    };
    let zero = script("zero", "while read l; do echo 0; done");
    // (a test thread that forks while the script is still open for writing holds it open in the
    // child until its exec: ETXTBSY, "Text file busy", a moment; spawning again is the remedy)
    let check = |exe: &str, only: &str| {
        let mut r = kikizake::lean::check(exe, Some(only));
        for _ in 0..50 {
            if !matches!(&r, Err(e) if e.contains("Text file busy")) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
            r = kikizake::lean::check(exe, Some(only));
        }
        r
    };
    let r = check(&zero, "gdi.alpha").unwrap();
    assert_eq!(r.len(), 1);
    assert!(!r[0].differ.is_empty() && r[0].calls > 0);
    let quit = script("quit", "read l; echo 0");
    assert!(matches!(check(&quit, "gdi.alpha"), Err(e) if !e.contains("Text file busy")));
    assert!(check(&zero, "no.such").is_err());
    std::fs::remove_dir_all(&dir).unwrap();
}

/// The samples of a `Sampled` mirror cover its ranges: in each argument (by the argument lists'
/// length and place) spanning more than 2^33, at least one sample in 1000 lies past the lowest
/// 2^32 values, and one spanning both signs past 2^32 each way has one sample in 100 on each
/// side (the review of 84b05c8, 4.1: a 32-bit draw put 100,001 of 100,002 64-bit pixels below
/// 2^32 and 300,004 of gdi.c8's 300,008 numbers below 0)
#[test]
fn sampled_mirrors_reach_their_whole_ranges() {
    use std::collections::BTreeMap;
    let mut bad = Vec::new();
    for m in kikizake::lean::mirrors() {
        if !matches!(m.domain, kikizake::lean::Domain::Sampled(_)) {
            continue;
        }
        let mut cols: BTreeMap<(usize, usize), Vec<i64>> = BTreeMap::new();
        (m.inputs)(&mut |a| {
            for (i, &v) in a.iter().enumerate() {
                cols.entry((a.len(), i)).or_default().push(v);
            }
        });
        for ((n, i), v) in &cols {
            if let Some(why) = kikizake::lean::uncovered(v) {
                bad.push(format!("{} argument {i} of {n}: {why}", m.name));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

/// the coverage rule above catches the review's distributions
#[test]
fn narrow_samples_are_uncovered() {
    let mut r = kikizake::rng::XorShift32::new(7);
    // (one 32-bit word past the low end of 0..=i64::MAX, as the review measured)
    let low: Vec<i64> = (0..10_000).map(|_| i64::from(r.next())).chain([i64::MAX]).collect();
    assert!(kikizake::lean::uncovered(&low).is_some());
    let negative: Vec<i64> = (0..10_000).map(|_| -(1 << 40) + i64::from(r.next())).chain([1 << 40]).collect();
    assert!(kikizake::lean::uncovered(&negative).is_some());
    let mut s = kikizake::rng::Sample64::new(7);
    let wide: Vec<i64> = (0..10_000).map(|_| s.range(-(1 << 40), 1 << 40)).collect();
    assert_eq!(kikizake::lean::uncovered(&wide), None);
    let all: Vec<i64> = (0..10_000).map(|_| s.range(0, i64::MAX)).collect();
    assert_eq!(kikizake::lean::uncovered(&all), None);
}
