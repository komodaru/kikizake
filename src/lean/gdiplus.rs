//! GDI+'s mirrors (spec/lean/Kikizake/GdiPlus/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::gdiplus::{self, Pf};

const CONV: &str = "spec/lean/Kikizake/GdiPlus/Conv.lean";

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
];
