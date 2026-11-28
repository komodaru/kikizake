//! Direct2D's widener (CPen / CWidener with the polygon sink), from the clean-room notes
//! (D2D.WIDEN): a figure widened by a circular pen in device space into its left rail and its
//! right rail, all in f64; lines offset along their normal, curves flattened and offset along
//! their own normal at every point, miter joins (limit 10) on the outer side of a corner and
//! the spine point on the inner one, flat caps. ID2D1Geometry::Widen hands the rails on as one
//! open figure each; DrawGeometry fills them as one WINDING polygon per figure.

use super::d2d::{flatten_t, Fig, Seg, M, P};
use super::{Fail, Res};
use crate::spec::{clause, hit, open};

type V = (f64, f64);

fn sub(a: V, b: V) -> V {
    (a.0 - b.0, a.1 - b.1)
}

fn add(a: V, b: V) -> V {
    (a.0 + b.0, a.1 + b.1)
}

fn scale(a: V, k: f64) -> V {
    (a.0 * k, a.1 * k)
}

fn dot(a: V, b: V) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

fn cross(a: V, b: V) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

/// a segment in device space: a line to a point, or a cubic through its control points
#[derive(Clone, Copy)]
enum DSeg {
    L(V),
    C(V, V, V),
}

/// the line join at a corner
#[derive(Clone, Copy, PartialEq)]
enum Join {
    /// MITER with this limit (in pen radii)
    Miter(f64),
    /// FORCE_ROUND_LINE_JOIN (ellipses, rounded rectangles): their corners are smooth
    Round,
}

/// the pen's shape in device space
#[derive(Clone, Copy)]
enum Shape {
    /// a circle of radius R: radius vectors are device vectors
    Circle,
    /// an ellipse, the unit circle through M (row 0 negated when M turns over), with its inverse:
    /// radius vectors are unit vectors in the pen's own space
    Ellipse { m: [f64; 4], inv: [f64; 4] },
}

struct Pen {
    r: f64,
    refine: f64,
    shape: Shape,
}

impl Pen {
    /// radiusVector: v (a device tangent) in the pen's space at the pen's radius
    fn rad(&self, v: V) -> V {
        let u = match self.shape {
            Shape::Circle => v,
            Shape::Ellipse { inv, .. } => ((v.1 * inv[2]) + (v.0 * inv[0]), (v.1 * inv[3]) + (v.0 * inv[1])),
        };
        let f = self.r / ((u.1 * u.1) + (u.0 * u.0)).sqrt();
        (u.0 * f, u.1 * f)
    }

    /// a turn of 90 degrees, in device space: the offset of a radius vector (rail 0 p - off,
    /// rail 1 p + off)
    fn off(&self, rad: V) -> V {
        let q = (-rad.1, rad.0);
        match self.shape {
            Shape::Circle => q,
            Shape::Ellipse { m, .. } => ((q.0 * m[0]) + (q.1 * m[2]), (q.1 * m[3]) + (q.0 * m[1])),
        }
    }

    /// penVec: a radius vector in device space
    fn vec(&self, rad: V) -> V {
        match self.shape {
            Shape::Circle => rad,
            Shape::Ellipse { m, .. } => ((rad.0 * m[0]) + (rad.1 * m[2]), (rad.1 * m[3]) + (rad.0 * m[1])),
        }
    }
}

fn unmodeled() -> Fail {
    Fail::Unmodeled(open::D2D_RT)
}

/// the end of a rail
fn end(rail: &[DSeg]) -> Res<V> {
    match rail.last() {
        Some(DSeg::L(p) | DSeg::C(_, _, p)) => Ok(*p),
        None => Err(unmodeled()),
    }
}

/// BezierDistance: the control point distance, in radii, of the cubic arc of a turn whose
/// cosine is dt / R^2 (4/3 tan of a quarter of it)
fn bezier_distance(dt: f64, r: f64) -> f64 {
    let rr = r * r;
    let c2 = (rr + dt) * 0.5;
    if c2 < 0.0 {
        return 0.0;
    }
    let s2 = rr - c2;
    if s2 <= 0.0 {
        return 0.0;
    }
    let sn = s2.sqrt();
    let num = (r - c2.sqrt()) * (4.0 / 3.0);
    if sn * 1e-6 >= num { 0.0 } else { num / sn }
}

/// RoundCorner: the arc from p0 (radius vector `from` around the spine point) to p3 (`to`):
/// a line when the turn is within the refinement, one cubic up to 90 degrees, else two split at
/// the bisector
fn round_corner(pen: &Pen, at: V, p0: V, p3: V, from: V, to: V, sweep: bool) -> Vec<DSeg> {
    let dt = dot(from, to);
    if dt > pen.refine {
        return vec![DSeg::L(p3)];
    }
    let rr = pen.r * pen.r;
    if dt >= 0.0 {
        let d = bezier_distance(dt, pen.r);
        return vec![DSeg::C(add(p0, scale(pen.vec(from), d)), sub(p3, scale(pen.vec(to), d)), p3)];
    }
    let re = from.0 * to.0 - from.1 * to.1;
    let im = to.0 * from.1 + from.0 * to.1;
    let mut mx = ((rr + re) * 0.5).abs().sqrt();
    let mut my = ((rr - re) * 0.5).abs().sqrt() * if im > 0.0 { 1.0 } else { -1.0 };
    let c = if sweep { (-to.1) * mx + to.0 * my } else { to.1 * mx + (-to.0) * my };
    if c < 0.0 {
        mx = -mx;
        my = -my;
    }
    let c2 = ((mx * to.0 + my * to.1).abs() + rr) * 0.5;
    let mut d = 0.0;
    if !(c2 < 0.0) && rr - c2 > 0.0 {
        let sn = (rr - c2).sqrt();
        let num = (pen.r - c2.sqrt()) * (4.0 / 3.0);
        if !(sn * 1e-6 >= num) {
            d = num / sn;
        }
    }
    let q = if sweep { (-my, mx) } else { (my, -mx) };
    let pm = add(at, pen.vec(q));
    let mm = scale(pen.vec((mx, my)), d);
    vec![DSeg::C(add(p0, scale(pen.vec(from), d)), sub(pm, mm), pm), DSeg::C(add(pm, mm), sub(p3, scale(pen.vec(to), d)), p3)]
}

struct Rails {
    /// rail 0, the spine point minus the offset (left in y-down device space)
    l: Vec<DSeg>,
    /// rail 1
    r: Vec<DSeg>,
    /// the radius vector the rails were last offset by
    rad: V,
    /// the spine point
    at: V,
}

impl Rails {
    /// DoCorner, polygon sink: the outer rail gets the miter (or the clipped miter's two
    /// points, or nothing: a bevel), then the next segment's offset point; the inner rail the
    /// spine point, then the next offset point
    fn corner(&mut self, pen: &Pen, vin: V, vout: V, join: Join) -> Res<()> {
        let rad_out = pen.rad(vout);
        let o = pen.off(rad_out);
        let c = self.at;
        let (next_l, next_r) = (sub(c, o), add(c, o));
        let det = cross(vin, vout);
        let dt = dot(vin, vout);
        let parallel = dt.abs() * 1e-4 >= det.abs();
        if parallel {
            if dt > 0.0 {
                return Ok(());
            }
            hit(clause::D2D_WIDEN_TURN);
            if let Join::Miter(lim) = join {
                // A miter reversal pushes both rails out along the incoming direction.
                let u = scale(pen.vec(pen.rad(vin)), lim);
                let (cl, cr) = (end(&self.l)?, end(&self.r)?);
                self.l.extend([add(cl, u), add(next_l, u), next_l].map(DSeg::L));
                self.r.extend([add(cr, u), add(next_r, u), next_r].map(DSeg::L));
                self.rad = rad_out;
                return Ok(());
            }
        }
        // A round reversal puts the arc on rail 1 and the spine point on rail 0.
        let outer_left = !parallel && det > 0.0;
        let lim = match join {
            Join::Miter(l) => l,
            Join::Round => {
                // a round join: the inner rail to the spine point and on, the outer one the arc
                hit(clause::D2D_WIDEN_ARC);
                let (outer, inner, next, inext) = if outer_left { (&mut self.l, &mut self.r, next_l, next_r) } else { (&mut self.r, &mut self.l, next_r, next_l) };
                inner.extend([DSeg::L(c), DSeg::L(inext)]);
                let p0 = end(outer)?;
                outer.extend(round_corner(pen, c, p0, next, self.rad, rad_out, !outer_left));
                self.rad = rad_out;
                return Ok(());
            }
        };
        let (cur, next) = if outer_left { (end(&self.l)?, next_l) } else { (end(&self.r)?, next_r) };
        let dot_r = -dot(rad_out, self.rad);
        let rr = pen.r * pen.r;
        let ll = lim * pen.r;
        let d = sub(next, cur);
        let (n_in, n_out) = (cross(d, vout), cross(d, vin));
        let valid = if det < 0.0 { n_in < 0.0 && n_out > 0.0 && det < n_in * 1e-6 } else { n_in > 0.0 && n_out < 0.0 && det > n_in * 1e-6 };
        let mut pts = Vec::new();
        let miter = if !valid {
            (dot_r < 0.0).then_some(next)
        } else {
            let m = add(cur, scale(vin, n_in / det));
            let a = (rr * lim) * lim;
            (((a - (rr + rr)) * rr) >= a * dot_r).then_some(m)
        };
        match miter {
            Some(m) => pts.push(m),
            None => {
                // the miter clipped at lim radii from the corner
                let den = (rr + dot_r) / 2.0;
                if den > 0.0 {
                    let den = den.sqrt();
                    let num = (rr - dot_r) / 2.0;
                    let num = if num < 0.0 { 0.0 } else { num.sqrt() };
                    let s = (ll - num).max(0.0);
                    if den > s * 1e-6 {
                        let s = s / den;
                        pts.push(add(cur, scale(pen.vec(self.rad), s)));
                        pts.push(sub(next, scale(pen.vec(rad_out), s)));
                    }
                }
            }
        }
        pts.push(next);
        let inner = if outer_left { next_r } else { next_l };
        let pts = pts.into_iter().map(DSeg::L);
        if outer_left {
            self.l.extend(pts);
            self.r.extend([DSeg::L(c), DSeg::L(inner)]);
        } else {
            self.r.extend(pts);
            self.l.extend([DSeg::L(c), DSeg::L(inner)]);
        }
        self.rad = rad_out;
        Ok(())
    }

    /// both rails to p, offset by the radius vector
    fn to(&mut self, pen: &Pen, p: V, rad: V) {
        let o = pen.off(rad);
        self.l.push(DSeg::L(sub(p, o)));
        self.r.push(DSeg::L(add(p, o)));
        self.rad = rad;
        self.at = p;
    }

    /// RoundTo, polygon sink: the refinement of a curve turning tighter than the pen, at the
    /// spine point `c` from the current radius vector to `rad_new`: the outer rail the arc, the
    /// inner one the spine point and its new offset point, and rail 0 a closed pie of the pen
    /// on the inner side (its old and new inner points, the arc between, the spine point)
    fn round_to(&mut self, pen: &Pen, rad_new: V, c: V, vin: V, vout: V) -> Res<()> {
        let o = pen.off(rad_new);
        let (n0, n1) = (sub(c, o), add(c, o));
        let outer0 = cross(vin, vout) > 0.0;
        let old_in = if outer0 { end(&self.r)? } else { end(&self.l)? };
        if outer0 {
            let p0 = end(&self.l)?;
            self.l.extend(round_corner(pen, c, p0, n0, self.rad, rad_new, false));
            self.r.extend([DSeg::L(c), DSeg::L(n1)]);
        } else {
            let p0 = end(&self.r)?;
            self.r.extend(round_corner(pen, c, p0, n1, self.rad, rad_new, true));
            self.l.extend([DSeg::L(c), DSeg::L(n0)]);
        }
        let (a, b, from, to) = if outer0 { (old_in, n1, scale(self.rad, -1.0), scale(rad_new, -1.0)) } else { (n0, old_in, rad_new, self.rad) };
        let keep = end(&self.l)?;
        self.l.push(DSeg::L(a));
        self.l.extend(round_corner(pen, c, a, b, from, to, false));
        self.l.extend([b, c, a, keep].map(DSeg::L));
        self.rad = rad_new;
        self.at = c;
        Ok(())
    }
}

/// the tangents a cubic starts and ends with (a control point at the end skipped while
/// within the fuzz)
fn tangents(p0: V, c1: V, c2: V, c3: V, tol: f64) -> (V, V) {
    let fuzz = tol * tol * 1e-4;
    let pick = |vs: [V; 3], f: f64| vs.into_iter().find(|v| dot(*v, *v) > f).unwrap_or(vs[2]);
    (pick([sub(c1, p0), sub(c2, p0), sub(c3, p0)], fuzz), pick([sub(c3, c2), sub(c3, c1), sub(c3, p0)], fuzz / 8.0))
}

/// the rails of every figure, f32, as Widen sends them: the right rail's start, the left rail,
/// the right rail back to its start. `m` the world transform, `w` the stroke width, `round` the
/// segments flagged FORCE_ROUND_LINE_JOIN, `tol` the flattening tolerance
pub fn widen(figs: &[Fig], round: bool, m: &M, w: f32, tol: f32) -> Res<Vec<Fig>> {
    let hw = (w.abs() * 0.5) as f32;
    let pm = [hw * m[0], hw * m[1], hw * m[2], hw * m[3]];
    let tol = f64::from(if tol > 0.0 { tol.max(f32::from_bits(0x358637bd)) } else { 0.25 });
    // Rmax, the pen ellipse's largest semi-axis, in f32
    let rmax = if pm[1] == 0.0 && pm[2] == 0.0 {
        let (a, b) = (pm[0].abs(), pm[3].abs());
        if a.is_nan() || b.is_nan() { f32::NAN } else { a.max(b) }
    } else {
        let a = pm[0] * pm[0] + pm[1] * pm[1];
        let b = pm[0] * pm[2] + pm[3] * pm[1];
        let c = pm[3] * pm[3] + pm[2] * pm[2];
        let s = (((b * 4.0) * b) + (a - c) * (a - c)).sqrt();
        ((s + (c + a)) * 0.5).sqrt()
    };
    let rmax = f64::from(rmax);
    let p = pm.map(f64::from);
    let eps2 = (0.004 * tol) * (0.004 * tol);
    let (r, shape) = if p[0] == p[3] && p[1] == -p[2] {
        let n2 = p[1] * p[1] + p[0] * p[0];
        if !(n2 >= eps2) {
            return Ok(Vec::new());
        }
        (n2.sqrt(), Shape::Circle)
    } else {
        hit(clause::D2D_WIDEN_PEN);
        let n2 = (p[1] * p[1] + p[0] * p[0]) + (p[3] * p[3] + p[2] * p[2]);
        let mut det = p[3] * p[0] - p[1] * p[2];
        if !(n2 >= eps2) || !(det.abs() >= eps2) {
            return Ok(Vec::new());
        }
        let mut m = p;
        if det < 0.0 {
            m[0] = -m[0];
            m[1] = -m[1];
            det = -det;
        }
        let inv = 1.0 / det;
        (1.0, Shape::Ellipse { m, inv: [m[3] * inv, (-m[1]) * inv, (-m[2]) * inv, inv * m[0]] })
    };
    if !r.is_finite() {
        return Err(unmodeled());
    }
    let rr = r * r;
    let refine = if tol > rmax {
        rr * -2.0
    } else {
        let g = 1.0 - tol / rmax;
        rr * (((g + g) * g) - 1.0)
    };
    let pen = Pen { r, refine, shape };
    let lf = |p: P| ((f64::from(m[0]) * f64::from(p.0) + f64::from(m[2]) * f64::from(p.1)) + f64::from(m[4]), (f64::from(m[1]) * f64::from(p.0) + f64::from(m[3]) * f64::from(p.1)) + f64::from(m[5]));
    let cf = |p: P| {
        let q = ((m[0] * p.0 + m[2] * p.1) + m[4], (m[1] * p.0 + m[3] * p.1) + m[5]);
        (f64::from(q.0), f64::from(q.1))
    };
    let mut out = Vec::new();
    for f in figs {
        let start = lf(f.start);
        let mut segs: Vec<DSeg> = f.segs.iter().map(|s| match *s {
            Seg::L(p) => DSeg::L(lf(p)),
            Seg::C(a, b, c) => DSeg::C(cf(a), cf(b), cf(c)),
        }).collect();
        let last_pt = segs.last().map(|s| match *s {
            DSeg::L(p) | DSeg::C(_, _, p) => p,
        });
        if f.closed && last_pt.is_some_and(|e| e != start) {
            segs.push(DSeg::L(start));
        }
        let mut rails: Option<Rails> = None;
        let mut first = (0.0, 0.0);
        let mut last_t = (0.0, 0.0);
        let mut p = start;
        let mut skipped = false;
        for sg in segs {
            let (t0, t1) = match sg {
                DSeg::L(q) => (sub(q, p), sub(q, p)),
                DSeg::C(a, b, c) => tangents(p, a, b, c, tol),
            };
            let q = match sg {
                DSeg::L(q) | DSeg::C(_, _, q) => q,
            };
            let degenerate = match sg {
                DSeg::L(q) => q == p,
                DSeg::C(a, b, c) => a == p && b == p && c == p,
            };
            if degenerate {
                skipped = true;
                continue;
            }
            let join = if round { Join::Round } else if skipped { Join::Miter(1.0) } else { Join::Miter(10.0) };
            skipped = false;
            match &mut rails {
                None => {
                    let mut rs = Rails { l: Vec::new(), r: Vec::new(), rad: (0.0, 0.0), at: p };
                    rs.to(&pen, p, pen.rad(t0));
                    rails = Some(rs);
                    first = t0;
                }
                Some(rs) => {
                    // (a round join of smooth segments: no turn)
                    rs.corner(&pen, last_t, t0, if round { Join::Round } else { join })?;
                }
            }
            let Some(rs) = &mut rails else { return Err(unmodeled()) };
            match sg {
                DSeg::L(q) => {
                    // AcceptLinePoint: the offset the rails hold (a corner straight within
                    // the fuzz keeps the last one, a curve's end tangent's)
                    let rad = rs.rad;
                    rs.to(&pen, q, rad);
                }
                DSeg::C(a, b, c) => {
                    hit(clause::D2D_WIDEN_CURVE);
                    let bz = [p, a, b, c].map(|v| [v.0, v.1]);
                    let mut pts: Vec<(V, V, bool)> = flatten_t(bz, tol).into_iter().map(|(v, _, t)| ((v[0], v[1]), (t[0], t[1]), false)).collect();
                    pts.push((c, t1, true));
                    // AcceptCurvePoint: each point offset along its tangent, unless the turn
                    // from the last is past the refinement or a rail would run backwards
                    let (mut prev_p, mut prev_t) = (p, t0);
                    for (pt, tg, last) in pts {
                        let rad = if dot(tg, tg) >= 1e-12 { pen.rad(tg) } else { rs.rad };
                        let mut ok = dot(rad, rs.rad) >= pen.refine;
                        if ok {
                            let o = pen.off(rad);
                            let (nl, nr) = (sub(pt, o), add(pt, o));
                            let (cl, cr) = (end(&rs.l)?, end(&rs.r)?);
                            ok = (nr.1 - cr.1) * (nl.1 - cl.1) + (nr.0 - cr.0) * (nl.0 - cl.0) > 0.0;
                        }
                        if ok {
                            rs.to(&pen, pt, rad);
                            prev_t = tg;
                        } else {
                            let seg = sub(pt, prev_p);
                            if dot(seg, seg) > 1e-12 {
                                hit(clause::D2D_WIDEN_ARC);
                                let rs_seg = pen.rad(seg);
                                rs.round_to(&pen, rs_seg, prev_p, prev_t, seg)?;
                                rs.to(&pen, pt, rs_seg);
                                if last {
                                    rs.round_to(&pen, rad, pt, seg, tg)?;
                                    prev_t = tg;
                                } else {
                                    prev_t = seg;
                                }
                            }
                        }
                        prev_p = pt;
                    }
                }
            }
            last_t = t1;
            p = q;
        }
        let Some(mut rs) = rails else { continue };
        if f.closed {
            rs.at = start;
            let join = if round { Join::Round } else if skipped { Join::Miter(1.0) } else { Join::Miter(10.0) };
            rs.corner(&pen, last_t, first, join)?;
        }
        let o = |v: V| (v.0 as f32, v.1 as f32);
        let seg = |s: &DSeg| match *s {
            DSeg::L(p) => Seg::L(o(p)),
            DSeg::C(a, b, c) => Seg::C(o(a), o(b), o(c)),
        };
        // rail 1's start, rail 0, then rail 1 back, each of its cubics turned around
        let ends: Vec<V> = rs.r.iter().map(|s| match *s {
            DSeg::L(p) | DSeg::C(_, _, p) => p,
        }).collect();
        let mut segs: Vec<Seg> = rs.l.iter().map(seg).collect();
        segs.push(Seg::L(o(end(&rs.r)?)));
        for (k, s) in rs.r.iter().enumerate().skip(1).rev() {
            let back = o(ends.get(k.wrapping_sub(1)).copied().ok_or_else(unmodeled)?);
            segs.push(match *s {
                DSeg::L(_) => Seg::L(back),
                DSeg::C(a, b, _) => Seg::C(o(b), o(a), back),
            });
        }
        out.push(Fig { start: o(ends.first().copied().ok_or_else(unmodeled)?), segs, closed: false });
    }
    hit(clause::D2D_WIDEN);
    Ok(out)
}
