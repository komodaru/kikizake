//! IDWriteGlyphRunAnalysis' runs (spec/dwrite.txt DW.RUN.EMPTY, FIT, PEN, TEXTURE) and
//! GetGdiCompatibleMetrics' rounding (DW.GDI.METRICS), each over the whole domain it names
//! (kind exhaustive).
use kikizake::generators::dwrite::font;
use kikizake::model::dwrite::gdi_round;
use kikizake::ttf::parse;



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

const MODES: [&str; 12] = ["aliased", "gray", "cleartype", "natural", "gdi", "gdinatural", "ctaliased", "ctnatural", "ctgdi", "ctgdinatural", "gray3x1", "ct1x1"];

/// DW.RUN.EMPTY: a run of nothing lit (the space's and the no-break space's empty glyphs, 8
/// and 12) in every mode but the other texture's, at every origin -16..16 quarter pixels each
/// way: empty bounds at the origin rounded to the pixel, (q + 2) div 4
#[test]
fn nothing_lit_is_empty_bounds_at_the_origin() {
    for seed in 0..4 {
        let mut lines = Vec::new();
        let mut want = Vec::new();
        for mode in &MODES[..10] {
            for x4 in -16..=16i64 {
                for y4 in -16..=16i64 {
                    lines.push(format!("dwrun {mode} 20 {x4} {y4} 8 40 12 40"));
                    let (x, y) = ((x4 + 2).div_euclid(4), (y4 + 2).div_euclid(4));
                    want.push(vec!["dwrun hr 00000000".to_string(), format!("bounds hr 00000000 {x} {y} {x} {y}")]);
                }
            }
        }
        assert_eq!(runs(&run(seed, &lines)), want, "{seed}");
    }
}

/// DW.RUN.TEXTURE: a grayscale analysis asked for the ClearType texture and a ClearType one
/// asked for the aliased texture: bounds 0 0 0 0, for every glyph of 4 fonts at every em
/// 1..40 and every origin 0..4 quarter pixels each way
#[test]
fn the_other_texture_is_empty() {
    for seed in 0..4 {
        let mut lines = Vec::new();
        for mode in ["gray3x1", "ct1x1"] {
            for g in 0..15 {
                for em in 1..=40 {
                    for q in 0..16 {
                        lines.push(format!("dwrun {mode} {em} {} {} {g} 40", q % 4, q / 4));
                    }
                }
            }
        }
        let out = runs(&run(seed, &lines));
        assert_eq!(out.len(), lines.len());
        assert!(out.iter().all(|r| r == &["dwrun hr 00000000", "bounds hr 00000000 0 0 0 0"]), "{seed}");
    }
}

/// DW.RUN.FIT: DWRITE_GRID_FIT_MODE_ENABLED draws as DEFAULT in every mode, DISABLED as
/// DEFAULT in aliased, gray, ClearType and natural: every glyph of 2 fonts at every em 1..32
/// and every origin 0..4 quarter pixels across
#[test]
fn grid_fit_draws_as_the_default() {
    for seed in 0..2 {
        for mode in &MODES[..10] {
            let mut lines = Vec::new();
            for g in 0..15 {
                for em in 1..=32 {
                    for q in 0..4 {
                        let rest = format!("{em} {q} 1 {g} 40");
                        lines.push(format!("dwrun {mode} {rest}"));
                        lines.push(format!("dwrun {mode}.fit {rest}"));
                        if matches!(*mode, "aliased" | "gray" | "cleartype" | "natural") {
                            lines.push(format!("dwrun {mode}.nofit {rest}"));
                        }
                    }
                }
            }
            let out = runs(&run(seed, &lines));
            let k = if matches!(*mode, "aliased" | "gray" | "cleartype" | "natural") { 3 } else { 2 };
            for c in out.chunks(k) {
                assert!(c.iter().all(|r| r == &c[0]), "{seed} {mode} {c:?}");
            }
        }
    }
}

/// the pixels an aliased run lights
fn lit(r: &[String]) -> std::collections::BTreeSet<(i64, i64)> {
    let mut s = std::collections::BTreeSet::new();
    let b: Vec<i64> = r[1].split(' ').skip(3).map(|v| v.parse().unwrap()).collect();
    for (j, row) in r[2..].iter().enumerate() {
        for i in 0..row.len() / 2 {
            if &row[2 * i..2 * i + 2] != "00" {
                s.insert((b[0] + i64::try_from(i).unwrap(), b[1] + i64::try_from(j).unwrap()));
            }
        }
    }
    s
}

/// DW.RUN.PEN: an aliased run of two glyphs is the two glyphs each alone, the second at the
/// origin moved by the first's advance (quarter pixels, not rounded): every pair of glyphs of
/// 2 fonts, advances -8..8 quarter pixels, origins 0..4 quarter pixels, em 12
#[test]
fn the_pen_moves_by_each_advance() {
    for seed in 0..2 {
        for em in [12] {
            let mut lines = Vec::new();
            let mut idx = Vec::new();
            for g1 in 0..15 {
                for g2 in 0..15 {
                    for adv in -8..=8i64 {
                        for x4 in 0..4i64 {
                            lines.push(format!("dwrun aliased {em} {x4} 1 {g1} {adv} {g2} 40"));
                            lines.push(format!("dwrun aliased {em} {x4} 1 {g1} 40"));
                            lines.push(format!("dwrun aliased {em} {} 1 {g2} 40", x4 + adv));
                            idx.push((g1, g2, adv, x4));
                        }
                    }
                }
            }
            let out = runs(&run(seed, &lines));
            for (c, at) in out.chunks(3).zip(idx) {
                let both: std::collections::BTreeSet<_> = lit(&c[1]).union(&lit(&c[2])).copied().collect();
                assert_eq!(lit(&c[0]), both, "{seed} {em} {at:?}");
            }
        }
    }
}

/// DW.GDI.METRICS: every value -32768..65535 at every size 1..200 pixels an em for each
/// generated font's units per em: s = f32(upm) / f32(ppem), round(round(v / s) s), each
/// division and product rounded to f32 once (computed exactly in f64 and rounded), each
/// round half up
#[test]
fn gdi_metrics_round_through_pixels() {
    let upms: std::collections::BTreeSet<u16> = (0..64).map(|s| parse(&font(s).bytes()).unwrap().upm).collect();
    // (a binary64 rounded to the nearest binary32: the quotient and product of binary32s are
    // exact in binary64 or rounded there finely enough that this is the binary32 one)
    let to32 = |x: f64| x as f32;
    for &upm in &upms {
        for ppem in 1..=200u16 {
            let s = f32::from(upm) / f32::from(ppem);
            let r = gdi_round(upm, i64::from(ppem));
            for v in -32768..=65535i32 {
                let q = f64::from(v) / f64::from(s);
                let r1 = (f64::from(to32(q)) + 0.5).floor();
                let want = (f64::from(to32(r1 * f64::from(s))) + 0.5).floor();
                assert_eq!(f64::from(r(v)), want, "{upm} {ppem} {v}");
            }
        }
    }
}
