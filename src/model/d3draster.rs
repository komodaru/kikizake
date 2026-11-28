//! Drawing on Direct3D 10 and 11: the input assembler (one vertex buffer, triangle lists and
//! strips, instances), the viewport transform, the rasterizer (16.8 fixed point vertices, pixel
//! centers, the top-left rule, culling, scissor), the fixed shader programs
//! (src/shaders.rs, identified here by their bytes), depth tests and blending.
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
use super::d3drules::{self as rules, covers as tri_covers, sample_pattern, sample_sub, DepthAction, Pick, TargetPlan, Topo, Verdict};
use super::d3dres::{slot, texel, D3dres, Dev, Resource, View, E_INVALIDARG};
use super::objects::{Id, Objects};
use super::dxgiconv::{decode_f, encode, encode_bits, to_unorm};
use super::dxgifmt::{format, Format, Num};
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::shaders::{Prog, Programs};
use crate::spec::hit;

crate::model::words! {
    /// the drawing commands (d3dres routes them here)
    pub enum DrawOp { Shader = "shader", Layout = "layout", Vtx = "vtx", Draw = "draw", Target = "target", Viewport = "viewport", Scissor = "scissor", Rast = "rast", Blend1 = "blend1", Depthst = "depthst", Tsrv = "tsrv", Samp = "samp", Cbuf = "cbuf" }
}

#[derive(Clone, Debug)]
pub struct Draw {
    shaders: [Option<(&'static str, bool)>; 8],
    vs: Option<&'static str>,
    ps: Option<&'static str>,
    layout: Option<usize>,
    vtx: Vec<u32>,
    /// the views bound (OMSetRenderTargets holds them: not their slots)
    target: Option<Id<View>>,
    depth: Option<Id<View>>,
    viewport: Option<[f32; 6]>,
    scissor: [i64; 4],
    /// fill, cull, front ccw, bias, clamp, slope, clip, scissor, ms, aa
    rast: [u32; 10],
    /// enable src dest op srca desta opa mask, the factor
    blend: ([u32; 8], [f32; 4]),
    depthst: [u32; 3],
    srv: Option<Id<View>>,
    /// filter, the three address modes, the comparison function
    sampler: Option<[u32; 5]>,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ShaderPlan {
    Accept(Prog),
    Refuse(Option<Prog>),
    Unknown,
}

/// Recognize a generated program by its bytes and accept it for its own shader stage.
pub(crate) fn shader_plan(vertex: bool, bytes: &[u8]) -> ShaderPlan {
    thread_local! {
        static S: Programs = crate::shaders::shaders();
    }
    match S.with(|s| s.find(bytes)) {
        Some(program) if program.vertex() == vertex => ShaderPlan::Accept(program),
        Some(program) => ShaderPlan::Refuse(Some(program)),
        None if crate::dxbc::intact(bytes) => ShaderPlan::Unknown,
        None => ShaderPlan::Refuse(None),
    }
}

/// whether reading through view `read` and writing through `write` meet in a subresource and
/// plane (Reach::meets; views of other resources never do)
fn meets(views: &Objects<View>, resources: &Objects<Resource>, dev: Dev, read: Id<View>, write: Id<View>) -> bool {
    let (Ok(r), Ok(w)) = (views.get(read), views.get(write)) else { return false };
    if r.res != w.res {
        return false;
    }
    let Ok(res) = resources.get(r.res) else { return false };
    super::d3dres::view_reach(dev, r, res).meets(&super::d3dres::view_reach(dev, w, res))
}

pub fn op(m: &mut D3dres, dev: Dev, cmd: DrawOp, op: &Op, out: &mut Vec<String>) -> Res<()> {
    let t = op.tok.as_slice();
    let d = &mut m.draw;
    match cmd {
        DrawOp::Vtx => {
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
        DrawOp::Shader => {
            let sl = n(t, 1)? as usize;
            let vs = t.get(2).is_some_and(|s| s == "vs");
            if t.len() != 4 || sl >= 8 || !(vs || t[2] == "ps") {
                return Err(Fail::Bad);
            }
            hit(clause::D3DRASTER_SHADER);
            d.shaders[sl] = None;
            let bytes = t.last().ok_or(Fail::Bad)?;
            if !bytes.len().is_multiple_of(2) || !bytes.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(Fail::Bad);
            }
            let b = hexbytes(bytes);
            match shader_plan(vs, &b) {
                ShaderPlan::Accept(program) => {
                    let name = program.name();
                    d.shaders[sl] = Some((name, vs));
                    if vs {
                        d.vs = Some(name);
                    } else {
                        d.ps = Some(name);
                    }
                }
                ShaderPlan::Unknown => {
                    return Err(Fail::Unmodeled(crate::spec::open::D3DRASTER_SHADER_BAD));
                }
                ShaderPlan::Refuse(program) => {
                    // (checksum, size or kind wrong: assumed refused like any other bad bytecode)
                    hit(clause::D3DRASTER_SHADER_BAD);
                    out.push(format!("shader hr {E_INVALIDARG:08x}"));
                    // (the blob is kept for an input layout: a program of the other kind by its
                    // name, bytes that are no program as "bad")
                    d.shaders[sl] = Some((program.map_or("bad", Prog::name), vs));
                }
            }
        }
        DrawOp::Layout => {
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
            let usable: Vec<bool> = fmts.iter().map(|f| format(*f).is_some_and(|f| !f.block() && !f.video() && f.num != Num::Typeless)).collect();
            match layout_verdict(name, &usable).ok_or(Fail::Bad)? {
                LayoutVerdict::Made => d.layout = Some(fmts.len()),
                LayoutVerdict::NoProgram => {
                    hit(clause::D3DRASTER_LAYOUT_BAD);
                    out.push(format!("layout hr {E_INVALIDARG:08x}"));
                }
                LayoutVerdict::PixelProgram => {
                    hit(clause::D3DRASTER_LAYOUT_SIG);
                    out.push(format!("layout hr {E_INVALIDARG:08x}"));
                }
                LayoutVerdict::Unfed => out.push(format!("layout hr {E_INVALIDARG:08x}")),
            }
        }
        DrawOp::Target => {
            let r = if t.get(1).is_some_and(|s| s == "-") { None } else { Some(slot(t, 1)?) };
            let ds = if t.len() > 2 { Some(slot(t, 2)?) } else { None };
            if t.len() > 3 {
                return Err(Fail::Bad);
            }
            let bound = |s: Option<super::objects::Slot<View>>, kind: u32| s.and_then(|v| m.views.id(v)).filter(|&i| m.views.get(i).is_ok_and(|x| x.kind == kind));
            let (target, depth) = (bound(r, 1), bound(ds, 2));
            let of = |v: Id<View>| m.views.get(v).ok().and_then(|x| m.res.get(x.res).ok().map(|r| (x, r)));
            let bound = |v: Id<View>| rules::Bound {
                samples: of(v).map_or(1, |(_, r)| r.d.count.max(1)),
                size: of(v).map(|(x, r)| {
                    let (w, h, _) = r.size(r.mip_of(super::d3dres::view_subs(dev, x, r).first().copied().unwrap_or(0)));
                    (w, h)
                }),
            };
            let api = match dev {
                Dev::D11(_) => rules::Api::D11,
                Dev::D10 => rules::Api::D10,
                Dev::D101(_) => rules::Api::D101,
            };
            let (rt, ds) = (target.map(bound), depth.map(bound));
            let differ = rt.zip(ds).is_some_and(|(a, b)| a.samples != b.samples);
            let depth = match rules::bind_targets(api, rt, ds) {
                // (MEASURED, WARP: probe-rt-ms4-depth1, -depth1-prev; DXVK's ValidateRenderTargets:
                // native D3D11 keeps the bound views when they are incompatible) a target and a
                // depth view of other sample counts: on Direct3D 11 the call changes nothing, on
                // Direct3D 10 the target is bound and no depth view
                // (MEASURED, WARP: probe-rt-dsv-small, -small-fail, probe-rt-ms4-dsv-small) a
                // depth view narrower or shorter than the target: on Direct3D 11 the call changes
                // nothing; on Direct3D 10 both are bound and the draw stays inside the depth view
                rules::Bind::Keep => {
                    hit(if differ { clause::D3DRASTER_MS_DEPTHCOUNT } else { clause::D3DRASTER_DEPTH_SMALL });
                    return Ok(());
                }
                rules::Bind::Set { depth: false } if differ => {
                    hit(clause::D3DRASTER_MS_DEPTHCOUNT);
                    None
                }
                rules::Bind::Set { depth: keep } => depth.filter(|_| keep),
            };
            d.target = target;
            d.depth = depth;
            // (a shader resource view that reaches what is now drawn into is unbound,
            // OMSetRenderTargets; the call refused above (Bind::Keep) unbinds nothing)
            if let Some(sv) = d.srv && [target, depth].into_iter().flatten().any(|w| meets(&m.views, &m.res, dev, sv, w)) {
                hit(clause::D3DRASTER_HAZARD);
                d.srv = None;
            }
        }
        DrawOp::Viewport => {
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
                    *x = vp10(*x);
                }
            }
            hit(clause::D3DRASTER_VIEWPORT);
            d.viewport = Some(v);
        }
        DrawOp::Scissor => {
            if t.len() != 5 {
                return Err(Fail::Bad);
            }
            for i in 0..4 {
                d.scissor[i] = num(&t[i + 1]).ok_or(Fail::Bad)?;
            }
        }
        DrawOp::Rast => {
            if t.len() != 11 {
                return Err(Fail::Bad);
            }
            let w: Vec<u32> = (1..11).map(|i| n(t, i)).collect::<Res<_>>()?;
            match normalize(Kind::Raster, &w) {
                Some(v) => d.rast.copy_from_slice(&v),
                None => out.push(format!("rast hr {E_INVALIDARG:08x}")),
            }
        }
        DrawOp::Blend1 => {
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
        DrawOp::Depthst => {
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
        DrawOp::Tsrv => {
            let v = slot(t, 1)?;
            if t.len() != 2 {
                return Err(Fail::Bad);
            }
            if let Some(i) = m.views.id(v).filter(|&i| m.views.get(i).is_ok_and(|x| x.kind == 0)) {
                // (one that reaches a bound target or depth view is bound as NULL,
                // PSSetShaderResources)
                if [d.target, d.depth].into_iter().flatten().any(|w| meets(&m.views, &m.res, dev, i, w)) {
                    hit(clause::D3DRASTER_HAZARD);
                    d.srv = None;
                } else {
                    d.srv = Some(i);
                }
            } else {
                out.push("tsrv noview".into());
            }
        }
        DrawOp::Samp => {
            if t.len() != 5 && t.len() != 6 {
                return Err(Fail::Bad);
            }
            let mut w: Vec<u32> = (1..t.len()).map(|i| n(t, i)).collect::<Res<_>>()?;
            // (the comparison function: never when not given)
            w.resize(5, 1);
            let desc = [w[0], w[1], w[2], w[3], 0, 1, w[4], 0, 0, 0, 0, 0xff7f_ffff, 0x7f7f_ffff];
            match normalize(Kind::Sampler, &desc) {
                Some(_) => d.sampler = Some([w[0], w[1], w[2], w[3], w[4]]),
                None => out.push(format!("samp hr {E_INVALIDARG:08x}")),
            }
        }
        // (MEASURED: a bound constant buffer no program reads changes nothing, probe-cbuf)
        DrawOp::Cbuf => {
            hit(clause::D3DRASTER_CBUF);
            let b = slot::<Resource>(t, 1)?;
            // (the executor binds only a buffer)
            if m.res.at(b).is_none_or(|r| r.d.dim != 4) {
                out.push("cbuf nores".into());
            }
        }
        DrawOp::Draw => {
            if t.len() < 4 || t.len() > 5 {
                return Err(Fail::Bad);
            }
            let (top, count, stride) = (n(t, 1)?, n(t, 2)?, n(t, 3)?);
            let inst = if t.len() == 5 { n(t, 4)? } else { 0 };
            let vtx = std::mem::take(&mut d.vtx);
            draw(m, dev, top, count, stride, inst, &vtx)?;
        }
    }
    Ok(())
}

/// a vertex after the vertex shader: clip position and the color it gives
#[derive(Clone, Copy)]
struct Vert {
    pos: [f32; 4],
    color: [f32; 4],
}

/// the components of an element written (D3DRASTER.WRITEMASK, rules::merge)
fn merge(f: &Format, old: &[u8], new: &[u8], mask: u32) -> Vec<u8> {
    let comps: Vec<(u32, u32)> = f.comps.iter().zip(f.chans).map(|(&b, &c)| (b, u32::from(c))).collect();
    rules::merge(&comps, old, new, mask)
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
            let mut v = u32::from_le_bytes([e[0], e[1], e[2], e[3]]);
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
        45 => (u32::from_le_bytes([e[0], e[1], e[2], e[3]]) & 0xff_ffff) as f64,
        _ => f32::from_le_bytes([e[0], e[1], e[2], e[3]]) as f64,
    }
}

fn quantized_depth(f: &Format, z: f32) -> f64 {
    match f.id {
        55 => to_unorm(z, 16) as f64,
        45 => to_unorm(z, 24) as f64,
        _ => z as f64,
    }
}

/// No pixel shader in the runner writes a second blend source.
pub fn secondary_factor(code: u32) -> Option<f32> {
    match code {
        16 | 17 => Some(0.0),
        18 | 19 => Some(1.0),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ColorWrite {
    Integer,
    Raw,
    Blended,
}

/// Integer shader outputs go straight to the target before applying the write mask.
pub fn color_write(integer: bool, blend_enabled: bool) -> ColorWrite {
    if integer {
        ColorWrite::Integer
    } else if blend_enabled {
        ColorWrite::Blended
    } else {
        ColorWrite::Raw
    }
}

/// Edges of a clipped polygon, omitting clip-plane edges and culled polygons.
pub fn clipped_wire_edges(visible: bool, along: impl ExactSizeIterator<Item = bool>) -> impl Iterator<Item = [usize; 2]> {
    let count = along.len();
    along.enumerate().filter_map(move |(i, on_plane)| {
        if visible && !on_plane {
            Some([i, i.wrapping_add(1).checked_rem(count).unwrap_or(0)])
        } else {
            None
        }
    })
}

/// Walk the triangle's cyclic edges from the first vertex of least clip x.
pub fn wire_edges([a, b, c]: [f32; 3]) -> [[usize; 2]; 3] {
    let second = b < a;
    let least = if second { b } else { a };
    if c < least {
        [[2, 0], [0, 1], [1, 2]]
    } else if second {
        [[1, 2], [2, 0], [0, 1]]
    } else {
        [[0, 1], [1, 2], [2, 0]]
    }
}

fn factor(f: u32, s: [f32; 4], dst: [f32; 4], bf: [f32; 4], ch: usize) -> Res<f32> {
    if let Some(value) = secondary_factor(f) {
        hit(clause::D3DRASTER_BLEND_FACTOR);
        return Ok(value);
    }
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

/// the address modes on a texel coordinate (in texels) of a size (rules::address)
fn address(mode: u32, i: i64, size: i64) -> Option<i64> {
    hit(clause::D3DRASTER_ADDRESS);
    rules::address(mode, i, size)
}

fn render_rows(dev: Dev, r: &Resource, view: &View, sub: u32) -> Res<rules::TargetRows> {
    let d3d11 = matches!(dev, Dev::D11(_));
    let size = r.size(r.mip_of(sub));
    rules::target_rows(r.d.dim, view.dim(), size, (view.u(0, d3d11), view.u(1, d3d11))).ok_or(Fail::Bad)
}

#[allow(clippy::too_many_arguments)]
fn draw(m: &mut D3dres, dev: Dev, top: u32, count: u32, stride: u32, inst: u32, vtx: &[u32]) -> Res<()> {
    hit(clause::D3DRASTER_DRAW);
    let d = m.draw.clone();
    // (MEASURED: without programs and layout, or without a viewport, nothing is drawn,
    // probe-incomplete, probe-noviewport; without a target nor a depth view nothing is there to
    // draw to)
    // (MEASURED: without programs and layout, or without a viewport, nothing is drawn,
    // probe-incomplete, probe-noviewport; without a target nor a depth view nothing is there to
    // draw to)
    // (MEASURED, WARP: probe-topology-33, x86 and x86-64, Direct3D 10 and 11) a patch list
    // with no hull and domain programs ends the executor (what it printed since its last
    // flush, before the case's last tex line, is lost)
    // (MEASURED, WARP: probe-topology-0, -6 .. -9, -65, x86 and x86-64, Direct3D 10 and 11)
    // a topology D3D11_PRIMITIVE_TOPOLOGY leaves undefined: 6 draws nothing; 0, 7, 8, 9 and
    // past 64 remove the device (the next read fails with DXGI_ERROR_DEVICE_REMOVED)
    let programs = d.vs.is_some() && d.ps.is_some() && d.layout.is_some();
    let plan = rules::target_plan(d.target, d.depth);
    let topo = match rules::draw_verdict(programs, plan != TargetPlan::None, d.viewport.is_some(), top) {
        Verdict::Incomplete => {
            hit(clause::D3DRASTER_INCOMPLETE);
            return Ok(());
        }
        Verdict::NoViewport => {
            hit(clause::D3DRASTER_NOVIEWPORT);
            return Ok(());
        }
        Verdict::Crash => {
            hit(clause::D3DRASTER_PATCH);
            return Err(Fail::Crash);
        }
        Verdict::Removed => {
            hit(clause::D3DRASTER_TOPOLOGY);
            m.removed = true;
            return Ok(());
        }
        Verdict::Nothing => {
            hit(clause::D3DRASTER_TOPOLOGY);
            return Ok(());
        }
        Verdict::Draw(t) => t,
    };
    let (Some(vs), Some(ps), Some(vp)) = (d.vs, d.ps, d.viewport) else {
        return Err(Fail::Bad);
    };
    let tv = match plan {
        TargetPlan::Color(view) => view,
        // (MEASURED, WARP: probe-depth-alone) the depth view supplies the drawable extent.
        TargetPlan::DepthOnly(view) => {
            hit(clause::D3DRASTER_DEPTH_ALONE);
            view
        }
        TargetPlan::None => return Err(Fail::Bad),
    };
    // (the views bound, whatever their slots hold since: the device holds them, and each view
    // its resource)
    let view = m.views.get(tv)?.clone();
    let rt_fmt = format(view.format()).ok_or(Fail::Unmodeled(open::D3DRASTER_RTFMT))?;
    let rt_sub = super::d3dres::view_subs(dev, &view, m.res.get(view.res)?)[0];
    let (rw, rh) = {
        let r = m.res.get(view.res)?;
        let rows = render_rows(dev, r, &view, rt_sub)?;
        // (MEASURED, WARP: probe-rt-3d, -3d-slice1, -1d, -1darray, -buffer, -buffer-off)
        if r.d.dim == 3 {
            hit(clause::D3DRASTER_RT_3D);
        } else if matches!(r.d.dim, 1 | 4) {
            hit(clause::D3DRASTER_RT_1D);
        }
        (rows.width, rows.height)
    };
    // (a multisampled target: D3DRASTER.MS.SAMPLES, quality 0; the viewport anywhere,
    // probe-rt-ms4-vphalf)
    let samples = {
        let r = m.res.get(view.res)?;
        match r.d.count {
            0 | 1 => None,
            n => {
                // (MEASURED, WARP: probe-rt-ms4-lines, -wire, -linesms, -linesboth) aliased lines
                // and wireframe's edges light every sample of the pixels they light, multisample
                // lines the samples their quad covers; (probe-rt-ms4-points) a point the samples
                // of the pixel square about it
                match topo {
                    Topo::Lines | Topo::LineStrip | Topo::LinesAdj | Topo::LineStripAdj => hit(clause::D3DRASTER_MS_LINES),
                    Topo::Points => hit(clause::D3DRASTER_MS_POINTS),
                    Topo::Tris | Topo::TriStrip | Topo::TrisAdj | Topo::TriStripAdj if d.rast[0] == 2 => hit(clause::D3DRASTER_MS_LINES),
                    Topo::Tris | Topo::TriStrip | Topo::TrisAdj | Topo::TriStripAdj => {}
                }
                // (quality levels other than 0: not measured)
                if r.d.q != 0 {
                    return Err(Fail::Bad);
                }
                // (WARP makes only quality 0 and counts 2, 4, 8 and 16, probe-rt-ms4-q1, -ms32)
                Some(sample_pattern(n).ok_or(Fail::Bad)?)
            }
        }
    };
    // (a depth view of another sample count is never bound, D3DRASTER.MS.DEPTHCOUNT; one whose
    // slot was emptied: guarded above)
    let dsv = d.depth.map(|v| m.views.get(v).cloned()).transpose()?;
    // the vertices through the vertex shader
    let fetch = |v: u32, i: u32| -> Res<Vert> {
        // (MEASURED: past the buffer the vertex reads zeros, probe-vb-past)
        let w = |k: u32| -> Res<f32> {
            if usize::try_from((v.wrapping_mul(stride) / 4).wrapping_add(k)).is_ok_and(|i| i >= vtx.len()) {
                hit(clause::D3DRASTER_VBPAST);
            }
            Ok(f32::from_bits(rules::vertex_word(vtx, v, stride, k)))
        };
        let pos = [w(0)?, w(1)?, w(2)?, w(3)?];
        let color = match vs {
            "vs" => [w(4)?, w(5)?, w(6)?, w(7)?],
            _ => {
                hit(clause::D3DRASTER_IDS);
                vertex_id_color(v, i)
            }
        };
        Ok(Vert { pos, color })
    };
    // (a triangle wholly past the buffer reads zeros, a point at w = 0, and with a zero stride
    // all three vertices are one point: nothing drawn, so the draw stops there)
    let nv = u64::try_from(vtx.len()).unwrap_or(u64::MAX).saturating_mul(4).checked_div(u64::from(stride)).map_or(1, |n| n + 1);
    let nprim = rules::prim_count(topo, count, nv);
    let nprim = u32::try_from(nprim).unwrap_or(u32::MAX);
    let tg = Target { dev, vp, size: (rw, rh), view: &view, fmt: rt_fmt, sub: rt_sub, dsv: dsv.as_ref(), samples };
    for (i, primitives) in rules::draw_ids(inst, nprim) {
        // (the lines of the draw, or the edges of its triangles in wireframe: drawn together,
        // since whether a line's end lights its pixel depends on the lines after it)
        let mut segs: Vec<Seg> = Vec::new();
        for p in primitives {
            if matches!(topo, Topo::LinesAdj | Topo::LineStripAdj | Topo::TrisAdj) {
                // (MEASURED, WARP: probe-topology-10 to -13) with adjacency and no geometry
                // program, the adjacent vertices are skipped (rules::primitive)
                hit(clause::D3DRASTER_ADJ);
            }
            let primitive = rules::primitive(topo, p).ok_or(Fail::Bad)?;
            match primitive {
                rules::Prim::Point(_) => point(m, &d, &tg, ps, fetch(primitive.leading(), i)?, p)?,
                rules::Prim::Line(_, k1) => {
                    let a = fetch(primitive.leading(), i)?;
                    segs.push(Seg { a, b: fetch(k1, i)?, lead: a, prim: p, front: true, clip: SegClip::Line });
                }
                rules::Prim::Tri(idx, _) => {
                    // (strips: every other triangle's first two vertices swapped; the leading
                    // vertex first)
                    if topo == Topo::TriStrip && p % 2 == 1 {
                        hit(clause::D3DRASTER_STRIP);
                    }
                    let tri = [fetch(idx[0], i)?, fetch(idx[1], i)?, fetch(idx[2], i)?];
                    let leadv = fetch(primitive.leading(), i)?;
                    triangle(m, dev, &d, vp, ps, &tri, leadv, p, (rw, rh), samples, &view, rt_fmt, rt_sub, dsv.as_ref(), &mut segs)?;
                }
            }
        }
        lines(m, &d, &tg, ps, &segs)?;
    }
    Ok(())
}

/// The generated vertex shader converts the two uint system values to color channels.
pub(crate) fn vertex_id_color(vertex: u32, instance: u32) -> [f32; 4] {
    [vertex as f32, instance as f32, 0.0, 1.0]
}

/// The generated pixel shader writes the primitive number and a one-bit front-face value.
pub(crate) fn primitive_id_color(primitive: u32, front: bool) -> [u32; 4] {
    [(primitive as f32).to_bits(), f32::from(u8::from(front)).to_bits(), 0, 1.0f32.to_bits()]
}

/// SV_Position supplies the pixel center and the selected depth and clip w.
pub(crate) fn position_color((px, py): (i64, i64), depth: f32, w: f32) -> [u32; 4] {
    [center_of(px), center_of(py), depth, w].map(f32::to_bits)
}

/// A constant pixel input preserves the leading vertex's four float channels.
pub(crate) fn flat_color(lead: [f32; 4]) -> [u32; 4] {
    lead.map(f32::to_bits)
}

/// Equal clip w values use the first vertex's w directly.
pub(crate) fn constant_clip_w(vertices: &[f32]) -> Option<f32> {
    let &first = vertices.first()?;
    vertices.iter().all(|&w| w == first).then_some(first)
}

/// A varying clip w is reconstructed from the interpolated reciprocal.
pub(crate) fn position_w_from_reciprocal(inverse: f32) -> f32 {
    1.0 / inverse
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
        let (cx, cy) = (center_of(px), center_of(py));
        (0..i64::from(self.size.0)).contains(&px)
            && (0..i64::from(self.size.1)).contains(&py)
            && cx >= vx
            && cx < vx + vw
            && cy >= vy
            && cy < vy + vh
            && rules::scissor_holds(d.rast[7] != 0, d.scissor, (px, py))
    }

    /// a depth in the clip volume's [0, 1] onto the viewport's range
    fn depth(&self, z: f32) -> f32 {
        let [_, _, _, _, minz, maxz] = self.vp;
        self.clamp_depth(minz + z * (maxz - minz))
    }

    /// a depth onto the viewport's range, clamped
    fn clamp_depth(&self, z: f32) -> f32 {
        let [_, _, _, _, minz, maxz] = self.vp;
        clamp_range(minz, maxz, z)
    }

    fn put(&self, m: &mut D3dres, d: &Draw, (px, py): (i64, i64), z: f32, out: [u32; 4]) -> Res<()> {
        let at = (usize::try_from(px).unwrap_or(0), usize::try_from(py).unwrap_or(0), self.size.0 as usize);
        match self.samples {
            // (on a multisampled target every sample of the pixel, D3DRASTER.MS.LINES)
            Some(pattern) => put_samples(m, self.dev, d, self.view, self.fmt, self.sub, self.dsv, at, Some(z), None, out, &line_samples(false, &vec![false; pattern.len()])),
            None => output(m, self.dev, d, self.view, self.fmt, self.sub, self.dsv, at, Some(z), out),
        }
    }
}

/// v pixels in 1/256, rounding half to even (cvtss2si), saturated to i32
fn snap_even(v: f32) -> i64 {
    i64::from(cvtss2si(v * 256.0))
}

/// cvtss2si with the default rounding: to nearest, ties to even; a NaN or a value past i32 is
/// the "integer indefinite" i32::MIN (D3DRASTER.CLIPW.NANPL: a clipped end of NaN lands at
/// -2^23 pixels)
fn cvtss2si(v: f32) -> i32 {
    let r = f64::from(v).round_ties_even();
    if (-2147483648.0..2147483648.0).contains(&r) { r as i32 } else { i32::MIN }
}

/// a sample offset of 1/16 pixels in pixels
fn sixteenths(o: i64) -> f32 {
    o as f32 / 16.0
}

/// a pixel coordinate as an index (the clipped bounds keep it at or above 0)
fn index(v: impl TryInto<usize>) -> usize {
    v.try_into().unwrap_or(usize::MAX)
}

/// two four-channel values joined channel by channel
fn zip4<T: Copy, U>(a: [T; 4], b: [T; 4], f: impl Fn(T, T) -> U) -> [U; 4] {
    let ([a0, a1, a2, a3], [b0, b1, b2, b3]) = (a, b);
    [f(a0, b0), f(a1, b1), f(a2, b2), f(a3, b3)]
}

/// a pixel's center along one axis
fn center_of(p: i64) -> f32 {
    p as f32 + 0.5
}

/// D3DRASTER.FILTER.LINEAR: a coordinate's first texel and the weight of the next, in 1/256:
/// s = c n - 0.5 in f32, floor(s) and floor(frac(s) 256)
pub fn split(c: f32, n: u32) -> (i64, i64) {
    let s = c * n as f32 - 0.5;
    let i = s.floor();
    (i as i64, ((s - i) * 256.0).floor() as i64)
}

/// D3DRASTER.FILTER.POINT: a coordinate's texel, floor(c n) with the product in f32
pub fn point_texel(c: f32, n: u32) -> i64 {
    (c * n as f32).floor() as i64
}

/// D3DRASTER.SAMPLE.CMP: a texel's red against the reference by the comparison function, 1 or 0
pub fn sample_cmp(func: u32, reference: f32, red: f32) -> f32 {
    if rules::compare(func, reference, red) { 1.0 } else { 0.0 }
}

/// D3DRASTER.FILTER.LEVEL, MIPLINEAR: the level of a lod (log2 of the span) below the last level:
/// mip point the nearest, floor(lod + 0.5); mip linear a whole lod's level alone, else the level
/// below and the next's weight round(frac(lod) 256)
pub fn level(lod: f64, last: u32, point: bool) -> Level {
    let last = f64::from(last);
    if point {
        Level::One(((lod + 0.5).floor().min(last)) as u32)
    } else if lod.fract() == 0.0 || lod >= last {
        Level::One(lod.floor().min(last) as u32)
    } else {
        Level::Two(lod.floor() as u32, (lod.fract() * 256.0).round() as i64)
    }
}

/// D3DRASTER.NOSAMPLER: the sampler bound, or with none the default one: linear filtering,
/// clamped addresses (filter, address u, v, w, comparison)
pub fn sampler_or_default(s: Option<[u32; 5]>) -> [u32; 5] {
    s.unwrap_or([0x15, 3, 3, 3, 1])
}

/// An ordinary linear float sample crashes WARP when its view stores integers.
pub fn linear_filter_crashes(integer: bool, comparison: bool) -> bool {
    integer && !comparison
}

/// D3DRASTER.FILTER.ANISO, MINMAG: a sampler's filter's minifying and magnifying filters,
/// linear or point; None for a filter the model does not draw with
pub fn filter_linear(f: u32) -> Option<(bool, bool)> {
    Filter::of(f).map(|x| (x.min, x.mag))
}

/// D3DRASTER.FILTER.MINMAG, LEVEL, ANISO: the texels of level 0 (w x h) a 2 x 2 quad spans, its
/// coordinates at its corner and its right and lower neighbours: the longer of
/// sqrt((du w)^2 + (dv h)^2) along x and along y, in f64
pub fn quad_span(q: [[f32; 2]; 3], w: u32, h: u32) -> f64 {
    let span = |a: [f32; 2], b: [f32; 2]| ((f64::from(b[0] - a[0]) * f64::from(w)).powi(2) + (f64::from(b[1] - a[1]) * f64::from(h)).powi(2)).sqrt();
    span(q[0], q[1]).max(span(q[0], q[2]))
}

/// which filter a quad of span rho takes
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterPick {
    /// the magnifying filter, at level 0
    Mag,
    /// the minifying filter, at the one level
    Min,
    /// the minifying filter at the level or levels of the lod
    Mip(Level),
}

/// D3DRASTER.FILTER.MINMAG, LEVEL: a span of up to one texel magnifies, past it minifies; with
/// mips at the level of log2rho (f64::log2 of rho, as the platform's libm gives it), mip point
/// or linear
pub fn filter_pick(mips: u32, mip_point: bool, rho: f64, log2rho: f64) -> FilterPick {
    if rho <= 1.0 {
        FilterPick::Mag
    } else if mips <= 1 {
        FilterPick::Min
    } else {
        FilterPick::Mip(level(log2rho, mips.saturating_sub(1), mip_point))
    }
}

/// D3DRASTER.DEPTH.SMALL (Direct3D 10): a pixel past a smaller depth view's width or height
pub fn past_depth(dw: u32, dh: u32, x: usize, y: usize) -> bool {
    index(dw) <= x || index(dh) <= y
}

/// a 16.8 fixed-point coordinate in pixels (exact below 2^24)
pub fn pixels(v: i64) -> f32 {
    v as f32 / 256.0
}

/// a position in 1/256 pixels (16.8 fixed point, D3DRASTER.SNAP)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Sub {
    x: i64,
    y: i64,
}

/// a coordinate snapped to 1/256 pixel, rounding half to even, as cvtss2si
fn snap256(v: f32) -> i64 {
    snap_even(v)
}

/// a clip position through the viewport transform, snapped
fn snapped(vp: [f32; 6], q: &[f32; 4]) -> Sub {
    let (x, y) = viewport_xy(vp, q);
    Sub { x, y }
}

/// whether a point's or a line's vertex leaves it to draw
#[derive(Clone, Copy, PartialEq, Eq)]
enum Drawn {
    /// drawn by the clipping as any other (a w not positive, D3DRASTER.CLIPW.POINTLINE; a w
    /// infinite, D3DRASTER.CLIPW.NANPL)
    Clip,
    /// (D3DRASTER.CLIPW.NANPL) a coordinate NaN: the point or line draws nothing
    Nothing,
}

/// a vertex of a point or a line: NaN anywhere takes the primitive away, an infinite w is clipped
/// as a finite one (probe-clip-w-point-nan, -line-nan, -line-inf); any other coordinate infinite
/// is not measured
fn drawable_w(v: &Vert) -> Res<Drawn> {
    if v.pos.iter().any(|x| x.is_nan()) {
        hit(clause::D3DRASTER_CLIPW_NANPL);
        return Ok(Drawn::Nothing);
    }
    if v.pos.iter().any(|x| x.is_infinite()) {
        hit(clause::D3DRASTER_CLIPW_NANPL);
    }
    if !(v.pos[3] > 0.0) {
        hit(clause::D3DRASTER_CLIPW_POINTLINE);
    }
    Ok(Drawn::Clip)
}

/// (MEASURED, WARP: probe-points-*, every pixel) a point inside the clip volume lights the
/// one pixel (ceil(x) - 1, ceil(y) - 1) of its snapped position, in the vertex's color
fn point(m: &mut D3dres, d: &Draw, tg: &Target, ps: &str, v: Vert, prim: u32) -> Res<()> {
    hit(clause::D3DRASTER_POINT);
    if drawable_w(&v)? == Drawn::Nothing {
        return Ok(());
    }
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
        for py in (s.y - 128).div_euclid(256) - 1..=(s.y + 128).div_euclid(256) + 1 {
            for px in (s.x - 128).div_euclid(256) - 1..=(s.x + 128).div_euclid(256) + 1 {
                if !tg.drawable(d, (px, py)) {
                    continue;
                }
                let mask: Vec<bool> = pattern.iter().map(|&(ox, oy)| rules::point_covers((s.x, s.y), (sample_sub(px, ox), sample_sub(py, oy)))).collect();
                if mask.contains(&true) {
                    // (MEASURED, WARP: probe-rt-ms4-points-pspos) each pixel shaded at its center
                    let out = prim_out(ps, v.color, (px, py), z, q[3], prim, true)?;
                    put_samples(m, tg.dev, d, tg.view, tg.fmt, tg.sub, tg.dsv, (index(px), index(py), index(tg.size.0)), Some(z), None, out, &mask)?;
                }
            }
        }
        return Ok(());
    }
    let px = (rules::point_pixel(s.x), rules::point_pixel(s.y));
    if !tg.drawable(d, px) {
        return Ok(());
    }
    let z = tg.depth(z);
    let out = prim_out(ps, v.color, px, z, q[3], prim, true)?;
    tg.put(m, d, px, z, out)
}

/// what the programs write for a point or a line's pixel: its vertex's (or leading vertex's)
/// color, the pixel's position, or the primitive's index
/// (MEASURED, WARP: probe-top1-*, probe-top2-*) as for triangles: SV_Position is the pixel
/// center, the depth and the clip w; points and lines face front
fn prim_out(ps: &str, c: [f32; 4], pixel: (i64, i64), z: f32, w: f32, prim: u32, front: bool) -> Res<[u32; 4]> {
    match ps {
        "ps" => Ok(c.map(f32::to_bits)),
        "psflat" => {
            hit(clause::D3DRASTER_FLAT);
            Ok(flat_color(c))
        }
        "pspos" => {
            hit(clause::D3DRASTER_POS);
            if w.to_bits() != 1.0f32.to_bits() {
                hit(clause::D3DRASTER_POSW);
            }
            Ok(position_color(pixel, z, w))
        }
        "psprim" => {
            hit(clause::D3DRASTER_PRIM);
            Ok(primitive_id_color(prim, front))
        }
        // (a pixel program the generators do not make)
        _ => Err(Fail::Bad),
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
    clip: SegClip,
}

/// whether a segment is still to be clipped as a line
#[derive(Clone, Copy, PartialEq, Eq)]
enum SegClip {
    /// a line's, or an unclipped triangle's edge
    Line,
    /// an edge of a triangle the homogeneous clipping cut (D3DRASTER.CLIPW.WIRE): drawn as it
    /// came out
    Done,
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
    major: Major,
    cover: Cover,
    /// the ends' depths on the viewport's range, minz + z (1/w) (maxz - minz), not clamped
    z: [f32; 2],
}

/// how lines are drawn, by the rasterizer state's MultisampleEnable and
/// AntialiasedLineEnable
#[derive(Clone, Copy)]
enum LineMode {
    /// neither: the diamond exit rule
    Aliased,
    /// MultisampleEnable (with or without the other): D3DRASTER.LINE.QUAD
    Quad,
    /// AntialiasedLineEnable alone: D3DRASTER.LINE.ALPHA
    Alpha,
}

impl LineMode {
    fn of(d: &Draw) -> Res<LineMode> {
        match (d.rast[8] != 0, d.rast[9] != 0) {
            (false, false) => Ok(LineMode::Aliased),
            (true, _) => {
                hit(clause::D3DRASTER_LINE_QUAD);
                Ok(LineMode::Quad)
            }
            (false, true) => {
                hit(clause::D3DRASTER_LINE_ALPHA);
                Ok(LineMode::Alpha)
            }
        }
    }
}

/// what CreateInputLayout does with a layout against a program
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LayoutVerdict {
    /// made
    Made,
    /// (MEASURED: bytes that are no program, a broken checksum, refused E_INVALIDARG,
    /// probe-layout-refused) D3DRASTER.LAYOUT.BAD
    NoProgram,
    /// (MEASURED: a pixel program given as a vertex one, refused E_INVALIDARG,
    /// probe-layout-otherkind) D3DRASTER.LAYOUT.SIG
    PixelProgram,
    /// an input of the program from no element, or an element of a format no input takes:
    /// refused E_INVALIDARG
    Unfed,
}

/// the generator's programs CreateInputLayout is given, in the order of the mirror
/// d3d11.layout's first argument
pub const LAYOUT_PROGRAMS: [&str; 11] = ["vs", "vsid", "bad", "ps", "psflat", "pspos", "psprim", "psld", "psldu", "pssample", "pssamplec"];

/// D3DRASTER.LAYOUT, LAYOUT.BAD, LAYOUT.SIG: an input layout of elements (each of a format an
/// input takes or not) against the program `name`; None for a program the generator does not
/// make
pub fn layout_verdict(name: &str, usable: &[bool]) -> Option<LayoutVerdict> {
    let needs = match name {
        "vs" => 2,
        "vsid" => 1,
        "bad" => return Some(LayoutVerdict::NoProgram),
        "ps" | "psflat" | "pspos" | "psprim" | "psld" | "psldu" | "pssample" | "pssamplec" => return Some(LayoutVerdict::PixelProgram),
        _ => return None,
    };
    // every input of the program from an element, no element of an unknown format
    Some(if usable.len() >= needs && usable.iter().all(|&u| u) { LayoutVerdict::Made } else { LayoutVerdict::Unfed })
}

/// half the width of a multisample line, in pixels
const QUAD_HALF: f32 = 0.7;

/// D3DRASTER.LINE: the major axis of a line from a to b: y when |dy| > |dx|
fn major_of(a: Sub, b: Sub) -> Major {
    if (b.y - a.y).abs() > (b.x - a.x).abs() { Major::Y } else { Major::X }
}

/// D3DRASTER.LINE, LINE.END: an aliased line from a to b (1/256 pixels) lights the pixel, by
/// its major axis, its ends moved out of their diamonds and the crossing tests
pub fn line_covers(a: (i64, i64), b: (i64, i64), px: (i64, i64)) -> bool {
    let (a, b) = (Sub { x: a.0, y: a.1 }, Sub { x: b.0, y: b.1 });
    Cover::of(a, b, major_of(a, b)).covers(px)
}

/// D3DRASTER.LINE.QUAD: a multisample line's corners a + n, b + n, b - n, a - n (n the normal
/// of length 0.7, in f32), snapped
pub fn quad_corners(a: (i64, i64), b: (i64, i64)) -> [(i64, i64); 4] {
    let (ax, ay, bx, by) = (pixels(a.0), pixels(a.1), pixels(b.0), pixels(b.1));
    let (dx, dy) = (bx - ax, by - ay);
    let k = QUAD_HALF / (dx * dx + dy * dy).sqrt();
    let (nx, ny) = (-dy * k, dx * k);
    let q = |x: f32, y: f32| (snap256(x), snap256(y));
    [q(ax + nx, ay + ny), q(bx + nx, by + ny), q(bx - nx, by - ny), q(ax - nx, ay - ny)]
}

/// D3DRASTER.LINE.ALPHA: an alpha antialiased line's corners a - o, a + o, b + o, b - o, o the
/// normal (dy, -dx) 2 / length in f32 snapped to 1/256
pub fn alpha_corners(a: (i64, i64), b: (i64, i64)) -> [(i64, i64); 4] {
    let f = pixels;
    let (dyf, ndx) = (f(b.1 - a.1), f(a.0 - b.0));
    let len = (ndx * ndx + dyf * dyf).sqrt();
    let k = 2.0 / len;
    let (ox, oy) = (snap_even(dyf * k), snap_even(ndx * k));
    [(a.0 - ox, a.1 - oy), (a.0 + ox, a.1 + oy), (b.0 + ox, b.1 + oy), (b.0 - ox, b.1 - oy)]
}

/// D3DRASTER.LINE.ALPHA: the coverage of an alpha antialiased line from a to b at the pixel:
/// the distance s from the line, its plane in f32 (A, B, C = (dy, -dx, y0 dx - x0 dy) /
/// sqrt(dx^2 + dy^2)), clamp((1.5 - |s|) 0.45, 0, 1)
pub fn alpha_coverage(a: (i64, i64), b: (i64, i64), (px, py): (i64, i64)) -> f32 {
    let f = pixels;
    let (dyf, dx) = (f(b.1 - a.1), f(b.0 - a.0));
    let (a0, b0) = (dyf, -dx);
    let c0 = f(a.1) * dx - f(a.0) * dyf;
    let inv = 1.0 / (b0 * b0 + dyf * dyf).sqrt();
    let (a, b, c) = (inv * a0, inv * b0, inv * c0);
    let s = a * (center_of(px)) + (b * (center_of(py)) + c);
    ((1.5 - s.abs()) * f32::from_bits(0x3EE6_6666)).clamp(0.0, 1.0)
}

/// D3DRASTER.MS.LINES: the samples a line writes at a pixel it lights: with the multisample
/// state those its quad covers, else every one
pub fn line_samples(multisample: bool, covered: &[bool]) -> Vec<bool> {
    if multisample { covered.to_vec() } else { vec![true; covered.len()] }
}

/// the major axis of a line: x when |dx| >= |dy|
#[derive(Clone, Copy, PartialEq, Eq)]
enum Major {
    X,
    Y,
}

/// (MEASURED, WARP: probe-lines-*, probe-linestrip-*, probe-wire-*, probe-lineend-*,
/// probe-linevert-*, probe-linetie-*, probe-linecorner-*) an aliased line as the rasterizer
/// tests it: its ends in f32, a start in its diamond moved back and an end in its diamond pulled
/// back to 1/256 before the diamond's center, and the bounds of the crossing tests
struct Cover {
    x0: f32,
    y0: f32,
    dx: f32,
    dy: f32,
    /// the upper bounds of the crossing of the column's and the row's center line
    hi_col: f32,
    hi_row: f32,
}

/// 0.5 and the next f32 up
const HALF_UP: f32 = f32::from_bits(0x3F00_0001);

/// ceil(v) - 0.5: the center of the pixel whose diamond may hold v
fn owner(v: f32) -> f32 {
    v.ceil() - 0.5
}

/// (x, y) in the diamond about (cx, cy): its lower half with its edges, its upper half without;
/// on the middle row a y-major line's diamond holds its right corner
fn diamond_holds(x: f32, y: f32, cx: f32, cy: f32, major: Major) -> bool {
    let d = (x - cx).abs() + (y - cy).abs();
    let inclusive = y > cy || y == cy && major == Major::Y && x > cx;
    if inclusive { d <= 0.5 } else { d < 0.5 }
}

impl Cover {
    fn of(a: Sub, b: Sub, major: Major) -> Cover {
        let f = pixels;
        let (x0, y0, x1, y1) = (f(a.x), f(a.y), f(b.x), f(b.y));
        let (dx, dy) = (x1 - x0, y1 - y0);
        let (cx0, cy0, cx1, cy1) = (owner(x0), owner(y0), owner(x1), owner(y1));
        let ins = diamond_holds(x0, y0, cx0, cy0, major);
        let ine = diamond_holds(x1, y1, cx1, cy1, major);
        let (hi_col, hi_row) = match major {
            Major::X => (HALF_UP, 0.5),
            Major::Y => (HALF_UP, HALF_UP),
        };
        if ins && ine && cx0 == cx1 && cy0 == cy1 {
            // (no pixel)
            return Cover { x0: -0.5, y0: -0.5, dx: 0.0, dy: 0.0, hi_col, hi_row };
        }
        const E: f32 = 1.0 / 256.0;
        let sign = |d: f32| if d > 0.0 { E } else { -E };
        let (mut cx, mut cy, mut cdx, mut cdy) = (x0, y0, dx, dy);
        if ins {
            let (u, v) = match major {
                Major::X => {
                    let u = (x0 - cx0) + sign(dx);
                    (u, (dy / dx) * u)
                }
                Major::Y => {
                    let v = (y0 - cy0) + sign(dy);
                    ((dx / dy) * v, v)
                }
            };
            cx = x0 - u;
            cy = y0 - v;
            cdx = dx + u;
            cdy = dy + v;
        }
        if ine {
            hit(clause::D3DRASTER_LINE_END);
            let (u, v) = match major {
                Major::X => {
                    let u = ((cx + cdx) - cx1) + sign(cdx);
                    (u, (cdy / cdx) * u)
                }
                Major::Y => {
                    let v = ((cy + cdy) - cy1) + sign(cdy);
                    ((cdx / cdy) * v, v)
                }
            };
            cdx -= u;
            cdy -= v;
        }
        Cover { x0: cx, y0: cy, dx: cdx, dy: cdy, hi_col, hi_row }
    }

    /// the line crosses the pixel's column center line or row center line inside the pixel
    /// (each f32 operation rounded on its own)
    fn covers(&self, (px, py): (i64, i64)) -> bool {
        let (inv_dx, inv_dy) = (-1.0 / self.dx, -1.0 / self.dy);
        let (a, b) = (self.x0 - (center_of(px)), self.y0 - (center_of(py)));
        let inside = |t: f32| (0.0..=1.0).contains(&t);
        let t = a * inv_dx;
        let col = inside(t) && {
            let e = t * self.dy + b;
            e > -0.5 && e < self.hi_col
        };
        let t2 = b * inv_dy;
        let row = inside(t2) && {
            let e = t2 * self.dx + a;
            e > -0.5 && e < self.hi_row
        };
        col || row
    }
}

impl Line {
    /// (MEASURED, WARP: probe-lines-ms, -msaa, 84 pixels) with the multisample state a line
    /// is the rectangle 1.4 pixels wide along it, ending at its ends: its corners a +- n,
    /// b +- n (n the normal of length 0.7, in f32) snapped, as two triangles a + n, b + n,
    /// b - n and a + n, b - n, a - n by the top-left rule; the values are the covering
    /// triangle's planes (D3DRASTER.INTERP.PLANE, not turned) over the snapped corners, a's
    /// values at a +- n and b's at b +- n (84 UNORM pixels; probe-linesms-*, 220 float values)
    fn quad(&self, (px, py): (i64, i64)) -> Option<Planes> {
        self.quad_at((px, py), &[(0, 0)]).1
    }

    /// the quad's two triangles: corners in 1/256 pixels, and whether each is at b
    fn quad_tris(&self) -> [[((i64, i64), bool); 3]; 2] {
        let c = quad_corners((self.a.x, self.a.y), (self.b.x, self.b.y));
        let corner = [(c[0], false), (c[1], true), (c[2], true), (c[3], false)];
        [[0, 1, 2], [0, 2, 3]].map(|t| t.map(|i| corner[i]))
    }

    /// the samples (offsets in 1/16 pixel from the center) the quad covers, and the planes of
    /// the triangle covering the first covered sample (D3DRASTER.MS.LINES)
    fn quad_at(&self, (px, py): (i64, i64), pattern: &[(i64, i64)]) -> (Vec<bool>, Option<Planes>) {
        self.tris_at(self.quad_tris(), (px, py), pattern)
    }

    /// (MEASURED, WARP: probe-linesaa-*, probe-lines-aa) an alpha antialiased line is the
    /// rectangle 4 pixels wide along it, ending at its ends: the normal (dy, -dx) 2 / length in
    /// f32 snapped to 1/256 as o, the corners a - o, a + o, b + o, b - o as two triangles
    /// (0, 1, 2) and (0, 2, 3) by the top-left rule
    fn alpha_tris(&self) -> [[((i64, i64), bool); 3]; 2] {
        let c = alpha_corners((self.a.x, self.a.y), (self.b.x, self.b.y));
        let corner = [(c[0], false), (c[1], false), (c[2], true), (c[3], true)];
        [[0, 1, 2], [0, 2, 3]].map(|t| t.map(|i| corner[i]))
    }

    /// (MEASURED, WARP: probe-linesaa-*, probe-lines-aa) the coverage of an alpha
    /// antialiased line at the pixel: the distance s from the line, its plane in f32 (A, B, C
    /// = (dy, -dx, y0 dx - x0 dy) / sqrt(dx^2 + dy^2)), clamp((1.5 - |s|) 0.45, 0, 1)
    fn alpha_cover(&self, px: (i64, i64)) -> f32 {
        alpha_coverage((self.a.x, self.a.y), (self.b.x, self.b.y), px)
    }

    /// the samples the triangles cover and the planes of the triangle covering the first
    fn tris_at(&self, tris: [[((i64, i64), bool); 3]; 2], (px, py): (i64, i64), pattern: &[(i64, i64)]) -> (Vec<bool>, Option<Planes>) {
        let at = |(ox, oy): (i64, i64)| (sample_sub(px, ox), sample_sub(py, oy));
        let mask: Vec<bool> = pattern.iter().map(|&o| tris.iter().any(|t| tri_covers(t.map(|v| v.0), at(o)))).collect();
        let Some(t) = pattern.iter().zip(&mask).filter(|(_, c)| **c).find_map(|(&o, _)| tris.into_iter().find(|t| tri_covers(t.map(|v| v.0), at(o)))) else {
            return (mask, None);
        };
        // (MEASURED, WARP: probe-linesms-*, 220 float values) the planes start at a + n, as
        // the triangles are given, not turned
        let iw = t.map(|v| if v.1 { self.iwb } else { self.iwa });
        let aw = [0, 1, 2].map(|i| (if t[i].1 { self.cb } else { self.ca }).map(|a| a * iw[i]));
        (mask, Some(Planes { xs: t.map(|v| pixels(v.0 .0)), ys: t.map(|v| pixels(v.0 .1)), iw, aw, zs: t.map(|v| if v.1 { self.z[1] } else { self.z[0] }) }))
    }

    /// (MEASURED, WARP: probe-lines-*, probe-linestrip-*, probe-wire-*, 904 pixels;
    /// probe-linesw-*, 114 pixels) the color along the line, perspective-correct: a plane over
    /// the major axis for 1/w and for each channel's c/w, g = 1 / (b - a), the gradient
    /// (vb - va) g, C = va - gradient a, the value C + gradient c at the pixel center, in f32;
    /// the channel's value times 1 / (the value of 1/w)
    fn color(&self, px: (i64, i64)) -> [f32; 4] {
        let q = 1.0 / self.plane(px, self.iwa, self.iwb);
        [0, 1, 2, 3].map(|k| self.plane(px, self.ca[k] * self.iwa, self.cb[k] * self.iwb) * q)
    }

    /// (MEASURED, WARP: probe-line-w-pos) SV_Position's w: 1 / the value of 1/w along the
    /// major axis (D3DRASTER.POSW.PLANE)
    fn pos_w(&self, px: (i64, i64)) -> f32 {
        match self.w {
            Some(w) => w,
            None => {
                hit(clause::D3DRASTER_POSW_PLANE);
                position_w_from_reciprocal(self.plane(px, self.iwa, self.iwb))
            }
        }
    }

    /// a value along the major axis at the pixel center, as for the color
    fn plane(&self, (px, py): (i64, i64), va: f32, vb: f32) -> f32 {
        let (a, b, c) = if self.major == Major::Y { (self.a.y, self.b.y, center_of(py)) } else { (self.a.x, self.b.x, center_of(px)) };
        line_value(a, b, c, va, vb)
    }

    /// (MEASURED, WARP: probe-line-clip-z) the depth along the line: the ends' depths planed
    /// over the major axis as the color (no perspective), clamped to the viewport's range
    fn depth(&self, tg: &Target, px: (i64, i64)) -> f32 {
        if self.z[0] == self.z[1] {
            return tg.clamp_depth(self.z[0]);
        }
        hit(clause::D3DRASTER_LINE_DEPTH);
        tg.clamp_depth(self.plane(px, self.z[0], self.z[1]))
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

/// a line clipped to 0 <= z <= w, as clip_line; None when wholly out
fn clip_line_z(mut a: Vert, mut b: Vert) -> Option<(Vert, Vert)> {
    for plane in [ZPlane::Near, ZPlane::Far] {
        let dist = |q: &Vert| match plane {
            ZPlane::Near => q.pos[2],
            ZPlane::Far => q.pos[3] - q.pos[2],
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

/// the depth range's planes
#[derive(Clone, Copy)]
enum ZPlane {
    Near,
    Far,
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
        if drawable_w(&s.a)? == Drawn::Nothing || drawable_w(&s.b)? == Drawn::Nothing {
            continue;
        }
        let (za, zb) = (s.a.pos[2] / s.a.pos[3], s.b.pos[2] / s.b.pos[3]);
        let inside = |z: f32| (0.0..=1.0).contains(&z);
        let behind = !(s.a.pos[3] > 0.0 && s.b.pos[3] > 0.0);
        let zclip = s.clip == SegClip::Line && !behind && d.rast[6] != 0 && !(inside(za) && inside(zb));
        if zclip && depth_away(&[za, zb]) {
            hit(clause::D3DRASTER_DEPTHCLIP);
            continue;
        }
        // (a depth that varies along a multisample or antialiased line: not measured)
        // (MEASURED, WARP: probe-line-clip-z) a line across the depth range is cut where it
        // leaves it (0 <= z <= w), as clip_line
        let (sa, sb) = if behind && s.clip == SegClip::Line {
            match clip_seg(s.a, s.b, d.rast[6] != 0) {
                Some(v) => v,
                None => continue,
            }
        } else if zclip {
            hit(clause::D3DRASTER_CLIPZ_LINE);
            match clip_line_z(s.a, s.b) {
                Some(v) => v,
                None => continue,
            }
        } else {
            (s.a, s.b)
        };
        let clipped = match s.clip {
            SegClip::Line if behind => Some((sa, sb)),
            SegClip::Line => clip_line(sa, sb),
            SegClip::Done => Some((sa, sb)),
        };
        let Some((a, b)) = clipped else {
            hit(clause::D3DRASTER_CLIP);
            continue;
        };
        let zs = |v: &Vert| {
            let [_, _, _, _, minz, maxz] = tg.vp;
            vertex_depth(minz, maxz, v.pos[2], v.pos[3])
        };
        let (pa, pb) = (snapped(tg.vp, &a.pos), snapped(tg.vp, &b.pos));
        if pa == pb {
            // (no length: it exits no diamond)
            hit(clause::D3DRASTER_DEGENERATE);
            continue;
        }
        let major = major_of(pa, pb);
        let cover = Cover::of(pa, pb, major);
        ls.push(Line { a: pa, b: pb, ca: a.color, cb: b.color, iwa: 1.0 / a.pos[3], iwb: 1.0 / b.pos[3], lead: s.lead.color, prim: s.prim, front: s.front, w: constant_clip_w(&[a.pos[3], b.pos[3]]), major, cover, z: [zs(&a), zs(&b)] });
    }
    for l in &ls {
        let (x0, x1) = (l.a.x.min(l.b.x).div_euclid(256) - 3, l.a.x.max(l.b.x).div_euclid(256) + 3);
        let (y0, y1) = (l.a.y.min(l.b.y).div_euclid(256) - 3, l.a.y.max(l.b.y).div_euclid(256) + 3);
        let (x0, y0) = (x0.max(0), y0.max(0));
        let (x1, y1) = (x1.min(i64::from(tg.size.0) - 1), y1.min(i64::from(tg.size.1) - 1));
        for py in y0..=y1 {
            for px in x0..=x1 {
                let (quad, mask) = match (mode, tg.samples) {
                    (LineMode::Aliased, _) if l.cover.covers((px, py)) => (None, None),
                    (LineMode::Aliased, _) => continue,
                    (LineMode::Quad, None) => match l.quad((px, py)) {
                        Some(p) => (Some(p), None),
                        None => continue,
                    },
                    // (on a multisampled target the samples the quad covers, D3DRASTER.MS.LINES)
                    (LineMode::Quad, Some(pattern)) => match l.quad_at((px, py), pattern) {
                        (mask, Some(p)) => (Some(p), Some(line_samples(true, &mask))),
                        (_, None) => continue,
                    },
                    (LineMode::Alpha, None) => match l.tris_at(l.alpha_tris(), (px, py), &[(0, 0)]).1 {
                        Some(p) => (Some(p), None),
                        None => continue,
                    },
                    // (MEASURED, WARP: probe-rt-ms4-linesaa) on a multisampled target the
                    // pixel center tells, for every sample
                    (LineMode::Alpha, Some(pattern)) => match l.tris_at(l.alpha_tris(), (px, py), &[(0, 0)]).1 {
                        Some(p) => (Some(p), Some(line_samples(false, &vec![false; pattern.len()]))),
                        None => continue,
                    },
                };
                if !tg.drawable(d, (px, py)) {
                    continue;
                }
                // (MEASURED, WARP: probe-linesms-z, probe-linesaa-*) a multisample or alpha
                // line's depth: the covering triangle's plane of the corners' depths
                let z = match &quad {
                    Some(p) => {
                        hit(clause::D3DRASTER_LINE_DEPTH);
                        tg.clamp_depth(p.plane(p.zs, center_of(px), center_of(py)))
                    }
                    None => l.depth(tg, (px, py)),
                };
                let out = match (ps, &quad) {
                    ("ps", None) => {
                        hit(clause::D3DRASTER_LINE_COLOR);
                        l.color((px, py)).map(f32::to_bits)
                    }
                    ("ps", Some(p)) => p.at(center_of(px), center_of(py)).map(f32::to_bits),
                    _ => prim_out(ps, l.lead, (px, py), z, l.pos_w((px, py)), l.prim, l.front)?,
                };
                let out = match mode {
                    // (the shader's alpha times the coverage, before the output merger)
                    LineMode::Alpha => {
                        let mut out = out;
                        out[3] = (f32::from_bits(out[3]) * l.alpha_cover((px, py))).to_bits();
                        out
                    }
                    LineMode::Aliased | LineMode::Quad => out,
                };
                match mask {
                    Some(mask) => put_samples(m, tg.dev, d, tg.view, tg.fmt, tg.sub, tg.dsv, (index(px), index(py), index(tg.size.0)), Some(z), None, out, &mask)?,
                    None => tg.put(m, d, (px, py), z, out)?,
                }
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
    // (MEASURED, WARP: probe-clip-w-nan, -inf) a coordinate NaN or infinite: nothing drawn
    if not_finite(tri.map(|v| v.pos)) {
        hit(clause::D3DRASTER_CLIPW_NAN);
        return Ok(());
    }
    if tri.iter().any(|v| !(v.pos[3] > 0.0)) {
        // (MEASURED, WARP: probe-clip-w) a vertex at w <= 0: the clipping to the viewport
        // planes in homogeneous coordinates takes it away; what is left with no area draws
        // nothing
        let poly = clip_xy(tri.map(|v| v.pos));
        if clipped_away(vp, &poly) {
            hit(clause::D3DRASTER_CLIPW_AWAY);
            return Ok(());
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
            let out = if ps == "psflat" {
                hit(clause::D3DRASTER_FLAT);
                flat_color(lead.color)
            } else {
                tri[0].color.map(f32::to_bits)
            };
            let [vx, vy, vw, vh, _, _] = vp;
            for py in 0..i64::from(rh) {
                for px in 0..i64::from(rw) {
                    let (cx, cy) = (center_of(px), center_of(py));
                    if cx < vx || cx >= vx + vw || cy < vy || cy >= vy + vh || !rules::scissor_holds(d.rast[7] != 0, d.scissor, (px, py)) {
                        continue;
                    }
                    let c = (sample_sub(px, 0), sample_sub(py, 0));
                    let lit = (1..q.len().saturating_sub(1)).any(|i| tri_covers([q[0], q[i], q[i + 1]], c));
                    if lit {
                        output(m, dev, d, view, rt_fmt, rt_sub, None, (index(px), index(py), index(rw)), None, out)?;
                    }
                }
            }
            return Ok(());
        }
        // (MEASURED, WARP: probe-clip-w3, probe-clip-w-cull2, -cull3, -pspos, -depth, -ms4) colors
        // that vary: the pieces of D3DRASTER.CLIP with their planes (D3DRASTER.INTERP.CLIP), each
        // culled by its own facing, depth tested and multisampled as any triangle
        hit(clause::D3DRASTER_CLIPW_AREA);
        let Some(poly) = clip_tri(tri, d.rast[6] != 0) else {
            return Ok(());
        };
        let culled = |cw: bool| rules::culls(d.rast[1], d.rast[2] != 0, cw);
        let pieces: Vec<([(i64, i64); 3], Planes, bool)> = clip_pieces(&poly, vp, d.rast[6] != 0).into_iter().filter(|(_, _, cw)| !culled(*cw)).collect();
        // (MEASURED, WARP: probe-clip-w-wire, every value) wireframe: the clipped polygon's
        // edges but those along a clip plane, as lines from the vertices the clipping gives,
        // not clipped again (D3DRASTER.CLIPW.WIRE); (probe-clip-w-wire-cull2, -cull3) culled
        // by the pieces' facing, as filled
        if d.rast[0] == 2 {
            hit(clause::D3DRASTER_CLIPW_WIRE);
            let front = pieces.first().is_some_and(|p| rules::front_face(d.rast[2] != 0, p.2));
            for [a, b] in clipped_wire_edges(!pieces.is_empty(), poly.iter().map(|v| v.along)) {
                let a = poly.get(a).ok_or(Fail::Bad)?;
                let b = poly.get(b).ok_or(Fail::Bad)?;
                wire.push(Seg { a: Vert { pos: a.pos, color: a.color }, b: Vert { pos: b.pos, color: b.color }, lead, prim, front, clip: SegClip::Done });
            }
            return Ok(());
        }
        let [vx, vy, vw, vh, minz, maxz] = vp;
        let covering = |c: (i64, i64)| pieces.iter().find(|(t, _, _)| tri_covers(*t, c));
        for py in 0..i64::from(rh) {
            for px in 0..i64::from(rw) {
                let (cx, cy) = (center_of(px), center_of(py));
                if cx < vx || cx >= vx + vw || cy < vy || cy >= vy + vh || !rules::scissor_holds(d.rast[7] != 0, d.scissor, (px, py)) {
                    continue;
                }
                let c = (sample_sub(px, 0), sample_sub(py, 0));
                let at = (index(px), index(py), index(rw));
                let Some(pattern) = samples else {
                    if let Some((_, p, cw)) = covering(c) {
                        let z = clamp_range(minz, maxz, p.plane(p.zs, cx, cy));
                        let out = shade(m, ps, tri, Some(p), lead, prim, rules::front_face(d.rast[2] != 0, *cw), (px, py), Some(z), d)?;
                        output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, Some(z), out)?;
                    }
                    continue;
                };
                // (each piece a triangle of its own: shaded for the samples it covers)
                let sample = |(ox, oy): (i64, i64)| (sample_sub(px, ox), sample_sub(py, oy));
                for (t, p, cw) in &pieces {
                    let mask: Vec<bool> = pattern.iter().map(|&o| tri_covers(*t, sample(o))).collect();
                    if !mask.contains(&true) {
                        continue;
                    }
                    let z = clamp_range(minz, maxz, p.plane(p.zs, cx, cy));
                    let each: Vec<f32> = pattern.iter().map(|&offset| {
                        sample_depth_plane(p.xs, p.ys, p.zs, (minz, maxz), (cx, cy), offset)
                    }).collect();
                    let out = shade(m, ps, tri, Some(p), lead, prim, rules::front_face(d.rast[2] != 0, *cw), (px, py), Some(z), d)?;
                    put_samples(m, dev, d, view, rt_fmt, rt_sub, dsv, at, Some(z), Some(&each), out, &mask)?;
                }
            }
        }
        return Ok(());
    }
    // depth clipping: whole triangles in these cases
    let zs: Vec<f32> = tri.iter().map(|v| v.pos[2] / v.pos[3]).collect();
    let inside = |z: f32| (0.0..=1.0).contains(&z);
    let mut zclip = false;
    if d.rast[6] != 0 && !zs.iter().all(|z| inside(*z)) {
        if depth_away(&zs) {
            hit(clause::D3DRASTER_DEPTHCLIP);
            return Ok(());
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
    let area = rules::area(p[0], p[1], p[2]);
    if area == 0 {
        hit(clause::D3DRASTER_DEGENERATE);
        return Ok(());
    }
    // (y down: a positive area is clockwise on the screen)
    let cw = area > 0;
    let front = rules::front_face(d.rast[2] != 0, cw);
    hit(clause::D3DRASTER_CULL);
    if rules::culls(d.rast[1], d.rast[2] != 0, cw) {
        return Ok(());
    }
    // (MEASURED, WARP: probe-wire-*, probe-wireframe) wireframe: the triangle's edges as
    // lines, from the vertex of least clip x on: 0 to 1, 1 to 2, 2 to 0
    if d.rast[0] == 2 {
        hit(clause::D3DRASTER_WIREFRAME);
        for [a, b] in wire_edges(tri.map(|v| v.pos[0])) {
            let a = *tri.get(a).ok_or(Fail::Bad)?;
            let b = *tri.get(b).ok_or(Fail::Bad)?;
            wire.push(Seg { a, b, lead, prim, front, clip: SegClip::Line });
        }
        return Ok(());
    }
    let v = if cw { [p[0], p[1], p[2]] } else { [p[0], p[2], p[1]] };
    // (MEASURED, WARP: a triangle reaching past the viewport is clipped to it first, in clip
    // space, and the pieces' new vertices snapped as any other, tri-2: so an edge moves by up
    // to the snapping of its new end)
    let Some(poly) = clip_tri(tri, zclip) else {
        hit(clause::D3DRASTER_CLIP);
        return Ok(());
    };
    if zclip {
        hit(clause::D3DRASTER_CLIPZ_PIECES);
    }
    let unclipped = poly.iter().all(|v| !v.made);
    let planes = unclipped.then(|| Planes::new(tri, |q| {
        let (x, y) = snap2(q);
        (pixels(x), pixels(y))
    }, |q| vertex_depth(minz, maxz, q[2], q[3])));
    // (MEASURED, WARP: probe-clip-z-pos) a triangle cut by the depth range takes its depth from
    // the whole triangle's plane (D3DRASTER.INTERP.DEPTH)
    let zplanes = zclip.then(|| {
        hit(clause::D3DRASTER_CLIPZ_DEPTH);
        Planes::new(tri, |q| {
            let (x, y) = snap2(q);
            (pixels(x), pixels(y))
        }, |q| vertex_depth(minz, maxz, q[2], q[3]))
    });
    // the pieces, turned for the edge tests, with their planes (the whole triangle's, or the
    // fan triangle's as it comes, D3DRASTER.INTERP.CLIP)
    let pieces: Vec<([(i64, i64); 3], Option<Planes>)> = if unclipped {
        vec![(v, None)]
    } else {
        hit(clause::D3DRASTER_CLIP);
        clip_pieces(&poly, vp, zclip).into_iter().map(|(t, p, _)| (t, Some(p))).collect()
    };
    // depth: one value per triangle in these cases
    let z = {
        let zz: Vec<f32> = zs.iter().map(|z| minz + z * (maxz - minz)).collect();
        if zz.iter().any(|x| *x != zz[0]) {
            None
        } else {
            Some(clamp_range(minz, maxz, zz[0]))
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
    // (the scissor rectangle bounds the pixels asked, each then tested by rules::scissor_holds)
    if d.rast[7] != 0 {
        hit(clause::D3DRASTER_SCISSOR);
        x0 = x0.max(d.scissor[0]);
        y0 = y0.max(d.scissor[1]);
        x1 = x1.min(d.scissor[2]);
        y1 = y1.min(d.scissor[3]);
    }
    for py in y0..y1 {
        for px in x0..x1 {
            let (cx, cy) = (center_of(px), center_of(py));
            if cx < vx || cx >= vx + vw || cy < vy || cy >= vy + vh || !rules::scissor_holds(d.rast[7] != 0, d.scissor, (px, py)) {
                continue;
            }
            let covering = |c: (i64, i64)| pieces.iter().find(|(t, _)| tri_covers(*t, c));
            let inside = |c: (i64, i64)| covering(c).is_some();
            // the planes there: the whole triangle's, or the covering piece's
            let here = |c: (i64, i64)| planes.as_ref().or_else(|| covering(c).and_then(|(_, p)| p.as_ref()));
            let at = (index(px), index(py), index(rw));
            let Some(pattern) = samples else {
                if !inside((sample_sub(px, 0), sample_sub(py, 0))) {
                    continue;
                }
                hit(clause::D3DRASTER_TOPLEFT);
                // (a depth that varies over an unclipped triangle: its plane at the pixel
                // center, as the attributes' (D3DRASTER.INTERP.DEPTH))
                let c = (sample_sub(px, 0), sample_sub(py, 0));
                let z = z.or_else(|| {
                    let p = planes.as_ref().or(zplanes.as_ref()).or_else(|| here(c))?;
                    hit(clause::D3DRASTER_INTERP_DEPTH);
                    Some(clamp_range(minz, maxz, p.plane(p.zs, cx, cy)))
                });
                let out = shade(m, ps, tri, here(c), lead, prim, front, (px, py), z, d)?;
                output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, out)?;
                continue;
            };
            // (MEASURED, WARP: probe-rt-ms*-edges) a pixel is shaded once, at its center,
            // when a sample is covered; the covered samples take the value
            hit(clause::D3DRASTER_MS_SAMPLES);
            let mask: Vec<bool> = pattern.iter().map(|&(ox, oy)| inside((sample_sub(px, ox), sample_sub(py, oy)))).collect();
            if !mask.contains(&true) {
                continue;
            }
            let p = pattern.iter().find_map(|&(ox, oy)| here((sample_sub(px, ox), sample_sub(py, oy))));
            let pl = here((sample_sub(px, 0), sample_sub(py, 0))).or(p);
            // (MEASURED, WARP: probe-rt-ms4-zplane) a depth that varies: its plane at each
            // sample, for the depth test; at the pixel center for the shader
            let q = planes.as_ref().or(zplanes.as_ref()).or(pl);
            let each: Option<Vec<f32>> = match (z, q) {
                (None, Some(q)) => {
                    hit(clause::D3DRASTER_MS_ZPLANE);
                    Some(pattern.iter().map(|&offset| {
                        sample_depth_plane(q.xs, q.ys, q.zs, (minz, maxz), (cx, cy), offset)
                    }).collect())
                }
                (Some(_), _) | (None, None) => None,
            };
            let z = z.or_else(|| q.map(|q| clamp_range(minz, maxz, q.plane(q.zs, cx, cy))));
            let out = shade(m, ps, tri, pl, lead, prim, front, (px, py), z, d)?;
            put_samples(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, each.as_deref(), out, &mask)?;
        }
    }
    Ok(())
}

/// a pixel's value into the covered samples of a multisampled target (D3DRASTER.MS.SAMPLES):
/// all of them while every sample is alike, else per sample, each through the pixel's own
/// output (its value put in the subresource's place for the blend, then taken back)
#[allow(clippy::too_many_arguments)]
fn put_samples(m: &mut D3dres, dev: Dev, d: &Draw, view: &super::d3dres::View, rt_fmt: &'static Format, rt_sub: u32, dsv: Option<&super::d3dres::View>, at: (usize, usize, usize), z: Option<f32>, each: Option<&[f32]>, out: [u32; 4], mask: &[bool]) -> Res<()> {
    let (px, py) = (at.0, at.1);
    let n = mask.len();
    let stored = |m: &D3dres, res: Id<Resource>, sub: u32| -> Res<bool> { Ok(m.res.get(res)?.samples.get(sub as usize).is_some_and(Option::is_some)) };
    // the color target's place, and the depth view's (MEASURED, WARP: probe-rt-ms4-depth, depth
    // per sample: each sample tested and written as its own)
    let mut slots = vec![Slot::of(m, view.res, rt_sub, rt_fmt, (px, py))?];
    if let Some(dv) = dsv {
        let df = format(dv.format()).ok_or(Fail::Unmodeled(open::D3DRASTER_DEPTHFMT))?;
        let dres = m.res.get(dv.res)?;
        // (a depth view of another sample count is unbound, D3DRASTER.MS.DEPTHCOUNT)
        if dres.d.count.max(1) as usize != n {
            return Err(Fail::Bad);
        }
        let dsub = super::d3dres::view_subs(dev, dv, dres)[0];
        // (a depth view smaller than the target, on Direct3D 10: nothing drawn past it,
        // D3DRASTER.DEPTH.SMALL)
        let (dw, dh, _) = dres.size(dres.mip_of(dsub));
        if past_depth(dw, dh, px, py) {
            hit(clause::D3DRASTER_DEPTH_SMALL);
            return Ok(());
        }
        slots.push(Slot::of(m, dv.res, dsub, df, (px, py))?);
    }
    let kept = slots.iter().map(|s| stored(m, s.res, s.sub)).collect::<Res<Vec<bool>>>()?;
    // (one depth for every sample, or none tested)
    let alike = each.is_none() || dsv.is_none();
    if mask.iter().all(|&k| k) && !kept.contains(&true) && alike {
        output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, z, out)?;
        return Ok(());
    }
    if dsv.is_some() {
        hit(clause::D3DRASTER_MS_DEPTH);
    }
    for s in &slots {
        s.keep(m, n)?;
    }
    // (each covered sample through the pixel's own output: its values put in the
    // subresources' places for the blend and the depth test, then taken back)
    for (k, depth) in rules::sample_depth_jobs(mask, z, each) {
        for s in &slots {
            s.swap(m, n, k, true)?;
        }
        output(m, dev, d, view, rt_fmt, rt_sub, dsv, at, depth, out)?;
        for s in &slots {
            s.swap(m, n, k, false)?;
        }
    }
    // (the subresources' own places keep sample 0)
    for s in &slots {
        s.swap(m, n, 0, true)?;
    }
    Ok(())
}

/// one element of a subresource kept per sample (D3DRASTER.MS.SAMPLES)
struct Slot {
    res: Id<Resource>,
    sub: u32,
    off: usize,
    el: usize,
}

impl Slot {
    fn of(m: &D3dres, res: Id<Resource>, sub: u32, f: &Format, (px, py): (usize, usize)) -> Res<Slot> {
        let r = m.res.get(res)?;
        let (rb, _, _) = r.layout(sub)?;
        let el = (f.bits / 8) as usize;
        let off = py * rb + px * el;
        let len = r.subs.get(sub as usize).and_then(Option::as_ref).ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?.len();
        // (a view's elements are its resource's: views keep the format group)
        if off + el > len {
            return Err(Fail::Bad);
        }
        Ok(Slot { res, sub, off, el })
    }

    /// the subresource's samples, made from its one value at first (the model keeps up to 64 MiB
    /// of them)
    fn keep(&self, m: &mut D3dres, n: usize) -> Res<()> {
        let r = m.res.get_mut(self.res)?;
        let base = r.subs[self.sub as usize].as_ref().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
        if base.len().saturating_mul(n) > 64 << 20 {
            return Err(Fail::Unmodeled(open::D3DRES_DESC_BIGBYTES));
        }
        let base = base.clone();
        if r.samples.len() <= self.sub as usize {
            r.samples.resize(self.sub as usize + 1, None);
        }
        r.samples[self.sub as usize].get_or_insert_with(|| base.repeat(n));
        Ok(())
    }

    /// sample k into the subresource's place (in) or back from it
    fn swap(&self, m: &mut D3dres, n: usize, k: usize, into: bool) -> Res<()> {
        let r = m.res.get_mut(self.res)?;
        let slot = index(self.sub);
        let sub = r.subs.get_mut(slot).ok_or(Fail::Bad)?;
        let (Some(sub), Some(Some(samples))) = (sub, r.samples.get_mut(slot)) else {
            return Ok(());
        };
        let (start, end) = rules::sample_span(samples.len(), n, k, self.off, self.el).ok_or(Fail::Bad)?;
        let place_end = self.off.checked_add(self.el).ok_or(Fail::Bad)?;
        let place = sub.get_mut(self.off..place_end).ok_or(Fail::Bad)?;
        let sample = samples.get_mut(start..end).ok_or(Fail::Bad)?;
        if into {
            place.copy_from_slice(sample);
        } else {
            sample.copy_from_slice(place);
        }
        Ok(())
    }
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
    /// the vertices' depths on the viewport's range (D3DRASTER.INTERP.DEPTH)
    zs: [f32; 3],
}

impl Planes {
    fn new(tri: &[Vert; 3], screen: impl Fn(&[f32; 4]) -> (f32, f32), depth: impl Fn(&[f32; 4]) -> f32) -> Planes {
        // (the last of the least clip x, as a stable rotation: MEASURED, WARP, probe-filter-float,
        // -unorm16, where two share it)
        let k0 = (0..3).fold(0, |k, i| if tri[i].pos[0] <= tri[k].pos[0] { i } else { k });
        let t: [&Vert; 3] = [0, 1, 2].map(|i| &tri[(k0 + i) % 3]);
        let s = t.map(|v| screen(&v.pos));
        let iw = t.map(|v| 1.0 / v.pos[3]);
        let aw = [0, 1, 2].map(|i| t[i].color.map(|a| a * iw[i]));
        Planes { xs: s.map(|p| p.0), ys: s.map(|p| p.1), iw, aw, zs: t.map(|v| depth(&v.pos)) }
    }

    fn plane(&self, a: [f32; 3], cx: f32, cy: f32) -> f32 {
        plane_value(self.xs, self.ys, a, cx, cy)
    }

    fn at(&self, cx: f32, cy: f32) -> [f32; 4] {
        hit(clause::D3DRASTER_INTERP);
        perspective_at(self.xs, self.ys, self.iw, self.aw, (cx, cy))
    }

    fn uv(&self, center: (f32, f32)) -> [f32; 2] {
        hit(clause::D3DRASTER_INTERP);
        perspective_at(self.xs, self.ys, self.iw, self.aw.map(|[u, v, _, _]| [u, v]), center)
    }
}

/// Evaluate attributes already multiplied by reciprocal w and remove that factor at the sample.
pub fn perspective_at<const N: usize>(
    xs: [f32; 3], ys: [f32; 3], iw: [f32; 3], aw: [[f32; N]; 3], (cx, cy): (f32, f32),
) -> [f32; N] {
    let q = 1.0 / plane_value(xs, ys, iw, cx, cy);
    std::array::from_fn(|k| plane_value(xs, ys, aw.map(|a| a[k]), cx, cy) * q)
}

/// D3DRASTER.INTERP.PLANE: an attribute's plane over a triangle (xs, ys its vertices in
/// pixels, a its values) at (cx, cy): f32 gradients times 1 / the doubled area (exact in f64,
/// then to f32), C = (a0 - gx x0) - gy y0, the value (C + gy cy) + gx cx
pub fn plane_value(xs: [f32; 3], ys: [f32; 3], a: [f32; 3], cx: f32, cy: f32) -> f32 {
    let ([x0, x1, x2], [y0, y1, y2]) = (xs, ys);
    let area = (f64::from(y2 - y0) * f64::from(x1 - x0) - f64::from(y0 - y1) * f64::from(x0 - x2)) as f32;
    let r = 1.0 / area;
    let (d1, d2) = (a[1] - a[0], a[2] - a[0]);
    let gx = (d1 * (y2 - y0) - d2 * (y1 - y0)) * r;
    let gy = (d2 * (x1 - x0) - d1 * (x2 - x0)) * r;
    let c = (a[0] - gx * x0) - gy * y0;
    (c + gy * cy) + gx * cx
}

/// D3DRASTER.MS.ZPLANE: evaluate the depth at the sample, then clamp it to the viewport.
pub(crate) fn sample_depth_plane(
    xs: [f32; 3], ys: [f32; 3], zs: [f32; 3], (minz, maxz): (f32, f32),
    (cx, cy): (f32, f32), (ox, oy): (i64, i64),
) -> f32 {
    let depth = plane_value(xs, ys, zs, cx + sixteenths(ox), cy + sixteenths(oy));
    clamp_range(minz, maxz, depth)
}

/// D3DRASTER.LINE.COLOR, LINE.DEPTH: a value along a line's major axis (its ends a, b in 1/256
/// pixels, values va, vb) at the coordinate c: gradient (vb - va) (1 / (b - a)), C = va -
/// gradient a, C + gradient c, in f32
pub fn line_value(a: i64, b: i64, c: f32, va: f32, vb: f32) -> f32 {
    let (a, b) = (pixels(a), pixels(b));
    let g = 1.0 / (b - a);
    let grad = (vb - va) * g;
    let c0 = va - grad * a;
    c0 + grad * c
}

/// D3DRASTER.INTERP.DEPTH, LINE.DEPTH: a depth onto the viewport's range minz..maxz, clamped
/// (max then min: a range of NaN leaves the value)
pub fn clamp_range(minz: f32, maxz: f32, z: f32) -> f32 {
    let (lo, hi) = if minz <= maxz { (minz, maxz) } else { (maxz, minz) };
    z.max(lo).min(hi)
}

/// D3DRASTER.VIEWPORT: a clip position through the viewport transform (x, y, width, height),
/// in 1/256 pixels rounded half to even: x + (x/w + 1) (width/2), y + (1 - y/w) (height/2)
pub fn viewport_xy(vp: [f32; 6], q: &[f32; 4]) -> (i64, i64) {
    let [vx, vy, vw, vh, _, _] = vp;
    (snap256(vx + (q[0] / q[3] + 1.0) * (vw * 0.5)), snap256(vy + (1.0 - q[1] / q[3]) * (vh * 0.5)))
}

/// D3DRASTER.INTERP.DEPTH: a vertex's depth on the viewport's range, minz + z (1/w) (maxz -
/// minz) in f32
pub fn vertex_depth(minz: f32, maxz: f32, z: f32, w: f32) -> f32 {
    minz + z * (1.0 / w) * (maxz - minz)
}

/// D3DRASTER.VP10: Direct3D 10's viewport origin and size, the executor's (INT) cast of them
pub fn vp10(x: f32) -> f32 {
    x.trunc()
}

/// D3D11_FILTER_REDUCTION_TYPE, the filter's bits 7 and 8
#[derive(Clone, Copy, PartialEq, Debug)]
enum Reduction {
    Standard,
    Comparison,
    Minimum,
    Maximum,
}

impl Reduction {
    fn of(f: u32) -> Reduction {
        match (f >> 7 & 1 != 0, f >> 8 & 1 != 0) {
            (false, false) => Reduction::Standard,
            (true, false) => Reduction::Comparison,
            (false, true) => Reduction::Minimum,
            (true, true) => Reduction::Maximum,
        }
    }

    fn pick(self) -> Option<Pick> {
        match self {
            Reduction::Standard | Reduction::Comparison => None,
            Reduction::Minimum => Some(Pick::Least),
            Reduction::Maximum => Some(Pick::Greatest),
        }
    }
}

/// (MEASURED, WARP: probe-sample-min, -max) a minimum or maximum filter: per component the
/// least or greatest of the 2 x 2 texels (a, b on the upper row, c, e on the lower) whose
/// weight is not 0 (the weights fu, fv in 1/256)
fn reduce(pick: Pick, t: [[f32; 4]; 4], w: (i64, i64)) -> [f32; 4] {
    hit(clause::D3DRASTER_FILTER_MINMAX);
    rules::reduce(pick, t, w)
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

/// a + (b - a) t as SSE computes it, NaNs included: an operation's result is its first operand
/// when that is a NaN, else its second, quieted (MEASURED, WARP: probe-filter-r11g11b10, 72
/// NaN values of differing payloads)
pub fn lerp_sse(a: f32, b: f32, t: f32) -> f32 {
    let op = |x: f32, y: f32, r: f32| {
        let quiet = |n: f32| f32::from_bits(n.to_bits() | 0x40_0000);
        if x.is_nan() { quiet(x) } else if y.is_nan() { quiet(y) } else { r }
    };
    let d = op(b, a, b - a);
    let m = op(d, t, d * t);
    op(a, m, a + m)
}

/// how a format's texels are filtered linearly
#[derive(Clone, Copy, PartialEq, Eq)]
enum FilterPath {
    /// 8-bit UNORM and the block formats decoded to bytes: D3DRASTER.FILTER.LINEAR
    Fixed,
    /// as f32: D3DRASTER.FILTER.FLOAT
    Wide,
}

/// (MEASURED, WARP: probe-filter-*) the path of a format's linear filtering; None where not
/// measured (integer formats end the executor)
fn filter_path(id: u32) -> Option<FilterPath> {
    let f = format(id)?;
    match (f.num, id) {
        // R8G8B8A8, R8G8, R8, A8, B8G8R8A8, B8G8R8X8 UNORM; BC1, BC2, BC3, BC7 UNORM
        (_, 28 | 49 | 61 | 65 | 87 | 88 | 71 | 74 | 77 | 98) => Some(FilterPath::Fixed),
        // (MEASURED, WARP: probe-filter-fmt68, -fmt69, every pixel) R8G8_B8G8 and G8R8_G8B8, each
        // texel its pair's red and blue with its own green, filtered as floats
        (_, 68 | 69) => Some(FilterPath::Wide),
        // R11G11B10, R9G9B9E5, B5G6R5, B5G5R5A1, B4G4R4A4; BC1 sRGB, BC4 and BC5 UNORM and SNORM,
        // BC6H; R24_UNORM_X8 and R32_FLOAT_X8X24 (depth formats' views)
        (_, 26 | 67 | 85 | 86 | 115 | 72 | 80 | 81 | 83 | 84 | 95 | 46 | 21) => Some(FilterPath::Wide),
        (Num::Float | Num::Srgb | Num::Snorm, _) if f.comps.iter().all(|c| matches!(c, 8 | 16 | 32)) => Some(FilterPath::Wide),
        (Num::Unorm, _) if f.comps.iter().all(|&c| c == 16) || f.comps == [10, 10, 10, 2] => Some(FilterPath::Wide),
        _ => None,
    }
}

/// the level or levels a sample reads
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
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

/// (MEASURED, WARP: probe-filter-*, 2425 values) a bilinear result n in 1/128 of a byte (n of
/// 32640 for 1.0) as a float: the f32 nearest n ceil(2^46 / 32640) / 2^46, a hair above
/// n / 32640 (seven values at 0.498 ulp past an f32 round up)
pub fn linear_float(n: i64) -> f32 {
    const M: i64 = (1 << 46) / 32640 + 1;
    const _: () = assert!(M * 32640 > 1 << 46 && (M - 1) * 32640 < 1 << 46);
    ((n * M) as f64 / (1u64 << 46) as f64) as f32
}

/// the clip planes, in the order WARP clips against them (D3DRASTER.CLIP)
#[derive(Clone, Copy, PartialEq, Eq)]
enum ClipPlane {
    Near,
    Far,
    Left,
    Right,
    Top,
    Bottom,
}

const CLIP_PLANES: [ClipPlane; 6] = [ClipPlane::Near, ClipPlane::Far, ClipPlane::Left, ClipPlane::Right, ClipPlane::Top, ClipPlane::Bottom];

impl ClipPlane {
    /// the distance from the plane, inside when not below 0
    fn dist(self, q: &[f32; 4]) -> f32 {
        match self {
            ClipPlane::Near => q[2],
            ClipPlane::Far => q[3] - q[2],
            ClipPlane::Left => q[3] + q[0],
            ClipPlane::Right => q[3] - q[0],
            ClipPlane::Top => q[3] - q[1],
            ClipPlane::Bottom => q[3] + q[1],
        }
    }

    fn depth(self) -> bool {
        match self {
            ClipPlane::Near | ClipPlane::Far => true,
            ClipPlane::Left | ClipPlane::Right | ClipPlane::Top | ClipPlane::Bottom => false,
        }
    }
}

/// a vertex's distances from the clip planes, one per plane
#[derive(Clone, Copy)]
struct Dists {
    near: f32,
    far: f32,
    left: f32,
    right: f32,
    top: f32,
    bottom: f32,
}

impl Dists {
    fn of(q: &[f32; 4]) -> Dists {
        Dists { near: ClipPlane::Near.dist(q), far: ClipPlane::Far.dist(q), left: ClipPlane::Left.dist(q), right: ClipPlane::Right.dist(q), top: ClipPlane::Top.dist(q), bottom: ClipPlane::Bottom.dist(q) }
    }

    fn get(&self, p: ClipPlane) -> f32 {
        match p {
            ClipPlane::Near => self.near,
            ClipPlane::Far => self.far,
            ClipPlane::Left => self.left,
            ClipPlane::Right => self.right,
            ClipPlane::Top => self.top,
            ClipPlane::Bottom => self.bottom,
        }
    }

    fn set(&mut self, p: ClipPlane, v: f32) {
        let d = match p {
            ClipPlane::Near => &mut self.near,
            ClipPlane::Far => &mut self.far,
            ClipPlane::Left => &mut self.left,
            ClipPlane::Right => &mut self.right,
            ClipPlane::Top => &mut self.top,
            ClipPlane::Bottom => &mut self.bottom,
        };
        *d = v;
    }
}

/// a vertex of a clipped polygon: the triangle's own, or one the clipping made
#[derive(Clone, Copy)]
struct ClipVert {
    pos: [f32; 4],
    color: [f32; 4],
    dist: Dists,
    made: bool,
    /// the edge from this vertex to the next lies along a clip plane
    along: bool,
}

/// a value of a vertex the clipping makes, from the inside end: f32(((out - in) as f64 dIn) /
/// (dIn - dOut) + in) (clip_tri, clip_seg)
pub fn clip_lerp(din: f32, dout: f32, vi: f32, vo: f32) -> f32 {
    ((f64::from(vo - vi) * f64::from(din)) / f64::from(din - dout) + f64::from(vi)) as f32
}

/// (MEASURED, WARP: probe-clip-w-line) a line with an end at w not positive, clipped as a
/// triangle's edge is (clip_tri): the planes some end is outside of in CLIP_PLANES' order, the
/// new end from the inside one in f64, the later planes' distances carried
fn clip_seg(a: Vert, b: Vert, zclip: bool) -> Option<(Vert, Vert)> {
    let mut ends = [a, b].map(|v| ClipVert { pos: v.pos, color: v.color, dist: Dists::of(&v.pos), made: false, along: false });
    let planes: Vec<ClipPlane> = CLIP_PLANES.into_iter().filter(|p| zclip || !p.depth()).collect();
    let out = |v: &ClipVert, p: ClipPlane| 0.0 > v.dist.get(p);
    let cut: Vec<ClipPlane> = planes.into_iter().filter(|&p| ends.iter().any(|v| out(v, p))).collect();
    for (k, &p) in cut.iter().enumerate() {
        let [e0, e1] = ends;
        let (i, o, first) = match (out(&e0, p), out(&e1, p)) {
            (true, true) => return None,
            (false, false) => continue,
            (false, true) => (e0, e1, false),
            (true, false) => (e1, e0, true),
        };
        let (din, dout) = (i.dist.get(p), o.dist.get(p));
        let lerp = |vi: f32, vo: f32| clip_lerp(din, dout, vi, vo);
        let mut dist = i.dist;
        for q in cut.iter().skip(k).skip(1) {
            dist.set(*q, lerp(i.dist.get(*q), o.dist.get(*q)));
        }
        let made = ClipVert { pos: i.pos.map(|_| 0.0), color: i.color, dist, made: true, along: false };
        let made = ClipVert { pos: zip4(i.pos, o.pos, lerp), color: zip4(i.color, o.color, lerp), ..made };
        ends = if first { [made, e1] } else { [e0, made] };
    }
    let [e0, e1] = ends;
    Some((Vert { pos: e0.pos, color: e0.color }, Vert { pos: e1.pos, color: e1.color }))
}

/// (MEASURED, WARP: probe-interp-clip*, probe-interp-*) a triangle clipped Sutherland-Hodgman
/// against the planes some vertex is outside of (near and far with depth clipping on), in
/// CLIP_PLANES' order, after turning it so the least clip x (then y) comes first; inside is
/// not below 0; a new vertex runs from the inside end: each value (position, color, the later
/// planes' distances) f32(((out - in) as f64 dIn) / (dIn - dOut) + in); None when every vertex
/// is outside one plane, the triangle as turned when none is outside any
fn clip_tri(tri: &[Vert; 3], zclip: bool) -> Option<Vec<ClipVert>> {
    let less = |a: &[f32; 4], b: &[f32; 4]| a[0] > b[0] || a[0] == b[0] && a[1] > b[1];
    let mut sel = if less(&tri[0].pos, &tri[1].pos) { 1 } else { 0 };
    if less(&tri[sel].pos, &tri[2].pos) {
        sel = 2;
    }
    let planes: Vec<ClipPlane> = CLIP_PLANES.into_iter().filter(|p| zclip || !p.depth()).collect();
    let mut poly: Vec<ClipVert> = (0..3)
        .map(|i| {
            let v = &tri[(sel + i) % 3];
            ClipVert { pos: v.pos, color: v.color, dist: Dists::of(&v.pos), made: false, along: false }
        })
        .collect();
    let out = |v: &ClipVert, p: ClipPlane| 0.0 > v.dist.get(p);
    if planes.iter().any(|&p| poly.iter().all(|v| out(v, p))) {
        return None;
    }
    let cut: Vec<ClipPlane> = planes.into_iter().filter(|&p| poly.iter().any(|v| out(v, p))).collect();
    for (k, &p) in cut.iter().enumerate() {
        let mut next = Vec::new();
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let (ina, inb) = (!out(&a, p), !out(&b, p));
            if ina {
                next.push(a);
            }
            if ina != inb {
                let (i, o) = if ina { (a, b) } else { (b, a) };
                let (din, dout) = (i.dist.get(p), o.dist.get(p));
                let lerp = |vi: f32, vo: f32| clip_lerp(din, dout, vi, vo);
                let mut dist = i.dist;
                for q in &cut[k + 1..] {
                    dist.set(*q, lerp(i.dist.get(*q), o.dist.get(*q)));
                }
                // (leaving: the edge on to the vertex where the polygon comes back runs along the
                // plane; entering: the rest of the edge a to b)
                next.push(ClipVert { pos: [0, 1, 2, 3].map(|c| lerp(i.pos[c], o.pos[c])), color: [0, 1, 2, 3].map(|c| lerp(i.color[c], o.color[c])), dist, made: true, along: ina || a.along });
            }
        }
        poly = next;
        if poly.is_empty() {
            return None;
        }
    }
    Some(poly)
}

/// a clipped polygon's fan triangles (D3DRASTER.CLIP), turned for the edge tests, with their
/// planes (D3DRASTER.INTERP.CLIP); those of no area left out
fn clip_pieces(poly: &[ClipVert], vp: [f32; 6], zclip: bool) -> Vec<([(i64, i64); 3], Planes, bool)> {
    hit(clause::D3DRASTER_INTERP_CLIP);
    let [_, _, _, _, minz, maxz] = vp;
    let cross = rules::area;
    let at: Vec<((i64, i64), f32, f32)> = poly
        .iter()
        .map(|c| {
            if c.made {
                made_screen(vp, &c.pos, zclip)
            } else {
                let s = snapped(vp, &c.pos);
                ((s.x, s.y), vertex_depth(minz, maxz, c.pos[2], c.pos[3]), 1.0 / c.pos[3])
            }
        })
        .collect();
    // (a fan from the first vertex: each piece's three corners)
    let pieces = at.iter().zip(poly).skip(1).zip(at.iter().zip(poly).skip(2));
    let first = at.first().zip(poly.first());
    first
        .into_iter()
        .flat_map(|a| pieces.clone().map(move |(b, c)| [a, b, c]))
        .filter(|k| cross(k[0].0 .0, k[1].0 .0, k[2].0 .0) != 0)
        .map(|k| {
            let t = k.map(|v| v.0 .0);
            let cw = cross(t[0], t[1], t[2]) > 0;
            let t = if cw { t } else { [t[0], t[2], t[1]] };
            let iw = k.map(|v| v.0 .2);
            let p = Planes { xs: k.map(|v| pixels(v.0 .0 .0)), ys: k.map(|v| pixels(v.0 .0 .1)), iw, aw: k.map(|v| v.1.color.map(|a| a * v.0 .2)), zs: k.map(|v| v.0 .1) };
            (t, p, cw)
        })
        .collect()
}

/// (MEASURED, WARP: probe-interp-clip*) a vertex the clipping made, on the screen: w' =
/// max(w, the least normal f32), x and y clamped to -w'..w' (z to 0..w' with depth clipping),
/// 1/w' by division, X = round_even(((x/w') W/2 + (W/2 + left)) 256), Y with -H/2 and H/2 + top;
/// depth (z/w') (maxz - minz) + minz
pub fn made_screen(vp: [f32; 6], q: &[f32; 4], zclip: bool) -> ((i64, i64), f32, f32) {
    let [vx, vy, vw, vh, minz, maxz] = vp;
    let w = if f32::MIN_POSITIVE > q[3] { f32::MIN_POSITIVE } else { q[3] };
    let clamp = |v: f32| {
        let m = if v > -w { v } else { -w };
        if w > m { if v > -w { v } else { -w } } else { w }
    };
    let (x, y) = (clamp(q[0]), clamp(q[1]));
    let z = if zclip { q[2].max(0.0).min(w) } else { q[2] };
    let rhw = 1.0 / w;
    let (sx, ox, sy, oy) = (vw * 0.5, vw * 0.5 + vx, -vh * 0.5, vh * 0.5 + vy);
    let snap = snap_even;
    ((snap((rhw * x) * sx + ox), snap((rhw * y) * sy + oy)), (rhw * z) * (maxz - minz) + minz, rhw)
}

/// D3DRASTER.DEPTHCLIP: depths (z / w) wholly in front of 0 or wholly past 1
pub fn depth_away(zs: &[f32]) -> bool {
    zs.iter().all(|z| *z < 0.0) || zs.iter().all(|z| *z > 1.0)
}

/// D3DRASTER.CLIPW.NAN: a triangle with a coordinate NaN or infinite
pub fn not_finite(tri: [[f32; 4]; 3]) -> bool {
    tri.iter().any(|q| q.iter().any(|x| !x.is_finite()))
}

/// D3DRASTER.CLIPW.AWAY: what the homogeneous clipping leaves (every w positive) has no area
/// once snapped
pub fn clipped_away(vp: [f32; 6], poly: &[[f32; 4]]) -> bool {
    if !poly.iter().all(|q| q[3] > 0.0) {
        return false;
    }
    let q: Vec<(i64, i64)> = poly.iter().map(|q| viewport_xy(vp, q)).collect();
    let Some(&first) = q.first() else { return true };
    q.windows(2).skip(1).all(|e| rules::area(first, e[0], e[1]) == 0)
}

/// D3DRASTER.CLIP: a triangle's vertices (position, color) clipped as clip_tri does: the
/// polygon's vertices (position, color, made by the clipping, its edge on along a plane)
pub fn clip_tri_values(tri: [([f32; 4], [f32; 4]); 3], zclip: bool) -> Option<Vec<([f32; 4], [f32; 4], bool, bool)>> {
    let t = tri.map(|(pos, color)| Vert { pos, color });
    clip_tri(&t, zclip).map(|p| p.iter().map(|c| (c.pos, c.color, c.made, c.along)).collect())
}

/// D3DRASTER.LINE, CLIPZ.LINE: a line's ends (position, color) clipped to the x and y planes
/// (z false) or to the depth range (z true)
pub fn clip_line_values(a: ([f32; 4], [f32; 4]), b: ([f32; 4], [f32; 4]), z: bool) -> Option<[([f32; 4], [f32; 4]); 2]> {
    let (a, b) = (Vert { pos: a.0, color: a.1 }, Vert { pos: b.0, color: b.1 });
    let r = if z { clip_line_z(a, b) } else { clip_line(a, b) };
    r.map(|(a, b)| [(a.pos, a.color), (b.pos, b.color)])
}

/// a triangle clipped to -w <= x <= w and -w <= y <= w (Sutherland-Hodgman, in single
/// precision: a new vertex a + (b - a) t, t the distance to the plane at a over the difference)
pub fn clip_xy(tri: [[f32; 4]; 3]) -> Vec<[f32; 4]> {
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
pub fn clip_z(mut poly: Vec<[f32; 4]>) -> Vec<[f32; 4]> {
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
            // (every drawn triangle has its planes, D3DRASTER.INTERP.PLANE and .CLIP)
            Err(Fail::Bad)
        }
    };
    Ok(match ps {
        "ps" => match planes {
            Some(p) => {
                hit(clause::D3DRASTER_INTERP_PLANE);
                p.at(center_of(px), center_of(py)).map(f32::to_bits)
            }
            None => same(&|v| v.color)?.map(f32::to_bits),
        },
        "psflat" => {
            hit(clause::D3DRASTER_FLAT);
            flat_color(lead.color)
        }
        "pspos" => {
            hit(clause::D3DRASTER_POS);
            // (MEASURED: w reads as the vertices' clip w, pos-3, pos-4)
            let w = match (constant_clip_w(&tri.map(|v| v.pos[3])), planes) {
                (Some(w), _) => w,
                // (MEASURED, WARP: probe-zplane-1, -3) w that varies: 1 / the plane of 1/w
                (None, Some(p)) => {
                    hit(clause::D3DRASTER_POSW_PLANE);
                    position_w_from_reciprocal(p.plane(p.iw, center_of(px), center_of(py)))
                }
                (None, None) => return Err(Fail::Bad),
            };
            if w.to_bits() != 1.0f32.to_bits() {
                hit(clause::D3DRASTER_POSW);
            }
            // (a drawn triangle's depth is its value or its plane's, D3DRASTER.INTERP.DEPTH)
            let z = z.ok_or(Fail::Bad)?;
            position_color((px, py), z, w)
        }
        "psprim" => {
            hit(clause::D3DRASTER_PRIM);
            primitive_id_color(prim, front)
        }
        "psld" | "psldu" => {
            hit(clause::D3DRASTER_LD);
            // (an unbound shader resource reads as zeros, ld-89)
            let sv = match rules::srv_or_zero(d.srv) {
                Ok(view) => view,
                Err(zero) => {
                    hit(clause::D3DRASTER_NOSRV);
                    return Ok(zero);
                }
            };
            let v = m.views.get(sv)?;
            let r = m.res.get(v.res)?;
            // (the view's level 0 and slice 0: its most detailed level, its first slice, of the
            // resource's, ld's mip and array index 0 against the view, D3D11_TEX2D_SRV)
            let reach = super::d3dres::view_reach(m.dev.ok_or(Fail::Bad)?, v, r);
            if reach.mip != 0 || reach.first != 0 {
                hit(clause::D3DRASTER_LD_VIEW);
            }
            let sub = reach.subs(r).first().copied().ok_or(Fail::Bad)?;
            let (w, h, _) = r.size(reach.mip);
            match rules::ld_reach((px, py), (w, h), v.format(), sub) {
                Ok(at) => texel(r, at.subresource, at.format, at.x, at.y)?,
                Err(zero) => {
                    hit(clause::D3DRASTER_LD_OUT);
                    zero
                }
            }
        }
        "pssample" | "pssamplec" => {
            hit(clause::D3DRASTER_SAMPLE);
            // (an unbound shader resource reads as zeros, ld-89)
            let sv = match rules::srv_or_zero(d.srv) {
                Ok(view) => view,
                Err(zero) => {
                    hit(clause::D3DRASTER_NOSRV);
                    return Ok(zero);
                }
            };
            // (MEASURED, WARP: probe-sample-nosampler) no sampler bound: the default one,
            // linear filtering, clamped addresses
            if d.sampler.is_none() {
                hit(clause::D3DRASTER_NOSAMPLER);
            }
            let s = sampler_or_default(d.sampler);
            let v = m.views.get(sv)?;
            let r = m.res.get(v.res)?;
            // (MEASURED, WARP: probe-filter-view-mips, -view-mips-mag) level 0 for
            // the footprint and level selection is the view's most detailed mip.
            let reach = rules::sample_reach(m.dev.ok_or(Fail::Bad)?, v, r).ok_or(Fail::Bad)?;
            let mips = reach.mips;
            if reach.base != 0 || mips != r.d.mips {
                hit(clause::D3DRASTER_FILTER_VIEW);
            }
            // Other supported multi-mip dimensions include arrays, cubes, 3D
            // (first depth slice) and 1D (one row).
            if r.d.mips > 1 && v.dim() != 4 {
                hit(clause::D3DRASTER_FILTER_ARRAY);
            }
            let (w, h, _) = r.size(reach.base);
            let uv_at = |x: i64, y: i64| -> Res<[f32; 2]> {
                match planes {
                    Some(p) => Ok(p.uv((center_of(x), center_of(y)))),
                    None => same(&|v| v.color).map(|c| [c[0], c[1]]),
                }
            };
            let uv = uv_at(px, py)?;
            // (MEASURED, WARP: probe-sample-cmp-*) sample_c: each texel's red against the
            // reference, the color's z, by the sampler's comparison function, 1 or 0, filtered as
            // floats
            let cmp = match (ps == "pssamplec", s[0] & 0x80 != 0) {
                (true, true) => {
                    hit(clause::D3DRASTER_SAMPLE_CMP);
                    let reference = match planes {
                        Some(p) => p.at(center_of(px), center_of(py))[2],
                        None => same(&|v| v.color)?[2],
                    };
                    Some((reference, s[4]))
                }
                // (MEASURED, WARP: probe-samplec-nocmp) through a sampler without comparison:
                // zeros
                (true, false) => {
                    hit(clause::D3DRASTER_SAMPLE_CMP);
                    return Ok([0; 4]);
                }
                // (MEASURED, WARP: probe-sample-cmp) sample through a comparison sampler filters
                // as its filter without the comparison
                (false, _) => None,
            };
            let reduction = Reduction::of(s[0]);
            let filter = Filter::of(s[0] & 0x7f).ok_or(Fail::Bad)?;
            // (the minifying and magnifying filters differ: which applies is by the texels a
            // pixel spans, as the 2 x 2 quad's differences give them)
            // (MEASURED, WARP: probe-filter-aniso, -aniso-min, -aniso-2, -4, -16, -aniso-mips,
            // -aniso-mips-2) anisotropic filtering is the linear one, between levels too, the level
            // by the longer span
            if s[0] == 0x55 {
                hit(clause::D3DRASTER_FILTER_ANISO);
            }
            let (linear, level) = if filter.min == filter.mag && mips == 1 {
                (filter.mag, Level::One(0))
            } else {
                let (qx, qy) = (px & !1, py & !1);
                let q = [uv_at(qx, qy)?, uv_at(qx + 1, qy)?, uv_at(qx, qy + 1)?];
                let rho = quad_span(q, w, h);
                // (MEASURED, WARP: probe-filter-min, -min-point, -rho-*) a span of up to one texel
                // magnifies, past it minifies (1 - 2^-10, 1, 1 + 2^-10, 1 + 2^-6, 1.1, 1.25);
                // (probe-filter-mips, -mips-point, -trilinear*) past one texel the level is log2
                // of the span (f64::log2, the platform's libm)
                match filter_pick(mips, s[0] & 1 == 0, rho, rho.log2()) {
                    FilterPick::Mag => {
                        hit(clause::D3DRASTER_FILTER_MINMAG);
                        (filter.mag, Level::One(0))
                    }
                    FilterPick::Min => {
                        hit(clause::D3DRASTER_FILTER_MINMAG);
                        (filter.min, Level::One(0))
                    }
                    FilterPick::Mip(level) => {
                        hit(clause::D3DRASTER_FILTER_LEVEL);
                        (filter.min, level)
                    }
                }
            };
            let one = |level: u32| -> Res<Filtered> {
                let (mip, sub) = reach.at(level).ok_or(Fail::Bad)?;
                let (w, h, _) = r.size(mip);
                let at = |i: i64, j: i64| -> Res<[u32; 4]> {
                    match (address(s[1], i, w as i64), address(s[2], j, h as i64)) {
                        (Some(x), Some(y)) => {
                            let t = texel(r, sub, v.format(), x as u32, y as u32)?;
                            Ok(match cmp {
                                None => t,
                                Some((reference, func)) => {
                                    [sample_cmp(func, reference, f32::from_bits(t[0])).to_bits(); 4]
                                }
                            })
                        }
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
                    return at(point_texel(uv[0], w), point_texel(uv[1], h)).map(Filtered::Point);
                }
                hit(clause::D3DRASTER_FILTER_LINEAR);
                let ((i, wu), (j, wv)) = (split(uv[0], w), split(uv[1], h));
                // (MEASURED, WARP: probe-filter-float, -unorm16, -r10, -srgb, -snorm, every value) float
                // texels, 16-bit and 10-10-10-2 UNORM, sRGB (linear) and SNORM ones (as their floats): in f32, between the rows first, l = a + (c -
                // a) fv, r = b + (d - b) fv, then l + (r - l) fu, with the weights fu, fv the
                // 1/256 below the fraction
                // (MEASURED, WARP: probe-filter-r8g8b8a8-uint, golden 878bb10, x86 and x86-64) an integer
                // format's view sampled through a float one ends the executor
                let integer = format(v.format()).is_some_and(|f| matches!(f.num, Num::Uint | Num::Sint));
                if linear_filter_crashes(integer, cmp.is_some()) {
                    hit(clause::D3DRASTER_FILTER_INT);
                    return Err(Fail::Crash);
                }
                let path = if cmp.is_some() { FilterPath::Wide } else { filter_path(v.format()).ok_or(Fail::Bad)? };
                if path == FilterPath::Wide {
                    hit(clause::D3DRASTER_FILTER_FLOAT);
                    if matches!(v.format(), 68 | 69) {
                        hit(clause::D3DRASTER_FILTER_PAIRS);
                    }
                    let (fu, fv) = (pixels(wu), pixels(wv));
                    let f = |t: [u32; 4]| t.map(f32::from_bits);
                    let (a, b, c, e) = (f(at(i, j)?), f(at(i + 1, j)?), f(at(i, j + 1)?), f(at(i + 1, j + 1)?));
                    if let Some(pick) = reduction.pick() {
                        return Ok(Filtered::Wide(reduce(pick, [a, b, c, e], (wu, wv))));
                    }
                    return Ok(Filtered::Wide([0, 1, 2, 3].map(|k| {
                        let l = lerp_sse(a[k], c[k], fv);
                        let r = lerp_sse(b[k], e[k], fv);
                        lerp_sse(l, r, fu)
                    })));
                }
                let bytes = |t: [u32; 4]| t.map(|b| (f32::from_bits(b) * 255.0).round() as i64);
                let (a, b, c, e) = (bytes(at(i, j)?), bytes(at(i + 1, j)?), bytes(at(i, j + 1)?), bytes(at(i + 1, j + 1)?));
                // (MEASURED, WARP: probe-sample-min-unorm8) of 8-bit UNORM texels the least or
                // greatest byte
                if let Some(pick) = reduction.pick() {
                    let w = |t: [i64; 4]| t.map(|n| n as f32);
                    return Ok(Filtered::Fixed(reduce(pick, [w(a), w(b), w(c), w(e)], (wu, wv)).map(|n| n as i64 * 128)));
                }
                Ok(Filtered::Fixed([0, 1, 2, 3].map(|k| rules::bilinear8(a[k], b[k], c[k], e[k], wu, wv))))
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
                // (MEASURED, WARP: probe-sample-max-mips) a minimum or maximum filter between two
                // levels: the least or greatest of the two levels' values
                Level::Two(l, _) if let Some(pick) = reduction.pick() => {
                    let take = |x: f32, y: f32| match pick {
                        Pick::Least => x.min(y),
                        Pick::Greatest => x.max(y),
                    };
                    match (one(l)?, one(l + 1)?) {
                        (Filtered::Wide(a), Filtered::Wide(b)) => done(Filtered::Wide([0, 1, 2, 3].map(|k| take(a[k], b[k])))),
                        (Filtered::Fixed(a), Filtered::Fixed(b)) => done(Filtered::Fixed([0, 1, 2, 3].map(|k| match pick {
                            Pick::Least => a[k].min(b[k]),
                            Pick::Greatest => a[k].max(b[k]),
                        }))),
                        (Filtered::Point(a), Filtered::Point(b)) => zip4(a, b, |a, b| take(f32::from_bits(a), f32::from_bits(b)).to_bits()),
                        (Filtered::Point(_) | Filtered::Wide(_) | Filtered::Fixed(_), _) => return Err(Fail::Bad),
                    }
                }
                Level::Two(l, f) => match (one(l)?, one(l + 1)?) {
                    (Filtered::Fixed(a), Filtered::Fixed(b)) => mip_linear(done(Filtered::Fixed(zip4(a, b, |a, b| rules::mip8(a, b, f))))),
                    (Filtered::Wide(a), Filtered::Wide(b)) => mip_linear(done(Filtered::Wide(zip4(a, b, |a, b| lerp_sse(a, b, pixels(f)))))),
                    (Filtered::Point(a), Filtered::Point(b)) => {
                        let (a, b) = (a.map(f32::from_bits), b.map(f32::from_bits));
                        mip_linear(done(Filtered::Wide(zip4(a, b, |a, b| lerp_sse(a, b, pixels(f))))))
                    }
                    (Filtered::Point(_) | Filtered::Wide(_) | Filtered::Fixed(_), _) => return Err(Fail::Bad),
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
        let dsub = super::d3dres::view_subs(dev, dv, m.res.get(dv.res)?)[0];
        let dres = m.res.get_mut(dv.res)?;
        let (rb, _, _) = dres.layout(dsub)?;
        let el = (df.bits / 8) as usize;
        let (dw, dh, _) = dres.size(dres.mip_of(dsub));
        let b = dres.subs[dsub as usize].as_mut().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
        // (a depth view smaller than the target, on Direct3D 10: nothing drawn past it,
        // D3DRASTER.DEPTH.SMALL)
        if past_depth(dw, dh, x, y) {
            hit(clause::D3DRASTER_DEPTH_SMALL);
            return Ok(());
        }
        let e = b.get_mut(y * rb + x * el..).and_then(|r| r.get_mut(..el)).ok_or(Fail::Bad)?;
        let test = if d.depthst[0] != 0 {
            // (only a triangle with a vertex at w <= 0 comes without depth, and with no depth view)
            let z = z.ok_or(Fail::Bad)?;
            Some((quantized_depth(df, z), stored_depth(df, e), z))
        } else {
            None
        };
        match rules::depth_action(d.depthst[2], d.depthst[1] != 0, test) {
            DepthAction::Reject => return Ok(()),
            DepthAction::Keep => {}
            DepthAction::Write(z) => write_depth(df, e, true, z, false, 0)?,
        }
    }
    // (a depth view alone: no color to write, D3DRASTER.DEPTH.ALONE)
    if !matches!(rules::target_plan(d.target, d.depth), TargetPlan::Color(_)) {
        return Ok(());
    }
    let res = m.res.get_mut(view.res)?;
    let (rb, rows, _) = res.layout(sub)?;
    let target = render_rows(dev, res, view, sub)?;
    let (slice, first) = (index(target.slice), index(target.first));
    let el = (f.bits / 8) as usize;
    let b = res.subs[sub as usize].as_mut().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
    // (a view whose elements are wider than its resource's, as mutations make: not measured)
    let e = b.get_mut(slice * rows * rb + y * rb + (first + x) * el..).and_then(|r| r.get_mut(..el)).ok_or(Fail::Bad)?;
    let blend_enabled = d.blend.0[0] != 0;
    let integer = matches!(f.num, Num::Uint | Num::Sint);
    let new = match color_write(integer, blend_enabled) {
        ColorWrite::Integer => {
            if blend_enabled {
                hit(clause::D3DRASTER_BLEND_INT);
            }
            encode_bits(f, out)
        }
        ColorWrite::Raw => encode(f, out.map(f32::from_bits))?,
        ColorWrite::Blended => {
            let s = out.map(f32::from_bits);
            encode(f, blend(&d.blend.0, d.blend.1, f, s, decode_f(f, e)?)?)?
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
