//! GDI+'s mirrors (spec/lean/Kikizake/GdiPlus/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::gdiplus::{self, Depth, Pf};

const CONV: &str = "spec/lean/Kikizake/GdiPlus/Conv.lean";
const BLEND: &str = "spec/lean/Kikizake/GdiPlus/Blend.lean";

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

/// every value of the 16-bit formats; a sample of the 24, 32, 48 and 64-bit ones
fn values(each: &mut dyn FnMut(&[i64])) {
    for f in 4..=6 {
        every(&[(0, 65535)], &mut |v| {
            if let [v] = *v {
                each(&[f, v]);
            }
        });
    }
    for (f, max) in [(7, 0xff_ffff), (8, 0xffff_ffff), (9, 0xffff_ffff), (10, 0xffff_ffff), (11, 0xffff_ffff_ffff), (12, i64::MAX), (13, i64::MAX)] {
        sampled(&[(0, max)], 100_000, 0x6d00_0000 ^ u32::try_from(f).unwrap_or(0), &mut |v| {
            if let [v] = *v {
                each(&[f, v]);
            }
        });
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
    let max = |f: i64| match f {
        4..=6 => 0xffff,
        7 => 0xff_ffff,
        8..=10 => 0xffff_ffff,
        11 => 0xffff_ffff_ffff,
        _ => i64::MAX,
    };
    for a in 4..=13 {
        for b in 4..=13 {
            sampled(&[(0, max(a))], 4000, 0x7100_0000 ^ u32::try_from(a).unwrap_or(0) << 4 ^ u32::try_from(b).unwrap_or(0), &mut |v| {
                if let [v] = *v {
                    each(&[a, b, v]);
                }
            });
        }
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
        model: |a| one(gdiplus::to_argb(pf(arg(a, 0)?)?, arg(a, 1)?, &[]).ok()?),
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
        model: |a| one(i64::try_from(gdiplus::convert(pf(arg(a, 0)?)?, pf(arg(a, 1)?)?, arg(a, 2)?, &[]).ok()?).ok()?),
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
];
