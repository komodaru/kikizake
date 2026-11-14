//! Direct3D 9 and 8 drawing (runner/d3d9draw.c): the tables the generators and the model share.

/// render target formats asked (D3DFORMAT, name)
pub const TARGET_FORMATS: &[(u32, &str)] = &[
    (20, "R8G8B8"), (21, "A8R8G8B8"), (22, "X8R8G8B8"), (23, "R5G6B5"), (24, "X1R5G5B5"), (25, "A1R5G5B5"), (26, "A4R4G4B4"), (27, "R3G3B2"), (28, "A8"), (29, "A8R3G3B2"),
    (30, "X4R4G4B4"), (31, "A2B10G10R10"), (32, "A8B8G8R8"), (33, "X8B8G8R8"), (34, "G16R16"), (35, "A2R10G10B10"), (36, "A16B16G16R16"), (50, "L8"), (51, "A8L8"), (60, "V8U8"),
    (81, "L16"), (111, "R16F"), (112, "G16R16F"), (113, "A16B16G16R16F"), (114, "R32F"), (115, "G32R32F"), (116, "A32B32G32R32F"),
];

pub const fn fourcc(s: &[u8; 4]) -> u32 {
    s[0] as u32 | (s[1] as u32) << 8 | (s[2] as u32) << 16 | (s[3] as u32) << 24
}

/// texture formats asked: the target formats and those only textures have
pub const TEXTURE_FORMATS: &[(u32, &str)] = &[
    (20, "R8G8B8"), (21, "A8R8G8B8"), (22, "X8R8G8B8"), (23, "R5G6B5"), (24, "X1R5G5B5"), (25, "A1R5G5B5"), (26, "A4R4G4B4"), (27, "R3G3B2"), (28, "A8"), (29, "A8R3G3B2"),
    (30, "X4R4G4B4"), (31, "A2B10G10R10"), (32, "A8B8G8R8"), (33, "X8B8G8R8"), (34, "G16R16"), (35, "A2R10G10B10"), (36, "A16B16G16R16"), (40, "A8P8"), (41, "P8"), (50, "L8"),
    (51, "A8L8"), (52, "A4L4"), (60, "V8U8"), (61, "L6V5U5"), (62, "X8L8V8U8"), (63, "Q8W8V8U8"), (64, "V16U16"), (67, "A2W10V10U10"), (81, "L16"), (110, "Q16W16V16U16"),
    (111, "R16F"), (112, "G16R16F"), (113, "A16B16G16R16F"), (114, "R32F"), (115, "G32R32F"), (116, "A32B32G32R32F"), (117, "CxV8U8"),
    (fourcc(b"DXT1"), "DXT1"), (fourcc(b"DXT2"), "DXT2"), (fourcc(b"DXT3"), "DXT3"), (fourcc(b"DXT4"), "DXT4"), (fourcc(b"DXT5"), "DXT5"), (fourcc(b"UYVY"), "UYVY"), (fourcc(b"YUY2"), "YUY2"),
];

/// D3DTEXTUREOP
pub const TOPS: &[(u32, &str)] = &[
    (1, "DISABLE"), (2, "SELECTARG1"), (3, "SELECTARG2"), (4, "MODULATE"), (5, "MODULATE2X"), (6, "MODULATE4X"), (7, "ADD"), (8, "ADDSIGNED"), (9, "ADDSIGNED2X"), (10, "SUBTRACT"),
    (11, "ADDSMOOTH"), (12, "BLENDDIFFUSEALPHA"), (13, "BLENDTEXTUREALPHA"), (14, "BLENDFACTORALPHA"), (15, "BLENDTEXTUREALPHAPM"), (16, "BLENDCURRENTALPHA"), (17, "PREMODULATE"),
    (18, "MODULATEALPHA_ADDCOLOR"), (19, "MODULATECOLOR_ADDALPHA"), (20, "MODULATEINVALPHA_ADDCOLOR"), (21, "MODULATEINVCOLOR_ADDALPHA"), (22, "BUMPENVMAP"), (23, "BUMPENVMAPLUMINANCE"),
    (24, "DOTPRODUCT3"), (25, "MULTIPLYADD"), (26, "LERP"),
];

/// D3DBLENDOP
pub const BLEND_OPS: &[(u32, &str)] = &[(1, "ADD"), (2, "SUBTRACT"), (3, "REVSUBTRACT"), (4, "MIN"), (5, "MAX")];
/// D3DCMPFUNC
pub const CMP: &[(u32, &str)] = &[(1, "NEVER"), (2, "LESS"), (3, "EQUAL"), (4, "LESSEQUAL"), (5, "GREATER"), (6, "NOTEQUAL"), (7, "GREATEREQUAL"), (8, "ALWAYS")];
/// D3DSTENCILOP
pub const STENCIL_OPS: &[(u32, &str)] = &[(1, "KEEP"), (2, "ZERO"), (3, "REPLACE"), (4, "INCRSAT"), (5, "DECRSAT"), (6, "INVERT"), (7, "INCR"), (8, "DECR")];

/// D3DDECLTYPE's size in bytes (D3DDECLTYPE): None past UNUSED
pub fn decl_type_size(t: u32) -> Option<u32> {
    Some(match t {
        0 | 4 | 5 | 6 | 8 | 9 | 11 | 13 | 14 | 15 => 4,
        1 | 7 | 10 | 12 | 16 => 8,
        2 => 12,
        3 => 16,
        17 => 0,
        _ => return None,
    })
}
