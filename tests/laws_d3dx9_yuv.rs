//! D3DXLoadSurfaceFromMemory's YUV conversions (spec/d3dx9.txt D3DX9.LOAD.YUY2, LOAD.YUV)
//! over their whole domains (kind exhaustive): every R, G, B of an 8-bit source into YUY2,
//! every four chroma bytes an odd pixel is made of, every Y, U, V read back.
use kikizake::model::d3dx9::{chroma_tap, yuv_rgb, yuy2_bytes};

mod every;
use every::all_u32;

/// an 8-bit channel k as D3DX reads it (k times the f32 nearest 1/255, D3DX9.LOAD.CONVERT) and
/// takes it into YUY2 (times 255 in f64): the value and the value times 2^31 exactly
fn channel(k: u8) -> (f64, i128) {
    let p = f32::from(k) * f32::from_bits(0x3b80_8081);
    let b = p.to_bits();
    let (ex, m) = (i32::try_from(b >> 23).unwrap(), i128::from(b & 0x7f_ffff | 0x80_0000));
    let n = if k == 0 { 0 } else { (m << (ex - 119)) * 255 };
    (f64::from(p) * 255.0, n)
}

/// base + (k0 c0 + k1 c1 + k2 c2) / 255000 + 1/2 truncated toward 0, exactly, c in 2^-31 steps,
/// clamped to a byte
fn offset(base: i128, k: [i128; 3], c: [i128; 3]) -> u8 {
    let num = k[0] * c[0] + k[1] * c[1] + k[2] * c[2];
    let den = 255_000i128 << 31;
    // (x + 1/2 = (2 num + den) / (2 den), and i128 division truncates toward 0)
    let q = (2 * num + den) / (2 * den);
    u8::try_from((base + q).clamp(0, 255)).unwrap()
}

/// D3DX9.LOAD.YUY2: every R, G, B of an 8-bit source (2^24): each Y
/// 16 + (65.481 R + 128.553 G + 24.966 B) / 255, U 128 + (-37.797 R - 74.203 G + 112 B) / 255,
/// V 128 + (112 R - 93.786 G - 18.214 B) / 255, each offset plus 1/2 truncated toward 0 (the
/// constants and sums here exact; the model's f64 agrees for every one), the second pixel's Y
/// its own (here the first's channels turned)
#[test]
fn yuy2_is_the_formula_for_every_colour() {
    assert_eq!(all_u32(|w| {
        if w >> 24 != 0 {
            return true;
        }
        let [r, g, b, _] = w.to_le_bytes();
        let (c0, c1) = ([channel(r), channel(g), channel(b)], [channel(g), channel(b), channel(r)]);
        let f = |c: [(f64, i128); 3]| [c[0].0, c[1].0, c[2].0];
        let n = |c: [(f64, i128); 3]| [c[0].1, c[1].1, c[2].1];
        let got = yuy2_bytes(f(c0), f(c1));
        let y = |c| offset(16, [65_481, 128_553, 24_966], n(c));
        let want = [y(c0), offset(128, [-37_797, -74_203, 112_000], n(c0)), y(c1), offset(128, [112_000, -93_786, -18_214], n(c0))];
        got == want
    }), None);
}

/// D3DX9.LOAD.YUV: every four bytes a, b, c, d of the pairs around an odd pixel (2^32):
/// (-a + 9 b + 9 c - d + 8) >> 4 in unsigned 32-bit arithmetic, a negative sum wrapped
#[test]
fn the_chroma_tap_for_every_four_bytes() {
    assert_eq!(all_u32(|w| {
        let [a, b, c, d] = w.to_le_bytes();
        let s = -i64::from(a) + 9 * i64::from(b) + 9 * i64::from(c) - i64::from(d) + 8;
        chroma_tap(a, b, c, d) == s.rem_euclid(1 << 32) >> 4
    }), None);
}

/// D3DX9.LOAD.YUV: every Y and every U and V a tap or a pair gives (0..287, and the wrapped
/// 2^28 - 32 .. 2^28): in f32 c = (Y - 16) 0.00456621, d = U - 128, e = V - 128,
/// R = c + e 0.00625893, G = (c - d 0.00153632) - e 0.00318811, B = c + d 0.00791071, each
/// clamped to 0..1, A 1
#[test]
fn yuv_is_the_formula_for_every_sample() {
    let uv: Vec<i64> = (0..=287).chain((1 << 28) - 32..1 << 28).collect();
    for y in 0..=255u8 {
        let c = f32::from(i16::from(y) - 16) * 0.004_566_21;
        for &u in &uv {
            for &v in &uv {
                let (d, e) = ((u - 128) as f32, (v - 128) as f32);
                let want = [(c + e * 0.006_258_93).clamp(0.0, 1.0), ((c - d * 0.001_536_32) - e * 0.003_188_11).clamp(0.0, 1.0), (c + d * 0.007_910_71).clamp(0.0, 1.0), 1.0];
                assert_eq!(yuv_rgb(y, u, v).map(f32::to_bits), want.map(f32::to_bits), "{y} {u} {v}");
            }
        }
    }
}
