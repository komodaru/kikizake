import Kikizake.Mirror
/-! StretchBlt's COLORONCOLOR sampling (spec/gdi.txt GDI.STRETCH.*), as `gdi::sample` computes it
(mirror `gdi.sample`, every extent 1..64 and every destination pixel). -/
namespace Kikizake.Gdi.Stretch

/-- GDI.STRETCH.DDA: the source pixel of destination pixel `i` of `n` from `sn`; `flat` when the
other direction is unscaled -/
def sample (i sn n : Nat) (flat : Bool) : Nat :=
  if flat && n + 1 == sn then i * sn / n
  else if flat && n == sn + 1 then (i * sn + sn - 1) / n
  else (2 * i + 1) * sn / (2 * n)

/-- the quotient's two bounds, for a positive divisor -/
private theorem div_bounds (a b : Nat) (hb : 0 < b) : b * (a / b) ≤ a ∧ a < b * (a / b + 1) := by
  have h := Nat.div_add_mod a b
  have l := Nat.mod_lt a hb
  constructor
  · omega
  · rw [Nat.mul_add]; omega

/-- GDI.STRETCH.DDA: otherwise the source pixel is the one under the destination pixel's center:
the center (2i + 1) / 2n of the destination lies in source pixel s's span [s / sn, (s + 1) / sn) -/
theorem sample_center (i sn n : Nat) (flat : Bool) (hn : 0 < n)
    (h : ¬ (flat && n + 1 == sn) = true ∧ ¬ (flat && n == sn + 1) = true) :
    2 * n * sample i sn n flat ≤ (2 * i + 1) * sn ∧ (2 * i + 1) * sn < 2 * n * (sample i sn n flat + 1) := by
  have e : sample i sn n flat = (2 * i + 1) * sn / (2 * n) := by
    unfold sample; simp only [h.1, h.2, Bool.false_eq_true, ite_false]
  rw [e]; exact div_bounds _ _ (by omega)

/-- GDI.STRETCH.SAMPLE: every destination pixel takes a pixel inside the source -/
theorem sample_lt (i sn n : Nat) (flat : Bool) (hi : i < n) (hs : 0 < sn) : sample i sn n flat < sn := by
  unfold sample
  split
  · rename_i h
    have hn : 0 < n := by omega
    have : i * sn < n * sn := Nat.mul_lt_mul_of_pos_right hi hs
    have b := div_bounds (i * sn) n hn
    have : n * (i * sn / n) < n * sn := by omega
    exact Nat.lt_of_mul_lt_mul_left this
  · split
    · have hn : 0 < n := by omega
      have : (i + 1) * sn ≤ n * sn := Nat.mul_le_mul_right sn hi
      have b := div_bounds (i * sn + sn - 1) n hn
      have : n * ((i * sn + sn - 1) / n) < n * sn := by rw [Nat.add_mul, Nat.one_mul] at *; omega
      exact Nat.lt_of_mul_lt_mul_left this
    · have hn : 0 < 2 * n := by omega
      have : (2 * i + 1) * sn < (2 * n) * sn := Nat.mul_lt_mul_of_pos_right (by omega) hs
      have b := div_bounds ((2 * i + 1) * sn) (2 * n) hn
      have : 2 * n * ((2 * i + 1) * sn / (2 * n)) < 2 * n * sn := by omega
      exact Nat.lt_of_mul_lt_mul_left this

/-- GDI.STRETCH.SAMPLE: the destination walks the source in order, never going back -/
theorem sample_mono (i sn n : Nat) (flat : Bool) : sample i sn n flat ≤ sample (i + 1) sn n flat := by
  unfold sample
  split
  · exact Nat.div_le_div_right (Nat.mul_le_mul_right sn (Nat.le_succ i))
  · split
    · apply Nat.div_le_div_right; rw [Nat.succ_mul]; omega
    · apply Nat.div_le_div_right; apply Nat.mul_le_mul_right; omega

/-- GDI.STRETCH.SAME: equal extents take each pixel itself -/
theorem sample_same (i n : Nat) (flat : Bool) (hn : 0 < n) : sample i n n flat = i := by
  have h : ¬ (flat && n + 1 == n) = true ∧ ¬ (flat && n == n + 1) = true := by
    constructor <;> simp
  have c := sample_center i n n flat hn h
  have a : 2 * n * sample i n n flat ≤ 2 * n * i + n := by
    have : (2 * i + 1) * n = 2 * n * i + n := by rw [Nat.add_mul, Nat.one_mul, Nat.mul_assoc, Nat.mul_comm i n, ← Nat.mul_assoc]
    omega
  have b : 2 * n * i + n < 2 * n * (sample i n n flat + 1) := by
    have : (2 * i + 1) * n = 2 * n * i + n := by rw [Nat.add_mul, Nat.one_mul, Nat.mul_assoc, Nat.mul_comm i n, ← Nat.mul_assoc]
    omega
  have lo : sample i n n flat ≤ i := by
    apply Nat.le_of_lt_succ; apply Nat.lt_of_mul_lt_mul_left (a := 2 * n); rw [Nat.mul_succ]; omega
  have hi : i ≤ sample i n n flat := by
    apply Nat.le_of_lt_succ; apply Nat.lt_of_mul_lt_mul_left (a := 2 * n); rw [Nat.mul_succ]; rw [Nat.mul_add, Nat.mul_one] at b; omega
  omega

def mirrors : List Mirror := [
  ⟨"gdi.sample", fun
    | [i, sn, n, f] => do pure [sample (← nat i) (← nat sn) (← nat n) (f != 0)]
    | _ => none⟩
]

end Kikizake.Gdi.Stretch
