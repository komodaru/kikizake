//! IDWriteGlyphRunAnalysis' modes (spec/dwrite.txt DW.RUN.GRAY, NATURAL, GDI, CLEARTYPE,
//! CLEARTYPE.6X1, FIT.GDI) and the aliased rasterizer's dropout control (DW.RUN.DROPOUT,
//! DROPOUT.SIZE), each over the whole domain it names (kind exhaustive).
mod pairs;
use pairs::zip_eq;
use std::collections::{BTreeMap, BTreeSet};
use kikizake::generators::dwrite::font;
use kikizake::model::dwrite::raster::{aliased, fixed16, rects, Dropout, Grid, Scale};
use kikizake::ttf::parse;

type Rect = (i64, i64, i64, i64);

/// a / b rounded to the nearest, a half up (b > 0)
fn round(a: i64, b: i64) -> i64 {
    (2 * a + b).div_euclid(2 * b)
}

/// DW.RUN.DROPOUT written from the clause: the pixels whose centers are inside a rectangle;
/// then each row, top to bottom, and each column, left to right: a span of the rectangles'
/// union on its center line holding no pixel center lights the pixel before it (row: left, when
/// that pixel's center is at or right of the union's left edge, else the one after) or after it
/// (column: below, when that pixel's center is at or above the union's bottom edge, else the one
/// before), none when either is already lit
fn dropout_ref(r: &[Rect]) -> BTreeSet<(i64, i64)> {
    let lo = r.iter().map(|q| q.0.min(q.1)).min().unwrap().div_euclid(64) - 3;
    let hi = r.iter().map(|q| q.2.max(q.3)).max().unwrap().div_euclid(64) + 3;
    let centre = |k: i64| 64 * k + 32;
    let mut lit = BTreeSet::new();
    for y in lo..=hi {
        for x in lo..=hi {
            if r.iter().any(|q| q.0 <= centre(x) && centre(x) <= q.2 && q.1 <= centre(y) && centre(y) <= q.3) {
                lit.insert((x, y));
            }
        }
    }
    let left = r.iter().map(|q| q.0).min().unwrap();
    let bottom = r.iter().map(|q| q.3).max().unwrap();
    for row in [true, false] {
        for a in lo..=hi {
            // (the union of the rectangles' extents along the line at a's center)
            let mut ext: Vec<(i64, i64)> = r
                .iter()
                .filter(|q| if row { q.1 <= centre(a) && centre(a) <= q.3 } else { q.0 <= centre(a) && centre(a) <= q.2 })
                .map(|q| if row { (q.0, q.2) } else { (q.1, q.3) })
                .collect();
            ext.sort();
            let mut union: Vec<(i64, i64)> = Vec::new();
            for e in ext {
                match union.last_mut() {
                    Some(u) if e.0 <= u.1 => u.1 = u.1.max(e.1),
                    _ => union.push(e),
                }
            }
            for (s0, s1) in union {
                if (lo..=hi).any(|k| s0 <= centre(k) && centre(k) <= s1) {
                    continue;
                }
                // (the pixels whose centers are just before and just after the span)
                let after = (lo..=hi + 1).find(|&k| centre(k) > s1).unwrap();
                let before = after - 1;
                let at = |k: i64| if row { (k, a) } else { (a, k) };
                if lit.contains(&at(before)) || lit.contains(&at(after)) {
                    continue;
                }
                let k = if row {
                    if centre(before) >= left { before } else { after }
                } else if centre(after) <= bottom {
                    after
                } else {
                    before
                };
                lit.insert(at(k));
            }
        }
    }
    lit
}

/// DW.RUN.DROPOUT: every rectangle with corners on 1/8 pixel steps across -1..3 pixels, and
/// every pair of rectangles (a glyph of two contours) with corners on 1/4 pixel steps across
/// 0..1.5 pixels: the pixels lit with dropout control are the clause's
#[test]
fn dropout_lights_one_pixel_beside_each_empty_span() {
    let rects_on = |step: i64, lo: i64, hi: i64| {
        let vs: Vec<i64> = (lo..=hi).step_by(usize::try_from(step).unwrap()).collect();
        let mut v = Vec::new();
        for &x0 in &vs {
            for &x1 in vs.iter().filter(|&&x| x >= x0) {
                for &y0 in &vs {
                    for &y1 in vs.iter().filter(|&&y| y >= y0) {
                        v.push((x0, y0, x1, y1));
                    }
                }
            }
        }
        v
    };
    for r in rects_on(8, -64, 192) {
        assert_eq!(aliased(&[r], Dropout::On).unwrap(), dropout_ref(&[r]), "{r:?}");
    }
    let two = rects_on(16, 0, 96);
    for (i, &a) in two.iter().enumerate() {
        for &b in &two[i..] {
            assert_eq!(aliased(&[a, b], Dropout::On).unwrap(), dropout_ref(&[a, b]), "{a:?} {b:?}");
        }
    }
}

/// DW.RUN.FIT.GDI: every design value -32768..32767 at every size 1..200 pixels an em for each
/// generated font's units per em: em / upm rounded half up to 16.16, 64 v times it rounded to
/// 26.6 half up, an exact half below zero away from zero
#[test]
fn the_gdi_unfitted_scale_is_a_16_16_number() {
    let upms: BTreeSet<u16> = (0..64).map(|s| parse(&font(s).bytes()).unwrap().upm).collect();
    for &upm in &upms {
        let u = i64::from(upm);
        for em in 1..=200i64 {
            let s16 = round(em * 65536, u);
            for v in -32768..=32767i64 {
                let m = 64 * v * s16;
                let want = m.signum() * round(m.abs(), 65536);
                assert_eq!(fixed16(v, em, u), want, "{upm} {em} {v}");
            }
        }
    }
}

/// the lines the model answers for these lines after loading font `seed`
fn run(seed: u32, lines: &[String]) -> Vec<String> {
    let hex: String = font(seed).bytes().iter().map(|x| format!("{x:02x}")).collect();
    let text = format!("case c\ndwfont {hex}\n{}\nend\n", lines.join("\n"));
    kikizake::model::run_case(&kikizake::case::parse_cases(&text)[0]).unwrap()
}

/// the answers of each dwrun line: its lines from its "dwrun hr" on
fn runs(out: &[String]) -> Vec<Vec<String>> {
    let mut v: Vec<Vec<String>> = Vec::new();
    for l in out.iter().skip(2) {
        if l.starts_with("dwrun hr") {
            v.push(Vec::new());
        }
        if let Some(last) = v.last_mut() {
            last.push(l.clone());
        }
    }
    v
}

/// how a mode counts its samples into pixels
#[derive(Clone, Copy)]
enum Count {
    /// DW.RUN.GRAY, NATURAL, GDI: round(lit * 255 / samples)
    Gray,
    /// DW.RUN.CLEARTYPE: 2 x 5 samples a subpixel, rows weighted 4 9 10 9 4 / 36;
    /// round(255 * (its weight and its two neighbours') / 6)
    ClearType,
    /// DW.RUN.CLEARTYPE.6X1: (255 n + 3) / 6 of the n lit among its 2 samples and its
    /// neighbours' 4
    ClearType6x1,
}

/// each mode's samples a pixel, where its origin snaps (1/q pixel; 0 unsnapped), as its clause
/// says, and how it counts
fn mode(m: &str) -> (Grid, Count) {
    let g = |nx, ny, qx, qy| Grid { nx, ny, qx, qy };
    match m {
        "gray" => (g(4, 4, 0, 0), Count::Gray),
        "natural" => (g(8, 1, 0, 1), Count::Gray),
        "gdi" | "gdinatural" => (g(8, 1, 1, 1), Count::Gray),
        "cleartype" => (g(6, 5, 6, 5), Count::ClearType),
        "ctnatural" => (g(6, 1, 6, 1), Count::ClearType6x1),
        "ctgdi" | "ctgdinatural" => (g(6, 1, 1, 1), Count::ClearType6x1),
        _ => unreachable!(),
    }
}

/// a pixel's (subpixel's) nonzero values of the samples lit
fn count(lit: &BTreeSet<(i64, i64)>, g: Grid, c: Count) -> BTreeMap<(i64, i64), i64> {
    let mut v = BTreeMap::new();
    match c {
        Count::Gray => {
            let mut n: BTreeMap<(i64, i64), i64> = BTreeMap::new();
            for &(x, y) in lit {
                *n.entry((x.div_euclid(g.nx), y.div_euclid(g.ny))).or_default() += 1;
            }
            for (k, n) in n {
                v.insert(k, round(n * 255, g.nx * g.ny));
            }
        }
        Count::ClearType | Count::ClearType6x1 => {
            let weight = [4, 9, 10, 9, 4];
            let mut w: BTreeMap<(i64, i64), i64> = BTreeMap::new();
            for &(x, y) in lit {
                let r = if g.ny == 1 { 1 } else { weight[usize::try_from(y.rem_euclid(5)).unwrap()] };
                *w.entry((x.div_euclid(2), y.div_euclid(g.ny))).or_default() += r;
            }
            let subs: BTreeSet<(i64, i64)> = w.keys().flat_map(|&(s, y)| [(s - 1, y), (s, y), (s + 1, y)]).collect();
            for (s, y) in subs {
                let t: i64 = (s - 1..=s + 1).map(|k| w.get(&(k, y)).copied().unwrap_or(0)).sum();
                v.insert((s, y), if matches!(c, Count::ClearType) { round(255 * t, 6 * 36) } else { (255 * t + 3).div_euclid(6) });
            }
        }
    }
    v.retain(|_, x| *x != 0);
    v
}

/// the texture of glyph `gid` alone in mode `m` at em pixels an em and origin (x4, y4) quarter
/// pixels, from the clauses: the aliased rasterizer on the mode's grid, dropout control in the
/// texture only up to 32 pixels an em (DW.RUN.DROPOUT.SIZE) and in the bounds at every size;
/// GDI modes with grid fit disabled scaled by fixed16 (DW.RUN.FIT.GDI)
fn want(p: &kikizake::ttf::Parsed, m: &str, nofit: bool, em: i64, x4: i64, y4: i64, gid: u16) -> Vec<String> {
    let (g, c) = mode(m);
    let scale = if nofit { Scale::Fixed16 } else { Scale::Direct };
    let (mut tex, mut bnd) = (BTreeSet::new(), BTreeSet::new());
    for r in rects(p, em, g, scale, x4, y4, &[(gid, 40)]) {
        tex.extend(aliased(&r, if em <= 32 { Dropout::On } else { Dropout::Off }).unwrap());
        bnd.extend(aliased(&r, Dropout::On).unwrap());
    }
    let (val, on) = (count(&tex, g, c), count(&bnd, g, c));
    let bpp = if matches!(c, Count::Gray) { 1 } else { 3 };
    let mut out = vec!["dwrun hr 00000000".to_string()];
    let x0 = on.keys().map(|k| k.0.div_euclid(bpp)).min();
    let x1 = on.keys().map(|k| k.0.div_euclid(bpp)).max();
    let y0 = on.keys().map(|k| k.1).min();
    let y1 = on.keys().map(|k| k.1).max();
    match (x0, y0, x1, y1) {
        (Some(x0), Some(y0), Some(x1), Some(y1)) => {
            out.push(format!("bounds hr 00000000 {x0} {y0} {} {}", x1 + 1, y1 + 1));
            for y in y0..=y1 {
                out.push((x0 * bpp..(x1 + 1) * bpp).map(|x| format!("{:02x}", val.get(&(x, y)).copied().unwrap_or(0))).collect());
            }
        }
        _ => {
            let (x, y) = ((x4 + 2).div_euclid(4), (y4 + 2).div_euclid(4));
            out.push(format!("bounds hr 00000000 {x} {y} {x} {y}"));
        }
    }
    out
}

/// DW.RUN.GRAY, NATURAL, GDI, CLEARTYPE, CLEARTYPE.6X1, DROPOUT.SIZE, FIT.GDI: every glyph of
/// 2 fonts alone, at every em 1..40 and the origins (0, 0), (1, 3), (2, 2), (3, 1) quarter
/// pixels (each quarter across and down), in each mode, and each GDI mode with grid fit
/// disabled: the texture the clauses compose
#[test]
fn each_mode_is_the_aliased_rasterizer_on_its_grid() {
    let modes = ["gray", "natural", "gdi", "gdinatural", "cleartype", "ctnatural", "ctgdi", "ctgdinatural"];
    for seed in 0..2 {
        let f = font(seed);
        let p = parse(&f.bytes()).unwrap();
        for m in modes {
            let gdi = m.contains("gdi");
            for nofit in [false, true].into_iter().filter(|&n| gdi || !n) {
                let mut lines = Vec::new();
                let mut wants = Vec::new();
                for gid in 0..p.num_glyphs {
                    for em in 1..=40i64 {
                        for q in 0..4i64 {
                            let (x4, y4) = (q, (4 - q) % 4);
                            let t = if nofit { format!("{m}.nofit") } else { m.to_string() };
                            lines.push(format!("dwrun {t} {em} {x4} {y4} {gid} 40"));
                            wants.push(want(&p, m, nofit, em, x4, y4, gid));
                        }
                    }
                }
                for (got, (w, l)) in zip_eq(runs(&run(seed, &lines)), zip_eq(wants, &lines)) {
                    assert_eq!(got, w, "{seed} {l}");
                }
            }
        }
    }
}

/// DW.RUN.DROPOUT.SIZE: glyphs of stems and bars thinner than a pixel (1, 3, 7, 15 and 31
/// units of 2048 across and down, and one of each crossing), in each mode at every em 1..64 and
/// the origins as above: the texture has dropout control up to 32 pixels an em and none above,
/// the bounds have it at every size
#[test]
fn dropout_control_ends_above_32_pixels_an_em() {
    use kikizake::model::dwrite::raster::{run as texture, Mode};
    let mut glyphs = Vec::new();
    for w in [1i16, 3, 7, 15, 31] {
        let stem = vec![(100, -200), (100 + w, -200), (100 + w, 1400), (100, 1400)];
        let bar = vec![(-200, 300), (1400, 300), (1400, 300 + w), (-200, 300 + w)];
        glyphs.push(Some(((0, 0, 0, 0), vec![stem.clone()])));
        glyphs.push(Some(((0, 0, 0, 0), vec![bar.clone()])));
        glyphs.push(Some(((0, 0, 0, 0), vec![stem, bar])));
    }
    let p = kikizake::ttf::Parsed { upm: 2048, num_glyphs: u16::try_from(glyphs.len()).unwrap(), glyphs, ..Default::default() };
    let mut empty = 0;
    for m in ["gray", "natural", "gdi", "gdinatural", "cleartype", "ctnatural", "ctgdi", "ctgdinatural"] {
        let (mode, fit) = Mode::parse(m).unwrap();
        for gid in 0..p.num_glyphs {
            for em in 1..=64i64 {
                for q in 0..4i64 {
                    let (x4, y4) = (q, (4 - q) % 4);
                    let got = texture(&p, mode, fit, em, x4, y4, &[(gid, 40)]).unwrap();
                    let w = want(&p, m, false, em, x4, y4, gid);
                    assert_eq!(got, w[1..], "{m} {gid} {em} {q}");
                    if em > 32 && w.len() > 2 && w[2..].iter().all(|r| r.bytes().all(|b| b == b'0')) {
                        empty += 1;
                    }
                }
            }
        }
    }
    // (above 32 pixels an em some of these textures are empty where their bounds are not)
    assert!(empty > 0);
}
