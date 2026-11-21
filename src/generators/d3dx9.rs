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
    // (the processor first, an input of the model's x64 halves (D3DX9.F16.CPU): a stop later in
    // the suite must not take it away)
    s.push_str("case cpu\nx9cpu\nend\n");
    let mut r = XorShift32::new(0xd3d0_0009);
    let mut fvfs: Vec<u32> = FVFS.to_vec();
    fvfs.extend((0..300).map(|_| rfvf(&mut r)));
    for (i, chunk) in fvfs.chunks(40).enumerate() {
        writeln!(s, "case fvf-{i}");
        for f in chunk {
            writeln!(s, "x9fvfsize {f:#x}\nx9fvfdecl {f:#x}");
        }
        writeln!(s, "end");
    }
    // declarations: random elements (types, usages, offsets, streams), each asked for its FVF,
    // its vertex size and length
    for i in 0..8 {
        writeln!(s, "case decl-{i}");
        for _ in 0..40 {
            let mut l = String::new();
            let mut off = 0u32;
            for k in 0..r.range(0, 6) {
                let ty = if r.chance(10) { r.range(0, 17) as u32 } else { *r.pick(&[0u32, 1, 2, 3, 4, 5, 15, 16, 2, 1]) };
                let usage = if k == 0 && r.chance(2) { *r.pick(&[0u32, 9]) } else { *r.pick(&[0u32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 5, 10, 3]) };
                let ui = if r.chance(3) { r.range(0, 8) as u32 } else { 0 };
                let stream = if r.chance(12) { 1 } else { 0 };
                let o = if r.chance(10) { off + 4 } else { off };
                write!(l, " {:#x} {:#x}", stream | o << 16, ty | usage << 16 | ui << 24);
                off = o + crate::model::d3d9draw::decl_type_size(ty).unwrap_or(4);
            }
            writeln!(s, "x9declfvf{l}\nx9declsize{l}");
        }
        writeln!(s, "end");
    }
    // declarations made from FVFs, asked back for their FVF (and with one element changed)
    let decls: Vec<Vec<(u32, u32)>> = fvfs.iter().filter_map(|&f| crate::model::d3dx9::fvf_decl(f).ok()).collect();
    let mut q = XorShift32::new(0xdec1_f0f0);
    for (i, chunk) in decls.chunks(40).enumerate() {
        writeln!(s, "case declfvf-{i}");
        for d in chunk {
            let mut d = d.clone();
            if q.chance(4) && !d.is_empty() {
                let k = q.range(0, d.len() as i64 - 1) as usize;
                d[k].1 ^= *q.pick(&[1u32, 1 << 16, 1 << 24, 1 << 8]);
            }
            writeln!(s, "x9declfvf{}", d.iter().map(|(a, b)| format!(" {a:#x} {b:#x}")).collect::<String>());
        }
        writeln!(s, "end");
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
        writeln!(s, "case f16-{i}");
        for c in chunk {
            write!(s, "x9f16");
            for x in *c {
                write!(s, " {x:#x}");
            }
            writeln!(s);
        }
        writeln!(s, "end");
    }
    // 16 -> 32 bits: every half
    for (i, chunk) in (0..0x10000u32).collect::<Vec<_>>().chunks(70 * 64).enumerate() {
        writeln!(s, "case f32-{i}");
        for c in chunk.chunks(70) {
            write!(s, "x9f32");
            for x in c {
                write!(s, " {x:#x}");
            }
            writeln!(s);
        }
        writeln!(s, "end");
    }
    // (D3DX9.F16) floats just past a tie below 2^-14, where an 8-at-a-time conversion that adds
    // 2^-14 in f32 (x64's intelsse3, the clean-room notes) rounds up and one that shifts the
    // mantissa down rounds down: 8 at a time, then as an array's rest
    let ties = [0x3300_0401u32, 0x3300_07a0, 0x33c0_0306, 0x3420_0101, 0x3460_01fc, 0x3490_00b1, 0x3508_0041, 0x3578_0075, 0x3602_0011, 0x364e_0016, 0x36d9_000b, 0x37f7_4003, 0xb300_0401, 0xb420_0101, 0xb508_0041, 0xb602_0011];
    s.push_str("case f16-ties\n");
    for c in [&ties[..8], &ties[8..], &ties[..3]] {
        s.push_str(&format!("x9f16{}\n", c.iter().map(|x| format!(" {x:#x}")).collect::<String>()));
    }
    s.push_str("end\n");
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
    // (the processor first, an input of the half codecs on x64 (D3DX9.F16.CPU): the review of
    // 8480456, 4, where a stop before the last case lost it and 42 answered cases with it)
    s.push_str("case cpu\nx9cpu\nend\n");
    let mut r = XorShift32::new(0xd3d0_10ad);
    // (not the palette formats: a NULL source palette is not asked)
    for src in TEXTURE_FORMATS.iter().filter(|f| !matches!(f.0, 40 | 41)) {
        writeln!(s, "case load-{}\ndevice swvp", src.1);
        writeln!(s, "x9load 21 {} 16 1 ramp 1\nx9load 116 {} 16 1 ramp 1", src.0, src.0);
        // (into the signed formats: a case of their own)
        let mut signed = String::new();
        for dst in TARGET_FORMATS {
            let l = format!("x9load {} {} 8 2 {:#x} 1\n", dst.0, src.0, r.next());
            if dst.0 == 60 { signed += &l } else { s += &l }
        }
        writeln!(s, "end\ncase load-{}-signed\ndevice swvp\n{signed}x9load 60 {} 64 1 ramp 1\nend", src.1, src.0);
    }
    // (into the block and YUV formats: D3DX's encoders, from A8R8G8B8, A32B32G32R32F and X8R8G8B8,
    // a ramp, random pictures, a 1 x 1 and a 5 x 3 one past a block)
    for dst in TEXTURE_FORMATS.iter().filter(|f| matches!(f.1, "DXT1" | "DXT2" | "DXT3" | "DXT4" | "DXT5" | "UYVY" | "YUY2")) {
        writeln!(s, "case load-into-{}\ndevice swvp\nx9load {} 21 16 4 ramp 1\nx9load {} 116 16 4 ramp 1", dst.1, dst.0, dst.0);
        for (src, w, h) in [(21, 8, 8), (21, 8, 8), (21, 8, 4), (116, 8, 8), (22, 8, 4), (21, 1, 1), (21, 5, 3)] {
            writeln!(s, "x9load {} {src} {w} {h} {:#x} 1", dst.0, r.next());
        }
        writeln!(s, "end");
    }
    // (D3DX's YUY2 encoder by probes, 16 x 1 A8R8G8B8 pictures given byte by byte: pairs of one
    // grey, one red, green or blue level; pairs of a mid grey and a level, and the other way round;
    // and two rows)
    let bgra = |px: &[(u8, u8, u8)]| px.iter().map(|&(r, g, b)| format!("{b:02x}{g:02x}{r:02x}ff")).collect::<String>();
    let level = |k: usize, c: usize| -> (u8, u8, u8) {
        let v = u8::try_from(k * 255 / 31).unwrap_or(255);
        match c {
            0 => (v, v, v),
            1 => (v, 0, 0),
            2 => (0, v, 0),
            _ => (0, 0, v),
        }
    };
    writeln!(s, "case load-yuy2-probe\ndevice swvp");
    for c in 0..4 {
        for line in 0..4 {
            let px: Vec<_> = (0..16).map(|x| level(line * 8 + x / 2, c)).collect();
            writeln!(s, "x9load 844715353 21 16 1 b:{} 1", bgra(&px));
        }
    }
    let mid = (128, 128, 128);
    for c in 1..4 {
        for first in [true, false] {
            for line in 0..4 {
                let px: Vec<_> = (0..16).map(|x| if (x % 2 == 0) == first { level(line * 8 + x / 2, c) } else { mid }).collect();
                writeln!(s, "x9load 844715353 21 16 1 b:{} 1", bgra(&px));
            }
        }
    }
    let two: Vec<_> = (0..32).map(|x| if x < 16 { level(x, 1) } else { level(x - 16, 3) }).collect();
    writeln!(s, "x9load 844715353 21 16 2 b:{} 1\nend", bgra(&two));
    // (into DXT and UYVY through a system memory texture, which takes them, and A8R8G8B8 alike)
    for dst in TEXTURE_FORMATS.iter().filter(|f| matches!(f.1, "DXT1" | "DXT2" | "DXT3" | "DXT4" | "DXT5" | "UYVY" | "A8R8G8B8")) {
        writeln!(s, "case loadtex-into-{}\ndevice swvp\nx9loadtex {} 21 16 4 ramp 1", dst.1, dst.0);
        for (src, w, h) in [(21, 8, 8), (21, 4, 4), (21, 8, 4), (116, 8, 8)] {
            writeln!(s, "x9loadtex {} {src} {w} {h} {:#x} 1", dst.0, r.next());
        }
        // (one colour a block, two colours a block, a gradient)
        let solid: Vec<_> = (0..16).map(|_| (200u8, 100u8, 50u8)).collect();
        let pair: Vec<_> = (0..16).map(|i| if i % 2 == 0 { (255u8, 0u8, 0u8) } else { (0, 0, 255) }).collect();
        let grad: Vec<_> = (0..16).map(|i| level(i * 2, 0)).collect();
        for px in [solid, pair, grad] {
            writeln!(s, "x9loadtex {} 21 4 4 b:{} 1", dst.0, bgra(&px));
        }
        writeln!(s, "end");
    }
    // (D3DX9.LOAD.FORMAT) into YUY2 from half and signed sources: A16B16G16R16F, R16F, V8U8,
    // Q8W8V8U8, V16U16
    for src in [113u32, 111, 60, 63, 64] {
        s.push_str(&format!("case load-yuy2-from-{src}\ndevice swvp\n"));
        for (w, h) in [(8, 2), (5, 1)] {
            s.push_str(&format!("x9load 844715353 {src} {w} {h} {:#x} 1\n", r.next()));
        }
        s.push_str(&format!("x9load 844715353 {src} 8 1 ramp 1\nend\n"));
    }
    // (D3DX9.LOAD.FORMAT on x86, D3DX9.LOAD.NEGFAULT on x64) a block whose OptimizeRGB index
    // falls below 0 (found by search on the model), into DXT1 and DXT3: last, x64 dies on it
    let neg = "a328ebff3232c4ffc0271afff632a2ff8e2ac4ffc32e6eff8e27fffffc2991ffe034a2ffde2a00ffeb2d68ffcf2c6dff6a154bff00e5ffffae27a8fff663c6ff";
    s.push_str(&format!("case loadtex-dxt-negindex\ndevice swvp\nx9loadtex 827611204 21 4 4 b:{neg} 1\nx9loadtex 861165636 21 4 4 b:{neg} 1\nend\n"));
    s
}
