import Kikizake.Mirror
/-! Direct3D 10 and 11's CreateInputLayout against the generator's programs (spec/d3dres.txt
D3DRASTER.LAYOUT.BAD, LAYOUT.SIG) as src/model/d3draster.rs decides it: `verdict` is
`layout_verdict` (mirror `d3d11.layout`). A program is its index in `LAYOUT_PROGRAMS`: 0 vs
(two inputs), 1 vsid (one), 2 bad (bytes that are no program), 3 to 10 the pixel programs;
any other none. An element is whether its format is one an input takes. -/
namespace Kikizake.D3d11.Layout

/-- made, no program, a pixel program, an input unfed -/
inductive Verdict where
  | made
  | noProgram
  | pixelProgram
  | unfed
  deriving DecidableEq

def Verdict.code : Verdict → Int
  | .made => 0
  | .noProgram => 1
  | .pixelProgram => 2
  | .unfed => 3

/-- D3DRASTER.LAYOUT.BAD, LAYOUT.SIG: bytes that are no program and a pixel program are
refused whatever the elements; a vertex program takes elements enough for its inputs, each of
a format an input takes -/
def verdict (program : Nat) (usable : List Bool) : Option Verdict :=
  let fed := fun needs => if needs ≤ usable.length ∧ usable.all id then Verdict.made else Verdict.unfed
  if program = 0 then some (fed 2)
  else if program = 1 then some (fed 1)
  else if program = 2 then some .noProgram
  else if program ≤ 10 then some .pixelProgram
  else none

/-! ## theorems -/

/-- D3DRASTER.LAYOUT.BAD: a layout against bytes that are no program is refused, whatever its
elements -/
theorem no_program (usable : List Bool) : verdict 2 usable = some .noProgram := rfl

/-- D3DRASTER.LAYOUT.SIG: a layout against a pixel program is refused, whatever its elements -/
theorem pixel_program (p : Nat) (h3 : 3 ≤ p) (h10 : p ≤ 10) (usable : List Bool) :
    verdict p usable = some .pixelProgram := by
  simp [verdict, show p ≠ 0 by omega, show p ≠ 1 by omega, show p ≠ 2 by omega, h10]

/-- a layout is made only against a vertex program -/
theorem made_vertex (p : Nat) (usable : List Bool) (h : verdict p usable = some .made) : p ≤ 1 := by
  rcases Nat.lt_or_ge p 2 with hp | hp
  · omega
  · simp only [verdict, show p ≠ 0 by omega, show p ≠ 1 by omega, ite_false] at h
    split at h <;> simp at h

def mirrors : List Mirror := [
  ⟨"d3d11.layout", fun
    | p :: us => do pure [match verdict (← nat p) (us.map (· != 0)) with | none => -1 | some v => v.code]
    | _ => none⟩
]

end Kikizake.D3d11.Layout
