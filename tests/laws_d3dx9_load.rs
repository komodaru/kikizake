//! D3DXLoadSurfaceFromMemory's halves (spec/d3dx9.txt D3DX9.LOAD.F16HALVES, LOAD.F16RANGE)
//! over their whole domains (kind exhaustive): every half read, and the routine of every place
//! of rows 1..16 pixels wide into each half format, on x86 and on x64 with each vendor class.
use kikizake::model::d3dx9::{f16_one, f16_sse, f16_to_f32, f16_x64, f16_x64_tail};
use kikizake::{case, model};

/// the rows the model writes for these x9load lines
fn load(lines: &str) -> Vec<String> {
    let out = model::run_case(&case::parse_cases(&format!("case c\ndevice swvp\n{lines}\nend\n"))[0]).unwrap();
    out.into_iter().filter(|l| !l.starts_with("px ") && !l.starts_with("device")).collect()
}

/// D3DX9.LOAD.F16HALVES: every half (R16F, all 2^16) read into A32B32G32R32F is its value,
/// exponent 31 a number like the others (D3DX9.F32)
#[test]
fn every_half_reads_as_its_value() {
    model::set_arch(32);
    for block in 0..16u32 {
        let hs: Vec<u16> = (block * 4096..(block + 1) * 4096).map(|h| u16::try_from(h).unwrap()).collect();
        let hex: String = hs.iter().map(|h| format!("{:02x}{:02x}", h & 255, h >> 8)).collect();
        let rows = load(&format!("x9load 116 111 4096 1 b:{hex} 1"));
        let row = rows[0].as_bytes();
        for (x, &h) in hs.iter().enumerate() {
            let r = std::str::from_utf8(&row[32 * x..32 * x + 8]).unwrap();
            let bits = u32::from_str_radix(r, 16).unwrap().swap_bytes();
            assert_eq!(bits, f16_to_f32(h).to_bits(), "{h:#06x}");
        }
    }
}

/// D3DX9.LOAD.F16RANGE, D3DX9.LOAD.F16HALVES: into A16B16G16R16F a row goes through one
/// D3DXFloat32To16Array call (its places in blocks of 8, of 4 on x64 off GenuineIntel, then
/// the rest), into R16F and G16R16F a pixel at a time (every value as an array's rest): every
/// row 1..16 pixels wide of NaNs, whose half tells the routine (7c00, 7fff, 7e00, ffff)
#[test]
fn each_place_takes_its_routine() {
    let nan = 0x7fc0_0000u32;
    let runs: [(u32, &str); 3] = [(32, "AuthenticAMD"), (64, "GenuineIntel"), (64, "AuthenticAMD")];
    for (arch, vendor) in runs {
        model::set_arch(arch);
        model::set_cpu(Some(format!("cpu {vendor} fefa3203 1f8bfbff f1bf2fb9")));
        let amd = arch == 64 && vendor != "GenuineIntel";
        let (block, rest) = if amd { (f16_x64(nan), f16_x64_tail(nan)) } else { (f16_sse(nan), f16_one(nan)) };
        for (fmt, n) in [(111u32, 1usize), (112, 2), (113, 4)] {
            for w in 1..=16usize {
                let src = "0000c07f".repeat(4 * w);
                let rows = load(&format!("x9load {fmt} 116 {w} 1 b:{src} 1"));
                // (the row's array of 4 w, or each pixel's own of n)
                let (len, step) = (if n == 4 { 4 * w } else { n }, if amd { 4 } else { 8 });
                let bulk = len - len % step;
                let want: String = (0..w)
                    .flat_map(|x| (0..n).map(move |k| if n == 4 { n * x + k } else { k }))
                    .map(|at| if at < bulk { block } else { rest })
                    .map(|h| format!("{:02x}{:02x}", h & 255, h >> 8))
                    .collect();
                assert_eq!(rows, [want], "{arch} {vendor} {fmt} {w}");
            }
        }
    }
    model::set_arch(32);
    model::set_cpu(None);
}

/// D3DX9.LOAD: the surface D3DXLoadSurfaceFromMemory is given, for every format code 0..255 and
/// every FOURCC format: a system memory surface (CreateOffscreenPlainSurface) of a format
/// CreateTexture does not take (D3D9.FORMATS: R8G8B8, R3G3B2, A8R3G3B2, X8B8G8R8 among them)
/// or a DXT one, but YUY2, is D3DERR_INVALIDCALL; a texture of a format CreateTexture does
/// not take is too; any other is loaded
#[test]
fn surfaces_of_formats_not_taken_are_invalid() {
    use kikizake::model::d3d9draw::{format, fourcc, supported, Kind, TEXTURE_FORMATS};
    model::set_arch(32);
    let codes: std::collections::BTreeSet<u32> = (0..=255).chain(TEXTURE_FORMATS.iter().map(|t| t.0)).chain([fourcc(b"UYVY"), fourcc(b"YUY2")]).collect();
    for &df in &codes {
        let dxt = format(df).is_some_and(|m| matches!(m.kind, Kind::Dxt(_)));
        let plain_bad = df != fourcc(b"YUY2") && (dxt || !supported(df, false, false));
        for (op, bad, what) in [("x9load", plain_bad, "surface"), ("x9loadtex", !supported(df, false, false), "texture")] {
            let out = model::run_case(&case::parse_cases(&format!("case c\ndevice swvp\n{op} {df} 21 4 4 ramp 1\nend\n"))[0]);
            let refused = out.as_ref().is_ok_and(|o| o.iter().any(|l| l == &format!("load {what} hr 8876086c")));
            assert_eq!(refused, bad, "{op} {df} {out:?}");
        }
    }
}
