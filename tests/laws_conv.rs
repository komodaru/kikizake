//! The format conversions of Direct3D 10, 11 and 12 (spec/d3dres.txt CONV.*, spec/d3d12.txt
//! D3D12.CLEAR.X, XALPHA, CLEARDSV): each law runs over the whole domain it names (every 32-bit
//! float, every code, every endpoint pair), so it is a proof for that domain (kind exhaustive).
//! The references are the clauses' own words in exact integer arithmetic, apart from the
//! model's functions.
use kikizake::model::bptc::{self, bc6h_mode, bc7_mode, finish, lerp, unquantize, BC6H_BITS, BC6H_MODES, BC7_ALPHA, BC7_PBITS, BC7_RGB, PART2, PART3, W2, W3, W4};
use kikizake::model::d3d12::{self, clear_depth, clear_input, clear_texel, unorm_depth};
use kikizake::model::dxgi_format::{channels, Enc};
use kikizake::model::dxgiconv::{bc1_codes, bc3_alpha, bc4_code, bc4_snorm_code, decode, decode_block, encode, f32_store, from_small_float, int_clear, srgb8, srgb_read, Unorm12, SRGB_READ};
use kikizake::model::dxgifmt::{format, Format};

mod every;
use every::all_u32;

/// runs `f` over 0..n on the machine's threads (a failing assert fails the law)
fn par(n: u32, f: impl Fn(u32) + Sync) {
    let k = std::thread::available_parallelism().map_or(4, |n| n.get()).min(16) as u32;
    std::thread::scope(|s| {
        for t in 0..k {
            let f = &f;
            s.spawn(move || (t * (n / k)..if t + 1 == k { n } else { (t + 1) * (n / k) }).for_each(f));
        }
    });
}

fn fmt(id: u32) -> &'static Format {
    format(id).unwrap()
}

/// a float's value truncated toward zero, exactly, from its bits (None: NaN or an infinity)
fn trunc_exact(v: u32) -> Option<i128> {
    let (sign, ex, man) = (v >> 31, (v >> 23 & 0xff) as i32, i128::from(v & 0x7f_ffff));
    if ex == 255 {
        return None;
    }
    // (value = m 2^(e - 150), m with the hidden bit)
    let (m, e) = if ex == 0 { (man, -149) } else { (man | 1 << 23, ex - 150) };
    // (past 2^80 every type clamps: the shift stops there)
    let t = if e >= 0 { m << e.min(56) } else if e <= -25 { 0 } else { m >> -e };
    Some(if sign == 1 { -t } else { t })
}

/// the exact sRGB curve of a linear value in 0..1
fn curve(q: f64) -> f64 {
    if q <= 0.003_130_8 { q * 12.92 } else { 1.055 * q.powf(1.0 / 2.4) - 0.055 }
}

/// CONV.SRGB: for every float: its 12-bit UNORM code is the nearest (ties to even) of the
/// product, NaN 0, clamped to 0..1; the 8-bit sRGB code is round(255 curve(code / 4095)) with
/// the exact curve (no code lies within 1e-4 of a tie, so the rounding is not in doubt)
#[test]
fn srgb_is_the_curve_of_the_twelve_bit_code() {
    let table: Vec<u8> = (0..4096u32)
        .map(|k| {
            let x = 255.0 * curve(f64::from(k) / 4095.0);
            assert!((x - x.floor() - 0.5).abs() > 1e-4, "code {k} too near a tie");
            (x + 0.5).floor() as u8
        })
        .collect();
    assert_eq!(all_u32(|v| {
        let f = f32::from_bits(v);
        let k = Unorm12::of(f).get();
        let code_ok = if f.is_nan() || f <= 0.0 {
            k == 0
        } else if f >= 1.0 {
            k == 4095
        } else {
            let p = f64::from(f * 4095.0);
            let d = (f64::from(k) - p).abs();
            d < 0.5 || d == 0.5 && k % 2 == 0
        };
        code_ok && srgb8(f) == table[usize::from(k)]
    }), None);
}

/// the distance in ulps of two positive floats
fn ulps(a: f32, b: f32) -> u32 {
    a.to_bits().abs_diff(b.to_bits())
}

/// CONV.SRGBREAD: every 8-bit sRGB code reads as WARP's table, at most 6 ulps off the exact
/// curve, rising, 0 and 1 at the ends; R8G8B8A8_UNORM_SRGB and B8G8R8A8_UNORM_SRGB read every
/// code of every colour component through it (each texel every code in all four components, and
/// each component every code with the others 0), alpha as UNORM; the sRGB block formats read the
/// UNORM block's codes through it (every UNORM code k / 255 gives k back; every c0 with c1 0, c0,
/// its complement and 0xffff, the four indices of BC1_UNORM_SRGB, BC2 and BC3; every BC7 mode 6
/// block of one endpoint colour)
#[test]
fn srgb_reads_are_warps_table() {
    let mut last = -1.0f32;
    for c in 0..=255u8 {
        let r = srgb_read(c);
        assert_eq!(r.to_bits(), SRGB_READ[usize::from(c)]);
        let x = f64::from(c) / 255.0;
        let exact = (if x <= 0.040_45 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }) as f32;
        assert!(c == 0 && r == 0.0 || ulps(r, exact) <= 6, "code {c}: {r} vs {exact}");
        assert!(r > last, "code {c}");
        last = r;
    }
    assert_eq!(srgb_read(255), 1.0);
    for id in [29u32, 91] {
        let f = fmt(id);
        for c in 0..=255u8 {
            let s = srgb_read(c).to_bits();
            let a = (f32::from(c) / 255.0).to_bits();
            assert_eq!(decode(f, &[c; 4]).unwrap(), [s, s, s, a], "{id} {c}");
            for at in 0..4 {
                let mut b = [0u8; 4];
                b[at] = c;
                let got = decode(f, &b).unwrap();
                // (B8G8R8A8: the first byte is blue)
                let ch = if id == 91 && at < 3 { 2 - at } else { at };
                for (k, g) in got.iter().enumerate() {
                    let want = if k == ch { if k == 3 { a } else { s } } else if k == 3 { 0 } else { 0 };
                    assert_eq!(*g, want, "{id} byte {at} code {c} channel {k}");
                }
            }
        }
    }
    for k in 0..=255u32 {
        assert_eq!(((k as f32 / 255.0) * 255.0).round() as u32, k);
    }
    let (b1, b1s) = (fmt(71), fmt(72));
    let (b2, b2s, b3, b3s) = (fmt(74), fmt(75), fmt(77), fmt(78));
    for c0 in 0..=0xffffu32 {
        for c1 in [0, c0, !c0 & 0xffff, 0xffff] {
            let mut blk = [0u8; 16];
            blk[0..2].copy_from_slice(&(c0 as u16).to_le_bytes());
            blk[2..4].copy_from_slice(&(c1 as u16).to_le_bytes());
            blk[4] = 0xe4;
            let mut wide = [0u8; 16];
            wide[0] = 0xff;
            wide[8..16].copy_from_slice(&blk[0..8]);
            for x in 0..4 {
                for (u, s, b) in [(b1, b1s, &blk), (b2, b2s, &wide), (b3, b3s, &wide)] {
                    let lin = decode_block(u, b, x, 0).unwrap();
                    let got = decode_block(s, b, x, 0).unwrap();
                    for k in 0..3 {
                        let code = (f32::from_bits(lin[k]) * 255.0).round() as u8;
                        assert_eq!(got[k], srgb_read(code).to_bits(), "c0 {c0:x} c1 {c1:x} texel {x}");
                    }
                    assert_eq!(got[3], lin[3]);
                }
            }
        }
    }
    // (BC7 mode 6: one subset, 7-bit endpoints and a p-bit each, 4-bit indices; both endpoints
    // the colour c and index 0 everywhere: every texel c)
    let (b7, b7s) = (fmt(98), fmt(99));
    for c in 0..=127u128 {
        for p in 0..=1u128 {
            let mut v: u128 = 1 << 6;
            let mut at = 7;
            for _ in 0..8 {
                v |= c << at;
                at += 7;
            }
            v |= p << at | p << (at + 1);
            let b = v.to_le_bytes();
            let lin = decode_block(b7, &b, 1, 2).unwrap();
            let got = decode_block(b7s, &b, 1, 2).unwrap();
            for k in 0..3 {
                let code = (f32::from_bits(lin[k]) * 255.0).round() as u8;
                assert_eq!(got[k], srgb_read(code).to_bits());
            }
            assert_eq!(got[3], lin[3]);
        }
    }
}

/// CONV.NANREAD: every NaN of a 16-bit float reads with its sign and its mantissa on top of a
/// 32-bit NaN's, every 11- and 10-bit NaN the same; R16_FLOAT reads every 16-bit NaN so, R11G11B10
/// each component's, R32_FLOAT keeps every 32-bit NaN's bits
#[test]
fn nan_reads_keep_their_payload() {
    for (e, m, signed) in [(5u32, 10u32, true), (5, 6, false), (5, 5, false)] {
        for s in 0..=u32::from(signed) {
            for man in 1..(1u32 << m) {
                let h = s << (e + m) | ((1 << e) - 1) << m | man;
                assert_eq!(from_small_float(h, e, m, signed).to_bits(), s << 31 | 0x7f80_0000 | man << (23 - m), "{e} {m} {h:x}");
            }
        }
    }
    let r16 = fmt(54);
    for h in 0..=0xffffu32 {
        if h & 0x7c00 == 0x7c00 && h & 0x3ff != 0 {
            assert_eq!(decode(r16, &(h as u16).to_le_bytes()).unwrap()[0], (h >> 15) << 31 | 0x7f80_0000 | (h & 0x3ff) << 13);
        }
    }
    let r11 = fmt(26);
    for man in 1..64u32 {
        let w = 0x7c0 | man | (0x7c0 | man) << 11 | (0x3e0 | man >> 1) << 22;
        let got = decode(r11, &w.to_le_bytes()).unwrap();
        assert_eq!(got[..3], [0x7f80_0000 | man << 17, 0x7f80_0000 | man << 17, if man >> 1 == 0 { got[2] } else { 0x7f80_0000 | (man >> 1) << 18 }]);
    }
    let r32 = fmt(41);
    for s in 0..2u32 {
        for man in 1..(1u32 << 23) {
            let v = s << 31 | 0x7f80_0000 | man;
            assert_eq!(decode(r32, &v.to_le_bytes()).unwrap()[0], v);
        }
    }
}

/// CONV.F32: every 32-bit float is stored as it is, a denormal as the zero of its sign; R32_FLOAT
/// stores so every float whose low 12 bits are 0 (the bytes of the element)
#[test]
fn float32_is_stored_denormals_flushed() {
    assert_eq!(all_u32(|v| {
        let f = f32::from_bits(v);
        f32_store(f) == if f.is_subnormal() { v & 0x8000_0000 } else { v }
    }), None);
    let r32 = fmt(41);
    par(1 << 20, |h| {
        let v = h << 12;
        assert_eq!(encode(r32, [f32::from_bits(v), 0.0, 0.0, 0.0]).unwrap(), f32_store(f32::from_bits(v)).to_le_bytes());
    });
}

/// CONV.STENCIL: X24_TYPELESS_G8_UINT reads the stencil (the top byte) as green, red and blue 0,
/// alpha 1, for every element (every depth and stencil)
#[test]
fn stencil_reads_as_green() {
    let f = fmt(47);
    assert_eq!(all_u32(|v| decode(f, &v.to_le_bytes()).ok() == Some([0, v >> 24, 0, 1])), None);
}

/// CONV.INTCLEAR: a float clear of an 8, 16 or 32-bit UINT or SINT: truncated toward zero (the
/// exact value from the float's bits), clamped to the type, NaN 0, for every float; the formats
/// R8, R16, R32 UINT and SINT store it so for every float whose low 12 bits are 0
#[test]
fn int_clears_truncate_and_clamp() {
    assert_eq!(all_u32(|v| {
        let f = f32::from_bits(v);
        let t = trunc_exact(v);
        [8u32, 16, 32].iter().all(|&n| {
            [false, true].iter().all(|&signed| {
                let (lo, hi) = if signed { (-(1i128 << (n - 1)), (1i128 << (n - 1)) - 1) } else { (0, (1i128 << n) - 1) };
                let want = match t {
                    _ if f.is_nan() => 0,
                    None => if f > 0.0 { hi } else { lo },
                    Some(t) => t.clamp(lo, hi),
                };
                int_clear(f, n, signed) == (want as u64) & ((1u64 << n) - 1)
            })
        })
    }), None);
    for (id, n, signed) in [(62u32, 8u32, false), (64, 8, true), (57, 16, false), (59, 16, true), (42, 32, false), (43, 32, true)] {
        let f = fmt(id);
        par(1 << 20, |h| {
            let v = f32::from_bits(h << 12);
            let want = int_clear(v, n, signed).to_le_bytes();
            assert_eq!(encode(f, [v, 0.0, 0.0, 0.0]).unwrap(), want[..(n / 8) as usize], "{id} {v}");
        });
    }
}

/// CONV.X8: B8G8R8X8_UNORM (88) is written as B8G8R8A8_UNORM, the X byte the alpha;
/// B8G8R8X8_UNORM_SRGB (93) as B8G8R8A8_UNORM_SRGB with the X byte 0; reads give B8G8R8A8's colour
/// and alpha 1: every colour (b, g, r) with each of the X bytes 0 and 0xff read; every float whose
/// low 12 bits are 0 written as alpha and red, and as green
#[test]
fn x8_takes_alpha_or_zero() {
    let (f87, f88, f91, f93) = (fmt(87), fmt(88), fmt(91), fmt(93));
    let one = 1.0f32.to_bits();
    par(1 << 24, |c| {
        for x in [0u32, 0xff] {
            let b = (c | x << 24).to_le_bytes();
            let (a, s) = (decode(f87, &b).unwrap(), decode(f91, &b).unwrap());
            assert_eq!(decode(f88, &b).unwrap(), [a[0], a[1], a[2], one]);
            assert_eq!(decode(f93, &b).unwrap(), [s[0], s[1], s[2], one]);
        }
    });
    par(1 << 20, |h| {
        let v = f32::from_bits(h << 12);
        for c in [[v, 0.5, 0.25, v], [0.5, v, 1.0, 0.75]] {
            assert_eq!(encode(f88, c).unwrap(), encode(f87, c).unwrap());
            let mut s = encode(f91, c).unwrap();
            s[3] = 0;
            assert_eq!(encode(f93, c).unwrap(), s);
        }
    });
}

/// 5 or 6-bit endpoint channel widened to 8 bits by repeating its top bits
fn widen(v: u32, n: u32) -> u32 {
    v << (8 - n) | v >> (2 * n - 8)
}

/// CONV.BLOCK (BC1): for every pair of endpoints (c0, c1), every index, both colour modes: the
/// endpoints bit-replicated to 8 bits; in four colour mode (always in BC2 and BC3, in BC1 when c0
/// > c1) index 2 and 3 the thirds (171 a + 85 b) >> 8 and (85 a + 171 b) >> 8; in three colour mode
/// index 2 the half (a + b) >> 1, index 3 black and transparent; BC1_UNORM reads each texel's
/// codes over 255 (every c0 with c1 0, c0, its complement and 0xffff, the four indices)
#[test]
fn bc1_colours_are_replicated_thirds_and_halves() {
    // (bit replication keeps 0 and the largest code: 0 and 255)
    for n in [5u32, 6] {
        assert_eq!((widen(0, n), widen((1 << n) - 1, n)), (0, 255));
    }
    assert_eq!(all_u32(|w| {
        let (c0, c1) = (w >> 16, w & 0xffff);
        let e = |c: u32| [widen(c >> 11, 5), widen(c >> 5 & 63, 6), widen(c & 31, 5)];
        let (a, b) = (e(c0), e(c1));
        [false, true].iter().all(|&four| {
            let four_mode = four || c0 > c1;
            (0..4).all(|idx| {
                let want = match idx {
                    0 => Some(a),
                    1 => Some(b),
                    2 if four_mode => Some([0, 1, 2].map(|k| (171 * a[k] + 85 * b[k]) >> 8)),
                    3 if four_mode => Some([0, 1, 2].map(|k| (85 * a[k] + 171 * b[k]) >> 8)),
                    2 => Some([0, 1, 2].map(|k| (a[k] + b[k]) >> 1)),
                    _ => None,
                };
                bc1_codes(c0, c1, idx, four) == want
            })
        })
    }), None);
    let b1 = fmt(71);
    for c0 in 0..=0xffffu32 {
        for c1 in [0, c0, !c0 & 0xffff, 0xffff] {
            let mut blk = [0u8; 8];
            blk[0..2].copy_from_slice(&(c0 as u16).to_le_bytes());
            blk[2..4].copy_from_slice(&(c1 as u16).to_le_bytes());
            blk[4] = 0xe4;
            for x in 0..4 {
                let got = decode_block(b1, &blk, x, 0).unwrap();
                let want = match bc1_codes(c0, c1, x, false) {
                    Some(c) => [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0],
                    None => [0.0; 4],
                };
                assert_eq!(got, want.map(f32::to_bits), "c0 {c0:x} c1 {c1:x} texel {x}");
            }
        }
    }
}

/// the index bits of a BC3 alpha or BC4 channel block with index k at texel k (texels 0..7)
fn ramp_block(e0: u8, e1: u8) -> [u8; 8] {
    let bits: u64 = (0..8u64).fold(0, |v, k| v | k << (3 * k));
    let b = bits.to_le_bytes();
    [e0, e1, b[0], b[1], b[2], b[3], b[4], b[5]]
}

/// CONV.BLOCK (BC2, BC3, BC4, BC5): BC2's alpha a 4-bit code over 15 (every code at every texel);
/// BC3's alpha steps for every pair of endpoints and every index: the nearest 8-bit code to the
/// exact step (sevenths between when a0 > a1, else fifths and 0 and 255); BC4 and BC5 UNORM: the
/// endpoints times 257, the step in 16 bits truncated, read over 65535; BC3, BC4 and BC5 read so
/// at each texel (every pair of endpoints, the eight indices at texels 0..7)
#[test]
fn bc2_to_bc5_alpha_and_channels() {
    let b2 = fmt(74);
    for t in 0..16u32 {
        for a in 0..16u8 {
            let mut blk = [0u8; 16];
            blk[(t / 2) as usize] = a << (4 * (t % 2));
            let got = decode_block(b2, &blk, t % 4, t / 4).unwrap();
            assert_eq!(got[3], (f32::from(a) / 15.0).to_bits());
        }
    }
    let (b3, b4, b5) = (fmt(77), fmt(80), fmt(83));
    for a0 in 0..=255u32 {
        for a1 in 0..=255u32 {
            for k in 0..8u32 {
                // (the step as n / d)
                let (n, d) = match k {
                    0 => (a0, 1),
                    1 => (a1, 1),
                    _ if a0 > a1 => ((8 - k) * a0 + (k - 1) * a1, 7),
                    6 => (0, 1),
                    7 => (255, 1),
                    _ => ((6 - k) * a0 + (k - 1) * a1, 5),
                };
                let near = bc3_alpha(a0, a1, k);
                assert!((2 * near * d).abs_diff(2 * n) <= d, "{a0} {a1} {k}");
                assert!(near <= 255);
                let (n16, d16) = (n * 257, d);
                assert_eq!(bc4_code(a0, a1, k), n16 / d16, "{a0} {a1} {k}");
            }
            let blk = ramp_block(a0 as u8, a1 as u8);
            let mut wide = [0u8; 16];
            wide[..8].copy_from_slice(&blk);
            wide[8..].copy_from_slice(&blk);
            for k in 0..8u32 {
                let (x, y) = (k % 4, k / 4);
                assert_eq!(decode_block(b3, &wide, x, y).unwrap()[3], (bc3_alpha(a0, a1, k) as f32 / 255.0).to_bits());
                let c = (bc4_code(a0, a1, k) as f32 / 65535.0).to_bits();
                assert_eq!(decode_block(b4, &blk, x, y).unwrap()[0], c);
                assert_eq!(decode_block(b5, &wide, x, y).unwrap()[..2], [c, c]);
            }
        }
    }
}

/// CONV.BC4SNORM: for every pair of signed endpoints and every index: the endpoints at least
/// -127; the step's exact value over 127, times 32767, truncated toward zero to a 16-bit code
/// (sevenths between when r0 > r1, else fifths and -1 and 1); BC4_SNORM and BC5_SNORM read it times
/// the float 1/32767 at each texel
#[test]
fn bc4_snorm_steps_truncate() {
    let (b4, b5) = (fmt(81), fmt(84));
    for r0 in i8::MIN..=i8::MAX {
        for r1 in i8::MIN..=i8::MAX {
            let (a0, a1) = (i128::from(r0.max(-127)), i128::from(r1.max(-127)));
            for k in 0..8i128 {
                let (n, d) = match k {
                    0 => (a0, 1),
                    1 => (a1, 1),
                    _ if r0 > r1 => ((8 - k) * a0 + (k - 1) * a1, 7),
                    6 => (-127, 1),
                    7 => (127, 1),
                    _ => ((6 - k) * a0 + (k - 1) * a1, 5),
                };
                // (trunc toward zero of n 32767 / (127 d), by magnitudes)
                let q = (n.abs() * 32767) / (127 * d);
                let want = if n < 0 { -q } else { q };
                assert_eq!(i128::from(bc4_snorm_code(r0, r1, k as i64)), want, "{r0} {r1} {k}");
            }
            let blk = ramp_block(r0 as u8, r1 as u8);
            let mut wide = [0u8; 16];
            wide[..8].copy_from_slice(&blk);
            wide[8..].copy_from_slice(&blk);
            for k in 0..8u32 {
                let c = (f32::from(bc4_snorm_code(r0, r1, i64::from(k))) * (1.0f32 / 32767.0)).to_bits();
                assert_eq!(decode_block(b4, &blk, k % 4, k / 4).unwrap()[0], c);
                assert_eq!(decode_block(b5, &wide, k % 4, k / 4).unwrap()[..2], [c, c]);
            }
        }
    }
}

/// CONV.BC6H: the mode bits: two bits, five when the low two are 2 or 3; the 14 modes the
/// format page lists, the four others (0x13, 0x17, 0x1b, 0x1f) reserved and read as zeros; each
/// mode's fields fill the block: every endpoint and delta bit read once, at the page's precisions,
/// the partition's 5 bits in two-region modes, then the indices (46 or 63 bits) to bit 128; the
/// endpoint unquantization and the 31/64 (31/32 signed) scale for every value; the 3 and 4-bit
/// weights and the step reaching both endpoints for every pair of 16-bit endpoints
#[test]
fn bc6h_modes_fill_the_block() {
    // (Microsoft Learn "BC6H Format": the modes' bits, endpoint precision, deltas r g b)
    const PAGE: [(u32, u32, [u32; 3]); 14] = [
        (0b00, 10, [5, 5, 5]), (0b01, 7, [6, 6, 6]), (0b00010, 11, [5, 4, 4]), (0b00110, 11, [4, 5, 4]),
        (0b01010, 11, [4, 4, 5]), (0b01110, 9, [5, 5, 5]), (0b10010, 8, [6, 5, 5]), (0b10110, 8, [5, 6, 5]),
        (0b11010, 8, [5, 5, 6]), (0b11110, 6, [6, 6, 6]), (0b00011, 10, [10, 10, 10]), (0b00111, 11, [9, 9, 9]),
        (0b01011, 12, [8, 8, 8]), (0b01111, 16, [4, 4, 4]),
    ];
    for m in 0..32u32 {
        let bits = if m & 3 < 2 { m & 3 } else { m };
        let want = PAGE.iter().position(|p| p.0 == bits);
        assert_eq!(bc6h_mode(bits), want, "{m:05b}");
        if want.is_none() {
            assert!([0x13, 0x17, 0x1b, 0x1f].contains(&m));
            for rest in [0u128, !0, 0x5555_5555_5555_5555_5555_5555_5555_5555] {
                let b = (rest << 5 | u128::from(m)).to_le_bytes();
                for signed in [false, true] {
                    assert_eq!(bptc::bc6h(&b, signed, 1, 3), [0; 3]);
                }
            }
        }
    }
    for (i, (bits, reads)) in BC6H_MODES.iter().enumerate() {
        let (pb, p, d) = PAGE[i];
        assert_eq!(*bits, pb);
        assert_eq!(BC6H_BITS[0][i], p);
        assert_eq!([BC6H_BITS[1][i], BC6H_BITS[2][i], BC6H_BITS[3][i]], d);
        let one = i >= 10;
        // (each target's bits: r0..r3 0..3, g 4..7, b 8..11, the partition 12)
        let mut seen = [0u32; 13];
        let mut total = if *bits < 2 { 2 } else { 5 };
        for &(t, n, sh, _) in *reads {
            let mask = ((1u32 << n) - 1) << sh;
            assert_eq!(seen[t as usize] & mask, 0, "mode {i} target {t} bit read twice");
            seen[t as usize] |= mask;
            total += u32::from(n);
        }
        for c in 0..3 {
            assert_eq!(seen[c * 4], (1 << p) - 1, "mode {i} endpoint {c}");
            for k in 1..(if one { 2 } else { 4 }) {
                assert_eq!(seen[c * 4 + k], (1 << d[c]) - 1, "mode {i} delta {c} {k}");
            }
            if one {
                assert_eq!(seen[c * 4 + 2] | seen[c * 4 + 3], 0);
            }
        }
        assert_eq!(seen[12], if one { 0 } else { 31 });
        assert_eq!(total + if one { 63 } else { 46 }, 128, "mode {i}");
    }
    for bits in 6..=16u32 {
        for v in 0..(1i32 << bits) {
            let max = (1i32 << bits) - 1;
            let want = if bits >= 15 { v } else if v == 0 { 0 } else if v == max { 0xffff } else { ((i64::from(v) * 65536 + 32768) >> bits) as i32 };
            assert_eq!(unquantize(v, bits, false), want, "{bits} {v}");
        }
        for v in -(1i32 << (bits - 1))..(1i32 << (bits - 1)) {
            let a = i64::from(v).abs();
            let u = if bits >= 16 { a } else if a == 0 { 0 } else if a >= (1 << (bits - 1)) - 1 { 0x7fff } else { (a * 32768 + 16384) >> (bits - 1) };
            assert_eq!(i64::from(unquantize(v, bits, true)), if v < 0 { -u } else { u }, "{bits} {v}");
        }
    }
    for v in 0..=0xffffi32 {
        assert_eq!(u32::from(finish(v, false)), (v as u32 * 31) / 64);
    }
    for v in -0x7fffi32..=0x7fff {
        let a = (v.unsigned_abs() * 31) / 32;
        assert_eq!(u32::from(finish(v, true)), if v < 0 { 0x8000 | a } else { a }, "{v}");
    }
    assert_eq!(W3, [0, 9, 18, 27, 37, 46, 55, 64]);
    assert_eq!(W4, [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64]);
    assert_eq!(all_u32(|w| {
        let (a, b) = ((w >> 16) as i32, (w & 0xffff) as i32);
        lerp(a, b, &W3, 0) == a && lerp(a, b, &W3, 7) == b && lerp(a, b, &W4, 0) == a && lerp(a, b, &W4, 15) == b
            && lerp(-a, -b, &W4, 0) == -a && lerp(-a, -b, &W4, 15) == -b
    }), None);
}

/// CONV.BC7: the mode is the first byte's lowest set bit, a 0 byte the reserved mode read as
/// zeros (every first byte); each mode's fields fill the block as the format page lists them
/// (subsets, partition bits, rotation and index selection, endpoint colour and alpha bits,
/// p-bits, the indices with one bit less at each anchor) to bit 128; the partition tables' subsets
/// and anchors; the weights (64 - w) a + w b + 32 >> 6 of every pair of 8-bit endpoints stay in
/// 0..255 and reach both endpoints
#[test]
fn bc7_modes_fill_the_block() {
    for b0 in 0..=255u8 {
        let want = (0..8).find(|&k| b0 >> k & 1 == 1);
        assert_eq!(bc7_mode(b0), want);
    }
    for rest in [0u128, !0, 0x5555_5555_5555_5555_5555_5555_5555_5555] {
        let b = (rest << 8).to_le_bytes();
        for (x, y) in [(0, 0), (3, 3), (1, 2)] {
            assert_eq!(bptc::bc7(&b, x, y), [0; 4]);
        }
    }
    // (Microsoft Learn "BC7 Format": subsets, partition bits, rotation bits, index selection
    // bit, colour bits, alpha bits, p-bits per endpoint (mode 1: per subset), index bits,
    // second index bits)
    const PAGE: [(u32, u32, u32, u32, u32, u32, u32, u32, u32); 8] = [
        (3, 4, 0, 0, 4, 0, 1, 3, 0), (2, 6, 0, 0, 6, 0, 1, 3, 0), (3, 6, 0, 0, 5, 0, 0, 2, 0), (2, 6, 0, 0, 7, 0, 1, 2, 0),
        (1, 0, 2, 1, 5, 6, 0, 2, 3), (1, 0, 2, 0, 7, 8, 0, 2, 2), (1, 0, 0, 0, 7, 7, 1, 4, 0), (2, 6, 0, 0, 5, 5, 1, 2, 0),
    ];
    for (m, &(subsets, part, rot, isel, rgb, alpha, pbit, ib, ib2)) in PAGE.iter().enumerate() {
        assert_eq!((BC7_RGB[m], BC7_ALPHA[m], BC7_PBITS >> m & 1), (rgb, alpha, pbit), "mode {m}");
        let ends = 2 * subsets;
        let pbits = if pbit == 0 { 0 } else if m == 1 { 2 } else { ends };
        let index = 16 * ib - subsets + if ib2 > 0 { 16 * ib2 - 1 } else { 0 };
        let total = (m as u32 + 1) + part + rot + isel + ends * (3 * rgb + alpha) + pbits + index;
        assert_eq!(total, 128, "mode {m}");
    }
    for (t, parts) in [(&PART2[..], 2u8), (&PART3[..], 3u8)] {
        for row in t {
            assert_eq!(row[0], 128, "texel 0 is subset 0's anchor");
            let anchors = row.iter().filter(|&&v| v & 128 != 0).count();
            assert_eq!(anchors, parts as usize);
            for s in 0..parts {
                assert_eq!(row.iter().filter(|&&v| v & 128 != 0 && v & 3 == s).count(), 1, "one anchor of subset {s}");
            }
            assert!(row.iter().all(|&v| v & 3 < parts && v & !(128 | 3) == 0));
        }
    }
    assert_eq!(W2, [0, 21, 43, 64]);
    for a in 0..=255i32 {
        for b in 0..=255i32 {
            for w in [&W2[..], &W3[..], &W4[..]] {
                for i in 0..w.len() {
                    let v = lerp(a, b, w, i);
                    assert_eq!(v, ((64 - w[i]) * a + w[i] * b + 32) >> 6);
                    assert!((0..=255).contains(&v) && v >= a.min(b) && v <= a.max(b));
                }
                assert_eq!((lerp(a, b, w, 0), lerp(a, b, w, w.len() - 1)), (a, b));
            }
        }
    }
}

/// D3D12.CLEAR.X, D3D12.CLEAR.XALPHA: B8G8R8X8_UNORM's X byte (bits 24..31) is the alpha as an
/// 8-bit UNORM, B8G8R8X8_UNORM_SRGB's X byte is unused (0); a clear of either writes the colour as
/// B8G8R8A8 (UNORM, UNORM_SRGB) does, and the X byte so, for every float whose low 12 bits are 0
/// as the alpha (and as each colour)
#[test]
fn d3d12_x_byte_is_alpha_or_zero() {
    let x = |f: u32| channels(f).unwrap().into_iter().find(|c| c.at == 24).unwrap();
    let (a, z) = (x(88), x(93));
    assert!(matches!(a.enc, Enc::Alpha) && a.bits == 8 && a.src == 3);
    assert!(matches!(z.enc, Enc::Unused) && z.bits == 8);
    let one = 1.0f32.to_bits();
    par(1 << 20, |h| {
        let v = h << 12;
        for c in [[one, 0, one >> 1, v], [v, v, v, v]] {
            let (t88, t87) = (clear_texel(88, c).unwrap(), clear_texel(87, c).unwrap());
            assert_eq!(t88, t87);
            assert_eq!(u64::from(t88[3]), d3d12::to_unorm(f32::from_bits(clear_input(v)), 8));
            let (t93, mut t91) = (clear_texel(93, c).unwrap(), clear_texel(91, c).unwrap());
            t91[3] = 0;
            assert_eq!(t93, t91);
        }
    });
}

/// D3D12.CLEARDSV, D3D12.CLEARDSV.NAN: for every depth float a UNORM depth (D16, D24) is the
/// clear's input (denormals 0) clamped to 0..1 as an n-bit UNORM, NaN 0; D16_UNORM and
/// D24_UNORM_S8_UINT clear plane 0 to it (the 24 bits in the low bits of 4 bytes), D32_FLOAT and
/// D32_FLOAT_S8X24_UINT keep the float as given, not clamped; the stencil plane the stencil's
/// low byte (every float whose low 12 bits are 0, its top 8 bits below them, with stencils of
/// every byte and high bits)
#[test]
fn depth_clears_clamp_unorm_and_keep_floats() {
    assert_eq!(all_u32(|v| {
        let f = f32::from_bits(clear_input(v));
        let d = if f.is_nan() { 0.0 } else { f.clamp(0.0, 1.0) };
        unorm_depth(v, 16) == d3d12::to_unorm(d, 16) && unorm_depth(v, 24) == d3d12::to_unorm(d, 24) && (!f.is_nan() || unorm_depth(v, 24) == 0 && unorm_depth(v, 16) == 0)
    }), None);
    par(1 << 20, |h| {
        let v = h << 12 | h >> 12;
        let s = h.rotate_left(13);
        let u16v = unorm_depth(v, 16) as u16;
        let u24 = unorm_depth(v, 24) as u32;
        assert!(u24 < 1 << 24);
        let raw = clear_input(v).to_le_bytes().to_vec();
        assert_eq!(clear_depth(55, v, s).unwrap(), vec![u16v.to_le_bytes().to_vec()]);
        assert_eq!(clear_depth(45, v, s).unwrap(), vec![u24.to_le_bytes().to_vec(), vec![s as u8]]);
        assert_eq!(clear_depth(40, v, s).unwrap(), vec![raw.clone()]);
        assert_eq!(clear_depth(20, v, s).unwrap(), vec![raw, vec![s as u8]]);
    });
    for f in [0u32, 1, 2, 41, 44, 46, 56, 0xffff] {
        if ![40, 55, 45, 20].contains(&f) {
            assert_eq!(clear_depth(f, 0, 0), None);
        }
    }
}
