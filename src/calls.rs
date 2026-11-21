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
//!   kikizake calls <suite>-<arch> cases <fnv64 of the case file> src <runner/golden.py src> shirusu <rev> stdout <same|repeated|differ|truncated|-> events <n> other <n>
//!   case <name>
//!   bad <line>                                    a "bad line" the executor printed for it
//!   <API>(<arg>,<arg>,...) = <return>[ ! <last error>]
//! Values: integers in decimal or (unsigned 32-bit ones of 0x10000 and more) hex, handles and
//! pointers numbered h1, h2, ... in the order they first appear in the case (0 is h0), values
//! shirusu did not capture as <how> (<unsupported>, ...), a call that never returned `= ?`, one
//! that returns nothing `= void`. The last error is there only when shirusu says it means
//! something (the call failed by the API's contract). The executor's own output calls
//! (WriteFile, GetStdHandle) are left out: their data cut the recording into cases (output is
//! flushed at every case's end, SPEC.md).
use crate::json::{self, Json};
use std::collections::HashMap;
use std::fmt;

/// An API's name without its module ("BitBlt").
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Api(pub String);

/// A handle or pointer value: compared by identity within a case, never by value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Handle(pub u64);

/// A line of a case file, counted from 1 (as "bad line <n>" counts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Line(pub usize);

/// A last error a call left, when shirusu says it means something.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LastError(pub u32);

/// How shirusu captured a value (shirusu-trace's ValueCapture).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capture {
    Captured,
    Unreadable,
    Truncated,
    Unsupported,
    LimitExceeded,
    Stale,
}

impl Capture {
    const ALL: [Capture; 6] = [Capture::Captured, Capture::Unreadable, Capture::Truncated, Capture::Unsupported, Capture::LimitExceeded, Capture::Stale];
    pub fn name(self) -> &'static str {
        match self {
            Capture::Captured => "captured",
            Capture::Unreadable => "unreadable",
            Capture::Truncated => "truncated",
            Capture::Unsupported => "unsupported",
            Capture::LimitExceeded => "limit_exceeded",
            Capture::Stale => "stale",
        }
    }
    pub fn from_name(s: &str) -> Option<Capture> {
        Capture::ALL.into_iter().find(|c| c.name() == s)
    }
}

/// A value's type in shirusu's JSON ("ty": shirusu-trace's Ty, by its Debug name).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ty {
    I32,
    I64,
    U32,
    U64,
    F32,
    F64,
    Bool,
    Str,
    Str16,
    Bytes,
    BlobRef,
    Ptr32,
    Ptr64,
}

impl Ty {
    const ALL: [Ty; 13] = [Ty::I32, Ty::I64, Ty::U32, Ty::U64, Ty::F32, Ty::F64, Ty::Bool, Ty::Str, Ty::Str16, Ty::Bytes, Ty::BlobRef, Ty::Ptr32, Ty::Ptr64];
    fn name(self) -> &'static str {
        match self {
            Ty::I32 => "I32",
            Ty::I64 => "I64",
            Ty::U32 => "U32",
            Ty::U64 => "U64",
            Ty::F32 => "F32",
            Ty::F64 => "F64",
            Ty::Bool => "Bool",
            Ty::Str => "Str",
            Ty::Str16 => "Str16",
            Ty::Bytes => "Bytes",
            Ty::BlobRef => "BlobRef",
            Ty::Ptr32 => "Ptr32",
            Ty::Ptr64 => "Ptr64",
        }
    }
    fn from_name(s: &str) -> Option<Ty> {
        Ty::ALL.into_iter().find(|t| t.name() == s)
    }
}

/// What a value means by its Schema ("kind", shirusu-trace's ValueKind; absent: unspecified).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Unspecified,
    Scalar,
    Pointer,
    Handle,
    CreatedObject,
}

impl Kind {
    fn from_json(v: Option<&Json>) -> Result<Kind, String> {
        match v.map(Json::str) {
            None => Ok(Kind::Unspecified),
            Some(Some("scalar")) => Ok(Kind::Scalar),
            Some(Some("pointer")) => Ok(Kind::Pointer),
            Some(Some("handle")) => Ok(Kind::Handle),
            Some(Some("created_object")) => Ok(Kind::CreatedObject),
            Some(other) => Err(format!("unknown kind {other:?}")),
        }
    }
}

/// An event's phase (shirusu-analysis's Phase).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Enter,
    Leave,
    Aborted,
    Completed,
    Event,
}

impl Phase {
    fn from_name(s: &str) -> Option<Phase> {
        match s {
            "enter" => Some(Phase::Enter),
            "leave" => Some(Phase::Leave),
            "aborted" => Some(Phase::Aborted),
            "completed" => Some(Phase::Completed),
            "event" => Some(Phase::Event),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Val {
    /// a handle or pointer that is not 0
    Handle(Handle),
    /// a handle or pointer that is 0
    Null,
    /// a pointer that is not 0 (where the executor's memory is: the stack, the heap; differs
    /// between runs and architectures, so its identity is not kept)
    Ptr,
    Dec(i64),
    /// an unsigned 32-bit value of 0x10000 or more (a ROP, a color)
    Hex(u32),
    Bool(bool),
    /// a string as Rust's {:?} writes it
    Str(String),
    Bytes(usize),
    /// a value shirusu writes as text: float bits ("f32(0x...)"), a blob reference
    Text(String),
    /// a value shirusu did not capture
    Missing(Capture),
}

impl Val {
    /// the value as a 32-bit integer, for comparing with a case's token
    pub fn int32(&self) -> Option<u32> {
        match self {
            // (the ABI passes the low 32 bits: -1 and 0xffffffff are the same argument)
            Val::Dec(v) => Some(low32(*v)),
            Val::Hex(v) => Some(*v),
            Val::Null | Val::Handle(_) | Val::Ptr | Val::Bool(_) | Val::Str(_) | Val::Bytes(_) | Val::Text(_) | Val::Missing(_) => None,
        }
    }
    fn from_u64(x: u64) -> Val {
        match u32::try_from(x) {
            Ok(v) if v >= 0x10000 => Val::Hex(v),
            Ok(_) | Err(_) => match i64::try_from(x) {
                Ok(v) => Val::Dec(v),
                Err(_) => Val::Text(format!("{x:#x}")),
            },
        }
    }
}

/// The low 32 bits of an integer, as the ABI passes an int or a DWORD (-1 and 0xffffffff are the
/// same argument).
pub fn low32(v: i64) -> u32 {
    u32::try_from(v & 0xffff_ffff).unwrap_or(0)
}

/// What a call returned.
#[derive(Clone, Debug, PartialEq)]
pub enum Ret {
    Value(Val),
    Void,
    /// no leave event: the call never returned (or its event was lost)
    Never,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub api: Api,
    pub args: Vec<Val>,
    pub ret: Ret,
    pub err: Option<LastError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    pub name: String,
    pub bad: Vec<Line>,
    /// what the case printed under shirusu against the run without it
    pub out: Stdout,
    pub calls: Vec<Call>,
}

impl Case {
    fn new(name: &str) -> Case {
        Case { name: name.to_string(), bad: Vec::new(), out: Stdout::Unknown, calls: Vec::new() }
    }
}

/// What the executor printed under shirusu against independent plain runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stdout {
    Same,
    /// Complete case output matched an independent repeat on the same executor.
    Repeated,
    Differ,
    /// shirusu kept only the start of some WriteFile's data
    Truncated,
    /// no run without shirusu to compare with
    Unknown,
}

impl Stdout {
    const ALL: [Stdout; 5] = [Stdout::Same, Stdout::Repeated, Stdout::Differ, Stdout::Truncated, Stdout::Unknown];
    fn name(self) -> &'static str {
        match self {
            Stdout::Same => "same",
            Stdout::Repeated => "repeated",
            Stdout::Differ => "differ",
            Stdout::Truncated => "truncated",
            Stdout::Unknown => "-",
        }
    }
}

/// FNV-1a 64 of a case file: which cases a recording ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CasesHash(pub u64);

impl CasesHash {
    pub fn of(text: &str) -> CasesHash {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in text.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100000001b3);
        }
        CasesHash(h)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Header {
    /// <suite>-<arch>
    pub name: String,
    pub cases: CasesHash,
    /// runner/golden.py src of the executor
    pub src: String,
    /// the shirusu commit
    pub shirusu: String,
    pub stdout: Stdout,
    pub events: usize,
    /// events neither enter nor leave (loss records, aborted calls)
    pub other: usize,
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Header { name, cases, src, shirusu, stdout, events, other } = self;
        write!(f, "kikizake calls {name} cases {:016x} src {src} shirusu {shirusu} stdout {} events {events} other {other}", cases.0, stdout.name())
    }
}

impl Header {
    pub fn parse(l: &str) -> Result<Header, String> {
        let t: Vec<&str> = l.split(' ').collect();
        let [k, c, name, "cases", cases, "src", src, "shirusu", shirusu, "stdout", stdout, "events", events, "other", other] = t.as_slice() else {
            return Err(format!("not a calls header: {l}"));
        };
        if (*k, *c) != ("kikizake", "calls") {
            return Err(format!("not a calls header: {l}"));
        }
        let n = |s: &str| s.parse::<usize>().map_err(|_| format!("bad count {s}"));
        Ok(Header {
            name: name.to_string(),
            cases: CasesHash(u64::from_str_radix(cases, 16).map_err(|_| format!("bad cases hash {cases}"))?),
            src: src.to_string(),
            shirusu: shirusu.to_string(),
            stdout: Stdout::ALL.into_iter().find(|s| s.name() == *stdout).ok_or_else(|| format!("bad stdout {stdout}"))?,
            events: n(events)?,
            other: n(other)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Recording {
    pub header: Header,
    pub cases: Vec<Case>,
}

fn unhex(s: &str) -> Vec<u8> {
    let s = s.trim_end_matches("..");
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(s.get(2 * i..2 * i + 2)?, 16).ok()).collect()
}

fn hexnum(s: &str) -> Option<u64> {
    u64::from_str_radix(s.strip_prefix("0x")?, 16).ok()
}

fn capture_of(f: &Json) -> Result<Capture, String> {
    let c = f.get("capture").and_then(Json::str).ok_or("a field without capture")?;
    Capture::from_name(c).ok_or_else(|| format!("unknown capture {c}"))
}

/// one field of a shirusu event ({"ty", "capture", "value"[, "kind"]})
fn val(f: &Json) -> Result<Val, String> {
    let tyname = f.get("ty").and_then(Json::str).ok_or("a field without ty")?;
    let ty = Ty::from_name(tyname).ok_or_else(|| format!("unknown ty {tyname}"))?;
    let kind = Kind::from_json(f.get("kind"))?;
    let capture = capture_of(f)?;
    if capture != Capture::Captured {
        return Ok(Val::Missing(capture));
    }
    let v = f.get("value").ok_or("a captured field without value")?;
    let bad = || format!("{tyname} value {v:?}");
    let opaque = |s: &str| hexnum(s).map(|x| if x == 0 { Val::Null } else { Val::Handle(Handle(x)) }).ok_or_else(bad);
    let pointer = |s: &str| hexnum(s).map(|x| if x == 0 { Val::Null } else { Val::Ptr }).ok_or_else(bad);
    match ty {
        Ty::Ptr32 | Ty::Ptr64 => match kind {
            Kind::Pointer => pointer(v.str().ok_or_else(bad)?),
            Kind::Unspecified | Kind::Scalar | Kind::Handle | Kind::CreatedObject => opaque(v.str().ok_or_else(bad)?),
        },
        Ty::U64 => {
            let s = v.str().ok_or_else(bad)?;
            match kind {
                // (no kind: shirusu before 8525a38; a U64 is then a handle or pointer far more often
                // than a count in these APIs)
                Kind::Unspecified | Kind::Handle | Kind::CreatedObject => opaque(s),
                Kind::Pointer => pointer(s),
                Kind::Scalar => hexnum(s).map(Val::from_u64).ok_or_else(bad),
            }
        }
        Ty::U32 => v.u64().map(Val::from_u64).ok_or_else(bad),
        Ty::I32 | Ty::I64 => match v {
            Json::Num(n) => n.parse().map(Val::Dec).map_err(|_| bad()),
            Json::Null | Json::Bool(_) | Json::Str(_) | Json::Arr(_) | Json::Obj(_) => Err(bad()),
        },
        Ty::Bool => match v {
            Json::Bool(b) => Ok(Val::Bool(*b)),
            Json::Null | Json::Num(_) | Json::Str(_) | Json::Arr(_) | Json::Obj(_) => Err(bad()),
        },
        Ty::Str | Ty::Str16 => match v {
            Json::Str(s) => Ok(Val::Str(format!("{s:?}"))),
            // (UTF-16 that is not Unicode: its units)
            Json::Obj(_) => Ok(Val::Text("utf16".into())),
            Json::Null | Json::Bool(_) | Json::Num(_) | Json::Arr(_) => Err(bad()),
        },
        Ty::Bytes => v.str().map(|s| Val::Bytes(s.len() / 2)).ok_or_else(bad),
        Ty::F32 | Ty::F64 | Ty::BlobRef => v.str().map(|s| Val::Text(s.to_string())).ok_or_else(bad),
    }
}

/// The executor's own output calls: not recorded, WriteFile's data cuts the recording into cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Own {
    Write,
    Other,
}

fn own(api: &Api) -> Option<Own> {
    match api.0.as_str() {
        "WriteFile" => Some(Own::Write),
        "GetStdHandle" => Some(Own::Other),
        _ => None,
    }
}

/// The modules whose calls a recording keeps: the graphics APIs and the window manager. Calls into
/// kernel32 and the like come from the C runtime (heaps, locks, TLS; 32-bit mingw's runtime sets
/// the last error on every TLS read) and differ between x86 and x86-64 for that reason alone.
fn kept(module: &str) -> bool {
    let m = module.to_ascii_lowercase();
    let m = m.strip_suffix(".dll").unwrap_or(&m);
    GRAPHICS.contains(&m) || ["ole32", "oleaut32", "d2d1", "dwrite", "d3dx9_43", "glu32", "", "?"].contains(&m)
}

/// A recording from shirusu's JSONL export, its header's name, src and shirusu empty.
/// `stdout`: the executor's output from a run without shirusu, compared with what the recorded
/// WriteFile calls wrote.
pub fn from_jsonl(jsonl: &str, stdout: Option<&str>) -> Result<Recording, String> {
    from_jsonl_with_repeats(jsonl, stdout, &[])
}

/// Independent plain runs must use the same executable and architecture.
pub fn from_jsonl_with_repeats(jsonl: &str, stdout: Option<&str>, repeats: &[String]) -> Result<Recording, String> {
    let mut ev = Vec::new();
    for (i, l) in jsonl.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let e = json::parse(l).map_err(|m| format!("line {}: {m}", i + 1))?;
        let order = e.get("global_order").and_then(Json::u64).unwrap_or_else(|| u64::try_from(i).unwrap_or(u64::MAX));
        ev.push((order, i + 1, e));
    }
    ev.sort_by_key(|(o, _, _)| *o);
    let mut cases: Vec<Case> = Vec::new();
    // (every call in order, and the case it belongs to once the next output names it)
    let mut all: Vec<(Call, Option<usize>)> = Vec::new();
    let mut pending = 0;
    let mut open: HashMap<u64, usize> = HashMap::new();
    let mut written: Vec<u8> = Vec::new();
    // (the cases whose output shirusu kept only the start of)
    let mut truncated: Vec<usize> = Vec::new();
    let mut partial_mismatch = false;
    let mut other = 0;
    for (_, line, e) in &ev {
        let at = |m: String| format!("line {line}: {m}");
        let phase = e.get("phase").and_then(Json::str).ok_or_else(|| at("no phase".into()))?;
        let phase = Phase::from_name(phase).ok_or_else(|| at(format!("unknown phase {phase}")))?;
        let full = e.get("api").and_then(Json::str).unwrap_or("?");
        let (module, name) = full.split_once('!').unwrap_or(("", full));
        let api = Api(name.to_string());
        let id = e.get("call_id").and_then(Json::u64).unwrap_or(0);
        let fields: &[(String, Json)] = match e.get("fields") {
            Some(Json::Obj(p)) => p,
            None | Some(Json::Null | Json::Bool(_) | Json::Num(_) | Json::Str(_) | Json::Arr(_)) => &[],
        };
        // (the runtime's own calls, heaps, locks, TLS: what the C runtime does, not the case)
        if own(&api).is_none() && phase == Phase::Enter && !kept(module) {
            continue;
        }
        match (phase, own(&api)) {
            (Phase::Enter, Some(Own::Write)) => {
                let d = fields.iter().find(|(n, _)| n == "data").map(|(_, f)| f).ok_or_else(|| at("WriteFile without data".into()))?;
                let kept_bytes = d.get("value").and_then(Json::str).map_or_default(unhex);
                let cut = capture_of(d).map_err(at)? != Capture::Captured;
                // (shirusu keeps the start of a long write: the rest is what the plain run printed
                // there when it starts the same, so that the next write's case line stays a line;
                // the case is then Truncated, its tail not checked)
                let n = fields.iter().find(|(n, _)| n == "nNumberOfBytesToWrite").and_then(|(_, f)| f.get("value")).and_then(Json::u64).and_then(|n| usize::try_from(n).ok());
                let rest = n.and_then(|n| written.len().checked_add(n)).and_then(|end| stdout?.as_bytes().get(written.len()..end));
                if cut && rest.is_some_and(|full| !full.starts_with(&kept_bytes)) {
                    partial_mismatch = true;
                }
                let bytes = match rest {
                    Some(full) if cut && full.starts_with(&kept_bytes) => full.to_vec(),
                    Some(_) | None => kept_bytes,
                };
                written.extend_from_slice(&bytes);
                let text = String::from_utf8_lossy(&bytes);
                if let Some(n) = text.lines().find_map(|l| l.trim_end_matches('\r').strip_prefix("case ")) {
                    cases.push(Case::new(n));
                } else if cases.is_empty() {
                    cases.push(Case::new("-"));
                }
                let c = cases.len() - 1;
                if cut {
                    truncated.push(c);
                }
                for (_, o) in &mut all[pending..] {
                    *o = Some(c);
                }
                pending = all.len();
            }
            (Phase::Enter, Some(Own::Other)) => {}
            (Phase::Enter, None) => {
                open.insert(id, all.len());
                let args = fields.iter().map(|(_, f)| val(f)).collect::<Result<_, _>>().map_err(at)?;
                all.push((Call { api, args, ret: Ret::Never, err: None }, None));
            }
            (Phase::Leave, _) => {
                let Some(k) = open.remove(&id) else { continue };
                let call = &mut all[k].0;
                call.ret = Ret::Void;
                for (n, f) in fields {
                    if n == "return" {
                        call.ret = Ret::Value(val(f).map_err(at)?);
                    } else if n == "last_error" && capture_of(f).map_err(at)? == Capture::Captured {
                        let v = f.get("value").and_then(Json::u64).and_then(|v| u32::try_from(v).ok());
                        call.err = Some(LastError(v.ok_or_else(|| at("bad last_error".into()))?));
                    }
                }
            }
            (Phase::Aborted | Phase::Completed | Phase::Event, _) => other += 1,
        }
    }
    if pending < all.len() && cases.is_empty() {
        cases.push(Case::new("-"));
    }
    let last = cases.len().saturating_sub(1);
    for (c, o) in all {
        cases[o.unwrap_or(last)].calls.push(c);
    }
    let mut stdout_all = Stdout::Unknown;
    if let Some(out) = stdout {
        let mut by = HashMap::new();
        for (i, c) in cases.iter().enumerate() {
            by.entry(c.name.clone()).or_insert_with(std::collections::VecDeque::new).push_back(i);
        }
        let primary = crate::case::parse_output(out);
        let actual_text = String::from_utf8_lossy(&written);
        let actual = crate::case::parse_output(&actual_text);
        let same_sequence = |o: &crate::case::Output| o.head == primary.head && o.done == primary.done
            && !o.after_done && o.cases.iter().map(|c| &c.0).eq(primary.cases.iter().map(|c| &c.0));
        let alternatives: Vec<_> = repeats.iter().map(|text| crate::case::parse_output(text)).collect();
        if alternatives.iter().any(|o| !same_sequence(o)) {
            return Err("plain repeat has a different header, case sequence or termination".into());
        }
        let mut observed_repeat = false;
        for (position, (name, lines)) in primary.cases.iter().enumerate() {
            let Some(i) = by.get_mut(name).and_then(|indices| indices.pop_front()) else { continue };
            cases[i].bad = lines.iter().filter_map(|l| l.strip_prefix("bad line ")?.parse().ok().map(Line)).collect();
            let captured = actual.cases.get(position);
            cases[i].out = if truncated.contains(&i) {
                Stdout::Truncated
            } else if captured == primary.cases.get(position) {
                Stdout::Same
            } else if captured.is_some() && alternatives.iter().any(|run| run.cases.get(position) == captured) {
                observed_repeat = true;
                Stdout::Repeated
            } else {
                Stdout::Differ
            };
        }
        let unmatched = partial_mismatch || cases.iter().any(|c| c.out == Stdout::Differ);
        // Keep byte-level framing checks when all cases claim to match the primary.
        // A repeat can explain only complete case outputs in the same stream.
        let same_stream = if observed_repeat {
            same_sequence(&actual) && out.ends_with('\n') == actual_text.ends_with('\n')
        } else {
            out.as_bytes() == written.as_slice()
        };
        stdout_all = if unmatched || !same_stream {
            Stdout::Differ
        } else if !truncated.is_empty() {
            Stdout::Truncated
        } else if observed_repeat {
            Stdout::Repeated
        } else {
            Stdout::Same
        };
    }
    let stdout = stdout_all;
    let header = Header { name: String::new(), cases: CasesHash(0), src: String::new(), shirusu: String::new(), stdout, events: ev.len(), other };
    Ok(Recording { header, cases })
}

/// handles numbered by first appearance in a case
#[derive(Default)]
struct Ids(HashMap<Handle, usize>);

impl Ids {
    fn show(&mut self, v: &Val) -> String {
        match v {
            Val::Handle(h) => {
                let n = self.0.len() + 1;
                format!("h{}", self.0.entry(*h).or_insert(n))
            }
            Val::Null => "h0".into(),
            Val::Ptr => "p".into(),
            Val::Dec(x) => x.to_string(),
            Val::Hex(x) => format!("{x:#x}"),
            Val::Bool(b) => b.to_string(),
            Val::Str(s) | Val::Text(s) => s.clone(),
            Val::Bytes(n) => format!("bytes{n}"),
            Val::Missing(c) => format!("<{}>", c.name()),
        }
    }
}

pub fn format(r: &Recording) -> String {
    let mut s = format!("{}\n", r.header);
    for c in &r.cases {
        s += &format!("case {}\n", c.name);
        for b in &c.bad {
            s += &format!("bad {}\n", b.0);
        }
        match c.out {
            Stdout::Differ | Stdout::Truncated | Stdout::Repeated => s += &format!("out {}\n", c.out.name()),
            Stdout::Same | Stdout::Unknown => {}
        }
        let mut ids = Ids::default();
        for k in &c.calls {
            let a: Vec<String> = k.args.iter().map(|v| ids.show(v)).collect();
            let r = match &k.ret {
                Ret::Value(v) => ids.show(v),
                Ret::Void => "void".into(),
                Ret::Never => "?".into(),
            };
            s += &format!("{}({}) = {r}", k.api.0, a.join(","));
            if let Some(e) = k.err {
                s += &format!(" ! {}", e.0);
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

/// a value as `format` wrote it (handles as the numbers they were given)
fn read_val(t: &str) -> Val {
    if let Some(n) = t.strip_prefix('h').and_then(|n| n.parse::<u64>().ok()) {
        return if n == 0 { Val::Null } else { Val::Handle(Handle(n)) };
    }
    if let Some(c) = t.strip_prefix('<').and_then(|c| c.strip_suffix('>')).and_then(Capture::from_name) {
        return Val::Missing(c);
    }
    if let Some(n) = t.strip_prefix("bytes").and_then(|n| n.parse().ok()) {
        return Val::Bytes(n);
    }
    match t {
        "p" => return Val::Ptr,
        "true" => return Val::Bool(true),
        "false" => return Val::Bool(false),
        _ => {}
    }
    if t.starts_with('"') {
        return Val::Str(t.to_string());
    }
    if let Some(x) = hexnum(t).and_then(|x| u32::try_from(x).ok()).filter(|x| *x >= 0x10000 && format!("{x:#x}") == t) {
        return Val::Hex(x);
    }
    match t.parse::<i64>() {
        Ok(x) if x.to_string() == t => Val::Dec(x),
        Ok(_) | Err(_) => Val::Text(t.to_string()),
    }
}

/// a calls file back
pub fn parse(text: &str) -> Result<Recording, String> {
    let mut lines = text.lines().enumerate();
    let header = Header::parse(lines.next().map(|(_, l)| l).unwrap_or(""))?;
    let mut cases: Vec<Case> = Vec::new();
    for (i, l) in lines {
        let e = |m: &str| format!("line {}: {m}", i + 1);
        if let Some(n) = l.strip_prefix("case ") {
            cases.push(Case::new(n));
        } else if let Some(o) = l.strip_prefix("out ") {
            let c = cases.last_mut().ok_or_else(|| e("outside a case"))?;
            c.out = Stdout::ALL.into_iter().find(|s| s.name() == o).ok_or_else(|| e("bad out"))?;
        } else if let Some(b) = l.strip_prefix("bad ") {
            let c = cases.last_mut().ok_or_else(|| e("outside a case"))?;
            c.bad.push(Line(b.parse().map_err(|_| e("bad line number"))?));
        } else if !l.is_empty() {
            let c = cases.last_mut().ok_or_else(|| e("outside a case"))?;
            // (the name before the first '(', the arguments up to the last ") = " after it: split,
            // not sliced by positions found apart, so ") = (" is an error and not a panic, the
            // review of 8480456, 5.4)
            let (name, rest) = l.split_once('(').ok_or_else(|| e("no '('"))?;
            let (args, tail) = rest.rsplit_once(") = ").ok_or_else(|| e("no ') = ' after the '('"))?;
            let (ret, err) = match tail.split_once(" ! ") {
                Some((r, er)) => (r, Some(LastError(er.parse().map_err(|_| e("bad last error"))?))),
                None => (tail, None),
            };
            let ret = match ret {
                "?" => Ret::Never,
                "void" => Ret::Void,
                v => Ret::Value(read_val(v)),
            };
            c.calls.push(Call { api: Api(name.to_string()), args: split_args(args).iter().map(|t| read_val(t)).collect(), ret, err });
        }
    }
    Ok(Recording { header, cases })
}

/// A token of a case line, counted from 0 (the op).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tok(pub usize);

/// spec/calls.txt: which call a case line makes, with which of its tokens
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    /// the handle of the slot the token names (one slot, one handle, until the slot is new)
    Slot(Tok),
    /// the token read as a number
    Num(Tok),
    /// the token read as #RRGGBB, passed as a COLORREF (0x00BBGGRR)
    Color(Tok),
    /// this exact value
    Lit(i64),
    Any,
}

/// What a spec/calls.txt line says a case line does.
#[derive(Clone, Debug, PartialEq)]
pub enum Does {
    /// calls the API with these arguments
    Call(Api, Vec<Arg>),
    /// makes the slots these tokens name anew (their handles may change)
    Renew(Vec<Tok>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub op: String,
    pub does: Does,
}

pub fn rules(text: &str) -> Result<Vec<Rule>, String> {
    let mut v = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let t = crate::case::tokens(l);
        let [op, api, args @ ..] = t.as_slice() else {
            if t.is_empty() {
                continue;
            }
            return Err(format!("spec/calls.txt:{}: want <op> <API> <args>", i + 1));
        };
        let bad = |a: &str| format!("spec/calls.txt:{}: bad argument {a}", i + 1);
        let tok = |a: &str, s: &str| s.parse::<usize>().map(Tok).map_err(|_| bad(a));
        let does = if api == "-" {
            Does::Renew(args.iter().map(|a| tok(a, a.strip_prefix("new@").ok_or_else(|| bad(a))?)).collect::<Result<_, _>>()?)
        } else {
            let a = args
                .iter()
                .map(|a| {
                    if let Some(s) = a.strip_prefix('@') {
                        tok(a, s).map(Arg::Slot)
                    } else if let Some(s) = a.strip_prefix('%') {
                        tok(a, s).map(Arg::Color)
                    } else if let Some(s) = a.strip_prefix('=') {
                        crate::case::num(s).map(Arg::Lit).ok_or_else(|| bad(a))
                    } else if a == "*" {
                        Ok(Arg::Any)
                    } else {
                        tok(a, a).map(Arg::Num)
                    }
                })
                .collect::<Result<_, _>>()?;
            Does::Call(Api(api.clone()), a)
        };
        v.push(Rule { op: op.clone(), does });
    }
    Ok(v)
}

/// Where in a call spec/arch.txt allows a value to differ between the architectures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pos {
    /// the argument at this place, from 1
    Arg(usize),
    Ret,
}

/// spec/arch.txt: a value that differs between x86 and x86-64 by Windows' own types (a struct's
/// size), with every pair (x86 value, x86-64 value) it may take.
#[derive(Clone, Debug, PartialEq)]
pub struct ArchRule {
    pub api: Api,
    pub at: Pos,
    pub pairs: Vec<(i64, i64)>,
}

pub fn arch_rules(text: &str) -> Result<Vec<ArchRule>, String> {
    let mut v = Vec::new();
    for (i, l) in text.lines().enumerate() {
        let t = crate::case::tokens(l);
        let bad = |what: &str| format!("spec/arch.txt:{}: {what}", i + 1);
        let [api, at, pairs @ ..] = t.as_slice() else {
            if t.is_empty() {
                continue;
            }
            return Err(bad("want <API> <argument|ret> <x86>=<x86-64>..."));
        };
        let at = match at.as_str() {
            "ret" => Pos::Ret,
            n => Pos::Arg(n.parse().ok().filter(|n| *n >= 1).ok_or_else(|| bad("bad place"))?),
        };
        let pairs = pairs
            .iter()
            .map(|p| {
                let (a, b) = p.split_once('=').ok_or_else(|| bad("bad pair"))?;
                Ok((a.parse().map_err(|_| bad("bad pair"))?, b.parse().map_err(|_| bad("bad pair"))?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if pairs.is_empty() {
            return Err(bad("no pairs"));
        }
        v.push(ArchRule { api: Api(api.clone()), at, pairs });
    }
    Ok(v)
}

/// the same call on x86 (`a`) and x86-64 (`b`), with the values spec/arch.txt allows to differ
fn same_call(a: &Call, b: &Call, arch: &[ArchRule]) -> bool {
    if a.api != b.api || a.args.len() != b.args.len() || a.err != b.err {
        return false;
    }
    let allowed = |at: Pos, x: &Val, y: &Val| {
        arch.iter().any(|r| {
            r.api == a.api
                && r.at == at
                && match (x, y) {
                    (Val::Dec(x), Val::Dec(y)) => r.pairs.contains(&(*x, *y)),
                    (_, _) => false,
                }
        })
    };
    let args = a.args.iter().zip(&b.args).enumerate().all(|(i, (x, y))| x == y || allowed(Pos::Arg(i + 1), x, y));
    let ret = match (&a.ret, &b.ret) {
        (Ret::Value(x), Ret::Value(y)) => x == y || allowed(Pos::Ret, x, y),
        (x, y) => x == y,
    };
    args && ret
}

/// #RRGGBB (0x00RRGGBB) as the COLORREF the executor passes (0x00BBGGRR)
pub fn colorref(rgb: u32) -> u32 {
    (rgb & 0xff) << 16 | (rgb & 0xff00) | (rgb >> 16 & 0xff)
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
        let mut slot: HashMap<i64, Handle> = HashMap::new();
        let mut at = 0;
        for op in &case.ops {
            if got.bad.contains(&Line(op.line)) {
                continue;
            }
            let tok = |t: Tok| op.tok.get(t.0).map(String::as_str).unwrap_or("");
            for r in rules.iter().filter(|r| r.op == op.tok[0]) {
                let (api, args) = match &r.does {
                    Does::Renew(ts) => {
                        for t in ts {
                            let Some(id) = crate::case::num(tok(*t)) else {
                                bad.push(format!("{} line {}: invalid DC slot", case.name, op.line));
                                continue;
                            };
                            if r.op == "ddb" {
                                let Some((idx, bitmap)) = got.calls.iter().enumerate().skip(at)
                                    .find(|(_, c)| matches!(c.api.0.as_str(), "CreateBitmap" | "CreateCompatibleBitmap")) else {
                                    bad.push(format!("{} line {}: missing bitmap creation", case.name, op.line));
                                    continue;
                                };
                                at = idx.saturating_add(1);
                                if !matches!(bitmap.ret, Ret::Value(Val::Handle(Handle(h))) if h != 0) { continue; }
                            } else {
                                slot.remove(&id);
                            }
                            let Some((index, creation)) = got.calls.iter().enumerate().skip(at).find(|(_, c)| c.api.0 == "CreateCompatibleDC") else {
                                bad.push(format!("{} line {}: missing CreateCompatibleDC", case.name, op.line));
                                continue;
                            };
                            at = index.saturating_add(1);
                            if let Ret::Value(Val::Handle(h)) = creation.ret {
                                if r.op == "ddb" {
                                    let Some((idx, select)) = got.calls.iter().enumerate().skip(at).find(|(_, c)| c.api.0 == "SelectObject") else {
                                        bad.push(format!("{} line {}: missing bitmap selection", case.name, op.line));
                                        continue;
                                    };
                                    at = idx.saturating_add(1);
                                    if !matches!(select.ret, Ret::Value(Val::Handle(Handle(old))) if old != 0) { continue; }
                                }
                                if slot.iter().any(|(other, value)| *other != id && *value == h) {
                                    bad.push(format!("{} line {}: DC already assigned to a live slot", case.name, op.line));
                                }
                                slot.insert(id, h);
                            }
                        }
                        continue;
                    }
                    Does::Call(api, args) => (api, args),
                };
                let Some(k) = got.calls[at..].iter().position(|c| c.api == *api) else {
                    bad.push(format!("{} line {}: {}: no {} call after the earlier lines'", case.name, op.line, op.tok.join(" "), api.0));
                    continue;
                };
                let call = &got.calls[at + k];
                let mut why = Vec::new();
                if call.args.len() < args.len() {
                    why.push(format!("{} arguments recorded", call.args.len()));
                }
                for (i, (a, v)) in args.iter().zip(&call.args).enumerate() {
                    let num = |t: Tok| crate::case::num(tok(t)).map(low32);
                    let ok = match a {
                        Arg::Any => true,
                        Arg::Lit(x) => v.int32() == Some(low32(*x)) || (*x == 0 && *v == Val::Null),
                        Arg::Num(t) => num(*t).is_some_and(|x| v.int32() == Some(x) || (x == 0 && *v == Val::Null)),
                        Arg::Color(t) => crate::case::color(tok(*t)).is_some_and(|c| v.int32() == Some(colorref(c)) || (c == 0 && *v == Val::Null)),
                        Arg::Slot(t) => match v {
                            Val::Handle(h) => crate::case::num(tok(*t)).is_some_and(|id| !slot.iter().any(|(s, other)| *s != id && other == h) && *slot.entry(id).or_insert(*h) == *h),
                            Val::Null | Val::Ptr | Val::Dec(_) | Val::Hex(_) | Val::Bool(_) | Val::Str(_) | Val::Bytes(_) | Val::Text(_) | Val::Missing(_) => false,
                        },
                    };
                    if !ok {
                        why.push(format!("argument {} is {}", i + 1, Ids::default().show(v)));
                    }
                }
                if !why.is_empty() {
                    bad.push(format!("{} line {}: {}: {} {}", case.name, op.line, op.tok.join(" "), api.0, why.join(", ")));
                }
                at += k + 1;
            }
        }
    }
    bad
}

/// The cases whose calls (API, arguments, return) differ between two recordings of the same cases
/// (x86 and x86-64), with the first differing call of each.
pub fn differ(a: &Recording, b: &Recording, arch: &[ArchRule]) -> Vec<String> {
    let by: HashMap<&str, &Case> = b.cases.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut v = Vec::new();
    for c in &a.cases {
        let Some(d) = by.get(c.name.as_str()) else { continue };
        let n = c.calls.len().max(d.calls.len());
        let same = |i: usize| match (c.calls.get(i), d.calls.get(i)) {
            (Some(x), Some(y)) => same_call(x, y, arch),
            (x, y) => x == y,
        };
        if let Some(i) = (0..n).find(|&i| !same(i)) {
            let s = |x: Option<&Call>| x.map_or("-".to_string(), |k| format!("{}({:?}) = {:?}{}", k.api.0, k.args, k.ret, k.err.map_or_default(|e| format!(" ! {}", e.0))));
            v.push(format!("{}: call {}: {} / {}", c.name, i + 1, s(c.calls.get(i)), s(d.calls.get(i))));
        }
    }
    v
}

/// every failed call (one whose last error shirusu kept), as "<case>: <API> ! <error>"
pub fn failures(r: &Recording) -> Vec<String> {
    r.cases.iter().flat_map(|c| c.calls.iter().filter_map(move |k| k.err.map(|e| format!("{}: {} ! {}", c.name, k.api.0, e.0)))).collect()
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
    /// recordings a complete set wants and does not have current (check_complete only): a
    /// suite and architecture missing, or recorded from other cases, another executor source or
    /// another shirusu
    pub missing: Vec<String>,
    /// recordings whose stdout shirusu got only in part: their calls are checked, but not that
    /// recording left the output of those cases alone (a result of its own, not a problem)
    pub partial: Vec<String>,
}

impl Check {
    pub fn problems(&self) -> usize {
        self.ops.len() + self.arch.len() + self.stdout.len() + self.missing.len()
    }
    pub fn report(&self) -> String {
        let mut s = String::new();
        for (n, i) in &self.info {
            s += &format!("{n}: {i}\n");
        }
        for (what, v) in [("case line", &self.ops), ("x86 / x86-64", &self.arch), ("output", &self.stdout), ("missing", &self.missing), ("partial stdout", &self.partial), ("failed call", &self.failures)] {
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
    let rules = match std::fs::read_to_string(root.join("spec/calls.txt")).map_err(|e| e.to_string()).and_then(|t| rules(&t)) {
        Ok(r) => r,
        Err(e) => {
            ch.ops.push(format!("spec/calls.txt: {e}"));
            return ch;
        }
    };
    let arch = match std::fs::read_to_string(root.join("spec/arch.txt")).map_err(|e| e.to_string()).and_then(|t| arch_rules(&t)) {
        Ok(r) => r,
        Err(e) => {
            ch.arch.push(format!("spec/arch.txt: {e}"));
            return ch;
        }
    };
    // (cases whose answer Windows itself gives differently from run to run) (optional: without
    // the list every case must repeat, which is stricter, not laxer)
    let unstable: Vec<String> = std::fs::read_to_string(root.join("spec/unstable.txt"))
        .map_or_default(|t| t.lines().filter_map(|l| crate::case::tokens(l).first().cloned()).collect());
    let mut names = Vec::new();
    match std::fs::read_dir(dir) {
        Ok(entries) => for entry in entries {
            match entry {
                Ok(e) => if let Some(name) = e.file_name().to_str().and_then(|s| s.strip_suffix(".txt")) {
                    names.push(name.to_string());
                },
                Err(e) => ch.ops.push(format!("{}: {e}", dir.display())),
            }
        },
        Err(e) => { ch.ops.push(format!("{}: {e}", dir.display())); return ch; }
    }
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
        if rec.header.cases != CasesHash::of(&text) {
            ch.info.push((n.clone(), "recorded from other cases, waits for a new recording".into()));
            continue;
        }
        let calls: usize = rec.cases.iter().map(|c| c.calls.len()).sum();
        let bad = check_ops(&crate::case::parse_cases(&text), &rec, &rules);
        let f = failures(&rec);
        ch.info.push((n.clone(), format!("{} cases, {calls} calls, {} failed, {} case lines not as recorded, stdout {}", rec.cases.len(), f.len(), bad.len(), rec.header.stdout.name())));
        ch.ops.extend(bad.into_iter().map(|b| format!("{n} {b}")));
        ch.failures.extend(f.into_iter().map(|b| format!("{n} {b}")));
        let repeated = rec.cases.iter().filter(|c| c.out == Stdout::Repeated).count();
        if repeated > 0 { ch.info.push((n.clone(), format!("{repeated} complete case outputs matched a plain repeat"))); }
        let truncated = rec.cases.iter().filter(|c| c.out == Stdout::Truncated).count();
        if truncated > 0 {
            ch.info.push((n.clone(), format!("{truncated} cases with partial stdout")));
            ch.partial.push(format!("{n}: {truncated} of {} cases (their calls checked, their output not)", rec.cases.len()));
        }
        let differ: Vec<&Case> = rec.cases.iter().filter(|c| c.out == Stdout::Differ).collect();
        for c in &differ {
            if !unstable.iter().any(|u| *u == c.name) {
                ch.stdout.push(format!("{n} {}: printed something else under shirusu (and Windows is not known to answer it differently from run to run: spec/unstable.txt)", c.name));
            }
        }
        if rec.header.stdout == Stdout::Differ && differ.is_empty() {
            ch.stdout.push(format!("{n}: the output around the cases differs under shirusu"));
        }
        current.insert(n.clone(), rec);
    }
    if current.is_empty() { ch.ops.push("no current call recordings".into()); }
    for (n, a) in &current {
        if let Some(s) = n.strip_suffix("-32") && let Some(b) = current.get(&format!("{s}-64")) {
            ch.arch.extend(differ(a, b, &arch).into_iter().map(|d| format!("{s} {d}")));
        }
    }
    ch.arch.sort();
    ch
}

/// What a header must say to be current: its cases' hash, the executor source's hash
/// (runner/golden.py src) and the shirusu commit (shirusu.rev)
pub struct Wanted {
    pub cases: CasesHash,
    pub src: String,
    pub shirusu: String,
}

/// The problems of a set of recordings that is to be complete (`cargo test`, CI): every
/// (suite, architecture) of `required` has a recording in `dir` whose header is `want`'s; what
/// `check_dir` finds besides. `check_dir` alone checks the recordings there are (a partial
/// directory, the workflow's fresh ones); this is the success condition of the whole set (the
/// review of 8480456, 5.3: one recording alone, or one of an old source and shirusu, passed).
pub fn check_complete(dir: &std::path::Path, root: &std::path::Path, required: &[(String, Wanted)]) -> Check {
    let mut ch = check_dir(dir, root);
    for (n, want) in required {
        let path = dir.join(format!("{n}.txt"));
        let header = match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| parse(&t)) {
            Ok(r) => r.header,
            Err(e) => {
                ch.missing.push(format!("{n}: {e}"));
                continue;
            }
        };
        if header.cases != want.cases {
            ch.missing.push(format!("{n}: recorded from other cases"));
        }
        if header.src != want.src {
            ch.missing.push(format!("{n}: recorded from executor source {}, now {}", header.src, want.src));
        }
        if header.shirusu != want.shirusu {
            ch.missing.push(format!("{n}: recorded by shirusu {}, now {}", header.shirusu, want.shirusu));
        }
    }
    if required.is_empty() {
        ch.missing.push("no recordings required: spec/calls-suites.txt is empty".into());
    }
    ch
}

/// The recordings spec/calls-suites.txt asks for, each architecture its case file names, with
/// what their headers must say now (the executor source by runner/golden.py src)
pub fn required(root: &std::path::Path) -> Result<Vec<(String, Wanted)>, String> {
    let list = std::fs::read_to_string(root.join("spec/calls-suites.txt")).map_err(|e| format!("spec/calls-suites.txt: {e}"))?;
    let shirusu = std::fs::read_to_string(root.join("shirusu.rev")).map_err(|e| format!("shirusu.rev: {e}"))?.trim().to_string();
    let mut v = Vec::new();
    for s in list.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let text = std::fs::read_to_string(root.join(format!("cases/{s}.txt"))).map_err(|e| format!("cases/{s}.txt: {e}"))?;
        for arch in crate::generators::architectures(&text) {
            let n = format!("{s}-{arch}");
            let out = std::process::Command::new("python3").arg(root.join("runner/golden.py")).arg("src").arg(&n).current_dir(root).output().map_err(|e| format!("runner/golden.py: {e}"))?;
            if !out.status.success() {
                return Err(format!("runner/golden.py src {n}: {}", String::from_utf8_lossy(&out.stderr)));
            }
            let src = String::from_utf8_lossy(&out.stdout).trim().to_string();
            v.push((n, Wanted { cases: CasesHash::of(&text), src, shirusu: shirusu.clone() }));
        }
    }
    Ok(v)
}

/// The modules whose calls `usage` looks at: Windows' graphics APIs.
const GRAPHICS: [&str; 14] = ["gdi32", "gdi32full", "msimg32", "user32", "gdiplus", "opengl32", "ddraw", "d3d8", "d3d9", "d3d10", "d3d11", "d3d12", "dxgi", "windowscodecs"];

/// user32's drawing calls (the rest of user32 is windows and messages)
const USER32_DRAWING: [&str; 16] = ["FillRect", "FrameRect", "InvertRect", "DrawTextA", "DrawTextW", "DrawTextExA", "DrawTextExW", "DrawEdge", "DrawFocusRect", "DrawIcon", "DrawIconEx", "GetDC", "GetDCEx", "GetWindowDC", "ReleaseDC", "BeginPaint"];

/// How an application (a game, recorded by shirusu) uses the graphics APIs against what the
/// executor's recordings exercise: the APIs it calls that no case calls, and for each API both
/// call, the values of its small-domain arguments (at most 16 different ones in the
/// application: modes, ROPs, flags) that no case passes. One line each.
pub fn usage(app_jsonl: &str, recordings: &[Recording]) -> Result<Vec<String>, String> {
    let mut seen: HashMap<&str, Vec<Vec<String>>> = HashMap::new();
    for r in recordings {
        for c in &r.cases {
            for k in &c.calls {
                let e = seen.entry(k.api.0.as_str()).or_default();
                e.resize(e.len().max(k.args.len()), Vec::new());
                for (i, v) in k.args.iter().enumerate() {
                    let s = Ids::default().show(v);
                    if !e[i].contains(&s) {
                        e[i].push(s);
                    }
                }
            }
        }
    }
    // (API -> calls, per argument its values)
    let mut app: Vec<(String, String, usize, Vec<Vec<String>>)> = Vec::new();
    for (i, l) in app_jsonl.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let e = json::parse(l).map_err(|m| format!("line {}: {m}", i + 1))?;
        if e.get("phase").and_then(Json::str) != Some("enter") {
            continue;
        }
        let full = e.get("api").and_then(Json::str).unwrap_or("?");
        let Some((module, api)) = full.split_once('!') else { continue };
        let module = module.to_ascii_lowercase();
        let module = module.trim_end_matches(".dll");
        if !GRAPHICS.contains(&module) || (module == "user32" && !USER32_DRAWING.contains(&api)) {
            continue;
        }
        let fields: &[(String, Json)] = match e.get("fields") {
            Some(Json::Obj(p)) => p,
            None | Some(Json::Null | Json::Bool(_) | Json::Num(_) | Json::Str(_) | Json::Arr(_)) => &[],
        };
        let k = match app.iter().position(|(_, a, _, _)| a == api) {
            Some(k) => k,
            None => {
                app.push((module.to_string(), api.to_string(), 0, Vec::new()));
                app.len() - 1
            }
        };
        let entry = &mut app[k];
        entry.2 += 1;
        entry.3.resize(entry.3.len().max(fields.len()), Vec::new());
        for (j, (_, f)) in fields.iter().enumerate() {
            let v = val(f).map_err(|m| format!("line {}: {m}", i + 1))?;
            // (handles and pointers say nothing about coverage)
            match v {
                Val::Handle(_) | Val::Null | Val::Ptr | Val::Missing(_) | Val::Str(_) | Val::Bytes(_) | Val::Text(_) => continue,
                Val::Dec(_) | Val::Hex(_) | Val::Bool(_) => {}
            }
            let s = Ids::default().show(&v);
            if entry.3[j].len() <= 16 && !entry.3[j].contains(&s) {
                entry.3[j].push(s);
            }
        }
    }
    app.sort_by(|a, b| b.2.cmp(&a.2));
    let mut out = Vec::new();
    for (module, api, n, args) in &app {
        let Some(ours) = seen.get(api.as_str()) else {
            out.push(format!("{module}!{api}: {n} calls, no case calls it"));
            continue;
        };
        for (j, vals) in args.iter().enumerate() {
            if vals.len() > 16 {
                continue;
            }
            let missing: Vec<&str> = vals.iter().filter(|v| ours.get(j).is_none_or(|o| !o.contains(v))).map(String::as_str).collect();
            if !missing.is_empty() {
                out.push(format!("{module}!{api}: {n} calls, argument {}: {} not passed by any case", j + 1, missing.join(" ")));
            }
        }
    }
    Ok(out)
}
