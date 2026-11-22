import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's texels as src/model/d3d9draw.rs reads them: `texel` `dxt_block_texel` of a DXT
block, its pieces `widen` `dxt_widen`, `mix` `dxt_mix`, `alpha` `dxt_alpha` (D3D9.TEX.DXT);
`toRgba` `to_rgba`, the channels a format lacks (D3D9.TEX.MISSING), on the channels' binary64
bits; `levelBytes` `level_bytes` and `createLevel` `create_level`, CreateTexture's one level
(D3D9.TEX.CREATE) (mirrors `d3d9.dxt`, `d3d9.rgba`, `d3d9.level`). -/
namespace Kikizake.D3d9.Texel

/-- a 5-bit (6-bit) endpoint channel widened to 8 bits by repeating its top bits -/
def widen (c bits : Nat) : Nat := if bits = 5 then c * 8 + c / 4 else c * 4 + c / 16

/-- D3D9.TEX.DXT: a widened channel is the channel's bits followed by its top bits: 0 stays 0,
the largest becomes 255, and a larger channel is a larger 8-bit one -/
theorem widen_top_bits :
    (∀ c, c < 32 → widen c 5 < 256 ∧ widen c 5 / 8 = c ∧ widen c 5 % 8 = c / 4) ∧
    (∀ c, c < 64 → widen c 6 < 256 ∧ widen c 6 / 4 = c ∧ widen c 6 % 4 = c / 16) ∧
    widen 0 5 = 0 ∧ widen 31 5 = 255 ∧ widen 0 6 = 0 ∧ widen 63 6 = 255 ∧
    (∀ c, c < 31 → widen c 5 < widen (c + 1) 5) ∧ (∀ c, c < 63 → widen c 6 < widen (c + 1) 6) := by
  refine ⟨fun c h => ?_, fun c h => ?_, by decide, by decide, by decide, by decide, fun c h => ?_, fun c h => ?_⟩ <;>
    simp only [widen] <;> simp <;> omega

/-- a color channel of the endpoints' a and b by the texel's index: a, b, the thirds as 171/256
and 85/256 taken down (four colors), the half taken down and 0 (DXT1's three) -/
def mix (idx : Nat) (four : Bool) (a b : Nat) : Nat :=
  if idx = 0 then a else if idx = 1 then b
  else if four then (if idx = 2 then (171 * a + 85 * b) / 256 else if idx = 3 then (85 * a + 171 * b) / 256 else 0)
  else if idx = 2 then (a + b) / 2 else 0

/-- D3D9.TEX.DXT: the thirds and the half lie between the endpoints and never above the exact
mix (taken down), less than a unit under it -/
theorem mix_between (a b : Nat) :
    (Nat.min a b ≤ mix 2 true a b ∧ mix 2 true a b ≤ Nat.max a b ∧ 256 * mix 2 true a b ≤ 171 * a + 85 * b ∧ 171 * a + 85 * b < 256 * (mix 2 true a b + 1)) ∧
    (Nat.min a b ≤ mix 3 true a b ∧ mix 3 true a b ≤ Nat.max a b ∧ 256 * mix 3 true a b ≤ 85 * a + 171 * b ∧ 85 * a + 171 * b < 256 * (mix 3 true a b + 1)) ∧
    (Nat.min a b ≤ mix 2 false a b ∧ mix 2 false a b ≤ Nat.max a b ∧ 2 * mix 2 false a b ≤ a + b) ∧
    mix 3 false a b = 0 ∧ mix 0 true a b = a ∧ mix 1 false a b = b := by
  simp only [mix, Nat.min_def, Nat.max_def]; simp; (repeat' split) <;> omega

/-- a DXT4/5 alpha of the endpoints a0, a1 by its 3-bit index: sevenths where a0 > a1, else
fifths and 0, 255, each rounded to nearest -/
def alpha (ai a0 a1 : Nat) : Nat :=
  let m (wa wb : Nat) := (wa * a0 + wb * a1 + (wa + wb) / 2) / (wa + wb)
  if ai = 0 then a0 else if ai = 1 then a1
  else if a1 < a0 then m (8 - ai) (ai - 1)
  else if ai = 6 then 0 else if ai = 7 then 255 else m (6 - ai) (ai - 1)

/-- D3D9.TEX.DXT: DXT4/5's alphas are the endpoints' sevenths (a0 > a1) and fifths rounded to
nearest: within half a step of the exact mix; and 0, 255 for the fifths' last two -/
theorem alpha_nearest (a0 a1 i : Nat) (hi : 2 ≤ i ∧ i ≤ 7) :
    (a1 < a0 → 2 * (7 * alpha i a0 a1) + 7 ≥ 2 * ((8 - i) * a0 + (i - 1) * a1) ∧
      2 * (7 * alpha i a0 a1) ≤ 2 * ((8 - i) * a0 + (i - 1) * a1) + 7) ∧
    (a0 ≤ a1 → i ≤ 5 → 2 * (5 * alpha i a0 a1) + 5 ≥ 2 * ((6 - i) * a0 + (i - 1) * a1) ∧
      2 * (5 * alpha i a0 a1) ≤ 2 * ((6 - i) * a0 + (i - 1) * a1) + 5) ∧
    (a0 ≤ a1 → alpha 6 a0 a1 = 0 ∧ alpha 7 a0 a1 = 255) := by
  have h : i = 2 ∨ i = 3 ∨ i = 4 ∨ i = 5 ∨ i = 6 ∨ i = 7 := by omega
  rcases h with rfl | rfl | rfl | rfl | rfl | rfl <;> simp only [alpha] <;>
    refine ⟨fun h => ?_, fun h h' => ?_, fun h => ?_⟩ <;> simp <;> (repeat' split) <;> omega

/-- k bytes from i, little-endian -/
def le : List Nat → Nat → Nat → Option Nat
  | _, _, 0 => some 0
  | b, i, k + 1 => do let x ← b[i]?; let r ← le b (i + 1) k; pure (x + 256 * r)

/-- texel t (0..15, row by row) of a DXT1..DXT5 block as 8-bit r, g, b, a: the color block's
565 endpoints widened, the texel's 2-bit index into them (four colors, or DXT1's three where
c0 <= c1, its index 3 transparent black); DXT2/3 alpha 4 bits times 17, DXT4/5 `alpha` -/
def texel (n : Nat) (b : List Nat) (t : Nat) : Option (List Nat) := do
  if 15 < t then none else
  let cb := if n = 1 then 0 else 8
  let c0 ← le b cb 2
  let c1 ← le b (cb + 2) 2
  let idx := (← le b (cb + 4) 4) / 4 ^ t % 4
  let ex (c : Nat) : List Nat := [widen (c / 2048 % 32) 5, widen (c / 32 % 64) 6, widen (c % 32) 5]
  let four := n != 1 || decide (c1 < c0)
  let col := List.zipWith (mix idx four) (ex c0) (ex c1)
  let a ← if n = 1 then pure (if !four && idx = 3 then 0 else 255)
    else if n = 2 ∨ n = 3 then do pure ((← le b 0 8) / 16 ^ t % 16 * 17)
    else do pure (alpha ((← le b 2 6) / 8 ^ t % 8) (← b[0]?) (← b[1]?))
  pure (col ++ [a])

/-- D3D9.TEX.DXT: DXT2/3's 4-bit alphas times 17 are the 8-bit ones of the same value
(k / 15 = 17 k / 255), 0 to 255 -/
theorem alpha4 (k : Nat) (h : k < 16) : k * 17 ≤ 255 ∧ k * 17 * 15 = k * 255 := by omega

/-- the channels a format has (D3DFORMAT's letters as ASCII: r g b a, u v w q signed, l
luminance), their values' binary64 bits; the color channels, the alpha, the luminance -/
abbrev Slots := Option Nat × Option Nat × Option Nat × Option Nat × Option Nat

def put (s : Slots) (e : Nat × Nat) : Slots :=
  let (r, g, b, a, l) := s
  let (c, v) := e
  if c = 114 ∨ c = 117 then (some v, g, b, a, l)
  else if c = 103 ∨ c = 118 then (r, some v, b, a, l)
  else if c = 98 ∨ c = 119 then (r, g, some v, a, l)
  else if c = 97 ∨ c = 113 then (r, g, b, some v, l)
  else if c = 108 then (r, g, b, a, some v) else s

def one64 : Nat := 0x3ff0000000000000

/-- a texel's r, g, b, a: luminance in the color channels the format lacks; then a missing
color channel 1 (0 where the format has none: A8), a missing alpha 1 -/
def toRgba (ch : List (Nat × Nat)) : List Nat :=
  let (r, g, b, a, l) := ch.foldl put (none, none, none, none, none)
  let fill (c : Option Nat) := match c with | some v => some v | none => l
  let (r, g, b) := (fill r, fill g, fill b)
  let miss := if r.isNone && g.isNone && b.isNone then 0 else one64
  [r.getD miss, g.getD miss, b.getD miss, a.getD one64]

private theorem keeps_alpha (ch : List (Nat × Nat)) (h : ∀ e ∈ ch, e.1 ≠ 97 ∧ e.1 ≠ 113) :
    ∀ s : Slots, s.2.2.2.1 = none → (ch.foldl put s).2.2.2.1 = none := by
  induction ch with
  | nil => intro s hs; exact hs
  | cons e t ih =>
    intro s hs
    have he := h e (by simp)
    apply ih (fun x hx => h x (by simp [hx]))
    obtain ⟨r, g, b, a, l⟩ := s
    obtain ⟨c, v⟩ := e
    simp only at hs he
    simp only [put]
    split <;> (try split) <;> (try split) <;> (try split) <;> (try split) <;> simp_all

/-- D3D9.TEX.MISSING: a format of no alpha channel samples alpha 1 -/
theorem no_alpha (ch : List (Nat × Nat)) (h : ∀ e ∈ ch, e.1 ≠ 97 ∧ e.1 ≠ 113) :
    (toRgba ch)[3]? = some one64 := by
  have := keeps_alpha ch h (none, none, none, none, none) rfl
  simp only [toRgba]
  revert this
  generalize ch.foldl put (none, none, none, none, none) = s
  obtain ⟨r, g, b, a, l⟩ := s
  intro h; simp_all

/-- D3D9.TEX.MISSING: the formats' shapes: A8 color 0; luminance in red, green and blue (L8,
A8L8), and after the bump channels (L6V5U5); other color channels missing 1 (G16R16, V8U8,
R5G6B5 without alpha) -/
theorem missing (r g b a l : Nat) :
    toRgba [(97, a)] = [0, 0, 0, a] ∧ toRgba [(108, l)] = [l, l, l, one64] ∧
    toRgba [(108, l), (97, a)] = [l, l, l, a] ∧ toRgba [(117, r), (118, g), (108, l)] = [r, g, l, one64] ∧
    toRgba [(114, r), (103, g)] = [r, g, one64, one64] ∧ toRgba [(117, r), (118, g)] = [r, g, one64, one64] ∧
    toRgba [(98, b), (103, g), (114, r)] = [r, g, b, one64] ∧ toRgba [(98, b), (103, g), (114, r), (120, a)] = [r, g, b, one64] := by
  simp [toRgba, put]

/-- the bytes of a texture's one level: rows of w pixels of bpp bytes, or of a DXT format rows of
4 x 4 blocks of bpp bytes, a part block whole -/
def levelBytes (dxt : Bool) (bpp w h : Nat) : Nat := if dxt then (w + 3) / 4 * bpp * ((h + 3) / 4) else h * (w * bpp)

/-- CreateTexture's level: refused for a format WARP does not take (INVALIDCALL), else its bytes -/
def createLevel (taken dxt : Bool) (bpp w h : Nat) : Option Nat := if taken then some (levelBytes dxt bpp w h) else none

private theorem slot {i n bpp : Nat} (h : i + 1 ≤ n) : i * bpp + bpp ≤ n * bpp := by
  have := Nat.mul_le_mul_right bpp h
  rwa [Nat.add_mul, Nat.one_mul] at this

/-- D3D9.TEX.CREATE: the one level locks as rows of its format: each pixel (x, y) of a w x h
level at (y w + x) bpp, its bytes inside the level, and each DXT texel's 4 x 4 block at
(y / 4 * blocks a row + x / 4) bpp inside it too; a format not taken makes no texture -/
theorem level_rows (bpp w h x y : Nat) (hx : x < w) (hy : y < h) :
    (y * w + x) * bpp + bpp ≤ levelBytes false bpp w h ∧
    (y / 4 * ((w + 3) / 4) + x / 4) * bpp + bpp ≤ levelBytes true bpp w h ∧
    createLevel false false bpp w h = none ∧ createLevel false true bpp w h = none := by
  refine ⟨?_, ?_, rfl, rfl⟩
  · have : y * w + x + 1 ≤ h * w := by
      have := Nat.mul_le_mul_right w (show y + 1 ≤ h by omega)
      rw [Nat.add_mul, Nat.one_mul] at this; omega
    have e : levelBytes false bpp w h = h * w * bpp := by simp [levelBytes, Nat.mul_assoc]
    rw [e]; exact slot this
  · have hb : y / 4 * ((w + 3) / 4) + x / 4 + 1 ≤ (h + 3) / 4 * ((w + 3) / 4) := by
      have := Nat.mul_le_mul_right ((w + 3) / 4) (show y / 4 + 1 ≤ (h + 3) / 4 by omega)
      rw [Nat.add_mul, Nat.one_mul] at this; omega
    have e : levelBytes true bpp w h = (h + 3) / 4 * ((w + 3) / 4) * bpp := by
      simp only [levelBytes, ↓reduceIte]
      rw [Nat.mul_comm ((w + 3) / 4) bpp, Nat.mul_assoc, Nat.mul_comm bpp, Nat.mul_comm ((h + 3) / 4), Nat.mul_assoc]
    rw [e]; exact slot hb

/-- a channel's letter and its binary64 from its halves, each in turn -/
def pairs : List Int → Option (List (Nat × Nat))
  | c :: hi :: lo :: r => do pure ((← nat c, F64.join (← nat hi) (← nat lo)) :: (← pairs r))
  | [] => some []
  | _ => none

def mirrors : List Mirror := [
  ⟨"d3d9.dxt", fun
    | n :: t :: b => do
      match texel (← nat n) (← b.mapM nat) (← nat t) with
      | some v => pure (v.map Int.ofNat)
      | none => pure []
    | _ => none⟩,
  ⟨"d3d9.rgba", fun l => do
    let v := toRgba (← pairs l)
    pure (v.flatMap fun x => F64.halves (F64.canon x))⟩,
  ⟨"d3d9.level", fun
    | [taken, dxt, bpp, w, h] => do
      match createLevel (taken != 0) (dxt != 0) (← nat bpp) (← nat w) (← nat h) with
      | some k => pure [0, k]
      | none => pure [1, 0x8876086c]
    | _ => none⟩
]

end Kikizake.D3d9.Texel
