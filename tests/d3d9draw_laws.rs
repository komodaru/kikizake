//! Laws of the Direct3D 9 / 8 drawing and D3DX 9 models (exhaustive over small domains,
//! property-based elsewhere): what the documentation states as properties, whatever Windows'
//! measured details turn out to be.
mod pairs;
use pairs::zip_eq;
use kikizake::case::parse_cases;
use kikizake::model::d3d9draw::*;
use kikizake::model::d3dx9::{decl_fvf, decl_size, fvf_decl, fvf_size};
use kikizake::model::run_case;
use proptest::prelude::*;

/// every finite half survives the trip through f64 and back; the trip back from f32 rounds to
/// the nearest half (ties to even)
#[test]
fn halves_round_trip() {
    for h in 0..=0xffffu16 {
        let v = f16_to_f64(h);
        if v.is_nan() {
            continue;
        }
        assert_eq!(f32_to_f16(v as f32), h, "{h:#06x}");
    }
}

proptest! {
    #[test]
    fn f32_to_f16_is_nearest(bits: u32) {
        let x = f32::from_bits(bits);
        prop_assume!(x.is_finite() && x.abs() < 65504.0);
        let h = f32_to_f16(x);
        let d = (f16_to_f64(h) - x as f64).abs();
        // no other half is nearer
        for o in [h.wrapping_add(1), h.wrapping_sub(1), h ^ 0x8000] {
            let v = f16_to_f64(o);
            if v.is_finite() {
                prop_assert!(d <= (v - x as f64).abs(), "{x} -> {h:#06x}, {o:#06x} is nearer");
            }
        }
    }
}

/// every value of every unsigned and signed channel of every known format decodes and encodes
/// back to itself
#[test]
fn channels_round_trip() {
    for &(f, name) in TARGET_FORMATS {
        let fm = format(f).unwrap();
        for c in &fm.ch {
            if fm.kind != Kind::Int || c.bits > 16 || c.c == b'x' {
                continue;
            }
            for raw in 0..1u64 << c.bits {
                // (signed: the most negative value is -1 as the next one is, and comes back as it)
                if matches!(c.c, b'u' | b'v' | b'w' | b'q') && raw == 1 << (c.bits - 1) {
                    continue;
                }
                let mut bytes = vec![0u8; fm.bytes];
                let w = (raw as u128) << c.shift;
                bytes.copy_from_slice(&w.to_le_bytes()[..fm.bytes]);
                let rgba = to_rgba(f, &decode(&fm, &bytes));
                let back = encode(&fm, rgba);
                let get = |b: &[u8]| {
                    let mut x = [0u8; 16];
                    x[..b.len()].copy_from_slice(b);
                    (u128::from_le_bytes(x) >> c.shift) as u64 & ((1 << c.bits) - 1)
                };
                // (luminance comes back through red: L8 and A8L8 alike)
                assert_eq!(get(&back), raw, "{name} channel {} value {raw}", c.c as char);
            }
        }
    }
}

proptest! {
    /// stencil operations: INVERT is an involution, INCR / DECR wrap and undo each other,
    /// the saturating ones stay within 0..255 and move at most one
    #[test]
    fn stencil_ops(s: u8, r: u8) {
        prop_assert_eq!(stencil_op(6, stencil_op(6, s, r), r), s);
        prop_assert_eq!(stencil_op(8, stencil_op(7, s, r), r), s);
        prop_assert_eq!(stencil_op(1, s, r), s);
        prop_assert_eq!(stencil_op(3, s, r), r);
        prop_assert!((stencil_op(4, s, r) as i32 - s as i32).abs() <= 1 && stencil_op(4, s, r) >= s);
        prop_assert!(stencil_op(5, s, r) <= s);
    }

    /// comparisons: NEVER and ALWAYS, and each with its complement
    #[test]
    fn comparisons(a: u64, b: u64) {
        prop_assert!(!cmp(1, a, b) && cmp(8, a, b));
        for (f, g) in [(2, 7), (3, 6), (4, 5)] {
            prop_assert_ne!(cmp(f, a, b), cmp(g, a, b));
        }
    }

    /// texture addressing: inside the texture every mode is the identity; WRAP has the period
    /// n, MIRROR 2n; every mode but BORDER lands inside
    #[test]
    fn addressing(i in -1000i64..1000, n in 1i64..64) {
        for m in 1..=5 {
            match address(m, i, n).unwrap() {
                Some(k) => prop_assert!((0..n).contains(&k)),
                None => prop_assert!(m == 4 && !(0..n).contains(&i)),
            }
            if (0..n).contains(&i) {
                prop_assert_eq!(address(m, i, n).unwrap(), Some(i));
            }
        }
        prop_assert_eq!(address(1, i, n).unwrap(), address(1, i + n, n).unwrap());
        prop_assert_eq!(address(2, i, n).unwrap(), address(2, i + 2 * n, n).unwrap());
        prop_assert_eq!(address(2, i, n).unwrap(), address(2, -1 - i, n).unwrap());
    }
}

fn picture(text: &str) -> Vec<String> {
    let c = &parse_cases(text)[0];
    run_case(c).unwrap_or_else(|e| panic!("{e:?}"))
}

fn tri_line(p: [(i64, i64); 3], c: u32) -> String {
    let w = |v: i64| format!("{:#x}", (v as f32 / 16.0).to_bits());
    let mut s = format!("d9fvf 0x44\nd9draw 4 3");
    for (x, y) in p {
        s += &format!(" {} {} 0 0x3f800000 {c:#x}", w(x), w(y));
    }
    s
}

/// pixels of an A8R8G8B8 picture that are not black
fn covered(lines: &[String]) -> Vec<Vec<u32>> {
    lines.iter().skip_while(|l| !l.starts_with("px rt")).skip(1).map(|r| (0..r.len() / 8).map(|i| u32::from_str_radix(&r[8 * i..8 * i + 8], 16).unwrap()).collect()).collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]
    /// the fill convention partitions: two triangles splitting any quadrilateral along a
    /// diagonal draw every pixel of it once, never twice (blended additively: a pixel drawn by
    /// both would hold their sum); and either winding draws the same pixels with culling off
    #[test]
    fn shared_edges_draw_once(p in proptest::array::uniform4((-8i64..=248, -8i64..=248)), flip: bool) {
        // (inside the target: a clipped triangle's new vertices are rounded, D3D9.RASTER.CLIPTIE)
        let [a, b, c, d] = p;
        // (convex quads only: a, b, c, d in order around)
        let cross = |o: (i64, i64), p: (i64, i64), q: (i64, i64)| (p.0 - o.0) * (q.1 - o.1) - (p.1 - o.1) * (q.0 - o.0);
        let s = [cross(a, b, c), cross(b, c, d), cross(c, d, a), cross(d, a, b)];
        prop_assume!(s.iter().all(|&x| x > 0) || s.iter().all(|&x| x < 0));
        let head = "case t\ndevice swvp\nd9target 16 16 21\nd9clear 1 0 0 0\nrs set 22 1\nrs set 27 1\nrs set 19 2\nrs set 20 2\ntss set 0 1 3\ntss set 0 4 3\n";
        let (t1, t2) = if flip { ([a, b, d], [b, c, d]) } else { ([a, b, c], [a, c, d]) };
        let two = picture(&format!("{head}{}\n{}\nd9rt\nend\n", tri_line(t1, 0x01010101), tri_line(t2, 0x01010101)));
        for row in covered(&two) {
            for px in row {
                prop_assert!(px == 0 || px == 0x01010101, "a pixel drawn twice: {px:08x}");
            }
        }
        let fwd = picture(&format!("{head}{}\nd9rt\nend\n", tri_line(t1, 0xffffffff)));
        let rev = picture(&format!("{head}{}\nd9rt\nend\n", tri_line([t1[0], t1[2], t1[1]], 0xffffffff)));
        prop_assert_eq!(fwd, rev);
    }
}

/// D3DX: a vertex's size from its FVF is the size of the declaration made from that FVF
#[test]
fn fvf_size_is_the_declarations() {
    let mut n = 0;
    for pos in [0x002u32, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e] {
        for parts in 0..16u32 {
            for tex in 0..=8u32 {
                for sizes in [0u32, 0xffff, 0x5555, 0xaaaa, 0x1b1b] {
                    for beta in [0, 0x1000, 0x8000] {
                        let f = pos | parts << 4 | tex << 8 | (sizes & ((1 << (2 * tex)) - 1)) << 16 | beta;
                        if let Ok(e) = fvf_decl(f) {
                            assert_eq!(decl_size(&e), fvf_size(f), "{f:#x}");
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 1000);
}

/// D3DX: the declaration made from an FVF gives that FVF back, for every FVF of one position
/// kind, the optional parts and up to 8 texture coordinate sets of any sizes
#[test]
fn fvf_decl_round_trips() {
    let mut n = 0;
    for pos in [0x002u32, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e] {
        for parts in 0..16u32 {
            for tex in 0..=8u32 {
                for sizes in [0u32, 0xffff, 0x5555, 0xaaaa, 0x1b1b, 0xe4e4] {
                    for beta in [0, 0x1000, 0x8000] {
                        let f = pos | parts << 4 | tex << 8 | (sizes & ((1u64 << (2 * tex)) - 1) as u32) << 16 | beta;
                        if let Ok(e) = fvf_decl(f) {
                            // (the indices' type is not in an FVF without weights' room: XYZ and
                            // XYZRHW drop LASTBETA)
                            let want = if pos < 6 { f & !0x9000 } else { f };
                            assert_eq!(decl_fvf(&e), Ok(want), "{f:#x}");
                            n += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(n > 1000);
}

proptest! {
    /// D3DX's float to half conversions agree with IEEE's (round to nearest even) on normal
    /// halves; the 8 at a time ones only differ at exact ties, by one toward zero; x64's 4 at
    /// a time one is IEEE's there, and its one by one one too but in the top 2^-12 of a
    /// binade, where 8193 times the value has the next exponent and it may come out one less
    /// (0x3b7ff9ab: 0x1bff, IEEE 0x1c00; counted over every normal float: 29696 of them)
    #[test]
    fn d3dx_halves(m in 0x3880_0000u32..0x477f_f000, sign: bool) {
        use kikizake::model::d3dx9::{f16_one, f16_sse, f16_x64, f16_x64_tail};
        let bits = m | (sign as u32) << 31;
        let x = f32::from_bits(bits);
        let ieee = f32_to_f16(x);
        prop_assert_eq!(f16_one(bits), ieee);
        prop_assert_eq!(f16_x64(bits), ieee);
        let tail = f16_x64_tail(bits);
        prop_assert!(tail == ieee || (m & 0x7f_ffff >= 0x7f_f800 && tail == ieee - 1), "{bits:#x}: {tail:#x} {ieee:#x}");
        let tie = bits & 0x1fff == 0x1000;
        prop_assert_eq!(f16_sse(bits), if tie && bits & 0x2000 != 0 { ieee - 1 } else { ieee });
    }

    /// below 2^-14: x64's one by one conversion never gives more than its 4 at a time one
    /// (it rounds at a normal half's bit and then truncates); the 8 at a time one is the
    /// magnitude + 2^-14 rounded in f32 (to a multiple of 2^-37), then ties toward zero at 2^-24:
    /// exactly IEEE's half of that f32 sum's excess, ties down
    #[test]
    fn d3dx_denormal_halves(m in 0u32..0x3880_0000, sign: bool) {
        use kikizake::model::d3dx9::{f16_sse, f16_x64, f16_x64_tail};
        let bits = m | (sign as u32) << 31;
        let (tail, four) = (f16_x64_tail(bits) & 0x7fff, f16_x64(bits) & 0x7fff);
        prop_assert!(tail <= four && four - tail <= 1, "{bits:#x}: {tail:#x} {four:#x}");
        let a = f32::from_bits(m) as f64;
        let r = ((f32::from_bits(m) + f32::from_bits(0x3880_0000)) as f64 - 2f64.powi(-14)) * 2f64.powi(24);
        // (ties toward zero: the half below a tie)
        let want = if r.fract() == 0.5 { r.floor() } else { r.round() } as u16;
        prop_assert_eq!(f16_sse(bits) & 0x7fff, want, "{:#x} ({})", bits, a);
        prop_assert_eq!(f16_sse(bits) & 0x8000, (bits >> 16 & 0x8000) as u16);
    }
}

/// the 8 at a time conversion of tiny values rounds the f32 sum |x| + 2^-14 first, keeping what
/// a shift of the mantissa throws away (the 2026-11-21 review: x86 was modeled with the shift,
/// golden d3dx9-32 f16-ties said 0001 0001 0002 0003 0004 0005 0009 0010 to its first 8);
/// the four values the clean-room notes name come out one more than a shift's
#[test]
fn tiny_halves_round_the_sum_first() {
    use kikizake::model::d3dx9::f16_sse;
    let x = [0x3300_0401u32, 0x3300_07a0, 0x33c0_0306, 0x3420_0101, 0x3460_01fc, 0x3490_00b1, 0x3508_0041, 0x3578_0075];
    let windows = [1u16, 1, 2, 3, 4, 5, 9, 0x10];
    // (what the shift of the mantissa, plus 0xfff, gave: the model before)
    let shift = [0u16, 0, 1, 2, 3, 4, 8, 0xf];
    for ((&x, &w), &sh) in zip_eq(zip_eq(x.iter(), &windows), &shift) {
        assert_eq!(f16_sse(x), w, "{x:#x}");
        let shifted = ((0x80_0000 | x & 0x7f_ffff) >> (113 - (x >> 23))).wrapping_add(0xfff) >> 13;
        assert_eq!(shifted, u32::from(sh), "{x:#x}");
        assert_eq!(f16_sse(x | 0x8000_0000), w | 0x8000);
    }
}

/// x64's one by one conversion at the top of a binade (the clean-room notes: (8192 v + v) -
/// 8192 v rounds on 8193 v's exponent)
#[test]
fn x64_tail_at_a_binade_top() {
    use kikizake::model::d3dx9::f16_x64_tail;
    assert_eq!(f16_x64_tail(0x3b7f_f9ab), 0x1bff);
    assert_eq!(f16_x64_tail(0x3f80_0000), 0x3c00);
}

#[test]
fn half_midpoint_neighbors() {
    use kikizake::model::d3dx9::f16_sse as f16_block;
    for sign in [0u32, 0x8000_0000] {
        for (base, half) in [(0x3f80_0000, 0x3c00), (0x3f80_2000, 0x3c01)] {
            for (tail, up) in [(0xfff, 0), (0x1000, 0), (0x1001, 1)] {
                assert_eq!(f16_block(sign | base | tail), (sign >> 16) as u16 | (half + up));
            }
        }
    }
}

/// every format WARP takes as a texture or a target (either API) has a decoding the model samples
/// with (texel_kind: its channels, DXT or the palette): none of the formats it takes falls to the
/// model's "bad line"; of all 2^16 low formats and the FOURCCs asked, the YUV ones and those the
/// model does not know are not taken (D3D9.TEX.FORMAT)
#[test]
fn every_taken_format_is_decoded() {
    let all = (0..=0xffffu32).chain(TEXTURE_FORMATS.iter().map(|t| t.0));
    for f in all {
        for v8 in [false, true] {
            for rt in [false, true] {
                if !supported(f, rt, v8) {
                    continue;
                }
                let m = format(f).unwrap_or_else(|| panic!("{f} taken but not decoded"));
                let kind = texel_kind(f, &m).unwrap_or_else(|| panic!("{f} taken but not sampled"));
                assert!(!matches!(m.kind, Kind::Yuv), "{f}");
                match kind {
                    TexelKind::Dxt(n) => assert!(m.kind == Kind::Dxt(n) && (1..=5).contains(&n), "{f}"),
                    TexelKind::Palette => assert!(matches!(f, 40 | 41) && !rt, "{f}"),
                    TexelKind::Channels => assert!(!m.ch.is_empty() && m.ch.iter().all(|c| c.c != b'p'), "{f}"),
                }
            }
        }
    }
    // (the YUV formats have no sampling)
    for s in [b"UYVY", b"YUY2"] {
        let f = fourcc(s);
        assert!(!supported(f, false, false) && !supported(f, false, true));
        assert_eq!(format(f).and_then(|m| texel_kind(f, &m)), None);
    }
}

/// one pretransformed white line on a 16x16 target, as the generators write it
fn free_line(a: (f32, f32), b: (f32, f32)) -> String {
    let v = |p: (f32, f32)| format!("{:#x} {:#x} 0x0 0x3f800000 0xffffffff", p.0.to_bits(), p.1.to_bits());
    format!("case l\ndevice swvp\nd9target 16 16 21\nd9clear 1 0 0 0\nd9fvf 0x44\nd9draw 2 2 {} {}\nd9rt\nend\n", v(a), v(b))
}

/// (D3D9.DRAW.POINTSLINES, review 2026-11-20 1.1) the single line of line-joint-pl5-3, (13, 9.5)
/// to (3.5, 4.25), starts on the corner rows 9 and 10 share: Windows lights (13, 10), not (13, 9)
/// (the model said (13, 9) at 90fcd881); and the translated line-free-corner-2-5 / -0-5 pair,
/// whose rows differ
#[test]
fn free_start_on_a_vertical_corner() {
    let px = |a, b, x: usize, y: usize| {
        let lines = picture(&free_line(a, b));
        let rows = &lines[lines.iter().position(|l| l == "px rt 16 16 32").unwrap() + 1..];
        &rows[y][x * 8..x * 8 + 8] != "00000000"
    };
    assert!(px((13.0, 9.5), (3.5, 4.25), 13, 10) && !px((13.0, 9.5), (3.5, 4.25), 13, 9));
    assert!(px((13.0, 9.5), (3.5, 14.75), 13, 9) && !px((13.0, 9.5), (3.5, 14.75), 13, 10));
    assert!(px((10.0, 7.5), (0.5, 12.75), 10, 8) && !px((10.0, 7.5), (0.5, 12.75), 10, 7));
}

/// the diamond-exit rule (Rasterization Rules, Direct3D 10) in f64 for a line whose distances
/// are all far from the diamonds' bounds (None otherwise): a pixel is lit when the segment meets
/// its diamond and does not end in it; ends in 1/256 pixel
fn exit_rule(a: (i32, i32), b: (i32, i32), w: i32, h: i32) -> Option<Vec<(i32, i32)>> {
    let f = |v: i32| f64::from(v) / 256.0;
    let (x0, y0, x1, y1) = (f(a.0), f(a.1), f(b.0), f(b.1));
    let at = |t: f64| (x0 + t * (x1 - x0), y0 + t * (y1 - y0));
    let mut lit = Vec::new();
    for j in 0..h {
        for i in 0..w {
            let (cx, cy) = (f64::from(i) + 0.5, f64::from(j) + 0.5);
            let d = |p: (f64, f64)| (p.0 - cx).abs() + (p.1 - cy).abs();
            // (the L1 distance along the segment is convex and piecewise linear: its least is at an
            // end or where the segment crosses the center column or row)
            let mut ts = vec![0.0, 1.0];
            if x1 != x0 { ts.push((cx - x0) / (x1 - x0)); }
            if y1 != y0 { ts.push((cy - y0) / (y1 - y0)); }
            let near = ts.into_iter().filter(|t| (0.0..=1.0).contains(t)).map(|t| d(at(t))).fold(f64::INFINITY, f64::min);
            let end = d((x1, y1));
            if (near - 0.5).abs() < 1e-3 || (end - 0.5).abs() < 1e-3 {
                return None;
            }
            if near < 0.5 && end > 0.5 {
                lit.push((i, j));
            }
        }
    }
    Some(lit)
}

proptest! {
    /// (D3D9.DRAW.LINES, D3D9.DRAW.LINEEND) away from ties WARP's f32 line is Direct3D 10's
    /// diamond-exit rule: the pixels whose diamond it meets, the end's own excluded; one pixel per
    /// column (row, y-major)
    #[test]
    fn warp_lines_are_the_diamond_exit_rule(a in (0i32..16 * 256, 0i32..16 * 256), b in (0i32..16 * 256, 0i32..16 * 256)) {
        prop_assume!(a != b);
        let got = kikizake::model::warpline::pixels(a, b, 16, 16);
        if let Some(want) = exit_rule(a, b, 16, 16) {
            // (WARP's own: the end's pixel, when the end is in its diamond, is lit if the line
            // crosses the pixel's other center line inside it before the end; lines-end-minor-*)
            let own = ((b.0 + 255) / 256 - 1, (b.1 + 255) / 256 - 1);
            let but = |v: &Vec<(i32, i32)>| v.iter().copied().filter(|&p| p != own).collect::<Vec<_>>();
            prop_assert_eq!(but(&got), but(&want));
        }
        let xmajor = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
        let mut cols: Vec<i32> = got.iter().map(|&(i, j)| if xmajor { i } else { j }).collect();
        cols.sort_unstable();
        prop_assert!(cols.windows(2).all(|w| w[0] != w[1]), "two pixels in one column: {got:?}");
    }

    /// a line of both ends in one diamond lights nothing, whatever its direction
    #[test]
    fn warp_line_inside_one_diamond_is_empty(c in (1i32..15, 1i32..15), d in (-40i32..40, -40i32..40), e in (-40i32..40, -40i32..40)) {
        prop_assume!(d.0.abs() + d.1.abs() < 120 && e.0.abs() + e.1.abs() < 120 && d != e);
        let at = |o: (i32, i32)| (c.0 * 256 + 128 + o.0, c.1 * 256 + 128 + o.1);
        prop_assert!(kikizake::model::warpline::pixels(at(d), at(e), 16, 16).is_empty());
    }
}
