//! The models: what Windows prints for a case file, in the executor's own output format. A case
//! the model cannot answer prints "unmodeled <why>" in place of its lines (and is then compared
//! with Windows only).

pub mod d2d;
pub mod d3d11;
pub mod d3d12;
pub mod dxgi_format;
pub mod d3dres;
pub mod dxgifmt;
pub mod ddraw;
pub mod d3d9;
pub mod gdi;
pub mod gdiplus;
pub mod gdiline;
pub mod rgn;
pub mod gdirgn;
pub mod gdixform;
pub mod gdipath;
pub mod gditext;
pub mod gdiblend;
pub mod gdiddb;
pub mod wic;
pub mod dwrite;
pub mod wicimage;

use crate::case::Case;

#[derive(Debug, Clone, PartialEq)]
pub enum Fail {
    /// a line the executor would print "bad line N" for (the generators never write one)
    Bad,
    /// the clause (spec/*.txt, status open) that does not settle what Windows does here
    Unmodeled(&'static str),
}

pub type Res<T> = Result<T, Fail>;

/// one case's lines, or why the model has none
pub fn run_case(case: &Case) -> Res<Vec<String>> {
    let r = run_ops(case);
    if let Err(Fail::Unmodeled(id)) = &r {
        crate::spec::hit(id);
    }
    r
}

/// a model of one API: Ok(true) when it ran the line, Ok(false) when the line is not its own
pub trait Api {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool>;
}

impl Api for gdi::Gdi {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        gdi::Gdi::op(self, op, out)
    }
}
impl Api for d3d9::D3d9 {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d3d9::D3d9::op(self, op, out)
    }
}
impl Api for d3d11::D3d11 {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d3d11::D3d11::op(self, op, out)
    }
}
impl Api for d3dres::D3dres {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d3dres::D3dres::op(self, op, out)
    }
}
struct Wic;
impl Api for Wic {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        wic::op(op, out)
    }
}
impl Api for d3d12::D3d12 {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d3d12::D3d12::op(self, op, out)
    }
}
impl Api for wicimage::WicImage {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        wicimage::WicImage::op(self, op, out)
    }
}
impl Api for ddraw::Ddraw {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        ddraw::Ddraw::op(self, op, out)
    }
}

impl Api for gdiplus::Gdiplus {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        gdiplus::Gdiplus::op(self, op, out)
    }
}
impl Api for dwrite::Dwrite {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        dwrite::Dwrite::op(self, op, out)
    }
}

impl Api for d2d::D2d {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d2d::D2d::op(self, op, out)
    }
}

/// every API's model, fresh for a case, in the order a line is offered to them (one line each)
fn apis() -> Vec<Box<dyn Api>> {
    vec![
        Box::new(gdi::Gdi::new()),
        Box::new(d3d9::D3d9::default()),
        // (before d3d11: it sees device11 and leaves it to d3d11)
        Box::new(d3dres::D3dres::default()),
        Box::new(d3d11::D3d11::default()),
        Box::new(Wic),
        Box::new(ddraw::Ddraw::default()),
        Box::new(gdiplus::Gdiplus::default()),
        Box::new(dwrite::Dwrite::default()),
        Box::new(d3d12::D3d12::default()),
        Box::new(d2d::D2d::default()),
        Box::new(wicimage::WicImage::default()),
    ]
}

fn run_ops(case: &Case) -> Res<Vec<String>> {
    let mut out = Vec::new();
    let mut apis = apis();
    'line: for op in &case.ops {
        for a in apis.iter_mut() {
            if a.op(op, &mut out)? {
                continue 'line;
            }
        }
        return Err(Fail::Bad);
    }
    Ok(out)
}

/// the whole output for a case file of `suite`
pub fn run(suite: &str, cases: &[Case]) -> String {
    let mut s = format!("kikizake {suite}\n");
    for c in cases {
        s += &format!("case {}\n", c.name);
        match run_case(c) {
            Ok(lines) => lines.iter().for_each(|l| {
                s += l;
                s += "\n"
            }),
            Err(Fail::Bad) => s += "unmodeled bad line\n",
            Err(Fail::Unmodeled(why)) => s += &format!("unmodeled {why}\n"),
        }
    }
    s + "done\n"
}
