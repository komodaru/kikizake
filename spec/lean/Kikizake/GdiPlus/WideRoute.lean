import Kikizake.GdiPlus.Dash
import Kikizake.GdiPlus.Wide

/-! Compose the widening stages for a centered pen with ordinary caps. The device
matrix and its inverse, binary32 flattening, axes, dash walker, joins and band
outlines are the same definitions their own mirrors use. -/
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

structure DashStyle where
  style : Int
  cap : Int
  offset : Nat
  custom : List Nat

private structure Subpath where
  points : List Pair
  closed : Bool
  startCap : Nat
  endCap : Nat

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

private def resultPoint (r : Ops) (m minv : List Nat) (thin : Bool) (o : Out) : Pt :=
  let p := if thin then pointThrough r minv o.point else o.point
  let (x, y) := pointThrough r m p
  ⟨x, y, o.type % 64, o.type ≥ 128⟩

def encodePoints (pts : List Pt) : List Int :=
  pts.flatMap fun p => [p.t + (if p.close then 128 else 0), F32.canon p.x, F32.canon p.y]

private def closedPoints (closed : Bool) (pts : List Pair) : List Pair :=
  if closed && pts.length > 2 &&
    (match pts.head?, pts.getLast? with
     | some first, some last => pointEq first last
     | _, _ => false)
  then pts.dropLast else pts

private def figureBands (d : Bool) (pen : Pen) (m : List Nat) (thin : Bool) (width : Nat)
    (doubleRound : Bool) (sub : Subpath) : List (List Out) :=
  let r := ops d
  let points := if thin then sub.points.map (pointThrough r m) else sub.points
  let points := (closedPoints sub.closed (dedup points)).toArray
  if points.size < 2 then [] else
  let halfWidth := r.mul width half
  let whole : Band := ⟨width, halfWidth, r.sub halfWidth width, pen.miter, pen.join, false⟩
  let compound := pen.compound.length ≥ 4
  let bands := if pen.compound.isEmpty then [(whole.left, whole.right)] else penBands r width halfWidth pen.compound
  let apart cap := compound && (cap == 2 || cap == 3)
  let bandCap cap := if apart cap then 0 else cap
  let outlines := bands.flatMap fun (wl, wr) =>
    let b := { whole with left := wl, right := wr, compound := compound }
    if sub.closed then closedBand d b points
    else [openBand d b points (bandCap sub.startCap) (bandCap sub.endCap) doubleRound]
  if sub.closed then outlines else
  let extra := [(sub.startCap, true), (sub.endCap, false)].filterMap fun (cap, start) =>
    if apart cap then wholeCap d whole points cap start doubleRound else none
  outlines ++ extra

/-- Flatten in device space and return the figure points to world space. -/
def flattenedWorld (d : Bool) (m minv : List Nat) (flatness : Nat) (path : List Pt) : Option (List Pt) := do
  let r := ops d
  let flat ← flatten r (some m) flatness path
  pure (flat.map (toPt r minv))

private def outlineSubpaths (d : Bool) (pen : Pen) (m minv : List Nat) (minor minimum : Nat)
    (doubleRound : Bool) (subs : List Subpath) : List Pt :=
  let r := ops d
  let thin := flt (r.mul pen.width minor) minimum
  let width := if thin then minimum else pen.width
  let outlines := subs.flatMap (figureBands d pen m thin width doubleRound)
  outlines.flatMap fun fig => (closeFigure fig).map (resultPoint r m minv thin)

private def worldSubpaths (pen : Pen) (world : List Pt) : List Subpath :=
  (figures world).map fun f =>
    ⟨f.map (fun p => (p.x, p.y)), (f.getLast?.map (·.close)).getD false, pen.startCap, pen.endCap⟩

def centeredFromWorld (d : Bool) (pen : Pen) (m minv : List Nat) (world : List Pt) : List Pt :=
  outlineSubpaths d pen m minv (Stroke.axes d m).2 0x3f800008 false (worldSubpaths pen world)

def dashPattern (dash : DashStyle) : Option (List Nat) :=
  match dash.style with
  | 1 => some [0x40400000, one]
  | 2 => some [one, one]
  | 3 => some [0x40400000, one, one, one]
  | 4 => some [0x40400000, one, one, one, one, one]
  | 5 => if dash.custom.isEmpty then none else some dash.custom
  | _ => none

private def flatDashCap (dash : DashStyle) : Bool :=
  dash.cap != 2 && dash.cap != 3

private def dashedWith (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m minv : List Nat)
    (world : List Pt) (inset : Bool) : Option (List Pt) := do
  let some pattern := dashPattern dash | pure (centeredFromWorld d pen m minv world)
  let r := ops d
  let (major, minor) := Stroke.axes d m
  let cap := if flatDashCap dash then 0 else dash.cap.toNat
  let width := if inset then r.add pen.width pen.width else pen.width
  let unit := Dash.dashUnit r pen.width major (if inset then 0x40000000 else one)
  let (lengths, gap) := Dash.patternAtUnit r unit (cap != 0) pattern
  let spans ← (worldSubpaths pen world).mapM fun sub => do
    let points := sub.points.map (pointThrough r minv ∘ pointThrough r m)
    let off := r.sub (r.mul width dash.offset) (if sub.closed then gap else 0)
    Dash.walk fuel d sub.closed points.toArray lengths off pen.width cap sub.startCap sub.endCap
  let capStyle style := if inset && cap == 3 then 0 else style
  let subs := spans.flatten.map fun s => ⟨s.points.toList, false, capStyle s.startCap, capStyle s.endCap⟩
  let minimum := if inset then 0x40800000 else 0x3f800008
  pure (outlineSubpaths d { pen with width } m minv minor minimum inset subs)

def dashedFromWorld (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m minv : List Nat)
    (world : List Pt) : Option (List Pt) :=
  dashedWith fuel d pen dash m minv world false

/-- Closed inset dashes use a two-pixel dash unit minimum and a four-pixel outline minimum.
The outline and offset use the doubled pen width, and round caps split into two half-width arcs. -/
def insetDashFromWorld (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m minv : List Nat)
    (world : List Pt) : Option (List Pt) :=
  dashedWith fuel d pen dash m minv world true

def centeredPath (d : Bool) (pen : Pen) (m minv : List Nat) (flatness : Nat) (path : List Pt) : Option (List Pt) := do
  pure (centeredFromWorld d pen m minv (← flattenedWorld d m minv flatness path))

/-- The returned point stream starts with 0, or -1 when the transform has no inverse. -/
def centeredOutline (d : Bool) (pen : Pen) (m : List Nat) (flatness : Nat) (path : List Pt) :
    Option (List Int) := do
  match inverse (ops d) m with
  | none => pure [-1]
  | some minv => pure (0 :: encodePoints (← centeredPath d pen m minv flatness path))

def dashedOutline (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m : List Nat)
    (flatness : Nat) (path : List Pt) : Option (List Int) := do
  match inverse (ops d) m with
  | none => pure [-1]
  | some minv =>
    let world ← flattenedWorld d m minv flatness path
    let out ← dashedFromWorld fuel d pen dash m minv world
    pure (0 :: encodePoints out)

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

theorem no_pattern_matches_solid (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m : List Nat)
    (flatness : Nat) (path : List Pt) (h : dashPattern dash = none) :
    dashedOutline fuel d pen dash m flatness path = centeredOutline d pen m flatness path := by
  simp [dashedOutline, dashedFromWorld, dashedWith, centeredOutline, centeredPath, h, Option.pure_def,
    Option.bind_assoc]

private def dashedLine : List Pt := [⟨0, 0, 0, false⟩, ⟨0x40c00000, 0, 1, false⟩]

theorem dashed_line_widens_each_span :
    dashedOutline 16 false ⟨0x40000000, 0x40800000, 0, 0, 0, []⟩ ⟨2, 0, 0, []⟩
      translation 0x3e800000 dashedLine =
    some [0, 0, 0x40400000, 0xc0400000, 1, 0x40a00000, 0xc0400000,
      1, 0x40a00000, 0xbf800000, 0x81, 0x40400000, 0xbf800000,
      0, 0x40e00000, 0xc0400000, 1, 0x41100000, 0xc0400000,
      1, 0x41100000, 0xbf800000, 0x81, 0x40e00000, 0xbf800000] := by
  decide +kernel

theorem dashed_line_rounds_down :
    dashedOutline 16 true ⟨0x40000000, 0x40800000, 0, 0, 0, []⟩ ⟨2, 0, 0, []⟩
      translation 0x3e800000 dashedLine =
    some [0, 0, 0x40400000, 0xc0400000, 1, 0x409fffff, 0xc0400000,
      1, 0x409fffff, 0xbf800000, 0x81, 0x40400000, 0xbf800000,
      0, 0x40dfffff, 0xc0400001, 1, 0x41100000, 0xc0400001,
      1, 0x41100000, 0xbf7ffffe, 0x81, 0x40dfffff, 0xbf7ffffe] := by
  decide +kernel

def points : List Int → Option (List Pt)
  | [] => some []
  | x :: y :: t :: rest => do
    let t ← nat t
    pure (⟨← nat x, ← nat y, t % 128, t ≥ 128⟩ :: (← points rest))
  | _ => none

def routeArgs : List Int → Option (Pen × List Nat × Nat × List Pt)
  | width :: miter :: join :: sc :: ec :: flatness :: a :: b :: c :: e :: x :: y :: count :: rest => do
    let n ← nat count
    let _ ← if n % 2 == 0 && n ≤ rest.length then some () else none
    let compound ← (rest.take n).mapM nat
    let pen : Pen := ⟨← nat width, ← nat miter, join, ← nat sc, ← nat ec, compound⟩
    pure (pen, ← [a, b, c, e, x, y].mapM nat, ← nat flatness, ← points (rest.drop n))
  | _ => none

def dashArgs : List Int → Option (DashStyle × List Int)
  | style :: cap :: offset :: count :: rest => do
    let n ← nat count
    let _ ← if n ≤ rest.length then some () else none
    pure (⟨style, cap, ← nat offset, ← (rest.take n).mapM nat⟩, rest.drop n)
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.wide-centered", fun
    | d :: rest => do
      let (pen, m, flatness, path) ← routeArgs rest
      centeredOutline (d != 0) pen m flatness path
    | _ => none⟩,
  ⟨"gp.wide-dashed", fun
    | d :: rest => do
      let (dash, rest) ← dashArgs rest
      let (pen, m, flatness, path) ← routeArgs rest
      dashedOutline 8192 (d != 0) pen dash m flatness path
    | _ => none⟩
]

end Kikizake.GdiPlus.WideRoute
