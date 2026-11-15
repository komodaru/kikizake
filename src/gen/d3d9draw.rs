//! The d3d9draw and d3d8draw suites' case generators: Direct3D 9 and 8 drawing (SPEC.md
//! "Direct3D 9 and 8 drawing"). Every case draws on a small lockable render target and reads it
//! back; pictures are made so that each pixel asks one question (a texture mapped texel for
//! pixel, one clear rectangle per pixel), so a 16x16 target asks 256 at once.
use super::*;
use crate::model::d3d9draw::{BLEND_OPS, CMP, STENCIL_OPS, TARGET_FORMATS, TEXTURE_FORMATS, TOPS};

pub fn d3d9draw() -> String {
    suite(false)
}

pub fn d3d8draw() -> String {
    suite(true)
}

pub(crate) fn fb(x: f32) -> String {
    format!("{:#x}", x.to_bits())
}

/// XYZRHW | DIFFUSE | TEX1
pub const FVF_TL_TEX: u32 = 0x144;
/// XYZRHW | DIFFUSE
pub const FVF_TL: u32 = 0x44;

/// a vertex's words: x y z rhw diffuse [u v]
fn v(x: f32, y: f32, z: f32, c: u32, uv: Option<(f32, f32)>) -> String {
    let mut s = format!("{} {} {} {} {c:#x}", fb(x), fb(y), fb(z), fb(1.0));
    if let Some((u, w)) = uv {
        write!(s, " {} {}", fb(u), fb(w)).unwrap();
    }
    s
}

/// a quad covering x0..x1, y0..y1 (pixel edges: the D3D9 pixel centers are at integers, so the
/// edges sit at -0.5), texture coordinates 0..1, as a triangle strip of 4
fn quad(s: &mut String, x0: f32, y0: f32, x1: f32, y1: f32, z: f32, c: u32) {
    let (a, b, cc, d) = (x0 - 0.5, y0 - 0.5, x1 - 0.5, y1 - 0.5);
    writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}", v(a, b, z, c, Some((0.0, 0.0))), v(cc, b, z, c, Some((1.0, 0.0))), v(a, d, z, c, Some((0.0, 1.0))), v(cc, d, z, c, Some((1.0, 1.0)))).unwrap();
}

/// the texture's texels straight to the target's pixels: stage 0 selects the texture's color and
/// alpha
fn texture_stage(s: &mut String) {
    s.push_str("tss set 0 1 2\ntss set 0 4 2\n");
}

fn tri(s: &mut String, p: [(f32, f32); 3], c: [u32; 3], z: f32) {
    writeln!(s, "d9fvf {FVF_TL:#x}\nd9draw 4 3 {} {} {}", v(p[0].0, p[0].1, z, c[0], None), v(p[1].0, p[1].1, z, c[1], None), v(p[2].0, p[2].1, z, c[2], None)).unwrap();
}

fn suite(v8: bool) -> String {
    let (name, api, dev) = if v8 { ("d3d8draw", "Direct3D 8", "device8") } else { ("d3d9draw", "Direct3D 9", "device") };
    let mut s = format!("# gpu\n# kikizake suite {name}: {api} drawing: clears, texture formats, rasterization, blending, tests, texture stages (SPEC.md). Made by `kikizake gen {name}`.\n");
    if v8 {
        s += "# x86 only: Direct3D 8 has no x86-64 runtime\n";
    }
    let mut r = XorShift32::new(if v8 { 0xd8d8_0001 } else { 0xd9d9_0001 });
    let open = |s: &mut String, n: &str, fmt: u32, w: u32, h: u32| writeln!(s, "case {n}\n{dev} swvp\nd9target {w} {h} {fmt}").unwrap();
    let fmts9 = |f: &&(u32, &str)| !v8 || f.0 < 100 || f.0 > 0xffff;

    // format support
    writeln!(s, "case check\n{dev} swvp").unwrap();
    for f in TEXTURE_FORMATS.iter().filter(fmts9) {
        for (u, t) in [(0, 3), (1, 1), (1, 3)] {
            writeln!(s, "d9check {} {u} {t}", f.0).unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    // other usages and resource types: plain surfaces, depth stencil, dynamic, cube and volume textures
    writeln!(s, "case check-usage\n{dev} swvp").unwrap();
    for f in TEXTURE_FORMATS.iter().filter(fmts9).map(|f| f.0).chain([71, 75, 77, 79, 80, 82, 83]) {
        for (u, t) in [(0, 1), (2, 1), (0x200, 3), (0, 4), (1, 4), (0, 5), (0x400, 3)] {
            writeln!(s, "d9check {f} {u:#x} {t}").unwrap();
        }
    }
    writeln!(s, "end").unwrap();

    // clears: every 8-bit value of each channel (color i,i,i,i in pixel i) and random colors, one
    // 1x1 rectangle per pixel, on each target format
    for f in TARGET_FORMATS.iter().filter(fmts9) {
        for kind in ["ramp", "random"] {
            open(&mut s, &format!("clear-{kind}-{}", f.1), f.0, 16, 16);
            for row in 0..16 {
                // (a Clear has one color: one call of one 1x1 rectangle per pixel)
                for x in 0..16u32 {
                    let i = row * 16 + x;
                    let c = if kind == "ramp" { i * 0x0101_0101 } else { r.next() };
                    writeln!(s, "d9clear 1 {c:#x} 0 0 {x} {row} {} {}", x + 1, row + 1).unwrap();
                }
            }
            writeln!(s, "d9rt\nend").unwrap();
        }
    }
    // Clear's rectangles and flags: random rectangles (inside, crossing the edges, empty,
    // inverted), several per call
    for i in 0..40 {
        open(&mut s, &format!("clear-rects-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 {:#x} 0 0", r.next()).unwrap();
        for _ in 0..r.range(1, 4) {
            let mut l = format!("d9clear {} {:#x} 0 0", if r.chance(8) { 0 } else { 1 }, r.next());
            for _ in 0..r.range(0, 4) {
                let (x0, y0) = (r.range(-4, 18), r.range(-4, 18));
                let (x1, y1) = if r.chance(6) { (x0 - r.range(0, 3), y0) } else { (x0 + r.range(1, 10), y0 + r.range(1, 10)) };
                write!(l, " {x0} {y0} {x1} {y1}").unwrap();
            }
            writeln!(s, "{l}").unwrap();
        }
        writeln!(s, "d9rt\nend").unwrap();
    }

    // texture formats: a 16x16 texture of each format, random bytes and a ramp, mapped texel for
    // pixel onto A8R8G8B8, and an 8x8 one onto A32B32G32R32F (what the sampler reads, unrounded)
    for f in TEXTURE_FORMATS.iter().filter(fmts9) {
        for (kind, tfmt, n) in [("random", 21, 16), ("ramp", 21, 16), ("float", 116, 8)] {
            if v8 && tfmt == 116 {
                continue;
            }
            open(&mut s, &format!("tex-{kind}-{}", f.1), tfmt, n, n);
            writeln!(s, "d9clear 1 0xff00ff 0 0\nd9tex 0 {n} {n} {}\nd9texfill 0 {}\nd9settex 0 0", f.0, if kind == "ramp" { "ramp".to_string() } else { format!("{:#x}", r.next()) }).unwrap();
            texture_stage(&mut s);
            quad(&mut s, 0.0, 0.0, n as f32, n as f32, 0.0, 0xffff_ffff);
            writeln!(s, "d9rt\nend").unwrap();
        }
    }

    // rasterization: flat-colored triangles, vertices on a 1/16 pixel grid, culling off
    let grid = |r: &mut XorShift32, q: i64| r.range(-3 * q, 19 * q) as f32 / q as f32;
    for i in 0..120 {
        open(&mut s, &format!("raster-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1").unwrap();
        for k in 0..r.range(1, 3) {
            let q = *r.pick(&[1, 2, 4, 16]);
            let p = [(grid(&mut r, q), grid(&mut r, q)), (grid(&mut r, q), grid(&mut r, q)), (grid(&mut r, q), grid(&mut r, q))];
            let c = 0xff00_0000 | (0x40 << (8 * k)) | r.next() & 0x3f3f3f;
            tri(&mut s, p, [c; 3], 0.0);
        }
        writeln!(s, "d9rt\nend").unwrap();
    }
    // the fill convention on purpose: quads split along a diagonal, triangles sharing edges, every
    // vertex on a pixel center
    for i in 0..24 {
        open(&mut s, &format!("raster-edges-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1").unwrap();
        let (x0, y0) = (r.range(-2, 8) as f32, r.range(-2, 8) as f32);
        let (x1, y1) = (x0 + r.range(1, 10) as f32, y0 + r.range(1, 10) as f32);
        let (a, b, c, d) = ((x0, y0), (x1, y0), (x0, y1), (x1, y1));
        if r.chance(2) {
            tri(&mut s, [a, b, c], [0xffff_0000; 3], 0.0);
            tri(&mut s, [b, d, c], [0xff00_ff00; 3], 0.0);
        } else {
            tri(&mut s, [a, b, d], [0xffff_0000; 3], 0.0);
            tri(&mut s, [a, d, c], [0xff00_ff00; 3], 0.0);
        }
        // a fan around the middle of the quad
        let m = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        if r.chance(2) {
            tri(&mut s, [m, a, b], [0xff00_00ff; 3], 0.0);
            tri(&mut s, [m, d, c], [0xffff_ff00; 3], 0.0);
        }
        writeln!(s, "d9rt\nend").unwrap();
    }
    // primitive types: strips, fans, lists of several triangles; points and lines
    for (i, ty) in [(0, 4), (1, 5), (2, 6), (3, 4), (4, 5), (5, 6)] {
        open(&mut s, &format!("prims-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nd9fvf {FVF_TL:#x}").unwrap();
        let n = if ty == 4 { 6 } else { 5 };
        let mut l = format!("d9draw {ty} {n}");
        for k in 0..n {
            write!(l, " {}", v(grid(&mut r, 2), grid(&mut r, 2), 0.0, 0xff00_0000 | 0x30 * (k + 1) as u32, None)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    for (i, ty) in [(0, 1), (1, 2), (2, 3), (3, 1), (4, 2), (5, 3)] {
        open(&mut s, &format!("points-lines-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nd9fvf {FVF_TL:#x}").unwrap();
        let n = 6;
        let mut l = format!("d9draw {ty} {n}");
        for _ in 0..n {
            write!(l, " {}", v(grid(&mut r, 4), grid(&mut r, 4), 0.0, 0xffff_ffff, None)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // culling: each mode, both windings
    for mode in 1..=3 {
        open(&mut s, &format!("cull-{mode}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 {mode}").unwrap();
        tri(&mut s, [(1.0, 1.0), (7.0, 1.0), (1.0, 7.0)], [0xffff_0000; 3], 0.0); // clockwise on screen
        tri(&mut s, [(9.0, 9.0), (9.0, 15.0), (15.0, 9.0)], [0xff00_ff00; 3], 0.0); // counterclockwise
        writeln!(s, "d9rt\nend").unwrap();
    }
    // flat and Gouraud shading
    for i in 0..16 {
        open(&mut s, &format!("shade-{i}"), 21, 16, 16);
        let flat = i % 4 == 0;
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1{}", if flat { "\nrs set 9 1" } else { "" }).unwrap();
        let p = if i < 8 { [(-0.5, -0.5), (15.5, -0.5), (-0.5, 15.5)] } else { [(grid(&mut r, 4), grid(&mut r, 4)), (grid(&mut r, 4), grid(&mut r, 4)), (grid(&mut r, 4), grid(&mut r, 4))] };
        let c = if i == 1 { [0xffff_0000, 0xff00_ff00, 0xff00_00ff] } else { [r.next(), r.next(), r.next()] };
        tri(&mut s, p, c, 0.0);
        writeln!(s, "d9rt\nend").unwrap();
    }

    // texture stages: a random texture with each color operation and each alpha operation of the
    // texture against a diffuse color (and the texture factor)
    for &(op, top) in TOPS {
        for alpha in [false, true] {
            open(&mut s, &format!("top-{}-{top}", if alpha { "alpha" } else { "color" }), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
            // the operation on one channel kind, the other selects the texture; arg2 the diffuse
            // color, arg0 (the three-argument operations) the texture factor
            let (o, other) = if alpha { (4, 1) } else { (1, 4) };
            writeln!(s, "tss set 0 {other} 2\ntss set 0 {o} {op}\nrs set 60 {:#x}", r.next()).unwrap();
            writeln!(s, "tss set 0 {} 3", if alpha { 27 } else { 26 }).unwrap();
            quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
            writeln!(s, "d9rt\nend").unwrap();
        }
    }

    let blend_case = |s: &mut String, r: &mut XorShift32, name: &str, states: &[(u32, u32)]| blend_sized(s, dev, r, name, states, 16);
    let factors: Vec<u32> = (1..=if v8 { 13 } else { 15 }).collect();
    for &f in &factors {
        blend_case(&mut s, &mut r, &format!("blend-src-{f}"), &[(19, f), (20, 1)]);
        let bf = r.next();
        blend_case(&mut s, &mut r, &format!("blend-dest-{f}"), &[(19, 1), (20, f), (193, bf)]);
    }
    for &(op, n) in BLEND_OPS {
        blend_case(&mut s, &mut r, &format!("blend-op-{n}"), &[(19, 5), (20, 6), (171, op)]);
    }
    for i in 0..40 {
        let mut st = vec![(19, *r.pick(&factors)), (20, *r.pick(&factors)), (171, r.pick(BLEND_OPS).0)];
        if !v8 {
            st.push((193, r.next()));
            if r.chance(2) {
                st.extend([(206, 1), (207, *r.pick(&factors)), (208, *r.pick(&factors)), (209, r.pick(BLEND_OPS).0)]);
            }
        }
        st.retain(|x| !v8 || x.0 != 193);
        blend_case(&mut s, &mut r, &format!("blend-random-{i}"), &st);
    }

    // alpha test: each comparison with a few references, on a random texture's alpha
    for &(f, n) in CMP {
        for refv in [0u32, 0x80, 0xff, r.next() & 0xff] {
            open(&mut s, &format!("alphatest-{n}-{refv:x}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0xff00ff 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0\nrs set 15 1\nrs set 25 {f}\nrs set 24 {refv:#x}", r.next()).unwrap();
            texture_stage(&mut s);
            quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
            writeln!(s, "d9rt\nend").unwrap();
        }
    }

    // depth: a cleared depth, quads at depths around it with each comparison, with and without
    // depth writes (a second quad shows what was written)
    for &(f, n) in CMP {
        for write in [1, 0] {
            open(&mut s, &format!("depth-{n}-{write}"), 21, 16, 16);
            writeln!(s, "d9clear 3 0 {} 0\nrs set 23 {f}\nrs set 14 {write}", fb(0.5)).unwrap();
            for (k, z) in [0.25f32, 0.5, 0.75].iter().enumerate() {
                quad(&mut s, k as f32 * 5.0, 0.0, k as f32 * 5.0 + 4.0, 16.0, *z, 0xff00_0000 | 0x50 << (8 * k));
            }
            // then over everything at 0.5 with LESSEQUAL
            writeln!(s, "rs set 23 4\nrs set 14 1").unwrap();
            quad(&mut s, 0.0, 8.0, 16.0, 16.0, 0.5, 0xffff_ffff);
            writeln!(s, "d9rt\nend").unwrap();
        }
    }
    // stencil: a cleared stencil value, each comparison against references, each operation on
    // pass / fail; a second pass shows the stored values (EQUAL to each possible result)
    for i in 0..32 {
        open(&mut s, &format!("stencil-{i}"), 21, 16, 16);
        let any = r.next() & 0xff;
        let clear = *r.pick(&[0u32, 1, 0x7f, 0x80, 0xfe, 0xff, any]);
        let f = r.pick(CMP).0;
        writeln!(s, "d9clear 5 0 {} {clear}\nrs set 52 1\nrs set 56 {f}\nrs set 57 {:#x}\nrs set 58 {:#x}\nrs set 59 {:#x}\nrs set 53 {}\nrs set 55 {}", fb(0.5), r.next() & 0xff, if r.chance(2) { 0xff } else { r.next() }, if r.chance(2) { 0xff } else { r.next() }, r.pick(STENCIL_OPS).0, r.pick(STENCIL_OPS).0).unwrap();
        // left half passes the depth test, the right half fails it (ZFAIL)
        writeln!(s, "rs set 54 {}", r.pick(STENCIL_OPS).0).unwrap();
        quad(&mut s, 0.0, 0.0, 8.0, 16.0, 0.25, 0xffff_0000);
        quad(&mut s, 8.0, 0.0, 16.0, 16.0, 0.75, 0xffff_0000);
        // read back: rows by stencil value, EQUAL with references of the values the ops make
        writeln!(s, "rs set 23 8\nrs set 14 0\nrs set 56 3\nrs set 58 0xff\nrs set 53 1\nrs set 54 1\nrs set 55 1").unwrap();
        let mut refs = vec![clear, clear.wrapping_add(1) & 0xff, clear.wrapping_sub(1) & 0xff, !clear & 0xff, 0, 0xff];
        refs.dedup();
        for (k, v) in refs.iter().enumerate() {
            writeln!(s, "rs set 57 {v:#x}").unwrap();
            quad(&mut s, 0.0, 2.0 * k as f32 + 2.0, 16.0, 2.0 * k as f32 + 4.0, 0.5, 0xff00_0000 | 0x2a * (k as u32 + 1) << 8);
        }
        writeln!(s, "d9rt\nend").unwrap();
    }
    // color write masks
    for m in 0..16 {
        open(&mut s, &format!("colorwrite-{m:x}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0x80808080 0 0\nrs set 168 {m}").unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0x1234_5678);
        writeln!(s, "d9rt\nend").unwrap();
    }

    // texture addressing and point sampling: coordinates past 0..1 with each address mode, and
    // magnification / minification
    for mode in 1..=5 {
        open(&mut s, &format!("address-{mode}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 4 4 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        if v8 {
            writeln!(s, "tss set 0 13 {mode}\ntss set 0 14 {mode}\ntss set 0 15 0xff123456").unwrap();
        } else {
            writeln!(s, "ss set 0 1 {mode}\nss set 0 2 {mode}\nss set 0 4 0xff123456").unwrap();
        }
        // 16 pixels across u from -1.5 to 2.5: 4 pixels per texel
        writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}", v(-0.5, -0.5, 0.0, 0xffff_ffff, Some((-1.5, -1.5))), v(15.5, -0.5, 0.0, 0xffff_ffff, Some((2.5, -1.5))), v(-0.5, 15.5, 0.0, 0xffff_ffff, Some((-1.5, 2.5))), v(15.5, 15.5, 0.0, 0xffff_ffff, Some((2.5, 2.5)))).unwrap();
        writeln!(s, "d9rt\nend").unwrap();
    }
    for (i, filter) in [(0, 1), (1, 2), (2, 1), (3, 2)] {
        open(&mut s, &format!("filter-{i}"), 21, 16, 16);
        let n = if i < 2 { 4 } else { 32 };
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 {n} {n} 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        if v8 {
            writeln!(s, "tss set 0 16 {filter}\ntss set 0 17 {filter}").unwrap();
        } else {
            writeln!(s, "ss set 0 5 {filter}\nss set 0 6 {filter}").unwrap();
        }
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
        writeln!(s, "d9rt\nend").unwrap();
    }

    if !v8 {
        // scissor
        for i in 0..8 {
            open(&mut s, &format!("scissor-{i}"), 21, 16, 16);
            let (x0, y0) = (r.range(-2, 12), r.range(-2, 12));
            writeln!(s, "d9clear 1 0 0 0\nrs set 174 1\nd9scissor {x0} {y0} {} {}", x0 + r.range(0, 10), y0 + r.range(0, 10)).unwrap();
            quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xff40_80c0);
            // (a clear with the scissor test on)
            writeln!(s, "d9clear 1 0xffffffff 0 0 0 0 2 2\nd9rt\nend").unwrap();
        }
        // vertex formats and declarations: what GetFVF / GetVertexDeclaration answer after each
        for (i, fvf) in FVFS.iter().enumerate() {
            writeln!(s, "case fvf-{i}\n{dev} swvp\nd9fvf {fvf:#x}\nd9getfvf\nd9getdecl\nend").unwrap();
        }
        for i in 0..40 {
            writeln!(s, "case decl-{i}\n{dev} swvp").unwrap();
            let mut l = "d9decl".to_string();
            let mut off = 0u32;
            for k in 0..r.range(1, 5) {
                let ty = if r.chance(8) { r.range(0, 18) as u32 } else { *r.pick(&[0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]) };
                let usage = if k == 0 && r.chance(2) { 0 } else { r.range(0, 13) as u32 };
                let ui = if r.chance(3) { r.range(0, 9) as u32 } else { 0 };
                let stream = if r.chance(8) { 1 } else { 0 };
                let o = if r.chance(8) { off + 2 } else { off };
                write!(l, " {:#x} {:#x}", stream | o << 16, ty | usage << 16 | ui << 24).unwrap();
                off = o + crate::model::d3d9draw::decl_type_size(ty).unwrap_or(4);
            }
            writeln!(s, "{l}\nd9getfvf\nd9getdecl\nend").unwrap();
        }
    }
    // Gouraud shading of triangles the target clips: one vertex past one edge, then any
    let mut r = XorShift32(0x3c1a_9e55);
    for i in 0..16 {
        open(&mut s, &format!("shade-clip-{i}"), 21, 16, 16);
        s.push_str("d9clear 1 0 0 0\nrs set 22 1\n");
        let mut p = [(r.range(0, 16) as f32, r.range(0, 16) as f32), (r.range(0, 16) as f32 + 0.25, r.range(0, 16) as f32), (r.range(0, 16) as f32, r.range(0, 16) as f32 + 0.5)];
        if i < 8 {
            p[0] = [(-3.0 - i as f32, 7.0), (22.0, 3.0 + i as f32), (5.0, -2.5 - i as f32), (9.0 + i as f32, 20.0)][i % 4];
        } else {
            p[0] = (r.range(-12, 28) as f32, r.range(-12, 28) as f32);
            p[1] = (r.range(-12, 28) as f32 + 0.5, r.range(-12, 28) as f32);
        }
        let c = [r.next(), r.next(), r.next()];
        tri(&mut s, p, c, 0.0);
        writeln!(s, "d9rt\nend").unwrap();
    }
    // the specular color added (SPECULARENABLE), on and off, over Gouraud shading
    let mut r = XorShift32(0x5bec_0001);
    for i in 0..4 {
        open(&mut s, &format!("specular-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 29 {}\nd9fvf 0xc4", (i < 3) as u32).unwrap();
        let mut l = "d9draw 4 3".to_string();
        for (x, y) in [(-0.5f32, -0.5f32), (15.5, -0.5), (-0.5, 15.5)] {
            write!(l, " {} {} {} {} {:#x} {:#x}", fb(x), fb(y), fb(0.0), fb(1.0), r.next(), r.next()).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    if !v8 {
        // the declarations SetFVF makes of FVFs with the odd bits (0x1, 0x2000, 0x4000, 0x8000)
        let mut r = XorShift32(0xf1f0_2000);
        writeln!(s, "case fvf-bits\ndevice swvp").unwrap();
        for _ in 0..64 {
            let mut f = *r.pick(&[0x002u32, 0x004, 0x006, 0x00a, 0x00e, 0x4002]) | (r.next() & 0xf0) | (r.range(0, 8) as u32) << 8;
            f |= *r.pick(&[0x1u32, 0x2000, 0x4000, 0x8000, 0x1000, 0x2001, 0x6000, 0x8001]);
            f |= ((r.next() as u64 & 0xffff) << 16 & ((1u64 << (16 + 2 * (f >> 8 & 0xf))) - 1)) as u32;
            writeln!(s, "d9fvf {f:#x}\nd9getfvf\nd9getdecl").unwrap();
        }
        writeln!(s, "end").unwrap();
        // (five blend weights and no indices, which D3DXDeclaratorFromFVF refuses)
        writeln!(s, "case fvf-blend5\ndevice swvp").unwrap();
        for f in [0x00eu32, 0x04e, 0x14e, 0x200e, 0x10e, 0x2014e] {
            writeln!(s, "d9fvf {f:#x}\nd9getfvf\nd9getdecl").unwrap();
        }
        writeln!(s, "end").unwrap();
        // (more than 8 texture coordinate sets)
        writeln!(s, "case fvf-texcount\ndevice swvp").unwrap();
        for n in 8..=15u32 {
            for f in [0x002u32, 0x004, 0x4002] {
                writeln!(s, "d9fvf {:#x}\nd9getfvf\nd9getdecl", f | n << 8).unwrap();
            }
        }
        writeln!(s, "end").unwrap();
        // CreateVertexDeclaration's checks: every one-element declaration (type, method, usage,
        // a usage index), then pairs and random ones of elements that pass alone
        for ty in 0..=17u32 {
            writeln!(s, "case decl-one-{ty}\ndevice swvp").unwrap();
            for method in 0..=7u32 {
                for usage in 0..=13u32 {
                    for index in [0u32, 1, 15, 16] {
                        writeln!(s, "d9decl 0x0 {:#x}\nd9getdecl", ty | method << 8 | usage << 16 | index << 24).unwrap();
                    }
                }
            }
            writeln!(s, "end").unwrap();
        }
        let mut r = XorShift32(0xdec1_0002);
        for i in 0..16 {
            writeln!(s, "case decl-pair-{i}\ndevice swvp").unwrap();
            for _ in 0..40 {
                let mut l = "d9decl".to_string();
                let mut off = 0u32;
                let n = r.range(2, 5);
                for _ in 0..n {
                    let ty = r.range(0, 16) as u32;
                    let usage = r.range(0, 12) as u32;
                    let index = if r.chance(3) { r.range(0, 3) as u32 } else { 0 };
                    let stream = if r.chance(6) { 1 } else { 0 };
                    let o = if r.chance(8) { off + 4 } else if r.chance(10) { off.saturating_sub(4) } else { off };
                    write!(l, " {:#x} {:#x}", stream | o << 16, ty | usage << 16 | index << 24).unwrap();
                    off = o + crate::model::d3d9draw::decl_type_size(ty).unwrap_or(4);
                }
                writeln!(s, "{l}\nd9getdecl").unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // untransformed vertices (lighting off): world, view and projection matrices, viewports;
    // and pretransformed ones of RHW other than 1 (perspective-correct interpolation)
    let mut r = XorShift32(0x7a3f_0001);
    let mat = |m: [f32; 16]| m.iter().map(|&x| format!(" {}", fb(x))).collect::<String>();
    for i in 0..16 {
        open(&mut s, &format!("xyz-{i}"), 21, 16, 16);
        s.push_str("d9clear 1 0 0 0\nrs set 22 1\nrs set 137 0\nrs set 7 0\n");
        let sc = 0.5 + r.range(0, 8) as f32 / 8.0;
        let (tx, ty) = (r.range(-4, 4) as f32 / 8.0, r.range(-4, 4) as f32 / 8.0);
        writeln!(s, "d9xform 256{}", mat([sc, 0.0, 0.0, 0.0, 0.0, sc, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, 0.0, 1.0])).unwrap();
        if i % 2 == 1 {
            // (a perspective projection: w = z)
            writeln!(s, "d9xform 3{}", mat([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 0.0, 0.0, 0.0, 0.0])).unwrap();
        }
        if i >= 8 {
            writeln!(s, "d9viewport {} {} {} {} {} {}", r.range(0, 4), r.range(0, 4), r.range(8, 12), r.range(8, 12), fb(0.0), fb(1.0)).unwrap();
        }
        let mut l = "d9fvf 0x42\nd9draw 4 3".to_string();
        for _ in 0..3 {
            let (x, y) = (r.range(-12, 12) as f32 / 10.0, r.range(-12, 12) as f32 / 10.0);
            // (inside 0 ..= 1 in depth: z itself, or z / w = 0.5 under the perspective)
            let z = if i % 2 == 1 { 1.0 + r.range(0, 30) as f32 / 10.0 } else { r.range(1, 9) as f32 / 10.0 };
            write!(l, " {} {} {} {:#x}", fb(x), fb(y), fb(z), r.next() | 0xff00_0000).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    for i in 0..8 {
        open(&mut s, &format!("rhw-{i}"), 21, 16, 16);
        s.push_str("d9clear 1 0 0 0\nrs set 22 1\nrs set 7 0\n");
        let mut l = format!("d9fvf {FVF_TL:#x}\nd9draw 4 3");
        for _ in 0..3 {
            let rhw = 1.0 / (1.0 + r.range(0, 40) as f32 / 10.0);
            write!(l, " {} {} {} {} {:#x}", fb(r.range(-2, 17) as f32), fb(r.range(-2, 17) as f32), fb(0.5), fb(rhw), r.next()).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    let mut r = XorShift32(0x0f09_0001);
    // fog: vertex fog from the specular alpha (table mode NONE), table fog EXP, EXP2, LINEAR on z
    for i in 0..8 {
        open(&mut s, &format!("fog-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 7 0\nrs set 28 1\nrs set 34 {:#x}\nrs set 35 {}\nrs set 140 0", r.next(), i % 4).unwrap();
        writeln!(s, "rs set 36 {}\nrs set 37 {}\nrs set 38 {}\nd9fvf 0xc4", fb(r.range(0, 4) as f32 / 10.0), fb(0.5 + r.range(0, 5) as f32 / 10.0), fb(0.5 + r.range(0, 30) as f32 / 10.0)).unwrap();
        let mut l = "d9draw 4 3".to_string();
        for (x, y) in [(-0.5f32, -0.5f32), (15.5, -0.5), (-0.5, 15.5)] {
            write!(l, " {} {} {} {} {:#x} {:#x}", fb(x), fb(y), fb(r.range(0, 10) as f32 / 10.0), fb(1.0), r.next(), r.next()).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // two-sided stencil (9 only): the counterclockwise ops, both windings drawn
    if !v8 {
        for i in 0..8 {
            open(&mut s, &format!("stencil2-{i}"), 21, 16, 16);
            writeln!(s, "d9clear 5 0 0 {}\nrs set 22 1\nrs set 52 1\nrs set 185 1\nrs set 57 {}", r.range(0, 4), r.range(0, 4)).unwrap();
            for k in 53..=56 {
                writeln!(s, "rs set {k} {}", if k == 56 { r.range(1, 8) } else { r.range(1, 8) }).unwrap();
            }
            for k in 186..=189 {
                writeln!(s, "rs set {k} {}", r.range(1, 8)).unwrap();
            }
            tri(&mut s, [(-0.5, -0.5), (15.5, -0.5), (-0.5, 15.5)], [0xffff_0000; 3], 0.0);
            tri(&mut s, [(15.5, 15.5), (15.5, -0.5), (-0.5, 15.5)], [0xff00_ff00; 3], 0.0);
            tri(&mut s, [(-0.5, -0.5), (15.5, 15.5), (15.5, -0.5)], [0xff00_00ff; 3], 0.0);
            writeln!(s, "rs set 52 0\nrs set 56 3\nd9rt\nend").unwrap();
        }
    }
    // two stages: stage 1 with a second texture and each operation; RESULTARG TEMP; CONSTANT
    for &(op, top) in TOPS {
        open(&mut s, &format!("stage1-{top}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9tex 1 16 16 21\nd9texfill 1 {:#x}\nd9settex 0 0\nd9settex 1 1", r.next(), r.next()).unwrap();
        writeln!(s, "tss set 0 1 4\ntss set 0 4 4\ntss set 1 1 {op}\ntss set 1 2 2\ntss set 1 3 1\ntss set 1 4 {op}\ntss set 1 5 2\ntss set 1 6 1\nrs set 60 {:#x}", r.next()).unwrap();
        writeln!(s, "tss set 1 26 3\ntss set 1 27 3\ntss set 1 11 0").unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // bump mapping: stage 0 a V8U8 (or L6V5U5, X8L8V8U8) texture, BUMPENVMAT and LSCALE / LOFFSET,
    // stage 1 the texture it perturbs
    for (i, &(f, op)) in [(60u32, 22u32), (60, 23), (61, 23), (62, 23), (63, 22), (64, 22)].iter().enumerate() {
        open(&mut s, &format!("bump-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 {f}\nd9texfill 0 {:#x}\nd9tex 1 16 16 21\nd9texfill 1 {:#x}\nd9settex 0 0\nd9settex 1 1", r.next(), r.next()).unwrap();
        writeln!(s, "tss set 0 1 {op}\ntss set 0 2 2\ntss set 0 4 2\ntss set 1 1 2\ntss set 1 2 2\ntss set 1 4 2\ntss set 1 11 0").unwrap();
        for k in 7..=10 {
            writeln!(s, "tss set 0 {k} {}", fb(r.range(-8, 8) as f32 / 16.0)).unwrap();
        }
        writeln!(s, "tss set 0 22 {}\ntss set 0 23 {}", fb(r.range(0, 8) as f32 / 4.0), fb(r.range(-4, 4) as f32 / 8.0)).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
        writeln!(s, "d9rt\nend").unwrap();
    }
    if !v8 {
        for i in 0..6 {
            open(&mut s, &format!("stage-temp-const-{i}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0\nd9settex 1 0", r.next()).unwrap();
            // (stage 0 into TEMP, stage 1 the diffuse with TEMP and CONSTANT)
            writeln!(s, "tss set 0 1 {}\ntss set 0 2 2\ntss set 0 3 0\ntss set 0 4 2\ntss set 0 28 5\ntss set 0 32 {:#x}", r.pick(&[2u32, 4, 7, 10]), r.next()).unwrap();
            writeln!(s, "tss set 1 1 {}\ntss set 1 2 5\ntss set 1 3 6\ntss set 1 4 4\ntss set 1 5 6\ntss set 1 6 0\ntss set 1 32 {:#x}\ntss set 1 11 0", r.pick(&[2u32, 3, 4, 7, 10, 12, 16]), r.next()).unwrap();
            quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
            writeln!(s, "d9rt\nend").unwrap();
        }
    }
    // filters: magnification and minification apart, anisotropic, a mip filter on one level
    let (mag, min, mip, aniso) = if v8 { ("tss set 0 16", "tss set 0 17", "tss set 0 18", "tss set 0 21") } else { ("ss set 0 5", "ss set 0 6", "ss set 0 7", "ss set 0 10") };
    for (i, (a, b, c)) in [(1u32, 2u32, 0u32), (2, 1, 0), (3, 3, 0), (1, 1, 2), (2, 2, 1), (3, 2, 2)].iter().enumerate() {
        for n in [4u32, 32] {
            open(&mut s, &format!("filter2-{i}-{n}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 {n} {n} 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
            texture_stage(&mut s);
            writeln!(s, "{mag} {a}\n{min} {b}\n{mip} {c}\n{aniso} 4").unwrap();
            let k = r.range(1, 7) as f32 / 4.0;
            writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, Some((0.0, 0.0))), v(15.5, -0.5, 0.0, !0, Some((k, 0.1))), v(-0.5, 15.5, 0.0, !0, Some((0.2, k))), v(15.5, 15.5, 0.0, !0, Some((k, k)))).unwrap();
        }
    }
    // blend operations past MAX, Direct3D 8's BLENDFACTOR state, a stride past the FVF's size
    // (the x86 executor does not draw them: D3D9.BLEND.OPX86)
    for op in [0u32, 6, 7, 0xffff_ffff] {
        blend_sized(&mut s, dev, &mut r, &format!("blend-badop-{op}"), &[(19, 5), (20, 6), (171, op)], 16);
    }
    if v8 {
        blend_sized(&mut s, dev, &mut r, "blend-factor8", &[(19, 14), (20, 15)], 16);
    }
    for i in 0..4 {
        open(&mut s, &format!("stride-{i}"), 21, 16, 16);
        s.push_str("d9clear 1 0 0 0\nrs set 22 1\n");
        let mut l = format!("d9fvf {FVF_TL:#x}\nd9draw 4 3");
        for (x, y) in [(-0.5f32, -0.5f32), (15.5, -0.5), (-0.5, 15.5)] {
            write!(l, " {} {} 0x0 0x3f800000 {:#x}", fb(x), fb(y), r.next()).unwrap();
            for _ in 0..=i {
                write!(l, " {:#x}", r.next()).unwrap();
            }
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // palette textures: P8 and A8P8 with a palette of random entries (flags as alpha)
    for (f, name) in [(41u32, "P8"), (40, "A8P8")] {
        open(&mut s, &format!("palette-{name}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 {f}\nd9texfill 0 {:#x}\nd9palette {:#x}\nd9settex 0 0", r.next(), r.next()).unwrap();
        texture_stage(&mut s);
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
        writeln!(s, "d9rt\nend").unwrap();
    }
    // values outside the enumerations: address modes 0 and 6, cull modes 0 and 4
    for mode in [0u32, 6] {
        open(&mut s, &format!("address-{mode}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 4 4 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        if v8 {
            writeln!(s, "tss set 0 13 {mode}\ntss set 0 14 {mode}\ntss set 0 15 0xff123456").unwrap();
        } else {
            writeln!(s, "ss set 0 1 {mode}\nss set 0 2 {mode}\nss set 0 4 0xff123456").unwrap();
        }
        writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}", v(-0.5, -0.5, 0.0, 0xffff_ffff, Some((-1.5, -1.5))), v(15.5, -0.5, 0.0, 0xffff_ffff, Some((2.5, -1.5))), v(-0.5, 15.5, 0.0, 0xffff_ffff, Some((-1.5, 2.5))), v(15.5, 15.5, 0.0, 0xffff_ffff, Some((2.5, 2.5)))).unwrap();
        writeln!(s, "d9rt\nend").unwrap();
    }
    for mode in [0u32, 4] {
        open(&mut s, &format!("cull-{mode}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 {mode}").unwrap();
        tri(&mut s, [(1.0, 1.0), (7.0, 1.0), (1.0, 7.0)], [0xffff_0000; 3], 0.0);
        tri(&mut s, [(9.0, 9.0), (9.0, 15.0), (15.0, 9.0)], [0xff00_ff00; 3], 0.0);
        writeln!(s, "d9rt\nend").unwrap();
    }
    // blending on targets of other formats (each kind of factor), ALPHAOP DISABLE under an enabled COLOROP
    let mut r = XorShift32(0xb1e0_f0a7);
    for &(f, name) in TARGET_FORMATS.iter().filter(|f| f.0 != 21 && fmts9(f)) {
        for (k, (a, b)) in [(5u32, 6u32), (2, 2), (9, 4), (3, 8), (10, 7)].iter().enumerate() {
            blend_on(&mut s, dev, &mut r, &format!("blend-fmt-{name}-{k}"), &[(19, *a), (20, *b)], 16, f);
        }
    }
    for i in 0..4 {
        open(&mut s, &format!("alphadisable-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 {:#x} 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next(), r.next()).unwrap();
        writeln!(s, "tss set 0 1 {}\ntss set 0 2 2\ntss set 0 3 0\ntss set 0 4 1\nrs set 27 {}\nrs set 19 5\nrs set 20 6", r.pick(&[2u32, 4, 7]), i % 2).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // ALPHAOP DISABLE under every color operation, its arguments the texture, the diffuse color,
    // the texture factor
    for (i, &(op, _)) in TOPS.iter().enumerate() {
        open(&mut s, &format!("alphadisable-op-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 {:#x} 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0\nrs set 60 {:#x}", r.next(), r.next(), r.next()).unwrap();
        writeln!(s, "tss set 0 1 {op}\ntss set 0 2 {}\ntss set 0 3 {}\ntss set 0 26 {}\ntss set 0 4 1", r.pick(&[2u32, 0, 3]), r.pick(&[0u32, 2, 3]), r.pick(&[0u32, 2, 3])).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // ALPHAOP DISABLE on stage 1 under stage 0's MODULATE (whose alpha the texture's times the
    // diffuse one), stage 1 reading CURRENT, the texture, the texture factor
    for (i, &(op, _)) in TOPS.iter().enumerate().filter(|(_, t)| !matches!(t.0, 1 | 17 | 22 | 23)) {
        open(&mut s, &format!("alphadisable-stage1-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 {:#x} 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0\nd9settex 1 0\nrs set 60 {:#x}", r.next(), r.next(), r.next()).unwrap();
        writeln!(s, "tss set 0 1 4\ntss set 0 2 2\ntss set 0 3 0\ntss set 0 4 4\ntss set 0 5 2\ntss set 0 6 0").unwrap();
        writeln!(s, "tss set 1 1 {op}\ntss set 1 2 {}\ntss set 1 3 {}\ntss set 1 26 {}\ntss set 1 4 1\ntss set 1 11 0", r.pick(&[1u32, 2, 3]), r.pick(&[1u32, 2, 3]), r.pick(&[1u32, 2, 3])).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // which triangles are clipped: one vertex k/8 pixel past -0.5 or size - 0.5 on one side,
    // the others inside, Gouraud shaded over a 32 x 32 target (a clipped triangle's pieces shade a
    // few pixels one off from the whole triangle's)
    let mut r = XorShift32(0xc11d_ec1d);
    for (side, name) in ["left", "right", "top", "bottom"].iter().enumerate() {
        for k in 0..=32 {
            open(&mut s, &format!("clip-decide-{name}-{k}"), 21, 32, 32);
            s.push_str("d9clear 1 0 0 0\nrs set 22 1\n");
            let d = k as f32 / 8.0;
            let along = r.range(0, 31 * 16) as f32 / 16.0;
            let out = match side {
                0 => (-0.5 - d, along),
                1 => (31.5 + d, along),
                2 => (along, -0.5 - d),
                _ => (along, 31.5 + d),
            };
            let mut inner = || (r.range(0, 31 * 16) as f32 / 16.0, r.range(0, 31 * 16) as f32 / 16.0);
            let p = [out, inner(), inner()];
            let c = [r.next(), r.next(), r.next()];
            tri(&mut s, p, c, 0.0);
            writeln!(s, "d9rt\nend").unwrap();
        }
    }
    // linear filtering at texture coordinates that are not a whole number of 16ths of a texel
    let mut r = XorShift32(0x51f1_7e2d);
    for i in 0..8 {
        open(&mut s, &format!("filter-uv-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 8 8 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        if v8 {
            s.push_str("tss set 0 16 2\ntss set 0 17 2\n");
        } else {
            s.push_str("ss set 0 5 2\nss set 0 6 2\n");
        }
        let mut c = || r.range(-300, 1300) as f32 / 997.0;
        let (u0, v0, u1, v1) = (c(), c(), c(), c());
        writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, 0xffff_ffff, Some((u0, v0))), v(15.5, -0.5, 0.0, 0xffff_ffff, Some((u1, v0))), v(-0.5, 15.5, 0.0, 0xffff_ffff, Some((u0, v1))), v(15.5, 15.5, 0.0, 0xffff_ffff, Some((u1, v1)))).unwrap();
    }
    // blending's rounding over (nearly) every pair of 8-bit values: 256 x 256 random texels
    let mut r = XorShift32(0x6b1e_7d03);
    for (name, st) in [("src", [(19, 5), (20, 1), (171, 1)]), ("dest", [(19, 1), (20, 5), (171, 1)]), ("lerp", [(19, 5), (20, 6), (171, 1)]), ("sum", [(19, 3), (20, 9), (171, 1)]), ("sub", [(19, 5), (20, 7), (171, 2)])] {
        blend_sized(&mut s, dev, &mut r, &format!("blend-all-{name}"), &st, 256);
    }
    if !v8 {
        // BOTHSRCALPHA / BOTHINVSRCALPHA as the separate alpha's source factor
        for f in [12, 13] {
            blend_sized(&mut s, dev, &mut r, &format!("blend-bothalpha-{f}"), &[(206, 1), (207, f), (208, 2), (209, 1)], 16);
        }
    }
    // lines, one segment a case (D3D9.DRAW.POINTSLINES): 45-degree ones along the pixels' diamond
    // edges and through their corners, both ways, at each quarter offset; horizontal and vertical
    // ones on pixel edges and between them; lines between random quarter-pixel points
    let line = |s: &mut String, name: &str, a: (f32, f32), b: (f32, f32)| {
        open(s, name, 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9fvf {FVF_TL:#x}\nd9draw 2 2 {} {}\nd9rt\nend", v(a.0, a.1, 0.0, 0xffff_ffff, None), v(b.0, b.1, 0.0, 0xffff_ffff, None)).unwrap();
    };
    for k in 0..8 {
        let o = k as f32 / 4.0;
        line(&mut s, &format!("line-diag-{k}"), (1.0 + o, 1.5), (13.0 + o, 13.5));
        line(&mut s, &format!("line-diag-back-{k}"), (13.0 + o, 13.5), (1.0 + o, 1.5));
        line(&mut s, &format!("line-anti-{k}"), (14.0 - o, 1.5), (2.0 - o, 13.5));
        line(&mut s, &format!("line-anti-back-{k}"), (2.0 - o, 13.5), (14.0 - o, 1.5));
    }
    for k in 0..4 {
        let o = k as f32 / 4.0;
        line(&mut s, &format!("line-h-{k}"), (1.25, 7.0 + o), (14.75, 7.0 + o));
        line(&mut s, &format!("line-h-back-{k}"), (14.75, 7.0 + o), (1.25, 7.0 + o));
        line(&mut s, &format!("line-v-{k}"), (7.0 + o, 1.25), (7.0 + o, 14.75));
        line(&mut s, &format!("line-v-back-{k}"), (7.0 + o, 14.75), (7.0 + o, 1.25));
    }
    let mut r = XorShift32(0x11e5_d1a6);
    for i in 0..48 {
        let mut q = || r.range(-8, 72) as f32 / 4.0;
        let (a, b) = ((q(), q()), (q(), q()));
        line(&mut s, &format!("line-random-{i}"), a, b);
    }
    // (D3D9.DRAW.POINTSLINES) lines from past the target's edge to a point inside; 45-degree lines
    // along diamond edges of each length from 1 to 12, both ways, on both diagonals
    let mut r = XorShift32(0x11e5_c11b);
    for i in 0..48 {
        let mut q = |lo: i64, hi: i64| r.range(lo, hi) as f32 / 4.0;
        let a = match i % 4 {
            0 => (q(-12, -3), q(-4, 68)),
            1 => (q(63, 76), q(-4, 68)),
            2 => (q(-4, 68), q(-12, -3)),
            _ => (q(-4, 68), q(63, 76)),
        };
        let b = (q(0, 61), q(0, 61));
        line(&mut s, &format!("line-clip-{i}"), a, b);
    }
    for len in 1..=12 {
        let x = (len % 3) as f32 + 1.0;
        let y = x + 0.5;
        line(&mut s, &format!("line-edge-{len}"), (x, y), (x + len as f32, y + len as f32));
        line(&mut s, &format!("line-edge-back-{len}"), (x + len as f32, y + len as f32), (x, y));
        line(&mut s, &format!("line-edge-anti-{len}"), (14.0 - x, y), (14.0 - x - len as f32, y + len as f32));
        line(&mut s, &format!("line-edge-anti-back-{len}"), (14.0 - x - len as f32, y + len as f32), (14.0 - x, y));
    }
    // fog over more depths and factors (D3D9.FOG: which precision the factor and the blend take)
    let mut r = XorShift32(0x0f09_0002);
    for i in 0..24 {
        open(&mut s, &format!("fog2-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 7 0\nrs set 28 1\nrs set 34 {:#x}\nrs set 35 {}\nrs set 140 0", r.next(), i % 4).unwrap();
        writeln!(s, "rs set 36 {}\nrs set 37 {}\nrs set 38 {}\nd9fvf 0xc4", fb(r.range(0, 40) as f32 / 97.0), fb(0.45 + r.range(0, 50) as f32 / 89.0), fb(0.3 + r.range(0, 300) as f32 / 71.0)).unwrap();
        let mut l = "d9draw 4 3".to_string();
        for (x, y) in [(-0.5f32, -0.5f32), (15.5, -0.5), (-0.5, 15.5)] {
            write!(l, " {} {} {} {} {:#x} {:#x}", fb(x), fb(y), fb(r.range(0, 1000) as f32 / 999.0), fb(1.0), r.next(), r.next()).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // anisotropic filtering on one level: a footprint of known axes (texels per pixel along x
    // and along y of the screen), each anisotropy limit, the magnifying filter apart
    for i in 0..32 {
        open(&mut s, &format!("aniso-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 32 32 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        let th = [0.0f32, 0.25, 0.5, 0.785, 1.2][r.range(0, 4) as usize];
        let big = [1.25f32, 1.5, 2.0, 3.0, 4.0, 6.0][r.range(0, 5) as usize];
        let small = [0.5f32, 0.75, 1.0, 1.25][r.range(0, 3) as usize].min(big);
        let (mx, my) = if i % 2 == 0 { (big, small) } else { (small, big) };
        let (dx, dy) = ((mx * th.cos(), mx * th.sin()), (-my * th.sin(), my * th.cos()));
        let (fmag, fmip) = ([1u32, 2, 3][r.range(0, 2) as usize], if i % 8 == 7 { 2 } else { 0 });
        writeln!(s, "{mag} {fmag}\n{min} 3\n{mip} {fmip}\n{aniso} {}", [1u32, 2, 3, 4, 8, 16][r.range(0, 5) as usize]).unwrap();
        // (16 pixels a side, 32 texels: a texture coordinate step of d / 2 per side)
        let uv = |a: f32, b: f32| Some((0.3 + (dx.0 * a + dy.0 * b) / 2.0, 0.2 + (dx.1 * a + dy.1 * b) / 2.0));
        writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, uv(0.0, 0.0)), v(15.5, -0.5, 0.0, !0, uv(1.0, 0.0)), v(-0.5, 15.5, 0.0, !0, uv(0.0, 1.0)), v(15.5, 15.5, 0.0, !0, uv(1.0, 1.0))).unwrap();
    }
    // the other D3DTEXTUREFILTERTYPE values (NONE, PYRAMIDALQUAD, GAUSSIANQUAD,
    // CONVOLUTIONMONO, past them) as the magnifying filter, magnifying and minifying
    for f in [0u32, 4, 5, 6, 7, 0xffff_ffff] {
        for n in [4u32, 32] {
            open(&mut s, &format!("filter-other-{f}-{n}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 {n} {n} 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
            texture_stage(&mut s);
            writeln!(s, "{mag} {f:#x}\n{min} 2\n{mip} 0").unwrap();
            writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, Some((0.0, 0.0))), v(15.5, -0.5, 0.0, !0, Some((1.3, 0.1))), v(-0.5, 15.5, 0.0, !0, Some((0.2, 1.1))), v(15.5, 15.5, 0.0, !0, Some((1.5, 1.2)))).unwrap();
        }
    }
    // a bump mapping stage after another and before another: what CURRENT holds after it
    for i in 0..12u32 {
        open(&mut s, &format!("bump-chain-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9tex 1 16 16 60\nd9texfill 1 {:#x}\nd9tex 2 16 16 21\nd9texfill 2 {:#x}", r.next(), r.next(), r.next()).unwrap();
        writeln!(s, "{}\nd9settex 1 1\nd9settex 2 2", if i % 3 == 2 { "d9settex 0 none" } else { "d9settex 0 0" }).unwrap();
        let op0 = [4u32, 3, 7, 2][(i % 4) as usize];
        let op2 = [2u32, 4, 3][(i / 4) as usize];
        writeln!(s, "tss set 0 1 {op0}\ntss set 0 4 {op0}\ntss set 1 1 {}\ntss set 1 4 2\ntss set 2 1 {op2}\ntss set 2 2 2\ntss set 2 3 1\ntss set 2 4 {op2}\ntss set 2 5 2\ntss set 2 6 1", 22 + i % 2).unwrap();
        writeln!(s, "tss set 1 11 0\ntss set 2 11 0").unwrap();
        for k in 7..=10 {
            writeln!(s, "tss set 1 {k} {}", fb(r.range(-8, 8) as f32 / 16.0)).unwrap();
        }
        writeln!(s, "tss set 1 22 {}\ntss set 1 23 {}", fb(r.range(0, 8) as f32 / 4.0), fb(r.range(-4, 4) as f32 / 8.0)).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // PREMODULATE before another stage, DOTPRODUCT3 as a later stage's alpha operation
    for i in 0..6u32 {
        open(&mut s, &format!("premod-chain-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9tex 1 16 16 21\nd9texfill 1 {:#x}\nd9settex 0 0\nd9settex 1 1", r.next(), r.next()).unwrap();
        let (op0, op1) = if i < 4 { (17u32, [4u32, 3, 2, 7][i as usize]) } else { (4, 24) };
        writeln!(s, "tss set 0 1 {op0}\ntss set 0 4 {op0}\ntss set 1 1 {}\ntss set 1 2 2\ntss set 1 3 1\ntss set 1 4 {op1}\ntss set 1 5 2\ntss set 1 6 1\ntss set 1 11 0", if i < 4 { op1 } else { 4 }).unwrap();
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // TEXCOORDINDEX: the second coordinate set, one past the vertex's sets, generated ones
    for (i, tci) in [1u32, 2, 0x10000, 0x20000, 0x30000, 0x40000, 0x10001].iter().enumerate() {
        open(&mut s, &format!("texcoordindex-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        writeln!(s, "tss set 0 11 {tci:#x}\nd9fvf 0x244").unwrap();
        let mut l = "d9draw 5 4".to_string();
        for (x, y, u, v) in [(-0.5f32, -0.5f32, 0.0f32, 0.0f32), (15.5, -0.5, 1.0, 0.0), (-0.5, 15.5, 0.0, 1.0), (15.5, 15.5, 1.0, 1.0)] {
            write!(l, " {} {} {} {} {:#x} {} {} {} {}", fb(x), fb(y), fb(0.0), fb(1.0), r.next(), fb(u), fb(v), fb(0.7 - v * 0.6), fb(0.1 + u * 0.8)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // PYRAMIDALQUAD, GAUSSIANQUAD as the minifying filter under a point or linear one
    for (i, (mag, f)) in [(1u32, 4u32), (2, 4), (1, 5), (2, 5)].iter().enumerate() {
        for n in [4u32, 32] {
            open(&mut s, &format!("filter-min-quad-{i}-{n}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 {n} {n} 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
            texture_stage(&mut s);
            writeln!(s, "{mag_s} {mag}\n{min} {f}\n{mip} 0", mag_s = if v8 { "tss set 0 16" } else { "ss set 0 5" }).unwrap();
            writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, Some((0.0, 0.0))), v(15.5, -0.5, 0.0, !0, Some((1.3, 0.1))), v(-0.5, 15.5, 0.0, !0, Some((0.2, 1.1))), v(15.5, 15.5, 0.0, !0, Some((1.5, 1.2)))).unwrap();
        }
    }
    // NONE, CONVOLUTIONMONO and past them as the minifying filter under a linear one
    for f in [0u32, 6, 7, 0xffff_ffff] {
        open(&mut s, &format!("filter-min-other-{f}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 4 4 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        writeln!(s, "{mag_s} 2\n{min} {f:#x}\n{mip} 0", mag_s = if v8 { "tss set 0 16" } else { "ss set 0 5" }).unwrap();
        writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, Some((0.0, 0.0))), v(15.5, -0.5, 0.0, !0, Some((1.3, 0.1))), v(-0.5, 15.5, 0.0, !0, Some((0.2, 1.1))), v(15.5, 15.5, 0.0, !0, Some((1.5, 1.2)))).unwrap();
    }
    // TEXCOORDINDEX differing between two stages (stage 1 modulates stage 0's texture)
    for (i, (a, b)) in [(0u32, 1u32), (1, 0), (0, 2), (2, 1)].iter().enumerate() {
        open(&mut s, &format!("tci-multi-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0\nd9tex 1 16 16 21\nd9texfill 1 {:#x}\nd9settex 1 1", r.next(), r.next()).unwrap();
        texture_stage(&mut s);
        writeln!(s, "tss set 0 11 {a:#x}\ntss set 1 1 4\ntss set 1 2 2\ntss set 1 3 1\ntss set 1 11 {b:#x}\nd9fvf 0x244").unwrap();
        let mut l = "d9draw 5 4".to_string();
        for (x, y, u, v) in [(-0.5f32, -0.5f32, 0.0f32, 0.0f32), (15.5, -0.5, 1.0, 0.0), (-0.5, 15.5, 0.0, 1.0), (15.5, 15.5, 1.0, 1.0)] {
            write!(l, " {} {} {} {} {:#x} {} {} {} {}", fb(x), fb(y), fb(0.0), fb(1.0), r.next(), fb(u), fb(v), fb(0.7 - v * 0.6), fb(0.1 + u * 0.8)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // generated texture coordinates from untransformed vertices (lighting off, identity
    // matrices; no normals in the vertex)
    for (i, tci) in [0x10000u32, 0x20000, 0x30000, 0x40000].iter().enumerate() {
        open(&mut s, &format!("tci-xyz-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 137 0\nrs set 7 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        writeln!(s, "tss set 0 11 {tci:#x}").unwrap();
        let mut l = "d9fvf 0x142\nd9draw 5 4".to_string();
        for (x, y) in [(-0.9f32, 0.9f32), (0.9, 0.9), (-0.9, -0.9), (0.9, -0.9)] {
            write!(l, " {} {} {} {:#x} {} {}", fb(x), fb(y), fb(0.5), r.next() | 0xff00_0000, fb(x * 0.5 + 0.5), fb(0.5 - y * 0.5)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // a stage after the first that reads the texture it lacks, before a bump mapping stage
    // followed by another
    for i in 0..4u32 {
        open(&mut s, &format!("bump-late-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9tex 2 16 16 60\nd9texfill 2 {:#x}\nd9tex 3 16 16 21\nd9texfill 3 {:#x}", r.next(), r.next(), r.next()).unwrap();
        writeln!(s, "d9settex 0 0\nd9settex 1 none\nd9settex 2 2\nd9settex 3 3").unwrap();
        let op1 = [4u32, 2][(i % 2) as usize];
        let op3 = [3u32, 2][(i / 2) as usize];
        writeln!(s, "tss set 0 1 2\ntss set 0 4 2\ntss set 1 1 {op1}\ntss set 1 2 2\ntss set 1 3 1\ntss set 1 4 {op1}\ntss set 1 5 2\ntss set 1 6 1\ntss set 2 1 22\ntss set 2 4 2\ntss set 3 1 {op3}\ntss set 3 2 2\ntss set 3 3 1\ntss set 3 4 {op3}\ntss set 3 5 2\ntss set 3 6 1").unwrap();
        writeln!(s, "tss set 1 11 0\ntss set 2 11 0\ntss set 3 11 0").unwrap();
        for k in 7..=10 {
            writeln!(s, "tss set 2 {k} {}", fb(r.range(-8, 8) as f32 / 16.0)).unwrap();
        }
        quad(&mut s, 0.0, 0.0, 16.0, 16.0, 0.0, r.next());
        writeln!(s, "d9rt\nend").unwrap();
    }
    // a clipped triangle with a vertex at the target's size itself on one axis and outside the
    // low side (or the high one) on the other, as Direct3D 8 clips it
    for i in 0..16u32 {
        open(&mut s, &format!("clip8-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1").unwrap();
        let off = [-0.75f32, -1.0, -2.5, -6.0][(i % 4) as usize];
        let out = if i / 4 % 2 == 0 { off } else { 16.0 - off };
        let a = (out, 16.0);
        let p = [a, (r.range(2, 13) as f32 + 0.25, r.range(1, 6) as f32 + 0.5), (r.range(4, 14) as f32 + 0.75, r.range(9, 14) as f32 + 0.25)];
        let c = 0xff00_0000 | r.next() & 0x3f3f3f | 0x40;
        tri(&mut s, if i / 8 == 0 { p } else { p.map(|(x, y)| (y, x)) }, [c; 3], 0.0);
        writeln!(s, "d9rt\nend").unwrap();
    }
    // ANISOTROPIC minifying with footprints along an axis (x, or y for odd i), so the kernel
    // of texels each pixel weighs comes out exactly: a grid of major and minor axis lengths
    // (texels a pixel) and anisotropy limits
    let mut k = 0;
    for &maj in &[1.5f32, 2.5, 3.0, 3.5, 4.5, 5.0, 6.0, 7.0, 9.0, 12.0] {
        for &(mnr, lim) in &[(0.75f32, 16u32), (1.5, 16), (1.0, 2), (1.0, 4), (1.0, 8)] {
            open(&mut s, &format!("aniso-axis-{k}"), 21, 16, 16);
            writeln!(s, "d9clear 1 0 0 0\nd9tex 0 32 32 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
            texture_stage(&mut s);
            writeln!(s, "{mag} 2\n{min} 3\n{mip} 0\n{aniso} {lim}").unwrap();
            let (dx, dy) = if k % 2 == 0 { ((maj, 0.0f32), (0.0f32, mnr)) } else { ((mnr, 0.0), (0.0, maj)) };
            let uv = |a: f32, b: f32| Some((0.3 + (dx.0 * a + dy.0 * b) / 2.0, 0.2 + (dx.1 * a + dy.1 * b) / 2.0));
            writeln!(s, "d9fvf {FVF_TL_TEX:#x}\nd9draw 5 4 {} {} {} {}\nd9rt\nend", v(-0.5, -0.5, 0.0, !0, uv(0.0, 0.0)), v(15.5, -0.5, 0.0, !0, uv(1.0, 0.0)), v(-0.5, 15.5, 0.0, !0, uv(0.0, 1.0)), v(15.5, 15.5, 0.0, !0, uv(1.0, 1.0))).unwrap();
            k += 1;
        }
    }
    // generated texture coordinates: vertices with normals (0..3), LOCALVIEWER off (4, 5), a
    // world matrix that scales and moves (6, 7)
    let mat = |m: [f32; 16]| m.iter().map(|&x| format!(" {}", fb(x))).collect::<String>();
    for i in 0..8u32 {
        open(&mut s, &format!("tci-gen-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 137 0\nrs set 7 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        let tci = [0x10000u32, 0x20000, 0x30000, 0x40000, 0x30000, 0x40000, 0x20000, 0x30000][i as usize];
        writeln!(s, "tss set 0 11 {tci:#x}").unwrap();
        if i == 4 || i == 5 {
            writeln!(s, "rs set 142 0").unwrap();
        }
        if i >= 6 {
            writeln!(s, "d9xform 256{}", mat([0.8, 0.0, 0.0, 0.0, 0.0, 0.7, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.1, -0.05, 0.3, 1.0])).unwrap();
        }
        let normals = i < 4;
        let mut l = format!("d9fvf {:#x}\nd9draw 5 4", if normals { 0x152 } else { 0x142 });
        for (x, y) in [(-0.9f32, 0.9f32), (0.9, 0.9), (-0.9, -0.9), (0.9, -0.9)] {
            write!(l, " {} {} {}", fb(x), fb(y), fb(0.5)).unwrap();
            if normals {
                let (nx, ny) = (r.range(-8, 8) as f32 / 10.0, r.range(-8, 8) as f32 / 10.0);
                let nz = -(1.0 - (nx * nx + ny * ny).min(1.0)).sqrt();
                write!(l, " {} {} {}", fb(nx), fb(ny), fb(nz)).unwrap();
            }
            write!(l, " {:#x} {} {}", r.next() | 0xff00_0000, fb(x * 0.5 + 0.5), fb(0.5 - y * 0.5)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    // generated coordinates further: normals under a world matrix (0, 1), NORMALIZENORMALS
    // with normals of other lengths (2, 3), a texture matrix by TEXTURETRANSFORMFLAGS COUNT2 on
    // generated (4) and passed (5) coordinates
    for i in 0..6u32 {
        open(&mut s, &format!("tci-gen2-{i}"), 21, 16, 16);
        writeln!(s, "d9clear 1 0 0 0\nrs set 22 1\nrs set 137 0\nrs set 7 0\nd9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9settex 0 0", r.next()).unwrap();
        texture_stage(&mut s);
        let tci = [0x10000u32, 0x30000, 0x10000, 0x40000, 0x20000, 0][i as usize];
        writeln!(s, "tss set 0 11 {tci:#x}").unwrap();
        if i < 2 {
            writeln!(s, "d9xform 256{}", mat([0.6, 0.2, 0.0, 0.0, -0.3, 0.8, 0.0, 0.0, 0.0, 0.0, 1.5, 0.0, 0.1, 0.0, 0.2, 1.0])).unwrap();
        }
        if i == 2 || i == 3 {
            writeln!(s, "rs set 143 1").unwrap();
        }
        if i >= 4 {
            writeln!(s, "tss set 0 24 2\nd9xform 16{}", mat([0.5, 0.25, 0.0, 0.0, -0.25, 0.75, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.3, 0.1, 0.0, 1.0])).unwrap();
        }
        let mut l = "d9fvf 0x152\nd9draw 5 4".to_string();
        for (x, y) in [(-0.9f32, 0.9f32), (0.9, 0.9), (-0.9, -0.9), (0.9, -0.9)] {
            let (nx, ny) = (r.range(-8, 8) as f32 / 10.0, r.range(-8, 8) as f32 / 10.0);
            let nz = -(1.0 - (nx * nx + ny * ny).min(1.0)).sqrt();
            let k = if i == 2 || i == 3 { 0.5 + r.range(0, 8) as f32 / 4.0 } else { 1.0 };
            write!(l, " {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(nx * k), fb(ny * k), fb(nz * k)).unwrap();
            write!(l, " {:#x} {} {}", r.next() | 0xff00_0000, fb(x * 0.5 + 0.5), fb(0.5 - y * 0.5)).unwrap();
        }
        writeln!(s, "{l}\nd9rt\nend").unwrap();
    }
    s
}

/// blending: the target holds a random texture (drawn first), a second one is blended over it
fn blend_sized(s: &mut String, dev: &str, r: &mut XorShift32, name: &str, states: &[(u32, u32)], n: u32) {
    blend_on(s, dev, r, name, states, n, 21);
}

fn blend_on(s: &mut String, dev: &str, r: &mut XorShift32, name: &str, states: &[(u32, u32)], n: u32, fmt: u32) {
    writeln!(s, "case {name}\n{dev} swvp\nd9target {n} {n} {fmt}").unwrap();
    writeln!(s, "d9tex 0 {n} {n} 21\nd9texfill 0 {:#x}\nd9tex 1 {n} {n} 21\nd9texfill 1 {:#x}\nd9settex 0 0", r.next(), r.next()).unwrap();
    texture_stage(s);
    let f = n as f32;
    quad(s, 0.0, 0.0, f, f, 0.0, 0xffff_ffff);
    writeln!(s, "d9settex 0 1\nrs set 27 1").unwrap();
    for (k, v) in states {
        writeln!(s, "rs set {k} {v:#x}").unwrap();
    }
    quad(s, 0.0, 0.0, f, f, 0.0, 0xffff_ffff);
    writeln!(s, "d9rt\nend").unwrap();
}

/// FVF codes asked of GetFVF / GetVertexDeclaration and D3DX: the common ones, every position
/// kind, the blend weights, texture coordinate counts and sizes
pub const FVFS: &[u32] = &[
    0x002, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e, 0x4002, 0x012, 0x042, 0x044, 0x082, 0x0c2, 0x0c4, 0x102, 0x144, 0x152, 0x1c4, 0x202, 0x302, 0x402, 0x802,
    0x10002 | 0x100, 0x20002 | 0x100, 0x30002 | 0x100, 0x00030002 | 0x200, 0x10000 | 0x2 | 0x100, 0x1000 | 0x00c, 0x1000 | 0x006, 0x1002, 0x2002, 0x4012, 0x8002,
    0x0, 0x001, 0x010, 0x020, 0x40, 0x100, 0x00e | 0x1000 | 0x100,
];
