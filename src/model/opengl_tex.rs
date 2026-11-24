//! Texturing of the GDI generic OpenGL 1.1 (GL 1.1 3.8): texture objects and their names,
//! parameters, images and the queries of them, the environment and coordinate generation.
//!
//! The rules come from clean-room notes of opengl32.dll (behaviour only, no code) and each is
//! confirmed by the goldens of the tex-open-* cases (spec/opengl.txt GL.TEX.*).

use std::collections::{BTreeMap, BTreeSet};

use super::e::{BITMAP, COLOR_INDEX, INVALID_ENUM, INVALID_OPERATION, INVALID_VALUE};
use super::{Ctx, Fail, Given, Res, A, V};
use crate::case::num;
use crate::spec::{clause, hit};

pub const TEXTURE_1D: u32 = 0xde0;
pub const TEXTURE_2D: u32 = 0xde1;
const PROXY_1D: u32 = 0x8063;
pub(super) const PROXY_2D: u32 = 0x8064;
const NEAREST: u32 = 0x2600;
const LINEAR: u32 = 0x2601;
const NEAREST_MIPMAP_NEAREST: u32 = 0x2700;
const LINEAR_MIPMAP_LINEAR: u32 = 0x2703;
const CLAMP: u32 = 0x2900;
const REPEAT: u32 = 0x2901;
const MAG_FILTER: u32 = 0x2800;
const MIN_FILTER: u32 = 0x2801;
const WRAP_S: u32 = 0x2802;
const WRAP_T: u32 = 0x2803;
const BORDER_COLOR: u32 = 0x1004;
const PRIORITY: u32 = 0x8066;
const RESIDENT: u32 = 0x8067;
const TEXTURE_ENV: u32 = 0x2300;
const ENV_MODE: u32 = 0x2200;
const ENV_COLOR: u32 = 0x2201;
const MODULATE: u32 = 0x2100;
const DECAL: u32 = 0x2101;
const BLEND: u32 = 0x0be2;
const REPLACE: u32 = 0x1e01;
const GEN_MODE: u32 = 0x2500;
const OBJECT_PLANE: u32 = 0x2501;
const EYE_PLANE: u32 = 0x2502;
const EYE_LINEAR: u32 = 0x2400;
const OBJECT_LINEAR: u32 = 0x2401;
const SPHERE_MAP: u32 = 0x2402;
const GEN_S: u32 = 0x2000;
/// levels 0 .. 10 (a 1024 texel side)
pub const LEVELS: usize = 11;
const MAX_SIZE: u32 = 1024;
/// f32(1/255), the step back from channel units
const INV255: f32 = 1.0 / 255.0;

/// A GL error a call raises.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bad {
    Enum,
    Value,
    Operation,
}

impl Bad {
    pub fn code(self) -> u32 {
        match self {
            Bad::Enum => INVALID_ENUM,
            Bad::Value => INVALID_VALUE,
            Bad::Operation => INVALID_OPERATION,
        }
    }
}

/// A texture target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    One,
    Two,
}

impl Target {
    /// the target a proxy target stands for
    pub fn proxy(name: u32) -> Option<Target> {
        match name {
            PROXY_1D => Some(Target::One),
            PROXY_2D => Some(Target::Two),
            _ => None,
        }
    }

    pub fn of(name: u32) -> Option<Target> {
        match name {
            TEXTURE_1D => Some(Target::One),
            TEXTURE_2D => Some(Target::Two),
            _ => None,
        }
    }
}

/// One value per target.
#[derive(Clone, Debug)]
pub struct PerTarget<T> {
    one: T,
    two: T,
}

impl<T> PerTarget<T> {
    pub fn get(&self, t: Target) -> &T {
        match t {
            Target::One => &self.one,
            Target::Two => &self.two,
        }
    }
    pub fn get_mut(&mut self, t: Target) -> &mut T {
        match t {
            Target::One => &mut self.one,
            Target::Two => &mut self.two,
        }
    }
}

/// The base format of a level (GL 1.1 table 3.15).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Alpha,
    Luminance,
    LuminanceAlpha,
    Intensity,
    Rgb,
    Rgba,
    /// GL_EXT_paletted_texture's color indices (MEASURED, golden opengl-32 / 64
    /// tex-open-more-paletted: taken from COLOR_INDEX, no component sizes, read back undefined,
    /// the texture drawing as none, GL.TEX.FORMAT)
    Index,
}

/// How a level keeps its texels: bytes of the four channels up to 8 bits a channel, floats of the
/// base's components otherwise (sized formats keep their full precision).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    Bytes,
    Floats,
}

/// The base and storage of a requested internal format (MEASURED, golden opengl-32 / 64
/// tex-open-format-*: GL.TEX.FORMAT).
pub fn internal(ifmt: u32) -> Option<(Base, Store)> {
    use Base::*;
    use Store::*;
    hit(clause::GL_TEX_FORMAT);
    Some(match ifmt {
        1 | 0x1909 | 0x803f..=0x8042 => (Luminance, Floats),
        2 | 0x190a | 0x8043..=0x8048 => (LuminanceAlpha, Floats),
        0x1906 | 0x803b..=0x803e => (Alpha, Floats),
        0x8049..=0x804d => (Intensity, Floats),
        3 | 0x1907 | 0x2a10 | 0x804f..=0x8051 => (Rgb, Bytes),
        0x8052..=0x8054 => (Rgb, Floats),
        4 | 0x1908 | 0x8055..=0x8058 => (Rgba, Bytes),
        0x8059..=0x805b => (Rgba, Floats),
        _ => return None,
    })
}

/// The color index internal formats of GL_EXT_paletted_texture.
fn paletted(ifmt: u32) -> bool {
    (0x80e2..=0x80e7).contains(&ifmt)
}

/// One mipmap level's image.
#[derive(Clone, Debug)]
pub struct Level {
    /// width and height with the border
    pub width: u32,
    pub height: u32,
    pub border: u32,
    /// the internal format as it was asked for
    pub ifmt: u32,
    pub base: Base,
    pub store: Store,
    /// rows from the bottom, each `width` texels: the channels in [0, 1], of bytes as k / 255
    pub texels: Vec<[f32; 4]>,
}

impl Level {
    /// a texel at column `i`, row `j` (with the border)
    /// whether any texel is undefined (NaN: copied from outside the window, GL.UNDEFINED)
    pub fn undefined(&self) -> bool {
        self.texels.iter().any(|t| t.iter().any(|v| v.is_nan()))
    }

    pub fn texel(&self, i: u32, j: u32) -> Option<[f32; 4]> {
        let at = usize::try_from(j.checked_mul(self.width)?.checked_add(i)?).ok()?;
        self.texels.get(at).copied()
    }

    /// the level read back as RGBA (MEASURED, golden opengl-32 tex-open-format-*: luminance as
    /// red, intensity as red, alpha alone; bytes as they are, alpha included: GL.TEX.GETIMAGE)
    pub fn rgba(&self, t: [f32; 4]) -> [f32; 4] {
        let [r, g, b, a] = t;
        match (self.store, self.base) {
            (Store::Bytes, Base::Alpha | Base::Luminance | Base::LuminanceAlpha | Base::Intensity | Base::Rgb | Base::Rgba) => t,
            (Store::Floats, Base::Luminance | Base::Intensity) => [r, 0.0, 0.0, 1.0],
            (Store::Floats, Base::LuminanceAlpha) => [r, 0.0, 0.0, a],
            (Store::Floats, Base::Alpha) => [0.0, 0.0, 0.0, a],
            (Store::Floats, Base::Rgb) => [r, g, b, 1.0],
            (Store::Floats, Base::Rgba) => t,
            // (its texels undefined)
            (Store::Bytes | Store::Floats, Base::Index) => t,
        }
    }

    /// the component sizes R G B A L I (GL.TEX.LEVEL)
    fn sizes(&self) -> [i64; 6] {
        match (self.store, self.base) {
            (Store::Bytes, Base::Rgb) => [8, 8, 8, 0, 0, 0],
            (Store::Bytes, Base::Rgba) => [8, 8, 8, 8, 0, 0],
            (Store::Floats, Base::Rgb) => [24, 24, 24, 0, 0, 0],
            (Store::Floats, Base::Rgba) => [24, 24, 24, 24, 0, 0],
            (Store::Bytes | Store::Floats, Base::Luminance) => [0, 0, 0, 0, 24, 0],
            (Store::Bytes | Store::Floats, Base::LuminanceAlpha) => [0, 0, 0, 24, 24, 0],
            (Store::Bytes | Store::Floats, Base::Alpha) => [0, 0, 0, 24, 0, 0],
            (Store::Bytes | Store::Floats, Base::Intensity) => [0, 0, 0, 0, 0, 24],
            (Store::Bytes | Store::Floats, Base::Index) => [0; 6],
        }
    }
}

/// A channel-unit value (0 .. 255) as a level keeps it (MEASURED, golden opengl-32
/// tex-open-format-*, tex-open-source-*: floats times f32(1/255); bytes the nearest of 255
/// steps: GL.TEX.IMAGE)
pub fn kept(store: Store, units: f32) -> f32 {
    let v = units * INV255;
    match store {
        Store::Floats => v,
        Store::Bytes => {
            let b = super::tri::trunc(v.mul_add(255.0, 0.5)).clamp(0, 255);
            u8::try_from(b).map_or(0.0, |b| f32::from(b) * INV255)
        }
    }
}

/// a texel from an image pixel (RGBA in [0, 1], expanded as GL 1.1 3.6.4 says)
fn reduce(base: Base, store: Store, c: [f32; 4]) -> [f32; 4] {
    // (an undefined texel stays so: GL.UNDEFINED)
    if c.iter().any(|v| v.is_nan()) {
        return [f32::NAN; 4];
    }
    let [r, g, b, a] = c.map(|v| kept(store, v * 255.0));
    match (store, base) {
        (Store::Bytes, Base::Alpha | Base::Luminance | Base::LuminanceAlpha | Base::Intensity | Base::Rgb | Base::Rgba) => [r, g, b, a],
        // (luminance and intensity of an RGB image: its red, not a weighted sum)
        (Store::Floats, Base::Luminance | Base::Intensity) => [r, 0.0, 0.0, 0.0],
        (Store::Floats, Base::LuminanceAlpha) => [r, 0.0, 0.0, a],
        (Store::Floats, Base::Alpha) => [0.0, 0.0, 0.0, a],
        (Store::Floats, Base::Rgb) => [r, g, b, 0.0],
        (Store::Floats, Base::Rgba) => [r, g, b, a],
        // (indices kept as nothing GL_RGBA reads back: undefined, GL.UNDEFINED)
        (Store::Bytes | Store::Floats, Base::Index) => [f32::NAN; 4],
    }
}

/// A texture object's parameters (glTexParameter).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub min: u32,
    pub mag: u32,
    pub wrap_s: u32,
    pub wrap_t: u32,
    pub border: [f32; 4],
    pub priority: f32,
}

impl Params {
    /// (MEASURED, golden opengl-32 tex-open-0: GL.TEX.PARAM)
    const DEFAULT: Params = Params { min: 0x2702, mag: LINEAR, wrap_s: REPEAT, wrap_t: REPEAT, border: [0.0; 4], priority: 1.0 };
}

/// A texture object.
#[derive(Clone, Debug)]
pub struct Object {
    pub target: Target,
    pub params: Params,
    /// set by a level's image (glAreTexturesResident)
    pub resident: bool,
    pub levels: Vec<Option<Level>>,
}

impl Object {
    fn new(target: Target) -> Object {
        Object { target, params: Params::DEFAULT, resident: false, levels: vec![None; LEVELS] }
    }
}

/// One coordinate's generation (glTexGen).
#[derive(Clone, Copy, Debug)]
pub struct Gen {
    pub mode: u32,
    pub object: [f32; 4],
    pub eye: [f32; 4],
}

/// The texturing state of a context.
#[derive(Clone, Debug)]
pub struct Tex {
    pub objects: BTreeMap<u32, Object>,
    /// names glGenTextures gave that are not bound yet
    pub reserved: BTreeSet<u32>,
    pub defaults: PerTarget<Object>,
    /// GL_PROXY_TEXTURE_1D / 2D: their levels' sizes and formats, no texels
    pub proxies: PerTarget<Object>,
    pub bound: PerTarget<u32>,
    pub env_mode: u32,
    /// in channel units
    pub env_color: [f32; 4],
    pub gens: [Gen; 4],
    /// the current texture coordinates
    pub coord: [f32; 4],
}

impl Tex {
    pub fn new() -> Tex {
        let plane = |k: usize| -> [f32; 4] { [0, 1, 2, 3].map(|n| if n == k { 1.0 } else { 0.0 }) };
        // (MEASURED, golden opengl-32 tex-open-gen-*: the S plane (1, 0, 0, 0), T (0, 1, 0, 0))
        let gens = [0usize, 1, 4, 4].map(|k| Gen { mode: EYE_LINEAR, object: plane(k), eye: plane(k) });
        Tex {
            objects: BTreeMap::new(),
            reserved: BTreeSet::new(),
            defaults: PerTarget { one: Object::new(Target::One), two: Object::new(Target::Two) },
            proxies: PerTarget { one: Object::new(Target::One), two: Object::new(Target::Two) },
            bound: PerTarget { one: 0, two: 0 },
            env_mode: MODULATE,
            env_color: [0.0; 4],
            gens,
            coord: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn object(&self, t: Target) -> &Object {
        let name = *self.bound.get(t);
        self.objects.get(&name).filter(|_| name != 0).unwrap_or_else(|| self.defaults.get(t))
    }

    fn object_mut(&mut self, t: Target) -> &mut Object {
        let name = *self.bound.get(t);
        match self.objects.get_mut(&name) {
            Some(o) if name != 0 => o,
            Some(_) | None => self.defaults.get_mut(t),
        }
    }

    /// glGenTextures: one run of free names, the lowest that fits, from 1 (MEASURED, golden
    /// opengl-32 tex-open-0: GL.TEX.NAMES)
    pub fn gen_names(&mut self, n: u32) -> Vec<u32> {
        hit(clause::GL_TEX_NAMES);
        let used = |k: u32| self.objects.contains_key(&k) || self.reserved.contains(&k);
        let mut first = 1u32;
        let mut k = 1u32;
        while k.wrapping_sub(first) < n {
            if used(k) {
                first = k.wrapping_add(1);
            }
            k = k.wrapping_add(1);
        }
        let names: Vec<u32> = (0..n).map(|i| first.wrapping_add(i)).collect();
        self.reserved.extend(names.iter().copied());
        names
    }

    /// glDeleteTextures: zeros and unknown names skipped, a bound one rebinding the default
    pub fn delete(&mut self, names: &[u32]) {
        hit(clause::GL_TEX_NAMES);
        for name in names {
            if *name == 0 {
                continue;
            }
            self.reserved.remove(name);
            if let Some(o) = self.objects.remove(name)
                && *self.bound.get(o.target) == *name
            {
                *self.bound.get_mut(o.target) = 0;
            }
        }
    }

    /// glIsTexture: true once the name was bound
    pub fn is_texture(&self, name: u32) -> bool {
        hit(clause::GL_TEX_NAMES);
        name != 0 && self.objects.contains_key(&name)
    }

    /// glBindTexture
    pub fn bind(&mut self, target: u32, name: u32) -> Result<(), Bad> {
        let t = Target::of(target).ok_or(Bad::Enum)?;
        hit(clause::GL_TEX_BIND);
        if name != 0 {
            match self.objects.get(&name) {
                Some(o) if o.target != t => return Err(Bad::Operation),
                Some(_) => {}
                None => {
                    self.reserved.remove(&name);
                    self.objects.insert(name, Object::new(t));
                }
            }
        }
        *self.bound.get_mut(t) = name;
        Ok(())
    }

    /// glAreTexturesResident: each flag written up to the first name of no object, which is
    /// INVALID_VALUE (MEASURED, golden opengl-32 tex-open-0: GL.TEX.RESIDENT)
    pub fn resident(&self, names: &[u32]) -> (bool, Vec<bool>, Option<Bad>) {
        hit(clause::GL_TEX_RESIDENT);
        let mut flags = Vec::new();
        for name in names {
            match self.objects.get(name).filter(|_| *name != 0) {
                Some(o) => flags.push(o.resident),
                None => return (false, flags, Some(Bad::Value)),
            }
        }
        (!names.is_empty() && flags.iter().all(|f| *f), flags, None)
    }

    /// glPrioritizeTextures: clamped to [0, 1], names of no object skipped
    pub fn prioritize(&mut self, names: &[u32], p: f32) {
        hit(clause::GL_TEX_RESIDENT);
        for name in names {
            if let Some(o) = self.objects.get_mut(name).filter(|_| *name != 0) {
                o.params.priority = if p.is_nan() { p } else { p.clamp(0.0, 1.0) };
            }
        }
    }

    /// glTexParameter: `v` the values as floats, `int` for the integer entry points
    pub fn set_param(&mut self, target: u32, pname: u32, v: &[f64], int: bool) -> Result<(), Bad> {
        let t = Target::of(target).ok_or(Bad::Enum)?;
        hit(clause::GL_TEX_PARAM);
        let first = v.first().copied().ok_or(Bad::Enum)?;
        let enumerant = || u32::try_from(super::float_to_int(super::tri::narrow(first))).ok();
        let p = &mut self.object_mut(t).params;
        match pname {
            MAG_FILTER => p.mag = enumerant().filter(|e| matches!(*e, NEAREST | LINEAR)).ok_or(Bad::Enum)?,
            MIN_FILTER => p.min = enumerant().filter(|e| matches!(*e, NEAREST | LINEAR | NEAREST_MIPMAP_NEAREST..=LINEAR_MIPMAP_LINEAR)).ok_or(Bad::Enum)?,
            WRAP_S => p.wrap_s = enumerant().filter(|e| matches!(*e, CLAMP | REPEAT)).ok_or(Bad::Enum)?,
            WRAP_T => p.wrap_t = enumerant().filter(|e| matches!(*e, CLAMP | REPEAT)).ok_or(Bad::Enum)?,
            BORDER_COLOR if v.len() == 4 && !int => {
                for (b, x) in p.border.iter_mut().zip(v) {
                    *b = super::tri::narrow(*x).clamp(0.0, 1.0);
                }
            }
            PRIORITY if !int => p.priority = super::tri::narrow(first).clamp(0.0, 1.0),
            _ => return Err(Bad::Enum),
        }
        Ok(())
    }

    /// glGetTexParameter (MEASURED, golden opengl-32 tex-open-0: GL.TEX.PARAM)
    pub fn param(&self, target: u32, pname: u32) -> Result<Vec<V>, Bad> {
        let t = Target::of(target).ok_or(Bad::Enum)?;
        hit(clause::GL_TEX_PARAM);
        let o = self.object(t);
        let p = &o.params;
        let e = |x: u32| V::I(i64::from(x));
        Ok(match pname {
            MAG_FILTER => vec![e(p.mag)],
            MIN_FILTER => vec![e(p.min)],
            WRAP_S => vec![e(p.wrap_s)],
            WRAP_T => vec![e(p.wrap_t)],
            BORDER_COLOR => p.border.iter().map(|c| V::C(*c)).collect(),
            PRIORITY => vec![V::C(p.priority)],
            RESIDENT => vec![V::I(i64::from(o.resident))],
            _ => return Err(Bad::Enum),
        })
    }

    /// glGetTexLevelParameter (MEASURED, golden opengl-32 tex-open-format-*, tex-open-state: an
    /// empty level 0 wide with 1 component; a 1D level 1 high: GL.TEX.LEVEL)
    pub fn level_param(&self, target: u32, level: i64, pname: u32) -> Result<Vec<V>, Bad> {
        let (t, o) = match (Target::of(target), Target::proxy(target)) {
            (Some(t), _) => (t, self.object(t)),
            (None, Some(t)) => (t, self.proxies.get(t)),
            (None, None) => return Err(Bad::Enum),
        };
        let l = usize::try_from(level).ok().filter(|l| *l < LEVELS).ok_or(Bad::Value)?;
        hit(clause::GL_TEX_LEVEL);
        let lv = o.levels.get(l).and_then(Option::as_ref);
        let i = |x: u32| vec![V::I(i64::from(x))];
        let size = |k: usize| vec![V::I(lv.map_or(0, |l| l.sizes().get(k).copied().unwrap_or(0)))];
        Ok(match pname {
            0x1000 => i(lv.map_or(0, |l| l.width)),
            0x1001 => i(lv.map_or(0, |l| if t == Target::One { 1 } else { l.height })),
            0x1003 => i(lv.map_or(1, |l| l.ifmt)),
            0x1005 => i(lv.map_or(0, |l| l.border)),
            0x805c => size(0),
            0x805d => size(1),
            0x805e => size(2),
            0x805f => size(3),
            0x8060 => size(4),
            0x8061 => size(5),
            _ => return Err(Bad::Enum),
        })
    }

    /// glTexEnv: the color clamped and kept in channel units (MEASURED, golden opengl-32
    /// tex-open-env-*: read back times f32(1/255), 0.75 as 0.75000006: GL.TEX.ENV)
    pub fn set_env(&mut self, target: u32, pname: u32, v: &[f64]) -> Result<(), Bad> {
        if target != TEXTURE_ENV {
            return Err(Bad::Enum);
        }
        hit(clause::GL_TEX_ENV);
        let first = v.first().copied().ok_or(Bad::Enum)?;
        match pname {
            ENV_MODE => {
                let m = u32::try_from(super::float_to_int(super::tri::narrow(first))).ok();
                self.env_mode = m.filter(|m| matches!(*m, MODULATE | DECAL | BLEND | REPLACE)).ok_or(Bad::Enum)?;
            }
            ENV_COLOR if v.len() == 4 => {
                for (c, x) in self.env_color.iter_mut().zip(v) {
                    *c = super::tri::narrow(*x).clamp(0.0, 1.0) * 255.0;
                }
            }
            _ => return Err(Bad::Enum),
        }
        Ok(())
    }

    pub fn env(&self, target: u32, pname: u32) -> Result<Vec<V>, Bad> {
        if target != TEXTURE_ENV {
            return Err(Bad::Enum);
        }
        hit(clause::GL_TEX_ENV);
        Ok(match pname {
            ENV_MODE => vec![V::I(i64::from(self.env_mode))],
            ENV_COLOR => self.env_color.iter().map(|c| V::C(*c * INV255)).collect(),
            _ => return Err(Bad::Enum),
        })
    }

    /// glTexGen: planes as given (the eye plane times the inverse modelview, here the identity's)
    /// (MEASURED, golden opengl-32 tex-open-gen-*: GL.TEX.GEN)
    pub fn set_gen(&mut self, coord: u32, pname: u32, v: &[f64]) -> Result<(), Bad> {
        let k = coord.checked_sub(GEN_S).and_then(|k| usize::try_from(k).ok()).filter(|k| *k < 4).ok_or(Bad::Enum)?;
        hit(clause::GL_TEX_GEN);
        let first = v.first().copied().ok_or(Bad::Enum)?;
        let g = self.gens.get_mut(k).ok_or(Bad::Enum)?;
        let plane = |v: &[f64]| -> [f32; 4] { [0usize, 1, 2, 3].map(|n| v.get(n).map_or(0.0, |x| super::tri::narrow(*x))) };
        match pname {
            GEN_MODE => {
                let m = u32::try_from(super::float_to_int(super::tri::narrow(first))).ok();
                g.mode = m.filter(|m| matches!(*m, EYE_LINEAR | OBJECT_LINEAR) || (*m == SPHERE_MAP && k < 2)).ok_or(Bad::Enum)?;
            }
            OBJECT_PLANE if v.len() == 4 => g.object = plane(v),
            EYE_PLANE if v.len() == 4 => g.eye = plane(v),
            _ => return Err(Bad::Enum),
        }
        Ok(())
    }

    pub fn gen_state(&self, coord: u32, pname: u32) -> Result<Vec<V>, Bad> {
        let k = coord.checked_sub(GEN_S).and_then(|k| usize::try_from(k).ok()).ok_or(Bad::Enum)?;
        let g = self.gens.get(k).ok_or(Bad::Enum)?;
        hit(clause::GL_TEX_GEN);
        Ok(match pname {
            GEN_MODE => vec![V::I(i64::from(g.mode))],
            OBJECT_PLANE => g.object.iter().map(|x| V::F(*x)).collect(),
            EYE_PLANE => g.eye.iter().map(|x| V::F(*x)).collect(),
            _ => return Err(Bad::Enum),
        })
    }
}

impl Tex {
    /// glPopAttrib of GL_TEXTURE_BIT: the coordinate generation; each target whose bound name
    /// changed bound again to the saved name; the bound textures' parameters and the environment
    /// as saved (MEASURED, golden opengl-32 tex-open-state: GL.TEX.BIND)
    /// glPopAttrib's texture state: TEXTURE_BIT's (`texture`), and the current coordinates,
    /// CURRENT_BIT's (`current`). Every field is placed, so a new one does not compile until it is
    pub fn pop(&mut self, saved: &Tex, texture: bool, current: bool) {
        // (no group: the texture objects themselves, the names reserved, the proxies' answers;
        // the bound objects' parameters are TEXTURE_BIT's, below)
        let Tex { objects: _, reserved: _, defaults: _, proxies: _, bound, env_mode, env_color, gens, coord } = saved;
        if current {
            self.coord = *coord;
        }
        if !texture {
            return;
        }
        hit(clause::GL_TEX_BIND);
        self.gens = *gens;
        for t in [Target::One, Target::Two] {
            let name = *bound.get(t);
            if *self.bound.get(t) != name {
                let target = match t {
                    Target::One => TEXTURE_1D,
                    Target::Two => TEXTURE_2D,
                };
                // (a name of the other target meanwhile: left as it is)
                let _kept = self.bind(target, name).is_ok();
            }
            self.object_mut(t).params = saved.object(t).params;
        }
        self.env_mode = *env_mode;
        self.env_color = *env_color;
    }
}

/// %.9g as C's printf writes a double
pub fn g9(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return if v.is_nan() { "nan".into() } else if v.is_infinite() { if v < 0.0 { "-inf".into() } else { "inf".into() } } else if v.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let sci = format!("{v:.8e}");
    let (mantissa, exp) = sci.split_once('e').unwrap_or((sci.as_str(), "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let strip = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    };
    if !(-4..9).contains(&exp) {
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{}e{sign}{:02}", strip(mantissa), exp.unsigned_abs());
    }
    let digits = usize::try_from(8i32.saturating_sub(exp)).unwrap_or(0);
    strip(&format!("{v:.digits$}"))
}

/// the integers of an op's tokens from `from` on
fn nums(t: &[String], from: usize) -> Res<Vec<i64>> {
    t.iter().skip(from).map(|x| num(x).ok_or(Fail::Bad)).collect()
}

fn int_at(v: &[i64], k: usize) -> Res<i64> {
    v.get(k).copied().ok_or(Fail::Bad)
}

fn enum_at(v: &[i64], k: usize) -> Res<u32> {
    // (the executor passes a C long: its low 32 bits)
    Ok(u32::try_from(int_at(v, k)? & 0xFFFF_FFFF).unwrap_or(0))
}

/// The pixels of an image source: each RGBA in [0, 1].
type Pixels = Vec<[f32; 4]>;

impl Ctx {
    /// the GL error of a call, if any
    fn tex_err(&mut self, r: Result<(), Bad>) {
        if let Err(b) = r {
            hit(clause::GL_TEX_ERRORS);
            self.error(b.code());
        }
    }

    /// whether a call inside glBegin / glEnd is refused (INVALID_OPERATION)
    fn tex_in_begin(&mut self) -> bool {
        if self.in_begin.is_some() {
            hit(clause::GL_BEGIN_ONLY);
            self.error(INVALID_OPERATION);
            return true;
        }
        false
    }

    /// An executor op of textures (runner/opengl.c gltex .. glresident).
    pub(super) fn tex_op(&mut self, t: &[String], out: &mut Vec<String>) -> Res<()> {
        let name = t.first().map(String::as_str).ok_or(Fail::Bad)?;
        match name {
            "gltex" => {
                let v = nums(t, 1)?;
                if v.len() != 10 {
                    return Err(Fail::Bad);
                }
                let given = given(int_at(&v, 8)?, int_at(&v, 9)?)?;
                let target = enum_at(&v, 0)?;
                let (level, ifmt, w, h, b, f, ty) = (int_at(&v, 1)?, enum_at(&v, 2)?, int_at(&v, 3)?, int_at(&v, 4)?, int_at(&v, 5)?, enum_at(&v, 6)?, enum_at(&v, 7)?);
                if self.tex_in_begin() {
                    return Ok(());
                }
                let one = target == TEXTURE_1D;
                let r = self.tex_image(target, one, level, ifmt, (w, h, b), Source::Image { f, ty, given: given.as_ref() })?;
                self.tex_err(r);
            }
            "gltexsub" => {
                let v = nums(t, 1)?;
                if v.len() != 10 {
                    return Err(Fail::Bad);
                }
                let given = given(int_at(&v, 8)?, int_at(&v, 9)?)?;
                let target = enum_at(&v, 0)?;
                if self.tex_in_begin() {
                    return Ok(());
                }
                let one = target == TEXTURE_1D;
                let rect = [int_at(&v, 2)?, int_at(&v, 3)?, int_at(&v, 4)?, int_at(&v, 5)?];
                let r = self.tex_sub(target, one, int_at(&v, 1)?, rect, Source::Image { f: enum_at(&v, 6)?, ty: enum_at(&v, 7)?, given: given.as_ref() })?;
                self.tex_err(r);
            }
            "glteximg" => {
                let v = nums(t, 1)?;
                if v.len() != 5 {
                    return Err(Fail::Bad);
                }
                let n = usize::try_from(int_at(&v, 4)?).ok().ok_or(Fail::Bad)?;
                let mut buf = vec![0xccu8; n.checked_add(64).ok_or(Fail::Bad)?];
                let mut undefined = vec![false; buf.len()];
                let r = self.get_tex_image(enum_at(&v, 0)?, int_at(&v, 1)?, enum_at(&v, 2)?, enum_at(&v, 3)?, &mut buf, &mut undefined)?;
                self.tex_err(r);
                let (shown, past) = buf.split_at(n);
                // (an undefined byte as "??": any byte matches it, crate::compare)
                let text: String = shown.iter().zip(&undefined).map(|(b, u)| if *u { "??".to_string() } else { format!("{b:02x}") }).collect();
                out.push(format!("read {text}"));
                if past.iter().any(|b| *b != 0xcc) {
                    out.push("over".into());
                }
            }
            "gltexq" => {
                let kind = t.get(1).map(String::as_str).ok_or(Fail::Bad)?;
                let level = kind == "level";
                let at: usize = if level { 5 } else { 4 };
                let v: Vec<i64> = t.iter().skip(2).take(at.saturating_sub(2)).map(|x| num(x).ok_or(Fail::Bad)).collect::<Res<_>>()?;
                let ty = t.get(at).map(String::as_str).ok_or(Fail::Bad)?;
                let n = t.get(at.saturating_add(1)).and_then(|x| num(x)).and_then(|n| usize::try_from(n).ok()).ok_or(Fail::Bad)?;
                let target = enum_at(&v, 0)?;
                let r = if self.in_begin.is_some() {
                    hit(clause::GL_BEGIN_ONLY);
                    Err(Bad::Operation)
                } else {
                    match kind {
                        "param" => self.tex.param(target, enum_at(&v, 1)?),
                        "level" => self.tex.level_param(target, int_at(&v, 1)?, enum_at(&v, 2)?),
                        "env" => self.tex.env(target, enum_at(&v, 1)?),
                        "gen" => self.tex.gen_state(target, enum_at(&v, 1)?),
                        _ => return Err(Fail::Bad),
                    }
                };
                let values = match r {
                    Ok(values) => values,
                    Err(b) => {
                        self.tex_err(Err(b));
                        Vec::new()
                    }
                };
                let mut line = String::from("texq");
                for k in 0..n {
                    let shown = match (values.get(k), ty) {
                        (None, "i") => "2125315823".to_string(),
                        (None, _) => "-12345".to_string(),
                        (Some(x), "i") => match x {
                            V::I(i) => i.to_string(),
                            V::C(c) => super::color_to_int(*c).to_string(),
                            V::F(f) => super::float_to_int(*f).to_string(),
                            V::B(b) => i64::from(*b).to_string(),
                        },
                        (Some(x), _) => match x {
                            V::I(i) => g9(f64::from(i32::try_from(*i).unwrap_or(0))),
                            V::C(f) | V::F(f) => g9(f64::from(*f)),
                            V::B(b) => g9(f64::from(u8::from(*b))),
                        },
                    };
                    line.push(' ');
                    line.push_str(&shown);
                }
                out.push(line);
            }
            "gltexv" => {
                let kind = t.get(1).map(String::as_str).ok_or(Fail::Bad)?;
                let head: Vec<i64> = t.iter().skip(2).take(2).map(|x| num(x).ok_or(Fail::Bad)).collect::<Res<_>>()?;
                let ty = t.get(4).map(String::as_str).ok_or(Fail::Bad)?;
                let values: Vec<f64> = t.iter().skip(5).map(|x| super::parse_float(x)).collect::<Res<_>>()?;
                let (target, pname) = (enum_at(&head, 0)?, enum_at(&head, 1)?);
                if self.tex_in_begin() {
                    return Ok(());
                }
                let r = match kind {
                    "param" => self.tex.set_param(target, pname, &values, ty == "i"),
                    "env" => self.tex.set_env(target, pname, &values),
                    "gen" => self.tex_gen(target, pname, &values)?,
                    _ => return Err(Fail::Bad),
                };
                self.tex_err(r);
            }
            "glgentex" => {
                let v = nums(t, 1)?;
                let n = u32::try_from(int_at(&v, 0)?).ok().ok_or(Fail::Bad)?;
                let names = if self.tex_in_begin() { Vec::new() } else { self.tex.gen_names(n) };
                let shown: Vec<String> = (0..n).map(|k| names.get(usize::try_from(k).unwrap_or(usize::MAX)).copied().unwrap_or(0).to_string()).collect();
                out.push(format!("tex {}", shown.join(" ")).trim_end().to_string());
            }
            "gldeltex" => {
                let names = names(t)?;
                if !self.tex_in_begin() {
                    self.tex.delete(&names);
                }
            }
            "glistex" => {
                let names = names(t)?;
                let name = names.first().copied().ok_or(Fail::Bad)?;
                let is = !self.tex_in_begin() && self.tex.is_texture(name);
                out.push(format!("istex {}", u8::from(is)));
            }
            "glprio" => {
                let p = super::tri::narrow(super::parse_float(t.get(1).ok_or(Fail::Bad)?)?);
                let names = names(t.get(1..).ok_or(Fail::Bad)?)?;
                if !self.tex_in_begin() {
                    self.tex.prioritize(&names, p);
                }
            }
            "glresident" => {
                let names = names(t)?;
                let (ret, flags, bad) = if self.tex_in_begin() { (false, Vec::new(), None) } else { self.tex.resident(&names) };
                self.tex_err(bad.map_or(Ok(()), Err));
                let mut line = format!("resident {}", u8::from(ret));
                for k in 0..names.len() {
                    line.push_str(&format!(" {}", flags.get(k).map_or(90, |f| u8::from(*f))));
                }
                out.push(line);
            }
            _ => return Err(Fail::Bad),
        }
        Ok(())
    }

    /// glTexGen's planes: the eye plane times the inverse modelview as a row vector, p' = p M^-1
    /// (MEASURED, golden opengl-32 / 64 on a real machine tex-open-more-eye: a translated,
    /// scaled modelview; GL.TEX.GEN)
    fn tex_gen(&mut self, coord: u32, pname: u32, v: &[f64]) -> Res<Result<(), Bad>> {
        if pname != EYE_PLANE || v.len() != 4 {
            return Ok(self.tex.set_gen(coord, pname, v));
        }
        let m = self.stacks.first().map_or(super::IDENTITY, |s| *s.top());
        // (MEASURED, golden opengl-32 / 64 tex-open-more-eye-singular: under a singular
        // modelview the plane kept as given, no error)
        let Some(inv) = invert(&m) else { return Ok(self.tex.set_gen(coord, pname, v)) };
        // (column k of the inverse: entries k*4 .. k*4 + 3 of the column-major array; each
        // entry of p M^-1 summed in float32 from p0 on)
        let p: Vec<f32> = v.iter().map(|x| super::tri::narrow(*x)).collect();
        let plane: Vec<f64> = inv.chunks_exact(4).map(|col| f64::from(col.iter().zip(&p).fold(0.0f32, |s, (a, p)| s + p * a))).collect();
        Ok(self.tex.set_gen(coord, pname, &plane))
    }

    /// A gl call of textures (glBindTexture .. glCopyTexSubImage2D).
    pub(super) fn tex_call(&mut self, name: &str, a: &[A]) -> Res<()> {
        let i = |k: usize| -> Res<i64> { a.get(k).map(|_| super::int(a, k)).ok_or(Fail::Bad) };
        let e = |k: usize| -> Res<u32> { Ok(u32::try_from(i(k)? & 0xFFFF_FFFF).unwrap_or(0)) };
        let f = |k: usize| -> Res<f64> { a.get(k).map(|_| super::fl(a, k)).ok_or(Fail::Bad) };
        match name {
            "glTexCoord1f" | "glTexCoord2f" | "glTexCoord3f" | "glTexCoord4f" => {
                let c = [0usize, 1, 2, 3].map(|k| a.get(k).map_or(if k == 3 { 1.0 } else { 0.0 }, |_| super::tri::narrow(super::fl(a, k))));
                self.tex.coord = c;
                return Ok(());
            }
            _ => {}
        }
        if self.tex_in_begin() {
            return Ok(());
        }
        let r = match name {
            "glBindTexture" => self.tex.bind(e(0)?, e(1)?),
            "glTexParameteri" | "glTexParameterf" => self.tex.set_param(e(0)?, e(1)?, &[f(2)?], false),
            "glTexEnvi" | "glTexEnvf" => self.tex.set_env(e(0)?, e(1)?, &[f(2)?]),
            "glTexGeni" | "glTexGenf" | "glTexGend" => self.tex_gen(e(0)?, e(1)?, &[f(2)?])?,
            "glCopyTexImage1D" => {
                let src = self.copy_source(i(3)?, i(4)?, i(5)?, 1)?;
                self.tex_image(e(0)?, true, i(1)?, e(2)?, (i(5)?, 1, i(6)?), src)?
            }
            "glCopyTexImage2D" => {
                let src = self.copy_source(i(3)?, i(4)?, i(5)?, i(6)?)?;
                self.tex_image(e(0)?, false, i(1)?, e(2)?, (i(5)?, i(6)?, i(7)?), src)?
            }
            "glCopyTexSubImage1D" => {
                let src = self.copy_source(i(3)?, i(4)?, i(5)?, 1)?;
                self.tex_sub(e(0)?, true, i(1)?, [i(2)?, 0, i(5)?, 1], src)?
            }
            "glCopyTexSubImage2D" => {
                let src = self.copy_source(i(4)?, i(5)?, i(6)?, i(7)?)?;
                self.tex_sub(e(0)?, false, i(1)?, [i(2)?, i(3)?, i(6)?, i(7)?], src)?
            }
            _ => return Err(Fail::Bad),
        };
        self.tex_err(r);
        Ok(())
    }

    /// the color buffer's pixels from (x, y), w x h, as glCopyTexImage reads them (through the
    /// pixel transfer: GL.TEX.COPY)
    fn copy_source(&self, x: i64, y: i64, w: i64, h: i64) -> Res<Source<'static>> {
        hit(clause::GL_TEX_COPY);
        let mut px = Vec::new();
        for j in 0..h.max(0) {
            for i in 0..w.max(0) {
                let (cx, cy) = (x.checked_add(i).ok_or(Fail::Bad)?, y.checked_add(j).ok_or(Fail::Bad)?);
                // (outside the window: memory past the DIB's, undefined, MEASURED on a real
                // machine tex-open-more-copy: other bytes on each executor; GL.UNDEFINED)
                if !(0..super::WIDTH).contains(&cx) || !(0..super::HEIGHT).contains(&cy) {
                    hit(clause::GL_UNDEFINED);
                    px.push([f32::NAN; 4]);
                    continue;
                }
                let p = usize::try_from(cy.checked_mul(super::WIDTH).and_then(|r| r.checked_add(cx)).ok_or(Fail::Bad)?).ok().ok_or(Fail::Bad)?;
                let c = self.transfer(self.dst_color(p).map(f64::from))?;
                px.push(c.map(super::tri::narrow));
            }
        }
        Ok(Source::Pixels(px))
    }

    /// an image source's pixels, w x h, unpacked and expanded to RGBA (GL 1.1 3.6.3, 3.6.4)
    fn source_pixels(&self, src: Source<'_>, w: u32, h: u32) -> Res<Pixels> {
        let (f, ty, given) = match src {
            Source::Pixels(p) => return Ok(p),
            Source::Image { f, ty, given } => (f, ty, given),
        };
        let (Some(comps), Some(size)) = (super::components(f), super::type_size(ty)) else { return Err(Fail::Bad) };
        let n = usize::try_from(w.checked_mul(h).ok_or(Fail::Bad)?).ok().ok_or(Fail::Bad)?;
        let Some(given) = given else {
            // (a null pointer: the level's texels undefined; the model makes them 0)
            return Ok(vec![[0.0; 4]; n]);
        };
        // (color indices, bitmaps too: through GL_PIXEL_MAP_I_TO_R .. A, whose initial tables
        // hold one zero each, then the transfer, as glDrawPixels has them in RGBA mode; MEASURED,
        // golden opengl-32 / 64 tex-open-source-16: GL.DRAW.INDEX)
        if f == COLOR_INDEX {
            hit(clause::GL_DRAW_INDEX);
            let c = self.transfer([0.0; 4])?.map(super::tri::narrow);
            return Ok(vec![c; n]);
        }
        if size == 0 {
            return Err(Fail::Bad);
        }
        let (start, k) = super::layout(&self.unpack, comps.len(), size, i64::from(w));
        let mut px = Vec::with_capacity(n);
        for j in 0..h {
            for i in 0..w {
                let mut c = [0.0f64, 0.0, 0.0, 1.0];
                for (m, which) in comps.iter().enumerate() {
                    let row = usize::try_from(j).ok().and_then(|j| j.checked_mul(k));
                    let col = usize::try_from(i).ok().and_then(|i| i.checked_mul(comps.len())).and_then(|c| c.checked_add(m));
                    let at = row.zip(col).and_then(|(r, c)| start.checked_add(r)?.checked_add(c)?.checked_mul(size)).ok_or(Fail::Bad)?;
                    let mut e = given.bytes(at, size)?.to_vec();
                    if self.unpack.first().is_some_and(|s| *s != 0) {
                        e.reverse();
                    }
                    let v = super::to_float(&e, ty);
                    // (clamped to [0, 1], a NaN to 0)
                    let v = if v.is_nan() { 0.0 } else { v.clamp(0.0, 1.0) };
                    match which {
                        4 => {
                            c[0] = v;
                            c[1] = v;
                            c[2] = v;
                        }
                        0..=3 => {
                            if let Some(x) = c.get_mut(*which) {
                                *x = v;
                            }
                        }
                        // (indices returned above, depth and stencil formats refused: GL.TEX.ERRORS)
                        _ => return Err(Fail::Bad),
                    }
                }
                px.push(self.transfer(c)?.map(super::tri::narrow));
            }
        }
        Ok(px)
    }

    /// glTexImage1D / 2D and glCopyTexImage1D / 2D (MEASURED, golden opengl-32 / 64
    /// tex-open-errors: the target, the type, the format, the level, the internal format, the
    /// border, the sizes (2^n plus the border, 0 taken), then the largest size: GL.TEX.ERRORS)
    fn tex_image(&mut self, target: u32, one: bool, level: i64, ifmt: u32, (w, h, b): (i64, i64, i64), src: Source<'_>) -> Res<Result<(), Bad>> {
        hit(clause::GL_TEX_IMAGE);
        // (a proxy target: the same checks, a size past the largest leaving the level empty with
        // no error, no texels read: MEASURED, on a real machine tex-open-more-proxy; GL.TEX.PROXY)
        let proxy = Target::proxy(target).filter(|t| (*t == Target::One) == one && matches!(src, Source::Image { .. }));
        let Some(t) = Target::of(target).or(proxy).filter(|t| (*t == Target::One) == one) else { return Ok(Err(Bad::Enum)) };
        if let Source::Image { f, ty, .. } = src {
            let types = (0x1400..=0x1406).contains(&ty) || (ty == BITMAP && f == COLOR_INDEX);
            if !types {
                return Ok(Err(Bad::Enum));
            }
            if super::components(f).is_none_or(|c| c.iter().any(|k| matches!(*k, 5 | 6))) {
                return Ok(Err(Bad::Enum));
            }
        }
        let Some(l) = usize::try_from(level).ok().filter(|l| *l < LEVELS) else { return Ok(Err(Bad::Value)) };
        let index = matches!(src, Source::Image { f: COLOR_INDEX, .. });
        // (the paletted formats only from color indices: MEASURED, golden opengl-32
        // tex-open-format-32, INVALID_OPERATION from RGBA; tex-open-more-paletted from them)
        let (base, store) = match internal(ifmt) {
            Some(k) => k,
            None if paletted(ifmt) && index => (Base::Index, Store::Bytes),
            None => return Ok(Err(if paletted(ifmt) { Bad::Operation } else { Bad::Enum })),
        };
        let Ok(border) = u32::try_from(b & 0xFFFF_FFFF) else { return Ok(Err(Bad::Value)) };
        if border > 1 {
            return Ok(Err(Bad::Value));
        }
        let h = if one { 1i64.saturating_add(2i64.saturating_mul(i64::from(border))) } else { h };
        let inner = |s: i64| -> Option<u32> { u32::try_from(s.checked_sub(2i64.saturating_mul(i64::from(border)))?).ok().filter(|v| *v == 0 || v.is_power_of_two()) };
        let (Some(iw), Some(ih)) = (inner(w), inner(h)) else { return Ok(Err(Bad::Value)) };
        let (Ok(width), Ok(height)) = (u32::try_from(w), u32::try_from(h)) else { return Ok(Err(Bad::Value)) };
        if proxy.is_some() {
            hit(clause::GL_TEX_PROXY);
            let fits = iw <= MAX_SIZE && ih <= MAX_SIZE;
            if let Some(slot) = self.tex.proxies.get_mut(t).levels.get_mut(l) {
                *slot = fits.then(|| Level { width, height, border, ifmt, base, store, texels: Vec::new() });
            }
            return Ok(Ok(()));
        }
        if iw > MAX_SIZE || ih > MAX_SIZE {
            return Ok(Err(Bad::Value));
        }
        let px = self.source_pixels(src, width, height)?;
        let texels = px.into_iter().map(|c| reduce(base, store, c)).collect();
        let o = self.tex.object_mut(t);
        if let Some(slot) = o.levels.get_mut(l) {
            *slot = Some(Level { width, height, border, ifmt, base, store, texels });
        }
        o.resident = true;
        Ok(Ok(()))
    }

    /// glTexSubImage1D / 2D and glCopyTexSubImage1D / 2D: `rect` x, y, w, h (MEASURED, golden
    /// opengl-32 tex-open-sub, tex-open-errors: past the level, or of an empty level,
    /// INVALID_VALUE: GL.TEX.SUB)
    fn tex_sub(&mut self, target: u32, one: bool, level: i64, rect: [i64; 4], src: Source<'_>) -> Res<Result<(), Bad>> {
        hit(clause::GL_TEX_SUB);
        let Some(t) = Target::of(target).filter(|t| (*t == Target::One) == one) else { return Ok(Err(Bad::Enum)) };
        if let Source::Image { f, ty, .. } = src
            && (!(0x1400..=0x1406).contains(&ty) || super::components(f).is_none_or(|c| c.iter().any(|k| matches!(*k, 5..=7))))
        {
            return Ok(Err(Bad::Enum));
        }
        let Some(l) = usize::try_from(level).ok().filter(|l| *l < LEVELS) else { return Ok(Err(Bad::Value)) };
        let [x, y, w, h] = rect;
        let Some(lv) = self.tex.object(t).levels.get(l).and_then(Option::as_ref) else { return Ok(Err(Bad::Value)) };
        let (b, lw, lh) = (i64::from(lv.border), i64::from(lv.width), i64::from(if one { 1 } else { lv.height }));
        let fits = |o: i64, n: i64, size: i64| o >= b.saturating_neg() && n >= 0 && o.saturating_add(n) <= size.saturating_sub(b);
        if !fits(x, w, lw) || !fits(if one { 0 } else { y }, h, lh) {
            return Ok(Err(Bad::Value));
        }
        let (base, store) = (lv.base, lv.store);
        let (Ok(width), Ok(height)) = (u32::try_from(w), u32::try_from(h)) else { return Ok(Err(Bad::Value)) };
        let px = self.source_pixels(src, width, height)?;
        let o = self.tex.object_mut(t);
        let Some(lv) = o.levels.get_mut(l).and_then(Option::as_mut) else { return Err(Fail::Bad) };
        let row_len = i64::from(lv.width);
        for (k, c) in px.into_iter().enumerate() {
            let k = i64::try_from(k).ok().ok_or(Fail::Bad)?;
            let (i, j) = (k.checked_rem(w).ok_or(Fail::Bad)?, k.checked_div(w).ok_or(Fail::Bad)?);
            let (tx, ty) = (x.saturating_add(b).saturating_add(i), (if one { 0 } else { y }).saturating_add(b).saturating_add(j));
            let at = usize::try_from(ty.saturating_mul(row_len).saturating_add(tx)).ok().ok_or(Fail::Bad)?;
            if let Some(texel) = lv.texels.get_mut(at) {
                *texel = reduce(base, store, c);
            }
        }
        Ok(Ok(()))
    }

    /// glGetTexImage into `buf` (MEASURED, golden opengl-32 tex-open-format-*: each level as
    /// RGBA (Level::rgba), through the pixel transfer, packed as glReadPixels packs; an empty
    /// level writes nothing: GL.TEX.GETIMAGE)
    fn get_tex_image(&mut self, target: u32, level: i64, f: u32, ty: u32, buf: &mut [u8], undefined: &mut [bool]) -> Res<Result<(), Bad>> {
        hit(clause::GL_TEX_GETIMAGE);
        if self.in_begin.is_some() {
            return Ok(Err(Bad::Operation));
        }
        let Some(t) = Target::of(target) else { return Ok(Err(Bad::Enum)) };
        let Some(l) = usize::try_from(level).ok().filter(|l| *l < LEVELS) else { return Ok(Err(Bad::Value)) };
        let Some(comps) = super::components(f).filter(|c| c.iter().all(|k| matches!(*k, 0..=4 | 7))) else { return Ok(Err(Bad::Enum)) };
        let Some(size) = super::type_size(ty).filter(|_| ty != BITMAP) else { return Ok(Err(Bad::Enum)) };
        let Some(lv) = self.tex.object(t).levels.get(l).and_then(Option::as_ref).cloned() else { return Ok(Ok(())) };
        if f == COLOR_INDEX {
            return Ok(Err(Bad::Operation));
        }
        let (start, k) = super::layout(&self.pack, comps.len(), size, i64::from(lv.width));
        for j in 0..lv.height {
            for i in 0..lv.width {
                let texel = lv.texel(i, j).ok_or(Fail::Bad)?;
                let unknown = texel.iter().any(|v| v.is_nan());
                let c = lv.rgba(texel);
                let c = self.transfer(c.map(f64::from))?.map(super::tri::narrow);
                for (m, which) in comps.iter().enumerate() {
                    let [r, g, b, _] = c;
                    let v = match which {
                        // (in channel units, each times f32(1/255): MEASURED, golden opengl-32
                        // tex-open-format-1, a luminance-alpha level's L read back an ulp above)
                        // (a luminance or intensity level's own value as it is)
                        4 if matches!((lv.store, lv.base), (Store::Floats, Base::Luminance | Base::Intensity)) => r,
                        // (byte levels: the sum of the bytes' own values, tex-open-format-3)
                        4 if lv.store == Store::Bytes => ((g + r) + b).min(1.0),
                        4 => ((g * 255.0 * INV255 + r * 255.0 * INV255) + b * 255.0 * INV255).min(1.0),
                        _ => c.get(*which).copied().ok_or(Fail::Bad)?,
                    };
                    let mut e = super::from_float(v, ty);
                    if self.pack.first().is_some_and(|s| *s != 0) {
                        e.reverse();
                    }
                    let row = usize::try_from(j).ok().and_then(|j| j.checked_mul(k));
                    let col = usize::try_from(i).ok().and_then(|i| i.checked_mul(comps.len())).and_then(|c| c.checked_add(m));
                    let at = row.zip(col).and_then(|(r, c)| start.checked_add(r)?.checked_add(c)?.checked_mul(size)).ok_or(Fail::Bad)?;
                    if let Some(dst) = at.checked_add(size).and_then(|end| buf.get_mut(at..end)) {
                        dst.copy_from_slice(&e);
                    }
                    if let Some(u) = at.checked_add(size).and_then(|end| undefined.get_mut(at..end)) {
                        u.fill(unknown);
                    }
                }
            }
        }
        Ok(Ok(()))
    }
}

/// Where an image's pixels come from.
pub enum Source<'a> {
    /// the executor's buffer as a format and type (None: a null pointer)
    Image { f: u32, ty: u32, given: Option<&'a Given> },
    /// the color buffer, read already
    Pixels(Pixels),
}

/// `gltex`'s bytes: xorshift32 of the seed (runner/opengl.c fill_rbuf), none for 0 bytes
fn given(seed: i64, n: i64) -> Res<Option<Given>> {
    if n == 0 {
        return Ok(None);
    }
    let n = usize::try_from(n).ok().ok_or(Fail::Bad)?;
    let mut r = crate::rng::XorShift32::new(u32::try_from(seed & 0xFFFF_FFFF).unwrap_or(0));
    Ok(Some(Given::new((0..n).map(|_| r.next().to_le_bytes().first().copied().unwrap_or(0)).collect())))
}

/// the names of an op's tokens from the second on
fn names(t: &[String]) -> Res<Vec<u32>> {
    t.iter().skip(1).map(|x| num(x).and_then(|v| u32::try_from(v & 0xFFFF_FFFF).ok()).ok_or(Fail::Bad)).collect()
}

/// the inverse of a column-major 4 by 4 matrix as Windows takes it for eye planes, column-major
/// too, in float32; none when singular (MEASURED, golden opengl-32 / 64 tex-eye-more-*: a
/// matrix whose last row is 0 0 0 1 by the cofactors of its 3 by 3 part times 1 / det, the
/// translation then -(M3^-1 t); any other by the 4 by 4 cofactors times 1 / det: GL.TEX.GEN)
pub fn invert(m: &[f32; 16]) -> Option<[f32; 16]> {
    let [a00, a10, a20, a30, a01, a11, a21, a31, a02, a12, a22, a32, a03, a13, a23, a33] = *m;
    let rows = [[a00, a01, a02, a03], [a10, a11, a12, a13], [a20, a21, a22, a23], [a30, a31, a32, a33]];
    let affine = [a30, a31, a32].iter().all(|v| v.to_bits() & 0x7FFF_FFFF == 0) && a33.to_bits() == 1.0f32.to_bits();
    let inv: [[f32; 4]; 4] = if affine {
        let x = [[a00, a01, a02], [a10, a11, a12], [a20, a21, a22]];
        let cof = |r: usize, k: usize| -> Option<f32> {
            let [r1, r2] = others3(r);
            let [k1, k2] = others3(k);
            let at = |i: usize, j: usize| x.get(i).and_then(|row| row.get(j)).copied();
            let v = at(r1, k1)? * at(r2, k2)? - at(r1, k2)? * at(r2, k1)?;
            Some(if r.wrapping_add(k).is_multiple_of(2) { v } else { -v })
        };
        let det = a00 * cof(0, 0)? + a01 * cof(0, 1)? + a02 * cof(0, 2)?;
        if det.to_bits() & 0x7FFF_FFFF == 0 {
            return None;
        }
        let rd = 1.0 / det;
        let row = |r: usize| -> Option<[f32; 4]> {
            let [i0, i1, i2] = [cof(0, r)? * rd, cof(1, r)? * rd, cof(2, r)? * rd];
            Some([i0, i1, i2, -(i0 * a03 + i1 * a13 + i2 * a23)])
        };
        [row(0)?, row(1)?, row(2)?, [0.0, 0.0, 0.0, 1.0]]
    } else {
        let cof = |r: usize, c: usize| -> Option<f32> {
            let pick = |i: usize| -> Option<[f32; 3]> {
                let row = rows.get(i)?;
                let [j0, j1, j2] = others4(c);
                Some([*row.get(j0)?, *row.get(j1)?, *row.get(j2)?])
            };
            let [i0, i1, i2] = others4(r);
            let v = det3([pick(i0)?, pick(i1)?, pick(i2)?]);
            Some(if r.wrapping_add(c).is_multiple_of(2) { v } else { -v })
        };
        let det = 0.0 + a00 * cof(0, 0)? + a01 * cof(0, 1)? + a02 * cof(0, 2)? + a03 * cof(0, 3)?;
        if det.to_bits() & 0x7FFF_FFFF == 0 {
            return None;
        }
        let rd = 1.0 / det;
        let row = |r: usize| -> Option<[f32; 4]> { Some([cof(0, r)? * rd, cof(1, r)? * rd, cof(2, r)? * rd, cof(3, r)? * rd]) };
        [row(0)?, row(1)?, row(2)?, row(3)?]
    };
    let [[i00, i01, i02, i03], [i10, i11, i12, i13], [i20, i21, i22, i23], [i30, i31, i32, i33]] = inv;
    Some([i00, i10, i20, i30, i01, i11, i21, i31, i02, i12, i22, i32, i03, i13, i23, i33])
}

/// the indices of a 3 by 3 matrix other than `i`
fn others3(i: usize) -> [usize; 2] {
    match i {
        0 => [1, 2],
        1 => [0, 2],
        _ => [0, 1],
    }
}

/// the indices of a 4 by 4 matrix other than `i`
fn others4(i: usize) -> [usize; 3] {
    match i {
        0 => [1, 2, 3],
        1 => [0, 2, 3],
        2 => [0, 1, 3],
        _ => [0, 1, 2],
    }
}

/// a 3 by 3 determinant along its first row, each product rounded, in float32
fn det3(r: [[f32; 3]; 3]) -> f32 {
    let [[a00, a01, a02], [a10, a11, a12], [a20, a21, a22]] = r;
    a00 * (a11 * a22 - a12 * a21) - a01 * (a10 * a22 - a12 * a20) + a02 * (a10 * a21 - a11 * a20)
}
