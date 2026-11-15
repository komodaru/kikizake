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

/// a point in 28.4 device units (1/16 pixel), pixel (x, y)'s center at (16 x, 16 y)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fix28(pub i64, pub i64);

/// the pixels of a cosmetic line between two 28.4 points by grid intersect quantization
/// (MEASURED, GDI.LINE.GIQ: PolyBezier's flattened polylines, 14 of 14): a pixel is lit when the
/// line crosses its diamond, the open square |x - cx| + |y - cy| < 1/2 with its right and bottom
/// corners, and does not end inside it; whole ends give line_pixels' pixels. A diagonal
/// running along diamond edges takes their closed lower edges instead (xform diagline-*)
pub fn giq_pixels(a: Fix28, b: Fix28) -> Option<Vec<(i32, i32)>> {
    hit(clause::GDI_LINE_GIQ);
    let (dx, dy) = (i128::from(b.0 - a.0), i128::from(b.1 - a.1));
    if dx == 0 && dy == 0 {
        return Some(Vec::new());
    }
    let px = |v: i64| v.div_euclid(16);
    let (x0, x1) = (px(a.0.min(b.0)) - 1, px(a.0.max(b.0)) + 2);
    let (y0, y1) = (px(a.1.min(b.1)) - 1, px(a.1.max(b.1)) + 2);
    if dx.abs() == dy.abs() && (a.0 + if dx == dy { -a.1 } else { a.1 }).rem_euclid(16) == 8 {
        // MEASURED (xform diagline-*, 24 of 24): a diagonal running along diamond edges meets
        // no open diamond; it lights the pixels whose lower edge on its line (the lower left
        // one for x - y, the lower right one for x + y), closed at both corners, it runs along,
        // except the one whose such edge holds the end
        let (lo, hi) = (a.0.min(b.0), a.0.max(b.0));
        let mut v = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (cx, cy) = (i64::from(x) * 16, i64::from(y) * 16);
                // the edge from its corner at y = cy to the bottom one, in x
                let (ex0, ex1) = if dx == dy { (cx - 8, cx) } else { (cx, cx + 8) };
                let on_line = dx * i128::from(cy - a.1) == dy * i128::from(if dx == dy { cx - 8 } else { cx + 8 } - a.0);
                if on_line && lo.max(ex0) <= hi.min(ex1) && !(ex0..=ex1).contains(&b.0) {
                    v.push((i32::try_from(x).ok()?, i32::try_from(y).ok()?));
                }
            }
        }
        return Some(v);
    }
    // (corner (x, y) on the segment: collinear, and between the ends)
    let on = |x: i128, y: i128| -> bool {
        let (ux, uy) = (x - i128::from(a.0), y - i128::from(a.1));
        dx * uy - dy * ux == 0 && (0..=dx * dx + dy * dy).contains(&(ux * dx + uy * dy))
    };
    let inside = |p: (i128, i128), c: (i128, i128)| -> bool { (p.0 - c.0).abs() + (p.1 - c.1).abs() < 8 || p == (c.0 + 8, c.1) || p == (c.0, c.1 + 8) };
    // the least L1 distance from the segment to c is at an end or where x or y crosses c's: t
    // = n / d, compared as d * distance < 8 d
    let crosses = |c: (i128, i128)| -> bool {
        let (ox, oy) = (i128::from(a.0) - c.0, i128::from(a.1) - c.1);
        let mut ts: Vec<(i128, i128)> = vec![(0, 1), (1, 1)];
        if dx != 0 {
            ts.push(if dx > 0 { (-ox, dx) } else { (ox, -dx) });
        }
        if dy != 0 {
            ts.push(if dy > 0 { (-oy, dy) } else { (oy, -dy) });
        }
        ts.iter().any(|&(n, d)| (0..=d).contains(&n) && (ox * d + n * dx).abs() + (oy * d + n * dy).abs() < 8 * d) || on(c.0 + 8, c.1) || on(c.0, c.1 + 8)
    };
    let end = (i128::from(b.0), i128::from(b.1));
    let mut v = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let c = (i128::from(x) * 16, i128::from(y) * 16);
            if crosses(c) && !inside(end, c) {
                v.push((x as i32, y as i32));
            }
        }
    }
    Some(v)
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
    /// a line between 28.4 points with the pen (GDI.LINE.GIQ)
    pub fn line_fix(&mut self, a: Fix28, b: Fix28) -> Res<()> {
        if let Some(v) = self.pen_value()? {
            let Some(px) = giq_pixels(a, b) else { return Err(Fail::Bad) };
            for (x, y) in px {
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
        if t0 == "rectangle" && sl.ext.map.world != super::gdixform::IDENTITY && (sl.ext.map.world[1] != 0.0 || sl.ext.map.world[2] != 0.0) {
            // MEASURED (xform draw-*): the parallelogram, corners through the world in 28.4,
            // filled by the 28.4 path rule and outlined by GIQ lines (GDI.MAP.DRAWADV)
            world_pen_thin(sl.ext.map.world, &sl.ext.pen)?;
            hit(clause::GDI_MAP_DRAWADV);
            let p = points(op, 2)?;
            if p.len() != 2 {
                return Err(Fail::Bad);
            }
            let m = &sl.ext.map;
            let mut q = Vec::new();
            for c in [(p[0].0, p[0].1), (p[1].0, p[0].1), (p[1].0, p[1].1), (p[0].0, p[1].1)] {
                q.push(m.world_to_fix(c)?);
            }
            let r = super::rgn::fix_polygon_pixels(&[q.iter().map(|v| (v.0 as i32, v.1 as i32)).collect()], false);
            for b in r.rects() {
                sl.fill_shape(b.0, b.1, b.2, b.3)?;
            }
            for i in 0..4 {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                sl.line_fix(Fix28(a.0, a.1), Fix28(b.0, b.1))?;
            }
            out.push("ret 1".into());
            return Ok(true);
        }
        if t0 == "lineto" && sl.ext.map.world != super::gdixform::IDENTITY {
            // MEASURED (xform fraclineadv-*, 32 of 32 cosmetic, 16 of 16 width 1): a line
            // through a GM_ADVANCED world transform keeps its ends in 28.4 (GDI.LINE.GIQ); a pen
            // of width 1 stays a cosmetic one while the transform stretches no direction past
            // about 1.41 (√2 measured thin, 1.5 wide), past 1.5 it is a wide pen (GDI.MAP.DRAWADV)
            let to = (n(op, 2)? as i32, n(op, 3)? as i32);
            world_pen_thin(sl.ext.map.world, &sl.ext.pen)?;
            hit(clause::GDI_MAP_DRAWADV);
            let m = &sl.ext.map;
            let (p0, p1) = (m.world_to_fix(pos)?, m.world_to_fix(to)?);
            sl.line_fix(Fix28(p0.0, p0.1), Fix28(p1.0, p1.1))?;
            sl.ext.pos = to;
            out.push("ret 1".into());
            return Ok(true);
        }
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
        "polybezier" | "polybezierto" => {
            let pts = points(op, 2)?;
            let to = t0 == "polybezierto";
            let n = if to { pts.len() } else { pts.len().saturating_sub(1) };
            if pts.is_empty() || n % 3 != 0 {
                return Err(Fail::Bad);
            }
            // MEASURED (lines poly-*, 14 of 14): each curve flattened in 28.4 as a path is
            // (GDI.PATH.FLATTENHFD), the points joined by 28.4 lines (GDI.LINE.GIQ)
            hit(clause::GDI_LINE_BEZIER);
            let mut at = if to { sl.ext.pos } else { pts[0] };
            for c in pts[pts.len() - n..].chunks(3) {
                let q = [at, c[0], c[1], c[2]].map(|v| (v.0 * 16, v.1 * 16));
                let Some(f) = super::bezier::flatten(q) else { return Err(Fail::Unmodeled(open::GDI_PATH_CURVE)) };
                let mut prev = Fix28(i64::from(q[0].0), i64::from(q[0].1));
                for v in f {
                    let r = Fix28(i64::from(v.0), i64::from(v.1));
                    sl.line_fix(prev, r)?;
                    prev = r;
                }
                at = c[2];
            }
            if to {
                sl.ext.pos = at;
            }
            out.push("ret 1".into());
        }
        _ => return Err(Fail::Bad),
    }
    Ok(true)
}

/// A pen of width 1 under a GM_ADVANCED world stays cosmetic only while the transform
/// stretches no direction past about √2 (largest singular value; √|det| ≥ 1.5 is wide)
fn world_pen_thin(w: [f32; 6], pen: &Pen) -> Res<()> {
    let (a, b, c, d) = (f64::from(w[0]), f64::from(w[1]), f64::from(w[2]), f64::from(w[3]));
    let t = a * a + b * b + c * c + d * d;
    let det = (a * d - b * c).abs();
    let smax = ((t + (t * t - 4.0 * det * det).max(0.0).sqrt()) / 2.0).sqrt();
    match pen {
        Pen::Plain { width: 1, .. } if det.sqrt() >= 1.5 => Err(Fail::Unmodeled(open::GDI_PEN_WIDE)),
        Pen::Plain { width: 1, .. } if smax > 1.4143 => Err(Fail::Unmodeled(open::GDI_MAP_DRAW)),
        _ => Ok(()),
    }
}
