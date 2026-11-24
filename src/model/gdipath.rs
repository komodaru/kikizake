//! The model of runner/gdipath.c's ops (spec/gdipath.txt): the path bracket, GetPath, paths of
//! lines, rectangles, ellipses and Béziers, FlattenPath, StrokePath, FillPath & co.
//!
//! Sources (Microsoft Learn): "BeginPath", "EndPath", "AbortPath", "CloseFigure", "GetPath",
//! "FlattenPath", "WidenPath", "StrokePath", "FillPath", "StrokeAndFillPath", "PathToRegion",
//! "SelectClipPath", "SetMiterLimit", "Paths" (which functions a path records).

use crate::spec::{clause, open};
use super::gdiline::Fix28;
use super::gdi::{Gdi, Slot};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

/// a coordinate in 28.4 fixed point, as GDI keeps a path's points
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fix(pub i32);

impl Fix {
    pub fn int(v: i32) -> Fix {
        Fix(v.saturating_mul(16))
    }
    /// GetPath's integer: the nearest, halves up (MEASURED, GDI.PATH.CURVEPOINTS), past
    /// 32 bits for a point within 8 of 2^31 (path ellipsewrap-19: 2^27)
    pub fn round(self) -> i32 {
        // ((x + 8) >> 4 lies in -2^27 ..= 2^27 for every 32-bit x)
        i32::try_from(i64::from(self.0).saturating_add(8) >> 4).unwrap_or(i32::MAX)
    }
    /// the integer below: the logical current position after AngleArc (MEASURED, golden path
    /// anglearc-*)
    pub fn floor(self) -> i32 {
        self.0 >> 4
    }
    /// the integer it is, when it is one
    pub fn exact(self) -> Option<i32> {
        (self.0 & 15 == 0).then_some(self.0 >> 4)
    }
}

/// what a path point is (GetPath's PT_MOVETO 6, PT_LINETO 2, PT_BEZIERTO 4)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Move,
    Line,
    Bezier,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pt {
    pub x: Fix,
    pub y: Fix,
    pub kind: Kind,
    /// PT_CLOSEFIGURE
    pub close: bool,
}

impl Pt {
    fn int(at: (i32, i32), kind: Kind) -> Pt {
        Pt { x: Fix::int(at.0), y: Fix::int(at.1), kind, close: false }
    }
    fn fix(at: (i32, i32), kind: Kind) -> Pt {
        Pt { x: Fix(at.0), y: Fix(at.1), kind, close: false }
    }
    /// GetPath's type byte
    fn ty(&self) -> u8 {
        let t = match self.kind {
            Kind::Move => 6,
            Kind::Line => 2,
            Kind::Bezier => 4,
        };
        t | u8::from(self.close)
    }
    fn at(&self) -> (i32, i32) {
        (self.x.0, self.y.0)
    }
}

/// a DC's path: none, open (being recorded) or closed (EndPath done)
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Path {
    #[default]
    None,
    Open(Vec<Pt>),
    Closed(Vec<Pt>),
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// AngleArc's x, y, radius (a DWORD), start and sweep (float, as strtod reads them)
pub fn angle_args(op: &Op) -> Res<(i32, i32, u32, f32, f32)> {
    let [_, _, x, y, r, a, b] = &op.tok[..] else { return Err(Fail::Bad) };
    let int = |t: &String| num(t).and_then(|v| i32::try_from(v).ok()).ok_or(Fail::Bad);
    let float = |t: &String| t.parse::<f64>().map(|v| v as f32).map_err(|_| Fail::Bad);
    Ok((int(x)?, int(y)?, int(r)? as u32, float(a)?, float(b)?))
}

/// a line op of gdiline.c while a path is open: its points go into the path, nothing is drawn
pub fn record(sl: &mut Slot, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let Path::Open(p) = &mut sl.ext.path else { return Ok(false) };
    let t0 = op.tok[0].as_str();
    if t0 == "anglearc" {
        let (x, y, r, st, sw) = angle_args(op)?;
        let Some(arc) = super::gdiarc::angle_arc(x, y, r, st, sw) else {
            // (a radius or coordinates out of GDI's range: not measured)
            return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
        };
        hit(clause::GDI_PATH_RECORD);
        hit(clause::GDI_PATH_ANGLEARC);
        if p.last().is_none_or(|l| l.close) {
            p.push(Pt::int(sl.ext.pos, Kind::Move));
        }
        p.push(Pt::fix(arc.start, Kind::Line));
        for g in &arc.beziers {
            for q in g {
                p.push(Pt::fix(*q, Kind::Bezier));
            }
        }
        let last = p.last().ok_or(Fail::Bad)?;
        sl.ext.pos = (last.x.floor(), last.y.floor());
        out.push("ret 1".into());
        return Ok(true);
    }
    let pts: Vec<(i32, i32)> = op.tok[2.min(op.tok.len())..].chunks(2).map(|c| Ok((num(&c[0]).ok_or(Fail::Bad)? as i32, num(c.get(1).ok_or(Fail::Bad)?).ok_or(Fail::Bad)? as i32))).collect::<Res<_>>()?;
    hit(clause::GDI_PATH_RECORD);
    // (a figure starts at the current position when the path has no open figure there)
    let start = |p: &mut Vec<Pt>, pos: (i32, i32)| {
        if p.last().is_none_or(|l| l.close) {
            p.push(Pt::int(pos, Kind::Move));
        }
    };
    let close = |p: &mut Vec<Pt>| {
        if let Some(l) = p.last_mut() {
            l.close = true;
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
                p.push(Pt::int(q, Kind::Line));
            }
            sl.ext.pos = *pts.last().ok_or(Fail::Bad)?;
            out.push("ret 1".into());
        }
        "polyline" | "polygon" => {
            if pts.len() < 2 {
                // (as outside a path: fails, nothing recorded)
                hit(clause::GDI_PATH_POLYSHORT);
                out.push("ret 0".into());
                return Ok(true);
            }
            p.push(Pt::int(pts[0], Kind::Move));
            for &q in &pts[1..] {
                p.push(Pt::int(q, Kind::Line));
            }
            if t0 == "polygon" {
                close(p);
            }
            out.push("ret 1".into());
        }
        "polybezier" | "polybezierto" => {
            let to = t0 == "polybezierto";
            let n = if to { pts.len() } else { pts.len().saturating_sub(1) };
            if pts.is_empty() || n % 3 != 0 {
                return Err(Fail::Bad);
            }
            // MEASURED: PolyBezier begins a figure at its first point (replacing a lone
            // MoveToEx), PolyBezierTo continues from the current position; the points are kept
            // as given, PT_BEZIERTO each
            hit(clause::GDI_PATH_BEZIER);
            if to {
                start(p, sl.ext.pos);
            } else {
                record_move(p, pts[0])?;
            }
            for &q in &pts[pts.len() - n..] {
                p.push(Pt::int(q, Kind::Bezier));
            }
            if to {
                sl.ext.pos = *pts.last().ok_or(Fail::Bad)?;
            }
            out.push("ret 1".into());
        }
        "rectangle" | "ellipse" | "roundrect" | "arc" | "arcto" | "pie" | "chord"
            if matches!(pts.as_slice(), [a, b, ..] if a.0 == b.0 || a.1 == b.1) => {
            let expected = match t0 { "rectangle" | "ellipse" => 2, "roundrect" => 3, _ => 4 };
            if pts.len() != expected { return Err(Fail::Bad); }
            // Both arc directions and reversed endpoints leave even an open figure intact.
            hit(clause::GDI_PATH_EMPTYBOX);
            out.push("ret 1".into());
        }
        "rectangle" | "roundrect" if t0 == "rectangle" || (pts.len() == 3 && (pts[2].0 == 0 || pts[2].1 == 0)) => {
            // (RoundRect with a corner 0 wide or high is Rectangle: MEASURED, arcpath-25/48/90)
            hit(if t0 == "rectangle" { clause::GDI_PATH_RECT } else { clause::GDI_PATH_ROUNDFLAT });
            let (l, r) = (pts[0].0.min(pts[1].0), pts[0].0.max(pts[1].0));
            let (t, b) = (pts[0].1.min(pts[1].1), pts[0].1.max(pts[1].1));
            // (GM_COMPATIBLE: the right and bottom edges excluded)
            let (r, b) = (r - 1, b - 1);
            let c = if sl.ext.arcdir == 2 { [(r, b), (l, b), (l, t), (r, t)] } else { [(r, t), (l, t), (l, b), (r, b)] };
            for (i, q) in c.iter().enumerate() {
                p.push(Pt::int(*q, if i == 0 { Kind::Move } else { Kind::Line }));
            }
            close(p);
            out.push("ret 1".into());
        }
        "ellipse" => {
            let [mut a, mut b] = pts[..] else { return Err(Fail::Bad) };
            if sl.ext.map.path_sixteenths() {
                hit(clause::GDI_PATH_ELLIPSEMAP);
                a = (sixteenths_to_pixel(a.0), sixteenths_to_pixel(a.1));
                b = (sixteenths_to_pixel(b.0), sixteenths_to_pixel(b.1));
            }
            let (l, r) = (a.0.min(b.0), a.0.max(b.0));
            let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
            // MEASURED: four Béziers in 28.4 around (l, t) - (r - 1, b - 1), from the right
            // end of the middle row, counterclockwise (upwards) or, with AD_CLOCKWISE,
            // clockwise; each control point the radius times 1 - 4/3 (sqrt 2 - 1) in from the
            // box's edge, that inset rounded down across, and down the box rounded up
            // counterclockwise, down clockwise
            hit(clause::GDI_PATH_ELLIPSE);
            // (a box whose 32-bit 28.4 width is 0 or -2^31 records nothing, the call succeeds)
            if let Some(c) = ellipse_points(l, t, r, bt, sl.ext.arcdir == 2) {
                p.push(Pt::fix(c[0], Kind::Move));
                for q in &c[1..] {
                    p.push(Pt::fix(*q, Kind::Bezier));
                }
                close(p);
            }
            out.push("ret 1".into());
        }
        "roundrect" => {
            let [a, b, wh] = pts[..] else { return Err(Fail::Bad) };
            if wh.0 < 0 || wh.1 < 0 {
                // (a corner of negative size: not measured)
                return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
            }
            hit(clause::GDI_PATH_ROUNDRECT);
            let (l, r) = (a.0.min(b.0), a.0.max(b.0));
            let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
            let c = round_rect_points((l, t, r, bt), wh, sl.ext.arcdir == 2);
            for (i, q) in c.iter().enumerate() {
                p.push(Pt::fix(*q, if i == 0 { Kind::Move } else if i % 4 == 0 { Kind::Line } else { Kind::Bezier }));
            }
            close(p);
            out.push("ret 1".into());
        }
        "arc" | "arcto" | "pie" | "chord" => {
            let [a, b, rs, re] = pts[..] else { return Err(Fail::Bad) };
            let (l, r) = (a.0.min(b.0), a.0.max(b.0));
            let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
            hit(clause::GDI_PATH_ARC);
            let Some(arc) = super::gdiarc::arc((l, t, r, bt), rs, re, sl.ext.arcdir == 2) else {
                if t0 != "arc" {
                    // Degenerate transformed axes of a nonempty logical box.
                    return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
                }
                // MEASURED (curve-24, curve-30): an empty box records nothing, the call succeeds
                out.push("ret 1".into());
                return Ok(true);
            };
            if t0 == "arcto" {
                start(p, sl.ext.pos);
                p.push(Pt::fix(arc.start, Kind::Line));
            } else {
                // (a new figure, replacing a lone MoveToEx as PolyBezier does)
                if p.last().is_some_and(|l| l.kind == Kind::Move) {
                    p.pop();
                }
                p.push(Pt::fix(arc.start, Kind::Move));
            }
            for g in &arc.beziers {
                for q in g {
                    p.push(Pt::fix(*q, Kind::Bezier));
                }
            }
            match t0 {
                "pie" => {
                    p.push(Pt::fix(arc.middle, Kind::Line));
                    close(p);
                }
                "chord" => close(p),
                "arcto" => {
                    let last = p.last().ok_or(Fail::Bad)?;
                    sl.ext.pos = (last.x.round(), last.y.round());
                }
                _ => {}
            }
            out.push("ret 1".into());
        }
        _ => return Err(Fail::Unmodeled(open::GDI_PATH_CURVE)),
    }
    Ok(true)
}

/// StrokePath's lines (MEASURED): the flattened path's lines in 28.4 (GDI.LINE.GIQ; whole points
/// give the integer lines), each figure closed where PT_CLOSEFIGURE says
fn stroke(sl: &mut Slot, p: &[Pt]) -> Res<()> {
    let fix = |q: &Pt| Fix28(i64::from(q.x.0), i64::from(q.y.0));
    let mut fig_start = Fix28(0, 0);
    let mut prev = Fix28(0, 0);
    for q in &flatten(p)? {
        let at = fix(q);
        match q.kind {
            Kind::Move => fig_start = at,
            Kind::Bezier => return Err(Fail::Bad),
            Kind::Line => {
                sl.line_fix(prev, at)?;
                if q.close {
                    sl.line_fix(at, fig_start)?;
                }
            }
        }
        prev = at;
    }
    Ok(())
}

/// Ellipse outside a path (MEASURED, GDI.SHAPE.ELLIPSEPEN, lines ellipse-frame-* 169 of 169,
/// ellipse-both-* 169 of 169): the path's Ellipse (counterclockwise), with the null brush
/// stroked as StrokePath strokes it, else filled and outlined as StrokeAndFillPath does
pub fn ellipse_shape(sl: &mut Slot, a: (i32, i32), b: (i32, i32), filled: bool) -> Res<()> {
    let (l, r) = (a.0.min(b.0), a.0.max(b.0));
    let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
    hit(clause::GDI_SHAPE_ELLIPSEPEN);
    let Some(c) = ellipse_points(l, t, r, bt, false) else {
        // A zero device extent or exactly half the 28.4 period produces no figure.
        return Ok(());
    };
    // Wrapping a logical endpoint by 2^28 can put the whole curve back on the DIB.
    // Check the resulting control points before handing it to the 32-bit flattener.
    if c.iter().any(|&(x, y)| !(-(1 << 30)..1 << 30).contains(&x) || !(-(1 << 30)..1 << 30).contains(&y)) {
        return Err(Fail::Unmodeled(open::GDI_SHAPE_CURVE));
    }
    let mut p = vec![Pt::fix(c[0], Kind::Move)];
    for q in &c[1..] {
        p.push(Pt::fix(*q, Kind::Bezier));
    }
    if let Some(last) = p.last_mut() {
        last.close = true;
    }
    draw_shape(sl, &p, filled)
}

/// RoundRect outside a path (GDI.SHAPE.ROUNDPEN): the path's RoundRect (counterclockwise)
/// stroked, or filled and outlined, as ellipse_shape does
pub fn round_rect_shape(sl: &mut Slot, a: (i32, i32), b: (i32, i32), wh: (i32, i32), filled: bool) -> Res<()> {
    let (l, r) = (a.0.min(b.0), a.0.max(b.0));
    let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
    if r == l || bt == t || wh.0 <= 0 || wh.1 <= 0 || a.0 > b.0 || a.1 > b.1 {
        // (empty or inverted boxes, flat or negative corners: not measured outside a path)
        return Err(Fail::Unmodeled(open::GDI_SHAPE_CURVE));
    }
    hit(clause::GDI_SHAPE_ROUNDPEN);
    let c = round_rect_points((l, t, r, bt), wh, false);
    let mut p: Vec<Pt> = c.iter().enumerate().map(|(i, q)| Pt::fix(*q, if i == 0 { Kind::Move } else if i % 4 == 0 { Kind::Line } else { Kind::Bezier })).collect();
    if let Some(last) = p.last_mut() {
        last.close = true;
    }
    draw_shape(sl, &p, filled)
}

fn draw_shape(sl: &mut Slot, p: &[Pt], filled: bool) -> Res<()> {
    if !filled {
        return stroke(sl, p);
    }
    let figs = figures(p)?;
    for q in super::rgn::fix_polygon_pixels(&figs, sl.ext.polyfill == 2).rects() {
        sl.fill_shape(q.0, q.1, q.2, q.3)?;
    }
    for f in &figs {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            sl.line_fix(Fix28(i64::from(a.0), i64::from(a.1)), Fix28(i64::from(b.0), i64::from(b.1)))?;
        }
    }
    Ok(())
}

/// v (7 - 4 sqrt 2) / q rounded down (v >= 0, q > 0): the inset of a Bézier's control point,
/// 1 - 4/3 (sqrt 2 - 1) = (7 - 4 sqrt 2) / 3 of a radius, exactly (MEASURED, GDI.PATH.ELLIPSE:
/// the irrational's floor at every size, path ellipsebig-* ellipsebigpx-*, where a decimal
/// 0.5522847498 floors apart from 82800 pixels on). With s = sqrt(32 v^2), irrational for v > 0,
/// and N = 7 v - isqrt(32 v^2): 7 v - s lies in (N - 1, N), which holds no multiple of q, so its
/// floor over q is (N - 1)'s.
pub fn inset_floor(v: i32, q: i32) -> i32 {
    if v <= 0 || q <= 0 {
        return 0;
    }
    let v = i128::from(v);
    let n = v.saturating_mul(7).saturating_sub(v.saturating_mul(v).saturating_mul(32).isqrt());
    i32::try_from(n.saturating_sub(1).div_euclid(i128::from(q))).unwrap_or(i32::MAX)
}

/// rounded up: the floor plus one (the value irrational) but at 0
pub fn inset_ceil(v: i32, q: i32) -> i32 {
    inset_floor(v, q).saturating_add(i32::from(v > 0))
}

/// the inset of a side of either sign (a box's 32-bit width past 2^31 comes out negative):
/// v (7 - 4 sqrt 2) / 6 rounded down, or up
fn signed_inset(v: i32, up: bool) -> i32 {
    let floor = if v >= 0 { inset_floor(v, 6) } else { inset_floor(v.saturating_neg(), 6).saturating_add(1).saturating_neg() };
    floor.saturating_add(i32::from(up && v != 0))
}

/// Round an integer to binary32 precision. The largest positive result is 2^31.
fn single_precision_integer(v: i32) -> i64 {
    let shift = v.unsigned_abs().checked_ilog2().unwrap_or(0).saturating_sub(23);
    let step = 1_i64 << shift;
    let magnitude = i64::from(v.unsigned_abs());
    let q = magnitude / step;
    let remainder = magnitude % step;
    let up = remainder.saturating_mul(2) > step || (remainder.saturating_mul(2) == step && q % 2 == 1);
    let rounded = q.saturating_add(i64::from(up)).saturating_mul(step);
    if v < 0 { rounded.saturating_neg() } else { rounded }
}

/// GM_COMPATIBLE first rounds both corners to whole device pixels, halves up.
pub fn sixteenths_to_pixel(v: i32) -> i32 {
    i32::try_from(single_precision_integer(v).saturating_add(8).div_euclid(16))
        .unwrap_or(i32::MAX)
}

/// GetPath converts a device 28.4 component back to the logical LONG.
pub fn sixteenths_to_logical(v: i32) -> i32 {
    i32::try_from(single_precision_integer(v)).unwrap_or(i32::MIN)
}

/// one axis of Ellipse's box in 28.4 (MEASURED, GDI.PATH.ELLIPSE, path ellipsewrap-*
/// ellipsebigpx-* ellipseedge-*): the ordered ends times 16 to 32 bits and their width W = 16 hi
/// - 16 lo to 32 bits; the box (16 lo, 16 hi - 16) where W > 0, (16 lo - 16, 16 hi) where W < 0;
/// None (no figure) where W is 0 or -2^31
fn ellipse_axis(lo: i32, hi: i32) -> Option<(i32, i32)> {
    let (a, b) = (lo.wrapping_mul(16), hi.wrapping_mul(16));
    let w = b.wrapping_sub(a);
    match w {
        0 | i32::MIN => None,
        w if w > 0 => Some((a, b.wrapping_sub(16))),
        _ => Some((a.wrapping_sub(16), b)),
    }
}

/// Ellipse's 13 points in 28.4 (GDI.PATH.ELLIPSE): four Béziers around (l, t) - (r - 1, b - 1)
/// (l <= r, t <= b) from the right end of the middle row, counterclockwise or (cw) clockwise;
/// GDI's arithmetic is 32-bit: a box past the 28.4 range wraps (ellipse_axis), the centre the
/// near side plus half the side (truncated); None where the box records no figure
pub fn ellipse_points(l: i32, t: i32, r: i32, bt: i32, cw: bool) -> Option<[(i32, i32); 13]> {
    let ((fl, fr), (ft, fb)) = (ellipse_axis(l, r)?, ellipse_axis(t, bt)?);
    let (vx, vy) = (fr.wrapping_sub(fl), fb.wrapping_sub(ft));
    let (cx, cy) = (fl.wrapping_add(vx / 2), ft.wrapping_add(vy / 2));
    // (half the axis times the factor: the whole axis over 6)
    let (dx, dy) = (signed_inset(vx, false), signed_inset(vy, !cw));
    let (xp, xm, ym, yp) = (fr.wrapping_sub(dx), fl.wrapping_add(dx), ft.wrapping_add(dy), fb.wrapping_sub(dy));
    let mut c = [(fr, cy), (fr, ym), (xp, ft), (cx, ft), (xm, ft), (fl, ym), (fl, cy), (fl, yp), (xm, fb), (cx, fb), (xp, fb), (fr, yp), (fr, cy)];
    if cw {
        for q in c.iter_mut() {
            q.1 = ft.wrapping_add(fb).wrapping_sub(q.1);
        }
    }
    Some(c)
}

/// RoundRect's 16 points in 28.4 (MEASURED, GDI.PATH.ROUNDRECT, path rrgrid-* arcpath-* curve-*
/// 240 of 240 GetPath): the box (l, t) - (r - 1, b - 1) as Ellipse's; each corner's radius its
/// width (height) times 8, shrunk as the box is, (W - 1) / W, to the nearest 1/16 pixel (halves
/// up), at most half the box; from the right side's upper end counterclockwise (the lower end
/// clockwise, mirrored), each corner a quarter of Ellipse's (its control points the radius
/// times 1 - 4/3 (sqrt 2 - 1) in, rounded as Ellipse rounds them), the sides lines
pub fn round_rect_points(bx: (i32, i32, i32, i32), wh: (i32, i32), cw: bool) -> [(i32, i32); 16] {
    let (l, t, r, bt) = bx;
    let (fl, fr, ft, fb) = (l * 16, (r - 1) * 16, t * 16, (bt - 1) * 16);
    // (d * 8 * (n - 1) / n, halves up)
    let radius = |d: i32, n: i32, half: i32| -> i32 {
        let (d, n) = (i64::from(d), i64::from(n.max(1)));
        let v = (2 * d * 8 * (n - 1) + n).div_euclid(2 * n);
        i32::try_from(v).unwrap_or(i32::MAX).min(half)
    };
    let (rx, ry) = (radius(wh.0, r - l, (fr - fl) / 2), radius(wh.1, bt - t, (fb - ft) / 2));
    let ix = inset_floor(rx, 3);
    let iy = if cw { inset_floor(ry, 3) } else { inset_ceil(ry, 3) };
    let mut c = [(fr, ft + ry), (fr, ft + iy), (fr - ix, ft), (fr - rx, ft), (fl + rx, ft), (fl + ix, ft), (fl, ft + iy), (fl, ft + ry), (fl, fb - ry), (fl, fb - iy), (fl + ix, fb), (fl + rx, fb), (fr - rx, fb), (fr - ix, fb), (fr, fb - iy), (fr, fb - ry)];
    if cw {
        for q in c.iter_mut() {
            q.1 = ft + fb - q.1;
        }
    }
    c
}

/// MoveToEx in a path: a new figure at the point (a lone PT_MOVETO is replaced by the next)
fn record_move(p: &mut Vec<Pt>, at: (i32, i32)) -> Res<bool> {
    hit(clause::GDI_PATH_MOVETO);
    if p.last().is_some_and(|l| l.kind == Kind::Move) {
        p.pop();
    }
    p.push(Pt::int(at, Kind::Move));
    Ok(true)
}

/// FlattenPath: each Bézier's points replaced by its lines (GDI.PATH.FLATTENHFD), a closed
/// Bézier's last line keeping PT_CLOSEFIGURE
fn flatten(p: &[Pt]) -> Res<Vec<Pt>> {
    let mut o: Vec<Pt> = Vec::with_capacity(p.len());
    let mut i = 0;
    while i < p.len() {
        let q = p[i];
        if q.kind != Kind::Bezier {
            o.push(q);
            i += 1;
            continue;
        }
        // (from the Bézier's own start point, not the flattened one before)
        let (Some(from), Some(c)) = (i.checked_sub(1).and_then(|j| p.get(j)), p.get(i..i + 3)) else { return Err(Fail::Bad) };
        if c.iter().any(|q| q.kind != Kind::Bezier) {
            return Err(Fail::Bad);
        }
        hit(clause::GDI_PATH_FLATTENHFD);
        let Some(line) = super::bezier::flatten([from.at(), c[0].at(), c[1].at(), c[2].at()]) else {
            // (a curve too large for the 32 bit flattener: GDI's 64 bit one, open)
            return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
        };
        let n = line.len();
        for (j, at) in line.into_iter().enumerate() {
            let mut q = Pt::fix(at, Kind::Line);
            q.close = j + 1 == n && c[2].close;
            o.push(q);
        }
        i += 3;
    }
    Ok(o)
}

/// a 28.4 point that is a whole pixel; a fractional one (a flattened curve) stroked is open
fn whole(at: (i32, i32)) -> Res<(i32, i32)> {
    match (Fix(at.0).exact(), Fix(at.1).exact()) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(Fail::Unmodeled(open::GDI_PATH_CURVE)),
    }
}

/// the figures of a closed path (each from its PT_MOVETO), curves flattened, in 28.4
fn figures(p: &[Pt]) -> Res<Vec<Vec<(i32, i32)>>> {
    let mut figs: Vec<Vec<(i32, i32)>> = Vec::new();
    for q in flatten(p)? {
        match q.kind {
            Kind::Move => figs.push(vec![q.at()]),
            Kind::Line | Kind::Bezier => figs.last_mut().ok_or(Fail::Bad)?.push(q.at()),
        }
    }
    Ok(figs)
}

crate::model::words! {
    /// the path commands
    enum PathOp { Beginpath = "beginpath", Endpath = "endpath", Abortpath = "abortpath", Closefig = "closefig", Flatten = "flatten", Widen = "widen", Strokepath = "strokepath", Fillpath = "fillpath", Strokefill = "strokefill", Selclippath = "selclippath", Getpath = "getpath", Pathrgn = "pathrgn", Miter = "miter" }
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    let Some(cmd) = PathOp::parse(t0) else {
        return Ok(false);
    };
    let k = n(op, 1)?;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
    let sixteenths = sl.ext.map.path_sixteenths();
    if sl.ext.mapped() && !(sixteenths && matches!(cmd, PathOp::Beginpath | PathOp::Endpath | PathOp::Getpath | PathOp::Abortpath)) {
        return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
    }
    let path = &mut sl.ext.path;
    let ok = match cmd {
        PathOp::Miter => {
            let v: f32 = op.tok.get(2).and_then(|t| t.parse().ok()).ok_or(Fail::Bad)?;
            // MEASURED: a limit below 1 fails and leaves the old value unwritten; the limit is kept
            // as given (10 at first)
            hit(clause::GDI_PATH_MITER);
            if v < 1.0 {
                out.push("miter 0 -1000".into());
            } else {
                out.push(format!("miter 1 {}", (std::mem::replace(&mut sl.ext.miter, v) * 1000.0) as i32));
            }
            return Ok(true);
        }
        PathOp::Beginpath => {
            hit(clause::GDI_PATH_BEGIN);
            if matches!(path, Path::Open(_)) {
                // MEASURED: the points so far are dropped, a new path begins
                hit(clause::GDI_PATH_BEGINAGAIN);
            }
            *path = Path::Open(Vec::new());
            true
        }
        PathOp::Endpath => {
            hit(clause::GDI_PATH_END);
            match std::mem::take(path) {
                Path::Open(mut p) => {
                    // MEASURED: a MoveToEx last in the path starts no figure and is dropped
                    if p.last().is_some_and(|l| l.kind == Kind::Move) {
                        hit(clause::GDI_PATH_ENDMOVE);
                        p.pop();
                    }
                    *path = Path::Closed(p);
                    true
                }
                other => {
                    *path = other;
                    false
                }
            }
        }
        PathOp::Abortpath => {
            hit(clause::GDI_PATH_ABORT);
            *path = Path::None;
            true
        }
        PathOp::Closefig => {
            hit(clause::GDI_PATH_CLOSE);
            match path {
                Path::Open(p) if p.last().is_some_and(|l| l.kind != Kind::Move) => {
                    if let Some(l) = p.last_mut() {
                        l.close = true;
                    }
                    true
                }
                Path::Open(_) => {
                    // MEASURED: no figure (or a lone MoveToEx): succeeds, nothing changes
                    hit(clause::GDI_PATH_CLOSEEMPTY);
                    true
                }
                _ => false,
            }
        }
        PathOp::Getpath => {
            hit(clause::GDI_PATH_GET);
            match path {
                Path::Closed(p) => {
                    out.push(format!("path {}", p.len()));
                    for q in p.iter() {
                        let logical = |v: Fix| {
                            if sixteenths {
                                // A rounded positive 2^31 has LONG's 0x80000000 bit pattern.
                                sixteenths_to_logical(v.0)
                            } else { v.round() }
                        };
                        out.push(format!("pt {} {} {}", logical(q.x), logical(q.y), q.ty()));
                    }
                }
                _ => out.push("path -1".into()),
            }
            return Ok(true);
        }
        PathOp::Flatten => {
            hit(clause::GDI_PATH_FLATTEN);
            match path {
                Path::Closed(p) => {
                    *p = flatten(p)?;
                    true
                }
                _ => false,
            }
        }
        PathOp::Strokepath => {
            hit(clause::GDI_PATH_STROKE);
            let Path::Closed(p) = std::mem::take(path) else {
                out.push("strokepath 0".into());
                return Ok(true);
            };
            stroke(sl, &p)?;
            true
        }
        PathOp::Fillpath | PathOp::Strokefill | PathOp::Selclippath | PathOp::Pathrgn => {
            if !matches!(path, Path::Closed(_)) {
                // (no closed path: the call fails, the DC keeps what it has)
                hit(clause::GDI_PATH_NOPATH);
                out.push(if t0 == "pathrgn" { "pathrgn 0".into() } else { format!("{t0} 0") });
                return Ok(true);
            }
            let Path::Closed(p) = std::mem::take(path) else { unreachable!() };
            let figs = figures(&p)?;
            if figs.is_empty() {
                // The drawing calls consume an empty closed path successfully. Region conversion fails.
                hit(clause::GDI_PATH_EMPTYFILL);
                let drawn = matches!(cmd, PathOp::Fillpath | PathOp::Strokefill);
                out.push(format!("{t0} {}", u8::from(drawn)));
                return Ok(true);
            }
            // MEASURED: the interior is CreatePolygonRgn's region of the figures (each closed) in
            // the DC's fill mode (GDI.RGN.POLYSCAN); the call discards the path
            hit(clause::GDI_PATH_FILL);
            let ints: Option<Vec<Vec<(i32, i32)>>> = figs.iter().map(|f| f.iter().map(|&q| whole(q).ok()).collect()).collect();
            let r = match &ints {
                // (PathToRegion's bound is CreatePolygonRgn's, loose too: golden path loose-41)
                Some(f) if t0 == "pathrgn" || t0 == "selclippath" => super::rgn::polygon(f, sl.ext.polyfill == 2)?,
                Some(f) => super::rgn::polygon_pixels(f, sl.ext.polyfill == 2),
                None => {
                    hit(clause::GDI_PATH_FILLFIX);
                    super::rgn::fix_polygon_pixels(&figs, sl.ext.polyfill == 2)
                }
            };
            match t0 {
                "pathrgn" => {
                    let i = n(op, 2)?;
                    *g.rgns.get_mut(i as usize).ok_or(Fail::Bad)? = Some(r);
                    out.push("pathrgn 1".into());
                    return Ok(true);
                }
                "selclippath" => {
                    let m = n(op, 2)?;
                    let ok = super::gdirgn::select_clip(sl, Some(r), m).is_some();
                    out.push(format!("selclippath {}", ok as u8));
                    return Ok(true);
                }
                _ => {}
            }
            for q in r.rects() {
                sl.fill_shape(q.0, q.1, q.2, q.3)?;
            }
            if t0 == "strokefill" {
                // (StrokeAndFillPath: the outline of every figure, closed)
                let Some(figs) = ints else {
                    // (between pixels: each figure's closed outline in 28.4, GDI.LINE.GIQ, as
                    // StrokePath draws a curve's lines)
                    hit(clause::GDI_PATH_STROKEFILLFIX);
                    for f in &figs {
                        for i in 0..f.len() {
                            let (a, b) = (f[i], f[(i + 1) % f.len()]);
                            sl.line_fix(Fix28(i64::from(a.0), i64::from(a.1)), Fix28(i64::from(b.0), i64::from(b.1)))?;
                        }
                    }
                    out.push(format!("{t0} 1"));
                    return Ok(true);
                };
                for f in &figs {
                    for i in 0..f.len() {
                        sl.line(f[i], f[(i + 1) % f.len()])?;
                    }
                }
            }
            true
        }
        PathOp::Widen => return Err(Fail::Unmodeled(open::GDI_PATH_WIDEN)),
    };
    out.push(format!("{t0} {}", ok as u8));
    Ok(true)
}
