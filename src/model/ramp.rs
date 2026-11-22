//! The ramp device of Direct3D 1 and 2 (d3dim.dll, 10.0.26100.4202): the rasterizer of
//! d3dim700 fed through a 4 KB buffer of primitive headers and spans that lives as long as the
//! device and is never cleared, as the clean-room notes on d3dim.dll describe it
//! (spec/d3d7.txt D3D7.RAMP, D3D7.TRI.CLIP).
//!
//! The colour stage adds each span's specular words, which nothing on the ramp path writes: they
//! hold what earlier primitives left at those bytes (a z value, an earlier span's specular), and
//! that is the colour a pixel gets. So the buffer is modelled byte for byte: every field the
//! setup, the walkers and the fills write, at its offset, and the per-pixel adds read back.
use super::d3dim::{self, Raster, Walker};
use super::{Fail, Res};

const SIZE: usize = 0x1000;
const HEADER: usize = 0x60;
const SPAN: usize = 0x40;

/// the device's buffer: zeros when the device is made, then whatever was last written
pub struct Spans {
    b: Box<[u8; SIZE]>,
}

impl Default for Spans {
    fn default() -> Spans {
        Spans { b: Box::new([0; SIZE]) }
    }
}

/// a pixel the render writes: its column and row as the span walks them, and its B, G, R in 16
/// bits (the 8-bit value the high byte)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Px {
    pub x: i64,
    pub y: i64,
    pub c: [u16; 3],
}

/// the span fill the states choose (BeginPrimSet: flag 0x80 for ZENABLE or ZWRITEENABLE, 0x20
/// for fog)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fill {
    /// Z_DIdx: the z, and the index in both colour dwords
    ZIdx { z: u32 },
    /// Any_Either: the index and the alpha (J); with fog its word too
    Either { z_stage: bool },
}

/// how a primitive set is shaded
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shading {
    pub fill: Fill,
    /// the first vertex's index and alpha (16.16 each: I = (index + 1/2) · 256 · 256 / 256)
    pub i: u32,
    pub j: u32,
    /// D3DSHADE_GOURAUD (TriSetup_DIdx writes the steps, ColorGen adds them)
    pub gouraud: bool,
    /// SPECULARENABLE: the colour stage adds the span's specular words
    pub specular: bool,
    /// FOGENABLE: the fog colour's B, G, R
    pub fog: Option<[u16; 3]>,
}

/// the z a fill writes: z · 2147483520 rounded toward zero, then truncated
pub fn z_word(z: f32) -> Option<u32> {
    let v = d3dim::trunc(d3dim::x87_mul(z, f32::from_bits(0x4eff_ffff)))?;
    u32::try_from(v).ok()
}

impl Spans {
    fn rd16(&self, a: usize) -> Res<u16> {
        let b = self.b.get(a..a.wrapping_add(2)).ok_or(Fail::Bad)?;
        Ok(u16::from_le_bytes([*b.first().ok_or(Fail::Bad)?, *b.get(1).ok_or(Fail::Bad)?]))
    }

    fn wr16(&mut self, a: usize, v: u16) -> Res<()> {
        let b = self.b.get_mut(a..a.wrapping_add(2)).ok_or(Fail::Bad)?;
        b.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    fn wr32(&mut self, a: usize, v: u32) -> Res<()> {
        let b = self.b.get_mut(a..a.wrapping_add(4)).ok_or(Fail::Bad)?;
        b.copy_from_slice(&v.to_le_bytes());
        Ok(())
    }

    fn rd32(&self, a: usize) -> Res<u32> {
        let lo = u32::from(self.rd16(a)?);
        let hi = u32::from(self.rd16(a.wrapping_add(2))?);
        Ok(lo | hi << 16)
    }
}

/// one DrawPrimitives2 batch (Begin to End): the cursor, the prims committed since the last
/// flush in order, and the pixels rendered so far
pub struct Batch<'a> {
    sp: &'a mut Spans,
    sh: Shading,
    cursor: usize,
    prims: Vec<usize>,
    /// the prim lines and points append to (a zeroed header of their own)
    open: Option<usize>,
    pub out: Vec<Px>,
    /// the J of the span being filled
    j_now: u32,
}

/// a value's low 32 bits
fn dword(v: i64) -> u32 {
    u32::try_from(v.rem_euclid(1 << 32)).unwrap_or(0)
}

/// J at a span's start: 256 times the alpha accumulator (8.8), truncated
fn j_of(acc: [f32; 4]) -> Res<u32> {
    Ok(dword(d3dim::trunc(acc[3] * 256.0).ok_or(Fail::Bad)?))
}

impl<'a> Batch<'a> {
    pub fn begin(sp: &'a mut Spans, sh: Shading) -> Batch<'a> {
        Batch { sp, sh, cursor: 0, prims: Vec::new(), open: None, out: Vec::new(), j_now: sh.j }
    }

    /// End (and Flush): every prim along its links, its spans in order
    pub fn end(&mut self) -> Res<()> {
        self.flush()
    }

    fn flush(&mut self) -> Res<()> {
        if self.cursor > HEADER {
            for h in std::mem::take(&mut self.prims) {
                self.render(h)?;
            }
        }
        self.prims.clear();
        self.cursor = 0;
        self.open = None;
        Ok(())
    }

    /// a prim header committed at the cursor
    fn commit(&mut self, h: usize) -> Res<()> {
        self.cursor = h.wrapping_add(HEADER);
        self.sp.wr16(h.wrapping_add(4), 0)?;
        self.prims.push(h);
        Ok(())
    }

    /// AllocSpans(n): the slots granted at the cursor (a partial flush when not even 8 fit)
    fn alloc(&mut self, n: usize, cur: &mut usize) -> Res<usize> {
        loop {
            let room = SIZE.saturating_sub(self.cursor);
            if n.saturating_mul(SPAN) <= room {
                return Ok(n);
            }
            if room >= 0x200 {
                return Ok(room / SPAN);
            }
            // FlushPartial: the current header set aside, everything rendered, the header back at
            // 0 with no spans and no link
            let mut keep = [0u8; HEADER];
            keep.copy_from_slice(self.sp.b.get(*cur..cur.wrapping_add(HEADER)).ok_or(Fail::Bad)?);
            self.flush()?;
            self.sp.b.get_mut(..HEADER).ok_or(Fail::Bad)?.copy_from_slice(&keep);
            self.sp.wr16(4, 0)?;
            self.sp.wr32(0x54, 0)?;
            *cur = 0;
            self.prims.push(0);
            self.cursor = HEADER;
        }
    }

    /// one span at the cursor: the walker's count, x and y, then the fill
    fn span(&mut self, h: usize, x: i64, y: i64, n: i64, kind: Kind) -> Res<()> {
        let s = self.cursor;
        let w = |v: i64| u16::from_le_bytes(i16::try_from(v).unwrap_or(0).to_le_bytes());
        self.sp.wr16(s, w(n))?;
        self.sp.wr16(s.wrapping_add(4), w(x))?;
        self.sp.wr16(s.wrapping_add(6), w(y))?;
        match (kind, self.sh.fill) {
            (Kind::Point, fill) => {
                if let Fill::ZIdx { z } = fill {
                    self.sp.wr32(s.wrapping_add(0x0c), z)?;
                }
                self.sp.wr32(s.wrapping_add(0x20), 0)?;
                self.sp.wr32(s.wrapping_add(0x24), self.sh.j)?;
            }
            (Kind::Tri, Fill::ZIdx { z }) => {
                self.sp.wr32(s.wrapping_add(0x0c), z)?;
                self.sp.wr32(s.wrapping_add(0x20), self.sh.i)?;
                self.sp.wr32(s.wrapping_add(0x24), self.sh.i)?;
            }
            (Kind::Tri, Fill::Either { .. }) => {
                self.sp.wr32(s.wrapping_add(0x20), self.sh.i)?;
                self.sp.wr32(s.wrapping_add(0x24), self.j_now)?;
                if self.sh.fog.is_some() {
                    // (the fog word and its step: the ramp vertex's fog is 0)
                    self.sp.wr16(s.wrapping_add(0x2e), 0)?;
                    self.sp.wr16(s.wrapping_add(0x02), 0)?;
                }
            }
        }
        let c = self.sp.rd16(h.wrapping_add(4))?;
        self.sp.wr16(h.wrapping_add(4), c.wrapping_add(1))?;
        self.cursor = s.wrapping_add(SPAN);
        Ok(())
    }

    /// a triangle the rasterizer set up (None: rejected, nothing written)
    pub fn tri(&mut self, r: Option<&Raster>) -> Res<()> {
        if self.cursor.wrapping_add(0xa0) > SIZE {
            self.flush()?;
        }
        self.open = None;
        let Some(r) = r else { return Ok(()) };
        let mut h = self.cursor;
        self.sp.wr32(h, u32::from(r.reversed))?;
        if matches!(self.sh.fill, Fill::ZIdx { .. } | Fill::Either { z_stage: true }) {
            self.sp.wr32(h.wrapping_add(0x08), 0)?;
        }
        if self.sh.gouraud {
            // the per-pixel steps of I (0: one index) and J; a step from 2^30 (flag 0x10000: spans
            // of one pixel, d3dim::raster_as) writes neither, the words stay as the buffer held
            // them (clean-room notes E)
            let dj = d3dim::trunc(r.step[3] * 256.0).ok_or(Fail::Bad)?;
            if dj.abs() < 1 << 30 {
                self.sp.wr32(h.wrapping_add(0x18), 0)?;
                self.sp.wr32(h.wrapping_add(0x1c), dword(dj))?;
            }
        }
        if let Some(&p) = self.prims.last() {
            self.sp.wr32(p.wrapping_add(0x54), 1)?;
        }
        self.sp.wr32(h.wrapping_add(0x54), 0)?;
        self.commit(h)?;
        for t in &r.traps {
            let mut slots = 0usize;
            let rows = usize::try_from(t.rows).ok().ok_or(Fail::Bad)?;
            for (k, row) in t.row.iter().enumerate() {
                let left = rows.saturating_sub(k);
                for &(x, n, acc) in row {
                    if slots == 0 {
                        let want = match r.walker {
                            Walker::NoClip => left,
                            Walker::Clip => left.min(8),
                        };
                        slots = self.alloc(want, &mut h)?;
                        if slots == 0 {
                            return Err(Fail::Bad);
                        }
                    }
                    let y = t.first.checked_add(i64::try_from(k).ok().ok_or(Fail::Bad)?).ok_or(Fail::Bad)?;
                    self.j_now = j_of(acc)?;
                    self.span(h, x, y, n, Kind::Tri)?;
                    slots = slots.saturating_sub(1);
                }
            }
            // (unused slots given back: the cursor already stands after the last span)
        }
        Ok(())
    }

    /// a line's or a point's pixels (1-pixel spans) in the prim lines and points share: a
    /// zeroed header when none is open; the clip rectangle skips pixels without a slot
    pub fn pixels(&mut self, px: &[(i64, i64)], w: i64, h: i64, kind: Kind) -> Res<()> {
        let drawn: Vec<(i64, i64)> = px.iter().copied().filter(|&(x, y)| (0..w).contains(&x) && (0..h).contains(&y)).collect();
        if drawn.is_empty() {
            return Ok(());
        }
        let mut hd = match self.open {
            Some(o) => o,
            None => {
                if self.cursor.wrapping_add(0xa0) > SIZE {
                    self.flush()?;
                }
                let o = self.cursor;
                self.sp.b.get_mut(o..o.wrapping_add(HEADER)).ok_or(Fail::Bad)?.fill(0);
                if let Some(&p) = self.prims.last() {
                    self.sp.wr32(p.wrapping_add(0x54), 1)?;
                }
                self.commit(o)?;
                self.open = Some(o);
                o
            }
        };
        let mut slots = 0usize;
        self.j_now = self.sh.j;
        for (k, &(x, y)) in drawn.iter().enumerate() {
            if slots == 0 {
                slots = self.alloc(drawn.len().saturating_sub(k), &mut hd)?;
                self.open = Some(hd);
                if slots == 0 {
                    return Err(Fail::Bad);
                }
            }
            self.span(hd, x, y, 1, kind)?;
            slots = slots.saturating_sub(1);
        }
        Ok(())
    }

    /// C_RenderSpansAny over one prim: TexBlend, ColorGen (the specular words added and stepped),
    /// the fog, per pixel
    fn render(&mut self, h: usize) -> Res<()> {
        let count = usize::from(self.sp.rd16(h.wrapping_add(4))?);
        let dir: i64 = if self.sp.rd32(h)? == 1 { -1 } else { 1 };
        let sh = self.sh;
        for k in 0..count {
            let s = h.wrapping_add(HEADER).wrapping_add(k.wrapping_mul(SPAN));
            let n = i16::from_le_bytes(self.sp.rd16(s)?.to_le_bytes());
            let x = i64::from(i16::from_le_bytes(self.sp.rd16(s.wrapping_add(4))?.to_le_bytes()));
            let y = i64::from(i16::from_le_bytes(self.sp.rd16(s.wrapping_add(6))?.to_le_bytes()));
            let fogw = |b: &Spans| -> Res<u16> { Ok(b.rd16(s.wrapping_add(0x2e))? >> 8) };
            let f0 = fogw(self.sp)?;
            if sh.fog.is_some() && self.sp.rd16(s.wrapping_add(0x02))? != 0 {
                return Err(Fail::Unmodeled(crate::spec::open::D3D7_TRI_CLIP));
            }
            for j in 0..i64::from(n.max(0)) {
                let mut c = [0u16; 3];
                for (i, ci) in c.iter_mut().enumerate() {
                    *ci = self.sp.rd16(s.wrapping_add(0x20).wrapping_add(i.wrapping_mul(2)))?;
                }
                if sh.gouraud {
                    for i in 0..4usize {
                        let a = s.wrapping_add(0x20).wrapping_add(i.wrapping_mul(2));
                        let d = self.sp.rd16(h.wrapping_add(0x18).wrapping_add(i.wrapping_mul(2)))?;
                        let v = self.sp.rd16(a)?.wrapping_add(d);
                        self.sp.wr16(a, v)?;
                    }
                }
                if sh.specular {
                    for (i, ci) in c.iter_mut().enumerate() {
                        let a = s.wrapping_add(0x28).wrapping_add(i.wrapping_mul(2));
                        let sp = self.sp.rd16(a)?;
                        *ci = ci.saturating_add(sp);
                        let d = self.sp.rd16(h.wrapping_add(0x28).wrapping_add(i.wrapping_mul(2)))?;
                        self.sp.wr16(a, sp.wrapping_add(d))?;
                    }
                }
                if let Some(fc) = sh.fog {
                    let f = u32::from(fogw(self.sp)?);
                    for (ci, &fcc) in c.iter_mut().zip(&fc) {
                        let term = u32::from(fcc).wrapping_mul(255u32.saturating_sub(u32::from(f0)));
                        let v = (u32::from(*ci >> 8).wrapping_mul(f) & 0xffff).wrapping_add(term);
                        *ci = u16::try_from(v.min(0xffff)).unwrap_or(u16::MAX);
                    }
                }
                self.out.push(Px { x: x.wrapping_add(dir.wrapping_mul(j)), y, c });
            }
        }
        Ok(())
    }
}

/// which fill a span takes
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// a triangle's or a line's span (the fill of the prim set)
    Tri,
    /// FillPointSpan: the colour dword shifted up 8 (index 0: 0) and the alpha in J
    Point,
}

/// one D3DTLVERTEX as the front end clips it: screen position, z, rhw
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Tl {
    pub sx: f32,
    pub sy: f32,
    pub sz: f32,
    pub rhw: f32,
    /// the colour dword (index and alpha)
    pub c: u32,
}

/// a 24-bit integer as f32 (exact)
fn f32_u(v: u32) -> f32 {
    let (hi, lo) = (u16::try_from(v >> 16).unwrap_or(0), u16::try_from(v & 0xffff).unwrap_or(0));
    f32::from(hi) * 65536.0 + f32::from(lo)
}

const LEFT: u32 = 0x1000;
const RIGHT: u32 = 0x2000;
const TOP: u32 = 0x4000;
const BOTTOM: u32 = 0x8000;

#[derive(Clone, Copy, Debug)]
struct Cv {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
    flags: u32,
    tl: Tl,
}

/// the viewport w × h at the origin in clip space (D3DFE: x = (sx − 0) / w · cw, y = (sy − h) /
/// −h · cw), its outcodes and the planes Left, Right, Bottom, Top in the order they are cut
struct View {
    w: f32,
    h: f32,
}

type Plane = (u32, fn(&Cv) -> f32);
const PLANES: [Plane; 4] = [(LEFT, |c| c.x), (RIGHT, |c| c.w - c.x), (BOTTOM, |c| c.y), (TOP, |c| c.w - c.y)];

impl View {
    fn code(&self, t: &Tl) -> u32 {
        [(t.sx < 0.0, LEFT), (t.sx >= self.w, RIGHT), (t.sy < 0.0, TOP), (t.sy >= self.h, BOTTOM)].iter().filter(|p| p.0).fold(0, |a, p| a | p.1)
    }

    fn clip_space(&self, t: &Tl, flags: u32) -> Cv {
        let cw = 1.0 / t.rhw;
        Cv { x: (t.sx * cw) * (1.0 / self.w), y: ((t.sy - self.h) * cw) * (1.0 / -self.h), z: t.sz * cw, w: cw, flags, tl: *t }
    }

    /// back to the screen: a vertex cut on a side snapped onto it
    fn screen(&self, c: &Cv) -> Tl {
        let iw = 1.0 / c.w;
        if c.flags == 0 {
            return Tl { rhw: iw, ..c.tl };
        }
        let sx = match c.flags & (LEFT | RIGHT) {
            LEFT => 0.0,
            RIGHT => self.w,
            _ => (((c.x * self.w) * iw) + 0.0).clamp(0.0, self.w),
        };
        let sy = match c.flags & (TOP | BOTTOM) {
            TOP => 0.0,
            BOTTOM => self.h,
            _ => (((c.y * -self.h) * iw) + self.h).clamp(0.0, self.h),
        };
        Tl { sx, sy, sz: iw * c.z, rhw: iw, c: c.tl.c }
    }
}

/// the vertex where a (inside) to b (outside) crosses a plane, lerped from the outside vertex
/// (the index and the alpha as integers, truncated)
fn cut(plane: Plane, a: &Cv, b: &Cv) -> Cv {
    let (p, dist) = plane;
    let (di, dout) = (dist(a), dist(b));
    let t = dout / (dout - di);
    let l = |pi: f32, po: f32| (pi - po) * t + po;
    let li = |ci: u32, co: u32| -> u32 {
        let (fi, fo) = (f32_u(ci), f32_u(co));
        d3dim::trunc((fi - fo) * t + fo).and_then(|v| u32::try_from(v).ok()).unwrap_or(0)
    };
    let c = li(a.tl.c >> 24, b.tl.c >> 24) << 24 | li(a.tl.c & 0xff_ffff, b.tl.c & 0xff_ffff) & 0xff_ffff;
    Cv { x: l(a.x, b.x), y: l(a.y, b.y), z: l(a.z, b.z), w: l(a.w, b.w), flags: (a.flags & b.flags) | p, tl: Tl { c, ..a.tl } }
}

/// the ends the ramp device sets up for one TL line in a viewport w × h at the origin
/// (ClipSingleLine; the ramp device has no guard band, so any end off the viewport is cut):
/// itself when inside, else each end outside a plane replaced by the crossing, Left, Right,
/// Bottom, Top, a new end snapped to its side; None when the line misses the viewport
pub fn clip_line(v: [Tl; 2], w: f32, h: f32) -> Option<[Tl; 2]> {
    let view = View { w, h };
    let codes = v.map(|t| view.code(&t));
    let any = codes.iter().fold(0, |a, &c| a | c);
    if any == 0 {
        return Some(v);
    }
    let [(t0, c0), (t1, c1)] = [(v[0], codes[0]), (v[1], codes[1])];
    let (mut a, mut b) = (view.clip_space(&t0, c0), view.clip_space(&t1, c1));
    for plane in PLANES {
        if any & plane.0 == 0 {
            continue;
        }
        let inside = |c: &Cv| !(plane.1)(c).is_sign_negative();
        match (inside(&a), inside(&b)) {
            (true, true) => {}
            (false, false) => return None,
            (true, false) => b = cut(plane, &a, &b),
            (false, true) => a = cut(plane, &b, &a),
        }
    }
    Some([view.screen(&a), view.screen(&b)])
}

/// the triangles the ramp device rasterizes for one TL triangle in a viewport w × h at the
/// origin: itself when inside, else its polygon clipped Left, Right, Bottom, Top (each pass
/// emitting the previous vertex, a new one lerped from the outside vertex) and drawn as the fan
/// (v[i], v[i+1], v[0]); a new vertex snapped to the side it was cut on
pub fn clip(v: [Tl; 3], w: f32, h: f32) -> Vec<[Tl; 3]> {
    let view = View { w, h };
    let codes = v.map(|t| view.code(&t));
    let any = codes.iter().fold(0, |a, &c| a | c);
    if any == 0 {
        return vec![v];
    }
    let mut poly: Vec<Cv> = v.iter().zip(codes).map(|(t, flags)| view.clip_space(t, flags)).collect();
    for plane in PLANES {
        if any & plane.0 == 0 || poly.is_empty() {
            continue;
        }
        let inside = |c: &Cv| !(plane.1)(c).is_sign_negative();
        let mut out = Vec::new();
        let Some(&last) = poly.last() else { break };
        let mut prev = last;
        for &cur in &poly {
            if inside(&prev) {
                out.push(prev);
                if !inside(&cur) {
                    out.push(cut(plane, &prev, &cur));
                }
            } else if inside(&cur) {
                out.push(cut(plane, &cur, &prev));
            }
            prev = cur;
        }
        poly = out;
    }
    let screen: Vec<Tl> = poly.iter().map(|c| view.screen(c)).collect();
    let mut fan = Vec::new();
    if let Some(&v0) = screen.first() {
        for p in screen.windows(2).skip(1) {
            if let [a, b] = p {
                fan.push([*a, *b, v0]);
            }
        }
    }
    fan
}
