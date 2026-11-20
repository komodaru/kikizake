//! The DDS encoder's BC3 (DXT5) blocks, as WIC writes them.
//!
//! Found clean-room (WIC's own block encoder, not D3DX's) and confirmed by golden wicencode
//! enc-dds-*, encdds-*, ddsblock-*: every byte. Integer arithmetic but for the least-squares
//! refits, in f32. The color part weighs blue 1, green 10, red 3; it starts from the corners of
//! the points' box that the signs of the blue-green and green-red covariances pick, rounds them
//! to 5:6:5 by integer multiplies, gives each pixel the step its projection onto the line falls
//! nearest (in integers), and refits the ends by least squares to those steps at most three
//! times, until neither the ends nor the steps change. The alpha part takes the 8-value mode between the least and greatest alpha when no
//! alpha is 0 or 255, else the 6-value mode between the other alphas with 0 and 255 as literals.

/// one pixel as the DDS frame holds it: blue, green, red, alpha
pub type Px = [u8; 4];

/// a pixel's color in the encoder's weighted units: blue, 10 times green, 3 times red
type Wx = [i64; 3];

fn weighted(p: &Px) -> Wx {
    [i64::from(p[0]), 10 * i64::from(p[1]), 3 * i64::from(p[2])]
}

/// weighted units rounded half up to 5, 6 and 5 bits by integer multiplies
fn pack(e: Wx) -> u16 {
    let q = |v: i64, m: i64| ((v * m + 32768) >> 16) as u16;
    q(e[2], 2655) << 11 | q(e[1], 1619) << 5 | q(e[0], 7967)
}

/// a 5:6:5 color back in weighted units
fn expand(c: u16) -> Wx {
    let e = |v: u16, m: i64| (i64::from(v) * m + 32768) >> 16;
    [e(c & 31, 538062), e(c >> 5 & 63, 2652647), e(c >> 11, 1617259)]
}

/// the start ends: the box's corners on the diagonal the signs of 16 * sum(xy) - sum(x) * sum(y)
/// pick for blue-green and green-red (a 0 counted as positive), the first end of low green
fn choose(x: &[Wx; 16]) -> (u16, u16) {
    let mn: Wx = std::array::from_fn(|k| x.iter().map(|p| p[k]).min().unwrap_or(0));
    let mx: Wx = std::array::from_fn(|k| x.iter().map(|p| p[k]).max().unwrap_or(0));
    let s: Wx = std::array::from_fn(|k| x.iter().map(|p| p[k]).sum());
    let p01: i64 = x.iter().map(|p| p[0] * p[1]).sum();
    let p12: i64 = x.iter().map(|p| p[1] * p[2]).sum();
    let (pos01, pos12) = (16 * p01 >= s[0] * s[1], 16 * p12 >= s[1] * s[2]);
    let e0 = [if pos01 { mn[0] } else { mx[0] }, mn[1], if pos12 { mn[2] } else { mx[2] }];
    let e1 = [if pos01 { mx[0] } else { mn[0] }, mx[1], if pos12 { mx[2] } else { mn[2] }];
    (pack(e0), pack(e1))
}

/// the ends ordered so that the first is greater (the 4-color mode), and each pixel's step: its
/// projection onto the line from the first end, in quarters rounded half up through a floored
/// reciprocal of the squared length
fn encode_colors(c0: u16, c1: u16, x: &[Wx; 16]) -> (u16, u16, u32) {
    const ENC: [u32; 4] = [0, 2, 3, 1];
    let (c0, c1) = if c0 < c1 { (c1, c0) } else { (c0, c1) };
    let (p, q) = (expand(c0), expand(c1));
    let d: Wx = std::array::from_fn(|k| q[k] - p[k]);
    let l = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
    let scale = 0xC000_0000u32 / u32::try_from(l).unwrap_or(u32::MAX);
    let mut idx = 0u32;
    for (i, px) in x.iter().enumerate() {
        let t = (px[2] - p[2]) * d[2] + (px[0] - p[0]) * d[0] + (px[1] - p[1]) * d[1];
        let code = if t > l {
            1
        } else if t < 0 {
            0
        } else {
            let t = u32::try_from(t).unwrap_or(0);
            ENC[((t * scale + 0x2000_0000) >> 30) as usize]
        };
        idx |= code << (2 * i);
    }
    (c0, c1, idx)
}

/// a refitted end in weighted units into 5:6:5: at most 0 the bottom, from the top the top, else
/// trunc(f32(f32(v * mult) + 0.5))
fn pack_saturate(v: [f32; 3]) -> u16 {
    const MAX: [f32; 3] = [255.0, 2550.0, 765.0];
    const TOP: [u16; 3] = [31, 63, 31];
    let mult = [f32::from_bits(0x3df8_f8f9), f32::from_bits(0x3cca_63fe), f32::from_bits(0x3d25_fb51)];
    let q: [u16; 3] = std::array::from_fn(|k| {
        if v[k] <= 0.0 {
            0
        } else if v[k] >= MAX[k] {
            TOP[k]
        } else {
            (v[k] * mult[k] + 0.5f32) as u16
        }
    });
    q[2] << 11 | q[1] << 5 | q[0]
}

/// the least-squares ends of the pixels' steps (None where the steps leave it undetermined)
fn optimize_colors(x: &[Wx; 16], idx: u32) -> Option<(u16, u16)> {
    const C0: [i64; 4] = [3, 0, 2, 1];
    const C1: [i64; 4] = [0, 3, 1, 2];
    let (mut sbb, mut sab, mut saa) = (0i64, 0i64, 0i64);
    let (mut sxb, mut sxa) = ([0i64; 3], [0i64; 3]);
    for (i, px) in x.iter().enumerate() {
        let q = (idx >> (2 * i) & 3) as usize;
        let (b, a) = (C0[q], C1[q]);
        sbb += b * b;
        sab += a * b;
        saa += a * a;
        for k in 0..3 {
            sxb[k] += px[k] * b;
            sxa[k] += px[k] * a;
        }
    }
    let det = saa * sbb - sab * sab;
    if det == 0 {
        return None;
    }
    let inv = 3.0f32 / det as f32;
    let x0: [f32; 3] = std::array::from_fn(|k| (sxb[k] * saa - sxa[k] * sab) as f32 * inv);
    let x1: [f32; 3] = std::array::from_fn(|k| (sxa[k] * sbb - sxb[k] * sab) as f32 * inv);
    Some((pack_saturate(x0), pack_saturate(x1)))
}

fn encode_color(px: &[Px; 16]) -> [u8; 8] {
    let x: [Wx; 16] = std::array::from_fn(|i| weighted(&px[i]));
    let (mut c0, mut c1) = choose(&x);
    let mut idx = 0;
    if c0 != c1 {
        (c0, c1, idx) = encode_colors(c0, c1, &x);
        for _ in 0..3 {
            let saved = (c0, c1);
            if let Some(n) = optimize_colors(&x, idx) {
                (c0, c1) = n;
            }
            if (c0, c1) == saved {
                break;
            }
            if c0 == c1 {
                idx = 0;
                break;
            }
            let old = idx;
            (c0, c1, idx) = encode_colors(c0, c1, &x);
            if idx == old {
                break;
            }
        }
    }
    let mut o = [0u8; 8];
    o[0..2].copy_from_slice(&c0.to_le_bytes());
    o[2..4].copy_from_slice(&c1.to_le_bytes());
    o[4..8].copy_from_slice(&idx.to_le_bytes());
    o
}

/// which of BC3's two alpha modes a block's ends make
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AlphaMode {
    /// first end greater: 8 values between them
    Eight,
    /// first end not greater: 6 values between them, then 0 and 255
    Six,
}

impl AlphaMode {
    /// the weights of the first and second end at each code, and their sum
    fn weights(self) -> (&'static [i64], &'static [i64], f32) {
        match self {
            Self::Eight => (&[7, 0, 6, 5, 4, 3, 2, 1], &[0, 7, 1, 2, 3, 4, 5, 6], 7.0),
            Self::Six => (&[5, 0, 4, 3, 2, 1], &[0, 5, 1, 2, 3, 4], 5.0),
        }
    }
}

/// every pixel's code where both ends are one value
fn solid_alpha(m: AlphaMode, a: &[u8; 16]) -> [u8; 16] {
    match m {
        AlphaMode::Eight => [0; 16],
        AlphaMode::Six => a.map(|v| match v {
            0 => 6,
            255 => 7,
            _ => 0,
        }),
    }
}

/// each pixel's code: its place between the ends in 7ths (5ths) rounded half up through a
/// floored reciprocal of their distance (which wraps to a reciprocal 0 where the ends cross);
/// in the 6-value mode a pixel outside the ends to the nearer of the end and the literal
fn encode_alphas(m: AlphaMode, a0: u8, a1: u8, a: &[u8; 16]) -> [u8; 16] {
    const ENC8: [u8; 8] = [1, 7, 6, 5, 4, 3, 2, 0];
    const ENC6: [u8; 6] = [0, 2, 3, 4, 5, 1];
    let (a0, a1) = (u32::from(a0), u32::from(a1));
    a.map(|x| {
        let x = u32::from(x);
        match m {
            AlphaMode::Eight => {
                let scale = 0x70000 / a0.wrapping_sub(a1);
                if x < a1 {
                    1
                } else if x > a0 {
                    0
                } else {
                    ENC8[(((x - a1) * scale + 0x8000) >> 16) as usize]
                }
            }
            AlphaMode::Six => {
                let (lo, hi) = (a0, a1);
                let scale = 0x50000 / hi.wrapping_sub(lo);
                if x < lo {
                    if x <= lo >> 1 { 6 } else { 0 }
                } else if x > hi {
                    if x > (hi + 255) >> 1 { 7 } else { 1 }
                } else {
                    ENC6[(((x - lo) * scale + 0x8000) >> 16) as usize]
                }
            }
        }
    })
}

/// the least-squares ends of the pixels' codes (the literals left out), each into 0..=255 as
/// trunc(f32(v + 0.5)); None where the codes leave them undetermined
fn optimize_alphas(m: AlphaMode, a: &[u8; 16], codes: &[u8; 16]) -> Option<(u8, u8)> {
    let (c0, c1, k) = m.weights();
    let (mut sbb, mut sab, mut saa, mut sxb, mut sxa) = (0i64, 0i64, 0i64, 0i64, 0i64);
    for (&x, &q) in a.iter().zip(codes) {
        let q = usize::from(q);
        if q >= c0.len() {
            continue;
        }
        let (b, w) = (c0[q], c1[q]);
        sbb += b * b;
        sab += w * b;
        saa += w * w;
        sxb += i64::from(x) * b;
        sxa += i64::from(x) * w;
    }
    let det = saa * sbb - sab * sab;
    if det == 0 {
        return None;
    }
    let inv = k / det as f32;
    let q = |v: f32| {
        if v < 0.0 {
            0
        } else if v > 255.0 {
            255
        } else {
            (v + 0.5f32) as u8
        }
    };
    Some((q((sxb * saa - sxa * sab) as f32 * inv), q((sxa * sbb - sxb * sab) as f32 * inv)))
}

fn encode_alpha(px: &[Px; 16]) -> [u8; 8] {
    let a: [u8; 16] = px.map(|p| p[3]);
    let zero = a.contains(&0);
    let full = a.contains(&255);
    let other: Vec<u8> = a.iter().copied().filter(|&v| v != 0 && v != 255).collect();
    let (a0, a1, codes) = match (other.iter().min(), other.iter().max()) {
        (Some(&lo), Some(&hi)) => {
            let (m, mut a0, mut a1) = if zero || full { (AlphaMode::Six, lo, hi) } else { (AlphaMode::Eight, hi, lo) };
            let mut codes = solid_alpha(m, &a);
            if a0 != a1 {
                codes = encode_alphas(m, a0, a1, &a);
                for _ in 0..3 {
                    let saved = (a0, a1);
                    if let Some(n) = optimize_alphas(m, &a, &codes) {
                        (a0, a1) = n;
                    }
                    if (a0, a1) == saved {
                        break;
                    }
                    if a0 == a1 {
                        codes = solid_alpha(m, &a);
                        break;
                    }
                    let old = codes;
                    codes = encode_alphas(m, a0, a1, &a);
                    if codes == old {
                        break;
                    }
                }
            }
            (a0, a1, codes)
        }
        // (only 0 and 255: both ends 255, the 6-value mode, 0 as its literal)
        _ if zero && full => (255, 255, a.map(|v| if v == 0 { 6 } else { 0 })),
        _ if zero => (0, 0, [0; 16]),
        _ => (255, 255, [0; 16]),
    };
    let bits = codes.iter().enumerate().fold(0u64, |b, (i, &c)| b | u64::from(c) << (3 * i));
    let mut o = [0u8; 8];
    o[0] = a0;
    o[1] = a1;
    o[2..8].copy_from_slice(&bits.to_le_bytes()[0..6]);
    o
}

/// one BC3 block: alpha in bytes 0..8, color in 8..16
pub fn encode_bc3(px: &[Px; 16]) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&encode_alpha(px));
    out[8..16].copy_from_slice(&encode_color(px));
    out
}
