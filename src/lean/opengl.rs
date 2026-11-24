//! OpenGL evaluator control-point storage (GL.EVAL).
use super::{arg, one, sampled, Domain, Mirror};

pub static MIRRORS: &[Mirror] = &[Mirror {
    name: "gl.control_index",
    lean: "spec/lean/Kikizake/OpenGL/Evaluator.lean",
    domain: Domain::Sampled("control indices 0..29, strides 0..1024 scalar components, and components 0..3"),
    inputs: |each| sampled(&[(0, 29), (0, 29), (0, 1024), (0, 1024), (0, 3)], 4096, 731, each),
    model: |a| {
        let index = crate::model::opengl::eval::control_index(arg(a, 0)?, arg(a, 1)?, arg(a, 2)?, arg(a, 3)?, arg(a, 4)?)?;
        one(i64::try_from(index).ok()?)
    },
}];
