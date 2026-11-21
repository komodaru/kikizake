//! WIC's JPEG XR (HD Photo, WMP) lossy encoder and the pixels its decoder gives back: WMPhoto.dll
//! as WIC drives it, an executable specification of the codec ITU-T T.832 normates (the decoder,
//! so the bitstream's syntax, the transforms, prediction, adaptive scan and VLC tables) with the
//! encoder's free choices WMPhoto makes.
//!
//! Sources: ITU-T T.832 (JPEG XR image coding system) and Microsoft's public BSD-licensed
//! reference codec (jxrlib), read for the algorithm and written again here; the analyst memo on
//! WMPhoto.dll (clean-room, behaviour only): the ImageQuality to QP mapping and its bit-depth
//! adjustment, the band and channel QPs (an RGB step table looked up one index low), DQUANT for
//! 16-bit RGB, the profile and level block (Main, 128) after the index table, frequency order,
//! 4:4:4, planar alpha coded lossless as its own codestream, and the HD Photo container's IFD.
//! MEASURED (golden wicencode enc-wmp-*: the bytes of every file and the pixels WIC decodes).
//!
//! The pixels are kept as jxrlib keeps them: macroblocks of 256 coefficients, each 4x4 block's
//! 16 in the transform's order (rows 0 2 10 8, columns 0 1 5 4), the blocks of a macroblock 16
//! apart down and 64 across; one column and one row of macroblocks more than the picture so the
//! one-macroblock-late pipelines of the transforms run as the reference runs them.

/// position in a macroblock of pixel (row, column) of its 16 x 16
fn idx_cc(r: usize, c: usize) -> usize {
    const ROW: [usize; 4] = [0, 2, 10, 8];
    const COL: [usize; 4] = [0, 1, 5, 4];
    ((c >> 2) << 6) + ((r >> 2) << 4) + ROW[r & 3] + COL[c & 3]
}

/// the start of each 4x4 block of a macroblock in the coding order
const BLK_OFFSET: [usize; 16] = [0, 64, 16, 80, 128, 192, 144, 208, 32, 96, 48, 112, 160, 224, 176, 240];
/// the highpass coefficients of a block in their natural order (position k of the 16)
const DCT_INDEX: [usize; 16] = [0, 5, 1, 6, 10, 12, 8, 14, 2, 4, 3, 7, 9, 13, 11, 15];
/// the lowpass (the blocks' DCs) of a macroblock in their natural order
const DC_INDEX: [usize; 16] = [0, 128, 64, 208, 32, 240, 48, 224, 16, 192, 80, 144, 112, 176, 96, 160];
const ZIGZAG_LP: [usize; 16] = [0, 1, 4, 5, 2, 8, 6, 9, 3, 12, 10, 7, 13, 11, 14, 15];
const ZIGZAG_H: [usize; 16] = [0, 1, 4, 5, 2, 8, 6, 9, 3, 12, 10, 7, 13, 11, 14, 15];
const ZIGZAG_V: [usize; 16] = [0, 4, 8, 5, 1, 12, 9, 6, 2, 13, 3, 15, 7, 10, 14, 11];

/* ---------------------------------------------------------------------------------------- */
/* bits */

/// bits written high first; `fill` pads the byte with zeros
#[derive(Default, Clone)]
struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    /// the low `n` bits of `v` (up to 32)
    fn put(&mut self, v: u32, n: u32) {
        debug_assert!(n <= 32);
        if n == 0 {
            return;
        }
        let v = if n == 32 { v } else { v & ((1u32 << n) - 1) };
        self.acc = (self.acc << n) | u64::from(v);
        self.n += n;
        while self.n >= 8 {
            self.n -= 8;
            self.out.push((self.acc >> self.n) as u8);
        }
        self.acc &= (1u64 << self.n) - 1;
    }
    fn bit(&mut self, b: bool) {
        self.put(u32::from(b), 1);
    }
    fn fill(&mut self) {
        if self.n % 8 != 0 {
            self.put(0, 8 - self.n % 8);
        }
    }
    /// bytes written so far (after `fill`)
    fn len(&self) -> usize {
        self.out.len()
    }
    /// a variable-length word, byte aligned (VLW): 16 bits, or 0xfb and 32, or 0xfc and 64
    fn vlw(&mut self, s: u64) {
        if s < 0xfb00 {
            self.put(s as u32, 16);
        } else if s >> 32 == 0 {
            self.put(0xfb, 8);
            self.put((s >> 16) as u32 & 0xffff, 16);
            self.put(s as u32 & 0xffff, 16);
        } else {
            self.put(0xfc, 8);
            self.put((s >> 48) as u32, 16);
            self.put((s >> 32) as u32 & 0xffff, 16);
            self.put((s >> 16) as u32 & 0xffff, 16);
            self.put(s as u32 & 0xffff, 16);
        }
    }
}

/* ---------------------------------------------------------------------------------------- */
/* adaptive VLC tables (T.832 9.3) */

/// (code, length) of each symbol, by table index
const CODES4: &[&[(u32, u32)]] = &[&[(1, 1), (1, 2), (0, 3), (1, 3)]];
const CODES5: &[&[(u32, u32)]] = &[&[(1, 1), (1, 2), (1, 3), (0, 4), (1, 4)], &[(1, 1), (0, 3), (1, 3), (2, 3), (3, 3)]];
const CODES6: &[&[(u32, u32)]] = &[
    &[(1, 1), (0, 5), (1, 3), (1, 5), (1, 2), (1, 4)],
    &[(1, 2), (0, 4), (2, 2), (1, 4), (3, 2), (1, 3)],
    &[(0, 4), (1, 4), (1, 2), (2, 2), (3, 2), (1, 3)],
    &[(0, 5), (1, 5), (1, 2), (1, 1), (1, 4), (1, 3)],
];
const CODES7: &[&[(u32, u32)]] = &[&[(1, 2), (2, 2), (3, 2), (1, 3), (1, 4), (0, 5), (1, 5)], &[(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (0, 6), (1, 6)]];
const CODES8: &[&[(u32, u32)]] = &[&[(2, 2), (1, 3), (1, 5), (1, 4), (3, 2), (2, 3), (0, 5), (3, 3)]];
const CODES9: &[&[(u32, u32)]] = &[
    &[(2, 3), (0, 5), (2, 4), (1, 5), (2, 5), (1, 1), (3, 3), (3, 5), (3, 4)],
    &[(1, 1), (1, 3), (2, 3), (1, 4), (1, 6), (3, 3), (1, 5), (0, 7), (1, 7)],
];
const CODES12: &[&[(u32, u32)]] = &[
    &[(1, 5), (1, 6), (0, 7), (1, 7), (4, 5), (2, 3), (5, 5), (1, 1), (6, 5), (1, 4), (7, 5), (3, 3)],
    &[(2, 4), (2, 5), (0, 6), (1, 6), (3, 4), (2, 3), (3, 5), (3, 2), (3, 3), (4, 3), (1, 5), (5, 3)],
    &[(3, 2), (1, 3), (0, 7), (1, 7), (1, 5), (2, 3), (2, 7), (3, 3), (4, 3), (5, 3), (3, 7), (1, 4)],
    &[(1, 3), (3, 2), (0, 7), (1, 5), (2, 5), (2, 3), (1, 7), (3, 3), (3, 5), (4, 3), (1, 6), (5, 3)],
    &[(2, 3), (1, 1), (1, 7), (1, 4), (2, 7), (3, 3), (0, 8), (2, 4), (3, 7), (3, 4), (1, 8), (1, 5)],
];
const DELTA5: [i32; 5] = [0, -1, 0, 1, 1];
const DELTA6: [[i32; 6]; 3] = [[-1, 1, 1, 1, 0, 1], [-2, 0, 0, 2, 0, 0], [-1, -1, 0, 1, -2, 0]];
const DELTA7: [i32; 7] = [1, 0, -1, -1, -1, -1, -1];
const DELTA9: [i32; 9] = [2, 2, 1, 1, -1, -2, -2, -2, -3];
const DELTA12: [[i32; 12]; 4] = [
    [1, 1, 1, 1, 1, 0, 0, -1, 2, 1, 0, 0],
    [2, 2, -1, -1, -1, 0, -2, -1, 0, 0, -2, -1],
    [-1, 1, 0, 2, 0, 0, 0, 0, -2, 0, 1, 1],
    [0, 1, 0, 1, -2, 0, -1, -1, -2, -1, -2, -2],
];

/// the alphabets the VLC tables have
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Alphabet {
    A4,
    A5,
    A6,
    A7,
    A8,
    A9,
    A12,
}

impl Alphabet {
    fn tables(self) -> &'static [&'static [(u32, u32)]] {
        match self {
            Alphabet::A4 => CODES4,
            Alphabet::A5 => CODES5,
            Alphabet::A6 => CODES6,
            Alphabet::A7 => CODES7,
            Alphabet::A8 => CODES8,
            Alphabet::A9 => CODES9,
            Alphabet::A12 => CODES12,
        }
    }
    /// how many tables the discriminant moves between (gMaxTables)
    fn count(self) -> usize {
        match self {
            Alphabet::A4 => 1,
            Alphabet::A5 | Alphabet::A7 | Alphabet::A8 | Alphabet::A9 => 2,
            Alphabet::A6 => 4,
            Alphabet::A12 => 5,
        }
    }
    /// whether a second discriminant raises the table
    fn second(self) -> bool {
        matches!(self, Alphabet::A6 | Alphabet::A12)
    }
}

/// an adaptive VLC table: its table moves by the discriminants (T.832 9.3, AdaptDiscriminant)
#[derive(Clone)]
struct Vlc {
    abc: Alphabet,
    table: usize,
    disc: i32,
    disc1: i32,
    lower: i32,
    upper: i32,
}

const THRESHOLD: i32 = 8;
const MEMORY: i32 = 8;

impl Vlc {
    fn new(abc: Alphabet) -> Vlc {
        let mut v = Vlc { abc, table: 0, disc: 0, disc1: 0, lower: 0, upper: 0 };
        v.reset();
        v
    }
    /// a table set up again (m_bInitialize cleared, then adapted)
    fn reset(&mut self) {
        self.disc = 0;
        self.disc1 = 0;
        self.table = usize::from(self.abc.second());
        self.adapt();
    }
    fn adapt(&mut self) {
        let dl = self.disc;
        let dh = if self.abc.second() { self.disc1 } else { self.disc };
        let mut change = false;
        if dl < self.lower {
            self.table -= 1;
            change = true;
        } else if dh > self.upper {
            self.table += 1;
            change = true;
        }
        if change {
            self.disc = 0;
            self.disc1 = 0;
        }
        self.disc = self.disc.clamp(-THRESHOLD * MEMORY, THRESHOLD * MEMORY);
        self.disc1 = self.disc1.clamp(-THRESHOLD * MEMORY, THRESHOLD * MEMORY);
        let t = self.table;
        self.lower = if t == 0 { i32::MIN } else { -THRESHOLD };
        self.upper = if t == self.abc.count() - 1 { 1 << 30 } else { THRESHOLD };
    }
    fn code(&self, sym: usize) -> (u32, u32) {
        let tables = self.abc.tables();
        // (the 8-symbol alphabet keeps its first table whatever the index)
        tables[if tables.len() == 1 { 0 } else { self.table }][sym]
    }
    fn delta(&self, sym: usize) -> i32 {
        let t = self.table;
        match self.abc {
            Alphabet::A5 => DELTA5[sym],
            Alphabet::A6 => DELTA6[t + usize::from(t == 0) - 1][sym],
            Alphabet::A7 => DELTA7[sym],
            Alphabet::A9 => DELTA9[sym],
            Alphabet::A12 => DELTA12[t + usize::from(t == 0) - 1][sym],
            Alphabet::A4 | Alphabet::A8 => unreachable!("no discriminant for 4 and 8 symbols"),
        }
    }
    fn delta1(&self, sym: usize) -> i32 {
        let t = self.table;
        let last = usize::from(t + 1 == self.abc.count());
        match self.abc {
            Alphabet::A6 => DELTA6[t - last][sym],
            Alphabet::A12 => DELTA12[t - last][sym],
            Alphabet::A4 | Alphabet::A5 | Alphabet::A7 | Alphabet::A8 | Alphabet::A9 => unreachable!("one discriminant"),
        }
    }
    /// writes a symbol, moving the discriminant
    fn put(&mut self, b: &mut Bits, sym: usize) {
        self.disc += self.delta(sym);
        let (c, l) = self.code(sym);
        b.put(c, l);
    }
    /// writes a symbol and a sign after it, moving both discriminants
    fn put_signed(&mut self, b: &mut Bits, sym: usize, neg: bool) {
        self.disc += self.delta(sym);
        self.disc1 += self.delta1(sym);
        let (c, l) = self.code(sym);
        b.put(c * 2 + u32::from(neg), l + 1);
    }
    /// writes a symbol of a table whose discriminant does not move
    fn put_fixed(&self, b: &mut Bits, sym: usize) {
        let (c, l) = self.code(sym);
        b.put(c, l);
    }
}

/// the alphabets of the 21 VLC tables of a context (CTDC 5 for DC and lowpass, then 8 + 8)
const AHEXPT: [Alphabet; 21] = {
    use Alphabet::*;
    [A5, A4, A8, A7, A7, A12, A6, A6, A12, A6, A6, A7, A7, A12, A6, A6, A12, A6, A6, A7, A7]
};
const CTDC: usize = 5;
const CONTEXTX: usize = 8;

#[derive(Clone, Copy, Default)]
struct ScanEntry {
    total: u32,
    pos: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Band {
    Dc,
    Lp,
    Ac,
}

/// the adaptive bit-reduction model of a band (T.832 9.6, UpdateModelMB)
#[derive(Clone, Copy)]
struct Model {
    state: [i32; 2],
    bits: [i32; 2],
    band: Band,
}

impl Model {
    fn new(band: Band) -> Model {
        let b = match band {
            Band::Dc => 8,
            Band::Lp => 4,
            Band::Ac => 0,
        };
        Model { state: [0; 2], bits: [b; 2], band }
    }
    fn update(&mut self, cf: Cf, nch: usize, mut lm: [i32; 2]) {
        const W0: [i32; 3] = [240, 12, 1];
        const W1: [[i32; 16]; 3] = [
            [0, 240, 120, 80, 60, 48, 40, 34, 30, 27, 24, 22, 20, 18, 17, 16],
            [0, 12, 6, 4, 3, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1],
            [0, 16, 8, 5, 4, 3, 3, 2, 2, 2, 2, 1, 1, 1, 1, 1],
        ];
        let b = match self.band {
            Band::Dc => 0,
            Band::Lp => 1,
            Band::Ac => 2,
        };
        lm[0] *= W0[b];
        lm[1] *= W1[b][nch - 1];
        if self.band == Band::Ac {
            lm[1] >>= 4;
        }
        for j in 0..2 {
            let mut ms = self.state[j];
            let mut delta = (lm[j] - 70) >> 2;
            if delta <= -8 {
                delta += 4;
                delta = delta.max(-16);
                ms += delta;
                if ms < -8 {
                    if self.bits[j] == 0 {
                        ms = -8;
                    } else {
                        ms = 0;
                        self.bits[j] -= 1;
                    }
                }
            } else if delta >= 8 {
                delta -= 4;
                delta = delta.min(15);
                ms += delta;
                if ms > 8 {
                    if self.bits[j] >= 15 {
                        self.bits[j] = 15;
                        ms = 8;
                    } else {
                        ms = 0;
                        self.bits[j] += 1;
                    }
                }
            }
            self.state[j] = ms;
            if cf == Cf::YOnly {
                break;
            }
        }
    }
}

/// a tile column's coding context (T.832 9.x)
#[derive(Clone)]
struct Context {
    cbpcy: Vlc,
    cbpcy1: Vlc,
    ah: Vec<Vlc>,
    scan_lp: [ScanEntry; 16],
    scan_h: [ScanEntry; 16],
    scan_v: [ScanEntry; 16],
    model_ac: Model,
    model_lp: Model,
    model_dc: Model,
    cbp_count_zero: i32,
    cbp_count_max: i32,
    cbp_count0: [i32; 2],
    cbp_count1: [i32; 2],
    cbp_state: [i32; 2],
}

impl Context {
    fn new(cf: Cf) -> Context {
        let cbp = if cf == Cf::Yuv444 { Alphabet::A9 } else { Alphabet::A5 };
        let mut c = Context {
            cbpcy: Vlc::new(cbp),
            cbpcy1: Vlc::new(Alphabet::A5),
            ah: AHEXPT.iter().map(|&a| Vlc::new(a)).collect(),
            scan_lp: [ScanEntry::default(); 16],
            scan_h: [ScanEntry::default(); 16],
            scan_v: [ScanEntry::default(); 16],
            model_ac: Model::new(Band::Ac),
            model_lp: Model::new(Band::Lp),
            model_dc: Model::new(Band::Dc),
            cbp_count_zero: 1,
            cbp_count_max: 1,
            cbp_count0: [-4; 2],
            cbp_count1: [4; 2],
            cbp_state: [0; 2],
        };
        c.reset();
        c
    }
    /// ResetCodingContextEnc
    fn reset(&mut self) {
        self.cbpcy.reset();
        self.cbpcy1.reset();
        for a in &mut self.ah {
            a.reset();
        }
        for i in 0..16 {
            self.scan_lp[i].pos = ZIGZAG_LP[i];
            self.scan_h[i].pos = DCT_INDEX[ZIGZAG_H[i]];
            self.scan_v[i].pos = DCT_INDEX[ZIGZAG_V[i]];
        }
        self.model_ac = Model::new(Band::Ac);
        self.model_lp = Model::new(Band::Lp);
        self.model_dc = Model::new(Band::Dc);
        self.cbp_count_zero = 1;
        self.cbp_count_max = 1;
        self.cbp_count0 = [-4; 2];
        self.cbp_count1 = [4; 2];
        self.cbp_state = [0; 2];
    }
    fn adapt_lowpass(&mut self) {
        for a in &mut self.ah[..CONTEXTX + CTDC] {
            a.adapt();
        }
    }
    fn adapt_highpass(&mut self) {
        self.cbpcy.adapt();
        self.cbpcy1.adapt();
        for a in &mut self.ah[CONTEXTX + CTDC..] {
            a.adapt();
        }
    }
}

const MAXTOTAL: u32 = 32767;

fn reset_totals(s: &mut [ScanEntry; 16]) {
    s[0].total = MAXTOTAL;
    for (k, e) in s.iter_mut().enumerate().skip(1) {
        e.total = 32 - 2 * (k as u32 - 1);
    }
}

/// the run-level pairs of a block in the adaptive scan order, the residual bits below the model
/// bits by position (T.832 9.5): (run, level) pairs
fn adaptive_scan(coef: &[i32], resid: &mut [i32; 16], scan: &mut [ScanEntry; 16], model_bits: i32) -> Vec<(i32, i32)> {
    let mut rl = Vec::new();
    let mut run = 0;
    let th_off = (1i32 << model_bits) - 1;
    for k in 1..16 {
        let sk = scan[k].pos;
        let l = coef[sk];
        if l.unsigned_abs() > th_off as u32 {
            let a = l.unsigned_abs() as i32;
            if model_bits > 0 {
                resid[sk] = (a & th_off) * 2;
            }
            scan[k].total += 1;
            if k > 1 && scan[k].total > scan[k - 1].total {
                scan.swap(k, k - 1);
            }
            let v = a >> model_bits;
            rl.push((run, if l < 0 { -v } else { v }));
            run = 0;
        } else {
            run += 1;
            if model_bits > 0 {
                resid[sk] = if l < 0 { 4 * -l + 3 } else if l > 0 { 4 * l + 1 } else { 0 };
            }
        }
    }
    rl
}

/// EncodeSignificantAbsLevel: a level of 1 or more
fn abs_level(b: &mut Bits, vlc: &mut Vlc, level: u32) {
    const INDEX: [usize; 16] = [0, 1, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5];
    const FIXED: [u32; 6] = [0, 0, 1, 2, 2, 2];
    debug_assert!(level > 0);
    let l = level - 1;
    if l >= 16 {
        let mut fixed = 4;
        let mut i = l >> 5;
        while i != 0 {
            fixed += 1;
            i >>= 1;
        }
        vlc.put(b, 6);
        if fixed > 18 {
            b.put(15, 4);
            if fixed > 21 {
                b.put(3, 2);
                b.put(fixed - 22, 3);
            } else {
                b.put(fixed - 19, 2);
            }
        } else {
            b.put(fixed - 4, 4);
        }
        b.put(l, fixed);
    } else {
        let i = INDEX[l as usize];
        vlc.put(b, i);
        b.put(l, FIXED[i]);
    }
}

const SIGNIFICANT_RUN_BIN: [i32; 15] = [-1, -1, -1, -1, 2, 2, 2, 1, 1, 1, 1, 0, 0, 0, 0];
const SIGNIFICANT_RUN_FIXED: [u32; 15] = [0, 0, 1, 1, 3, 0, 0, 1, 1, 2, 0, 0, 0, 0, 1];

/// EncodeSignificantRun
fn significant_run(b: &mut Bits, vlc: &Vlc, run: i32, max_run: i32) {
    const INDEX: [usize; 34] = [0, 1, 2, 2, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 0, 1, 2, 2, 3, 3, 4, 4, 4, 4, 0, 0, 0, 0, 0, 1, 2, 3, 4, 4];
    if max_run < 5 {
        const LEN: [u32; 4] = [3, 3, 2, 1];
        if max_run > 1 {
            b.put(u32::from(max_run != run), LEN[(max_run - run) as usize] - (4 - max_run as u32));
        }
        return;
    }
    let bin = SIGNIFICANT_RUN_BIN[max_run as usize];
    let i = INDEX[(run + bin * 14 - 1) as usize];
    let flc = SIGNIFICANT_RUN_FIXED[i + bin as usize * 5];
    vlc.put_fixed(b, i);
    b.put((run + 1) as u32, flc);
}

/// EncodeBlock: run-level pairs of a lowpass block (`off` CTDC) or highpass one (CTDC + CONTEXTX)
fn encode_block(b: &mut Bits, ah: &mut [Vlc], chroma: bool, rl: &[(i32, i32)], off: usize, mut location: i32) {
    let n = rl.len();
    let (run0, lev) = rl[0];
    let sr = usize::from(run0 == 0);
    let sl = usize::from(lev.unsigned_abs() > 1);
    let mut srn = if n == 1 {
        0
    } else if rl[1].0 > 0 {
        2
    } else {
        1
    };
    ah[off + usize::from(chroma) * 3].put_signed(b, srn * 4 + sl * 2 + sr, lev < 0);
    let mut cont = sr & srn;
    if sl == 1 {
        abs_level(b, &mut ah[6 + off + cont], lev.unsigned_abs() - 1);
    }
    if sr == 0 {
        significant_run(b, &ah[0], run0, 15 - location);
    }
    location += run0 + 1;
    for k in 1..n {
        if srn == 2 {
            significant_run(b, &ah[0], rl[k].0, 15 - location);
        }
        location += rl[k].0 + 1;
        srn = if k == n - 1 {
            0
        } else if rl[k + 1].0 > 0 {
            2
        } else {
            1
        };
        let lev = rl[k].1;
        let sl = usize::from(lev.unsigned_abs() > 1);
        let index = srn * 2 + sl;
        if location < 15 {
            ah[off + cont + 1 + usize::from(chroma) * 3].put_signed(b, index, lev < 0);
        } else if location == 15 {
            const CODE: [u32; 4] = [0, 6, 2, 7];
            const LEN: [u32; 4] = [1, 3, 2, 3];
            b.put(CODE[index] * 2 + u32::from(lev < 0), LEN[index] + 1);
        } else {
            b.put(index as u32 * 2 + u32::from(lev < 0), 2);
        }
        cont &= srn;
        if sl == 1 {
            abs_level(b, &mut ah[6 + off + cont], lev.unsigned_abs() - 1);
        }
    }
}

/* ---------------------------------------------------------------------------------------- */
/* formats and parameters */

/// the internal color format (T.832 Table 9..., INTERNAL_CLR_FMT)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cf {
    YOnly,
    Yuv444,
    Cmyk,
}

impl Cf {
    fn code(self) -> u32 {
        match self {
            Cf::YOnly => 0,
            Cf::Yuv444 => 3,
            Cf::Cmyk => 4,
        }
    }
}

/// the source's color format (OUTPUT_CLR_FMT)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExtCf {
    YOnly,
    Rgb,
    Cmyk,
}

impl ExtCf {
    fn code(self) -> u32 {
        match self {
            ExtCf::YOnly => 0,
            ExtCf::Cmyk => 4,
            ExtCf::Rgb => 7,
        }
    }
}

/// the source's bit depth (OUTPUT_BITDEPTH)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bd {
    B1,
    B8,
    B16,
    B5,
    B565,
}

impl Bd {
    fn code(self) -> u32 {
        match self {
            Bd::B1 => 0,
            Bd::B8 => 1,
            Bd::B16 => 2,
            Bd::B5 => 8,
            Bd::B565 => 10,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Overlap {
    None,
    One,
    Two,
}

impl Overlap {
    fn code(self) -> u32 {
        match self {
            Overlap::None => 0,
            Overlap::One => 1,
            Overlap::Two => 2,
        }
    }
}

/// a quantizer: what an index means (remapQP)
#[derive(Clone, Copy, Debug)]
struct Quant {
    qp: i32,
    offset: i32,
    man: u32,
    exp: i32,
}

/// the reciprocal (mantissa, exponent) of each mantissa 0..32
const RECIP: [(u32, i32); 32] = [
    (0x0, 0), (0x0, 0), (0x0, 1), (0xaaaaaaab, 1), (0x0, 2), (0xcccccccd, 2), (0xaaaaaaab, 2), (0x92492493, 2),
    (0x0, 3), (0xe38e38e4, 3), (0xcccccccd, 3), (0xba2e8ba3, 3), (0xaaaaaaab, 3), (0x9d89d89e, 3), (0x92492493, 3), (0x88888889, 3),
    (0x0, 4), (0xf0f0f0f1, 4), (0xe38e38e4, 4), (0xd79435e6, 4), (0xcccccccd, 4), (0xc30c30c4, 4), (0xba2e8ba3, 4), (0xb21642c9, 4),
    (0xaaaaaaab, 4), (0xa3d70a3e, 4), (0x9d89d89e, 4), (0x97b425ee, 4), (0x92492493, 4), (0x8d3dcb09, 4), (0x88888889, 4), (0x84210843, 4),
];

impl Quant {
    /// remapQP: the step of an index, with `shift` 1 (or 0 for chroma's DC and lowpass)
    fn new(index: u8, shift: i32, scaled: bool) -> Quant {
        if index == 0 {
            return Quant { qp: 1, offset: 0, man: 0, exp: 0 };
        }
        let i = i32::from(index);
        let (man, exp) = if !scaled {
            if i < 32 {
                ((i + 3) >> 2, 0)
            } else if i < 48 {
                ((16 + (i & 15) + 1) >> 1, (i >> 4) - 2)
            } else {
                (16 + (i & 15), (i >> 4) - 3)
            }
        } else if i < 16 {
            (i, shift)
        } else {
            (16 + (i & 15), (i >> 4) - 1 + shift)
        };
        let qp = man << exp;
        let (rm, re) = RECIP[man as usize];
        Quant { qp, offset: (qp * 3 + 1) >> 3, man: rm, exp: re + exp }
    }
    fn dc(index: u8, shift: i32, scaled: bool) -> Quant {
        let q = Quant::new(index, shift, scaled);
        Quant { offset: q.qp >> 1, ..q }
    }
    fn quantize(&self, v: i32) -> i32 {
        let a = v.unsigned_abs() as u64 + self.offset as u64;
        let q = if self.man == 0 { (a >> self.exp) as i32 } else { (((a * u64::from(self.man)) >> 32) >> self.exp) as i32 };
        if v < 0 {
            -q
        } else {
            q
        }
    }
}

/// how a band's QPs are shared by the channels
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ChMode {
    Uniform,
    Mixed,
    Independent,
}

/// a band's QPs: one index a channel
#[derive(Clone, Debug)]
struct QpSet {
    mode: ChMode,
    index: Vec<u8>,
}

impl QpSet {
    fn write(&self, b: &mut Bits) {
        let n = self.index.len();
        let mode = if n > 1 { self.mode } else { ChMode::Uniform };
        if n > 1 {
            b.put(
                match mode {
                    ChMode::Uniform => 0,
                    ChMode::Mixed => 1,
                    ChMode::Independent => 2,
                },
                2,
            );
        }
        b.put(u32::from(self.index[0]), 8);
        match mode {
            ChMode::Uniform => {}
            ChMode::Mixed => b.put(u32::from(self.index[1]), 8),
            ChMode::Independent => {
                for &i in &self.index[1..] {
                    b.put(u32::from(i), 8);
                }
            }
        }
    }
    fn quants(&self, shifted_uv: bool, scaled: bool, dc: bool) -> Vec<Quant> {
        (0..self.index.len())
            .map(|c| {
                let shift = if c > 0 && shifted_uv { 0 } else { 1 };
                if dc {
                    Quant::dc(self.index[c], shift, scaled)
                } else {
                    Quant::new(self.index[c], shift, scaled)
                }
            })
            .collect()
    }
}

/// the QPs of a plane: frame-uniform, or 3 lowpass and 3 highpass sets a tile chosen a
/// macroblock by its luma DC (DQUANT)
#[derive(Clone, Debug)]
enum QpPlan {
    Uniform { dc: QpSet, lp: QpSet, hp: QpSet },
    Dquant { dc: QpSet, lp: [QpSet; 3], hp: [QpSet; 3], lo: i32, hi: i32 },
}

/// what codes one image plane (the image, or the planar alpha)
#[derive(Clone, Debug)]
struct Plane {
    w: usize,
    h: usize,
    ext: ExtCf,
    bd: Bd,
    cf: Cf,
    nch: usize,
    overlap: Overlap,
    scaled: bool,
    qp: QpPlan,
    /// tile starts in macroblocks
    tile_x: Vec<usize>,
    tile_y: Vec<usize>,
}

/* ---------------------------------------------------------------------------------------- */
/* QPs from ImageQuality (memo 2, 3) */

/// an ImageQuality, 0.0 ..= 1.0
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Quality(f32);

impl Quality {
    pub fn new(q: f32) -> Option<Quality> {
        (0.0..=1.0).contains(&q).then_some(Quality(q))
    }
    /// WIC's default
    pub fn default_wic() -> Quality {
        Quality(0.9)
    }
    /// the QP index (uiDefaultQPIndex) for a source bit depth class, float32 as WMPhoto
    fn index(self, bd: Bd) -> u8 {
        let q = self.0;
        if !(0.0..1.0).contains(&q) {
            return 1;
        }
        let t = |o: f32, m: f32| (o - q * m) as i32;
        let base = if q > 0.9 {
            t(142.01, 140.0)
        } else if q > 0.8 {
            t(160.01, 160.0)
        } else if q > 0.4 {
            t(64.01, 40.0)
        } else if q > 0.1 {
            t(69.01, 51.0)
        } else {
            t(112.01, 480.0)
        } as u8;
        if base <= 1 {
            return base;
        }
        match bd {
            Bd::B1 | Bd::B8 | Bd::B5 | Bd::B565 => base,
            Bd::B16 => (208.0f32 - (q * 192.0) * q) as i32 as u8,
        }
    }
    fn overlap(self, bd: Bd) -> Overlap {
        let q = self.0;
        let idx = usize::from(q >= 0.9) + usize::from(q > 0.4);
        if bd == Bd::B16 {
            return Overlap::None;
        }
        [Overlap::Two, Overlap::One, Overlap::None][idx]
    }
}

/// the spec's QP step (T.832, scaled or not), shifted once
fn step(k: i32, scaled: bool) -> i32 {
    if scaled {
        if k < 16 {
            2 * k
        } else {
            (16 + (k & 15)) << (k >> 4)
        }
    } else if k < 32 {
        (k + 3) >> 2
    } else if k < 48 {
        ((17 + (k & 15)) >> 1) << ((k >> 4) - 2)
    } else {
        (16 + (k & 15)) << ((k >> 4) - 3)
    }
}

/// WMPhoto's step table: entry i the step of index i + 1, the last 0
fn step_table(i: i32, scaled: bool) -> i64 {
    if i == 255 {
        0
    } else {
        i64::from(step(i + 1, scaled))
    }
}

/// StepSize2QPLUT: the index 1..=254 whose table step is nearest (the first on a tie)
fn step_to_qp(s: i64, scaled: bool) -> u8 {
    let mut best = 1;
    let mut bd = u64::MAX;
    for i in 1..=254 {
        let d = (s - step_table(i, scaled)).unsigned_abs();
        if d < bd {
            bd = d;
            best = i;
        }
    }
    best as u8
}

/// the RGB path's QPs (memo 3.3): [DC, LP, HP] by channel
fn rgb_qps(q: u8, scaled: bool) -> [[u8; 3]; 3] {
    let q = if q == 1 { 0 } else { q };
    if q <= 1 {
        return [[0; 3]; 3];
    }
    let t = step_table(i32::from(q), scaled);
    let m = |f: f64| (t as f64 * f) as i64;
    let dc = [q, step_to_qp(m(1.65), scaled), step_to_qp(m(1.65), scaled)];
    let lp = [step_to_qp(t, scaled), step_to_qp(m(2.5), scaled), step_to_qp(m(2.5), scaled)];
    let hp = [step_to_qp(m(1.5), scaled).max(2), step_to_qp(m(3.2), scaled).max(2), step_to_qp(m(3.2), scaled).max(2)];
    [dc, lp, hp]
}

/// the other path's QPs (memo 3.2): (channel 0, the others)
fn mixed_qps(q: u8, cf: Cf) -> (u8, u8) {
    let y = if q >= 2 { i32::from(q) } else { 0 };
    let mut c = if y < 16 {
        2 * y
    } else {
        let c = y + 18;
        if y > 48 {
            c + 2
        } else {
            c
        }
    };
    // (4:2:0 and 4:2:2 do not occur: the internal format here is Y, 4:4:4 or CMYK)
    let _ = cf;
    c = c.min(255);
    (y as u8, c as u8)
}

/// setUniformTiling then the starts: n tiles over `mbs` macroblocks, larger first
fn tiles(n: usize, mbs: usize) -> Vec<usize> {
    let mut n = n.max(1);
    while mbs.div_ceil(n) > 256 * 1024 * 1024 {
        n += 1;
    }
    let n = n.min(4096).min(mbs.max(1));
    let mut sizes = Vec::new();
    let mut rem = mbs;
    for i in (2..=n).rev() {
        let s = rem.div_ceil(i);
        sizes.push(s);
        rem -= s;
    }
    let mut starts = vec![0];
    let mut acc = 0;
    for s in sizes {
        acc += s;
        starts.push(acc);
    }
    starts
}

/* ---------------------------------------------------------------------------------------- */
/* the transforms (T.832 5.x: PCT and POT), jxrlib's pipelines */

/// one macroblock column and row more than the picture
struct Layout {
    mbw: usize,
    mbh: usize,
}

impl Layout {
    fn len(&self) -> usize {
        (self.mbh + 2) * (self.mbw + 1) * 256
    }
    fn mb(&self, row: usize, col: usize) -> isize {
        ((row * (self.mbw + 1) + col) * 256) as isize
    }
}

fn at(base: isize, off: isize) -> usize {
    (base + off) as usize
}

fn dct2x2dn(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, cc, mut d) = (p[ia], p[ib], p[ic], p[id]);
    a += d;
    b -= cc;
    let t = (a - b) >> 1;
    let c = t - d;
    d = t - cc;
    a -= d;
    b += c;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn dct2x2up(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, cc, mut d) = (p[ia], p[ib], p[ic], p[id]);
    a += d;
    b -= cc;
    let t = (a - b + 1) >> 1;
    let c = t - d;
    d = t - cc;
    a -= d;
    b += c;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn four_butterfly(p: &mut [i32], base: usize, i: [usize; 16]) {
    for k in 0..4 {
        dct2x2dn(p, base + i[4 * k], base + i[4 * k + 1], base + i[4 * k + 2], base + i[4 * k + 3]);
    }
}

const BUTTERFLY1: [usize; 16] = [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15];
const BUTTERFLY2: [usize; 16] = [0, 192, 48, 240, 64, 128, 112, 176, 16, 208, 32, 224, 80, 144, 96, 160];

fn rotate1(p: &mut [i32], ia: usize, ib: usize) {
    p[ib] -= (p[ia] + 1) >> 1;
    p[ia] += (p[ib] + 1) >> 1;
}

fn irotate1(p: &mut [i32], ia: usize, ib: usize) {
    p[ia] -= (p[ib] + 1) >> 1;
    p[ib] += (p[ia] + 1) >> 1;
}

fn fwd_odd_odd(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], -p[ib], -p[ic], p[id]);
    d += a;
    c -= b;
    let t1 = d >> 1;
    a -= t1;
    let t2 = c >> 1;
    b += t2;
    a += (b * 3 + 4) >> 3;
    b -= (a * 3 + 3) >> 2;
    a += (b * 3 + 3) >> 3;
    b -= t2;
    a += t1;
    c += b;
    d -= a;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn fwd_odd_odd_pre(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    d += a;
    c -= b;
    let t1 = d >> 1;
    a -= t1;
    let t2 = c >> 1;
    b += t2;
    a += (b * 3 + 4) >> 3;
    b -= (a * 3 + 2) >> 2;
    a += (b * 3 + 6) >> 3;
    b -= t2;
    a += t1;
    c += b;
    d -= a;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn fwd_odd(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    b -= c;
    a += d;
    c += (b + 1) >> 1;
    d = ((a + 1) >> 1) - d;
    // ROTATE2(a, b), ROTATE2(c, d)
    b -= (a * 3 + 4) >> 3;
    a += (b * 3 + 4) >> 3;
    d -= (c * 3 + 4) >> 3;
    c += (d * 3 + 4) >> 3;
    d += b >> 1;
    c -= (a + 1) >> 1;
    b -= d;
    a += c;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn dct4x4_stage1(p: &mut [i32], o: usize) {
    four_butterfly(p, o, BUTTERFLY1);
    dct2x2up(p, o, o + 1, o + 2, o + 3);
    fwd_odd_odd(p, o + 15, o + 14, o + 13, o + 12);
    fwd_odd(p, o + 5, o + 4, o + 7, o + 6);
    fwd_odd(p, o + 10, o + 8, o + 11, o + 9);
}

fn dct4x4_stage2(p: &mut [i32], o: usize) {
    four_butterfly(p, o, BUTTERFLY2);
    dct2x2up(p, o, o + 64, o + 16, o + 80);
    fwd_odd_odd(p, o + 160, o + 224, o + 176, o + 240);
    fwd_odd(p, o + 128, o + 192, o + 144, o + 208);
    fwd_odd(p, o + 32, o + 48, o + 96, o + 112);
}

fn hst_enc(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut d, mut c) = (p[ia], p[ib], p[ic], p[id]);
    a += c;
    b -= d;
    c = ((a - b) >> 1) - c;
    d += b >> 1;
    b += c;
    a -= (d * 3 + 4) >> 3;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn hst_enc1(p: &mut [i32], ia: usize, id: usize) {
    let (mut a, mut d) = (p[ia], p[id]);
    d -= a >> 7;
    d += a >> 10;
    d -= (a * 3) >> 4;
    a -= (d * 3) >> 3;
    d = (a >> 1) - d;
    a -= d;
    (p[ia], p[id]) = (a, d);
}

fn hst_enc1_edge(a: &mut i32, d0: &mut i32) {
    let mut d = -*d0;
    *a -= d;
    d += *a >> 1;
    *a -= (d * 3 + 4) >> 3;
    d -= *a >> 7;
    d += *a >> 10;
    d -= (*a * 3) >> 4;
    *a -= (d * 3) >> 3;
    d = (*a >> 1) - d;
    *a -= d;
    *d0 = d;
}

fn pre4(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    a += d;
    b += c;
    d -= (a + 1) >> 1;
    c -= (b + 1) >> 1;
    // ROTATE1(c, d)
    d -= (c + 1) >> 1;
    c += (d + 1) >> 1;
    hst_enc1_edge(&mut a, &mut d);
    hst_enc1_edge(&mut b, &mut c);
    d += (a + 1) >> 1;
    c += (b + 1) >> 1;
    a -= d;
    b -= c;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

/// strPre4x4Stage1Split: the 4x4 around a block corner, `p0` the block up left, `p1` down left
fn pre4x4_stage1_split(p: &mut [i32], p0: usize, p1: usize) {
    let p2 = p0 + 72;
    let p3 = p1 + 64;
    let p0 = p0 + 12;
    let p1 = p1 + 4;
    for k in 0..4 {
        hst_enc(p, p0 + k, p2 + k, p1 + k, p3 + k);
    }
    for k in 0..4 {
        hst_enc1(p, p0 + k, p3 + k);
    }
    rotate1(p, p1 + 2, p1 + 3);
    rotate1(p, p1, p1 + 1);
    rotate1(p, p2 + 1, p2 + 3);
    rotate1(p, p2, p2 + 2);
    fwd_odd_odd_pre(p, p3, p3 + 1, p3 + 2, p3 + 3);
    for k in 0..4 {
        dct2x2dn(p, p0 + k, p2 + k, p1 + k, p3 + k);
    }
}

fn pre4x4_stage1(p: &mut [i32], o: usize) {
    pre4x4_stage1_split(p, o, o + 16);
}

fn pre4x4_stage2_split(p: &mut [i32], p0: isize, p1: isize) {
    let a = |o: isize| at(p0, o);
    let b = |o: isize| at(p1, o);
    hst_enc(p, a(-96), a(96), b(-112), b(80));
    hst_enc(p, a(-32), a(32), b(-48), b(16));
    hst_enc(p, a(-80), a(112), b(-128), b(64));
    hst_enc(p, a(-16), a(48), b(-64), b(0));
    hst_enc1(p, a(-96), b(80));
    hst_enc1(p, a(-32), b(16));
    hst_enc1(p, a(-80), b(64));
    hst_enc1(p, a(-16), b(0));
    rotate1(p, b(-48), b(-112));
    rotate1(p, b(-64), b(-128));
    rotate1(p, a(112), a(96));
    rotate1(p, a(48), a(32));
    fwd_odd_odd_pre(p, b(0), b(64), b(16), b(80));
    dct2x2dn(p, a(-96), b(-112), a(96), b(80));
    dct2x2dn(p, a(-32), b(-48), a(32), b(16));
    dct2x2dn(p, a(-80), b(-128), a(112), b(64));
    dct2x2dn(p, a(-16), b(-64), a(48), b(0));
}

/// transformMacroblock for a full-resolution plane (no hard tiles): called at every
/// (column, row) of 0..=mbw x 0..=mbh, it finishes the macroblock up left
fn forward_mb(p: &mut [i32], l: &Layout, col: usize, row: usize, ov: Overlap, scaled: bool, chroma: bool) {
    let (left, right, top, bottom) = (col == 0, col == l.mbw, row == 0, row == l.mbh);
    let p1 = l.mb(row, col);
    let p0 = if top { 0 } else { l.mb(row - 1, col) };
    let a = |o: isize| at(p0, o);
    let b = |o: isize| at(p1, o);
    if ov != Overlap::None {
        if top && left {
            pre4(p, b(0), b(1), b(2), b(3));
        }
        if top && right {
            pre4(p, b(-59), b(-60), b(-57), b(-58));
        }
        if bottom && left {
            pre4(p, a(58), a(59), a(56), a(57));
        }
        if bottom && right {
            pre4(p, a(-1), a(-2), a(-3), a(-4));
        }
        if !right && !bottom {
            let mut j: isize = if left { 0 } else { -64 };
            while j < 192 {
                if top {
                    let q = p1 + j;
                    pre4(p, at(q, 5), at(q, 4), at(q, 64), at(q, 65));
                    pre4(p, at(q, 7), at(q, 6), at(q, 66), at(q, 67));
                } else {
                    pre4x4_stage1_split(p, a(48 + j), b(j));
                }
                j += 64;
            }
            if left {
                if !top {
                    pre4(p, a(58), a(56), b(0), b(2));
                    pre4(p, a(59), a(57), b(1), b(3));
                }
                for j in [-64isize, -48, -32] {
                    let q = p1 + j;
                    pre4(p, at(q, 74), at(q, 72), at(q, 80), at(q, 82));
                    pre4(p, at(q, 75), at(q, 73), at(q, 81), at(q, 83));
                }
            } else {
                for j in [-64isize, -48, -32] {
                    pre4x4_stage1(p, b(j));
                }
            }
            for o in [0isize, 16, 32, 64, 80, 96, 128, 144, 160] {
                pre4x4_stage1(p, b(o));
            }
        }
        if bottom {
            let mut j: isize = if left { 48 } else { -16 };
            while j < if right { -16 } else { 240 } {
                let q = p0 + j;
                pre4(p, at(q, 15), at(q, 14), at(q, 74), at(q, 75));
                pre4(p, at(q, 13), at(q, 12), at(q, 72), at(q, 73));
                j += 64;
            }
        }
        if right && !bottom {
            if !top {
                pre4(p, a(-1), a(-3), b(-59), b(-57));
                pre4(p, a(-2), a(-4), b(-60), b(-58));
            }
            for j in [-64isize, -48, -32] {
                let q = p1 + j;
                pre4(p, at(q, 15), at(q, 13), at(q, 21), at(q, 23));
                pre4(p, at(q, 14), at(q, 12), at(q, 20), at(q, 22));
            }
        }
    }
    if !top {
        let mut j: isize = if left { 48 } else { -16 };
        while j < if right { 48 } else { 240 } {
            dct4x4_stage1(p, a(j));
            j += 64;
        }
    }
    if !bottom {
        let mut j: isize = if left { 0 } else { -64 };
        while j < if right { 0 } else { 192 } {
            dct4x4_stage1(p, b(j));
            dct4x4_stage1(p, b(j + 16));
            dct4x4_stage1(p, b(j + 32));
            j += 64;
        }
    }
    if ov == Overlap::Two {
        if top && left {
            pre4(p, b(0), b(64), b(16), b(80));
        }
        if top && right {
            pre4(p, b(-128), b(-64), b(-112), b(-48));
        }
        if bottom && left {
            pre4(p, a(32), a(96), a(48), a(112));
        }
        if bottom && right {
            pre4(p, a(-96), a(-32), a(-80), a(-16));
        }
        if (left || right) && !(top || bottom) {
            if left {
                pre4(p, a(32), a(48), b(0), b(16));
                pre4(p, a(96), a(112), b(64), b(80));
            }
            if right {
                pre4(p, a(-96), a(-80), b(-128), b(-112));
                pre4(p, a(-32), a(-16), b(-64), b(-48));
            }
        }
        if !(left || right) {
            if top || bottom {
                if top {
                    pre4(p, b(-128), b(-64), b(0), b(64));
                    pre4(p, b(-112), b(-48), b(16), b(80));
                }
                if bottom {
                    let q = p0 + 32;
                    pre4(p, at(q, -128), at(q, -64), at(q, 0), at(q, 64));
                    pre4(p, at(q, -112), at(q, -48), at(q, 16), at(q, 80));
                }
            } else {
                pre4x4_stage2_split(p, p0, p1);
            }
        }
    }
    if !(top || left) {
        let o = a(-256);
        if scaled && chroma {
            for i in (0..256).step_by(16) {
                p[o + i] >>= 1;
            }
        }
        dct4x4_stage2(p, o);
    }
}

fn forward(planes: &mut [Vec<i32>], l: &Layout, ov: Overlap, scaled: bool) {
    for row in 0..=l.mbh {
        for col in 0..=l.mbw {
            for (i, p) in planes.iter_mut().enumerate() {
                forward_mb(p, l, col, row, ov, scaled, i != 0);
            }
        }
    }
}

fn inv_odd_odd(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    d += a;
    c -= b;
    let t1 = d >> 1;
    a -= t1;
    let t2 = c >> 1;
    b += t2;
    a -= (b * 3 + 3) >> 3;
    b += (a * 3 + 3) >> 2;
    a -= (b * 3 + 4) >> 3;
    b -= t2;
    a += t1;
    c += b;
    d -= a;
    (p[ia], p[ib], p[ic], p[id]) = (a, -b, -c, d);
}

fn inv_odd_odd_post(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    d += a;
    c -= b;
    let t1 = d >> 1;
    a -= t1;
    let t2 = c >> 1;
    b += t2;
    a -= (b * 3 + 6) >> 3;
    b += (a * 3 + 2) >> 2;
    a -= (b * 3 + 4) >> 3;
    b -= t2;
    a += t1;
    c += b;
    d -= a;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn inv_odd(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    b += d;
    a -= c;
    d -= b >> 1;
    c += (a + 1) >> 1;
    // IROTATE2(a, b), IROTATE2(c, d)
    a -= (b * 3 + 4) >> 3;
    b += (a * 3 + 4) >> 3;
    c -= (d * 3 + 4) >> 3;
    d += (c * 3 + 4) >> 3;
    c -= (b + 1) >> 1;
    d = ((a + 1) >> 1) - d;
    b += c;
    a -= d;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

fn idct4x4_stage1(p: &mut [i32], o: usize) {
    dct2x2up(p, o, o + 1, o + 2, o + 3);
    inv_odd(p, o + 5, o + 4, o + 7, o + 6);
    inv_odd(p, o + 10, o + 8, o + 11, o + 9);
    inv_odd_odd(p, o + 15, o + 14, o + 13, o + 12);
    four_butterfly(p, o, BUTTERFLY1);
}

fn idct4x4_stage2(p: &mut [i32], o: usize) {
    inv_odd(p, o + 32, o + 48, o + 96, o + 112);
    inv_odd(p, o + 128, o + 192, o + 144, o + 208);
    inv_odd_odd(p, o + 160, o + 224, o + 176, o + 240);
    dct2x2up(p, o, o + 64, o + 16, o + 80);
    four_butterfly(p, o, BUTTERFLY2);
}

/// strHSTdec1_alternate
fn hst_dec1(p: &mut [i32], ia: usize, id: usize) {
    let (mut a, mut d) = (p[ia], p[id]);
    a += d;
    d = (a >> 1) - d;
    a += (d * 3) >> 3;
    d += (a * 3) >> 4;
    d += a >> 7;
    d -= a >> 10;
    (p[ia], p[id]) = (a, d);
}

/// strHSTdec1_edge
fn hst_dec1_edge(a: &mut i32, d0: &mut i32) {
    let (mut a1, mut d) = (*a, *d0);
    a1 += d;
    d = (a1 >> 1) - d;
    a1 += (d * 3) >> 3;
    d += (a1 * 3) >> 4;
    d += a1 >> 7;
    d -= a1 >> 10;
    a1 += (d * 3 + 4) >> 3;
    d -= a1 >> 1;
    a1 += d;
    (*a, *d0) = (a1, -d);
}

fn hst_dec(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    b -= c;
    a += (d * 3 + 4) >> 3;
    d -= b >> 1;
    c = ((a - b) >> 1) - c;
    p[ic] = d;
    p[id] = c;
    p[ia] = a - c;
    p[ib] = b + d;
}

/// strPost4_alternate: the 4-point post filter of an edge
fn post4(p: &mut [i32], ia: usize, ib: usize, ic: usize, id: usize) {
    let (mut a, mut b, mut c, mut d) = (p[ia], p[ib], p[ic], p[id]);
    a += d;
    b += c;
    d -= (a + 1) >> 1;
    c -= (b + 1) >> 1;
    hst_dec1_edge(&mut a, &mut d);
    hst_dec1_edge(&mut b, &mut c);
    // IROTATE1(c, d)
    c -= (d + 1) >> 1;
    d += (c + 1) >> 1;
    d += (a + 1) >> 1;
    c += (b + 1) >> 1;
    a -= d;
    b -= c;
    (p[ia], p[ib], p[ic], p[id]) = (a, b, c, d);
}

/// strPost4x4Stage1Split_alternate
fn post4x4_stage1_split(p: &mut [i32], p0: usize, p1: usize) {
    let p2 = p0 + 72;
    let p3 = p1 + 64;
    let p0 = p0 + 12;
    let p1 = p1 + 4;
    for k in 0..4 {
        dct2x2dn(p, p0 + k, p2 + k, p1 + k, p3 + k);
    }
    inv_odd_odd_post(p, p3, p3 + 1, p3 + 2, p3 + 3);
    irotate1(p, p1 + 2, p1 + 3);
    irotate1(p, p1, p1 + 1);
    irotate1(p, p2 + 1, p2 + 3);
    irotate1(p, p2, p2 + 2);
    for k in 0..4 {
        hst_dec1(p, p0 + k, p3 + k);
    }
    for k in 0..4 {
        hst_dec(p, p0 + k, p2 + k, p1 + k, p3 + k);
    }
}

fn post4x4_stage1(p: &mut [i32], o: usize) {
    post4x4_stage1_split(p, o, o + 16);
}

/// strPost4x4Stage2Split_alternate
fn post4x4_stage2_split(p: &mut [i32], p0: isize, p1: isize) {
    let a = |o: isize| at(p0, o);
    let b = |o: isize| at(p1, o);
    dct2x2dn(p, a(-96), a(96), b(-112), b(80));
    dct2x2dn(p, a(-32), a(32), b(-48), b(16));
    dct2x2dn(p, a(-80), a(112), b(-128), b(64));
    dct2x2dn(p, a(-16), a(48), b(-64), b(0));
    inv_odd_odd_post(p, b(0), b(64), b(16), b(80));
    irotate1(p, a(48), a(32));
    irotate1(p, a(112), a(96));
    irotate1(p, b(-64), b(-128));
    irotate1(p, b(-48), b(-112));
    hst_dec1(p, a(-96), b(80));
    hst_dec1(p, a(-32), b(16));
    hst_dec1(p, a(-80), b(64));
    hst_dec1(p, a(-16), b(0));
    hst_dec(p, a(-96), b(-112), a(96), b(80));
    hst_dec(p, a(-32), b(-48), a(32), b(16));
    hst_dec(p, a(-80), b(-128), a(112), b(64));
    hst_dec(p, a(-16), b(-64), a(48), b(0));
}

/// invTransformMacroblock_alteredOperators_hard (the header's subversion 1, soft tiles, no
/// post-processing), called at every (column, row) of 0..=mbw x 0..=mbh after the macroblock
/// there is dequantized
fn inverse_mb(p: &mut [i32], l: &Layout, col: usize, row: usize, ov: Overlap, scaled: bool, chroma: bool) {
    let (left, right, top, bottom) = (col == 0, col == l.mbw, row == 0, row == l.mbh);
    let p1 = l.mb(row, col);
    let p0 = if top { 0 } else { l.mb(row - 1, col) };
    let a = |o: isize| at(p0, o);
    let b = |o: isize| at(p1, o);
    if !(bottom || right) {
        idct4x4_stage2(p, b(0));
        if scaled && chroma {
            for i in (0..256).step_by(16) {
                p[b(0) + i] += p[b(0) + i];
            }
        }
    }
    if ov == Overlap::Two {
        if top && left {
            post4(p, b(0), b(64), b(16), b(80));
        }
        if top && right {
            post4(p, b(-128), b(-64), b(-112), b(-48));
        }
        if bottom && left {
            post4(p, a(32), a(96), a(48), a(112));
        }
        if bottom && right {
            post4(p, a(-96), a(-32), a(-80), a(-16));
        }
        if (left || right) && !(top || bottom) {
            for (edge, j) in [(left, 0), (right, -128)] {
                if edge {
                    post4(p, a(j + 32), a(j + 48), b(j), b(j + 16));
                    post4(p, a(j + 96), a(j + 112), b(j + 64), b(j + 80));
                }
            }
        }
        if !(left || right) {
            for (edge, q) in [(top, p1), (bottom, p0 + 32)] {
                if edge {
                    post4(p, at(q, -128), at(q, -64), at(q, 0), at(q, 64));
                    post4(p, at(q, -112), at(q, -48), at(q, 16), at(q, 80));
                }
            }
            if !(top || bottom) {
                post4x4_stage2_split(p, p0, p1);
            }
        }
    }
    if !top {
        let mut j: isize = if left { 32 } else { -96 };
        while j < if right { 32 } else { 160 } {
            idct4x4_stage1(p, a(j));
            idct4x4_stage1(p, a(j + 16));
            j += 64;
        }
    }
    if !bottom {
        let mut j: isize = if left { 0 } else { -128 };
        while j < if right { 0 } else { 128 } {
            idct4x4_stage1(p, b(j));
            idct4x4_stage1(p, b(j + 16));
            j += 64;
        }
    }
    if ov == Overlap::None {
        return;
    }
    if left || right {
        if top && left {
            post4(p, b(0), b(1), b(2), b(3));
        }
        if top && right {
            post4(p, b(-59), b(-60), b(-57), b(-58));
        }
        if bottom && left {
            post4(p, a(58), a(59), a(56), a(57));
        }
        if bottom && right {
            post4(p, a(-1), a(-2), a(-3), a(-4));
        }
        for (edge, j) in [(left, 10), (right, -64 + 14)] {
            if !edge {
                continue;
            }
            if !top {
                let q = p0 + 16 + j;
                post4(p, at(q, 0), at(q, -2), at(q, 6), at(q, 8));
                post4(p, at(q, 1), at(q, -1), at(q, 7), at(q, 9));
                post4(p, at(q, 16), at(q, 14), at(q, 22), at(q, 24));
                post4(p, at(q, 17), at(q, 15), at(q, 23), at(q, 25));
            }
            if !bottom {
                let q = p1 + j;
                post4(p, at(q, 0), at(q, -2), at(q, 6), at(q, 8));
                post4(p, at(q, 1), at(q, -1), at(q, 7), at(q, 9));
            }
            if !(top || bottom) {
                post4(p, a(48 + j), a(48 + j - 2), b(j - 10), b(j - 8));
                post4(p, a(48 + j + 1), a(48 + j - 1), b(j - 9), b(j - 7));
            }
        }
    }
    let mut j: isize = if left { 0 } else { -192 };
    while j < if right { -64 } else { 64 } {
        if top {
            let q = p1 + j;
            post4(p, at(q, 5), at(q, 4), at(q, 64), at(q, 65));
            post4(p, at(q, 7), at(q, 6), at(q, 66), at(q, 67));
            post4x4_stage1(p, b(j));
        } else if bottom {
            post4x4_stage1(p, a(16 + j));
            post4x4_stage1(p, a(32 + j));
            let q = p0 + 48 + j;
            post4(p, at(q, 15), at(q, 14), at(q, 74), at(q, 75));
            post4(p, at(q, 13), at(q, 12), at(q, 72), at(q, 73));
        } else {
            post4x4_stage1(p, a(16 + j));
            post4x4_stage1(p, a(32 + j));
            post4x4_stage1_split(p, a(48 + j), b(j));
            post4x4_stage1(p, b(j));
        }
        j += 64;
    }
}

/* ---------------------------------------------------------------------------------------- */
/* coding a plane */

/// where a macroblock's DC, lowpass or AC coefficients are predicted from (predMacroblockEnc's
/// modes 0..=3)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Pred {
    Left,
    Top,
    Both,
    None,
}

#[derive(Clone, Copy, Default)]
struct PredInfo {
    qp_index: i32,
    cbp: i32,
    dc: i32,
    ad: [i32; 6],
}

/// a plane's codestream and its coefficients as quantized (macroblock layout)
struct Coded {
    bytes: Vec<u8>,
    /// the lowpass set each macroblock took
    lp_set: Vec<usize>,
}

fn num_ones(i: i32) -> i32 {
    (i & 0xffff).count_ones() as i32
}

fn saturate32(x: &mut i32) {
    if (*x + 16) as u32 >= 32 {
        *x = if *x < 0 { -16 } else { 15 };
    }
}

/// writes the image header and the plane header (T.832 8.x)
fn write_headers(pl: &Plane, b: &mut Bits) {
    for &c in b"WMPHOTO\0" {
        b.put(u32::from(c), 8);
    }
    let short = pl.w.div_ceil(16) <= 255 && pl.h.div_ceil(16) <= 255;
    let ncol = pl.tile_x.len() - 1;
    let nrow = pl.tile_y.len() - 1;
    b.put(1, 4); // version
    b.put(0, 1); // hard tiling
    b.put(1, 3); // subversion: new scaling
    b.bit(ncol > 0 || nrow > 0);
    b.put(1, 1); // frequency order
    b.put(0, 3); // orientation
    b.put(1, 1); // index table
    b.put(pl.overlap.code(), 2);
    b.bit(short);
    b.put(1, 1); // long words
    b.put(0, 1); // windowing
    b.put(0, 1); // trimmed flexbits
    b.put(0, 1); // tile stretching
    b.put(0, 2); // reserved
    b.put(0, 1); // interleaved alpha
    b.put(pl.ext.code(), 4);
    b.put(pl.bd.code(), 4);
    let nb = if short { 16 } else { 32 };
    b.put(pl.w as u32 - 1, nb);
    b.put(pl.h as u32 - 1, nb);
    if ncol > 0 || nrow > 0 {
        b.put(ncol as u32, 12);
        b.put(nrow as u32, 12);
    }
    let tb = if short { 8 } else { 16 };
    for i in 0..ncol {
        b.put((pl.tile_x[i + 1] - pl.tile_x[i]) as u32, tb);
    }
    for i in 0..nrow {
        b.put((pl.tile_y[i + 1] - pl.tile_y[i]) as u32, tb);
    }
    b.fill();
    // the image plane header
    b.put(pl.cf.code(), 3);
    b.bit(pl.scaled);
    b.put(0, 4); // all subbands
    match pl.cf {
        Cf::Yuv444 => b.put(0, 8),
        Cf::YOnly | Cf::Cmyk => {}
    }
    match pl.bd {
        Bd::B16 => b.put(0, 8), // shift
        Bd::B1 | Bd::B8 | Bd::B5 | Bd::B565 => {}
    }
    match &pl.qp {
        QpPlan::Uniform { dc, lp, hp } => {
            b.put(1, 1);
            dc.write(b);
            b.put(0, 1); // LP: not DC's
            b.put(1, 1);
            lp.write(b);
            b.put(0, 1); // HP: not LP's
            b.put(1, 1);
            hp.write(b);
        }
        QpPlan::Dquant { .. } => {
            b.put(0, 1);
            b.put(0, 1);
            b.put(0, 1);
            b.put(0, 1);
            b.put(0, 1);
        }
    }
    b.fill();
}

/// the quantizers of a plane: DC by channel, LP and HP sets by channel
struct Quants {
    dc: Vec<Quant>,
    lp: Vec<Vec<Quant>>,
    hp: Vec<Vec<Quant>>,
}

fn quants(pl: &Plane) -> Quants {
    match &pl.qp {
        QpPlan::Uniform { dc, lp, hp } => Quants { dc: dc.quants(true, pl.scaled, true), lp: vec![lp.quants(true, pl.scaled, false)], hp: vec![hp.quants(false, pl.scaled, false)] },
        QpPlan::Dquant { dc, lp, hp, .. } => Quants {
            dc: dc.quants(true, pl.scaled, true),
            lp: lp.iter().map(|s| s.quants(true, pl.scaled, false)).collect(),
            hp: hp.iter().map(|s| s.quants(false, pl.scaled, false)).collect(),
        },
    }
}

/// codes a plane whose coefficients `planes` holds (transformed); leaves them quantized
fn code_plane(pl: &Plane, planes: &mut [Vec<i32>], l: &Layout) -> Coded {
    let nch = pl.nch;
    let cf = pl.cf;
    let q = quants(pl);
    let ncol = pl.tile_x.len();
    let nrow = pl.tile_y.len();
    let mut ctx: Vec<Context> = (0..ncol).map(|_| Context::new(cf)).collect();
    // DC, LP, AC, FL of each tile column
    let mut streams: Vec<[Bits; 4]> = (0..ncol).map(|_| Default::default()).collect();
    // the length of each packet: [tile row][tile column][band]
    let mut lengths: Vec<Vec<[usize; 4]>> = Vec::new();
    let mut marks: Vec<[usize; 4]> = vec![[0; 4]; ncol];
    let mut cur = vec![vec![PredInfo::default(); l.mbw]; nch];
    let mut prev = vec![vec![PredInfo::default(); l.mbw]; nch];
    let mut lp_set = vec![0; l.mbw * l.mbh];
    let (lo, hi, dquant) = match &pl.qp {
        QpPlan::Dquant { lo, hi, .. } => (*lo, *hi, true),
        QpPlan::Uniform { .. } => (0x0fff_ffff, 0x0fff_ffff, false),
    };
    let (mut trow, mut tcol) = (0, 0);
    for mby in 0..l.mbh {
        for mbx in 0..l.mbw {
            // getTilePos
            if mbx == 0 {
                tcol = 0;
            } else if tcol + 1 < ncol && mbx == pl.tile_x[tcol + 1] {
                tcol += 1;
            }
            if mby == 0 {
                trow = 0;
            } else if trow + 1 < nrow && mby == pl.tile_y[trow + 1] {
                trow += 1;
            }
            let ctx_left = mbx == pl.tile_x[tcol];
            let ctx_top = mby == pl.tile_y[trow];
            let reset_rgi = (mbx - pl.tile_x[tcol]) & 15 == 0;
            let reset_ctx = reset_rgi || mbx + 1 == if tcol + 1 == ncol { l.mbw } else { pl.tile_x[tcol + 1] };
            let c = &mut ctx[tcol];
            let st = &mut streams[tcol];
            if ctx_left && ctx_top {
                let pid = ((trow * ncol + tcol) & 0x1f) as u32;
                for (band, s) in st.iter_mut().enumerate() {
                    s.put(0, 8);
                    s.put(0, 8);
                    s.put(1, 8);
                    s.put((pid << 3) + band as u32 + 1, 8);
                }
                if let QpPlan::Dquant { dc, lp, hp, .. } = &pl.qp {
                    dc.write(&mut st[0]);
                    st[1].put(0, 1);
                    st[1].put(2, 4);
                    for s in lp {
                        s.write(&mut st[1]);
                    }
                    st[2].put(0, 1);
                    st[2].put(2, 4);
                    for s in hp {
                        s.write(&mut st[2]);
                    }
                }
            }
            let base = l.mb(mby, mbx) as usize;
            let mut mb: Vec<[i32; 256]> = planes.iter().map(|p| p[base..base + 256].try_into().unwrap()).collect();
            // the macroblock's QP sets from its luma DC (DQUANT)
            let set = if !dquant {
                0
            } else if mb[0][0] < lo {
                1
            } else if mb[0][0] > hi {
                2
            } else {
                0
            };
            lp_set[mby * l.mbw + mbx] = set;
            let (bits_lp, bits_hp) = if dquant { (1, 1) } else { (0, 0) };
            // quantize
            for ch in 0..nch {
                let (qdc, qlp, qhp) = (q.dc[ch], q.lp[set][ch], q.hp[set][ch]);
                for (j, &o) in BLK_OFFSET.iter().enumerate() {
                    let d = &mut mb[ch][o..o + 16];
                    d[0] = if j == 0 { qdc.quantize(d[0]) } else { qlp.quantize(d[0]) };
                    for v in &mut d[1..] {
                        *v = qhp.quantize(*v);
                    }
                }
                planes[ch][base..base + 256].copy_from_slice(&mb[ch]);
            }
            let mut dcb: Vec<[i32; 16]> = mb.iter().map(|m| DC_INDEX.map(|i| m[i])).collect();
            // prediction (predMacroblockEnc)
            let qi = set as i32;
            let dc_mode = if ctx_left && ctx_top {
                Pred::None
            } else if ctx_left {
                Pred::Top
            } else if ctx_top {
                Pred::Left
            } else {
                let (il, it, itl) = (cur[0][mbx - 1].dc, prev[0][mbx].dc, prev[0][mbx - 1].dc);
                let (sh, sv) = if cf == Cf::YOnly {
                    ((itl - il).abs(), (itl - it).abs())
                } else {
                    let s = |ch: usize, other: i32| (prev[ch][mbx - 1].dc - other).abs();
                    ((itl - il).abs() * 2 + s(1, cur[1][mbx - 1].dc) + s(2, cur[2][mbx - 1].dc), (itl - it).abs() * 2 + s(1, prev[1][mbx].dc) + s(2, prev[2][mbx].dc))
                };
                if sh * 4 < sv {
                    Pred::Top
                } else if sv * 4 < sh {
                    Pred::Left
                } else {
                    Pred::Both
                }
            };
            let ad_mode = if dc_mode == Pred::Top && qi == prev[0][mbx].qp_index {
                Pred::Top
            } else if dc_mode == Pred::Left && qi == cur[0][mbx - 1].qp_index {
                Pred::Left
            } else {
                Pred::None
            };
            let ac_mode = {
                let d0 = &dcb[0];
                let mut sh = d0[1].abs() + d0[2].abs() + d0[3].abs();
                let mut sv = d0[4].abs() + d0[8].abs() + d0[12].abs();
                if cf != Cf::YOnly {
                    sh += dcb[1][1].abs() + dcb[2][1].abs();
                    sv += dcb[1][4].abs() + dcb[2][4].abs();
                }
                if sh * 4 < sv {
                    Pred::Top
                } else if sv * 4 < sh {
                    Pred::Left
                } else {
                    Pred::Both
                }
            };
            for ch in 0..nch {
                let d = &dcb[ch];
                cur[ch][mbx] = PredInfo { qp_index: qi, cbp: cur[ch][mbx].cbp, dc: d[0], ad: [d[1], d[2], d[3], d[4], d[8], d[12]] };
            }
            for ch in 0..nch {
                let d = &mut dcb[ch];
                match dc_mode {
                    Pred::Top => d[0] -= prev[ch][mbx].dc,
                    Pred::Left => d[0] -= cur[ch][mbx - 1].dc,
                    Pred::Both => d[0] -= (cur[ch][mbx - 1].dc + prev[ch][mbx].dc) >> 1,
                    Pred::None => {}
                }
                match ad_mode {
                    Pred::Top => {
                        let r = prev[ch][mbx].ad;
                        d[4] -= r[3];
                        d[8] -= r[4];
                        d[12] -= r[5];
                    }
                    Pred::Left => {
                        let r = cur[ch][mbx - 1].ad;
                        d[1] -= r[0];
                        d[2] -= r[1];
                        d[3] -= r[2];
                    }
                    Pred::Both | Pred::None => {}
                }
                let o = &mut mb[ch];
                if ac_mode == Pred::Top {
                    for k in [0, 64, 128, 192] {
                        for j in [48, 32, 16] {
                            for t in [10, 2, 9] {
                                o[k + j + t] -= o[k + j + t - 16];
                            }
                        }
                    }
                } else if ac_mode == Pred::Left {
                    for k in [0, 16, 32, 48] {
                        for j in [192, 128, 64] {
                            for t in [5, 1, 6] {
                                o[k + j + t] -= o[k + j + t - 64];
                            }
                        }
                    }
                }
            }
            let vertical = ac_mode == Pred::Top;
            // DC
            {
                let io = &mut st[0];
                let mut lm = [0i32; 2];
                if cf == Cf::Yuv444 {
                    let (dy, du, dv) = (dcb[0][0], dcb[1][0], dcb[2][0]);
                    let b0 = c.model_dc.bits[0] as u32;
                    let b1 = c.model_dc.bits[1] as u32;
                    let (qy, qu, qv) = (dy.unsigned_abs() >> b0, du.unsigned_abs() >> b1, dv.unsigned_abs() >> b1);
                    let index = usize::from(qy != 0) * 4 + usize::from(qu != 0) * 2 + usize::from(qv != 0);
                    c.ah[2].put_fixed(io, index);
                    if qy != 0 {
                        abs_level(io, &mut c.ah[3], qy);
                        lm[0] += 1;
                    }
                    io.put(dy.unsigned_abs(), b0);
                    if dy != 0 {
                        io.bit(dy < 0);
                    }
                    for (v, qv) in [(du, qu), (dv, qv)] {
                        if qv != 0 {
                            abs_level(io, &mut c.ah[4], qv);
                            lm[1] += 1;
                        }
                        io.put(v.unsigned_abs(), b1);
                        if v != 0 {
                            io.bit(v < 0);
                        }
                    }
                } else {
                    for (ch, d) in dcb.iter().enumerate() {
                        let mbits = c.model_dc.bits[usize::from(ch > 0)] as u32;
                        let v = d[0];
                        let qv = v.unsigned_abs() >> mbits;
                        if qv != 0 {
                            io.bit(true);
                            abs_level(io, &mut c.ah[3], qv);
                            lm[usize::from(ch > 0)] += 1;
                        } else {
                            io.bit(false);
                        }
                        io.put(v.unsigned_abs(), mbits);
                        if v != 0 {
                            io.bit(v < 0);
                        }
                    }
                }
                c.model_dc.update(cf, nch, lm);
            }
            // lowpass
            {
                let io = &mut st[1];
                if bits_lp > 0 {
                    io.put(u32::from(set != 0), 1);
                    if set != 0 {
                        io.put(set as u32 - 1, bits_lp);
                    }
                }
                if reset_rgi {
                    reset_totals(&mut c.scan_lp);
                }
                let mut resid = vec![[0i32; 16]; nch];
                let mut rl = Vec::with_capacity(nch);
                for ch in 0..nch {
                    let mbits = c.model_lp.bits[usize::from(ch > 0)];
                    rl.push(adaptive_scan(&dcb[ch], &mut resid[ch], &mut c.scan_lp, mbits));
                }
                if cf == Cf::Yuv444 {
                    let imax = 7;
                    let (mut cm, mut cz) = (c.cbp_count_max, c.cbp_count_zero);
                    let cbp = i32::from(!rl[0].is_empty()) + i32::from(!rl[1].is_empty()) * 2 + i32::from(!rl[2].is_empty()) * 4;
                    if cz <= 0 || cm < 0 {
                        let mut v = cbp;
                        if cm < cz {
                            v = imax - cbp;
                        }
                        if v == 0 {
                            io.put(0, 1);
                        } else if v == 1 {
                            io.put(4, 3);
                        } else {
                            io.put((v + imax + 1) as u32, 4);
                        }
                    } else {
                        io.put(cbp as u32, 3);
                    }
                    cm += 1 - 4 * i32::from(cbp == imax);
                    cz += 1 - 4 * i32::from(cbp == 0);
                    c.cbp_count_max = cm.clamp(-8, 7);
                    c.cbp_count_zero = cz.clamp(-8, 7);
                } else {
                    for r in &rl {
                        io.bit(!r.is_empty());
                    }
                }
                let mut lm = [0i32; 2];
                for ch in 0..nch {
                    let mbits = c.model_lp.bits[usize::from(ch > 0)];
                    if !rl[ch].is_empty() {
                        lm[usize::from(ch > 0)] += rl[ch].len() as i32;
                        encode_block(io, &mut c.ah, ch > 0, &rl[ch], CTDC, 1);
                    }
                    if mbits > 0 {
                        for k in 1..16 {
                            let r = resid[ch][k];
                            io.put((r >> 1) as u32, mbits as u32 + (r & 1) as u32);
                        }
                    }
                }
                c.model_lp.update(cf, nch, lm);
                if reset_ctx {
                    c.adapt_lowpass();
                }
            }
            // highpass
            {
                let [_, _, io, fl] = st;
                if bits_hp > 0 {
                    io.put(u32::from(set != 0), 1);
                    if set != 0 {
                        io.put(set as u32 - 1, bits_hp);
                    }
                }
                if reset_rgi {
                    reset_totals(&mut c.scan_h);
                    reset_totals(&mut c.scan_v);
                }
                // CBP (predCBPEnc, CodeCBP)
                let mut cbp = vec![0i32; nch];
                let mut diff = vec![0i32; nch];
                for ch in 0..nch {
                    let thr = (1i32 << c.model_ac.bits[usize::from(ch > 0)]) - 1;
                    let thr2 = thr * 2 + 1;
                    let mut v = 0;
                    for (j, &o) in BLK_OFFSET.iter().enumerate() {
                        if (1..16).any(|i| (mb[ch][o + i] + thr) as u32 >= thr2 as u32) {
                            v |= 1 << j;
                        }
                    }
                    cbp[ch] = v;
                    cur[ch][mbx].cbp = v;
                    let mut pred = if ctx_left {
                        if ctx_top {
                            1
                        } else {
                            (prev[ch][mbx].cbp >> 10) & 1
                        }
                    } else {
                        (cur[ch][mbx - 1].cbp >> 5) & 1
                    };
                    pred |= (v & 0x3300) << 2;
                    pred |= (v & 0xcc) << 6;
                    pred |= (v & 0x33) << 2;
                    pred |= (v & 0x11) << 1;
                    pred |= (v & 0x2) << 3;
                    let m = usize::from(ch > 0);
                    diff[ch] = match c.cbp_state[m] {
                        0 => pred ^ v,
                        1 => v,
                        _ => v ^ 0xffff,
                    };
                    let n = num_ones(v);
                    c.cbp_count0[m] += n - 3;
                    saturate32(&mut c.cbp_count0[m]);
                    c.cbp_count1[m] += 16 - n - 3;
                    saturate32(&mut c.cbp_count1[m]);
                    c.cbp_state[m] = if c.cbp_count0[m] < 0 {
                        if c.cbp_count0[m] < c.cbp_count1[m] {
                            1
                        } else {
                            2
                        }
                    } else if c.cbp_count1[m] < 0 {
                        2
                    } else {
                        0
                    };
                }
                const NUM_ONES: [usize; 16] = [0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4];
                const TAB_LEN: [u32; 16] = [0, 2, 2, 2, 2, 2, 3, 2, 2, 3, 3, 2, 3, 2, 2, 0];
                const TAB_CODE: [u32; 16] = [0, 0, 1, 0, 2, 1, 4, 3, 3, 5, 6, 2, 7, 1, 0, 0];
                let rounds = if cf == Cf::Yuv444 { 1 } else { nch };
                for i in 0..rounds {
                    let mut dy = diff[i];
                    let (mut du, mut dv) = if cf == Cf::Yuv444 { (diff[1], diff[2]) } else { (0, 0) };
                    let mut d = dy;
                    if cf == Cf::Yuv444 {
                        d |= du | dv;
                    }
                    let mut pattern = 0usize;
                    for _ in 0..4 {
                        pattern |= usize::from(d & 0xf != 0) * 0x10;
                        d >>= 4;
                        pattern >>= 1;
                    }
                    let count = NUM_ONES[pattern];
                    c.cbpcy1.put(io, count);
                    if TAB_LEN[pattern] > 0 {
                        io.put(TAB_CODE[pattern], TAB_LEN[pattern]);
                    }
                    for _ in 0..4 {
                        let mut code = (dy & 0xf) as usize;
                        let (cu, cv) = ((du & 0xf) as usize, (dv & 0xf) as usize);
                        if cf == Cf::Yuv444 {
                            code |= usize::from(cu != 0) << 4;
                            code |= usize::from(cv != 0) << 5;
                        }
                        dy >>= 4;
                        du >>= 4;
                        dv >>= 4;
                        if code != 0 {
                            const TAB0: [usize; 16] = [0, 1, 1, 2, 1, 3, 3, 4, 1, 3, 3, 4, 2, 4, 4, 5];
                            const FL0: [u32; 16] = [0, 2, 2, 1, 2, 2, 2, 2, 2, 2, 2, 2, 1, 2, 2, 0];
                            const CODE0: [u32; 16] = [0, 0, 1, 0, 2, 0, 1, 0, 3, 2, 3, 1, 1, 2, 3, 0];
                            let chroma = code >> 4;
                            let y = code & 0xf;
                            let val = if chroma != 0 {
                                if TAB0[y] > 2 {
                                    8
                                } else {
                                    TAB0[y] + 5
                                }
                            } else {
                                TAB0[y] - 1
                            };
                            c.cbpcy.put(io, val);
                            if chroma != 0 {
                                if chroma == 1 {
                                    io.put(1, 1);
                                } else {
                                    io.put(3 - chroma as u32, 2);
                                }
                            }
                            if val == 8 {
                                if TAB0[y] == 3 {
                                    io.put(1, 1);
                                } else {
                                    io.put(5 - TAB0[y] as u32, 2);
                                }
                            }
                            if FL0[y] > 0 {
                                io.put(CODE0[y], FL0[y]);
                            }
                            if cf == Cf::Yuv444 {
                                for pat in [cu, cv] {
                                    if pat != 0 {
                                        c.ah[1].put_fixed(io, NUM_ONES[pat] - 1);
                                        if TAB_LEN[pat] > 0 {
                                            io.put(TAB_CODE[pat], TAB_LEN[pat]);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // coefficients (CodeCoeffs)
                let mut lm = [0i32; 2];
                let mut mbits = c.model_ac.bits[0];
                let mut chroma = false;
                for ch in 0..nch {
                    let mut pattern = cbp[ch];
                    let mut index = 0;
                    for block in 0..4 {
                        for _ in 0..4 {
                            let o = BLK_OFFSET[index];
                            let coef = &mb[ch][o..o + 16];
                            let flex = mbits as u32;
                            if pattern & 1 == 0 {
                                if flex > 0 {
                                    for k in 1..16 {
                                        let d = coef[DCT_INDEX[k]];
                                        let a = d.unsigned_abs();
                                        let mut word = a & ((1 << flex) - 1);
                                        let mut len = flex;
                                        if a != 0 {
                                            word = word * 2 + u32::from(d < 0);
                                            len += 1;
                                        }
                                        fl.put(word, len);
                                    }
                                }
                            } else {
                                let mut resid = [0i32; 16];
                                let scan = if vertical { &mut c.scan_v } else { &mut c.scan_h };
                                let rl = adaptive_scan(coef, &mut resid, scan, mbits);
                                lm[usize::from(chroma)] += rl.len() as i32;
                                encode_block(io, &mut c.ah, chroma, &rl, CTDC + CONTEXTX, 1);
                                if flex > 0 {
                                    for k in 1..16 {
                                        let r = resid[DCT_INDEX[k]];
                                        fl.put((r >> 1) as u32, flex + (r & 1) as u32);
                                    }
                                }
                            }
                            pattern >>= 1;
                            index += 1;
                        }
                        if block == 3 {
                            mbits = c.model_ac.bits[1];
                            chroma = true;
                        }
                    }
                }
                c.model_ac.update(cf, nch, lm);
                if reset_ctx {
                    c.adapt_highpass();
                }
            }
            // the end of a tile row
            if mbx + 1 == l.mbw && (mby + 1 == l.mbh || (trow + 1 < nrow && mby + 1 == pl.tile_y[trow + 1])) {
                let mut row = Vec::new();
                for (t, s) in streams.iter_mut().enumerate() {
                    let mut lens = [0; 4];
                    for k in 0..4 {
                        s[k].fill();
                        lens[k] = s[k].len() - marks[t][k];
                        marks[t][k] = s[k].len();
                    }
                    row.push(lens);
                }
                lengths.push(row);
                if mby + 1 != l.mbh {
                    for c in &mut ctx {
                        c.reset();
                    }
                }
            }
        }
        std::mem::swap(&mut cur, &mut prev);
    }
    // the header, the index table, the profile and level, then the packets
    let mut b = Bits::default();
    write_headers(pl, &mut b);
    b.put(1, 16);
    let mut off = 0u64;
    for row in &lengths {
        for t in row {
            for &n in t {
                if n <= 4 {
                    b.put(0xff, 8);
                } else {
                    b.vlw(off);
                    off += n as u64;
                }
            }
        }
    }
    b.vlw(4);
    b.put(66, 8);
    b.put(128, 8);
    b.put(1, 16);
    b.fill();
    let mut bytes = b.out;
    let mut from = vec![[0usize; 4]; ncol];
    for row in &lengths {
        for (t, lens) in row.iter().enumerate() {
            for k in 0..4 {
                if lens[k] > 4 {
                    bytes.extend_from_slice(&streams[t][k].out[from[t][k]..from[t][k] + lens[k]]);
                }
                from[t][k] += lens[k];
            }
        }
    }
    Coded { bytes, lp_set }
}

/// dequantizes the coefficients `code_plane` left and transforms them back (the decoder)
fn reconstruct(pl: &Plane, planes: &mut [Vec<i32>], l: &Layout, lp_set: &[usize]) {
    let q = quants(pl);
    for mby in 0..l.mbh {
        for mbx in 0..l.mbw {
            let base = l.mb(mby, mbx) as usize;
            let set = lp_set[mby * l.mbw + mbx];
            for (ch, p) in planes.iter_mut().enumerate() {
                for (j, &o) in BLK_OFFSET.iter().enumerate() {
                    let d = &mut p[base + o..base + o + 16];
                    d[0] *= if j == 0 { q.dc[ch].qp } else { q.lp[set][ch].qp };
                    for v in &mut d[1..] {
                        *v *= q.hp[set][ch].qp;
                    }
                }
            }
        }
    }
    for row in 0..=l.mbh {
        for col in 0..=l.mbw {
            for (ch, p) in planes.iter_mut().enumerate() {
                inverse_mb(p, l, col, row, pl.overlap, pl.scaled, ch != 0);
            }
        }
    }
}

/* ---------------------------------------------------------------------------------------- */
/* pixel formats */

/// the formats WIC hands this encoder as they are
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Bw,
    Gray8,
    Gray16,
    Bgr555,
    Bgr565,
    Bgr24,
    Rgb24,
    Bgr32,
    Bgra,
    Pbgra,
    Rgb48,
    Rgba64,
    Cmyk,
}

impl Format {
    pub fn from_name(f: &str) -> Option<Format> {
        Some(match f {
            "bw" => Format::Bw,
            "gray8" => Format::Gray8,
            "gray16" => Format::Gray16,
            "bgr555" => Format::Bgr555,
            "bgr565" => Format::Bgr565,
            "bgr24" => Format::Bgr24,
            "rgb24" => Format::Rgb24,
            "bgr32" => Format::Bgr32,
            "bgra" => Format::Bgra,
            "pbgra" => Format::Pbgra,
            "rgb48" => Format::Rgb48,
            "rgba64" => Format::Rgba64,
            "cmyk" => Format::Cmyk,
            _ => return None,
        })
    }
    pub fn bits(self) -> usize {
        match self {
            Format::Bw => 1,
            Format::Gray8 => 8,
            Format::Gray16 | Format::Bgr555 | Format::Bgr565 => 16,
            Format::Bgr24 | Format::Rgb24 => 24,
            Format::Bgr32 | Format::Bgra | Format::Pbgra | Format::Cmyk => 32,
            Format::Rgb48 => 48,
            Format::Rgba64 => 64,
        }
    }
    /// the last byte of its WIC GUID {6fddc324-4e03-4bfe-b185-3d77768dc9xx}
    fn guid_tail(self) -> u8 {
        match self {
            Format::Bw => 0x05,
            Format::Gray8 => 0x08,
            Format::Bgr555 => 0x09,
            Format::Bgr565 => 0x0a,
            Format::Gray16 => 0x0b,
            Format::Bgr24 => 0x0c,
            Format::Rgb24 => 0x0d,
            Format::Bgr32 => 0x0e,
            Format::Bgra => 0x0f,
            Format::Pbgra => 0x10,
            Format::Rgb48 => 0x15,
            Format::Rgba64 => 0x16,
            Format::Cmyk => 0x1c,
        }
    }
    fn ext(self) -> (ExtCf, Bd) {
        match self {
            Format::Bw => (ExtCf::YOnly, Bd::B1),
            Format::Gray8 => (ExtCf::YOnly, Bd::B8),
            Format::Gray16 => (ExtCf::YOnly, Bd::B16),
            Format::Bgr555 => (ExtCf::Rgb, Bd::B5),
            Format::Bgr565 => (ExtCf::Rgb, Bd::B565),
            Format::Bgr24 | Format::Rgb24 | Format::Bgr32 | Format::Bgra | Format::Pbgra => (ExtCf::Rgb, Bd::B8),
            Format::Rgb48 | Format::Rgba64 => (ExtCf::Rgb, Bd::B16),
            Format::Cmyk => (ExtCf::Cmyk, Bd::B8),
        }
    }
    /// the planar alpha's bit depth, if it has alpha
    fn alpha(self) -> Option<Bd> {
        match self {
            Format::Bgra | Format::Pbgra => Some(Bd::B8),
            Format::Rgba64 => Some(Bd::B16),
            Format::Bw | Format::Gray8 | Format::Gray16 | Format::Bgr555 | Format::Bgr565 | Format::Bgr24 | Format::Rgb24 | Format::Bgr32 | Format::Rgb48 | Format::Cmyk => None,
        }
    }
}

/// a picture: rows of `stride` bytes
pub struct Picture<'a> {
    pub fmt: Format,
    pub w: usize,
    pub h: usize,
    pub stride: usize,
    pub px: &'a [u8],
    pub dpi: (f64, f64),
}

pub struct Encoded {
    pub file: Vec<u8>,
    /// the pixels WIC decodes from the file, rows packed to whole bytes
    pub decoded: Vec<u8>,
}

fn u16_at(row: &[u8], i: usize) -> i32 {
    i32::from(u16::from_le_bytes([row[2 * i], row[2 * i + 1]]))
}

/// the channel planes of a picture (inputMBRow): color converted, padded by repeating the last
/// column and row
fn load(p: &Picture, pl: &Plane, l: &Layout) -> Vec<Vec<i32>> {
    let mut planes = vec![vec![0i32; l.len()]; pl.nch];
    let sh = if pl.scaled { 3 } else { 0 };
    for y in 0..l.mbh * 16 {
        let row = &p.px[y.min(p.h - 1) * p.stride..];
        for x in 0..l.mbw * 16 {
            let i = x.min(p.w - 1);
            let pos = l.mb(y >> 4, x >> 4) as usize + idx_cc(y & 15, x & 15);
            let rgb = |r: i32, g: i32, b: i32, off: i32| {
                let (mut r, mut g, mut b) = (r << sh, g << sh, b << sh);
                b -= r;
                r += ((b + 1) >> 1) - g;
                g += r >> 1;
                [g - (off << sh), -r, b]
            };
            let v: Vec<i32> = match p.fmt {
                Format::Bw => vec![i32::from((row[i >> 3] >> (7 - (i & 7))) & 1) << sh],
                Format::Gray8 => vec![(i32::from(row[i]) << sh) - (128 << sh)],
                Format::Gray16 => vec![(u16_at(row, i) << sh) - ((1 << 15) << sh)],
                Format::Bgr24 | Format::Bgr32 | Format::Bgra | Format::Pbgra => {
                    let n = p.fmt.bits() / 8;
                    let s = &row[i * n..];
                    rgb(i32::from(s[2]), i32::from(s[1]), i32::from(s[0]), 128).to_vec()
                }
                Format::Rgb24 => {
                    let s = &row[i * 3..];
                    rgb(i32::from(s[0]), i32::from(s[1]), i32::from(s[2]), 128).to_vec()
                }
                Format::Rgb48 | Format::Rgba64 => {
                    let n = p.fmt.bits() / 16;
                    rgb(u16_at(row, i * n), u16_at(row, i * n + 1), u16_at(row, i * n + 2), 1 << 15).to_vec()
                }
                Format::Bgr555 => {
                    let v = u16_at(row, i);
                    // (as jxrlib: "r" the low five bits)
                    rgb(v & 0x1f, (v >> 5) & 0x1f, (v >> 10) & 0x1f, 16).to_vec()
                }
                Format::Bgr565 => {
                    let v = u16_at(row, i);
                    let (r, g, b) = ((v & 0x1f) << (sh + 1), ((v >> 5) & 0x3f) << sh, (v >> 11) << (sh + 1));
                    let (mut r, mut g, mut b) = (r, g, b);
                    b -= r;
                    r += ((b + 1) >> 1) - g;
                    g += r >> 1;
                    vec![g - (32 << sh), -r, b]
                }
                Format::Cmyk => {
                    let s = &row[i * 4..];
                    let (mut c, mut m, mut yy, mut k) = (i32::from(s[0]) << sh, i32::from(s[1]) << sh, i32::from(s[2]) << sh, i32::from(s[3]) << sh);
                    yy -= c;
                    c += ((yy + 1) >> 1) - m;
                    m += (c >> 1) - k;
                    k += (m + 1) >> 1;
                    vec![(128 << sh) - m, c, -yy, k]
                }
            };
            for (ch, v) in v.into_iter().enumerate() {
                planes[ch][pos] = v;
            }
        }
    }
    planes
}

/// the planar alpha (Y only, lossless, unscaled)
fn load_alpha(p: &Picture, l: &Layout, bd: Bd) -> Vec<Vec<i32>> {
    let mut plane = vec![0i32; l.len()];
    for y in 0..l.mbh * 16 {
        let row = &p.px[y.min(p.h - 1) * p.stride..];
        for x in 0..l.mbw * 16 {
            let i = x.min(p.w - 1);
            let pos = l.mb(y >> 4, x >> 4) as usize + idx_cc(y & 15, x & 15);
            plane[pos] = match bd {
                Bd::B16 => u16_at(row, i * 4 + 3) - (1 << 15),
                Bd::B1 | Bd::B8 | Bd::B5 | Bd::B565 => i32::from(row[i * 4 + 3]) - 128,
            };
        }
    }
    vec![plane]
}

fn clip8(v: i32) -> u8 {
    v.clamp(0, 255) as u8
}

fn clip16(v: i32) -> [u8; 2] {
    (v.clamp(0, 65535) as u16).to_le_bytes()
}

/// the decoder's output (outputMBRow): the pixels back in the picture's format
fn output(p: &Picture, pl: &Plane, planes: &[Vec<i32>], l: &Layout, alpha: Option<&[Vec<i32>]>) -> Vec<u8> {
    let stride = (p.w * p.fmt.bits()).div_ceil(8);
    let mut out = vec![0u8; stride * p.h];
    let sh = if pl.scaled { 3 } else { 0 };
    let round8 = if pl.scaled { 3 } else { 0 };
    let round16 = if sh == 0 { 0 } else { 1 << (sh - 1) };
    for y in 0..p.h {
        for x in 0..p.w {
            let pos = l.mb(y >> 4, x >> 4) as usize + idx_cc(y & 15, x & 15);
            let v: Vec<i32> = planes.iter().map(|pl| pl[pos]).collect();
            let a = alpha.map(|a| a[0][pos]);
            let o = &mut out[y * stride..];
            let icc = |y: i32, u: i32, vv: i32, bias: i32| {
                let (mut r, mut g, mut b) = (-u, y + bias, vv);
                g -= r >> 1;
                r -= ((b + 1) >> 1) - g;
                b += r;
                (r, g, b)
            };
            match p.fmt {
                Format::Bw => {
                    if v[0] >= if sh > 0 { 1 << (sh - 1) } else { 1 } {
                        o[x >> 3] |= 0x80 >> (x & 7);
                    }
                }
                Format::Gray8 => o[x] = clip8((v[0] + (128 << sh) + round8) >> sh),
                Format::Gray16 => o[2 * x..2 * x + 2].copy_from_slice(&clip16((v[0] + ((1 << 15) << sh) + round16) >> sh)),
                Format::Bgr24 | Format::Rgb24 | Format::Bgr32 | Format::Bgra | Format::Pbgra => {
                    let (r, g, b) = icc(v[0], v[1], v[2], (128 << sh) + round8);
                    let n = p.fmt.bits() / 8;
                    let s = &mut o[x * n..];
                    let (ir, ib) = if p.fmt == Format::Rgb24 { (0, 2) } else { (2, 0) };
                    s[ir] = clip8(r >> sh);
                    s[1] = clip8(g >> sh);
                    s[ib] = clip8(b >> sh);
                    if let Some(a) = a {
                        s[3] = clip8(a + 128);
                    }
                }
                Format::Rgb48 | Format::Rgba64 => {
                    let (r, g, b) = icc(v[0], v[1], v[2], ((1 << 15) << sh) + round16);
                    let n = p.fmt.bits() / 8;
                    let s = &mut o[x * n..];
                    s[0..2].copy_from_slice(&clip16(r >> sh));
                    s[2..4].copy_from_slice(&clip16(g >> sh));
                    s[4..6].copy_from_slice(&clip16(b >> sh));
                    if let Some(a) = a {
                        s[6..8].copy_from_slice(&clip16(a + (1 << 15)));
                    }
                }
                Format::Bgr555 => {
                    let (r, g, b) = icc(v[0], v[1], v[2], (16 << sh) + round8);
                    let px = (r >> sh).clamp(0, 31) + ((g >> sh).clamp(0, 31) << 5) + ((b >> sh).clamp(0, 31) << 10);
                    o[2 * x..2 * x + 2].copy_from_slice(&(px as u16).to_le_bytes());
                }
                Format::Bgr565 => {
                    let (r, g, b) = icc(v[0], v[1], v[2], (32 << sh) + round8);
                    let px = (r >> (sh + 1)).clamp(0, 31) + ((g >> sh).clamp(0, 63) << 5) + ((b >> (sh + 1)).clamp(0, 31) << 11);
                    o[2 * x..2 * x + 2].copy_from_slice(&(px as u16).to_le_bytes());
                }
                Format::Cmyk => {
                    let (mut c, mut m, mut yy, mut k) = (v[1], -v[0] + (128 << sh), -v[2], v[3] + round8);
                    k -= (m + 1) >> 1;
                    m -= (c >> 1) - k;
                    c -= ((yy + 1) >> 1) - m;
                    yy += c;
                    let s = &mut o[x * 4..];
                    s[0] = clip8(c >> sh);
                    s[1] = clip8(m >> sh);
                    s[2] = clip8(yy >> sh);
                    s[3] = clip8(k >> sh);
                }
            }
        }
    }
    out
}

/// the plane parameters of the image at a quality (memo 2, 3)
fn image_plane(p: &Picture, q: u8, overlap: Overlap) -> Plane {
    let (ext, bd) = p.fmt.ext();
    let scaled = q > 1;
    let (cf, nch) = match ext {
        ExtCf::YOnly => (Cf::YOnly, 1),
        ExtCf::Rgb => (Cf::Yuv444, 3),
        ExtCf::Cmyk => (Cf::Cmyk, 4),
    };
    let qp = if ext == ExtCf::Rgb {
        let [dc, lp, hp] = rgb_qps(q, scaled);
        let set = |i: [u8; 3]| QpSet { mode: ChMode::Independent, index: i.to_vec() };
        if bd == Bd::B16 && q > 16 {
            let (lo, hi) = match overlap {
                Overlap::None => (-38, -24),
                Overlap::One => (-28, -18),
                Overlap::Two => (-18, -11),
            };
            let finer = |i: [u8; 3]| i.map(|i| if i < 18 { 2 } else { i - 16 });
            let coarser = |i: [u8; 3]| i.map(|i| if i > 238 { 255 } else { i + 16 });
            QpPlan::Dquant { dc: set(dc), lp: [set(lp), set(finer(lp)), set(coarser(lp))], hp: [set(hp), set(finer(hp)), set(coarser(hp))], lo: lo * 100000, hi: hi * 100000 }
        } else {
            QpPlan::Uniform { dc: set(dc), lp: set(lp), hp: set(hp) }
        }
    } else {
        let (y, c) = mixed_qps(q, cf);
        let mut index = vec![c; nch];
        index[0] = y;
        let set = QpSet { mode: ChMode::Mixed, index };
        QpPlan::Uniform { dc: set.clone(), lp: set.clone(), hp: set }
    };
    Plane {
        w: p.w,
        h: p.h,
        ext,
        bd,
        cf,
        nch,
        overlap,
        scaled,
        qp,
        tile_x: tiles(((p.w - 1) >> 8) + 1, p.w.div_ceil(16)),
        tile_y: tiles(((p.h - 1) >> 8) + 1, p.h.div_ceil(16)),
    }
}

fn ifd_entry(out: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32) {
    out.extend_from_slice(&tag.to_le_bytes());
    out.extend_from_slice(&ty.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&value.to_le_bytes());
}

/// encodes a picture as WIC's JPEG XR encoder does, and decodes it back as WIC's decoder does
pub fn encode(p: &Picture, quality: Quality) -> Encoded {
    let bd = p.fmt.ext().1;
    encode_at(p, quality.index(bd), quality.overlap(bd))
}

/// encodes at a QP index (0 and 1 lossless) and overlap, the rest as `encode`
pub fn encode_at(p: &Picture, index: u8, overlap: Overlap) -> Encoded {
    let pl = image_plane(p, index, overlap);
    let l = Layout { mbw: p.w.div_ceil(16), mbh: p.h.div_ceil(16) };
    let mut planes = load(p, &pl, &l);
    forward(&mut planes, &l, pl.overlap, pl.scaled);
    let coded = code_plane(&pl, &mut planes, &l);
    reconstruct(&pl, &mut planes, &l, &coded.lp_set);
    let mut alpha = None;
    if let Some(bd) = p.fmt.alpha() {
        let apl = Plane {
            ext: ExtCf::YOnly,
            bd,
            cf: Cf::YOnly,
            nch: 1,
            scaled: false,
            qp: {
                let s = QpSet { mode: ChMode::Uniform, index: vec![0] };
                QpPlan::Uniform { dc: s.clone(), lp: s.clone(), hp: s }
            },
            ..pl.clone()
        };
        let mut a = load_alpha(p, &l, bd);
        forward(&mut a, &l, apl.overlap, false);
        let ac = code_plane(&apl, &mut a, &l);
        reconstruct(&apl, &mut a, &l, &ac.lp_set);
        alpha = Some((ac.bytes, a));
    }
    let decoded = output(p, &pl, &planes, &l, alpha.as_ref().map(|a| a.1.as_slice()));
    // the container
    let n: u32 = if alpha.is_some() { 11 } else { 9 };
    let guid_at = 8 + 2 + 12 * n + 4;
    let image_at = guid_at + 16;
    let image_len = coded.bytes.len() as u32;
    let mut f = vec![b'I', b'I', 0xbc, 0x01, 8, 0, 0, 0];
    f.extend_from_slice(&(n as u16).to_le_bytes());
    ifd_entry(&mut f, 0xbc01, 1, 16, guid_at);
    ifd_entry(&mut f, 0xbc02, 4, 1, 0);
    ifd_entry(&mut f, 0xbc04, 4, 1, 0);
    ifd_entry(&mut f, 0xbc80, 4, 1, p.w as u32);
    ifd_entry(&mut f, 0xbc81, 4, 1, p.h as u32);
    ifd_entry(&mut f, 0xbc82, 11, 1, (p.dpi.0 as f32).to_bits());
    ifd_entry(&mut f, 0xbc83, 11, 1, (p.dpi.1 as f32).to_bits());
    ifd_entry(&mut f, 0xbcc0, 4, 1, image_at);
    ifd_entry(&mut f, 0xbcc1, 4, 1, image_len);
    if let Some((a, _)) = &alpha {
        ifd_entry(&mut f, 0xbcc2, 4, 1, image_at + image_len);
        ifd_entry(&mut f, 0xbcc3, 4, 1, a.len() as u32);
    }
    f.extend_from_slice(&[0; 4]);
    f.extend_from_slice(&[0x24, 0xc3, 0xdd, 0x6f, 0x03, 0x4e, 0xfe, 0x4b, 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, p.fmt.guid_tail()]);
    f.extend_from_slice(&coded.bytes);
    if let Some((a, _)) = alpha {
        f.extend_from_slice(&a);
    }
    Encoded { file: f, decoded }
}
