//! Direct3D 12 resources and DXGI swap chains: an executable specification (being written;
//! spec/d3d12.txt, spec/dxgi.txt).

use super::{Fail, Res};
use crate::case::Op;
use crate::spec::hit;

#[derive(Default)]
pub struct D3d12 {
    dev: bool,
    factory: bool,
}

impl D3d12 {
    pub fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        let t = op.tok.as_slice();
        match t[0].as_str() {
            "device12" => {
                hit("D3D12.DEVICE");
                self.dev = true;
                out.push("device12 ok".into());
                Ok(true)
            }
            "footprint12" | "alloc12" | "create12" | "clear12" | "cleardsv12" if !self.dev => {
                out.push("nodev".into());
                Ok(true)
            }
            "footprint12" => Err(Fail::Unmodeled("D3D12.FOOTPRINT")),
            "alloc12" => Err(Fail::Unmodeled("D3D12.ALLOC")),
            "create12" => Err(Fail::Unmodeled("D3D12.CREATE")),
            "clear12" | "cleardsv12" => Err(Fail::Unmodeled("D3D12.CLEAR")),
            "dxgifactory" => {
                self.factory = true;
                Err(Fail::Unmodeled("DXGI.FACTORY"))
            }
            "dxgiwarp" | "dxgiassoc" | "swapchain" | "scresize" | "scbuffer" | "scpresent" | "sclatency" => Err(Fail::Unmodeled("DXGI.SWAPCHAIN")),
            _ => Ok(false),
        }
    }
}
