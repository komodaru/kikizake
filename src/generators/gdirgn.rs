//! The region suite: regions (rectangles, ellipses, round rectangles, polygons), CombineRgn,
//! their data, hit tests, and drawing and clipping with them.
use super::*;

fn rect(r: &mut XorShift32, lo: i64, hi: i64) -> String {
    let (a, b) = (r.range(lo, hi), r.range(lo, hi));
    let (c, d) = (a + r.range(-2, 10), b + r.range(-2, 10));
    format!("{a} {b} {c} {d}")
}

pub fn region() -> String {
    let mut s = String::from("# kikizake suite region: GDI regions, CombineRgn, region data, FillRgn & co, clip regions (SPEC.md). Made by `kikizake gen region`.\n");
    let mut r = XorShift32::new(0x4e61_0000);
    // CombineRgn: every mode on random rectangles and on the results again
    for i in 0..200 {
        writeln!(s, "case combine-{i}");
        for k in 0..3 {
            writeln!(s, "rgn {k} rect {}", rect(&mut r, -4, 12));
        }
        let mut made = vec![0, 1, 2];
        for _ in 0..r.range(1, 4) {
            let (d, a, b) = (r.range(0, 5), *r.pick(&made), *r.pick(&made));
            let m = r.range(1, 5);
            writeln!(s, "combine {d} {a} {b} {m}");
            if !made.contains(&d) {
                made.push(d);
            }
        }
        for k in made {
            writeln!(s, "rgndata {k}");
        }
        writeln!(s, "rgnbox 0\nend");
    }
    // empty, unordered and huge rectangles; SetRectRgn
    let edge = ["0 0 0 0", "5 5 1 1", "1 5 5 1", "3 3 3 9", "-5 -5 5 5", "-100000 -100000 100000 100000", "0 0 1 1", "-1 -1 0 0"];
    for (i, e) in edge.iter().enumerate() {
        writeln!(s, "case edge-{i}\nrgn 0 rect {e}\nrgndata 0\nrgnbox 0\nptin 0 0 0\nptin 0 4 4\nrectin 0 0 0 1 1\nsetrect 0 {e}\nrgndata 0\nrgn 1 ellipse {e}\nrgndata 1\nend");
    }
    // the shape of every elliptic region up to 16x16, and of round rectangles
    for h in 1..=16 {
        writeln!(s, "case ellipse-{h}");
        for w in 1..=16 {
            writeln!(s, "rgn 0 ellipse 2 3 {} {}\nrgndata 0", 2 + w, 3 + h);
        }
        writeln!(s, "end");
    }
    for i in 0..60 {
        writeln!(s, "case round-{i}");
        let (w, h) = (r.range(1, 20), r.range(1, 20));
        writeln!(s, "rgn 0 round 1 2 {} {} {} {}\nrgndata 0\nend", 1 + w, 2 + h, r.range(0, 24), r.range(0, 24));
    }
    // polygons in both fill modes
    for i in 0..120 {
        writeln!(s, "case poly-{i}");
        let n = r.range(3, 8);
        let p: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(-3, 16), r.range(-3, 16))).collect();
        writeln!(s, "rgn 0 poly {} {}\nrgndata 0\nend", 1 + i % 2, p.join(" "));
    }
    // hit tests, offsets, equality
    for i in 0..60 {
        writeln!(s, "case query-{i}");
        writeln!(s, "rgn 0 rect {}\nrgn 1 rect {}\ncombine 2 0 1 {}", rect(&mut r, -2, 8), rect(&mut r, -2, 8), r.range(1, 5));
        for _ in 0..6 {
            writeln!(s, "ptin 2 {} {}", r.range(-3, 18), r.range(-3, 18));
        }
        for _ in 0..4 {
            writeln!(s, "rectin 2 {}", rect(&mut r, -3, 14));
        }
        writeln!(s, "equal 2 0\nequal 0 0\nrgn 3 rect {}\nequal 2 3\noffsetrgn 2 {} {}\nrgndata 2\nrgnbox 2\nend", rect(&mut r, -2, 8), r.range(-9, 9), r.range(-9, 9));
    }
    // FillRgn, FrameRgn, InvertRgn, PaintRgn into every format
    for (i, f) in FORMATS.iter().enumerate() {
        for op in ["fillrgn", "framergn", "invertrgn", "paintrgn"] {
            writeln!(s, "case draw-{op}-{}", fmt_name(f));
            let (l, pal) = dib_line(&mut r, 0, 16, 12, f, false);
            writeln!(s, "{l}\nfill 0 {}\nbrush 0 solid #{:06x}", r.next(), rcolor(&mut r, &pal));
            writeln!(s, "rgn 0 rect {}\nrgn 1 rect {}\ncombine 0 0 1 {}", rect(&mut r, -2, 10), rect(&mut r, -2, 10), 2 + i % 3);
            if op == "framergn" {
                writeln!(s, "framergn 0 0 {} {}", r.range(1, 3), r.range(1, 3));
            } else {
                writeln!(s, "{op} 0 0");
            }
            writeln!(s, "dump 0\nend");
        }
    }
    // clip regions: SelectClipRgn / ExtSelectClipRgn in each mode, IntersectClipRect,
    // ExcludeClipRect, OffsetClipRgn, GetClipBox, GetClipRgn, PtVisible, RectVisible
    for i in 0..120 {
        writeln!(s, "case clip-{i}");
        writeln!(s, "dib 0 16 12 32\nfill 0 {}\nbrush 0 solid #{:06x}", r.next(), r.next() & 0xffffff);
        writeln!(s, "clipbox 0\ngetcliprgn 0 7");
        for _ in 0..r.range(1, 4) {
            match r.range(0, 5) {
                0 => writeln!(s, "rgn 0 rect {}\nselclip 0 0", rect(&mut r, -2, 12)),
                1 => writeln!(s, "rgn 0 rect {}\nselclip 0 0 {}", rect(&mut r, -2, 12), r.range(1, 5)),
                2 => writeln!(s, "clip 0 {}", rect(&mut r, -2, 12)),
                3 => writeln!(s, "exclude 0 {}", rect(&mut r, -2, 12)),
                4 => writeln!(s, "offsetclip 0 {} {}", r.range(-4, 4), r.range(-4, 4)),
                _ => writeln!(s, "selclip 0 none"),
            }
            writeln!(s, "clipbox 0");
        }
        writeln!(s, "getcliprgn 0 6\nrgndata 6\nptvisible 0 {} {}\nrectvisible 0 {}", r.range(-1, 16), r.range(-1, 12), rect(&mut r, -2, 12));
        writeln!(s, "patblt 0 0 0 16 12 0xf00021\ndump 0\nend");
    }
    // CombineRgn with a mode out of range
    for m in [0, 6, -1] {
        writeln!(s, "case combine-mode-{m}\nrgn 0 rect 0 0 4 4\nrgn 1 rect 2 2 6 6\nrgn 2 rect 9 9 10 10\ncombine 2 0 1 {m}\nrgndata 2\nend");
    }
    // the documented edge cases: selclip none with each mode, a null region selected
    for m in 1..=5 {
        writeln!(s, "case selclip-none-{m}\ndib 0 6 4 32\nclip 0 1 1 4 3\nselclip 0 none {m}\nclipbox 0\nend");
    }
    // ExtSelectClipRgn with a mode out of range, with and without a clip region
    for m in [0, 6, -1] {
        writeln!(s, "case selclip-mode-{m}\ndib 0 6 4 32\nrgn 0 rect 0 0 2 2\nselclip 0 0 {m}\nclipbox 0\nclip 0 1 1 4 3\nselclip 0 0 {m}\nclipbox 0\nend");
    }
    writeln!(s, "case selclip-null\ndib 0 6 4 32\nrgn 0 null\nselclip 0 0\nclipbox 0\nptvisible 0 0 0\nbrush 0 solid #ffffff\npatblt 0 0 0 6 4 0xf00021\ndump 0\nend");
    // the region is copied: changing it afterwards leaves the clip region
    writeln!(s, "case selclip-copy\ndib 0 6 4 32\nrgn 0 rect 1 1 3 3\nselclip 0 0\noffsetrgn 0 2 0\nclipbox 0\nbrush 0 solid #ffffff\npatblt 0 0 0 6 4 0xf00021\ndump 0\nend");
    // CreatePolygonRgn with a fill mode out of range
    for m in [0, 3, -1] {
        writeln!(s, "case poly-mode-{m}\nrgn 0 poly {m} 0 0 8 1 3 9\nrgndata 0\nend");
    }
    // the smallest and the largest elliptic regions (no empty box; the 64 bit flattener)
    for (i, b) in ["0 0 0 0", "3 3 3 3", "0 0 1 1", "0 0 2 1", "0 0 1 5", "0 0 0 5", "0 0 2 2", "0 0 3 3", "-3 -3 3 3", "0 0 2048 6", "0 0 1100 1100"].iter().enumerate() {
        writeln!(s, "case ellipse-size-{i}\nrgn 0 ellipse {b}\nrgndata 0\nrgnbox 0\nend");
    }
    // a bound below the last row of pixels: slivers through each converter, then copied,
    // offset and combined
    let slivers = [
        "1 0 0 9 4 9 5 0 0",
        "1 0 0 8 2 8 6 8 3 0 0",
        "2 0 0 6 6 7 9 6 6",
        "1 0 0 10 0 10 1 4 1 4 1 4 7",
    ];
    for (i, p) in slivers.iter().enumerate() {
        writeln!(s, "case loose-{i}\nrgn 0 poly {p}\nrgndata 0\nrgnbox 0\nrgn 1 null\ncombine 1 0 0 5\nrgndata 1\noffsetrgn 1 3 2\nrgndata 1\ncombine 1 0 0 2\nrgndata 1\nend");
    }
    // a sliver of more than 40 points
    let many: Vec<String> = (0..44).map(|i| format!("{} {}", i, if i < 22 { 0 } else { 8 - (i - 22) / 3 })).collect();
    writeln!(s, "case loose-many\nrgn 0 poly 1 {}\nrgndata 0\nend", many.join(" "));
    s
}
