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

def mirrors : List Mirror := [
  ⟨"gp.wide-join", fun
    | [d, compound, style, wl, wr, miter, ax, ay, px, py, bx, by_] => do
      let out := mirrorJoin (d != 0) (compound != 0) style (← nat ax, ← nat ay)
        (← nat px, ← nat py) (← nat bx, ← nat by_) (← nat wl) (← nat wr) (← nat miter)
      pure (out.map Int.ofNat)
    | _ => none⟩
]

end Kikizake.GdiPlus.Wide
