//! Any bytes as shirusu's JSONL, as a calls file, as spec/calls.txt and spec/arch.txt: reading never panics, a
//! recording read from JSONL writes a calls file that reads back to the same text, and checking
//! it against any cases never panics.
#![no_main]
use libfuzzer_sys::fuzz_target;
use kikizake::{calls, case};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = kikizake::json::parse(&text);
    if let Ok(mut r) = calls::from_jsonl(&text, Some(&text)) {
        r.header.name = "f-32".into();
        r.header.src = "s".into();
        r.header.shirusu = "r".into();
        let f = calls::format(&r);
        let back = calls::parse(&f).expect("a written calls file reads back");
        assert_eq!(calls::format(&back), f);
        let rules = calls::rules(&text).unwrap_or_default();
        let _ = calls::check_ops(&case::parse_cases(&text), &back, &rules);
        let arch = calls::arch_rules(&text).unwrap_or_default();
        let _ = calls::differ(&back, &r, &arch);
    }
    let _ = calls::parse(&text);
});
