//! GDI+'s mirrors (spec/lean/Kikizake/GdiPlus/).
use super::{arg, arg_f64, every, f64_halves, one, sampled, Domain, Mirror};
use crate::model::gdiplus::{self, Depth, Pf, Round};
use crate::model::gplinear::{self, Libm, Linear, Repeat, Span};
use crate::model::gppath::{self, Kind, Path, Point};

const CONV: &str = "spec/lean/Kikizake/GdiPlus/Conv.lean";
const BLEND: &str = "spec/lean/Kikizake/GdiPlus/Blend.lean";
const MATRIX: &str = "spec/lean/Kikizake/GdiPlus/Matrix.lean";
const FLATTEN: &str = "spec/lean/Kikizake/GdiPlus/Flatten.lean";
const DITHER: &str = "spec/lean/Kikizake/GdiPlus/Dither.lean";
const LINEAR: &str = "spec/lean/Kikizake/GdiPlus/Linear.lean";
const RECOLOR: &str = "spec/lean/Kikizake/GdiPlus/Recolor.lean";
const STROKE: &str = "spec/lean/Kikizake/GdiPlus/Stroke.lean";
const ARC: &str = "spec/lean/Kikizake/GdiPlus/Arc.lean";
const RASTER: &str = "spec/lean/Kikizake/GdiPlus/Raster.lean";
const LINE: &str = "spec/lean/Kikizake/GdiPlus/Line.lean";
const CHAIN: &str = "spec/lean/Kikizake/GdiPlus/Chain.lean";
const DASH: &str = "spec/lean/Kikizake/GdiPlus/Dash.lean";
const INSET: &str = "spec/lean/Kikizake/GdiPlus/Inset.lean";

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
fn coord(rng: &mut crate::rng::Sample64) -> u32 {
    match rng.range(0, 7) {
        0 => rng.word(),
        1 | 2 => u32::try_from(rng.range(0x3c00_0000, 0x4300_0000)).unwrap_or(0) | if rng.chance(2) { 0x8000_0000 } else { 0 },
        _ => (f32::from(i16::try_from(rng.range(-512, 512)).unwrap_or(0)) / 8.0).to_bits(),
    }
}

/// a path's coordinate: a multiple of 1/8 in -64..64 or a moderate value (any pattern makes
/// Beziers of billions of points)
fn path_coord(rng: &mut crate::rng::Sample64) -> u32 {
    match rng.range(0, 3) {
        0 => u32::try_from(rng.range(0x3c00_0000, 0x4280_0000)).unwrap_or(0) | if rng.chance(2) { 0x8000_0000 } else { 0 },
        _ => (f32::from(i16::try_from(rng.range(-512, 512)).unwrap_or(0)) / 8.0).to_bits(),
    }
}

/// a matrix element: near 1 or 0 (within GpMatrix's tolerance or just past it), a small integer,
/// a moderate value
fn element(rng: &mut crate::rng::Sample64, diagonal: bool) -> u32 {
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
fn path_element(rng: &mut crate::rng::Sample64, diagonal: bool) -> u32 {
    let e = element(rng, diagonal);
    if f32::from_bits(e).abs() < 16.0 { e } else { e & 0x80ff_ffff | 0x3f00_0000 }
}

/// a path for Flatten: rounding, a matrix or none, the flatness, then (x, y, type) points of one
/// to three figures of lines and Beziers, closed or not
fn paths(count: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
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
    let mut rng = crate::rng::Sample64::new(seed);
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

/// libm's answers to a line brush of these points, as the platform gives them (atan2, sin, cos;
/// 0 for a call not made)
fn linear_libm(r: Round, p1: (i32, i32), p2: (i32, i32), wrap: i64) -> [f64; 3] {
    let mut got = [0f64; 3];
    let _made = gplinear::new_with(r, p1, p2, (0, 0), wrap, &mut |c| {
        let v = gplinear::libm(c);
        let k = match c {
            Libm::Atan2(..) => 0,
            Libm::Sin(_) => 1,
            Libm::Cos(_) => 2,
        };
        if let Some(g) = got.get_mut(k) {
            *g = v;
        }
        v
    });
    got
}

/// the point coordinates the line brush mirrors draw from: small, the goldens' size, and the
/// int range
const LINEAR_SPANS: [i64; 4] = [8, 600, 1 << 24, 2_147_483_647];

/// `n` line brushes of each span: rounding, the two points and a WrapMode (-1 ..= 6)
fn linear_points(n: usize, seed: u32, each: &mut dyn FnMut(Round, (i32, i32), (i32, i32), i64)) {
    for (k, &m) in LINEAR_SPANS.iter().enumerate() {
        let seed = seed.wrapping_add(u32::try_from(k).unwrap_or(0));
        let lo = m.saturating_neg();
        sampled(&[(0, 1), (lo, m), (lo, m), (lo, m), (lo, m), (-1, 6)], n, seed, &mut |v| {
            let c = |i: usize| v.get(i).and_then(|&x| i32::try_from(x).ok()).unwrap_or(0);
            each(rounding(v.first().copied().unwrap_or(0)), (c(1), c(2)), (c(3), c(4)), v.get(5).copied().unwrap_or(0));
        });
        // (lines along an axis and its diagonals, whose rectangles are made square)
        sampled(&[(0, 1), (lo, m), (lo, m), (lo, m), (0, 3)], n / 4, seed ^ 0x55, &mut |v| {
            let c = |i: usize| v.get(i).and_then(|&x| i32::try_from(x).ok()).unwrap_or(0);
            let (x, y, d) = (c(1), c(2), c(3));
            let p2 = match v.get(4).copied().unwrap_or(0) {
                0 => (x.saturating_add(d), y),
                1 => (x, y.saturating_add(d)),
                2 => (x.saturating_add(d), y.saturating_add(d)),
                _ => (x.saturating_add(d), y.saturating_sub(d)),
            };
            each(rounding(v.first().copied().unwrap_or(0)), (x, y), p2, 0);
        });
    }
}

/// AddEllipse (kind 0), AddArc (1) or AddPie (2) onto a new path, libm answered by `m`
fn arc_case(path: &mut Path, r: Round, kind: i64, rect: (f32, f32, f32, f32), start: f32, sweep: f32, m: &mut dyn FnMut(gplinear::Libm) -> f64) -> Result<(), gppath::Refused> {
    match kind {
        0 => path.add_ellipse(r, rect),
        1 => path.add_arc_with(r, rect, start, sweep, m),
        _ => path.add_pie_with(r, rect, start, sweep, m),
    }
}

/// a fill: rounding, the half pixel offset, a matrix or none, Winding, the rows (0 aliased), the
/// bitmap's size, then (x, y, type) points of one to three figures of lines and Beziers on and
/// about it, closed or not
fn fills(count: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    for k in 0..count {
        let mut v = vec![i64::from(u8::from(k % 2 == 1)), rng.range(0, 1), i64::from(u8::from(rng.chance(2)))];
        for i in 0..6 {
            let e = path_element(&mut rng, i == 0 || i == 3);
            // (the translation within a few pixels)
            let e = if i >= 4 { (f32::from(i8::try_from(rng.range(-24, 24)).unwrap_or(0)) / 8.0).to_bits() } else { e };
            v.push(i64::from(e));
        }
        v.push(rng.range(0, 1));
        v.push(*rng.pick(&[0i64, 0, 1, 2, 4, 8]));
        let (w, h) = (rng.range(1, 12), rng.range(1, 10));
        v.extend([w, h]);
        let at = |rng: &mut crate::rng::Sample64, m: i64| i64::from((f32::from(i16::try_from(rng.range(-16, m.saturating_mul(8).saturating_add(16))).unwrap_or(0)) / 8.0).to_bits());
        for _ in 0..rng.range(1, 3) {
            v.extend([at(&mut rng, w), at(&mut rng, h), 0]);
            for _ in 0..rng.range(1, 4) {
                if rng.chance(3) {
                    for _ in 0..3 {
                        v.extend([at(&mut rng, w), at(&mut rng, h), 3]);
                    }
                } else {
                    v.extend([at(&mut rng, w), at(&mut rng, h), 1]);
                }
            }
            if rng.chance(2) {
                let n = v.len().saturating_sub(1);
                if let Some(t) = v.get_mut(n) {
                    *t |= 128;
                }
            }
        }
        each(&v);
    }
}

/// a 28.4 coordinate about a bitmap of n pixels: mostly on and about it, sometimes far
fn fixed(rng: &mut crate::rng::Sample64, n: i64) -> i64 {
    match rng.range(0, 9) {
        0 => rng.range(-1 << 20, 1 << 20),
        1 => rng.range(-(1 << 30), 1 << 30),
        2 => rng.range(-8, 8).saturating_add(rng.range(0, n).saturating_mul(16)),
        _ => rng.range(-40, n.saturating_mul(16).saturating_add(40)),
    }
}

/// a coverage's counts, row by row
fn counts(c: &gppath::Coverage) -> Vec<i64> {
    (0..c.h).flat_map(|y| (0..c.w).map(move |x| (x, y))).map(|(x, y)| i64::from(c.count(x, y))).collect()
}

/// the (x, y, type) path points from i on
fn path_points(a: &[i64], i: usize) -> Option<Vec<Point>> {
    let mut pts = Vec::new();
    for p in a.get(i..)?.chunks(3) {
        let t: u32 = arg(p, 2)?;
        let kind = match t & 127 {
            0 => Kind::Start,
            1 => Kind::Line,
            _ => Kind::Bezier,
        };
        pts.push(Point { x: f32::from_bits(arg(p, 0)?), y: f32::from_bits(arg(p, 1)?), kind, close: t >= 128, inner: false });
    }
    Some(pts)
}

fn round_flag(r: Round) -> i64 {
    match r {
        Round::Nearest => 0,
        Round::Down => 1,
    }
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
            let mut rng = crate::rng::Sample64::new(0x6c16_0016);
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
            let mut rng = crate::rng::Sample64::new(0x6c17_0017);
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
    Mirror {
        name: "gp.linear",
        lean: LINEAR,
        domain: Domain::Sampled("to nearest and down: 30,000 pairs of points of each of four spans (8, 600, 2^24, the int range) and WrapModes -1 ..= 6, and lines along the axes and diagonals; libm's atan2, sin and cos as the platform answers them"),
        inputs: |each| {
            linear_points(30_000, 0x11ea_0001, &mut |r, p1, p2, wrap| {
                let lib = linear_libm(r, p1, p2, wrap);
                let mut v = vec![round_flag(r), i64::from(p1.0), i64::from(p1.1), i64::from(p2.0), i64::from(p2.1), wrap];
                for x in lib {
                    v.extend(f64_halves(x));
                }
                each(&v);
            });
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let (la, ls, lc) = (arg_f64(a, 6)?, arg_f64(a, 8)?, arg_f64(a, 10)?);
            let mut calls = Vec::new();
            let got = gplinear::new_with(r, (arg(a, 1)?, arg(a, 2)?), (arg(a, 3)?, arg(a, 4)?), (0, 0), arg(a, 5)?, &mut |c| match c {
                Libm::Atan2(y, x) => {
                    calls.extend([y, x]);
                    la
                }
                Libm::Sin(t) => {
                    calls.push(t);
                    ls
                }
                Libm::Cos(t) => {
                    calls.push(t);
                    lc
                }
            });
            let st = match got {
                Ok(_) => 0,
                Err(gplinear::Refused::InvalidParameter) => 2,
                Err(gplinear::Refused::OutOfMemory) => 3,
            };
            let mut out = vec![st];
            out.extend(calls.into_iter().flat_map(f64_halves));
            if let Ok(l) = got {
                let (x, y, w, h) = l.rect;
                out.extend([x, y, w, h].into_iter().chain(l.matrix).map(canon));
                out.push(i64::from(l.repeat == Repeat::Mirror));
            }
            Some(out)
        },
    },
    Mirror {
        name: "gp.linear_span",
        lean: LINEAR,
        domain: Domain::Sampled("to nearest and down, with and without the half pixel offset: the brushes of 6,000 pairs of points of each span, random colors, four pixels each of 0 ..= 64 and of the 34-bit range; an identity product"),
        inputs: |each| {
            let mut k = 0u32;
            let mut rng = crate::rng::Sample64::new(0x11ea_0002);
            let emit = |r: Round, l: &Linear, rng: &mut crate::rng::Sample64, each: &mut dyn FnMut(&[i64])| {
                for half in [0, 1] {
                    let (x, y, w, h) = l.rect;
                    let mut v = vec![round_flag(r), half];
                    v.extend([x, y, w, h].into_iter().chain(l.matrix).map(|f| i64::from(f.to_bits())));
                    v.extend([i64::from(l.colors.0), i64::from(l.colors.1), i64::from(l.repeat == Repeat::Mirror)]);
                    for big in [false, false, true, true] {
                        let m = if big { 1i64 << 33 } else { 64 };
                        v.extend([rng.range(-m, m), rng.range(-m, m)]);
                    }
                    each(&v);
                }
            };
            linear_points(6_000, 0x11ea_0003, &mut |r, p1, p2, wrap| {
                k = k.wrapping_add(1);
                let colors = (u32::try_from(rng.range(0, 0xffff_ffff)).unwrap_or(0), u32::try_from(rng.range(0, 0xffff_ffff)).unwrap_or(0));
                // (an opaque, a clear and a translucent end among them)
                let colors = match k % 4 {
                    0 => (colors.0 | 0xff00_0000, colors.1 & 0x00ff_ffff),
                    1 => (colors.0 | 0xff00_0000, colors.1 | 0xff00_0000),
                    _ => colors,
                };
                if let Ok(l) = gplinear::new(r, p1, p2, colors, wrap) {
                    emit(r, &l, &mut rng, each);
                }
            });
            for r in [Round::Nearest, Round::Down] {
                for repeat in [Repeat::Tile, Repeat::Mirror] {
                    let l = Linear { rect: (0.0, 0.0, 1_048_576.0, 1_048_576.0), matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0], colors: (0xff20_4080, 0x80ff_8000), repeat };
                    emit(r, &l, &mut rng, each);
                }
            }
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let f = floats::<10>(a, 2)?;
            let repeat = if arg::<u8>(a, 14)? != 0 { Repeat::Mirror } else { Repeat::Tile };
            let l = Linear { rect: (f[0], f[1], f[2], f[3]), matrix: [f[4], f[5], f[6], f[7], f[8], f[9]], colors: (arg(a, 12)?, arg(a, 13)?), repeat };
            let Some(s) = Span::new(r, &l, arg::<u8>(a, 1)? != 0) else { return Some(vec![-1]) };
            let (du, dv, u0, ta, tb) = s.parts();
            let mut out = vec![i64::from(du), i64::from(dv), i64::from(u0)];
            out.extend(ta.iter().chain(tb.iter()).flatten().map(|&v| i64::from(v)));
            for px in a.get(15..)?.chunks(2) {
                if let [x, y] = *px {
                    out.push(i64::from(s.at(x, y)));
                }
            }
            Some(out)
        },
    },
    Mirror {
        name: "gp.recolor",
        lean: RECOLOR,
        domain: Domain::Sampled("300,000 straight colors through 5 x 5 matrices of special values, small steps and random bits"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x4ec0_0001);
            for _ in 0..300_000 {
                let mut v = vec![i64::from(rng.word())];
                let style = rng.range(0, 3);
                for i in 0..25 {
                    let e = match style {
                        // (near the identity: each entry 0 or 1 give or take a small step)
                        0 => {
                            let base = if i % 6 == 0 { 1.0f32 } else { 0.0 };
                            (base + f32::from(i16::try_from(rng.range(-64, 64)).unwrap_or(0)) / 128.0).to_bits()
                        }
                        1 => *rng.pick(&SPECIAL),
                        2 => (f32::from(i16::try_from(rng.range(-2000, 2000)).unwrap_or(0)) / 1000.0).to_bits(),
                        _ => rng.word(),
                    };
                    v.push(i64::from(e));
                }
                each(&v);
            }
        },
        model: |a| {
            let m = floats::<25>(a, 1)?;
            one(gdiplus::recolor(arg(a, 0)?, &gdiplus::ColorMatrix(m), gdiplus::MatrixFlags::Default))
        },
    },
    Mirror {
        name: "gp.sqrt32",
        lean: STROKE,
        domain: Domain::Sampled("to nearest and down: 200,000 binary32 values, the special ones and every exponent"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x5174_0001);
            for d in [0, 1] {
                for &x in &SPECIAL {
                    each(&[d, i64::from(x)]);
                    each(&[d, i64::from(x | 0x8000_0000)]);
                }
                for _ in 0..100_000 {
                    each(&[d, i64::from(rng.word() & 0x7fff_ffff)]);
                }
            }
        },
        model: |a| one(canon(gppath::sqrt32(rounding(arg(a, 0)?), f32::from_bits(arg(a, 1)?)))),
    },
    Mirror {
        name: "gp.axes",
        lean: STROKE,
        domain: Domain::Sampled("to nearest and down: 200,000 matrices of special values, small steps and rotations"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x5174_0002);
            for k in 0..200_000u32 {
                let mut v = vec![i64::from(k % 2)];
                for i in 0..6 {
                    v.push(i64::from(element(&mut rng, i == 0 || i == 3)));
                }
                each(&v);
            }
        },
        model: |a| {
            let (major, minor) = gppath::axes(rounding(arg(a, 0)?), &floats::<6>(a, 1)?);
            Some(vec![canon(major), canon(minor)])
        },
    },
    Mirror {
        name: "gp.stroke",
        lean: STROKE,
        domain: Domain::Sampled("to nearest and down: 200,000 pens (widths about 1.5 and special, every alpha class, caps, dashes) with and without a world matrix, aliased or not, dashed or not"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x5174_0003);
            let widths = [0.0f32, -1.0, 1.0, 1.5, 1.500_000_1, 1.499_999_9, 0.75, 2.0, 3.0, f32::NAN, f32::INFINITY];
            for k in 0..200_000u32 {
                let width = if rng.chance(2) { *rng.pick(&widths) } else { f32::from(i16::try_from(rng.range(-100, 400)).unwrap_or(0)) / 128.0 };
                let color = match rng.range(0, 2) {
                    0 => 0xff00_0000 | rng.word() & 0xff_ffff,
                    1 => rng.word() & 0x00ff_ffff,
                    _ => rng.word(),
                };
                let cap = |rng: &mut crate::rng::Sample64| if rng.chance(3) { rng.range(0, 255) } else { rng.range(0, 3) };
                let mut v = vec![i64::from(k % 2), i64::from(color), i64::from(width.to_bits()), rng.range(0, 6), cap(&mut rng), cap(&mut rng), rng.range(0, 1), rng.range(0, 1), rng.range(0, 1)];
                for i in 0..6 {
                    v.push(i64::from(if rng.chance(4) { if i == 0 || i == 3 { 0x3f80_0000 } else { 0 } } else { element(&mut rng, i == 0 || i == 3) }));
                }
                each(&v);
            }
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let mut pen = gppath::Pen::new(arg(a, 1)?, f32::from_bits(arg(a, 2)?));
            pen.dash = arg(a, 3)?;
            pen.start_cap = arg(a, 4)?;
            pen.end_cap = arg(a, 5)?;
            let world = if arg::<u8>(a, 8)? != 0 { Some(floats::<6>(a, 9)?) } else { None };
            let s = gppath::stroke(r, &pen, world.as_ref(), arg::<u8>(a, 6)? != 0, arg::<u8>(a, 7)? != 0);
            one(match s {
                gppath::Stroke::OnePixel => 0,
                gppath::Stroke::Diamond => 1,
                gppath::Stroke::DashedPixel => 2,
                gppath::Stroke::Wide => 3,
            })
        },
    },
    Mirror {
        name: "gp.arc",
        lean: ARC,
        domain: Domain::Sampled("to nearest and down: 60,000 ellipses, arcs and pies of rectangles circular or not, empty, tiny and large, angles in eighths of a degree and random, sweeps past +-360 and 0; libm's sin, cos and atan2 as the platform answers them"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x4a2c_0001);
            for k in 0..60_000u32 {
                let r = rounding(i64::from(k % 2));
                let kind = rng.range(0, 2);
                let side = |rng: &mut crate::rng::Sample64| match rng.range(0, 5) {
                    0 => 0.0f32,
                    1 => -f32::from(i16::try_from(rng.range(1, 80)).unwrap_or(0)) / 8.0,
                    2 => f32::from(i16::try_from(rng.range(1, 40)).unwrap_or(0)) / 1024.0,
                    3 => f32::from(i16::try_from(rng.range(1, 32000)).unwrap_or(0)),
                    _ => f32::from(i16::try_from(rng.range(1, 2000)).unwrap_or(0)) / 8.0,
                };
                let w = side(&mut rng);
                let h = if rng.chance(3) { w } else { side(&mut rng) };
                let (x, y) = (f32::from(i16::try_from(rng.range(-800, 800)).unwrap_or(0)) / 8.0, f32::from(i16::try_from(rng.range(-800, 800)).unwrap_or(0)) / 8.0);
                let angle = |rng: &mut crate::rng::Sample64| match rng.range(0, 4) {
                    0 => f32::from(i16::try_from(rng.range(-8, 8)).unwrap_or(0)) * 90.0,
                    1 => f32::from(i16::try_from(rng.range(-8000, 8000)).unwrap_or(0)) / 8.0,
                    2 => *rng.pick(&[0.0f32, -0.0, 360.0, -360.0, 360.5, -720.25, 1e-7, 89.999_99, f32::NAN]),
                    3 => f32::from(i16::try_from(rng.range(-100, 100)).unwrap_or(0)) * 0.001,
                    _ => f32::from_bits(rng.word()),
                };
                let (start, sweep) = (angle(&mut rng), angle(&mut rng));
                let mut v: Vec<i64> = [x, y, w, h, start, sweep].iter().map(|f| i64::from(f.to_bits())).collect();
                v.insert(0, kind);
                v.insert(0, i64::from(k % 2));
                let mut path = Path::of(0, Vec::new());
                let mut answer = |c: gplinear::Libm| {
                    let a = gplinear::libm(c);
                    v.extend(f64_halves(a));
                    a
                };
                let _made = arc_case(&mut path, r, kind, (x, y, w, h), start, sweep, &mut answer);
                each(&v);
            }
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let f = floats::<6>(a, 2)?;
            let mut lib = Vec::new();
            for p in a.get(8..)?.chunks(2) {
                lib.push(arg_f64(p, 0)?);
            }
            let mut calls = Vec::new();
            let mut k = 0usize;
            let mut answer = |c: gplinear::Libm| {
                match c {
                    gplinear::Libm::Atan2(y, x) => calls.extend([y, x]),
                    gplinear::Libm::Sin(t) | gplinear::Libm::Cos(t) => calls.push(t),
                }
                let v = lib.get(k).copied().unwrap_or(0.0);
                k = k.saturating_add(1);
                v
            };
            let mut path = Path::of(0, Vec::new());
            let st = match arc_case(&mut path, r, arg(a, 1)?, (f[0], f[1], f[2], f[3]), f[4], f[5], &mut answer) {
                Ok(()) => 0,
                Err(_) => 2,
            };
            let mut out = vec![st];
            out.extend(calls.into_iter().flat_map(f64_halves));
            out.extend(path.pts.iter().flat_map(|p| [canon(p.x), canon(p.y), i64::from(p.type_byte())]));
            Some(out)
        },
    },
    Mirror {
        name: "gp.fill",
        lean: RASTER,
        domain: Domain::Sampled("to nearest and down: 60,000 fills of one to three figures of lines and Beziers on and about bitmaps up to 12 x 10, with and without a matrix and the half pixel offset, Alternate and Winding, aliased and 8 x 1, 2, 4, 8 antialiased"),
        inputs: |each| fills(60_000, 0x4a57_0001, each),
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let po = if arg::<u8>(a, 1)? != 0 { gdiplus::PixelOffset::Half } else { gdiplus::PixelOffset::None };
            let m = if arg::<u8>(a, 2)? == 0 { None } else { Some(floats::<6>(a, 3)?) };
            let fm = if arg::<u8>(a, 9)? != 0 { gdiplus::FillMode::Winding } else { gdiplus::FillMode::Alternate };
            let rows = match arg::<u32>(a, 10)? {
                0 => None,
                n => Some(n),
            };
            let (w, h) = (arg::<u32>(a, 11)?, arg::<u32>(a, 12)?);
            let mut pts = Vec::new();
            for p in a.get(13..)?.chunks(3) {
                let t: u32 = arg(p, 2)?;
                let kind = match t & 127 {
                    0 => Kind::Start,
                    1 => Kind::Line,
                    _ => Kind::Bezier,
                };
                pts.push(Point { x: f32::from_bits(arg(p, 0)?), y: f32::from_bits(arg(p, 1)?), kind, close: t >= 128, inner: false });
            }
            let path = Path::of(0, pts);
            let Ok(cov) = gppath::fill(r, &path, m.as_ref(), fm, rows, po, w, h) else { return Some(vec![-1]) };
            Some((0..h).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| i64::from(cov.count(x, y))).collect())
        },
    },
    Mirror {
        name: "gp.dda",
        lean: LINE,
        domain: Domain::Sampled("to nearest and down: 300,000 segments between 28.4 points on, about and far from bitmaps up to 12 x 10, steep and shallow, diagonals and points, with and without the last pixel"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x11de_0001);
            for k in 0..300_000u32 {
                let (w, h) = (rng.range(1, 12), rng.range(1, 10));
                let (x1, y1) = (fixed(&mut rng, w), fixed(&mut rng, h));
                let (x2, y2) = match rng.range(0, 5) {
                    0 => (x1, y1),
                    1 => {
                        let t = rng.range(-200, 200);
                        (x1.saturating_add(t), y1.saturating_add(if rng.chance(2) { t } else { t.saturating_neg() }))
                    }
                    2 => (x1.saturating_add(rng.range(-3, 3)), y1.saturating_add(rng.range(-3, 3))),
                    _ => (fixed(&mut rng, w), fixed(&mut rng, h)),
                };
                let c = |v: i64| v.clamp(-2_147_483_648, 2_147_483_647);
                each(&[i64::from(k % 2), c(x1), c(y1), c(x2), c(y2), rng.range(0, 1), w, h]);
            }
        },
        model: |a| {
            let last = if arg::<u8>(a, 5)? != 0 { gppath::Last::Skipped } else { gppath::Last::Drawn };
            let cov = gppath::segment(rounding(arg(a, 0)?), (arg(a, 1)?, arg(a, 2)?), (arg(a, 3)?, arg(a, 4)?), last, arg(a, 6)?, arg(a, 7)?).ok()?;
            Some(counts(&cov))
        },
    },
    Mirror {
        name: "gp.sweep",
        lean: LINE,
        domain: Domain::Sampled("100,000 polylines of no to eight 28.4 points: repeats, straight runs, turns both ways and reversals onto the segment before, steps of every octant class"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x11de_0005);
            for _ in 0..100_000u32 {
                let (mut x, mut y) = (rng.range(-64, 64), rng.range(-64, 64));
                let mut step = (rng.range(-48, 48), rng.range(-48, 48));
                let mut v = Vec::new();
                for _ in 0..rng.range(0, 8) {
                    v.extend([x, y]);
                    step = match rng.range(0, 5) {
                        // (on, back the way it came, a repeat, a turn)
                        0 => step,
                        1 => (step.0.saturating_neg().saturating_mul(rng.range(1, 2)), step.1.saturating_neg().saturating_mul(rng.range(1, 2))),
                        2 => (0, 0),
                        _ => (rng.range(-48, 48), rng.range(-48, 48)),
                    };
                    (x, y) = (x.saturating_add(step.0), y.saturating_add(step.1));
                }
                each(&v);
            }
        },
        model: |a| {
            let pts = a.chunks_exact(2).map(|c| Some((i32::try_from(*c.first()?).ok()?, i32::try_from(*c.get(1)?).ok()?))).collect::<Option<Vec<_>>>()?;
            Some(match gppath::sweep(&pts) {
                None => vec![-1],
                Some(q) => q.iter().flat_map(|&(x, y)| [i64::from(x), i64::from(y)]).collect(),
            })
        },
    },
    Mirror {
        name: "gp.nominal",
        lean: LINE,
        domain: Domain::Sampled("100,000 sets of one to three polylines of one to six 28.4 points (repeats, turns of every octant class), closed or not, on bitmaps up to 10 x 8, aliased and 8 x 1, 2, 4, 8"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x11de_0002);
            for _ in 0..100_000u32 {
                let (w, h) = (rng.range(1, 10), rng.range(1, 8));
                let mut v = vec![*rng.pick(&[0i64, 0, 1, 2, 4, 8]), w, h];
                for _ in 0..rng.range(1, 3) {
                    let n = rng.range(1, 6);
                    v.extend([rng.range(0, 1), n]);
                    let (mut x, mut y) = (rng.range(-16, w.saturating_mul(16).saturating_add(16)), rng.range(-16, h.saturating_mul(16).saturating_add(16)));
                    for _ in 0..n {
                        v.extend([x, y]);
                        if !rng.chance(5) {
                            x = x.saturating_add(rng.range(-48, 48));
                            y = y.saturating_add(rng.range(-48, 48));
                        }
                    }
                }
                each(&v);
            }
        },
        model: |a| {
            let rows = match arg::<u32>(a, 0)? {
                0 => None,
                n => Some(n),
            };
            let mut figs = Vec::new();
            let mut i = 3usize;
            while i < a.len() {
                let closed = arg::<u8>(a, i)? != 0;
                let n: usize = arg(a, i.checked_add(1)?)?;
                let from = i.checked_add(2)?;
                let to = from.checked_add(n.checked_mul(2)?)?;
                let pts = a.get(from..to)?.chunks(2).map(|p| Some((arg(p, 0)?, arg(p, 1)?))).collect::<Option<Vec<(i32, i32)>>>()?;
                figs.push(gppath::Fig { pts, closed });
                i = to;
            }
            Some(counts(&gppath::nominal(&figs, rows, arg(a, 1)?, arg(a, 2)?)))
        },
    },
    Mirror {
        name: "gp.outline",
        lean: LINE,
        domain: Domain::Sampled("to nearest and down: 60,000 paths of lines and Beziers through device matrices, stroked by the DDA with and without open figures' last pixels and by the nominal rasterizer aliased and 8 x 4, 8 x 8"),
        inputs: |each| {
            fills(60_000, 0x11de_0003, &mut |f| {
                // (fills' rounding, offset, matrix, mode, rows, size, points: here how, rows, size, the device matrix)
                let get = |i: usize| f.get(i).copied().unwrap_or(0);
                let how = get(1).saturating_add(get(9).saturating_mul(2)).rem_euclid(3);
                let mut v = vec![get(0), how, *[0i64, 4, 8].get(usize::try_from(get(10).rem_euclid(3)).unwrap_or(0)).unwrap_or(&0), get(11), get(12)];
                let m = if get(2) == 0 { [0x3f80_0000i64, 0, 0, 0x3f80_0000, 0, 0] } else { [get(3), get(4), get(5), get(6), get(7), get(8)] };
                // (28.4: each element times 16)
                v.extend(m.iter().map(|&e| i64::from((f32::from_bits(u32::try_from(e).unwrap_or(0)) * 16.0).to_bits())));
                v.extend(f.get(13..).unwrap_or(&[]));
                each(&v);
            });
        },
        model: |a| {
            let r = rounding(arg(a, 0)?);
            let (w, h): (u32, u32) = (arg(a, 3)?, arg(a, 4)?);
            let m = floats::<6>(a, 5)?;
            let path = Path::of(0, path_points(a, 11)?);
            let cov = match arg::<u8>(a, 1)? {
                2 => {
                    let rows = match arg::<u32>(a, 2)? {
                        0 => None,
                        n => Some(n),
                    };
                    gppath::outline_nominal(r, &path, &m, rows, w, h)
                }
                k => gppath::outline_dda(r, &path, &m, if k == 1 { gppath::Last::Skipped } else { gppath::Last::Drawn }, w, h),
            };
            Some(cov.map_or_else(|_| vec![-1], |c| counts(&c)))
        },
    },
    Mirror {
        name: "gp.too_small",
        lean: LINE,
        domain: Domain::Sampled("to nearest and down: 200,000 sets of two to four points apart by 0, about 5.96e-4 and more, through matrices"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0x11de_0004);
            for k in 0..200_000u32 {
                let mut v = vec![i64::from(k % 2)];
                let plain = rng.chance(2);
                for i in 0..6 {
                    v.push(i64::from(if plain { if i == 0 || i == 3 { 0x3f80_0000 } else { 0 } } else { path_element(&mut rng, i == 0 || i == 3) }));
                }
                let base = (f32::from(i16::try_from(rng.range(-800, 800)).unwrap_or(0)) / 8.0, f32::from(i16::try_from(rng.range(-800, 800)).unwrap_or(0)) / 8.0);
                for _ in 0..rng.range(2, 4) {
                    let off = |rng: &mut crate::rng::Sample64| match rng.range(0, 3) {
                        0 => 0.0f32,
                        1 => f32::from(i16::try_from(rng.range(-1200, 1200)).unwrap_or(0)) * 1e-6,
                        _ => f32::from(i16::try_from(rng.range(-100, 100)).unwrap_or(0)) * 1e-3,
                    };
                    let (dx, dy) = (off(&mut rng), off(&mut rng));
                    v.extend([i64::from((base.0 + dx).to_bits()), i64::from((base.1 + dy).to_bits())]);
                }
                each(&v);
            }
        },
        model: |a| {
            let m = floats::<6>(a, 1)?;
            let mut pts = Vec::new();
            for p in a.get(7..)?.chunks(2) {
                pts.push((f32::from_bits(arg(p, 0)?), f32::from_bits(arg(p, 1)?)));
            }
            one(i64::from(gppath::too_small(rounding(arg(a, 0)?), &m, &pts)))
        },
    },
    Mirror {
        name: "gp.inset-region",
        lean: INSET,
        domain: Domain::Sampled("all 3×3 pixel subsets, empty and overlapping rectangles, and 4,000 aligned or shifted mask pairs up to 7×7"),
        inputs: |each| {
            each(&[0, 0, 0, 3, 0, 0, 0, 3, 3, 511]);
            each(&[0, 0, 3, 2, 63, 1, 0, 3, 2, 63]);
            each(&[0, 0, 2, 2, 9, 0, 0, 2, 2, 15]);
            for bits in 0..512 {
                each(&[0, 0, 3, 3, bits, 0, 0, 3, 3, 511]);
            }
            let mut rng = crate::rng::Sample64::new(0x1a5e_7001);
            for i in 0..4000 {
                let (ax, ay) = (rng.range(-4, 4), rng.range(-4, 4));
                let (aw, ah) = (rng.range(1, 7), rng.range(1, 7));
                let (bx, by, bw, bh) = if i < 1000 {
                    (ax, ay, aw, ah)
                } else {
                    (rng.range(-4, 4), rng.range(-4, 4), rng.range(1, 7), rng.range(1, 7))
                };
                let mask = |w: i64, h: i64| {
                    1_i64.checked_shl(u32::try_from(w * h).unwrap_or(0)).unwrap_or(1).saturating_sub(1)
                };
                let a = rng.range(0, mask(aw, ah));
                let b = rng.range(0, mask(bw, bh));
                each(&[ax, ay, aw, ah, a, bx, by, bw, bh, b]);
            }
        },
        model: |a| {
            use crate::model::gpregion::{inset_contours, Pixels};
            let mask = |x, y, w, h, bits| {
                Pixels::from_bits((arg(a, x)?, arg(a, y)?), arg(a, w)?, arg(a, h)?, arg(a, bits)?)
            };
            let figures = inset_contours(&mask(0, 1, 2, 3, 4)?, &mask(5, 6, 7, 8, 9)?);
            let mut out = Vec::new();
            for points in figures {
                out.push(i64::try_from(points.len()).ok()?);
                out.extend(points.into_iter().flat_map(|(x, y)| [x, y]));
            }
            Some(out)
        },
    },
    Mirror {
        name: "gp.dashes",
        lean: DASH,
        domain: Domain::Sampled("two theorem cases and 6,000 sampled open and closed polylines including repeated endpoints, odd and even patterns, signed offsets, thin and wide pens, and adjusted and unadjusted caps under both rounding modes"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xda54_0001);
            let bits = |v| i64::from((f32::from(i16::try_from(v).unwrap_or(0)) / 4.0).to_bits());
            // Both kernel theorems' inputs run through the Rust walker as well.
            let one = bits(4);
            each(&[0, 0, 0, one, one, 0, 2, 1, 3, 2, bits(12), one, one, one, bits(36), one]);
            each(&[0, 0, 1, one, one, 0, 2, 1, 3, 2, bits(8), bits(8), one, one, bits(20), one]);
            for i in 0..6000 {
                let closed = rng.chance(2);
                let round_adjustment = rng.chance(2);
                let cap = match rng.range(0, 2) { 0 => 0, 1 => 2, _ => 3 };
                let start = rng.range(0, 3);
                let end = rng.range(0, 3);
                let count = rng.range(1, 6);
                let mut input = vec![
                    i % 2, i64::from(closed), i64::from(round_adjustment), bits(rng.range(1, 16)),
                    bits(rng.range(1, 16)), bits(rng.range(-48, 48)), cap, start, end, count,
                ];
                input.extend((0..count).map(|_| bits(rng.range(1, 12))));
                let first = [bits(rng.range(1, 40)), bits(rng.range(1, 40))];
                input.extend(first);
                for _ in 1..rng.range(1, 5) {
                    input.extend([bits(rng.range(1, 40)), bits(rng.range(1, 40))]);
                }
                if closed && rng.chance(2) { input.extend(first); }
                each(&input);
            }
        },
        model: |a| {
            use crate::model::gppath::Pen;
            use crate::model::gpwiden::{dashes, Anchor, Cap};
            let [d, closed, adjust, width, major, offset, dash_cap, start, end, count, rest @ ..] = a else { return None; };
            let f32_arg = |v| u32::try_from(v).ok().map(f32::from_bits);
            let (pattern, points) = rest.split_at_checked(usize::try_from(*count).ok()?)?;
            if pattern.is_empty() { return None; }
            let pattern: Vec<_> = pattern.iter().copied().map(f32_arg).collect::<Option<_>>()?;
            let (points, odd) = points.as_chunks::<2>();
            if !odd.is_empty() { return None; }
            let points: Vec<_> = points.iter().map(|&[x, y]| Some((f32_arg(x)?, f32_arg(y)?))).collect::<Option<_>>()?;
            let mut pen = Pen::new(0, f32_arg(*width)?);
            pen.dash_cap = *dash_cap;
            pen.dash_offset = f32_arg(*offset)?;
            let spans = dashes(rounding(*d), &[(points, *closed != 0)], &pen, f32_arg(*major)?, *adjust != 0, &pattern, (Cap::line(*start), Cap::line(*end)));
            let cap_code = |cap| match cap {
                Cap::Flat => 0,
                Cap::Square => 1,
                Cap::Round => 2,
                Cap::Triangle => 3,
                Cap::Anchor(Anchor::Square) => 0x11,
                Cap::Anchor(Anchor::Round) => 0x12,
                Cap::Anchor(Anchor::Diamond) => 0x13,
                Cap::Anchor(Anchor::Arrow) => 0x14,
            };
            let mut out = Vec::new();
            for span in spans {
                out.extend([cap_code(span.caps.0), cap_code(span.caps.1), i64::try_from(span.pts.len()).ok()?]);
                out.extend(span.pts.into_iter().flat_map(|(x, y)| [canon(x), canon(y)]));
            }
            Some(out)
        },
    },
    Mirror {
        name: "gp.chain",
        lean: CHAIN,
        domain: Domain::Sampled("to nearest and down, every span kind, with and without the half pixel offset: 200,000 draws of source rectangles in and past images up to 8 x 8, signed sizes and 0, onto rectangles in and past bitmaps up to 12 x 12, flips and 1:1 copies among them"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xc4a1_0001);
            for k in 0..200_000u32 {
                let (iw, ih, bw, bh) = (rng.range(1, 8), rng.range(1, 8), rng.range(1, 12), rng.range(1, 12));
                let src = [rng.range(-2, 6), rng.range(-2, 6), rng.range(-9, 10), rng.range(-9, 10)];
                let dst = match rng.range(0, 3) {
                    // (1:1, flipped, or anything)
                    0 => [rng.range(-4, 12), rng.range(-4, 12), src[2], src[3]],
                    1 => [rng.range(-4, 12), rng.range(-4, 12), src[2].saturating_neg(), src[3].saturating_mul(if rng.chance(2) { -1 } else { 1 })],
                    _ => [rng.range(-6, 12), rng.range(-6, 12), rng.range(-14, 26), rng.range(-14, 26)],
                };
                let mut v = vec![i64::from(k % 2), rng.range(0, 4)];
                v.extend(dst);
                v.extend(src);
                v.extend([iw, ih, bw, bh, rng.range(0, 1)]);
                each(&v);
            }
        },
        model: |a| {
            use crate::model::gpimage::{chain, Front, Span as ImgSpan};
            use crate::model::gpstretch::Kernel;
            let span = match arg::<u8>(a, 1)? {
                0 => ImgSpan::Nearest,
                1 => ImgSpan::Bilinear,
                2 => ImgSpan::Bicubic,
                3 => ImgSpan::Stretch(Kernel::Tent),
                _ => ImgSpan::Stretch(Kernel::Cubic),
            };
            let q = |i: usize| arg::<i64>(a, i);
            let front = chain(rounding(arg(a, 0)?), span, [q(2)?, q(3)?, q(4)?, q(5)?], [q(6)?, q(7)?, q(8)?, q(9)?], (q(10)?, q(11)?), (q(12)?, q(13)?), q(14)? != 0);
            Some(match front {
                Err(_) => vec![-1],
                Ok(Front::Empty) => vec![0],
                Ok(Front::Draw(c)) => {
                    let mut out = vec![1, c.top, c.rows];
                    let (a0, a1, a2, a3) = c.sf;
                    let (d0, d1, d2, d3) = c.d;
                    out.extend([a0, a1, a2, a3].into_iter().chain(c.ms2).chain([d0, d1, d2, d3]).map(canon));
                    out.extend([c.cols.start, c.cols.end, c.lines.start, c.lines.end]);
                    out.push(match c.flip {
                        None => 0,
                        Some(crate::model::gpimage::Flip::X) => 1,
                        Some(crate::model::gpimage::Flip::Y) => 2,
                        Some(crate::model::gpimage::Flip::Both) => 3,
                    });
                    match c.copy {
                        None => out.push(0),
                        Some((x, y)) => out.extend([1, x, y]),
                    }
                    out
                }
            })
        },
    },
    Mirror {
        name: "gp.cubic",
        lean: CHAIN,
        domain: Domain::Every("the cubic's running integral at every entry -300 ..= 300"),
        inputs: |each| every(&[(-300, 300)], each),
        model: |a| one(crate::model::gpstretch::cubic(arg(a, 0)?)),
    },
    Mirror {
        name: "gp.keys",
        lean: CHAIN,
        domain: Domain::Every("Keys' cubic tables in 1/65536 and 1/16384 at every entry 0 ..= 140"),
        inputs: |each| {
            for one in [65536, 16384] {
                every(&[(one, one), (0, 140)], each);
            }
        },
        model: |a| one(crate::model::gpimage::keys(f64::from(arg::<u32>(a, 0)?), arg(a, 1)?)),
    },
    Mirror {
        name: "gp.render",
        lean: CHAIN,
        domain: Domain::Sampled("to nearest and down, x86 and x86-64, nearest, bilinear, HighQualityBilinear and HighQualityBicubic, with and without the half pixel offset: 100,000 draws of random images up to 6 x 6 in and past bitmaps up to 10 x 10, and 4,000 of images 24 wide stretched across 30 to 60 (the MMX columns), scales, flips, copies"),
        inputs: |each| {
            let mut rng = crate::rng::Sample64::new(0xc4a1_0002);
            for k in 0..100_000u32 {
                let (iw, ih, bw, bh) = (rng.range(1, 6), rng.range(1, 6), rng.range(1, 10), rng.range(1, 10));
                let src = [rng.range(-1, 4), rng.range(-1, 4), rng.range(-7, 7), rng.range(-7, 7)];
                let dst = match rng.range(0, 2) {
                    0 => [rng.range(-3, 8), rng.range(-3, 8), src[2], src[3].saturating_mul(if rng.chance(2) { -1 } else { 1 })],
                    _ => [rng.range(-4, 9), rng.range(-4, 9), rng.range(-12, 14), rng.range(-12, 14)],
                };
                let mut v = vec![i64::from(k % 2), rng.range(0, 1), *rng.pick(&[0i64, 1, 3, 4])];
                v.extend(dst);
                v.extend(src);
                v.extend([iw, ih, bw, bh, rng.range(0, 1)]);
                for _ in 0..iw.saturating_mul(ih) {
                    // (premultiplied: each color at most its alpha)
                    let a = rng.range(0, 255);
                    let c = |rng: &mut crate::rng::Sample64| rng.range(0, a);
                    v.push(a << 24 | c(&mut rng) << 16 | c(&mut rng) << 8 | c(&mut rng));
                }
                each(&v);
            }
            // (wide stretches of the cubic on x86: the MMX columns and StretchMiddleScanline2)
            for k in 0..4_000u32 {
                let (iw, ih) = (24, rng.range(1, 3));
                let dw = rng.range(30, 60).saturating_mul(if k % 4 == 3 { -1 } else { 1 });
                let mut v = vec![i64::from(k % 2), 1, 4, rng.range(-2, 2), 0, dw, rng.range(1, 3), 0, 0, iw, ih, iw, ih, 64, 4, rng.range(0, 1)];
                for _ in 0..iw.saturating_mul(ih) {
                    let a = rng.range(0, 255);
                    let c = |rng: &mut crate::rng::Sample64| rng.range(0, a);
                    v.push(a << 24 | c(&mut rng) << 16 | c(&mut rng) << 8 | c(&mut rng));
                }
                each(&v);
            }
        },
        model: |a| {
            use crate::model::gpimage::{render, Draw, Span as ImgSpan};
            use crate::model::gpstretch::Kernel;
            let span = match arg::<u8>(a, 2)? {
                0 => ImgSpan::Nearest,
                1 => ImgSpan::Bilinear,
                3 => ImgSpan::Stretch(Kernel::Tent),
                4 => ImgSpan::Stretch(Kernel::Cubic),
                _ => return None,
            };
            let q = |i: usize| arg::<i64>(a, i);
            let (iw, ih) = (q(11)?, q(12)?);
            let img: Vec<u32> = a.get(16..)?.iter().map(|&v| u32::try_from(v).ok()).collect::<Option<Vec<u32>>>()?;
            let d = Draw { dst: [q(3)?, q(4)?, q(5)?, q(6)?], src: [q(7)?, q(8)?, q(9)?, q(10)?], image: (iw, ih), target: (q(13)?, q(14)?), half: q(15)? != 0 };
            let texel = |x: i64, y: i64| -> crate::model::Res<u32> {
                let inside = (0..iw).contains(&x) && (0..ih).contains(&y);
                let i = y.checked_mul(iw).and_then(|v| v.checked_add(x)).and_then(|v| usize::try_from(v).ok());
                Ok(if inside { i.and_then(|i| img.get(i)).copied().unwrap_or(0) } else { 0 })
            };
            Some(match render(rounding(arg(a, 0)?), span, &d, arg::<u8>(a, 1)? != 0, texel) {
                Err(_) => vec![-1],
                Ok(px) => px.iter().flat_map(|&(x, y, c)| [x, y, i64::from(c)]).collect(),
            })
        },
    },
];
