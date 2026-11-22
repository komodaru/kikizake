//! GDI's mirrors (spec/lean/Kikizake/Gdi/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::{bezier, gdi, gdiblend, gdixform};

const BLEND: &str = "spec/lean/Kikizake/Gdi/Blend.lean";
const STRETCH: &str = "spec/lean/Kikizake/Gdi/Stretch.lean";
const XFORM: &str = "spec/lean/Kikizake/Gdi/Xform.lean";
const GRADIENT: &str = "spec/lean/Kikizake/Gdi/Gradient.lean";
const HFD: &str = "spec/lean/Kikizake/Gdi/Hfd.lean";

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "gdi.mix",
        lean: BLEND,
        domain: Domain::Every("source, destination and alpha 0..255"),
        inputs: |each| every(&[(0, 255), (0, 255), (0, 255)], each),
        model: |a| one(gdiblend::mix(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
    Mirror {
        name: "gdi.alpha",
        lean: BLEND,
        domain: Domain::Sampled("32 bpp source and destination pixels, alpha 0..255, both modes"),
        inputs: |each| sampled(&[(0, 0xffff_ffff), (0, 0xffff_ffff), (0, 255), (0, 1)], 200_000, 0x0a1f_a001, each),
        model: |a| one(gdiblend::alpha(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg::<i64>(a, 3)? != 0)),
    },
    Mirror {
        name: "gdi.sample",
        lean: STRETCH,
        domain: Domain::Every("source and destination extents 1..64, every destination pixel, both modes"),
        inputs: |each| {
            every(&[(1, 64), (1, 64), (0, 1)], &mut |a| {
                if let [sn, n, flat] = *a {
                    for i in 0..n {
                        each(&[i, sn, n, flat]);
                    }
                }
            })
        },
        model: |a| one(gdi::sample(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg::<i64>(a, 3)? != 0)),
    },
    Mirror {
        name: "gdi.page",
        lean: XFORM,
        domain: Domain::Sampled("point, extents and origins in -2^20..2^20, the window extent not 0"),
        inputs: |each| {
            let r = (-(1 << 20), 1 << 20);
            sampled(&[r, r, r, r, r], 300_000, 0x9a6e_0001, &mut |a| {
                if let [p, w, v, wo, vo] = *a {
                    each(&[p, if w == 0 { 1 } else { w }, v, wo, vo]);
                }
            })
        },
        model: |a| one(gdixform::page(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?)),
    },
    Mirror {
        name: "gdi.c8",
        lean: GRADIENT,
        domain: Domain::Sampled("numerators -2^40..2^40, denominators 1..2^24, both roundings"),
        inputs: |each| sampled(&[(-(1 << 40), 1 << 40), (1, 1 << 24), (0, 1)], 300_000, 0xc8c8_0001, each),
        model: |a| {
            let r = if arg::<i64>(a, 2)? != 0 { gdiblend::Round::Nearest16 } else { gdiblend::Round::Floor };
            one(gdiblend::c8(arg(a, 0)?, arg(a, 1)?, r))
        },
    },
    Mirror {
        name: "gdi.dither16",
        lean: GRADIENT,
        domain: Domain::Every("16 bit values 0..65535, matrix values 0..15, 5 and 6 bit channels"),
        inputs: |each| {
            for (unit, max) in [(128, 31), (64, 63)] {
                every(&[(0, 65535), (0, 15)], &mut |a| {
                    if let [v, d] = *a {
                        each(&[v, d, unit, max]);
                    }
                });
            }
        },
        model: |a| one(gdiblend::dither16(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?)),
    },
    Mirror {
        name: "gdi.cell5",
        lean: GRADIENT,
        domain: Domain::Every("1 bpp, halftone and plain tables, channels 0..255, matrix values 0..127"),
        inputs: |each| every(&[(0, 1), (0, 1), (0, 255), (0, 127)], each),
        model: |a| one(gdiblend::cell5(arg::<i64>(a, 0)? != 0, arg::<i64>(a, 1)? != 0, arg(a, 2)?, arg(a, 3)?)),
    },
    Mirror {
        name: "gdi.matrix",
        lean: GRADIENT,
        domain: Domain::Every("every entry of the three matrices (4x4, 16x16, 16x16)"),
        inputs: |each| {
            every(&[(0, 0), (0, 3), (0, 3)], each);
            every(&[(1, 2), (0, 15), (0, 15)], each);
        },
        model: |a| {
            let (x, y): (usize, usize) = (arg(a, 1)?, arg(a, 2)?);
            match arg::<i64>(a, 0)? {
                0 => one(*gdiblend::BAYER4.get(y)?.get(x)?),
                1 => one(*gdiblend::DITHER6.get(y)?.get(x)?),
                2 => one(*gdiblend::DITHER3.get(y)?.get(x)?),
                _ => None,
            }
        },
    },
    Mirror {
        name: "gdi.flatten",
        lean: HFD,
        domain: Domain::Sampled("four 28.4 control points in -4000..4000 (within the 32 bit flattener's reach)"),
        inputs: |each| sampled(&[(-4000, 4000); 8], 100_000, 0x4fd3_0001, each),
        model: |a| {
            let p = |i: usize| -> Option<(i32, i32)> { Some((arg(a, 2 * i)?, arg(a, 2 * i + 1)?)) };
            let f = bezier::flatten([p(0)?, p(1)?, p(2)?, p(3)?])?;
            Some(f.iter().flat_map(|q| [i64::from(q.0), i64::from(q.1)]).collect())
        },
    },
];
