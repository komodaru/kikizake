//! D3DX's DXT encoder, as D3DXLoadSurfaceFromMemory writes into DXT1 .. DXT5 (d3dx9load
//! loadtex-into-DXT*): the block's colours quantized to 565, endpoints found by Newton's method
//! on the luminance-weighted colours, the indices by projection on the endpoints' line.
//! Generic over the float it works in; x64 works in f32 (MEASURED), x86 on the x87 is open.

use std::ops::{Add, Div, Mul, Neg, Sub};

pub trait Real: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> {
    fn of(v: f32) -> Self;
    /// toward zero, as C's (int) cast
    fn trunc_i(self) -> i32;
    /// stored to an f32 variable
    fn store(self) -> Self;

}

impl Real for f32 {
    fn of(v: f32) -> Self {
        v
    }
    fn trunc_i(self) -> i32 {
        self as i32
    }
    fn store(self) -> Self {
        self
    }
}

impl Real for f64 {
    fn of(v: f32) -> Self {
        f64::from(v)
    }
    fn trunc_i(self) -> i32 {
        self as i32
    }
    fn store(self) -> Self {
        self as f32 as f64
    }
}

type C3<F> = [F; 3];

fn lum<F: Real>() -> C3<F> {
    [F::of(0.2125 / 0.7154), F::of(1.0), F::of(0.0721 / 0.7154)]
}

fn lum_inv<F: Real>() -> C3<F> {
    [F::of(0.7154 / 0.2125), F::of(1.0), F::of(0.7154 / 0.0721)]
}

/// how many colours a DXT1 block takes: four, or three and transparent black
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Steps {
    Three,
    Four,
}

impl Steps {
    fn n(self) -> usize {
        match self {
            Steps::Three => 3,
            Steps::Four => 4,
        }
    }
}

fn clamp01<F: Real>(v: F) -> F {
    if v < F::of(0.0) {
        F::of(0.0)
    } else if v > F::of(1.0) {
        F::of(1.0)
    } else {
        v
    }
}

fn encode565<F: Real>(c: C3<F>) -> u16 {
    let q = |v: F, m: f32| u16::try_from((clamp01(v) * F::of(m) + F::of(0.5)).trunc_i()).unwrap_or(0);
    q(c[0], 31.0) << 11 | q(c[1], 63.0) << 5 | q(c[2], 31.0)
}

fn decode565<F: Real>(w: u16) -> C3<F> {
    let u = |v: u16, m: f32| F::of(f32::from(v)) * F::of(1.0 / m);
    [u(w >> 11 & 31, 31.0), u(w >> 5 & 63, 63.0), u(w & 31, 31.0)]
}

/// the endpoints of the block's points (luminance-weighted colours)
fn optimize_rgb<F: Real>(pts: &[C3<F>; 16], steps: Steps) -> (C3<F>, C3<F>) {
    let eps = F::of((0.25 / 64.0) * (0.25 / 64.0));
    let (pc, pd): (&[f32], &[f32]) = match steps {
        Steps::Three => (&[1.0, 0.5, 0.0], &[0.0, 0.5, 1.0]),
        Steps::Four => (&[1.0, 2.0 / 3.0, 1.0 / 3.0, 0.0], &[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]),
    };
    let mut x = lum::<F>();
    let mut y = [F::of(0.0); 3];
    for p in pts {
        for k in 0..3 {
            if p[k] < x[k] {
                x[k] = p[k];
            }
            if p[k] > y[k] {
                y[k] = p[k];
            }
        }
    }
    let ab: C3<F> = std::array::from_fn(|k| y[k] - x[k]);
    let fab = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    if fab < F::of(f32::MIN_POSITIVE) {
        return (x, y);
    }
    let inv = F::of(1.0) / fab;
    let dir: C3<F> = std::array::from_fn(|k| ab[k] * inv);
    let mid: C3<F> = std::array::from_fn(|k| (x[k] + y[k]) * F::of(0.5));
    let mut fdir = [F::of(0.0); 4];
    for p in pts {
        let pt: C3<F> = std::array::from_fn(|k| (p[k] - mid[k]) * dir[k]);
        let fs = [pt[0] + pt[1] + pt[2], pt[0] + pt[1] - pt[2], pt[0] - pt[1] + pt[2], pt[0] - pt[1] - pt[2]];
        for (d, f) in fdir.iter_mut().zip(fs) {
            *d = *d + f * f;
        }
    }
    let mut best = 0;
    for i in 1..4 {
        if fdir[i] > fdir[best] {
            best = i;
        }
    }
    if best & 2 != 0 {
        std::mem::swap(&mut x[1], &mut y[1]);
    }
    if best & 1 != 0 {
        std::mem::swap(&mut x[2], &mut y[2]);
    }
    if fab < F::of(1.0 / 4096.0) {
        return (x, y);
    }
    let n = steps.n();
    let fsteps = F::of((n - 1) as f32);
    for _ in 0..8 {
        let st: Vec<C3<F>> = (0..n).map(|i| std::array::from_fn(|k| x[k] * F::of(pc[i]) + y[k] * F::of(pd[i]))).collect();
        let d: C3<F> = std::array::from_fn(|k| y[k] - x[k]);
        let len = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if len < F::of(1.0 / 4096.0) {
            break;
        }
        let scale = fsteps / len;
        let d: C3<F> = d.map(|v| v * scale);
        let (mut d2x, mut d2y) = (F::of(0.0), F::of(0.0));
        let (mut dx, mut dy) = ([F::of(0.0); 3], [F::of(0.0); 3]);
        for p in pts {
            let dot = (p[0] - x[0]) * d[0] + (p[1] - x[1]) * d[1] + (p[2] - x[2]) * d[2];
            let i = if dot <= F::of(0.0) {
                0
            } else if dot >= fsteps {
                n - 1
            } else {
                usize::try_from((dot + F::of(0.5)).trunc_i()).unwrap_or(0)
            };
            let diff: C3<F> = std::array::from_fn(|k| st[i][k] - p[k]);
            let fc = F::of(pc[i]) * F::of(1.0 / 8.0);
            let fd = F::of(pd[i]) * F::of(1.0 / 8.0);
            d2x = d2x + fc * F::of(pc[i]);
            d2y = d2y + fd * F::of(pd[i]);
            for k in 0..3 {
                dx[k] = dx[k] + fc * diff[k];
                dy[k] = dy[k] + fd * diff[k];
            }
        }
        if d2x > F::of(0.0) {
            let f = -(F::of(1.0) / d2x);
            for k in 0..3 {
                x[k] = x[k] + dx[k] * f;
            }
        }
        if d2y > F::of(0.0) {
            let f = -(F::of(1.0) / d2y);
            for k in 0..3 {
                y[k] = y[k] + dy[k] * f;
            }
        }
        if (0..3).all(|k| dx[k] * dx[k] < eps && dy[k] * dy[k] < eps) {
            break;
        }
    }
    (x, y)
}

/// a DXT1 colour block of 16 (r, g, b, a) in 0..1; where steps is Three, the texels whose
/// alpha is below one half become index 3
pub fn encode_bc1<F: Real>(px: &[[F; 4]; 16], steps: Steps) -> [u8; 8] {
    let l = lum::<F>();
    let q = |v: F, m: f32| F::of(f32::from(u8::try_from((v * F::of(m) + F::of(0.5)).trunc_i().clamp(0, 255)).unwrap_or(0))) * F::of(1.0 / m);
    let pts: [C3<F>; 16] = std::array::from_fn(|i| {
        let c = px[i];
        [(q(c[0], 31.0) * l[0]), (q(c[1], 63.0) * l[1]), (q(c[2], 31.0) * l[2])]
    });
    let (a, b) = optimize_rgb(&pts, steps);
    let li = lum_inv::<F>();
    let wa = encode565(std::array::from_fn(|k| a[k] * li[k]));
    let wb = encode565(std::array::from_fn(|k| b[k] * li[k]));
    let block = |c0: u16, c1: u16, bits: u32| {
        let mut o = [0u8; 8];
        o[..2].copy_from_slice(&c0.to_le_bytes());
        o[2..4].copy_from_slice(&c1.to_le_bytes());
        o[4..].copy_from_slice(&bits.to_le_bytes());
        o
    };
    if steps == Steps::Four && wa == wb {
        return block(wa, wb, 0);
    }
    let ca: C3<F> = {
        let d = decode565::<F>(wa);
        std::array::from_fn(|k| d[k] * l[k])
    };
    let cb: C3<F> = {
        let d = decode565::<F>(wb);
        std::array::from_fn(|k| d[k] * l[k])
    };
    let ((c0, s0), (c1, s1)) = if (steps == Steps::Three) == (wa <= wb) { ((wa, ca), (wb, cb)) } else { ((wb, cb), (wa, ca)) };
    let order: &[u32] = match steps {
        Steps::Three => &[0, 2, 1],
        Steps::Four => &[0, 2, 3, 1],
    };
    let fsteps = F::of((steps.n() - 1) as f32);
    let dir: C3<F> = std::array::from_fn(|k| s1[k] - s0[k]);
    let scale = if wa != wb { fsteps / (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]) } else { F::of(0.0) };
    let dir = dir.map(|v| v * scale);
    let mut dw = 0u32;
    for c in px {
        let i = if steps == Steps::Three && c[3] < F::of(0.5) {
            3
        } else {
            let p: C3<F> = std::array::from_fn(|k| c[k] * l[k]);
            let dot = (p[0] - s0[0]) * dir[0] + (p[1] - s0[1]) * dir[1] + (p[2] - s0[2]) * dir[2];
            if dot <= F::of(0.0) {
                0
            } else if dot >= fsteps {
                1
            } else {
                order[usize::try_from((dot + F::of(0.5)).trunc_i()).unwrap_or(0)]
            }
        };
        dw = i << 30 | dw >> 2;
    }
    block(c0, c1, dw)
}

/// DXT2 and DXT3's explicit alpha: each texel's alpha in 4 bits
pub fn encode_alpha4<F: Real>(px: &[[F; 4]; 16]) -> [u8; 8] {
    let mut w = 0u64;
    for (i, c) in px.iter().enumerate() {
        let a = u64::try_from((clamp01(c[3]) * F::of(15.0) + F::of(0.5)).trunc_i()).unwrap_or(0) & 15;
        w |= a << (4 * i);
    }
    w.to_le_bytes()
}

/// the alpha endpoints, Newton's method as for the colours, six steps (and 0 and 1) or eight
fn optimize_alpha<F: Real>(pts: &[F; 16], six: bool) -> (F, F) {
    let n = if six { 6 } else { 8 };
    let fs = |i: usize| F::of(i as f32 / (n - 1) as f32);
    let pc = |i: usize| F::of((n - 1 - i) as f32 / (n - 1) as f32);
    let (mut x, mut y) = (F::of(1.0), F::of(0.0));
    if !six {
        for &p in pts {
            if p < x {
                x = p;
            }
            if p > y {
                y = p;
            }
        }
    } else {
        for &p in pts {
            if p < x && p > F::of(0.0) {
                x = p;
            }
            if p > y && p < F::of(1.0) {
                y = p;
            }
        }
        if x == y {
            y = F::of(1.0);
        }
    }
    let fsteps = F::of((n - 1) as f32);
    for _ in 0..8 {
        if (y - x) < F::of(1.0 / 256.0) {
            break;
        }
        let scale = fsteps / (y - x);
        let mut st: Vec<F> = (0..n).map(|i| pc(i) * x + fs(i) * y).collect();
        if six {
            st.push(F::of(0.0));
            st.push(F::of(1.0));
        }
        let (mut dx, mut dy, mut d2x, mut d2y) = (F::of(0.0), F::of(0.0), F::of(0.0), F::of(0.0));
        for &p in pts {
            let dot = (p - x) * scale;
            let i = if dot <= F::of(0.0) {
                if six && p <= (x * F::of(0.5)) { 6 } else { 0 }
            } else if dot >= fsteps {
                if six && p >= ((y + F::of(1.0)) * F::of(0.5)) { 7 } else { n - 1 }
            } else {
                usize::try_from((dot + F::of(0.5)).trunc_i()).unwrap_or(0)
            };
            // (MEASURED loadtex-into-DXT4, -DXT5: the miss as the point less the step, the
            // other way 22 of 90 blocks differ)
            if i < n {
                let diff = p - st[i];
                dx = dx + pc(i) * diff;
                d2x = d2x + pc(i) * pc(i);
                dy = dy + fs(i) * diff;
                d2y = d2y + fs(i) * fs(i);
            }
        }
        if d2x > F::of(0.0) {
            x = x - dx / d2x;
        }
        if d2y > F::of(0.0) {
            y = y - dy / d2y;
        }
        if x > y {
            std::mem::swap(&mut x, &mut y);
        }
        if dx * dx < F::of(1.0 / 64.0) && dy * dy < F::of(1.0 / 64.0) {
            break;
        }
    }
    (clamp01(x), clamp01(y))
}


/// DXT4 and DXT5's interpolated alpha
pub fn encode_alpha8<F: Real>(px: &[[F; 4]; 16]) -> [u8; 8] {
    let a: [F; 16] = std::array::from_fn(|i| F::of(f32::from(u8::try_from((px[i][3] * F::of(255.0) + F::of(0.5)).trunc_i().clamp(0, 255)).unwrap_or(0))) * F::of(1.0 / 255.0));
    let (mut lo, mut hi) = (a[0], a[0]);
    for &v in &a {
        if v < lo {
            lo = v;
        } else if v > hi {
            hi = v;
        }
    }
    let mut o = [0u8; 8];
    if lo == F::of(1.0) {
        o[0] = 0xff;
        o[1] = 0xff;
        return o;
    }
    let six = lo == F::of(0.0) || hi == F::of(1.0);
    let (fa, fb) = optimize_alpha(&a, six);
    let byte = |v: F| u8::try_from((v * F::of(255.0) + F::of(0.5)).trunc_i().clamp(0, 255)).unwrap_or(0);
    let (ba, bb) = (byte(fa), byte(fb));
    let (fa, fb) = ((F::of(f32::from(ba)) * F::of(1.0 / 255.0)), (F::of(f32::from(bb)) * F::of(1.0 / 255.0)));
    if !six && ba == bb {
        o[0] = ba;
        o[1] = bb;
        return o;
    }
    let mut st = [F::of(0.0); 8];
    let order: &[u64] = if six {
        o[0] = ba;
        o[1] = bb;
        st[0] = fa;
        st[1] = fb;
        for i in 1..5 {
            st[i + 1] = (st[0] * F::of((5 - i) as f32) + st[1] * F::of(i as f32)) * F::of(1.0 / 5.0);
        }
        st[6] = F::of(0.0);
        st[7] = F::of(1.0);
        &[0, 2, 3, 4, 5, 1]
    } else {
        o[0] = bb;
        o[1] = ba;
        st[0] = fb;
        st[1] = fa;
        for i in 1..7 {
            st[i + 1] = (st[0] * F::of((7 - i) as f32) + st[1] * F::of(i as f32)) * F::of(1.0 / 7.0);
        }
        &[0, 2, 3, 4, 5, 6, 7, 1]
    };
    let n = if six { 6 } else { 8 };
    let fsteps = F::of((n - 1) as f32);
    let scale = if st[0] != st[1] { fsteps / (st[1] - st[0]) } else { F::of(0.0) };
    let mut bits = 0u64;
    for (i, c) in px.iter().enumerate() {
        let p = c[3];
        let dot = (p - st[0]) * scale;
        let k = if dot <= F::of(0.0) {
            if six && p <= (st[0] * F::of(0.5)) { 6 } else { 0 }
        } else if dot >= fsteps {
            if six && p >= ((st[1] + F::of(1.0)) * F::of(0.5)) { 7 } else { 1 }
        } else {
            order[usize::try_from((dot + F::of(0.5)).trunc_i()).unwrap_or(0)]
        };
        bits |= k << (3 * i);
    }
    o[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    o
}

