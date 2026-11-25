import Kikizake.F32

/-! The three small floating-point formats used by Direct3D resources. Encoding drops significand
bits toward zero and saturates finite overflow. Decoding uses exact binary32 arithmetic and keeps
the NaN payload that the shader reads. -/
namespace Kikizake.Dxgi.SmallFloat

open Kikizake

def encode (f : Float.Fmt) (signed : Bool) (x : Nat) : Nat :=
  let negative := Float.sign F32.fmt x
  let sb := if signed && negative then Float.sbit f else 0
  let inf := Float.inf f
  let e := Float.expField F32.fmt x
  let dropped := 23 - f.m
  let mant := Float.frac F32.fmt x >>> dropped
  let biasDelta := Float.bias F32.fmt - Float.bias f
  if F32.isNaN x then
    if signed then sb + inf + max 1 mant else 2 ^ (f.w + f.m) - 1
  else if !signed && negative then 0
  else if Float.isInf F32.fmt x then sb + inf
  else if e ≥ biasDelta + 2 ^ f.w - 1 then sb + inf - 1
  else if e ≤ biasDelta then sb + (Float.sig F32.fmt x >>> (biasDelta + 1 - max 1 e + dropped))
  else sb + (e - biasDelta) * 2 ^ f.m + mant

def decode (f : Float.Fmt) (signed : Bool) (bits : Nat) : Nat :=
  let negative := signed && Float.sign f bits
  if Float.expField f bits = 2 ^ f.w - 1 then
    (if negative then Float.sbit F32.fmt else 0) + Float.inf F32.fmt +
      (Float.frac f bits <<< (23 - f.m))
  else
    let (_, n, d) := Float.toRat f bits
    Float.round F32.fmt negative n d

/-- An unsigned destination flushes every negative binary32 other than NaNs to zero. -/
theorem encode_negative_unsigned (f : Float.Fmt) (x : Nat)
    (neg : Float.sign F32.fmt x = true) (num : F32.isNaN x = false) :
    encode f false x = 0 := by
  simp [encode, neg, num]

/-- Signed NaNs keep the upper payload bits, including when a payload of one must be substituted. -/
theorem encode_signed_nan (f : Float.Fmt) (x : Nat) (nan : F32.isNaN x = true) :
    encode f true x = (if Float.sign F32.fmt x then Float.sbit f else 0) +
      Float.inf f + max 1 (Float.frac F32.fmt x >>> (23 - f.m)) := by
  simp [encode, nan]

/-- At a representable normal exponent, narrowing is exactly division by the dropped-bit unit. -/
theorem encode_normal (f : Float.Fmt) (signed : Bool) (x : Nat)
    (num : F32.isNaN x = false) (finite : Float.isInf F32.fmt x = false)
    (keep : (!signed && Float.sign F32.fmt x) = false)
    (lo : Float.bias F32.fmt - Float.bias f < Float.expField F32.fmt x)
    (hi : Float.expField F32.fmt x < Float.bias F32.fmt - Float.bias f + 2 ^ f.w - 1) :
    encode f signed x = (if signed && Float.sign F32.fmt x then Float.sbit f else 0) +
      (Float.expField F32.fmt x - (Float.bias F32.fmt - Float.bias f)) * 2 ^ f.m +
      Float.frac F32.fmt x / 2 ^ (23 - f.m) := by
  simp [encode, num, finite, keep, Nat.not_le.mpr lo, Nat.not_le.mpr hi, Nat.shiftRight_eq_div_pow]

/-- Finite overflow uses the code immediately below infinity. -/
theorem encode_overflow (f : Float.Fmt) (signed : Bool) (x : Nat)
    (num : F32.isNaN x = false) (finite : Float.isInf F32.fmt x = false)
    (keep : (!signed && Float.sign F32.fmt x) = false)
    (hi : Float.bias F32.fmt - Float.bias f + 2 ^ f.w - 1 ≤ Float.expField F32.fmt x) :
    encode f signed x =
      (if signed && Float.sign F32.fmt x then Float.sbit f else 0) + Float.inf f - 1 := by
  simp [encode, num, finite, keep, hi]

/-- Reading a small float with an all-ones exponent preserves the sign and aligns its payload. -/
theorem decode_special (f : Float.Fmt) (signed : Bool) (bits : Nat)
    (special : Float.expField f bits = 2 ^ f.w - 1) :
    decode f signed bits = (if signed && Float.sign f bits then Float.sbit F32.fmt else 0) +
      Float.inf F32.fmt + Float.frac f bits * 2 ^ (23 - f.m) := by
  simp [decode, special, Nat.shiftLeft_eq]

/-- Unsigned NaNs use all bits of the destination, independently of sign and payload. -/
theorem encode_unsigned_nan (f : Float.Fmt) (x : Nat) (nan : F32.isNaN x = true) :
    encode f false x = 2 ^ (f.w + f.m) - 1 := by
  simp [encode, nan]

/-- Below the normal range, the encoded magnitude counts whole subnormal units. -/
theorem encode_subnormal (f : Float.Fmt) (signed : Bool) (x : Nat)
    (num : F32.isNaN x = false) (finite : Float.isInf F32.fmt x = false)
    (keep : (!signed && Float.sign F32.fmt x) = false)
    (low : Float.expField F32.fmt x ≤ Float.bias F32.fmt - Float.bias f)
    (hi : Float.expField F32.fmt x < Float.bias F32.fmt - Float.bias f + 2 ^ f.w - 1) :
    encode f signed x = (if signed && Float.sign F32.fmt x then Float.sbit f else 0) +
      Float.sig F32.fmt x / 2 ^
        (Float.bias F32.fmt - Float.bias f + 1 - max 1 (Float.expField F32.fmt x) + (23 - f.m)) := by
  simp [encode, num, finite, keep, low, Nat.not_le.mpr hi, Nat.shiftRight_eq_div_pow]

private def format : Nat → Option (Float.Fmt × Bool)
  | 0 => some (⟨10, 5⟩, true)
  | 1 => some (⟨6, 5⟩, false)
  | 2 => some (⟨5, 5⟩, false)
  | _ => none

def mirrors : List Mirror := [
  ⟨"dxgi.smallEncode", fun
    | [kind, bits] => do
      let (f, s) ← (nat kind >>= format)
      return [(encode f s (← nat bits) : Nat)]
    | _ => none⟩,
  ⟨"dxgi.smallDecode", fun
    | [kind, bits] => do
      let (f, s) ← (nat kind >>= format)
      return [(decode f s (← nat bits) : Nat)]
    | _ => none⟩
]

end Kikizake.Dxgi.SmallFloat
