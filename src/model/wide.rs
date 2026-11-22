//! A 256-bit two's complement integer for the scanner's exact fractions: workspace points to
//! 2^48 (Windows' limit) make crossings whose numerators reach about 2^148 and whose
//! comparisons multiply them by denominators of about 2^98.

use std::cmp::Ordering;
use std::ops::{Add, Mul, Neg, Sub};

/// four 64-bit limbs, least significant first, two's complement
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct W([u64; 4]);

impl W {
    pub const ZERO: W = W([0; 4]);
    pub const ONE: W = W([1, 0, 0, 0]);

    pub fn is_neg(self) -> bool {
        self.0[3] >> 63 == 1
    }

    pub fn abs(self) -> W {
        if self.is_neg() { -self } else { self }
    }

    fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }

    fn shr1(self) -> W {
        let l = self.0;
        W([l[0] >> 1 | l[1] << 63, l[1] >> 1 | l[2] << 63, l[2] >> 1 | l[3] << 63, l[3] >> 1])
    }

    fn shl1(self) -> W {
        let l = self.0;
        W([l[0] << 1, l[1] << 1 | l[0] >> 63, l[2] << 1 | l[1] >> 63, l[3] << 1 | l[2] >> 63])
    }

    fn bit(self, i: u32) -> bool {
        self.0.get((i / 64) as usize).is_some_and(|l| l >> (i % 64) & 1 == 1)
    }

    fn trailing_zeros(self) -> u32 {
        let mut n = 0;
        for l in self.0 {
            if l != 0 {
                return n + l.trailing_zeros();
            }
            n += 64;
        }
        n
    }

    fn shr(self, k: u32) -> W {
        (0..k).fold(self, |a, _| a.shr1())
    }

    fn shl(self, k: u32) -> W {
        (0..k).fold(self, |a, _| a.shl1())
    }

    /// the greatest common divisor of the magnitudes (Stein's)
    pub fn gcd(a: W, b: W) -> W {
        let (mut a, mut b) = (a.abs(), b.abs());
        if a.is_zero() {
            return b;
        }
        if b.is_zero() {
            return a;
        }
        let k = a.trailing_zeros().min(b.trailing_zeros());
        a = a.shr(a.trailing_zeros());
        loop {
            b = b.shr(b.trailing_zeros());
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }
            b = b - a;
            if b.is_zero() {
                return a.shl(k);
            }
        }
    }

    /// the quotient, truncated toward zero, of a by d (d not zero)
    pub fn div(a: W, d: W) -> W {
        let neg = a.is_neg() != d.is_neg();
        let (n, d) = (a.abs(), d.abs());
        let mut q = W::ZERO;
        let mut r = W::ZERO;
        for i in (0..256).rev() {
            r = r.shl1();
            if n.bit(i) {
                r.0[0] |= 1;
            }
            q = q.shl1();
            if r >= d {
                r = r - d;
                q.0[0] |= 1;
            }
        }
        if neg { -q } else { q }
    }

    /// the nearest f64, as `as f64` rounds an i128
    pub fn to_f64(self) -> f64 {
        let neg = self.is_neg();
        let m = self.abs();
        let top = (0..256u32).rev().find(|&i| m.bit(i));
        let v = match top {
            None => 0.0,
            Some(t) if t < 127 => (u128::from(m.0[1]) << 64 | u128::from(m.0[0])) as f64,
            Some(t) => {
                // the top 127 bits, the rest kept as a sticky bit: rounding to 53 bits is then
                // the same as rounding the whole
                let s = t - 126;
                let sticky = m.trailing_zeros() < s;
                let h = m.shr(s);
                let x = (u128::from(h.0[1]) << 64 | u128::from(h.0[0])) | u128::from(sticky);
                (0..s).fold(x as f64, |a, _| a * 2.0)
            }
        };
        if neg { -v } else { v }
    }
}

impl From<i128> for W {
    fn from(v: i128) -> W {
        let u = v as u128;
        let ext = if v < 0 { u64::MAX } else { 0 };
        W([u as u64, (u >> 64) as u64, ext, ext])
    }
}

impl Add for W {
    type Output = W;
    fn add(self, o: W) -> W {
        let mut r = [0u64; 4];
        let mut c = false;
        for (i, x) in r.iter_mut().enumerate() {
            let (s1, c1) = self.0[i].overflowing_add(o.0[i]);
            let (s2, c2) = s1.overflowing_add(u64::from(c));
            *x = s2;
            c = c1 || c2;
        }
        W(r)
    }
}

impl Neg for W {
    type Output = W;
    fn neg(self) -> W {
        W(self.0.map(|l| !l)) + W::ONE
    }
}

impl Sub for W {
    type Output = W;
    fn sub(self, o: W) -> W {
        self + -o
    }
}

impl Mul for W {
    type Output = W;
    /// the product modulo 2^256 (exact while it fits)
    fn mul(self, o: W) -> W {
        let mut r = [0u64; 4];
        for i in 0..4 {
            let mut c: u128 = 0;
            for j in 0..4 - i {
                let t = u128::from(self.0[i]) * u128::from(o.0[j]) + u128::from(r[i + j]) + c;
                r[i + j] = t as u64;
                c = t >> 64;
            }
        }
        W(r)
    }
}

impl Ord for W {
    fn cmp(&self, o: &W) -> Ordering {
        let f = |w: &W| {
            let mut l = w.0;
            l[3] ^= 1 << 63;
            [l[3], l[2], l[1], l[0]]
        };
        f(self).cmp(&f(o))
    }
}

impl PartialOrd for W {
    fn partial_cmp(&self, o: &W) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
