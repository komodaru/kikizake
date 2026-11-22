import Kikizake.Mirror
import Kikizake.D3d9.Pixel
/-! Direct3D 9's integer rules of drawing (spec/d3d9draw.txt), as src/model/d3d9draw.rs computes
them: `covers` is `covers` (the top-left fill convention in 1/256 pixel units), `culls` `culls`
(D3DCULL), `blendOp` `blend_op` (D3DBLENDOP), `blendColor` `blend_color`, `factor8` `factor8`
(D3DBLEND on 8-bit channels), `bothAlpha` `both_alpha`, `colorWrite` `color_write`
(D3DRS_COLORWRITEENABLE) (mirrors `d3d9.covers`, `d3d9.cull`, `d3d9.blendop`, `d3d9.factor8`,
`d3d9.bothalpha`, `d3d9.colorwrite`). -/
namespace Kikizake.D3d9.Raster

open Kikizake.D3d9.Pixel (Op)

/-- a point in 1/256 pixel units: x, y (y down) -/
abbrev Pt := Int × Int

/-- twice the signed area of the triangle a b c (positive: clockwise on the screen) -/
def area (a b c : Pt) : Int := (b.1 - a.1) * (c.2 - a.2) - (b.2 - a.2) * (c.1 - a.1)

/-- which side of the edge p q the point s is on (positive: inside a triangle of positive area) -/
def edge (p q s : Pt) : Int := (q.1 - p.1) * (s.2 - p.2) - (q.2 - p.2) * (s.1 - p.1)

/-- a top edge (going left, y the same) or a left one (going up) -/
def topLeft (p q : Pt) : Bool := decide (q.2 - p.2 < 0) || (decide (q.2 = p.2) && decide (q.1 - p.1 > 0))

/-- the edge p q draws s: strictly inside, or on it where it is a top or a left edge -/
def incl (p q s : Pt) : Bool := decide (edge p q s > 0) || (decide (edge p q s = 0) && topLeft p q)

/-- D3D9.RASTER.TOPLEFT: the triangle draws s; turned to wind with a positive area, nothing for
no area -/
def covers (a b c s : Pt) : Bool :=
  if area a b c = 0 then false
  else if area a b c < 0 then incl a c s && incl c b s && incl b a s
  else incl a b s && incl b c s && incl c a s

private theorem edge_rev (p q s : Pt) : edge q p s = -edge p q s := by
  obtain ⟨px, py⟩ := p
  obtain ⟨qx, qy⟩ := q
  obtain ⟨sx, sy⟩ := s
  simp only [edge, Int.sub_mul, Int.mul_sub]
  have h1 : px * sy - px * qy = px * (sy - qy) := by rw [Int.mul_sub]
  have h2 : (qx - px) * (sy - py) = qx * sy - qx * py - (px * sy - px * py) := by
    rw [Int.sub_mul, Int.mul_sub, Int.mul_sub]
  have h3 : (qy - py) * (sx - px) = qy * sx - qy * px - (py * sx - py * px) := by
    rw [Int.sub_mul, Int.mul_sub, Int.mul_sub]
  have c1 : qy * px = px * qy := Int.mul_comm _ _
  have c2 : py * px = px * py := Int.mul_comm _ _
  have c3 : qy * qx = qx * qy := Int.mul_comm _ _
  have c4 : py * qx = qx * py := Int.mul_comm _ _
  omega

private theorem area_turn (a b c : Pt) : area b c a = area a b c := by
  obtain ⟨ax, ay⟩ := a
  obtain ⟨bx, by'⟩ := b
  obtain ⟨cx, cy⟩ := c
  simp only [area, Int.sub_mul, Int.mul_sub]
  have := Int.mul_comm ax by'
  have := Int.mul_comm ax cy
  have := Int.mul_comm bx ay
  have := Int.mul_comm bx cy
  have := Int.mul_comm cx ay
  have := Int.mul_comm cx by'
  have := Int.mul_comm ay ax
  have := Int.mul_comm by' bx
  have := Int.mul_comm cy cx
  omega

private theorem area_swap (a b c : Pt) : area a c b = -area a b c := by
  obtain ⟨ax, ay⟩ := a
  obtain ⟨bx, by'⟩ := b
  obtain ⟨cx, cy⟩ := c
  simp only [area, Int.sub_mul, Int.mul_sub]
  have := Int.mul_comm ax by'
  have := Int.mul_comm ax cy
  have := Int.mul_comm bx ay
  have := Int.mul_comm bx cy
  have := Int.mul_comm cx ay
  have := Int.mul_comm cx by'
  have := Int.mul_comm ay ax
  have := Int.mul_comm by' bx
  have := Int.mul_comm cy cx
  omega

/-- D3D9.RASTER.TOPLEFT: an edge and the same edge the other way draw each point once between
them: on the line exactly one is a top or a left edge, off it exactly one has the point inside -/
theorem incl_shared (p q s : Pt) (h : p ≠ q) : incl p q s = !incl q p s := by
  have hr := edge_rev p q s
  have ht : topLeft q p = !topLeft p q := by
    obtain ⟨px, py⟩ := p
    obtain ⟨qx, qy⟩ := q
    have hne : px ≠ qx ∨ py ≠ qy := by
      by_cases hx : px = qx
      · subst hx; exact Or.inr (fun hy => h (by rw [hy]))
      · exact Or.inl hx
    simp only [topLeft]
    by_cases h1 : qy - py < 0
    · have : ¬py - qy < 0 := by omega
      have : ¬py = qy := by omega
      simp [*]
    · by_cases h2 : qy = py
      · subst h2
        have hx : px ≠ qx := by
          rcases hne with h | h
          · exact h
          · exact absurd rfl h
        by_cases h3 : qx < px
        · have : ¬px < qx := by omega
          simp [h3, this]
        · have : px < qx := by omega
          simp [h3, this]
      · have : py - qy < 0 := by omega
        simp [*]
  simp only [incl, hr, ht]
  generalize edge p q s = e
  generalize topLeft p q = t
  by_cases h1 : e > 0
  · have : ¬-e > 0 := by omega
    have : ¬-e = 0 := by omega
    simp [*]
    omega
  · by_cases h2 : e = 0
    · subst h2; cases t <;> simp
    · have : -e > 0 := by omega
      simp [*]
      omega

/-- D3D9.RASTER.TOPLEFT: the triangle draws the same points whichever vertex it starts at and
whichever way it winds -/
theorem covers_turn (a b c s : Pt) : covers b c a s = covers a b c s ∧ covers a c b s = covers a b c s := by
  have t := area_turn a b c
  have w := area_swap a b c
  constructor
  · simp only [covers, t]
    split
    · rfl
    · split <;> cases incl a b s <;> cases incl b c s <;> cases incl c a s <;>
        cases incl a c s <;> cases incl c b s <;> cases incl b a s <;> rfl
  · simp only [covers, w]
    by_cases h0 : area a b c = 0
    · simp [h0]
    · have : -area a b c ≠ 0 := by omega
      by_cases h1 : area a b c < 0
      · have : ¬-area a b c < 0 := by omega
        simp only [h0, this, h1, ↓reduceIte, Int.neg_eq_zero]
      · have : -area a b c < 0 := by omega
        simp only [h0, this, h1, ↓reduceIte, Int.neg_eq_zero]

/-- D3D9.RASTER.TOPLEFT: two triangles that share the edge a b from opposite sides (both of
positive area: a b c and b a d) never both draw a point, and where the point is drawn by their
other edges exactly one of them draws it: no pixel twice, none missed along the edge -/
theorem shared_edge (a b c d s : Pt) (hc : area a b c > 0) (hd : area b a d > 0) :
    !(covers a b c s && covers b a d s) ∧
    (incl b c s = true → incl c a s = true → incl a d s = true → incl d b s = true →
      covers a b c s = !covers b a d s) := by
  have hab : a ≠ b := by
    intro e; subst e; simp [area] at hc
  have hs := incl_shared a b s hab
  have h1 : ¬area a b c = 0 := by omega
  have h2 : ¬area a b c < 0 := by omega
  have h3 : ¬area b a d = 0 := by omega
  have h4 : ¬area b a d < 0 := by omega
  simp only [covers, h1, h2, h3, h4, ↓reduceIte, hs]
  constructor
  · cases incl b a s <;> simp
  · intro e1 e2 e3 e4
    simp [e1, e2, e3, e4]

/-- how a triangle winds on the screen: the sign of its area -/
inductive Winding where
  | cw
  | ccw
  | flat
  deriving DecidableEq

/-- D3D9.CULL, CULL.BAD: whether CULLMODE (NONE 1, CW 2, CCW 3; others as CW on x64, as CCW on
x86) culls a triangle of this winding -/
def culls (mode : Nat) (x86 : Bool) (w : Winding) : Bool :=
  let m := if 1 ≤ mode ∧ mode ≤ 3 then mode else if x86 then 3 else 2
  if m = 1 then false
  else match w with
    | .flat => false
    | .ccw => m == 3
    | .cw => m == 2

/-- D3D9.CULL: NONE culls nothing; CW culls the clockwise triangles and CCW the others, so
each triangle with an area is drawn by exactly one of them -/
theorem cull_modes (x : Bool) (w : Winding) :
    culls 1 x w = false ∧ culls 2 x .cw = true ∧ culls 2 x .ccw = false ∧ culls 3 x .ccw = true ∧
    culls 3 x .cw = false ∧ (w ≠ .flat → culls 2 x w = !culls 3 x w) := by
  cases x <;> cases w <;> decide

/-- D3D9.CULL.BAD: a mode outside the enumeration culls as CW on x64 and as CCW on x86 -/
theorem cull_bad (mode : Nat) (w : Winding) (h : mode = 0 ∨ 3 < mode) :
    culls mode false w = culls 2 false w ∧ culls mode true w = culls 3 true w := by
  have : ¬(1 ≤ mode ∧ mode ≤ 3) := by omega
  simp only [culls, this, ↓reduceIte]
  cases w <;> decide

/-- D3DBLENDOP: ADD, SUBTRACT, REVSUBTRACT, MIN, MAX; another as MAX (x64: the x86 runtimes do not
draw, D3D9.BLEND.OPX86) -/
def blendOp (n : Nat) : Op :=
  match n with
  | 1 => .add
  | 2 => .sub
  | 3 => .revsub
  | 4 => .min
  | _ => .max

/-- the operation's number (D3DBLENDOP) -/
def opCode : Op → Nat
  | .add => 1
  | .sub => 2
  | .revsub => 3
  | .min => 4
  | .max => 5

/-- D3D9.BLEND.OP: each operation is its own number; a number outside ADD..MAX blends as MAX -/
theorem blend_op_bad (n : Nat) : (∀ o, blendOp (opCode o) = o) ∧ (n = 0 ∨ 5 < n → blendOp n = .max) := by
  refine ⟨fun o => by cases o <;> rfl, fun h => ?_⟩
  unfold blendOp
  match n, h with
  | 0, _ | 5, _ | _ + 6, _ => rfl
  | 1, h | 2, h | 3, h | 4, h => exact absurd h (by omega)

/-- the BLENDFACTOR color (ARGB) blends take: white on Direct3D 8, which has no such state -/
def blendColor (v8 : Bool) (state : Nat) : Nat := if v8 then 0xffffffff else state

/-- the four 8-bit channels of a color: red, green, blue, alpha -/
structure C4 where
  r : Int
  g : Int
  b : Int
  a : Int

/-- channel k (0 red .. 3 alpha) -/
def C4.get (c : C4) : Nat → Option Int
  | 0 => some c.r
  | 1 => some c.g
  | 2 => some c.b
  | 3 => some c.a
  | _ => none

/-- the channels of an ARGB color -/
def argb (c : Nat) : C4 := ⟨(c >>> 16) % 256, (c >>> 8) % 256, c % 256, c >>> 24⟩

/-- D3DBLEND on 8-bit channels: channel k's factor of the source s, destination d and
BLENDFACTOR c; none outside the enumeration and for BOTHSRCALPHA, BOTHINVSRCALPHA -/
def factor8 (f k : Nat) (s d : C4) (c : Nat) : Option Int :=
  match f with
  | 1 => some 0
  | 2 => some 255
  | 3 => s.get k
  | 4 => (s.get k).map (255 - ·)
  | 5 => some s.a
  | 6 => some (255 - s.a)
  | 7 => some d.a
  | 8 => some (255 - d.a)
  | 9 => d.get k
  | 10 => (d.get k).map (255 - ·)
  | 11 => if k < 3 then some (Min.min s.a (255 - d.a)) else some 255
  | 14 => (argb c).get k
  | 15 => ((argb c).get k).map (255 - ·)
  | _ => none

/-- D3D9.BLEND.FACTOR: on Direct3D 8 BLENDFACTOR is ONE and INVBLENDFACTOR ZERO, whatever the
state holds -/
theorem blend_factor_d3d8 (k : Nat) (s d : C4) (c : Nat) (hk : k < 4) :
    factor8 14 k s d (blendColor true c) = factor8 2 k s d c ∧
    factor8 15 k s d (blendColor true c) = factor8 1 k s d c := by
  match k, hk with
  | 0, _ | 1, _ | 2, _ | 3, _ => simp [factor8, blendColor, argb, C4.get]

/-- every channel 8-bit -/
def C4.bytes (x : C4) : Prop :=
  0 ≤ x.r ∧ x.r ≤ 255 ∧ 0 ≤ x.g ∧ x.g ≤ 255 ∧ 0 ≤ x.b ∧ x.b ≤ 255 ∧ 0 ≤ x.a ∧ x.a ≤ 255

private theorem argb_bytes (c : Nat) (hc : c < 2 ^ 32) : (argb c).bytes := by
  have ha : c >>> 24 < 256 := by
    rw [Nat.shiftRight_eq_div_pow]
    exact Nat.div_lt_of_lt_mul (by omega)
  simp only [C4.bytes, argb]
  omega

/-- D3D9.BLEND: every factor of 8-bit channels is 8-bit -/
theorem factor8_range (f k : Nat) (s d : C4) (c : Nat) (v : Int) (hc : c < 2 ^ 32)
    (hs : s.bytes) (hd : d.bytes) (h : factor8 f k s d c = some v) : 0 ≤ v ∧ v ≤ 255 := by
  have get_bytes : ∀ (x : C4) (v : Int), x.bytes → x.get k = some v → 0 ≤ v ∧ v ≤ 255 := by
    intro x v hx hv
    obtain ⟨_, _, _, _, _, _, _, _⟩ := hx
    match k, hv with
    | 0, hv | 1, hv | 2, hv | 3, hv => simp only [C4.get, Option.some.injEq] at hv; omega
    | _ + 4, hv => simp [C4.get] at hv
  have inv : ∀ (x : C4) (v : Int), x.bytes → (x.get k).map (255 - ·) = some v → 0 ≤ v ∧ v ≤ 255 := by
    intro x v hx hv
    obtain ⟨w, hw, rfl⟩ := Option.map_eq_some_iff.mp hv
    have := get_bytes x w hx hw
    omega
  have ha := argb_bytes c hc
  unfold C4.bytes at hs hd
  unfold factor8 at h
  split at h
  all_goals first
    | exact get_bytes _ _ (by unfold C4.bytes; omega) h
    | exact get_bytes _ _ ha h
    | exact inv _ _ (by unfold C4.bytes; omega) h
    | exact inv _ _ ha h
    | (split at h <;> simp only [Option.some.injEq] at h <;> subst h <;> omega)
    | (simp only [Option.some.injEq, reduceCtorEq] at h; subst h; omega)
    | simp at h

/-- the factors after BOTHSRCALPHA (12) and BOTHINVSRCALPHA (13) -/
def bothAlpha (sf df : Nat) (alpha : Bool) : Nat × Nat :=
  if !(sf = 12 ∨ sf = 13 ∨ df = 12 ∨ df = 13) then (sf, df)
  else if alpha then (if sf = 12 ∨ sf = 13 then 1 else sf, if df = 12 ∨ df = 13 then 1 else df)
  else if sf = 12 ∨ (sf ≠ 13 ∧ df = 12) then (5, 6)
  else (6, 5)

/-- D3D9.BLEND.BOTHALPHA: as the color's factors BOTHSRCALPHA makes the source factor SRCALPHA
and the destination's INVSRCALPHA, BOTHINVSRCALPHA the reverse, the source's winning where both
are named; as separate alpha factors either is ZERO, the other kept -/
theorem both_alpha (f g : Nat) (hg : g ≠ 12 ∧ g ≠ 13) :
    bothAlpha 12 f false = (5, 6) ∧ bothAlpha 13 f false = (6, 5) ∧ bothAlpha g 12 false = (5, 6) ∧
    bothAlpha g 13 false = (6, 5) ∧ bothAlpha 12 g true = (1, g) ∧ bothAlpha 13 g true = (1, g) ∧
    bothAlpha g 12 true = (g, 1) ∧ bothAlpha g 13 true = (g, 1) := by
  obtain ⟨h1, h2⟩ := hg
  simp [bothAlpha, h1, h2]

/-- D3D9.COLORWRITE: each channel the new one where its bit of the mask (1 red, 2 green, 4 blue,
8 alpha) is set, the old one where not -/
def colorWrite (mask : Nat) (n o : C4) : C4 :=
  ⟨if mask &&& 1 = 0 then o.r else n.r, if mask &&& 2 = 0 then o.g else n.g,
   if mask &&& 4 = 0 then o.b else n.b, if mask &&& 8 = 0 then o.a else n.a⟩

/-- D3D9.COLORWRITE: only the mask's low four bits count; all four write the new color, none of
them leave the old -/
theorem color_write (mask : Nat) (n o : C4) :
    colorWrite mask n o = colorWrite (mask % 16) n o ∧ colorWrite 15 n o = n ∧ colorWrite 0 n o = o := by
  refine ⟨?_, by simp [colorWrite], by simp [colorWrite]⟩
  have low (b : Nat) (hb : b < 16) : mask &&& b = (mask % 16) &&& b := by
    apply Nat.eq_of_testBit_eq
    intro i
    rw [Nat.testBit_and, Nat.testBit_and, show (16 : Nat) = 2 ^ 4 from rfl, Nat.testBit_mod_two_pow]
    by_cases hi : i < 4
    · simp [hi]
    · have : b.testBit i = false :=
        Nat.testBit_lt_two_pow (Nat.lt_of_lt_of_le (show b < 2 ^ 4 from hb) (Nat.pow_le_pow_right (n := 2) (by omega) (show 4 ≤ i by omega)))
      simp [this]
  simp only [colorWrite, low 1 (by omega), low 2 (by omega), low 4 (by omega), low 8 (by omega)]

def mirrors : List Mirror := [
  ⟨"d3d9.covers", fun
    | [ax, ay, bx, by', cx, cy, sx, sy] => pure [if covers (ax, ay) (bx, by') (cx, cy) (sx, sy) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d9.cull", fun
    | [m, x, w] => do
      let wi ← match w with | 0 => some Winding.cw | 1 => some .ccw | 2 => some .flat | _ => none
      pure [if culls (← nat m) (x != 0) wi then 1 else 0]
    | _ => none⟩,
  ⟨"d3d9.blendop", fun | [n] => do pure [opCode (blendOp (← nat n))] | _ => none⟩,
  ⟨"d3d9.factor8", fun
    | [f, k, sr, sg, sb, sa, dr, dg, db, da, v8, c] => do
      let col := blendColor (v8 != 0) (← nat c)
      pure ((factor8 (← nat f) (← nat k) ⟨sr, sg, sb, sa⟩ ⟨dr, dg, db, da⟩ col).toList)
    | _ => none⟩,
  ⟨"d3d9.bothalpha", fun
    | [s, d, a] => do
      let r := bothAlpha (← nat s) (← nat d) (a != 0)
      pure [r.1, r.2]
    | _ => none⟩,
  ⟨"d3d9.colorwrite", fun
    | [m, n0, n1, n2, n3, o0, o1, o2, o3] => do
      let r := colorWrite (← nat m) ⟨n0, n1, n2, n3⟩ ⟨o0, o1, o2, o3⟩
      pure [r.r, r.g, r.b, r.a]
    | _ => none⟩
]

end Kikizake.D3d9.Raster
