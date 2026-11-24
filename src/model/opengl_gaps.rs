//! The calls of what the opengl model does not follow yet, each named by its open clause
//! (spec/opengl.txt GL.LIST .. WGL.OTHER): the gap-* cases reach them.

use super::{Fail, Res};
use crate::spec::{open, OpenId};


/// the open clause of a function the model does not follow, by its name
pub fn of(name: &str) -> Option<OpenId> {
    Some(match name {
        "glLightfv" | "glLightiv" | "glLightModelfv" | "glLightModeliv" | "glMaterialfv" | "glMaterialiv" | "glGetLightfv" | "glGetLightiv" | "glGetMaterialfv" | "glGetMaterialiv" => open::GL_LIGHT_STATE,
        _ => return None,
    })
}

/// a `gl` call of the executor's table the model does not follow
pub fn call(name: &str) -> Option<OpenId> {
    of(name)
}

/// the open clause an executor op of arrays, results or WGL reaches (glp, glq, glr: by the function
/// it names)
pub fn op(t: &[String]) -> Res<OpenId> {
    match t.first().map(String::as_str) {
        Some("glp" | "glq" | "glr") => t.get(1).and_then(|n| of(n)).ok_or(Fail::Bad),
        Some(_) | None => Err(Fail::Bad),
    }
}
