//! GDI+ path regions as PenAlignment Inset uses them (GpPath::GetWidenedPath): a path's
//! aliased pixels (DpRegion::Set, the aliased fill rule of GP.RASTER.FILL), and a pixel set back
//! to a path (RegionToPath: the pixel-edge boundaries, DiagonalizePath), from the clean-room
//! notes (geometry.md 17), MEASURED on GDI+ itself, both arches alike.
use std::collections::BTreeMap;
use super::gdiplus::{FillMode, Matrix, Round};
use super::gppath::{enumerate, rasterize, Fig, Kind, Path, Point};
use super::Res;
use crate::spec::{clause, hit};

/// a set of device pixels: inside[(y - y0) w + x - x0]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pixels {
    x0: i64,
    y0: i64,
    w: i64,
    h: i64,
    inside: Vec<bool>,
}
impl Pixels {
    /// A bounded mask in row order, used to compare region algorithms with their Lean mirror.
    pub(crate) fn from_bits((x, y): (i32, i32), w: u8, h: u8, bits: u64) -> Option<Self> {
        if w > 7 || h > 7 {
            return None;
        }
        let cells = u32::from(w).checked_mul(u32::from(h))?;
        let inside = (0..cells).map(|i| bits.checked_shr(i).is_some_and(|suffix| suffix & 1 != 0)).collect();
        Some(Self { x0: i64::from(x), y0: i64::from(y), w: i64::from(w), h: i64::from(h), inside })
    }

    pub fn has(&self, x: i64, y: i64) -> bool {
        let (i, j) = (x - self.x0, y - self.y0);
        (0..self.w).contains(&i) && (0..self.h).contains(&j) && self.inside[(j * self.w + i) as usize]
    }
    /// the pixels in both
    pub fn and(&self, o: &Pixels) -> Pixels {
        let (x0, y0) = (self.x0.max(o.x0), self.y0.max(o.y0));
        let (w, h) = (((self.x0 + self.w).min(o.x0 + o.w) - x0).max(0), ((self.y0 + self.h).min(o.y0 + o.h) - y0).max(0));
        let inside = (0..h).flat_map(|j| (0..w).map(move |i| (i, j))).map(|(i, j)| self.has(x0 + i, y0 + j) && o.has(x0 + i, y0 + j)).collect();
        Pixels { x0, y0, w, h, inside }
    }
    fn all(&self) -> impl Iterator<Item = (i64, i64)> + '_ {
        (0..self.h).flat_map(move |j| (0..self.w).map(move |i| (self.x0 + i, self.y0 + j))).filter(|&(x, y)| self.has(x, y))
    }
}

/// a path's aliased pixels through m (device space, the pixel offset in it)
pub fn pixels(r: Round, path: &Path, m: &Matrix, fm: FillMode) -> Res<Pixels> {
    let m16 = m.map(|e| r.mul(e, 16.0));
    let figs = enumerate(r, path, &m16)?;
    let pts = figs.iter().flat_map(|f| f.pts.iter());
    let (lo, hi) = pts.fold(((i32::MAX, i32::MAX), (i32::MIN, i32::MIN)), |(lo, hi), &(x, y)| ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y))));
    if lo.0 > hi.0 {
        return Ok(Pixels { x0: 0, y0: 0, w: 0, h: 0, inside: Vec::new() });
    }
    // (the whole set on a grid of its own: moved by whole pixels, 16 in 28.4, exactly)
    let (x0, y0) = (i64::from(lo.0.div_euclid(16)) - 1, i64::from(lo.1.div_euclid(16)) - 1);
    let (w, h) = (i64::from(hi.0.div_euclid(16)) + 2 - x0, i64::from(hi.1.div_euclid(16)) + 2 - y0);
    let shift = |v: i32, by: i64| -> i32 { (i64::from(v) - by * 16) as i32 };
    let moved: Vec<Fig> = figs.iter().map(|f| Fig { pts: f.pts.iter().map(|&(x, y)| (shift(x, x0), shift(y, y0))).collect(), closed: f.closed }).collect();
    let cov = rasterize(&moved, fm, None, w as u32, h as u32);
    let inside = (0..h).flat_map(|j| (0..w).map(move |i| (i, j))).map(|(i, j)| cov.count(i as u32, j as u32) > 0).collect();
    Ok(Pixels { x0, y0, w, h, inside })
}

type V = (i64, i64);

/// An inset figure's clipped contours, retaining the whole path's fill.
pub(crate) fn inset_contours(own: &Pixels, whole: &Pixels) -> Vec<Vec<V>> {
    hit(clause::GP_PEN_INSET_REGION);
    to_path(&own.and(whole))
}

/// RegionToPath: the figures of a pixel set, closed, in device coordinates
pub fn to_path(px: &Pixels) -> Vec<Vec<V>> {
    let cells: Vec<V> = px.all().collect();
    let Some(&first) = cells.first() else { return vec![vec![(0, 0); 4]] };
    let (x0, x1) = cells.iter().fold((first.0, first.0), |(a, b), &(x, _)| (a.min(x), b.max(x)));
    let (y0, y1) = (first.1, cells[cells.len() - 1].1);
    if (x1 - x0 + 1) * (y1 - y0 + 1) == cells.len() as i64 {
        return vec![vec![(x0, y0), (x1 + 1, y0), (x1 + 1, y1 + 1), (x0, y1 + 1)]];
    }
    // (unit edges, the inside on the east going south)
    let mut out: BTreeMap<V, Vec<V>> = Default::default();
    let mut edge = |a: V, d: V| out.entry(a).or_default().push(d);
    for &(x, y) in &cells {
        if !px.has(x - 1, y) {
            edge((x, y), (0, 1));
        }
        if !px.has(x, y + 1) {
            edge((x, y + 1), (1, 0));
        }
        if !px.has(x + 1, y) {
            edge((x + 1, y + 1), (0, -1));
        }
        if !px.has(x, y - 1) {
            edge((x + 1, y), (-1, 0));
        }
    }
    let mut loops: Vec<Vec<V>> = Vec::new();
    while let Some((&start, _)) = out.iter().find(|(_, v)| !v.is_empty()) {
        let d0 = out.get_mut(&start).map(|v| v.remove(0)).unwrap_or((0, 0));
        let (mut at, mut d) = (start, d0);
        let mut corners = Vec::new();
        loop {
            let next = (at.0 + d.0, at.1 + d.1);
            let outs = out.entry(next).or_default();
            if next == start && !outs.contains(&d0) {
                outs.push(d0);
            }
            // (two ways on at a corner two pixels share: the left turn on screen)
            let left = (d.1, -d.0);
            let Some(k) = outs.iter().position(|&o| o == left).or(if outs.is_empty() { None } else { Some(0) }) else { break };
            let nd = outs.remove(k);
            if nd != d {
                corners.push(next);
            }
            if next == start && nd == d0 {
                break;
            }
            (at, d) = (next, nd);
        }
        split_touching_holes(corners, &mut loops);
    }
    let top_left = |l: &[V]| l.iter().copied().min_by_key(|&(x, y)| (y, x)).unwrap_or((0, 0));
    loops.sort_by_key(|l| {
        let (x, y) = top_left(l);
        (y, x)
    });
    let bounds: Vec<_> = loops.iter().map(|l| contour_bounds(l)).collect();
    loops
        .into_iter()
        .zip(bounds.iter().copied())
        .map(|(mut l, own)| {
            let hole = area_twice(&l) > 0;
            let ((left, top), (right, bottom)) = own;
            let depth = bounds.iter().filter(|&&other| {
                let ((x0, y0), (x1, y1)) = other;
                own != other && x0 <= left && y0 <= top && right <= x1 && bottom <= y1
            }).count();
            if hole {
                l.reverse();
            }
            let tl = top_left(&l);
            let at = l.iter().position(|&p| p == tl).unwrap_or(0);
            l.rotate_left(at);
            let mut f = diagonalize(l);
            // RegionToPath uses bounding-box parity for disconnected outer contours.
            if hole || depth % 2 == 1 {
                f.reverse();
            }
            f
        })
        .collect()
}

fn contour_bounds(l: &[V]) -> (V, V) {
    let first = l.first().copied().unwrap_or((0, 0));
    l.iter().fold((first, first), |(lo, hi), &(x, y)| {
        ((lo.0.min(x), lo.1.min(y)), (hi.0.max(x), hi.1.max(y)))
    })
}

fn area_twice(l: &[V]) -> i64 {
    l.iter().zip(l.iter().cycle().skip(1)).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum()
}

/// A connected outline can trace two corner-touching holes as one loop. Split at the
/// repeated vertex before choosing each hole's starting point and diagonalizing.
fn split_touching_holes(corners: Vec<V>, out: &mut Vec<Vec<V>>) {
    let mut pending = vec![corners];
    while let Some(mut l) = pending.pop() {
        let mut seen = BTreeMap::new();
        let split = if area_twice(&l) > 0 {
            l.iter().enumerate().find_map(|(i, &p)| seen.insert(p, i).map(|first| (first, i)))
        } else {
            None
        };
        if let Some((start, end)) = split {
            let suffix = l.split_off(end);
            let inner = l.split_off(start);
            l.extend(suffix);
            pending.extend([l, inner]);
        } else {
            out.push(l);
        }
    }
}

/// DiagonalizePath: a corner at the lower end of a vertical edge meeting a horizontal one
/// dropped when the vertical edge is 1 long or the horizontal one steps -1 (every row keeps its
/// ceiled x, the aliased pixels unchanged); a corner then collinear with the edge before it
/// merged, but never the first edge out of the start
fn diagonalize(mut p: Vec<V>) -> Vec<V> {
    let vertical = |a: V, b: V| a.0 == b.0 && a.1 != b.1;
    let horizontal = |a: V, b: V| a.1 == b.1 && a.0 != b.0;
    let mut i = 1;
    while i < p.len() && p.len() > 2 {
        let (a, q, b) = (p[i - 1], p[i], p[(i + 1) % p.len()]);
        let drop = match (vertical(a, q) && q.1 > a.1 && horizontal(q, b), vertical(q, b) && q.1 > b.1 && horizontal(a, q)) {
            (true, _) => (q.1 - a.1) == 1 || b.0 - q.0 == -1,
            (false, true) => (q.1 - b.1) == 1 || a.0 - q.0 == -1,
            (false, false) => false,
        };
        if !drop {
            i += 1;
            continue;
        }
        p.remove(i);
        if i >= 3 {
            let (a0, a1) = (p[i - 2], p[i - 1]);
            let b = p[i % p.len()];
            let (u, v) = ((a1.0 - a0.0, a1.1 - a0.1), (b.0 - a1.0, b.1 - a1.1));
            if u.0 * v.1 - u.1 * v.0 == 0 && u.0 * v.0 + u.1 * v.1 > 0 {
                p.remove(i - 1);
                i -= 1;
            }
        }
    }
    p
}

/// figures as closed path points
pub fn points(figs: &[Vec<V>]) -> Vec<Point> {
    figs.iter()
        .flat_map(|f| {
            let n = f.len();
            f.iter().enumerate().map(move |(i, &(x, y))| Point { x: x as f32, y: y as f32, kind: if i == 0 { Kind::Start } else { Kind::Line }, close: i + 1 == n, inner: false })
        })
        .collect()
}
