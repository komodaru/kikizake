//! Display lists (GL 1.1 5.4): names, compiling, executing and nesting, glCallLists' types and
//! the list base. A list keeps the executor's lines of the commands GL compiles and runs them
//! again when called (spec/opengl.txt GL.LIST).

use std::collections::BTreeMap;

use super::e::{INVALID_ENUM, INVALID_OPERATION, INVALID_VALUE};
use super::{tri, Fail, Gl, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit};

const COMPILE: u32 = 0x1300;
pub(super) const COMPILE_AND_EXECUTE: u32 = 0x1301;
/// GL_MAX_LIST_NESTING
const NESTING: u32 = 64;

/// A list being compiled.
#[derive(Clone, Debug)]
pub struct Compiling {
    pub name: u32,
    pub mode: u32,
    pub lines: Vec<Line>,
}

/// A line a list keeps: the executor's tokens and, for an image, the unpacking it was compiled
/// under (GL 1.1 5.4: client data are read at compile time; MEASURED, golden opengl-32 / 64
/// gap-list-2, GL_UNPACK_ROW_LENGTH changed after compiling leaves the image as it was)
#[derive(Clone, Debug)]
pub struct Line {
    pub tok: Vec<String>,
    pub unpack: Option<[i64; 6]>,
    /// for glArrayElement, glDrawArrays, glDrawElements: the arrays as they were at compile time
    /// (GL 1.1 5.4, their elements read then)
    pub arrays: Option<Box<super::array::Snapshot>>,
}

/// The display lists of a context.
#[derive(Clone, Debug, Default)]
pub struct Lists {
    pub lists: BTreeMap<u32, Vec<Line>>,
    pub compiling: Option<Compiling>,
    pub base: u32,
    /// how deep calls run lists now (0: none)
    pub depth: u32,
}

impl Lists {
    /// glPopAttrib's LIST_BIT: the list base (the lists themselves and the list being compiled
    /// are in no group; every field is placed, so a new one does not compile until it is)
    pub fn pop(&mut self, saved: &Lists) {
        let Lists { lists: _, compiling: _, base, depth: _ } = saved;
        self.base = *base;
    }

    pub(super) fn delete(&mut self, first: u32, range: u64) -> Res<()> {
        let includes = |name: u32| u64::from(name.wrapping_sub(first)) < range;
        // Native x86 and x64 fault when deletion reaches a live UINT_MAX list.
        if includes(u32::MAX) && self.lists.contains_key(&u32::MAX) {
            return Err(Fail::Crash);
        }
        self.lists.retain(|&name, _| !includes(name));
        Ok(())
    }

    /// glGenLists: n names in one run, the lowest free run from 1, each an empty list
    pub fn gen_names(&mut self, n: u32) -> u32 {
        let mut first = 1u32;
        let mut k = 1u32;
        while k.wrapping_sub(first) < n {
            if self.lists.contains_key(&k) {
                first = k.wrapping_add(1);
            }
            k = k.wrapping_add(1);
        }
        for i in 0..n {
            self.lists.insert(first.wrapping_add(i), Vec::new());
        }
        first
    }
}

/// what an executor line does to display lists
enum Kind {
    /// glNewList, glEndList, glGenLists, glIsList, glDeleteLists, glListBase, glCallList(s): here
    Lists,
    /// run at once, never compiled (GL 1.1 5.4's list, and the executor's queries)
    Immediate,
    /// compiled into a list being made
    Compiled,
}

fn kind(t: &[String]) -> Kind {
    let op = t.first().map_or("", String::as_str);
    let f = t.get(1).map_or("", String::as_str);
    match op {
        "gl" | "glp" | "glr" => match f {
            "glNewList" | "glEndList" | "glGenLists" | "glIsList" | "glDeleteLists" | "glListBase" | "glCallList" | "glCallLists" => Kind::Lists,
            "glFlush" | "glFinish" | "glPixelStorei" | "glPixelStoref" | "glEnableClientState" | "glDisableClientState" | "glPushClientAttrib" | "glPopClientAttrib" | "glVertexPointer" | "glNormalPointer" | "glColorPointer" | "glIndexPointer" | "glTexCoordPointer" | "glEdgeFlagPointer" | "glInterleavedArrays" | "glFeedbackBuffer" | "glSelectBuffer" | "glRenderMode" => Kind::Immediate,
            _ if op == "glr" => Kind::Immediate,
            _ => Kind::Compiled,
        },
        // (glTexImage2D of PROXY_TEXTURE_2D runs at once and is not compiled, GL 1.1 3.8.3:
        // the review of 84b05c8, 3.5; the executor gives every target but TEXTURE_1D to
        // glTexImage2D, so PROXY_TEXTURE_1D there is an INVALID_ENUM command, compiled)
        "gltex" if t.get(1).and_then(|x| num(x)).is_some_and(|target| target == i64::from(super::tex::PROXY_2D)) => Kind::Immediate,
        "glmat" | "glfogv" | "gldraw" | "gltex" | "gltexsub" | "gltexv" | "glprio" => Kind::Compiled,
        _ => Kind::Immediate,
    }
}

/// the integers of a line from token k on
fn ints(t: &[String], k: usize) -> Res<Vec<i64>> {
    t.iter().skip(k).map(|x| num(x).ok_or(Fail::Bad)).collect()
}

fn low32(v: i64) -> u32 {
    u32::try_from(v & 0xFFFF_FFFF).unwrap_or(0)
}

/// glCallLists' names: `glp glCallLists ii <n> <type> <element type> <values>` (the executor's
/// array, its elements read as the GL type: GL 1.1 5.4 table 5.2)
fn call_names(t: &[String]) -> Res<Result<Vec<u32>, u32>> {
    let v = ints(t.get(..5).ok_or(Fail::Bad)?, 3)?;
    let (n, ty) = (*v.first().ok_or(Fail::Bad)?, low32(*v.get(1).ok_or(Fail::Bad)?));
    let el = t.get(5).map(String::as_str).ok_or(Fail::Bad)?;
    let mut bytes = Vec::new();
    if el == "f" {
        for x in t.iter().skip(6) {
            let v: f32 = x.parse().ok().ok_or(Fail::Bad)?;
            bytes.extend(v.to_le_bytes());
        }
    } else {
        let size: usize = match el {
            "b" | "ub" => 1,
            "s" | "us" => 2,
            "i" | "ui" => 4,
            // (the executor's element types only)
            _ => return Err(Fail::Bad),
        };
        for x in ints(t, 6)? {
            bytes.extend(x.to_le_bytes().iter().take(size));
        }
    }
    let Ok(n) = usize::try_from(n) else { return Ok(Err(INVALID_VALUE)) };
    let at = |k: usize, w: usize| -> Vec<u8> { (0..w).map(|j| k.checked_mul(w).and_then(|o| o.checked_add(j)).and_then(|i| bytes.get(i)).copied().unwrap_or(0)).collect() };
    let mut names = Vec::new();
    for k in 0..n {
        let b = |w: usize| at(k, w);
        let value: i64 = match ty {
            0x1400 => i64::from(i8::from_le_bytes([b(1).first().copied().unwrap_or(0)])),
            0x1401 => i64::from(b(1).first().copied().unwrap_or(0)),
            0x1402 => i64::from(i16::from_le_bytes(b(2).try_into().unwrap_or([0; 2]))),
            0x1403 => i64::from(u16::from_le_bytes(b(2).try_into().unwrap_or([0; 2]))),
            0x1404 => i64::from(i32::from_le_bytes(b(4).try_into().unwrap_or([0; 4]))),
            0x1405 => i64::from(u32::from_le_bytes(b(4).try_into().unwrap_or([0; 4]))),
            // (GL_2_BYTES, 3, 4: the bytes high first, MEASURED, golden opengl-32 / 64 gap-list-1)
            0x1407..=0x1409 => {
                let w = usize::try_from(ty.saturating_sub(0x1405)).unwrap_or(0);
                b(w).iter().fold(0i64, |v, x| v.saturating_mul(256).saturating_add(i64::from(*x)))
            }
            // (GL_FLOAT: the float's value, gap-list-1)
            0x1406 => tri::trunc(f32::from_le_bytes(b(4).try_into().unwrap_or([0; 4]))),
            _ => return Ok(Err(INVALID_ENUM)),
        };
        names.push(low32(value));
    }
    Ok(Ok(names))
}

impl Gl {
    /// a line under display lists: Ok(true) when done here (a list command, or a line compiled
    /// and not run), Ok(false) to run it as any other
    pub(super) fn list_line(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        let Some(ctx) = self.ctx.as_mut() else { return Ok(false) };
        let replaying = ctx.lists.depth > 0;
        match kind(t) {
            Kind::Immediate => Ok(false),
            Kind::Compiled => {
                if replaying || ctx.lists.compiling.is_none() {
                    return Ok(false);
                }
                hit(clause::GL_LIST);
                let image = matches!(t.first().map(String::as_str), Some("gldraw" | "gltex" | "gltexsub"))
                    || t.get(1).is_some_and(|s| matches!(s.as_str(), "glPolygonStipple" | "glBitmap"));
                let drawn = matches!(t.get(1).map(String::as_str), Some("glArrayElement" | "glDrawArrays" | "glDrawElements"));
                let arrays = drawn.then(|| Box::new(super::array::Snapshot::of(ctx)));
                let Some(c) = ctx.lists.compiling.as_mut() else { return Err(Fail::Bad) };
                c.lines.push(Line { tok: t.to_vec(), unpack: image.then_some(ctx.unpack), arrays });
                Ok(c.mode == COMPILE)
            }
            Kind::Lists => self.list_command(t, out, replaying),
        }
    }

    fn list_command(&mut self, t: &[String], out: &mut Vec<String>, replaying: bool) -> Res<bool> {
        let Some(ctx) = self.ctx.as_mut() else { return Ok(false) };
        hit(clause::GL_LIST);
        let f = t.get(1).map_or("", String::as_str);
        let in_begin = ctx.in_begin.is_some();
        match f {
            "glNewList" => {
                let v = ints(t, 2)?;
                let (name, mode) = (low32(*v.first().ok_or(Fail::Bad)?), low32(*v.get(1).ok_or(Fail::Bad)?));
                // (MEASURED, golden opengl-32 / 64 gap-list-0: list 0 INVALID_VALUE, another mode
                // INVALID_ENUM, inside a list INVALID_OPERATION)
                let e = if in_begin || ctx.lists.compiling.is_some() {
                    Some(INVALID_OPERATION)
                } else if name == 0 {
                    Some(INVALID_VALUE)
                } else if mode != COMPILE && mode != COMPILE_AND_EXECUTE {
                    Some(INVALID_ENUM)
                } else {
                    None
                };
                match e {
                    Some(e) => ctx.error(e),
                    None => ctx.lists.compiling = Some(Compiling { name, mode, lines: Vec::new() }),
                }
                Ok(true)
            }
            "glEndList" => {
                if in_begin {
                    ctx.error(INVALID_OPERATION);
                    return Ok(true);
                }
                match ctx.lists.compiling.take() {
                    Some(c) => {
                        ctx.lists.lists.insert(c.name, c.lines);
                    }
                    None => ctx.error(INVALID_OPERATION),
                }
                Ok(true)
            }
            "glGenLists" | "glIsList" => {
                let v = ints(t, 4)?;
                let ty = t.get(2).map(String::as_str).ok_or(Fail::Bad)?;
                let x = *v.first().ok_or(Fail::Bad)?;
                let r: i64 = if in_begin {
                    ctx.error(INVALID_OPERATION);
                    0
                } else if f == "glIsList" {
                    i64::from(ctx.lists.lists.contains_key(&low32(x)))
                } else {
                    match u32::try_from(x) {
                        Ok(0) => 0,
                        Ok(n) => i64::from(ctx.lists.gen_names(n)),
                        Err(_) => {
                            ctx.error(INVALID_VALUE);
                            0
                        }
                    }
                };
                out.push(match ty {
                    "b" => format!("ret {}", r & 0xff),
                    "i" => format!("ret {}", i32::try_from(r & 0xFFFF_FFFF).unwrap_or(-1)),
                    _ => format!("ret {}", r & 0xFFFF_FFFF),
                });
                Ok(true)
            }
            "glDeleteLists" => {
                let v = ints(t, 2)?;
                let (first, range) = (low32(*v.first().ok_or(Fail::Bad)?), *v.get(1).ok_or(Fail::Bad)?);
                if in_begin {
                    ctx.error(INVALID_OPERATION);
                } else if range < 0 {
                    ctx.error(INVALID_VALUE);
                } else {
                    // (every list of first .. first + range - 1, names past 2^32 - 1 wrapping as
                    // the names do, the names no list has ignored; a range is read of the lists
                    // there are, not counted out: the review of 84b05c8, 5, a list 1048577 left
                    // by glDeleteLists(1, 1048577))
                    let range = u64::try_from(range).unwrap_or(0);
                    ctx.lists.delete(first, range)?;
                }
                Ok(true)
            }
            "glListBase" => {
                if let Some(c) = ctx.lists.compiling.as_mut().filter(|_| !replaying) {
                    c.lines.push(Line { tok: t.to_vec(), unpack: None, arrays: None });
                    if c.mode == COMPILE {
                        return Ok(true);
                    }
                }
                if in_begin {
                    ctx.error(INVALID_OPERATION);
                    return Ok(true);
                }
                ctx.lists.base = low32(*ints(t, 2)?.first().ok_or(Fail::Bad)?);
                Ok(true)
            }
            "glCallList" | "glCallLists" => {
                if let Some(c) = ctx.lists.compiling.as_mut().filter(|_| !replaying) {
                    c.lines.push(Line { tok: t.to_vec(), unpack: None, arrays: None });
                    if c.mode == COMPILE {
                        return Ok(true);
                    }
                }
                let names = if f == "glCallList" {
                    vec![low32(*ints(t, 2)?.first().ok_or(Fail::Bad)?)]
                } else {
                    match call_names(t)? {
                        Ok(n) => n.into_iter().map(|n| n.wrapping_add(ctx.lists.base)).collect(),
                        Err(e) => {
                            ctx.error(e);
                            return Ok(true);
                        }
                    }
                };
                for name in names {
                    self.call_list(name, out)?;
                }
                Ok(true)
            }
            _ => Err(Fail::Bad),
        }
    }

    /// a list's lines run again, up to GL_MAX_LIST_NESTING deep; an unknown list nothing
    fn call_list(&mut self, name: u32, out: &mut Vec<String>) -> Res<()> {
        let Some(ctx) = self.ctx.as_mut() else { return Ok(()) };
        // (past GL_MAX_LIST_NESTING the call is not made, no error: MEASURED, golden opengl-32 /
        // 64 gap-list-1, a list calling itself runs 64 deep)
        if ctx.lists.depth >= NESTING {
            return Ok(());
        }
        let Some(lines) = ctx.lists.lists.get(&name).cloned() else { return Ok(()) };
        ctx.lists.depth = ctx.lists.depth.saturating_add(1);
        let mut r = Ok(());
        for line in lines {
            let op = Op { line: 0, tok: line.tok };
            // (an image unpacked as it was at compile time)
            let kept = match (line.unpack, self.ctx.as_mut()) {
                (Some(u), Some(ctx)) => Some(std::mem::replace(&mut ctx.unpack, u)),
                _ => None,
            };
            // (arrays read as they were at compile time)
            let kept_arrays = match (line.arrays, self.ctx.as_mut()) {
                (Some(a), Some(ctx)) => Some(a.swap(ctx)),
                _ => None,
            };
            let done = self.op(&op, out);
            if let (Some(u), Some(ctx)) = (kept, self.ctx.as_mut()) {
                ctx.unpack = u;
            }
            if let (Some(a), Some(ctx)) = (kept_arrays, self.ctx.as_mut()) {
                a.swap(ctx);
            }
            if let Err(e) = done {
                r = Err(e);
                break;
            }
        }
        if let Some(ctx) = self.ctx.as_mut() {
            ctx.lists.depth = ctx.lists.depth.saturating_sub(1);
        }
        r
    }
}
