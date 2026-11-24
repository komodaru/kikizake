import Kikizake.F32
/-! UpdateSubresource's and CopySubresourceRegion's boxes and what they do (spec/d3dres.txt
D3DRES.UPDATE*, COPYSUB*, REMOVED), as src/model/d3dres.rs computes them: `boxElems` is
`box_elems`, `clip` `box_clip`, `updatePlan` `update_plan`, `regionPlan` `region_plan`,
`resolvePlan` `resolve_plan`, `compatible` `copy_compatible` (mirrors `d3d11.box`,
`d3d11.boxclip`, `d3d11.update`, `d3d11.region`, `d3d11.resolve`, `d3d11.copyfmt`). -/
namespace Kikizake.D3d11.Copy

/-- a box in elements: left, top, front inclusive, right, bottom, back exclusive -/
structure Box where
  l : Nat
  t : Nat
  f : Nat
  r : Nat
  b : Nat
  k : Nat
  deriving DecidableEq, Repr

/-- D3D11_BOX (in pixels; a buffer's left and right in bytes) in elements of bw by bh pixels,
or no box (`has` false): the whole of a subresource of c elements a row, r rows, s slices;
`flat`: one row from its top and one slice from its front, whatever its bottom and back (a 1D
texture's in UpdateSubresource) -/
def boxElems (has flat : Bool) (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) : Box :=
  if !has then ⟨0, 0, 0, c, r, s⟩
  else
    let w := max bw 1
    let h := max bh 1
    if flat then ⟨x0 / w, x1, x2, (x3 + w - 1) / w, x1 + 1, x2 + 1⟩
    else ⟨x0 / w, x1 / h, x2, (x3 + w - 1) / w, (x4 + h - 1) / h, x5⟩

/-- a box cut to a subresource of (c, r, s) -/
def clip (b : Box) (c r s : Nat) : Box := ⟨b.l, b.t, b.f, min b.r c, min b.b r, min b.k s⟩

/-- a box of no element -/
def empty (b : Box) : Bool := b.r ≤ b.l || b.b ≤ b.t || b.k ≤ b.f

/-- the box's extent put at (ax, ay, az) reaches past a subresource of (wx, wy, wz) -/
def past (b : Box) (ax ay az wx wy wz : Nat) : Bool :=
  wx < ax + (b.r - b.l) || wy < ay + (b.b - b.t) || wz < az + (b.k - b.f)

inductive Plan where
  | nothing
  | removed
  | copy
  deriving DecidableEq, Repr

def Plan.code : Plan → Int
  | .nothing => 0
  | .removed => 1
  | .copy => 2

/-- UpdateSubresource of subresource `sub` of `nsubs` of a resource of `usage`, `count` samples,
the box `b`, against the resource's top level of (c, r, s) (the write is then cut to the
subresource with `clip`) -/
def updatePlan (sub nsubs usage count : Nat) (b : Box) (c r s : Nat) : Plan :=
  if nsubs ≤ sub then .nothing
  else if usage ≠ 0 ∨ 1 < count then .nothing
  else if empty b then .nothing
  else if past b b.l b.t b.f c r s then .removed
  else .copy

/-- the two sides of a region copy -/
inductive Sides where
  | buffers
  | textures (compatible : Bool)
  | mixed

/-- CopySubresourceRegion of the box `b` of a source subresource of (sc, sr, ss) to (ax, ay, az)
of a destination one of (dc, dr, ds) -/
def regionPlan (ignored : Bool) (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat) : Plan :=
  if ignored then .nothing
  else match sides with
    | .mixed => .nothing
    | .textures false => .nothing
    | _ =>
      if past b b.l b.t b.f sc sr ss || past b ax ay az dc dr ds then .removed
      else if empty b then .nothing
      else .copy

/-- ResolveSubresource of a source of `sn` samples to a destination of `dn` samples and `du`
usage: `groups` the format's typeless group is both resources', `int` an integer format,
`subs` both subresources there and the dimensions agree; the source's row bytes and rows
(sw, sh), the destination's (dw, dh) -/
def resolvePlan (sn dn du : Nat) (groups int subs : Bool) (sw sh dw dh : Nat) : Plan :=
  if dn ≠ 1 ∨ du ≠ 0 ∨ groups = false ∨ int = true then .nothing
  else if subs = false then .nothing
  else if sn < 2 ∧ (dw < sw ∨ dh < sh) then .removed
  else .copy

/-! ## D3DRES.UPDATE.BOX -/

/-- no box: the whole subresource -/
theorem whole (buffer : Bool) (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) :
    boxElems false buffer bw bh x0 x1 x2 x3 x4 x5 c r s = ⟨0, 0, 0, c, r, s⟩ := by
  simp [boxElems]

/-- a box of a block format, in pixels: the blocks it touches, from the one holding its left
and top pixel to the one holding its last (block aligned, exactly the blocks) -/
theorem blocks (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) (hw : 0 < bw) (hh : 0 < bh) :
    let e := boxElems true false bw bh x0 x1 x2 x3 x4 x5 c r s
    e.l * bw ≤ x0 ∧ x0 < (e.l + 1) * bw ∧ x3 ≤ e.r * bw ∧ e.r * bw < x3 + bw ∧
    e.t * bh ≤ x1 ∧ x1 < (e.t + 1) * bh ∧ x4 ≤ e.b * bh ∧ e.b * bh < x4 + bh ∧ e.f = x2 ∧ e.k = x5 := by
  have hmw : max bw 1 = bw := by omega
  have hmh : max bh 1 = bh := by omega
  simp only [boxElems, Bool.not_true, Bool.false_eq_true, ite_false, hmw, hmh]
  refine ⟨Nat.div_mul_le_self x0 bw, ?_, ?_, ?_, Nat.div_mul_le_self x1 bh, ?_, ?_, ?_, trivial, trivial⟩
  · have := Nat.lt_div_mul_add (a := x0) hw; rw [Nat.add_mul, Nat.one_mul]; omega
  · have := Nat.div_mul_le_self (x3 + bw - 1) bw
    have := Nat.lt_div_mul_add (a := x3 + bw - 1) hw
    omega
  · have := Nat.div_mul_le_self (x3 + bw - 1) bw; omega
  · have := Nat.lt_div_mul_add (a := x1) hh; rw [Nat.add_mul, Nat.one_mul]; omega
  · have := Nat.div_mul_le_self (x4 + bh - 1) bh
    have := Nat.lt_div_mul_add (a := x4 + bh - 1) hh
    omega
  · have := Nat.div_mul_le_self (x4 + bh - 1) bh; omega

/-- a buffer's box (elements of one byte): its left and right bytes, top, bottom, front, back -/
theorem buffer_box (x0 x1 x2 x3 x4 x5 c r s : Nat) :
    boxElems true false 1 1 x0 x1 x2 x3 x4 x5 c r s = ⟨x0, x1, x2, x3, x4, x5⟩ := by
  simp [boxElems]

/-- a 1D texture's box in an update: one row from its top, one slice from its front -/
theorem flat_box (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) :
    let e := boxElems true true bw bh x0 x1 x2 x3 x4 x5 c r s
    e.t = x1 ∧ e.b = x1 + 1 ∧ e.f = x2 ∧ e.k = x2 + 1 := by
  simp [boxElems]

/-- the cut box lies inside the subresource, and a box inside it is not cut -/
theorem clip_inside (b : Box) (c r s : Nat) :
    let e := clip b c r s
    e.r ≤ c ∧ e.b ≤ r ∧ e.k ≤ s ∧ e.l = b.l ∧ e.t = b.t ∧ e.f = b.f ∧
      (b.r ≤ c → b.b ≤ r → b.k ≤ s → e = b) := by
  simp only [clip]
  refine ⟨Nat.min_le_right _ _, Nat.min_le_right _ _, Nat.min_le_right _ _, trivial, trivial, trivial, ?_⟩
  intro h1 h2 h3
  cases b; simp_all [Nat.min_eq_left]

/-! ## D3DRES.UPDATE, UPDATE.IGNORED, UPDATE.BAD, UPDATE.EMPTY, REMOVED -/

/-- the update copies exactly when the subresource exists, the resource is of default usage and
single-sampled, and the box holds an element and lies inside the subresource: left, top, front
inclusive, right, bottom, back exclusive -/
theorem update_copies (sub nsubs usage count : Nat) (b : Box) (c r s : Nat) :
    updatePlan sub nsubs usage count b c r s = .copy ↔
      sub < nsubs ∧ usage = 0 ∧ count ≤ 1 ∧ b.l < b.r ∧ b.t < b.b ∧ b.f < b.k ∧ b.r ≤ c ∧ b.b ≤ r ∧ b.k ≤ s := by
  unfold updatePlan past empty
  by_cases h1 : nsubs ≤ sub <;> by_cases h2 : (usage ≠ 0 ∨ 1 < count) <;> simp [h1, h2] <;> (try omega) <;>
    (try split) <;> simp_all <;> omega

/-- a subresource past the last: nothing -/
theorem update_ignored (sub nsubs usage count : Nat) (b : Box) (c r s : Nat) (h : nsubs ≤ sub) :
    updatePlan sub nsubs usage count b c r s = .nothing := by
  simp [updatePlan, h]

/-- a resource not of default usage, or multisampled: nothing -/
theorem update_bad (sub nsubs usage count : Nat) (b : Box) (c r s : Nat) (h : usage ≠ 0 ∨ 1 < count) :
    updatePlan sub nsubs usage count b c r s = .nothing := by
  unfold updatePlan; split <;> simp_all

/-- an empty box: nothing -/
theorem update_empty (sub nsubs usage count : Nat) (b : Box) (c r s : Nat) (h : empty b = true) :
    updatePlan sub nsubs usage count b c r s = .nothing := by
  unfold updatePlan; split <;> (try split) <;> simp_all

/-- a box past the subresource of an update that would run removes the device -/
theorem update_removed (sub nsubs usage count : Nat) (b : Box) (c r s : Nat)
    (hs : sub < nsubs) (hu : usage = 0) (hc : count ≤ 1) (he : empty b = false)
    (hp : c < b.r ∨ r < b.b ∨ s < b.k) :
    updatePlan sub nsubs usage count b c r s = .removed := by
  have h1 : ¬ nsubs ≤ sub := by omega
  have h2 : ¬ (usage ≠ 0 ∨ 1 < count) := by omega
  have hl : b.l < b.r ∧ b.t < b.b ∧ b.f < b.k := by simp [empty] at he; omega
  have hpast : past b b.l b.t b.f c r s = true := by simp [past]; omega
  simp [updatePlan, h1, h2, he, hpast]

/-! ## D3DRES.COPYSUB, COPYSUB.BAD, COPYSUB.IGNORED, REMOVED -/

/-- a missing subresource on either side, an immutable destination, sample counts that differ:
nothing -/
theorem region_ignored (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat) :
    regionPlan true sides b sc sr ss dc dr ds ax ay az = .nothing := by
  simp [regionPlan]

/-- a buffer and a texture: nothing -/
theorem region_mixed (ignored : Bool) (b : Box) (sc sr ss dc dr ds ax ay az : Nat) :
    regionPlan ignored .mixed b sc sr ss dc dr ds ax ay az = .nothing := by
  cases ignored <;> simp [regionPlan]

/-- an empty box inside both subresources: nothing -/
theorem region_empty (ignored : Bool) (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat)
    (h : empty b = true) (hs : past b b.l b.t b.f sc sr ss = false) (hd : past b ax ay az dc dr ds = false) :
    regionPlan ignored sides b sc sr ss dc dr ds ax ay az = .nothing := by
  unfold regionPlan
  cases ignored <;> cases sides <;> simp_all <;> split <;> simp_all

/-- the copy runs exactly when nothing refuses it, the box holds an element, lies inside the
source subresource, and its extent put at (x, y, z) inside the destination one -/
theorem region_copies (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat)
    (hs : match sides with | .mixed => False | .textures c => c = true | .buffers => True) :
    regionPlan false sides b sc sr ss dc dr ds ax ay az = .copy ↔
      b.l < b.r ∧ b.t < b.b ∧ b.f < b.k ∧ b.r ≤ sc ∧ b.b ≤ sr ∧ b.k ≤ ss ∧
      ax + (b.r - b.l) ≤ dc ∧ ay + (b.b - b.t) ≤ dr ∧ az + (b.k - b.f) ≤ ds := by
  unfold regionPlan past empty
  cases sides with
  | mixed => exact absurd hs id
  | textures c =>
    simp only at hs; subst hs
    simp only [Bool.false_eq_true, ite_false]
    split <;> (try split) <;> simp_all <;> omega
  | buffers =>
    simp only [Bool.false_eq_true, ite_false]
    split <;> (try split) <;> simp_all <;> omega

/-- a box past the source, or a region past the destination, of a copy that would run removes
the device, an empty box's too -/
theorem region_removed (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat)
    (hs : match sides with | .mixed => False | .textures c => c = true | .buffers => True)
    (hp : past b b.l b.t b.f sc sr ss = true ∨ past b ax ay az dc dr ds = true) :
    regionPlan false sides b sc sr ss dc dr ds ax ay az = .removed := by
  unfold regionPlan
  have hor : (past b b.l b.t b.f sc sr ss || past b ax ay az dc dr ds) = true := by
    cases hp <;> simp_all
  cases sides with
  | mixed => exact absurd hs id
  | textures c => simp only at hs; subst hs; simp [hor]
  | buffers => simp [hor]

/-! ## D3DRES.RESOLVE, RESOLVE.IGNORED, RESOLVE.BAD, RESOLVE.SIZE, REMOVED -/

/-- a multisampled destination or one not of default usage, a format of another group, an
integer format: nothing -/
theorem resolve_ignored (sn dn du : Nat) (groups int subs : Bool) (sw sh dw dh : Nat)
    (h : dn ≠ 1 ∨ du ≠ 0 ∨ groups = false ∨ int = true) :
    resolvePlan sn dn du groups int subs sw sh dw dh = .nothing := by
  simp [resolvePlan, h]

/-- a missing subresource or other dimensions: nothing -/
theorem resolve_bad (sn dn du : Nat) (groups int : Bool) (sw sh dw dh : Nat) :
    resolvePlan sn dn du groups int false sw sh dw dh = .nothing := by
  unfold resolvePlan; split <;> simp

/-- a single-sampled source larger than the destination, resolved where the rest allows it,
removes the device -/
theorem resolve_removed (sw sh dw dh : Nat) (h : dw < sw ∨ dh < sh) :
    resolvePlan 1 1 0 true false true sw sh dw dh = .removed := by
  simp [resolvePlan, h]

/-- the resolve runs exactly when nothing refuses it and the source is multisampled or fits the
destination: where both are, whatever their sizes (RESOLVE.SIZE) -/
theorem resolve_copies (sn dn du : Nat) (groups int subs : Bool) (sw sh dw dh : Nat) :
    resolvePlan sn dn du groups int subs sw sh dw dh = .copy ↔
      dn = 1 ∧ du = 0 ∧ groups = true ∧ int = false ∧ subs = true ∧ (2 ≤ sn ∨ (sw ≤ dw ∧ sh ≤ dh)) := by
  unfold resolvePlan
  cases groups <;> cases int <;> cases subs <;> simp <;>
    by_cases h1 : dn = 1 <;> by_cases h2 : du = 0 <;> simp [h1, h2] <;>
    (try split) <;> (try simp_all) <;> omega

/-! ## D3DRES.COPY.FORMAT -/

/-- what CopyResource asks of a format the model knows: its element is a block, its bits, its
numbers (0 typeless, 1 unorm, 2 snorm, 3 uint, 4 sint, 5 float, 6 srgb, 7 its own layout), its
component count, all components of one width; the format tables' part is data -/
structure Fmt where
  block : Bool
  bits : Nat
  num : Nat
  n : Nat
  uniform : Bool
  deriving DecidableEq, Repr

/-- a block compressed format c and an uncompressed one u of the block's bits with two or more
components of one width, integer or float (or typeless) -/
def blockPair (c u : Fmt) : Bool :=
  c.block && !u.block && c.bits == u.bits && (u.num == 0 || u.num == 3 || u.num == 4 || u.num == 5) &&
    2 ≤ u.n && u.uniform

/-- R9G9B9E5_SHAREDEXP (67) and R32_TYPELESS, FLOAT, UINT (41..43), either way -/
def sharedExp (a b : Nat) : Bool := (a == 67 && 41 ≤ b && b ≤ 43) || (b == 67 && 41 ≤ a && a ≤ 43)

/-- formats a and b of typeless groups ga, gb, their facts if the model knows them; `d10` the
device is d3d10.dll -/
def compatible (d10 : Bool) (a b ga gb : Nat) (fa fb : Option Fmt) : Bool :=
  if ga = gb then true
  else match fa, fb with
    | some x, some y => !d10 && (sharedExp a b || blockPair x y || blockPair y x)
    | _, _ => false

/-- one typeless group: compatible -/
theorem copy_group (d10 : Bool) (a b g : Nat) (fa fb : Option Fmt) : compatible d10 a b g g fa fb = true := by
  simp [compatible]

/-- d3d10.dll copies only within a typeless group -/
theorem copy_d3d10 (a b ga gb : Nat) (fa fb : Option Fmt) :
    compatible true a b ga gb fa fb = true ↔ ga = gb := by
  unfold compatible
  by_cases h : ga = gb <;> simp [h]
  cases fa <;> cases fb <;> simp

/-- on Direct3D 10.1 and 11, formats of other groups copy exactly when one is block compressed
and the other uncompressed of the block's bits with equal integer or float components, or they
are R9G9B9E5_SHAREDEXP and a 32-bit single-component format of the R32 group (measured) -/
theorem copy_other (a b ga gb : Nat) (x y : Fmt) (h : ga ≠ gb) :
    compatible false a b ga gb (some x) (some y) = true ↔
      ((a = 67 ∧ 41 ≤ b ∧ b ≤ 43) ∨ (b = 67 ∧ 41 ≤ a ∧ a ≤ 43)) ∨
      (x.block = true ∧ y.block = false ∧ x.bits = y.bits ∧ (y.num = 0 ∨ y.num = 3 ∨ y.num = 4 ∨ y.num = 5) ∧ 2 ≤ y.n ∧ y.uniform = true) ∨
      (y.block = true ∧ x.block = false ∧ y.bits = x.bits ∧ (x.num = 0 ∨ x.num = 3 ∨ x.num = 4 ∨ x.num = 5) ∧ 2 ≤ x.n ∧ x.uniform = true) := by
  simp [compatible, h, sharedExp, blockPair, and_assoc, or_assoc]

/-- a format the model does not know copies only within its group -/
theorem copy_unknown (d10 : Bool) (a b ga gb : Nat) (fb : Option Fmt) (h : ga ≠ gb) :
    compatible d10 a b ga gb none fb = false := by
  simp [compatible, h]

def integerMean (sum count : Nat) : Nat := (sum + count / 2) / count

/-- The exact interval of sums that round to the given value. -/
theorem integer_mean_interval (sum count value : Nat) (h : 0 < count) :
    integerMean sum count = value ↔
      value * count ≤ sum + count / 2 ∧ sum + count / 2 ≤ value * count + count - 1 := by
  exact Nat.div_eq_iff h

/-- An exact tie is resolved to the higher integer for every even sample count. -/
theorem integer_mean_tie (value half : Nat) (h : 0 < half) :
    integerMean ((2 * half) * value + half) (2 * half) = value + 1 := by
  unfold integerMean
  have hd : 0 < 2 * half := by omega
  have hn : (2 * half) * value + half + 2 * half / 2 = (2 * half) * (value + 1) := by
    rw [Nat.mul_add]
    omega
  rw [hn, Nat.mul_div_cancel_left _ hd]

def nearestEven (n d : Nat) : Nat :=
  let q := n / d
  let r := n % d
  if 2*r > d ∨ (2*r = d ∧ q % 2 = 1) then q+1 else q

def wordMean (samples : List Nat) : Nat :=
  let scale := F32.ofNat 65535
  let sum := samples.foldl (fun acc v => F32.add acc (F32.div (F32.ofNat v) scale)) 0
  let x := F32.mul (F32.div sum (F32.ofNat samples.length)) scale
  let (_, n, d) := Float.toRat F32.fmt x
  min 65535 (nearestEven n d)

/-- At an exact midpoint the low bit of the quotient determines the increment. -/
theorem word_round_tie (n d : Nat) (h : 2 * (n % d) = d) :
    nearestEven n d = n / d + (n / d) % 2 := by
  simp only [nearestEven, h, Nat.lt_irrefl, false_or, true_and]
  split <;> omega

theorem word_mean_range (samples : List Nat) : wordMean samples ≤ 65535 := by
  exact Nat.min_le_left _ _

def args6 (a : List Int) : Option (List Nat) := a.mapM nat

def mirrors : List Mirror := [
  ⟨"d3d11.wordMean", fun a =>
    if a.isEmpty || a.length > 32 || a.any (fun v => v < 0 || v > 65535) then some [-1]
    else some [(wordMean (a.map Int.toNat) : Int)]⟩,
  ⟨"d3d11.integerMean", fun a => match a with
    | [s, n] => some [if s < 0 || n ≤ 0 then -1 else (integerMean s.toNat n.toNat : Int)]
    | _ => some [-1]⟩,
  ⟨"d3d11.box", fun a => do
    let v ← args6 a
    match v with
    | [has, flat, bw, bh, x0, x1, x2, x3, x4, x5, c, r, s] =>
      let e := boxElems (has != 0) (flat != 0) bw bh x0 x1 x2 x3 x4 x5 c r s
      pure [e.l, e.t, e.f, e.r, e.b, e.k]
    | _ => none⟩,
  ⟨"d3d11.boxclip", fun a => do
    let v ← args6 a
    match v with
    | [l, t, f, r, b, k, c, rr, s] =>
      let e := clip ⟨l, t, f, r, b, k⟩ c rr s
      pure [e.l, e.t, e.f, e.r, e.b, e.k]
    | _ => none⟩,
  ⟨"d3d11.update", fun a => do
    let v ← args6 a
    match v with
    | [sub, nsubs, usage, count, l, t, f, r, b, k, c, rr, s] =>
      pure [(updatePlan sub nsubs usage count ⟨l, t, f, r, b, k⟩ c rr s).code]
    | _ => none⟩,
  ⟨"d3d11.region", fun a => do
    let v ← args6 a
    match v with
    | [ignored, sides, l, t, f, r, b, k, sc, sr, ss, dc, dr, ds, ax, ay, az] =>
      let sd ← match sides with
        | 0 => some Sides.buffers | 1 => some (Sides.textures false) | 2 => some (Sides.textures true)
        | 3 => some Sides.mixed | _ => none
      pure [(regionPlan (ignored != 0) sd ⟨l, t, f, r, b, k⟩ sc sr ss dc dr ds ax ay az).code]
    | _ => none⟩,
  ⟨"d3d11.resolve", fun a => do
    let v ← args6 a
    match v with
    | [sn, dn, du, groups, int, subs, sw, sh, dw, dh] =>
      pure [(resolvePlan sn dn du (groups != 0) (int != 0) (subs != 0) sw sh dw dh).code]
    | _ => none⟩,
  ⟨"d3d11.copyfmt", fun a => do
    let v ← args6 a
    match v with
    | [dev, x, y, gx, gy, kx, bx, sx, nx, cx, ux, ky, by_, sy, ny, cy, uy] =>
      let f := fun (k b s n c u : Nat) => if k = 0 then none else some (Fmt.mk (b != 0) s n c (u != 0))
      pure [if compatible (dev == 0) x y gx gy (f kx bx sx nx cx ux) (f ky by_ sy ny cy uy) then 1 else 0]
    | _ => none⟩
]

end Kikizake.D3d11.Copy
