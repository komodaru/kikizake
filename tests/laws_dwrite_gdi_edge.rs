//! GetGdiCompatibleGlyphMetrics' metrics back in design units (spec/dwrite.txt DW.GDI.EDGE),
//! over the whole domain each law names (kind exhaustive).
mod pairs;
use pairs::zip_eq;
use kikizake::generators::dwrite::font;
use kikizake::model::dwrite::{classic_px, design_units, down6, exact_fraction, gdi_edges, phantom_px, GlyphMetrics, Pixels, Step, ToEm};
use kikizake::ttf::parse;

/// a binary64 rounded to the nearest binary32 (each product and quotient of binary32s below is
/// taken exactly or finely enough in binary64 that this is the binary32 one)
fn r32(x: f64) -> f64 {
    f64::from(x as f32)
}

fn f(v: i64) -> f64 {
    f64::from(i32::try_from(v).unwrap())
}

/// the clause's 16.16 of a float32 em fraction: times 65536 twice (exact), truncated, then
/// floor((t + 0x8000) / 65536)
fn fixed(x: f64) -> i64 {
    let t = (x * 65536.0 * 65536.0).trunc() as i64;
    (t + 0x8000).div_euclid(65536)
}

/// float32 1 / (p p) times p, p = ppem
fn inverse(ppem: i64) -> f64 {
    r32(r32(1.0 / r32(f(ppem) * f(ppem))) * f(ppem))
}

/// DW.GDI.EDGE: k whole pixels (-512..512), a box edge of e sixths (-3072..3072, both signs)
/// and v 1/65536 pixels across (-131072..131072) at every ppem 1..4096: the float32 em
/// fraction the clause writes (x times 1 / (p p) times p, the box's sixths times float32 1/6,
/// across the em's y: times -1)
#[test]
fn each_edge_is_a_float32_fraction() {
    let sixth = r32(1.0 / 6.0);
    let inv = r32(1.0 / 65536.0);
    for ppem in 1..=4096i64 {
        let to = ToEm::new(ppem);
        let m = inverse(ppem);
        for k in -512..=512i64 {
            assert_eq!(to.whole(k), fixed(r32(f(k) * m)), "{ppem} {k}");
        }
        for e in -3072..=3072i64 {
            let ex = r32(f(e) * sixth);
            assert_eq!(to.sixths(e, 1.0), fixed(r32(ex * m)), "{ppem} {e}");
            assert_eq!(to.sixths(e, -1.0), fixed(r32(ex * -m)), "{ppem} {e}");
        }
        for v in -131072..=131072i64 {
            assert_eq!(to.across(v), fixed(r32(r32(f(v) * inv) * -m)), "{ppem} {v}");
        }
    }
}

/// the units per em of the generated fonts
fn upms() -> std::collections::BTreeSet<i64> {
    (0..64).map(|s| i64::from(parse(&font(s).bytes()).unwrap().upm)).collect()
}

/// DW.GDI.EDGE: design units (upm E + 0x8000) >> 16 for every fraction -2^20..2^20 at each
/// generated font's units per em; a classic advance's exact fraction of k whole pixels
/// (-1024..1024 at every ppem 1..255) the nearest 16.16, a half up; past 255 ppem the classic
/// advance is the 26.6 phantom point rounded to the pixel (every advance 0..4095 at every ppem
/// 256..4096 for each units per em)
#[test]
fn fractions_back_in_design_units() {
    for upm in upms() {
        for e in -(1i64 << 20)..=1 << 20 {
            assert_eq!(i64::from(design_units(e, upm)), (upm * e + 0x8000) >> 16, "{upm} {e}");
        }
    }
    for ppem in 1..=255i64 {
        for k in -1024..=1024i64 {
            // (the nearest of 65536 k / ppem, a half up: twice the numerator against the
            // denominator)
            let n = 65536 * k;
            let lo = n.div_euclid(ppem);
            let want = if 2 * (n - lo * ppem) >= ppem { lo + 1 } else { lo };
            assert_eq!(exact_fraction(k, ppem), want, "{ppem} {k}");
        }
    }
    for upm in upms() {
        for ppem in 256..=4096i64 {
            for adv in 0..=4095i64 {
                assert_eq!(classic_px(adv, ppem, upm), phantom_px(adv, ppem, upm, Step::Pixel), "{upm} {ppem} {adv}");
            }
        }
    }
}

/// the clause's metrics of a glyph's pixels: the edges' em fractions, the side bearings their
/// differences, each back in design units on its own
fn want(ppem: i64, upm: i64, sideways: bool, classic: bool, px: Pixels) -> GlyphMetrics {
    let m = inverse(ppem);
    let whole = |k: i64| fixed(r32(f(k) * m));
    let sixths = |e: i64, s: f64| fixed(r32(r32(f(e) * r32(1.0 / 6.0)) * (s * m)));
    let across = |v: i64| fixed(r32(r32(f(v) * r32(1.0 / 65536.0)) * -m));
    // ((v + 3) / 6 truncated toward zero)
    let six = |v: i64| (v + 3) / 6;
    let (x0, y0, x1, y1) = px.bb;
    let (eo, eah, eaw, left, top, right, bottom) = if sideways {
        // (the origin of k pixels from the box's top column L: L 65536 / 6 less
        // ((64 L + 384 k) 1024 + 3) / 6, each truncated; the advance height of k pixels the
        // scaler's -6 65536 k sixths divided back)
        let vo = (y0 * 65536) / 6 - six((64 * y0 + 384 * px.voy) * 1024);
        (across(vo), across(six(-6 * 65536 * px.advh)), whole(px.adv), whole(x0), sixths(y0, -1.0), whole(x1), sixths(y1, -1.0))
    } else {
        let eaw = if classic { exact_fraction(px.adv, ppem) } else { whole(px.adv) };
        (whole(px.voy), whole(px.advh), eaw, sixths(x0, 1.0), whole(-y0), sixths(x1, 1.0), whole(-y1))
    };
    let fu = |e: i64| i32::try_from((upm * e + 0x8000) >> 16).unwrap();
    GlyphMetrics {
        adv: fu(eaw).cast_unsigned(),
        lsb: fu(left),
        rsb: fu(eaw - right),
        tsb: fu(eo - top),
        advh: fu(eah).cast_unsigned(),
        bsb: fu(bottom - eo + eah),
        voy: fu(eo),
    }
}

/// DW.GDI.EDGE: every advance, origin and advance height -1..3 pixels and every box of
/// columns and rows of -12, -7, -3, 0, 1, 2, 5, 11 samples (x0 <= x1, y0 <= y1; each residue
/// mod 6), axis aligned (classic and not) and sideways, at every ppem 1..300 (a step of 7) for
/// each units per em: the metrics the clause composes; and the sideways advance height
/// (1 - 65536 k) / 65536 pixels for k >= 1, -65536 k / 65536 for k <= 0 (every k -4096..4096)
#[test]
fn metrics_are_the_edges_fractions() {
    for k in -4096..=4096i64 {
        let want = if k >= 1 { 1 - 65536 * k } else { -65536 * k };
        assert_eq!(down6(-6 * 65536 * k), want, "{k}");
    }
    let side: Vec<i64> = vec![-12, -7, -3, 0, 1, 2, 5, 11];
    let mut boxes = Vec::new();
    for &x0 in &side {
        for &x1 in side.iter().filter(|&&x| x >= x0) {
            for &y0 in &side {
                for &y1 in side.iter().filter(|&&y| y >= y0) {
                    boxes.push((x0, y0, x1, y1));
                }
            }
        }
    }
    for upm in upms() {
        for ppem in (1..=300i64).step_by(7) {
            for adv in -1..=3 {
                for voy in -1..=3 {
                    for advh in -1..=3 {
                        for &bb in &boxes {
                            let px = Pixels { adv, voy, advh, bb };
                            for (sideways, classic) in [(false, false), (false, true), (true, false)] {
                                assert_eq!(gdi_edges(ppem, upm, sideways, classic, px), want(ppem, upm, sideways, classic, px), "{upm} {ppem} {px:?} {sideways} {classic}");
                            }
                        }
                    }
                }
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

/// DW.GDI.EDGE: at ppem == upm (em upm, 1 pixel a dip; em 1, upm pixels a dip), not sideways,
/// classic and natural, every glyph of each of 64 fonts: the design metrics as they are
#[test]
fn at_the_em_the_design_metrics() {
    for seed in 0..64 {
        let upm = parse(&font(seed).bytes()).unwrap().upm;
        if upm > 4096 {
            continue;
        }
        let n = parse(&font(seed).bytes()).unwrap().num_glyphs;
        let gs: Vec<String> = (0..n).map(|g| g.to_string()).collect();
        let gs = gs.join(" ");
        let mut lines = vec![format!("dwglyph 0 {gs}")];
        for nat in 0..2 {
            lines.push(format!("dwgdiglyph {upm} 1 {nat} 0 {gs}"));
            lines.push(format!("dwgdiglyph 1 {upm} {nat} 0 {gs}"));
        }
        let out = run(seed, &lines);
        let design: Vec<&String> = out.iter().skip(3).take(usize::from(n)).collect();
        let rest = &out[3 + usize::from(n)..];
        assert_eq!(rest.len(), 4 * (1 + usize::from(n)), "{seed}");
        for c in rest.chunks(1 + usize::from(n)) {
            assert_eq!(c[0], "dwgdiglyph hr 00000000", "{seed}");
            for (a, b) in zip_eq(&c[1..], &design) {
                assert_eq!(&a, b, "{seed}");
            }
        }
    }
}
