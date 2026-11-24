//! Control points of the GL 1.1 polynomial evaluators.
use super::{array, e::{INVALID_ENUM, INVALID_OPERATION, INVALID_VALUE}, float_to_int, parse_float, query_bytes, tri, Ctx, Fail, Res};
use crate::case::windows_long;
use crate::spec::open::GL_EVAL;

#[derive(Clone, Debug)]
pub(super) struct Map {
    domain: Vec<f32>,
    order: Vec<f32>,
    coefficients: Vec<f32>,
}

fn initial(target: i32) -> Option<(&'static [f32], bool)> {
    let surface = (0xdb0..=0xdb8).contains(&target);
    let index = target.checked_sub(if surface { 0xdb0 } else { 0xd90 })?;
    let point: &[f32] = match index {
        0 => &[1.0; 4], 1 => &[1.0], 2 => &[0.0, 0.0, 1.0],
        3 => &[0.0], 4 => &[0.0; 2], 5 | 7 => &[0.0; 3],
        6 | 8 => &[0.0, 0.0, 0.0, 1.0], _ => return None,
    };
    Some((point, surface))
}

struct Axis {
    domain: [f32; 2],
    stride: usize,
    order: u8,
}

fn axis(args: &[String], width: usize) -> Res<Option<Axis>> {
    let [lo, hi, stride, order] = args else { return Err(Fail::Bad) };
    let domain = [tri::narrow(parse_float(lo)?), tri::narrow(parse_float(hi)?)];
    let stride = usize::try_from(windows_long(stride).ok_or(Fail::Bad)?).ok();
    let order = u8::try_from(windows_long(order).ok_or(Fail::Bad)?).ok();
    Ok(stride.zip(order).filter(|(s, n)| *s >= width && (1..=30).contains(n) && domain[0].partial_cmp(&domain[1]) != Some(std::cmp::Ordering::Equal))
        .map(|(stride, order)| Axis { domain, stride, order }))
}

impl Ctx {
    pub(super) fn define_map(&mut self, t: &[String]) -> Res<()> {
        super::hit(GL_EVAL.into());
        let [_, name, sig, target, args @ ..] = t else { return Err(Fail::Bad) };
        let signature = match name.as_str() {
            "glMap1f" => "iffii", "glMap1d" => "iddii", "glMap2f" => "iffiiffii", "glMap2d" => "iddiiddii",
            _ => return Err(Fail::Bad),
        };
        if sig != signature { return Err(Fail::Bad); }
        let target = windows_long(target).ok_or(Fail::Bad)?;
        if self.in_begin.is_some() { self.error(INVALID_OPERATION); return Ok(()); }
        let Some((point, surface)) = initial(target).filter(|(_, surface)| *surface == name.starts_with("glMap2")) else {
            self.error(INVALID_ENUM); return Ok(());
        };
        let width = point.len();
        let Some(u) = axis(args.get(..4).ok_or(Fail::Bad)?, width)? else { self.error(INVALID_VALUE); return Ok(()) };
        let v = if surface {
            let Some(v) = axis(args.get(4..8).ok_or(Fail::Bad)?, width)? else { self.error(INVALID_VALUE); return Ok(()) };
            Some(v)
        } else { None };
        let mut map = Map { domain: u.domain.to_vec(), order: vec![f32::from(u.order)], coefficients: Vec::new() };
        if let Some(v) = &v { map.domain.extend(v.domain); map.order.push(f32::from(v.order)); }
        let tail = t.get(3usize.saturating_add(sig.len())).ok_or(Fail::Bad)?;
        let mut bytes = match tail.as_str() {
            "obuf" if t.len() == sig.len().saturating_add(4) => vec![0xcc; 4096],
            _ => array::glp_bytes(t)?.ok_or(Fail::Unmodeled(GL_EVAL))?,
        };
        bytes.resize(4096, 0);
        let values: Vec<f32> = if name.ends_with('d') {
            bytes.as_chunks::<8>().0.iter().map(|w| tri::narrow(f64::from_le_bytes(*w))).collect()
        } else { bytes.as_chunks::<4>().0.iter().map(|w| f32::from_le_bytes(*w)).collect() };
        let (v_order, v_stride) = v.map_or((1, 0), |v| (v.order, v.stride));
        for i in 0..usize::from(u.order) {
            for j in 0..usize::from(v_order) {
                let start = i.checked_mul(u.stride).and_then(|x| x.checked_add(j.checked_mul(v_stride)?)).ok_or(Fail::Unmodeled(GL_EVAL))?;
                let row = values.get(start..).and_then(|v| v.get(..width)).ok_or(Fail::Unmodeled(GL_EVAL))?;
                map.coefficients.extend_from_slice(row);
            }
        }
        self.evaluator_maps.insert(target, map);
        Ok(())
    }

    pub(super) fn query_map(&mut self, t: &[String], out: &mut Vec<String>) -> Res<()> {
        let [_, name, sig, target, query, ty, count] = t else { return Err(Fail::Bad) };
        if sig != "ii" { return Err(Fail::Bad); }
        let target = windows_long(target).ok_or(Fail::Bad)?;
        let query = windows_long(query).ok_or(Fail::Bad)?;
        let values: &[f32] = if self.in_begin.is_some() {
            self.error(INVALID_OPERATION); &[]
        } else if let Some((point, surface)) = initial(target) {
            let map = self.evaluator_maps.get(&target);
            match query {
                0xa00 => map.map_or(point, |m| m.coefficients.as_slice()),
                0xa01 => map.map_or(if surface { &[1.0; 2][..] } else { &[1.0][..] }, |m| m.order.as_slice()),
                0xa02 => map.map_or(if surface { &[0.0, 1.0, 0.0, 1.0][..] } else { &[0.0, 1.0][..] }, |m| m.domain.as_slice()),
                _ => { self.error(INVALID_ENUM); &[] }
            }
        } else { self.error(INVALID_ENUM); &[] };
        out.push(query_bytes(ty, count, |dst| {
            if name == "glGetMapdv" {
                for (w, &v) in dst.chunks_exact_mut(8).zip(values) { w.copy_from_slice(&f64::from(v).to_le_bytes()); }
            } else {
                for (w, &v) in dst.chunks_exact_mut(4).zip(values) {
                    let bytes = if name == "glGetMapiv" { i32::try_from(float_to_int(v.round())).unwrap_or(i32::MIN).to_le_bytes() } else { v.to_le_bytes() };
                    w.copy_from_slice(&bytes);
                }
            }
            Ok(())
        })?);
        Ok(())
    }
}

/// Evaluate a Bezier curve by interpolating adjacent control points down to one.
fn curve_value(points: &[f32], width: usize, t: f32) -> Res<[f32; 4]> {
    let mut work: Vec<[f32; 4]> = points.chunks_exact(width).map(|components| {
        let mut value = [0.0, 0.0, 0.0, 1.0];
        for (dst, &src) in value.iter_mut().zip(components) { *dst = src; }
        value
    }).collect();
    while work.len() > 1 {
        let Some(mut left) = work.first().copied() else { return Err(Fail::Bad) };
        for i in 1..work.len() {
            let right = *work.get(i).ok_or(Fail::Bad)?;
            let dst = work.get_mut(i.saturating_sub(1)).ok_or(Fail::Bad)?;
            *dst = tri::zip4(left, right, |a, b| (1.0 - t) * a + t * b);
            left = right;
        }
        work.pop();
    }
    work.first().copied().ok_or(Fail::Bad)
}

impl Ctx {
    fn eval_attribute(&self, target: i32, u: f32, v: Option<f32>) -> Res<Option<[f32; 4]>> {
        if !self.cap(target.cast_unsigned()) { return Ok(None); }
        let (initial, _) = initial(target).ok_or(Fail::Bad)?;
        let value = if let Some(map) = self.evaluator_maps.get(&target) {
            match (map.domain.as_slice(), v) {
                ([lo, hi], None) => curve_value(&map.coefficients, initial.len(), (u - lo) / (hi - lo))?,
                ([u0, u1, v0, v1], Some(v)) => {
                    let order = usize::try_from(float_to_int(*map.order.get(1).ok_or(Fail::Bad)?)).ok().ok_or(Fail::Bad)?;
                    let stride = order.checked_mul(initial.len()).filter(|&n| n > 0).ok_or(Fail::Bad)?;
                    let mut rows = Vec::new();
                    for row in map.coefficients.chunks_exact(stride) {
                        rows.extend(curve_value(row, initial.len(), (v - v0) / (v1 - v0))?);
                    }
                    curve_value(&rows, 4, (u - u0) / (u1 - u0))?
                }
                _ => return Err(Fail::Bad),
            }
        } else { curve_value(initial, initial.len(), 0.0)? };
        Ok(Some(value))
    }

    pub(super) fn eval_coord(&mut self, u: f32, v: Option<f32>) -> Res<()> {
        if self.in_begin.is_none() { return Ok(()); }
        let base: i32 = if v.is_some() { 0xdb0 } else { 0xd90 };
        let attribute = |offset| self.eval_attribute(base.saturating_add(offset), u, v);
        let vertex = if let Some(v) = attribute(8)? { v }
            else if let Some(v) = attribute(7)? { v }
            else { return Ok(()) };
        if v.is_some() && self.cap(0xd80) { return Err(Fail::Unmodeled(GL_EVAL)); }
        let (color, normal, index, tex, materials) = (self.color, self.normal, self.index, self.tex.coord, self.materials);
        let color_map = attribute(0)?;
        let normal_map = attribute(2)?;
        let index_map = attribute(1)?;
        let texture_map = (3..=6).rev().find(|t| self.cap(base.saturating_add(*t).cast_unsigned()))
            .map(attribute).transpose()?.flatten();
        if let Some(v) = color_map { self.color = v; self.track_current_color(); }
        if let Some([x, y, z, _]) = normal_map { self.normal = [x, y, z]; }
        if let Some(v) = index_map { self.index = v[0]; }
        if let Some(v) = texture_map { self.tex.coord = v; }
        let result = self.call("glVertex4f", &vertex.map(|v| super::A::F(f64::from(v))));
        (self.color, self.normal, self.index, self.tex.coord, self.materials) = (color, normal, index, tex, materials);
        result
    }
}

impl Ctx {
    pub(super) fn eval_mesh2(&mut self, mode: i64, i1: i64, i2: i64, j1: i64, j2: i64) -> Res<()> {
        use super::A::I;
        if (0x1b00..=0x1b02).contains(&mode)
            && (i1 > i2 || j1 > j2 || !(self.cap(0xdb7) || self.cap(0xdb8))) {
            return Ok(());
        }
        let point = |ctx: &mut Self, i, j| ctx.call("glEvalPoint2", &[I(i), I(j)]);
        match mode {
            0x1b00 => {
                self.call("glBegin", &[I(0)])?;
                for j in j1..=j2 {
                    for i in i1..=i2 { point(self, i, j)?; }
                }
                self.call("glEnd", &[])?;
            }
            0x1b01 => {
                for j in j1..=j2 {
                    self.call("glBegin", &[I(3)])?;
                    for i in i1..=i2 { point(self, i, j)?; }
                    self.call("glEnd", &[])?;
                }
                for i in i1..=i2 {
                    self.call("glBegin", &[I(3)])?;
                    for j in j1..=j2 { point(self, i, j)?; }
                    self.call("glEnd", &[])?;
                }
            }
            0x1b02 => {
                for j in j1..j2 {
                    self.call("glBegin", &[I(8)])?;
                    for i in i1..=i2 {
                        point(self, i, j)?;
                        point(self, i, j.saturating_add(1))?;
                    }
                    self.call("glEnd", &[])?;
                }
            }
            _ => self.error(INVALID_ENUM),
        }
        Ok(())
    }
}
