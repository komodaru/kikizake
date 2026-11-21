//! Direct3D 10, 10.1 and 11 resources and drawing (SPEC.md "Direct3D 10 and 11 resources"): an
//! executable specification of which resource and view descriptions the devices accept and
//! what GetDesc gives back (src/model/d3dres_check.rs), of what clears, updates, copies and
//! resolves leave in the subresources, and of drawing (src/model/d3draster.rs).
//!
//! Sources (Microsoft Learn): "D3D11_TEXTURE2D_DESC" and the 1D / 3D / buffer ones,
//! "ID3D11DeviceContext::UpdateSubresource" (the box, the pitches), "CopyResource" and
//! "CopySubresourceRegion" (compatible formats: one typeless group, or a block compressed format
//! and the uncompressed format of its block's size), "ClearRenderTargetView",
//! "ClearDepthStencilView" (the depth clamped to 0..1), "ClearUnorderedAccessViewUint" (the low
//! bits of each value, no conversion; raw and structured buffers: the first value),
//! "ClearUnorderedAccessViewFloat" (float, unorm and snorm views only; others: not run),
//! "ResolveSubresource", "Data Conversion Rules" (src/model/dxgiconv.rs).

use crate::spec::{clause, open};
use super::dxgiconv::{decode, encode, encode_bits};
use super::dxgifmt::{format, Format, Num};
use super::{filled, filled_mut, Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub use super::d3dres_check::*;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Dev {
    /// device11 at a feature level
    D11(u32),
    /// device10 (d3d10.dll)
    D10,
    /// device10_1 at a feature level
    D101(u32),
}

impl Dev {
    pub fn d3d11(self) -> bool {
        matches!(self, Dev::D11(_))
    }
    pub fn level(self) -> u32 {
        match self {
            Dev::D11(l) | Dev::D101(l) => l,
            Dev::D10 => 0xa000,
        }
    }
}

/// a resource: its description (as GetDesc gives it) and each subresource's bytes, rows of
/// elements packed (None: not known)
#[derive(Clone, Debug)]
pub struct Resource {
    pub d: Desc,
    pub subs: Vec<Option<Vec<u8>>>,
    /// a multisampled subresource's samples, once a draw covers part of a pixel: d.count
    /// planes laid out as the subresource's bytes, plane k sample k (none: every sample is
    /// the subresource's value)
    pub samples: Vec<Option<Vec<u8>>>,
}

#[derive(Clone, Debug)]
pub struct View {
    pub kind: u32,
    pub res: usize,
    /// Format, ViewDimension, [Flags], the union words (as GetDesc gives them)
    pub words: Vec<u32>,
}

impl View {
    pub fn format(&self) -> u32 {
        self.words[0]
    }
    pub fn dim(&self) -> u32 {
        self.words[1]
    }
    /// the union words (after Format, ViewDimension and the 11 dsv's Flags)
    pub fn u(&self, i: usize, d3d11: bool) -> u32 {
        let head = if self.kind == 2 && d3d11 { 3 } else { 2 };
        self.words.get(head + i).copied().unwrap_or(0)
    }
}

/// the bytes `fill` and initial data give: xorshift32 from the seed
pub fn xorbytes(seed: u32, n: usize) -> Vec<u8> {
    let mut r = crate::rng::XorShift32::new(seed);
    (0..n).map(|_| r.next() as u8).collect()
}

/// the initial data the executor passes: 4 MiB from xorshift32
pub const INIT: usize = 4 << 20;

impl Resource {
    pub fn fmt(&self) -> Option<&'static Format> {
        format(self.d.fmt)
    }
    /// the format of a texture (create takes only formats of the table for one)
    pub fn texture_format(&self) -> Res<&'static Format> {
        self.fmt().ok_or(Fail::Bad)
    }
    /// (width, height, depth) of mip m
    pub fn size(&self, m: u32) -> (u32, u32, u32) {
        let d = &self.d;
        match d.dim {
            4 => (d.w, 1, 1),
            1 => ((d.w >> m).max(1), 1, 1),
            2 => ((d.w >> m).max(1), (d.h >> m).max(1), 1),
            _ => ((d.w >> m).max(1), (d.h >> m).max(1), (d.da >> m).max(1)),
        }
    }
    pub fn mip_of(&self, sub: u32) -> u32 {
        sub % self.d.mips.max(1)
    }
    pub fn nsubs(&self) -> u32 {
        if self.d.dim == 4 {
            1
        } else {
            self.d.mips * if self.d.dim == 3 { 1 } else { self.d.da }
        }
    }
    /// (row bytes, rows, slices) of a subresource
    pub fn layout(&self, sub: u32) -> Res<(usize, usize, usize)> {
        if self.d.dim == 4 {
            return Ok((self.d.w as usize, 1, 1));
        }
        let f = self.texture_format()?;
        let (w, h, d) = self.size(self.mip_of(sub));
        Ok((f.row_bytes(w) as usize, f.rows(h) as usize, d as usize))
    }
    /// the bytes of every subresource of a description
    pub fn zeroed_bytes(d: &Desc) -> Res<u64> {
        let r = Resource { d: *d, subs: Vec::new(), samples: Vec::new() };
        (0..r.nsubs()).try_fold(0u64, |total, s| {
            let (rb, rows, sl) = r.layout(s)?;
            Ok(total.saturating_add([rb, rows, sl].iter().map(|&x| u64::try_from(x).unwrap_or(u64::MAX)).fold(1u64, u64::saturating_mul)))
        })
    }

    pub fn zeroed(d: Desc) -> Res<Resource> {
        let mut r = Resource { d, subs: Vec::new(), samples: Vec::new() };
        r.subs = (0..r.nsubs())
            .map(|s| {
                let (rb, rows, sl) = r.layout(s)?;
                Ok(Some(vec![0; rb * rows * sl]))
            })
            .collect::<Res<_>>()?;
        Ok(r)
    }
}

#[derive(Default)]
pub struct D3dres {
    states: super::d3d11::D3d11,
    pub dev: Option<Dev>,
    pub res: [Option<Resource>; 16],
    pub views: [Option<View>; 16],
    pub draw: super::d3draster::Draw,
    /// (an update or copy past a subresource removed the device)
    pub removed: bool,
}

crate::model::words! {
    /// the resource commands
    pub enum ResOp { Fmt = "fmt", Tex = "tex", Buf = "buf", Srv = "srv", Rtv = "rtv", Dsv = "dsv", Uav = "uav", Clear = "clear", Cleards = "cleards", Clearuav = "clearuav", Update = "update", Copy = "copy", Copysub = "copysub", Resolve = "resolve", Read = "read" }
}

/// where a command goes: drawing (d3draster) or resources (here)
#[derive(Clone, Copy, PartialEq, Eq)]
enum Route {
    Draw(super::d3draster::DrawOp),
    Res(ResOp),
}

fn n(t: &[String], i: usize) -> Res<u32> {
    t.get(i).and_then(|s| num(s)).map(|v| v as u32).ok_or(Fail::Bad)
}

fn slot(t: &[String], i: usize) -> Res<usize> {
    let v = t.get(i).and_then(|s| num(s)).ok_or(Fail::Bad)?;
    if !(0..16).contains(&v) {
        return Err(Fail::Bad);
    }
    Ok(v as usize)
}

/// whether CopyResource may copy between the formats: the same typeless group, or (10.1 and
/// later) a block compressed format and the uncompressed one of its block's bits
pub fn copy_compatible(a: u32, b: u32, dev: Dev) -> bool {
    hit(clause::D3DRES_COPY_FORMAT);
    if group(a) == group(b) {
        return true;
    }
    let (Some(fa), Some(fb)) = (format(a), format(b)) else { return false };
    if dev == Dev::D10 {
        return false;
    }
    // (MEASURED: R9G9B9E5_SHAREDEXP copies to and from R32_UINT, copyfmt-67-42; not
    // R11G11B10_FLOAT, copyfmt-26-42)
    if matches!((a, b), (67, 41..=43) | (41..=43, 67)) {
        return true;
    }
    let pair = |c: &Format, u: &Format| c.block() && !u.block() && c.bits == u.bits && matches!(u.num, Num::Uint | Num::Sint | Num::Float | Num::Typeless) && u.comps.len() >= 2 && u.comps.iter().all(|&x| x == u.comps[0]);
    pair(fa, fb) || pair(fb, fa)
}

/// the typeless group of a format (its first member)
pub fn group(f: u32) -> u32 {
    const STARTS: &[u32] = &[1, 5, 9, 15, 19, 23, 26, 27, 33, 39, 44, 48, 53, 60, 65, 66, 67, 68, 69, 70, 73, 76, 79, 82, 85, 86, 87, 88, 89, 90, 92, 94, 97, 100];
    let mut g = 0;
    for &s in STARTS {
        if f >= s {
            g = s;
        }
    }
    // (one group each from here on; R10G10B10_XR_BIAS_A2 is its own)
    if f >= 100 {
        return f;
    }
    match f {
        87 => 90,
        91 => 90,
        88 | 93 => 92,
        _ => g,
    }
}

impl D3dres {
    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let name = t[0].as_str();
        match name {
            "device11" => {
                *self = D3dres::default();
                let fl = if t.len() > 1 { n(t, 1)? } else { 0xb000 };
                if t.len() > 2 {
                    return Err(Fail::Bad);
                }
                if fl == 0 {
                    out.push("device11 failed 80070057".into());
                    return Ok(true);
                }
                self.dev = Some(Dev::D11(fl));
                return self.states.op(op, out);
            }
            "device10" | "device10_1" => {
                *self = D3dres::default();
                self.dev = Some(if name == "device10" { Dev::D10 } else { Dev::D101(n(t, 1)?) });
                out.push(format!("{name} ok"));
                return Ok(true);
            }
            _ => {}
        }
        let route = match (super::d3draster::DrawOp::parse(name), ResOp::parse(name)) {
            (Some(d), _) => Route::Draw(d),
            (None, Some(r)) => Route::Res(r),
            (None, None) => return self.states.op(op, out),
        };
        let Some(dev) = self.dev else {
            if route == Route::Draw(super::d3draster::DrawOp::Vtx) {
                return super::d3draster::op(self, Dev::D10, super::d3draster::DrawOp::Vtx, op, out).map(|_| true);
            }
            hit(clause::D3DRES_NODEV);
            out.push("nodev".into());
            return Ok(true);
        };
        if self.removed {
            // (MEASURED: after the device is removed, the staging copy (11) or its Map (10)
            // fails with DXGI_ERROR_DEVICE_REMOVED, probe-update-past, probe-copysub-past; a
            // missing resource is still "nores")
            // (MEASURED: Direct3D 11 refuses new resources and views with it, 10 makes them;
            // everything else runs, unseen but by the reads, probe-removed)
            match name {
                "read" => {
                    if t.len() != 6 {
                        return Err(Fail::Bad);
                    }
                    if self.res[slot(t, 1)?].is_none() {
                        out.push("read nores".into());
                        return Ok(true);
                    }
                    hit(clause::D3DRES_REMOVED);
                    out.push(if dev.d3d11() { "read hr 887a0005" } else { "read map hr 887a0005" }.into());
                    return Ok(true);
                }
                "tex" | "buf" | "srv" | "rtv" | "dsv" | "uav" if dev.d3d11() => {
                    hit(clause::D3DRES_REMOVED_OPS);
                    if t.get(1).is_some_and(|s| s != "check") && t.get(3).is_none_or(|s| s != "check") {
                        let at = slot(t, 1)?;
                        if name == "tex" || name == "buf" {
                            self.res[at] = None;
                        } else {
                            self.views[at] = None;
                        }
                    }
                    out.push(format!("{name} hr 887a0005"));
                    return Ok(true);
                }
                _ => {}
            }
        }
        let cmd = match route {
            Route::Draw(d) => {
                super::d3draster::op(self, dev, d, op, out)?;
                return Ok(true);
            }
            Route::Res(r) => r,
        };
        // (samples that differ in a pixel, D3DRASTER.MS.SAMPLES: only clears, draws, resolves
        // and reads (zeros, D3DRES.READ.MS) are modeled on them)
        let at: &[usize] = match name {
            "update" => &[1],
            // (a whole subresource's copy without a box: D3DRES.COPYSUB.SAMPLES)
            "copysub" if t.len() == 8 && (3..6).all(|i| n(t, i).is_ok_and(|v| v == 0)) => &[],
            "copysub" => &[1, 6],
            _ => &[],
        };
        // (MEASURED, WARP: probe-rt-ms4-update, -copysub-box; "ID3D11DeviceContext::
        // UpdateSubresource", "CopySubresourceRegion": a multisampled resource takes neither,
        // nor a region of one) an update of a multisampled resource, or a region copy with a box
        // or an offset to or from one, changes nothing
        if at.iter().any(|&i| slot(t, i).ok().and_then(|k| self.res[k].as_ref()).is_some_and(|r| r.d.count > 1)) {
            hit(clause::D3DRES_MS_REGION);
            return Ok(true);
        }
        match cmd {
            ResOp::Fmt => {
                let f = n(t, 1)?;
                out.push(fmt_line(dev, f)?);
            }
            ResOp::Tex | ResOp::Buf => self.create(dev, t, out)?,
            ResOp::Srv => self.view(dev, 0, t, out)?,
            ResOp::Rtv => self.view(dev, 1, t, out)?,
            ResOp::Dsv => self.view(dev, 2, t, out)?,
            ResOp::Uav => self.view(dev, 3, t, out)?,
            ResOp::Clear | ResOp::Cleards | ResOp::Clearuav => self.clear(dev, t, out)?,
            ResOp::Update => {
                let r = slot(t, 1)?;
                let (sub, seed, rp, dp) = (n(t, 2)?, n(t, 3)?, n(t, 4)? as usize, n(t, 5)? as usize);
                let bx = if t.len() == 12 { Some((6..12).map(|i| n(t, i)).collect::<Res<Vec<u32>>>()?) } else if t.len() == 6 { None } else { return Err(Fail::Bad) };
                let Some(res) = self.res[r].as_mut() else {
                    out.push("update nores".into());
                    return Ok(true);
                };
                self.removed |= update(res, sub, &xorbytes(seed, INIT), rp, dp, bx.as_deref())?;
            }
            ResOp::Copy => {
                if t.len() != 3 {
                    return Err(Fail::Bad);
                }
                let (d, s) = (slot(t, 1)?, slot(t, 2)?);
                let (Some(_), Some(src)) = (&self.res[d], self.res[s].clone()) else {
                    out.push("copy nores".into());
                    return Ok(true);
                };
                let dst = filled_mut(&mut self.res, d)?;
                copy_resource(dev, dst, &src)?;
            }
            ResOp::Copysub => {
                let (d, ds, x, y, z, s, ss) = (slot(t, 1)?, n(t, 2)?, n(t, 3)?, n(t, 4)?, n(t, 5)?, slot(t, 6)?, n(t, 7)?);
                let bx = if t.len() == 14 { Some((8..14).map(|i| n(t, i)).collect::<Res<Vec<u32>>>()?) } else if t.len() == 8 { None } else { return Err(Fail::Bad) };
                let (Some(_), Some(src)) = (&self.res[d], self.res[s].clone()) else {
                    out.push("copysub nores".into());
                    return Ok(true);
                };
                let dst = filled_mut(&mut self.res, d)?;
                // (MEASURED, WARP: probe-rt-ms4-copysub) a whole multisampled subresource copies
                // with its samples on Direct3D 11, and not at all on Direct3D 10 (as
                // D3DRES.COPY)
                let ms = src.d.count > 1 && src.samples.iter().chain(&dst.samples).any(Option::is_some);
                if ms {
                    hit(clause::D3DRES_COPYSUB_SAMPLES);
                    if dev == Dev::D10 {
                        return Ok(true);
                    }
                }
                self.removed |= copy_region(dev, dst, ds, (x, y, z), &src, ss, bx.as_deref())?;
                if ms && dst.d.count == src.d.count && ds < dst.nsubs() && ss < src.nsubs() {
                    let v = src.samples.get(ss as usize).cloned().flatten();
                    if dst.samples.len() <= ds as usize {
                        dst.samples.resize(ds as usize + 1, None);
                    }
                    dst.samples[ds as usize] = v;
                }
            }
            ResOp::Resolve => {
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                let (d, ds, s, ss, f) = (slot(t, 1)?, n(t, 2)?, slot(t, 3)?, n(t, 4)?, n(t, 5)?);
                let (Some(_), Some(src)) = (&self.res[d], self.res[s].clone()) else {
                    out.push("resolve nores".into());
                    return Ok(true);
                };
                hit(clause::D3DRES_RESOLVE);
                let dst = filled_mut(&mut self.res, d)?;
                // (every sample holds the same value in these cases: the resolve is a copy)
                // (MEASURED: a single-sampled source changes nothing, probe-resolve-single)
                if src.d.count < 2 {
                    hit(clause::D3DRES_RESOLVE_IGNORED);
                    return Ok(true);
                }
                // (MEASURED: a multisampled destination, or a format of another group, changes
                // nothing, probe-resolve-ms-dst, probe-resolve-format)
                if dst.d.count != 1 || group(f) != group(src.d.fmt) || group(f) != group(dst.d.fmt) {
                    hit(clause::D3DRES_RESOLVE_IGNORED);
                    return Ok(true);
                }
                // (MEASURED: a missing subresource, or textures of other dimensions: nothing
                // changes, probe-resolve-dsub, probe-resolve-dim)
                if ds >= dst.nsubs() || ss >= src.nsubs() || dst.d.dim != src.d.dim {
                    hit(clause::D3DRES_RESOLVE_BAD);
                    return Ok(true);
                }
                // (MEASURED: sizes that differ resolve where both are, probe-resolve-size)
                let (srb, srows, _) = src.layout(ss)?;
                let (drb, drows, _) = dst.layout(ds)?;
                if (srb, srows) != (drb, drows) {
                    hit(clause::D3DRES_RESOLVE_SIZE);
                }
                let (Some(sv), Some(dv)) = (src.subs[ss as usize].as_ref(), dst.subs[ds as usize].as_mut()) else {
                    return Err(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN));
                };
                let w = srb.min(drb);
                // (MEASURED, WARP: probe-rt-ms*-edges, probe-rt-ms4-colors-*, -unorm16) samples
                // that differ resolve to their mean, by the format's Mean
                if let Some(Some(samp)) = src.samples.get(ss as usize) {
                    hit(clause::D3DRES_RESOLVE_MEAN);
                    let f = format(f).ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
                    let el = (f.bits / 8) as usize;
                    let mean = Mean::of(f).ok_or(Fail::Unmodeled(open::D3DRASTER_RTKIND))?;
                    let (n, plane) = (src.d.count as usize, sv.len());
                    for y in 0..srows.min(drows) {
                        for x in 0..w / el {
                            let at = |k: usize| k * plane + y * srb + x * el;
                            let e = mean.resolve(f, n, |k| &samp[at(k)..][..el])?;
                            dv[y * drb + x * el..][..el].copy_from_slice(&e);
                        }
                    }
                    return Ok(true);
                }
                for y in 0..srows.min(drows) {
                    dv[y * drb..][..w].copy_from_slice(&sv[y * srb..][..w]);
                }
            }
            ResOp::Read => {
                if t.len() != 6 {
                    return Err(Fail::Bad);
                }
                let r = slot(t, 1)?;
                let (sub, rb, rows, slices) = (n(t, 2)?, n(t, 3)? as usize, n(t, 4)? as usize, n(t, 5)? as usize);
                let Some(res) = &self.res[r] else {
                    out.push("read nores".into());
                    return Ok(true);
                };
                read(dev, res, sub, rb, rows, slices, out)?;
            }
        }
        Ok(true)
    }

    fn create(&mut self, dev: Dev, t: &[String], out: &mut Vec<String>) -> Res<()> {
        let check = t.get(1).is_some_and(|s| s == "check");
        let s = if check { 0 } else { slot(t, 1)? };
        let buf = t[0] == "buf";
        let (d, init) = if buf {
            let v: Vec<u32> = (2..8).map(|i| n(t, i)).collect::<Res<_>>()?;
            let init = match t.len() {
                8 => None,
                10 if t[8] == "init" => Some((n(t, 9)?, 0, 0)),
                _ => return Err(Fail::Bad),
            };
            (Desc { dim: 4, w: v[0], h: 1, da: 1, mips: 1, fmt: 0, usage: v[1], bind: v[2], cpu: v[3], misc: v[4], count: 1, q: v[5] }, init)
        } else {
            let v: Vec<u32> = (2..14).map(|i| n(t, i)).collect::<Res<_>>()?;
            if !(1..=3).contains(&v[0]) {
                return Err(Fail::Bad);
            }
            let init = match t.len() {
                14 => None,
                18 if t[14] == "init" => Some((n(t, 15)?, n(t, 16)?, n(t, 17)?)),
                _ => return Err(Fail::Bad),
            };
            (Desc { dim: v[0], w: v[1], h: v[2], da: v[3], mips: v[4], fmt: v[5], usage: v[6], bind: v[7], cpu: v[8], misc: v[9], count: v[10], q: v[11] }, init)
        };
        if !check {
            self.res[s] = None;
        }
        let kind = if buf { "buf" } else { "tex" };
        // (MEASURED: the x86 runtime dies on a texture of usage 0xffffffff, x86-usageff-0
        // line 2; an unknown usage with CPU access is refused, x86-usage-cpu-0; buffers
        // answer, buf-cbdyn-usage; x86-64 answers all)
        if super::arch() == 32 && !buf && d.usage == 0xffff_ffff {
            hit(clause::D3DRES_X86CRASH);
            return Err(Fail::Crash);
        }
        let got = check_desc(dev, &d, init.is_some())?;
        let g = match got {
            Err(hr) => {
                out.push(format!("{kind} hr {hr:08x}"));
                return Ok(());
            }
            Ok(g) => g,
        };
        if check {
            hit(clause::D3DRES_CHECK);
            out.push(format!("{kind} hr {S_FALSE:08x}"));
            return Ok(());
        }
        if let Some(hr) = create_refused(dev, &g) {
            out.push(format!("{kind} hr {hr:08x}"));
            return Ok(());
        }
        // (MEASURED: past the level's sizes it is made as checked, up to 1 GiB, big-make;
        // past that, big-make-huge)
        if !buf && super::d3dres_check::past_level_size(dev, &g) && Resource::zeroed_bytes(&g)? > 1 << 30 {
            return Err(Fail::Unmodeled(open::D3DRES_DESC_BIGBYTES));
        }
        let words = desc_words(dev, &g);
        out.push(format!("{kind} = {}", words.iter().map(|w| format!("{w:x}")).collect::<Vec<_>>().join(" ")));
        let mut r = Resource::zeroed(g)?;
        hit(clause::D3DRES_ZERO);
        if let Some((seed, rp, sp)) = init {
            let data = xorbytes(seed, INIT);
            for sub in 0..r.nsubs() {
                let (rb, rows, sl) = r.layout(sub)?;
                let (rp, sp) = if buf { (rb, rb) } else { (rp as usize, sp as usize) };
                let v = filled_mut(&mut r.subs, sub as usize)?;
                for z in 0..sl {
                    for y in 0..rows {
                        let at = z * sp + y * rp;
                        if at + rb > INIT {
                            return Err(Fail::Unmodeled(open::D3DRES_INIT_PAST));
                        }
                        v[(z * rows + y) * rb..][..rb].copy_from_slice(&data[at..at + rb]);
                    }
                }
            }
        }
        self.res[s] = Some(r);
        Ok(())
    }

    /// a view of `kind` (0 srv, 1 rtv, 2 dsv, 3 uav)
    fn view(&mut self, dev: Dev, kind: u32, t: &[String], out: &mut Vec<String>) -> Res<()> {
        let (v, r) = (slot(t, 1)?, slot(t, 2)?);
        let check = t.get(3).is_some_and(|s| s == "check");
        let at = 3 + check as usize;
        let nodesc = t.get(at).is_some_and(|s| s == "nodesc");
        let words = if nodesc {
            if t.len() != at + 1 {
                return Err(Fail::Bad);
            }
            None
        } else {
            if t.len() - at > 8 {
                return Err(Fail::Bad);
            }
            let mut w: Vec<u32> = (at..t.len()).map(|i| n(t, i)).collect::<Res<_>>()?;
            w.resize(8, 0);
            Some(w)
        };
        let Some(res) = &self.res[r] else {
            out.push(format!("{} nores", t[0]));
            return Ok(());
        };
        if kind == 3 && !dev.d3d11() {
            out.push("uav none".into());
            return Ok(());
        }
        if !check {
            self.views[v] = None;
        }
        match check_view(dev, kind, res, words.as_deref(), check)? {
            Err(hr) => out.push(format!("{} hr {hr:08x}", t[0])),
            Ok(_) if check => {
                hit(clause::D3DRES_CHECK);
                out.push(format!("{} hr {S_FALSE:08x}", t[0]));
            }
            Ok(w) => {
                out.push(format!("{} = {}", t[0], w.iter().map(|x| format!("{x:x}")).collect::<Vec<_>>().join(" ")));
                self.views[v] = Some(View { kind, res: r, words: w });
            }
        }
        Ok(())
    }

    fn clear(&mut self, dev: Dev, t: &[String], out: &mut Vec<String>) -> Res<()> {
        let v = slot(t, 1)?;
        let name = t[0].as_str();
        let want = match name {
            "clear" => 1,
            "cleards" => 2,
            _ => 3,
        };
        let len = [6, 5, 7][want as usize - 1];
        if t.len() != len {
            return Err(Fail::Bad);
        }
        if want == 3 && !(t[2] == "f" || t[2] == "u") {
            return Err(Fail::Bad);
        }
        let w: Vec<u32> = (if want == 3 { 3 } else { 2 }..len).map(|i| n(t, i)).collect::<Res<_>>()?;
        if want == 3 && !dev.d3d11() {
            out.push("clearuav none".into());
            return Ok(());
        }
        let Some(view) = self.views[v].clone().filter(|x| x.kind == want) else {
            out.push(format!("{name} noview"));
            return Ok(());
        };
        let res = filled_mut(&mut self.res, view.res)?;
        let vf = format(view.format()).or_else(|| res.fmt());
        let subs = view_subs(dev, &view, res);
        match want {
            1 => {
                hit(clause::D3DRES_CLEAR_RTV);
                let c = [0, 1, 2, 3].map(|i| f32::from_bits(w[i]));
                let f = vf.ok_or(Fail::Unmodeled(open::D3DRES_CLEAR_FORMAT))?;
                let el = encode(f, c)?;
                fill_subs(res, &subs, &el)?;
            }
            2 => {
                hit(clause::D3DRES_CLEAR_DSV);
                let (flags, depth, st) = (w[0], f32::from_bits(w[1]), w[2] as u8);
                // (MEASURED: a depth outside 0..1, NaN too, drops the whole call, even one that
                // clears only the stencil; cleards-*)
                if !(0.0..=1.0).contains(&depth) {
                    hit(clause::D3DRES_CLEAR_DEPTHRANGE);
                    return Ok(());
                }
                let f = vf.ok_or(Fail::Unmodeled(open::D3DRES_CLEAR_FORMAT))?;
                for &s in &subs {
                    let Some(Some(b)) = res.subs.get_mut(s as usize) else { continue };
                    let el = (f.bits / 8) as usize;
                    for e in b.chunks_mut(el) {
                        super::d3draster::write_depth(f, e, flags & 1 != 0, depth, flags & 2 != 0, st)?;
                    }
                }
            }
            _ => {
                hit(clause::D3DRES_CLEAR_UAV);
                let float = t[2] == "f";
                if res.d.dim == 4 && view.u(2, true) & 1 != 0 || res.d.misc & 0x40 != 0 {
                    // a raw or structured buffer: 32-bit words, the first value
                    hit(clause::D3DRES_CLEAR_UAVRAW);
                    if float {
                        // (MEASURED: a float clear of a raw view changes nothing, clearuav-raw)
                        hit(clause::D3DRES_CLEAR_UAVRAWF);
                        return Ok(());
                    }
                    let (first, count) = (view.u(0, true) as usize * 4, view.u(1, true) as usize * 4);
                    let b = filled_mut(&mut res.subs, 0)?;
                    for i in (first..first + count).step_by(4) {
                        b[i..i + 4].copy_from_slice(&w[0].to_le_bytes());
                    }
                    return Ok(());
                }
                let f = vf.ok_or(Fail::Unmodeled(open::D3DRES_CLEAR_FORMAT))?;
                let el = if float {
                    if !matches!(f.num, Num::Float | Num::Unorm | Num::Snorm) && f.id != 26 {
                        hit(clause::D3DRES_CLEAR_UAVFINT);
                        return Ok(());
                    }
                    encode(f, [0, 1, 2, 3].map(|i| f32::from_bits(w[i])))?
                } else {
                    if f.comps.is_empty() {
                        return Err(Fail::Unmodeled(open::D3DRES_CLEAR_UAVBITS));
                    }
                    encode_bits(f, [w[0], w[1], w[2], w[3]])
                };
                fill_subs(res, &subs, &el)?;
            }
        }
        Ok(())
    }
}

/// the subresources a view covers
pub fn view_subs(dev: Dev, v: &View, r: &Resource) -> Vec<u32> {
    let d3d11 = dev.d3d11();
    let mips = r.d.mips;
    let u = |i| v.u(i, d3d11);
    let (mip, first, n) = match (v.kind, v.dim()) {
        (_, _) if r.d.dim == 4 => return vec![0],
        // rtv: 2 1D(mip) 3 1DA(mip first n) 4 2D(mip) 5 2DA 6 MS 7 MSA(first n) 8 3D(mip first n: w slices)
        (1, 2 | 4) | (2, 1 | 3) | (3, 2 | 4) => (u(0), 0, 1),
        (1, 3 | 5) | (2, 2 | 4) | (3, 3 | 5) => (u(0), u(1), u(2)),
        (1, 6) | (2, 5) => (0, 0, 1),
        (1, 7) | (2, 6) => (0, u(0), u(1)),
        (1, 8) | (3, 8) => (u(0), 0, 1),
        _ => (0, 0, 1),
    };
    (first..first.saturating_add(n).min(r.d.da.max(1))).map(|a| mip + a * mips).collect()
}

fn fill_subs(res: &mut Resource, subs: &[u32], el: &[u8]) -> Res<()> {
    for &s in subs {
        if let Some(v) = res.samples.get_mut(s as usize) {
            *v = None;
        }
        if let Some(Some(b)) = res.subs.get_mut(s as usize) {
            for e in b.chunks_mut(el.len()) {
                e.copy_from_slice(el);
            }
        }
    }
    Ok(())
}

/// true: the device is removed
fn update(res: &mut Resource, sub: u32, data: &[u8], rp: usize, dp: usize, bx: Option<&[u32]>) -> Res<bool> {
    hit(clause::D3DRES_UPDATE);
    // (MEASURED: a subresource past the last changes nothing, probe-update-sub)
    if sub >= res.nsubs() {
        hit(clause::D3DRES_UPDATE_IGNORED);
        return Ok(false);
    }
    // (MEASURED: not a default resource, or multisampled: nothing changes, probe-update-immutable,
    // -dynamic, -staging, -ms)
    if res.d.usage != 0 || res.d.count > 1 {
        hit(clause::D3DRES_UPDATE_BAD);
        return Ok(false);
    }
    let (rb, rows, sl) = res.layout(sub)?;
    let (bw, bh, el) = match res.fmt() {
        Some(f) => (f.bw as usize, f.bh as usize, (f.bits / 8) as usize),
        None => (1, 1, 1),
    };
    // the box in elements (bytes for a buffer)
    let (l, tp, fr, r, b, bk) = match bx {
        None => (0, 0, 0, rb / el, rows, sl),
        Some(x) => {
            hit(clause::D3DRES_UPDATE_BOX);
            let x: Vec<usize> = x.iter().map(|v| *v as usize).collect();
            if res.d.dim == 4 {
                (x[0], 0, 0, x[3], 1, 1)
            } else {
                (x[0] / bw, x[1] / bh, x[2], x[3].div_ceil(bw), x[4].div_ceil(bh), x[5])
            }
        }
    };
    if l >= r || tp >= b || fr >= bk {
        hit(clause::D3DRES_UPDATE_EMPTY);
        return Ok(false);
    }
    // (MEASURED: a box past the subresource removes the device (WARP), probe-update-past)
    if r * el > rb || b > rows || bk > sl {
        hit(clause::D3DRES_REMOVED);
        return Ok(true);
    }
    let w = (r - l) * el;
    // (the source is the executor's 4 MiB: a box whose last row lies past it reads outside)
    let last = (bk - fr - 1).checked_mul(dp).zip((b - tp - 1).checked_mul(rp)).and_then(|(a, c)| a.checked_add(c)?.checked_add(w));
    if last.is_none_or(|e| e > data.len()) {
        return Err(Fail::Unmodeled(open::D3DRES_INIT_PAST));
    }
    let v = filled_mut(&mut res.subs, sub as usize)?;
    for z in fr..bk {
        for y in tp..b {
            let at = (z - fr) * dp + (y - tp) * rp;
            v[(z * rows + y) * rb + l * el..][..w].copy_from_slice(&data[at..at + w]);
        }
    }
    Ok(false)
}

fn copy_resource(dev: Dev, dst: &mut Resource, src: &Resource) -> Res<()> {
    hit(clause::D3DRES_COPY);
    let (a, b) = (&dst.d, &src.d);
    let same_shape = a.dim == b.dim && a.mips == b.mips && a.da == b.da && a.count == b.count;
    let blocks = |r: &Resource| {
        let f = r.fmt();
        (f.map_or(r.d.w, |f| r.d.w.div_ceil(f.bw)), f.map_or(r.d.h, |f| r.d.h.div_ceil(f.bh)))
    };
    // (MEASURED, WARP: probe-rt-ms4-copy) Direct3D 10 copies no multisampled resource
    let ok = same_shape && dst.d.usage != 1 && !(dev == Dev::D10 && a.count > 1) && (a.dim == 4 && a.w == b.w || a.dim != 4 && copy_compatible(a.fmt, b.fmt, dev) && (if group(a.fmt) == group(b.fmt) { (a.w, a.h) == (b.w, b.h) } else { blocks(dst) == blocks(src) }));
    if !ok {
        hit(clause::D3DRES_COPY_IGNORED);
        return Ok(());
    }
    dst.subs = src.subs.clone();
    // (MEASURED, WARP: probe-rt-ms4-copy) a multisampled resource's samples copy with it
    if src.samples.iter().any(Option::is_some) || dst.samples.iter().any(Option::is_some) {
        hit(clause::D3DRES_COPY_SAMPLES);
    }
    dst.samples = src.samples.clone();
    Ok(())
}

/// true: the device is removed
fn copy_region(dev: Dev, dst: &mut Resource, ds: u32, at: (u32, u32, u32), src: &Resource, ss: u32, bx: Option<&[u32]>) -> Res<bool> {
    hit(clause::D3DRES_COPYSUB);
    // (MEASURED: a missing subresource on either side, an immutable destination, or sample
    // counts that differ: nothing changes, probe-copysub-dsub, -ssub, -immutable, -ms)
    if ds >= dst.nsubs() || ss >= src.nsubs() || dst.d.usage == 1 || dst.d.count != src.d.count {
        hit(clause::D3DRES_COPYSUB_IGNORED);
        return Ok(false);
    }
    // (MEASURED: textures of other dimensions copy the box all the same, probe-copysub-dim; a
    // buffer to a texture or back changes nothing, probe-copysub-buftex, -texbuf)
    if (dst.d.dim == 4) != (src.d.dim == 4) {
        hit(clause::D3DRES_COPYSUB_BAD);
        return Ok(false);
    }
    if dst.d.dim == 4 {
        let (l, r) = bx.map_or((0, src.d.w as usize), |x| (x[0] as usize, x[3] as usize));
        let x = at.0 as usize;
        // (as for textures: an empty box changes nothing, one past either buffer removes the
        // device, probe-copysub-buf)
        if r <= l {
            hit(clause::D3DRES_COPYSUB_BAD);
            return Ok(false);
        }
        if x.saturating_add(r - l) > dst.d.w as usize || r > src.d.w as usize {
            hit(clause::D3DRES_REMOVED);
            return Ok(true);
        }
        let s = filled(&src.subs, 0)?[l..r].to_vec();
        filled_mut(&mut dst.subs, 0)?[x..x + (r - l)].copy_from_slice(&s);
        return Ok(false);
    }
    if !copy_compatible(dst.d.fmt, src.d.fmt, dev) {
        hit(clause::D3DRES_COPY_IGNORED);
        return Ok(false);
    }
    let (sf, df) = (src.texture_format()?, dst.texture_format()?);
    let (srb, srows, ssl) = src.layout(ss)?;
    let (drb, drows, dsl) = dst.layout(ds)?;
    let (sel, del) = ((sf.bits / 8) as usize, (df.bits / 8) as usize);
    if sel != del {
        hit(clause::D3DRES_COPY_IGNORED);
        return Ok(false);
    }
    let (l, t, f, r, b, k) = match bx {
        None => (0, 0, 0, srb / sel, srows, ssl),
        Some(x) => {
            let x: Vec<usize> = x.iter().map(|v| *v as usize).collect();
            (x[0] / sf.bw as usize, x[1] / sf.bh as usize, x[2], x[3].div_ceil(sf.bw as usize), x[4].div_ceil(sf.bh as usize), x[5])
        }
    };
    let (dx, dy, dz) = (at.0 as usize / df.bw as usize, at.1 as usize / df.bh as usize, at.2 as usize);
    // (MEASURED: an empty box changes nothing, probe-copysub-empty)
    if r <= l || b <= t || k <= f {
        hit(clause::D3DRES_COPYSUB_BAD);
        return Ok(false);
    }
    // (MEASURED: a box past the source removes the device too, probe-copysub-srcbox)
    if r * sel > srb || b > srows || k > ssl {
        hit(clause::D3DRES_REMOVED);
        return Ok(true);
    }
    // (MEASURED: a region past the destination removes the device (WARP), probe-copysub-past)
    if dx + (r - l) > drb / del || dy + (b - t) > drows || dz + (k - f) > dsl {
        hit(clause::D3DRES_REMOVED);
        return Ok(true);
    }
    let s = filled(&src.subs, ss as usize)?.clone();
    let d = filled_mut(&mut dst.subs, ds as usize)?;
    for z in f..k {
        for y in t..b {
            let from = (z * srows + y) * srb + l * sel;
            let to = ((dz + z - f) * drows + dy + y - t) * drb + dx * del;
            d[to..to + (r - l) * sel].copy_from_slice(&s[from..from + (r - l) * sel]);
        }
    }
    Ok(false)
}

fn read(dev: Dev, res: &Resource, sub: u32, rb: usize, rows: usize, slices: usize, out: &mut Vec<String>) -> Res<()> {
    hit(clause::D3DRES_READ);
    if let Some(hr) = staging_refused(dev, res, sub)? {
        out.push(format!("read hr {hr:08x}"));
        return Ok(());
    }
    // (MEASURED: a subresource past the last reads as zeros, probe-read-sub)
    // (more than 16 MiB of zeros: not measured)
    if rb.saturating_mul(rows.saturating_mul(slices).min(1 << 16)) > 1 << 24 {
        return Err(Fail::Unmodeled(open::D3DRES_READ_PAST));
    }
    if sub >= res.nsubs() {
        hit(clause::D3DRES_READ_BAD);
        out.push(format!("read {rows} {slices}"));
        out.extend(std::iter::repeat_n("00".repeat(rb), rows.saturating_mul(slices).min(1 << 16)));
        return Ok(());
    }
    let (srb, srows, ssl) = res.layout(sub)?;
    if rb > srb || rows > srows || slices > ssl {
        return Err(Fail::Unmodeled(open::D3DRES_READ_PAST));
    }
    // (MEASURED: a multisampled resource reads as zeros: the copy to the single-sampled
    // staging texture does not run, probe-read-ms)
    if res.d.count > 1 {
        hit(clause::D3DRES_READ_MS);
        out.push(format!("read {rows} {slices}"));
        out.extend(std::iter::repeat_n("00".repeat(rb), rows * slices));
        return Ok(());
    }
    let Some(b) = &res.subs[sub as usize] else { return Err(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN)) };
    out.push(format!("read {rows} {slices}"));
    for z in 0..slices {
        for y in 0..rows {
            let at = (z * srows + y) * srb;
            out.push(b[at..at + rb].iter().map(|x| format!("{x:02x}")).collect());
        }
    }
    Ok(())
}

/// what a shader reads from texel (x, y) of a subresource, through a view's format
pub fn texel(res: &Resource, sub: u32, fmt: u32, x: u32, y: u32) -> Res<[u32; 4]> {
    let f = format(fmt).ok_or(Fail::Unmodeled(open::CONV_OTHER))?;
    let (rb, _, _) = res.layout(sub)?;
    if f.bw != 1 || f.bh != 1 {
        let el = (f.bits / 8) as usize;
        let b = res.subs[sub as usize].as_ref().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
        let at = (y / f.bh) as usize * rb + (x / f.bw) as usize * el;
        return super::dxgiconv::decode_block(f, &b[at..at + el], x % f.bw, y % f.bh);
    }
    let el = (f.bits / 8) as usize;
    let b = res.subs[sub as usize].as_ref().ok_or(Fail::Unmodeled(open::D3DRES_READ_UNKNOWN))?;
    let at = y as usize * rb + x as usize * el;
    decode(f, &b[at..at + el])
}

/// how samples that differ resolve to their mean, by the format (D3DRES.RESOLVE.MEAN)
#[derive(Clone, Copy, PartialEq, Debug)]
enum Mean {
    /// 8-bit UNORM: bytes as integers, (sum + n / 2) / n
    Bytes,
    /// 16-bit UNORM: v / 65535 in f32 summed, / n, times 65535 rounded half to even
    Words,
    /// 32-bit floats in f32
    Floats,
    /// 10-10-10-2 UNORM: each component's integer mean rounded half up
    Packed,
    /// other UNORM and SNORM (5-6-5, 8-bit SNORM): v times the f32 reciprocal of the largest
    /// value, summed in order, / n, times the largest value rounded half to even (SNORM's
    /// lowest value read as -largest)
    Norm,
    /// 16-bit float, sRGB, packed floats: the decoded values' f32 mean, encoded as a pixel's
    /// output to the format
    Decoded,
}

impl Mean {
    fn of(f: &Format) -> Option<Mean> {
        let all = |b: u32| f.comps.iter().all(|&c| c == b);
        match f.num {
            Num::Unorm if all(8) => Some(Mean::Bytes),
            Num::Unorm if all(16) => Some(Mean::Words),
            Num::Unorm if f.comps == [10, 10, 10, 2] => Some(Mean::Packed),
            Num::Unorm | Num::Snorm => Some(Mean::Norm),
            Num::Float if all(32) => Some(Mean::Floats),
            Num::Float if all(16) => Some(Mean::Decoded),
            Num::Srgb if all(8) => Some(Mean::Decoded),
            Num::Other if f.id == 26 => Some(Mean::Decoded),
            Num::Float | Num::Srgb | Num::Other | Num::Typeless | Num::Uint | Num::Sint => None,
        }
    }

    fn resolve<'a>(self, f: &Format, n: usize, sample: impl Fn(usize) -> &'a [u8]) -> Res<Vec<u8>> {
        let el = (f.bits / 8) as usize;
        let word = |k: usize| sample(k).iter().rev().fold(0u64, |a, &b| a << 8 | u64::from(b));
        Ok(match self {
            Mean::Packed | Mean::Norm => {
                let mut out = 0u64;
                let mut shift = 0;
                for &c in f.comps {
                    let mask = (1u64 << c) - 1;
                    let field = |k: usize| word(k) >> shift & mask;
                    let v = if self == Mean::Packed {
                        let sum: u64 = (0..n).map(field).sum();
                        ((sum + n as u64 / 2) / n as u64).min(mask)
                    } else {
                        let (top, signed) = if f.num == Num::Snorm { ((mask >> 1) as f32, true) } else { (mask as f32, false) };
                        let value = |k: usize| {
                            let u = field(k);
                            let i = if signed && u >> (c - 1) & 1 == 1 { u as i64 - (1i64 << c) } else { u as i64 };
                            (i as f32).max(-top)
                        };
                        let r = 1.0 / top;
                        let sum = (0..n).fold(0f32, |a, k| a + value(k) * r);
                        let m = ((sum / n as f32) * top).round_ties_even() as i64;
                        (m as u64) & mask
                    };
                    out |= v << shift;
                    shift += c;
                }
                out.to_le_bytes()[..el].to_vec()
            }
            Mean::Decoded => {
                let mut sum = [0f32; 4];
                for k in 0..n {
                    let c = super::dxgiconv::decode_f(f, sample(k))?;
                    for (s, v) in sum.iter_mut().zip(c) {
                        *s += v;
                    }
                }
                super::dxgiconv::encode(f, sum.map(|v| v / n as f32))?
            }
            Mean::Bytes => (0..el).map(|i| {
                let sum: usize = (0..n).map(|k| usize::from(sample(k)[i])).sum();
                u8::try_from((sum + n / 2) / n).unwrap_or(u8::MAX)
            }).collect(),
            Mean::Words => (0..el / 2).flat_map(|i| {
                let mean = (0..n).fold(0f32, |a, k| a + f32::from(u16::from_le_bytes([sample(k)[2 * i], sample(k)[2 * i + 1]])) / 65535.0) / n as f32;
                ((mean * 65535.0).round_ties_even() as u16).to_le_bytes()
            }).collect(),
            Mean::Floats => (0..el / 4).flat_map(|i| {
                let sum = (0..n).fold(0f32, |a, k| a + f32::from_le_bytes(sample(k)[4 * i..][..4].try_into().unwrap_or([0; 4])));
                (sum / n as f32).to_le_bytes()
            }).collect(),
        })
    }
}
