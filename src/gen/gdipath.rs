//! The path suite: paths of every shape, GetPath, FlattenPath, WidenPath, PathToRegion,
//! StrokePath / FillPath, SelectClipPath.
use super::*;

const SHAPES: &[&str] = &[
    "moveto 0 2 3\nlineto 0 9 4\nlineto 0 5 11",
    "moveto 0 2 3\nlineto 0 9 4\nlineto 0 5 11\nclosefig 0",
    "rectangle 0 2 3 12 9",
    "rectangle 0 12 9 2 3",
    "ellipse 0 2 3 12 9",
    "ellipse 0 1 1 4 4",
    "roundrect 0 2 3 14 11 5 4",
    "arc 0 2 3 14 11 14 3 2 11",
    "arcto 0 2 3 14 11 14 3 2 11",
    "pie 0 2 3 14 11 14 3 2 11",
    "chord 0 2 3 14 11 14 3 2 11",
    "polyline 0 1 1 8 2 3 9 12 12",
    "polygon 0 1 1 8 2 3 9 12 12",
    "moveto 0 1 1\npolybezier 0 1 1 9 0 13 12 2 12",
    "moveto 0 1 1\npolybezierto 0 9 0 13 12 2 12",
    "moveto 0 1 1\npolylineto 0 5 5 9 1",
    "moveto 0 1 1\nlineto 0 5 5\nmoveto 0 8 1\nlineto 0 12 7\nlineto 0 8 9\nclosefig 0",
];

pub fn path() -> String {
    let mut s = String::from("# kikizake suite path: GDI paths (SPEC.md). Made by `kikizake gen path`.\n");
    let mut r = XorShift32::new(0x9a7b_0000);
    for (i, sh) in SHAPES.iter().enumerate() {
        for dir in [2, 1] {
            writeln!(s, "case shape-{i}-{dir}\ndib 0 16 16 8\npen 0 0 1 #ffffff\nbrush 0 solid #ff0000\narcdir 0 {dir}\nbeginpath 0\n{sh}\nendpath 0\ngetpath 0").unwrap();
            writeln!(s, "flatten 0\ngetpath 0\npathrgn 0 0\nrgndata 0\nend").unwrap();
        }
        for op in ["strokepath", "fillpath", "strokefill"] {
            writeln!(s, "case draw-{i}-{op}\ndib 0 16 16 8\npen 0 0 1 #ffffff\nbrush 0 solid #ff0000\npolyfill 0 {}\nbeginpath 0\n{sh}\nendpath 0\n{op} 0\ngetpath 0\ndump 0\nend", 1 + i % 2).unwrap();
        }
        writeln!(s, "case clip-{i}\ndib 0 16 16 8\nfill 0 {}\nbeginpath 0\n{sh}\nendpath 0\nselclippath 0 {}\nclipbox 0\nbrush 0 solid #ffffff\npatblt 0 0 0 16 16 0xf00021\ndump 0\nend", r.next(), r.range(1, 5)).unwrap();
    }
    // WidenPath with geometric pens
    for (i, (st, w)) in [(0u32, 1), (0, 3), (0x10000, 3), (0x10100, 4), (0x10200, 3), (0x11000, 5), (0x12000, 5)].iter().enumerate() {
        for k in [2, 11, 4] {
            writeln!(s, "case widen-{i}-{k}\ndib 0 16 16 8").unwrap();
            if *st >= 0x10000 {
                writeln!(s, "extpen 0 {st:#x} {w} #ffffff").unwrap();
            } else {
                writeln!(s, "pen 0 {st} {w} #ffffff").unwrap();
            }
            writeln!(s, "beginpath 0\n{}\nendpath 0\nwiden 0\ngetpath 0\nend", SHAPES[k]).unwrap();
        }
    }
    // the path's state machine: calls out of order
    for (i, c) in ["getpath 0\nendpath 0\nflatten 0\nstrokepath 0\nfillpath 0\npathrgn 0 1\nselclippath 0 5\nabortpath 0\nclosefig 0",
        "beginpath 0\ngetpath 0\nmoveto 0 1 1\nlineto 0 4 4\ngetpath 0\nfillpath 0\nabortpath 0\ngetpath 0\nendpath 0",
        "beginpath 0\nmoveto 0 1 1\nlineto 0 4 4\nbeginpath 0\nlineto 0 2 5\nendpath 0\ngetpath 0",
        "beginpath 0\nendpath 0\ngetpath 0\npathrgn 0 1\nfillpath 0\ngetpath 0",
        "beginpath 0\nclosefig 0\nmoveto 0 3 3\nclosefig 0\nlineto 0 5 3\nclosefig 0\nclosefig 0\nendpath 0\ngetpath 0",
        "beginpath 0\nmoveto 0 1 1\nmoveto 0 2 2\nlineto 0 5 2\nmoveto 0 6 6\nendpath 0\ngetpath 0\nstrokepath 0\ngetpath 0"].iter().enumerate() {
        writeln!(s, "case order-{i}\ndib 0 8 8 8\npen 0 0 1 #ffffff\n{c}\ndump 0\nend").unwrap();
    }
    writeln!(s, "case miter\ndib 0 8 8 8\nmiter 0 3\nmiter 0 0.5\nmiter 0 20\nend").unwrap();
    // random polygons and polylines, through a region and drawn
    for i in 0..60 {
        writeln!(s, "case random-{i}\ndib 0 20 20 8\npen 0 0 1 #ffffff\nbrush 0 solid #ff0000\npolyfill 0 {}\nbeginpath 0", 1 + i % 2).unwrap();
        for _ in 0..r.range(1, 3) {
            let n = r.range(2, 6);
            let p: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(-2, 22), r.range(-2, 22))).collect();
            writeln!(s, "{} 0 {}", if r.chance(2) { "polygon" } else { "polyline" }, p.join(" ")).unwrap();
        }
        writeln!(s, "endpath 0\ngetpath 0\n{} 0\ndump 0\nend", ["fillpath", "strokepath", "strokefill"][r.range(0, 2) as usize]).unwrap();
    }
    s
}
