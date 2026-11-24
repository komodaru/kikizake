//! OpenGL 1.1 pixel transfer tables. The first two store signed integers,
//! the remaining eight store single-precision colors.
use super::{Ctx, Fail, Res, array, e, float_to_int, index_bits, query_bytes};
use crate::spec::{clause, hit};

#[derive(Clone, Debug)]
pub(super) struct Maps([Vec<f64>; 10]);

impl Default for Maps {
    fn default() -> Self { Self(std::array::from_fn(|_| vec![0.0])) }
}

fn slot(token: &str) -> Res<Option<usize>> {
    let name = crate::case::windows_long(token).ok_or(Fail::Bad)?;
    Ok(name.checked_sub(0xc70).and_then(|n| usize::try_from(n).ok()).filter(|n| *n < 10))
}

impl Maps {
    pub(super) fn size(&self, name: u32) -> i64 {
        usize::try_from(name.saturating_sub(0xc70)).ok().and_then(|i| self.0.get(i))
            .and_then(|v| i64::try_from(v.len()).ok()).unwrap_or(0)
    }

    pub(super) fn indexed(&self, slot: usize, index: f64) -> f64 {
        let Some(table) = self.0.get(slot) else { return 0.0 };
        let at = usize::try_from(index_bits(index)).unwrap_or(0) & table.len().saturating_sub(1);
        table.get(at).copied().unwrap_or_default()
    }

    pub(super) fn index_rgba(&self, index: f64) -> [f64; 4] {
        [2, 3, 4, 5].map(|slot| self.indexed(slot, index))
    }

    pub(super) fn channel(&self, channel: usize, value: f64) -> f64 {
        let Some(table) = self.0.get(6usize.saturating_add(channel)) else { return 0.0 };
        let last = table.len().saturating_sub(1);
        let size = u32::try_from(table.len()).unwrap_or(0);
        let at = index_bits(value.clamp(0.0, 1.0) * f64::from(size));
        table.get(usize::try_from(at).unwrap_or(0).min(last)).copied().unwrap_or_default()
    }
}

#[derive(Clone, Copy)]
enum Element { Float, Uint, Ushort }

#[test]
fn all_unsigned_short_map_entries_round_trip() {
    for word in 0..=u16::MAX {
        for integer in [false, true] {
            let input = word.to_le_bytes();
            let value = Element::Ushort.decode(&input, integer).unwrap();
            let mut output = [0; 2];
            Element::Ushort.encode(value, integer, &mut output);
            assert_eq!(output, input);
        }
    }
}

impl Element {
    fn of(name: &str) -> Self {
        if name.ends_with("usv") { Self::Ushort } else if name.ends_with("uiv") { Self::Uint } else { Self::Float }
    }
    fn width(self) -> usize { if matches!(self, Self::Ushort) { 2 } else { 4 } }

    fn decode(self, bytes: &[u8], integer: bool) -> Res<f64> {
        let bad = |_| Fail::Bad;
        Ok(match self {
            Self::Float => {
                let value = f32::from_le_bytes(bytes.try_into().map_err(bad)?);
                if integer { f64::from(i32::try_from(float_to_int(value.round())).unwrap_or(i32::MIN)) }
                else { f64::from(value.clamp(0.0, 1.0)) }
            }
            Self::Uint => {
                let value = u32::from_le_bytes(bytes.try_into().map_err(bad)?);
                if integer { f64::from(value.cast_signed()) } else { f64::from(array::color_component(e::UNSIGNED_INT, f64::from(value))) }
            }
            Self::Ushort => {
                let value = u16::from_le_bytes(bytes.try_into().map_err(bad)?);
                if integer { f64::from(value) } else { f64::from(array::color_component(e::UNSIGNED_SHORT, f64::from(value))) }
            }
        })
    }

    fn encode(self, value: f64, integer: bool, bytes: &mut [u8]) {
        let f = super::tri::narrow(value);
        match self {
            Self::Float => bytes.copy_from_slice(&f.to_le_bytes()),
            Self::Uint => {
                // The measured u32 scale is nine f32 steps below 2^32.
                let n = if integer { index_bits(value) } else {
                    u32::try_from(super::tri::trunc((f * f32::from_bits(0x4f7f_fff7)).round())).unwrap_or(u32::MAX)
                };
                bytes.copy_from_slice(&n.to_le_bytes());
            }
            Self::Ushort => {
                let n = if integer { u16::try_from(index_bits(value) & 0xffff).unwrap_or(0) }
                        else { u16::try_from(super::tri::trunc((f * 65535.0).round())).unwrap_or(u16::MAX) };
                bytes.copy_from_slice(&n.to_le_bytes());
            }
        }
    }
}

impl Ctx {
    pub(super) fn set_pixel_map(&mut self, t: &[String]) -> Res<()> {
        hit(clause::GL_PIXELMAP);
        let [_, call, sig, name, size, ..] = t else { return Err(Fail::Bad) };
        if sig != "ii" { return Err(Fail::Bad); }
        let size = crate::case::windows_long(size).ok_or(Fail::Bad)?;
        if self.in_begin.is_some() { self.error(e::INVALID_OPERATION); return Ok(()); }
        let Some(slot) = slot(name)? else { self.error(e::INVALID_ENUM); return Ok(()) };
        if !(0..=65536).contains(&size) || (slot < 6 && !size.cast_unsigned().is_power_of_two()) {
            self.error(e::INVALID_VALUE);
            return Ok(());
        }
        if size == 0 {
            *self.pixel_maps.0.get_mut(slot).ok_or(Fail::Bad)? = vec![0.0];
            return Ok(());
        }
        let mut data = match array::glp_bytes(t)? {
            Some(values) => values,
            None if t.get(5).is_some_and(|s| s == "obuf") => vec![0xcc; 4096],
            None => return Err(Fail::Bad),
        };
        data.resize(4096, 0);
        let el = Element::of(call);
        let n = usize::try_from(size).ok().and_then(|s| s.checked_mul(el.width())).ok_or(Fail::Bad)?;
        let values = data.get(..n).ok_or(Fail::Bad)?.chunks_exact(el.width())
            .map(|b| el.decode(b, slot < 2)).collect::<Res<Vec<_>>>()?;
        *self.pixel_maps.0.get_mut(slot).ok_or(Fail::Bad)? = values;
        Ok(())
    }

    pub(super) fn get_pixel_map(&mut self, t: &[String], out: &mut Vec<String>) -> Res<()> {
        hit(clause::GL_PIXELMAP);
        let [_, call, sig, name, ty, count] = t else { return Err(Fail::Bad) };
        if sig != "i" { return Err(Fail::Bad); }
        out.push(query_bytes(ty, count, |buf| {
            if self.in_begin.is_some() { self.error(e::INVALID_OPERATION); return Ok(()); }
            let Some(slot) = slot(name)? else { self.error(e::INVALID_ENUM); return Ok(()) };
            let el = Element::of(call);
            let values = self.pixel_maps.0.get(slot).ok_or(Fail::Bad)?;
            if values.len().saturating_mul(el.width()) > buf.len() { return Err(Fail::Bad); }
            for (bytes, &value) in buf.chunks_exact_mut(el.width()).zip(values) {
                el.encode(value, slot < 2, bytes);
            }
            Ok(())
        })?);
        Ok(())
    }
}
