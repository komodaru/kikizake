import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Clip
/-! WARP's aliased line, the rasterizer behind Direct3D 9's lines (spec/d3d9draw.txt D3D9.DRAW.LINES,
DRAW.LINEEND, DRAW.POINTSLINES), in binary32 (Kikizake.F32) as src/model/warpline.rs `setup` and
`coverage` compute it (mirror `d3d9.line`), and the cut of a line past the target (DRAW.LINECLIP,
LINECLIP_NEAR) as src/model/d3d9draw.rs `line_cut` (mirror `d3d9.linecut`): the ends in 1/256 pixel in Direct3D 10's coordinates,
an end in its pixel's diamond moved to 1/256 short of the center along the major axis, a pixel lit
where the segment crosses its center column or row within (-0.5, 0x3F000001) of the center. -/
namespace Kikizake.D3d9.Line

open Kikizake.F32 (add sub mul div lt isNaN)
open Kikizake.D3d9.Clip (half mhalf zero)

def one : Nat := 0x3f800000
def mone : Nat := 0xbf800000
/-- 1/256, the setup's nudge -/
def nudge : Nat := 0x3b800000
/-- 0.5000001, the bound above on the crossing -/
def hi : Nat := 0x3f000001
/-- 2^-12, how near a bound a crossing is a tie -/
def tieNear : Nat := 0x39800000

/-- Rust's `i as f32` of an integer -/
def ofInt (i : Int) : Nat := Float.round F32.fmt (decide (i < 0)) i.natAbs 1

def abs (x : Nat) : Nat := if Float.sign F32.fmt x then Float.neg F32.fmt x else x

/-- a == b of two non-NaN values -/
def same (a b : Nat) : Bool := !lt a b && !lt b a

/-- ceil(v) - 0.5: the center of the pixel whose diamond can hold v -/
def owner (v : Nat) : Nat := sub (Clip.ceil v) half

/-- in the diamond of the pixel centered at c: the bottom half with its edge, the top half without;
on the middle row a y-major line's right corner too -/
def inDiamond (p c : Nat × Nat) (xmajor : Bool) : Bool :=
  let d := add (abs (sub p.1 c.1)) (abs (sub p.2 c.2))
  let inclusive := lt c.2 p.2 || (same p.2 c.2 && !xmajor && lt c.1 p.1)
  if inclusive then !lt half d else lt d half

/-- ±1/256 by the sign of d (Rust's `if d > 0.0 { NUDGE } else { -NUDGE }`) -/
def toward (d : Nat) : Nat := if lt zero d then nudge else Float.neg F32.fmt nudge

/-- the line's setup: start, direction, x-major (none: nothing lit) -/
def setup (a b : Int × Int) : Option ((Nat × Nat) × (Nat × Nat) × Bool) :=
  let fx := fun v : Int => mul (ofInt v) (div one (ofInt 256))
  let x0 := fx a.1
  let y0 := fx a.2
  let x1 := fx b.1
  let y1 := fx b.2
  if same x0 x1 && same y0 y1 then none else
  let xmajor := !lt (abs (sub x0 x1)) (abs (sub y0 y1))
  let dx := sub x1 x0
  let dy := sub y1 y0
  let c0 := (owner x0, owner y0)
  let c1 := (owner x1, owner y1)
  let inS := inDiamond (x0, y0) c0 xmajor
  let inE := inDiamond (x1, y1) c1 xmajor
  if inS && inE && same c0.1 c1.1 && same c0.2 c1.2 then none else
  let (x, y, ddx, ddy) :=
    if inS then
      let (u, v) :=
        if xmajor then let u := add (sub x0 c0.1) (toward dx); (u, mul (div dy dx) u)
        else let v := add (sub y0 c0.2) (toward dy); (mul (div dx dy) v, v)
      (sub x0 u, sub y0 v, add dx u, add dy v)
    else (x0, y0, dx, dy)
  let (ddx, ddy) :=
    if inE then
      let (u, v) :=
        if xmajor then let u := add (sub (add x ddx) c1.1) (toward ddx); (u, mul (div ddy ddx) u)
        else let v := add (sub (add y ddy) c1.2) (toward ddy); (mul (div ddx ddy) v, v)
      (sub ddx u, sub ddy v)
    else (ddx, ddy)
  some ((x, y), (ddx, ddy), xmajor)

/-- 0 ≤ t ≤ 1 (false for NaN) -/
def unit (t : Nat) : Bool := !isNaN t && !lt t zero && !lt one t

/-- the crossing e within (-0.5, top) (false for NaN) -/
def within (e top : Nat) : Bool := !isNaN e && lt mhalf e && lt e top

/-- within 2^-12 of ±0.5: f32's rounding decided -/
def near (e : Nat) : Bool := !isNaN e && lt (abs (sub (abs e) half)) tieNear

/-- the crossings at pixel (i, j)'s center column and center row, where the segment meets them -/
def crossings (s : (Nat × Nat) × (Nat × Nat) × Bool) (i j : Nat) : Option Nat × Option Nat :=
  let ((x0, y0), (dx, dy), _) := s
  let px := add (F32.ofNat i) half
  let py := add (F32.ofNat j) half
  let ea := sub x0 px
  let eb := sub y0 py
  let t := mul ea (div mone dx)
  let t2 := mul eb (div mone dy)
  (if unit t then some (add (mul t dy) eb) else none, if unit t2 then some (add (mul t2 dx) ea) else none)

/-- D3D9.DRAW.LINES: pixel (i, j) lit: its center column crossed within (-0.5, 0x3F000001) of the
center, or its center row within (-0.5, 0.5) for an x-major line, else (-0.5, 0x3F000001) -/
def lit (s : (Nat × Nat) × (Nat × Nat) × Bool) (i j : Nat) : Bool :=
  let c := crossings s i j
  (match c.1 with | some e => within e hi | none => false) ||
  (match c.2 with | some e => within e (if s.2.2 then half else hi) | none => false)

def cells (w h : Nat) : List (Nat × Nat) := (List.range h).flatMap fun j => (List.range w).map fun i => (i, j)

/-- the pixels of a w x h target the line lights, row by row, and whether a crossing was a tie -/
def coverage (a b : Int × Int) (w h : Nat) : List (Nat × Nat) × Bool :=
  match setup a b with
  | none => ([], false)
  | some s =>
    ((cells w h).filter (fun p => lit s p.1 p.2),
     (cells w h).any fun p => let c := crossings s p.1 p.2; (c.1.map near).getD false || (c.2.map near).getD false)

private theorem lt_irrefl (x : Nat) : lt x x = false := by
  simp only [F32.lt, Float.lt]
  split <;> simp_all

/-- D3D9.DRAW.LINES: a line of one point lights nothing -/
theorem point_unlit (a : Int × Int) (w h : Nat) : coverage a a w h = ([], false) := by
  simp [coverage, setup, same, lt_irrefl]

/-- D3D9.DRAW.LINES: both ends in one pixel's diamond light nothing -/
theorem one_diamond (a b : Int × Int) (w h : Nat)
    (h1 : setup a b = none) : coverage a b w h = ([], false) := by
  simp [coverage, h1]

/-- D3D9.DRAW.LINES: only the target's pixels are lit, each by the column or row test -/
theorem lit_within (a b : Int × Int) (w h : Nat) (p : Nat × Nat) (hp : p ∈ (coverage a b w h).1) :
    p.1 < w ∧ p.2 < h ∧ ∃ s, setup a b = some s ∧ lit s p.1 p.2 = true := by
  unfold coverage at hp
  split at hp
  · simp at hp
  · rename_i s hs
    simp only [List.mem_filter, cells, List.mem_flatMap, List.mem_range, List.mem_map] at hp
    obtain ⟨⟨j, hj, i, hi, rfl⟩, hl⟩ := hp
    exact ⟨hi, hj, s, hs, hl⟩

/-- D3D9.DRAW.LINEEND: an end outside its diamond stays where it is: the setup starts at the
snapped start -/
theorem start_kept (a b : Int × Int) (s : (Nat × Nat) × (Nat × Nat) × Bool) (h : setup a b = some s)
    (hs : inDiamond (mul (ofInt a.1) (div one (ofInt 256)), mul (ofInt a.2) (div one (ofInt 256)))
      (owner (mul (ofInt a.1) (div one (ofInt 256))), owner (mul (ofInt a.2) (div one (ofInt 256)))) s.2.2 = false) :
    s.1 = (mul (ofInt a.1) (div one (ofInt 256)), mul (ofInt a.2) (div one (ofInt 256))) := by
  unfold setup at h
  simp only at h
  split at h
  · simp at h
  · split at h
    · simp at h
    · simp only [Option.some.injEq] at h
      subst h
      simp_all

/-- Rust's `a.max(b)` of two non-NaN values (a zero's sign aside, which no use here tells) -/
def max (a b : Nat) : Nat := if lt a b then b else a

/-- D3D9.DRAW.LINECLIP: the stretch of a..b inside [-0.5, mx] x [-0.5, my] in f32 (Liang-Barsky):
per edge t = (edge - a) / (b - a), the ends a + t (b - a), t 0 and 1 the ends themselves -/
def clipLine (a b : Nat × Nat) (mx my : Nat) : Option ((Nat × Nat) × (Nat × Nat)) :=
  let step := fun (acc : Option (Nat × Nat)) (e : Nat × Nat × Nat) =>
    match acc with
    | none => none
    | some (t0, t1) =>
      let (pa, pb, top) := e
      let d := sub pb pa
      if same d zero then (if lt pa mhalf || lt top pa then none else some (t0, t1)) else
      let tl := div (sub mhalf pa) d
      let th := div (sub top pa) d
      let (enter, leave) := if lt zero d then (tl, th) else (th, tl)
      some (max t0 enter, Clip.min t1 leave)
  match [(a.1, b.1, mx), (a.2, b.2, my)].foldl step (some (zero, one)) with
  | none => none
  | some (t0, t1) =>
    if lt t1 t0 then none else
    let pt := fun t => if same t zero then a else if same t one then b else
      (add a.1 (mul t (sub b.1 a.1)), add a.2 (mul t (sub b.2 a.2)))
    some (pt t0, pt t1)

def inside (p : Nat × Nat) (w h : Nat) : Bool :=
  !isNaN p.1 && !isNaN p.2 && !lt p.1 mhalf && !lt (sub (F32.ofNat w) half) p.1 && !lt p.2 mhalf && !lt (sub (F32.ofNat h) half) p.2

/-- D3D9.DRAW.LINECLIP_NEAR: an end past the target only on the right or bottom, by at most half a
pixel, moved onto the edges (min(x, w - 0.5), min(y, h - 0.5)) -/
def nearMove (p : Nat × Nat) (rx by_ : Nat) : Nat × Nat :=
  let past := lt rx p.1 || lt by_ p.2
  let near := !lt p.1 mhalf && !lt p.2 mhalf && !lt (add rx half) p.1 && !lt (add by_ half) p.2
  if past && near then (Clip.min p.1 rx, Clip.min p.2 by_) else p

def snapPt (p : Nat × Nat) : Int × Int := (Clip.snap p.1, Clip.snap p.2)

/-- the segment from pa to pb cut to the target (Direct3D 8: its right (bottom) edge at the size
itself where an end is at the width (height)) and snapped again -/
def cutFrom (pa pb : Nat × Nat) (w h : Nat) (v8 : Bool) : Option ((Int × Int) × (Int × Int)) :=
  let wf := F32.ofNat w
  let hf := F32.ofNat h
  let side := fun (size : Nat) (onIt : Bool) => if v8 && onIt then size else sub size half
  let mx := side wf (same pa.1 wf || same pb.1 wf)
  let my := side hf (same pa.2 hf || same pb.2 hf)
  (clipLine pa pb mx my).map fun (ca, cb) => (snapPt ca, snapPt cb)

def unsnap (p : Int × Int) : Nat × Nat := (div (ofInt p.1) (ofInt 256), div (ofInt p.2) (ofInt 256))

/-- D3D9.DRAW.LINECLIP: the line's snapped ends and the segment drawn (past the target cut to it in
f32 and snapped again); none: nothing drawn -/
def cut (a b : Nat × Nat) (w h : Nat) (v8 : Bool) : Option ((Int × Int) × (Int × Int) × (Int × Int) × (Int × Int)) :=
  let la := snapPt a
  let lb := snapPt b
  if la = lb then none else
  let r : Option ((Int × Int) × (Int × Int)) :=
    if inside a w h && inside b w h then some (la, lb) else
    let rx := sub (F32.ofNat w) half
    let by_ := sub (F32.ofNat h) half
    let (pa, pb) := if !inside a w h && !inside b w h then (nearMove (unsnap la) rx by_, nearMove (unsnap lb) rx by_)
      else (unsnap la, unsnap lb)
    cutFrom pa pb w h v8
  match r with
  | none => none
  | some (ra, rb) => if ra = rb then none else some (la, lb, ra, rb)

/-- D3D9.DRAW.LINECLIP: a line inside the target is drawn as snapped, not cut -/
theorem inside_kept (a b : Nat × Nat) (w h : Nat) (v8 : Bool) (ha : inside a w h = true) (hb : inside b w h = true)
    (hne : snapPt a ≠ snapPt b) : cut a b w h v8 = some (snapPt a, snapPt b, snapPt a, snapPt b) := by
  simp [cut, ha, hb, hne]

/-- D3D9.DRAW.LINECLIP: a line of one snapped point draws nothing -/
theorem one_point (a b : Nat × Nat) (w h : Nat) (v8 : Bool) (he : snapPt a = snapPt b) : cut a b w h v8 = none := by
  simp [cut, he]

/-- D3D9.DRAW.LINECLIP: the segment drawn is never one point -/
theorem drawn_apart (a b : Nat × Nat) (w h : Nat) (v8 : Bool) (la lb ra rb : Int × Int)
    (hc : cut a b w h v8 = some (la, lb, ra, rb)) : ra ≠ rb := by
  unfold cut at hc
  simp only at hc
  split at hc
  · simp at hc
  · split at hc
    · simp at hc
    · rename_i r0 r1 _ _
      split at hc
      · simp at hc
      · simp only [Option.some.injEq, Prod.mk.injEq] at hc
        obtain ⟨_, _, rfl, rfl⟩ := hc
        assumption

/-- D3D9.DRAW.LINECLIP_NEAR: a line past the target at both ends has its ends moved onto the right
and bottom edges first (when only so near past them), then is cut -/
theorem cut_near (a b : Nat × Nat) (w h : Nat) (v8 : Bool) (ha : inside a w h = false) (hb : inside b w h = false)
    (hne : snapPt a ≠ snapPt b) :
    cut a b w h v8 = (match cutFrom (nearMove (unsnap (snapPt a)) (sub (F32.ofNat w) Clip.half) (sub (F32.ofNat h) Clip.half))
        (nearMove (unsnap (snapPt b)) (sub (F32.ofNat w) Clip.half) (sub (F32.ofNat h) Clip.half)) w h v8 with
      | none => none
      | some (ra, rb) => if ra = rb then none else some (snapPt a, snapPt b, ra, rb)) := by
  simp [cut, ha, hb, hne]

/-- D3D9.DRAW.LINECLIP: a line with an end inside is cut as snapped, nothing moved -/
theorem cut_one_inside (a b : Nat × Nat) (w h : Nat) (v8 : Bool) (ha : inside a w h = true) (hb : inside b w h = false)
    (hne : snapPt a ≠ snapPt b) :
    cut a b w h v8 = (match cutFrom (unsnap (snapPt a)) (unsnap (snapPt b)) w h v8 with
      | none => none
      | some (ra, rb) => if ra = rb then none else some (snapPt a, snapPt b, ra, rb)) := by
  simp [cut, ha, hb, hne]

/-- D3D9.DRAW.LINECLIP_NEAR: an end inside the target or past it on the left or top is not moved -/
theorem near_keeps (p : Nat × Nat) (rx by_ : Nat) (h : lt rx p.1 = false ∧ lt by_ p.2 = false ∨ lt p.1 mhalf = true ∨ lt p.2 mhalf = true) :
    nearMove p rx by_ = p := by
  unfold nearMove
  rcases h with ⟨h1, h2⟩ | h | h
  · simp [h1, h2]
  · simp [h]
  · simp [h]

def mirrors : List Mirror := [
  ⟨"d3d9.line", fun
    | [ax, ay, bx, by_, w, h] => do
      let (l, t) := coverage (ax, ay) (bx, by_) (← nat w) (← nat h)
      let st : List Int := match setup (ax, ay) (bx, by_) with
        | none => [-1]
        | some ((x, y), (dx, dy), xm) => [x, y, dx, dy].map (fun b => (F32.canon b : Int)) ++ [if xm then 1 else 0]
      pure ((l.flatMap fun p => [(p.1 : Int), (p.2 : Int)]) ++ [if t then 1 else 0] ++ st)
    | _ => none⟩,
  ⟨"d3d9.linecut", fun
    | [ax, ay, bx, by_, w, h, v8] => do
      match cut (← nat ax, ← nat ay) (← nat bx, ← nat by_) (← nat w) (← nat h) (v8 != 0) with
      | none => pure [-1]
      | some (la, lb, ra, rb) => pure [la.1, la.2, lb.1, lb.2, ra.1, ra.2, rb.1, rb.2]
    | _ => none⟩
]

end Kikizake.D3d9.Line
