//! Laws of GDI+ DrawImage's chain and its nearest, bilinear and bicubic spans
//! (src/model/gpimage.rs): the tables, the pixels a draw writes (flips included), uniform
//! sources staying uniform, integer enlargements repeating pixels.
use kikizake::model::gdiplus::Round;
use kikizake::model::gpimage::{cubic64, cubic86, render, Draw, Span};
use kikizake::model::gpstretch::Kernel;
use proptest::prelude::*;

/// Keys' cubic a = -1/2: each fraction's four weights sum to one (x64 exactly, x86's stored
/// table within one), the x86 table's entries as the notes give them
#[test]
fn cubic_weights_sum_to_one() {
    for fr in 0..64 {
        let idx = [64 + fr, fr, 64 - fr, 128 - fr];
        let s: i64 = idx.iter().map(|&i| cubic64(i)).sum();
        assert!((65535..=65537).contains(&s), "fr {fr}: {s}");
        let s: i64 = idx.iter().map(|&i| cubic86(i)).sum();
        assert!((16383..=16385).contains(&s), "fr {fr}: {s}");
    }
    assert_eq!((cubic64(0), cubic64(64), cubic64(128)), (65536, 0, 0));
    assert_eq!((cubic86(0), cubic86(15), cubic86(79)), (16384, 14451, -1126));
}

fn span(k: u8) -> Span {
    match k % 5 {
        0 => Span::Nearest,
        1 => Span::Bilinear,
        2 => Span::Bicubic,
        3 => Span::Stretch(Kernel::Tent),
        _ => Span::Stretch(Kernel::Cubic),
    }
}
fn round(x86: bool) -> Round {
    if x86 { Round::Nearest } else { Round::Down }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    /// a draw inside the target writes each pixel of the destination rectangle once and nothing
    /// else, whichever way the rectangles are flipped
    #[test]
    fn writes_its_rectangle(sw in 2i64..10, sh in 2i64..10, dw in 1i64..24, dh in 1i64..24, flips in 0u8..16, k: u8, half: bool, x86: bool) {
        let sign = |b: u8| if flips >> b & 1 == 1 { -1 } else { 1 };
        let (dw, dh, sw, sh) = (sign(0) * dw, sign(1) * dh, sign(2) * sw, sign(3) * sh);
        let (dx, dy) = (if dw < 0 { 30 } else { 4 }, if dh < 0 { 30 } else { 4 });
        let (sx, sy) = (if sw < 0 { 12 } else { 2 }, if sh < 0 { 12 } else { 2 });
        let d = Draw { dst: [dx, dy, dw, dh], src: [sx, sy, sw, sh], image: (16, 16), target: (40, 40), half };
        let px = render(round(x86), span(k), &d, x86, |x, y| Ok((x * 16 + y) as u32 | 0xff00_0000));
        prop_assert!(px.is_ok(), "{:?}", px);
        if let Ok(px) = px {
            let (x0, y0) = (dx.min(dx + dw), dy.min(dy + dh));
            let mut seen = std::collections::HashSet::new();
            for (x, y, _) in px {
                prop_assert!((x0..x0 + dw.abs()).contains(&x) && (y0..y0 + dh.abs()).contains(&y), "pixel {} {}", x, y);
                prop_assert!(seen.insert((x, y)));
            }
            prop_assert_eq!(seen.len() as i64, dw.abs() * dh.abs());
        }
    }
    /// an opaque uniform source well inside the image comes back uniform in the middle third
    /// of the rectangle, every span but x86's bicubic stretch, to a 2x reduction
    #[test]
    fn uniform_stays_uniform(c in any::<u32>(), sw in 6i64..12, sh in 6i64..12, dw in 3i64..30, dh in 3i64..30, k: u8, half: bool, x86: bool) {
        prop_assume!(2 * dw >= sw && 2 * dh >= sh);
        let c = c | 0xff00_0000;
        let d = Draw { dst: [1, 1, dw, dh], src: [10, 10, sw, sh], image: (40, 40), target: (32, 32), half };
        let px = render(round(x86), span(k), &d, x86, |_, _| Ok(c));
        prop_assert!(px.is_ok());
        if let Ok(px) = px {
            let mid = |v: i64, n: i64| 3 * (v - 1) >= n && 3 * (v - 1) < 2 * n;
            for (x, y, p) in px {
                if mid(x, dw) && mid(y, dh) {
                    prop_assert_eq!(p, c, "pixel {} {}", x, y);
                }
            }
        }
    }
    /// NearestNeighbor enlarging by whole numbers under a half pixel offset repeats each source
    /// pixel n times on both arches (a corner offset rounds k s / d half up instead, so the last
    /// of each run reads the next pixel)
    #[test]
    fn nearest_repeats(sw in 1i64..12, sh in 1i64..12, nx in 2i64..5, ny in 2i64..5, x86: bool) {
        let d = Draw { dst: [1, 2, nx * sw, ny * sh], src: [3, 1, sw, sh], image: (16, 16), target: (64, 64), half: true };
        let px = render(round(x86), Span::Nearest, &d, x86, |x, y| Ok((x * 16 + y) as u32 | 0xff00_0000));
        prop_assert!(px.is_ok());
        if let Ok(px) = px {
            for (x, y, p) in px {
                let want = (3 + (x - 1) / nx) * 16 + 1 + (y - 2) / ny;
                prop_assert_eq!(p, want as u32 | 0xff00_0000, "pixel {} {}", x, y);
            }
        }
    }
    /// a scale of +-1 is a copy, mirrored on each flipped axis, under every mode and pixel
    /// offset, on x86 (the rotate / flip branch and the copy span, image.md 13; x64's
    /// InferAffineMatrix, rounding down, can leave a flipped scale 2 ulps short of 1, which takes
    /// the ordinary spans, 13b)
    #[test]
    fn a_unit_scale_copies(sw in 1i64..10, sh in 1i64..10, flips in 0u8..16, k: u8, po in 0u8..5) {
        let x86 = true;
        let sign = |b: u8| if flips >> b & 1 == 1 { -1 } else { 1 };
        let (dw, dh, sw, sh) = (sign(0) * sw, sign(1) * sh, sign(2) * sw, sign(3) * sh);
        let (dx, dy) = (if dw < 0 { 20 } else { 4 }, if dh < 0 { 20 } else { 4 });
        let (sx, sy) = (if sw < 0 { 12 } else { 2 }, if sh < 0 { 12 } else { 2 });
        let d = Draw { dst: [dx, dy, dw, dh], src: [sx, sy, sw, sh], image: (16, 16), target: (32, 32), half: po == 2 || po == 4 };
        let px = render(round(x86), span(k), &d, x86, |x, y| Ok((x * 16 + y) as u32 | 0xff00_0000));
        prop_assert!(px.is_ok(), "{:?}", px);
        if let Ok(px) = px {
            let (x0, y0, ux, uy) = (dx.min(dx + dw), dy.min(dy + dh), sx.min(sx + sw), sy.min(sy + sh));
            let (fx, fy) = ((dw < 0) != (sw < 0), (dh < 0) != (sh < 0));
            prop_assert_eq!(px.len() as i64, dw.abs() * dh.abs());
            for (x, y, p) in px {
                let (i, j) = (x - x0, y - y0);
                let u = ux + if fx { sw.abs() - 1 - i } else { i };
                let v = uy + if fy { sh.abs() - 1 - j } else { j };
                prop_assert_eq!(p, (u * 16 + v) as u32 | 0xff00_0000, "pixel {} {}", x, y);
            }
        }
    }
}
