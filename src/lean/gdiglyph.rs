//! GDI's glyph and arc mirrors (spec/lean/Kikizake/Gdi/Glyph.lean, Arc.lean).
use super::{arg, sampled, Domain, Mirror};
use crate::model::{gdiarc, gdifont};

const GLYPH: &str = "spec/lean/Kikizake/Gdi/Glyph.lean";
const ARC: &str = "spec/lean/Kikizake/Gdi/Arc.lean";

/// a rectangle (x0, y0, x1, y1) in font units
type Rect = (i64, i64, i64, i64);

/// the suite's font of one rectangle glyph per character ('A' on): units per em, ascent,
/// descent (OS/2's usWinAscent and usWinDescent); each character its advance and rectangles
fn rect_font(upm: i64, a: i64, d: i64, chars: &[(i64, Vec<Rect>)]) -> Option<crate::ttf::Parsed> {
    let i16_of = |v: i64| i16::try_from(v).ok();
    let mut glyphs = vec![crate::ttf::Glyph::default()];
    for (adv, r) in chars {
        let rects = r.iter().map(|q| Some((i16_of(q.0)?, i16_of(q.1)?, i16_of(q.2)?, i16_of(q.3)?))).collect::<Option<Vec<_>>>()?;
        glyphs.push(crate::ttf::Glyph { adv: u16::try_from(*adv).ok()?, rects, vert: (0, 0) });
    }
    let cmap = (0..chars.len()).map(|i| Some((0x41u32.checked_add(u32::try_from(i).ok()?)?, u16::try_from(i.checked_add(1)?).ok()?))).collect::<Option<Vec<_>>>()?;
    let f = crate::ttf::Font {
        upm: u16::try_from(upm).ok()?,
        hhea: (i16_of(a)?, i16_of(d.checked_neg()?)?, 0),
        os2: Some(4),
        typo: (0, 0, 0),
        win: (u16::try_from(a).ok()?, u16::try_from(d).ok()?),
        fs_selection: 0,
        x_height: 0,
        cap_height: 0,
        strike: (0, 0),
        underline: (0, 0),
        script: [0; 8],
        fixed_pitch: false,
        symbol: false,
        vertical: false,
        family: "Kz Mirror".into(),
        glyphs,
        cmap,
    };
    crate::ttf::parse(&f.bytes())
}

fn text_font(ppem: i64, bold: i64, italic: i64) -> Option<gdifont::TextFont> {
    let weight = if bold != 0 { gdifont::Weight::Bold } else { gdifont::Weight::Normal };
    let slant = if italic != 0 { gdifont::Slant::Italic } else { gdifont::Slant::Upright };
    Some(gdifont::TextFont { file: 0, height: ppem.checked_neg()?, weight, slant })
}

fn rects_of(v: &[i64]) -> Vec<Rect> {
    v.chunks_exact(4).filter_map(|c| if let [a, b, c, d] = c { Some((*a, *b, *c, *d)) } else { None }).collect()
}

/// gdi.glyph's model: [upm, ppem, ascent, descent, bold, italic, format, advance, n, rects]
fn glyph_model(a: &[i64]) -> Option<Vec<i64>> {
    let [upm, ppem, asc, desc, bold, italic, fmt, adv, n, rest @ ..] = a else { return None };
    let r = rects_of(rest.get(..usize::try_from(*n).ok()?.checked_mul(4)?)?);
    let p = rect_font(*upm, *asc, *desc, &[(*adv, r)])?;
    let b = gdifont::glyph(&p, text_font(*ppem, *bold, *italic)?, 0x41, gdifont::Format::of(*fmt)?)?;
    let mut v = vec![i64::try_from(b.size).ok()?, b.black.0, b.black.1, b.origin.0, b.origin.1, b.inc];
    if *fmt != 0 {
        v.extend(b.bytes.iter().map(|x| i64::from(*x)));
    }
    Some(v)
}

/// gdi.text's model: [upm, ppem, ascent, descent, bold, italic, x, y, then each character's
/// advance, n and rectangles]
fn text_model(a: &[i64]) -> Option<Vec<i64>> {
    let [upm, ppem, asc, desc, bold, italic, x, y, rest @ ..] = a else { return None };
    let mut rest = rest;
    let mut chars = Vec::new();
    while let [adv, n, more @ ..] = rest {
        let k = usize::try_from(*n).ok()?.checked_mul(4)?;
        chars.push((*adv, rects_of(more.get(..k)?)));
        rest = more.get(k..)?;
    }
    let p = rect_font(*upm, *asc, *desc, &chars)?;
    let s = (0..chars.len()).map(|i| 0x41u32.checked_add(u32::try_from(i).ok()?)).collect::<Option<Vec<u32>>>()?;
    let mut px = gdifont::text_pixels(&p, text_font(*ppem, *bold, *italic)?, &s, *x, *y)?;
    px.sort_unstable();
    px.dedup();
    Some(px.iter().flat_map(|q| [q.0, q.1]).collect())
}

/// a glyph's arguments from the sampler's: units per em, ppem, bold, italic, the format's
/// index, the advance (per mille of the em), the count of rectangles and the seed of their
/// corners (per mille of the em in -200..1000, each pair in either order, at least a unit each
/// way)
fn glyph_args(a: &[i64]) -> Option<Vec<i64>> {
    let [upm, ppem, bold, italic, f, adv, n, seed] = a else { return None };
    let fmt = *[0, 1, 4, 5, 6].get(usize::try_from(*f).ok()?)?;
    let s = |t: i64| t.checked_mul(*upm).map(|v| v / 1000);
    let mut v = vec![*upm, *ppem, s(800)?, s(250)?, *bold, *italic, fmt, s(*adv)?, *n];
    let mut rng = crate::rng::Sample64::new(u32::try_from(*seed).ok()?);
    for _ in 0..*n {
        let q: [i64; 4] = std::array::from_fn(|_| rng.range(-200, 1000));
        let (x0, x1) = (s(q[0].min(q[1]))?, s(q[0].max(q[1]))?.checked_add(1)?);
        let (y0, y1) = (s(q[2].min(q[3]))?, s(q[2].max(q[3]))?.checked_add(1)?);
        v.extend([x0, y0, x1, y1]);
    }
    Some(v)
}

/// (the sampler tries every combination of the ranges' ends: the corners come from one seed)
fn glyph_ranges() -> Vec<(i64, i64)> {
    vec![(64, 2048), (5, 40), (0, 1), (0, 1), (0, 4), (0, 1000), (1, 3), (1, 0x7fff_ffff)]
}

fn arc_model(a: &[i64]) -> Option<Vec<i64>> {
    let v = gdiarc::arc((arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?), (arg(a, 4)?, arg(a, 5)?), (arg(a, 6)?, arg(a, 7)?), arg::<i64>(a, 8)? != 0)?;
    let mut o = vec![i64::from(v.start.0), i64::from(v.start.1)];
    for b in &v.beziers {
        for p in b {
            o.extend([i64::from(p.0), i64::from(p.1)]);
        }
    }
    o.extend([i64::from(v.middle.0), i64::from(v.middle.1)]);
    Some(o)
}

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "gdi.glyph",
        lean: GLYPH,
        domain: Domain::Sampled("one character of up to three rectangles in the em, units per em 64..2048, ppem 5..40, every format, bold and italic, where the model rasterizes it (no unmeasured dropout)"),
        inputs: |each| {
            sampled(&glyph_ranges(), 20_000, 0x91f0_0001, &mut |a| {
                if let Some(v) = glyph_args(a) && glyph_model(&v).is_some() {
                    each(&v);
                }
            })
        },
        model: glyph_model,
    },
    Mirror {
        name: "gdi.text",
        lean: GLYPH,
        domain: Domain::Sampled("TextOut of two or three such characters at a pen and top (GGO_BITMAP's pixels), where the model rasterizes them"),
        inputs: |each| {
            let mut r = glyph_ranges();
            r.extend([(-20, 20), (-20, 20), (2, 3)]);
            sampled(&r, 5_000, 0x7e47_0001, &mut |a| {
                let (Some(g), [.., x, y, k]) = (a.get(..8).and_then(glyph_args), a) else { return };
                let [upm, ppem, asc, desc, bold, italic, _, adv, n, rects @ ..] = g.as_slice() else { return };
                let mut v = vec![*upm, *ppem, *asc, *desc, *bold, *italic, *x, *y];
                for i in 0..*k {
                    // (the character's rectangles, its advance a little more each time)
                    let Some(adv) = i.checked_mul(7).and_then(|d| adv.checked_add(d)) else { return };
                    v.extend([adv, *n]);
                    v.extend(rects);
                }
                if text_model(&v).is_some() {
                    each(&v);
                }
            })
        },
        model: text_model,
    },
    Mirror {
        name: "gdi.arc",
        lean: ARC,
        domain: Domain::Sampled("boxes of corners in -300..300 at least two pixels each way, radials in -400..400, both directions"),
        inputs: |each| {
            let (c, q) = ((-300, 300), (-400, 400));
            sampled(&[c, c, c, c, q, q, q, q, (0, 1)], 100_000, 0xa4c0_0001, &mut |a| {
                let [x0, y0, x1, y1, a4, a5, a6, a7, a8] = *a else { return };
                if x1.abs_diff(x0) >= 2 && y1.abs_diff(y0) >= 2 {
                    each(&[x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1), a4, a5, a6, a7, a8]);
                }
            })
        },
        model: arc_model,
    },
];
