//! Case generators: the case files are made here, from fixed seeds (cases/<suite>.txt is what
//! `kikizake gen <suite>` prints; a test keeps them equal). Coverage comes from structure (every
//! raster operation on every pixel format, every state, every state block type) and from many
//! random cases around the edges (clipping, bitmap borders, overlap, odd sizes, both row orders).
//!
//! One file per API (src/gen/<api>.rs); SUITES lists every suite and the function that makes it.

mod d3d11;
mod d3d9;
mod ddraw;
mod gdi;
mod wic;

pub use d3d11::{d3d11_base, d3d11_floats};
pub(crate) use crate::rng::XorShift32;
pub(crate) use std::fmt::Write;
use crate::model::gdi::default_palette;

/// every suite: its name (cases/<name>.txt) and its generator, one line each
pub const GENERATORS: &[(&str, fn() -> String)] = &[
    ("gdi", gdi::gdi),
    ("stretch", gdi::stretch),
    ("pixel", gdi::pixel),
    ("d3d8", d3d9::d3d8),
    ("d3d9", d3d9::d3d9),
    ("d3d11", d3d11::d3d11),
    ("wic", wic::wic),
    ("ddraw", ddraw::ddraw),
];

pub const SUITES: &[&str] = &{
    let mut s = [""; GENERATORS.len()];
    let mut i = 0;
    while i < s.len() {
        s[i] = GENERATORS[i].0;
        i += 1;
    }
    s
};

pub fn suite(name: &str) -> Option<String> {
    GENERATORS.iter().find(|g| g.0 == name).map(|g| (g.1)())
}

/// the raster operation codes windows.h names
pub const NAMED_ROPS: &[(u32, &str)] = &[
    (0x00CC0020, "SRCCOPY"),
    (0x00EE0086, "SRCPAINT"),
    (0x008800C6, "SRCAND"),
    (0x00660046, "SRCINVERT"),
    (0x00440328, "SRCERASE"),
    (0x00330008, "NOTSRCCOPY"),
    (0x001100A6, "NOTSRCERASE"),
    (0x00C000CA, "MERGECOPY"),
    (0x00BB0226, "MERGEPAINT"),
    (0x00F00021, "PATCOPY"),
    (0x00FB0A09, "PATPAINT"),
    (0x005A0049, "PATINVERT"),
    (0x00550009, "DSTINVERT"),
    (0x00000042, "BLACKNESS"),
    (0x00FF0062, "WHITENESS"),
];

/// formats as `dib` arguments: (bpp, extra)
pub(crate) const FORMATS: &[(u32, &str)] = &[(1, ""), (4, ""), (8, ""), (16, ""), (16, " 565"), (24, ""), (32, "")];

pub(crate) fn fmt_name(f: &(u32, &str)) -> String {
    format!("{}{}", f.0, f.1.replace(' ', ""))
}

pub(crate) fn rcolor(r: &mut XorShift32, pal: &[u32]) -> u32 {
    // (exact palette colors and the extremes are where conversions round differently)
    match r.range(0, 5) {
        0 => *r.pick(&[0, 0xffffff, 0xff0000, 0x00ff00, 0x0000ff, 0x808080, 0x7f7f7f]),
        1 if !pal.is_empty() => *r.pick(pal),
        _ => r.next() & 0xffffff,
    }
}

pub(crate) fn dib_line(r: &mut XorShift32, slot: u32, w: i64, h: i64, f: &(u32, &str), custom_pal: bool) -> (String, Vec<u32>) {
    let order = if r.chance(3) { " bu" } else { "" };
    let mut s = format!("dib {slot} {w} {h} {}{order}{}", f.0, f.1);
    let mut pal = default_palette(f.0);
    if custom_pal && !pal.is_empty() {
        let n = pal.len().min(16);
        for e in pal.iter_mut().take(n) {
            *e = r.next() & 0xffffff;
            write!(s, " #{e:06x}").unwrap();
        }
    }
    (s, pal)
}

pub(crate) fn rop(r: &mut XorShift32) -> u32 {
    if r.chance(2) {
        r.pick(NAMED_ROPS).0
    } else {
        // any of the 256 operations, with an arbitrary low word (Windows reads bits 16..23)
        (r.next() & 0xff) << 16 | (r.next() & 0xffff)
    }
}

