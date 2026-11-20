//! Laws of the GDI+ high quality stretch (src/model/gpstretch.rs): its kernel tables, and a
//! uniform image drawn at any scale staying uniform where the kernel lies inside it.
use kikizake::model::gdiplus::Round;
use kikizake::model::gpstretch::{self as st, Draw, Kernel};
use proptest::prelude::*;

/// the tent's running integral rises from 0 to 65536, symmetric about its middle
#[test]
fn tent_table_is_an_integral() {
    assert_eq!((st::tent(-128), st::tent(0), st::tent(128)), (0, 32768, 65536));
    for i in -200..200 {
        assert!(st::tent(i + 1) >= st::tent(i));
        assert_eq!(st::tent(i) + st::tent(-i), 65536, "i {i}");
    }
}

/// the cubic's: mirrored about the half, its extremes those of Keys' a = -1 integrated
#[test]
fn cubic_table_is_an_integral() {
    for i in -300..300 {
        assert_eq!(st::cubic(i) + st::cubic(-1 - i), 65536, "i {i}");
    }
    let v: Vec<i64> = (-300..300).map(st::cubic).collect();
    assert_eq!(*v.iter().min().unwrap(), -5461);
    assert_eq!(*v.iter().max().unwrap(), 70997);
    assert_eq!((st::cubic(-129), st::cubic(-128), st::cubic(127), st::cubic(128)), (-5461, -5461, 70997, 70997));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    /// a uniform opaque source drawn from well inside it is uniform on x64 (the weights sum to
    /// one exactly) in the rectangle's middle third, where the kernel reads no row past the
    /// locked ones, at any scale up to a 2x reduction, both kernels, any pixel offset but x64's
    /// unfitted half pixel reductions
    #[test]
    fn uniform_stays_uniform(c in any::<u32>(), sw in 6i64..12, sh in 6i64..12, dw in 1i64..30, dh in 1i64..30, cubic: bool, half: bool) {
        prop_assume!(2 * dw >= sw && 2 * dh >= sh);
        let c = c | 0xff00_0000;
        let k = if cubic { Kernel::Cubic } else { Kernel::Tent };
        let d = Draw { dst: (1, 1, dw, dh), src: (10, 10, sw, sh), half };
        let px = st::stretch(Round::Down, k, &d, (40, 40), (32, 32), false, |_, _| Ok(Some(c))).unwrap();
        if let Some(px) = px {
            prop_assert!(!px.is_empty());
            let mid = |v: i64, n: i64| 3 * (v - 1) >= n && 3 * (v - 1) < 2 * n;
            for (x, y, p) in px {
                if mid(x, dw) && mid(y, dh) {
                    prop_assert_eq!(p, c, "pixel {} {}", x, y);
                }
            }
        } else {
            prop_assert!(half && (sw > dw || sh > dh));
        }
    }
    /// the draw writes the destination rectangle's pixels, each once, and nothing outside
    #[test]
    fn writes_its_rectangle(sw in 1i64..12, sh in 1i64..12, dw in 1i64..30, dh in 1i64..30, cubic: bool, half: bool, x86: bool) {
        let k = if cubic { Kernel::Cubic } else { Kernel::Tent };
        let d = Draw { dst: (1, 1, dw, dh), src: (2, 2, sw, sh), half };
        let r = if x86 { Round::Nearest } else { Round::Down };
        if let Some(px) = st::stretch(r, k, &d, (16, 16), (32, 32), x86, |x, y| Ok(Some((x * 16 + y) as u32 | 0xff00_0000))).unwrap() {
            let mut seen = std::collections::HashSet::new();
            for (x, y, _) in px {
                prop_assert!((1..1 + dw).contains(&x) && (1..1 + dh).contains(&y), "pixel {} {}", x, y);
                prop_assert!(seen.insert((x, y)));
            }
            prop_assert_eq!(seen.len() as i64, dw * dh);
        }
    }
}
