import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
import Kikizake.D3d11.Interp
/-! Direct3D 10 and 11's clipping (spec/d3dres.txt D3DRASTER.CLIP, CLIPW.AWAY, CLIPW.NAN,
CLIPZ.PIECES, CLIPZ.LINE, DEPTHCLIP, LINE's clipping) in binary32 and binary64 as
src/model/d3draster.rs computes it: `clipTri` is `clip_tri_values` (`clip_tri`), `shF32` its
`clip_xy` and `clip_z`, `clipLine` its `clip_line_values`, `clippedAway` its `clipped_away`,
`notFinite` its `not_finite`, `depthAway` its `depth_away` (mirrors `d3d11.cliptri`, `d3d11.clipxy`, `d3d11.clipline`,
`d3d11.clippedaway`, `d3d11.notfinite`, `d3d11.depthaway`; a NaN answer compared as the one quiet NaN). A position
is [x, y, z, w]; the clip planes are 0 near (z), 1 far (w - z), 2 left (w + x), 3 right
(w - x), 4 top (w - y), 5 bottom (w + y), inside when the distance is not below 0. -/
namespace Kikizake.D3d11.Clip

open Kikizake.Float (isNaN isInf)
open Kikizake.F32 (add sub mul div)
open Kikizake.D3d11.Filter (ltF eqF one)
open Kikizake.D3d11.Interp (clipLerp viewportXY)

/-- the distance of a position from a clip plane -/
def planeDist (q : List Nat) : Nat → Nat
  | 0 => q.getD 2 0
  | 1 => sub (q.getD 3 0) (q.getD 2 0)
  | 2 => add (q.getD 3 0) (q.getD 0 0)
  | 3 => sub (q.getD 3 0) (q.getD 0 0)
  | 4 => sub (q.getD 3 0) (q.getD 1 0)
  | _ => add (q.getD 3 0) (q.getD 1 0)

/-- the planes clipped against: near and far only with depth clipping -/
def planesOf (zclip : Bool) : List Nat := if zclip then [0, 1, 2, 3, 4, 5] else [2, 3, 4, 5]

/-- a polygon's vertex: position, color, its distances from the six planes, made by the
clipping, the edge from it on along a plane -/
structure CV where
  pos : List Nat
  color : List Nat
  dist : List Nat
  made : Bool
  along : Bool

def CV.of (pos color : List Nat) : CV := ⟨pos, color, (List.range 6).map (planeDist pos), false, false⟩

/-- outside a plane: the distance below 0 -/
def out (v : CV) (p : Nat) : Bool := ltF (v.dist.getD p 0) 0

def lerpL (din dout : Nat) (vi vo : List Nat) : List Nat := List.zipWith (clipLerp din dout) vi vo

/-- one plane's Sutherland-Hodgman step: each edge a to b keeps a inside, and where it crosses
makes a vertex from the inside end (the later planes' distances carried) -/
def step (later : List Nat) (p : Nat) (poly : List CV) : List CV :=
  let n := poly.length
  let dflt : CV := ⟨[], [], [], false, false⟩
  (List.range n).flatMap fun i =>
    let a := poly.getD i dflt
    let b := poly.getD ((i + 1) % n) dflt
    let ina := !out a p
    let inb := !out b p
    let keep := if ina then [a] else []
    if ina != inb then
      let (vi, vo) := if ina then (a, b) else (b, a)
      let din := vi.dist.getD p 0
      let dout := vo.dist.getD p 0
      let d := (List.range 6).map fun q =>
        if q ∈ later then clipLerp din dout (vi.dist.getD q 0) (vo.dist.getD q 0) else vi.dist.getD q 0
      keep ++ [⟨lerpL din dout vi.pos vo.pos, lerpL din dout vi.color vo.color, d, true, ina || a.along⟩]
    else keep

/-- the cut planes in turn: none once a step leaves nothing -/
def steps : List Nat → List CV → Option (List CV)
  | [], poly => some poly
  | p :: rest, poly =>
    let next := step rest p poly
    if next.isEmpty then none else steps rest next

/-- D3DRASTER.CLIP: a triangle turned so the least clip x (then y) comes first, clipped against
the planes some vertex is outside of, in order; none when every vertex is outside one plane
(`turn` the turning) -/
def turn (v0 v1 v2 : CV) : List CV :=
  let less := fun (a b : CV) => ltF (b.pos.getD 0 0) (a.pos.getD 0 0) ||
    eqF (a.pos.getD 0 0) (b.pos.getD 0 0) && ltF (b.pos.getD 1 0) (a.pos.getD 1 0)
  let tri := [v0, v1, v2]
  let s0 := if less v0 v1 then 1 else 0
  let sel := if less (tri.getD s0 v0) v2 then 2 else s0
  (List.range 3).map fun i => tri.getD ((sel + i) % 3) v0

def clipTri (v0 v1 v2 : CV) (zclip : Bool) : Option (List CV) :=
  let poly := turn v0 v1 v2
  let planes := planesOf zclip
  if planes.any (fun p => poly.all (out · p)) then none
  else steps (planes.filter fun p => poly.any (out · p)) poly

/-- inside a plane for the f32 clippings: the distance not below 0, a NaN outside -/
def ge0 (d : Nat) : Bool := !isNaN F32.fmt d && !ltF d 0

/-- D3DRASTER.CLIPW.AWAY: Sutherland-Hodgman in f32 against the given distances in turn: a
new vertex a + (b - a) t, t = da / (da - db) -/
def shF32 (ds : List (List Nat → Nat)) (poly : List (List Nat)) : List (List Nat) :=
  ds.foldl (fun poly d =>
    let inside := fun q => ge0 (d q)
    if poly.all inside then poly
    else
      let n := poly.length
      (List.range n).flatMap fun i =>
        let a := poly.getD i []
        let b := poly.getD ((i + 1) % n) []
        let keep := if inside a then [a] else []
        if inside a != inside b then
          let t := div (d a) (sub (d a) (d b))
          keep ++ [List.zipWith (fun x y => add x (mul (sub y x) t)) a b]
        else keep) poly

def xyPlanes : List (List Nat → Nat) := [(planeDist · 2), (planeDist · 3), (planeDist · 5), (planeDist · 4)]
def zPlanes : List (List Nat → Nat) := [(planeDist · 0), (planeDist · 1)]

/-- D3DRASTER.LINE, CLIPZ.LINE: a line's ends (positions and colors as one list of eight)
clipped against the distances in turn: the end outside a plane moved to a + (b - a) t,
t = da / (da - db) in f32; none when both are outside one -/
def clipLine (ds : List (List Nat → Nat)) (a b : List Nat) : Option (List Nat × List Nat) :=
  ds.foldlM (fun (a, b) d =>
    let da := d a
    let db := d b
    let ina := ge0 da
    let inb := ge0 db
    if !ina && !inb then none
    else if ina && inb then some (a, b)
    else
      let t := div da (sub da db)
      let v := List.zipWith (fun x y => add x (mul (sub y x) t)) a b
      some (if ina then (a, v) else (v, b))) (a, b)

/-- the doubled area of three snapped points -/
def area (a b c : Int × Int) : Int := (b.1 - a.1) * (c.2 - a.2) - (b.2 - a.2) * (c.1 - a.1)

/-- D3DRASTER.CLIPW.AWAY: what the clipping leaves (every w positive) has no area once snapped -/
def clippedAway (vx vy vw vh : Nat) (poly : List (List Nat)) : Bool :=
  if !poly.all (fun q => ltF 0 (q.getD 3 0)) then false
  else
    let q := poly.map fun q => viewportXY vx vy vw vh (q.getD 0 0) (q.getD 1 0) (q.getD 3 0)
    match q with
    | [] => true
    | first :: rest => (List.range (rest.length - 1)).all fun i => area first (rest.getD i (0, 0)) (rest.getD (i + 1) (0, 0)) == 0

/-- D3DRASTER.DEPTHCLIP: depths (z / w) wholly in front of 0 or wholly past 1 -/
def depthAway (zs : List Nat) : Bool := zs.all (ltF · 0) || zs.all (ltF one ·)

/-- D3DRASTER.CLIPW.NAN: a coordinate NaN or infinite -/
def notFinite (q : List Nat) : Bool := q.any fun x => isNaN F32.fmt x || isInf F32.fmt x

/-! ## theorems -/

/-- D3DRASTER.CLIPZ.PIECES: with depth clipping a triangle is clipped to 0 <= z and z <= w
first, then to the x and y planes -/
theorem depth_first : planesOf true = [0, 1] ++ planesOf false := rfl

private theorem getD_mem {α} (l : List α) (n : Nat) {d : α} (hd : d ∈ l) : l.getD n d ∈ l := by
  rw [List.getD_eq_getElem?_getD]
  cases h : l[n]? with
  | none => simpa using hd
  | some x => simpa using List.mem_of_getElem? h

private theorem turn_mem {v0 v1 v2 v : CV} (h : v ∈ turn v0 v1 v2) : v ∈ [v0, v1, v2] := by
  simp only [turn, List.mem_map, List.mem_range] at h
  obtain ⟨i, -, rfl⟩ := h
  exact getD_mem _ _ (by simp)

private theorem turn_length (v0 v1 v2 : CV) : (turn v0 v1 v2).length = 3 := by simp [turn]

/-- D3DRASTER.CLIP, DEPTHCLIP: a triangle every vertex of which is outside one plane is
clipped away -/
theorem all_out (v0 v1 v2 : CV) (zclip : Bool) (p : Nat) (hp : p ∈ planesOf zclip)
    (h : ∀ v ∈ [v0, v1, v2], out v p = true) : clipTri v0 v1 v2 zclip = none := by
  have : (planesOf zclip).any (fun p => (turn v0 v1 v2).all (out · p)) = true :=
    List.any_eq_true.2 ⟨p, hp, List.all_eq_true.2 fun v hv => h v (turn_mem hv)⟩
  simp [clipTri, this]

/-- D3DRASTER.DEPTHCLIP: with depth clipping a triangle wholly in front of 0 is clipped away -/
theorem in_front (v0 v1 v2 : CV) (h0 : out v0 0 = true) (h1 : out v1 0 = true) (h2 : out v2 0 = true) :
    clipTri v0 v1 v2 true = none :=
  all_out v0 v1 v2 true 0 (by simp [planesOf]) (by simp [h0, h1, h2])

/-- D3DRASTER.DEPTHCLIP: with depth clipping a triangle wholly past w is clipped away -/
theorem past_far (v0 v1 v2 : CV) (h0 : out v0 1 = true) (h1 : out v1 1 = true) (h2 : out v2 1 = true) :
    clipTri v0 v1 v2 true = none :=
  all_out v0 v1 v2 true 1 (by simp [planesOf]) (by simp [h0, h1, h2])

/-- D3DRASTER.CLIP: a triangle no vertex of which is outside a plane comes back whole -/
theorem inside_whole (v0 v1 v2 : CV) (zclip : Bool) (h : ∀ v ∈ [v0, v1, v2], ∀ p ∈ planesOf zclip, out v p = false) :
    (clipTri v0 v1 v2 zclip).map List.length = some 3 := by
  have hv : (turn v0 v1 v2)[0]'(by simp [turn_length]) ∈ turn v0 v1 v2 := List.getElem_mem _
  have hany : (planesOf zclip).any (fun p => (turn v0 v1 v2).all (out · p)) = false := by
    rw [List.any_eq_false]
    intro p hp hall
    have := List.all_eq_true.1 hall _ hv
    simp [h _ (turn_mem hv) p hp] at this
  have hfil : (planesOf zclip).filter (fun p => (turn v0 v1 v2).any (out · p)) = [] := by
    rw [List.filter_eq_nil_iff]
    intro p hp
    simp only [List.any_eq_true, not_exists, not_and]
    intro v hv'
    simp [h v (turn_mem hv') p hp]
  simp [clipTri, hany, hfil, steps, turn_length]

/-- D3DRASTER.DEPTHCLIP: depths all below 0 are clipped away -/
theorem depth_front (zs : List Nat) (h : ∀ z ∈ zs, ltF z 0 = true) : depthAway zs = true := by
  simp [depthAway, List.all_eq_true.2 h]

/-- D3DRASTER.LINE, CLIPZ.LINE: a line both ends of which are outside one plane draws nothing -/
theorem line_out (d : List Nat → Nat) (rest : List (List Nat → Nat)) (a b : List Nat)
    (ha : ge0 (d a) = false) (hb : ge0 (d b) = false) : clipLine (d :: rest) a b = none := by
  simp [clipLine, List.foldlM, ha, hb]

/-- D3DRASTER.CLIPW.AWAY: what is left in front of the eye at one point has no area -/
theorem point_away (vx vy vw vh : Nat) (q : List Nat) (n : Nat) (hw : ltF 0 (q.getD 3 0) = true) :
    clippedAway vx vy vw vh (List.replicate n q) = true := by
  cases n with
  | zero => simp [clippedAway]
  | succ n =>
    have hall : (List.replicate (n + 1) q).all (fun q => ltF 0 (q.getD 3 0)) = true := by
      simpa [List.getD_eq_getElem?_getD] using hw
    rw [clippedAway, hall, List.map_replicate, List.replicate_succ]
    simp only [Bool.not_true, Bool.false_eq_true, ite_false, List.all_eq_true, List.mem_range, List.length_replicate]
    intro x hx
    simp [List.getD_eq_getElem?_getD, show x < n by omega, show x + 1 < n by omega, area]

/-- D3DRASTER.CLIPW.NAN: a NaN anywhere is not finite -/
theorem nan_not_finite (q : List Nat) (x : Nat) (hx : x ∈ q) (h : isNaN F32.fmt x = true) : notFinite q = true := by
  simp only [notFinite, List.any_eq_true]
  exact ⟨x, hx, by simp [h]⟩

/-- Keep each cyclic edge whose start is unmarked, if any piece survives culling. -/
def clippedWireEdges (visible : Bool) (along : List Bool) : List (Nat × Nat) :=
  if visible then
    ((List.range along.length).filter fun i => !(along.getD i true)).map
      fun i => (i, (i + 1) % along.length)
  else []

theorem culled_wire_has_no_edges (along : List Bool) :
    clippedWireEdges false along = [] := by
  rfl

theorem only_unmarked_edges_survive (along : List Bool) (a b : Nat) :
    (a, b) ∈ clippedWireEdges true along ↔
      a < along.length ∧ along.getD a true = false ∧ b = (a + 1) % along.length := by
  simp only [clippedWireEdges, ↓reduceIte, List.mem_map, List.mem_filter, List.mem_range,
    Bool.not_eq_true', Prod.mk.injEq]
  constructor
  · rintro ⟨i, ⟨hi, hm⟩, ha, hb⟩
    subst a
    exact ⟨hi, hm, hb.symm⟩
  · rintro ⟨ha, hm, rfl⟩
    exact ⟨a, ⟨ha, hm⟩, rfl, rfl⟩

/-- the mirrors' answers: a NaN as the one quiet NaN -/
def canon (x : Nat) : Int := (F32.canon x : Nat)

def cvOut (v : CV) : List Int := v.pos.map canon ++ v.color.map canon ++ [if v.made then 1 else 0, if v.along then 1 else 0]

def mirrors : List Mirror := [
  ⟨"d3d11.clippedwireedges", fun
    | visible :: along => some ((clippedWireEdges (visible != 0) (along.map (· != 0))).flatMap
        fun (a, b) => [Int.ofNat a, Int.ofNat b])
    | _ => none⟩,
  ⟨"d3d11.cliptri", fun a => do
    if a.length != 25 then none
    let xs ← a.mapM nat
    let v := fun k => CV.of ((xs.drop (8 * k)).take 4) ((xs.drop (8 * k + 4)).take 4)
    match clipTri (v 0) (v 1) (v 2) (xs.getD 24 0 != 0) with
    | none => pure [0]
    | some p => pure ((1 : Int) :: (p.length : Int) :: p.flatMap cvOut)⟩,
  ⟨"d3d11.clipxy", fun a => do
    if a.length != 13 then none
    let xs ← a.mapM nat
    let poly := (List.range 3).map fun k => (xs.drop (4 * k)).take 4
    let p := shF32 xyPlanes poly
    let p := if xs.getD 12 0 != 0 then shF32 zPlanes p else p
    pure ((p.length : Int) :: p.flatMap (·.map canon))⟩,
  ⟨"d3d11.clipline", fun a => do
    if a.length != 17 then none
    let xs ← a.mapM nat
    let ds := if xs.getD 16 0 != 0 then zPlanes else xyPlanes
    match clipLine ds (xs.take 8) ((xs.drop 8).take 8) with
    | none => pure [0]
    | some (p, q) => pure ((1 : Int) :: (p ++ q).map canon)⟩,
  ⟨"d3d11.clippedaway", fun a => do
    if a.length != 16 then none
    let xs ← a.mapM nat
    let poly := (List.range 3).map fun k => (xs.drop (4 + 4 * k)).take 4
    pure [if clippedAway (xs.getD 0 0) (xs.getD 1 0) (xs.getD 2 0) (xs.getD 3 0) (shF32 xyPlanes poly) then 1 else 0]⟩,
  ⟨"d3d11.notfinite", fun a => do
    if a.length != 12 then none
    let xs ← a.mapM nat
    pure [if notFinite xs then 1 else 0]⟩,
  ⟨"d3d11.depthaway", fun a => do
    let xs ← a.mapM nat
    pure [if depthAway xs then 1 else 0]⟩
]

end Kikizake.D3d11.Clip
