//! WIC's JPEG encoder and the pixels its decoder gives back: IJG libjpeg 6b's baseline path.
//!
//! MEASURED (golden wicencode enc-jpeg-*: every byte of the files and every decoded pixel as
//! this): the JFIF APP0 (version 1.1, units 1, density 0), IJG's tables at quality 90
//! (`jpeg_set_quality`), 4:2:0 for color, the standard Huffman tables, the slow integer DCT
//! both ways (jfdctint, jidctint), rgb_ycc's fixed-point tables, h2v2 downsampling with
//! alternating bias, dummy blocks past the picture (zero AC, the previous block's DC), h2v2
//! fancy upsampling, ycc_rgb's tables; CMYK as Adobe YCCK (APP14 transform 2, no APP0) of the inverted CMYK.

/// the zigzag order: the natural index of each coefficient in the order written
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39,
    46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// the IJG (JPEG standard Annex K) base tables, natural order
const LUMINANCE: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49,
    64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];
const CHROMINANCE: [u32; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

const DC_LUMA_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_CHROMA_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_VALUES: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
const AC_LUMA_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7d];
const AC_LUMA_VALUES: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xa1, 0x08, 0x23, 0x42, 0xb1, 0xc1, 0x15, 0x52, 0xd1, 0xf0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0a, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3, 0xc4, 0xc5,
    0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe1, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const AC_CHROMA_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROMA_VALUES: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xa1, 0xb1, 0xc1, 0x09, 0x23, 0x33, 0x52, 0xf0,
    0x15, 0x62, 0x72, 0xd1, 0x0a, 0x16, 0x24, 0x34, 0xe1, 0x25, 0xf1, 0x17, 0x18, 0x19, 0x1a, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4a, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5a, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7a, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8a, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7, 0xa8, 0xa9, 0xaa, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xb8, 0xb9, 0xba, 0xc2, 0xc3,
    0xc4, 0xc5, 0xc6, 0xc7, 0xc8, 0xc9, 0xca, 0xd2, 0xd3, 0xd4, 0xd5, 0xd6, 0xd7, 0xd8, 0xd9, 0xda, 0xe2, 0xe3, 0xe4, 0xe5, 0xe6, 0xe7, 0xe8, 0xe9, 0xea, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8,
    0xf9, 0xfa,
];
const _: () = assert!(DC_VALUES.len() == 12 && AC_LUMA_VALUES.len() == 162 && AC_CHROMA_VALUES.len() == 162);

/// a JPEG quality, 1..=100 (`jpeg_set_quality`'s range)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Quality(u32);

impl Quality {
    /// IJG clamps the quality into 1..=100
    pub fn new(q: i64) -> Quality {
        Quality(q.clamp(1, 100) as u32)
    }
    /// `jpeg_quality_scaling`, then each entry (b * s + 50) / 100 kept within 1..=255
    fn table(self, base: &[u32; 64]) -> [u32; 64] {
        let q = self.0;
        let s = if q < 50 { 5000 / q } else { 200 - q * 2 };
        base.map(|b| ((b * s + 50) / 100).clamp(1, 255))
    }
}

/// which planes a file holds
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Gray,
    /// YCbCr from BGR, chroma halved both ways
    Color,
    /// Adobe YCCK from CMYK
    Cmyk,
}

/// a Huffman table as codes by symbol
struct Huffman {
    code: [(u16, u8); 256],
}

impl Huffman {
    fn new(bits: &[u8; 16], values: &[u8]) -> Huffman {
        let mut code = [(0u16, 0u8); 256];
        let (mut c, mut k) = (0u16, 0usize);
        for (l, &n) in bits.iter().enumerate() {
            for _ in 0..n {
                code[usize::from(values[k])] = (c, l as u8 + 1);
                c += 1;
                k += 1;
            }
            c <<= 1;
        }
        Huffman { code }
    }
}

/// the entropy coder's bit writer: MSB first, 0xff stuffed, flushed with 1 bits
struct Bits {
    out: Vec<u8>,
    acc: u32,
    n: u32,
}

impl Bits {
    fn put(&mut self, code: u32, size: u32) {
        self.acc = (self.acc << size) | (code & ((1 << size) - 1));
        self.n += size;
        while self.n >= 8 {
            let b = (self.acc >> (self.n - 8)) as u8;
            self.out.push(b);
            if b == 0xff {
                self.out.push(0);
            }
            self.n -= 8;
        }
        self.acc &= (1 << self.n) - 1;
    }
    fn flush(&mut self) {
        let pad = (8 - self.n % 8) % 8;
        self.put((1 << pad) - 1, pad);
    }
}

fn descale(x: i64, n: u32) -> i64 {
    (x + (1 << (n - 1))) >> n
}

/// jfdctint's slow integer forward DCT (CONST_BITS 13, PASS1_BITS 2), in place
fn fdct(d: &mut [i64; 64]) {
    for pass in 0..2 {
        for r in 0..8 {
            let idx = |c: usize| if pass == 0 { r * 8 + c } else { c * 8 + r };
            let x: [i64; 8] = std::array::from_fn(|c| d[idx(c)]);
            let (t0, t7, t1, t6) = (x[0] + x[7], x[0] - x[7], x[1] + x[6], x[1] - x[6]);
            let (t2, t5, t3, t4) = (x[2] + x[5], x[2] - x[5], x[3] + x[4], x[3] - x[4]);
            let (t10, t13, t11, t12) = (t0 + t3, t0 - t3, t1 + t2, t1 - t2);
            let mut o = [0i64; 8];
            let sh = if pass == 0 { 11 } else { 15 };
            if pass == 0 {
                o[0] = (t10 + t11) << 2;
                o[4] = (t10 - t11) << 2;
            } else {
                o[0] = descale(t10 + t11, 2);
                o[4] = descale(t10 - t11, 2);
            }
            let z1 = (t12 + t13) * 4433;
            o[2] = descale(z1 + t13 * 6270, sh);
            o[6] = descale(z1 - t12 * 15137, sh);
            let (z1, z2, z3, z4) = (t4 + t7, t5 + t6, t4 + t6, t5 + t7);
            let z5 = (z3 + z4) * 9633;
            let (t4, t5, t6, t7) = (t4 * 2446, t5 * 16819, t6 * 25172, t7 * 12299);
            let (z1, z2) = (z1 * -7373, z2 * -20995);
            let (z3, z4) = (z3 * -16069 + z5, z4 * -3196 + z5);
            o[7] = descale(t4 + z1 + z3, sh);
            o[5] = descale(t5 + z2 + z4, sh);
            o[3] = descale(t6 + z2 + z3, sh);
            o[1] = descale(t7 + z1 + z4, sh);
            for (c, v) in o.into_iter().enumerate() {
                d[idx(c)] = v;
            }
        }
    }
}

/// jcdctmgr's quantization: the table times 8, rounded half away from zero
fn quantize(c: &[i64; 64], q: &[u32; 64]) -> [i64; 64] {
    std::array::from_fn(|i| {
        let d = i64::from(q[i]) << 3;
        let t = c[i].abs() + (d >> 1);
        let v = if t >= d { t / d } else { 0 };
        if c[i] < 0 { -v } else { v }
    })
}

/// one pass of jidctint's 1-D inverse DCT over 8 values
fn idct_1d(x: [i64; 8], sh: u32) -> [i64; 8] {
    let z1 = (x[2] + x[6]) * 4433;
    let (t2, t3) = (z1 - x[6] * 15137, z1 + x[2] * 6270);
    let (t0, t1) = ((x[0] + x[4]) << 13, (x[0] - x[4]) << 13);
    let (t10, t13, t11, t12) = (t0 + t3, t0 - t3, t1 + t2, t1 - t2);
    let (a0, a1, a2, a3) = (x[7], x[5], x[3], x[1]);
    let (z1, z2, z3, z4) = (a0 + a3, a1 + a2, a0 + a2, a1 + a3);
    let z5 = (z3 + z4) * 9633;
    let (z1, z2) = (z1 * -7373, z2 * -20995);
    let (z3, z4) = (z3 * -16069 + z5, z4 * -3196 + z5);
    let a0 = a0 * 2446 + z1 + z3;
    let a1 = a1 * 16819 + z2 + z4;
    let a2 = a2 * 25172 + z2 + z3;
    let a3 = a3 * 12299 + z1 + z4;
    [t10 + a3, t11 + a2, t12 + a1, t13 + a0, t13 - a0, t12 - a1, t11 - a2, t10 - a3].map(|v| descale(v, sh))
}

/// jidctint's slow integer inverse DCT of dequantized coefficients, to samples 0..=255
fn idct(c: &[i64; 64], q: &[u32; 64]) -> [u8; 64] {
    let mut ws = [0i64; 64];
    for col in 0..8 {
        let x: [i64; 8] = std::array::from_fn(|r| c[r * 8 + col] * i64::from(q[r * 8 + col]));
        // (the column with no AC is its DC times 4, as jidctint shortcuts it)
        let o = if x[1..].iter().all(|&v| v == 0) { [x[0] << 2; 8] } else { idct_1d(x, 11) };
        for r in 0..8 {
            ws[r * 8 + col] = o[r];
        }
    }
    let mut out = [0u8; 64];
    for r in 0..8 {
        let o = idct_1d(std::array::from_fn(|k| ws[r * 8 + k]), 18);
        for k in 0..8 {
            out[r * 8 + k] = (o[k] + 128).clamp(0, 255) as u8;
        }
    }
    out
}

/// a plane of samples: `w` x `h` of the picture, `px` of `sw` columns (wider when the
/// downsampler filled the last blocks itself), its rows past `h` the last one again
#[derive(Clone)]
struct Plane {
    w: usize,
    h: usize,
    sw: usize,
    px: Vec<i64>,
}

impl Plane {
    fn new(w: usize, h: usize, px: Vec<i64>) -> Plane {
        Plane { w, h, sw: w, px }
    }
    fn at(&self, x: usize, y: usize) -> i64 {
        self.px[y.min(self.h - 1) * self.sw + x.min(self.sw - 1)]
    }
}

/// h2v2 downsampling: each 2x2 averaged with a bias of 1, 2, 1, 2 along the row, the source's
/// edges replicated; across the whole of the last blocks (their columns past the picture are
/// averaged too, bias and all), its rows past the picture the last one again
fn downsample(p: &Plane) -> Plane {
    let (w, h) = (p.w.div_ceil(2), p.h.div_ceil(2));
    let sw = w.div_ceil(8) * 8;
    let mut px = vec![0; sw * h];
    for y in 0..h {
        for x in 0..sw {
            let s = p.at(2 * x, 2 * y) + p.at(2 * x + 1, 2 * y) + p.at(2 * x, 2 * y + 1) + p.at(2 * x + 1, 2 * y + 1);
            px[y * sw + x] = (s + 1 + (x as i64 & 1)) >> 2;
        }
    }
    Plane { w, h, sw, px }
}

/// h2v2 fancy upsampling (a triangle filter, 3/4 and 1/4 each way) to w x h
fn upsample(p: &Plane, w: usize, h: usize) -> Plane {
    let mut px = vec![0; w * h];
    for oy in 0..h {
        let y = oy / 2;
        let far = if oy % 2 == 0 { y.saturating_sub(1) } else { (y + 1).min(p.h - 1) };
        let col: Vec<i64> = (0..p.w).map(|x| 3 * p.at(x, y) + p.at(x, far)).collect();
        let n = col.len();
        for ox in 0..w {
            let x = ox / 2;
            px[oy * w + ox] = if ox % 2 == 0 {
                if x == 0 { (col[0] * 4 + 8) >> 4 } else { (col[x] * 3 + col[x - 1] + 8) >> 4 }
            } else if x + 1 >= n {
                (col[x] * 4 + 7) >> 4
            } else {
                (col[x] * 3 + col[x + 1] + 7) >> 4
            };
        }
    }
    Plane::new(w, h, px)
}

const fn fix(x: f64) -> i64 {
    (x * 65536.0 + 0.5) as i64
}

/// rgb_ycc_convert's fixed-point tables
fn rgb_ycc(r: i64, g: i64, b: i64) -> [i64; 3] {
    let half = 1 << 15;
    let cb = 128 << 16;
    [
        (fix(0.299) * r + fix(0.587) * g + fix(0.114) * b + half) >> 16,
        (-fix(0.16874) * r - fix(0.33126) * g + fix(0.5) * b + cb + half - 1) >> 16,
        (fix(0.5) * r - fix(0.41869) * g - fix(0.08131) * b + cb + half - 1) >> 16,
    ]
}

/// ycc_rgb_convert's tables
fn ycc_rgb(y: i64, cb: i64, cr: i64) -> [u8; 3] {
    let half = 1 << 15;
    let (cb, cr) = (cb - 128, cr - 128);
    let c = |v: i64| v.clamp(0, 255) as u8;
    [c(y + ((fix(1.402) * cr + half) >> 16)), c(y + ((-fix(0.34414) * cb - fix(0.71414) * cr + half) >> 16)), c(y + ((fix(1.772) * cb + half) >> 16))]
}

/// what the encoder writes and what decoding it gives
pub struct Jpeg {
    pub bytes: Vec<u8>,
    /// the decoded samples: 8bppGray, 24bppBGR or 32bppCMYK rows, packed
    pub px: Vec<u8>,
}

/// encodes `px` (8bppGray, 24bppBGR or 32bppCMYK rows, packed) of w x h
pub fn encode(kind: Kind, w: usize, h: usize, px: &[u8], q: Quality, app0: bool, full_chroma: bool) -> Jpeg {
    let plane = |f: &dyn Fn(usize) -> i64| Plane::new(w, h, (0..w * h).map(f).collect());
    let planes: Vec<Plane> = match kind {
        Kind::Gray => vec![plane(&|i| i64::from(px[i]))],
        Kind::Color | Kind::Cmyk => {
            let n = if kind == Kind::Color { 3 } else { 4 };
            // (MEASURED: golden enc-jpeg-cmyk: WIC hands IJG the CMYK inverted, as Adobe writes it,
            // so YCCK is the YCbCr of C, M, Y themselves and K inverted)
            let rgb = |i: usize| -> [i64; 3] {
                let p = &px[i * n..i * n + n];
                if kind == Kind::Color { [p[2], p[1], p[0]].map(i64::from) } else { [p[0], p[1], p[2]].map(i64::from) }
            };
            let ycc = |k: usize| plane(&|i| { let [r, g, b] = rgb(i); rgb_ycc(r, g, b)[k] });
            let down = |p: Plane| if full_chroma { p } else { downsample(&p) };
            let mut v = vec![ycc(0), down(ycc(1)), down(ycc(2))];
            if kind == Kind::Cmyk {
                v.push(plane(&|i| 255 - i64::from(px[i * 4 + 3])));
            }
            v
        }
    };
    let ql = q.table(&LUMINANCE);
    let qc = q.table(&CHROMINANCE);
    let nc = planes.len();
    let full = |c: usize| c == 0 || c == 3;
    let samp = if nc == 1 || full_chroma { 1 } else { 2 };
    let mut out = vec![0xff, 0xd8];
    let seg = |out: &mut Vec<u8>, m: u8, data: &[u8]| {
        out.extend_from_slice(&[0xff, m]);
        out.extend_from_slice(&(data.len() as u16 + 2).to_be_bytes());
        out.extend_from_slice(data);
    };
    if kind == Kind::Cmyk {
        seg(&mut out, 0xee, b"Adobe\x00\x64\x00\x00\x00\x00\x02");
    } else if app0 {
        seg(&mut out, 0xe0, b"JFIF\x00\x01\x01\x01\x00\x00\x00\x00\x00\x00");
    }
    let zz = |t: &[u32; 64]| -> Vec<u8> { ZIGZAG.iter().map(|&z| t[z] as u8).collect() };
    seg(&mut out, 0xdb, &[&[0u8][..], &zz(&ql)].concat());
    if nc > 1 {
        seg(&mut out, 0xdb, &[&[1u8][..], &zz(&qc)].concat());
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(h as u16).to_be_bytes());
    sof.extend_from_slice(&(w as u16).to_be_bytes());
    sof.push(nc as u8);
    for c in 0..nc {
        let f = if full(c) { samp } else { 1 };
        sof.extend_from_slice(&[c as u8 + 1, (f << 4 | f) as u8, u8::from(!full(c))]);
    }
    seg(&mut out, 0xc0, &sof);
    let dht = |class: u8, bits: &[u8; 16], vals: &[u8]| [&[class][..], bits, vals].concat();
    seg(&mut out, 0xc4, &dht(0x00, &DC_LUMA_BITS, &DC_VALUES));
    seg(&mut out, 0xc4, &dht(0x10, &AC_LUMA_BITS, &AC_LUMA_VALUES));
    if nc > 1 {
        seg(&mut out, 0xc4, &dht(0x01, &DC_CHROMA_BITS, &DC_VALUES));
        seg(&mut out, 0xc4, &dht(0x11, &AC_CHROMA_BITS, &AC_CHROMA_VALUES));
    }
    let mut sos = vec![nc as u8];
    for c in 0..nc {
        sos.extend_from_slice(&[c as u8 + 1, if full(c) { 0 } else { 0x11 }]);
    }
    sos.extend_from_slice(&[0, 63, 0]);
    seg(&mut out, 0xda, &sos);
    let tables = [
        (Huffman::new(&DC_LUMA_BITS, &DC_VALUES), Huffman::new(&AC_LUMA_BITS, &AC_LUMA_VALUES), ql),
        (Huffman::new(&DC_CHROMA_BITS, &DC_VALUES), Huffman::new(&AC_CHROMA_BITS, &AC_CHROMA_VALUES), qc),
    ];
    let mcu = 8 * samp;
    let (mx, my) = (w.div_ceil(mcu), h.div_ceil(mcu));
    // (each plane's blocks, kept to decode them again)
    let mut coefs: Vec<Vec<Option<[i64; 64]>>> = planes.iter().map(|p| vec![None; p.w.div_ceil(8) * p.h.div_ceil(8)]).collect();
    let mut bits = Bits { out: vec![], acc: 0, n: 0 };
    let mut prev = vec![0i64; nc];
    for by in 0..my {
        for bx in 0..mx {
            for (c, p) in planes.iter().enumerate() {
                let f = if full(c) { samp } else { 1 };
                let (dc, ac, qt) = &tables[usize::from(!full(c))];
                let (wb, hb) = (p.w.div_ceil(8), p.h.div_ceil(8));
                let mut last = 0;
                for yy in 0..f {
                    for xx in 0..f {
                        let (x0, y0) = (bx * f + xx, by * f + yy);
                        let blk = if x0 < wb && y0 < hb {
                            let mut d: [i64; 64] = std::array::from_fn(|i| p.at(x0 * 8 + i % 8, y0 * 8 + i / 8) - 128);
                            fdct(&mut d);
                            let b = quantize(&d, qt);
                            coefs[c][y0 * wb + x0] = Some(b);
                            b
                        } else {
                            // (a dummy block: no AC, the DC of the block before)
                            let mut b = [0i64; 64];
                            b[0] = last;
                            b
                        };
                        let diff = blk[0] - prev[c];
                        prev[c] = blk[0];
                        last = blk[0];
                        put_value(&mut bits, &dc.code, 0, diff);
                        let mut run = 0u32;
                        for &z in &ZIGZAG[1..] {
                            let v = blk[z];
                            if v == 0 {
                                run += 1;
                                continue;
                            }
                            while run > 15 {
                                let (code, l) = ac.code[0xf0];
                                bits.put(u32::from(code), u32::from(l));
                                run -= 16;
                            }
                            put_value(&mut bits, &ac.code, run, v);
                            run = 0;
                        }
                        if run > 0 {
                            let (code, l) = ac.code[0];
                            bits.put(u32::from(code), u32::from(l));
                        }
                    }
                }
            }
        }
    }
    bits.flush();
    out.extend_from_slice(&bits.out);
    out.extend_from_slice(&[0xff, 0xd9]);
    // decoding: each plane's blocks back to samples, chroma upsampled, then the colors
    let decoded: Vec<Plane> = planes
        .iter()
        .zip(&coefs)
        .enumerate()
        .map(|(c, (p, bl))| {
            let qt = &tables[usize::from(!full(c))].2;
            let wb = p.w.div_ceil(8);
            let mut px = vec![0i64; p.w * p.h];
            for (k, b) in bl.iter().enumerate() {
                let Some(b) = b else { continue };
                let s = idct(b, qt);
                for i in 0..64 {
                    let (x, y) = ((k % wb) * 8 + i % 8, (k / wb) * 8 + i / 8);
                    if x < p.w && y < p.h {
                        px[y * p.w + x] = i64::from(s[i]);
                    }
                }
            }
            let d = Plane::new(p.w, p.h, px);
            if full(c) || full_chroma { d } else if d.w <= 2 {
                // (IJG jdsample.c: fancy only when the plane is wider than 2; MEASURED opt-16-bgr24)
                Plane::new(w, h, (0..w * h).map(|i| d.at(i % w / 2, i / w / 2)).collect()) } else { upsample(&d, w, h) }
        })
        .collect();
    let mut dec = Vec::with_capacity(w * h * 4);
    for i in 0..w * h {
        match kind {
            Kind::Gray => dec.push(decoded[0].px[i] as u8),
            Kind::Color => {
                let [r, g, b] = ycc_rgb(decoded[0].px[i], decoded[1].px[i], decoded[2].px[i]);
                dec.extend_from_slice(&[b, g, r]);
            }
            Kind::Cmyk => {
                let [r, g, b] = ycc_rgb(decoded[0].px[i], decoded[1].px[i], decoded[2].px[i]);
                dec.extend_from_slice(&[r, g, b, 255 - decoded[3].px[i] as u8]);
            }
        }
    }
    Jpeg { bytes: out, px: dec }
}

/// a coefficient (or DC difference) as its run/size symbol and its bits
fn put_value(bits: &mut Bits, code: &[(u16, u8); 256], run: u32, v: i64) {
    let n = v.unsigned_abs().bit_width();
    let (c, l) = code[((run << 4) | n) as usize];
    bits.put(u32::from(c), u32::from(l));
    if n > 0 {
        let t = if v < 0 { v - 1 } else { v };
        bits.put(t as u32, n);
    }
}
