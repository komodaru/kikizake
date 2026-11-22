//! AnalyzeScript's scripts (spec/dwrite.txt DW.SCRIPT.UCD, WINDOWS, ALLCOMMON, NOVISUAL) for
//! every code point (kind exhaustive), against Scripts.txt, DerivedGeneralCategory.txt and
//! iso15924.txt (spec/ucd) read here on their own.
use kikizake::model::dwrite::script::{self, Script};

/// a UCD file's property of every code point (None where it lists none)
fn table(text: &str) -> Vec<Option<String>> {
    let mut v = vec![None; 0x11_0000];
    for l in text.lines() {
        let body = l.split('#').next().unwrap_or("");
        let mut f = body.split(';').map(str::trim);
        let (Some(r), Some(val)) = (f.next(), f.next()) else { continue };
        if r.is_empty() {
            continue;
        }
        let (lo, hi) = r.split_once("..").unwrap_or((r, r));
        let (lo, hi) = (u32::from_str_radix(lo, 16).unwrap(), u32::from_str_radix(hi, 16).unwrap());
        for c in lo..=hi {
            v[usize::try_from(c).unwrap()] = Some(val.to_string());
        }
    }
    v
}

/// each Unicode long name's code (PropertyValueAliases.txt "sc ; Code ; Long", Georgian's
/// Geor where iso15924.txt names Geok too) and that code's number (iso15924.txt)
fn iso() -> Vec<(String, String, u32)> {
    let num: Vec<(String, u32)> = include_str!("../spec/ucd/iso15924.txt")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let f: Vec<&str> = l.split(';').collect();
            Some((f.first()?.to_string(), f.get(1)?.parse().ok()?))
        })
        .collect();
    include_str!("../spec/ucd/PropertyValueAliases.txt")
        .lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.split('#').next()?.split(';').map(str::trim).collect();
            match f.as_slice() {
                ["sc", code, long, ..] => Some((long.to_string(), code.to_string(), num.iter().find(|n| n.0 == *code)?.1)),
                _ => None,
            }
        })
        .collect()
}

/// Windows' own codes (DW.SCRIPT.WINDOWS): the 14 scripts given another's
const SHIFTED: [(&str, &str, u32); 14] = [
    ("Chorasmian", "Dogr", 328),
    ("Dives_Akuru", "Gong", 312),
    ("Dogra", "Rohg", 167),
    ("Elymaic", "Maka", 366),
    ("Gunjala_Gondi", "Medf", 265),
    ("Hanifi_Rohingya", "Sogo", 142),
    ("Khitan_Small_Script", "Sogd", 141),
    ("Makasar", "Elym", 128),
    ("Medefaidrin", "Hmnp", 451),
    ("Nyiakeng_Puachue_Hmong", "Wcho", 283),
    ("Old_Sogdian", "Chrs", 109),
    ("Sogdian", "Diak", 342),
    ("Wancho", "Kits", 288),
    ("Mro", "Mroo", 199),
];

/// the common characters Windows puts in Arabic (DW.SCRIPT.WINDOWS)
fn arabic(c: u32) -> bool {
    matches!(c, 0x605 | 0x61b | 0x61f | 0x640 | 0x6dd | 0x8e2) || (0x1ec71..=0x1ecb4).contains(&c) || (0x1ed01..=0x1ed3d).contains(&c)
}

/// what a character alone is, as the clauses say: (code, number, shapes)
fn alone(c: u32, sc: &[Option<String>], gc: &[Option<String>], iso: &[(String, String, u32)]) -> (String, u32, u32) {
    let i = usize::try_from(c).unwrap();
    let long = sc[i].as_deref().unwrap_or("Unknown");
    let g = gc[i].as_deref().unwrap_or("Cn");
    let by = |name: &str| -> (String, u32, u32) {
        if let Some(s) = SHIFTED.iter().find(|s| s.0 == name) {
            return (s.1.to_string(), s.2, 0);
        }
        let x = iso.iter().find(|x| x.0 == name).unwrap_or_else(|| panic!("{name}"));
        (x.1.clone(), x.2, 0)
    };
    match long {
        "Common" if arabic(c) => by("Arabic"),
        "Common" if matches!(g, "Cc" | "Cf" | "Zl" | "Zp") => ("Zyyy".into(), 998, 1),
        "Unknown" if matches!(c, 0xfffe | 0xffff) => ("Zyyy".into(), 998, 1),
        "Common" | "Inherited" | "Unknown" => ("Zzzz".into(), 999, 0),
        name => by(name),
    }
}

fn as_tuple(s: &Script) -> (String, u32, u32) {
    (s.code.to_string(), s.num, s.shapes.value())
}

/// DW.SCRIPT.UCD, DW.SCRIPT.WINDOWS, DW.SCRIPT.ALLCOMMON: every code point alone is one run
/// of its UTF-16 length, of its Scripts.txt script's ISO 15924 code and number (Windows' 14
/// shifted ones and its Arabic common characters as they are), a no-visual one (Common Cc, Cf,
/// Zl, Zp, and U+FFFE, U+FFFF) Zyyy 998 NO_VISUAL, a common, inherited or unassigned one Zzzz
/// 999
#[test]
fn every_character_alone_is_its_script() {
    let sc = table(include_str!("../spec/ucd/Scripts.txt"));
    let gc = table(include_str!("../spec/ucd/DerivedGeneralCategory.txt"));
    let iso = iso();
    for c in 0..0x11_0000u32 {
        let runs = script::runs(&[c]).unwrap();
        let n = if c >= 0x10000 { 2 } else { 1 };
        assert_eq!(runs.len(), 1, "{c:x}");
        assert_eq!((runs[0].0, runs[0].1), (0, n), "{c:x}");
        assert_eq!(as_tuple(&runs[0].2), alone(c, &sc, &gc, &iso), "{c:x}");
    }
}

/// DW.SCRIPT.ALLCOMMON: text only of common characters (every Common or Inherited character
/// that is not no-visual nor Windows' Arabic, alone, doubled and around a space): one run Zzzz
/// 999 over all of it
#[test]
fn common_text_is_one_zzzz_run() {
    let sc = table(include_str!("../spec/ucd/Scripts.txt"));
    let gc = table(include_str!("../spec/ucd/DerivedGeneralCategory.txt"));
    for c in 0..0x11_0000u32 {
        let i = usize::try_from(c).unwrap();
        let common = matches!(sc[i].as_deref(), Some("Common" | "Inherited")) && !arabic(c) && !matches!(gc[i].as_deref(), Some("Cc" | "Cf" | "Zl" | "Zp"));
        if !common {
            continue;
        }
        for t in [vec![c, c], vec![0x20, c, 0x20]] {
            let units: usize = t.iter().map(|&x| if x >= 0x10000 { 2 } else { 1 }).sum();
            let runs = script::runs(&t).unwrap();
            assert_eq!(runs.iter().map(|r| (r.0, r.1, as_tuple(&r.2))).collect::<Vec<_>>(), [(0, units, ("Zzzz".into(), 999, 0))], "{c:x}");
        }
    }
}

/// DW.SCRIPT.NOVISUAL: every no-visual character and every unassigned one of the BMP, between,
/// before and after letters, after a space and twice: an unassigned one a Zzzz run of its own
/// that common characters before it join; a no-visual one a Zyyy NO_VISUAL run, common
/// characters before it a Zzzz run of their own, after it joining the script after them or a
/// Zzzz run at the end
#[test]
fn no_visual_and_unassigned_characters_in_context() {
    let sc = table(include_str!("../spec/ucd/Scripts.txt"));
    let gc = table(include_str!("../spec/ucd/DerivedGeneralCategory.txt"));
    let (a, sp) = (0x61u32, 0x20u32);
    let latn = || ("Latn".to_string(), 215u32, 0u32);
    let zzzz = || ("Zzzz".to_string(), 999u32, 0u32);
    let zyyy = || ("Zyyy".to_string(), 998u32, 1u32);
    for c in 0..0x11_0000u32 {
        let i = usize::try_from(c).unwrap();
        let novis = matches!(sc[i].as_deref(), Some("Common")) && !arabic(c) && matches!(gc[i].as_deref(), Some("Cc" | "Cf" | "Zl" | "Zp")) || matches!(c, 0xfffe | 0xffff);
        let unassigned = sc[i].is_none() && !matches!(c, 0xfffe | 0xffff) && c < 0x10000 && !(0xd800..0xe000).contains(&c);
        if !novis && !unassigned {
            continue;
        }
        let n = if c >= 0x10000 { 2 } else { 1 };
        let x = if novis { zyyy() } else { zzzz() };
        let cases: Vec<(Vec<u32>, Vec<(usize, usize, (String, u32, u32))>)> = if novis {
            vec![
                (vec![a, c, a], vec![(0, 1, latn()), (1, n, x.clone()), (1 + n, 1, latn())]),
                (vec![sp, c, a], vec![(0, 1, zzzz()), (1, n, x.clone()), (1 + n, 1, latn())]),
                (vec![c, sp, a], vec![(0, n, x.clone()), (n, 2, latn())]),
                (vec![a, c, sp], vec![(0, 1, latn()), (1, n, x.clone()), (1 + n, 1, zzzz())]),
                (vec![c, c], vec![(0, 2 * n, x.clone())]),
            ]
        } else {
            vec![
                (vec![a, c, a], vec![(0, 1, latn()), (1, n, x.clone()), (1 + n, 1, latn())]),
                (vec![sp, c, a], vec![(0, 1 + n, x.clone()), (1 + n, 1, latn())]),
                (vec![c, sp, a], vec![(0, n + 1, x.clone()), (n + 1, 1, latn())]),
                (vec![a, c, sp], vec![(0, 1, latn()), (1, n + 1, x.clone())]),
                (vec![c, c], vec![(0, 2 * n, x.clone())]),
            ]
        };
        for (t, want) in cases {
            let runs = script::runs(&t).unwrap();
            assert_eq!(runs.iter().map(|r| (r.0, r.1, as_tuple(&r.2))).collect::<Vec<_>>(), want, "{t:x?}");
        }
    }
}
