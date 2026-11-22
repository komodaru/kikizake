//! Direct2D's ID2D1Geometry::Outline and CombineWithGeometry: the scanner's outline
//! reconstructor, from the clean-room notes (D2D.OUTLINE). The inputs are snapped to the 2^-20
//! grid, cut where they cross or touch, and every piece whose two sides differ by the operation
//! is a boundary edge, the fill on its right; the edges are joined into figures by the
//! sharpest right turn, each begun at its top left vertex, the figures in decreasing order of
//! that vertex. Pieces of a curve that nothing cuts come back as Bezier fragments.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::d2d::{bounds, flatten_t, shape, Fig, Geom, Seg, P};
use super::d2dtess::{cuts, head_order, sweep, Line, Pt, Q, I};
use super::{Fail, Res};
use crate::spec::{clause, hit, open};

const SCALE: f64 = 1048576.0;

fn bad() -> Fail {
    Fail::Unmodeled(open::D2D_SIMP)
}

/// the operation: which side of every piece is filled, from whether it is inside each input
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Outline: inside the one geometry
    Outline,
    Union,
    Intersect,
    Xor,
    Exclude,
}

impl Op {
    pub fn parse(t: &str) -> Option<Op> {
        Some(match t {
            "union" => Op::Union,
            "intersect" => Op::Intersect,
            "xor" => Op::Xor,
            "exclude" => Op::Exclude,
            _ => return None,
        })
    }

    fn keep(self, a: bool, b: bool) -> bool {
        match self {
            Op::Outline => a,
            Op::Union => a || b,
            Op::Intersect => a && b,
            Op::Xor => a != b,
            Op::Exclude => a && !b,
        }
    }
}

/// the part of a curve an edge is of: the curve, the parameters at its ends in the curve's
/// direction (ta < tb)
#[derive(Clone, Copy, Debug, PartialEq)]
struct Part {
    curve: usize,
    ta: f64,
    tb: f64,
}

/// a directed boundary edge: its ends exactly, their f64 values as the piece has them, its
/// direction, the curve it is of when nothing cut it and whether it runs against the curve
struct Bnd {
    from: Q,
    to: Q,
    vf: (f64, f64),
    vt: (f64, f64),
    d: Pt,
    part: Option<(Part, bool)>,
}

fn snap(u: f64) -> f64 {
    (if u >= 0.0 { u + 0.5 } else { u - 0.5 }).trunc()
}

/// de Casteljau: the curve on [0, t]
fn trim_end(p: [[f64; 2]; 4], t: f64) -> [[f64; 2]; 4] {
    let s = 1.0 - t;
    let m = |a: [f64; 2], b: [f64; 2]| [a[0] * s + b[0] * t, a[1] * s + b[1] * t];
    let (q1, q2, q3) = (m(p[0], p[1]), m(p[1], p[2]), m(p[2], p[3]));
    let (r2, r3) = (m(q1, q2), m(q2, q3));
    [p[0], q1, r2, m(r2, r3)]
}

/// de Casteljau: the curve on [t, 1]
fn trim_start(p: [[f64; 2]; 4], t: f64) -> [[f64; 2]; 4] {
    let s = 1.0 - t;
    let m = |a: [f64; 2], b: [f64; 2]| [a[0] * s + b[0] * t, a[1] * s + b[1] * t];
    let (q0, q1, q2) = (m(p[0], p[1]), m(p[1], p[2]), m(p[2], p[3]));
    let (r0, r1) = (m(q0, q1), m(q1, q2));
    [m(r0, r1), r1, q2, p[3]]
}

fn out(v: (f64, f64)) -> P {
    ((v.0 * (1.0 / SCALE) + 0.0) as f32, (v.1 * (1.0 / SCALE) + 0.0) as f32)
}

/// MEASURED (D2D.OUTLINE, from the clean-room notes; golden d2d-32/-64: outline-*, combine-*
/// bit for bit): the figures Outline (one geometry) or CombineWithGeometry (two) give the sink,
/// its fill mode ALTERNATE
pub fn outline(gs: &[&Geom], op: Op, tol: f32) -> Res<Vec<Fig>> {
    let tol = if tol > 0.0 { tol.max(f32::from_bits(0x358637bd)) } else { 0.25 };
    let mut ext = 0f64;
    for g in gs {
        let b = bounds(g, None)?;
        if !b.iter().all(|v| v.is_finite()) {
            return Err(bad());
        }
        if b[2] >= b[0] && b[3] >= b[1] {
            ext = ext.max(f64::from(b[2]) - f64::from(b[0])).max(f64::from(b[3]) - f64::from(b[1]));
        }
    }
    let tol = SCALE * f64::from(tol).max(ext * 3e-12);
    let ws = |p: P| (snap(f64::from(p.0) * SCALE), snap(f64::from(p.1) * SCALE));
    let mut curves: Vec<[[f64; 2]; 4]> = Vec::new();
    let mut lines: Vec<Line> = Vec::new();
    let mut parts: Vec<Option<(Part, bool)>> = Vec::new();
    let mut winds = Vec::new();
    for (tag, g) in gs.iter().enumerate() {
        let (wind, _, figs) = shape(g);
        winds.push(wind);
        for f in &figs {
            // each point with the part of a curve the edge to it is of
            let mut pts: Vec<((f64, f64), Option<Part>)> = vec![(ws(f.start), None)];
            for sg in &f.segs {
                match *sg {
                    Seg::L(p) => pts.push((ws(p), None)),
                    Seg::C(p1, p2, p3) => {
                        let p0 = pts[pts.len() - 1].0;
                        let b = [p0, ws(p1), ws(p2), ws(p3)].map(|p| [p.0, p.1]);
                        let curve = curves.len();
                        curves.push(b);
                        let mut t0 = 0.0;
                        for (v, t, _) in flatten_t(b, tol) {
                            pts.push(((snap(v[0]), snap(v[1])), Some(Part { curve, ta: t0, tb: t })));
                            t0 = t;
                        }
                        pts.push((ws(p3), Some(Part { curve, ta: t0, tb: 1.0 })));
                    }
                }
            }
            // (past 2^31 workspace units, about 2048 DIP, the exact crossing test here could
            // overflow: the spec's open D2D.SIMP, asked by outlinehuge-0)
            if pts.iter().any(|p| !(p.0.0.abs() < 2147483648.0 && p.0.1.abs() < 2147483648.0)) {
                return Err(bad());
            }
            let mut fp: Vec<(Pt, Option<Part>)> = Vec::new();
            for (p, part) in pts {
                let p = (p.0 as I, p.1 as I);
                if fp.last().map(|l| l.0) != Some(p) {
                    fp.push((p, part));
                }
            }
            // (closed for filling; when the last point is the first, its edge closes it)
            let mut close = None;
            if fp.len() > 1 && fp[0].0 == fp[fp.len() - 1].0 {
                close = fp.pop().and_then(|l| l.1);
            }
            let n = fp.len();
            if n < 2 {
                continue;
            }
            for i in 0..n {
                let (p, q) = (fp[i].0, fp[(i + 1) % n].0);
                let part = if i + 1 < n { fp[i + 1].1 } else { close };
                let fwd = sweep(Q::int(p), Q::int(q)) == Ordering::Less;
                lines.push(if fwd { Line { s: p, e: q, dir: 1, tag } } else { Line { s: q, e: p, dir: -1, tag } });
                // (whether the curve runs from the line's earlier end)
                parts.push(part.map(|pt| (pt, fwd)));
            }
        }
    }
    let (mut on, inner) = cuts(&lines);
    for vs in on.iter_mut() {
        vs.sort_by(|a, b| sweep(*a, *b));
        vs.dedup();
    }
    // each line's f64 value at each of its vertices: the ends and touching points exact, a
    // crossing from the line's base (its last cut) and the crossing line's: A = base_i,
    // B = T_i, C = base_j, D = T_j, lambda = clamp(cross(AC, DC) / cross(AB, DC), 0, 1),
    // V = (B - A) lambda + A; cut points in sweep order
    let mut val: Vec<Vec<(f64, f64)>> = on.iter().map(|vs| vs.iter().map(|v| (v.x as f64 / v.d as f64, v.y as f64 / v.d as f64)).collect()).collect();
    let mut base: Vec<(f64, f64)> = lines.iter().map(|l| (l.s.0 as f64, l.s.1 as f64)).collect();
    let mut evs: Vec<(Q, usize, usize)> = Vec::new();
    for (i, vs) in on.iter().enumerate() {
        for k in 1..vs.len().saturating_sub(1) {
            evs.push((vs[k], i, k));
        }
    }
    evs.sort_by(|a, b| sweep(a.0, b.0));
    let mut g = 0;
    while g < evs.len() {
        let mut h = g;
        while h < evs.len() && evs[h].0 == evs[g].0 {
            h += 1;
        }
        let x = evs[g].0;
        let mut news = Vec::new();
        for &(_, i, k) in &evs[g..h] {
            let v = if x.d == 1 {
                (x.x as f64, x.y as f64)
            } else {
                let j = inner.get(&x).and_then(|ls| ls.iter().find(|l| l.0 != i)).ok_or_else(bad)?.0;
                let (a, c) = (base[i], base[j]);
                let (bb, dd) = ((lines[i].e.0 as f64, lines[i].e.1 as f64), (lines[j].e.0 as f64, lines[j].e.1 as f64));
                let ab = (bb.0 - a.0, bb.1 - a.1);
                let dc = (c.0 - dd.0, c.1 - dd.1);
                let ac = (c.0 - a.0, c.1 - a.1);
                let den = ab.0 * dc.1 - ab.1 * dc.0;
                let lam = ((ac.0 * dc.1 - ac.1 * dc.0) / den).clamp(0.0, 1.0);
                ((bb.0 - a.0) * lam + a.0, (bb.1 - a.1) * lam + a.1)
            };
            news.push((i, k, v));
        }
        for (i, k, v) in news {
            val[i][k] = v;
            base[i] = v;
        }
        g = h;
    }
    // the pieces; a cut line's are lines all
    struct Piece {
        a: Q,
        b: Q,
        va: (f64, f64),
        vb: (f64, f64),
        line: usize,
    }
    let mut pieces = Vec::new();
    for (i, vs) in on.iter().enumerate() {
        for k in 0..vs.len() - 1 {
            pieces.push(Piece { a: vs[k], b: vs[k + 1], va: val[i][k], vb: val[i][k + 1], line: i });
        }
    }
    let mut verts: Vec<Q> = pieces.iter().flat_map(|p| [p.a, p.b]).collect();
    verts.sort_by(|a, b| sweep(*a, *b));
    verts.dedup();
    let mut starts: HashMap<Q, Vec<usize>> = HashMap::new();
    for (k, p) in pieces.iter().enumerate() {
        starts.entry(p.a).or_default().push(k);
    }
    let filled = |tag: usize, w: i32| if winds[tag] { w != 0 } else { w % 2 != 0 };
    let inside = |w: &[i32; 2]| op.keep(filled(0, w[0]), gs.len() > 1 && filled(1, w[1]));
    // the sweep: the windings left of every head, coincident heads one candidate
    let mut bnd: Vec<Bnd> = Vec::new();
    let mut active: Vec<usize> = Vec::new();
    for v in verts {
        let ends: Vec<usize> = (0..active.len()).filter(|&i| pieces[active[i]].b == v).collect();
        let pos = match ends.first() {
            Some(&f) => {
                if ends.iter().enumerate().any(|(k, &i)| i != f + k) {
                    return Err(bad());
                }
                f
            }
            None => active.iter().filter(|&&c| lines[pieces[c].line].left_of(v)).count(),
        };
        active.drain(pos..pos + ends.len());
        let mut hs: Vec<usize> = starts.get(&v).cloned().unwrap_or_default();
        hs.sort_by(|a, b| head_order(&lines[pieces[*a].line], &lines[pieces[*b].line]));
        let mut w = [0i32; 2];
        for &c in &active[..pos] {
            let l = &lines[pieces[c].line];
            w[l.tag] += l.dir;
        }
        let mut k = 0;
        while k < hs.len() {
            let mut e = k + 1;
            while e < hs.len() && pieces[hs[e]].b == pieces[hs[k]].b {
                e += 1;
            }
            let left = inside(&w);
            for &h in &hs[k..e] {
                let l = &lines[pieces[h].line];
                w[l.tag] += l.dir;
            }
            let right = inside(&w);
            if left != right {
                let pc = &pieces[hs[k]];
                let l = &lines[pc.line];
                let whole = on[pc.line].len() == 2;
                let d = l.d();
                // (fill on the right of the way it runs: the right of a head in the sweep)
                let part = parts[pc.line].filter(|_| whole);
                bnd.push(if right {
                    Bnd { from: pc.a, to: pc.b, vf: pc.va, vt: pc.vb, d, part: part.map(|(p, fwd)| (p, !fwd)) }
                } else {
                    Bnd { from: pc.b, to: pc.a, vf: pc.vb, vt: pc.va, d: (-d.0, -d.1), part: part.map(|(p, fwd)| (p, fwd)) }
                });
            }
            k = e;
        }
        active.splice(pos..pos, hs);
    }
    // the figures: at each vertex the sharpest right turn
    let mut from: HashMap<Q, Vec<usize>> = HashMap::new();
    for (k, e) in bnd.iter().enumerate() {
        from.entry(e.from).or_default().push(k);
    }
    let ang = |d: Pt| (d.1 as f64).atan2(d.0 as f64);
    let mut next = vec![usize::MAX; bnd.len()];
    let mut taken = vec![false; bnd.len()];
    for (k, e) in bnd.iter().enumerate() {
        let a = ang(e.d);
        let turn = |o: usize| {
            let mut t = ang(bnd[o].d) - a;
            while t > std::f64::consts::PI {
                t -= std::f64::consts::TAU;
            }
            while t <= -std::f64::consts::PI {
                t += std::f64::consts::TAU;
            }
            t
        };
        let o = from.get(&e.to).and_then(|os| os.iter().copied().max_by(|&x, &y| turn(x).total_cmp(&turn(y)))).ok_or_else(bad)?;
        if taken[o] {
            return Err(bad());
        }
        taken[o] = true;
        next[k] = o;
    }
    let mut seen = vec![false; bnd.len()];
    let mut cycles: Vec<Vec<usize>> = Vec::new();
    for k in 0..bnd.len() {
        if seen[k] {
            continue;
        }
        let mut c = Vec::new();
        let mut i = k;
        while !seen[i] {
            seen[i] = true;
            c.push(i);
            i = next[i];
        }
        // (begun at its smallest (y, x) vertex: the last in the sweep)
        let s = (0..c.len()).max_by(|&x, &y| sweep(bnd[c[x]].from, bnd[c[y]].from)).unwrap_or(0);
        c.rotate_left(s);
        cycles.push(c);
    }
    cycles.sort_by(|a, b| sweep(bnd[a[0]].from, bnd[b[0]].from));
    let mut figs = Vec::new();
    for c in cycles {
        let start = bnd[c[0]].vf;
        let mut segs = Vec::new();
        let mut last = start;
        // a Bezier fragment not yet sent: part, reversed, its end
        let mut pend: Option<(Part, bool, (f64, f64))> = None;
        let flush = |pend: &mut Option<(Part, bool, (f64, f64))>, segs: &mut Vec<Seg>, last: &mut (f64, f64)| {
            if let Some((pt, rev, v)) = pend.take() && pt.tb - pt.ta >= 1e-6 {
                let mut b = curves[pt.curve];
                if pt.tb < 1.0 {
                    b = trim_end(b, pt.tb);
                }
                if pt.ta > 0.0 {
                    b = trim_start(b, pt.ta / pt.tb);
                }
                let (c1, c2) = if rev { (b[2], b[1]) } else { (b[1], b[2]) };
                segs.push(Seg::C(out((c1[0], c1[1])), out((c2[0], c2[1])), out(v)));
                *last = v;
            }
        };
        for &k in &c {
            let e = &bnd[k];
            match e.part {
                None => {
                    flush(&mut pend, &mut segs, &mut last);
                    segs.push(Seg::L(out(e.vt)));
                    last = e.vt;
                }
                Some((pt, rev)) => {
                    match &mut pend {
                        Some((pp, prev, end)) if pp.curve == pt.curve && *prev == rev && ((!rev && pp.tb == pt.ta) || (rev && pp.ta == pt.tb)) => {
                            if rev {
                                pp.ta = pt.ta;
                            } else {
                                pp.tb = pt.tb;
                            }
                            *end = e.vt;
                        }
                        _ => {
                            flush(&mut pend, &mut segs, &mut last);
                            pend = Some((pt, rev, e.vt));
                        }
                    }
                }
            }
        }
        flush(&mut pend, &mut segs, &mut last);
        figs.push(Fig { start: out(start), segs, closed: last == start });
    }
    hit(clause::D2D_OUTLINE);
    Ok(figs)
}
