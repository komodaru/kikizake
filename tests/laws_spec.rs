//! Laws for clauses no other law checks and no Windows answer settles yet (kikizake trace lists
//! them): each written from the clause's documented text, independent of Windows' answers.
use proptest::prelude::*;
use kikizake::case::parse_cases;
use kikizake::model::run_case;

fn run(text: &str) -> Vec<String> {
    let c = parse_cases(&format!("case law\n{text}\nend\n"));
    run_case(&c[0]).unwrap_or_else(|e| panic!("{text}: {e:?}"))
}

proptest! {
    /// SetRectRgn: "the region becomes the rectangle's", whatever it was before
    #[test]
    fn setrect_is_the_rectangle(a in prop::array::uniform4(-9i32..9), b in prop::array::uniform4(-9i32..9)) {
        let set = run(&format!("rgn 0 rect {} {} {} {}\nsetrect 0 {} {} {} {}\nrgndata 0", a[0], a[1], a[2], a[3], b[0], b[1], b[2], b[3]));
        let made = run(&format!("rgn 0 rect {} {} {} {}\nrgndata 0", b[0], b[1], b[2], b[3]));
        prop_assert_eq!(&set[0], "setrect 1");
        prop_assert_eq!(&set[1..], &made[..]);
    }
}

/// GetBitmapBits of a DDB: rows top to bottom, each a multiple of 2 bytes, the count returned;
/// GetObject's BITMAP gives the same bytes per row (exhaustive over small sizes, both depths a
/// memory DC takes)
#[test]
fn ddb_rows_are_words() {
    for (kind, bpp) in [("mono", 1), ("bitmap", 32)] {
        for w in 1..=40 {
            for h in 1..=4 {
                let make = if kind == "mono" { format!("ddb 0 mono {w} {h}") } else { format!("ddb 0 bitmap {w} {h} 1 {bpp}") };
                let o = run(&format!("{make}\nbmbits 0\nbmobject 0"));
                let stride = (w * bpp + 15) / 16 * 2;
                assert_eq!(o[0], format!("bmbits {}", stride * h), "{make}");
                assert_eq!(o[1].len(), 2 * (stride * h) as usize, "{make}");
                assert_eq!(o[2], format!("bm bitmap {w} {h} {stride} 1 {bpp} 0"), "{make}");
            }
        }
    }
}

/// GetObject of a DIB section (GDI.DDB.DIBOBJECT): DIBSECTION, rows a multiple of 4 bytes, the
/// header with a positive height whatever the row order, BI_BITFIELDS for 16 bpp, biClrUsed the
/// color table's size, and GetObject(h, 0, NULL) the size of a BITMAP
#[test]
fn dibsection_object_is_its_header() {
    for bpp in [1, 4, 8, 16, 24, 32] {
        for w in 1..=33 {
            for dir in ["td", "bu"] {
                let o = run(&format!("dib 0 {w} 3 {bpp} {dir}\nbmobject 0"));
                let stride = (w * bpp + 31) / 32 * 4;
                let used = if bpp <= 8 { 1 << bpp } else { 0 };
                let comp = if bpp == 16 { 3 } else { 0 };
                assert_eq!(o, [format!("bm dibsection {w} 3 {stride} 1 {bpp} 1"), format!("dib {bpp} 3 {comp} {used}"), "bmsize bitmap".to_string()], "{bpp} bpp, {w} wide {dir}");
            }
        }
    }
}

/// GDI.MAP.FORMULA / ROUND: equal extents are a translation; otherwise the device coordinate is
/// within 9/16 of a pixel of Dx = (Lx - xWO) * xVE / xWE + xVO (each of the two terms off by at
/// most 1/32, the sum rounded to a pixel). Every extent of magnitude up to 16, points and
/// origins in [-32, 32) and [-8, 8) (as a Kani proof the division did not finish in 25 minutes)
#[test]
fn page_transform_is_the_formula() {
    use kikizake::model::gdixform::page;
    for w in (-16..=16).filter(|&w| w != 0) {
        for v in (-16..=16).filter(|&v| v != 0) {
            for a in -32..32 {
                for wo in -8..8 {
                    for vo in -8..8 {
                        let p = page(a, w, v, wo, vo) as i64;
                        let (a, w, v, wo, vo) = (a as i64, w as i64, v as i64, wo as i64, vo as i64);
                        if w == v {
                            assert_eq!(p, a - wo + vo);
                        }
                        let e = 16 * w * p - 16 * ((a - wo) * v + vo * w);
                        assert!(e.abs() <= 9 * w.abs(), "{a} {w} {v} {wo} {vo}: {p}");
                    }
                }
            }
        }
    }
}
