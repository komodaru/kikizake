import Kikizake.Mirror
/-! GetCopyableFootprints' layout of a texture's subresources (spec/d3d12.txt D3D12.FOOTPRINT.*) and
the mip chain (D3D12.DESC.MIPS), as src/model/d3d12.rs computes them: `layout` is `d3d12::layout`,
`chain` is `d3d12::chain`, `align` is `d3d12::align` (mirrors `d3d12.layout`, `d3d12.chain`,
`d3d12.alloc_buffer`). -/
namespace Kikizake.D3d12.Footprint

/-- v rounded up to a multiple of a -/
def align (v a : Nat) : Nat := (v + a - 1) / a * a

theorem align_bounds (v a : Nat) (ha : 0 < a) : v ≤ align v a ∧ align v a % a = 0 ∧ align v a < v + a := by
  unfold align
  have h := Nat.mod_add_div (v + a - 1) a
  have l := Nat.mod_lt (v + a - 1) ha
  rw [Nat.mul_comm ((v + a - 1) / a) a]
  refine ⟨by omega, Nat.mul_mod_right _ _, by omega⟩

/-- D3D12.ALLOC.BUFFER: a buffer's allocation, the smallest multiple of 64KB at least Width -/
theorem alloc_buffer (w : Nat) : w ≤ align w 65536 ∧ align w 65536 % 65536 = 0 ∧ align w 65536 < w + 65536 :=
  align_bounds w 65536 (by decide)

/-- D3D12.DESC.MIPS: the levels of a full chain of largest dimension m: its bit width -/
def chain (m : Nat) : Nat := Nat.log2 (max m 1) + 1

/-- D3D12.DESC.MIPS: the full chain halves the largest dimension down to 1 and stops there: its
last level is 1 wide, one more level would be 0 -/
theorem chain_ends (m : Nat) (h : 0 < m) : m / 2 ^ (chain m - 1) = 1 ∧ m / 2 ^ chain m = 0 := by
  unfold chain
  rw [Nat.max_eq_left h, Nat.add_sub_cancel]
  have lo := Nat.log2_self_le (Nat.pos_iff_ne_zero.mp h)
  have hi := Nat.lt_log2_self (n := m)
  have hi' : m < 2 * 2 ^ Nat.log2 m := by rw [Nat.pow_succ] at hi; omega
  have pos := Nat.two_pow_pos (Nat.log2 m)
  constructor
  · apply Nat.le_antisymm
    · exact Nat.le_of_lt_succ ((Nat.div_lt_iff_lt_mul pos).mpr hi')
    · exact Nat.div_pos lo pos
  · exact Nat.div_eq_of_lt hi

/-- a texture as the layout reads it (kind 2: 1D, 3: 2D, 4: 3D) -/
structure L where
  mips : Nat
  array : Nat
  kind : Nat
  w : Nat
  h : Nat
  d : Nat
  fmt : Nat
  bytes : Nat
  bw : Nat
  bh : Nat
  palign : Nat

/-- one subresource's footprint (its place apart) -/
structure Sub where
  fmt : Nat
  w : Nat
  h : Nat
  depth : Nat
  pitch : Nat
  rows : Nat
  row : Nat

/-- D3D12.FOOTPRINT.LAYOUT, BLOCK, STENCIL: subresource i is mip i mod MipLevels; its size halves
per level, 1 at least, rounded up to whole blocks; the stencil plane's R8_TYPELESS after the
depth plane's subresources; rows of whole blocks, RowPitch the row size rounded up -/
def sub (l : L) (i : Nat) : Sub :=
  let m := i % l.mips
  let w := max 1 (l.w / 2 ^ m)
  let h := if l.kind = 2 then 1 else max 1 (l.h / 2 ^ m)
  let depth := if l.kind = 4 then max 1 (l.d / 2 ^ m) else 1
  let fw := align w l.bw
  let fh := align h l.bh
  let st := decide (i ≥ l.mips * l.array)
  let bytes := if st then 1 else l.bytes
  let row := fw / l.bw * bytes
  ⟨if st then 60 else l.fmt, fw, fh, depth, align row l.palign, fh / l.bh, row⟩

/-- the bytes a subresource spans, and the bytes up to the end of its last row -/
def extent (s : Sub) : Nat := s.pitch * s.rows * s.depth
def last (s : Sub) : Nat := s.pitch * (s.rows * s.depth - 1) + s.row

/-- subresources i.. (c of them) from offset `off` past `base`: each at the next multiple of 512 -/
def subs (l : L) (base : Nat) : Nat → Nat → Nat → List (Nat × Sub)
  | _, 0, _ => []
  | i, c + 1, off => (base + off, sub l i) :: subs l base (i + 1) c (align (off + extent (sub l i)) 512)

/-- TotalBytes: up to the last subresource's last row -/
def total (l : L) : Nat → Nat → Nat → Nat → Nat
  | _, 0, _, t => t
  | i, c + 1, off, _ => total l (i + 1) c (align (off + extent (sub l i)) 512) (off + last (sub l i))

/-- D3D12.FOOTPRINT.LAYOUT: RowPitch holds the row and is a multiple of the alignment (256,
D3D12_TEXTURE_DATA_PITCH_ALIGNMENT; 512 for the depth plane), less than one alignment more -/
theorem pitch_holds_row (l : L) (i : Nat) (h : 0 < l.palign) :
    (sub l i).row ≤ (sub l i).pitch ∧ (sub l i).pitch % l.palign = 0 ∧ (sub l i).pitch < (sub l i).row + l.palign := by
  simp only [sub]; exact align_bounds _ _ h

/-- D3D12.FOOTPRINT.BLOCK: whole blocks, at least one row and one slice -/
theorem rows_whole (l : L) (i : Nat) (hw : 0 < l.bw) (hh : 0 < l.bh) :
    (sub l i).w % l.bw = 0 ∧ (sub l i).h % l.bh = 0 ∧ 1 ≤ (sub l i).rows ∧ 1 ≤ (sub l i).depth := by
  simp only [sub]
  refine ⟨(align_bounds _ _ hw).2.1, (align_bounds _ _ hh).2.1, ?_, ?_⟩
  · have b := align_bounds (if l.kind = 2 then 1 else max 1 (l.h / 2 ^ (i % l.mips))) l.bh hh
    have one : 1 ≤ (if l.kind = 2 then 1 else max 1 (l.h / 2 ^ (i % l.mips))) := by split <;> omega
    have : l.bh ≤ align (if l.kind = 2 then 1 else max 1 (l.h / 2 ^ (i % l.mips))) l.bh := by
      have := b.2.1; have := b.1
      rcases Nat.eq_zero_or_pos (align (if l.kind = 2 then 1 else max 1 (l.h / 2 ^ (i % l.mips))) l.bh) with z | p
      · omega
      · exact Nat.le_of_dvd p (Nat.dvd_of_mod_eq_zero b.2.1)
    exact (Nat.le_div_iff_mul_le hh).mpr (by omega)
  · split <;> omega

theorem last_le_extent (s : Sub) (hr : s.row ≤ s.pitch) (hn : 1 ≤ s.rows * s.depth) : last s ≤ extent s := by
  unfold last extent
  have : s.pitch * (s.rows * s.depth - 1) + s.pitch = s.pitch * (s.rows * s.depth) := by
    rw [← Nat.mul_succ]; congr 1; omega
  rw [Nat.mul_assoc]; omega

/-- consecutive subresources: each starts after the one before ends -/
def Apart : List (Nat × Sub) → Prop
  | x :: y :: r => x.1 + last x.2 ≤ y.1 ∧ Apart (y :: r)
  | _ => True

private theorem sub_fits (l : L) (i : Nat) (hp : 0 < l.palign) (hw : 0 < l.bw) (hh : 0 < l.bh) :
    last (sub l i) ≤ extent (sub l i) := by
  have p := pitch_holds_row l i hp
  have r := rows_whole l i hw hh
  exact last_le_extent _ p.1 (Nat.mul_le_mul r.2.2.1 r.2.2.2)

/-- D3D12.FOOTPRINT.LAYOUT: no two subresources overlap, and each starts at a multiple of 512
(D3D12_TEXTURE_DATA_PLACEMENT_ALIGNMENT) past the base -/
theorem layout_apart (l : L) (base i c off : Nat) (hp : 0 < l.palign) (hw : 0 < l.bw) (hh : 0 < l.bh) (ha : off % 512 = 0) :
    Apart (subs l base i c off) ∧ ∀ x ∈ subs l base i c off, base ≤ x.1 ∧ (x.1 - base) % 512 = 0 := by
  induction c generalizing i off with
  | zero => simp [subs, Apart]
  | succ c ih =>
    have next := align_bounds (off + extent (sub l i)) 512 (by decide)
    have fit := sub_fits l i hp hw hh
    have t := ih (i + 1) (align (off + extent (sub l i)) 512) next.2.1
    refine ⟨?_, ?_⟩
    · cases c with
      | zero => simp [subs, Apart]
      | succ c' =>
        simp only [subs, Apart] at t ⊢
        exact ⟨by omega, t.1⟩
    · intro x hx
      simp only [subs, List.mem_cons] at hx
      rcases hx with e | m
      · subst e; simp only; omega
      · exact t.2 x m

/-- the footprints as the mirror prints them: TotalBytes, then each one's offset, format, width,
height, depth, row pitch, rows, row size -/
def flat (l : L) (first count base : Nat) : List Nat :=
  total l first count 0 0 :: (subs l base first count 0).flatMap
    fun (o, s) => [o, s.fmt, s.w, s.h, s.depth, s.pitch, s.rows, s.row]

def mirrors : List Mirror := [
  ⟨"d3d12.layout", fun
    | [first, count, base, mips, array, kind, w, h, d, fmt, bytes, bw, bh, pa] => do
      let l : L := ⟨← nat mips, ← nat array, ← nat kind, ← nat w, ← nat h, ← nat d, ← nat fmt, ← nat bytes, ← nat bw, ← nat bh, ← nat pa⟩
      if l.mips = 0 ∨ l.bw = 0 ∨ l.bh = 0 ∨ l.palign = 0 then none
      else pure ((flat l (← nat first) (← nat count) (← nat base)).map Int.ofNat)
    | _ => none⟩,
  ⟨"d3d12.chain", fun | [m] => do pure [chain (← nat m)] | _ => none⟩,
  ⟨"d3d12.align", fun | [v, a] => do let a ← nat a; if a = 0 then none else pure [align (← nat v) a] | _ => none⟩
]

end Kikizake.D3d12.Footprint
