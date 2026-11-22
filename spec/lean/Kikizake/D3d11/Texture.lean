import Kikizake.Mirror
import Kikizake.D3d9.Pixel
/-! The integer rules of texturing and the output merger on Direct3D 10 and 11 (spec/d3dres.txt
D3DRASTER.*), as src/model/d3drules.rs computes them: `address` is `address`
(D3D11_TEXTURE_ADDRESS_MODE, the same rule as Direct3D 9's), `cmp` `compare`
(D3D11_COMPARISON_FUNC, Direct3D 9's), `bilinear8` `bilinear8`, `mip8` `mip8`, `reduce` `reduce`
(the minimum and maximum filters) (mirrors `d3d11.address`, `d3d11.cmp`, `d3d11.bilinear8`,
`d3d11.mip8`, `d3d11.reduce`). -/
namespace Kikizake.D3d11.Texture

/-- D3DRASTER.ADDRESS, BORDER: a texel coordinate through the address mode (WRAP 1, MIRROR 2,
CLAMP 3, BORDER 4, MIRROR_ONCE 5); none: the border color -/
def address (mode : Nat) (i n : Int) : Option Int := D3d9.Pixel.address mode i n

/-- D3DRASTER.ADDRESS, BORDER: every mode but BORDER gives a texel of the texture; BORDER gives the
border color exactly outside it -/
theorem address_inside (mode : Nat) (i n : Int) (hn : 0 < n) :
    (∀ t, address mode i n = some t → 0 ≤ t ∧ t < n) ∧ (mode ≠ 4 → address mode i n ≠ none) ∧
    (address 4 i n = none ↔ i < 0 ∨ n ≤ i) :=
  D3d9.Pixel.address_inside mode i n hn

/-- D3DRASTER.ADDRESS: wrap repeats every n texels; mirror every 2n and reflects about the
texture's edge; clamp is the identity inside; mirror once is mirror between -n and n -/
theorem address_modes (i n : Int) (hn : 0 < n) :
    address 1 (i + n) n = address 1 i n ∧ address 2 (i + 2 * n) n = address 2 i n ∧
    address 2 (-1 - i) n = address 2 i n ∧ (0 ≤ i → i < n → address 3 i n = some i) ∧
    (-n ≤ i → i < n → address 5 i n = address 2 i n) :=
  D3d9.Pixel.address_modes i n hn

/-- D3DRASTER.DEPTH, SAMPLE.CMP: the comparison function of a new value against a stored one -/
def cmp (f a b : Nat) : Bool := D3d9.Pixel.cmp f a b

/-- D3DRASTER.DEPTH, SAMPLE.CMP: the functions pair up as complements (LESS and GREATER_EQUAL,
EQUAL and NOT_EQUAL, LESS_EQUAL and GREATER, NEVER and ALWAYS); exactly one of LESS, EQUAL,
GREATER holds -/
theorem cmp_pairs (a b : Nat) :
    cmp 2 a b = !cmp 7 a b ∧ cmp 3 a b = !cmp 6 a b ∧ cmp 4 a b = !cmp 5 a b ∧ cmp 1 a b = !cmp 8 a b ∧
    ((cmp 2 a b && cmp 3 a b) = false ∧ (cmp 3 a b && cmp 5 a b) = false ∧ (cmp 2 a b && cmp 5 a b) = false) ∧
    (cmp 2 a b || cmp 3 a b || cmp 5 a b) = true ∧ cmp 2 a b = cmp 5 b a ∧ cmp 4 a b = cmp 7 b a :=
  D3d9.Pixel.cmp_pairs a b

/-- x and y by the weight w of y (in 1/256) -/
def mix (x y w : Nat) : Nat := x * (256 - w) + y * w

/-- D3DRASTER.FILTER.LINEAR: 8-bit texels a (i, j), b (i + 1, j), c (i, j + 1), e (i + 1, j + 1)
by the weights wu, wv: down the columns first, halved, then across; in 1/128 of a byte -/
def bilinear8 (a b c e wu wv : Nat) : Nat := mix (mix a c wv / 2) (mix b e wv / 2) wu / 256

/-- D3DRASTER.FILTER.MIPLINEAR: two levels' 8-bit values (in 1/128) by the next level's weight -/
def mip8 (n0 n1 f : Nat) : Nat := mix n0 n1 f / 256

private theorem mix_le (x y w m : Nat) (hx : x ≤ m) (hy : y ≤ m) (hw : w ≤ 256) : mix x y w ≤ m * 256 := by
  have h1 : x * (256 - w) ≤ m * (256 - w) := Nat.mul_le_mul_right _ hx
  have h2 : y * w ≤ m * w := Nat.mul_le_mul_right _ hy
  have h3 : m * (256 - w) + m * w = m * 256 := by rw [← Nat.mul_add]; congr 1; omega
  unfold mix; omega

private theorem mix_ge (x y w m : Nat) (hx : m ≤ x) (hy : m ≤ y) (hw : w ≤ 256) : m * 256 ≤ mix x y w := by
  have h1 : m * (256 - w) ≤ x * (256 - w) := Nat.mul_le_mul_right _ hx
  have h2 : m * w ≤ y * w := Nat.mul_le_mul_right _ hy
  have h3 : m * (256 - w) + m * w = m * 256 := by rw [← Nat.mul_add]; congr 1; omega
  unfold mix; omega

private theorem mix_same (v w : Nat) (hw : w ≤ 256) : mix v v w = v * 256 := by
  have h3 : v * (256 - w) + v * w = v * 256 := by rw [← Nat.mul_add]; congr 1; omega
  unfold mix; omega

/-- D3DRASTER.FILTER.LINEAR: the filtered value lies between 128 times the least and the greatest
texel (so 8-bit texels give 0..32640); four equal texels give exactly 128 times their byte; with
both weights 0 the first texel alone -/
theorem bilinear8_range (a b c e wu wv lo hi : Nat) (hu : wu ≤ 256) (hv : wv ≤ 256)
    (hl : lo ≤ a ∧ lo ≤ b ∧ lo ≤ c ∧ lo ≤ e) (hh : a ≤ hi ∧ b ≤ hi ∧ c ≤ hi ∧ e ≤ hi) :
    lo * 128 ≤ bilinear8 a b c e wu wv ∧ bilinear8 a b c e wu wv ≤ hi * 128 ∧
    bilinear8 a a a a wu wv = a * 128 ∧ bilinear8 a b c e 0 0 = a * 128 := by
  obtain ⟨l1, l2, l3, l4⟩ := hl
  obtain ⟨h1, h2, h3, h4⟩ := hh
  have la := mix_ge a c wv lo l1 l3 hv
  have lb := mix_ge b e wv lo l2 l4 hv
  have ua := mix_le a c wv hi h1 h3 hv
  have ub := mix_le b e wv hi h2 h4 hv
  have g1 : lo * 128 ≤ mix a c wv / 2 := by omega
  have g2 : lo * 128 ≤ mix b e wv / 2 := by omega
  have k1 : mix a c wv / 2 ≤ hi * 128 := by omega
  have k2 : mix b e wv / 2 ≤ hi * 128 := by omega
  have f1 := mix_ge _ _ wu (lo * 128) g1 g2 hu
  have f2 := mix_le _ _ wu (hi * 128) k1 k2 hu
  refine ⟨?_, ?_, ?_, ?_⟩
  · unfold bilinear8; omega
  · unfold bilinear8; omega
  · unfold bilinear8
    rw [mix_same a wv hv, show a * 256 / 2 = a * 128 by omega, mix_same _ wu hu]
    omega
  · simp only [bilinear8, mix]
    omega

/-- D3DRASTER.FILTER.MIPLINEAR: the blend of two levels lies between them; weight 0 is the first
level, equal levels give their value -/
theorem mip8_range (n0 n1 f : Nat) (hf : f ≤ 256) :
    min n0 n1 ≤ mip8 n0 n1 f ∧ mip8 n0 n1 f ≤ max n0 n1 ∧ mip8 n0 n1 0 = n0 ∧ mip8 n0 n0 f = n0 := by
  have g := mix_ge n0 n1 f (min n0 n1) (Nat.min_le_left _ _) (Nat.min_le_right _ _) hf
  have k := mix_le n0 n1 f (max n0 n1) (Nat.le_max_left _ _) (Nat.le_max_right _ _) hf
  refine ⟨?_, ?_, ?_, ?_⟩
  · unfold mip8; omega
  · unfold mip8; omega
  · simp only [mip8, mix]; omega
  · unfold mip8; rw [mix_same n0 f hf]; omega

/-- D3DRASTER.FILTER.MINMAX: of the 2 x 2 texels a (i, j), b (i + 1, j), c (i, j + 1), e
(i + 1, j + 1) (one component) the least (least true) or greatest of those whose weight is not 0 -/
def reduce (least : Bool) (a b c e wu wv : Int) : Int :=
  let l := [(true, a), (wu != 0, b), (wv != 0, c), (wu != 0 && wv != 0, e)].filter (·.1)
  if least then l.foldl (fun x t => min x t.2) a else l.foldl (fun x t => max x t.2) a

/-- the texels a minimum or maximum filter takes: those of weight not 0 -/
def taken (a b c e wu wv : Int) : List Int :=
  ([(true, a), (wu != 0, b), (wv != 0, c), (wu != 0 && wv != 0, e)].filter (·.1)).map (·.2)

private theorem fold_min (l : List (Bool × Int)) (x : Int) :
    (∀ t ∈ l, l.foldl (fun x t => min x t.2) x ≤ t.2) ∧ l.foldl (fun x t => min x t.2) x ≤ x ∧
    (l.foldl (fun x t => min x t.2) x = x ∨ ∃ t ∈ l, l.foldl (fun x t => min x t.2) x = t.2) := by
  induction l generalizing x with
  | nil => simp
  | cons h t ih =>
    obtain ⟨i1, i2, i3⟩ := ih (min x h.2)
    simp only [List.foldl_cons, List.mem_cons, forall_eq_or_imp, exists_eq_or_imp]
    refine ⟨⟨by omega, i1⟩, by omega, ?_⟩
    rcases i3 with e | ⟨u, hu, e⟩
    · by_cases hx : x ≤ h.2
      · left; omega
      · right; left; omega
    · right; right; exact ⟨u, hu, e⟩

private theorem fold_max (l : List (Bool × Int)) (x : Int) :
    (∀ t ∈ l, t.2 ≤ l.foldl (fun x t => max x t.2) x) ∧ x ≤ l.foldl (fun x t => max x t.2) x ∧
    (l.foldl (fun x t => max x t.2) x = x ∨ ∃ t ∈ l, l.foldl (fun x t => max x t.2) x = t.2) := by
  induction l generalizing x with
  | nil => simp
  | cons h t ih =>
    obtain ⟨i1, i2, i3⟩ := ih (max x h.2)
    simp only [List.foldl_cons, List.mem_cons, forall_eq_or_imp, exists_eq_or_imp]
    refine ⟨⟨by omega, i1⟩, by omega, ?_⟩
    rcases i3 with e | ⟨u, hu, e⟩
    · by_cases hx : h.2 ≤ x
      · left; omega
      · right; left; omega
    · right; right; exact ⟨u, hu, e⟩

/-- D3DRASTER.FILTER.MINMAX: the minimum filter's value is one of the texels of weight not 0 and
no greater than any of them, the maximum's one of them and no less; texels of weight 0 do not
count (with both weights 0 the first texel alone) -/
theorem reduce_picks (a b c e wu wv : Int) :
    (reduce true a b c e wu wv ∈ taken a b c e wu wv ∧ ∀ t ∈ taken a b c e wu wv, reduce true a b c e wu wv ≤ t) ∧
    (reduce false a b c e wu wv ∈ taken a b c e wu wv ∧ ∀ t ∈ taken a b c e wu wv, t ≤ reduce false a b c e wu wv) ∧
    reduce true a b c e 0 0 = a ∧ reduce false a b c e 0 0 = a := by
  let l := [(true, a), (wu != 0, b), (wv != 0, c), (wu != 0 && wv != 0, e)].filter (·.1)
  have ha : (true, a) ∈ l := by simp [l]
  have hmem : ∀ t ∈ l, t.2 ∈ taken a b c e wu wv := fun t ht => List.mem_map_of_mem ht
  have hall : ∀ v ∈ taken a b c e wu wv, ∃ t ∈ l, t.2 = v := by
    intro v hv
    simp only [taken, List.mem_map] at hv
    exact hv
  obtain ⟨m1, m2, m3⟩ := fold_min l a
  obtain ⟨x1, x2, x3⟩ := fold_max l a
  simp only [reduce, ↓reduceIte, Bool.false_eq_true]
  refine ⟨⟨?_, ?_⟩, ⟨?_, ?_⟩, by simp, by simp⟩
  · show l.foldl _ a ∈ _
    rcases m3 with e | ⟨u, hu, e⟩
    · rw [e]; exact hmem _ ha
    · rw [e]; exact hmem _ hu
  · intro t ht
    obtain ⟨u, hu, rfl⟩ := hall t ht
    exact m1 u hu
  · show l.foldl _ a ∈ _
    rcases x3 with e | ⟨u, hu, e⟩
    · rw [e]; exact hmem _ ha
    · rw [e]; exact hmem _ hu
  · intro t ht
    obtain ⟨u, hu, rfl⟩ := hall t ht
    exact x1 u hu

def mirrors : List Mirror := [
  ⟨"d3d11.address", fun
    | [m, i, n] => do pure ((address (← nat m) i n).toList)
    | _ => none⟩,
  ⟨"d3d11.cmp", fun
    | [f, a, b] => do pure [if cmp (← nat f) (← nat a) (← nat b) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.bilinear8", fun
    | [a, b, c, e, wu, wv] => do pure [bilinear8 (← nat a) (← nat b) (← nat c) (← nat e) (← nat wu) (← nat wv)]
    | _ => none⟩,
  ⟨"d3d11.mip8", fun
    | [a, b, f] => do pure [mip8 (← nat a) (← nat b) (← nat f)]
    | _ => none⟩,
  ⟨"d3d11.reduce", fun
    | [p, a, b, c, e, wu, wv] => pure [reduce (p == 0) a b c e wu wv]
    | _ => none⟩
]

end Kikizake.D3d11.Texture
