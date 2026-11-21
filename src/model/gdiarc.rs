//! GDI's arcs in 28.4 (GDI.PATH.ARC): Arc, ArcTo, Pie and Chord as win32k builds them, in
//! float32 with its sine and arctangent tables.
//!
//! Sources: Microsoft Learn ("Arc", "ArcTo", "Pie", "Chord", "SetArcDirection") and the
//! clean-room notes on the runner's win32kfull (a behavioural spec, no code). Every rule is
//! confirmed by the goldens named in spec/gdipath.txt.

/// a point on the unit circle (x right, y up), in float32 as GDI keeps it
#[derive(Clone, Copy, Debug, PartialEq)]
struct Unit {
    x: f32,
    y: f32,
}

/// an angle in degrees, in float32
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
struct Deg(f32);

/// the quadrant an angle's point lies in (0: x > 0 and y >= 0, counterclockwise on)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Quad(u8);

impl Quad {
    fn next(self) -> Quad {
        Quad((self.0 + 1) & 3)
    }
    /// the axis point and angle the quadrant begins at
    fn axis(self) -> (Unit, Deg) {
        match self.0 & 3 {
            0 => (Unit { x: 1.0, y: 0.0 }, Deg(0.0)),
            1 => (Unit { x: 0.0, y: 1.0 }, Deg(90.0)),
            2 => (Unit { x: -1.0, y: 0.0 }, Deg(180.0)),
            _ => (Unit { x: 0.0, y: -1.0 }, Deg(270.0)),
        }
    }
}

/// truncation toward zero
fn trunc(x: f32) -> i32 {
    x as i32
}

/// the fraction of x (x itself below 1, 0 from 2^23 on)
fn frac(x: f32) -> f32 {
    if x.abs() < 1.0 {
        x
    } else if x.abs() >= 8_388_608.0 {
        0.0
    } else {
        x - trunc(x.abs()) as f32
    }
}

/// sin(k pi / 64), k = 0..32
fn sin_table() -> [f32; 33] {
    std::array::from_fn(|k| (k as f64 * std::f64::consts::PI / 64.0).sin() as f32)
}

/// atan(k / 32) in degrees, k = 0..32, and a 0 after it
fn atan_table() -> [f32; 34] {
    std::array::from_fn(|k| if k > 32 { 0.0 } else { (k as f64 / 32.0).atan().to_degrees() as f32 })
}

const SINE_FACTOR: f32 = 32.0 / 90.0;

/// the table's cosine and sine of an angle: 32 steps a quarter, linear between them
fn cos_sin(a: Deg) -> Unit {
    let t = sin_table();
    let neg = a.0 < 0.0;
    let v = SINE_FACTOR * a.0.abs();
    let i = trunc(v);
    let f = frac(v);
    let (k, j) = (i >> 5, (i & 31) as usize);
    let at = |k: i32| {
        if k & 1 == 0 {
            (t[j + 1] - t[j]) * f + t[j]
        } else {
            t[32 - j] - (t[32 - j] - t[31 - j]) * f
        }
    };
    let mut s = at(k);
    if neg != (k & 2 != 0) {
        s = -s;
    }
    let mut c = at(k + 1);
    if (k + 1) & 2 != 0 {
        c = -c;
    }
    Unit { x: c, y: s }
}

/// the cosine of an angle by the table (the sine 90 degrees on)
fn cos(a: Deg) -> f32 {
    cos_sin(Deg(a.0 + 90.0)).y
}

/// the series' cosine and sine, for sweeps under 3 degrees
fn cos_sin_precise(a: Deg) -> Unit {
    const PI: f32 = f32::from_bits(0x4049_0FDA);
    let neg = a.0 < 0.0;
    let mut r = frac(a.0.abs() / 360.0) * 360.0;
    let (mut sn, mut cn) = (false, false);
    if 180.0 - r < 0.0 {
        r = 360.0 - r;
        sn = true;
    }
    if 90.0 - r < 0.0 {
        r = 180.0 - r;
        cn = true;
    }
    let x = (PI * r) / 180.0;
    let (mut s, mut c, mut term, mut den, mut n) = (x, 1.0f32, x, 2.0f32, 2.0f32);
    for e in 2..=12u32 {
        term *= x;
        let mut v = term / den;
        if e & 2 != 0 {
            v = -v;
        }
        if e & 1 == 1 {
            s += v;
        } else {
            c += v;
        }
        n += 1.0;
        den *= n;
    }
    if neg != sn {
        s = -s;
    }
    if cn {
        c = -c;
    }
    Unit { x: c, y: s }
}

/// the table's angle of (x, y) in [0, 360] and its quadrant (from the signs only)
fn arctan(x: f32, y: f32) -> (Deg, Quad) {
    let t = atan_table();
    let (mut x, mut y, mut code) = (x, y, 0u8);
    if x < 0.0 {
        x = -x;
        code = 1;
    }
    if y < 0.0 {
        y = -y;
        code |= 2;
    }
    if y > x {
        std::mem::swap(&mut x, &mut y);
        code |= 4;
    }
    if x == 0.0 {
        return (Deg(0.0), Quad(0));
    }
    let v = (y * 32.0) / x;
    let i = trunc(v).clamp(0, 32) as usize;
    let a = (t[i + 1] - t[i]) * frac(v) + t[i];
    let d = match code {
        0 => a,
        1 => 180.0 - a,
        2 => 360.0 - a,
        3 => 180.0 + a,
        4 => 90.0 - a,
        5 => a + 90.0,
        6 => a + 270.0,
        _ => 270.0 - a,
    };
    (Deg(d), Quad([0, 1, 3, 2, 0, 1, 3, 2][usize::from(code)]))
}

/// a 28.4 point
pub type P = (i32, i32);

/// the box in 28.4: its corners P0 = (R, T), P1 = (L, T), P2 = (L, B), P3 = (R, B), half axes
/// and middle
#[derive(Clone, Copy, Debug)]
struct EBox {
    p: [P; 4],
    ha: P,
    hb: P,
    c: P,
}

impl EBox {
    fn new(p0: P, p1: P, p2: P) -> EBox {
        let p3 = (p2.0 + p0.0 - p1.0, p2.1 + p0.1 - p1.1);
        let a = (p0.0 - p1.0, p0.1 - p1.1);
        let b = (p1.0 - p2.0, p1.1 - p2.1);
        let ha = ((a.0 + 1) >> 1, (a.1 + 1) >> 1);
        let hb = ((b.0 + 1) >> 1, (b.1 + 1) >> 1);
        EBox { p: [p0, p1, p2, p3], ha, hb, c: (p2.0 + ha.0 + hb.0, p2.1 + ha.1 + hb.1) }
    }
    /// a unit point on the box's ellipse, rounded to 28.4 (halves away from 0)
    fn at(&self, u: Unit) -> P {
        let rnd = |v: f32| v.signum() as i32 * (f64::from(v.abs()) + 0.5).floor() as i32;
        let x = (self.ha.0 as f32) * u.x + (self.hb.0 as f32) * u.y;
        let y = (self.ha.1 as f32) * u.x + (self.hb.1 as f32) * u.y;
        (rnd(x) + self.c.0, rnd(y) + self.c.1)
    }
    /// quarter q's whole Bézier, integer as Ellipse's
    fn quarter(&self, q: Quad) -> [P; 3] {
        let k = |v: i32| ((i64::from(v) * 0x729D_7775) >> 32) as i32;
        let (ca, cb) = ((k(self.ha.0), k(self.ha.1)), (k(self.hb.0), k(self.hb.1)));
        let add = |a: P, b: P| (a.0 + b.0, a.1 + b.1);
        let sub = |a: P, b: P| (a.0 - b.0, a.1 - b.1);
        let [p0, p1, p2, p3] = self.p;
        match q.0 & 3 {
            0 => [sub(p0, cb), sub(p0, ca), sub(p0, self.ha)],
            1 => [add(p1, ca), sub(p1, cb), sub(p1, self.hb)],
            2 => [add(p2, cb), add(p2, ca), add(p2, self.ha)],
            _ => [sub(p3, ca), add(p3, cb), add(p3, self.hb)],
        }
    }
    /// the arc from s to e: one part of a quarter, or (multi) a part to the next axis, the
    /// whole quarters on to e's and a part from its axis
    fn partial(&self, v: &mut Vec<[P; 3]>, s: (Unit, Quad, Deg), e: (Unit, Quad, Deg), multi: bool) {
        if !multi {
            v.push(self.part(s.0, s.2, e.0, e.2));
            return;
        }
        let mut q = s.1.next();
        let (u, d) = q.axis();
        v.push(self.part(s.0, s.2, u, d));
        while q != e.1 {
            v.push(self.quarter(q));
            q = q.next();
        }
        let (u, d) = e.1.axis();
        v.push(self.part(u, d, e.0, e.2));
    }
    /// a part of a quarter from s to e: the Bézier of the span on the unit circle, mapped
    fn part(&self, s: Unit, a0: Deg, e: Unit, a1: Deg) -> [P; 3] {
        let cr = s.x * e.y - s.y * e.x;
        let acr = cr.abs();
        let (p1, p2) = if acr <= 1.0 / 65536.0 {
            (s, e)
        } else {
            let tx = (e.y - s.y) / acr;
            let ty = (s.x - e.x) / acr;
            let c = cos(Deg((a1.0 - a0.0) * 0.5)).abs();
            let k = (f32::from_bits(0x3FAA_AAAB) * c) / (c + 1.0);
            let m = 1.0 - k;
            let (kx, ky) = (k * tx, k * ty);
            (Unit { x: m * s.x + kx, y: m * s.y + ky }, Unit { x: m * e.x + kx, y: m * e.y + ky })
        };
        [self.at(p1), self.at(p2), self.at(e)]
    }
}

/// Ellipse's closed figure of the 28.4 box (L, T) - (R, B): from the right end of the middle
/// row, four whole quarters (B below T counterclockwise on the screen)
pub fn ellipse(l: i32, t: i32, r: i32, b: i32) -> [P; 13] {
    let bx = EBox::new((r, t), (l, t), (l, b));
    let mut v = [bx.quarter(Quad(3))[2]; 13];
    for q in 0..4u8 {
        let g = bx.quarter(Quad(q));
        v[1 + 3 * usize::from(q)..4 + 3 * usize::from(q)].copy_from_slice(&g);
    }
    v
}

/// what the arc's path is: its first point (a figure's start for Arc, Pie and Chord, a line's
/// end for ArcTo) and its Béziers
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arc {
    pub start: P,
    pub beziers: Vec<[P; 3]>,
    /// the box's 28.4 middle (where Pie's line goes)
    pub middle: P,
}

/// the arc of the box (l, t) - (r, b) (ordered) from radial rs to radial re, counterclockwise or
/// (cw) clockwise; None for an empty box (it records nothing)
pub fn arc(bx: (i32, i32, i32, i32), rs: (i32, i32), re: (i32, i32), cw: bool) -> Option<Arc> {
    let (l, mut t, r, mut b) = bx;
    if cw {
        std::mem::swap(&mut t, &mut b);
    }
    let (mut fl, mut ft, mut fr, mut fb) = (l * 16, t * 16, r * 16, b * 16);
    if (fr - fl).abs() < 16 || (fb - ft).abs() < 16 {
        return None;
    }
    if fr - fl > 0 { fr -= 16 } else { fl -= 16 }
    if fb - ft > 0 { fb -= 16 } else { ft -= 16 }
    let bx = EBox::new((fr, ft), (fl, ft), (fl, fb));
    // (the radials' parametric angles on the logical box's ellipse, y up)
    let half = |a: i32, b: i32| ((f64::from(a) - f64::from(b)) / 2.0) as f32;
    let (cx, cy) = (half(l, -r), half(t, -b));
    let (ax, ay) = (half(r, l), half(t, b));
    let angle = |q: (i32, i32)| arctan((q.0 as f32 - cx) / ax, (q.1 as f32 - cy) / ay);
    let (a0, q0) = angle(rs);
    let (a1, q1) = angle(re);
    let sweep = (a1.0 - a0.0).abs();
    let cs = if sweep != 0.0 && sweep - 3.0 < 0.0 { cos_sin_precise } else { cos_sin };
    let (s, e) = (cs(a0), cs(a1));
    // (not past it: also NaN, as GDI compares)
    let multi = q0 != q1 || a1.partial_cmp(&a0) != Some(std::cmp::Ordering::Greater);
    let mut beziers = Vec::new();
    bx.partial(&mut beziers, (s, q0, a0), (e, q1, a1), multi);
    Some(Arc { start: bx.at(s), beziers, middle: bx.c })
}

/// a point on the unit circle's quadrant as AngleArc takes it (0 to 3 counterclockwise from
/// x > 0, y >= 0; the axis points' own rules)
fn quadrant(p: Unit) -> Quad {
    if p.y < 0.0 {
        Quad(if p.x >= 0.0 { 3 } else { 2 })
    } else if p.x <= 0.0 {
        Quad(if p.y == 0.0 { 2 } else { 1 })
    } else {
        Quad(0)
    }
}

/// AngleArc's arc (GDI.PATH.ANGLEARC): the circle of the radius about (x, y), from the start
/// angle by the sweep (degrees, counterclockwise; a negative sweep mirrors it, whatever the arc
/// direction), whole turns past the first up to 8 more; its start the end of the line from the
/// current position; None where GDI fails (a radius past 2^31, coordinates out of range)
pub fn angle_arc(x: i32, y: i32, radius: u32, start: f32, sweep: f32) -> Option<Arc> {
    let rad = i32::try_from(radius).ok()?;
    let (l, t, r, b) = (x.checked_sub(rad)?, y.checked_sub(rad)?, x.checked_add(rad)?, y.checked_add(rad)?);
    // (in 28.4: GDI's 28 bit coordinates)
    let fix = |v: i32| v.checked_mul(16).filter(|v| (-(1 << 31)..(1 << 31) - 16).contains(&i64::from(*v)));
    let (mut tt, mut bb) = (fix(t)?, fix(b)?);
    let (fl, fr) = (fix(l)?, fix(r)?);
    let (mut st, mut sw) = (start, sweep);
    if sw < 0.0 {
        st = -st;
        sw = -sw;
        std::mem::swap(&mut tt, &mut bb);
    }
    let bx = EBox::new((fr, tt), (fl, tt), (fl, bb));
    let mut n = trunc(sw * (1.0 / 90.0));
    let revs = (n >> 2).min(8);
    let mut en = st + sw;
    let d = en - st;
    let cs = if d != 0.0 && d - 3.0 < 0.0 { cos_sin_precise } else { cos_sin };
    let (s, e) = (cs(Deg(st)), cs(Deg(en)));
    let mut sq = quadrant(s);
    if !(-3600.0..=3600.0).contains(&st) {
        let (a, q) = arctan(s.x, s.y);
        (st, sq) = (a.0, q);
    }
    let mut eq = quadrant(e);
    if !(-3600.0..=3600.0).contains(&en) {
        let (a, q) = arctan(e.x, e.y);
        (en, eq) = (a.0, q);
        n = i32::from((eq.0 + 4 - sq.0) & 3);
        if n == 0 && st > en {
            n = 3;
        }
    }
    let multi = n & 3;
    let (from, to) = ((s, sq, Deg(st)), (e, eq, Deg(en)));
    let mut beziers = Vec::new();
    bx.partial(&mut beziers, from, to, multi != 0);
    for _ in 0..revs {
        bx.partial(&mut beziers, to, from, 3 - multi != 0);
        bx.partial(&mut beziers, from, to, multi != 0);
    }
    Some(Arc { start: bx.at(s), beziers, middle: bx.c })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(a: &Arc) -> String {
        let mut s = format!("M{:?}", a.start);
        for b in &a.beziers {
            s += &format!(" C{:?}{:?}{:?}", b[0], b[1], b[2]);
        }
        s.replace(", ", ",")
    }

    #[test]
    fn the_notes_examples() {
        let a = |bx, rs, re, cw| path(&arc(bx, rs, re, cw).unwrap());
        assert_eq!(a((0, 0, 100, 100), (100, 0), (0, 0), false), "M(1352,232) C(1203,83)(1002,0)(792,0) C(582,0)(381,83)(232,232)");
        assert_eq!(a((0, 0, 100, 100), (100, 0), (0, 0), true), "M(1352,232) C(1501,381)(1584,582)(1584,792) C(1584,1230)(1230,1584)(792,1584) C(354,1584)(0,1230)(0,792) C(0,582)(83,381)(232,232)");
        assert_eq!(a((0, 0, 100, 50), (100, 0), (0, 0), false), "M(1352,115) C(1203,41)(1002,0)(792,0) C(582,0)(381,41)(232,115)");
        assert_eq!(a((0, 0, 200, 50), (150, 0), (0, 50), false), "M(2304,41) C(2084,14)(1840,0)(1592,0) C(712,0)(0,176)(0,392) C(0,496)(168,596)(466,669)");
        assert_eq!(a((0, 0, 301, 101), (300, 40), (0, 30), false), "M(4749,636) C(4517,266)(3536,0)(2400,0) C(1423,0)(544,197)(177,499)");
        assert_eq!(a((0, 0, 100, 100), (100, 50), (100, 50), false), "M(1584,792) C(1584,355)(1229,0)(792,0) C(354,0)(0,355)(0,792) C(0,1229)(354,1584)(792,1584) C(1230,1584)(1584,1229)(1584,792) C(1584,792)(1584,792)(1584,792)");
        assert_eq!(a((0, 0, 100, 100), (100, 49), (100, 51), false), "M(1584,776) C(1575,345)(1223,0)(792,0) C(354,0)(0,355)(0,792) C(0,1229)(354,1584)(792,1584) C(1223,1584)(1575,1239)(1584,808)");
        assert_eq!(a((0, 0, 100, 100), (100, 50), (100, 49), false), "M(1584,792) C(1584,787)(1584,781)(1584,776)");
        let g = |x, y, r, st, sw| path(&angle_arc(x, y, r, st, sw).unwrap()).replacen('M', "L", 1);
        assert_eq!(g(50, 50, 40, 30.0, 100.0), "L(1354,480) C(1240,282)(1029,160)(800,160) C(649,160)(504,213)(389,310)");
        assert_eq!(g(50, 50, 40, 30.0, -100.0), "L(1354,480) C(1410,577)(1440,687)(1440,800) C(1440,1069)(1272,1309)(1019,1401)");
        assert_eq!(g(0, 0, 10, 0.0, 45.0), "L(160,0) C(160,-42)(143,-83)(113,-113)");
    }
}
