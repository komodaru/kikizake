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

/// MEASURED: a negative width or height counts as its magnitude
fn abs(op: &Op, i: usize) -> Res<(i64, i64)> {
    let (w, h) = (n(op, i)?, n(op, i + 1)?);
    if w < 0 || h < 0 {
        hit("GDI.DDB.NEGATIVE");
    }
    Ok((w.abs(), h.abs()))
}
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
                    let (w, h) = abs(op, 3)?;
                    hit("GDI.DDB.MONO");
                    if w == 0 || h == 0 {
                        hit("GDI.DDB.ZERO");
                        Surf::ddb(1, 1, 1)
                    } else {
                        Surf::ddb(w as i32, h as i32, 1)
                    }
                }
                "bitmap" => {
                    let ((w, h), p, b) = (abs(op, 3)?, n(op, 5)?, n(op, 6)?);
                    if p < 1 || b < 1 {
                        // MEASURED: no bitmap of 0 planes or 0 bits
                        hit("GDI.DDB.NOBITS");
                        out.push("ddb failed".into());
                        g.slots[k as usize] = None;
                        return Ok(true);
                    }
                    if p != 1 {
                        // MEASURED: the planes' bits make one plane of planes * bits
                        hit("GDI.DDB.PLANES");
                    }
                    let b = p * b;
                    if w == 0 || h == 0 {
                        hit("GDI.DDB.ZERO");
                        g.slots[k as usize] = Some(Slot::new(Surf::ddb(1, 1, 1)));
                        return Ok(true);
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
                    let of = if kind == "compat" { Some(super::gdi::get(&g.slots, n(op, 3)?)?.s.clone()) } else { None };
                    let at = if kind == "compat" { 4 } else { 3 };
                    let (w, h) = abs(op, at)?;
                    if w == 0 || h == 0 {
                        // MEASURED: "a 1-by-1 pixel, monochrome bitmap", whatever it is compatible with
                        hit("GDI.DDB.ZERO");
                        g.slots[k as usize] = Some(Slot::new(Surf::ddb(1, 1, 1)));
                        return Ok(true);
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
                        Some(o) => {
                            // MEASURED: a DIB section of the DC's bitmap's format (GetObject
                            // cannot tell its row order: memory taken as bottom-up, as the
                            // executor reads GetObject's positive height)
                            hit("GDI.DDB.COMPATDIB");
                            Surf::new(w as i32, h as i32, o.bpp, o.f565, false, o.pal.clone())
                        }
                    }
                }
                _ => return Err(Fail::Bad),
            };
            g.slots[k as usize] = Some(Slot::new(s));
        }
        "bmobject" => {
            let s = &super::gdi::get(&g.slots, n(op, 1)?)?.s;
            if s.ddb {
                hit("GDI.DDB.OBJECT");
                out.push(format!("bm bitmap {} {} {} 1 {} 0", s.w, s.h, s.stride, s.bpp));
                out.push("bmsize bitmap".into());
            } else {
                hit("GDI.DDB.DIBOBJECT");
                out.push(format!("bm dibsection {} {} {} 1 {} 1", s.w, s.h, s.stride, s.bpp));
                let used = if s.bpp <= 8 { 1 << s.bpp } else { 0 };
                // MEASURED: the height positive whatever the row order, BI_BITFIELDS for every
                // 16 bpp section, and GetObject(h, 0, NULL) the size of a BITMAP
                out.push(format!("dib {} {} {} {used}", s.bpp, s.h, if s.bpp == 16 { 3 } else { 0 }));
                out.push("bmsize bitmap".into());
            }
        }
        "bmbits" => {
            let sl = super::gdi::get(&g.slots, n(op, 1)?)?;
            let s = &sl.s;
            // a DIB section: MEASURED: its memory rows, each cut to a WORD aligned row
            let wb = if s.ddb { s.stride } else { hit("GDI.DDB.DIBBITS"); (s.w as usize * s.bpp as usize).div_ceil(16) * 2 };
            hit("GDI.DDB.BITS");
            out.push(format!("bmbits {}", wb * s.h as usize));
            let mem: Vec<&Vec<u8>> = if s.ddb || s.topdown { s.rows.iter().collect() } else { s.rows.iter().rev().collect() };
            out.push(mem.iter().flat_map(|r| r[..wb].iter().map(|b| format!("{b:02x}"))).collect());
        }
        "devcaps" => {
            super::gdi::get(&g.slots, n(op, 1)?)?;
            hit("GDI.DDB.CAPS");
            let mut l = String::from("caps");
            for i in 2..op.tok.len() {
                l += &format!(" {}", devcap(n(op, i)?)?);
            }
            out.push(l);
        }
        "objtype" => {
            super::gdi::get(&g.slots, n(op, 1)?)?;
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
