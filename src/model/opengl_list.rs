//! Display lists (GL 1.1 5.4): names, compiling, executing and nesting, glCallLists' types and
//! the list base. A list keeps the executor's lines of the commands GL compiles and runs them
//! again when called (spec/opengl.txt GL.LIST).

use std::collections::BTreeMap;

use super::e::{INVALID_ENUM, INVALID_OPERATION, INVALID_VALUE};
use super::{Fail, Gl, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit, open};

const COMPILE: u32 = 0x1300;
const COMPILE_AND_EXECUTE: u32 = 0x1301;
/// GL_MAX_LIST_NESTING
const NESTING: u32 = 64;

/// A list being compiled.
#[derive(Clone, Debug)]
pub struct Compiling {
    pub name: u32,
    pub mode: u32,
    pub lines: Vec<Vec<String>>,
}

/// The display lists of a context.
#[derive(Clone, Debug, Default)]
pub struct Lists {
    pub lists: BTreeMap<u32, Vec<Vec<String>>>,
    pub compiling: Option<Compiling>,
    pub base: u32,
    /// how deep calls run lists now (0: none)
    pub depth: u32,
}

impl Lists {
    /// glGenLists: n names in one run, the lowest free run from 1, each an empty list
    fn gen_names(&mut self, n: u32) -> u32 {
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
    let size: usize = match el {
        "b" | "ub" => 1,
        "s" | "us" => 2,
        "i" | "ui" => 4,
        _ => return Err(Fail::Unmodeled(open::GL_LIST)),
    };
    let mut bytes = Vec::new();
    for x in ints(t, 6)? {
        bytes.extend(x.to_le_bytes().iter().take(size));
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
            0x1407..=0x1409 => return Err(Fail::Unmodeled(open::GL_LIST)),
            0x1406 => return Err(Fail::Unmodeled(open::GL_LIST)),
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
                let Some(c) = ctx.lists.compiling.as_mut().filter(|_| !replaying) else { return Ok(false) };
                hit(clause::GL_LIST);
                c.lines.push(t.to_vec());
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
                    for k in 0..u32::try_from(range.min(1 << 20)).unwrap_or(0) {
                        ctx.lists.lists.remove(&first.wrapping_add(k));
                    }
                }
                Ok(true)
            }
            "glListBase" => {
                if let Some(c) = ctx.lists.compiling.as_mut().filter(|_| !replaying) {
                    c.lines.push(t.to_vec());
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
                    c.lines.push(t.to_vec());
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
        if ctx.lists.depth >= NESTING {
            return Err(Fail::Unmodeled(open::GL_LIST));
        }
        let Some(lines) = ctx.lists.lists.get(&name).cloned() else { return Ok(()) };
        ctx.lists.depth = ctx.lists.depth.saturating_add(1);
        let mut r = Ok(());
        for tok in lines {
            let op = Op { line: 0, tok };
            if let Err(e) = self.op(&op, out) {
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
