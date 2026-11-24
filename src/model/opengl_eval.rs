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
