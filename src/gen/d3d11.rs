//! The d3d11 suites' case generators.
use super::*;
use crate::model::d3d11::Kind;

/// the descriptions the d3d11 cases start from (those of masu's d3d11desc probe: valid, rt0 and
/// depth enabled, stencil disabled)
pub fn d3d11_base(k: Kind) -> Vec<u32> {
    match k {
        Kind::Blend => {
            let mut d = vec![0u32; 66];
            d[2..10].copy_from_slice(&[1, 5, 6, 1, 2, 1, 1, 0xf]);
            d
        }
        Kind::Depth => vec![1, 1, 2, 0, 0xffff, 1, 1, 1, 8, 1, 1, 1, 8],
        Kind::Raster => vec![3, 3, 0, 0, 0, 0, 1, 0, 0, 0],
        Kind::Sampler => vec![0x15, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0x7f7f_ffff],
    }
}

/// float values around what the descriptions check (no -0.0: see D3D11.DEDUP.FLOAT)
pub fn d3d11_floats() -> Vec<u32> {
    let up = |v: f32| v.to_bits() + 1;
    vec![
        0,
        1.0f32.to_bits(),
        (-1.0f32).to_bits(),
        0.5f32.to_bits(),
        15.99f32.to_bits(),
        up(15.99),
        16.0f32.to_bits(),
        (-16.0f32).to_bits(),
        up(-16.0),
        (-15.99f32).to_bits(),
        f32::MAX.to_bits(),
        f32::MIN.to_bits(),
        f32::INFINITY.to_bits(),
        f32::NEG_INFINITY.to_bits(),
        0x7fc0_0000,
        0xffc0_0000,
        0x7f80_0001,
        1,
        1000.0f32.to_bits(),
    ]
}

/// the values each word of a description is tried with
fn d3d11_pool(k: Kind, i: usize) -> Vec<u32> {
    let enums = || (0..=21).chain([0xffff_ffff, 0x8000_0000]).collect::<Vec<u32>>();
    let bools = || vec![0, 1, 2, 0xffff_ffff];
    match (k, i) {
        (Kind::Blend, 0 | 1) => bools(),
        (Kind::Blend, i) if (i - 2) % 8 == 0 => bools(),
        (Kind::Blend, i) if (i - 2) % 8 == 7 => (0..=0x10).chain([0x1f, 0xff, 0x100, 0x10f, 0xffff_ffff]).collect(),
        (Kind::Blend, _) => enums(),
        (Kind::Depth, 0 | 3) => bools(),
        (Kind::Depth, 4) => vec![0, 0xff, 0xff00, 0xffff, 0x0f3c, 0x1_ffff, 0xffff_ffff],
        (Kind::Depth, _) => (0..=10).chain([0xffff_ffff]).collect(),
        (Kind::Raster, 0 | 1) => (0..=5).chain([0xffff_ffff]).collect(),
        (Kind::Raster, 3) => vec![0, 1, (-5i32) as u32, i32::MAX as u32, i32::MIN as u32],
        (Kind::Raster, 4 | 5) => d3d11_floats(),
        (Kind::Raster, _) => bools(),
        (Kind::Sampler, 0) => vec![0, 1, 4, 5, 0x10, 0x11, 0x14, 0x15, 0x55, 0x80, 0x95, 0xd5, 0x115, 0x155, 0x195, 0x1d5, 2, 0x54, 0x200],
        (Kind::Sampler, 1..=3) => (0..=7).chain([0xffff_ffff]).collect(),
        (Kind::Sampler, 5) => (0..=18).chain([0xffff_ffff]).collect(),
        (Kind::Sampler, 6) => (0..=10).chain([0xffff_ffff]).collect(),
        (Kind::Sampler, _) => d3d11_floats(),
    }
}

fn d3d11_line(s: &mut String, k: Kind, d: &[u32], check: bool) {
    let n = d.iter().rposition(|w| *w != 0).map_or(0, |i| i + 1);
    let words: Vec<String> = d[..n].iter().map(|w| if *w < 10 { w.to_string() } else { format!("{w:#x}") }).collect();
    writeln!(s, "{}{} {}", k.name(), if check { " check" } else { "" }, words.join(" ")).unwrap();
}

pub fn d3d11() -> String {
    let mut s = String::from("# gpu\n# kikizake suite d3d11: Direct3D 11 state objects (SPEC.md). Made by `kikizake gen d3d11`.\n");
    // one word of a description through every value of interest, the others as `base`; at most
    // 60 lines a case (an executor case holds 64 objects)
    let sweep = |s: &mut String, name: &str, k: Kind, base: &[u32], at: usize, values: &[u32]| {
        for (c, chunk) in values.chunks(60).enumerate() {
            writeln!(s, "case {name}-{c}\ndevice11").unwrap();
            for v in chunk {
                let mut d = base.to_vec();
                d[at] = *v;
                d3d11_line(s, k, &d, false);
            }
            writeln!(s, "end").unwrap();
        }
    };
    // blend: each word of render target 0, 3 and 7, blending on and off, targets shared and not
    let blend = d3d11_base(Kind::Blend);
    for indep in [0, 1] {
        for (enable, rt) in [(1, 0), (0, 0), (1, 3), (1, 7)] {
            let mut base = blend.clone();
            base[1] = indep;
            base[2 + rt * 8..10 + rt * 8].copy_from_slice(&[enable, 5, 6, 1, 2, 1, 1, 0xf]);
            for f in 0..8 {
                let at = 2 + rt * 8 + f;
                sweep(&mut s, &format!("blend-i{indep}-rt{rt}-e{enable}-w{f}"), Kind::Blend, &base, at, &d3d11_pool(Kind::Blend, at));
            }
        }
        let mut base = blend.clone();
        base[1] = indep;
        sweep(&mut s, &format!("blend-i{indep}-a2c"), Kind::Blend, &base, 0, &d3d11_pool(Kind::Blend, 0));
        sweep(&mut s, &format!("blend-i{indep}-indep"), Kind::Blend, &base, 1, &d3d11_pool(Kind::Blend, 1));
    }
    // depth stencil: each word, depth and stencil each on and off
    for depth in [0, 1] {
        for stencil in [0, 1] {
            let mut base = d3d11_base(Kind::Depth);
            base[0] = depth;
            base[3] = stencil;
            for at in 0..13 {
                sweep(&mut s, &format!("depth-d{depth}-s{stencil}-w{at}"), Kind::Depth, &base, at, &d3d11_pool(Kind::Depth, at));
            }
        }
    }
    // rasterizer: each word
    let base = d3d11_base(Kind::Raster);
    for at in 0..10 {
        sweep(&mut s, &format!("raster-w{at}"), Kind::Raster, &base, at, &d3d11_pool(Kind::Raster, at));
    }
    // sampler: every filter value below 0x400, and some above
    let base = d3d11_base(Kind::Sampler);
    let filters: Vec<u32> = (0..0x400).chain([0x1000, 0x8000_0000, 0xffff_ffff]).collect();
    sweep(&mut s, "sampler-filter", Kind::Sampler, &base, 0, &filters);
    // each other word under a plain, an anisotropic, a comparison and a maximum filter, with
    // and without a BORDER address
    for filter in [0x15, 0x55, 0x95, 0x195] {
        for addr in [1, 4] {
            let mut base = d3d11_base(Kind::Sampler);
            base[0] = filter;
            base[3] = addr;
            for at in 1..13 {
                sweep(&mut s, &format!("sampler-f{filter:x}-a{addr}-w{at}"), Kind::Sampler, &base, at, &d3d11_pool(Kind::Sampler, at));
            }
        }
    }
    // random descriptions near the valid ones, repeats of earlier ones (the same object?) and
    // validation only (ppState NULL)
    let mut r = XorShift32::new(0xd3d1_1011);
    for i in 0..300 {
        writeln!(s, "case random-{i}\ndevice11").unwrap();
        let mut made: Vec<(Kind, Vec<u32>)> = Vec::new();
        for _ in 0..r.range(5, 40) {
            let (k, mut d) = if !made.is_empty() && r.chance(4) {
                r.pick(&made).clone()
            } else {
                let k = *r.pick(&Kind::ALL);
                (k, d3d11_base(k))
            };
            for _ in 0..r.range(0, 4) {
                let at = r.range(0, k.words() as i64 - 1) as usize;
                d[at] = *r.pick(&d3d11_pool(k, at));
            }
            d3d11_line(&mut s, k, &d, r.chance(10));
            made.push((k, d));
        }
        writeln!(s, "end").unwrap();
    }
    // the calls around the descriptions
    writeln!(s, "case nodev").unwrap();
    for k in Kind::ALL {
        d3d11_line(&mut s, k, &d3d11_base(k), false);
    }
    writeln!(s, "end\ncase nodesc\ndevice11").unwrap();
    for k in Kind::ALL {
        writeln!(s, "{} nodesc", k.name()).unwrap();
    }
    writeln!(s, "end\ncase check\ndevice11").unwrap();
    for k in Kind::ALL {
        let mut bad = d3d11_base(k);
        bad[if k == Kind::Blend { 3 } else { 0 }] = 0;
        d3d11_line(&mut s, k, &d3d11_base(k), true);
        d3d11_line(&mut s, k, &bad, true);
        d3d11_line(&mut s, k, &d3d11_base(k), false);
        d3d11_line(&mut s, k, &d3d11_base(k), true);
    }
    writeln!(s, "end").unwrap();
    // what the documentation leaves open: 0.0 and -0.0
    for (k, at) in [(Kind::Raster, 4), (Kind::Raster, 5), (Kind::Sampler, 4), (Kind::Sampler, 11)] {
        writeln!(s, "case edge-negzero-{}-w{at}\ndevice11", k.name()).unwrap();
        let mut d = d3d11_base(k);
        d3d11_line(&mut s, k, &d, false);
        d[at] = 0x8000_0000;
        d3d11_line(&mut s, k, &d, false);
        writeln!(s, "end").unwrap();
    }
    s
}
