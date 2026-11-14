//! The models: what Windows prints for a case file, in the executor's own output format. A case
//! the model cannot answer prints "unmodeled <why>" in place of its lines (and is then compared
//! with Windows only).

pub mod d3d11;
pub mod ddraw;
pub mod d3d9;
pub mod gdi;
pub mod gdiplus;
pub mod wic;
pub mod dwrite;

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
struct Wic;
impl Api for Wic {
    fn op(&mut self, op: &crate::case::Op, out: &mut Vec<String>) -> Res<bool> {
        wic::op(op, out)
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

/// every API's model, fresh for a case, in the order a line is offered to them (one line each)
fn apis() -> Vec<Box<dyn Api>> {
    vec![
        Box::new(gdi::Gdi::new()),
        Box::new(d3d9::D3d9::default()),
        Box::new(d3d11::D3d11::default()),
        Box::new(Wic),
        Box::new(ddraw::Ddraw::default()),
        Box::new(gdiplus::Gdiplus::default()),
        Box::new(dwrite::Dwrite::default()),
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
