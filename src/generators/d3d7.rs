//! The d3d7 suite's case generator: Direct3D 1, 2, 3 and 7 through DirectDraw.
use super::*;

pub const DEVS: &[&str] = &["rgb", "hal", "tnl", "ref", "mmx", "ramp", "null"];
pub const VERSIONS: &[u32] = &[1, 2, 3, 7];

/// a triangle line: color(s) and three vertices in 16ths of a pixel
fn tri(r: &mut XorShift32, w: i64, h: i64, gouraud: bool) -> String {
    let mut c = format!("{:#x}", 0xff00_0000 | (r.next() & 0xffffff));
    if gouraud {
        for _ in 0..2 {
            write!(c, "/{:#x}", 0xff00_0000 | (r.next() & 0xffffff));
        }
    }
    let mut l = format!("d3dtri {c}");
    for _ in 0..3 {
        // (mostly on a quarter-pixel grid, some anywhere in 16ths, some past the target)
        let (x, y) = if r.chance(4) { (r.range(-40, w * 16 + 40), r.range(-40, h * 16 + 40)) } else { (r.range(-1, w * 4 + 1) * 4, r.range(-1, h * 4 + 1) * 4) };
        write!(l, " {x} {y}");
    }
    l
}

pub fn d3d7() -> String {
    let mut s = String::from("# kikizake suite d3d7: Direct3D 1, 2, 3 and 7 (immediate mode through DirectDraw; SPEC.md). Made by `kikizake gen d3d7`.\n");
    let mut r = XorShift32::new(0x00d3_d007);
    for v in VERSIONS {
        writeln!(s, "case enum-{v}\nd3d {v}\nd3denum caps\nend");
    }
    // which devices each version makes, on which targets
    for v in VERSIONS {
        for d in DEVS {
            for (f, z) in [("565", ""), ("32", ""), ("565", " z16"), ("32", " z16"), ("32", " z24"), ("32", " z32")] {
                writeln!(s, "case dev-{v}-{d}-{f}{}\nd3d {v}\nd3ddev {d} {f} 8 4{z}\nend", z.replace(' ', "-"));
            }
        }
        for f in ["555", "24", "8"] {
            writeln!(s, "case dev-{v}-rgb-{f}\nd3d {v}\nd3ddev rgb {f} 8 4\nend");
        }
    }
    // render states: the defaults, without and with a z-buffer, then what a set leaves
    for v in [2, 3, 7] {
        for z in ["", " z16"] {
            writeln!(s, "case rs-{v}{}\nd3d {v}\nd3ddev rgb 565 8 4{z}", z.replace(' ', "-"));
            for st in 0..=160 {
                writeln!(s, "d3drs get {st}");
            }
            writeln!(s, "end");
        }
        writeln!(s, "case rsset-{v}\nd3d {v}\nd3ddev rgb 565 8 4");
        for st in [7, 8, 9, 14, 15, 16, 19, 20, 22, 23, 24, 25, 26, 27, 28, 29, 34, 36, 37, 38, 48, 52, 53, 56, 128, 136, 137, 139, 140, 141, 145, 146] {
            let val = *r.pick(&[0u32, 1, 2, 3, 4, 0x80, 0xffff_ffff]);
            writeln!(s, "d3drs set {st} {val:#x}\nd3drs get {st}");
        }
        writeln!(s, "end");
    }
    // Clear: every value of each channel into 1-pixel rectangles of each target format
    for v in [2, 3, 7] {
        for f in ["565", "555", "32", "24"] {
            writeln!(s, "case clear-{v}-{f}\nd3d {v}\nd3ddev rgb {f} 64 12");
            for ch in 0..3u32 {
                for val in 0..256u32 {
                    let (x, y) = (val % 64, ch * 4 + val / 64);
                    writeln!(s, "d3dclear {:#x} {x} {y} {} {}", 0xff00_0000 | val << (8 * (2 - ch)), x + 1, y + 1);
                }
            }
            writeln!(s, "d3ddump\nend");
        }
        writeln!(s, "case clearalpha-{v}\nd3d {v}\nd3ddev rgb 32 16 1\nd3dfill 3");
        for x in 0..16u32 {
            writeln!(s, "d3dclear {:#x} {x} 0 {} 1", (x * 17) << 24 | 0x123456, x + 1);
        }
        writeln!(s, "d3ddump\nend");
        writeln!(s, "case clearrects-{v}\nd3d {v}\nd3ddev rgb 32 16 8\nd3dfill 1\nd3dclear 0xff102030 2 2 6 5\nd3dclear 0xff405060 -2 -2 3 3\nd3dclear 0xff708090 14 6 20 12\nd3dclear 0xffa0b0c0 5 5 5 7\nd3dclear 0xffd0e0f0 7 3 4 6\nd3ddump\nend");
    }
    // the z-buffer's values for z = 0, 1/1000, ..., 1 (version 7)
    for z in ["z16", "z24", "z32"] {
        writeln!(s, "case clearz-{z}\nd3d 7\nd3ddev rgb 32 64 16 {z}");
        for m in 0..=1000u32 {
            let (x, y) = (m % 64, m / 64);
            writeln!(s, "d3dclear 0xff000000 z {m} {x} {y} {} {}", x + 1, y + 1);
        }
        writeln!(s, "d3dzdump\nend");
    }
    // triangles: which pixels each covers (flat color, culling off)
    for i in 0..160 {
        let v = *r.pick(&[7, 7, 7, 3, 2]);
        let f = *r.pick(&["32", "32", "565", "555", "24"]);
        let (w, h) = (r.range(4, 24), r.range(3, 16));
        writeln!(s, "case tri-{i}\nd3d {v}\nd3ddev rgb {f} {w} {h}\nd3drs set 22 1\nd3dclear 0xff000000");
        for _ in 0..r.range(1, 3) {
            writeln!(s, "{}", tri(&mut r, w, h, false));
        }
        writeln!(s, "d3ddump\nend");
    }
    // Gouraud shading, flat shading (the first vertex's color), dithering
    for i in 0..24 {
        let v = *r.pick(&[7, 3, 2]);
        let f = *r.pick(&["32", "565"]);
        let extra = match i % 4 {
            0 => "\nd3drs set 9 1",
            1 => "\nd3drs set 26 1",
            _ => "",
        };
        writeln!(s, "case shade-{i}\nd3d {v}\nd3ddev rgb {f} 16 12\nd3drs set 22 1{extra}\nd3dclear 0xff000000\n{}\nd3ddump\nend", tri(&mut r, 16, 12, true));
    }
    // culling: each mode and winding
    for v in [2, 3, 7] {
        for (m, cull) in ["", "\nd3drs set 22 1", "\nd3drs set 22 2", "\nd3drs set 22 3"].iter().enumerate() {
            writeln!(s, "case cull-{v}-{}\nd3d {v}\nd3ddev rgb 32 8 8{cull}\nd3dclear 0xff000000\nd3dtri 0xffff0000 0 0 128 0 0 128\nd3dtri 0xff00ff00 128 128 128 0 0 128\nd3ddump\nend", m);
        }
    }
    // alpha blending (SRCALPHA, INVSRCALPHA) of a flat color over known pixels, each alpha
    for a in (0..=255u32).step_by(5) {
        let v = *r.pick(&[7, 3]);
        writeln!(s, "case blend-{a}\nd3d {v}\nd3ddev rgb 32 16 8\nd3drs set 22 1\nd3dfill {}\nd3drs set 27 1\nd3drs set 19 5\nd3drs set 20 6\nd3dtri {:#x} -16 -16 600 -16 -16 600\nd3ddump\nend", a + 1, a << 24 | (r.next() & 0xffffff));
    }
    // the z test: a nearer triangle over a farther one and the other way round
    for v in [3, 7] {
        for z in ["z16", "z32"] {
            writeln!(s, "case ztest-{v}-{z}\nd3d {v}\nd3ddev rgb 32 16 8 {z}\nd3drs set 22 1\nd3drs set 7 1\nd3dclear 0xff000000 z 1000\nd3dtri 0xffff0000 0 0 256 0 0 128 z 600\nd3dtri 0xff00ff00 0 0 256 128 0 128 z 300\nd3dtri 0xff0000ff 0 0 256 0 256 128 z 450\nd3ddump\nd3dzdump\nend");
        }
    }
    // the same windings with no pixel center near an edge (1/4 px off the grid)
    for v in [2, 3, 7] {
        for (m, cull) in ["", "\nd3drs set 22 1", "\nd3drs set 22 2", "\nd3drs set 22 3"].iter().enumerate() {
            writeln!(s, "case cullin-{v}-{}\nd3d {v}\nd3ddev rgb 32 8 8{cull}\nd3dclear 0xff000000\nd3dtri 0xffff0000 4 4 116 4 4 116\nd3dtri 0xff00ff00 116 116 116 4 4 116\nd3ddump\nend", m);
        }
    }
    // the z test with triangles covering the whole target, each order of z
    for v in [3, 7] {
        for z in ["z16", "z32"] {
            for (i, (a, b)) in [(600, 300), (300, 600), (500, 500)].iter().enumerate() {
                writeln!(s, "case zcover-{v}-{z}-{i}\nd3d {v}\nd3ddev rgb 32 8 4 {z}\nd3drs set 22 1\nd3drs set 7 1\nd3dclear 0xff000000 z 1000\nd3dtri 0xffff0000 -16 -16 600 -16 -16 600 z {a}\nd3dtri 0xff00ff00 -16 -16 600 -16 -16 600 z {b}\nd3ddump\nd3dzdump\nend");
            }
        }
    }
    // the z a triangle writes: every 7/1000 from 0 to 1 (and 1)
    for z in ["z16", "z32"] {
        writeln!(s, "case zsweep-{z}\nd3d 7\nd3ddev rgb 32 2 1 {z}\nd3drs set 22 1\nd3drs set 7 1");
        for m in (0..=1000).step_by(7).chain([1000]) {
            writeln!(s, "d3dclear 0xff000000 z 1000\nd3dtri 0xffff0000 -16 -16 600 -16 -16 600 z {m}\nd3dzdump");
        }
        writeln!(s, "end");
    }
    // a z test other than LESSEQUAL, other blend factors, blending and dithering on 16 bits
    writeln!(s, "case zfunc\nd3d 7\nd3ddev rgb 32 8 4 z16\nd3drs set 22 1\nd3drs set 7 1\nd3drs set 23 2\nd3dclear 0xff000000 z 500\nd3dtri 0xffff0000 -16 -16 600 -16 -16 600 z 500\nd3ddump\nend");
    writeln!(s, "case blendone\nd3d 7\nd3ddev rgb 32 8 4\nd3drs set 22 1\nd3dfill 7\nd3drs set 27 1\nd3drs set 19 2\nd3drs set 20 2\nd3dtri 0x80402010 -16 -16 600 -16 -16 600\nd3ddump\nend");
    writeln!(s, "case blend565\nd3d 7\nd3ddev rgb 565 8 4\nd3drs set 22 1\nd3dfill 9\nd3drs set 27 1\nd3drs set 19 5\nd3drs set 20 6\nd3dtri 0x80c08040 -16 -16 600 -16 -16 600\nd3ddump\nend");
    writeln!(s, "case dither565\nd3d 7\nd3ddev rgb 565 8 4\nd3drs set 22 1\nd3drs set 9 1\nd3drs set 26 1\nd3dtri 0xff837261 -16 -16 600 -16 -16 600\nd3ddump\nend");
    // Clear of the z-buffer without one
    writeln!(s, "case clearnoz\nd3d 7\nd3ddev rgb 32 8 4\nd3dclear 0xff102030 z 500\nd3ddump\nend");
    // drawing on the HAL
    writeln!(s, "case haldraw\nd3d 7\nd3ddev hal 32 8 4\nd3dclear 0xff102030\nd3dtri 0xffff0000 4 4 132 4 4 132\nd3ddump\nend");
    // without a device
    writeln!(s, "case nodev\nd3denum\nd3ddev rgb 32 8 4\nd3drs get 7\nd3dclear 0\nd3dtri 0 0 0 1 1 2 2\nd3ddump\nend");
    writeln!(s, "case nodevice\nd3d 7\nd3drs get 7\nd3dclear 0\nd3dtri 0 0 0 1 1 2 2\nd3ddump\nend");
    // version 1 has no render states, Clear or DrawPrimitive here
    writeln!(s, "case v1\nd3d 1\nd3ddev rgb 32 8 4\nd3drs get 7\nd3dclear 0\nd3ddump\nend");
    // states past the documented ones last (Windows may not check them)
    for v in [2, 3, 7] {
        writeln!(s, "case rs-high-{v}\nd3d {v}\nd3ddev rgb 565 8 4");
        for st in 161..=255 {
            writeln!(s, "d3drs get {st}");
        }
        writeln!(s, "end");
    }
    s
}

/// the devices that draw: (version, device) pairs, the RGB device first
const DRAWERS: &[(u32, &str)] = &[(7, "rgb"), (7, "hal"), (7, "tnl"), (7, "mmx"), (3, "rgb"), (2, "rgb"), (2, "ramp"), (3, "mmx")];

/// which pixels triangles cover: each of a few triangles in every vertex order and moved by every
/// 16th of a pixel, inside the target and across its edges
pub fn d3d7rast() -> String {
    let mut s = String::from("# kikizake suite d3d7rast: Direct3D 2, 3 and 7 triangle coverage (SPEC.md). Made by `kikizake gen d3d7rast`.\n# x86 only\n");
    let mut r = XorShift32::new(0x00d3_d7a5);
    // (inside a 12 x 12 target, room for a pixel's shift)
    let inside: &[[(i64, i64); 3]] = &[
        [(24, 20), (150, 52), (60, 160)],
        [(20, 20), (160, 20), (20, 160)],
        [(90, 16), (170, 170), (16, 120)],
        [(40, 30), (44, 170), (170, 100)],
        [(16, 80), (170, 72), (88, 84)],
        [(100, 17), (101, 171), (17, 93)],
    ];
    let perms: [[usize; 3]; 6] = [[0, 1, 2], [1, 2, 0], [2, 0, 1], [0, 2, 1], [2, 1, 0], [1, 0, 2]];
    let line = |v: &[(i64, i64); 3], p: [usize; 3], dx: i64, dy: i64, c: u32| format!("d3dclear 0xff000000\nd3dtri {c:#x} {} {} {} {} {} {}\nd3ddump", v[p[0]].0 + dx, v[p[0]].1 + dy, v[p[1]].0 + dx, v[p[1]].1 + dy, v[p[2]].0 + dx, v[p[2]].1 + dy);
    for (t, v) in inside.iter().enumerate() {
        // every shift in one vertex order; every order at 16 shifts
        writeln!(s, "case in-{t}\nd3d 7\nd3ddev rgb 565 12 12\nd3drs set 22 1");
        for dy in 0..16 {
            for dx in 0..16 {
                writeln!(s, "{}", line(v, perms[0], dx, dy, 0xffffffff));
            }
        }
        writeln!(s, "end");
        writeln!(s, "case in-{t}-orders\nd3d 7\nd3ddev rgb 565 12 12\nd3drs set 22 1");
        for p in &perms[1..] {
            for k in 0..16 {
                writeln!(s, "{}", line(v, *p, k, (k * 7) % 16, 0xffffffff));
            }
        }
        writeln!(s, "end");
    }
    // across the target's edges by a pixel, ten and a hundred pixels, and far past it
    let mut across = vec![];
    for far in [16i64, 160, 1600, 16000] {
        across.push([(-far, 20), (100, 40), (40, 120)]);
        across.push([(30, -far), (120, 60), (20, 110)]);
        across.push([(20, 30), (128 + far, 60), (60, 110)]);
        across.push([(30, 20), (110, 50), (50, 128 + far)]);
        across.push([(-far, -far), (100, 30), (30, 110)]);
        across.push([(-far, 60), (128 + far, 20), (60, 128 + far)]);
    }
    for (t, v) in across.iter().enumerate() {
        writeln!(s, "case across-{t}\nd3d 7\nd3ddev rgb 565 8 8\nd3drs set 22 1");
        for k in 0..32 {
            writeln!(s, "{}", line(v, perms[k % 6], k as i64 % 16, (k as i64 * 5) % 16, 0xffffffff));
        }
        writeln!(s, "end");
    }
    // random triangles, some across the edges, one a line
    for i in 0..64 {
        writeln!(s, "case rand-{i}\nd3d 7\nd3ddev rgb 565 12 10\nd3drs set 22 1");
        for _ in 0..8 {
            writeln!(s, "d3dclear 0xff000000\n{}\nd3ddump", tri(&mut r, 12, 10, false));
        }
        writeln!(s, "end");
    }
    // the other devices: a few shifts of each triangle
    for &(v, d) in &DRAWERS[1..] {
        writeln!(s, "case dev-{v}-{d}\nd3d {v}\nd3ddev {d} 565 12 12\nd3drs set 22 1");
        for (t, tv) in inside.iter().enumerate().take(4) {
            for k in 0..8 {
                writeln!(s, "{}", line(tv, perms[(t + k as usize) % 6], k * 2, (k * 5) % 16, 0xffffffff));
            }
        }
        for tv in across.iter().take(12) {
            writeln!(s, "{}", line(tv, perms[0], 3, 7, 0xffffffff));
        }
        writeln!(s, "end");
    }
    // triangles crossing the target on the ramp device and the RGB one (dev-2-ramp drew a pixel
    // by a clipped edge that the RGB walk cut at the sides does not): the one, shifted, and
    // random ones, a dump each
    let mut rc = XorShift32::new(0x00d3_d7c1);
    let mut ramp = Vec::new();
    for d in ["ramp", "rgb"] {
        writeln!(s, "case clip-2-{d}\nd3d 2\nd3ddev {d} 565 12 12\nd3drs set 22 1");
        let mut tris = Vec::new();
        for k in 0..8 {
            tris.push([((-13i64).wrapping_sub(k), 27i64.wrapping_add(k)), (103i64.wrapping_add(k), 47), (43, 127i64.wrapping_sub(k))]);
        }
        for _ in 0..40 {
            let v: Vec<i64> = (0..6).map(|_| rc.range(0, 260).wrapping_sub(40)).collect();
            tris.push([(v[0], v[1]), (v[2], v[3]), (v[4], v[5])]);
        }
        for tv in &tris {
            writeln!(s, "{}", line(tv, perms[0], 0, 0, 0xffffffff));
        }
        writeln!(s, "end");
        if d == "ramp" {
            ramp = tris;
        }
    }
    // the ramp device's clipped triangles with D3DRENDERSTATE_ZWRITEENABLE (14) set to 0 and to
    // 1 (no z-buffer): whether the color of a clipped triangle follows the span function the z
    // states choose (D3D7.TRI.CLIP)
    for zw in [0, 1] {
        writeln!(s, "case clip-2-ramp-zw{zw}\nd3d 2\nd3ddev ramp 565 12 12\nd3drs set 22 1\nd3drs set 14 {zw}");
        for tv in &ramp {
            writeln!(s, "{}", line(tv, perms[0], 0, 0, 0xffffffff));
        }
        writeln!(s, "end");
    }
    // the ramp device's near-flat slivers whose alpha changes by 2^30 or more of J (alpha · 2^16)
    // a pixel across (D3D7.TRI.CLIP: TriSetup_DIdx's integer steps then set aside)
    writeln!(s, "case steep-2-ramp\nd3d 2\nd3ddev ramp 565 12 12\nd3drs set 22 1");
    for (k, zw) in [(1i64, 1), (2, 1), (3, 0), (5, 0)] {
        writeln!(s, "d3drs set 14 {zw}\nd3dclear 0\nd3dtri 0x00ffffff/0xffffffff/0x00ffffff 8 0 {} 80 {} {}\nd3ddump", 8i64.wrapping_add(k), 8i64.wrapping_add(k.wrapping_mul(2)), 160i64.wrapping_add(1));
    }
    writeln!(s, "end");
    // the same steep alpha steps over pixels that are drawn: a narrow upright triangle around
    // column 1 (its centre x = 16 sixteenths) from a flat top at y = 1/2 to an apex at y = 10,
    // alphas 0x00 / 0xff / 0x00, then 0x00 / 0x00 / 0xff, z writes off and on, half widths 1 to 8
    writeln!(s, "case steepvis-2-ramp\nd3d 2\nd3ddev ramp 565 12 12\nd3drs set 22 1");
    for zw in [0, 1] {
        for cols in ["0x00ffffff/0xffffffff/0x00ffffff", "0x00ffffff/0x00ffffff/0xffffffff"] {
            for hw in [1i64, 2, 4, 8] {
                writeln!(s, "d3drs set 14 {zw}\nd3dclear 0\nd3dtri {cols} {} 8 {} 8 16 160\nd3ddump", 16i64.wrapping_sub(hw), 16i64.wrapping_add(hw));
            }
        }
    }
    writeln!(s, "end");
    s
}

/// colors: Gouraud shading, dithering, blending (every factor), the z functions, the alpha
/// test, fog, fill modes, on every device that draws
pub fn d3d7shade() -> String {
    let mut s = String::from("# kikizake suite d3d7shade: Direct3D 2, 3 and 7 shading, dithering, blending and per-pixel tests (SPEC.md). Made by `kikizake gen d3d7shade`.\n# x86 only\n");
    let mut r = XorShift32::new(0x00d3_d75e);
    let cover = "-16 -16 600 -16 -16 600";
    for &(v, d) in DRAWERS {
        let dv = format!("d3d {v}\nd3ddev {d}");
        // Gouraud: one channel 0 to 255 along x, along y, and random colors, inside the target
        writeln!(s, "case gouraud-{v}-{d}\n{dv} 32 16 16\nd3drs set 22 1");
        for (c0, c1, c2) in [(0xff000000u32, 0xffff0000u32, 0xff000000u32), (0xff000000, 0xff000000, 0xff00ff00), (0xff0000ff, 0xff000000, 0xff000000), (0xff102030, 0xffe0c0a0, 0xff7f8081)] {
            writeln!(s, "d3dclear 0\nd3dtri {c0:#x}/{c1:#x}/{c2:#x} 0 0 256 0 0 256\nd3ddump");
            writeln!(s, "d3dclear 0\nd3dtri {c0:#x}/{c1:#x}/{c2:#x} 8 8 248 8 8 248\nd3ddump");
        }
        for _ in 0..if d == "rgb" && v == 7 { 24 } else { 4 } {
            let c: Vec<u32> = (0..3).map(|_| r.next()).collect();
            let p: Vec<i64> = (0..6).map(|_| r.range(0, 255)).collect();
            writeln!(s, "d3dclear 0\nd3dtri {:#x}/{:#x}/{:#x} {} {} {} {} {} {}\nd3ddump", c[0], c[1], c[2], p[0], p[1], p[2], p[3], p[4], p[5]);
        }
        writeln!(s, "end");
        // the same on 16 bits, dithered and not
        for f in ["565", "555"] {
            for dither in [0, 1] {
                writeln!(s, "case gouraud16-{v}-{d}-{f}-{dither}\n{dv} {f} 16 16\nd3drs set 22 1\nd3drs set 26 {dither}");
                for _ in 0..4 {
                    let c: Vec<u32> = (0..3).map(|_| r.next()).collect();
                    writeln!(s, "d3dclear 0\nd3dtri {:#x}/{:#x}/{:#x} 0 0 256 0 0 256\nd3ddump", c[0], c[1], c[2]);
                }
                writeln!(s, "end");
            }
        }
        // dithering flat colors: gray 0..255, then random
        for f in ["565", "555"] {
            writeln!(s, "case dither-{v}-{d}-{f}\n{dv} {f} 8 8\nd3drs set 22 1\nd3drs set 9 1\nd3drs set 26 1");
            let n = if d == "rgb" && v == 7 { 256 } else { 32 };
            for k in 0..n {
                let g = if n == 256 { k } else { k * 8 + 3 };
                let c = if k % 2 == 0 && n == 256 { g * 0x10101 } else { r.next() & 0xffffff };
                writeln!(s, "d3dtri {:#x} {cover}\nd3ddump", 0xff00_0000 | c);
            }
            writeln!(s, "end");
        }
        // blending, every source and destination factor, on 32 and 16 bits
        for f in ["32", "565", "555"] {
            writeln!(s, "case blend-{v}-{d}-{f}\n{dv} {f} 4 2\nd3drs set 22 1\nd3drs set 27 1");
            let all = d == "rgb" && v == 7 || f == "32";
            for sf in 1..=13 {
                for df in 1..=13 {
                    if !all && (sf + df) % 5 != 0 {
                        continue;
                    }
                    writeln!(s, "d3dfill {}\nd3drs set 19 {sf}\nd3drs set 20 {df}\nd3dtri {:#x} {cover}\nd3ddump", r.range(1, 99999), r.next());
                }
            }
            writeln!(s, "end");
            // SRCALPHA, INVSRCALPHA: each alpha
            writeln!(s, "case blendalpha-{v}-{d}-{f}\n{dv} {f} 8 2\nd3drs set 22 1\nd3drs set 27 1\nd3drs set 19 5\nd3drs set 20 6");
            for a in (0..=255u32).step_by(if all { 3 } else { 17 }) {
                writeln!(s, "d3dfill {}\nd3dtri {:#x} {cover}\nd3ddump", a + 1, a << 24 | (r.next() & 0xffffff));
            }
            writeln!(s, "end");
        }
        // the z functions: nearer, equal, farther, with z writes on and off
        for z in ["z16", "z32"] {
            writeln!(s, "case zfunc-{v}-{d}-{z}\n{dv} 32 2 1 {z}\nd3drs set 22 1\nd3drs set 7 1");
            for func in 1..=8 {
                for write in [1, 0] {
                    for (a, b) in [(500, 300), (500, 500), (500, 700), (0, 0), (1000, 1000)] {
                        writeln!(s, "d3drs set 23 4\nd3drs set 14 1\nd3dclear 0xff000000 z 1000\nd3dtri 0xff0000ff {cover} z {a}\nd3drs set 23 {func}\nd3drs set 14 {write}\nd3dtri 0xff00ff00 {cover} z {b}\nd3ddump\nd3dzdump");
                    }
                }
            }
            writeln!(s, "end");
        }
        // the alpha test: each function and reference against each alpha step
        writeln!(s, "case alphatest-{v}-{d}\n{dv} 32 2 1\nd3drs set 22 1\nd3drs set 15 1");
        for func in 1..=8 {
            for aref in [0, 1, 127, 128, 255] {
                writeln!(s, "d3drs set 25 {func}\nd3drs set 24 {aref}");
                for a in [0u32, 1, 126, 127, 128, 129, 254, 255] {
                    writeln!(s, "d3dclear 0xff000000\nd3dtri {:#x} {cover}\nd3ddump", a << 24 | 0xffffff);
                }
            }
        }
        writeln!(s, "end");
        // fog: vertex fog from the specular alpha, with the fog color, and table fog off
        writeln!(s, "case fog-{v}-{d}\n{dv} 32 4 1\nd3drs set 22 1\nd3drs set 28 1\nd3drs set 34 0xff4080c0");
        for f in (0..=255u32).step_by(5) {
            writeln!(s, "d3dclear 0\nd3dtri 0xff102030 {cover} spec {:#x}\nd3ddump", f << 24);
        }
        writeln!(s, "end");
        // specular added, and the fill modes (points, wireframe) of a triangle inside
        writeln!(s, "case specular-{v}-{d}\n{dv} 32 4 1\nd3drs set 22 1\nd3drs set 29 1");
        for _ in 0..16 {
            writeln!(s, "d3dclear 0\nd3dtri {:#x} {cover} spec {:#x}\nd3ddump", r.next(), r.next());
        }
        writeln!(s, "end");
        writeln!(s, "case fill-{v}-{d}\n{dv} 565 12 12\nd3drs set 22 1");
        for mode in [1, 2, 3] {
            for lp in [1, 0] {
                writeln!(s, "d3drs set 8 {mode}\nd3drs set 16 {lp}\nd3dclear 0\nd3dtri 0xffffffff 24 20 150 52 60 160\nd3ddump\nd3dclear 0\nd3dtri 0xffffffff 20 20 160 20 20 160\nd3ddump");
            }
        }
        writeln!(s, "end");
    }
    // the Gouraud fixed point (D3D7.GOURAUD): random triangles with long spans in a 64 x 16
    // target, and ones whose colors change along x only (two vertices one color, on a column)
    // (its own generator, so that the cases before keep theirs)
    let mut rg = XorShift32::new(0x00d3_d7a5);
    for d in ["rgb", "mmx"] {
        writeln!(s, "case gstep-{d}\nd3d 7\nd3ddev {d} 32 64 16\nd3drs set 22 1");
        for _ in 0..48 {
            let c: Vec<u32> = (0..3).map(|_| rg.next()).collect();
            let p: Vec<i64> = (0..3).flat_map(|_| [rg.range(0, 1023), rg.range(0, 255)]).collect();
            writeln!(s, "d3dclear 0\nd3dtri {:#x}/{:#x}/{:#x} {} {} {} {} {} {}\nd3ddump", c[0], c[1], c[2], p[0], p[1], p[2], p[3], p[4], p[5]);
        }
        writeln!(s, "end");
        writeln!(s, "case gstepx-{d}\nd3d 7\nd3ddev {d} 32 64 16\nd3drs set 22 1");
        for _ in 0..48 {
            let (a, b) = (rg.next(), rg.next());
            let (x0, x1) = (rg.range(0, 200), rg.range(600, 1023));
            let (y0, y1, y2) = (rg.range(0, 40), rg.range(216, 255), rg.range(0, 255));
            writeln!(s, "d3dclear 0\nd3dtri {a:#x}/{b:#x}/{a:#x} {x0} {y0} {x1} {y2} {x0} {y1}\nd3ddump");
        }
        writeln!(s, "end");
    }
    // Gouraud colors dithered to 565 and 555 (which fraction the dither sees; D3D7.GOURAUD):
    // wide random triangles, the dither on (its own generator)
    let mut rd = XorShift32::new(0x00d3_d7a6);
    for d in ["rgb", "mmx"] {
        for f in ["565", "555"] {
            writeln!(s, "case gdither-{d}-{f}\nd3d 7\nd3ddev {d} {f} 64 16\nd3drs set 22 1\nd3drs set 26 1");
            for _ in 0..24 {
                let c: Vec<u32> = (0..3).map(|_| rd.next()).collect();
                let p: Vec<i64> = (0..3).flat_map(|_| [rd.range(0, 1023), rd.range(0, 255)]).collect();
                writeln!(s, "d3dclear 0\nd3dtri {:#x}/{:#x}/{:#x} {} {} {} {} {} {}\nd3ddump", c[0], c[1], c[2], p[0], p[1], p[2], p[3], p[4], p[5]);
            }
            writeln!(s, "end");
        }
    }
    s
}

/// a vertex on the 16th grid: x, y in 16ths of a pixel
#[derive(Clone, Copy)]
struct V16 {
    x: i64,
    y: i64,
}

/// the edge from `t` (above) to `b`: its x at the first row at or below t, in 16ths, when that
/// is a whole pixel
fn x_at_first_row(t: V16, b: V16) -> Option<i64> {
    let row = (t.y + 15).div_euclid(16) * 16;
    let num = (t.x * (b.y - t.y) + (b.x - t.x) * (row - t.y)) * 16;
    let den = (b.y - t.y) * 16;
    (num % (den * 16) == 0).then(|| num / den)
}

/// the edges the d3d7rast scans found shifted: vertices on pixel centers at each row, edges whose
/// x is a whole pixel on their first row (short and long), on the RGB and MMX devices
/// no row of the triangle spills past the 12 x 12 target's last pixel (memory after it would be
/// written: d3d7edge-32 died after whole-rgb-short): a triangle reaching the last row stays
/// left of x 10
fn stays_in(v: &[V16]) -> bool {
    v.iter().all(|p| p.y < 176) || v.iter().all(|p| p.x <= 160)
}

/// The pixel slopes a/b (minor over major) of the long-edge and long-line sequences: odd and
/// even denominators up to 13, both signs
const LONG_SLOPES: [(i64, i64); 24] = [(1, 3), (-1, 3), (1, 5), (-1, 5), (2, 5), (-2, 5), (1, 7), (-1, 7), (2, 7), (-3, 7), (3, 7), (1, 9), (-2, 9), (4, 9), (-4, 9), (1, 11), (-3, 11), (4, 11), (-1, 13), (4, 13), (-5, 13), (3, 8), (-3, 8), (-1, 4)];

pub fn d3d7edge() -> String {
    let mut s = String::from("# kikizake suite d3d7edge: Direct3D 7 software rasterizer edges (SPEC.md). Made by `kikizake gen d3d7edge`.\n# x86 only\n");
    let mut r = XorShift32::new(0x00d3_d7ed);
    let tri = |a: V16, b: V16, c: V16| format!("d3dclear 0xff000000\nd3dtri 0xffffffff {} {} {} {} {} {}\nd3ddump", a.x, a.y, b.x, b.y, c.x, c.y);
    for dev in ["rgb", "mmx"] {
        // a vertex on a pixel center at each row as the top, middle and bottom vertex
        for role in ["top", "mid", "bot"] {
            writeln!(s, "case center-{dev}-{role}\nd3d 7\nd3ddev {dev} 565 12 12\nd3drs set 22 1");
            for row in 0..10 {
                for _ in 0..6 {
                    let c = V16 { x: r.range(2, 9) * 16, y: row * 16 };
                    let (a, b) = match role {
                        "top" => (V16 { x: r.range(0, 191), y: r.range(row * 16 + 8, 191) }, V16 { x: r.range(0, 191), y: r.range(row * 16 + 8, 191) }),
                        "mid" => (V16 { x: r.range(0, 191), y: r.range(0, (row * 16 - 4).max(0)) }, V16 { x: r.range(0, 191), y: r.range(row * 16 + 8, 191) }),
                        _ => (V16 { x: r.range(0, 191), y: r.range(0, (row * 16 - 8).max(0)) }, V16 { x: r.range(0, 191), y: r.range(0, (row * 16 - 8).max(0)) }),
                    };
                    if stays_in(&[c, a, b]) {
                        writeln!(s, "{}", tri(c, a, b));
                    }
                }
            }
            writeln!(s, "end");
        }
        // an edge whose x is a whole pixel on its first row, its top off the rows: as the
        // short edge (third vertex below its top, above its bottom) and as the long edge
        for kind in ["short", "long"] {
            if kind == "long" {
                // (the Windows executor crashed in whole-rgb-long: the long edges come last, in
                // cases of 12, so that the crash leaves the rest and names its triangles)
                continue;
            }
            writeln!(s, "case whole-{dev}-{kind}\nd3d 7\nd3ddev {dev} 565 12 12\nd3drs set 22 1");
            let mut n = 0;
            while n < 96 {
                let t = V16 { x: r.range(0, 191), y: r.range(0, 120) };
                let b = V16 { x: r.range(0, 191), y: r.range(t.y + 24, 191) };
                if t.y % 16 == 0 || x_at_first_row(t, b).is_none() {
                    continue;
                }
                let third = if kind == "short" { V16 { x: r.range(0, 191), y: r.range(b.y + 1, 192) } } else { V16 { x: r.range(0, 191), y: r.range(t.y + 1, b.y - 1) } };
                if third.y > 191 || third.y == b.y || !stays_in(&[t, b, third]) {
                    continue;
                }
                writeln!(s, "{}", tri(t, b, third));
                n += 1;
            }
            writeln!(s, "end");
        }
    }
    for dev in ["rgb", "mmx"] {
        for chunk in 0..8 {
            writeln!(s, "case whole-{dev}-long-{chunk}\nd3d 7\nd3ddev {dev} 565 12 12\nd3drs set 22 1");
            let mut n = 0;
            while n < 12 {
                let t = V16 { x: r.range(0, 191), y: r.range(0, 120) };
                let b = V16 { x: r.range(0, 191), y: r.range(t.y + 24, 191) };
                if t.y % 16 == 0 || x_at_first_row(t, b).is_none() {
                    continue;
                }
                let third = V16 { x: r.range(0, 191), y: r.range(t.y + 1, b.y - 1) };
                if !stays_in(&[t, b, third]) {
                    continue;
                }
                writeln!(s, "{}", tri(t, b, third));
                n += 1;
            }
            writeln!(s, "end");
        }
    }
    // the long edge on the left with an exact tie on a later row (D3D7.TRI.EDGE): a third
    // vertex right of it, between its ends; cases of 12 (its own generator)
    let mut rl = XorShift32::new(0x00d3_d7e1);
    for dev in ["rgb", "mmx"] {
        for chunk in 0..8 {
            writeln!(s, "case longleft-{dev}-{chunk}\nd3d 7\nd3ddev {dev} 565 12 12\nd3drs set 22 1");
            let mut n = 0;
            while n < 12 {
                let t = V16 { x: rl.range(0, 191), y: rl.range(0, 100) };
                let b = V16 { x: rl.range(0, 191), y: rl.range(t.y + 40, 191) };
                let (dx, dy) = (b.x - t.x, b.y - t.y);
                let first = (t.y + 15) / 16 + 1;
                let tie = (first..=b.y / 16).any(|y| (t.x * dy + (16 * y - t.y) * dx).rem_euclid(16 * dy) == 0);
                let third = V16 { x: rl.range(0, 191), y: rl.range(t.y + 1, b.y - 1) };
                // (left: the third vertex right of the long edge)
                let right = dx * (third.y - t.y) - dy * (third.x - t.x) < 0;
                if !tie || !right || !stays_in(&[t, b, third]) {
                    continue;
                }
                writeln!(s, "{}", tri(t, b, third));
                n += 1;
            }
            writeln!(s, "end");
        }
    }
    // the same long-left ties on both drawers: each triangle as drawn, its vertices rotated,
    // moved a whole pixel across and down (which of those keep a tie's side)
    let mut rm = XorShift32::new(0x00d3_d7e2);
    let mut base = Vec::new();
    while base.len() < 24 {
        let t = V16 { x: rm.range(0, 170), y: rm.range(0, 80) };
        let b = V16 { x: rm.range(0, 170), y: rm.range(t.y + 40, 170) };
        let (dx, dy) = (b.x - t.x, b.y - t.y);
        let first = (t.y + 15) / 16 + 1;
        let tie = (first..=b.y / 16).any(|y| (t.x * dy + (16 * y - t.y) * dx).rem_euclid(16 * dy) == 0);
        let third = V16 { x: rm.range(0, 170), y: rm.range(t.y + 1, b.y - 1) };
        let right = dx * (third.y - t.y) - dy * (third.x - t.x) < 0;
        let moved = |v: V16| V16 { x: v.x + 16, y: v.y + 16 };
        if tie && right && stays_in(&[t, b, third]) && stays_in(&[moved(t), moved(b), moved(third)]) {
            base.push((t, b, third));
        }
    }
    for dev in ["rgb", "mmx"] {
        writeln!(s, "case longmoves-{dev}\nd3d 7\nd3ddev {dev} 565 12 12\nd3drs set 22 1");
        for &(t, b, c) in &base {
            let (x, y) = (|v: V16| V16 { x: v.x + 16, y: v.y }, |v: V16| V16 { x: v.x, y: v.y + 16 });
            for (p, q, r) in [(t, b, c), (b, c, t), (x(t), x(b), x(c)), (y(t), y(b), y(c))] {
                writeln!(s, "{}", tri(p, q, r));
            }
        }
        writeln!(s, "end");
    }
    // a top exactly on row 0, off a pixel, its long edge on a pixel 1 or 2 rows down (D3D7.TRI.EDGE:
    // 2 of 2 such went a pixel right): the long edge on either side, its bottom m whole steps on
    let mut rt = XorShift32::new(0x00d3_d7e4);
    for side in ["left", "right"] {
        writeln!(s, "case toprow-rgb-{side}\nd3d 7\nd3ddev rgb 565 12 12\nd3drs set 22 1");
        let mut n = 0;
        while n < 32 {
            let tx = rt.range(1, 150);
            let px = rt.range(0, 10);
            let k = rt.range(1, 2);
            let m = rt.range(1, 5);
            if tx % 16 == 0 {
                continue;
            }
            let t = V16 { x: tx, y: 0 };
            let b = V16 { x: tx + m * (16 * px - tx), y: m * 16 * k };
            let mid = b.y / 2;
            let along = t.x + (b.x - t.x) * mid / b.y;
            let c = V16 { x: if side == "left" { along + rt.range(24, 80) } else { along - rt.range(24, 80) }, y: mid };
            if !(0..=176).contains(&b.x) || b.y > 176 || !(0..=176).contains(&c.x) || !stays_in(&[t, b, c]) {
                continue;
            }
            writeln!(s, "{}", tri(t, b, c));
            n += 1;
        }
        writeln!(s, "end");
    }
    // a long edge one row tall, from exactly row 0 off a pixel to a bottom vertex on a pixel of row
    // 1 (D3D7.TRI.EDGE: 7 of 7 went a pixel right there), on either side, both soft drawers; and
    // its controls, a top above row 0 (-1/2) and one just under it (1/16), which go by the walk
    let mut ro = XorShift32::new(0x00d3_d7e5);
    for dev in ["rgb", "mmx"] {
        for side in ["left", "right"] {
            writeln!(s, "case onerow-{dev}-{side}\nd3d 7\nd3ddev {dev} 565 12 4\nd3drs set 22 1");
            let mut n = 0;
            while n < 36 {
                let ty = [0, 0, 0, 0, -8, 1][usize::try_from(n % 6).unwrap_or(0)];
                let tx = ro.range(1, 175);
                let bx = 16 * ro.range(0, 10);
                let my = ro.range(2, 14);
                let along = tx + (bx - tx) * (my - ty) / (16 - ty);
                let mx = if side == "left" { along + ro.range(24, 80) } else { along - ro.range(24, 80) };
                if tx % 16 == 0 || !(0..=176).contains(&mx) {
                    continue;
                }
                let (t, b, c) = (V16 { x: tx, y: ty }, V16 { x: bx, y: 16 }, V16 { x: mx, y: my });
                writeln!(s, "{}", tri(t, b, c));
                n += 1;
            }
            writeln!(s, "end");
        }
    }
    // the same one row tall triangles a few rows lower, the bottom vertex on a pixel of row 2 to 4
    // (D3D7.TRI.EDGE: row 1 only is measured empty, toprow-* on row 2 two rows tall went by the walk)
    let mut rl = XorShift32::new(0x00d3_d7e6);
    for side in ["left", "right"] {
        writeln!(s, "case lowrow-rgb-{side}\nd3d 7\nd3ddev rgb 565 12 6\nd3drs set 22 1");
        let mut n = 0;
        while n < 36 {
            let row = 2 + n % 3;
            let ty = 16 * (row - 1) + [0, -8, 1][usize::try_from(n / 3 % 3).unwrap_or(0)];
            let tx = rl.range(1, 175);
            let bx = 16 * rl.range(0, 10);
            let my = ty + rl.range(2, 14);
            let along = tx + (bx - tx) * (my - ty) / (16 * row - ty);
            let mx = if side == "left" { along + rl.range(24, 80) } else { along - rl.range(24, 80) };
            if tx % 16 == 0 || !(0..=176).contains(&mx) || my >= 16 * row {
                continue;
            }
            let (t, b, c) = (V16 { x: tx, y: ty }, V16 { x: bx, y: 16 * row }, V16 { x: mx, y: my });
            writeln!(s, "{}", tri(t, b, c));
            n += 1;
        }
        writeln!(s, "end");
    }
    // long-left edges through a whole pixel every few rows down a tall target (D3D7.TRI.EDGE):
    // a pixel slope a/b, the edge through pixel P and its vertices whole steps of (a, b)
    // sixteenths from P, so its x is a whole pixel on every b-th row; how each such row settles
    let mut rq = XorShift32::new(0x00d3_d7e3);
    for part in 0..2 {
        writeln!(s, "case longseq-rgb-{part}\nd3d 7\nd3ddev rgb 565 64 128\nd3drs set 22 1");
        for &(a, b) in &LONG_SLOPES {
            loop {
                let p = V16 { x: rq.range(4, 59) * 16, y: rq.range(40, 80) * 16 };
                let m = p.y / b - rq.range(0, 15);
                let n = (16 * 126 - p.y) / b - rq.range(0, 15);
                let t = V16 { x: p.x - m * a, y: p.y - m * b };
                let bot = V16 { x: p.x + n * a, y: p.y + n * b };
                let c = V16 { x: 16 * 62, y: (t.y + bot.y) / 2 };
                let inside = |v: V16| (0..=16 * 60).contains(&v.x) && (0..=16 * 126).contains(&v.y);
                if inside(t) && inside(bot) && t.x.max(bot.x) < 16 * 56 {
                    writeln!(s, "{}", tri(t, bot, c));
                    break;
                }
            }
        }
        writeln!(s, "end");
    }
    s
}

/// each drawing device's Clear and flat colors (every value of each channel), z near and past
/// the ends of 0..1, and wireframe and point triangles (where lines and points fall)
pub fn d3d7dev() -> String {
    let mut s = String::from("# kikizake suite d3d7dev: Direct3D 2, 3 and 7 colors, z range, points and wireframe on every device (SPEC.md). Made by `kikizake gen d3d7dev`.\n# x86 only\n");
    let mut r = XorShift32::new(0x00d3_d7de);
    let cover = "-16 -16 600 -16 -16 600";
    for &(v, d) in DRAWERS {
        let dv = format!("d3d {v}\nd3ddev {d}");
        for f in ["565", "555", "24", "32"] {
            // Clear and a flat triangle of each value of each channel (alpha 0x80)
            writeln!(s, "case color-{v}-{d}-{f}\n{dv} {f} 2 1\nd3drs set 22 1");
            for sh in [16, 8, 0] {
                for val in 0..256u32 {
                    writeln!(s, "d3dclear {:#x}\nd3ddump\nd3dtri {:#x} {cover}\nd3ddump", 0x8000_0000 | val << sh, 0x8000_0000 | (val ^ 0x5a) << sh);
                }
            }
            writeln!(s, "end");
        }
        // z near and past 0 and 1: which triangles are drawn and the z they write
        for z in ["z16", "z32"] {
            writeln!(s, "case zrange-{v}-{d}-{z}\n{dv} 32 2 1 {z}\nd3drs set 22 1\nd3drs set 7 1\nd3drs set 23 8");
            for m in [-1000, -10, -1, 0, 1, 500, 990, 998, 999, 1000, 1001, 1010, 2000] {
                writeln!(s, "d3dclear 0xff000000 z 500\nd3dtri 0xff00ff00 {cover} z {m}\nd3ddump\nd3dzdump");
            }
            writeln!(s, "end");
        }
        // 8 bpp targets (the HAL's are not measured yet)
        writeln!(s, "case dev8-{v}-{d}\n{dv} 8 4 2\nd3dclear 0xff102030\nd3ddump\nend");
        // D3DCLEAR_ZBUFFER without a z-buffer, whole and in a rectangle
        writeln!(s, "case clearnoz-{v}-{d}\n{dv} 32 4 2\nd3dclear 0xff102030\nd3dclear 0xff405060 z 500\nd3ddump\nd3dclear 0xff708090 z 0 1 0 3 2\nd3ddump\nend");
        // the z a triangle writes after each z function was set first (zrange's ALWAYS from the
        // start wrote other 32-bit z than zfunc's ALWAYS after LESSEQUAL)
        for z in ["z16", "z32"] {
            for first in 1..=8 {
                writeln!(s, "case zhist-{v}-{d}-{z}-{first}\n{dv} 32 2 1 {z}\nd3drs set 22 1\nd3drs set 7 1\nd3drs set 23 {first}");
                for (func, m) in [(first, 500), (first, 1), (8, 500), (8, 999), (4, 300), (8, 700), (first, 250)] {
                    writeln!(s, "d3dclear 0xff000000 z 1000\nd3drs set 23 {func}\nd3dtri 0xff00ff00 {cover} z {m}\nd3ddump\nd3dzdump");
                }
                writeln!(s, "end");
            }
        }
        // points and wireframe of random triangles inside the target, with LASTPIXEL on and off
        for mode in [1, 2] {
            for lp in [1, 0] {
                writeln!(s, "case fillmode-{v}-{d}-{mode}-{lp}\n{dv} 565 12 12\nd3drs set 22 1\nd3drs set 8 {mode}\nd3drs set 16 {lp}");
                for _ in 0..if d == "rgb" && v == 7 { 96 } else { 24 } {
                    let p: Vec<i64> = (0..6).map(|_| r.range(0, 191)).collect();
                    writeln!(s, "d3dclear 0\nd3dtri 0xffffffff {} {} {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3], p[4], p[5]);
                }
                writeln!(s, "end");
            }
        }
    }
    // lines (D3D7.STATES.OTHER: wireframe triangles draw their edges as lines): one line list of
    // one line at a time, every drawer, LASTPIXEL on and off, random ends inside the target
    // (its own generator, so that the cases before keep theirs)
    let mut rl = XorShift32::new(0x00d3_d711);
    for &(v, d) in DRAWERS {
        for lp in [1, 0] {
            writeln!(s, "case lines-{v}-{d}-{lp}\nd3d {v}\nd3ddev {d} 565 16 16\nd3drs set 22 1\nd3drs set 16 {lp}");
            for _ in 0..64 {
                let p: Vec<i64> = (0..4).map(|_| rl.range(0, 255)).collect();
                writeln!(s, "d3dclear 0\nd3dline 0xffffffff {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3]);
            }
            writeln!(s, "end");
        }
    }
    // lines whose exact minor coordinate lands on a whole pixel boundary at some column (an
    // exact tie of floor(v + 7/16)), lines of |dx| = |dy|, and lines ending on a corner of their
    // end pixel's diamond (x.5 with a whole y, or a whole x with y.5): the cases D3D7.LINE leaves
    // to D3D7.STATES.OTHER
    let mut rt = XorShift32::new(0x00d3_d712);
    let tie = |p: &[i64]| -> bool {
        let (dx, dy) = (p[2] - p[0], p[3] - p[1]);
        if dx.abs() == dy.abs() {
            return false;
        }
        let (a0, b0, a1, b1) = if dx.abs() > dy.abs() { (p[0], p[1], p[2], p[3]) } else { (p[1], p[0], p[3], p[2]) };
        let (da, db) = (a1 - a0, b1 - b0);
        (0..16).any(|m: i64| a0.min(a1) < m * 16 && m * 16 < a0.max(a1) && ((b0 + 7) * da + (m * 16 - a0) * db).rem_euclid(16 * da.abs()) == 0)
    };
    for &(v, d) in DRAWERS {
        for lp in [1, 0] {
            writeln!(s, "case lineties-{v}-{d}-{lp}\nd3d {v}\nd3ddev {d} 565 16 16\nd3drs set 22 1\nd3drs set 16 {lp}");
            let mut k = 0;
            while k < 48 {
                let p: Vec<i64> = (0..4).map(|_| rt.range(0, 255)).collect();
                if tie(&p) {
                    writeln!(s, "d3dclear 0\nd3dline 0xffffffff {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3]);
                    k += 1;
                }
            }
            writeln!(s, "end");
            writeln!(s, "case linediag-{v}-{d}-{lp}\nd3d {v}\nd3ddev {d} 565 16 16\nd3drs set 22 1\nd3drs set 16 {lp}");
            for _ in 0..32 {
                let (x, y) = (rt.range(0, 255), rt.range(0, 255));
                let n = rt.range(1, 255);
                let (sx, sy) = (if rt.next() & 1 == 0 { 1 } else { -1 }, if rt.next() & 1 == 0 { 1 } else { -1 });
                let (x1, y1) = (x + sx * n, y + sy * n);
                if (0..256).contains(&x1) && (0..256).contains(&y1) {
                    writeln!(s, "d3dclear 0\nd3dline 0xffffffff {x} {y} {x1} {y1}\nd3ddump");
                }
            }
            writeln!(s, "end");
            writeln!(s, "case linecorner-{v}-{d}-{lp}\nd3d {v}\nd3ddev {d} 565 16 16\nd3drs set 22 1\nd3drs set 16 {lp}");
            for _ in 0..32 {
                let (x0, y0) = (rt.range(0, 255), rt.range(0, 255));
                // the last end on a corner: x.5 and a whole y, or a whole x and y.5
                let (cx, cy) = (rt.range(0, 15) * 16, rt.range(0, 15) * 16);
                let (x1, y1) = if rt.next() & 1 == 0 { (cx + 8, cy) } else { (cx, cy + 8) };
                writeln!(s, "d3dclear 0\nd3dline 0xffffffff {x0} {y0} {x1} {y1}\nd3ddump");
            }
            writeln!(s, "end");
        }
    }
    // the same tie lines on every drawer: each as drawn, reversed, moved a whole pixel across
    // and down, and moved 1/16 along its major axis (which of those keep a tie's side)
    let mut rp = XorShift32::new(0x00d3_d713);
    let mut base = Vec::new();
    while base.len() < 24 {
        let p: Vec<i64> = (0..4).map(|_| rp.range(16, 223)).collect();
        if tie(&p) {
            base.push(p);
        }
    }
    for &(v, d) in DRAWERS {
        writeln!(s, "case linemoves-{v}-{d}\nd3d {v}\nd3ddev {d} 565 16 16\nd3drs set 22 1");
        for p in &base {
            let major_x = (p[2] - p[0]).abs() > (p[3] - p[1]).abs();
            let nudge = if major_x { [1, 0, 1, 0] } else { [0, 1, 0, 1] };
            let moved: [[i64; 4]; 5] = [
                [p[0], p[1], p[2], p[3]],
                [p[2], p[3], p[0], p[1]],
                [p[0] + 16, p[1], p[2] + 16, p[3]],
                [p[0], p[1] + 16, p[2], p[3] + 16],
                [p[0] + nudge[0], p[1] + nudge[1], p[2] + nudge[2], p[3] + nudge[3]],
            ];
            for q in moved {
                writeln!(s, "d3dclear 0\nd3dline 0xffffffff {} {} {} {}\nd3ddump", q[0], q[1], q[2], q[3]);
            }
        }
        writeln!(s, "end");
    }
    // long lines of minor/major slopes a/b through a tie (minor 9/16 past a whole pixel at a
    // whole major column) every b-th column, each drawn both ways, x-major on a 64 x 40 target
    // and y-major on a 40 x 64 one: how each of a line's ties settles in turn
    let mut rq = XorShift32::new(0x00d3_d714);
    for major_x in [true, false] {
        let (w, h, axis) = if major_x { (64, 40, "x") } else { (40, 64, "y") };
        writeln!(s, "case lineseq-rgb-{axis}\nd3d 7\nd3ddev rgb 565 {w} {h}\nd3drs set 22 1");
        for &(a, b) in &LONG_SLOPES {
            // (major, minor) in 16ths
            let (p, m0, m1) = loop {
                let p = (rq.range(20, 44) * 16, rq.range(4, 35) * 16 + 9);
                let (m0, m1) = (p.0 / b - rq.range(1, 15), (16 * 63 - p.0) / b - rq.range(1, 15));
                let inside = |q: i64| (0..16 * 40).contains(&q);
                if m0 > 0 && m1 > 0 && inside(p.1 - m0 * a) && inside(p.1 + m1 * a) {
                    break (p, m0, m1);
                }
            };
            let ends = [(p.0 - m0 * b, p.1 - m0 * a), (p.0 + m1 * b, p.1 + m1 * a)];
            for (e0, e1) in [(ends[0], ends[1]), (ends[1], ends[0])] {
                let ((x0, y0), (x1, y1)) = if major_x { (e0, e1) } else { ((e0.1, e0.0), (e1.1, e1.0)) };
                writeln!(s, "d3dclear 0\nd3dline 0xffffffff {x0} {y0} {x1} {y1}\nd3ddump");
            }
        }
        writeln!(s, "end");
    }
    // the HAL's last pixel: a line from far off to each 16th inside and on pixel (6, 6)'s diamond,
    // in eight directions (diamond exit leaves out a last pixel the HAL drew in about 3 of 10)
    for (k, (dx, dy)) in [(160, 37), (160, -37), (-160, 37), (-160, -37), (37, 160), (-37, 160), (37, -160), (-37, -160)].into_iter().enumerate() {
        writeln!(s, "case halend-{k}\nd3d 7\nd3ddev hal 565 12 12\nd3drs set 22 1");
        for ex in -8i64..=8 {
            for ey in -8i64..=8 {
                if ex.abs() + ey.abs() <= 8 {
                    let (x1, y1) = (96 + ex, 96 + ey);
                    writeln!(s, "d3dclear 0\nd3dline 0xffffffff {} {} {x1} {y1}\nd3ddump", x1 - dx / 2, y1 - dy / 2);
                }
            }
        }
        writeln!(s, "end");
    }
    // the HAL's first pixel: a line from each 16th inside and on pixel (6, 6)'s diamond out far,
    // in the same eight directions (which starts the diamond exit rule draws)
    for (k, (dx, dy)) in [(160, 37), (160, -37), (-160, 37), (-160, -37), (37, 160), (-37, 160), (37, -160), (-37, -160)].into_iter().enumerate() {
        writeln!(s, "case halstart-{k}\nd3d 7\nd3ddev hal 565 12 12\nd3drs set 22 1");
        for ex in -8i64..=8 {
            for ey in -8i64..=8 {
                if ex.abs() + ey.abs() <= 8 {
                    let (x0, y0) = (96 + ex, 96 + ey);
                    writeln!(s, "d3dclear 0\nd3dline 0xffffffff {x0} {y0} {} {}\nd3ddump", x0 + dx / 2, y0 + dy / 2);
                }
            }
        }
        writeln!(s, "end");
    }
    // D3D7.STATES.OTHER: lines with ends outside the target (inside the guard band), points and
    // wireframe of culled triangles and of several colors, ZENABLE without a z-buffer
    let mut ro = XorShift32::new(0x00d3_d70e);
    for &(v, d) in DRAWERS {
        let dv = format!("d3d {v}\nd3ddev {d} 565 12 12");
        writeln!(s, "case outlines-{v}-{d}\n{dv}\nd3drs set 22 1");
        for _ in 0..24 {
            let p: [i64; 4] = std::array::from_fn(|_| ro.range(0, 480).wrapping_sub(144));
            writeln!(s, "d3dclear 0\nd3dline 0xffffffff {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3]);
        }
        writeln!(s, "end");
        for mode in [1, 2] {
            writeln!(s, "case fillcull-{v}-{d}-{mode}\n{dv}\nd3drs set 8 {mode}");
            for _ in 0..8 {
                let p: [i64; 6] = std::array::from_fn(|_| ro.range(0, 191));
                writeln!(s, "d3dclear 0\nd3dtri 0xffffffff {} {} {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3], p[4], p[5]);
            }
            writeln!(s, "end");
            writeln!(s, "case fillgouraud-{v}-{d}-{mode}\n{dv}\nd3drs set 22 1\nd3drs set 8 {mode}");
            for _ in 0..8 {
                let p: [i64; 6] = std::array::from_fn(|_| ro.range(0, 191));
                writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff {} {} {} {} {} {}\nd3ddump", p[0], p[1], p[2], p[3], p[4], p[5]);
            }
            writeln!(s, "end");
        }
        writeln!(s, "case znoz-{v}-{d}\n{dv}\nd3drs set 22 1\nd3drs set 7 1\nd3dclear 0\nd3dtri 0xffffffff 8 8 160 24 40 170\nd3ddump\nend");
    }
    // D3D7.STATES.OTHER, D3D7.TRI.CLIP still asked: points and wireframe of triangles of no
    // area (a point, a line), the ramp device's points and wireframe of a triangle it clips, the
    // HAL's wireframe whose least x two vertices share and its points and edges of several
    // colours under specular and fog, the ramp device's fog stepping along a crossing span
    for &(v, d) in DRAWERS {
        let dv = format!("d3d {v}\nd3ddev {d} 565 12 12");
        for mode in [1, 2] {
            writeln!(s, "case zeroarea-{v}-{d}-{mode}\n{dv}\nd3drs set 22 1\nd3drs set 8 {mode}");
            for p in ["24 24 96 96 168 168", "40 100 40 100 40 100", "16 72 176 72 96 72", "96 8 96 8 96 170"] {
                writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff {p}\nd3ddump");
            }
            writeln!(s, "end");
        }
    }
    for mode in [1, 2] {
        writeln!(s, "case rampclip-2-ramp-{mode}\nd3d 2\nd3ddev ramp 565 12 12\nd3drs set 22 1\nd3drs set 8 {mode}");
        for p in ["-40 16 100 50 30 170", "20 -30 230 90 60 150", "8 8 170 40 -60 260", "-50 -50 250 -40 90 300"] {
            writeln!(s, "d3dclear 0\nd3dtri 0xffffffff {p}\nd3ddump");
        }
        writeln!(s, "end");
    }
    for d in ["hal", "tnl"] {
        writeln!(s, "case haltie-7-{d}\nd3d 7\nd3ddev {d} 565 12 12\nd3drs set 22 1\nd3drs set 8 2");
        for p in ["16 16 16 160 150 90", "16 160 150 90 16 16", "150 90 16 16 16 160", "40 20 170 60 40 150"] {
            writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff {p}\nd3ddump");
        }
        writeln!(s, "end");
        for mode in [1, 2] {
            writeln!(s, "case halspec-7-{d}-{mode}\nd3d 7\nd3ddev {d} 565 12 12\nd3drs set 22 1\nd3drs set 8 {mode}\nd3drs set 29 1");
            writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff 20 30 170 60 60 165 spec 0x00102030/0x00302010/0x00000040\nd3ddump");
            writeln!(s, "d3drs set 29 0\nd3drs set 28 1\nd3drs set 34 0xff4080c0\nd3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff 20 30 170 60 60 165 spec 0x00000000/0xff000000/0x80000000\nd3ddump\nend");
        }
    }
    writeln!(s, "case rampfog-2-ramp\nd3d 2\nd3ddev ramp 565 12 12\nd3drs set 22 1\nd3drs set 28 1\nd3drs set 34 0xff4080c0");
    for p in ["-40 16 220 50 30 170", "8 8 170 40 90 180"] {
        writeln!(s, "d3dclear 0\nd3dtri 0xffffffff {p} spec 0x00000000/0xff000000/0x80000000\nd3ddump");
    }
    writeln!(s, "end");
    // a vertex inside the guard band but past 2048 pixels (beyond 16-bit sixteenths)
    for (v, d) in [(7, "rgb"), (7, "mmx"), (3, "rgb")] {
        writeln!(s, "case bigtri-{v}-{d}\nd3d {v}\nd3ddev {d} 565 12 12\nd3drs set 22 1");
        for p in ["8 8 48000 40 40 170", "8 8 40 170 -40000 90", "16 16 170 30 90 50000"] {
            writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff {p}\nd3ddump");
        }
        writeln!(s, "end");
    }
    // D3D7.STATES.OTHER: states set past their enumeration (and SHADE_PHONG), then a triangle of
    // three colours; D3D7.TRI.CLIP: a vertex past the software guard band
    let bad: [(&str, &str); 12] = [
        ("cull0", "22 0"),
        ("cull4", "22 4"),
        ("fill0", "8 0"),
        ("fill4", "8 4"),
        ("shade0", "9 0"),
        ("shade3", "9 3"),
        ("shade4", "9 4"),
        ("afunc0", "15 1\nd3drs set 24 0x80\nd3drs set 25 0"),
        ("afunc9", "15 1\nd3drs set 24 0x80\nd3drs set 25 9"),
        ("src0", "27 1\nd3drs set 19 0"),
        ("src14", "27 1\nd3drs set 19 14"),
        ("dst14", "27 1\nd3drs set 20 14"),
    ];
    for &(v, d) in DRAWERS {
        for (name, set) in bad {
            writeln!(s, "case badstate-{v}-{d}-{name}\nd3d {v}\nd3ddev {d} 565 12 12\nd3drs set 22 1\nd3drs set {set}");
            writeln!(s, "d3dclear 0x00404040\nd3dtri 0xffff0000/0xff00ff00/0x800000ff 20 30 170 60 60 165\nd3ddump\nd3dclear 0x00404040\nd3dtri 0xffff0000/0xff00ff00/0x800000ff 20 30 60 165 170 60\nd3ddump\nend");
        }
    }
    for (v, d) in [(7, "rgb"), (7, "mmx"), (3, "rgb"), (2, "rgb"), (3, "mmx")] {
        writeln!(s, "case farvert-{v}-{d}\nd3d {v}\nd3ddev {d} 565 12 12\nd3drs set 22 1");
        for p in ["8 8 70000 40 40 170", "8 8 40 170 -70000 90", "16 16 170 30 90 66000", "-66000 -66000 170 30 90 170"] {
            writeln!(s, "d3dclear 0\nd3dtri 0xffff0000/0xff00ff00/0xff0000ff {p}\nd3ddump");
        }
        writeln!(s, "end");
    }
    s
}
