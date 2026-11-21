//! Laws of GDI+ path regions (src/model/gpregion.rs): RegionToPath gives back the same aliased
//! pixels whatever the set, an Inset stroke's outline is the examples the clean-room notes
//! measured in GDI+ itself, and a set's figures come in scan order.
use kikizake::model::gdiplus::{FillMode, Round};
use kikizake::model::gppath::{Path, Pen};
use kikizake::model::gpregion::{pixels, points, to_path};
use kikizake::model::gpwiden as wd;
use proptest::prelude::*;

const ID: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

/// the pixel set of a list of cells, through a path of unit squares
fn cells(r: Round, c: &[(i64, i64)]) -> kikizake::model::gpregion::Pixels {
    let mut p = Path::new(1);
    for &(x, y) in c {
        p.add_rectangle(r, (x as f32, y as f32, 1.0, 1.0)).unwrap();
    }
    pixels(r, &p, &ID, FillMode::Winding).unwrap()
}

fn inset(r: Round, rect: (f32, f32, f32, f32), w: f32) -> Vec<Vec<(f32, f32)>> {
    let mut p = Path::new(0);
    p.add_rectangle(r, rect).unwrap();
    let mut pen = Pen::new(0xff00_0000, w);
    pen.align = 1;
    let o = wd::widen(r, &p, &pen, &ID, 0.25).unwrap().unwrap();
    o.figures().iter().map(|f| f.iter().map(|q| (q.x, q.y)).collect()).collect()
}

#[test]
fn inset_outlines_as_measured() {
    for r in [Round::Down, Round::Nearest] {
        assert_eq!(inset(r, (3.25, 5.0, 7.25, 8.25), 3.0), vec![vec![(4.0, 5.0), (4.0, 14.0), (11.0, 14.0), (11.0, 5.0)], vec![(8.0, 8.0), (7.0, 11.0), (7.0, 8.0)]]);
        assert_eq!(inset(r, (3.0, 3.0, 3.0, 1.0), 3.0), vec![vec![(3.0, 3.0), (4.0, 4.0), (4.0, 3.0)], vec![(5.0, 3.0), (6.0, 4.0), (6.0, 3.0)]]);
        assert_eq!(inset(r, (3.0, 3.0, 9.0, 9.0), 3.0), vec![vec![(3.0, 3.0), (3.0, 12.0), (12.0, 12.0), (12.0, 3.0)], vec![(9.0, 6.0), (9.0, 9.0), (6.0, 9.0), (6.0, 6.0)]]);
    }
}

#[test]
fn holes_diagonalize_as_measured() {
    let r = Round::Down;
    let ring = |hole: &[(i64, i64)]| -> Vec<(i64, i64)> { (2..10).flat_map(|y| (2..10).map(move |x| (x, y))).filter(|c| !hole.contains(c)).collect() };
    let hole = |h: &[(i64, i64)]| to_path(&cells(r, &ring(h)))[1].clone();
    assert_eq!(hole(&[(5, 5)]), vec![(6, 5), (6, 6), (5, 5)]);
    assert_eq!(hole(&[(5, 5), (5, 6)]), vec![(6, 5), (5, 7), (5, 5)]);
    assert_eq!(hole(&[(5, 5), (6, 5)]), vec![(7, 5), (7, 6), (5, 5)]);
    assert_eq!(to_path(&cells(r, &[])), vec![vec![(0, 0); 4]]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]
    /// any pixel set's figures, filled aliased with Winding, are that pixel set again, and come
    /// in the scan order of their top-left points
    #[test]
    fn region_to_path_keeps_the_pixels(bits in proptest::collection::vec(any::<bool>(), 36), down: bool) {
        let r = if down { Round::Down } else { Round::Nearest };
        let c: Vec<(i64, i64)> = (0..36).filter(|&i| bits[i]).map(|i| (i as i64 % 6 + 1, i as i64 / 6 + 1)).collect();
        let px = cells(r, &c);
        let figs = to_path(&px);
        let back = pixels(r, &Path::of(1, points(&figs)), &ID, FillMode::Winding).unwrap();
        for y in -1..9 {
            for x in -1..9 {
                prop_assert_eq!(back.has(x, y), c.contains(&(x, y)), "pixel {} {}", x, y);
            }
        }
        let tl: Vec<(i64, i64)> = figs.iter().map(|f| f.iter().map(|&(x, y)| (y, x)).min().unwrap()).collect();
        prop_assert!(tl.windows(2).all(|w| w[0] <= w[1]));
    }
}
