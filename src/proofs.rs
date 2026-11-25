//! Kani proofs (`cargo kani`): clauses' arithmetic proved for every input of the stated domain,
//! independent of Windows' answers (spec/laws.txt lists them with the clauses they prove). The
//! models' own functions are called, so a proof fails when the model breaks the clause.
#![cfg(kani)]

use crate::model::{d3d7::{tri_z, Depth}, gdi::sample, gdiblend::alpha, gdixform::page};

fn ch(v: u32, i: u32) -> u32 {
    v >> (8 * i) & 255
}

/// GDI.STRETCH.SAMPLE / DDA: each destination pixel takes one source pixel inside the source,
/// in order (never going back), and equal extents take each pixel itself (GDI.STRETCH.SAME);
/// extents up to 64, a bound that keeps the division within the solver's reach
#[kani::proof]
fn stretch_samples_the_source_in_order() {
    let (i, sn, n, flat): (i32, i32, i32, bool) = (kani::any(), kani::any(), kani::any(), kani::any());
    kani::assume(sn >= 1 && sn <= 64 && n >= 1 && n <= 64 && i >= 0 && i < n - 1);
    let (a, b) = (sample(i, sn, n, flat), sample(i + 1, sn, n, flat));
    assert!(a >= 0 && b < sn && a <= b);
    if n == sn {
        assert!(a == i && b == i + 1);
    }
}

/// GDI.ALPHA.CONSTANT / ROUND: with a constant alpha each channel lies between the source's and
/// the destination's, 255 gives the source and 0 the destination
#[kani::proof]
fn constant_alpha_lies_between() {
    let (s, d, sca): (u32, u32, u32) = (kani::any(), kani::any(), kani::any());
    kani::assume(sca <= 255);
    let r = alpha(s, d, sca, false);
    for i in 0..4 {
        let (x, y, z) = (ch(s, i), ch(d, i), ch(r, i));
        assert!(z >= x.min(y) && z <= x.max(y));
        if sca == 255 {
            assert!(z == x);
        }
        if sca == 0 {
            assert!(z == y);
        }
    }
}

/// GDI.ALPHA.OPAQUE: a per-pixel source of alpha 255 blends as the constant alpha alone
#[kani::proof]
fn opaque_pixels_blend_as_constant() {
    let (s, d, sca): (u32, u32, u32) = (kani::any(), kani::any(), kani::any());
    kani::assume(sca <= 255 && s >> 24 == 255);
    assert!(alpha(s, d, sca, true) == alpha(s, d, sca, false));
}

/// D3D7.TRI / CLEAR.Z: a triangle's z value grows with z, 0 at 0 and the largest value at 1
#[kani::proof]
fn triangle_z_is_monotone() {
    let (z1, z2, wide): (f32, f32, bool) = (kani::any(), kani::any(), kani::any());
    kani::assume(z1 >= 0.0 && z1 <= z2 && z2 <= 1.0);
    let bits = if wide { Depth::Z32 } else { Depth::Z16 };
    assert!(tri_z(bits, z1) <= tri_z(bits, z2));
    if z2 == 1.0 {
        assert!(tri_z(bits, z2) == if wide { 0xffff_ff00 } else { 0xffff });
    }
    if z1 == 0.0 {
        assert!(tri_z(bits, z1) == 0);
    }
    // (the 16-bit z is the top of the 32-bit one)
    assert!(tri_z(Depth::Z16, z1) == tri_z(Depth::Z32, z1) >> 16);
}

// The ntstr runner allows 0..79 ASCII bytes and a destination of 0..80
// bytes. One symbolic output position checks every written and untouched byte.
#[kani::proof]
#[kani::unwind(81)]
fn nt_counted_ascii_output() {
    use crate::model::ntstr::{Status, UString, ansi_to_unicode};
    let input: [u8; 79] = kani::any();
    let (len, maximum, index): (u8, u8, u8) = kani::any();
    kani::assume(len <= 79 && maximum <= 80 && index < 80);
    let len = usize::from(len);
    let index = usize::from(index);
    let null: bool = kani::any();
    let initial: u8 = kani::any();
    let original_length: u16 = kani::any();
    let mut buf = [initial; 80];
    let mut dst = UString {
        length: original_length,
        maximum: u16::from(maximum),
        buffer: (!null).then_some(&mut buf),
    };
    let result = ansi_to_unicode(&mut dst, &input[..len]);
    let fits = !null && 2 * (len + 1) <= usize::from(maximum);
    assert_eq!(dst.maximum, u16::from(maximum));
    assert_eq!(dst.buffer.is_none(), null);
    if fits {
        assert_eq!(result, Status::Success);
        assert_eq!(usize::from(dst.length), len * 2);
    } else {
        assert_eq!(result, Status::BufferOverflow);
        assert_eq!(dst.length, original_length);
    }
    let expected = if !fits || index >= 2 * (len + 1) {
        initial
    } else if index % 2 == 1 || index / 2 == len {
        0
    } else {
        input[index / 2]
    };
    assert_eq!(buf[index], expected);
}

// The oracle computes the mathematical end in u128, even when the destination
// offset and extent would overflow u64. All three dimensions are independent.
fn fits_box(b: [u64; 6], at: [u64; 3], size: [u64; 3]) -> bool {
    (0..3).all(|i| {
        let width = u128::from(b[i + 3]).saturating_sub(u128::from(b[i]));
        u128::from(at[i]) + width <= u128::from(size[i])
    })
}

#[kani::proof]
#[kani::unwind(5)]
fn d3d_update_respects_box_and_subresource() {
    use crate::model::d3dres::{Plan, update_plan};
    let (sub, nsubs, usage, count) = kani::any();
    let (b, top): ([u64; 6], [u64; 3]) = kani::any();
    let nonempty = (0..3).all(|i| b[i] < b[i + 3]);
    let expected = if sub >= nsubs || usage != 0 || count > 1 || !nonempty {
        Plan::Nothing
    } else if fits_box(b, [b[0], b[1], b[2]], top) {
        Plan::Copy
    } else {
        Plan::Removed
    };
    assert_eq!(update_plan(sub, nsubs, usage, count, b, top), expected);
}

#[kani::proof]
#[kani::unwind(5)]
fn d3d_region_respects_both_subresources() {
    use crate::model::d3dres::{Plan, Sides, region_plan};
    let (b, src, dst, at): ([u64; 6], [u64; 3], [u64; 3], [u64; 3]) = kani::any();
    let (ignored, src_buffer, dst_buffer, compatible): (bool, bool, bool, bool) = kani::any();
    let sides = match (src_buffer, dst_buffer) {
        (true, true) => Sides::Buffers,
        (false, false) => Sides::Textures { compatible },
        _ => Sides::Mixed,
    };
    let expected = if ignored || src_buffer != dst_buffer || (!src_buffer && !compatible) {
        Plan::Nothing
    } else if !fits_box(b, [b[0], b[1], b[2]], src) || !fits_box(b, at, dst) {
        Plan::Removed
    } else if (0..3).all(|i| b[i] < b[i + 3]) {
        Plan::Copy
    } else {
        Plan::Nothing
    };
    assert_eq!(region_plan(ignored, sides, b, src, dst, at), expected);
}

#[kani::proof]
fn nt_digit_step_matches_modular_arithmetic() {
    use crate::model::ntstr::{Radix, append_digit};
    let value: u32 = kani::any();
    let unit: u16 = kani::any();
    let radix = match kani::any::<u8>() {
        0 => Radix::Bin,
        1 => Radix::Oct,
        2 => Radix::Dec,
        _ => Radix::Hex,
    };
    let expected = char::from_u32(u32::from(unit))
        .and_then(|c| c.to_digit(radix.value()))
        .map(|d| ((u64::from(value) * u64::from(radix.value()) + u64::from(d)) % (1 << 32)) as u32);
    assert_eq!(append_digit(value, radix, u32::from(unit)), expected);
}

#[kani::proof]
#[kani::unwind(6)]
fn nt_invalid_base_preserves_ansi_output_and_zeroes_unicode() {
    use crate::model::ntstr::{Status, char_to_integer, unicode_to_integer};
    let base: u32 = kani::any();
    kani::assume(!matches!(base, 0 | 2 | 8 | 10 | 16));
    let ansi: [u8; 4] = kani::any();
    let units: [u16; 4] = kani::any();
    let length: u8 = kani::any();
    kani::assume(length <= 8);
    assert_eq!(char_to_integer(&ansi, base), (Status::InvalidParameter, None));
    assert_eq!(unicode_to_integer(&units, usize::from(length), base), (Status::InvalidParameter, 0));
}

#[kani::proof]
#[kani::unwind(42)]
fn nt_integer_output_writes_only_its_digits_pad_and_terminator() {
    use crate::model::ntstr::{Status, write_digits};
    let digits: [u8; 32] = kani::any();
    let original: [u8; 40] = kani::any();
    let (count, length, index): (u8, i8, u8) = kani::any();
    kani::assume((1..=32).contains(&count) && (-40..=40).contains(&length) && index < 40);
    let count = usize::from(count);
    let index = usize::from(index);
    let capacity = usize::from(length.unsigned_abs());
    let mut actual = original;
    let status = write_digits(&digits[..count], i32::from(length), &mut actual);
    if capacity < count {
        assert_eq!(status, Status::BufferOverflow);
        assert_eq!(actual[index], original[index]);
    } else {
        assert_eq!(status, Status::Success);
        let start = if length < 0 { capacity - count } else { 0 };
        let expected = if index < start {
            b'0'
        } else if index - start < count {
            digits[index - start]
        } else if length >= 0 && index == count && capacity > count {
            0
        } else {
            original[index]
        };
        assert_eq!(actual[index], expected);
    }
}

#[kani::proof]
#[kani::unwind(2)]
fn nt_rejected_integer_calls_leave_the_specified_outputs() {
    use crate::model::ntstr::{Status, UString, integer_to_char, integer_to_unicode, unicode_to_integer};
    let (value, base, byte, length, maximum): (u32, u32, u8, u16, u16) = kani::any();
    kani::assume(!matches!(base, 0 | 2 | 8 | 10 | 16));
    let mut buf = [byte];
    assert_eq!(integer_to_char(value, base, kani::any(), &mut buf), Status::InvalidParameter);
    assert_eq!(buf, [byte]);
    let mut dst = UString { length, maximum, buffer: Some(&mut buf) };
    assert_eq!(integer_to_unicode(value, base, &mut dst), Status::InvalidParameter);
    assert_eq!((dst.length, dst.maximum), (length, maximum));
    assert_eq!(buf, [byte]);
    let invalid_length: u16 = kani::any();
    kani::assume(invalid_length == 0 || invalid_length % 2 == 1);
    assert_eq!(unicode_to_integer(&[], usize::from(invalid_length), kani::any()), (Status::InvalidParameter, 0));
}

// Nine hex digits wrap modulo 2^32. The high digit is discarded, the next
// eight encode an arbitrary u32, and a non-digit ends both forms of the call.
#[kani::proof]
#[kani::unwind(16)]
fn nt_signed_hexadecimal_parse() {
    use crate::model::ntstr::{Status, char_to_integer, unicode_to_integer};
    let value: u32 = kani::any();
    let negative: bool = kani::any();
    let mut bytes = *b"+0xf00000000!";
    bytes[0] = if negative { b'-' } else { b'+' };
    for i in 0..8 {
        let nibble = ((value >> (28 - 4 * i)) & 15) as u8;
        bytes[4 + i] = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };
    }
    let expected = if negative { value.wrapping_neg() } else { value };
    assert_eq!(char_to_integer(&bytes, 0), (Status::Success, Some(expected)));
    let wide = bytes.map(u16::from);
    assert_eq!(unicode_to_integer(&wide, wide.len() * 2, 0), (Status::Success, expected));
    assert_eq!(char_to_integer(&bytes, 16), (Status::Success, Some(0)));
    assert_eq!(unicode_to_integer(&wide, wide.len() * 2, 16), (Status::Success, 0));
}

// Every 32-bit value in hexadecimal, including zero. Derive the length from
// its highest bit and each character from its nibble, independently of digits().
#[kani::proof]
#[kani::unwind(35)]
fn nt_hex_digits_and_unicode_destination() {
    use crate::model::ntstr::{Status, UString, integer_to_char, integer_to_unicode};
    let value: u32 = kani::any();
    let n = (value.bit_width().div_ceil(4) as usize).max(1);
    let i: u8 = kani::any();
    kani::assume(usize::from(i) < n);
    let i = usize::from(i);
    let nibble = ((value >> (4 * (n - 1 - i))) & 15) as u8;
    let digit = if nibble < 10 { b'0' + nibble } else { b'A' + (nibble - 10) };

    let mut ansi = [0xcc; 9];
    assert_eq!(integer_to_char(value, 16, 9, &mut ansi), Status::Success);
    assert_eq!(ansi[i], digit);
    assert_eq!(ansi[n], 0);

    let mut wide = [0xcc; 18];
    let mut dst = UString { length: 0x5555, maximum: 18, buffer: Some(&mut wide) };
    assert_eq!(integer_to_unicode(value, 16, &mut dst), Status::Success);
    assert_eq!(usize::from(dst.length), 2 * n);
    assert_eq!((wide[2 * i], wide[2 * i + 1]), (digit, 0));
    assert_eq!((wide[2 * n], wide[2 * n + 1]), (0, 0));
}

/// For every pair of finite positive workspace scales, a unit tolerance within
/// the extent floor follows the dominant axis, including a tie and subnormals.
#[kani::proof]
fn d2d_unit_curve_tolerance_uses_the_dominant_axis() {
    use crate::model::d2d::area_tolerance;
    let (sx, sy): (f64, f64) = kani::any();
    kani::assume(sx.is_finite() && sy.is_finite() && sx > 0.0 && sy > 0.0);
    let expected = if sx >= sy { sx } else { sy };
    assert_eq!(area_tolerance(1.0, 1.0, sx, sy), expected);
    assert_eq!(area_tolerance(1.0, 1.0, sy, sx), expected);
}

/// The two 1:16 path conversions against IEEE binary32 for every logical LONG.
#[kani::proof]
fn mapped_ellipse_coordinate_conversions() {
    use crate::model::gdipath::{sixteenths_to_logical, sixteenths_to_pixel};
    let x: i32 = kani::any();
    let rounded = (x as f32) as i64;
    assert_eq!(i64::from(sixteenths_to_pixel(x)), (rounded + 8).div_euclid(16));
    assert_eq!(sixteenths_to_logical(x), rounded as i32);
}

/// Every stored brush value and horizontal DWORD position in the packed DIB formats.
#[kani::proof]
fn polygon_notcopy_uses_packed_brush_word() {
    use crate::model::gdiline::polygon_notcopy_source;
    let x: u8 = kani::any();
    let bpp: u32 = kani::any();
    kani::assume(matches!(bpp, 1 | 4 | 8 | 16));
    let brush: u32 = kani::any();
    let mask = (1_u32 << bpp) - 1;
    kani::assume(brush <= mask);
    let pos = (u32::from(x) * bpp) % 32;
    let shift = if bpp < 8 { (pos / 8) * 8 + (8 - bpp - pos % 8) } else { pos };
    assert_eq!(polygon_notcopy_source(bpp, i32::from(x), brush), (brush >> shift) & mask);
}

/// A triangle with one vertex outside a user plane acquires two cut vertices.
/// For opposite unit distances, each cut is the exact midpoint of the edge,
/// including its varying attribute. All signed byte attributes are represented.
#[kani::proof]
#[kani::unwind(6)]
fn clip_plane_triangle_preserves_edge_interpolants() {
    use crate::model::opengl::tri::clip_halfspace;
    let a = f32::from(kani::any::<i8>());
    let b = f32::from(kani::any::<i8>());
    let c = f32::from(kani::any::<i8>());
    let triangle = [[-1.0, a], [1.0, b], [1.0, c]];
    let (vertices, crossings) = clip_halfspace(&triangle, |v| v[0]);
    assert_eq!(crossings, 2);
    assert_eq!(vertices.len(), 4);
    assert_eq!(vertices[0], [0.0, (a + c) / 2.0]);
    assert_eq!(vertices[1], [0.0, (a + b) / 2.0]);
    assert_eq!(vertices[2], triangle[1]);
    assert_eq!(vertices[3], triangle[2]);
}

/// For every pair of signed byte distances, keeping and rejecting agree with
/// half-space membership. A crossing cuts the outside endpoint with a fraction
/// in [0,1], including an inside endpoint exactly on the plane.
#[kani::proof]
fn clip_line_classifies_both_ends() {
    use crate::model::opengl::tri::{line_plane_cut, LinePlaneCut};
    let a = kani::any::<i8>();
    let b = kani::any::<i8>();
    match line_plane_cut([f32::from(a), f32::from(b)]) {
        LinePlaneCut::Keep => assert!(a >= 0 && b >= 0),
        LinePlaneCut::Reject => assert!(a < 0 && b < 0),
        LinePlaneCut::Cut { outside, t } => {
            assert!(if outside == 0 { a < 0 && b >= 0 } else { outside == 1 && b < 0 && a >= 0 });
            assert!(t >= 0.0 && t <= 1.0);
            assert_eq!(t == 0.0, a == 0 || b == 0);
        }
    }
}

/// Every bit in the native return word represents exactly one submitted DIB DC.
#[kani::proof]
fn wgl_swap_result_identifies_every_entry() {
    let count: u32 = kani::any();
    let bit: u32 = kani::any();
    kani::assume(count <= 16 && bit < u32::BITS);
    let result = crate::model::opengl::low_bits(count);
    assert_eq!((result >> bit) & 1 != 0, bit < count);
}

/// For every GLuint base and signed count, accepted font batches have exactly
/// count nonzero names, and every offset used to build a list is in range.
#[kani::proof]
fn wgl_font_batch_has_exact_nonzero_names() {
    let base: u32 = kani::any();
    let count: i32 = kani::any();
    let offset: u32 = kani::any();
    let result = crate::model::opengl::wgl_font_last_name(base, count);
    let end = i64::from(base) + i64::from(count) - 1;
    assert_eq!(result.is_some(), base != 0 && (1..=64).contains(&count) && end <= i64::from(u32::MAX));
    if let Some(last) = result {
        assert_eq!(i64::from(last), end);
        if offset < count.cast_unsigned() {
            let name = base + offset;
            assert!(name != 0 && name <= last);
            assert_eq!(name - base, offset);
        }
    }
}

/// For every pair of DWORDs the glyph selector is their sum modulo
/// the ANSI byte or the Unicode word, including DWORD wraparound.
#[kani::proof]
fn wgl_outline_code_matches_modular_addition() {
    let first: u32 = kani::any();
    let offset: u32 = kani::any();
    let wide: bool = kani::any();
    let expected = (u64::from(first) + u64::from(offset)) % if wide { 65536 } else { 256 };
    assert_eq!(u64::from(crate::model::opengl::wgl_outline_code(first, offset, wide)), expected);
}

#[kani::proof]
fn tracked_material_matches_face_and_property_masks() {
    use crate::model::opengl::e::{BACK, FRONT, FRONT_AND_BACK};
    let enabled: bool = kani::any();
    let face: u32 = kani::any();
    let mode: u32 = kani::any();
    let back: bool = kani::any();
    let property: u32 = kani::any();
    let faces = match face { FRONT => 1u8, BACK => 2, FRONT_AND_BACK => 3, _ => 0 };
    let mask = |p| match p { 0x1200 => 1u8, 0x1201 => 2, 0x1202 => 4, 0x1600 => 8, 0x1602 => 3, _ => 0 };
    kani::assume(matches!(property, 0x1200..=0x1202 | 0x1600));
    let expected = enabled && faces & (1 << u8::from(back)) != 0 && mask(mode) & mask(property) != 0;
    let target = if back { BACK } else { FRONT };
    assert_eq!(crate::model::opengl::tracks_material(enabled, (face, mode), target, property), expected);
}

#[kani::proof]
fn layer_palette_rejects_every_nonzero_plane() {
    let plane: i32 = kani::any();
    let result = crate::model::opengl::wgl_palette_error(plane);
    assert_eq!(result == 0, plane == 0);
    assert_eq!(result == 87, plane != 0);
}

#[kani::proof]
#[kani::unwind(4)]
fn evaluator_grid_updates_both_axes_atomically() {
    use crate::model::opengl::{set_grid, GridAxis};
    let axis = || GridAxis { segments: i64::from(kani::any::<i32>()), domain: kani::any::<[u32; 2]>().map(f32::from_bits) };
    let before = [axis(), axis()];
    let next = [axis(), axis()];
    let mut grid = before;
    let valid = next[0].segments > 0 && next[1].segments > 0;
    assert_eq!(set_grid(&mut grid, next), valid);
    let expected = if valid { next } else { before };
    for (actual, wanted) in grid.iter().zip(expected) {
        assert_eq!(actual.segments, wanted.segments);
        assert_eq!(actual.domain[0].to_bits(), wanted.domain[0].to_bits());
        assert_eq!(actual.domain[1].to_bits(), wanted.domain[1].to_bits());
    }
}

#[kani::proof]
fn evaluator_integer_query_matches_binary32_fields() {
    let bits: u32 = kani::any();
    let magnitude = bits & 0x7fff_ffff;
    let exponent = magnitude >> 23;
    let significand = (magnitude & 0x007f_ffff) | 0x0080_0000;
    let expected = if exponent >= 158 {
        // Includes both signs of 2^31, infinities and every NaN payload.
        i32::MIN
    } else {
        let rounded = match exponent {
            // The predecessor of 0.5 plus 0.5 ties at the midpoint below 1.
            0..=125 => u32::from(magnitude == 0x3eff_ffff),
            126..=149 => {
                let shift = 150 - exponent;
                (significand + (1 << (shift - 1))) >> shift
            }
            // A spacing of one makes the half-unit bias a tie to even.
            150 => significand + (significand & 1),
            _ => significand << (exponent - 150),
        };
        let signed = i32::try_from(rounded).unwrap();
        if bits >> 31 == 0 { signed } else { -signed }
    };
    assert_eq!(crate::model::opengl::eval::query_integer(f32::from_bits(bits)), expected);
}

#[kani::proof]
fn selection_stack_preserves_every_unwritten_name() {
    use crate::model::opengl::{e::{INVALID_OPERATION, STACK_OVERFLOW, STACK_UNDERFLOW}, NameStack};
    let depth: usize = kani::any();
    let index: usize = kani::any();
    kani::assume(depth <= 128 && index < 128);
    let mut stack = NameStack { names: kani::any(), depth };
    let old = stack.names[index];
    let name: u32 = kani::any();
    let op: u8 = kani::any();
    let (result, expected, next, changed) = match op {
        0 if depth == 128 => (stack.push(name), Err(STACK_OVERFLOW), depth, false),
        0 => (stack.push(name), Ok(()), depth + 1, index == depth),
        1 if depth == 0 => (stack.pop(), Err(STACK_UNDERFLOW), depth, false),
        1 => (stack.pop(), Ok(()), depth - 1, false),
        _ if depth == 0 => (stack.load(name), Err(INVALID_OPERATION), depth, false),
        _ => (stack.load(name), Ok(()), depth, index == depth - 1),
    };
    assert_eq!(result, expected);
    assert_eq!(stack.depth, next);
    assert_eq!(stack.names[index], if changed { name } else { old });
}

#[kani::proof]
fn feedback_keeps_the_maximal_prefix_within_capacity() {
    let (written, pending, requested, capacity): (usize, usize, usize, i32) = kani::any();
    let count = crate::model::opengl::feedback_prefix(written, pending, requested, capacity);
    let used = written as u128 + pending as u128;
    let limit = capacity.max(0) as u128;
    assert!(count <= requested);
    if used >= limit { assert_eq!(count, 0); }
    else {
        assert!(used + count as u128 <= limit);
        assert!(count == requested || used + count as u128 == limit);
    }
}

#[kani::proof]
fn selection_depths_preserve_extrema_and_first_hit() {
    use crate::model::opengl::{feedback_prefix, SelectionDepth};

    let (first, min, max, depth, previous): (u32, u32, u32, u32, u32) = kani::any();
    kani::assume(min <= first && first <= max);
    kani::assume(min <= previous && previous <= max);
    let mut hit = SelectionDepth { first, min, max };
    hit.include(depth);
    assert_eq!(hit.first, first);
    assert!(hit.min <= depth && depth <= hit.max);
    assert!(hit.min <= previous && previous <= hit.max);
    assert!(hit.min == min || hit.min == depth);
    assert!(hit.max == max || hit.max == depth);

    let (used, names, capacity): (usize, usize, i32) = kani::any();
    kani::assume(names <= 128);
    let len = names + 3;
    let room = feedback_prefix(0, used, len, capacity);
    let encoded = hit.record_depths(room == len);
    if used <= usize::try_from(capacity).unwrap_or(0).saturating_sub(len)
        && usize::try_from(capacity).unwrap_or(0) >= len
    {
        assert_eq!(room, len);
        assert_eq!(encoded, [hit.min, hit.max]);
    } else {
        assert!(room < len);
        assert_eq!(encoded, [first; 2]);
    }
}

#[kani::proof]
fn signed_light_colors_preserve_sign_and_half_step_symmetry() {
    use crate::model::opengl::signed_light_component;

    let word: i32 = kani::any();
    let color = signed_light_component(word);
    assert_eq!(color.is_sign_negative(), word < 0);
    assert!(color.is_finite() && color != 0.0);
    assert_eq!(color.to_bits() ^ 0x8000_0000, signed_light_component(!word).to_bits());
}

// Two RGBA8 array slices with one or two samples. Each stored byte is symbolic.
#[kani::proof]
#[kani::unwind(12)]
fn d3d_copy_resource_copies_every_subresource_and_sample_plane() {
    use crate::model::d3dres::{Desc, Dev, Resource, copy_resource};

    let (source, target): ([[u8; 4]; 2], [[u8; 4]; 2]) = kani::any();
    let (source_samples, target_samples): ([[u8; 8]; 2], [[u8; 8]; 2]) = kani::any();
    let immutable: bool = kani::any();
    let d3d10: bool = kani::any();
    let sample_count: u32 = if kani::any() { 1 } else { 2 };
    let desc = Desc {
        dim: 2, w: 1, h: 1, da: 2, mips: 1, fmt: 28,
        usage: 0, bind: 0, cpu: 0, misc: 0, count: sample_count, q: 0,
    };
    let storage = |[first, second]: [[u8; 4]; 2]| vec![Some(first.to_vec()), Some(second.to_vec())];
    let planes = |[first, second]: [[u8; 8]; 2]| {
        if sample_count == 2 { vec![Some(first.to_vec()), Some(second.to_vec())] } else { vec![None, None] }
    };
    let src = Resource { d: desc, subs: storage(source), samples: planes(source_samples) };
    let target_desc = Desc { usage: u32::from(immutable), ..desc };
    let mut dst = Resource { d: target_desc, subs: storage(target), samples: planes(target_samples) };
    let dev = if d3d10 { Dev::D10 } else { Dev::D11(0xb000) };
    assert!(copy_resource(dev, &mut dst, &src).is_ok());

    let allowed = !immutable && (!d3d10 || sample_count == 1);
    let expected = if allowed { source } else { target };
    let expected_samples = if allowed { source_samples } else { target_samples };
    assert_eq!(dst.d, target_desc);
    assert_eq!(dst.subs, storage(expected));
    assert_eq!(dst.samples, planes(expected_samples));
}
