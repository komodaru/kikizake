import Kikizake.Mirror
/-! The argument checks of DXGI (spec/dxgi.txt), as src/model/d3d12.rs computes them: `assoc` is
`assoc_hr`, `feature` `feature_args_taken`, `hdr` `hdr_taken`, `spaces` `color_spaces`, `chain`
`chain_desc`, `refused` `present_refused`, `refused1` `present1_refused` (mirrors `dxgi.assoc`,
`dxgi.feature`, `dxgi.hdr`, `dxgi.spaces`, `dxgi.chain`, `dxgi.present`, `dxgi.present1`). -/
namespace Kikizake.Dxgi.Args

def INVALID_CALL : Nat := 0x887a0001
def INVALIDARG : Nat := 0x80070057
def OUTOFMEMORY : Nat := 0x8007000e

/-- a 32-bit value's bits outside `known` -/
def outside (f known : Nat) : Bool := f &&& (0xffffffff - known) != 0

/-- DXGI.ASSOC: MakeWindowAssociation takes NO_WINDOW_CHANGES, NO_ALT_ENTER, NO_PRINT_SCREEN -/
def assoc (f : Nat) : Nat := if outside f 7 then INVALID_CALL else 0

/-- DXGI.ASSOC: the three flags in any combination are taken -/
theorem assoc_taken : ∀ f, f < 8 → assoc f = 0 := by decide

/-- DXGI.ASSOC: any other bit is refused -/
theorem assoc_other_bits : ∀ b, b < 32 → 3 ≤ b → ∀ f, f < 8 → assoc (f ||| 2 ^ b) = INVALID_CALL := by decide

/-- DXGI.FEATURE.ARGS: CheckFeatureSupport takes DXGI_FEATURE_PRESENT_ALLOW_TEARING with 4 bytes -/
def feature (f size : Nat) : Bool := f == 0 && size == 4

theorem feature_only (f size : Nat) : feature f size = true ↔ f = 0 ∧ size = 4 := by
  simp [feature]

/-- DXGI.HDR: SetHDRMetaData's type, size, and whether the data is NULL -/
def hdr (kind size : Nat) (null : Bool) : Bool :=
  match kind with
  | 0 => size == 0 && null
  | 1 => size == 28 && !null
  | 2 => size == 72 && !null
  | _ => false

/-- DXGI.HDR: each type is taken with one size only, NONE alone with no data; no other type -/
theorem hdr_one_size (k s s' : Nat) (n n' : Bool) (h : hdr k s n = true) (h' : hdr k s' n' = true) :
    s = s' ∧ n = n' ∧ (n = true ↔ k = 0) ∧ k ≤ 2 := by
  unfold hdr at h h'
  match k with
  | 0 => simp at h h'; exact ⟨by omega, by simp [h.2, h'.2], by simp [h.2], by omega⟩
  | 1 => simp at h h'; exact ⟨by omega, by simp [h.2, h'.2], by simp [h.2], by omega⟩
  | 2 => simp at h h'; exact ⟨by omega, by simp [h.2, h'.2], by simp [h.2], by omega⟩
  | _ + 3 => simp at h

/-- DXGI.CHAIN.FORMAT: the formats a flip chain takes -/
def flip : List Nat := [10, 24, 28, 87]

/-- DXGI.COLORSPACE.FLIP: the color spaces a flip chain of the format presents in -/
def spaces (fmt : Nat) : List Nat :=
  if fmt = 10 then [1] else if fmt ∈ [24, 28, 87] then [0, 2, 3, 12, 14, 17, 20, 21] else []

/-- DXGI.COLORSPACE.FLIP: the float chain presents in RGB_FULL_G10_NONE_P709 alone, the 8 and
10-bit ones in the same eight, all of DXGI_COLOR_SPACE_TYPE (0..25); the full-range sRGB space
(0) for every one but the float chain -/
theorem spaces_by_format :
    spaces 10 = [1] ∧ spaces 24 = spaces 28 ∧ spaces 28 = spaces 87 ∧
    (∀ f ∈ flip, (spaces f).length > 0 ∧ ∀ c ∈ spaces f, c ≤ 25) ∧
    (∀ f ∈ flip, (0 ∈ spaces f ↔ f ≠ 10)) := by decide

/-- a swap chain's DXGI_SWAP_CHAIN_DESC1 words -/
structure Desc where
  w : Nat
  h : Nat
  fmt : Nat
  stereo : Nat
  count : Nat
  quality : Nat
  usage : Nat
  buffers : Nat
  scaling : Nat
  effect : Nat
  alpha : Nat
  flags : Nat
deriving DecidableEq

/-- DXGI.CHAIN.FLAGS: the flags a chain takes -/
def CHAIN_FLAGS : Nat := 0x2 ||| 0x20 ||| 0x40 ||| 0x800 ||| 0x1000 ||| 0x2000
/-- DXGI.CHAIN.MEMORY: x86 runs out of memory at this many bytes of buffers -/
def OOM : Nat := 16384 * 14250 * 8

/-- DXGI.CHAIN.*: what CreateSwapChainForHwnd refuses -/
def bad (d : Desc) : Bool :=
  !(d.effect == 3 || d.effect == 4) || d.buffers < 2 || d.buffers > 16 || d.count != 1 || d.quality != 0 ||
    !(flip.contains d.fmt) || d.scaling > 1 || !(d.alpha == 0 || d.alpha == 3) || outside d.usage 0x70 ||
    outside d.flags CHAIN_FLAGS

/-- DXGI.CHAIN.SIZE: a 0 width or height is the window's 64 by 64 -/
def sized (d : Desc) : Desc := if d.w = 0 ∨ d.h = 0 then { d with w := 64, h := 64 } else d

/-- CreateSwapChainForHwnd: the chain's description, or the error -/
def chain (d : Desc) (arch : Nat) : Except Nat Desc :=
  if bad d then .error INVALID_CALL
  else if arch = 32 ∧ (sized d).w * (sized d).h * 4 * d.buffers ≥ OOM then .error OUTOFMEMORY
  else .ok (sized d)

/-- DXGI.CHAIN.*: a chain made is of 2 to 16 buffers of a flip format, of a size, one sample -/
theorem chain_made (d e : Desc) (a : Nat) (h : chain d a = .ok e) :
    2 ≤ e.buffers ∧ e.buffers ≤ 16 ∧ e.fmt ∈ flip ∧ 0 < e.w ∧ 0 < e.h ∧ e.count = 1 ∧ e.quality = 0 := by
  unfold chain at h
  split at h
  · cases h
  · split at h
    · cases h
    · cases h
      rename_i hb _
      simp only [bad, Bool.or_eq_true, Bool.not_eq_true', decide_eq_true_eq, bne_iff_ne, ne_eq] at hb
      simp only [sized]
      split <;> simp_all <;> omega

/-- DXGI.CHAIN.STEREO, USAGE, SCALING, ALPHA, FLAGS: everything but the size is kept as given;
the size too unless one of it is 0 -/
theorem chain_keeps (d e : Desc) (a : Nat) (h : chain d a = .ok e) :
    e.fmt = d.fmt ∧ e.stereo = d.stereo ∧ e.usage = d.usage ∧ e.buffers = d.buffers ∧ e.scaling = d.scaling ∧
    e.effect = d.effect ∧ e.alpha = d.alpha ∧ e.flags = d.flags ∧ (0 < d.w → 0 < d.h → e.w = d.w ∧ e.h = d.h) := by
  unfold chain at h
  split at h
  · cases h
  · split at h
    · cases h
    · cases h
      simp only [sized]
      split <;> simp_all <;> omega

/-- DXGI.CHAIN.FORMAT: no _SRGB format makes a flip chain (R8G8B8A8, B8G8R8A8, B8G8R8X8) -/
theorem chain_no_srgb (d : Desc) (a : Nat) (h : d.fmt ∈ [29, 91, 93]) : chain d a = .error INVALID_CALL := by
  have : bad d = true := by
    simp only [bad, Bool.or_eq_true]
    simp at h
    rcases h with h | h | h <;> simp [h, flip]
  simp [chain, this]

/-- DXGI.CHAIN.MEMORY: only x86 runs out of memory -/
theorem chain_memory (d : Desc) (a : Nat) (ha : a ≠ 32) : chain d a ≠ .error OUTOFMEMORY := by
  unfold chain
  split
  · simp [INVALID_CALL, OUTOFMEMORY]
  · simp [ha]

/-- DXGI.PRESENT: TEST, DO_NOT_SEQUENCE, RESTART, RESTRICT_TO_OUTPUT, ALLOW_TEARING -/
def PRESENT_FLAGS : Nat := 1 ||| 2 ||| 4 ||| 0x100 ||| 0x200

/-- DXGI.PRESENT: the refusal as a function of what it reads of the interval (past 4, not 0) and
the chain's flags (ALLOW_TEARING) -/
def refusedBy (late beyond tearing : Bool) (flags : Nat) (fresh : Bool) : Bool :=
  late || outside flags PRESENT_FLAGS || (flags &&& 0x200 != 0 && (!tearing || beyond)) ||
    (flags &&& 1 != 0 && flags != 1) || (flags &&& 2 != 0 && fresh)

/-- DXGI.PRESENT: Present refuses: sync intervals past 4, unknown flags, ALLOW_TEARING without
the chain's flag or with an interval, TEST with another flag, DO_NOT_SEQUENCE before the chain
has presented -/
def refused (sync flags cflags : Nat) (fresh : Bool) : Bool :=
  refusedBy (sync > 4) (sync != 0) (cflags &&& 0x800 != 0) flags fresh

/-- DXGI.PRESENT.RESTART: with any flags but TEST (and any interval, chain, state), RESTART
changes nothing: a present with it is refused exactly when one without it is -/
theorem restart_as_without (sync cflags : Nat) (fresh : Bool) :
    ∀ f, f < 1024 → f &&& 1 = 0 → refused sync (f ||| 4) cflags fresh = refused sync (f &&& 1019) cflags fresh := by
  have k : ∀ l b t fr, ∀ f, f < 1024 → f &&& 1 = 0 → refusedBy l b t (f ||| 4) fr = refusedBy l b t (f &&& 1019) fr := by
    decide +kernel
  intro f hf h1
  exact k _ _ _ _ f hf h1

/-- DXGI.PRESENT.TEST: TEST with any other flag is refused, whatever the interval and chain -/
theorem test_alone (sync cflags : Nat) (fresh : Bool) : ∀ b, b < 32 → 0 < b → refused sync (1 ||| 2 ^ b) cflags fresh = true := by
  have k : ∀ l c t fr, ∀ b, b < 32 → 0 < b → refusedBy l c t (1 ||| 2 ^ b) fr = true := by decide +kernel
  intro b hb hp
  exact k _ _ _ _ b hb hp

/-- DXGI.PRESENT: ALLOW_TEARING needs the chain's ALLOW_TEARING and interval 0 -/
theorem tearing_needs (sync flags cflags : Nat) (fresh : Bool) (h : flags &&& 0x200 ≠ 0)
    (ok : refused sync flags cflags fresh = false) : cflags &&& 0x800 ≠ 0 ∧ sync = 0 := by
  simp only [refused, refusedBy, Bool.or_eq_false_iff, Bool.and_eq_false_iff] at ok
  have := ok.1.1.2
  simp_all

/-- a rectangle: left, top, right, bottom -/
abbrev Rect := Int × Int × Int × Int

/-- a Present1 call: interval, flags, the scroll rectangle and offset if any, the dirty rects -/
structure P1 where
  sync : Nat
  flags : Nat
  scroll : Option (Rect × Int × Int)
  dirty : List Rect

/-- DXGI.PRESENT.DIRTY: Present1 refuses what Present does, and with rects: the first present
after the chain is made or resized not covering the buffer, TEST or DO_NOT_SEQUENCE, dirty
rects on FLIP_DISCARD, a scroll anywhere but FLIP_SEQUENTIAL, rects empty or outside the buffer,
the scroll source outside it -/
def refused1 (p : P1) (bw bh : Int) (effect cflags : Nat) (fresh : Bool) : Bool :=
  let empty : Rect → Bool := fun (l, t, r, b) => r ≤ l || b ≤ t
  let inside : Rect → Bool := fun (l, t, r, b) => 0 ≤ l && 0 ≤ t && r ≤ bw && b ≤ bh
  let base := refused p.sync p.flags cflags fresh
  if p.dirty.isEmpty && p.scroll.isNone then base
  else
    let full := p.dirty == [(0, 0, bw, bh)] && p.scroll.isNone
    let scrollBad := match p.scroll with
      | none => false
      | some ((l, t, r, b), dx, dy) => effect != 3 || empty (l, t, r, b) || !inside (l, t, r, b) || !inside (l - dx, t - dy, r - dx, b - dy)
    base || (fresh && !full) || p.flags &&& 3 != 0 || (!p.dirty.isEmpty && effect == 4) || scrollBad ||
      p.dirty.any (fun x => empty x || !inside x)

/-- DXGI.PRESENT.DIRTY: with no rects Present1 is Present -/
theorem refused1_plain (p : P1) (bw bh : Int) (e c : Nat) (fr : Bool) (h : p.dirty = []) (s : p.scroll = none) :
    refused1 p bw bh e c fr = refused p.sync p.flags c fr := by
  simp [refused1, h, s]

/-- DXGI.PRESENT.DIRTY: what Present1 takes with rects: not the first present unless the one
rect covers the buffer; no TEST, DO_NOT_SEQUENCE; each dirty rect a rect inside the buffer, none
on FLIP_DISCARD; a scroll only on FLIP_SEQUENTIAL, inside the buffer, its source too -/
theorem refused1_taken (p : P1) (bw bh : Int) (e c : Nat) (fr : Bool) (h : refused1 p bw bh e c fr = false)
    (some : p.dirty ≠ [] ∨ p.scroll ≠ none) :
    refused p.sync p.flags c fr = false ∧ (fr = true → p.dirty = [(0, 0, bw, bh)] ∧ p.scroll = none) ∧
    p.flags &&& 3 = 0 ∧ (p.dirty ≠ [] → e ≠ 4) ∧
    (∀ x ∈ p.dirty, x.1 < x.2.2.1 ∧ x.2.1 < x.2.2.2 ∧ 0 ≤ x.1 ∧ 0 ≤ x.2.1 ∧ x.2.2.1 ≤ bw ∧ x.2.2.2 ≤ bh) ∧
    (∀ r dx dy, p.scroll = .some (r, dx, dy) → e = 3 ∧ 0 ≤ r.1 ∧ 0 ≤ r.2.1 ∧ r.2.2.1 ≤ bw ∧ r.2.2.2 ≤ bh ∧
      0 ≤ r.1 - dx ∧ 0 ≤ r.2.1 - dy ∧ r.2.2.1 - dx ≤ bw ∧ r.2.2.2 - dy ≤ bh) := by
  have ne : (p.dirty.isEmpty && p.scroll.isNone) = false := by
    rcases some with d | s
    · cases hd : p.dirty with
      | nil => exact absurd hd d
      | cons _ _ => simp
    · cases hs : p.scroll with
      | none => exact absurd hs s
      | some _ => simp
  simp only [refused1, ne, Bool.false_eq_true, ↓reduceIte, Bool.or_eq_false_iff] at h
  obtain ⟨⟨⟨⟨⟨base, first⟩, test⟩, discard⟩, scroll⟩, rects⟩ := h
  refine ⟨base, ?_, by simpa using test, ?_, ?_, ?_⟩
  · intro hf
    simp only [hf, Bool.true_and, Bool.not_eq_false', Bool.and_eq_true, beq_iff_eq, Option.isNone_iff_eq_none] at first
    exact first
  · intro hd
    cases hd' : p.dirty with
    | nil => exact absurd hd' hd
    | cons _ _ => simp [hd'] at discard; exact discard
  · intro x hx
    have := List.any_eq_false.mp rects x hx
    obtain ⟨l, t, r, b⟩ := x
    simp at this
    simp only
    omega
  · intro r dx dy hs
    obtain ⟨l, t, rr, b⟩ := r
    simp [hs] at scroll
    simp only
    omega

def mirrors : List Mirror := [
  ⟨"dxgi.assoc", fun | [f] => do pure [assoc (← nat f)] | _ => none⟩,
  ⟨"dxgi.feature", fun | [f, s] => do pure [if feature (← nat f) (← nat s) then 1 else 0] | _ => none⟩,
  ⟨"dxgi.hdr", fun | [k, s, n] => do pure [if hdr (← nat k) (← nat s) (n != 0) then 1 else 0] | _ => none⟩,
  ⟨"dxgi.spaces", fun | [f] => do pure ((spaces (← nat f)).map Int.ofNat) | _ => none⟩,
  ⟨"dxgi.chain", fun
    | [w, h, fmt, stereo, count, quality, usage, buffers, scaling, effect, alpha, flags, arch] => do
      let d : Desc := ⟨← nat w, ← nat h, ← nat fmt, ← nat stereo, ← nat count, ← nat quality, ← nat usage,
        ← nat buffers, ← nat scaling, ← nat effect, ← nat alpha, ← nat flags⟩
      match chain d (← nat arch) with
      | .error e => pure [1, e]
      | .ok e => pure ([0, e.w, e.h, e.fmt, e.stereo, e.count, e.quality, e.usage, e.buffers, e.scaling, e.effect, e.alpha, e.flags].map Int.ofNat)
    | _ => none⟩,
  ⟨"dxgi.present", fun
    | [s, f, c, fr] => do pure [if refused (← nat s) (← nat f) (← nat c) (fr != 0) then 1 else 0]
    | _ => none⟩,
  ⟨"dxgi.present1", fun
    | s :: f :: fr :: e :: c :: bw :: bh :: sc :: l :: t :: r :: b :: dx :: dy :: rects => do
      let rec quads : List Int → Option (List Rect)
        | [] => some []
        | a :: b :: c :: d :: rest => (quads rest).map ((a, b, c, d) :: ·)
        | _ => none
      let p : P1 := ⟨← nat s, ← nat f, if sc != 0 then some ((l, t, r, b), dx, dy) else none, ← quads rects⟩
      pure [if refused1 p bw bh (← nat e) (← nat c) (fr != 0) then 1 else 0]
    | _ => none⟩
]

end Kikizake.Dxgi.Args
