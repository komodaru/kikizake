//! DXGI_FORMAT: the layout of each format (dxgiformat.h and "DXGI_FORMAT" on Microsoft Learn),
//! shared by the Direct3D 12 and DXGI models and generators.

/// a channel's encoding
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Enc {
    Float,
    Unorm,
    Snorm,
    Srgb,
    Uint,
    Sint,
    /// X: not used
    Unused,
    /// X of B8G8R8X8_UNORM: WARP writes the alpha there (measured)
    Alpha,
}

/// one channel of a texel: which of the clear color's r g b a (0..3) it holds, its bit offset in
/// the texel (little-endian), its width and its encoding
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chan {
    pub src: usize,
    pub at: u32,
    pub bits: u32,
    pub enc: Enc,
}

/// bytes per block and the block's width and height in texels (1x1 for plain formats); None for
/// UNKNOWN, the planar (depth stencil and video), the opaque and the unassigned values
pub fn block(f: u32) -> Option<(u32, u32, u32)> {
    Some(match f {
        1..=4 => (16, 1, 1),
        5..=8 => (12, 1, 1),
        9..=18 => (8, 1, 1),
        23..=43 => (4, 1, 1),
        48..=59 => (2, 1, 1),
        60..=65 => (1, 1, 1),
        67 => (4, 1, 1),
        68 | 69 => (4, 2, 1),
        70..=72 | 79..=81 => (8, 4, 4),
        73..=78 | 82..=84 | 94..=99 => (16, 4, 4),
        85 | 86 | 115 | 191 => (2, 1, 1),
        87..=93 => (4, 1, 1),
        100 | 101 => (4, 1, 1),
        102 => (8, 1, 1),
        107 => (4, 2, 1),
        108 | 109 => (8, 2, 1),
        111..=113 => (1, 1, 1),
        114 => (2, 1, 1),
        _ => return None,
    })
}

/// the depth stencil formats Direct3D 12 keeps as two planes (depth, stencil) and the typeless
/// formats of their family
pub fn planar_depth(f: u32) -> bool {
    matches!(f, 19..=22 | 44..=47)
}

fn rgba(bits: [u32; 4], enc: Enc) -> Vec<Chan> {
    let mut v = Vec::new();
    let mut at = 0;
    for (src, b) in bits.into_iter().enumerate() {
        if b > 0 {
            v.push(Chan { src, at, bits: b, enc });
            at += b;
        }
    }
    v
}

/// the channels of a color format a render target can be cleared in (the formats of "Format
/// support for Direct3D feature level 11.0 hardware" with render target support, required or
/// optional); None for others
pub fn channels(f: u32) -> Option<Vec<Chan>> {
    use Enc::*;
    let typed = |base: u32| -> Enc { [Float, Uint, Sint][(f - base) as usize] };
    Some(match f {
        2..=4 => rgba([32; 4], typed(2)),
        6..=8 => rgba([32, 32, 32, 0], typed(6)),
        10..=14 => rgba([16; 4], [Float, Unorm, Uint, Snorm, Sint][(f - 10) as usize]),
        16..=18 => rgba([32, 32, 0, 0], typed(16)),
        24 | 25 => rgba([10, 10, 10, 2], if f == 24 { Unorm } else { Uint }),
        26 => vec![Chan { src: 0, at: 0, bits: 11, enc: Float }, Chan { src: 1, at: 11, bits: 11, enc: Float }, Chan { src: 2, at: 22, bits: 10, enc: Float }],
        28..=32 => {
            let e = [Unorm, Srgb, Uint, Snorm, Sint][(f - 28) as usize];
            let mut v = rgba([8; 4], e);
            if e == Srgb {
                v[3].enc = Unorm;
            }
            v
        }
        34..=38 => rgba([16, 16, 0, 0], [Float, Unorm, Uint, Snorm, Sint][(f - 34) as usize]),
        41..=43 => rgba([32, 0, 0, 0], typed(41)),
        49..=52 => rgba([8, 8, 0, 0], [Unorm, Uint, Snorm, Sint][(f - 49) as usize]),
        54 | 56..=59 => rgba([16, 0, 0, 0], [Float, Unorm, Unorm, Uint, Snorm, Sint][(f - 54) as usize]),
        61..=64 => rgba([8, 0, 0, 0], [Unorm, Uint, Snorm, Sint][(f - 61) as usize]),
        65 => vec![Chan { src: 3, at: 0, bits: 8, enc: Unorm }],
        // (B5G6R5: blue in the low bits)
        85 => vec![Chan { src: 2, at: 0, bits: 5, enc: Unorm }, Chan { src: 1, at: 5, bits: 6, enc: Unorm }, Chan { src: 0, at: 11, bits: 5, enc: Unorm }],
        86 => vec![Chan { src: 2, at: 0, bits: 5, enc: Unorm }, Chan { src: 1, at: 5, bits: 5, enc: Unorm }, Chan { src: 0, at: 10, bits: 5, enc: Unorm }, Chan { src: 3, at: 15, bits: 1, enc: Unorm }],
        87 | 88 | 91 | 93 => {
            let e = if f >= 91 { Srgb } else { Unorm };
            let a = match f {
                88 => Alpha,
                93 => Unused,
                _ => Unorm,
            };
            vec![Chan { src: 2, at: 0, bits: 8, enc: e }, Chan { src: 1, at: 8, bits: 8, enc: e }, Chan { src: 0, at: 16, bits: 8, enc: e }, Chan { src: 3, at: 24, bits: 8, enc: a }]
        }
        115 => vec![Chan { src: 2, at: 0, bits: 4, enc: Unorm }, Chan { src: 1, at: 4, bits: 4, enc: Unorm }, Chan { src: 0, at: 8, bits: 4, enc: Unorm }, Chan { src: 3, at: 12, bits: 4, enc: Unorm }],
        191 => vec![Chan { src: 3, at: 0, bits: 4, enc: Unorm }, Chan { src: 2, at: 4, bits: 4, enc: Unorm }, Chan { src: 1, at: 8, bits: 4, enc: Unorm }, Chan { src: 0, at: 12, bits: 4, enc: Unorm }],
        _ => return None,
    })
}

/// the depth formats a depth stencil view can be cleared in
pub const DEPTH: &[u32] = &[20, 40, 45, 55];

/// the formats a flip-model swap chain takes (DXGI_SWAP_CHAIN_DESC1)
pub const FLIP: &[u32] = &[10, 24, 28, 87];

/// the texture dimensions a format takes, bit 0 1D, bit 1 2D, bit 2 3D (D3D12_FORMAT_SUPPORT1_TEXTURE1D
/// and the others; MEASURED on WARP by GetCopyableFootprints, golden d3d12-32 fp-format-*, fp-blocks-*,
/// fp-random-*, and d3d12-64 create-split: of the video and palette formats only AYUV, Y416, NV12, P010,
/// P016, YUY2, Y210, Y216, in 2D)
pub fn dims(f: u32) -> Option<u32> {
    Some(match f {
        19..=22 | 40 | 44..=47 | 55 => 0b011,
        1..=65 | 67..=69 | 85..=88 | 90..=93 | 115 | 191 => 0b111,
        70..=84 | 94..=99 => 0b110,
        100 | 102..=105 | 107..=109 => 0b010,
        _ => 0,
    })
}

pub const T1: u32 = 1;
pub const T2: u32 = 2;
pub const T3: u32 = 4;
/// render target
pub const RT: u32 = 8;
/// depth stencil (the format's family has a depth format)
pub const DS: u32 = 16;
/// multisampled render target
pub const MS: u32 = 32;
/// one mip level only, arrays taken (the video formats)
pub const SINGLE: u32 = 64;

/// what CreateCommittedResource takes a format for on WARP (MEASURED, golden d3d12-64 create-split
/// for the video formats at 64x32, and golden d3d12-32
/// create-tex1d-format, create-tex2d-format, create-tex3d-format, create-tex2drt-format,
/// create-depth-format, create-msaa-format: every value 0..132, 189..192 and 0xffffffff for each)
pub fn support(f: u32) -> u32 {
    let color = T1 | T2 | T3 | RT | MS;
    match f {
        1..=18 | 23..=38 | 48..=52 | 60..=65 | 67 | 85..=88 | 90..=93 | 115 | 191 => color,
        39 | 41..=43 | 53 | 54 | 56..=59 => color | DS,
        40 | 55 => T1 | T2 | RT | MS | DS,
        // (multisampled depth: MEASURED for D24_UNORM_S8_UINT, golden d3d12-64 alloc-other; assumed
        // for the others of the two families)
        19..=22 | 44..=47 => T1 | T2 | DS | MS,
        68 | 69 => T1 | T2 | T3,
        70..=84 | 94..=99 => T2 | T3,
        89 => RT,
        100 => T2 | RT,
        102 => T2,
        103..=105 => T2 | RT | SINGLE,
        106..=109 => T2 | SINGLE,
        _ => 0,
    }
}
