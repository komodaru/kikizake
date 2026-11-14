//! WIC bitmap sources other than the format converter (runner/wicimage.c): not modelled yet.
use super::{Fail, Res};

const OPS: &[&str] = &["wbits", "wscale", "wflip", "wclip", "wconv", "wcolor", "wctx", "winfo", "wpixels", "wpalette", "wsrcpal", "wdecode", "wframe", "wencode", "wbytes", "wredecode"];

#[derive(Default)]
pub struct WicImage;

impl WicImage {
    pub fn op(&mut self, op: &crate::case::Op, _out: &mut Vec<String>) -> Res<bool> {
        if !OPS.contains(&op.tok[0].as_str()) {
            return Ok(false);
        }
        Err(Fail::Unmodeled("WICIMAGE.OPEN"))
    }
}
