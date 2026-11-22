//! The Lean proofs (spec/lean/) against the models. Each theorem there is about a Lean
//! definition written as the model's function computes (its mirror); `kikizake lean <mirror>`
//! runs the Lean definitions (spec/lean's `mirror` executable) and the Rust functions on the
//! same arguments and fails on the first that answer differently. So a proof holds of the code
//! the goldens test: over the whole stated domain when the mirror is `Every`, on the stated
//! samples when it is `Sampled` (spec/lean/README.md lists which).

use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Command, Stdio};

mod d3d11;
mod d3d12;
mod d3d9;
mod d3d9tex;
mod f32;
mod dwrite;
mod dxgi;
mod gdi;
mod gdiplus;

/// what a mirror's arguments cover
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Domain {
    /// every argument of the domain named (the equality is then proved there by enumeration)
    Every(&'static str),
    /// the edges and a fixed pseudo-random sample of the domain named
    Sampled(&'static str),
}

/// one Lean definition and the model function it mirrors
pub struct Mirror {
    /// its name in spec/lean (`Mirror.name`)
    pub name: &'static str,
    /// the Lean file that defines it, from the repository's root
    pub lean: &'static str,
    pub domain: Domain,
    /// calls `each` with every argument list, the same ones in the same order each time
    pub inputs: fn(&mut dyn FnMut(&[i64])),
    /// the model's answer (None: arguments the model's function cannot take)
    pub model: fn(&[i64]) -> Option<Vec<i64>>,
}

/// every mirror, per API
pub fn mirrors() -> Vec<&'static Mirror> {
    // (one API a line)
    let all: &[&[Mirror]] = &[
        gdi::MIRRORS,
        gdiplus::MIRRORS,
        d3d12::MIRRORS,
        dxgi::MIRRORS,
        d3d9::MIRRORS,
        d3d9tex::MIRRORS,
        d3d11::MIRRORS,
        d3d11::DESC_MIRRORS,
        d3d11::VIEW_MIRRORS,
        d3d11::FILTER_MIRRORS,
        dwrite::MIRRORS,
        f32::MIRRORS,
    ];
    all.iter().flat_map(|m| m.iter()).collect()
}

/// the argument as the type the model takes
pub fn arg<T: TryFrom<i64>>(a: &[i64], i: usize) -> Option<T> {
    a.get(i).and_then(|v| T::try_from(*v).ok())
}

/// the binary64 of arguments i and i + 1, its two 32-bit halves
pub fn arg_f64(a: &[i64], i: usize) -> Option<f64> {
    let (hi, lo) = (arg::<u32>(a, i)?, arg::<u32>(a, i.checked_add(1)?)?);
    Some(f64::from_bits(u64::from(hi) << 32 | u64::from(lo)))
}

/// a binary64's two 32-bit halves, a NaN as the one quiet NaN
pub fn f64_halves(x: f64) -> Vec<i64> {
    let b = if x.is_nan() { 0x7ff8_0000_0000_0000 } else { x.to_bits() };
    vec![i64::from(u32::try_from(b >> 32).unwrap_or(0)), i64::from(u32::try_from(b & 0xffff_ffff).unwrap_or(0))]
}

/// a model's single answer
pub fn one(v: impl Into<i64>) -> Option<Vec<i64>> {
    Some(vec![v.into()])
}

/// every value of lo..=hi of each argument (the last one fastest)
pub fn every(ranges: &[(i64, i64)], each: &mut dyn FnMut(&[i64])) {
    if ranges.iter().any(|r| r.0 > r.1) {
        return;
    }
    let mut v: Vec<i64> = ranges.iter().map(|r| r.0).collect();
    loop {
        each(&v);
        let mut done = true;
        for (x, r) in v.iter_mut().zip(ranges).rev() {
            if *x < r.1 {
                *x = x.saturating_add(1);
                done = false;
                break;
            }
            *x = r.0;
        }
        if done {
            return;
        }
    }
}

/// every combination of the ranges' ends, then `n` pseudo-random argument lists in them
pub fn sampled(ranges: &[(i64, i64)], n: usize, seed: u32, each: &mut dyn FnMut(&[i64])) {
    let ends: Vec<(i64, i64)> = ranges.iter().map(|_| (0, 1)).collect();
    every(&ends, &mut |e| {
        let v: Vec<i64> = e.iter().zip(ranges).map(|(&k, r)| if k == 0 { r.0 } else { r.1 }).collect();
        each(&v);
    });
    let mut rng = crate::rng::XorShift32::new(seed);
    let mut v: Vec<i64> = ranges.iter().map(|r| r.0).collect();
    for _ in 0..n {
        for (x, r) in v.iter_mut().zip(ranges) {
            *x = rng.range(r.0, r.1);
        }
        each(&v);
    }
}

/// per mirror: (name, calls, first differences)
pub struct Report {
    pub name: &'static str,
    pub calls: usize,
    pub differ: Vec<String>,
}

fn line(name: &str, args: &[i64]) -> String {
    let mut s = String::from(name);
    for a in args {
        s.push(' ');
        s += &a.to_string();
    }
    s
}

fn answer(m: &Mirror, args: &[i64]) -> String {
    match (m.model)(args) {
        Some(v) => v.iter().map(i64::to_string).collect::<Vec<_>>().join(" "),
        None => "outside".into(),
    }
}

/// runs every mirror (or those whose name starts with `only`) through the Lean executable
pub fn check(exe: &str, only: Option<&str>) -> Result<Vec<Report>, String> {
    let ms: Vec<&'static Mirror> = mirrors().into_iter().filter(|m| only.is_none_or(|o| m.name.starts_with(o))).collect();
    if ms.is_empty() {
        return Err("no mirror by that name".into());
    }
    let mut child = Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().map_err(|e| format!("{exe}: {e}"))?;
    let stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let feed = ms.clone();
    let writer = std::thread::spawn(move || -> std::io::Result<()> {
        let mut w = BufWriter::with_capacity(1 << 20, stdin);
        for m in feed {
            let mut err = Ok(());
            (m.inputs)(&mut |a| {
                if err.is_ok() {
                    err = writeln!(w, "{}", line(m.name, a));
                }
            });
            err?;
        }
        w.flush()
    });
    let mut lines = BufReader::with_capacity(1 << 20, stdout).lines();
    let mut reports = Vec::new();
    let mut lost = None;
    for m in &ms {
        let mut r = Report { name: m.name, calls: 0, differ: Vec::new() };
        let mut differ = 0usize;
        (m.inputs)(&mut |a| {
            if lost.is_some() {
                return;
            }
            let got = match lines.next() {
                Some(Ok(l)) => l,
                Some(Err(e)) => { lost = Some(e.to_string()); return; }
                None => { lost = Some("the mirror stopped answering".into()); return; }
            };
            r.calls = r.calls.saturating_add(1);
            let want = answer(m, a);
            // (a mirror's inputs are its domain: an answer of neither side is a malformed input,
            // which would hide every difference)
            if got != want || want == "outside" {
                differ = differ.saturating_add(1);
                if r.differ.len() < 5 {
                    r.differ.push(format!("{}: lean {got}, model {want}", line(m.name, a)));
                }
            }
        });
        if differ > r.differ.len() {
            r.differ.push(format!("({differ} in all)"));
        }
        reports.push(r);
    }
    let fed = writer.join().map_err(|p| format!("the writer panicked: {p:?}"))?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if let Some(e) = lost {
        return Err(e);
    }
    fed.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("{exe}: {status}"));
    }
    Ok(reports)
}

pub fn main(args: &[&str]) -> i32 {
    let (exe, only) = match args {
        [exe] => (*exe, None),
        [exe, only] => (*exe, Some(*only)),
        _ => {
            eprintln!("usage: kikizake lean <spec/lean/.lake/build/bin/mirror> [mirror name prefix]");
            return 2;
        }
    };
    let reports = match check(exe, only) {
        Ok(r) => r,
        Err(e) => { eprintln!("{e}"); return 2; }
    };
    let mut bad = 0usize;
    for r in &reports {
        let state = if r.differ.is_empty() { "same" } else { "DIFFER" };
        println!("{} {} calls {state}", r.name, r.calls);
        for d in &r.differ {
            println!("  {d}");
        }
        bad = bad.saturating_add(usize::from(!r.differ.is_empty()));
    }
    println!("{} mirrors, {bad} differ from the model", reports.len());
    i32::from(bad > 0)
}
