//! The d3d10draw and d3d11draw suites' case generators: triangles through the rasterizer
//! (coverage, culling, scissor, viewports), the pixel shader's system values, depth tests,
//! blending, and textures read by ld and sample (how every format decodes).
use super::dxbc::*;
use super::*;
use crate::model::dxgifmt::{format, Num, FORMATS};

/// the programs the cases use, by name (the model knows each one by its bytes)
pub fn shaders() -> Vec<(&'static str, Vec<u8>)> {
    let f4 = |name, idx, reg, used| sig(name, idx, 0, 3, reg, 0xf, used);
    let pos_out = sig("SV_Position", 0, 1, 3, 0, 0xf, 0);
    let target = [sig("SV_Target", 0, 0, 3, 0, 0xf, 0)];
    let target_u = [sig("SV_Target", 0, 0, 1, 0, 0xf, 0)];
    let ret = ins(RET, &[]);
    let mov = |d: [u32; 2], s: [u32; 2]| ins(MOV, &[d, s].concat());
    let mut v = Vec::new();
    // vs: TEXCOORD0 position, TEXCOORD1 color, passed on
    v.push(("vs", blob(1, &[f4("TEXCOORD", 0, 0, 0xf), f4("TEXCOORD", 1, 1, 0xf)], &[pos_out, f4("TEXCOORD", 0, 1, 0)], &cat(&[
        ins(DCL_INPUT, &dst(INPUT, 0, 0xf)),
        ins(DCL_INPUT, &dst(INPUT, 1, 0xf)),
        ins(DCL_OUTPUT_SIV, &[&dst(OUTPUT, 0, 0xf)[..], &[1]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 1, 0xf)),
        mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)),
        mov(dst(OUTPUT, 1, 0xf), src(INPUT, 1, XYZW)),
        ret.clone(),
    ]))));
    // vsid: TEXCOORD0 position; the color is (vertex id, instance id, 0, 1) as floats
    v.push(("vsid", blob(1, &[f4("TEXCOORD", 0, 0, 0xf), sig("SV_VertexID", 0, 6, 1, 1, 1, 1), sig("SV_InstanceID", 0, 8, 1, 2, 1, 1)], &[pos_out, f4("TEXCOORD", 0, 1, 0)], &cat(&[
        ins(DCL_INPUT, &dst(INPUT, 0, 0xf)),
        ins(DCL_INPUT_SGV, &[&dst(INPUT, 1, 1)[..], &[6]].concat()),
        ins(DCL_INPUT_SGV, &[&dst(INPUT, 2, 1)[..], &[8]].concat()),
        ins(DCL_OUTPUT_SIV, &[&dst(OUTPUT, 0, 0xf)[..], &[1]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 1, 0xf)),
        mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)),
        ins(UTOF, &[dst(OUTPUT, 1, 1), src(INPUT, 1, 0)].concat()),
        ins(UTOF, &[dst(OUTPUT, 1, 2), src(INPUT, 2, 0)].concat()),
        ins(MOV, &[&dst(OUTPUT, 1, 0xc)[..], &imm([0, 0, 0, 0x3f80_0000])].concat()),
        ret.clone(),
    ]))));
    let ps_in = |interp: u32, used: u32| (interp, [sig("SV_Position", 0, 1, 3, 0, 0xf, used), f4("TEXCOORD", 0, 1, 0xf)]);
    // ps: the color, interpolated (linear) / flat (constant)
    for (name, interp) in [("ps", LINEAR), ("psflat", CONSTANT)] {
        let (i, isgn) = ps_in(interp, 0);
        v.push((name, blob(0, &isgn, &target, &cat(&[ins(DCL_INPUT_PS | i, &dst(INPUT, 1, 0xf)), ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)), mov(dst(OUTPUT, 0, 0xf), src(INPUT, 1, XYZW)), ret.clone()]))));
    }
    // pspos: SV_Position
    let (_, isgn) = ps_in(0, 0xf);
    v.push(("pspos", blob(0, &isgn, &target, &cat(&[ins(DCL_INPUT_PS_SIV | NOPERSPECTIVE, &[&dst(INPUT, 0, 0xf)[..], &[1]].concat()), ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)), mov(dst(OUTPUT, 0, 0xf), src(INPUT, 0, XYZW)), ret.clone()]))));
    // psprim: (SV_PrimitiveID, SV_IsFrontFace & 1.0, 0, 1)
    // (the system generated values after what the vertex shader gives, as fxc places them)
    let isgn = [sig("SV_Position", 0, 1, 3, 0, 0xf, 0), f4("TEXCOORD", 0, 1, 0), sig("SV_PrimitiveID", 0, 7, 1, 2, 1, 1), sig("SV_IsFrontFace", 0, 9, 1, 3, 1, 1)];
    v.push(("psprim", blob(0, &isgn, &target, &cat(&[
        ins(DCL_INPUT_PS_SGV | CONSTANT, &[&dst(INPUT, 2, 1)[..], &[7]].concat()),
        ins(DCL_INPUT_PS_SGV | CONSTANT, &[&dst(INPUT, 3, 1)[..], &[9]].concat()),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(UTOF, &[dst(OUTPUT, 0, 1), src(INPUT, 2, 0)].concat()),
        ins(AND, &[&dst(OUTPUT, 0, 2)[..], &src(INPUT, 3, 0), &imm([0x3f80_0000; 4])].concat()),
        ins(MOV, &[&dst(OUTPUT, 0, 0xc)[..], &imm([0, 0, 0, 0x3f80_0000])].concat()),
        ret.clone(),
    ]))));
    // psld / psldu: texture 0 loaded at the pixel (mip 0), to a float / uint target
    let (_, isgn) = ps_in(0, 3);
    for (name, osgn) in [("psld", &target), ("psldu", &target_u)] {
        let ret_ty = if name == "psld" { 0x5555 } else { 0x4444 };
        v.push((name, blob(0, &isgn, osgn, &cat(&[
            ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, ret_ty]),
            ins(DCL_INPUT_PS_SIV | NOPERSPECTIVE, &[&dst(INPUT, 0, 3)[..], &[1]].concat()),
            ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
            ins(DCL_TEMPS, &[1]),
            ins(FTOU, &[dst(TEMP, 0, 3), src(INPUT, 0, swz(0, 1, 0, 0))].concat()),
            ins(MOV, &[&dst(TEMP, 0, 0xc)[..], &imm([0; 4])].concat()),
            ins(LD, &[dst(OUTPUT, 0, 0xf), src(TEMP, 0, XYZW), src(RESOURCE, 0, XYZW)].concat()),
            ret.clone(),
        ]))));
    }
    // pssample: texture 0 sampled with sampler 0 at the color's xy
    let (_, isgn) = ps_in(0, 0);
    let isgn = [isgn[0], f4("TEXCOORD", 0, 1, 3)];
    v.push(("pssample", blob(0, &isgn, &target, &cat(&[
        ins(DCL_SAMPLER, &[SAMPLER << 12 | 1 << 20, 0]),
        ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, 0x5555]),
        ins(DCL_INPUT_PS | LINEAR, &dst(INPUT, 1, 3)),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(SAMPLE, &[&dst(OUTPUT, 0, 0xf)[..], &src(INPUT, 1, swz(0, 1, 0, 0)), &src(RESOURCE, 0, XYZW), &[SAMPLER << 12 | 1 << 20, 0]].concat()),
        ret.clone(),
    ]))));
    // pssamplec: texture 0 compared with sampler 0 (comparison mode) at the color's xy against
    // its z, the result in every channel
    let isgn = [isgn[0], f4("TEXCOORD", 0, 1, 7)];
    v.push(("pssamplec", blob(0, &isgn, &target, &cat(&[
        ins(DCL_SAMPLER | 1 << 11, &[SAMPLER << 12 | 1 << 20, 0]),
        ins(DCL_RESOURCE | 3 << 11, &[RESOURCE << 12 | 1 << 20, 0, 0x5555]),
        ins(DCL_INPUT_PS | LINEAR, &dst(INPUT, 1, 7)),
        ins(DCL_OUTPUT, &dst(OUTPUT, 0, 0xf)),
        ins(DCL_TEMPS, &[1]),
        ins(SAMPLE_C, &[&dst(TEMP, 0, 1)[..], &src(INPUT, 1, swz(0, 1, 0, 0)), &src(RESOURCE, 0, XYZW), &[SAMPLER << 12 | 1 << 20, 0], &src1(INPUT, 1, 2)].concat()),
        mov(dst(OUTPUT, 0, 0xf), src(TEMP, 0, 0)),
        ret.clone(),
    ]))));
    v
}

pub fn shader(name: &str) -> Vec<u8> {
    shaders().into_iter().find(|s| s.0 == name).unwrap().1
}

fn fb(v: f32) -> String {
    let b = v.to_bits();
    if b < 10 {
        b.to_string()
    } else {
        format!("{b:#x}")
    }
}

/// a vertex of the vs program: position (x, y in pixels of a `w` x `h` viewport at 0,0, z),
/// color
fn vertex(s: &mut String, x: f32, y: f32, z: f32, c: [f32; 4], w: f32, h: f32) {
    let (nx, ny) = (x / w * 2.0 - 1.0, 1.0 - y / h * 2.0);
    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx), fb(ny), fb(z), fb(1.0), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
}

/// what every drawing case starts with: the device, a w x h render target of `fmt` cleared to
/// 0 in slot 0 (view 0), the vs and `ps` programs, the layout, a viewport over the target, a
/// rasterizer state that culls nothing
fn begin(s: &mut String, dev: &str, name: &str, w: u32, h: u32, fmt: u32, ps: &str) {
    writeln!(s, "case {name}\n{dev}").unwrap();
    writeln!(s, "tex 0 2 {w} {h} 1 1 {fmt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader(ps))).unwrap();
    writeln!(s, "viewport 0 0 {} {} 0 {}", fb(w as f32), fb(h as f32), fb(1.0)).unwrap();
    writeln!(s, "rast 3 1 0 0 0 0 1 0 0 0").unwrap();
}

fn read_rt(s: &mut String, slot: u32, fmt: u32, w: u32, h: u32) {
    let f = format(fmt).unwrap();
    writeln!(s, "read {slot} 0 {} {h} 1", f.row_bytes(w)).unwrap();
}

/// an 8-bit color that no interpolation error can round elsewhere
fn color(i: u32) -> [f32; 4] {
    let c = |k: u32| (k % 256) as f32 / 255.0;
    [c(i * 37 + 11), c(i * 91 + 5), c(i * 53 + 200), 1.0]
}

pub fn d3d11draw() -> String {
    suite(true)
}
pub fn d3d10draw() -> String {
    suite(false)
}

fn suite(d3d11: bool) -> String {
    let name = if d3d11 { "d3d11draw" } else { "d3d10draw" };
    let dev = if d3d11 { "device11" } else { "device10" };
    let mut s = format!("# gpu\n# kikizake suite {name}: Direct3D {} drawing (SPEC.md). Made by `kikizake gen {name}`.\n", if d3d11 { 11 } else { 10 });
    let mut r = XorShift32::new(if d3d11 { 0xd4a1_0011 } else { 0xd4a1_0010 });
    // a coordinate on the 1/256 pixel grid, around and inside 0..16
    let coord = |r: &mut XorShift32, sub: bool| -> f32 {
        let px = r.range(-3, 19) as f32;
        let f = if sub { *r.pick(&[0, 0, 128, 127, 129, 1, 255, 64, 192]) } else { r.range(0, 255) as i32 } as f32;
        px + f / 256.0
    };
    // random triangles, one color each, on a 16x16 target
    for i in 0..200 {
        begin(&mut s, dev, &format!("tri-{i}"), 16, 16, 28, "ps");
        let n = r.range(1, 6) as u32;
        for t in 0..n {
            let sub = i % 2 == 0;
            for _ in 0..3 {
                vertex(&mut s, coord(&mut r, sub), coord(&mut r, sub), 0.5, color(i * 7 + t), 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 {} 32", n * 3).unwrap();
        read_rt(&mut s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // a grid of quads (two triangles each, sharing the diagonal), every pixel covered once:
    // the fill rule on shared edges; corners on pixel centers and between them
    for (gi, (step, off)) in [(4.0f32, 0.0f32), (4.0, 0.5), (3.0, 0.25), (2.5, 0.5), (4.0, 1.0 / 256.0), (5.0, 0.5 - 1.0 / 256.0)].iter().enumerate() {
        begin(&mut s, dev, &format!("grid-{gi}"), 16, 16, 28, "ps");
        let mut n = 0;
        let mut k = 0;
        let mut y = off - step;
        while y < 16.0 {
            let mut x = off - step;
            while x < 16.0 {
                for (a, b, c) in [((x, y), (x + step, y), (x, y + step)), ((x + step, y), (x + step, y + step), (x, y + step))] {
                    for p in [a, b, c] {
                        vertex(&mut s, p.0, p.1, 0.5, color(k), 16.0, 16.0);
                    }
                    n += 3;
                    k += 1;
                }
                x += step;
            }
            y += step;
            if n > 400 {
                writeln!(s, "draw 4 {n} 32").unwrap();
                n = 0;
            }
        }
        if n > 0 {
            writeln!(s, "draw 4 {n} 32").unwrap();
        }
        read_rt(&mut s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // the same grids with additive blending: a pixel drawn twice shows the sum
    for (gi, (step, off)) in [(4.0f32, 0.5f32), (3.0, 0.25)].iter().enumerate() {
        begin(&mut s, dev, &format!("gridadd-{gi}"), 16, 16, 28, "ps");
        writeln!(s, "blend1 1 2 2 1 2 2 1 0xf").unwrap();
        let mut y = off - step;
        while y < 16.0 {
            let mut x = off - step;
            let mut n = 0;
            while x < 16.0 {
                for (a, b, c) in [((x, y), (x + step, y), (x, y + step)), ((x + step, y), (x + step, y + step), (x, y + step))] {
                    for p in [a, b, c] {
                        vertex(&mut s, p.0, p.1, 0.5, [16.0 / 255.0, 32.0 / 255.0, 64.0 / 255.0, 1.0 / 255.0], 16.0, 16.0);
                    }
                    n += 3;
                }
                x += step;
            }
            writeln!(s, "draw 4 {n} 32").unwrap();
            y += step;
        }
        read_rt(&mut s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // culling: a clockwise and a counterclockwise triangle under each cull mode and winding
    for cull in [1, 2, 3] {
        for ccw in [0, 1] {
            begin(&mut s, dev, &format!("cull-{cull}-{ccw}"), 8, 8, 28, "psprim");
            writeln!(s, "rast 3 {cull} {ccw} 0 0 0 1 0 0 0").unwrap();
            for (a, b, c) in [((0.0, 0.0), (8.0, 0.0), (0.0, 8.0)), ((8.0, 8.0), (8.0, 0.5), (0.5, 8.0))] {
                for p in [a, b, c] {
                    vertex(&mut s, p.0, p.1, 0.5, [1.0; 4], 8.0, 8.0);
                }
            }
            writeln!(s, "draw 4 6 32").unwrap();
            read_rt(&mut s, 0, 28, 8, 8);
            writeln!(s, "end").unwrap();
        }
    }
    // scissor rectangles (on and off)
    for (i, rc) in [(2, 3, 6, 7), (0, 0, 8, 8), (-5, -5, 3, 20), (5, 5, 5, 6), (6, 6, 2, 2), (7, 0, 100, 1), (0, 0, 0, 0)].iter().enumerate() {
        for on in [1, 0] {
            begin(&mut s, dev, &format!("scissor-{i}-{on}"), 8, 8, 28, "ps");
            writeln!(s, "rast 3 1 0 0 0 0 1 {on} 0 0\nscissor {} {} {} {}", rc.0, rc.1, rc.2, rc.3).unwrap();
            for p in [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0), (8.0, 8.0)] {
                vertex(&mut s, p.0, p.1, 0.5, color(i as u32), 8.0, 8.0);
            }
            writeln!(s, "draw 5 4 32").unwrap();
            read_rt(&mut s, 0, 28, 8, 8);
            writeln!(s, "end").unwrap();
        }
    }
    // viewports: offsets (fractional ones on 11), larger and smaller than the target, depth
    // ranges; a triangle in normalized coordinates
    let vps: Vec<[f32; 6]> = vec![
        [0.0, 0.0, 8.0, 8.0, 0.0, 1.0],
        [2.0, 1.0, 4.0, 4.0, 0.0, 1.0],
        [-4.0, -4.0, 16.0, 16.0, 0.0, 1.0],
        [0.5, 0.0, 8.0, 8.0, 0.0, 1.0],
        [0.25, 0.75, 6.5, 5.5, 0.0, 1.0],
        [1.0 / 256.0, 0.0, 8.0, 8.0, 0.0, 1.0],
        [0.0, 0.0, 8.0, 8.0, 0.25, 0.75],
        [0.0, 0.0, 0.0, 8.0, 0.0, 1.0],
        [0.0, 0.0, 8.0, 8.0, 1.0, 0.0],
    ];
    for (i, vp) in vps.iter().enumerate() {
        if !d3d11 && vp[..4].iter().any(|v| v.fract() != 0.0) {
            continue;
        }
        begin(&mut s, dev, &format!("viewport-{i}"), 8, 8, 2, "pspos");
        writeln!(s, "viewport {}", vp.iter().map(|v| fb(*v)).collect::<Vec<_>>().join(" ")).unwrap();
        for (x, y) in [(-0.75f32, 0.75f32), (0.75, 0.5), (-0.5, -0.75)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
        read_rt(&mut s, 0, 2, 8, 8);
        writeln!(s, "end").unwrap();
    }
    // SV_Position: a quad over the target at constant depths and w
    for (i, (z, w)) in [(0.5f32, 1.0f32), (0.0, 1.0), (1.0, 1.0), (0.25, 2.0), (0.75, 0.5)].iter().enumerate() {
        begin(&mut s, dev, &format!("pos-{i}"), 4, 4, 2, "pspos");
        for (x, y) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 {}", fb(x * w), fb(y * w), fb(z * w), fb(*w), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32").unwrap();
        read_rt(&mut s, 0, 2, 4, 4);
        writeln!(s, "end").unwrap();
    }
    // vertex, instance and primitive ids, the front face, for each topology of triangles
    for (top, n, inst) in [(4u32, 6u32, 0u32), (5, 6, 0), (4, 3, 3), (5, 4, 2)] {
        writeln!(s, "case ids-{top}-{n}-{inst}\n{dev}").unwrap();
        writeln!(s, "tex 0 2 8 1 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0").unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(8.0), fb(1.0), fb(1.0)).unwrap();
        writeln!(s, "shader 0 vs {}\nlayout 0 2\nshader 1 ps {}", hex(&shader("vsid")), hex(&shader("psflat"))).unwrap();
        // (list: triangle t over pixel column t; strip: vertex k at x = k, alternately above
        // and below, so triangle i spans columns i and i + 1)
        let pos = |s: &mut String| {
            for v in 0..n {
                let (x, y) = if top == 4 { ((v / 3) as f32 + [0.0, 1.0, 0.5][v as usize % 3], [-10.0f32, -10.0, 10.0][v as usize % 3]) } else { (v as f32, if v % 2 == 0 { -10.0 } else { 10.0 }) };
                writeln!(s, "vtx {} {} {} {}", fb(x / 4.0 - 1.0), fb(-y), fb(0.5), fb(1.0)).unwrap();
            }
        };
        pos(&mut s);
        writeln!(s, "draw {top} {n} 16{}", if inst > 0 { format!(" {inst}") } else { String::new() }).unwrap();
        read_rt(&mut s, 0, 2, 8, 1);
        writeln!(s, "shader 1 ps {}\nclear 0 0 0 0 0", hex(&shader("psprim"))).unwrap();
        pos(&mut s);
        writeln!(s, "draw {top} {n} 16{}", if inst > 0 { format!(" {inst}") } else { String::new() }).unwrap();
        read_rt(&mut s, 0, 2, 8, 1);
        writeln!(s, "end").unwrap();
    }
    // depth: triangles at constant depths through each comparison, with and without writes,
    // into each depth format; the color and the depth read back
    let zs = [0.5f32, 0.25, 0.75, 0.5, 1.0, 0.0, 0.3, 0.7];
    for (df, tl) in [(40u32, 40u32), (55, 55), (45, 45), (20, 20)] {
        for func in 1..=8 {
            for write in [1, 0] {
                begin(&mut s, dev, &format!("depth-{df}-{func}-{write}"), 8, 8, 28, "ps");
                writeln!(s, "tex 1 2 8 8 1 1 {tl} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 {write} {func}", fb(0.5)).unwrap();
                for (i, z) in zs.iter().enumerate() {
                    let x0 = (i % 4) as f32 * 2.0;
                    for p in [(x0 - 1.0, -1.0), (x0 + 6.0, 0.0), (x0, 9.0)] {
                        vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
                    }
                }
                writeln!(s, "draw 4 {} 32", zs.len() * 3).unwrap();
                read_rt(&mut s, 0, 28, 8, 8);
                writeln!(s, "{}", super::d3dres::read_line(1, 0, format(df).unwrap(), 8, 8, 1, 0)).unwrap();
                writeln!(s, "end").unwrap();
            }
        }
        // depth values written: a ramp of constant-depth triangles
        begin(&mut s, dev, &format!("depthval-{df}"), 8, 8, 28, "ps");
        writeln!(s, "tex 1 2 8 8 1 1 {tl} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 8", fb(1.0)).unwrap();
        let vals = [0.0f32, 1.0, 0.5, 1.0 / 3.0, 0.1, 0.9, 1.0 / 65535.0, 0.5 / 65535.0, 1.5 / 65535.0, 1.0 / 16777215.0, 0.5 / 16777215.0, 2.5 / 16777215.0, 0.999_999_9, 1e-20, 0.25, 0.75];
        for (i, z) in vals.iter().enumerate() {
            let (x, y) = ((i % 4) as f32 * 2.0, (i / 4) as f32 * 2.0);
            for p in [(x, y), (x + 2.0, y), (x, y + 2.0), (x + 2.0, y), (x + 2.0, y + 2.0), (x, y + 2.0)] {
                vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
            }
        }
        writeln!(s, "draw 4 {} 32", vals.len() * 6).unwrap();
        writeln!(s, "{}", super::d3dres::read_line(1, 0, format(df).unwrap(), 8, 8, 1, 0)).unwrap();
        writeln!(s, "end").unwrap();
    }
    // depth clipping: triangles outside 0..1, clipped or clamped
    for clip in [1, 0] {
        begin(&mut s, dev, &format!("depthclip-{clip}"), 8, 8, 28, "ps");
        writeln!(s, "rast 3 1 0 0 0 0 {clip} 0 0 0\ntex 1 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 8", fb(0.5)).unwrap();
        for (i, z) in [-0.5f32, 1.5, -0.0, 1.0, 2.0, -1e-7].iter().enumerate() {
            let x = i as f32;
            for p in [(x, 0.0), (x + 1.0, 0.0), (x, 8.0), (x + 1.0, 0.0), (x + 1.0, 8.0), (x, 8.0)] {
                vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
            }
        }
        writeln!(s, "draw 4 36 32").unwrap();
        read_rt(&mut s, 0, 28, 8, 8);
        writeln!(s, "read 1 0 32 8 1\nend").unwrap();
    }
    // blending: every factor for source and destination, each operation, on unorm, float and
    // srgb targets; the target cleared to one color, a quad of another drawn over it
    let factors: Vec<u32> = (1..=11).chain([14, 15]).collect();
    let src_c = [0.25f32, 0.5, 0.75, 0.6];
    let dst_c = [0.8f32, 0.4, 0.2, 0.3];
    let bf = [0.2f32, 0.4, 0.6, 0.8];
    for fmt in [28u32, 10, 2, 29, 24] {
        for (ci, chunk) in factors.iter().flat_map(|a| factors.iter().map(move |b| (*a, *b))).collect::<Vec<_>>().chunks(13).enumerate() {
            writeln!(s, "case blend-{fmt}-{ci}\n{dev}").unwrap();
            writeln!(s, "tex 0 2 1 1 1 1 {fmt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("ps"))).unwrap();
            writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(1.0), fb(1.0), fb(1.0)).unwrap();
            for (k, (a, b)) in chunk.iter().enumerate() {
                let op = [1u32, 2, 3, 4, 5][k % 5];
                writeln!(s, "clear 0 {} {} {} {}", fb(dst_c[0]), fb(dst_c[1]), fb(dst_c[2]), fb(dst_c[3])).unwrap();
                // (alpha factors: the _COLOR ones are not allowed)
                let al = |f: u32| match f {
                    3 => 5,
                    4 => 6,
                    9 => 7,
                    10 => 8,
                    f => f,
                };
                writeln!(s, "blend1 1 {a} {b} {op} {} {} {} 0xf {} {} {} {}", al(*a), al(*b), [1u32, 2, 3, 4, 5][(k + 2) % 5], fb(bf[0]), fb(bf[1]), fb(bf[2]), fb(bf[3])).unwrap();
                for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
                    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0), fb(p.1), fb(0.5), fb(1.0), fb(src_c[0]), fb(src_c[1]), fb(src_c[2]), fb(src_c[3])).unwrap();
                }
                writeln!(s, "draw 4 3 32").unwrap();
                read_rt(&mut s, 0, fmt, 1, 1);
            }
            writeln!(s, "end").unwrap();
        }
    }
    // write masks
    writeln!(s, "case writemask\n{dev}\ntex 0 2 16 1 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("ps"))).unwrap();
    writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(1.0), fb(1.0)).unwrap();
    for m in 0..16 {
        writeln!(s, "viewport {} 0 {} {} 0 {}\nblend1 0 2 1 1 2 1 1 {m}", fb(m as f32), fb(1.0), fb(1.0), fb(1.0)).unwrap();
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0), fb(p.1), fb(0.5), fb(1.0), fb(1.0), fb(0.5), fb(0.25), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
    }
    read_rt(&mut s, 0, 28, 16, 1);
    writeln!(s, "end").unwrap();
    // every format with shader resource support read by ld (a 4x4 texture of random bytes, to
    // a float or uint target): how each format decodes
    for f in FORMATS.iter().filter(|f| !f.video() && f.num != Num::Typeless && !matches!(f.id, 19 | 20 | 22 | 23 | 40 | 44 | 45 | 47 | 55 | 66) && (d3d11 || !(94..=99).contains(&f.id))) {
        let int = matches!(f.num, Num::Uint | Num::Sint);
        let (rt, ps) = if int { (3u32, "psldu") } else { (2, "psld") };
        let (w, ht) = (4 * f.bw.min(2), 4 * f.bh);
        writeln!(s, "case ld-{}\n{dev}", f.id).unwrap();
        writeln!(s, "tex 0 2 {w} {ht} 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        writeln!(s, "tex 1 2 {w} {ht} 1 1 {} 0 8 0 0 1 0 init {} {} 0\nsrv 1 1 nodesc\ntsrv 1", f.id, f.id * 3 + 1, f.row_bytes(w)).unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader(ps))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(w as f32), fb(ht as f32), fb(1.0)).unwrap();
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
        writeln!(s, "read 0 0 {} {ht} 1", 16 * w).unwrap();
        writeln!(s, "end").unwrap();
    }
    // tables: 32x32 textures (every byte value many times over) of the formats whose decoding
    // is fitted to Windows' answers (sRGB, signed and HDR / mode-based blocks)
    for id in [29u32, 91, 81, 84, 95, 96, 98, 99] {
        if !d3d11 && (94..=99).contains(&id) {
            continue;
        }
        let f = format(id).unwrap();
        writeln!(s, "case ld-table-{id}\n{dev}").unwrap();
        writeln!(s, "tex 0 2 32 32 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        writeln!(s, "tex 1 2 32 32 1 1 {id} 0 8 0 0 1 0 init {} {} 0\nsrv 1 1 nodesc\ntsrv 1", id * 5 + 3, f.row_bytes(32)).unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("psld"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(32.0), fb(32.0), fb(1.0)).unwrap();
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 4 3 32\nread 0 0 512 32 1\nend").unwrap();
    }
    // depth formats read through their shader resource formats
    for (tl, view) in [(40u32, 41u32), (55, 56), (45, 46), (20, 21), (45, 47)] {
        let rt = if view == 47 { 3 } else { 2 };
        let ps = if view == 47 { "psldu" } else { "psld" };
        writeln!(s, "case ld-depth-{tl}-{view}\n{dev}").unwrap();
        writeln!(s, "tex 0 2 4 4 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let tf = match tl {
            40 => 39,
            55 => 53,
            45 => 44,
            _ => 19,
        };
        writeln!(s, "tex 1 2 4 4 1 1 {tf} 0 0x48 0 0 1 0 init {} {} 0\nsrv 1 1 {view} 4 0 1", tl + 7, format(tf).unwrap().row_bytes(4)).unwrap();
        writeln!(s, "tsrv 1\nshader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader(ps))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(4.0), fb(4.0), fb(1.0)).unwrap();
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 4 3 32\nread 0 0 64 4 1\nend").unwrap();
    }
    // sampling a 4x4 texture with point filtering at texel centers from -1.5 to 2.5 under each
    // address mode (16 pixels: every texel four times over)
    for addr in 1..=5 {
        for filter in [0u32, 0x15] {
            writeln!(s, "case sample-{addr}-{filter:x}\n{dev}").unwrap();
            writeln!(s, "tex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
            writeln!(s, "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init {} 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter} {addr} {addr} {addr}", 77 + addr).unwrap();
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
            writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
            for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 2.5, -1.5), (-1.0, -1.0, -1.5, 2.5), (1.0, -1.0, 2.5, 2.5)] {
                writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
            }
            writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
        }
    }
    probes(&mut s, dev);
    // shaders the runtime refuses: a broken checksum, a truncated container, nothing
    let mut broken = shader("ps");
    broken[5] ^= 1;
    let short = shader("ps")[..40].to_vec();
    writeln!(s, "case shader-bad\n{dev}\nshader 0 ps {}\nshader 1 ps {}\nshader 2 vs {}\nshader 3 ps 00\nend", hex(&broken), hex(&short), hex(&shader("ps"))).unwrap();
    // an input layout that the program's input signature does not match
    writeln!(s, "case layout-bad\n{dev}\nshader 0 vs {}\nlayout 0 2\nlayout 0 2 2 2\nlayout 0 0x1c 2\nlayout 0 0x29 2\nlayout 0 0 2\nlayout 1 2 2\nend", hex(&shader("vs"))).unwrap();
    // sample_c through a sampler without comparison, sample through one with it, and the
    // minimum and maximum filters (what D3DRASTER.FILTER leaves open; undefined by the
    // documentation, or a tier WARP may lack: near the end, as WARP may end the executor on them)
    for (name, ps, filter) in [("samplec-nocmp", "pssamplec", 0x15u32), ("sample-cmp", "pssample", 0x95), ("sample-min", "pssample", 0x115), ("sample-max", "pssample", 0x195), ("sample-min-point", "pssample", 0x100), ("sample-min-unorm8", "pssample", 0x115), ("sample-max-mips", "pssample", 0x195)] {
        writeln!(s, "case probe-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let tex = match name {
            "sample-min-unorm8" => "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init 400 16 0",
            // (two levels, 24 x 24 and 12 x 12, minified about 1.65 times: between them)
            "sample-max-mips" => "tex 1 2 24 24 1 2 56 0 8 0 0 1 0 init 400 48 0",
            _ => "tex 1 2 4 4 1 1 56 0 8 0 0 1 0 init 400 8 0",
        };
        writeln!(s, "{tex}\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3 2").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader(ps))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v, r) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32, 0.0f32), (1.0, 1.0, 0.9, -0.2, 1.0), (-1.0, -1.0, -0.2, 0.9, 0.0), (1.0, -1.0, 0.9, 0.9, 1.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(r), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // a patch list drawn without hull and domain programs: WARP ends the executor there, so
    // it comes last
    begin(&mut s, dev, "probe-topology-33", 16, 16, 28, "ps");
    for k in 0..3 {
        vertex(&mut s, 2.0 + 5.0 * k as f32, 3.0 + 4.0 * k as f32, 0.5, color(k), 16.0, 16.0);
    }
    writeln!(s, "draw 33 3 32").unwrap();
    read_rt(&mut s, 0, 28, 16, 16);
    writeln!(s, "end").unwrap();
    s
}

/// one case per draw the other cases do not make: what Windows does there is to be measured
fn probes(s: &mut String, dev: &str) {
    // (one color, unless `vary`)
    let tri3 = |s: &mut String, z: [f32; 3], w: [f32; 3], vary: bool| {
        for (i, p) in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)].iter().enumerate() {
            let c = color(if vary { i as u32 } else { 0 });
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0 * w[i]), fb(p.1 * w[i]), fb(z[i] * w[i]), fb(w[i]), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
        }
    };
    let tri = |s: &mut String, z: [f32; 3], w: [f32; 3]| tri3(s, z, w, false);
    let one = [1.0f32; 3];
    let half = [0.5f32; 3];
    // (the name, lines after begin, the triangle's z and w, the draw)
    let cases: [(&str, &str, [f32; 3], [f32; 3], &str); 9] = [
        ("vp-frac", &format!("viewport {} {} {} {} 0 {}", fb(-0.5), fb(-0.5), fb(16.0), fb(16.0), fb(1.0)), half, one, "draw 4 3 32"),
        ("topology-points", "", half, one, "draw 1 3 32"),
        ("topology-lines", "", half, one, "draw 2 3 32"),
        ("vb-past", "", half, one, "draw 4 6 32"),
        ("clip-w", "", half, [1.0, -1.0, 1.0], "draw 4 3 32"),
        ("clip-z", "", [-0.5, 0.5, 1.5], one, "draw 4 3 32"),
        ("wireframe", "rast 2 1 0 0 0 0 1 0 0 0", half, one, "draw 4 3 32"),
        ("blend-src1", "blend1 1 16 2 1 2 2 1 0xf", half, one, "draw 4 3 32"),
        ("blend-badop", "blend1 1 2 2 6 2 2 1 0xf", half, one, "draw 4 3 32"),
    ];
    for (name, extra, z, w, draw) in cases {
        begin(s, dev, &format!("probe-{name}"), 16, 16, 28, "ps");
        if !extra.is_empty() {
            writeln!(s, "{extra}").unwrap();
        }
        tri(s, z, w);
        writeln!(s, "{draw}").unwrap();
        read_rt(s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // colors that vary over the triangle (the interpolation)
    begin(s, dev, "probe-interp", 16, 16, 2, "ps");
    tri3(s, [0.1, 0.5, 0.9], [1.0, 2.0, 0.5], true);
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 2, 16, 16);
    writeln!(s, "end").unwrap();
    // no shaders and layout; no viewport; a multisampled target
    writeln!(s, "case probe-incomplete\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0\nviewport 0 0 {} {} 0 {}", fb(16.0), fb(16.0), fb(1.0)).unwrap();
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nread 0 0 64 16 1\nend").unwrap();
    writeln!(s, "case probe-noviewport\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0", hex(&shader("vs")), hex(&shader("ps"))).unwrap();
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nread 0 0 64 16 1\nend").unwrap();
    writeln!(s, "case probe-rt-ms\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    // to fit what D3DRASTER.RTKIND leaves open: where the samples are, by the share of a
    // pixel's samples that triangles' slanted edges cover (the resolve's average)
    for n in [2u32, 4, 8] {
        writeln!(s, "case probe-rt-ms{n}-edges\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 {n} 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for t in [[(0.0f32, 0.0f32), (16.0, 1.0), (0.0, 3.0)], [(0.0, 4.0), (16.0, 4.0), (8.0, 8.0)], [(3.0, 9.0), (4.0, 16.0), (0.5, 16.0)], [(9.0, 9.0), (15.5, 10.25), (11.0, 15.75)]] {
            for (x, y) in t {
                vertex(s, x, y, 0.5, [1.0; 4], 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 12 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    }
    // the resolve's mean of samples of other values, overlapping colored triangles, in a
    // UNORM and a float target
    for (fmt, pitch) in [(28u32, 64u32), (2, 256), (10, 128), (24, 64), (29, 64), (87, 64), (85, 32), (26, 64), (31, 64), (16, 128), (35, 64), (41, 64), (13, 128), (86, 32), (115, 32)] {
        writeln!(s, "case probe-rt-ms4-colors-{fmt}\n{dev}\ntex 0 2 16 16 1 1 {fmt} 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 {fmt} 0 0x20 0 0 1 0").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (k, t) in [[(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)], [(3.0, 1.0), (15.5, 15.0), (0.0, 9.75)], [(8.25, 0.0), (12.0, 16.0), (2.0, 12.5)]].iter().enumerate() {
            for (i, &(x, y)) in t.iter().enumerate() {
                vertex(s, x, y, 0.5, color(k as u32 * 3 + i as u32), 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 9 32\nresolve 1 0 0 0 {fmt}\nread 1 0 {pitch} 16 1\nend").unwrap();
    }
    // what D3DRASTER.RTKIND and CLIPW leave open: lines on a multisampled target, a resolve
    // of differing samples of 16-bit UNORM, a vertex behind the eye under colors that differ
    writeln!(s, "case probe-rt-ms4-lines\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 2 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    writeln!(s, "case probe-rt-ms4-points\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    writeln!(s, "case probe-rt-ms4-unorm16\n{dev}\ntex 0 2 16 16 1 1 11 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 11 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32 + 5), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 11\nread 1 0 128 16 1\nend").unwrap();
    // what D3DRASTER.RTKIND leaves open: depth per sample, a viewport off whole pixels, lines
    // with the multisample state, points through pspos and over differing samples on a
    // multisampled target; a depth view alone
    let ms4 = |s: &mut String, name: &str, rast: &str, vp: (f32, f32), ps: &str| {
        writeln!(s, "case probe-rt-ms4-{name}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast {rast}\nviewport {} {} {} {} 0 {}", hex(&shader("vs")), hex(&shader(ps)), fb(vp.0), fb(vp.1), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    };
    ms4(s, "depth", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), "ps");
    writeln!(s, "tex 2 2 16 16 1 1 40 0 0x40 0 0 4 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(1.0)).unwrap();
    for (k, (t, z)) in [([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)], 0.25f32), ([(3.0, 1.0), (15.5, 15.0), (0.0, 9.75)], 0.75), ([(8.25, 0.0), (12.0, 16.0), (2.0, 12.5)], 0.5)].into_iter().enumerate() {
        for (i, (x, y)) in t.into_iter().enumerate() {
            vertex(s, x, y, z, color(k as u32 * 3 + i as u32), 16.0, 16.0);
        }
    }
    writeln!(s, "draw 4 9 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    ms4(s, "vphalf", "3 1 0 0 0 0 1 0 0 0", (0.5, 0.25), "ps");
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    for (name, rast) in [("linesms", "3 1 0 0 0 0 1 0 1 0"), ("linesaa", "3 1 0 0 0 0 1 0 0 1"), ("linesboth", "3 1 0 0 0 0 1 0 1 1")] {
        ms4(s, name, rast, (0.0, 0.0), "ps");
        for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter().enumerate() {
            vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
        }
        writeln!(s, "draw 2 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    }
    ms4(s, "points-pspos", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), "pspos");
    for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    ms4(s, "points-over", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), "ps");
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32 + 5), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32").unwrap();
    for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 1.25), (5.5, 0.75)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    // lines and wireframe over samples that differ
    for (name, rast, top) in [("lines-over", "3 1 0 0 0 0 1 0 0 0", 2u32), ("wire-over", "2 1 0 0 0 0 1 0 0 0", 4)] {
        ms4(s, name, "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), "ps");
        for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
            vertex(s, x, y, 0.5, color(i as u32 + 5), 16.0, 16.0);
        }
        writeln!(s, "draw 4 3 32\nrast {rast}").unwrap();
        for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75), (0.5, 9.0), (15.0, 3.0)].into_iter().enumerate() {
            vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
        }
        writeln!(s, "draw {top} 6 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    }
    // what D3DRASTER.RTKIND leaves open: a target in an array's second slice, a multisampled
    // target of quality 1
    writeln!(s, "case probe-rt-array\n{dev}\ntex 0 2 16 16 2 1 28 0 0x20 0 0 1 0\nrtv 0 0 28 5 0 1 1\nclear 0 0 0 0 0\ntarget 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nread 0 1 64 16 1\nread 0 0 64 16 1\nend").unwrap();
    // what D3DRASTER.RTKIND leaves open: a 3D texture's view as the target (2 slices)
    // (and from its second slice)
    // (and a 1D texture's view, 16 x 1)
    for (name, rtv) in [("rt-3d", "rtv 0 0 nodesc"), ("rt-3d-slice1", "rtv 0 0 28 8 0 1 1"), ("rt-1d", "rtv 0 0 nodesc")] {
        let tex = if name == "rt-1d" { "tex 0 1 16 1 1 1 28 0 0x20 0 0 1 0" } else { "tex 0 3 16 16 2 1 28 0 0x20 0 0 1 0" };
        writeln!(s, "case probe-{name}\n{dev}\n{tex}\n{rtv}\nclear 0 0 0 0 0\ntarget 0").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
            vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
        }
        let read = if name == "rt-1d" { "read 0 0 64 1 1" } else { "read 0 0 64 16 2" };
        writeln!(s, "draw 4 3 32\n{read}\nend").unwrap();
    }
    writeln!(s, "case probe-rt-ms4-q1\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 1\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    // copies of differing samples: the whole resource, then resolved
    writeln!(s, "case probe-rt-ms4-copy\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0\ntex 2 2 16 16 1 1 28 0 0x20 0 0 4 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\ncopy 2 0\nresolve 1 0 2 0 28\nread 1 0 64 16 1\nend").unwrap();
    // what D3DRASTER.RTKIND leaves open: a whole-subresource region copy of differing samples,
    // a depth view of one sample under a 4-sample target, a region copy with a box, an update
    for (name, setup, after) in [("copysub", "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0", "copysub 2 0 0 0 0 0 0\nresolve 1 0 2 0 28"), ("depth1", "tex 2 2 16 16 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 0x3f000000 0\ntarget 0 2\ndepthst 1 1 2", "resolve 1 0 0 0 28"), ("depth1-prev", "rtv 1 1 nodesc\ntarget 1\ntex 2 2 16 16 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 0x3f000000 0\ntarget 0 2\ndepthst 1 1 2", "read 1 0 64 16 1\nresolve 1 0 0 0 28"), ("copysub-box", "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0", "copysub 2 0 0 0 0 0 0 0 0 0 8 8 1\nresolve 1 0 2 0 28"), ("update", "", "update 0 0 7 64 0\nresolve 1 0 0 0 28")] {
        writeln!(s, "case probe-rt-ms4-{name}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
        if !setup.is_empty() {
            writeln!(s, "{setup}").unwrap();
        }
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (i, (x, y)) in [(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter().enumerate() {
            vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
        }
        writeln!(s, "draw 4 3 32\n{after}\nread 1 0 64 16 1\nend").unwrap();
    }
    writeln!(s, "case probe-depth-alone\n{dev}\ntex 1 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget - 1\ndepthst 1 1 8", fb(1.0)).unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(8.0), fb(8.0), fb(1.0)).unwrap();
    for (x, y) in [(0.0f32, 0.0f32), (8.0, 1.5), (0.5, 7.0)] {
        vertex(s, x, y, 0.375, [1.0; 4], 8.0, 8.0);
    }
    writeln!(s, "draw 4 3 32\nread 1 0 32 8 1\nend").unwrap();
    // what D3DRASTER.RTKIND leaves open: wireframe on a multisampled target
    writeln!(s, "case probe-rt-ms4-wire\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 2 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
    for (i, (x, y)) in [(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25)].into_iter().enumerate() {
        vertex(s, x, y, 0.5, color(i as u32), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend").unwrap();
    // an input layout against a refused program
    let mut broken = shader("vs");
    broken[5] ^= 1;
    writeln!(s, "case probe-layout-refused\n{dev}\nshader 0 vs {}\nlayout 0 2 2\nend", hex(&broken)).unwrap();
    // an input layout against a pixel program given as a vertex one
    writeln!(s, "case probe-layout-otherkind\n{dev}\nshader 0 vs {}\nlayout 0 2 2\nlayout 0 2\nend", hex(&shader("ps"))).unwrap();
    // ld past the texture, no sampler, samples between texel centers, blending an integer target
    let ldtex = "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init 91 16 0\nsrv 1 1 nodesc\ntsrv 1";
    let quad = |s: &mut String, uv: (f32, f32)| {
        for (x, y, u, v) in [(-1.0f32, 1.0f32, -uv.0, -uv.0), (1.0, 1.0, uv.1, -uv.0), (-1.0, -1.0, -uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32").unwrap();
    };
    for (name, ps, rt, extra, uv) in [("ld-out", "psld", 2u32, "", (0.0f32, 1.0f32)), ("sample-nosampler", "pssample", 2, "", (0.0, 1.0)), ("sample-between", "pssample", 2, "samp 0x15 3 3 3", (0.1, 1.3)), ("blend-uint", "psldu", 3, "blend1 1 2 2 1 2 2 1 0xf", (0.0, 1.0))] {
        let tex = if rt == 3 { ldtex.replace(" 28 0 8", " 30 0 8") } else { ldtex.to_string() };
        writeln!(s, "case probe-{name}\n{dev}\ntex 0 2 8 8 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0\n{tex}").unwrap();
        if !extra.is_empty() {
            writeln!(s, "{extra}").unwrap();
        }
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nviewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", hex(&shader("vs")), hex(&shader(ps)), fb(8.0), fb(8.0), fb(1.0)).unwrap();
        quad(s, uv);
        writeln!(s, "read 0 0 128 8 1\nend").unwrap();
    }
    // many of each, to fit: perspective interpolation into a float target, points, lines,
    // wireframe, filtering between texel centers
    let mut r = XorShift32::new(0x9e0b_e5);
    let px = |r: &mut XorShift32, lo: i64, hi: i64| r.range(lo * 256, hi * 256) as f32 / 256.0;
    let unit = |r: &mut XorShift32| r.range(0, 1 << 16) as f32 / 65536.0;
    for i in 0..24 {
        begin(s, dev, &format!("probe-interp-{i}"), 16, 16, 2, "ps");
        for _ in 0..3 {
            let (x, y, z, w) = (px(&mut r, -4, 20), px(&mut r, -4, 20), unit(&mut r), 0.25 + 3.75 * unit(&mut r));
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = [unit(&mut r), unit(&mut r), unit(&mut r), unit(&mut r)];
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
        read_rt(s, 0, 2, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // depths that vary over unclipped triangles: read through SV_Position (w = 1 and with w),
    // and tested against a 32-bit float and a 16-bit UNORM depth view
    let mut rz = XorShift32::new(0x2_9a7e);
    for i in 0..6u32 {
        let (ps, fmt) = if i < 4 { ("pspos", 2u32) } else { ("ps", 28) };
        begin(s, dev, &format!("probe-zplane-{i}"), 16, 16, fmt, ps);
        if i >= 4 {
            let df = if i == 4 { 40 } else { 55 };
            writeln!(s, "tex 1 2 16 16 1 1 {df} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 2", fb(0.5)).unwrap();
        }
        for _ in 0..3 {
            let (x, y, z) = (px(&mut rz, 1, 15), px(&mut rz, 1, 15), unit(&mut rz));
            let w = if i == 1 || i == 3 { 0.5 + 1.5 * unit(&mut rz) } else { 1.0 };
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = color(i * 3);
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
        read_rt(s, 0, fmt, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // to separate the interpolation's parts: w = 1 inside the target (the plane equations'
    // arithmetic), inside with w (the perspective division), one vertex past one edge (the
    // clipping and its triangles), with w = 1 and with w
    let mut r2 = XorShift32::new(0x1e7e_4b);
    let tri_case = |s: &mut String, r: &mut XorShift32, name: String, past: Option<u32>, persp: bool| {
        begin(s, dev, &name, 24, 24, 2, "ps");
        for k in 0..3u32 {
            let (mut x, mut y) = (px(r, 1, 23), px(r, 1, 23));
            if k == 0 {
                match past {
                    Some(0) => x = -px(r, 1, 12),
                    Some(1) => x = 24.0 + px(r, 1, 12),
                    Some(2) => y = 24.0 + px(r, 1, 12),
                    Some(_) => y = -px(r, 1, 12),
                    None => {}
                }
            }
            let (z, w) = (unit(r), if persp { 0.25 + 3.75 * unit(r) } else { 1.0 });
            let (nx, ny) = (x / 24.0 * 2.0 - 1.0, 1.0 - y / 24.0 * 2.0);
            let c = [unit(r), unit(r), unit(r), unit(r)];
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
        }
        writeln!(s, "draw 4 3 32").unwrap();
        read_rt(s, 0, 2, 24, 24);
        writeln!(s, "end").unwrap();
    };
    for i in 0..12 {
        tri_case(s, &mut r2, format!("probe-interp-affine-{i}"), None, false);
        tri_case(s, &mut r2, format!("probe-interp-persp-{i}"), None, true);
    }
    for plane in 0..4u32 {
        for i in 0..3 {
            tri_case(s, &mut r2, format!("probe-interp-clip{plane}-{i}"), Some(plane), false);
            tri_case(s, &mut r2, format!("probe-interp-clip{plane}w-{i}"), Some(plane), true);
        }
    }
    for (name, top, n, fill) in [("points", 1u32, 24u32, 3u32), ("lines", 2, 16, 3), ("linestrip", 3, 9, 3), ("wire", 4, 6, 2)] {
        for i in 0..8 {
            begin(s, dev, &format!("probe-{name}-{i}"), 16, 16, 28, "ps");
            if fill != 3 {
                writeln!(s, "rast {fill} 1 0 0 0 0 1 0 0 0").unwrap();
            }
            for k in 0..n {
                vertex(s, px(&mut r, -2, 18), px(&mut r, -2, 18), 0.5, color(i * 31 + k), 16.0, 16.0);
            }
            writeln!(s, "draw {top} {n} 32").unwrap();
            read_rt(s, 0, 28, 16, 16);
            writeln!(s, "end").unwrap();
        }
    }
    for i in 0..12u32 {
        let (filter, addr) = (*r.pick(&[0x15u32, 0x14, 0x11, 0x10, 0x5, 0x1, 0x4]), r.range(1, 5));
        let (u0, u1, v0, v1) = (px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0);
        writeln!(s, "case probe-filter-{i}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        writeln!(s, "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init {} 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} {addr} {addr} {addr}", 300 + i).unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v) in [(-1.0f32, 1.0f32, u0, v0), (1.0, 1.0, u1, v0), (-1.0, -1.0, u0, v1), (1.0, -1.0, u1, v1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // to fit what D3DRASTER.TOPOLOGY leaves open: line ends inside a pixel's diamond (16
    // short lines a case, each alone in a 4 x 4 cell, ending near the cell's pixel (2, 2) from
    // every direction), lines with w, the multisample and antialiased-line states, the
    // adjacency and patch topologies, points and lines through the other programs
    let mut r3 = XorShift32::new(0x11e_e7d);
    for i in 0..8u32 {
        begin(s, dev, &format!("probe-lineend-{i}"), 16, 16, 28, "ps");
        for cell in 0..16u32 {
            let (cx, cy) = ((cell % 4 * 4) as f32 + 2.5, (cell / 4 * 4) as f32 + 2.5);
            let (ox, oy) = loop {
                let (a, b) = (r3.range(-127, 128), r3.range(-127, 128));
                if a.abs() + b.abs() < 128 {
                    break (a as f32 / 256.0, b as f32 / 256.0);
                }
            };
            let (dx, dy) = loop {
                let (a, b) = (r3.range(-256, 257) as f32, r3.range(-256, 257) as f32);
                if a.abs() + b.abs() >= 64.0 {
                    break (a, b);
                }
            };
            let len = (1.2 + r3.range(0, 154) as f32 / 256.0) / (dx * dx + dy * dy).sqrt();
            let (bx, by) = (cx + ox, cy + oy);
            let grid = |v: f32| (v * 256.0).round() / 256.0;
            vertex(s, grid(bx - dx * len), grid(by - dy * len), 0.5, color(i * 37 + 2 * cell), 16.0, 16.0);
            vertex(s, bx, by, 0.5, color(i * 37 + 2 * cell + 1), 16.0, 16.0);
        }
        writeln!(s, "draw 2 32 32").unwrap();
        read_rt(s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // lines that start or end on a corner of a pixel's diamond (left, right, top, bottom),
    // x-major and y-major in every direction, one line a 4 x 4 cell (probe-rt-ms4-lines-over
    // lit a pixel a line only touches)
    let combos: Vec<(usize, (f32, f32), bool)> = (0..4).flat_map(|v| [(1.0f32, 0.375f32), (1.0, -0.375), (-1.0, 0.375), (-1.0, -0.375), (0.375, 1.0), (-0.375, 1.0), (0.375, -1.0), (-0.375, -1.0)].into_iter().flat_map(move |d| [(v, d, true), (v, d, false)])).collect();
    for (ms, (i, part)) in [false, true].into_iter().flat_map(|ms| combos.chunks(16).enumerate().map(move |p| (ms, p))) {
        if ms {
            // (and on a 4-sample target, resolved)
            writeln!(s, "case probe-rt-ms4-linevert-{i}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0").unwrap();
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader("vs")), hex(&shader("ps")), fb(16.0), fb(16.0), fb(1.0)).unwrap();
        } else {
            begin(s, dev, &format!("probe-linevert-{i}"), 16, 16, 28, "ps");
        }
        for (cell, &(v, (dx, dy), starts)) in part.iter().enumerate() {
            let (cx, cy) = ((cell % 4 * 4) as f32 + 2.5, (cell / 4 * 4) as f32 + 2.5);
            let (vx, vy) = [(cx - 0.5, cy), (cx + 0.5, cy), (cx, cy - 0.5), (cx, cy + 0.5)][v];
            let (ax, ay, bx, by) = if starts { (vx, vy, vx + dx * 1.5, vy + dy * 1.5) } else { (vx - dx * 1.5, vy - dy * 1.5, vx, vy) };
            vertex(s, ax, ay, 0.5, color(i as u32 * 37 + 2 * cell as u32), 16.0, 16.0);
            vertex(s, bx, by, 0.5, color(i as u32 * 37 + 2 * cell as u32 + 1), 16.0, 16.0);
        }
        writeln!(s, "draw 2 32 32").unwrap();
        if ms {
            writeln!(s, "resolve 1 0 0 0 28\nread 1 0 64 16 1").unwrap();
        } else {
            read_rt(s, 0, 28, 16, 16);
        }
        writeln!(s, "end").unwrap();
    }
    // x-major lines from a corner shared by two pixels above each other (a top corner going
    // up, a bottom one going down, rightward and leftward), of five slopes and three lengths:
    // probe-rt-ms4-lines-over lit the lower pixel where the 1.5-pixel lines of probe-linevert-*
    // light the upper one; two lines a case, in the upper and lower halves
    let long: Vec<(f32, f32, f32)> = [1.5f32, 4.0, 14.0].into_iter().flat_map(|len| [0.125f32, 0.25, 0.414, 0.5, 0.75].into_iter().flat_map(move |k| [(1.0f32, -1.0f32), (1.0, 1.0), (-1.0, -1.0), (-1.0, 1.0)].into_iter().map(move |(sx, sy)| (len * sx, len * k * sy, sy)))).collect();
    for (i, pair) in long.chunks(2).enumerate() {
        begin(s, dev, &format!("probe-linecorner-{i}"), 16, 16, 28, "ps");
        for (half, &(dx, dy, sy)) in pair.iter().enumerate() {
            // (the start on the corner between rows 3 and 4, or 11 and 12, at column 1 or 14's
            // center)
            let x = if dx > 0.0 { 1.5 } else { 14.5 };
            let y = if half == 0 { 4.0 } else { 12.0 } + if sy < 0.0 { 3.0 } else { -3.0 } * f32::from(u8::from(dy.abs() > 3.0));
            vertex(s, x, y, 0.5, color(i as u32 * 7 + 2 * half as u32), 16.0, 16.0);
            vertex(s, x + dx, y + dy, 0.5, color(i as u32 * 7 + 2 * half as u32 + 1), 16.0, 16.0);
        }
        writeln!(s, "draw 2 4 32").unwrap();
        read_rt(s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // x-major lines from a pixel center through diamond corners at every other column (slope
    // 1/2) or every fourth (1/4), of major lengths 5 to 15, rightward and leftward, down and up:
    // probe-linecorner-22 lit the pixel below or above such a corner by its distance along the
    // line
    for (k, (slope, sx)) in [(0.5f32, 1.0f32), (0.5, -1.0), (-0.5, 1.0), (0.25, 1.0), (0.25, -1.0), (-0.25, 1.0)].into_iter().enumerate() {
        for len in 5..=15u8 {
            let len = f32::from(len);
            begin(s, dev, &format!("probe-linetie-{k}-{len}"), 16, 16, 28, "ps");
            let x = if sx > 0.0 { 0.5 } else { 15.5 };
            let y = if slope > 0.0 { 0.5 } else { 15.5 };
            vertex(s, x, y, 0.5, color(k as u32 * 3), 16.0, 16.0);
            vertex(s, x + sx * len, y + slope * len, 0.5, color(k as u32 * 3 + 1), 16.0, 16.0);
            writeln!(s, "draw 2 2 32").unwrap();
            read_rt(s, 0, 28, 16, 16);
            writeln!(s, "end").unwrap();
        }
    }
    for i in 0..4u32 {
        begin(s, dev, &format!("probe-linesw-{i}"), 16, 16, 2, "ps");
        for _ in 0..8 {
            let (x, y, w) = (px(&mut r3, 0, 16), px(&mut r3, 0, 16), 0.25 + 3.75 * unit(&mut r3));
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = [unit(&mut r3), unit(&mut r3), unit(&mut r3), unit(&mut r3)];
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
        }
        writeln!(s, "draw 2 8 32").unwrap();
        read_rt(s, 0, 2, 16, 16);
        writeln!(s, "end").unwrap();
    }
    let some = |s: &mut String, r: &mut XorShift32, n: u32| {
        for k in 0..n {
            vertex(s, px(r, 0, 16), px(r, 0, 16), 0.5, color(k * 7 + 3), 16.0, 16.0);
        }
    };
    for (name, rast) in [("ms", "rast 3 1 0 0 0 0 1 0 1 0"), ("aa", "rast 3 1 0 0 0 0 1 0 0 1"), ("msaa", "rast 3 1 0 0 0 0 1 0 1 1")] {
        begin(s, dev, &format!("probe-lines-{name}"), 16, 16, 28, "ps");
        writeln!(s, "{rast}").unwrap();
        some(s, &mut r3, 8);
        writeln!(s, "draw 2 8 32").unwrap();
        read_rt(s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    // to fit D3DRASTER.LINE.QUAD to the bit, and the alpha antialiased lines that
    // D3DRASTER.TOPOLOGY leaves open, in float targets: horizontal and vertical lines with a
    // row (column) of centers 176/256 to 183/256 from them (a half width of 0.7 or of
    // sqrt 2 / 2) and ends -4/256 to 3/256 from a center; lines alone in 8 x 8 cells, half of
    // them with w
    // (their own numbers, so the cases after them keep theirs)
    let mut r4 = XorShift32::new(0x9_11e5);
    for (name, rast) in [("ms", "rast 3 1 0 0 0 0 1 0 1 0"), ("aa", "rast 3 1 0 0 0 0 1 0 0 1")] {
        for vert in [false, true] {
            begin(s, dev, &format!("probe-lines{name}-{}", if vert { "col" } else { "row" }), 16, 16, 2, "ps");
            writeln!(s, "{rast}").unwrap();
            for k in 0..8u32 {
                let off = (2 * k) as f32 + 0.5 + (176 + k) as f32 / 256.0;
                let (e0, e1) = (1.5 + (k as f32 - 4.0) / 256.0, 14.5 + (3.0 - k as f32) / 256.0);
                let (a, b) = if vert { ((off, e0), (off, e1)) } else { ((e0, off), (e1, off)) };
                vertex(s, a.0, a.1, 0.5, color(2 * k), 16.0, 16.0);
                vertex(s, b.0, b.1, 0.5, color(2 * k + 1), 16.0, 16.0);
            }
            writeln!(s, "draw 2 16 32").unwrap();
            read_rt(s, 0, 2, 16, 16);
            writeln!(s, "end").unwrap();
        }
        for i in 0..4u32 {
            begin(s, dev, &format!("probe-lines{name}-{i}"), 16, 16, 2, "ps");
            writeln!(s, "{rast}").unwrap();
            for cell in 0..4u32 {
                let (ox, oy) = ((cell % 2 * 8) as f32, (cell / 2 * 8) as f32);
                for _ in 0..2 {
                    let (x, y) = (ox + 1.75 + 4.5 * unit(&mut r4), oy + 1.75 + 4.5 * unit(&mut r4));
                    let grid = |v: f32| (v * 256.0).round() / 256.0;
                    let w = if cell % 2 == 1 { 0.25 + 3.75 * unit(&mut r4) } else { 1.0 };
                    let (nx, ny) = (grid(x) / 16.0 * 2.0 - 1.0, 1.0 - grid(y) / 16.0 * 2.0);
                    let c = [unit(&mut r4), unit(&mut r4), unit(&mut r4), unit(&mut r4)];
                    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
                }
            }
            writeln!(s, "draw 2 8 32").unwrap();
            read_rt(s, 0, 2, 16, 16);
            writeln!(s, "end").unwrap();
        }
    }
    for top in [10u32, 11, 12, 13] {
        begin(s, dev, &format!("probe-topology-{top}"), 16, 16, 28, "ps");
        some(s, &mut r3, 12);
        writeln!(s, "draw {top} 12 32").unwrap();
        read_rt(s, 0, 28, 16, 16);
        writeln!(s, "end").unwrap();
    }
    for (top, n) in [(1u32, 12u32), (2, 8)] {
        for ps in ["psflat", "pspos", "psprim"] {
            begin(s, dev, &format!("probe-top{top}-{ps}"), 16, 16, 2, ps);
            some(s, &mut r3, n);
            writeln!(s, "draw {top} {n} 32").unwrap();
            read_rt(s, 0, 2, 16, 16);
            writeln!(s, "end").unwrap();
        }
    }
    // what D3DRASTER.CLIPW, CLIPZ and FILTER leave open: a vertex behind the eye with the
    // triangle keeping an area, the depth of a triangle crossing the depth range as
    // SV_Position, minification, linear filtering of 16-bit UNORM and float texels
    begin(s, dev, "probe-clip-w2", 16, 16, 28, "ps");
    for (x, y, w) in [(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)] {
        let c = color(9);
        writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x * w), fb(y * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
    }
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 28, 16, 16);
    writeln!(s, "end").unwrap();
    begin(s, dev, "probe-clip-w3", 16, 16, 2, "ps");
    for (k, (x, y, w)) in [(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter().enumerate() {
        let c = color(k as u32 * 5 + 1);
        writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x * w.abs()), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3])).unwrap();
    }
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 2, 16, 16);
    writeln!(s, "end").unwrap();
    // a line crossing the depth range, depth clipping on: read through SV_Position
    begin(s, dev, "probe-line-clip-z", 16, 16, 2, "pspos");
    vertex(s, 1.5, 2.25, -0.5, [1.0; 4], 16.0, 16.0);
    vertex(s, 14.25, 13.5, 1.5, [1.0; 4], 16.0, 16.0);
    writeln!(s, "draw 2 2 32").unwrap();
    read_rt(s, 0, 2, 16, 16);
    writeln!(s, "end").unwrap();
    // depths that vary along unclipped lines, x-major and y-major, w = 1 and not
    begin(s, dev, "probe-line-z", 16, 16, 2, "pspos");
    vertex(s, 0.75, 1.25, 0.125, [1.0; 4], 16.0, 16.0);
    vertex(s, 15.25, 6.5, 0.875, [1.0; 4], 16.0, 16.0);
    vertex(s, 3.25, 7.5, 0.9, [1.0; 4], 16.0, 16.0);
    vertex(s, 9.75, 15.5, 0.05, [1.0; 4], 16.0, 16.0);
    writeln!(s, "draw 2 4 32").unwrap();
    read_rt(s, 0, 2, 16, 16);
    writeln!(s, "end").unwrap();
    begin(s, dev, "probe-clip-z-pos", 16, 16, 2, "pspos");
    tri(s, [-0.5, 0.5, 1.5], one);
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 2, 16, 16);
    writeln!(s, "end").unwrap();
    for (name, filter, fmt, uv) in [("min", 0x10u32, 28u32, (-2.0f32, 3.0f32)), ("min-point", 0x04, 28, (-1.5, 2.5)), ("unorm16", 0x15, 11, (-0.2, 0.9)), ("float", 0x15, 2, (-0.2, 0.9)), ("rho-1024", 0x10, 28, (-1.5, 2.5 + 4.0 / 1024.0)), ("rho-64", 0x10, 28, (-1.5, 2.5 + 4.0 / 64.0)), ("rho-10", 0x10, 28, (-1.5, 2.9)), ("rho-under", 0x10, 28, (-1.5, 2.5 - 4.0 / 1024.0))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let pitch = format(fmt).unwrap().row_bytes(4);
        writeln!(s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // what D3DRASTER.FILTER leaves open: anisotropic filtering, linear filtering of sRGB and
    // SNORM texels, minifying a texture with mips (two texels a pixel), between two levels
    // (spans of 1.0625, 1.25, sqrt 2, 1.75, 2.5 and 3 texels; float, 16-bit UNORM and point texels)
    for (name, filter, fmt, mips, uv) in [("aniso", 0x55u32, 28u32, 1u32, (-0.2f32, 0.9f32)), ("srgb", 0x15, 29, 1, (-0.2, 0.9)), ("snorm", 0x15, 31, 1, (-0.2, 0.9)), ("mips", 0x15, 28, 3, (-3.5, 4.5)), ("trilinear", 0x15, 28, 3, (-1.5, 4.5)), ("aniso-min", 0x55, 28, 1, (-3.5, 4.5)), ("r10", 0x15, 24, 1, (-0.2, 0.9)), ("mips-point", 0x14, 28, 3, (-1.5, 4.5)), ("trilinear-1", 0x15, 28, 3, (-1.0, 3.25)), ("trilinear-2", 0x15, 28, 3, (-1.0, 4.0)), ("trilinear-3", 0x15, 28, 3, (-1.0, 4.656854)), ("trilinear-4", 0x15, 28, 3, (-1.0, 6.0)), ("trilinear-5", 0x15, 28, 3, (-1.0, 9.0)), ("trilinear-6", 0x15, 28, 3, (-1.0, 11.0)), ("trilinear-float", 0x15, 2, 3, (-1.5, 4.5)), ("trilinear-point", 0x1, 28, 3, (-1.5, 4.5)), ("trilinear-unorm16", 0x15, 11, 3, (-1.0, 6.0)), ("r11g11b10", 0x15, 26, 1, (-0.2, 0.9)), ("rgb9e5", 0x15, 67, 1, (-0.2, 0.9)), ("b5g6r5", 0x15, 85, 1, (-0.2, 0.9)), ("b5g5r5a1", 0x15, 86, 1, (-0.2, 0.9)), ("b4g4r4a4", 0x15, 115, 1, (-0.2, 0.9)), ("trilinear-point-2", 0x1, 28, 3, (-1.0, 6.0)), ("trilinear-point-float", 0x1, 2, 3, (-1.0, 9.0)), ("bc1", 0x15, 71, 1, (-0.2, 0.9)), ("bc3", 0x15, 77, 1, (-0.2, 0.9)), ("bc7", 0x15, 98, 1, (-0.2, 0.9)), ("r8g8", 0x15, 49, 1, (-0.2, 0.9)), ("a8", 0x15, 65, 1, (-0.2, 0.9)), ("r8", 0x15, 61, 1, (-0.2, 0.9)), ("bc2", 0x15, 74, 1, (-0.2, 0.9)), ("bc4", 0x15, 80, 1, (-0.2, 0.9)), ("bc4s", 0x15, 81, 1, (-0.2, 0.9)), ("bc5", 0x15, 83, 1, (-0.2, 0.9)), ("bc5s", 0x15, 84, 1, (-0.2, 0.9)), ("r24x8", 0x15, 46, 1, (-0.2, 0.9)), ("r32x8x24", 0x15, 21, 1, (-0.2, 0.9)), ("bc6h", 0x15, 95, 1, (-0.2, 0.9)), ("bc1-srgb", 0x15, 72, 1, (-0.2, 0.9)), ("b8g8r8a8-srgb", 0x15, 91, 1, (-0.2, 0.9)), ("r8g8b8a8-mips", 0x15, 28, 3, (-1.0, 9.0))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let pitch = format(fmt).unwrap().row_bytes(4);
        writeln!(s, "tex 1 2 4 4 1 {mips} {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // sample_c: a comparison filter (point, linear) and every comparison function, against a
    // reference that runs across the triangle, of a 16-bit UNORM and a 32-bit float texture
    for (filter, func, fmt) in [0x80u32, 0x95].into_iter().flat_map(|f| (1..=8u32).map(move |c| (f, c, 56u32))).chain([(0x95, 2, 41), (0x80, 4, 41)]) {
        writeln!(s, "case probe-sample-cmp-{filter:x}-{func}-{fmt}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let pitch = format(fmt).unwrap().row_bytes(4);
        writeln!(s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3 {func}").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssamplec"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v, r) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32, 0.0f32), (1.0, 1.0, 0.9, -0.2, 1.0), (-1.0, -1.0, -0.2, 0.9, 0.0), (1.0, -1.0, 0.9, 0.9, 1.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(r), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // what D3DRASTER.FILTER leaves open: a view of its own range of levels (levels 1 and 2 of
    // three, minifying and magnifying)
    // (and what D3DRASTER.FILTER leaves open: a 2D array of 2 slices with mips, its whole view)
    for (name, uv) in [("view-mips", (-1.5f32, 4.5f32)), ("view-mips-mag", (-0.2, 0.9)), ("array-mips", (-1.5, 4.5)), ("cube-mips", (-1.5, 4.5)), ("array1-mips", (-1.5, 4.5))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        let (tex, srv) = match name {
            "array-mips" => ("tex 1 2 4 4 2 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 nodesc"),
            // (its view from slice 1)
            "array1-mips" => ("tex 1 2 4 4 2 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 28 5 0 3 1 1"),
            // (a cube of 6 faces with mips, its cube view)
            "cube-mips" => ("tex 1 2 4 4 6 3 28 0 8 0 4 1 0 init 400 16 0", "srv 1 1 nodesc"),
            _ => ("tex 1 2 4 4 1 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 28 4 1 2"),
        };
        writeln!(s, "{tex}\n{srv}\ntsrv 1\nsamp 0x15 3 3 3").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // what D3DRASTER.FILTER leaves open: anisotropic filtering of a footprint longer one way
    // than the other (2 to 1, 4 to 1, 16 to 1), and over a texture with mips
    for (name, mips, u, v) in [("aniso-2", 1u32, (-3.5f32, 4.5f32), (-1.5f32, 2.5f32)), ("aniso-4", 1, (-3.5, 4.5), (-0.5, 1.5)), ("aniso-16", 1, (-7.5, 8.5), (-0.5, 1.5)), ("aniso-mips", 3, (-1.5, 4.5), (-1.5, 4.5)), ("aniso-mips-2", 3, (-3.5, 4.5), (-1.5, 2.5))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0").unwrap();
        writeln!(s, "tex 1 2 4 4 1 {mips} 28 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x55 3 3 3").unwrap();
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader("vs")), hex(&shader("pssample"))).unwrap();
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0)).unwrap();
        for (x, y, a, b) in [(-1.0f32, 1.0f32, u.0, v.0), (1.0, 1.0, u.1, v.0), (-1.0, -1.0, u.0, v.1), (1.0, -1.0, u.1, v.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(a), fb(b), fb(1.0)).unwrap();
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend").unwrap();
    }
    // a constant buffer bound (no program reads it)
    begin(s, dev, "probe-cbuf", 16, 16, 28, "ps");
    writeln!(s, "buf 2 16 0 4 0 0 0\ncbuf 2").unwrap();
    tri(s, half, one);
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 28, 16, 16);
    writeln!(s, "end").unwrap();
}
