import Kikizake.Mirror
/-! DirectWrite's GDI-compatible advances in whole pixels (spec/dwrite.txt DW.GDI.GLYPH.H,
DW.SHAPE.GDIPLACE, DW.SHAPE.NATPLACE), as src/model/dwrite.rs computes them: `phantom` is
`phantom_px` (the scaler's advance phantom point, by step), `classic` `classic_px` (mirrors
`dw.phantom`, `dw.classic`). -/
namespace Kikizake.DWrite.Advance

/-- the advance phantom point: adv design units at ppem rounded half away from zero to 16.16,
then to 26.6, then to a 1/16 pixel (`sixteenth`) or not, then to the pixel, half up -/
def phantom (adv ppem upm : Int) (sixteenth : Bool) : Int :=
  let n := adv * (ppem * 65536)
  let a16 := if n ≥ 0 then (n + upm / 2) / upm else -((upm / 2 - n) / upm)
  let a26 := (a16 + 512) / 1024
  let q := if sixteenth then (a26 + 2) / 4 * 4 else a26
  (q + 32) / 64

/-- GDI classic: below 256 ppem the 16.16 truncated scale's advance, past it the phantom
point's to the pixel -/
def classic (adv ppem upm : Int) : Int :=
  if ppem ≤ 255 then (adv * (ppem * 65536 / upm) + 32768) / 65536 else phantom adv ppem upm false

private theorem div_mono {a b : Int} (c : Int) (hc : 0 < c) (h : a ≤ b) : a / c ≤ b / c :=
  Int.ediv_le_ediv hc h

/-- DW.GDI.GLYPH.H, DW.SHAPE.NATPLACE: a wider advance is never fewer pixels, either step -/
theorem phantom_mono (adv adv' ppem upm : Int) (s : Bool) (h0 : 0 ≤ adv) (h : adv ≤ adv') (hp : 0 ≤ ppem) (hu : 0 < upm) :
    phantom adv ppem upm s ≤ phantom adv' ppem upm s := by
  have n0 : 0 ≤ adv * (ppem * 65536) := Int.mul_nonneg h0 (by omega)
  have n1 : adv * (ppem * 65536) ≤ adv' * (ppem * 65536) := Int.mul_le_mul_of_nonneg_right h (by omega)
  unfold phantom
  simp only [show adv * (ppem * 65536) ≥ 0 from n0, show adv' * (ppem * 65536) ≥ 0 by omega, ↓reduceIte]
  apply div_mono 64 (by decide)
  have a := div_mono upm hu (Int.add_le_add_right n1 (upm / 2))
  have b := div_mono 1024 (by decide) (Int.add_le_add_right a 512)
  cases s
  · simp only [Bool.false_eq_true, ↓reduceIte]; omega
  · simp only [↓reduceIte]
    have c := div_mono 4 (by decide) (Int.add_le_add_right b 2)
    omega

/-- DW.SHAPE.NATPLACE, DW.GDI.GLYPH: the 1/16 pixel step moves the advance by at most a pixel -/
theorem sixteenth_near (adv ppem upm : Int) : phantom adv ppem upm true - phantom adv ppem upm false ≤ 1 ∧
    phantom adv ppem upm false - phantom adv ppem upm true ≤ 1 := by
  unfold phantom
  simp only [↓reduceIte, Bool.false_eq_true]
  generalize (if adv * (ppem * 65536) ≥ 0 then (adv * (ppem * 65536) + upm / 2) / upm else -((upm / 2 - adv * (ppem * 65536)) / upm)) = a16
  omega

/-- DW.GDI.GLYPH.H, DW.SHAPE.GDIPLACE, NATPLACE: an advance of one em is the em's pixels, in
every mode (design units per em up to 32768) -/
theorem em_is_ppem (ppem upm : Int) (s : Bool) (hp : 0 ≤ ppem) (hu : 0 < upm) (hu' : upm ≤ 32768) :
    phantom upm ppem upm s = ppem ∧ classic upm ppem upm = ppem := by
  have exact : (upm * (ppem * 65536) + upm / 2) / upm = ppem * 65536 := by
    rw [Int.add_comm, Int.add_mul_ediv_left _ _ (by omega), Int.ediv_eq_zero_of_lt (by omega) (by omega)]
    omega
  have ph : ∀ s, phantom upm ppem upm s = ppem := by
    intro s
    unfold phantom
    simp only [show upm * (ppem * 65536) ≥ 0 from Int.mul_nonneg (by omega) (by omega), ↓reduceIte, exact]
    cases s <;> simp <;> omega
  refine ⟨ph s, ?_⟩
  unfold classic
  split
  · have q := Int.emod_add_mul_ediv (ppem * 65536) upm
    have r0 := Int.emod_nonneg (ppem * 65536) (by omega : upm ≠ 0)
    have r1 := Int.emod_lt_of_pos (ppem * 65536) hu
    generalize ppem * 65536 / upm = k at q ⊢
    generalize (ppem * 65536) % upm = r at q r0 r1
    omega
  · exact ph false

def mirrors : List Mirror := [
  ⟨"dw.phantom", fun | [adv, ppem, upm, s] => if upm ≤ 0 then none else some [phantom adv ppem upm (s != 0)] | _ => none⟩,
  ⟨"dw.classic", fun | [adv, ppem, upm] => if upm ≤ 0 then none else some [classic adv ppem upm] | _ => none⟩
]

end Kikizake.DWrite.Advance
