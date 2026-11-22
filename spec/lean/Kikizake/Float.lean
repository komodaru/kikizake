import Kikizake.Mirror
/-! IEEE 754 binary floating point of m fraction bits and w exponent bits, as the models compute it
(Rust `f32` and `f64` on x86 and x86-64, round to nearest even), in exact integer arithmetic: a
value is its bits; `add`, `sub`, `mul`, `div` take the exact result of the two values and round it
once. NaN results are the one quiet NaN here (the mirrors compare any NaN as that one).
Kikizake.F32 names the binary32 and binary64 instances and their mirrors. -/
namespace Kikizake.Float

/-- a format: fraction bits, exponent bits -/
structure Fmt where
  m : Nat
  w : Nat

variable (F : Fmt)

/-- the quiet NaN, +infinity -/
def qnan : Nat := (2 ^ F.w - 1) * 2 ^ F.m + 2 ^ (F.m - 1)
def inf : Nat := (2 ^ F.w - 1) * 2 ^ F.m
/-- the sign F bit -/
def sbit : Nat := 2 ^ (F.w + F.m)
/-- the exponent's bias -/
def bias : Nat := 2 ^ (F.w - 1) - 1

def sign (x : Nat) : Bool := (x >>> (F.w + F.m)) % 2 == 1
def expField (x : Nat) : Nat := (x >>> F.m) % 2 ^ F.w
def frac (x : Nat) : Nat := x % 2 ^ F.m

def isNaN (x : Nat) : Bool := expField F x == 2 ^ F.w - 1 && frac F x != 0
def isInf (x : Nat) : Bool := expField F x == 2 ^ F.w - 1 && frac F x == 0

/-- the least power of two of a subnormal's unit -/
def emin : Int := 1 - (bias F : Int) - F.m

/-- a finite value's integer significand and its power of two: x = ±m 2^e -/
def sig (x : Nat) : Nat := if expField F x = 0 then frac F x else frac F x + 2 ^ F.m
def ex (x : Nat) : Int := if expField F x = 0 then emin F else (expField F x : Int) - (bias F : Int) - F.m

/-- 2^k for an integer k as a numerator over a denominator -/
def pow2 (k : Int) : Nat × Nat := if 0 ≤ k then (2 ^ k.toNat, 1) else (1, 2 ^ (-k).toNat)

/-- the binary32 nearest a / b (b > 0), halves to even, of sign F s; past the largest finite
value infinity -/
def round (s : Bool) (a b : Nat) : Nat :=
  let sb := if s then sbit F else 0
  if a = 0 then sb else
  -- L: floor(log2(a / b))
  let d : Int := (Nat.log2 a : Int) - (Nat.log2 b : Int)
  let L : Int := if a * (pow2 d).2 < b * (pow2 d).1 then d - 1 else d
  let Q : Int := Max.max (L - F.m) (emin F)
  let (num, den) := if 0 ≤ Q then (a, b * 2 ^ Q.toNat) else (a * 2 ^ (-Q).toNat, b)
  let m0 := num / den
  let r := num % den
  let m := if 2 * r > den ∨ (2 * r = den ∧ m0 % 2 = 1) then m0 + 1 else m0
  let (m, Q) := if m = 2 ^ (F.m + 1) then (2 ^ F.m, Q + 1) else (m, Q)
  if Q + F.m > (bias F : Int) then sb + inf F
  else if m < 2 ^ F.m then sb + m
  else sb + (Q + bias F + F.m).toNat * 2 ^ F.m + (m - 2 ^ F.m)

/-- the exact signed value ±m 2^e as an integer over a power of two: (sign, numerator, denominator)
of a sum -/
def addExact (x y : Nat) : Bool × Nat × Nat :=
  let e := Min.min (ex F x) (ex F y)
  let vx : Int := (if sign F x then -1 else 1) * (sig F x * 2 ^ (ex F x - e).toNat : Nat)
  let vy : Int := (if sign F y then -1 else 1) * (sig F y * 2 ^ (ex F y - e).toNat : Nat)
  let v := vx + vy
  let (pn, pd) := pow2 e
  (decide (v < 0), v.natAbs * pn, pd)

def add (x y : Nat) : Nat :=
  if isNaN F x || isNaN F y then qnan F
  else if isInf F x && isInf F y then (if sign F x == sign F y then (if sign F x then sbit F else 0) + inf F else qnan F)
  else if isInf F x then x
  else if isInf F y then y
  else
    let (s, a, b) := addExact F x y
    if a = 0 then (if sign F x && sign F y then sbit F else 0) else round F s a b

/-- the sign F bit flipped -/
def neg (x : Nat) : Nat := if sign F x then x - sbit F else x + sbit F

def sub (x y : Nat) : Nat := add F x (neg F y)

def mul (x y : Nat) : Nat :=
  let s := sign F x != sign F y
  let sb := if s then sbit F else 0
  if isNaN F x || isNaN F y then qnan F
  else if isInf F x || isInf F y then (if sig F x = 0 && !isInf F x || sig F y = 0 && !isInf F y then qnan F else sb + inf F)
  else
    let (pn, pd) := pow2 (ex F x + ex F y)
    round F s (sig F x * sig F y * pn) pd

def div (x y : Nat) : Nat :=
  let s := sign F x != sign F y
  let sb := if s then sbit F else 0
  if isNaN F x || isNaN F y then qnan F
  else if isInf F x then (if isInf F y then qnan F else sb + inf F)
  else if isInf F y then sb
  else if sig F y = 0 then (if sig F x = 0 then qnan F else sb + inf F)
  else
    let (pn, pd) := pow2 (ex F x - ex F y)
    round F s (sig F x * pn) (sig F y * pd)

/-- Newton's iteration for the integer square root from x at or above it: it falls to
floor(sqrt n) and stops there -/
def isqrtGo (n : Nat) : Nat → Nat → Nat
  | 0, x => x
  | fuel + 1, x => let y := (x + n / x) / 2; if y < x then isqrtGo n fuel y else x

/-- floor(sqrt n), from 2^(floor(log2 n) / 2 + 1), above it -/
def isqrt (n : Nat) : Nat := if n = 0 then 0 else isqrtGo n 64 (2 ^ (Nat.log2 n / 2 + 1))

/-- the square root (IEEE 754 squareRoot, correctly rounded): the significand with an even
exponent, scaled by 4^(m + 3), has the integer square root s of m + 4 bits and more; when s^2
falls short of it the root lies strictly between s and s + 1, where no value of the format and
no halfway point between two lies, so s + 1/2 rounds as the root does; then rounded once. A
NaN or one below 0 is the quiet NaN, a zero itself, +infinity itself -/
def sqrt (x : Nat) : Nat :=
  if isNaN F x then qnan F
  else if sig F x = 0 then x
  else if sign F x then qnan F
  else if isInf F x then x
  else
    let (m, e) := if ex F x % 2 = 0 then (sig F x, ex F x) else (2 * sig F x, ex F x - 1)
    let k := F.m + 3
    let n := m * 4 ^ k
    let s := isqrt n
    let (a, b) := if s * s = n then (s, 1) else (2 * s + 1, 2)
    let (pn, pd) := pow2 (e / 2 - k)
    round F false (a * pn) (b * pd)

/-- the binary32 nearest a natural number (Rust `n as f32`) -/
def ofNat (n : Nat) : Nat := round F false n 1

/-- the value of a finite binary32 as a rational: (sign, numerator, denominator) -/
def toRat (x : Nat) : Bool × Nat × Nat := let (pn, pd) := pow2 (ex F x); (sign F x, sig F x * pn, pd)

/-- f32's order on non-NaN values: x < y -/
def lt (x y : Nat) : Bool :=
  let (sx, nx, dx) := toRat F x
  let (sy, ny, dy) := toRat F y
  let vx : Int := (if sx then -1 else 1) * (nx * dy : Nat)
  let vy : Int := (if sy then -1 else 1) * (ny * dx : Nat)
  if isInf F x || isInf F y then
    (isInf F x && sign F x && !(isInf F y && sign F y)) || (isInf F y && !sign F y && !(isInf F x && !sign F x))
  else decide (vx < vy)

/-- x clamped to lo..hi (Rust `f32::clamp`: a NaN stays) -/
def clamp (x lo hi : Nat) : Nat := if isNaN F x then x else if lt F x lo then lo else if lt F hi x then hi else x

/-- the same, NaN as the one quiet NaN -/
def canon (x : Nat) : Nat := if isNaN F x then qnan F else x

end Kikizake.Float
