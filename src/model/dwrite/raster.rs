//! IDWriteGlyphRunAnalysis: the alpha texture of a glyph run (the fonts' glyphs are
//! rectangles, so coverage is a matter of where their edges fall): every mode is the aliased
//! rasterizer on a grid of samples, the samples then counted (or filtered) into pixels.
use super::super::{Fail, Res};
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

/// the pixels the aliased rasterizer sets: those whose centers are inside, and (MEASURED:
/// dropout control) on a scan line whose span between two crossings holds no center, the pixel
/// after the span on a row and the one before it (above) on a column
pub fn aliased(r: &[(i64, i64, i64, i64)]) -> Res<std::collections::BTreeSet<(i64, i64)>> {
    let mut lit = std::collections::BTreeSet::new();
    let (mut y0, mut y1, mut x0, mut x1) = (i64::MAX, i64::MIN, i64::MAX, i64::MIN);
    for q in r {
        x0 = x0.min(q.0.div_euclid(64) - 1);
        x1 = x1.max(q.2.div_euclid(64) + 1);
        y0 = y0.min(q.1.div_euclid(64) - 1);
        y1 = y1.max(q.3.div_euclid(64) + 1);
    }
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
                    // after it on a row and the one before it (above) on a column
                    hit("DW.RUN.DROPOUT");
                    if r.len() > 1 {
                        return Err(Fail::Unmodeled("DW.RUN.DROPOUT.CONTOURS"));
                    }
                    lit.insert(px(if horizontal { first } else { first - 1 }));
                }
            }
        }
    }
    Ok(lit)
}

pub fn run(p: &Parsed, mode: &str, em: i64, x4: i64, y4: i64, run: &[(u16, i64)]) -> Res<Vec<String>> {
    // MEASURED: the samples of each mode and where its glyphs' origins snap (aliased: one sample
    // a pixel, origin to the pixel; gray (natural symmetric): 4 x 4, unsnapped; natural: 8 x 1,
    // y to the pixel; GDI classic and natural: 8 x 1, origin to the pixel; ClearType: 6 x 5,
    // origin to the sample)
    let (g, clause) = match mode {
        "aliased" => (Grid { nx: 1, ny: 1, qx: 1, qy: 1 }, "DW.RUN.ALIASED"),
        "gray" => (Grid { nx: 4, ny: 4, qx: 0, qy: 0 }, "DW.RUN.GRAY"),
        "natural" => (Grid { nx: 8, ny: 1, qx: 0, qy: 1 }, "DW.RUN.NATURAL"),
        "gdi" | "gdinatural" => (Grid { nx: 8, ny: 1, qx: 1, qy: 1 }, "DW.RUN.GDI"),
        _ => (Grid { nx: 6, ny: 5, qx: 6, qy: 5 }, "DW.RUN.CLEARTYPE"),
    };
    hit(clause);
    // MEASURED: the pen starts at the unsnapped origin and moves by each advance; each glyph
    // is drawn at the pen snapped as the mode snaps the origin
    if run.len() > 1 {
        hit("DW.RUN.PEN");
    }
    let mut lit = std::collections::BTreeSet::new();
    for r in rects(p, em, g, x4, y4, run) {
        lit.extend(aliased(&r)?);
    }
    // each pixel's (or ClearType subpixel's) value
    let mut val: std::collections::BTreeMap<(i64, i64), i64> = Default::default();
    let cleartype = mode == "cleartype";
    if cleartype {
        // MEASURED: a subpixel is 2 x 5 samples, the 5 rows weighted 4 9 10 9 4 (/36); each
        // subpixel's value is the sum of it and its two neighbours, of 255 for 6 x 36
        const W: [i64; 5] = [4, 9, 10, 9, 4];
        let mut acc: std::collections::BTreeMap<(i64, i64), i64> = Default::default();
        for &(x, y) in &lit {
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
        for &(x, y) in &lit {
            *acc.entry((x.div_euclid(g.nx), y.div_euclid(g.ny))).or_default() += 1;
        }
        for (k, v) in acc {
            val.insert(k, (2 * 255 * v + n).div_euclid(2 * n));
        }
    }
    val.retain(|_, v| *v != 0);
    let bpp = if cleartype { 3 } else { 1 };
    let mut out = Vec::new();
    let xs = val.keys().map(|k| k.0.div_euclid(bpp));
    let ys = val.keys().map(|k| k.1);
    match (xs.clone().min(), ys.clone().min(), xs.max(), ys.max()) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => {
            out.push(format!("bounds hr 00000000 {x0} {y0} {} {}", x1 + 1, y1 + 1));
            for y in y0..=y1 {
                out.push((x0 * bpp..(x1 + 1) * bpp).map(|x| format!("{:02x}", val.get(&(x, y)).copied().unwrap_or(0))).collect());
            }
        }
        _ => {
            // MEASURED: nothing lit: empty bounds at the origin rounded to the pixel
            hit("DW.RUN.EMPTY");
            let snap = |v: i64| (v + 2).div_euclid(4);
            let (x, y) = (snap(x4), snap(y4));
            out.push(format!("bounds hr 00000000 {x} {y} {x} {y}"));
        }
    }
    Ok(out)
}
