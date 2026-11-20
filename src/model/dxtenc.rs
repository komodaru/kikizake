//! D3DX's DXT encoder, as D3DXLoadSurfaceFromMemory writes into DXT1 .. DXT5 (d3dx9load
//! loadtex-into-DXT*): the block's colours quantized to 565, endpoints found by Newton's method
//! on the luminance-weighted colours, the indices by projection on the endpoints' line.
//! Generic over the float it works in: x64 in f32, x86 on the x87 (X87) with the rounding
//! control the encoder sets around its loops (the clean-room notes).

use std::ops::{Add, Div, Mul, Neg, Sub};

pub trait Real: Copy + PartialOrd + Add<Output = Self> + Sub<Output = Self> + Mul<Output = Self> + Div<Output = Self> + Neg<Output = Self> {
    fn of(v: f32) -> Self;
    /// toward zero, as C's (int) cast
    fn trunc_i(self) -> i32;
    /// stored to an f32 variable
    fn store(self) -> Self;
    /// the x87's rounding control toward zero (true) or to nearest, from here on (only the x87
    /// has one, the clean-room notes)
    fn chop(_on: bool) {}
    /// the x87 build (OptimizeRGB has no test for a dot below 0 there, the clean-room notes)
    const X87: bool = false;
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

/// a value on the x87 as the x86 build computes: every operation rounded to the thread's
/// precision with the rounding control the encoder set (the clean-room notes)
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct X87(pub f64);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Rc {
    Nearest,
    Chop,
}

thread_local! {
    static RC: std::cell::Cell<Rc> = const { std::cell::Cell::new(Rc::Nearest) };
}

/// the precision control's mantissa bits: single precision, as Direct3D 9 sets it on
/// CreateDevice without D3DCREATE_FPU_PRESERVE (MEASURED loadtex-into-DXT*: 90 of 90 blocks;
/// double precision 82)
fn pc_bits() -> u32 {
    24
}

/// the exact hi + lo rounded to `bits` with the current rounding control
fn round_x87(hi: f64, lo: f64, bits: u32) -> f64 {
    let rc = RC.with(std::cell::Cell::get);
    if hi == 0.0 || !hi.is_finite() {
        return hi;
    }
    // (the exact value's magnitude below hi's)
    let below = lo != 0.0 && lo.signum() != hi.signum();
    if bits == 53 {
        return match rc {
            Rc::Chop if below => f64::from_bits(hi.to_bits() - 1),
            Rc::Chop | Rc::Nearest => hi,
        };
    }
    let near = hi as f32;
    let down = |m: f32| f64::from(f32::from_bits(m.to_bits() - 1));
    match rc {
        Rc::Chop => {
            if f64::from(near).abs() > hi.abs() || f64::from(near) == hi && below {
                down(near)
            } else {
                f64::from(near)
            }
        }
        Rc::Nearest => {
            let other = 2.0 * hi - f64::from(near);
            if lo != 0.0 && f64::from(other as f32) == other && other != f64::from(near) {
                // hi is the midpoint of two f32: lo decides
                if (lo > 0.0) == (f64::from(near) > hi) { f64::from(near) } else { other }
            } else {
                f64::from(near)
            }
        }
    }
}

fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let s = a + b;
    let bb = s - a;
    (s, (a - (s - bb)) + (b - bb))
}

impl Add for X87 {
    type Output = X87;
    fn add(self, o: X87) -> X87 {
        let (s, e) = two_sum(self.0, o.0);
        X87(round_x87(s, e, pc_bits()))
    }
}
impl Sub for X87 {
    type Output = X87;
    fn sub(self, o: X87) -> X87 {
        self + X87(-o.0)
    }
}
impl Mul for X87 {
    type Output = X87;
    fn mul(self, o: X87) -> X87 {
        let p = self.0 * o.0;
        X87(round_x87(p, self.0.mul_add(o.0, -p), pc_bits()))
    }
}
impl Div for X87 {
    type Output = X87;
    fn div(self, o: X87) -> X87 {
        let q = self.0 / o.0;
        let r = (-q).mul_add(o.0, self.0);
        X87(round_x87(q, r / o.0, pc_bits()))
    }
}
impl Neg for X87 {
    type Output = X87;
    fn neg(self) -> X87 {
        X87(-self.0)
    }
}
impl Real for X87 {
    fn of(v: f32) -> Self {
        X87(f64::from(v))
    }
    fn trunc_i(self) -> i32 {
        // (fistp toward zero; out of range the integer indefinite)
        if self.0.is_nan() || self.0 >= 2147483648.0 || self.0 < -2147483648.0 { i32::MIN } else { self.0 as i32 }
    }
    fn store(self) -> Self {
        X87(round_x87(self.0, 0.0, 24))
    }
    fn chop(on: bool) {
        RC.with(|c| c.set(if on { Rc::Chop } else { Rc::Nearest }));
    }
    const X87: bool = true;
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
    let q = |v: F, m: f32| u16::try_from((clamp01(v) * F::of(m) + F::of(0.5)).store().trunc_i()).unwrap_or(0);
    F::chop(true);
    let w = q(c[0], 31.0) << 11 | q(c[1], 63.0) << 5 | q(c[2], 31.0);
    F::chop(false);
    w
}

fn decode565<F: Real>(w: u16) -> C3<F> {
    let u = |v: u16, m: f32| F::of(f32::from(v)) * F::of(1.0 / m);
    [u(w >> 11 & 31, 31.0), u(w >> 5 & 63, 63.0), u(w & 31, 31.0)]
}

/// the endpoints of the block's points (luminance-weighted colours)
fn optimize_rgb<F: Real>(pts: &[C3<F>; 16], steps: Steps) -> Option<(C3<F>, C3<F>)> {
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
        return Some((x, y));
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
        return Some((x, y));
    }
    let n = steps.n();
    let fsteps = F::of((n - 1) as f32);
    F::chop(true);
    let mut bad = false;
    for _ in 0..8 {
        let st: Vec<C3<F>> = (0..n).map(|i| std::array::from_fn(|k| (F::of(pd[i]) * y[k] + x[k] * F::of(pc[i])).store())).collect();
        let d: C3<F> = std::array::from_fn(|k| (y[k] - x[k]).store());
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).store();
        if len < F::of(1.0 / 4096.0) {
            break;
        }
        let scale = (fsteps / len).store();
        let d: C3<F> = d.map(|v| (v * scale).store());
        let (mut d2x, mut d2y) = (F::of(0.0), F::of(0.0));
        let (mut dx, mut dy) = ([F::of(0.0); 3], [F::of(0.0); 3]);
        for p in pts {
            let dot = (((p[1] - x[1]) * d[1] + (p[0] - x[0]) * d[0]) + (p[2] - x[2]) * d[2]).store();
            let i = if !F::X87 && dot <= F::of(0.0) {
                0
            } else if !(fsteps > dot) {
                n - 1
            } else {
                // (x87: no test below 0; a dot at or below -1.5 indexes outside the table)
                let t = (dot + F::of(0.5)).store().trunc_i();
                bad |= t < 0;
                usize::try_from(t).unwrap_or(0)
            };
            let diff: C3<F> = std::array::from_fn(|k| (st[i][k] - p[k]).store());
            let fc = (F::of(pc[i]) * F::of(1.0 / 8.0)).store();
            let fd = (F::of(1.0 / 8.0) * F::of(pd[i])).store();
            d2x = (F::of(pc[i]) * fc + d2x).store();
            for k in 0..3 {
                dx[k] = (fc * diff[k] + dx[k]).store();
            }
            d2y = (fd * F::of(pd[i]) + d2y).store();
            for k in 0..3 {
                dy[k] = (fd * diff[k] + dy[k]).store();
            }
        }
        if d2x > F::of(0.0) {
            let f = (-(F::of(1.0) / d2x)).store();
            for k in 0..3 {
                x[k] = (f * dx[k] + x[k]).store();
            }
        }
        if d2y > F::of(0.0) {
            let f = (-(F::of(1.0) / d2y)).store();
            for k in 0..3 {
                y[k] = (f * dy[k] + y[k]).store();
            }
        }
        if (0..3).all(|k| dx[k] * dx[k] < eps && dy[k] * dy[k] < eps) {
            break;
        }
    }
    F::chop(false);
    if bad {
        return None;
    }
    Some((x, y))
}

/// a DXT1 colour block of 16 (r, g, b, a) in 0..1; where steps is Three, the texels whose
/// alpha is below one half become index 3
pub fn encode_bc1<F: Real>(px: &[[F; 4]; 16], steps: Steps) -> Option<[u8; 8]> {
    let l = lum::<F>();
    let q = |v: F, m: f32| (F::of(f32::from(u8::try_from((v * F::of(m) + F::of(0.5)).store().trunc_i().clamp(0, 255)).unwrap_or(0))) * F::of(1.0 / m)).store();
    F::chop(true);
    let pts: [C3<F>; 16] = std::array::from_fn(|i| {
        let c = px[i];
        [(q(c[0], 31.0) * l[0]).store(), (q(c[1], 63.0) * l[1]).store(), (q(c[2], 31.0) * l[2]).store()]
    });
    F::chop(false);
    let (a, b) = optimize_rgb(&pts, steps)?;
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
        return Some(block(wa, wb, 0));
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
    F::chop(true);
    for c in px {
        let i = if steps == Steps::Three && c[3] < F::of(0.5) {
            3
        } else {
            let p: C3<F> = std::array::from_fn(|k| (c[k] * l[k]).store());
            let dot = (((p[1] - s0[1]) * dir[1] + (p[0] - s0[0]) * dir[0]) + (p[2] - s0[2]) * dir[2]).store();
            if dot <= F::of(0.0) {
                0
            } else if dot >= fsteps {
                1
            } else {
                order[usize::try_from((dot + F::of(0.5)).store().trunc_i()).unwrap_or(0)]
            }
        };
        dw = i << 30 | dw >> 2;
    }
    F::chop(false);
    Some(block(c0, c1, dw))
}

/// DXT2 and DXT3's explicit alpha: each texel's alpha in 4 bits
pub fn encode_alpha4<F: Real>(px: &[[F; 4]; 16]) -> [u8; 8] {
    let mut w = 0u64;
    F::chop(true);
    for (i, c) in px.iter().enumerate() {
        let a = u64::try_from((clamp01(c[3]) * F::of(15.0) + F::of(0.5)).store().trunc_i()).unwrap_or(0) & 15;
        w |= a << (4 * i);
    }
    F::chop(false);
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
    F::chop(true);
    for _ in 0..8 {
        if (y - x) < F::of(1.0 / 256.0) {
            break;
        }
        let scale = (fsteps / (y - x)).store();
        let mut st: Vec<F> = (0..n).map(|i| (fs(i) * y + pc(i) * x).store()).collect();
        if six {
            st.push(F::of(0.0));
            st.push(F::of(1.0));
        }
        let (mut dx, mut dy, mut d2x, mut d2y) = (F::of(0.0), F::of(0.0), F::of(0.0), F::of(0.0));
        for &p in pts {
            let dot = ((p - x) * scale).store();
            let i = if dot <= F::of(0.0) {
                if six && p <= (x * F::of(0.5)).store() { 6 } else { 0 }
            } else if dot >= fsteps {
                if six && p >= ((y + F::of(1.0)) * F::of(0.5)).store() { 7 } else { n - 1 }
            } else {
                usize::try_from((dot + F::of(0.5)).store().trunc_i()).unwrap_or(0)
            };
            // (MEASURED loadtex-into-DXT4, -DXT5: the miss as the point less the step, the
            // other way 22 of 90 blocks differ)
            if i < n {
                let diff = (p - st[i]).store();
                dx = (diff * pc(i) + dx).store();
                d2x = (pc(i) * pc(i) + d2x).store();
                dy = (diff * fs(i) + dy).store();
                d2y = (fs(i) * fs(i) + d2y).store();
            }
        }
        if d2x > F::of(0.0) {
            x = (x - dx / d2x).store();
        }
        if d2y > F::of(0.0) {
            y = (y - dy / d2y).store();
        }
        if x > y {
            std::mem::swap(&mut x, &mut y);
        }
        if dx * dx < F::of(1.0 / 64.0) && dy * dy < F::of(1.0 / 64.0) {
            break;
        }
    }
    F::chop(false);
    (clamp01(x), clamp01(y))
}


/// DXT4 and DXT5's interpolated alpha
pub fn encode_alpha8<F: Real>(px: &[[F; 4]; 16]) -> [u8; 8] {
    F::chop(true);
    let a: [F; 16] = std::array::from_fn(|i| (F::of(f32::from(u8::try_from((px[i][3] * F::of(255.0) + F::of(0.5)).store().trunc_i().clamp(0, 255)).unwrap_or(0))) * F::of(1.0 / 255.0)).store());
    F::chop(false);
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
    let byte = |v: F| u8::try_from((v * F::of(255.0) + F::of(0.5)).store().trunc_i().clamp(0, 255)).unwrap_or(0);
    F::chop(true);
    let (ba, bb) = (byte(fa), byte(fb));
    F::chop(false);
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
            st[i + 1] = ((F::of((5 - i) as f32) * st[0] + F::of(i as f32) * st[1]) * F::of(0.2)).store();
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
            st[i + 1] = ((F::of((7 - i) as f32) * st[0] + F::of(i as f32) * st[1]) * F::of(1.0 / 7.0)).store();
        }
        &[0, 2, 3, 4, 5, 6, 7, 1]
    };
    let n = if six { 6 } else { 8 };
    let fsteps = F::of((n - 1) as f32);
    let scale = if st[0] != st[1] { fsteps / (st[1] - st[0]) } else { F::of(0.0) };
    let mut bits = 0u64;
    F::chop(true);
    for (i, c) in px.iter().enumerate() {
        let p = c[3];
        let dot = ((p - st[0]) * scale).store();
        let k = if dot <= F::of(0.0) {
            if six && p <= (st[0] * F::of(0.5)).store() { 6 } else { 0 }
        } else if dot >= fsteps {
            if six && p >= ((st[1] + F::of(1.0)) * F::of(0.5)).store() { 7 } else { 1 }
        } else {
            order[usize::try_from((dot + F::of(0.5)).store().trunc_i()).unwrap_or(0)]
        };
        bits |= k << (3 * i);
    }
    F::chop(false);
    o[2..].copy_from_slice(&bits.to_le_bytes()[..6]);
    o
}

