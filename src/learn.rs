//! The documentation the clauses cite (spec/learn.txt): finding each page on Microsoft Learn and
//! checking the clauses against the pages as they are now. Uses curl (no crates).
//!
//!   kikizake learn pages     every page the clauses cite, and whether spec/learn.txt has it
//!   kikizake learn resolve   the pages spec/learn.txt lacks, found by Microsoft Learn's search:
//!                            lines to add (check them by eye: the first title that names the page)
//!   kikizake learn check     fetch every page; each clause's quoted words and the constants its
//!                            source names must be on one of its pages (exit 1 if not)

use std::collections::{BTreeMap, BTreeSet};

use crate::spec::clauses;
use crate::trace::{checkable, learn, pages};

/// every page cited, with the clauses citing it
pub fn cited() -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in clauses() {
        for p in pages(&c.source) {
            m.entry(p).or_default().push(c.id.clone());
        }
    }
    m
}

fn curl(url: &str) -> Option<String> {
    let o = std::process::Command::new("curl").args(["-sSfL", "--retry", "3", "--max-time", "60", "-A", "kikizake-learn-check", url]).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
}

fn enc(q: &str) -> String {
    q.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// the JSON string after `"key":"` from `at`
fn field(json: &str, key: &str, at: usize) -> Option<(String, usize)> {
    let k = format!("\"{key}\":\"");
    let i = json[at..].find(&k)? + at + k.len();
    let mut s = String::new();
    let mut it = json[i..].char_indices();
    while let Some((j, ch)) = it.next() {
        match ch {
            '"' => return Some((s, i + j)),
            '\\' => {
                if let Some((_, e)) = it.next() {
                    s.push(match e {
                        'n' => ' ',
                        _ => e,
                    })
                }
            }
            _ => s.push(ch),
        }
    }
    None
}

/// the page's address: the first search result whose title is the page's name (then a space, a
/// parenthesis or the end) on the Win32 / DirectX documentation
pub fn resolve(page: &str) -> Option<String> {
    let found = resolve1(page);
    // (an enumerator, X_Y_Z: the page of its enumeration, X_Y)
    if found.is_none() && page.contains('_') && page.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        let mut p = page;
        while let Some((head, _)) = p.rsplit_once('_') {
            if let Some(u) = resolve1(head) {
                return Some(u);
            }
            p = head;
        }
    }
    found
}

fn resolve1(page: &str) -> Option<String> {
    let json = curl(&format!("https://learn.microsoft.com/api/search?search={}&locale=en-us&%24top=10", enc(page)))?;
    let mut at = 0;
    let want = page.to_ascii_lowercase();
    let mut first = None;
    while let Some((title, i)) = field(&json, "title", at) {
        let Some((url, j)) = field(&json, "url", i) else { break };
        at = j;
        let t = title.to_ascii_lowercase();
        let named = t == want || t.strip_prefix(&want).is_some_and(|r| r.starts_with(' ') || r.starts_with('('));
        if named && url.contains("/windows/") {
            return Some(url);
        }
        if named && first.is_none() {
            first = Some(url);
        }
    }
    first
}

/// the page's words: tags out, entities decoded, white space one space, quotes plain, lower case
pub fn text(html: &str) -> String {
    let body = html.find("<main").map_or(html, |i| &html[i..]);
    let mut s = String::with_capacity(body.len());
    let mut tag = false;
    for ch in body.chars() {
        match ch {
            '<' => {
                tag = true;
                s.push(' ')
            }
            '>' => tag = false,
            _ if !tag => s.push(ch),
            _ => {}
        }
    }
    norm(&s.replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&nbsp;", " ").replace("&#160;", " "))
}

pub fn norm(s: &str) -> String {
    let s = s.replace(['\u{2018}', '\u{2019}'], "'").replace(['\u{201c}', '\u{201d}'], "\"").replace('\u{a0}', " ");
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

pub fn main(args: &[&str]) -> i32 {
    let known = learn();
    match args {
        ["pages"] => {
            for (p, ids) in cited() {
                println!("{} {p} ({})", if known.contains_key(&p) { "ok     " } else { "missing" }, ids.join(" "));
            }
            0
        }
        ["resolve"] => {
            for p in cited().keys().filter(|p| !known.contains_key(*p)) {
                match resolve(p) {
                    Some(u) => println!("{p} | {u}"),
                    None => eprintln!("not found: {p}"),
                }
            }
            0
        }
        ["check"] => {
            let mut pagetext: BTreeMap<String, Option<String>> = BTreeMap::new();
            let urls: BTreeSet<&String> = known.values().filter(|u| u.starts_with("http")).collect();
            for u in urls {
                pagetext.insert(u.clone(), curl(u).map(|h| text(&h)));
            }
            let mut bad = 0;
            for (p, u) in &known {
                if u.starts_with("http") && pagetext[u].is_none() {
                    println!("{p}: {u} does not load");
                    bad += 1;
                }
            }
            let (mut words, mut found) = (0, 0);
            for c in clauses() {
                let ps: Vec<&String> = pages(&c.source).iter().filter_map(|p| known.get(p)).filter_map(|u| pagetext.get(u).and_then(|t| t.as_ref())).collect();
                if ps.is_empty() {
                    continue;
                }
                for w in checkable(&c) {
                    words += 1;
                    let nw = norm(&w);
                    if ps.iter().any(|t| t.contains(&nw)) {
                        found += 1;
                    } else {
                        println!("{} ({}): {:?} is on none of {}", c.id, c.file, w, pages(&c.source).join(", "));
                        bad += 1;
                    }
                }
            }
            println!("{} pages, {words} quoted words and constants, {found} found, {bad} problems", pagetext.len());
            (bad > 0) as i32
        }
        _ => {
            eprintln!("usage: kikizake learn pages | resolve | check");
            2
        }
    }
}
