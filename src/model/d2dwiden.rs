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

struct Pen {
    r: f64,
    refine: f64,
}

impl Pen {
    /// the radius vector: v at the pen's radius
    fn rad(&self, v: V) -> V {
        scale(v, self.r / dot(v, v).sqrt())
    }
}

/// a turn of 90 degrees: the offset of a radius vector (left rail p - off, right rail p + off)
fn off(rad: V) -> V {
    (-rad.1, rad.0)
}

fn unmodeled() -> Fail {
    Fail::Unmodeled(open::D2D_RT)
}

struct Rails {
    l: Vec<V>,
    r: Vec<V>,
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
        let o = off(rad_out);
        let c = self.at;
        let (next_l, next_r) = (sub(c, o), add(c, o));
        let det = cross(vin, vout);
        let dt = dot(vin, vout);
        if det.abs() <= dt.abs() * 1e-6 {
            if dt > 0.0 {
                return Ok(());
            }
            // a turn of 180 degrees (golden d2d-32/-64: widenturn-*, strketurn-*): a miter join
            // pushes both rails out by the limit along the incoming direction, then across
            let Join::Miter(lim) = join else {
                // (a round join turning back: the spec's open D2D.SIMP / D2D.RT)
                return Err(unmodeled());
            };
            hit(clause::D2D_WIDEN_TURN);
            let u = scale(pen.rad(vin), lim);
            let (Some(&cl), Some(&cr)) = (self.l.last(), self.r.last()) else { return Err(unmodeled()) };
            self.l.extend([add(cl, u), add(next_l, u), next_l]);
            self.r.extend([add(cr, u), add(next_r, u), next_r]);
            self.rad = rad_out;
            return Ok(());
        }
        let lim = match join {
            Join::Miter(l) => l,
            // (a round join with a real turn, its arcs: the spec's open D2D.SIMP / D2D.RT)
            Join::Round => return Err(unmodeled()),
        };
        let outer_left = det > 0.0;
        let (cur, next) = if outer_left { (self.l[self.l.len() - 1], next_l) } else { (self.r[self.r.len() - 1], next_r) };
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
            (dot_r * ll * ll <= rr * (ll * ll - 2.0 * rr)).then_some(m)
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
                        pts.push(add(cur, scale(self.rad, s)));
                        pts.push(sub(next, scale(rad_out, s)));
                    }
                }
            }
        }
        pts.push(next);
        let inner = if outer_left { next_r } else { next_l };
        if outer_left {
            self.l.extend(pts);
            self.r.extend([c, inner]);
        } else {
            self.r.extend(pts);
            self.l.extend([c, inner]);
        }
        self.rad = rad_out;
        Ok(())
    }

    /// both rails to p, offset by the radius vector
    fn to(&mut self, p: V, rad: V) {
        let o = off(rad);
        self.l.push(sub(p, o));
        self.r.push(add(p, o));
        self.rad = rad;
        self.at = p;
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
pub fn widen(figs: &[Fig], round: bool, m: &M, w: f32, tol: f32) -> Res<Vec<Vec<P>>> {
    let hw = (w.abs() * 0.5) as f32;
    let pm = [hw * m[0], hw * m[1], hw * m[2], hw * m[3]];
    // (an elliptic pen: the spec's open D2D.SIMP / D2D.RT)
    if !(pm[0] == pm[3] && pm[1] == -pm[2]) {
        return Err(unmodeled());
    }
    let r = (f64::from(pm[0]) * f64::from(pm[0]) + f64::from(pm[1]) * f64::from(pm[1])).sqrt();
    if r == 0.0 {
        return Ok(Vec::new());
    }
    let tol = f64::from(if tol > 0.0 { tol.max(f32::from_bits(0x358637bd)) } else { 0.25 });
    // (the empty pen's threshold, about 0.004 of the tolerance: not settled)
    if !r.is_finite() || r < 0.01 * tol {
        return Err(unmodeled());
    }
    let rmax = if pm[1] == 0.0 { f64::from(pm[0].abs().max(pm[3].abs())) } else { f64::from(r as f32) };
    let refine = if rmax < tol { -2.0 } else { r * r * (2.0 * (1.0 - tol / rmax) * (1.0 - tol / rmax) - 1.0) };
    let pen = Pen { r, refine };
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
        let end = segs.last().map(|s| match *s {
            DSeg::L(p) | DSeg::C(_, _, p) => p,
        });
        if f.closed && end.is_some_and(|e| e != start) {
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
                    rs.to(p, pen.rad(t0));
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
                    let rad = pen.rad(t0);
                    rs.to(q, rad);
                }
                DSeg::C(a, b, c) => {
                    hit(clause::D2D_WIDEN_CURVE);
                    let bz = [p, a, b, c].map(|v| [v.0, v.1]);
                    let mut pts: Vec<(V, V, bool)> = flatten_t(bz, tol).into_iter().map(|(v, _, t)| ((v[0], v[1]), (t[0], t[1]), false)).collect();
                    pts.push((c, t1, true));
                    for (pt, tg, _) in pts {
                        let rad = if dot(tg, tg) >= 1e-12 { pen.rad(tg) } else { rs.rad };
                        let mut need = dot(rs.rad, rad) < pen.refine;
                        if !need {
                            let o = off(rad);
                            let (nl, nr) = (sub(pt, o), add(pt, o));
                            let (cl, cr) = (rs.l[rs.l.len() - 1], rs.r[rs.r.len() - 1]);
                            if dot(sub(nl, cl), sub(nr, cr)) <= 0.0 {
                                need = true;
                            }
                        }
                        // (the refinement arcs of a tight turn: the spec's open D2D.SIMP / D2D.RT, asked of Windows by widentight-*, strketight-*)
                        if need {
                            return Err(unmodeled());
                        }
                        rs.to(pt, rad);
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
        let mut pts = vec![o(rs.r[0])];
        pts.extend(rs.l.iter().map(|&v| o(v)));
        pts.extend(rs.r.iter().rev().map(|&v| o(v)));
        out.push(pts);
    }
    hit(clause::D2D_WIDEN);
    Ok(out)
}
