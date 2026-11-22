//! DirectWrite's mirrors (spec/lean/Kikizake/DWrite/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::dwrite::{self, Step};

const ADVANCE: &str = "spec/lean/Kikizake/DWrite/Advance.lean";

pub static MIRRORS: &[Mirror] = &[
    Mirror {
        name: "dw.phantom",
        lean: ADVANCE,
        domain: Domain::Sampled("every advance 0..1024 at every ppem 1..300 for 2048 design units, both steps; 300,000 advances -300..65535 at ppem 1..1000 and 16..16384 design units"),
        inputs: |each| {
            every(&[(0, 1024), (1, 300), (0, 1)], &mut |v| {
                if let [adv, ppem, s] = *v {
                    each(&[adv, ppem, 2048, s]);
                }
            });
            sampled(&[(-300, 65535), (1, 1000), (16, 16384), (0, 1)], 300_000, 0xd417_0001, each);
        },
        model: |a| one(dwrite::phantom_px(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, if arg::<u8>(a, 3)? != 0 { Step::Sixteenth } else { Step::Pixel })),
    },
    Mirror {
        name: "dw.classic",
        lean: ADVANCE,
        domain: Domain::Sampled("every advance 0..1024 at every ppem 1..400 for 1000 design units; 300,000 advances 0..65535 at ppem 1..1000 and 16..16384 design units"),
        inputs: |each| {
            every(&[(0, 1024), (1, 400)], &mut |v| {
                if let [adv, ppem] = *v {
                    each(&[adv, ppem, 1000]);
                }
            });
            sampled(&[(0, 65535), (1, 1000), (16, 16384)], 300_000, 0xd417_0002, each);
        },
        model: |a| one(dwrite::classic_px(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?)),
    },
];
