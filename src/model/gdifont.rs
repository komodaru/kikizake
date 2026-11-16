//! What GDI measures of a TrueType font from the font's own tables (spec/gditext.txt,
//! GDI.TEXT.*): TEXTMETRIC, GetCharWidth32, GetCharABCWidths, GetTextExtentPoint32 and
//! GetTextExtentExPoint. The rules are the ones Windows' answers showed for the fonts the suite
//! writes (golden text mfont-*); Microsoft Learn ("TEXTMETRIC", "GetCharABCWidths",
//! "OS/2 table") says what the fields are, not how they are rounded.
//!
//! Every scaling is exact: a value in font units times a rational scale, rounded half up.

use crate::ttf::Parsed;

/// a font selected into a DC: a font file the case added (its index) and what CreateFont asked
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextFont {
    pub file: usize,
    /// lfHeight (0: the default height)
    pub height: i64,
    pub weight: Weight,
    pub slant: Slant,
}

/// lfItalic
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slant {
    Upright,
    /// synthesized: the suite's fonts have no italic face
    Italic,
}

impl Slant {
    pub fn of(v: i64) -> Option<Slant> {
        match v {
            0 => Some(Slant::Upright),
            1 => Some(Slant::Italic),
            _ => None,
        }
    }
}

/// the weights the model knows (CreateFont's FW_NORMAL and FW_BOLD)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Normal,
    Bold,
}

impl Weight {
    pub fn of(w: i64) -> Option<Weight> {
        match w {
            400 => Some(Weight::Normal),
            700 => Some(Weight::Bold),
            _ => None,
        }
    }
    pub fn value(self) -> i64 {
        match self {
            Weight::Normal => 400,
            Weight::Bold => 700,
        }
    }
    /// the pixel a synthesized bold adds to an advance and a black box
    fn extra(self) -> i64 {
        match self {
            Weight::Normal => 0,
            Weight::Bold => 1,
        }
    }
}

/// font units to pixels: v * num / den (den > 0)
#[derive(Clone, Copy, Debug, PartialEq)]
struct Scale {
    num: i64,
    den: i64,
}

/// n / d rounded half up (d > 0)
fn rnd(n: i64, d: i64) -> i64 {
    (2 * n + d).div_euclid(2 * d)
}

/// n / d rounded up (d > 0)
fn ceil(n: i64, d: i64) -> i64 {
    -(-n).div_euclid(d)
}

impl Scale {
    fn px(self, v: i64) -> i64 {
        rnd(v * self.num, self.den)
    }
}

/// TEXTMETRIC's fields that depend on the font
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tm {
    pub height: i64,
    pub ascent: i64,
    pub descent: i64,
    pub internal: i64,
    pub external: i64,
    pub ave: i64,
    pub max: i64,
    pub weight: i64,
    pub pitch: i64,
    /// tmItalic
    pub italic: i64,
}

/// what the measuring takes from a font and the request
#[derive(Clone, Copy, Debug)]
struct Sized {
    scale: Scale,
    /// ppem rounded to a whole pixel (the outlines' grid)
    ppem: i64,
    upm: i64,
    /// the ascent and the (signed) descent in font units
    a: i64,
    d: i64,
    bold: Weight,
}

fn sized(p: &Parsed, f: TextFont) -> Option<Sized> {
    let hh = p.hhea?;
    let upm = p.upm as i64;
    // (MEASURED: with OS/2 the ascent and the descent are usWinAscent and usWinDescent, the
    // descent taken as signed; the cell is their sum. Without it they are head's yMax and
    // -yMin and the cell is hhea's ascender - descender)
    let (a, d, cell) = match &p.os2 {
        Some(o) => {
            let (a, d) = (o.win.0 as i64, o.win.1 as i16 as i64);
            (a, d, a + d)
        }
        None => (p.head_box.3 as i64, -(p.head_box.1 as i64), hh.0 as i64 - hh.1 as i64),
    };
    if upm <= 0 || cell <= 0 {
        return None;
    }
    // (MEASURED: lfHeight 0 is -16; below 0 it is the em in pixels, above 0 the cell)
    let h = if f.height == 0 { -16 } else { f.height };
    let scale = if h < 0 { Scale { num: -h, den: upm } } else { Scale { num: h, den: cell } };
    Some(Sized { scale, ppem: rnd(scale.num * upm, scale.den), upm, a, d, bold: f.weight })
}

/// GetTextMetrics (GDI.TEXT.TM, GDI.TEXT.AVE)
pub fn metrics(p: &Parsed, f: TextFont) -> Option<Tm> {
    let z = sized(p, f)?;
    let hh = p.hhea?;
    let s = z.scale;
    let ascent = s.px(z.a);
    let descent = s.px(z.d).max(0);
    let height = ascent + descent;
    let e = match &p.os2 {
        Some(_) => hh.2 as i64 - ((z.a + z.d) - (hh.0 as i64 - hh.1 as i64)),
        None => hh.2 as i64,
    };
    let bw = p.head_box.2 as i64 - p.head_box.0 as i64;
    let bold = z.bold.extra();
    // (MEASURED: the average is OS/2 xAvgCharWidth; 0 there: half the font's box; no OS/2:
    // two thirds of it)
    let ave = match &p.os2 {
        Some(o) if o.avg != 0 => s.px(o.avg as i64),
        Some(_) => rnd(bw * s.num, 2 * s.den),
        None => rnd(2 * bw * s.num, 3 * s.den),
    } + bold;
    let fixed = p.post.is_some_and(|x| x.2 != 0);
    // (MEASURED: TMPF_TRUETYPE | TMPF_VECTOR, | TMPF_FIXED_PITCH (its sense reversed) when
    // the font is proportional; a fixed font without OS/2 is FF_MODERN, 0x30, too)
    let pitch = match (fixed, p.os2.is_some()) {
        (false, _) => 7,
        (true, true) => 6,
        (true, false) => 0x36,
    };
    Some(Tm { height, ascent, descent, internal: height - z.ppem, external: s.px(e.max(0)), ave, max: s.px(bw) + bold, weight: f.weight.value(), pitch, italic: match f.slant {
        Slant::Upright => 0,
        Slant::Italic => 255,
    } })
}

/// GetCharWidth32 of a code point (GDI.TEXT.WIDTH): the advance scaled, the bold pixel added
/// where the advance is not 0
pub fn width(p: &Parsed, f: TextFont, c: u32) -> Option<i64> {
    let z = sized(p, f)?;
    let adv = p.hmtx.get(crate::ttf::glyph_of(p, c) as usize)?.0 as i64;
    Some(z.scale.px(adv) + if adv > 0 { z.bold.extra() } else { 0 })
}

/// GetCharABCWidths' answer for a character
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Abc {
    pub a: i64,
    pub b: i64,
    pub c: i64,
    pub black: Black,
}

/// where the black box came from
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Black {
    /// the outline's box
    Box,
    /// the outline lies outside the ascent and the descent: an empty box of 1 pixel
    Outside,
}

/// GetCharABCWidths of a code point (GDI.TEXT.ABC, GDI.TEXT.ABCEMPTY); None for a glyph with no
/// outline (not measured)
pub fn abc(p: &Parsed, f: TextFont, c: u32) -> Option<Abc> {
    let z = sized(p, f)?;
    let w = width(p, f, c)?;
    let (b, _) = p.glyphs.get(crate::ttf::glyph_of(p, c) as usize)?.as_ref()?;
    let (x0, y0, x1, y1) = (b.0 as i64, b.1 as i64, b.2 as i64, b.3 as i64);
    // (MEASURED: a glyph wholly above the ascent or below the descent has no black box)
    if y1 < -z.d || y0 > z.a {
        return Some(Abc { a: 0, b: 1, c: w - 1, black: Black::Outside });
    }
    // (MEASURED: the box on the rounded ppem, snapped to 1/64 pixel; its left edge rounded
    // half down, its right edge half up)
    let q = |v: i64| rnd(v * z.ppem * 64, z.upm);
    let a = ceil(q(x0) - 32, 64);
    let right = (q(x1) + 32).div_euclid(64);
    let b = (right - a).max(1) + z.bold.extra();
    Some(Abc { a, b, c: w - a - b, black: Black::Box })
}

/// a width with the character extra (GDI.TEXT.EXTRAEXTENT): a negative extra leaves a
/// character at least 1 pixel wide, one of width 0 at 0
fn with_extra(w: i64, extra: i64) -> i64 {
    (w + extra).max(w.min(1))
}

/// GetTextExtentPoint32 (GDI.TEXT.EXTENT, GDI.TEXT.EXTRAEXTENT): the widths' sum, each
/// with the character extra, the font's height
pub fn extent(p: &Parsed, f: TextFont, s: &[u32], extra: i64) -> Option<(i64, i64)> {
    let mut x = 0;
    for &c in s {
        x += with_extra(width(p, f, c)?, extra);
    }
    Some((x, metrics(p, f)?.height))
}

/// GetTextExtentExPoint (GDI.TEXT.EXTENT): the characters that fit in max, each one's
/// running width, and the whole extent
pub fn extent_ex(p: &Parsed, f: TextFont, s: &[u32], max: i64, extra: i64) -> Option<(usize, Vec<i64>, (i64, i64))> {
    let mut dx = Vec::new();
    let mut x = 0;
    for &c in s {
        x += with_extra(width(p, f, c)?, extra);
        dx.push(x);
    }
    let fit = dx.iter().take_while(|&&v| v <= max).count();
    dx.truncate(fit);
    Some((fit, dx, extent(p, f, s, extra)?))
}

/// a glyph's outline as the suite writes it: rectangles in font units, each one contour of
/// its corners (x0, y0), (x0, y1), (x1, y1), (x1, y0); None for any other outline
fn rects(p: &Parsed, g: u16) -> Option<Vec<(i64, i64, i64, i64)>> {
    let Some(Some((_, contours))) = p.glyphs.get(g as usize) else {
        return if (g as usize) < p.glyphs.len() { Some(Vec::new()) } else { None };
    };
    contours
        .iter()
        .map(|c| match c.as_slice() {
            &[(x0, y0), (xa, y1), (x1, yb), (xc, yc)] if xa == x0 && yb == y1 && xc == x1 && yc == y0 && x0 < x1 && y0 < y1 => Some((x0 as i64, y0 as i64, x1 as i64, y1 as i64)),
            _ => None,
        })
        .collect()
}

/// a pixel of a glyph's grid: (x, y) is the pixel [x, x + 1) x [y, y + 1), y up from the
/// baseline
pub type Px = (i64, i64);

/// the pixels a glyph's rectangles light on a grid k times finer than the pixel's
/// (GDI.TEXT.BITMAP): the outline on the rounded ppem snapped to 1/64 pixel, then scaled by k;
/// a sample point (a cell's center) on an edge is inside; a scan line through sample points
/// that crosses a rectangle between two of them lights the left (lower) one, or the right
/// (upper) one where the left lies before the glyph's black box (its left edge rounded half
/// down, as GetCharABCWidths')
fn samples(z: &Sized, r: &[(i64, i64, i64, i64)], k: i64) -> std::collections::BTreeSet<Px> {
    let q = |v: i64| rnd(v * z.ppem * 64, z.upm) * k;
    let r: Vec<_> = r.iter().map(|&(a, b, c, d)| (q(a), q(b), q(c), q(d))).collect();
    let mut lit = std::collections::BTreeSet::new();
    let (Some(mx), Some(my)) = (r.iter().map(|x| x.0).min(), r.iter().map(|x| x.1).min()) else {
        return lit;
    };
    let (bx, by) = (ceil(mx - 32, 64), ceil(my - 32, 64));
    for &(x0, y0, x1, y1) in &r {
        let cols = ceil(x0 - 32, 64)..=(x1 - 32).div_euclid(64);
        let rows = ceil(y0 - 32, 64)..=(y1 - 32).div_euclid(64);
        let near = |lo: i64, b: i64| {
            let k = (lo - 32).div_euclid(64);
            if k < b { k + 1 } else { k }
        };
        let cols: Vec<i64> = if cols.is_empty() { vec![near(x0, bx)] } else { cols.collect() };
        let rows: Vec<i64> = if rows.is_empty() { vec![near(y0, by)] } else { rows.collect() };
        for &i in &cols {
            for &j in &rows {
                lit.insert((i, j));
            }
        }
    }
    lit
}

/// GetGlyphOutline's answer for a bitmap format
#[derive(Clone, Debug, PartialEq)]
pub struct Bits {
    /// the buffer's size (0: an empty glyph, no buffer)
    pub size: usize,
    /// gmBlackBoxX, gmBlackBoxY, gmptGlyphOrigin
    pub black: (i64, i64),
    pub origin: (i64, i64),
    pub inc: i64,
    /// the bitmap, its rows top down, each a whole number of DWORDs
    pub bytes: Vec<u8>,
}

/// GetGlyphOutline's bitmap formats the model knows
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// GGO_METRICS: the black box of GGO_BITMAP
    Metrics,
    /// GGO_BITMAP: 1 bit per pixel
    Mono,
    /// GGO_GRAY8_BITMAP: a byte per pixel, 0 to 64
    Gray8,
}

impl Format {
    pub fn of(f: i64) -> Option<Format> {
        match f {
            0 => Some(Format::Metrics),
            1 => Some(Format::Mono),
            6 => Some(Format::Gray8),
            _ => None,
        }
    }
}

/// a glyph's pixels with their values (Mono: 1; Gray8: the sample count, 0 to 64), bold
/// applied: each pixel adds its left neighbour's value, at most the format's top
/// (GDI.TEXT.BITMAP, GDI.TEXT.GRAY)
fn pixels(z: &Sized, r: &[(i64, i64, i64, i64)], f: Format) -> std::collections::BTreeMap<Px, i64> {
    let (k, top) = match f {
        Format::Metrics | Format::Mono => (1, 1),
        Format::Gray8 => (8, 64),
    };
    let mut v = std::collections::BTreeMap::new();
    for (i, j) in samples(z, r, k) {
        *v.entry((i.div_euclid(k), j.div_euclid(k))).or_insert(0) += 1;
    }
    match z.bold {
        Weight::Normal => v,
        Weight::Bold => {
            let mut b = std::collections::BTreeMap::new();
            for (&(i, j), &n) in &v {
                for p in [(i, j), (i + 1, j)] {
                    let e = b.entry(p).or_insert(0);
                    *e = (*e + n).min(top);
                }
            }
            b
        }
    }
}

/// GetGlyphOutline of a code point (GDI.TEXT.BITMAP, GDI.TEXT.GRAY): the bitmap of every pixel
/// lit, the black box its rows within the ascent and the descent; None where the model does not
/// know the glyph
pub fn glyph(p: &Parsed, f: TextFont, c: u32, fmt: Format) -> Option<Bits> {
    let z = sized(p, f)?;
    let r = rects(p, crate::ttf::glyph_of(p, c))?;
    let inc = width(p, f, c)?;
    let v = pixels(&z, &r, fmt);
    let (Some(x0), Some(x1), Some(y0), Some(y1)) = (v.keys().map(|k| k.0).min(), v.keys().map(|k| k.0).max(), v.keys().map(|k| k.1).min(), v.keys().map(|k| k.1).max()) else {
        return Some(Bits { size: if fmt == Format::Metrics { 1 } else { 0 }, black: (1, 1), origin: (0, 0), inc, bytes: Vec::new() });
    };
    let w = x1 - x0 + 1;
    let stride = match fmt {
        Format::Metrics | Format::Mono => (w + 31) / 32 * 4,
        Format::Gray8 => (w + 3) / 4 * 4,
    };
    let mut bytes = vec![0u8; (stride * (y1 - y0 + 1)) as usize];
    for (&(i, j), &n) in &v {
        let at = ((y1 - j) * stride) as usize;
        match fmt {
            Format::Metrics | Format::Mono => bytes[at + ((i - x0) / 8) as usize] |= 0x80 >> ((i - x0) % 8),
            Format::Gray8 => bytes[at + (i - x0) as usize] = n as u8,
        }
    }
    let (lo, hi) = clip_rows(&z);
    let (Some(r0), Some(r1)) = (v.keys().map(|k| k.1).filter(|j| (lo..hi).contains(j)).min(), v.keys().map(|k| k.1).filter(|j| (lo..hi).contains(j)).max()) else {
        return None; // (every row outside the cell: not measured)
    };
    let size = match fmt {
        Format::Metrics => 1,
        Format::Mono | Format::Gray8 => bytes.len(),
    };
    if fmt == Format::Metrics {
        bytes.clear();
    }
    Some(Bits { size, black: (w, r1 - r0 + 1), origin: (x0, r1 + 1), inc, bytes })
}

/// the rows of a glyph's black box and of what TextOut draws: from the descent below the
/// baseline (signed) to the ascent (GDI.TEXT.BITMAP)
fn clip_rows(z: &Sized) -> (i64, i64) {
    (-z.scale.px(z.d), z.scale.px(z.a))
}

/// the pixels TextOut lights for a string at (x, y), TA_TOP | TA_LEFT, in device coordinates
/// (GDI.TEXT.DRAW): each character's GGO_BITMAP pixels within its black box's rows, its
/// baseline tmAscent below y, the pen moving by each character's width
pub fn text_pixels(p: &Parsed, f: TextFont, s: &[u32], x: i64, y: i64) -> Option<Vec<(i64, i64)>> {
    let z = sized(p, f)?;
    let asc = metrics(p, f)?.ascent;
    let (lo, hi) = clip_rows(&z);
    let mut pen = x;
    let mut out = Vec::new();
    for &c in s {
        let r = rects(p, crate::ttf::glyph_of(p, c))?;
        for ((i, j), _) in pixels(&z, &r, Format::Mono) {
            if (lo..hi).contains(&j) {
                out.push((pen + i, y + asc - j - 1));
            }
        }
        pen += width(p, f, c)?;
    }
    Some(out)
}
