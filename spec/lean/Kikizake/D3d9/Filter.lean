import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
/-! Direct3D 9's point and linear filtering (D3D9.TEX.POINT, D3D9.TEX.LINEAR) as
src/model/d3d9draw.rs computes them: `pointAt` `point_at`, `bilinearAt` `bilinear_at`, `weights`
`bilinear_weights`, `bytesSum` `bilinear_bytes`, `unit` `bilinear_unit`, `floatSum`
`bilinear_float` (mirrors `d3d9.point`, `d3d9.bilinear`, `d3d9.bibytes`, `d3d9.bifloat`). -/
namespace Kikizake.D3d9.Filter

open Kikizake.D3d11.Filter (floorF roundF castInt)

/-- Rust `as i64` of a binary64 -/
def i64 (x : Nat) : Int := castInt F64.fmt (-(2 ^ 63)) (2 ^ 63 - 1) x

def d64 : Nat := 0x4050000000000000
def d32 : Nat := 0x4040000000000000
def dhalf : Nat := 0x3fe0000000000000

/-- point filtering's texel of the texel coordinate x: x rounded to the nearest 64th, taken down -/
def pointAt (x : Nat) : Int := i64 (floorF F64.fmt (F64.div (roundF F64.fmt (F64.mul x d64)) d64))

/-- linear filtering's first texel and the next one's weight in 32nds: x - 0.5 taken down, its
fraction times 32 taken down -/
def bilinearAt (x : Nat) : Int × Int :=
  let y := F64.sub x dhalf
  let y0 := floorF F64.fmt y
  (i64 y0, i64 (floorF F64.fmt (F64.mul (F64.sub y y0) d32)))

/-- the four texels' weights of the next ones' weights fx, fy -/
def weights (fx fy : Int) : List Int := [(32 - fx) * (32 - fy), fx * (32 - fy), (32 - fx) * fy, fx * fy]

/-- 8-bit texels' weighted sum -/
def bytesSum (t w : List Int) : Int := (List.zipWith (· * ·) t w).foldl (· + ·) 0

/-- a binary32 of a whole number (Rust `as f32` of an i64) -/
def ofInt (v : Int) : Nat := Float.round F32.fmt (v < 0) v.natAbs 1

/-- the sum truncated to a whole 255th, binary32 taken to binary64 -/
def unit (v : Int) : Nat := F64.ofF32 (F32.div (ofInt (v.tdiv 1024)) 0x437f0000)

/-- a binary64 of a whole number (Rust `as f64` of an i64) -/
def ofInt64 (v : Int) : Nat := Float.round F64.fmt (v < 0) v.natAbs 1

/-- other texels' weighted sum in binary64 (Rust's `sum` starts at -0), over 1024 -/
def floatSum (t : List Nat) (w : List Int) : Nat :=
  F64.div ((List.zipWith (fun a b => F64.mul a (ofInt64 b)) t w).foldl F64.add 0x8000000000000000) 0x4090000000000000

private theorem weights_sum (fx fy : Int) : (weights fx fy).foldl (· + ·) 0 = 1024 := by
  simp only [weights, List.foldl]
  have h1 : (32 - fx) * (32 - fy) + fx * (32 - fy) = 32 * (32 - fy) := by
    rw [← Int.add_mul]; simp
  have h2 : (32 - fx) * fy + fx * fy = 32 * fy := by
    rw [← Int.add_mul]; simp
  omega

/-- D3D9.TEX.LINEAR: the four weights make a whole texel (1024 32nds squared) whatever the
fractions, so a texture of one 8-bit value b filters to b / 255 at every point -/
theorem linear_whole (fx fy b : Int) :
    (weights fx fy).foldl (· + ·) 0 = 1024 ∧ bytesSum [b, b, b, b] (weights fx fy) = b * 1024 := by
  refine ⟨weights_sum fx fy, ?_⟩
  have h := weights_sum fx fy
  simp only [weights, List.foldl] at h
  simp only [bytesSum, weights, List.zipWith, List.foldl]
  rw [← h]
  simp only [Int.mul_add]
  omega

/-- D3D9.TEX.LINEAR: a sum truncated, not rounded: 1023 1024ths of a 255th are none, 1024 one -/
theorem linear_truncated : unit 1023 = 0 ∧ unit 1024 = F64.ofF32 (F32.div 0x3f800000 0x437f0000) := by
  decide +kernel

/-- D3D9.TEX.POINT: a texel center (2.5) is its texel, 1.99375 rounds to the 64th at 2 and takes
texel 2, 1.98 (127 64ths) texel 1; a NaN coordinate is texel 0 -/
theorem point_nearest64 :
    pointAt 0x4004000000000000 = 2 ∧ pointAt 0x3fffe66666666666 = 2 ∧ pointAt 0x3fffae147ae147ae = 1 ∧
    pointAt 0x7ff8000000000000 = 0 := by
  decide +kernel

/-- D3D9.TEX.LINEAR: texel centers at half-integer coordinates: 2.5 is texel 2 with no weight
of the next, 2.75 texel 2 with 8 32nds of texel 3, 0.25 texel -1 with 24 -/
theorem linear_centers :
    bilinearAt 0x4004000000000000 = (2, 0) ∧ bilinearAt 0x4006000000000000 = (2, 8) ∧
    bilinearAt 0x3fd0000000000000 = (-1, 24) := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"d3d9.point", fun | [h, l] => do pure [pointAt (F64.join (← nat h) (← nat l))] | _ => none⟩,
  ⟨"d3d9.bilinear", fun | [h, l] => do let (i, f) := bilinearAt (F64.join (← nat h) (← nat l)); pure [i, f] | _ => none⟩,
  ⟨"d3d9.bibytes", fun
    | [t0, t1, t2, t3, fx, fy] => do
      let v := bytesSum [t0, t1, t2, t3] (weights fx fy)
      pure ((weights fx fy) ++ [v] ++ F64.halves (unit v))
    | _ => none⟩,
  ⟨"d3d9.bifloat", fun l => do
    let t ← [0, 1, 2, 3].mapM fun k => do F64.join (← nat (← l[2 * k]?)) (← nat (← l[2 * k + 1]?))
    pure (F64.halves (F64.canon (floatSum t (weights (← l[8]?) (← l[9]?)))))⟩
]

end Kikizake.D3d9.Filter
