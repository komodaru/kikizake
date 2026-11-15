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
    /// GetPath's integer: the nearest, halves up (MEASURED, GDI.PATH.CURVEPOINTS)
    pub fn round(self) -> i32 {
        self.0.saturating_add(8) >> 4
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
    /// how well the point is known
    pub known: Known,
}

/// how well a point of the path is known
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Known {
    /// to the 1/16 pixel
    Exact,
    /// only to the pixel GetPath rounds it to (a part-quarter arc's points: GDI.PATH.CURVE)
    ToThePixel,
}

impl Pt {
    fn int(at: (i32, i32), kind: Kind) -> Pt {
        Pt { x: Fix::int(at.0), y: Fix::int(at.1), kind, close: false, known: Known::Exact }
    }
    fn fix(at: (i32, i32), kind: Kind) -> Pt {
        Pt { x: Fix(at.0), y: Fix(at.1), kind, close: false, known: Known::Exact }
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

/// a line op of gdiline.c while a path is open: its points go into the path, nothing is drawn
pub fn record(sl: &mut Slot, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let Path::Open(p) = &mut sl.ext.path else { return Ok(false) };
    let t0 = op.tok[0].as_str();
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
        "roundrect" if pts.len() == 3 && (pts[2].0 == 0 || pts[2].1 == 0) && (pts[0].0 == pts[1].0 || pts[0].1 == pts[1].1) => {
            // (an empty box with a corner 0 wide or high: a figure FillPath takes, filling
            // nothing, curve-33; its points not measured)
            return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
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
        "roundrect" if pts.len() == 3 && (pts[0].0 == pts[1].0 || pts[0].1 == pts[1].1) => {
            // MEASURED (curve-158, arcpath-85): an empty box records nothing, the call succeeds
            hit(clause::GDI_PATH_ROUNDFLAT);
            out.push("ret 1".into());
        }
        "ellipse" => {
            let [a, b] = pts[..] else { return Err(Fail::Bad) };
            let (l, r) = (a.0.min(b.0), a.0.max(b.0));
            let (t, bt) = (a.1.min(b.1), a.1.max(b.1));
            if r == l || bt == t {
                // (an empty box: FillPath and StrokeAndFillPath succeed over it drawing nothing,
                // curve-57 curve-63; its figure not measured)
                return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
            }
            // MEASURED: four Béziers in 28.4 around (l, t) - (r - 1, b - 1), from the right
            // end of the middle row, counterclockwise (upwards) or, with AD_CLOCKWISE,
            // clockwise; each control point the radius times 1 - 4/3 (sqrt 2 - 1) in from the
            // box's edge, that inset rounded down across, and down the box rounded up
            // counterclockwise, down clockwise
            hit(clause::GDI_PATH_ELLIPSE);
            let c = ellipse_points(l, t, r, bt, sl.ext.arcdir == 2);
            p.push(Pt::fix(c[0], Kind::Move));
            for q in &c[1..] {
                p.push(Pt::fix(*q, Kind::Bezier));
            }
            close(p);
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
            if t0 == "arc" && (r == l || bt == t) {
                // MEASURED (curve-24, curve-30): an empty box records nothing, the call succeeds
                hit(clause::GDI_PATH_ARC);
                out.push("ret 1".into());
                return Ok(true);
            }
            if r - l < 3 || bt - t < 3 || r - l > 32 || bt - t > 32 {
                // (an arc of 2 pixels or less across: not measured yet; one more than 32 across:
                // a part quarter's points are a pixel off now and then, arcflat-*)
                return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
            }
            hit(clause::GDI_PATH_ARC);
            let segs = arc_beziers((l, t, r, bt), rs, re, sl.ext.arcdir == 2).ok_or(Fail::Bad)?;
            let first = segs.first().ok_or(Fail::Bad)?.0[0];
            if t0 == "arcto" {
                start(p, sl.ext.pos);
                p.push(Pt::fix(first, Kind::Line));
            } else {
                // (a new figure, replacing a lone MoveToEx as PolyBezier does)
                if p.last().is_some_and(|l| l.kind == Kind::Move) {
                    p.pop();
                }
                p.push(Pt::fix(first, Kind::Move));
            }
            for (g, known) in &segs {
                for q in &g[1..] {
                    let mut b = Pt::fix(*q, Kind::Bezier);
                    b.known = *known;
                    p.push(b);
                }
            }
            match t0 {
                "pie" => {
                    let c = ellipse_points(l, t, r, bt, false);
                    p.push(Pt::fix((c[3].0, c[0].1), Kind::Line));
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
    if r == l || bt == t || ((a.0 > b.0 || a.1 > b.1) && (r - l < 2 || bt - t < 2)) {
        // (empty boxes, and inverted boxes one pixel thin (random-34): not measured outside a path)
        return Err(Fail::Unmodeled(open::GDI_SHAPE_CURVE));
    }
    hit(clause::GDI_SHAPE_ELLIPSEPEN);
    let c = ellipse_points(l, t, r, bt, false);
    let mut p = vec![Pt::fix(c[0], Kind::Move)];
    for q in &c[1..] {
        p.push(Pt::fix(*q, Kind::Bezier));
    }
    if let Some(last) = p.last_mut() {
        last.close = true;
    }
    if !filled {
        return stroke(sl, &p);
    }
    let figs = figures(&p)?;
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

/// Ellipse's 13 points in 28.4 (GDI.PATH.ELLIPSE): four Béziers around (l, t) - (r - 1, b - 1)
/// from the right end of the middle row, counterclockwise or (cw) clockwise
pub fn ellipse_points(l: i32, t: i32, r: i32, bt: i32, cw: bool) -> [(i32, i32); 13] {
    let (fl, fr, ft, fb) = (l * 16, (r - 1) * 16, t * 16, (bt - 1) * 16);
    let (cx, cy) = ((fl + fr) / 2, (ft + fb) / 2);
    let inset = |rad2: i32| f64::from(rad2) * 0.5 * (1.0 - 0.552_284_749_8);
    let dx = inset(fr - fl).floor() as i32;
    let dy = if cw { inset(fb - ft).floor() } else { inset(fb - ft).ceil() } as i32;
    let (xp, xm, ym, yp) = (fr - dx, fl + dx, ft + dy, fb - dy);
    let mut c = [(fr, cy), (fr, ym), (xp, ft), (cx, ft), (xm, ft), (fl, ym), (fl, cy), (fl, yp), (xm, fb), (cx, fb), (xp, fb), (fr, yp), (fr, cy)];
    if cw {
        for q in c.iter_mut() {
            q.1 = ft + fb - q.1;
        }
    }
    c
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
    let inset = |rad: i32| f64::from(rad) * (1.0 - 0.552_284_749_8);
    let ix = inset(rx).floor() as i32;
    let iy = if cw { inset(ry).floor() } else { inset(ry).ceil() } as i32;
    let mut c = [(fr, ft + ry), (fr, ft + iy), (fr - ix, ft), (fr - rx, ft), (fl + rx, ft), (fl + ix, ft), (fl, ft + iy), (fl, ft + ry), (fl, fb - ry), (fl, fb - iy), (fl + ix, fb), (fl + rx, fb), (fr - rx, fb), (fr - ix, fb), (fr, fb - iy), (fr, fb - ry)];
    if cw {
        for q in c.iter_mut() {
            q.1 = ft + fb - q.1;
        }
    }
    c
}

/// Arc's Béziers in 28.4 (MEASURED, GDI.PATH.ARC, path curve-* 49 of 49 GetPath): the angles
/// of the radials through the logical box (l, t) - (r, b), normalized by its half sizes; the
/// arc from the start's angle to the end's (a whole turn when they are equal) the way of the
/// arc direction, cut at the quarter points; a whole quarter is Ellipse's, a part of one the
/// Bézier of that angle span on the ellipse of (l, t) - (r - 1, b - 1) in floating point
/// (control points 4/3 tan(span / 4) along the tangents), each point rounded to 28.4
pub fn arc_beziers(bx: (i32, i32, i32, i32), rs: (i32, i32), re: (i32, i32), cw: bool) -> Option<Vec<([(i32, i32); 4], Known)>> {
    use std::f64::consts::{FRAC_PI_2, TAU};
    let (l, t, r, bt) = bx;
    let c = ellipse_points(l, t, r, bt, cw);
    let (fl, fr, ft, fb) = (f64::from(l * 16), f64::from((r - 1) * 16), f64::from(t * 16), f64::from((bt - 1) * 16));
    // (counterclockwise throughout: clockwise mirrors y about the box's middle)
    let s = ft + fb;
    let mir = |y: f64| if cw { s - y } else { y };
    let (lx0, lx1) = (f64::from(l * 16), f64::from(r * 16));
    let (ly0, ly1) = if cw { (s - f64::from(bt * 16), s - f64::from(t * 16)) } else { (f64::from(t * 16), f64::from(bt * 16)) };
    let (lcx, lcy, lrx, lry) = ((lx0 + lx1) / 2.0, (ly0 + ly1) / 2.0, (lx1 - lx0) / 2.0, (ly1 - ly0) / 2.0);
    let angle = |q: (i32, i32)| (((lcy - mir(f64::from(q.1 * 16))) / lry).atan2((f64::from(q.0 * 16) - lcx) / lrx)).rem_euclid(TAU);
    let a0 = angle(rs);
    let mut a1 = angle(re);
    if a1 <= a0 {
        a1 += TAU;
    }
    let mut cuts = vec![a0];
    let mut k = (a0 / FRAC_PI_2).floor() + 1.0;
    while k * FRAC_PI_2 <= a1 {
        cuts.push(k * FRAC_PI_2);
        k += 1.0;
    }
    cuts.push(a1);
    let (cx, cy, rx, ry) = ((fl + fr) / 2.0, (ft + fb) / 2.0, (fr - fl) / 2.0, (fb - ft) / 2.0);
    let on_quarter = |v: f64| (v / FRAC_PI_2 - (v / FRAC_PI_2).round()).abs() < 1e-12;
    let fix = |x: f64, y: f64| -> Option<(i32, i32)> {
        let (x, y) = (x.round(), mir(y).round());
        (x.abs() < 1e9 && y.abs() < 1e9).then_some((x as i32, y as i32))
    };
    let mut v = Vec::with_capacity(cuts.len());
    for w in cuts.windows(2) {
        let (al, be) = (w[0], w[1]);
        let q = ((al / FRAC_PI_2 + 1e-12).floor() as usize) % 4;
        if on_quarter(al) && on_quarter(be) && (be - al - FRAC_PI_2).abs() < 1e-9 {
            v.push(([c[3 * q], c[3 * q + 1], c[3 * q + 2], c[3 * q + 3]], Known::Exact));
            continue;
        }
        let k = 4.0 / 3.0 * ((be - al) / 4.0).tan();
        let (p0, p3) = ((cx + rx * al.cos(), cy - ry * al.sin()), (cx + rx * be.cos(), cy - ry * be.sin()));
        let (d0, d1) = ((-rx * al.sin(), -ry * al.cos()), (-rx * be.sin(), -ry * be.cos()));
        v.push(([fix(p0.0, p0.1)?, fix(p0.0 + k * d0.0, p0.1 + k * d0.1)?, fix(p3.0 - k * d1.0, p3.1 - k * d1.1)?, fix(p3.0, p3.1)?], Known::ToThePixel));
    }
    Some(v)
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
        if c.iter().any(|q| q.known == Known::ToThePixel) {
            // (a part-quarter arc: GetPath's pixels measured, its 1/16 pixels not)
            return Err(Fail::Unmodeled(open::GDI_PATH_CURVE));
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

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if !matches!(t0, "beginpath" | "endpath" | "abortpath" | "closefig" | "flatten" | "widen" | "strokepath" | "fillpath" | "strokefill" | "selclippath" | "getpath" | "pathrgn" | "miter") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
    if sl.ext.mapped() {
        return Err(Fail::Unmodeled(open::GDI_MAP_DRAW));
    }
    if t0 == "miter" {
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
    let path = &mut sl.ext.path;
    let ok = match t0 {
        "beginpath" => {
            hit(clause::GDI_PATH_BEGIN);
            if matches!(path, Path::Open(_)) {
                // MEASURED: the points so far are dropped, a new path begins
                hit(clause::GDI_PATH_BEGINAGAIN);
            }
            *path = Path::Open(Vec::new());
            true
        }
        "endpath" => {
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
        "abortpath" => {
            hit(clause::GDI_PATH_ABORT);
            *path = Path::None;
            true
        }
        "closefig" => {
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
        "getpath" => {
            hit(clause::GDI_PATH_GET);
            match path {
                Path::Closed(p) => {
                    out.push(format!("path {}", p.len()));
                    for q in p.iter() {
                        out.push(format!("pt {} {} {}", q.x.round(), q.y.round(), q.ty()));
                    }
                }
                _ => out.push("path -1".into()),
            }
            return Ok(true);
        }
        "flatten" => {
            hit(clause::GDI_PATH_FLATTEN);
            match path {
                Path::Closed(p) => {
                    *p = flatten(p)?;
                    true
                }
                _ => false,
            }
        }
        "strokepath" => {
            hit(clause::GDI_PATH_STROKE);
            let Path::Closed(p) = std::mem::take(path) else {
                out.push("strokepath 0".into());
                return Ok(true);
            };
            stroke(sl, &p)?;
            true
        }
        "fillpath" | "strokefill" | "selclippath" | "pathrgn" => {
            if !matches!(path, Path::Closed(_)) {
                // (no closed path: the call fails, the DC keeps what it has)
                hit(clause::GDI_PATH_NOPATH);
                out.push(if t0 == "pathrgn" { "pathrgn 0".into() } else { format!("{t0} 0") });
                return Ok(true);
            }
            let Path::Closed(p) = std::mem::take(path) else { unreachable!() };
            let figs = figures(&p)?;
            if figs.is_empty() {
                // MEASURED: an empty path fails too, and is discarded
                hit(clause::GDI_PATH_EMPTYFILL);
                out.push(format!("{t0} 0"));
                return Ok(true);
            }
            // MEASURED: the interior is CreatePolygonRgn's region of the figures (each closed) in
            // the DC's fill mode (GDI.RGN.POLYSCAN); the call discards the path
            hit(clause::GDI_PATH_FILL);
            let ints: Option<Vec<Vec<(i32, i32)>>> = figs.iter().map(|f| f.iter().map(|&q| whole(q).ok()).collect()).collect();
            let r = match &ints {
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
        "widen" => return Err(Fail::Unmodeled(open::GDI_PATH_WIDEN)),
        _ => unreachable!(),
    };
    out.push(format!("{t0} {}", ok as u8));
    Ok(true)
}
