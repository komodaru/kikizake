//! DirectDraw 7 blits between system-memory offscreen surfaces: an executable specification of
//! IDirectDrawSurface7::Blt (copy, color fill, source and destination color keys and their
//! overrides, mirroring) and BltFast, with Lock showing the bytes.
//!
//! Sources (Microsoft Learn): "IDirectDrawSurface7::Blt", "IDirectDrawSurface7::BltFast",
//! "DDBLTFX", "DDCOLORKEY", "Color Keying" (a source key's pixels are not copied; a destination
//! key's pixels are the only ones written). MEASURED (golden ddraw-32/-64): a
//! rectangle past the surface or empty is DDERR_INVALIDRECT, without a clipper; no destination
//! color keys on system-memory surfaces; the order of the checks; fills drop high bits;
//! different formats are DDERR_UNSUPPORTED.

use crate::spec::open;
use crate::spec::clause;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const DDERR_INVALIDRECT: u32 = 0x8876_0096;
pub const DDERR_NOCOLORKEYHW: u32 = 0x8876_00dc;
pub const DDERR_UNSUPPORTED: u32 = 0x8000_4001;
pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const E_NOINTERFACE: u32 = 0x8000_4002;
pub const DDERR_SURFACEBUSY: u32 = 0x8876_01ae;
pub const DDERR_NOTLOCKED: u32 = 0x8876_0248;
pub const DDERR_NOCLIPLIST: u32 = 0x8876_00cd;
pub const DDERR_NOCLIPPERATTACHED: u32 = 0x8876_0238;
pub const DDERR_BLTFASTCANTCLIP: u32 = 0x8876_023e;
pub const DDERR_INVALIDPIXELFORMAT: u32 = 0x8876_0091;
pub const DDERR_NOPALETTEATTACHED: u32 = 0x8876_023c;

/// MEASURED (dd8 table-0, the same on 32 and 64 bits): the colors an 8 bpp surface's DC gives
/// the indices while the surface has no palette: the 20 static VGA colors at 0..10 and
/// 246..256, 236 fixed colors between
pub const SYSTEM_PALETTE: [u32; 256] = [
    0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0,
    0xc0dcc0, 0xa6caf0, 0x241808, 0x302814, 0x3c3018, 0x614d30, 0x4d5151, 0x61714d,
    0x616d82, 0x14140c, 0x827559, 0x089e75, 0x383430, 0x3869aa, 0x283420, 0x616149,
    0x9e8649, 0x6d9a7d, 0xcb6908, 0x82868e, 0x5d6175, 0xeb6100, 0x710800, 0x30382c,
    0xef7104, 0xaa928e, 0xf76d30, 0xc3c304, 0xb2aa92, 0x141810, 0x080c04, 0x10710c,
    0x82a2cf, 0xaa0800, 0x2c4128, 0xb28e49, 0x614d20, 0x555955, 0xd3d304, 0x4d3c1c,
    0xf7a620, 0x10a210, 0x1c2418, 0xf3ae5d, 0xaa9e71, 0x20e7b2, 0x712810, 0xd3c386,
    0x2c8a28, 0xba510c, 0x6d7159, 0x4d4d49, 0xc3b6aa, 0x005100, 0xf7cb20, 0x518a4d,
    0xb2c7be, 0xba3c04, 0x181c10, 0xdf5108, 0xa6e7a6, 0x24cf9e, 0x927579, 0x5975ae,
    0x69829e, 0xdfe3cf, 0x30200c, 0x2c6928, 0xa2a29e, 0x82c7f7, 0x7d6134, 0xbebeb6,
    0x869e96, 0xd3fbdb, 0x495165, 0x65ef65, 0xd3d3ae, 0x4d92e7, 0xb2beb2, 0xdbdbd7,
    0x717579, 0x594d34, 0xcfb286, 0x142c51, 0xfbfba6, 0x655938, 0x928e82, 0x61411c,
    0x615959, 0x002000, 0x7d6d3c, 0xd7b25d, 0x3cef38, 0x55cb51, 0x24241c, 0xf3c361,
    0xa2a208, 0x3c4138, 0x514920, 0x148a10, 0x103010, 0xf7e77d, 0x493414, 0x2c65b2,
    0xaaebf7, 0x92713c, 0xfbfb04, 0x516169, 0x96c7ef, 0xd74104, 0x040400, 0xf78a38,
    0xf3d38a, 0x006500, 0xe30400, 0xffffdb, 0x69aef7, 0x4d86cf, 0xd3a255, 0xe3efeb,
    0x418aeb, 0x6192cf, 0x10f7c3, 0x828e8e, 0xfffffb, 0x104110, 0x510804, 0xfbfb82,
    0x34c73c, 0x8a8a08, 0x454538, 0x344151, 0x96793c, 0x61611c, 0x82b2eb, 0x004100,
    0x515971, 0xa6aaa2, 0xb2b6b2, 0xfbfbc3, 0x340800, 0x3c4128, 0xcfc7c7, 0xd3d3cf,
    0x204582, 0x0ccb08, 0x1c1c1c, 0x9ab2a6, 0xbea671, 0x6d9ecf, 0x61716d, 0x048a00,
    0xbe7151, 0xc3d3c7, 0x969e96, 0x968679, 0x101c2c, 0x495138, 0x3875be, 0x1c1408,
    0xc7c7c3, 0x282c20, 0xcfe3d3, 0x6d8271, 0x1c3c65, 0x08ef04, 0x755534, 0xa6926d,
    0x797979, 0x8af386, 0x285592, 0xe7e7e3, 0x516145, 0x9a491c, 0x616965, 0xa29e8e,
    0xd38679, 0x514120, 0xe3c78a, 0x007100, 0xbe9e51, 0x0c5110, 0xaaaaa6, 0x30302c,
    0x3479d3, 0x303018, 0x8e8249, 0xc3fbcb, 0x71716d, 0xa62804, 0x45454d, 0x140c04,
    0x757508, 0x79cb71, 0x0c6d4d, 0xd3fbfb, 0xaeb2aa, 0x929292, 0x559ef3, 0x005d00,
    0xb2d7e3, 0xc3fbf7, 0x51593c, 0xb2b204, 0x8e6534, 0xef8604, 0xe3fbf7, 0x616161,
    0xdfe3df, 0x0c101c, 0x0c1008, 0x0c1808, 0x008659, 0xfbfb24, 0x716134, 0xc72c04,
    0x9ac7ae, 0x4dae45, 0x2ca628, 0x65a2ef, 0x82827d, 0x9ad7f7, 0xf7d365, 0xbae7e3,
    0x003000, 0x715524, 0x3c82df, 0xc3ae8a, 0xd3c3a2, 0xa2fba6, 0xf3fff3, 0xe7d7aa,
    0xc3efef, 0xfbf755, 0xf3f3ef, 0xb2d3be, 0xebeb04, 0xf7e3a6, 0xfffbf0, 0xa0a0a4,
    0x808080, 0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
];

/// MEASURED (dd8 nearest-*, nearestrect-*: 0 wrong of 2048 SetPixel colors and 32 FillRect
/// colors; 36 of 36 ties to the lowest index): the entry a color goes to on an 8 bpp surface's
/// DC is the one at the least Euclidean RGB distance, the lowest index of those
pub fn nearest_entry(tab: &[u32], c: u32) -> u8 {
    let dist = |e: u32| -> u32 {
        [16u32, 8, 0].iter().map(|&s| (e >> s & 255).abs_diff(c >> s & 255).pow(2)).sum()
    };
    let best = tab.iter().take(256).enumerate().min_by_key(|&(i, &e)| (dist(e), i)).map(|(i, _)| i).unwrap_or(0);
    u8::try_from(best).unwrap_or(u8::MAX)
}
pub const DDERR_DCALREADYCREATED: u32 = 0x8876_026c;
pub const DDERR_NODC: u32 = 0x8876_024a;
pub const DDERR_INVALIDCAPS: u32 = 0x8876_0064;
pub const DDERR_NOEXCLUSIVEMODE: u32 = 0x8876_00e1;
pub const DDERR_PRIMARYSURFACEALREADYEXISTS: u32 = 0x8876_0234;
pub const DDERR_NOTFLIPPABLE: u32 = 0x8876_0246;

#[derive(Clone, Default)]
pub struct Surf {
    pub w: i64,
    pub h: i64,
    pub bpp: u32,
    pub px: Vec<u32>,
    pub src_key: Option<u32>,
    pub dest_key: Option<u32>,
    /// the ddsurf format token ("8", "555", "565", "24", "32", "a32"; chains "32")
    pub fmt: &'static str,
    /// the DDSCAPS GetSurfaceDesc reports, and its dwFlags
    pub caps: u32,
    pub flags: u32,
    pub vid: bool,
    /// no clipper / a clipper without a clip list / a clipper with this list
    pub clip: Option<Option<Vec<Rect>>>,
    /// the palette attached (its id)
    pub pal: Option<u32>,
    pub dc: bool,
    /// the rectangles locked (None: the whole surface), in order
    pub locks: Vec<Option<Rect>>,
    /// the first lock's rectangle origin, while ddpoke may write through it
    pub poke: Option<(i64, i64)>,
    /// a flipping chain: (its first slot, this surface's place in it)
    pub chain: Option<(usize, usize)>,
}

impl Surf {
    fn get(&self, x: i64, y: i64) -> u32 {
        self.px[(y * self.w + x) as usize]
    }
    fn set(&mut self, x: i64, y: i64, v: u32) {
        self.px[(y * self.w + x) as usize] = v;
    }
    fn mask(&self) -> u32 {
        if self.bpp == 32 { !0 } else { (1 << self.bpp) - 1 }
    }
}

#[derive(Clone)]
pub struct Pal {
    pub id: u32,
    pub caps: u32,
    /// r g b flags of each entry (2, 4, 16 or 256)
    pub e: Vec<[u8; 4]>,
    /// DDPCAPS_8BITENTRIES: a byte an entry (MEASURED, ddpal golden: CreatePalette, SetEntries
    /// and GetEntries move that many bytes of the PALETTEENTRY arrays given, r g b flags r ...)
    pub bytes: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct Ddraw {
    dev: bool,
    s: [Option<Surf>; 8],
    pal: [Option<Pal>; 4],
    /// palettes replaced in their slot (surfaces may still hold them)
    old: Vec<Pal>,
    next_pal: u32,
    excl: bool,
    primary: bool,
}

/// the line names an empty slot: "noslot" / "nopal" / "nodc", once
struct Skip;

/// 256 entries from xorshift32: r, g, b, flags 0 (SPEC.md)
pub fn pal_entries(seed: u32) -> Vec<[u8; 4]> {
    let mut r = crate::rng::XorShift32::new(seed);
    (0..256).map(|_| [r.next() as u8, r.next() as u8, r.next() as u8, 0]).collect()
}

/// the pitch of a system-memory surface: MEASURED (ddlock golden, every width 1..33 of every
/// format) rows padded to 8 bytes on x86, to 4 on x86-64
pub fn pitch(w: i64, bpp: u32) -> i64 {
    let a = if super::arch() == 64 { 4 } else { 8 };
    hit(clause::DDRAW_PITCH);
    (w * bpp as i64 / 8 + a - 1) / a * a
}

type Rect = (i64, i64, i64, i64);

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// a rect at token i ("all" or 4 numbers) and the next token
fn rect(op: &Op, i: usize) -> Res<(Option<Rect>, usize)> {
    if op.tok.get(i).is_some_and(|t| t == "all") {
        return Ok((None, i + 1));
    }
    Ok((Some((n(op, i)?, n(op, i + 1)?, n(op, i + 2)?, n(op, i + 3)?)), i + 4))
}

/// the rect inside the surface and not empty (MEASURED: else DDERR_INVALIDRECT)
fn check(r: Option<Rect>, s: &Surf) -> Option<Rect> {
    let (x0, y0, x1, y1) = r.unwrap_or((0, 0, s.w, s.h));
    hit(clause::DDRAW_RECT);
    (x0 >= 0 && y0 >= 0 && x1 <= s.w && y1 <= s.h && x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1))
}

/// how a blit within one surface differs from a blit from a copy of its source
#[derive(Clone, Copy, Default)]
struct Within {
    /// pixels written left of the destination's rows (8 bpp backward copies)
    extra: i64,
    /// a stretch takes floor(x·s / d) (as mirrored ones do), not the DDA
    floor: bool,
}

/// a blit within one surface: MEASURED as if from a copy of the source (16, 24 and 32 bpp:
/// ddraw, ddclip overlap-*; 8 bpp: dd8 overlap8-*, overlap8align*, overlap8seq-*) but at 8 bpp:
/// the unkeyed backward copy (moving right and not down, its rectangles overlapping and at least
/// 4 wide, the source's right side on a dword) writes dx mod 4 more pixels left of the
/// destination, each from dx left and dy down of it in memory, through Blt and BltFast, clipped
/// or not, not mirrored (overlap8alignfast, -clip, -key, -down, -mirrorlr, -mirrorud); an
/// overlapping stretch, at every depth, takes the source by floor(x·s / d) (overlap8alignstretch:
/// 50 of 50; ddraw overlapstretch-565, -24, -32: exact, the DDA misses 565 and 32)
fn overlap(s: &Surf, sr: Rect, dr: Rect, k: &Keys) -> Res<Within> {
    let (dx, dy) = (dr.0 - sr.0, dr.1 - sr.1);
    let w = sr.2 - sr.0;
    let overlapping = dr.0 < sr.2 && sr.0 < dr.2 && dr.1 < sr.3 && sr.1 < dr.3;
    let stretched = (dr.2 - dr.0, dr.3 - dr.1) != (w, sr.3 - sr.1);
    if s.bpp != 8 {
        hit(clause::DDRAW_OVERLAP);
        return Ok(Within { extra: 0, floor: overlapping && stretched });
    }
    hit(clause::DDRAW_OVERLAP8);
    if overlapping && stretched {
        return Ok(Within { extra: 0, floor: true });
    }
    let keyed = k.src.is_some() || k.dest.is_some();
    let aligned = sr.2 % 4 == 0 && w >= 4 && overlapping && dx > 0 && dy <= 0;
    Ok(Within { extra: if aligned && !keyed && !k.lr && !k.ud { dx % 4 } else { 0 }, floor: false })
}

/// which source column (row) a destination one takes when a blit stretches: MEASURED (dd8
/// stretchx-*, stretchy-*: every count 1..9 to every other 1..20 of each format, 0 wrong;
/// stretch-*: 160 rectangles, mirrored ones too)
#[derive(Clone, Copy)]
pub enum Stretch {
    /// 24 bpp, and any mirrored blit: floor(x·s / d), a mirrored axis taking destination index
    /// d − 1 − x
    Floor,
    /// 8, 16 and 32 bpp: a DDA, floor((e + x·s) / d) (integer divisions), whose start e when it
    /// shrinks is (s / d / 2)·d + d / 2 and when it grows depends on the axis
    Dda,
}

/// the axis a stretch runs along
#[derive(Clone, Copy)]
pub enum Axis {
    /// growing columns start at e = (s − 1) / 2
    X,
    /// growing rows start at e = s / 2
    Y,
}

impl Stretch {
    pub fn of(bpp: u32, mirrored: bool) -> Stretch {
        if bpp == 24 || mirrored { Stretch::Floor } else { Stretch::Dda }
    }
    /// the source index (0..s) of destination index x (0..d)
    pub fn source(self, axis: Axis, x: i64, s: i64, d: i64) -> i64 {
        if s == d {
            return x;
        }
        let e = match (self, axis) {
            (Stretch::Floor, Axis::X | Axis::Y) => 0,
            (Stretch::Dda, Axis::X) if s < d => (s - 1) / 2,
            (Stretch::Dda, Axis::Y) if s < d => s / 2,
            (Stretch::Dda, Axis::X | Axis::Y) => s / d / 2 * d + d / 2,
        };
        ((e + x * s) / d).min(s - 1)
    }
}

struct Keys {
    src: Option<u32>,
    dest: Option<u32>,
    lr: bool,
    ud: bool,
}

impl Ddraw {
    /// a slot, or Skip when it holds nothing (MEASURED and SPEC.md: "noslot")
    fn slot_or(&self, op: &Op, i: usize) -> Res<Result<usize, Skip>> {
        let s = n(op, i)?;
        if !(0..8).contains(&s) {
            return Err(Fail::Bad);
        }
        Ok(if self.s[s as usize].is_none() { Err(Skip) } else { Ok(s as usize) })
    }

    /// a copy of src's rect sr to d's rect dr (equal sizes), as the keys say, only where the
    /// destination's clip list (if any) and the surface hold the pixel; its HRESULT
    fn copy(&mut self, d: usize, dr: Rect, src: &Surf, sr: Rect, k: &Keys, floor: bool) -> Res<u32> {
        let (w, h) = (dr.2 - dr.0, dr.3 - dr.1);
        let (sw, sh) = (sr.2 - sr.0, sr.3 - sr.1);
        let dst = self.s[d].as_mut().unwrap();
        if dst.bpp != src.bpp {
            hit(clause::DDRAW_FORMATS);
            return Ok(DDERR_UNSUPPORTED);
        }
        hit(clause::DDRAW_COPY);
        if src.bpp == 32 && (k.src.is_some_and(|v| v >> 24 != 0) || k.dest.is_some_and(|v| v >> 24 != 0)) {
            hit(clause::DDRAW_KEY_BITS);
        }
        if k.lr || k.ud {
            hit(clause::DDRAW_MIRROR);
        }
        let list = dst.clip.clone().flatten();
        let rule = Stretch::of(src.bpp, k.lr || k.ud || floor);
        if (w, h) != (sw, sh) {
            hit(clause::DDRAW_STRETCH);
        }
        for y in 0..h {
            for x in 0..w {
                let (tx, ty) = (dr.0 + x, dr.1 + y);
                if !visible(dst, &list, tx, ty) {
                    continue;
                }
                let sx = sr.0 + rule.source(Axis::X, if k.lr { w - 1 - x } else { x }, sw, w);
                let sy = sr.1 + rule.source(Axis::Y, if k.ud { h - 1 - y } else { y }, sh, h);
                let v = src.get(sx, sy);
                if k.src == Some(v) {
                    continue;
                }
                if let Some(dk) = k.dest {
                    if dst.get(tx, ty) != dk {
                        continue;
                    }
                }
                dst.set(tx, ty, v);
            }
        }
        Ok(0)
    }

    /// the pixels an 8 bpp backward Blt writes left of its destination (DDRAW.OVERLAP8): each
    /// row's `n` pixels before dr, each from the byte dx left and dy down of it in memory (rows
    /// a pitch apart; a byte in a row's padding or before the surface is not modeled)
    fn extra(&mut self, d: usize, dr: Rect, src: &Surf, sr: Rect, n: i64) -> Res<()> {
        let (dx, dy) = (dr.0 - sr.0, dr.1 - sr.1);
        let p = pitch(src.w, src.bpp);
        let dst = self.s[d].as_mut().ok_or(Fail::Bad)?;
        let list = dst.clip.clone().flatten();
        for y in dr.1..dr.3 {
            for x in dr.0 - n..dr.0 {
                if !visible(dst, &list, x, y) {
                    continue;
                }
                let a = (y - dy) * p + x - dx;
                let (ry, rx) = (a.div_euclid(p), a.rem_euclid(p));
                if a < 0 || rx >= src.w {
                    return Err(Fail::Unmodeled(open::DDRAW_OVERLAP8));
                }
                dst.set(x, y, src.get(rx, ry));
            }
        }
        Ok(())
    }

    /// whether a surface is busy for a blit: locked or its DC out (MEASURED: DDERR_SURFACEBUSY)
    fn busy(&self, i: usize) -> bool {
        let s = self.s[i].as_ref().unwrap();
        let b = !s.locks.is_empty() || s.dc;
        if b {
            hit(clause::DDRAW_BUSY);
        }
        b
    }

    /// the destination rectangle of a blit: with a clip list it may lie past the surface
    /// (MEASURED, ddclip golden: clipped), else DDRAW.RECT
    fn dest_rect(&self, d: usize, r: Option<Rect>) -> Option<Rect> {
        let dst = self.s[d].as_ref().unwrap();
        if let Some(Some(_)) = &dst.clip {
            hit(clause::DDRAW_CLIP_BLT);
            let (x0, y0, x1, y1) = r.unwrap_or((0, 0, dst.w, dst.h));
            return (x0 < x1 && y0 < y1).then_some((x0, y0, x1, y1));
        }
        check(r, dst)
    }

    fn free_slot(&mut self, i: usize) {
        // (a front buffer takes its chain with it)
        let root = self.s[i].as_ref().and_then(|s| s.chain).is_some_and(|c| c.0 == i && c.1 == 0);
        if root {
            for k in 0..8 {
                if k != i && self.s[k].as_ref().and_then(|s| s.chain).is_some_and(|c| c.0 == i) {
                    self.s[k] = None;
                }
            }
        }
        self.s[i] = None;
    }

    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        macro_rules! slot {
            ($i:expr) => {
                match self.slot_or(op, $i)? {
                    Ok(s) => s,
                    Err(Skip) => {
                        out.push("noslot".into());
                        return Ok(true);
                    }
                }
            };
        }
        macro_rules! ret {
            ($v:expr) => {{
                out.push(format!("ret {:08x}", $v));
                return Ok(true);
            }};
        }
        let t = op.tok.as_slice();
        const OPS: &[&str] = &[
            "ddsurf", "ddfill", "dddump", "ddkey", "ddblt", "ddcolor", "ddfast", "ddclip", "ddcliplist", "ddpal", "ddpalget", "ddpalset", "ddpalcaps", "ddsetpal", "ddgetpal", "ddgetdc", "ddreldc", "dcpix",
            "dcget", "dcrect", "dcbpp", "ddlock", "ddpoke", "ddunlock", "dddesc", "ddcoop", "ddchain", "ddflip",
        ];
        match t[0].as_str() {
            "dd" => {
                *self = Ddraw { dev: true, ..Default::default() };
                hit(clause::DDRAW_DEVICE);
                out.push("dd ok".into());
            }
            o if OPS.contains(&o) && !self.dev => {
                // (lines the executor reads before it looks for the device)
                match o {
                    "ddsurf" => {
                        let (i, w, h) = (n(op, 1)?, n(op, 2)?, n(op, 3)?);
                        if !(0..8).contains(&i) || !(1..=256).contains(&w) || !(1..=256).contains(&h) {
                            return Err(Fail::Bad);
                        }
                    }
                    "ddpal" => {
                        n(op, 1)?;
                    }
                    "ddcoop" | "ddchain" => {}
                    // (every other line names a slot first: there are none)
                    _ => {
                        out.push(if o.starts_with("ddpal") { "nopal" } else { "noslot" }.into());
                        return Ok(true);
                    }
                }
                out.push("nodev".into());
            }
            "ddsurf" => {
                let (i, w, h) = (n(op, 1)?, n(op, 2)?, n(op, 3)?);
                if !(0..8).contains(&i) || !(1..=256).contains(&w) || !(1..=256).contains(&h) {
                    return Err(Fail::Bad);
                }
                let (bpp, fmt, pf) = match t.get(4).map(String::as_str) {
                    Some("8") => (8, "8", 0x60),
                    Some("555") => (16, "555", 0x40),
                    Some("565") => (16, "565", 0x40),
                    Some("24") => (24, "24", 0x40),
                    Some("32") => (32, "32", 0x40),
                    Some("a32") => (32, "a32", 0x41),
                    _ => return Err(Fail::Bad),
                };
                let _ = pf;
                let vid = match t.get(5).map(String::as_str) {
                    None => false,
                    Some("vid") => true,
                    _ => return Err(Fail::Bad),
                };
                self.free_slot(i as usize);
                if vid && bpp == 24 {
                    // MEASURED: the display driver of Windows' runner has no 24 bpp surfaces
                    hit(clause::DDRAW_VID);
                    out.push(format!("ddsurf failed {DDERR_UNSUPPORTED:08x}"));
                    return Ok(true);
                }
                // (the pixels a new surface starts with are not documented: cases fill first)
                let caps = if vid { 0x1000_4040 } else { 0x840 };
                self.s[i as usize] = Some(Surf { w, h, bpp, fmt, caps, flags: 0x100f, vid, px: vec![0; (w * h) as usize], ..Default::default() });
            }
            "ddfill" | "dddump" => {
                let s = slot!(1);
                let r = if t[0] == "dddump" && t.len() > 2 { rect(op, 2)?.0 } else { None };
                let seed = if t[0] == "ddfill" { n(op, 2)? as u32 } else { 0 };
                if let Some(hr) = self.lock_hr(s, r) {
                    out.push(format!("lock hr {hr:08x}"));
                    return Ok(true);
                }
                let surf = self.s[s].as_mut().unwrap();
                let bytes = surf.bpp / 8;
                if t[0] == "ddfill" {
                    let mut g = crate::rng::XorShift32::new(seed);
                    for p in surf.px.iter_mut() {
                        let mut v = 0;
                        for b in 0..bytes {
                            v |= (g.next() & 0xff) << (8 * b);
                        }
                        *p = v;
                    }
                } else {
                    let (x0, y0, x1, y1) = r.unwrap_or((0, 0, surf.w, surf.h));
                    out.push(format!("dd {s} {} {} {}", x1 - x0, y1 - y0, surf.bpp));
                    for y in y0..y1 {
                        out.push((x0..x1).map(|x| surf.get(x, y).to_le_bytes()[..bytes as usize].iter().map(|b| format!("{b:02x}")).collect::<String>()).collect());
                    }
                }
            }
            "ddkey" => {
                let s = slot!(1);
                let v = if t.get(3).is_some_and(|v| v == "none") { None } else { Some(n(op, 3)? as u32) };
                let surf = self.s[s].as_mut().unwrap();
                let v = v.map(|v| v & surf.mask());
                match t.get(2).map(String::as_str) {
                    Some("src") => surf.src_key = v,
                    Some("dest") => {
                        // MEASURED: no destination color keys on system-memory surfaces
                        hit(clause::DDRAW_KEY_DESTHW);
                        out.push(format!("ddkey hr {DDERR_NOCOLORKEYHW:08x}"));
                        return Ok(true);
                    }
                    _ => return Err(Fail::Bad),
                }
                hit(clause::DDRAW_KEY_SET);
            }
            "ddcolor" => {
                let d = slot!(1);
                let (r, i) = rect(op, 2)?;
                let v = n(op, i)? as u32;
                if t.len() != i + 1 {
                    return Err(Fail::Bad);
                }
                if self.busy(d) {
                    ret!(DDERR_SURFACEBUSY);
                }
                if self.s[d].as_ref().unwrap().clip == Some(None) {
                    hit(clause::DDRAW_CLIP_NOLIST);
                    ret!(DDERR_NOCLIPLIST);
                }
                let Some(r) = self.dest_rect(d, r) else { ret!(DDERR_INVALIDRECT) };
                let surf = self.s[d].as_mut().unwrap();
                if v & !surf.mask() != 0 {
                    hit(clause::DDRAW_FILL_HIGHBITS);
                }
                let v = v & surf.mask();
                hit(clause::DDRAW_FILL);
                let list = surf.clip.clone().flatten();
                for y in r.1..r.3 {
                    for x in r.0..r.2 {
                        if visible(surf, &list, x, y) {
                            surf.set(x, y, v);
                        }
                    }
                }
                ret!(0);
            }
            "ddblt" => {
                let d = slot!(1);
                let (dr, i) = rect(op, 2)?;
                let s = if t.get(i).is_some_and(|v| v == "-") { None } else { Some(slot!(i)) };
                let (sr, mut i) = rect(op, i + 1)?;
                let mut k = Keys { src: None, dest: None, lr: false, ud: false };
                let src = s.map(|s| self.s[s].clone().unwrap());
                let dst = self.s[d].as_ref().unwrap();
                let (mut unset, mut destkey) = (false, false);
                while i < t.len() {
                    match t[i].as_str() {
                        "keysrc" => match src.as_ref().and_then(|s| s.src_key) {
                            Some(v) => k.src = Some(v),
                            None => unset = true,
                        },
                        // (a system-memory surface never has a destination key: like an unset key)
                        "keydest" => unset = true,
                        "mirrorlr" => k.lr = true,
                        "mirrorud" => k.ud = true,
                        "keysrcov" | "keydestov" => {
                            i += 1;
                            let v = n(op, i)? as u32;
                            hit(clause::DDRAW_KEY_OVERRIDE);
                            if t[i - 1] == "keysrcov" {
                                k.src = Some(v & src.as_ref().map_or(!0, |s| s.mask()));
                            } else {
                                k.dest = Some(v & dst.mask());
                                destkey = true;
                            }
                        }
                        _ => return Err(Fail::Bad),
                    }
                    i += 1;
                }
                // MEASURED: the order Windows checks them in
                if destkey {
                    hit(clause::DDRAW_KEY_DESTHW);
                    ret!(DDERR_NOCOLORKEYHW);
                }
                if unset {
                    hit(clause::DDRAW_KEY_UNSET);
                    ret!(E_INVALIDARG);
                }
                if k.src.is_some() {
                    hit(clause::DDRAW_KEY);
                }
                let Some(src) = src else {
                    hit(clause::DDRAW_NOSOURCE);
                    ret!(E_INVALIDARG);
                };
                if self.busy(d) || self.busy(s.unwrap()) {
                    ret!(DDERR_SURFACEBUSY);
                }
                if self.s[d].as_ref().unwrap().clip == Some(None) {
                    hit(clause::DDRAW_CLIP_NOLIST);
                    ret!(DDERR_NOCLIPLIST);
                }
                let (Some(dr), Some(sr)) = (self.dest_rect(d, dr), check(sr, &src)) else { ret!(DDERR_INVALIDRECT) };
                let within = if s == Some(d) { overlap(&src, sr, dr, &k)? } else { Within::default() };
                let hr = self.copy(d, dr, &src, sr, &k, within.floor)?;
                if within.extra > 0 {
                    self.extra(d, dr, &src, sr, within.extra)?;
                }
                ret!(hr);
            }
            "ddfast" => {
                let d = slot!(1);
                let (x, y) = (n(op, 2)?, n(op, 3)?);
                let s = slot!(4);
                let (sr, mut i) = rect(op, 5)?;
                let src = self.s[s].clone().unwrap();
                let dst = self.s[d].as_ref().unwrap();
                let mut k = Keys { src: None, dest: None, lr: false, ud: false };
                let (mut srckey, mut destkey) = (false, false);
                while i < t.len() {
                    match t[i].as_str() {
                        "srckey" => srckey = true,
                        "destkey" => destkey = true,
                        _ => return Err(Fail::Bad),
                    }
                    i += 1;
                }
                if dst.clip.is_some() {
                    // MEASURED (ddclip golden): DDERR_BLTFASTCANTCLIP, with or without a list
                    hit(clause::DDRAW_CLIP_FAST);
                    ret!(DDERR_BLTFASTCANTCLIP);
                }
                // MEASURED: a key flag with no key is E_INVALIDARG; with both flags only the
                // source key counts (a system-memory surface has no destination key)
                if srckey && destkey {
                    hit(clause::DDRAW_FAST_BOTHKEYS);
                }
                if srckey {
                    k.src = src.src_key;
                }
                if k.src.is_some() {
                    hit(clause::DDRAW_KEY);
                }
                hit(clause::DDRAW_FAST);
                let Some(sr) = check(sr, &src) else { ret!(DDERR_INVALIDRECT) };
                let dr = (x, y, x + sr.2 - sr.0, y + sr.3 - sr.1);
                let Some(dr) = check(Some(dr), dst) else { ret!(DDERR_INVALIDRECT) };
                // (MEASURED: after the rectangles, unlike Blt)
                if (srckey && k.src.is_none()) || (destkey && !srckey) {
                    hit(clause::DDRAW_KEY_UNSET);
                    ret!(E_INVALIDARG);
                }
                if self.busy(d) || self.busy(s) {
                    ret!(DDERR_SURFACEBUSY);
                }
                let within = if s == d { overlap(&src, sr, dr, &k)? } else { Within::default() };
                let hr = self.copy(d, dr, &src, sr, &k, within.floor)?;
                if within.extra > 0 {
                    self.extra(d, dr, &src, sr, within.extra)?;
                }
                ret!(hr);
            }
            "ddclip" => {
                let s = slot!(1);
                let surf = self.s[s].as_mut().unwrap();
                match t.get(2).map(String::as_str) {
                    Some("none") => {
                        hit(clause::DDRAW_CLIP_SET);
                        surf.clip = None;
                    }
                    Some("empty") => {
                        hit(clause::DDRAW_CLIP_SET);
                        surf.clip = Some(None);
                    }
                    _ => {
                        if (t.len() - 2) % 4 != 0 || t.len() - 2 > 64 {
                            return Err(Fail::Bad);
                        }
                        let mut l = Vec::new();
                        for k in 0..(t.len() - 2) / 4 {
                            l.push((n(op, 2 + 4 * k)?, n(op, 3 + 4 * k)?, n(op, 4 + 4 * k)?, n(op, 5 + 4 * k)?));
                        }
                        hit(clause::DDRAW_CLIP_SET);
                        surf.clip = Some(Some(l));
                    }
                }
            }
            "ddcliplist" => {
                let s = slot!(1);
                match &self.s[s].as_ref().unwrap().clip {
                    None => {
                        hit(clause::DDRAW_CLIP_NONE);
                        out.push(format!("cliplist clipper hr {DDERR_NOCLIPPERATTACHED:08x}"));
                    }
                    Some(None) => {
                        hit(clause::DDRAW_CLIP_NOLIST);
                        out.push(format!("cliplist hr {DDERR_NOCLIPLIST:08x}"));
                    }
                    Some(Some(l)) => {
                        // MEASURED: the list as given (not made into bands), the bounding box the
                        // executor gave
                        hit(clause::DDRAW_CLIP_LIST);
                        let b = l.iter().skip(1).fold(l[0], |b, r| (b.0.min(r.0), b.1.min(r.1), b.2.max(r.2), b.3.max(r.3)));
                        let mut line = format!("cliplist {} {} {} {} {} :", l.len(), b.0, b.1, b.2, b.3);
                        for r in l {
                            line += &format!(" {} {} {} {}", r.0, r.1, r.2, r.3);
                        }
                        out.push(line);
                    }
                }
            }
            "ddpal" => {
                let p = n(op, 1)?;
                if !(0..4).contains(&p) {
                    return Err(Fail::Bad);
                }
                let seed = n(op, 2)? as u32;
                let mut f = std::collections::BTreeSet::new();
                for x in &t[3..] {
                    if !["1bit", "2bit", "4bit", "8bit", "8bitentries", "allow256", "primary", "alpha", "initialize"].contains(&x.as_str()) {
                        return Err(Fail::Bad);
                    }
                    f.insert(x.as_str());
                }
                if let Some(o) = self.pal[p as usize].take() {
                    self.old.push(o);
                }
                let sizes: Vec<_> = [("1bit", 2, 0x100), ("2bit", 4, 0x200), ("4bit", 16, 0x1), ("8bit", 256, 0x4)].into_iter().filter(|s| f.contains(s.0)).collect();
                // MEASURED (ddpal golden): exactly one size; ALLOW256 only with 8 bits,
                // 8BITENTRIES only with fewer; PRIMARYSURFACE refused (no primary surface);
                // INITIALIZE and ALPHA ignored
                hit(clause::DDRAW_PAL_CREATE);
                let ok = sizes.len() == 1 && !f.contains("primary") && (!f.contains("allow256") || sizes[0].1 == 256) && (!f.contains("8bitentries") || sizes[0].1 < 256) && !(f.contains("allow256") && f.contains("8bitentries"));
                if !ok {
                    out.push(format!("ddpal hr {E_INVALIDARG:08x}"));
                    return Ok(true);
                }
                let (_, cnt, sz) = sizes[0];
                let allow = f.contains("allow256");
                let mut e = pal_entries(seed);
                e.truncate(cnt);
                if cnt == 256 && !allow {
                    // MEASURED: without ALLOW256 entries 0 and 255 are black and white
                    hit(clause::DDRAW_PAL_RESERVED);
                    e[0] = [0, 0, 0, 0];
                    e[255] = [255, 255, 255, 0];
                }
                let bytes = f.contains("8bitentries").then(|| {
                    hit(clause::DDRAW_PAL_8BITENTRIES);
                    pal_entries(seed).iter().flatten().take(cnt).copied().collect()
                });
                self.next_pal += 1;
                let caps = sz | if f.contains("8bitentries") { 2 } else { 0 } | if allow { 0x40 } else { 0 };
                self.pal[p as usize] = Some(Pal { id: self.next_pal, caps, e, bytes });
            }
            "ddpalget" | "ddpalset" | "ddpalcaps" => {
                let p = n(op, 1)?;
                if !(0..4).contains(&p) {
                    return Err(Fail::Bad);
                }
                let (st, cnt) = if t[0] == "ddpalcaps" { (0, 0) } else { (n(op, 2)?, n(op, 3)?) };
                if t[0] != "ddpalcaps" && !(0..=256).contains(&cnt) {
                    return Err(Fail::Bad);
                }
                let seed = if t[0] == "ddpalset" { n(op, 4)? as u32 } else { 0 };
                let Some(pal) = self.pal[p as usize].as_mut() else {
                    out.push("nopal".into());
                    return Ok(true);
                };
                if t[0] == "ddpalcaps" {
                    hit(clause::DDRAW_PAL_CAPS);
                    out.push(format!("palcaps {:08x}", pal.caps));
                    return Ok(true);
                }
                // MEASURED: a range past the palette's entries or of none: E_INVALIDARG
                hit(clause::DDRAW_PAL_RANGE);
                if cnt == 0 || st < 0 || st + cnt > pal.e.len() as i64 {
                    out.push(format!("{} hr {E_INVALIDARG:08x}", if t[0] == "ddpalget" { "palget" } else { "palset" }));
                    return Ok(true);
                }
                if let Some(b) = pal.bytes.as_mut() {
                    let (st, cnt) = (st as usize, cnt as usize);
                    if t[0] == "ddpalget" {
                        // (the rest of the executor's 4 bytes an entry as it filled them, 0xcc)
                        out.push(format!("pal {}{}", b[st..st + cnt].iter().map(|v| format!("{v:02x}")).collect::<String>(), "cc".repeat(3 * cnt)));
                    } else {
                        let new: Vec<u8> = pal_entries(seed).iter().flatten().copied().collect();
                        b[st..st + cnt].copy_from_slice(&new[..cnt]);
                    }
                    return Ok(true);
                }
                if t[0] == "ddpalget" {
                    hit(clause::DDRAW_PAL_GET);
                    out.push(format!("pal {}", pal.e[st as usize..(st + cnt) as usize].iter().flatten().map(|b| format!("{b:02x}")).collect::<String>()));
                } else {
                    hit(clause::DDRAW_PAL_SET);
                    let new = pal_entries(seed);
                    let reserved = pal.e.len() == 256 && pal.caps & 0x40 == 0;
                    for (k, i) in (st..st + cnt).enumerate() {
                        // MEASURED: without ALLOW256 entries 0 and 255 stay as they are
                        if reserved && (i == 0 || i == 255) {
                            hit(clause::DDRAW_PAL_RESERVED);
                            continue;
                        }
                        pal.e[i as usize] = new[k];
                    }
                }
            }
            "ddsetpal" => {
                let s = slot!(1);
                let p = if t.get(2).is_some_and(|v| v == "none") {
                    None
                } else {
                    let p = n(op, 2)?;
                    if !(0..4).contains(&p) {
                        return Err(Fail::Bad);
                    }
                    match &self.pal[p as usize] {
                        Some(pal) => Some(pal.clone()),
                        None => {
                            out.push("nopal".into());
                            return Ok(true);
                        }
                    }
                };
                let surf = self.s[s].as_mut().unwrap();
                hit(clause::DDRAW_PAL_ATTACH);
                // MEASURED: a palette on a surface without palette indices (or of other than 256
                // entries on 8 bpp): DDERR_INVALIDPIXELFORMAT; detaching none:
                // DDERR_NOPALETTEATTACHED
                match p {
                    Some(pal) if surf.bpp != 8 || pal.e.len() != 256 => out.push(format!("setpal hr {DDERR_INVALIDPIXELFORMAT:08x}")),
                    Some(pal) => surf.pal = Some(pal.id),
                    None if surf.pal.is_none() => out.push(format!("setpal hr {DDERR_NOPALETTEATTACHED:08x}")),
                    None => surf.pal = None,
                }
            }
            "ddgetpal" => {
                let s = slot!(1);
                hit(clause::DDRAW_PAL_ATTACH);
                match self.s[s].as_ref().unwrap().pal {
                    None => out.push(format!("getpal hr {DDERR_NOPALETTEATTACHED:08x}")),
                    Some(id) => match self.pal.iter().position(|p| p.as_ref().is_some_and(|p| p.id == id)) {
                        Some(k) => out.push(format!("getpal {k}")),
                        None => out.push("getpal other".into()),
                    },
                }
            }
            "ddgetdc" | "ddreldc" => {
                let s = slot!(1);
                let surf = self.s[s].as_mut().unwrap();
                hit(clause::DDRAW_DC);
                if t[0] == "ddgetdc" {
                    // MEASURED: a second GetDC: DDERR_DCALREADYCREATED (also on a locked surface,
                    // whose first GetDC works)
                    if surf.dc {
                        out.push(format!("getdc hr {DDERR_DCALREADYCREATED:08x}"));
                    } else {
                        surf.dc = true;
                    }
                } else if surf.dc {
                    surf.dc = false;
                } else {
                    out.push(format!("reldc hr {DDERR_NODC:08x}"));
                }
            }
            "dcpix" | "dcget" | "dcrect" | "dcbpp" => {
                let s = slot!(1);
                let pal = self.s[s].as_ref().unwrap().pal.and_then(|id| self.pal_by_id(id)).map(|p| p.e.iter().map(|e| (e[0] as u32) << 16 | (e[1] as u32) << 8 | e[2] as u32).collect::<Vec<_>>());
                let surf = self.s[s].as_mut().unwrap();
                if !surf.dc {
                    out.push("nodc".into());
                    return Ok(true);
                }
                if t[0] == "dcbpp" {
                    // MEASURED: BITSPIXEL is the display's (32 on Windows' runner), the bitmap is the
                    // surface's
                    hit(clause::DDRAW_DC_BPP);
                    out.push(format!("dcbpp 32 {} {} {}", surf.bpp, surf.w, surf.h));
                    return Ok(true);
                }
                let g = crate::model::gdi::Surf::new(1, 1, surf.bpp, surf.fmt == "565", true, pal.clone().unwrap_or_default());
                if t[0] == "dcrect" {
                    let (x0, y0, x1, y1) = (n(op, 2)?, n(op, 3)?, n(op, 4)?, n(op, 5)?);
                    // MEASURED: a rectangle turned round is filled as its normal form
                    let (x0, x1, y0, y1) = (x0.min(x1), x0.max(x1), y0.min(y1), y0.max(y1));
                    let c = crate::case::color(t.get(6).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?;
                    hit(clause::DDRAW_DC_FILL);
                    let v = if surf.bpp == 8 {
                        hit(clause::DDRAW_DC_NEAREST);
                        nearest_entry(pal.as_deref().unwrap_or(&SYSTEM_PALETTE[..]), c).into()
                    } else {
                        g.from_rgb(c)
                    };
                    for y in y0.max(0)..y1.min(surf.h) {
                        for x in x0.max(0)..x1.min(surf.w) {
                            surf.set(x, y, v);
                        }
                    }
                    out.push("ret 1".into());
                    return Ok(true);
                }
                let (x, y) = (n(op, 2)?, n(op, 3)?);
                let c = if t[0] == "dcpix" { Some(crate::case::color(t.get(4).ok_or(Fail::Bad)?).ok_or(Fail::Bad)?) } else { None };
                if !(0..surf.w).contains(&x) || !(0..surf.h).contains(&y) {
                    hit(clause::DDRAW_DC_OUTSIDE);
                    out.push("pixel invalid".into());
                    return Ok(true);
                }
                hit(clause::DDRAW_DC_PIXEL);
                if surf.bpp == 8 {
                    let tab: &[u32] = match &pal {
                        Some(p) => p,
                        None => {
                            hit(clause::DDRAW_DC_NOPAL);
                            &SYSTEM_PALETTE
                        }
                    };
                    if let Some(c) = c {
                        hit(clause::DDRAW_DC_NEAREST);
                        surf.set(x, y, nearest_entry(tab, c).into());
                    }
                    let e = usize::try_from(surf.get(x, y) & 0xff).ok().and_then(|i| tab.get(i)).copied().unwrap_or(0);
                    out.push(format!("pixel {:06x}", e & 0xff_ffff));
                    return Ok(true);
                }
                if let Some(c) = c {
                    surf.set(x, y, g.from_rgb(c));
                }
                out.push(format!("pixel {:06x}", g.to_rgb(surf.get(x, y) & g.mask())));
            }
            _ => return self.op2(op, out),
        }
        Ok(true)
    }

    fn pal_by_id(&self, id: u32) -> Option<&Pal> {
        self.pal.iter().flatten().chain(&self.old).find(|p| p.id == id)
    }

    /// why Lock of rectangle r (None: all) fails now, if it does
    fn lock_hr(&self, s: usize, r: Option<Rect>) -> Option<u32> {
        let surf = self.s[s].as_ref().unwrap();
        hit(clause::DDRAW_LOCK_BUSY);
        if surf.dc || surf.locks.iter().any(|&l| overlaps(l, r)) {
            return Some(DDERR_SURFACEBUSY);
        }
        None
    }

    fn op2(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        macro_rules! slot {
            ($i:expr) => {
                match self.slot_or(op, $i)? {
                    Ok(s) => s,
                    Err(Skip) => {
                        out.push("noslot".into());
                        return Ok(true);
                    }
                }
            };
        }
        let t = op.tok.as_slice();
        match t[0].as_str() {
            "ddlock" => {
                let s = slot!(1);
                let (r, mut i) = rect(op, 2)?;
                while i < t.len() {
                    if !["readonly", "writeonly", "nosyslock", "nowait"].contains(&t[i].as_str()) {
                        return Err(Fail::Bad);
                    }
                    i += 1;
                }
                let surf = self.s[s].as_ref().unwrap();
                // MEASURED: a rectangle past the surface or turned round: E_INVALIDARG (an empty
                // one is fine); one overlapping a lock held, or the DC out: DDERR_SURFACEBUSY
                hit(clause::DDRAW_LOCK_RECT);
                if let Some((x0, y0, x1, y1)) = r {
                    if x0 < 0 || y0 < 0 || x1 > surf.w || y1 > surf.h || x0 > x1 || y0 > y1 {
                        out.push(format!("lock hr {E_INVALIDARG:08x}"));
                        return Ok(true);
                    }
                }
                if let Some(hr) = self.lock_hr(s, r) {
                    out.push(format!("lock hr {hr:08x}"));
                    return Ok(true);
                }
                let surf = self.s[s].as_mut().unwrap();
                let p = surf_pitch(surf)?;
                let (x0, y0) = r.map_or((0, 0), |r| (r.0, r.1));
                hit(clause::DDRAW_LOCK);
                out.push(format!("lock {p} {}", y0 * p + x0 * (surf.bpp as i64 / 8)));
                if surf.locks.is_empty() {
                    surf.poke = Some((x0, y0));
                }
                surf.locks.push(r);
            }
            "ddpoke" => {
                let s = slot!(1);
                let (x, y) = (n(op, 2)?, n(op, 3)?);
                let hex = t.get(4).ok_or(Fail::Bad)?;
                if t.len() != 5 || hex.len() % 2 != 0 || x < 0 || y < 0 {
                    return Err(Fail::Bad);
                }
                let surf = self.s[s].as_mut().unwrap();
                let Some((x0, y0)) = surf.poke else {
                    out.push("nolock".into());
                    return Ok(true);
                };
                let b = surf.bpp as i64 / 8;
                hit(clause::DDRAW_LOCK_WRITE);
                for k in 0..hex.len() as i64 / 2 {
                    let v = u32::from_str_radix(&hex[2 * k as usize..2 * k as usize + 2], 16).map_err(|_| Fail::Bad)?;
                    let bx = x0 * b + x + k;
                    let (px, py, byte) = (bx / b, y0 + y, bx % b);
                    if px >= surf.w || py >= surf.h {
                        // (into the padding past the row, or past the surface)
                        return Err(Fail::Unmodeled(open::DDRAW_LOCK_PAD));
                    }
                    let old = surf.get(px, py);
                    surf.set(px, py, old & !(0xff << (8 * byte)) | v << (8 * byte));
                }
            }
            "ddunlock" => {
                let s = slot!(1);
                let r = if t.get(2).is_some_and(|v| v == "none") { None } else { rect(op, 2)?.0 };
                let surf = self.s[s].as_mut().unwrap();
                hit(clause::DDRAW_UNLOCK);
                // MEASURED: Unlock(NULL) takes the first lock; a rectangle takes the lock of that
                // rectangle (the whole surface's lock also as its own rectangle); none:
                // DDERR_NOTLOCKED; a lock under a DC taken later: DDERR_INVALIDRECT
                if surf.dc && !surf.locks.is_empty() {
                    hit(clause::DDRAW_UNLOCK_DC);
                    out.push(format!("unlock {DDERR_INVALIDRECT:08x}"));
                    return Ok(true);
                }
                let all = (0, 0, surf.w, surf.h);
                let k = match r {
                    None => (!surf.locks.is_empty()).then_some(0),
                    Some(r) => surf.locks.iter().position(|l| l.unwrap_or(all) == r),
                };
                match k {
                    None => out.push(format!("unlock {DDERR_NOTLOCKED:08x}")),
                    Some(k) => {
                        surf.locks.remove(k);
                        if surf.locks.is_empty() {
                            surf.poke = None;
                        }
                        out.push("unlock 00000000".into());
                    }
                }
            }
            "dddesc" => {
                let s = slot!(1);
                let surf = self.s[s].as_ref().unwrap();
                let p = surf_pitch(surf)?;
                let (pf, bits, m) = match surf.fmt {
                    "8" => (0x60, 8, [0u32, 0, 0, 0]),
                    "555" => (0x40, 16, [0x7c00, 0x3e0, 0x1f, 0]),
                    "565" => (0x40, 16, [0xf800, 0x7e0, 0x1f, 0]),
                    "24" => (0x40, 24, [0xff0000, 0xff00, 0xff, 0]),
                    "a32" => (0x41, 32, [0xff0000, 0xff00, 0xff, 0xff00_0000]),
                    _ => (0x40, 32, [0xff0000, 0xff00, 0xff, 0]),
                };
                hit(clause::DDRAW_DESC);
                out.push(format!("desc {:08x} {} {} {p} {:08x} {pf:08x} {bits} {:08x} {:08x} {:08x} {:08x}", surf.flags, surf.w, surf.h, surf.caps, m[0], m[1], m[2], m[3]));
            }
            "ddcoop" => {
                if !["normal", "exclusive"].contains(&t.get(1).map_or("", String::as_str)) {
                    return Err(Fail::Bad);
                }
                if !self.dev {
                    out.push("nodev".into());
                    return Ok(true);
                }
                // MEASURED: both levels are granted with the executor's hidden popup window
                hit(clause::DDRAW_COOP);
                self.excl = t[1] == "exclusive";
                out.push("coop 00000000".into());
            }
            "ddchain" => return self.chain(op, out),
            "ddflip" => {
                let s = slot!(1);
                let target = if t.get(2).is_some_and(|v| v == "-") { None } else { Some(slot!(2)) };
                for f in &t[3..] {
                    if !["novsync", "nowait", "odd", "even", "interval2"].contains(&f.as_str()) {
                        return Err(Fail::Bad);
                    }
                }
                let surf = self.s[s].as_ref().unwrap();
                hit(clause::DDRAW_FLIP);
                // MEASURED: Flip of a surface that is no chain's front buffer, or to the front
                // buffer itself: DDERR_NOTFLIPPABLE
                let chain = match surf.chain {
                    Some((root, 0)) if root == s => (0..8).filter(|&k| self.s[k].as_ref().is_some_and(|x| x.chain.is_some_and(|c| c.0 == s))).collect::<Vec<_>>(),
                    _ => {
                        out.push(format!("flip {DDERR_NOTFLIPPABLE:08x}"));
                        return Ok(true);
                    }
                };
                if self.s[s].as_ref().unwrap().caps & 0x200 != 0 {
                    // (a primary chain: its pixels are the screen's)
                    hit(clause::DDRAW_FLIP_PRIMARY);
                    out.push(format!("flip {:08x}", if self.excl { 0 } else { DDERR_NOEXCLUSIVEMODE }));
                    return Ok(true);
                }
                let mut chain = chain;
                chain.sort_by_key(|&k| self.s[k].as_ref().unwrap().chain.unwrap().1);
                match target {
                    Some(tg) if tg == s || !chain.contains(&tg) => {
                        out.push(format!("flip {DDERR_NOTFLIPPABLE:08x}"));
                        return Ok(true);
                    }
                    Some(tg) => {
                        // (DDFLIP to a given back buffer: the front buffer and it exchange pixels)
                        hit(clause::DDRAW_FLIP_TARGET);
                        let a = self.s[s].as_ref().unwrap().px.clone();
                        let b = std::mem::replace(&mut self.s[tg].as_mut().unwrap().px, a);
                        self.s[s].as_mut().unwrap().px = b;
                    }
                    None => {
                        // the front buffer shows the first back buffer's pixels, each back buffer
                        // the next one's, the last the front buffer's
                        let first = self.s[chain[0]].as_ref().unwrap().px.clone();
                        for w in 0..chain.len() - 1 {
                            let next = self.s[chain[w + 1]].as_ref().unwrap().px.clone();
                            self.s[chain[w]].as_mut().unwrap().px = next;
                        }
                        self.s[*chain.last().unwrap()].as_mut().unwrap().px = first;
                    }
                }
                out.push("flip 00000000".into());
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn chain(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let (s, backs, w, h) = (n(op, 1)?, n(op, 2)?, n(op, 3)?, n(op, 4)?);
        if !(0..8).contains(&s) || !(0..=6).contains(&backs) || !(1..=256).contains(&w) || !(1..=256).contains(&h) {
            return Err(Fail::Bad);
        }
        let mut caps = 0u32;
        for c in &t[5..] {
            caps |= match c.as_str() {
                "primary" => 0x200,
                "offscreen" => 0x40,
                "texture" => 0x1000,
                "3ddevice" => 0x2000,
                "sysmem" => 0x800,
                "vidmem" => 0x4000,
                _ => return Err(Fail::Bad),
            };
        }
        if !self.dev {
            out.push("nodev".into());
            return Ok(true);
        }
        let s = s as usize;
        for i in (s..=(s + backs as usize).min(7)).rev() {
            self.free_slot(i);
        }
        hit(clause::DDRAW_CHAIN);
        let primary = caps & 0x200 != 0;
        let hr = if primary {
            // MEASURED: one primary surface; a primary flipping chain only in exclusive mode
            if backs > 0 && !self.excl {
                Some(DDERR_NOEXCLUSIVEMODE)
            } else if self.primary {
                Some(DDERR_PRIMARYSURFACEALREADYEXISTS)
            } else {
                None
            }
        } else if caps & 0x1040 == 0 {
            // MEASURED: a chain that is no offscreen plain surface or texture (3ddevice alone, or
            // no type): DDERR_INVALIDCAPS
            Some(DDERR_INVALIDCAPS)
        } else if caps & 0x1000 != 0 && super::arch() == 64 {
            // MEASURED: no textures on x86-64 (no Direct3D 7 there)
            Some(E_NOINTERFACE)
        } else {
            None
        };
        if let Some(hr) = hr {
            out.push(format!("chain hr {hr:08x}"));
            return Ok(true);
        }
        if primary {
            self.primary = true;
            // (the screen's size and pixels: not this model's; a chain's back buffers are the
            // screen's size too)
            let n = (backs as usize + 1).min(8 - s);
            for k in 0..n {
                self.s[s + k] = Some(Surf { w: 1, h: 1, bpp: 32, fmt: "32", caps: 0x200, px: vec![0], chain: Some((s, k)), ..Default::default() });
            }
            out.push(format!("chain {n}"));
            return Ok(true);
        }
        // MEASURED: no memory asked: local video memory; the front buffer FRONTBUFFER, each back
        // buffer BACKBUFFER (the first) or none, all FLIP | COMPLEX
        let mem = if caps & 0x4800 == 0 { 0x1000_4000 } else if caps & 0x4000 != 0 { 0x1000_4000 } else { 0 };
        let n = (backs as usize + 1).min(8 - s);
        for k in 0..n {
            let role = if k == 0 { 0x20 } else if k == 1 { 0x4 } else { 0 };
            let flags = if k == 0 { 0x102f } else { 0x100f };
            let c = caps | mem | if backs > 0 { 0x18 | role } else { 0 };
            self.s[s + k] = Some(Surf { w, h, bpp: 32, fmt: "32", caps: c & !(if mem != 0 { 0x800 } else { 0 }), flags, vid: mem != 0, px: vec![0; (w * h) as usize], chain: Some((s, k)), ..Default::default() });
        }
        out.push(format!("chain {n}"));
        Ok(true)
    }
}

/// whether the pixel (x, y) of a surface is written by a blit: on the surface and, with a clip
/// list, in one of its rectangles
fn visible(s: &Surf, list: &Option<Vec<Rect>>, x: i64, y: i64) -> bool {
    (0..s.w).contains(&x) && (0..s.h).contains(&y) && list.as_ref().is_none_or(|l| l.iter().any(|r| (r.0..r.2).contains(&x) && (r.1..r.3).contains(&y)))
}

/// two lock rectangles (None: all) that share a pixel
fn overlaps(a: Option<Rect>, b: Option<Rect>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3,
        _ => true,
    }
}

/// the pitch Lock and GetSurfaceDesc report
fn surf_pitch(s: &Surf) -> Res<i64> {
    if s.vid {
        // MEASURED (ddlock, ddflip golden): 17 pixels of 1, 2, 4 bytes: 20, 36, 72; 8 of 4: 32
        hit(clause::DDRAW_VID);
        return match (s.w, s.bpp) {
            (17, 8) => Ok(20),
            (17, 16) => Ok(36),
            (17, 32) => Ok(72),
            (8, 32) => Ok(32),
            _ => Err(Fail::Unmodeled(open::DDRAW_VID_PITCH)),
        };
    }
    Ok(pitch(s.w, s.bpp))
}
