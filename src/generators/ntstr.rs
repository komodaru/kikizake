//! The ntstr suite: ntdll's integer <-> string conversions (RtlCharToInteger,
//! RtlUnicodeStringToInteger, RtlIntegerToChar, RtlIntegerToUnicodeString,
//! RtlAnsiStringToUnicodeString). Every base (the valid ones and a few others) over strings at the
//! edges (white space, signs, prefixes, overflow, digits past the base, NULs, bytes and units past
//! ASCII), every value at the edges against lengths around its digit count, then random ones.
use super::*;

/// the strings, as bytes (the unicode ones widen each byte, plus WIDE)
const STRINGS: &[&[u8]] = &[
    b"", b"0", b"7", b"123", b"-123", b"+42", b"  12", b"\t-7", b"\n\r 9", b"0x1F", b"0X1F", b"0x1f", b"0o17",
    b"0O17", b"0b101", b"0B101", b"012", b"08", b"0x", b"0b", b"0o", b"-0x10", b"+0b11", b"4294967295",
    b"4294967296", b"99999999999", b"-2147483648", b"ff", b"FF", b"1a", b"Az", b"12 34", b"- 5", b"+-5", b"--5",
    b"\x80 12", b"\xa0 9", b"\xff7", b"\x0012", b"\x00 12", b" ", b"  ", b"1\x002", b"0x\x001", b"-", b"+",
    b"0x7fffffff", b"0xffffffff", b"0x100000000", b"0b11111111111111111111111111111111", b"0o37777777777",
    b"777", b"1010", b"zz", b".5", b"5.", b"0x-1", b"0 x1", b"\x01\x1f3", b"\x21\x31",
];
/// UTF-16 strings past ASCII: ideographic space, fullwidth digits, Arabic-Indic digits, a digit
/// under a high byte, NUL inside
const WIDE: &[&[u16]] = &[
    &[0x3000, 0x31], &[0xff11, 0xff12], &[0x0661, 0x0662], &[0x0131], &[0x0141, 0x31], &[0x20, 0x00, 0x31],
    &[0x00, 0x31], &[0x2d, 0x3000, 0x31], &[0xa0, 0x35], &[0x85, 0x36], &[0x0130, 0x0130], &[0x30, 0x0178, 0x31],
];
const BASES: &[u32] = &[0, 2, 8, 10, 16, 1, 3, 36, 0xffff_ffff];
const VALUES: &[u32] = &[0, 1, 7, 9, 10, 15, 16, 255, 256, 1000, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, 123_456_789, 0xdead_beef];

fn hexb(b: &[u8]) -> String {
    if b.is_empty() { "-".into() } else { b.iter().map(|x| format!("{x:02x}")).collect() }
}
fn hexw(w: &[u16]) -> String {
    if w.is_empty() { "-".into() } else { w.iter().map(|x| format!("{x:04x}")).collect() }
}
/// how many digits `v` has in `base` (0 is 10; an invalid one as 10: only the lengths change)
fn ndigits(v: u32, base: u32) -> i64 {
    let b = if [2, 8, 16].contains(&base) { base } else { 10 };
    let (mut v, mut n) = (v, 1);
    while v >= b {
        v /= b;
        n += 1;
    }
    n
}
/// u2i lines of `w` at every interesting Length: whole, 0, odd, one unit short, half
fn u2i_lines(s: &mut String, base: u32, w: &[u16]) {
    let full = 2 * w.len() as i64;
    let mut lens = vec![full, 0, full - 1, full + 1, full - 2, 2 * (w.len() as i64 / 2)];
    lens.retain(|&l| l >= 0);
    lens.dedup();
    for l in lens {
        writeln!(s, "u2i {base} {l} {}", hexw(w)).unwrap();
    }
}

pub fn ntstr() -> String {
    let mut s = String::from("# kikizake suite ntstr: ntdll's integer <-> string conversions (SPEC.md). Made by `kikizake gen ntstr`.\n");
    let mut r = XorShift32::new(0x4e75_0000);
    for &base in BASES {
        writeln!(s, "case a2i-base-{base}").unwrap();
        for t in STRINGS {
            writeln!(s, "a2i {base} {}", hexb(t)).unwrap();
        }
        writeln!(s, "end\ncase u2i-base-{base}").unwrap();
        for t in STRINGS {
            u2i_lines(&mut s, base, &t.iter().map(|&b| u16::from(b)).collect::<Vec<_>>());
        }
        for w in WIDE {
            u2i_lines(&mut s, base, w);
        }
        writeln!(s, "end").unwrap();
    }
    for &base in BASES {
        writeln!(s, "case i2a-base-{base}").unwrap();
        for &v in VALUES {
            let d = ndigits(v, base);
            let mut lens = vec![40, 0, 1, d - 1, d, d + 1, -1, -(d - 1), -d, -(d + 3), -40];
            lens.sort();
            lens.dedup();
            for l in lens.into_iter().filter(|l| (-40..=40).contains(l)) {
                writeln!(s, "i2a {v:#x} {base} {l}").unwrap();
            }
        }
        writeln!(s, "end\ncase i2u-base-{base}").unwrap();
        for &v in VALUES {
            let d = 2 * ndigits(v, base);
            let mut maxes = vec![80, 0, 1, 2, d - 1, d, d + 1, d + 2, d + 3];
            maxes.sort();
            maxes.dedup();
            for m in maxes.into_iter().filter(|m| (0..=80).contains(m)) {
                writeln!(s, "i2u {v:#x} {base} {m}").unwrap();
            }
            writeln!(s, "i2u {v:#x} {base} 80 null").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    writeln!(s, "case a2u").unwrap();
    for t in [&b""[..], b"a", b"abc", b"Hello, world", b"0123456789012345678901234567890123456789"] {
        let n = t.len() as i64;
        let mut maxes = vec![80, 0, 1, 2 * n, 2 * n + 1, 2 * n + 2, 2 * n + 3];
        maxes.sort();
        maxes.dedup();
        for m in maxes.into_iter().filter(|m| (0..=80).contains(m)) {
            writeln!(s, "a2u {n} {m} {}", hexb(t)).unwrap();
        }
        writeln!(s, "a2u {n} 80 {} null", hexb(t)).unwrap();
        if n > 1 {
            writeln!(s, "a2u {} 80 {}", n - 1, hexb(t)).unwrap();
        }
    }
    writeln!(s, "end").unwrap();
    // random: strings of digits, signs, prefixes, white space and other bytes; values and lengths
    const ALPHABET: &[u8] = b"0123456789abcdefABCDEFxob+- \t\x00\x80\xff.zXOB";
    for k in 0..40 {
        writeln!(s, "case random-{k}").unwrap();
        for _ in 0..25 {
            let base = *r.pick(&[0u32, 0, 0, 2, 8, 10, 16, 16]);
            let len = r.range(0, 14) as usize;
            let t: Vec<u8> = (0..len).map(|_| *r.pick(ALPHABET)).collect();
            writeln!(s, "a2i {base} {}", hexb(&t)).unwrap();
            let w: Vec<u16> = t.iter().map(|&b| u16::from(b)).collect();
            let l = r.range(0, 2 * len as i64 + 1);
            writeln!(s, "u2i {base} {l} {}", hexw(&w)).unwrap();
            let v = r.next();
            let d = ndigits(v, base);
            writeln!(s, "i2a {v:#x} {base} {}", r.range(-(d + 2), d + 3)).unwrap();
            writeln!(s, "i2u {v:#x} {base} {}", r.range(0, 2 * d + 4)).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    s
}
