//! A small JSON reader (RFC 8259), enough for shirusu's JSONL export (src/calls.rs). Objects keep
//! their key order: shirusu writes an API's fields in its parameter order.

#[derive(Clone, Debug, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    /// the number as written (shirusu writes integers; the caller parses what it needs)
    Num(String),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, k: &str) -> Option<&Json> {
        match self {
            Json::Obj(p) => p.iter().find(|(n, _)| n == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn u64(&self) -> Option<u64> {
        match self {
            Json::Num(n) => n.parse().ok(),
            _ => None,
        }
    }
}

pub fn parse(s: &str) -> Result<Json, String> {
    let b = s.as_bytes();
    let mut i = 0;
    let v = value(b, &mut i, 0)?;
    ws(b, &mut i);
    if i != b.len() {
        return Err(format!("trailing bytes at {i}"));
    }
    Ok(v)
}

fn ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
        *i += 1;
    }
}

fn value(b: &[u8], i: &mut usize, depth: u32) -> Result<Json, String> {
    if depth > 128 {
        return Err("nested too deep".into());
    }
    ws(b, i);
    let Some(&c) = b.get(*i) else { return Err("unexpected end".into()) };
    match c {
        b'{' => {
            *i += 1;
            let mut p = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b'}') {
                *i += 1;
                return Ok(Json::Obj(p));
            }
            loop {
                ws(b, i);
                if b.get(*i) != Some(&b'"') {
                    return Err(format!("expected a key at {i}"));
                }
                let k = string(b, i)?;
                ws(b, i);
                if b.get(*i) != Some(&b':') {
                    return Err(format!("expected ':' at {i}"));
                }
                *i += 1;
                p.push((k, value(b, i, depth + 1)?));
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b'}') => {
                        *i += 1;
                        return Ok(Json::Obj(p));
                    }
                    _ => return Err(format!("expected ',' or '}}' at {i}")),
                }
            }
        }
        b'[' => {
            *i += 1;
            let mut a = Vec::new();
            ws(b, i);
            if b.get(*i) == Some(&b']') {
                *i += 1;
                return Ok(Json::Arr(a));
            }
            loop {
                a.push(value(b, i, depth + 1)?);
                ws(b, i);
                match b.get(*i) {
                    Some(b',') => *i += 1,
                    Some(b']') => {
                        *i += 1;
                        return Ok(Json::Arr(a));
                    }
                    _ => return Err(format!("expected ',' or ']' at {i}")),
                }
            }
        }
        b'"' => string(b, i).map(Json::Str),
        b't' | b'f' | b'n' => {
            for (w, v) in [("true", Json::Bool(true)), ("false", Json::Bool(false)), ("null", Json::Null)] {
                if b[*i..].starts_with(w.as_bytes()) {
                    *i += w.len();
                    return Ok(v);
                }
            }
            Err(format!("bad literal at {i}"))
        }
        _ => {
            let s = *i;
            if b.get(*i) == Some(&b'-') {
                *i += 1;
            }
            let digits = |i: &mut usize| {
                let s = *i;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    *i += 1;
                }
                *i > s
            };
            if !digits(i) {
                return Err(format!("bad value at {s}"));
            }
            if b.get(*i) == Some(&b'.') {
                *i += 1;
                if !digits(i) {
                    return Err(format!("bad number at {s}"));
                }
            }
            if matches!(b.get(*i), Some(b'e' | b'E')) {
                *i += 1;
                if matches!(b.get(*i), Some(b'+' | b'-')) {
                    *i += 1;
                }
                if !digits(i) {
                    return Err(format!("bad number at {s}"));
                }
            }
            Ok(Json::Num(String::from_utf8_lossy(&b[s..*i]).into_owned()))
        }
    }
}

fn hex4(b: &[u8], i: usize) -> Result<u32, String> {
    let h = b.get(i..i + 4).ok_or("short \\u escape")?;
    let h = std::str::from_utf8(h).map_err(|_| "bad \\u escape")?;
    u32::from_str_radix(h, 16).map_err(|_| "bad \\u escape".into())
}

fn string(b: &[u8], i: &mut usize) -> Result<String, String> {
    *i += 1;
    let mut s: Vec<u8> = Vec::new();
    loop {
        let Some(&c) = b.get(*i) else { return Err("unterminated string".into()) };
        *i += 1;
        match c {
            b'"' => return String::from_utf8(s).map_err(|_| "string is not UTF-8".into()),
            b'\\' => {
                let Some(&e) = b.get(*i) else { return Err("unterminated escape".into()) };
                *i += 1;
                let ch = match e {
                    b'"' => '"',
                    b'\\' => '\\',
                    b'/' => '/',
                    b'b' => '\u{8}',
                    b'f' => '\u{c}',
                    b'n' => '\n',
                    b'r' => '\r',
                    b't' => '\t',
                    b'u' => {
                        let mut u = hex4(b, *i)?;
                        *i += 4;
                        if (0xd800..0xdc00).contains(&u) && b.get(*i..*i + 2) == Some(b"\\u") {
                            let lo = hex4(b, *i + 2)?;
                            if (0xdc00..0xe000).contains(&lo) {
                                u = 0x10000 + ((u - 0xd800) << 10) + (lo - 0xdc00);
                                *i += 6;
                            }
                        }
                        char::from_u32(u).unwrap_or('\u{fffd}')
                    }
                    _ => return Err(format!("bad escape at {i}")),
                };
                let mut buf = [0; 4];
                s.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
            }
            0..=0x1f => return Err("control character in a string".into()),
            _ => s.push(c),
        }
    }
}
