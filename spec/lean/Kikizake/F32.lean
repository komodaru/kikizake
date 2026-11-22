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

/-- binary32 multiplication is commutative, NaNs and infinities too -/
theorem mul_comm (x y : Nat) : mul x y = mul y x := Float.mul_comm fmt x y
/-- binary32 addition is commutative, NaNs, infinities and signed zeros too -/
theorem add_comm (x y : Nat) : add x y = add y x := Float.add_comm fmt x y

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

theorem mul_comm (x y : Nat) : mul x y = mul y x := Float.mul_comm fmt x y
theorem add_comm (x y : Nat) : add x y = add y x := Float.add_comm fmt x y

/-- a binary64 from its halves, and its halves -/
def join (hi lo : Nat) : Nat := hi * 2 ^ 32 + lo
def halves (x : Nat) : List Int := [(x / 2 ^ 32 : Nat), (x % 2 ^ 32 : Nat)]

end Kikizake.F64

namespace Kikizake.F32

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
