import Kikizake.Mirror
import Kikizake.D3d11.Desc
/-! The view descriptions Direct3D 10 and 11 accept (spec/d3dres.txt D3DRES.VIEW.*), as
src/model/d3dres_check.rs computes them: `checkView` is `check_view_with` (CreateShaderResourceView,
CreateRenderTargetView, CreateDepthStencilView and CreateUnorderedAccessView of a resource
described by `d`, the view's and the resource's formats having the facts given: the format
tables' part is data from Windows' answers; mirror `d3d11.view`). A view kind is 0 a shader
resource, 1 a render target, 2 a depth stencil, 3 an unordered access view. The view's words
are its description as the executor prints it: the format, the dimension, (11's depth stencil
flags), then the union. -/
namespace Kikizake.D3d11.View
open Kikizake.D3d11.Desc

/-- the view's format and the resource's: the view's known or not, its bits an element,
typeless or not, the resource's typeless or not, the two of one typeless group or not, the
resource's a 32-bit typeless format of one-texel elements or not, the view format's support
bits -/
structure Facts where
  known : Bool
  bits : Nat
  typeless : Bool
  resTypeless : Bool
  sameGroup : Bool
  resTl32 : Bool
  s : Nat

def U32 : Nat := 0xffffffff
def OUTOFMEMORY : Nat := 0x8007000e

/-- the bind flag a kind needs -/
def bindOf : Nat → Nat
  | 0 => 0x8 | 1 => 0x20 | 2 => 0x40 | 3 => 0x80 | _ => 0

/-- the support bit of a kind (D3D11_FORMAT_SUPPORT_SHADER_LOAD, RENDER_TARGET, DEPTH_STENCIL,
TYPED_UNORDERED_ACCESS_VIEW) -/
def supOf : Nat → Nat
  | 0 => 0x100 | 1 => 0x4000 | 2 => 0x10000 | 3 => 0x2000000 | _ => 0

/-- the view dimensions a resource takes, by kind (the multisampled dimensions also view a
single-sampled 2D texture; Direct3D 10 no multisampled shader resource view of a cube, 10.0
no array of cubes) -/
def viewDims (dev : Dev) (kind : Nat) (d : Desc) : List Nat :=
  let ms := 1 < d.count
  let cube := d.misc &&& 4 ≠ 0
  if d.dim = 4 then (if kind = 0 then [1, 11] else if kind = 1 ∨ kind = 3 then [1] else [])
  else if d.dim = 1 then (if kind = 2 then [1, 2] else if kind ≤ 3 then [2, 3] else [])
  else if d.dim = 2 then
    if kind = 0 ∨ kind = 1 then
      if ms then [6, 7]
      else if kind = 0 ∧ cube then
        (if dev = .d10 then [4, 5, 9] else if dev.d3d11 then [4, 5, 6, 7, 9, 10] else [4, 5, 9, 10])
      else [4, 5, 6, 7]
    else if kind = 2 then (if ms then [5, 6] else [3, 4, 5, 6])
    else if kind = 3 then (if ms then [] else [4, 5])
    else []
  else if d.dim = 3 then (if kind = 2 ∨ 3 < kind then [] else [8])
  else []

/-- the head of a view's words: the format, the dimension, and 11's depth stencil flags -/
def heads (dev : Dev) (kind : Nat) : Nat := if kind = 2 ∧ dev.d3d11 then 3 else 2

/-- the description a view without one takes: the whole resource in its own format, an array
dimension for an array (a 1D one too, even of one slice), a cube for a shader resource of a
cube (an array of cubes from 12 slices); none for a buffer -/
def defaultView (dev : Dev) (kind : Nat) (d : Desc) : Option (List Nat) :=
  let arr := d.dim ≠ 3 ∧ 1 < d.da
  let du : Option (Nat × List Nat) :=
    if d.dim = 1 then
      if kind = 2 then some (if arr then 2 else 1, [0, 0, d.da])
      else if kind = 0 then some (if arr then 3 else 2, [0, d.mips, 0, d.da])
      else some (if arr then 3 else 2, [0, 0, d.da])
    else if d.dim = 2 then
      if kind = 0 ∧ d.misc &&& 4 ≠ 0 then some (if 6 < d.da then 10 else 9, [0, d.mips, 0, d.da / 6])
      else if kind = 2 then
        (if 1 < d.count then some (if arr then 6 else 5, [0, d.da]) else some (if arr then 4 else 3, [0, 0, d.da]))
      else if 1 < d.count then some (if arr then 7 else 6, [0, d.da])
      else if kind = 0 then some (if arr then 5 else 4, [0, d.mips, 0, d.da])
      else some (if arr then 5 else 4, [0, 0, d.da])
    else if d.dim = 3 then
      (if kind = 0 then some (8, [0, d.mips]) else some (8, [0, 0, d.da]))
    else none
  du.map fun (dim, u) => [d.fmt, dim] ++ (if kind = 2 ∧ dev.d3d11 then [0] else []) ++ u

/-- the union words a view dimension uses -/
def unionWords (kind dim : Nat) : Nat :=
  (match kind with
    | 0 => [0, 2, 2, 4, 2, 4, 0, 2, 2, 2, 4, 3]
    | 1 => [0, 2, 1, 3, 1, 3, 0, 2, 3]
    | 2 => [0, 1, 3, 1, 3, 0, 2]
    | _ => [0, 3, 1, 3, 1, 3, 0, 0, 3]).getD dim 0

/-- a range of `total`: the first and a count (-1 for the rest); the count it comes to -/
def range (first n total : Nat) : Option Nat :=
  let m := if n = U32 ∧ first < total then total - first else n
  if first < total ∧ m ≠ 0 ∧ m + first ≤ total then some m else none

/-- the union's four words checked against the resource: the words GetDesc gives back (a
count of -1 made the rest), or none for a range outside it -/
def ranges (kind dim : Nat) (d : Desc) (raw : Bool) (bits u0 u1 u2 u3 : Nat) : Option (List Nat) :=
  let mips := d.mips
  let arr := if d.dim = 3 then 1 else d.da
  if d.dim = 4 ∧ ((kind = 0 ∨ kind = 1 ∨ kind = 3) ∧ dim = 1 ∨ kind = 0 ∧ dim = 11) then
    -- (buffers: elements of the view's format, a raw view's 32-bit words; -1 is no rest)
    let el := if raw then 4 else bits / 8
    if el ≠ 0 ∧ u1 ≠ U32 then (range u0 u1 (d.w / el)).map fun b => [u0, b, u2, u3] else none
  else match kind, dim with
  | 0, 2 | 0, 4 | 0, 8 | 0, 9 => (range u0 u1 mips).map fun b => [u0, b, u2, u3]
  | 0, 3 | 0, 5 =>
    match range u0 u1 mips, range u2 u3 arr with
    | some b, some e => some [u0, b, u2, e]
    | _, _ => none
  | 0, 10 =>
    let e := if u3 = U32 ∧ u2 < arr then (arr - u2) / 6 else u3
    match range u0 u1 mips with
    | some b => if e ≠ 0 ∧ e * 6 + u2 ≤ arr then some [u0, b, u2, e] else none
    | none => none
  | 0, 6 | 1, 6 | 2, 5 => some [u0, u1, u2, u3]
  | 0, 7 | 1, 7 | 2, 6 => (range u0 u1 arr).map fun b => [u0, b, u2, u3]
  | 1, 2 | 1, 4 | 3, 2 | 3, 4 | 2, 1 | 2, 3 => if u0 < mips then some [u0, u1, u2, u3] else none
  | 1, 3 | 1, 5 | 3, 3 | 3, 5 | 2, 2 | 2, 4 =>
    (if u0 < mips then range u1 u2 arr else none).map fun c => [u0, u1, c, u3]
  | 1, 8 | 3, 8 => (if u0 < mips then range u1 u2 (max 1 (d.da >>> min u0 31)) else none).map fun c => [u0, u1, c, u3]
  | _, _ => none

/-- the description a view is checked by: its own, or the default one -/
def chosen (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) : Option (List Nat) :=
  match words with
  | some w => some w
  | none => defaultView dev kind d

/-- the view's format: its first word, the resource's for 0 -/
def fmtOf (d : Desc) (w : List Nat) : Nat := if w.getD 0 0 = 0 then d.fmt else w.getD 0 0

/-- a raw view: a BUFFEREX shader resource view or a buffer UAV with the RAW flag -/
def rawView (kind dim flag : Nat) : Bool := (kind = 0 ∧ dim = 11 ∨ kind = 3 ∧ dim = 1) ∧ flag % 2 = 1

/-- the checks before the ranges that refuse the view (E_INVALIDARG) -/
def refused (dev : Dev) (kind : Nat) (d : Desc) (w : List Nat) (fx : Facts) : Prop :=
  let f := fmtOf d w
  let head := heads dev kind
  let dim := w.getD 1 0
  let flag := w.getD (head + 2) 0
  let raw := rawView kind dim flag
  3 < kind ∨ d.bind &&& bindOf kind = 0 ∨ fx.known = false
    -- (a packed YUV texture's view in its own format)
    ∨ (107 ≤ f ∧ f ≤ 109 ∧ d.dim ≠ 4)
    -- (a format of the resource's typeless group, or an R32 UAV of a 32-bit typeless one)
    ∨ (d.dim ≠ 4 ∧ f ≠ d.fmt ∧ ¬(fx.resTypeless = true ∧ fx.sameGroup = true) ∧
      ¬(kind = 3 ∧ 41 ≤ f ∧ f ≤ 43 ∧ fx.resTl32 = true))
    -- (11's depth stencil flags: READ_ONLY_DEPTH, READ_ONLY_STENCIL)
    ∨ (head = 3 ∧ 3 < w.getD 2 0)
    ∨ dim ∉ viewDims dev kind d
    -- (typeless: only R32_TYPELESS raw views of a raw buffer)
    ∨ (fx.typeless = true ∧ ¬(raw = true ∧ f = 39))
    ∨ (raw = true ∧ ¬(d.dim = 4 ∧ d.misc &&& 0x20 ≠ 0 ∧ f = 39))
    -- (a UAV's APPEND and COUNTER flags: a structured buffer's)
    ∨ (kind = 3 ∧ dim = 1 ∧ (7 < flag ∨ flag &&& 6 ≠ 0 ∧ d.misc &&& 0x40 = 0))
    -- (the format's support for the kind, and a buffer's BUFFER bit)
    ∨ (raw = false ∧ (fx.s &&& supOf kind ≠ supOf kind ∨ d.dim = 4 ∧ fx.s % 2 = 0))

instance (dev : Dev) (kind : Nat) (d : Desc) (w : List Nat) (fx : Facts) : Decidable (refused dev kind d w fx) := by
  unfold refused; infer_instance

/-- Create*View's check of a view of `kind` of the resource `d` (`words` its description, none
for NULL; `check` a NULL out pointer): the words GetDesc gives or the HRESULT -/
def checkView (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts) :
    Except Nat (List Nat) :=
  match chosen dev kind d words with
  | none => .error INVALIDARG
  | some w =>
    let head := heads dev kind
    let dim := w.getD 1 0
    let flag := w.getD (head + 2) 0
    let u := ranges kind dim d (rawView kind dim flag) fx.bits
      (w.getD head 0) (w.getD (head + 1) 0) (w.getD (head + 2) 0) (w.getD (head + 3) 0)
    if refused dev kind d w fx then .error INVALIDARG
    else match u with
    | none => .error INVALIDARG
    | some u =>
      -- (a multisampled depth stencil view of a single-sampled array: out of memory when made)
      if kind = 2 ∧ dim = (if dev.d3d11 then 5 else 6) ∧ d.count = 1 ∧ 1 < d.da ∧ check = false then
        .error OUTOFMEMORY
      else .ok ((fmtOf d w :: (List.range (head - 1)).map (fun i => w.getD (i + 1) 0) ++ u).take
        (head + unionWords kind dim))

/-- a refused view is E_INVALIDARG -/
private theorem of_refused {dev : Dev} {kind : Nat} {d : Desc} {words : Option (List Nat)} {check : Bool} {fx : Facts}
    {w : List Nat} (hw : chosen dev kind d words = some w) (h : refused dev kind d w fx) :
    checkView dev kind d words check fx = .error INVALIDARG := by
  simp [checkView, hw, h]

/-- a view that is made passed the checks -/
private theorem of_ok {dev : Dev} {kind : Nat} {d : Desc} {words : Option (List Nat)} {check : Bool} {fx : Facts}
    {o : List Nat} (h : checkView dev kind d words check fx = .ok o) :
    ∃ w, chosen dev kind d words = some w ∧ ¬refused dev kind d w fx := by
  unfold checkView at h
  split at h
  · cases h
  · rename_i w hw
    refine ⟨w, hw, fun hr => ?_⟩
    simp [hr] at h

/-! ## D3DRES.VIEW -/

/-- a view is made only of a resource with the bind flag of its kind, a known format, the
resource's dimension, its format's support bit, a format that is not typeless (but a raw
view's R32_TYPELESS), of the resource's typeless group (or an R32 UAV of a 32-bit typeless
texture) -/
theorem view_needs (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts)
    (o : List Nat) (h : checkView dev kind d words check fx = .ok o) :
    ∃ w, chosen dev kind d words = some w ∧ kind ≤ 3 ∧ d.bind &&& bindOf kind ≠ 0 ∧ fx.known = true ∧
      w.getD 1 0 ∈ viewDims dev kind d ∧
      (fx.typeless = true → fmtOf d w = 39) ∧
      (rawView kind (w.getD 1 0) (w.getD (heads dev kind + 2) 0) = false → fx.s &&& supOf kind = supOf kind) ∧
      (d.dim ≠ 4 → fmtOf d w ≠ d.fmt →
        fx.resTypeless = true ∧ fx.sameGroup = true ∨ kind = 3 ∧ 41 ≤ fmtOf d w ∧ fmtOf d w ≤ 43 ∧ fx.resTl32 = true) := by
  obtain ⟨w, hw, hr⟩ := of_ok h
  simp only [refused, not_or] at hr
  obtain ⟨h1, h2, h3, -, h5, -, h7, h8, -, -, h11⟩ := hr
  refine ⟨w, hw, by omega, h2, by simpa using h3, Classical.byContradiction h7,
    fun ht => Classical.byContradiction fun hn => h8 ⟨ht, fun hf => hn hf.2⟩,
    fun hraw => Classical.byContradiction fun hn => h11 ⟨hraw, .inl hn⟩,
    fun hd hf => Classical.byContradiction fun hn => h5 ⟨hd, hf, fun g => hn (.inl g), fun g => hn (.inr g)⟩⟩

/-! ## D3DRES.VIEW.YUY2 -/

/-- a view of a YUY2, Y210 or Y216 texture in that format is refused -/
theorem yuy2 (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts)
    (w : List Nat) (hw : chosen dev kind d words = some w) (hf : 107 ≤ fmtOf d w ∧ fmtOf d w ≤ 109)
    (hd : d.dim ≠ 4) : checkView dev kind d words check fx = .error INVALIDARG :=
  of_refused hw (.inr (.inr (.inr (.inl ⟨hf.1, hf.2, hd⟩))))

/-! ## D3DRES.VIEW.DEFAULT -/

/-- a NULL description views the whole resource in its own format: the last word is the
resource's slices (a cube's faces over six; a 3D shader resource view's levels) -/
theorem default_whole (dev : Dev) (kind : Nat) (d : Desc) (w : List Nat) (h : defaultView dev kind d = some w) :
    w.head? = some d.fmt ∧ d.dim ≠ 4 ∧
      w.getLast? = some (if kind = 0 ∧ d.dim = 3 then d.mips
        else if kind = 0 ∧ d.dim = 2 ∧ d.misc &&& 4 ≠ 0 then d.da / 6 else d.da) := by
  unfold defaultView at h
  simp only [Option.map_eq_some_iff] at h
  obtain ⟨⟨dim, u⟩, hdu, rfl⟩ := h
  refine ⟨by simp, ?_, ?_⟩
  · intro h4; simp [h4] at hdu
  · repeat' split at hdu
    all_goals first
      | (simp only [Option.some.injEq, Prod.mk.injEq] at hdu; obtain ⟨-, rfl⟩ := hdu
         repeat' split
         all_goals simp_all <;> omega)
      | simp at hdu

/-- the dimension of a default view: no BUFFEREX, and a UAV's no buffer -/
theorem default_dim (dev : Dev) (kind : Nat) (d : Desc) (w : List Nat) (h : defaultView dev kind d = some w) :
    w.getD 1 0 ≠ 11 ∧ (kind = 3 → w.getD 1 0 ≠ 1) := by
  unfold defaultView at h
  simp only [Option.map_eq_some_iff] at h
  obtain ⟨⟨dim, u⟩, hdu, rfl⟩ := h
  repeat' split at hdu
  all_goals first
    | (simp only [Option.some.injEq, Prod.mk.injEq] at hdu; obtain ⟨rfl, -⟩ := hdu
       simp only [List.cons_append, List.getD_cons_succ, List.getD_cons_zero]
       repeat' split
       all_goals omega)
    | simp at hdu

/-- a NULL description of a typeless texture is refused (only a raw buffer view is of a typeless
format, and a buffer has no default view) -/
theorem default_typeless (dev : Dev) (kind : Nat) (d : Desc) (check : Bool) (fx : Facts)
    (ht : fx.typeless = true) : checkView dev kind d none check fx = .error INVALIDARG := by
  cases hw : chosen dev kind d none with
  | none => simp [checkView, hw]
  | some w =>
    have hd := default_dim dev kind d w (by simpa [chosen] using hw)
    have hraw : rawView kind (w.getD 1 0) (w.getD (heads dev kind + 2) 0) = false := by
      simp only [rawView, decide_eq_false_iff_not]
      omega
    exact of_refused hw (.inr (.inr (.inr (.inr (.inr (.inr (.inr (.inl ⟨ht, fun ⟨h, _⟩ => by rw [hraw] at h; cases h⟩))))))))

/-! ## D3DRES.VIEW.BUF -/

/-- a buffer view needs the format's BUFFER bit (but a raw view) -/
theorem buf_bit (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts)
    (w : List Nat) (hw : chosen dev kind d words = some w) (hd : d.dim = 4) (hs : fx.s % 2 = 0)
    (hr : rawView kind (w.getD 1 0) (w.getD (heads dev kind + 2) 0) = false) :
    checkView dev kind d words check fx = .error INVALIDARG :=
  of_refused hw (.inr (.inr (.inr (.inr (.inr (.inr (.inr (.inr (.inr (.inr ⟨hr, .inr ⟨hd, hs⟩⟩))))))))))

/-- a buffer view's count of -1 is refused, not the rest of the buffer -/
theorem buf_rest (dev : Dev) (kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts)
    (w : List Nat) (hw : chosen dev kind d words = some w) (hd : d.dim = 4)
    (hn : w.getD (heads dev kind + 1) 0 = U32) : checkView dev kind d words check fx = .error INVALIDARG := by
  by_cases hr : refused dev kind d w fx
  · exact of_refused hw hr
  · have hin : w.getD 1 0 ∈ viewDims dev kind d := by
      simp only [refused, not_or] at hr
      exact Classical.byContradiction hr.2.2.2.2.2.2.1
    have hk : (kind = 0 ∨ kind = 1 ∨ kind = 3) ∧ w.getD 1 0 = 1 ∨ kind = 0 ∧ w.getD 1 0 = 11 := by
      simp only [viewDims, hd, ite_true] at hin
      split at hin
      · simp only [List.mem_cons, List.not_mem_nil, or_false] at hin; omega
      · split at hin
        · simp only [List.mem_cons, List.not_mem_nil, or_false] at hin; omega
        · simp at hin
    have hu : ranges kind (w.getD 1 0) d (rawView kind (w.getD 1 0) (w.getD (heads dev kind + 2) 0)) fx.bits
        (w.getD (heads dev kind) 0) (w.getD (heads dev kind + 1) 0) (w.getD (heads dev kind + 2) 0)
        (w.getD (heads dev kind + 3) 0) = none := by
      unfold ranges
      simp only [List.getD_eq_getElem?_getD] at hk hn
      simp [hd, hk, hn]
    simp only [checkView, hw, hr, ite_false, hu]

/-- the range a view takes is inside the resource: from `first`, a count, not empty -/
theorem range_inside (first n total m : Nat) (h : range first n total = some m) :
    first < total ∧ m ≠ 0 ∧ first + m ≤ total := by
  unfold range at h
  split at h <;> (try split at h) <;> simp at h <;> omega

/-! ## D3DRES.VIEW.DSVMS -/

/-- a multisampled depth stencil view (2D array multisampled on 11, 2D multisampled on 10) of a
single-sampled array is out of memory when made, and checks as it would be made otherwise -/
theorem dsvms (dev : Dev) (d : Desc) (words : Option (List Nat)) (fx : Facts) (w : List Nat)
    (hw : chosen dev 2 d words = some w) (hdim : w.getD 1 0 = if dev.d3d11 then 5 else 6)
    (hc : d.count = 1) (ha : 1 < d.da) :
    checkView dev 2 d words false fx =
      (match checkView dev 2 d words true fx with | .ok _ => .error OUTOFMEMORY | e => e) := by
  simp only [checkView, hw]
  split
  · rfl
  · split
    · rfl
    · simp only [List.getD_eq_getElem?_getD] at hdim
      simp [hdim, hc, ha]

/-! ## D3DRES.VIEW.BUFEX10 -/

/-- Direct3D 10 (and 10.1) check buffer views as 11 does: BUFFEREX views of a plain buffer
and buffer render target views by the rules of 11 -/
theorem bufex10 (dev : Dev) (l kind : Nat) (d : Desc) (words : Option (List Nat)) (check : Bool) (fx : Facts)
    (hd : d.dim = 4) : checkView dev kind d words check fx = checkView (.d11 l) kind d words check fx := by
  have hdef : ∀ e : Dev, defaultView e kind d = none := by intro e; simp [defaultView, hd]
  have hc : ∀ e : Dev, chosen e kind d words = words := by
    intro e; cases words <;> simp [chosen, hdef]
  by_cases hk : kind = 2
  · -- (no depth stencil view of a buffer: refused on either)
    subst hk
    have hr : ∀ (e : Dev) w, refused e 2 d w fx := by
      intro e w; simp [refused, viewDims, hd]
    cases hwd : words with
    | none => simp [checkView, chosen, hdef]
    | some w =>
      rw [of_refused (by simp [chosen]) (hr dev w), of_refused (by simp [chosen]) (hr (.d11 l) w)]
  · have hh : ∀ e : Dev, heads e kind = 2 := by intro e; simp [heads, hk]
    have hv : ∀ e : Dev, viewDims e kind d = viewDims (.d11 l) kind d := by intro e; simp [viewDims, hd]
    have hr : ∀ (e : Dev) w, refused e kind d w fx ↔ refused (.d11 l) kind d w fx := by
      intro e w; simp only [refused, hh, hv]
    simp only [checkView, hc, hh, hr, hk, false_and, ite_false]

/-- the mirror's view check: [0, the HRESULT] or [1, GetDesc's words] -/
def mirrors : List Mirror := [
  ⟨"d3d11.view", fun
    | [dk, lv, kind, dim, w, h, da, mips, fmt, usage, bind, cpu, misc, count, q,
       has, len, x0, x1, x2, x3, x4, x5, x6, check,
       known, bits, tl, rtl, same, rtl32, s] => do
      let dev ← match dk with
        | 0 => some Dev.d10 | 1 => some (Dev.d101 (← nat lv)) | 2 => some (Dev.d11 (← nat lv)) | _ => none
      let d : Desc := ⟨← nat dim, ← nat w, ← nat h, ← nat da, ← nat mips, ← nat fmt, ← nat usage,
        ← nat bind, ← nat cpu, ← nat misc, ← nat count, ← nat q⟩
      let xs := [← nat x0, ← nat x1, ← nat x2, ← nat x3, ← nat x4, ← nat x5, ← nat x6]
      let n ← nat len
      let words := if has != 0 then some (xs.take n) else none
      let fx : Facts := ⟨known != 0, ← nat bits, tl != 0, rtl != 0, same != 0, rtl32 != 0, ← nat s⟩
      pure (match checkView dev (← nat kind) d words (check != 0) fx with
        | .error e => [0, (e : Int)]
        | .ok o => 1 :: o.map (fun x : Nat => (x : Int)))
    | _ => none⟩
]

end Kikizake.D3d11.View
