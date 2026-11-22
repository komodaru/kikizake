//! The d3d10draw and d3d11draw suites' case generators: triangles through the rasterizer
//! (coverage, culling, scissor, viewports), the pixel shader's system values, depth tests,
//! blending, and textures read by ld and sample (how every format decodes).
use super::dxbc::*;
use super::*;
use crate::model::dxgifmt::{Format, Num, FORMATS};

pub use crate::shaders::{shaders, Prog};

pub fn shader(p: Prog) -> Vec<u8> {
    shaders().get(p).to_vec()
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
    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx), fb(ny), fb(z), fb(1.0), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
}

/// what every drawing case starts with: the device, a w x h render target of `fmt` cleared to
/// 0 in slot 0 (view 0), the vs and `ps` programs, the layout, a viewport over the target, a
/// rasterizer state that culls nothing
fn begin(s: &mut String, dev: &str, name: &str, w: u32, h: u32, fmt: u32, ps: Prog) {
    writeln!(s, "case {name}\n{dev}");
    writeln!(s, "tex 0 2 {w} {h} 1 1 {fmt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(ps)));
    writeln!(s, "viewport 0 0 {} {} 0 {}", fb(w as f32), fb(h as f32), fb(1.0));
    writeln!(s, "rast 3 1 0 0 0 0 1 0 0 0");
}

fn read_rt(s: &mut String, slot: u32, f: &Format, w: u32, h: u32) {
    writeln!(s, "read {slot} 0 {} {h} 1", f.row_bytes(w));
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
        begin(&mut s, dev, &format!("tri-{i}"), 16, 16, 28, Prog::Ps);
        let n = r.range(1, 6) as u32;
        for t in 0..n {
            let sub = i % 2 == 0;
            for _ in 0..3 {
                vertex(&mut s, coord(&mut r, sub), coord(&mut r, sub), 0.5, color(i * 7 + t), 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 {} 32", n * 3);
        read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // a grid of quads (two triangles each, sharing the diagonal), every pixel covered once:
    // the fill rule on shared edges; corners on pixel centers and between them
    for (gi, (step, off)) in [(4.0f32, 0.0f32), (4.0, 0.5), (3.0, 0.25), (2.5, 0.5), (4.0, 1.0 / 256.0), (5.0, 0.5 - 1.0 / 256.0)].iter().enumerate() {
        begin(&mut s, dev, &format!("grid-{gi}"), 16, 16, 28, Prog::Ps);
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
                writeln!(s, "draw 4 {n} 32");
                n = 0;
            }
        }
        if n > 0 {
            writeln!(s, "draw 4 {n} 32");
        }
        read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // the same grids with additive blending: a pixel drawn twice shows the sum
    for (gi, (step, off)) in [(4.0f32, 0.5f32), (3.0, 0.25)].iter().enumerate() {
        begin(&mut s, dev, &format!("gridadd-{gi}"), 16, 16, 28, Prog::Ps);
        writeln!(s, "blend1 1 2 2 1 2 2 1 0xf");
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
            writeln!(s, "draw 4 {n} 32");
            y += step;
        }
        read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // culling: a clockwise and a counterclockwise triangle under each cull mode and winding
    for cull in [1, 2, 3] {
        for ccw in [0, 1] {
            begin(&mut s, dev, &format!("cull-{cull}-{ccw}"), 8, 8, 28, Prog::Psprim);
            writeln!(s, "rast 3 {cull} {ccw} 0 0 0 1 0 0 0");
            for (a, b, c) in [((0.0, 0.0), (8.0, 0.0), (0.0, 8.0)), ((8.0, 8.0), (8.0, 0.5), (0.5, 8.0))] {
                for p in [a, b, c] {
                    vertex(&mut s, p.0, p.1, 0.5, [1.0; 4], 8.0, 8.0);
                }
            }
            writeln!(s, "draw 4 6 32");
            read_rt(&mut s, 0, crate::dxgi!(28), 8, 8);
            writeln!(s, "end");
        }
    }
    // scissor rectangles (on and off)
    for (i, rc) in [(2, 3, 6, 7), (0, 0, 8, 8), (-5, -5, 3, 20), (5, 5, 5, 6), (6, 6, 2, 2), (7, 0, 100, 1), (0, 0, 0, 0)].iter().enumerate() {
        for on in [1, 0] {
            begin(&mut s, dev, &format!("scissor-{i}-{on}"), 8, 8, 28, Prog::Ps);
            writeln!(s, "rast 3 1 0 0 0 0 1 {on} 0 0\nscissor {} {} {} {}", rc.0, rc.1, rc.2, rc.3);
            for p in [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0), (8.0, 8.0)] {
                vertex(&mut s, p.0, p.1, 0.5, color(i as u32), 8.0, 8.0);
            }
            writeln!(s, "draw 5 4 32");
            read_rt(&mut s, 0, crate::dxgi!(28), 8, 8);
            writeln!(s, "end");
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
        begin(&mut s, dev, &format!("viewport-{i}"), 8, 8, 2, Prog::Pspos);
        writeln!(s, "viewport {}", vp.iter().map(|v| fb(*v)).collect::<Vec<_>>().join(" "));
        for (x, y) in [(-0.75f32, 0.75f32), (0.75, 0.5), (-0.5, -0.75)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(1.0));
        }
        writeln!(s, "draw 4 3 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 8, 8);
        writeln!(s, "end");
    }
    // SV_Position: a quad over the target at constant depths and w
    for (i, (z, w)) in [(0.5f32, 1.0f32), (0.0, 1.0), (1.0, 1.0), (0.25, 2.0), (0.75, 0.5)].iter().enumerate() {
        begin(&mut s, dev, &format!("pos-{i}"), 4, 4, 2, Prog::Pspos);
        for (x, y) in [(-1.0f32, 1.0f32), (1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 {}", fb(x * w), fb(y * w), fb(z * w), fb(*w), fb(1.0));
        }
        writeln!(s, "draw 5 4 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 4, 4);
        writeln!(s, "end");
    }
    // vertex, instance and primitive ids, the front face, for each topology of triangles
    for (top, n, inst) in [(4u32, 6u32, 0u32), (5, 6, 0), (4, 3, 3), (5, 4, 2)] {
        writeln!(s, "case ids-{top}-{n}-{inst}\n{dev}");
        writeln!(s, "tex 0 2 8 1 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0");
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(8.0), fb(1.0), fb(1.0));
        writeln!(s, "shader 0 vs {}\nlayout 0 2\nshader 1 ps {}", hex(&shader(Prog::Vsid)), hex(&shader(Prog::Psflat)));
        // (list: triangle t over pixel column t; strip: vertex k at x = k, alternately above
        // and below, so triangle i spans columns i and i + 1)
        let pos = |s: &mut String| {
            for v in 0..n {
                let (x, y) = if top == 4 { ((v / 3) as f32 + [0.0, 1.0, 0.5][v as usize % 3], [-10.0f32, -10.0, 10.0][v as usize % 3]) } else { (v as f32, if v % 2 == 0 { -10.0 } else { 10.0 }) };
                writeln!(s, "vtx {} {} {} {}", fb(x / 4.0 - 1.0), fb(-y), fb(0.5), fb(1.0));
            }
        };
        pos(&mut s);
        writeln!(s, "draw {top} {n} 16{}", if inst > 0 { format!(" {inst}") } else { String::new() });
        read_rt(&mut s, 0, crate::dxgi!(2), 8, 1);
        writeln!(s, "shader 1 ps {}\nclear 0 0 0 0 0", hex(&shader(Prog::Psprim)));
        pos(&mut s);
        writeln!(s, "draw {top} {n} 16{}", if inst > 0 { format!(" {inst}") } else { String::new() });
        read_rt(&mut s, 0, crate::dxgi!(2), 8, 1);
        writeln!(s, "end");
    }
    // depth: triangles at constant depths through each comparison, with and without writes,
    // into each depth format; the color and the depth read back
    let zs = [0.5f32, 0.25, 0.75, 0.5, 1.0, 0.0, 0.3, 0.7];
    for (dfm, tl) in [(crate::dxgi!(40), 40u32), (crate::dxgi!(55), 55), (crate::dxgi!(45), 45), (crate::dxgi!(20), 20)] {
        let df = dfm.id;
        for func in 1..=8 {
            for write in [1, 0] {
                begin(&mut s, dev, &format!("depth-{df}-{func}-{write}"), 8, 8, 28, Prog::Ps);
                writeln!(s, "tex 1 2 8 8 1 1 {tl} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 {write} {func}", fb(0.5));
                for (i, z) in zs.iter().enumerate() {
                    let x0 = (i % 4) as f32 * 2.0;
                    for p in [(x0 - 1.0, -1.0), (x0 + 6.0, 0.0), (x0, 9.0)] {
                        vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
                    }
                }
                writeln!(s, "draw 4 {} 32", zs.len() * 3);
                read_rt(&mut s, 0, crate::dxgi!(28), 8, 8);
                writeln!(s, "{}", super::d3dres::read_line(1, 0, dfm, 8, 8, 1, 0));
                writeln!(s, "end");
            }
        }
        // depth values written: a ramp of constant-depth triangles
        begin(&mut s, dev, &format!("depthval-{df}"), 8, 8, 28, Prog::Ps);
        writeln!(s, "tex 1 2 8 8 1 1 {tl} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 8", fb(1.0));
        let vals = [0.0f32, 1.0, 0.5, 1.0 / 3.0, 0.1, 0.9, 1.0 / 65535.0, 0.5 / 65535.0, 1.5 / 65535.0, 1.0 / 16777215.0, 0.5 / 16777215.0, 2.5 / 16777215.0, 0.999_999_9, 1e-20, 0.25, 0.75];
        for (i, z) in vals.iter().enumerate() {
            let (x, y) = ((i % 4) as f32 * 2.0, (i / 4) as f32 * 2.0);
            for p in [(x, y), (x + 2.0, y), (x, y + 2.0), (x + 2.0, y), (x + 2.0, y + 2.0), (x, y + 2.0)] {
                vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
            }
        }
        writeln!(s, "draw 4 {} 32", vals.len() * 6);
        writeln!(s, "{}", super::d3dres::read_line(1, 0, dfm, 8, 8, 1, 0));
        writeln!(s, "end");
    }
    // depth clipping: triangles outside 0..1, clipped or clamped
    for clip in [1, 0] {
        begin(&mut s, dev, &format!("depthclip-{clip}"), 8, 8, 28, Prog::Ps);
        writeln!(s, "rast 3 1 0 0 0 0 {clip} 0 0 0\ntex 1 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 8", fb(0.5));
        for (i, z) in [-0.5f32, 1.5, -0.0, 1.0, 2.0, -1e-7].iter().enumerate() {
            let x = i as f32;
            for p in [(x, 0.0), (x + 1.0, 0.0), (x, 8.0), (x + 1.0, 0.0), (x + 1.0, 8.0), (x, 8.0)] {
                vertex(&mut s, p.0, p.1, *z, color(i as u32), 8.0, 8.0);
            }
        }
        writeln!(s, "draw 4 36 32");
        read_rt(&mut s, 0, crate::dxgi!(28), 8, 8);
        writeln!(s, "read 1 0 32 8 1\nend");
    }
    // blending: every factor for source and destination, each operation, on unorm, float and
    // srgb targets; the target cleared to one color, a quad of another drawn over it
    let factors: Vec<u32> = (1..=11).chain([14, 15]).collect();
    let src_c = [0.25f32, 0.5, 0.75, 0.6];
    let dst_c = [0.8f32, 0.4, 0.2, 0.3];
    let bf = [0.2f32, 0.4, 0.6, 0.8];
    for f in crate::fmts![28, 10, 2, 29, 24] {
        let fmt = f.id;
        for (ci, chunk) in factors.iter().flat_map(|a| factors.iter().map(move |b| (*a, *b))).collect::<Vec<_>>().chunks(13).enumerate() {
            writeln!(s, "case blend-{fmt}-{ci}\n{dev}");
            writeln!(s, "tex 0 2 1 1 1 1 {fmt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)));
            writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(1.0), fb(1.0), fb(1.0));
            for (k, (a, b)) in chunk.iter().enumerate() {
                let op = [1u32, 2, 3, 4, 5][k % 5];
                writeln!(s, "clear 0 {} {} {} {}", fb(dst_c[0]), fb(dst_c[1]), fb(dst_c[2]), fb(dst_c[3]));
                // (alpha factors: the _COLOR ones are not allowed)
                let al = |f: u32| match f {
                    3 => 5,
                    4 => 6,
                    9 => 7,
                    10 => 8,
                    f => f,
                };
                writeln!(s, "blend1 1 {a} {b} {op} {} {} {} 0xf {} {} {} {}", al(*a), al(*b), [1u32, 2, 3, 4, 5][(k + 2) % 5], fb(bf[0]), fb(bf[1]), fb(bf[2]), fb(bf[3]));
                for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
                    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0), fb(p.1), fb(0.5), fb(1.0), fb(src_c[0]), fb(src_c[1]), fb(src_c[2]), fb(src_c[3]));
                }
                writeln!(s, "draw 4 3 32");
                read_rt(&mut s, 0, f, 1, 1);
            }
            writeln!(s, "end");
        }
    }
    // write masks
    writeln!(s, "case writemask\n{dev}\ntex 0 2 16 1 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)));
    writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(1.0), fb(1.0));
    for m in 0..16 {
        writeln!(s, "viewport {} 0 {} {} 0 {}\nblend1 0 2 1 1 2 1 1 {m}", fb(m as f32), fb(1.0), fb(1.0), fb(1.0));
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0), fb(p.1), fb(0.5), fb(1.0), fb(1.0), fb(0.5), fb(0.25), fb(1.0));
        }
        writeln!(s, "draw 4 3 32");
    }
    read_rt(&mut s, 0, crate::dxgi!(28), 16, 1);
    writeln!(s, "end");
    // every format with shader resource support read by ld (a 4x4 texture of random bytes, to
    // a float or uint target): how each format decodes
    for f in FORMATS.iter().filter(|f| !f.video() && f.num != Num::Typeless && !matches!(f.id, 19 | 20 | 22 | 23 | 40 | 44 | 45 | 47 | 55 | 66) && (d3d11 || !(94..=99).contains(&f.id))) {
        let int = matches!(f.num, Num::Uint | Num::Sint);
        let (rt, ps) = if int { (3u32, Prog::Psldu) } else { (2, Prog::Psld) };
        let (w, ht) = (4 * f.bw.min(2), 4 * f.bh);
        writeln!(s, "case ld-{}\n{dev}", f.id);
        writeln!(s, "tex 0 2 {w} {ht} 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(s, "tex 1 2 {w} {ht} 1 1 {} 0 8 0 0 1 0 init {} {} 0\nsrv 1 1 nodesc\ntsrv 1", f.id, f.id * 3 + 1, f.row_bytes(w));
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(ps)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(w as f32), fb(ht as f32), fb(1.0));
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0));
        }
        writeln!(s, "draw 4 3 32");
        writeln!(s, "read 0 0 {} {ht} 1", 16 * w);
        writeln!(s, "end");
    }
    // tables: 32x32 textures (every byte value many times over) of the formats whose decoding
    // is fitted to Windows' answers (sRGB, signed and HDR / mode-based blocks)
    for f in crate::fmts![29, 91, 81, 84, 95, 96, 98, 99] {
        let id = f.id;
        if !d3d11 && (94..=99).contains(&id) {
            continue;
        }
        writeln!(s, "case ld-table-{id}\n{dev}");
        writeln!(s, "tex 0 2 32 32 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(s, "tex 1 2 32 32 1 1 {id} 0 8 0 0 1 0 init {} {} 0\nsrv 1 1 nodesc\ntsrv 1", id * 5 + 3, f.row_bytes(32));
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Psld)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(32.0), fb(32.0), fb(1.0));
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0));
        }
        writeln!(s, "draw 4 3 32\nread 0 0 512 32 1\nend");
    }
    // depth formats read through their shader resource formats
    for (tl, view) in [(40u32, 41u32), (55, 56), (45, 46), (20, 21), (45, 47)] {
        let rt = if view == 47 { 3 } else { 2 };
        let ps = if view == 47 { Prog::Psldu } else { Prog::Psld };
        writeln!(s, "case ld-depth-{tl}-{view}\n{dev}");
        writeln!(s, "tex 0 2 4 4 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        let tfm = match tl {
            40 => crate::dxgi!(39),
            55 => crate::dxgi!(53),
            45 => crate::dxgi!(44),
            _ => crate::dxgi!(19),
        };
        let tf = tfm.id;
        writeln!(s, "tex 1 2 4 4 1 1 {tf} 0 0x48 0 0 1 0 init {} {} 0\nsrv 1 1 {view} 4 0 1", tl + 7, tfm.row_bytes(4));
        writeln!(s, "tsrv 1\nshader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(ps)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(4.0), fb(4.0), fb(1.0));
        for p in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)] {
            writeln!(s, "vtx {} {} {} {} 0 0 0 0", fb(p.0), fb(p.1), fb(0.5), fb(1.0));
        }
        writeln!(s, "draw 4 3 32\nread 0 0 64 4 1\nend");
    }
    // sampling a 4x4 texture with point filtering at texel centers from -1.5 to 2.5 under each
    // address mode (16 pixels: every texel four times over)
    for addr in 1..=5 {
        for filter in [0u32, 0x15] {
            writeln!(s, "case sample-{addr}-{filter:x}\n{dev}");
            writeln!(s, "tex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
            writeln!(s, "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init {} 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter} {addr} {addr} {addr}", 77 + addr);
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
            writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
            for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 2.5, -1.5), (-1.0, -1.0, -1.5, 2.5), (1.0, -1.0, 2.5, 2.5)] {
                writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
            }
            writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
        }
    }
    probes(&mut s, dev);
    // shaders the runtime refuses: a broken checksum, a truncated container, nothing
    let mut broken = shader(Prog::Ps);
    broken[5] ^= 1;
    let short = shader(Prog::Ps)[..40].to_vec();
    writeln!(s, "case shader-bad\n{dev}\nshader 0 ps {}\nshader 1 ps {}\nshader 2 vs {}\nshader 3 ps 00\nend", hex(&broken), hex(&short), hex(&shader(Prog::Ps)));
    // an input layout that the program's input signature does not match
    writeln!(s, "case layout-bad\n{dev}\nshader 0 vs {}\nlayout 0 2\nlayout 0 2 2 2\nlayout 0 0x1c 2\nlayout 0 0x29 2\nlayout 0 0 2\nlayout 1 2 2\nend", hex(&shader(Prog::Vs)));
    // sample_c through a sampler without comparison, sample through one with it, and the
    // minimum and maximum filters (what D3DRASTER.FILTER leaves open; undefined by the
    // documentation, or a tier WARP may lack: near the end, as WARP may end the executor on them)
    for (name, ps, filter) in [("samplec-nocmp", Prog::Pssamplec, 0x15u32), ("sample-cmp", Prog::Pssample, 0x95), ("sample-min", Prog::Pssample, 0x115), ("sample-max", Prog::Pssample, 0x195), ("sample-min-point", Prog::Pssample, 0x100), ("sample-min-unorm8", Prog::Pssample, 0x115), ("sample-max-mips", Prog::Pssample, 0x195)] {
        writeln!(s, "case probe-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        let tex = match name {
            "sample-min-unorm8" => "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init 400 16 0",
            // (two levels, 24 x 24 and 12 x 12, minified about 1.65 times: between them)
            "sample-max-mips" => "tex 1 2 24 24 1 2 56 0 8 0 0 1 0 init 400 48 0",
            _ => "tex 1 2 4 4 1 1 56 0 8 0 0 1 0 init 400 8 0",
        };
        writeln!(s, "{tex}\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3 2");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(ps)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v, r) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32, 0.0f32), (1.0, 1.0, 0.9, -0.2, 1.0), (-1.0, -1.0, -0.2, 0.9, 0.0), (1.0, -1.0, 0.9, 0.9, 1.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(r), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // a patch list drawn without hull and domain programs: WARP ends the executor there, so
    // it comes last
    // SV_Position's w along lines whose ends' w differ, and depth that varies along
    // multisample lines
    begin(&mut s, dev, "probe-line-w-pos", 16, 16, 2, Prog::Pspos);
    for (x, y, z, w) in [(0.75f32, 1.25f32, 0.25f32, 0.5f32), (15.25, 6.5, 0.5, 3.0), (3.25, 7.5, 0.75, 1.5), (9.75, 15.5, 0.25, 0.75)] {
        let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
        writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(1.0), fb(1.0), fb(1.0), fb(1.0));
    }
    writeln!(&mut s, "draw 2 4 32");
    read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
    writeln!(&mut s, "end");
    begin(&mut s, dev, "probe-linesms-z", 16, 16, 2, Prog::Pspos);
    writeln!(&mut s, "rast 3 1 0 0 0 0 1 0 1 0");
    vertex(&mut s, 0.75, 1.25, 0.125, [1.0; 4], 16.0, 16.0);
    vertex(&mut s, 15.25, 6.5, 0.875, [1.0; 4], 16.0, 16.0);
    vertex(&mut s, 3.25, 7.5, 0.9, [1.0; 4], 16.0, 16.0);
    vertex(&mut s, 9.75, 15.5, 0.05, [1.0; 4], 16.0, 16.0);
    writeln!(&mut s, "draw 2 4 32");
    read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
    writeln!(&mut s, "end");
    // a vertex at w < 0 whose triangle keeps an area: culled either way, through SV_Position;
    // a coordinate not finite
    for (name, cull, ps) in [("cull2", 2u32, Prog::Ps), ("cull3", 3, Prog::Ps), ("pspos", 1, Prog::Pspos), ("nan", 1, Prog::Ps)] {
        begin(&mut s, dev, &format!("probe-clip-w-{name}"), 16, 16, 2, ps);
        writeln!(&mut s, "rast 3 {cull} 0 0 0 0 1 0 0 0");
        for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter()) {
            let c = color(k.wrapping_mul(5).wrapping_add(1));
            let x = if name == "nan" && k == 1 { f32::NAN } else { x * w.abs() };
            writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(&mut s, "draw 4 3 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
        writeln!(&mut s, "end");
    }
    // a 1D array's second slice as the target
    writeln!(&mut s, "case probe-rt-1darray\n{dev}\ntex 0 1 16 1 2 1 28 0 0x20 0 0 1 0\nrtv 0 0 28 3 0 1 1\nclear 0 0 0 0 0\ntarget 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nread 0 0 64 1 1\nread 0 1 64 1 1\nend");
    // a cube array (2 cubes with mips) through its view from the second cube
    writeln!(&mut s, "case probe-filter-cubearray-mips\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(&mut s, "tex 1 2 4 4 12 3 28 0 8 0 4 1 0 init 400 16 0\nsrv 1 1 28 10 0 3 6 1\ntsrv 1\nsamp 0x15 3 3 3");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
    writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
    for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 4.5, -1.5), (-1.0, -1.0, -1.5, 4.5), (1.0, -1.0, 4.5, 4.5)] {
        writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
    }
    writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    // a coordinate infinite
    begin(&mut s, dev, "probe-clip-w-inf", 16, 16, 2, Prog::Ps);
    for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter()) {
        let c = color(k.wrapping_mul(5).wrapping_add(1));
        let x = if k == 1 { f32::INFINITY } else { x * w.abs() };
        writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
    }
    writeln!(&mut s, "draw 4 3 32");
    read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
    writeln!(&mut s, "end");
    // a depth that varies over triangles on a multisampled target with a depth view
    writeln!(&mut s, "case probe-rt-ms4-zplane\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    writeln!(&mut s, "tex 2 2 16 16 1 1 40 0 0x40 0 0 4 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(1.0));
    for (k, t) in (0u32..).zip([[(0.0f32, 0.0f32, 0.1f32), (16.0, 2.5, 0.9), (1.0, 14.0, 0.5)], [(3.0, 1.0, 0.8), (15.5, 15.0, 0.2), (0.0, 9.75, 0.45)]].into_iter()) {
        for (i, (x, y, z)) in (0u32..).zip(t.into_iter()) {
            vertex(&mut s, x, y, z, color(k.wrapping_mul(3).wrapping_add(i)), 16.0, 16.0);
        }
    }
    writeln!(&mut s, "draw 4 6 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // a 3D texture with mips through its whole view
    writeln!(&mut s, "case probe-filter-3d-mips\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(&mut s, "tex 1 3 4 4 2 3 28 0 8 0 0 1 0 init 400 16 64\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
    writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
    for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 4.5, -1.5), (-1.0, -1.0, -1.5, 4.5), (1.0, -1.0, 4.5, 4.5)] {
        writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
    }
    writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    // a target of 16 samples
    writeln!(&mut s, "case probe-rt-ms16\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 16 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // a vertex at w < 0 whose triangle keeps an area: in wireframe, with a depth view, on a
    // multisampled target
    for name in ["wire", "depth", "ms4"] {
        let tex = if name == "ms4" { "tex 0 2 16 16 1 1 2 0 0x20 0 0 4 0" } else { "tex 0 2 16 16 1 1 2 0 0x20 0 0 1 0" };
        writeln!(&mut s, "case probe-clip-w-{name}\n{dev}\n{tex}\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 2 0 0x20 0 0 1 0");
        writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        writeln!(&mut s, "rast {} 1 0 0 0 0 1 0 0 0", if name == "wire" { 2 } else { 3 });
        if name == "depth" {
            writeln!(&mut s, "tex 2 2 16 16 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(1.0));
        }
        for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter()) {
            let c = color(k.wrapping_mul(5).wrapping_add(1));
            writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x * w.abs()), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        let read = if name == "ms4" { "resolve 1 0 0 0 2\nread 1 0 256 16 1" } else { "read 0 0 256 16 1" };
        writeln!(&mut s, "draw 4 3 32\n{read}\nend");
    }
    // a 1D texture with mips through its whole view
    writeln!(&mut s, "case probe-filter-1d-mips\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(&mut s, "tex 1 1 4 1 1 3 28 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
    writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
    for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 4.5, -1.5), (-1.0, -1.0, -1.5, 4.5), (1.0, -1.0, 4.5, 4.5)] {
        writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
    }
    writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    // a float target of 4 samples, resolved where its samples differ
    writeln!(&mut s, "case probe-rt-ms4-float\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 2 0 0x20 0 0 1 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nresolve 1 0 0 0 2\nread 1 0 256 16 1\nend");
    // a buffer's view (16 RGBA8 elements) as the target
    writeln!(&mut s, "case probe-rt-buffer\n{dev}\nbuf 0 64 0 0x20 0 0 0\nrtv 0 0 28 1 0 16\nclear 0 0 0 0 0\ntarget 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nread 0 0 64 1 1\nend");
    // linear filtering of the packed 4:2:2 formats R8G8_B8G8 and G8R8_G8B8
    for fmt in [68u32, 69] {
        writeln!(&mut s, "case probe-filter-fmt{fmt}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(&mut s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
        writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32), (1.0, 1.0, 0.9, -0.2), (-1.0, -1.0, -0.2, 0.9), (1.0, -1.0, 0.9, 0.9)] {
            writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // a multisampled 2D array's second slice as the target (TEXTURE2DMSARRAY)
    writeln!(&mut s, "case probe-rt-ms4-array1\n{dev}\ntex 0 2 16 16 2 1 28 0 0x20 0 0 4 0\nrtv 0 0 28 7 1 1\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nresolve 1 0 0 1 28\nread 1 0 64 16 1\nend");
    // a depth view smaller than the target (8 x 8 under 16 x 16)
    begin(&mut s, dev, "probe-rt-dsv-small", 16, 16, 28, Prog::Ps);
    writeln!(&mut s, "tex 2 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32");
    read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
    writeln!(&mut s, "end");
    // the same with the depth cleared to 0: inside the depth view every pixel fails, outside it
    // tells whether the test is made at all
    begin(&mut s, dev, "probe-rt-dsv-small-fail", 16, 16, 28, Prog::Ps);
    writeln!(&mut s, "tex 2 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(0.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32");
    read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
    writeln!(&mut s, "end");
    // a depth view smaller than a multisampled target (4 samples each)
    begin(&mut s, dev, "probe-rt-ms4-dsv-small", 16, 16, 28, Prog::Ps);
    writeln!(&mut s, "tex 3 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 3 3 nodesc\ntex 2 2 8 8 1 1 40 0 0x40 0 0 4 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 3 2\ndepthst 1 1 2", fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nresolve 0 0 3 0 28");
    read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
    writeln!(&mut s, "end");
    // linear filtering of the packed YUY2 (a pixel pair's Y0 U Y1 V)
    writeln!(&mut s, "case probe-filter-fmt107\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(&mut s, "tex 1 2 4 4 1 1 107 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
    writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
    for (x, y, u, v) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32), (1.0, 1.0, 0.9, -0.2), (-1.0, -1.0, -0.2, 0.9), (1.0, -1.0, 0.9, 0.9)] {
        writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
    }
    writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    // a buffer's view from its fifth element (8 elements)
    writeln!(&mut s, "case probe-rt-buffer-off\n{dev}\nbuf 0 64 0 0x20 0 0 0\nrtv 0 0 28 1 4 8\nclear 0 0 0 0 0\ntarget 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nread 0 0 64 1 1\nend");
    // a 1D array with mips sampled through a Texture2D (TEXTURE1DARRAY view)
    writeln!(&mut s, "case probe-filter-1darray-mips\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(&mut s, "tex 1 1 4 1 2 3 28 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
    writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
    for (x, y, u, v) in [(-1.0f32, 1.0f32, -1.5f32, -1.5f32), (1.0, 1.0, 4.5, -1.5), (-1.0, -1.0, -1.5, 4.5), (1.0, -1.0, 4.5, 4.5)] {
        writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
    }
    writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    // an R8G8B8A8_UINT target of 4 samples, resolved where its samples differ
    writeln!(&mut s, "case probe-rt-ms4-uint\n{dev}\ntex 0 2 16 16 1 1 30 0 0x20 0 0 4 0\nrtv 0 0 nodesc\ntarget 0\ntex 1 2 16 16 1 1 30 0 0x20 0 0 1 0");
    writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(&mut s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(&mut s, "draw 4 3 32\nresolve 1 0 0 0 30\nread 1 0 64 16 1\nend");
    // linear filtering of the other formats the model does not class: R1, Y210, Y216
    for fmt in [66u32, 108, 109] {
        writeln!(&mut s, "case probe-filter-fmt{fmt}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(&mut s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
        writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32), (1.0, 1.0, 0.9, -0.2), (-1.0, -1.0, -0.2, 0.9), (1.0, -1.0, 0.9, 0.9)] {
            writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // topologies D3D11_PRIMITIVE_TOPOLOGY leaves undefined (they may end the executor too, so
    // they come just before)
    for top in [0u32, 6, 7, 8, 9, 65] {
        begin(&mut s, dev, &format!("probe-topology-{top}"), 16, 16, 28, Prog::Ps);
        for (k, f) in (0u32..).zip([0.0f32, 1.0, 2.0]) {
            vertex(&mut s, 2.0 + 5.0 * f, 3.0 + 4.0 * f, 0.5, color(k), 16.0, 16.0);
        }
        writeln!(s, "draw {top} 3 32");
        read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // a vertex at w < 0 in wireframe: culled either way, and with depth clipping off
    for (name, cull, zclip) in [("cull2", 2u32, 1u32), ("cull3", 3, 1), ("noclip", 1, 0)] {
        begin(&mut s, dev, &format!("probe-clip-w-wire-{name}"), 16, 16, 2, Prog::Ps);
        writeln!(&mut s, "rast 2 {cull} 0 0 0 0 {zclip} 0 0 0");
        for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter()) {
            let c = color(k.wrapping_mul(5).wrapping_add(1));
            writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x * w.abs()), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(&mut s, "draw 4 3 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
        writeln!(&mut s, "end");
    }
    // points and lines with a vertex at w < 0 (a point behind the eye; a line from in front
    // to behind, and one wholly behind)
    for (name, top, vs) in [("point", 1u32, [(0.25f32, 0.25f32, -0.5f32), (-0.5, 0.5, 1.0)]), ("line", 2, [(-0.5, 0.5, 1.0), (0.25, -0.75, -0.5)]), ("line-behind", 2, [(-0.5, 0.5, -1.0), (0.25, -0.75, -0.5)])] {
        begin(&mut s, dev, &format!("probe-clip-w-{name}"), 16, 16, 2, Prog::Ps);
        for (k, (x, y, w)) in (0u32..).zip(vs.into_iter()) {
            let c = color(k.wrapping_mul(5).wrapping_add(1));
            writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x * w.abs()), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(&mut s, "draw {top} 2 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
        writeln!(&mut s, "end");
    }
    // points and lines with a coordinate not finite (NaN x, infinite x or w)
    for (name, top, bad) in [("point-nan", 1u32, f32::NAN), ("line-nan", 2, f32::NAN), ("line-inf", 2, f32::INFINITY), ("point-infx", 1, f32::INFINITY), ("line-infx", 2, f32::INFINITY), ("line-neginfw", 2, f32::NEG_INFINITY), ("line-inf-out", 2, f32::INFINITY)] {
        begin(&mut s, dev, &format!("probe-clip-w-{name}"), 16, 16, 2, Prog::Ps);
        for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.25, -0.75, 1.0), (0.5, 0.25, 1.0)].into_iter()) {
            let c = color(k.wrapping_mul(5).wrapping_add(1));
            let (x, w) = match (k, name) {
                (1, "line-inf" | "line-neginfw" | "line-inf-out") => (x, bad),
                (0, "line-inf-out") => (-1.5, w),
                (1, _) => (bad, w),
                _ => (x, w),
            };
            writeln!(&mut s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(&mut s, "draw {top} 3 32");
        read_rt(&mut s, 0, crate::dxgi!(2), 16, 16);
        writeln!(&mut s, "end");
    }
    // the last case ends the executor: on Direct3D 11 a patch list, on 10 an integer view sampled
    if !d3d11 {
        writeln!(&mut s, "case probe-filter-r8g8b8a8-uint\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(&mut s, "tex 1 2 4 4 1 1 30 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x15 3 3 3");
        writeln!(&mut s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(&mut s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32), (1.0, 1.0, 0.9, -0.2), (-1.0, -1.0, -0.2, 0.9), (1.0, -1.0, 0.9, 0.9)] {
            writeln!(&mut s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(&mut s, "draw 5 4 32\nread 0 0 256 16 1\nend");
        return s;
    }
    begin(&mut s, dev, "probe-topology-33", 16, 16, 28, Prog::Ps);
    for (k, f) in (0u32..).zip([0.0f32, 1.0, 2.0]) {
        vertex(&mut s, 2.0 + 5.0 * f, 3.0 + 4.0 * f, 0.5, color(k), 16.0, 16.0);
    }
    writeln!(s, "draw 33 3 32");
    read_rt(&mut s, 0, crate::dxgi!(28), 16, 16);
    writeln!(s, "end");
    s
}

/// one case per draw the other cases do not make: what Windows does there is to be measured
fn probes(s: &mut String, dev: &str) {
    // (one color, unless `vary`)
    let tri3 = |s: &mut String, z: [f32; 3], w: [f32; 3], vary: bool| {
        for (i, p) in [(-1.0f32, 1.0f32), (3.0, 1.0), (-1.0, -3.0)].iter().enumerate() {
            let c = color(if vary { i as u32 } else { 0 });
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(p.0 * w[i]), fb(p.1 * w[i]), fb(z[i] * w[i]), fb(w[i]), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
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
        begin(s, dev, &format!("probe-{name}"), 16, 16, 28, Prog::Ps);
        if !extra.is_empty() {
            writeln!(s, "{extra}");
        }
        tri(s, z, w);
        writeln!(s, "{draw}");
        read_rt(s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // colors that vary over the triangle (the interpolation)
    begin(s, dev, "probe-interp", 16, 16, 2, Prog::Ps);
    tri3(s, [0.1, 0.5, 0.9], [1.0, 2.0, 0.5], true);
    writeln!(s, "draw 4 3 32");
    read_rt(s, 0, crate::dxgi!(2), 16, 16);
    writeln!(s, "end");
    // no shaders and layout; no viewport; a multisampled target
    writeln!(s, "case probe-incomplete\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0\nviewport 0 0 {} {} 0 {}", fb(16.0), fb(16.0), fb(1.0));
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nread 0 0 64 16 1\nend");
    writeln!(s, "case probe-noviewport\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)));
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nread 0 0 64 16 1\nend");
    writeln!(s, "case probe-rt-ms\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    tri(s, half, one);
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // to fit what D3DRASTER.RTKIND leaves open: where the samples are, by the share of a
    // pixel's samples that triangles' slanted edges cover (the resolve's average)
    for n in [2u32, 4, 8] {
        writeln!(s, "case probe-rt-ms{n}-edges\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 {n} 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        for t in [[(0.0f32, 0.0f32), (16.0, 1.0), (0.0, 3.0)], [(0.0, 4.0), (16.0, 4.0), (8.0, 8.0)], [(3.0, 9.0), (4.0, 16.0), (0.5, 16.0)], [(9.0, 9.0), (15.5, 10.25), (11.0, 15.75)]] {
            for (x, y) in t {
                vertex(s, x, y, 0.5, [1.0; 4], 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 12 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    }
    // the resolve's mean of samples of other values, overlapping colored triangles, in a
    // UNORM and a float target
    for (fmt, pitch) in [(28u32, 64u32), (2, 256), (10, 128), (24, 64), (29, 64), (87, 64), (85, 32), (26, 64), (31, 64), (16, 128), (35, 64), (41, 64), (13, 128), (86, 32), (115, 32)] {
        writeln!(s, "case probe-rt-ms4-colors-{fmt}\n{dev}\ntex 0 2 16 16 1 1 {fmt} 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 {fmt} 0 0x20 0 0 1 0");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        for (k, t) in (0u32..).zip([[(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)], [(3.0, 1.0), (15.5, 15.0), (0.0, 9.75)], [(8.25, 0.0), (12.0, 16.0), (2.0, 12.5)]].iter()) {
            for (i, &(x, y)) in (0u32..).zip(t.iter()) {
                vertex(s, x, y, 0.5, color(k.wrapping_mul(3).wrapping_add(i)), 16.0, 16.0);
            }
        }
        writeln!(s, "draw 4 9 32\nresolve 1 0 0 0 {fmt}\nread 1 0 {pitch} 16 1\nend");
    }
    // what D3DRASTER.RTKIND and CLIPW leave open: lines on a multisampled target, a resolve
    // of differing samples of 16-bit UNORM, a vertex behind the eye under colors that differ
    writeln!(s, "case probe-rt-ms4-lines\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 2 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    writeln!(s, "case probe-rt-ms4-points\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    writeln!(s, "case probe-rt-ms4-unorm16\n{dev}\ntex 0 2 16 16 1 1 11 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 11 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i.wrapping_add(5)), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 11\nread 1 0 128 16 1\nend");
    // what D3DRASTER.RTKIND leaves open: depth per sample, a viewport off whole pixels, lines
    // with the multisample state, points through pspos and over differing samples on a
    // multisampled target; a depth view alone
    let ms4 = |s: &mut String, name: &str, rast: &str, vp: (f32, f32), ps: Prog| {
        writeln!(s, "case probe-rt-ms4-{name}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast {rast}\nviewport {} {} {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(ps)), fb(vp.0), fb(vp.1), fb(16.0), fb(16.0), fb(1.0));
    };
    ms4(s, "depth", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), Prog::Ps);
    writeln!(s, "tex 2 2 16 16 1 1 40 0 0x40 0 0 4 0\ndsv 2 2 nodesc\ncleards 2 3 {} 0\ntarget 0 2\ndepthst 1 1 2", fb(1.0));
    for (k, (t, z)) in (0u32..).zip([([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)], 0.25f32), ([(3.0, 1.0), (15.5, 15.0), (0.0, 9.75)], 0.75), ([(8.25, 0.0), (12.0, 16.0), (2.0, 12.5)], 0.5)].into_iter()) {
        for (i, (x, y)) in (0u32..).zip(t.into_iter()) {
            vertex(s, x, y, z, color(k.wrapping_mul(3).wrapping_add(i)), 16.0, 16.0);
        }
    }
    writeln!(s, "draw 4 9 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    ms4(s, "vphalf", "3 1 0 0 0 0 1 0 0 0", (0.5, 0.25), Prog::Ps);
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    for (name, rast) in [("linesms", "3 1 0 0 0 0 1 0 1 0"), ("linesaa", "3 1 0 0 0 0 1 0 0 1"), ("linesboth", "3 1 0 0 0 0 1 0 1 1")] {
        ms4(s, name, rast, (0.0, 0.0), Prog::Ps);
        for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter()) {
            vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
        }
        writeln!(s, "draw 2 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    }
    ms4(s, "points-pspos", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), Prog::Pspos);
    for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    ms4(s, "points-over", "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), Prog::Ps);
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i.wrapping_add(5)), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32");
    for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 1.25), (5.5, 0.75)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 1 4 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // lines and wireframe over samples that differ
    for (name, rast, top) in [("lines-over", "3 1 0 0 0 0 1 0 0 0", 2u32), ("wire-over", "2 1 0 0 0 0 1 0 0 0", 4)] {
        ms4(s, name, "3 1 0 0 0 0 1 0 0 0", (0.0, 0.0), Prog::Ps);
        for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
            vertex(s, x, y, 0.5, color(i.wrapping_add(5)), 16.0, 16.0);
        }
        writeln!(s, "draw 4 3 32\nrast {rast}");
        for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25), (12.5, 0.75), (0.5, 9.0), (15.0, 3.0)].into_iter()) {
            vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
        }
        writeln!(s, "draw {top} 6 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    }
    // what D3DRASTER.RTKIND leaves open: a target in an array's second slice, a multisampled
    // target of quality 1
    writeln!(s, "case probe-rt-array\n{dev}\ntex 0 2 16 16 2 1 28 0 0x20 0 0 1 0\nrtv 0 0 28 5 0 1 1\nclear 0 0 0 0 0\ntarget 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nread 0 1 64 16 1\nread 0 0 64 16 1\nend");
    // what D3DRASTER.RTKIND leaves open: a 3D texture's view as the target (2 slices)
    // (and from its second slice)
    // (and a 1D texture's view, 16 x 1)
    for (name, rtv) in [("rt-3d", "rtv 0 0 nodesc"), ("rt-3d-slice1", "rtv 0 0 28 8 0 1 1"), ("rt-1d", "rtv 0 0 nodesc")] {
        let tex = if name == "rt-1d" { "tex 0 1 16 1 1 1 28 0 0x20 0 0 1 0" } else { "tex 0 3 16 16 2 1 28 0 0x20 0 0 1 0" };
        writeln!(s, "case probe-{name}\n{dev}\n{tex}\n{rtv}\nclear 0 0 0 0 0\ntarget 0");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
            vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
        }
        let read = if name == "rt-1d" { "read 0 0 64 1 1" } else { "read 0 0 64 16 2" };
        writeln!(s, "draw 4 3 32\n{read}\nend");
    }
    writeln!(s, "case probe-rt-ms4-q1\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 1\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // copies of differing samples: the whole resource, then resolved
    writeln!(s, "case probe-rt-ms4-copy\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0\ntex 2 2 16 16 1 1 28 0 0x20 0 0 4 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\ncopy 2 0\nresolve 1 0 2 0 28\nread 1 0 64 16 1\nend");
    // what D3DRASTER.RTKIND leaves open: a whole-subresource region copy of differing samples,
    // a depth view of one sample under a 4-sample target, a region copy with a box, an update
    for (name, setup, after) in [("copysub", "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0", "copysub 2 0 0 0 0 0 0\nresolve 1 0 2 0 28"), ("depth1", "tex 2 2 16 16 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 0x3f000000 0\ntarget 0 2\ndepthst 1 1 2", "resolve 1 0 0 0 28"), ("depth1-prev", "rtv 1 1 nodesc\ntarget 1\ntex 2 2 16 16 1 1 40 0 0x40 0 0 1 0\ndsv 2 2 nodesc\ncleards 2 3 0x3f000000 0\ntarget 0 2\ndepthst 1 1 2", "read 1 0 64 16 1\nresolve 1 0 0 0 28"), ("copysub-box", "tex 2 2 16 16 1 1 28 0 0x20 0 0 4 0", "copysub 2 0 0 0 0 0 0 0 0 0 8 8 1\nresolve 1 0 2 0 28"), ("update", "", "update 0 0 7 64 0\nresolve 1 0 0 0 28")] {
        writeln!(s, "case probe-rt-ms4-{name}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
        if !setup.is_empty() {
            writeln!(s, "{setup}");
        }
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        for (i, (x, y)) in (0u32..).zip([(0.0f32, 0.0f32), (16.0, 2.5), (1.0, 14.0)].into_iter()) {
            vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
        }
        writeln!(s, "draw 4 3 32\n{after}\nread 1 0 64 16 1\nend");
    }
    writeln!(s, "case probe-depth-alone\n{dev}\ntex 1 2 8 8 1 1 40 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget - 1\ndepthst 1 1 8", fb(1.0));
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(8.0), fb(8.0), fb(1.0));
    for (x, y) in [(0.0f32, 0.0f32), (8.0, 1.5), (0.5, 7.0)] {
        vertex(s, x, y, 0.375, [1.0; 4], 8.0, 8.0);
    }
    writeln!(s, "draw 4 3 32\nread 1 0 32 8 1\nend");
    // what D3DRASTER.RTKIND leaves open: wireframe on a multisampled target
    writeln!(s, "case probe-rt-ms4-wire\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
    writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 2 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
    for (i, (x, y)) in (0u32..).zip([(1.25f32, 2.5f32), (14.5, 11.75), (3.0, 15.25)].into_iter()) {
        vertex(s, x, y, 0.5, color(i), 16.0, 16.0);
    }
    writeln!(s, "draw 4 3 32\nresolve 1 0 0 0 28\nread 1 0 64 16 1\nend");
    // an input layout against a refused program
    let mut broken = shader(Prog::Vs);
    broken[5] ^= 1;
    writeln!(s, "case probe-layout-refused\n{dev}\nshader 0 vs {}\nlayout 0 2 2\nend", hex(&broken));
    // an input layout against a pixel program given as a vertex one
    writeln!(s, "case probe-layout-otherkind\n{dev}\nshader 0 vs {}\nlayout 0 2 2\nlayout 0 2\nend", hex(&shader(Prog::Ps)));
    // ld past the texture, no sampler, samples between texel centers, blending an integer target
    let ldtex = "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init 91 16 0\nsrv 1 1 nodesc\ntsrv 1";
    let quad = |s: &mut String, uv: (f32, f32)| {
        for (x, y, u, v) in [(-1.0f32, 1.0f32, -uv.0, -uv.0), (1.0, 1.0, uv.1, -uv.0), (-1.0, -1.0, -uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(s, "draw 5 4 32");
    };
    for (name, ps, rt, extra, uv) in [("ld-out", Prog::Psld, 2u32, "", (0.0f32, 1.0f32)), ("sample-nosampler", Prog::Pssample, 2, "", (0.0, 1.0)), ("sample-between", Prog::Pssample, 2, "samp 0x15 3 3 3", (0.1, 1.3)), ("blend-uint", Prog::Psldu, 3, "blend1 1 2 2 1 2 2 1 0xf", (0.0, 1.0))] {
        let tex = if rt == 3 { ldtex.replace(" 28 0 8", " 30 0 8") } else { ldtex.to_string() };
        writeln!(s, "case probe-{name}\n{dev}\ntex 0 2 8 8 1 1 {rt} 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0\n{tex}");
        if !extra.is_empty() {
            writeln!(s, "{extra}");
        }
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nviewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", hex(&shader(Prog::Vs)), hex(&shader(ps)), fb(8.0), fb(8.0), fb(1.0));
        quad(s, uv);
        writeln!(s, "read 0 0 128 8 1\nend");
    }
    // many of each, to fit: perspective interpolation into a float target, points, lines,
    // wireframe, filtering between texel centers
    let mut r = XorShift32::new(0x9e0b_e5);
    let px = |r: &mut XorShift32, lo: i64, hi: i64| r.range(lo * 256, hi * 256) as f32 / 256.0;
    let unit = |r: &mut XorShift32| r.range(0, 1 << 16) as f32 / 65536.0;
    for i in 0..24 {
        begin(s, dev, &format!("probe-interp-{i}"), 16, 16, 2, Prog::Ps);
        for _ in 0..3 {
            let (x, y, z, w) = (px(&mut r, -4, 20), px(&mut r, -4, 20), unit(&mut r), 0.25 + 3.75 * unit(&mut r));
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = [unit(&mut r), unit(&mut r), unit(&mut r), unit(&mut r)];
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(s, "draw 4 3 32");
        read_rt(s, 0, crate::dxgi!(2), 16, 16);
        writeln!(s, "end");
    }
    // depths that vary over unclipped triangles: read through SV_Position (w = 1 and with w),
    // and tested against a 32-bit float and a 16-bit UNORM depth view
    let mut rz = XorShift32::new(0x2_9a7e);
    for i in 0..6u32 {
        let (ps, f) = if i < 4 { (Prog::Pspos, crate::dxgi!(2)) } else { (Prog::Ps, crate::dxgi!(28)) };
        begin(s, dev, &format!("probe-zplane-{i}"), 16, 16, f.id, ps);
        if i >= 4 {
            let df = if i == 4 { 40 } else { 55 };
            writeln!(s, "tex 1 2 16 16 1 1 {df} 0 0x40 0 0 1 0\ndsv 1 1 nodesc\ncleards 1 3 {} 0\ntarget 0 1\ndepthst 1 1 2", fb(0.5));
        }
        for _ in 0..3 {
            let (x, y, z) = (px(&mut rz, 1, 15), px(&mut rz, 1, 15), unit(&mut rz));
            let w = if i == 1 || i == 3 { 0.5 + 1.5 * unit(&mut rz) } else { 1.0 };
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = color(i * 3);
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(s, "draw 4 3 32");
        read_rt(s, 0, f, 16, 16);
        writeln!(s, "end");
    }
    // to separate the interpolation's parts: w = 1 inside the target (the plane equations'
    // arithmetic), inside with w (the perspective division), one vertex past one edge (the
    // clipping and its triangles), with w = 1 and with w
    let mut r2 = XorShift32::new(0x1e7e_4b);
    let tri_case = |s: &mut String, r: &mut XorShift32, name: String, past: Option<u32>, persp: bool| {
        begin(s, dev, &name, 24, 24, 2, Prog::Ps);
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
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(z * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(s, "draw 4 3 32");
        read_rt(s, 0, crate::dxgi!(2), 24, 24);
        writeln!(s, "end");
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
            begin(s, dev, &format!("probe-{name}-{i}"), 16, 16, 28, Prog::Ps);
            if fill != 3 {
                writeln!(s, "rast {fill} 1 0 0 0 0 1 0 0 0");
            }
            for k in 0..n {
                vertex(s, px(&mut r, -2, 18), px(&mut r, -2, 18), 0.5, color(i * 31 + k), 16.0, 16.0);
            }
            writeln!(s, "draw {top} {n} 32");
            read_rt(s, 0, crate::dxgi!(28), 16, 16);
            writeln!(s, "end");
        }
    }
    for i in 0..12u32 {
        let (filter, addr) = (*r.pick(&[0x15u32, 0x14, 0x11, 0x10, 0x5, 0x1, 0x4]), r.range(1, 5));
        let (u0, u1, v0, v1) = (px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0, px(&mut r, -2, 3) / 4.0);
        writeln!(s, "case probe-filter-{i}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(s, "tex 1 2 4 4 1 1 28 0 8 0 0 1 0 init {} 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} {addr} {addr} {addr}", 300 + i);
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, u0, v0), (1.0, 1.0, u1, v0), (-1.0, -1.0, u0, v1), (1.0, -1.0, u1, v1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // to fit what D3DRASTER.TOPOLOGY leaves open: line ends inside a pixel's diamond (16
    // short lines a case, each alone in a 4 x 4 cell, ending near the cell's pixel (2, 2) from
    // every direction), lines with w, the multisample and antialiased-line states, the
    // adjacency and patch topologies, points and lines through the other programs
    let mut r3 = XorShift32::new(0x11e_e7d);
    for i in 0..8u32 {
        begin(s, dev, &format!("probe-lineend-{i}"), 16, 16, 28, Prog::Ps);
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
        writeln!(s, "draw 2 32 32");
        read_rt(s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // lines that start or end on a corner of a pixel's diamond (left, right, top, bottom),
    // x-major and y-major in every direction, one line a 4 x 4 cell (probe-rt-ms4-lines-over
    // lit a pixel a line only touches)
    let combos: Vec<(usize, (f32, f32), bool)> = (0..4).flat_map(|v| [(1.0f32, 0.375f32), (1.0, -0.375), (-1.0, 0.375), (-1.0, -0.375), (0.375, 1.0), (-0.375, 1.0), (0.375, -1.0), (-0.375, -1.0)].into_iter().flat_map(move |d| [(v, d, true), (v, d, false)])).collect();
    for (ms, (i, part)) in [false, true].into_iter().flat_map(|ms| combos.chunks(16).enumerate().map(move |p| (ms, p))) {
        if ms {
            // (and on a 4-sample target, resolved)
            writeln!(s, "case probe-rt-ms4-linevert-{i}\n{dev}\ntex 0 2 16 16 1 1 28 0 0x20 0 0 4 0\nrtv 0 0 nodesc\nclear 0 0 0 0 0\ntarget 0\ntex 1 2 16 16 1 1 28 0 0x20 0 0 1 0");
            writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2\nrast 3 1 0 0 0 0 1 0 0 0\nviewport 0 0 {} {} 0 {}", hex(&shader(Prog::Vs)), hex(&shader(Prog::Ps)), fb(16.0), fb(16.0), fb(1.0));
        } else {
            begin(s, dev, &format!("probe-linevert-{i}"), 16, 16, 28, Prog::Ps);
        }
        for (cell, &(v, (dx, dy), starts)) in (0u32..).zip(part.iter()) {
            let (cx, cy) = ((cell % 4 * 4) as f32 + 2.5, (cell / 4 * 4) as f32 + 2.5);
            let (vx, vy) = [(cx - 0.5, cy), (cx + 0.5, cy), (cx, cy - 0.5), (cx, cy + 0.5)][v];
            let (ax, ay, bx, by) = if starts { (vx, vy, vx + dx * 1.5, vy + dy * 1.5) } else { (vx - dx * 1.5, vy - dy * 1.5, vx, vy) };
            vertex(s, ax, ay, 0.5, color(i as u32 * 37 + 2 * cell), 16.0, 16.0);
            vertex(s, bx, by, 0.5, color(i as u32 * 37 + 2 * cell + 1), 16.0, 16.0);
        }
        writeln!(s, "draw 2 32 32");
        if ms {
            writeln!(s, "resolve 1 0 0 0 28\nread 1 0 64 16 1");
        } else {
            read_rt(s, 0, crate::dxgi!(28), 16, 16);
        }
        writeln!(s, "end");
    }
    // x-major lines from a corner shared by two pixels above each other (a top corner going
    // up, a bottom one going down, rightward and leftward), of five slopes and three lengths:
    // probe-rt-ms4-lines-over lit the lower pixel where the 1.5-pixel lines of probe-linevert-*
    // light the upper one; two lines a case, in the upper and lower halves
    let long: Vec<(f32, f32, f32)> = [1.5f32, 4.0, 14.0].into_iter().flat_map(|len| [0.125f32, 0.25, 0.414, 0.5, 0.75].into_iter().flat_map(move |k| [(1.0f32, -1.0f32), (1.0, 1.0), (-1.0, -1.0), (-1.0, 1.0)].into_iter().map(move |(sx, sy)| (len * sx, len * k * sy, sy)))).collect();
    for (i, pair) in long.chunks(2).enumerate() {
        begin(s, dev, &format!("probe-linecorner-{i}"), 16, 16, 28, Prog::Ps);
        for (half, &(dx, dy, sy)) in (0u32..).zip(pair.iter()) {
            // (the start on the corner between rows 3 and 4, or 11 and 12, at column 1 or 14's
            // center)
            let x = if dx > 0.0 { 1.5 } else { 14.5 };
            let y = if half == 0 { 4.0 } else { 12.0 } + if sy < 0.0 { 3.0 } else { -3.0 } * f32::from(u8::from(dy.abs() > 3.0));
            vertex(s, x, y, 0.5, color(i as u32 * 7 + 2 * half), 16.0, 16.0);
            vertex(s, x + dx, y + dy, 0.5, color(i as u32 * 7 + 2 * half + 1), 16.0, 16.0);
        }
        writeln!(s, "draw 2 4 32");
        read_rt(s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    // x-major lines from a pixel center through diamond corners at every other column (slope
    // 1/2) or every fourth (1/4), of major lengths 5 to 15, rightward and leftward, down and up:
    // probe-linecorner-22 lit the pixel below or above such a corner by its distance along the
    // line
    for (k, (slope, sx)) in (0u32..).zip([(0.5f32, 1.0f32), (0.5, -1.0), (-0.5, 1.0), (0.25, 1.0), (0.25, -1.0), (-0.25, 1.0)].into_iter()) {
        for len in 5..=15u8 {
            let len = f32::from(len);
            begin(s, dev, &format!("probe-linetie-{k}-{len}"), 16, 16, 28, Prog::Ps);
            let x = if sx > 0.0 { 0.5 } else { 15.5 };
            let y = if slope > 0.0 { 0.5 } else { 15.5 };
            vertex(s, x, y, 0.5, color(k * 3), 16.0, 16.0);
            vertex(s, x + sx * len, y + slope * len, 0.5, color(k * 3 + 1), 16.0, 16.0);
            writeln!(s, "draw 2 2 32");
            read_rt(s, 0, crate::dxgi!(28), 16, 16);
            writeln!(s, "end");
        }
    }
    for i in 0..4u32 {
        begin(s, dev, &format!("probe-linesw-{i}"), 16, 16, 2, Prog::Ps);
        for _ in 0..8 {
            let (x, y, w) = (px(&mut r3, 0, 16), px(&mut r3, 0, 16), 0.25 + 3.75 * unit(&mut r3));
            let (nx, ny) = (x / 16.0 * 2.0 - 1.0, 1.0 - y / 16.0 * 2.0);
            let c = [unit(&mut r3), unit(&mut r3), unit(&mut r3), unit(&mut r3)];
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
        }
        writeln!(s, "draw 2 8 32");
        read_rt(s, 0, crate::dxgi!(2), 16, 16);
        writeln!(s, "end");
    }
    let some = |s: &mut String, r: &mut XorShift32, n: u32| {
        for k in 0..n {
            vertex(s, px(r, 0, 16), px(r, 0, 16), 0.5, color(k * 7 + 3), 16.0, 16.0);
        }
    };
    for (name, rast) in [("ms", "rast 3 1 0 0 0 0 1 0 1 0"), ("aa", "rast 3 1 0 0 0 0 1 0 0 1"), ("msaa", "rast 3 1 0 0 0 0 1 0 1 1")] {
        begin(s, dev, &format!("probe-lines-{name}"), 16, 16, 28, Prog::Ps);
        writeln!(s, "{rast}");
        some(s, &mut r3, 8);
        writeln!(s, "draw 2 8 32");
        read_rt(s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
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
            begin(s, dev, &format!("probe-lines{name}-{}", if vert { "col" } else { "row" }), 16, 16, 2, Prog::Ps);
            writeln!(s, "{rast}");
            for k in 0..8u32 {
                let off = (2 * k) as f32 + 0.5 + (176 + k) as f32 / 256.0;
                let (e0, e1) = (1.5 + (k as f32 - 4.0) / 256.0, 14.5 + (3.0 - k as f32) / 256.0);
                let (a, b) = if vert { ((off, e0), (off, e1)) } else { ((e0, off), (e1, off)) };
                vertex(s, a.0, a.1, 0.5, color(2 * k), 16.0, 16.0);
                vertex(s, b.0, b.1, 0.5, color(2 * k + 1), 16.0, 16.0);
            }
            writeln!(s, "draw 2 16 32");
            read_rt(s, 0, crate::dxgi!(2), 16, 16);
            writeln!(s, "end");
        }
        for i in 0..4u32 {
            begin(s, dev, &format!("probe-lines{name}-{i}"), 16, 16, 2, Prog::Ps);
            writeln!(s, "{rast}");
            for cell in 0..4u32 {
                let (ox, oy) = ((cell % 2 * 8) as f32, (cell / 2 * 8) as f32);
                for _ in 0..2 {
                    let (x, y) = (ox + 1.75 + 4.5 * unit(&mut r4), oy + 1.75 + 4.5 * unit(&mut r4));
                    let grid = |v: f32| (v * 256.0).round() / 256.0;
                    let w = if cell % 2 == 1 { 0.25 + 3.75 * unit(&mut r4) } else { 1.0 };
                    let (nx, ny) = (grid(x) / 16.0 * 2.0 - 1.0, 1.0 - grid(y) / 16.0 * 2.0);
                    let c = [unit(&mut r4), unit(&mut r4), unit(&mut r4), unit(&mut r4)];
                    writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(nx * w), fb(ny * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
                }
            }
            writeln!(s, "draw 2 8 32");
            read_rt(s, 0, crate::dxgi!(2), 16, 16);
            writeln!(s, "end");
        }
    }
    for top in [10u32, 11, 12, 13] {
        begin(s, dev, &format!("probe-topology-{top}"), 16, 16, 28, Prog::Ps);
        some(s, &mut r3, 12);
        writeln!(s, "draw {top} 12 32");
        read_rt(s, 0, crate::dxgi!(28), 16, 16);
        writeln!(s, "end");
    }
    for (top, n) in [(1u32, 12u32), (2, 8)] {
        for ps in [Prog::Psflat, Prog::Pspos, Prog::Psprim] {
            begin(s, dev, &format!("probe-top{top}-{}", ps.name()), 16, 16, 2, ps);
            some(s, &mut r3, n);
            writeln!(s, "draw {top} {n} 32");
            read_rt(s, 0, crate::dxgi!(2), 16, 16);
            writeln!(s, "end");
        }
    }
    // what D3DRASTER.CLIPW, CLIPZ and FILTER leave open: a vertex behind the eye with the
    // triangle keeping an area, the depth of a triangle crossing the depth range as
    // SV_Position, minification, linear filtering of 16-bit UNORM and float texels
    begin(s, dev, "probe-clip-w2", 16, 16, 28, Prog::Ps);
    for (x, y, w) in [(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)] {
        let c = color(9);
        writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x * w), fb(y * w), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
    }
    writeln!(s, "draw 4 3 32");
    read_rt(s, 0, crate::dxgi!(28), 16, 16);
    writeln!(s, "end");
    begin(s, dev, "probe-clip-w3", 16, 16, 2, Prog::Ps);
    for (k, (x, y, w)) in (0u32..).zip([(-0.5f32, 0.5f32, 1.0f32), (0.5, 0.25, 1.0), (0.25, -0.75, -0.5)].into_iter()) {
        let c = color(k * 5 + 1);
        writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x * w.abs()), fb(y * w.abs()), fb(0.5 * w), fb(w), fb(c[0]), fb(c[1]), fb(c[2]), fb(c[3]));
    }
    writeln!(s, "draw 4 3 32");
    read_rt(s, 0, crate::dxgi!(2), 16, 16);
    writeln!(s, "end");
    // a line crossing the depth range, depth clipping on: read through SV_Position
    begin(s, dev, "probe-line-clip-z", 16, 16, 2, Prog::Pspos);
    vertex(s, 1.5, 2.25, -0.5, [1.0; 4], 16.0, 16.0);
    vertex(s, 14.25, 13.5, 1.5, [1.0; 4], 16.0, 16.0);
    writeln!(s, "draw 2 2 32");
    read_rt(s, 0, crate::dxgi!(2), 16, 16);
    writeln!(s, "end");
    // depths that vary along unclipped lines, x-major and y-major, w = 1 and not
    begin(s, dev, "probe-line-z", 16, 16, 2, Prog::Pspos);
    vertex(s, 0.75, 1.25, 0.125, [1.0; 4], 16.0, 16.0);
    vertex(s, 15.25, 6.5, 0.875, [1.0; 4], 16.0, 16.0);
    vertex(s, 3.25, 7.5, 0.9, [1.0; 4], 16.0, 16.0);
    vertex(s, 9.75, 15.5, 0.05, [1.0; 4], 16.0, 16.0);
    writeln!(s, "draw 2 4 32");
    read_rt(s, 0, crate::dxgi!(2), 16, 16);
    writeln!(s, "end");
    begin(s, dev, "probe-clip-z-pos", 16, 16, 2, Prog::Pspos);
    tri(s, [-0.5, 0.5, 1.5], one);
    writeln!(s, "draw 4 3 32");
    read_rt(s, 0, crate::dxgi!(2), 16, 16);
    writeln!(s, "end");
    for (name, filter, fmt, uv) in [("min", 0x10u32, crate::dxgi!(28), (-2.0f32, 3.0f32)), ("min-point", 0x04, crate::dxgi!(28), (-1.5, 2.5)), ("unorm16", 0x15, crate::dxgi!(11), (-0.2, 0.9)), ("float", 0x15, crate::dxgi!(2), (-0.2, 0.9)), ("rho-1024", 0x10, crate::dxgi!(28), (-1.5, 2.5 + 4.0 / 1024.0)), ("rho-64", 0x10, crate::dxgi!(28), (-1.5, 2.5 + 4.0 / 64.0)), ("rho-10", 0x10, crate::dxgi!(28), (-1.5, 2.9)), ("rho-under", 0x10, crate::dxgi!(28), (-1.5, 2.5 - 4.0 / 1024.0))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        let (pitch, fmt) = (fmt.row_bytes(4), fmt.id);
        writeln!(s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // what D3DRASTER.FILTER leaves open: anisotropic filtering, linear filtering of sRGB and
    // SNORM texels, minifying a texture with mips (two texels a pixel), between two levels
    // (spans of 1.0625, 1.25, sqrt 2, 1.75, 2.5 and 3 texels; float, 16-bit UNORM and point texels)
    for (name, filter, fmt, mips, uv) in [("aniso", 0x55u32, crate::dxgi!(28), 1u32, (-0.2f32, 0.9f32)), ("srgb", 0x15, crate::dxgi!(29), 1, (-0.2, 0.9)), ("snorm", 0x15, crate::dxgi!(31), 1, (-0.2, 0.9)), ("mips", 0x15, crate::dxgi!(28), 3, (-3.5, 4.5)), ("trilinear", 0x15, crate::dxgi!(28), 3, (-1.5, 4.5)), ("aniso-min", 0x55, crate::dxgi!(28), 1, (-3.5, 4.5)), ("r10", 0x15, crate::dxgi!(24), 1, (-0.2, 0.9)), ("mips-point", 0x14, crate::dxgi!(28), 3, (-1.5, 4.5)), ("trilinear-1", 0x15, crate::dxgi!(28), 3, (-1.0, 3.25)), ("trilinear-2", 0x15, crate::dxgi!(28), 3, (-1.0, 4.0)), ("trilinear-3", 0x15, crate::dxgi!(28), 3, (-1.0, 4.656854)), ("trilinear-4", 0x15, crate::dxgi!(28), 3, (-1.0, 6.0)), ("trilinear-5", 0x15, crate::dxgi!(28), 3, (-1.0, 9.0)), ("trilinear-6", 0x15, crate::dxgi!(28), 3, (-1.0, 11.0)), ("trilinear-float", 0x15, crate::dxgi!(2), 3, (-1.5, 4.5)), ("trilinear-point", 0x1, crate::dxgi!(28), 3, (-1.5, 4.5)), ("trilinear-unorm16", 0x15, crate::dxgi!(11), 3, (-1.0, 6.0)), ("r11g11b10", 0x15, crate::dxgi!(26), 1, (-0.2, 0.9)), ("rgb9e5", 0x15, crate::dxgi!(67), 1, (-0.2, 0.9)), ("b5g6r5", 0x15, crate::dxgi!(85), 1, (-0.2, 0.9)), ("b5g5r5a1", 0x15, crate::dxgi!(86), 1, (-0.2, 0.9)), ("b4g4r4a4", 0x15, crate::dxgi!(115), 1, (-0.2, 0.9)), ("trilinear-point-2", 0x1, crate::dxgi!(28), 3, (-1.0, 6.0)), ("trilinear-point-float", 0x1, crate::dxgi!(2), 3, (-1.0, 9.0)), ("bc1", 0x15, crate::dxgi!(71), 1, (-0.2, 0.9)), ("bc3", 0x15, crate::dxgi!(77), 1, (-0.2, 0.9)), ("bc7", 0x15, crate::dxgi!(98), 1, (-0.2, 0.9)), ("r8g8", 0x15, crate::dxgi!(49), 1, (-0.2, 0.9)), ("a8", 0x15, crate::dxgi!(65), 1, (-0.2, 0.9)), ("r8", 0x15, crate::dxgi!(61), 1, (-0.2, 0.9)), ("bc2", 0x15, crate::dxgi!(74), 1, (-0.2, 0.9)), ("bc4", 0x15, crate::dxgi!(80), 1, (-0.2, 0.9)), ("bc4s", 0x15, crate::dxgi!(81), 1, (-0.2, 0.9)), ("bc5", 0x15, crate::dxgi!(83), 1, (-0.2, 0.9)), ("bc5s", 0x15, crate::dxgi!(84), 1, (-0.2, 0.9)), ("r24x8", 0x15, crate::dxgi!(46), 1, (-0.2, 0.9)), ("r32x8x24", 0x15, crate::dxgi!(21), 1, (-0.2, 0.9)), ("bc6h", 0x15, crate::dxgi!(95), 1, (-0.2, 0.9)), ("bc1-srgb", 0x15, crate::dxgi!(72), 1, (-0.2, 0.9)), ("b8g8r8a8-srgb", 0x15, crate::dxgi!(91), 1, (-0.2, 0.9)), ("r8g8b8a8-mips", 0x15, crate::dxgi!(28), 3, (-1.0, 9.0))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        let (pitch, fmt) = (fmt.row_bytes(4), fmt.id);
        writeln!(s, "tex 1 2 4 4 1 {mips} {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // sample_c: a comparison filter (point, linear) and every comparison function, against a
    // reference that runs across the triangle, of a 16-bit UNORM and a 32-bit float texture
    for (filter, func, fmt) in [0x80u32, 0x95].into_iter().flat_map(|f| (1..=8u32).map(move |c| (f, c, crate::dxgi!(56)))).chain([(0x95, 2, crate::dxgi!(41)), (0x80, 4, crate::dxgi!(41))]) {
        let (pitch, fmt) = (fmt.row_bytes(4), fmt.id);
        writeln!(s, "case probe-sample-cmp-{filter:x}-{func}-{fmt}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(s, "tex 1 2 4 4 1 1 {fmt} 0 8 0 0 1 0 init 400 {pitch} 0\nsrv 1 1 nodesc\ntsrv 1\nsamp {filter:#x} 3 3 3 {func}");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssamplec)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v, r) in [(-1.0f32, 1.0f32, -0.2f32, -0.2f32, 0.0f32), (1.0, 1.0, 0.9, -0.2, 1.0), (-1.0, -1.0, -0.2, 0.9, 0.0), (1.0, -1.0, 0.9, 0.9, 1.0)] {
            writeln!(s, "vtx {} {} {} {} {} {} {} {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(r), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // what D3DRASTER.FILTER leaves open: a view of its own range of levels (levels 1 and 2 of
    // three, minifying and magnifying)
    // (and what D3DRASTER.FILTER leaves open: a 2D array of 2 slices with mips, its whole view)
    for (name, uv) in [("view-mips", (-1.5f32, 4.5f32)), ("view-mips-mag", (-0.2, 0.9)), ("array-mips", (-1.5, 4.5)), ("cube-mips", (-1.5, 4.5)), ("array1-mips", (-1.5, 4.5))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        let (tex, srv) = match name {
            "array-mips" => ("tex 1 2 4 4 2 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 nodesc"),
            // (its view from slice 1)
            "array1-mips" => ("tex 1 2 4 4 2 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 28 5 0 3 1 1"),
            // (a cube of 6 faces with mips, its cube view)
            "cube-mips" => ("tex 1 2 4 4 6 3 28 0 8 0 4 1 0 init 400 16 0", "srv 1 1 nodesc"),
            _ => ("tex 1 2 4 4 1 3 28 0 8 0 0 1 0 init 400 16 0", "srv 1 1 28 4 1 2"),
        };
        writeln!(s, "{tex}\n{srv}\ntsrv 1\nsamp 0x15 3 3 3");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, u, v) in [(-1.0f32, 1.0f32, uv.0, uv.0), (1.0, 1.0, uv.1, uv.0), (-1.0, -1.0, uv.0, uv.1), (1.0, -1.0, uv.1, uv.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(u), fb(v), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // what D3DRASTER.FILTER leaves open: anisotropic filtering of a footprint longer one way
    // than the other (2 to 1, 4 to 1, 16 to 1), and over a texture with mips
    for (name, mips, u, v) in [("aniso-2", 1u32, (-3.5f32, 4.5f32), (-1.5f32, 2.5f32)), ("aniso-4", 1, (-3.5, 4.5), (-0.5, 1.5)), ("aniso-16", 1, (-7.5, 8.5), (-0.5, 1.5)), ("aniso-mips", 3, (-1.5, 4.5), (-1.5, 4.5)), ("aniso-mips-2", 3, (-3.5, 4.5), (-1.5, 2.5))] {
        writeln!(s, "case probe-filter-{name}\n{dev}\ntex 0 2 16 16 1 1 2 0 0x20 0 0 1 0\nrtv 0 0 nodesc\ntarget 0");
        writeln!(s, "tex 1 2 4 4 1 {mips} 28 0 8 0 0 1 0 init 400 16 0\nsrv 1 1 nodesc\ntsrv 1\nsamp 0x55 3 3 3");
        writeln!(s, "shader 0 vs {}\nshader 1 ps {}\nlayout 0 2 2", hex(&shader(Prog::Vs)), hex(&shader(Prog::Pssample)));
        writeln!(s, "viewport 0 0 {} {} 0 {}\nrast 3 1 0 0 0 0 1 0 0 0", fb(16.0), fb(16.0), fb(1.0));
        for (x, y, a, b) in [(-1.0f32, 1.0f32, u.0, v.0), (1.0, 1.0, u.1, v.0), (-1.0, -1.0, u.0, v.1), (1.0, -1.0, u.1, v.1)] {
            writeln!(s, "vtx {} {} {} {} {} {} 0 {}", fb(x), fb(y), fb(0.5), fb(1.0), fb(a), fb(b), fb(1.0));
        }
        writeln!(s, "draw 5 4 32\nread 0 0 256 16 1\nend");
    }
    // a constant buffer bound (no program reads it)
    begin(s, dev, "probe-cbuf", 16, 16, 28, Prog::Ps);
    writeln!(s, "buf 2 16 0 4 0 0 0\ncbuf 2");
    tri(s, half, one);
    writeln!(s, "draw 4 3 32");
    read_rt(s, 0, crate::dxgi!(28), 16, 16);
    writeln!(s, "end");
}
