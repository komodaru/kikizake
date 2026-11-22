import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Shade
/-! Direct3D 9's clipping of pretransformed triangles (spec/d3d9draw.txt D3D9.RASTER.CLIP,
RASTER.CLIP8, RASTER.CLIP8SIDE) in binary32 (Kikizake.F32), as src/model/d3d9draw.rs
`clip_polygon`, `snap256`, `least_x` and `clip_bounds` compute it (mirrors `d3d9.clip`, `d3d9.snap`,
`d3d9.leastx`, `d3d9.clipbounds`); the strip the polygon is then cut into is Shade.strip. -/
namespace Kikizake.D3d9.Clip

open Kikizake.F32 (add sub mul div lt clamp)

def half : Nat := 0x3f000000
def mhalf : Nat := 0xbf000000
def zero : Nat := 0

/-- a vertex: x, y and its attributes (bits) -/
structure V where
  x : Nat
  y : Nat
  attrs : List Nat

/-- Rust's `d >= 0.0` on the finite values clipping makes -/
def ge0 (d : Nat) : Bool := !lt d zero

/-- a side's distance: left x + 0.5, right x1 - x, top y + 0.5, bottom y1 - y -/
def dist (s : Fin 4) (x1 y1 : Nat) (v : V) : Nat :=
  match s with
  | 0 => add v.x half
  | 1 => sub x1 v.x
  | 2 => add v.y half
  | 3 => sub y1 v.y

/-- p + t (q - p) for the position and each attribute -/
def lerp (t : Nat) (p q : V) : V :=
  ⟨add p.x (mul t (sub q.x p.x)), add p.y (mul t (sub q.y p.y)), (p.attrs.zip q.attrs).map fun (a, b) => add a (mul t (sub b a))⟩

/-- the polygon rotated by one: each vertex's next -/
def next (l : List (V × Bool)) : List (V × Bool) := l.drop 1 ++ l.take 1

/-- one side (Sutherland-Hodgman): each edge from its first vertex kept when inside, and the
crossing p + t (q - p), t = dp / (dp - dq), marked new -/
def pass (s : Fin 4) (x1 y1 : Nat) (poly : List (V × Bool)) : List (V × Bool) :=
  (poly.zip (next poly)).flatMap fun ((p, pn), (q, _)) =>
    let dp := dist s x1 y1 p
    let dq := dist s x1 y1 q
    (if ge0 dp then [(p, pn)] else []) ++
      (if ge0 dp != ge0 dq then [(lerp (div dp (sub dp dq)) p q, true)] else [])

/-- f32's ceil of a finite value -/
def ceil (x : Nat) : Nat :=
  let (s, n, d) := Float.toRat F32.fmt x
  if s then (if n / d = 0 then Float.sbit F32.fmt else Float.neg F32.fmt (F32.ofNat (n / d)))
  else F32.ofNat ((n + d - 1) / d)

/-- Rust's `a.min(b)` of two finite values -/
def min (a b : Nat) : Nat := if lt b a then b else a

/-- D3D9.RASTER.CLIP, CLIP8: the polygon clipped to -0.5 ..= x1, -0.5 ..= y1 on the sides
(left, right, top, bottom) some vertex is outside of, in that order; the new vertices clamped
into the viewport (to x1 on a side clipped, else to the last pixel center below it) -/
def clipPolygon (poly : List V) (x1 y1 : Nat) (sides : Fin 4 → Bool) : List V :=
  let p0 := poly.map fun v => (v, false)
  let p := [0, 1, 2, 3].foldl (fun acc (s : Fin 4) => if sides s then pass s x1 y1 acc else acc) p0
  let cx := if sides 1 then x1 else min x1 (sub (ceil x1) half)
  let cy := if sides 3 then y1 else min y1 (sub (ceil y1) half)
  p.map fun (v, new) => if new then { v with x := clamp v.x mhalf cx, y := clamp v.y mhalf cy } else v

/-- D3D9.RASTER.CLIP: a triangle on no side's wrong side is drawn as it is -/
theorem no_sides (poly : List V) (x1 y1 : Nat) : clipPolygon poly x1 y1 (fun _ => false) = poly := by
  simp only [clipPolygon, List.foldl, Bool.false_eq_true, ↓reduceIte, List.map_map]
  conv => rhs; rw [← List.map_id poly]
  apply List.map_congr_left
  intro v _
  simp

private theorem pass_inside (s : Fin 4) (x1 y1 : Nat) : ∀ (poly : List (V × Bool)) (nx : List (V × Bool)),
    poly.length = nx.length → (∀ v ∈ poly, ge0 (dist s x1 y1 v.1) = true) → (∀ v ∈ nx, ge0 (dist s x1 y1 v.1) = true) →
    (poly.zip nx).flatMap (fun ((p, pn), (q, _)) =>
      (if ge0 (dist s x1 y1 p) then [(p, pn)] else []) ++
        (if ge0 (dist s x1 y1 p) != ge0 (dist s x1 y1 q) then [(lerp (div (dist s x1 y1 p) (sub (dist s x1 y1 p) (dist s x1 y1 q))) p q, true)] else [])) = poly := by
  intro poly
  induction poly with
  | nil => intro nx _ _ _; simp
  | cons a r ih =>
    intro nx hl hp hn
    cases nx with
    | nil => simp at hl
    | cons b nr =>
      have ha := hp a (by simp)
      have hb := hn b (by simp)
      simp only [List.zip_cons_cons, List.flatMap_cons, ha, hb, ↓reduceIte, bne_self_eq_false, Bool.false_eq_true,
        List.append_nil, List.singleton_append]
      rw [ih nr (by simpa using hl) (fun v h => hp v (by simp [h])) (fun v h => hn v (by simp [h]))]

/-- D3D9.RASTER.CLIP: clipping to a side every vertex is inside of keeps the polygon as it is -/
theorem pass_keeps (s : Fin 4) (x1 y1 : Nat) (poly : List (V × Bool)) (h : ∀ v ∈ poly, ge0 (dist s x1 y1 v.1) = true) :
    pass s x1 y1 poly = poly := by
  unfold pass
  apply pass_inside s x1 y1 poly (next poly)
  · simp [next]; omega
  · exact h
  · intro v hv; apply h; simp only [next, List.mem_append] at hv
    rcases hv with hv | hv
    · exact List.mem_of_mem_drop hv
    · exact List.mem_of_mem_take hv

/-- the triangle's vertex of least x, then least y, the first of equals (Rust's `min_by` on
`partial_cmp`) -/
def leastX (v : List (Nat × Nat)) : Nat :=
  let gt (a b : Nat × Nat) : Bool := lt b.1 a.1 || (!lt a.1 b.1 && !lt b.1 a.1 && lt b.2 a.2)
  (List.range v.length).foldl (fun k i => match v[k]?, v[i]? with
    | some a, some b => if gt a b then i else k
    | _, _ => k) 0

/-- D3D9.RASTER.CLIP8SIDE, CLIP: where the triangle is clipped: (right side x1, bottom side y1,
the sides some vertex is outside of: x < 0, x > w, y < 0, y > h, strictly). Direct3D 8 with a
vertex at the width (height) itself clips that side at the size, not half a pixel in, and so does
a triangle of RHW (persp) -/
def bounds (pts : List (Nat × Nat)) (w h : Nat) (v8 persp : Bool) : Nat × Nat × List Bool :=
  let W := F32.ofNat w
  let H := F32.ofNat h
  let atW := v8 && pts.any (fun p => !lt p.1 W && !lt W p.1)
  let atH := v8 && pts.any (fun p => !lt p.2 H && !lt H p.2)
  let edge := if persp then zero else half
  (sub W (if atW then zero else edge), sub H (if atH then zero else edge),
   [pts.any (fun p => lt p.1 zero), pts.any (fun p => lt W p.1), pts.any (fun p => lt p.2 zero), pts.any (fun p => lt H p.2)])

/-- D3D9.RASTER.CLIP8SIDE: Direct3D 9 clips the right and bottom sides half a pixel in -/
theorem d3d9_half_in (pts : List (Nat × Nat)) (w h : Nat) :
    (bounds pts w h false false).1 = sub (F32.ofNat w) half ∧ (bounds pts w h false false).2.1 = sub (F32.ofNat h) half := by
  simp [bounds]

/-- D3D9.RASTER.CLIP8: the sides clipped are those some vertex is strictly outside of -/
theorem outside_sides (pts : List (Nat × Nat)) (w h : Nat) (v8 persp : Bool) :
    (bounds pts w h v8 persp).2.2 = [pts.any (fun p => lt p.1 zero), pts.any (fun p => lt (F32.ofNat w) p.1),
      pts.any (fun p => lt p.2 zero), pts.any (fun p => lt (F32.ofNat h) p.2)] := rfl

/-- D3D9.RASTER.CLIP: a coordinate snapped to 1/256 pixel, to nearest even, in 1/256ths (Rust's
`as i64` of the f64: past i64 it saturates, NaN gives 0) -/
def snap (v : Nat) : Int :=
  if F32.isNaN v then 0 else
  let (s, n, d) := Float.toRat F32.fmt v
  let q := n * 256 / d
  let r := n * 256 % d
  let m : Nat := if 2 * r > d || (2 * r = d && q % 2 = 1) then q + 1 else q
  if s then -(m : Int) else m

/-- D3D9.RASTER.CLIP: snapping keeps the sign and is within half of a 1/256th of the value -/
theorem snap_near (v : Nat) (h : F32.isNaN v = false) (s : Bool) (n d : Nat)
    (hv : Float.toRat F32.fmt v = (s, n, d)) (hd : 0 < d) :
    (2 * ((n * 256 : Nat) : Int) - 2 * (d : Int) * (if s then -snap v else snap v)).natAbs ≤ d := by
  simp only [snap, h, Bool.false_eq_true, ↓reduceIte, hv]
  have e := Nat.div_add_mod (n * 256) d
  have hr := Nat.mod_lt (n * 256) hd
  generalize n * 256 / d = q at *
  generalize n * 256 % d = r at *
  generalize n * 256 = N at *
  have e' : (d : Int) * q + r = N := by exact_mod_cast e
  cases s <;> simp only [Bool.false_eq_true, ↓reduceIte, Int.neg_neg] <;> split <;>
    simp only [Int.natCast_add, Int.natCast_one, Int.mul_add, Int.mul_one, Int.mul_assoc, Bool.or_eq_true, decide_eq_true_eq, Bool.and_eq_true, not_or, not_and] at * <;> omega

private def toV : List Nat → Option V
  | x :: y :: a => some ⟨x, y, a⟩
  | _ => none

def mirrors : List Mirror := [
  ⟨"d3d9.clip", fun
    | x1 :: y1 :: l :: r :: t :: b :: n :: rest => do
      let k ← nat n
      let ws ← rest.mapM nat
      let per := ws.length / 3
      if k != 19 || ws.length != 3 * per then none
      let vs ← [0, 1, 2].mapM fun i => toV ((ws.drop (i * per)).take per)
      let sides : Fin 4 → Bool := fun s => [l, r, t, b].getD s.val 0 != 0
      let out := clipPolygon vs (← nat x1) (← nat y1) sides
      pure (out.flatMap fun v => (v.x :: v.y :: v.attrs).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.snap", fun
    | [v] => do pure [snap (← nat v)]
    | _ => none⟩,
  ⟨"d3d9.leastx", fun
    | [a, b, c, d, e, f] => do pure [leastX [(← nat a, ← nat b), (← nat c, ← nat d), (← nat e, ← nat f)]]
    | _ => none⟩,
  ⟨"d3d9.clipbounds", fun
    | [w, h, v8, persp, a, b, c, d, e, f] => do
      let r := bounds [(← nat a, ← nat b), (← nat c, ← nat d), (← nat e, ← nat f)] (← nat w) (← nat h) (v8 != 0) (persp != 0)
      pure ([r.1, r.2.1].map (fun x : Nat => (x : Int)) ++ r.2.2.map (fun s => if s then 1 else 0))
    | _ => none⟩
]

end Kikizake.D3d9.Clip
