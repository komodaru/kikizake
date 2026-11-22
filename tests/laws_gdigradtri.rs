//! GradientFill's triangles in every format (spec/gdiblend.txt GDI.GRAD.TRIANGLE): a pixel's
//! color is its 16 bit value by GDI.GRAD.TRIROUND (the barycentric mix rounded halves up,
//! vertex channels above 0xff00 as 0xff00) carried into the format as a rectangle gradient
//! carries a flat 16 bit value: its high byte in 32 and 24 bpp, GDI.GRAD.DITHER16 in 16 bpp,
//! GDI.GRAD.INDEXED in 1, 4 and 8 bpp. Exhaustive over triangles on a grid: the triangle's
//! bitmap is the bitmap of one-pixel GRADIENT_FILL_RECT_H fills of those values at the pixels
//! the triangle covers (the rows of GDI.GRAD.TRIROUND, read from a 32 bpp fill on two
//! backgrounds).
use kikizake::case::parse_cases;
use kikizake::model::run_case;

type V = (i64, i64, [i64; 4]);

fn run(text: &str) -> Result<Vec<String>, String> {
    run_case(&parse_cases(&format!("case grad\n{text}\nend\n"))[0]).map_err(|e| format!("{e:?}"))
}

fn tri(v: &[V; 3]) -> String {
    let p: Vec<String> = v.iter().map(|q| format!("{} {} {} {} {} {}", q.0, q.1, q.2[0], q.2[1], q.2[2], q.2[3])).collect();
    format!("gradtri 0 {}", p.join(" "))
}

/// the pixels the triangle covers: those a 32 bpp fill writes alike over black and white
fn covered(v: &[V; 3]) -> Vec<(i64, i64)> {
    let a = run(&format!("dib 0 6 5 32\n{}\ndump 0", tri(v))).unwrap();
    let b = run(&format!("dib 0 6 5 32\npatblt 0 0 0 6 5 0xff0062\n{}\ndump 0", tri(v))).unwrap();
    let rows = |o: &[String]| -> Vec<String> { let at = o.iter().rposition(|l| l.starts_with("px ")).unwrap(); o[at + 1..].to_vec() };
    let (ra, rb) = (rows(&a), rows(&b));
    let mut out = Vec::new();
    for (y, (la, lb)) in ra.iter().zip(&rb).enumerate() {
        for x in 0..6 {
            if la[8 * x..8 * x + 8] == lb[8 * x..8 * x + 8] {
                out.push((i64::try_from(x).unwrap(), i64::try_from(y).unwrap()));
            }
        }
    }
    out
}

/// GDI.GRAD.TRIROUND: channel i at (x, y), the barycentric mix (Cramer's rule over the
/// triangle's determinant) rounded halves up
fn value(v: &[V; 3], x: i64, y: i64, i: usize) -> i64 {
    let c = |k: usize| if i < 3 { v[k].2[i].min(0xff00) } else { v[k].2[i] };
    let (a, b, d) = (v[0], v[1], v[2]);
    let det = (b.0 - a.0) * (d.1 - a.1) - (d.0 - a.0) * (b.1 - a.1);
    // (the weights of b and d: the areas of a p d and a b p)
    let wb = (x - a.0) * (d.1 - a.1) - (d.0 - a.0) * (y - a.1);
    let wd = (b.0 - a.0) * (y - a.1) - (x - a.0) * (b.1 - a.1);
    let (n, q) = (c(0) * (det - wb - wd) + c(1) * wb + c(2) * wd, det);
    let (n, q) = if q < 0 { (-n, -q) } else { (n, q) };
    (2 * n + q).div_euclid(2 * q)
}

/// every triangle with corners on the grid, two vertex color sets (one past 0xff00), every
/// format
#[test]
fn triangle_is_its_values_carried_as_flat_rectangles() {
    let g = [(0, 0), (5, 0), (0, 4), (5, 4), (2, 1), (3, 3), (-1, 2), (6, 3)];
    let colors = [
        [[0xff00, 0x4000, 0x0000, 0xff00], [0x0000, 0xc000, 0x8000, 0x8000], [0x1234, 0x7777, 0xffff, 0x0000]],
        [[0xffff, 0x5555, 0xfff0, 0x2000], [0x8080, 0xbfff, 0x0100, 0xffff], [0x0001, 0x4001, 0x7f7f, 0x1000]],
    ];
    let formats = ["32", "24", "16", "16 565", "8", "4", "1"];
    let mut checked = 0;
    for &p0 in &g {
        for &p1 in &g {
            for &p2 in &g {
                for cs in &colors {
                    let v: [V; 3] = [(p0.0, p0.1, cs[0]), (p1.0, p1.1, cs[1]), (p2.0, p2.1, cs[2])];
                    let det = (p1.0 - p0.0) * (p2.1 - p0.1) - (p2.0 - p0.0) * (p1.1 - p0.1);
                    if det == 0 {
                        continue;
                    }
                    let px = covered(&v);
                    for f in formats {
                        let want: Vec<String> = px.iter().map(|&(x, y)| {
                            let c: Vec<String> = (0..4).map(|i| value(&v, x, y, i).to_string()).collect();
                            let c = c.join(" ");
                            format!("gradrect 0 0 {x} {y} {c} {} {} {c}", x + 1, y + 1)
                        }).collect();
                        let got = run(&format!("dib 0 6 5 {f}\n{}\ndump 0", tri(&v)));
                        let flat = run(&format!("dib 0 6 5 {f}\n{}\ndump 0", want.join("\n")));
                        let dump = |r: Result<Vec<String>, String>| r.map(|o| { let at = o.iter().rposition(|l| l.starts_with("px ")).unwrap(); o[at..].to_vec() });
                        assert_eq!(dump(got), dump(flat), "{v:?} format {f}");
                        checked += 1;
                    }
                }
            }
        }
    }
    assert!(checked > 2000, "{checked}");
}
