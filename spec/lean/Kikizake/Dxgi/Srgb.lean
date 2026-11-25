import Kikizake.F32

/-! WARP reads sRGB through a table and writes it after quantizing the linear value to UNORM12.
The write curve is evaluated by comparing integer powers at the half-code boundaries. -/
namespace Kikizake.Dxgi.Srgb

open Kikizake

-- Extracted from ld-table-29 and ld-table-91 in the d3d11draw 32/64-bit recordings.
private def warpRead : Array Nat := #[
  0x00000000, 0x399f22b4, 0x3a1f22b4, 0x3a6eb40f, 0x3a9f22b4, 0x3ac6eb61, 0x3aeeb40f, 0x3b0b3e5e,
  0x3b1f22b4, 0x3b33070b, 0x3b46eb61, 0x3b5b518d, 0x3b70f18d, 0x3b83e1c6, 0x3b8fe616, 0x3b9c87fd,
  0x3ba9c9b7, 0x3bb7ad6f, 0x3bc63549, 0x3bd56361, 0x3be539c1, 0x3bf5ba70, 0x3c0373b5, 0x3c0c6152,
  0x3c15a706, 0x3c1f45be, 0x3c293e6b, 0x3c3391f7, 0x3c3e414b, 0x3c494d46, 0x3c54b6ca, 0x3c607eb4,
  0x3c6ca5df, 0x3c792d22, 0x3c830aa8, 0x3c89afa0, 0x3c9085dd, 0x3c978dc7, 0x3c9ec7c2, 0x3ca63433,
  0x3cadd37d, 0x3cb5a601, 0x3cbdac22, 0x3cc5e63b, 0x3cce54ad, 0x3cd6f7d5, 0x3cdfd010, 0x3ce8ddb9,
  0x3cf2212e, 0x3cfb9ac6, 0x3d02a56c, 0x3d0798df, 0x3d0ca7e7, 0x3d11d2b0, 0x3d171965, 0x3d1c7c31,
  0x3d21fb3c, 0x3d2796b2, 0x3d2d4ebb, 0x3d332380, 0x3d39152e, 0x3d3f23e6, 0x3d454fd4, 0x3d4b991f,
  0x3d51ffef, 0x3d58846a, 0x3d5f26b7, 0x3d65e6fe, 0x3d6cc564, 0x3d73c20f, 0x3d7add25, 0x3d810b66,
  0x3d84b793, 0x3d887330, 0x3d8c3e4a, 0x3d9018f6, 0x3d940345, 0x3d97fd4a, 0x3d9c0716, 0x3da020bb,
  0x3da44a4b, 0x3da883d7, 0x3daccd70, 0x3db12728, 0x3db59110, 0x3dba0b38, 0x3dbe95b3, 0x3dc33092,
  0x3dc7dbe2, 0x3dcc97b6, 0x3dd1641f, 0x3dd6412c, 0x3ddb2eef, 0x3de02d77, 0x3de53cd4, 0x3dea5d19,
  0x3def8e56, 0x3df4d093, 0x3dfa23e8, 0x3dff8861, 0x3e027f07, 0x3e054280, 0x3e080ea5, 0x3e0ae379,
  0x3e0dc107, 0x3e10a755, 0x3e13966c, 0x3e168e53, 0x3e198f11, 0x3e1c98ae, 0x3e1fab32, 0x3e22c6a3,
  0x3e25eb09, 0x3e29186c, 0x3e2c4ed2, 0x3e2f8e45, 0x3e32d6c8, 0x3e362865, 0x3e398322, 0x3e3ce706,
  0x3e405419, 0x3e43ca62, 0x3e4749e8, 0x3e4ad2b1, 0x3e4e64c6, 0x3e52002b, 0x3e55a4e9, 0x3e595307,
  0x3e5d0a8b, 0x3e60cb7c, 0x3e6495e0, 0x3e6869bf, 0x3e6c4720, 0x3e702e08, 0x3e741e7f, 0x3e78188c,
  0x3e7c1c38, 0x3e8014c2, 0x3e82203c, 0x3e84308d, 0x3e8645ba, 0x3e885fc5, 0x3e8a7eb2, 0x3e8ca283,
  0x3e8ecb3d, 0x3e90f8e1, 0x3e932b74, 0x3e9562f8, 0x3e979f71, 0x3e99e0e2, 0x3e9c274e, 0x3e9e72b7,
  0x3ea0c322, 0x3ea31892, 0x3ea57308, 0x3ea7d289, 0x3eaa3718, 0x3eaca0b7, 0x3eaf0f69, 0x3eb18333,
  0x3eb3fc16, 0x3eb67a15, 0x3eb8fd34, 0x3ebb8576, 0x3ebe12e1, 0x3ec0a571, 0x3ec33d2d, 0x3ec5da17,
  0x3ec87c33, 0x3ecb2383, 0x3ecdd00b, 0x3ed081cd, 0x3ed338cc, 0x3ed5f50b, 0x3ed8b68d, 0x3edb7d54,
  0x3ede4965, 0x3ee11ac1, 0x3ee3f16b, 0x3ee6cd67, 0x3ee9aeb6, 0x3eec955d, 0x3eef815d, 0x3ef272ba,
  0x3ef56976, 0x3ef86594, 0x3efb6717, 0x3efe6e02, 0x3f00bd2b, 0x3f02460c, 0x3f03d1a5, 0x3f055ff9,
  0x3f06f106, 0x3f0884d1, 0x3f0a1b57, 0x3f0bb49d, 0x3f0d50a2, 0x3f0eef69, 0x3f1090f2, 0x3f123540,
  0x3f13dc53, 0x3f15862d, 0x3f1732cf, 0x3f18e23b, 0x3f1a9471, 0x3f1c4973, 0x3f1e0143, 0x3f1fbbe1,
  0x3f217950, 0x3f23398f, 0x3f24fca2, 0x3f26c288, 0x3f288b43, 0x3f2a56d5, 0x3f2c253f, 0x3f2df681,
  0x3f2fca9e, 0x3f31a197, 0x3f337b6c, 0x3f355820, 0x3f3737b5, 0x3f391a28, 0x3f3aff7e, 0x3f3ce7b7,
  0x3f3ed2d4, 0x3f40c0d6, 0x3f42b1c0, 0x3f44a592, 0x3f469c4d, 0x3f4895f3, 0x3f4a9284, 0x3f4c9203,
  0x3f4e9470, 0x3f5099cd, 0x3f52a21a, 0x3f54ad59, 0x3f56bb8c, 0x3f58ccb3, 0x3f5ae0cf, 0x3f5cf7e2,
  0x3f5f11ee, 0x3f612ef2, 0x3f634ef1, 0x3f6571ec, 0x3f6797e3, 0x3f69c0d8, 0x3f6beccd, 0x3f6e1bc2,
  0x3f704dba, 0x3f7282b4, 0x3f74bab1, 0x3f76f5b3, 0x3f7933bb, 0x3f7b74cb, 0x3f7db8e3, 0x3f800000
]

def decode (code : Nat) : Nat := warpRead[code]?.getD 0

theorem decode_endpoints : decode 0 = 0 ∧ decode 255 = 0x3f800000 := by decide +kernel

theorem decode_increases : ∀ c : Fin 255, decode c.val < decode (c.val + 1) := by decide +kernel

/-- Whether 255 times the sRGB curve at q/4095 reaches the midpoint below code k.
The nonlinear test isolates (q/4095)^(5/12) and raises both sides to power 12. -/
def reaches (q k : Nat) : Bool :=
  if k = 0 then true else
  if q ≤ 12 then
    decide ((2 * k - 1) * 100 * 4095 ≤ 2 * 255 * 1292 * q)
  else
    let boundary := 2000 * k - 1000 + 255 * 110
    decide (4095 ^ 5 * boundary ^ 12 ≤ q ^ 5 * (255 * 2110) ^ 12)

private def search (q : Nat) : Nat → Nat → Nat → Nat
  | 0, lo, _ => lo
  | fuel + 1, lo, hi =>
    if lo ≥ hi then lo else
      let mid := (lo + hi + 1) / 2
      if reaches q mid then search q fuel mid hi else search q fuel lo (mid - 1)

def curve (q : Nat) : Nat := search q 8 0 255

private theorem search_correct (q fuel lo hi : Nat) (ordered : lo ≤ hi)
    (width : hi - lo < 2 ^ fuel) (accepted : reaches q lo = true)
    (excluded : hi = 255 ∨ reaches q (hi + 1) = false) :
    let k := search q fuel lo hi
    lo ≤ k ∧ k ≤ hi ∧ reaches q k = true ∧ (k = 255 ∨ reaches q (k + 1) = false) := by
  induction fuel generalizing lo hi with
  | zero =>
    have same : lo = hi := by simp only [Nat.pow_zero] at width; omega
    subst hi
    exact ⟨Nat.le_refl _, Nat.le_refl _, accepted, excluded⟩
  | succ fuel ih =>
    simp only [search]
    split
    · rename_i same
      have eq : lo = hi := by omega
      subst hi
      exact ⟨Nat.le_refl _, Nat.le_refl _, accepted, excluded⟩
    · rw [Nat.pow_succ] at width
      let mid := (lo + hi + 1) / 2
      change let k := if reaches q mid then search q fuel mid hi else search q fuel lo (mid - 1)
        lo ≤ k ∧ k ≤ hi ∧ reaches q k = true ∧ (k = 255 ∨ reaches q (k + 1) = false)
      by_cases hit : reaches q mid = true
      · have result := ih mid hi (by dsimp [mid]; omega) (by dsimp [mid]; omega) hit excluded
        simp only [hit, ↓reduceIte]
        exact ⟨Nat.le_trans (by dsimp [mid]; omega) result.1, result.2⟩
      · have miss : reaches q mid = false := Bool.eq_false_iff.mpr hit
        have next : mid - 1 + 1 = mid := by dsimp [mid]; omega
        have result := ih lo (mid - 1) (by dsimp [mid]; omega) (by dsimp [mid]; omega)
          accepted (Or.inr (next ▸ miss))
        simp only [miss, Bool.false_eq_true, ↓reduceIte]
        exact ⟨result.1, Nat.le_trans result.2.1 (by dsimp [mid]; omega), result.2.2⟩

/-- The code is a byte whose lower midpoint has been reached and whose upper midpoint has not,
unless the output is saturated. Exact midpoint equality rounds upward. -/
theorem curve_correct (q : Nat) :
    curve q ≤ 255 ∧ reaches q (curve q) = true ∧
      (curve q = 255 ∨ reaches q (curve q + 1) = false) := by
  exact (search_correct q 8 0 255 (by decide) (by decide) (by simp [reaches]) (Or.inl rfl)).2

theorem curve_endpoints : curve 0 = 0 ∧ curve 4095 = 255 := by decide +kernel

def quantize (x : Nat) : Nat :=
  if F32.isNaN x then 0
  else
    let x := F32.mul (F32.clamp x 0 0x3f800000) (F32.ofNat 4095)
    let (_, n, d) := Float.toRat F32.fmt x
    let q := n / d
    let r := n % d
    min 4095 (if 2 * r > d ∨ (2 * r = d ∧ q % 2 = 1) then q + 1 else q)

def encode (x : Nat) : Nat := curve (quantize x)

theorem encode_nan (x : Nat) (nan : F32.isNaN x = true) : encode x = 0 := by
  simp [encode, quantize, nan, curve_endpoints.1]

def mirrors : List Mirror := [
  ⟨"dxgi.srgbDecode", fun
    | [x] => do return [(decode (← nat x) : Nat)]
    | _ => none⟩,
  ⟨"dxgi.srgbCurve", fun
    | [q] => do return [(curve (← nat q) : Nat)]
    | _ => none⟩,
  ⟨"dxgi.srgbEncode", fun
    | [x] => do return [(encode (← nat x) : Nat)]
    | _ => none⟩
]

end Kikizake.Dxgi.Srgb
