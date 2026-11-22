import Kikizake.GdiPlus.Linear
/-! GDI+'s color matrix (spec/gdiplus.txt GP.DRAW.MATRIX) as src/model/gdiplus.rs `recolor`
computes it under ColorMatrixFlagsDefault: the straight color's channels [r g b a 255] times the
matrix in binary32 (each sum from -0, to nearest on both arches, as the model's own f32 sum),
clamped to 0 ..= 255, plus 1/2, truncated; mirror `gp.recolor`. -/
namespace Kikizake.GdiPlus.Recolor
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Linear

private abbrev F := F32.fmt

/-- 255 -/
def k255 : Nat := 0x437f0000

/-- the channel of column j: the products of v and column j of m summed in order -/
def column (v m : List Nat) (j : Nat) : Nat :=
  let s := (List.range 5).foldl (fun acc i => F32.add acc (F32.mul (v.getD i 0) (m.getD (i * 5 + j) 0))) 0x80000000
  floorU (F32.add (F32.clamp s 0 k255) half)

/-- GP.DRAW.MATRIX: a straight ARGB color through a 5 x 5 matrix, row by row -/
def recolor (c : Nat) (m : List Nat) : Nat :=
  let ch (k : Nat) := F32.ofNat (c / 2 ^ (8 * k) % 256)
  let v := [ch 2, ch 1, ch 0, ch 3, k255]
  column v m 3 * 2 ^ 24 + column v m 0 * 2 ^ 16 + column v m 1 * 2 ^ 8 + column v m 2

/-- the identity -/
def ident : List Nat := (List.range 25).map fun i => if i % 6 = 0 then one else 0

/-- GP.DRAW.MATRIX: the identity keeps a color; halving every channel rounds 255 / 2 up; a
translation of 1 in red saturates it; a NaN gives 0 -/
theorem examples :
    recolor 0x80ff4020 ident = 0x80ff4020 ∧
    recolor 0xffffffff ((List.range 25).map fun i => if i = 24 then one else if i % 6 = 0 then half else 0) = 0x80808080 ∧
    recolor 0x00000000 (ident.set 20 one) = 0x00ff0000 ∧
    recolor 0x12345678 (ident.set 0 0x7fc00000) = 0x12005678 := by decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.recolor", fun
    | c :: m => do
      let m ← m.mapM nat
      if m.length = 25 then pure [recolor (← nat c) m] else none
    | _ => none⟩
]

end Kikizake.GdiPlus.Recolor
