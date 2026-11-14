//! The Unicode Character Database files the text models read (spec/ucd, Unicode 15.1): each a
//! list of code point ranges and their value, looked up by binary search.
use std::sync::OnceLock;

/// a UCD property file's ranges (lo, hi, value), sorted, not overlapping
pub struct Table {
    ranges: Vec<(u32, u32, &'static str)>,
}

impl Table {
    /// the "XXXX[..YYYY] ; Value [# ...]" lines of a UCD file (field `field` after the range)
    fn parse(text: &'static str, field: usize) -> Table {
        let mut ranges = Vec::new();
        for l in text.lines() {
            let l = l.split('#').next().unwrap_or("");
            let mut f = l.split(';').map(str::trim);
            let (Some(r), Some(v)) = (f.next(), f.nth(field)) else { continue };
            if r.is_empty() {
                continue;
            }
            let (lo, hi) = r.split_once("..").unwrap_or((r, r));
            let (Ok(lo), Ok(hi)) = (u32::from_str_radix(lo, 16), u32::from_str_radix(hi, 16)) else { continue };
            ranges.push((lo, hi, v));
        }
        ranges.sort();
        Table { ranges }
    }

    /// the value of code point `c`, None when the file does not list it
    pub fn get(&self, c: u32) -> Option<&'static str> {
        let i = self.ranges.partition_point(|r| r.1 < c);
        self.ranges.get(i).filter(|r| r.0 <= c).map(|r| r.2)
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

/// a script's ISO 15924 code and number by its Unicode long name (PropertyValueAliases' value,
/// iso15924.txt's PVA column)
pub fn iso15924(long: &str) -> Option<(&'static str, u32)> {
    static T: OnceLock<Vec<(&'static str, u32, &'static str)>> = OnceLock::new();
    let t = T.get_or_init(|| {
        include_str!("../spec/ucd/iso15924.txt")
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| {
                let f: Vec<&str> = l.split(';').collect();
                Some((*f.first()?, f.get(1)?.parse().ok()?, *f.get(4)?))
            })
            .collect()
    });
    t.iter().find(|e| e.2 == long).map(|e| (e.0, e.1))
}
