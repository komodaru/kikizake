//! The system font fallback (IDWriteFactory2::GetSystemFontFallback, MapCharacters): the font a
//! text's characters are drawn in when the layout's own font does not map them. Which of the
//! machine's fonts that is, and those fonts' metrics and glyphs, are the machine's, not the
//! API's: MEASURED (golden dwrite-32 and -64, fallback-chars; the same on both) for the
//! characters the cases name, the table below. The cases name no other character the font does
//! not map (tests/windows.rs, no_generated_line_is_bad).
use crate::spec::clause;
use crate::spec::hit;
use crate::ttf::{glyph_of, Parsed};
use super::super::{Fail, Res};

/// a font of the machine's that the fallback picks
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sys {
    SegoeUi,
    SegoeUiEmoji,
    YuGothicUi,
    NirmalaUi,
    MalgunGothic,
    LeelawadeeUi,
}

/// a font's DWRITE_FONT_METRICS that a layout takes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineMetrics {
    pub upm: u16,
    pub asc: u16,
    pub desc: u16,
    pub gap: i16,
}

impl Sys {
    /// its family name (en-us)
    pub fn name(self) -> &'static str {
        match self {
            Sys::SegoeUi => "Segoe UI",
            Sys::SegoeUiEmoji => "Segoe UI Emoji",
            Sys::YuGothicUi => "Yu Gothic UI",
            Sys::NirmalaUi => "Nirmala UI",
            Sys::MalgunGothic => "Malgun Gothic",
            Sys::LeelawadeeUi => "Leelawadee UI",
        }
    }
    pub fn metrics(self) -> LineMetrics {
        match self {
            Sys::SegoeUi | Sys::SegoeUiEmoji | Sys::YuGothicUi | Sys::NirmalaUi | Sys::LeelawadeeUi => LineMetrics { upm: 2048, asc: 2210, desc: 514, gap: 0 },
            Sys::MalgunGothic => LineMetrics { upm: 2048, asc: 2229, desc: 495, gap: 0 },
        }
    }
}

/// the Unicode block a character is in (Blocks.txt), as far as the table's characters go: a
/// piece of the fallback's answer ends where the block changes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    BasicLatin,
    Latin1Supplement,
    GreekAndCoptic,
    Cyrillic,
    Hebrew,
    Arabic,
    Devanagari,
    Thai,
    CurrencySymbols,
    Arrows,
    MiscellaneousSymbols,
    Hiragana,
    Katakana,
    CjkUnifiedIdeographs,
    HangulSyllables,
    PrivateUseArea,
    Emoticons,
    SupplementaryPrivateUseAreaB,
}

/// what the fallback answers for a character the font does not map: its block, and the font,
/// glyph and design advance (None: no font of the machine's maps it)
pub fn sys(c: u32) -> Option<(Block, Option<(Sys, u16, u16)>)> {
    use Block::*;
    use Sys::*;
    Some(match c {
        0x7e => (BasicLatin, Some((SegoeUi, 97, 1401))),
        0xe9 => (Latin1Supplement, Some((SegoeUi, 112, 1071))),
        0x378 => (GreekAndCoptic, None),
        0x3b1 => (GreekAndCoptic, Some((SegoeUi, 303, 1258))),
        0x430 => (Cyrillic, Some((SegoeUi, 519, 1042))),
        0x5d1 => (Hebrew, Some((SegoeUi, 2921, 1168))),
        0x627 => (Arabic, Some((SegoeUi, 2317, 513))),
        0x628 => (Arabic, Some((SegoeUi, 2318, 1881))),
        0x915 => (Devanagari, Some((NirmalaUi, 248, 1768))),
        0xe01 => (Thai, Some((LeelawadeeUi, 251, 1175))),
        0x20ac => (CurrencySymbols, Some((SegoeUi, 189, 1104))),
        0x2192 => (Arrows, Some((SegoeUi, 2862, 1767))),
        0x2603 => (MiscellaneousSymbols, Some((SegoeUiEmoji, 3869, 2812))),
        0x3042 => (Hiragana, Some((YuGothicUi, 23644, 1671))),
        0x30a2 => (Katakana, Some((YuGothicUi, 23727, 1589))),
        0x4e00 => (CjkUnifiedIdeographs, Some((YuGothicUi, 1202, 2048))),
        0xac00 => (HangulSyllables, Some((MalgunGothic, 636, 2048))),
        0xe000 => (PrivateUseArea, None),
        0x1f601 => (Emoticons, Some((SegoeUiEmoji, 2267, 2812))),
        0x10fffd => (SupplementaryPrivateUseAreaB, None),
        _ => return None,
    })
}

/// the font a character is drawn in
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// the layout's own font
    Own,
    /// a font of the machine's
    Sys(Sys),
    /// none (the layout draws the own font's glyph 0)
    None,
}

/// one piece of MapCharacters' answer: the characters (code points) one font takes
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Piece {
    pub pick: Pick,
    pub cps: Vec<u32>,
    /// (glyph, design advance) of each character in that font (the own font's for Pick::None)
    pub glyphs: Vec<(u16, u16)>,
}

impl Piece {
    /// its length in UTF-16 units
    pub fn units(&self) -> usize {
        self.cps.iter().map(|&c| if c >= 0x10000 { 2 } else { 1 }).sum()
    }
}

/// what the fallback answers for a text: its pieces. MEASURED (golden dwrite, fallback-*): a
/// character the own font maps is the own font's; the others the table's; a piece ends where the
/// font changes, and, among the machine's fonts (and none), where the block changes too
/// (Hebrew | Greek, both Segoe UI; CJK | Hiragana, both Yu Gothic UI; Greek | Supplementary
/// Private Use Area-B, both none), the own font's going on across blocks
pub fn pieces(p: &Parsed, cps: &[u32]) -> Res<Vec<Piece>> {
    hit(clause::DW_FALLBACK_MAP);
    let mut v: Vec<(Pick, Option<Block>, Piece)> = Vec::new();
    for &c in cps {
        let g = glyph_of(p, c);
        let (pick, block, glyph) = if g != 0 {
            (Pick::Own, None, (g, p.hmtx[g as usize].0))
        } else {
            hit(clause::DW_FALLBACK_FONTS);
            // (a character outside the table: the generators never write one)
            match sys(c).ok_or(Fail::Bad)? {
                (b, Some((s, g, a))) => (Pick::Sys(s), Some(b), (g, a)),
                (b, None) => (Pick::None, Some(b), (0, p.hmtx[0].0)),
            }
        };
        match v.last_mut() {
            Some(l) if l.0 == pick && l.1 == block => {
                l.2.cps.push(c);
                l.2.glyphs.push(glyph);
            }
            _ => v.push((pick, block, Piece { pick, cps: vec![c], glyphs: vec![glyph] })),
        }
    }
    Ok(v.into_iter().map(|e| e.2).collect())
}
