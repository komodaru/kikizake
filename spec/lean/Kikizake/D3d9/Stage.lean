import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's texture stage operations (D3DTEXTUREOP, spec/d3d9draw.txt D3D9.STAGE.OP,
STAGE.PASTOP), one channel in binary32 (Kikizake.F32) as src/model/d3d9draw.rs `stage_value`
computes it, then saturated to 0..1 (mirror `d3d9.stage`). -/
namespace Kikizake.D3d9.Stage

open Kikizake.F32 (add sub mul clamp)

def zero : Nat := 0
def half : Nat := 0x3f000000
def one : Nat := 0x3f800000
def two : Nat := 0x40000000
def four : Nat := 0x40800000

/-- one channel's inputs: the operation, the stage, whether it is the alpha operation, the
arguments (bits), the first argument's alpha, the diffuse, current, texture and texture factor
alphas, the channel's current value -/
structure In where
  op : Nat
  st : Nat
  alpha : Bool
  a0 : Nat
  a1 : Nat
  a2 : Nat
  a1a : Nat
  da : Nat
  ca : Nat
  ta : Nat
  fa : Nat
  cur : Nat

/-- x a + y (1 - a) -/
def lerp (x y a : Nat) : Nat := add (mul x a) (mul y (sub one a))

/-- D3D9.STAGE.OP, STAGE.PASTOP: the value before it is saturated; none for DISABLE (1), the bump
mapping and DOTPRODUCT3 operations as color ones (22..24) and 0 -/
def value (x : In) : Option Nat :=
  match x.op with
  | 2 | 17 => some x.a1
  | 3 => some x.a2
  | 4 => some (mul x.a1 x.a2)
  | 5 => some (mul (mul x.a1 x.a2) two)
  | 6 => some (mul (mul x.a1 x.a2) four)
  | 7 => some (add x.a1 x.a2)
  | 8 => some (sub (add x.a1 x.a2) half)
  | 9 => some (mul (sub (add x.a1 x.a2) half) two)
  | 10 => some (sub x.a1 x.a2)
  | 11 => some (sub (add x.a1 x.a2) (mul x.a1 x.a2))
  | 12 => some (lerp x.a1 x.a2 x.da)
  | 13 => some (lerp x.a1 x.a2 x.ta)
  | 14 => some (lerp x.a1 x.a2 x.fa)
  | 15 => some (add x.a1 (mul x.a2 (sub one x.ta)))
  | 16 => some (lerp x.a1 x.a2 x.ca)
  | n =>
    if 18 ≤ n ∧ n ≤ 24 ∧ x.alpha then some (if x.st = 0 then one else x.ca)
    else match n with
    | 18 => some (add x.a1 (mul x.a1a x.a2))
    | 19 => some (add (mul x.a1 x.a2) x.a1a)
    | 20 => some (add (mul (sub one x.a1a) x.a2) x.a1)
    | 21 => some (add (mul (sub one x.a1) x.a2) x.a1a)
    | 25 => some (add x.a0 (mul x.a1 x.a2))
    | 26 => some (add (mul x.a0 x.a1) (mul (sub one x.a0) x.a2))
    | n =>
      if 27 ≤ n then
        if x.st = 0 then (if !x.alpha then some zero else if n = 32 then some one else some x.da)
        else if !x.alpha then some x.cur else some x.ca
      else none

/-- the channel written: the value saturated to 0..1 -/
def out (x : In) : Option Nat := (value x).map fun v => clamp v zero one

/-- D3D9.STAGE.OP: MODULATE, MODULATE2X, MODULATE4X, ADD, ADDSIGNED, ADDSIGNED2X, ADDSMOOTH and
MULTIPLYADD's product give the same whichever way round the two arguments are -/
theorem symmetric (x : In) (h : x.op ∈ [4, 5, 6, 7, 8, 9, 11, 25]) :
    value x = value { x with a1 := x.a2, a2 := x.a1 } := by
  simp only [List.mem_cons, List.mem_nil_iff, or_false] at h
  rcases h with h | h | h | h | h | h | h | h <;>
    simp [value, h, Kikizake.F32.mul_comm x.a1 x.a2, Kikizake.F32.add_comm x.a1 x.a2]

/-- D3D9.STAGE.OP: SELECTARG1 (and PREMODULATE) gives its first argument, SELECTARG2 its second;
LERP with a first argument of 1 is BLENDDIFFUSEALPHA's formula with that alpha -/
theorem selects (x : In) :
    value { x with op := 2 } = some x.a1 ∧ value { x with op := 17 } = some x.a1 ∧
    value { x with op := 3 } = some x.a2 := by
  simp [value]

/-- D3D9.STAGE.PASTOP: an operation past LERP makes black as the color operation on stage 0, 1 as
alpha operation 32 there and the diffuse alpha for the others; on a later stage it leaves the
current value -/
theorem past_lerp (x : In) (h : 27 ≤ x.op) :
    value x = some (if x.st = 0 then (if !x.alpha then zero else if x.op = 32 then one else x.da)
      else if !x.alpha then x.cur else x.ca) := by
  have h1 : ¬(18 ≤ x.op ∧ x.op ≤ 24 ∧ x.alpha) := by omega
  unfold value
  split <;> (try omega)
  all_goals (simp only [h1, ↓reduceIte]; split <;> (try omega); simp only [h, ↓reduceIte]; split <;> split <;> (try split) <;> rfl)

/-- D3D9.STAGE.OP: the color-only operations (the premodulated and bump mapping ones and
DOTPRODUCT3) as alpha operations give 1 on stage 0 and the current alpha after it -/
theorem color_only_alpha (x : In) (h : 18 ≤ x.op ∧ x.op ≤ 24) (ha : x.alpha = true) :
    value x = some (if x.st = 0 then one else x.ca) := by
  have h1 : 18 ≤ x.op ∧ x.op ≤ 24 ∧ x.alpha := ⟨h.1, h.2, ha⟩
  unfold value
  split <;> (try omega)
  simp [h1]

def mirrors : List Mirror := [
  ⟨"d3d9.stage", fun
    | [op, st, al, a0, a1, a2, a1a, da, ca, ta, fa, cur] => do
      let x : In := ⟨← nat op, ← nat st, al != 0, ← nat a0, ← nat a1, ← nat a2, ← nat a1a, ← nat da,
        ← nat ca, ← nat ta, ← nat fa, ← nat cur⟩
      pure ((out x).map (fun v => (v : Int))).toList
    | _ => none⟩
]

end Kikizake.D3d9.Stage
