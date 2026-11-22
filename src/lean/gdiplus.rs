//! GDI+'s mirrors (spec/lean/Kikizake/GdiPlus/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::gdiplus::{self, Depth, Pf, Round};
use crate::model::gppath::{self, Kind, Path, Point};

const CONV: &str = "spec/lean/Kikizake/GdiPlus/Conv.lean";
const BLEND: &str = "spec/lean/Kikizake/GdiPlus/Blend.lean";
const MATRIX: &str = "spec/lean/Kikizake/GdiPlus/Matrix.lean";
const FLATTEN: &str = "spec/lean/Kikizake/GdiPlus/Flatten.lean";
const DITHER: &str = "spec/lean/Kikizake/GdiPlus/Dither.lean";

/// the formats the mirror converts, by their place in `Pf` (the indexed ones and 16bppGrayScale
/// convert through tables and refusals the Lean file does not hold)
fn pf(i: i64) -> Option<Pf> {
    Some(match i {
        4 => Pf::Rgb555,
        5 => Pf::Rgb565,
        6 => Pf::Argb1555,
        7 => Pf::Rgb24,
        8 => Pf::Rgb32,
        9 => Pf::Argb32,
        10 => Pf::Pargb32,
        11 => Pf::Rgb48,
        12 => Pf::Argb64,
        13 => Pf::Pargb64,
        _ => return None,
    })
}

/// a format's largest value (its bits all set)
fn largest(f: i64) -> u64 {
    match f {
        4..=6 => 0xffff,
        7 => 0xff_ffff,
        8..=10 => 0xffff_ffff,
        11 => 0xffff_ffff_ffff,
        _ => u64::MAX,
    }
}

/// a 64-bit value's halves as mirror arguments, the high one first (Mirror.lean's nat64)
fn halves(v: u64) -> [i64; 2] {
    [i64::from(u32::try_from(v >> 32).unwrap_or(0)), i64::from(u32::try_from(v & 0xffff_ffff).unwrap_or(0))]
}

/// the 64-bit value of arguments i, i + 1 (halves)
fn arg_u64(a: &[i64], i: usize) -> Option<u64> {
    Some(u64::from(arg::<u32>(a, i)?) << 32 | u64::from(arg::<u32>(a, i.checked_add(1)?)?))
}

/// `n` samples of a format's values as halves, its ends first: the high half over its own range,
/// so every bit of a 48 or 64-bit pixel varies (the review of 84b05c8, 4.1: drawn as one 32-bit
/// word, the 64-bit formats' alpha was 0 in all but one sample)
fn samples_of(f: i64, n: usize, seed: u32, each: &mut dyn FnMut([i64; 2])) {
    let max = largest(f);
    let [hi, lo] = halves(max);
    let lo = if hi == 0 { lo } else { 0xffff_ffff };
    sampled(&[(0, hi), (0, lo)], n, seed, &mut |v| {
        if let [h, l] = *v {
            each([h, l]);
        }
    });
}

/// every value of the 16-bit formats; a sample of the 24, 32, 48 and 64-bit ones
fn values(each: &mut dyn FnMut(&[i64])) {
    for f in 4..=6 {
        every(&[(0, 65535)], &mut |v| {
            if let [v] = *v {
                each(&[f, 0, v]);
            }
        });
    }
    for f in 7..=13 {
        samples_of(f, 100_000, 0x6d00_0000 ^ u32::try_from(f).unwrap_or(0), &mut |[h, l]| each(&[f, h, l]));
    }
}

/// a depth of an indexed format by its bits
fn depth(d: i64) -> Option<Depth> {
    Some(match d {
        1 => Depth::One,
        4 => Depth::Four,
        8 => Depth::Eight,
        _ => return None,
    })
}

/// pairs of formats with values of the first: every 16-bit value from the 16-bit formats, samples
/// of the others
fn pairs(each: &mut dyn FnMut(&[i64])) {
    for a in 4..=13 {
        for b in 4..=13 {
            samples_of(a, 4000, 0x7100_0000 ^ u32::try_from(a).unwrap_or(0) << 4 ^ u32::try_from(b).unwrap_or(0), &mut |[h, l]| each(&[a, b, h, l]));
        }
    }
}

/// a NaN as the one quiet NaN
fn canon(x: f32) -> i64 {
    if x.is_nan() { 0x7fc0_0000 } else { i64::from(x.to_bits()) }
}

fn rounding(d: i64) -> Round {
    if d == 0 { Round::Nearest } else { Round::Down }
}

/// the f32 arguments from `from` on
fn floats<const N: usize>(a: &[i64], from: usize) -> Option<[f32; N]> {
    let mut v = [0f32; N];
    for (i, x) in v.iter_mut().enumerate() {
        *x = f32::from_bits(arg::<u32>(a, from.checked_add(i)?)?);
    }
    Some(v)
}

/// the special values: zeros, 1, 2, 3, 0.5, 0.1, the smallest subnormal, the largest finite
/// value, infinity, a NaN (each signed by the draw)
const SPECIAL: [u32; 11] = [0, 0x3f80_0000, 0x4000_0000, 0x4040_0000, 0x3f00_0000, 0x3dcc_cccd, 1, 0x7f7f_ffff, 0x7f80_0000, 0x7fc0_0000, 0x0080_0000];

/// rounding (0 to nearest, 1 down) and n f32 bit patterns, drawn from three pools: any pattern, a
/// value of magnitude 2^-7 ..= 2^13, small integers and the special values (where sums cancel to
/// exact zeros)
fn draws(n: usize, count: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    let mut v = vec![0i64; n.saturating_add(1)];
    for k in 0..count {
        if let Some(d) = v.first_mut() {
            *d = i64::from(u8::from(k % 2 == 1));
        }
        for x in v.iter_mut().skip(1) {
            let sign = if rng.chance(2) { 0x8000_0000 } else { 0 };
            let b = match rng.range(0, 3) {
                0 => rng.word(),
                1 => u32::try_from(rng.range(0x3c00_0000, 0x4600_0000)).unwrap_or(0) | sign,
                2 => f32::from(u8::try_from(rng.range(0, 6)).unwrap_or(0)).to_bits() | sign,
                _ => *rng.pick(&SPECIAL) | sign,
            };
            *x = i64::from(b);
        }
        each(&v);
    }
}

/// a float the paths take: a multiple of 1/8 in -64..64 mostly, a moderate value, any pattern
fn coord(rng: &mut crate::rng::XorShift32) -> u32 {
    match rng.range(0, 7) {
        0 => rng.next(),
        1 | 2 => u32::try_from(rng.range(0x3c00_0000, 0x4300_0000)).unwrap_or(0) | if rng.chance(2) { 0x8000_0000 } else { 0 },
        _ => (f32::from(i16::try_from(rng.range(-512, 512)).unwrap_or(0)) / 8.0).to_bits(),
    }
}

/// a path's coordinate: a multiple of 1/8 in -64..64 or a moderate value (any pattern makes
/// Beziers of billions of points)
fn path_coord(rng: &mut crate::rng::XorShift32) -> u32 {
    match rng.range(0, 3) {
        0 => u32::try_from(rng.range(0x3c00_0000, 0x4280_0000)).unwrap_or(0) | if rng.chance(2) { 0x8000_0000 } else { 0 },
        _ => (f32::from(i16::try_from(rng.range(-512, 512)).unwrap_or(0)) / 8.0).to_bits(),
    }
}

/// a matrix element: near 1 or 0 (within GpMatrix's tolerance or just past it), a small integer,
/// a moderate value
fn element(rng: &mut crate::rng::XorShift32, diagonal: bool) -> u32 {
    let base: f32 = if diagonal { 1.0 } else { 0.0 };
    match rng.range(0, 5) {
        0 => base.to_bits(),
        1 => (base + f32::from(i16::try_from(rng.range(-40, 40)).unwrap_or(0)) * 2e-5).to_bits(),
        2 => f32::from(i8::try_from(rng.range(-4, 4)).unwrap_or(0)).to_bits(),
        3 => (f32::from(i16::try_from(rng.range(-4000, 4000)).unwrap_or(0)) / 512.0).to_bits(),
        _ => coord(rng),
    }
}

/// a path's matrix element: as element gives it, but never far from 1
fn path_element(rng: &mut crate::rng::XorShift32, diagonal: bool) -> u32 {
    let e = element(rng, diagonal);
    if f32::from_bits(e).abs() < 16.0 { e } else { e & 0x80ff_ffff | 0x3f00_0000 }
}

/// a path for Flatten: rounding, a matrix or none, the flatness, then (x, y, type) points of one
/// to three figures of lines and Beziers, closed or not
fn paths(count: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::XorShift32::new(seed);
    for k in 0..count {
        let mut v = vec![i64::from(u8::from(k % 2 == 1)), i64::from(u8::from(rng.chance(2)))];
        for i in 0..6 {
            v.push(i64::from(path_element(&mut rng, i == 0 || i == 3)));
        }
        let fl: f32 = match rng.range(0, 5) {
            0 => 0.25,
            1 => 0.0,
            2 => 1.0,
            3 => f32::from(i16::try_from(rng.range(1, 400)).unwrap_or(0)) / 64.0,
            _ => f32::from_bits(u32::try_from(rng.range(0x3a00_0000, 0x4100_0000)).unwrap_or(0)),
        };
        v.push(i64::from(fl.to_bits()));
        for _ in 0..rng.range(1, 3) {
            v.extend([i64::from(path_coord(&mut rng)), i64::from(path_coord(&mut rng)), 0]);
            for _ in 0..rng.range(0, 3) {
                if rng.chance(2) {
                    v.extend([i64::from(path_coord(&mut rng)), i64::from(path_coord(&mut rng)), 1]);
                } else {
                    for _ in 0..3 {
                        v.extend([i64::from(path_coord(&mut rng)), i64::from(path_coord(&mut rng)), 3]);
                    }
                }
            }
            if rng.chance(2) {
                if let Some(t) = v.last_mut() {
                    *t |= 128;
                }
            }
        }
        each(&v);
    }
}

/// four control points: spans within 14 bits, past them, far apart
fn controls(count: usize, seed: u32, span: i64, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::XorShift32::new(seed);
    for _ in 0..count {
        let (ox, oy) = (rng.range(span.saturating_neg(), span), rng.range(span.saturating_neg(), span));
        let s = match rng.range(0, 3) {
            0 => 64,
            1 => 2000,
            _ => span,
        };
        let v: Vec<i64> = [ox, oy, ox, oy, ox, oy, ox, oy].iter().map(|&o| o.saturating_add(rng.range(s.saturating_neg(), s))).collect();
        each(&v);
    }
}

fn int_pairs(a: &[i64]) -> Option<[(i32, i32); 4]> {
    let mut q = [(0i32, 0i32); 4];
    for (p, xy) in q.iter_mut().zip(a.chunks(2)) {
        *p = (arg(xy, 0)?, arg(xy, 1)?);
    }
    Some(q)
}

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "gp.premultiply",
        lean: CONV,
        domain: Domain::Every("color and alpha 0..255"),
        inputs: |each| every(&[(0, 255), (0, 255)], each),
        model: |a| one(i64::try_from(gdiplus::premultiply(arg(a, 0)?, arg(a, 1)?)).ok()?),
    },
    Mirror {
        name: "gp.unpremultiply",
        lean: CONV,
        domain: Domain::Every("color and alpha 0..255"),
        inputs: |each| every(&[(0, 255), (0, 255)], each),
        model: |a| one(gdiplus::unpremultiply(arg(a, 0)?, arg(a, 1)?)),
    },
    Mirror {
        name: "gp.widen",
        lean: CONV,
        domain: Domain::Every("every 8-bit level"),
        inputs: |each| every(&[(0, 255)], each),
        model: |a| one(i64::try_from(gdiplus::widen(arg(a, 0)?)).ok()?),
    },
    Mirror {
        name: "gp.narrow",
        lean: CONV,
        domain: Domain::Every("every 16-bit word"),
        inputs: |each| every(&[(0, 65535)], each),
        model: |a| one(gdiplus::narrow(arg(a, 0)?)),
    },
    Mirror {
        name: "gp.to_argb",
        lean: CONV,
        domain: Domain::Sampled("every value of the 16-bit formats, samples of the others"),
        inputs: values,
        model: |a| one(gdiplus::to_argb(pf(arg(a, 0)?)?, arg_u64(a, 1)?, &[]).ok()?),
    },
    Mirror {
        name: "gp.from_argb",
        lean: CONV,
        domain: Domain::Sampled("every ARGB value's channels apart: samples of 32-bit colors into each format"),
        inputs: |each| {
            for f in 4..=13 {
                sampled(&[(0, 0xffff_ffff)], 100_000, 0x6f00_0000 ^ u32::try_from(f).unwrap_or(0), &mut |c| {
                    if let [c] = *c {
                        each(&[f, c]);
                    }
                });
            }
        },
        model: |a| one(i64::try_from(gdiplus::from_argb(pf(arg(a, 0)?)?, arg(a, 1)?).ok()?).ok()?),
    },
    Mirror {
        name: "gp.lanes",
        lean: BLEND,
        domain: Domain::Sampled("the 16-bit words' products: samples of d, f in 0..65535"),
        inputs: |each| sampled(&[(0, 65535), (0, 65535)], 200_000, 0x7200_0000, each),
        model: |a| one(i64::try_from(gdiplus::lanes(arg(a, 0)?, arg(a, 1)?)).ok()?),
    },
    Mirror {
        name: "gp.hatch_mix",
        lean: BLEND,
        domain: Domain::Every("every fore and back channel, the weights 0, 64, 234 and 256"),
        inputs: |each| {
            for w in [0, 64, 234, 256] {
                every(&[(0, 255), (0, 255)], &mut |v| {
                    if let [f, b] = *v {
                        each(&[f, b, w]);
                    }
                });
            }
        },
        model: |a| one(gdiplus::hatch_mix(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
    Mirror {
        name: "gp.gamma234",
        lean: BLEND,
        domain: Domain::Every("every fore and back alpha"),
        inputs: |each| every(&[(0, 255), (0, 255)], each),
        model: |a| one(gdiplus::gamma_alpha_234(arg(a, 0)?, arg(a, 1)?)),
    },
    Mirror {
        name: "gp.coverage_share",
        lean: BLEND,
        domain: Domain::Every("every channel, every count of 8 x 4 and 8 x 8 subsamples"),
        inputs: |each| {
            for full in [32, 64] {
                every(&[(0, 255), (0, full)], &mut |v| {
                    if let [c, k] = *v {
                        each(&[c, k, full]);
                    }
                });
            }
        },
        model: |a| one(gdiplus::coverage_share(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
    Mirror {
        name: "gp.convert",
        lean: BLEND,
        domain: Domain::Sampled("every pair of the formats but the indexed ones and 16bppGrayScale, samples of their values"),
        inputs: pairs,
        model: |a| Some(halves(gdiplus::convert(pf(arg(a, 0)?)?, pf(arg(a, 1)?)?, arg_u64(a, 2)?, &[]).ok()?).to_vec()),
    },
    Mirror {
        name: "gp.fixed_palette",
        lean: BLEND,
        domain: Domain::Every("every entry of the three fixed palettes, and their sizes and flags"),
        inputs: |each| {
            for (d, n) in [(1, 1), (4, 15), (8, 223)] {
                every(&[(-1, n)], &mut |k| {
                    if let [k] = *k {
                        each(&[d, k]);
                    }
                });
            }
        },
        model: |a| {
            let (p, flags) = gdiplus::fixed_palette(depth(arg(a, 0)?)?);
            match arg::<i64>(a, 1)? {
                -1 => Some(vec![i64::try_from(p.len()).ok()?, i64::from(flags)]),
                k => one(*p.get(usize::try_from(k).ok()?)?),
            }
        },
    },
    Mirror {
        name: "gp.round",
        lean: MATRIX,
        domain: Domain::Sampled("add, sub, mul and div to nearest and down: 400,000 pairs of any patterns, moderate values, small integers and special values"),
        inputs: |each| {
            for op in 0..4 {
                draws(2, 100_000, 0x6d70_0000 ^ op, &mut |v| {
                    let mut w = vec![i64::from(op)];
                    w.extend_from_slice(v);
                    each(&w);
                });
            }
        },
        model: |a| {
            let r = rounding(arg(a, 1)?);
            let [x, y] = floats::<2>(a, 2)?;
            one(canon(match arg::<u8>(a, 0)? {
                0 => r.add(x, y),
                1 => r.sub(x, y),
                2 => r.mul(x, y),
                3 => r.div(x, y),
                _ => return None,
            }))
        },
    },
    Mirror {
        name: "gp.matmul",
        lean: MATRIX,
        domain: Domain::Sampled("150,000 pairs of matrices, to nearest and down, of any patterns, moderate values, small integers and special values"),
        inputs: |each| draws(12, 150_000, 0x6d71_0000, each),
        model: |a| {
            let m = floats::<12>(a, 1)?;
            let x: [f32; 6] = m.get(..6)?.try_into().ok()?;
            let y: [f32; 6] = m.get(6..)?.try_into().ok()?;
            Some(gdiplus::mul_in(rounding(arg(a, 0)?), &x, &y).iter().map(|&v| canon(v)).collect())
        },
    },
    Mirror {
        name: "gp.matpoint",
        lean: MATRIX,
        domain: Domain::Sampled("150,000 matrices and points, to nearest and down, of any patterns, moderate values, small integers and special values"),
        inputs: |each| draws(8, 150_000, 0x6d72_0000, each),
        model: |a| {
            let m = floats::<8>(a, 1)?;
            let x: [f32; 6] = m.get(..6)?.try_into().ok()?;
            let (px, py) = gdiplus::transform_in(rounding(arg(a, 0)?), &x, *m.get(6)?, *m.get(7)?);
            Some(vec![canon(px), canon(py)])
        },
    },
    Mirror {
        name: "gp.gamma64",
        lean: MATRIX,
        domain: Domain::Sampled("to nearest and down, the alpha word of every 8-bit alpha: 400,000 source words at most it and destination words 0..8192"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x6d74_0064);
            for k in 0..400_000u32 {
                let sa = i64::try_from(gdiplus::widen_alpha(u64::from(k % 256))).unwrap_or(0);
                each(&[i64::from(u8::from(k % 512 >= 256)), rng.range(0, sa), rng.range(0, 8192), sa]);
            }
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            one(i64::try_from(gdiplus::mix64(r, arg(a, 1)?, arg(a, 2)?, gdiplus::gamma_rest(r, arg(a, 3)?))).ok()?)
        },
    },
    Mirror {
        name: "gp.matinv",
        lean: MATRIX,
        domain: Domain::Sampled("150,000 matrices, to nearest and down, of any patterns, moderate values, small integers and special values (singular ones among them)"),
        inputs: |each| draws(6, 150_000, 0x6d73_0000, each),
        model: |a| {
            let m = floats::<6>(a, 1)?;
            Some(gdiplus::inverse(rounding(arg(a, 0)?), &m).map_or_default(|e| e.iter().map(|&v| canon(v)).collect()))
        },
    },
    Mirror {
        name: "gp.ceil16",
        lean: FLATTEN,
        domain: Domain::Sampled("to nearest and down: 300,000 floats, 1/8 multiples, moderate values and any patterns"),
        inputs: |each| {
            let mut rng = crate::rng::XorShift32::new(0x6c16_0016);
            for k in 0..300_000u32 {
                each(&[i64::from(u8::from(k % 2 == 1)), i64::from(coord(&mut rng))]);
            }
        },
        model: |a| one(i64::from(gppath::ceil16(rounding(arg(a, 0)?), f32::from_bits(arg(a, 1)?)))),
    },
    Mirror {
        name: "gp.apply",
        lean: FLATTEN,
        domain: Domain::Sampled("to nearest and down: 300,000 matrices of elements near 1 and 0 within and past the tolerance, and points"),
        inputs: |each| {
            let mut rng = crate::rng::XorShift32::new(0x6c17_0017);
            for k in 0..300_000u32 {
                let mut v = vec![i64::from(u8::from(k % 2 == 1))];
                for i in 0..6 {
                    v.push(i64::from(element(&mut rng, i == 0 || i == 3)));
                }
                v.extend([i64::from(coord(&mut rng)), i64::from(coord(&mut rng))]);
                each(&v);
            }
        },
        model: |a| {
            let m = floats::<8>(a, 1)?;
            let x: [f32; 6] = m.get(..6)?.try_into().ok()?;
            let (px, py) = gppath::apply(rounding(arg(a, 0)?), &x, *m.get(6)?, *m.get(7)?);
            Some(vec![canon(px), canon(py)])
        },
    },
    Mirror {
        name: "gp.bezier32",
        lean: FLATTEN,
        domain: Domain::Sampled("200,000 sets of four control points spanning 64, 2000 and 40,000 (past 14 bits: none)"),
        inputs: |each| controls(200_000, 0x6c32_0032, 20_000, each),
        model: |a| Some(gppath::bezier32(int_pairs(a)?).map_or_default(|v| v.iter().flat_map(|&(x, y)| [i64::from(x), i64::from(y)]).collect())),
    },
    Mirror {
        name: "gp.bezier64",
        lean: FLATTEN,
        domain: Domain::Sampled("20,000 sets of four control points spanning 64, 2000 and 2^24"),
        inputs: |each| controls(20_000, 0x6c64_0064, 1 << 24, each),
        model: |a| Some(gppath::bezier64(int_pairs(a)?).iter().flat_map(|&(x, y)| [i64::from(x), i64::from(y)]).collect()),
    },
    Mirror {
        name: "gp.flatten",
        lean: FLATTEN,
        domain: Domain::Sampled("to nearest and down: 40,000 paths of one to three figures of lines and Beziers, closed or not, with and without a matrix, flatness 0, 1/4, 1 and others"),
        inputs: |each| paths(40_000, 0x6c70_0070, each),
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let m = if arg::<u8>(a, 1)? == 0 { None } else { Some(floats::<6>(a, 2)?) };
            let fl = f32::from_bits(arg(a, 8)?);
            let mut pts = Vec::new();
            let mut i = 9;
            while i < a.len() {
                let t: u32 = arg(a, i + 2)?;
                let kind = match t & 127 {
                    0 => Kind::Start,
                    1 => Kind::Line,
                    _ => Kind::Bezier,
                };
                pts.push(Point { x: f32::from_bits(arg(a, i)?), y: f32::from_bits(arg(a, i + 1)?), kind, close: t >= 128, inner: false });
                i = i.checked_add(3)?;
            }
            let mut path = Path::of(0, pts);
            path.flatten(r, m.as_ref(), fl).ok()?;
            Some(path.pts.iter().flat_map(|p| [canon(p.x), canon(p.y), i64::from(p.type_byte())]).collect())
        },
    },
    Mirror {
        name: "gp.dither_place",
        lean: DITHER,
        domain: Domain::Every("x86 and x86-64, texture and solid, every row of 1 ..= 7 pixels each left out, translucent or opaque; rows of 1 ..= 5: every fill from -1 to past the row, origins x 0 ..= 3; rows of 6 and 7: fills from -1 ..= 1 to bw - 1 ..= bw + 1, origin 0; every drawn pixel"),
        inputs: |each| {
            for bw in 1..=7u32 {
                let n = 3i64.pow(bw);
                let wide = bw > 5;
                for code in 0..n {
                    let cells: Vec<i64> = (0..bw).scan(code, |c, _| {
                        let d = c.rem_euclid(3);
                        *c = c.div_euclid(3);
                        Some(d)
                    }).collect();
                    let bwi = i64::from(bw);
                    for x0 in -1..=bwi {
                        for x1 in x0..=bwi.saturating_add(1) {
                            // (rows of 6 and 7: the fills from -1 ..= 1 to bw - 1 ..= bw + 1)
                            if wide && (x0 > 1 || x1 < bwi.saturating_sub(1)) {
                                continue;
                            }
                            for x in x0.max(0)..x1.min(bwi) {
                                for ox in if wide { 0..1 } else { 0..4 } {
                                    for (x86, tex) in [(1, 1), (0, 1), (1, 0)] {
                                        let mut v = vec![x86, tex, ox, -1, x0, x1, x, 2];
                                        v.extend_from_slice(&cells);
                                        each(&v);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        },
        model: |a| {
            let tex = arg::<u8>(a, 1)? != 0;
            let cells: Vec<Option<bool>> = a.get(8..)?.iter().map(|&c| (c != 0).then_some(c == 2)).collect();
            let col = if tex { gdiplus::dither_column(arg::<u8>(a, 0)? != 0, arg(a, 2)?, &cells, arg(a, 4)?, arg(a, 5)?, arg(a, 6)?) } else { arg(a, 6)? };
            let row = gdiplus::dither_row(tex.then_some(arg(a, 3)?), arg(a, 7)?);
            Some(vec![i64::from(col), i64::from(row)])
        },
    },
];
