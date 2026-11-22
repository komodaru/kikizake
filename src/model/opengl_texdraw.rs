//! Textured polygons of the GDI generic OpenGL 1.1 (GL 1.1 3.8.5 .. 3.8.7): which texture a
//! fragment reads, its coordinates' planes, the filters, wrapping and mipmaps, and the texture
//! environment.
//!
//! The rules come from clean-room notes of opengl32.dll (behaviour only, no code) and each is
//! confirmed by the goldens of the tex-open-* cases (spec/opengl.txt GL.RASTER.TEXTURE).

use super::e::{ALPHA_TEST, BLEND as BLENDING, COLOR_LOGIC_OP, DEPTH_TEST, DITHER, FLAT, FRONT_AND_BACK, LESS, ONE_MINUS_SRC_ALPHA, POLYGON_MODE_POLYGON, POLYGON_STIPPLE, POLY_FILL, QUADS, QUAD_STRIP, SRC_ALPHA, STENCIL_TEST, TRIANGLES, TRIANGLE_FAN, TRIANGLE_STRIP};
use super::tex::{Base, Level, Object, Store, Target, TEXTURE_1D, TEXTURE_2D};
use std::collections::BTreeMap;

use super::{tri, Ctx, Fail, FixedZ, Frag, Obj, Res, ToFixed};
use crate::spec::{clause, hit, open};

const NEAREST: u32 = 0x2600;
const LINEAR: u32 = 0x2601;
const NEAREST_MIPMAP_NEAREST: u32 = 0x2700;
const LINEAR_MIPMAP_NEAREST: u32 = 0x2701;
const NEAREST_MIPMAP_LINEAR: u32 = 0x2702;
const LINEAR_MIPMAP_LINEAR: u32 = 0x2703;
const CLAMP: u32 = 0x2900;
const REPEAT: u32 = 0x2901;
const MODULATE: u32 = 0x2100;
const DECAL: u32 = 0x2101;
const BLEND: u32 = 0x0be2;
const REPLACE: u32 = 0x1e01;
const EYE_LINEAR: u32 = 0x2400;
const OBJECT_LINEAR: u32 = 0x2401;
/// GL_TEXTURE_GEN_S .. Q
const GENS: [u32; 4] = [0xc60, 0xc61, 0xc62, 0xc63];
/// the channel scale of the 8-bit channels
const S: f32 = 255.0;

/// A pixel's texture coordinates s / w, t / w, q / w and their change per pixel in x and y.
#[derive(Clone, Copy, Debug)]
pub struct Stq {
    pub at: [f32; 3],
    pub dx: [f32; 3],
    pub dy: [f32; 3],
}

impl Stq {
    pub const NONE: Stq = Stq { at: [0.0; 3], dx: [0.0; 3], dy: [0.0; 3] };

    /// coordinates (s, t, q) that do not change from pixel to pixel
    pub const fn at(at: [f32; 3]) -> Stq {
        Stq { at, dx: [0.0; 3], dy: [0.0; 3] }
    }

    /// the coordinates at each pixel of a triangle `at` whose corners have `value` (s / w,
    /// t / w, q / w), each walked as the generic span walks them (`tri::walk`)
    pub fn walk(at: [tri::Vertex; 3], value: [[f64; 4]; 3], w: i64, h: i64) -> BTreeMap<(i64, i64), Stq> {
        let mut out: BTreeMap<(i64, i64), Stq> = BTreeMap::new();
        for k in 0..3 {
            let (px, (a, b)) = tri::walk(at, value.map(|v| tri::narrow(v.get(k).copied().unwrap_or(0.0))), w, h);
            for (x, y, v) in px {
                let e = out.entry((x, y)).or_insert(Stq::NONE);
                if let (Some(p), Some(dx), Some(dy)) = (e.at.get_mut(k), e.dx.get_mut(k), e.dy.get_mut(k)) {
                    *p = v;
                    *dx = a;
                    *dy = b;
                }
            }
        }
        out
    }
}

/// A line's texture coordinates: those at its first pixel and their change a pixel along its
/// major axis.
#[derive(Clone, Copy, Debug)]
pub struct LineTex {
    c0: [f32; 4],
    step: [f32; 4],
    first: i64,
    sign: i64,
    y_major: bool,
}

impl LineTex {
    /// at pixel (x, y)
    pub fn at(&self, x: i64, y: i64) -> Stq {
        let j = if self.y_major { y } else { x };
        let n = tri::float(j.saturating_sub(self.first).saturating_mul(self.sign));
        let [s, t, q, _]: [f32; 4] = std::array::from_fn(|k| self.c0.get(k).copied().unwrap_or(0.0) + n * self.step.get(k).copied().unwrap_or(0.0));
        Stq::at([s, t, q])
    }
}

/// A filter (GL_TEXTURE_MIN_FILTER, GL_TEXTURE_MAG_FILTER).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Filter {
    Nearest,
    Linear,
    /// a mipmap filter: the filter in a level, and whether two levels are mixed
    Mip { linear: bool, between: bool },
}

impl Filter {
    fn of(v: u32) -> Option<Filter> {
        Some(match v {
            NEAREST => Filter::Nearest,
            LINEAR => Filter::Linear,
            NEAREST_MIPMAP_NEAREST => Filter::Mip { linear: false, between: false },
            LINEAR_MIPMAP_NEAREST => Filter::Mip { linear: true, between: false },
            NEAREST_MIPMAP_LINEAR => Filter::Mip { linear: false, between: true },
            LINEAR_MIPMAP_LINEAR => Filter::Mip { linear: true, between: true },
            _ => return None,
        })
    }
}

/// A wrap mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    Repeat,
    Clamp,
}

impl Wrap {
    fn of(v: u32) -> Option<Wrap> {
        match v {
            REPEAT => Some(Wrap::Repeat),
            CLAMP => Some(Wrap::Clamp),
            _ => None,
        }
    }
}

/// A texture environment mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Env {
    Modulate,
    Decal,
    Blend,
    Replace,
}

impl Env {
    fn of(v: u32) -> Option<Env> {
        Some(match v {
            MODULATE => Env::Modulate,
            DECAL => Env::Decal,
            BLEND => Env::Blend,
            REPLACE => Env::Replace,
            _ => return None,
        })
    }
}

/// The fixed-point span routines of a 32-bit DIB (texture.md 7.1, 7.3): the texel's bytes,
/// the color's in 16.16
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Fast {
    /// MODULATE: (c t) >> 8 a channel, the color stepped in 16.16 from one half when smooth
    Modulate { smooth: bool },
    /// REPLACE (or DECAL on RGB) undithered: the texel's bytes
    Copy,
}

/// GL_NICEST
const NICEST: u32 = 0x1102;
/// GL_LEQUAL
const LEQUAL: u32 = 0x203;

/// The texture a fragment reads: its levels down to the last of a mipmap chain, how it is
/// filtered and wrapped, and the border color.
struct Bound<'a> {
    levels: Vec<&'a Level>,
    min: Filter,
    mag: Filter,
    wrap: [Wrap; 2],
    border: [f32; 4],
    one_d: bool,
}

/// a level's size without the border
fn inner(l: &Level, one_d: bool) -> (u32, u32) {
    let b2 = l.border.saturating_mul(2);
    (l.width.saturating_sub(b2), if one_d { 1 } else { l.height.saturating_sub(b2) })
}

/// the levels a texture object samples, if it is complete (GL 1.1 3.8.8; texture.md 4.1)
fn complete(o: &Object, one_d: bool) -> Option<Vec<&Level>> {
    let first = o.levels.first()?.as_ref()?;
    let (w0, h0) = inner(first, one_d);
    if w0 == 0 || h0 == 0 {
        return None;
    }
    if !matches!(Filter::of(o.params.min)?, Filter::Mip { .. }) {
        return Some(vec![first]);
    }
    let mut out = vec![first];
    let (mut w, mut h) = (w0, h0);
    while w > 1 || h > 1 {
        w = (w >> 1).max(1);
        h = (h >> 1).max(1);
        let l = o.levels.get(out.len())?.as_ref()?;
        if inner(l, one_d) != (w, h) || l.ifmt != first.ifmt || l.border != first.border {
            return None;
        }
        out.push(l);
    }
    Some(out)
}

impl Bound<'_> {
    /// the texel at column i, row j of `level` (from 0 inside the border; the border at -1 and
    /// the size, or the border color without one)
    fn fetch(&self, level: &Level, i: i64, j: i64) -> [f32; 4] {
        let b = i64::from(level.border);
        let (ci, cj) = (i.saturating_add(b), if self.one_d { 0 } else { j.saturating_add(b) });
        let got = u32::try_from(ci).ok().zip(u32::try_from(cj).ok()).filter(|(x, y)| *x < level.width && *y < level.height).and_then(|(x, y)| level.texel(x, y));
        got.unwrap_or(self.border)
    }

    /// NEAREST in a level (texture.md 4.3)
    fn nearest(&self, level: &Level, s: f32, t: f32) -> [f32; 4] {
        let (w2, h2) = inner(level, self.one_d);
        let i = index(s, w2, self.wrap[0]);
        let j = if self.one_d { 0 } else { index(t, h2, self.wrap[1]) };
        self.fetch(level, i, j)
    }

    /// LINEAR in a level (texture.md 4.4)
    fn linear(&self, level: &Level, s: f32, t: f32) -> [f32; 4] {
        let (w2, h2) = inner(level, self.one_d);
        let (i0, i1, a) = pair(s, w2, self.wrap[0]);
        let (j0, j1, b) = if self.one_d { (0, 0, 0.0) } else { pair(t, h2, self.wrap[1]) };
        let (t00, t10, t01, t11) = (self.fetch(level, i0, j0), self.fetch(level, i1, j0), self.fetch(level, i0, j1), self.fetch(level, i1, j1));
        let (w00, w10, w01, w11) = ((1.0 - b) * (1.0 - a), (1.0 - b) * a, (1.0 - a) * b, b * a);
        std::array::from_fn(|k| {
            let at = |v: [f32; 4]| v.get(k).copied().unwrap_or(0.0);
            ((w10 * at(t10) + w00 * at(t00)) + w01 * at(t01)) + w11 * at(t11)
        })
    }

    fn in_level(&self, level: usize, linear: bool, s: f32, t: f32) -> [f32; 4] {
        let Some(l) = self.levels.get(level).or(self.levels.last()).copied() else { return self.border };
        if linear { self.linear(l, s, t) } else { self.nearest(l, s, t) }
    }

    /// the texel color at (s, t), rho squared `rho2` (texture.md 4.2, 4.6)
    fn sample(&self, s: f32, t: f32, rho2: Option<f32>) -> [f32; 4] {
        hit(clause::GL_RASTER_TEXTURE_FILTER);
        let mag_linear = self.mag == Filter::Linear;
        let Some(rho2) = rho2 else { return self.in_level(0, mag_linear, s, t) };
        let c = if mag_linear && matches!(self.min, Filter::Mip { between: false, .. }) { 2.0 } else { 1.0 };
        if rho2 <= c {
            return self.in_level(0, mag_linear, s, t);
        }
        match self.min {
            Filter::Nearest => self.in_level(0, false, s, t),
            Filter::Linear => self.in_level(0, true, s, t),
            Filter::Mip { linear, between } => {
                let lambda = log2_half(rho2);
                let p = self.levels.len().saturating_sub(1);
                if !between {
                    let d = if lambda <= 0.5 { 0 } else { usize::try_from(tri::trunc(lambda + f32::from_bits(0x3EFF_F972))).unwrap_or(0).min(p) };
                    return self.in_level(d, linear, s, t);
                }
                let hi = tri::trunc(lambda).saturating_add(1);
                match usize::try_from(hi).ok().filter(|h| *h <= p) {
                    None => self.in_level(p, linear, s, t),
                    Some(hi) => {
                        let f = lambda - lambda.floor();
                        let (a, b) = (self.in_level(hi.saturating_sub(1), linear, s, t), self.in_level(hi, linear, s, t));
                        std::array::from_fn(|k| a.get(k).copied().unwrap_or(0.0) * (1.0 - f) + b.get(k).copied().unwrap_or(0.0) * f)
                    }
                }
            }
        }
    }
}

/// 2^n - 1 of a size (a power of two)
fn mask(n: u32) -> i64 {
    i64::from(n.saturating_sub(1))
}

/// NEAREST's index of s in a level `n` texels wide (texture.md 4.3)
pub fn index(s: f32, n: u32, wrap: Wrap) -> i64 {
    let nf = tri::float(i64::from(n));
    match wrap {
        Wrap::Repeat if s >= 0.0 => tri::trunc(s * nf) & mask(n),
        Wrap::Repeat => mask(n).saturating_sub(tri::trunc(-s * nf) & mask(n)),
        Wrap::Clamp if s >= 1.0 => mask(n),
        Wrap::Clamp if s <= 0.0 => 0,
        Wrap::Clamp => tri::trunc(s * nf),
    }
}

/// LINEAR's two indices of s in a level `n` texels wide and the weight of the second
/// (texture.md 4.4)
pub fn pair(s: f32, n: u32, wrap: Wrap) -> (i64, i64, f32) {
    let nf = tri::float(i64::from(n));
    let s = match wrap {
        Wrap::Clamp => s.clamp(0.0, 1.0),
        Wrap::Repeat => s,
    };
    let u = s * nf - 0.5;
    let i0 = tri::trunc(u.floor());
    let a = u - tri::float(i0);
    match wrap {
        Wrap::Repeat => (i0 & mask(n), i0.saturating_add(1) & mask(n), a),
        Wrap::Clamp => (i0, i0.saturating_add(1), a),
    }
}

/// lambda from rho squared: half a piecewise-linear log2 (texture.md 4.6)
pub fn log2_half(rho2: f32) -> f32 {
    if rho2.to_bits() == 0 {
        return 0.0;
    }
    let bits = rho2.to_bits();
    let e = tri::float(i64::from((bits >> 23) & 0xFF)) - 127.0;
    let m = f32::from_bits((bits & 0x007F_FFFF) | 0x3F80_0000);
    0.5 * (e + (m - 1.0))
}

/// rho squared at a pixel from its homogeneous s, t, q and their changes (texture.md 4.5),
/// `w0`, `h0` level 0's size
fn rho2(c: &Stq, w0: f32, h0: f32) -> f32 {
    let [s, t, q] = c.at;
    if q.to_bits() & 0x7FFF_FFFF == 0 {
        return 0.0;
    }
    let ([dsdx, dtdx, dqdx], [dsdy, dtdy, dqdy]) = (c.dx, c.dy);
    let (a, b) = (1.0 / (q - dqdx), 1.0 / (q + dqdx));
    let dsx = ((s + dsdx) * b - (s - dsdx) * a) * w0;
    let dtx = ((t + dtdx) * b - (t - dtdx) * a) * h0;
    let rx = dtx * dtx + dsx * dsx;
    let (c1, d) = (1.0 / (q + dqdy), 1.0 / (q - dqdy));
    let dsy = ((s + dsdy) * c1 - (s - dsdy) * d) * w0;
    let dty = ((t + dtdy) * c1 - (t - dtdy) * d) * h0;
    let ry = dty * dty + dsy * dsy;
    rx.max(ry) * 0.25
}

/// The texture environment on a fragment color `c` in channel units, texel `t` of a level of
/// base `base` in [0, 1], the environment color `cc` in channel units, the channels' scales
/// `scale` (texture.md 5.2)
pub fn env(mode: Env, base: Base, c: [f32; 4], t: [f32; 4], cc: [f32; 4], scale: [f32; 4]) -> [f32; 4] {
    let [sr, sg, sb, sa] = scale;
    let [r, g, b, a] = c;
    let [tr, tg, tb, ta] = t;
    let mix = |x: f32, k: f32, xc: f32| (1.0 - k) * x + k * xc;
    let [cr, cg, cb, ca] = cc;
    match (mode, base) {
        (Env::Modulate | Env::Blend, Base::Alpha) => [r, g, b, a * ta],
        (Env::Modulate, Base::Luminance) => [r * tr, g * tr, b * tr, a],
        (Env::Modulate, Base::LuminanceAlpha) => [r * tr, g * tr, b * tr, a * ta],
        (Env::Modulate, Base::Intensity) => [r * tr, g * tr, b * tr, a * tr],
        (Env::Modulate, Base::Rgb) => [r * tr, g * tg, b * tb, a],
        (Env::Modulate, Base::Rgba) => [r * tr, g * tg, b * tb, a * ta],
        (Env::Replace, Base::Alpha) => [r, g, b, sa * ta],
        (Env::Replace, Base::Luminance) => [sr * tr, sg * tr, sb * tr, a],
        (Env::Replace, Base::LuminanceAlpha) => [sr * tr, sg * tr, sb * tr, sa * ta],
        (Env::Replace, Base::Intensity) => [sr * tr, sg * tr, sb * tr, sa * tr],
        (Env::Replace | Env::Decal, Base::Rgb) => [sr * tr, sg * tg, sb * tb, a],
        (Env::Replace, Base::Rgba) => [sr * tr, sg * tg, sb * tb, sa * ta],
        (Env::Decal, Base::Alpha | Base::Luminance | Base::LuminanceAlpha | Base::Intensity) => c,
        (Env::Decal, Base::Rgba) => {
            let d = |tc: f32, x: f32, s: f32| (ta * tc) * s + (1.0 - ta) * x;
            [d(tr, r, sr), d(tg, g, sg), d(tb, b, sb), a]
        }
        (Env::Blend, Base::Luminance) => [mix(r, tr, cr), mix(g, tr, cg), mix(b, tr, cb), a],
        (Env::Blend, Base::LuminanceAlpha) => [mix(r, tr, cr), mix(g, tr, cg), mix(b, tr, cb), a * ta],
        (Env::Blend, Base::Intensity) => [mix(r, tr, cr), mix(g, tr, cg), mix(b, tr, cb), mix(a, tr, ca)],
        (Env::Blend, Base::Rgb) => [mix(r, tr, cr), mix(g, tg, cg), mix(b, tb, cb), a],
        (Env::Blend, Base::Rgba) => [mix(r, tr, cr), mix(g, tg, cg), mix(b, tb, cb), a * ta],
    }
}

impl Ctx {
    /// the texture fragments read now: GL_TEXTURE_2D's before GL_TEXTURE_1D's, enabled and
    /// complete (an incomplete one leaves fragments untextured, texture.md 4.1)
    fn bound_texture(&self) -> Option<Bound<'_>> {
        let (t, one_d) = if self.cap(TEXTURE_2D) {
            (Target::Two, false)
        } else if self.cap(TEXTURE_1D) {
            (Target::One, true)
        } else {
            return None;
        };
        let o = self.tex.object(t);
        let levels = complete(o, one_d)?;
        Some(Bound {
            levels,
            min: Filter::of(o.params.min)?,
            mag: Filter::of(o.params.mag)?,
            wrap: [Wrap::of(o.params.wrap_s)?, Wrap::of(o.params.wrap_t)?],
            border: o.params.border,
            one_d,
        })
    }

    /// the fast routine of a textured polygon, if one is taken (texture.md 7.1); a fast one
    /// blended, or into alpha bits, the open clause GL.RASTER.MODE
    fn fast_routine(&self, b: &Bound<'_>, mode: Env) -> Res<Option<Fast>> {
        let Some(first) = b.levels.first() else { return Ok(None) };
        let rgba_decal = mode == Env::Decal && first.base == Base::Rgba;
        let generic = self.cap(ALPHA_TEST)
            || self.cap(STENCIL_TEST)
            || self.cap(COLOR_LOGIC_OP)
            || self.cap(POLYGON_STIPPLE)
            || self.color_mask != [true; 4]
            || self.bits().iter().take(3).any(|v| *v < 8)
            || (self.cap(DEPTH_TEST) && !self.depth_mask)
            || mode == Env::Blend
            || rgba_decal
            || b.min != Filter::Nearest
            || b.mag != Filter::Nearest
            || b.wrap != [Wrap::Repeat; 2]
            || first.border != 0
            || first.store != Store::Bytes
            || (self.cap(BLENDING) && self.blend != (SRC_ALPHA, ONE_MINUS_SRC_ALPHA))
            || self.buffers.first() == Some(&FRONT_AND_BACK);
        if generic {
            return Ok(None);
        }
        let tested = self.pf.depth > 0 && self.cap(DEPTH_TEST);
        let depth_ok = !tested || ((self.depth_func == LESS || self.depth_func == LEQUAL) && self.pf.depth <= 16);
        if !depth_ok && self.hints.first() == Some(&NICEST) {
            return Ok(None);
        }
        // (depth tested (the 32-bit span of a deep buffer writes byte 3 as 0), blended or into
        // alpha bits: to be seen in tex-open-more-1 .. 5)
        if tested || self.cap(BLENDING) || self.pf.rgba.get(3).is_some_and(|a| a.0 > 0) {
            return Err(Fail::Unmodeled(open::GL_RASTER_MODE));
        }
        Ok(Some(match mode {
            Env::Modulate => Fast::Modulate { smooth: self.shade != FLAT },
            // (dithered: (255 t) >> 8 by texture.md 7.1, to be seen in tex-open-more-0)
            Env::Replace | Env::Decal if self.cap(DITHER) => return Err(Fail::Unmodeled(open::GL_RASTER_MODE)),
            Env::Replace | Env::Decal => Fast::Copy,
            Env::Blend => return Ok(None),
        }))
    }

    /// whether fragments are textured now
    pub(super) fn texturing(&self) -> bool {
        self.bound_texture().is_some()
    }

    /// what is drawn under a texture: filled polygons; any other primitive, coordinate
    /// generation, fog, antialiased or outlined polygons the open clause GL.RASTER.MODE
    pub(super) fn textured_mode(&self, mode: u32) -> Res<()> {
        let polygons = matches!(mode, TRIANGLES | TRIANGLE_STRIP | TRIANGLE_FAN | QUADS | QUAD_STRIP | POLYGON_MODE_POLYGON);
        if self.pf.ty != 0 || (polygons && self.poly_mode != [POLY_FILL, POLY_FILL]) {
            return Err(Fail::Unmodeled(open::GL_RASTER_MODE));
        }
        Ok(())
    }

    /// the texture coordinates of a vertex at object coordinates `xyz`, `w` given now: the
    /// current ones, those of each enabled coordinate generated (GL 1.1 2.10.4; texture.md 6.2)
    pub(super) fn vertex_coords(&self, xyz: [f64; 3], w: f64) -> [f32; 4] {
        let [x, y, z] = xyz;
        let o = [x, y, z, w].map(tri::narrow);
        let mv = self.stacks.first().map_or(&super::IDENTITY, |s| s.top());
        let row = |m: &[f32; 16], v: [f32; 4], k: usize| {
            let e = |i: usize| m.get(i.saturating_mul(4).saturating_add(k)).copied().unwrap_or(0.0);
            let [x, y, z, w] = v;
            ((e(0) * x + e(1) * y) + e(2) * z) + e(3) * w
        };
        let eye: [f32; 4] = std::array::from_fn(|k| row(mv, o, k));
        let dot = |p: [f32; 4], v: [f32; 4]| {
            let ([a, b, c, d], [x, y, z, w]) = (p, v);
            ((a * x + b * y) + c * z) + d * w
        };
        let mut tc = self.tex.coord;
        let sphere = self.sphere_coords(eye);
        for (k, (cap, g)) in GENS.iter().zip(self.tex.gens.iter()).enumerate() {
            if !self.cap(*cap) {
                continue;
            }
            let v = match g.mode {
                OBJECT_LINEAR => dot(g.object, o),
                EYE_LINEAR => dot(g.eye, eye),
                _ => sphere.get(k).copied().unwrap_or(0.0),
            };
            if let Some(c) = tc.get_mut(k) {
                *c = v;
            }
        }
        tc
    }

    /// GL_SPHERE_MAP's s and t at a vertex at eye coordinates `eye` (GL 1.1 2.10.4), the
    /// current normal in eye coordinates
    fn sphere_coords(&self, eye: [f32; 4]) -> [f32; 2] {
        let [x, y, z, _] = eye;
        let len = ((x * x + y * y) + z * z).sqrt();
        let u = if len > 0.0 { [x / len, y / len, z / len] } else { [0.0; 3] };
        let n = self.normal;
        let [nx, ny, nz] = n;
        let [ux, uy, uz] = u;
        let d = (nx * ux + ny * uy) + nz * uz;
        let r = [ux - 2.0 * nx * d, uy - 2.0 * ny * d, uz - 2.0 * nz * d];
        let [rx, ry, rz] = r;
        let m = 2.0 * ((rx * rx + ry * ry) + (rz + 1.0) * (rz + 1.0)).sqrt();
        [rx / m + 0.5, ry / m + 0.5]
    }

    /// the texture coordinates of a vertex through the texture matrix, as (s, t, q)
    pub(super) fn tex_coords(&self, v: &Obj) -> [f32; 3] {
        let m = self.stacks.get(2).map_or(&super::IDENTITY, |s| s.top());
        let [s, t, r, q] = v.tc;
        let row = |k: usize| {
            let e = |i: usize| m.get(i.saturating_mul(4).saturating_add(k)).copied().unwrap_or(0.0);
            ((e(0) * s + e(1) * t) + e(2) * r) + e(3) * q
        };
        [row(0), row(1), row(3)]
    }

    /// the texture coordinates along a line from `a` to `b` (window positions `wa`, `wb`)
    /// whose first pixel is at `first` along its major axis, as its colors step
    /// (`tri::line_color`): there the first end's, each next pixel the change over the major
    /// length in the window more (MEASURED, golden opengl-32 / 64 tex-open-prim-line,
    /// line-smooth: an antialiased line's column as the aliased line's)
    pub(super) fn line_tex(&self, a: &Obj, b: &Obj, wa: [f64; 3], wb: [f64; 3], first: Option<i64>) -> LineTex {
        let [vx, vy, vw, vh] = self.viewport.map(|v| f64::from(tri::float(v)));
        let center = self.viewport_center();
        let snap = |p: [f64; 3]| {
            let [x, y, _] = p;
            [tri::Sub::snap((x - vx) * 2.0 / vw - 1.0, vw, center.0), tri::Sub::snap((y - vy) * 2.0 / vh - 1.0, vh, center.1)]
        };
        let ([ax, ay], [bx, by]) = (snap(wa), snap(wb));
        let y_major = (by.0.saturating_sub(ay.0)).abs() >= (bx.0.saturating_sub(ax.0)).abs();
        let (from, run) = if y_major { (wa[1], wb[1] - wa[1]) } else { (wa[0], wb[0] - wa[0]) };
        let per = 1.0 / tri::narrow(run.abs());
        let four = |v: [f32; 3]| -> [f32; 4] {
            let [s, t, q] = v;
            [s, t, q, 0.0]
        };
        let (c0, c1) = (four(self.tex_coords(a)), four(self.tex_coords(b)));
        let step: [f32; 4] = std::array::from_fn(|k| (c1.get(k).copied().unwrap_or(0.0) - c0.get(k).copied().unwrap_or(0.0)) * per);
        let first = first.unwrap_or_else(|| tri::trunc(tri::narrow(from.floor())));
        LineTex { c0, step, first, sign: if run < 0.0 { -1 } else { 1 }, y_major }
    }

    /// A point's, line's or image's fragment `f` textured at `stq` (GL 1.1 3.8): its color in
    /// channel units, the texel sampled at (s / q, t / q), the environment applied
    pub(super) fn texture_frag(&self, f: Frag, stq: Stq) -> Res<Frag> {
        hit(clause::GL_RASTER_TEXTURE_PRIM);
        let mode = Env::of(self.tex.env_mode).ok_or(Fail::Bad)?;
        let Some(bound) = self.bound_texture() else { return Err(Fail::Bad) };
        let first = bound.levels.first().copied().ok_or(Fail::Bad)?;
        let scale = self.channel_scales();
        let units: [f32; 4] = match f.fixed {
            ToFixed::Scaled => f.c,
            ToFixed::Nearest | ToFixed::Trunc | ToFixed::Span | ToFixed::Floor | ToFixed::Wu { .. } => {
                std::array::from_fn(|k| f.c.get(k).copied().unwrap_or(0.0).clamp(0.0, 1.0) * scale.get(k).copied().unwrap_or(S))
            }
        };
        let cc: [f32; 4] = std::array::from_fn(|k| self.tex.env_color.get(k).copied().unwrap_or(0.0) * (scale.get(k).copied().unwrap_or(S) / S));
        let (w0, h0) = inner(first, bound.one_d);
        let rho_used = bound.min != bound.mag || matches!(bound.min, Filter::Mip { .. });
        let qv = stq.at[2];
        let (s, t, rho) = if qv.to_bits() & 0x7FFF_FFFF == 0 {
            (0.0, 0.0, 0.0)
        } else {
            let rcp = 1.0 / qv;
            (stq.at[0] * rcp, stq.at[1] * rcp, rho2(&stq, tri::float(i64::from(w0)), tri::float(i64::from(h0))))
        };
        let texel = bound.sample(s, t, rho_used.then_some(rho));
        let c = env(mode, first.base, units, texel, cc, scale);
        Ok(Frag { c, fixed: ToFixed::Scaled, ..f })
    }

    /// A filled, textured polygon (GL 1.1 3.8; texture.md 4.7 the generic path): the colors
    /// and (s / w, t / w, q / w) planes of `fill`, each pixel's coordinates s / q, t / q
    /// sampled and the environment applied, in channel units.
    pub(super) fn textured_polygon(&mut self, q: &[(Obj, [f32; 4])], colors: &[[f64; 4]], fog: Option<&[[f64; 4]]>, depth: Option<tri::DepthBits>, z0: f64, zfixed: Option<FixedZ>) -> Res<()> {
        hit(clause::GL_RASTER_TEXTURE);
        let mode = Env::of(self.tex.env_mode).ok_or(Fail::Bad)?;
        let cc = self.tex.env_color;
        let scale = self.channel_scales();
        let units: Vec<[f64; 4]> = colors.iter().map(|c| std::array::from_fn(|k| c.get(k).copied().unwrap_or(0.0) * f64::from(scale.get(k).copied().unwrap_or(S)))).collect();
        // (the environment color kept in 8-bit channel units: in the buffer's)
        let cc: [f32; 4] = std::array::from_fn(|k| cc.get(k).copied().unwrap_or(0.0) * (scale.get(k).copied().unwrap_or(S) / S));
        let stq: Vec<[f32; 3]> = q.iter().map(|v| self.tex_coords(&v.0)).collect();
        let px = self.fill_with(q, &units, Some(&stq), depth)?;
        let Some(bound) = self.bound_texture() else { return Err(Fail::Bad) };
        let first = bound.levels.first().copied().ok_or(Fail::Bad)?;
        let (w0, h0) = inner(first, bound.one_d);
        let base = first.base;
        let rho_used = bound.min != bound.mag || matches!(bound.min, Filter::Mip { .. });
        // (fogged after the environment, each pixel's fog factor interpolated from the
        // corners' as their colors: GL.FOG.APPLY)
        let factors: Option<Vec<f32>> = match fog {
            Some(f) => Some(self.fill(q, f, depth)?.into_iter().map(|(_, _, c, _)| tri::narrow(c.first().copied().unwrap_or(1.0))).collect()),
            None => None,
        };
        let fog_color: [f32; 4] = std::array::from_fn(|k| self.fog.color.get(k).copied().unwrap_or(0.0) * scale.get(k).copied().unwrap_or(S));
        let fast = if factors.is_some() { None } else { self.fast_routine(&bound, mode)? };
        let mut frags = Vec::with_capacity(px.len());
        for (n, ((x, y, c, zf), stq)) in px.into_iter().enumerate() {
            if let Some(fast) = fast {
                hit(clause::GL_RASTER_TEXTURE_FAST);
                let texel = bound.nearest(first, stq.at[0], stq.at[1]).map(|v| tri::trunc(v.mul_add(S, 0.5)));
                let color = c.map(|v| tri::trunc(tri::narrow(v) * 65536.0));
                let byte = |v: i64| (v >> 16) & 0xFF;
                let m = |c: i64, t: i64| c.saturating_mul(t) >> 8;
                let out: [i64; 4] = std::array::from_fn(|k| {
                    let (c, t) = (color.get(k).copied().unwrap_or(0), texel.get(k).copied().unwrap_or(0));
                    match fast {
                        Fast::Modulate { smooth: true } => m(byte(c.saturating_add(0x80)), t),
                        Fast::Modulate { smooth: false } => m(byte(c), t),
                        Fast::Copy => t,
                    }
                });
                frags.push(Frag { x, y, c: out.map(tri::float), z: z0, fixed: ToFixed::Scaled, zfixed: zf.or(zfixed) });
                continue;
            }
            let qv = stq.at[2];
            let (s, t, rho) = if qv.to_bits() & 0x7FFF_FFFF == 0 {
                (0.0, 0.0, 0.0)
            } else {
                let rcp = 1.0 / qv;
                (stq.at[0] * rcp, stq.at[1] * rcp, rho2(&stq, tri::float(i64::from(w0)), tri::float(i64::from(h0))))
            };
            let texel = bound.sample(s, t, rho_used.then_some(rho));
            let c = env(mode, base, c.map(tri::narrow), texel, cc, scale);
            let c = match factors.as_ref().and_then(|f| f.get(n)) {
                Some(f) => {
                    let [r, g, b, a] = c;
                    let [kr, kg, kb, _] = fog_color;
                    [f * r + (1.0 - f) * kr, f * g + (1.0 - f) * kg, f * b + (1.0 - f) * kb, a]
                }
                None => c,
            };
            frags.push(Frag { x, y, c, z: z0, fixed: ToFixed::Scaled, zfixed: zf.or(zfixed) });
        }
        self.textured_frags = true;
        let r = frags.into_iter().try_for_each(|f| self.fragment(f));
        self.textured_frags = false;
        r
    }
}
