import Kikizake.D3d12.Footprint
/-! GetResourceAllocationInfo's sizes of textures (spec/d3d12.txt D3D12.ALLOC.*) and the 4KB
small-resource rule (D3D12.DESC.SMALL), as src/model/d3d12.rs computes them: `tight` is
`tight_bytes`, `swizzle` `swizzle_bytes`, `stdTile` `std_tile`, `size` `alloc_size`, `smallTiles`
`small_tiles` (mirrors `d3d12.tight`, `d3d12.swizzle`, `d3d12.alloc_size`, `d3d12.small_tiles`). -/
namespace Kikizake.D3d12.Alloc
open Kikizake.D3d12.Footprint (align align_bounds)

/-- a / b rounded up (Rust's div_ceil) -/
def cdiv (a b : Nat) : Nat := (a + b - 1) / b

private theorem cdiv_covers (a b : Nat) (hb : 0 < b) : a ≤ cdiv a b * b := by
  unfold cdiv
  have := Nat.mod_add_div (a + b - 1) b
  have := Nat.mod_lt (a + b - 1) hb
  rw [Nat.mul_comm]
  omega

/-- a texture as the allocation counts it (kind 2: 1D, 3: 2D, 4: 3D; mips resolved) -/
structure T where
  kind : Nat
  w : Nat
  h : Nat
  d : Nat
  mips : Nat
  bytes : Nat
  bw : Nat
  bh : Nat

/-- level m's elements across, down and deep -/
def level (t : T) (m : Nat) : Nat × Nat × Nat :=
  (cdiv (max (t.w / 2 ^ m) 1) t.bw, if t.kind = 2 then 1 else cdiv (max (t.h / 2 ^ m) 1) t.bh,
    if t.kind = 4 then max (t.d / 2 ^ m) 1 else 1)

/-- the array's slices (a 3D texture's depth is its levels') -/
def array (t : T) : Nat := if t.kind = 4 then 1 else t.d

/-- f 0 + ... + f (n - 1) -/
def levels (f : Nat → Nat) : Nat → Nat
  | 0 => 0
  | n + 1 => levels f n + f n

private theorem levels_le (f g : Nat → Nat) (h : ∀ m, f m ≤ g m) (n : Nat) : levels f n ≤ levels g n := by
  induction n with
  | zero => simp [levels]
  | succ n ih => simp only [levels]; exact Nat.add_le_add ih (h n)

private theorem levels_mul (f : Nat → Nat) (c n : Nat) : levels f n * c = levels (fun m => f m * c) n := by
  induction n with
  | zero => simp [levels]
  | succ n ih => simp only [levels]; rw [Nat.add_mul, ih]

/-- D3D12.ALLOC.TEXTURE: every subresource packed tightly, rows of whole elements -/
def tight (t : T) : Nat :=
  array t * levels (fun m => (level t m).1 * t.bytes * (level t m).2.1 * (level t m).2.2) t.mips

/-- D3D12.ALLOC.SWIZZLE: the standard 64KB tile shapes, in elements -/
def stdTile (tex2d : Bool) (bytes : Nat) : Nat × Nat × Nat :=
  match tex2d, bytes with
  | true, 1 => (256, 256, 1)
  | true, 2 => (256, 128, 1)
  | true, 4 => (128, 128, 1)
  | true, 8 => (128, 64, 1)
  | true, _ => (64, 64, 1)
  | false, 1 => (64, 32, 32)
  | false, 2 => (32, 32, 32)
  | false, 4 => (32, 32, 16)
  | false, 8 => (32, 16, 16)
  | false, _ => (16, 16, 16)

/-- D3D12.ALLOC.SWIZZLE: whole 64KB tiles, each mip level its own, times the array -/
def swizzle (t : T) : Nat :=
  let s := stdTile (t.kind = 3) t.bytes
  levels (fun m => cdiv (level t m).1 s.1 * cdiv (level t m).2.1 s.2.1 * cdiv (level t m).2.2 s.2.2) t.mips * array t * 65536

/-- D3D12.ALLOC.SWIZZLE: each standard shape is 64KB of its elements (1, 2, 4, 8, 16 bytes) -/
theorem std_tiles_64k : ∀ k : Bool, ∀ b ∈ [1, 2, 4, 8, 16],
    (stdTile k b).1 * (stdTile k b).2.1 * (stdTile k b).2.2 * b = 65536 := by decide

private theorem cover_levels (f g : Nat → Nat) (a n c : Nat) (h : ∀ m, f m ≤ g m * c) :
    a * levels f n ≤ levels g n * a * c := by
  have := levels_le f (fun m => g m * c) h n
  rw [← levels_mul] at this
  calc a * levels f n ≤ a * (levels g n * c) := Nat.mul_le_mul_left a this
    _ = levels g n * a * c := by rw [Nat.mul_comm a, Nat.mul_assoc, Nat.mul_comm c a, ← Nat.mul_assoc]

/-- D3D12.ALLOC.SWIZZLE: the tiles hold every element of every level: a swizzled texture takes at
least the bytes of the packed one -/
theorem swizzle_covers (t : T) (hb : t.bytes ∈ [1, 2, 4, 8, 16]) : tight t ≤ swizzle t := by
  have shape := std_tiles_64k (decide (t.kind = 3)) t.bytes hb
  unfold tight swizzle
  simp only
  generalize stdTile (decide (t.kind = 3)) t.bytes = s at shape
  obtain ⟨tw, th, td⟩ := s
  simp only at shape ⊢
  have p1 : 0 < tw := Nat.pos_of_ne_zero (by intro z; simp [z] at shape)
  have p2 : 0 < th := Nat.pos_of_ne_zero (by intro z; simp [z] at shape)
  have p3 : 0 < td := Nat.pos_of_ne_zero (by intro z; simp [z] at shape)
  apply cover_levels
  intro m
  generalize (level t m).1 = x
  generalize (level t m).2.1 = y
  generalize (level t m).2.2 = z
  have cx := cdiv_covers x tw p1
  have cy := cdiv_covers y th p2
  have cz := cdiv_covers z td p3
  calc x * t.bytes * y * z = x * y * z * t.bytes := by
        rw [Nat.mul_assoc x, Nat.mul_comm t.bytes y, ← Nat.mul_assoc, Nat.mul_assoc (x * y), Nat.mul_comm t.bytes z,
          ← Nat.mul_assoc]
    _ ≤ (cdiv x tw * tw) * (cdiv y th * th) * (cdiv z td * td) * t.bytes :=
        Nat.mul_le_mul_right _ (Nat.mul_le_mul (Nat.mul_le_mul cx cy) cz)
    _ = cdiv x tw * cdiv y th * cdiv z td * (tw * th * td * t.bytes) := by
        simp only [Nat.mul_assoc, Nat.mul_left_comm]
    _ = cdiv x tw * cdiv y th * cdiv z td * 65536 := by rw [shape]

/-- D3D12.ALLOC.MSAA, RT1D, TEXTURE: the allocation of `total` tight bytes and its alignment -/
def size (total samples asked : Nat) (rt1d : Bool) : Nat × Nat :=
  if samples > 1 then
    let al := if asked = 65536 then 65536 else 4194304
    (align (total * samples) al, al)
  else
    let t := if rt1d then total * 2 else total
    let al := if asked = 4096 then 4096 else 65536
    (align t al, al)

/-- D3D12.ALLOC.TEXTURE, MSAA, RT1D: the size is the bytes stored (every sample; a 1D render
target's twice) rounded up to the alignment: a multiple of it, less than one more; the
alignment 4KB only when asked and single-sampled, 4MB for samples unless 64KB is asked -/
theorem size_bounds (total samples asked : Nat) (rt1d : Bool) :
    let stored := if samples > 1 then total * samples else if rt1d then total * 2 else total
    let r := size total samples asked rt1d
    r.1 % r.2 = 0 ∧ stored ≤ r.1 ∧ r.1 < stored + r.2 ∧
    (r.2 = 4096 ↔ samples ≤ 1 ∧ asked = 4096) ∧ (r.2 = 4194304 ↔ samples > 1 ∧ asked ≠ 65536) := by
  simp only [size]
  by_cases hs : samples > 1
  · by_cases ha : asked = 65536
    · have := align_bounds (total * samples) 65536 (by decide)
      simp only [hs, ha, ↓reduceIte]
      exact ⟨this.2.1, this.1, this.2.2, by simp_all <;> omega, by simp_all <;> omega⟩
    · have := align_bounds (total * samples) 4194304 (by decide)
      simp only [hs, ha, ↓reduceIte]
      exact ⟨this.2.1, this.1, this.2.2, by simp_all <;> omega, by simp_all <;> omega⟩
  · by_cases ha : asked = 4096
    · have := align_bounds (if rt1d then total * 2 else total) 4096 (by decide)
      simp only [hs, ha, ↓reduceIte]
      exact ⟨this.2.1, this.1, this.2.2, by simp_all <;> omega, by simp_all <;> omega⟩
    · have := align_bounds (if rt1d then total * 2 else total) 65536 (by decide)
      simp only [hs, ha, ↓reduceIte]
      exact ⟨this.2.1, this.1, this.2.2, by simp_all <;> omega, by simp_all <;> omega⟩

/-- D3D12.DESC.SMALL: the standard 4KB tile shapes, for elements of 2^i bytes -/
def smallTile (tex2d : Bool) (i : Nat) : Option (Nat × Nat × Nat) :=
  match tex2d, i with
  | true, 0 => some (64, 64, 1)
  | true, 1 => some (64, 32, 1)
  | true, 2 => some (32, 32, 1)
  | true, 3 => some (32, 16, 1)
  | true, 4 => some (16, 16, 1)
  | false, 0 => some (16, 16, 16)
  | false, 1 => some (16, 8, 16)
  | false, 2 => some (16, 8, 8)
  | false, 3 => some (8, 8, 8)
  | false, 4 => some (8, 8, 4)
  | _, _ => none

/-- D3D12.DESC.SMALL: each 4KB shape is 4KB of its elements -/
theorem small_tiles_4k : ∀ k : Bool, ∀ i, i < 5 → ∃ s, smallTile k i = some s ∧ s.1 * s.2.1 * s.2.2 * 2 ^ i = 4096 := by
  decide

/-- D3D12.DESC.SMALL: the 4KB tiles the most detailed level takes (12-byte elements as 16) -/
def smallTiles (kind w h d bytes bw bh : Nat) : Option Nat := do
  let (ew, eh) := (cdiv w bw, cdiv h bh)
  let (tw, th, td) ← smallTile (kind = 3) (Nat.log2 bytes.nextPowerOfTwo)
  pure (if kind = 2 then cdiv ew (4096 / bytes) else if kind = 3 then cdiv ew tw * cdiv eh th
    else cdiv ew tw * cdiv eh th * cdiv d td)

def mirrors : List Mirror := [
  ⟨"d3d12.tight", fun
    | [kind, w, h, d, mips, bytes, bw, bh] => do
      let t : T := ⟨← nat kind, ← nat w, ← nat h, ← nat d, ← nat mips, ← nat bytes, ← nat bw, ← nat bh⟩
      if t.bw = 0 ∨ t.bh = 0 then none else pure [tight t, swizzle t]
    | _ => none⟩,
  ⟨"d3d12.alloc_size", fun
    | [total, samples, asked, rt1d] => do
      let r := size (← nat total) (← nat samples) (← nat asked) (rt1d != 0)
      pure [r.1, r.2]
    | _ => none⟩,
  ⟨"d3d12.small_tiles", fun
    | [kind, w, h, d, bytes, bw, bh] => do
      let bw ← nat bw
      let bh ← nat bh
      if bw = 0 ∨ bh = 0 then none
      else pure ((smallTiles (← nat kind) (← nat w) (← nat h) (← nat d) (← nat bytes) bw bh).toList.map Int.ofNat)
    | _ => none⟩
]

end Kikizake.D3d12.Alloc
