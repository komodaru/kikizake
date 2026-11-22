//! PolyBezier and PolyBezierTo in a path (spec/gdipath.txt GDI.PATH.BEZIER), exhaustive over
//! every sequence of up to three calls from MoveToEx, LineTo, CloseFigure and the two Bézier
//! calls, against the clauses' recording: a line starts a figure at the current position
//! (GDI.PATH.RECORD), a lone MoveToEx is replaced by the next (GDI.PATH.MOVETO), CloseFigure
//! marks a figure's last point (GDI.PATH.CLOSE, nothing without one: CLOSEEMPTY), EndPath drops
//! a MoveToEx left last (GDI.PATH.ENDMOVE); PolyBezier
//! begins a figure at its first point, PolyBezierTo continues from the current position and
//! leaves it at its last point (GDI.LINE.BEZIER's position rule), each point PT_BEZIERTO.
use kikizake::case::parse_cases;
use kikizake::model::run_case;

#[derive(Clone, Copy)]
enum Call {
    Move((i32, i32)),
    Line((i32, i32)),
    Close,
    Bezier(&'static [(i32, i32)]),
    BezierTo(&'static [(i32, i32)]),
}

const CALLS: [Call; 8] = [
    Call::Move((1, 2)),
    Call::Move((3, 0)),
    Call::Line((2, 2)),
    Call::Close,
    Call::Bezier(&[(0, 0), (1, 1), (2, 0), (3, 3)]),
    Call::Bezier(&[(5, 1), (4, 4), (0, 6), (2, 2), (7, 7), (1, 0), (6, 3)]),
    Call::BezierTo(&[(4, 4), (5, 0), (6, 6)]),
    Call::BezierTo(&[(1, 5), (5, 5), (0, 1), (3, 2), (2, 3), (4, 1)]),
];

fn text(c: Call) -> String {
    let pts = |p: &[(i32, i32)]| p.iter().map(|q| format!("{} {}", q.0, q.1)).collect::<Vec<_>>().join(" ");
    match c {
        Call::Move(p) => format!("moveto 0 {} {}", p.0, p.1),
        Call::Line(p) => format!("lineto 0 {} {}", p.0, p.1),
        Call::Close => "closefig 0".into(),
        Call::Bezier(p) => format!("polybezier 0 {}", pts(p)),
        Call::BezierTo(p) => format!("polybezierto 0 {}", pts(p)),
    }
}

/// the clauses' path (point, type, closed) and current position after the calls
fn reference(calls: &[Call]) -> Vec<String> {
    let mut path: Vec<((i32, i32), u8, bool)> = Vec::new();
    let mut pos = (0, 0);
    // (a line or PolyBezierTo starts a figure at the position when none is open)
    let start = |path: &mut Vec<((i32, i32), u8, bool)>, pos: (i32, i32)| {
        if path.last().is_none_or(|l| l.2) {
            path.push((pos, 6, false));
        }
    };
    for &c in calls {
        match c {
            Call::Move(p) => {
                if path.last().is_some_and(|l| l.1 == 6) {
                    path.pop();
                }
                path.push((p, 6, false));
                pos = p;
            }
            Call::Line(p) => {
                start(&mut path, pos);
                path.push((p, 2, false));
                pos = p;
            }
            Call::Close => {
                if let Some(l) = path.last_mut().filter(|l| l.1 != 6) {
                    l.2 = true;
                }
            }
            Call::Bezier(p) => {
                if path.last().is_some_and(|l| l.1 == 6) {
                    path.pop();
                }
                path.push((p[0], 6, false));
                path.extend(p[1..].iter().map(|q| (*q, 4, false)));
            }
            Call::BezierTo(p) => {
                start(&mut path, pos);
                path.extend(p.iter().map(|q| (*q, 4, false)));
                pos = p[p.len() - 1];
            }
        }
    }
    // (EndPath drops a MoveToEx left last, GDI.PATH.ENDMOVE)
    if path.last().is_some_and(|l| l.1 == 6) {
        path.pop();
    }
    let mut v = vec![format!("path {}", path.len())];
    v.extend(path.iter().map(|(q, t, c)| format!("pt {} {} {}", q.0, q.1, t | u8::from(*c))));
    v.push(format!("pos {} {}", pos.0, pos.1));
    v
}

fn model(calls: &[Call]) -> Vec<String> {
    let body: Vec<String> = calls.iter().map(|c| text(*c)).collect();
    let t = format!("case bez\ndib 0 8 8 8\nbeginpath 0\n{}\nendpath 0\ngetpath 0\ngetpos 0\nend\n", body.join("\n"));
    let c = parse_cases(&t);
    let out = run_case(&c[0]).unwrap_or_else(|e| panic!("{t}: {e:?}"));
    // (getpath's lines and the position getpos answers)
    let at = out.iter().rposition(|l| l.starts_with("path ")).unwrap();
    out[at..].to_vec()
}

/// GDI.PATH.BEZIER: every sequence of up to three of the calls
#[test]
fn bezier_calls_record_as_the_clauses_say() {
    let mut seqs: Vec<Vec<Call>> = vec![vec![]];
    for _ in 0..3 {
        let next: Vec<Vec<Call>> = seqs.iter().filter(|s| s.len() == seqs.last().unwrap().len()).flat_map(|s| CALLS.iter().map(move |c| [s.clone(), vec![*c]].concat())).collect();
        seqs.extend(next);
    }
    assert_eq!(seqs.len(), 1 + 8 + 64 + 512);
    for s in &seqs {
        let body: Vec<String> = s.iter().map(|c| text(*c)).collect();
        assert_eq!(model(s), reference(s), "{body:?}");
    }
}
