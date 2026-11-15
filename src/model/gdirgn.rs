//! The model of runner/gdirgn.c's ops (spec/gdirgn.txt): rectangle regions and their set
//! algebra (CombineRgn), region data, hit tests, FillRgn & co, and clip regions.
//!
//! Sources (Microsoft Learn): "CreateRectRgn", "SetRectRgn", "CombineRgn", "GetRegionData",
//! "RGNDATAHEADER", "GetRgnBox", "OffsetRgn", "PtInRegion", "RectInRegion", "EqualRgn",
//! "FillRgn", "FrameRgn", "InvertRgn", "PaintRgn", "ExtSelectClipRgn", "SelectClipRgn",
//! "GetClipBox", "GetClipRgn", "ExcludeClipRect", "OffsetClipRgn", "PtVisible", "RectVisible".

use crate::spec::{clause, open};
use super::gdi::{Brush, Gdi};
use super::rgn::{Rgn, SetOp, NULLREGION};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn n32(op: &Op, i: usize) -> Res<i32> {
    n(op, i).map(|v| v as i32)
}
fn rect(op: &Op, from: usize) -> Res<(i32, i32, i32, i32)> {
    Ok((n32(op, from)?, n32(op, from + 1)?, n32(op, from + 2)?, n32(op, from + 3)?))
}

/// RectInRegion / RectVisible: the rectangle (corners in either order, even of no area)
/// overlaps one of the region's: l < right, r > left, t < bottom, b > top (MEASURED)
fn overlaps(g: &Rgn, q: (i32, i32, i32, i32)) -> bool {
    let (l, r) = (q.0.min(q.2), q.0.max(q.2));
    let (t, b) = (q.1.min(q.3), q.1.max(q.3));
    hit(clause::GDI_RGN_OVERLAP);
    g.rects().iter().any(|x| l < x.2 && r > x.0 && t < x.3 && b > x.1)
}

fn data(g: &Rgn) -> Vec<String> {
    let rs = g.rects();
    let b = g.bounds();
    let mut l = String::from("rects");
    for r in &rs {
        l += &format!(" {} {} {} {}", r.0, r.1, r.2, r.3);
    }
    vec![format!("rgndata {} {} {} {} {}", rs.len(), b.0, b.1, b.2, b.3), l]
}

impl Gdi {
    fn rslot(&mut self, op: &Op, i: usize) -> Res<&mut Option<Rgn>> {
        let k = n(op, i)?;
        self.rgns.get_mut(k as usize).ok_or(Fail::Bad)
    }
    fn rgn(&mut self, op: &Op, i: usize) -> Res<Rgn> {
        // (an empty slot: the executor prints "bad line N" and goes on, as golden path curve-24 shows)
        self.rslot(op, i)?.clone().ok_or(Fail::Empty)
    }
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    match t0 {
        "rgn" => {
            let kind = op.tok.get(2).ok_or(Fail::Bad)?.clone();
            let r = match kind.as_str() {
                "null" => Rgn::empty(),
                "rect" => {
                    let r = rect(op, 3)?;
                    hit(clause::GDI_RGN_RECT);
                    Rgn::rect(r.0, r.1, r.2, r.3)
                }
                "ellipse" | "round" => return Err(Fail::Unmodeled(open::GDI_RGN_ELLIPSE)),
                "poly" => {
                    let mode = n(op, 3)?;
                    let t = &op.tok[4..];
                    if t.len() % 2 != 0 {
                        return Err(Fail::Bad);
                    }
                    let pts: Vec<(i32, i32)> = t.chunks(2).take(64).map(|p| Ok((num(&p[0]).ok_or(Fail::Bad)? as i32, num(&p[1]).ok_or(Fail::Bad)? as i32))).collect::<Res<_>>()?;
                    let winding = match mode {
                        1 => false,
                        2 => true,
                        _ => return Err(Fail::Unmodeled(open::GDI_RGN_POLYGON)),
                    };
                    hit(clause::GDI_RGN_POLYSCAN);
                    super::rgn::polygon(&[pts], winding)?
                }
                _ => return Err(Fail::Bad),
            };
            *g.rslot(op, 1)? = Some(r);
        }
        "setrect" => {
            let r = rect(op, 2)?;
            g.rgn(op, 1)?;
            hit(clause::GDI_RGN_SETRECT);
            *g.rslot(op, 1)? = Some(Rgn::rect(r.0, r.1, r.2, r.3));
            out.push("setrect 1".into());
        }
        "combine" => {
            let (a, b, m) = (g.rgn(op, 2)?, g.rgn(op, 3)?, n(op, 4)?);
            let r = match (m, SetOp::from_mode(m)) {
                (5, _) => {
                    hit(clause::GDI_RGN_COPY);
                    a
                }
                (_, Some(o)) => {
                    hit(clause::GDI_RGN_COMBINE);
                    a.combine(&b, o)
                }
                _ => {
                    // MEASURED: ERROR, the destination as it was
                    hit(clause::GDI_RGN_BADMODE);
                    out.push("combine 0".into());
                    return Ok(true);
                }
            };
            hit(clause::GDI_RGN_KIND);
            out.push(format!("combine {}", r.kind()));
            *g.rslot(op, 1)? = Some(r);
        }
        "offsetrgn" | "rgnbox" | "ptin" | "rectin" | "equal" | "rgndata" => {
            let mut r = g.rgn(op, 1)?;
            match t0 {
                "offsetrgn" => {
                    let (dx, dy) = (n32(op, 2)?, n32(op, 3)?);
                    hit(clause::GDI_RGN_OFFSET);
                    r.offset(dx, dy);
                    out.push(format!("offset {}", r.kind()));
                    *g.rslot(op, 1)? = Some(r);
                }
                "rgnbox" => {
                    hit(clause::GDI_RGN_BOX);
                    let b = r.bounds();
                    out.push(format!("box {} {} {} {} {}", r.kind(), b.0, b.1, b.2, b.3));
                }
                "ptin" => {
                    hit(clause::GDI_RGN_PTIN);
                    out.push(format!("ptin {}", r.contains(n32(op, 2)?, n32(op, 3)?) as u8));
                }
                "rectin" => {
                    let q = rect(op, 2)?;
                    hit(clause::GDI_RGN_RECTIN);
                    out.push(format!("rectin {}", overlaps(&r, q) as u8));
                }
                "equal" => {
                    let o = g.rgn(op, 2)?;
                    hit(clause::GDI_RGN_EQUAL);
                    out.push(format!("equal {}", (o == r) as u8));
                }
                _ => {
                    hit(clause::GDI_RGN_DATA);
                    out.extend(data(&r));
                }
            }
        }
        "fillrgn" | "framergn" | "invertrgn" | "paintrgn" => {
            let r = g.rgn(op, 2)?;
            let k = n(op, 1)?;
            let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
            if sl.ext.mapped() {
                return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
            }
            let area = match t0 {
                "framergn" => {
                    let (w, h) = (n32(op, 3)?, n32(op, 4)?);
                    hit(clause::GDI_RGN_FRAME);
                    let mut inner = r.clone();
                    for (dx, dy) in [(-w, 0), (w, 0), (0, -h), (0, h)] {
                        inner = inner.combine(&r.offset_by(dx, dy), SetOp::And);
                    }
                    r.combine(&inner, SetOp::Diff)
                }
                _ => r,
            };
            let m = sl.s.mask();
            let v = match (&sl.brush, t0) {
                (_, "invertrgn") => {
                    hit(clause::GDI_RGN_INVERT);
                    None
                }
                (Brush::Solid(c), _) => {
                    hit(if t0 == "paintrgn" { clause::GDI_RGN_PAINT } else { clause::GDI_RGN_FILL });
                    Some(sl.s.from_rgb(*c))
                }
                _ => return Err(Fail::Unmodeled(open::GDI_SHAPE_BRUSH)),
            };
            for (l, t, rr, b) in area.rects() {
                for y in t.max(0)..b.min(sl.s.h) {
                    for x in l.max(0)..rr.min(sl.s.w) {
                        if sl.visible(x, y) {
                            let d = sl.s.get(x, y);
                            let nv = match v {
                                None => !d & m,
                                Some(p) if t0 == "paintrgn" => super::gdiline::rop2(sl.ext.rop2, p, d) & m,
                                Some(p) => p,
                            };
                            sl.s.set(x, y, nv);
                        }
                    }
                }
            }
            out.push("ret 1".into());
        }
        "selclip" | "clipbox" | "getcliprgn" | "exclude" | "offsetclip" | "ptvisible" | "rectvisible" => return clip_op(g, op, out),
        _ => return Ok(false),
    }
    Ok(true)
}

/// SelectClipRgn / ExtSelectClipRgn of r (None: NULL) in mode m: the type of the clip region as
/// it now limits the bitmap, or None (ERROR, the clip region as it was)
pub fn select_clip(sl: &mut super::gdi::Slot, arg: Option<Rgn>, m: i64) -> Option<i32> {
    let dev = Rgn::rect(0, 0, sl.s.w, sl.s.h);
    let new = match (arg, m) {
        (None, 5) => {
            hit(clause::GDI_CLIP_SELNONE);
            None
        }
        (None, _) => {
            // MEASURED: ERROR, the clip region as it was
            hit(clause::GDI_CLIP_SELNULLMODE);
            return None;
        }
        (Some(r), 5) => {
            hit(clause::GDI_CLIP_SELCOPY);
            Some(r)
        }
        (Some(r), m) => {
            let Some(o) = SetOp::from_mode(m) else {
                // (as CombineRgn: ERROR, the clip region as it was)
                hit(clause::GDI_CLIP_SELBADMODE);
                return None;
            };
            hit(clause::GDI_CLIP_SELMODE);
            match &sl.clip {
                Some(c) => Some(c.combine(&r, o)),
                // MEASURED: RGN_AND without a clip region keeps the region whole
                // (GetClipRgn returns it unclipped by the bitmap)
                None if o == SetOp::And => {
                    hit(clause::GDI_CLIP_SELNOCLIPAND);
                    Some(r)
                }
                None => {
                    // MEASURED: no clip region combines as the bitmap's rectangle
                    hit(clause::GDI_CLIP_SELNOCLIP);
                    Some(dev.combine(&r, o))
                }
            }
        }
    };
    let kind = new.as_ref().map_or(dev.kind(), |c| c.combine(&dev, SetOp::And).kind());
    sl.clip = new;
    Some(kind)
}

fn clip_op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    let arg = if t0 == "selclip" && op.tok.get(2).is_some_and(|t| t != "none") { Some(g.rgn(op, 2)?) } else { None };
    let k = n(op, 1)?;
    if g.slots.get(k as usize).and_then(|s| s.as_ref()).is_none() {
        return Err(Fail::Bad);
    }
    if g.slots[k as usize].as_ref().unwrap().ext.mapped() {
        return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
    }
    let sl = g.slots[k as usize].as_mut().unwrap();
    let dev = Rgn::rect(0, 0, sl.s.w, sl.s.h);
    match t0 {
        "selclip" => {
            let m = if op.tok.len() > 3 { n(op, 3)? } else { 5 };
            match select_clip(sl, arg, m) {
                Some(kind) => out.push(format!("selclip {kind}")),
                None => out.push("selclip 0".into()),
            }
        }
        "clipbox" => {
            hit(clause::GDI_CLIP_BOX);
            let v = sl.clip.as_ref().map_or(dev.clone(), |c| c.combine(&dev, SetOp::And));
            let b = v.bounds();
            out.push(format!("clipbox {} {} {} {} {}", v.kind(), b.0, b.1, b.2, b.3));
        }
        "getcliprgn" => {
            hit(clause::GDI_CLIP_GETRGN);
            let c = sl.clip.clone();
            out.push(format!("getcliprgn {}", c.is_some() as u8));
            let slot = g.rslot(op, 2)?;
            match c {
                Some(c) => *slot = Some(c),
                None => {
                    if slot.is_none() {
                        *slot = Some(Rgn::empty());
                    }
                }
            }
        }
        "exclude" => {
            let q = rect(op, 2)?;
            hit(clause::GDI_CLIP_EXCLUDE);
            if q.0 > q.2 || q.1 > q.3 {
                // MEASURED: the corners in either order
                hit(clause::GDI_CLIP_EXUNORDERED);
            }
            let c = sl.clip.as_ref().unwrap_or(&dev).combine(&Rgn::rect(q.0, q.1, q.2, q.3), SetOp::Diff);
            // MEASURED: COMPLEXREGION (3) whatever the result, NULLREGION (1) when it is empty
            hit(clause::GDI_CLIP_EXRESULT);
            out.push(format!("exclude {}", if c.kind() == NULLREGION { 1 } else { 3 }));
            sl.clip = Some(c);
        }
        "offsetclip" => {
            let (dx, dy) = (n32(op, 2)?, n32(op, 3)?);
            hit(clause::GDI_CLIP_OFFSET);
            match &mut sl.clip {
                Some(c) => {
                    // MEASURED: the type of the clip region itself, not of its visible part
                    c.offset(dx, dy);
                    out.push(format!("offsetclip {}", c.kind()));
                }
                None => {
                    // MEASURED: SIMPLEREGION, and the DC keeps no clip region
                    hit(clause::GDI_CLIP_OFFSETNONE);
                    out.push("offsetclip 2".into());
                }
            }
        }
        "ptvisible" => {
            hit(clause::GDI_CLIP_PTVISIBLE);
            let (x, y) = (n32(op, 2)?, n32(op, 3)?);
            out.push(format!("ptvisible {}", sl.visible(x, y) as u8));
        }
        _ => {
            let q = rect(op, 2)?;
            hit(clause::GDI_CLIP_RECTVISIBLE);
            let v = sl.clip.as_ref().map_or(dev.clone(), |c| c.combine(&dev, SetOp::And));
            out.push(format!("rectvisible {}", overlaps(&v, q) as u8));
        }
    }
    Ok(true)
}
