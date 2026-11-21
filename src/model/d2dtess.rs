//! Direct2D's ID2D1Geometry::Tessellate: the fill tessellator of d2d1.dll's scanner, from the
//! clean-room notes (D2D.TESS). The figures are snapped to the 1/64 grid as integers, cut where
//! they cross or touch, and swept vertex by vertex from the largest (y, x) down; each filled
//! band between a left and a right boundary keeps the chain of its vertices not yet cut into
//! triangles, and every vertex added to an end cuts off the ears that turn the right way.
//! Triangles come out as they are cut.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::d2d::{bounds, flatten_in, shape, Geom, Seg, P};
use super::{Fail, Res};
use crate::spec::{clause, hit, open};

pub(crate) type I = i128;

/// a point of the workspace as an exact fraction (x / d, y / d), d > 0, in lowest terms
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) struct Q {
    pub x: I,
    pub y: I,
    pub d: I,
}

fn gcd(a: I, b: I) -> I {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

impl Q {
    pub fn int(p: Pt) -> Q {
        Q { x: p.0, y: p.1, d: 1 }
    }

    pub fn new(x: I, y: I, d: I) -> Q {
        let s = if d < 0 { -1 } else { 1 };
        let g = gcd(gcd(x, y), d).max(1) * s;
        Q { x: x / g, y: y / g, d: d / g }
    }
}

/// the sweep order: Less when `a` comes first (larger y, then larger x)
pub(crate) fn sweep(a: Q, b: Q) -> Ordering {
    (b.y * a.d).cmp(&(a.y * b.d)).then((b.x * a.d).cmp(&(a.x * b.d)))
}

/// an integer workspace point
pub(crate) type Pt = (I, I);

pub(crate) fn cross(a: Pt, b: Pt) -> I {
    a.0 * b.1 - a.1 * b.0
}

/// an input segment, `s` the end swept first; `dir` +1 when the figure runs from s to e
pub(crate) struct Line {
    pub s: Pt,
    pub e: Pt,
    pub dir: i32,
    /// which input it is of (Combine's two geometries)
    pub tag: usize,
}

impl Line {
    pub fn d(&self) -> Pt {
        (self.e.0 - self.s.0, self.e.1 - self.s.1)
    }

    /// whether the point is right of the line (the line passes left of it)
    pub fn left_of(&self, v: Q) -> bool {
        let d = self.d();
        d.0 * (v.y - self.s.1 * v.d) - d.1 * (v.x - self.s.0 * v.d) > 0
    }
}

/// tails left to right: by dx / dy of the way back to the earlier end, a horizontal one last
pub(crate) fn tail_order(a: &Line, b: &Line) -> Ordering {
    let (da, db) = (a.d(), b.d());
    // (the way back is -d, dy > 0)
    match (da.1 == 0, db.1 == 0) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Greater,
        (false, true) => Ordering::Less,
        (false, false) => (da.0 * db.1).cmp(&(db.0 * da.1)),
    }
}

/// heads left to right: by dx / -dy of the first edge, a horizontal one first
pub(crate) fn head_order(a: &Line, b: &Line) -> Ordering {
    let (da, db) = (a.d(), b.d());
    match (da.1 == 0, db.1 == 0) {
        (true, true) => Ordering::Equal,
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => (da.0 * -db.1).cmp(&(db.0 * -da.1)),
    }
}

/// a piece of a line between two of the vertices on it
struct Edge {
    a: Q,
    b: Q,
    line: usize,
}

/// an active chain: its edge and its classification
#[derive(Clone, Copy)]
struct Chain {
    edge: usize,
    usable: bool,
    /// the fill is on its right (a band's left boundary)
    left: bool,
    /// the end of its band's vertex list it holds
    r: Option<usize>,
}

/// a vertex of a band's list
struct Node {
    v: (f64, f64),
    prev: Option<usize>,
    next: Option<usize>,
}

struct Bands {
    nodes: Vec<Node>,
    out: Vec<[(f64, f64); 3]>,
}

fn bad() -> Fail {
    Fail::Unmodeled(open::D2D_SIMP)
}

impl Bands {
    fn node(&mut self, v: (f64, f64)) -> usize {
        self.nodes.push(Node { v, prev: None, next: None });
        self.nodes.len() - 1
    }

    /// AddLeft / AddRight: the vertex at the chain's end of its band, ears cut while the
    /// cross product is below zero (f64, strict; empty triangles of rounded crossings too)
    fn add(&mut self, c: &mut Chain, p: (f64, f64)) -> Res<()> {
        let end = c.r.ok_or_else(bad)?;
        let r = self.node(p);
        if c.left {
            self.nodes[r].next = Some(end);
            self.nodes[end].prev = Some(r);
            while let Some(a) = self.nodes[r].next && let Some(b) = self.nodes[a].next {
                let (aa, bb) = (self.nodes[a].v, self.nodes[b].v);
                let cr = (bb.1 - aa.1) * (aa.0 - p.0) - (aa.1 - p.1) * (bb.0 - aa.0);
                if !(cr < 0.0) {
                    break;
                }
                self.out.push([aa, p, bb]);
                self.nodes[r].next = Some(b);
                self.nodes[b].prev = Some(r);
            }
        } else {
            self.nodes[end].next = Some(r);
            self.nodes[r].prev = Some(end);
            while let Some(a) = self.nodes[r].prev && let Some(b) = self.nodes[a].prev {
                let (aa, bb) = (self.nodes[a].v, self.nodes[b].v);
                let cr = (aa.0 - bb.0) * (p.1 - aa.1) - (aa.1 - bb.1) * (p.0 - aa.0);
                if !(cr < 0.0) {
                    break;
                }
                self.out.push([p, aa, bb]);
                self.nodes[b].next = Some(r);
                self.nodes[r].prev = Some(b);
            }
        }
        c.r = Some(r);
        Ok(())
    }

    /// CreateBands: heads paired left and right into new bands of the one vertex; an
    /// unpaired last one carries on the band of the last tail
    fn create(&mut self, hs: &mut [Chain], list: &[usize], tr: Option<&Chain>, v: (f64, f64)) -> Res<()> {
        let mut k = 0;
        while k < list.len() {
            if k + 1 == list.len() {
                hs[list[k]].r = Some(tr.and_then(|t| t.r).ok_or_else(bad)?);
                return Ok(());
            }
            let (l, r) = (list[k], list[k + 1]);
            if !hs[l].left || hs[r].left {
                return Err(bad());
            }
            let n = self.node(v);
            hs[l].r = Some(n);
            hs[r].r = Some(n);
            k += 2;
        }
        Ok(())
    }
}

/// the vertices on every line, in no order: its ends, crossings, touching points, overlaps'
/// ends; and for every crossing off the grid the lines it is inside, each with its fraction
/// along the line from its earlier end (numerator, denominator)
pub(crate) fn cuts(lines: &[Line]) -> (Vec<Vec<Q>>, HashMap<Q, Vec<(usize, I, I)>>) {
    let mut on: Vec<Vec<Q>> = lines.iter().map(|l| vec![Q::int(l.s), Q::int(l.e)]).collect();
    let mut inner: HashMap<Q, Vec<(usize, I, I)>> = HashMap::new();
    let within = |p: Pt, l: &Line| p.0 >= l.s.0.min(l.e.0) && p.0 <= l.s.0.max(l.e.0) && p.1 >= l.s.1.min(l.e.1) && p.1 <= l.s.1.max(l.e.1);
    for i in 0..lines.len() {
        for j in i + 1..lines.len() {
            let (l1, l2) = (&lines[i], &lines[j]);
            let (r, q) = (l1.d(), l2.d());
            let w = (l2.s.0 - l1.s.0, l2.s.1 - l1.s.1);
            let mut den = cross(r, q);
            if den == 0 {
                if cross(w, r) != 0 {
                    continue;
                }
                for p in [l2.s, l2.e] {
                    if within(p, l1) {
                        on[i].push(Q::int(p));
                    }
                }
                for p in [l1.s, l1.e] {
                    if within(p, l2) {
                        on[j].push(Q::int(p));
                    }
                }
                continue;
            }
            let (mut tn, mut un) = (cross(w, q), cross(w, r));
            if den < 0 {
                (den, tn, un) = (-den, -tn, -un);
            }
            if tn < 0 || tn > den || un < 0 || un > den {
                continue;
            }
            let v = Q::new(l1.s.0 * den + tn * r.0, l1.s.1 * den + tn * r.1, den);
            on[i].push(v);
            on[j].push(v);
            if v.d != 1 {
                let e = inner.entry(v).or_default();
                for (k, num) in [(i, tn), (j, un)] {
                    if !e.iter().any(|x| x.0 == k) {
                        e.push((k, num, den));
                    }
                }
            }
        }
    }
    (on, inner)
}

/// the figures in workspace units (1/64 DIP, integers), each without repeated points
fn workspace(g: &Geom, tol: f32, ext: f64) -> Res<(bool, Vec<Vec<Pt>>)> {
    let (wind, _, figs) = shape(g);
    let snap = |u: f64| (if u >= 0.0 { u + 0.5 } else { u - 0.5 }).trunc();
    let ws = |p: P| (snap(f64::from(p.0) * 64.0), snap(f64::from(p.1) * 64.0));
    let tol = 64.0 * f64::from(tol).max(ext * 3e-12);
    let mut all = Vec::new();
    for f in &figs {
        let mut pts = vec![ws(f.start)];
        for sg in &f.segs {
            match *sg {
                Seg::L(p) => pts.push(ws(p)),
                Seg::C(p1, p2, p3) => {
                    let p0 = pts[pts.len() - 1];
                    let b = [p0, ws(p1), ws(p2), ws(p3)].map(|p| [p.0, p.1]);
                    pts.extend(flatten_in(b, tol).into_iter().map(|v| (snap(v[0]), snap(v[1]))));
                    pts.push(ws(p3));
                }
            }
        }
        // (beyond 2^24 workspace units the exact arithmetic here could overflow: not modelled)
        if pts.iter().any(|p| !(p.0.abs() < 16777216.0 && p.1.abs() < 16777216.0)) {
            return Err(bad());
        }
        all.push(pts.iter().map(|p| (p.0 as I, p.1 as I)).collect());
    }
    Ok((wind, all))
}

/// MEASURED (D2D.TESS, from the clean-room notes; golden d2d-32/-64: tess-*, parea-*, carea-*
/// bit for bit): the triangles ID2D1Geometry::Tessellate gives its sink, in order, each as
/// three points
pub fn tessellate(g: &Geom, tol: f32) -> Res<Vec<[P; 3]>> {
    let b = bounds(g, None)?;
    if b[2] < b[0] || b[3] < b[1] {
        return Ok(Vec::new());
    }
    if !b.iter().all(|v| v.is_finite()) {
        return Err(bad());
    }
    let c = |v: f32| f64::from(v.clamp(-524287.0, 524287.0));
    let ext = (c(b[2]) - c(b[0])).max(c(b[3]) - c(b[1]));
    let (wind, figs) = workspace(g, tol, ext)?;
    let sc = scan(wind, &figs)?;
    hit(clause::D2D_TESS);
    // (each coordinate u 0.015625 + 0 in f64, then to f32 to nearest)
    let o = |v: (f64, f64)| ((v.0 * 0.015625 + 0.0) as f32, (v.1 * 0.015625 + 0.0) as f32);
    Ok(sc.tris.into_iter().map(|t| t.map(o)).collect())
}

/// what the scanner's sweep gives: the tessellator's triangles in workspace units, and CArea's
/// sum of the cross products of the filled region's boundary edges
pub struct Scan {
    pub tris: Vec<[(f64, f64); 3]>,
    pub acc: f64,
}

/// the scanner's sweep of integer figures (below 2^24 workspace units) by a fill mode
pub fn scan(wind: bool, figs: &[Vec<(i128, i128)>]) -> Res<Scan> {
    if figs.iter().flatten().any(|p| !(p.0.abs() < 1 << 24 && p.1.abs() < 1 << 24)) {
        return Err(bad());
    }
    let mut acc = 0f64;
    let filled = |w: i32| if wind { w != 0 } else { w % 2 != 0 };

    // the input lines, each from its earlier end
    let mut lines = Vec::new();
    let figs: Vec<Vec<Pt>> = figs.iter().map(|f| {
        let mut out: Vec<Pt> = Vec::new();
        for &p in f {
            if out.last() != Some(&p) {
                out.push(p);
            }
        }
        while out.len() > 1 && out.first() == out.last() {
            out.pop();
        }
        out
    }).filter(|f| f.len() > 1).collect();
    for f in &figs {
        let n = f.len();
        for i in 0..n {
            let (p, q) = (f[i], f[(i + 1) % n]);
            if sweep(Q::int(p), Q::int(q)) == Ordering::Less {
                lines.push(Line { s: p, e: q, dir: 1, tag: 0 });
            } else {
                lines.push(Line { s: q, e: p, dir: -1, tag: 0 });
            }
        }
    }
    let (mut on, inner) = cuts(&lines);
    // the one f64 copy of every vertex: integers as they are, a crossing as the line leftmost
    // just before it computes it, (E - S) clamp(num / den, 0, 1) + S
    let mut at: HashMap<Q, (f64, f64)> = HashMap::new();
    let mut edges = Vec::new();
    for (k, vs) in on.iter_mut().enumerate() {
        vs.sort_by(|a, b| sweep(*a, *b));
        vs.dedup();
        for w in vs.windows(2) {
            edges.push(Edge { a: w[0], b: w[1], line: k });
        }
        for &v in vs.iter() {
            if v.d == 1 {
                at.insert(v, (v.x as f64, v.y as f64));
            }
        }
    }
    for (v, ls) in &inner {
        let &(k, num, den) = ls.iter().min_by(|a, b| tail_order(&lines[a.0], &lines[b.0])).ok_or_else(bad)?;
        let l = &lines[k];
        let lambda = (num as f64 / den as f64).clamp(0.0, 1.0);
        let (sx, sy, ex, ey) = (l.s.0 as f64, l.s.1 as f64, l.e.0 as f64, l.e.1 as f64);
        at.insert(*v, ((ex - sx) * lambda + sx, (ey - sy) * lambda + sy));
    }
    let mut verts: Vec<Q> = at.keys().copied().collect();
    verts.sort_by(|a, b| sweep(*a, *b));
    let mut starts: HashMap<Q, Vec<usize>> = HashMap::new();
    for (k, e) in edges.iter().enumerate() {
        starts.entry(e.a).or_default().push(k);
    }

    let mut bands = Bands { nodes: Vec::new(), out: Vec::new() };
    let mut active: Vec<Chain> = Vec::new();
    for v in verts {
        let p = at[&v];
        let ends: Vec<usize> = (0..active.len()).filter(|&i| edges[active[i].edge].b == v).collect();
        let pos = match ends.first() {
            Some(&f) => {
                if ends.iter().enumerate().any(|(k, &i)| i != f + k) {
                    return Err(bad());
                }
                f
            }
            None => active.iter().filter(|c| lines[edges[c.edge].line].left_of(v)).count(),
        };
        let mut tails: Vec<Chain> = active.drain(pos..pos + ends.len()).collect();
        let mut hl: Vec<usize> = starts.get(&v).cloned().unwrap_or_default();
        hl.sort_by(|a, b| head_order(&lines[edges[*a].line], &lines[edges[*b].line]));
        // classification: the winding left of each head, a pass from the left
        let mut w: i32 = active[..pos].iter().map(|c| lines[edges[c.edge].line].dir).sum();
        let mut hs: Vec<Chain> = Vec::new();
        let mut sig = Vec::new();
        for &e in &hl {
            let wl = w;
            w += lines[edges[e].line].dir;
            let (fl, fr) = (filled(wl), filled(w));
            sig.push(fl != fr);
            hs.push(Chain { edge: e, usable: fl != fr, left: fr, r: None });
        }
        // coincident heads: consecutive significant ones to the same vertex, both left out
        let sl: Vec<usize> = (0..hs.len()).filter(|&k| sig[k]).collect();
        let mut k = 0;
        while k + 1 < sl.len() {
            if edges[hs[sl[k]].edge].b == edges[hs[sl[k + 1]].edge].b {
                hs[sl[k]].usable = false;
                hs[sl[k + 1]].usable = false;
                k += 2;
            } else {
                k += 1;
            }
        }
        let ut: Vec<usize> = (0..tails.len()).filter(|&k| tails[k].usable).collect();
        let uh: Vec<usize> = (0..hs.len()).filter(|&k| hs[k].usable).collect();
        // 1. the tails, left to right, each adds the vertex
        for &t in &ut {
            // (CArea: the edge's x0 y1 - x1 y0, added for a left boundary, taken away for a right)
            let a = at[&edges[tails[t].edge].a];
            let c = a.0 * p.1 - p.0 * a.1;
            if tails[t].left {
                acc += c;
            } else {
                acc -= c;
            }
            bands.add(&mut tails[t], p)?;
        }
        let tl = ut.first().map(|&t| tails[t]);
        let tr = ut.last().map(|&t| tails[t]);
        match (uh.first(), tl) {
            (None, None) => {}
            (None, Some(tl)) => {
                // 2. no heads: the bands close, or two meet
                if !tl.left {
                    let (Some(a), Some(b)) = (tl.r, tr.and_then(|t| t.r)) else { return Err(bad()) };
                    if ut.len() < 2 {
                        return Err(bad());
                    }
                    if let Some(pr) = bands.nodes[a].prev {
                        bands.nodes[pr].next = Some(b);
                        bands.nodes[b].prev = Some(pr);
                    } else {
                        let l = active[..pos].iter().rposition(|c| c.usable).ok_or_else(bad)?;
                        active[l].r = Some(b);
                        bands.nodes[b].prev = None;
                    }
                }
            }
            (Some(&h0), _) => {
                // 3. heads
                let mut rest: &[usize] = &uh;
                if !hs[h0].left {
                    if let Some(tl) = tl {
                        hs[h0].r = tl.r;
                        rest = &uh[1..];
                    } else {
                        // a vertex inside a band: split it at its latest vertex
                        let &hr = uh.last().ok_or_else(bad)?;
                        if hr == h0 || !hs[hr].left {
                            return Err(bad());
                        }
                        let l = active[..pos].iter().rposition(|c| c.usable).ok_or_else(bad)?;
                        let mut s = active[l].r.ok_or_else(bad)?;
                        while let Some(nx) = bands.nodes[s].next {
                            let (a, b) = (bands.nodes[s].v, bands.nodes[nx].v);
                            if !(a.1 > b.1 || (a.1 == b.1 && a.0 > b.0)) {
                                break;
                            }
                            s = nx;
                        }
                        let t = bands.node(bands.nodes[s].v);
                        let nx = bands.nodes[s].next;
                        bands.nodes[t].next = nx;
                        if let Some(nx) = nx {
                            bands.nodes[nx].prev = Some(t);
                        }
                        bands.nodes[s].next = None;
                        if nx.is_none() {
                            let r = active[pos..].iter().position(|c| c.usable).ok_or_else(bad)? + pos;
                            active[r].r = Some(t);
                        }
                        hs[h0].r = Some(s);
                        let mut c = hs[h0];
                        bands.add(&mut c, p)?;
                        hs[h0] = c;
                        hs[hr].r = Some(t);
                        let mut c = hs[hr];
                        bands.add(&mut c, p)?;
                        hs[hr] = c;
                        let inner: Vec<usize> = uh[1..uh.len() - 1].to_vec();
                        bands.create(&mut hs, &inner, None, p)?;
                        rest = &[];
                    }
                }
                let rest = rest.to_vec();
                bands.create(&mut hs, &rest, tr.as_ref(), p)?;
            }
        }
        active.splice(pos..pos, hs);
    }
    Ok(Scan { tris: bands.out, acc })
}
