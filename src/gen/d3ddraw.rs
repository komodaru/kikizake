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
    // an input layout against a refused program
    let mut broken = shader("vs");
    broken[5] ^= 1;
    writeln!(s, "case probe-layout-refused\n{dev}\nshader 0 vs {}\nlayout 0 2 2\nend", hex(&broken)).unwrap();
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
    // a constant buffer bound (no program reads it)
    begin(s, dev, "probe-cbuf", 16, 16, 28, "ps");
    writeln!(s, "buf 2 16 0 4 0 0 0\ncbuf 2").unwrap();
    tri(s, half, one);
    writeln!(s, "draw 4 3 32").unwrap();
    read_rt(s, 0, 28, 16, 16);
    writeln!(s, "end").unwrap();
}
