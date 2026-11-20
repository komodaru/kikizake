//! The lines suite: pens, SetROP2, lines, polylines, polygons, rectangles, ellipses, arcs,
//! FillRect & co into DIB sections.
use super::*;

/// a black 8 bpp picture (default color table: white is entry 255, exact)
fn canvas(s: &mut String, k: u32, w: i64, h: i64) {
    writeln!(s, "dib {k} {w} {h} 8");
}

pub fn lines() -> String {
    let mut s = String::from("# kikizake suite lines: GDI pens, lines and filled shapes into DIB sections (SPEC.md). Made by `kikizake gen lines`.\n");
    // every 1 pixel line from the center of a 17x17 picture to every point of it, both ways
    for dir in ["out", "in"] {
        for ey in 0..17 {
            writeln!(s, "case line-{dir}-{ey}");
            for ex in 0..17 {
                canvas(&mut s, 0, 17, 17);
                writeln!(s, "pen 0 0 0 #ffffff");
                let (a, b) = if dir == "out" { ((8, 8), (ex, ey)) } else { ((ex, ey), (8, 8)) };
                writeln!(s, "moveto 0 {} {}\nlineto 0 {} {}\ngetpos 0\ndump 0", a.0, a.1, b.0, b.1);
            }
            writeln!(s, "end");
        }
    }
    // long lines of every slope class, clipped by the bitmap and by a clip rectangle
    let mut r = XorShift32::new(0x11e5_0001);
    for i in 0..60 {
        writeln!(s, "case long-{i}");
        canvas(&mut s, 0, 24, 20);
        if r.chance(3) {
            let (x0, y0) = (r.range(0, 12), r.range(0, 10));
            writeln!(s, "clip 0 {x0} {y0} {} {}", x0 + r.range(1, 12), y0 + r.range(1, 10));
        }
        writeln!(s, "pen 0 0 {} #ffffff", r.range(0, 1));
        writeln!(s, "moveto 0 {} {}", r.range(-30, 54), r.range(-30, 50));
        writeln!(s, "lineto 0 {} {}\ndump 0\nend", r.range(-30, 54), r.range(-30, 50));
    }
    // the 16 binary raster operations on every format
    for f in FORMATS {
        writeln!(s, "case rop2-{}", fmt_name(f));
        let (l, pal) = dib_line(&mut r, 0, 16, 16, f, false);
        writeln!(s, "{l}\nfill 0 {}", r.next());
        writeln!(s, "pen 0 0 0 #{:06x}", rcolor(&mut r, &pal));
        for m in 1..=16 {
            writeln!(s, "rop2 0 {m}\nmoveto 0 0 {}\nlineto 0 16 {}", m - 1, m - 1);
        }
        writeln!(s, "rop2 0 0\nrop2 0 17\ndump 0\nend");
    }
    // what SetROP2(0) leaves: a line drawn after it, then the mode it reports
    writeln!(s, "case rop2-zero\ndib 0 8 4 32\nfill 0 0x00808080\npen 0 0 0 #ff0000\nrop2 0 7\nrop2 0 0\nmoveto 0 0 1\nlineto 0 8 1\nrop2 0 13\nrop2 0 0\nrop2 0 13\ndump 0\nend");
    // pen styles, cosmetic: dashes and dots in both background modes, along each direction
    for st in 0..=8 {
        for bk in [1, 2] {
            writeln!(s, "case style-{st}-{bk}");
            writeln!(s, "dib 0 40 12 32\nbk 0 #0000ff\nbkmode 0 {bk}");
            if st == 7 {
                writeln!(s, "extpen 0 0x8 1 #ffffff"); // PS_COSMETIC | PS_ALTERNATE
            } else if st == 8 {
                writeln!(s, "extpen 0 0x2 1 #ffffff"); // cosmetic PS_DOT by ExtCreatePen
            } else {
                writeln!(s, "pen 0 {st} 1 #ffffff");
            }
            writeln!(s, "moveto 0 0 0\nlineto 0 40 0\nmoveto 0 39 2\nlineto 0 0 2\nmoveto 0 0 4\nlineto 0 7 11\nlineto 0 30 4\nlineto 0 39 11\ndump 0\nend");
        }
    }
    // wide pens: width, end caps and joins (geometric), cosmetic widths
    for (i, (st, w)) in [(0u32, 2), (0, 3), (0, 4), (0, 5), (6, 3), (0x10000, 3), (0x10100, 4), (0x10200, 5), (0x10000 | 0x2000, 4), (0x10000 | 0x1000, 4), (0x10100 | 0x2000, 5), (0x10200 | 0x1000, 6)].iter().enumerate() {
        writeln!(s, "case wide-{i}");
        canvas(&mut s, 0, 24, 24);
        if *st >= 0x10000 {
            writeln!(s, "extpen 0 {st:#x} {w} #ffffff");
        } else {
            writeln!(s, "pen 0 {st} {w} #ffffff");
        }
        writeln!(s, "polyline 0 3 3 18 4 5 19 20 20\nmoveto 0 2 12\nlineto 0 2 12\ndump 0\nend");
    }
    // rectangles: every size up to 6x6, with a pen and a brush, a null pen, a null brush
    for kind in ["both", "nopen", "nobrush", "inside"] {
        writeln!(s, "case rect-{kind}");
        for w in -1..=6 {
            for h in -1..=6 {
                canvas(&mut s, 0, 10, 10);
                match kind {
                    "nopen" => writeln!(s, "pen 0 null\nbrush 0 solid #ff0000"),
                    "nobrush" => writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 null"),
                    "inside" => writeln!(s, "pen 0 6 3 #ffffff\nbrush 0 solid #ff0000"),
                    _ => writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 solid #ff0000"),
                }
                writeln!(s, "rectangle 0 2 2 {} {}\ndump 0", 2 + w, 2 + h);
            }
        }
        writeln!(s, "end");
    }
    // ellipses, round rectangles: every size up to 13x13 (filled, then outlined)
    for kind in ["fill", "frame", "both"] {
        for h in 1..=13 {
            writeln!(s, "case ellipse-{kind}-{h}");
            for w in 1..=13 {
                canvas(&mut s, 0, 16, 16);
                match kind {
                    "fill" => writeln!(s, "pen 0 null\nbrush 0 solid #ffffff"),
                    "frame" => writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 null"),
                    _ => writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 solid #ff0000"),
                }
                writeln!(s, "ellipse 0 1 1 {} {}\ndump 0", 1 + w, 1 + h);
            }
            writeln!(s, "end");
        }
    }
    for i in 0..40 {
        writeln!(s, "case roundrect-{i}");
        canvas(&mut s, 0, 20, 20);
        writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 solid #ff0000");
        let (w, h) = (r.range(1, 18), r.range(1, 18));
        writeln!(s, "roundrect 0 1 1 {} {} {} {}\ndump 0\nend", 1 + w, 1 + h, r.range(0, 20), r.range(0, 20));
    }
    // arcs, pies, chords in both directions
    for i in 0..48 {
        let op = ["arc", "pie", "chord", "arcto"][i % 4];
        writeln!(s, "case {op}-{i}");
        canvas(&mut s, 0, 24, 24);
        writeln!(s, "pen 0 0 1 #ffffff\nbrush 0 solid #ff0000");
        if i / 4 % 2 == 1 {
            writeln!(s, "arcdir 0 1");
        }
        writeln!(s, "moveto 0 0 0");
        let (l, t) = (r.range(0, 6), r.range(0, 6));
        writeln!(s, "{op} 0 {l} {t} {} {} {} {} {} {}\ngetpos 0\ndump 0\nend", l + r.range(2, 18), t + r.range(2, 18), r.range(-5, 28), r.range(-5, 28), r.range(-5, 28), r.range(-5, 28));
    }
    // polygons in both fill modes, null pen
    for i in 0..60 {
        writeln!(s, "case polygon-{i}");
        canvas(&mut s, 0, 20, 20);
        writeln!(s, "pen 0 null\nbrush 0 solid #ffffff\npolyfill 0 {}", 1 + i % 2);
        let n = r.range(3, 7);
        let p: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(-2, 21), r.range(-2, 21))).collect();
        writeln!(s, "polygon 0 {}\ndump 0\nend", p.join(" "));
    }
    // polylines, Béziers and the current position
    for i in 0..30 {
        writeln!(s, "case poly-{i}");
        canvas(&mut s, 0, 20, 20);
        writeln!(s, "pen 0 0 0 #ffffff\nmoveto 0 {} {}", r.range(0, 19), r.range(0, 19));
        let op = ["polyline", "polylineto", "polybezier", "polybezierto"][i % 4];
        let n = match op {
            "polybezier" => 4 + 3 * r.range(0, 1),
            "polybezierto" => 3 + 3 * r.range(0, 1),
            _ => r.range(1, 6),
        };
        let p: Vec<String> = (0..n).map(|_| format!("{} {}", r.range(-2, 21), r.range(-2, 21))).collect();
        writeln!(s, "{op} 0 {}\ngetpos 0\ndump 0\nend", p.join(" "));
    }
    // FillRect, FrameRect, InvertRect with every rectangle around a 6x6 picture's edges
    for op in ["fillrect", "framerect", "invertrect"] {
        for i in 0..24 {
            writeln!(s, "case {op}-{i}\ndib 0 6 6 32\nfill 0 {}", 300 + i);
            match i % 6 {
                5 => writeln!(s, "brush 0 hatch {} #{:06x}", r.range(0, 5), r.next() & 0xffffff),
                _ => writeln!(s, "brush 0 solid #{:06x}", r.next() & 0xffffff),
            }
            let (l, t) = (r.range(-2, 6), r.range(-2, 6));
            writeln!(s, "{op} 0 {l} {t} {} {}\ndump 0\nend", l + r.range(-1, 6), t + r.range(-1, 6));
        }
    }
    // random: formats, colors, ROP2, shapes, clipping
    for i in 0..200 {
        writeln!(s, "case random-{i}");
        let f = *r.pick(FORMATS);
        let (w, h) = (r.range(4, 20), r.range(4, 16));
        let (l, pal) = dib_line(&mut r, 0, w, h, &f, false);
        writeln!(s, "{l}\nfill 0 {}", r.next());
        writeln!(s, "pen 0 0 {} #{:06x}\nbrush 0 solid #{:06x}", r.range(0, 1), rcolor(&mut r, &pal), rcolor(&mut r, &pal));
        if r.chance(2) {
            writeln!(s, "rop2 0 {}", r.range(1, 16));
        }
        if r.chance(4) {
            let (x0, y0) = (r.range(-2, w), r.range(-2, h));
            writeln!(s, "clip 0 {x0} {y0} {} {}", x0 + r.range(0, w + 2), y0 + r.range(0, h + 2));
        }
        for _ in 0..r.range(1, 3) {
            let (x0, y0, x1, y1) = (r.range(-3, w + 2), r.range(-3, h + 2), r.range(-3, w + 2), r.range(-3, h + 2));
            match r.range(0, 3) {
                0 => writeln!(s, "moveto 0 {x0} {y0}\nlineto 0 {x1} {y1}"),
                1 => writeln!(s, "rectangle 0 {x0} {y0} {x1} {y1}"),
                2 => writeln!(s, "ellipse 0 {x0} {y0} {x1} {y1}"),
                _ => writeln!(s, "polygon 0 {x0} {y0} {x1} {y1} {} {}", r.range(-3, w + 2), r.range(-3, h + 2)),
            }
        }
        writeln!(s, "dump 0\nend");
    }
    // Ellipse with the null pen over larger boxes, anywhere in a 64 pixel square (their own
    // stream): the filled pixels alone
    let mut r = XorShift32::new(0x0e11_f177);
    for i in 0..48 {
        let (x0, y0) = (r.range(0, 30), r.range(0, 30));
        let (x1, y1) = (x0 + r.range(2, 33), y0 + r.range(2, 33));
        writeln!(s, "case ellbig-{i}\ndib 0 64 64 8\npen 0 null\nbrush 0 solid #ffffff\nellipse 0 {x0} {y0} {x1} {y1}\ndump 0\nend");
    }
    s
}
