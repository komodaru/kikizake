import Kikizake.GdiPlus.Conv
/-! GDI+'s integer blending and conversion rules past GP.CONV's (spec/gdiplus.txt), as
src/model/gdiplus.rs computes them: `lanes` is `gdiplus::lanes` (GP.FILL.GAMMA.X86), `hatchMix`
`gdiplus::hatch_mix` (GP.PAINT.HATCH), `gammaAlpha234` `gdiplus::gamma_alpha_234`
(GP.PAINT.GAMMA.BETWEEN), `coverageShare` `gdiplus::coverage_share` (GP.PAINT.AA), `convert`
`gdiplus::convert` for every format but the indexed ones and 16bppGrayScale (GP.CONV.WIDE,
GP.CONV.PREMULTIPLIED.OPAQUE), `fixedPalette` `gdiplus::fixed_palette` (GP.CLONE.PALETTE);
mirrors `gp.lanes`, `gp.hatch_mix`, `gp.gamma234`, `gp.coverage_share`, `gp.convert`,
`gp.fixed_palette`. -/
namespace Kikizake.GdiPlus.Blend
open Kikizake.GdiPlus.Conv

/-- x86's d f / 8192 as 16-bit lanes make it: the product's high word times 8, plus its low word
with 4096 added and the carry out of 16 bits lost, >> 13 -/
def lanes (d f : Nat) : Nat := d * f / 65536 * 8 + (d * f % 65536 + 4096) % 65536 / 8192

/-- GP.FILL.GAMMA.X86: the lanes are d f / 8192 rounded, 8 lower exactly where the low word is
61440 or more (for every product) -/
theorem lanes_carry (d f : Nat) :
    lanes d f = if d * f % 65536 ≥ 61440 then (d * f + 4096) / 8192 - 8 else (d * f + 4096) / 8192 := by
  unfold lanes
  generalize d * f = p
  split <;> omega

/-- x86's gamma corrected SourceOver of one 16-bit word: s + d (8192 - sa) / 8192 in lanes, kept to
16 bits -/
def mix86 (s d sa : Nat) : Nat := (s + lanes d (8192 - sa)) % 65536

/-- GP.FILL.GAMMA.X86: a brush of alpha 254 over alpha 58 ..= 61 adds nothing (the lanes lose the
carry of the destination's part, where rounding would add 8: 253 back in 8 bits, not 254) -/
theorem alpha_254_adds_nothing :
    ∀ a : Fin 4, mix86 (widenAlpha 254) (widenAlpha (58 + a.val)) (widenAlpha 254) = widenAlpha 254 ∧
      (widenAlpha 254 * 8192 + widenAlpha (58 + a.val) * (8192 - widenAlpha 254) + 4096) / 8192 = widenAlpha 254 + 8 ∧
      narrowAlpha (widenAlpha 254) = 253 := by decide

/-- GP.PAINT.HATCH: a texel between the premultiplied fore and back channels -/
def hatchMix (f b w : Nat) : Nat := (f * w + b * (256 - w)) / 256

private theorem hatch_between (f b w : Nat) (hw : w ≤ 256) :
    min f b ≤ hatchMix f b w ∧ hatchMix f b w ≤ max f b := by
  unfold hatchMix
  have e : w + (256 - w) = 256 := by omega
  constructor
  · apply (Nat.le_div_iff_mul_le (by decide)).mpr
    have h1 : min f b * w ≤ f * w := Nat.mul_le_mul_right _ (Nat.min_le_left _ _)
    have h2 : min f b * (256 - w) ≤ b * (256 - w) := Nat.mul_le_mul_right _ (Nat.min_le_right _ _)
    have : min f b * 256 = min f b * w + min f b * (256 - w) := by rw [← Nat.mul_add, e]
    omega
  · apply Nat.div_le_of_le_mul
    have h1 : f * w ≤ max f b * w := Nat.mul_le_mul_right _ (Nat.le_max_left _ _)
    have h2 : b * (256 - w) ≤ max f b * (256 - w) := Nat.mul_le_mul_right _ (Nat.le_max_right _ _)
    have : max f b * 256 = max f b * w + max f b * (256 - w) := by rw [← Nat.mul_add, e]
    omega

/-- GP.PAINT.HATCH: the two colors' weights of the measured texels (64 and 234) -/
private theorem hatch_ends (f b : Nat) : hatchMix f b 0 = b ∧ hatchMix f f 64 = f ∧ hatchMix f f 234 = f := by
  unfold hatchMix; refine ⟨by omega, by omega, by omega⟩

/-- GP.PAINT.GAMMA.BETWEEN: gamma corrected, the alpha of a texel of weight 234 -/
def gammaAlpha234 (f b : Nat) : Nat :=
  if f = b then b else if f > b then b + (32 * (f - b) - 1) / 35 else b - (32 * (b - f) / 35 + 1)

/-- GP.PAINT.GAMMA.BETWEEN: back + (fore - back) 32 / 35, a hair less, floored: above the back
color b + floor((32 (f - b) - 1) / 35), below it b - floor(32 (b - f) / 35) - 1, and between the
two colors -/
theorem gamma234_floor (f b : Nat) (hf : f ≤ 255) (hb : b ≤ 255) :
    (f ≠ b → 35 * (gammaAlpha234 f b : Int) < 35 * b + 32 * ((f : Int) - b) ∧
               35 * b + 32 * ((f : Int) - b) ≤ 35 * (gammaAlpha234 f b : Int) + 35) ∧
    min f b ≤ gammaAlpha234 f b ∧ gammaAlpha234 f b ≤ max f b := by
  unfold gammaAlpha234
  by_cases e : f = b
  · subst e; simp
  · simp only [e, ite_false]
    split <;> omega

/-- GP.PAINT.GAMMA.BETWEEN: not f 234 + b 22, >> 8 as without gamma -/
theorem gamma234_not_hatch : gammaAlpha234 0 35 = 2 ∧ hatchMix 0 35 234 = 3 ∧
    gammaAlpha234 163 0 = 149 ∧ hatchMix 163 0 234 = 148 ∧ gammaAlpha234 255 0 = hatchMix 255 0 234 := by decide

/-- GP.PAINT.AA: a channel v times c of full subsamples, rounded, halves up -/
def coverageShare (v c full : Nat) : Nat := (2 * v * c + full) / (2 * full)

private theorem coverage_nearest (v c full : Nat) (h : 0 < full) (hc : c ≤ full) :
    2 * full * coverageShare v c full ≤ 2 * v * c + full ∧ 2 * v * c + full < 2 * full * (coverageShare v c full + 1) ∧
    coverageShare v c full ≤ v ∧ coverageShare v full full = v ∧ coverageShare v 0 full = 0 := by
  unfold coverageShare
  have d := Nat.div_add_mod (2 * v * c + full) (2 * full)
  have m := Nat.mod_lt (2 * v * c + full) (show 2 * full > 0 by omega)
  have vc : v * c ≤ v * full := Nat.mul_le_mul_left v hc
  have up : 2 * full * ((2 * v * c + full) / (2 * full) + 1) = 2 * full * ((2 * v * c + full) / (2 * full)) + 2 * full := Nat.mul_add_one _ _
  refine ⟨by omega, by omega, ?_, ?_, ?_⟩
  · have k : 2 * full * (v + 1) = 2 * (v * full) + 2 * full := by
      rw [Nat.mul_add, Nat.mul_one, Nat.mul_assoc, Nat.mul_comm full v]
    have : 2 * v * c = 2 * (v * c) := Nat.mul_assoc 2 v c
    have q := Nat.div_lt_of_lt_mul (m := 2 * v * c + full) (n := 2 * full) (k := v + 1) (by omega)
    omega
  · have : 2 * v * full + full = full + v * (2 * full) := by rw [Nat.mul_comm 2 v, Nat.mul_assoc]; omega
    rw [this, Nat.add_mul_div_right _ _ (by omega), Nat.div_eq_of_lt (by omega)]; simp
  · simp only [Nat.mul_zero, Nat.zero_add]; exact Nat.div_eq_of_lt (by omega)

def wide : Pf → Bool | .rgb48 | .argb64 | .pargb64 => true | _ => false
def isOpaque : Pf → Bool | .rgb555 | .rgb565 | .rgb24 | .rgb32 => true | _ => false
def peq : Pf → Pf → Bool
  | .rgb555, .rgb555 | .rgb565, .rgb565 | .argb1555, .argb1555 | .rgb24, .rgb24 | .rgb32, .rgb32
  | .argb32, .argb32 | .pargb32, .pargb32 | .rgb48, .rgb48 | .argb64, .argb64 | .pargb64, .pargb64 => true
  | _, _ => false

/-- a 16-bit channel pixel as straight 16-bit ARGB, the words as they are (a premultiplied one
undone in 32 bits) -/
def toArgb64 (f : Pf) (v : Nat) : Nat :=
  match f with
  | .rgb48 => 8192 * 2 ^ 48 + v % 2 ^ 48
  | .pargb64 => let a := level (bits v 48 16)
      let un := fun s => if a = 0 then bits v s 16 else undo16 (bits v s 16) a
      bits v 48 16 * 2 ^ 48 + un 32 * 2 ^ 32 + un 16 * 2 ^ 16 + un 0
  | _ => v
/-- and back: premultiplied (c a) >> 13 in 32 bits, kept to 16 bits -/
def fromArgb64 (f : Pf) (v : Nat) : Nat :=
  match f with
  | .rgb48 => v % 2 ^ 48
  | .pargb64 => let a := signed16 (bits v 48 16)
      let p := fun s => ((signed16 (bits v s 16) * a) / 8192 % 65536).toNat
      bits v 48 16 * 2 ^ 48 + p 32 * 2 ^ 32 + p 16 * 2 ^ 16 + p 0
  | _ => v

/-- LockBits' and Clone's conversion between two formats -/
def convert (a b : Pf) (v : Nat) : Option Nat :=
  if peq a b then some v
  else if wide a && wide b then some (fromArgb64 b (toArgb64 a v))
  else if peq a .pargb32 && isOpaque b then fromArgb b v
  else (toArgb a v).bind (fromArgb b)

/-- GP.CONV.WIDE: between the 16-bit channel formats the words as they are, through straight
64bppARGB: 48bppRGB gains alpha 8192 and loses it again, and never goes through 8 bits -/
theorem wide_words (v : Nat) (h : v < 2 ^ 48) :
    convert .rgb48 .argb64 v = some (8192 * 2 ^ 48 + v) ∧ convert .argb64 .rgb48 (8192 * 2 ^ 48 + v) = some v ∧
    (convert .rgb48 .argb64 v).bind (convert .argb64 .rgb48) = some v := by
  simp only [convert, peq, wide, isOpaque, Bool.and_self, ite_true, toArgb64, fromArgb64, Nat.mod_eq_of_lt h,
    Option.bind_some, Bool.false_eq_true, ite_false, Option.some.injEq]
  refine ⟨trivial, by omega, by omega⟩

/-- GP.CONV.WIDE: an opaque 64bppPARGB pixel (alpha 8192) is its straight words -/
theorem wide_opaque (c : Nat) (h : c < 32768) : undo16 c 8192 = c ∧ ((signed16 c * signed16 8192) / 8192 % 65536).toNat = c := by
  have hs : signed16 c = (c : Int) := by
    unfold signed16; simp only [Nat.mod_eq_of_lt (show c < 65536 by omega)]; split <;> omega
  have h8 : signed16 8192 = 8192 := by decide
  unfold undo16
  simp only [show (8192 : Nat) ≠ 0 by decide, ite_false, hs, h8]
  have e1 : ((c : Int) * 8192).tdiv ((8192 : Nat) : Int) = c := Int.mul_tdiv_cancel _ (by decide)
  have e2 : ((c : Int) * 8192) / 8192 = c := Int.mul_ediv_cancel _ (by decide)
  rw [e1, e2]
  constructor <;> omega

/-- GP.CONV.PREMULTIPLIED.OPAQUE: 32bppPARGB to 24bppRGB and 32bppRGB the premultiplied bytes as
they are (over black), the alpha byte kept for 32bppRGB; to 16bppRGB565 its channels truncated;
to 48bppRGB it is undone first -/
theorem pargb_opaque (v : Nat) (h : v < 2 ^ 32) :
    convert .pargb32 .rgb24 v = some (v % 2 ^ 24) ∧ convert .pargb32 .rgb32 v = some v ∧
    convert .pargb32 .rgb565 v = some ((byte v 16 / 8) * 2048 + (byte v 8 / 4) * 32 + byte v 0 / 8) ∧
    convert .pargb32 .rgb48 v = (toArgb .pargb32 v).bind (fromArgb .rgb48) := by
  simp only [convert, peq, isOpaque, wide, fromArgb, Nat.mod_eq_of_lt h, Bool.false_eq_true, ite_false,
    Bool.and_false, Bool.and_true, ite_true]
  exact ⟨trivial, trivial, trivial, trivial⟩

/-- the palettes a clone into another indexed format gets: entry k of 1 bpp FixedBW, 4 bpp
FixedHalftone8, 8 bpp FixedHalftone216 (depth 1, 4, 8), and the flags -/
def more : List Nat := [0xffc0c0c0, 0xff808080, 0xff800000, 0xff008000, 0xff000080, 0xff808000, 0xff800080, 0xff008080]
def fixedPalette : Nat → List Nat
  | 1 => [0xff000000, 0xffffffff]
  | 4 => (List.range 8).map (fun k => 0xff000000 + k / 4 * 0xff0000 + k / 2 % 2 * 0xff00 + k % 2 * 0xff) ++ more
  | 8 => (List.range 216).map (fun k => 0xff000000 + k / 36 * 0x330000 + k / 6 % 6 * 0x3300 + k % 6 * 0x33) ++ more
  | _ => []
def fixedFlags : Nat → Nat | 1 => 0x200 | 4 => 0x300 | 8 => 0x700 | _ => 0

/-- GP.CLONE.PALETTE: FixedBW black and white; FixedHalftone8 the 8 corners of the cube, then
c0c0c0 808080 800000 008000 000080 808000 800080 008080; FixedHalftone216 the 6x6x6 cube in steps
of 0x33, red slowest, then the same 8 (224 entries); the palette type in the flags' bits 8 and up
(FixedBW 2, FixedHalftone8 3, FixedHalftone216 7) -/
theorem fixed_palettes :
    fixedPalette 1 = [0xff000000, 0xffffffff] ∧
    fixedPalette 4 = [0xff000000, 0xff0000ff, 0xff00ff00, 0xff00ffff, 0xffff0000, 0xffff00ff, 0xffffff00, 0xffffffff] ++ more ∧
    (fixedPalette 8).length = 224 ∧ (fixedPalette 8).drop 216 = more ∧ (fixedPalette 4).drop 8 = more ∧
    (∀ r g b : Fin 6, (fixedPalette 8)[36 * r.val + 6 * g.val + b.val]? = some (0xff000000 + r.val * 0x330000 + g.val * 0x3300 + b.val * 0x33)) ∧
    fixedFlags 1 = 2 * 256 ∧ fixedFlags 4 = 3 * 256 ∧ fixedFlags 8 = 7 * 256 := by decide +kernel

private def fmt (i : Int) : Option Pf := do pf (← nat i)

def mirrors : List Mirror := [
  ⟨"gp.lanes", fun | [d, f] => do pure [lanes (← nat d) (← nat f)] | _ => none⟩,
  ⟨"gp.hatch_mix", fun | [f, b, w] => do pure [hatchMix (← nat f) (← nat b) (← nat w)] | _ => none⟩,
  ⟨"gp.gamma234", fun | [f, b] => do pure [gammaAlpha234 (← nat f) (← nat b)] | _ => none⟩,
  ⟨"gp.coverage_share", fun | [v, c, full] => do pure [coverageShare (← nat v) (← nat c) (← nat full)] | _ => none⟩,
  ⟨"gp.convert", fun | [a, b, v] => do pure [← convert (← fmt a) (← fmt b) (← nat v)] | _ => none⟩,
  ⟨"gp.fixed_palette", fun | [d, k] => do
      let p := fixedPalette (← nat d)
      if k = -1 then pure [p.length, fixedFlags (← nat d)] else pure [← p[(← nat k)]?] | _ => none⟩
]

end Kikizake.GdiPlus.Blend
