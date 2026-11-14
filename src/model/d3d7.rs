//! Direct3D 1, 2, 3 and 7 (immediate mode through DirectDraw) on system-memory render
//! targets: the device list, which devices a version makes, render states, Clear, and
//! pretransformed triangles drawn by the RGB rasterizer.
//!
//! Direct3D 7 and older are no longer documented on Microsoft Learn (only DirectDraw is): the
//! clauses come from the DirectX 7 SDK's rules as they still hold (spec/d3d7.txt) and from
//! Windows' answers (golden d3d7-32, -64). The device list and the render state defaults are
//! measured tables (src/model/d3d7.txt).

use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

const TABLES: &str = include_str!("d3d7.txt");

pub const E_NOINTERFACE: u32 = 0x8000_4002;
pub const E_INVALIDARG: u32 = 0x8007_0057;
pub const DDERR_UNSUPPORTED: u32 = 0x8000_4001;
pub const DDERR_INVALIDPIXELFORMAT: u32 = 0x8876_0091;
pub const DDERR_NOPALETTEATTACHED: u32 = 0x8876_023c;

/// the render target: pixels (bytes per pixel 2, 3 or 4) and the z-buffer's values
#[derive(Clone, Default)]
struct Target {
    w: i64,
    h: i64,
    fmt: &'static str,
    px: Vec<u32>,
    zbits: u32,
    z: Vec<u32>,
}

#[derive(Default)]
pub struct D3d7 {
    ver: u32,
    dev: bool,
    rt: Option<Target>,
    rs: std::collections::BTreeMap<i64, u32>,
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}

/// a render state's default (None: GetRenderState fails with E_INVALIDARG)
pub fn rs_default(ver: u32, st: i64) -> Option<u32> {
    let key = format!("rs {ver} {st} ");
    let l = TABLES.lines().find(|l| l.starts_with(&key))?;
    let v = &l[key.len()..];
    v.strip_prefix("= ").map(|h| u32::from_str_radix(h, 16).unwrap())
}

fn bytes(fmt: &str) -> usize {
    match fmt {
        "565" | "555" => 2,
        "24" => 3,
        _ => 4,
    }
}

/// ARGB to the target's pixel: MEASURED (clear golden, every value of each channel, versions
/// 2, 3, 7) each channel's top bits, 0 elsewhere (no alpha in the target)
pub fn pixel(fmt: &str, c: u32) -> u32 {
    let (r, g, b) = (c >> 16 & 255, c >> 8 & 255, c & 255);
    match fmt {
        "565" => (r >> 3) << 11 | (g >> 2) << 5 | b >> 3,
        "555" => (r >> 3) << 10 | (g >> 3) << 5 | b >> 3,
        _ => c & 0xff_ffff,
    }
}

/// a pixel's channels as 8 bits each (r, g, b), the low bits 0
fn channels(fmt: &str, p: u32) -> [u32; 3] {
    match fmt {
        "565" => [(p >> 11 & 31) << 3, (p >> 5 & 63) << 2, (p & 31) << 3],
        "555" => [(p >> 10 & 31) << 3, (p >> 5 & 31) << 3, (p & 31) << 3],
        _ => [p >> 16 & 255, p >> 8 & 255, p & 255],
    }
}

/// Clear's z value: MEASURED (clearz golden, z = 0, 1/1000, ..., 1 into 16 and 32 bits) the
/// float z times the largest value, in float, rounded to nearest, at most the largest value
pub fn clear_z(bits: u32, z: f32) -> u32 {
    let max = if bits == 32 { u32::MAX } else { (1u32 << bits) - 1 };
    let v = (z * max as f32) as f64 + 0.5;
    if v >= max as f64 { max } else { v as u32 }
}

/// a triangle's z value (constant over it): MEASURED (zsweep golden: 144 z values, every 7/1000
/// from 0 to 1, into 16 and 32 bits, all exact; zcover: 0.3, 0.5, 0.6) the largest integer below
/// z * 2^16 (16 bits) or z * 2^24 (32 bits: shifted up 8, the low byte 0xff), at least 0 and at
/// most the largest value
pub fn tri_z(bits: u32, z: f32) -> u32 {
    let k = if bits == 32 { 24 } else { 16 };
    let x = z as f64 * (1u64 << k) as f64;
    let v = (x.ceil() - 1.0).clamp(0.0, ((1u64 << k) - 1) as f64) as u32;
    if bits == 32 { v << 8 | 0xff } else { v }
}

impl D3d7 {
    fn state(&self, st: i64) -> Option<u32> {
        self.rs.get(&st).copied().or_else(|| rs_default(self.ver, st))
    }

    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        match t[0].as_str() {
            "d3d" => {
                let v = n(op, 1)? as u32;
                if ![1, 2, 3, 7].contains(&v) {
                    return Err(Fail::Bad);
                }
                *self = D3d7::default();
                // MEASURED: the x86-64 DirectDraw has no Direct3D interfaces
                hit("D3D7.X64");
                if super::arch() == 64 {
                    out.push(format!("d3d qi {E_NOINTERFACE:08x}"));
                    return Ok(true);
                }
                hit("D3D7.CREATE");
                self.ver = v;
                out.push("d3d ok".into());
            }
            "d3denum" => {
                let caps = match t.get(1).map(String::as_str) {
                    None => false,
                    Some("caps") if t.len() == 2 => true,
                    _ => return Err(Fail::Bad),
                };
                if self.ver == 0 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                hit("D3D7.ENUM");
                let key = format!("enum {} ", self.ver);
                for l in TABLES.lines().filter_map(|l| l.strip_prefix(&key)) {
                    if caps || l.starts_with("dev ") {
                        out.push(l.to_string());
                    }
                }
            }
            "d3ddev" => {
                let dev = t.get(1).map(String::as_str).unwrap_or("");
                if !crate::gen::d3d7::DEVS.contains(&dev) {
                    return Err(Fail::Bad);
                }
                let fmt: &'static str = match t.get(2).map(String::as_str) {
                    Some("565") => "565",
                    Some("555") => "555",
                    Some("32") => "32",
                    Some("24") => "24",
                    Some("8") => "8",
                    _ => return Err(Fail::Bad),
                };
                let (w, h) = (n(op, 3)?, n(op, 4)?);
                let zbits = match t.get(5).map(String::as_str) {
                    None => 0,
                    Some("z16") => 16,
                    Some("z24") => 24,
                    Some("z32") => 32,
                    _ => return Err(Fail::Bad),
                };
                if !(1..=256).contains(&w) || !(1..=256).contains(&h) || t.len() > 6 {
                    return Err(Fail::Bad);
                }
                if self.ver == 0 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                self.dev = false;
                self.rt = None;
                self.rs.clear();
                hit("D3D7.DEVICE");
                let hw = dev == "hal" || dev == "tnl";
                // MEASURED (dev golden: 7 devices x 4 versions x 9 targets): a 24-bit z-buffer
                // in system memory has no pixel format, none in the display's video memory takes
                // 24 or 32 bits
                if zbits == 24 && !hw {
                    out.push(format!("d3ddev z hr {DDERR_INVALIDPIXELFORMAT:08x}"));
                    return Ok(true);
                }
                if zbits >= 24 && hw {
                    out.push(format!("d3ddev z hr {DDERR_UNSUPPORTED:08x}"));
                    return Ok(true);
                }
                // MEASURED: which device each version makes: RGB, MMX and the HAL everywhere,
                // T&L HAL only 7, ramp only 1 and 2 (3: E_INVALIDARG, 7: unknown); no
                // reference or null device; 8 bpp targets want a palette
                let hr = match (dev, self.ver) {
                    ("ref" | "null", _) => Some(E_NOINTERFACE),
                    ("ramp", 7) => Some(E_NOINTERFACE),
                    ("ramp", 3) | ("tnl", 1..=3) => Some(E_INVALIDARG),
                    _ if fmt == "8" => Some(DDERR_NOPALETTEATTACHED),
                    _ => None,
                };
                if let Some(hr) = hr {
                    out.push(format!("d3ddev hr {hr:08x}"));
                    return Ok(true);
                }
                if hw || dev == "ramp" || dev == "mmx" {
                    // (only the RGB rasterizer's pixels are modelled; the others are made)
                    self.rt = None;
                    self.dev = true;
                    self.ver += 100;
                } else {
                    self.dev = true;
                    // MEASURED (v1 golden): a new target holds zeros
                    self.rt = Some(Target { w, h, fmt, px: vec![0; (w * h) as usize], zbits, z: vec![0; if zbits > 0 { (w * h) as usize } else { 0 }] });
                }
                if zbits > 0 {
                    // MEASURED: ZENABLE is on when the target had a z-buffer
                    self.rs.insert(7, 1);
                }
                out.push("d3ddev ok".into());
            }
            "d3drs" | "d3dclear" | "d3dtri" | "d3dfill" | "d3ddump" | "d3dzdump" => return self.draw(op, out),
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn draw(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let ver = self.ver % 100;
        // (a device whose pixels the model does not draw: the HAL, MMX, ramp)
        let other = self.ver > 100;
        match t[0].as_str() {
            "d3drs" => {
                let set = match t.get(1).map(String::as_str) {
                    Some("set") => true,
                    Some("get") => false,
                    _ => return Err(Fail::Bad),
                };
                let st = n(op, 2)?;
                let v = if set { n(op, 3)? as u32 } else { 0 };
                if t.len() != if set { 4 } else { 3 } {
                    return Err(Fail::Bad);
                }
                if !self.dev || ver == 1 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if other {
                    return Err(Fail::Unmodeled("D3D7.OTHERDEV"));
                }
                hit("D3D7.RS");
                if set {
                    // MEASURED (rsset golden): SetRenderState keeps any value of a known state
                    // for GetRenderState; an unknown state: E_INVALIDARG
                    if rs_default(ver, st).is_none() {
                        out.push(format!("rs set {st} hr {E_INVALIDARG:08x}"));
                    } else {
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
                let c = n(op, 1)? as u32;
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
                if !self.dev || ver == 1 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if other {
                    return Err(Fail::Unmodeled("D3D7.OTHERDEV"));
                }
                let rt = self.rt.as_mut().unwrap();
                if z.is_some() && rt.zbits == 0 {
                    return Err(Fail::Unmodeled("D3D7.CLEAR.NOZ"));
                }
                // MEASURED (clearrects golden): the rectangle clipped to the target; one turned
                // round or empty clears nothing
                hit("D3D7.CLEAR");
                let (x0, y0, x1, y1) = r.unwrap_or((0, 0, rt.w, rt.h));
                let v = pixel(rt.fmt, c);
                for y in y0.max(0)..y1.min(rt.h) {
                    for x in x0.max(0)..x1.min(rt.w) {
                        let k = (y * rt.w + x) as usize;
                        rt.px[k] = v;
                        if let Some(z) = z {
                            hit("D3D7.CLEAR.Z");
                            rt.z[k] = clear_z(rt.zbits, z);
                        }
                    }
                }
            }
            "d3dtri" => {
                let cs: Vec<u32> = t.get(1).ok_or(Fail::Bad)?.split('/').map(|c| num(c).map(|v| v as u32).ok_or(Fail::Bad)).collect::<Res<_>>()?;
                if cs.len() != 1 && cs.len() != 3 {
                    return Err(Fail::Bad);
                }
                let mut v = [(0i64, 0i64); 3];
                for (k, p) in v.iter_mut().enumerate() {
                    *p = (n(op, 2 + 2 * k)?, n(op, 3 + 2 * k)?);
                }
                let mut z = 0.5f32;
                // (the specular colors only count with fog or specular on: D3D7.STATES)
                let mut k = 8;
                while k < t.len() {
                    match (t[k].as_str(), t.get(k + 1)) {
                        ("z", Some(_)) => z = n(op, k + 1)? as f32 / 1000.0,
                        ("spec", Some(sp)) => {
                            let ns = sp.split('/').map(|c| num(c).ok_or(Fail::Bad)).collect::<Res<Vec<_>>>()?.len();
                            if ns != 1 && ns != 3 {
                                return Err(Fail::Bad);
                            }
                        }
                        _ => return Err(Fail::Bad),
                    }
                    k += 2;
                }
                if !self.dev || ver == 1 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if other {
                    return Err(Fail::Unmodeled("D3D7.OTHERDEV"));
                }
                let c = [cs[0], *cs.get(1).unwrap_or(&cs[0]), *cs.get(2).unwrap_or(&cs[0])];
                self.tri(v, c, z)?;
            }
            "d3dfill" | "d3ddump" | "d3dzdump" => {
                let seed = if t[0] == "d3dfill" { Some(n(op, 1)? as u32) } else { None };
                if t.len() != if seed.is_some() { 2 } else { 1 } {
                    return Err(Fail::Bad);
                }
                if !self.dev {
                    out.push("nodev".into());
                    return Ok(true);
                }
                if other {
                    return Err(Fail::Unmodeled("D3D7.OTHERDEV"));
                }
                let rt = self.rt.as_mut().unwrap();
                let z = t[0] == "d3dzdump";
                if z && rt.zbits == 0 {
                    out.push("nodev".into());
                    return Ok(true);
                }
                hit("D3D7.LOCK");
                let b = if z { rt.zbits as usize / 8 } else { bytes(rt.fmt) };
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
                let (bits, data) = if z { (rt.zbits, &rt.z) } else { (b as u32 * 8, &rt.px) };
                out.push(format!("dd {} {} {} {bits}", if z { "z" } else { "rt" }, rt.w, rt.h));
                for row in data.chunks(rt.w as usize) {
                    out.push(row.iter().map(|v| v.to_le_bytes()[..b].iter().map(|x| format!("{x:02x}")).collect::<String>()).collect());
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// one D3DTLVERTEX triangle (x, y in 16ths of a pixel), the render states applied
    fn tri(&mut self, v: [(i64, i64); 3], c: [u32; 3], z: f32) -> Res<()> {
        let st = |k: i64| self.state(k).unwrap_or(0);
        let (cull, shade, dither, zen, zwrite, zfunc, blend, src, dst) = (st(22), st(9), st(26), st(7), st(14), st(23), st(27), st(19), st(20));
        let others = [(8, 3), (15, 0), (28, 0), (29, 0), (16, 1), (41, 0)];
        if others.iter().any(|&(k, d)| st(k) != d) {
            // (fill modes, the alpha test, fog, specular light, LASTPIXEL, stippling)
            return Err(Fail::Unmodeled("D3D7.STATES"));
        }
        let area = (v[1].0 - v[0].0) * (v[2].1 - v[0].1) - (v[2].0 - v[0].0) * (v[1].1 - v[0].1);
        // MEASURED (cull golden): D3DCULL_CCW (the default) drops triangles of negative area in
        // screen coordinates (x right, y down; area of v0 v1 v2), CW positive ones
        hit("D3D7.CULL");
        if area == 0 || (cull == 3 && area < 0) || (cull == 2 && area > 0) {
            return Ok(());
        }
        if !(1..=3).contains(&cull) {
            return Err(Fail::Unmodeled("D3D7.STATES"));
        }
        let gouraud = c[0] != c[1] || c[0] != c[2];
        if gouraud && shade != 1 {
            return Err(Fail::Unmodeled("D3D7.GOURAUD"));
        }
        // D3DSHADE_FLAT: the first vertex's color
        let col = c[0];
        if shade == 1 && gouraud {
            hit("D3D7.FLAT");
        }
        let rt = self.rt.as_mut().unwrap();
        if dither != 0 && rt.fmt != "32" && rt.fmt != "24" {
            return Err(Fail::Unmodeled("D3D7.DITHER"));
        }
        let (w, h) = (rt.w, rt.h);
        let inside = v.iter().all(|&(x, y)| (0..=w * 16).contains(&x) && (0..=h * 16).contains(&y));
        // the pixel (px, py) is covered when its sample point (px, py) is inside all three edges
        let (a, b, cc) = if area > 0 { (v[0], v[1], v[2]) } else { (v[0], v[2], v[1]) };
        let edges = [(a, b), (b, cc), (cc, a)];
        let mut cover = vec![false; (w * h) as usize];
        for py in 0..h {
            for px in 0..w {
                let (sx, sy) = (px * 16, py * 16);
                let mut ins = true;
                for &((ax, ay), (bx, by)) in &edges {
                    let e = (bx - ax) * (sy - ay) - (by - ay) * (sx - ax);
                    let len = (((bx - ax).pow(2) + (by - ay).pow(2)) as f64).sqrt();
                    // (within 1/8 pixel of an edge Windows' RGB rasterizer decides by its own
                    // arithmetic: not modelled)
                    if (e as f64).abs() / len < 2.0 {
                        return Err(Fail::Unmodeled("D3D7.TRI.EDGE"));
                    }
                    if e < 0 {
                        ins = false;
                    }
                }
                cover[(py * w + px) as usize] = ins;
            }
        }
        if !inside && !cover.iter().all(|&c| c) {
            // (a triangle past the target is clipped first: the edges Windows rasterizes then
            // are not modelled; one covering the whole target is)
            return Err(Fail::Unmodeled("D3D7.TRI.CLIP"));
        }
        hit(if inside { "D3D7.TRI" } else { "D3D7.TRI.COVERALL" });
        if blend != 0 && (src, dst) != (5, 6) {
            return Err(Fail::Unmodeled("D3D7.BLEND"));
        }
        if zen != 0 && rt.zbits == 0 {
            return Err(Fail::Unmodeled("D3D7.STATES"));
        }
        if zen != 0 && zfunc != 4 {
            return Err(Fail::Unmodeled("D3D7.ZFUNC"));
        }
        let zv = if rt.zbits > 0 { tri_z(rt.zbits, z) } else { 0 };
        let p = pixel(rt.fmt, col);
        for k in 0..(w * h) as usize {
            if !cover[k] {
                continue;
            }
            if zen != 0 {
                hit("D3D7.ZTEST");
                // D3DCMP_LESSEQUAL (the default)
                if zv > rt.z[k] {
                    continue;
                }
                if zwrite != 0 {
                    rt.z[k] = zv;
                }
            }
            if blend != 0 {
                // MEASURED (blend golden: 52 alphas x 128 pixels x 3 channels, all exact) SRCALPHA,
                // INVSRCALPHA: (s * a + d * (255 - a)) >> 8 per 8-bit channel
                hit("D3D7.BLEND.SRCALPHA");
                let a = col >> 24;
                let s = channels(rt.fmt, p);
                let d = channels(rt.fmt, rt.px[k]);
                let o: Vec<u32> = (0..3).map(|i| (s[i] * a + d[i] * (255 - a)) >> 8).collect();
                if rt.fmt != "32" && rt.fmt != "24" {
                    return Err(Fail::Unmodeled("D3D7.BLEND16"));
                }
                // (alpha 0, as Clear writes it: MEASURED blend golden)
                rt.px[k] = o[0] << 16 | o[1] << 8 | o[2];
            } else {
                rt.px[k] = p;
            }
        }
        Ok(())
    }
}
