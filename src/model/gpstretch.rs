//! GDI+ DrawImage with HighQualityBilinear (6) and HighQualityBicubic (7, and HighQuality 2)
//! under a scale and translate only: DpOutputSpanStretch<0> and <1> (GP.DRAW.SCALED), from the
//! clean-room notes. The source and device rectangles come from DrawImage's matrix chain
//! (GpGraphics::DrawImage, PipeLockBits' row window, DpDriver::DrawImage), then a separable
//! filter: each needed source row stretched across into a cached line, then the lines stretched
//! down. Every float op in the arch's rounding (x64 down, x86 nearest).
use super::gdiplus::{Matrix, Round};
use super::gplinear::infer;
use super::gppath::apply;
use super::Res;
use crate::spec::{clause, hit};

/// which stretch span: the tent (6) or Keys' cubic (7)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kernel {
    Tent,
    Cubic,
}

/// the tent's running integral, every 1/128 of a pixel, in 1/65536
pub const fn tent(i: i64) -> i64 {
    if i <= -128 {
        0
    } else if i <= 0 {
        2 * (128 + i) * (128 + i)
    } else if i <= 128 {
        65536 - 2 * (128 - i) * (128 - i)
    } else {
        65536
    }
}

/// Keys' cubic of a = -1 integrated from -inf to x
fn keys_integral(x: f64) -> f64 {
    let u = x.abs().min(2.0);
    let i = if u < 1.0 { u.powi(4) / 4.0 - 2.0 * u.powi(3) / 3.0 + u } else { 7.0 / 12.0 + (-u.powi(4) / 4.0 + 5.0 * u.powi(3) / 3.0 - 4.0 * u * u + 4.0 * u) - 17.0 / 12.0 };
    0.5 + i.copysign(x)
}
/// the entries the table (single precision made) holds one off the exact integral, i >= 0
const CUBIC_OFF: [(i64, i64); 22] = [(26, 1), (29, 1), (44, 1), (56, 1), (76, 1), (85, 1), (86, 1), (88, 1), (89, 1), (95, 1), (96, 1), (106, 1), (108, 1), (118, 1), (124, 1), (134, 1), (135, 1), (139, 1), (147, 1), (150, 1), (153, 1), (213, -1)];
/// the cubic's running integral, every 1/128 of a pixel at the half (i + 1/2), in 1/65536
pub fn cubic(i: i64) -> i64 {
    if i <= -256 {
        return 0;
    }
    if i >= 256 {
        return 65536;
    }
    if i < 0 {
        return 65536 - cubic(-1 - i);
    }
    let exact = (65536.0 * keys_integral((i as f64 + 0.5) / 128.0) + 0.5).floor() as i64;
    exact + CUBIC_OFF.iter().find(|&&(k, _)| k == i).map_or(0, |&(_, d)| d)
}

impl Kernel {
    /// the running integral at a 16.16 place t from the tap's edge
    fn at(self, t: i64) -> i64 {
        match self {
            Kernel::Tent => tent(t >> 9),
            Kernel::Cubic => cubic(t >> 8),
        }
    }
    /// its half width's multiple of s / d and least half width (16.16)
    fn width(self) -> (f32, i64) {
        match self {
            Kernel::Tent => (1.0, 65536),
            Kernel::Cubic => (2.0, 131072),
        }
    }
    /// Margin(mode, ratio): the rows PipeLockBits keeps past the source rectangle
    fn margin(self, r: Round, ratio: f32) -> f32 {
        let k = self.width().0;
        if ratio >= 1.0 { k } else { r.div(k, ratio) }
    }
}

/// floor(v + 1/2), the add rounded
fn fl(r: Round, v: f32) -> i64 {
    r.add(v, 0.5).floor() as i64
}

/// the source pixels one center reaches: (pixel, the running integral at its far edge)
fn taps(k: Kernel, c: i64, half: i64, inv: i64) -> Vec<(i64, i64)> {
    let first = (c - half + 0xffff) >> 16;
    let last = (c + half + 0xffff) >> 16;
    let mut t = (((first << 16) - c) * inv) >> 16;
    let mut out = Vec::with_capacity((last - first + 1).max(0) as usize);
    for j in first..=last {
        out.push((j, k.at(t)));
        t += inv;
    }
    out
}
/// the weights of taps: each running integral less the one before
fn weights(t: &[(i64, i64)], shift: u32) -> Vec<(i64, i64)> {
    let mut prev = 0;
    t.iter()
        .map(|&(j, v)| {
            let w = (v >> shift) - prev;
            prev = v >> shift;
            (j, w)
        })
        .collect()
}
fn channel(p: u32, c: usize) -> i64 {
    i64::from(p >> (8 * c) & 255)
}
fn pack(c: [i64; 4]) -> u32 {
    (0..4).fold(0, |o, k| o | (c[k].clamp(0, 255) as u32) << (8 * k))
}
/// B, G, R at most A
fn capped(mut c: [i64; 4]) -> [i64; 4] {
    for k in 0..3 {
        c[k] = c[k].min(c[3]);
    }
    c
}
fn sums(line: &[(u32, i64)]) -> [i64; 4] {
    let mut s = [0; 4];
    for (k, v) in s.iter_mut().enumerate() {
        *v = line.iter().map(|&(p, w)| channel(p, k) * w).sum();
    }
    s
}
/// an MMX sum in 1/16384: rounded, saturated to 16 bits then to a byte
fn mmx(s: [i64; 4]) -> [i64; 4] {
    s.map(|v| ((v + 0x2000) >> 14).clamp(-32768, 32767).clamp(0, 255))
}

/// the device rectangle and the source pixels of one draw
pub struct Draw {
    pub dst: (i64, i64, i64, i64),
    pub src: (i64, i64, i64, i64),
    pub half: bool,
}

/// the pixels a draw writes: (x, y, premultiplied color) on a bitmap of bw x bh, the source
/// iw x ih read through texel (premultiplied, None past it); Ok(None) where not modelled
pub fn stretch<F>(r: Round, k: Kernel, d: &Draw, (iw, ih): (i64, i64), (bw, bh): (i64, i64), x86: bool, texel: F) -> Res<Option<Vec<(i64, i64, u32)>>>
where
    F: Fn(i64, i64) -> Res<Option<u32>>,
{
    // (x64 with a half pixel offset reducing: a place one 1/65536 off in a few cases, which op
    // rounds it not known yet)
    if !x86 && d.half && (d.src.2 > d.dst.2 || d.src.3 > d.dst.3) {
        return Ok(None);
    }
    hit(match k {
        Kernel::Tent => clause::GP_DRAW_HQ_BILINEAR,
        Kernel::Cubic => clause::GP_DRAW_HQ_BICUBIC,
    });
    let f = |v: i64| v as f32;
    let (dx, dy, dw, dh) = (f(d.dst.0), f(d.dst.1), f(d.dst.2), f(d.dst.3));
    let (sx, sy, sw, sh) = (f(d.src.0), f(d.src.1), f(d.src.2), f(d.src.3));
    // (GpGraphics::DrawImage: the source to world matrix, the source corners through it)
    let (xr, yb, sxr, syb) = (r.add(dx, dw), r.add(dy, dh), r.add(sw, sx), r.add(sh, sy));
    let m11 = r.div(r.sub(xr, dx), r.sub(sxr, sx));
    let m22 = r.div(r.sub(yb, dy), r.sub(syb, sy));
    let world: Matrix = [m11, 0.0, 0.0, m22, r.sub(xr, r.mul(m11, sxr)), r.sub(yb, r.mul(m22, syb))];
    let pts = [apply(r, &world, sx, sy), apply(r, &world, sxr, sy), apply(r, &world, sx, syb)];
    // (the world to device matrix: the half pixel offset)
    let o = if d.half { -0.5 } else { 0.0 };
    // (PipeLockBits: rows only, a margin by the y ratio)
    let ratio = r.div(r.sub(apply(r, &world, sx, syb).1, pts[0].1).abs(), sh);
    let m = k.margin(r, ratio).ceil() as i64;
    let top = (d.src.1 - m).max(0);
    let bottom = (r.add(sh, sy).ceil() as i64 + m).min(ih);
    if top > ih || bottom < 0 {
        return Ok(Some(Vec::new()));
    }
    let lh = bottom - top;
    let ly = r.sub(sy, f(top));
    let (ux, uy) = if d.half { (r.sub(sx, 0.5), r.sub(ly, 0.5)) } else { (sx, ly) };
    // (DpDriver::DrawImage: the source to points, through the device matrix, its corners)
    let Some(a) = infer(r, pts, (ux, uy, sw, sh)) else { return Ok(None) };
    let mm: Matrix = [
        r.add(r.mul(1.0, a[0]), r.mul(0.0, a[1])),
        r.add(r.mul(0.0, a[0]), r.mul(1.0, a[1])),
        r.add(r.mul(1.0, a[2]), r.mul(0.0, a[3])),
        r.add(r.mul(0.0, a[2]), r.mul(1.0, a[3])),
        r.add(r.add(r.mul(1.0, a[4]), r.mul(0.0, a[5])), o),
        r.add(r.add(r.mul(0.0, a[4]), r.mul(1.0, a[5])), o),
    ];
    let (x0, y0) = apply(r, &mm, ux, uy);
    let (x2, y2) = apply(r, &mm, r.add(ux, sw), r.add(uy, sh));
    let (ddx, ddy, ddw, ddh) = (x0, y0, r.sub(x2, x0), r.sub(y2, y0));
    // (InitializeClass)
    let xscale = fl(r, r.mul(r.div(sw, ddw), 65536.0));
    let yscale = fl(r, r.mul(r.div(sh, ddh), 65536.0));
    if xscale < 0 || yscale < 0 {
        return Ok(None);
    }
    let (rx, ry) = (r.div(ddw, sw), r.div(ddh, sh));
    let edges = |d0: f32, s: f32, ratio: f32| {
        let (a, b) = (fl(r, r.mul(d0, 16.0)), fl(r, r.mul(r.add(r.mul(s, ratio), d0), 16.0)));
        (a.min(b), a.max(b))
    };
    let (left, right) = edges(ddx, sw, rx);
    let (tp, bt) = edges(ddy, sh, ry);
    let (l16, r16, t16) = (left << 12, right << 12, tp << 12);
    let firstcol = (left + 15) >> 4;
    let sx0 = fl(r, r.mul(r.add(r.mul(r.mul(r.sub(ddx, f(l16 >> 16)), xscale as f32), 1.0 / 65536.0), ux), 65536.0));
    let sy0 = fl(r, r.mul(r.add(r.mul(r.mul(r.sub(f(t16 >> 16), ddy), yscale as f32), 1.0 / 65536.0), uy), 65536.0));
    let (kmul, kmin) = k.width();
    let half = |s: f32, d: f32| fl(r, r.mul(r.div(r.mul(kmul, s), d), 65536.0)).abs().max(kmin);
    let (kx, ky) = (half(sw, ddw), half(sh, ddh));
    let inv = |h: i64| fl(r, r.mul(r.div(65536.0, h as f32), 65536.0));
    let (kxi, kyi) = (inv(kx), inv(ky));
    let row0 = t16 >> 16;
    let width = ((r16 + 0xffff) >> 16) - (l16 >> 16) + 1;
    // (the device pixels: the 28.4 edges ceiled)
    let (cols, rows) = (firstcol..(right + 15) >> 4, (tp + 15) >> 4..(bt + 15) >> 4);
    // (ApplyVisibleClipToSourceRect clips the source when the bitmap cuts the rows: not asked)
    if rows.start < 0 || rows.end > bh {
        return Ok(None);
    }
    // (x86 mode 7: the MMX columns, all taps inside)
    let c0 = sx0 - 0x8000;
    let mmx_cols = match (x86, k) {
        (true, Kernel::Cubic) => {
            let lo = (kx - c0 + xscale - 1) / xscale;
            let hi = ((((iw - 1) << 16) - c0 - kx) / xscale).min(width);
            // (StretchMiddleScanline2_MMX, a 2x reduction of 16 columns or more: no case asks)
            let n = hi - lo.max(0);
            if n >= 16 && (xscale - 0x20000).abs() * n < 4 << 16 {
                return Ok(None);
            }
            lo.max(0)..hi
        }
        (true, Kernel::Tent) | (false, _) => 0..0,
    };
    // (the horizontal pass, one cached line a locked row)
    let mut lines = Vec::with_capacity(lh as usize);
    for j in 0..lh {
        let mut line = Vec::with_capacity(width as usize);
        for i in 0..width {
            let c = c0 + i * xscale;
            let t = taps(k, c, kx, kxi);
            let px = |p: &(i64, i64)| -> Res<u32> { Ok(if (0..iw).contains(&p.0) { texel(p.0, top + j)?.unwrap_or(0) } else { 0 }) };
            let mut v = Vec::with_capacity(t.len());
            let ws = weights(&t, if mmx_cols.contains(&i) { 2 } else { 0 });
            for p in &ws {
                v.push((px(p)?, p.1));
            }
            let s = sums(&v);
            line.push(match k {
                Kernel::Tent => pack(s.map(|v| ((v + 0x8000) >> 16).min(255))),
                Kernel::Cubic if mmx_cols.contains(&i) => pack(capped(mmx(s))),
                Kernel::Cubic => {
                    let v = s.map(|v| (v + 0x8000) >> 16);
                    let a = v[3].min(255);
                    pack(capped([v[0], v[1], v[2], a]).map(|c| c.max(0)))
                }
            });
        }
        lines.push(line);
    }
    // (the vertical pass)
    let mut out = Vec::new();
    for y in rows {
        let c = sy0 - 0x8000 + (y - row0) * yscale;
        let t = taps(k, c, ky, kyi);
        let ws: Vec<(i64, i64)> = weights(&t, 0).into_iter().filter(|&(j, _)| (0..lh).contains(&j)).collect();
        for x in cols.clone() {
            if !(0..bw).contains(&x) {
                continue;
            }
            let i = x - firstcol;
            let Some(i) = usize::try_from(i).ok().filter(|&i| i < width as usize) else { return Ok(None) };
            let s = if x86 {
                sums(&ws.iter().map(|&(j, w)| (lines[j as usize][i], w >> 2)).collect::<Vec<_>>())
            } else {
                sums(&ws.iter().map(|&(j, w)| (lines[j as usize][i], w)).collect::<Vec<_>>())
            };
            let p = match (x86, k) {
                (true, _) => pack(capped(mmx(s))),
                (false, Kernel::Tent) => pack(s.map(|v| ((v + 0x8000) >> 16).min(255))),
                (false, Kernel::Cubic) => pack(capped(s.map(|v| ((v + 0x8000) >> 16).clamp(0, 255)))),
            };
            out.push((x, y, p));
        }
    }
    Ok(Some(out))
}
