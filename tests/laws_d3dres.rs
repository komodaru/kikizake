//! Laws of the Direct3D 10/11 models (conversions, the rasterizer), apart from Windows' answers:
//! exhaustive over the small formats, properties over the rest.
use kikizake::case::parse_cases;
use kikizake::generators::d3ddraw::{shader, Prog};
use kikizake::generators::dxbc::hex;
use kikizake::model::dxgiconv::*;
use kikizake::model::run_case;
use proptest::prelude::*;

#[test]
fn unorm_codes_round_trip() {
    for n in [2u32, 4, 5, 6, 8, 10, 16] {
        let max = (1u32 << n) - 1;
        for c in 0..=max {
            assert_eq!(to_unorm(c as f32 / max as f32, n), c, "{n} bits, {c}");
        }
        assert_eq!(to_unorm(-1.0, n), 0);
        assert_eq!(to_unorm(2.0, n), max);
        assert_eq!(to_unorm(f32::NAN, n), 0);
    }
}

#[test]
fn snorm_codes_round_trip() {
    for n in [8u32, 16] {
        let max = (1i32 << (n - 1)) - 1;
        for c in -max..=max {
            let v = c as f32 * (1.0 / max as f32);
            let got = to_snorm(v, n) as i32;
            let got = if got > max { got - (1 << n) } else { got };
            assert_eq!(got, c, "{n} bits, {c}");
        }
    }
}

/// every finite 16-bit float, and every 11 and 10 bit one, back to its bits; truncation never
/// rounds up
#[test]
fn small_floats_round_trip() {
    for (e, m, signed) in [(5u32, 10u32, true), (5, 6, false), (5, 5, false)] {
        for b in 0..1u32 << (e + m + signed as u32) {
            let v = from_small_float(b, e, m, signed);
            if v.is_nan() {
                continue;
            }
            assert_eq!(to_small_float(v, e, m, signed), b, "{e} {m} {signed}: {b:#x} {v}");
        }
    }
}

fn pos_f32() -> impl Strategy<Value = f32> {
    (0u32..0x7f80_0000).prop_map(f32::from_bits)
}

proptest! {
    #[test]
    fn unorm_is_monotone(a in -0.5f32..1.5, b in -0.5f32..1.5, n in prop::sample::select(vec![2u32, 5, 8, 10, 16])) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(to_unorm(lo, n) <= to_unorm(hi, n));
    }

    #[test]
    fn small_float_truncates(v in pos_f32(), k in 0usize..3) {
        let (e, m, signed) = [(5u32, 10u32, true), (5, 6, false), (5, 5, false)][k];
        let b = to_small_float(v, e, m, signed);
        let back = from_small_float(b, e, m, signed);
        // (truncation: never above the value; the next code is above it unless past the largest)
        prop_assert!(back <= v);
        let next = from_small_float(b + 1, e, m, signed);
        prop_assert!(next > v || b + 1 == ((1 << e) - 1) << m);
        prop_assert_eq!(to_small_float(-v, e, m, signed), if signed { b | 1 << (e + m) } else { 0 });
    }

    #[test]
    fn small_float_is_monotone(a in pos_f32(), b in pos_f32()) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(to_small_float(lo, 5, 10, true) <= to_small_float(hi, 5, 10, true));
        prop_assert!(to_small_float(lo, 5, 6, false) <= to_small_float(hi, 5, 6, false));
    }

    #[test]
    fn rgb9e5_reads_back_within_its_precision(r in 0f32..65000.0, g in 0f32..65000.0, b in 0f32..65000.0) {
        let back = from_rgb9e5(to_rgb9e5([r, g, b]));
        let big = r.max(g).max(b);
        // (the shared exponent: every channel within an ulp of the largest's 9-bit mantissa)
        let ulp = 2f32.powi((big.log2().floor() as i32) - 8).max(2f32.powi(-24));
        for (x, y) in [r, g, b].iter().zip(back) {
            prop_assert!((x - y).abs() <= ulp * 1.01, "{} {} ulp {}", x, y, ulp);
        }
    }
}

/// a 16x16 RGBA8 target, the vs and ps programs, additive blending, `tris` (pixel coordinates,
/// one color: 16/255 in red), drawn; the red bytes
fn draw(tris: &[[(f32, f32); 3]]) -> Vec<u8> {
    let mut s = String::from("case law\ndevice11\ntex 0 2 16 16 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\n");
    s += &format!("shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\n", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)));
    let f = |v: f32| format!("{:#x}", v.to_bits());
    s += &format!("viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0\nblend1 1 2 2 1 2 2 1 0xf\n", f(16.0), f(16.0), f(1.0));
    for t in tris {
        for p in t {
            s += &format!("vtx {} {} {} {} {} 0 0 {}\n", f(p.0 / 16.0 * 2.0 - 1.0), f(1.0 - p.1 / 16.0 * 2.0), f(0.5), f(1.0), f(16.0 / 255.0), f(1.0));
        }
    }
    s += &format!("draw 4 {} 32\nread 0 0 64 16 1\nend\n", tris.len() * 3);
    let c = &parse_cases(&s)[0];
    let out = run_case(c).unwrap();
    let rows: Vec<&String> = out.iter().rev().take(16).collect();
    let mut red = Vec::new();
    for r in rows.iter().rev() {
        for px in 0..16 {
            red.push(u8::from_str_radix(&r[px * 8..px * 8 + 2], 16).unwrap());
        }
    }
    red
}

fn grid() -> impl Strategy<Value = (f32, f32)> {
    ((-3i32 * 256)..(19 * 256), (-3i32 * 256)..(19 * 256)).prop_map(|(x, y)| (x as f32 / 256.0, y as f32 / 256.0))
}

fn side(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> f32 {
    (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// two triangles on either side of a shared edge (the top-left rule): no pixel drawn twice,
    /// and together they draw what each draws alone
    #[test]
    fn a_shared_edge_is_drawn_once(a in grid(), b in grid(), c in grid(), d in grid()) {
        prop_assume!(side(a, c, b) * side(a, c, d) < 0.0);
        let both = draw(&[[a, b, c], [a, c, d]]);
        prop_assert!(both.iter().all(|&v| v == 0 || v == 16), "{:?}", both);
        let (one, two) = (draw(&[[a, b, c]]), draw(&[[a, c, d]]));
        for i in 0..256 {
            prop_assert_eq!(both[i], one[i] + two[i]);
        }
    }

    /// the vertices' order (and so the winding, with nothing culled) does not change what is drawn
    #[test]
    fn winding_does_not_change_coverage(a in grid(), b in grid(), c in grid()) {
        prop_assert_eq!(draw(&[[a, b, c]]), draw(&[[a, c, b]]));
        prop_assert_eq!(draw(&[[a, b, c]]), draw(&[[b, c, a]]));
    }
}

/// (the harness draws: a triangle over the whole target covers every pixel once)
#[test]
fn a_covering_triangle_draws_every_pixel() {
    assert!(draw(&[[(-1.0, -1.0), (40.0, -1.0), (-1.0, 40.0)]]).iter().all(|&v| v == 16));
    assert!(draw(&[[(2.0, 2.0), (2.0, 2.0), (9.0, 9.0)]]).iter().all(|&v| v == 0));
}

/// the suites' cases with one word changed (to 0, 1, -1, a large or a nearby value): the models
/// never panic and answer every case, or end where the x86 runtime dies
#[test]
fn mutated_cases_never_panic() {
    let mut seed = 0x1234_5678u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for s in ["d3d10res", "d3d11res", "d3d10draw", "d3d11draw"] {
        let text = kikizake::generators::suite(s).unwrap();
        let cases: Vec<&str> = text.split("\ncase ").skip(1).collect();
        for _ in 0..600 {
            let c = cases[next() as usize % cases.len()];
            // (the size suites make textures of up to 1 GiB: mutated, their reads and clears
            // would touch all of it)
            if c.starts_with("big-") {
                continue;
            }
            let mut lines: Vec<Vec<String>> = c.lines().map(|l| l.split(' ').map(String::from).collect()).collect();
            for _ in 0..1 + next() % 3 {
                let li = 1 + next() as usize % (lines.len() - 1).max(1);
                let Some(l) = lines.get_mut(li) else { continue };
                // (a draw of billions of vertices or instances takes Windows as long: not one)
                if l[0] == "draw" {
                    continue;
                }
                let sized = l[0] == "tex" || l[0] == "buf";
                let wi = next() as usize % l.len();
                let w = &mut l[wi];
                let v: i64 = w.strip_prefix("0x").map_or_else(|| w.parse().unwrap_or(0), |x| i64::from_str_radix(x, 16).unwrap_or(0));
                *w = match next() % 6 {
                    0 => "0".into(),
                    1 => "1".into(),
                    2 => "0xffffffff".into(),
                    // (sizes kept to what a test touches: below 2^16)
                    3 if sized => format!("{:#x}", next() & 0xffff),
                    3 => format!("{:#x}", next()),
                    4 => (v + 1).to_string(),
                    _ => (v - 1).max(0).to_string(),
                };
            }
            let t = format!("case {}\n", lines.iter().map(|l| l.join(" ")).collect::<Vec<_>>().join("\n"));
            let parsed = parse_cases(&t);
            for arch in [32, 64] {
                kikizake::model::set_arch(arch);
                let o = kikizake::case::parse_output(&kikizake::model::run(s, &parsed));
                // (a line the x86 runtime dies on ends the output there, as a patch list's draw
                // does on either)
                let patch = lines.iter().any(|l| l[0] == "draw" && l.get(1).and_then(|w| w.parse::<u32>().ok()).is_some_and(|top| (33..=64).contains(&top)));
                assert!(o.cases.len() == parsed.len() && o.done || (arch == 32 || patch) && !o.done && o.cases.len() == 1, "{t}");
            }
        }
    }
}

/// the Direct3D 10/11 models against Windows' answers, the cases Windows has answered
/// (a case newer than the answers is left to tests/windows.rs): for mutation testing
#[test]
fn d3d_models_match_the_answered_cases() {
    for s in ["d3d10res", "d3d11res", "d3d10draw", "d3d11draw"] {
        for arch in [32, 64] {
            let Ok(g) = std::fs::read_to_string(format!("{}/golden/{s}-{arch}/stdout.txt", env!("CARGO_MANIFEST_DIR"))) else { continue };
            kikizake::model::set_arch(arch);
            let want = kikizake::model::run(s, &parse_cases(&kikizake::generators::suite(s).unwrap()));
            let r = kikizake::compare::compare_text(&want, &g);
            let bad: Vec<_> = r.differ.iter().filter(|d| !d.contains(": missing (")).collect();
            assert!(bad.is_empty(), "{s}-{arch}: {bad:?}");
        }
    }
}

proptest! {
    /// any 16 bytes are a BC6H and a BC7 block: every texel decodes, a reserved mode to zeros;
    /// an unsigned BC6H texel is never negative nor past the largest finite half
    #[test]
    fn bptc_blocks_decode(b in proptest::array::uniform16(any::<u8>()), x in 0u32..4, y in 0u32..4) {
        use kikizake::model::bptc::{bc6h, bc7};
        let u = bc6h(&b, false, x, y);
        prop_assert!(u.iter().all(|&h| h <= 0x7bff));
        let _ = bc6h(&b, true, x, y);
        let _ = bc7(&b, x, y);
        let mut z = b;
        z[0] = 0;
        prop_assert_eq!(bc7(&z, x, y), [0; 4]);
    }
}

/// every signed BC4 endpoint pair and index: the step lies between the endpoints' values (or is
/// -1, 0 or 1 at the six-step block's fixed codes), and never past [-1, 1]
#[test]
fn bc4_snorm_steps_lie_between_their_endpoints() {
    let f = kikizake::model::dxgifmt::format(81).unwrap();
    let end = |r: i8| f32::from(r.max(-127)) / 127.0;
    for r0 in i8::MIN..=i8::MAX {
        for r1 in i8::MIN..=i8::MAX {
            for k in 0..8u64 {
                let mut b = [0u8; 8];
                b[0] = r0.to_le_bytes()[0];
                b[1] = r1.to_le_bytes()[0];
                b[2..8].copy_from_slice(&(k.to_le_bytes())[..6]);
                let v = f32::from_bits(decode_block(f, &b, 0, 0).unwrap()[0]);
                assert!((-1.0..=1.0).contains(&v), "{r0} {r1} {k}: {v}");
                if r0 <= r1 && k >= 6 {
                    assert_eq!(v, if k == 6 { -1.0 } else { 1.0 });
                    continue;
                }
                let (lo, hi) = (end(r0).min(end(r1)), end(r0).max(end(r1)));
                assert!(v >= lo - 1e-4 && v <= hi + 1e-4, "{r0} {r1} {k}: {v} not in {lo}..{hi}");
            }
        }
    }
}

/// objects, slots and views in any order (the review of 8480456, 2.1: a view kept its
/// resource's slot, and clearing it after the slot was given another texture wrote that one):
/// random lines of making textures, views of them, clears through views and copies, against a
/// reference of objects that views hold by identity (D3DRES.VIEW.HOLDS); every slot read back
#[derive(Clone, Debug)]
enum Life {
    Tex(u8),
    Rtv(u8, u8),
    Clear(u8, u8),
    Copy(u8, u8),
}

fn life() -> impl Strategy<Value = Life> {
    prop_oneof![
        (0u8..3).prop_map(Life::Tex),
        (0u8..3, 0u8..3).prop_map(|(v, r)| Life::Rtv(v, r)),
        (0u8..3, 0u8..4).prop_map(|(v, c)| Life::Clear(v, c)),
        (0u8..3, 0u8..3).prop_map(|(d, s)| Life::Copy(d, s)),
    ]
}

proptest! {
    #[test]
    fn views_hold_their_resources(ops in prop::collection::vec(life(), 1..24), d3d11: bool) {
        // (1x1 R8G8B8A8_UNORM, render target and shader resource; the four clear colors)
        let colors = ["0x3f800000 0 0 0x3f800000", "0 0x3f800000 0 0x3f800000", "0 0 0x3f800000 0x3f800000", "0x3f800000 0x3f800000 0x3f800000 0"];
        let bytes: [[u8; 4]; 4] = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 0]];
        let mut text = String::from(if d3d11 { "device11\n" } else { "device10\n" });
        let (mut objs, mut res, mut views): (Vec<[u8; 4]>, [Option<usize>; 3], [Option<usize>; 3]) = (Vec::new(), [None; 3], [None; 3]);
        let mut want: Vec<String> = Vec::new();
        for op in &ops {
            match *op {
                Life::Tex(s) => {
                    text += &format!("tex {s} 2 1 1 1 1 0x1c 0 0x28 0 0 1 0\n");
                    objs.push([0; 4]);
                    res[s as usize] = Some(objs.len() - 1);
                }
                Life::Rtv(v, r) => {
                    text += &format!("rtv {v} {r} nodesc\n");
                    match res[r as usize] {
                        Some(o) => {
                            views[v as usize] = Some(o);
                        }
                        None => want.push("rtv nores".into()),
                    }
                }
                Life::Clear(v, c) => {
                    text += &format!("clear {v} {}\n", colors[c as usize]);
                    match views[v as usize] {
                        Some(o) => objs[o] = bytes[c as usize],
                        None => want.push("clear noview".into()),
                    }
                }
                Life::Copy(d, s) => {
                    text += &format!("copy {d} {s}\n");
                    match (res[d as usize], res[s as usize]) {
                        (Some(a), Some(b)) => objs[a] = objs[b],
                        _ => want.push("copy nores".into()),
                    }
                }
            }
        }
        for s in 0..3 {
            text += &format!("read {s} 0 4 1 1\n");
        }
        let got = run_case(&parse_cases(&format!("case p\n{text}end\n"))[0]).map_err(|e| TestCaseError::fail(format!("{e:?}")))?;
        // (the reads: each slot's object as the reference has it)
        let mut reads: Vec<String> = Vec::new();
        for r in res {
            match r {
                Some(o) => reads.extend(["read 1 1".to_string(), objs[o].iter().map(|b| format!("{b:02x}")).collect()]),
                None => reads.push("read nores".into()),
            }
        }
        prop_assert!(got.ends_with(&reads), "{}\n{:?}\nwant {:?}", text, got, reads);
        // (the refusals, in order)
        let refused: Vec<&String> = got.iter().filter(|l| l.ends_with("nores") && !l.starts_with("read") || l.ends_with("noview")).collect();
        let wanted: Vec<&String> = want.iter().filter(|l| l.ends_with("nores") || l.ends_with("noview")).collect();
        prop_assert_eq!(refused, wanted, "{}", text);
    }
}

/// the review's own sequence: a view of texture A, slot 0 given texture B, the old view cleared:
/// B stays as made
#[test]
fn a_reused_slot_keeps_its_new_texture() {
    for dev in ["device11", "device10"] {
        let text = format!("{dev}\ntex 0 2 1 1 1 1 0x1c 0 0x28 0 0 1 0\nrtv 0 0 nodesc\ntex 0 2 1 1 1 1 0x1c 0 0x28 0 0 1 0\nclear 0 0x3f800000 0 0 0x3f800000\nread 0 0 4 1 1\n");
        let got = run_case(&parse_cases(&format!("case p\n{text}end\n"))[0]).unwrap();
        assert_eq!(got.last().map(String::as_str), Some("00000000"), "{dev}: {got:?}");
    }
}
