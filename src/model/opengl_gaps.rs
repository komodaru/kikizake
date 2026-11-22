//! The calls of what the opengl model does not follow yet, each named by its open clause
//! (spec/opengl.txt GL.LIST .. WGL.OTHER): the gap-* cases reach them.

use super::{Fail, Res};
use crate::spec::{open, OpenId};


/// the open clause of a function the model does not follow, by its name
pub fn of(name: &str) -> Option<OpenId> {
    Some(match name {
        "glMap1f" | "glMap1d" | "glMap2f" | "glMap2d" | "glMapGrid1f" | "glMapGrid2f" | "glMapGrid1d" | "glMapGrid2d" | "glEvalCoord1f" | "glEvalCoord2f" | "glEvalCoord1d" | "glEvalCoord2d" | "glEvalCoord1fv" | "glEvalCoord2fv" | "glEvalCoord1dv" | "glEvalCoord2dv" | "glEvalPoint1" | "glEvalPoint2" | "glEvalMesh1" | "glEvalMesh2" | "glGetMapfv" | "glGetMapdv" | "glGetMapiv" => open::GL_EVAL,
        "glRenderMode" | "glSelectBuffer" | "glFeedbackBuffer" | "glInitNames" | "glLoadName" | "glPushName" | "glPopName" | "glPassThrough" => open::GL_SELECT,
        "glLightf" | "glLighti" | "glLightfv" | "glLightiv" | "glLightModelf" | "glLightModeli" | "glLightModelfv" | "glLightModeliv" | "glMaterialf" | "glMateriali" | "glMaterialfv" | "glMaterialiv" | "glColorMaterial" | "glGetLightfv" | "glGetLightiv" | "glGetMaterialfv" | "glGetMaterialiv" => open::GL_LIGHT_STATE,
        "glClipPlane" | "glGetClipPlane" => open::GL_CLIPPLANE,
        "glBitmap" => open::GL_BITMAP,
        "glCopyPixels" => open::GL_COPYPIXELS,
        "glPolygonStipple" | "glGetPolygonStipple" => open::GL_POLYGON_STIPPLE_PATTERN,
        "glPixelMapfv" | "glPixelMapuiv" | "glPixelMapusv" | "glGetPixelMapfv" | "glGetPixelMapuiv" | "glGetPixelMapusv" => open::GL_PIXELMAP,
        _ => return None,
    })
}

/// a `gl` call of the executor's table the model does not follow
pub fn call(name: &str) -> Option<OpenId> {
    of(name)
}

/// the open clause an executor op of arrays, results or WGL reaches (glp, glq, glr: by the function
/// it names; globuf: selection and feedback; wglx: the other WGL functions)
pub fn op(t: &[String]) -> Res<OpenId> {
    match t.first().map(String::as_str) {
        Some("globuf") => Ok(open::GL_SELECT),
        Some("wglx") => Ok(open::WGL_OTHER),
        Some("glp" | "glq" | "glr") => t.get(1).and_then(|n| of(n)).ok_or(Fail::Bad),
        Some(_) | None => Err(Fail::Bad),
    }
}
