//! The d3d12 and dxgi suites' case generators.
use super::*;
use crate::model::dxgi_format::{block, channels, DEPTH};

/// a D3D12_RESOURCE_DESC as the executor's 11 tokens
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Desc {
    pub dim: u32,
    pub align: u32,
    pub w: u32,
    pub h: u32,
    pub d: u32,
    pub mips: u32,
    pub fmt: u32,
    pub samples: u32,
    pub quality: u32,
    pub layout: u32,
    pub flags: u32,
}

impl Desc {
    pub fn words(&self) -> [u32; 11] {
        [self.dim, self.align, self.w, self.h, self.d, self.mips, self.fmt, self.samples, self.quality, self.layout, self.flags]
    }
    pub fn from_words(w: &[u32]) -> Desc {
        Desc { dim: w[0], align: w[1], w: w[2], h: w[3], d: w[4], mips: w[5], fmt: w[6], samples: w[7], quality: w[8], layout: w[9], flags: w[10] }
    }
    pub fn buffer(w: u32) -> Desc {
        Desc { dim: 1, align: 0, w, h: 1, d: 1, mips: 1, fmt: 0, samples: 1, quality: 0, layout: 1, flags: 0 }
    }
    pub fn tex(dim: u32, w: u32, h: u32, d: u32, mips: u32, fmt: u32) -> Desc {
        Desc { dim, align: 0, w, h, d, mips, fmt, samples: 1, quality: 0, layout: 0, flags: 0 }
    }
}

fn hexw(v: u32) -> String {
    if v < 10 {
        v.to_string()
    } else {
        format!("{v:#x}")
    }
}

fn desc_str(d: &Desc) -> String {
    d.words().iter().map(|v| hexw(*v)).collect::<Vec<_>>().join(" ")
}

/// how many subresources a description has (planes not counted), for the generators
fn subresources(d: &Desc) -> u32 {
    let mips = if d.mips == 0 {
        let m = if d.dim == 4 { d.w.max(d.h).max(d.d) } else if d.dim == 3 { d.w.max(d.h) } else { d.w };
        32 - m.max(1).leading_zeros()
    } else {
        d.mips
    };
    mips * if d.dim == 4 { 1 } else { d.d.max(1) }
}

/// every format value the footprint cases go through: all of dxgiformat.h's
fn all_formats() -> Vec<u32> {
    (0..=132).chain([189, 190, 191, 192, 0xffff_ffff]).collect()
}

/// float values around what the conversions decide
pub fn clear_floats() -> Vec<u32> {
    let mut v: Vec<f32> = vec![0.0, 1.0, 0.5, 0.25, 0.75, -0.5, -1.0, 2.0, 0.1, 0.3, 1.0 / 3.0, 0.7, 0.9, 0.001, -0.001, 0.0031308, 0.04045, 65504.0, 65520.0, 1.0e5, -1.0e5, 127.5, 255.5, 2.5, 3.5, -2.5, 1.5, 4294967296.0, -2147483648.0, 2147483648.0, 1.0e-5, 6.1e-5, 1.0e-7];
    // (the rounding points of 4, 5, 6, 8, 10 and 16 bit unsigned and of 8 bit signed values)
    for n in [4u32, 5, 6, 8, 10, 16] {
        let m = ((1u32 << n) - 1) as f32;
        v.push(0.5 / m);
        v.push(1.5 / m);
        v.push((m - 0.5) / m);
    }
    v.push(0.5 / 127.0);
    v.push(-0.5 / 127.0);
    v.push(-126.5 / 127.0);
    let mut bits: Vec<u32> = v.into_iter().map(f32::to_bits).collect();
    bits.extend([0x8000_0000, 0x7fc0_0000, 0xffc0_0000, 0x7f80_0000, 0xff80_0000, 0x7f7f_ffff, 0xff7f_ffff, 1, 0x8000_0001, 0x0080_0000, 0x3f80_0001, 0x3f7f_ffff]);
    bits
}

pub fn d3d12() -> String {
    let mut s = String::from("# gpu\n# kikizake suite d3d12: Direct3D 12 resources (SPEC.md). Made by `kikizake gen d3d12`.\n");
    let fp = |s: &mut String, first: u32, count: u32, base: u32, d: &Desc| {
        writeln!(s, "footprint12 {first} {count} {} {}", hexw(base), desc_str(d)).unwrap();
    };
    // footprints of every format: a 2D array with its mip chain, a 1D and a 3D texture
    for f in all_formats() {
        writeln!(s, "case fp-format-{f}\ndevice12").unwrap();
        let (bw, bh) = block(f).map_or((1, 1), |b| (b.1, b.2));
        let (w, h) = if bw > 1 { (16, 12) } else { (13, 7) };
        let _ = bh;
        fp(&mut s, 0, 8, 0, &Desc::tex(3, w, h, 2, 0, f));
        fp(&mut s, 0, 6, 0, &Desc::tex(2, 37, 1, 1, 0, f));
        fp(&mut s, 0, 4, 0, &Desc::tex(4, 9, 5, 3, 0, f));
        writeln!(s, "end").unwrap();
    }
    // block compressed and two-texel formats at sizes that are not multiples of their blocks
    for f in [68, 71, 74, 77, 80, 83, 95, 98, 107] {
        writeln!(s, "case fp-blocks-{f}\ndevice12").unwrap();
        for (w, h) in [(1, 1), (2, 2), (3, 5), (5, 3), (4, 4), (13, 7), (17, 9)] {
            fp(&mut s, 0, 1, 0, &Desc::tex(3, w, h, 1, 1, f));
            fp(&mut s, 0, 3, 0, &Desc::tex(3, w * 4, h * 4, 1, 3, f));
        }
        writeln!(s, "end").unwrap();
    }
    // buffers
    writeln!(s, "case fp-buffer\ndevice12").unwrap();
    for w in [1, 2, 255, 256, 257, 511, 512, 513, 4096, 65535, 65536, 65537, 0x10_0000, 0x7fff_ffff, 0xffff_ffff, 0] {
        fp(&mut s, 0, 1, 0, &Desc::buffer(w));
    }
    for base in [1, 256, 511, 512, 0x1234] {
        fp(&mut s, 0, 1, base, &Desc::buffer(1000));
    }
    writeln!(s, "end").unwrap();
    // base offsets, subresource ranges
    writeln!(s, "case fp-base\ndevice12").unwrap();
    let chain = Desc::tex(3, 100, 60, 3, 0, 28);
    for base in [0, 1, 255, 256, 511, 512, 513, 1000, 0x1_0000, 0x1234_5678] {
        fp(&mut s, 0, 7, base, &chain);
    }
    writeln!(s, "end\ncase fp-range\ndevice12").unwrap();
    for (first, count) in [(0, 0), (0, 1), (3, 2), (6, 1), (7, 9), (14, 4), (20, 1), (21, 1), (25, 1), (0, 16)] {
        fp(&mut s, first, count, 0, &chain);
    }
    writeln!(s, "end").unwrap();
    // descriptions the documentation calls invalid
    writeln!(s, "case fp-invalid\ndevice12").unwrap();
    let base = Desc::tex(3, 64, 32, 1, 1, 28);
    let mut bad = vec![];
    for dim in [0, 5, 0xffff_ffff] {
        bad.push(Desc { dim, ..base });
    }
    for (w, h) in [(0, 32), (64, 0), (16384, 16384), (16385, 1), (1, 16385)] {
        bad.push(Desc { w, h, ..base });
    }
    for mips in [7, 8, 15, 0xffff] {
        bad.push(Desc { mips, ..base });
    }
    bad.push(Desc { d: 0, ..base });
    bad.push(Desc { d: 2048, ..base });
    bad.push(Desc { d: 2049, ..base });
    bad.push(Desc { samples: 0, ..base });
    bad.push(Desc { samples: 4, ..base });
    bad.push(Desc { layout: 1, ..base });
    bad.push(Desc { layout: 4, ..base });
    bad.push(Desc { align: 1, ..base });
    bad.push(Desc { h: 2, ..Desc::tex(2, 64, 1, 1, 1, 28) });
    bad.push(Desc { mips: 2, ..Desc::buffer(1024) });
    bad.push(Desc { fmt: 28, ..Desc::buffer(1024) });
    bad.push(Desc { layout: 0, ..Desc::buffer(1024) });
    bad.push(Desc { h: 2, ..Desc::buffer(1024) });
    bad.push(Desc { d: 2049, ..Desc::tex(4, 4, 4, 2048, 1, 28) });
    for d in bad {
        fp(&mut s, 0, 1, 0, &d);
    }
    writeln!(s, "end").unwrap();
    // random textures: dimension, size, array, mips, format
    let mut r = XorShift32::new(0xd3d1_2f00);
    let fmts: Vec<u32> = (1..=115).filter(|f| block(*f).is_some()).collect();
    for i in 0..200 {
        writeln!(s, "case fp-random-{i}\ndevice12").unwrap();
        for _ in 0..r.range(1, 4) {
            let dim = r.range(2, 4) as u32;
            let big = |r: &mut XorShift32| if r.chance(4) { r.range(1, 3000) as u32 } else { r.range(1, 70) as u32 };
            let w = big(&mut r);
            let h = if dim == 2 { 1 } else { big(&mut r) };
            let d = if dim == 4 { r.range(1, 20) as u32 } else { r.range(1, 4) as u32 };
            let f = *r.pick(&fmts);
            let mut desc = Desc::tex(dim, w, h, d, r.range(0, 4) as u32, f);
            let (_, bw, bh) = block(f).unwrap();
            if bw > 1 || bh > 1 {
                desc.w = desc.w.div_ceil(bw) * bw;
                if dim != 2 {
                    desc.h = desc.h.div_ceil(bh) * bh;
                }
            }
            let n = subresources(&desc);
            let first = if n > 1 && r.chance(3) { r.range(0, n as i64 - 1) as u32 } else { 0 };
            let count = (n - first.min(n)).min(16);
            let base = if r.chance(3) { r.next() & 0xffff } else { 0 };
            fp(&mut s, first, count, base, &desc);
        }
        writeln!(s, "end").unwrap();
    }
    // allocation sizes: buffers (the documentation's rule), then textures (the driver's)
    writeln!(s, "case alloc-buffer\ndevice12").unwrap();
    for w in [1, 4096, 65535, 65536, 65537, 131072, 1000000, 0x7fff_ffff, 0] {
        writeln!(s, "alloc12 {}", desc_str(&Desc::buffer(w))).unwrap();
    }
    for align in [65536, 4096, 4194304, 1, 32768] {
        writeln!(s, "alloc12 {}", desc_str(&Desc { align, ..Desc::buffer(100) })).unwrap();
    }
    writeln!(s, "end").unwrap();
    let mut r = XorShift32::new(0xd3d1_2a00);
    for (i, f) in [28u32, 2, 10, 61, 71, 98, 40, 45, 87].into_iter().enumerate() {
        writeln!(s, "case alloc-texture-{f}\ndevice12").unwrap();
        for _ in 0..30 {
            let dim = *r.pick(&[3, 3, 3, 2, 4]);
            let w = r.range(1, 1024) as u32 & !3;
            let h = if dim == 2 { 1 } else { r.range(1, 1024) as u32 & !3 };
            let mut d = Desc::tex(dim, w.max(4), h.max(if dim == 2 { 1 } else { 4 }), r.range(1, 3) as u32, r.range(0, 3) as u32, f);
            if f == 40 || f == 45 {
                d.dim = 3;
                d.h = d.h.max(4);
                d.flags = 2;
            } else if r.chance(3) && f != 71 && f != 98 {
                d.flags = 1;
            }
            d.align = *r.pick(&[0, 0, 65536, 4096, 4194304]);
            writeln!(s, "alloc12 {}", desc_str(&d)).unwrap();
        }
        let _ = i;
        writeln!(s, "end").unwrap();
    }
    // CreateCommittedResource's validation: each field of a buffer and of a texture through
    // values of interest, on each heap type
    let states = [0u32, 0x1, 0x2, 0x4, 0x8, 0x10, 0x40, 0x80, 0x400, 0x800, 0xac3, 0x1000];
    let sweep = |s: &mut String, name: &str, heap: u32, state: u32, base: Desc, at: usize, vals: &[u32]| {
        writeln!(s, "case create-{name}\ndevice12").unwrap();
        for v in vals {
            let mut w = base.words();
            w[at] = *v;
            writeln!(s, "create12 {heap} {} {}", hexw(state), desc_str(&Desc::from_words(&w))).unwrap();
        }
        writeln!(s, "end").unwrap();
    };
    let pools: [Vec<u32>; 11] = [
        (0..=5).chain([0xffff_ffff]).collect(),
        vec![0, 1, 512, 4096, 65536, 0x10_0000, 4194304, 0x80_0000],
        vec![0, 1, 3, 4, 13, 256, 2048, 2049, 16384, 16385, 65536],
        vec![0, 1, 2, 3, 4, 2048, 2049, 16384, 16385],
        vec![0, 1, 2, 6, 512, 2048, 2049, 4096],
        (0..=16).chain([0xffff]).collect(),
        all_formats(),
        vec![0, 1, 2, 3, 4, 8, 16, 32],
        vec![0, 1, 2, 0xffff_ffff],
        (0..=4).chain([0xffff_ffff]).collect(),
        (0..=9).map(|b| 1u32 << b).chain([0, 3, 5, 6, 9, 10, 0x400, 0x8000_0000]).collect(),
    ];
    let names = ["dim", "align", "width", "height", "depth", "mips", "format", "samples", "quality", "layout", "flags"];
    let buf = Desc::buffer(1024);
    for (heap, state) in [(1, 0), (2, 0xac3), (3, 0x400)] {
        for at in 0..11 {
            sweep(&mut s, &format!("buffer-h{heap}-{}", names[at]), heap, state, buf, at, &pools[at]);
        }
        sweep(&mut s, &format!("buffer-h{heap}-state"), heap, 0, buf, 10, &[0]);
        writeln!(s, "case create-buffer-h{heap}-states\ndevice12").unwrap();
        for st in states {
            writeln!(s, "create12 {heap} {} {}", hexw(st), desc_str(&buf)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    for heap in [0, 4, 5] {
        writeln!(s, "case create-heap-{heap}\ndevice12\ncreate12 {heap} 0 {}\nend", desc_str(&buf)).unwrap();
    }
    for (name, base) in [("tex2d", Desc::tex(3, 64, 32, 2, 3, 28)), ("tex2drt", Desc { flags: 1, ..Desc::tex(3, 64, 32, 1, 1, 28) }), ("tex1d", Desc::tex(2, 64, 1, 2, 3, 28)), ("tex3d", Desc::tex(4, 16, 8, 4, 3, 28)), ("depth", Desc { flags: 2, ..Desc::tex(3, 64, 32, 1, 1, 40) }), ("msaa", Desc { flags: 1, samples: 4, ..Desc::tex(3, 64, 32, 1, 1, 28) })] {
        for at in 0..11 {
            sweep(&mut s, &format!("{name}-{}", names[at]), 1, 0, base, at, &pools[at]);
        }
        writeln!(s, "case create-{name}-heaps\ndevice12").unwrap();
        for (heap, st) in [(2, 0xac3), (3, 0x400), (1, 0x4), (1, 0x10)] {
            writeln!(s, "create12 {heap} {} {}", hexw(st), desc_str(&base)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // a render target cleared in each format, read back; every value in every channel
    let floats = clear_floats();
    for f in (0..=191).filter(|f| channels(*f).is_some()) {
        for (c, chunk) in floats.chunks(25).enumerate() {
            writeln!(s, "case clear-{f}-{c}\ndevice12").unwrap();
            for (i, _) in chunk.iter().enumerate() {
                let k = c * 25 + i;
                let px: Vec<String> = (0..4).map(|j| format!("{:#x}", floats[(k + j * 7) % floats.len()])).collect();
                writeln!(s, "clear12 {f} {}", px.join(" ")).unwrap();
            }
            writeln!(s, "end").unwrap();
        }
    }
    for f in DEPTH {
        writeln!(s, "case cleardsv-{f}\ndevice12").unwrap();
        for (i, v) in floats.iter().enumerate() {
            writeln!(s, "cleardsv12 {f} {v:#x} {}", [0, 1, 0x80, 0xff, 0x1ff][i % 5]).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // (not D32_FLOAT, which removes the device: the executor crashed after it, golden 936cec8)
    writeln!(s, "case clear-unsupported\ndevice12").unwrap();
    for f in [0, 71, 192] {
        writeln!(s, "clear12 {f} 0 0 0 0").unwrap();
    }
    writeln!(s, "cleardsv12 28 0 0\nend").unwrap();
    // what the sweeps above leave open: 64KB layouts of 2D arrays and mip chains, the planar
    // video formats without mips, GPU upload heaps, 1D render target sizes
    writeln!(s, "case create-split\ndevice12").unwrap();
    for (d, mips) in [(2, 1), (1, 3), (1, 0), (4, 1)] {
        for layout in [2, 3] {
            writeln!(s, "create12 1 0 {}", desc_str(&Desc { layout, ..Desc::tex(3, 64, 32, d, mips, 28) })).unwrap();
        }
    }
    for f in [103, 104, 105, 106, 107, 108, 109, 110, 100, 102] {
        for (w, h, d, mips) in [(64, 32, 1, 1), (63, 32, 1, 1), (64, 31, 1, 1), (64, 32, 2, 1), (64, 32, 1, 2)] {
            writeln!(s, "create12 1 0 {}", desc_str(&Desc::tex(3, w, h, d, mips, f))).unwrap();
        }
        writeln!(s, "footprint12 0 2 0 {}", desc_str(&Desc::tex(3, 64, 32, 1, 1, f))).unwrap();
    }
    writeln!(s, "end\ncase create-gpuupload\ndevice12").unwrap();
    for st in [0u32, 1, 4, 8, 0x400, 0xac3] {
        writeln!(s, "create12 5 {} {}", hexw(st), desc_str(&buf)).unwrap();
    }
    for f in [0u32, 1, 4] {
        writeln!(s, "create12 5 0 {}", desc_str(&Desc { flags: f, ..Desc::tex(3, 64, 32, 1, 1, 28) })).unwrap();
    }
    writeln!(s, "create12 5 0 {}", desc_str(&Desc::tex(4, 16, 16, 4, 1, 28))).unwrap();
    writeln!(s, "end\ncase alloc-rt1d\ndevice12").unwrap();
    for (w, a, m, f) in [(944, 3, 3, 2), (944, 1, 1, 2), (944, 3, 1, 2), (944, 1, 3, 2), (4096, 1, 1, 28), (16384, 1, 1, 28), (100, 1, 1, 28), (1024, 2, 1, 10), (4000, 1, 1, 2), (2000, 4, 2, 87)] {
        writeln!(s, "alloc12 {}", desc_str(&Desc { flags: 1, ..Desc::tex(2, w, 1, a, m, f) })).unwrap();
        writeln!(s, "alloc12 {}", desc_str(&Desc::tex(2, w, 1, a, m, f))).unwrap();
    }
    // multisampled, 64KB-swizzled and planar textures (sizes of the driver's own layout)
    // (one description a case, so that one the model lacks leaves the others compared)
    writeln!(s, "end").unwrap();
    let mut o = String::new();
    for (w, h, n, f) in [(64u32, 64u32, 2u32, 28u32), (64, 64, 4, 28), (256, 100, 8, 28), (1000, 700, 4, 10), (300, 200, 16, 2), (128, 128, 4, 45)] {
        writeln!(o, "alloc12 {}", desc_str(&Desc { samples: n, flags: if f == 45 { 2 } else { 1 }, ..Desc::tex(3, w, h, 1, 1, f) })).unwrap();
    }
    for (w, h, m, f) in [(256u32, 256u32, 1u32, 28u32), (1000, 300, 1, 2), (64, 64, 1, 71)] {
        writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, ..Desc::tex(3, w, h, 1, m, f) })).unwrap();
    }
    for (w, h, m, f) in [(256u32, 256u32, 1u32, 20u32), (100, 60, 3, 20), (64, 64, 1, 103), (640, 480, 1, 103), (128, 64, 1, 44)] {
        writeln!(o, "alloc12 {}", desc_str(&Desc { flags: if f == 103 { 0 } else { 2 }, ..Desc::tex(3, w, h, 1, m, f) })).unwrap();
    }
    // (P010, an NV12 array, standard swizzle, 64KB swizzle with mips, samples at 64KB)
    writeln!(o, "alloc12 {}", desc_str(&Desc::tex(3, 640, 480, 1, 1, 104))).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc::tex(3, 640, 480, 3, 1, 103))).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 3, ..Desc::tex(3, 256, 256, 1, 1, 28) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, ..Desc::tex(3, 256, 256, 1, 3, 28) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { samples: 4, align: 65536, flags: 1, ..Desc::tex(3, 64, 64, 1, 1, 28) })).unwrap();
    // (420_OPAQUE, standard swizzle with an array and BC1)
    writeln!(o, "alloc12 {}", desc_str(&Desc::tex(3, 64, 32, 1, 1, 106))).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 3, ..Desc::tex(3, 300, 200, 2, 2, 28) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 3, ..Desc::tex(3, 512, 256, 1, 0, 71) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, ..Desc::tex(4, 64, 64, 64, 1, 28) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 3, ..Desc::tex(4, 100, 50, 20, 2, 10) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, flags: 2, ..Desc::tex(3, 256, 256, 1, 1, 45) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, flags: 2, ..Desc::tex(3, 256, 256, 1, 1, 20) })).unwrap();
    writeln!(o, "alloc12 {}", desc_str(&Desc { layout: 2, ..Desc::tex(3, 256, 256, 1, 1, 103) })).unwrap();
    for (i, l) in o.lines().enumerate() {
        writeln!(s, "case alloc-other-{i}\ndevice12\n{l}\nend").unwrap();
    }
    // (standard swizzle of the two-plane formats, no target)
    for (i, f) in [45u32, 20, 103].into_iter().enumerate() {
        writeln!(s, "case alloc-std-planar-{i}\ndevice12\nalloc12 {}\ncreate12 1 0 {}\nend", desc_str(&Desc { layout: 3, ..Desc::tex(3, 256, 256, 1, 1, f) }), desc_str(&Desc { layout: 3, ..Desc::tex(3, 256, 256, 1, 1, f) })).unwrap();
    }
    writeln!(s, "case nodev\nfootprint12 0 1 0 {}\nalloc12 {}\ncreate12 1 0 {}\nclear12 28 0 0 0 0\nend", desc_str(&buf), desc_str(&buf), desc_str(&buf)).unwrap();
    // CheckFeatureSupport: every format's support, planes and multisample quality levels
    for f in (0u32..=191).chain([0xffff_ffff]) {
        writeln!(s, "case fsup-{f}\ndevice12\nfeature12 {}\nend\ncase msq-{f}\ndevice12", hexw(f)).unwrap();
        for n in [0u32, 1, 2, 4, 8, 16, 32, 3] {
            writeln!(s, "msq12 {} {n}", hexw(f)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    writeln!(s, "case incr\ndevice12\nincr12\nend").unwrap();
    // the stencil planes' footprints, and one mip level of R10G10B10_XR_BIAS_A2 and Y410
    for f in [19u32, 20, 44, 45] {
        writeln!(s, "case fp-stencil-{f}\ndevice12\nfootprint12 0 2 0 {}\nfootprint12 0 6 0 {}\nfootprint12 1 2 64 {}\nend", desc_str(&Desc::tex(3, 100, 30, 1, 1, f)), desc_str(&Desc::tex(3, 64, 64, 3, 1, f)), desc_str(&Desc::tex(2, 300, 1, 2, 2, f))).unwrap();
    }
    for f in [89u32, 101, 113, 114] {
        for (dim, w, h, d) in [(2u32, 64u32, 1u32, 1u32), (3, 7, 3, 2), (4, 16, 8, 4)] {
            writeln!(s, "case fp-onemip-{f}-{dim}\ndevice12\nfootprint12 0 1 0 {}\ncreate12 1 0 {}\nend", desc_str(&Desc::tex(dim, w, h, d, 1, f)), desc_str(&Desc::tex(dim, w, h, d, 1, f))).unwrap();
        }
    }
    // copies to a texture of each format and back (8x4: whole blocks)
    // (not 420_OPAQUE: the CPU has no access to it, and the executor stopped at it, golden 936cec8)
    for f in (1u32..=132).chain([189, 190, 191]).filter(|f| *f != 106) {
        writeln!(s, "case copy-{f}\ndevice12\ncopy12 {f} 8 4 0\nend").unwrap();
    }
    for f in [20u32, 40, 45, 55] {
        writeln!(s, "case copy-ds-{f}\ndevice12\ncopy12 {f} 8 4 1\nend").unwrap();
    }
    for (w, h) in [(1u32, 1u32), (300, 7), (64, 64), (4096, 2)] {
        writeln!(s, "case copy-size-{w}x{h}\ndevice12\ncopy12 28 {w} {h} 0\ncopy12 2 {w} {h} 0\nend").unwrap();
    }
    // object creation: one line a case, each field swept around a valid description
    let mut mk: Vec<String> = Vec::new();
    for t in 0..=6u32 {
        mk.push(format!("heap12 {t} 65536 0 0"));
    }
    for z in [0u64, 1, 4096, 65535, 65537, 1 << 20, 1 << 32] {
        mk.push(format!("heap12 1 {} 0 0", z.min(u32::MAX as u64)));
    }
    for a in [1u32, 4096, 65536, 4194304, 3, 131072] {
        mk.push(format!("heap12 1 65536 {a} 0"));
    }
    for f in [1u32, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0xc0, 0xc4, 0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000, 0x8000_0000] {
        mk.push(format!("heap12 1 65536 0 {}", hexw(f)));
        mk.push(format!("heap12 2 65536 0 {}", hexw(f)));
    }
    for t in 0..=5u32 {
        for (n, f) in [(1u32, 0u32), (0, 0), (1, 1), (2048, 0), (2049, 0), (2048, 1), (2049, 1), (1_000_000, 1), (1_000_001, 1), (1_000_000, 0), (1, 2), (1, 0x8000_0000)] {
            mk.push(format!("dheap12 {t} {n} {}", hexw(f)));
        }
    }
    for t in 0..=8u32 {
        mk.push(format!("queue12 {t} 0 0"));
        mk.push(format!("cmdlist12 {t}"));
        mk.push(format!("qheap12 {t} 1"));
    }
    for p in [100u32, 10000, 1, 99, 0xffff_ffff] {
        mk.push(format!("queue12 0 {} 0", hexw(p)));
    }
    for f in [1u32, 2, 4] {
        mk.push(format!("queue12 0 0 {f}"));
    }
    for n in [0u32, 65536, 65537, 1 << 24] {
        mk.push(format!("qheap12 0 {n}"));
        mk.push(format!("qheap12 2 {n}"));
    }
    for (v, f) in [(0u32, 0u32), (5, 0), (0xffff_ffff, 0), (0, 1), (0, 2), (0, 4), (0, 8), (0, 0x10)] {
        mk.push(format!("fence12 {} {}", hexw(v), hexw(f)));
    }
    let tex = Desc::tex(3, 64, 64, 1, 1, 28);
    let rt = Desc { flags: 1, ..tex };
    let b = Desc::buffer(65536);
    for (ht, hf, size, off, st, d) in [
        (1u32, 0u32, 1u32 << 20, 0u32, 0u32, b), (1, 0, 1 << 20, 65536, 0, b), (1, 0, 1 << 20, 4096, 0, b), (1, 0, 1 << 20, 1 << 20, 0, b), (1, 0, 65536, 0, 0, Desc::buffer(65537)),
        (1, 4, 1 << 20, 0, 0, b), (1, 0x40, 1 << 20, 0, 0, b), (1, 0x80, 1 << 20, 0, 0, b), (1, 0xc0, 1 << 20, 0, 0, b),
        (1, 0, 1 << 20, 0, 0, tex), (1, 4, 1 << 20, 0, 0, tex), (1, 0x80, 1 << 20, 0, 0, tex), (1, 0x40, 1 << 20, 0, 0, tex), (1, 0x84, 1 << 20, 0, 0, tex),
        (1, 0, 1 << 20, 0, 4, rt), (1, 0x40, 1 << 20, 0, 4, rt), (1, 0x80, 1 << 20, 0, 4, rt), (1, 0x44, 1 << 20, 0, 4, rt),
        (1, 0, 1 << 20, 4096, 0, Desc { align: 4096, ..tex }), (1, 0, 1 << 20, 4096, 0, tex), (1, 0, 4194304, 0, 4, Desc { samples: 4, ..rt }), (1, 0, 8 << 20, 65536, 4, Desc { samples: 4, ..rt }),
        (2, 0, 1 << 20, 0, 0xac3, b), (2, 0, 1 << 20, 0, 0, b), (2, 0, 1 << 20, 0, 0, tex), (3, 0, 1 << 20, 0, 0x400, b), (3, 0, 1 << 20, 0, 0, b),
    ] {
        mk.push(format!("placed12 {ht} {} {size} {off} {} {}", hexw(hf), hexw(st), desc_str(&d)));
    }
    // (added after: new lines only at the end, so that the cases keep their numbers)
    for t in [3u32, 5] {
        for f in [1u32, 4, 0x40, 0x80, 0xc0, 0x44, 0x84, 0x100, 0x200, 0x400, 0x800, 0x1000] {
            mk.push(format!("heap12 {t} 65536 0 {}", hexw(f)));
        }
    }
    for f in [0x44u32, 0x84, 0x1801, 0x4c0, 0x244] {
        mk.push(format!("heap12 1 65536 0 {}", hexw(f)));
        mk.push(format!("heap12 2 65536 0 {}", hexw(f)));
    }
    for (v, f) in [(0u32, 3u32), (0, 5)] {
        mk.push(format!("fence12 {} {}", hexw(v), hexw(f)));
    }
    // (the bound on a 32-bit process's query heaps)
    for t in [0u32, 1, 2, 3, 5, 6, 7] {
        for n in [1u32 << 20, 1 << 21, 1 << 22, 1 << 23, (1 << 24) - 1] {
            mk.push(format!("qheap12 {t} {n}"));
        }
    }
    for t in [0u32, 3] {
        for n in [5u32 << 20, 6 << 20, 7 << 20, (1 << 23) - 1] {
            mk.push(format!("qheap12 {t} {n}"));
        }
    }
    for t in [0u32, 3] {
        for n in [7u32 << 20 | 1, 15 << 19, 31 << 18, 63 << 17, 127 << 16, (1 << 23) - 4096, (1 << 23) - 256, (1 << 23) - 2] {
            mk.push(format!("qheap12 {t} {n}"));
        }
    }
    for (i, l) in mk.iter().enumerate() {
        // (descriptor heap types past the last, 4 and 5: a 32-bit process crashed making one,
        // golden e91efb1; their numbers left unused)
        if l.starts_with("dheap12 4 ") || l.starts_with("dheap12 5 ") {
            continue;
        }
        writeln!(s, "case make-{i}\ndevice12\n{l}\nend").unwrap();
    }

    // root signatures (D3D12SerializeVersionedRootSignature, CreateRootSignature): new lines only
    // at the end, so that the cases keep their numbers
    let mut rs: Vec<String> = Vec::new();
    for v in [1u32, 2] {
        for f in [0u32, 1, 0x7f, 0x800, 0xfff, 0x1000, 0x8000_0000] {
            rs.push(format!("{v} {} ", hexw(f)));
        }
        for n in [0u32, 1, 4, 63, 64, 65] {
            rs.push(format!("{v} 0 c 0 0 {n} 0"));
        }
        for vis in 0u32..=8 {
            rs.push(format!("{v} 0 c 1 2 3 {vis}"));
            rs.push(format!("{v} 0 t {vis} 1 0 1 0 0 0 0"));
            rs.push(format!("{v} 0 s 0x15 0 0 {vis}"));
        }
        for k in ["cbv", "srv", "uav"] {
            for f in [0u32, 1, 2, 4, 6, 8, 0xe, 0x10] {
                rs.push(format!("{v} 0 {k} 3 1 0 {}", hexw(f)));
            }
        }
        for ty in 0u32..=4 {
            for (n, f) in [(1u32, 0u32), (0, 0), (0xffff_ffff, 0), (4, 1), (4, 2), (4, 3), (4, 8), (4, 0xa), (4, 0x10000), (4, 0x10001), (4, 0x10008), (4, 0x20000)] {
                rs.push(format!("{v} 0 t 0 1 {ty} {} 2 1 0 {}", hexw(n), hexw(f)));
            }
            rs.push(format!("{v} 0 t 0 1 {ty} 4 0 0 0xffffffff 0"));
        }
        rs.push(format!("{v} 0 t 0 0"));
        rs.push(format!("{v} 0 t 0 2 3 1 0 0 0 0 1 1 0 0 0 0"));
        rs.push(format!("{v} 0 t 0 2 3 1 0 0 0 0 3 1 1 0 1 0"));
        rs.push(format!("{v} 0 t 0 3 0 2 0 0 0 0 1 4 0 0 0xffffffff 0 2 1 0 0 0xffffffff 0"));
        rs.push(format!("{v} 0 t 0 2 1 4 0 0 0 0 1 4 2 0 0 0"));
        rs.push(format!("{v} 0 c 0 0 1 0 cbv 0 0 0 0"));
        rs.push(format!("{v} 0 c 0 0 1 1 cbv 0 0 2 0"));
        rs.push(format!("{v} 0 cbv 0 0 0 0 cbv 0 0 0 0"));
        rs.push(format!("{v} 0 s 0x15 0 0 0 s 0x15 0 0 0"));
        rs.push(format!("{v} 0 s 0x15 0 0 0 s 0x15 1 0 0"));
        for f in [0u32, 1, 0x14, 0x15, 0x55, 0x80, 0x95, 0xd5, 0x100, 0x115, 0x155, 0x180, 0x195, 0x1d5, 0xffff] {
            rs.push(format!("{v} 0 s {} 5 6 0", hexw(f)));
        }
        rs.push(format!("{v} 0 {}", (0..16).map(|k| format!("c {k} 0 1 0")).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 {}", (0..32).map(|k| format!("cbv {k} 0 0 0")).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 {}", (0..16).map(|k| format!("s 0x15 {k} 0 0")).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 c 0 0 62 0 cbv 0 0 0 0"));
        rs.push(format!("{v} 0 c 0 0 61 0 cbv 0 0 0 0 t 0 1 0 1 1 0 0 0"));
    }
    // (random root signatures: a few of each parameter, valid values)
    let mut r = XorShift32::new(0xd3d1_2500);
    for _ in 0..48 {
        let v = r.range(1, 2);
        let mut l = format!("{v} {}", hexw(*r.pick(&[0u32, 1, 2, 0x10, 0x41, 0x7f])));
        for _ in 0..r.range(0, 6) {
            let vis = r.range(0, 7);
            match r.range(0, 4) {
                0 => write!(l, " c {} {} {} {vis}", r.range(0, 8), r.range(0, 3), r.range(1, 9)).unwrap(),
                1 => write!(l, " {} {} {} {vis} {}", r.pick(&["cbv", "srv", "uav"]), r.range(0, 8), r.range(0, 3), r.pick(&[0u32, 2, 4, 8])).unwrap(),
                2 => {
                    let n = r.range(1, 4);
                    let sampler = r.chance(4);
                    write!(l, " t {vis} {n}").unwrap();
                    for _ in 0..n {
                        let ty = if sampler { 3 } else { r.range(0, 3) };
                        let f = if sampler { *r.pick(&[0u32, 1]) } else { *r.pick(&[0u32, 1, 2, 4, 8, 0x10000]) };
                        write!(l, " {ty} {} {} {} {} {}", r.range(1, 9), r.range(0, 8), r.range(0, 3), hexw(*r.pick(&[0u32, 8, 0xffff_ffff])), hexw(f)).unwrap();
                    }
                }
                _ => write!(l, " s {} {} {} {vis}", hexw(*r.pick(&[0u32, 0x15, 0x55, 0x94])), r.range(0, 8), r.range(0, 3)).unwrap(),
            }
        }
        rs.push(l);
    }
    // (added after: the flags bit by bit, LOCAL_ROOT_SIGNATURE with each, filters, bindings that
    // meet or not, sixteen parameters within the executor's tokens)
    for v in [1u32, 2] {
        for b in 0..32 {
            rs.push(format!("{v} {}", hexw(1 << b)));
        }
        for b in (0..12).filter(|b| *b != 7) {
            rs.push(format!("{v} {}", hexw(0x80 | 1 << b)));
        }
        rs.push(format!("{v} 0xf7f"));
        rs.push(format!("{v} 0x8000007f"));
        for f in [0x2u32, 0x4, 0x5, 0x10, 0x11, 0x40, 0x41, 0x44, 0x50, 0x54, 0xd4, 0x154, 0x1d4, 0x200, 0x1d5] {
            rs.push(format!("{v} 0 s {} 0 0 0", hexw(f)));
        }
        for (a, b) in [("c 0 0 1 1", "cbv 0 0 2 0"), ("c 0 0 1 0", "cbv 0 1 0 0"), ("c 0 0 1 1", "cbv 0 0 0 0"), ("srv 0 0 0 0", "uav 0 0 0 0"), ("srv 5 0 0 0", "t 0 1 0 0xffffffff 0 0 0 0"), ("srv 5 0 0 0", "t 0 1 0 5 0 0 0 0"), ("srv 5 0 0 0", "t 0 1 0 5 1 0 0 0"), ("s 0x15 3 0 1", "t 2 1 3 1 3 0 0 0"), ("s 0x15 3 0 1", "t 1 1 3 1 3 0 0 0")] {
            rs.push(format!("{v} 0 {a} {b}"));
        }
        rs.push(format!("{v} 0 t 0 2 0 2 0 0 0 0 0 2 2 0 0 0"));
        rs.push(format!("{v} 0 t 0 2 0 2 0 0 0 0 0 2 1 0 0 0"));
        rs.push(format!("{v} 0 t 0 1 0 4 0xfffffffe 0 0 0"));
        for f in [9u32, 5, 0x10002, 0x10004] {
            rs.push(format!("{v} 0 t 0 1 0 4 0 0 0 {}", hexw(f)));
        }
        rs.push(format!("{v} 0 {}", (0..16).map(|_| "t 0 0".to_string()).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 {}", (0..15).map(|k| format!("s 0x15 {k} 0 0")).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 {} c 0 1 1 0", (0..15).map(|_| "t 0 0".to_string()).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 {} c 0 1 1 0", (0..16).map(|_| "t 0 0".to_string()).collect::<Vec<_>>().join(" ")));
        rs.push(format!("{v} 0 c 0 0 64 0"));
        rs.push(format!("{v} 0 c 0 0 62 0 cbv 1 0 0 0"));
        rs.push(format!("{v} 0 c 0 0 63 0 cbv 1 0 0 0"));
        rs.push(format!("{v} 0 c 0 0 0xffffffff 0"));
    }
    for v in [1u32, 2] {
        rs.push(format!("{v} 0x80000080"));
        rs.push(format!("{v} 0 t 0 1 0 2 0xfffffffe 0 0 0"));
        rs.push(format!("{v} 0 t 0 1 0 0xffffffff 0xfffffffe 0 0 0"));
        rs.push(format!("{v} 0 srv 0 0 0 0 t 0 1 0 0xffffffff 1 0 0 0"));
    }
    for (i, l) in rs.iter().enumerate() {
        writeln!(s, "case rootsig-{i}\ndevice12\nrootsig12 {}\nend", l.trim_end()).unwrap();
    }

    s
}

/// a DXGI_SWAP_CHAIN_DESC1 as the executor's 12 tokens: width height format stereo samples
/// quality usage buffers scaling effect alpha flags
pub const CHAIN: [u32; 12] = [0, 0, 87, 0, 1, 0, 0x20, 2, 0, 4, 0, 0];

fn chain_line(s: &mut String, op: &str, w: &[u32]) {
    writeln!(s, "{op} {}", w.iter().map(|v| hexw(*v)).collect::<Vec<_>>().join(" ")).unwrap();
}

/// a D32_FLOAT render target removes the device (D3D12.CLEAR.REMOVED): alone in a suite, a
/// process of its own (in the d3d12 suite the executor crashed after it, golden e91efb1)
pub fn d3d12rm() -> String {
    // (removed by RemoveDevice: a D32_FLOAT render target removed it too, but WARP crashed some
    // runs after it, golden 936cec8, 465fd71, 5366b0e)
    "# kikizake suite d3d12rm: a removed Direct3D 12 device. Made by `kikizake gen d3d12rm`.\ncase clear-removed\ndevice12\nremove12\nclear12 28 0 0 0 0\nend\n".into()
}

pub fn dxgi() -> String {
    let mut s = String::from("# gpu\n# kikizake suite dxgi: DXGI factories and flip-model swap chains on Direct3D 12 (SPEC.md). Made by `kikizake gen dxgi`.\n");
    writeln!(s, "case factory\ndxgifactory 0\ndxgiwarp").unwrap();
    for f in [2, 4, 0x10, 0x8000_0000u32, 0] {
        writeln!(s, "dxgifactory {}\ndxgiwarp", hexw(f)).unwrap();
    }
    writeln!(s, "case tearing\ndxgifactory 0\ndxgitearing\nend\ncase adapters\ndxgifactory 0\ndxgiadapters\nend").unwrap();
    for cs in (0u32..=25).chain([0xffff_ffff]) {
        writeln!(s, "case color-{cs}\ndevice12\ndxgifactory 0").unwrap();
        chain_line(&mut s, "swapchain", &CHAIN);
        writeln!(s, "sccolor {}\nend", hexw(cs)).unwrap();
    }
    for f in [10u32, 24, 28] {
        for cs in (0u32..=25).chain([0xffff_ffff]) {
            writeln!(s, "case color-f{f}-{cs}\ndevice12\ndxgifactory 0").unwrap();
            let mut c = CHAIN;
            c[2] = f;
            chain_line(&mut s, "swapchain", &c);
            writeln!(s, "sccolor {}\nend", hexw(cs)).unwrap();
        }
    }
    for f in [10u32, 24] {
        writeln!(s, "case color-fmt{f}\ndevice12\ndxgifactory 0").unwrap();
        let mut c = CHAIN;
        c[2] = f;
        chain_line(&mut s, "swapchain", &c);
        for cs in [0u32, 1, 12, 14, 17] {
            writeln!(s, "sccolor {}", hexw(cs)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // (DXGI_CREATE_FACTORY_DEBUG on its own: its answer depends on the Graphics Tools being there)
    writeln!(s, "end\ncase factory-debug\ndxgifactory 1\ndxgiwarp").unwrap();
    writeln!(s, "end\ncase assoc\ndxgifactory 0").unwrap();
    for f in (0..=8).chain([0x10, 0x100, 0x8000_0000u32]) {
        writeln!(s, "dxgiassoc {}", hexw(f)).unwrap();
        writeln!(s, "dxgiassoc {} null", hexw(f)).unwrap();
    }
    writeln!(s, "end").unwrap();
    // each field of the description through values of interest
    let pools: [Vec<u32>; 12] = [
        vec![0, 1, 2, 63, 64, 100, 8192, 16384, 16385],
        vec![0, 1, 2, 63, 64, 100, 8192, 16384, 16385],
        (0..=132).chain([191, 192]).collect(),
        vec![0, 1, 2],
        vec![0, 1, 2, 4, 8],
        vec![0, 1, 2],
        vec![0, 0x10, 0x20, 0x30, 0x40, 0x60, 0x80, 0x100, 0x200, 0x400, 0x420, 0x800, 0x1000, 0xffff_ffff],
        (0..=17).chain([0xffff_ffff]).collect(),
        (0..=4).collect(),
        (0..=5).collect(),
        (0..=5).collect(),
        (0..=13).map(|b| 1u32 << b).chain([0, 0x40 | 0x800, 0x8000_0000]).collect(),
    ];
    let names = ["width", "height", "format", "stereo", "samples", "quality", "usage", "buffers", "scaling", "effect", "alpha", "flags"];
    for (at, pool) in pools.iter().enumerate() {
        for (c, chunk) in pool.chunks(50).enumerate() {
            writeln!(s, "case swapchain-{}-{c}\ndevice12\ndxgifactory 0", names[at]).unwrap();
            for v in chunk {
                let mut w = CHAIN;
                w[at] = *v;
                chain_line(&mut s, "swapchain", &w);
            }
            writeln!(s, "end").unwrap();
        }
    }
    // the other effects, scalings and formats together
    writeln!(s, "case swapchain-pairs\ndevice12\ndxgifactory 0").unwrap();
    for effect in [3, 4] {
        for scaling in 0..3 {
            for f in [10, 24, 28, 87, 29, 91] {
                let mut w = CHAIN;
                (w[9], w[8], w[2]) = (effect, scaling, f);
                chain_line(&mut s, "swapchain", &w);
            }
        }
    }
    writeln!(s, "end").unwrap();
    // buffers, presents, resizes, frame latency
    for n in [2, 3, 5, 16] {
        writeln!(s, "case chain-buffers-{n}\ndevice12\ndxgifactory 0").unwrap();
        let mut w = CHAIN;
        w[7] = n;
        chain_line(&mut s, "swapchain", &w);
        for i in 0..=n {
            writeln!(s, "scbuffer {i}").unwrap();
        }
        writeln!(s, "scbuffer 0xffffffff").unwrap();
        for i in 0..n + 2 {
            writeln!(s, "scpresent {} 0", i % 2).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    writeln!(s, "case chain-present\ndevice12\ndxgifactory 0").unwrap();
    chain_line(&mut s, "swapchain", &CHAIN);
    for (sync, flags) in [(0, 0), (1, 0), (4, 0), (5, 0), (0, 1), (0, 2), (0, 0x200), (0, 0x8000_0000), (1, 0x200), (0, 0x100)] {
        writeln!(s, "scpresent {sync} {}", hexw(flags)).unwrap();
    }
    // (DXGI_PRESENT_RESTART apart: the model lacks it)
    writeln!(s, "end\ncase chain-present-restart\ndevice12\ndxgifactory 0").unwrap();
    chain_line(&mut s, "swapchain", &CHAIN);
    writeln!(s, "scpresent 0 4\nend").unwrap();
    writeln!(s, "case chain-resize\ndevice12\ndxgifactory 0").unwrap();
    chain_line(&mut s, "swapchain", &CHAIN);
    for (n, w, h, f, fl) in [(0u32, 0u32, 0u32, 0u32, 0u32), (3, 0, 0, 0, 0), (0, 100, 50, 0, 0), (0, 0, 0, 28, 0), (0, 0, 0, 10, 0), (0, 0, 0, 24, 0), (0, 0, 0, 29, 0), (0, 0, 0, 2, 0), (1, 0, 0, 0, 0), (16, 0, 0, 0, 0), (17, 0, 0, 0, 0), (2, 0, 0, 0, 0), (0, 16385, 1, 0, 0), (0, 0, 0, 0, 0x40), (0, 0, 0, 0, 0x800), (0, 0, 0, 0, 2), (0, 0, 0, 0, 0x8000_0000), (0, 0, 0, 0xffff_ffff, 0)] {
        writeln!(s, "scresize {n} {w} {h} {f} {}", hexw(fl)).unwrap();
        writeln!(s, "scpresent 0 0").unwrap();
    }
    writeln!(s, "end").unwrap();
    for flags in [0, 0x40] {
        writeln!(s, "case chain-latency-{flags:x}\ndevice12\ndxgifactory 0").unwrap();
        let mut w = CHAIN;
        w[11] = flags;
        chain_line(&mut s, "swapchain", &w);
        for n in [0, 1, 2, 3, 16, 17, 0xffff_ffffu32] {
            writeln!(s, "sclatency {}", hexw(n)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // random descriptions near the valid ones
    let mut r = XorShift32::new(0xd61_5a00);
    for i in 0..60 {
        writeln!(s, "case swapchain-random-{i}\ndevice12\ndxgifactory 0").unwrap();
        for _ in 0..r.range(3, 12) {
            let mut w = CHAIN;
            for _ in 0..r.range(1, 3) {
                let at = r.range(0, 11) as usize;
                w[at] = *r.pick(&pools[at]);
            }
            chain_line(&mut s, "swapchain", &w);
            if r.chance(3) {
                writeln!(s, "scpresent 0 0").unwrap();
            }
        }
        writeln!(s, "end").unwrap();
    }
    writeln!(s, "case nofactory\ndxgiwarp\ndxgiassoc 0\nswapchain 0 0 87 0 1 0 0x20 2 0 4 0 0\nend").unwrap();
    writeln!(s, "case nochain\ndxgifactory 0\nscresize 0 0 0 0 0\nscbuffer 0\nscpresent 0 0\nsclatency 1\nend").unwrap();
    writeln!(s, "case nodev\ndxgifactory 0\nswapchain 0 0 87 0 1 0 0x20 2 0 4 0 0\nend").unwrap();
    s
}
