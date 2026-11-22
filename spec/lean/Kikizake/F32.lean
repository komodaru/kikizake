import Kikizake.Mirror
import Kikizake.Float
/-! binary32 and binary64 (Kikizake.Float), Rust's `f32` and `f64`: mirrors `f32.add`, `f32.sub`,
`f32.mul`, `f32.div`, `f32.ofNat`, `f32.clamp` and `f64.add`, `f64.sub`, `f64.mul`, `f64.div`,
`f64.ofF32`, `f64.toF32` hold them to Rust's operators bit for bit (a binary64 passed as its two
32-bit halves). -/
namespace Kikizake.F32

def fmt : Float.Fmt := ⟨23, 8⟩

abbrev add := Float.add fmt
abbrev sub := Float.sub fmt
abbrev mul := Float.mul fmt
abbrev div := Float.div fmt
abbrev ofNat := Float.ofNat fmt
abbrev clamp := Float.clamp fmt
abbrev lt := Float.lt fmt
abbrev canon := Float.canon fmt
abbrev isNaN := Float.isNaN fmt


end Kikizake.F32

namespace Kikizake.F64

def fmt : Float.Fmt := ⟨52, 11⟩

abbrev add := Float.add fmt
abbrev sub := Float.sub fmt
abbrev mul := Float.mul fmt
abbrev div := Float.div fmt
abbrev clamp := Float.clamp fmt
abbrev lt := Float.lt fmt
abbrev canon := Float.canon fmt

/-- a binary32 as the binary64 of the same value (exact) -/
def ofF32 (x : Nat) : Nat :=
  if F32.isNaN x then Float.qnan fmt
  else if Float.isInf F32.fmt x then (if Float.sign F32.fmt x then Float.sbit fmt else 0) + Float.inf fmt
  else
    let (s, n, d) := Float.toRat F32.fmt x
    Float.round fmt s n d

/-- a binary64 rounded to the nearest binary32 (Rust `as f32`) -/
def toF32 (x : Nat) : Nat :=
  if Float.isNaN fmt x then Float.qnan F32.fmt
  else if Float.isInf fmt x then (if Float.sign fmt x then Float.sbit F32.fmt else 0) + Float.inf F32.fmt
  else
    let (s, n, d) := Float.toRat fmt x
    Float.round F32.fmt s n d


/-- a binary64 from its halves, and its halves -/
def join (hi lo : Nat) : Nat := hi * 2 ^ 32 + lo
def halves (x : Nat) : List Int := [(x / 2 ^ 32 : Nat), (x % 2 ^ 32 : Nat)]

end Kikizake.F64

/-! (the theorems about Kikizake.Float's operations sit here, beside the mirrors that hold the
binary32 and binary64 instances to Rust) -/
namespace Kikizake.Float

variable (F : Fmt)

/-- multiplication in any format is commutative, NaNs and infinities too -/
theorem fmt_mul_comm (x y : Nat) : mul F x y = mul F y x := by
  unfold mul
  dsimp only
  rw [bne_comm, Nat.mul_comm (sig F x) (sig F y), Int.add_comm (ex F x) (ex F y)]
  by_cases hx : isNaN F x <;> by_cases hy : isNaN F y <;> simp only [hx, hy, Bool.or_true, Bool.or_false, ↓reduceIte, Bool.false_eq_true]
  by_cases ix : isInf F x <;> by_cases iy : isInf F y <;> simp [ix, iy, Bool.or_comm]

private theorem addExact_comm (x y : Nat) : addExact F x y = addExact F y x := by
  unfold addExact
  dsimp only
  rw [Int.min_comm (ex F x) (ex F y), Int.add_comm ((if sign F x = true then -1 else 1) * _)]

/-- addition in any format is commutative, NaNs, infinities and signed zeros too -/
theorem fmt_add_comm (x y : Nat) : add F x y = add F y x := by
  unfold add
  rw [addExact_comm F x y]
  by_cases hx : isNaN F x <;> by_cases hy : isNaN F y <;> simp only [hx, hy, Bool.or_true, Bool.or_false, ↓reduceIte, Bool.false_eq_true]
  by_cases ix : isInf F x <;> by_cases iy : isInf F y <;> simp only [ix, iy, Bool.and_true, Bool.and_false, ↓reduceIte, Bool.false_eq_true]
  · by_cases h : sign F x = sign F y
    · simp [h]
    · have : (sign F y == sign F x) = false := by simp; exact fun e => h e.symm
      simp [h, this]
  · rw [Bool.and_comm]

end Kikizake.Float

namespace Kikizake.F64

theorem mul_comm (x y : Nat) : mul x y = mul y x := Float.fmt_mul_comm fmt x y
theorem add_comm (x y : Nat) : add x y = add y x := Float.fmt_add_comm fmt x y

end Kikizake.F64

namespace Kikizake.F32

/-- binary32 multiplication is commutative, NaNs and infinities too -/
theorem mul_comm (x y : Nat) : mul x y = mul y x := Float.fmt_mul_comm fmt x y
/-- binary32 addition is commutative, NaNs, infinities and signed zeros too -/
theorem add_comm (x y : Nat) : add x y = add y x := Float.fmt_add_comm fmt x y

open Kikizake.F64 (join halves)

def mirrors : List Mirror := [
  ⟨"f32.add", fun | [x, y] => do pure [canon (add (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.sub", fun | [x, y] => do pure [canon (sub (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.mul", fun | [x, y] => do pure [canon (mul (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.div", fun | [x, y] => do pure [canon (div (← nat x) (← nat y))] | _ => none⟩,
  ⟨"f32.ofNat", fun | [n] => do pure [ofNat (← nat n)] | _ => none⟩,
  ⟨"f32.clamp", fun | [x, lo, hi] => do pure [clamp (← nat x) (← nat lo) (← nat hi)] | _ => none⟩,
  ⟨"f64.add", fun | [a, b, c, d] => do pure (halves (F64.canon (F64.add (join (← nat a) (← nat b)) (join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"f64.sub", fun | [a, b, c, d] => do pure (halves (F64.canon (F64.sub (join (← nat a) (← nat b)) (join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"f64.mul", fun | [a, b, c, d] => do pure (halves (F64.canon (F64.mul (join (← nat a) (← nat b)) (join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"f64.div", fun | [a, b, c, d] => do pure (halves (F64.canon (F64.div (join (← nat a) (← nat b)) (join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"f64.ofF32", fun | [x] => do pure (halves (F64.ofF32 (← nat x))) | _ => none⟩,
  ⟨"f64.toF32", fun | [a, b] => do pure [canon (F64.toF32 (join (← nat a) (← nat b)))] | _ => none⟩
]

end Kikizake.F32
