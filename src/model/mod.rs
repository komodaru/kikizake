//! The models: what Windows prints for a case file, in the executor's own output format. A case
//! the model cannot answer prints "unmodeled <why>" in place of its lines (and is then compared
//! with Windows only).

pub mod d2d;
pub mod d3d11;
pub mod d3d12;
pub mod dxgi_format;
pub mod d3draster;
pub mod d3dres;
pub mod bptc;
pub mod d3dres_check;
pub mod d3dfmt_table;
pub mod dxgiconv;
pub mod dxgifmt;
pub mod ddraw;
pub mod d3d7;
pub mod d3d9;
pub mod d3d9draw;
pub mod d3dx9;
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
pub mod opengl;
pub mod wic;
pub mod dwrite;
pub mod wicimage;

use crate::case::Case;

#[derive(Debug, Clone, PartialEq)]
pub enum Fail {
    /// a line the executor would print "bad line N" for (the generators never write one)
    Bad,
    /// an op on an empty slot (a bitmap that could not be selected): the executor prints
    /// "bad line N" and goes on to the next line
    Empty,
    /// the clause (spec/*.txt, status open) that does not settle what Windows does here
    Unmodeled(crate::spec::OpenId),
    /// the executor dies on this line (a crash inside Windows' code): the lines before it are
    /// the case's, and no later case runs
    Crash,
}

/// how the executor leaves a case
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    /// it goes on to the next case
    Next,
    /// it died on the case's last printed line
    Crash,
}

pub type Res<T> = Result<T, Fail>;

thread_local! {
    static ARCH: std::cell::Cell<u32> = const { std::cell::Cell::new(32) };
}

/// the executor the model answers for: 32 (x86) or 64 (x86-64); a few calls differ by it
/// (DirectDraw's system-memory pitches, Direct3D 7 only on x86). The default is 32.
pub fn set_arch(a: u32) {
    ARCH.with(|c| c.set(a));
}
pub fn arch() -> u32 {
    ARCH.with(|c| c.get())
}

/// one case's lines, or why the model has none
pub fn run_case(case: &Case) -> Res<Vec<String>> {
    run_case_ended(case).0
}

/// one case's lines, or why the model has none, and whether the executor lives on
pub fn run_case_ended(case: &Case) -> (Res<Vec<String>>, Ended) {
    let (r, e) = run_ops(case);
    if let Err(Fail::Unmodeled(id)) = &r {
        crate::spec::hit((*id).into());
    }
    (r, e)
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
impl Api for opengl::Gl {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        opengl::Gl::op(self, op, out)
    }
}
impl Api for ddraw::Ddraw {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        ddraw::Ddraw::op(self, op, out)
    }
}

impl Api for d3d7::D3d7 {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        d3d7::D3d7::op(self, op, out)
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
        Box::new(d3d7::D3d7::default()),
        Box::new(gdiplus::Gdiplus::default()),
        Box::new(dwrite::Dwrite::default()),
        Box::new(d3d12::D3d12::default()),
        Box::new(d2d::D2d::default()),
        Box::new(wicimage::WicImage::default()),
        Box::new(d3dx9::D3dx9),
        Box::new(opengl::Gl::default()),
    ]
}

fn run_ops(case: &Case) -> (Res<Vec<String>>, Ended) {
    let mut out = Vec::new();
    let mut apis = apis();
    'line: for op in &case.ops {
        for a in apis.iter_mut() {
            match a.op(op, &mut out) {
                Ok(true) => continue 'line,
                Ok(false) => {}
                Err(Fail::Empty) => {
                    out.push(format!("bad line {}", op.line));
                    continue 'line;
                }
                Err(Fail::Crash) => return (Ok(out), Ended::Crash),
                Err(e @ (Fail::Bad | Fail::Unmodeled(_))) => return (Err(e), Ended::Next),
            }
        }
        return (Err(Fail::Bad), Ended::Next);
    }
    (Ok(out), Ended::Next)
}

/// the whole output for a case file of `suite`
pub fn run(suite: &str, cases: &[Case]) -> String {
    let mut s = format!("kikizake {suite}\n");
    for c in cases {
        s += &format!("case {}\n", c.name);
        let (r, ended) = run_case_ended(c);
        match r {
            Ok(lines) => lines.iter().for_each(|l| {
                s += l;
                s += "\n"
            }),
            Err(Fail::Bad | Fail::Empty) => s += "unmodeled bad line\n",
            Err(Fail::Unmodeled(why)) => s += &format!("unmodeled {why}\n"),
            Err(Fail::Crash) => unreachable!("run_ops answers a crash as its lines"),
        }
        match ended {
            Ended::Next => {}
            // (the executor's output ends there, without "done")
            Ended::Crash => return s,
        }
    }
    s + "done\n"
}
