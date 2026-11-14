//! DirectWrite: an executable specification of font faces (from the font file a case gives,
//! src/ttf.rs), shaping, glyph run rasterization, text analysis and text layout.
//!
//! Sources (Microsoft Learn): "IDWriteFontFile::Analyze", "DWRITE_FONT_FILE_TYPE",
//! "DWRITE_FONT_FACE_TYPE", "IDWriteFontFace" and "IDWriteFontFace1" (GetGlyphCount,
//! GetMetrics, GetGdiCompatibleMetrics, GetGlyphIndices, GetDesignGlyphMetrics,
//! GetGdiCompatibleGlyphMetrics), "DWRITE_FONT_METRICS", "DWRITE_FONT_METRICS1",
//! "DWRITE_GLYPH_METRICS", "IDWriteTextAnalyzer" (AnalyzeScript, AnalyzeBidi,
//! AnalyzeLineBreakpoints, GetGlyphs, GetGlyphPlacements), "IDWriteGlyphRunAnalysis",
//! "IDWriteTextLayout"; the OpenType specification for the tables. What the documentation
//! leaves to the font tables' details is MEASURED from Windows' answers (golden dwrite-32/-64).
pub mod bidi;
pub mod linebreak;
pub mod raster;
pub mod script;

use crate::spec::open;
use crate::spec::clause;
use super::{Fail, Res};
use crate::spec::hit;
use crate::ttf::{glyph_of, parse, Parsed};

/// whether the GDI-compatible glyph metrics are modelled: not yet (DW.GDI.GLYPH, open), the
/// model below matches most of them but not every rounding
const GDI_GLYPH: bool = false;

#[derive(Default)]
pub struct Dwrite {
    font: Option<Parsed>,
}

fn hex(t: &str) -> Option<Vec<u8>> {
    if t.len() % 2 != 0 {
        return None;
    }
    (0..t.len() / 2).map(|i| u8::from_str_radix(t.get(2 * i..2 * i + 2)?, 16).ok()).collect()
}

/// DWRITE_FONT_METRICS1, in the struct's own types
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metrics {
    pub upm: u16,
    pub asc: u16,
    pub desc: u16,
    pub gap: i16,
    pub cap: u16,
    pub x: u16,
    pub ulpos: i16,
    pub ulthick: u16,
    pub stpos: i16,
    pub stthick: u16,
    pub bbox: [i16; 4],
    pub sub: [i16; 4],
    pub sup: [i16; 4],
    pub typo: bool,
}

/// the face's design metrics
pub fn metrics(p: &Parsed) -> Metrics {
    hit(clause::DW_METRICS_HEAD);
    let (x0, y0, x1, y1) = p.head_box;
    let mut m = Metrics { upm: p.upm, bbox: [x0, y1, x1, y0], ..Default::default() };
    match &p.os2 {
        Some(o) => {
            hit(clause::DW_METRICS_WIN);
            m.asc = o.win.0;
            m.desc = (o.win.1 as i16 as i32).unsigned_abs() as u16;
            if let Some((a, d, g)) = p.hhea {
                let gap = a as i32 + (d as i32).abs() + (g as i32).max(0) - m.asc as i32 - m.desc as i32;
                m.gap = gap.max(0) as i16;
            }
            m.stpos = o.strike.1;
            m.stthick = o.strike.0 as u16;
            let s = o.script;
            m.sub = [s[2], -s[3], s[0], s[1]];
            m.sup = [s[6], s[7], s[4], s[5]];
            if o.version >= 2 {
                hit(clause::DW_METRICS_OS2V2);
                m.cap = o.cap_height as u16;
                m.x = o.x_height as u16;
            }
            if o.fs_selection & 0x80 != 0 {
                hit(clause::DW_METRICS_TYPO);
                m.asc = o.typo.0 as u16;
                m.desc = if o.typo.1 < 0 { (-(o.typo.1 as i32)) as u16 } else { 0 };
                m.gap = o.typo.2;
                m.typo = true;
            }
        }
        None => {
            hit(clause::DW_METRICS_NOOS2);
            m.stpos = (p.upm / 3) as i16;
            if let Some((a, d, g)) = p.hhea {
                m.asc = a as u16;
                m.desc = (d as i32).unsigned_abs() as u16;
                m.gap = g.max(0);
            }
        }
    }
    if let Some((pos, thick, _)) = p.post {
        m.ulpos = pos;
        m.ulthick = thick as u16;
    }
    if m.ulthick == 0 {
        hit(clause::DW_METRICS_DEFAULTS);
        m.ulthick = ((p.upm as u32 + 7) / 14) as u16;
    }
    if m.stthick == 0 {
        hit(clause::DW_METRICS_DEFAULTS);
        m.stthick = m.ulthick;
    }
    if m.x == 0 {
        hit(clause::DW_METRICS_DEFAULTS);
        m.x = p.upm / 2;
    }
    if m.cap == 0 {
        hit(clause::DW_METRICS_DEFAULTS);
        m.cap = (p.upm as u32 * 7 / 10) as u16;
    }
    m
}

fn metrics_line(name: &str, m: &Metrics) -> String {
    format!("{name} upm {} asc {} desc {} gap {} cap {} x {} ulpos {} ulthick {} stpos {} stthick {}", m.upm, m.asc, m.desc, m.gap, m.cap, m.x, m.ulpos, m.ulthick, m.stpos, m.stthick)
}

fn round(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// MEASURED (golden dwrite, every field of 360 GetGdiCompatibleMetrics calls): a design value
/// to whole pixels and back, through design units per pixel in single precision:
/// s = upm / (em * ppd), round(round(v / s) * s), rounding halves up
pub fn gdi_round(upm: u16, ppem: i64) -> impl Fn(i32) -> i32 {
    let s = upm as f32 / ppem as f32;
    move |v| round(round(v as f32 / s) as f32 * s)
}

/// DWRITE_GLYPH_METRICS
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlyphMetrics {
    pub adv: u32,
    pub lsb: i32,
    pub rsb: i32,
    pub tsb: i32,
    pub advh: u32,
    pub bsb: i32,
    pub voy: i32,
}

/// a glyph's design metrics (None: past the font's glyphs)
pub fn glyph_metrics(p: &Parsed, m: &Metrics, g: u16) -> Option<GlyphMetrics> {
    let (adv, hlsb) = *p.hmtx.get(g as usize)?;
    let bbox = p.glyphs.get(g as usize)?.as_ref().map(|x| x.0);
    let (x0, y0, x1, y1) = bbox.map_or((hlsb as i32, 0, hlsb as i32, 0), |b| (b.0 as i32, b.1 as i32, b.2 as i32, b.3 as i32));
    hit(clause::DW_GLYPH_H);
    let (advh, voy) = match &p.vmtx {
        Some(v) => {
            hit(clause::DW_GLYPH_VMTX);
            let (a, t) = v[g as usize];
            (a as u32, t as i32 + y1)
        }
        // MEASURED: without vmtx, the OS/2 typographic ascender and the descender's magnitude
        // (whether or not USE_TYPO_METRICS is set); without OS/2 either, the ascent and the hhea
        // descender only where it is positive
        None => match &p.os2 {
            Some(o) => {
                hit(clause::DW_GLYPH_NOVMTX);
                ((o.typo.0 as i32 + (o.typo.1 as i32).abs()) as u32, o.typo.0 as i32)
            }
            None => {
                hit(clause::DW_GLYPH_NOVMTX_NOOS2);
                let d = p.hhea.map_or(0, |h| (h.1 as i32).max(0));
                ((m.asc as i32 + d) as u32, m.asc as i32)
            }
        },
    };
    Some(GlyphMetrics { adv: adv as u32, lsb: x0, rsb: adv as i32 - x1, tsb: voy - y1, advh, bsb: advh as i32 - voy + y0, voy })
}

fn glyph_line(g: &GlyphMetrics) -> String {
    format!("glyph adv {} lsb {} rsb {} tsb {} advh {} bsb {} voy {}", g.adv, g.lsb, g.rsb, g.tsb, g.advh, g.bsb, g.voy)
}

/// a text layout of the face's font alone (the text only of characters its cmap maps)
fn layout(p: &Parsed, em: i64, mw4: i64, wrap: bool, cps: &[u32]) -> Res<Vec<String>> {
    let mut out = Vec::new();
    let name = p.family.encode_utf16().map(|u| format!(" {u}")).collect::<String>();
    if p.family.is_empty() {
        return Err(Fail::Unmodeled(open::DW_LAYOUT_NAME));
    }
    hit(clause::DW_LAYOUT_FAMILY);
    out.push(format!("dwlayout families 1 hr 00000000 name{name}"));
    if cps.iter().any(|&c| glyph_of(p, c) == 0) {
        return Err(Fail::Unmodeled(open::DW_LAYOUT_FALLBACK));
    }
    let br = linebreak::breaks(cps)?;
    let scale = em as f32 / p.upm as f32;
    let adv: Vec<f32> = cps.iter().map(|&c| p.hmtx[glyph_of(p, c) as usize].0 as f32 * scale).collect();
    let m = metrics(p);
    hit(clause::DW_LAYOUT_LINE);
    // MEASURED: the line gap above the ascent
    let base = (m.asc as i32 + m.gap as i32) as f32 * em as f32 / p.upm as f32;
    let lh = base + m.desc as f32 * em as f32 / p.upm as f32;
    let maxw = mw4 as f32 / 4.0;
    let ws = |i: usize| br[i].2 != 0;
    // lines: [start, end) by the break opportunities (MEASURED: greedy, the width without trailing
    // whitespace within the layout's)
    let mut lines = Vec::new();
    let mut s = 0;
    while s < cps.len() {
        let mut e = cps.len();
        if wrap {
            // the width of [s, e) without its trailing whitespace
            let width = |e: usize| {
                let mut k = e;
                while k > s && ws(k - 1) {
                    k -= 1;
                }
                adv[s..k].iter().fold(0.0f32, |a, b| a + b)
            };
            // the last break opportunity that fits; else (MEASURED: WRAP breaks a word that
            // does not fit) the most clusters that fit, at least one
            let mut fit = None;
            for e2 in s + 1..=cps.len() {
                if (e2 == cps.len() || br[e2 - 1].1 == linebreak::CAN) && width(e2) <= maxw {
                    fit = Some(e2);
                }
            }
            e = match fit {
                Some(f) => f,
                None => {
                    hit(clause::DW_LAYOUT_EMERGENCY);
                    let mut f = s + 1;
                    while f < cps.len() && width(f + 1) <= maxw {
                        f += 1;
                    }
                    // (whitespace after it stays on the line)
                    while f < cps.len() && ws(f) {
                        f += 1;
                    }
                    f
                }
            };
            hit(clause::DW_LAYOUT_WRAP);
        }
        lines.push((s, e));
        s = e;
    }
    let mut widths = Vec::new();
    for &(s, e) in &lines {
        let mut k = e;
        while k > s && ws(k - 1) {
            k -= 1;
        }
        let w: f32 = adv[s..k].iter().fold(0.0, |a, b| a + b);
        let wt: f32 = adv[s..e].iter().fold(0.0, |a, b| a + b);
        widths.push((w, wt, e - k));
    }
    let width = widths.iter().map(|w| w.0).fold(0.0f32, f32::max);
    let wtrail = widths.iter().map(|w| w.1).fold(0.0f32, f32::max);
    let b = |v: f32| format!("{:08x}", v.to_bits());
    // (MEASURED: the last line's height is its baseline plus the descent)
    let height: f32 = (0..lines.len()).map(|_| lh).fold(0.0, |a, b| a + b);
    // (MEASURED: left is negative zero)
    out.push(format!("metrics hr 00000000 left {} top 00000000 width {} wtrail {} height {} lines {} depth 1", b(-0.0), b(width), b(wtrail), b(height), lines.len()));
    for (&(s, e), w) in lines.iter().zip(&widths) {
        out.push(format!("line {} trail {} nl 0 height {} base {} trimmed 0", e - s, w.2, b(lh), b(base)));
    }
    hit(clause::DW_LAYOUT_CLUSTERS);
    let cl: Vec<String> = (0..cps.len()).map(|i| format!(" {}/1/{}{}0{}0", b(adv[i]), (br[i].1 == linebreak::CAN) as u8, br[i].2, br[i].3)).collect();
    out.push(format!("clusters hr 00000000 n {}{}", cps.len(), cl.concat()));
    Ok(out)
}

impl Dwrite {
    fn face(&self) -> Res<&Parsed> {
        self.font.as_ref().ok_or(Fail::Unmodeled(open::DW_NOFACE))
    }

    pub fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let n = |i: usize| t.get(i).and_then(|v| crate::case::num(v)).ok_or(Fail::Bad);
        match t[0].as_str() {
            "dwfont" => {
                if t.len() != 2 {
                    return Err(Fail::Bad);
                }
                self.font = None;
                let b = hex(&t[1]).ok_or(Fail::Bad)?;
                let p = parse(&b).ok_or(Fail::Unmodeled(open::DW_FILE_OTHER))?;
                hit(clause::DW_FILE_ANALYZE);
                out.push("dwfont analyze hr 00000000 supported 1 file 2 face 1 faces 1".into());
                out.push("dwfont face hr 00000000".into());
                self.font = Some(p);
            }
            "dwinfo" => {
                let p = self.face()?;
                hit(clause::DW_INFO);
                out.push(format!("dwinfo glyphs {} type 1 index 0 sim 0 symbol {} mono {} vert 0", p.num_glyphs, p.symbol as u8, p.post.is_some_and(|x| x.2 != 0) as u8));
            }
            "dwmetrics" => {
                let m = metrics(self.face()?);
                out.push(metrics_line("dwmetrics", &m));
                let b = m.bbox;
                out.push(format!(
                    "dwmetrics box {} {} {} {} sub {} {} {} {} sup {} {} {} {} typo {}",
                    b[0], b[1], b[2], b[3], m.sub[0], m.sub[1], m.sub[2], m.sub[3], m.sup[0], m.sup[1], m.sup[2], m.sup[3], m.typo as u8
                ));
            }
            "dwgdimetrics" => {
                let (em, ppd) = (n(1)?, n(2)?);
                if t.len() != 3 {
                    return Err(Fail::Bad);
                }
                let mut m = metrics(self.face()?);
                if em <= 0 || ppd <= 0 {
                    return Err(Fail::Unmodeled(open::DW_GDI_INVALID));
                }
                hit(clause::DW_GDI_METRICS);
                let r = gdi_round(m.upm, em * ppd);
                m.asc = r(m.asc as i32) as u16;
                m.desc = r(m.desc as i32) as u16;
                m.gap = r(m.gap as i32) as i16;
                m.cap = r(m.cap as i32) as u16;
                m.x = r(m.x as i32) as u16;
                m.ulpos = r(m.ulpos as i32) as i16;
                m.ulthick = r(m.ulthick as i32) as u16;
                m.stpos = r(m.stpos as i32) as i16;
                m.stthick = r(m.stthick as i32) as u16;
                out.push(metrics_line("dwgdimetrics", &m));
            }
            "dwcmap" => {
                let p = self.face()?;
                if t.len() < 2 {
                    return Err(Fail::Bad);
                }
                hit(clause::DW_CMAP);
                let mut s = String::from("dwcmap");
                for i in 1..t.len() {
                    s += &format!(" {}", glyph_of(p, n(i)? as u32));
                }
                out.push(s);
            }
            "dwglyph" | "dwgdiglyph" => {
                let gdi = t[0] == "dwgdiglyph";
                let at = if gdi { 5 } else { 2 };
                let mut gs = Vec::new();
                for i in at..t.len() {
                    gs.push(n(i)? as u16);
                }
                let side = n(at - 1)?;
                if gs.is_empty() {
                    return Err(Fail::Bad);
                }
                let p = self.face()?;
                let m = metrics(p);
                if side != 0 {
                    hit(clause::DW_GLYPH_SIDEWAYS);
                }
                let mut v = Vec::new();
                let mut gids = Vec::new();
                for g in gs {
                    // (MEASURED: a glyph past the font's has glyph 0's metrics)
                    let g = if g >= p.num_glyphs {
                        hit(clause::DW_GLYPH_OUTSIDE);
                        0
                    } else {
                        g
                    };
                    v.push(glyph_metrics(p, &m, g).ok_or(Fail::Unmodeled(open::DW_FILE_OTHER))?);
                    gids.push(g);
                }
                if gdi {
                    let (em, ppd) = (n(1)?, n(2)?);
                    if em <= 0 || ppd <= 0 {
                        return Err(Fail::Unmodeled(open::DW_GDI_INVALID));
                    }
                    if !GDI_GLYPH {
                        return Err(Fail::Unmodeled(open::DW_GDI_GLYPH));
                    }
                    let ppem = em * ppd;
                    // MEASURED: each metric is a whole number of pixels (advances as the GDI
                    // placements round them: DW.SHAPE.GDIPLACE, or DW.SHAPE.NATPLACE for GDI
                    // natural), back in design units rounded; the vertical advance without vmtx
                    // is the ascent less the signed descent (typographic, or hhea without OS/2);
                    // the side bearings are the black box of the glyph drawn on 6 samples a pixel
                    // across and 1 down (sideways: 1 across and 6 down), from its unsnapped origin
                    let nat = n(3)? != 0;
                    let upm = m.upm as i64;
                    let fix = (ppem << 16) / upm;
                    let pxv = |v: i32| (v as i64 * fix + 0x8000) >> 16;
                    let px = |v: i32| if nat { (128 * v as i64 * ppem + 69 * upm).div_euclid(128 * upm) } else { (v as i64 * fix + 0x8000) >> 16 };
                    // (a length of k / n pixels in design units, rounded)
                    let du = |k: i64, n: i64| (2 * k * upm + n * ppem).div_euclid(2 * n * ppem) as i32;
                    let (vadv, vorg) = match (&p.vmtx, &p.os2) {
                        (Some(_), _) => (None, None),
                        (None, Some(o)) => (Some(o.typo.0 as i32 - o.typo.1 as i32), Some(o.typo.0 as i32)),
                        (None, None) => p.hhea.map_or((None, None), |h| (Some(h.0 as i32 - h.1 as i32), Some(h.0 as i32))),
                    };
                    for (g, gid) in v.iter_mut().zip(&gids) {
                        // (sideways: the glyph's y is drawn across, so the 6 samples are its y)
                        let (nx, ny) = if side != 0 { (1, 6) } else { (6, 1) };
                        let bb = raster::black_box(p, ppem, *gid, nx, ny)?.unwrap_or((0, 0, 0, 0));
                        let (advp, voyp, advhp) = (px(g.adv as i32), pxv(vorg.unwrap_or(g.voy)), pxv(vadv.unwrap_or(g.advh as i32)));
                        *g = GlyphMetrics {
                            adv: du(advp, 1) as u32,
                            lsb: du(bb.0, nx),
                            rsb: du(nx * advp - bb.2, nx),
                            tsb: du(ny * voyp + bb.1, ny),
                            advh: du(advhp, 1) as u32,
                            bsb: du(ny * (advhp - voyp) - bb.3, ny),
                            voy: du(voyp, 1),
                        };
                    }
                }
                out.push(format!("{} hr 00000000", t[0]));
                out.extend(v.iter().map(glyph_line));
            }
            "dwshape" => {
                let em = n(1)?;
                let mut cps = Vec::new();
                for i in 2..t.len() {
                    cps.push(n(i)? as u32);
                }
                if cps.is_empty() || cps.iter().any(|&c| c >= 0x10000) {
                    return Err(Fail::Bad);
                }
                let p = self.face()?;
                // GetGlyphs: the cmap's glyph of each character, one cluster each (the fonts have
                // no GSUB)
                hit(clause::DW_SHAPE_GLYPHS);
                let gids: Vec<u16> = cps.iter().map(|&c| glyph_of(p, c)).collect();
                let space = |c: u32| c == 0x20;
                let list = |v: Vec<String>| v.join(" ");
                out.push(format!("dwshape glyphs hr 00000000 n {}", gids.len()));
                out.push(format!("gids {}", list(gids.iter().map(|g| g.to_string()).collect())));
                out.push(format!("clusters {}", list((0..cps.len()).map(|i| i.to_string()).collect())));
                // MEASURED: canBreakShapingAfter everywhere, isShapedAlone on a space; each glyph
                // a cluster start (and a bit Windows sets in the reserved ones), justification
                // BLANK for a space and a no-break space (natplace cases), CHARACTER otherwise
                hit(clause::DW_SHAPE_PROPS);
                out.push(format!("textprops {}", list(cps.iter().map(|&c| if space(c) { "5" } else { "4" }.to_string()).collect())));
                out.push(format!("glyphprops {}", list(cps.iter().map(|&c| if matches!(c, 0x20 | 0xa0) { "114" } else { "112" }.to_string()).collect())));
                let upm = p.upm;
                let adv = |g: u16| p.hmtx.get(g as usize).map_or(0, |h| h.0);
                hit(clause::DW_SHAPE_PLACE);
                let f = |v: f32| format!("{:08x}/00000000/00000000", v.to_bits());
                out.push(format!("place hr 00000000 {}", list(gids.iter().map(|&g| f(adv(g) as f32 * (em as f32 / upm as f32))).collect())));
                // MEASURED: GDI classic advances scaled by em / upm as a truncated 16.16 number,
                // rounded to whole pixels (612 advances); GDI natural ones round up from a
                // little below the half (between 0.4545 and 0.462 of a pixel, 612 advances)
                hit(clause::DW_SHAPE_GDIPLACE);
                let fix = ((em << 16) / upm as i64) as i64;
                out.push(format!("gdiplace hr 00000000 {}", list(gids.iter().map(|&g| f(((adv(g) as i64 * fix + 0x8000) >> 16) as f32)).collect())));
                hit(clause::DW_SHAPE_NATPLACE);
                out.push(format!("natplace hr 00000000 {}", list(gids.iter().map(|&g| f((adv(g) as i64 * em * 128 + 69 * upm as i64).div_euclid(128 * upm as i64) as f32)).collect())));
            }
            "dwrun" => {
                let (mode, fit) = raster::Mode::parse(t.get(1).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?;
                let (em, x4, y4) = (n(2)?, n(3)?, n(4)?);
                if t.len() < 7 || (t.len() - 5) % 2 != 0 {
                    return Err(Fail::Bad);
                }
                let mut run = Vec::new();
                for i in (5..t.len()).step_by(2) {
                    run.push((n(i)? as u16, n(i + 1)?));
                }
                let p = self.face()?;
                out.push("dwrun hr 00000000".into());
                out.extend(raster::run(p, mode, fit, em, x4, y4, &run)?);
            }
            "dwlayout" => {
                let (em, mw4, wrap) = (n(1)?, n(2)?, n(3)?);
                let mut cps = Vec::new();
                for i in 4..t.len() {
                    let c = n(i)?;
                    if !(0..0x10000).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cps.push(c as u32);
                }
                if cps.is_empty() {
                    return Err(Fail::Bad);
                }
                let p = self.face()?;
                out.extend(layout(p, em, mw4, wrap != 0, &cps)?);
            }
            "dwscript" => {
                let mut cps = Vec::new();
                for i in 1..t.len() {
                    let c = n(i)?;
                    if !(0..=0x10ffff).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cps.push(c as u32);
                }
                if cps.is_empty() {
                    return Err(Fail::Bad);
                }
                for (pos, len, sc) in script::runs(&cps)? {
                    out.push(format!("run {pos} {len} {} {} shapes {}", sc.0, sc.1, sc.2));
                }
                out.push("dwscript hr 00000000".into());
            }
            "dwbidi" => {
                let para = match t.get(1).map(String::as_str) {
                    Some("ltr") => 0,
                    Some("rtl") => 1,
                    _ => return Err(Fail::Bad),
                };
                let mut cs = Vec::new();
                for i in 2..t.len() {
                    let c = n(i)?;
                    if !(0..=0x10ffff).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cs.push(bidi::class(c as u32).ok_or(Fail::Unmodeled(open::DW_BIDI_CLASS))?);
                }
                if cs.is_empty() {
                    return Err(Fail::Bad);
                }
                let lv = bidi::levels(&cs, para);
                out.push("dwbidi hr 00000000".into());
                out.push(format!("explicit{}", cs.iter().map(|_| format!(" {para}")).collect::<String>()));
                out.push(format!("resolved{}", lv.iter().map(|l| format!(" {l}")).collect::<String>()));
            }
            "dwbreak" => {
                let mut cps = Vec::new();
                for i in 1..t.len() {
                    let c = n(i)?;
                    if !(0..=0x10ffff).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cps.push(c as u32);
                }
                if cps.is_empty() {
                    return Err(Fail::Bad);
                }
                let b = linebreak::breaks(&cps)?;
                out.push("dwbreak hr 00000000".into());
                out.push(format!("breaks{}", b.iter().map(|x| format!(" {}{}{}{}", x.0, x.1, x.2, x.3)).collect::<String>()));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
