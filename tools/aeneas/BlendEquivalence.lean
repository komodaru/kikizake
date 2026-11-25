import GdiMix
import Kikizake.Gdi.Blend

open Aeneas Aeneas.Std

theorem generated_mix_matches_handwritten
    (s d a : Std.U32) (hs : s.val ≤ 255) (hd : d.val ≤ 255) (ha : a.val ≤ 255) :
    BlendTrial.mix s d a ⦃ out => out.val = Kikizake.Gdi.Blend.mix s.val d.val a.val ⦄ := by
  have hsa : s.val * a.val ≤ 65025 := Nat.mul_le_mul hs ha
  have hsub : 255 - a.val ≤ 255 := Nat.sub_le 255 a.val
  have hda : d.val * (255 - a.val) ≤ 65025 := Nat.mul_le_mul hd hsub
  unfold BlendTrial.mix Kikizake.Gdi.Blend.mix
  step*
