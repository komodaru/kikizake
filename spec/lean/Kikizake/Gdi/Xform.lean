import Kikizake.Mirror
/-! The page transform's rounding (spec/gdixform.txt GDI.MAP.*), as `gdixform::page` computes it
(mirror `gdi.page`). -/
namespace Kikizake.Gdi.Xform

/-- n / d in 28.4 fixed point, halves up -/
def fix (n d : Int) : Int := if d < 0 then (32 * -n + -d) / (2 * -d) else (32 * n + d) / (2 * d)

/-- GDI.MAP.ROUND: one axis: the scaled point and the offset each to 28.4, their sum to the
nearest pixel, halves up (an answer past 32 bits is the largest) -/
def page (a w v wo vo : Int) : Int :=
  let s := fix (a * v) w + fix (vo * w - wo * v) w
  let p := (s + 8) / 16
  if p > 2147483647 ∨ p < -2147483648 then 2147483647 else p

private theorem ediv_bounds (a b : Int) (hb : 0 < b) : b * (a / b) ≤ a ∧ a < b * (a / b) + b := by
  have h := Int.emod_add_mul_ediv a b
  have l := Int.emod_nonneg a (show b ≠ 0 by omega)
  have u := Int.emod_lt_of_pos a hb
  constructor <;> omega

/-- GDI.MAP.ROUND: `fix` is n / d to the nearest sixteenth, halves up: 16 n / d lies in
(f - 1/2, f + 1/2] -/
theorem fix_nearest (n d : Int) (hd : 0 < d) : 2 * d * fix n d ≤ 32 * n + d ∧ 32 * n + d < 2 * d * fix n d + 2 * d := by
  have e : fix n d = (32 * n + d) / (2 * d) := by unfold fix; simp [Int.not_lt.mpr (Int.le_of_lt hd)]
  rw [e]; exact ediv_bounds _ _ (by omega)

/-- a negative divisor divides as the positive one with both signs turned -/
theorem fix_neg (n d : Int) (hd : d < 0) : fix n d = (32 * -n + -d) / (2 * -d) := by
  unfold fix; simp [hd]

/-- GDI.MAP.FORMULA: equal extents and origins at 0 map each point to itself -/
theorem page_identity (a w : Int) (hw : 0 < w) (ha : -134217728 ≤ a ∧ a ≤ 134217727) : page a w w 0 0 = a := by
  have h1 : fix (a * w) w = 16 * a := by
    have b := fix_nearest (a * w) w hw
    have e : 32 * (a * w) = 2 * w * (16 * a) := by grind
    rw [e] at b
    have lo : 16 * a ≤ fix (a * w) w := by
      apply Int.le_of_lt_add_one; apply Int.lt_of_mul_lt_mul_left (a := 2 * w) _ (by omega); rw [Int.mul_add]; omega
    have hi : fix (a * w) w ≤ 16 * a := by
      apply Int.le_of_lt_add_one; apply Int.lt_of_mul_lt_mul_left (a := 2 * w) _ (by omega); rw [Int.mul_add]; omega
    omega
  have h2 : fix (0 * w - 0 * w) w = 0 := by
    unfold fix; simp [Int.not_lt.mpr (Int.le_of_lt hw)]
    exact Int.ediv_eq_zero_of_lt (by omega) (by omega)
  unfold page; simp only [h1, h2]; omega

/-- GDI.MAP.ROUND, GDI.MAP.FORMULA: with a positive window extent the mapped point is within
9/16 of the documented (a - xWO) * xVE / xWE + xVO: 32 w p - 32 E lies in (-18 w, 18 w] for
E = a v + vo w - wo v -/
theorem page_near_formula (a w v wo vo : Int) (hw : 0 < w)
    (hr : -2147483648 ≤ (fix (a * v) w + fix (vo * w - wo * v) w + 8) / 16 ∧ (fix (a * v) w + fix (vo * w - wo * v) w + 8) / 16 ≤ 2147483647) :
    32 * w * page a w v wo vo ≤ 32 * (a * v + vo * w - wo * v) + 18 * w ∧
    32 * (a * v + vo * w - wo * v) - 18 * w < 32 * w * page a w v wo vo := by
  have f1 := fix_nearest (a * v) w hw
  have f2 := fix_nearest (vo * w - wo * v) w hw
  have q := ediv_bounds (fix (a * v) w + fix (vo * w - wo * v) w + 8) 16 (by decide)
  have e : page a w v wo vo = (fix (a * v) w + fix (vo * w - wo * v) w + 8) / 16 := by
    unfold page; simp only; split <;> omega
  rw [e]
  generalize fix (a * v) w = x at *
  generalize fix (vo * w - wo * v) w = y at *
  generalize (x + y + 8) / 16 = p at *
  have m1 := Int.mul_le_mul_of_nonneg_left q.1 (show 0 ≤ 2 * w by omega)
  have m2 := Int.mul_lt_mul_of_pos_left q.2 (show 0 < 2 * w by omega)
  constructor <;> grind

def mirrors : List Mirror := [
  ⟨"gdi.page", fun
    | [a, w, v, wo, vo] => pure [page a w v wo vo]
    | _ => none⟩
]

end Kikizake.Gdi.Xform
