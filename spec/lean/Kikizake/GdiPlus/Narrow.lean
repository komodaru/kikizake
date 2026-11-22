import Kikizake.GdiPlus.Conv
/-! GP.CONV.NARROW's table grows with the level: one row of 256 levels a lemma (the kernel checks
each by evaluation; one statement over all 8192 levels takes more memory than a runner has). -/
namespace Kikizake.GdiPlus.Narrow
open Kikizake.GdiPlus.Conv

private theorem row0 : ∀ j : Fin 256, narrow (0 * 256 + j.val) ≤ narrow (0 * 256 + j.val + 1) := by decide +kernel
private theorem row1 : ∀ j : Fin 256, narrow (1 * 256 + j.val) ≤ narrow (1 * 256 + j.val + 1) := by decide +kernel
private theorem row2 : ∀ j : Fin 256, narrow (2 * 256 + j.val) ≤ narrow (2 * 256 + j.val + 1) := by decide +kernel
private theorem row3 : ∀ j : Fin 256, narrow (3 * 256 + j.val) ≤ narrow (3 * 256 + j.val + 1) := by decide +kernel
private theorem row4 : ∀ j : Fin 256, narrow (4 * 256 + j.val) ≤ narrow (4 * 256 + j.val + 1) := by decide +kernel
private theorem row5 : ∀ j : Fin 256, narrow (5 * 256 + j.val) ≤ narrow (5 * 256 + j.val + 1) := by decide +kernel
private theorem row6 : ∀ j : Fin 256, narrow (6 * 256 + j.val) ≤ narrow (6 * 256 + j.val + 1) := by decide +kernel
private theorem row7 : ∀ j : Fin 256, narrow (7 * 256 + j.val) ≤ narrow (7 * 256 + j.val + 1) := by decide +kernel
private theorem row8 : ∀ j : Fin 256, narrow (8 * 256 + j.val) ≤ narrow (8 * 256 + j.val + 1) := by decide +kernel
private theorem row9 : ∀ j : Fin 256, narrow (9 * 256 + j.val) ≤ narrow (9 * 256 + j.val + 1) := by decide +kernel
private theorem row10 : ∀ j : Fin 256, narrow (10 * 256 + j.val) ≤ narrow (10 * 256 + j.val + 1) := by decide +kernel
private theorem row11 : ∀ j : Fin 256, narrow (11 * 256 + j.val) ≤ narrow (11 * 256 + j.val + 1) := by decide +kernel
private theorem row12 : ∀ j : Fin 256, narrow (12 * 256 + j.val) ≤ narrow (12 * 256 + j.val + 1) := by decide +kernel
private theorem row13 : ∀ j : Fin 256, narrow (13 * 256 + j.val) ≤ narrow (13 * 256 + j.val + 1) := by decide +kernel
private theorem row14 : ∀ j : Fin 256, narrow (14 * 256 + j.val) ≤ narrow (14 * 256 + j.val + 1) := by decide +kernel
private theorem row15 : ∀ j : Fin 256, narrow (15 * 256 + j.val) ≤ narrow (15 * 256 + j.val + 1) := by decide +kernel
private theorem row16 : ∀ j : Fin 256, narrow (16 * 256 + j.val) ≤ narrow (16 * 256 + j.val + 1) := by decide +kernel
private theorem row17 : ∀ j : Fin 256, narrow (17 * 256 + j.val) ≤ narrow (17 * 256 + j.val + 1) := by decide +kernel
private theorem row18 : ∀ j : Fin 256, narrow (18 * 256 + j.val) ≤ narrow (18 * 256 + j.val + 1) := by decide +kernel
private theorem row19 : ∀ j : Fin 256, narrow (19 * 256 + j.val) ≤ narrow (19 * 256 + j.val + 1) := by decide +kernel
private theorem row20 : ∀ j : Fin 256, narrow (20 * 256 + j.val) ≤ narrow (20 * 256 + j.val + 1) := by decide +kernel
private theorem row21 : ∀ j : Fin 256, narrow (21 * 256 + j.val) ≤ narrow (21 * 256 + j.val + 1) := by decide +kernel
private theorem row22 : ∀ j : Fin 256, narrow (22 * 256 + j.val) ≤ narrow (22 * 256 + j.val + 1) := by decide +kernel
private theorem row23 : ∀ j : Fin 256, narrow (23 * 256 + j.val) ≤ narrow (23 * 256 + j.val + 1) := by decide +kernel
private theorem row24 : ∀ j : Fin 256, narrow (24 * 256 + j.val) ≤ narrow (24 * 256 + j.val + 1) := by decide +kernel
private theorem row25 : ∀ j : Fin 256, narrow (25 * 256 + j.val) ≤ narrow (25 * 256 + j.val + 1) := by decide +kernel
private theorem row26 : ∀ j : Fin 256, narrow (26 * 256 + j.val) ≤ narrow (26 * 256 + j.val + 1) := by decide +kernel
private theorem row27 : ∀ j : Fin 256, narrow (27 * 256 + j.val) ≤ narrow (27 * 256 + j.val + 1) := by decide +kernel
private theorem row28 : ∀ j : Fin 256, narrow (28 * 256 + j.val) ≤ narrow (28 * 256 + j.val + 1) := by decide +kernel
private theorem row29 : ∀ j : Fin 256, narrow (29 * 256 + j.val) ≤ narrow (29 * 256 + j.val + 1) := by decide +kernel
private theorem row30 : ∀ j : Fin 256, narrow (30 * 256 + j.val) ≤ narrow (30 * 256 + j.val + 1) := by decide +kernel
private theorem row31 : ∀ j : Fin 256, narrow (31 * 256 + j.val) ≤ narrow (31 * 256 + j.val + 1) := by decide +kernel

/-- GP.CONV.NARROW: the inverse sRGB curve grows with the level, 0 at 0 and 255 at 8192 -/
theorem narrow_mono (v : Nat) (h : v < 8192) : narrow v ≤ narrow (v + 1) := by
  have e : v = v / 256 * 256 + v % 256 := by omega
  have hi : v / 256 < 32 := by omega
  rw [e]
  have r : ∀ i (hi : i < 32) (j : Fin 256), narrow (i * 256 + j.val) ≤ narrow (i * 256 + j.val + 1) := by
    intro i hi j
    match i, hi with
    | 0, _ => exact row0 j
    | 1, _ => exact row1 j
    | 2, _ => exact row2 j
    | 3, _ => exact row3 j
    | 4, _ => exact row4 j
    | 5, _ => exact row5 j
    | 6, _ => exact row6 j
    | 7, _ => exact row7 j
    | 8, _ => exact row8 j
    | 9, _ => exact row9 j
    | 10, _ => exact row10 j
    | 11, _ => exact row11 j
    | 12, _ => exact row12 j
    | 13, _ => exact row13 j
    | 14, _ => exact row14 j
    | 15, _ => exact row15 j
    | 16, _ => exact row16 j
    | 17, _ => exact row17 j
    | 18, _ => exact row18 j
    | 19, _ => exact row19 j
    | 20, _ => exact row20 j
    | 21, _ => exact row21 j
    | 22, _ => exact row22 j
    | 23, _ => exact row23 j
    | 24, _ => exact row24 j
    | 25, _ => exact row25 j
    | 26, _ => exact row26 j
    | 27, _ => exact row27 j
    | 28, _ => exact row28 j
    | 29, _ => exact row29 j
    | 30, _ => exact row30 j
    | 31, _ => exact row31 j
    | _ + 32, h => exact absurd h (by omega)
  exact r (v / 256) hi ⟨v % 256, Nat.mod_lt _ (by decide)⟩

theorem narrow_ends : narrow 0 = 0 ∧ narrow 8192 = 255 := by decide +kernel

end Kikizake.GdiPlus.Narrow
