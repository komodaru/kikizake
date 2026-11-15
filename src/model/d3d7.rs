//! Direct3D 1, 2, 3 and 7 (immediate mode through DirectDraw): the device list, which devices a
//! version makes, render states, Clear, and pretransformed triangles drawn by each device: the
//! software rasterizers (RGB, MMX), the HAL and T&L HAL (WARP behind them on Windows' runner)
//! and the ramp device.
//!
//! Direct3D 7 and older are no longer documented on Microsoft Learn (only DirectDraw is): the
//! clauses come from the DirectX 7 SDK's rules as they still hold (spec/d3d7.txt) and from
//! Windows' answers (golden d3d7, d3d7rast, d3d7shade, d3d7edge, d3d7dev). The device list and
//! the render state defaults are measured tables (src/model/d3d7.txt).
//!
//! Everything a case names is parsed into the types below first (versions, devices, formats,
//! depths, render states, compare functions, blend factors), so that each rule is a match over
//! all of a type's values.

use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit, open};

const TABLES: &str = include_str!("d3d7.txt");

pub const E_NOINTERFACE: u32 = 0x8000_4002;
pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const DDERR_UNSUPPORTED: u32 = 0x8000_4001;
pub const DDERR_INVALIDPIXELFORMAT: u32 = 0x8876_0091;
pub const DDERR_NOPALETTEATTACHED: u32 = 0x8876_023c;
pub const D3DERR_ZBUFFER_NOTPRESENT: u32 = 0x8876_0816;

/// the interface version a case asks for
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ver {
    V1,
    V2,
    V3,
    V7,
}

impl Ver {
    pub const ALL: [Ver; 4] = [Ver::V1, Ver::V2, Ver::V3, Ver::V7];
    pub fn parse(v: i64) -> Option<Ver> {
        match v {
            1 => Some(Ver::V1),
            2 => Some(Ver::V2),
            3 => Some(Ver::V3),
            7 => Some(Ver::V7),
            _ => None,
        }
    }
    pub fn number(self) -> u32 {
        match self {
            Ver::V1 => 1,
            Ver::V2 => 2,
            Ver::V3 => 3,
            Ver::V7 => 7,
        }
    }
}

/// the versions whose devices draw (version 1 devices have no render states, Clear or
/// DrawPrimitive here)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawVer {
    V2,
    V3,
    V7,
}

impl From<DrawVer> for Ver {
    fn from(v: DrawVer) -> Ver {
        match v {
            DrawVer::V2 => Ver::V2,
            DrawVer::V3 => Ver::V3,
            DrawVer::V7 => Ver::V7,
        }
    }
}

/// the device GUIDs the executor knows
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dev {
    Rgb,
    Hal,
    Tnl,
    Ref,
    Mmx,
    Ramp,
    Null,
}

impl Dev {
    pub fn parse(s: &str) -> Option<Dev> {
        match s {
            "rgb" => Some(Dev::Rgb),
            "hal" => Some(Dev::Hal),
            "tnl" => Some(Dev::Tnl),
            "ref" => Some(Dev::Ref),
            "mmx" => Some(Dev::Mmx),
            "ramp" => Some(Dev::Ramp),
            "null" => Some(Dev::Null),
            _ => None,
        }
    }
}

/// how a device that exists draws
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// the RGB and MMX rasterizers: fixed point, truncation, dithering
    Soft { mmx: bool },
    /// the HAL and the T&L HAL: floats, rounding, no dithering
    Hw { tnl: bool },
    /// the ramp (palette) device
    Ramp,
}

/// the render target's pixel format
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fmt {
    P8,
    R565,
    R555,
    R24,
    X32,
}

impl Fmt {
    pub const ALL: [Fmt; 5] = [Fmt::P8, Fmt::R565, Fmt::R555, Fmt::R24, Fmt::X32];
    pub fn parse(s: &str) -> Option<Fmt> {
        match s {
            "8" => Some(Fmt::P8),
            "565" => Some(Fmt::R565),
            "555" => Some(Fmt::R555),
            "24" => Some(Fmt::R24),
            "32" => Some(Fmt::X32),
            _ => None,
        }
    }
    pub fn bytes(self) -> usize {
        match self {
            Fmt::P8 => 1,
            Fmt::R565 | Fmt::R555 => 2,
            Fmt::R24 => 3,
            Fmt::X32 => 4,
        }
    }
    /// the bits of r, g and b
    pub fn bits(self) -> [u32; 3] {
        match self {
            Fmt::R565 => [5, 6, 5],
            Fmt::R555 => [5, 5, 5],
            Fmt::P8 | Fmt::R24 | Fmt::X32 => [8, 8, 8],
        }
    }
    /// where r, g and b sit
    fn shifts(self) -> [u32; 3] {
        match self {
            Fmt::R565 => [11, 5, 0],
            Fmt::R555 => [10, 5, 0],
            Fmt::P8 | Fmt::R24 | Fmt::X32 => [16, 8, 0],
        }
    }
    fn pack(self, q: [u32; 3]) -> u32 {
        let s = self.shifts();
        q[0] << s[0] | q[1] << s[1] | q[2] << s[2]
    }
    /// a pixel's channels as 8 bits each (r, g, b), the low bits 0
    pub fn channels(self, p: u32) -> [u32; 3] {
        let (s, b) = (self.shifts(), self.bits());
        [0, 1, 2].map(|i| (p >> s[i] & ((1 << b[i]) - 1)) << (8 - b[i]))
    }
}

/// the z-buffer's depth
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Depth {
    Z16,
    Z24,
    Z32,
}

impl Depth {
    pub fn parse(s: &str) -> Option<Depth> {
        match s {
            "z16" => Some(Depth::Z16),
            "z24" => Some(Depth::Z24),
            "z32" => Some(Depth::Z32),
            _ => None,
        }
    }
    pub fn bytes(self) -> usize {
        match self {
            Depth::Z16 => 2,
            Depth::Z24 => 3,
            Depth::Z32 => 4,
        }
    }
    pub fn max(self) -> u32 {
        match self {
            Depth::Z16 => 0xffff,
            Depth::Z24 => 0xff_ffff,
            Depth::Z32 => u32::MAX,
        }
    }
}

/// the render states the drawing reads (D3DRENDERSTATETYPE)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rs {
    ZEnable,
    FillMode,
    ShadeMode,
    ZWriteEnable,
    AlphaTestEnable,
    SrcBlend,
    DestBlend,
    CullMode,
    ZFunc,
    AlphaRef,
    AlphaFunc,
    DitherEnable,
    AlphaBlendEnable,
    FogEnable,
    SpecularEnable,
    FogColor,
}

impl Rs {
    pub const fn id(self) -> i64 {
        match self {
            Rs::ZEnable => 7,
            Rs::FillMode => 8,
            Rs::ShadeMode => 9,
            Rs::ZWriteEnable => 14,
            Rs::AlphaTestEnable => 15,
            Rs::SrcBlend => 19,
            Rs::DestBlend => 20,
            Rs::CullMode => 22,
            Rs::ZFunc => 23,
            Rs::AlphaRef => 24,
            Rs::AlphaFunc => 25,
            Rs::DitherEnable => 26,
            Rs::AlphaBlendEnable => 27,
            Rs::FogEnable => 28,
            Rs::SpecularEnable => 29,
            Rs::FogColor => 34,
        }
    }
}

/// D3DCMPFUNC
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cmp {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

impl Cmp {
    pub const ALL: [Cmp; 8] = [Cmp::Never, Cmp::Less, Cmp::Equal, Cmp::LessEqual, Cmp::Greater, Cmp::NotEqual, Cmp::GreaterEqual, Cmp::Always];
    pub fn parse(v: u32) -> Option<Cmp> {
        match v {
            1 => Some(Cmp::Never),
            2 => Some(Cmp::Less),
            3 => Some(Cmp::Equal),
            4 => Some(Cmp::LessEqual),
            5 => Some(Cmp::Greater),
            6 => Some(Cmp::NotEqual),
            7 => Some(Cmp::GreaterEqual),
            8 => Some(Cmp::Always),
            _ => None,
        }
    }
    /// whether `new` passes against `old`
    pub fn passes(self, new: u32, old: u32) -> bool {
        match self {
            Cmp::Never => false,
            Cmp::Less => new < old,
            Cmp::Equal => new == old,
            Cmp::LessEqual => new <= old,
            Cmp::Greater => new > old,
            Cmp::NotEqual => new != old,
            Cmp::GreaterEqual => new >= old,
            Cmp::Always => true,
        }
    }
}

/// D3DBLEND
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Factor {
    Zero,
    One,
    SrcColor,
    InvSrcColor,
    SrcAlpha,
    InvSrcAlpha,
    DestAlpha,
    InvDestAlpha,
    DestColor,
    InvDestColor,
    SrcAlphaSat,
    BothSrcAlpha,
    BothInvSrcAlpha,
}

impl Factor {
    pub fn parse(v: u32) -> Option<Factor> {
        match v {
            1 => Some(Factor::Zero),
            2 => Some(Factor::One),
            3 => Some(Factor::SrcColor),
            4 => Some(Factor::InvSrcColor),
            5 => Some(Factor::SrcAlpha),
            6 => Some(Factor::InvSrcAlpha),
            7 => Some(Factor::DestAlpha),
            8 => Some(Factor::InvDestAlpha),
            9 => Some(Factor::DestColor),
            10 => Some(Factor::InvDestColor),
            11 => Some(Factor::SrcAlphaSat),
            12 => Some(Factor::BothSrcAlpha),
            13 => Some(Factor::BothInvSrcAlpha),
            _ => None,
        }
    }
}

/// D3DCULL
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cull {
    None,
    Cw,
    Ccw,
}

/// D3DFILLMODE
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fill {
    Point,
    Wireframe,
    Solid,
}

/// the render target: pixels and the z-buffer's values
#[derive(Clone)]
struct Target {
    w: i64,
    h: i64,
    fmt: Fmt,
    px: Vec<u32>,
    depth: Option<Depth>,
    z: Vec<u32>,
}

/// a device that was made
struct Device {
    family: Family,
    rt: Target,
}

#[derive(Default)]
pub struct D3d7 {
    ver: Option<Ver>,
    dev: Option<Device>,
    rs: std::collections::BTreeMap<i64, u32>,
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// a 32-bit value of a case token (colors, states; negative numbers as their two's complement)
fn n32(op: &Op, i: usize) -> Res<u32> {
    let v = n(op, i)?;
    u32::try_from(v).or_else(|_| i32::try_from(v).map(i32::cast_unsigned)).map_err(|_| Fail::Bad)
}

/// a color list "c" or "c0/c1/c2"
fn colors(s: &str) -> Res<[u32; 3]> {
    let cs: Vec<u32> = s.split('/').map(|c| num(c).and_then(|v| u32::try_from(v).ok()).ok_or(Fail::Bad)).collect::<Res<_>>()?;
    match cs.as_slice() {
        &[c] => Ok([c; 3]),
        &[a, b, c] => Ok([a, b, c]),
        _ => Err(Fail::Bad),
    }
}

/// a render state's default (None: GetRenderState fails with E_INVALIDARG)
pub fn rs_default(ver: Ver, st: i64) -> Option<u32> {
    let key = format!("rs {} {st} ", ver.number());
    let l = TABLES.lines().find(|l| l.starts_with(&key))?;
    let v = &l[key.len()..];
    v.strip_prefix("= ").and_then(|h| u32::from_str_radix(h, 16).ok())
}

/// ARGB to a software target's pixel: MEASURED (clear golden, d3d7dev color-*: every value of
/// each channel, versions 2, 3, 7, RGB and MMX, Clear and flat triangles) each channel's top bits,
/// 0 elsewhere (no alpha in the target)
pub fn pixel(fmt: Fmt, c: u32) -> u32 {
    let b = fmt.bits();
    fmt.pack([(c >> 16 & 255) >> (8 - b[0]), (c >> 8 & 255) >> (8 - b[1]), (c & 255) >> (8 - b[2])])
}

/// an 8-bit channel to `bits` by rounding: round(v · max / 255)
fn round_to(v: u32, bits: u32) -> u32 {
    (v * ((1 << bits) - 1) + 127) / 255
}

/// ARGB to a HAL target's pixel: MEASURED (d3d7dev color-*: every value of each channel, HAL and
/// T&L HAL, Clear and flat triangles) round(v · max / 255) per channel
pub fn pixel_hw(fmt: Fmt, c: u32) -> u32 {
    let b = fmt.bits();
    fmt.pack([round_to(c >> 16 & 255, b[0]), round_to(c >> 8 & 255, b[1]), round_to(c & 255, b[2])])
}

/// Clear's z value: MEASURED (clearz golden, z = 0, 1/1000, ..., 1 into 16 and 32 bits; d3d7dev
/// zrange: every device) the float z times the largest value, in float, rounded to nearest, at
/// most the largest value
pub fn clear_z(depth: Depth, z: f32) -> u32 {
    let max = depth.max();
    let v = f64::from(z * max as f32) + 0.5;
    if v >= f64::from(max) { max } else { v as u32 }
}

/// x rounded toward zero to a float (the software rasterizers run the FPU in single precision,
/// chopping)
fn chop_f32(x: f64) -> f32 {
    let f = x as f32;
    if f64::from(f).abs() > x.abs() { f32::from_bits(f.to_bits() - 1) } else { f }
}

/// a software triangle's z value (constant over it): MEASURED (zsweep golden: 144 z values, every
/// 7/1000 from 0 to 1, into 16 and 32 bits, all exact; zcover: 0.3, 0.5, 0.6; d3d7shade zfunc-*;
/// d3d7dev zrange and zhist: the low byte of 32-bit z after clears to 0.5 and 1) z · 2^32 less 1
/// in single precision rounded toward zero, at least 0: all 32 bits of it (32 bits; the write
/// keeps the low byte's set bits, tri_z_write) or its top 16 (16 bits)
pub fn tri_z(depth: Depth, z: f32) -> u32 {
    let x = f64::from(chop_f32(f64::from(z) * 4_294_967_296.0 - 1.0)).clamp(0.0, f64::from(u32::MAX)) as u32;
    match depth {
        Depth::Z16 => x >> 16,
        Depth::Z24 | Depth::Z32 => x,
    }
}

/// the z a software triangle writes over `old`: MEASURED (d3d7dev zhist: every 32-bit z written
/// over 1's 0xffffffff ends in 0xff, zrange: over 0.5's 0x80000000 as tri_z) 32 bits keep the
/// old low byte's set bits
pub fn tri_z_write(depth: Depth, z: f32, old: u32) -> u32 {
    match depth {
        Depth::Z16 => tri_z(depth, z),
        Depth::Z24 | Depth::Z32 => tri_z(depth, z) | (old & 0xff),
    }
}

/// a HAL triangle's z value: MEASURED (d3d7dev zrange, d3d7shade zfunc-*: z16) z · 65535 in
/// float, rounded to nearest even
pub fn tri_z_hw(z: f32) -> u32 {
    (z * 65535.0f32).round_ties_even().clamp(0.0, 65535.0) as u32
}

/// the ordered dither matrix of the software rasterizers: MEASURED (d3d7shade dither-*: 256
/// grays and random colors at every position, 565 and 555, RGB and MMX)
const DITHER: [[u32; 4]; 4] = [[7, 1, 6, 0], [3, 5, 2, 4], [6, 0, 7, 1], [2, 4, 3, 5]];

/// an 8-bit channel dithered down to `bits` at (x, y)
pub fn dither(v: u32, bits: u32, x: i64, y: i64) -> u32 {
    let (xi, yi) = (usize::try_from(x.rem_euclid(4)).unwrap_or(0), usize::try_from(y.rem_euclid(4)).unwrap_or(0));
    (((v << bits) + DITHER[yi][xi] * 32 + 16) >> 8).min((1 << bits) - 1)
}

impl D3d7 {
    fn state(&self, st: i64) -> Option<u32> {
        let v = self.ver?;
        self.rs.get(&st).copied().or_else(|| rs_default(v, st))
    }
    fn rs(&self, st: Rs) -> u32 {
        self.state(st.id()).unwrap_or(0)
    }

    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        match t[0].as_str() {
            "d3d" => {
                let v = Ver::parse(n(op, 1)?).ok_or(Fail::Bad)?;
                *self = D3d7::default();
                // MEASURED: the x86-64 DirectDraw has no Direct3D interfaces
                hit(clause::D3D7_X64);
                if super::arch() == 64 {
                    out.push(format!("d3d qi {E_NOINTERFACE:08x}"));
                    return Ok(true);
                }
                hit(clause::D3D7_CREATE);
                self.ver = Some(v);
                out.push("d3d ok".into());
            }
            "d3denum" => {
                let caps = match t.get(1).map(String::as_str) {
                    None => false,
                    Some("caps") if t.len() == 2 => true,
                    _ => return Err(Fail::Bad),
                };
                let Some(v) = self.ver else {
                    out.push("nodev".into());
                    return Ok(true);
                };
                hit(clause::D3D7_ENUM);
                let key = format!("enum {} ", v.number());
                for l in TABLES.lines().filter_map(|l| l.strip_prefix(&key)) {
                    if caps || l.starts_with("dev ") {
                        out.push(l.to_string());
                    }
                }
            }
            "d3ddev" => return self.make_device(op, out),
            "d3drs" | "d3dclear" | "d3dtri" | "d3dline" | "d3dfill" | "d3ddump" | "d3dzdump" => return self.draw(op, out),
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn make_device(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let dev = t.get(1).and_then(|d| Dev::parse(d)).ok_or(Fail::Bad)?;
        let fmt = t.get(2).and_then(|f| Fmt::parse(f)).ok_or(Fail::Bad)?;
        let (w, h) = (n(op, 3)?, n(op, 4)?);
        let depth = match t.get(5) {
            None => None,
            Some(z) => Some(Depth::parse(z).ok_or(Fail::Bad)?),
        };
        if !(1..=256).contains(&w) || !(1..=256).contains(&h) || t.len() > 6 {
            return Err(Fail::Bad);
        }
        let Some(ver) = self.ver else {
            out.push("nodev".into());
            return Ok(true);
        };
        self.dev = None;
        self.rs.clear();
        hit(clause::D3D7_DEVICE);
        let hw = match dev {
            Dev::Hal | Dev::Tnl => true,
            Dev::Rgb | Dev::Ref | Dev::Mmx | Dev::Ramp | Dev::Null => false,
        };
        // MEASURED (d3d7dev color-*, dev8-*): the display's video memory has no 24-bit or 8-bit
        // render targets
        if hw && matches!(fmt, Fmt::R24 | Fmt::P8) {
            hit(clause::D3D7_DEVICE_HW24);
            out.push(format!("d3ddev surface hr {DDERR_UNSUPPORTED:08x}"));
            return Ok(true);
        }
        // MEASURED (dev golden: 7 devices x 4 versions x 9 targets): a 24-bit z-buffer in system
        // memory has no pixel format, none in the display's video memory takes 24 or 32 bits
        match (depth, hw) {
            (Some(Depth::Z24), false) => {
                out.push(format!("d3ddev z hr {DDERR_INVALIDPIXELFORMAT:08x}"));
                return Ok(true);
            }
            (Some(Depth::Z24 | Depth::Z32), true) => {
                out.push(format!("d3ddev z hr {DDERR_UNSUPPORTED:08x}"));
                return Ok(true);
            }
            (None | Some(Depth::Z16), true) | (None | Some(Depth::Z16 | Depth::Z32), false) => {}
        }
        // MEASURED: which device each version makes: RGB, MMX and the HAL everywhere, T&L HAL
        // only 7, ramp only 1 and 2 (3: E_INVALIDARG, 7: unknown); no reference or null device;
        // 8 bpp targets want a palette
        let made = match (dev, ver) {
            (Dev::Ref | Dev::Null, _) | (Dev::Ramp, Ver::V7) => Err(E_NOINTERFACE),
            (Dev::Ramp, Ver::V3) | (Dev::Tnl, Ver::V1 | Ver::V2 | Ver::V3) => Err(E_INVALIDARG),
            (Dev::Rgb | Dev::Hal | Dev::Tnl | Dev::Mmx | Dev::Ramp, _) if fmt == Fmt::P8 => Err(DDERR_NOPALETTEATTACHED),
            (Dev::Rgb, _) => Ok(Family::Soft { mmx: false }),
            (Dev::Mmx, _) => Ok(Family::Soft { mmx: true }),
            (Dev::Hal, _) => Ok(Family::Hw { tnl: false }),
            (Dev::Tnl, Ver::V7) => Ok(Family::Hw { tnl: true }),
            (Dev::Ramp, Ver::V1 | Ver::V2) => Ok(Family::Ramp),
        };
        let family = match made {
            Ok(f) => f,
            Err(hr) => {
                out.push(format!("d3ddev hr {hr:08x}"));
                return Ok(true);
            }
        };
        let k = usize::try_from(w * h).map_err(|_| Fail::Bad)?;
        // MEASURED (v1 golden): a new target holds zeros
        let rt = Target { w, h, fmt, px: vec![0; k], depth, z: vec![0; if depth.is_some() { k } else { 0 }] };
        self.dev = Some(Device { family, rt });
        if depth.is_some() {
            // MEASURED: ZENABLE is on when the target had a z-buffer
            self.rs.insert(Rs::ZEnable.id(), 1);
        }
        out.push("d3ddev ok".into());
        Ok(true)
    }

    fn draw(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let ver = match (self.ver, &self.dev) {
            (Some(Ver::V1), _) | (None, _) | (_, None) => None,
            (Some(Ver::V2), Some(_)) => Some(DrawVer::V2),
            (Some(Ver::V3), Some(_)) => Some(DrawVer::V3),
            (Some(Ver::V7), Some(_)) => Some(DrawVer::V7),
        };
        match t[0].as_str() {
            "d3drs" => {
                let set = match t.get(1).map(String::as_str) {
                    Some("set") => true,
                    Some("get") => false,
                    _ => return Err(Fail::Bad),
                };
                let st = n(op, 2)?;
                let v = if set { n32(op, 3)? } else { 0 };
                if t.len() != if set { 4 } else { 3 } {
                    return Err(Fail::Bad);
                }
                let Some(ver) = ver else {
                    out.push("nodev".into());
                    return Ok(true);
                };
                hit(clause::D3D7_RS);
                if set {
                    // MEASURED (rsset golden): SetRenderState keeps any value of a known state
                    // for GetRenderState; an unknown state: E_INVALIDARG
                    if rs_default(ver.into(), st).is_none() {
                        out.push(format!("rs set {st} hr {E_INVALIDARG:08x}"));
                    } else {
                        hit(clause::D3D7_STATES);
                        self.rs.insert(st, v);
                    }
                } else {
                    match self.state(st) {
                        Some(v) => out.push(format!("rs {st} = {v:08x}")),
                        None => out.push(format!("rs {st} hr {E_INVALIDARG:08x}")),
                    }
                }
            }
            "d3dclear" => {
                let c = n32(op, 1)?;
                let mut i = 2;
                let mut z = None;
                if t.get(i).is_some_and(|v| v == "z") {
                    z = Some(n(op, i + 1)? as f32 / 1000.0);
                    i += 2;
                }
                let r = if t.len() == i + 4 {
                    Some((n(op, i)?, n(op, i + 1)?, n(op, i + 2)?, n(op, i + 3)?))
                } else if t.len() == i {
                    None
                } else {
                    return Err(Fail::Bad);
                };
                let (Some(ver), Some(d)) = (ver, self.dev.as_mut()) else {
                    out.push("nodev".into());
                    return Ok(true);
                };
                clear(ver, d, c, z, r, out)?;
            }
            "d3dline" => {
                // (a line list of one line: the rasterizers' lines are not worked out, as
                // wireframe triangles, D3D7.STATES.OTHER)
                n32(op, 1)?;
                for k in 2..6 {
                    n(op, k)?;
                }
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                if ver.is_none() || self.dev.is_none() {
                    out.push("nodev".into());
                    return Ok(true);
                }
                return Err(Fail::Unmodeled(open::D3D7_STATES_OTHER));
            }
            "d3dtri" => {
                let c = colors(t.get(1).ok_or(Fail::Bad)?)?;
                let mut v = [(0i64, 0i64); 3];
                for (k, p) in v.iter_mut().enumerate() {
                    *p = (n(op, 2 + 2 * k)?, n(op, 3 + 2 * k)?);
                }
                let mut z = 0.5f32;
                let mut spec = [0u32; 3];
                let mut k = 8;
                while k < t.len() {
                    match (t[k].as_str(), t.get(k + 1)) {
                        ("z", Some(_)) => z = n(op, k + 1)? as f32 / 1000.0,
                        ("spec", Some(sp)) => spec = colors(sp)?,
                        _ => return Err(Fail::Bad),
                    }
                    k += 2;
                }
                if ver.is_none() {
                    out.push("nodev".into());
                    return Ok(true);
                }
                let st = States::read(self)?;
                let d = self.dev.as_mut().ok_or(Fail::Bad)?;
                draw_tri(d, &st, &Tri { v, c, spec, z })?;
            }
            "d3dfill" | "d3ddump" | "d3dzdump" => {
                let seed = if t[0] == "d3dfill" { Some(n32(op, 1)?) } else { None };
                if t.len() != if seed.is_some() { 2 } else { 1 } {
                    return Err(Fail::Bad);
                }
                let Some(d) = self.dev.as_mut() else {
                    out.push("nodev".into());
                    return Ok(true);
                };
                let rt = &mut d.rt;
                let zdump = t[0] == "d3dzdump";
                let b = match (zdump, rt.depth) {
                    (true, None) => {
                        out.push("nodev".into());
                        return Ok(true);
                    }
                    (true, Some(dp)) => dp.bytes(),
                    (false, _) => rt.fmt.bytes(),
                };
                hit(clause::D3D7_LOCK);
                if let Some(seed) = seed {
                    let mut g = crate::rng::XorShift32::new(seed);
                    for p in rt.px.iter_mut() {
                        let mut v = 0;
                        for k in 0..b {
                            v |= (g.next() & 0xff) << (8 * k);
                        }
                        *p = v;
                    }
                    return Ok(true);
                }
                let data = if zdump { &rt.z } else { &rt.px };
                out.push(format!("dd {} {} {} {}", if zdump { "z" } else { "rt" }, rt.w, rt.h, b * 8));
                for row in data.chunks(usize::try_from(rt.w).map_err(|_| Fail::Bad)?) {
                    out.push(row.iter().map(|v| v.to_le_bytes()[..b].iter().map(|x| format!("{x:02x}")).collect::<String>()).collect());
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}

/// Clear of the target (and z-buffer) in rectangle r (None: all)
fn clear(ver: DrawVer, d: &mut Device, c: u32, z: Option<f32>, r: Option<(i64, i64, i64, i64)>, out: &mut Vec<String>) -> Res<()> {
    let rt = &mut d.rt;
    let zv = match (z, rt.depth) {
        (Some(_), None) => {
            // MEASURED (d3d7dev clearnoz-*: whole and in a rectangle, every software drawer):
            // D3DCLEAR_ZBUFFER without a z-buffer fails and clears nothing in versions 3 and 7;
            // IDirect3DViewport2::Clear clears the target alone
            hit(clause::D3D7_CLEAR_NOZ);
            match ver {
                DrawVer::V3 | DrawVer::V7 => {
                    out.push(format!("clear hr {D3DERR_ZBUFFER_NOTPRESENT:08x}"));
                    return Ok(());
                }
                DrawVer::V2 => None,
            }
        }
        (None, _) => None,
        // MEASURED (d3d7dev zrange): IDirect3DViewport2::Clear takes no z, it clears to the
        // largest value
        (Some(_), Some(dp)) if ver == DrawVer::V2 => {
            hit(clause::D3D7_CLEAR_Z2);
            Some(dp.max())
        }
        (Some(z), Some(dp)) => {
            hit(clause::D3D7_CLEAR_Z);
            Some(clear_z(dp, z))
        }
    };
    // MEASURED (clearrects golden): the rectangle clipped to the target; one turned round or
    // empty clears nothing
    hit(clause::D3D7_CLEAR);
    // MEASURED (d3d7dev color-*): software targets truncate and drop alpha; the HAL rounds, and
    // keeps the alpha bits Clear's color has (32 bits: 8, 555: the top one); ramp clears to 0
    let v = match d.family {
        Family::Soft { .. } => pixel(rt.fmt, c),
        Family::Hw { .. } => {
            hit(clause::D3D7_HW_COLOR);
            pixel_hw(rt.fmt, c)
                | match rt.fmt {
                    Fmt::X32 => c & 0xff00_0000,
                    Fmt::R555 => (c >> 31) << 15,
                    Fmt::P8 | Fmt::R565 | Fmt::R24 => 0,
                }
        }
        Family::Ramp => {
            hit(clause::D3D7_RAMP);
            0
        }
    };
    let (x0, y0, x1, y1) = r.unwrap_or((0, 0, rt.w, rt.h));
    for y in y0.max(0)..y1.min(rt.h) {
        for x in x0.max(0)..x1.min(rt.w) {
            let k = usize::try_from(y * rt.w + x).map_err(|_| Fail::Bad)?;
            rt.px[k] = v;
            if let Some(zv) = zv {
                rt.z[k] = zv;
            }
        }
    }
    Ok(())
}

/// how a triangle's z is written: by the software rasterizers over each pixel's old z, by the
/// HAL as one value
#[derive(Clone, Copy)]
enum Zw {
    Soft(Depth),
    Hw(u32),
}

/// one D3DTLVERTEX triangle: x, y in 16ths of a pixel, diffuse and specular ARGB, z
struct Tri {
    v: [(i64, i64); 3],
    c: [u32; 3],
    spec: [u32; 3],
    z: f32,
}

/// the render states a triangle is drawn with, parsed
struct States {
    cull: Cull,
    fill: Fill,
    flat: bool,
    dither: bool,
    zenable: bool,
    zwrite: bool,
    zfunc: Cmp,
    alphatest: Option<(Cmp, u32)>,
    blend: Option<(Factor, Factor)>,
    fog: Option<u32>,
    specular: bool,
}

impl States {
    fn read(d: &D3d7) -> Res<States> {
        let st = |s: Rs| d.rs(s);
        let other = Fail::Unmodeled(open::D3D7_STATES_OTHER);
        let cull = match st(Rs::CullMode) {
            1 => Cull::None,
            2 => Cull::Cw,
            3 => Cull::Ccw,
            _ => return Err(other),
        };
        let fill = match st(Rs::FillMode) {
            1 => Fill::Point,
            2 => Fill::Wireframe,
            3 => Fill::Solid,
            _ => return Err(other),
        };
        let flat = match st(Rs::ShadeMode) {
            1 => true,
            2 => false,
            _ => return Err(other),
        };
        let zfunc = Cmp::parse(st(Rs::ZFunc)).ok_or(other.clone())?;
        let alphatest = match st(Rs::AlphaTestEnable) {
            0 => None,
            _ => Some((Cmp::parse(st(Rs::AlphaFunc)).ok_or(other.clone())?, st(Rs::AlphaRef) & 255)),
        };
        let blend = match st(Rs::AlphaBlendEnable) {
            0 => None,
            _ => {
                let f = |s| Factor::parse(st(s)).ok_or(Fail::Unmodeled(open::D3D7_STATES_OTHER));
                Some((f(Rs::SrcBlend)?, f(Rs::DestBlend)?))
            }
        };
        Ok(States {
            cull,
            fill,
            flat,
            dither: st(Rs::DitherEnable) != 0,
            zenable: st(Rs::ZEnable) != 0,
            zwrite: st(Rs::ZWriteEnable) != 0,
            zfunc,
            alphatest,
            blend,
            fog: (st(Rs::FogEnable) != 0).then(|| st(Rs::FogColor)),
            specular: st(Rs::SpecularEnable) != 0,
        })
    }
}

/// which pixels a triangle covers
fn coverage(family: Family, v: [(i64, i64); 3], w: i64, h: i64, fmt: Fmt) -> Res<Vec<bool>> {
    let area = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
    let (a, b, c) = if area > 0 { (v[0], v[1], v[2]) } else { (v[0], v[2], v[1]) };
    let edges = [(a, b), (b, c), (c, a)];
    // (a vertex on the right side itself crosses the target: d3d7rast rand-17)
    let inside = v.iter().all(|&(x, y)| (0..w * 16).contains(&x) && (0..=h * 16).contains(&y));
    let mut cover = Vec::with_capacity(usize::try_from(w * h).map_err(|_| Fail::Bad)?);
    for py in 0..h {
        for px in 0..w {
            let (sx, sy) = (px * 16, py * 16);
            let mut ins = true;
            for &((ax, ay), (bx, by)) in &edges {
                let (dx, dy) = (bx - ax, by - ay);
                let e = dx * (sy - ay) - dy * (sx - ax);
                match family {
                    // MEASURED (d3d7rast dev-*: 44 of 44 dumps, clipped ones too): the top-left
                    // rule at integer sample points (y down, the edges clockwise on screen; a
                    // pixel on an edge is drawn when the edge runs up the screen or is flat with
                    // the inside below)
                    Family::Hw { .. } => {
                        let topleft = dy < 0 || (dy == 0 && dx > 0);
                        if e < 0 || (e == 0 && !topleft) {
                            ins = false;
                        }
                    }
                    // (the exact inside, for a triangle crossing the target; one inside is
                    // walked)
                    Family::Soft { .. } | Family::Ramp => {
                        if e < 0 {
                            ins = false;
                        }
                    }
                }
            }
            cover.push(ins);
        }
    }
    match family {
        Family::Hw { .. } => {
            hit(clause::D3D7_TRI_HW);
            Ok(cover)
        }
        Family::Soft { .. } | Family::Ramp => {
            if !inside && cover.iter().all(|&c| c) {
                hit(clause::D3D7_TRI_COVERALL);
                return Ok(cover);
            }
            match (inside, family) {
                (true, _) => {
                    hit(clause::D3D7_TRI);
                    walk(v, w, h, Spans::Spill(fmt))
                }
                // (the ramp device drew one more pixel by a clipped edge, d3d7rast dev-2-ramp)
                (false, Family::Ramp) => Err(Fail::Unmodeled(open::D3D7_TRI_CLIP)),
                (false, Family::Soft { .. }) => {
                    hit(clause::D3D7_TRI_GUARD);
                    walk(v, w, h, Spans::Clamp)
                }
                // (the HAL's coverage is the arm above)
                (false, Family::Hw { .. }) => Err(Fail::Bad),
            }
        }
    }
}

/// f32's "ceiling" of the software rasterizers: x + (1 − 2^-23) in single precision (rounded to
/// nearest), then its floor: the next integer above x, but x itself for an integer x below 2
fn ceil_f32(num: i64, den: i64) -> i64 {
    // (num / den: below 2^24 · den here, a vertex within the target)
    let x = (num as f64 / den as f64) as f32;
    (x + (1.0 - f32::EPSILON)).floor() as i64
}

/// how an edge settles an x exactly on a pixel on a row past its first (D3D7.TRI.TIE)
#[derive(Clone, Copy)]
enum Tie {
    /// a short edge: by where its x lies in single precision, the first row's x (top x + (row −
    /// top y) · slope, the slope (bottom x − top x) / (bottom y − top y), each step rounded to
    /// f32) and the f32 slope times the rows after it taken exactly: a pixel more when that is
    /// past the pixel
    F32,
    /// the long edge on the right: never a pixel more
    Exact,
    /// the long edge on the left: not worked out (D3D7.TRI.EDGE)
    Open,
}

/// a finite f32 times 2^64, when that is a whole number that fits
fn f32_scaled(v: f32) -> Option<i128> {
    let bits = v.to_bits();
    let exp = i32::try_from(bits >> 23 & 0xff).ok()?;
    let man = i128::from(bits & 0x7f_ffff);
    if exp == 0 && man == 0 {
        return Some(0);
    }
    if exp == 0 || exp == 255 {
        return None;
    }
    let m = (man | 1 << 23) * if bits >> 31 == 1 { -1 } else { 1 };
    // (v = m · 2^(exp − 150))
    let sh = exp - 150 + 64;
    match u32::try_from(sh) {
        Ok(up) if up <= 96 => Some(m << up),
        Ok(_) => None,
        Err(_) => {
            let down = u32::try_from(-sh).ok().filter(|d| *d < 120)?;
            (m & ((1i128 << down) - 1) == 0).then(|| m >> down)
        }
    }
}

/// a 16ths coordinate in f32 pixels (exact)
fn px_f32(c: i64) -> Option<f32> {
    i16::try_from(c).ok().map(|c| f32::from(c) / 16.0)
}

/// whether the f32 walk of edge t..b lies past the pixel on the k-th row after its first row
/// `first`, where the exact x is a whole pixel (Tie::F32); None where the arithmetic leaves i128
pub fn f32_past(t: (i64, i64), b: (i64, i64), first: i64, k: i64) -> Option<bool> {
    let (tx, ty, bx, by) = (px_f32(t.0)?, px_f32(t.1)?, px_f32(b.0)?, px_f32(b.1)?);
    let slope = (bx - tx) / (by - ty);
    let x0 = tx + (f32::from(i16::try_from(first).ok()?) - ty) * slope;
    let (dxn, dyn_) = (i128::from(b.0 - t.0), i128::from(b.1 - t.1));
    let den = 16 * dyn_;
    let num = i128::from(t.0) * dyn_ + i128::from(first * 16 - t.1) * dxn;
    let one = 1i128 << 64;
    // ((x0 − num / den) + k (slope − 16 dxn / den)) · den · 2^64
    let e0 = f32_scaled(x0)?.checked_mul(den)?.checked_sub(num.checked_mul(one)?)?;
    let ed = f32_scaled(slope)?.checked_mul(den)?.checked_sub((16 * dxn).checked_mul(one)?)?;
    Some(e0.checked_add(ed.checked_mul(i128::from(k))?)? > 0)
}

/// one edge's walk, from top t to bottom b (in 16ths, t above b): the rows it spans, each with
/// its x as the next pixel at or right of the edge, and whether that row's x lies exactly on a
/// pixel past the first row where the edge's Tie leaves it open
struct EdgeWalk {
    first: i64,
    xs: Vec<(i64, bool)>,
}

impl EdgeWalk {
    fn new(t: (i64, i64), b: (i64, i64), tie: Tie) -> EdgeWalk {
        let (dxn, dyn_) = (b.0 - t.0, b.1 - t.1);
        let first = ceil_f32(t.1, 16);
        let last = b.1.div_euclid(16);
        // x at row y = (t.x dy + (16 y − t.y) dx) / (16 dy)
        let den = 16 * dyn_;
        let num = t.0 * dyn_ + (first * 16 - t.1) * dxn;
        let mut xi = ceil_f32(num, den);
        // the error to the next pixel, in 1/den: with xi it walks the edge; an x on a pixel
        // on the first row (x + 1 there, but no error) moves the whole edge one pixel right
        let mut err = (-num).rem_euclid(den);
        let (fl, fr) = ((16 * dxn).div_euclid(den), (16 * dxn).rem_euclid(den));
        let mut xs = Vec::new();
        for y in first..=last {
            let mut open = false;
            if y > first {
                xi += fl;
                err -= fr;
                if err < 0 {
                    xi += 1;
                    err += den;
                }
                if err == 0 {
                    // (past the pixel: the pixel after it, its error a whole den)
                    match tie {
                        Tie::F32 => match f32_past(t, b, first, y - first) {
                            Some(true) => {
                                hit(clause::D3D7_TRI_TIE);
                                xi += 1;
                                err += den;
                            }
                            Some(false) => hit(clause::D3D7_TRI_TIE),
                            None => open = true,
                        },
                        Tie::Exact => hit(clause::D3D7_TRI_TIE),
                        Tie::Open => open = true,
                    }
                }
            }
            xs.push((xi, open));
        }
        EdgeWalk { first, xs }
    }
    fn at(&self, y: i64) -> Option<(i64, bool)> {
        usize::try_from(y - self.first).ok().and_then(|k| self.xs.get(k).copied())
    }
}

/// how a walked row meets the target's sides
#[derive(Clone, Copy)]
enum Spans {
    /// a triangle inside the target: past the right side on in memory, into the row's padding
    /// (a system-memory surface's pitch, DDRAW.PITCH) and then the next row
    Spill(Fmt),
    /// a triangle crossing the target (its guard band): cut at the sides
    Clamp,
}

/// the pixels the RGB, MMX and ramp rasterizers draw of a triangle inside the target: MEASURED
/// (d3d7rast, d3d7edge, d3d7, d3d7shade: 2400 of 2400 dumps with no exact tie on a later row)
/// each edge walked down from its first row (the f32 ceiling of its top: rows below the top,
/// and the top's own row when that is 0 or 1) to the row of its bottom (inclusive), its x the
/// f32 ceiling of the edge's x on the first row, then stepped by a whole part and an exact
/// error; the long edge (top to bottom vertex) against the two short ones, a row drawn from the
/// left edge's x to the right edge's x less 1 (nothing when they cross); pixels past the row's
/// end go on into the next row, as in memory. An x exactly on a pixel past an edge's first row
/// goes by the edge's Tie (D3D7.TRI.TIE; the long edge on the left: D3D7.TRI.EDGE)
fn walk(v: [(i64, i64); 3], w: i64, h: i64, spans: Spans) -> Res<Vec<bool>> {
    let mut s = v;
    s.sort_by_key(|p| p.1);
    let [t, m, b] = s;
    hit(clause::D3D7_TRI_WALK);
    let long_left = (b.0 - t.0) * (m.1 - t.1) - (b.1 - t.1) * (m.0 - t.0) < 0;
    let long = EdgeWalk::new(t, b, if long_left { Tie::Open } else { Tie::Exact });
    let upper = (t.1 != m.1).then(|| EdgeWalk::new(t, m, Tie::F32));
    let lower = (m.1 != b.1).then(|| EdgeWalk::new(m, b, Tie::F32));
    let n = usize::try_from(w * h).map_err(|_| Fail::Bad)?;
    let mut cover = vec![false; n];
    for y in 0..h {
        let short = lower.as_ref().and_then(|e| e.at(y)).or_else(|| upper.as_ref().and_then(|e| e.at(y)));
        let (Some(l), Some(r)) = (long.at(y), short) else { continue };
        let ((xl, tl), (xr, tr)) = if long_left { (l, r) } else { (r, l) };
        if tl || tr {
            return Err(Fail::Unmodeled(open::D3D7_TRI_EDGE));
        }
        match spans {
            Spans::Spill(fmt) => {
                let b = i64::try_from(fmt.bytes()).map_err(|_| Fail::Bad)?;
                let p = crate::model::ddraw::pitch(w, 8 * u32::try_from(fmt.bytes()).map_err(|_| Fail::Bad)?);
                for x in xl..xr {
                    let a = y * p + x * b;
                    // (a pixel past the surface's last byte writes the memory after it:
                    // d3d7edge-32 died in the case after whole-rgb-short, whose triangle 36 did;
                    // one across a row's end is not measured)
                    if a < 0 || a + b > h * p || (a % p) % b != 0 || (a % p < w * b && a % p + b > w * b) {
                        return Err(Fail::Unmodeled(open::D3D7_TRI_CLIP));
                    }
                    let (row, col) = (a / p, a % p / b);
                    if col < w {
                        if let Some(c) = usize::try_from(row * w + col).ok().and_then(|k| cover.get_mut(k)) {
                            *c = true;
                        }
                    }
                }
            }
            Spans::Clamp => {
                for x in xl.max(0)..xr.min(w) {
                    if let Some(c) = usize::try_from(y * w + x).ok().and_then(|k| cover.get_mut(k)) {
                        *c = true;
                    }
                }
            }
        }
    }
    Ok(cover)
}

/// the pixels of D3DFILL_POINT: MEASURED (d3d7dev fillmode-*-1-*: 48 x 2 random triangles,
/// LASTPIXEL on and off, every drawer) each vertex at its position rounded, x.5 up on the
/// software rasterizers and down on the HAL; the ramp device draws no points
fn points(family: Family, v: [(i64, i64); 3], w: i64, h: i64) -> Res<Vec<bool>> {
    hit(clause::D3D7_POINT);
    let mut cover = vec![false; usize::try_from(w * h).map_err(|_| Fail::Bad)?];
    let round = |c: i64| match family {
        Family::Soft { .. } | Family::Ramp => (c + 8).div_euclid(16),
        Family::Hw { .. } => (c + 7).div_euclid(16),
    };
    if family == Family::Ramp {
        return Ok(cover);
    }
    for (x, y) in v.map(|(x, y)| (round(x), round(y))) {
        if (0..w).contains(&x) && (0..h).contains(&y) {
            cover[usize::try_from(y * w + x).map_err(|_| Fail::Bad)?] = true;
        }
    }
    Ok(cover)
}

/// one channel (at bit `sh` of ARGB) of the color at pixel (px, py): flat, or Gouraud
fn shade(family: Family, flat: bool, tri: &Tri, sh: u32, px: i64, py: i64) -> Res<u32> {
    let ch = |c: u32| c >> sh & 255;
    let c = tri.c;
    if flat || (ch(c[0]) == ch(c[1]) && ch(c[0]) == ch(c[2])) {
        // D3DSHADE_FLAT: the first vertex's color
        if flat {
            hit(clause::D3D7_FLAT);
        }
        return Ok(ch(c[0]));
    }
    let p = |i: usize| (tri.v[i].0 as f64 / 16.0, tri.v[i].1 as f64 / 16.0);
    let (a, b, cc) = (p(0), p(1), p(2));
    let (x, y) = (px as f64, py as f64);
    let den = (b.0 - a.0) * (cc.1 - a.1) - (cc.0 - a.0) * (b.1 - a.1);
    let l1 = ((x - a.0) * (cc.1 - a.1) - (cc.0 - a.0) * (y - a.1)) / den;
    let l2 = ((b.0 - a.0) * (y - a.1) - (x - a.0) * (b.1 - a.1)) / den;
    let f = |c: u32| f64::from(ch(c));
    let val = f(c[0]) + l1 * (f(c[1]) - f(c[0])) + l2 * (f(c[2]) - f(c[0]));
    let fr = val - val.floor();
    // the change of the value over one pixel across and down
    let gx = ((f(c[1]) - f(c[0])) * (cc.1 - a.1) - (f(c[2]) - f(c[0])) * (b.1 - a.1)) / den;
    let gy = ((f(c[2]) - f(c[0])) * (b.0 - a.0) - (f(c[1]) - f(c[0])) * (cc.0 - a.0)) / den;
    let margin = 1.0 / 32.0 + (gx.abs() + gy.abs()) / 128.0;
    match family {
        // MEASURED (d3d7shade gouraud-*): the software rasterizers take the floor of the exact
        // value; near a whole number (nearer as the color changes faster across the pixels)
        // their fixed point decides (D3D7.GOURAUD)
        Family::Soft { .. } | Family::Ramp => {
            if fr < margin || fr > 1.0 - margin {
                return Err(Fail::Unmodeled(open::D3D7_GOURAUD));
            }
            hit(clause::D3D7_GOURAUD);
            Ok(val.floor().clamp(0.0, 255.0) as u32)
        }
        // MEASURED: the HAL rounds; near a half its float arithmetic decides
        Family::Hw { .. } => {
            if (fr - 0.5).abs() < 1.0 / 32.0 {
                return Err(Fail::Unmodeled(open::D3D7_GOURAUD));
            }
            hit(clause::D3D7_GOURAUD);
            Ok(val.round().clamp(0.0, 255.0) as u32)
        }
    }
}

/// the software blend factors (source, destination) of one channel in 256ths (ONE = 256): s, d,
/// a the 8-bit source, destination and source alpha
pub fn factors_soft(f: (Factor, Factor), s: u32, d: u32, a: u32) -> (u32, u32) {
    let one = |x: Factor| -> u32 {
        match x {
            Factor::Zero => 0,
            Factor::One => 256,
            Factor::SrcColor => s,
            Factor::InvSrcColor => 255 - s,
            Factor::SrcAlpha => a,
            Factor::InvSrcAlpha => 255 - a,
            // (the target has no alpha: it reads as 255)
            Factor::DestAlpha => 255,
            Factor::InvDestAlpha => 0,
            Factor::DestColor => d,
            Factor::InvDestColor => 255 - d,
            // min(a, 1 - dest alpha) = 0
            Factor::SrcAlphaSat => 0,
            // (as the destination factor: 0; as the source one they set both, below)
            Factor::BothSrcAlpha | Factor::BothInvSrcAlpha => 0,
        }
    };
    match f.0 {
        Factor::BothSrcAlpha => (a, 255 - a),
        Factor::BothInvSrcAlpha => (255 - a, a),
        Factor::Zero
        | Factor::One
        | Factor::SrcColor
        | Factor::InvSrcColor
        | Factor::SrcAlpha
        | Factor::InvSrcAlpha
        | Factor::DestAlpha
        | Factor::InvDestAlpha
        | Factor::DestColor
        | Factor::InvDestColor
        | Factor::SrcAlphaSat => (one(f.0), one(f.1)),
    }
}

/// the HAL's blend factors in [0, 1] (the destination's alpha reads as 1)
pub fn factors_hw(f: (Factor, Factor), s: f64, d: f64, a: f64) -> (f64, f64) {
    let one = |x: Factor| -> f64 {
        match x {
            Factor::Zero => 0.0,
            Factor::One => 1.0,
            Factor::SrcColor => s,
            Factor::InvSrcColor => 1.0 - s,
            Factor::SrcAlpha => a,
            Factor::InvSrcAlpha => 1.0 - a,
            Factor::DestAlpha => 1.0,
            Factor::InvDestAlpha => 0.0,
            Factor::DestColor => d,
            Factor::InvDestColor => 1.0 - d,
            Factor::SrcAlphaSat => 0.0,
            Factor::BothSrcAlpha => a,
            Factor::BothInvSrcAlpha => 1.0 - a,
        }
    };
    // either factor BOTHSRCALPHA: (a, 1 - a); else either BOTHINVSRCALPHA: (1 - a, a)
    if f.0 == Factor::BothSrcAlpha || f.1 == Factor::BothSrcAlpha {
        (a, 1.0 - a)
    } else if f.0 == Factor::BothInvSrcAlpha || f.1 == Factor::BothInvSrcAlpha {
        (1.0 - a, a)
    } else {
        (one(f.0), one(f.1))
    }
}

fn draw_tri(dev: &mut Device, st: &States, tri: &Tri) -> Res<()> {
    let family = dev.family;
    let rt = &mut dev.rt;
    let v = tri.v;
    let area = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
    // MEASURED (cull golden): D3DCULL_CCW (the default) drops triangles of negative area in
    // screen coordinates (x right, y down; area of v0 v1 v2), CW positive ones
    hit(clause::D3D7_CULL);
    let culled = match st.cull {
        Cull::None => false,
        Cull::Ccw => area < 0,
        Cull::Cw => area > 0,
    };
    match st.fill {
        Fill::Solid => {}
        // (points of a culled or flat triangle, and points of several colors, not measured)
        Fill::Point if st.cull == Cull::None && area != 0 && tri.c[0] == tri.c[1] && tri.c[0] == tri.c[2] && tri.spec[0] == tri.spec[1] && tri.spec[0] == tri.spec[2] => {}
        Fill::Point | Fill::Wireframe => return Err(Fail::Unmodeled(open::D3D7_STATES_OTHER)),
    }
    if area == 0 || culled {
        return Ok(());
    }
    // MEASURED (d3d7dev zrange: z from -1 to 2 on every device): a triangle with z below 0 or
    // above 1 is not drawn, nor one at 1 except by the T&L HAL
    let z = tri.z;
    hit(clause::D3D7_ZRANGE);
    if !(0.0..=1.0).contains(&z) || (z == 1.0 && family != (Family::Hw { tnl: true })) {
        return Ok(());
    }
    let (w, h) = (rt.w, rt.h);
    let cover = match st.fill {
        Fill::Point => points(family, v, w, h)?,
        Fill::Solid | Fill::Wireframe => coverage(family, v, w, h, rt.fmt)?,
    };
    let zv = match (st.zenable, rt.depth, family) {
        (false, _, _) => None,
        (true, None, _) => return Err(Fail::Unmodeled(open::D3D7_STATES_OTHER)),
        (true, Some(dp), Family::Soft { .. } | Family::Ramp) => Some(Zw::Soft(dp)),
        (true, Some(Depth::Z16), Family::Hw { .. }) => Some(Zw::Hw(tri_z_hw(z))),
        (true, Some(Depth::Z24 | Depth::Z32), Family::Hw { .. }) => return Err(Fail::Bad),
    };
    let fmt = rt.fmt;
    let bits = fmt.bits();
    let a = match family {
        Family::Ramp => 0,
        Family::Soft { .. } | Family::Hw { .. } => tri.c[0] >> 24,
    };
    for py in 0..h {
        for px in 0..w {
            let k = usize::try_from(py * w + px).map_err(|_| Fail::Bad)?;
            if !cover[k] {
                continue;
            }
            // MEASURED (d3d7shade alphatest-*: each function and reference against 8 alphas,
            // every device): the documented compare of the vertex alpha (ramp: 0) with ALPHAREF
            if let Some((f, aref)) = st.alphatest {
                hit(clause::D3D7_ALPHATEST);
                if !f.passes(a, aref) {
                    continue;
                }
            }
            if let Some(zw) = zv {
                let zv = match zw {
                    Zw::Soft(dp) => tri_z_write(dp, z, rt.z[k]),
                    Zw::Hw(v) => v,
                };
                // MEASURED (d3d7shade zfunc-*: every function, z writes on and off, every
                // device): the documented compares
                hit(clause::D3D7_ZTEST);
                hit(if st.zfunc == Cmp::LessEqual { clause::D3D7_ZFUNC } else { clause::D3D7_ZFUNC_ALL });
                if !st.zfunc.passes(zv, rt.z[k]) {
                    continue;
                }
                if st.zwrite {
                    rt.z[k] = zv;
                }
            }
            let src = match family {
                // MEASURED (d3d7dev color-*, d3d7shade *-2-ramp): the ramp device draws one
                // color, 0x800080 with alpha 0, whatever the vertices say; no specular; fog acts as if every vertex's fog were 0 (the fog color, in 256ths)
                Family::Ramp => {
                    hit(clause::D3D7_RAMP);
                    match st.fog {
                        None => [0x80, 0, 0x80],
                        Some(fc) => [16u32, 8, 0].map(|sh| ((fc >> sh & 255) * 255) >> 8),
                    }
                }
                Family::Soft { .. } | Family::Hw { .. } => {
                    let mut src = [0u32; 3];
                    for (i, sh) in [16u32, 8, 0].into_iter().enumerate() {
                        let mut c = shade(family, st.flat, tri, sh, px, py)?;
                        // MEASURED (d3d7shade specular-*): the specular color added, saturating
                        if st.specular {
                            hit(clause::D3D7_SPECULAR);
                            c = (c + (tri.spec[0] >> sh & 255)).min(255);
                        }
                        // MEASURED (d3d7shade fog-*: f every 5 of 0..255): vertex fog by the
                        // specular alpha f toward FOGCOLOR, software in 256ths, the HAL in
                        // floats rounded
                        if let Some(fc) = st.fog {
                            hit(clause::D3D7_FOG);
                            let (f, fv) = (tri.spec[0] >> 24, fc >> sh & 255);
                            c = match family {
                                Family::Soft { .. } | Family::Ramp => (c * f + fv * (255 - f)) >> 8,
                                Family::Hw { .. } => ((f64::from(c) * f64::from(f) + f64::from(fv) * f64::from(255 - f)) / 255.0).round() as u32,
                            };
                        }
                        src[i] = c;
                    }
                    src
                }
            };
            let dst = fmt.channels(rt.px[k]);
            if st.blend.is_some() {
                hit(clause::D3D7_BLEND);
                hit(clause::D3D7_BLEND_OTHER);
                if bits != [8, 8, 8] {
                    hit(clause::D3D7_BLEND16);
                }
            }
            let q: [u32; 3] = match (family, st.blend) {
                // MEASURED (d3d7shade blend-*: 169 factor pairs x 8 pixels x 3 channels,
                // versions 2, 3, 7, RGB, MMX and ramp; blendalpha-*: every 3rd alpha):
                // min(255, (s·Fs + d·Fd) >> 8), on 16 bits with the destination widened by a
                // shift and the result truncated
                (Family::Soft { .. } | Family::Ramp, Some(f)) => [0, 1, 2].map(|i| {
                    let (fs, fd) = factors_soft(f, src[i], dst[i], a);
                    ((src[i] * fs + dst[i] * fd) >> 8).min(255) >> (8 - bits[i])
                }),
                // MEASURED: software truncates, or dithers (d3d7shade dither-*, gouraud16-*)
                (Family::Soft { .. } | Family::Ramp, None) => {
                    let dith = st.dither && bits != [8, 8, 8] && family != Family::Ramp;
                    if dith {
                        hit(clause::D3D7_DITHER);
                    }
                    [0, 1, 2].map(|i| if dith { dither(src[i], bits[i], px, py) } else { src[i] >> (8 - bits[i]) })
                }
                // MEASURED (all 169 pairs on 32 bits; 565 and 555): floor(max · clamp(s·Fs +
                // d·Fd) + 0.5) in floats, the destination its exact fraction q / max
                (Family::Hw { .. }, Some(f)) => [0, 1, 2].map(|i| {
                    let max = f64::from((1u32 << bits[i]) - 1);
                    let dq = rt.px[k] >> fmt.shifts()[i] & ((1 << bits[i]) - 1);
                    let (s, d, al) = (f64::from(src[i]) / 255.0, f64::from(dq) / max, f64::from(a) / 255.0);
                    let (fs, fd) = factors_hw(f, s, d, al);
                    (max * (s * fs + d * fd).clamp(0.0, 1.0) + 0.5).floor() as u32
                }),
                // MEASURED (d3d7dev color-*): the HAL rounds
                (Family::Hw { .. }, None) => {
                    hit(clause::D3D7_HW_COLOR);
                    [0, 1, 2].map(|i| round_to(src[i], bits[i]))
                }
            };
            rt.px[k] = fmt.pack(q);
        }
    }
    Ok(())
}
