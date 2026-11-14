//! The d3d7 suite's case generator: Direct3D 1, 2, 3 and 7 through DirectDraw.
use super::*;

pub const DEVS: &[&str] = &["rgb", "hal", "tnl", "ref", "mmx", "ramp", "null"];
pub const VERSIONS: &[u32] = &[1, 2, 3, 7];

/// a triangle line: color(s) and three vertices in 16ths of a pixel
fn tri(r: &mut XorShift32, w: i64, h: i64, gouraud: bool) -> String {
    let mut c = format!("{:#x}", 0xff00_0000 | (r.next() & 0xffffff));
    if gouraud {
        for _ in 0..2 {
            write!(c, "/{:#x}", 0xff00_0000 | (r.next() & 0xffffff)).unwrap();
        }
    }
    let mut l = format!("d3dtri {c}");
    for _ in 0..3 {
        // (mostly on a quarter-pixel grid, some anywhere in 16ths, some past the target)
        let (x, y) = if r.chance(4) { (r.range(-40, w * 16 + 40), r.range(-40, h * 16 + 40)) } else { (r.range(-1, w * 4 + 1) * 4, r.range(-1, h * 4 + 1) * 4) };
        write!(l, " {x} {y}").unwrap();
    }
    l
}

pub fn d3d7() -> String {
    let mut s = String::from("# kikizake suite d3d7: Direct3D 1, 2, 3 and 7 (immediate mode through DirectDraw; SPEC.md). Made by `kikizake gen d3d7`.\n");
    let mut r = XorShift32::new(0x00d3_d007);
    for v in VERSIONS {
        writeln!(s, "case enum-{v}\nd3d {v}\nd3denum caps\nend").unwrap();
    }
    // which devices each version makes, on which targets
    for v in VERSIONS {
        for d in DEVS {
            for (f, z) in [("565", ""), ("32", ""), ("565", " z16"), ("32", " z16"), ("32", " z24"), ("32", " z32")] {
                writeln!(s, "case dev-{v}-{d}-{f}{}\nd3d {v}\nd3ddev {d} {f} 8 4{z}\nend", z.replace(' ', "-")).unwrap();
            }
        }
        for f in ["555", "24", "8"] {
            writeln!(s, "case dev-{v}-rgb-{f}\nd3d {v}\nd3ddev rgb {f} 8 4\nend").unwrap();
        }
    }
    // render states: the defaults, without and with a z-buffer, then what a set leaves
    for v in [2, 3, 7] {
        for z in ["", " z16"] {
            writeln!(s, "case rs-{v}{}\nd3d {v}\nd3ddev rgb 565 8 4{z}", z.replace(' ', "-")).unwrap();
            for st in 0..=160 {
                writeln!(s, "d3drs get {st}").unwrap();
            }
            writeln!(s, "end").unwrap();
        }
        writeln!(s, "case rsset-{v}\nd3d {v}\nd3ddev rgb 565 8 4").unwrap();
        for st in [7, 8, 9, 14, 15, 16, 19, 20, 22, 23, 24, 25, 26, 27, 28, 29, 34, 36, 37, 38, 48, 52, 53, 56, 128, 136, 137, 139, 140, 141, 145, 146] {
            let val = *r.pick(&[0u32, 1, 2, 3, 4, 0x80, 0xffff_ffff]);
            writeln!(s, "d3drs set {st} {val:#x}\nd3drs get {st}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // Clear: every value of each channel into 1-pixel rectangles of each target format
    for v in [2, 3, 7] {
        for f in ["565", "555", "32", "24"] {
            writeln!(s, "case clear-{v}-{f}\nd3d {v}\nd3ddev rgb {f} 64 12").unwrap();
            for ch in 0..3u32 {
                for val in 0..256u32 {
                    let (x, y) = (val % 64, ch * 4 + val / 64);
                    writeln!(s, "d3dclear {:#x} {x} {y} {} {}", 0xff00_0000 | val << (8 * (2 - ch)), x + 1, y + 1).unwrap();
                }
            }
            writeln!(s, "d3ddump\nend").unwrap();
        }
        writeln!(s, "case clearalpha-{v}\nd3d {v}\nd3ddev rgb 32 16 1\nd3dfill 3").unwrap();
        for x in 0..16u32 {
            writeln!(s, "d3dclear {:#x} {x} 0 {} 1", (x * 17) << 24 | 0x123456, x + 1).unwrap();
        }
        writeln!(s, "d3ddump\nend").unwrap();
        writeln!(s, "case clearrects-{v}\nd3d {v}\nd3ddev rgb 32 16 8\nd3dfill 1\nd3dclear 0xff102030 2 2 6 5\nd3dclear 0xff405060 -2 -2 3 3\nd3dclear 0xff708090 14 6 20 12\nd3dclear 0xffa0b0c0 5 5 5 7\nd3dclear 0xffd0e0f0 7 3 4 6\nd3ddump\nend").unwrap();
    }
    // the z-buffer's values for z = 0, 1/1000, ..., 1 (version 7)
    for z in ["z16", "z24", "z32"] {
        writeln!(s, "case clearz-{z}\nd3d 7\nd3ddev rgb 32 64 16 {z}").unwrap();
        for m in 0..=1000u32 {
            let (x, y) = (m % 64, m / 64);
            writeln!(s, "d3dclear 0xff000000 z {m} {x} {y} {} {}", x + 1, y + 1).unwrap();
        }
        writeln!(s, "d3dzdump\nend").unwrap();
    }
    // triangles: which pixels each covers (flat color, culling off)
    for i in 0..160 {
        let v = *r.pick(&[7, 7, 7, 3, 2]);
        let f = *r.pick(&["32", "32", "565", "555", "24"]);
        let (w, h) = (r.range(4, 24), r.range(3, 16));
        writeln!(s, "case tri-{i}\nd3d {v}\nd3ddev rgb {f} {w} {h}\nd3drs set 22 1\nd3dclear 0xff000000").unwrap();
        for _ in 0..r.range(1, 3) {
            writeln!(s, "{}", tri(&mut r, w, h, false)).unwrap();
        }
        writeln!(s, "d3ddump\nend").unwrap();
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
        writeln!(s, "case shade-{i}\nd3d {v}\nd3ddev rgb {f} 16 12\nd3drs set 22 1{extra}\nd3dclear 0xff000000\n{}\nd3ddump\nend", tri(&mut r, 16, 12, true)).unwrap();
    }
    // culling: each mode and winding
    for v in [2, 3, 7] {
        for cull in ["", "\nd3drs set 22 1", "\nd3drs set 22 2", "\nd3drs set 22 3"] {
            writeln!(s, "case cull-{v}-{}\nd3d {v}\nd3ddev rgb 32 8 8{cull}\nd3dclear 0xff000000\nd3dtri 0xffff0000 0 0 128 0 0 128\nd3dtri 0xff00ff00 128 128 128 0 0 128\nd3ddump\nend", cull.len()).unwrap();
        }
    }
    // alpha blending (SRCALPHA, INVSRCALPHA) of a flat color over known pixels, each alpha
    for a in (0..=255u32).step_by(5) {
        let v = *r.pick(&[7, 3]);
        writeln!(s, "case blend-{a}\nd3d {v}\nd3ddev rgb 32 16 8\nd3drs set 22 1\nd3dfill {}\nd3drs set 27 1\nd3drs set 19 5\nd3drs set 20 6\nd3dtri {:#x} -16 -16 600 -16 -16 600\nd3ddump\nend", a + 1, a << 24 | (r.next() & 0xffffff)).unwrap();
    }
    // the z test: a nearer triangle over a farther one and the other way round
    for v in [3, 7] {
        for z in ["z16", "z32"] {
            writeln!(s, "case ztest-{v}-{z}\nd3d {v}\nd3ddev rgb 32 16 8 {z}\nd3drs set 22 1\nd3drs set 7 1\nd3dclear 0xff000000 z 1000\nd3dtri 0xffff0000 0 0 256 0 0 128 z 600\nd3dtri 0xff00ff00 0 0 256 128 0 128 z 300\nd3dtri 0xff0000ff 0 0 256 0 256 128 z 450\nd3ddump\nd3dzdump\nend").unwrap();
        }
    }
    // without a device
    writeln!(s, "case nodev\nd3denum\nd3ddev rgb 32 8 4\nd3drs get 7\nd3dclear 0\nd3dtri 0 0 0 1 1 2 2\nd3ddump\nend").unwrap();
    writeln!(s, "case nodevice\nd3d 7\nd3drs get 7\nd3dclear 0\nd3dtri 0 0 0 1 1 2 2\nd3ddump\nend").unwrap();
    // version 1 has no render states, Clear or DrawPrimitive here
    writeln!(s, "case v1\nd3d 1\nd3ddev rgb 32 8 4\nd3drs get 7\nd3dclear 0\nd3ddump\nend").unwrap();
    // states past the documented ones last (Windows may not check them)
    for v in [2, 3, 7] {
        writeln!(s, "case rs-high-{v}\nd3d {v}\nd3ddev rgb 565 8 4").unwrap();
        for st in 161..=255 {
            writeln!(s, "d3drs get {st}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    s
}
