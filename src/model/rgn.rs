//! GDI regions as Windows keeps them: y-x banded rectangles (bands top to bottom, each band's
//! rectangles left to right, none touching in a band, equal neighbouring bands merged), so that
//! GetRegionData's list is this list.

/// a region: bands (top, bottom, spans), spans [left, right) sorted, apart
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rgn {
    pub bands: Vec<(i32, i32, Vec<(i32, i32)>)>,
}

/// NULLREGION, SIMPLEREGION, COMPLEXREGION
pub const NULLREGION: i32 = 1;
pub const SIMPLEREGION: i32 = 2;
pub const COMPLEXREGION: i32 = 3;

/// the set operations of CombineRgn (RGN_AND .. RGN_DIFF)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetOp {
    And,
    Or,
    Xor,
    Diff,
}

impl SetOp {
    pub fn from_mode(m: i64) -> Option<SetOp> {
        Some(match m {
            1 => SetOp::And,
            2 => SetOp::Or,
            3 => SetOp::Xor,
            4 => SetOp::Diff,
            _ => return None,
        })
    }
    fn keep(self, a: bool, b: bool) -> bool {
        match self {
            SetOp::And => a && b,
            SetOp::Or => a || b,
            SetOp::Xor => a != b,
            SetOp::Diff => a && !b,
        }
    }
}

fn inside(spans: &[(i32, i32)], x: i32) -> bool {
    spans.iter().any(|&(l, r)| l <= x && x < r)
}

/// spans of `a` op `b`, touching spans joined
fn span_op(a: &[(i32, i32)], b: &[(i32, i32)], op: SetOp) -> Vec<(i32, i32)> {
    let mut xs: Vec<i32> = a.iter().chain(b).flat_map(|&(l, r)| [l, r]).collect();
    xs.sort_unstable();
    xs.dedup();
    let mut out: Vec<(i32, i32)> = Vec::new();
    for w in xs.windows(2) {
        if op.keep(inside(a, w[0]), inside(b, w[0])) {
            match out.last_mut() {
                Some(l) if l.1 == w[0] => l.1 = w[1],
                _ => out.push((w[0], w[1])),
            }
        }
    }
    out
}

impl Rgn {
    pub fn empty() -> Rgn {
        Rgn::default()
    }
    /// CreateRectRgn / SetRectRgn: the corners in either order; an empty rectangle is no region
    pub fn rect(l: i32, t: i32, r: i32, b: i32) -> Rgn {
        let (l, r) = (l.min(r), l.max(r));
        let (t, b) = (t.min(b), t.max(b));
        if l == r || t == b {
            return Rgn::empty();
        }
        Rgn { bands: vec![(t, b, vec![(l, r)])] }
    }
    /// from rows of spans (y, spans), y increasing by one: equal neighbouring rows merged
    pub fn from_rows(rows: impl IntoIterator<Item = (i32, Vec<(i32, i32)>)>) -> Rgn {
        let mut g = Rgn::empty();
        for (y, s) in rows {
            g.push_band(y, y + 1, s);
        }
        g
    }
    fn push_band(&mut self, t: i32, b: i32, s: Vec<(i32, i32)>) {
        if s.is_empty() || t >= b {
            return;
        }
        if let Some(last) = self.bands.last_mut() {
            if last.1 == t && last.2 == s {
                last.1 = b;
                return;
            }
        }
        self.bands.push((t, b, s));
    }
    fn spans_at(&self, y: i32) -> &[(i32, i32)] {
        self.bands.iter().find(|b| b.0 <= y && y < b.1).map_or(&[], |b| &b.2)
    }
    /// CombineRgn's set operations
    pub fn combine(&self, o: &Rgn, op: SetOp) -> Rgn {
        let mut ys: Vec<i32> = self.bands.iter().chain(&o.bands).flat_map(|b| [b.0, b.1]).collect();
        ys.sort_unstable();
        ys.dedup();
        let mut g = Rgn::empty();
        for w in ys.windows(2) {
            g.push_band(w[0], w[1], span_op(self.spans_at(w[0]), o.spans_at(w[0]), op));
        }
        g
    }
    pub fn kind(&self) -> i32 {
        match self.bands.as_slice() {
            [] => NULLREGION,
            [(_, _, s)] if s.len() == 1 => SIMPLEREGION,
            _ => COMPLEXREGION,
        }
    }
    /// the rectangles (left, top, right, bottom), as GetRegionData lists them
    pub fn rects(&self) -> Vec<(i32, i32, i32, i32)> {
        self.bands.iter().flat_map(|(t, b, s)| s.iter().map(move |&(l, r)| (l, *t, r, *b))).collect()
    }
    /// GetRgnBox's rectangle (0 0 0 0 for no region)
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        if self.bands.is_empty() {
            return (0, 0, 0, 0);
        }
        let l = self.bands.iter().map(|b| b.2[0].0).min().unwrap();
        let r = self.bands.iter().map(|b| b.2.last().unwrap().1).max().unwrap();
        (l, self.bands[0].0, r, self.bands.last().unwrap().1)
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        inside(self.spans_at(y), x)
    }
    pub fn offset(&mut self, dx: i32, dy: i32) {
        for b in self.bands.iter_mut() {
            b.0 += dy;
            b.1 += dy;
            for s in b.2.iter_mut() {
                s.0 += dx;
                s.1 += dx;
            }
        }
    }
    pub fn offset_by(&self, dx: i32, dy: i32) -> Rgn {
        let mut g = self.clone();
        g.offset(dx, dy);
        g
    }
}

/// one polygon edge as the scan converter steps it: the x of each scanline from `top` to
/// `ymax`, by the integer Bresenham rule X11's (and Wine's) region code uses
#[derive(Clone, Copy, Debug)]
struct Edge {
    ymax: i32,
    x: i32,
    m: i32,
    m1: i32,
    incr1: i32,
    incr2: i32,
    d: i32,
    /// the edge runs downwards in the polygon's order
    down: bool,
}

impl Edge {
    fn new(top: (i32, i32), bottom: (i32, i32), down: bool) -> Edge {
        let dy = bottom.1 - top.1;
        let dx = bottom.0 - top.0;
        // (Rust's / truncates toward zero, as C's)
        let m = dx / dy;
        let (m1, incr1, incr2, d) = if dx < 0 {
            let m1 = m - 1;
            (m1, -2 * dx + 2 * dy * m1, -2 * dx + 2 * dy * m, 2 * m * dy - 2 * dx - 2 * dy)
        } else {
            let m1 = m + 1;
            (m1, 2 * dx - 2 * dy * m1, 2 * dx - 2 * dy * m, -2 * m * dy + 2 * dx)
        };
        Edge { ymax: bottom.1 - 1, x: top.0, m, m1, incr1, incr2, d, down }
    }
    fn step(&mut self) {
        let up = if self.m1 > 0 { self.d > 0 } else { self.d >= 0 };
        if up {
            self.x += self.m1;
            self.d += self.incr1;
        } else {
            self.x += self.m;
            self.d += self.incr2;
        }
    }
}

/// CreatePolyPolygonRgn's region (MEASURED against Windows' answers, GDI.RGN.POLYSCAN): each
/// scanline y takes the spans between its edges' x (left included, right not), edges stepped
/// as X11's integer scan converter steps them; ALTERNATE pairs the edges in x order, WINDING
/// keeps the spans of nonzero winding; a closed axis-aligned rectangle is that rectangle
pub fn polygon(polys: &[Vec<(i32, i32)>], winding: bool) -> Result<Rgn, super::Fail> {
    if let [p] = polys {
        let rectangle = (p.len() == 4 || p.len() == 5 && p[4] == p[0])
            && ((p[0].1 == p[1].1 && p[1].0 == p[2].0 && p[2].1 == p[3].1 && p[3].0 == p[0].0) || (p[0].0 == p[1].0 && p[1].1 == p[2].1 && p[2].0 == p[3].0 && p[3].1 == p[0].1));
        if rectangle {
            return Ok(Rgn::rect(p[0].0.min(p[2].0), p[0].1.min(p[2].1), p[0].0.max(p[2].0), p[0].1.max(p[2].1)));
        }
    }
    // the edge table: (top scanline, edge), in the order the polygons give them
    let mut table: Vec<(i32, Edge)> = Vec::new();
    for p in polys.iter().filter(|p| p.len() >= 2) {
        for i in 0..p.len() {
            let (prev, cur) = (p[(i + p.len() - 1) % p.len()], p[i]);
            if prev.1 == cur.1 {
                continue;
            }
            let (top, bottom, down) = if prev.1 > cur.1 { (cur, prev, false) } else { (prev, cur, true) };
            table.push((top.1, Edge::new(top, bottom, down)));
        }
    }
    let Some(ymin) = table.iter().map(|e| e.0).min() else { return Ok(Rgn::empty()) };
    let ymax = table.iter().map(|e| e.1.ymax + 1).max().unwrap();
    // edges entering at one scanline, in x order (a later one before an earlier of equal x)
    let mut buckets: std::collections::BTreeMap<i32, Vec<Edge>> = Default::default();
    for (y, e) in table {
        let b = buckets.entry(y).or_default();
        let at = b.iter().position(|o| o.x >= e.x).unwrap_or(b.len());
        b.insert(at, e);
    }
    let mut active: Vec<Edge> = Vec::new();
    let mut rows = Vec::new();
    for y in ymin..ymax {
        if let Some(b) = buckets.remove(&y) {
            for e in b {
                let at = active.iter().position(|o| o.x >= e.x).unwrap_or(active.len());
                active.insert(at, e);
            }
        }
        let mut spans: Vec<(i32, i32)> = Vec::new();
        if winding {
            let mut inside = 0;
            let mut start = None;
            for e in &active {
                let was = inside != 0;
                inside += if e.down { 1 } else { -1 };
                match (was, inside != 0) {
                    (false, true) => start = Some(e.x),
                    (true, false) => spans.push((start.take().unwrap(), e.x)),
                    _ => {}
                }
            }
        } else {
            for pair in active.chunks(2) {
                if let [a, b] = pair {
                    spans.push((a.x, b.x));
                }
            }
        }
        rows.push((y, normalize(spans)));
        // step, drop the edges that end, keep x order (a stable insertion sort)
        active.retain_mut(|e| {
            if e.ymax == y {
                return false;
            }
            e.step();
            true
        });
        for i in 1..active.len() {
            let mut j = i;
            while j > 0 && active[j - 1].x > active[j].x {
                active.swap(j - 1, j);
                j -= 1;
            }
        }
    }
    let g = Rgn::from_rows(rows);
    if g.bands.last().is_some_and(|b| b.1 != ymax) {
        // (rows left empty above the lowest vertex: Windows' bounding rectangle reaches down to
        // that vertex in some such regions and not in others, open)
        return Err(super::Fail::Unmodeled(crate::spec::open::GDI_RGN_POLYGON));
    }
    Ok(g)
}

/// spans sorted, overlapping and touching ones merged, empty ones dropped
fn normalize(mut s: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    s.retain(|&(l, r)| l < r);
    s.sort();
    let mut out: Vec<(i32, i32)> = Vec::new();
    for (l, r) in s {
        match out.last_mut() {
            Some(last) if l <= last.1 => last.1 = last.1.max(r),
            _ => out.push((l, r)),
        }
    }
    out
}
