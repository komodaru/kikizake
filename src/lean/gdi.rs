//! GDI's mirrors (spec/lean/Kikizake/Gdi/).
use super::{arg, every, sampled, Domain, Mirror};
use crate::model::gdiblend;

const BLEND: &str = "spec/lean/Kikizake/Gdi/Blend.lean";

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "gdi.mix",
        lean: BLEND,
        domain: Domain::Every("source, destination and alpha 0..255"),
        inputs: |each| every(&[(0, 255), (0, 255), (0, 255)], each),
        model: |a| Some(vec![i64::from(gdiblend::mix(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?))]),
    },
    Mirror {
        name: "gdi.alpha",
        lean: BLEND,
        domain: Domain::Sampled("32 bpp source and destination pixels, alpha 0..255, both modes"),
        inputs: |each| sampled(&[(0, 0xffff_ffff), (0, 0xffff_ffff), (0, 255), (0, 1)], 200_000, 0x0a1f_a001, each),
        model: |a| Some(vec![i64::from(gdiblend::alpha(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg::<i64>(a, 3)? != 0))]),
    },
];
