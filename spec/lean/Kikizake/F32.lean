import Kikizake.Mirror
/-! IEEE 754 binary32 as the models compute it (Rust `f32` on x86 and x86-64, round to nearest
even), in exact integer arithmetic: a value is its 32 bits; `add`, `sub`, `mul`, `div` take the
exact result of the two values and round it once. NaN results are the quiet NaN 0x7fc00000 here
(the mirrors compare any NaN as that one). Mirrors `f32.add`, `f32.sub`, `f32.mul`, `f32.div`,
`f32.ofNat`, against Rust's `f32` operators. -/
namespace Kikizake.F32

/-- the quiet NaN, +infinity -/
def qnan : Nat := 0x7fc00000
def inf : Nat := 0x7f800000

def sign (x : Nat) : Bool := (x >>> 31) % 2 == 1
def expField (x : Nat) : Nat := (x >>> 23) % 256
def frac (x : Nat) : Nat := x % 2 ^ 23

def isNaN (x : Nat) : Bool := expField x == 255 && frac x != 0
def isInf (x : Nat) : Bool := expField x == 255 && frac x == 0

/-- a finite value's integer significand and its power of two: x = ±m 2^e -/
def sig (x : Nat) : Nat := if expField x = 0 then frac x else frac x + 2 ^ 23
def ex (x : Nat) : Int := if expField x = 0 then -149 else (expField x : Int) - 150

/-- 2^k for an integer k as a numerator over a denominator -/
def pow2 (k : Int) : Nat × Nat := if 0 ≤ k then (2 ^ k.toNat, 1) else (1, 2 ^ (-k).toNat)

/-- the binary32 nearest a / b (b > 0), halves to even, of sign s; past the largest finite
value infinity -/
def round (s : Bool) (a b : Nat) : Nat :=
  let sb := if s then 2 ^ 31 else 0
  if a = 0 then sb else
  -- L: floor(log2(a / b))
  let d : Int := (Nat.log2 a : Int) - (Nat.log2 b : Int)
  let L : Int := if a * (pow2 d).2 < b * (pow2 d).1 then d - 1 else d
  let Q : Int := Max.max (L - 23) (-149)
  let (num, den) := if 0 ≤ Q then (a, b * 2 ^ Q.toNat) else (a * 2 ^ (-Q).toNat, b)
  let m0 := num / den
  let r := num % den
  let m := if 2 * r > den ∨ (2 * r = den ∧ m0 % 2 = 1) then m0 + 1 else m0
  let (m, Q) := if m = 2 ^ 24 then (2 ^ 23, Q + 1) else (m, Q)
  if Q + 23 > 127 then sb + inf
  else if m < 2 ^ 23 then sb + m
  else sb + (Q + 150).toNat * 2 ^ 23 + (m - 2 ^ 23)

/-- the exact signed value ±m 2^e as an integer over a power of two: (sign, numerator, denominator)
of a sum -/
def addExact (x y : Nat) : Bool × Nat × Nat :=
  let e := Min.min (ex x) (ex y)
  let vx : Int := (if sign x then -1 else 1) * (sig x * 2 ^ (ex x - e).toNat : Nat)
  let vy : Int := (if sign y then -1 else 1) * (sig y * 2 ^ (ex y - e).toNat : Nat)
  let v := vx + vy
  let (pn, pd) := pow2 e
  (decide (v < 0), v.natAbs * pn, pd)

def add (x y : Nat) : Nat :=
  if isNaN x || isNaN y then qnan
  else if isInf x && isInf y then (if sign x == sign y then (if sign x then 2 ^ 31 else 0) + inf else qnan)
  else if isInf x then x
  else if isInf y then y
  else
    let (s, a, b) := addExact x y
    if a = 0 then (if sign x && sign y then 2 ^ 31 else 0) else round s a b

/-- the sign bit flipped -/
def neg (x : Nat) : Nat := if sign x then x - 2 ^ 31 else x + 2 ^ 31

def sub (x y : Nat) : Nat := add x (neg y)

def mul (x y : Nat) : Nat :=
  let s := sign x != sign y
  let sb := if s then 2 ^ 31 else 0
  if isNaN x || isNaN y then qnan
  else if isInf x || isInf y then (if sig x = 0 && !isInf x || sig y = 0 && !isInf y then qnan else sb + inf)
  else
    let (pn, pd) := pow2 (ex x + ex y)
    round s (sig x * sig y * pn) pd

def div (x y : Nat) : Nat :=
  let s := sign x != sign y
  let sb := if s then 2 ^ 31 else 0
  if isNaN x || isNaN y then qnan
  else if isInf x then (if isInf y then qnan else sb + inf)
  else if isInf y then sb
  else if sig y = 0 then (if sig x = 0 then qnan else sb + inf)
  else
    let (pn, pd) := pow2 (ex x - ex y)
    round s (sig x * pn) (sig y * pd)

/-- the binary32 nearest a natural number (Rust `n as f32`) -/
def ofNat (n : Nat) : Nat := round false n 1

/-- the value of a finite binary32 as a rational: (sign, numerator, denominator) -/
def toRat (x : Nat) : Bool × Nat × Nat := let (pn, pd) := pow2 (ex x); (sign x, sig x * pn, pd)

/-- f32's order on non-NaN values: x < y -/
def lt (x y : Nat) : Bool :=
  let (sx, nx, dx) := toRat x
  let (sy, ny, dy) := toRat y
  let vx : Int := (if sx then -1 else 1) * (nx * dy : Nat)
  let vy : Int := (if sy then -1 else 1) * (ny * dx : Nat)
  if isInf x || isInf y then
    (isInf x && sign x && !(isInf y && sign y)) || (isInf y && !sign y && !(isInf x && !sign x))
  else decide (vx < vy)

/-- x clamped to lo..hi (Rust `f32::clamp`: a NaN stays) -/
def clamp (x lo hi : Nat) : Nat := if isNaN x then x else if lt x lo then lo else if lt hi x then hi else x

/-- the same, NaN as the one quiet NaN -/
def canon (x : Nat) : Nat := if isNaN x then qnan else x

/-- binary32 multiplication is commutative, NaNs and infinities too -/
theorem mul_comm (x y : Nat) : mul x y = mul y x := by
  unfold mul
  dsimp only
  rw [bne_comm, Nat.mul_comm (sig x) (sig y), Int.add_comm (ex x) (ex y)]
  by_cases hx : isNaN x <;> by_cases hy : isNaN y <;> simp only [hx, hy, Bool.or_true, Bool.or_false, ↓reduceIte, Bool.false_eq_true]
  by_cases ix : isInf x <;> by_cases iy : isInf y <;> simp [ix, iy, Bool.or_comm]

private theorem addExact_comm (x y : Nat) : addExact x y = addExact y x := by
  unfold addExact
  dsimp only
  rw [Int.min_comm (ex x) (ex y), Int.add_comm ((if sign x = true then -1 else 1) * _)]

/-- binary32 addition is commutative, NaNs, infinities and signed zeros too -/
theorem add_comm (x y : Nat) : add x y = add y x := by
  unfold add
  rw [addExact_comm x y]
  by_cases hx : isNaN x <;> by_cases hy : isNaN y <;> simp only [hx, hy, Bool.or_true, Bool.or_false, ↓reduceIte, Bool.false_eq_true]
  by_cases ix : isInf x <;> by_cases iy : isInf y <;> simp only [ix, iy, Bool.and_true, Bool.and_false, ↓reduceIte, Bool.false_eq_true]
  · by_cases h : sign x = sign y
    · simp [h]
    · have : (sign y == sign x) = false := by simp; exact fun e => h e.symm
      simp [h, this]
  · rw [Bool.and_comm]

def mirrors : List Mirror := [
  ⟨"f32.add", fun | [x, y] => do pure [canon (add (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.sub", fun | [x, y] => do pure [canon (sub (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.mul", fun | [x, y] => do pure [canon (mul (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.div", fun | [x, y] => do pure [canon (div (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.ofNat", fun | [n] => do pure [ofNat (← nat n)] | _ => none⟩,
  ⟨"f32.clamp", fun | [x, lo, hi] => do pure [clamp (← nat x) (← nat lo) (← nat hi)] | _ => none⟩
]

end Kikizake.F32
