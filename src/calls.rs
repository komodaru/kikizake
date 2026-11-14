//! What the executor called on Windows, as shirusu (github.com/komodaru/shirusu) recorded it:
//! calls/<suite>-<arch>.txt, made by the shirusu workflow from `shirusu export --format jsonl` of
//! the executor run under `shirusu capture`.
//!
//! A recording checks both sides:
//!   * kikizake: every case line spec/calls.txt names made the call SPEC.md says, with the case's
//!     arguments (the answers in golden/ are only as good as the executor's faithfulness), and
//!     the calls that failed are known;
//!   * shirusu: the recorded arguments are the ones the case gave (its argument decoding and its
//!     x86 / x86-64 ABI relay), x86 and x86-64 record the same calls, and recording does not
//!     change what the executor prints.
//!
//! File format, one line each:
//!   kikizake calls <suite>-<arch> cases <fnv64 of the case file> src <runner/golden.py src> shirusu <rev> stdout <same|differ|truncated|-> events <n> other <n>
//!   case <name>
//!   bad <line>                                    a "bad line" the executor printed for it
//!   <API>(<arg>,<arg>,...) = <return>[ ! <last error>]
//! Values: integers in decimal (unsigned 32-bit ones of 0x10000 and more in hex), handles and
//! pointers numbered h1, h2, ... in the order they first appear in the case (0 stays 0), values
//! shirusu did not capture as <how> (<unsupported>, ...), a call that never returned `= ?`. The
//! last error is there only when shirusu says it means something (the call failed by the API's
//! contract). The executor's own output calls (WriteFile, GetStdHandle) are left out: their
//! data cut the recording into cases (output is flushed at every case's end, SPEC.md).
use crate::json::{self, Json};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// a handle or pointer: compared by identity within a case
    Opaque(u64),
    Text(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub api: String,
    pub args: Vec<Val>,
    pub ret: Option<Val>,
    pub err: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Case {
    pub name: String,
    pub bad: Vec<usize>,
    pub calls: Vec<Call>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recording {
    pub header: String,
    pub cases: Vec<Case>,
}

impl Recording {
    pub fn field(&self, k: &str) -> Option<&str> {
        let t: Vec<&str> = self.header.split(' ').collect();
        t.iter().position(|w| *w == k).and_then(|i| t.get(i + 1).copied())
    }
}

/// FNV-1a 64 of a case file: which cases a recording ran
pub fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in text.bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn unhex(s: &str) -> Vec<u8> {
    let s = s.trim_end_matches("..");
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()).collect()
}

fn hexnum(s: &str) -> Option<u64> {
    u64::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

/// one field of a shirusu event ({"ty", "capture", "value"[, "kind"]})
fn val(f: &Json) -> Val {
    let ty = f.get("ty").and_then(Json::str).unwrap_or("");
    let cap = f.get("capture").and_then(Json::str).unwrap_or("?");
    if cap != "captured" {
        return Val::Text(format!("<{cap}>"));
    }
    let v = f.get("value").unwrap_or(&Json::Null);
    let scalar = f.get("kind").and_then(Json::str) == Some("scalar");
    match (ty, v) {
        ("U64" | "Ptr32" | "Ptr64", Json::Str(s)) if !scalar => hexnum(s).map_or(Val::Text(s.clone()), Val::Opaque),
        ("U32", Json::Num(n)) => match n.parse::<u64>() {
            Ok(x) if x >= 0x10000 => Val::Text(format!("{x:#x}")),
            _ => Val::Text(n.clone()),
        },
        (_, Json::Num(n)) => Val::Text(n.clone()),
        (_, Json::Bool(b)) => Val::Text(b.to_string()),
        ("Str" | "Str16", Json::Str(s)) => Val::Text(format!("{s:?}")),
        ("Bytes", Json::Str(s)) => Val::Text(format!("bytes{}", s.len() / 2)),
        (_, Json::Str(s)) => Val::Text(s.clone()),
        _ => Val::Text("<?>".into()),
    }
}

const OUTPUT: [&str; 2] = ["WriteFile", "GetStdHandle"];

/// A recording from shirusu's JSONL export. `stdout`: the executor's output from a run without
/// shirusu, compared with what the recorded WriteFile calls wrote.
pub fn from_jsonl(jsonl: &str, stdout: Option<&str>) -> Result<(Vec<Case>, &'static str, usize, usize), String> {
    let mut ev = Vec::new();
    for (i, l) in jsonl.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let e = json::parse(l).map_err(|m| format!("line {}: {m}", i + 1))?;
        let order = e.get("global_order").and_then(Json::u64).unwrap_or(i as u64);
        ev.push((order, e));
    }
    ev.sort_by_key(|(o, _)| *o);
    let mut cases: Vec<Case> = Vec::new();
    // (every call in order, the case it belongs to once the next output names it)
    let (mut all, mut owner): (Vec<Call>, Vec<usize>) = (Vec::new(), Vec::new());
    let mut pending = 0;
    let mut open: HashMap<u64, usize> = HashMap::new();
    let mut written: Vec<u8> = Vec::new();
    let mut truncated = false;
    let mut other = 0;
    for (_, e) in &ev {
        let phase = e.get("phase").and_then(Json::str).unwrap_or("");
        let api = e.get("api").and_then(Json::str).unwrap_or("?");
        let api = api.rsplit('!').next().unwrap_or(api).to_string();
        let id = e.get("call_id").and_then(Json::u64).unwrap_or(0);
        let fields = match e.get("fields") {
            Some(Json::Obj(p)) => p.as_slice(),
            _ => &[],
        };
        match phase {
            "enter" if api == "WriteFile" => {
                let d = fields.iter().find(|(n, _)| n == "data").map(|(_, f)| f);
                let cap = d.and_then(|f| f.get("capture")).and_then(Json::str).unwrap_or("");
                let bytes = d.and_then(|f| f.get("value")).and_then(Json::str).map(unhex).unwrap_or_default();
                truncated |= cap != "captured";
                written.extend_from_slice(&bytes);
                let text = String::from_utf8_lossy(&bytes);
                if let Some(n) = text.lines().find_map(|l| l.trim_end_matches('\r').strip_prefix("case ")) {
                    cases.push(Case { name: n.to_string(), ..Default::default() });
                } else if cases.is_empty() {
                    cases.push(Case { name: "-".into(), ..Default::default() });
                }
                owner[pending..].fill(cases.len() - 1);
                pending = all.len();
            }
            "enter" if OUTPUT.contains(&api.as_str()) => {}
            "enter" => {
                open.insert(id, all.len());
                all.push(Call { api, args: fields.iter().map(|(_, f)| val(f)).collect(), ret: None, err: None });
                owner.push(usize::MAX);
            }
            "leave" => {
                let Some(k) = open.remove(&id) else { continue };
                let call = &mut all[k];
                for (n, f) in fields {
                    if n == "return" {
                        call.ret = Some(val(f));
                    } else if n == "last_error" && f.get("capture").and_then(Json::str) == Some("captured") {
                        call.err = f.get("value").map(|v| match v {
                            Json::Num(n) => n.clone(),
                            _ => "?".into(),
                        });
                    }
                }
                if call.ret.is_none() {
                    call.ret = Some(Val::Text("void".into()));
                }
            }
            _ => other += 1,
        }
    }
    if pending < all.len() {
        if cases.is_empty() {
            cases.push(Case { name: "-".into(), ..Default::default() });
        }
        owner[pending..].fill(cases.len() - 1);
    }
    for (c, o) in all.into_iter().zip(owner) {
        cases[o].calls.push(c);
    }
    if let Some(out) = stdout {
        let mut by = HashMap::new();
        for (i, c) in cases.iter().enumerate() {
            by.entry(c.name.clone()).or_insert(i);
        }
        let o = crate::case::parse_output(out);
        for (n, lines) in &o.cases {
            if let Some(&i) = by.get(n) {
                cases[i].bad = lines.iter().filter_map(|l| l.strip_prefix("bad line ")?.parse().ok()).collect();
            }
        }
    }
    let same = match stdout {
        None => "-",
        Some(_) if truncated => "truncated",
        Some(s) if s.as_bytes() == written.as_slice() => "same",
        Some(_) => "differ",
    };
    Ok((cases, same, ev.len(), other))
}

fn show(v: &Val, ids: &mut HashMap<u64, usize>) -> String {
    match v {
        Val::Opaque(0) => "0".into(),
        Val::Opaque(x) => {
            let n = ids.len() + 1;
            format!("h{}", ids.entry(*x).or_insert(n))
        }
        Val::Text(t) => t.clone(),
    }
}

pub fn format(r: &Recording) -> String {
    let mut s = format!("{}\n", r.header);
    for c in &r.cases {
        s += &format!("case {}\n", c.name);
        for b in &c.bad {
            s += &format!("bad {b}\n");
        }
        let mut ids = HashMap::new();
        for k in &c.calls {
            let a: Vec<String> = k.args.iter().map(|v| show(v, &mut ids)).collect();
            let r = k.ret.as_ref().map_or("?".into(), |v| show(v, &mut ids));
            s += &format!("{}({}) = {r}", k.api, a.join(","));
            if let Some(e) = &k.err {
                s += &format!(" ! {e}");
            }
            s += "\n";
        }
    }
    s
}

/// split "a,b,"c,d"" at the commas outside quotes
fn split_args(s: &str) -> Vec<String> {
    let (mut v, mut cur, mut q, mut esc) = (Vec::new(), String::new(), false, false);
    for ch in s.chars() {
        if q {
            if esc {
                esc = false;
            } else if ch == '\\' {
                esc = true;
            } else if ch == '"' {
                q = false;
            }
            cur.push(ch);
        } else if ch == '"' {
            q = true;
            cur.push(ch);
        } else if ch == ',' {
            v.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() || !v.is_empty() {
        v.push(cur);
    }
    v
}

fn read_val(t: &str) -> Val {
    match t.strip_prefix('h').and_then(|n| n.parse().ok()) {
        Some(n) => Val::Opaque(n),
        None => Val::Text(t.to_string()),
    }
}

/// a calls file back (handles as the numbers they were given)
pub fn parse(text: &str) -> Result<Recording, String> {
    let mut r = Recording::default();
    for (i, l) in text.lines().enumerate() {
        let e = |m: &str| format!("line {}: {m}", i + 1);
        if i == 0 {
            if !l.starts_with("kikizake calls ") {
                return Err(e("not a calls file"));
            }
            r.header = l.to_string();
        } else if let Some(n) = l.strip_prefix("case ") {
            r.cases.push(Case { name: n.to_string(), ..Default::default() });
        } else if let Some(b) = l.strip_prefix("bad ") {
            let c = r.cases.last_mut().ok_or_else(|| e("outside a case"))?;
            c.bad.push(b.parse().map_err(|_| e("bad line number"))?);
        } else if !l.is_empty() {
            let c = r.cases.last_mut().ok_or_else(|| e("outside a case"))?;
            let open = l.find('(').ok_or_else(|| e("no '('"))?;
            let close = l.rfind(") = ").ok_or_else(|| e("no ') = '"))?;
            let (ret, err) = match l[close + 4..].split_once(" ! ") {
                Some((r, er)) => (r, Some(er.to_string())),
                None => (&l[close + 4..], None),
            };
            c.calls.push(Call {
                api: l[..open].to_string(),
                args: split_args(&l[open + 1..close]).iter().map(|t| read_val(t)).collect(),
                ret: if ret == "?" { None } else { Some(read_val(ret)) },
                err,
            });
        }
    }
    Ok(r)
}

/// spec/calls.txt: which call a case line makes, with which of its tokens
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// the handle of the slot named by token n (one slot, one handle, until `new`)
    Slot(usize),
    /// token n read as a number
    Num(usize),
    /// token n read as #RRGGBB, passed as a COLORREF (0x00BBGGRR)
    Color(usize),
    /// this exact value
    Lit(i64),
    Any,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub op: String,
    /// "-": the line makes the slots `new` names new (their handles may change)
    pub api: String,
    pub args: Vec<Arg>,
    pub new: Vec<usize>,
}

pub fn rules(text: &str) -> Result<Vec<Rule>, String> {
    let mut v = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let t = crate::case::tokens(l);
        if t.is_empty() {
            continue;
        }
        if t.len() < 2 {
            return Err(format!("spec/calls.txt:{}: want <op> <API> <args>", i + 1));
        }
        let mut r = Rule { op: t[0].clone(), api: t[1].clone(), args: Vec::new(), new: Vec::new() };
        for a in &t[2..] {
            let bad = || format!("spec/calls.txt:{}: bad argument {a}", i + 1);
            let n = |s: &str| s.parse::<usize>().map_err(|_| bad());
            if let Some(s) = a.strip_prefix("new@") {
                r.new.push(n(s)?);
            } else if let Some(s) = a.strip_prefix('@') {
                r.args.push(Arg::Slot(n(s)?));
            } else if let Some(s) = a.strip_prefix('%') {
                r.args.push(Arg::Color(n(s)?));
            } else if let Some(s) = a.strip_prefix('=') {
                r.args.push(Arg::Lit(crate::case::num(s).ok_or_else(bad)?));
            } else if a == "*" {
                r.args.push(Arg::Any);
            } else {
                r.args.push(Arg::Num(n(a)?));
            }
        }
        v.push(r);
    }
    Ok(v)
}

fn as_int(v: &Val) -> Option<i64> {
    match v {
        Val::Text(t) => crate::case::num(t),
        Val::Opaque(_) => None,
    }
}

/// The case lines spec/calls.txt names against the recorded calls of the same cases: each such
/// line (not a bad line) must be followed, in order, by its call, and that call must be the next
/// call of that API. Returns what does not hold, one line each.
pub fn check_ops(cases: &[crate::case::Case], rec: &Recording, rules: &[Rule]) -> Vec<String> {
    let by: HashMap<&str, &Case> = rec.cases.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut bad = Vec::new();
    for case in cases {
        let Some(got) = by.get(case.name.as_str()) else {
            bad.push(format!("{}: not recorded", case.name));
            continue;
        };
        let mut slot: HashMap<String, u64> = HashMap::new();
        let mut at = 0;
        for op in &case.ops {
            if got.bad.contains(&op.line) {
                continue;
            }
            for r in rules.iter().filter(|r| r.op == op.tok[0]) {
                for &n in &r.new {
                    if let Some(t) = op.tok.get(n) {
                        slot.remove(t);
                    }
                }
                if r.api == "-" {
                    continue;
                }
                let Some(k) = got.calls[at..].iter().position(|c| c.api == r.api) else {
                    bad.push(format!("{} line {}: {}: no {} call after the earlier lines'", case.name, op.line, op.tok.join(" "), r.api));
                    continue;
                };
                let call = &got.calls[at + k];
                let mut why = Vec::new();
                if call.args.len() < r.args.len() {
                    why.push(format!("{} arguments recorded", call.args.len()));
                }
                for (i, (a, v)) in r.args.iter().zip(&call.args).enumerate() {
                    let tok = |n: usize| op.tok.get(n).map(String::as_str).unwrap_or("");
                    let ok = match a {
                        Arg::Any => true,
                        Arg::Lit(x) => as_int(v).is_some_and(|y| y as u32 == *x as u32),
                        Arg::Num(n) => crate::case::num(tok(*n)).zip(as_int(v)).is_some_and(|(x, y)| x as u32 == y as u32),
                        Arg::Color(n) => crate::case::color(tok(*n)).zip(as_int(v)).is_some_and(|(c, y)| {
                            let bgr = (c & 0xff) << 16 | (c & 0xff00) | (c >> 16 & 0xff);
                            bgr == y as u32
                        }),
                        Arg::Slot(n) => match v {
                            Val::Opaque(h) => *slot.entry(tok(*n).to_string()).or_insert(*h) == *h,
                            _ => false,
                        },
                    };
                    if !ok {
                        why.push(format!("argument {} is {}", i + 1, show(v, &mut HashMap::new())));
                    }
                }
                if !why.is_empty() {
                    bad.push(format!("{} line {}: {}: {} {}", case.name, op.line, op.tok.join(" "), r.api, why.join(", ")));
                }
                at += k + 1;
            }
        }
    }
    bad
}

/// The cases whose calls (API, arguments, return) differ between two recordings of the same cases
/// (x86 and x86-64), with the first differing call of each.
pub fn differ(a: &Recording, b: &Recording) -> Vec<String> {
    let by: HashMap<&str, &Case> = b.cases.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut v = Vec::new();
    for c in &a.cases {
        let Some(d) = by.get(c.name.as_str()) else { continue };
        let n = c.calls.len().max(d.calls.len());
        if let Some(i) = (0..n).find(|&i| c.calls.get(i) != d.calls.get(i)) {
            let s = |x: Option<&Call>| x.map_or("-".to_string(), |k| format!("{}({:?}) = {:?}", k.api, k.args, k.ret));
            v.push(format!("{}: call {}: {} / {}", c.name, i + 1, s(c.calls.get(i)), s(d.calls.get(i))));
        }
    }
    v
}

/// every failed call (one whose last error shirusu kept), as "<case>: <API> ! <error>"
pub fn failures(r: &Recording) -> Vec<String> {
    let mut v = Vec::new();
    for c in &r.cases {
        for k in &c.calls {
            if let Some(e) = &k.err {
                v.push(format!("{}: {} ! {e}", c.name, k.api));
            }
        }
    }
    v
}

/// Every recording in a directory against the case files under `root` (cases/, spec/calls.txt).
#[derive(Debug, Default)]
pub struct Check {
    /// (recording, what), for the summary
    pub info: Vec<(String, String)>,
    /// case lines whose call is not as the case says
    pub ops: Vec<String>,
    /// cases whose x86 and x86-64 calls differ
    pub arch: Vec<String>,
    /// recordings whose executor printed something else than without shirusu
    pub stdout: Vec<String>,
    pub failures: Vec<String>,
}

impl Check {
    pub fn problems(&self) -> usize {
        self.ops.len() + self.arch.len() + self.stdout.len()
    }
    pub fn report(&self) -> String {
        let mut s = String::new();
        for (n, i) in &self.info {
            s += &format!("{n}: {i}\n");
        }
        for (what, v) in [("case line", &self.ops), ("x86 / x86-64", &self.arch), ("output", &self.stdout), ("failed call", &self.failures)] {
            for l in v {
                s += &format!("{what}: {l}\n");
            }
        }
        s += &format!("{} problems, {} failed calls\n", self.problems(), self.failures.len());
        s
    }
}

pub fn check_dir(dir: &std::path::Path, root: &std::path::Path) -> Check {
    let mut ch = Check::default();
    let rules = std::fs::read_to_string(root.join("spec/calls.txt")).map_err(|e| e.to_string()).and_then(|t| rules(&t));
    let rules = match rules {
        Ok(r) => r,
        Err(e) => {
            ch.ops.push(format!("spec/calls.txt: {e}"));
            return ch;
        }
    };
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|d| d.filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".txt").map(str::to_string)).collect())
        .unwrap_or_default();
    names.sort();
    let mut current: HashMap<String, Recording> = HashMap::new();
    for n in &names {
        let rec = match std::fs::read_to_string(dir.join(format!("{n}.txt"))).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
            Ok(r) => r,
            Err(e) => {
                ch.ops.push(format!("{n}: {e}"));
                continue;
            }
        };
        let suite = n.rsplit_once('-').map_or(n.as_str(), |(s, _)| s);
        let Ok(text) = std::fs::read_to_string(root.join(format!("cases/{suite}.txt"))) else {
            ch.info.push((n.clone(), "no such suite".into()));
            continue;
        };
        if rec.field("cases") != Some(fnv(&text).as_str()) {
            ch.info.push((n.clone(), "recorded from other cases, waits for a new recording".into()));
            continue;
        }
        let calls: usize = rec.cases.iter().map(|c| c.calls.len()).sum();
        let bad = check_ops(&crate::case::parse_cases(&text), &rec, &rules);
        let f = failures(&rec);
        ch.info.push((n.clone(), format!("{} cases, {calls} calls, {} failed, {} case lines not as recorded, stdout {}", rec.cases.len(), f.len(), bad.len(), rec.field("stdout").unwrap_or("?"))));
        ch.ops.extend(bad.into_iter().map(|b| format!("{n} {b}")));
        ch.failures.extend(f.into_iter().map(|b| format!("{n} {b}")));
        if rec.field("stdout") == Some("differ") {
            ch.stdout.push(format!("{n}: the executor printed something else under shirusu"));
        }
        current.insert(n.clone(), rec);
    }
    for (n, a) in &current {
        if let Some(s) = n.strip_suffix("-32") {
            if let Some(b) = current.get(&format!("{s}-64")) {
                ch.arch.extend(differ(a, b).into_iter().map(|d| format!("{s} {d}")));
            }
        }
    }
    ch.arch.sort();
    ch
}
