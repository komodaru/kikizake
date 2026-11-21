//! The model of runner/ntstr.c's ops (spec/ntstr.txt): ntdll's integer <-> string conversions,
//! RtlCharToInteger, RtlUnicodeStringToInteger, RtlIntegerToChar, RtlIntegerToUnicodeString and
//! RtlAnsiStringToUnicodeString (without allocation).
//!
//! Sources (Microsoft Learn): "RtlCharToInteger", "RtlUnicodeStringToInteger",
//! "RtlIntegerToUnicodeString", "RtlAnsiStringToUnicodeString". Where they are silent, the
//! clean-room notes (re/README.md) give the rule, assumed until the golden ntstr answers settle it.

use super::{Api, Fail, Res};
use crate::case::{num, Op};
use crate::spec::{clause, hit};

/// an NTSTATUS the functions return
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    BufferOverflow,
    InvalidParameter,
}

impl Status {
    pub fn code(self) -> u32 {
        match self {
            Status::Success => 0,
            Status::BufferOverflow => 0x8000_0005,
            Status::InvalidParameter => 0xc000_000d,
        }
    }
}

/// a base the functions take, after 0 is resolved
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Radix {
    Bin,
    Oct,
    Dec,
    Hex,
}

impl Radix {
    /// 2, 8, 10, 16; None for any other (0 is the caller's to resolve)
    pub fn of(base: u32) -> Option<Radix> {
        match base {
            2 => Some(Radix::Bin),
            8 => Some(Radix::Oct),
            10 => Some(Radix::Dec),
            16 => Some(Radix::Hex),
            _ => None,
        }
    }
    pub fn value(self) -> u32 {
        match self {
            Radix::Bin => 2,
            Radix::Oct => 8,
            Radix::Dec => 10,
            Radix::Hex => 16,
        }
    }
    /// the base a lowercase prefix letter after a 0 names
    fn prefix(c: u32) -> Option<Radix> {
        match c {
            0x78 => Some(Radix::Hex), // x
            0x6f => Some(Radix::Oct), // o
            0x62 => Some(Radix::Bin), // b
            _ => None,
        }
    }
}

/// a character's digit value (0-9, A-F, a-f)
fn digit(c: u32) -> Option<u32> {
    match c {
        0x30..=0x39 => Some(c - 0x30),
        0x41..=0x46 => Some(c - 0x37),
        0x61..=0x66 => Some(c - 0x57),
        _ => None,
    }
}

/// the characters of a number after its white space: the sign, the base, the digits
/// (NT.SIGN, NT.DIGITS, the prefixes); `chars` ends at the string's end or at a 0
fn number(chars: &[u32], base: Option<Radix>) -> u32 {
    let mut i = 0;
    let at = |i: usize| chars.get(i).copied().unwrap_or(0);
    let sign = at(i);
    if sign == 0x2b || sign == 0x2d {
        i += 1;
    }
    let radix = match base {
        Some(r) => r,
        None => {
            if at(i) == 0x30 {
                match Radix::prefix(at(i + 1)) {
                    Some(r) => {
                        i += 2;
                        r
                    }
                    None => {
                        // (the 0 adds nothing: the digits start after it)
                        i += 1;
                        Radix::Dec
                    }
                }
            } else {
                Radix::Dec
            }
        }
    };
    hit(clause::NT_DIGITS);
    let mut v: u32 = 0;
    while let Some(d) = digit(at(i)) {
        if d >= radix.value() {
            break;
        }
        v = v.wrapping_mul(radix.value()).wrapping_add(d);
        i += 1;
    }
    if sign == 0x2d {
        hit(clause::NT_SIGN);
        v = v.wrapping_neg();
    }
    v
}

/// RtlCharToInteger: the status and the value (None: left as it was)
pub fn char_to_integer(s: &[u8], base: u32) -> (Status, Option<u32>) {
    hit(clause::NT_A2I_SYNTAX);
    // (the bytes and the executor's NUL after them: a blank byte, a NUL too, is passed while the
    // byte after it is not NUL, so bytes after a leading NUL can count)
    let s: Vec<u8> = s.iter().copied().chain([0]).collect();
    let at = |i: usize| s.get(i).copied().unwrap_or(0);
    let mut i = 0;
    while (at(i) as i8) <= 0x20 {
        hit(clause::NT_A2I_BLANK);
        if at(i + 1) == 0 {
            break;
        }
        i += 1;
    }
    let radix = match base {
        0 => None,
        b => match Radix::of(b) {
            Some(r) => Some(r),
            None => {
                hit(clause::NT_A2I_BASE);
                return (Status::InvalidParameter, None);
            }
        },
    };
    // (a blank byte left at the end is no number)
    let chars: Vec<u32> = s[i.min(s.len())..].iter().map(|&b| u32::from(b)).collect();
    let chars: Vec<u32> = if chars.first().is_some_and(|&c| (c as u8 as i8) <= 0x20) { Vec::new() } else { chars.into_iter().take_while(|&c| c != 0).collect() };
    (Status::Success, Some(number(&chars, radix)))
}

/// RtlUnicodeStringToInteger of a UNICODE_STRING of `length` bytes over `units`: the status and
/// the value written (it is always written)
pub fn unicode_to_integer(units: &[u16], length: usize, base: u32) -> (Status, u32) {
    if length == 0 || length % 2 == 1 {
        hit(clause::NT_U2I_EMPTY);
        return (Status::InvalidParameter, 0);
    }
    let s: Vec<u32> = units.iter().take(length / 2).map(|&u| u32::from(u)).collect();
    hit(clause::NT_U2I_BLANK);
    // (a 0 unit is white space too while it leads; after the white space it ends the number)
    let start = s.iter().position(|&u| u > 0x20).unwrap_or(s.len());
    let s: Vec<u32> = s[start..].iter().copied().take_while(|&u| u != 0).collect();
    let radix = match base {
        0 => {
            hit(clause::NT_U2I_PREFIX);
            None
        }
        b => match Radix::of(b) {
            Some(r) => Some(r),
            None => {
                hit(clause::NT_U2I_EMPTY);
                return (Status::InvalidParameter, 0);
            }
        },
    };
    (Status::Success, number(&s, radix))
}

/// RtlIntegerToChar's digits of `value`
fn digits(value: u32, radix: Radix) -> Vec<u8> {
    hit(clause::NT_I2A_DIGITS);
    let mut v = value;
    let mut d = Vec::new();
    loop {
        d.push(b"0123456789ABCDEF"[(v % radix.value()) as usize]);
        v /= radix.value();
        if v == 0 {
            break;
        }
    }
    d.reverse();
    d
}

fn radix_or_decimal(base: u32) -> Option<Radix> {
    hit(clause::NT_I2A_BASE);
    if base == 0 { Some(Radix::Dec) } else { Radix::of(base) }
}

/// RtlIntegerToChar into `buf`: the status
pub fn integer_to_char(value: u32, base: u32, length: i32, buf: &mut [u8]) -> Status {
    let Some(radix) = radix_or_decimal(base) else {
        return Status::InvalidParameter;
    };
    let d = digits(value, radix);
    let n = d.len() as i64;
    if length >= 0 {
        hit(clause::NT_I2A_LENGTH);
        if n > i64::from(length) {
            return Status::BufferOverflow;
        }
        buf[..d.len()].copy_from_slice(&d);
        if n < i64::from(length) {
            buf[d.len()] = 0;
        }
    } else {
        hit(clause::NT_I2A_PAD);
        let l = -i64::from(length);
        if n > l {
            return Status::BufferOverflow;
        }
        let pad = (l - n) as usize;
        buf[..pad].fill(b'0');
        buf[pad..pad + d.len()].copy_from_slice(&d);
    }
    Status::Success
}

/// a UNICODE_STRING the model writes into: Length, MaximumLength, the buffer (None: NULL)
pub struct UString<'a> {
    pub length: u16,
    pub maximum: u16,
    pub buffer: Option<&'a mut [u8]>,
}

/// RtlAnsiStringToUnicodeString(dst, ASCII bytes, FALSE)
pub fn ansi_to_unicode(dst: &mut UString, src: &[u8]) -> Status {
    hit(clause::NT_A2U_ASCII);
    hit(clause::NT_A2U_SIZE);
    let size = 2 * src.len() + 2;
    let max = usize::from(dst.maximum);
    let Some(buf) = dst.buffer.as_deref_mut() else {
        return Status::BufferOverflow;
    };
    if size > max {
        return Status::BufferOverflow;
    }
    for (k, &b) in src.iter().enumerate() {
        buf[2 * k] = b;
        buf[2 * k + 1] = 0;
    }
    buf[2 * src.len()] = 0;
    buf[2 * src.len() + 1] = 0;
    dst.length = (2 * src.len()) as u16;
    Status::Success
}

/// RtlIntegerToUnicodeString
pub fn integer_to_unicode(value: u32, base: u32, dst: &mut UString) -> Status {
    let mut tmp = [0u8; 33];
    let st = integer_to_char(value, base, 33, &mut tmp);
    if st != Status::Success {
        return st;
    }
    hit(clause::NT_I2U_ANSI);
    let n = tmp.iter().position(|&b| b == 0).unwrap_or(33);
    ansi_to_unicode(dst, &tmp[..n])
}

pub struct Ntstr;

fn n(op: &Op, i: usize) -> Res<i64> {
    op.tok.get(i).and_then(|t| num(t)).ok_or(Fail::Bad)
}
/// token i as bytes, as the executor's hexbytes reads it: "-" none, else pairs of lowercase hex
/// digits, at most MAX bytes (the op's buffer); anything else a bad line (the review of 8480456,
/// 5.4: a byte past ASCII was sliced through and the model stopped; capitals and a length past
/// the buffer were taken where the executor refuses them)
fn hex<const MAX: usize>(op: &Op, i: usize) -> Res<Vec<u8>> {
    let t = op.tok.get(i).ok_or(Fail::Bad)?;
    if t == "-" {
        return Ok(Vec::new());
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let b = t.as_bytes();
    if b.len() % 2 == 1 || b.len() / 2 > MAX {
        return Err(Fail::Bad);
    }
    b.chunks_exact(2).map(|p| digit(p[0]).zip(digit(p[1])).map(|(h, l)| h << 4 | l).ok_or(Fail::Bad)).collect()
}

/// the executor's buffers (runner/ntstr.c): a2i's and a2u's 80 bytes less the NUL, u2i's 158
const A2I_BYTES: usize = 79;
const U2I_BYTES: usize = 158;
const A2U_BYTES: usize = 79;
fn hexs(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
/// the executor prints numbers as C's %08lx of a ULONG: the low 32 bits
fn ulong(v: i64) -> u32 {
    v as u32
}

impl Api for Ntstr {
    fn op(&mut self, op: &Op, out: &mut Vec<String>) -> Res<bool> {
        match op.tok[0].as_str() {
            "a2i" => {
                let (base, b) = (ulong(n(op, 1)?), hex::<A2I_BYTES>(op, 2)?);
                let (st, v) = char_to_integer(&b, base);
                out.push(format!("a2i {:08x} {:08x}", st.code(), v.unwrap_or(0xdead_beef)));
            }
            "u2i" => {
                let (base, len, b) = (ulong(n(op, 1)?), n(op, 2)?, hex::<U2I_BYTES>(op, 3)?);
                if b.len() % 2 == 1 || len < 0 || len > b.len() as i64 + 1 {
                    return Err(Fail::Bad);
                }
                let units: Vec<u16> = b.chunks(2).map(|c| u16::from(c[0]) << 8 | u16::from(c[1])).collect();
                // (an odd Length rounds down to whole units: the half unit is never read)
                let (st, v) = unicode_to_integer(&units, len as usize, base);
                out.push(format!("u2i {:08x} {v:08x}", st.code()));
            }
            "i2a" => {
                let (v, base, len) = (ulong(n(op, 1)?), ulong(n(op, 2)?), n(op, 3)?);
                if !(-40..=40).contains(&len) {
                    return Err(Fail::Bad);
                }
                let mut b = [0xccu8; 40];
                let st = integer_to_char(v, base, len as i32, &mut b);
                out.push(format!("i2a {:08x} {}", st.code(), hexs(&b)));
            }
            "i2u" | "a2u" => {
                let i2u = op.tok[0] == "i2u";
                let (first, max, null_at) = if i2u { (ulong(n(op, 1)?), n(op, 3)?, 4) } else { (0, n(op, 2)?, 4) };
                let null = match op.tok.get(null_at).map(String::as_str) {
                    None => false,
                    Some("null") => true,
                    Some(_) => return Err(Fail::Bad),
                };
                if !(0..=80).contains(&max) {
                    return Err(Fail::Bad);
                }
                let mut b = [0xccu8; 80];
                let mut s = UString { length: 0x5555, maximum: max as u16, buffer: if null { None } else { Some(&mut b[..]) } };
                let st = if i2u {
                    integer_to_unicode(first, ulong(n(op, 2)?), &mut s)
                } else {
                    let (len, a) = (n(op, 1)?, hex::<A2U_BYTES>(op, 3)?);
                    if len < 0 || len as usize > a.len() || a.len() > 79 || a.iter().any(|&x| x >= 0x80) {
                        return Err(Fail::Bad);
                    }
                    ansi_to_unicode(&mut s, &a[..len as usize])
                };
                let (length, maximum) = (s.length, s.maximum);
                out.push(format!("{} {:08x} {length:04x} {maximum:04x} {} {}", op.tok[0], st.code(), if null { "null" } else { "buf" }, hexs(&b)));
            }
            _ => return Ok(false),
        }
        Ok(true)
    }
}
