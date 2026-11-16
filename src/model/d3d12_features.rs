//! CheckFeatureSupport of every feature on WARP (runner/d3d12.c featscan12): the size each
//! feature's structure succeeds at and the bytes it fills (MEASURED, golden d3d12-* featscan-*).

/// (feature, inputs, size, bytes as hex): the features that answer; the others fail
pub const ANSWERS: &[(u32, &[u32], u32, &str)] = &[];

/// the size and bytes of a feature's answer for these inputs, None where every size fails
pub fn answer(f: u32, inputs: &[u32]) -> Option<(u32, &'static str)> {
    ANSWERS.iter().find(|a| a.0 == f && a.1 == inputs).map(|a| (a.2, a.3))
}
