use kikizake::model::ntcase::{downcase, towlower, towupper, upcase};
use kikizake::model::ntstr::{
    Status, UString, ansi_to_unicode, char_to_integer, integer_to_char, integer_to_unicode,
    unicode_to_integer,
};
use proptest::prelude::*;

#[test]
fn case_maps_are_inverse_pairs() {
    let mut pairs = 0;
    for unit in 0..=u16::MAX {
        let upper = upcase(unit);
        let lower = downcase(unit);
        if upper != unit {
            pairs += 1;
            assert_eq!(downcase(upper), unit);
            assert_eq!(lower, unit);
        }
        if lower != unit {
            assert_eq!(upcase(lower), unit);
            assert_eq!(upper, unit);
        }
        assert_eq!(towupper(unit), upper);
        let expected = match unit {
            0x41..=0x5a | 0xc0..=0xd6 | 0xd8..=0xde => unit + 32,
            other => other,
        };
        assert_eq!(towlower(unit), expected);
    }
    assert_eq!(pairs, 973);
}

// Every possible leading unit, followed by a decimal digit. This distinguishes
// signed ANSI whitespace from UTF-16 whitespace, including a leading NUL.
#[test]
fn every_leading_unit_before_a_digit() {
    for unit in 0..=u16::MAX {
        let expected = match unit {
            0..=32 | 43 => 7,
            45 => 7u32.wrapping_neg(),
            48..=57 => u32::from(unit - 48) * 10 + 7,
            _ => 0,
        };
        assert_eq!(unicode_to_integer(&[unit, 55], 4, 10), (Status::Success, expected));
        if let Ok(byte) = u8::try_from(unit) {
            let ansi = if byte >= 128 { 7 } else { expected };
            assert_eq!(char_to_integer(&[byte, 55], 10), (Status::Success, Some(ansi)));
        }
    }
}

fn formatted(value: u32, base: u32) -> Option<String> {
    Some(match base {
        0 | 10 => value.to_string(),
        2 => format!("{value:b}"),
        8 => format!("{value:o}"),
        16 => format!("{value:X}"),
        _ => return None,
    })
}

fn check_unicode(digits: Option<&[u8]>, max: u16, null: bool, convert: impl FnOnce(&mut UString) -> Status) {
    let mut actual = [0xcc; 80];
    let mut expected = actual;
    let fits = digits.filter(|d| !null && 2 * (d.len() + 1) <= usize::from(max));
    let (status, length) = if let Some(d) = fits {
        for (i, byte) in d.iter().copied().chain([0]).enumerate() {
            expected[2 * i..2 * i + 2].copy_from_slice(&u16::from(byte).to_le_bytes());
        }
        (Status::Success, (2 * d.len()) as u16)
    } else if digits.is_none() {
        (Status::InvalidParameter, 0x5555)
    } else {
        (Status::BufferOverflow, 0x5555)
    };
    let mut dst = UString { length: 0x5555, maximum: max, buffer: (!null).then_some(&mut actual) };
    assert_eq!(convert(&mut dst), status);
    assert_eq!((dst.length, dst.maximum, dst.buffer.is_none()), (length, max, null));
    assert_eq!(actual, expected);
}

#[test]
fn ascii_counted_output_boundaries() {
    let source: Vec<u8> = (0..79).map(|i| ((i * 37) % 128) as u8).collect();
    for len in 0..=source.len() {
        for max in 0..=80 {
            for null in [false, true] {
                let bytes = &source[..len];
                check_unicode(Some(bytes), max, null, |dst| ansi_to_unicode(dst, bytes));
            }
        }
    }
}

fn check_integer(value: u32, base: u32, len: i32, max: u16, null: bool) {
    let digits = formatted(value, base);
    let mut actual = [0xcc; 40];
    let mut expected = actual;
    let capacity = len.unsigned_abs() as usize;
    let status = match digits.as_deref() {
        None => Status::InvalidParameter,
        Some(d) if d.len() > capacity => Status::BufferOverflow,
        Some(d) => {
            if len < 0 {
                let padded = format!("{d:0>capacity$}");
                expected[..capacity].copy_from_slice(padded.as_bytes());
            } else {
                expected[..d.len()].copy_from_slice(d.as_bytes());
                if d.len() < capacity {
                    expected[d.len()] = 0;
                }
            }
            Status::Success
        }
    };
    assert_eq!(integer_to_char(value, base, len, &mut actual), status);
    assert_eq!(actual, expected);
    check_unicode(digits.as_deref().map(str::as_bytes), max, null, |dst| integer_to_unicode(value, base, dst));
}

proptest! {
    #[test]
    fn integer_format_contract(value in any::<u32>(), base in prop_oneof![Just(0u32), Just(2), Just(8), Just(10), Just(16), any::<u32>()],
                               len in -40i32..=40, max in 0u16..=80, null in any::<bool>()) {
        check_integer(value, base, len, max, null);
    }
}

#[test]
fn integer_digit_boundaries() {
    for base in [0, 2, 8, 10, 16, 3] {
        let radix = if base == 0 { 10 } else { base };
        let mut power = 1u32;
        loop {
            for value in [power - 1, power, power + 1, u32::MAX] {
                for len in -40i32..=40 {
                    check_integer(value, base, len, (len + 40) as u16, false);
                }
            }
            let Some(next) = power.checked_mul(radix) else { break };
            power = next;
        }
    }
}

proptest! {
    #[test]
    fn signed_prefixed_numbers_wrap(value in any::<u64>(), negative in any::<bool>(),
                                   radix in prop_oneof![Just(2u32), Just(8), Just(10), Just(16)]) {
        let (prefix, digits) = match radix {
            2 => ("0b", format!("{value:b}")),
            8 => ("0o", format!("{value:o}")),
            16 => ("0x", format!("{value:x}")),
            _ => ("", value.to_string()),
        };
        let sign = if negative { '-' } else { '+' };
        let expected = if negative { (value as u32).wrapping_neg() } else { value as u32 };
        for (base, prefix) in [(0, prefix), (radix, "")] {
            let text = format!(" \t{sign}{prefix}{digits}!9");
            let wide: Vec<_> = text.encode_utf16().collect();
            prop_assert_eq!(char_to_integer(text.as_bytes(), base), (Status::Success, Some(expected)));
            prop_assert_eq!(unicode_to_integer(&wide, 2 * wide.len(), base), (Status::Success, expected));
        }
    }
}
