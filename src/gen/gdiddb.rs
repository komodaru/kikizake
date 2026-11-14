//! The ddb suite: device-dependent bitmaps, CreateCompatibleBitmap, GetObject, GetBitmapBits,
//! GetDeviceCaps, monochrome <-> color blits through DDBs.
use super::*;

/// GetDeviceCaps indices: TECHNOLOGY, BITSPIXEL, PLANES, NUMBRUSHES, NUMPENS, NUMCOLORS,
/// RASTERCAPS, SIZEPALETTE, NUMRESERVED, COLORRES, SHADEBLENDCAPS
const CAPS: &str = "2 12 14 16 18 24 38 104 106 108 120";

pub fn ddb() -> String {
    let mut s = String::from("# kikizake suite ddb: device-dependent bitmaps (SPEC.md). Made by `kikizake gen ddb`.\n");
    let mut r = XorShift32::new(0xddb0_0000);
    // the kinds of bitmap: what GetObject and GetDeviceCaps say
    let kinds = ["mono 9 5", "bitmap 9 5 1 1", "bitmap 9 5 1 4", "bitmap 9 5 1 8", "bitmap 9 5 1 16", "bitmap 9 5 1 24", "bitmap 9 5 1 32", "bitmap 9 5 4 1", "bitmap 9 5 1 2", "screen 9 5", "screen 0 0", "mono 0 3", "bitmap 9 5 3 8"];
    for (i, k) in kinds.iter().enumerate() {
        writeln!(s, "case kind-{i}\nddb 0 {k}\nbmobject 0\ndevcaps 0 {CAPS}\nobjtype 0 bitmap\nend").unwrap();
    }
    // compatible bitmaps of DIB sections of every format and of DDBs
    for f in FORMATS {
        writeln!(s, "case compat-{}\ndib 0 4 4 {}{}\nbmobject 0\ndevcaps 0 {CAPS}\nddb 1 compat 0 7 3\nbmobject 1\ndevcaps 1 {CAPS}\nend", fmt_name(f), f.0, f.1).unwrap();
    }
    writeln!(s, "case compat-ddb\nddb 0 mono 4 4\nddb 1 compat 0 5 3\nbmobject 1\nddb 2 screen 4 4\nddb 3 compat 2 5 3\nbmobject 3\nend").unwrap();
    // GetBitmapBits / SetBitmapBits (fill and dump) round trips, GetDIBits of DDBs
    for k in ["mono 13 3", "screen 5 3", "bitmap 7 2 1 8", "bitmap 5 2 1 16", "bitmap 5 2 1 24"] {
        writeln!(s, "case bits-{}\nddb 0 {k}\nfill 0 {}\ndump 0\nbmbits 0\ngetdibits 0 32\ngetdibits 0 24\ngetdibits 0 1\nend", k.replace(' ', "-"), r.next()).unwrap();
    }
    // monochrome DDB to color and back: the text and background colors (BitBlt's rules)
    for f in FORMATS.iter().filter(|f| f.0 > 1) {
        for dir in ["to", "from"] {
            writeln!(s, "case mono-{dir}-{}", fmt_name(f)).unwrap();
            let (l, pal) = dib_line(&mut r, 1, 9, 4, f, false);
            writeln!(s, "{l}\nfill 1 {}\nddb 0 mono 9 4\nfill 0 {}", r.next(), r.next()).unwrap();
            let (d, sl) = if dir == "to" { (1, 0) } else { (0, 1) };
            writeln!(s, "text {d} #{:06x}\nbk {d} #{:06x}", rcolor(&mut r, &pal), rcolor(&mut r, &pal)).unwrap();
            writeln!(s, "text {sl} #{:06x}\nbk {sl} #{:06x}", rcolor(&mut r, &pal), rcolor(&mut r, &pal)).unwrap();
            if dir == "from" && f.0 >= 8 {
                // (a source pixel equal to the background color)
                writeln!(s, "getpixel 1 0 0").unwrap();
            }
            writeln!(s, "bitblt {d} 0 0 9 4 {sl} 0 0 0xcc0020\ndump {d}\nend").unwrap();
        }
    }
    // the screen format's DDB against DIB sections: blits both ways
    for f in FORMATS.iter().filter(|f| f.0 > 1) {
        writeln!(s, "case screen-{}", fmt_name(f)).unwrap();
        let (l, _) = dib_line(&mut r, 1, 7, 3, f, false);
        writeln!(s, "{l}\nfill 1 {}\nddb 0 screen 7 3\nfill 0 {}\nbitblt 0 0 0 4 3 1 0 0 0xcc0020\ndump 0\nbitblt 1 3 0 4 3 0 3 0 0xcc0020\ndump 1\nend", r.next(), r.next()).unwrap();
    }
    s
}
