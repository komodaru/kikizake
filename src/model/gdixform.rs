//! The model of runner/gdixform.c's ops (spec/gdixform.txt): mapping modes, window / viewport
//! extents and origins, the world transform, LPtoDP / DPtoLP, CombineTransform.
//!
//! Sources (Microsoft Learn): "SetMapMode", "SetWindowExtEx", "SetViewportExtEx",
//! "SetWindowOrgEx", "SetViewportOrgEx", "ScaleWindowExtEx", "OffsetWindowOrgEx",
//! "SetGraphicsMode", "SetWorldTransform", "ModifyWorldTransform", "CombineTransform",
//! "LPtoDP", "DPtoLP", "Transformation of Coordinate Spaces" (the formulas).

use crate::spec::clause;
use super::gdi::Gdi;
use super::{Fail, Res};
use crate::case::{num, Op};
use crate::spec::hit;

pub type Xform = [f32; 6];
pub const IDENTITY: Xform = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Clone, Debug, PartialEq)]
pub struct Map {
    pub mode: i64,
    pub gmode: i64,
    pub wext: (i32, i32),
    pub vext: (i32, i32),
    pub worg: (i32, i32),
    pub vorg: (i32, i32),
    pub world: Xform,
    /// x86 only: an LPtoDP has run since the mapping last changed (GDI.MAP.ROUNDX86)
    pub warm: bool,
    /// the extents or the mode changed since the mapping was last read (GDI.MAP.ISOTROPIC)
    pub unsettled: bool,
}

impl Default for Map {
    /// MM_TEXT, GM_COMPATIBLE, extents 1 x 1, origins 0, the identity
    fn default() -> Map {
        Map { mode: 1, gmode: 1, wext: (1, 1), vext: (1, 1), worg: (0, 0), vorg: (0, 0), world: IDENTITY, warm: false, unsettled: false }
    }
}

impl Map {
    pub fn identity(&self) -> bool {
        self.wext == self.ve() && self.worg == self.vorg && self.world == IDENTITY
    }
    /// the viewport extents in effect: MM_ISOTROPIC shrinks one of those stored so that a
    /// logical unit is as wide as high, rounded to the nearest, 0 included (see settle)
    pub fn ve(&self) -> (i32, i32) {
        if self.mode != 7 || !self.unsettled {
            return self.vext;
        }
        self.shrunk()
    }
    /// MEASURED (GDI.MAP.ISOPOINTS, isomore-*, iso-51): LPtoDP and DPtoLP in MM_ISOTROPIC map
    /// through the extents stored shrunk once more, y only: when the rule would now shrink x,
    /// they keep those stored
    pub fn ve_points(&self) -> (i32, i32) {
        let v = self.ve();
        if self.mode != 7 || self.unsettled {
            return v;
        }
        hit(clause::GDI_MAP_ISOPOINTS);
        let s = self.shrunk();
        if s.0 == v.0 { s } else { v }
    }
    /// the stored viewport extents shrunk for MM_ISOTROPIC (GDI.MAP.ISOTROPIC)
    fn shrunk(&self) -> (i32, i32) {
        hit(clause::GDI_MAP_ISOTROPIC);
        let v = self.vext;
        let (xd, yd) = (v.0 as f64 / self.wext.0 as f64, v.1 as f64 / self.wext.1 as f64);
        let near = |r: f64| -> i32 { (r + 0.5).floor() as i32 };
        if xd.abs() > yd.abs() {
            (near(v.0 as f64 * (yd / xd).abs()), v.1)
        } else {
            (v.0, near(v.1 as f64 * (xd / yd).abs()))
        }
    }
    /// MEASURED (GDI.MAP.ISOTROPIC): the shrunk viewport extents are stored the first time
    /// something reads the mapping after a change (GetViewportExtEx, LPtoDP, Scale*ExtEx),
    /// and later changes start from them; Set*ExtEx return the extents stored, unshrunk
    /// (cases iso-*: Set*ExtEx alone; scalemode-*: a read between each change)
    fn settle(&mut self) {
        if self.unsettled {
            self.vext = self.ve();
            self.unsettled = false;
        }
    }
    /// LPtoDP of one point: world (f32), then page: p * vext / wext + (vorg - worg * vext / wext)
    pub fn lp_to_dp(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        if self.world != IDENTITY {
            return self.world_to_dp(p);
        }
        hit(clause::GDI_MAP_FORMULA);
        if self.mode == 7 {
            // MEASURED (both archs, iso-* isomore-*): the page rule with the viewport extents
            // of GDI.MAP.ISOPOINTS (GDI.MAP.ISOLP)
            hit(clause::GDI_MAP_ISOLP);
        }
        // MEASURED: 32-bit GDI's LPtoDP after another with the same mapping truncates the
        // scaled point to 28.4 toward zero (the first call, and every call on x64, rounds it)
        let trunc = self.warm && super::arch() == 32;
        if trunc {
            hit(clause::GDI_MAP_ROUNDX86);
        }
        let e = self.ve_points();
        Ok((page_with(p.0, self.wext.0, e.0, self.worg.0, self.vorg.0, trunc), page_with(p.1, self.wext.1, e.1, self.worg.1, self.vorg.1, trunc)))
    }
    /// DPtoLP (MEASURED, GDI.MAP.DPROUND): the page transform's matrix in f32 (vext / wext
    /// each axis), inverted through its determinant in f32; each axis the point times the
    /// inverse and the offset wO - vO times the inverse, each rounded halves away from zero
    pub fn dp_to_lp(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        if self.world != IDENTITY {
            return self.dp_to_world(p);
        }
        hit(clause::GDI_MAP_INVERSE);
        let v = self.ve_points();
        if v.0 == 0 || v.1 == 0 || self.wext.0 == 0 || self.wext.1 == 0 {
            return Err(Fail::Bad);
        }
        hit(clause::GDI_MAP_DPROUND);
        let m11 = v.0 as f32 / self.wext.0 as f32;
        let m22 = v.1 as f32 / self.wext.1 as f32;
        let det = m11 * m22;
        let (i11, i22) = (m22 / det, m11 / det);
        let iso = self.mode == 7;
        let axis = |d: i32, inv: f32, wo: i32, vo: i32| -> i32 {
            let a = f64::from(d as f32 * inv).round();
            // (MM_ISOTROPIC: the offset in f64, isomore-0: 8 - 2 * -1.2499999 is 10.4999998,
            // not f32's 10.5; GDI.MAP.ISOPOINTS)
            let b = if iso { (f64::from(wo) - f64::from(vo) * f64::from(inv)).round() } else { f64::from(wo as f32 - vo as f32 * inv).round() };
            (a + b) as i32
        };
        Ok((axis(p.0, i11, self.worg.0, self.vorg.0), axis(p.1, i22, self.worg.1, self.vorg.1)))
    }
}

impl Map {
    /// whether a point maps to a whole device point (no rounding), the world's numbers and the
    /// page's quotient exact in f64
    fn whole(&self, p: (i32, i32)) -> bool {
        let w = self.world;
        let e = self.ve_points();
        let axis = |x: f64, we: i32, ve: i32, wo: i32, vo: i32| -> bool { we != 0 && ((x - f64::from(wo)) * f64::from(ve) / f64::from(we) + f64::from(vo)).fract() == 0.0 };
        let (x, y) = (f64::from(p.0), f64::from(p.1));
        let wx = x * f64::from(w[0]) + y * f64::from(w[2]) + f64::from(w[4]);
        let wy = x * f64::from(w[1]) + y * f64::from(w[3]) + f64::from(w[5]);
        axis(wx, self.wext.0, e.0, self.worg.0, self.vorg.0) && axis(wy, self.wext.1, e.1, self.worg.1, self.vorg.1)
    }
    /// the world transform times the page transform, in f32: each of the world's six numbers
    /// times vext / wext (an f32 quotient) of its axis, the offsets plus vO - wO * that
    /// (GDI.XFORM.WORLDPOINT, GDI.XFORM.ROUNDWORLD)
    fn device_matrix(&self) -> Res<Xform> {
        let e = self.ve_points();
        if self.wext.0 == 0 || self.wext.1 == 0 {
            return Err(Fail::Bad);
        }
        let (sx, sy) = (e.0 as f32 / self.wext.0 as f32, e.1 as f32 / self.wext.1 as f32);
        let w = self.world;
        if self.worg != (0, 0) || self.vorg != (0, 0) {
            hit(clause::GDI_XFORM_ROUNDWORLD);
        }
        let (ox, oy) = (self.vorg.0 as f32 - self.worg.0 as f32 * sx, self.vorg.1 as f32 - self.worg.1 as f32 * sy);
        Ok([w[0] * sx, w[1] * sy, w[2] * sx, w[3] * sy, w[4] * sx + ox, w[5] * sy + oy])
    }
    /// MEASURED (GDI.XFORM.WORLDPOINT, 5120 of 5120 world-* coordinates each arch): LPtoDP
    /// through a world transform: each axis the point times the f32 matrix (device_matrix),
    /// exactly, to 28.4 rounded halves away from zero (truncated toward zero on x86 once warm:
    /// GDI.XFORM.WARMX86), plus the offset to 28.4 rounded halves away; then (v + 8) >> 4
    fn world_to_dp(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        let f = self.world_to_fix(p)?;
        Ok((((f.0 + 8).div_euclid(16)) as i32, ((f.1 + 8).div_euclid(16)) as i32))
    }
    /// a point through the world transform to 28.4 device units (world_to_dp before its last
    /// rounding)
    pub fn world_to_fix(&self, p: (i32, i32)) -> Res<(i64, i64)> {
        let m = self.device_matrix()?;
        hit(clause::GDI_XFORM_WORLDPOINT);
        let trunc = self.warm && super::arch() == 32;
        if trunc {
            hit(clause::GDI_XFORM_WARMX86);
        }
        let axis = |a: f32, b: f32, d: f32| -> i64 {
            // (x * a + y * b is exact in f64: two 24-bit mantissas times 32-bit integers)
            let v = (f64::from(p.0) * f64::from(a) + f64::from(p.1) * f64::from(b)) * 16.0;
            (if trunc { v.trunc() } else { v.round() } + (f64::from(d) * 16.0).round()) as i64
        };
        Ok((axis(m[0], m[2], m[4]), axis(m[1], m[3], m[5])))
    }
    /// MEASURED (GDI.XFORM.WORLDINV, 3840 of 3840 world-* coordinates each arch): DPtoLP
    /// through a world transform: the f32 matrix inverted in f32 through its determinant
    /// (m11 m22 - m12 m21), the inverse offset -(dx i11 + dy i21) in f32; each axis the f32 sum
    /// of the point's two f32 products rounded halves away, plus the offset rounded halves away
    fn dp_to_world(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        let [a, b, c, d, e, f] = self.device_matrix()?;
        let det = a * d - b * c;
        if det == 0.0 {
            return Err(Fail::Bad);
        }
        hit(clause::GDI_XFORM_WORLDINV);
        let i = [d / det, -b / det, -c / det, a / det];
        let (ox, oy) = (-(e * i[0] + f * i[2]), -(e * i[1] + f * i[3]));
        let (x, y) = (p.0 as f32, p.1 as f32);
        let axis = |v: f32, o: f32| -> i32 { (f64::from(v).round() + f64::from(o).round()) as i32 };
        Ok((axis(x * i[0] + y * i[2], ox), axis(x * i[1] + y * i[3], oy)))
    }
}

/// n / d to the nearest 1/16 (28.4 fixed point, halves up), in 1/16 units
fn fix(n: i64, d: i64) -> i64 {
    let (n, d) = if d < 0 { (-n, -d) } else { (n, d) };
    (32 * n + d).div_euclid(2 * d)
}

/// one axis of the page transform as Windows rounds it (MEASURED): the scaled point and the
/// offset each to 28.4 fixed point, their sum to the nearest pixel, halves up
pub fn page(a: i32, w: i32, v: i32, worg: i32, vorg: i32) -> i32 {
    page_with(a, w, v, worg, vorg, false)
}

/// page, the scaled point truncated toward zero to 28.4 instead when `trunc`
fn page_with(a: i32, w: i32, v: i32, worg: i32, vorg: i32, trunc: bool) -> i32 {
    hit(clause::GDI_MAP_ROUND);
    let (a, w, v, wo, vo) = (i64::from(a), i64::from(w), i64::from(v), i64::from(worg), i64::from(vorg));
    let scaled = if trunc { (16 * a * v) / w } else { fix(a * v, w) };
    let s = scaled + fix(vo * w - wo * v, w);
    i32::try_from((s + 8).div_euclid(16)).unwrap_or(i32::MAX)
}

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
fn f(op: &Op, i: usize) -> Res<f32> {
    op.tok.get(i).and_then(|t| t.parse::<f32>().ok()).ok_or(Fail::Bad)
}
fn xf(op: &Op, from: usize) -> Res<Xform> {
    let mut x = [0f32; 6];
    for (i, v) in x.iter_mut().enumerate() {
        *v = f(op, from + i)?;
    }
    Ok(x)
}
fn show(x: &Xform) -> String {
    let b: Vec<String> = x.iter().map(|v| format!("{:08x}", v.to_bits())).collect();
    format!("xform {}", b.join(" "))
}

/// a * b as CombineTransform documents it (the transformation a, then b)
pub fn multiply(a: &Xform, b: &Xform) -> Xform {
    hit(clause::GDI_XFORM_COMBINE);
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// a line op of gdiline.c or an op of gdi.c on a slot whose logical coordinates are not device
/// coordinates, as the op in device coordinates: lineto from the current position (logical)
/// to the point, rectangle and patblt by their two corners, each point through LPtoDP
/// (GDI.MAP.DRAW), with the current position (device) it starts from; None for an op or a transform no case measures (rotations, shears)
pub fn device_op(m: &mut Map, pos: (i32, i32), op: &Op) -> Res<Option<(Op, (i32, i32))>> {
    let t0 = op.tok[0].as_str();
    if m.world[1] != 0.0 || m.world[2] != 0.0 {
        // (a rectangle under a rotation or shear is a parallelogram)
        return Ok(None);
    }
    if t0 == "lineto" && m.world != IDENTITY {
        // (gdiline draws these itself, in 28.4: GDI.MAP.DRAWADV)
        return Ok(None);
    }
    m.settle();
    hit(clause::GDI_MAP_DRAWPTS);
    let lp = |x: i64, y: i64| -> Res<(i32, i32)> {
        let q = m.lp_to_dp((i32::try_from(x).map_err(|_| Fail::Bad)?, i32::try_from(y).map_err(|_| Fail::Bad)?))?;
        Ok(q)
    };
    let tok = |v: Vec<String>, from: (i32, i32)| Some((Op { line: op.line, tok: v }, from));
    let k = op.tok.get(1).cloned().ok_or(Fail::Bad)?;
    Ok(match t0 {
        "lineto" => {
            let to = (n(op, 2)?, n(op, 3)?);
            let a = lp(i64::from(pos.0), i64::from(pos.1))?;
            let b = lp(to.0, to.1)?;
            // MEASURED (fracline-*, 48 of 48, both archs): a line whose ends fall between
            // pixels is the integer line between its ends rounded as LPtoDP rounds them
            // (GDI.MAP.DRAWPTS)
            tok(vec!["lineto".into(), k, b.0.to_string(), b.1.to_string()], a)
        }
        "rectangle" => {
            let a = lp(n(op, 2)?, n(op, 3)?)?;
            let b = lp(n(op, 4)?, n(op, 5)?)?;
            tok(vec!["rectangle".into(), k, a.0.to_string(), a.1.to_string(), b.0.to_string(), b.1.to_string()], pos)
        }
        "patblt" => {
            let (x, y) = (n(op, 2)?, n(op, 3)?);
            let a = lp(x, y)?;
            let b = lp(x + n(op, 4)?, y + n(op, 5)?)?;
            tok(vec!["patblt".into(), k, a.0.to_string(), a.1.to_string(), (b.0 - a.0).to_string(), (b.1 - a.1).to_string(), op.tok.get(6).cloned().ok_or(Fail::Bad)?], pos)
        }
        _ => None,
    })
}

pub fn op(g: &mut Gdi, op: &Op, out: &mut Vec<String>) -> Res<bool> {
    let t0 = op.tok[0].as_str();
    if t0 == "combinexform" {
        let (a, b) = (xf(op, 1)?, xf(op, 7)?);
        let r = multiply(&a, &b);
        out.push("combinexform 1".into());
        out.push(show(&r));
        return Ok(true);
    }
    if !matches!(t0, "mapmode" | "gmode" | "winext" | "vpext" | "winorg" | "vporg" | "offsetwin" | "offsetvp" | "scalewin" | "scalevp" | "getmap" | "lptodp" | "dptolp" | "setxform" | "modxform" | "getxform") {
        return Ok(false);
    }
    let k = n(op, 1)?;
    let sl = g.slots.get_mut(k as usize).and_then(|s| s.as_mut()).ok_or(Fail::Bad)?;
    let m = &mut sl.ext.map;
    if !matches!(t0, "lptodp" | "dptolp" | "getmap" | "getxform") {
        m.warm = false;
    }
    match t0 {
        "mapmode" => {
            let v = n(op, 2)?;
            hit(clause::GDI_MAP_MODE);
            if !(1..=8).contains(&v) {
                out.push("mapmode 0".into());
            } else {
                match v {
                    1 => (m.wext, m.vext) = ((1, 1), (1, 1)),
                    2..=7 => {
                        // MEASURED: the window extents are the display's size in the mode's
                        // units (at 96 pixels per inch, rounded), the viewport's its pixels, y up
                        hit(clause::GDI_MAP_METRIC);
                        let k = [254, 2540, 100, 1000, 1440, 254][v as usize - 2];
                        let (w, h) = super::gdiddb::SCREEN;
                        m.wext = (((w * k) as f64 / 96.0).round() as i32, ((h * k) as f64 / 96.0).round() as i32);
                        m.vext = (w, -h);
                    }
                    // (MM_ANISOTROPIC keeps the extents it finds)
                    _ => {}
                }
                out.push(format!("mapmode {}", std::mem::replace(&mut m.mode, v)));
                m.unsettled = true;
            }
        }
        "gmode" => {
            let v = n(op, 2)?;
            hit(clause::GDI_XFORM_GMODE);
            // MEASURED (world-*, gmodeback-*): GM_COMPATIBLE over a world transform other than
            // the identity is set all the same (not the doc's failure), the transform staying
            // and still mapping points; ModifyWorldTransform then fails as in GM_COMPATIBLE
            let out_mode = match v {
                1 | 2 => std::mem::replace(&mut m.gmode, v),
                _ => 0,
            };
            out.push(format!("gmode {out_mode}"));
        }
        "winext" | "vpext" => {
            let (x, y) = (n(op, 2)? as i32, n(op, 3)? as i32);
            let e = if t0 == "winext" { &mut m.wext } else { &mut m.vext };
            let old = *e;
            hit(clause::GDI_MAP_EXT);
            if m.mode == 7 && (x == 0 || y == 0) {
                // MEASURED (isomore-2): MM_ISOTROPIC refuses an extent of 0, failing
                hit(clause::GDI_MAP_ZEROEXT);
                out.push(format!("ext 0 {} {}", old.0, old.1));
                return Ok(true);
            }
            if m.mode >= 7 {
                if x == 0 || y == 0 {
                    // MEASURED: succeeds, the extents as they were
                    hit(clause::GDI_MAP_ZEROEXT);
                } else {
                    *e = (x, y);
                }
            } else {
                // MEASURED: the fixed modes ignore them, succeeding
                hit(clause::GDI_MAP_FIXEDEXT);
            }
            out.push(format!("ext 1 {} {}", old.0, old.1));
            m.unsettled = true;
        }
        "winorg" | "vporg" | "offsetwin" | "offsetvp" => {
            let (x, y) = (n(op, 2)? as i32, n(op, 3)? as i32);
            let o = if t0.contains("win") { &mut m.worg } else { &mut m.vorg };
            let old = *o;
            hit(clause::GDI_MAP_ORG);
            *o = if t0.starts_with("offset") { (old.0 + x, old.1 + y) } else { (x, y) };
            out.push(format!("org 1 {} {}", old.0, old.1));
        }
        "scalewin" | "scalevp" => {
            m.settle();
            let (xn, xd, yn, yd) = (n(op, 2)?, n(op, 3)?, n(op, 4)?, n(op, 5)?);
            if m.mode < 7 {
                // MEASURED (scalemode-*): the fixed modes keep their extents, succeeding
                hit(clause::GDI_MAP_SCALE);
                let e = if t0 == "scalewin" { m.wext } else { m.vext };
                out.push(format!("ext 1 {} {}", e.0, e.1));
                return Ok(true);
            }
            // MEASURED (MM_ANISOTROPIC): each extent times num / denom truncated toward zero;
            // a 0 among the four, or an extent that comes out 0, fails and changes nothing;
            // ScaleWindowExtEx then leaves the SIZE unwritten, ScaleViewportExtEx writes the old
            hit(clause::GDI_MAP_SCALE);
            let win = t0 == "scalewin";
            let e = if win { &mut m.wext } else { &mut m.vext };
            let old = *e;
            let scale = |v: i32, num: i64, den: i64| -> Option<i32> {
                if num == 0 || den == 0 {
                    return None;
                }
                i32::try_from(i64::from(v) * num / den).ok().filter(|&r| r != 0)
            };
            match (scale(old.0, xn, xd), scale(old.1, yn, yd)) {
                (Some(x), Some(y)) => {
                    *e = (x, y);
                    out.push(format!("ext 1 {} {}", old.0, old.1));
                    m.unsettled = true;
                }
                _ if win => out.push("ext 0 -1 -1".into()),
                _ => out.push(format!("ext 0 {} {}", old.0, old.1)),
            }
        }
        "getmap" => {
            m.settle();
            hit(clause::GDI_MAP_GET);
            out.push(format!("map {} {} wext {} {} vext {} {} worg {} {} vorg {} {}", m.mode, m.gmode, m.wext.0, m.wext.1, m.ve().0, m.ve().1, m.worg.0, m.worg.1, m.vorg.0, m.vorg.1));
        }
        "lptodp" | "dptolp" => {
            m.settle();
            let t = &op.tok[2..];
            if t.len() % 2 == 1 {
                return Err(Fail::Bad);
            }
            let v = m.ve_points();
            if t0 == "dptolp" && m.world == IDENTITY && (v.0 == 0 || v.1 == 0 || m.wext.0 == 0 || m.wext.1 == 0) {
                // MEASURED (isomore-13): MM_ISOTROPIC shrinking an axis to 0 leaves no inverse;
                // DPtoLP fails, the points as they were (GDI.MAP.ZEROEXT)
                hit(clause::GDI_MAP_ZEROEXT);
                out.push(format!("pts 0 {}", t.join(" ")));
                return Ok(true);
            }
            let mut s = String::from("pts 1");
            for p in t.chunks(2) {
                let p = (num(&p[0]).ok_or(Fail::Bad)? as i32, num(&p[1]).ok_or(Fail::Bad)? as i32);
                let q = if t0 == "lptodp" { m.lp_to_dp(p)? } else { m.dp_to_lp(p)? };
                s += &format!(" {} {}", q.0, q.1);
            }
            out.push(s);
            if t0 == "lptodp" {
                m.warm = true;
            }
        }
        "getxform" => {
            hit(clause::GDI_XFORM_GET);
            out.push("getxform 1".into());
            out.push(show(&m.world));
        }
        "setxform" => {
            let x = xf(op, 2)?;
            hit(clause::GDI_XFORM_SET);
            if m.gmode != 2 {
                out.push("setxform 0".into());
            } else {
                if x[0] * x[3] - x[1] * x[2] == 0.0 {
                    // MEASURED: fails, the transform as it was
                    hit(clause::GDI_XFORM_SINGULAR);
                    out.push("setxform 0".into());
                    return Ok(true);
                }
                m.world = x;
                out.push("setxform 1".into());
            }
        }
        _ => {
            let mode = n(op, 2)?;
            let x = xf(op, 3)?;
            hit(clause::GDI_XFORM_MODIFY);
            if m.gmode != 2 || !(1..=3).contains(&mode) {
                out.push("modxform 0".into());
            } else {
                m.world = match mode {
                    1 => IDENTITY,
                    2 => multiply(&x, &m.world),
                    _ => multiply(&m.world, &x),
                };
                // MEASURED (x86): a multiplying ModifyWorldTransform leaves the transform warm,
                // as an LPtoDP does (GDI.XFORM.WARMX86); SetWorldTransform does not
                m.warm = mode != 1;
                out.push("modxform 1".into());
            }
        }
    }
    Ok(true)
}
