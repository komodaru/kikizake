//! Case generators: the case files are made here, from fixed seeds (cases/<suite>.txt is what
//! `kikizake gen <suite>` prints; a test keeps them equal). Coverage comes from structure (every
//! raster operation on every pixel format, every state, every state block type) and from many
//! random cases around the edges (clipping, bitmap borders, overlap, odd sizes, both row orders).
//!
//! One file per API (src/gen/<api>.rs); SUITES lists every suite and the function that makes it.

mod d2d;
mod d3d11;
pub mod d3d12;
pub mod d3ddraw;
mod d3dres;
pub mod d3d7;
pub use crate::dxbc;
mod d3d9;
mod d3d9draw;
mod d3dx9;
mod ddraw;
mod gdi;
mod gdiplus;
mod opengl;
mod wic;
pub mod dwrite;
mod gdiline;
mod gdirgn;
mod gdixform;
mod gdipath;
mod gditext;
mod gdiblend;
mod gdiddb;
mod ntstr;
mod ntcase;
mod wicimage;
pub use wicimage::{bmp as wicimage_bmp, gif as wicimage_gif, png as wicimage_png, png_filter as wicimage_png_filter, png_channels as wicimage_png_channels, ico as wicimage_ico, tiff as wicimage_tiff, TiffImage as WicTiffImage, GifFrame as WicGifFrame, ADAM7 as WICIMAGE_ADAM7};

pub use d3d11::{d3d11_base, d3d11_floats};
pub(crate) use crate::rng::XorShift32;
pub(crate) use text::Write;

/// The case text is written to a `String`, which cannot fail: `write!(s, ..)`/`writeln!(s, ..)`
/// here are `()`, not a `fmt::Result` to unwrap (std's `fmt::Write` is not in scope in the
/// generators; this trait takes its place under the same name).
mod text {
    use std::fmt;
    pub(crate) trait Write {
        fn write_fmt(&mut self, args: fmt::Arguments<'_>);
    }
    impl Write for String {
        #[inline]
        fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
            match args.as_str() {
                Some(text) => self.push_str(text),
                None => self.push_str(&fmt::format(args)),
            }
        }
    }
}
use crate::model::gdi::default_palette;

/// every suite: its name (cases/<name>.txt) and its generator, one line each
pub const GENERATORS: &[(&str, fn() -> String)] = &[
    ("gdi", gdi::gdi),
    ("stretch", gdi::stretch),
    ("pixel", gdi::pixel),
    ("d3d8", d3d9::d3d8),
    ("d3d9", d3d9::d3d9),
    ("d3d9draw", d3d9draw::d3d9draw),
    ("d3d8draw", d3d9draw::d3d8draw),
    ("d3dx9", d3dx9::d3dx9),
    ("d3dx9load", d3dx9::d3dx9load),
    ("d3d11", d3d11::d3d11),
    ("wic", wic::wic),
    ("ddraw", ddraw::ddraw),
    ("d2d", d2d::d2d),
    ("d2dlimits", d2d::d2dlimits),
    ("d2dhugecurve", d2d::d2dhugecurve),
    ("d2doutlinehollow", d2d::d2doutlinehollow),
    ("d2dgradientmap", d2d::d2dgradientmap),
    ("d2droundturn", d2d::d2droundturn),
    ("gdiplus", gdiplus::gdiplus),
    ("gdiplusmat", gdiplus::gdiplusmat),
    ("gdipluspaint", gdiplus::gdipluspaint),
    ("gdiplusdraw", gdiplus::gdiplusdraw),
    ("gdiplusbrush", gdiplus::gdiplusbrush),
    ("gdiplusline", gdiplus::gdiplusline),
    ("gdipluspath", gdiplus::gdipluspath),
    ("dwrite", dwrite::dwrite),
    ("d3d12", d3d12::d3d12),
    ("dxgi", d3d12::dxgi),
    ("d3d12rm", d3d12::d3d12rm),
    ("d3d10res", d3dres::d3d10res),
    ("d3d11res", d3dres::d3d11res),
    ("lines", gdiline::lines),
    ("region", gdirgn::region),
    ("xform", gdixform::xform),
    ("path", gdipath::path),
    ("text", gditext::text),
    ("blend", gdiblend::blend),
    ("ddb", gdiddb::ddb),
    ("wicpalette", wicimage::wicpalette),
    ("wicscale", wicimage::wicscale),
    ("wictransform", wicimage::wictransform),
    ("wiccolor", wicimage::wiccolor),
    ("wicdecode", wicimage::wicdecode),
    ("wicencode", wicimage::wicencode),
    ("opengl", opengl::opengl),
    ("glstipple", opengl::glstipple),
    ("ntstr", ntstr::ntstr),
    ("ntcase", ntcase::ntcase),
    ("ddclip", ddraw::ddclip),
    ("ddpal", ddraw::ddpal),
    ("dddc", ddraw::dddc),
    ("ddlock", ddraw::ddlock),
    ("ddflip", ddraw::ddflip),
    ("dd8", ddraw::dd8),
    ("d3d7", d3d7::d3d7),
    ("d3d7rast", d3d7::d3d7rast),
    ("d3d7shade", d3d7::d3d7shade),
    ("d3d7edge", d3d7::d3d7edge),
    ("d3d7dev", d3d7::d3d7dev),
    ("d3d7wide", d3d7::d3d7wide),
    ("d3d7rowspill", d3d7::d3d7rowspill),
    ("d3d7rampclip", d3d7::d3d7rampclip),
    ("d3d7spill0", d3d7::d3d7spill::<0>),
    ("d3d7spill1", d3d7::d3d7spill::<1>),
    ("d3d7spill2", d3d7::d3d7spill::<2>),
    ("d3d7spill3", d3d7::d3d7spill::<3>),
    ("d3d7spill4", d3d7::d3d7spill::<4>),
    ("d3d7bad0", d3d7::d3d7bad::<0>),
    ("d3d7bad1", d3d7::d3d7bad::<1>),
    ("d3d7bad2", d3d7::d3d7bad::<2>),
    ("d3d7bad3", d3d7::d3d7bad::<3>),
    ("d3d7bad4", d3d7::d3d7bad::<4>),
    ("d3d7bad5", d3d7::d3d7bad::<5>),
    ("d3d7bad6", d3d7::d3d7bad::<6>),
    ("d3d7bad7", d3d7::d3d7bad::<7>),
    ("d3d7bad8", d3d7::d3d7bad::<8>),
    ("d3d7bad9", d3d7::d3d7bad::<9>),
    ("d3d7bad10", d3d7::d3d7bad::<10>),
    ("d3d7bad11", d3d7::d3d7bad::<11>),
    ("d3d7bad12", d3d7::d3d7bad::<12>),
    ("d3d7bad13", d3d7::d3d7bad::<13>),
    ("d3d7bad14", d3d7::d3d7bad::<14>),
    ("d3d7bad15", d3d7::d3d7bad::<15>),
    ("d3d7bad16", d3d7::d3d7bad::<16>),
    ("d3d7bad17", d3d7::d3d7bad::<17>),
    ("d3d7bad18", d3d7::d3d7bad::<18>),
    ("d3d7bad19", d3d7::d3d7bad::<19>),
    ("d3d7bad20", d3d7::d3d7bad::<20>),
    ("d3d7bad21", d3d7::d3d7bad::<21>),
    ("d3d7bad22", d3d7::d3d7bad::<22>),
    ("d3d7bad23", d3d7::d3d7bad::<23>),
    ("d3d7bad24", d3d7::d3d7bad::<24>),
    ("d3d7bad25", d3d7::d3d7bad::<25>),
    ("d3d7bad26", d3d7::d3d7bad::<26>),
    ("d3d7bad27", d3d7::d3d7bad::<27>),
    ("d3d7bad28", d3d7::d3d7bad::<28>),
    ("d3d7bad29", d3d7::d3d7bad::<29>),
    ("d3d7bad30", d3d7::d3d7bad::<30>),
    ("d3d7bad31", d3d7::d3d7bad::<31>),
    ("d3d7bad32", d3d7::d3d7bad::<32>),
    ("d3d7bad33", d3d7::d3d7bad::<33>),
    ("d3d7bad34", d3d7::d3d7bad::<34>),
    ("d3d7bad35", d3d7::d3d7bad::<35>),
    ("d3d7bad36", d3d7::d3d7bad::<36>),
    ("d3d7bad37", d3d7::d3d7bad::<37>),
    ("d3d7bad38", d3d7::d3d7bad::<38>),
    ("d3d7bad39", d3d7::d3d7bad::<39>),
    ("d3d7bad40", d3d7::d3d7bad::<40>),
    ("d3d7bad41", d3d7::d3d7bad::<41>),
    ("d3d7bad42", d3d7::d3d7bad::<42>),
    ("d3d7bad43", d3d7::d3d7bad::<43>),
    ("d3d7bad44", d3d7::d3d7bad::<44>),
    ("d3d7bad45", d3d7::d3d7bad::<45>),
    ("d3d10draw", d3ddraw::d3d10draw),
    ("d3d11draw", d3ddraw::d3d11draw),
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
            write!(s, " #{e:06x}");
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


/// Architecture declarations shared with runner/build.sh.
pub fn architectures(input: &str) -> impl Iterator<Item = u32> + '_ {
    [32, 64].into_iter().filter(|arch| {
        let excluded = if *arch == 64 { "# x86 only" } else { "# x64 only" };
        !input.lines().any(|line| line.starts_with(excluded))
    })
}
