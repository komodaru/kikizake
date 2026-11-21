//! The ntcase suite: ntdll's case tables, every UTF-16 unit (256 blocks of 256) through
//! RtlUpcaseUnicodeChar, RtlDowncaseUnicodeChar and ntdll's own towupper / towlower.
use super::*;

pub fn ntcase() -> String {
    let mut s = String::from("# kikizake suite ntcase: ntdll's case tables (SPEC.md). Made by `kikizake gen ntcase`.\n");
    for f in ["up", "down", "towupper", "towlower"] {
        for block in 0..256 {
            writeln!(s, "case {f}-{block:02x}\ntab {f} {block}\nend");
        }
    }
    s
}
