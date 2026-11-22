import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
/-! Direct3D 10 and 11's choice of filter and level (spec/d3dres.txt D3DRASTER.FILTER.MINMAG,
FILTER.LEVEL, FILTER.ANISO, NOSAMPLER) as src/model/d3draster.rs computes it: `quadSpan` is
`quad_span` in binary64, `filterPick` its `filter_pick`, `filterLinear` its `filter_linear`,
`samplerOrDefault` its `sampler_or_default` (mirrors `d3d11.quadspan`, `d3d11.filterpick`,
`d3d11.filterlinear`, `d3d11.sampler`). The level's lod is f64::log2 of the span, which the
model takes from the platform's libm: `filterPick` takes it as an argument, as the mirror
`d3d11.filterpick` takes the model's. -/
namespace Kikizake.D3d11.Lod

open Kikizake.Float (isNaN)
open Kikizake.D3d11.Filter (level)

/-- D3DRASTER.NOSAMPLER: with no sampler bound the default one: linear filtering (0x15),
clamped addresses (3), comparison never (1) -/
def samplerOrDefault : Option (List Nat) → List Nat
  | some s => s
  | none => [0x15, 3, 3, 3, 1]

/-- D3DRASTER.FILTER.ANISO, MINMAG: a filter's minifying and magnifying filters (linear or
point); anisotropic (0x55) is linear for both -/
def filterLinear (f : Nat) : Option (Bool × Bool) :=
  if f = 0x00 ∨ f = 0x01 then some (false, false)
  else if f = 0x04 ∨ f = 0x05 then some (false, true)
  else if f = 0x10 ∨ f = 0x11 then some (true, false)
  else if f = 0x14 ∨ f = 0x15 ∨ f = 0x55 then some (true, true)
  else none

/-- f64::max: a NaN gives the other, else the larger (the second when they compare equal) -/
def maxD (x y : Nat) : Nat :=
  if isNaN F64.fmt x then y else if isNaN F64.fmt y then x else if F64.lt y x then x else y

/-- one of the quad's spans: sqrt((du w)^2 + (dv h)^2) in f64, du and dv the f32 differences -/
def span (w h : Nat) (a b : Nat × Nat) : Nat :=
  let du := F64.mul (F64.ofF32 (F32.sub b.1 a.1)) (Float.ofNat F64.fmt w)
  let dv := F64.mul (F64.ofF32 (F32.sub b.2 a.2)) (Float.ofNat F64.fmt h)
  F64.sqrt (F64.add (F64.mul du du) (F64.mul dv dv))

/-- D3DRASTER.FILTER.MINMAG, LEVEL, ANISO: the texels a quad spans, the longer of its spans
along x (the corner to its right neighbour) and along y (to its lower one) -/
def quadSpan (q0 q1 q2 : Nat × Nat) (w h : Nat) : Nat := maxD (span w h q0 q1) (span w h q0 q2)

/-- the binary64 1 -/
def one64 : Nat := 0x3ff0000000000000

/-- the magnifying filter, the minifying one, or the minifying one at the lod's levels -/
inductive Pick where
  | mag
  | min
  | mip (l : Int × Option Int)

/-- D3DRASTER.FILTER.MINMAG, LEVEL: a span of up to one texel magnifies, past it minifies; with
mips at the level of log2rho (f64::log2 of the span, the platform's libm) -/
def filterPick (mips : Nat) (point : Bool) (rho log2rho : Nat) : Pick :=
  if !isNaN F64.fmt rho && !F64.lt one64 rho then .mag
  else if mips ≤ 1 then .min
  else .mip (level log2rho (mips - 1) point)

/-! ## theorems -/

/-- D3DRASTER.NOSAMPLER: the default sampler filters linearly, minifying and magnifying, with
clamped addresses -/
theorem default_linear : filterLinear ((samplerOrDefault none).getD 0 0) = some (true, true) ∧
    (samplerOrDefault none).drop 1 = [3, 3, 3, 1] := by decide

/-- D3DRASTER.FILTER.ANISO: anisotropic filtering is the linear filter -/
theorem aniso_linear : filterLinear 0x55 = filterLinear 0x15 := by decide

/-- D3DRASTER.FILTER.MINMAG: a span of up to one texel magnifies -/
theorem up_to_one (mips : Nat) (point : Bool) (rho l : Nat) (h : isNaN F64.fmt rho = false)
    (hle : F64.lt one64 rho = false) : filterPick mips point rho l = .mag := by
  simp [filterPick, h, hle]

/-- D3DRASTER.FILTER.MINMAG: past one texel with one level, minifies -/
theorem past_one (point : Bool) (rho l : Nat) (h : F64.lt one64 rho = true) :
    filterPick 1 point rho l = .min := by
  simp [filterPick, h]

/-- D3DRASTER.FILTER.LEVEL: past one texel with mips, the level of the lod -/
theorem past_one_mips (mips : Nat) (hm : 2 ≤ mips) (point : Bool) (rho l : Nat) (h : F64.lt one64 rho = true) :
    filterPick mips point rho l = .mip (level l (mips - 1) point) := by
  simp [filterPick, h, show ¬ mips ≤ 1 by omega]

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- D3DRASTER.FILTER.MINMAG: a quad spanning a texel and a half along x, of a 4 x 4 texture
(coordinates 0, 3/8, 0) -/
theorem span_example : quadSpan (0, 0) (0x3ec00000, 0) (0, 0) 4 4 = 0x3ff8000000000000 := by decide

def mirrors : List Mirror := [
  ⟨"d3d11.sampler", fun
    | [0] => pure ((samplerOrDefault none).map Int.ofNat)
    | [1, a, b, c, d, e] => do pure ((samplerOrDefault (some [← nat a, ← nat b, ← nat c, ← nat d, ← nat e])).map Int.ofNat)
    | _ => none⟩,
  ⟨"d3d11.filterlinear", fun
    | [f] => do pure (match filterLinear (← nat f) with
      | none => [-1]
      | some (mn, mg) => [if mn then 1 else 0, if mg then 1 else 0])
    | _ => none⟩,
  ⟨"d3d11.quadspan", fun
    | [a0, a1, b0, b1, c0, c1, w, h] => do
      pure (F64.halves (F64.canon (quadSpan (← nat a0, ← nat a1) (← nat b0, ← nat b1) (← nat c0, ← nat c1) (← nat w) (← nat h))))
    | _ => none⟩,
  ⟨"d3d11.filterpick", fun
    | [mips, point, rh, rl, lh, ll] => do
      pure (match filterPick (← nat mips) (point != 0) (F64.join (← nat rh) (← nat rl)) (F64.join (← nat lh) (← nat ll)) with
      | .mag => [0]
      | .min => [1]
      | .mip (n, none) => [2, 0, n]
      | .mip (n, some w) => [2, 1, n, w])
    | _ => none⟩
]

end Kikizake.D3d11.Lod
