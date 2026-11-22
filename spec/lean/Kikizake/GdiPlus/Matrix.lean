import Kikizake.Mirror
import Kikizake.F32
/-! GDI+'s matrix arithmetic (spec/gdiplus.txt GP.MATRIX.X86, GP.MATRIX.X64) as
src/model/gdiplus.rs `Round`, `mul_in`, `transform_in` and `inverse` compute it, on binary32 bits
(Kikizake.F32): x86 rounds each operation to nearest, x86-64 toward minus infinity (an exact zero
sum -0 unless both terms are +0; a quotient whose nearest value is a zero, an infinity or a NaN is
that value); and the same arithmetic in the gamma corrected SourceOver of x86-64 (`gamma_rest`, `mix64`,
GP.FILL.GAMMA.X64); mirrors `gp.round`, `gp.matmul`, `gp.matpoint`, `gp.matinv`, `gp.gamma64`. -/
namespace Kikizake.GdiPlus.Matrix
open Kikizake.Float

private abbrev F := F32.fmt

/-- the largest binary32 at most a / b (b > 0) of sign s: a positive value truncated, a negative
one's magnitude rounded up; past the largest finite value, that value when positive and minus
infinity when negative -/
def roundD (s : Bool) (a b : Nat) : Nat :=
  let sb := if s then sbit F else 0
  if a = 0 then sb else
  let d : Int := (Nat.log2 a : Int) - (Nat.log2 b : Int)
  let L : Int := if a * (pow2 d).2 < b * (pow2 d).1 then d - 1 else d
  let Q : Int := Max.max (L - F.m) (emin F)
  let (num, den) := if 0 ≤ Q then (a, b * 2 ^ Q.toNat) else (a * 2 ^ (-Q).toNat, b)
  let m0 := num / den
  let m := if s ∧ num % den ≠ 0 then m0 + 1 else m0
  let (m, Q) := if m = 2 ^ (F.m + 1) then (2 ^ F.m, Q + 1) else (m, Q)
  if Q + F.m > (bias F : Int) then (if s then sb + inf F else inf F - 1)
  else if m < 2 ^ F.m then sb + m
  else sb + (Q + bias F + F.m).toNat * 2 ^ F.m + (m - 2 ^ F.m)

/-- the sum rounded toward minus infinity -/
def addD (x y : Nat) : Nat :=
  if isNaN F x || isNaN F y || isInf F x || isInf F y then F32.add x y
  else
    let (s, a, b) := addExact F x y
    if a = 0 then (if !sign F x && !sign F y then 0 else sbit F) else roundD s a b

def subD (x y : Nat) : Nat := addD x (neg F y)

/-- the product rounded toward minus infinity (a zero product as to nearest) -/
def mulD (x y : Nat) : Nat :=
  if isNaN F x || isNaN F y || isInf F x || isInf F y || sig F x = 0 || sig F y = 0 then F32.mul x y
  else
    let (pn, pd) := pow2 (ex F x + ex F y)
    roundD (sign F x != sign F y) (sig F x * sig F y * pn) pd

/-- the quotient rounded toward minus infinity, but the nearest one when that is a zero, an infinity
or a NaN -/
def divD (x y : Nat) : Nat :=
  let q := F32.div x y
  if isNaN F q || isInf F q || sig F q = 0 then q
  else
    let (pn, pd) := pow2 (ex F x - ex F y)
    roundD (sign F x != sign F y) (sig F x * pn) (sig F y * pd)

/-- one way of rounding: to nearest (x86) or toward minus infinity (x86-64) -/
structure Ops where
  add : Nat → Nat → Nat
  sub : Nat → Nat → Nat
  mul : Nat → Nat → Nat
  div : Nat → Nat → Nat

def nearest : Ops := ⟨F32.add, F32.sub, F32.mul, F32.div⟩
def down : Ops := ⟨addD, subD, mulD, divD⟩

def ops (d : Bool) : Ops := if d then down else nearest

/-- GP.MATRIX.MUL: a b, each element a dot product of two products, the translation added last -/
def matMul (r : Ops) (a b : List Nat) : List Nat :=
  let e (i : Nat) := a.getD i 0
  let f (i : Nat) := b.getD i 0
  let dot (x y z w : Nat) := r.add (r.mul x y) (r.mul z w)
  [dot (e 0) (f 0) (e 1) (f 2), dot (e 0) (f 1) (e 1) (f 3),
   dot (e 2) (f 0) (e 3) (f 2), dot (e 2) (f 1) (e 3) (f 3),
   r.add (dot (e 4) (f 0) (e 5) (f 2)) (f 4), r.add (dot (e 4) (f 1) (e 5) (f 3)) (f 5)]

/-- GP.MATRIX.POINT: (m11 x + m21 y) + dx, (m12 x + m22 y) + dy -/
def point (r : Ops) (m : List Nat) (x y : Nat) : List Nat :=
  let e (i : Nat) := m.getD i 0
  [r.add (r.add (r.mul (e 0) x) (r.mul (e 2) y)) (e 4), r.add (r.add (r.mul (e 1) x) (r.mul (e 3) y)) (e 5)]

def one : Nat := 0x3f800000

def finite (x : Nat) : Bool := !isNaN F x && !isInf F x

/-- GP.MATRIX.INVERT: none for a determinant of ±0 or an element not finite -/
def inverse (r : Ops) (m : List Nat) : Option (List Nat) :=
  let e (i : Nat) := m.getD i 0
  let det := r.sub (r.mul (e 0) (e 3)) (r.mul (e 1) (e 2))
  if sig F det = 0 then none else
  let q := r.div one det
  let v := [r.mul (e 3) q, r.mul (neg F (e 1)) q, r.mul (neg F (e 2)) q, r.mul (e 0) q,
    r.mul (r.sub (r.mul (e 2) (e 5)) (r.mul (e 3) (e 4))) q,
    r.mul (r.sub (r.mul (e 1) (e 4)) (r.mul (e 0) (e 5))) q]
  if v.all finite then some v else none

/-- GP.MATRIX.X64: rounding toward minus infinity, 1 / 3 is a unit of the last place below
x86's, -1 / 3 the same value negated (now one above in magnitude), and x - x is -0 where x86 gives
+0 -/
theorem down_steps :
    divD one 0x40400000 = 0x3eaaaaaa ∧ F32.div one 0x40400000 = 0x3eaaaaab ∧
    divD (neg F one) 0x40400000 = 0xbeaaaaab ∧ F32.div (neg F one) 0x40400000 = 0xbeaaaaab ∧
    subD one one = 0x80000000 ∧ F32.sub one one = 0 ∧ addD 0 0 = 0 := by decide

/-- GP.MATRIX.X64: 0.1 times 0.1 rounds down where x86 rounds up; the largest finite value
doubled stays that value, and its negation doubled is minus infinity -/
theorem down_products :
    mulD 0x3dcccccd 0x3dcccccd = 0x3c23d70a ∧ F32.mul 0x3dcccccd 0x3dcccccd = 0x3c23d70b ∧
    mulD 0x7f7fffff 0x40000000 = 0x7f7fffff ∧ mulD 0xff7fffff 0x40000000 = 0xff800000 := by decide

/-- GP.MATRIX.INVERT: scaling by 3 inverts to scaling by the reciprocal, a unit of the last place
apart on x86 and x86-64; a determinant of 0 has none -/
theorem inverse_scale :
    inverse nearest [0x40400000, 0, 0, 0x40400000, 0, 0] =
      some [0x3eaaaaab, 0x80000000, 0x80000000, 0x3eaaaaab, 0, 0] ∧
    inverse down [0x40400000, 0, 0, 0x40400000, 0, 0] =
      some [0x3eaaaaaa, 0x80000000, 0x80000000, 0x3eaaaaaa, 0x80000000, 0x80000000] ∧
    inverse down [one, one, one, one, 0, 0] = none ∧ inverse nearest [one, one, one, one, 0, 0] = none := by
  decide

private theorem addExact_swap (x y : Nat) : addExact F x y = addExact F y x := by
  unfold addExact
  dsimp only
  rw [Int.min_comm (ex F x) (ex F y), Int.add_comm ((if sign F x = true then -1 else 1) * _)]

/-- GP.MATRIX.MUL: rounding toward minus infinity, the sum and the product of two values are the
same either way round, as to nearest (F32.add_comm, F32.mul_comm) -/
theorem down_comm (x y : Nat) : addD x y = addD y x ∧ mulD x y = mulD y x := by
  constructor
  · unfold addD
    rw [addExact_swap x y, F32.add_comm x y]
    have h : (isNaN F x || isNaN F y || isInf F x || isInf F y) = (isNaN F y || isNaN F x || isInf F y || isInf F x) := by
      cases isNaN F x <;> cases isNaN F y <;> cases isInf F x <;> cases isInf F y <;> rfl
    rw [h, Bool.and_comm (!sign F x)]
  · unfold mulD
    rw [F32.mul_comm x y, bne_comm, Nat.mul_comm (sig F x) (sig F y), Int.add_comm (ex F x) (ex F y)]
    have h : (isNaN F x || isNaN F y || isInf F x || isInf F y || decide (sig F x = 0) || decide (sig F y = 0)) =
        (isNaN F y || isNaN F x || isInf F y || isInf F x || decide (sig F y = 0) || decide (sig F x = 0)) := by
      cases isNaN F x <;> cases isNaN F y <;> cases isInf F x <;> cases isInf F y <;>
        cases decide (sig F x = 0) <;> cases decide (sig F y = 0) <;> rfl
    rw [h]

/-- 8192 and 1/2 -/
def k8192 : Nat := 0x46000000
def half : Nat := 0x3f000000

/-- GP.FILL.GAMMA.X64: the destination's share 1 - sa / 8192 of a source of alpha word sa -/
def gammaRest (r : Ops) (sa : Nat) : Nat := r.sub one (r.div (F32.ofNat sa) k8192)

/-- a binary32 of a value of 0 or more floored (a negative one 0) -/
def floorNat (x : Nat) : Nat := let (s, n, d) := Float.toRat F x; if s then 0 else n / d

/-- GP.FILL.GAMMA.X64: SourceOver of one 64bppPARGB word, floor((s / 8192 + d / 8192 rest) 8192 +
0.5), kept to 16 bits -/
def mix64 (r : Ops) (s d rest : Nat) : Nat :=
  floorNat (r.add (r.mul (r.add (r.div (F32.ofNat s) k8192) (r.mul (r.div (F32.ofNat d) k8192) rest)) k8192) half) % 65536

/-- GP.FILL.GAMMA.X64: rounding down, an opaque source's word (alpha word 8192) replaces the
destination's, a clear one (alpha word 0) leaves it, at every multiple of 1024 -/
theorem gamma64_ends : ∀ k : Fin 9,
    mix64 down (k.val * 1024) 5000 (gammaRest down 8192) = k.val * 1024 ∧
    mix64 down 0 (k.val * 1024) (gammaRest down 0) = k.val * 1024 := by decide

/-- GP.FILL.GAMMA.X64: a word x86-64 makes one less than rounding to nearest would: 105 over 4833
under an alpha word of 289 -/
theorem gamma64_down :
    mix64 down 105 4833 (gammaRest down 289) = 4767 ∧ mix64 nearest 105 4833 (gammaRest nearest 289) = 4768 := by
  decide

private def bits (l : List Int) : Option (List Nat) := l.mapM nat
private def out (l : List Nat) : List Int := l.map fun v => (F32.canon v : Int)

def mirrors : List Mirror := [
  ⟨"gp.round", fun
    | [op, d, x, y] => do
      let r := ops (d != 0)
      let f ← match op with | 0 => some r.add | 1 => some r.sub | 2 => some r.mul | 3 => some r.div | _ => none
      pure (out [f (← nat x) (← nat y)])
    | _ => none⟩,
  ⟨"gp.matmul", fun
    | d :: l => do
      let v ← bits l
      if v.length ≠ 12 then none else pure (out (matMul (ops (d != 0)) (v.take 6) (v.drop 6)))
    | _ => none⟩,
  ⟨"gp.matpoint", fun
    | [d, a, b, c, e, f, g, x, y] => do
      pure (out (point (ops (d != 0)) (← bits [a, b, c, e, f, g]) (← nat x) (← nat y)))
    | _ => none⟩,
  ⟨"gp.gamma64", fun
    | [d, s, w, sa] => do
      let r := ops (d != 0)
      pure [mix64 r (← nat s) (← nat w) (gammaRest r (← nat sa))]
    | _ => none⟩,
  ⟨"gp.matinv", fun
    | d :: l => do
      let v ← bits l
      if v.length ≠ 6 then none else pure (((inverse (ops (d != 0)) v).map out).getD [])
    | _ => none⟩
]

end Kikizake.GdiPlus.Matrix
