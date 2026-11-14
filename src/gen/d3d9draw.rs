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

    // blending: the target holds a random texture (drawn first), a second one is blended over it
    let blend_case = |s: &mut String, r: &mut XorShift32, name: &str, states: &[(u32, u32)]| {
        open(s, name, 21, 16, 16);
        writeln!(s, "d9tex 0 16 16 21\nd9texfill 0 {:#x}\nd9tex 1 16 16 21\nd9texfill 1 {:#x}\nd9settex 0 0", r.next(), r.next()).unwrap();
        texture_stage(s);
        quad(s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
        writeln!(s, "d9settex 0 1\nrs set 27 1").unwrap();
        for (k, v) in states {
            writeln!(s, "rs set {k} {v:#x}").unwrap();
        }
        quad(s, 0.0, 0.0, 16.0, 16.0, 0.0, 0xffff_ffff);
        writeln!(s, "d9rt\nend").unwrap();
    };
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
            // (Clear ignores the scissor rectangle)
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
    s
}

/// FVF codes asked of GetFVF / GetVertexDeclaration and D3DX: the common ones, every position
/// kind, the blend weights, texture coordinate counts and sizes
pub const FVFS: &[u32] = &[
    0x002, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e, 0x4002, 0x012, 0x042, 0x044, 0x082, 0x0c2, 0x0c4, 0x102, 0x144, 0x152, 0x1c4, 0x202, 0x302, 0x402, 0x802,
    0x10002 | 0x100, 0x20002 | 0x100, 0x30002 | 0x100, 0x00030002 | 0x200, 0x10000 | 0x2 | 0x100, 0x1000 | 0x00c, 0x1000 | 0x006, 0x1002, 0x2002, 0x4012, 0x8002,
    0x0, 0x001, 0x010, 0x020, 0x40, 0x100, 0x00e | 0x1000 | 0x100,
];
