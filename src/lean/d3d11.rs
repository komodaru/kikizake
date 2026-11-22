//! Direct3D 10 and 11's mirrors (spec/lean/Kikizake/D3d11/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3drules::{self, Api, Bind, Bound, Pick, Prim, Topo, Verdict};

const RASTER: &str = "spec/lean/Kikizake/D3d11/Raster.lean";
const DRAW: &str = "spec/lean/Kikizake/D3d11/Draw.lean";
const TEXTURE: &str = "spec/lean/Kikizake/D3d11/Texture.lean";
const U32: i64 = 0xffff_ffff;

/// a bool argument
fn flag(a: &[i64], i: usize) -> Option<bool> {
    arg::<u8>(a, i).map(|v| v != 0)
}

/// a point of two arguments
fn pt(a: &[i64], i: usize) -> Option<(i64, i64)> {
    Some((arg(a, i)?, arg(a, i.checked_add(1)?)?))
}

/// a view's arguments: has, samples, has a size, width, height
fn bound(a: &[i64], i: usize) -> Option<Option<Bound>> {
    let at = |k: usize| i.checked_add(k);
    if !flag(a, i)? {
        return Some(None);
    }
    let size = if flag(a, at(2)?)? { Some((arg(a, at(3)?)?, arg(a, at(4)?)?)) } else { None };
    Some(Some(Bound { samples: arg(a, at(1)?)?, size }))
}

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.covers",
        lean: RASTER,
        domain: Domain::Sampled("every triangle and point of coordinates -256..256 in half pixels (1/256 units), and 300,000 of -700..700"),
        inputs: |each| {
            every(&[(-2, 2); 8], &mut |v| {
                let w: Vec<i64> = v.iter().map(|x| x.saturating_mul(128)).collect();
                each(&w);
            });
            sampled(&[(-700, 700); 8], 300_000, 0xc0e7_d011, each);
        },
        model: |a| one(d3drules::covers([pt(a, 0)?, pt(a, 2)?, pt(a, 4)?], pt(a, 6)?)),
    },
    Mirror {
        name: "d3d11.cull",
        lean: RASTER,
        domain: Domain::Every("modes 0..9, FrontCounterClockwise or not, clockwise or not"),
        inputs: |each| every(&[(0, 9), (0, 1), (0, 1)], each),
        model: |a| one(d3drules::culls(arg(a, 0)?, flag(a, 1)?, flag(a, 2)?)),
    },
    Mirror {
        name: "d3d11.scissor",
        lean: RASTER,
        domain: Domain::Sampled("on or off, rectangles and pixels of -6..6 each, and 100,000 of -5000..5000"),
        inputs: |each| {
            sampled(&[(0, 1), (-6, 6), (-6, 6), (-6, 6), (-6, 6), (-6, 6), (-6, 6)], 200_000, 0x5c15_d011, each);
            sampled(&[(0, 1), (-5000, 5000), (-5000, 5000), (-5000, 5000), (-5000, 5000), (-5000, 5000), (-5000, 5000)], 100_000, 0x5c16_d011, each);
        },
        model: |a| one(d3drules::scissor_holds(flag(a, 0)?, [arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?], (arg(a, 5)?, arg(a, 6)?))),
    },
    Mirror {
        name: "d3d11.point",
        lean: RASTER,
        domain: Domain::Every("snapped coordinates -70000..70000 (1/256 pixel)"),
        inputs: |each| every(&[(-70_000, 70_000)], each),
        model: |a| one(d3drules::point_pixel(arg(a, 0)?)),
    },
    Mirror {
        name: "d3d11.pointms",
        lean: RASTER,
        domain: Domain::Sampled("points and samples of -600..600 (1/256 pixel): every combination of the ends and 300,000"),
        inputs: |each| sampled(&[(-600, 600); 4], 300_000, 0x9017_d011, each),
        model: |a| one(d3drules::point_covers(pt(a, 0)?, pt(a, 2)?)),
    },
    Mirror {
        name: "d3d11.pattern",
        lean: RASTER,
        domain: Domain::Every("sample counts 0..40"),
        inputs: |each| every(&[(0, 40)], each),
        model: |a| Some(d3drules::sample_pattern(arg(a, 0)?).unwrap_or(&[]).iter().flat_map(|&(x, y)| [x, y]).collect()),
    },
    Mirror {
        name: "d3d11.mask",
        lean: RASTER,
        domain: Domain::Sampled("triangles of -1024..1024 (1/256 pixel), pixels -4..4, 1, 2, 4, 8 and 16 samples: 300,000"),
        inputs: |each| {
            sampled(&[(-1024, 1024), (-1024, 1024), (-1024, 1024), (-1024, 1024), (-1024, 1024), (-1024, 1024), (-4, 4), (-4, 4), (0, 4)], 300_000, 0x3a5c_d011, &mut |v| {
                let mut w = v.to_vec();
                if let Some(n) = w.get_mut(8) {
                    *n = [1, 2, 4, 8, 16].get(usize::try_from(*n).unwrap_or(0)).copied().unwrap_or(1);
                }
                each(&w);
            });
        },
        model: |a| {
            let pattern = d3drules::sample_pattern(arg(a, 8)?).unwrap_or(&[]);
            Some(d3drules::sample_mask([pt(a, 0)?, pt(a, 2)?, pt(a, 4)?], pt(a, 6)?, pattern).into_iter().map(i64::from).collect())
        },
    },
    Mirror {
        name: "d3d11.verdict",
        lean: DRAW,
        domain: Domain::Sampled("programs, views, viewport each bound or not with topologies 0..200 and the 32-bit ends, and 20,000 32-bit topologies"),
        inputs: |each| {
            every(&[(0, 1), (0, 1), (0, 1), (0, 200)], each);
            sampled(&[(0, 1), (0, 1), (0, 1), (0, U32)], 20_000, 0x7090_d011, each);
        },
        model: |a| {
            one(match d3drules::draw_verdict(flag(a, 0)?, flag(a, 1)?, flag(a, 2)?, arg(a, 3)?) {
                Verdict::Incomplete => 0,
                Verdict::NoViewport => 1,
                Verdict::Crash => 2,
                Verdict::Removed => 3,
                Verdict::Nothing => 4,
                Verdict::Draw(t) => 100 + i64::from(topo_code(t)),
            })
        },
    },
    Mirror {
        name: "d3d11.prim",
        lean: DRAW,
        domain: Domain::Sampled("the topologies drawn with primitives 0..4096, and 20,000 up to 2^28"),
        inputs: |each| {
            for t in [1, 2, 3, 4, 5, 10, 11, 12, 13] {
                every(&[(t, t), (0, 4096)], each);
                sampled(&[(t, t), (0, 1 << 28)], 20_000, 0x9a1e_d011, each);
            }
        },
        model: |a| {
            let topo = Topo::of(arg(a, 0)?)?;
            Some(match d3drules::primitive(topo, arg(a, 1)?)? {
                Prim::Point(v) => vec![i64::from(v)],
                Prim::Line(x, y) => vec![i64::from(x), i64::from(y)],
                Prim::Tri([x, y, z], l) => vec![i64::from(x), i64::from(y), i64::from(z), i64::from(l)],
            })
        },
    },
    Mirror {
        name: "d3d11.prims",
        lean: DRAW,
        domain: Domain::Sampled("the topologies drawn, counts and buffers of 0..80 vertices, and 20,000 of 32-bit counts"),
        inputs: |each| {
            for t in [1, 2, 3, 4, 5, 10, 11, 12, 13] {
                every(&[(t, t), (0, 80), (0, 80)], each);
                sampled(&[(t, t), (0, U32), (0, 1 << 40)], 20_000, 0x9a1f_d011, each);
            }
        },
        model: |a| one(i64::try_from(d3drules::prim_count(Topo::of(arg(a, 0)?)?, arg(a, 1)?, arg(a, 2)?)).ok()?),
    },
    Mirror {
        name: "d3d11.word",
        lean: DRAW,
        domain: Domain::Sampled("buffers of 0..24 words, vertices 0..24, strides 0..40, words 0..8, and 100,000 32-bit vertices and strides (the offset's wrap)"),
        inputs: |each| {
            every(&[(0, 24), (0, 24), (0, 40), (0, 8)], each);
            sampled(&[(0, 24), (0, U32), (0, U32), (0, U32)], 100_000, 0x30d0_d011, each);
        },
        model: |a| {
            let n: u32 = arg(a, 0)?;
            let vtx: Vec<u32> = (1..=n).collect();
            one(d3drules::vertex_word(&vtx, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?))
        },
    },
    Mirror {
        name: "d3d11.bind",
        lean: DRAW,
        domain: Domain::Every("the three APIs; a target and a depth view each given or not, of 1, 2 or 4 samples, with a size or not, of widths and heights 1..3"),
        inputs: |each| {
            every(&[(0, 2), (0, 1), (1, 2), (0, 1), (1, 3), (1, 3), (0, 1), (1, 2), (0, 1), (1, 3), (1, 3)], &mut |v| {
                let mut w = v.to_vec();
                for i in [2usize, 7] {
                    if let Some(s) = w.get_mut(i) {
                        *s = if *s == 2 { 4 } else { 1 };
                    }
                }
                each(&w);
            });
            every(&[(0, 2), (1, 1), (1, 2), (1, 1), (1, 2), (1, 2), (1, 1), (2, 2), (1, 1), (1, 2), (1, 2)], each);
        },
        model: |a| {
            let api = match arg::<u8>(a, 0)? {
                0 => Api::D10,
                1 => Api::D101,
                2 => Api::D11,
                _ => return None,
            };
            one(match d3drules::bind_targets(api, bound(a, 1)?, bound(a, 6)?) {
                Bind::Keep => 0,
                Bind::Set { depth: false } => 1,
                Bind::Set { depth: true } => 2,
            })
        },
    },
    Mirror {
        name: "d3d11.address",
        lean: TEXTURE,
        domain: Domain::Every("modes 0..7, texel indices -60..60, textures 1..16 texels"),
        inputs: |each| every(&[(0, 7), (-60, 60), (1, 16)], each),
        model: |a| Some(d3drules::address(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?).into_iter().collect()),
    },
    Mirror {
        name: "d3d11.cmp",
        lean: TEXTURE,
        domain: Domain::Every("functions 0..9, values 0..40 compared"),
        inputs: |each| every(&[(0, 9), (0, 40), (0, 40)], each),
        model: |a| one(d3drules::compare(arg(a, 0)?, arg::<i64>(a, 1)?, arg::<i64>(a, 2)?)),
    },
    Mirror {
        name: "d3d11.bilinear8",
        lean: TEXTURE,
        domain: Domain::Sampled("8-bit texels and weights 0..256: every combination of the ends and 500,000"),
        inputs: |each| sampled(&[(0, 255), (0, 255), (0, 255), (0, 255), (0, 256), (0, 256)], 500_000, 0xb11e_d011, each),
        model: |a| one(d3drules::bilinear8(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?, arg(a, 5)?)),
    },
    Mirror {
        name: "d3d11.mip8",
        lean: TEXTURE,
        domain: Domain::Sampled("two levels' values 0..32640 and weights 0..256: every combination of the ends and 300,000"),
        inputs: |each| sampled(&[(0, 32640), (0, 32640), (0, 256)], 300_000, 0x3198_d011, each),
        model: |a| one(d3drules::mip8(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
    Mirror {
        name: "d3d11.reduce",
        lean: TEXTURE,
        domain: Domain::Every("least and greatest, texels -2..2 (whole floats), weights 0, 1, 255"),
        inputs: |each| {
            every(&[(0, 1), (-2, 2), (-2, 2), (-2, 2), (-2, 2), (0, 2), (0, 2)], &mut |v| {
                let mut w = v.to_vec();
                for i in [5usize, 6] {
                    if let Some(x) = w.get_mut(i) {
                        *x = [0, 1, 255].get(usize::try_from(*x).unwrap_or(0)).copied().unwrap_or(0);
                    }
                }
                each(&w);
            });
        },
        model: |a| {
            let pick = if arg::<u8>(a, 0)? == 0 { Pick::Least } else { Pick::Greatest };
            let t = |i: usize| -> Option<[f32; 4]> {
                let v = f32::from(arg::<i16>(a, i)?);
                Some([v, 0.0, 0.0, 0.0])
            };
            let r = d3drules::reduce(pick, [t(1)?, t(2)?, t(3)?, t(4)?], (arg(a, 5)?, arg(a, 6)?));
            let v = r.first().copied()?;
            // (whole floats of -2..2: exact as integers)
            one((-2i16..=2).find(|&k| f32::from(k).to_bits() == v.to_bits())?)
        },
    },
];

/// a topology's number (D3D11_PRIMITIVE_TOPOLOGY)
fn topo_code(t: Topo) -> u8 {
    match t {
        Topo::Points => 1,
        Topo::Lines => 2,
        Topo::LineStrip => 3,
        Topo::Tris => 4,
        Topo::TriStrip => 5,
        Topo::LinesAdj => 10,
        Topo::LineStripAdj => 11,
        Topo::TrisAdj => 12,
        Topo::TriStripAdj => 13,
    }
}

const DESC: &str = "spec/lean/Kikizake/D3d11/Desc.lean";

/// the devices asked: kind (0 Direct3D 10, 1 10.1, 2 11) and feature level
const DEVS: [(i64, i64); 8] = [(0, 0xa000), (1, 0xa100), (1, 0xa000), (1, 0x9300), (2, 0xb000), (2, 0xa100), (2, 0x9300), (2, 0x9100)];
/// the values asked of a description's words: dims, sizes, depths or slices, levels, usages,
/// binds, CPU access, misc flags, sample counts, qualities or strides
const DIMS: [i64; 6] = [1, 2, 3, 4, 0, 5];
const SIZES: [i64; 25] = [0, 1, 2, 3, 4, 6, 7, 8, 12, 16, 48, 64, 512, 2048, 2049, 4096, 8192, 8193, 16384, 16385, 1 << 23, (1 << 23) + 1, 0x7fff_ffff, 0x8000_0000, U32];
const DEPTHS: [i64; 13] = [0, 1, 2, 3, 6, 7, 12, 256, 512, 513, 2048, 2049, 0x8000_0000];
const MIPS: [i64; 8] = [0, 1, 2, 3, 5, 12, 15, 16];
const USAGES: [i64; 6] = [0, 1, 2, 3, 4, U32];
const BINDS: [i64; 18] = [0, 1, 2, 4, 8, 0x10, 0x20, 0x28, 0x40, 0x48, 0x80, 0x88, 0xa8, 0xc0, 0x400, 0x100, 0x89, 0x2b];
const CPUS: [i64; 6] = [0, 0x10000, 0x20000, 0x30000, 1, 0x40000];
const MISCS: [i64; 18] = [0, 1, 2, 4, 5, 6, 0x10, 0x20, 0x40, 0x60, 0x80, 0x100, 0x200, 0x1000, 0x800, 7, 0x1387, 0x24];
const COUNTS: [i64; 8] = [0, 1, 2, 3, 4, 8, 16, 32];
const QS: [i64; 14] = [0, 1, 2, 4, 16, 12, 2048, 2049, 2052, 4096, 0xffff_fffe, U32, 3, 8];

/// descriptions that are taken (dim, w, h, da, mips, fmt, usage, bind, cpu, misc, count, q,
/// init): a 2D shader resource and target, a cube, a GDI-compatible target, a staging texture,
/// a 3D one, a 1D array, a multisampled target, a structured, a raw and a constant buffer
const NEAR: [[i64; 13]; 11] = [
    [2, 4, 4, 1, 1, 28, 0, 0x28, 0, 0, 1, 0, 0],
    [2, 4, 4, 6, 1, 28, 0, 8, 0, 4, 1, 0, 0],
    [2, 4, 4, 1, 1, 87, 0, 0x28, 0, 0x200, 1, 0, 0],
    [2, 4, 4, 1, 1, 28, 3, 0, 0x20000, 0, 1, 0, 0],
    [3, 4, 4, 4, 1, 28, 0, 8, 0, 0, 1, 0, 0],
    [1, 8, 1, 2, 0, 28, 0, 8, 0, 0, 1, 0, 0],
    [2, 4, 4, 1, 1, 28, 0, 0x20, 0, 0, 4, 0, 0],
    [4, 64, 1, 1, 1, 0, 0, 8, 0, 0x40, 1, 16, 0],
    [4, 64, 1, 1, 1, 0, 0, 8, 0, 0x20, 1, 0, 0],
    [4, 64, 1, 1, 1, 0, 0, 4, 0, 0, 1, 0, 0],
    [2, 8, 8, 1, 4, 71, 1, 8, 0, 0, 1, 0, 1],
];
/// formats near the rules: RGBA8, its sRGB, BGRA8 and its sRGB and typeless, R10G10B10_XR, R32
/// float, BC1, D24S8, R8G8_B8G8, YUY2, an unknown one
const FMTS: [i64; 13] = [28, 29, 87, 91, 90, 89, 41, 71, 45, 68, 107, 0, 200];

/// a description of the mirror's words
fn res_desc(a: &[i64], at: usize) -> Option<crate::model::d3dres_check::Desc> {
    let w = |k: usize| arg::<u32>(a, at.checked_add(k)?);
    Some(crate::model::d3dres_check::Desc { dim: w(0)?, w: w(1)?, h: w(2)?, da: w(3)?, mips: w(4)?, fmt: w(5)?, usage: w(6)?, bind: w(7)?, cpu: w(8)?, misc: w(9)?, count: w(10)?, q: w(11)? })
}

fn res_dev(a: &[i64]) -> Option<crate::model::d3dres::Dev> {
    use crate::model::d3dres::Dev;
    let l = arg::<u32>(a, 1)?;
    match arg::<u8>(a, 0)? {
        0 => Some(Dev::D10),
        1 => Some(Dev::D101(l)),
        2 => Some(Dev::D11(l)),
        _ => None,
    }
}

/// `n` descriptions of the values above on every device, their format's facts from the tables
/// (or, one in five, made up)
fn descs(n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::XorShift32::new(seed);
    for _ in 0..n {
        let (dk, lv) = *rng.pick(&DEVS);
        let mut v = vec![dk, lv, *rng.pick(&DIMS), *rng.pick(&SIZES), *rng.pick(&SIZES), *rng.pick(&DEPTHS), *rng.pick(&MIPS), rng.range(0, 133), *rng.pick(&USAGES), *rng.pick(&BINDS), *rng.pick(&CPUS), *rng.pick(&MISCS), *rng.pick(&COUNTS), *rng.pick(&QS), rng.range(0, 1)];
        if rng.chance(2) {
            // (near a description that is taken: one of NEAR with one or two words changed)
            let base = *rng.pick(&NEAR);
            for (i, x) in base.iter().enumerate() {
                if let Some(y) = v.get_mut(i.saturating_add(2)) {
                    *y = *x;
                }
            }
            for _ in 0..rng.range(1, 2) {
                let k = usize::try_from(rng.range(0, 12)).unwrap_or(0);
                let fresh = match k {
                    0 => *rng.pick(&DIMS),
                    1 | 2 => *rng.pick(&SIZES),
                    3 => *rng.pick(&DEPTHS),
                    4 => *rng.pick(&MIPS),
                    5 => *rng.pick(&FMTS),
                    6 => *rng.pick(&USAGES),
                    7 => *rng.pick(&BINDS),
                    8 => *rng.pick(&CPUS),
                    9 => *rng.pick(&MISCS),
                    10 => *rng.pick(&COUNTS),
                    11 => *rng.pick(&QS),
                    _ => rng.range(0, 1),
                };
                if let Some(y) = v.get_mut(k.saturating_add(2)) {
                    *y = fresh;
                }
            }
        }
        if rng.chance(3) {
            // (small sizes, where the rest of the checks decide)
            for i in [3usize, 4] {
                if let Some(x) = v.get_mut(i) {
                    *x = rng.range(1, 8);
                }
            }
            if let Some(x) = v.get_mut(5) {
                *x = *rng.pick(&[1, 1, 2, 6, 12]);
            }
        }
        let Some(d) = res_desc(&v, 2) else { continue };
        let Some(dev) = res_dev(&v) else { continue };
        let f = if rng.chance(5) {
            let bits = [0x10, 0x20, 0x40, 0x80, 0x100, 0x1000, 0x4000, 0x10000, 0x20000, 0x200_0000];
            let mut s = 0;
            for b in bits {
                if !rng.chance(4) {
                    s |= b;
                }
            }
            crate::model::d3dres_check::Facts { known: !rng.chance(10), s, bs: s ^ if rng.chance(3) { 0x4000 } else { 0 }, uav: if rng.chance(2) { s } else { 0 }, bw: *rng.pick(&[1, 2, 4]), bh: *rng.pick(&[1, 4]), q: [*rng.pick(&[0, 1, 4]), *rng.pick(&[0, 1, 0x8007_0057]), 1, 0] }
        } else {
            crate::model::d3dres_check::facts(dev, &d)
        };
        v.extend([i64::from(f.known), i64::from(f.s), i64::from(f.bs), i64::from(f.uav), i64::from(f.bw), i64::from(f.bh)]);
        v.extend(f.q.map(i64::from));
        each(&v);
    }
}

pub static DESC_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.desc",
        lean: DESC,
        domain: Domain::Sampled("600,000 descriptions over the devices, dimensions, sizes, slices, levels, every format, usages, binds, CPU access, flags, samples and qualities, their formats' facts from the tables (one in five made up)"),
        inputs: |each| descs(600_000, 0xde5c_d011, each),
        model: |a| {
            let d = res_desc(a, 2)?;
            let w = |k: usize| arg::<u32>(a, k);
            let fx = crate::model::d3dres_check::Facts { known: flag(a, 15)?, s: w(16)?, bs: w(17)?, uav: w(18)?, bw: w(19)?, bh: w(20)?, q: [w(21)?, w(22)?, w(23)?, w(24)?] };
            Some(match crate::model::d3dres_check::check_with(res_dev(a)?, &d, flag(a, 14)?, &fx) {
                Err(_) => vec![0],
                Ok(g) => [1, g.dim, g.w, g.h, g.da, g.mips, g.fmt, g.usage, g.bind, g.cpu, g.misc, g.count, g.q].map(i64::from).to_vec(),
            })
        },
    },
    Mirror {
        name: "d3d11.create",
        lean: DESC,
        domain: Domain::Sampled("the descriptions of d3d11.desc"),
        inputs: |each| descs(600_000, 0xde5c_d011, &mut |v| each(v.get(..14).unwrap_or(&[]))),
        model: |a| one(crate::model::d3dres_check::create_refused(res_dev(a)?, &res_desc(a, 2)?).is_some()),
    },
];

const VIEW: &str = "spec/lean/Kikizake/D3d11/View.lean";

/// resources views are asked of (dim, w, h, da, mips, fmt, usage, bind, cpu, misc, count, q):
/// 2D RGBA8 textures (an array of its typeless group, a cube, an array of cubes, multisampled
/// ones), depth textures (R24G8 typeless, D24S8), a 1D R32 float array, a 1D R32 typeless one, a
/// 3D one, buffers (plain, raw, structured), a YUY2 and a BGRA8 texture
const VIEW_RES: [[i64; 12]; 17] = [
    [2, 8, 8, 1, 4, 28, 0, 0xa8, 0, 0, 1, 0],
    [2, 8, 8, 3, 1, 27, 0, 0xa8, 0, 0, 1, 0],
    [2, 8, 8, 6, 3, 28, 0, 0x28, 0, 4, 1, 0],
    [2, 8, 8, 12, 1, 28, 0, 8, 0, 4, 1, 0],
    [2, 8, 8, 1, 1, 28, 0, 0x28, 0, 0, 4, 0],
    [2, 8, 8, 2, 1, 28, 0, 0x28, 0, 0, 4, 0],
    [2, 8, 8, 2, 1, 44, 0, 0x48, 0, 0, 1, 0],
    [2, 8, 8, 1, 1, 45, 0, 0x40, 0, 0, 1, 0],
    [2, 8, 8, 1, 1, 44, 0, 0x48, 0, 0, 4, 0],
    [1, 16, 1, 4, 3, 41, 0, 0xa8, 0, 0, 1, 0],
    [1, 16, 1, 1, 1, 39, 0, 0x88, 0, 0, 1, 0],
    [3, 8, 8, 4, 3, 28, 0, 0xa8, 0, 0, 1, 0],
    [4, 256, 1, 1, 1, 0, 0, 0xa8, 0, 0, 1, 0],
    [4, 256, 1, 1, 1, 0, 0, 0x88, 0, 0x20, 1, 0],
    [4, 256, 1, 1, 1, 0, 0, 0x88, 0, 0x40, 1, 16],
    [2, 8, 8, 1, 1, 107, 0, 8, 0, 0, 1, 0],
    [2, 8, 8, 1, 1, 87, 0, 0x28, 0, 0, 1, 0],
];
/// view formats: the resource's (0), RGBA8 and its group, R32 typeless, float, uint, sint,
/// R24G8 typeless, D24S8, R24 UNORM X8, X24 G8 UINT, YUY2, BGRA8 and its sRGB, RGBA32 float,
/// RGBA16 float, R8 UNORM, an unknown one
const VIEW_FMTS: [i64; 20] = [0, 0, 28, 29, 27, 39, 41, 42, 43, 44, 45, 46, 47, 107, 87, 91, 2, 10, 61, 200];
/// the union's words: firsts and counts near the resources' (and -1, -2), the flags
const VIEW_VALS: [i64; 20] = [0, 0, 1, 1, 2, 3, 4, 5, 6, 7, 8, 12, 16, 63, 64, 65, 256, U32, 0xffff_fffe, 0x8000_0000];

/// `n` views of the resources above on every device: half a resource's default view with a
/// word or two changed, half words at random; their formats' facts from the tables (or, one in
/// five, made up)
fn views(n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    use crate::model::d3dres_check as c;
    let mut rng = crate::rng::XorShift32::new(seed);
    for _ in 0..n {
        let (dk, lv) = *rng.pick(&DEVS);
        let kind = if rng.chance(50) { 4 } else { rng.range(0, 3) };
        let mut v = vec![dk, lv, kind];
        v.extend(rng.pick(&VIEW_RES).iter().copied());
        let Some(d) = res_desc(&v, 3) else { continue };
        let Some(dev) = res_dev(&v) else { continue };
        let ku = u32::try_from(kind).unwrap_or(0);
        let mut x: Vec<i64> = match c::default_view(dev, ku, &d) {
            Some(w) if rng.chance(2) => w.into_iter().map(i64::from).collect(),
            _ => vec![*rng.pick(&VIEW_FMTS), rng.range(0, 12), *rng.pick(&VIEW_VALS), *rng.pick(&VIEW_VALS), *rng.pick(&VIEW_VALS), *rng.pick(&VIEW_VALS), *rng.pick(&VIEW_VALS)],
        };
        x.resize(7, 0);
        for _ in 0..rng.range(0, 2) {
            let k = usize::try_from(rng.range(0, 6)).unwrap_or(0);
            let fresh = match k {
                0 => *rng.pick(&VIEW_FMTS),
                1 => rng.range(0, 12),
                _ => *rng.pick(&VIEW_VALS),
            };
            if let Some(y) = x.get_mut(k) {
                *y = fresh;
            }
        }
        let has = !rng.chance(5);
        let len = if rng.chance(4) { rng.range(0, 7) } else { 7 };
        v.extend([i64::from(has), len]);
        v.extend(x.iter().copied());
        v.push(i64::from(rng.chance(3)));
        let words: Option<Vec<u32>> = if has {
            let l = usize::try_from(len).unwrap_or(0);
            x.iter().take(l).map(|&y| u32::try_from(y).ok()).collect()
        } else {
            None
        };
        let f = if rng.chance(5) {
            let bits = [1, 0x100, 0x4000, 0x10000, 0x200_0000];
            let mut s = 0;
            for b in bits {
                if !rng.chance(4) {
                    s |= b;
                }
            }
            c::ViewFacts { known: !rng.chance(10), bits: *rng.pick(&[0, 8, 16, 32, 128]), typeless: rng.chance(4), res_typeless: rng.chance(2), same_group: rng.chance(2), res_tl32: rng.chance(2), s }
        } else {
            c::view_facts(dev, &d, c::view_format(&d, words.as_deref()))
        };
        v.extend([i64::from(f.known), i64::from(f.bits), i64::from(f.typeless), i64::from(f.res_typeless), i64::from(f.same_group), i64::from(f.res_tl32), i64::from(f.s)]);
        each(&v);
    }
}

pub static VIEW_MIRRORS: &[Mirror] = &[Mirror {
    name: "d3d11.view",
    lean: VIEW,
    domain: Domain::Sampled("600,000 views of 17 resources (textures of each dimension, arrays, cubes, multisampled, depth, typeless, buffers plain, raw and structured, YUY2) on every device and of each kind, half a default view with a word or two changed, with and without a description, short ones, made and only checked; their formats' facts from the tables (one in five made up)"),
    inputs: |each| views(600_000, 0x0d3d_1e55, each),
    model: |a| {
        let d = res_desc(a, 3)?;
        let w = |k: usize| arg::<u32>(a, k);
        let words: Option<Vec<u32>> = if flag(a, 15)? {
            let l = arg::<usize>(a, 16)?;
            (17..24).take(l).map(w).collect::<Option<Vec<u32>>>().map(Some)?
        } else {
            None
        };
        let fx = crate::model::d3dres_check::ViewFacts { known: flag(a, 25)?, bits: w(26)?, typeless: flag(a, 27)?, res_typeless: flag(a, 28)?, same_group: flag(a, 29)?, res_tl32: flag(a, 30)?, s: w(31)? };
        Some(match crate::model::d3dres_check::check_view_with(res_dev(a)?, w(2)?, &d, words.as_deref(), flag(a, 24)?, &fx) {
            Err(hr) => vec![0, i64::from(hr)],
            Ok(o) => std::iter::once(1).chain(o.into_iter().map(i64::from)).collect(),
        })
    },
}];
