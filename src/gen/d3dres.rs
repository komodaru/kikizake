//! The d3d10res and d3d11res suites' case generators: format support, resource and view
//! descriptions, clears, updates, copies and reading back. Both suites are made by one function;
//! the Direct3D 10 one leaves out what Direct3D 10 lacks (unordered access views, structured
//! buffers, BC6H / BC7 textures).
use super::*;
use crate::model::dxgifmt::{format, Format, Num, FORMATS};

pub const CPU_W: u32 = 0x10000;
pub const CPU_R: u32 = 0x20000;
pub const SRV: u32 = 8;
pub const RTV: u32 = 0x20;
pub const DSV: u32 = 0x40;
pub const UAV: u32 = 0x80;

/// floats around what the conversions to each format round, clamp and keep
pub fn clear_floats() -> Vec<u32> {
    let mut v: Vec<u32> = [0.0f32, 1.0, -1.0, 0.5, 0.25, 0.75, 2.0, -2.0, 1.0 / 3.0, 0.1, 0.9, 1e-5, -1e-5, 65504.0, 65520.0, 65536.0, 1e10, -1e10, 255.0, 256.0, 127.5, 1.5, -0.5]
        .iter()
        .map(|f| f.to_bits())
        .collect();
    // the halfway points of 8 and 5 bit unorm and of 8 bit snorm, and one float to each side
    for k in [0u32, 1, 2, 63, 127, 128, 200, 254] {
        let h = (k as f32 + 0.5) / 255.0;
        v.extend([h.to_bits() - 1, h.to_bits(), h.to_bits() + 1]);
    }
    for k in [0u32, 7, 15, 30] {
        v.push(((k as f32 + 0.5) / 31.0).to_bits());
    }
    for k in [0u32, 1, 63, 126] {
        v.push(((k as f32 + 0.5) / 127.0).to_bits());
        v.push((-(k as f32 + 0.5) / 127.0).to_bits());
    }
    v.extend([0x8000_0000, 1, 0x8000_0001, 0x0080_0000, 0x3380_0000, 0x3880_0000, 0x387f_e000, 0x7f7f_ffff, 0xff7f_ffff, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0xffc0_0000, 0x7f80_0001, 0x4b80_0000, 0x4f80_0000, 0xcf00_0000]);
    v
}

/// a hex or decimal token
fn h(v: u32) -> String {
    if v < 10 {
        v.to_string()
    } else {
        format!("{v:#x}")
    }
}

/// `tex` arguments for a 2D texture
#[allow(clippy::too_many_arguments)]
fn tex2(slot: &str, w: u32, ht: u32, arr: u32, mips: u32, f: u32, usage: u32, bind: u32, cpu: u32, misc: u32, ms: (u32, u32)) -> String {
    format!("tex {slot} 2 {w} {ht} {arr} {mips} {} {usage} {} {} {} {} {}", h(f), h(bind), h(cpu), h(misc), ms.0, h(ms.1))
}

/// the bytes `read` prints of mip `m` of a w x h x d texture
pub fn read_line(slot: u32, sub: u32, f: &Format, w: u32, ht: u32, d: u32, m: u32) -> String {
    let (w, ht, d) = ((w >> m).max(1), (ht >> m).max(1), (d >> m).max(1));
    format!("read {slot} {sub} {} {} {d}", f.row_bytes(w), f.rows(ht))
}

/// the formats the clear and copy cases use (every one that is not video, typeless or depth)
fn plain(f: &Format, d3d11: bool) -> bool {
    !f.video() && f.num != Num::Typeless && !matches!(f.id, 20 | 21 | 22 | 40 | 45 | 46 | 47 | 55 | 66 | 68 | 69) && (d3d11 || !(94..=99).contains(&f.id))
}

pub fn d3d11res() -> String {
    suite(true)
}
pub fn d3d10res() -> String {
    suite(false)
}

fn suite(d3d11: bool) -> String {
    let name = if d3d11 { "d3d11res" } else { "d3d10res" };
    let mut s = format!("# gpu\n# kikizake suite {name}: Direct3D {} resources (SPEC.md). Made by `kikizake gen {name}`.\n", if d3d11 { 11 } else { 10 });
    let dev = if d3d11 { "device11" } else { "device10" };
    // format support at every feature level
    let levels: &[(&str, &str)] = if d3d11 {
        &[("device11 0x9100", "9_1"), ("device11 0x9200", "9_2"), ("device11 0x9300", "9_3"), ("device11 0xa000", "10_0"), ("device11 0xa100", "10_1"), ("device11", "11_0"), ("device11 0xb100", "11_1")]
    } else {
        &[("device10", "10"), ("device10_1 0x9100", "9_1"), ("device10_1 0x9200", "9_2"), ("device10_1 0x9300", "9_3"), ("device10_1 0xa000", "10_0"), ("device10_1 0xa100", "10_1")]
    };
    for (line, lv) in levels {
        writeln!(s, "case fmt-{lv}\n{line}").unwrap();
        for f in (0..=133).chain([191, 0x8000_0000, 0xffff_ffff]) {
            writeln!(s, "fmt {}", h(f)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // every format as a 2D texture with each kind of binding (and none)
    let binds = [0, SRV, RTV, DSV, SRV | RTV, SRV | DSV, RTV | DSV, 1, 2, 4];
    let binds11 = [UAV, SRV | UAV, SRV | RTV | UAV];
    for f in (0..=133u32).filter(|f| format(*f).is_some() || *f == 0) {
        writeln!(s, "case tex-format-{f}\n{dev}").unwrap();
        let fm = format(f);
        let (w, ht) = fm.map_or((8, 8), |fm| (4 * fm.bw.max(2), 4 * fm.bh.max(2)));
        for b in binds.iter().chain(if d3d11 { &binds11[..] } else { &[] }) {
            writeln!(s, "{}", tex2("0", w, ht, 1, 1, f, 0, *b, 0, 0, (1, 0))).unwrap();
        }
        // mipmapped, an array, a cube, mip generation, staging, dynamic, immutable
        writeln!(s, "{}", tex2("0", w, ht, 1, 0, f, 0, SRV, 0, 0, (1, 0))).unwrap();
        writeln!(s, "{}", tex2("0", w, w, 6, 1, f, 0, SRV, 0, 4, (1, 0))).unwrap();
        writeln!(s, "{}", tex2("0", w, ht, 1, 0, f, 0, SRV | RTV, 0, 1, (1, 0))).unwrap();
        writeln!(s, "{}", tex2("0", w, ht, 1, 1, f, 3, 0, CPU_R | CPU_W, 0, (1, 0))).unwrap();
        writeln!(s, "{}", tex2("0", w, ht, 1, 1, f, 2, SRV, CPU_W, 0, (1, 0))).unwrap();
        if let Some(fm) = fm {
            writeln!(s, "{} init 7 {} 0", tex2("0", w, ht, 1, 1, f, 1, SRV, 0, 0, (1, 0)), fm.row_bytes(w)).unwrap();
            // 1D and 3D
            writeln!(s, "tex 0 1 {w} 1 1 1 {} 0 8 0 0 1 0", h(f)).unwrap();
            writeln!(s, "tex 0 3 {w} {ht} 4 1 {} 0 8 0 0 1 0", h(f)).unwrap();
            writeln!(s, "tex 0 3 {w} {ht} 4 1 {} 0 {} 0 0 1 0", h(f), h(RTV)).unwrap();
            // multisampled render targets
            for c in [2, 4, 8] {
                writeln!(s, "{}", tex2("0", w, ht, 1, 1, f, 0, RTV, 0, 0, (c, 0))).unwrap();
            }
        }
        writeln!(s, "end").unwrap();
    }
    // the 2D description's words through values of interest, on R8G8B8A8_UNORM and
    // R24G8_TYPELESS (depth)
    let fmts2 = [(28u32, SRV | RTV), (44, DSV | SRV), (71, SRV)];
    let words: Vec<(&str, Vec<u32>)> = vec![
        ("w", vec![0, 1, 2, 3, 4, 7, 8, 4096, 8192, 16384, 16385, 0x8000_0000, 0xffff_ffff]),
        ("h", vec![0, 1, 2, 3, 4, 5, 8, 4096, 16384, 16385, 0xffff_ffff]),
        ("arr", vec![0, 1, 2, 5, 6, 7, 12, 512, 2048, 2049, 0xffff_ffff]),
        ("mips", vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 15, 0xffff_ffff]),
        ("usage", vec![0, 1, 2, 3, 4, 0xffff_ffff]),
        ("bind", vec![0, 1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x800, 0x28, 0x48, 0x60, 0xa8, 0xffff_ffff]),
        ("cpu", vec![0, CPU_W, CPU_R, CPU_W | CPU_R, 1, 0x40000, 0xffff_ffff]),
        ("misc", vec![0, 1, 2, 4, 5, 8, 0x10, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x10000, 0xffff_ffff]),
        ("count", vec![0, 1, 2, 3, 4, 8, 16, 32]),
        ("quality", vec![0, 1, 2, 0xffff_fffe, 0xffff_ffff]),
    ];
    for (f, bind) in fmts2 {
        // (w h arr mips usage bind cpu misc count quality) of the base: 8x8, 1, 1, default
        for (wn, vals) in &words {
            for base_usage in [0u32, 2, 3] {
                writeln!(s, "case tex2-f{f}-{wn}-u{base_usage}\n{dev}").unwrap();
                for v in vals {
                    let mut d = [8u32, 8, 1, 1, base_usage, if base_usage == 3 { 0 } else { bind }, [0, 0, CPU_W, CPU_R][base_usage as usize], 0, 1, 0];
                    let at = words.iter().position(|w| w.0 == *wn).unwrap();
                    d[at] = *v;
                    if (d[0] as u64) * (d[1] as u64) * (d[2] as u64) > 1 << 26 {
                        // (no allocations of gigabytes: check only)
                        writeln!(s, "{}", tex2("check", d[0], d[1], d[2], d[3], f, d[4], d[5], d[6], d[7], (d[8], d[9]))).unwrap();
                    } else {
                        writeln!(s, "{}", tex2("0", d[0], d[1], d[2], d[3], f, d[4], d[5], d[6], d[7], (d[8], d[9]))).unwrap();
                    }
                }
                writeln!(s, "end").unwrap();
            }
        }
    }
    // random 1D, 2D and 3D descriptions near the valid ones, created and checked
    let mut r = XorShift32::new(if d3d11 { 0xd3d1_1e55 } else { 0xd3d1_0e55 });
    let some_formats: Vec<u32> = FORMATS.iter().filter(|f| !f.video()).map(|f| f.id).collect();
    for i in 0..300 {
        writeln!(s, "case tex-random-{i}\n{dev}").unwrap();
        for _ in 0..r.range(4, 16) {
            let dim = r.range(1, 3) as u32;
            let f = if r.chance(8) { r.next() % 135 } else { *r.pick(&some_formats) };
            let mut d = [*r.pick(&[1u32, 2, 3, 4, 5, 8, 16, 33, 64]), *r.pick(&[1u32, 2, 3, 4, 8, 13, 16]), *r.pick(&[1u32, 1, 2, 3, 6, 8, 12]), *r.pick(&[0u32, 1, 1, 2, 3, 4, 5, 9])];
            if dim == 1 {
                d[1] = 1;
            }
            let usage = *r.pick(&[0u32, 0, 0, 1, 2, 3]);
            let mut bind = [0u32, SRV, SRV | RTV, DSV, SRV | DSV, RTV, SRV | UAV][r.range(0, if d3d11 { 6 } else { 5 }) as usize];
            let mut cpu = [0, 0, CPU_W, CPU_R][usage as usize];
            let mut misc = *r.pick(&[0u32, 0, 0, 0, 1, 4, 5]);
            if r.chance(5) {
                bind ^= 1 << r.range(0, 11);
            }
            if r.chance(6) {
                cpu ^= *r.pick(&[CPU_W, CPU_R]);
            }
            if r.chance(6) {
                misc ^= 1 << r.range(0, 12);
            }
            let ms = if dim == 2 && r.chance(5) { (*r.pick(&[2u32, 4, 8]), *r.pick(&[0u32, 0, 1, 0xffff_ffff])) } else { (1, 0) };
            let slot = if r.chance(6) { "check".to_string() } else { r.range(0, 15).to_string() };
            let init = usage == 1 || r.chance(5);
            let mut l = format!("tex {slot} {dim} {} {} {} {} {} {usage} {} {} {} {} {}", d[0], d[1], d[2], d[3], h(f), h(bind), h(cpu), h(misc), ms.0, h(ms.1));
            if init {
                let fm = format(f);
                let rp = fm.map_or(64, |fm| fm.row_bytes(d[0]));
                let sp = fm.map_or(64, |fm| fm.row_bytes(d[0]) * fm.rows(d[1]));
                write!(l, " init {} {rp} {sp}", r.next() % 1000).unwrap();
            }
            writeln!(s, "{l}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // buffers
    let bwords: Vec<(&str, Vec<u32>)> = vec![
        ("size", vec![0, 1, 3, 4, 15, 16, 17, 64, 65536, 1 << 20, 0x0800_0000, 0x8000_0000, 0xffff_ffff]),
        ("usage", vec![0, 1, 2, 3, 4]),
        ("bind", vec![0, 1, 2, 3, 4, 5, 8, 9, 0x10, 0x11, 0x20, 0x40, 0x80, 0x88, 0x84, 0x14, 0x400, 0xffff_ffff]),
        ("cpu", vec![0, CPU_W, CPU_R, CPU_W | CPU_R, 1, 0xffff_ffff]),
        ("misc", vec![0, 1, 2, 4, 0x10, 0x20, 0x40, 0x60, 0x80, 0x100, 0x200, 0x800, 0xffff_ffff]),
        ("stride", vec![0, 1, 2, 3, 4, 8, 12, 16, 64, 2048, 2049, 0xffff_ffff]),
    ];
    let bases: &[(&str, [u32; 6])] = &[
        ("vb", [64, 0, 1, 0, 0, 0]),
        ("cb", [64, 0, 4, 0, 0, 0]),
        ("cbdyn", [64, 2, 4, CPU_W, 0, 0]),
        ("srv", [64, 0, 8, 0, 0, 0]),
        ("staging", [64, 3, 0, CPU_R, 0, 0]),
        ("raw", [64, 0, 8 | UAV, 0, 0x20, 0]),
        ("struct", [64, 0, 8 | UAV, 0, 0x40, 16]),
    ];
    for (bn, base) in bases {
        if !d3d11 && matches!(*bn, "raw" | "struct") {
            continue;
        }
        for (at, (wn, vals)) in bwords.iter().enumerate() {
            writeln!(s, "case buf-{bn}-{wn}\n{dev}").unwrap();
            for v in vals {
                let mut d = *base;
                d[at] = *v;
                let slot = if d[0] > 1 << 20 { "check" } else { "0" };
                let init = if d[1] == 1 { " init 5" } else { "" };
                writeln!(s, "buf {slot} {} {} {} {} {} {}{init}", h(d[0]), d[1], h(d[2]), h(d[3]), h(d[4]), h(d[5])).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // views: every kind with no description, then descriptions, on textures of each dimension
    // and on buffers
    let kinds: &[&str] = if d3d11 { &["srv", "rtv", "dsv", "uav"] } else { &["srv", "rtv", "dsv"] };
    let all_bind = |depth: bool| if depth { SRV | DSV } else if d3d11 { SRV | RTV | UAV } else { SRV | RTV };
    let resources: Vec<(&str, String, bool)> = vec![
        ("t1", "tex 0 1 16 1 1 0 0x1c 0 B 0 0 1 0".into(), false),
        ("t1a", "tex 0 1 16 1 4 0 0x1c 0 B 0 0 1 0".into(), false),
        ("t2", "tex 0 2 16 8 1 0 0x1c 0 B 0 0 1 0".into(), false),
        ("t2a", "tex 0 2 16 8 4 0 0x1c 0 B 0 0 1 0".into(), false),
        ("t2tl", "tex 0 2 16 8 4 0 0x1b 0 B 0 0 1 0".into(), false),
        ("t2ms", "tex 0 2 16 8 1 1 0x1c 0 0x28 0 0 4 0".into(), false),
        ("t2msa", "tex 0 2 16 8 3 1 0x1c 0 0x28 0 0 4 0".into(), false),
        ("cube", "tex 0 2 8 8 6 0 0x1c 0 0x28 0 4 1 0".into(), false),
        ("cubea", "tex 0 2 8 8 12 0 0x1c 0 0x28 0 4 1 0".into(), false),
        ("t3", "tex 0 3 16 8 4 0 0x1c 0 B 0 0 1 0".into(), false),
        ("d2", "tex 0 2 16 8 1 1 0x2d 0 0x40 0 0 1 0".into(), true),
        ("d2tl", "tex 0 2 16 8 4 0 0x2c 0 0x48 0 0 1 0".into(), true),
        ("d1", "tex 0 1 16 1 3 0 0x2c 0 0x48 0 0 1 0".into(), true),
        ("d2ms", "tex 0 2 16 8 2 1 0x2c 0 0x48 0 0 4 0".into(), true),
        ("dcube", "tex 0 2 8 8 6 0 0x2c 0 0x48 0 4 1 0".into(), true),
        ("b", "buf 0 256 0 B 0 0 0".into(), false),
        ("braw", "buf 0 256 0 B 0 0x20 0".into(), false),
        ("bstruct", "buf 0 256 0 B 0 0x40 16".into(), false),
    ];
    let mut r = XorShift32::new(if d3d11 { 0x7e55_0011 } else { 0x7e55_0010 });
    for (rn, line, depth) in &resources {
        if !d3d11 && matches!(*rn, "braw" | "bstruct") {
            continue;
        }
        let line = line.replace(" B ", &format!(" {} ", h(all_bind(*depth))));
        writeln!(s, "case view-{rn}-nodesc\n{dev}\n{line}").unwrap();
        for (i, k) in kinds.iter().enumerate() {
            writeln!(s, "{k} {i} 0 nodesc").unwrap();
        }
        writeln!(s, "end").unwrap();
        for k in kinds {
            writeln!(s, "case view-{rn}-{k}\n{dev}\n{line}").unwrap();
            let fmts: &[u32] = if *depth { &[0, 0x2c, 0x2d, 0x2e, 0x2f, 0x28, 0x29, 0x37, 0x1c] } else { &[0, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x29, 0x57, 0x2a, 0x2d, 0x47] };
            let dims: u32 = match *k {
                "srv" => 12,
                "rtv" => 9,
                "dsv" => 7,
                _ => 9,
            };
            for n in 0..60 {
                let f = if n < fmts.len() { fmts[n] } else { *r.pick(fmts) };
                let dim = if n < dims as usize { n as u32 } else { r.range(0, dims as i64 - 1) as u32 };
                let mut w: Vec<u32> = vec![f, dim];
                if *k == "dsv" && d3d11 {
                    w.push(*r.pick(&[0u32, 0, 0, 1, 2, 3, 4]));
                }
                for _ in 0..4 {
                    w.push(*r.pick(&[0u32, 0, 1, 1, 2, 3, 4, 5, 6, 7, 8, 16, 17, 0xffff_ffff]));
                }
                let ws: Vec<String> = w.iter().map(|v| h(*v)).collect();
                writeln!(s, "{k} {} 0 {}{}", n % 16, if r.chance(8) { "check " } else { "" }, ws.join(" ")).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // clears of every renderable format, read back
    let floats = clear_floats();
    for f in FORMATS.iter().filter(|f| plain(f, d3d11) && !f.block()) {
        for (ci, chunk) in floats.chunks(20).enumerate() {
            writeln!(s, "case clear-{}-{ci}\n{dev}", f.id).unwrap();
            writeln!(s, "{}", tex2("0", 2, 1, 1, 1, f.id, 0, RTV, 0, 0, (1, 0))).unwrap();
            writeln!(s, "rtv 0 0 nodesc").unwrap();
            for (j, v) in chunk.iter().enumerate() {
                // (each channel its own value: r the value, g, b, a others from the list)
                let o = |k: usize| floats[(ci * 20 + j + k * 17) % floats.len()];
                writeln!(s, "clear 0 {} {} {} {}", h(*v), h(o(1)), h(o(2)), h(o(3))).unwrap();
                writeln!(s, "{}", read_line(0, 0, f, 2, 1, 1, 0)).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    // depth stencil clears
    let depths = [0.0f32.to_bits(), 1.0f32.to_bits(), 0.5f32.to_bits(), 0.25f32.to_bits(), (1.0f32 / 3.0).to_bits(), 2.0f32.to_bits(), (-1.0f32).to_bits(), 0x7fc0_0000, 0x7f80_0000, 0xff80_0000, 0x8000_0000, 1, (0.5f32 / 65535.0).to_bits(), (1.5f32 / 65535.0).to_bits(), (0.5f32 / 16777215.0).to_bits(), (100.5f32 / 16777215.0).to_bits(), 0x3f7f_ffff];
    for (f, tl) in [(55u32, 53u32), (45, 44), (40, 39), (20, 19)] {
        let fm = format(f).unwrap();
        writeln!(s, "case cleards-{f}\n{dev}").unwrap();
        writeln!(s, "{}", tex2("0", 2, 1, 1, 1, tl, 0, DSV, 0, 0, (1, 0))).unwrap();
        writeln!(s, "dsv 0 0 {}{}", h(f), if d3d11 { " 3 0" } else { " 1 0" }).unwrap();
        for (i, d) in depths.iter().enumerate() {
            let flags = [1u32, 2, 3, 0][i % 4];
            writeln!(s, "cleards 0 {flags} {} {}", h(*d), h([0u32, 1, 0x7f, 0x80, 0xff][i % 5])).unwrap();
            writeln!(s, "{}", read_line(0, 0, fm, 2, 1, 1, 0)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // unordered access view clears
    if d3d11 {
        let uavs = [2u32, 3, 4, 10, 11, 12, 13, 14, 16, 17, 18, 24, 25, 26, 28, 30, 31, 32, 34, 35, 36, 37, 38, 41, 42, 43, 49, 50, 51, 52, 54, 56, 57, 58, 59, 61, 62, 63, 64, 65, 87];
        let words = [0u32, 1, 0x7f, 0x80, 0xff, 0x100, 0x3ff, 0x7fff, 0x8000, 0xffff, 0x1_0000, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 0x3f80_0000, 0xbf80_0000, 0x3f00_0000, 0x7fc0_0000];
        for f in uavs {
            let fm = format(f).unwrap();
            writeln!(s, "case clearuav-{f}\n{dev}").unwrap();
            writeln!(s, "{}", tex2("0", 2, 1, 1, 1, f, 0, UAV, 0, 0, (1, 0))).unwrap();
            writeln!(s, "uav 0 0 nodesc").unwrap();
            for (i, w) in words.iter().enumerate() {
                let o = |k: usize| words[(i + k * 5) % words.len()];
                for t in ["u", "f"] {
                    writeln!(s, "clearuav 0 {t} {} {} {} {}", h(*w), h(o(1)), h(o(2)), h(o(3))).unwrap();
                    writeln!(s, "{}", read_line(0, 0, fm, 2, 1, 1, 0)).unwrap();
                }
            }
            writeln!(s, "end").unwrap();
        }
        writeln!(s, "case clearuav-raw\n{dev}\nbuf 0 64 0 0x88 0 0x20 0\nuav 0 0 0x27 1 0 16 1").unwrap();
        writeln!(s, "clearuav 0 u 1 2 3 4\nread 0 0 64 1 1\nclearuav 0 f 0x3f800000 2 3 4\nread 0 0 64 1 1\nend").unwrap();
    }
    // initial data, updates and copies of every plain and block format, read back mip by mip
    for f in FORMATS.iter().filter(|f| plain(f, d3d11) || f.block() && f.num != Num::Typeless && (d3d11 || f.id < 94)) {
        let (w, ht) = (8 * f.bw.min(2), 4 * f.bh);
        let rp = f.row_bytes(w);
        writeln!(s, "case data-{}\n{dev}", f.id).unwrap();
        writeln!(s, "{} init {} {rp} 0", tex2("0", w, ht, 2, 0, f.id, 0, SRV, 0, 0, (1, 0)), f.id).unwrap();
        let mips = 32 - w.max(ht).leading_zeros();
        for sub in 0..mips * 2 {
            writeln!(s, "{}", read_line(0, sub, f, w, ht, 1, sub % mips)).unwrap();
        }
        // update a subresource, then a box of it (block formats: block aligned)
        writeln!(s, "update 0 1 {} {} 0", f.id + 1000, rp + 4).unwrap();
        writeln!(s, "{}", read_line(0, 1, f, w, ht, 1, 1)).unwrap();
        writeln!(s, "update 0 0 {} {rp} 0 {} {} 0 {} {} 1", f.id + 2000, f.bw, f.bh, 3 * f.bw, 3 * f.bh).unwrap();
        writeln!(s, "{}", read_line(0, 0, f, w, ht, 1, 0)).unwrap();
        // copies: the whole resource, a region, one subresource to another
        writeln!(s, "{}", tex2("1", w, ht, 2, 0, f.id, 0, SRV, 0, 0, (1, 0))).unwrap();
        writeln!(s, "copy 1 0").unwrap();
        writeln!(s, "{}", read_line(1, 0, f, w, ht, 1, 0)).unwrap();
        writeln!(s, "copysub 1 0 {} 0 0 0 {} 0 0 0 {} {} 1", f.bw, mips, 2 * f.bw, f.bh * 2).unwrap();
        writeln!(s, "{}", read_line(1, 0, f, w, ht, 1, 0)).unwrap();
        writeln!(s, "copysub 1 {} 0 0 0 0 1", mips + 1).unwrap();
        writeln!(s, "{}", read_line(1, mips + 1, f, w, ht, 1, 1)).unwrap();
        writeln!(s, "end").unwrap();
    }
    // copies between formats: the same typeless group, other groups of the same size, block
    // formats and the uncompressed formats of their block's size
    let pairs = [(28u32, 29u32), (28, 31), (28, 87), (28, 42), (2, 3), (2, 10), (41, 42), (41, 28), (71, 17), (71, 34), (77, 3), (77, 2), (80, 18), (83, 4), (98, 3), (24, 28), (67, 42), (26, 42), (61, 62), (54, 56), (87, 91), (87, 88)];
    for (a, b) in pairs {
        let (fa, fb) = (format(a).unwrap(), format(b).unwrap());
        if !d3d11 && (a >= 94 || b >= 94) {
            continue;
        }
        let wa = 8 * fa.bw.min(2);
        let ha = 4 * fa.bh;
        // (the other texture's size: the same in elements)
        let (wb, hb) = (wa / fa.bw * fb.bw, ha / fa.bh * fb.bh);
        writeln!(s, "case copyfmt-{a}-{b}\n{dev}").unwrap();
        writeln!(s, "{} init {} {} 0", tex2("0", wa, ha, 1, 1, a, 0, SRV, 0, 0, (1, 0)), a * 7 + b, fa.row_bytes(wa)).unwrap();
        writeln!(s, "{}", tex2("1", wb, hb, 1, 1, b, 0, SRV, 0, 0, (1, 0))).unwrap();
        writeln!(s, "copy 1 0\n{}", read_line(1, 0, fb, wb, hb, 1, 0)).unwrap();
        writeln!(s, "{}", tex2("2", wb, hb, 1, 1, b, 0, SRV, 0, 0, (1, 0))).unwrap();
        writeln!(s, "copysub 2 0 0 0 0 0 0\n{}", read_line(2, 0, fb, wb, hb, 1, 0)).unwrap();
        writeln!(s, "end").unwrap();
    }
    // buffers: initial data, updates (a range), copies (a range), read back
    writeln!(s, "case data-buffer\n{dev}\nbuf 0 64 0 8 0 0 0 init 9\nread 0 0 64 1 1").unwrap();
    writeln!(s, "update 0 0 10 0 0 4 0 0 20 1 1\nread 0 0 64 1 1\nupdate 0 0 11 0 0\nread 0 0 64 1 1").unwrap();
    writeln!(s, "buf 1 64 0 8 0 0 0\ncopysub 1 0 8 0 0 0 0 0 0 0 16 1 1\nread 1 0 64 1 1\ncopy 1 0\nread 1 0 64 1 1\nend").unwrap();
    // 3D textures: initial data with slice pitches, updates of a box
    for f in [28u32, 2, 61] {
        let fm = format(f).unwrap();
        let rp = fm.row_bytes(4);
        writeln!(s, "case data3d-{f}\n{dev}\ntex 0 3 4 4 4 0 {} 0 8 0 0 1 0 init {} {rp} {}", h(f), f + 50, rp * 4 + 16).unwrap();
        for m in 0..3 {
            writeln!(s, "{}", read_line(0, m, fm, 4, 4, 4, m)).unwrap();
        }
        writeln!(s, "update 0 0 {} {rp} {} 1 1 1 3 3 3\n{}", f + 60, rp * 4, read_line(0, 0, fm, 4, 4, 4, 0)).unwrap();
        writeln!(s, "end").unwrap();
    }
    // multisampled render targets: cleared, resolved, read back
    for f in [28u32, 10, 2, 29, 87] {
        let fm = format(f).unwrap();
        writeln!(s, "case resolve-{f}\n{dev}").unwrap();
        writeln!(s, "{}\nrtv 0 0 nodesc", tex2("0", 4, 2, 1, 1, f, 0, RTV, 0, 0, (4, 0))).unwrap();
        writeln!(s, "{}", tex2("1", 4, 2, 1, 1, f, 0, SRV, 0, 0, (1, 0))).unwrap();
        for c in [[0x3f80_0000u32, 0x3f00_0000, 0, 0x3e80_0000], [0x3e4c_cccd, 0x3f4c_cccd, 0x3c80_8081, 0x3f80_0000]] {
            writeln!(s, "clear 0 {} {} {} {}\nresolve 1 0 0 0 {}\n{}", h(c[0]), h(c[1]), h(c[2]), h(c[3]), h(f), read_line(1, 0, fm, 4, 2, 1, 0)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // the lines without a device, without a resource, of the wrong view kind
    writeln!(s, "case nodev\nfmt 0x1c\n{}\nbuf 0 16 0 1 0 0 0\nend", tex2("0", 4, 4, 1, 1, 28, 0, SRV, 0, 0, (1, 0))).unwrap();
    writeln!(s, "case nores\n{dev}\nsrv 0 3 nodesc\nread 5 0 4 1 1\ncopy 0 1\nupdate 2 0 1 4 0\nend").unwrap();
    writeln!(s, "case wrongview\n{dev}\n{}\nsrv 0 0 nodesc\nclear 0 0 0 0 0\ncleards 0 1 0 0\nclear 1 0 0 0 0\nend", tex2("0", 4, 4, 1, 1, 28, 0, SRV | RTV, 0, 0, (1, 0))).unwrap();
    // where the sizes stop: checks only (nothing allocated), at 11_0, 10_0 and Direct3D 10
    let devs: &[&str] = if d3d11 { &["device11", "device11 0xa000"] } else { &["device10"] };
    for (i, line) in devs.iter().enumerate() {
        writeln!(s, "case big-{i}\n{line}").unwrap();
        for v in [8192u32, 8193, 16384, 16385, 16386, 20000, 32767, 32768, 32769, 65536, 0x10_0000, 0x1000_0000, 0x7fff_ffff] {
            writeln!(s, "{}\n{}", tex2("check", v, 1, 1, 1, 28, 0, SRV, 0, 0, (1, 0)), tex2("check", 1, v, 1, 1, 28, 0, SRV, 0, 0, (1, 0))).unwrap();
            writeln!(s, "tex check 1 {} 1 1 1 0x1c 0 8 0 0 1 0", h(v)).unwrap();
        }
        for v in [2048u32, 2049, 4096, 8192, 16384, 16385, 0x10_0000] {
            writeln!(s, "tex check 3 {} 1 1 1 0x1c 0 8 0 0 1 0\ntex check 3 1 {} 1 1 0x1c 0 8 0 0 1 0\ntex check 3 1 1 {} 1 0x1c 0 8 0 0 1 0", h(v), h(v), h(v)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // BUFFEREX and buffer render target views of a plain buffer: formats, ranges, flags
    writeln!(s, "case view-b-bufex\n{dev}\nbuf 0 256 0 {} 0 0 0", h(if d3d11 { SRV | RTV | UAV } else { SRV | RTV })).unwrap();
    for f in [0x1cu32, 0x2a, 0x29, 0x1d] {
        for (a, n) in [(0u32, 8u32), (2, 62), (0, 64), (2, 8), (0, 63), (63, 1), (64, 1), (0, 65), (0, 0xffff_ffff)] {
            for fl in [0u32, 1, 6] {
                writeln!(s, "srv 0 0 {} 0xb {} {} {fl}", h(f), h(a), h(n)).unwrap();
            }
            writeln!(s, "srv 0 0 {} 1 {} {}\nrtv 0 0 {} 1 {} {}", h(f), h(a), h(n), h(f), h(a), h(n)).unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    // the sRGB curve near each half step: probes a little to each side of the exact point
    writeln!(s, "case clear-srgb-curve\n{dev}\n{}\nrtv 0 0 nodesc", tex2("0", 1, 1, 1, 1, 29, 0, RTV, 0, 0, (1, 0))).unwrap();
    for k in 0..255u32 {
        let c = (k as f64 + 0.5) / 255.0;
        let x = if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        let p: Vec<String> = [-4e-4, -2e-4, -1e-4, 0.0, 1e-4, 2e-4].iter().map(|d| h(((x * (1.0 + d)) as f32).to_bits())).collect();
        writeln!(s, "clear 0 {} {} {} 0\nread 0 0 4 1 1\nclear 0 {} {} {} 0\nread 0 0 4 1 1", p[0], p[1], p[2], p[3], p[4], p[5]).unwrap();
    }
    writeln!(s, "end").unwrap();
    // sizes past 2^31; an XR_BIAS target cleared; updates, copies, resolves and reads the
    // documentation calls invalid (each its own case: the model stops at the first)
    writeln!(s, "case probe-size\n{dev}\n{}\n{}\n{}\nend", tex2("check", 0x8000_0000, 8, 1, 1, 28, 0, SRV, 0, 0, (1, 0)), tex2("check", 8, 0xffff_ffff, 1, 1, 28, 0, SRV, 0, 0, (1, 0)), tex2("check", 0xffff_ffff, 8, 1, 1, 28, 0, SRV, 0, 0, (1, 0))).unwrap();
    writeln!(s, "case probe-xrbias\n{dev}\n{}\nrtv 0 0 nodesc\nclear 0 0x3f800000 0 0x3f000000 0x3f800000\nread 0 0 16 1 1\nclear 0 0x3f400000 0x3e800000 0x3d000000 0\nread 0 0 16 1 1\nend", tex2("0", 4, 1, 1, 1, 89, 0, RTV, 0, 0, (1, 0))).unwrap();
    let t44 = |slot: u32, bind: u32, count: u32| tex2(&slot.to_string(), 4, 4, 1, 1, 28, 0, bind, 0, 0, (count, 0));
    for (name, body) in [
        ("update-empty", "update 0 0 5 16 0 1 0 0 1 4 1\nupdate 0 0 5 16 0 0 2 0 4 2 1\nread 0 0 16 4 1"),
        ("update-past", "update 0 0 5 16 0 0 0 0 8 4 1\nread 0 0 16 4 1"),
        ("update-sub", "update 0 1 5 16 0\nread 0 0 16 4 1"),
        ("copysub-past", "copysub 1 0 3 3 0 0 0 0 0 0 4 4 1\nread 1 0 16 4 1"),
        ("resolve-single", "resolve 1 0 0 0 28\nread 1 0 16 4 1"),
    ] {
        writeln!(s, "case probe-{name}\n{dev}\n{}\n{}\n{body}\nend", t44(0, SRV, 1), t44(1, SRV, 1)).unwrap();
    }
    writeln!(s, "case probe-read-ms\n{dev}\n{}\nrtv 0 0 nodesc\nclear 0 0x3f800000 0 0 0x3f800000\nread 0 0 16 4 1\nend", t44(0, RTV, 4)).unwrap();
    // (an immutable texture made without initial data crashes the x86 WARP: give it some)
    let s = s.lines().map(immutable_init).collect::<Vec<_>>().join("\n") + "\n";
    isolate_x86_crashes(&s)
}

/// the texture lines one of which ends the x86 executor (golden d3d11res-32 and d3d10res-32 stop
/// in tex2-f28-usage-u0: usage 0xffffffff, or immutable with a render target, depth or UAV bind),
/// out of their cases into cases of their own at the end, so that the rest is answered (each
/// refused, by the model: the lines stay as a size 0 texture, refused the same way)
fn isolate_x86_crashes(s: &str) -> String {
    let kind = |l: &str| -> Option<usize> {
        let t: Vec<&str> = l.split(' ').collect();
        if t.len() < 14 || t[0] != "tex" {
            return None;
        }
        let num = |s: &str| if let Some(x) = s.strip_prefix("0x") { u32::from_str_radix(x, 16).ok() } else { s.parse().ok() };
        let (usage, bind) = (num(t[8])?, num(t[9])?);
        if usage == 0xffff_ffff && bind != 0 {
            Some(0)
        } else if usage == 1 && bind & (0x20 | 0x40 | 0x80) != 0 {
            Some(1)
        } else {
            None
        }
    };
    let mut out = String::new();
    let mut moved: [Vec<(String, String)>; 2] = [vec![], vec![]];
    let mut dev = String::new();
    let mut prev = "";
    for l in s.lines() {
        if prev.starts_with("case ") {
            dev = l.to_string();
        }
        prev = l;
        match kind(l) {
            Some(k) => {
                moved[k].push((dev.clone(), l.to_string()));
                // (in its place a size-0 texture: refused as it is, the slot emptied as by it)
                let mut t: Vec<&str> = l.split(' ').take(14).collect();
                t[3] = "0";
                out += &t.join(" ");
                out += "\n";
            }
            None => {
                out += l;
                out += "\n";
            }
        }
    }
    for (k, name) in [(0, "x86-usageff"), (1, "x86-immutable-bound")] {
        let mut devs: Vec<&String> = moved[k].iter().map(|m| &m.0).collect();
        devs.dedup();
        devs.sort();
        devs.dedup();
        for (i, d) in devs.iter().enumerate() {
            writeln!(out, "case {name}-{i}\n{d}").unwrap();
            for (_, l) in moved[k].iter().filter(|m| &m.0 == *d) {
                writeln!(out, "{l}").unwrap();
            }
            writeln!(out, "end").unwrap();
        }
    }
    out
}

/// a texture line of usage IMMUTABLE without initial data, with some (pitches of its width)
fn immutable_init(l: &str) -> String {
    let t: Vec<&str> = l.split(' ').collect();
    if t.len() != 14 || t[0] != "tex" || t[8] != "1" {
        return l.to_string();
    }
    let num = |s: &str| if let Some(x) = s.strip_prefix("0x") { u32::from_str_radix(x, 16).ok() } else { s.parse().ok() };
    let (Some(w), Some(ht), Some(f)) = (num(t[3]), num(t[4]), num(t[7])) else { return l.to_string() };
    let rb = format(f).map_or(16, |f| f.row_bytes(w.min(1 << 16)) as u64);
    let rows = format(f).map_or(1, |f| f.rows(ht.min(1 << 16)) as u64);
    format!("{l} init 1 {rb} {}", rb * rows)
}
