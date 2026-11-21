//! Laws of D3DX's DXT colour encoder (src/model/dxtenc.rs): OptimizeRGB's index below 0, which
//! neither build tests for, as each build meets it (D3DX9.LOAD.NEGFAULT, D3DX9.LOAD.FORMAT).
use kikizake::case::checked_cases;
use kikizake::model::dxtenc::{encode_bc1, Below, Real, Steps, X87};
use kikizake::model::{run_case_ended, set_arch, Ended, Fail};
use proptest::prelude::*;

/// the block of case loadtex-dxt-negindex (A8R8G8B8 bytes: b, g, r, a), as 0..1 (r, g, b, a)
fn review_block() -> [[f32; 4]; 16] {
    let hex = "a328ebff3232c4ffc0271afff632a2ff8e2ac4ffc32e6eff8e27fffffc2991ffe034a2ffde2a00ffeb2d68ffcf2c6dff6a154bff00e5ffffae27a8fff663c6ff";
    let b: Vec<f32> = (0..64).map(|i| f32::from(u8::from_str_radix(hex.get(i * 2..i * 2 + 2).unwrap_or("00"), 16).unwrap_or(0)) / 255.0).collect();
    std::array::from_fn(|i| [b[i * 4 + 2], b[i * 4 + 1], b[i * 4], b[i * 4 + 3]])
}

fn x87(px: [[f32; 4]; 16]) -> [[X87; 4]; 16] {
    px.map(|c| c.map(X87::of))
}

/// the review's block: texel 13's dot reaches -1.5035 in the fifth Newton round, the index
/// trunc(-1.0035) is -1: x64 faults, x86 reads the stack below its table
#[test]
fn the_review_block_faults_on_x64() {
    let px = review_block();
    assert_eq!(encode_bc1(&px, Steps::Four), Err(Below::Fault));
    assert_eq!(encode_bc1(&x87(px), Steps::Four), Err(Below::Unknown));
}

/// the suite as the executor runs it: on x64 loadtex-dxt-negindex dies with nothing of it
/// written but its header (the lines before the fault were never flushed), on x86 it is open
#[test]
fn loadtex_negindex_ends_the_x64_run() {
    let Ok(text) = std::fs::read_to_string("cases/d3dx9load.txt") else { panic!("cases/d3dx9load.txt") };
    let Ok(cases) = checked_cases(&text) else { panic!("cases/d3dx9load.txt does not parse") };
    let Some(case) = cases.iter().find(|c| c.name == "loadtex-dxt-negindex") else { panic!("no loadtex-dxt-negindex") };
    assert_eq!(cases.last().map(|c| c.name.as_str()), Some("loadtex-dxt-negindex"), "the crash probe is the suite's last case");
    set_arch(64);
    let (out, ended) = run_case_ended(case);
    assert_eq!(ended, Ended::Crash);
    assert_eq!(out, Ok(Vec::new()));
    set_arch(32);
    let (out, ended) = run_case_ended(case);
    assert_eq!(ended, Ended::Next);
    assert!(matches!(out, Err(Fail::Unmodeled(_))), "{out:?}");
}

proptest! {
    /// each build meets an index below 0 its own way, never the other's, and a block of one
    /// grey (no direction to fit along) never meets one
    #[test]
    fn each_build_its_own_below(v in proptest::collection::vec(0u8..=255, 64), g in 0u8..=255) {
        let px: [[f32; 4]; 16] = std::array::from_fn(|i| std::array::from_fn(|k| f32::from(v[i * 4 + k]) / 255.0));
        for steps in [Steps::Three, Steps::Four] {
            prop_assert_ne!(encode_bc1(&px, steps).err(), Some(Below::Unknown));
            prop_assert_ne!(encode_bc1(&x87(px), steps).err(), Some(Below::Fault));
        }
        let grey = [[f32::from(g) / 255.0, f32::from(g) / 255.0, f32::from(g) / 255.0, 1.0]; 16];
        prop_assert!(encode_bc1(&grey, Steps::Four).is_ok());
        prop_assert!(encode_bc1(&x87(grey), Steps::Four).is_ok());
    }
}
