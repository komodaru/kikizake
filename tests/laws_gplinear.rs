//! Laws of the GDI+ linear gradient model (src/model/gplinear.rs).
use kikizake::model::gdiplus::Round;
use kikizake::model::gplinear::{self as lin, Refused, Span};
use proptest::prelude::*;

fn round(down: bool) -> Round {
    if down { Round::Down } else { Round::Nearest }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]
    /// a gradient between one opaque color and itself is that color everywhere
    #[test]
    fn one_color_is_solid(p1 in (-64i32..64, -64i32..64), p2 in (-64i32..64, -64i32..64), c in any::<u32>(), wrap in 0i64..4, x in -100i64..100, y in -100i64..100, half: bool, down: bool) {
        prop_assume!(p1 != p2);
        let c = c | 0xff00_0000;
        let l = lin::new(round(down), p1, p2, (c, c), wrap).unwrap();
        if let Some(s) = Span::new(round(down), &l, half) {
            prop_assert_eq!(s.at(x, y), c);
        }
    }
    /// InferAffineMatrix takes the rectangle's corners to the points (to float precision)
    #[test]
    fn infer_takes_corners_to_points(rect in (-50f32..50.0, -50f32..50.0, 1f32..60.0, 1f32..60.0), p in proptest::array::uniform3((-80f32..80.0, -80f32..80.0)), down: bool) {
        let r = round(down);
        let m = lin::infer(r, p, rect).unwrap();
        let (x, y, w, h) = rect;
        for (i, (cx, cy)) in [(x, y), (x + w, y), (x, y + h)].into_iter().enumerate() {
            let (tx, ty) = (cx * m[0] + cy * m[2] + m[4], cx * m[1] + cy * m[3] + m[5]);
            prop_assert!((tx - p[i].0).abs() < 2e-2 && (ty - p[i].1).abs() < 2e-2, "{} {} {:?}", tx, ty, p[i]);
        }
    }
    /// the rectangle does not depend on which point comes first
    #[test]
    fn rect_is_symmetric(p1 in (-64i32..64, -64i32..64), p2 in (-64i32..64, -64i32..64), down: bool) {
        prop_assume!(p1 != p2);
        let a = lin::new(round(down), p1, p2, (0, 0), 0).unwrap();
        let b = lin::new(round(down), p2, p1, (0, 0), 0).unwrap();
        prop_assert_eq!(a.rect.0.min(b.rect.0), a.rect.0);
        prop_assert_eq!(a.rect.2, b.rect.2);
    }
}

#[test]
fn statuses() {
    assert_eq!(lin::new(Round::Nearest, (0, 0), (3, 4), (0, 0), 4), Err(Refused::InvalidParameter));
    assert_eq!(lin::new(Round::Nearest, (2, 2), (2, 2), (0, 0), 0), Err(Refused::OutOfMemory));
    assert!(lin::new(Round::Nearest, (0, 0), (3, 4), (0, 0), 5).is_ok());
}
