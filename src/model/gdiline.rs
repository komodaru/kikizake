//! The model of runner/gdiline.c's ops (spec/gdiline.txt): pens, SetROP2, the current position,
//! lines and polylines with cosmetic pens, rectangles, FillRect / FrameRect / InvertRect.
//!
//! Sources (Microsoft Learn): "Binary Raster Operations", "SetROP2", "CreatePen", "LineTo",
//! "MoveToEx", "Polyline", "PolylineTo", "Rectangle", "FillRect", "FrameRect", "InvertRect",
//! "SetPolyFillMode", "SetArcDirection". Which pixels a line lights is not documented; the rule
//! is the one Windows' answers showed (golden lines-32/-64).

use crate::spec::{clause, open};
use super::gdi::{Brush, Gdi, Slot};
use super::{Fail, Res};
use crate::case::{color, num, Op};
use crate::spec::hit;

#[derive(Clone, Debug, PartialEq)]
pub enum Pen {
    /// CreatePen(style, width, color)
    Plain { style: i64, width: i64, color: u32 },
    /// ExtCreatePen(style, width, solid or hatched brush)
    Ext { style: i64, width: i64, color: u32 },
    Null,
}

/// what a DC keeps beyond gdi.c's state
#[derive(Clone, Debug, PartialEq)]
pub struct Draw {
    pub pen: Pen,
    pub rop2: i64,
    pub pos: (i32, i32),
    pub polyfill: i64,
    pub arcdir: i64,
    pub map: super::gdixform::Map,
    pub path: super::gdipath::Path,
    /// SetTextAlign, SetTextCharacterExtra
    pub align: i64,
    pub extra: i64,
    /// SetMiterLimit's limit
    pub miter: f32,
}

impl Default for Draw {
    /// a new DC: the black pen, R2_COPYPEN, (0, 0), ALTERNATE, AD_COUNTERCLOCKWISE
    fn default() -> Draw {
        Draw { pen: Pen::Plain { style: 0, width: 0, color: 0 }, rop2: 13, pos: (0, 0), polyfill: 1, arcdir: 1, map: Default::default(), path: Default::default(), align: 0, extra: 0, miter: 10.0 }
    }
}

impl Draw {
    /// whether logical coordinates differ from device coordinates
    pub fn mapped(&self) -> bool {
        !self.map.identity()
    }
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn c(op: &Op, i: usize) -> Res<u32> {
    op.tok.get(i).and_then(|t| color(t)).ok_or(Fail::Bad)
}
fn points(op: &Op, from: usize) -> Res<Vec<(i32, i32)>> {
    let t = &op.tok[from.min(op.tok.len())..];
    if t.len() % 2 == 1 {
        return Err(Fail::Bad);
    }
    t.chunks(2).take(64).map(|p| Ok((num(&p[0]).ok_or(Fail::Bad)? as i32, num(&p[1]).ok_or(Fail::Bad)? as i32))).collect()
}

/// the binary raster operation `code` (R2_BLACK = 1 .. R2_WHITE = 16) on pen and destination
/// values: each result bit is bit (P<<1 | D) of code - 1
pub fn rop2(code: i64, p: u32, d: u32) -> u32 {
    let k = (code - 1) as u32;
    let mut r = 0;
    for (bit, (mp, md)) in [(!p, !d), (!p, d), (p, !d), (p, d)].into_iter().enumerate() {
        if k >> bit & 1 == 1 {
            r |= mp & md;
        }
    }
    r
}

/// the pixels of a cosmetic line from a to b (the last point not drawn): Bresenham's, stepping
/// the minor axis when the error passes half a pixel; at exactly half (`tie`), per octant
pub fn line_pixels(a: (i32, i32), b: (i32, i32)) -> Vec<(i32, i32)> {
    hit(clause::GDI_LINE_PIXELS);
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (adx, ady) = (dx.abs(), dy.abs());
    let (sx, sy) = (dx.signum(), dy.signum());
    let xmajor = adx >= ady;
    let (len, minor) = if xmajor { (adx, ady) } else { (ady, adx) };
    let up = tie_up(dx, dy);
    let mut v = Vec::with_capacity(len as usize);
    for i in 0..len {
        // minor offset: round(i * minor / len), a half going `up` (away from the start) or not
        let num = 2 * i * minor + len;
        let den = 2 * len;
        let m = if num % den == 0 && !up { num / den - 1 } else { num / den };
        v.push(if xmajor { (a.0 + sx * i, a.1 + sy * m) } else { (a.0 + sx * m, a.1 + sy * i) });
    }
    v
}

/// whether a line's minor step at exactly half a pixel is taken: the half goes to the smaller
/// minor coordinate (up for x-major lines, left for y-major ones) whichever way the line runs
/// (MEASURED, golden lines: every line from the center of a 17x17 picture, both ways)
fn tie_up(dx: i32, dy: i32) -> bool {
    hit(clause::GDI_LINE_TIE);
    if dx.abs() >= dy.abs() { dy < 0 } else { dx < 0 }
}

impl Slot {
    /// the pen as a pixel value of this format, or why it is not modelled
    fn pen_value(&self) -> Res<Option<u32>> {
        match &self.ext.pen {
            Pen::Null => Ok(None),
            Pen::Plain { style: 0, width: 0 | 1, color } => Ok(Some(self.s.from_rgb(*color))),
            Pen::Plain { style: 5, .. } => Ok(None),
            Pen::Plain { style: 1..=4, width: 0 | 1, .. } => Err(Fail::Unmodeled(open::GDI_PEN_STYLE)),
            Pen::Plain { style: 6, width: 0 | 1, color } => Ok(Some(self.s.from_rgb(*color))),
            Pen::Plain { .. } => Err(Fail::Unmodeled(open::GDI_PEN_WIDE)),
            Pen::Ext { .. } => Err(Fail::Unmodeled(open::GDI_PEN_EXT)),
        }
    }
    /// one pixel of the pen through ROP2, where visible
    pub fn plot(&mut self, x: i32, y: i32, v: u32) {
        if self.visible(x, y) {
            let d = self.s.get(x, y);
            let m = self.s.mask();
            self.s.set(x, y, rop2(self.ext.rop2, v, d) & m);
        }
    }
    pub fn line(&mut self, a: (i32, i32), b: (i32, i32)) -> Res<()> {
        hit(clause::GDI_LINE_LAST);
        if let Some(v) = self.pen_value()? {
            if self.ext.rop2 != 13 {
                hit(clause::GDI_ROP2_TRUTH);
            }
            for (x, y) in line_pixels(a, b) {
                self.plot(x, y, v);
            }
        }
        Ok(())
    }
    /// the brush over [l, r) x [t, b) with PATCOPY through the clip region
    pub fn fill_rect(&mut self, l: i32, t: i32, r: i32, b: i32) -> Res<()> {
        self.fill_with(l, t, r, b, 13)
    }
    /// the brush over [l, r) x [t, b) through the DC's ROP2 (a shape's interior)
    pub fn fill_shape(&mut self, l: i32, t: i32, r: i32, b: i32) -> Res<()> {
        self.fill_with(l, t, r, b, self.ext.rop2)
    }
    fn fill_with(&mut self, l: i32, t: i32, r: i32, b: i32, rop: i64) -> Res<()> {
        let v = match &self.brush {
            Brush::Solid(c) => self.s.from_rgb(*c),
            Brush::Null => return Ok(()),
            _ => return Err(Fail::Unmodeled(open::GDI_SHAPE_BRUSH)),
        };
        let m = self.s.mask();
        for y in t.max(0)..b.min(self.s.h) {
            for x in l.max(0)..r.min(self.s.w) {
                if self.visible(x, y) {
                    let d = self.s.get(x, y);
                    self.s.set(x, y, rop2(rop, v, d) & m);
                }
            }
        }
        Ok(())
    }
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if !matches!(t0, "pen" | "extpen" | "moveto" | "getpos" | "lineto" | "polyline" | "polylineto" | "polygon" | "polybezier" | "polybezierto" | "rectangle" | "ellipse" | "roundrect" | "arc" | "arcto" | "pie" | "chord" | "fillrect" | "framerect" | "invertrect" | "rop2" | "polyfill" | "arcdir") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    super::gdi::get(&g.slots, k)?;
    let sl = g.slots[k as usize].as_mut().unwrap();
    if sl.ext.mapped() && !matches!(t0, "pen" | "extpen" | "rop2" | "polyfill" | "arcdir" | "moveto" | "getpos") {
        if !matches!(sl.ext.path, super::gdipath::Path::None) {
            return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
        }
        let pos = sl.ext.pos;
        let Some((d, from)) = super::gdixform::device_op(&mut sl.ext.map, pos, op)? else {
            return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
        };
        let map = std::mem::take(&mut sl.ext.map);
        sl.ext.pos = from;
        let r = op_device(g, k, &d, out);
        let sl = g.slots[k as usize].as_mut().unwrap();
        sl.ext.map = map;
        sl.ext.pos = if t0 == "lineto" { (n(op, 2)? as i32, n(op, 3)? as i32) } else { pos };
        return r;
    }
    op_device(g, k, op, out)
}

/// an op in device coordinates (or with the mapping the identity)
fn op_device(g: &mut Gdi, k: i64, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    let sl = g.slots[k as usize].as_mut().unwrap();
    if !matches!(t0, "pen" | "extpen" | "rop2" | "polyfill" | "arcdir" | "getpos" | "fillrect" | "framerect" | "invertrect") && super::gdipath::record(sl, op, out)? {
        return Ok(true);
    }
    match t0 {
        "pen" => {
            sl.ext.pen = if op.tok.get(2).is_some_and(|t| t == "null") {
                Pen::Null
            } else {
                hit(clause::GDI_PEN_CREATE);
                Pen::Plain { style: n(op, 2)?, width: n(op, 3)?, color: c(op, 4)? }
            };
        }
        "extpen" => {
            let (style, width, color) = (n(op, 2)?, n(op, 3)?, c(op, 4)?);
            sl.ext.pen = Pen::Ext { style, width, color };
        }
        "rop2" | "polyfill" | "arcdir" => {
            let m = n(op, 2)?;
            let (old, ok) = match t0 {
                // MEASURED: SetROP2(0) is taken (and draws as R2_WHITE, its truth table's low bits)
                "rop2" => (&mut sl.ext.rop2, (0..=16).contains(&m)),
                "polyfill" => (&mut sl.ext.polyfill, (1..=2).contains(&m)),
                _ => (&mut sl.ext.arcdir, (1..=2).contains(&m)),
            };
            hit(clause::GDI_STATE_OLD);
            // (out of range: 0, the mode stays)
            if t0 == "rop2" && m == 0 {
                hit(clause::GDI_ROP2_ZERO);
            }
            let v = if ok { std::mem::replace(old, m) } else { 0 };
            out.push(format!("{t0} {v}"));
        }
        "moveto" => {
            let p = (n(op, 2)? as i32, n(op, 3)? as i32);
            hit(clause::GDI_POS_MOVE);
            let old = std::mem::replace(&mut sl.ext.pos, p);
            out.push(format!("pos {} {}", old.0, old.1));
        }
        "getpos" => out.push(format!("pos {} {}", sl.ext.pos.0, sl.ext.pos.1)),
        "lineto" => {
            let p = (n(op, 2)? as i32, n(op, 3)? as i32);
            let a = sl.ext.pos;
            sl.line(a, p)?;
            sl.ext.pos = p;
            out.push("ret 1".into());
        }
        "polyline" | "polylineto" => {
            let p = points(op, 2)?;
            hit(clause::GDI_LINE_POLY);
            if t0 == "polyline" && p.len() < 2 {
                // MEASURED: fails, nothing drawn, the current position kept
                hit(clause::GDI_LINE_POLYSHORT);
                out.push("ret 0".into());
                return Ok(true);
            }
            let mut a = if t0 == "polyline" { p[0] } else { sl.ext.pos };
            for &b in &p[if t0 == "polyline" { 1 } else { 0 }..] {
                sl.line(a, b)?;
                a = b;
            }
            if t0 == "polylineto" {
                sl.ext.pos = a;
            }
            out.push("ret 1".into());
        }
        "fillrect" | "framerect" | "invertrect" => {
            let p = points(op, 2)?;
            if p.len() != 2 {
                return Err(Fail::Bad);
            }
            let ((l, t), (r, b)) = (p[0], p[1]);
            if l >= r || t >= b {
                hit(clause::GDI_RECT_EMPTY);
                if t0 == "framerect" && (l > r || t > b) {
                    // MEASURED: FrameRect fails on corners out of order, drawing nothing
                    out.push("ret 0".into());
                    return Ok(true);
                }
            }
            // MEASURED: FillRect and InvertRect take the corners in either order (an empty
            // rectangle draws nothing); FrameRect's edges of a 0 wide or high one still draw
            let (l, r, t, b) = if t0 == "framerect" { (l, r, t, b) } else { (l.min(r), l.max(r), t.min(b), t.max(b)) };
            match t0 {
                "fillrect" => {
                    hit(clause::GDI_RECT_FILL);
                    sl.fill_rect(l, t, r, b)?;
                }
                "invertrect" => {
                    hit(clause::GDI_RECT_INVERT);
                    let m = sl.s.mask();
                    for y in t..b {
                        for x in l..r {
                            if sl.visible(x, y) {
                                let d = sl.s.get(x, y);
                                sl.s.set(x, y, !d & m);
                            }
                        }
                    }
                }
                _ => {
                    hit(clause::GDI_RECT_FRAME);
                    sl.fill_rect(l, t, r, t + 1)?;
                    sl.fill_rect(l, b - 1, r, b)?;
                    sl.fill_rect(l, t, l + 1, b)?;
                    sl.fill_rect(r - 1, t, r, b)?;
                }
            }
            out.push("ret 1".into());
        }
        "rectangle" => {
            let p = points(op, 2)?;
            if p.len() != 2 {
                return Err(Fail::Bad);
            }
            let (l, r) = (p[0].0.min(p[1].0), p[0].0.max(p[1].0));
            let (t, b) = (p[0].1.min(p[1].1), p[0].1.max(p[1].1));
            hit(clause::GDI_SHAPE_RECT);
            match sl.pen_value()? {
                None => {
                    // a null pen: the interior is one pixel larger each way
                    sl.fill_shape(l, t, r - 1, b - 1)?;
                }
                Some(v) => {
                    if r - l < 1 || b - t < 1 {
                        // MEASURED: an empty rectangle draws nothing
                        hit(clause::GDI_SHAPE_RECTTHIN);
                    } else {
                        // MEASURED (through XOR and NOT): the outline is four lines, each
                        // without its last pixel, from (l, t) through (r - 1, t), (r - 1, b - 1)
                        // and (l, b - 1) back; a one pixel wide or high rectangle so passes
                        // its inner pixels twice, and one pixel draws nothing
                        if r - l < 3 || b - t < 3 {
                            hit(clause::GDI_SHAPE_RECTTHIN);
                        }
                        sl.fill_shape(l + 1, t + 1, r - 1, b - 1)?;
                        let c = [(l, t), (r - 1, t), (r - 1, b - 1), (l, b - 1)];
                        for i in 0..4 {
                            for (x, y) in line_pixels(c[i], c[(i + 1) % 4]) {
                                sl.plot(x, y, v);
                            }
                        }
                    }
                }
            }
            out.push("ret 1".into());
        }
        "polygon" => {
            let p = points(op, 2)?;
            if p.len() < 2 {
                return Err(Fail::Bad);
            }
            // MEASURED: the interior is CreatePolygonRgn's region of the points in the fill
            // mode, the brush over it; then the pen's closed outline
            hit(clause::GDI_SHAPE_POLYFILL);
            let r = super::rgn::polygon_pixels(&[p.clone()], sl.ext.polyfill == 2);
            for (l, t, rr, b) in r.rects() {
                sl.fill_shape(l, t, rr, b)?;
            }
            for i in 0..p.len() {
                sl.line(p[i], p[(i + 1) % p.len()])?;
            }
            out.push("ret 1".into());
        }
        "ellipse" | "roundrect" | "arc" | "arcto" | "pie" | "chord" => return Err(Fail::Unmodeled(open::GDI_SHAPE_CURVE)),
        _ => return Err(Fail::Unmodeled(open::GDI_LINE_BEZIER)),
    }
    Ok(true)
}
