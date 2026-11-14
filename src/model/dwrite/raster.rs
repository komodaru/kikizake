//! IDWriteGlyphRunAnalysis: the alpha texture of a glyph run (the fonts' glyphs are
//! rectangles, so coverage is a matter of where their edges fall): every mode is the aliased
//! rasterizer on a grid of samples, the samples then counted (or filtered) into pixels.
use crate::spec::clause;
use super::super::Res;
use crate::spec::hit;
use crate::ttf::Parsed;

/// how a mode samples: nx by ny samples a pixel, and each glyph's origin snapped to 1/qx and
/// 1/qy of a pixel (0: not snapped)
#[derive(Clone, Copy)]
struct Grid {
    nx: i64,
    ny: i64,
    qx: i64,
    qy: i64,
}

/// each glyph's rectangles (x0, y0, x1, y1), y down, in 1/64 of a sample: its points scaled and
/// rounded to 1/64 pixel, at the pen position snapped to the grid, the pen moved by each advance
/// (adv4 / 4 pixels) from the unsnapped origin
fn rects(p: &Parsed, em: i64, g: Grid, x4: i64, y4: i64, run: &[(u16, i64)]) -> Vec<Vec<(i64, i64, i64, i64)>> {
    let sc = |v: i16| (v as i64 * em * 64 * 2 + p.upm as i64).div_euclid(2 * p.upm as i64);
    // (a position in 1/64 sample snapped to 1/q pixel, a step of 64 n / q of them)
    let snap = |v: i64, n: i64, q: i64| if q == 0 { v } else { (v + 32 * n / q).div_euclid(64 * n / q) * (64 * n / q) };
    let mut pen = x4 * 16 * g.nx;
    let oy = snap(y4 * 16 * g.ny, g.ny, g.qy);
    let mut v = Vec::new();
    for &(gid, adv) in run {
        let gid = if gid >= p.num_glyphs { 0 } else { gid };
        let at = snap(pen, g.nx, g.qx);
        let mut gv = Vec::new();
        if let Some(Some((_, contours))) = p.glyphs.get(gid as usize) {
            for c in contours {
                let xs = c.iter().map(|q| q.0);
                let ys = c.iter().map(|q| q.1);
                let (fx0, fx1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
                let (fy0, fy1) = (ys.clone().min().unwrap_or(0), ys.max().unwrap_or(0));
                gv.push((at + sc(fx0) * g.nx, oy - sc(fy1) * g.ny, at + sc(fx1) * g.nx, oy - sc(fy0) * g.ny));
            }
        }
        v.push(gv);
        pen += adv * 16 * g.nx;
    }
    v
}

/// the union of rectangles (each [lo, hi] along a scan line) crossing the scan line at `c`
fn spans(r: &[(i64, i64, i64, i64)], c: i64, horizontal: bool) -> Vec<(i64, i64)> {
    let mut v: Vec<(i64, i64)> = r.iter().filter(|q| if horizontal { q.1 <= c && c <= q.3 } else { q.0 <= c && c <= q.2 }).map(|q| if horizontal { (q.0, q.2) } else { (q.1, q.3) }).collect();
    v.sort();
    let mut m: Vec<(i64, i64)> = Vec::new();
    for s in v {
        match m.last_mut() {
            Some(l) if s.0 <= l.1 => l.1 = l.1.max(s.1),
            _ => m.push(s),
        }
    }
    m
}

/// MEASURED: the largest size whose textures have dropout control (32 has it, 39 not)
const DROPOUT_MAX_PPEM: i64 = 32;

/// whether the rasterizer keeps dropout control on (MEASURED: only up to a size)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dropout {
    On,
    Off,
}

impl Dropout {
    /// at `ppem` pixels an em
    pub fn at(ppem: i64) -> Dropout {
        if ppem <= DROPOUT_MAX_PPEM { Dropout::On } else { Dropout::Off }
    }
}

/// the pixels the aliased rasterizer sets: those whose centers are inside, and (MEASURED:
/// dropout control) one pixel beside each scan line span between two crossings that holds no
/// center, chosen by the glyph's box
pub fn aliased(r: &[(i64, i64, i64, i64)], dropout: Dropout) -> Res<std::collections::BTreeSet<(i64, i64)>> {
    let mut lit = std::collections::BTreeSet::new();
    let (mut y0, mut y1, mut x0, mut x1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for q in r {
        x0 = x0.min(q.0.div_euclid(64) - 1);
        x1 = x1.max(q.2.div_euclid(64) + 1);
        y0 = y0.min(q.1.div_euclid(64) - 1);
        y1 = y1.max(q.3.div_euclid(64) + 1);
    }
    let bx0 = r.iter().map(|q| q.0).min().unwrap_or(0);
    let by1 = r.iter().map(|q| q.3).max().unwrap_or(0);
    for (horizontal, a0, a1) in [(true, y0, y1), (false, x0, x1)] {
        for a in a0..=a1 {
            for (lo, hi) in spans(r, a * 64 + 32, horizontal) {
                // (the pixels whose centers are in [lo, hi]: a contour on a center sets it)
                let (first, last) = ((lo - 32 + 63).div_euclid(64), (hi - 32).div_euclid(64));
                let px = |b: i64| if horizontal { (b, a) } else { (a, b) };
                if first <= last {
                    if horizontal {
                        (first..=last).for_each(|b| {
                            lit.insert(px(b));
                        });
                    }
                } else {
                    // MEASURED (dropout control): a span between two centers lights the pixel
                    // before it on a row (left) when that pixel's center is inside the glyph's box,
                    // else the one after; on a column the one after it (below) when its center is
                    // inside the box, else the one before (above)
                    hit(clause::DW_RUN_DROPOUT);
                    if dropout == Dropout::Off {
                        hit(clause::DW_RUN_DROPOUT_SIZE);
                    }
                    // MEASURED: none when either pixel beside the span is already lit
                    if dropout == Dropout::Off || lit.contains(&px(first - 1)) || lit.contains(&px(first)) {
                        continue;
                    }
                    let b = if horizontal {
                        if (first - 1) * 64 + 32 >= bx0 { first - 1 } else { first }
                    } else if first * 64 + 32 <= by1 {
                        first
                    } else {
                        first - 1
                    };
                    lit.insert(px(b));
                }
            }
        }
    }
    Ok(lit)
}

/// how CreateGlyphRunAnalysis renders (the runner's dwrun modes)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// rendering aliased, measuring natural
    Aliased,
    /// natural symmetric, grayscale
    Gray,
    /// natural symmetric, ClearType 3x1
    ClearType,
    /// natural, grayscale
    Natural,
    /// GDI classic rendering and measuring
    Gdi,
    /// GDI natural rendering and measuring
    GdiNatural,
}

/// the grid fit mode asked for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fit {
    Default,
    Enabled,
    Disabled,
}

impl Mode {
    /// a dwrun mode token: the mode, then .fit or .nofit for the grid fit mode
    pub fn parse(t: &str) -> Option<(Mode, Fit)> {
        let (m, f) = match t.split_once('.') {
            None => (t, Fit::Default),
            Some((m, "fit")) => (m, Fit::Enabled),
            Some((m, "nofit")) => (m, Fit::Disabled),
            Some(_) => return None,
        };
        let m = match m {
            "aliased" => Mode::Aliased,
            "gray" => Mode::Gray,
            "cleartype" => Mode::ClearType,
            "natural" => Mode::Natural,
            "gdi" => Mode::Gdi,
            "gdinatural" => Mode::GdiNatural,
            _ => return None,
        };
        Some((m, f))
    }
}

pub fn run(p: &Parsed, mode: Mode, fit: Fit, em: i64, x4: i64, y4: i64, run: &[(u16, i64)]) -> Res<Vec<String>> {
    // MEASURED: the samples of each mode and where its glyphs' origins snap (aliased: one sample
    // a pixel, origin to the pixel; gray (natural symmetric): 4 x 4, unsnapped; natural: 8 x 1,
    // y to the pixel; GDI classic and natural: 8 x 1, origin to the pixel; ClearType: 6 x 5,
    // origin to the sample)
    let (g, clause) = match mode {
        Mode::Aliased => (Grid { nx: 1, ny: 1, qx: 1, qy: 1 }, clause::DW_RUN_ALIASED),
        Mode::Gray => (Grid { nx: 4, ny: 4, qx: 0, qy: 0 }, clause::DW_RUN_GRAY),
        Mode::Natural => (Grid { nx: 8, ny: 1, qx: 0, qy: 1 }, clause::DW_RUN_NATURAL),
        Mode::Gdi | Mode::GdiNatural => (Grid { nx: 8, ny: 1, qx: 1, qy: 1 }, clause::DW_RUN_GDI),
        Mode::ClearType => (Grid { nx: 6, ny: 5, qx: 6, qy: 5 }, clause::DW_RUN_CLEARTYPE),
    };
    hit(clause);
    // MEASURED: the pen starts at the unsnapped origin and moves by each advance; each glyph
    // is drawn at the pen snapped as the mode snaps the origin
    if run.len() > 1 {
        hit(clause::DW_RUN_PEN);
    }
    // MEASURED: the texture's bounds hold the pixels dropout control lights, also at the sizes
    // where the texture itself has them off
    let (mut lit, mut on) = (std::collections::BTreeSet::new(), std::collections::BTreeSet::new());
    for r in rects(p, em, g, x4, y4, run) {
        // (grid fit disabled: no dropout control at any size)
        let d = match fit {
            Fit::Default | Fit::Enabled => Dropout::at(em),
            Fit::Disabled => Dropout::Off,
        };
        lit.extend(aliased(&r, d)?);
        on.extend(aliased(&r, Dropout::On)?);
    }
    let cleartype = mode == Mode::ClearType;
    let val = values(&lit, g, cleartype);
    let bound = values(&on, g, cleartype);
    let bpp = if cleartype { 3 } else { 1 };
    let mut out = Vec::new();
    let xs = bound.keys().map(|k| k.0.div_euclid(bpp));
    let ys = bound.keys().map(|k| k.1);
    match (xs.clone().min(), ys.clone().min(), xs.max(), ys.max()) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => {
            out.push(format!("bounds hr 00000000 {x0} {y0} {} {}", x1 + 1, y1 + 1));
            for y in y0..=y1 {
                out.push((x0 * bpp..(x1 + 1) * bpp).map(|x| format!("{:02x}", val.get(&(x, y)).copied().unwrap_or(0))).collect());
            }
        }
        _ => {
            // MEASURED: nothing lit: empty bounds at the origin rounded to the pixel
            hit(clause::DW_RUN_EMPTY);
            let snap = |v: i64| (v + 2).div_euclid(4);
            let (x, y) = (snap(x4), snap(y4));
            out.push(format!("bounds hr 00000000 {x} {y} {x} {y}"));
        }
    }
    Ok(out)
}

/// each pixel's (or ClearType subpixel's) nonzero value, of the samples lit
fn values(lit: &std::collections::BTreeSet<(i64, i64)>, g: Grid, cleartype: bool) -> std::collections::BTreeMap<(i64, i64), i64> {
    let mut val: std::collections::BTreeMap<(i64, i64), i64> = Default::default();
    if cleartype {
        // MEASURED: a subpixel is 2 x 5 samples, the 5 rows weighted 4 9 10 9 4 (/36); each
        // subpixel's value is the sum of it and its two neighbours, of 255 for 6 x 36
        const W: [i64; 5] = [4, 9, 10, 9, 4];
        let mut acc: std::collections::BTreeMap<(i64, i64), i64> = Default::default();
        for &(x, y) in lit {
            let s = x.div_euclid(2);
            for d in -1..=1 {
                *acc.entry((s + d, y.div_euclid(5))).or_default() += W[y.rem_euclid(5) as usize];
            }
        }
        for (k, v) in acc {
            val.insert(k, (2 * 255 * v + 216).div_euclid(432));
        }
    } else {
        let n = g.nx * g.ny;
        let mut acc: std::collections::BTreeMap<(i64, i64), i64> = Default::default();
        for &(x, y) in lit {
            *acc.entry((x.div_euclid(g.nx), y.div_euclid(g.ny))).or_default() += 1;
        }
        for (k, v) in acc {
            val.insert(k, (2 * 255 * v + n).div_euclid(2 * n));
        }
    }
    val.retain(|_, v| *v != 0);
    val
}

/// the black box of glyph `gid` at `ppem` with its origin at (0, 0), in samples of an nx by ny
/// grid (x0, y0, x1, y1: the lit samples' columns x0..x1 and rows y0..y1, y down); None when
/// nothing is lit
pub fn black_box(p: &Parsed, ppem: i64, gid: u16, nx: i64, ny: i64) -> Res<Option<(i64, i64, i64, i64)>> {
    let g = Grid { nx, ny, qx: 0, qy: 0 };
    let mut lit = std::collections::BTreeSet::new();
    for r in rects(p, ppem, g, 0, 0, &[(gid, 0)]) {
        lit.extend(aliased(&r, Dropout::On)?);
    }
    let xs = lit.iter().map(|q| q.0);
    let ys = lit.iter().map(|q| q.1);
    Ok(match (xs.clone().min(), ys.clone().min(), xs.max(), ys.max()) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => Some((x0, y0, x1 + 1, y1 + 1)),
        _ => None,
    })
}
