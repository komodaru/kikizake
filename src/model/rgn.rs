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
