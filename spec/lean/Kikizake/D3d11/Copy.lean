import Kikizake.Mirror
/-! UpdateSubresource's and CopySubresourceRegion's boxes and what they do (spec/d3dres.txt
D3DRES.UPDATE*, COPYSUB*, REMOVED), as src/model/d3dres.rs computes them: `boxElems` is
`box_elems`, `updatePlan` `update_plan`, `regionPlan` `region_plan` (mirrors `d3d11.box`,
`d3d11.update`, `d3d11.region`). -/
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
or no box (`has` false): the whole of a subresource of c elements a row, r rows, s slices -/
def boxElems (has buffer : Bool) (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) : Box :=
  if !has then ⟨0, 0, 0, c, r, s⟩
  else if buffer then ⟨x0, 0, 0, x3, 1, 1⟩
  else
    let w := max bw 1
    let h := max bh 1
    ⟨x0 / w, x1 / h, x2, (x3 + w - 1) / w, (x4 + h - 1) / h, x5⟩

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
the box `b` of a subresource of (c, r, s) -/
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
      if empty b then .nothing
      else if past b b.l b.t b.f sc sr ss || past b ax ay az dc dr ds then .removed
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

/-- a buffer's box: its left and right bytes, one row, one slice -/
theorem buffer_box (bw bh x0 x1 x2 x3 x4 x5 c r s : Nat) :
    boxElems true true bw bh x0 x1 x2 x3 x4 x5 c r s = ⟨x0, 0, 0, x3, 1, 1⟩ := by
  simp [boxElems]

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

/-- an empty box: nothing -/
theorem region_empty (ignored : Bool) (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat)
    (h : empty b = true) : regionPlan ignored sides b sc sr ss dc dr ds ax ay az = .nothing := by
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
the device -/
theorem region_removed (sides : Sides) (b : Box) (sc sr ss dc dr ds ax ay az : Nat)
    (hs : match sides with | .mixed => False | .textures c => c = true | .buffers => True)
    (he : empty b = false) (hp : past b b.l b.t b.f sc sr ss = true ∨ past b ax ay az dc dr ds = true) :
    regionPlan false sides b sc sr ss dc dr ds ax ay az = .removed := by
  unfold regionPlan
  have hor : (past b b.l b.t b.f sc sr ss || past b ax ay az dc dr ds) = true := by
    cases hp <;> simp_all
  cases sides with
  | mixed => exact absurd hs id
  | textures c => simp only at hs; subst hs; simp [he, hor]
  | buffers => simp [he, hor]

def args6 (a : List Int) : Option (List Nat) := a.mapM nat

def mirrors : List Mirror := [
  ⟨"d3d11.box", fun a => do
    let v ← args6 a
    match v with
    | [has, buffer, bw, bh, x0, x1, x2, x3, x4, x5, c, r, s] =>
      let e := boxElems (has != 0) (buffer != 0) bw bh x0 x1 x2 x3 x4 x5 c r s
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
    | _ => none⟩
]

end Kikizake.D3d11.Copy
