//! Direct3D 10 and 11's mirrors (spec/lean/Kikizake/D3d11/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::d3draster::{shader_plan, ShaderPlan};
use crate::model::d3drules::{self, Api, Bind, Bound, Pick, Prim, TargetPlan, Topo, Verdict};

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
        name: "d3d11.shaderStage",
        lean: DRAW,
        domain: Domain::Every("both stages against every generated shader blob, with malformed mirror arguments"),
        inputs: |each| {
            for (index, _) in crate::shaders::Prog::ALL.iter().enumerate() {
                if let Ok(index) = i64::try_from(index) {
                    each(&[0, index]);
                    each(&[1, index]);
                }
            }
            for bad in [&[][..], &[2, 0], &[1, -1], &[1, 99]] {
                each(bad);
            }
        },
        model: |a| {
            let answer = || {
                use crate::shaders::{shaders, Prog, Programs};
                thread_local! {
                    static PROGRAMS: Programs = shaders();
                }
                let &[stage @ (0 | 1), p] = a else { return None };
                let prog = *Prog::ALL.get(usize::try_from(p).ok()?)?;
                let (accepted, found) = match PROGRAMS.with(|s| shader_plan(stage == 1, s.get(prog))) {
                    ShaderPlan::Accept(found) => (1, found),
                    ShaderPlan::Refuse(Some(found)) => (0, found),
                    ShaderPlan::Refuse(None) | ShaderPlan::Unknown => return None,
                };
                let id = Prog::ALL.iter().position(|&p| p == found)?;
                Some(vec![accepted, i64::try_from(id).ok()?])
            };
            Some(answer().unwrap_or_else(|| vec![-1]))
        },
    },
    Mirror {
        name: "d3d11.targetPlan",
        lean: DRAW,
        domain: Domain::Every("bound/unbound color and depth views, each with a distinct identity"),
        inputs: |each| every(&[(0, 1), (0, 1)], each),
        model: |a| Some(match a {
            &[rt, ds] => match d3drules::target_plan((rt == 1).then_some(10), (ds == 1).then_some(20)) {
                TargetPlan::Color(v) => vec![0, v],
                TargetPlan::DepthOnly(v) => vec![1, v],
                TargetPlan::None => vec![-1],
            },
            _ => vec![-1],
        }),
    },
    Mirror {
        name: "d3d11.targetRows",
        lean: DRAW,
        domain: Domain::Sampled("resource dimensions and view classes, every small pair of depth and slice including the limit, and 20,000 full u32 extent and offset combinations"),
        inputs: |each| {
            every(&[(0, 5), (0, 9), (4, 4), (5, 5), (0, 4), (0, 3), (0, 4)], each);
            sampled(&[(0, 5), (0, 9), (0, U32), (0, U32), (0, U32), (0, U32), (0, U32)], 20_000, 0x28f1_d011, each);
            for bad in [&[][..], &[1, 3, 4, -1, 0, 0, 0]] {
                each(bad);
            }
        },
        model: |a| {
            let answer = || {
                let [_, _, _, _, _, _, _] = a else { return None };
                let rows = d3drules::target_rows(arg(a, 0)?, arg(a, 1)?, (arg(a, 2)?, arg(a, 3)?, arg(a, 4)?), (arg(a, 5)?, arg(a, 6)?))?;
                Some([rows.width, rows.height, rows.first, rows.slice].into_iter().map(i64::from).collect())
            };
            Some(answer().unwrap_or_else(|| vec![-1]))
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
        name: "d3d11.srvOrZero",
        lean: TEXTURE,
        domain: Domain::Every("the two binding states plus a malformed argument"),
        inputs: |each| {
            each(&[0]);
            each(&[1]);
            each(&[-1]);
        },
        model: |a| {
            let bound = match *a {
                [0] => None,
                [1] => Some(13),
                _ => return Some(vec![-1]),
            };
            Some(match d3drules::srv_or_zero(bound) {
                Ok(view) => vec![view],
                Err(zero) => zero.into_iter().map(i64::from).collect(),
            })
        },
    },
    Mirror {
        name: "d3d11.ldReach",
        lean: TEXTURE,
        domain: Domain::Sampled("all small signed coordinates and dimensions, 30,000 full u32 and 20,000 full i64 coordinate inputs with full u32 view fields"),
        inputs: |each| {
            every(&[(-1, 4), (-1, 4), (0, 4), (0, 4), (29, 29), (3, 3)], each);
            sampled(&[(0, U32); 6], 30_000, 0x1da0_d011, each);
            sampled(&[(i64::MIN, i64::MAX), (i64::MIN, i64::MAX), (0, U32), (0, U32), (0, U32), (0, U32)], 20_000, 0x1dae_d011, each);
            for bad in [&[][..], &[0, 0, -1, 1, 0, 0], &[0, 0, 1, 0x1_0000_0000, 0, 0]] {
                each(bad);
            }
        },
        model: |a| {
            let answer = || {
                let &[x, y, _, _, _, _] = a else { return None };
                let result = d3drules::ld_reach((x, y), (arg(a, 2)?, arg(a, 3)?), arg(a, 4)?, arg(a, 5)?);
                Some(match result {
                    Ok(at) => vec![1, i64::from(at.x), i64::from(at.y), i64::from(at.format), i64::from(at.subresource)],
                    Err(zero) => std::iter::once(0).chain(zero.into_iter().map(i64::from)).collect(),
                })
            };
            Some(answer().unwrap_or_else(|| vec![-1]))
        },
    },
    Mirror {
        name: "d3d11.sampleReach",
        lean: TEXTURE,
        domain: Domain::Sampled("supported SRV dimensions and all small windows, mip and subresource overflow boundaries, and 10,000 full 32-bit fact inputs"),
        inputs: |each| {
            for (resource, view) in [(1, 2), (1, 3), (2, 4), (2, 5), (2, 6), (2, 7), (3, 8), (2, 9), (2, 10), (4, 1), (4, 4), (2, 11)] {
                every(&[(resource, resource), (view, view), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3)], each);
            }
            sampled(&[(1, 4), (1, 11), (0, U32), (0, U32), (0, U32), (0, U32), (0, U32), (0, U32)], 10_000, 0x5a6d_d011, each);
            for a in [
                &[2, 4, U32, 1, 0xffff_fffd, 2, 0, 1][..],
                &[2, 5, 0x10000, 0x10000, 0xffff, 1, 0xffff, 0],
                &[2, 5, 0x10001, 0x10001, 0, 1, 0x10000, 0],
                &[2, 5, U32, 3, 0, 2, 1, 0],
                &[2, 5, 4, 5, 1, U32, 3, 2],
                &[2, 5, 4, 5, 1, U32, 3, 3],
                &[2, 5, 4, 5, 1, 1, 3, -1],
                &[],
            ] {
                each(a);
            }
        },
        model: |a| {
            let answer = || {
                use crate::model::{d3dres::{Desc, Dev, Resource, View}, objects::{Objects, Slot}};
                let [_, _, _, _, _, _, _, _] = a else { return None };
                let d = Desc { dim: arg(a, 0)?, mips: arg(a, 2)?, da: arg(a, 3)?, w: 8, h: 8,
                    fmt: 28, usage: 0, bind: 8, cpu: 0, misc: 0, count: 1, q: 0 };
                let words = vec![28, arg(a, 1)?, arg(a, 4)?, arg(a, 5)?, arg(a, 6)?, 1];
                let level: u32 = arg(a, 7)?;
                let mut resources = Objects::default();
                let res = resources.put(Slot::new(0).ok()?, Resource { d, subs: Vec::new(), samples: Vec::new() });
                let view = View { res, kind: 0, words };
                let reach = d3drules::sample_reach(Dev::D11(0xb000), &view, resources.get(res).ok()?)?;
                let [mip, sub] = reach.at(level).map_or([-1, -1], |(mip, sub)| [i64::from(mip), i64::from(sub)]);
                Some(vec![i64::from(reach.base), i64::from(reach.mips), mip, sub])
            };
            Some(answer().unwrap_or_else(|| vec![-1]))
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
    let mut rng = crate::rng::Sample64::new(seed);
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
    let mut rng = crate::rng::Sample64::new(seed);
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

const FILTER: &str = "spec/lean/Kikizake/D3d11/Filter.lean";

/// binary32 values near the filters' arithmetic: zeros, the smallest subnormal, the smallest
/// normal, a quarter, a half and its neighbours, 1 and its neighbours, 2, 255, 2^23 and its
/// neighbour, 2^24, the largest finite value, infinity, a quiet and a signalling NaN
const FLOATS: [u32; 20] = [0, 1, 0x0080_0000, 0x3e80_0000, 0x3eff_ffff, 0x3f00_0000, 0x3f00_0001, 0x3f7f_ffff, 0x3f80_0000, 0x3f80_0001, 0x4000_0000, 0x437f_0000, 0x4b00_0000, 0x4b00_0001, 0x4b80_0000, 0x7f7f_ffff, 0x7f80_0000, 0x7fc0_0000, 0x7f80_0001, 0x7fc1_2345];
/// texture sizes: small ones, odd ones, the largest, past 2^24 (where n as f32 rounds), u32's
const TEXTURE_SIZES: [i64; 14] = [0, 1, 2, 3, 4, 5, 7, 8, 64, 255, 16384, (1 << 24) + 1, (1 << 25) + 3, U32];

/// each of FLOATS, signed; then `n` of: a coordinate in -1..2, of 0..1 as a multiple of 2^-k,
/// or any 32-bit pattern
fn floats(n: usize, rng: &mut crate::rng::Sample64, each: &mut dyn FnMut(u32)) {
    for &x in &FLOATS {
        each(x);
        each(x | 1 << 31);
    }
    for _ in 0..n {
        let v = match rng.range(0, 3) {
            0 => {
                let k = u32::try_from(rng.range(0, 12)).unwrap_or(0);
                (f64::from(u32::try_from(rng.range(0, 1 << 12)).unwrap_or(0)) / f64::from(1u32 << k)).to_string().parse::<f32>().unwrap_or(0.0).to_bits()
            }
            1 => (f64::from(u32::try_from(rng.range(0, 3_000_000)).unwrap_or(0)) / 1e6 - 1.0).to_string().parse::<f32>().unwrap_or(0.0).to_bits(),
            _ => u32::try_from(rng.range(0, U32)).unwrap_or(0),
        };
        each(v);
    }
}

/// coordinates and texture sizes
fn coords(seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let mut cs = Vec::new();
    floats(60_000, &mut rng, &mut |c| cs.push(i64::from(c)));
    for c in cs {
        let n = *rng.pick(&TEXTURE_SIZES);
        each(&[c, n]);
        each(&[c, rng.range(1, 4096)]);
    }
}

/// three floats: of FLOATS each with each, then texel values and weights, then any patterns
fn triples(seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let mut fs = Vec::new();
    floats(0, &mut rng, &mut |c| fs.push(i64::from(c)));
    for &a in &fs {
        for &b in &fs {
            each(&[a, b, *rng.pick(&fs)]);
        }
    }
    let mut more = Vec::new();
    floats(300_000, &mut rng, &mut |c| more.push(i64::from(c)));
    for w in more.chunks_exact(3) {
        if let [a, b, _] = *w {
            each(&[a, b, i64::from(crate::model::d3draster::pixels(rng.range(0, 256)).to_bits())]);
        }
        each(w);
    }
}

/// lods: whole and half levels, near a 1/256 boundary and its half, anywhere in 0..20, the
/// special values, any patterns; every last level to 15, point and linear
fn lods(seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let put = |x: f64, rng: &mut crate::rng::Sample64, each: &mut dyn FnMut(&[i64])| {
        let h = super::f64_halves(x);
        let (hi, lo) = (h.first().copied().unwrap_or(0), h.get(1).copied().unwrap_or(0));
        let (hi, lo) = if x.is_nan() { (0x7ff8_0000, 0) } else { (hi, lo) };
        each(&[hi, lo, rng.range(0, 15), rng.range(0, 1)]);
    };
    for x in [0.0, -0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 14.5, 15.0, 15.5, 16.0, 1e300, f64::INFINITY, f64::NEG_INFINITY, f64::NAN, f64::MIN_POSITIVE, 4e9, 5e9] {
        for _ in 0..40 {
            put(x, &mut rng, each);
        }
    }
    for _ in 0..200_000 {
        let k = f64::from(u32::try_from(rng.range(0, 16)).unwrap_or(0));
        let x = match rng.range(0, 4) {
            0 => k + f64::from(u32::try_from(rng.range(0, 512)).unwrap_or(0)) / 512.0,
            1 => k + (f64::from(u32::try_from(rng.range(0, 512)).unwrap_or(0)) / 512.0) + f64::from(u32::try_from(rng.range(0, 4)).unwrap_or(2)) * 1e-17 - 2e-17,
            2 => f64::from(u32::try_from(rng.range(0, U32)).unwrap_or(0)) / f64::from(u32::MAX) * 20.0,
            3 => f64::from_bits(u64::from(u32::try_from(rng.range(0, U32)).unwrap_or(0)) << 32 | u64::from(u32::try_from(rng.range(0, U32)).unwrap_or(0))),
            _ => k,
        };
        put(x, &mut rng, each);
    }
}

pub static FILTER_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.split",
        lean: FILTER,
        domain: Domain::Sampled("40 signed boundary floats and 60,000 coordinates (in -1..2, multiples of 2^-k in 0..1, any pattern), each with a texture size of 14 (0 to 2^32 - 1) and one of 1..4096"),
        inputs: |each| coords(0x5b17_d311, each),
        model: |a| {
            let (i, w) = crate::model::d3draster::split(f32::from_bits(arg(a, 0)?), arg(a, 1)?);
            Some(vec![i, w])
        },
    },
    Mirror {
        name: "d3d11.pointtexel",
        lean: FILTER,
        domain: Domain::Sampled("the coordinates and sizes of d3d11.split"),
        inputs: |each| coords(0x9017_d311, each),
        model: |a| one(crate::model::d3draster::point_texel(f32::from_bits(arg(a, 0)?), arg(a, 1)?)),
    },
    Mirror {
        name: "d3d11.linearfloat",
        lean: FILTER,
        domain: Domain::Every("every 8-bit filtered value, 0..32640 (in 1/128 of a byte)"),
        inputs: |each| every(&[(0, 32640)], each),
        model: |a| one(crate::model::d3draster::linear_float(arg(a, 0)?).to_bits()),
    },
    Mirror {
        name: "d3d11.pixels",
        lean: FILTER,
        domain: Domain::Every("every weight, 0..256 (in 1/256)"),
        inputs: |each| every(&[(0, 256)], each),
        model: |a| one(crate::model::d3draster::pixels(arg(a, 0)?).to_bits()),
    },
    Mirror {
        name: "d3d11.lerp",
        lean: FILTER,
        domain: Domain::Sampled("every pair of 40 signed boundary floats (NaNs quiet and signalling, infinities) with one of them as the weight, 100,000 texel pairs with a weight of 1/256s, 100,000 triples of coordinates or any patterns; NaNs compared bit for bit"),
        inputs: |each| triples(0x1e2b_d311, each),
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(crate::model::d3draster::lerp_sse(f(0)?, f(1)?, f(2)?).to_bits())
        },
    },
    Mirror {
        name: "d3d11.level",
        lean: FILTER,
        domain: Domain::Sampled("720 lods of 18 special values (whole and half levels, the largest, infinities, a NaN), 200,000 lods: in 1/512 of a level, a little off them, anywhere in 0..20, any binary64; each with a last level of 0..15, point and linear"),
        inputs: |each| lods(0x1e7e_d311, each),
        model: |a| {
            use crate::model::d3draster::Level;
            Some(match crate::model::d3draster::level(super::arg_f64(a, 0)?, arg(a, 2)?, flag(a, 3)?) {
                Level::One(n) => vec![0, i64::from(n)],
                Level::Two(n, w) => vec![1, i64::from(n), w],
            })
        },
    },
    Mirror {
        name: "d3d11.samplecmp",
        lean: FILTER,
        domain: Domain::Sampled("every function 0..9 with every pair of 40 signed boundary floats, and with 100,000 pairs of coordinates or any patterns"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x5c3b_d311);
            let mut fs = Vec::new();
            floats(200_000, &mut rng, &mut |c| fs.push(i64::from(c)));
            for f in 0..10 {
                for &r in fs.iter().take(40) {
                    for &t in fs.iter().take(40) {
                        each(&[f, r, t]);
                    }
                }
            }
            for p in fs.chunks_exact(2) {
                each(&[rng.range(0, 9), p.first().copied().unwrap_or(0), p.get(1).copied().unwrap_or(0)]);
            }
        },
        model: |a| one(crate::model::d3draster::sample_cmp(arg(a, 0)?, f32::from_bits(arg(a, 1)?), f32::from_bits(arg(a, 2)?)).to_bits()),
    },
    Mirror {
        name: "d3d11.pastdepth",
        lean: FILTER,
        domain: Domain::Every("depth views of 0..5 by 0..5, pixels 0..6 by 0..6"),
        inputs: |each| every(&[(0, 5), (0, 5), (0, 6), (0, 6)], each),
        model: |a| one(crate::model::d3draster::past_depth(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?)),
    },
];

const MERGE: &str = "spec/lean/Kikizake/D3d11/Merge.lean";

pub static MERGE_MIRRORS: &[Mirror] = &[Mirror {
    name: "d3d11.merge",
    lean: MERGE,
    domain: Domain::Sampled("every DXGI format's components with every mask 0..15 (and 16, 0xffffffff) over 40 pairs of elements each: zeros, ones, alternating bits, any bytes"),
    inputs: |each| {
        let mut rng = crate::rng::Sample64::new(0x3e26_d311);
        for id in 0..=200u32 {
            let Some(f) = crate::model::dxgifmt::format(id) else { continue };
            let bits: u32 = f.comps.iter().sum();
            let len = bits.div_ceil(8).max(1);
            let comps: Vec<(u32, u32)> = f.comps.iter().zip(f.chans).map(|(&b, &c)| (b, u32::from(c))).collect();
            for mask in (0..=16).chain([U32]) {
                for k in 0..40 {
                    let mut v = vec![i64::try_from(comps.len()).unwrap_or(0)];
                    for &(b, c) in &comps {
                        v.extend([i64::from(b), i64::from(c)]);
                    }
                    v.push(i64::from(len));
                    let byte = |rng: &mut crate::rng::Sample64| match k {
                        0 => 0,
                        1 => 0xff,
                        2 => 0x55,
                        3 => 0xaa,
                        _ => i64::from(rng.byte()),
                    };
                    for _ in 0..len.saturating_mul(2) {
                        v.push(byte(&mut rng));
                    }
                    v.push(mask);
                    each(&v);
                }
            }
        }
    },
    model: |a| {
        let n = arg::<usize>(a, 0)?;
        let comps = (0..n).map(|i| Some((arg::<u32>(a, i.checked_mul(2)?.checked_add(1)?)?, arg::<u32>(a, i.checked_mul(2)?.checked_add(2)?)?))).collect::<Option<Vec<_>>>()?;
        let at = n.checked_mul(2)?.checked_add(1)?;
        let len = arg::<usize>(a, at)?;
        let bytes = |from: usize| (from..from.checked_add(len)?).map(|i| arg::<u8>(a, i)).collect::<Option<Vec<u8>>>();
        let old = bytes(at.checked_add(1)?)?;
        let new = bytes(at.checked_add(1)?.checked_add(len)?)?;
        let mask = arg::<u32>(a, at.checked_add(1)?.checked_add(len.checked_mul(2)?)?)?;
        if a.len() != at.checked_add(2)?.checked_add(len.checked_mul(2)?)? {
            return None;
        }
        Some(crate::model::d3drules::merge(&comps, &old, &new, mask).into_iter().map(i64::from).collect())
    },
}];

const INTERP: &str = "spec/lean/Kikizake/D3d11/Interp.lean";

/// a NaN as the one quiet NaN
fn canon32(x: f32) -> i64 {
    if x.is_nan() { 0x7fc0_0000 } else { i64::from(x.to_bits()) }
}

/// `n` calls of `args` binary32 arguments: each one of the boundary floats (signed), a
/// coordinate (in -1..2, a multiple of 2^-k in 0..1), a position in 1/256 pixels of -64..64, a
/// pixel center, a viewport's size or any 32-bit pattern
fn float_args(args: usize, n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let mut pool = Vec::new();
    floats(4000, &mut rng, &mut |c| pool.push(i64::from(c)));
    for k in -64i64..=64 {
        pool.push(i64::from(crate::model::d3draster::pixels(k.saturating_mul(256).saturating_add(rng.range(0, 255))).to_bits()));
        pool.push(i64::from((f64::from(i32::try_from(k).unwrap_or(0)) + 0.5).to_string().parse::<f32>().unwrap_or(0.0).to_bits()));
    }
    for s in [1, 2, 3, 16, 64, 640, 4096] {
        pool.push(i64::from(f32::from(u16::try_from(s).unwrap_or(1)).to_bits()));
    }
    for _ in 0..n {
        let v: Vec<i64> = (0..args).map(|_| *rng.pick(&pool)).collect();
        each(&v);
    }
}

pub static INTERP_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.plane",
        lean: INTERP,
        domain: Domain::Sampled("200,000 triangles of 11 binary32 arguments (vertices, values, the point), each a boundary float, a coordinate, a 16.8 position, a pixel center, a size or any pattern; a NaN answer as the one quiet NaN"),
        inputs: |each| float_args(11, 200_000, 0x91a2_d311, each),
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::plane_value([f(0)?, f(1)?, f(2)?], [f(3)?, f(4)?, f(5)?], [f(6)?, f(7)?, f(8)?], f(9)?, f(10)?)))
        },
    },
    Mirror {
        name: "d3d11.sampledepthplane",
        lean: INTERP,
        domain: Domain::Sampled("all offsets in -8..7 per axis on a fixed depth plane with and without viewport clipping, 20,000 triangles and sample positions of the floats of d3d11.plane"),
        inputs: |each| {
            for hi in [0x4000_0000, 0x3fc0_0000] {
                for x in -8..=7 {
                    for y in -8..=7 {
                        each(&[0, 0x4080_0000, 0, 0, 0, 0x4080_0000, 0, 0x3f80_0000, 0x4000_0000,
                            0, hi, 0x4000_0000, 0x4000_0000, x, y]);
                    }
                }
            }
            let mut rng = crate::rng::Sample64::new(0x5a71_d311);
            float_args(13, 20_000, 0x5a72_d311, &mut |v| {
                let mut args = v.to_vec();
                args.extend([rng.range(-8, 7), rng.range(-8, 7)]);
                each(&args);
            });
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::sample_depth_plane(
                [f(0)?, f(1)?, f(2)?], [f(3)?, f(4)?, f(5)?], [f(6)?, f(7)?, f(8)?],
                (f(9)?, f(10)?), (f(11)?, f(12)?), (arg(a, 13)?, arg(a, 14)?),
            )))
        },
    },
    Mirror {
        name: "d3d11.sampledepthjobs",
        lean: INTERP,
        domain: Domain::Sampled("every coverage mask up to 8 samples with absent, short and complete depths, and 20,000 masks and depth lists up to 16 samples"),
        inputs: |each| {
            for width in 0..=8 {
                for mask in 0..(1i64 << width) {
                    each(&[mask, width, -1]);
                    each(&[mask, width, 0x8000_0000, 0]);
                    let mut args = vec![mask, width, 0x3f00_0000];
                    args.extend(0..width);
                    each(&args);
                }
            }
            let mut rng = crate::rng::Sample64::new(0x5a73_d311);
            for _ in 0..20_000 {
                let width = rng.range(0, 16);
                let mask = rng.range(0, (1i64 << width).saturating_sub(1));
                let center = if rng.chance(3) { -1 } else { rng.range(0, i64::from(u32::MAX)) };
                let mut args = vec![mask, width, center];
                for _ in 0..rng.range(0, 16) {
                    args.push(rng.range(0, i64::from(u32::MAX)));
                }
                each(&args);
            }
        },
        model: |a| {
            let mask: u16 = arg(a, 0)?;
            let width: u32 = arg(a, 1)?;
            if width > u16::BITS {
                return None;
            }
            let center = arg::<u32>(a, 2).map(f32::from_bits);
            let coverage: Vec<_> = (0..width).map(|k| mask & (1 << k) != 0).collect();
            let depths = a.get(3..)?.iter().map(|&bits| u32::try_from(bits).ok().map(f32::from_bits))
                .collect::<Option<Vec<_>>>()?;
            let depths = (!depths.is_empty()).then_some(depths.as_slice());
            let mut out = Vec::new();
            for (k, depth) in crate::model::d3drules::sample_depth_jobs(&coverage, center, depths) {
                out.extend([i64::try_from(k).ok()?, depth.map_or(-1, |z| i64::from(z.to_bits()))]);
            }
            Some(out)
        },
    },
    Mirror {
        name: "d3d11.samplespan",
        lean: INTERP,
        domain: Domain::Sampled("20,000 packed sample buffers and legal element offsets with one boundary violation each, plus zero and u32-limit sizes"),
        inputs: |each| {
            for bytes in [0, 1, 16, i64::from(u32::MAX)] {
                for count in [0, 1, 2, 16, i64::from(u32::MAX)] {
                    for off in [0, 1, bytes] {
                        each(&[bytes, count, 0, off, 1]);
                    }
                }
            }
            let mut rng = crate::rng::Sample64::new(0x5a74_d311);
            for _ in 0..20_000 {
                let count = rng.range(2, 16);
                let plane = rng.range(0, 1 << 22);
                let bytes = count.saturating_mul(plane);
                let k = rng.range(0, count.saturating_sub(1));
                let off = rng.range(0, plane);
                let elem = rng.range(0, plane.saturating_sub(off));
                each(&[bytes, count, k, off, elem]);
                match rng.range(0, 3) {
                    0 => each(&[bytes, 0, k, off, elem]),
                    1 => each(&[bytes, count, count, off, elem]),
                    2 => each(&[bytes, count, k, off, plane.saturating_sub(off).saturating_add(1)]),
                    _ => each(&[bytes.saturating_add(1), count, k, off, elem]),
                }
            }
        },
        model: |a| match crate::model::d3drules::sample_span(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?) {
            None => one(-1),
            Some((start, end)) => Some(vec![i64::try_from(start).ok()?, i64::try_from(end).ok()?]),
        },
    },
    Mirror {
        name: "d3d11.depthaction",
        lean: INTERP,
        domain: Domain::Sampled("every pair of signed boundary floats widened to binary64 for each comparison and flag, plus 20,000 sampled decisions with D16, D24 and binary64 operands"),
        inputs: |each| {
            let mut fs = Vec::new();
            floats(0, &mut crate::rng::Sample64::new(1), &mut |c| fs.push(f64::from(f32::from_bits(c))));
            fs.extend([65_535.0, 16_777_215.0]);
            let put = |func, write, enabled, q, stored, z, each: &mut dyn FnMut(&[i64])| {
                let mut args = vec![func, write, enabled];
                args.extend(super::f64_halves(q));
                args.extend(super::f64_halves(stored));
                args.push(z);
                each(&args);
            };
            for func in 0..=9 {
                for write in 0..=1 {
                    for enabled in 0..=1 {
                        for &q in &fs {
                            for &stored in &fs {
                                put(func, write, enabled, q, stored, 0x3f80_0000, each);
                            }
                        }
                    }
                }
            }
            let mut rng = crate::rng::Sample64::new(0x5a75_d311);
            for _ in 0..20_000 {
                let (q, stored) = match rng.range(0, 2) {
                    0 => (f64::from(u16::try_from(rng.range(0, i64::from(u16::MAX))).unwrap_or(0)),
                          f64::from(u16::try_from(rng.range(0, i64::from(u16::MAX))).unwrap_or(0))),
                    1 => (f64::from(u32::try_from(rng.range(0, 0xff_ffff)).unwrap_or(0)),
                          f64::from(u32::try_from(rng.range(0, 0xff_ffff)).unwrap_or(0))),
                    _ => (f64::from_bits(rng.next()), f64::from_bits(rng.next())),
                };
                put(rng.range(0, 9), rng.range(0, 1), rng.range(0, 1), q, stored,
                    rng.range(0, i64::from(u32::MAX)), each);
            }
        },
        model: |a| {
            use crate::model::d3drules::{depth_action, DepthAction};
            let values = (super::arg_f64(a, 3)?, super::arg_f64(a, 5)?, f32::from_bits(arg(a, 7)?));
            match depth_action(arg(a, 0)?, flag(a, 1)?, flag(a, 2)?.then_some(values)) {
                DepthAction::Reject => one(0),
                DepthAction::Keep => one(1),
                DepthAction::Write(depth) => Some(vec![2, i64::from(depth.to_bits())]),
            }
        },
    },
    Mirror {
        name: "d3d11.lineplane",
        lean: INTERP,
        domain: Domain::Sampled("200,000 lines: ends in 1/256 pixels of -2^20..2^20 (equal ones too), the coordinate and the ends' values from the floats of d3d11.plane"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x11e2_d311);
            float_args(3, 200_000, 0x11e3_d311, &mut |v| {
                let a = rng.range(-(1 << 20), 1 << 20);
                let b = if rng.chance(20) { a } else if rng.chance(2) { a.saturating_add(rng.range(-600, 600)) } else { rng.range(-(1 << 20), 1 << 20) };
                let mut w = vec![a, b];
                w.extend_from_slice(v);
                each(&w);
            });
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::line_value(arg(a, 0)?, arg(a, 1)?, f(2)?, f(3)?, f(4)?)))
        },
    },
    Mirror {
        name: "d3d11.clamprange",
        lean: INTERP,
        domain: Domain::Sampled("every triple of 40 signed boundary floats (NaNs, infinities, both zeros), 200,000 triples of the floats of d3d11.plane"),
        inputs: |each| {
            let mut fs = Vec::new();
            floats(0, &mut crate::rng::Sample64::new(1), &mut |c| fs.push(i64::from(c)));
            for &x in &fs {
                for &y in &fs {
                    for &z in &fs {
                        each(&[x, y, z]);
                    }
                }
            }
            float_args(3, 200_000, 0xc1a3_d311, each);
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::clamp_range(f(0)?, f(1)?, f(2)?)))
        },
    },
    Mirror {
        name: "d3d11.vertexdepth",
        lean: INTERP,
        domain: Domain::Sampled("200,000 quadruples (minz, maxz, z, w) of the floats of d3d11.plane"),
        inputs: |each| float_args(4, 200_000, 0xde97_d311, each),
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::vertex_depth(f(0)?, f(1)?, f(2)?, f(3)?)))
        },
    },
    Mirror {
        name: "d3d11.viewport",
        lean: INTERP,
        domain: Domain::Sampled("200,000 viewports and clip positions (x, y, width, height, x, y, w) of the floats of d3d11.plane"),
        inputs: |each| float_args(7, 200_000, 0x7e9a_d311, each),
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            let (x, y) = crate::model::d3draster::viewport_xy([f(0)?, f(1)?, f(2)?, f(3)?, 0.0, 1.0], &[f(4)?, f(5)?, 0.0, f(6)?]);
            Some(vec![x, y])
        },
    },
    Mirror {
        name: "d3d11.vp10",
        lean: INTERP,
        domain: Domain::Sampled("the 40 signed boundary floats and 300,000 of the floats of d3d11.plane"),
        inputs: |each| {
            let mut fs = Vec::new();
            floats(0, &mut crate::rng::Sample64::new(1), &mut |c| fs.push(i64::from(c)));
            for &x in &fs {
                each(&[x]);
            }
            float_args(1, 300_000, 0x7010_d311, each);
        },
        model: |a| one(canon32(crate::model::d3draster::vp10(f32::from_bits(arg(a, 0)?)))),
    },
    Mirror {
        name: "d3d11.cliplerp",
        lean: INTERP,
        domain: Domain::Sampled("200,000 quadruples (the two distances, the two values) of the floats of d3d11.plane"),
        inputs: |each| float_args(4, 200_000, 0xc11e_d311, each),
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            one(canon32(crate::model::d3draster::clip_lerp(f(0)?, f(1)?, f(2)?, f(3)?)))
        },
    },
    Mirror {
        name: "d3d11.madescreen",
        lean: INTERP,
        domain: Domain::Sampled("200,000 viewports and made vertices (x, y, width, height, minz, maxz, x, y, z, w) of the floats of d3d11.plane, with and without depth clipping"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x3ade_d311);
            float_args(10, 200_000, 0x3adf_d311, &mut |v| {
                let mut w = v.to_vec();
                w.push(i64::from(rng.chance(2)));
                each(&w);
            });
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            let ((x, y), z, rhw) = crate::model::d3draster::made_screen([f(0)?, f(1)?, f(2)?, f(3)?, f(4)?, f(5)?], &[f(6)?, f(7)?, f(8)?, f(9)?], flag(a, 10)?);
            Some(vec![x, y, canon32(z), canon32(rhw)])
        },
    },
];

const CLIP: &str = "spec/lean/Kikizake/D3d11/Clip.lean";

/// `n` calls of `args` binary32 clip-space values: mostly multiples of 1/8 in -4..4 (so that
/// vertices fall on a plane, behind the eye and at w = 0 too), one in 16 a float of
/// d3d11.plane; `flags` flags (0 or 1) after them
fn clip_args(args: usize, flags: usize, n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let mut pool = Vec::new();
    float_args(1, 4000, seed ^ 0x5a5a_5a5a, &mut |v| pool.extend_from_slice(v));
    for _ in 0..n {
        let mut v: Vec<i64> = (0..args)
            .map(|_| {
                if rng.chance(16) {
                    *rng.pick(&pool)
                } else {
                    let k = i32::try_from(rng.range(-32, 32)).unwrap_or(0);
                    i64::from((f64::from(k) / 8.0).to_string().parse::<f32>().unwrap_or(0.0).to_bits())
                }
            })
            .collect();
        for _ in 0..flags {
            v.push(rng.range(0, 1));
        }
        each(&v);
    }
}

/// four binary32 arguments from `i` on
fn vec4(a: &[i64], i: usize) -> Option<[f32; 4]> {
    let f = |k: usize| arg::<u32>(a, i.checked_add(k)?).map(f32::from_bits);
    Some([f(0)?, f(1)?, f(2)?, f(3)?])
}

pub static CLIP_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.cliptri",
        lean: CLIP,
        domain: Domain::Sampled("200,000 triangles (positions and colors of the values of clip_args), with and without depth clipping; a NaN answer as the one quiet NaN"),
        inputs: |each| clip_args(24, 1, 200_000, 0xc11f_d311, each),
        model: |a| {
            if a.len() != 25 {
                return None;
            }
            let v = |k: usize| Some((vec4(a, k.checked_mul(8)?)?, vec4(a, k.checked_mul(8)?.checked_add(4)?)?));
            let Some(p) = crate::model::d3draster::clip_tri_values([v(0)?, v(1)?, v(2)?], flag(a, 24)?) else { return Some(vec![0]) };
            let mut out = vec![1, i64::try_from(p.len()).ok()?];
            for (pos, color, made, along) in p {
                out.extend(pos.iter().chain(color.iter()).map(|&x| canon32(x)));
                out.extend([i64::from(made), i64::from(along)]);
            }
            Some(out)
        },
    },
    Mirror {
        name: "d3d11.clipxy",
        lean: CLIP,
        domain: Domain::Sampled("200,000 triangles' positions of the values of clip_args, clipped to the x and y planes, then (the flag) to the depth range"),
        inputs: |each| clip_args(12, 1, 200_000, 0xc1f0_d311, each),
        model: |a| {
            if a.len() != 13 {
                return None;
            }
            let p = crate::model::d3draster::clip_xy([vec4(a, 0)?, vec4(a, 4)?, vec4(a, 8)?]);
            let p = if flag(a, 12)? { crate::model::d3draster::clip_z(p) } else { p };
            let mut out = vec![i64::try_from(p.len()).ok()?];
            out.extend(p.iter().flat_map(|q| q.map(canon32)));
            Some(out)
        },
    },
    Mirror {
        name: "d3d11.clipline",
        lean: CLIP,
        domain: Domain::Sampled("200,000 lines' ends (positions and colors of the values of clip_args), clipped to the x and y planes or (the flag) to the depth range"),
        inputs: |each| clip_args(16, 1, 200_000, 0xc11a_d311, each),
        model: |a| {
            if a.len() != 17 {
                return None;
            }
            let e = |i: usize| Some((vec4(a, i)?, vec4(a, i.checked_add(4)?)?));
            let Some([p, q]) = crate::model::d3draster::clip_line_values(e(0)?, e(8)?, flag(a, 16)?) else { return Some(vec![0]) };
            let mut out = vec![1];
            out.extend([p.0, p.1, q.0, q.1].iter().flat_map(|v| v.map(canon32)));
            Some(out)
        },
    },
    Mirror {
        name: "d3d11.clippedaway",
        lean: CLIP,
        domain: Domain::Sampled("200,000 viewports (x, y of -2..2, sizes 1..64) and triangles' positions of the values of clip_args, clipped to the x and y planes"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xa3a7_d311);
            clip_args(12, 0, 200_000, 0xa3a8_d311, &mut |v| {
                let f = |x: i64| i64::from(f32::from(i16::try_from(x).unwrap_or(0)).to_bits());
                let mut w = vec![f(rng.range(-2, 2)), f(rng.range(-2, 2)), f(rng.range(1, 64)), f(rng.range(1, 64))];
                w.extend_from_slice(v);
                each(&w);
            });
        },
        model: |a| {
            if a.len() != 16 {
                return None;
            }
            let [vx, vy, vw, vh] = vec4(a, 0)?;
            let p = crate::model::d3draster::clip_xy([vec4(a, 4)?, vec4(a, 8)?, vec4(a, 12)?]);
            one(crate::model::d3draster::clipped_away([vx, vy, vw, vh, 0.0, 1.0], &p))
        },
    },
    Mirror {
        name: "d3d11.notfinite",
        lean: CLIP,
        domain: Domain::Sampled("200,000 triangles' positions of the values of clip_args"),
        inputs: |each| clip_args(12, 0, 200_000, 0x0f17_d311, each),
        model: |a| {
            if a.len() != 12 {
                return None;
            }
            one(crate::model::d3draster::not_finite([vec4(a, 0)?, vec4(a, 4)?, vec4(a, 8)?]))
        },
    },
    Mirror {
        name: "d3d11.depthaway",
        lean: CLIP,
        domain: Domain::Sampled("100,000 triples and 100,000 pairs of depths of the values of clip_args"),
        inputs: |each| {
            clip_args(3, 0, 100_000, 0xde9a_d311, each);
            clip_args(2, 0, 100_000, 0xde9b_d311, each);
        },
        model: |a| {
            let zs = a.iter().map(|&x| u32::try_from(x).ok().map(f32::from_bits)).collect::<Option<Vec<f32>>>()?;
            one(crate::model::d3draster::depth_away(&zs))
        },
    },
];

const COPY: &str = "spec/lean/Kikizake/D3d11/Copy.lean";

/// numbers as the mirror's arguments
fn nums(v: &[u64]) -> Option<Vec<i64>> {
    v.iter().map(|&x| i64::try_from(x).ok()).collect()
}

/// a box's six arguments from `i`
fn box6(a: &[i64], i: usize) -> Option<[u64; 6]> {
    let g = |k: usize| arg::<u64>(a, i.checked_add(k)?);
    Some([g(0)?, g(1)?, g(2)?, g(3)?, g(4)?, g(5)?])
}

/// three arguments from `i`
fn three(a: &[i64], i: usize) -> Option<[u64; 3]> {
    let g = |k: usize| arg::<u64>(a, i.checked_add(k)?);
    Some([g(0)?, g(1)?, g(2)?])
}

/// a format's facts as d3d11.copyfmt takes them: known, block, bits, numbers, components, all of
/// one width
fn fmt_facts(id: u32) -> [i64; 6] {
    use crate::model::dxgifmt::Num;
    let Some(f) = crate::model::dxgifmt::format(id) else { return [0; 6] };
    let num = match f.num {
        Num::Typeless => 0,
        Num::Unorm => 1,
        Num::Snorm => 2,
        Num::Uint => 3,
        Num::Sint => 4,
        Num::Float => 5,
        Num::Srgb => 6,
        Num::Other => 7,
    };
    let uniform = f.comps.first().is_some_and(|&c| f.comps.iter().all(|&x| x == c));
    [1, i64::from(f.block()), i64::from(f.bits), num, i64::try_from(f.comps.len()).unwrap_or(0), i64::from(uniform)]
}

fn whole_copy_facts(d: crate::model::d3dres::Desc) -> Vec<i64> {
    let [bw, bh] = crate::model::dxgifmt::format(d.fmt).map_or([1, 1], |f| [f.bw, f.bh]);
    [d.dim, d.w, d.h, d.da, d.mips, d.fmt, d.usage, d.count, crate::model::d3dres::group(d.fmt), bw, bh]
        .map(i64::from).into_iter().chain(fmt_facts(d.fmt)).collect()
}

fn whole_copy_side(words: &[i64]) -> Option<crate::model::d3dres::Desc> {
    use crate::model::d3dres::Desc;
    let v = |i| arg::<u32>(words, i);
    let d = Desc {
        dim: v(0)?, w: v(1)?, h: v(2)?, da: v(3)?, mips: v(4)?, fmt: v(5)?, usage: v(6)?, count: v(7)?,
        bind: 0, cpu: 0, misc: 0, q: 0,
    };
    (words == whole_copy_facts(d).as_slice()).then_some(d)
}

fn plan_code(p: crate::model::d3dres::Plan) -> Option<Vec<i64>> {
    use crate::model::d3dres::Plan;
    one(match p {
        Plan::Nothing => 0,
        Plan::Removed => 1,
        Plan::Copy => 2,
    })
}

pub static COPY_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.wordMean",
        lean: COPY,
        domain: Domain::Sampled("word endpoints and adjacent values, ties of both parities, and seeded ordered vectors for 1..32 samples"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x16d3_d011);
            for n in 1..=32 {
                for value in [0, 1, 32767, 32768, 65534, 65535] { each(&vec![value; n]); }
                for _ in 0..64 {
                    let words: Vec<_> = (0..n).map(|_| rng.range(0, 65535)).collect();
                    each(&words);
                }
            }
            for v in 0..=65534 { each(&[v, v+1]); }
            for bad in [&[][..], &[-1], &[65536], &[0; 33]] { each(bad); }
        },
        model: |a| {
            let mean = (1..=32).contains(&a.len()).then(|| a.iter().map(|&v| u16::try_from(v).ok()).collect::<Option<Vec<_>>>())
                .flatten().and_then(|v| crate::model::d3dres::word_resolve_mean(v.into_iter()));
            one(mean.map_or(-1, i64::from))
        },
    },
    Mirror {
        name: "d3d11.integerMean",
        lean: COPY,
        domain: Domain::Every("every possible UNORM field sum through 10 bits for 1, 2, 4, 8, 16 and 32 samples"),
        inputs: |each| {
            for count in [1, 2, 4, 8, 16, 32] {
                for sum in 0..=1023 * count { each(&[sum, count]); }
            }
            let mut rng = crate::rng::Sample64::new(0x18d3_d011);
            for _ in 0..128 {
                each(&[rng.range(-32736, -1), rng.range(1, 32)]);
                each(&[rng.range(0, 32736), rng.range(-32, 0)]);
            }
            each(&[]);
            each(&[1]);
            each(&[1, 2, 3]);
        },
        model: |a| {
            let result = match a {
                [s, n] => u64::try_from(*s).ok().zip(u64::try_from(*n).ok())
                    .and_then(|(s, n)| crate::model::d3dres::integer_resolve_mean(s, n))
                    .and_then(|v| i64::try_from(v).ok()),
                _ => None,
            };
            one(result.unwrap_or(-1))
        },
    },
    Mirror {
        name: "d3d11.box",
        lean: COPY,
        domain: Domain::Sampled("a box or none, flat or not, blocks 0..4 by 0..4 and boxes 0..3 each: every one; 300,000 of blocks 0..16, boxes of any 32-bit numbers, wholes 0..2^40"),
        inputs: |each| {
            every(&[(0, 1), (0, 1), (0, 4), (0, 4), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3), (2, 2), (3, 3), (1, 1)], each);
            sampled(&[(0, 1), (0, 1), (0, 16), (0, 16), (0, U32), (0, U32), (0, U32), (0, U32), (0, U32), (0, U32), (0, 1 << 40), (0, 1 << 40), (0, 1 << 40)], 300_000, 0x0b0c_d311, each);
        },
        model: |a| {
            let x: Vec<u32> = (4..10).map(|i| arg::<u32>(a, i)).collect::<Option<_>>()?;
            let bx = if flag(a, 0)? { Some(x.as_slice()) } else { None };
            nums(&crate::model::d3dres::box_elems(bx, flag(a, 1)?, (arg(a, 2)?, arg(a, 3)?), three(a, 10)?))
        },
    },
    Mirror {
        name: "d3d11.boxclip",
        lean: COPY,
        domain: Domain::Sampled("boxes and subresources 0..3 each: every one; 300,000 of 0..2^40 each"),
        inputs: |each| {
            every(&[(0, 3); 9], each);
            sampled(&[(0, 1 << 40); 9], 300_000, 0x0c11_d311, each);
        },
        model: |a| nums(&crate::model::d3dres::box_clip(box6(a, 0)?, three(a, 6)?)),
    },
    Mirror {
        name: "d3d11.update",
        lean: COPY,
        domain: Domain::Sampled("subresources 0..1 of 1..2, usages 0..1, 1 or 2 samples, boxes 0..2 each in subresources 1..2 each: every one; 300,000 of 0..5 each"),
        inputs: |each| {
            every(&[(0, 1), (1, 2), (0, 1), (1, 2), (0, 2), (0, 2), (0, 2), (0, 2), (0, 2), (0, 2), (1, 2), (1, 2), (1, 2)], each);
            sampled(&[(0, 5); 13], 300_000, 0x0bd8_d311, each);
        },
        model: |a| plan_code(crate::model::d3dres::update_plan(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, box6(a, 4)?, three(a, 10)?)),
    },
    Mirror {
        name: "d3d11.region",
        lean: COPY,
        domain: Domain::Sampled("ignored or not, each kind of sides, boxes 0..2 each, a source and a destination of 1..2 each, offsets 0..1: every one; 400,000 of 0..5 each"),
        inputs: |each| {
            every(&[(0, 1), (0, 3), (0, 2), (0, 2), (0, 2), (0, 2), (0, 2), (0, 2), (1, 2), (1, 2), (1, 1), (1, 2), (1, 2), (1, 1), (0, 1), (0, 1), (0, 0)], each);
            sampled(&[(0, 1), (0, 3), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5), (0, 5)], 400_000, 0x0e91_d311, each);
        },
        model: |a| {
            use crate::model::d3dres::Sides;
            let sides = match arg::<u8>(a, 1)? {
                0 => Sides::Buffers,
                1 => Sides::Textures { compatible: false },
                2 => Sides::Textures { compatible: true },
                3 => Sides::Mixed,
                _ => return None,
            };
            plan_code(crate::model::d3dres::region_plan(flag(a, 0)?, sides, box6(a, 2)?, three(a, 8)?, three(a, 11)?, three(a, 14)?))
        },
    },
    Mirror {
        name: "d3d11.resolve",
        lean: COPY,
        domain: Domain::Sampled("samples 0..4 each side, usages 0..1, each flag, sizes 0..2 each: every one; 300,000 of samples and usages of any 32-bit number, sizes 0..2^40"),
        inputs: |each| {
            every(&[(0, 4), (0, 4), (0, 1), (0, 1), (0, 1), (0, 1), (0, 2), (0, 2), (0, 2), (0, 2)], each);
            sampled(&[(0, U32), (0, U32), (0, U32), (0, 1), (0, 1), (0, 1), (0, 1 << 40), (0, 1 << 40), (0, 1 << 40), (0, 1 << 40)], 300_000, 0x0e50_d311, each);
        },
        model: |a| {
            let g = |i: usize| arg::<u64>(a, i);
            let plan = crate::model::d3dres::resolve_plan(arg(a, 0)?, (arg(a, 1)?, arg(a, 2)?), flag(a, 3)?, flag(a, 4)?, flag(a, 5)?, [g(6)?, g(7)?], [g(8)?, g(9)?]);
            plan_code(plan)
        },
    },
    Mirror {
        name: "d3d11.wholeCopy",
        lean: COPY,
        domain: Domain::Sampled("every format pair 0..200 on three devices, with equal texel sizes and equal storage grids, plus 10,000 descriptor pairs"),
        inputs: |each| {
            use crate::model::d3dres::Desc;
            let desc = Desc { dim: 2, w: 2, h: 2, da: 1, mips: 1, fmt: 0, usage: 0, count: 1, bind: 0, cpu: 0, misc: 0, q: 0 };
            let mut emit = |device, dst, src| {
                let mut input = vec![device];
                input.extend(whole_copy_facts(dst));
                input.extend(whole_copy_facts(src));
                each(&input);
            };
            let grid = |fmt| {
                let [bw, bh] = crate::model::dxgifmt::format(fmt).map_or([1, 1], |f| [f.bw, f.bh]);
                Desc { fmt, w: bw.saturating_mul(2), h: bh.saturating_mul(2), ..desc }
            };
            for dev in 0..3 {
                for a in 0..=200 {
                    for b in 0..=200 {
                        emit(dev, Desc { fmt: a, ..desc }, Desc { fmt: b, ..desc });
                        emit(dev, grid(a), grid(b));
                    }
                }
            }
            let mut rng = crate::rng::Sample64::new(0x76ee_11ca);
            let word = |value| u32::try_from(value).unwrap_or_default();
            for _ in 0..10_000 {
                let a = Desc {
                    dim: word(rng.range(1, 4)), w: word(rng.range(1, U32)), h: word(rng.range(1, U32)),
                    da: word(rng.range(1, 8)), mips: word(rng.range(1, 8)), count: word(rng.range(1, 4)),
                    usage: word(rng.range(0, 3)), fmt: word(rng.range(0, 200)), ..desc
                };
                let mut b = a;
                match rng.range(0, 7) {
                    0 => b.dim = word(rng.range(1, 4)),
                    1 => b.da = a.da.saturating_add(1),
                    2 => b.mips = a.mips.saturating_add(1),
                    3 => b.count = a.count.saturating_add(1),
                    4 => b.w = a.w.saturating_sub(1),
                    5 => b.h = a.h.saturating_sub(1),
                    6 => b.fmt = word(rng.range(0, 200)),
                    _ => {}
                }
                emit(rng.range(0, 2), a, b);
            }
        },
        model: |a| {
            use crate::model::d3dres::{Dev, copy_resource_allowed};
            let (&device, sides) = a.split_first()?;
            let (dst, src) = sides.split_at_checked(17)?;
            let dev = match device {
                0 => Dev::D10,
                1 => Dev::D101(0xa100),
                2 => Dev::D11(0xb000),
                _ => return None,
            };
            one(i64::from(copy_resource_allowed(dev, &whole_copy_side(dst)?, &whole_copy_side(src)?)))
        },
    },
    Mirror {
        name: "d3d11.copyfmt",
        lean: COPY,
        domain: Domain::Every("every pair of formats 0..200 (known to the model or not, with their typeless groups and facts from its tables) on d3d10.dll, 10.1 and 11"),
        inputs: |each| {
            for dev in 0..3 {
                for x in 0..=200u32 {
                    for y in 0..=200u32 {
                        let mut v = vec![dev, i64::from(x), i64::from(y), i64::from(crate::model::d3dres::group(x)), i64::from(crate::model::d3dres::group(y))];
                        v.extend(fmt_facts(x));
                        v.extend(fmt_facts(y));
                        each(&v);
                    }
                }
            }
        },
        model: |a| {
            use crate::model::d3dres::Dev;
            let dev = match arg::<u8>(a, 0)? {
                0 => Dev::D10,
                1 => Dev::D101(0xa100),
                2 => Dev::D11(0xb000),
                _ => return None,
            };
            let (x, y) = (arg::<u32>(a, 1)?, arg::<u32>(a, 2)?);
            let facts: Vec<i64> = [i64::from(crate::model::d3dres::group(x)), i64::from(crate::model::d3dres::group(y))].into_iter().chain(fmt_facts(x)).chain(fmt_facts(y)).collect();
            if a.get(3..)? != facts.as_slice() {
                return None;
            }
            one(i64::from(crate::model::d3dres::copy_compatible(x, y, dev)))
        },
    },
];

const LINE: &str = "spec/lean/Kikizake/D3d11/Line.lean";

/// `n` lines' ends in 1/256 pixels of -40..40 pixels (one in three on a pixel center, one in
/// three on a multiple of 1/4 pixel, equal ends now and then), and with `pixel` a pixel within
/// two of a point along the line
fn line_args(n: usize, pixel: bool, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let coord = |rng: &mut crate::rng::Sample64| match rng.range(0, 2) {
        0 => rng.range(-40, 40).saturating_mul(256).saturating_add(128),
        1 => rng.range(-160, 160).saturating_mul(64),
        _ => rng.range(-10_240, 10_240),
    };
    for _ in 0..n {
        let (ax, ay) = (coord(&mut rng), coord(&mut rng));
        let (bx, by) = if rng.chance(50) { (ax, ay) } else if rng.chance(3) { (ax.saturating_add(rng.range(-600, 600)), ay.saturating_add(rng.range(-600, 600))) } else { (coord(&mut rng), coord(&mut rng)) };
        let mut v = vec![ax, ay, bx, by];
        if pixel {
            let t = rng.range(0, 256);
            let at = |a: i64, b: i64| a.saturating_add(b.saturating_sub(a).saturating_mul(t).checked_div(256).unwrap_or(0)).div_euclid(256);
            v.extend([at(ax, bx).saturating_add(rng.range(-2, 2)), at(ay, by).saturating_add(rng.range(-2, 2))]);
        }
        each(&v);
    }
}

pub static LINE_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.linecovers",
        lean: LINE,
        domain: Domain::Sampled("300,000 aliased lines and pixels of line_args"),
        inputs: |each| line_args(300_000, true, 0x11c0_d311, each),
        model: |a| one(crate::model::d3draster::line_covers(pt(a, 0)?, pt(a, 2)?, pt(a, 4)?)),
    },
    Mirror {
        name: "d3d11.quadcorners",
        lean: LINE,
        domain: Domain::Sampled("200,000 multisample lines of line_args"),
        inputs: |each| line_args(200_000, false, 0x9ad0_d311, each),
        model: |a| Some(crate::model::d3draster::quad_corners(pt(a, 0)?, pt(a, 2)?).iter().flat_map(|&(x, y)| [x, y]).collect()),
    },
    Mirror {
        name: "d3d11.alphacorners",
        lean: LINE,
        domain: Domain::Sampled("200,000 alpha antialiased lines of line_args"),
        inputs: |each| line_args(200_000, false, 0xa1c0_d311, each),
        model: |a| Some(crate::model::d3draster::alpha_corners(pt(a, 0)?, pt(a, 2)?).iter().flat_map(|&(x, y)| [x, y]).collect()),
    },
    Mirror {
        name: "d3d11.alphacover",
        lean: LINE,
        domain: Domain::Sampled("200,000 alpha antialiased lines and pixels of line_args; a NaN answer as the one quiet NaN"),
        inputs: |each| line_args(200_000, true, 0xa1c1_d311, each),
        model: |a| one(canon32(crate::model::d3draster::alpha_coverage(pt(a, 0)?, pt(a, 2)?, pt(a, 4)?))),
    },
    Mirror {
        name: "d3d11.linesamples",
        lean: LINE,
        domain: Domain::Every("with the multisample state or not, every coverage of 0..8 samples and of 16"),
        inputs: |each| {
            for ms in 0..=1 {
                for n in (0..=8u32).chain([16]) {
                    for bits in 0..1u32 << n {
                        let mut v = vec![ms];
                        v.extend((0..n).map(|k| i64::from(bits >> k & 1)));
                        each(&v);
                    }
                }
            }
        },
        model: |a| {
            let covered = a.get(1..)?.iter().map(|&c| Some(c != 0)).collect::<Option<Vec<bool>>>()?;
            Some(crate::model::d3draster::line_samples(flag(a, 0)?, &covered).into_iter().map(i64::from).collect())
        },
    },
];

const LAYOUT: &str = "spec/lean/Kikizake/D3d11/Layout.lean";

pub static LAYOUT_MIRRORS: &[Mirror] = &[Mirror {
    name: "d3d11.layout",
    lean: LAYOUT,
    domain: Domain::Every("the 11 programs of LAYOUT_PROGRAMS and one other, every list of 0..8 elements of a format an input takes or not"),
    inputs: |each| {
        for p in 0..=11 {
            for n in 0..=8u32 {
                for bits in 0..1u32 << n {
                    let mut v = vec![p];
                    v.extend((0..n).map(|k| i64::from(bits >> k & 1)));
                    each(&v);
                }
            }
        }
    },
    model: |a| {
        use crate::model::d3draster::{layout_verdict, LayoutVerdict, LAYOUT_PROGRAMS};
        let name = LAYOUT_PROGRAMS.get(arg::<usize>(a, 0)?).copied().unwrap_or("other");
        let usable = a.get(1..)?.iter().map(|&c| c != 0).collect::<Vec<bool>>();
        one(match layout_verdict(name, &usable) {
            None => -1,
            Some(LayoutVerdict::Made) => 0,
            Some(LayoutVerdict::NoProgram) => 1,
            Some(LayoutVerdict::PixelProgram) => 2,
            Some(LayoutVerdict::Unfed) => 3,
        })
    },
}];

const LOD: &str = "spec/lean/Kikizake/D3d11/Lod.lean";

pub static LOD_MIRRORS: &[Mirror] = &[
    Mirror {
        name: "d3d11.sampler",
        lean: LOD,
        domain: Domain::Every("no sampler, and every sampler of five words 0..3"),
        inputs: |each| {
            each(&[0]);
            every(&[(1, 1), (0, 3), (0, 3), (0, 3), (0, 3), (0, 3)], each);
        },
        model: |a| {
            let s = match a {
                [0] => None,
                [1, ..] if a.len() == 6 => Some([arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?, arg(a, 5)?]),
                _ => return None,
            };
            Some(crate::model::d3draster::sampler_or_default(s).map(i64::from).to_vec())
        },
    },
    Mirror {
        name: "d3d11.filterlinear",
        lean: LOD,
        domain: Domain::Every("every filter 0..255"),
        inputs: |each| every(&[(0, 255)], each),
        model: |a| {
            Some(match crate::model::d3draster::filter_linear(arg(a, 0)?) {
                None => vec![-1],
                Some((mn, mg)) => vec![i64::from(mn), i64::from(mg)],
            })
        },
    },
    Mirror {
        name: "d3d11.quadspan",
        lean: LOD,
        domain: Domain::Sampled("200,000 quads: a corner's coordinates in 0..1 (multiples of 2^-k too), its neighbours' a little off it (exactly 1/w and 1/h too), and the 40 signed boundary floats now and then; widths of TEXTURE_SIZES (0 and 2^32 - 1 among them), heights 1..4096"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x59a0_d311);
            let mut bounds = Vec::new();
            floats(0, &mut crate::rng::Sample64::new(1), &mut |c| bounds.push(i64::from(c)));
            for _ in 0..200_000 {
                let (w, h) = (*rng.pick(&TEXTURE_SIZES), rng.range(1, 4096));
                let base = |rng: &mut crate::rng::Sample64| f64::from(u32::try_from(rng.range(0, 1 << 16)).unwrap_or(0)) / 65536.0;
                let near = |rng: &mut crate::rng::Sample64, c: f64, n: i64| match rng.range(0, 3) {
                    0 => c + f64::from(i32::try_from(rng.range(-3, 3)).unwrap_or(0)) / f64::from(i32::try_from(n).unwrap_or(1)),
                    1 => c + f64::from(i32::try_from(rng.range(-4000, 4000)).unwrap_or(0)) / 1e6,
                    _ => c,
                };
                let bits = |x: f64| i64::from(x.to_string().parse::<f32>().unwrap_or(0.0).to_bits());
                let (u, v) = (base(&mut rng), base(&mut rng));
                let mut q = vec![bits(u), bits(v), bits(near(&mut rng, u, w)), bits(near(&mut rng, v, h)), bits(near(&mut rng, u, w)), bits(near(&mut rng, v, h))];
                if rng.chance(20) {
                    let k = usize::try_from(rng.range(0, 5)).unwrap_or(0);
                    if let Some(slot) = q.get_mut(k) {
                        *slot = *rng.pick(&bounds);
                    }
                }
                q.extend([w, h]);
                each(&q);
            }
        },
        model: |a| {
            let f = |i: usize| arg::<u32>(a, i).map(f32::from_bits);
            let q = [[f(0)?, f(1)?], [f(2)?, f(3)?], [f(4)?, f(5)?]];
            Some(super::f64_halves(crate::model::d3draster::quad_span(q, arg(a, 6)?, arg(a, 7)?)))
        },
    },
    Mirror {
        name: "d3d11.filterpick",
        lean: LOD,
        domain: Domain::Sampled("mips 1..12, mip point and linear, 200,000 spans: about 1 (1 - 2^-10, 1, 1 + 2^-10, 1 + 2^-6, 1.1, 1.25), powers of two and between them, 0..2^13, a NaN, infinity; each with the model's f64::log2 of it (the platform's libm), and 20,000 with any binary64 as the lod"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xf1c7_d311);
            let special = [1.0 - 1.0 / 1024.0, 1.0, 1.0 + 1.0 / 1024.0, 1.0 + 1.0 / 64.0, 1.1, 1.25, 0.0, 0.5, 2.0, 4.0, 8.0, 3.0, 6.0, f64::NAN, f64::INFINITY];
            for k in 0..220_000 {
                let rho = match rng.range(0, 3) {
                    0 => *rng.pick(&special),
                    1 => f64::from(u32::try_from(rng.range(1, 1 << 20)).unwrap_or(1)) / 128.0,
                    _ => 2f64.powi(i32::try_from(rng.range(0, 12)).unwrap_or(0)) * (1.0 + f64::from(u32::try_from(rng.range(0, 1 << 10)).unwrap_or(0)) / 1024.0),
                };
                let lod = if k >= 200_000 { f64::from_bits(rng.next()) } else { rho.log2() };
                let mut v = vec![rng.range(1, 12), rng.range(0, 1)];
                v.extend(super::f64_halves(rho));
                v.extend(super::f64_halves(lod));
                each(&v);
            }
        },
        model: |a| {
            use crate::model::d3draster::{filter_pick, FilterPick, Level};
            Some(match filter_pick(arg(a, 0)?, flag(a, 1)?, super::arg_f64(a, 2)?, super::arg_f64(a, 4)?) {
                FilterPick::Mag => vec![0],
                FilterPick::Min => vec![1],
                FilterPick::Mip(Level::One(n)) => vec![2, 0, i64::from(n)],
                FilterPick::Mip(Level::Two(n, w)) => vec![2, 1, i64::from(n), w],
            })
        },
    },
];

const RESOLVE_MEAN: &str = "spec/lean/Kikizake/D3d11/ResolveMean.lean";

/// When an f32 operation returns NaN its payload and sign may vary with the compiler.
/// Compare the NaN class at the pixel boundary. The conversion mirrors check raw payloads.
fn canon_resolve_nan(f: &crate::model::dxgifmt::Format, out: &mut [u8]) {
    use crate::model::dxgifmt::Num;
    if f.num != Num::Float { return; }
    match f.comps.first() {
        Some(16) => {
            for word in out.as_chunks_mut::<2>().0 {
                let code = u16::from_le_bytes(*word);
                if code & 0x7fff > 0x7c00 { *word = 0x7e00u16.to_le_bytes(); }
            }
        }
        Some(32) => {
            for word in out.as_chunks_mut::<4>().0 {
                let code = u32::from_le_bytes(*word);
                if f32::from_bits(code).is_nan() { *word = 0x7fc0_0000u32.to_le_bytes(); }
            }
        }
        _ => {}
    }
}

pub static RESOLVE_MEAN_MIRRORS: &[Mirror] = &[Mirror {
    name: "d3d11.resolveMean",
    lean: RESOLVE_MEAN,
    domain: Domain::Sampled("all supported DXGI layouts, 1 to 32 samples, pairs of boundary texels, and 2,000 seeded byte vectors per layout"),
    inputs: |each| {
        use crate::model::dxgifmt::{format, Num};
        let mut rng = crate::rng::Sample64::new(0x18ce_a4d3);
        for id in 0..=200 {
            let width = format(id).and_then(|f| usize::try_from(f.bits / 8).ok()).unwrap_or(4);
            each(&[i64::from(id)]);
            let mut args = vec![i64::from(id)];
            args.extend(std::iter::repeat_n(0, width.saturating_mul(2)));
            each(&args);
            let Some(f) = format(id) else { continue };
            if !matches!(f.num, Num::Unorm | Num::Snorm | Num::Float | Num::Srgb) && id != 26 { continue; }

            let fill = |b: u8| vec![b; width];
            let mut edges: Vec<Vec<u8>> = [0, 1, 2, 0x7e, 0x7f, 0x80, 0x81, 0xfe, 0xff].into_iter().map(fill).collect();
            let mut repeat = |code: &[u8]| edges.push(code.iter().copied().cycle().take(width).collect());
            if f.num == Num::Float {
                if f.comps.first() == Some(&16) {
                    for code in [0x0001u16, 0x03ff, 0x0400, 0x3c00, 0x7bff, 0x7c00, 0x7c01, 0x8000, 0xbc00, 0xfc00] {
                        repeat(&code.to_le_bytes());
                    }
                } else {
                    for code in [1u32, 0x007f_ffff, 0x0080_0000, 0x3f80_0000, 0x7f7f_ffff, 0x7f80_0000,
                        0x7f80_0001, 0x8000_0000, 0xbf80_0000, 0xff80_0000] {
                        repeat(&code.to_le_bytes());
                    }
                }
            }
            if id == 26 {
                for code in [0u32, 1, 0x3e0, 0x3e1, 0x7c0, 0x7c1, 0xf800_0000, 0xf840_0000] {
                    repeat(&code.to_le_bytes());
                }
            }
            for a in &edges {
                for b in &edges {
                    args.truncate(1);
                    args.extend(a.iter().chain(b).copied().map(i64::from));
                    each(&args);
                }
            }
            if f.num == Num::Float && f.comps.first() == Some(&32) {
                let large = 0x60ad_78ecu32;
                let one = 1.0f32.to_bits();
                let negative = large | 0x8000_0000;
                for ordered in [[large, negative, one], [large, one, negative]] {
                    args.truncate(1);
                    for code in ordered {
                        args.extend(code.to_le_bytes().into_iter().cycle().take(width).map(i64::from));
                    }
                    each(&args);
                }
            }
            for i in 0usize..2000 {
                let count = (i % 32).saturating_add(1);
                args.truncate(1);
                args.extend((0..count.saturating_mul(width)).map(|_| rng.range(0, 255)));
                each(&args);
            }
            for invalid in [[-1, 0], [256, 0]] {
                args.truncate(1);
                args.extend(invalid.into_iter().cycle().take(width.saturating_mul(2)));
                each(&args);
            }
        }
        each(&[]);
        each(&[-1, 0]);
        each(&[28, 0]);
        let mut long = vec![61];
        long.resize(34, 0);
        each(&long);
    },
    model: |a| {
        use crate::model::{d3dres::Mean, dxgifmt::format};
        let answer = || {
            let f = format(arg::<u32>(a, 0)?)?;
            let mean = Mean::of(f)?;
            let width = usize::try_from(f.bits / 8).ok().filter(|&w| w != 0)?;
            let data = a.get(1..)?.iter().map(|&b| u8::try_from(b).ok()).collect::<Option<Vec<_>>>()?;
            let samples = data.chunks_exact(width);
            let n = samples.len();
            if !samples.remainder().is_empty() || !(1..=32).contains(&n) { return None; }
            let mut out = mean.resolve(f, n, |i| samples.clone().nth(i).unwrap_or_default()).ok()?;
            canon_resolve_nan(f, &mut out);
            Some(out.into_iter().map(i64::from).collect::<Vec<_>>())
        };
        Some(answer().unwrap_or_else(|| vec![-1]))
    },
}];
