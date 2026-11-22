import Kikizake.Mirror
/-! GDI+'s dither place of a gamma corrected texture fill on 16bppRGB555/565 (spec/gdiplus.txt
GP.PAINT.GAMMA.RUNS) as src/model/gdiplus.rs `dither_column` and `dither_row` compute it: the runs
of opaque and of translucent source pixels in a row; mirror `gp.dither_place`. -/
namespace Kikizake.GdiPlus.Dither

/-- a row's source pixels: none where the fill leaves the pixel out, else whether it is opaque -/
abbrev Row := List (Option Bool)

def at_ (row : Row) (i : Int) : Option Bool := if i < 0 then none else (row.getD i.toNat none)

/-- how far a run of o goes down from a (exclusive of the end lo) -/
def runDown : Nat → Row → Option Bool → Int → Int → Int
  | 0, _, _, _, a => a
  | fuel + 1, row, o, lo, a => if a > lo ∧ at_ row (a - 1) = o then runDown fuel row o lo (a - 1) else a
def runUp : Nat → Row → Option Bool → Int → Int → Int
  | 0, _, _, _, a => a
  | fuel + 1, row, o, hi, a => if a < hi ∧ at_ row a = o then runUp fuel row o hi (a + 1) else a
def firstDrawn : Nat → Row → Int → Int → Int
  | 0, _, _, l => l
  | fuel + 1, row, x, l => if l < x ∧ at_ row l = none then firstDrawn fuel row x (l + 1) else l

/-- GP.PAINT.GAMMA.RUNS: a texture's dither column at x of a fill from x0 to x1 (exclusive) on a
bitmap bw wide, the rendering origin at ox: the place in the run plus the row's first drawn pixel
less ox; x86 takes place 2 for a run of one and the last pixel of a run of odd length that is
translucent or ends the fill; a solid brush the bitmap's own column -/
def column (x86 texture : Bool) (ox : Int) (row : Row) (x0 x1 bw x : Int) : Int :=
  if !texture then x else
  let o := at_ row x
  let lo := max x0 0
  let hi := min x1 bw
  let n := row.length + 2
  let a0 := runDown n row o lo x
  let a1 := runUp n row o hi (x + 1)
  let oddEnd := (a1 - a0) % 2 = 1 ∧ x = a1 - 1 ∧ (o = some false ∨ a1 - a0 = 1 ∨ a1 = hi)
  let place := if oddEnd ∧ x86 then 2 else x - a0
  let left := firstDrawn n row x lo
  (place + left - ox) % 4

/-- the dither row: y less the origin's y, modulo 4, for a texture; y for a solid brush -/
def row (texture : Bool) (oy y : Int) : Int := if texture then (y - oy) % 4 else y

/-- GP.PAINT.GAMMA.RUNS: in a row of opaque pixels drawn from 0, x86-64 counts the place from the
run's start; x86 gives a run of one place 2, and the last of a translucent run of 3 too -/
theorem places :
    ([0, 1, 2, 3] : List Int).map (fun x => column false true 0 [some true, some true, some true, some true] 0 4 4 x) = [0, 1, 2, 3] ∧
    column true true 0 [some true, some false, some true] 0 3 3 1 = 2 ∧
    column false true 0 [some true, some false, some true] 0 3 3 1 = 0 ∧
    column true true 0 [some false, some false, some false, some true] 0 4 4 2 = 2 ∧
    column false true 0 [some false, some false, some false, some true] 0 4 4 2 = 2 ∧
    column true true 0 [some false, some false, some false, some true] 0 4 4 1 = 1 ∧
    column true true 1 [none, some true, some true] 0 3 3 2 = 1 ∧ column true false 1 [none, some true] 0 2 2 1 = 1 := by
  decide

private def code : Int → Option (Option Bool)
  | 0 => some none
  | 1 => some (some false)
  | 2 => some (some true)
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.dither_place", fun
    | x86 :: tex :: ox :: oy :: x0 :: x1 :: x :: y :: cells => do
      let row ← cells.mapM code
      pure [column (x86 != 0) (tex != 0) ox row x0 x1 row.length x, Dither.row (tex != 0) oy y]
    | _ => none⟩
]

end Kikizake.GdiPlus.Dither
