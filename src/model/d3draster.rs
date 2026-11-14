//! Drawing on Direct3D 10 and 11: the input assembler (one vertex buffer, triangle lists and
//! strips, instances), the viewport transform, the rasterizer (16.8 fixed point vertices, pixel
//! centers, the top-left rule, culling, scissor), the pixel shaders the generator writes
//! (src/gen/d3ddraw.rs, known here by their bytes), depth tests and blending.
//!
//! Sources (Microsoft Learn): "Rasterization Rules" (triangles: a pixel is drawn when its center
//! is inside, the top-left rule on edges), "D3D11_RASTERIZER_DESC" (FrontCounterClockwise FALSE:
//! clockwise triangles face front; DepthClipEnable), "D3D11_CULL_MODE", "D3D11_VIEWPORT",
//! "Primitive Topologies" (the strip's alternate winding, the leading vertex), "D3D11_BLEND",
//! "D3D11_BLEND_OP", "D3D11_COMPARISON_FUNC", "ld (sm4 - asm)" (out of range: 0),
//! "D3D11_TEXTURE_ADDRESS_MODE", "SV_PrimitiveID", "SV_IsFrontFace", "SV_VertexID",
//! "SV_InstanceID", "Semantics" (SV_Position in the pixel shader: the pixel center).

use crate::spec::{clause, open};
use super::d3d11::{normalize, Kind};
use super::d3dres::{texel, D3dres, Dev, E_INVALIDARG};
use super::dxgiconv::{decode_f, encode, encode_bits, to_unorm};
use super::dxgifmt::{format, Format, Num};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub const OPS: &[&str] = &["shader", "layout", "vtx", "draw", "target", "viewport", "scissor", "rast", "blend1", "depthst", "tsrv", "samp", "cbuf"];

#[derive(Clone, Debug)]
pub struct Draw {
    shaders: [Option<(&'static str, bool)>; 8],
    vs: Option<&'static str>,
    ps: Option<&'static str>,
    layout: Option<usize>,
    vtx: Vec<u32>,
    target: Option<usize>,
    depth: Option<usize>,
    viewport: Option<[f32; 6]>,
    scissor: [i64; 4],
    /// fill, cull, front ccw, bias, clamp, slope, clip, scissor, ms, aa
    rast: [u32; 10],
    /// enable src dest op srca desta opa mask, the factor
    blend: ([u32; 8], [f32; 4]),
    depthst: [u32; 3],
    srv: Option<usize>,
    sampler: Option<[u32; 4]>,
}

impl Default for Draw {
    fn default() -> Draw {
        Draw {
            shaders: [None; 8],
            vs: None,
            ps: None,
            layout: None,
            vtx: Vec::new(),
            target: None,
            depth: None,
            viewport: None,
            scissor: [0; 4],
            rast: [3, 3, 0, 0, 0, 0, 1, 0, 0, 0],
            blend: ([0, 2, 1, 1, 2, 1, 1, 0xf], [0.0; 4]),
            depthst: [1, 1, 2],
            srv: None,
            sampler: None,
        }
    }
}

fn n(t: &[String], i: usize) -> Res<u32> {
    t.get(i).and_then(|s| num(s)).map(|v| v as u32).ok_or(Fail::Bad)
}

fn hexbytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap_or(0)).collect()
}

/// the generator's programs: name, bytes, a vertex shader
fn known(b: &[u8]) -> Option<(&'static str, bool)> {
    thread_local! {
        static S: Vec<(&'static str, Vec<u8>)> = crate::gen::d3ddraw::shaders();
    }
    S.with(|s| s.iter().find(|x| x.1 == b).map(|x| (x.0, x.0.starts_with("vs"))))
}

pub fn op(m: &mut D3dres, dev: Dev, op: &Op, out: &mut Vec<String>) -> Res<()> {
    let t = op.tok.as_slice();
    let d = &mut m.draw;
    match t[0].as_str() {
        "vtx" => {
            for i in 1..t.len() {
                if d.vtx.len() == 4096 {
                    return Err(Fail::Bad);
                }
                d.vtx.push(n(t, i)?);
            }
            if m.dev.is_none() {
                // (the executor appends them even without a device)
                return Ok(());
            }
        }
        "shader" => {
            let sl = n(t, 1)? as usize;
            let vs = t.get(2).is_some_and(|s| s == "vs");
            if t.len() != 4 || sl >= 8 || !(vs || t[2] == "ps") {
                return Err(Fail::Bad);
            }
            hit(clause::D3DRASTER_SHADER);
            d.shaders[sl] = None;
            let b = hexbytes(&t[3]);
            match known(&b) {
                Some((name, is_vs)) if is_vs == vs => {
                    d.shaders[sl] = Some((name, vs));
                    if vs {
                        d.vs = Some(name);
                    } else {
                        d.ps = Some(name);
                    }
                }
                _ => {
                    // (checksum, size or kind wrong: assumed refused like any other bad bytecode)
                    hit(clause::D3DRASTER_SHADER_BAD);
                    out.push(format!("shader hr {E_INVALIDARG:08x}"));
                    // (the blob is kept for an input layout: a program of the other kind by its
                    // name, bytes that are no program as "bad")
                    d.shaders[sl] = Some((known(&b).map_or("bad", |k| k.0), vs));
                }
            }
        }
        "layout" => {
            let sl = n(t, 1)? as usize;
            if sl >= 8 || t.len() < 3 || t.len() > 10 {
                return Err(Fail::Bad);
            }
            let fmts: Vec<u32> = (2..t.len()).map(|i| n(t, i)).collect::<Res<_>>()?;
            let Some((name, _)) = d.shaders[sl] else {
                out.push("layout noshader".into());
                return Ok(());
            };
            hit(clause::D3DRASTER_LAYOUT);
            d.layout = None;
            let needs = match name {
                "vs" => 2,
                "vsid" => 1,
                // (MEASURED: against bytes that are no program (a broken checksum), refused,
                // probe-layout-refused)
                "bad" => {
                    hit(clause::D3DRASTER_LAYOUT_BAD);
                    out.push(format!("layout hr {E_INVALIDARG:08x}"));
                    return Ok(());
                }
                _ => return Err(Fail::Unmodeled(open::D3DRASTER_LAYOUT_SIG)),
            };
            // every input of the program from an element, no element of an unknown format
            let ok = fmts.len() >= needs && fmts.iter().all(|f| format(*f).is_some_and(|f| !f.block() && !f.video() && f.num != Num::Typeless));
            if ok {
                d.layout = Some(fmts.len());
            } else {
                out.push(format!("layout hr {E_INVALIDARG:08x}"));
            }
        }
        "target" => {
            let r = if t.get(1).is_some_and(|s| s == "-") { None } else { Some(n(t, 1)? as usize) };
            let ds = if t.len() > 2 { Some(n(t, 2)? as usize) } else { None };
            if t.len() > 3 || r.is_some_and(|v| v >= 16) || ds.is_some_and(|v| v >= 16) {
                return Err(Fail::Bad);
            }
            d.target = r.filter(|&v| m.views[v].as_ref().is_some_and(|x| x.kind == 1));
            d.depth = ds.filter(|&v| m.views[v].as_ref().is_some_and(|x| x.kind == 2));
        }
        "viewport" => {
            if t.len() != 7 {
                return Err(Fail::Bad);
            }
            let mut v = [0f32; 6];
            for (i, x) in v.iter_mut().enumerate() {
                *x = f32::from_bits(n(t, i + 1)?);
            }
            if !dev.d3d11() {
                // (Direct3D 10 viewports: integer origin and size)
                for x in v.iter_mut().take(4) {
                    // (MEASURED: the executor's (INT) cast, toward zero, probe-vp-frac)
                    if *x < 0.0 && x.fract() != 0.0 {
                        hit(clause::D3DRASTER_VP10);
                    }
                    *x = x.trunc();
                }
            }
            hit(clause::D3DRASTER_VIEWPORT);
            d.viewport = Some(v);
        }
        "scissor" => {
            if t.len() != 5 {
                return Err(Fail::Bad);
            }
            for i in 0..4 {
                d.scissor[i] = num(&t[i + 1]).ok_or(Fail::Bad)?;
            }
        }
        "rast" => {
            if t.len() != 11 {
                return Err(Fail::Bad);
            }
            let w: Vec<u32> = (1..11).map(|i| n(t, i)).collect::<Res<_>>()?;
            match normalize(Kind::Raster, &w) {
                Some(v) => d.rast.copy_from_slice(&v),
                None => out.push(format!("rast hr {E_INVALIDARG:08x}")),
            }
        }
        "blend1" => {
            if t.len() != 9 && t.len() != 13 {
                return Err(Fail::Bad);
            }
            let w: Vec<u32> = (1..t.len()).map(|i| n(t, i)).collect::<Res<_>>()?;
            let mut desc = vec![0u32; 66];
            desc[1] = 1;
            desc[2..10].copy_from_slice(&w[..8]);
            for i in 1..8 {
                desc[2 + i * 8..10 + i * 8].copy_from_slice(&[0, 2, 1, 1, 2, 1, 1, 0xf]);
            }
            match normalize(Kind::Blend, &desc) {
                Some(v) => {
                    d.blend.0.copy_from_slice(&v[2..10]);
                    d.blend.1 = if w.len() == 12 { [8, 9, 10, 11].map(|i| f32::from_bits(w[i])) } else { [0.0; 4] };
                }
                None => out.push(format!("blend1 hr {E_INVALIDARG:08x}")),
            }
        }
        "depthst" => {
            if t.len() != 4 {
                return Err(Fail::Bad);
            }
            let w: Vec<u32> = (1..4).map(|i| n(t, i)).collect::<Res<_>>()?;
            let desc = [w[0], w[1], w[2], 0, 0xff, 1, 1, 1, 8, 1, 1, 1, 8];
            match normalize(Kind::Depth, &desc) {
                Some(v) => d.depthst.copy_from_slice(&v[..3]),
                None => out.push(format!("depthst hr {E_INVALIDARG:08x}")),
            }
        }
        "tsrv" => {
            let v = n(t, 1)? as usize;
            if t.len() != 2 || v >= 16 {
                return Err(Fail::Bad);
            }
            if m.views[v].as_ref().is_some_and(|x| x.kind == 0) {
                d.srv = Some(v);
            } else {
                out.push("tsrv noview".into());
            }
        }
        "samp" => {
            if t.len() != 5 {
                return Err(Fail::Bad);
            }
            let w: Vec<u32> = (1..5).map(|i| n(t, i)).collect::<Res<_>>()?;
            let desc = [w[0], w[1], w[2], w[3], 0, 1, 1, 0, 0, 0, 0, 0xff7f_ffff, 0x7f7f_ffff];
            match normalize(Kind::Sampler, &desc) {
                Some(_) => d.sampler = Some(w.try_into().unwrap()),
                None => out.push(format!("samp hr {E_INVALIDARG:08x}")),
            }
        }
        // (MEASURED: a bound constant buffer no program reads changes nothing, probe-cbuf)
        "cbuf" => {
            hit(clause::D3DRASTER_CBUF);
            let b = n(t, 1)? as usize;
            if b >= 16 {
                return Err(Fail::Bad);
            }
            // (the executor binds only a buffer)
            if m.res[b].as_ref().is_none_or(|r| r.d.dim != 4) {
                out.push("cbuf nores".into());
            }
        }
        "draw" => {
            if t.len() < 4 || t.len() > 5 {
                return Err(Fail::Bad);
            }
            let (top, count, stride) = (n(t, 1)?, n(t, 2)?, n(t, 3)?);
            let inst = if t.len() == 5 { n(t, 4)? } else { 0 };
            let vtx = std::mem::take(&mut d.vtx);
            draw(m, dev, top, count, stride, inst, &vtx)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

/// a vertex after the vertex shader: clip position and the color it gives
#[derive(Clone, Copy)]
struct Vert {
    pos: [f32; 4],
    color: [f32; 4],
}

/// the components of an element written: each component of the format whose channel the
/// mask names takes the new bits
fn merge(f: &Format, old: &[u8], new: &[u8], mask: u32) -> Vec<u8> {
    let mut out = old.to_vec();
    let mut at = 0;
    for (i, &bits) in f.comps.iter().enumerate() {
        if mask >> f.chans[i] & 1 == 1 {
            for b in at..at + bits {
                let (byte, bit) = ((b / 8) as usize, b % 8);
                out[byte] = out[byte] & !(1 << bit) | new[byte] & (1 << bit);
            }
        }
        at += bits;
    }
    out
}

/// a depth (and stencil) written into one element of a depth format
pub fn write_depth(f: &Format, e: &mut [u8], dw: bool, depth: f32, sw: bool, st: u8) -> Res<()> {
    match f.id {
        55 => {
            if dw {
                e.copy_from_slice(&(to_unorm(depth, 16) as u16).to_le_bytes());
            }
        }
        45 => {
            let mut v = u32::from_le_bytes(e[..4].try_into().unwrap());
            if dw {
                v = v & 0xff00_0000 | to_unorm(depth, 24);
            }
            if sw {
                v = v & 0xff_ffff | (st as u32) << 24;
            }
            e.copy_from_slice(&v.to_le_bytes());
        }
        40 => {
            if dw {
                e.copy_from_slice(&depth.to_bits().to_le_bytes());
            }
        }
        20 => {
            if dw {
                e[..4].copy_from_slice(&depth.to_bits().to_le_bytes());
            }
            if sw {
                e[4] = st;
            }
        }
        _ => return Err(Fail::Unmodeled(open::D3DRASTER_DEPTHFMT)),
    }
    Ok(())
}

/// the depth a format stores, as a number to compare
fn stored_depth(f: &Format, e: &[u8]) -> f64 {
    match f.id {
        55 => u16::from_le_bytes([e[0], e[1]]) as f64,
        45 => (u32::from_le_bytes(e[..4].try_into().unwrap()) & 0xff_ffff) as f64,
        _ => f32::from_le_bytes(e[..4].try_into().unwrap()) as f64,
    }
}

fn quantized_depth(f: &Format, z: f32) -> f64 {
    match f.id {
        55 => to_unorm(z, 16) as f64,
        45 => to_unorm(z, 24) as f64,
        _ => z as f64,
    }
}

fn factor(f: u32, s: [f32; 4], dst: [f32; 4], bf: [f32; 4], ch: usize) -> Res<f32> {
    Ok(match f {
        1 => 0.0,
        2 => 1.0,
        3 => s[ch],
        4 => 1.0 - s[ch],
        5 => s[3],
        6 => 1.0 - s[3],
        7 => dst[3],
        8 => 1.0 - dst[3],
        9 => dst[ch],
        10 => 1.0 - dst[ch],
        11 => {
            if ch == 3 {
                1.0
            } else {
                s[3].min(1.0 - dst[3])
            }
        }
        14 => bf[ch],
        15 => 1.0 - bf[ch],
        // (MEASURED: the second source, which no program here writes, is 0, probe-blend-src1)
        16 | 17 => {
            hit(clause::D3DRASTER_BLEND_FACTOR);
            0.0
        }
        18 | 19 => {
            hit(clause::D3DRASTER_BLEND_FACTOR);
            1.0
        }
        _ => return Err(Fail::Unmodeled(open::D3DRASTER_BLEND_OP)),
    })
}

fn blend(b: &[u32; 8], bf: [f32; 4], f: &Format, s: [f32; 4], dst: [f32; 4]) -> Res<[f32; 4]> {
    hit(clause::D3DRASTER_BLEND);
    // (normalized targets: the shader's value clamped to the format's range first)
    let s = match f.num {
        Num::Unorm | Num::Srgb => s.map(|v| v.clamp(0.0, 1.0)),
        Num::Snorm => s.map(|v| v.clamp(-1.0, 1.0)),
        _ => s,
    };
    let mut o = [0f32; 4];
    for ch in 0..4 {
        let (sf, df, op) = if ch < 3 { (b[1], b[2], b[3]) } else { (b[4], b[5], b[6]) };
        let (x, y) = (s[ch] * factor(sf, s, dst, bf, ch)?, dst[ch] * factor(df, s, dst, bf, ch)?);
        o[ch] = match op {
            1 => x + y,
            2 => x - y,
            3 => y - x,
            4 => s[ch].min(dst[ch]),
            5 => s[ch].max(dst[ch]),
            _ => return Err(Fail::Unmodeled(open::D3DRASTER_BLEND_OP)),
        };
    }
    Ok(o)
}

/// the address modes on a texel coordinate (in texels) of a size
fn address(mode: u32, i: i64, size: i64) -> Option<i64> {
    hit(clause::D3DRASTER_ADDRESS);
    Some(match mode {
        1 => i.rem_euclid(size),
        2 => {
            let k = i.rem_euclid(2 * size);
            if k < size {
                k
            } else {
                2 * size - 1 - k
            }
        }
        3 => i.clamp(0, size - 1),
        4 => {
            if i < 0 || i >= size {
                return None;
            }
            i
        }
        _ => {
            let k = if i < 0 { -1 - i } else { i };
            k.min(size - 1)
        }
    })
}

#[allow(clippy::too_many_arguments)]
fn draw(m: &mut D3dres, dev: Dev, top: u32, count: u32, stride: u32, inst: u32, vtx: &[u32]) -> Res<()> {
    hit(clause::D3DRASTER_DRAW);
    let d = m.draw.clone();
    // (MEASURED: without programs and layout, or without a viewport, nothing is drawn,
    // probe-incomplete, probe-noviewport; without a target nor a depth view nothing is there to
    // draw to)
    let (Some(vs), Some(ps), Some(_)) = (d.vs, d.ps, d.layout) else {
        hit(clause::D3DRASTER_INCOMPLETE);
        return Ok(());
    };
    let Some(tv) = d.target else {
        if d.depth.is_some() {
            return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
        }
        hit(clause::D3DRASTER_INCOMPLETE);
        return Ok(());
    };
    let Some(vp) = d.viewport else {
        hit(clause::D3DRASTER_NOVIEWPORT);
        return Ok(());
    };
    if !matches!(top, 4 | 5) {
        return Err(Fail::Unmodeled(open::D3DRASTER_TOPOLOGY));
    }
    let view = m.views[tv].clone().unwrap();
    let rt_fmt = format(view.format()).ok_or(Fail::Unmodeled(open::D3DRASTER_RTFMT))?;
    let rt_sub = super::d3dres::view_subs(dev, &view, m.res[view.res].as_ref().unwrap())[0];
    let (rw, rh, _) = {
        let r = m.res[view.res].as_ref().unwrap();
        if r.d.count > 1 || r.d.dim != 2 {
            return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
        }
        r.size(r.mip_of(rt_sub))
    };
    let dsv = d.depth.map(|v| m.views[v].clone().unwrap());
    let instances = inst.max(1);
    // the vertices through the vertex shader
    let fetch = |v: u32, i: u32| -> Res<Vert> {
        // (MEASURED: past the buffer the vertex reads zeros, probe-vb-past)
        let w = |k: u32| -> Res<f32> {
            Ok(vtx.get((v * stride / 4 + k) as usize).map_or_else(|| {
                hit(clause::D3DRASTER_VBPAST);
                0.0
            }, |x| f32::from_bits(*x)))
        };
        let pos = [w(0)?, w(1)?, w(2)?, w(3)?];
        let color = match vs {
            "vs" => [w(4)?, w(5)?, w(6)?, w(7)?],
            _ => {
                hit(clause::D3DRASTER_IDS);
                [v as f32, i as f32, 0.0, 1.0]
            }
        };
        Ok(Vert { pos, color })
    };
    let ntri = match top {
        4 => count / 3,
        _ => count.saturating_sub(2),
    };
    for i in 0..instances {
        for p in 0..ntri {
            // (strips: every other triangle's first two vertices swapped; the leading vertex first)
            let idx = match top {
                4 => [3 * p, 3 * p + 1, 3 * p + 2],
                _ if p % 2 == 0 => [p, p + 1, p + 2],
                _ => {
                    hit(clause::D3DRASTER_STRIP);
                    [p + 1, p, p + 2]
                }
            };
            let lead = if top == 5 { p } else { 3 * p };
            let tri = [fetch(idx[0], i)?, fetch(idx[1], i)?, fetch(idx[2], i)?];
            let leadv = fetch(lead, i)?;
            triangle(m, dev, &d, vp, ps, &tri, leadv, p, (rw, rh), &view, rt_fmt, rt_sub, dsv.as_ref())?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn triangle(m: &mut D3dres, dev: Dev, d: &Draw, vp: [f32; 6], ps: &str, tri: &[Vert; 3], lead: Vert, prim: u32, (rw, rh): (u32, u32), view: &super::d3dres::View, rt_fmt: &'static Format, rt_sub: u32, dsv: Option<&super::d3dres::View>) -> Res<()> {
    // (every vertex at w <= 0: wholly outside the clip volume, or (w = 0, so x = y = 0 inside
    // it) a point: nothing drawn, probe-vb-past)
    if tri.iter().all(|v| v.pos[3] <= 0.0 && v.pos.iter().all(|x| x.is_finite()) && (v.pos[3] < 0.0 || v.pos[0] == 0.0 && v.pos[1] == 0.0)) {
        hit(clause::D3DRASTER_CLIP);
        return Ok(());
    }
    if tri.iter().any(|v| !(v.pos[3] > 0.0) || v.pos.iter().any(|x| !x.is_finite())) {
        return Err(Fail::Unmodeled(open::D3DRASTER_CLIPW));
    }
    // depth clipping: whole triangles in these cases
    let zs: Vec<f32> = tri.iter().map(|v| v.pos[2] / v.pos[3]).collect();
    let inside = |z: f32| (0.0..=1.0).contains(&z);
    if d.rast[6] != 0 && !zs.iter().all(|z| inside(*z)) {
        if zs.iter().all(|z| *z < 0.0) || zs.iter().all(|z| *z > 1.0) {
            hit(clause::D3DRASTER_DEPTHCLIP);
            return Ok(());
        }
        return Err(Fail::Unmodeled(open::D3DRASTER_CLIPZ));
    }
    // the viewport transform, then 16.8 fixed point
    let [vx, vy, vw, vh, minz, maxz] = vp;
    let to_screen = |q: &[f32; 4]| (vx + (q[0] / q[3] + 1.0) * (vw * 0.5), vy + (1.0 - q[1] / q[3]) * (vh * 0.5));
    hit(clause::D3DRASTER_SNAP);
    let snap = |v: f32| -> i64 {
        let x = v as f64 * 256.0;
        let f = x.floor();
        let r = x - f;
        (if r > 0.5 || r == 0.5 && (f as i64) % 2 != 0 { f + 1.0 } else { f }) as i64
    };
    let snap2 = |q: &[f32; 4]| {
        let (x, y) = to_screen(q);
        (snap(x), snap(y))
    };
    let p: Vec<(i64, i64)> = tri.iter().map(|v| snap2(&v.pos)).collect();
    let cross = |a: (i64, i64), b: (i64, i64), c: (i64, i64)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let area = cross(p[0], p[1], p[2]);
    if area == 0 {
        hit(clause::D3DRASTER_DEGENERATE);
        return Ok(());
    }
    // (y down: a positive area is clockwise on the screen)
    let cw = area > 0;
    let front = cw != (d.rast[2] != 0);
    hit(clause::D3DRASTER_CULL);
    if d.rast[1] == 2 && front || d.rast[1] == 3 && !front {
        return Ok(());
    }
    if d.rast[0] != 3 {
        return Err(Fail::Unmodeled(open::D3DRASTER_WIREFRAME));
    }
    let v = if cw { [p[0], p[1], p[2]] } else { [p[0], p[2], p[1]] };
    let order = if cw { [0, 1, 2] } else { [0, 2, 1] };
    let top_left = |a: (i64, i64), b: (i64, i64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        dy < 0 || dy == 0 && dx > 0
    };
    // (MEASURED, WARP: a triangle reaching past the viewport is clipped to it first, in clip
    // space, and the pieces' new vertices snapped as any other, tri-2: so an edge moves by up
    // to the snapping of its new end)
    let poly = clip_xy(tri.map(|v| v.pos));
    let pieces: Vec<[(i64, i64); 3]> = if poly.len() == 3 && poly.iter().zip(tri.iter()).all(|(a, b)| *a == b.pos) {
        vec![v]
    } else {
        hit(clause::D3DRASTER_CLIP);
        let q: Vec<(i64, i64)> = poly.iter().map(snap2).collect();
        (1..q.len().saturating_sub(1))
            .map(|i| [q[0], q[i], q[i + 1]])
            .filter(|t| cross(t[0], t[1], t[2]) != 0)
            .map(|t| if cross(t[0], t[1], t[2]) > 0 { t } else { [t[0], t[2], t[1]] })
            .collect()
    };
    // depth: one value per triangle in these cases
    let z = {
        let zz: Vec<f32> = zs.iter().map(|z| minz + z * (maxz - minz)).collect();
        if zz.iter().any(|x| *x != zz[0]) {
            None
        } else {
            let (lo, hi) = if minz <= maxz { (minz, maxz) } else { (maxz, minz) };
            Some(zz[0].clamp(lo, hi))
        }
    };
    // the pixels: inside the target, the viewport and (when on) the scissor rectangle
    hit(clause::D3DRASTER_VPCLIP);
    let mut x0 = 0i64.max(vx.ceil() as i64 - 1);
    let mut y0 = 0i64.max(vy.ceil() as i64 - 1);
    let mut x1 = (rw as i64).min((vx + vw).ceil() as i64 + 1);
    let mut y1 = (rh as i64).min((vy + vh).ceil() as i64 + 1);
    if d.rast[7] != 0 {
        hit(clause::D3DRASTER_SCISSOR);
        x0 = x0.max(d.scissor[0]);
        y0 = y0.max(d.scissor[1]);
        x1 = x1.min(d.scissor[2]);
        y1 = y1.min(d.scissor[3]);
    }
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
            if cx < vx || cx >= vx + vw || cy < vy || cy >= vy + vh {
                continue;
            }
            let c = (px * 256 + 128, py * 256 + 128);
            let inside = |t: &[(i64, i64); 3]| {
                (0..3).all(|e| {
                    let (a, b) = (t[e], t[(e + 1) % 3]);
                    let w = cross(a, b, c);
                    w > 0 || w == 0 && top_left(a, b)
                })
            };
            if !pieces.iter().any(inside) {
                continue;
            }
            hit(clause::D3DRASTER_TOPLEFT);
            let bary = {
                // (barycentric weights at the pixel center, in the clockwise order)
                let a = area.unsigned_abs() as f64;
                let w0 = cross(v[1], v[2], c) as f64 / a;
                let w1 = cross(v[2], v[0], c) as f64 / a;
                let w2 = 1.0 - w0 - w1;
                let mut w = [0f64; 3];
                w[order[0]] = w0;
                w[order[1]] = w1;
                w[order[2]] = w2;
                w
            };
            let out = shade(m, ps, tri, lead, prim, front, (px, py), z, bary, d)?;
            output(m, dev, d, view, rt_fmt, rt_sub, dsv, (px as usize, py as usize, rw as usize), z, out)?;
        }
    }
    Ok(())
}

/// a triangle clipped to -w <= x <= w and -w <= y <= w (Sutherland-Hodgman, in single
/// precision: a new vertex a + (b - a) t, t the distance to the plane at a over the difference)
fn clip_xy(tri: [[f32; 4]; 3]) -> Vec<[f32; 4]> {
    let mut poly = tri.to_vec();
    for plane in 0..4 {
        let dist = |q: &[f32; 4]| match plane {
            0 => q[3] + q[0],
            1 => q[3] - q[0],
            2 => q[3] + q[1],
            _ => q[3] - q[1],
        };
        if poly.iter().all(|q| dist(q) >= 0.0) {
            continue;
        }
        let mut out = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (da, db) = (dist(&a), dist(&b));
            if da >= 0.0 {
                out.push(a);
            }
            if (da >= 0.0) != (db >= 0.0) {
                let t = da / (da - db);
                out.push([0, 1, 2, 3].map(|k| a[k] + (b[k] - a[k]) * t));
            }
        }
        poly = out;
    }
    poly
}

/// the pixel shader: what it writes to SV_Target, as bits (floats or uints)
#[allow(clippy::too_many_arguments)]
fn shade(m: &D3dres, ps: &str, tri: &[Vert; 3], lead: Vert, prim: u32, front: bool, (px, py): (i64, i64), z: Option<f32>, bary: [f64; 3], d: &Draw) -> Res<[u32; 4]> {
    let same = |f: &dyn Fn(&Vert) -> [f32; 4]| -> Res<[f32; 4]> {
        let a = f(&tri[0]);
        if tri.iter().all(|v| f(v) == a) {
            Ok(a)
        } else {
            Err(Fail::Unmodeled(open::D3DRASTER_INTERP))
        }
    };
    Ok(match ps {
        "ps" => same(&|v| v.color)?.map(f32::to_bits),
        "psflat" => {
            hit(clause::D3DRASTER_FLAT);
            lead.color.map(f32::to_bits)
        }
        "pspos" => {
            hit(clause::D3DRASTER_POS);
            // (MEASURED: w reads as the vertices' clip w, pos-3, pos-4)
            let w = same(&|v| [v.pos[3]; 4])?[0];
            if w != 1.0 {
                hit(clause::D3DRASTER_POSW);
            }
            let z = z.ok_or(Fail::Unmodeled(open::D3DRASTER_INTERP))?;
            [px as f32 + 0.5, py as f32 + 0.5, z, w].map(f32::to_bits)
        }
        "psprim" => {
            hit(clause::D3DRASTER_PRIM);
            [(prim as f32).to_bits(), if front { 1.0f32 } else { 0.0 }.to_bits(), 0, 1.0f32.to_bits()]
        }
        "psld" | "psldu" => {
            hit(clause::D3DRASTER_LD);
            // (an unbound shader resource reads as zeros, ld-89)
            let Some(sv) = d.srv else {
                hit(clause::D3DRASTER_NOSRV);
                return Ok([0; 4]);
            };
            let v = m.views[sv].as_ref().unwrap();
            let r = m.res[v.res].as_ref().unwrap();
            let (w, h, _) = r.size(0);
            if px >= w as i64 || py >= h as i64 {
                hit(clause::D3DRASTER_LD_OUT);
                [0; 4]
            } else {
                texel(r, 0, v.format(), px as u32, py as u32)?
            }
        }
        "pssample" => {
            hit(clause::D3DRASTER_SAMPLE);
            // (an unbound shader resource reads as zeros, ld-89)
            let Some(sv) = d.srv else {
                hit(clause::D3DRASTER_NOSRV);
                return Ok([0; 4]);
            };
            let Some(s) = d.sampler else { return Err(Fail::Unmodeled(open::D3DRASTER_NOSAMPLER)) };
            let v = m.views[sv].as_ref().unwrap();
            let r = m.res[v.res].as_ref().unwrap();
            let (w, h, _) = r.size(0);
            let uv: Vec<f64> = (0..2).map(|k| (0..3).map(|i| bary[i] * tri[i].color[k] as f64).sum()).collect();
            let tc = [uv[0] * w as f64, uv[1] * h as f64];
            // (only texel centers in these cases: point and linear filtering agree there)
            for c in tc {
                if ((c - 0.5) - (c - 0.5).round()).abs() > 1e-3 {
                    return Err(Fail::Unmodeled(open::D3DRASTER_FILTER));
                }
            }
            if !matches!(s[0], 0 | 0x15) {
                return Err(Fail::Unmodeled(open::D3DRASTER_FILTER));
            }
            let i = (tc[0] - 0.5).round() as i64;
            let j = (tc[1] - 0.5).round() as i64;
            match (address(s[1], i, w as i64), address(s[2], j, h as i64)) {
                (Some(x), Some(y)) => texel(r, 0, v.format(), x as u32, y as u32)?,
                _ => {
                    hit(clause::D3DRASTER_BORDER);
                    [0; 4]
                }
            }
        }
        _ => return Err(Fail::Unmodeled(open::D3DRASTER_PS)),
    })
}

#[allow(clippy::too_many_arguments)]
fn output(m: &mut D3dres, dev: Dev, d: &Draw, view: &super::d3dres::View, f: &'static Format, sub: u32, dsv: Option<&super::d3dres::View>, (x, y, _w): (usize, usize, usize), z: Option<f32>, out: [u32; 4]) -> Res<()> {
    // the depth test
    if let Some(dv) = dsv {
        hit(clause::D3DRASTER_DEPTH);
        let df = format(dv.format()).ok_or(Fail::Unmodeled(open::D3DRASTER_DEPTHFMT))?;
        let dsub = super::d3dres::view_subs(dev, dv, m.res[dv.res].as_ref().unwrap())[0];
        let dres = m.res[dv.res].as_mut().unwrap();
        let (rb, _, _) = dres.layout(dsub);
        let el = (df.bits / 8) as usize;
        let b = dres.subs[dsub as usize].as_mut().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
        let e = &mut b[y * rb + x * el..][..el];
        if d.depthst[0] != 0 {
            let z = z.ok_or(Fail::Unmodeled(open::D3DRASTER_INTERP))?;
            let (a, s) = (quantized_depth(df, z), stored_depth(df, e));
            let pass = match d.depthst[2] {
                1 => false,
                2 => a < s,
                3 => a == s,
                4 => a <= s,
                5 => a > s,
                6 => a != s,
                7 => a >= s,
                _ => true,
            };
            if !pass {
                return Ok(());
            }
            if d.depthst[1] != 0 {
                write_depth(df, e, true, z, false, 0)?;
            }
        }
    }
    let res = m.res[view.res].as_mut().unwrap();
    let (rb, _, _) = res.layout(sub);
    let el = (f.bits / 8) as usize;
    let b = res.subs[sub as usize].as_mut().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
    let e = &mut b[y * rb + x * el..][..el];
    let new = match f.num {
        Num::Uint | Num::Sint => {
            if d.blend.0[0] != 0 {
                hit(clause::D3DRASTER_BLEND_INT);
            }
            encode_bits(f, out)
        }
        _ => {
            let s = out.map(f32::from_bits);
            let c = if d.blend.0[0] != 0 { blend(&d.blend.0, d.blend.1, f, s, decode_f(f, e)?)? } else { s };
            encode(f, c)?
        }
    };
    let mask = d.blend.0[7];
    if mask != 0xf {
        hit(clause::D3DRASTER_WRITEMASK);
    }
    let merged = merge(f, e, &new, mask);
    e.copy_from_slice(&merged);
    Ok(())
}
