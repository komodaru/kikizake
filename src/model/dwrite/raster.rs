//! IDWriteGlyphRunAnalysis: the alpha texture of a glyph run (the fonts' glyphs are
//! rectangles, so coverage is a matter of where their edges fall).
use super::super::{Fail, Res};
use crate::spec::hit;
use crate::ttf::Parsed;

/// each glyph's rectangles in 26.6 pixels (x0, y0, x1, y1), y down: its points scaled and
/// rounded to 1/64 pixel, at the pen position, the pen moved by each advance (adv4 / 4 pixels)
fn rects(p: &Parsed, em: i64, (ox, oy): (i64, i64), run: &[(u16, i64)]) -> Vec<Vec<(i64, i64, i64, i64)>> {
    let sc = |v: i16| (v as i64 * em * 64 * 2 + p.upm as i64).div_euclid(2 * p.upm as i64);
    let mut pen = ox;
    let mut v = Vec::new();
    for &(g, adv) in run {
        let g = if g >= p.num_glyphs { 0 } else { g };
        let at = (pen + 32) & !63;
        let mut gv = Vec::new();
        if let Some(Some((_, contours))) = p.glyphs.get(g as usize) {
            for c in contours {
                let xs = c.iter().map(|q| q.0);
                let ys = c.iter().map(|q| q.1);
                let (fx0, fx1) = (xs.clone().min().unwrap_or(0), xs.max().unwrap_or(0));
                let (fy0, fy1) = (ys.clone().min().unwrap_or(0), ys.max().unwrap_or(0));
                gv.push((at + sc(fx0), oy - sc(fy1), at + sc(fx1), oy - sc(fy0)));
            }
        }
        v.push(gv);
        pen += adv * 16;
    }
    v
}

fn texture(lit: impl Fn(i64, i64) -> u8, x0: i64, y0: i64, x1: i64, y1: i64, out: &mut Vec<String>) {
    out.push(format!("bounds hr 00000000 {x0} {y0} {x1} {y1}"));
    for y in y0..y1 {
        out.push((x0..x1).map(|x| format!("{:02x}", lit(x, y))).collect());
    }
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
    // MEASURED: aliased, the baseline origin snapped to the nearest pixel corner
    let snap = |v: i64| (v + 2).div_euclid(4);
    // MEASURED: the pen starts at the unsnapped origin and moves by each advance; each glyph
    // is drawn at the pen snapped to the nearest pixel
    if run.len() > 1 {
        hit("DW.RUN.PEN");
    }
    let r = rects(p, em, (x4 * 16, snap(y4) * 64), run);
    let mut out = Vec::new();
    match mode {
        "aliased" => {
            hit("DW.RUN.ALIASED");
            let mut lit = std::collections::BTreeSet::new();
            for g in &r {
                lit.extend(aliased(g)?);
            }
            match (lit.iter().map(|q| q.0).min(), lit.iter().map(|q| q.1).min(), lit.iter().map(|q| q.0).max(), lit.iter().map(|q| q.1).max()) {
                (Some(x0), Some(y0), Some(x1), Some(y1)) => texture(|x, y| if lit.contains(&(x, y)) { 0xff } else { 0 }, x0, y0, x1 + 1, y1 + 1, &mut out),
                _ => {
                    hit("DW.RUN.EMPTY");
                    let (x, y) = (snap(x4), snap(y4));
                    out.push(format!("bounds hr 00000000 {x} {y} {x} {y}"));
                }
            }
        }
        _ => return Err(Fail::Unmodeled("DW.RUN.AA")),
    }
    Ok(out)
}
