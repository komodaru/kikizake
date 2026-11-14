//! The Unicode Character Database files the text models read (spec/ucd, Unicode 16.0, the version
//! Windows' DirectWrite answers by): each a list of code point ranges and their value, looked up
//! by binary search, with the file's @missing defaults for the code points it does not list.
use std::sync::OnceLock;

/// a UCD property file's ranges (lo, hi, value), sorted, not overlapping, and its @missing lines
/// (a later one over an earlier)
pub struct Table {
    ranges: Vec<(u32, u32, &'static str)>,
    missing: Vec<(u32, u32, &'static str)>,
}

/// "XXXX[..YYYY] ; Value": the range and the value (field `field` after the range)
fn range(l: &'static str, field: usize) -> Option<(u32, u32, &'static str)> {
    let mut f = l.split(';').map(str::trim);
    let (r, v) = (f.next()?, f.nth(field)?);
    let (lo, hi) = r.split_once("..").unwrap_or((r, r));
    Some((u32::from_str_radix(lo, 16).ok()?, u32::from_str_radix(hi, 16).ok()?, v))
}

impl Table {
    /// the "XXXX[..YYYY] ; Value [# ...]" lines of a UCD file (field `field` after the range)
    fn parse(text: &'static str, field: usize) -> Table {
        let mut ranges = Vec::new();
        let mut missing = Vec::new();
        for l in text.lines() {
            if let Some(m) = l.strip_prefix("# @missing:") {
                missing.extend(range(m, field));
                continue;
            }
            ranges.extend(range(l.split('#').next().unwrap_or(""), field));
        }
        ranges.sort();
        Table { ranges, missing }
    }

    /// the value of code point `c`: its line, else the last @missing line over it; None when
    /// neither is (a file without @missing lines)
    pub fn get(&self, c: u32) -> Option<&'static str> {
        let i = self.ranges.partition_point(|r| r.1 < c);
        let listed = self.ranges.get(i).filter(|r| r.0 <= c).map(|r| r.2);
        listed.or_else(|| self.missing.iter().rev().find(|r| r.0 <= c && c <= r.1).map(|r| r.2))
    }

    /// every range the file lists
    pub fn ranges(&self) -> &[(u32, u32, &'static str)] {
        &self.ranges
    }
}

/// Scripts.txt: each code point's script (long name, e.g. Latin); unlisted ones are Unknown
pub fn scripts() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| Table::parse(include_str!("../spec/ucd/Scripts.txt"), 0))
}

/// LineBreak.txt: each code point's line break class (e.g. AL)
pub fn line_break() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| Table::parse(include_str!("../spec/ucd/LineBreak.txt"), 0))
}

/// DerivedBidiClass.txt: each code point's bidi class (e.g. L)
pub fn bidi_class() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| Table::parse(include_str!("../spec/ucd/DerivedBidiClass.txt"), 0))
}

/// DerivedGeneralCategory.txt: each code point's general category (e.g. Lu; Cn unassigned)
pub fn general_category() -> &'static Table {
    static T: OnceLock<Table> = OnceLock::new();
    T.get_or_init(|| Table::parse(include_str!("../spec/ucd/DerivedGeneralCategory.txt"), 0))
}

/// a script's ISO 15924 code and number by its Unicode long name: the code PropertyValueAliases
/// gives the script ("sc ; Code ; Long"), the number iso15924.txt gives the code
pub fn iso15924(long: &str) -> Option<(&'static str, u32)> {
    static T: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    static N: OnceLock<Vec<(&'static str, u32)>> = OnceLock::new();
    let sc = T.get_or_init(|| {
        include_str!("../spec/ucd/PropertyValueAliases.txt")
            .lines()
            .filter_map(|l| {
                let f: Vec<&str> = l.split('#').next()?.split(';').map(str::trim).collect();
                match f.as_slice() {
                    ["sc", code, long, ..] => Some((*code, *long)),
                    _ => None,
                }
            })
            .collect()
    });
    let num = N.get_or_init(|| {
        include_str!("../spec/ucd/iso15924.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let f: Vec<&str> = l.split(';').collect();
                Some((*f.first()?, f.get(1)?.parse().ok()?))
            })
            .collect()
    });
    let code = sc.iter().find(|e| e.1 == long)?.0;
    num.iter().find(|e| e.0 == code).map(|&(c, n)| (c, n))
}
