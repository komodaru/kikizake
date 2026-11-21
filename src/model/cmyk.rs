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

/// the simplex in D dimensions: the vertices from `idx` one channel up at a time in falling
/// fraction order (clamped to `top`), the weights `one - f0`, `f0 - f1`, .., the last fraction;
/// `cell(v, ch)` a vertex's value for each of the O outputs
fn simplex<const D: usize, const O: usize>(idx: [usize; D], fr: [u64; D], one: u64, top: usize, cell: impl Fn([usize; D], usize) -> u64) -> [u64; O] {
    let mut order: [usize; D] = std::array::from_fn(|k| k);
    order.sort_by(|&a, &b| fr[b].cmp(&fr[a]));
    let mut v = vec![idx; D + 1];
    for (k, &c) in order.iter().enumerate() {
        v[k + 1] = v[k];
        v[k + 1][c] = (v[k + 1][c] + 1).min(top);
    }
    let f = order.map(|c| fr[c]);
    let w: Vec<u64> = (0..=D).map(|k| if k == 0 { one - f[0] } else if k == D { f[D - 1] } else { f[k - 1] - f[k] }).collect();
    std::array::from_fn(|ch| (0..=D).filter(|&k| w[k] != 0).map(|k| w[k] * cell(v[k], ch)).sum())
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

/// FillDescaledInputLut: each 256-entry input curve made 16.16 grid positions' fractions
/// ((v * 65536 / 65535) * (g - 1) / g), one after another
fn descaled(curves: &[Vec<u16>], g: usize) -> Vec<u32> {
    let g = g as u64;
    curves.iter().flatten().map(|&v| ((u64::from(v) * 65536 / 65535) * (g - 1) / g) as u32).collect()
}

/// CalcNDim_Data8To8_Lut16 on a cube node: each 16-bit input through its descaled curve, times
/// the grid size (index 16.16), the simplex in the lut16's grid (weights summing to 65536), the
/// generic output step through `al`
fn grid_stage<const D: usize, const O: usize>(lut: &Lut16, dl: &[u32], al: &[u32], node: [u16; D]) -> [u32; O] {
    let g = lut.grid;
    let pos: [u64; D] = std::array::from_fn(|c| u64::from(input16(&dl[256 * c..256 * c + 256], node[c])) * g as u64);
    let s = simplex::<D, O>(pos.map(|p| (p >> 16) as usize), pos.map(|p| p & 0xffff), 65536, g - 1, |v, ch| {
        u64::from(lut.clut[O * v.iter().fold(0, |at, &i| at * g + i) + ch])
    });
    std::array::from_fn(|ch| ndim_out(s[ch], al, ch))
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
        let al1: Vec<u32> = a2b.outtab.iter().flat_map(|t| alut_lut16(t)).collect();
        // (stage 1's input: the identity, descaled to the grid; A2B0's own input curves are
        // the per-pixel ones)
        let identity: Vec<u16> = (0..256u16).map(|i| i * 257).collect();
        let dl = descaled(&vec![identity; 4], a2b.grid);
        let srgb = icm::Space::Srgb.profile();
        let to_rgb = icm::inverse(&icm::half(&srgb));
        let alc = icm::alut_curve(icm::srgb_curve());
        let mut cube: Vec<[u32; 3]> = (0..1usize << 16)
            .map(|n| {
                let node: [u16; 4] = std::array::from_fn(|c| ((n >> (4 * (3 - c))) & 15) as u16 * 4369);
                lab_to_xyz(grid_stage::<4, 3>(&a2b, &dl, &al1, node))
            })
            .collect();
        icm::matrix_pass_to(&mut cube, &to_rgb, &icm::LINEAR_ELUT, &alc);
        let e16 = a2b.intab.iter().flatten().map(|&v| u32::from(v)).collect();
        Some(CmykToRgb { e16, cube })
    }

    /// TranslateColors COLOR_CMYK to COLOR_RGB: 16-bit C, M, Y, K to 16-bit R, G, B
    pub fn rgb16(&self, cmyk: [u16; 4]) -> [u16; 3] {
        let g: [u64; 4] = std::array::from_fn(|c| {
            let e = u64::from(input16(&self.e16[256 * c..256 * c + 256], cmyk[c]));
            (e * 65536 / 65535) * 15 / 16
        });
        let s = simplex::<4, 3>(g.map(|v| (v >> 12) as usize), g.map(|v| v & 0xfff), 0x1000, 15, |v, ch| {
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

/// the RGB to CMYK transform (the sRGB profile into RSWOP.icm, B2A0): a 32^3 table of 16-bit
/// C, M, Y, K filled through the sRGB tone curve and colorants, XYZ to Lab and B2A0 (17^3 grid,
/// its input curves descaled into the stage, a linear output table); per pixel the linear
/// input, the 3D simplex (11-bit fractions) and B2A0's output curves at 1024 entries
pub struct RgbToCmyk {
    elut: [u32; 256],
    cube: Vec<[u32; 4]>,
    alut: Vec<u32>,
}

/// XYZ2Lab_forCube16's table: 32767 times CIE L*/100 of Y = i/256, rounded; six entries differ
/// by one in icm32 (FOUND clean-room: the DLL's 432 entries, the formula giving each of the
/// others)
fn lab_l_table() -> &'static [i64; 432] {
    static T: OnceLock<[i64; 432]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0i64; 432];
        for (i, v) in t.iter_mut().enumerate() {
            let y = i as f64 / 256.0;
            let l = if y <= 216.0 / 24389.0 { 24389.0 / 27.0 * y } else { 116.0 * y.cbrt() - 16.0 };
            *v = ((32767.0 * l / 100.0).round() as i64).min(65535);
        }
        for i in [106, 280, 350, 358] {
            t[i] -= 1;
        }
        for i in [415, 420] {
            t[i] += 1;
        }
        t
    })
}

/// XYZ2Lab_forCube16: XYZ in 1.15 (X and Z first scaled by 1/0.9642 and 1/0.8249 in 12-bit
/// fractions) to Lab16; past the table 0x9c27
fn xyz_to_lab([x, y, z]: [u32; 3]) -> [u32; 3] {
    let t = lab_l_table();
    let look = |v: i64, sh: u32, half: i64| -> i64 {
        let i = (v >> sh) as usize;
        if i >= 0x1af {
            return 0x9c27;
        }
        t[i] + (((t[i + 1] - t[i]) * (v & ((1 << sh) - 1)) + half) >> sh)
    };
    let (x, y, z) = (i64::from(x), i64::from(y), i64::from(z));
    let fy = look(y, 7, 0x3f);
    let fx = look((x * 0x213) >> 4, 12, 0x7ff);
    let fz = look((z * 0x26cb) >> 8, 12, 0x7ff);
    let a = (((fx - fy) * 0x1af + 0x40_0040) >> 7).clamp(0, 0xffff);
    let b = (((fy - fz) * 0x2b1b + 0x1000_1000) >> 13).clamp(0, 0xffff);
    [(2 * fy).min(0xffff) as u32, a as u32, b as u32]
}

impl RgbToCmyk {
    fn new() -> Option<RgbToCmyk> {
        let b2a = lut16(SwopTag::B2A0.bytes())?;
        if b2a.inputs != 3 || b2a.outputs != 4 || b2a.intab.iter().any(|t| t.len() != 256) {
            return None;
        }
        let srgb = icm::Space::Srgb.profile();
        let mut lab: Vec<[u32; 3]> = (0..32 * 32 * 32u32).map(|i| [icm::node(i >> 10), icm::node((i >> 5) & 31), icm::node(i & 31)]).collect();
        let lin: [u32; 1024] = std::array::from_fn(|j| icm::linear_alut(j as i64) as u32);
        icm::matrix_pass_to(&mut lab, &icm::half(&srgb), &icm::elut_curve(icm::srgb_curve()), &lin);
        let dl = descaled(&b2a.intab, b2a.grid);
        let lin4: Vec<u32> = (0..4).flat_map(|_| lin).collect();
        let cube = lab.into_iter().map(|xyz| grid_stage::<3, 4>(&b2a, &dl, &lin4, xyz_to_lab(xyz).map(|v| v as u16))).collect();
        let alut = b2a.outtab.iter().flat_map(|t| alut_lut16(t)).collect();
        let elut = icm::LINEAR_ELUT.map(|e| ((u64::from(e) * 65536 / 65535) * 31 / 32) as u32);
        Some(RgbToCmyk { elut, cube, alut })
    }

    /// TranslateColors COLOR_RGB to COLOR_CMYK (LHCalc3toX_Di16_Do16_Lut16_G32): 16-bit R, G, B
    /// to 16-bit C, M, Y, K
    pub fn cmyk16(&self, rgb: [u16; 3]) -> [u16; 4] {
        let e: [u64; 3] = rgb.map(|v| u64::from(input16(&self.elut, v)));
        let s = simplex::<3, 4>(e.map(|v| (v >> 11) as usize), e.map(|v| v & 0x7ff), 0x800, 31, |v, ch| u64::from(self.cube[v[0] * 1024 + v[1] * 32 + v[2]][ch]));
        std::array::from_fn(|ch| {
            let mut s = s[ch];
            s += s >> 16;
            s -= s >> 10;
            let (idx, g) = ((s >> 17) as usize, ((s >> 8) & 0x1ff) as u32);
            let at = 1024 * ch + idx;
            let next = self.alut.get(at + 1).copied().unwrap_or(0);
            ((self.alut[at] * (512 - g) + next * g) >> 9) as u16
        })
    }

    /// Convert_32BGRA_32CMYK: 8-bit B, G, R (each x257; alpha dropped) to C, M, Y, K (each /257,
    /// truncated)
    pub fn cmyk8(&self, [b, g, r]: [u8; 3]) -> [u8; 4] {
        self.cmyk16([r, g, b].map(|v| u16::from(v) * 257)).map(|v| (v / 257) as u8)
    }
}

/// the RGB to CMYK transform, built once
pub fn from_rgb() -> Option<&'static RgbToCmyk> {
    static T: OnceLock<Option<RgbToCmyk>> = OnceLock::new();
    T.get_or_init(RgbToCmyk::new).as_ref()
}
