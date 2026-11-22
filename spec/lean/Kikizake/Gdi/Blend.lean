import Kikizake.Mirror
/-! GDI's AlphaBlend (spec/gdiblend.txt), as src/model/gdiblend.rs computes it: `mix` is
`gdiblend::mix`, `alpha` is `gdiblend::alpha`; `kikizake lean` holds them equal on every channel
triple and on sampled pixels. -/
namespace Kikizake.Gdi.Blend

/-- GDI.ALPHA.ROUND: one channel at a constant alpha -/
def mix (s d a : Nat) : Nat := (s * a + d * (255 - a) + 127) / 255

/-- the documented value (GDI.ALPHA.CONSTANT) times 255: Src * SCA + Dst * (255 - SCA) -/
def exact (s d a : Nat) : Nat := s * a + d * (255 - a)

/-- GDI.ALPHA.ROUND is the documented formula rounded to the nearest integer: 255 being odd,
no value lies halfway, so it is within 127/255 of Src * SCA / 255 + Dst * (1 - SCA / 255) -/
theorem mix_is_nearest (s d a : Nat) :
    255 * mix s d a ≤ exact s d a + 127 ∧ exact s d a ≤ 255 * mix s d a + 127 := by
  unfold mix exact; omega

private theorem exact_between {s d a : Nat} (ha : a ≤ 255) :
    s * 255 ≤ exact s d a ∨ d * 255 ≤ exact s d a := by
  unfold exact
  have h : s * a + s * (255 - a) = s * 255 := by rw [← Nat.mul_add]; congr 1; omega
  have h' : d * a + d * (255 - a) = d * 255 := by rw [← Nat.mul_add]; congr 1; omega
  rcases Nat.le_total s d with sd | ds
  · have := Nat.mul_le_mul_right (255 - a) sd; omega
  · have := Nat.mul_le_mul_right a ds; omega

/-- GDI.ALPHA.CONSTANT: each channel lies between the source's and the destination's -/
theorem mix_between (s d a : Nat) (ha : a ≤ 255) :
    min s d ≤ mix s d a ∧ mix s d a ≤ max s d := by
  have hs : s * a + s * (255 - a) = s * 255 := by rw [← Nat.mul_add]; congr 1; omega
  have hd : d * a + d * (255 - a) = d * 255 := by rw [← Nat.mul_add]; congr 1; omega
  unfold mix
  rcases Nat.le_total s d with sd | ds
  · have l := Nat.mul_le_mul_right (255 - a) sd
    have u := Nat.mul_le_mul_right a sd
    simp only [Nat.min_eq_left sd, Nat.max_eq_right sd]; omega
  · have l := Nat.mul_le_mul_right a ds
    have u := Nat.mul_le_mul_right (255 - a) ds
    simp only [Nat.min_eq_right ds, Nat.max_eq_left ds]; omega

/-- GDI.ALPHA.CONSTANT: SCA 255 gives the source, 0 the destination -/
theorem mix_ends (s d : Nat) : mix s d 255 = s ∧ mix s d 0 = d := by
  unfold mix; omega

/-- a channel of a 32 bpp value (0xAARRGGBB, channel 0 blue) -/
def ch (v i : Nat) : Nat := v / 256 ^ i % 256

/-- four channels as one 32 bit word, each added in at its place (a channel over 255 carries
into the next, the word wraps at 2^32) -/
def word (c0 c1 c2 c3 : Nat) : Nat := (c0 + 256 * c1 + 65536 * c2 + 16777216 * c3) % 4294967296

/-- GDI.ALPHA.CARRY: one channel of AC_SRC_ALPHA's sum before it is added into the word -/
def carryChannel (c d sa sca : Nat) : Nat := (c * sca + 127) / 255 + (d * (255 - sa) + 127) / 255

/-- AlphaBlend of one pixel (GDI.ALPHA.CONSTANT, .PERPIXEL, .OPAQUE, .CARRY) -/
def alpha (s d sca : Nat) (perPixel : Bool) : Nat :=
  let m := fun i => mix (ch s i) (ch d i) sca
  if !perPixel || ch s 3 = 255 then word (m 0) (m 1) (m 2) (m 3)
  else
    let sa := (ch s 3 * sca + 127) / 255
    let k := fun i => carryChannel (ch s i) (ch d i) sa sca
    word (k 0) (k 1) (k 2) (k 3)

/-- the word's channels are the four values when none is over 255 -/
theorem word_channels (c0 c1 c2 c3 : Nat) (h0 : c0 ≤ 255) (h1 : c1 ≤ 255) (h2 : c2 ≤ 255) (h3 : c3 ≤ 255) :
    ch (word c0 c1 c2 c3) 0 = c0 ∧ ch (word c0 c1 c2 c3) 1 = c1 ∧
    ch (word c0 c1 c2 c3) 2 = c2 ∧ ch (word c0 c1 c2 c3) 3 = c3 := by
  unfold ch word; simp only [Nat.pow_zero, Nat.pow_succ, Nat.one_mul]; omega

theorem mix_le (s d a : Nat) (hs : s ≤ 255) (hd : d ≤ 255) (ha : a ≤ 255) : mix s d a ≤ 255 := by
  have := (mix_between s d a ha).2; omega

theorem ch_le (v i : Nat) : ch v i ≤ 255 := by unfold ch; omega

/-- GDI.ALPHA.CONSTANT: at a constant alpha every channel of the result is the channel mix -/
theorem alpha_constant (s d sca : Nat) (ha : sca ≤ 255) (i : Nat) (hi : i < 4) :
    ch (alpha s d sca false) i = mix (ch s i) (ch d i) sca := by
  have b := fun j => mix_le (ch s j) (ch d j) sca (ch_le s j) (ch_le d j) ha
  have w := word_channels _ _ _ _ (b 0) (b 1) (b 2) (b 3)
  simp only [alpha, Bool.not_false, Bool.true_or, ite_true]
  match i, hi with
  | 0, _ => exact w.1
  | 1, _ => exact w.2.1
  | 2, _ => exact w.2.2.1
  | 3, _ => exact w.2.2.2

/-- GDI.ALPHA.OPAQUE: a source pixel of alpha 255 blends as the constant alpha alone -/
theorem alpha_opaque (s d sca : Nat) (h : ch s 3 = 255) : alpha s d sca true = alpha s d sca false := by
  simp [alpha, h]

/-- GDI.ALPHA.PERPIXEL: for a premultiplied source (no channel above its alpha) no channel
carries, and each is Src * SCA / 255 + (1 - Src.alpha / 255) * Dst, both terms rounded -/
theorem carry_premultiplied (c d a sca : Nat) (hc : c ≤ a) (ha : a ≤ 255) (hd : d ≤ 255) (hs : sca ≤ 255) :
    carryChannel c d ((a * sca + 127) / 255) sca ≤ 255 := by
  unfold carryChannel
  have h1 : c * sca ≤ a * sca := Nat.mul_le_mul_right sca hc
  have h2 : a * sca ≤ 255 * 255 := Nat.mul_le_mul ha hs
  have h3 : d * (255 - (a * sca + 127) / 255) ≤ 255 * (255 - (a * sca + 127) / 255) :=
    Nat.mul_le_mul_right _ hd
  omega

/-- the pixel version: every channel of a premultiplied source's blend is its own sum -/
theorem alpha_premultiplied (s d sca : Nat) (hsca : sca ≤ 255)
    (hp : ch s 0 ≤ ch s 3 ∧ ch s 1 ≤ ch s 3 ∧ ch s 2 ≤ ch s 3) (ho : ch s 3 ≠ 255) (i : Nat) (hi : i < 4) :
    ch (alpha s d sca true) i = carryChannel (ch s i) (ch d i) ((ch s 3 * sca + 127) / 255) sca := by
  let sa := (ch s 3 * sca + 127) / 255
  have b : ∀ j, ch s j ≤ ch s 3 → carryChannel (ch s j) (ch d j) sa sca ≤ 255 := fun j hj =>
    carry_premultiplied _ _ _ _ hj (ch_le s 3) (ch_le d j) hsca
  have w := word_channels _ _ _ _ (b 0 hp.1) (b 1 hp.2.1) (b 2 hp.2.2) (b 3 (Nat.le_refl _))
  have e : alpha s d sca true = word (carryChannel (ch s 0) (ch d 0) sa sca) (carryChannel (ch s 1) (ch d 1) sa sca)
      (carryChannel (ch s 2) (ch d 2) sa sca) (carryChannel (ch s 3) (ch d 3) sa sca) := by
    simp [alpha, ho, sa]
  rw [e]
  match i, hi with
  | 0, _ => exact w.1
  | 1, _ => exact w.2.1
  | 2, _ => exact w.2.2.1
  | 3, _ => exact w.2.2.2

def mirrors : List Mirror := [
  ⟨"gdi.mix", fun
    | [s, d, a] => do pure [mix (← nat s) (← nat d) (← nat a)]
    | _ => none⟩,
  ⟨"gdi.alpha", fun
    | [s, d, a, p] => do pure [alpha (← nat s) (← nat d) (← nat a) (p != 0)]
    | _ => none⟩
]

end Kikizake.Gdi.Blend
