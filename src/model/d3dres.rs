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
use super::objects::{Id, Objects, Slot};
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
    /// the resource the view was made of (not its slot: the view keeps it when the slot is
    /// given another, ID3D11Device::CreateRenderTargetView's pResource)
    pub res: Id<Resource>,
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

thread_local! {
    /// the last seeds' initial data (a case makes and updates from a few seeds, each 4 MiB)
    static INIT_DATA: std::cell::RefCell<Vec<(u32, std::rc::Rc<[u8]>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// `xorbytes(seed, INIT)`, kept for the next lines of the same seeds
pub fn init_data(seed: u32) -> std::rc::Rc<[u8]> {
    INIT_DATA.with(|c| {
        let mut c = c.borrow_mut();
        if let Some((_, d)) = c.iter().find(|(s, _)| *s == seed) {
            return d.clone();
        }
        let d: std::rc::Rc<[u8]> = xorbytes(seed, INIT).into();
        if c.len() >= 4 {
            c.remove(0);
        }
        c.push((seed, d.clone()));
        d
    })
}

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
    pub res: Objects<Resource>,
    pub views: Objects<View>,
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

/// the slot token `i` names (a resource's or a view's, as the line's place says)
pub fn slot<T>(t: &[String], i: usize) -> Res<Slot<T>> {
    let v = t.get(i).and_then(|s| num(s)).ok_or(Fail::Bad)?;
    Slot::new(u64::try_from(v).ok().ok_or(Fail::Bad)?)
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
                    if self.res.at(slot(t, 1)?).is_none() {
                        out.push("read nores".into());
                        return Ok(true);
                    }
                    hit(clause::D3DRES_REMOVED);
                    out.push(if dev.d3d11() { "read hr 887a0005" } else { "read map hr 887a0005" }.into());
                    return Ok(true);
                }
                // (the executor answers a view of an empty slot itself, runner/d3dres.c op_view)
                "srv" | "rtv" | "dsv" | "uav" if self.res.at(slot(t, 2)?).is_none() => {
                    out.push(format!("{name} nores"));
                    return Ok(true);
                }
                // (MEASURED: a NULL out pointer still only validates, S_FALSE for a description
                // taken, probe-removed-calls)
                "tex" | "buf" if dev.d3d11() && t.get(1).is_some_and(|s| s != "check") => {
                    hit(clause::D3DRES_REMOVED_OPS);
                    self.res.empty(slot(t, 1)?);
                    out.push(format!("{name} hr 887a0005"));
                    return Ok(true);
                }
                "srv" | "rtv" | "dsv" | "uav" if dev.d3d11() && t.get(3).is_none_or(|s| s != "check") => {
                    hit(clause::D3DRES_REMOVED_OPS);
                    self.views.empty(slot(t, 1)?);
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
        // (a slot without a resource is the executor's nores first, runner/d3dres.c op_copy)
        let held = |i: usize| slot(t, i).ok().and_then(|k| self.res.at(k));
        if at.iter().all(|&i| held(i).is_some()) && at.iter().any(|&i| held(i).is_some_and(|r| r.d.count > 1)) {
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
                let Some(res) = self.res.at_mut(r) else {
                    out.push("update nores".into());
                    return Ok(true);
                };
                self.removed |= update(res, sub, &init_data(seed), rp, dp, bx.as_deref())?;
            }
            ResOp::Copy => {
                if t.len() != 3 {
                    return Err(Fail::Bad);
                }
                let (d, s) = (slot(t, 1)?, slot(t, 2)?);
                let (Some(_), Some(src)) = (self.res.at(d), self.res.at(s).cloned()) else {
                    out.push("copy nores".into());
                    return Ok(true);
                };
                let dst = self.res.at_mut(d).ok_or(Fail::Empty)?;
                copy_resource(dev, dst, &src)?;
            }
            ResOp::Copysub => {
                let (d, ds, x, y, z, s, ss) = (slot(t, 1)?, n(t, 2)?, n(t, 3)?, n(t, 4)?, n(t, 5)?, slot(t, 6)?, n(t, 7)?);
                let bx = if t.len() == 14 { Some((8..14).map(|i| n(t, i)).collect::<Res<Vec<u32>>>()?) } else if t.len() == 8 { None } else { return Err(Fail::Bad) };
                let (Some(_), Some(src)) = (self.res.at(d), self.res.at(s).cloned()) else {
                    out.push("copysub nores".into());
                    return Ok(true);
                };
                let dst = self.res.at_mut(d).ok_or(Fail::Empty)?;
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
                let (Some(_), Some(src)) = (self.res.at(d), self.res.at(s).cloned()) else {
                    out.push("resolve nores".into());
                    return Ok(true);
                };
                hit(clause::D3DRES_RESOLVE);
                let dst = self.res.at_mut(d).ok_or(Fail::Empty)?;
                // (every sample holds the same value in these cases: the resolve is a copy)
                let (srb, srows, _) = if ss < src.nsubs() { src.layout(ss)? } else { (0, 0, 0) };
                let (drb, drows, _) = if ds < dst.nsubs() { dst.layout(ds)? } else { (0, 0, 0) };
                let n = |v: usize| u64::try_from(v).unwrap_or(u64::MAX);
                let int = format(f).is_some_and(|x| matches!(x.num, Num::Uint | Num::Sint));
                let groups = group(f) == group(src.d.fmt) && group(f) == group(dst.d.fmt);
                let subs = ds < dst.nsubs() && ss < src.nsubs() && dst.d.dim == src.d.dim;
                match resolve_plan(src.d.count, (dst.d.count, dst.d.usage), groups, int, subs, [n(srb), n(srows)], [n(drb), n(drows)]) {
                    Plan::Nothing => return Ok(true),
                    Plan::Removed => {
                        self.removed = true;
                        return Ok(true);
                    }
                    Plan::Copy => {}
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
                    // (every format a multisampled target takes, but the integer ones, has its Mean)
                    let mean = Mean::of(f).ok_or(Fail::Bad)?;
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
                let Some(res) = self.res.at(r) else {
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
        // (a check makes nothing: no slot)
        let s: Option<Slot<Resource>> = if check { None } else { Some(slot(t, 1)?) };
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
        if let Some(s) = s {
            self.res.empty(s);
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
        let Some(s) = s else {
            hit(clause::D3DRES_CHECK);
            out.push(format!("{kind} hr {S_FALSE:08x}"));
            return Ok(());
        };
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
            let data = init_data(seed);
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
        self.res.put(s, r);
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
        let Some((rid, res)) = self.res.id(r).zip(self.res.at(r)) else {
            out.push(format!("{} nores", t[0]));
            return Ok(());
        };
        // (the executor empties the view slot before it finds Direct3D 10 has no UAV,
        // runner/d3dres.c op_view)
        if !check {
            self.views.empty(v);
        }
        if kind == 3 && !dev.d3d11() {
            out.push("uav none".into());
            return Ok(());
        }
        match check_view(dev, kind, res, words.as_deref(), check)? {
            Err(hr) => out.push(format!("{} hr {hr:08x}", t[0])),
            Ok(_) if check => {
                hit(clause::D3DRES_CHECK);
                out.push(format!("{} hr {S_FALSE:08x}", t[0]));
            }
            Ok(w) => {
                out.push(format!("{} = {}", t[0], w.iter().map(|x| format!("{x:x}")).collect::<Vec<_>>().join(" ")));
                self.views.put(v, View { kind, res: rid, words: w });
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
        let Some(view) = self.views.at(v).cloned().filter(|x| x.kind == want) else {
            out.push(format!("{name} noview"));
            return Ok(());
        };
        hit(clause::D3DRES_VIEW_HOLDS);
        let res = self.res.get_mut(view.res)?;
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
/// the subresources of its resource a view reaches: levels `mip ..` (`mips` of them), array
/// slices `first ..` (`slices` of them; a 3D texture's one), and the planes a depth view writes
/// (the review of 84b05c8, 3.1 to 3.3: Load, Sample and the binding hazards each read this one
/// answer, so none of them takes a view's level 0 for its resource's)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reach {
    pub mip: u32,
    pub mips: u32,
    pub first: u32,
    pub slices: u32,
    /// the planes it reads or writes: depth, stencil (a read-only depth view writes neither of
    /// the planes it is read-only for; other views reach the one plane of their resource, depth)
    pub planes: [bool; 2],
}

impl Reach {
    /// the subresource indices, each slice's levels in turn (D3D11CalcSubresource)
    pub fn subs(&self, r: &Resource) -> Vec<u32> {
        if r.d.dim == 4 {
            return vec![0];
        }
        let total = r.d.mips.max(1);
        let slices = if r.d.dim == 3 { 1 } else { r.d.da.max(1) };
        let last_slice = self.first.saturating_add(self.slices).min(slices);
        let last_mip = self.mip.saturating_add(self.mips).min(total);
        (self.first..last_slice).flat_map(|a| (self.mip..last_mip).map(move |m| m.saturating_add(a.saturating_mul(total)))).collect()
    }
    /// whether a write through `w` and a read through `self` meet: a subresource both reach, of a
    /// plane both reach (ID3D11DeviceContext::OMSetRenderTargets, PSSetShaderResources: "a
    /// resource cannot be bound for reading and writing at once")
    pub fn meets(&self, w: &Reach) -> bool {
        let plane = self.planes.iter().zip(w.planes).any(|(&a, b)| a && b);
        let range = |a0: u32, an: u32, b0: u32, bn: u32| a0 < b0.saturating_add(bn) && b0 < a0.saturating_add(an) && an > 0 && bn > 0;
        plane && range(self.mip, self.mips, w.mip, w.mips) && range(self.first, self.slices, w.first, w.slices)
    }
}

/// a level count the view gives as -1 (or past the resource's levels): every level from its
/// most detailed
fn levels_from(mip: u32, n: u32, total: u32) -> u32 {
    n.min(total.saturating_sub(mip))
}

/// what a view reaches (Reach): its kind's and dimension's union words, as GetDesc gives them
pub fn view_reach(dev: Dev, v: &View, r: &Resource) -> Reach {
    let d3d11 = dev.d3d11();
    let total = r.d.mips.max(1);
    let u = |i| v.u(i, d3d11);
    let one = |mip: u32, first: u32, slices: u32| (mip, 1, first, slices);
    let (mip, mips, first, slices) = match (v.kind, v.dim()) {
        (_, _) if r.d.dim == 4 => (0, 1, 0, 1),
        // srv: 2 1D(mip n) 3 1DA(mip n first size) 4 2D 5 2DA 6 MS 7 MSA(first size) 8 3D(mip n)
        // 9 cube(mip n: 6 faces) 10 cube array(mip n first2dface cubes)
        (0, 2 | 4 | 8) => (u(0), levels_from(u(0), u(1), total), 0, 1),
        (0, 3 | 5) => (u(0), levels_from(u(0), u(1), total), u(2), u(3)),
        (0, 6) => (0, 1, 0, 1),
        (0, 7) => (0, 1, u(0), u(1)),
        (0, 9) => (u(0), levels_from(u(0), u(1), total), 0, 6),
        (0, 10) => (u(0), levels_from(u(0), u(1), total), u(2), u(3).saturating_mul(6)),
        // rtv: 2 1D(mip) 3 1DA(mip first n) 4 2D(mip) 5 2DA 6 MS 7 MSA(first n) 8 3D(mip first n: w slices)
        (1, 2 | 4) | (2, 1 | 3) | (3, 2 | 4) => one(u(0), 0, 1),
        (1, 3 | 5) | (2, 2 | 4) | (3, 3 | 5) => one(u(0), u(1), u(2)),
        (1, 6) | (2, 5) => one(0, 0, 1),
        (1, 7) | (2, 6) => one(0, u(0), u(1)),
        (1, 8) | (3, 8) => one(u(0), 0, 1),
        // (buffers and dimensions no view of a texture has: its first subresource)
        _ => one(0, 0, 1),
    };
    // (a depth view: the planes it does not hold read-only, D3D11_DSV_FLAG_READ_ONLY_DEPTH 1,
    // _STENCIL 2; Direct3D 10's have no flags. A view of a stencil format's stencil, X24_TYPELESS_G8_UINT
    // 47 or X32_TYPELESS_G8X24_UINT 22, reads that plane; any other the depth one)
    let planes = match v.kind {
        2 => {
            let flags = if d3d11 { v.words.get(2).copied().unwrap_or(0) } else { 0 };
            [flags & 1 == 0, flags & 2 == 0]
        }
        _ if matches!(v.format(), 22 | 47) => [false, true],
        _ => [true, false],
    };
    Reach { mip, mips, first, slices, planes }
}

/// the subresources a view reaches (view_reach)
pub fn view_subs(dev: Dev, v: &View, r: &Resource) -> Vec<u32> {
    view_reach(dev, v, r).subs(r)
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

/// what an update or a region copy does: nothing, remove the device, or copy its box
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Plan {
    Nothing,
    Removed,
    Copy,
}

/// a box (D3D11_BOX: left, top, front inclusive, right, bottom, back exclusive; in pixels,
/// block aligned; a buffer's in bytes) in elements of `block` pixels, or none: the whole of a
/// subresource of `whole` elements a row, rows and slices; `flat`: one row from its top and one
/// slice from its front, whatever its bottom and back (a 1D texture's in UpdateSubresource)
/// (D3DRES.UPDATE.BOX)
pub fn box_elems(bx: Option<&[u32]>, flat: bool, block: (u32, u32), whole: [u64; 3]) -> [u64; 6] {
    let Some(x) = bx else { return [0, 0, 0, whole[0], whole[1], whole[2]] };
    let g = |i: usize| x.get(i).copied().map_or(0, u64::from);
    let (bw, bh) = (u64::from(block.0.max(1)), u64::from(block.1.max(1)));
    let (l, r) = (g(0).checked_div(bw).unwrap_or(0), g(3).div_ceil(bw));
    if flat {
        return [l, g(1), g(2), r, g(1).saturating_add(1), g(2).saturating_add(1)];
    }
    [l, g(1).checked_div(bh).unwrap_or(0), g(2), r, g(4).div_ceil(bh), g(5)]
}

/// a box cut to a subresource of `whole`
pub fn box_clip(b: [u64; 6], whole: [u64; 3]) -> [u64; 6] {
    [b[0], b[1], b[2], b[3].min(whole[0]), b[4].min(whole[1]), b[5].min(whole[2])]
}

/// a box of no element (D3DRES.UPDATE.EMPTY, COPYSUB.BAD)
pub fn box_empty(b: [u64; 6]) -> bool {
    b[0] >= b[3] || b[1] >= b[4] || b[2] >= b[5]
}

/// a box's extent put at `at` reaches past `whole` (D3DRES.REMOVED)
pub fn box_past(b: [u64; 6], at: [u64; 3], whole: [u64; 3]) -> bool {
    let reach = |i: usize, j: usize| at[i].saturating_add(b[j].saturating_sub(b[i])) > whole[i];
    reach(0, 3) || reach(1, 4) || reach(2, 5)
}

/// UpdateSubresource of subresource `sub` of `nsubs` of a resource of `usage` and `count`
/// samples, the box `b` in elements, against the resource's top level of `top` elements a row,
/// rows and slices (D3DRES.UPDATE.IGNORED, UPDATE.BAD, UPDATE.EMPTY, REMOVED)
pub fn update_plan(sub: u32, nsubs: u32, usage: u32, count: u32, b: [u64; 6], top: [u64; 3]) -> Plan {
    // (MEASURED: a subresource past the last changes nothing, probe-update-sub)
    if sub >= nsubs {
        hit(clause::D3DRES_UPDATE_IGNORED);
        return Plan::Nothing;
    }
    // (MEASURED: not a default resource, or multisampled: nothing changes, probe-update-immutable,
    // -dynamic, -staging, -ms)
    if usage != 0 || count > 1 {
        hit(clause::D3DRES_UPDATE_BAD);
        return Plan::Nothing;
    }
    if box_empty(b) {
        hit(clause::D3DRES_UPDATE_EMPTY);
        return Plan::Nothing;
    }
    // (MEASURED: a box past the top level removes the device (WARP), probe-update-past; one past
    // only a smaller level's size is cut to it, d3d10res seq-136, seq-151)
    if box_past(b, [b[0], b[1], b[2]], top) {
        hit(clause::D3DRES_REMOVED);
        return Plan::Removed;
    }
    Plan::Copy
}

/// ResolveSubresource of a source of `src_count` samples to a destination of (samples, usage) `dst`:
/// `groups` the format's typeless group is both resources', `int` the format is an integer one,
/// `subs` both subresources are there and the dimensions agree; each subresource's (row bytes,
/// rows) (D3DRES.RESOLVE, RESOLVE.IGNORED, RESOLVE.BAD, RESOLVE.SIZE)
pub fn resolve_plan(src_count: u32, dst: (u32, u32), groups: bool, int: bool, subs: bool, src_size: [u64; 2], dst_size: [u64; 2]) -> Plan {
    hit(clause::D3DRES_RESOLVE);
    // (MEASURED: a multisampled destination, or a format of another group, changes nothing,
    // probe-resolve-ms-dst, probe-resolve-format; an integer format, probe-rt-ms4-uint, d3d11res
    // seq-10, seq-67; a destination not of default usage, d3d11res seq-16, seq-171, seq-224)
    let (dst_count, dst_usage) = dst;
    if dst_count != 1 || dst_usage != 0 || !groups || int {
        hit(clause::D3DRES_RESOLVE_IGNORED);
        return Plan::Nothing;
    }
    // (MEASURED: a missing subresource, or textures of other dimensions: nothing changes,
    // probe-resolve-dsub, probe-resolve-dim)
    if !subs {
        hit(clause::D3DRES_RESOLVE_BAD);
        return Plan::Nothing;
    }
    // (MEASURED: a single-sampled source is copied, as the whole subresource to the corner: one
    // larger than the destination removes the device, d3d10res seq-68, seq-150, d3d11res seq-130,
    // seq-164)
    if src_count < 2 && (src_size[0] > dst_size[0] || src_size[1] > dst_size[1]) {
        hit(clause::D3DRES_RESOLVE_IGNORED);
        hit(clause::D3DRES_REMOVED);
        return Plan::Removed;
    }
    // (MEASURED: sizes that differ resolve where both are, probe-resolve-size)
    if src_size != dst_size {
        hit(clause::D3DRES_RESOLVE_SIZE);
    }
    Plan::Copy
}

/// the two sides of CopySubresourceRegion: buffers, textures (of compatible formats with
/// elements of one size, or not), or one of each
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sides {
    Buffers,
    Textures { compatible: bool },
    Mixed,
}

/// CopySubresourceRegion of the box `b` (in the source's elements) of a source subresource of
/// `src` to `at` of a destination one of `dst`; `ignored`: a subresource missing on either side,
/// an immutable destination or sample counts that differ (D3DRES.COPYSUB.IGNORED, COPYSUB.BAD,
/// COPY.IGNORED, REMOVED)
pub fn region_plan(ignored: bool, sides: Sides, b: [u64; 6], src: [u64; 3], dst: [u64; 3], at: [u64; 3]) -> Plan {
    // (MEASURED: a missing subresource on either side, an immutable destination, or sample
    // counts that differ: nothing changes, probe-copysub-dsub, -ssub, -immutable, -ms)
    if ignored {
        hit(clause::D3DRES_COPYSUB_IGNORED);
        return Plan::Nothing;
    }
    match sides {
        // (MEASURED: a buffer to a texture or back changes nothing, probe-copysub-buftex, -texbuf)
        Sides::Mixed => {
            hit(clause::D3DRES_COPYSUB_BAD);
            return Plan::Nothing;
        }
        Sides::Textures { compatible: false } => {
            hit(clause::D3DRES_COPY_IGNORED);
            return Plan::Nothing;
        }
        Sides::Buffers | Sides::Textures { compatible: true } => {}
    }
    // (MEASURED: a box past the source removes the device, probe-copysub-srcbox, and a region
    // past the destination (WARP), probe-copysub-past, -buf, an empty box's too, seq-11, seq-109)
    if box_past(b, [b[0], b[1], b[2]], src) || box_past(b, at, dst) {
        hit(clause::D3DRES_REMOVED);
        return Plan::Removed;
    }
    // (MEASURED: an empty box changes nothing, probe-copysub-empty, -buf)
    if box_empty(b) {
        hit(clause::D3DRES_COPYSUB_BAD);
        return Plan::Nothing;
    }
    Plan::Copy
}

/// a subresource's (row bytes, rows, slices) as (elements of `el` bytes a row, rows, slices)
fn whole_of(layout: (usize, usize, usize), el: usize) -> [u64; 3] {
    let n = |v: usize| u64::try_from(v).unwrap_or(u64::MAX);
    [n(layout.0.checked_div(el).unwrap_or(0)), n(layout.1), n(layout.2)]
}

/// true: the device is removed
fn update(res: &mut Resource, sub: u32, data: &[u8], rp: usize, dp: usize, bx: Option<&[u32]>) -> Res<bool> {
    hit(clause::D3DRES_UPDATE);
    let (rb, rows, sl) = res.layout(sub)?;
    let (block, el) = match res.fmt() {
        Some(f) => ((f.bw, f.bh), usize::try_from(f.bits / 8).unwrap_or(1)),
        None => ((1, 1), 1),
    };
    if bx.is_some() {
        hit(clause::D3DRES_UPDATE_BOX);
    }
    let whole = whole_of((rb, rows, sl), el);
    // (MEASURED: a 1D texture's box is one row from its top whatever its bottom, d3d11res
    // seq-100, seq-187, seq-254; a buffer's has its rows, seq-117)
    let e = box_elems(bx, res.d.dim == 1, block, whole);
    match update_plan(sub, res.nsubs(), res.d.usage, res.d.count, e, whole_of(res.layout(0)?, el)) {
        Plan::Nothing => return Ok(false),
        Plan::Removed => return Ok(true),
        Plan::Copy => {}
    }
    let c = box_clip(e, whole);
    if box_empty(c) {
        return Ok(false);
    }
    let [l, tp, fr, r, b, bk] = c.map(|v| usize::try_from(v).unwrap_or(usize::MAX));
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
    let ignored = ds >= dst.nsubs() || ss >= src.nsubs() || dst.d.usage == 1 || dst.d.count != src.d.count;
    let n = |v: u32| u64::from(v);
    let removed = |plan: Plan| match plan {
        Plan::Nothing => Some(false),
        Plan::Removed => Some(true),
        Plan::Copy => None,
    };
    match (dst.d.dim == 4, src.d.dim == 4) {
        // (as for textures: an empty box changes nothing, one past either buffer removes the
        // device, probe-copysub-buf; the destination's y and z are not read)
        (true, true) => {
            let e = box_elems(bx, false, (1, 1), [n(src.d.w), 1, 1]);
            if let Some(r) = removed(region_plan(ignored, Sides::Buffers, e, [n(src.d.w), 1, 1], [n(dst.d.w), 1, 1], [n(at.0), 0, 0])) {
                return Ok(r);
            }
            let [l, _, _, r, _, _] = e.map(|v| usize::try_from(v).unwrap_or(usize::MAX));
            let x = usize::try_from(at.0).unwrap_or(usize::MAX);
            let s = filled(&src.subs, 0)?[l..r].to_vec();
            filled_mut(&mut dst.subs, 0)?[x..x + (r - l)].copy_from_slice(&s);
            return Ok(false);
        }
        // (MEASURED: textures of other dimensions copy the box all the same, probe-copysub-dim)
        (false, false) => {}
        _ => return Ok(removed(region_plan(ignored, Sides::Mixed, [0; 6], [0; 3], [0; 3], [0; 3])).unwrap_or(false)),
    }
    let (sf, df) = (src.texture_format()?, dst.texture_format()?);
    let (srb, srows, ssl) = src.layout(ss)?;
    let (drb, drows, dsl) = dst.layout(ds)?;
    let (sel, del) = (usize::try_from(sf.bits / 8).unwrap_or(1), usize::try_from(df.bits / 8).unwrap_or(1));
    let compatible = !ignored && copy_compatible(dst.d.fmt, src.d.fmt, dev) && sel == del;
    let whole = whole_of((srb, srows, ssl), sel);
    let e = box_elems(bx, false, (sf.bw, sf.bh), whole);
    let to = [n(at.0).checked_div(n(df.bw)).unwrap_or(0), n(at.1).checked_div(n(df.bh)).unwrap_or(0), n(at.2)];
    if let Some(r) = removed(region_plan(ignored, Sides::Textures { compatible }, e, whole, whole_of((drb, drows, dsl), del), to)) {
        return Ok(r);
    }
    let [l, t, f, r, b, k] = e.map(|v| usize::try_from(v).unwrap_or(usize::MAX));
    let [dx, dy, dz] = to.map(|v| usize::try_from(v).unwrap_or(usize::MAX));
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
