import Kikizake.D3d12.Footprint
import Kikizake.D3d12.Create
/-! GetCopyableFootprints of a description (spec/d3d12.txt D3D12.FOOTPRINT.*), as
src/model/d3d12.rs `footprints` computes it: the description's checks (`Create.validate`), a
buffer's one footprint, the formats' dimensions (`dims`, `dxgi_format::dims`), the two-texel and
one-level formats, the video formats' two planes, then the texture's layout
(`Footprint.flat`); `none` is the all-ones answer of an invalid description (mirror
`d3d12.footprints`, and `d3d12.dims`). -/
namespace Kikizake.D3d12.Copyable

open Create (Desc validate r texBit)
open Footprint (align chain L flat)

/-- D3D12.FORMAT.DIMS: the texture dimensions a format takes on WARP, bit 0 1D, 1 2D, 2 3D -/
def dims (f : Nat) : Nat :=
  if r f 19 22 || f == 40 || r f 44 47 || f == 55 then 3
  else if r f 1 65 || r f 67 69 || r f 85 88 || r f 90 93 || f == 115 || f == 191 then 7
  else if r f 70 84 || r f 94 99 then 6
  else if f == 89 || r f 100 105 || r f 107 109 || f == 113 || f == 114 then 2
  else 0

/-- D3D12.FORMAT.DIMS: the depth formats and their planar families 1D and 2D; the block
compressed ones 2D and 3D; R1_UNORM, 420_OPAQUE, NV11, AI44, IA44 none; the other video and the
palette formats and R10G10B10_XR_BIAS_A2_UNORM 2D; every other format of 1..115 all three, and
none past 115 but B4G4R4A4 (191) -/
theorem dims_table :
    (∀ f ∈ [19, 20, 21, 22, 40, 44, 45, 46, 47, 55], dims f = 3) ∧
    (∀ f, (70 ≤ f ∧ f ≤ 84 ∨ 94 ≤ f ∧ f ≤ 99) → dims f = 6) ∧
    (∀ f ∈ [66, 106, 110, 111, 112], dims f = 0) ∧
    (∀ f ∈ [89, 100, 101, 102, 103, 104, 105, 107, 108, 109, 113, 114], dims f = 2) ∧
    (∀ f, 1 ≤ f → f ≤ 115 → f ∉ [19, 20, 21, 22, 40, 44, 45, 46, 47, 55, 66, 89, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114] →
      ¬(70 ≤ f ∧ f ≤ 84 ∨ 94 ≤ f ∧ f ≤ 99) → dims f = 7) ∧
    (∀ f, f < 1024 → 115 < f → dims f = if f = 191 then 7 else 0) ∧ dims 0 = 0 := by
  refine ⟨by decide, fun f h => ?_, by decide, by decide, ?_, ?_, by decide⟩
  · unfold dims r; rcases h with h | h <;> (repeat' split) <;> simp_all <;> omega
  · intro f a b c d; unfold dims r; (repeat' split) <;> simp_all <;> omega
  · intro f a b; unfold dims r; (repeat' split) <;> simp_all <;> omega

/-- bytes per block, its width and height (`dxgi_format::block`); none for the formats without one -/
def block (f : Nat) : Option (Nat × Nat × Nat) :=
  if r f 1 4 then some (16, 1, 1)
  else if r f 5 8 then some (12, 1, 1)
  else if r f 9 18 then some (8, 1, 1)
  else if r f 23 43 then some (4, 1, 1)
  else if r f 48 59 then some (2, 1, 1)
  else if r f 60 65 then some (1, 1, 1)
  else if f == 67 then some (4, 1, 1)
  else if f == 68 || f == 69 then some (4, 2, 1)
  else if r f 70 72 || r f 79 81 then some (8, 4, 4)
  else if r f 73 78 || r f 82 84 || r f 94 99 then some (16, 4, 4)
  else if f == 85 || f == 86 || f == 115 || f == 191 then some (2, 1, 1)
  else if r f 87 93 then some (4, 1, 1)
  else if f == 100 || f == 101 then some (4, 1, 1)
  else if f == 102 then some (8, 1, 1)
  else if f == 107 then some (4, 2, 1)
  else if f == 108 || f == 109 then some (8, 2, 1)
  else if r f 111 113 then some (1, 1, 1)
  else if f == 114 then some (2, 1, 1)
  else none

/-- the depth stencil formats kept as two planes, and their typeless family -/
def planarDepth (f : Nat) : Bool := r f 19 22 || r f 44 47

/-- the full chain's levels by the dimension's largest size -/
def fullChain (x : Desc) : Nat :=
  chain (if x.dim = 4 then max (max x.w x.h) x.d else if x.dim = 3 then max x.w x.h else x.w)

/-- a video format's plane: luma (R8 or R16_TYPELESS) at full size, chroma (R8G8 or
R16G16_TYPELESS) at half, rows the width times 1 or 2 bytes, aligned to 512 -/
def plane (x : Desc) (b i : Nat) : Nat × Nat × Nat × Nat :=
  if i < x.d then (if b = 1 then 60 else 53, x.w, x.h, x.w * b) else (if b = 1 then 48 else 33, x.w / 2, x.h / 2, x.w * b)

/-- the video subresources i.. (c of them) at `off`, each item's offset, format, size, depth,
pitch, rows, row; with the total so far (a plane's bytes and its last row's end to 32 bits,
D3D12.FOOTPRINT.WRAP) -/
def video (x : Desc) (b base : Nat) : Nat → Nat → Nat → Nat → List Nat × Nat
  | _, 0, _, t => ([], t)
  | i, c + 1, off, _ =>
    let (fmt, w, h, row) := plane x b i
    let pitch := align row 512
    let rest := video x b base (i + 1) c (align (off + pitch * h % 2 ^ 32) 512) (off + (pitch * (h - 1) + row) % 2 ^ 32)
    ([base + off, fmt, w, h, 1, pitch, h, row] ++ rest.1, rest.2)

/-- the footprints' element (bytes, block width and height), format and row alignment: the depth
plane's R32_TYPELESS rows 512 apart for the two-plane depth formats, else the format's own -/
def shape (f : Nat) : Option ((Nat × Nat × Nat) × Nat × Nat) :=
  if planarDepth f then some ((4, 1, 1), 39, 512) else (block f).map fun b => (b, f, 256)

/-- the levels laid out (MipLevels 0: the full chain), the array slices (a 3D texture one), the
planes (the two-plane depth formats two) -/
def levels (x : Desc) : Nat := if x.mips = 0 then fullChain x else x.mips
def slices (x : Desc) : Nat := if x.dim = 4 then 1 else x.d
def planes (f : Nat) : Nat := if planarDepth f then 2 else 1

/-- D3D12.FOOTPRINT.*: GetCopyableFootprints' answer for subresources first..first + count from
`base`: TotalBytes, then each footprint (offset, format, width, height, depth, row pitch, rows,
row size); none when every field is all ones -/
def footprints (first count base : Nat) (x : Desc) : Option (List Nat) :=
  if validate x ≠ 0 then none
  else if x.dim = 1 then
    if x.w = 0xffffffff ∨ first + count > 1 then none
    else some ((if count = 0 then 0 else x.w) :: (List.replicate count [base, 0, x.w, 1, 1, align x.w 256, 1, x.w]).flatten)
  else if dims x.fmt &&& texBit x.dim = 0 then none
  else if (x.fmt == 89 || x.fmt == 101 || r x.fmt 107 109 || x.fmt == 113 || x.fmt == 114) ∧ x.mips ≠ 1 then none
  else if (x.fmt == 68 || x.fmt == 69 || r x.fmt 107 109) ∧ x.w % 2 ≠ 0 then none
  else if r x.fmt 103 105 then
    if x.mips ≠ 1 ∨ x.w % 2 ≠ 0 ∨ x.h % 2 ≠ 0 ∨ first + count > 2 * x.d then none
    else let v := video x (if x.fmt = 103 then 1 else 2) base first count 0 0
      some (v.2 :: v.1)
  else match shape x.fmt with
    | none => none
    | some ((bytes, bw, bh), fmt, pa) =>
      if first + count > levels x * slices x * planes x.fmt then none
      else some (flat ⟨levels x, slices x, x.dim, x.w, x.h, x.d, fmt, bytes, bw, bh, pa⟩ first count base)

/-- D3D12.FOOTPRINT.INVALID: a description `validate` refuses gets all ones -/
theorem invalid_desc (first count base : Nat) (x : Desc) (h : validate x ≠ 0) : footprints first count base x = none := by
  simp [footprints, h]

/-- D3D12.FOOTPRINT.BUFFER, RANGE: a buffer is one subresource: format UNKNOWN, Width bytes, 1
high and deep, RowPitch Width rounded up to 256, one row; TotalBytes Width; Width 0xffffffff
and subresources past the first all ones -/
theorem buffer (base : Nat) (x : Desc) (hd : x.dim = 1) (hv : validate x = 0) :
    (x.w ≠ 0xffffffff → footprints 0 1 base x = some [x.w, base, 0, x.w, 1, 1, align x.w 256, 1, x.w] ∧
      footprints 0 0 base x = some [0]) ∧
    (x.w = 0xffffffff → ∀ f c, footprints f c base x = none) ∧
    (∀ f c, f + c > 1 → footprints f c base x = none) := by
  refine ⟨fun hw => ⟨by simp [footprints, hd, hv, hw], by simp [footprints, hd, hv, hw]⟩, fun hw f c => by simp [footprints, hd, hv, hw],
    fun f c hc => by simp [footprints, hd, hv, hc]⟩

/-- a buffer's description has format UNKNOWN -/
private theorem buffer_unknown (x : Desc) (hd : x.dim = 1) (hv : validate x = 0) : x.fmt = 0 := by
  by_cases hf : x.fmt = 0
  · exact hf
  · exfalso
    simp [validate, hd, hf, Create.BUFFER] at hv

/-- D3D12.FOOTPRINT.PAIRS: R8G8_B8G8, G8R8_G8B8, YUY2, Y210, Y216 want an even width; YUY2,
Y210, Y216, R10G10B10_XR_BIAS_A2_UNORM, Y410, P8 and A8P8 one mip level -/
theorem pairs (first count base : Nat) (x : Desc) :
    ((x.fmt = 68 ∨ x.fmt = 69 ∨ 107 ≤ x.fmt ∧ x.fmt ≤ 109) → x.w % 2 ≠ 0 → footprints first count base x = none) ∧
    ((x.fmt = 89 ∨ x.fmt = 101 ∨ 107 ≤ x.fmt ∧ x.fmt ≤ 109 ∨ x.fmt = 113 ∨ x.fmt = 114) → x.mips ≠ 1 → footprints first count base x = none) := by
  constructor <;> intro hf hw <;> by_cases hv : validate x = 0 <;> (try simp [footprints, hv]) <;> by_cases hd : x.dim = 1
  all_goals first
    | (have := buffer_unknown x hd hv; omega)
    | (simp [hd, r]; intros; omega)

/-- D3D12.FOOTPRINT.VIDEO: an NV12 2D texture of one level and an even size (up to 2^23) is two planes:
luma R8_TYPELESS at full size, then chroma R8G8_TYPELESS at half, rows of the width in bytes
aligned to 512, the chroma plane at the next multiple of 512 after the luma one -/
theorem nv12 (base w h : Nat) (hw : 2 ≤ w) (hh : 2 ≤ h) (hw' : w ≤ 2 ^ 23) (hh' : h ≤ 2 ^ 23) (ew : w % 2 = 0) (eh : h % 2 = 0) :
    footprints 0 2 base ⟨3, 0, w, h, 1, 1, 103, 1, 0, 0, 0⟩ = some
      [align (align w 512 * h % 2 ^ 32) 512 + (align w 512 * (h / 2 - 1) + w) % 2 ^ 32,
       base, 60, w, h, 1, align w 512, h, w,
       base + align (align w 512 * h % 2 ^ 32) 512, 48, w / 2, h / 2, 1, align w 512, h / 2, w] := by
  have a : w ≠ 0 := by omega
  have b : h ≠ 0 := by omega
  have c : ¬(w > 2 ^ 23 ∨ h > 2 ^ 23) := by omega
  simp [footprints, validate, Create.lim, video, plane, dims, texBit, r, a, b, c, ew, eh, Create.T2]

/-- D3D12.FOOTPRINT.VIDEO: an NV12, P010 or P016 texture of an odd size or more than one level is
all ones -/
theorem video_even (first count base : Nat) (x : Desc) (hf : 103 ≤ x.fmt ∧ x.fmt ≤ 105) (h : x.w % 2 ≠ 0 ∨ x.h % 2 ≠ 0 ∨ x.mips ≠ 1) :
    footprints first count base x = none := by
  by_cases hv : validate x = 0 <;> (try simp [footprints, hv]) <;> by_cases hd : x.dim = 1
  all_goals first
    | (have := buffer_unknown x hd hv; omega)
    | (simp [hd, r]; intros; rw [ite_eq_left hf, ite_eq_left (by omega)])

private theorem subs_flat (l : L) (base i c off : Nat) :
    ((Footprint.subs l base i c off).flatMap fun (o, s) => [o, s.fmt, s.w, s.h, s.depth, s.pitch, s.rows, s.row]).length = 8 * c := by
  induction c generalizing i off with
  | zero => simp [Footprint.subs]
  | succ c ih => simp [Footprint.subs, ih]; omega

private theorem video_length (x : Desc) (b base i c off t : Nat) : (video x b base i c off t).1.length = 8 * c := by
  induction c generalizing i off t with
  | zero => simp [video]
  | succ c ih => simp [video, ih]; omega

/-- D3D12.FOOTPRINT.LAYOUT: an answer is TotalBytes and one footprint of eight fields for each
subresource asked -/
theorem answer_length (first count base : Nat) (x : Desc) (l : List Nat) (h : footprints first count base x = some l) :
    l.length = 8 * count + 1 := by
  unfold footprints at h
  repeat' split at h
  all_goals first | (simp at h; done) | skip
  all_goals (try split at h)
  all_goals first | (simp at h; done) | skip
  all_goals (simp only [Option.some.injEq] at h; subst h)
  all_goals first | (simp only [flat, List.length_cons, subs_flat]; done) | (simp [video_length, List.length_flatten]; try omega)

/-- D3D12.FOOTPRINT.RANGE: subresources past the last are all ones: past the levels times the
slices times the planes of a texture, past both planes of each slice of a video one -/
theorem range (first count base : Nat) (x : Desc) (hd : x.dim ≠ 1) :
    (¬(103 ≤ x.fmt ∧ x.fmt ≤ 105) → first + count > levels x * slices x * planes x.fmt → footprints first count base x = none) ∧
    (103 ≤ x.fmt ∧ x.fmt ≤ 105 → first + count > 2 * x.d → footprints first count base x = none) := by
  constructor <;> intro hf hc <;> unfold footprints <;> repeat' split
  all_goals first | rfl | (simp_all [r]; done) | omega | (simp only [r, Bool.and_eq_true, decide_eq_true_eq] at *; omega)

def mirrors : List Mirror := [
  ⟨"d3d12.dims", fun | [f] => (nat f).map fun f => [dims f] | _ => none⟩,
  ⟨"d3d12.footprints", fun
    | [first, count, base, dim, align, w, h, d, mips, fmt, samples, quality, layout, flags] => do
      let x : Desc := ⟨← nat dim, ← nat align, ← nat w, ← nat h, ← nat d, ← nat mips, ← nat fmt, ← nat samples, ← nat quality, ← nat layout, ← nat flags⟩
      pure ((footprints (← nat first) (← nat count) (← nat base) x).elim [-1] (·.map Int.ofNat))
    | _ => none⟩
]

end Kikizake.D3d12.Copyable
