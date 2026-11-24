//! GDI's flattening of a cubic Bézier: hybrid forward differencing in fixed point, the curve
//! cut where its distance from the chord exceeds 2/3 of a pixel (MEASURED, GDI.PATH.FLATTENHFD:
//! the points FlattenPath returns match those of the HFD32 flattener that dotnet/wpf's
//! WpfGfx/core/geometry/bezier.cpp documents as GDI's, BEZIER_FLATTEN_GDI_COMPATIBLE).

/// 28.4 to GDI's 18.14 (shift 10): a finer point steps through other values once a curve is
/// some hundreds of pixels long (region ellipse-size-9, ellipse-size-10)
const INITIAL_SHIFT: u32 = 10;
const GDI_INITIAL_SHIFT: u32 = 10;
/// 18.14 to 15.17
const ADDITIONAL_SHIFT: u32 = 3;
const SHIFT: u32 = INITIAL_SHIFT + ADDITIONAL_SHIFT;
const ROUND: i128 = 1 << (SHIFT - 1);
/// six times an error of 2/3 in 18.14, in the finer point
const TEST_INITIAL: i128 = (6 * 0x2aa0) << (INITIAL_SHIFT - GDI_INITIAL_SHIFT);
const TEST: i128 = TEST_INITIAL << ADDITIONAL_SHIFT;
/// the 32 bit flattener's reach: a control point 0x4000 (1024 pixels) or more from the bound
/// box's corner takes GDI's 64 bit one (clean-room note; region edge-5 differs with this one)
const MAX_SIZE: i128 = 0x4000 + 16;

#[derive(Clone, Copy, Debug)]
struct Basis {
    e0: i128,
    e1: i128,
    e2: i128,
    e3: i128,
}

impl Basis {
    fn new(p: [i128; 4]) -> Basis {
        Basis { e0: p[0] << INITIAL_SHIFT, e1: (p[3] - p[0]) << INITIAL_SHIFT, e2: (6 * (p[1] - 2 * p[2] + p[3])) << INITIAL_SHIFT, e3: (6 * (p[0] - 2 * p[1] + p[2])) << INITIAL_SHIFT }
    }
    fn error(&self) -> i128 {
        self.e2.abs().max(self.e3.abs())
    }
    fn parent_error_by4(&self) -> i128 {
        self.e3.abs().max((2 * self.e2 - self.e3).abs())
    }
    fn value(&self) -> i128 {
        (self.e0 + ROUND) >> SHIFT
    }
    fn lazy_halve(&mut self, shift: u32) {
        self.e2 = (self.e2 + self.e3) >> 1;
        self.e1 = (self.e1 - (self.e2 >> shift)) >> 1;
    }
    fn steady(&mut self, shift: u32) {
        self.e0 <<= ADDITIONAL_SHIFT;
        self.e1 <<= ADDITIONAL_SHIFT;
        if shift < ADDITIONAL_SHIFT {
            self.e2 <<= ADDITIONAL_SHIFT - shift;
            self.e3 <<= ADDITIONAL_SHIFT - shift;
        } else {
            self.e2 >>= shift - ADDITIONAL_SHIFT;
            self.e3 >>= shift - ADDITIONAL_SHIFT;
        }
    }
    fn halve(&mut self) {
        self.e2 = (self.e2 + self.e3) >> 3;
        self.e1 = (self.e1 - self.e2) >> 1;
        self.e3 >>= 2;
    }
    fn double(&mut self) {
        self.e1 += self.e1 + self.e2;
        self.e3 <<= 2;
        self.e2 = (self.e2 << 3) - self.e3;
    }
    fn step(&mut self) {
        self.e0 += self.e1;
        let t = self.e2;
        self.e1 += t;
        self.e2 += t - self.e3;
        self.e3 = t;
    }
}

/// the points (28.4) after the first that flatten the Bézier p (28.4), the last being p[3]'s;
/// large curves use a coarse subdivision before their final flattening
pub fn flatten(p: [(i32, i32); 4]) -> Option<Vec<(i32, i32)>> {
    Some(flatten_reporting_large(p, || {}))
}

pub(super) fn flatten_reporting_large(p: [(i32, i32); 4], on_large: impl FnOnce()) -> Vec<(i32, i32)> {
    // (relative to the bound box, one pixel loose, as the flattener works)
    let left = i128::from(p.iter().map(|q| q.0).min().unwrap_or(0)) - 16;
    let top = i128::from(p.iter().map(|q| q.1).min().unwrap_or(0)) - 16;
    let (px, py) = (p.map(|q| i128::from(q.0) - left), p.map(|q| i128::from(q.1) - top));
    if px.iter().chain(py.iter()).any(|&v| v >= MAX_SIZE) {
        on_large();
        return flatten_large(p);
    }
    let mut x = Basis::new(px);
    let mut y = Basis::new(py);
    let mut steps: u128 = 1;
    let mut shift = 0;
    while x.error() > TEST_INITIAL << shift || y.error() > TEST_INITIAL << shift {
        shift += 2;
        x.lazy_halve(shift);
        y.lazy_halve(shift);
        steps <<= 1;
    }
    x.steady(shift);
    y.steady(shift);
    x.step();
    y.step();
    steps -= 1;
    let mut out = Vec::new();
    loop {
        let at = |v: i128, o: i128| i32::try_from(v + o).unwrap_or(i32::MAX);
        out.push((at(x.value(), left), at(y.value(), top)));
        if steps == 0 {
            return out;
        }
        if x.error().max(y.error()) > TEST {
            x.halve();
            y.halve();
            steps <<= 1;
        }
        while steps & 1 == 0 && x.parent_error_by4() <= TEST >> 2 && y.parent_error_by4() <= TEST >> 2 {
            x.double();
            y.double();
            steps >>= 1;
        }
        steps -= 1;
        x.step();
        y.step();
    }
}


/// Subdivide in 36.28, rebuilding the controls in 28.4 between the two passes.
fn flatten_large(p: [(i32, i32); 4]) -> Vec<(i32, i32)> {
    type Point = [i128; 2];
    fn visit(p: [Point; 4], coarse: bool, out: &mut Vec<(i32, i32)>) {
        let limit = if coarse { 196608_i128 << 28 } else { 4_i128 << 32 };
        let bend = |a: Point, b: Point, c: Point| {
            a.into_iter().zip(b).zip(c).map(|((start, middle), end)| {
                start.saturating_sub(middle).saturating_add(end.saturating_sub(middle)).saturating_abs().saturating_mul(6)
            }).max().unwrap_or(0)
        };
        let round = |v: i128| v.saturating_add(1_i128 << 27) >> 28;
        if bend(p[0], p[1], p[2]).max(bend(p[1], p[2], p[3])) <= limit {
            if coarse {
                visit(p.map(|q| q.map(|v| round(v) << 28)), false, out);
            } else {
                let [x, y] = p[3].map(|v| i32::try_from(round(v)).unwrap_or(i32::MAX));
                out.push((x, y));
            }
            return;
        }
        let mid = |a: Point, b: Point| [a[0].saturating_add(b[0]) >> 1, a[1].saturating_add(b[1]) >> 1];
        let [a, b, c, d] = p;
        let (ab, bc, cd) = (mid(a,b), mid(b,c), mid(c,d));
        let (left, right) = (mid(ab,bc), mid(bc,cd));
        let center = mid(left,right);
        visit([a,ab,left,center], coarse, out);
        visit([center,right,cd,d], coarse, out);
    }
    let mut out = Vec::new();
    visit(p.map(|(x,y)| [i128::from(x) << 28, i128::from(y) << 28]), true, &mut out);
    out
}
