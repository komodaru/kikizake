//! Laws of src/calls.rs (property-based): the recording is cut into the executor's cases however
//! its output was flushed and in whatever order shirusu's export lists the events; a calls file
//! reads back as written; a faithful executor passes check_ops and any one wrong argument is
//! found; the JSON reader reads what a writer writes.
use kikizake::calls::{self, Call, Val};
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
    match v {
        Val::Opaque(h) => format!("{}:{{\"ty\":\"U64\",\"capture\":\"captured\",\"value\":\"{h:#x}\"}}", esc(name)),
        Val::Text(t) => format!("{}:{{\"ty\":\"I32\",\"capture\":\"captured\",\"value\":{t}}}", esc(name)),
    }
}

/// shirusu's JSONL for an executor that made `cases` (name, calls, output chunks: the calls of
/// chunk i come before its WriteFile), the events listed in `perm` order with their global order
fn jsonl(cases: &[(String, Vec<Vec<Call>>, Vec<String>)], perm: &[usize]) -> (String, String) {
    let mut ev = Vec::new();
    let mut id = 0;
    let mut stdout = String::new();
    let mut emit = |ev: &mut Vec<String>, api: &str, enter: String, leave: String| {
        id += 1;
        ev.push(format!("{{\"phase\":\"enter\",\"api\":\"x!{api}\",\"call_id\":{id},\"fields\":{{{enter}}}}}"));
        ev.push(format!("{{\"phase\":\"leave\",\"api\":\"x!{api}\",\"call_id\":{id},\"fields\":{{{leave}}}}}"));
    };
    for (k, (name, chunks, outs)) in cases.iter().enumerate() {
        for (j, (calls, out)) in chunks.iter().zip(outs).enumerate() {
            for c in calls {
                let a: Vec<String> = c.args.iter().enumerate().map(|(i, v)| field(&format!("a{i}"), v)).collect();
                let mut l = vec![field("return", c.ret.as_ref().unwrap())];
                if let Some(e) = &c.err {
                    l.push(format!("\"last_error\":{{\"ty\":\"U32\",\"capture\":\"captured\",\"value\":{e}}}"));
                }
                emit(&mut ev, &c.api, a.join(","), l.join(","));
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
            emit(&mut ev, "WriteFile", format!("\"hFile\":{{\"ty\":\"U64\",\"capture\":\"captured\",\"value\":\"0x8\"}},\"data\":{{\"ty\":\"Bytes\",\"capture\":\"captured\",\"value\":\"{}\"}}", hex(text.as_bytes())), String::new());
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

fn call() -> impl Strategy<Value = Call> {
    (
        prop::sample::select(vec!["BitBlt", "PatBlt", "SelectObject", "CreateDIBSection"]),
        prop::collection::vec(prop_oneof![(1u64..6).prop_map(Val::Opaque), (-5i64..300).prop_map(|n| Val::Text(n.to_string()))], 0..5),
        prop_oneof![(1u64..6).prop_map(Val::Opaque), (0i64..2).prop_map(|n| Val::Text(n.to_string()))],
        prop::option::of(1u32..100),
    )
        .prop_map(|(api, args, ret, err)| Call { api: api.into(), args, ret: Some(ret), err: err.map(|e| e.to_string()) })
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
        Val::Opaque(h) => {
            let n = ids.len() as u64 + 1;
            Val::Opaque(*ids.entry(*h).or_insert(n))
        }
        t => t.clone(),
    };
    calls.iter().map(|c| Call { api: c.api.clone(), args: c.args.iter().map(&mut f).collect(), ret: c.ret.as_ref().map(&mut f), err: c.err.clone() }).collect()
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
        let (got, same, _, other) = calls::from_jsonl(&text, Some(&stdout)).unwrap();
        prop_assert_eq!(same, "same");
        prop_assert_eq!(other, 0);
        prop_assert_eq!(got.len(), cs.len());
        for (g, (name, chunks, _)) in got.iter().zip(&cs) {
            prop_assert_eq!(&g.name, name);
            let want: Vec<Call> = chunks.iter().flatten().cloned().collect();
            prop_assert_eq!(&g.calls, &want);
        }
        // and a calls file reads back as written, handles renumbered per case
        let r = calls::Recording { header: "kikizake calls s-32 cases 0".into(), cases: got.clone() };
        let back = calls::parse(&calls::format(&r)).unwrap();
        prop_assert_eq!(back.cases.len(), got.len());
        for (b, g) in back.cases.iter().zip(&got) {
            prop_assert_eq!(&b.calls, &renumber(&g.calls));
        }
        prop_assert_eq!(calls::format(&back), calls::format(&r));
    }

    #[test]
    fn output_changed_under_recording_is_seen(cs in cases()) {
        let (text, stdout) = jsonl(&cs, &[]);
        let (_, same, _, _) = calls::from_jsonl(&text, Some(&(stdout + "x"))).unwrap();
        prop_assert_eq!(same, "differ");
    }
}

/// a faithful executor of the spec/calls.txt lines: each op's call with the case's arguments,
/// slot s -> handle 100 + s, other calls in between
fn faithful(ops: &[Vec<String>], rules: &[calls::Rule], noise: &[usize]) -> Vec<Call> {
    let mut v = Vec::new();
    for (i, t) in ops.iter().enumerate() {
        for _ in 0..noise.get(i).copied().unwrap_or(0) {
            v.push(Call { api: "SelectObject".into(), args: vec![Val::Opaque(7)], ret: Some(Val::Opaque(8)), err: None });
        }
        for r in rules.iter().filter(|r| r.op == t[0] && r.api != "-") {
            let args = r
                .args
                .iter()
                .map(|a| match a {
                    calls::Arg::Slot(n) => Val::Opaque(100 + t[*n].parse::<u64>().unwrap()),
                    calls::Arg::Num(n) => Val::Text(kikizake::case::num(&t[*n]).unwrap().to_string()),
                    calls::Arg::Color(n) => {
                        let c = kikizake::case::color(&t[*n]).unwrap();
                        Val::Text(((c & 0xff) << 16 | (c & 0xff00) | (c >> 16 & 0xff)).to_string())
                    }
                    calls::Arg::Lit(x) => Val::Text(x.to_string()),
                    calls::Arg::Any => Val::Opaque(9),
                })
                .collect();
            v.push(Call { api: r.api.clone(), args, ret: Some(Val::Text("1".into())), err: None });
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

fn rules() -> Vec<calls::Rule> {
    calls::rules(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spec/calls.txt")).unwrap()).unwrap()
}

fn setup(ops: &[Vec<String>], noise: &[usize]) -> (Vec<kikizake::case::Case>, calls::Recording) {
    let rs = rules();
    let case = kikizake::case::Case { name: "x".into(), ops: ops.iter().enumerate().map(|(i, t)| kikizake::case::Op { line: i + 2, tok: t.clone() }).collect() };
    let rec = calls::Recording { header: String::new(), cases: vec![calls::Case { name: "x".into(), bad: vec![], calls: faithful(ops, &rs, noise) }] };
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
        let nums: Vec<usize> = (0..calls_[k].args.len()).filter(|&i| matches!(calls_[k].args[i], Val::Text(_))).collect();
        let a = nums[arg.index(nums.len())];
        let Val::Text(t) = &calls_[k].args[a] else { unreachable!() };
        calls_[k].args[a] = Val::Text((t.parse::<i64>().unwrap() ^ 0x40).to_string());
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
        rec.cases[0].bad = vec![k + 2];
        prop_assert_eq!(calls::check_ops(&cases, &rec, &rules()), Vec::<String>::new());
    }
}

#[test]
fn x86_and_x86_64_differences_are_found() {
    let c = |r: &str| calls::Case { name: "a".into(), bad: vec![], calls: vec![Call { api: "BitBlt".into(), args: vec![Val::Opaque(1), Val::Text("3".into())], ret: Some(Val::Text(r.into())), err: None }] };
    let a = calls::Recording { header: String::new(), cases: vec![c("1")] };
    let b = calls::Recording { header: String::new(), cases: vec![c("0")] };
    assert!(calls::differ(&a, &a).is_empty());
    assert_eq!(calls::differ(&a, &b).len(), 1);
}
