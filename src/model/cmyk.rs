//! The CMYK color step behind WIC's 32bppCMYK: windowscodecsext's NChannel converter hands
//! Windows' RSWOP.icm (perceptual, the source) and the sRGB profile to mscms, which builds an
//! icm32 transform: a 16^4 table of 16-bit RGB filled through RSWOP's A2B0 table (9^4 grid, Lab
//! out), Lab to XYZ, the sRGB inverse matrix and the inverse tone curve; each pixel then goes
//! through A2B0's input curves, a 4D simplex in that table and a linear output table.
//!
//! FOUND clean-room (windowscodecsext CFormatConverterNChannel::Initialize, Convert_32CMYK_32BGRA;
//! icm32 CMCreateMultiProfileTransform, MakeCube16, CalcNDim_Data8To8_Lut16,
//! Fill_ushort_ALUTs_from_lut16Tag, Lab2XYZ_forCube16, DoMatrixForCube16, the per-pixel
//! CalcNDim) and checked against an emulation of icm32 with the measured profile (331 random
//! colors, every one); confirmed by Windows (golden wic pair-cmyk-*, ramp-cmyk): spec/wic.txt
//! WIC.TO.CMYK. The RSWOP.icm tables are MEASURED (golden wiccolor profile-swop-<tag>), as the
//! user chose.

use std::sync::OnceLock;

use super::icm;

/// RSWOP.icm's large tags one after another, in `SwopTag` order (golden wiccolor
/// profile-swop-<tag>, the bytes as GetColorProfileElement gives them)
const RSWOP: &[u8] = include_bytes!("data/rswop.bin");

/// the RSWOP.icm tags the suite reads whole
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwopTag {
    A2B0,
    A2B1,
    A2B2,
    B2A0,
    B2A1,
    B2A2,
    Gamt,
}

impl SwopTag {
    pub const ALL: [SwopTag; 7] = [SwopTag::A2B0, SwopTag::A2B1, SwopTag::A2B2, SwopTag::B2A0, SwopTag::B2A1, SwopTag::B2A2, SwopTag::Gamt];

    pub fn name(self) -> &'static str {
        match self {
            SwopTag::A2B0 => "A2B0",
            SwopTag::A2B1 => "A2B1",
            SwopTag::A2B2 => "A2B2",
            SwopTag::B2A0 => "B2A0",
            SwopTag::B2A1 => "B2A1",
            SwopTag::B2A2 => "B2A2",
            SwopTag::Gamt => "gamt",
        }
    }

    pub fn parse(s: &str) -> Option<SwopTag> {
        SwopTag::ALL.into_iter().find(|t| t.name() == s)
    }

    /// the tag signature as a big-endian number
    pub fn sig(self) -> u32 {
        let n = self.name().as_bytes();
        u32::from_be_bytes([n[0], n[1], n[2], n[3]])
    }

    /// MEASURED (golden wiccolor profile-swop): the tag's size in bytes
    pub fn size(self) -> usize {
        match self {
            SwopTag::A2B0 | SwopTag::A2B1 | SwopTag::A2B2 => 41478,
            SwopTag::B2A0 | SwopTag::B2A1 | SwopTag::B2A2 => 40908,
            SwopTag::Gamt => 11418,
        }
    }

    pub fn bytes(self) -> &'static [u8] {
        let at: usize = SwopTag::ALL.iter().take_while(|&&t| t != self).map(|t| t.size()).sum();
        &RSWOP[at..at + self.size()]
    }
}

const _: () = assert!(RSWOP.len() == 3 * 41478 + 3 * 40908 + 11418);

/// MEASURED (golden wiccolor profile-swop): RSWOP.icm's header line and tag table as the runner
/// prints them, the tags read whole marked by their size
pub fn swop_profile() -> Vec<String> {
    [
        "wprofile 1 218103 02000000 70727472 434d594b 4c616220 0 16",
        "tag 64657363 64657363 117",
        "tag 63616c74 6474696d 20",
        "tag 63707274 74657874 71",
        "tag 77747074 58595a20 20 0000dc98 0000e4af 0000be3b",
        "tag 626b7074 58595a20 20 00000b81 00000afd 00000636",
        "tag 41324230 6d667432 41478",
        "tag 41324232 6d667432 41478",
        "tag 41324231 6d667432 41478",
        "tag 42324130 6d667432 40908",
        "tag 42324131 6d667432 40908",
        "tag 42324132 6d667432 40908",
        "tag 67616d74 6d667432 11418",
        "tag 74656368 73696720 12",
        "tag 646d6e64 64657363 101",
        "tag 646d6464 64657363 110",
        "tag 76756564 64657363 195",
    ]
    .map(String::from)
    .to_vec()
}

/// an ICC lut16Type ('mft2') with an identity matrix: input tables, the grid, output tables
struct Lut16 {
    inputs: usize,
    outputs: usize,
    grid: usize,
    intab: Vec<Vec<u16>>,
    clut: Vec<u16>,
    outtab: Vec<Vec<u16>>,
}

fn lut16(b: &[u8]) -> Option<Lut16> {
    if b.get(..4)? != b"mft2" {
        return None;
    }
    let (inputs, outputs, grid) = (usize::from(*b.get(8)?), usize::from(*b.get(9)?), usize::from(*b.get(10)?));
    let be = |o: usize| -> Option<u16> { Some(u16::from_be_bytes([*b.get(o)?, *b.get(o + 1)?])) };
    let (nin, nout) = (usize::from(be(48)?), usize::from(be(50)?));
    let words = |o: usize, n: usize| -> Option<Vec<u16>> { (0..n).map(|k| be(o + 2 * k)).collect() };
    let mut o = 52;
    let mut intab = Vec::new();
    for _ in 0..inputs {
        intab.push(words(o, nin)?);
        o += 2 * nin;
    }
    let cells = grid.checked_pow(u32::try_from(inputs).ok()?)? * outputs;
    let clut = words(o, cells)?;
    o += 2 * cells;
    let mut outtab = Vec::new();
    for _ in 0..outputs {
        outtab.push(words(o, nout)?);
        o += 2 * nout;
    }
    Some(Lut16 { inputs, outputs, grid, intab, clut, outtab })
}

/// Fill_ushort_ALUTs_from_lut16Tag: one output table resampled to 1024 entries in 15-bit
/// fractions, rounded
fn alut_lut16(t: &[u16]) -> Vec<u32> {
    let n = t.len() as i64;
    let step = ((n - 1) << 20) / 1023;
    (0..1024i64)
        .map(|j| {
            let pos = (j * step + 16) >> 5;
            let (k, fr) = ((pos >> 15) as usize, pos & 0x7fff);
            let y0 = i64::from(t[k]);
            let v = if fr == 0 { y0 } else { y0 + (((i64::from(t[k + 1]) - y0) * fr + 0x3fff) >> 15) };
            (v & 0xffff) as u32
        })
        .collect()
}

/// a 16-bit input interpolated in a 256-entry table in 8-bit fractions (v - v>>8 as 8.8),
/// past the last entry 0
fn input16(t: &[u32], v: u16) -> u32 {
    let v = u32::from(v);
    let d = v - (v >> 8);
    let (i, f) = ((d >> 8) as usize, d & 0xff);
    let next = if i < 255 { t[i + 1] * f } else { 0 };
    (t[i] * (256 - f) + next) >> 8
}

/// the 4D simplex: the vertices from `idx` one channel up at a time in falling fraction order
/// (clamped to `top`), the weights `one - f0`, `f0 - f1`, .., `f3`; `cell(v, ch)` a vertex's value
fn simplex4(idx: [usize; 4], fr: [u64; 4], one: u64, top: usize, cell: impl Fn([usize; 4], usize) -> u64) -> [u64; 3] {
    let mut order = [0usize, 1, 2, 3];
    order.sort_by(|&a, &b| fr[b].cmp(&fr[a]));
    let mut v = [idx; 5];
    for (k, &c) in order.iter().enumerate() {
        v[k + 1] = v[k];
        v[k + 1][c] = (v[k + 1][c] + 1).min(top);
    }
    let f = order.map(|c| fr[c]);
    let w = [one - f[0], f[0] - f[1], f[1] - f[2], f[2] - f[3], f[3]];
    std::array::from_fn(|ch| (0..5).filter(|&k| w[k] != 0).map(|k| w[k] * cell(v[k], ch)).sum())
}

/// a sum through the generic routine's output step: +s>>16, -s>>10, >>6 (32-bit), then the
/// 1024-entry table in 16-bit fractions, past the end of the whole table 0
fn ndim_out(s: u64, al: &[u32], ch: usize) -> u32 {
    let mut t = (s & 0xffff_ffff) as u32;
    t = t.wrapping_add(t >> 16);
    t -= t >> 10;
    t >>= 6;
    let (f, idx) = (t & 0xffff, (t >> 16) as usize);
    let at = 1024 * ch + idx;
    let next = al.get(at + 1).copied().unwrap_or(0);
    (al[at] * (65536 - f) + next * f) >> 16
}

/// Lab2XYZ_forCube16's table: 16383 times CIE Y of L = i*100/256 (the cube root part in
/// single precision), rounded; three entries are one lower in icm32 (FOUND clean-room: the
/// DLL's 432 entries, the formula giving each of the others)
fn lab_table() -> &'static [i64; 432] {
    static T: OnceLock<[i64; 432]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0i64; 432];
        for (i, v) in t.iter_mut().enumerate() {
            let l = i as f64 * 100.0 / 256.0;
            let fy = (l + 16.0) / 116.0;
            *v = if fy <= 6.0 / 29.0 {
                (16383.0 * l / (24389.0 / 27.0)).round() as i64
            } else {
                let f = i as f32 * (100.0f64 / 256.0 / 116.0) as f32 + (16.0f64 / 116.0) as f32;
                ((16383.0f32 * (f * f * f)) + 0.5) as i64
            }
            .min(65535);
        }
        for i in [34, 164, 167] {
            t[i] -= 1;
        }
        t
    })
}

/// Lab2XYZ_forCube16: Lab16 (L 0..0xffff, a and b offset 0x8000 in 1/256) to XYZ in 1.15
fn lab_to_xyz([l, a, b]: [u32; 3]) -> [u32; 3] {
    let t = lab_table();
    let look = |x: i64| -> i64 {
        if x < 0 {
            return 0;
        }
        let i = (x >> 8) as usize;
        if i >= 0x1af {
            return 0x1fffe;
        }
        2 * t[i] + (((t[i + 1] - t[i]) * (x & 0xff) + 0x3f) >> 7)
    };
    let (l, a, b) = (i64::from(l), i64::from(a), i64::from(b));
    let y = look(l);
    let vx = look(l - 0x4c00 + ((a * 19 + 15) >> 5));
    let vz = look(l - ((b * 0x5f + 0x1f) >> 6) + 0xbe00);
    let x = ((vx * 0x7b6b + 0x4000) >> 15).min(0xffff);
    let z = ((vz * 0x34cb + 0x2000) >> 14).min(0xffff);
    [x as u32, y as u32, z as u32]
}

/// the CMYK to RGB transform: A2B0's input curves (4 x 256) and the 16^4 table
pub struct CmykToRgb {
    e16: Vec<u32>,
    cube: Vec<[u32; 3]>,
}

impl CmykToRgb {
    fn new() -> Option<CmykToRgb> {
        let a2b = lut16(SwopTag::A2B0.bytes())?;
        if a2b.inputs != 4 || a2b.outputs != 3 || a2b.intab.iter().any(|t| t.len() != 256) {
            return None;
        }
        let g = a2b.grid;
        let al1: Vec<u32> = a2b.outtab.iter().flat_map(|t| alut_lut16(t)).collect();
        // (stage 1's input: the identity, descaled to the 9-point grid)
        let dl: Vec<u32> = (0..256u64).map(|i| ((i * 257 * 65536 / 65535) * (g as u64 - 1) / g as u64) as u32).collect();
        let srgb = icm::Space::Srgb.profile();
        let to_rgb = icm::inverse(&icm::half(&srgb));
        let alc = icm::alut_curve(icm::srgb_curve());
        let mut cube: Vec<[u32; 3]> = (0..1usize << 16)
            .map(|n| {
                let node: [u16; 4] = std::array::from_fn(|c| ((n >> (4 * (3 - c))) & 15) as u16 * 4369);
                let pos: [u64; 4] = node.map(|v| u64::from(input16(&dl, v)) * g as u64);
                let s = simplex4(pos.map(|p| (p >> 16) as usize), pos.map(|p| p & 0xffff), 65536, g - 1, |v, ch| {
                    u64::from(a2b.clut[3 * (((v[0] * g + v[1]) * g + v[2]) * g + v[3]) + ch])
                });
                lab_to_xyz(std::array::from_fn(|ch| ndim_out(s[ch], &al1, ch)))
            })
            .collect();
        icm::matrix_pass_to(&mut cube, &to_rgb, &alc);
        let e16 = a2b.intab.iter().flatten().map(|&v| u32::from(v)).collect();
        Some(CmykToRgb { e16, cube })
    }

    /// TranslateColors COLOR_CMYK to COLOR_RGB: 16-bit C, M, Y, K to 16-bit R, G, B
    pub fn rgb16(&self, cmyk: [u16; 4]) -> [u16; 3] {
        let g: [u64; 4] = std::array::from_fn(|c| {
            let e = u64::from(input16(&self.e16[256 * c..256 * c + 256], cmyk[c]));
            (e * 65536 / 65535) * 15 / 16
        });
        let s = simplex4(g.map(|v| (v >> 12) as usize), g.map(|v| v & 0xfff), 0x1000, 15, |v, ch| {
            u64::from(self.cube[((v[0] * 16 + v[1]) * 16 + v[2]) * 16 + v[3]][ch])
        });
        std::array::from_fn(|ch| {
            let al = |j: usize| icm::linear_alut(j as i64) as u32;
            let mut t = ((s[ch] * 16) & 0xffff_ffff) as u32;
            t = t.wrapping_add(t >> 16);
            t -= t >> 10;
            t >>= 6;
            let (f, idx) = (t & 0xffff, (t >> 16) as usize);
            let next = if idx < 1023 { al(idx + 1) } else { 0 };
            ((al(idx) * (65536 - f) + next * f) >> 16) as u16
        })
    }

    /// Convert_32CMYK_32BGRA: 8-bit C, M, Y, K (each x257) to B, G, R (each /257, truncated)
    pub fn bgr8(&self, [c, m, y, k]: [u8; 4]) -> [u8; 3] {
        let [r, g, b] = self.rgb16([c, m, y, k].map(|v| u16::from(v) * 257));
        [b, g, r].map(|v| (v / 257) as u8)
    }
}

/// the transform, built once (None: the measured table is not a 4-in 3-out lut16)
pub fn to_rgb() -> Option<&'static CmykToRgb> {
    static T: OnceLock<Option<CmykToRgb>> = OnceLock::new();
    T.get_or_init(CmykToRgb::new).as_ref()
}
