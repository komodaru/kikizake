//! The model of runner/gdixform.c's ops (spec/gdixform.txt): mapping modes, window / viewport
//! extents and origins, the world transform, LPtoDP / DPtoLP, CombineTransform.
//!
//! Sources (Microsoft Learn): "SetMapMode", "SetWindowExtEx", "SetViewportExtEx",
//! "SetWindowOrgEx", "SetViewportOrgEx", "ScaleWindowExtEx", "OffsetWindowOrgEx",
//! "SetGraphicsMode", "SetWorldTransform", "ModifyWorldTransform", "CombineTransform",
//! "LPtoDP", "DPtoLP", "Transformation of Coordinate Spaces" (the formulas).

use super::gdi::{Gdi, Slot};
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
}

impl Default for Map {
    /// MM_TEXT, GM_COMPATIBLE, extents 1 x 1, origins 0, the identity
    fn default() -> Map {
        Map { mode: 1, gmode: 1, wext: (1, 1), vext: (1, 1), worg: (0, 0), vorg: (0, 0), world: IDENTITY }
    }
}

impl Map {
    pub fn identity(&self) -> bool {
        self.wext == self.vext && self.worg == self.vorg && self.world == IDENTITY
    }
    /// LPtoDP of one point: world (f32), then page: (p - worg) * vext / wext + vorg, rounded
    pub fn lp_to_dp(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        let (x, y) = (p.0 as f64, p.1 as f64);
        let w = self.world;
        let (x, y) = if w == IDENTITY { (x, y) } else {
            hit("GDI.XFORM.WORLDPOINT");
            return Err(Fail::Unmodeled("GDI.XFORM.ROUNDWORLD"));
        };
        hit("GDI.MAP.FORMULA");
        let fx = (x - self.worg.0 as f64) * self.vext.0 as f64 / self.wext.0 as f64 + self.vorg.0 as f64;
        let fy = (y - self.worg.1 as f64) * self.vext.1 as f64 / self.wext.1 as f64 + self.vorg.1 as f64;
        Ok((round(fx)?, round(fy)?))
    }
    pub fn dp_to_lp(&self, p: (i32, i32)) -> Res<(i32, i32)> {
        if self.world != IDENTITY {
            return Err(Fail::Unmodeled("GDI.XFORM.ROUNDWORLD"));
        }
        hit("GDI.MAP.INVERSE");
        let fx = (p.0 as f64 - self.vorg.0 as f64) * self.wext.0 as f64 / self.vext.0 as f64 + self.worg.0 as f64;
        let fy = (p.1 as f64 - self.vorg.1 as f64) * self.wext.1 as f64 / self.vext.1 as f64 + self.worg.1 as f64;
        Ok((round(fx)?, round(fy)?))
    }
}

/// the rounding of a mapped coordinate (to the nearest, halves away from zero; MEASURED)
fn round(v: f64) -> Res<i32> {
    hit("GDI.MAP.ROUND");
    if v.fract().abs() == 0.5 {
        return Err(Fail::Unmodeled("GDI.MAP.HALF"));
    }
    Ok(v.round() as i32)
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
    hit("GDI.XFORM.COMBINE");
    [
        a[0] * b[0] + a[1] * b[2],
        a[0] * b[1] + a[1] * b[3],
        a[2] * b[0] + a[3] * b[2],
        a[2] * b[1] + a[3] * b[3],
        a[4] * b[0] + a[5] * b[2] + b[4],
        a[4] * b[1] + a[5] * b[3] + b[5],
    ]
}

/// a line op of gdiline.c on a slot whose logical coordinates are not device coordinates
pub fn mapped_line_op(_sl: &mut Slot, _op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    Err(Fail::Unmodeled("GDI.MAP.DRAW"))
}

/// an op of gdi.c (PatBlt, BitBlt, ...) on a slot whose logical coordinates are not device
/// coordinates
pub fn mapped_gdi_op(_g: &mut Gdi, _op: &Op, _out: &mut Vec<String>) -> Res<bool> {
    Err(Fail::Unmodeled("GDI.MAP.DRAW"))
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
    match t0 {
        "mapmode" => {
            let v = n(op, 2)?;
            hit("GDI.MAP.MODE");
            if !(1..=8).contains(&v) {
                out.push("mapmode 0".into());
            } else {
                if !matches!(v, 1 | 8) || m.mode != 1 {
                    // (the metric modes' extents and MM_ISOTROPIC's come from the device)
                    return Err(Fail::Unmodeled("GDI.MAP.METRIC"));
                }
                out.push(format!("mapmode {}", std::mem::replace(&mut m.mode, v)));
            }
        }
        "gmode" => {
            let v = n(op, 2)?;
            hit("GDI.XFORM.GMODE");
            let ok = (v == 1 && m.world == IDENTITY) || v == 2;
            out.push(format!("gmode {}", if ok { std::mem::replace(&mut m.gmode, v) } else { 0 }));
        }
        "winext" | "vpext" => {
            let (x, y) = (n(op, 2)? as i32, n(op, 3)? as i32);
            let e = if t0 == "winext" { &mut m.wext } else { &mut m.vext };
            let old = *e;
            hit("GDI.MAP.EXT");
            if m.mode == 8 {
                if x == 0 || y == 0 {
                    return Err(Fail::Unmodeled("GDI.MAP.ZEROEXT"));
                }
                *e = (x, y);
            } else if m.mode == 7 {
                return Err(Fail::Unmodeled("GDI.MAP.METRIC"));
            }
            out.push(format!("ext 1 {} {}", old.0, old.1));
        }
        "winorg" | "vporg" | "offsetwin" | "offsetvp" => {
            let (x, y) = (n(op, 2)? as i32, n(op, 3)? as i32);
            let o = if t0.contains("win") { &mut m.worg } else { &mut m.vorg };
            let old = *o;
            hit("GDI.MAP.ORG");
            *o = if t0.starts_with("offset") { (old.0 + x, old.1 + y) } else { (x, y) };
            out.push(format!("org 1 {} {}", old.0, old.1));
        }
        "scalewin" | "scalevp" => return Err(Fail::Unmodeled("GDI.MAP.SCALE")),
        "getmap" => {
            hit("GDI.MAP.GET");
            out.push(format!("map {} {} wext {} {} vext {} {} worg {} {} vorg {} {}", m.mode, m.gmode, m.wext.0, m.wext.1, m.vext.0, m.vext.1, m.worg.0, m.worg.1, m.vorg.0, m.vorg.1));
        }
        "lptodp" | "dptolp" => {
            let t = &op.tok[2..];
            if t.len() % 2 == 1 {
                return Err(Fail::Bad);
            }
            let mut s = String::from("pts 1");
            for p in t.chunks(2) {
                let p = (num(&p[0]).ok_or(Fail::Bad)? as i32, num(&p[1]).ok_or(Fail::Bad)? as i32);
                let q = if t0 == "lptodp" { m.lp_to_dp(p)? } else { m.dp_to_lp(p)? };
                s += &format!(" {} {}", q.0, q.1);
            }
            out.push(s);
        }
        "getxform" => {
            hit("GDI.XFORM.GET");
            out.push("getxform 1".into());
            out.push(show(&m.world));
        }
        "setxform" => {
            let x = xf(op, 2)?;
            hit("GDI.XFORM.SET");
            if m.gmode != 2 {
                out.push("setxform 0".into());
            } else {
                if x[0] * x[3] - x[1] * x[2] == 0.0 {
                    return Err(Fail::Unmodeled("GDI.XFORM.SINGULAR"));
                }
                m.world = x;
                out.push("setxform 1".into());
            }
        }
        _ => {
            let mode = n(op, 2)?;
            let x = xf(op, 3)?;
            hit("GDI.XFORM.MODIFY");
            if m.gmode != 2 || !(1..=3).contains(&mode) {
                out.push("modxform 0".into());
            } else {
                m.world = match mode {
                    1 => IDENTITY,
                    2 => multiply(&x, &m.world),
                    _ => multiply(&m.world, &x),
                };
                out.push("modxform 1".into());
            }
        }
    }
    Ok(true)
}
