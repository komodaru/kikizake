//! PolyBezier and PolyBezierTo outside a path (spec/gdiline.txt GDI.LINE.BEZIER): the curve
//! flattened as in a path and joined by 28.4 cosmetic lines is what StrokePath draws of the
//! same curve recorded in a path (GDI.PATH.FLATTENHFD, GDI.LINE.GIQ); exhaustive over curves
//! whose four points lie on a grid, and PolyBezierTo leaves the position at its last point.
use kikizake::case::parse_cases;
use kikizake::model::run_case;

fn run(body: &str) -> Vec<String> {
    let t = format!("case pb\ndib 0 12 12 8\npen 0 0 1 #ff0000\n{body}\ndump 0\ngetpos 0\nend\n");
    let c = parse_cases(&t);
    run_case(&c[0]).unwrap_or_else(|e| panic!("{body}: {e:?}"))
}

/// the dumped pixels and the position (the lines after the last call's own answer)
fn tail(v: &[String]) -> Vec<String> {
    let at = v.iter().rposition(|l| l.starts_with("px ")).unwrap();
    v[at..].to_vec()
}

/// GDI.LINE.BEZIER: every curve with its four points on the grid {1, 5, 10}², drawn by
/// PolyBezier and by PolyBezierTo from its first point, against StrokePath of it
#[test]
fn poly_bezier_draws_what_stroke_path_draws() {
    let g: Vec<(i32, i32)> = [1, 5, 10].iter().flat_map(|&x| [1, 5, 10].iter().map(move |&y| (x, y))).collect();
    for &a in &g {
        for &b in &g {
            for &c in &g {
                for &d in &g {
                    let pts = format!("{} {} {} {} {} {} {} {}", a.0, a.1, b.0, b.1, c.0, c.1, d.0, d.1);
                    let rest = format!("{} {} {} {} {} {}", b.0, b.1, c.0, c.1, d.0, d.1);
                    let stroked = run(&format!("beginpath 0\npolybezier 0 {pts}\nendpath 0\nstrokepath 0"));
                    let drawn = run(&format!("polybezier 0 {pts}"));
                    // (StrokePath leaves the position where it was: PolyBezier does not move it)
                    assert_eq!(tail(&drawn), tail(&stroked), "polybezier {pts}");
                    let to = run(&format!("moveto 0 {} {}\npolybezierto 0 {rest}", a.0, a.1));
                    let mut want = tail(&stroked);
                    if let Some(last) = want.last_mut() {
                        *last = format!("pos {} {}", d.0, d.1);
                    }
                    assert_eq!(tail(&to), want, "polybezierto from {a:?}: {rest}");
                }
            }
        }
    }
}
