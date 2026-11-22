import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Line
/-! Direct3D 9's untransformed vertices (spec/d3d9draw.txt D3D9.DRAW.XYZ, DRAW.XYZ.CLIP) and the
perspective-correct attributes of pretransformed ones (DRAW.RHW), in binary32 (Kikizake.F32) as
src/model/d3d9draw.rs `transform_xyz`, `clip_xyz`, `xyz_vertex`, `rhw_attrs` and `rhw_value`
compute them (mirrors `d3d9.xyz`, `d3d9.clipxyz`, `d3d9.xyzvertex`, `d3d9.rhw`). -/
namespace Kikizake.D3d9.Xyz

open Kikizake.F32 (add sub mul div lt isNaN)
open Kikizake.D3d9.Clip (zero)
open Kikizake.D3d9.Line (one ofInt)

def two : Nat := 0x40000000

/-- the sum from 0 of f k for k = 0, 1, 2, 3 in turn, in f32 -/
def sum4 (f : Nat → Nat) : Nat := (List.range 4).foldl (fun s k => add s (f k)) zero

/-- the 4 x 4 product a b (row-major), each entry a sum from 0 -/
def mat (a b : List Nat) : List Nat :=
  (List.range 16).map fun i => sum4 fun k => mul (a.getD (i / 4 * 4 + k) 0) (b.getD (k * 4 + i % 4) 0)

/-- the row vector v times a -/
def xv (v a : List Nat) : List Nat := (List.range 4).map fun j => sum4 fun k => mul (v.getD k 0) (a.getD (k * 4 + j) 0)

/-- D3D9.DRAW.XYZ: the position times WORLD VIEW (first) then PROJECTION, divided by w, through the
viewport: (x, y, z, rhw, the clip-space position) -/
def transform (p : List Nat) (world view proj : List Nat) (vx vy vw vh : Int) (z0 z1 : Nat) : Nat × Nat × Nat × Nat × List Nat :=
  let c := xv [p.getD 0 0, p.getD 1 0, p.getD 2 0, one] (mat (mat world view) proj)
  let rhw := div one (c.getD 3 0)
  let x := mul (c.getD 0 0) rhw
  let y := mul (c.getD 1 0) rhw
  let z := mul (c.getD 2 0) rhw
  let hw := div (ofInt vw) two
  let hh := div (ofInt vh) two
  (add (ofInt vx) (mul (add one x) hw), add (ofInt vy) (mul (sub one y) hh), add z0 (mul z (sub z1 z0)), rhw, c)

/-- D3D9.DRAW.XYZ: the products are WORLD VIEW first, then PROJECTION, the position a row vector -/
theorem order (p world view proj : List Nat) (vx vy vw vh : Int) (z0 z1 : Nat) :
    (transform p world view proj vx vy vw vh z0 z1).2.2.2.2 = xv [p.getD 0 0, p.getD 1 0, p.getD 2 0, one] (mat (mat world view) proj) := rfl

/-- D3D9.DRAW.XYZ: x and y through the viewport: vx + (1 + x/w) vw/2, vy + (1 - y/w) vh/2 -/
theorem viewport (p world view proj : List Nat) (vx vy vw vh : Int) (z0 z1 : Nat) :
    let r := transform p world view proj vx vy vw vh z0 z1
    r.1 = add (ofInt vx) (mul (add one (mul (r.2.2.2.2.getD 0 0) r.2.2.2.1)) (div (ofInt vw) two)) ∧
    r.2.1 = add (ofInt vy) (mul (sub one (mul (r.2.2.2.2.getD 1 0) r.2.2.2.1)) (div (ofInt vh) two)) ∧
    r.2.2.2.1 = div one (r.2.2.2.2.getD 3 0) := by
  simp [transform]

/-- a clip-space vertex: its position (4) and attributes -/
structure CV where
  q : List Nat
  at' : List Nat

/-- d < 0 (false for NaN) -/
def neg0 (d : Nat) : Bool := !isNaN d && lt d zero
/-- d >= 0 (false for NaN) -/
def ge0 (d : Nat) : Bool := !isNaN d && !lt d zero

/-- the distance to a plane: w + x, w - x, w + y, w - y -/
def dist (q : List Nat) (plane : Nat) : Nat :=
  let w := q.getD 3 0
  match plane with
  | 0 => add w (q.getD 0 0)
  | 1 => sub w (q.getD 0 0)
  | 2 => add w (q.getD 1 0)
  | _ => sub w (q.getD 1 0)

def lerpL (t : Nat) (a b : List Nat) : List Nat := (a.zip b).map fun (x, y) => add x (mul (sub y x) t)

/-- one plane (Sutherland-Hodgman): each edge's first vertex kept when inside, and the crossing
a + (b - a) t, t = da / (da - db) -/
def pass (plane : Nat) (poly : List CV) : List CV :=
  (poly.zip (poly.drop 1 ++ poly.take 1)).flatMap fun (p, q) =>
    let dp := dist p.q plane
    let dq := dist q.q plane
    (if ge0 dp then [p] else []) ++
      (if ge0 dp != ge0 dq then
        let t := div dp (sub dp dq)
        [⟨lerpL t p.q q.q, lerpL t p.at' q.at'⟩] else [])

/-- the positions shifted half a pixel in: (x + w / vw, y - w / vh) -/
def shift (vw vh : Int) (v : CV) : CV :=
  let w := v.q.getD 3 0
  ⟨[add (v.q.getD 0 0) (div w (ofInt vw)), sub (v.q.getD 1 0) (div w (ofInt vh)), v.q.getD 2 0, w], v.at'⟩

/-- D3D9.DRAW.XYZ.CLIP: the triangle clipped in clip space to the planes half a pixel in from the
viewport's edges, in turn; none when no vertex is outside -/
def clipXyz (poly : List CV) (vw vh : Int) : Option (List CV) :=
  let p := poly.map (shift vw vh)
  if !p.any (fun v => (List.range 4).any fun pl => neg0 (dist v.q pl)) then none
  else some ([0, 1, 2, 3].foldl (fun acc pl => pass pl acc) p)

/-- D3D9.DRAW.XYZ.CLIP: a triangle inside every plane is not clipped -/
theorem inside_none (poly : List CV) (vw vh : Int)
    (h : ∀ v ∈ poly, ∀ pl < 4, neg0 (dist (shift vw vh v).q pl) = false) : clipXyz poly vw vh = none := by
  simp only [clipXyz]
  have : (poly.map (shift vw vh)).any (fun v => (List.range 4).any fun pl => neg0 (dist v.q pl)) = false := by
    simp only [List.any_eq_false, List.mem_map]
    rintro _ ⟨v, hv, rfl⟩ hx
    simp only [List.any_eq_true, List.mem_range] at hx
    obtain ⟨pl, hpl, hn⟩ := hx
    rw [h v hv pl hpl] at hn
    exact Bool.false_ne_true hn
  simp [this]

/-- D3D9.DRAW.XYZ.CLIP: a plane every vertex is inside of keeps the polygon -/
theorem pass_keeps (plane : Nat) (poly : List CV) (h : ∀ v ∈ poly, ge0 (dist v.q plane) = true) :
    pass plane poly = poly := by
  unfold pass
  have hq : ∀ v ∈ poly.drop 1 ++ poly.take 1, ge0 (dist v.q plane) = true := by
    intro v hv
    simp only [List.mem_append] at hv
    rcases hv with hv | hv
    · exact h v (List.mem_of_mem_drop hv)
    · exact h v (List.mem_of_mem_take hv)
  have hl : poly.length = (poly.drop 1 ++ poly.take 1).length := by simp; omega
  generalize poly.drop 1 ++ poly.take 1 = nx at hq hl
  induction poly generalizing nx with
  | nil => simp
  | cons a r ih =>
    cases nx with
    | nil => simp at hl
    | cons b nr =>
      have ha := h a (by simp)
      have hb := hq b (by simp)
      simp only [List.zip_cons_cons, List.flatMap_cons, ha, hb, ↓reduceIte, bne_self_eq_false, Bool.false_eq_true,
        List.append_nil, List.singleton_append]
      rw [ih (fun v hv => h v (by simp [hv])) nr (fun v hv => hq v (by simp [hv])) (by simpa using hl)]

/-- D3D9.DRAW.XYZ.CLIP: a clipped vertex on the screen: x, y half a pixel up and left of the
viewport's, rhw, its position unshifted -/
def vertex (q : List Nat) (vx vy vw vh : Int) : Nat × Nat × Nat × List Nat :=
  let w := q.getD 3 0
  let rhw := div one w
  (add (sub (ofInt vx) Clip.half) (mul (add one (mul (q.getD 0 0) rhw)) (div (ofInt vw) two)),
   add (sub (ofInt vy) Clip.half) (mul (sub one (mul (q.getD 1 0) rhw)) (div (ofInt vh) two)),
   rhw, [sub (q.getD 0 0) (div w (ofInt vw)), add (q.getD 1 0) (div w (ofInt vh)), q.getD 2 0, w])

/-- the attributes interpolated perspective-correct: all but z (0) and rhw (11) -/
def persp (k : Nat) : Bool := k != 0 && k != 11

/-- D3D9.DRAW.RHW: the attributes times rhw, and the interpolated ones divided by the interpolated
rhw -/
def rhwAttrs (at' : List Nat) : List Nat := (List.range at'.length).map fun k =>
  if persp k then mul (at'.getD k 0) (at'.getD 11 0) else at'.getD k 0

def rhwValue (at' : List Nat) : List Nat := (List.range at'.length).map fun k =>
  if persp k then div (at'.getD k 0) (at'.getD 11 0) else at'.getD k 0

/-- D3D9.DRAW.RHW: z and rhw itself are not weighted -/
theorem rhw_keeps (at' : List Nat) (k : Nat) (hk : k < at'.length) (h : k = 0 ∨ k = 11) :
    (rhwAttrs at')[k]? = some (at'.getD k 0) ∧ (rhwValue at')[k]? = some (at'.getD k 0) := by
  have : persp k = false := by rcases h with rfl | rfl <;> rfl
  simp [rhwAttrs, rhwValue, hk, this]

/-- D3D9.DRAW.RHW: every other attribute times rhw, then over the interpolated rhw -/
theorem rhw_weights (at' : List Nat) (k : Nat) (hk : k < at'.length) (h0 : k ≠ 0) (h11 : k ≠ 11) :
    (rhwAttrs at')[k]? = some (mul (at'.getD k 0) (at'.getD 11 0)) ∧
    (rhwValue at')[k]? = some (div (at'.getD k 0) (at'.getD 11 0)) := by
  have : persp k = true := by simp [persp, h0, h11]
  simp [rhwAttrs, rhwValue, hk, this]

private def take4 : List Int → Option (List Nat × List Int)
  | a :: b :: c :: d :: r => do pure ([← nat a, ← nat b, ← nat c, ← nat d], r)
  | _ => none

def mirrors : List Mirror := [
  ⟨"d3d9.xyz", fun
    | px :: py :: pz :: vx :: vy :: vw :: vh :: z0 :: z1 :: ms => do
      if ms.length != 48 then none
      let m ← ms.mapM nat
      let r := transform [← nat px, ← nat py, ← nat pz] (m.take 16) ((m.drop 16).take 16) (m.drop 32) vx vy vw vh (← nat z0) (← nat z1)
      pure (([r.1, r.2.1, r.2.2.1, r.2.2.2.1] ++ r.2.2.2.2).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.clipxyz", fun
    | vw :: vh :: n :: rest => do
      let k ← nat n
      let ws ← rest.mapM nat
      if ws.length != 3 * (4 + k) then none
      let vs := [0, 1, 2].map fun i => let v := (ws.drop (i * (4 + k))).take (4 + k); (⟨v.take 4, v.drop 4⟩ : CV)
      match clipXyz vs vw vh with
      | none => pure [-1]
      | some out => pure (out.flatMap fun v => (v.q ++ v.at').map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.xyzvertex", fun
    | vx :: vy :: vw :: vh :: q => do
      let (qs, r) ← take4 q
      if !r.isEmpty then none
      let (x, y, rhw, c) := vertex qs vx vy vw vh
      pure (([x, y, rhw] ++ c).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.rhw", fun
    | ws => do
      let at' ← ws.mapM nat
      if at'.length != 19 then none
      pure ((rhwAttrs at' ++ rhwValue at').map fun b => (F32.canon b : Int))⟩
]

end Kikizake.D3d9.Xyz
