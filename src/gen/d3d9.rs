//! The d3d9 suites' case generators.
use super::*;
use crate::model::d3d9::{all_states, all_states8, RENDER_STATES};

pub fn d3d9() -> String {
    state_suite(false)
}

pub fn d3d8() -> String {
    state_suite(true)
}

/// Direct3D 9's or 8's device state and state blocks
fn state_suite(v8: bool) -> String {
    let (suite, api, dev, stage_types) = if v8 { ("d3d8", "Direct3D 8", "device8", 30) } else { ("d3d9", "Direct3D 9", "device", 34) };
    let mut s = format!("# gpu\n# kikizake suite {suite}: {api} device state and state blocks (SPEC.md). Made by `kikizake gen {suite}`.\n");
    if v8 {
        // (MEASURED: x86-64 Windows has no d3d8.dll, every case printed "device8 none")
        s += "# x86 only: Direct3D 8 has no x86-64 runtime\n";
    }
    // every state a new device reports, on each kind of device
    for (name, mode) in [("defaults", "swvp"), ("defaults-hwvp", "hwvp"), ("defaults-mixed", "mixed")] {
        if name != "defaults" && !v8 {
            continue;
        }
        writeln!(s, "case {name}\n{dev} {mode}").unwrap();
        for i in 0..256 {
            writeln!(s, "rs get {i}").unwrap();
        }
        for st in 0..8 {
            for t in 0..stage_types {
                writeln!(s, "tss get {st} {t}").unwrap();
            }
        }
        if !v8 {
            for sm in (0..16).chain(256..=260) {
                for t in 0..14 {
                    writeln!(s, "ss get {sm} {t}").unwrap();
                }
            }
        }
        writeln!(s, "end").unwrap();
    }
    let states = if v8 { all_states8(true) } else { all_states() };
    let device = |s: &mut String, name: &str| writeln!(s, "case {name}\n{dev} swvp").unwrap();
    let get = |s: &mut String, k: &crate::model::d3d9::Key| {
        use crate::model::d3d9::Key::*;
        match k {
            Rs(i) => writeln!(s, "rs get {i}"),
            Tss(a, b) => writeln!(s, "tss get {a} {b}"),
            Ss(a, b) => writeln!(s, "ss get {a} {b}"),
        }
        .unwrap()
    };
    let set = |s: &mut String, k: &crate::model::d3d9::Key, v: u32| {
        use crate::model::d3d9::Key::*;
        match k {
            Rs(i) => writeln!(s, "rs set {i} {v:#x}"),
            Tss(a, b) => writeln!(s, "tss set {a} {b} {v:#x}"),
            Ss(a, b) => writeln!(s, "ss set {a} {b} {v:#x}"),
        }
        .unwrap()
    };
    // every documented state set to a value of its own and read back
    device(&mut s, "set-each");
    for (i, (k, ..)) in states.iter().enumerate() {
        set(&mut s, k, 0x1000 + i as u32);
    }
    for (k, ..) in &states {
        get(&mut s, k);
    }
    writeln!(s, "end").unwrap();
    // each state block type: everything changed after the block was made, the block applied,
    // everything read
    for ty in 1..=3 {
        device(&mut s, &format!("create-{ty}"));
        writeln!(s, "sb create 0 {ty}").unwrap();
        for (i, (k, ..)) in states.iter().enumerate() {
            set(&mut s, k, 0x2000 + i as u32);
        }
        writeln!(s, "sb apply 0").unwrap();
        for (k, ..) in &states {
            get(&mut s, k);
        }
        writeln!(s, "end").unwrap();
    }
    // random sequences of sets, gets and state block calls
    let mut r = XorShift32::new(if v8 { 0xd3d8_0008 } else { 0xd3d9_0009 });
    for i in 0..150 {
        device(&mut s, &format!("random-{i}"));
        let mut touched = Vec::new();
        let mut recording = false;
        for _ in 0..r.range(5, 40) {
            let k = r.pick(&states).0;
            match r.range(0, 19) {
                0..=8 => {
                    let v = match r.range(0, 3) {
                        0 => r.range(0, 9) as u32,
                        1 => *r.pick(&[0u32, 1, 0xffff_ffff, 0x3f80_0000, 0x8000_0000]),
                        _ => r.next(),
                    };
                    set(&mut s, &k, v);
                    touched.push(k);
                }
                9..=12 => get(&mut s, &k),
                13 if !recording => {
                    writeln!(s, "sb begin").unwrap();
                    recording = true;
                }
                14 if recording => {
                    writeln!(s, "sb end {}", r.range(0, 3)).unwrap();
                    recording = false;
                }
                15 if !recording => writeln!(s, "sb create {} {}", r.range(0, 3), r.range(1, 3)).unwrap(),
                16 | 17 if !recording => writeln!(s, "sb apply {}", r.range(0, 3)).unwrap(),
                18 if !recording => writeln!(s, "sb capture {}", r.range(0, 3)).unwrap(),
                19 if !recording => writeln!(s, "sb release {}", r.range(0, 3)).unwrap(),
                _ => {}
            }
        }
        if recording {
            writeln!(s, "sb end {}", r.range(0, 3)).unwrap();
        }
        // apply each block once more and read what the case touched
        for b in 0..3 {
            if r.chance(2) {
                writeln!(s, "sb apply {b}").unwrap();
            }
        }
        touched.sort();
        touched.dedup();
        for k in &touched {
            get(&mut s, k);
        }
        writeln!(s, "end").unwrap();
    }
    // what the documentation leaves open (unmodeled until Windows' answers come)
    let mut probe = |name: &str, body: &str| writeln!(s, "case edge-{name}\n{dev} swvp\n{body}\nend").unwrap();
    probe("set-legacy-rs", "rs set 0 5\nrs get 0\nrs set 42 6\nrs get 42\nrs set 165 7\nrs get 165\nrs set 1 8\nrs get 1");
    if v8 {
        probe("set-legacy-tss", "tss set 0 0 5\ntss get 0 0\ntss set 1 12 6\ntss get 1 12\ntss set 2 29 7\ntss get 2 29");
    } else {
        probe("set-legacy-tss", "tss set 0 0 5\ntss get 0 0\ntss set 1 12 6\ntss get 1 12\ntss set 2 33 7\ntss get 2 33");
        probe("set-legacy-ss", "ss set 0 0 5\nss get 0 0");
    }
    probe("end-without-begin", "sb end 0\nsb apply 0");
    probe("begin-twice", "sb begin\nsb begin\nrs set 7 0\nsb end 0\nrs get 7");
    probe("calls-while-recording", "sb create 1 1\nsb begin\nrs set 7 0\nsb apply 1\nsb capture 1\nsb create 2 2\nsb end 0\nrs get 7");
    probe("type-0", "sb create 0 0\nrs set 7 0\nsb apply 0\nrs get 7");
    probe("type-4", "sb create 0 4\nrs set 7 0\nsb apply 0\nrs get 7");
    probe("release-while-recording", "sb create 1 1\nrs set 7 0\nsb begin\nsb release 1\nsb end 0\nsb apply 1\nrs get 7");
    let _ = RENDER_STATES;
    s
}
