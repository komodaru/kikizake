//! GDI+ DrawImage under a scale and translate: the chain from the rectangles to the span
//! (GpGraphics::DrawImage, DrvDrawImage's ApplyVisibleClipToSourceRect, PipeLockBits' row window,
//! DpDriver::DrawImage), the device pixels it writes, and the nearest, bilinear and bicubic
//! spans, from the clean-room notes (image.md 12), checked on the goldens of both arches. Every
//! float op in the arch's rounding (x64 down, x86 nearest).
use super::gdiplus::{invert_in, mul_in, Matrix, Round};
use super::gplinear::infer;
use super::gppath::{apply, complexity, to32, Complexity, NEAR};
use super::gpwiden::sqrt64;
use super::gpstretch::Kernel;
use super::{Fail, Res};
use crate::spec::{clause, hit};
use std::ops::Range;

/// the span CreateOutputSpan picks by the stored mode (0, 1 store as 3, 2 as 7)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Span {
    Nearest,
    Bilinear,
    Bicubic,
    Stretch(Kernel),
}

impl Span {
    /// Margin(mode, scale): the rows kept past the source rectangle
    fn margin(self, r: Round, scale: f32) -> f32 {
        match self {
            Span::Nearest => 0.0,
            Span::Bilinear => 1.0,
            Span::Bicubic => 2.0,
            Span::Stretch(k) => k.margin(r, scale),
        }
    }
}

/// what the span gets: the locked rows top .. top + rows of the source, the source rectangle
/// relative to them (Sf), the span matrix (Ms2), the device rectangle (D, negative sizes for
/// flips), and the device pixels written
#[derive(Clone, Debug)]
pub struct Chain {
    pub top: i64,
    pub rows: i64,
    pub sf: (f32, f32, f32, f32),
    pub ms2: Matrix,
    pub d: (f32, f32, f32, f32),
    pub cols: Range<i64>,
    pub lines: Range<i64>,
    /// the source read flipped (the rotate / flip branch's clone)
    pub flip: Option<Flip>,
    /// the copy span's offset: device (x, y) reads locked (x + ox, y + oy)
    pub copy: Option<(i64, i64)>,
}

/// AnalyzeRotateFlip's answers a scale and translate can give (RotateFlipType 4, 6, 2)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flip {
    X,
    Y,
    Both,
}

/// IsIntegerTranslate: no scale (within ComputeComplexity's tolerance), each translation within
/// 1/64 of an integer
pub fn integer_translate(r: Round, a: &Matrix) -> bool {
    let moves = match complexity(r, a) {
        Complexity::Identity | Complexity::Translate => true,
        Complexity::Scale | Complexity::General => false,
    };
    moves && [a[4], a[5]].iter().all(|&v| r.sub(r.add(v, 0.5).floor(), v).abs() <= 1.0 / 64.0)
}

/// AnalyzeRotateFlip of the chain's matrix: m12 and m21 within 2^-23 of 0 (InferAffineMatrix of
/// axis-aligned points; its rotations 1, 3, 5, 7 want m21 and m12 near +-1, a world transform,
/// which DrawImageRectRectI has not), each scale +-1 within 2^-23, strictly
pub fn analyze(r: Round, m: &Matrix) -> Option<Flip> {
    let near = |a: f32, c: f32| r.sub(a, c).abs() < EPS;
    if !(m[1].abs() < EPS && m[2].abs() < EPS) {
        return None;
    }
    match (near(m[0], 1.0), near(m[0], -1.0), near(m[3], 1.0), near(m[3], -1.0)) {
        (true, _, _, true) => Some(Flip::Y),
        (_, true, true, _) => Some(Flip::X),
        (_, true, _, true) => Some(Flip::Both),
        _ => None,
    }
}
const EPS: f32 = f32::EPSILON;

/// where a draw goes
#[derive(Clone, Debug)]
pub enum Front {
    Draw(Chain),
    /// no rows locked: nothing drawn
    Empty,
}

/// the rectangles (integers, sizes signed) of a draw onto a bw x bh bitmap from one ih high
/// the larger, the second on a tie (the signed zeros among them), the other past a NaN
fn most(a: f32, b: f32) -> f32 {
    if b.is_nan() || a > b { a } else { b }
}

/// the smaller, the second on a tie (the signed zeros among them), the other past a NaN
fn least(a: f32, b: f32) -> f32 {
    if b.is_nan() || a < b { a } else { b }
}

pub fn chain(r: Round, span: Span, dst: [i64; 4], src: [i64; 4], (iw, ih): (i64, i64), (bw, bh): (i64, i64), half: bool) -> Res<Front> {
    let f = |v: i64| v as f32;
    let [dx, dy, dw, dh] = dst.map(f);
    let [mut sx, mut sy, mut sw, mut sh] = src.map(f);
    // (GpGraphics::DrawImage: the matrix from the source as given, signs kept)
    let (xr, yb, sxr, syb) = (r.add(dx, dw), r.add(dy, dh), r.add(sw, sx), r.add(sh, sy));
    let (mut m11, mut m22, mut tx, mut ty) = (1.0, 1.0, 0.0, 0.0);
    if !(sx == sxr || sy == syb) {
        m11 = r.div(r.sub(xr, dx), r.sub(sxr, sx));
        tx = r.sub(xr, r.mul(m11, sxr));
        m22 = r.div(r.sub(yb, dy), r.sub(syb, sy));
        ty = r.sub(yb, r.mul(m22, syb));
    }
    // (then the source rectangle normalized)
    if sw < 0.0 {
        (sx, sw) = (sxr, -sw);
    }
    if sh < 0.0 {
        (sy, sh) = (syb, -sh);
    }
    if half {
        tx = r.add(tx, r.add(r.mul(0.0, 0.5), r.mul(m11, 0.5)));
        ty = r.add(ty, r.add(r.mul(0.0, 0.5), r.mul(m22, 0.5)));
        sx = r.add(sx, -0.5);
        sy = r.add(sy, -0.5);
    }
    let ms: Matrix = [m11, 0.0, 0.0, m22, tx, ty];
    let mut p = [apply(r, &ms, sx, sy), apply(r, &ms, r.add(sx, sw), sy), apply(r, &ms, sx, r.add(sy, sh))];
    let mut s = (sx, sy, sw, sh);
    // (DrvDrawImage: the world to device matrix carries the half pixel offset)
    let o = if half { -0.5 } else { 0.0 };
    let mut c: Matrix = [1.0, 0.0, 0.0, 1.0, o, o];
    // (a source of no area: InferAffineMatrix refuses, the rectangles being whole pixels the
    // determinant is never under FLT_EPSILON otherwise)
    let mut m = mul_in(r, &infer(r, p, s).ok_or(Fail::Bad)?, &c);
    // (DrvDrawImage's rotate / flip branch: a scale of +-1, not an integer translation; the source
    // a flipped PARGB clone, the device matrix a translation, clean-room notes image.md 13)
    let flip = if integer_translate(r, &m) { None } else { analyze(r, &m) };
    if let Some(fl) = flip {
        hit(clause::GP_DRAW_SCALED);
        let (mut tx, mut ty) = (m[4], m[5]);
        if half {
            tx = r.add(r.add(tx, 0.5), r.sub(r.mul(m[0], -0.5), r.mul(m[2], 0.5)));
            ty = r.add(r.add(ty, 0.5), r.sub(r.mul(m[3], -0.5), r.mul(m[1], 0.5)));
            s.0 = r.add(s.0, 0.5);
            s.1 = r.add(s.1, 0.5);
        }
        let (w, h) = (f(iw), f(ih));
        let mirror = |v: f32, n: f32, size: f32| r.add(v, r.sub(n, r.add(r.mul(v, 2.0), size)));
        match fl {
            Flip::X => {
                tx = r.add(tx, r.add(r.mul(-w, 1.0), r.mul(0.0, 0.0)));
                ty = r.add(ty, r.add(r.mul(-w, 0.0), r.mul(0.0, 1.0)));
                s.0 = mirror(s.0, w, s.2);
            }
            Flip::Y => {
                ty = r.add(ty, r.add(-h, 0.0));
                tx = r.add(tx, r.add(r.mul(-h, 0.0), 0.0));
                s.1 = mirror(s.1, h, s.3);
            }
            Flip::Both => {
                tx = r.add(tx, r.add(r.mul(-w, 1.0), r.mul(-h, 0.0)));
                ty = r.add(ty, r.add(r.mul(-w, 0.0), r.mul(-h, 1.0)));
                s.1 = mirror(s.1, h, s.3);
                s.0 = mirror(s.0, w, s.2);
            }
        }
        c = [1.0, 0.0, 0.0, 1.0, tx, ty];
        p = [(s.0, s.1), (r.add(s.0, s.2), s.1), (s.0, r.add(s.1, s.3))];
        m = mul_in(r, &infer(r, p, s).ok_or(Fail::Bad)?, &c);
    }
    // (ApplyVisibleClipToSourceRect, the visible rectangle the bitmap's through the device to
    // world matrix, image.md 12e, 13f)
    let shift = invert_in(r, &c).ok_or(Fail::Bad)?;
    let vis = {
        let (x, y) = (r.add(0.0, shift[4]), r.add(0.0, shift[5]));
        (x, y, r.sub(r.add(f(bw), shift[4]), x), r.sub(r.add(f(bh), shift[5]), y))
    };
    let q = invert_in(r, &mul_in(r, &m, &shift)).ok_or(Fail::Bad)?;
    let (vx, vy, vw, vh) = vis;
    let ys = [(vx, vy), (r.add(vx, vw), vy), (vx, r.add(vy, vh)), (r.add(vx, vw), r.add(vy, vh))].map(|(x, y)| apply(r, &q, x, y).1);
    let eps = -f32::EPSILON;
    let (mut top, mut hh) = (r.add(s.1, 1.0), r.add(s.3, -2.0));
    let (mut left, mut ww) = (r.add(s.0, 1.0), r.add(s.2, -2.0));
    let mut changed = false;
    for (k, &yk) in ys.iter().enumerate() {
        if yk > top && r.add(hh, top) > yk {
            let others = || ys.iter().enumerate().filter(move |&(j, _)| j != k).map(|(_, &y)| y);
            if others().all(|yj| r.sub(yj, yk) > eps) {
                let d = r.sub(yk, top);
                top = yk;
                hh = r.sub(hh, d);
                changed = true;
            } else if others().all(|yj| r.sub(yk, yj) > eps) {
                hh = r.sub(yk, top);
                changed = true;
            }
        }
    }
    if changed {
        let scale = to32(r, sqrt64(r, f64::from(r.add(r.mul(m[2], m[2]), r.mul(m[3], m[3])))));
        let mg = span.margin(r, scale);
        ww = r.add(ww, 2.0);
        left = r.sub(left, 1.0);
        top = r.sub(top, 1.0);
        hh = r.add(hh, 2.0);
        left = r.sub(left, 0.0);
        ww = r.add(ww, 0.0);
        top = r.sub(top, mg);
        let bot = least(r.add(r.add(r.add(mg, mg), hh), top), r.add(s.1, s.3));
        let right = least(r.add(ww, left), r.add(s.0, s.2));
        let (left, top) = (most(left, s.0), most(top, s.1));
        s = (left, top, r.sub(right, left), r.sub(bot, top));
        let b = invert_in(r, &q).ok_or(Fail::Bad)?;
        p = [apply(r, &b, left, top), apply(r, &b, r.add(s.2, left), top), apply(r, &b, left, r.add(s.3, top))];
    }
    // (PipeLockBits: the rows, by the y ratio of the lock matrix)
    let s1 = if half { (r.add(s.0, 0.5), r.add(s.1, 0.5), s.2, s.3) } else { s };
    let mut l = m;
    if half {
        l[4] = r.add(l[4], 0.5);
        l[5] = r.add(l[5], 0.5);
        l[4] = r.add(l[4], r.add(r.mul(-0.5, l[0]), r.mul(-0.5, l[2])));
        l[5] = r.add(l[5], r.add(r.mul(-0.5, l[1]), r.mul(-0.5, l[3])));
    }
    let (ya, yz) = (apply(r, &l, s1.0, s1.1).1, apply(r, &l, r.add(s1.2, s1.0), r.add(s1.3, s1.1)).1);
    let oy = least(ya, yz);
    let oh = r.sub(most(ya, yz), oy);
    let oh = if oh < NEAR { 0.0 } else { oh };
    let ratio = r.div(r.sub(r.add(oy, oh), oy).abs(), s1.3.abs());
    let mg = span.margin(r, ratio).ceil() as i64;
    let first = s1.1.floor() as i64 - mg;
    let last = r.add(s1.3.abs(), s1.1).ceil() as i64 + mg;
    if first > ih || last < 0 {
        return Ok(Front::Empty);
    }
    let (first, last) = (first.max(0), last.min(ih));
    let lock = (s1.0, r.sub(s1.1, first as f32), s1.2, s1.3);
    let sf = if half { (r.add(lock.0, -0.5), r.add(lock.1, -0.5), lock.2, lock.3) } else { lock };
    // (DpDriver::DrawImage: the span matrix, the device rectangle its corners)
    let ms2 = mul_in(r, &infer(r, p, sf).ok_or(Fail::Bad)?, &c);
    let x0 = r.add(r.mul(sf.0, ms2[0]), ms2[4]);
    let x2 = r.add(r.mul(r.add(sf.0, sf.2), ms2[0]), ms2[4]);
    let y0 = r.add(r.mul(sf.1, ms2[3]), ms2[5]);
    let y2 = r.add(r.mul(r.add(sf.1, sf.3), ms2[3]), ms2[5]);
    // (the device pixels: the points to 1/16, ceiled past the offset)
    // (the points through the device translation (exactly), to 1/16, ceiled)
    let to16 = |v: f64| (v * 16.0 + 0.5).floor() / 16.0;
    let ex = |v: f32| to16(f64::from(v) + f64::from(c[4])).ceil() as i64;
    let ey = |v: f32| to16(f64::from(v) + f64::from(c[5])).ceil() as i64;
    let (xl, xh) = (least(p[0].0, p[1].0), most(p[0].0, p[1].0));
    let (yl, yh) = (least(p[0].1, p[2].1), most(p[0].1, p[2].1));
    // (CreateOutputSpan: an integer translation the copy span, whatever the mode, Clamp)
    let copy = integer_translate(r, &ms2).then(|| (-(r.add(ms2[4], 0.5).floor() as i64), -(r.add(ms2[5], 0.5).floor() as i64)));
    Ok(Front::Draw(Chain {
        top: first,
        rows: last - first,
        sf,
        ms2,
        d: (x0, y0, r.sub(x2, x0), r.sub(y2, y0)),
        cols: ex(xl).max(0)..ex(xh).min(bw),
        lines: ey(yl).max(0)..ey(yh).min(bh),
        flip,
        copy,
    }))
}

/// a premultiplied source pixel of the locked rows, 0 past them or past the bitmap's columns
pub trait Read {
    fn at(&self, x: i64, y: i64) -> Res<u32>;
}
impl<F: Fn(i64, i64) -> Res<u32>> Read for F {
    fn at(&self, x: i64, y: i64) -> Res<u32> {
        self(x, y)
    }
}

fn channel(p: u32, c: usize) -> i64 {
    i64::from(p >> (8 * c) & 255)
}
fn pack(c: [i64; 4]) -> u32 {
    (0..4).fold(0, |o, k| o | ((c[k] & 255) as u32) << (8 * k))
}
/// floor(v + 1/2) of a float product v 65536, the add rounded
fn fix16(r: Round, v: f32) -> i32 {
    r.add(r.mul(v, 65536.0), 0.5).floor() as i32
}

/// the span's pixels of one draw, (x, y, premultiplied color), the source iw wide
pub fn draw<R: Read>(r: Round, span: Span, ch: &Chain, iw: i64, x86: bool, read: &R) -> Res<Vec<(i64, i64, u32)>> {
    let inv = invert_in(r, &ch.ms2).ok_or(Fail::Bad)?;
    let mut out = Vec::new();
    let (xmin, xmax) = (ch.cols.start, ch.cols.end);
    if xmin >= xmax {
        return Ok(out);
    }
    let n = (xmax - xmin) as f32;
    let tap = |x: i64, y: i64| -> Res<u32> { if (0..iw).contains(&x) && (0..ch.rows).contains(&y) { read.at(x, y) } else { Ok(0) } };
    for y in ch.lines.clone() {
        let (u0, v0) = apply(r, &inv, xmin as f32, y as f32);
        let (u1, v1) = apply(r, &inv, xmax as f32, y as f32);
        // (the 16.16 places of the nearest and bicubic spans)
        let steps = || {
            let (bu, bv) = (fix16(r, u0), fix16(r, v0));
            let du = r.add(r.div(r.mul(r.sub(u1, u0), 65536.0), n), 0.5).floor() as i32;
            let dv = r.add(r.div(r.mul(r.sub(v1, v0), 65536.0), n), 0.5).floor() as i32;
            (bu, bv, du, dv)
        };
        match span {
            Span::Nearest => {
                let (bu, bv, du, dv) = steps();
                for (i, x) in (xmin..xmax).enumerate() {
                    let i = i as i32;
                    let su = bu.wrapping_add(i.wrapping_mul(du)).wrapping_add(0x8000) >> 16;
                    let sv = bv.wrapping_add(i.wrapping_mul(dv)).wrapping_add(0x8000) >> 16;
                    out.push((x, y, tap(i64::from(su), i64::from(sv))?));
                }
            }
            Span::Bilinear if x86 => {
                let e = inv.map(|v| fix16(r, v));
                let du = fix16(r, r.add(r.mul(inv[2], 0.0), inv[0]));
                let dv = fix16(r, r.add(r.mul(0.0, inv[3]), inv[1]));
                let (xm, yy) = (xmin as i32, y as i32);
                let bu = e[0].wrapping_mul(xm).wrapping_add(e[2].wrapping_mul(yy)).wrapping_add(e[4]);
                let bv = e[1].wrapping_mul(xm).wrapping_add(e[3].wrapping_mul(yy)).wrapping_add(e[5]);
                let lerp = |a: i64, b: i64, t: i64| (a + ((((b - a) * t + 128) & 0xffff) >> 8)) & 255;
                for (i, x) in (xmin..xmax).enumerate() {
                    let i = i as i32;
                    let (pu, pv) = (bu.wrapping_add(i.wrapping_mul(du)), bv.wrapping_add(i.wrapping_mul(dv)));
                    let (ix, iy) = (i64::from(pu >> 16), i64::from(pv >> 16));
                    let (fx, fy) = (i64::from((pu & 0xffff) >> 8), i64::from((pv & 0xffff) >> 8));
                    let c = if (-1..iw).contains(&ix) && (-1..ch.rows).contains(&iy) {
                        let q = [tap(ix, iy)?, tap(ix + 1, iy)?, tap(ix, iy + 1)?, tap(ix + 1, iy + 1)?];
                        pack(std::array::from_fn(|k| {
                            let ch = |j: usize| channel(q[j], k);
                            lerp(lerp(ch(0), ch(1), fx), lerp(ch(2), ch(3), fx), fy)
                        }))
                    } else {
                        0
                    };
                    out.push((x, y, c));
                }
            }
            Span::Bilinear => {
                let (du, dv) = (r.div(r.sub(u1, u0), n), r.div(r.sub(v1, v0), n));
                let (mut u, mut v) = (u0, v0);
                for x in xmin..xmax {
                    let (ix, iy) = (u.floor(), v.floor());
                    let fx = i64::from(fix11(r, r.sub(u, ix)));
                    let fy = i64::from(fix11(r, r.sub(v, iy)));
                    let (ix, iy) = (ix as i64, iy as i64);
                    let c = if ix + 1 < 0 || ix >= iw || iy + 1 < 0 || iy >= ch.rows {
                        0
                    } else {
                        let q = [tap(ix, iy)?, tap(ix + 1, iy)?, tap(ix, iy + 1)?, tap(ix + 1, iy + 1)?];
                        pack(std::array::from_fn(|k| {
                            let ch = |j: usize| channel(q[j], k);
                            let t = (ch(1) - ch(0)) * fx + (ch(0) << 11);
                            let b = (ch(3) - ch(2)) * fx + (ch(2) << 11);
                            (t * (2048 - fy) + b * fy + 0x20_0000) >> 22
                        }))
                    };
                    out.push((x, y, c));
                    u = r.add(u, du);
                    v = r.add(v, dv);
                }
            }
            Span::Bicubic => {
                let (bu, bv, du, dv) = steps();
                for (i, x) in (xmin..xmax).enumerate() {
                    let i = i as i32;
                    let (pu, pv) = (bu.wrapping_add(i.wrapping_mul(du)), bv.wrapping_add(i.wrapping_mul(dv)));
                    let (ix, iy) = (i64::from(pu >> 16), i64::from(pv >> 16));
                    let (fx, fy) = (((pu >> 10) & 63) as usize, ((pv >> 10) & 63) as usize);
                    let mut mid = [0u32; 4];
                    for (k, m) in mid.iter_mut().enumerate() {
                        let col = ix - 1 + k as i64;
                        *m = cubic_pass([tap(col, iy - 1)?, tap(col, iy)?, tap(col, iy + 1)?, tap(col, iy + 2)?], fy, x86);
                    }
                    out.push((x, y, cubic_pass(mid, fx, x86)));
                }
            }
            Span::Stretch(_) => return Err(Fail::Bad),
        }
    }
    Ok(out)
}

/// floor((v 2048) + 1/2), the bilinear fraction (0 ..= 2048)
fn fix11(r: Round, v: f32) -> i32 {
    r.add(r.mul(v, 2048.0), 0.5).floor() as i32
}

/// one times Keys' cubic (a = -1/2) at i / 64, i in 0 ..= 128, rounded: exact in f64
pub fn keys(one: f64, i: usize) -> i64 {
    let x = i as f64 / 64.0;
    let k = if x < 1.0 { 1.5 * x * x * x - 2.5 * x * x + 1.0 } else if x < 2.0 { -0.5 * x * x * x + 2.5 * x * x - 4.0 * x + 2.0 } else { 0.0 };
    (one * k + 0.5).floor() as i64
}
/// x64's bicubic table, in 1/65536
pub fn cubic64(i: usize) -> i64 {
    keys(65536.0, i)
}
/// x86's MMX table, in 1/16384, eight entries one off as stored
pub fn cubic86(i: usize) -> i64 {
    keys(16384.0, i)
        + match i {
            15 | 29 | 47 | 61 => 1,
            79 | 85 | 111 | 117 => -1,
            _ => 0,
        }
}

/// a byte from a signed 16-bit saturation
fn sat16(v: i64) -> i64 {
    v.clamp(-32768, 32767)
}

/// four premultiplied pixels (taps -1 ..= +2) through the weights of fraction fr (0 ..= 63)
fn cubic_pass(p: [u32; 4], fr: usize, x86: bool) -> u32 {
    let idx = [64 + fr, fr, 64 - fr, 128 - fr];
    let mut v = [0i64; 4];
    for (c, o) in v.iter_mut().enumerate() {
        *o = if x86 {
            let t: [i64; 4] = std::array::from_fn(|k| ((channel(p[k], c) << 5) * cubic86(idx[k])) >> 16);
            (sat16(sat16(sat16(t[0] + t[1]) + sat16(t[2] + t[3])) + 4) >> 3).clamp(0, 255)
        } else {
            (0..4).map(|k| channel(p[k], c) * cubic64(idx[k])).sum::<i64>() >> 16
        };
    }
    let a = v[3].clamp(0, 255);
    pack([v[0].clamp(0, a), v[1].clamp(0, a), v[2].clamp(0, a), a])
}

/// one draw: the rectangles (sizes signed), the source image's size and the target bitmap's
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Draw {
    pub dst: [i64; 4],
    pub src: [i64; 4],
    pub image: (i64, i64),
    pub target: (i64, i64),
    pub half: bool,
}

/// the pixels a draw writes, (x, y, premultiplied color), the source read through texel (the
/// image's own coordinates, inside it)
pub fn render<F: Fn(i64, i64) -> Res<u32>>(r: Round, span: Span, d: &Draw, x86: bool, texel: F) -> Res<Vec<(i64, i64, u32)>> {
    let ch = match chain(r, span, d.dst, d.src, d.image, d.target, d.half)? {
        Front::Draw(ch) => ch,
        Front::Empty => return Ok(Vec::new()),
    };
    let (top, (iw, ih)) = (ch.top, d.image);
    // (the clone RotateFlip made: rows reversed, columns, or both)
    let read = |x: i64, y: i64| {
        let y = top + y;
        match ch.flip {
            None => texel(x, y),
            Some(Flip::X) => texel(iw - 1 - x, y),
            Some(Flip::Y) => texel(x, ih - 1 - y),
            Some(Flip::Both) => texel(iw - 1 - x, ih - 1 - y),
        }
    };
    if let Some((ox, oy)) = ch.copy {
        hit(clause::GP_DRAW);
        let mut out = Vec::new();
        for y in ch.lines.clone() {
            for x in ch.cols.clone() {
                let (sx, sy) = (x + ox, y + oy);
                out.push((x, y, if (0..iw).contains(&sx) && (0..ch.rows).contains(&sy) { read(sx, sy)? } else { 0 }));
            }
        }
        return Ok(out);
    }
    match span {
        Span::Stretch(k) => super::gpstretch::stretch(r, k, &ch, iw, x86, &read),
        Span::Nearest | Span::Bilinear | Span::Bicubic => draw(r, span, &ch, iw, x86, &read),
    }
}
