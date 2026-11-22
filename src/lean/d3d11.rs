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
