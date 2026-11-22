//! DirectWrite's mirrors (spec/lean/Kikizake/DWrite/).
use super::{arg, every, one, sampled, Domain, Mirror};
use crate::model::dwrite::{self, Step};

const ADVANCE: &str = "spec/lean/Kikizake/DWrite/Advance.lean";
const METRICS: &str = "spec/lean/Kikizake/DWrite/Metrics.lean";

/// `n` argument lists in the ranges, each argument one of its range's ends, 0, 1 or -1 (where
/// the range holds them) one time in four, else uniform in it
fn edged(ranges: &[(i64, i64)], n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let mut rng = crate::rng::Sample64::new(seed);
    for _ in 0..n {
        let v: Vec<i64> = ranges
            .iter()
            .map(|&(lo, hi)| {
                if rng.chance(4) {
                    let e = *rng.pick(&[lo, hi, 0, 1, -1]);
                    if (lo..=hi).contains(&e) { e } else { lo }
                } else {
                    rng.range(lo, hi)
                }
            })
            .collect();
        each(&v);
    }
}

/// a flag argument's option
fn opt<T>(a: &[i64], i: usize, v: impl FnOnce() -> Option<T>) -> Option<Option<T>> {
    match arg::<u8>(a, i)? {
        0 => Some(None),
        1 => v().map(Some),
        _ => None,
    }
}

/// the face dw.metrics takes: units per em, head's box, hhea, OS/2 and post (each after its flag)
fn face(a: &[i64]) -> Option<crate::ttf::Parsed> {
    if a.len() != 32 {
        return None;
    }
    let i = |k: usize| arg::<i16>(a, k);
    let os2 = opt(a, 9, || {
        let script: Vec<i16> = (11..19).map(i).collect::<Option<_>>()?;
        Some(crate::ttf::Os2 {
            version: arg(a, 10)?,
            script: script.try_into().ok()?,
            strike: (i(19)?, i(20)?),
            fs_selection: arg(a, 21)?,
            typo: (i(22)?, i(23)?, i(24)?),
            win: (arg(a, 25)?, arg(a, 26)?),
            x_height: i(27)?,
            cap_height: i(28)?,
            ..Default::default()
        })
    })?;
    Some(crate::ttf::Parsed {
        upm: arg(a, 0)?,
        head_box: (i(1)?, i(2)?, i(3)?, i(4)?),
        hhea: opt(a, 5, || Some((i(6)?, i(7)?, i(8)?)))?,
        os2,
        post: opt(a, 29, || Some((i(30)?, i(31)?, 0)))?,
        ..Default::default()
    })
}

/// a Metrics as dw.metrics answers it
fn metrics_out(m: &dwrite::Metrics) -> Vec<i64> {
    let mut v = vec![i64::from(m.upm), i64::from(m.asc), i64::from(m.desc), i64::from(m.gap), i64::from(m.cap), i64::from(m.x)];
    v.extend([i64::from(m.ulpos), i64::from(m.ulthick), i64::from(m.stpos), i64::from(m.stthick)]);
    v.extend(m.bbox.iter().chain(&m.sub).chain(&m.sup).map(|&x| i64::from(x)));
    v.push(i64::from(m.typo));
    v
}

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
    Mirror {
        name: "dw.metrics",
        lean: METRICS,
        domain: Domain::Sampled("400,000 faces: units per em 1..65535, every INT16 field -32768..32767 and UINT16 one 0..65535, OS/2 versions 0..5, each of hhea, OS/2 and post there or not, a quarter of the values an end, 0, 1 or -1"),
        inputs: |each| {
            let s = (-32768, 32767);
            let mut r = vec![(1, 65535), s, s, s, s, (0, 1), s, s, s, (0, 1), (0, 5)];
            r.extend([s; 10]);
            r.extend([(0, 65535), s, s, s, (0, 65535), (0, 65535), s, s, (0, 1), s, s]);
            edged(&r, 400_000, 0xd417_0003, each);
        },
        model: |a| Some(metrics_out(&dwrite::metrics(&face(a)?))),
    },
    Mirror {
        name: "dw.glyph",
        lean: METRICS,
        domain: Domain::Sampled("400,000 glyphs: advances 0..65535, every INT16 bearing, box and typo value, vmtx, OS/2 and hhea there or not, ascents 0..65535, a quarter of the values an end, 0, 1 or -1"),
        inputs: |each| {
            let s = (-32768, 32767);
            let r = [(0, 65535), s, (0, 1), s, s, s, s, (0, 1), (0, 65535), s, (0, 1), s, s, (0, 1), s, (0, 65535)];
            edged(&r, 400_000, 0xd417_0004, each);
        },
        model: |a| {
            if a.len() != 16 {
                return None;
            }
            let i = |k: usize| arg::<i16>(a, k);
            let bx = opt(a, 2, || Some((i(3)?, i(4)?, i(5)?, i(6)?)))?;
            let vm = opt(a, 7, || Some((arg::<u16>(a, 8)?, i(9)?)))?;
            let os2 = opt(a, 10, || Some(crate::ttf::Os2 { typo: (i(11)?, i(12)?, 0), ..Default::default() }))?;
            let hhea = opt(a, 13, || Some((0, i(14)?, 0)))?;
            let p = crate::ttf::Parsed {
                hmtx: vec![(arg(a, 0)?, i(1)?)],
                glyphs: vec![bx.map(|b| (b, Vec::new()))],
                vmtx: vm.map(|v| vec![v]),
                os2,
                hhea,
                ..Default::default()
            };
            let m = dwrite::Metrics { asc: arg(a, 15)?, ..Default::default() };
            let g = dwrite::glyph_metrics(&p, &m, 0)?;
            Some(vec![i64::from(g.adv), i64::from(g.lsb), i64::from(g.rsb), i64::from(g.tsb), i64::from(g.advh), i64::from(g.bsb), i64::from(g.voy)])
        },
    },
];
