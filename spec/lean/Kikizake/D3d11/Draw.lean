import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Raster
/-! What a draw on Direct3D 10 and 11 does before it rasterizes (spec/d3dres.txt D3DRASTER.*), as
src/model/d3drules.rs computes it: `verdict` is `draw_verdict` (programs, views, viewport,
D3D11_PRIMITIVE_TOPOLOGY), `primitive` `primitive` (the vertices each topology takes),
`primCount` `prim_count`, `vertexWord` `vertex_word` (the vertex buffer's words), `bind`
`bind_targets` (OMSetRenderTargets). The generated programs' shader stage, the selection of a
depth-only target and drawable rows are held to `shader_plan`, `target_plan` and `target_rows`
by `d3d11.shaderStage`, `d3d11.targetPlan` and `d3d11.targetRows`. -/
namespace Kikizake.D3d11.Draw

/-- the topologies drawn: D3D11_PRIMITIVE_TOPOLOGY 1 to 5, 10 to 13 -/
inductive Topo where
  | points | lines | lineStrip | tris | triStrip | linesAdj | lineStripAdj | trisAdj | triStripAdj
  deriving DecidableEq, Repr

def Topo.of : Nat → Option Topo
  | 1 => some .points
  | 2 => some .lines
  | 3 => some .lineStrip
  | 4 => some .tris
  | 5 => some .triStrip
  | 10 => some .linesAdj
  | 11 => some .lineStripAdj
  | 12 => some .trisAdj
  | 13 => some .triStripAdj
  | _ => none

/-- the topology's number -/
def Topo.code : Topo → Nat
  | .points => 1 | .lines => 2 | .lineStrip => 3 | .tris => 4 | .triStrip => 5
  | .linesAdj => 10 | .lineStripAdj => 11 | .trisAdj => 12 | .triStripAdj => 13

/-- what a Draw does -/
inductive Verdict where
  | incomplete | noViewport | crash | removed | nothing
  | draw (t : Topo)
  deriving DecidableEq, Repr

/-- D3DRASTER.INCOMPLETE, NOVIEWPORT, TOPOLOGY, PATCH: a draw with programs and an input layout
or not, a target or depth view or not, a viewport or not, of topology top -/
def verdict (programs view viewport : Bool) (top : Nat) : Verdict :=
  if !programs || !view then .incomplete
  else if !viewport then .noViewport
  else match Topo.of top with
    | some t => .draw t
    | none => if 33 ≤ top ∧ top ≤ 64 then .crash else if top = 6 then .nothing else .removed

/-- D3DRASTER.INCOMPLETE, NOVIEWPORT: without programs and an input layout, or without a target
or depth view to draw to, nothing is drawn (whatever the rest); without a viewport neither -/
theorem incomplete (p v vp : Bool) (top : Nat) :
    verdict false v vp top = .incomplete ∧ verdict p false vp top = .incomplete ∧
    verdict true true false top = .noViewport := by
  simp [verdict]

/-- D3DRASTER.TOPOLOGY, PATCH: with everything bound, each defined topology draws as itself;
6 draws nothing; 0, 7, 8, 9 and past 64 remove the device; 33 to 64 (patch lists, no hull and
domain programs) end the executor -/
theorem topology (top : Nat) :
    (∀ t : Topo, verdict true true true t.code = .draw t) ∧
    verdict true true true 6 = .nothing ∧
    (top = 0 ∨ top = 7 ∨ top = 8 ∨ top = 9 ∨ 64 < top → verdict true true true top = .removed) ∧
    (33 ≤ top → top ≤ 64 → verdict true true true top = .crash) := by
  refine ⟨fun t => by cases t <;> rfl, by decide, fun h => ?_, fun h1 h2 => ?_⟩
  · have none : Topo.of top = none := by
      unfold Topo.of
      rcases h with h | h | h | h | h
      · subst h; rfl
      · subst h; rfl
      · subst h; rfl
      · subst h; rfl
      · split <;> first | rfl | omega
    have : ¬(33 ≤ top ∧ top ≤ 64) := by omega
    have : top ≠ 6 := by omega
    simp [verdict, *]
  · have none : Topo.of top = none := by
      unfold Topo.of
      split <;> first | rfl | omega
    simp [verdict, none, h1, h2]

/-- The case command uses zero instances for Draw, otherwise DrawInstanced starts at instance zero. -/
def drawIds (instances primitives : Nat) : List (Nat × List Nat) :=
  (List.range (max 1 instances)).map (fun i => (i, List.range primitives))

/-- The system values at given instance and primitive offsets, without constructing the draw. -/
def drawIdAt (instances primitives i p : Nat) : Option (Nat × Nat) :=
  if i < max 1 instances ∧ p < primitives then some (i, p) else none

/-- Every instance restarts the primitive index at zero and has its own zero-based instance index. -/
theorem draw_ids_offsets (instances primitives i p : Nat) :
    (drawIds instances primitives)[i]?.bind (fun (j, ps) => ps[p]?.map (fun q => (j, q))) =
      drawIdAt instances primitives i p := by
  by_cases hi : i < max 1 instances <;> by_cases hp : p < primitives <;>
    simp [drawIds, drawIdAt, hi, hp]

/-- A Draw has only instance zero, even if its primitive count is zero. -/
theorem noninstanced_ids (primitives i p : Nat) :
    drawIdAt 0 primitives i p = if i = 0 ∧ p < primitives then some (i, p) else none := by
  simp [drawIdAt, Nat.lt_one_iff]

/-- The generated vertex shader converts SV_VertexID and SV_InstanceID to binary32. -/
def vertexIdColor (v i : Nat) : List Nat :=
  [F32.ofNat v, F32.ofNat i, 0, 0x3f800000]

/-- The generated pixel shader converts SV_PrimitiveID and SV_IsFrontFace to binary32. -/
def primitiveIdColor (p : Nat) (front : Bool) : List Nat :=
  [F32.ofNat p, if front then 0x3f800000 else 0, 0, 0x3f800000]

/-- Offset zero and one from the vertex shader hold vertex and instance IDs independently. -/
theorem vertex_id_channels (v i : Nat) :
    (vertexIdColor v i)[0]? = some (F32.ofNat v) ∧
    (vertexIdColor v i)[1]? = some (F32.ofNat i) := by
  simp [vertexIdColor]

/-- The uint shader conversions round ties to even at the binary32 precision boundary. -/
theorem vertex_id_rounding :
    vertexIdColor 16777217 16777219 = [0x4b800000, 0x4b800002, 0, 0x3f800000] := by decide

/-- FrontCounterClockwise toggles the facing channel without changing the primitive index. -/
theorem primitive_id_facing (p : Nat) (cw : Bool) :
    primitiveIdColor p (Raster.frontFace false cw) =
      [F32.ofNat p, if cw then 0x3f800000 else 0, 0, 0x3f800000] ∧
    primitiveIdColor p (Raster.frontFace true cw) =
      [F32.ofNat p, if cw then 0 else 0x3f800000, 0, 0x3f800000] := by
  cases cw <;> simp [primitiveIdColor, Raster.frontFace]

/-- As in the vertex shader, a primitive ID beyond the exact binary32 range is rounded. -/
theorem primitive_id_rounding :
    primitiveIdColor 16777217 true = [0x4b800000, 0x3f800000, 0, 0x3f800000] := by decide

/-- a primitive's vertices (their indices in the draw), a triangle's leading vertex last -/
inductive Prim where
  | point (v : Nat)
  | line (a b : Nat)
  | tri (a b c lead : Nat)
  deriving DecidableEq, Repr

/-- The provoking vertex, independent of the rasterizer's vertex order. -/
def Prim.leading : Prim → Nat
  | .point v | .line v _ | .tri _ _ _ v => v

/-- the vertex indices doubled (an adjacency topology's vertices without the adjacent ones) -/
def Prim.double : Prim → Prim
  | .point v => .point (2 * v)
  | .line a b => .line (2 * a) (2 * b)
  | .tri a b c l => .tri (2 * a) (2 * b) (2 * c) (2 * l)

/-- D3DRASTER.STRIP, ADJ: primitive p's vertices -/
def primitive (t : Topo) (p : Nat) : Prim :=
  match t with
  | .points => .point p
  | .lines => .line (2 * p) (2 * p + 1)
  | .lineStrip => .line p (p + 1)
  | .linesAdj => .line (4 * p + 1) (4 * p + 2)
  | .lineStripAdj => .line (p + 1) (p + 2)
  | .tris => .tri (3 * p) (3 * p + 1) (3 * p + 2) (3 * p)
  | .trisAdj => .tri (6 * p) (6 * p + 2) (6 * p + 4) (6 * p)
  | .triStrip => if p % 2 = 1 then .tri (p + 1) p (p + 2) p else .tri p (p + 1) (p + 2) p
  | .triStripAdj => if p % 2 = 1 then .tri (2 * p + 2) (2 * p) (2 * p + 4) (2 * p) else .tri (2 * p) (2 * p + 2) (2 * p + 4) (2 * p)

/-- D3DRASTER.STRIP: a strip's triangle p is vertices p, p + 1, p + 2, every other one with its
first two swapped (so all wind the same way), the leading vertex p; each shares its last two
vertices' edge with the next -/
theorem strip (p : Nat) :
    primitive .triStrip (2 * p) = .tri (2 * p) (2 * p + 1) (2 * p + 2) (2 * p) ∧
    primitive .triStrip (2 * p + 1) = .tri (2 * p + 2) (2 * p + 1) (2 * p + 3) (2 * p + 1) := by
  constructor
  · have : 2 * p % 2 = 0 := by omega
    simp [primitive, this]
  · have : (2 * p + 1) % 2 = 1 := by omega
    simp only [primitive, this, ↓reduceIte]

/-- D3DRASTER.ADJ: drawn with no geometry program the adjacency topologies skip the adjacent
vertices: a line list's lines 4p + 1 to 4p + 2, a line strip's p + 1 to p + 2 (the strip without
its first vertex), a triangle list's and a triangle strip's triangles those of the plain topology
over the even vertices -/
theorem adjacency (p : Nat) :
    primitive .linesAdj p = .line (4 * p + 1) (4 * p + 2) ∧
    primitive .lineStripAdj p = primitive .lineStrip (p + 1) ∧
    primitive .trisAdj p = (primitive .tris p).double ∧
    primitive .triStripAdj p = (primitive .triStrip p).double := by
  refine ⟨rfl, rfl, ?_, ?_⟩
  · simp only [primitive, Prim.double, Prim.tri.injEq]; omega
  · by_cases h : p % 2 = 1
    · simp only [primitive, Prim.double, h, ↓reduceIte, Prim.tri.injEq, and_true, true_and]; omega
    · simp only [primitive, Prim.double, h, ↓reduceIte, Prim.tri.injEq, and_true, true_and]; omega

/-- the primitives a draw of count vertices makes, at most those that reach the nv vertices the
buffer holds -/
def primCount (t : Topo) (count nv : Nat) : Nat :=
  match t with
  | .points => min count nv
  | .lines => min (count / 2) ((nv + 1) / 2)
  | .lineStrip => min (count - 1) nv
  | .tris => min (count / 3) ((nv + 2) / 3)
  | .triStrip => min (count - 2) nv
  | .linesAdj => min (count / 4) ((nv + 3) / 4)
  | .lineStripAdj => min (count - 3) nv
  | .trisAdj => min (count / 6) ((nv + 5) / 6)
  | .triStripAdj => min (count / 2 - 2) ((nv + 1) / 2)

/-- the vertices primitive p reads -/
def Prim.verts : Prim → List Nat
  | .point v => [v]
  | .line a b => [a, b]
  | .tri a b c l => [a, b, c, l]

/-- Flat shading uses this vertex of the assembled primitive;
assembly rejects a topology's indices that do not fit uint. -/
def leadingVertex (t : Topo) (p : Nat) : Option Nat :=
  let prim := primitive t p
  if p < 2 ^ 32 ∧ prim.verts.all (fun k => decide (k < 2 ^ 32)) then some prim.leading else none

/-- The odd strip's vertex swap changes winding without changing the leading vertex. -/
theorem flat_strip_leading (p : Nat) (fits : p + 2 < 2 ^ 32) :
    leadingVertex .triStrip p = some p := by
  have h0 : p < 2 ^ 32 := by omega
  have h1 : p + 1 < 2 ^ 32 := by omega
  by_cases odd : p % 2 = 1 <;> simp [leadingVertex, primitive, Prim.verts, Prim.leading, odd, h0, h1, fits]

/-- The selected vertex is one of the primitive's input vertices for every supported topology. -/
theorem flat_leading_member (t : Topo) (p v : Nat) (h : leadingVertex t p = some v) :
    v ∈ (primitive t p).verts := by
  have hlead : (primitive t p).leading = v := by
    simp only [leadingVertex] at h
    split at h <;> simp_all
  rw [← hlead]
  cases t <;> (by_cases odd : p % 2 = 1) <;> simp_all [primitive, Prim.leading, Prim.verts]

/-- D3DRASTER.ADJ, STRIP: every primitive the draw makes (the buffer holding them all) reads
only the draw's vertices -/
theorem prims_inside (t : Topo) (count p : Nat) (hp : p < primCount t count count) :
    ∀ v ∈ (primitive t p).verts, v < count := by
  intro v hv
  cases t <;> simp only [primCount] at hp <;> simp only [primitive] at hv
  all_goals first
    | (simp only [Prim.verts, List.mem_cons, List.not_mem_nil, or_false] at hv; omega)
    | (split at hv <;> (simp only [Prim.verts, List.mem_cons, List.not_mem_nil, or_false] at hv; omega))

/-- D3DRASTER.VBPAST: word k of vertex v (stride bytes apart, the executor's 32-bit offset) of the
buffer's words, 0 past them -/
def vertexWord (vtx : List Nat) (v stride k : Nat) : Nat :=
  vtx.getD (((v * stride) % 2 ^ 32 / 4 + k) % 2 ^ 32) 0

/-- D3DRASTER.VBPAST: a vertex read past the vertex buffer reads zeros; inside it, the buffer's
word -/
theorem vb_past (vtx : List Nat) (v stride k : Nat) :
    let i := ((v * stride) % 2 ^ 32 / 4 + k) % 2 ^ 32
    (vtx.length ≤ i → vertexWord vtx v stride k = 0) ∧ (∀ h : i < vtx.length, vertexWord vtx v stride k = vtx[i]) := by
  intro i
  refine ⟨fun h => ?_, fun h => ?_⟩
  · simp [vertexWord, i, List.getD_eq_getElem?_getD, List.getElem?_eq_none h]
  · simp [vertexWord, i, List.getD_eq_getElem?_getD, List.getElem?_eq_getElem h]

/-- a view OMSetRenderTargets is given: its resource's sample count, its first subresource's
size (none: no resource) -/
structure Bound where
  samples : Nat
  size : Option (Nat × Nat)

/-- Direct3D 10, 10.1, 11 -/
inductive Api where
  | d10 | d101 | d11
  deriving DecidableEq

/-- what OMSetRenderTargets does: keep the views bound before, or bind the target given with the
depth view given or none -/
inductive Bind where
  | keep
  | set (depth : Bool)
  deriving DecidableEq, Repr

/-- the depth view's size narrower or shorter than the target's -/
def smaller (ds rt : Bound) : Bool :=
  match ds.size, rt.size with
  | some (dw, dh), some (w, h) => decide (dw < w) || decide (dh < h)
  | _, _ => false

/-- D3DRASTER.MS.DEPTHCOUNT, DEPTH.SMALL: OMSetRenderTargets with a target and a depth view (or
either alone) -/
def bind (api : Api) (rt ds : Option Bound) : Bind :=
  match rt, ds with
  | some a, some b =>
    if api = .d11 ∧ (a.samples ≠ b.samples ∨ smaller b a) then .keep
    else if api = .d10 ∧ a.samples ≠ b.samples then .set false
    else .set true
  | _, _ => .set ds.isSome

/-- D3DRASTER.MS.DEPTHCOUNT: a target and a depth view of other sample counts: Direct3D 11
changes nothing, Direct3D 10 binds the target without the depth view -/
theorem depth_count (a b : Bound) (h : a.samples ≠ b.samples) :
    bind .d11 (some a) (some b) = .keep ∧ bind .d10 (some a) (some b) = .set false := by
  simp [bind, h]

/-- D3DRASTER.DEPTH.SMALL: a depth view narrower or shorter than the target, of the same sample
count: Direct3D 11 changes nothing, Direct3D 10 binds both; views of one size bind as given -/
theorem depth_small (a b : Bound) (dw dh w h : Nat) (hs : a.samples = b.samples)
    (hd : b.size = some (dw, dh)) (ht : a.size = some (w, h)) :
    (dw < w ∨ dh < h → bind .d11 (some a) (some b) = .keep ∧ bind .d10 (some a) (some b) = .set true) ∧
    (w ≤ dw → h ≤ dh → ∀ api, bind api (some a) (some b) = .set true) := by
  refine ⟨fun hl => ?_, fun h1 h2 api => ?_⟩
  · have : smaller b a = true := by simp [smaller, hd, ht]; omega
    simp [bind, hs, this]
  · have : smaller b a = false := by simp [smaller, hd, ht]; omega
    cases api <;> simp [bind, hs, this]

/-- D3DRASTER.SHADER: program numbers follow Prog.ALL (two vertex programs, then eight
pixel programs). A known blob may bind only to its own stage. -/
def shaderPlan (vertex : Bool) (program : Nat) : Option (Bool × Nat) :=
  if program < 10 then some ((decide (program < 2)) == vertex, program) else none

theorem shader_generated (p : Nat) (known : p < 10) :
    shaderPlan (decide (p < 2)) p = some (true, p) ∧
    shaderPlan (!(decide (p < 2))) p = some (false, p) := by
  cases stage : decide (p < 2) <;> simp [shaderPlan, known, stage]

/-- D3DRASTER.DEPTH.ALONE: the depth view supplies the drawable extent when alone,
and cannot act as a color target. -/
inductive TargetPlan (α : Type) where
  | none
  | color (view : α)
  | depthOnly (view : α)
  deriving DecidableEq

def targetPlan {α : Type} (color depth : Option α) : TargetPlan α :=
  match color, depth with
  | some view, _ => .color view
  | none, some view => .depthOnly view
  | none, none => .none

theorem depth_alone {α : Type} (d c : α) :
    targetPlan none (some d) = .depthOnly d ∧
    targetPlan (some c) (some d) = .color c ∧
    targetPlan (none : Option α) none = .none := by
  exact ⟨rfl, rfl, rfl⟩

structure TargetRows where
  width : Nat
  height : Nat
  first : Nat
  slice : Nat
  deriving DecidableEq

/-- The extent and first element/slice of the view. Subresource selection precedes this
rule. In a 1D array the extent is one row of the selected subresource. -/
def targetRows (dim view w h d first slice : Nat) : Option TargetRows :=
  if dim = 2 then some ⟨w, h, 0, 0⟩
  else if dim = 1 ∧ (view = 2 ∨ view = 3) then some ⟨w, 1, 0, 0⟩
  else if dim = 4 ∧ view = 1 then some ⟨w, 1, first, 0⟩
  else if dim = 3 ∧ view = 8 ∧ slice < d then some ⟨w, h, 0, slice⟩
  else none

theorem target_1d (w h d first slice : Nat) :
    targetRows 1 2 w h d first slice = some ⟨w, 1, 0, 0⟩ ∧
    targetRows 1 3 w h d first slice = some ⟨w, 1, 0, 0⟩ ∧
    targetRows 4 1 w h d first slice = some ⟨w, 1, first, 0⟩ := by
  simp [targetRows]

theorem target_3d (w h d first slice : Nat) :
    (slice < d → targetRows 3 8 w h d first slice = some ⟨w, h, 0, slice⟩) ∧
    (d ≤ slice → targetRows 3 8 w h d first slice = none) := by
  constructor
  · intro valid
    simp [targetRows, valid]
  · intro outside
    simp [targetRows, Nat.not_lt.mpr outside]

def boundOf (s has w h : Int) : Option Bound := do
  let s ← nat s
  let w ← nat w
  let h ← nat h
  pure ⟨s, if has = 0 then none else some (w, h)⟩

private def uintArg (bits : Int) : Option Nat := do
  let n ← nat bits
  if n < 2 ^ 32 then some n else none

def mirrors : List Mirror := [
  ⟨"d3d11.verdict", fun
    | [p, v, vp, top] => do
      pure [match verdict (p != 0) (v != 0) (vp != 0) (← nat top) with
        | .incomplete => 0 | .noViewport => 1 | .crash => 2 | .removed => 3 | .nothing => 4
        | .draw t => 100 + t.code]
    | _ => none⟩,
  ⟨"d3d11.prim", fun
    | [top, p] => do
      let t ← Topo.of (← nat top)
      pure (match primitive t (← nat p) with
        | .point v => [v] | .line a b => [a, b] | .tri a b c l => [a, b, c, l])
    | _ => none⟩,
  ⟨"d3d11.leadingvertex", fun args => some ((do
    let [top, p] ← args.mapM uintArg | none
    let t ← Topo.of top
    let v ← leadingVertex t p
    pure [(v : Int)]).getD [-1])⟩,
  ⟨"d3d11.prims", fun
    | [top, count, nv] => do
      let t ← Topo.of (← nat top)
      pure [primCount t (← nat count) (← nat nv)]
    | _ => none⟩,
  ⟨"d3d11.drawids", fun args => some ((do
    let [instances, primitives, i, p] ← args.mapM uintArg | none
    let (j, q) ← drawIdAt instances primitives i p
    pure [(j : Int), (q : Int)]).getD [-1])⟩,
  ⟨"d3d11.vertexid", fun args => some ((do
    let [v, i] ← args.mapM uintArg | none
    pure ((vertexIdColor v i).map Int.ofNat)).getD [-1])⟩,
  ⟨"d3d11.primitiveid", fun
    | [p, front] => some (if front = 0 ∨ front = 1 then
        ((uintArg p).map (fun n => (primitiveIdColor n (front = 1)).map Int.ofNat)).getD [-1]
      else [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.word", fun
    | [n, v, stride, k] => do
      let vtx := (List.range (← nat n)).map (· + 1)
      pure [vertexWord vtx (← nat v) (← nat stride) (← nat k)]
    | _ => none⟩,
  ⟨"d3d11.shaderStage", fun
    | [s, p] => some (if s = 0 ∨ s = 1 then
        match (nat p).bind (shaderPlan (s = 1)) with
        | some (ok, program) => [if ok then 1 else 0, program]
        | none => [-1]
      else [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.targetPlan", fun
    | [rt, ds] =>
      let color := if rt = 1 then some (10 : Nat) else none
      let depth := if ds = 1 then some (20 : Nat) else none
      some (match targetPlan color depth with
        | .color v => [0, v] | .depthOnly v => [1, v] | .none => [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.targetRows", fun a => some ((do
    let [dim, view, w, h, d, first, slice] ← a.mapM nat | none
    let rows ← targetRows dim view w h d first slice
    pure [Int.ofNat rows.width, rows.height, rows.first, rows.slice]).getD [-1])⟩,
  ⟨"d3d11.bind", fun
    | [api, hr, rs, rh, rw, rhh, hd, ds, dh, dw, dhh] => do
      let a ← match api with | 0 => some Api.d10 | 1 => some .d101 | 2 => some .d11 | _ => none
      let rt ← if hr = 0 then some none else (boundOf rs rh rw rhh).map some
      let d ← if hd = 0 then some none else (boundOf ds dh dw dhh).map some
      pure (match bind a rt d with | .keep => [0] | .set false => [1] | .set true => [2])
    | _ => none⟩
]

end Kikizake.D3d11.Draw
