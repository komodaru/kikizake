import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Raster
/-! Direct3D 9's blending on targets of other than 8-bit unsigned channels (D3D9.BLEND.FORMAT),
as src/model/d3d9draw.rs `blend_float` computes it: in binary32, the source clamped to 0..1, the
factors `factorF` `factor_f32` (mirror `d3d9.blendf`). -/
namespace Kikizake.D3d9.Blend

open Kikizake.D3d9.Pixel (Op)
open Kikizake.D3d9.Raster (blendOp bothAlpha)

def one : Nat := 0x3f800000

/-- Rust `f32::min`, `max` as the model's build computes them (x86-64 MINSS, MAXSS): a NaN gives
the other, and of two equal ones (zeros of either sign) the second -/
def fmin (a b : Nat) : Nat :=
  if F32.isNaN a then b else if F32.isNaN b then a else if F32.lt a b then a else b
def fmax (a b : Nat) : Nat :=
  if F32.isNaN a then b else if F32.isNaN b then a else if F32.lt b a then a else b

/-- four channels (binary32 bits): red, green, blue, alpha -/
abbrev C4 := List Nat

/-- a factor (D3DBLEND) of channel k: of the source s, the destination d, the BLENDFACTOR color's
channels bf; none outside the enumeration and for the BOTH ones -/
def factorF (f k : Nat) (s d bf : C4) : Option Nat := do
  let (sk, dk, bk, sa, da) := (← s[k]?, ← d[k]?, ← bf[k]?, ← s[3]?, ← d[3]?)
  match f with
  | 1 => some 0
  | 2 => some one
  | 3 => some sk
  | 4 => some (F32.sub one sk)
  | 5 => some sa
  | 6 => some (F32.sub one sa)
  | 7 => some da
  | 8 => some (F32.sub one da)
  | 9 => some dk
  | 10 => some (F32.sub one dk)
  | 11 => some (if k < 3 then fmin sa (F32.sub one da) else one)
  | 14 => some bk
  | 15 => some (F32.sub one bk)
  | _ => none

/-- what blending reads of the render states -/
structure States where
  src : Nat
  dst : Nat
  op : Nat
  separate : Bool
  srcA : Nat
  dstA : Nat
  opA : Nat
  factor : Nat

/-- the BLENDFACTOR color's channel k / 255 in binary32 -/
def unit (k : Nat) : Nat := F32.div (F32.ofNat k) 0x437f0000
def bfOf (c : Nat) : C4 := [unit ((c >>> 16) % 256), unit ((c >>> 8) % 256), unit (c % 256), unit ((c >>> 24) % 256)]

/-- the operation on the source sk, the destination dk and their products with the factors x, y -/
def combine (op : Op) (sk dk x y : Nat) : Nat :=
  match op with
  | .add => F32.add x y
  | .sub => F32.sub x y
  | .revsub => F32.sub y x
  | .min => fmin sk dk
  | .max => fmax sk dk

/-- one channel k of the blend of the clamped source s and the destination d -/
def channel (s d : C4) (b : States) (k : Nat) : Option Nat := do
  let alpha := b.separate && k == 3
  let (sf, df, op) := if alpha then (b.srcA, b.dstA, b.opA) else (b.src, b.dst, b.op)
  let (sf, df) := bothAlpha sf df alpha
  let (sk, dk) := (← s[k]?, ← d[k]?)
  let x := F32.mul sk (← factorF sf k s d (bfOf b.factor))
  let y := F32.mul dk (← factorF df k s d (bfOf b.factor))
  pure (combine (blendOp op) sk dk x y)

/-- the source's channels (binary64) clamped to 0..1 and taken to binary32, the destination's
taken to binary32 -/
def src32 (s : List Nat) : C4 := s.map fun x => F64.toF32 (F64.clamp x 0 0x3ff0000000000000)
def dst32 (d : List Nat) : C4 := d.map F64.toF32

/-- the blended color, binary64 -/
def blendFloat (s d : List Nat) (b : States) : Option (List Nat) :=
  [0, 1, 2, 3].mapM fun k => do pure (F64.ofF32 (← channel (src32 s) (dst32 d) b k))

/-- D3D9.BLEND.FORMAT: the source is clamped to 0..1 (below 0 is 0, past 1 is 1, a source in 0..1
blends as it is), the destination taken as read; the products with the factors added,
subtracted either way, and MIN and MAX of the source and destination alone, whatever the factors;
the alpha blends by the separate states exactly where they are on; a factor outside the
enumeration (0, 16) blends nothing; the blended color is the four channels of the clamped source
and the destination -/
theorem blend_format (s d : C4) (b : States) (k x : Nat) :
    (blendFloat s d b = [0, 1, 2, 3].mapM fun k => do pure (F64.ofF32 (← channel (src32 s) (dst32 d) b k))) ∧
    (F64.lt x 0 = true → Float.isNaN F64.fmt x = false → F64.clamp x 0 0x3ff0000000000000 = 0) ∧
    (F64.lt x 0 = false → F64.lt 0x3ff0000000000000 x = true → Float.isNaN F64.fmt x = false →
      F64.clamp x 0 0x3ff0000000000000 = 0x3ff0000000000000) ∧
    (F64.lt x 0 = false → F64.lt 0x3ff0000000000000 x = false → F64.clamp x 0 0x3ff0000000000000 = x) ∧
    (∀ sk dk p q, combine .add sk dk p q = F32.add p q ∧ combine .sub sk dk p q = F32.sub p q ∧
      combine .revsub sk dk p q = F32.sub q p ∧ combine .min sk dk p q = fmin sk dk ∧ combine .max sk dk p q = fmax sk dk) ∧
    (channel s d { b with separate := true } 3 = channel s d { b with separate := true, src := b.srcA, dst := b.dstA, op := b.opA } 3) ∧
    (k ≠ 3 → channel s d { b with separate := true } k = channel s d { b with separate := false } k) ∧
    (∀ bf, factorF 0 k s d bf = none ∧ factorF 12 k s d bf = none ∧ factorF 13 k s d bf = none ∧ factorF 16 k s d bf = none) := by
  refine ⟨rfl, fun h hn => ?_, fun h1 h2 hn => ?_, fun h1 h2 => ?_, fun _ _ _ _ => ⟨rfl, rfl, rfl, rfl, rfl⟩, ?_, fun hk => ?_, fun bf => ?_⟩
  · simp [F64.clamp, Float.clamp, hn, show F64.lt x 0 = true from h]
  · simp [F64.clamp, Float.clamp, hn, show F64.lt x 0 = false from h1, show F64.lt 0x3ff0000000000000 x = true from h2]
  · simp only [F64.clamp, Float.clamp, show F64.lt x 0 = false from h1, show F64.lt 0x3ff0000000000000 x = false from h2]
    simp
  · simp [channel]
  · have : (k == 3) = false := by simpa using hk
    simp [channel, this]
  · simp only [factorF]
    refine ⟨?_, ?_, ?_, ?_⟩ <;> cases s[k]? <;> cases d[k]? <;> cases bf[k]? <;> cases s[3]? <;> cases d[3]? <;> rfl

def c4 (l : List Int) (i : Nat) : Option (List Nat) :=
  [0, 1, 2, 3].mapM fun k => do F64.join (← nat (← l[i + 2 * k]?)) (← nat (← l[i + 2 * k + 1]?))

def mirrors : List Mirror := [
  ⟨"d3d9.blendf", fun l => do
    let s ← c4 l 0
    let d ← c4 l 8
    let g (i : Nat) : Option Nat := do nat (← l[i]?)
    let b : States := ⟨← g 16, ← g 17, ← g 18, (← l[19]?) != 0, ← g 20, ← g 21, ← g 22, ← g 23⟩
    match blendFloat s d b with
    | some v => pure (v.flatMap fun x => F64.halves (F64.canon x))
    | none => pure []⟩
]

end Kikizake.D3d9.Blend
