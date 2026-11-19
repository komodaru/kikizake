//! Laws of src/calls.rs (property-based): the recording is cut into the executor's cases however
//! its output was flushed and in whatever order shirusu's export lists the events; a calls file
//! reads back as written; a faithful executor passes check_ops and any one wrong argument is
//! found; the JSON reader reads what a writer writes.
use kikizake::calls::{self, Api, Call, Handle, LastError, Ret, Val};
use kikizake::json::{self, Json};
use proptest::prelude::*;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn esc(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o += "\\\"",
            '\\' => o += "\\\\",
            c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32),
            c => o.push(c),
        }
    }
    o + "\""
}

fn write(v: &Json) -> String {
    match v {
        Json::Null => "null".into(),
        Json::Bool(b) => b.to_string(),
        Json::Num(n) => n.clone(),
        Json::Str(s) => esc(s),
        Json::Arr(a) => format!("[{}]", a.iter().map(write).collect::<Vec<_>>().join(",")),
        Json::Obj(p) => format!("{{{}}}", p.iter().map(|(k, v)| format!("{}:{}", esc(k), write(v))).collect::<Vec<_>>().join(",")),
    }
}

fn json_value() -> impl Strategy<Value = Json> {
    let leaf = prop_oneof![
        Just(Json::Null),
        any::<bool>().prop_map(Json::Bool),
        any::<i64>().prop_map(|n| Json::Num(n.to_string())),
        "\\PC{0,8}".prop_map(Json::Str),
    ];
    leaf.prop_recursive(4, 32, 4, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Json::Arr),
            prop::collection::vec(("[a-z]{1,4}", inner), 0..4).prop_map(Json::Obj),
        ]
    })
}

/// one shirusu field
fn field(name: &str, v: &Val) -> String {
    let (ty, kind, value) = match v {
        Val::Handle(Handle(h)) => ("U64", ",\"kind\":\"handle\"", format!("\"{h:#x}\"")),
        Val::Null => ("U64", "", "\"0x0\"".into()),
        Val::Ptr => ("U64", ",\"kind\":\"pointer\"", "\"0x5ffe10\"".into()),
        Val::Dec(x) => ("I32", "", x.to_string()),
        Val::Hex(x) => ("U32", "", x.to_string()),
        Val::Bool(b) => ("Bool", "", b.to_string()),
        Val::Missing(_) | Val::Str(_) | Val::Bytes(_) | Val::Text(_) => unreachable!("not generated"),
    };
    format!("{}:{{\"ty\":\"{ty}\",\"capture\":\"captured\",\"value\":{value}{kind}}}", esc(name))
}

/// shirusu's JSONL for an executor that made `cases` (name, calls, output chunks: the calls of
/// chunk i come before its WriteFile), the events listed in `perm` order with their global order
fn jsonl(cases: &[(String, Vec<Vec<Call>>, Vec<String>)], perm: &[usize]) -> (String, String) {
    jsonl_cut(cases, perm, None)
}

/// as `jsonl`, shirusu keeping only the first `keep.1` bytes of write `keep.0`
fn jsonl_cut(cases: &[(String, Vec<Vec<Call>>, Vec<String>)], perm: &[usize], keep: Option<(usize, usize)>) -> (String, String) {
    let mut writes = 0;
    let mut ev = Vec::new();
    let mut id = 0;
    let mut stdout = String::new();
    let mut emit = |ev: &mut Vec<String>, api: &str, enter: String, leave: String| {
        id += 1;
        ev.push(format!("{{\"phase\":\"enter\",\"api\":\"gdi32!{api}\",\"call_id\":{id},\"fields\":{{{enter}}}}}"));
        ev.push(format!("{{\"phase\":\"leave\",\"api\":\"gdi32!{api}\",\"call_id\":{id},\"fields\":{{{leave}}}}}"));
    };
    for (k, (name, chunks, outs)) in cases.iter().enumerate() {
        for (j, (calls, out)) in chunks.iter().zip(outs).enumerate() {
            for c in calls {
                let a: Vec<String> = c.args.iter().enumerate().map(|(i, v)| field(&format!("a{i}"), v)).collect();
                let Ret::Value(r) = &c.ret else { unreachable!("not generated") };
                let mut l = vec![field("return", r)];
                if let Some(LastError(e)) = c.err {
                    l.push(format!("\"last_error\":{{\"ty\":\"U32\",\"capture\":\"captured\",\"value\":{e}}}"));
                }
                emit(&mut ev, &c.api.0, a.join(","), l.join(","));
            }
            let mut text = String::new();
            if k == 0 && j == 0 {
                text += "kikizake s\n";
            }
            if j == 0 {
                text += &format!("case {name}\n");
            }
            text += out;
            stdout += &text;
            let b = text.as_bytes();
            let (kept, capture) = match keep {
                Some((w, n)) if w == writes && n < b.len() => (&b[..n], "truncated"),
                Some(_) | None => (b, "captured"),
            };
            writes += 1;
            emit(&mut ev, "WriteFile", format!("\"hFile\":{{\"ty\":\"U64\",\"capture\":\"captured\",\"value\":\"0x8\"}},\"data\":{{\"ty\":\"Bytes\",\"capture\":\"{capture}\",\"value\":\"{}\"}},\"nNumberOfBytesToWrite\":{{\"ty\":\"U32\",\"capture\":\"captured\",\"value\":{}}}", hex(kept), b.len()), String::new());
        }
    }
    stdout += "done\n";
    emit(&mut ev, "WriteFile", format!("\"data\":{{\"ty\":\"Bytes\",\"capture\":\"captured\",\"value\":\"{}\"}}", hex(b"done\n")), String::new());
    let n = ev.len();
    let mut lines = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for &p in perm.iter().filter(|&&p| p < n && seen.insert(p)) {
        let e = &ev[p];
        lines.push(format!("{},\"global_order\":{p}}}", &e[..e.len() - 1]));
    }
    for (p, e) in ev.iter().enumerate() {
        if !perm.contains(&p) {
            lines.push(format!("{},\"global_order\":{p}}}", &e[..e.len() - 1]));
        }
    }
    (lines.join("\n"), stdout)
}

fn value() -> impl Strategy<Value = Val> {
    prop_oneof![
        (1u64..6).prop_map(|h| Val::Handle(Handle(h))),
        (1u64..u64::MAX).prop_map(|h| Val::Handle(Handle(h))),
        Just(Val::Null),
        any::<i32>().prop_map(|n| Val::Dec(n.into())),
        (0x10000u32..).prop_map(Val::Hex),
        any::<bool>().prop_map(Val::Bool),
        Just(Val::Ptr),
    ]
}

fn call() -> impl Strategy<Value = Call> {
    (
        prop::sample::select(vec!["BitBlt", "PatBlt", "SelectObject", "CreateDIBSection"]),
        prop::collection::vec(value(), 0..5),
        value(),
        prop::option::of(any::<u32>()),
    )
        .prop_map(|(api, args, ret, err)| Call { api: Api(api.into()), args, ret: Ret::Value(ret), err: err.map(LastError) })
}

fn cases() -> impl Strategy<Value = Vec<(String, Vec<Vec<Call>>, Vec<String>)>> {
    prop::collection::vec(
        prop::collection::vec((prop::collection::vec(call(), 0..4), "([a-f0-9 ]{0,12}\n){0,2}"), 1..4),
        1..5,
    )
    .prop_map(|cs| {
        cs.into_iter()
            .enumerate()
            .map(|(i, ch)| {
                let (calls, outs): (Vec<_>, Vec<_>) = ch.into_iter().unzip();
                (format!("c{i}"), calls, outs)
            })
            .collect()
    })
}

/// the same calls, handles renumbered by first appearance in the case (what a calls file keeps)
fn renumber(calls: &[Call]) -> Vec<Call> {
    let mut ids = std::collections::HashMap::new();
    let mut f = |v: &Val| match v {
        Val::Handle(h) => {
            let n = ids.len() as u64 + 1;
            Val::Handle(Handle(*ids.entry(*h).or_insert(n)))
        }
        t => t.clone(),
    };
    calls
        .iter()
        .map(|c| Call {
            api: c.api.clone(),
            args: c.args.iter().map(&mut f).collect(),
            ret: match &c.ret {
                Ret::Value(v) => Ret::Value(f(v)),
                r => r.clone(),
            },
            err: c.err,
        })
        .collect()
}

proptest! {
    #[test]
    fn json_reads_what_is_written(v in json_value()) {
        prop_assert_eq!(json::parse(&write(&v)), Ok(v));
    }

    #[test]
    fn json_never_panics(s in "\\PC{0,40}") {
        let _ = json::parse(&s);
    }

    #[test]
    fn recording_is_cut_into_the_cases(cs in cases(), perm in prop::collection::vec(0usize..200, 0..40)) {
        let (text, stdout) = jsonl(&cs, &perm);
        let got = calls::from_jsonl(&text, Some(&stdout)).unwrap();
        prop_assert_eq!(got.header.stdout, calls::Stdout::Same);
        prop_assert_eq!(got.header.other, 0);
        let got = got.cases;
        prop_assert_eq!(got.len(), cs.len());
        for (g, (name, chunks, _)) in got.iter().zip(&cs) {
            prop_assert_eq!(&g.name, name);
            let want: Vec<Call> = chunks.iter().flatten().cloned().collect();
            prop_assert_eq!(&g.calls, &want);
        }
        // and a calls file reads back as written, handles renumbered per case
        let header = calls::Header { name: "s-32".into(), cases: calls::CasesHash(7), src: "x".into(), shirusu: "y".into(), stdout: calls::Stdout::Same, events: 1, other: 0 };
        let r = calls::Recording { header: header.clone(), cases: got.clone() };
        let back = calls::parse(&calls::format(&r)).unwrap();
        prop_assert_eq!(back.cases.len(), got.len());
        for (b, g) in back.cases.iter().zip(&got) {
            prop_assert_eq!(&b.calls, &renumber(&g.calls));
        }
        prop_assert_eq!(&back.header, &header);
        prop_assert_eq!(calls::format(&back), calls::format(&r));
    }

    /// shirusu keeps the start of a long write: the rest comes from the plain run, so the cases
    /// are cut as before and the next case's line stays a line; only the cut case is unchecked
    #[test]
    fn a_truncated_write_cuts_the_same_cases(cs in cases(), w in 0usize..16, n in 0usize..24) {
        let (text, stdout) = jsonl_cut(&cs, &[], Some((w, n)));
        let (whole, _) = jsonl(&cs, &[]);
        let got = calls::from_jsonl(&text, Some(&stdout)).unwrap();
        let want = calls::from_jsonl(&whole, Some(&stdout)).unwrap();
        prop_assert_eq!(got.cases.len(), want.cases.len());
        let mut cut = 0;
        for (g, x) in got.cases.iter().zip(&want.cases) {
            prop_assert_eq!(&g.name, &x.name);
            prop_assert_eq!(&g.calls, &x.calls);
            match g.out {
                calls::Stdout::Truncated => cut += 1,
                calls::Stdout::Same => {}
                calls::Stdout::Differ | calls::Stdout::Unknown => prop_assert!(false, "{} is {:?}", g.name, g.out),
            }
        }
        let truncated = text.contains("\"capture\":\"truncated\"");
        prop_assert_eq!(cut, usize::from(truncated));
        prop_assert_eq!(got.header.stdout, if truncated { calls::Stdout::Truncated } else { calls::Stdout::Same });
    }

    #[test]
    fn output_changed_under_recording_is_seen(cs in cases()) {
        let (text, stdout) = jsonl(&cs, &[]);
        let r = calls::from_jsonl(&text, Some(&(stdout + "x"))).unwrap();
        prop_assert_eq!(r.header.stdout, calls::Stdout::Differ);
    }
}

/// a faithful executor of the spec/calls.txt lines: each op's call with the case's arguments,
/// slot s -> handle 100 + s, other calls in between
fn faithful(ops: &[Vec<String>], rules: &[calls::Rule], noise: &[usize]) -> Vec<Call> {
    let mut v = Vec::new();
    for (i, t) in ops.iter().enumerate() {
        for _ in 0..noise.get(i).copied().unwrap_or(0) {
            v.push(Call { api: Api("SelectObject".into()), args: vec![Val::Handle(Handle(7))], ret: Ret::Value(Val::Handle(Handle(8))), err: None });
        }
        for r in rules.iter().filter(|r| r.op == t[0]) {
            let calls::Does::Call(api, args) = &r.does else { continue };
            let tok = |n: &calls::Tok| &t[n.0];
            let args = args
                .iter()
                .map(|a| match a {
                    calls::Arg::Slot(n) => Val::Handle(Handle(100 + tok(n).parse::<u64>().unwrap())),
                    calls::Arg::Num(n) => Val::Dec(kikizake::case::num(tok(n)).unwrap()),
                    calls::Arg::Color(n) => Val::Dec(calls::colorref(kikizake::case::color(tok(n)).unwrap()).into()),
                    calls::Arg::Lit(x) => Val::Dec(*x),
                    calls::Arg::Any => Val::Handle(Handle(9)),
                })
                .collect();
            v.push(Call { api: api.clone(), args, ret: Ret::Value(Val::Dec(1)), err: None });
        }
    }
    v
}

fn op() -> impl Strategy<Value = Vec<String>> {
    let n = || (-3i64..70).prop_map(|v| v.to_string());
    let s = || (0u32..8).prop_map(|v| v.to_string());
    let c = || any::<u32>().prop_map(|v| format!("#{:06x}", v & 0xffffff));
    prop_oneof![
        (s(), n(), n(), n(), n(), any::<u32>()).prop_map(|(a, b, c, d, e, f)| vec!["patblt".into(), a, b, c, d, e, format!("{f:#x}")]),
        (s(), n(), n(), n(), n(), s(), n(), n(), any::<u32>()).prop_map(|(a, b, c, d, e, f, g, h, r)| vec!["bitblt".into(), a, b, c, d, e, f, g, h, format!("{r:#x}")]),
        (s(), n(), n(), c()).prop_map(|(a, b, d, e)| vec!["setpixel".into(), a, b, d, e]),
        (s(), c()).prop_map(|(a, b)| vec!["text".into(), a, b]),
        (s(), n(), n()).prop_map(|(a, b, d)| vec!["org".into(), a, b, d]),
    ]
}

fn header() -> calls::Header {
    calls::Header { name: "s-32".into(), cases: calls::CasesHash(7), src: "x".into(), shirusu: "y".into(), stdout: calls::Stdout::Same, events: 1, other: 0 }
}

fn rules() -> Vec<calls::Rule> {
    calls::rules(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/calls.txt")).unwrap()).unwrap()
}

fn setup(ops: &[Vec<String>], noise: &[usize]) -> (Vec<kikizake::case::Case>, calls::Recording) {
    let rs = rules();
    let case = kikizake::case::Case { name: "x".into(), ops: ops.iter().enumerate().map(|(i, t)| kikizake::case::Op { line: i + 2, tok: t.clone() }).collect() };
    let rec = calls::Recording { header: header(), cases: vec![calls::Case { name: "x".into(), bad: vec![], out: calls::Stdout::Unknown, calls: faithful(ops, &rs, noise) }] };
    (vec![case], rec)
}

proptest! {
    #[test]
    fn a_faithful_executor_passes(ops in prop::collection::vec(op(), 1..12), noise in prop::collection::vec(0usize..3, 0..12)) {
        let (cases, rec) = setup(&ops, &noise);
        prop_assert_eq!(calls::check_ops(&cases, &rec, &rules()), Vec::<String>::new());
    }

    #[test]
    fn one_wrong_argument_is_found(ops in prop::collection::vec(op(), 1..12), pick in any::<prop::sample::Index>(), arg in any::<prop::sample::Index>()) {
        let (cases, mut rec) = setup(&ops, &[]);
        let calls_ = &mut rec.cases[0].calls;
        let k = pick.index(calls_.len());
        // (a slot's first handle is whatever it is: a wrong one shows only where it is used again)
        let nums: Vec<usize> = (0..calls_[k].args.len()).filter(|&i| matches!(calls_[k].args[i], Val::Dec(_))).collect();
        let a = nums[arg.index(nums.len())];
        let Val::Dec(x) = calls_[k].args[a] else { unreachable!() };
        calls_[k].args[a] = Val::Dec(x ^ 0x40);
        prop_assert!(!calls::check_ops(&cases, &rec, &rules()).is_empty());
    }

    #[test]
    fn a_missing_call_is_found(ops in prop::collection::vec(op(), 1..12), pick in any::<prop::sample::Index>()) {
        let (cases, mut rec) = setup(&ops, &[]);
        let k = pick.index(rec.cases[0].calls.len());
        rec.cases[0].calls.remove(k);
        prop_assert!(!calls::check_ops(&cases, &rec, &rules()).is_empty());
    }

    #[test]
    fn bad_lines_are_not_checked(ops in prop::collection::vec(op(), 1..12), pick in any::<prop::sample::Index>()) {
        let (cases, mut rec) = setup(&ops, &[]);
        let k = pick.index(ops.len());
        // (the executor refused line k: it made no call for it)
        let without: Vec<Vec<String>> = ops.iter().enumerate().filter(|(i, _)| *i != k).map(|(_, t)| t.clone()).collect();
        rec.cases[0].calls = faithful(&without, &rules(), &[]);
        rec.cases[0].bad = vec![calls::Line(k + 2)];
        prop_assert_eq!(calls::check_ops(&cases, &rec, &rules()), Vec::<String>::new());
    }
}

proptest! {
    /// x86 and x86-64 calls must be the same but for what spec/arch.txt allows: exactly its pairs,
    /// at exactly its place, of exactly its API
    #[test]
    fn x86_and_x86_64_differences_are_found(x in 0i64..200, y in 0i64..200, at_ret in any::<bool>(), other_api in any::<bool>()) {
        let arch = calls::arch_rules("GetObjectA 2 84=104 24=32\nGetObjectA ret 84=104\n").unwrap();
        let api = if other_api { "GetObjectW" } else { "GetObjectA" };
        let c = |size: i64, ret: i64| calls::Case { name: "a".into(), bad: vec![], out: calls::Stdout::Unknown, calls: vec![Call { api: Api(api.into()), args: vec![Val::Handle(Handle(1)), Val::Dec(size), Val::Ptr], ret: Ret::Value(Val::Dec(ret)), err: None }] };
        let (a, b) = if at_ret { (c(84, x), c(104, y)) } else { (c(x, 24), c(y, 24)) };
        let a = calls::Recording { header: header(), cases: vec![a] };
        let b = calls::Recording { header: header(), cases: vec![b] };
        prop_assert!(calls::differ(&a, &a, &arch).is_empty());
        let rule = !other_api;
        let same = if at_ret { rule && (x == y || (x, y) == (84, 104)) } else { x == y || rule && [(84, 104), (24, 32)].contains(&(x, y)) };
        prop_assert_eq!(calls::differ(&a, &b, &arch).is_empty(), same);
    }
}

#[test]
fn spec_arch_txt_reads() {
    let t = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/arch.txt")).unwrap();
    assert!(!calls::arch_rules(&t).unwrap().is_empty());
    for bad in ["GetObjectA", "GetObjectA 0 1=2", "GetObjectA ret", "GetObjectA ret 1-2", "GetObjectA x 1=2"] {
        assert!(calls::arch_rules(bad).is_err(), "{bad}");
    }
}

proptest! {
    /// an application that makes exactly the recorded calls needs nothing new; one more API or
    /// one more mode value is reported, and non-graphics modules are not looked at
    #[test]
    fn usage_reports_what_no_case_does(modes in prop::collection::vec(0i32..20, 1..6), extra in 20i32..40) {
        let call = |m: i32| Call { api: Api("SetStretchBltMode".into()), args: vec![Val::Handle(Handle(5)), Val::Dec(m.into())], ret: Ret::Value(Val::Dec(1)), err: None };
        let rec = calls::Recording { header: header(), cases: vec![calls::Case { name: "a".into(), bad: vec![], out: calls::Stdout::Unknown, calls: modes.iter().map(|m| call(*m)).collect() }] };
        let ev = |api: &str, m: i32| format!("{{\"phase\":\"enter\",\"api\":\"{api}\",\"call_id\":1,\"fields\":{{\"hdc\":{{\"ty\":\"U64\",\"capture\":\"captured\",\"value\":\"0x99\",\"kind\":\"handle\"}},\"mode\":{{\"ty\":\"I32\",\"capture\":\"captured\",\"value\":{m}}}}}}}");
        let same: Vec<String> = modes.iter().map(|m| ev("gdi32!SetStretchBltMode", *m)).chain([ev("kernel32!Sleep", 3)]).collect();
        prop_assert_eq!(calls::usage(&same.join("\n"), std::slice::from_ref(&rec)).unwrap(), Vec::<String>::new());
        let more = [same.clone(), vec![ev("gdi32!SetStretchBltMode", extra), ev("gdi32!SetROP2", 1)]].concat();
        let u = calls::usage(&more.join("\n"), &[rec]).unwrap();
        prop_assert_eq!(u.len(), 2, "{:?}", u);
        prop_assert!(u.iter().any(|l| l.starts_with("gdi32!SetROP2") && l.ends_with("no case calls it")));
        let want = format!("argument 2: {extra} ");
        prop_assert!(u.iter().any(|l| l.starts_with("gdi32!SetStretchBltMode") && l.contains(&want)));
    }
}

#[test]
fn created_dc_belongs_to_its_slot_until_recreated() {
    let input = kikizake::case::parse_cases("case x\ndib 0 1 1 32\ndib 1 1 1 32\ntext 0 #000000\ntext 1 #000000\ndib 0 1 1 32\ntext 0 #000000\nend");
    let create = |h| Call { api: Api("CreateCompatibleDC".into()), args: vec![], ret: Ret::Value(Val::Handle(Handle(h))), err: None };
    let text = |h| Call { api: Api("SetTextColor".into()), args: vec![Val::Handle(Handle(h)), Val::Dec(0)], ret: Ret::Value(Val::Dec(0)), err: None };
    let record = |a, b, c| calls::Recording { header: header(), cases: vec![calls::Case {
        name: "x".into(), bad: vec![], out: calls::Stdout::Same,
        calls: vec![create(1), create(2), text(a), text(b), create(3), text(c)],
    }] };
    for (a, b, c, valid) in [(1, 2, 3, true), (2, 1, 3, false), (1, 1, 3, false), (1, 2, 1, false)] {
        assert_eq!(calls::check_ops(&input, &record(a, b, c), &rules()).is_empty(), valid);
    }
}
