import Kikizake.GdiPlus.Stroke

/-! The widening kernel for an interior vertex. A segment becomes a binary32 unit
gradient and its clockwise normal. The join uses those gradients to generate the
points of each offset side, including Bezier types and inner bevel flags. -/
namespace Kikizake.GdiPlus.Wide

open Kikizake.Float
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Matrix

private abbrev F := F32.fmt
private def epsF : Nat := 0x34000000
abbrev Pair := Nat × Nat

structure Segment where
  g : Pair
  n : Pair

structure Out where
  point : Pair
  type : Nat

private def line (p : Pair) : Out := ⟨p, 1⟩
private def bezier (p : Pair) : Out := ⟨p, 3⟩
private def negPoint (p : Pair) : Pair := (neg F p.1, neg F p.2)

/-- Equality and ordering with the binary32 zero and NaN rules. -/
private def feq (a b : Nat) : Bool :=
  !isNaN F a && !isNaN F b && (a == b || (isZero a && isZero b))
private def fle (a b : Nat) : Bool := flt a b || feq a b

private def scale (r : Ops) (w : Nat) (v : Pair) : Pair := (r.mul w v.1, r.mul w v.2)
private def add (r : Ops) (a b : Pair) : Pair := (r.add a.1 b.1, r.add a.2 b.2)
private def sub (r : Ops) (a b : Pair) : Pair := (r.sub a.1 b.1, r.sub a.2 b.2)
private def off (r : Ops) (w : Nat) (n p : Pair) : Pair := add r (scale r w n) p
private def square (r : Ops) (a : Nat) : Nat := r.mul a a

private def norm2 (r : Ops) (p : Pair) : Nat :=
  r.add (square r p.2) (square r p.1)

def segment (d : Bool) (r : Ops) (a b : Pair) : Segment :=
  let delta := sub r b a
  let len := Stroke.sqrt32 d (norm2 r delta)
  let g := (r.div delta.1 len, r.div delta.2 len)
  ⟨g, (g.2, neg F g.1)⟩

private def tiny (p : Pair) : Bool := flt (fabs p.1) epsF && flt (fabs p.2) epsF
private def closeGradients (r : Ops) (g0 g1 : Pair) : Bool :=
  let delta := sub r g0 g1
  flt (r.add (square r delta.1) (square r delta.2)) 0x3727c5ac

private def rawCross (r : Ops) (g0 g1 : Pair) : Nat :=
  r.sub (r.mul g1.2 g0.1) (r.mul g0.2 g1.1)
private def zeroNearCross (c : Nat) : Nat := if fle (fabs c) epsF then 0 else c
private def crossOf (r : Ops) (g0 g1 : Pair) : Nat :=
  if tiny g0 || tiny g1 || closeGradients r g0 g1 then 0
  else zeroNearCross (rawCross r g0 g1)

private def miterExcess (r : Ops) (delta : Pair) (c lim : Nat) : Nat :=
  r.sub (norm2 r delta) (r.mul (square r c) lim)

private def exceeded (r : Ops) (g0 g1 : Pair) (lim : Nat) : Bool :=
  let c := r.sub (r.mul g0.1 g1.2) (r.mul g1.1 g0.2)
  fle (fabs c) epsF || flt 0 (miterExcess r (sub r g0 g1) c lim)

private def marked (p : Pair) (inside : Bool) : Out :=
  ⟨p, if inside then 0x41 else 1⟩

/-- The miter, bevel, and limit-clipped join geometry, including compound pen bands. -/
def miterBevel (d : Bool) (r : Ops) (p : Pair) (s0 s1 : Segment)
    (wl wr lim : Nat) (compound allowMiter : Bool) : List Out × List Out :=
  if tiny s0.g || tiny s1.g || closeGradients r s0.g s1.g then
    let n := if isZero s0.n.1 && isZero s0.n.2 then s1.n else s0.n
    ([line (off r wl n p)], [line (off r wr n p)])
  else
    let cross := zeroNearCross (rawCross r s0.g s1.g)
    let positive := flt 0 cross
    let inside w := if positive then flt w 0 else flt 0 w
    let delta := sub r s0.g s1.g
    let enabled := allowMiter && flt one lim && !isZero cross
    let legal := enabled && fle (miterExcess r delta cross lim) 0
    let atMiter := (r.div delta.1 cross, r.div delta.2 cross)
    let clipped := enabled && !legal
    let tt := Stroke.sqrt32 d (r.add (r.mul 0 0) (r.sub lim one))
    let side (w : Nat) (miterSide : Bool) :=
      if legal && miterSide then [line (off r w atMiter p)] else
      let (t0, t1) :=
        if clipped && !inside w then
          let s := if positive then w else neg F w
          (r.mul (r.sub tt 0) s, r.mul (r.add tt 0) s)
        else (0, 0)
      let a := add r (off r w s0.n p) (scale r t0 s0.g)
      let b := sub r (off r w s1.n p) (scale r t1 s1.g)
      let distance := sub r b a
      if flt 0x3a1c4000 (r.add (fabs distance.1) (fabs distance.2)) then
        [marked a (inside w), marked b (inside w)]
      else [line a]
    (side wl (compound || positive), side wr (compound || (fle cross 0 && !inside wr)))

inductive Make
  | both
  | arcs
  | inner
  deriving BEq

private def withArc (m : Make) : Bool := m != Make.inner
private def withInside (m : Make) : Bool := m != Make.arcs

/-- A bevel on the inner side and a cubic arc on the outer side of a round join. -/
def smallRound (d : Bool) (r : Ops) (p : Pair) (s0 s1 : Segment)
    (dot cross wl wr : Nat) (compound : Bool) (make : Make) : List Out × List Out :=
  if !flt dot 0x3f7d70a4 then
    if withInside make then ([line (off r wl s0.n p)], [line (off r wr s0.n p)]) else ([], [])
  else
    let om := r.sub one dot
    let t := r.div (r.mul (r.sub (Stroke.sqrt32 d (r.add om om)) (fabs cross)) 0x40800000)
      (r.mul om 0x40400000)
    let ac := fabs cross
    let delta := sub r s0.g s1.g
    let hasInnerMiter :=
      compound && !flt ac epsF && !flt 0 (miterExcess r delta cross 0x41100000)
    let side w :=
      let (w, an, bn) :=
        if fle 0 cross then (w, s0.n, s1.n)
        else (neg F w, negPoint s0.n, negPoint s1.n)
      let pointAt n := add r (scale r w n) p
      if flt 0 w then
        if !withArc make then [] else
        let k := r.mul w t
        let a := pointAt an
        let b := pointAt bn
        [line a, bezier (add r (scale r k s0.g) a), bezier (sub r b (scale r k s1.g)), bezier b]
      else if !withInside make then []
      else if isZero w then [line p]
      else if hasInnerMiter then [line (pointAt (r.div delta.1 ac, r.div delta.2 ac))]
      else [line (pointAt an), line (pointAt bn)]
    (side wl, side wr)

private def combineArcs (inside a b : List Out) : List Out :=
  match a, b with
  | [], _ => inside ++ b
  | _, [] => inside ++ a
  | _, p :: tail => inside ++ a.dropLast ++ bezier p.point :: tail

/-- A sharper round join uses two quarter-arcs and the midpoint normal. -/
def roundJoin (d : Bool) (r : Ops) (p : Pair) (s0 s1 : Segment)
    (wl wr : Nat) (compound : Bool) : List Out × List Out :=
  let dot := r.add (r.mul s1.g.2 s0.g.2) (r.mul s0.g.1 s1.g.1)
  let cross := crossOf r s0.g s1.g
  if flt 0xba83126f dot then smallRound d r p s0 s1 dot cross wl wr compound .both else
  let (il, ir) := smallRound d r p s0 s1 dot cross wl wr compound .inner
  let sum := add r s0.n s1.n
  let (mx, my) := if !tiny sum then
      let len := Stroke.sqrt32 d (norm2 r sum)
      (r.div sum.1 len, r.div sum.2 len)
    else (neg F s0.n.2, s0.n.1)
  let m := ⟨(neg F my, mx), (mx, my)⟩
  let dot2 := r.add (r.mul s0.g.2 mx) (r.mul s0.g.1 (neg F my))
  let cross2 := r.sub (r.mul s0.g.1 mx) (r.mul s0.g.2 (neg F my))
  let (a1l, a1r) := smallRound d r p s0 m dot2 cross2 wl wr compound .arcs
  let dot3 := r.add (r.mul s1.g.2 mx) (r.mul s1.g.1 (neg F my))
  let cross3 := r.sub (r.mul s1.g.2 (neg F my)) (r.mul s1.g.1 mx)
  let (a2l, a2r) := smallRound d r p m s1 dot3 cross3 wl wr compound .arcs
  (combineArcs il a1l a2l, combineArcs ir a1r a2r)

def join (d compound : Bool) (style : Int) (prev p next : Pair)
    (wl wr miter : Nat) : List Out × List Out :=
  let r := ops d
  let s0 := segment d r prev p
  let s1 := segment d r p next
  let lim := square r (if flt one miter then miter else one)
  match style with
  | 0 => miterBevel d r p s0 s1 wl wr lim compound true
  | 1 => miterBevel d r p s0 s1 wl wr lim compound false
  | 2 => roundJoin d r p s0 s1 wl wr compound
  | 3 => miterBevel d r p s0 s1 wl wr lim compound (!exceeded r s0.g s1.g lim)
  | _ => ([], [])

def encoded (l : List Out) : List Nat :=
  l.length :: l.flatMap fun o => [o.type, F32.canon o.point.1, F32.canon o.point.2]

def mirrorJoin (d compound : Bool) (style : Int) (prev p next : Pair)
    (wl wr miter : Nat) : List Nat :=
  let (left, right) := join d compound style prev p next wl wr miter
  encoded left ++ encoded right

/-- Every bevel has at most two points per side, including degenerate gradients. -/
theorem bevel_side_bound (d : Bool) (r : Ops) (p : Pair) (s0 s1 : Segment)
    (wl wr lim : Nat) (compound : Bool) :
    (miterBevel d r p s0 s1 wl wr lim compound false).1.length ≤ 2 ∧
    (miterBevel d r p s0 s1 wl wr lim compound false).2.length ≤ 2 := by
  unfold miterBevel
  split
  · simp
  · constructor <;> simp <;> split <;> simp

/-- A plain right-angle miter leaves the two inner bevel points marked. -/
theorem right_angle_miter :
    mirrorJoin false false 0 (0, 0) (0x40000000, 0) (0x40000000, 0x40000000)
      one 0xbf800000 0x40800000 =
    [1, 1, 0x40400000, 0xbf800000, 2, 0x41, 0x40000000, one, 0x41, one, 0] := by
  decide +kernel

/-- A compound band permits one miter point on each side within the same limit. -/
theorem right_angle_compound :
    mirrorJoin false true 0 (0, 0) (0x40000000, 0) (0x40000000, 0x40000000)
      one 0xbf800000 0x40800000 =
    [1, 1, 0x40400000, 0xbf800000, 1, 1, one, one] := by
  decide +kernel

/-- A limit of 1.25 is too short for this clipped right-angle miter. -/
theorem limited_miter_bevels :
    mirrorJoin false false 3 (0, 0) (0x40000000, 0) (0x40000000, 0x40000000)
      one 0xbf800000 0x3fa00000 =
    mirrorJoin false false 1 (0, 0) (0x40000000, 0) (0x40000000, 0x40000000)
      one 0xbf800000 0x3fa00000 := by
  decide +kernel

/-- The outer right-angle round join uses three cubic points after its line point. -/
theorem round_right_angle :
    mirrorJoin false false 2 (0, 0) (0x40000000, 0) (0x40000000, 0x40000000)
      one 0xbf800000 0x40800000 =
    [4, 1, 0x40000000, 0xbf800000, 3, 0x402358a2, 0xbf800000, 3, 0x40400000, 0xbf0d6288,
     3, 0x40400000, 0, 2, 1, 0x40000000, one, 1, one, 0] := by
  decide +kernel

structure Band where
  width : Nat
  left : Nat
  right : Nat
  miter : Nat
  style : Int
  compound : Bool

private def firstPoint (out : List Out) : List Out :=
  match out with
  | [] => []
  | p :: rest => { p with type := p.type / 64 * 64 } :: rest

/-- Reverse an outline side while keeping the type of each backward curve edge. -/
def reverseSide (out : List Out) : List Out :=
  let kinds := (out.drop 1).map (fun p => p.type % 64) ++ [1]
  (out.zip kinds).reverse.map fun (p, k) =>
    { p with type := p.type / 64 * 64 + if k == 3 then 3 else 1 }

private def bandJoin (d : Bool) (b : Band) (a p c : Pair) :=
  join d b.compound b.style a p c b.left b.right b.miter

private def capTip (r : Ops) (b : Band) (p g : Pair) (start : Bool) : Pair :=
  let dir := if start then negPoint g else g
  let n := (g.2, neg F g.1)
  let (a, c) := (off r b.right n p, off r b.left n p)
  let width := fabs (r.sub b.left b.right)
  scale r 0x3f000000 (add r (add r a c) (scale r width dir))

/-- One line point and six cubic points make a round end cap. -/
def roundCap (r : Ops) (b : Band) (p g : Pair) (start : Bool) : List Out :=
  let (dx, dy) := if start then negPoint g else g
  let radius := r.mul (r.sub b.left b.right) 0x3f000000
  let side := r.add b.left b.right
  let c := (r.add (r.mul (r.mul side g.2) 0x3f000000) p.1,
    r.sub p.2 (r.mul (r.mul side g.1) 0x3f000000))
  let (x, y) := (r.mul radius dy, r.mul (neg F radius) dx)
  let k := 0x3f0d6289
  let units := [(one, 0), (one, k), (k, one), (0, one), (neg F k, one),
    (neg F one, k), (neg F one, 0)]
  let signedZero := r.sub 0 0
  units.zipIdx.map fun ((u, v), i) =>
    let px := r.add (r.sub (r.mul u x) (r.mul v y)) c.1
    let py := r.add (r.add (r.mul v x) (r.mul u y)) c.2
    if i == 0 then line (px, py) else bezier (r.add px signedZero, r.add py signedZero)

private def capArc (r : Ops) (b : Band) (p g : Pair) (start doubleRound : Bool) : List Out :=
  if !doubleRound then roundCap r b p g start else
  let mid := r.mul (r.add b.left b.right) 0x3f000000
  let left := { b with right := mid }
  let right := { b with left := mid }
  let (first, second) := if start then (right, left) else (left, right)
  roundCap r first p g start ++ (roundCap r second p g start).drop 1

private def setFirstPoint (out : List Out) (p : Pair) : List Out :=
  match out with
  | [] => []
  | h :: t => { h with point := p } :: t

private def dot (r : Ops) (a b : Pair) : Nat :=
  r.add (r.mul a.1 b.1) (r.mul a.2 b.2)

/-- If an interior join extends behind an endpoint, move the endpoint to the join. -/
private def repairSide (r : Ops) (first last : Segment) (side : List Out) : List Out :=
  let a := side.toArray
  let n := a.size
  if n ≤ 2 then side else
  let p0 := a.getD 0 (line (0, 0))
  let p1 := a.getD 1 p0
  let a := if flt (dot r (sub r p1.point p0.point) first.g) 0 then
      a.modify 0 (fun p => { p with point := p1.point })
    else a
  let pn := a.getD (n - 1) p0
  let prev := a.getD (n - 2) pn
  let a := if flt (dot r (sub r pn.point prev.point) last.g) 0 then
      a.modify (n - 1) (fun p => { p with point := prev.point })
    else a
  a.toList

private def getPoint (p : Array Pair) (i : Nat) : Pair := p.getD i (0, 0)

def openBand (d : Bool) (b : Band) (p : Array Pair) (startCap endCap : Nat)
    (doubleRound : Bool := false) : List Out :=
  let r := ops d
  let n := p.size
  if n < 2 then [] else
  let first := segment d r (getPoint p 0) (getPoint p 1)
  let last := segment d r (getPoint p (n - 2)) (getPoint p (n - 1))
  let start := getPoint p 0
  let end_ := getPoint p (n - 1)
  let wrap := segment d r end_ start
  let startNormal :=
    if n ≥ 3 && (tiny wrap.g || tiny first.g || closeGradients r wrap.g first.g) &&
      !(isZero wrap.n.1 && isZero wrap.n.2) then wrap.n else first.n
  let wrapped := !(feq startNormal.1 first.n.1 && feq startNormal.2 first.n.2)
  let capShift := r.mul b.width 0xbf000000
  let atStart w :=
    let q := off r w startNormal start
    let shift := if startCap == 1 then capShift else 0x80000000
    add r q (scale r shift first.g)
  let atEnd w right :=
    let q := off r w last.n end_
    let shift := if endCap == 1 then capShift else 0x80000000
    if endCap == 3 && right then q else sub r q (scale r shift last.g)
  let (innerLeft, innerRight) := (List.range (n - 2)).foldl (fun (l, rt) i =>
    let (a, c) := bandJoin d b (getPoint p i) (getPoint p (i + 1)) (getPoint p (i + 2))
    (l ++ a, rt ++ c)) ([], [])
  let side w inner right := repairSide r first last (line (atStart w) :: inner ++ [line (atEnd w right)])
  let (left, right) := (side b.left innerLeft false, side b.right innerRight true)
  let firstLeft := (left.head?.getD (line (atStart b.left))).point
  let firstRight := (right.head?.getD (line (atStart b.right))).point
  let lastLeft := (left.getLast?.getD (line (atEnd b.left false))).point
  let front := firstPoint left
  let back := reverseSide right
  let (front, back) :=
    if endCap == 3 then (front ++ [line (capTip r b end_ last.g false)], back)
    else if endCap == 2 then
      (front.dropLast ++ setFirstPoint (capArc r b end_ last.g false doubleRound) lastLeft, back.drop 1)
    else (front, back)
  let back := if startCap == 2 && !wrapped then back.dropLast else back
  let out := front ++ back
  let closeTo (a c : Pair) :=
    flt (r.add (fabs (r.sub a.2 c.2)) (fabs (r.sub a.1 c.1))) 0x3a1c4000
  if startCap == 3 then
    let tip := line (capTip r b start first.g true)
    if wrapped then
      let rightStart := off r b.right first.n start
      let optional := if closeTo firstRight rightStart then [] else [line rightStart]
      out ++ optional ++ [tip, line (off r b.left first.n start)]
    else
      let base := off r b.left startNormal start
      let zeroFrom x y := if isZero x && isZero y then y else x
      out ++ [tip, line (zeroFrom firstLeft.1 base.1, zeroFrom firstLeft.2 base.2)]
  else if startCap == 2 then
    let cap := capArc r b start first.g true doubleRound
    if wrapped then
      let firstCap := (cap.head?.getD (line firstRight)).point
      out ++ (if closeTo firstRight firstCap then cap.drop 1 else cap)
    else out ++ setFirstPoint cap firstRight
  else out

def closedBand (d : Bool) (b : Band) (p : Array Pair) : List (List Out) :=
  let n := p.size
  if n < 2 then [] else
  let (left, right) := (List.range n).foldl (fun (l, rt) i =>
    let (a, c) := bandJoin d b (getPoint p ((i + n - 1) % n)) (getPoint p i) (getPoint p ((i + 1) % n))
    (l ++ a, rt ++ c)) ([], [])
  [firstPoint left, firstPoint (reverseSide right)]

/-- A compound pen's whole-width round or triangle cap is a separate figure. -/
def wholeCap (d : Bool) (b : Band) (p : Array Pair) (cap : Nat) (start : Bool)
    (doubleRound : Bool := false) : Option (List Out) :=
  let r := ops d
  let n := p.size
  if n < 2 then none else
  let (at_, s) := if start then (p.getD 0 (0, 0), segment d r (p.getD 0 (0, 0)) (p.getD 1 (0, 0)))
    else (p.getD (n - 1) (0, 0), segment d r (p.getD (n - 2) (0, 0)) (p.getD (n - 1) (0, 0)))
  let (a, c) := (off r b.right s.n at_, off r b.left s.n at_)
  let (first, last) := if start then (a, c) else (c, a)
  if cap == 3 then some (firstPoint [line first, line (capTip r b at_ s.g start), line last])
  else if cap == 2 then
    some (firstPoint (setFirstPoint (capArc r b at_ s.g start doubleRound) first))
  else none

def closeFigure (points : List Out) : List Out :=
  match points.reverse with
  | [] => []
  | p :: rest => ({ p with type := p.type + 128 } :: rest).reverse

def mirrorBand (d closed : Bool) (b : Band) (p : Array Pair) (startCap endCap : Nat) : List Nat :=
  let outlines := if closed then closedBand d b p
    else if p.size < 2 then [] else [openBand d b p startCap endCap]
  outlines.length :: outlines.flatMap (encoded ∘ closeFigure)

private def pointPairs : List Int → Option (List Pair)
  | [] => some []
  | x :: y :: rest => do pure ((← nat x, ← nat y) :: (← pointPairs rest))
  | [_] => none

theorem reversed_side_keeps_point_count (side : List Out) :
    (reverseSide side).length = side.length := by
  cases side <;> simp [reverseSide, List.length_zip, Nat.add_comm]

theorem closed_band_has_two_contours (d : Bool) (b : Band) (p : Array Pair) (h : 2 ≤ p.size) :
    (closedBand d b p).length = 2 := by
  simp [closedBand, Nat.not_lt.mpr h]

private def horizontalBand : Band := ⟨0x40000000, one, 0xbf800000, 0x40800000, 0, false⟩
private def horizontalPoints : Array Pair := #[(0, 0), (0x40800000, 0)]

theorem horizontal_flat_caps :
    mirrorBand false false horizontalBand horizontalPoints 0 0 =
    [1, 4, 0, 0, 0xbf800000, 1, 0x40800000, 0xbf800000, 1, 0x40800000, one, 0x81, 0, one] := by
  decide +kernel

theorem horizontal_square_caps :
    mirrorBand false false horizontalBand horizontalPoints 1 1 =
    [1, 4, 0, 0xbf800000, 0xbf800000, 1, 0x40a00000, 0xbf800000,
     1, 0x40a00000, one, 0x81, 0xbf800000, one] := by
  decide +kernel

theorem horizontal_triangle_caps :
    mirrorBand false false horizontalBand horizontalPoints 3 3 =
    [1, 7, 0, 0, 0xbf800000, 1, 0x40800000, 0xbf800000, 1, 0x40a00000, 0, 1, 0x40800000, one,
     1, 0, one, 1, 0xbf800000, 0, 0x81, 0, 0xbf800000] := by
  decide +kernel

theorem horizontal_round_caps :
    mirrorBand false false horizontalBand horizontalPoints 2 2 =
    [1, 15, 0, 0, 0xbf800000, 1, 0x40800000, 0xbf800000, 3, 0x4091ac51, 0xbf800000,
     3, 0x40a00000, 0xbf0d6289, 3, 0x40a00000, 0, 3, 0x40a00000, 0x3f0d6289,
     3, 0x4091ac51, one, 3, 0x40800000, one, 1, 0, one, 3, 0xbf0d6289, one,
     3, 0xbf800000, 0x3f0d6289, 3, 0xbf800000, 0, 3, 0xbf800000, 0xbf0d6289,
     3, 0xbf0d6289, 0xbf800000, 0x83, 0, 0xbf800000] := by
  decide +kernel

theorem flat_endpoint_signed_zero :
    mirrorBand true false horizontalBand #[(0, one), (0x40400000, one)] 0 0 =
    [1, 4, 0, 0x80000000, 0x80000000, 1, 0x40400000, 0x80000000,
      1, 0x40400000, 0x40000000, 0x81, 0x80000000, 0x40000000] := by
  decide +kernel

theorem round_cap_signed_zero :
    mirrorBand true false horizontalBand #[(0xc0400000, one), (0, one)] 2 2 =
    [1, 15, 0, 0xc0400000, 0x80000000, 1, 0x80000000, 0x80000000, 3, 0x3f0d6289, 0x80000000,
      3, one, 0x3ee53aee, 3, one, one, 3, one, 0x3fc6b144, 3, 0x3f0d6289, 0x40000000,
      3, 0x80000000, 0x40000000, 1, 0xc0400000, 0x40000000, 3, 0xc06358a3, 0x40000000,
      3, 0xc0800000, 0x3fc6b144, 3, 0xc0800000, one, 3, 0xc0800000, 0x3ee53aee,
      3, 0xc06358a3, 0x80000000, 0x83, 0xc0400000, 0x80000000] := by
  decide +kernel

theorem triangle_cap_start_keeps_unshifted_zero :
    mirrorBand true false horizontalBand #[(one, 0), (one, 0x40400000)] 3 3 =
    [1, 7, 0, 0x40000000, 0x80000000, 1, 0x40000000, 0x40400000, 1, one, 0x40800000,
      1, 0x80000000, 0x40400000, 1, 0x80000000, 0x80000000, 1, one, 0xbf800000,
      0x81, 0x40000000, 0] := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.wide-join", fun
    | [d, compound, style, wl, wr, miter, ax, ay, px, py, bx, by_] => do
      let out := mirrorJoin (d != 0) (compound != 0) style (← nat ax, ← nat ay)
        (← nat px, ← nat py) (← nat bx, ← nat by_) (← nat wl) (← nat wr) (← nat miter)
      pure (out.map Int.ofNat)
    | _ => none⟩,
  ⟨"gp.wide-band", fun
    | d :: closed :: compound :: style :: width :: wl :: wr :: miter :: start :: end_ :: rest => do
      let b : Band := ⟨← nat width, ← nat wl, ← nat wr, ← nat miter, style, compound != 0⟩
      let out := mirrorBand (d != 0) (closed != 0) b (← pointPairs rest).toArray (← nat start) (← nat end_)
      pure (out.map Int.ofNat)
    | _ => none⟩
]

end Kikizake.GdiPlus.Wide
