//! The DXGI formats Direct3D 10 and 11 know: number, name, bits per element, block size, and
//! what each component is (Microsoft Learn "DXGI_FORMAT", "Format Support for Direct3D Feature
//! Level 11.0 Hardware").

/// how a component's bits mean a number
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Num {
    Typeless,
    Unorm,
    Snorm,
    Uint,
    Sint,
    Float,
    Srgb,
    /// a format of its own layout (shared exponent, packed floats, depth/stencil, video, ...)
    Other,
}
use Num::*;

#[derive(Clone, Copy, Debug)]
pub struct Format {
    pub id: u32,
    pub name: &'static str,
    /// bits per element (an element: a pixel, a 4x4 block, a 2x1 pair)
    pub bits: u32,
    /// the element's width and height in pixels
    pub bw: u32,
    pub bh: u32,
    pub num: Num,
    /// the components' bit counts, in memory order from bit 0 (R G B A, or B G R A ...)
    pub comps: &'static [u32],
    /// which channel (0 r, 1 g, 2 b, 3 a) each component holds
    pub chans: &'static [u8],
}

const RGBA: &[u8] = &[0, 1, 2, 3];
const BGRA: &[u8] = &[2, 1, 0, 3];

macro_rules! f {
    ($id:expr, $name:expr, $bits:expr, $num:expr) => {
        Format { id: $id, name: $name, bits: $bits, bw: 1, bh: 1, num: $num, comps: &[], chans: RGBA }
    };
    ($id:expr, $name:expr, $bits:expr, $num:expr, $comps:expr) => {
        Format { id: $id, name: $name, bits: $bits, bw: 1, bh: 1, num: $num, comps: $comps, chans: RGBA }
    };
    ($id:expr, $name:expr, $bits:expr, $num:expr, $comps:expr, $chans:expr) => {
        Format { id: $id, name: $name, bits: $bits, bw: 1, bh: 1, num: $num, comps: $comps, chans: $chans }
    };
}
macro_rules! bc {
    ($id:expr, $name:expr, $bits:expr, $num:expr) => {
        Format { id: $id, name: $name, bits: $bits, bw: 4, bh: 4, num: $num, comps: &[], chans: RGBA }
    };
}

pub const FORMATS: &[Format] = &[
    f!(1, "R32G32B32A32_TYPELESS", 128, Typeless, &[32, 32, 32, 32]),
    f!(2, "R32G32B32A32_FLOAT", 128, Float, &[32, 32, 32, 32]),
    f!(3, "R32G32B32A32_UINT", 128, Uint, &[32, 32, 32, 32]),
    f!(4, "R32G32B32A32_SINT", 128, Sint, &[32, 32, 32, 32]),
    f!(5, "R32G32B32_TYPELESS", 96, Typeless, &[32, 32, 32]),
    f!(6, "R32G32B32_FLOAT", 96, Float, &[32, 32, 32]),
    f!(7, "R32G32B32_UINT", 96, Uint, &[32, 32, 32]),
    f!(8, "R32G32B32_SINT", 96, Sint, &[32, 32, 32]),
    f!(9, "R16G16B16A16_TYPELESS", 64, Typeless, &[16, 16, 16, 16]),
    f!(10, "R16G16B16A16_FLOAT", 64, Float, &[16, 16, 16, 16]),
    f!(11, "R16G16B16A16_UNORM", 64, Unorm, &[16, 16, 16, 16]),
    f!(12, "R16G16B16A16_UINT", 64, Uint, &[16, 16, 16, 16]),
    f!(13, "R16G16B16A16_SNORM", 64, Snorm, &[16, 16, 16, 16]),
    f!(14, "R16G16B16A16_SINT", 64, Sint, &[16, 16, 16, 16]),
    f!(15, "R32G32_TYPELESS", 64, Typeless, &[32, 32]),
    f!(16, "R32G32_FLOAT", 64, Float, &[32, 32]),
    f!(17, "R32G32_UINT", 64, Uint, &[32, 32]),
    f!(18, "R32G32_SINT", 64, Sint, &[32, 32]),
    f!(19, "R32G8X24_TYPELESS", 64, Typeless),
    f!(20, "D32_FLOAT_S8X24_UINT", 64, Other),
    f!(21, "R32_FLOAT_X8X24_TYPELESS", 64, Other),
    f!(22, "X32_TYPELESS_G8X24_UINT", 64, Other),
    f!(23, "R10G10B10A2_TYPELESS", 32, Typeless, &[10, 10, 10, 2]),
    f!(24, "R10G10B10A2_UNORM", 32, Unorm, &[10, 10, 10, 2]),
    f!(25, "R10G10B10A2_UINT", 32, Uint, &[10, 10, 10, 2]),
    f!(26, "R11G11B10_FLOAT", 32, Other, &[11, 11, 10]),
    f!(27, "R8G8B8A8_TYPELESS", 32, Typeless, &[8, 8, 8, 8]),
    f!(28, "R8G8B8A8_UNORM", 32, Unorm, &[8, 8, 8, 8]),
    f!(29, "R8G8B8A8_UNORM_SRGB", 32, Srgb, &[8, 8, 8, 8]),
    f!(30, "R8G8B8A8_UINT", 32, Uint, &[8, 8, 8, 8]),
    f!(31, "R8G8B8A8_SNORM", 32, Snorm, &[8, 8, 8, 8]),
    f!(32, "R8G8B8A8_SINT", 32, Sint, &[8, 8, 8, 8]),
    f!(33, "R16G16_TYPELESS", 32, Typeless, &[16, 16]),
    f!(34, "R16G16_FLOAT", 32, Float, &[16, 16]),
    f!(35, "R16G16_UNORM", 32, Unorm, &[16, 16]),
    f!(36, "R16G16_UINT", 32, Uint, &[16, 16]),
    f!(37, "R16G16_SNORM", 32, Snorm, &[16, 16]),
    f!(38, "R16G16_SINT", 32, Sint, &[16, 16]),
    f!(39, "R32_TYPELESS", 32, Typeless, &[32]),
    f!(40, "D32_FLOAT", 32, Other),
    f!(41, "R32_FLOAT", 32, Float, &[32]),
    f!(42, "R32_UINT", 32, Uint, &[32]),
    f!(43, "R32_SINT", 32, Sint, &[32]),
    f!(44, "R24G8_TYPELESS", 32, Typeless),
    f!(45, "D24_UNORM_S8_UINT", 32, Other),
    f!(46, "R24_UNORM_X8_TYPELESS", 32, Other),
    f!(47, "X24_TYPELESS_G8_UINT", 32, Other),
    f!(48, "R8G8_TYPELESS", 16, Typeless, &[8, 8]),
    f!(49, "R8G8_UNORM", 16, Unorm, &[8, 8]),
    f!(50, "R8G8_UINT", 16, Uint, &[8, 8]),
    f!(51, "R8G8_SNORM", 16, Snorm, &[8, 8]),
    f!(52, "R8G8_SINT", 16, Sint, &[8, 8]),
    f!(53, "R16_TYPELESS", 16, Typeless, &[16]),
    f!(54, "R16_FLOAT", 16, Float, &[16]),
    f!(55, "D16_UNORM", 16, Other),
    f!(56, "R16_UNORM", 16, Unorm, &[16]),
    f!(57, "R16_UINT", 16, Uint, &[16]),
    f!(58, "R16_SNORM", 16, Snorm, &[16]),
    f!(59, "R16_SINT", 16, Sint, &[16]),
    f!(60, "R8_TYPELESS", 8, Typeless, &[8]),
    f!(61, "R8_UNORM", 8, Unorm, &[8]),
    f!(62, "R8_UINT", 8, Uint, &[8]),
    f!(63, "R8_SNORM", 8, Snorm, &[8]),
    f!(64, "R8_SINT", 8, Sint, &[8]),
    f!(65, "A8_UNORM", 8, Unorm, &[8], &[3]),
    Format { id: 66, name: "R1_UNORM", bits: 8, bw: 8, bh: 1, num: Other, comps: &[], chans: RGBA },
    f!(67, "R9G9B9E5_SHAREDEXP", 32, Other),
    Format { id: 68, name: "R8G8_B8G8_UNORM", bits: 32, bw: 2, bh: 1, num: Other, comps: &[], chans: RGBA },
    Format { id: 69, name: "G8R8_G8B8_UNORM", bits: 32, bw: 2, bh: 1, num: Other, comps: &[], chans: RGBA },
    bc!(70, "BC1_TYPELESS", 64, Typeless),
    bc!(71, "BC1_UNORM", 64, Other),
    bc!(72, "BC1_UNORM_SRGB", 64, Other),
    bc!(73, "BC2_TYPELESS", 128, Typeless),
    bc!(74, "BC2_UNORM", 128, Other),
    bc!(75, "BC2_UNORM_SRGB", 128, Other),
    bc!(76, "BC3_TYPELESS", 128, Typeless),
    bc!(77, "BC3_UNORM", 128, Other),
    bc!(78, "BC3_UNORM_SRGB", 128, Other),
    bc!(79, "BC4_TYPELESS", 64, Typeless),
    bc!(80, "BC4_UNORM", 64, Other),
    bc!(81, "BC4_SNORM", 64, Other),
    bc!(82, "BC5_TYPELESS", 128, Typeless),
    bc!(83, "BC5_UNORM", 128, Other),
    bc!(84, "BC5_SNORM", 128, Other),
    f!(85, "B5G6R5_UNORM", 16, Unorm, &[5, 6, 5], BGRA),
    f!(86, "B5G5R5A1_UNORM", 16, Unorm, &[5, 5, 5, 1], BGRA),
    f!(87, "B8G8R8A8_UNORM", 32, Unorm, &[8, 8, 8, 8], BGRA),
    f!(88, "B8G8R8X8_UNORM", 32, Unorm, &[8, 8, 8, 8], BGRA),
    f!(89, "R10G10B10_XR_BIAS_A2_UNORM", 32, Other, &[10, 10, 10, 2]),
    f!(90, "B8G8R8A8_TYPELESS", 32, Typeless, &[8, 8, 8, 8], BGRA),
    f!(91, "B8G8R8A8_UNORM_SRGB", 32, Srgb, &[8, 8, 8, 8], BGRA),
    f!(92, "B8G8R8X8_TYPELESS", 32, Typeless, &[8, 8, 8, 8], BGRA),
    f!(93, "B8G8R8X8_UNORM_SRGB", 32, Srgb, &[8, 8, 8, 8], BGRA),
    bc!(94, "BC6H_TYPELESS", 128, Typeless),
    bc!(95, "BC6H_UF16", 128, Other),
    bc!(96, "BC6H_SF16", 128, Other),
    bc!(97, "BC7_TYPELESS", 128, Typeless),
    bc!(98, "BC7_UNORM", 128, Other),
    bc!(99, "BC7_UNORM_SRGB", 128, Other),
    f!(100, "AYUV", 32, Other),
    f!(101, "Y410", 32, Other),
    f!(102, "Y416", 64, Other),
    f!(103, "NV12", 8, Other),
    f!(104, "P010", 16, Other),
    f!(105, "P016", 16, Other),
    f!(106, "420_OPAQUE", 8, Other),
    Format { id: 107, name: "YUY2", bits: 32, bw: 2, bh: 1, num: Other, comps: &[], chans: RGBA },
    Format { id: 108, name: "Y210", bits: 64, bw: 2, bh: 1, num: Other, comps: &[], chans: RGBA },
    Format { id: 109, name: "Y216", bits: 64, bw: 2, bh: 1, num: Other, comps: &[], chans: RGBA },
    f!(110, "NV11", 8, Other),
    f!(111, "AI44", 8, Other),
    f!(112, "IA44", 8, Other),
    f!(113, "P8", 8, Other),
    f!(114, "A8P8", 16, Other),
    f!(115, "B4G4R4A4_UNORM", 16, Unorm, &[4, 4, 4, 4], BGRA),
    f!(130, "P208", 8, Other),
    f!(131, "V208", 8, Other),
    f!(132, "V408", 8, Other),
];

pub fn format(id: u32) -> Option<&'static Format> {
    FORMATS.iter().find(|f| f.id == id)
}

impl Format {
    /// bytes of one row of elements of a `w` pixel wide picture
    pub fn row_bytes(&self, w: u32) -> u32 {
        w.div_ceil(self.bw) * self.bits / 8
    }
    /// rows of elements of an `h` pixel high picture
    pub fn rows(&self, h: u32) -> u32 {
        h.div_ceil(self.bh)
    }
    pub fn block(&self) -> bool {
        self.bw == 4
    }
    /// planar and other video formats (their layout is not elements of `bits`)
    pub fn video(&self) -> bool {
        (100..=114).contains(&self.id) || self.id >= 130
    }
}
