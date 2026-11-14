//! The model of runner/gdiddb.c's ops (spec/gdiddb.txt): device-dependent bitmaps
//! (CreateBitmap, CreateCompatibleBitmap), GetObject, GetBitmapBits, GetDeviceCaps of memory
//! DCs, GetObjectType.
//!
//! Sources (Microsoft Learn): "CreateBitmap", "CreateCompatibleBitmap", "CreateCompatibleDC",
//! "GetObject", "BITMAP", "DIBSECTION", "GetBitmapBits", "GetDeviceCaps", "GetObjectType".
//! Windows' runner has a 32 bpp display; what a memory DC reports of it is MEASURED.

use super::gdi::{Gdi, Slot, Surf};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// the display's bits per pixel (Windows' runner: 32)
pub const SCREEN_BPP: u32 = 32;
/// the display of Windows' runner (windows-2025): 1024 x 768 at 96 dpi
pub const SCREEN: (i32, i32) = (1024, 768);

/// GetDeviceCaps of a memory DC, any bitmap selected: the display's (MEASURED)
fn devcap(i: i64) -> Res<i64> {
    Ok(match i {
        2 => 1,                   // TECHNOLOGY: DT_RASDISPLAY
        12 => SCREEN_BPP as i64,  // BITSPIXEL
        14 => 1,                  // PLANES
        16 | 18 | 24 => -1,       // NUMBRUSHES, NUMPENS, NUMCOLORS (more than 8 bpp)
        104 => 0,                 // SIZEPALETTE
        106 => 20,                // NUMRESERVED
        108 => 24,                // COLORRES
        // MEASURED (ddb golden, every case of both executors): RASTERCAPS RC_BITBLT | RC_BITMAP64 |
        // RC_GDI20_OUTPUT | RC_DI_BITMAP | RC_DIBTODEV | RC_BIGFONT | RC_OP_DX_OUTPUT |
        // RC_STRETCHBLT | RC_FLOODFILL | RC_STRETCHDIB (0x7e99), SHADEBLENDCAPS none
        38 => {
            hit("GDI.DDB.CAPS.RASTER");
            0x7e99
        }
        120 => {
            hit("GDI.DDB.CAPS.RASTER");
            0
        }
        _ => return Err(Fail::Unmodeled("GDI.DDB.CAPS.OTHER")),
    })
}


pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    match t0 {
        "ddb" => {
            let k = n(op, 1)?;
            if !(0..8).contains(&k) {
                return Err(Fail::Bad);
            }
            let kind = op.tok.get(2).ok_or(Fail::Bad)?.clone();
            let s = match kind.as_str() {
                "mono" => {
                    let (w, h) = (n(op, 3)?, n(op, 4)?);
                    hit("GDI.DDB.MONO");
                    if w <= 0 || h <= 0 {
                        return Err(Fail::Unmodeled("GDI.DDB.ZERO"));
                    }
                    Surf::ddb(w as i32, h as i32, 1)
                }
                "bitmap" => {
                    let (w, h, p, b) = (n(op, 3)?, n(op, 4)?, n(op, 5)?, n(op, 6)?);
                    if w <= 0 || h <= 0 || p != 1 {
                        return Err(Fail::Unmodeled("GDI.DDB.PLANES"));
                    }
                    hit("GDI.DDB.SELECT");
                    if b != 1 && b != SCREEN_BPP as i64 {
                        // (a memory DC takes a monochrome bitmap or one of the display's format)
                        out.push("select failed".into());
                        g.slots[k as usize] = None;
                        return Ok(true);
                    }
                    Surf::ddb(w as i32, h as i32, b as u32)
                }
                "screen" | "compat" => {
                    let of = if kind == "compat" { Some(g.slots.get(n(op, 3)? as usize).and_then(|s| s.as_ref()).ok_or(Fail::Bad)?.s.clone()) } else { None };
                    let at = if kind == "compat" { 4 } else { 3 };
                    let (w, h) = (n(op, at)?, n(op, at + 1)?);
                    if w <= 0 || h <= 0 {
                        return Err(Fail::Unmodeled("GDI.DDB.ZERO"));
                    }
                    match of {
                        None => {
                            hit("GDI.DDB.SCREEN");
                            Surf::ddb(w as i32, h as i32, SCREEN_BPP)
                        }
                        Some(o) if o.ddb => {
                            hit("GDI.DDB.COMPAT");
                            Surf::ddb(w as i32, h as i32, o.bpp)
                        }
                        Some(_) => return Err(Fail::Unmodeled("GDI.DDB.COMPATDIB")),
                    }
                }
                _ => return Err(Fail::Bad),
            };
            g.slots[k as usize] = Some(Slot::new(s));
        }
        "bmobject" => {
            let s = &g.slots.get(n(op, 1)? as usize).and_then(|s| s.as_ref()).ok_or(Fail::Bad)?.s;
            if s.ddb {
                hit("GDI.DDB.OBJECT");
                out.push(format!("bm bitmap {} {} {} 1 {} 0", s.w, s.h, s.stride, s.bpp));
                out.push("bmsize bitmap".into());
            } else {
                hit("GDI.DDB.DIBOBJECT");
                out.push(format!("bm dibsection {} {} {} 1 {} 1", s.w, s.h, s.stride, s.bpp));
                let used = if s.bpp <= 8 { 1 << s.bpp } else { 0 };
                out.push(format!("dib {} {} {} {used}", s.bpp, if s.topdown { -s.h } else { s.h }, if s.f565 { 3 } else { 0 }));
                out.push("bmsize dibsection".into());
            }
        }
        "bmbits" => {
            let sl = g.slots.get(n(op, 1)? as usize).and_then(|s| s.as_ref()).ok_or(Fail::Bad)?;
            if !sl.s.ddb {
                return Err(Fail::Unmodeled("GDI.DDB.DIBBITS"));
            }
            hit("GDI.DDB.BITS");
            out.push(format!("bmbits {}", sl.s.stride * sl.s.h as usize));
            out.push(sl.s.rows.iter().flat_map(|r| r.iter().map(|b| format!("{b:02x}"))).collect());
        }
        "devcaps" => {
            g.slots.get(n(op, 1)? as usize).and_then(|s| s.as_ref()).ok_or(Fail::Bad)?;
            hit("GDI.DDB.CAPS");
            let mut l = String::from("caps");
            for i in 2..op.tok.len() {
                l += &format!(" {}", devcap(n(op, i)?)?);
            }
            out.push(l);
        }
        "objtype" => {
            g.slots.get(n(op, 1)? as usize).and_then(|s| s.as_ref()).ok_or(Fail::Bad)?;
            let t = match op.tok.get(2).map(String::as_str) {
                Some("pen") => 1,
                Some("brush") => 2,
                Some("font") => 6,
                Some("bitmap") => 7,
                _ => return Err(Fail::Bad),
            };
            hit("GDI.DDB.OBJTYPE");
            out.push(format!("objtype {t}"));
        }
        _ => return Ok(false),
    }
    Ok(true)
}
