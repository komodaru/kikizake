//! The model of runner/gdipath.c's ops (spec/gdipath.txt): the path bracket, GetPath, paths of
//! lines and rectangles, StrokePath, AbortPath, FlattenPath of straight paths.
//!
//! Sources (Microsoft Learn): "BeginPath", "EndPath", "AbortPath", "CloseFigure", "GetPath",
//! "FlattenPath", "WidenPath", "StrokePath", "FillPath", "StrokeAndFillPath", "PathToRegion",
//! "SelectClipPath", "SetMiterLimit", "Paths" (which functions a path records).

use super::gdi::{Gdi, Slot};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const PT_CLOSEFIGURE: u8 = 1;
pub const PT_LINETO: u8 = 2;
pub const PT_BEZIERTO: u8 = 4;
pub const PT_MOVETO: u8 = 6;

/// a DC's path: none, open (being recorded) or closed (EndPath done)
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Path {
    #[default]
    None,
    Open(Vec<(i32, i32, u8)>),
    Closed(Vec<(i32, i32, u8)>),
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// a line op of gdiline.c while a path is open: its points go into the path, nothing is drawn
pub fn record(sl: &mut Slot, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let Path::Open(p) = &mut sl.ext.path else { return Ok(false) };
    let t0 = op.tok[0].as_str();
    let pts: Vec<(i32, i32)> = op.tok[2.min(op.tok.len())..].chunks(2).map(|c| Ok((num(&c[0]).ok_or(Fail::Bad)? as i32, num(c.get(1).ok_or(Fail::Bad)?).ok_or(Fail::Bad)? as i32))).collect::<Res<_>>()?;
    hit("GDI.PATH.RECORD");
    // (a figure starts at the current position when the path has no open figure there)
    let start = |p: &mut Vec<(i32, i32, u8)>, pos: (i32, i32)| {
        if p.last().is_none_or(|l| l.2 & PT_CLOSEFIGURE != 0) {
            p.push((pos.0, pos.1, PT_MOVETO));
        }
    };
    match t0 {
        "moveto" => {
            let old = sl.ext.pos;
            sl.ext.pos = pts[0];
            out.push(format!("pos {} {}", old.0, old.1));
            return record_move(p, sl.ext.pos);
        }
        "lineto" | "polylineto" => {
            start(p, sl.ext.pos);
            for &q in &pts {
                p.push((q.0, q.1, PT_LINETO));
            }
            sl.ext.pos = *pts.last().ok_or(Fail::Bad)?;
            out.push("ret 1".into());
        }
        "polyline" | "polygon" => {
            if pts.len() < 2 {
                return Err(Fail::Unmodeled("GDI.LINE.POLYSHORT"));
            }
            p.push((pts[0].0, pts[0].1, PT_MOVETO));
            for &q in &pts[1..] {
                p.push((q.0, q.1, PT_LINETO));
            }
            if t0 == "polygon" {
                p.last_mut().unwrap().2 |= PT_CLOSEFIGURE;
            }
            out.push("ret 1".into());
        }
        "rectangle" => {
            hit("GDI.PATH.RECT");
            let (l, r) = (pts[0].0.min(pts[1].0), pts[0].0.max(pts[1].0));
            let (t, b) = (pts[0].1.min(pts[1].1), pts[0].1.max(pts[1].1));
            // (GM_COMPATIBLE: the right and bottom edges excluded)
            let (r, b) = (r - 1, b - 1);
            let c = if sl.ext.arcdir == 2 { [(r, b), (l, b), (l, t), (r, t)] } else { [(r, t), (l, t), (l, b), (r, b)] };
            for (i, q) in c.iter().enumerate() {
                p.push((q.0, q.1, if i == 0 { PT_MOVETO } else { PT_LINETO }));
            }
            p.last_mut().unwrap().2 |= PT_CLOSEFIGURE;
            out.push("ret 1".into());
        }
        _ => return Err(Fail::Unmodeled("GDI.PATH.CURVE")),
    }
    Ok(true)
}

/// MoveToEx in a path: a new figure at the point (a lone PT_MOVETO is replaced by the next)
fn record_move(p: &mut Vec<(i32, i32, u8)>, at: (i32, i32)) -> Res<bool> {
    hit("GDI.PATH.MOVETO");
    if p.last().is_some_and(|l| l.2 == PT_MOVETO) {
        p.pop();
    }
    p.push((at.0, at.1, PT_MOVETO));
    Ok(true)
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if !matches!(t0, "beginpath" | "endpath" | "abortpath" | "closefig" | "flatten" | "widen" | "strokepath" | "fillpath" | "strokefill" | "selclippath" | "getpath" | "pathrgn" | "miter") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
    if sl.ext.mapped() {
        return Err(Fail::Unmodeled("GDI.MAP.DRAW"));
    }
    let path = &mut sl.ext.path;
    let ok = match t0 {
        "beginpath" => {
            hit("GDI.PATH.BEGIN");
            if matches!(path, Path::Open(_)) {
                return Err(Fail::Unmodeled("GDI.PATH.BEGINAGAIN"));
            }
            *path = Path::Open(Vec::new());
            true
        }
        "endpath" => {
            hit("GDI.PATH.END");
            match std::mem::take(path) {
                Path::Open(p) => {
                    *path = Path::Closed(p);
                    true
                }
                other => {
                    *path = other;
                    false
                }
            }
        }
        "abortpath" => {
            hit("GDI.PATH.ABORT");
            *path = Path::None;
            true
        }
        "closefig" => {
            hit("GDI.PATH.CLOSE");
            match path {
                Path::Open(p) if p.last().is_some_and(|l| l.2 != PT_MOVETO) => {
                    p.last_mut().unwrap().2 |= PT_CLOSEFIGURE;
                    true
                }
                Path::Open(_) => return Err(Fail::Unmodeled("GDI.PATH.CLOSEEMPTY")),
                _ => false,
            }
        }
        "getpath" => {
            hit("GDI.PATH.GET");
            match path {
                Path::Closed(p) => {
                    out.push(format!("path {}", p.len()));
                    for q in p.iter() {
                        out.push(format!("pt {} {} {}", q.0, q.1, q.2));
                    }
                }
                _ => out.push("path -1".into()),
            }
            return Ok(true);
        }
        "flatten" => {
            hit("GDI.PATH.FLATTEN");
            match path {
                Path::Closed(p) if p.iter().all(|q| q.2 & PT_BEZIERTO == 0 || q.2 == PT_MOVETO) => true,
                Path::Closed(_) => return Err(Fail::Unmodeled("GDI.PATH.CURVE")),
                _ => false,
            }
        }
        "strokepath" => {
            hit("GDI.PATH.STROKE");
            let Path::Closed(p) = std::mem::take(path) else {
                out.push("strokepath 0".into());
                return Ok(true);
            };
            let mut fig_start = (0, 0);
            let mut prev = (0, 0);
            for q in &p {
                let at = (q.0, q.1);
                if q.2 == PT_MOVETO {
                    fig_start = at;
                } else if q.2 & PT_BEZIERTO != 0 {
                    return Err(Fail::Unmodeled("GDI.PATH.CURVE"));
                } else {
                    sl.line(prev, at)?;
                    if q.2 & PT_CLOSEFIGURE != 0 {
                        sl.line(at, fig_start)?;
                    }
                }
                prev = at;
            }
            true
        }
        "fillpath" | "strokefill" | "selclippath" | "pathrgn" => {
            if !matches!(path, Path::Closed(_)) {
                // (no closed path: the call fails, the DC keeps what it has)
                hit("GDI.PATH.NOPATH");
                out.push(if t0 == "pathrgn" { "pathrgn 0".into() } else { format!("{t0} 0") });
                return Ok(true);
            }
            return Err(Fail::Unmodeled("GDI.PATH.FILL"));
        }
        "widen" => return Err(Fail::Unmodeled("GDI.PATH.WIDEN")),
        _ => return Err(Fail::Unmodeled("GDI.PATH.MITER")),
    };
    out.push(format!("{t0} {}", ok as u8));
    Ok(true)
}
