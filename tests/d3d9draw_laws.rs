//! Laws of the Direct3D 9 / 8 drawing and D3DX 9 models (exhaustive over small domains,
//! property-based elsewhere): what the documentation states as properties, whatever Windows'
//! measured details turn out to be.
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
    /// halves; the 8 at a time one only differs at exact ties, by one toward zero; x64's 4 at a
    /// time one is the one by one one but for NaNs, its one by one one truncates denormals
    #[test]
    fn d3dx_halves(m in 0x3880_0000u32..0x477f_f000, sign: bool) {
        use kikizake::model::d3dx9::{f16_block, f16_one, f16_x64};
        let bits = m | (sign as u32) << 31;
        let x = f32::from_bits(bits);
        let ieee = f32_to_f16(x);
        prop_assert_eq!(f16_one(bits), ieee);
        prop_assert_eq!(f16_x64(bits, true), ieee);
        prop_assert_eq!(f16_x64(bits, false), ieee);
        let tie = bits & 0x1fff == 0x1000;
        prop_assert_eq!(f16_block(bits), if tie && bits & 0x2000 != 0 { ieee - 1 } else { ieee });
    }
}

#[test]
fn half_midpoint_neighbors() {
    use kikizake::model::d3dx9::f16_block;
    for sign in [0u32, 0x8000_0000] {
        for (base, half) in [(0x3f80_0000, 0x3c00), (0x3f80_2000, 0x3c01)] {
            for (tail, up) in [(0xfff, 0), (0x1000, 0), (0x1001, 1)] {
                assert_eq!(f16_block(sign | base | tail), (sign >> 16) as u16 | (half + up));
            }
        }
    }
}

/// every format WARP takes as a texture or a target (either API) has a decoding the model samples
/// with: none of the formats it takes falls to the model's "bad line" (D3D9.TEX.FORMAT)
#[test]
fn every_taken_format_is_decoded() {
    for &(f, name) in TEXTURE_FORMATS {
        for v8 in [false, true] {
            for rt in [false, true] {
                if !supported(f, rt, v8) {
                    continue;
                }
                let m = format(f).unwrap_or_else(|| panic!("{name} taken but not decoded"));
                assert!(!matches!(m.kind, Kind::Yuv), "{name}");
                let p = m.ch.iter().any(|c| c.c == b'p');
                assert!(!p || matches!(f, 40 | 41), "{name}");
            }
        }
    }
}

#[test]
fn free_line_start_selects_the_measured_row() {
    let lines = picture("case x\ndevice swvp\nd9target 16 16 21\nd9clear 1 0 0 0\nd9fvf 0x44\nd9draw 2 2 0x41500000 0x41180000 0 0x3f800000 0xffffffff 0x40600000 0x40880000 0 0x3f800000 0xffffffff\nd9rt\nend");
    let rows = &lines[lines.iter().position(|l| l == "px rt 16 16 32").unwrap() + 1..];
    assert_eq!(&rows[9][13 * 8..14 * 8], "00000000");
    assert_eq!(&rows[10][13 * 8..14 * 8], "ffffffff");
}
