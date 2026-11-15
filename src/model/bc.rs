//! The DDS encoder's BC3 (DXT5) blocks, as WIC writes them.
//!
//! MEASURED (golden wicencode enc-dds-*, encdds-*: every byte): the D3DX family's block encoder,
//! weighted by luminance: the color box the fit starts from is that of the points rounded to
//! 5:6:5, the diagonal of it the one the points as they are spread along most (D3DX's sums of
//! squares about the box's middle); the endpoints of both the color and the alpha line are then
//! refitted by least squares to the steps the points (as they are) fall on until no point moves;
//! each pixel's step is chosen from its color rounded to 5:6:5; past the picture a block repeats
//! its last column and row.
//! OPEN (WICIMAGE.ENCODE.LOSSY): Windows' fit is not this one everywhere (golden ddsblock-*:
//! 152 of 172 blocks as this; ddsblock-bgr24-24, -38: the same steps but other ends, so its ends
//! are not the least-squares fit of its steps; enc-dds-bgr555 block 1 starts from another
//! diagonal; ddsblock-bgra-15, -28, -31: other alpha ends), so a block is modelled only where it
//! comes out the same from every start diagonal, whether or not the start box and the steps take
//! the points rounded, and after 2, 3, 4 or all refits.

/// one pixel: red, green, blue, alpha in 0..=1
pub type Px = [f32; 4];

/// the luminance weights (red and blue against green)
const LUM: [f32; 3] = [0.2125f32 / 0.7154f32, 1.0, 0.0721f32 / 0.7154f32];
const LUM_INV: [f32; 3] = [0.7154f32 / 0.2125f32, 1.0, 0.7154f32 / 0.0721f32];

/// at most this many refits (none of the goldens takes more than a few)
const REFITS: usize = 64;

/// a 0..=1 channel rounded to the 5:6:5 grid (5 bits red and blue, 6 green), as D3DX rounds
fn grid565(c: [f32; 3]) -> [f32; 3] {
    const LEVELS: [f32; 3] = [31.0, 63.0, 31.0];
    std::array::from_fn(|k| trunc(c[k] * LEVELS[k] + 0.5) as f32 * (1.0 / LEVELS[k]))
}

/// a float to int as C's cast: toward zero
fn trunc(x: f32) -> i32 {
    x.trunc() as i32
}

fn encode565(c: [f32; 3]) -> u16 {
    let q = |v: f32, m: f32| trunc(v.clamp(0.0, 1.0) * m + 0.5);
    let (r, g, b) = (q(c[0], 31.0), q(c[1], 63.0), q(c[2], 31.0));
    // (the fields do not overlap: added, so that no mutant of the packing is equivalent)
    u16::try_from((r << 11) + (g << 5) + b).unwrap_or(u16::MAX)
}

fn decode565(w: u16) -> [f32; 3] {
    [f32::from((w >> 11) & 31) * (1.0 / 31.0), f32::from((w >> 5) & 63) * (1.0 / 63.0), f32::from(w & 31) * (1.0 / 31.0)]
}

/// which diagonal of the box the fit starts along: green and blue swapped or not
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Diagonal {
    swap_green: bool,
    swap_blue: bool,
}

impl Diagonal {
    pub const ALL: [Diagonal; 4] = [
        Diagonal { swap_green: false, swap_blue: false },
        Diagonal { swap_green: false, swap_blue: true },
        Diagonal { swap_green: true, swap_blue: false },
        Diagonal { swap_green: true, swap_blue: true },
    ];
}

/// the step a point projects to on a line from `x` along `dir` (scaled so that the steps are
/// 0, 1, ..): 0 and the last at the ends
fn project(p: &[f32; 3], x: &[f32; 3], dir: &[f32; 3], last: f32, steps: usize) -> usize {
    let dot = (p[0] - x[0]) * dir[0] + (p[1] - x[1]) * dir[1] + (p[2] - x[2]) * dir[2];
    if dot <= 0.0 {
        0
    } else if dot >= last {
        steps - 1
    } else {
        trunc(dot + 0.5) as usize
    }
}

/// the two ends of the color line through luminance-weighted points
/// whether a stage takes the points rounded to 5:6:5 or as they are
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rounding {
    Grid,
    Plain,
}

impl Rounding {
    const ALL: [Rounding; 2] = [Rounding::Grid, Rounding::Plain];
}

fn optimize_rgb(pts: &[[f32; 3]; 16], grid: &[[f32; 3]; 16], start: Option<Diagonal>, start_box: Rounding, refits: usize) -> ([f32; 3], [f32; 3]) {
    let c4 = [1.0f32, 2.0 / 3.0, 1.0 / 3.0, 0.0];
    let d4 = [0.0f32, 1.0 / 3.0, 2.0 / 3.0, 1.0];
    let mut x = LUM;
    let mut y = [0.0f32; 3];
    // (the box of the points rounded to 5:6:5)
    for p in match start_box { Rounding::Grid => grid, Rounding::Plain => pts } {
        for k in 0..3 {
            x[k] = x[k].min(p[k]);
            y[k] = y[k].max(p[k]);
        }
    }
    let ab = [y[0] - x[0], y[1] - x[1], y[2] - x[2]];
    let fab = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    if fab < f32::MIN_POSITIVE {
        return (x, y);
    }
    let inv = 1.0 / fab;
    let dir = [ab[0] * inv, ab[1] * inv, ab[2] * inv];
    let mid = [(x[0] + y[0]) * 0.5, (x[1] + y[1]) * 0.5, (x[2] + y[2]) * 0.5];
    let mut fd = [0.0f32; 4];
    for p in pts {
        let t = [(p[0] - mid[0]) * dir[0], (p[1] - mid[1]) * dir[1], (p[2] - mid[2]) * dir[2]];
        let f = [t[0] + t[1] + t[2], t[0] + t[1] - t[2], t[0] - t[1] + t[2], t[0] - t[1] - t[2]];
        for j in 0..4 {
            fd[j] += f[j] * f[j];
        }
    }
    let mut best = 0;
    for j in 1..4 {
        if fd[j] > fd[best] {
            best = j;
        }
    }
    let d = start.unwrap_or(Diagonal::ALL[best]);
    if d.swap_green {
        std::mem::swap(&mut x[1], &mut y[1]);
    }
    if d.swap_blue {
        std::mem::swap(&mut x[2], &mut y[2]);
    }
    if fab < 1.0 / 4096.0 {
        return (x, y);
    }
    let mut was: Option<[usize; 16]> = None;
    for _ in 0..refits {
        let mut dir = [y[0] - x[0], y[1] - x[1], y[2] - x[2]];
        let len = dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2];
        if len < 1.0 / 4096.0 {
            break;
        }
        let scale = 3.0 / len;
        dir = dir.map(|v| v * scale);
        let at: [usize; 16] = std::array::from_fn(|i| project(&pts[i], &x, &dir, 3.0, 4));
        if was == Some(at) {
            break;
        }
        was = Some(at);
        let (mut sa, mut sb, mut sab) = (0.0f32, 0.0f32, 0.0f32);
        let (mut ta, mut tb) = ([0.0f32; 3], [0.0f32; 3]);
        for (p, &s) in pts.iter().zip(&at) {
            let (a, b) = (c4[s], d4[s]);
            sa += a * a;
            sb += b * b;
            sab += a * b;
            for k in 0..3 {
                ta[k] += a * p[k];
                tb[k] += b * p[k];
            }
        }
        let det = sa * sb - sab * sab;
        if det == 0.0 {
            break;
        }
        x = std::array::from_fn(|k| (sb * ta[k] - sab * tb[k]) / det);
        y = std::array::from_fn(|k| (sa * tb[k] - sab * ta[k]) / det);
    }
    (x, y)
}

/// the 8 color bytes of a block (4 steps: endpoints, then 2 bits a pixel)
fn encode_color(px: &[Px; 16], start: Option<Diagonal>, start_box: Rounding, steps_from: Rounding, refits: usize) -> [u8; 8] {
    let lum = |c: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| c[k] * LUM[k]) };
    let pts: [[f32; 3]; 16] = std::array::from_fn(|i| lum([px[i][0], px[i][1], px[i][2]]));
    let grid: [[f32; 3]; 16] = std::array::from_fn(|i| lum(grid565([px[i][0], px[i][1], px[i][2]])));
    let (a, b) = optimize_rgb(&pts, &grid, start, start_box, refits);
    let wa = encode565(std::array::from_fn(|k| a[k] * LUM_INV[k]));
    let wb = encode565(std::array::from_fn(|k| b[k] * LUM_INV[k]));
    let mut out = [0u8; 8];
    if wa == wb {
        out[0..2].copy_from_slice(&wa.to_le_bytes());
        out[2..4].copy_from_slice(&wb.to_le_bytes());
        return out;
    }
    let (ca, cb) = (lum(decode565(wa)), lum(decode565(wb)));
    // (4 steps: the larger endpoint first)
    let (w0, w1, s0, s1) = if wa <= wb { (wb, wa, cb, ca) } else { (wa, wb, ca, cb) };
    let order = [0u32, 2, 3, 1];
    let mut dir = [s1[0] - s0[0], s1[1] - s0[1], s1[2] - s0[2]];
    let scale = 3.0 / (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]);
    dir = dir.map(|v| v * scale);
    let mut dw = 0u32;
    for p in px {
        // (each pixel's step from its color rounded to 5:6:5)
        let c = lum(match steps_from { Rounding::Grid => grid565([p[0], p[1], p[2]]), Rounding::Plain => [p[0], p[1], p[2]] });
        let dot = (c[0] - s0[0]) * dir[0] + (c[1] - s0[1]) * dir[1] + (c[2] - s0[2]) * dir[2];
        let step = if dot <= 0.0 { 0 } else if dot >= 3.0 { 1 } else { order[trunc(dot + 0.5) as usize] };
        dw = (step << 30) | (dw >> 2);
    }
    out[0..2].copy_from_slice(&w0.to_le_bytes());
    out[2..4].copy_from_slice(&w1.to_le_bytes());
    out[4..8].copy_from_slice(&dw.to_le_bytes());
    out
}

/// whether the alpha block has the two extra steps 0 and 1 (6 between the ends) or 8
#[derive(Clone, Copy, PartialEq, Eq)]
enum AlphaSteps {
    Six,
    Eight,
}

impl AlphaSteps {
    fn count(self) -> usize {
        match self {
            AlphaSteps::Six => 6,
            AlphaSteps::Eight => 8,
        }
    }
}

/// the step of an alpha value on the line `s0`..`s1` (the D3DX rule, plus the extra steps of
/// six; MEASURED golden enc-dds-idx4: when the ends are one value, a value above them takes the
/// far end, below the near one, the value itself step 0)
fn alpha_step(v: f32, s0: f32, s1: f32, scale: f32, steps: AlphaSteps) -> usize {
    let six = steps == AlphaSteps::Six;
    let last = (steps.count() - 1) as f32;
    let low = |v: f32| if six && v <= s0 * 0.5 { 6 } else { 0 };
    let high = |v: f32| if six && v >= (s1 + 1.0) * 0.5 { 7 } else { 1 };
    if s0 == s1 {
        return if v > s0 { high(v) } else if v < s0 { low(v) } else { 0 };
    }
    let dot = (v - s0) * scale;
    if dot <= 0.0 {
        low(v)
    } else if dot >= last {
        high(v)
    } else {
        let i = trunc(dot + 0.5) as usize;
        if i == 0 { 0 } else if i == steps.count() - 1 { 1 } else { i + 1 }
    }
}

/// the ends of the alpha line, refitted by least squares
fn optimize_alpha(pts: &[f32; 16], steps: AlphaSteps, refits: usize) -> (f32, f32) {
    let n = steps.count();
    let last = (n - 1) as f32;
    let c = |s: usize| (n - 1 - s) as f32 / last;
    let d = |s: usize| s as f32 / last;
    let (mut x, mut y) = (1.0f32, 0.0f32);
    for &p in pts {
        match steps {
            AlphaSteps::Eight => {
                x = x.min(p);
                y = y.max(p);
            }
            AlphaSteps::Six => {
                if p < x && p > 0.0 {
                    x = p;
                }
                if p > y && p < 1.0 {
                    y = p;
                }
            }
        }
    }
    let mut was: Option<[usize; 16]> = None;
    for _ in 0..refits {
        if y - x < 1.0 / 256.0 {
            break;
        }
        let scale = last / (y - x);
        let at: [usize; 16] = std::array::from_fn(|i| {
            let p = pts[i];
            let dot = (p - x) * scale;
            if dot <= 0.0 {
                if steps == AlphaSteps::Six && p <= x * 0.5 { 6 } else { 0 }
            } else if dot >= last {
                if steps == AlphaSteps::Six && p >= (y + 1.0) * 0.5 { 7 } else { n - 1 }
            } else {
                trunc(dot + 0.5) as usize
            }
        });
        if was == Some(at) {
            break;
        }
        was = Some(at);
        let (mut sa, mut sb, mut sab, mut ta, mut tb) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
        for (&p, &s) in pts.iter().zip(&at) {
            if s < n {
                let (a, b) = (c(s), d(s));
                sa += a * a;
                sb += b * b;
                sab += a * b;
                ta += a * p;
                tb += b * p;
            }
        }
        let det = sa * sb - sab * sab;
        if det == 0.0 {
            break;
        }
        x = (sb * ta - sab * tb) / det;
        y = (sa * tb - sab * ta) / det;
        if x > y {
            std::mem::swap(&mut x, &mut y);
        }
    }
    (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
}

fn encode_alpha(px: &[Px; 16], refits: usize) -> [u8; 8] {
    let q = |v: f32| trunc(v * 255.0 + 0.5);
    let fa: [f32; 16] = std::array::from_fn(|i| q(px[i][3]) as f32 * (1.0 / 255.0));
    let (mut lo, mut hi) = (px[0][3], px[0][3]);
    for &v in &fa {
        if v < lo {
            lo = v;
        } else if v > hi {
            hi = v;
        }
    }
    let mut out = [0u8; 8];
    if lo == 1.0 {
        out[0] = 0xff;
        out[1] = 0xff;
        return out;
    }
    let steps = if lo == 0.0 || hi == 1.0 { AlphaSteps::Six } else { AlphaSteps::Eight };
    let (a, b) = optimize_alpha(&fa, steps, refits);
    let byte = |v: f32| u8::try_from(q(v) & 0xff).unwrap_or(0);
    let (ba, bb) = (byte(a), byte(b));
    let (fa_, fb_) = (f32::from(ba) * (1.0 / 255.0), f32::from(bb) * (1.0 / 255.0));
    if steps == AlphaSteps::Eight && ba == bb {
        out[0] = ba;
        out[1] = bb;
        return out;
    }
    let (b0, b1, s0, s1) = match steps {
        AlphaSteps::Six => (ba, bb, fa_, fb_),
        AlphaSteps::Eight => (bb, ba, fb_, fa_),
    };
    out[0] = b0;
    out[1] = b1;
    let last = (steps.count() - 1) as f32;
    let scale = if s0 != s1 { last / (s1 - s0) } else { 0.0 };
    for half in 0..2 {
        let mut dw = 0u32;
        for p in &px[half * 8..half * 8 + 8] {
            let s = alpha_step(p[3], s0, s1, scale, steps) as u32;
            dw = (s << 21) | (dw >> 3);
        }
        out[2 + half * 3..5 + half * 3].copy_from_slice(&dw.to_le_bytes()[0..3]);
    }
    out
}

/// one BC3 block; None when the color part depends on the start diagonal (open; even a gray
/// block's can: golden encdds-gray8-6x10)
pub fn encode_bc3(px: &[Px; 16]) -> Option<[u8; 16]> {
    let first = encode_color(px, Some(Diagonal::ALL[0]), Rounding::Grid, Rounding::Grid, REFITS);
    // (modelled only where the block comes out the same from every start diagonal and whether
    // or not the start box and the steps take the points rounded: the goldens' blocks that hang
    // on them follow no one of the choices)
    for d in Diagonal::ALL {
        for b in Rounding::ALL {
            for s in Rounding::ALL {
                for r in [2, 3, 4, REFITS] {
                    if encode_color(px, Some(d), b, s, r) != first {
                        return None;
                    }
                }
            }
        }
    }
    let alpha = encode_alpha(px, REFITS);
    if [2, 3, 4].iter().any(|&r| encode_alpha(px, r) != alpha) {
        return None;
    }
    let color = first;
    let mut out = [0u8; 16];
    out[0..8].copy_from_slice(&alpha);
    out[8..16].copy_from_slice(&color);
    Some(out)
}
