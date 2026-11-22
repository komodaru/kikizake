//! Vertex arrays (GL 1.1 2.8): the executor's array slots, the six arrays' pointers, sizes, types
//! and strides, glArrayElement / glDrawArrays / glDrawElements as the immediate calls they stand
//! for, glInterleavedArrays' layouts, the client attribute stack and edge flags.

use super::e::{BYTE, FLOAT, INT, INVALID_ENUM, INVALID_OPERATION, INVALID_VALUE, SHORT, STACK_OVERFLOW, STACK_UNDERFLOW, UNSIGNED_BYTE, UNSIGNED_INT, UNSIGNED_SHORT};
use super::{Ctx, Fail, Res, A};
use crate::spec::{clause, hit};

const DOUBLE: u32 = 0x140a;
/// one slot's bytes and the slots the executor keeps (runner/opengl.c pslot)
const SLOT: usize = 4096;
const SLOTS: u8 = 16;
/// GL_MAX_CLIENT_ATTRIB_STACK_DEPTH
const CLIENT_DEPTH: usize = 16;
const CLIENT_PIXEL_STORE_BIT: i64 = 1;
const CLIENT_VERTEX_ARRAY_BIT: i64 = 2;

/// One of the executor's 16 array slots (runner/opengl.c pslot), used in turn from the first at
/// each glctx; contiguous, so an array read past one goes on into the next.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SlotId(u8);

impl SlotId {
    fn start(self) -> usize {
        usize::from(self.0).saturating_mul(SLOT)
    }
    /// as glq prints a pointer into it: its number from 1
    pub fn shown(self) -> u8 {
        self.0.saturating_add(1)
    }
}

/// what an array pointer points at
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ptr {
    Null,
    /// `off` bytes into a slot
    Slot(SlotId, usize),
    /// the executor's output buffer (glp .. obuf)
    Other,
}

/// the six arrays
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Vertex,
    Normal,
    Color,
    Index,
    Tex,
    Edge,
}

impl Kind {
    pub const ALL: [Kind; 6] = [Kind::Vertex, Kind::Normal, Kind::Color, Kind::Index, Kind::Tex, Kind::Edge];

    /// GL_VERTEX_ARRAY .. GL_EDGE_FLAG_ARRAY (glEnableClientState, glIsEnabled)
    pub fn cap(self) -> u32 {
        match self {
            Kind::Vertex => 0x8074,
            Kind::Normal => 0x8075,
            Kind::Color => 0x8076,
            Kind::Index => 0x8077,
            Kind::Tex => 0x8078,
            Kind::Edge => 0x8079,
        }
    }

    pub fn of_cap(c: u32) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.cap() == c)
    }

    /// GL_VERTEX_ARRAY_POINTER .. GL_EDGE_FLAG_ARRAY_POINTER (glGetPointerv)
    fn pointer(self) -> u32 {
        match self {
            Kind::Vertex => 0x808e,
            Kind::Normal => 0x808f,
            Kind::Color => 0x8090,
            Kind::Index => 0x8091,
            Kind::Tex => 0x8092,
            Kind::Edge => 0x8093,
        }
    }

    fn of_function(name: &str) -> Option<Kind> {
        Some(match name {
            "glVertexPointer" => Kind::Vertex,
            "glNormalPointer" => Kind::Normal,
            "glColorPointer" => Kind::Color,
            "glIndexPointer" => Kind::Index,
            "glTexCoordPointer" => Kind::Tex,
            "glEdgeFlagPointer" => Kind::Edge,
            _ => return None,
        })
    }

    /// the sizes it takes (GL 1.1 table 2.4); fixed ones not given
    fn sizes(self) -> &'static [i64] {
        match self {
            Kind::Vertex => &[2, 3, 4],
            Kind::Normal => &[3],
            Kind::Color => &[3, 4],
            Kind::Index | Kind::Edge => &[1],
            Kind::Tex => &[1, 2, 3, 4],
        }
    }

    /// the types it takes (GL 1.1 table 2.4); an edge flag's is a byte, never given
    fn types(self) -> &'static [u32] {
        match self {
            Kind::Vertex | Kind::Tex => &[SHORT, INT, FLOAT, DOUBLE],
            Kind::Normal => &[BYTE, SHORT, INT, FLOAT, DOUBLE],
            Kind::Color => &[BYTE, UNSIGNED_BYTE, SHORT, UNSIGNED_SHORT, INT, UNSIGNED_INT, FLOAT, DOUBLE],
            Kind::Index => &[UNSIGNED_BYTE, SHORT, INT, FLOAT, DOUBLE],
            Kind::Edge => &[UNSIGNED_BYTE],
        }
    }

    /// what it holds at first (GL 1.1 table 6.6)
    fn initial(self) -> Spec {
        let (size, ty) = match self {
            Kind::Vertex | Kind::Color | Kind::Tex => (4, FLOAT),
            Kind::Normal => (3, FLOAT),
            Kind::Index => (1, FLOAT),
            Kind::Edge => (1, UNSIGNED_BYTE),
        };
        Spec { size, ty, stride: 0, ptr: Ptr::Null }
    }
}

/// an array's pointer, components a element, type and stride (0: packed)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Spec {
    pub size: i64,
    pub ty: u32,
    pub stride: i64,
    pub ptr: Ptr,
}

/// the bytes of one element of a type
fn type_size(ty: u32) -> Option<usize> {
    Some(match ty {
        BYTE | UNSIGNED_BYTE => 1,
        SHORT | UNSIGNED_SHORT => 2,
        INT | UNSIGNED_INT | FLOAT => 4,
        DOUBLE => 8,
        _ => return None,
    })
}

/// The arrays of a context and the executor's slots they point into.
#[derive(Clone, Debug)]
pub struct Arrays {
    vertex: Spec,
    normal: Spec,
    color: Spec,
    index: Spec,
    tex: Spec,
    edge: Spec,
    /// the 16 slots, contiguous
    mem: Vec<u8>,
    next: u8,
    /// the slot the line being run filled (glp with values), what a pointer call points at
    pub last: Option<Ptr>,
    /// glPushClientAttrib's saves: the mask, the arrays, their enables, the pixel stores
    stack: Vec<(i64, ClientSaved)>,
}

#[derive(Clone, Debug)]
struct ClientSaved {
    specs: [Spec; 6],
    enabled: [bool; 6],
    pack: [i64; 6],
    unpack: [i64; 6],
}

impl Default for Arrays {
    fn default() -> Arrays {
        Arrays {
            vertex: Kind::Vertex.initial(),
            normal: Kind::Normal.initial(),
            color: Kind::Color.initial(),
            index: Kind::Index.initial(),
            tex: Kind::Tex.initial(),
            edge: Kind::Edge.initial(),
            mem: vec![0; SLOT.saturating_mul(usize::from(SLOTS))],
            next: 0,
            last: None,
            stack: Vec::new(),
        }
    }
}

impl Arrays {
    pub fn spec(&self, k: Kind) -> Spec {
        match k {
            Kind::Vertex => self.vertex,
            Kind::Normal => self.normal,
            Kind::Color => self.color,
            Kind::Index => self.index,
            Kind::Tex => self.tex,
            Kind::Edge => self.edge,
        }
    }

    fn spec_mut(&mut self, k: Kind) -> &mut Spec {
        match k {
            Kind::Vertex => &mut self.vertex,
            Kind::Normal => &mut self.normal,
            Kind::Color => &mut self.color,
            Kind::Index => &mut self.index,
            Kind::Tex => &mut self.tex,
            Kind::Edge => &mut self.edge,
        }
    }

    /// a glp line's values into the next slot, as the executor does (zeroed, then the values as
    /// their element type, little-endian): what its pointer is; None for obuf, null or none
    pub fn fill(&mut self, t: &[String]) -> Res<Option<Ptr>> {
        let Some(bytes) = glp_bytes(t)? else {
            return Ok(match glp_tail(t)? {
                Tail::Null => Some(Ptr::Null),
                Tail::Obuf => Some(Ptr::Other),
                Tail::Values(..) => None,
            });
        };
        let id = SlotId(self.next);
        self.next = self.next.saturating_add(1).checked_rem(SLOTS).unwrap_or(0);
        let start = id.start();
        let slot = self.mem.get_mut(start..start.saturating_add(SLOT)).ok_or(Fail::Bad)?;
        slot.fill(0);
        for (d, s) in slot.iter_mut().zip(&bytes) {
            *d = *s;
        }
        Ok(Some(Ptr::Slot(id, 0)))
    }

    /// `n` bytes at element `i` of array `s` (its stride, else packed)
    fn element(&self, s: Spec, i: i64, n: usize) -> Res<&[u8]> {
        let Ptr::Slot(id, off) = s.ptr else { return Err(Fail::Bad) };
        let size = usize::try_from(s.size).ok().and_then(|z| z.checked_mul(type_size(s.ty)?)).ok_or(Fail::Bad)?;
        let stride = if s.stride == 0 { size } else { usize::try_from(s.stride).ok().ok_or(Fail::Bad)? };
        let at = usize::try_from(i).ok().and_then(|i| i.checked_mul(stride)).and_then(|o| o.checked_add(off)).and_then(|o| o.checked_add(id.start())).ok_or(Fail::Bad)?;
        // (past the 16 slots: memory the executor does not own, not done by a case)
        self.mem.get(at..at.checked_add(n).ok_or(Fail::Bad)?).ok_or(Fail::Bad)
    }

    /// element `i` of array `k`: its components as doubles, as the type holds them
    fn values(&self, k: Kind, i: i64) -> Res<Vec<f64>> {
        let s = self.spec(k);
        let w = type_size(s.ty).ok_or(Fail::Bad)?;
        let n = usize::try_from(s.size).ok().ok_or(Fail::Bad)?;
        let b = self.element(s, i, n.checked_mul(w).ok_or(Fail::Bad)?)?;
        b.chunks_exact(w).map(|c| read(s.ty, c)).collect()
    }
}

/// the value of one element of type `ty` (little-endian bytes)
pub fn read(ty: u32, c: &[u8]) -> Res<f64> {
    let a = |n: usize| -> Res<[u8; 8]> {
        let mut v = [0u8; 8];
        for (d, s) in v.iter_mut().zip(c.iter().take(n)) {
            *d = *s;
        }
        Ok(v)
    };
    let [b0, b1, b2, b3, b4, b5, b6, b7] = a(8)?;
    Ok(match ty {
        BYTE => f64::from(i8::from_le_bytes([b0])),
        UNSIGNED_BYTE => f64::from(b0),
        SHORT => f64::from(i16::from_le_bytes([b0, b1])),
        UNSIGNED_SHORT => f64::from(u16::from_le_bytes([b0, b1])),
        INT => f64::from(i32::from_le_bytes([b0, b1, b2, b3])),
        UNSIGNED_INT => f64::from(u32::from_le_bytes([b0, b1, b2, b3])),
        FLOAT => f64::from(f32::from_le_bytes([b0, b1, b2, b3])),
        DOUBLE => f64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7]),
        _ => return Err(Fail::Bad),
    })
}

/// a color or normal component of an integer type as GL 1.1 table 2.6 maps it to [0, 1] or
/// [-1, 1], a float as it is (in float32)
pub fn normalized(ty: u32, v: f64) -> f32 {
    let f = match ty {
        UNSIGNED_BYTE => v / 255.0,
        BYTE => (2.0 * v + 1.0) / 255.0,
        UNSIGNED_SHORT => v / 65535.0,
        SHORT => (2.0 * v + 1.0) / 65535.0,
        UNSIGNED_INT => v / 4_294_967_295.0,
        INT => (2.0 * v + 1.0) / 4_294_967_295.0,
        _ => v,
    };
    super::tri::narrow(f)
}

/// what follows a glp line's leading arguments
enum Tail<'a> {
    Null,
    Obuf,
    /// the element type and the values
    Values(&'a str, &'a [String]),
}

/// a glp line `glp <name> <sig> <leading args> <type> <values> | obuf | null`
fn glp_tail(t: &[String]) -> Res<Tail<'_>> {
    let sig = t.get(2).ok_or(Fail::Bad)?;
    let lead = if sig == "-" { 0 } else { sig.len() };
    let at = lead.checked_add(3).ok_or(Fail::Bad)?;
    let rest = t.get(at..).ok_or(Fail::Bad)?;
    Ok(match rest {
        [x] if x == "null" => Tail::Null,
        [x] if x == "obuf" => Tail::Obuf,
        [ty, values @ ..] => Tail::Values(ty.as_str(), values),
        [] => return Err(Fail::Bad),
    })
}

/// a glp line's leading integer arguments
fn glp_ints(t: &[String]) -> Res<Vec<i64>> {
    let sig = t.get(2).ok_or(Fail::Bad)?;
    if sig == "-" {
        return Ok(Vec::new());
    }
    if sig.chars().any(|c| c != 'i') {
        return Err(Fail::Bad);
    }
    t.iter().skip(3).take(sig.len()).map(|x| crate::case::num(x).ok_or(Fail::Bad)).collect()
}

/// a glp line's values as the executor lays them out, None for obuf or null
pub fn glp_bytes(t: &[String]) -> Res<Option<Vec<u8>>> {
    let Tail::Values(ty, values) = glp_tail(t)? else { return Ok(None) };
    let mut out = Vec::new();
    for v in values {
        match ty {
            "f" => out.extend(v.parse::<f32>().ok().ok_or(Fail::Bad)?.to_le_bytes()),
            "d" => out.extend(v.parse::<f64>().ok().ok_or(Fail::Bad)?.to_le_bytes()),
            _ => {
                let size = match ty {
                    "b" | "ub" => 1,
                    "s" | "us" => 2,
                    "i" | "ui" => 4,
                    _ => return Err(Fail::Bad),
                };
                let n = crate::case::num(v).ok_or(Fail::Bad)?;
                out.extend(n.to_le_bytes().iter().take(size));
            }
        }
    }
    if out.len() > SLOT {
        return Err(Fail::Bad);
    }
    Ok(Some(out))
}

/// the layouts of glInterleavedArrays (GL 1.1 table 2.5): texture coordinates, color (size and
/// whether bytes), normal, vertex size
struct Layout {
    tex: Option<i64>,
    color: Option<(i64, bool)>,
    normal: bool,
    vertex: i64,
}

fn layout(format: u32) -> Option<Layout> {
    let l = |tex, color, normal, vertex| Layout { tex, color, normal, vertex };
    Some(match format {
        0x2a20 => l(None, None, false, 2),
        0x2a21 => l(None, None, false, 3),
        0x2a22 => l(None, Some((4, true)), false, 2),
        0x2a23 => l(None, Some((4, true)), false, 3),
        0x2a24 => l(None, Some((3, false)), false, 3),
        0x2a25 => l(None, None, true, 3),
        0x2a26 => l(None, Some((4, false)), true, 3),
        0x2a27 => l(Some(2), None, false, 3),
        0x2a28 => l(Some(4), None, false, 4),
        0x2a29 => l(Some(2), Some((4, true)), false, 3),
        0x2a2a => l(Some(2), Some((3, false)), false, 3),
        0x2a2b => l(Some(2), None, true, 3),
        0x2a2c => l(Some(2), Some((4, false)), true, 3),
        0x2a2d => l(Some(4), Some((4, false)), true, 4),
        _ => return None,
    })
}

/// The arrays and their enables as a list compiled them (opengl_list.rs Line)
#[derive(Clone, Debug)]
pub struct Snapshot {
    arrays: Arrays,
    enabled: [bool; 6],
}

impl Snapshot {
    pub fn of(ctx: &Ctx) -> Snapshot {
        Snapshot { arrays: ctx.arrays.clone(), enabled: Kind::ALL.map(|k| ctx.client_enabled(k)) }
    }

    /// puts these in `ctx`, giving back what it held
    pub fn swap(self, ctx: &mut Ctx) -> Snapshot {
        let was = Snapshot::of(ctx);
        // (the client stack and the slot being filled stay the context's)
        let Snapshot { mut arrays, enabled } = self;
        arrays.stack = std::mem::take(&mut ctx.arrays.stack);
        arrays.next = ctx.arrays.next;
        arrays.last = ctx.arrays.last;
        ctx.arrays = arrays;
        for (k, on) in Kind::ALL.into_iter().zip(enabled) {
            ctx.set_client(k, on);
        }
        was
    }
}

/// whether a glp line's function is one of arrays (Ctx::array_glp)
pub fn is_glp(name: &str) -> bool {
    Kind::of_function(name).is_some() || matches!(name, "glInterleavedArrays" | "glDrawElements" | "glEdgeFlagv")
}

impl Ctx {
    fn client_enabled(&self, k: Kind) -> bool {
        self.caps.get(&k.cap()).copied().unwrap_or(false)
    }

    fn set_client(&mut self, k: Kind, on: bool) {
        self.caps.insert(k.cap(), on);
    }

    /// the gl calls of arrays and edge flags (glEnableClientState .. glEdgeFlag)
    pub(super) fn array_call(&mut self, name: &str, a: &[A]) -> Res<()> {
        hit(clause::GL_ARRAY);
        let i = |k: usize| super::int(a, k);
        match name {
            "glEnableClientState" | "glDisableClientState" => match u32::try_from(i(0) & 0xFFFF_FFFF).ok().and_then(Kind::of_cap) {
                Some(k) => self.set_client(k, name == "glEnableClientState"),
                None => self.error(INVALID_ENUM),
            },
            "glEdgeFlag" => self.edge_flag = i(0) != 0,
            "glArrayElement" => self.array_element(i(0))?,
            "glDrawArrays" => {
                let (mode, first, count) = (i(0), i(1), i(2));
                if let Some(e) = self.draw_error(mode, count) {
                    self.error(e);
                    return Ok(());
                }
                let elements: Vec<i64> = (0..count).map(|k| first.saturating_add(k)).collect();
                self.draw_elements(mode, &elements)?;
            }
            "glPushClientAttrib" => {
                if self.arrays.stack.len() >= CLIENT_DEPTH {
                    self.error(STACK_OVERFLOW);
                } else {
                    let enabled = Kind::ALL.map(|k| self.client_enabled(k));
                    let saved = ClientSaved { specs: Kind::ALL.map(|k| self.arrays.spec(k)), enabled, pack: self.pack, unpack: self.unpack };
                    self.arrays.stack.push((i(0), saved));
                }
            }
            "glPopClientAttrib" => match self.arrays.stack.pop() {
                None => self.error(STACK_UNDERFLOW),
                Some((mask, saved)) => {
                    if mask & CLIENT_VERTEX_ARRAY_BIT != 0 {
                        for ((k, s), on) in Kind::ALL.into_iter().zip(saved.specs).zip(saved.enabled) {
                            *self.arrays.spec_mut(k) = s;
                            self.set_client(k, on);
                        }
                    }
                    if mask & CLIENT_PIXEL_STORE_BIT != 0 {
                        self.pack = saved.pack;
                        self.unpack = saved.unpack;
                    }
                }
            },
            _ => return Err(Fail::Bad),
        }
        Ok(())
    }

    /// an error of glDrawArrays / glDrawElements before anything is drawn: inside glBegin,
    /// a mode past GL_POLYGON, a negative count
    fn draw_error(&self, mode: i64, count: i64) -> Option<u32> {
        if self.in_begin.is_some() {
            Some(INVALID_OPERATION)
        } else if !(0..=9).contains(&mode) {
            Some(INVALID_ENUM)
        } else if count < 0 {
            Some(INVALID_VALUE)
        } else {
            None
        }
    }

    /// glBegin(mode), glArrayElement of each element, glEnd
    fn draw_elements(&mut self, mode: i64, elements: &[i64]) -> Res<()> {
        self.call("glBegin", &[A::I(mode)])?;
        for e in elements {
            self.array_element(*e)?;
        }
        self.call("glEnd", &[])
    }

    /// glArrayElement(i): each enabled array's element i as the call it stands for, the vertex
    /// last (GL 1.1 2.8)
    pub(super) fn array_element(&mut self, i: i64) -> Res<()> {
        if self.client_enabled(Kind::Edge) {
            let v = self.arrays.values(Kind::Edge, i)?;
            self.edge_flag = v.first().is_some_and(|x| x.to_bits() & 0x7FFF_FFFF_FFFF_FFFF != 0);
        }
        if self.client_enabled(Kind::Tex) {
            let v = self.arrays.values(Kind::Tex, i)?;
            let c = |k: usize, d: f32| v.get(k).map_or(d, |x| super::tri::narrow(*x));
            self.tex.coord = [c(0, 0.0), c(1, 0.0), c(2, 0.0), c(3, 1.0)];
        }
        if self.client_enabled(Kind::Color) {
            let ty = self.arrays.spec(Kind::Color).ty;
            let v = self.arrays.values(Kind::Color, i)?;
            let c = |k: usize| v.get(k).map_or(1.0, |x| normalized(ty, *x));
            self.color = [c(0), c(1), c(2), c(3)];
        }
        if self.client_enabled(Kind::Index) {
            let v = self.arrays.values(Kind::Index, i)?;
            self.index = v.first().map_or(0.0, |x| super::tri::narrow(*x));
        }
        if self.client_enabled(Kind::Normal) {
            let ty = self.arrays.spec(Kind::Normal).ty;
            let v = self.arrays.values(Kind::Normal, i)?;
            let c = |k: usize| v.get(k).map_or(0.0, |x| normalized(ty, *x));
            self.normal = [c(0), c(1), c(2)];
        }
        if self.client_enabled(Kind::Vertex) {
            let v = self.arrays.values(Kind::Vertex, i)?;
            let f: Vec<A> = v.iter().map(|x| A::F(f64::from(super::tri::narrow(*x)))).collect();
            let name = match f.len() {
                2 => "glVertex2f",
                3 => "glVertex3f",
                4 => "glVertex4f",
                _ => return Err(Fail::Bad),
            };
            self.call(name, &f)?;
        }
        Ok(())
    }

    /// a glp line of arrays (the pointers, glInterleavedArrays, glDrawElements, glEdgeFlagv);
    /// `at` what the executor's slot for the line holds
    pub(super) fn array_glp(&mut self, t: &[String], at: Option<Ptr>) -> Res<()> {
        hit(clause::GL_ARRAY);
        let name = t.get(1).map_or("", String::as_str);
        let v = glp_ints(t)?;
        let n = |k: usize| v.get(k).copied().ok_or(Fail::Bad);
        if let Some(k) = Kind::of_function(name) {
            // (size, type, stride; glNormalPointer and glIndexPointer type, stride;
            // glEdgeFlagPointer stride)
            let (size, ty, stride) = match k {
                Kind::Vertex | Kind::Color | Kind::Tex => (n(0)?, n(1)?, n(2)?),
                Kind::Normal => (3, n(0)?, n(1)?),
                Kind::Index => (1, n(0)?, n(1)?),
                Kind::Edge => (1, i64::from(UNSIGNED_BYTE), n(0)?),
            };
            let ty = u32::try_from(ty & 0xFFFF_FFFF).ok().ok_or(Fail::Bad)?;
            if !k.sizes().contains(&size) || stride < 0 {
                self.error(INVALID_VALUE);
            } else if !k.types().contains(&ty) {
                self.error(INVALID_ENUM);
            } else {
                *self.arrays.spec_mut(k) = Spec { size, ty, stride, ptr: at.ok_or(Fail::Bad)? };
            }
            return Ok(());
        }
        match name {
            "glInterleavedArrays" => self.interleaved(u32::try_from(n(0)? & 0xFFFF_FFFF).ok().ok_or(Fail::Bad)?, n(1)?, at.ok_or(Fail::Bad)?),
            "glDrawElements" => {
                let (mode, count, ty) = (n(0)?, n(1)?, n(2)?);
                if let Some(e) = self.draw_error(mode, count) {
                    self.error(e);
                    return Ok(());
                }
                let w = match u32::try_from(ty & 0xFFFF_FFFF).ok() {
                    Some(UNSIGNED_BYTE) => 1,
                    Some(UNSIGNED_SHORT) => 2,
                    Some(UNSIGNED_INT) => 4,
                    _ => {
                        self.error(INVALID_ENUM);
                        return Ok(());
                    }
                };
                let ty = u32::try_from(ty & 0xFFFF_FFFF).ok().ok_or(Fail::Bad)?;
                let bytes = glp_bytes(t)?.ok_or(Fail::Bad)?;
                let mut elements = Vec::new();
                for k in 0..usize::try_from(count).ok().ok_or(Fail::Bad)? {
                    let at = k.checked_mul(w).ok_or(Fail::Bad)?;
                    let c = bytes.get(at..at.checked_add(w).ok_or(Fail::Bad)?).unwrap_or(&[]);
                    let e = read(ty, c)?;
                    elements.push(super::tri::trunc(super::tri::narrow(e)));
                }
                self.draw_elements(mode, &elements)
            }
            "glEdgeFlagv" => {
                let bytes = glp_bytes(t)?.ok_or(Fail::Bad)?;
                self.edge_flag = bytes.first().is_some_and(|b| *b != 0);
                Ok(())
            }
            _ => Err(Fail::Bad),
        }
    }

    /// glInterleavedArrays(format, stride, p): the arrays of the layout enabled and pointed into
    /// p at their offsets, the others disabled (GL 1.1 2.8, table 2.5)
    fn interleaved(&mut self, format: u32, stride: i64, at: Ptr) -> Res<()> {
        let Some(l) = layout(format) else {
            self.error(INVALID_ENUM);
            return Ok(());
        };
        if stride < 0 {
            self.error(INVALID_VALUE);
            return Ok(());
        }
        let f = 4i64;
        let pc = l.tex.unwrap_or(0).saturating_mul(f);
        let pn = pc.saturating_add(match l.color {
            Some((_, true)) => f,
            Some((n, false)) => n.saturating_mul(f),
            None => 0,
        });
        let pv = pn.saturating_add(if l.normal { 3i64.saturating_mul(f) } else { 0 });
        let s = if stride == 0 { pv.saturating_add(l.vertex.saturating_mul(f)) } else { stride };
        let at_off = |o: i64| -> Res<Ptr> {
            Ok(match at {
                Ptr::Slot(id, off) => Ptr::Slot(id, off.checked_add(usize::try_from(o).ok().ok_or(Fail::Bad)?).ok_or(Fail::Bad)?),
                Ptr::Null | Ptr::Other => at,
            })
        };
        self.set_client(Kind::Edge, false);
        self.set_client(Kind::Index, false);
        self.set_client(Kind::Tex, l.tex.is_some());
        if let Some(n) = l.tex {
            *self.arrays.spec_mut(Kind::Tex) = Spec { size: n, ty: FLOAT, stride: s, ptr: at_off(0)? };
        }
        self.set_client(Kind::Color, l.color.is_some());
        if let Some((n, bytes)) = l.color {
            *self.arrays.spec_mut(Kind::Color) = Spec { size: n, ty: if bytes { UNSIGNED_BYTE } else { FLOAT }, stride: s, ptr: at_off(pc)? };
        }
        self.set_client(Kind::Normal, l.normal);
        if l.normal {
            *self.arrays.spec_mut(Kind::Normal) = Spec { size: 3, ty: FLOAT, stride: s, ptr: at_off(pn)? };
        }
        self.set_client(Kind::Vertex, true);
        *self.arrays.spec_mut(Kind::Vertex) = Spec { size: l.vertex, ty: FLOAT, stride: s, ptr: at_off(pv)? };
        Ok(())
    }

    /// glGetPointerv(pname): the slot an array points into (from 1), 0 for none, -1 elsewhere
    pub(super) fn array_pointer(&mut self, pname: u32) -> Option<i64> {
        let k = Kind::ALL.into_iter().find(|k| k.pointer() == pname);
        let Some(k) = k else {
            self.error(INVALID_ENUM);
            return None;
        };
        Some(match self.arrays.spec(k).ptr {
            Ptr::Null => 0,
            Ptr::Slot(id, _) => i64::from(id.shown()),
            Ptr::Other => -1,
        })
    }

    /// glGet of an array's size, type or stride, the edge flag and the client stack's depth
    pub(super) fn array_get(&self, p: u32) -> Option<i64> {
        let s = |k: Kind| self.arrays.spec(k);
        let ty = |k: Kind| i64::from(s(k).ty);
        Some(match p {
            0x807a => s(Kind::Vertex).size,
            0x807b => ty(Kind::Vertex),
            0x807c => s(Kind::Vertex).stride,
            0x807e => ty(Kind::Normal),
            0x807f => s(Kind::Normal).stride,
            0x8081 => s(Kind::Color).size,
            0x8082 => ty(Kind::Color),
            0x8083 => s(Kind::Color).stride,
            0x8085 => ty(Kind::Index),
            0x8086 => s(Kind::Index).stride,
            0x8088 => s(Kind::Tex).size,
            0x8089 => ty(Kind::Tex),
            0x808a => s(Kind::Tex).stride,
            0x808c => s(Kind::Edge).stride,
            0xbb1 => i64::try_from(self.arrays.stack.len()).unwrap_or(0),
            _ => return None,
        })
    }
}
