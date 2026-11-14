//! Direct3D 10, 10.1 and 11 resources (SPEC.md "Direct3D 10 and 11 resources").
use super::{Fail, Res};
use crate::case::Op;
use crate::spec::hit;

#[derive(Default)]
pub struct D3dres {
    dev: bool,
}

const OPS: &[&str] = &["fmt", "tex", "buf", "srv", "rtv", "dsv", "uav", "clear", "cleards", "clearuav", "update", "copy", "copysub", "resolve", "read", "shader", "layout", "vtx", "draw", "target", "viewport", "scissor", "rast", "blend1", "depthst", "tsrv", "samp", "cbuf"];

impl D3dres {
    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        if t[0] == "device11" {
            self.dev = true;
            return Ok(false);
        }
        if t[0] == "device10" || t[0] == "device10_1" {
            self.dev = true;
            out.push(format!("{} ok", t[0]));
            return Ok(true);
        }
        if !OPS.contains(&t[0].as_str()) {
            return Ok(false);
        }
        if !self.dev {
            hit("D3DRES.NODEV");
            out.push("nodev".into());
            return Ok(true);
        }
        Err(Fail::Unmodeled("D3DRES.TODO"))
    }
}
