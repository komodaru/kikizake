import Kikizake.D3d11.Copy
import Kikizake.Dxgi.SmallFloat
import Kikizake.Dxgi.Srgb

/-! The pixel stored by ResolveSubresource when the sample planes differ. Each field is read
and averaged in memory order, with binary32 rounding in the paths that average decoded values.
Binary32 and binary16 NaN outputs are compared as one quiet NaN of their format. -/
namespace Kikizake.D3d11.ResolveMean

open Kikizake
open Kikizake.Dxgi

inductive Op
  | integer | word | norm (signed : Bool) | binary32 | small (signed : Bool)
  | srgb | alpha | zero

abbrev Lane := Nat × Op

private def lanes (count width : Nat) (op : Op) : List Lane :=
  List.replicate count (width, op)

/-- The render-target layouts supported by the nonuniform-sample resolve path. -/
def layout : Nat → Option (List Lane)
  | 2 => some (lanes 4 32 .binary32)
  | 6 => some (lanes 3 32 .binary32)
  | 10 => some (lanes 4 16 (.small true))
  | 11 => some (lanes 4 16 .word)
  | 13 => some (lanes 4 16 (.norm true))
  | 16 => some (lanes 2 32 .binary32)
  | 24 => some [(10, .integer), (10, .integer), (10, .integer), (2, .integer)]
  | 26 => some [(11, .small false), (11, .small false), (10, .small false)]
  | 28 | 87 | 88 => some (lanes 4 8 .integer)
  | 29 | 91 => some (lanes 3 8 .srgb ++ [(8, .alpha)])
  | 31 => some (lanes 4 8 (.norm true))
  | 34 => some (lanes 2 16 (.small true))
  | 35 => some (lanes 2 16 .word)
  | 37 => some (lanes 2 16 (.norm true))
  | 41 => some (lanes 1 32 .binary32)
  | 49 => some (lanes 2 8 .integer)
  | 51 => some (lanes 2 8 (.norm true))
  | 54 => some (lanes 1 16 (.small true))
  | 56 => some (lanes 1 16 .word)
  | 58 => some (lanes 1 16 (.norm true))
  | 61 | 65 => some (lanes 1 8 .integer)
  | 63 => some (lanes 1 8 (.norm true))
  | 85 => some [(5, .norm false), (6, .norm false), (5, .norm false)]
  | 86 => some (lanes 3 5 (.norm false) ++ [(1, .norm false)])
  | 93 => some (lanes 3 8 .srgb ++ [(8, .zero)])
  | 115 => some (lanes 4 4 (.norm false))
  | _ => none

def average (samples : List Nat) : Nat :=
  F32.div (samples.foldl F32.add 0) (F32.ofNat samples.length)

def normValue (signed : Bool) (bits code : Nat) : Int :=
  let top := (2 ^ (bits - 1) : Nat)
  if signed && decide (code ≥ top) then max (-(top : Int) + 1) ((code : Int) - 2 * (top : Int))
  else code

/-- Both the minimum SNORM encoding and the next encoding denote −1 before averaging. -/
theorem snorm_minimum (bits : Nat) :
    normValue true bits (2 ^ (bits - 1)) = -(2 ^ (bits - 1) : Int) + 1 := by
  simp [normValue]; omega

private def cast (v : Int) : Nat := Float.round F32.fmt (decide (v < 0)) v.natAbs 1

private def even (x : Nat) : Int :=
  let (sign, n, d) := Float.toRat F32.fmt x
  let q := Copy.nearestEven n d
  if sign then -(q : Int) else q

def normMean (signed : Bool) (bits : Nat) (samples : List Nat) : Nat :=
  let top := F32.ofNat (2 ^ (bits - (if signed then 1 else 0)) - 1)
  let factor := F32.div 0x3f800000 top
  let scaled := samples.map (fun c => F32.mul (cast (normValue signed bits c)) factor)
  let x := even (F32.mul (average scaled) top)
  (x % (2 ^ bits : Nat) : Int).toNat

theorem norm_mean_fits (signed : Bool) (bits : Nat) (samples : List Nat) :
    normMean signed bits samples < 2 ^ bits := by
  unfold normMean
  dsimp only
  have hp : 0 < (2 ^ bits : Int) := Int.pow_pos (by decide : (0 : Int) < 2)
  have fit (v : Int) : (v % (2 ^ bits : Int)).toNat < 2 ^ bits := by
    rw [← Int.ofNat_lt, Int.toNat_of_nonneg (Int.emod_nonneg _ (Int.ne_of_gt hp))]
    exact Int.emod_lt_of_pos _ hp
  exact fit _

def laneMean (lane : Lane) (samples : List Nat) : Nat :=
  let (bits, op) := lane
  match op with
  | .integer => Copy.integerMean samples.sum samples.length
  | .word => Copy.wordMean samples
  | .norm s => normMean s bits samples
  | .binary32 => average samples
  | .small s =>
    let f : Float.Fmt := ⟨bits - 5 - (if s then 1 else 0), 5⟩
    SmallFloat.encode f s (average (samples.map (SmallFloat.decode f s)))
  | .srgb => Srgb.encode (average (samples.map Srgb.decode))
  | .alpha =>
    let scale := F32.ofNat 255
    let a := average (samples.map (fun c => F32.div (F32.ofNat c) scale))
    let x := F32.clamp a 0 0x3f800000
    min 255 (even (F32.mul x scale)).toNat
  | .zero => 0

private def readPixel (bytes : List Nat) : Nat :=
  bytes.foldr (fun b acc => b + acc * 256) 0

private def pack (format : List Lane) (pixels : List Nat) : Nat :=
  (format.foldl (fun (state : Nat × Nat) lane =>
    let radix := 2 ^ lane.1
    let values := pixels.map (fun p => (p >>> state.1) % radix)
    let out := laneMean lane values % radix
    (state.1 + lane.1, state.2 + (out <<< state.1))) (0, 0)).2

def byteCount (format : List Lane) : Nat := (format.map Prod.fst).sum / 8

def resolve (format : List Lane) (samples : List Nat) : List Nat :=
  let width := byteCount format
  let pixels := (List.range (samples.length / width)).map (fun i =>
    readPixel ((samples.drop (i * width)).take width))
  let pixel := pack format pixels
  (List.range width).map fun i => (pixel >>> (8 * i)) % 256

/-- The output always writes one pixel and every output element is a byte. -/
theorem resolve_pixel (format : List Lane) (samples : List Nat) :
    (resolve format samples).length = byteCount format ∧
      ∀ b ∈ resolve format samples, b < 256 := by
  constructor
  · simp [resolve]
  · intro b h
    simp only [resolve, List.mem_map] at h
    obtain ⟨i, _, rfl⟩ := h
    exact Nat.mod_lt _ (by decide)

/-- The same binary32 samples can resolve differently when their order changes. -/
theorem resolve_binary32_order :
    resolve ((layout 41).getD []) [0xec, 0x78, 0xad, 0x60, 0xec, 0x78, 0xad, 0xe0, 0, 0, 0x80, 0x3f] =
      [0xab, 0xaa, 0xaa, 0x3e] ∧
    resolve ((layout 41).getD []) [0xec, 0x78, 0xad, 0x60, 0, 0, 0x80, 0x3f, 0xec, 0x78, 0xad, 0xe0] =
      [0, 0, 0, 0] := by decide +kernel

/-- Packed 10:10:10:2 ties round up separately and do not carry between fields. -/
theorem resolve_packed_midpoint :
    resolve ((layout 24).getD []) [0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff] =
      [0, 2, 8, 0xa0] := by decide +kernel

def mirrors : List Mirror := [
  ⟨"d3d11.resolveMean", fun a =>
    let answer := do
      let fmt ← a.head? >>= nat >>= layout
      let width := byteCount fmt
      let bytes ← a.drop 1 |>.mapM nat
      if bytes.length % width != 0 || bytes.isEmpty || bytes.length > 32 * width ||
          bytes.any (· ≥ 256) then none
      else some ((resolve fmt bytes).map Int.ofNat)
    some (answer.getD [-1])⟩
]

end Kikizake.D3d11.ResolveMean
