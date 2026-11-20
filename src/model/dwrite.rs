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
pub mod fallback;
pub mod linebreak;
pub mod raster;
pub mod script;

use crate::spec::clause;
use super::{Fail, Res};
use crate::spec::hit;
use crate::ttf::{analyze, face_at, glyph_of, parse_face, FileType, Parsed};

#[derive(Default)]
pub struct Dwrite {
    font: Option<Parsed>,
    /// what Analyze made of the face's file
    file: Option<FileType>,
}

pub(crate) fn hex(t: &str) -> Option<Vec<u8>> {
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
                m.desc = (-(o.typo.1 as i32)).max(0) as u16;
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
    match p.post {
        Some((pos, thick, _)) => {
            m.ulpos = pos;
            m.ulthick = thick as u16;
        }
        // MEASURED (golden dwrite, file-no-post): a tenth of the em below
        None => {
            hit(clause::DW_METRICS_DEFAULTS);
            m.ulpos = -((p.upm / 10) as i16);
        }
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

/// GetGdiCompatibleGlyphMetrics' way back from pixels to the em (re/dwrite.md: TrueTypeRasterizer
/// GetMetrics, DisplayToEm, FloatToFixed): float32 through the inverse of the 16.16 transform
/// {p, 0, 0, p} (sideways {0, -p, p, 0}), whose entries are 1 / (p p) times p, not 1 / p
#[derive(Clone, Copy, Debug)]
struct ToEm {
    m: f32,
}

impl ToEm {
    fn new(ppem: i64) -> ToEm {
        let p = ppem as f32;
        ToEm { m: (1.0 / (p * p)) * p }
    }

    /// an em fraction in float to 16.16: times 65536 twice, truncated, then the half up
    fn fixed(f: f32) -> i64 {
        let t = (f * 65536.0 * 65536.0) as i64;
        ((t + 0x8000) >> 16).clamp(i32::MIN as i64, i32::MAX as i64)
    }

    /// k whole pixels along an axis
    fn whole(self, k: i64) -> i64 {
        ToEm::fixed(k as f32 * self.m)
    }

    /// sideways: v 1/65536 pixels across (display x, not rounded to the pixel) to the em's y
    fn across(self, v: i64) -> i64 {
        ToEm::fixed((v as f32 * (1.0 / 65536.0)) * -self.m)
    }

    /// a black box edge of e samples of a sixth of a pixel (x in display axes)
    fn sixths(self, e: i64, sign: f32) -> i64 {
        ToEm::fixed((e as f32 * (1.0f32 / 6.0)) * (sign * self.m))
    }
}

/// the scaler's advance phantom point (re/dwrite.md, scl_RoundCurrentSideBearingPnt): adv design
/// units at ppem rounded half away from zero to 16.16, then to 26.6, then to a 1/16 pixel
/// (axis aligned) or a whole pixel (sideways); the advance's whole pixels (half up)
fn phantom_px(adv: i64, ppem: i64, upm: i64, sideways: bool) -> i64 {
    let n = adv * (ppem << 16);
    let a16 = if n >= 0 { (n + upm / 2) / upm } else { -((upm / 2 - n) / upm) };
    let a26 = (a16 + 0x200) >> 10;
    let q = if sideways { a26 } else { (a26 + 2) & !3 };
    (q + 32) >> 6
}

/// the scaler's vectors are in 6 samples a pixel across, divided back as (v + 3) / 6 truncated
fn down6(v: i64) -> i64 {
    (v + 3) / 6
}

fn glyph_line(g: &GlyphMetrics) -> String {
    format!("glyph adv {} lsb {} rsb {} tsb {} advh {} bsb {} voy {}", g.adv, g.lsb, g.rsb, g.tsb, g.advh, g.bsb, g.voy)
}

/// where a layout finds the characters its font does not map
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fallback {
    /// the system's font fallback (its fonts: not modelled)
    System,
    /// an empty IDWriteFontFallback: the font's glyph 0
    None,
}

/// a layout's maxBidiReorderingDepth: MEASURED (golden dwrite, fallback-*: 48 layouts) the most
/// runs of one resolved level on a line (left to right, right to left, left to right: 3)
fn depth(lv: &[u8], lines: &[(usize, usize)]) -> usize {
    lines.iter().map(|&(s, e)| 1 + lv[s..e].windows(2).filter(|w| w[0] != w[1]).count()).max().unwrap_or(1)
}

/// a text layout of the face's font, and the fallback's
fn layout(p: &Parsed, em: i64, mw4: i64, wrap: bool, fallback: Fallback, cps: &[u32]) -> Res<Vec<String>> {
    let mut out = Vec::new();
    let name = p.family.encode_utf16().map(|u| format!(" {u}")).collect::<String>();
    if p.family.is_empty() {
        // MEASURED: a font without a family name is in no collection's family: E_FAIL
        hit(clause::DW_LAYOUT_NAME);
        out.push("dwlayout families 1 hr 80004005 name".into());
        return Ok(out);
    }
    hit(clause::DW_LAYOUT_FAMILY);
    out.push(format!("dwlayout families 1 hr 00000000 name{name}"));
    if cps.iter().any(|&c| glyph_of(p, c) == 0) {
        match fallback {
            Fallback::System => hit(clause::DW_LAYOUT_FALLBACK),
            Fallback::None => hit(clause::DW_LAYOUT_NOFALLBACK),
        }
    }
    // each character's font: its design advance and the font's line metrics
    let m = metrics(p);
    let own = fallback::LineMetrics { upm: m.upm, asc: m.asc, desc: m.desc, gap: m.gap };
    let fonts: Vec<(u16, fallback::LineMetrics)> = match fallback {
        Fallback::None => cps.iter().map(|&c| (p.hmtx[glyph_of(p, c) as usize].0, own)).collect(),
        Fallback::System => {
            let mut v = Vec::new();
            for pc in fallback::pieces(p, cps)? {
                let fm = match pc.pick {
                    fallback::Pick::Own | fallback::Pick::None => own,
                    fallback::Pick::Sys(s) => s.metrics(),
                };
                v.extend(pc.glyphs.iter().map(|g| (g.1, fm)));
            }
            v
        }
    };
    // (the break analysis is of UTF-16 units: a character's is its first unit's, the break
    // after it its last's)
    let len = |c: u32| if c >= 0x10000 { 2 } else { 1 };
    let bu = linebreak::breaks(cps);
    let mut br = Vec::with_capacity(cps.len());
    let mut u = 0;
    for &c in cps {
        br.push((bu[u].0, bu[u + len(c) - 1].1, bu[u].2, bu[u].3));
        u += len(c);
    }
    let adv: Vec<f32> = fonts.iter().map(|&(a, fm)| a as f32 * (em as f32 / fm.upm as f32)).collect();
    hit(clause::DW_LAYOUT_LINE);
    // MEASURED: the line gap above the ascent; a line of several fonts takes the greatest
    // baseline and the greatest descent of its characters' fonts
    let line_metrics = |s: usize, e: usize| {
        let mut base = 0.0f32;
        let mut desc = 0.0f32;
        for &(_, fm) in &fonts[s..e] {
            base = base.max((fm.asc as i32 + fm.gap as i32) as f32 * em as f32 / fm.upm as f32);
            desc = desc.max(fm.desc as f32 * em as f32 / fm.upm as f32);
        }
        (base, base + desc)
    };
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
    let height: f32 = lines.iter().map(|&(s, e)| line_metrics(s, e).1).fold(0.0, |a, b| a + b);
    // the resolved bidi levels (MEASURED: a left-to-right paragraph); a cluster at an odd one
    // is right to left
    let classes: Vec<bidi::Class> = cps.iter().map(|&c| bidi::class(c)).collect();
    let (_, mut lv) = bidi::analyze(&classes, 0);
    // (MEASURED, fallback-bidi: a line's trailing whitespace takes the paragraph's level, UAX #9
    // L1, line by line)
    for &(s, e) in &lines {
        let mut k = e;
        while k > s && ws(k - 1) {
            k -= 1;
            lv[k] = 0;
        }
    }
    hit(clause::DW_LAYOUT_DEPTH);
    let depth = depth(&lv, &lines);
    // (MEASURED: left is negative zero)
    out.push(format!("metrics hr 00000000 left {} top 00000000 width {} wtrail {} height {} lines {} depth {depth}", b(-0.0), b(width), b(wtrail), b(height), lines.len()));
    let units = |s: usize, e: usize| cps[s..e].iter().map(|&c| len(c)).sum::<usize>();
    for (&(s, e), w) in lines.iter().zip(&widths) {
        let (base, lh) = line_metrics(s, e);
        out.push(format!("line {} trail {} nl 0 height {} base {} trimmed 0", units(s, e), w.2, b(lh), b(base)));
    }
    hit(clause::DW_LAYOUT_CLUSTERS);
    let cl: Vec<String> = (0..cps.len()).map(|i| format!(" {}/{}/{}{}0{}{}", b(adv[i]), len(cps[i]), (br[i].1 == linebreak::CAN) as u8, br[i].2, br[i].3, lv[i] % 2)).collect();
    out.push(format!("clusters hr 00000000 n {}{}", cps.len(), cl.concat()));
    Ok(out)
}

impl Dwrite {

    pub fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let n = |i: usize| t.get(i).and_then(|v| crate::case::num(v)).ok_or(Fail::Bad);
        // MEASURED: an op on the face before one is loaded prints "<op> noface"
        macro_rules! face {
            () => {
                match self.font.as_ref() {
                    Some(p) => p,
                    None => {
                        hit(clause::DW_NOFACE);
                        out.push(format!("{} noface", t[0]));
                        return Ok(true);
                    }
                }
            };
        }
        match t[0].as_str() {
            "dwfont" => {
                if t.len() != 2 {
                    return Err(Fail::Bad);
                }
                self.font = None;
                self.file = None;
                let b = hex(&t[1]).ok_or(Fail::Bad)?;
                // MEASURED (golden dwrite, file cases): Analyze by the signature, the header and
                // table directory (a collection's: its offsets) whole; the face, of the file's
                // first font, fails with DWRITE_E_FILEFORMAT where maxp is missing or a table is cut
                let ft = analyze(&b);
                hit(clause::DW_FILE_ANALYZE);
                let (sup, file, face, faces) = match ft {
                    FileType::TrueType => (1, 2, 1, 1),
                    FileType::Cff => (1, 1, 0, 1),
                    FileType::Collection(n) => (1, 3, 2, n),
                    FileType::Unknown => (0, 0, 6, 0),
                };
                out.push(format!("dwfont analyze hr 00000000 supported {sup} file {file} face {face} faces {faces}"));
                let Some(at) = face_at(&b, ft) else {
                    hit(clause::DW_FILE_OTHER);
                    return Ok(true);
                };
                match parse_face(&b, at) {
                    Some(mut p) => {
                        out.push("dwfont face hr 00000000".into());
                        if ft == FileType::Cff {
                            // (a CFF face of TrueType tables: no outlines)
                            p.outlines = false;
                            p.glyphs = vec![None; p.num_glyphs as usize];
                        }
                        self.font = Some(p);
                        self.file = Some(ft);
                    }
                    None => {
                        hit(clause::DW_FILE_OTHER);
                        out.push("dwfont face hr 88985000".into());
                    }
                }
            }
            "dwinfo" => {
                let p = face!();
                hit(clause::DW_INFO);
                let ty = match self.file {
                    Some(FileType::Cff) => 0,
                    Some(FileType::Collection(_)) => 2,
                    Some(FileType::TrueType | FileType::Unknown) | None => 1,
                };
                out.push(format!("dwinfo glyphs {} type {ty} index 0 sim 0 symbol {} mono {} vert 0", p.num_glyphs, p.symbol as u8, p.post.is_some_and(|x| x.2 != 0) as u8));
            }
            "dwmetrics" => {
                let m = metrics(face!());
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
                let mut m = metrics(face!());
                if em <= 0 || ppd <= 0 {
                    // MEASURED: E_INVALIDARG
                    hit(clause::DW_GDI_INVALID);
                    out.push("dwgdimetrics hr 80070057".into());
                    return Ok(true);
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
                let p = face!();
                if t.len() < 2 {
                    return Err(Fail::Bad);
                }
                hit(clause::DW_CMAP);
                let mut s = String::from("dwcmap");
                for i in 1..t.len() {
                    // (a glyph the face does not have: 0)
                    let g = glyph_of(p, n(i)? as u32);
                    s += &format!(" {}", if g < p.num_glyphs { g } else { 0 });
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
                let sideways = n(at - 1)? != 0;
                if gs.is_empty() {
                    return Err(Fail::Bad);
                }
                let p = face!();
                let m = metrics(p);
                if p.num_glyphs == 0 {
                    // MEASURED: a face of no glyphs (no hhea or hmtx): E_INVALIDARG
                    hit(clause::DW_FILE_OTHER);
                    out.push(format!("{} hr 80070057", t[0]));
                    return Ok(true);
                }
                if !gdi && self.file == Some(FileType::Cff) {
                    // MEASURED: a CFF face without its CFF table: DWRITE_E_FILEFORMAT
                    hit(clause::DW_FILE_OTHER);
                    out.push("dwglyph hr 88985000".into());
                    return Ok(true);
                }
                if sideways {
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
                    v.push(glyph_metrics(p, &m, g).ok_or(Fail::Bad)?);
                    gids.push(g);
                }
                if gdi {
                    let (em, ppd) = (n(1)?, n(2)?);
                    if em <= 0 || ppd <= 0 {
                        hit(clause::DW_GDI_INVALID);
                        out.push("dwgdiglyph hr 80070057".into());
                        return Ok(true);
                    }
                    if !p.outlines {
                        // MEASURED: glyphs without outlines have GDI compatible metrics all 0
                        hit(clause::DW_FILE_OTHER);
                        out.push(format!("{} hr 00000000", t[0]));
                        out.extend(v.iter().map(|_| glyph_line(&GlyphMetrics::default())));
                        return Ok(true);
                    }
                    if sideways {
                        hit(clause::DW_GDI_GLYPH);
                    }
                    hit(clause::DW_GDI_GLYPH_H);
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
                    // (the vertical advance: rounded half down, and down up to 65/128)
                    // (re/dwrite.md, scl_RoundCurrentSideBearingPnt: the vertical origin
                    // phantom is the origin in 26.6 rounded to the pixel, the advance's end the
                    // origin less the advance height rounded half away to 16.16 and to 26.6,
                    // then rounded to the pixel; the advance width see phantom_px, but the
                    // classic one below 256 ppem the 16.16 truncated scale's)
                    let p3 = |v: i32| (raster::direct(v as i64, ppem, upm) + 32) & !63;
                    let pxv = |v: i32| p3(v) >> 6;
                    let pxa = |o: i32, h: i32| {
                        let n = -(h as i64) * (ppem << 16);
                        let h16 = if n >= 0 { (n + upm / 2) / upm } else { -((upm / 2 - n) / upm) };
                        (p3(o) - ((p3(o) + 32 + ((h16 + 0x200) >> 10)) & !63)) >> 6
                    };
                    let classic = !nat && !sideways && ppem <= 255;
                    let px = |v: i32| if classic { (v as i64 * fix + 0x8000) >> 16 } else { phantom_px(v as i64, ppem, upm, sideways) };
                    // (the em fraction alone, and a fraction back in design units: a side bearing
                    // is the difference of its two edges' em fractions, each rounded on its own)
                    let ef = |k: i64, n: i64| (2 * k * 65536 + n * ppem).div_euclid(2 * n * ppem);
                    let fu = |f: i64| (2 * f * upm + 65536).div_euclid(131072) as i32;
                    if ppem > 4096 {
                        // (past the sizes measured: the generators write none)
                        return Err(Fail::Bad);
                    }
                    if ppem == upm && !sideways {
                        // (re/dwrite.md: the transform is the em itself, the design metrics as they
                        // are)
                        hit(clause::DW_GDI_EDGE);
                        out.push(format!("{} hr 00000000", t[0]));
                        out.extend(v.iter().map(glyph_line));
                        return Ok(true);
                    }
                    let to = ToEm::new(ppem);
                    let (vadv, vorg) = match (&p.vmtx, &p.os2) {
                        (Some(_), _) => (None, None),
                        (None, Some(o)) => (Some(o.typo.0 as i32 - o.typo.1 as i32), Some(o.typo.0 as i32)),
                        (None, None) => p.hhea.map_or((None, None), |h| (Some(h.0 as i32 - h.1 as i32), Some(h.0 as i32))),
                    };
                    for (g, gid) in v.iter_mut().zip(&gids) {
                        // (sideways: the glyph's y is drawn across, so the 6 samples are its y)
                        let (nx, ny) = if sideways { (1, 6) } else { (6, 1) };
                        let bb = raster::black_box(p, ppem, *gid, nx, ny)?.unwrap_or((0, 0, 0, 0));
                        let o = vorg.unwrap_or(g.voy);
                        let (advp, voyp, advhp) = (px(g.adv as i32), pxv(o), pxa(o, vadv.unwrap_or(g.advh as i32)));
                        hit(clause::DW_GDI_EDGE);
                        // (each edge's em fraction in 16.16: the origin, the advances and the box)
                        let (eo, eah, eaw, left, top, right, bottom) = if sideways {
                            // (the origin of voyp pixels from the box's top column L: the scaler's
                            // tsb vector, divided back, and GetMetrics' own L / 6 do not cancel)
                            let l = bb.1;
                            let vo = (l * 65536) / 6 - down6((l * 64 + 384 * voyp) * 1024);
                            let eo = to.across(vo);
                            let eah = to.across(down6(-6 * 65536 * advhp));
                            (eo, eah, to.whole(advp), to.whole(bb.0), to.sixths(bb.1, -1.0), to.whole(bb.2), to.sixths(bb.3, -1.0))
                        } else {
                            // (classic advances: an exact fraction of the whole pixels)
                            let eaw = if classic { ef(advp, 1) } else { to.whole(advp) };
                            (to.whole(voyp), to.whole(advhp), eaw, to.sixths(bb.0, 1.0), to.whole(-bb.1), to.sixths(bb.2, 1.0), to.whole(-bb.3))
                        };
                        *g = GlyphMetrics {
                            adv: fu(eaw) as u32,
                            lsb: fu(left),
                            rsb: fu(eaw - right),
                            tsb: fu(eo - top),
                            advh: fu(eah) as u32,
                            bsb: fu(bottom - eo + eah),
                            voy: fu(eo),
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
                // (the executor: no text is a bad line)
                if cps.is_empty() {
                    return Err(Fail::Empty);
                }
                // (supplementary characters: the generators write none)
                if cps.iter().any(|&c| c >= 0x10000) {
                    return Err(Fail::Bad);
                }
                let p = face!();
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
                // (re/dwrite.md: past 255 ppem the classic advance is the natural one)
                let fix = ((em << 16) / upm as i64) as i64;
                let nat = |g: u16| phantom_px(adv(g) as i64, em, upm as i64, false) as f32;
                out.push(format!("gdiplace hr 00000000 {}", list(gids.iter().map(|&g| f(if em <= 255 { ((adv(g) as i64 * fix + 0x8000) >> 16) as f32 } else { nat(g) })).collect())));
                hit(clause::DW_SHAPE_NATPLACE);
                out.push(format!("natplace hr 00000000 {}", list(gids.iter().map(|&g| f(nat(g))).collect())));
            }
            "dwrun" => {
                let (mode, fit) = raster::Mode::parse(t.get(1).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?;
                let (em, x4, y4) = (n(2)?, n(3)?, n(4)?);
                // (the executor: no glyph, or a glyph without its advance, is a bad line)
                if t.len() < 7 || t.len() % 2 == 0 {
                    return Err(Fail::Empty);
                }
                let mut run = Vec::new();
                for i in (5..t.len()).step_by(2) {
                    run.push((n(i)? as u16, n(i + 1)?));
                }
                let p = face!();
                if p.num_glyphs == 0 {
                    hit(clause::DW_FILE_OTHER);
                    out.push("dwrun hr 80070057".into());
                    return Ok(true);
                }
                out.push("dwrun hr 00000000".into());
                out.extend(raster::run(p, mode, fit, em, x4, y4, &run)?);
            }
            "dwfallback" => {
                if t.len() < 2 {
                    return Err(Fail::Empty);
                }
                let mut cps = Vec::new();
                for i in 1..t.len() {
                    let c = n(i)?;
                    if !(0..=0x10ffff).contains(&c) || (0xd800..0xe000).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cps.push(c as u32);
                }
                let p = face!();
                if p.family.is_empty() {
                    // (the collection of a font without a family name has no family: as the layout's)
                    hit(clause::DW_LAYOUT_NAME);
                    out.push("dwfallback hr 80004005".into());
                    return Ok(true);
                }
                out.push("dwfallback hr 00000000".into());
                let utf16 = |s: &str| s.encode_utf16().map(|u| format!(" {u}")).collect::<String>();
                for pc in fallback::pieces(p, &cps)? {
                    // MEASURED: the scale is always 1
                    out.push(format!("map {} scale 3f800000", pc.units()));
                    let (name, m) = match pc.pick {
                        fallback::Pick::None => {
                            out.push("nofont".into());
                            continue;
                        }
                        fallback::Pick::Own => {
                            let m = metrics(p);
                            (utf16(&p.family), fallback::LineMetrics { upm: m.upm, asc: m.asc, desc: m.desc, gap: m.gap })
                        }
                        fallback::Pick::Sys(s) => (utf16(s.name()), s.metrics()),
                    };
                    // MEASURED: normal weight, style and stretch (the own font's OS/2: 400,
                    // REGULAR, 5), no simulation
                    out.push(format!("font 400 0 5 sim 0 name{name}"));
                    out.push(format!("fmetrics upm {} asc {} desc {} gap {}", m.upm, m.asc, m.desc, m.gap));
                    out.push(format!("glyphs{}", pc.glyphs.iter().map(|(g, a)| format!(" {g}/{a}")).collect::<String>()));
                }
            }
            "dwlayout" | "dwlayoutnf" => {
                let (em, mw4, wrap) = (n(1)?, n(2)?, n(3)?);
                let mut cps = Vec::new();
                for i in 4..t.len() {
                    let c = n(i)?;
                    if !(0..=0x10ffff).contains(&c) || (0xd800..0xe000).contains(&c) {
                        return Err(Fail::Bad);
                    }
                    cps.push(c as u32);
                }
                if cps.is_empty() {
                    return Err(Fail::Bad);
                }
                let p = face!();
                let fallback = match t[0].as_str() {
                    "dwlayoutnf" => Fallback::None,
                    _ => Fallback::System,
                };
                out.extend(layout(p, em, mw4, wrap != 0, fallback, &cps)?);
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
                    out.push(format!("run {pos} {len} {} {} shapes {}", sc.code, sc.num, sc.shapes.value()));
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
                    cs.push(c as u32);
                }
                if cs.is_empty() {
                    return Err(Fail::Bad);
                }
                let classes: Vec<bidi::Class> = cs.iter().map(|&c| bidi::class(c)).collect();
                let (ex, lv) = bidi::analyze(&classes, para);
                // (a level for each UTF-16 unit)
                let units = |v: &[u8]| cs.iter().zip(v).map(|(&c, l)| if c >= 0x10000 { format!(" {l} {l}") } else { format!(" {l}") }).collect::<String>();
                out.push("dwbidi hr 00000000".into());
                out.push(format!("explicit{}", units(&ex)));
                out.push(format!("resolved{}", units(&lv)));
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
                let b = linebreak::breaks(&cps);
                out.push("dwbreak hr 00000000".into());
                out.push(format!("breaks{}", b.iter().map(|x| format!(" {}{}{}{}", x.0, x.1, x.2, x.3)).collect::<String>()));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
