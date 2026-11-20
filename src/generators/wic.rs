//! The wic suites' case generators.
use super::*;

pub fn wic() -> String {
    use crate::model::wic::{bits, FORMATS};
    let mut s = String::from("# kikizake suite wic: WIC pixel format conversion (SPEC.md). Made by `kikizake gen wic`.\n");
    let names: Vec<&str> = FORMATS.iter().map(|f| f.0).collect();
    // every pair, an 8x4 picture of random bytes
    let mut seed = 1u32;
    for from in &names {
        for to in &names {
            writeln!(s, "case pair-{from}-{to}\nconvert {from} 8 4 {seed} {to}\nend");
            seed += 1;
        }
    }
    // every byte value of each format to 32bppBGRA and back (byte i = i mod 256)
    for (f, from) in names.iter().enumerate() {
        let w = (256 * 8 / bits(f)).clamp(1, 256);
        writeln!(s, "case ramp-{from}\nconvert {from} {w} 1 ramp bgra\nconvert bgra 64 1 ramp {from}\nend");
    }
    // straight and premultiplied alpha over many (color, alpha) pairs
    for (i, f) in ["pbgra", "prgba", "bgra"].iter().enumerate() {
        for k in 0..4 {
            writeln!(s, "case alpha-{f}-{k}\nconvert {f} 256 64 {} bgra\nconvert bgra 256 64 {} {f}\nend", 7000 + i * 10 + k, 7100 + i * 10 + k);
        }
    }
    // odd sizes: rows that end inside a byte, one-pixel pictures
    let mut r = XorShift32::new(0x0057_1c00);
    for i in 0..300 {
        let (from, to) = (*r.pick(&names), *r.pick(&names));
        let (w, h) = (r.range(1, 13), r.range(1, 4));
        writeln!(s, "case odd-{i}\nconvert {from} {w} {h} {} {to}\nend", r.next());
    }
    s
}
