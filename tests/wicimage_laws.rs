//! Laws of the WIC bitmap source model (src/model/wicimage.rs): what the documentation states as
//! properties, exhaustive over small domains or property-based, whatever Windows' measured
//! details turn out to be.
use proptest::prelude::*;
use kikizake::model::wicimage::{self as wi, Img};

const FMTS: &[&str] = &["bw", "gray2", "gray4", "gray8", "gray16", "bgr565", "bgr24", "bgra", "rgb48", "rgba64", "cmyk", "idx1", "idx4", "idx8"];

fn img(f: &str, w: u32, h: u32, seed: u32) -> Img {
    wi::bitmap(f, w, h, Some(seed)).unwrap()
}

/// the pixels as values, so that bits past a row's end do not count
fn pixels(i: &Img) -> Vec<Vec<u8>> {
    (0..i.h).flat_map(|y| (0..i.w).map(move |x| (x, y))).map(|(x, y)| i.get(x, y)).collect()
}

const OPTS: [u32; 8] = [0, 1, 2, 3, 8, 9, 10, 11];

/// exhaustive over every transform, format and size up to 4x4: the documented group of the
/// square's symmetries (rotations compose by adding, a flip twice is nothing, a rotation by 180
/// is both flips, FlipVertical is FlipHorizontal after a rotation by 180)
#[test]
fn flips_are_the_square_symmetries() {
    for f in FMTS {
        for w in 1..=4 {
            for h in 1..=4 {
                let s = img(f, w, h, w * 7 + h);
                let p = pixels(&s);
                assert_eq!(pixels(&wi::flip(&s, 0)), p);
                for o in OPTS {
                    let once = wi::flip(&s, o);
                    let (ew, eh) = if o & 1 != 0 { (h, w) } else { (w, h) };
                    assert_eq!((once.w, once.h), (ew, eh));
                    assert_eq!(once.fmt, s.fmt);
                }
                let r1 = wi::flip(&s, 1);
                assert_eq!(pixels(&wi::flip(&wi::flip(&wi::flip(&r1, 1), 1), 1)), p, "{f} {w}x{h}: four quarter turns");
                assert_eq!(pixels(&wi::flip(&r1, 1)), pixels(&wi::flip(&s, 2)), "{f}: two quarter turns are a half turn");
                assert_eq!(pixels(&wi::flip(&wi::flip(&s, 8), 8)), p);
                assert_eq!(pixels(&wi::flip(&wi::flip(&s, 16), 16)), p);
                assert_eq!(pixels(&wi::flip(&s, 2)), pixels(&wi::flip(&s, 24)), "{f}: a half turn is both flips");
                assert_eq!(pixels(&wi::flip(&s, 16)), pixels(&wi::flip(&wi::flip(&s, 2), 8)));
                // a clockwise quarter turn: the top left pixel goes to the top right
                assert_eq!(r1.get(r1.w - 1, 0), s.get(0, 0));
                assert_eq!(wi::flip(&s, 3).get(0, s.w - 1), s.get(0, 0));
                assert_eq!(wi::flip(&s, 8).get(s.w - 1, 0), s.get(0, 0));
            }
        }
    }
}

/// nearest neighbor: the same size changes nothing, an integer upscale repeats each pixel, and
/// every output pixel is one of the source's (exhaustive over sizes 1..=8 -> 1..=17)
#[test]
fn nearest_neighbor_picks_pixels() {
    for sw in 1..=8u32 {
        for dw in 1..=17u32 {
            for d in 0..dw {
                let s = wi::nearest(d, sw, dw);
                assert!(s < sw);
                assert!(d + 1 < dw || s == sw - 1);
                // (measured) the source pixel whose span holds the output pixel's right edge, just inside
                let e = ((d + 1) * sw - 1) as u64;
                assert!(s as u64 * dw as u64 <= e && e < (s as u64 + 1) * dw as u64);
                // monotonic, and the last output pixel takes the last source pixel
                assert!(d == 0 || wi::nearest(d - 1, sw, dw) <= s);
            }
            if dw % sw == 0 {
                for d in 0..dw {
                    assert_eq!(wi::nearest(d, sw, dw), d / (dw / sw));
                }
            }
        }
        for d in 0..sw {
            assert_eq!(wi::nearest(d, sw, sw), d);
        }
    }
    // (the formats the scaler answers in as they are)
    for f in FMTS.iter().filter(|f| wi::scale_formats(f, "nn") == (**f, **f)) {
        let s = img(f, 5, 3, 9);
        assert_eq!(pixels(&wi::scale(&s, 5, 3, "nn").unwrap()), pixels(&s));
    }
}

/// predefined palettes: the documented counts, a transparent color one more entry (the last
/// one at 256), the gray palettes gray and evenly spaced from black to white
#[test]
fn predefined_palettes() {
    for (t, n) in [(2u32, 2usize), (10, 4), (11, 16), (12, 256)] {
        let (hr, ty, c) = wi::predefined(t, false).unwrap();
        assert_eq!((hr, ty, c.len()), (0, t, n));
        assert_eq!(c[0], 0xff00_0000);
        assert_eq!(*c.last().unwrap(), 0xffff_ffff);
        assert!(c.windows(2).all(|p| p[0] < p[1]));
        // (black and white and gray: a transparent color is not added)
        assert_eq!(wi::predefined(t, true).unwrap(), (0, t, c));
    }
    // the halftones: the documented cube of each primary's levels, black first, white in it,
    // every color opaque and different; a transparent color one more entry (the last at 256)
    for (t, n) in [(3u32, 16usize), (4, 28), (5, 72), (6, 126), (7, 224), (8, 252), (9, 256)] {
        let (hr, ty, c) = wi::predefined(t, false).unwrap();
        assert_eq!((hr, ty, c.len()), (0, t, n));
        assert_eq!(c[0], 0xff00_0000);
        assert!(c.contains(&0xffff_ffff));
        let mut u = c.clone();
        u.sort();
        u.dedup();
        assert_eq!(u.len(), n);
        let (_, ty, ct) = wi::predefined(t, true).unwrap();
        assert_eq!(ty, t);
        assert_eq!(ct.len(), if n == 256 { 256 } else { n + 1 });
        assert_eq!(*ct.last().unwrap(), 0);
        assert_eq!(ct[..n.min(255)], c[..n.min(255)]);
    }
    for t in [0u32, 1, 13, 14, 0x7fff_ffff] {
        assert_ne!(wi::predefined(t, false).unwrap().0, 0);
    }
}

fn rows_of(i: &Img) -> Vec<Vec<u8>> {
    let st = i.stride();
    i.px.chunks(st).map(<[u8]>::to_vec).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// a clipper and CopyPixels of a rectangle show the same pixels, and the whole picture
    /// clipped is the picture
    #[test]
    fn clip_and_copy_agree(fi in 0..FMTS.len(), w in 1u32..12, h in 1u32..8, seed: u32, a: u32, b: u32, c: u32, d: u32) {
        let s = img(FMTS[fi], w, h, seed | 1);
        let (x, y) = (a % w, b % h);
        let (cw, ch) = (1 + c % (w - x), 1 + d % (h - y));
        let cl = wi::clip(&s, x, y, cw, ch);
        let (hr, rows) = wi::copy(&s, Some((x as i64, y as i64, cw as i64, ch as i64)), None);
        prop_assert_eq!(hr, 0);
        let (hr2, rows2) = wi::copy(&cl, None, None);
        prop_assert_eq!(hr2, 0);
        prop_assert_eq!(rows, rows2);
        prop_assert_eq!(pixels(&wi::clip(&s, 0, 0, w, h)), pixels(&s));
        // (a rectangle reaching outside is refused, whatever the format)
        prop_assert_ne!(wi::copy(&s, Some((x as i64, y as i64, (w - x + 1) as i64, ch as i64)), None).0, 0);
    }

    /// a buffer is enough exactly when it holds stride * (height - 1) + one row
    #[test]
    fn copy_buffer_bounds(fi in 0..FMTS.len(), w in 1u32..9, h in 1u32..6, extra in 0i64..9, short in 0i64..2) {
        let s = img(FMTS[fi], w, h, 5);
        let row = wi::stride(FMTS[fi], w) as i64;
        let st = row + extra;
        let need = st * (h as i64 - 1) + row;
        prop_assert_eq!(wi::copy(&s, None, Some((st, need - short))).0 == 0, short == 0);
        prop_assert_ne!(wi::copy(&s, None, Some((row - 1, 1 << 20))).0, 0);
    }

    /// files written by the generators (from the formats' specifications) decode to what was
    /// written: BMP (both row orders), PNG (every filter, Adam7), GIF (LZW, interlace)
    #[test]
    fn bmp_round_trip(w in 1i32..20, h in 1i32..9, bpp_i in 0usize..3, top: bool, seed: u32) {
        let bpp = [8u32, 24, 32][bpp_i];
        let st = (w as u32 * bpp).div_ceil(32) * 4;
        let mut r = kikizake::rng::XorShift32::new(seed);
        let px: Vec<u8> = (0..st * h as u32).map(|_| r.next() as u8).collect();
        let pal: Vec<u32> = if bpp == 8 { (0..256).map(|i| i * 0x010101).collect() } else { vec![] };
        let f = kikizake::generators::wicimage_bmp(40, w, h, bpp, 0, [0; 4], 0, &pal, &px, top, (0, 0));
        let d = wi::decode(&f).unwrap().unwrap();
        let i = d.frames[0].clone().unwrap().pixels().unwrap();
        prop_assert_eq!((i.w, i.h), (w as u32, h as u32));
        let n = i.stride();
        for y in 0..h as usize {
            let sy = if top { y } else { h as usize - 1 - y };
            prop_assert_eq!(&rows_of(&i)[y][..], &px[sy * st as usize..sy * st as usize + n]);
        }
    }

    #[test]
    fn png_round_trip(w in 1u32..14, h in 1u32..10, il: bool, f0 in 0u8..5, f1 in 0u8..5, seed: u32, block in 1usize..90) {
        // 8-bit gray: a sample is a pixel
        let mut r = kikizake::rng::XorShift32::new(seed);
        let pix: Vec<Vec<u8>> = (0..h).map(|_| (0..w).map(|_| r.next() as u8).collect()).collect();
        let mut filtered = Vec::new();
        let passes: Vec<(u32, u32, u32, u32)> = if il { kikizake::generators::WICIMAGE_ADAM7.to_vec() } else { vec![(0, 0, 1, 1)] };
        for (x0, y0, dx, dy) in passes {
            let raw: Vec<Vec<u8>> = (y0..h).step_by(dy as usize).map(|y| (x0..w).step_by(dx as usize).map(|x| pix[y as usize][x as usize]).collect()).filter(|r: &Vec<u8>| !r.is_empty()).collect();
            if raw.is_empty() {
                continue;
            }
            filtered.extend(kikizake::generators::wicimage_png_filter(&raw, 1, &[f0, f1]));
        }
        let f = kikizake::generators::wicimage_png(w, h, 8, 0, il, &[], &filtered, block);
        let i = wi::decode(&f).unwrap().unwrap().frames[0].clone().unwrap().pixels().unwrap();
        prop_assert_eq!(i.fmt, "gray8");
        prop_assert_eq!(rows_of(&i), pix);
    }

    #[test]
    fn gif_round_trip(w in 1u32..40, h in 1u32..20, bits in 1u32..9, il: bool, seed: u32) {
        let mut r = kikizake::rng::XorShift32::new(seed);
        let idx: Vec<u8> = (0..w * h).map(|_| (r.next() % (1 << bits)) as u8).collect();
        let pal: Vec<u32> = (0..1u32 << bits).collect();
        let fr = kikizake::generators::WicGifFrame { x: 0, y: 0, w, h, local: None, transparent: None, interlace: il, idx: idx.clone(), min: bits.max(2) };
        let f = kikizake::generators::wicimage_gif(w, h, Some(&pal), 0, &[fr]);
        let i = wi::decode(&f).unwrap().unwrap().frames[0].clone().unwrap().pixels().unwrap();
        prop_assert_eq!(i.px, idx);
    }

    /// a long run of one index and noise both survive LZW, past the 12-bit table and its clear
    #[test]
    fn lzw_round_trip(n in 1usize..9000, bits in 1u32..9, seed: u32, runs: bool) {
        let mut r = kikizake::rng::XorShift32::new(seed);
        let idx: Vec<u8> = (0..n).map(|i| if runs { ((i / 37) as u32 % (1 << bits)) as u8 } else { (r.next() % (1 << bits)) as u8 }).collect();
        let pal: Vec<u32> = (0..1u32 << bits).collect();
        let fr = kikizake::generators::WicGifFrame { x: 0, y: 0, w: n as u32, h: 1, local: None, transparent: None, interlace: false, idx: idx.clone(), min: bits.max(2) };
        let f = kikizake::generators::wicimage_gif(n as u32, 1, Some(&pal), 0, &[fr]);
        let i = wi::decode(&f).unwrap().unwrap().frames[0].clone().unwrap().pixels().unwrap();
        prop_assert_eq!(i.px, idx);
    }

    /// BC blocks: the endpoints decode to themselves, BC1's index 3 of a 3-color block is
    /// transparent, BC2's alpha is a multiple of 17
    #[test]
    fn bc_blocks(blk in prop::collection::vec(any::<u8>(), 16)) {
        for kind in 1..=3u8 {
            let b = if kind == 1 { &blk[..8] } else { &blk[..] };
            let col = if kind == 1 { b } else { &b[8..] };
            let c0 = u16::from_le_bytes([col[0], col[1]]) as u32;
            let c1 = u16::from_le_bytes([col[2], col[3]]) as u32;
            let idx = u32::from_le_bytes([col[4], col[5], col[6], col[7]]);
            let px = wi::bc_block(kind, b).unwrap();
            for (i, p) in px.iter().enumerate() {
                let k = idx >> (2 * i) & 3;
                let e = if k == 0 { Some(c0) } else if k == 1 { Some(c1) } else { None };
                if let Some(c) = e {
                    prop_assert_eq!(p[2] >> 3, (c >> 11) as u8);
                    prop_assert_eq!(p[1] >> 2, (c >> 5 & 63) as u8);
                    prop_assert_eq!(p[0] >> 3, (c & 31) as u8);
                }
                if kind == 1 && c0 <= c1 && k == 3 {
                    prop_assert_eq!(p[3], 0);
                }
                if kind == 2 {
                    prop_assert_eq!(p[3] % 17, 0);
                }
            }
        }
    }
}

/// exhaustive over every size 1..=24 -> 1..=40: each filter's weights cover each output pixel
/// once (a flat picture stays flat), only source pixels, and the same size is the identity for
/// Linear, Fant and HighQualityCubic (whose centers fall on the source's)
#[test]
fn filter_weights_are_partitions_of_one() {
    for n in 1..=24u32 {
        for d in 1..=40u32 {
            for m in ["linear", "cubic", "fant"] {
                let t = wi::taps(m, n, d);
                assert_eq!(t.taps.len(), d as usize);
                for (i, ws) in t.taps.iter().enumerate() {
                    assert_eq!(ws.iter().map(|w| w.1).sum::<i64>(), t.den, "{m} {n}->{d} at {i}");
                    assert!(ws.iter().all(|w| w.0 < n as usize));
                    // (Linear and Fant only blend: no weight below 0)
                    if m != "cubic" {
                        assert!(ws.iter().all(|w| w.1 >= 0));
                    }
                }
                if n == d && m != "cubic" {
                    for (i, ws) in t.taps.iter().enumerate() {
                        assert!(ws.iter().all(|w| w.1 == 0 || w.0 == i), "{m} {n} at {i}: {ws:?}");
                    }
                }
            }
            let h = wi::hq_taps(n, d);
            for ws in &h {
                assert!((ws.iter().map(|w| w.1).sum::<f64>() - 1.0).abs() < 1e-9);
                assert!(ws.iter().all(|w| w.0 < n as usize));
            }
        }
    }
}

/// Fant's averages over the output pixels' spans together cover the whole row once: the weights
/// of all output pixels sum to d whole pixels, exactly
#[test]
fn fant_spends_every_source_span() {
    for n in 1..=24u32 {
        for d in 1..=40u32 {
            let t = wi::taps("fant", n, d);
            // (all output pixels together are the whole row: the weights sum to d whole pixels)
            let all: i64 = t.taps.iter().flatten().map(|w| w.1).sum();
            assert_eq!(all, d as i64 * t.den);
        }
    }
}

proptest! {
    /// a filter of only nonnegative weights (Linear, Fant) keeps every channel between the
    /// source's least and greatest; every filter keeps a flat picture flat, but Cubic down,
    /// whose four floored products may lose up to a level (MEASURED: Windows' flat columns come
    /// out one lower on x86), never more and never higher
    #[test]
    fn filters_stay_in_range(w in 1u32..7, h in 1u32..7, dw in 1u32..12, dh in 1u32..12, seed in 0u32..1000, flat in 0u8..=255) {
        for m in ["linear", "fant", "cubic", "hqcubic"] {
            let s = img("gray8", w, h, seed);
            let o = wi::scale(&s, dw, dh, m);
            let Ok(o) = o else { continue };
            let lo = *s.px.iter().min().unwrap();
            let hi = *s.px.iter().max().unwrap();
            if m == "linear" || m == "fant" {
                prop_assert!(o.px.iter().all(|&v| lo <= v && v <= hi), "{} {:?}", m, o.px);
            }
            let mut f = s.clone();
            f.px.fill(flat);
            if let Ok(o) = wi::scale(&f, dw, dh, m) {
                let low = if m == "cubic" && dh != h { flat.saturating_sub(1) } else { flat };
                prop_assert!(o.px.iter().all(|&v| low <= v && v <= flat), "{} flat {} {:?}", m, flat, o.px);
            }
        }
    }
}

/// (found by fuzzing) an output compared with itself has no differences, even when a case name
/// repeats after an unmodeled case of that name
#[test]
fn repeated_case_names_compare_with_themselves() {
    let out = "case a\nunmodeled WIC.SCALE\ncase b\nunmodeled WIC.SCALE\ncase a\nvalue\ndone\n";
    let r = kikizake::compare::compare_text(out, out);
    assert!(r.differ.is_empty(), "{:?}", r.differ);
    assert_eq!((r.same, r.unmodeled), (1, 2));
}

/// Every generated case the model leaves unmodeled is listed in tests/wicimage_unmodeled.txt,
/// and nothing else: a change that stops answering a case (a mutant that falls back to
/// Unmodeled) fails here. (Over the cases, not Windows' answers, so goldens arriving do not
/// change it.) KIKIZAKE_BLESS=1 rewrites the list.
#[test]
fn unmodeled_cases_are_the_listed_ones() {
    use kikizake::{case, generators, model};
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut now = Vec::new();
    for s in ["wicpalette", "wicscale", "wictransform", "wiccolor", "wicdecode", "wicencode"] {
        model::set_arch(32);
        let out = model::run(s, &case::parse_cases(&generators::suite(s).unwrap()));
        let mut name = "";
        for l in out.lines() {
            if let Some(n) = l.strip_prefix("case ") {
                name = n;
            } else if l.starts_with("unmodeled") {
                now.push(format!("{s} {name}"));
            }
        }
    }
    now.sort();
    now.dedup();
    let path = root.join("tests/wicimage_unmodeled.txt");
    if std::env::var("KIKIZAKE_BLESS").is_ok() {
        std::fs::write(&path, now.join("\n") + "\n").unwrap();
        return;
    }
    let listed: Vec<String> = std::fs::read_to_string(&path).unwrap_or_default().lines().map(String::from).collect();
    let new: Vec<&String> = now.iter().filter(|n| !listed.contains(n)).collect();
    let gone: Vec<&String> = listed.iter().filter(|n| !now.contains(n)).collect();
    assert!(new.is_empty() && gone.is_empty(), "unmodeled cases changed (KIKIZAKE_BLESS=1 after settling some):\nnewly unmodeled: {new:?}\nnow modeled: {gone:?}");
}

proptest! {
    /// (found by fuzzing: a depth of 0 read samples of no bytes) any IHDR, any image data: the
    /// PNG decoder answers or says it has no model, it does not panic, and an IHDR the format
    /// does not allow is no PNG to it
    #[test]
    fn png_any_ihdr_never_panics(w in 1u32..6, h in 1u32..4, depth in any::<u8>(), ct in 0u8..9, il in any::<bool>(), data in proptest::collection::vec(any::<u8>(), 0..200)) {
        let f = kikizake::generators::wicimage_png(w, h, depth, ct, il, &[(b"PLTE", vec![7; 768])], &data, 50);
        let r = wi::decode(&f);
        let allowed = matches!((ct, depth), (0, 1 | 2 | 4 | 8 | 16) | (3, 1 | 2 | 4 | 8) | (2 | 4 | 6, 8 | 16));
        // (Windows knows no decoder for such a file: 0x88982f07)
        if !allowed {
            prop_assert!(matches!(r, Ok(Err(0x8898_2f07))), "{ct} {depth}");
        }
    }
}

proptest! {
    /// BlackWhite's error diffusion: a gray of only black and white has no error to spread and
    /// comes out as it is; any gray comes out with as many white pixels as its levels make,
    /// within what the edges lose (the errors carried off the right, left and bottom edges)
    #[test]
    fn bw_dither_keeps_black_white_and_the_mean(w in 1u32..24, h in 1u32..24, seed in any::<u32>(), bw in any::<bool>()) {
        let mut g = Img::new(w, h, "gray8");
        let mut x = seed | 1;
        for y in 0..h as usize {
            for i in 0..w as usize {
                x ^= x << 13; x ^= x >> 17; x ^= x << 5;
                let v = if bw { if x & 1 == 0 { 0 } else { 255 } } else { (x >> 8) as u8 };
                let st = g.stride();
                g.px[y * st + i] = v;
            }
        }
        let o = wi::dither_bw(&g);
        let bit = |y: usize, i: usize| o.px[y * o.stride() + i / 8] >> (7 - i % 8) & 1;
        let mut whites = 0i64;
        let mut sum = 0i64;
        for y in 0..h as usize {
            for i in 0..w as usize {
                let v = g.px[y * g.stride() + i];
                if bw {
                    prop_assert_eq!(bit(y, i), (v == 255) as u8);
                }
                whites += bit(y, i) as i64;
                sum += v as i64;
            }
        }
        // (each pixel's error is below 256; what leaves the picture is at most its edge pixels'
        // errors, so the levels written and asked differ by less than 256 per edge pixel)
        let edge = 2 * (w as i64 + h as i64);
        prop_assert!((whites * 255 - sum).abs() <= 256 * edge, "{whites} {sum}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// the GIF encoder's file decodes to its pixels and palette, through runs and noise long
    /// enough to fill the 12-bit table and clear it
    #[test]
    fn gif_encoder_round_trips(w in 1u32..120, h in 1u32..80, colors in 1usize..=256, seed: u32, runs: bool) {
        let mut r = kikizake::rng::XorShift32::new(seed);
        let mut i = Img::new(w, h, "idx8");
        i.pal = Some((0..colors as u32).map(|k| 0xff00_0000 | k.wrapping_mul(0x01_0f_3b)).collect());
        for k in 0..(w * h) as usize {
            i.px[k] = if runs { (k / 53 % colors) as u8 } else { (r.next() as usize % colors) as u8 };
        }
        let f = wi::encode_gif(&i).unwrap();
        let d = wi::decode(&f).unwrap().unwrap().frames[0].clone().unwrap().pixels().unwrap();
        prop_assert_eq!(&d.px, &i.px);
        let bits = (2..=8u32).find(|&b| 1usize << b >= colors).unwrap();
        prop_assert_eq!(f[13 + 9], 0x80 | (bits - 1) as u8);
        let pal = d.pal.unwrap();
        prop_assert_eq!(&pal[..colors], &i.pal.unwrap()[..]);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// colors at their cells' centers that are all in the palette spend no error and keep their
    /// own entries; any color's pixels stay inside the palette, and transparent ones get none
    #[test]
    fn dither_keeps_cell_centers(w in 1usize..20, h in 1usize..20, n in 1usize..40, seed: u32, extra in proptest::collection::vec(any::<[u8; 3]>(), 0..20)) {
        let mut r = kikizake::rng::XorShift32::new(seed);
        let mut pal: Vec<[i32; 3]> = vec![];
        while pal.len() < n {
            let c = [0; 3].map(|_: i32| 17 * (r.next() % 16) as i32);
            if !pal.contains(&c) {
                pal.push(c);
            }
        }
        let px: Vec<usize> = (0..w * h).map(|_| r.next() as usize % n).collect();
        let at = |x: usize, y: usize| pal[px[y * w + x]];
        let ix = wi::dither(w, h, &at, &pal, &|_, _| false);
        prop_assert_eq!(ix, px.iter().map(|&i| Some(i)).collect::<Vec<_>>());
        let any = |x: usize, y: usize| { let c = extra.get((x + y * w) % extra.len().max(1)).copied().unwrap_or([7, 77, 177]); c.map(i32::from) };
        let clear = |x: usize, y: usize| (x * 3 + y) % 5 == 0;
        let ix = wi::dither(w, h, &any, &pal, &clear);
        let opaque = wi::dither(w, h, &any, &pal, &|_, _| false);
        for (k, i) in ix.iter().enumerate() {
            prop_assert_eq!(i.is_none(), clear(k % w, k / w));
            prop_assert!(i.is_none_or(|i| i < n));
            // (a transparent pixel spends its error as an opaque one would)
            prop_assert!(i.is_none() || *i == opaque[k]);
        }
    }
}

/// an option the encoder takes for lossless changes nothing the decoder gives back; every option
/// line parses into a measured effect or an open clause, never a bad line
#[test]
fn encoder_options_are_typed() {
    use wi::EncodeOption as O;
    for (c, o, want) in [("png", "FilterOption=3", O::Lossless), ("tiff", "TiffCompressionMethod=2", O::BlackWhite), ("tiff", "TiffCompressionMethod=3", O::BlackWhite), ("tiff", "TiffCompressionMethod=4", O::Lossless), ("tiff", "TiffCompressionMethod=5", O::Lossless), ("tiff", "TiffCompressionMethod=8", O::OutOfRange), ("bmp", "EnableV5Header32bppBGRA=1", O::V5Header(true)), ("png", "ImageQuality=0.5", O::Rejected), ("wmp", "Lossless=1", O::WmpLossless), ("wmp", "ImageQuality=1.0", O::WmpLossless), ("wmp", "ImageQuality=0.5", O::WmpQuality(0.5f32.to_bits())), ("jpeg", "ImageQuality=0.5", O::JpegQuality(50)), ("jpeg", "SuppressApp0=1", O::JpegNoApp0(true)), ("jpeg", "JpegYCrCbSubsampling=3", O::JpegChroma(kikizake::model::jpeg::Chroma::Full)), ("jpeg", "JpegYCrCbSubsampling=2", O::JpegChroma(kikizake::model::jpeg::Chroma::H2V1)), ("jpeg", "BitmapTransform=1", O::JpegTransform(1)), ("jpeg", "JpegYCrCbSubsampling=5", O::InvalidArg), ("jpeg", "JpegYCrCbSubsampling=4", O::JpegChroma(kikizake::model::jpeg::Chroma::H1V2)), ("png", "FilterOption=0", O::Lossless), ("png", "FilterOption=7", O::OutOfRange), ("gif", "InterlaceOption=1", O::Rejected), ("jpeg", "SuppressApp0=2", O::JpegNoApp0(true))] {
        assert_eq!(wi::option(c, o).ok(), Some(want), "{c} {o}");
    }
    for (c, o) in [("wmp", "ImageQuality=1.5"), ("bmp", "Overlap=1"), ("dds", "Quality=1")] {
        assert!(matches!(wi::option(c, o), Err(kikizake::model::Fail::Unmodeled(_))), "{c} {o}");
    }
    assert!(matches!(wi::option("png", "FilterOption"), Err(kikizake::model::Fail::Bad)));
}

/// the measured linear table rises with the level, from 0 to 1, and the float formats agree
/// with each other: gray is the channels' weighted sum, premultiplied is the straight times alpha
#[test]
fn linear16_is_a_curve_and_the_floats_agree() {
    assert_eq!(wi::linear16(0), 0.0);
    assert_eq!(wi::linear16(65535), 1.0);
    for v in 1..=65535u16 {
        assert!(wi::linear16(v) > wi::linear16(v - 1), "{v}");
    }
    let mut src = Img::new(64, 4, "rgba64");
    let mut r = kikizake::rng::XorShift32::new(7);
    for b in src.px.iter_mut() {
        *b = r.next() as u8;
    }
    let f = |to| wi::wic_convert(&src, to).unwrap();
    let (s, p, g) = (f("rgbafloat"), f("prgbafloat"), f("grayfloat"));
    let at = |i: &Img, k: usize| f32::from_le_bytes(i.px[k * 4..k * 4 + 4].try_into().unwrap());
    for x in 0..64 * 4 {
        let a = at(&s, x * 4 + 3);
        for c in 0..3 {
            assert_eq!(at(&p, x * 4 + c), at(&s, x * 4 + c) * a);
        }
        assert_eq!(at(&g, x), (at(&s, x * 4) * 0.2126 + at(&s, x * 4 + 1) * 0.7152) + at(&s, x * 4 + 2) * 0.0722);
    }
}

/// a conversion out of an indexed format drops the palette, one into it takes the executor's
#[test]
fn conversions_carry_the_palette_they_should() {
    let i = img("idx8", 4, 3, 5);
    assert!(i.pal.is_some());
    assert!(wi::wic_convert(&i, "bgra").unwrap().pal.is_none());
    let b = img("bgra", 4, 3, 5);
    assert!(wi::wic_convert(&b, "idx8").map(|o| o.pal.is_some()).unwrap_or(true));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// at quality 100 every quantizer is 1, so (ITU T.81 A.3.3's DCT being near-orthonormal) a
    /// decoded sample lands within a couple of steps of its source, whatever the size, the
    /// padding blocks and (at 4:4:4) the color; the stream is one SOI ... EOI and decodes to w x h
    #[test]
    fn jpeg_at_quality_100_is_nearly_lossless(w in 1usize..40, h in 1usize..40, seed: u32, color: bool) {
        use kikizake::model::jpeg::{self, Kind, Quality};
        let mut s = seed | 1;
        let mut px = Vec::new();
        let n = if color { 3 } else { 1 };
        for _ in 0..w * h * n {
            s ^= s << 13; s ^= s >> 17; s ^= s << 5;
            px.push((s >> 24) as u8);
        }
        let kind = if color { Kind::Color } else { Kind::Gray };
        let j = jpeg::encode(kind, w, h, &px, Quality::new(100), true, jpeg::Chroma::Full);
        prop_assert_eq!(&j.bytes[..2], &[0xff, 0xd8]);
        prop_assert_eq!(&j.bytes[j.bytes.len() - 2..], &[0xff, 0xd9]);
        prop_assert_eq!(j.px.len(), px.len());
        let worst = px.iter().zip(&j.px).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
        prop_assert!(worst <= if color { 4 } else { 2 }, "worst {}", worst);
    }
}

/// BC3 as Microsoft Learn's "Texture block compression in Direct3D 11" decodes it: two 5:6:5
/// colors and the thirds between them (color0 > color1), two alpha ends and the steps between
/// (6 and the ends 0 and 255 when alpha0 <= alpha1)
fn bc3_decode(b: &[u8; 16]) -> [[u8; 4]; 16] {
    let c = |w: u16| [(w >> 11) & 31, (w >> 5) & 63, w & 31];
    let up = |v: u16, bits: u32| f64::from(v) * 255.0 / f64::from((1u16 << bits) - 1);
    let (w0, w1) = (u16::from_le_bytes([b[8], b[9]]), u16::from_le_bytes([b[10], b[11]]));
    let (c0, c1) = (c(w0), c(w1));
    let e0 = [up(c0[0], 5), up(c0[1], 6), up(c0[2], 5)];
    let e1 = [up(c1[0], 5), up(c1[1], 6), up(c1[2], 5)];
    let pal: [[f64; 3]; 4] = [e0, e1, std::array::from_fn(|k| (2.0 * e0[k] + e1[k]) / 3.0), std::array::from_fn(|k| (e0[k] + 2.0 * e1[k]) / 3.0)];
    let (a0, a1) = (f64::from(b[0]), f64::from(b[1]));
    let apal: Vec<f64> = if b[0] > b[1] {
        (0..8).map(|i| match i { 0 => a0, 1 => a1, _ => ((8 - i) as f64 * a0 + (i - 1) as f64 * a1) / 7.0 }).collect()
    } else {
        (0..8).map(|i| match i { 0 => a0, 1 => a1, 6 => 0.0, 7 => 255.0, _ => ((6 - i) as f64 * a0 + (i - 1) as f64 * a1) / 5.0 }).collect()
    };
    let abits = (0..6).fold(0u64, |a, i| a | u64::from(b[2 + i]) << (8 * i));
    let cbits = u32::from_le_bytes([b[12], b[13], b[14], b[15]]);
    std::array::from_fn(|i| {
        let p = pal[((cbits >> (2 * i)) & 3) as usize];
        let a = apal[((abits >> (3 * i)) & 7) as usize];
        [p[0], p[1], p[2], a].map(|v| v.round() as u8)
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// a block of one color decodes to that color within the 5:6:5 rounding, its alpha exactly;
    /// a block of two alphas (or 0 and 255 with one more) keeps every alpha exactly
    #[test]
    fn bc3_keeps_what_it_can_hold(c: [u8; 3], alphas: [u8; 2], pick: u16, gray: bool) {
        use kikizake::model::bc;
        let rgb = if gray { [c[0]; 3] } else { c };
        let px: [bc::Px; 16] = std::array::from_fn(|i| {
            let a = alphas[usize::from(pick >> i & 1)];
            [rgb[2], rgb[1], rgb[0], a]
        });
        let b = bc::encode_bc3(&px);
        let d = bc3_decode(&b);
        for (i, p) in d.iter().enumerate() {
            for k in 0..3 {
                let tol = if k == 1 { 3 } else { 5 };
                prop_assert!(p[k].abs_diff(rgb[k]) <= tol, "{:?} vs {:?}", p, rgb);
            }
            prop_assert_eq!(p[3], alphas[usize::from(pick >> i & 1)]);
        }
    }
}

/// an ICO of one PNG entry and one BMP entry: both file orders give the same two frames, in the
/// same order or each with the later entry first (the rule is of the pair, not of the file), and
/// a PNG frame's CopyPixels fails exactly when it comes after a wider frame
#[test]
fn ico_pairs_order_alike_in_both_file_orders() {
    use kikizake::generators::{wicimage_bmp, wicimage_ico, wicimage_png, wicimage_png_filter};
    use wi::Frame;
    proptest!(ProptestConfig::with_cases(400), |(pw in 1u32..20, ph in 1u32..20, bw in 1u32..20, bh in 1u32..20, depth in prop::sample::select(vec![1u32, 4, 8, 24, 32]), seed in any::<u8>())| {
        let raw: Vec<Vec<u8>> = (0..ph).map(|y| (0..pw * 4).map(|x| (x * 31 + y * 7 + seed as u32) as u8).collect()).collect();
        let p = wicimage_png(pw, ph, 8, 6, false, &[], &wicimage_png_filter(&raw, 4, &[0]), 100);
        let stride = (bw * depth).div_ceil(32) * 4;
        let pal: Vec<u32> = if depth <= 8 { (0..1u32 << depth).map(|i| i * 0x10101).collect() } else { vec![] };
        let px: Vec<u8> = (0..(stride + bw.div_ceil(32) * 4) * bh).map(|i| (i * 13 + seed as u32) as u8).collect();
        let b = wicimage_bmp(40, bw as i32, (2 * bh) as i32, depth, 0, [0; 4], 0, &pal, &px, false, (0, 0))[14..].to_vec();
        let order = |f: &[u8]| -> Vec<(u32, u32, bool)> {
            let d = wi::decode(f).unwrap().unwrap();
            d.frames.into_iter().map(|fr| match fr.unwrap() {
                Frame::Pixels(i) => (i.w, i.h, false),
                Frame::NoPixels(i, hr) => { assert_eq!(hr, wi::INSUFFICIENT_BUFFER); (i.w, i.h, true) }
                Frame::NoInfo(hr) => panic!("no frame info {hr:x}"),
            }).collect()
        };
        let pb = order(&wicimage_ico(&[(pw, ph, 32, p.clone()), (bw, bh, depth, b.clone())]));
        let bp = order(&wicimage_ico(&[(bw, bh, depth, b), (pw, ph, 32, p)]));
        prop_assert_eq!(pb.len(), 2);
        let png_first = |o: &[(u32, u32, bool)], png_in_file_first: bool| if (pw, ph) == (bw, bh) { None } else { Some((o[0].0, o[0].1) == (pw, ph)) }.map(|x| x != png_in_file_first);
        // (each file order: did the later entry come first)
        let (a, z) = (png_first(&pb, true), png_first(&bp, false));
        if let (Some(a), Some(z)) = (a, z) {
            // (the same frame first in both, or the later first in both)
            prop_assert!(a != z || a, "{pw}x{ph} PNG, {bw}x{bh} {depth}-bit BMP: {pb:?} {bp:?}");
        }
        for o in [&pb, &bp] {
            let starved = o[1].2;
            let second_is_png = (o[1].0, o[1].1) == (pw, ph) && ((o[0].0, o[0].1) != (pw, ph) || starved);
            prop_assert_eq!(starved, second_is_png && o[0].0 > o[1].0);
            prop_assert!(!o[0].2);
        }
    });
}

/// every 16-bit gray level: into 64bppRGBAFixedPoint directly, or first into any s2.13 format,
/// is the same (both go through 48bppRGB, and s2.13 into 64bppRGBAFixedPoint keeps the values)
#[test]
fn gray16_into_fixed_point_through_any_s2_13_format() {
    let mut g = Img::new(256, 256, "gray16");
    for v in 0..65536usize {
        g.px[2 * v..2 * v + 2].copy_from_slice(&(v as u16).to_le_bytes());
    }
    let direct = wi::wic_convert(&g, "rgba64fixed").unwrap();
    for mid in ["grayfix16", "rgb48fixed", "rgb64fixed", "bgra64fixed"] {
        let m = wi::wic_convert(&g, mid).unwrap();
        let back = wi::wic_convert(&m, "rgba64fixed").unwrap();
        assert!(back.px == direct.px, "through {mid}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(40))]

    /// more colors than 8bppIndexed holds with transparent pixels: 255 grid entries, all
    /// different, then the transparent one, which every transparent pixel and no other takes
    #[test]
    fn grid_leaves_the_last_entry_transparent(w in 17u32..40, h in 16u32..30, seed: u32, every in 1usize..50) {
        let mut r = kikizake::rng::XorShift32::new(seed);
        let mut i = Img::new(w, h, "bgra");
        for k in 0..(w * h) as usize {
            let c = r.next().to_le_bytes();
            i.px[k * 4..k * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], if k % every == 0 { 0 } else { 255 }]);
        }
        let o = wi::encode_source(&i, "idx8").unwrap();
        let pal = o.pal.clone().unwrap();
        prop_assert_eq!(pal.len(), 256);
        prop_assert_eq!(pal[255], 0);
        let mut seen = pal[..255].to_vec();
        seen.sort_unstable();
        seen.dedup();
        prop_assert_eq!(seen.len(), 255);
        for k in 0..(w * h) as usize {
            prop_assert_eq!(o.px[k] == 255, k % every == 0);
        }
    }
}

/// s2.13 back into 16 bits climbs from black to white; every 8-bit level through
/// 64bppRGBAFixedPoint and back keeps its alpha and its color or the one below (the 8-bit
/// tables of the two ways are not each other's inverse)
#[test]
fn fixed_point_goes_back_where_it_came_from() {
    for t in [wi::FixBack::Color, wi::FixBack::Gray] {
        let at = |v: i16| wi::fix13_srgb16(wi::Fix13::new(v).unwrap(), t);
        assert_eq!((at(0), at(8192)), (0, 65535));
        assert!((1..=8192).all(|v| at(v - 1) <= at(v)));
    }
    assert!(wi::Fix13::new(-1).is_none() && wi::Fix13::new(8193).is_none());
    let mut i = Img::new(256, 256, "bgra");
    for k in 0..65536usize {
        i.px[k * 4..k * 4 + 4].copy_from_slice(&[k as u8, (k * 7) as u8, (k * 13) as u8, (k >> 8) as u8]);
    }
    let f = wi::wic_convert(&i, "rgba64fixed").unwrap();
    let b = wi::wic_convert(&f, "bgra").unwrap();
    for (k, (&got, &was)) in b.px.iter().zip(&i.px).enumerate() {
        match k % 4 {
            3 => assert_eq!(got, was),
            _ => assert!(got == was || got + 1 == was, "{k}: {was} back as {got}"),
        }
    }
}

/// floats back into 8 and 16 bits climb with the value from black to white, every 16-bit
/// level's float lands within one level of where it came from, and nothing outside 0..=1 or NaN
/// gets an answer
#[test]
fn floats_go_back_in_order() {
    for t in [wi::FloatBack::Bits16, wi::FloatBack::Bits8] {
        let top = if t == wi::FloatBack::Bits16 { 65535 } else { 255 };
        assert_eq!((wi::float_srgb(0.0, t), wi::float_srgb(1.0, t)), (0, top));
        for (x, want) in [(-0.0001f32, 0), (-0.0, 0), (f32::NEG_INFINITY, 0), (f32::NAN, 0), (-f32::NAN, 0), (1.0001, top), (f32::INFINITY, top)] {
            assert_eq!(wi::float_srgb(x, t), want, "{x}");
        }
        let mut was = 0;
        for v in 0..=65535u16 {
            let got = wi::float_srgb(wi::linear16(v), t);
            assert!(got >= was);
            was = got;
            if t == wi::FloatBack::Bits16 {
                assert!(got.abs_diff(v) <= 1, "{v} back as {got}");
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// every float of 0..1 has a level of its own steps, the levels climb with the bits of the float,
    /// and the 8-bit level is never far from the 16-bit one's 257ths
    #[test]
    fn every_float_has_its_level(a in 0u32..=0x3f80_0000, b in 0u32..=0x3f80_0000) {
        let (lo, hi) = (a.min(b), a.max(b));
        for t in [wi::FloatBack::Bits16, wi::FloatBack::Bits8] {
            let l = wi::float_srgb(f32::from_bits(lo), t);
            let h = wi::float_srgb(f32::from_bits(hi), t);
            prop_assert!(l <= h);
        }
        // (Windows spreads its 8.8 position over 16 bits so that no multiple of 256 but 0 is reached)
        let w16 = wi::float_srgb(f32::from_bits(hi), wi::FloatBack::Bits16);
        prop_assert!(w16 == 0 || w16 % 256 != 0, "{hi:#x}: {w16}");
        let x = f32::from_bits(lo);
        let (w, n) = (wi::float_srgb(x, wi::FloatBack::Bits16), wi::float_srgb(x, wi::FloatBack::Bits8));
        prop_assert!((f64::from(w) / 257.0 - f64::from(n)).abs() <= 1.0, "{x}: {w} and {n}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// 16bppGrayFixedPoint of a color lies between its darkest and brightest channel as
    /// 64bppRGBAFixedPoint, and a gray color (r = g = b) keeps its level exactly
    #[test]
    fn grayfix_is_a_mean_of_the_channels(w in 1u32..20, h in 1u32..10, seed: u32) {
        let i = img("bgra", w, h, seed);
        let f = wi::wic_convert(&i, "rgba64fixed").unwrap();
        let g = wi::wic_convert(&i, "grayfix16").unwrap();
        for k in 0..(w * h) as usize {
            let c = |j: usize| i16::from_le_bytes([f.px[k * 8 + j * 2], f.px[k * 8 + j * 2 + 1]]);
            let y = i16::from_le_bytes([g.px[k * 2], g.px[k * 2 + 1]]);
            let (lo, hi) = (c(0).min(c(1)).min(c(2)), c(0).max(c(1)).max(c(2)));
            prop_assert!(lo <= y && y <= hi);
        }
        let gray = img("gray8", w, h, seed);
        let a = wi::wic_convert(&gray, "grayfix16").unwrap();
        let b = wi::wic_convert(&wi::wic_convert(&gray, "bgr24").unwrap(), "grayfix16").unwrap();
        prop_assert_eq!(a.px, b.px);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// a 16-bit HighQualityCubic pass's weights: one window per output inside the source, its
    /// weights adding to 1 within f32 rounding, and a pass of the same size the source itself
    #[test]
    fn hq16_weights_add_to_one(n in 1u32..200, d in 1u32..200) {
        let t = wi::hq_taps32(n, d);
        prop_assert_eq!(t.len(), d as usize);
        for o in &t {
            prop_assert!(!o.w.is_empty() && o.start + o.w.len() <= n as usize);
            let sum: f32 = o.w.iter().sum();
            prop_assert!((sum - 1.0).abs() < 1e-5, "{n}->{d}: {sum}");
        }
        if n == d {
            for (i, o) in wi::hq_taps32(n, n).iter().enumerate() {
                for (m, &w) in o.w.iter().enumerate() {
                    prop_assert_eq!(w, if o.start + m == i { 1.0 } else { 0.0 });
                }
            }
        }
    }
}

/// a JPEG XR test picture's values, channel by channel, in its own units: bits past a row's
/// end, the pad of 32bppBGR and 16bppBGR555's top bit do not count
fn jxr_values(f: &str, w: usize, h: usize, stride: usize, px: &[u8]) -> Vec<i64> {
    let mut v = vec![];
    for y in 0..h {
        let r = &px[y * stride..];
        for x in 0..w {
            match f {
                "bw" => v.push(i64::from(r[x / 8] >> (7 - x % 8) & 1)),
                "bgr555" | "bgr565" => {
                    let u = u16::from_le_bytes([r[2 * x], r[2 * x + 1]]);
                    let c = if f == "bgr555" { [u & 31, u >> 5 & 31, u >> 10 & 31] } else { [u & 31, u >> 5 & 63, u >> 11] };
                    v.extend(c.map(i64::from));
                }
                "gray16" | "rgb48" | "rgba64" => {
                    let n = match f {
                        "gray16" => 1,
                        "rgb48" => 3,
                        _ => 4,
                    };
                    v.extend((0..n).map(|k| i64::from(u16::from_le_bytes([r[2 * (x * n + k)], r[2 * (x * n + k) + 1]]))));
                }
                "bgr32" => v.extend(r[4 * x..4 * x + 3].iter().map(|&b| i64::from(b))),
                _ => {
                    let n = match f {
                        "gray8" => 1,
                        "bgr24" | "rgb24" => 3,
                        _ => 4,
                    };
                    v.extend(r[n * x..n * (x + 1)].iter().map(|&b| i64::from(b)));
                }
            }
        }
    }
    v
}

const JXR_FMTS: &[&str] = &["bw", "gray8", "gray16", "bgr555", "bgr565", "bgr24", "rgb24", "bgr32", "bgra", "pbgra", "rgb48", "rgba64", "cmyk"];

proptest! {
    #![proptest_config(ProptestConfig::with_cases(150))]

    /// the JPEG XR model (src/model/jxr.rs) decodes what its encoder wrote: QP index 0 and 1
    /// (T.832's step 1) give back every value whatever the overlap; a coarser QP index up to 32
    /// (WIC's 0.9 is 16) gives 8-bit values back within 2 x index + 2; planar alpha is coded lossless
    /// at any quality; and the file holds the HD Photo header, the IFD's size and the codestream
    #[test]
    fn jxr_decodes_what_it_encodes(fi in 0usize..13, w in 1usize..40, h in 1usize..40, seed: u32, index in 0u8..=32, ov in 0usize..3, smooth: bool) {
        use kikizake::model::jxr::{self, Format, Overlap};
        let name = JXR_FMTS[fi];
        let fmt = Format::from_name(name).unwrap();
        let stride = (w * fmt.bits()).div_ceil(8);
        let mut r = kikizake::rng::XorShift32::new(seed);
        let px: Vec<u8> = (0..stride * h).map(|k| if smooth { (k / 7 * 3 % 251) as u8 } else { r.next() as u8 }).collect();
        let p = jxr::Picture { fmt, w, h, stride, px: &px, dpi: (0.0, 0.0) };
        let ov = [Overlap::None, Overlap::One, Overlap::Two][ov];
        let src = jxr_values(name, w, h, stride, &px);
        let lossless = jxr::encode_at(&p, index.min(1), ov);
        prop_assert_eq!(&jxr_values(name, w, h, stride, &lossless.decoded), &src, "{} {}x{} lossless", name, w, h);
        prop_assert_eq!(&lossless.file[..4], b"II\xbc\x01");
        let lossy = jxr::encode_at(&p, index, ov);
        let got = jxr_values(name, w, h, stride, &lossy.decoded);
        if matches!(name, "gray8" | "bgr24" | "rgb24" | "bgr32" | "bgra" | "pbgra" | "cmyk") {
            let worst = src.iter().zip(&got).map(|(a, b)| (a - b).abs()).max().unwrap_or(0);
            prop_assert!(worst <= 2 * i64::from(index) + 2, "{} {}x{} index {}: off by {}", name, w, h, index, worst);
        }
        if matches!(name, "bgra" | "pbgra" | "rgba64") {
            let alpha = |v: &[i64]| v.iter().skip(3).step_by(4).copied().collect::<Vec<_>>();
            prop_assert_eq!(alpha(&got), alpha(&src), "{} alpha", name);
        }
        // (the image codestream's offset: the IFD's 9 entries, 11 with planar alpha, then its
        // next-IFD word and the 16-byte pixel format GUID)
        let at = if fmt.bits() == 64 || matches!(name, "bgra" | "pbgra") { 162 } else { 138 };
        prop_assert_eq!(&lossy.file[at..at + 8], b"WMPHOTO\0");
        let wq = jxr::encode(&p, jxr::Quality::new(0.9).unwrap());
        prop_assert_eq!(&wq.file, &jxr::encode(&p, jxr::Quality::default_wic()).file);
    }
}

proptest! {
    /// icm32's engine (model::icm): sRGB to sRGB stays within 3 of each channel (the coarse
    /// grid near black), white stays white and black black, every space; and the 16-bit path
    /// agrees with the 8-bit one on the levels 8 bits name, within 5 past the darkest levels (near black the 16-bit path interpolates the steep curves between ALUT entries, where the 8-bit one takes the lower)
    #[test]
    fn icm_transforms_stay_near(r in any::<u8>(), g in any::<u8>(), b in any::<u8>()) {
        use kikizake::model::icm::{transform, Space};
        let same = transform(Space::Srgb, Space::Srgb).rgb8([r, g, b]);
        for (x, y) in same.iter().zip([r, g, b]) {
            prop_assert!(x.abs_diff(y) <= 3, "{same:?} {r} {g} {b}");
        }
        for (f, t) in [(Space::Srgb, Space::Adobe), (Space::Adobe, Space::Srgb), (Space::Adobe, Space::Adobe), (Space::Srgb, Space::Srgb)] {
            let x = transform(f, t);
            prop_assert_eq!(x.rgb8([255; 3]), [255; 3]);
            prop_assert_eq!(x.rgb8([0; 3]), [0; 3]);
            let e8 = x.rgb8([r, g, b]);
            let e16 = x.rgb16([r, g, b].map(|v| u16::from(v) * 257));
            for (a, (w, v)) in e8.iter().zip(e16.into_iter().zip([r, g, b])) {
                prop_assert!(v < 32 || (i32::from(*a) - i32::from(w >> 8)).abs() <= 5, "{e8:?} {e16:?}");
            }
        }
    }
}

/// a float reference for WIC's CMYK: RSWOP's A2B0 read as plain numbers, its input curves and
/// grid interpolated multilinearly, ICC v2 Lab to XYZ (D50), the inverse of the sRGB colorants,
/// the sRGB function; 8-bit B, G, R as reals
fn cmyk_float_reference(c: [u8; 4]) -> [f64; 3] {
    use kikizake::model::cmyk::SwopTag;
    let b = SwopTag::A2B0.bytes();
    let be = |o: usize| f64::from(u16::from_be_bytes([b[o], b[o + 1]]));
    let g = usize::from(b[10]);
    let nin = be(48) as usize;
    let curve = |k: usize, x: f64| {
        let p = x * (nin - 1) as f64;
        let i = (p as usize).min(nin - 2);
        let f = p - i as f64;
        (be(52 + 2 * (k * nin + i)) * (1.0 - f) + be(52 + 2 * (k * nin + i + 1)) * f) / 65535.0
    };
    let pos: [f64; 4] = std::array::from_fn(|k| curve(k, f64::from(c[k]) / 255.0) * (g - 1) as f64);
    let idx = pos.map(|p| (p as usize).min(g - 2));
    let clut = 52 + 2 * 4 * nin;
    let mut lab = [0.0f64; 3];
    for corner in 0..16usize {
        let mut w = 1.0;
        let mut at = 0;
        for k in 0..4 {
            let up = corner >> (3 - k) & 1;
            let f = pos[k] - idx[k] as f64;
            w *= if up == 1 { f } else { 1.0 - f };
            at = at * g + idx[k] + up;
        }
        for (ch, v) in lab.iter_mut().enumerate() {
            *v += w * be(clut + 2 * (3 * at + ch));
        }
    }
    let (l, a, bb) = (lab[0] / 65280.0 * 100.0, lab[1] / 256.0 - 128.0, lab[2] / 256.0 - 128.0);
    let fi = |t: f64| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let fy = (l + 16.0) / 116.0;
    let xyz = [0.9642 * fi(fy + a / 500.0), fi(fy), 0.8249 * fi(fy - bb / 200.0)];
    let m: [[f64; 3]; 3] = std::array::from_fn(|r| std::array::from_fn(|k| f64::from(kikizake::model::icm::SRGB_XYZ[k][r]) / 65536.0));
    let cof = |r: usize, k: usize| m[(r + 1) % 3][(k + 1) % 3] * m[(r + 2) % 3][(k + 2) % 3] - m[(r + 1) % 3][(k + 2) % 3] * m[(r + 2) % 3][(k + 1) % 3];
    let det = m[0][0] * cof(0, 0) + m[0][1] * cof(0, 1) + m[0][2] * cof(0, 2);
    let rgb: [f64; 3] = std::array::from_fn(|r| (0..3).map(|k| cof(k, r) / det * xyz[k]).sum());
    let enc = |v: f64| {
        let v = v.clamp(0.0, 1.0);
        255.0 * if v <= 0.0031308 { 12.92 * v } else { 1.055 * v.powf(1.0 / 2.4) - 0.055 }
    };
    [enc(rgb[2]), enc(rgb[1]), enc(rgb[0])]
}

proptest! {
    /// WIC's CMYK (model::cmyk, icm32's integer pipeline): within 8 levels of the float
    /// colorimetric reading of the same RSWOP table (the integer simplex in a 16^4 table against
    /// multilinear float); no ink is paper white, all ink near black
    #[test]
    fn cmyk_stays_near_float_reference(c in any::<[u8; 4]>()) {
        let t = kikizake::model::cmyk::to_rgb().ok_or_else(|| TestCaseError::fail("no CMYK table"))?;
        let got = t.bgr8(c);
        let want = cmyk_float_reference(c);
        for (g, w) in got.iter().zip(want) {
            prop_assert!((f64::from(*g) - w).abs() <= 8.0, "{c:?}: {got:?} against {want:?}");
        }
        prop_assert_eq!(t.rgb16([0; 4]), [65535, 65362, 65033]);
        prop_assert!(t.bgr8([255; 4]).iter().all(|&v| v <= 3));
    }
}

proptest! {
    /// WIC's GIF row reader (model::gifread): the encoder's own LZW, in sub-blocks of 255,
    /// reads back as the pixels it was made of, row by row; any bytes at all read without a
    /// panic, each row whole or WINCODEC_ERR_BADIMAGE
    #[test]
    fn gif_rows_read_back_and_never_panic(w in 1u32..40, h in 1u32..20, bits in 2u32..9, seed in any::<u32>(), junk in proptest::collection::vec(any::<u8>(), 0..400), min in 0u8..12) {
        use kikizake::model::gifread::Rows;
        let mut x = seed | 1;
        let px: Vec<u8> = (0..w * h).map(|_| { x ^= x << 13; x ^= x >> 17; x ^= x << 5; (x as u8) & ((1u16 << bits) - 1) as u8 }).collect();
        let lzw = wi::lzw_encode(bits, &px);
        let mut data = Vec::new();
        for c in lzw.chunks(255) {
            data.push(c.len() as u8);
            data.extend_from_slice(c);
        }
        data.push(0);
        let mut r = Rows::new(&data, bits as u8, w).ok_or_else(|| TestCaseError::fail("no decoder"))?;
        for y in 0..h as usize {
            let mut row = vec![0xaa; w as usize];
            prop_assert_eq!(r.row(&mut row), Ok(()));
            prop_assert_eq!(&row[..], &px[y * w as usize..(y + 1) * w as usize]);
        }
        if let Some(mut r) = Rows::new(&junk, min, w) {
            for _ in 0..h {
                let mut row = vec![0xaa; w as usize];
                match r.row(&mut row) {
                    Ok(()) => {}
                    Err(e) => prop_assert_eq!(e, wi::BAD_IMAGE),
                }
            }
        } else {
            prop_assert!(min > 8);
        }
    }
}

/// JPEG 4:4:0 (WICJpegYCrCbSubsampling440, chroma halved vertically only): the decoder
/// interpolates the chroma rows 3:1 (h1v2 fancy upsampling), it does not repeat them (the
/// review of 8480456: repeating gave 35/72, 723/819 and 95/102 bytes wrong). The three cases
/// Windows answered, pinned against their goldens on both architectures: an odd height, a
/// narrow image and edge rows, whatever KIKIZAKE_SUITES selects for the windows test
#[test]
fn jpeg_440_chroma_is_interpolated() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let all = kikizake::case::parse_cases(&std::fs::read_to_string(root.join("cases/wicencode.txt")).unwrap());
    let names = ["opt-jpeg-subs-2-4-bgra", "opt-jpeg-subs-4-4-bgr24", "opt-jpeg-subs-8-4-bgra"];
    let cases: Vec<_> = all.into_iter().filter(|c| names.contains(&c.name.as_str())).collect();
    assert_eq!(cases.len(), 3);
    for arch in [32, 64] {
        let g = std::fs::read_to_string(root.join(format!("golden/wicencode-{arch}/stdout.txt"))).unwrap();
        let want = kikizake::model::ModelContext::recorded(arch, &g).evaluate(|| kikizake::model::run("wicencode", &cases));
        let golden = kikizake::case::parse_output(&g);
        let mine = kikizake::case::parse_output(&want);
        for n in names {
            let (a, b) = (mine.cases.iter().find(|c| c.0 == n), golden.cases.iter().find(|c| c.0 == n));
            assert!(a.is_some() && a == b, "{arch} {n}: {a:?} {b:?}");
        }
    }
}
