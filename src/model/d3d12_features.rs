//! CheckFeatureSupport of every feature on WARP (runner/d3d12.c featscan12): the size each
//! feature's structure succeeds at and the bytes it fills (MEASURED, golden d3d12-* featscan-*;
//! the structures are d3d12.h's D3D12_FEATURE_DATA_*, DirectX-Specs' feature pages).

/// which processes an answer is for: the pointer-sized fields (D3D12_FEATURE_DATA_PROTECTED_
/// RESOURCE_SESSION_TYPES) and the virtual address bits differ by process
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Arch {
    Both,
    X86,
    X64,
}

impl Arch {
    fn holds(self, arch: u32) -> bool {
        match self {
            Arch::Both => true,
            Arch::X86 => arch == 32,
            Arch::X64 => arch == 64,
        }
    }
}

/// (feature, inputs, processes, size, bytes as hex): the features that answer; the others fail
pub const ANSWERS: &[(u32, &[u32], Arch, u32, &str)] = &[
    (0, &[], Arch::X86, 0x3c, "0100000001000000030000000300000003000000010000000100000001000000030000001f0000000100000000000000010000000100000002000000"),
    (0, &[], Arch::X64, 0x3c, "010000000100000003000000030000000300000001000000010000000100000003000000200000000100000000000000010000000100000002000000"),
    (1, &[], Arch::Both, 0x10, "00000000000000000100000001000000"),
    (6, &[], Arch::X86, 0x8, "1f0000001f000000"),
    (6, &[], Arch::X64, 0x8, "200000002f000000"),
    (7, &[0x69], Arch::Both, 0x4, "68000000"),
    (7, &[0x60], Arch::Both, 0x4, "60000000"),
    (8, &[], Arch::Both, 0x18, "010000000400000004000000100000000100000001000000"),
    (10, &[], Arch::Both, 0x8, "0000000000000000"),
    (12, &[0x3], Arch::Both, 0x4, "03000000"),
    (12, &[0x1], Arch::Both, 0x4, "01000000"),
    (16, &[], Arch::Both, 0x14, "0000000000000000010000000100000000000000"),
    (17, &[], Arch::Both, 0x4, "00000000"),
    (18, &[], Arch::Both, 0x8, "0100000002000000"),
    (19, &[], Arch::Both, 0x4, "63000000"),
    (20, &[0x0, 0x64], Arch::Both, 0xc, "000000006400000001000000"),
    (20, &[0x0, 0x2710], Arch::Both, 0xc, "000000001027000000000000"),
    (20, &[0x2, 0x0], Arch::Both, 0xc, "020000000000000001000000"),
    (21, &[], Arch::Both, 0x14, "01000000010000000f0000000100000001000000"),
    (22, &[], Arch::Both, 0x4, "01000000"),
    (23, &[], Arch::Both, 0xc, "010000000200000001000000"),
    (24, &[], Arch::Both, 0x8, "000000000a000000"),
    (25, &[], Arch::Both, 0x8, "0000000000000000"),
    (27, &[], Arch::Both, 0xc, "01000000000000000b000000"),
    (28, &[], Arch::Both, 0x8, "0000000000000000"),
    (30, &[], Arch::Both, 0x14, "0100000000000000010000000000000000000000"),
    (32, &[], Arch::Both, 0x8, "0a00000064000000"),
    (33, &[], Arch::Both, 0x8, "0000000000000000"),
    (34, &[], Arch::X86, 0xc, "000000000000000000000000"),
    (34, &[], Arch::X64, 0x10, "00000000000000000000000000000000"),
    (36, &[], Arch::Both, 0x4, "01000000"),
    (37, &[], Arch::Both, 0x18, "010000000100000001000000010000000100000000000000"),
    (39, &[], Arch::Both, 0x8, "0000000000000000"),
    (40, &[], Arch::Both, 0x4, "01000000"),
    (41, &[], Arch::Both, 0xc, "000000000100000001000000"),
    (42, &[], Arch::Both, 0x18, "010000000100000001000000000000000100000001000000"),
    (43, &[], Arch::Both, 0xc, "010000000100000001000000"),
    (44, &[], Arch::Both, 0x8, "0100000001000000"),
    (45, &[], Arch::Both, 0x8, "0100000001000000"),
    (46, &[], Arch::Both, 0x8, "0100000000000000"),
    (47, &[], Arch::Both, 0x4, "01000000"),
    (48, &[], Arch::Both, 0x28, "010000001f0000000100000001000000010000000100000000002000000020000000200000000000"),
    (49, &[], Arch::Both, 0x8, "0000000000000000"),
    (53, &[], Arch::Both, 0x10, "0a0000000b0000000100000001000000"),
    (54, &[], Arch::Both, 0x4, "00000000"),
    (55, &[], Arch::X86, 0x10, "00000000000000000000000000000000"),
    (55, &[], Arch::X64, 0x18, "000000000000000000000000000000000000000000000000"),
    (56, &[], Arch::Both, 0x4, "00000000"),
    (57, &[], Arch::Both, 0x4, "01000000"),
    (58, &[], Arch::Both, 0x4, "00000000"),
];

/// the features failing with E_UNEXPECTED rather than E_INVALIDARG (D3D12_FEATURE_PREDICATION 50
/// and 52 on WARP)
pub const UNEXPECTED: &[u32] = &[50, 52];

/// the size and bytes of a feature's answer for these inputs, None where every size fails
pub fn answer(f: u32, inputs: &[u32]) -> Option<(u32, &'static str)> {
    let arch = super::arch();
    ANSWERS.iter().find(|a| a.0 == f && a.1 == inputs && a.2.holds(arch)).map(|a| (a.3, a.4))
}
