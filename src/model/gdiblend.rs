//! The model of runner/gdiblend.c's ops (spec/gdiblend.txt): AlphaBlend, TransparentBlt,
//! GradientFill, MaskBlt, PlgBlt.
//!
//! Sources (Microsoft Learn): "AlphaBlend", "BLENDFUNCTION" (the blend formulas),
//! "TransparentBlt", "GradientFill", "TRIVERTEX", "GRADIENT_RECT", "MaskBlt", "MAKEROP4",
//! "PlgBlt". The rounding of the formulas is what Windows' answers showed (golden blend).

use super::gdi::{rop3, Brush, Gdi, Slot};
use super::{Fail, Res};
use crate::case::{color, num, Op};
use crate::spec::hit;

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn n32(op: &Op, i: usize) -> Res<i32> {
    n(op, i).map(|v| v as i32)
}
fn slot(g: &Gdi, op: &Op, i: usize) -> Res<Slot> {
    g.slots.get(n(op, i)? as usize).and_then(|s| s.clone()).ok_or(Fail::Bad)
}

/// s * a / 255 + d * (255 - a) / 255 for one 8 bit channel
pub fn mix(s: u32, d: u32, a: u32) -> u32 {
    hit("GDI.ALPHA.ROUND");
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
        hit("GDI.ALPHA.CONSTANT");
        for i in 0..4 {
            r |= mix(ch(s, i), ch(d, i), sca) << (8 * i);
        }
        return r;
    }
    hit("GDI.ALPHA.PERPIXEL");
    let sa = ch(s, 3);
    if sa == 255 {
        // MEASURED: an opaque source pixel blends as with the constant alpha alone
        hit("GDI.ALPHA.OPAQUE");
        return alpha(s, d, sca, false);
    }
    // the source scaled by the constant alpha, then source over destination; the channels are
    // added as one 32 bit word (MEASURED: a channel over 255 carries into the next one)
    hit("GDI.ALPHA.CARRY");
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
    if !matches!(t0, "alphablend" | "transparent" | "gradrect" | "gradtri" | "maskblt" | "plgblt") {
        return Ok(false);
    }
    let k = n(op, 1)? as usize;
    let d = slot(g, op, 1)?;
    if d.ext.mapped() {
        return Err(Fail::Unmodeled("GDI.MAP.DRAW"));
    }
    match t0 {
        "alphablend" | "transparent" => {
            let (x, y, w, h) = (n32(op, 2)?, n32(op, 3)?, n32(op, 4)?, n32(op, 5)?);
            let s = slot(g, op, 6)?;
            let (sx, sy, sw, sh) = (n32(op, 7)?, n32(op, 8)?, n32(op, 9)?, n32(op, 10)?);
            let (sca, pp, key) = if t0 == "alphablend" { (n(op, 11)? as u32, n(op, 12)? != 0, 0) } else { (0, false, color(op.tok.get(11).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?) };
            if w <= 0 || h <= 0 || sw <= 0 || sh <= 0 {
                return Err(Fail::Unmodeled("GDI.ALPHA.EXTENTS"));
            }
            if t0 == "alphablend" {
                hit("GDI.ALPHA.SRCRECT");
                if sx < 0 || sy < 0 || sx + sw > s.s.w || sy + sh > s.s.h {
                    out.push("ret 0".into());
                    return Ok(true);
                }
                if pp && s.s.bpp != 32 {
                    return Err(Fail::Unmodeled("GDI.ALPHA.PPFORMAT"));
                }
            } else if sx < 0 || sy < 0 || sx + sw > s.s.w || sy + sh > s.s.h {
                return Err(Fail::Unmodeled("GDI.TRANS.SRCOUT"));
            }
            if w != sw || h != sh {
                return Err(Fail::Unmodeled(if t0 == "alphablend" { "GDI.ALPHA.STRETCH" } else { "GDI.TRANS.STRETCH" }));
            }
            if s.s.bpp <= 8 || (s.s.bpp == 1) != (d.s.bpp == 1) {
                return Err(Fail::Unmodeled(if t0 == "alphablend" { "GDI.ALPHA.FORMATS" } else { "GDI.TRANS.FORMATS" }));
            }
            let dsl = g.slots[k].as_mut().unwrap();
            for j in 0..h {
                for i in 0..w {
                    let (px, py) = (x + i, y + j);
                    if !dsl.visible(px, py) {
                        continue;
                    }
                    let sv = s.s.get(sx + i, sy + j);
                    let dv = dsl.s.get(px, py);
                    let nv = if t0 == "transparent" {
                        hit("GDI.TRANS.KEY");
                        if s.s.to_rgb(sv) == key {
                            continue;
                        }
                        // MEASURED: through the color, even between equal formats (32 bpp: x = 0)
                        hit("GDI.TRANS.COPY");
                        dsl.s.from_rgb(s.s.to_rgb(sv))
                    } else {
                        // (other formats: as 32 bpp colors, alpha 0)
                        let s32 = if s.s.bpp == 32 { sv } else { s.s.to_rgb(sv) };
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
                return Err(Fail::Unmodeled("GDI.MASK.FORMATS"));
            }
            let (fore, back) = ((rop >> 16 & 255) as u8, (rop >> 24) as u8);
            let Some(m) = m else {
                hit("GDI.MASK.NONE");
                let r = g.blt(k, x, y, w, h, Some((n(op, 6)? as usize, sx, sy)), rop & 0xffffff, false)?;
                out.push(format!("ret {}", r as u8));
                return Ok(true);
            };
            if w <= 0 || h <= 0 || mx < 0 || my < 0 || mx + w > m.s.w || my + h > m.s.h || sx < 0 || sy < 0 || sx + w > s.s.w || sy + h > s.s.h {
                return Err(Fail::Unmodeled("GDI.MASK.OUTSIDE"));
            }
            hit("GDI.MASK.ROP4");
            let pv = match &d.brush {
                Brush::Solid(c) => d.s.from_rgb(*c),
                _ => return Err(Fail::Unmodeled("GDI.SHAPE.BRUSH")),
            };
            let dsl = g.slots[k].as_mut().unwrap();
            let mask = dsl.s.mask();
            for j in 0..h {
                for i in 0..w {
                    let (px, py) = (x + i, y + j);
                    if !dsl.visible(px, py) {
                        continue;
                    }
                    let idx = if m.s.get(mx + i, my + j) == 1 { fore } else { back };
                    let dv = dsl.s.get(px, py);
                    dsl.s.set(px, py, rop3(idx, pv, s.s.get(sx + i, sy + j), dv) & mask);
                }
            }
            out.push("ret 1".into());
        }
        "gradrect" => return Err(Fail::Unmodeled("GDI.GRAD.RECT")),
        "gradtri" => return Err(Fail::Unmodeled("GDI.GRAD.TRIANGLE")),
        _ => return Err(Fail::Unmodeled("GDI.PLG.MAP")),
    }
    Ok(true)
}
