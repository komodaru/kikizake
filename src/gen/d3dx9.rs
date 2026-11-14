//! The d3dx9 and d3dx9load suites' case generators (SPEC.md "D3DX 9").
use super::*;
use super::d3d9draw::FVFS;
use crate::model::d3d9draw::{TARGET_FORMATS, TEXTURE_FORMATS};

/// a random FVF code: mostly well formed (one position kind, optional parts, texture coordinate
/// counts and sizes), sometimes any bits
fn rfvf(r: &mut XorShift32) -> u32 {
    if r.chance(6) {
        return r.next() & if r.chance(2) { 0xffff } else { 0xffff_ffff };
    }
    let pos = *r.pick(&[0x002u32, 0x004, 0x006, 0x008, 0x00a, 0x00c, 0x00e, 0x4002]);
    let mut f = pos;
    for bit in [0x10, 0x20, 0x40, 0x80] {
        if r.chance(2) {
            f |= bit;
        }
    }
    let n = r.range(0, 8) as u32;
    f |= n << 8;
    for i in 0..n {
        f |= (r.range(0, 3) as u32) << (16 + 2 * i);
    }
    if (6..=0xe).contains(&pos) && r.chance(3) {
        f |= *r.pick(&[0x1000u32, 0x8000]);
    }
    f
}

pub fn d3dx9() -> String {
    let mut s = String::from("# kikizake suite d3dx9: D3DX 9's vertex format and 16-bit float functions (SPEC.md). Made by `kikizake gen d3dx9`.\n");
    let mut r = XorShift32::new(0xd3d0_0009);
    let mut fvfs: Vec<u32> = FVFS.to_vec();
    fvfs.extend((0..300).map(|_| rfvf(&mut r)));
    for (i, chunk) in fvfs.chunks(40).enumerate() {
        writeln!(s, "case fvf-{i}").unwrap();
        for f in chunk {
            writeln!(s, "x9fvfsize {f:#x}\nx9fvfdecl {f:#x}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // declarations: random elements (types, usages, offsets, streams), each asked for its FVF,
    // its vertex size and length
    for i in 0..8 {
        writeln!(s, "case decl-{i}").unwrap();
        for _ in 0..40 {
            let mut l = String::new();
            let mut off = 0u32;
            for k in 0..r.range(0, 6) {
                let ty = if r.chance(10) { r.range(0, 17) as u32 } else { *r.pick(&[0u32, 1, 2, 3, 4, 5, 15, 16, 2, 1]) };
                let usage = if k == 0 && r.chance(2) { *r.pick(&[0u32, 9]) } else { *r.pick(&[0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 5, 10, 3]) };
                let ui = if r.chance(3) { r.range(0, 8) as u32 } else { 0 };
                let stream = if r.chance(12) { 1 } else { 0 };
                let o = if r.chance(10) { off + 4 } else { off };
                write!(l, " {:#x} {:#x}", stream | o << 16, ty | usage << 16 | ui << 24).unwrap();
                off = o + crate::model::d3d9draw::decl_type_size(ty).unwrap_or(4);
            }
            writeln!(s, "x9declfvf{l}\nx9declsize{l}").unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // declarations made from FVFs, asked back for their FVF (and with one element changed)
    let decls: Vec<Vec<(u32, u32)>> = fvfs.iter().filter_map(|&f| crate::model::d3dx9::fvf_decl(f).ok()).collect();
    let mut q = XorShift32::new(0xdec1_f0f0);
    for (i, chunk) in decls.chunks(40).enumerate() {
        writeln!(s, "case declfvf-{i}").unwrap();
        for d in chunk {
            let mut d = d.clone();
            if q.chance(4) && !d.is_empty() {
                let k = q.range(0, d.len() as i64 - 1) as usize;
                d[k].1 ^= *q.pick(&[1u32, 1 << 16, 1 << 24, 1 << 8]);
            }
            writeln!(s, "x9declfvf{}", d.iter().map(|(a, b)| format!(" {a:#x} {b:#x}")).collect::<String>()).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // 32 -> 16 bits: special values, powers of two around the half range, every exponent's
    // rounding midpoints and their neighbors, random values
    let mut v: Vec<u32> = vec![0, 0x8000_0000, 0x7f80_0000, 0xff80_0000, 0x7fc0_0000, 0xffc0_0000, 0x7f80_0001, 0x3f80_0000, 0xbf80_0000, 0x477f_e000, 0x477f_f000, 0x4780_0000, 0x47ff_e000, 0x47ff_f000, 0x4800_0000, 0x7f7f_ffff, 1, 0x0080_0000];
    for e in -28i32..=18 {
        let b = ((127 + e) as u32) << 23;
        v.extend([b, b | 0x1000, b | 0x0fff, b | 0x1001, b | 0x3000, b | 0x2fff, b | 0x3001, b | 0x7f_f000, b | 0x7f_efff, b | 0x8000_0000]);
    }
    for _ in 0..600 {
        let h = r.next() & 0x7fff;
        // a half's value as f32 and the points around its next neighbor
        let f = f16_bits_to_f32_ieee(h as u16);
        let m = f.to_bits() + 0x1000;
        v.extend([m, m - 1, m + 1, f.to_bits() | (r.next() & 0x8000_0000)]);
    }
    v.extend((0..400).map(|_| r.next()));
    v.extend((0..400).map(|_| (r.next() & 0x87ff_ffff) | 0x3000_0000));
    for (i, chunk) in v.chunks(70).collect::<Vec<_>>().chunks(10).enumerate() {
        writeln!(s, "case f16-{i}").unwrap();
        for c in chunk {
            write!(s, "x9f16").unwrap();
            for x in *c {
                write!(s, " {x:#x}").unwrap();
            }
            writeln!(s).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    // 16 -> 32 bits: every half
    for (i, chunk) in (0..0x10000u32).collect::<Vec<_>>().chunks(70 * 64).enumerate() {
        writeln!(s, "case f32-{i}").unwrap();
        for c in chunk.chunks(70) {
            write!(s, "x9f32").unwrap();
            for x in c {
                write!(s, " {x:#x}").unwrap();
            }
            writeln!(s).unwrap();
        }
        writeln!(s, "end").unwrap();
    }
    s
}

/// IEEE 754 binary16 to f32 (for choosing test values only)
fn f16_bits_to_f32_ieee(h: u16) -> f32 {
    let (e, m) = ((h >> 10 & 31) as i32, (h & 0x3ff) as f32);
    let v = if e == 0 { m * 2f32.powi(-24) } else { (1.0 + m / 1024.0) * 2f32.powi(e - 15) };
    if h & 0x8000 != 0 {
        -v
    } else {
        v
    }
}

pub fn d3dx9load() -> String {
    let mut s = String::from("# gpu\n# kikizake suite d3dx9load: D3DXLoadSurfaceFromMemory between formats (SPEC.md). Made by `kikizake gen d3dx9load`.\n");
    let mut r = XorShift32::new(0xd3d0_10ad);
    // (not the palette formats: a NULL source palette is not asked)
    for src in TEXTURE_FORMATS.iter().filter(|f| !matches!(f.0, 40 | 41)) {
        writeln!(s, "case load-{}\ndevice swvp", src.1).unwrap();
        writeln!(s, "x9load 21 {} 16 1 ramp 1\nx9load 116 {} 16 1 ramp 1", src.0, src.0).unwrap();
        // (into the signed formats: a case of their own)
        let mut signed = String::new();
        for dst in TARGET_FORMATS {
            let l = format!("x9load {} {} 8 2 {:#x} 1\n", dst.0, src.0, r.next());
            if dst.0 == 60 { signed += &l } else { s += &l }
        }
        writeln!(s, "end\ncase load-{}-signed\ndevice swvp\n{signed}x9load 60 {} 64 1 ramp 1\nend", src.1, src.0).unwrap();
    }
    s
}
