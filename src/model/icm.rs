//! The color engine behind IWICColorTransform for RGB matrix/TRC profiles: mscms hands the two
//! profiles to icm32 (the Linotype-Hell CMM), which builds a 32-point 3D table with 8-bit input
//! curves, two integer matrix passes through the XYZ connection space and 10-bit output curves.
//!
//! FOUND clean-room (WindowsCodecsExt CIcmColorTransform, icm32 CMCreateMultiProfileTransform,
//! MakeCube16, DoMatrixForCube16, Fill_ushort_ELUT_from_CurveTag, Fill_inverse_ushort_ALUT_*,
//! LHCalc3to3_Di8_Do8_Lut16_G32, LHCalc3toX_Di16_Do16_Lut16_G32) and confirmed by Windows
//! (golden wiccolor cm-*, levels-*, xf-*): spec/wicimage.txt WICIMAGE.COLOR.ENGINE.

use std::sync::OnceLock;

/// a profile's tone curve: a sampled `curv` or one gamma (u8.8)
#[derive(Clone, Debug, PartialEq)]
pub enum Curve {
    Sampled(Vec<u16>),
    Gamma(u16),
}

/// an RGB matrix/TRC profile: the colorants (columns rXYZ, gXYZ, bXYZ in s15.16; row 0 is X) and
/// one curve for all three channels
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub colorants: [[i32; 3]; 3],
    pub curve: Curve,
}

/// The EXIF color spaces a context names: 2 Adobe RGB, any other sRGB
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Space {
    Srgb,
    Adobe,
}

/// sRGB's 1024 curve entries: MEASURED (golden wiccolor profile-srgb: Windows' "sRGB Color Space
/// Profile.icm", each entry) the sRGB function at i/1023, times 65535, rounded
pub fn srgb_curve() -> &'static [u16] {
    static C: OnceLock<Vec<u16>> = OnceLock::new();
    C.get_or_init(|| {
        (0..1024u32)
            .map(|i| {
                let x = f64::from(i) / 1023.0;
                let l = if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) };
                u16::try_from((l * 65535.0).round() as i64).unwrap_or(u16::MAX)
            })
            .collect()
    })
}

/// MEASURED (golden wiccolor profile-srgb): rXYZ, gXYZ, bXYZ of Windows' sRGB profile
pub const SRGB_XYZ: [[i32; 3]; 3] = [[0x6fa2, 0x38f5, 0x0390], [0x6299, 0xb785, 0x18da], [0x24a0, 0x0f84, 0xb6cf]];
/// FOUND clean-room (WindowsCodecsExt OpRGB_SetupAdobeRGB): the colorants Adobe RGB's built-in
/// profile gets
pub const ADOBE_XYZ: [[i32; 3]; 3] = [[0x9c18, 0x4fa5, 0x04fc], [0x348d, 0xa02c, 0x0f95], [0x2631, 0x102f, 0xbe9c]];
/// and its gamma, 0x233/256 = 2.19921875
pub const ADOBE_GAMMA: u16 = 0x233;

impl Space {
    pub fn profile(self) -> Profile {
        let (xyz, curve) = match self {
            Space::Srgb => (SRGB_XYZ, Curve::Sampled(srgb_curve().to_vec())),
            Space::Adobe => (ADOBE_XYZ, Curve::Gamma(ADOBE_GAMMA)),
        };
        // (columns are the colorants: row r, column c is colorant c's component r)
        Profile { colorants: std::array::from_fn(|r| std::array::from_fn(|c| xyz[c][r])), curve }
    }
}

/// the finished transform's tables: ELUT (8-bit in, descaled to grid units), the 32^3 table of
/// u16 triples, ALUT (1024 entries)
pub struct Transform {
    e16: [u32; 256],
    elut: [u32; 256],
    cube: Vec<[u32; 3]>,
    alut: [u32; 1024],
}

fn elut_curve(cv: &[u16]) -> [u32; 256] {
    let n = cv.len() as i64;
    let step = ((n - 1) << 18) / 255;
    std::array::from_fn(|i| {
        let pos = (i as i64 * step + 4) >> 3;
        let (k, fr) = ((pos >> 15) as usize, pos & 0x7fff);
        let y0 = i64::from(cv[k]);
        let v = if fr == 0 {
            y0
        } else {
            let h = ((i64::from(cv[k + 1]) - y0) * 0x4000) >> 15;
            (y0 * 0x2000 + ((h * fr) >> 1) + 0xfff) >> 13
        };
        (v & 0xffff) as u32
    })
}

fn gamma(x: f64, g: f64) -> f64 {
    if x <= 0.0 { 0.0 } else { x.powf(g).min(1.0) }
}

fn elut_gamma(g: f64) -> [u32; 256] {
    let mut e = [0i64; 256];
    for j in (0..255).step_by(4) {
        e[j] = (gamma(j as f64 * (1.0 / 255.0), g) * 65535.0) as i64;
    }
    e[255] = (gamma(1.0, g) * 65535.0) as i64;
    for j0 in (0..252).step_by(4) {
        let d = e[j0 + 4] - e[j0];
        for k in 1..4 {
            e[j0 + k] = (e[j0] + ((((d * k as i64) << 8).div_euclid(4) + 0x80) >> 8)) & 0xffff;
        }
    }
    let d = e[255] - e[252];
    for k in 1..3 {
        e[252 + k] = (e[252] + ((((d * k as i64) << 8).div_euclid(3) + 0x80) >> 8)) & 0xffff;
    }
    e.map(|v| v as u32)
}

pub(crate) fn alut_curve(cv: &[u16]) -> [u32; 1024] {
    let mut cv: Vec<i64> = cv.iter().map(|&v| i64::from(v)).collect();
    let n = cv.len();
    for k in 0..n - 1 {
        if cv[k] > cv[k + 1] {
            cv[k + 1] = cv[k];
        }
    }
    let lo = (cv[0] as f64 * (1023.0 / 65535.0) + 0.99998) as usize;
    let hi = (cv[n - 1] as f64 * (1023.0 / 65535.0)) as usize;
    let mut a = [0u32; 1024];
    for v in a.iter_mut().skip(hi + 1) {
        *v = 0xffff;
    }
    let mut k = 0;
    for j in lo..=hi.min(1023) {
        let t = (j as i64 * 65535 + 0x1ff) / 1023;
        while cv[k + 1] < t && k < n - 2 {
            k += 1;
        }
        let mut b = ((k as i64) << 16) / (n as i64 - 1);
        if cv[k + 1] != cv[k] {
            b += ((t - cv[k]) << 16).div_euclid((cv[k + 1] - cv[k]) * (n as i64 - 1));
        }
        a[j] = if b & 0x10000 != 0 { 0xffff } else { (b & 0xffff) as u32 };
    }
    a
}

fn alut_gamma(g: f64) -> [u32; 1024] {
    let f = |x: f64| gamma(x, 1.0 / g);
    let r = 1.0 / 1023.0;
    let mut a = [0i64; 1024];
    for j in (0..1023).step_by(16) {
        a[j] = (f((j as f64 * r).min(1.0)) * 65535.0 + 0.5) as i64;
    }
    a[1023] = (f(1.0) * 65535.0 + 0.5) as i64;
    for j0 in (0..1008).step_by(16) {
        let d = (a[j0 + 16] - a[j0]) & 0xffff_ffff;
        for k in 1..16 {
            a[j0 + k] = (a[j0] + ((((d * k as i64 + 8) << 4) & 0xffff_ffff) >> 8)) & 0xffff;
        }
    }
    let d = a[1023] - a[1008];
    for k in 1..15 {
        a[1008 + k] = (a[1008] + ((((d * k as i64) << 8).div_euclid(15) + 0x80) >> 8)) & 0xffff;
    }
    if 1.0 / g < 1.0 {
        for (j, v) in a.iter_mut().enumerate().take(16) {
            *v = (f(j as f64 * r) * 65535.0) as i64;
        }
    }
    a.map(|v| v as u32)
}

/// the linear output table DoMatrixForCube16 and the per-pixel step use when no curve follows
pub(crate) fn linear_alut(j: i64) -> i64 {
    ((0x7ff + j * 0x400fc) >> 12).min(65535)
}

/// one DoMatrixForCube16 pass with linear stage tables: each node through the matrix (scaled by
/// 1023*16, rounded half away from zero) in 14-bit steps, negatives 0
fn matrix_pass(cube: &mut [[u32; 3]], m: &[[f64; 3]; 3]) {
    let al: [u32; 1024] = std::array::from_fn(|j| linear_alut(j as i64) as u32);
    matrix_pass_to(cube, m, &al);
}

/// the pass with a linear input table and the output table `al` (interpolated in 8-bit
/// fractions, its last entry from 1023 on)
pub(crate) fn matrix_pass_to(cube: &mut [[u32; 3]], m: &[[f64; 3]; 3], al: &[u32; 1024]) {
    let el = |i: i64| i * 257;
    let al = |j: i64| i64::from(al[j as usize]);
    let k = 1023.0 * 16.0;
    let mi = m.map(|row| row.map(|x| { let v = x * k; if v > 0.0 { (v + 0.5) as i64 } else { (v - 0.5) as i64 } }));
    for node in cube.iter_mut() {
        let y = node.map(|v| {
            let v = i64::from(v);
            let d = v - (v >> 8);
            let (i, f) = (d >> 8, d & 0xff);
            let e = if i >= 255 { el(i) } else { (el(i) * (256 - f) + el(i + 1) * f) >> 8 };
            (e + 2) >> 2
        });
        *node = std::array::from_fn(|r| {
            let s = ((mi[r][0] * y[0] + mi[r][1] * y[1] + mi[r][2] * y[2] + 0x1ff) >> 10).max(0);
            let (i, f) = (s >> 8, s & 0xff);
            let o = if i >= 1023 { al(1023) } else { (al(i) * (256 - f) + al(i + 1) * f) >> 8 };
            (o & 0xffff) as u32
        });
    }
}

pub(crate) fn inverse(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let c = |r: usize, k: usize| m[(r + 1) % 3][(k + 1) % 3] * m[(r + 2) % 3][(k + 2) % 3] - m[(r + 1) % 3][(k + 2) % 3] * m[(r + 2) % 3][(k + 1) % 3];
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    std::array::from_fn(|r| std::array::from_fn(|k| c(k, r) / det))
}

pub(crate) fn half(p: &Profile) -> [[f64; 3]; 3] {
    p.colorants.map(|row| row.map(|v| f64::from(v) / 65536.0 / 2.0))
}

impl Transform {
    pub fn new(from: &Profile, to: &Profile) -> Transform {
        let e16 = match &from.curve {
            Curve::Sampled(c) => elut_curve(c),
            Curve::Gamma(g) => elut_gamma(f64::from(*g) / 256.0),
        };
        let elut = e16.map(|e| ((u64::from(e) * 65536 / 65535) * 31 / 32) as u32);
        let alut = match &to.curve {
            Curve::Sampled(c) => alut_curve(c),
            Curve::Gamma(g) => alut_gamma(f64::from(*g) / 256.0),
        };
        let node = |k: u32| (k << 11) | (k << 6) | (k << 1) | (k >> 4);
        let mut cube: Vec<[u32; 3]> = (0..32 * 32 * 32u32).map(|i| [node(i >> 10), node((i >> 5) & 31), node(i & 31)]).collect();
        matrix_pass(&mut cube, &half(from));
        matrix_pass(&mut cube, &inverse(&half(to)));
        Transform { e16, elut, cube, alut }
    }

    /// the tetrahedral sums (< 2^27) of the three outputs for grid inputs e (11-bit fractions)
    fn sums(&self, e: [u32; 3]) -> [u64; 3] {
        let i = e.map(|v| (v >> 11) as usize);
        let f = e.map(|v| v & 0x7ff);
        let mut order = [0usize, 1, 2];
        order.sort_by(|&a, &b| f[b].cmp(&f[a]));
        let mut v = [i; 4];
        for (k, &c) in order.iter().enumerate() {
            v[k + 1] = v[k];
            v[k + 1][c] += 1;
        }
        let (fa, fb, fc) = (f[order[0]], f[order[1]], f[order[2]]);
        let w = [0x800 - fa, fa - fb, fb - fc, fc];
        std::array::from_fn(|ch| {
            (0..4)
                .filter(|&k| w[k] != 0)
                .map(|k| u64::from(w[k]) * u64::from(self.cube[v[k][0] * 1024 + v[k][1] * 32 + v[k][2]][ch]))
                .sum()
        })
    }

    /// LHCalc3to3_Di8_Do8_Lut16_G32: an 8-bit R, G, B to 8 bits
    pub fn rgb8(&self, rgb: [u8; 3]) -> [u8; 3] {
        let s = self.sums(rgb.map(|c| self.elut[usize::from(c)]));
        s.map(|s| (self.alut[(s >> 17) as usize] >> 8) as u8)
    }

    /// the next entry past `idx` of the output table: past R's end G's first (the tables lie one
    /// after another), past B's 0
    fn alut_next(&self, idx: usize, ch: usize) -> u32 {
        if idx + 1 < 1024 { self.alut[idx + 1] } else if ch < 2 { self.alut[0] } else { 0 }
    }

    /// CalcNDim_Data8To8_Lut16 (8-bit in, 16-bit out): the 8-bit kernel's grid point and
    /// fractions as 16-bit weights (summing to 65536), the sum +s>>16 -s>>10, >>6, the output
    /// table interpolated in 16-bit fractions
    pub fn rgb8_16(&self, rgb: [u8; 3]) -> [u16; 3] {
        self.ndim(rgb.map(|c| self.elut[usize::from(c)]))
    }

    /// the generic routine from grid inputs: the weights 32 times the 8-bit kernel's
    fn ndim(&self, e: [u32; 3]) -> [u16; 3] {
        let s = self.sums(e);
        std::array::from_fn(|ch| {
            let mut t = (s[ch] * 32) as u32;
            t += t >> 16;
            t -= t >> 10;
            t >>= 6;
            let (f, idx) = (t & 0xffff, (t >> 16) as usize);
            ((self.alut[idx] * (65536 - f) + self.alut_next(idx, ch) * f) >> 16) as u16
        })
    }

    /// Calc323Dim_Data8To8_Lut16 (16-bit in, 8-bit out): the 16-bit kernel's input and sums,
    /// then +s>>16 -s>>10, >>1, the output table interpolated in 16-bit fractions (v, 16.16) and
    /// (v - v>>8 + 2^23) >> 24
    pub fn rgb16_8(&self, rgb: [u16; 3]) -> [u8; 3] {
        let s = self.sums(self.input16(rgb));
        std::array::from_fn(|ch| {
            let mut t = s[ch] as u32;
            t += t >> 16;
            t -= t >> 10;
            t >>= 1;
            let (f, idx) = (t & 0xffff, (t >> 16) as usize);
            let v = self.alut[idx] * (65536 - f) + self.alut_next(idx, ch) * f;
            ((v - (v >> 8) + (1 << 23)) >> 24) as u8
        })
    }

    /// CalcNDim_Data8To8_Lut16 (16-bit in, 8-bit 4-byte out): the input curve before its
    /// descaling interpolated in 8-bit fractions, then descaled, the generic routine's 16 bits,
    /// (v + 128) >> 8 at most 255
    pub fn rgb16_8_generic(&self, rgb: [u16; 3]) -> [u8; 3] {
        let e = rgb.map(|v| {
            let v = u32::from(v);
            let d = v - (v >> 8);
            let (i, f) = ((d >> 8) as usize, d & 0xff);
            let next = if i < 255 { self.e16[i + 1] * f } else { 0 };
            let e = (self.e16[i] * (256 - f) + next) >> 8;
            ((u64::from(e) * 65536 / 65535) * 31 / 32) as u32
        });
        self.ndim(e).map(|v| ((u32::from(v) + 128) >> 8).min(255) as u8)
    }

    fn input16(&self, rgb: [u16; 3]) -> [u32; 3] {
        rgb.map(|v| {
            let v = u32::from(v);
            let d = v - (v >> 8);
            let (i, f) = ((d >> 8) as usize, d & 0xff);
            let next = if i < 255 { self.elut[i + 1] * f } else { 0 };
            (self.elut[i] * (256 - f) + next) >> 8
        })
    }

    /// LHCalc3toX_Di16_Do16_Lut16_G32: a 16-bit R, G, B to 16 bits
    pub fn rgb16(&self, rgb: [u16; 3]) -> [u16; 3] {
        let s = self.sums(self.input16(rgb));
        std::array::from_fn(|ch| {
            let mut s = s[ch];
            s += s >> 16;
            s -= s >> 10;
            let (idx, g) = ((s >> 17) as usize, ((s >> 8) & 0x1ff) as u32);
            let next = self.alut_next(idx, ch);
            ((self.alut[idx] * (512 - g) + next * g) >> 9) as u16
        })
    }
}

/// the transforms between the two spaces, built once
pub fn transform(from: Space, to: Space) -> &'static Transform {
    static T: OnceLock<[Transform; 4]> = OnceLock::new();
    let t = T.get_or_init(|| {
        let sp = [Space::Srgb, Space::Adobe];
        std::array::from_fn(|i| Transform::new(&sp[i / 2].profile(), &sp[i % 2].profile()))
    });
    let k = |s: Space| match s {
        Space::Srgb => 0,
        Space::Adobe => 1,
    };
    &t[k(from) * 2 + k(to)]
}
