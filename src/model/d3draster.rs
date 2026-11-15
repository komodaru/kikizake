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
                // (MEASURED: against a pixel program given as a vertex one, refused,
                // probe-layout-otherkind)
                "ps" | "psflat" | "pspos" | "psprim" | "psld" | "psldu" | "pssample" => {
                    hit(clause::D3DRASTER_LAYOUT_SIG);
                    out.push(format!("layout hr {E_INVALIDARG:08x}"));
                    return Ok(());
                }
                _ => return Err(Fail::Bad),
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
    let Some(topo) = Topo::of(top) else {
        // (MEASURED, WARP: probe-topology-33, x86 and x86-64, Direct3D 10 and 11) a patch list
        // with no hull and domain programs ends the executor (what it printed since its last
        // flush, before the case's last tex line, is lost)
        if (33..=64).contains(&top) {
            hit(clause::D3DRASTER_PATCH);
            return Err(Fail::Crash);
        }
        return Err(Fail::Unmodeled(open::D3DRASTER_TOPOLOGY));
    };
    let view = m.views[tv].clone().unwrap();
    let rt_fmt = format(view.format()).ok_or(Fail::Unmodeled(open::D3DRASTER_RTFMT))?;
    let rt_sub = super::d3dres::view_subs(dev, &view, m.res[view.res].as_ref().unwrap())[0];
    let (rw, rh, _) = {
        let r = m.res[view.res].as_ref().unwrap();
        if r.d.dim != 2 {
            return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
        }
        r.size(r.mip_of(rt_sub))
    };
    // (a multisampled target: D3DRASTER.MS.SAMPLES, triangles only, the viewport on whole
    // pixels, quality 0)
    let samples = {
        let r = m.res[view.res].as_ref().unwrap();
        match r.d.count {
            0 | 1 => None,
            n => {
                let [vx, vy, vw, vh, _, _] = vp;
                let whole = [vx, vy, vx + vw, vy + vh].iter().all(|v| v.fract() == 0.0);
                // (MEASURED, WARP: probe-rt-ms4-lines) aliased lines (neither line state) light
                // every sample of the pixels they light
                let aliased_lines = matches!(topo, Topo::Lines | Topo::LineStrip | Topo::LinesAdj | Topo::LineStripAdj) && d.rast[8] == 0 && d.rast[9] == 0 && r.samples.iter().all(Option::is_none);
                if aliased_lines && whole && r.d.q == 0 {
                    hit(clause::D3DRASTER_MS_LINES);
                }
                // (MEASURED, WARP: probe-rt-ms4-points) a point covers the samples of the pixel
                // square about it, by the top-left rule
                let points = topo == Topo::Points && r.samples.iter().all(Option::is_none);
                if points && whole && r.d.q == 0 {
                    hit(clause::D3DRASTER_MS_POINTS);
                }
                // (MEASURED, WARP: probe-rt-ms4-wire) wireframe's aliased edges likewise
                let tris = matches!(topo, Topo::Tris | Topo::TriStrip | Topo::TrisAdj | Topo::TriStripAdj);
                let wire = tris && d.rast[0] == 2 && d.rast[8] == 0 && d.rast[9] == 0 && r.samples.iter().all(Option::is_none);
                if wire && whole && r.d.q == 0 {
                    hit(clause::D3DRASTER_MS_LINES);
                }
                if !whole || !(aliased_lines || points || wire || tris && d.rast[0] != 2) || r.d.q != 0 {
                    return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
                }
                Some(sample_pattern(n).ok_or(Fail::Unmodeled(open::D3DRASTER_RTKIND))?)
            }
        }
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
    // (a triangle wholly past the buffer reads zeros, a point at w = 0, and with a zero stride
    // all three vertices are one point: nothing drawn, so the draw stops there)
    let nv = u64::try_from(vtx.len()).unwrap_or(u64::MAX).saturating_mul(4).checked_div(u64::from(stride)).map_or(1, |n| n + 1);
    let nprim = match topo {
        Topo::Points => u64::from(count).min(nv),
        Topo::Lines => u64::from(count / 2).min(nv.div_ceil(2)),
        Topo::LineStrip => u64::from(count.saturating_sub(1)).min(nv),
        Topo::Tris => u64::from(count / 3).min(nv.div_ceil(3)),
        Topo::TriStrip => u64::from(count.saturating_sub(2)).min(nv),
        Topo::LinesAdj => u64::from(count / 4).min(nv.div_ceil(4)),
        Topo::LineStripAdj => u64::from(count.saturating_sub(3)).min(nv),
        Topo::TrisAdj => u64::from(count / 6).min(nv.div_ceil(6)),
        Topo::TriStripAdj => u64::from((count / 2).saturating_sub(2)).min(nv.div_ceil(2)),
    };
    let nprim = u32::try_from(nprim).unwrap_or(u32::MAX);
    let tg = Target { dev, vp, size: (rw, rh), view: &view, fmt: rt_fmt, sub: rt_sub, dsv: dsv.as_ref(), samples };
    for i in 0..instances {
        // (the lines of the draw, or the edges of its triangles in wireframe: drawn together,
        // since whether a line's end lights its pixel depends on the lines after it)
        let mut segs: Vec<Seg> = Vec::new();
        for p in 0..nprim {
            match topo {
                Topo::Points => point(m, &d, &tg, ps, fetch(p, i)?, p)?,
                Topo::Lines | Topo::LineStrip | Topo::LinesAdj | Topo::LineStripAdj => {
                    // (MEASURED, WARP: probe-topology-10 to -13, D3DRASTER.ADJ) with adjacency and no geometry
                    // program, the adjacent vertices are skipped: lines 4p + 1 to 4p + 2, and
                    // p + 1 to p + 2
                    if matches!(topo, Topo::LinesAdj | Topo::LineStripAdj) {
                        hit(clause::D3DRASTER_ADJ);
                    }
                    let k = match topo {
                        Topo::Lines => 2 * p,
                        Topo::LinesAdj => 4 * p + 1,
                        Topo::LineStripAdj => p + 1,
                        _ => p,
                    };
                    let a = fetch(k, i)?;
                    segs.push(Seg { a, b: fetch(k + 1, i)?, lead: a, prim: p, front: true });
                }
                Topo::Tris | Topo::TriStrip | Topo::TrisAdj | Topo::TriStripAdj => {
                    // (strips: every other triangle's first two vertices swapped; the leading
                    // vertex first)
                    // (MEASURED, WARP: probe-topology-12, -13) with adjacency and no geometry
                    // program, the triangles of the even vertices: 6p, 6p + 2, 6p + 4, and the
                    // strip of 0, 2, 4, ...
                    let (idx, lead) = match topo {
                        Topo::Tris => ([3 * p, 3 * p + 1, 3 * p + 2], 3 * p),
                        Topo::TrisAdj => {
                            hit(clause::D3DRASTER_ADJ);
                            ([6 * p, 6 * p + 2, 6 * p + 4], 6 * p)
                        }
                        Topo::TriStripAdj if p % 2 == 0 => ([2 * p, 2 * p + 2, 2 * p + 4], 2 * p),
                        Topo::TriStripAdj => ([2 * p + 2, 2 * p, 2 * p + 4], 2 * p),
                        _ if p % 2 == 0 => ([p, p + 1, p + 2], p),
                        _ => {
                            hit(clause::D3DRASTER_STRIP);
                            ([p + 1, p, p + 2], p)
                        }
                    };
                    let tri = [fetch(idx[0], i)?, fetch(idx[1], i)?, fetch(idx[2], i)?];
                    let leadv = fetch(lead, i)?;
                    triangle(m, dev, &d, vp, ps, &tri, leadv, p, (rw, rh), samples, &view, rt_fmt, rt_sub, dsv.as_ref(), &mut segs)?;
                }
            }
        }
        lines(m, &d, &tg, ps, &segs)?;
    }
    Ok(())
}

/// the primitive topologies the model draws (D3D11_PRIMITIVE_TOPOLOGY 1 to 5, 10 to 13)
#[derive(Clone, Copy, PartialEq, Eq)]
enum Topo {
    Points,
    Lines,
    LineStrip,
    Tris,
    TriStrip,
    LinesAdj,
    LineStripAdj,
    TrisAdj,
    TriStripAdj,
}

impl Topo {
    fn of(top: u32) -> Option<Topo> {
        match top {
            1 => Some(Topo::Points),
            2 => Some(Topo::Lines),
            3 => Some(Topo::LineStrip),
            4 => Some(Topo::Tris),
            5 => Some(Topo::TriStrip),
            10 => Some(Topo::LinesAdj),
            11 => Some(Topo::LineStripAdj),
            12 => Some(Topo::TrisAdj),
            13 => Some(Topo::TriStripAdj),
            _ => None,
        }
    }
}

/// what a primitive is drawn into
struct Target<'a> {
    dev: Dev,
    vp: [f32; 6],
    size: (u32, u32),
    view: &'a super::d3dres::View,
    fmt: &'static Format,
    sub: u32,
    dsv: Option<&'a super::d3dres::View>,
    /// a multisampled target's sample positions
    samples: Option<&'static [(i64, i64)]>,
}

impl Target<'_> {
    /// a pixel inside the target, its center inside the viewport and (when on) the scissor
    /// rectangle (D3DRASTER.VPCLIP)
    fn drawable(&self, d: &Draw, (px, py): (i64, i64)) -> bool {
        let [vx, vy, vw, vh, _, _] = self.vp;
        let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
        (0..i64::from(self.size.0)).contains(&px)
            && (0..i64::from(self.size.1)).contains(&py)
            && cx >= vx
            && cx < vx + vw
            && cy >= vy
            && cy < vy + vh
            && (d.rast[7] == 0 || px >= d.scissor[0] && px < d.scissor[2] && py >= d.scissor[1] && py < d.scissor[3])
    }

    /// a depth in the clip volume's [0, 1] onto the viewport's range
    fn depth(&self, z: f32) -> f32 {
        let [_, _, _, _, minz, maxz] = self.vp;
        let (lo, hi) = if minz <= maxz { (minz, maxz) } else { (maxz, minz) };
        // (max and min, not clamp: a range of NaN leaves the value)
        (minz + z * (maxz - minz)).max(lo).min(hi)
    }

    fn put(&self, m: &mut D3dres, d: &Draw, (px, py): (i64, i64), z: f32, out: [u32; 4]) -> Res<()> {
        let at = (usize::try_from(px).unwrap_or(0), usize::try_from(py).unwrap_or(0), self.size.0 as usize);
        output(m, self.dev, d, self.view, self.fmt, self.sub, self.dsv, at, Some(z), out)
    }
}

/// a position in 1/256 pixels (16.8 fixed point, D3DRASTER.SNAP)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Sub {
    x: i64,
    y: i64,
}

/// a coordinate snapped to 1/256 pixel, rounding half to even (2^31 pixels: past any guard
/// band; the products of the edge tests stay within i128)
fn snap256(v: f32) -> i64 {
    let x = f64::from(v).clamp(-2147483648.0, 2147483648.0) * 256.0;
    let f = x.floor();
    let r = x - f;
    (if r > 0.5 || r == 0.5 && (f as i64) % 2 != 0 { f + 1.0 } else { f }) as i64
}

/// a clip position through the viewport transform, snapped
fn snapped(vp: [f32; 6], q: &[f32; 4]) -> Sub {
    let [vx, vy, vw, vh, _, _] = vp;
    Sub { x: snap256(vx + (q[0] / q[3] + 1.0) * (vw * 0.5)), y: snap256(vy + (1.0 - q[1] / q[3]) * (vh * 0.5)) }
}

/// a vertex's w that the model can draw with: positive and every coordinate finite
fn drawable_w(v: &Vert) -> Res<()> {
    if !(v.pos[3] > 0.0) || v.pos.iter().any(|x| !x.is_finite()) {
        return Err(Fail::Unmodeled(open::D3DRASTER_CLIPW));
    }
    Ok(())
}

/// (MEASURED, WARP: probe-points-*, every pixel) a point inside the clip volume lights the
/// one pixel (ceil(x) - 1, ceil(y) - 1) of its snapped position, in the vertex's color
fn point(m: &mut D3dres, d: &Draw, tg: &Target, ps: &str, v: Vert, prim: u32) -> Res<()> {
    hit(clause::D3DRASTER_POINT);
    drawable_w(&v)?;
    let q = v.pos;
    if !(-q[3] <= q[0] && q[0] <= q[3] && -q[3] <= q[1] && q[1] <= q[3]) {
        hit(clause::D3DRASTER_CLIP);
        return Ok(());
    }
    let z = q[2] / q[3];
    if !(0.0..=1.0).contains(&z) {
        // (outside the depth range a point is wholly in or out, as a triangle on one side)
        if d.rast[6] != 0 {
            hit(clause::D3DRASTER_DEPTHCLIP);
            return Ok(());
        }
    }
    let s = snapped(tg.vp, &q);
    if let Some(pattern) = tg.samples {
        let z = tg.depth(z);
        let out = prim_out(ps, v.color, (0, 0), z, Some(q[3]), prim, true)?;
        if ps == "pspos" {
            return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
        }
        for py in (s.y - 128).div_euclid(256) - 1..=(s.y + 128).div_euclid(256) + 1 {
            for px in (s.x - 128).div_euclid(256) - 1..=(s.x + 128).div_euclid(256) + 1 {
                if !tg.drawable(d, (px, py)) {
                    continue;
                }
                let mask: Vec<bool> = pattern.iter().map(|&(ox, oy)| {
                    let (sx, sy) = (px * 256 + 128 + ox * 16, py * 256 + 128 + oy * 16);
                    s.x - 128 <= sx && sx < s.x + 128 && s.y - 128 <= sy && sy < s.y + 128
                }).collect();
                if mask.contains(&true) {
                    put_samples(m, tg.dev, d, tg.view, tg.fmt, tg.sub, tg.dsv, (px as usize, py as usize, tg.size.0 as usize), Some(z), out, &mask)?;
                }
            }
        }
        return Ok(());
    }
    let px = ((s.x - 1).div_euclid(256), (s.y - 1).div_euclid(256));
    if !tg.drawable(d, px) {
        return Ok(());
    }
    let z = tg.depth(z);
    let out = prim_out(ps, v.color, px, z, Some(q[3]), prim, true)?;
    tg.put(m, d, px, z, out)
}

/// what the programs write for a point or a line's pixel: its vertex's (or leading vertex's)
/// color, the pixel's position, or the primitive's index
/// (MEASURED, WARP: probe-top1-*, probe-top2-*) as for triangles: SV_Position is the pixel
/// center, the depth and the clip w; points and lines face front
fn prim_out(ps: &str, c: [f32; 4], (px, py): (i64, i64), z: f32, w: Option<f32>, prim: u32, front: bool) -> Res<[u32; 4]> {
    match ps {
        "ps" => Ok(c.map(f32::to_bits)),
        "psflat" => {
            hit(clause::D3DRASTER_FLAT);
            Ok(c.map(f32::to_bits))
        }
        "pspos" => {
            hit(clause::D3DRASTER_POS);
            let w = w.ok_or(Fail::Unmodeled(open::D3DRASTER_INTERP))?;
            if w != 1.0 {
                hit(clause::D3DRASTER_POSW);
            }
            Ok([px as f32 + 0.5, py as f32 + 0.5, z, w].map(f32::to_bits))
        }
        "psprim" => {
            hit(clause::D3DRASTER_PRIM);
            Ok([(prim as f32).to_bits(), if front { 1.0f32 } else { 0.0 }.to_bits(), 0, 1.0f32.to_bits()])
        }
        _ => Err(Fail::Unmodeled(open::D3DRASTER_TOPOLOGY)),
    }
}

/// a line of the draw, as the vertex shader leaves it
struct Seg {
    a: Vert,
    b: Vert,
    lead: Vert,
    /// SV_PrimitiveID, SV_IsFrontFace (the triangle's, for wireframe edges)
    prim: u32,
    front: bool,
}

/// a line clipped and snapped
struct Line {
    a: Sub,
    b: Sub,
    ca: [f32; 4],
    cb: [f32; 4],
    /// 1 / w at the ends
    iwa: f32,
    iwb: f32,
    lead: [f32; 4],
    prim: u32,
    front: bool,
    /// the clip w, where the ends share it
    w: Option<f32>,
    /// |dy| > |dx|: the y-major diamond (its right corner in)
    ymaj: bool,
    z: f32,
}

/// the diamond of half-diagonal 1/2 pixel, in 1/256
const DIAMOND: i128 = 128;

/// how lines are drawn, by the rasterizer state's MultisampleEnable and
/// AntialiasedLineEnable
#[derive(Clone, Copy)]
enum LineMode {
    /// neither: the diamond exit rule
    Aliased,
    /// MultisampleEnable (with or without the other): D3DRASTER.LINE.QUAD
    Quad,
}

impl LineMode {
    fn of(d: &Draw) -> Res<LineMode> {
        match (d.rast[8] != 0, d.rast[9] != 0) {
            (false, false) => Ok(LineMode::Aliased),
            (true, _) => {
                hit(clause::D3DRASTER_LINE_QUAD);
                Ok(LineMode::Quad)
            }
            // (alpha antialiased lines: not measured to the bit)
            (false, true) => Err(Fail::Unmodeled(open::D3DRASTER_TOPOLOGY)),
        }
    }
}

/// (MEASURED, WARP: probe-rt-ms2-edges, -ms4-, -ms8-, every pixel) the samples of a
/// multisampled target (quality 0) are at D3D11_STANDARD_MULTISAMPLE_PATTERN's positions, in
/// 1/16 pixel from the pixel center
fn sample_pattern(n: u32) -> Option<&'static [(i64, i64)]> {
    match n {
        2 => Some(&[(4, 4), (-4, -4)]),
        4 => Some(&[(-2, -6), (6, -2), (-6, 2), (2, 6)]),
        8 => Some(&[(1, -3), (-1, 3), (5, 1), (-3, -5), (-5, 5), (-7, -1), (3, 7), (7, -7)]),
        _ => None,
    }
}

/// half the width of a multisample line, in pixels
const QUAD_HALF: f32 = 0.7;

/// c inside the triangle t (either winding) by the top-left rule, in 1/256 pixel, y down
fn tri_covers(t: [(i64, i64); 3], c: (i64, i64)) -> bool {
    let cross = |a: (i64, i64), b: (i64, i64), c: (i64, i64)| -> i128 {
        let d = |p: i64, q: i64| i128::from(p) - i128::from(q);
        d(b.0, a.0) * d(c.1, a.1) - d(b.1, a.1) * d(c.0, a.0)
    };
    let t = match cross(t[0], t[1], t[2]).signum() {
        0 => return false,
        1 => t,
        _ => [t[0], t[2], t[1]],
    };
    // (a top edge runs right along y; a left edge goes up)
    let top_left = |a: (i64, i64), b: (i64, i64)| a.1 == b.1 && b.0 > a.0 || b.1 < a.1;
    (0..3).all(|e| {
        let (a, b) = (t[e], t[(e + 1) % 3]);
        let w = cross(a, b, c);
        w > 0 || w == 0 && top_left(a, b)
    })
}

impl Line {
    /// (MEASURED, WARP, by the Rasterization Rules' figure: probe-lines-*, probe-linestrip-*,
    /// probe-wire-*) p, scaled by s, in the diamond of the pixel whose center is c: its
    /// inside, its lower-left and lower-right edges and bottom corner, and for a y-major line
    /// also its right corner
    fn in_diamond(&self, p: (i128, i128), c: (i128, i128), s: i128) -> bool {
        let (dx, dy) = (p.0 - c.0 * s, p.1 - c.1 * s);
        let (d, r) = (dx.abs() + dy.abs(), DIAMOND * s);
        if d != r {
            return d < r;
        }
        dx == 0 && dy == r || dx == r && dy == 0 && self.ymaj || dy > 0 && dx != 0
    }

    /// some point of the line in the pixel's diamond
    fn meets(&self, c: (i128, i128)) -> bool {
        let (a, b) = ((i128::from(self.a.x), i128::from(self.a.y)), (i128::from(self.b.x), i128::from(self.b.y)));
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        // the chord's parameters t0 <= t <= t1 inside the closed diamond, as n / d (d > 0)
        let less = |p: (i128, i128), q: (i128, i128)| p.0 * q.1 < q.0 * p.1;
        let (mut t0, mut t1) = ((0i128, 1i128), (1i128, 1i128));
        for (u, v) in [(1i128, 1i128), (1, -1), (-1, 1), (-1, -1)] {
            let num = DIAMOND - (u * (a.0 - c.0) + v * (a.1 - c.1));
            let den = u * dx + v * dy;
            match den.signum() {
                0 if num < 0 => return false,
                0 => {}
                1 => {
                    if less((num, den), t1) {
                        t1 = (num, den);
                    }
                }
                _ => {
                    if less(t0, (-num, -den)) {
                        t0 = (-num, -den);
                    }
                }
            }
        }
        if less(t1, t0) {
            return false;
        }
        // (inside the open diamond somewhere along a chord of length, unless it runs along an
        // edge: then its middle tells; a chord of no length is one point)
        let mid = (t0.0 * t1.1 + t1.0 * t0.1, 2 * t0.1 * t1.1);
        [t0, t1, mid].iter().any(|&(n, s)| self.in_diamond((a.0 * s + dx * n, a.1 * s + dy * n), c, s))
    }

    /// the line lights the pixel: it exits the pixel's diamond going from a to b (it meets the
    /// diamond and b is not in it), or it ends in it as D3DRASTER.LINE.END lights
    fn lights(&self, c: (i128, i128)) -> bool {
        self.meets(c) && (!self.in_diamond((i128::from(self.b.x), i128::from(self.b.y)), c, 1) || self.end_lights(c))
    }

    /// (MEASURED, WARP: probe-lineend-*, 128 lines, and the ends of probe-lines-*,
    /// probe-linestrip-*, probe-wire-*) a line ending inside a pixel's diamond lights it when
    /// the line crosses the diamond's diagonal along the major axis (y = cy for an x-major
    /// line) inside the diamond, before the pixel center's major coordinate: there the line has
    /// passed the center along the minor axis
    fn end_lights(&self, c: (i128, i128)) -> bool {
        let (a, b) = ((i128::from(self.a.x), i128::from(self.a.y)), (i128::from(self.b.x), i128::from(self.b.y)));
        // (major and minor coordinates)
        let (am, an, bm, bn, cm, cn) = if self.ymaj { (a.1, a.0, b.1, b.0, c.1, c.0) } else { (a.0, a.1, b.0, b.1, c.0, c.1) };
        let (dm, dn) = (bm - am, bn - an);
        if dn == 0 {
            return false;
        }
        // (the crossing of the diagonal, (am - cm) dn + dm (cn - an) over dn, from cm)
        let n = (am - cm) * dn + dm * (cn - an);
        let inside = n.abs() < DIAMOND * dn.abs();
        // (at cm the minor coordinate minus cn is -n / dm: past it the way the line goes)
        let passed = -n * dn.signum() * dm.signum() > 0;
        hit(clause::D3DRASTER_LINE_END);
        inside && passed
    }

    /// (MEASURED, WARP: probe-lines-ms, -msaa, 84 pixels) with the multisample state a line
    /// is the rectangle 1.4 pixels wide along it, ending at its ends: its corners a +- n,
    /// b +- n (n the normal of length 0.7, in f32) snapped, as two triangles a + n, b + n,
    /// b - n and a + n, b - n, a - n by the top-left rule; the values are the covering
    /// triangle's planes (D3DRASTER.INTERP.PLANE, not turned) over the snapped corners, a's
    /// values at a +- n and b's at b +- n (84 UNORM pixels; probe-linesms-*, 220 float values)
    fn quad(&self, (px, py): (i64, i64)) -> Option<Planes> {
        let (ax, ay, bx, by) = (self.a.x as f32 / 256.0, self.a.y as f32 / 256.0, self.b.x as f32 / 256.0, self.b.y as f32 / 256.0);
        let (dx, dy) = (bx - ax, by - ay);
        let k = QUAD_HALF / (dx * dx + dy * dy).sqrt();
        let (nx, ny) = (-dy * k, dx * k);
        let q = |x: f32, y: f32| (snap256(x), snap256(y));
        let corner = [(q(ax + nx, ay + ny), false), (q(bx + nx, by + ny), true), (q(bx - nx, by - ny), true), (q(ax - nx, ay - ny), false)];
        let c = (px * 256 + 128, py * 256 + 128);
        let t = [[0, 1, 2], [0, 2, 3]].into_iter().map(|t| t.map(|i| corner[i])).find(|t| tri_covers(t.map(|v| v.0), c))?;
        // (MEASURED, WARP: probe-linesms-*, 220 float values) the planes start at a + n, as
        // the triangles are given, not turned
        let iw = t.map(|v| if v.1 { self.iwb } else { self.iwa });
        let aw = [0, 1, 2].map(|i| (if t[i].1 { self.cb } else { self.ca }).map(|a| a * iw[i]));
        Some(Planes { xs: t.map(|v| v.0 .0 as f32 / 256.0), ys: t.map(|v| v.0 .1 as f32 / 256.0), iw, aw })
    }

    /// (MEASURED, WARP: probe-lines-*, probe-linestrip-*, probe-wire-*, 904 pixels;
    /// probe-linesw-*, 114 pixels) the color along the line, perspective-correct: a plane over
    /// the major axis for 1/w and for each channel's c/w, g = 1 / (b - a), the gradient
    /// (vb - va) g, C = va - gradient a, the value C + gradient c at the pixel center, in f32;
    /// the channel's value times 1 / (the value of 1/w)
    fn color(&self, (px, py): (i64, i64)) -> [f32; 4] {
        let (a, b, c) = if self.ymaj { (self.a.y, self.b.y, py as f32 + 0.5) } else { (self.a.x, self.b.x, px as f32 + 0.5) };
        let (a, b) = (a as f32 / 256.0, b as f32 / 256.0);
        let g = 1.0 / (b - a);
        let plane = |va: f32, vb: f32| {
            let grad = (vb - va) * g;
            let c0 = va - grad * a;
            c0 + grad * c
        };
        let q = 1.0 / plane(self.iwa, self.iwb);
        [0, 1, 2, 3].map(|k| plane(self.ca[k] * self.iwa, self.cb[k] * self.iwb) * q)
    }
}

/// a line clipped to -w <= x <= w and -w <= y <= w in that order, in f32: the end outside a
/// plane moves to a + (b - a) t, t = da / (da - db), its color with it; None when wholly out
fn clip_line(mut a: Vert, mut b: Vert) -> Option<(Vert, Vert)> {
    for plane in 0..4 {
        let dist = |q: &Vert| match plane {
            0 => q.pos[3] + q.pos[0],
            1 => q.pos[3] - q.pos[0],
            2 => q.pos[3] + q.pos[1],
            _ => q.pos[3] - q.pos[1],
        };
        let (da, db) = (dist(&a), dist(&b));
        match (da >= 0.0, db >= 0.0) {
            (false, false) => return None,
            (true, true) => continue,
            (ina, _) => {
                let t = da / (da - db);
                let lerp = |p: [f32; 4], q: [f32; 4]| [0, 1, 2, 3].map(|k| p[k] + (q[k] - p[k]) * t);
                let v = Vert { pos: lerp(a.pos, b.pos), color: lerp(a.color, b.color) };
                if ina {
                    b = v;
                } else {
                    a = v;
                }
            }
        }
    }
    Some((a, b))
}

/// the lines of one draw (line lists and strips, the edges of wireframe triangles), in order
fn lines(m: &mut D3dres, d: &Draw, tg: &Target, ps: &str, segs: &[Seg]) -> Res<()> {
    if segs.is_empty() {
        return Ok(());
    }
    hit(clause::D3DRASTER_LINE);
    let mode = LineMode::of(d)?;
    let mut ls: Vec<Line> = Vec::new();
    for s in segs {
        drawable_w(&s.a)?;
        drawable_w(&s.b)?;
        let (za, zb) = (s.a.pos[2] / s.a.pos[3], s.b.pos[2] / s.b.pos[3]);
        let inside = |z: f32| (0.0..=1.0).contains(&z);
        if d.rast[6] != 0 && !(inside(za) && inside(zb)) {
            if za < 0.0 && zb < 0.0 || za > 1.0 && zb > 1.0 {
                hit(clause::D3DRASTER_DEPTHCLIP);
                continue;
            }
            return Err(Fail::Unmodeled(open::D3DRASTER_CLIPZ));
        }
        if za != zb {
            return Err(Fail::Unmodeled(open::D3DRASTER_INTERP));
        }
        let Some((a, b)) = clip_line(s.a, s.b) else {
            hit(clause::D3DRASTER_CLIP);
            continue;
        };
        let (pa, pb) = (snapped(tg.vp, &a.pos), snapped(tg.vp, &b.pos));
        if pa == pb {
            // (no length: it exits no diamond)
            hit(clause::D3DRASTER_DEGENERATE);
            continue;
        }
        let ymaj = (pb.y - pa.y).abs() > (pb.x - pa.x).abs();
        ls.push(Line { a: pa, b: pb, ca: a.color, cb: b.color, iwa: 1.0 / a.pos[3], iwb: 1.0 / b.pos[3], lead: s.lead.color, prim: s.prim, front: s.front, w: (a.pos[3] == b.pos[3]).then_some(a.pos[3]), ymaj, z: tg.depth(za) });
    }
    let center = |(px, py): (i64, i64)| (i128::from(px) * 256 + 128, i128::from(py) * 256 + 128);
    for l in &ls {
        let (x0, x1) = (l.a.x.min(l.b.x).div_euclid(256) - 1, l.a.x.max(l.b.x).div_euclid(256) + 1);
        let (y0, y1) = (l.a.y.min(l.b.y).div_euclid(256) - 1, l.a.y.max(l.b.y).div_euclid(256) + 1);
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(i64::from(tg.size.0) - 1), y1.min(i64::from(tg.size.1) - 1));
        for py in y0..=y1 {
            for px in x0..=x1 {
                // (a multisample line's triangle there)
                let quad = match mode {
                    LineMode::Aliased if l.lights(center((px, py))) => None,
                    LineMode::Aliased => continue,
                    LineMode::Quad => match l.quad((px, py)) {
                        Some(p) => Some(p),
                        None => continue,
                    },
                };
                if !tg.drawable(d, (px, py)) {
                    continue;
                }
                let out = match (ps, quad) {
                    ("ps", None) => {
                        hit(clause::D3DRASTER_LINE_COLOR);
                        l.color((px, py)).map(f32::to_bits)
                    }
                    ("ps", Some(p)) => p.at(px as f32 + 0.5, py as f32 + 0.5).map(f32::to_bits),
                    _ => prim_out(ps, l.lead, (px, py), l.z, l.w, l.prim, l.front)?,
                };
                tg.put(m, d, (px, py), l.z, out)?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn triangle(m: &mut D3dres, dev: Dev, d: &Draw, vp: [f32; 6], ps: &str, tri: &[Vert; 3], lead: Vert, prim: u32, (rw, rh): (u32, u32), samples: Option<&[(i64, i64)]>, view: &super::d3dres::View, rt_fmt: &'static Format, rt_sub: u32, dsv: Option<&super::d3dres::View>, wire: &mut Vec<Seg>) -> Res<()> {
    // (every vertex at w <= 0: wholly outside the clip volume, or (w = 0, so x = y = 0 inside
    // it) a point: nothing drawn, probe-vb-past)
    if tri.iter().all(|v| v.pos[3] <= 0.0 && v.pos.iter().all(|x| x.is_finite()) && (v.pos[3] < 0.0 || v.pos[0] == 0.0 && v.pos[1] == 0.0)) {
        hit(clause::D3DRASTER_CLIP);
        return Ok(());
    }
    if tri.iter().any(|v| v.pos.iter().any(|x| !x.is_finite())) {
        return Err(Fail::Unmodeled(open::D3DRASTER_CLIPW));
    }
    if tri.iter().any(|v| !(v.pos[3] > 0.0)) {
        // (MEASURED, WARP: probe-clip-w) a vertex at w <= 0: the clipping to the viewport
        // planes in homogeneous coordinates takes it away; what is left with no area draws
        // nothing
        let poly = clip_xy(tri.map(|v| v.pos));
        if poly.iter().all(|q| q[3] > 0.0) {
            let q: Vec<Sub> = poly.iter().map(|q| snapped(vp, q)).collect();
            let area = |a: Sub, b: Sub, c: Sub| (i128::from(b.x) - i128::from(a.x)) * (i128::from(c.y) - i128::from(a.y)) - (i128::from(b.y) - i128::from(a.y)) * (i128::from(c.x) - i128::from(a.x));
            if (1..q.len().saturating_sub(1)).all(|i| area(q[0], q[i], q[i + 1]) == 0) {
                hit(clause::D3DRASTER_CLIPW_AWAY);
                return Ok(());
            }
        }
        // (MEASURED, WARP: probe-clip-w2) what is left with an area: the pieces of the
        // homogeneous clipping (then 0 <= z <= w with depth clipping on), snapped and fanned
        // as any clipped triangle; drawn here where nothing tells the pixels apart (one color,
        // no culling, no depth, one sample)
        let poly = if d.rast[6] != 0 { clip_z(poly) } else { poly };
        let flat = tri.iter().all(|v| v.color == tri[0].color) && matches!(ps, "ps" | "psflat");
        if poly.iter().all(|q| q[3] > 0.0) && flat && d.rast[1] == 1 && d.rast[0] != 2 && dsv.is_none() && samples.is_none() {
            hit(clause::D3DRASTER_CLIPW_AREA);
            let q: Vec<Sub> = poly.iter().map(|q| snapped(vp, q)).collect();
            let q: Vec<(i64, i64)> = q.iter().map(|s| (s.x, s.y)).collect();
            let out = tri[0].color.map(f32::to_bits);
            let [vx, vy, vw, vh, _, _] = vp;
            for py in 0..i64::from(rh) {
                for px in 0..i64::from(rw) {
                    let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                    if cx < vx || cx >= vx + vw || cy < vy || cy >= vy + vh || !(d.rast[7] == 0 || px >= d.scissor[0] && px < d.scissor[2] && py >= d.scissor[1] && py < d.scissor[3]) {
                        continue;
                    }
                    let c = (px * 256 + 128, py * 256 + 128);
                    let lit = (1..q.len().saturating_sub(1)).any(|i| tri_covers([q[0], q[i], q[i + 1]], c));
                    if lit {
                        output(m, dev, d, view, rt_fmt, rt_sub, None, (px as usize, py as usize, rw as usize), None, out)?;
                    }
                }
            }
            return Ok(());
        }
        return Err(Fail::Unmodeled(open::D3DRASTER_CLIPW));
    }
    // depth clipping: whole triangles in these cases
    let zs: Vec<f32> = tri.iter().map(|v| v.pos[2] / v.pos[3]).collect();
    let inside = |z: f32| (0.0..=1.0).contains(&z);
    let mut zclip = false;
    if d.rast[6] != 0 && !zs.iter().all(|z| inside(*z)) {
        if zs.iter().all(|z| *z < 0.0) || zs.iter().all(|z| *z > 1.0) {
            hit(clause::D3DRASTER_DEPTHCLIP);
            return Ok(());
        }
        // (the depth of what is left is not measured: only where nothing reads it)
        if dsv.is_some() || ps == "pspos" {
            return Err(Fail::Unmodeled(open::D3DRASTER_CLIPZ));
        }
        zclip = true;
    }
    // the viewport transform, then 16.8 fixed point
    let [vx, vy, vw, vh, minz, maxz] = vp;
    let to_screen = |q: &[f32; 4]| (vx + (q[0] / q[3] + 1.0) * (vw * 0.5), vy + (1.0 - q[1] / q[3]) * (vh * 0.5));
    hit(clause::D3DRASTER_SNAP);
    let snap = |v: f32| -> i64 {
        // (2^31 pixels: past any guard band; the products below stay within i128)
        let x = f64::from(v).clamp(-2147483648.0, 2147483648.0) * 256.0;
        let f = x.floor();
        let r = x - f;
        (if r > 0.5 || r == 0.5 && (f as i64) % 2 != 0 { f + 1.0 } else { f }) as i64
    };
    let snap2 = |q: &[f32; 4]| {
        let (x, y) = to_screen(q);
        (snap(x), snap(y))
    };
    let p: Vec<(i64, i64)> = tri.iter().map(|v| snap2(&v.pos)).collect();
    let cross = |a: (i64, i64), b: (i64, i64), c: (i64, i64)| -> i128 {
        let d = |p: i64, q: i64| i128::from(p) - i128::from(q);
        d(b.0, a.0) * d(c.1, a.1) - d(b.1, a.1) * d(c.0, a.0)
    };
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
    // (MEASURED, WARP: probe-wire-*, probe-wireframe) wireframe: the triangle's edges as
    // lines, from the vertex of least clip x on: 0 to 1, 1 to 2, 2 to 0
    if d.rast[0] == 2 {
        hit(clause::D3DRASTER_WIREFRAME);
        let k0 = (0..3).fold(0, |k, i| if tri[i].pos[0] < tri[k].pos[0] { i } else { k });
        let t: [Vert; 3] = [0, 1, 2].map(|i| tri[(k0 + i) % 3]);
        for e in 0..3 {
            wire.push(Seg { a: t[e], b: t[(e + 1) % 3], lead, prim, front });
        }
        return Ok(());
    }
    let v = if cw { [p[0], p[1], p[2]] } else { [p[0], p[2], p[1]] };
    let top_left = |a: (i64, i64), b: (i64, i64)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        dy < 0 || dy == 0 && dx > 0
    };
    // (MEASURED, WARP: a triangle reaching past the viewport is clipped to it first, in clip
    // space, and the pieces' new vertices snapped as any other, tri-2: so an edge moves by up
    // to the snapping of its new end)
    let poly = clip_xy(tri.map(|v| v.pos));
    let poly = if zclip {
        hit(clause::D3DRASTER_CLIPZ_PIECES);
        clip_z(poly)
    } else {
        poly
    };
    let unclipped = poly.len() == 3 && poly.iter().zip(tri.iter()).all(|(a, b)| *a == b.pos);
    let planes = unclipped.then(|| Planes::new(tri, |q| {
        let (x, y) = snap2(q);
        (x as f32 / 256.0, y as f32 / 256.0)
    }));
    let pieces: Vec<[(i64, i64); 3]> = if unclipped {
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
            Some(zz[0].max(lo).min(hi))
        }
    };
    // the pixels: inside the target, the viewport and (when on) the scissor rectangle
    hit(clause::D3DRASTER_VPCLIP);
    // (the float casts saturate; the sums are kept from overflowing)
    let edge = |v: f32| v.ceil() as i64;
    let mut x0 = 0i64.max(edge(vx).saturating_sub(1));
    let mut y0 = 0i64.max(edge(vy).saturating_sub(1));
    let mut x1 = i64::from(rw).min(edge(vx + vw).saturating_add(1));
    let mut y1 = i64::from(rh).min(edge(vy + vh).saturating_add(1));
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
            let inside = |c: (i64, i64)| {
                pieces.iter().any(|t| {
                    (0..3).all(|e| {
                        let (a, b) = (t[e], t[(e + 1) % 3]);
                        let w = cross(a, b, c);
                        w > 0 || w == 0 && top_left(a, b)
                    })
                })
            };
            let at = (px as usize, py as usize, rw as usize);
            let Some(pattern) = samples else {
                if !inside((px * 256 + 128, py * 256 + 128)) {
                    continue;
                }
                hit(clause::D3DRASTER_TOPLEFT);
                let out = shade(m, ps, tri, planes.as_ref(), lead, prim, front, (px, py), z, d)?;
                output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, out)?;
                continue;
            };
            // (MEASURED, WARP: probe-rt-ms*-edges) a pixel is shaded once, at its center,
            // when a sample is covered; the covered samples take the value
            hit(clause::D3DRASTER_MS_SAMPLES);
            let mask: Vec<bool> = pattern.iter().map(|&(ox, oy)| inside((px * 256 + 128 + ox * 16, py * 256 + 128 + oy * 16))).collect();
            if !mask.contains(&true) {
                continue;
            }
            let out = shade(m, ps, tri, planes.as_ref(), lead, prim, front, (px, py), z, d)?;
            put_samples(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, out, &mask)?;
        }
    }
    Ok(())
}

/// a pixel's value into the covered samples of a multisampled target (D3DRASTER.MS.SAMPLES):
/// all of them while every sample is alike, else per sample, each through the pixel's own
/// output (its value put in the subresource's place for the blend, then taken back)
#[allow(clippy::too_many_arguments)]
fn put_samples(m: &mut D3dres, dev: Dev, d: &Draw, view: &super::d3dres::View, rt_fmt: &'static Format, rt_sub: u32, dsv: Option<&super::d3dres::View>, at: (usize, usize, usize), z: Option<f32>, out: [u32; 4], mask: &[bool]) -> Res<()> {
    let (px, py) = (at.0, at.1);
    let r = m.res[view.res].as_ref().unwrap();
    if mask.iter().all(|&k| k) && r.samples.get(rt_sub as usize).is_none_or(Option::is_none) {
        output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, out)?;
        return Ok(());
    }
    // (samples that differ: depth per sample is not measured)
    if dsv.is_some() {
        return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
    }
    let (rb, _, _) = r.layout(rt_sub);
    let el = (rt_fmt.bits / 8) as usize;
    let off = py as usize * rb + px as usize * el;
    let r = m.res[view.res].as_mut().unwrap();
    let base = r.subs[rt_sub as usize].clone().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
    let n = mask.len();
    // (the model keeps samples of targets up to 64 MiB of them)
    if base.len().saturating_mul(n) > 64 << 20 {
        return Err(Fail::Unmodeled(open::D3DRASTER_RTKIND));
    }
    if r.samples.len() <= rt_sub as usize {
        r.samples.resize(rt_sub as usize + 1, None);
    }
    let planes_n = r.samples[rt_sub as usize].get_or_insert_with(|| base.repeat(n)).len() / n;
    // (each covered sample through the pixel's own output: its value put in the
    // subresource's place for the blend, then taken back)
    for (k, _) in mask.iter().enumerate().filter(|(_, c)| **c) {
        let r = m.res[view.res].as_mut().unwrap();
        let sv = r.samples[rt_sub as usize].as_ref().unwrap()[k * planes_n + off..][..el].to_vec();
        r.subs[rt_sub as usize].as_mut().unwrap()[off..off + el].copy_from_slice(&sv);
        output(m, dev, d, view, rt_fmt, rt_sub, None, at, z, out)?;
        let r = m.res[view.res].as_mut().unwrap();
        let nv = r.subs[rt_sub as usize].as_ref().unwrap()[off..off + el].to_vec();
        r.samples[rt_sub as usize].as_mut().unwrap()[k * planes_n + off..][..el].copy_from_slice(&nv);
    }
    // (the subresource's own place keeps sample 0)
    let r = m.res[view.res].as_mut().unwrap();
    let s0 = r.samples[rt_sub as usize].as_ref().unwrap()[off..off + el].to_vec();
    r.subs[rt_sub as usize].as_mut().unwrap()[off..off + el].copy_from_slice(&s0);
    Ok(())
}

/// (MEASURED, WARP: probe-interp-affine-*, probe-interp-persp-*, every pixel bit for bit) a
/// triangle's perspective-correct color: the vertices turned so the least clip x comes first,
/// each at its snapped screen position; a plane per attribute a, with
/// A = (x1 - x0)(y2 - y0) - (y1 - y0)(x2 - x0), gx = (d1 (y2 - y0) - d2 (y1 - y0)) (1 / A),
/// gy = (d2 (x1 - x0) - d1 (x2 - x0)) (1 / A), C = (a0 - gx x0) - gy y0 and the value at a pixel
/// center (C + gy cy) + gx cx, all in f32; planes of 1/w and of a (1/w), the color their quotient
/// a/w times 1 / (1/w)
struct Planes {
    xs: [f32; 3],
    ys: [f32; 3],
    iw: [f32; 3],
    aw: [[f32; 4]; 3],
}

impl Planes {
    fn new(tri: &[Vert; 3], screen: impl Fn(&[f32; 4]) -> (f32, f32)) -> Planes {
        // (the last of the least clip x, as a stable rotation: MEASURED, WARP, probe-filter-float,
        // -unorm16, where two share it)
        let k0 = (0..3).fold(0, |k, i| if tri[i].pos[0] <= tri[k].pos[0] { i } else { k });
        let t: [&Vert; 3] = [0, 1, 2].map(|i| &tri[(k0 + i) % 3]);
        let s = t.map(|v| screen(&v.pos));
        let iw = t.map(|v| 1.0 / v.pos[3]);
        let aw = [0, 1, 2].map(|i| t[i].color.map(|a| a * iw[i]));
        Planes { xs: s.map(|p| p.0), ys: s.map(|p| p.1), iw, aw }
    }

    fn plane(&self, a: [f32; 3], cx: f32, cy: f32) -> f32 {
        let ([x0, x1, x2], [y0, y1, y2]) = (self.xs, self.ys);
        let area = (x1 - x0) * (y2 - y0) - (y1 - y0) * (x2 - x0);
        let r = 1.0 / area;
        let (d1, d2) = (a[1] - a[0], a[2] - a[0]);
        let gx = (d1 * (y2 - y0) - d2 * (y1 - y0)) * r;
        let gy = (d2 * (x1 - x0) - d1 * (x2 - x0)) * r;
        let c = (a[0] - gx * x0) - gy * y0;
        (c + gy * cy) + gx * cx
    }

    fn at(&self, cx: f32, cy: f32) -> [f32; 4] {
        let q = 1.0 / self.plane(self.iw, cx, cy);
        [0, 1, 2, 3].map(|k| self.plane(self.aw.map(|a| a[k]), cx, cy) * q)
    }
}

/// the filters of D3D11_FILTER the model samples with: point or linear for minifying and for
/// magnifying (the mip filter does not matter with one level)
struct Filter {
    min: bool,
    mag: bool,
}

impl Filter {
    fn of(f: u32) -> Option<Filter> {
        match f {
            0x00 | 0x01 => Some(Filter { min: false, mag: false }),
            0x04 | 0x05 => Some(Filter { min: false, mag: true }),
            0x10 | 0x11 => Some(Filter { min: true, mag: false }),
            0x14 | 0x15 => Some(Filter { min: true, mag: true }),
            // (anisotropic: as linear where it magnifies, D3DRASTER.FILTER.ANISO)
            0x55 => Some(Filter { min: true, mag: true }),
            _ => None,
        }
    }
}

/// (MEASURED, WARP: probe-filter-*, 2425 values) a bilinear result n in 1/128 of a byte (n of
/// 32640 for 1.0) as a float: the f32 nearest n ceil(2^46 / 32640) / 2^46, a hair above
/// n / 32640 (seven values at 0.498 ulp past an f32 round up)
/// a + (b - a) t as SSE computes it, NaNs included: an operation's result is its first operand
/// when that is a NaN, else its second, quieted (MEASURED, WARP: probe-filter-r11g11b10, 72
/// NaN values of differing payloads)
fn lerp_sse(a: f32, b: f32, t: f32) -> f32 {
    let op = |x: f32, y: f32, r: f32| {
        let quiet = |n: f32| f32::from_bits(n.to_bits() | 0x40_0000);
        if x.is_nan() { quiet(x) } else if y.is_nan() { quiet(y) } else { r }
    };
    let d = op(b, a, b - a);
    let m = op(d, t, d * t);
    op(a, m, a + m)
}

/// the level or levels a sample reads
#[derive(Clone, Copy)]
enum Level {
    One(u32),
    /// between a level and the next, the weight of the next in 1/256
    Two(u32, i64),
}

/// a level's filtered value before it is output
enum Filtered {
    Point([u32; 4]),
    Wide([f32; 4]),
    /// 8-bit UNORM: the value in 1/32640
    Fixed([i64; 4]),
}

fn linear_float(n: i64) -> f32 {
    const M: i64 = (1 << 46) / 32640 + 1;
    const _: () = assert!(M * 32640 > 1 << 46 && (M - 1) * 32640 < 1 << 46);
    ((n * M) as f64 / (1u64 << 46) as f64) as f32
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

/// a polygon clipped to 0 <= z <= w, as clip_xy
fn clip_z(mut poly: Vec<[f32; 4]>) -> Vec<[f32; 4]> {
    for plane in 0..2 {
        let dist = |q: &[f32; 4]| if plane == 0 { q[2] } else { q[3] - q[2] };
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
fn shade(m: &D3dres, ps: &str, tri: &[Vert; 3], planes: Option<&Planes>, lead: Vert, prim: u32, front: bool, (px, py): (i64, i64), z: Option<f32>, d: &Draw) -> Res<[u32; 4]> {
    let same = |f: &dyn Fn(&Vert) -> [f32; 4]| -> Res<[f32; 4]> {
        let a = f(&tri[0]);
        if tri.iter().all(|v| f(v) == a) {
            Ok(a)
        } else {
            Err(Fail::Unmodeled(open::D3DRASTER_INTERP))
        }
    };
    Ok(match ps {
        "ps" => match planes {
            Some(p) => {
                hit(clause::D3DRASTER_INTERP_PLANE);
                p.at(px as f32 + 0.5, py as f32 + 0.5).map(f32::to_bits)
            }
            None => same(&|v| v.color)?.map(f32::to_bits),
        },
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
            // (MEASURED, WARP: probe-sample-nosampler) no sampler bound: the default one,
            // linear filtering, clamped addresses
            let s = d.sampler.unwrap_or_else(|| {
                hit(clause::D3DRASTER_NOSAMPLER);
                [0x15, 3, 3, 3]
            });
            let v = m.views[sv].as_ref().unwrap();
            let r = m.res[v.res].as_ref().unwrap();
            let (w, h, _) = r.size(0);
            let uv_at = |x: i64, y: i64| -> Res<[f32; 2]> {
                match planes {
                    Some(p) => {
                        let c = p.at(x as f32 + 0.5, y as f32 + 0.5);
                        Ok([c[0], c[1]])
                    }
                    None => same(&|v| v.color).map(|c| [c[0], c[1]]),
                }
            };
            let uv = uv_at(px, py)?;
            let filter = Filter::of(s[0]).ok_or(Fail::Unmodeled(open::D3DRASTER_FILTER))?;
            // (the minifying and magnifying filters differ: which applies is by the texels a
            // pixel spans, as the 2 x 2 quad's differences give them)
            // (a view of its own range of levels: not measured)
            if r.d.mips > 1 && !(v.words.get(1) == Some(&4) && v.words.get(2) == Some(&0) && v.words.get(3).is_some_and(|&n| n == r.d.mips || n == u32::MAX)) {
                return Err(Fail::Unmodeled(open::D3DRASTER_FILTER));
            }
            let aniso = s[0] == 0x55;
            let (linear, level) = if filter.min == filter.mag && r.d.mips == 1 && !aniso {
                (filter.mag, Level::One(0))
            } else {
                let (qx, qy) = (px & !1, py & !1);
                let q = [uv_at(qx, qy)?, uv_at(qx + 1, qy)?, uv_at(qx, qy + 1)?];
                let span = |a: [f32; 2], b: [f32; 2]| ((f64::from(b[0] - a[0]) * w as f64).powi(2) + (f64::from(b[1] - a[1]) * h as f64).powi(2)).sqrt();
                let (sx, sy) = (span(q[0], q[1]), span(q[0], q[2]));
                let rho = sx.max(sy);
                // (MEASURED, WARP: probe-filter-min, -min-point, -rho-*) a span of up to one texel
                // magnifies, past it minifies (1 - 2^-10, 1, 1 + 2^-10, 1 + 2^-6, 1.1, 1.25); with
                // more than one mip level the level is not measured
                if rho <= 1.0 {
                    hit(clause::D3DRASTER_FILTER_MINMAG);
                    if aniso {
                        // (MEASURED, WARP: probe-filter-aniso) anisotropic filtering where a quad
                        // spans a texel or less is the linear filter
                        hit(clause::D3DRASTER_FILTER_ANISO);
                    }
                    (filter.mag, Level::One(0))
                } else if aniso && (sx != sy || r.d.mips > 1) {
                    // (a footprint longer one way than the other, or a texture with mips: not
                    // measured)
                    return Err(Fail::Unmodeled(open::D3DRASTER_FILTER));
                } else if r.d.mips == 1 {
                    // (MEASURED, WARP: probe-filter-aniso-min) anisotropic filtering over a square
                    // footprint is the linear filter
                    if aniso {
                        hit(clause::D3DRASTER_FILTER_ANISO);
                    }
                    hit(clause::D3DRASTER_FILTER_MINMAG);
                    (filter.min, Level::One(0))
                } else {
                    // (MEASURED, WARP: probe-filter-mips) past one texel the level is log2 of the
                    // span: a whole level of it samples that level alone (mip point or linear);
                    // between levels not measured
                    let lod = rho.log2();
                    let last = f64::from(r.d.mips - 1);
                    let level = if s[0] & 1 == 0 {
                        // (mip point: the nearest level)
                        Level::One(((lod + 0.5).floor().min(last)) as u32)
                    } else if lod.fract() == 0.0 || lod >= last {
                        Level::One(lod.floor().min(last) as u32)
                    } else {
                        Level::Two(lod.floor() as u32, (lod.fract() * 256.0).round() as i64)
                    };
                    hit(clause::D3DRASTER_FILTER_LEVEL);
                    (filter.min, level)
                }
            };
            let one = |level: u32| -> Res<Filtered> {
                let (w, h, _) = r.size(level);
                let at = |i: i64, j: i64| -> Res<[u32; 4]> {
                    match (address(s[1], i, w as i64), address(s[2], j, h as i64)) {
                        (Some(x), Some(y)) => texel(r, level, v.format(), x as u32, y as u32),
                        _ => {
                            hit(clause::D3DRASTER_BORDER);
                            Ok([0; 4])
                        }
                    }
                };
                // (MEASURED, WARP: probe-filter-*, every pixel) point: the texel floor(u w),
                // floor(v h), the products in f32
                if !linear {
                    hit(clause::D3DRASTER_FILTER_POINT);
                    return at((uv[0] * w as f32).floor() as i64, (uv[1] * h as f32).floor() as i64).map(Filtered::Point);
                }
                hit(clause::D3DRASTER_FILTER_LINEAR);
                let split = |c: f32, n: u32| {
                    let s = c * n as f32 - 0.5;
                    let i = s.floor();
                    (i as i64, ((s - i) * 256.0).floor() as i64)
                };
                let ((i, wu), (j, wv)) = (split(uv[0], w), split(uv[1], h));
                // (MEASURED, WARP: probe-filter-float, -unorm16, -r10, -srgb, -snorm, every value) float
                // texels, 16-bit and 10-10-10-2 UNORM, sRGB (linear) and SNORM ones (as their floats): in f32, between the rows first, l = a + (c -
                // a) fv, r = b + (d - b) fv, then l + (r - l) fu, with the weights fu, fv the
                // 1/256 below the fraction
                let wide = format(v.format()).is_some_and(|f| matches!(f.num, Num::Float | Num::Srgb | Num::Snorm) && f.comps.iter().all(|c| matches!(c, 8 | 16 | 32)) || f.num == Num::Unorm && (f.comps.iter().all(|&c| c == 16) || f.comps == [10, 10, 10, 2]) || matches!(v.format(), 26 | 67 | 85 | 86 | 115));
                // (and R11G11B10, R9G9B9E5, 5-6-5, 5-5-5-1 and 4-4-4-4 UNORM: probe-filter-r11g11b10, -rgb9e5, -b5g6r5, -b5g5r5a1, -b4g4r4a4;
                // NaNs as SSE passes them on, D3DRASTER.FILTER.FLOAT)
                if wide {
                    hit(clause::D3DRASTER_FILTER_FLOAT);
                    let (fu, fv) = (wu as f32 / 256.0, wv as f32 / 256.0);
                    let f = |t: [u32; 4]| t.map(f32::from_bits);
                    let (a, b, c, e) = (f(at(i, j)?), f(at(i + 1, j)?), f(at(i, j + 1)?), f(at(i + 1, j + 1)?));
                    return Ok(Filtered::Wide([0, 1, 2, 3].map(|k| {
                        let l = lerp_sse(a[k], c[k], fv);
                        let r = lerp_sse(b[k], e[k], fv);
                        lerp_sse(l, r, fu)
                    })));
                }
                if !matches!(v.format(), 28 | 49 | 61 | 65 | 87 | 88) {
                    return Err(Fail::Unmodeled(open::D3DRASTER_FILTER));
                }
                let bytes = |t: [u32; 4]| t.map(|b| (f32::from_bits(b) * 255.0).round() as i64);
                let (a, b, c, e) = (bytes(at(i, j)?), bytes(at(i + 1, j)?), bytes(at(i, j + 1)?), bytes(at(i + 1, j + 1)?));
                Ok(Filtered::Fixed([0, 1, 2, 3].map(|k| {
                    let l = (a[k] * (256 - wv) + c[k] * wv) >> 1;
                    let r = (b[k] * (256 - wv) + e[k] * wv) >> 1;
                    (l * (256 - wu) + r * wu) >> 8
                })))
            };
            let done = |t: Filtered| match t {
                Filtered::Point(t) => t,
                Filtered::Wide(t) => t.map(f32::to_bits),
                Filtered::Fixed(t) => t.map(|n| linear_float(n).to_bits()),
            };
            let mip_linear = |t: [u32; 4]| {
                hit(clause::D3DRASTER_FILTER_MIPLINEAR);
                t
            };
            match level {
                Level::One(l) => done(one(l)?),
                // (MEASURED, WARP: probe-filter-trilinear*) between two levels; point texels blend
                // in f32, 8-bit UNORM too
                Level::Two(l, f) => match (one(l)?, one(l + 1)?) {
                    (Filtered::Fixed(a), Filtered::Fixed(b)) => mip_linear(done(Filtered::Fixed([0, 1, 2, 3].map(|k| (a[k] * (256 - f) + b[k] * f) >> 8)))),
                    (Filtered::Wide(a), Filtered::Wide(b)) => mip_linear(done(Filtered::Wide([0, 1, 2, 3].map(|k| lerp_sse(a[k], b[k], f as f32 / 256.0))))),
                    (Filtered::Point(a), Filtered::Point(b)) => {
                        let (a, b) = (a.map(f32::from_bits), b.map(f32::from_bits));
                        mip_linear(done(Filtered::Wide([0, 1, 2, 3].map(|k| lerp_sse(a[k], b[k], f as f32 / 256.0)))))
                    }
                    (Filtered::Point(_) | Filtered::Wide(_) | Filtered::Fixed(_), _) => return Err(Fail::Unmodeled(open::D3DRASTER_FILTER)),
                },
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
        // (a depth view smaller than the target: not measured)
        let e = b.get_mut(y * rb + x * el..).and_then(|r| r.get_mut(..el)).ok_or(Fail::Unmodeled(open::D3DRASTER_RTKIND))?;
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
    // (a view whose elements are wider than its resource's, as mutations make: not measured)
    let e = b.get_mut(y * rb + x * el..).and_then(|r| r.get_mut(..el)).ok_or(Fail::Unmodeled(open::D3DRASTER_RTKIND))?;
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
