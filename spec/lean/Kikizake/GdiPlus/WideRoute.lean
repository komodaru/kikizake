import Kikizake.GdiPlus.Wide

/-! Compose the widening stages for a solid centered pen with ordinary caps. The
device matrix and its inverse, binary32 flattening, axes, joins and band outlines
are the same definitions their own mirrors use. -/
namespace Kikizake.GdiPlus.WideRoute

open Kikizake.Float
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Wide

structure Pen where
  width : Nat
  miter : Nat
  join : Int
  startCap : Nat
  endCap : Nat
  compound : List Nat

private def pointEq (a b : Pair) : Bool :=
  let same x y := x == y || (isZero x && isZero y)
  same a.1 b.1 && same a.2 b.2

private def dedup (p : List Pair) : List Pair :=
  (p.foldl (fun out q =>
    let closeTo prev := flt (fabs (F32.sub q.1 prev.1)) 0x34000000 &&
      flt (fabs (F32.sub q.2 prev.2)) 0x34000000
    if out.head?.any closeTo then out else q :: out) []).reverse

private def penBands (r : Ops) (width halfWidth : Nat) : List Nat → List Pair
  | a :: b :: rest =>
    if a == b || (isZero a && isZero b) then penBands r width halfWidth rest
    else (r.sub halfWidth (r.mul a width), r.sub halfWidth (r.mul b width)) :: penBands r width halfWidth rest
  | _ => []

private def pointThrough (r : Ops) (m : List Nat) (p : Pair) : Pair :=
  apply r m p.1 p.2

private def toPt (r : Ops) (minv : List Nat) (p : Nat × Nat × Nat) : Pt :=
  let (x, y) := apply r minv p.1 p.2.1
  ⟨x, y, p.2.2 % 128, p.2.2 ≥ 128⟩

private def resultPoint (r : Ops) (m minv : List Nat) (thin : Bool) (o : Out) : List Int :=
  let p := if thin then pointThrough r minv o.point else o.point
  let (x, y) := pointThrough r m p
  [o.type % 64 + o.type / 128 * 128, F32.canon x, F32.canon y]

private def closedPoints (closed : Bool) (pts : List Pair) : List Pair :=
  if closed && pts.length > 2 &&
    (match pts.head?, pts.getLast? with
     | some first, some last => pointEq first last
     | _, _ => false)
  then pts.dropLast else pts

def figureBands (d : Bool) (pen : Pen) (m : List Nat) (thin : Bool) (width : Nat)
    (closed : Bool) (fig : List Pt) : List (List Out) :=
  let r := ops d
  let points := fig.map (fun p => (p.x, p.y))
  let points := if thin then points.map (pointThrough r m) else points
  let points := (closedPoints closed (dedup points)).toArray
  if points.size < 2 then [] else
  let halfWidth := r.mul width half
  let whole : Band := ⟨width, halfWidth, r.sub halfWidth width, pen.miter, pen.join, false⟩
  let compound := pen.compound.length ≥ 4
  let bands := if pen.compound.isEmpty then [(whole.left, whole.right)] else penBands r width halfWidth pen.compound
  let apart cap := compound && (cap == 2 || cap == 3)
  let bandCap cap := if apart cap then 0 else cap
  let outlines := bands.flatMap fun (wl, wr) =>
    let b := { whole with left := wl, right := wr, compound := compound }
    if closed then closedBand d b points
    else [openBand d b points (bandCap pen.startCap) (bandCap pen.endCap)]
  if closed then outlines else
  let extra := [(pen.startCap, true), (pen.endCap, false)].filterMap fun (cap, start) =>
    if apart cap then wholeCap d whole points cap start else none
  outlines ++ extra

/-- The returned point stream starts with 0, or -1 when the transform has no inverse. -/
def centeredOutline (d : Bool) (pen : Pen) (m : List Nat) (flatness : Nat) (path : List Pt) :
    Option (List Int) := do
  let r := ops d
  match inverse r m with
  | none => pure [-1]
  | some minv =>
    let flat ← flatten r (some m) flatness path
    let (_, minor) := Stroke.axes d m
    let thin := flt (r.mul pen.width minor) 0x3f800008
    let width := if thin then 0x3f800008 else pen.width
    let all := figures (flat.map (toPt r minv))
    let outlines := all.flatMap fun f =>
      let closed := (f.getLast?.map (·.close)).getD false
      figureBands d pen m thin width closed f
    pure (0 :: outlines.flatMap (fun fig => (closeFigure fig).flatMap (resultPoint r m minv thin)))

theorem singular_matrix_leaves_no_outline (d : Bool) (pen : Pen) (m : List Nat)
    (flatness : Nat) (path : List Pt) (h : inverse (ops d) m = none) :
    centeredOutline d pen m flatness path = some [-1] := by
  simp [centeredOutline, h]

private def translation : List Nat := [one, 0, 0, one, 0x40400000, 0xc0000000]
private def linePath : List Pt := [⟨0, 0, 0, false⟩, ⟨0x40800000, 0, 1, false⟩]

theorem translated_solid_line :
    centeredOutline false ⟨0x40000000, 0x40800000, 0, 0, 0, []⟩ translation 0x3e800000 linePath =
    some [0, 0, 0x40400000, 0xc0400000, 1, 0x40e00000, 0xc0400000,
      1, 0x40e00000, 0xbf800000, 0x81, 0x40400000, 0xbf800000] := by
  decide +kernel

theorem thin_device_space_line :
    centeredOutline false ⟨0x3e800000, 0x40800000, 0, 0, 0, []⟩ translation 0x3e800000 linePath =
    some [0, 0, 0x40400000, 0xc0200002, 1, 0x40e00000, 0xc0200002,
      1, 0x40e00000, 0xbfbffffc, 0x81, 0x40400000, 0xbfbffffc] := by
  decide +kernel

theorem compound_caps_follow_bands :
    centeredOutline false ⟨0x40000000, 0x40800000, 0, 3, 3, [0, 0x3f000000, 0x3f400000, one]⟩
      translation 0x3e800000 linePath =
    some [0,
      0, 0x40400000, 0xc0400000, 1, 0x40e00000, 0xc0400000,
      1, 0x40e00000, 0xc0000000, 0x81, 0x40400000, 0xc0000000,
      0, 0x40400000, 0xbfc00000, 1, 0x40e00000, 0xbfc00000,
      1, 0x40e00000, 0xbf800000, 0x81, 0x40400000, 0xbf800000,
      0, 0x40400000, 0xbf800000, 1, 0x40000000, 0xc0000000, 0x81, 0x40400000, 0xc0400000,
      0, 0x40e00000, 0xc0400000, 1, 0x41000000, 0xc0000000, 0x81, 0x40e00000, 0xbf800000] := by
  decide +kernel

private def points : List Int → Option (List Pt)
  | [] => some []
  | x :: y :: t :: rest => do
    let t ← nat t
    pure (⟨← nat x, ← nat y, t % 128, t ≥ 128⟩ :: (← points rest))
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.wide-centered", fun
    | d :: width :: miter :: join :: sc :: ec :: flatness :: a :: b :: c :: e :: x :: y :: count :: rest => do
      let n ← nat count
      let _ ← if n % 2 == 0 && n ≤ rest.length then some () else none
      let compound ← (rest.take n).mapM nat
      let pen : Pen := ⟨← nat width, ← nat miter, join, ← nat sc, ← nat ec, compound⟩
      centeredOutline (d != 0) pen (← [a, b, c, e, x, y].mapM nat) (← nat flatness)
        (← points (rest.drop n))
    | _ => none⟩
]

end Kikizake.GdiPlus.WideRoute
