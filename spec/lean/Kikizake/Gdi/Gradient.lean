import Kikizake.Mirror
/-! GradientFill's rounding and dithering (spec/gdiblend.txt GDI.GRAD.*), as src/model/gdiblend.rs
computes them: `c8` is `gdiblend::c8`, `dither16` is `gdiblend::dither16`, `cell5` is
`gdiblend::cell5`, the matrices `BAYER4`, `DITHER6`, `DITHER3` (mirrors `gdi.c8`, `gdi.dither16`,
`gdi.cell5`, `gdi.matrix`). -/
namespace Kikizake.Gdi.Gradient

/-- a channel's 16 bit value num / den (den > 0): truncated (rectangles, GDI.GRAD.RECTH) or to the
nearest, halves up (triangles, GDI.GRAD.TRIROUND) -/
def value (num den : Int) (nearest : Bool) : Int :=
  if nearest then (2 * num + den) / (2 * den) else num / den

/-- the 16 bit value's high byte (an arithmetic shift, then its low 8 bits) -/
def c8 (num den : Int) (nearest : Bool) : Int := value num den nearest / 256 % 256

private theorem ediv_bounds (a b : Int) (hb : 0 < b) : b * (a / b) ≤ a ∧ a < b * (a / b) + b := by
  have h := Int.emod_add_mul_ediv a b
  have l := Int.emod_nonneg a (show b ≠ 0 by omega)
  have u := Int.emod_lt_of_pos a hb
  constructor <;> omega

/-- GDI.GRAD.RECTH, GDI.GRAD.RECTV: the truncated value is the largest at most num / den -/
theorem value_floor (num den : Int) (h : 0 < den) :
    den * value num den false ≤ num ∧ num < den * value num den false + den := by
  unfold value; simp only [Bool.false_eq_true, ite_false]; exact ediv_bounds _ _ h

/-- GDI.GRAD.TRIROUND: the nearest value, halves up: num / den lies in [v - 1/2, v + 1/2) -/
theorem value_nearest (num den : Int) (h : 0 < den) :
    2 * den * value num den true ≤ 2 * num + den ∧ 2 * num + den < 2 * den * value num den true + 2 * den := by
  unfold value; simp only [ite_true]; exact ediv_bounds _ _ (by omega)

/-- the 8 bits are the 16 bit value's high byte for every value a channel takes -/
theorem c8_high_byte (num den : Int) (r : Bool) (h : 0 ≤ value num den r ∧ value num den r < 65536) :
    c8 num den r = value num den r / 256 := by
  unfold c8; omega

/-- GDI.GRAD.DITHER16: the 4x4 ordered matrix -/
def BAYER4 : List (List Int) := [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]

/-- the matrix holds each of 0..15 once -/
theorem bayer_permutation : BAYER4.flatten.length = 16 ∧ ∀ k : Fin 16, BAYER4.flatten.count (k.val : Int) = 1 := by decide

/-- the larger of two -/
def atLeast (a b : Int) : Int := if a < b then b else a

/-- GDI.GRAD.DITHER16: one channel, `max`'s bits in steps of `unit` (Rust's `/` truncates) -/
def dither16 (v16 d unit max : Int) : Int := atLeast 0 (min max ((v16.tdiv unit + d).tdiv 16))

private theorem mean_small : ∀ r : Fin 16, (BAYER4.flatten.map (fun m => ((r.val : Int) + m) / 16)).sum = r.val := by decide

private theorem sum_add (q : Int) (l : List Int) (f : Int → Int) : (l.map (fun m => q + f m)).sum = l.length * q + (l.map f).sum := by
  induction l with
  | nil => simp
  | cons x t ih => simp only [List.map_cons, List.sum_cons, List.length_cons, ih]; grind

/-- GDI.GRAD.DITHER16: over the matrix's 16 pixels the dithered levels add up to the value in
1/16 steps (Hermite's identity): the dither keeps the gradient's mean exactly -/
theorem dither_mean (x : Int) : (BAYER4.flatten.map (fun m => (x + m) / 16)).sum = x := by
  have e : BAYER4.flatten.map (fun m => (x + m) / 16) = BAYER4.flatten.map (fun m => x / 16 + (x % 16 + m) / 16) := by
    apply List.map_congr_left; intro m hm
    have : 0 ≤ m ∧ m < 16 := by simp [BAYER4] at hm; omega
    omega
  rw [e, sum_add]
  have s := mean_small ⟨(x % 16).toNat, by omega⟩
  simp only [Int.toNat_of_nonneg (Int.emod_nonneg x (show (16 : Int) ≠ 0 by decide))] at s
  rw [s]; simp [BAYER4]; omega

/-- GDI.GRAD.DITHER16: unclamped, each pixel is that dither (truncation is the floor from 0), the
value's level or the next -/
theorem dither16_level (v16 d unit max : Int) (hv : 0 ≤ v16) (hu : 0 < unit) (hd : 0 ≤ d ∧ d < 16)
    (hm : (v16 / unit + d) / 16 ≤ max) :
    dither16 v16 d unit max = (v16 / unit + d) / 16 ∧
    v16 / unit / 16 ≤ dither16 v16 d unit max ∧ dither16 v16 d unit max ≤ v16 / unit / 16 + 1 := by
  have q : 0 ≤ v16 / unit := Int.ediv_nonneg hv (Int.le_of_lt hu)
  have t1 : v16.tdiv unit = v16 / unit := Int.tdiv_eq_ediv_of_nonneg hv
  have t2 : (v16 / unit + d).tdiv 16 = (v16 / unit + d) / 16 := Int.tdiv_eq_ediv_of_nonneg (by omega)
  unfold dither16 atLeast; rw [t1, t2]
  refine ⟨?_, ?_, ?_⟩ <;> split <;> omega

/-- GDI.GRAD.INDEXED, GDI.GRAD.HALFTONE: a channel's 5 bit cell -/
def cell5 (mono six : Bool) (v d : Int) : Int :=
  if mono then (if v + 2 * d ≥ 255 then 31 else 0)
  else if six then (if v + d ≥ 255 then 255 else 51 * (v + d).tdiv 51) / 8
  else if v + d ≤ 127 then 0 else if v + d ≤ 254 then 16 else 31

/-- GDI.GRAD.INDEXED: with a color table each channel goes to 0, 128 or 255 (cells 0, 16, 31),
the larger the channel the larger the level -/
theorem cell5_three (v v' d : Int) (h : v ≤ v') :
    (cell5 false false v d = 0 ∨ cell5 false false v d = 16 ∨ cell5 false false v d = 31) ∧
    cell5 false false v d ≤ cell5 false false v' d := by
  unfold cell5; simp only [Bool.false_eq_true, ite_false]
  constructor <;> (repeat' split) <;> omega

/-- GDI.GRAD.HALFTONE: the halftone level is the multiple of 51 at or under v + d, so, d being
0..50, one of the two of 0, 51, .. 255 around v; its cell is that level's top 5 bits -/
theorem cell5_six (v d : Int) (hv : 0 ≤ v) (hd : 0 ≤ d ∧ d ≤ 50) (h : v + d < 255) :
    cell5 false true v d = (51 * ((v + d) / 51)) / 8 ∧
    v - 51 < 51 * ((v + d) / 51) ∧ 51 * ((v + d) / 51) ≤ v + 50 := by
  have t : (v + d).tdiv 51 = (v + d) / 51 := Int.tdiv_eq_ediv_of_nonneg (by omega)
  have b := ediv_bounds (v + d) 51 (by decide)
  unfold cell5; simp only [Bool.false_eq_true, ite_false, ite_true, t]
  refine ⟨?_, ?_, ?_⟩
  · split <;> omega
  · omega
  · omega

/-- GDI.GRAD.INDEXED: 1 bpp is white where v + 2 d reaches 255, black otherwise -/
theorem cell5_mono (six : Bool) (v d : Int) : cell5 true six v d = (if v + 2 * d ≥ 255 then 31 else 0) := by
  simp [cell5]

/-- GDI.GRAD.HALFTONE: the halftone matrix (clean-room note on win32kfull) -/
def DITHER6 : List (List Int) := [
  [3, 28, 9, 35, 4, 30, 11, 36, 3, 29, 10, 35, 5, 30, 11, 37],
  [41, 16, 48, 22, 43, 17, 49, 24, 42, 16, 48, 22, 43, 18, 50, 24],
  [6, 32, 0, 25, 8, 33, 1, 27, 6, 32, 0, 26, 8, 34, 2, 27],
  [44, 19, 38, 12, 46, 20, 40, 14, 45, 19, 38, 13, 46, 21, 40, 14],
  [5, 31, 12, 37, 4, 29, 10, 36, 6, 31, 12, 38, 4, 30, 10, 36],
  [44, 18, 50, 24, 42, 16, 48, 23, 44, 18, 50, 25, 42, 17, 49, 23],
  [8, 34, 2, 28, 7, 32, 0, 26, 9, 34, 2, 28, 7, 33, 1, 26],
  [47, 21, 40, 15, 45, 20, 39, 13, 47, 22, 41, 15, 46, 20, 39, 14],
  [3, 29, 9, 35, 5, 30, 11, 37, 3, 28, 9, 35, 4, 30, 11, 36],
  [41, 16, 48, 22, 43, 17, 49, 24, 41, 15, 47, 22, 43, 17, 49, 23],
  [6, 32, 0, 25, 8, 33, 1, 27, 6, 31, 0, 25, 7, 33, 1, 27],
  [45, 19, 38, 13, 46, 21, 40, 14, 44, 19, 38, 12, 46, 20, 39, 14],
  [5, 31, 12, 37, 4, 29, 10, 36, 5, 31, 11, 37, 3, 29, 10, 35],
  [44, 18, 50, 25, 42, 17, 49, 23, 43, 18, 50, 24, 42, 16, 48, 23],
  [9, 34, 2, 28, 7, 33, 1, 26, 8, 34, 2, 27, 7, 32, 0, 26],
  [47, 21, 41, 15, 45, 20, 39, 13, 47, 21, 40, 15, 45, 19, 39, 13]]

/-- GDI.GRAD.INDEXED: the matrix with a plain color table -/
def DITHER3 : List (List Int) := [
  [8, 72, 24, 88, 12, 76, 28, 92, 9, 73, 25, 89, 13, 77, 29, 93],
  [104, 40, 120, 56, 108, 44, 124, 60, 105, 41, 121, 57, 109, 45, 125, 61],
  [16, 80, 0, 64, 20, 84, 4, 68, 17, 81, 1, 65, 21, 85, 5, 69],
  [112, 48, 96, 32, 116, 52, 100, 36, 113, 49, 97, 33, 117, 53, 101, 37],
  [14, 78, 30, 94, 10, 74, 26, 90, 15, 79, 31, 95, 11, 75, 27, 91],
  [110, 46, 126, 62, 106, 42, 122, 58, 111, 47, 126, 63, 107, 43, 123, 59],
  [22, 86, 6, 70, 18, 82, 2, 66, 23, 87, 7, 71, 19, 83, 3, 67],
  [118, 54, 102, 38, 114, 50, 98, 34, 119, 55, 103, 39, 115, 51, 99, 35],
  [9, 73, 25, 89, 13, 77, 29, 93, 8, 72, 24, 88, 12, 76, 28, 92],
  [105, 41, 121, 57, 109, 45, 125, 61, 104, 40, 120, 56, 108, 44, 124, 60],
  [17, 81, 1, 65, 21, 85, 5, 69, 16, 80, 0, 64, 20, 84, 4, 68],
  [113, 49, 97, 33, 117, 53, 101, 37, 112, 48, 96, 32, 116, 52, 100, 36],
  [15, 79, 31, 95, 11, 75, 27, 91, 14, 78, 30, 94, 10, 74, 26, 90],
  [111, 47, 126, 63, 107, 43, 123, 59, 110, 46, 126, 62, 106, 42, 122, 58],
  [23, 87, 7, 71, 19, 83, 3, 67, 22, 86, 6, 70, 18, 82, 2, 66],
  [119, 55, 103, 39, 115, 51, 99, 35, 118, 54, 102, 38, 114, 50, 98, 34]]

/-- GDI.GRAD.HALFTONE: 16 rows of 16, values 0 to 50 -/
theorem dither6_range : DITHER6.length = 16 ∧ DITHER6.all (fun r => r.length == 16 && r.all (fun v => decide (0 ≤ v) && decide (v ≤ 50))) := by decide

/-- GDI.GRAD.INDEXED: 16 rows of 16, values 0 to 127 (a channel moves at most to the next level) -/
theorem dither3_range : DITHER3.length = 16 ∧ DITHER3.all (fun r => r.length == 16 && r.all (fun v => decide (0 ≤ v) && decide (v ≤ 127))) := by decide

def entry (m : List (List Int)) (x y : Nat) : Option Int := (m[y]?).bind (·[x]?)

def mirrors : List Mirror := [
  ⟨"gdi.c8", fun
    | [n, d, r] => if d ≤ 0 then none else pure [c8 n d (r != 0)]
    | _ => none⟩,
  ⟨"gdi.dither16", fun
    | [v, d, u, m] => if u == 0 then none else pure [dither16 v d u m]
    | _ => none⟩,
  ⟨"gdi.cell5", fun
    | [mono, six, v, d] => pure [cell5 (mono != 0) (six != 0) v d]
    | _ => none⟩,
  ⟨"gdi.matrix", fun
    | [k, x, y] => do
      let m ← (if k == 0 then some BAYER4 else if k == 1 then some DITHER6 else if k == 2 then some DITHER3 else none)
      pure [← entry m (← nat x) (← nat y)]
    | _ => none⟩
]

end Kikizake.Gdi.Gradient
