//! The model of runner/gdiblend.c's ops (spec/gdiblend.txt): AlphaBlend, TransparentBlt,
//! GradientFill, MaskBlt, PlgBlt.
//!
//! Sources (Microsoft Learn): "AlphaBlend", "BLENDFUNCTION" (the blend formulas),
//! "TransparentBlt", "GradientFill", "TRIVERTEX", "GRADIENT_RECT", "MaskBlt", "MAKEROP4",
//! "PlgBlt". The rounding of the formulas is what Windows' answers showed (golden blend).

use crate::spec::open;
use crate::spec::clause;
use super::gdi::{rop3, Brush, Gdi, Slot};
use super::{filled_mut, Fail, Res};
use crate::case::{color, num, Op};
use crate::spec::hit;
use super::gdiinvcmap::{Cell, Table};

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn n32(op: &Op, i: usize) -> Res<i32> {
    n(op, i).map(|v| v as i32)
}
fn slot(g: &Gdi, op: &Op, i: usize) -> Res<Slot> {
    super::gdi::get(&g.slots, n(op, i)?).cloned()
}

/// s * a / 255 + d * (255 - a) / 255 for one 8 bit channel
pub fn mix(s: u32, d: u32, a: u32) -> u32 {
    hit(clause::GDI_ALPHA_ROUND);
    (s * a + d * (255 - a) + 127) / 255
}

/// 0xAARRGGBB channels of a 32 bpp value
fn ch(v: u32, i: u32) -> u32 {
    v >> (8 * i) & 255
}

/// AlphaBlend of one pixel: source and destination as 32 bpp values (BGRA)
pub fn alpha(s: u32, d: u32, sca: u32, per_pixel: bool) -> u32 {
    if !per_pixel {
        let mut r = 0;
        hit(clause::GDI_ALPHA_CONSTANT);
        for i in 0..4 {
            r |= mix(ch(s, i), ch(d, i), sca) << (8 * i);
        }
        return r;
    }
    hit(clause::GDI_ALPHA_PERPIXEL);
    let sa = ch(s, 3);
    if sa == 255 {
        // MEASURED: an opaque source pixel blends as with the constant alpha alone
        hit(clause::GDI_ALPHA_OPAQUE);
        return alpha(s, d, sca, false);
    }
    // the source scaled by the constant alpha, then source over destination; the channels are
    // added as one 32 bit word (MEASURED: a channel over 255 carries into the next one)
    hit(clause::GDI_ALPHA_CARRY);
    let sa = (sa * sca + 127) / 255;
    let mut r = 0u32;
    for i in 0..4 {
        let sc = (ch(s, i) * sca + 127) / 255;
        r = r.wrapping_add((sc + (ch(d, i) * (255 - sa) + 127) / 255) << (8 * i));
    }
    r
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if !matches!(t0, "alphablend" | "transparent" | "gradrect" | "gradtri" | "maskblt" | "plgblt" | "halftone") {
        return Ok(false);
    }
    let k = n(op, 1)? as usize;
    let d = slot(g, op, 1)?;
    if d.ext.mapped() {
        return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
    }
    match t0 {
        "alphablend" | "transparent" => {
            let (x, y, w, h) = (n32(op, 2)?, n32(op, 3)?, n32(op, 4)?, n32(op, 5)?);
            let s = slot(g, op, 6)?;
            let (sx, sy, sw, sh) = (n32(op, 7)?, n32(op, 8)?, n32(op, 9)?, n32(op, 10)?);
            let (sca, pp, key) = if t0 == "alphablend" { (n(op, 11)? as u32, n(op, 12)? != 0, 0) } else { (0, false, color(op.tok.get(11).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?) };
            if w <= 0 || h <= 0 || sw <= 0 || sh <= 0 {
                if t0 == "transparent" {
                    // MEASURED: any extent of 0 or less fails, drawing nothing
                    hit(clause::GDI_TRANS_EXTENTS);
                    out.push("ret 0".into());
                    return Ok(true);
                }
                // MEASURED: a negative source extent fails; then any extent of 0 succeeds
                // drawing nothing; then a negative destination extent fails
                hit(clause::GDI_ALPHA_EXTENTS);
                let ok = sw >= 0 && sh >= 0 && (w == 0 || h == 0 || sw == 0 || sh == 0);
                out.push(format!("ret {}", ok as u8));
                return Ok(true);
            }
            if t0 == "alphablend" {
                hit(clause::GDI_ALPHA_SRCRECT);
                if sx < 0 || sy < 0 || sx + sw > s.s.w || sy + sh > s.s.h {
                    out.push("ret 0".into());
                    return Ok(true);
                }
                if pp && s.s.bpp != 32 {
                    // MEASURED: AC_SRC_ALPHA needs a 32 bpp source
                    hit(clause::GDI_ALPHA_PPFORMAT);
                    out.push("ret 0".into());
                    return Ok(true);
                }
            } else if sx < 0 || sy < 0 || sx + sw > s.s.w || sy + sh > s.s.h {
                // MEASURED: a source rectangle not wholly in the source fails, drawing nothing
                hit(clause::GDI_TRANS_SRCOUT);
                out.push("ret 0".into());
                return Ok(true);
            }
            let at = |i: i32, sn: i32, n: i32, flat: bool| -> i32 {
                if sn == n {
                    return i;
                }
                // MEASURED: the pixel StretchBlt's COLORONCOLOR takes
                hit(if t0 == "alphablend" { clause::GDI_ALPHA_STRETCH } else { clause::GDI_TRANS_STRETCH });
                super::gdi::sample(i, sn, n, flat)
            };
            if s.s.bpp <= 8 || (s.s.bpp == 1) != (d.s.bpp == 1) {
                // MEASURED: sources with a color table (monochrome too) go through their colors
                hit(if t0 == "alphablend" { clause::GDI_ALPHA_FORMATS } else { clause::GDI_TRANS_FORMATS });
            }
            let dsl = filled_mut(&mut g.slots, k)?;
            for j in 0..h {
                for i in 0..w {
                    let (px, py) = (x + i, y + j);
                    if !dsl.visible(px, py) {
                        continue;
                    }
                    let sv = s.s.get(sx + at(i, sw, w, sh == h), sy + at(j, sh, h, sw == w));
                    let dv = dsl.s.get(px, py);
                    let nv = if t0 == "transparent" {
                        hit(clause::GDI_TRANS_KEY);
                        if s.s.to_rgb(sv) == key {
                            continue;
                        }
                        // MEASURED: through the color, even between equal formats (32 bpp: x = 0)
                        hit(clause::GDI_TRANS_COPY);
                        dsl.s.from_rgb(s.s.to_rgb(sv))
                    } else {
                        // (other formats: as 32 bpp colors, alpha 0)
                        // MEASURED (alpha-palette-*, alpha-stretch-*): a source of another
                        // format blends with alpha 255 when it has a color table or is
                        // stretched, 0 otherwise and at a constant alpha of 255
                        let opaque = (s.s.bpp <= 8 || w != sw || h != sh) && sca < 255;
                        let s32 = match s.s.bpp {
                            32 => sv,
                            _ => s.s.to_rgb(sv) | if opaque { 0xff00_0000 } else { 0 },
                        };
                        let d32 = if dsl.s.bpp == 32 { dv } else { dsl.s.to_rgb(dv) };
                        let r = alpha(s32, d32, sca, pp);
                        if dsl.s.bpp == 32 { r } else { dsl.s.from_rgb(r & 0xffffff) }
                    };
                    dsl.s.set(px, py, nv);
                }
            }
            out.push("ret 1".into());
        }
        "maskblt" => {
            let (x, y, w, h) = (n32(op, 2)?, n32(op, 3)?, n32(op, 4)?, n32(op, 5)?);
            let s = slot(g, op, 6)?;
            let (sx, sy) = (n32(op, 7)?, n32(op, 8)?);
            let m = if op.tok.get(9).is_some_and(|t| t == "none") { None } else { Some(slot(g, op, 9)?) };
            let (mx, my, rop) = (n32(op, 10)?, n32(op, 11)?, n(op, 12)? as u32);
            if !s.s.same_format(&d.s) {
                return Err(Fail::Unmodeled(open::GDI_MASK_FORMATS));
            }
            let (fore, back) = ((rop >> 16 & 255) as u8, (rop >> 24) as u8);
            let Some(m) = m else {
                hit(clause::GDI_MASK_NONE);
                let r = g.blt(k, x, y, w, h, Some((n(op, 6)? as usize, sx, sy)), rop & 0xffffff, false)?;
                out.push(format!("ret {}", r as u8));
                return Ok(true);
            };
            let (mut x, mut y, mut w, mut h, mut sx, mut sy, mut mx, mut my) = (x, y, w, h, sx, sy, mx, my);
            if w < 0 || h < 0 {
                // MEASURED: a negative width or height moves the destination, source and mask
                // origins back by it and draws unmirrored
                hit(clause::GDI_MASK_NEGATIVE);
                if w < 0 {
                    (x, sx, mx, w) = (x + w, sx + w, mx + w, -w);
                }
                if h < 0 {
                    (y, sy, my, h) = (y + h, sy + h, my + h, -h);
                }
            }
            if w == 0 || h == 0 {
                // MEASURED: nothing to draw, and the call succeeds
                hit(clause::GDI_MASK_EMPTY);
                out.push("ret 1".into());
                return Ok(true);
            }
            // MEASURED: nothing lands where the destination and the source overlap: the call
            // succeeds; otherwise the mask must hold the whole rectangle
            hit(clause::GDI_MASK_OUTSIDE);
            let lands = |p: i32, sp: i32, e: i32, de: i32, se: i32| (-p).max(-sp).max(0) < e.min(de - p).min(se - sp);
            if lands(x, sx, w, d.s.w, s.s.w) && lands(y, sy, h, d.s.h, s.s.h) && (mx < 0 || my < 0 || mx + w > m.s.w || my + h > m.s.h) {
                out.push("ret 0".into());
                return Ok(true);
            }
            hit(clause::GDI_MASK_ROP4);
            let pv = match &d.brush {
                Brush::Solid(c) => d.s.from_rgb(*c),
                _ => return Err(Fail::Unmodeled(open::GDI_SHAPE_BRUSH)),
            };
            let dsl = filled_mut(&mut g.slots, k)?;
            let mask = dsl.s.mask();
            for j in 0..h {
                for i in 0..w {
                    let (px, py) = (x + i, y + j);
                    if !dsl.visible(px, py) {
                        continue;
                    }
                    if sx + i < 0 || sy + j < 0 || sx + i >= s.s.w || sy + j >= s.s.h || mx + i >= m.s.w || my + j >= m.s.h {
                        continue;
                    }
                    let idx = if m.s.get(mx + i, my + j) == 1 { fore } else { back };
                    let dv = dsl.s.get(px, py);
                    dsl.s.set(px, py, rop3(idx, pv, s.s.get(sx + i, sy + j), dv) & mask);
                }
            }
            out.push("ret 1".into());
        }
        "halftone" => {
            hit(clause::GDI_GRAD_HALFTONE);
            filled_mut(&mut g.slots, k)?.ext.palette = super::gdiline::Palette::Halftone;
            out.push("halftone 1".into());
        }
        "gradrect" | "gradtri" => {
            let (mode, from, nv) = match (t0, n(op, 2)?) {
                ("gradtri", _) => (Grad::Triangle, 2, 3),
                (_, 0) => (Grad::H, 3, 2),
                (_, 1) => (Grad::V, 3, 2),
                _ => return Err(Fail::Bad),
            };
            let mut v = [Vert::default(); 3];
            for (i, vi) in v.iter_mut().enumerate().take(nv) {
                let k = from + 6 * i;
                let c16 = |j: usize| -> Res<i64> { n(op, k + j).map(|c| c & 0xffff) };
                *vi = Vert { x: n32(op, k)?, y: n32(op, k + 1)?, c: [c16(2)?, c16(3)?, c16(4)?, c16(5)?] };
            }
            let dsl = filled_mut(&mut g.slots, k)?;
            let ok = gradient(dsl, mode, v)?;
            out.push(format!("ret {}", ok as u8));
        }
        "plgblt" => {
            let p: Vec<i64> = (2..8).map(|i| n(op, i)).collect::<Res<_>>()?;
            let s = slot(g, op, 8)?;
            let (sx, sy, w, h) = (n(op, 9)?, n(op, 10)?, n(op, 11)?, n(op, 12)?);
            if op.tok.len() > 13 || !s.s.same_format(&d.s) || w <= 0 || h <= 0 || sx < 0 || sy < 0 || sx + w > s.s.w as i64 || sy + h > s.s.h as i64 {
                return Err(Fail::Unmodeled(open::GDI_PLG_MAP));
            }
            let ((x0, y0), (ax, ay), (bx, by)) = ((p[0], p[1]), (p[2] - p[0], p[3] - p[1]), (p[4] - p[0], p[5] - p[1]));
            let dsl = filled_mut(&mut g.slots, k)?;
            let mut put = |x: i64, y: i64, u: i64, v: i64| {
                let (x, y) = (x as i32, y as i32);
                if dsl.visible(x, y) {
                    dsl.s.set(x, y, s.s.get((sx + u) as i32, (sy + v) as i32));
                }
            };
            if ay == 0 && bx == 0 && ax != 0 && by != 0 {
                // MEASURED: a parallelogram that is a rectangle draws as StretchBlt does, a
                // negative side running from the point it starts at back (that pixel included)
                hit(clause::GDI_PLG_RECT);
                match d.mode {
                    4 => return Err(Fail::Unmodeled(open::GDI_STRETCH_HALFTONE)),
                    3 => {}
                    _ if w > ax.abs() || h > by.abs() => return Err(Fail::Unmodeled(open::GDI_STRETCH_COMBINE)),
                    _ => {}
                }
                for oy in 0..by.abs() {
                    for ox in 0..ax.abs() {
                        let u = super::gdi::sample(ox as i32, w as i32, ax.abs() as i32, h == by.abs()) as i64;
                        let v = super::gdi::sample(oy as i32, h as i32, by.abs() as i32, w == ax.abs()) as i64;
                        put(x0 + ox * ax.signum(), y0 + oy * by.signum(), u, v);
                    }
                }
            } else if ax == 0 && by == 0 && ay.abs() == w && bx.abs() == h {
                // MEASURED: a quarter turn of the source's size: each destination pixel takes the
                // source pixel under its center
                hit(clause::GDI_PLG_TURN);
                let dd = ax * by - ay * bx;
                for y in (y0 - h - 1)..=(y0 + w + 1) {
                    for x in (x0 - h - 1)..=(x0 + w + 1) {
                        let (dx, dy) = (2 * (x - x0) + 1, 2 * (y - y0) + 1);
                        let u = (w * (dx * by - dy * bx)).div_euclid(2 * dd);
                        let v = (h * (ax * dy - ay * dx)).div_euclid(2 * dd);
                        if (0..w).contains(&u) && (0..h).contains(&v) {
                            put(x, y, u, v);
                        }
                    }
                }
            } else {
                return Err(Fail::Unmodeled(open::GDI_PLG_MAP));
            }
            out.push("ret 1".into());
        }
        _ => return Err(Fail::Bad),
    }
    Ok(true)
}

/// GradientFill's modes
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Grad {
    H,
    V,
    Triangle,
}

/// a TRIVERTEX in device coordinates: red, green, blue, alpha (16 bit)
#[derive(Clone, Copy, Default, Debug)]
pub struct Vert {
    pub x: i32,
    pub y: i32,
    pub c: [i64; 4],
}

/// MEASURED: the 4x4 ordered dither of 16 bpp gradients
const BAYER4: [[i64; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// how a gradient's channel value (a 16 bit fraction num / den, den > 0) becomes 8 bits
#[derive(Clone, Copy, PartialEq, Debug)]
enum Round {
    /// rectangles: the value truncated (MEASURED, GDI.GRAD.RECTH)
    Floor,
    /// triangles: red, green and blue vertex values above 0xff00 count as 0xff00, then the
    /// value rounds to 16 bits, halves up, then truncates (MEASURED, GDI.GRAD.TRIROUND)
    Nearest16,
}

/// a gradient channel as 8 bits
fn c8(num: i64, den: i64, r: Round) -> u32 {
    let v = match r {
        Round::Floor => num.div_euclid(den),
        Round::Nearest16 => (2 * num + den).div_euclid(2 * den),
    };
    (v >> 8) as u8 as u32
}

/// one pixel of a gradient whose channel `i` (red, green, blue, alpha) is `num(i) / den`
/// (16 bit), in `s`'s format; None where it is not known yet
fn grad_pixel(s: &super::gdi::Surf, inv: Option<(&Table, bool)>, num: impl Fn(usize) -> i64, den: i64, r: Round, x: i32, y: i32) -> Res<u32> {
    let (num, den) = (|i: usize| if den < 0 { -num(i) } else { num(i) }, den.abs());
    let c = |i: usize| c8(num(i), den, r);
    match (s.bpp, r) {
        (32, _) => Ok(c(3) << 24 | c(0) << 16 | c(1) << 8 | c(2)),
        (24, _) => Ok(c(0) << 16 | c(1) << 8 | c(2)),
        (16, _) => {
            hit(clause::GDI_GRAD_DITHER16);
            let d = BAYER4[y.rem_euclid(4) as usize][x.rem_euclid(4) as usize];
            // (the 16 bit value as the format's 8 bit channels take it, GDI.GRAD.RECTH or
            // GDI.GRAD.TRIROUND)
            let v16 = |i: usize| match r {
                Round::Floor => num(i) / den,
                Round::Nearest16 => (2 * num(i) + den).div_euclid(2 * den),
            };
            // (5 bits from 1/128 steps; 5-6-5's green, 6 bits from 1/64 steps)
            let cn = |i: usize, unit: i64, max: i64| ((v16(i) / unit + d) / 16).clamp(0, max) as u32;
            Ok(if s.f565 { cn(0, 128, 31) << 11 | cn(1, 64, 63) << 5 | cn(2, 128, 31) } else { cn(0, 128, 31) << 10 | cn(1, 128, 31) << 5 | cn(2, 128, 31) })
        }
        (1 | 4 | 8, _) => {
            hit(clause::GDI_GRAD_INDEXED);
            let (inv, halftone) = inv.ok_or(Fail::Bad)?;
            let six = s.bpp == 8 && halftone;
            let m = if six { &DITHER6 } else { &DITHER3 };
            let d = i64::from(m[y.rem_euclid(16) as usize][x.rem_euclid(16) as usize]);
            // (each channel to a 5 bit cell of GDI's inverse color table)
            let cell = |i: usize| -> u8 {
                let v = i64::from(c(i));
                if s.bpp == 1 {
                    if v + 2 * d >= 255 { 31 } else { 0 }
                } else if six {
                    // (6 levels: 0, 51, .. 204 by 51s, 255 from 255 on)
                    (if v + d >= 255 { 255 } else { 51 * ((v + d) / 51) } >> 3) as u8
                } else {
                    match v + d {
                        ..=127 => 0,
                        128..=254 => 16,
                        _ => 31,
                    }
                }
            };
            // (a cell GDI's table builder never writes: Windows reads what that memory held)
            let v = inv.get(Cell::of(cell(0), cell(1), cell(2))).ok_or(Fail::Unmodeled(open::GDI_GRAD_HOLE))?;
            Ok(u32::from(v))
        }
        _ => Err(Fail::Bad),
    }
}

/// the 16x16 ordered dither of 8 bpp gradients with a halftone palette selected (clean-room
/// note on win32kfull)
const DITHER6: [[u8; 16]; 16] = [
    [3, 28, 9, 35, 4, 30, 11, 36, 3, 29, 10, 35, 5, 30, 11, 37],
    [41, 16, 48, 22, 43, 17, 49, 24, 42, 16, 48, 22, 43, 18, 50, 24],
    [6, 32, 0, 25, 8, 33, 1, 27, 6, 32, 0, 26, 8, 34, 2, 27],
    [44, 19, 38, 12, 46, 20, 40, 14, 45, 19, 38, 13, 46, 21, 40, 14],
    [5, 31, 12, 37, 4, 29, 10, 36, 6, 31, 12, 38, 4, 30, 10, 36],
    [44, 18, 50, 24, 42, 16, 48, 23, 44, 18, 50, 25, 42, 17, 49, 23],
    [8, 34, 2, 28, 7, 32, 0, 26, 9, 34, 2, 28, 7, 33, 1, 26],
    [47, 21, 40, 15, 45, 20, 39, 13, 47, 22, 41, 15, 46, 20, 39, 14],
    [3, 29, 9, 35, 5, 30, 11, 37, 3, 28, 9, 35, 4, 30, 11, 36],
    [41, 16, 48, 22, 43, 17, 49, 24, 41, 15, 47, 22, 43, 17, 49, 23],
    [6, 32, 0, 25, 8, 33, 1, 27, 6, 31, 0, 25, 7, 33, 1, 27],
    [45, 19, 38, 13, 46, 21, 40, 14, 44, 19, 38, 12, 46, 20, 39, 14],
    [5, 31, 12, 37, 4, 29, 10, 36, 5, 31, 11, 37, 3, 29, 10, 35],
    [44, 18, 50, 25, 42, 17, 49, 23, 43, 18, 50, 24, 42, 16, 48, 23],
    [9, 34, 2, 28, 7, 33, 1, 26, 8, 34, 2, 27, 7, 32, 0, 26],
    [47, 21, 41, 15, 45, 20, 39, 13, 47, 21, 40, 15, 45, 19, 39, 13],
];

/// the 16x16 ordered dither of gradients into color tables (clean-room note on win32kfull,
/// copied as it is: 126 four times, no 127)
const DITHER3: [[u8; 16]; 16] = [
    [8, 72, 24, 88, 12, 76, 28, 92, 9, 73, 25, 89, 13, 77, 29, 93],
    [104, 40, 120, 56, 108, 44, 124, 60, 105, 41, 121, 57, 109, 45, 125, 61],
    [16, 80, 0, 64, 20, 84, 4, 68, 17, 81, 1, 65, 21, 85, 5, 69],
    [112, 48, 96, 32, 116, 52, 100, 36, 113, 49, 97, 33, 117, 53, 101, 37],
    [14, 78, 30, 94, 10, 74, 26, 90, 15, 79, 31, 95, 11, 75, 27, 91],
    [110, 46, 126, 62, 106, 42, 122, 58, 111, 47, 126, 63, 107, 43, 123, 59],
    [22, 86, 6, 70, 18, 82, 2, 66, 23, 87, 7, 71, 19, 83, 3, 67],
    [118, 54, 102, 38, 114, 50, 98, 34, 119, 55, 103, 39, 115, 51, 99, 35],
    [9, 73, 25, 89, 13, 77, 29, 93, 8, 72, 24, 88, 12, 76, 28, 92],
    [105, 41, 121, 57, 109, 45, 125, 61, 104, 40, 120, 56, 108, 44, 124, 60],
    [17, 81, 1, 65, 21, 85, 5, 69, 16, 80, 0, 64, 20, 84, 4, 68],
    [113, 49, 97, 33, 117, 53, 101, 37, 112, 48, 96, 32, 116, 52, 100, 36],
    [15, 79, 31, 95, 11, 75, 27, 91, 14, 78, 30, 94, 10, 74, 26, 90],
    [111, 47, 126, 63, 107, 43, 123, 59, 110, 46, 126, 62, 106, 42, 122, 58],
    [23, 87, 7, 71, 19, 83, 3, 67, 22, 86, 6, 70, 18, 82, 2, 66],
    [119, 55, 103, 39, 115, 51, 99, 35, 118, 54, 102, 38, 114, 50, 98, 34],
];


/// x of the edge (x1, y1)-(x2, y2) at row y, followed from its right end
fn edge(y: i32, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    if x2 > x1 {
        x2 + (y - y2) * (x2 - x1) / (y2 - y1)
    } else {
        x1 + (y - y1) * (x2 - x1) / (y2 - y1)
    }
}

/// GradientFill of one rectangle (v[0], v[1]) or triangle into `d`; false when it fails
pub fn gradient(d: &mut Slot, mode: Grad, mut v: [Vert; 3]) -> Res<bool> {
    // (GDI's inverse table of the bitmap's color table, GDI.GRAD.INDEXED)
    let inv = (d.s.bpp <= 8).then(|| super::gdiinvcmap::table(&d.s.pal));
    let halftone = d.ext.palette == super::gdiline::Palette::Halftone;
    let vis = |d: &Slot, x: i32, y: i32| d.visible(x, y);
    match mode {
        Grad::H | Grad::V => {
            hit(if mode == Grad::H { clause::GDI_GRAD_RECTH } else { clause::GDI_GRAD_RECTV });
            // MEASURED: the colors follow the vertices sorted along the gradient
            let (a, b) = if mode == Grad::H { (v[0].x, v[1].x) } else { (v[0].y, v[1].y) };
            if b < a {
                v.swap(0, 1);
            }
            let (l, r) = (v[0].x.min(v[1].x), v[0].x.max(v[1].x));
            let (t, btm) = (v[0].y.min(v[1].y), v[0].y.max(v[1].y));
            let (l, r) = if mode == Grad::H { (v[0].x, v[1].x) } else { (l, r) };
            let (t, btm) = if mode == Grad::V { (v[0].y, v[1].y) } else { (t, btm) };
            for y in t..btm {
                for x in l..r {
                    if !vis(d, x, y) {
                        continue;
                    }
                    let (pos, len) = if mode == Grad::H { ((x - v[0].x) as i64, (v[1].x - v[0].x) as i64) } else { ((y - v[0].y) as i64, (v[1].y - v[0].y) as i64) };
                    let p = grad_pixel(&d.s, inv.as_ref().map(|t| (t, halftone)), |i| v[0].c[i] * (len - pos) + v[1].c[i] * pos, len, Round::Floor, x, y)?;
                    d.s.set(x, y, p);
                }
            }
            Ok(true)
        }
        Grad::Triangle => {
            hit(clause::GDI_GRAD_TRIANGLE);
            v.sort_by_key(|p| p.y);
            hit(clause::GDI_GRAD_TRIROUND);
            for p in v.iter_mut() {
                for c in &mut p.c[..3] {
                    *c = (*c).min(0xff00);
                }
            }
            let det = ((v[2].y - v[1].y) * (v[2].x - v[0].x) - (v[2].x - v[1].x) * (v[2].y - v[0].y)) as i64;
            let (l, r) = (v[0].x.min(v[1].x).min(v[2].x), v[0].x.max(v[1].x).max(v[2].x));
            let (t, b) = (v[0].y.max(0), v[2].y.min(d.s.h));
            if l.max(0) >= r.min(d.s.w) || t >= b {
                return Ok(true);
            }
            if det == 0 {
                return Ok(false);
            }
            for y in t..b {
                let x1 = if y < v[1].y { edge(y, v[0].x, v[0].y, v[1].x, v[1].y) } else { edge(y, v[1].x, v[1].y, v[2].x, v[2].y) };
                let x2 = edge(y, v[0].x, v[0].y, v[2].x, v[2].y);
                for x in x1.min(x2).max(l.max(0))..x1.max(x2).min(r.min(d.s.w)) {
                    if !vis(d, x, y) {
                        continue;
                    }
                    let (px, py) = (x as i64, y as i64);
                    let l1 = (v[1].y - v[2].y) as i64 * (px - v[2].x as i64) - (v[1].x - v[2].x) as i64 * (py - v[2].y as i64);
                    let l2 = (v[2].y - v[0].y) as i64 * (px - v[2].x as i64) - (v[2].x - v[0].x) as i64 * (py - v[2].y as i64);
                    let p = grad_pixel(&d.s, inv.as_ref().map(|t| (t, halftone)), |i| v[0].c[i] * l1 + v[1].c[i] * l2 + v[2].c[i] * (det - l1 - l2), det, Round::Nearest16, x, y)?;
                    d.s.set(x, y, p);
                }
            }
            Ok(true)
        }
    }
}
