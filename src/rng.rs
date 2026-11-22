//! xorshift32: the generator of `fill` (SPEC.md) and of the case generators; splitmix64: the
//! sampler of the Lean mirrors' domains (src/lean/), whose arguments span up to 64 bits.

#[derive(Clone, Debug)]
pub struct XorShift32(pub u32);

impl XorShift32 {
    pub fn new(seed: u32) -> Self {
        // (0 is a fixed point of xorshift: the executor uses the same substitute)
        XorShift32(if seed == 0 { 0x9e37_79b9 } else { seed })
    }
    pub fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// uniform enough in lo..=hi (modulo bias is irrelevant for test generation). A span of at
    /// most 2^32 values takes one word, as the cases were generated; a wider one two (high word
    /// first), so the values past lo + 2^32 are reached too (the review of 84b05c8, 4.1: one word
    /// left every value but the lowest 2^32 out, and hi - lo + 1 overflowed for 0..=i64::MAX).
    /// An empty span (hi < lo) gives lo.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let Some(span) = Span::of(lo, hi) else { return lo };
        let draw = if span.0 <= 1 << 32 {
            u64::from(self.next())
        } else {
            u64::from(self.next()) << 32 | u64::from(self.next())
        };
        span.at(lo, draw)
    }
    /// a byte: u8 cannot hold the 256 that range(0, 256) can give
    pub fn byte(&mut self) -> u8 {
        u8::try_from(self.range(0, 255)).unwrap_or(u8::MAX)
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        pick_at(xs, self.range(0, xs.len() as i64 - 1))
    }
    pub fn chance(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

/// xs[i] (an empty list has nothing to pick: it panics, as indexing does)
fn pick_at<T>(xs: &[T], i: i64) -> &T {
    &xs[usize::try_from(i).unwrap_or(0)]
}

/// the number of values of a nonempty span lo..=hi (1 ..= 2^64)
#[derive(Clone, Copy, Debug)]
struct Span(u128);

impl Span {
    fn of(lo: i64, hi: i64) -> Option<Span> {
        let n = i128::from(hi).checked_sub(i128::from(lo))?.checked_add(1)?;
        u128::try_from(n).ok().filter(|&n| n > 0).map(Span)
    }
    /// lo plus a draw reduced into the span
    fn at(self, lo: i64, draw: u64) -> i64 {
        let off = u128::from(draw).checked_rem(self.0).unwrap_or(0);
        i128::try_from(off).ok().and_then(|o| i128::from(lo).checked_add(o)).and_then(|v| i64::try_from(v).ok()).unwrap_or(lo)
    }
}

/// splitmix64 (Steele, Lea, Flood 2014): 64 bits a draw, so a sample of a domain wider than 32
/// bits reaches all of it. The Lean mirrors' samples come from it (kikizake-lint `lean-sampler`),
/// the cases and pixels from XorShift32, whose sequence the goldens were made with.
#[derive(Clone, Debug)]
pub struct Sample64(u64);

impl Sample64 {
    /// (a 32-bit seed, as XorShift32's: the mirrors' seeds stay as they were written)
    pub fn new(seed: u32) -> Self {
        Sample64(u64::from(seed))
    }
    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// uniform in lo..=hi (lo when hi < lo), any span of i64 (2^64 values at most)
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        let Some(span) = Span::of(lo, hi) else { return lo };
        span.at(lo, self.next())
    }
    /// a byte (a draw's top 8 bits)
    pub fn byte(&mut self) -> u8 {
        u8::try_from(self.next() >> 56).unwrap_or(u8::MAX)
    }
    /// 32 bits (a draw's high half)
    pub fn word(&mut self) -> u32 {
        u32::try_from(self.next() >> 32).unwrap_or(0)
    }
    pub fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        let last = i64::try_from(xs.len()).unwrap_or(i64::MAX).saturating_sub(1);
        pick_at(xs, self.range(0, last))
    }
    /// one time in n (never for n = 0)
    pub fn chance(&mut self, n: u32) -> bool {
        self.next().checked_rem(u64::from(n)) == Some(0)
    }
}
