//! D3DXLoadSurfaceFromMemory's read of DXT blocks (spec/d3dx9.txt D3DX9.LOAD.DXT) over the whole
//! domain (kind exhaustive): every pair of colour endpoints with every index, every DXT3 alpha,
//! every DXT5 pair of alpha endpoints with every index; and the formats' wiring, every index of
//! every texel of a block through x9load.
use kikizake::model::d3dx9::{dxt3_alpha, dxt5_alpha, dxt_colour};

mod every;
use every::all_u32;

/// a 565 colour's channels as the clause reads them: c * (1/31), c * (1/63), c * (1/31) in f32
fn ends(c: u16) -> [f32; 3] {
    [f32::from(c >> 11) * (1.0 / 31.0), f32::from(c >> 5 & 63) * (1.0 / 63.0), f32::from(c & 31) * (1.0 / 31.0)]
}

/// D3DX9.LOAD.DXT: every c0, c1 (2^32) and index: index 0 and 1 the endpoints; four colours
/// where c0 > c1, index 2 a + (b - a) * (1/3), 3 a + (b - a) * (2/3); else index 2
/// a + (b - a) * 0.5 and 3 black, in f32
#[test]
fn every_colour_block_texel() {
    assert_eq!(all_u32(|w| {
        let (c0, c1) = (u16::try_from(w & 0xffff).unwrap(), u16::try_from(w >> 16).unwrap());
        let (a, b) = (ends(c0), ends(c1));
        (0..4).all(|idx| {
            let want: [f32; 3] = std::array::from_fn(|k| match (idx, c0 > c1) {
                (0, _) => a[k],
                (1, _) => b[k],
                (2, true) => a[k] + (b[k] - a[k]) * (1.0 / 3.0),
                (3, true) => a[k] + (b[k] - a[k]) * (2.0 / 3.0),
                (2, false) => a[k] + (b[k] - a[k]) * 0.5,
                _ => 0.0,
            });
            dxt_colour(c0, c1, idx).map(f32::to_bits) == want.map(f32::to_bits)
        })
    }), None);
}

/// D3DX9.LOAD.DXT: DXT3's 16 alphas a4 * (1/15); DXT5's every b0, b1 and index: the endpoints
/// b * (1/255), between them ((n - k) a0 + k a1) * (1/n), n 7 where b0 > b1, else 5 and
/// indices 6 and 7 for 0 and 1
#[test]
fn every_alpha() {
    for a4 in 0..16u8 {
        assert_eq!(dxt3_alpha(a4).to_bits(), (f32::from(a4) * (1.0 / 15.0)).to_bits());
    }
    for b0 in 0..=255u8 {
        for b1 in 0..=255u8 {
            let (a0, a1) = (f32::from(b0) * (1.0 / 255.0), f32::from(b1) * (1.0 / 255.0));
            for i in 0..8u8 {
                let n = if b0 > b1 { 7 } else { 5 };
                let want = match i {
                    0 => a0,
                    1 => a1,
                    6 | 7 if n == 5 => f32::from(i - 6),
                    _ => (f32::from(n - (i - 1)) * a0 + f32::from(i - 1) * a1) * (1.0 / f32::from(n)),
                };
                assert_eq!(dxt5_alpha(b0, b1, i).to_bits(), want.to_bits(), "{b0} {b1} {i}");
            }
        }
    }
}

/// D3DX9.LOAD.DXT: the three colour mode where c0 <= c1 in every DXT format; DXT1's alpha 0 at
/// index 3 there, 1 elsewhere; DXT3's and DXT5's alpha their own; DXT2 and DXT4 divided back,
/// colour * (1/a) at most 1, 0 where a is 0: every texel of a 4 x 4 block of each format,
/// colour endpoint pairs both ways round, every colour index, alpha endpoints both ways round
#[test]
fn every_texel_of_each_format() {
    use kikizake::{case, model};
    model::set_arch(32);
    let fmts = [(0x3154_5844u32, 1), (0x3254_5844, 2), (0x3354_5844, 3), (0x3454_5844, 4), (0x3554_5844, 5)];
    for (fmt, n) in fmts {
        for (c0, c1) in [(0xf81fu16, 0x07e0u16), (0x07e0, 0xf81f), (0x1234, 0x1234)] {
            for (b0, b1) in [(200u8, 30u8), (30, 200)] {
                // (texel t takes colour index t % 4, DXT3 alpha t, DXT5 alpha index t % 8)
                let idx: u32 = (0..16).map(|t| (t % 4) << (2 * t)).fold(0, |a, x| a | x);
                let colour = [c0.to_le_bytes(), c1.to_le_bytes()].concat().into_iter().chain(idx.to_le_bytes()).collect::<Vec<u8>>();
                let a3: u64 = (0..16u64).map(|t| t << (4 * t)).fold(0, |a, x| a | x);
                let a5: u64 = (0..16u64).map(|t| (t % 8) << (3 * t)).fold(0, |a, x| a | x);
                let alpha: Vec<u8> = match n {
                    1 => Vec::new(),
                    2 | 3 => a3.to_le_bytes().to_vec(),
                    _ => [b0, b1].into_iter().chain(a5.to_le_bytes()[..6].iter().copied()).collect(),
                };
                let block: String = alpha.iter().chain(&colour).map(|x| format!("{x:02x}")).collect();
                let out = model::run_case(&case::parse_cases(&format!("case c\ndevice swvp\nx9load 116 {fmt} 4 4 b:{block} 1\nend\n"))[0]).unwrap();
                let rows: Vec<&String> = out.iter().filter(|l| !l.starts_with("px") && !l.starts_with("device")).collect();
                for t in 0..16usize {
                    let row = rows[t / 4].as_bytes();
                    let px = &row[32 * (t % 4)..32 * (t % 4) + 32];
                    let got: Vec<u32> = (0..4).map(|k| u32::from_str_radix(std::str::from_utf8(&px[8 * k..8 * k + 8]).unwrap(), 16).unwrap().swap_bytes()).collect();
                    let ti = u32::try_from(t % 4).unwrap();
                    let rgb = dxt_colour(c0, c1, ti);
                    let a = match n {
                        1 => if c0 <= c1 && ti == 3 { 0.0 } else { 1.0 },
                        2 | 3 => dxt3_alpha(u8::try_from(t).unwrap()),
                        _ => dxt5_alpha(b0, b1, u8::try_from(t % 8).unwrap()),
                    };
                    let rgb = if matches!(n, 2 | 4) { rgb.map(|v| if a == 0.0 { 0.0 } else { (v * (1.0 / a)).min(1.0) }) } else { rgb };
                    let want = [rgb[0], rgb[1], rgb[2], a].map(f32::to_bits);
                    assert_eq!(got, want, "{n} {c0:x} {c1:x} {b0} {b1} {t}");
                }
            }
        }
    }
}
