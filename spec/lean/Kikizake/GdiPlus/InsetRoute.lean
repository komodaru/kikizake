import Kikizake.GdiPlus.Inset
import Kikizake.GdiPlus.Raster
import Kikizake.GdiPlus.WideRoute

/-! The solid inset route: per-figure double-width centered outlines, aliased
device masks, RegionToPath, then centered open figures with the original pen. -/
namespace Kikizake.GdiPlus.InsetRoute

open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Inset
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.WideRoute

private def identity : List Nat := [one, 0, 0, one, 0, 0]

private def figClosed (f : List Pt) : Bool := (f.getLast?.map (·.close)).getD false

def hasClosedFigure (path : List Pt) : Bool := (figures path).any figClosed

/-- Render aliased device pixels using the path's integer bounding grid. -/
def deviceMask (r : Ops) (m : List Nat) (winding : Bool) (path : List Pt) : Option Mask := do
  let m16 := m.map (r.mul · k16)
  let figs ← (figures path).mapM (enumFig r m16)
  match figs.flatten with
  | [] => pure ⟨(0, 0), 0, 0, 0⟩
  | p :: rest =>
    let (lo, hi) := rest.foldl (fun (lo, hi) (x, y) =>
      ((min lo.1 x, min lo.2 y), (max hi.1 x, max hi.2 y))) (p, p)
    let (x0, y0) := (lo.1 / 16 - 1, lo.2 / 16 - 1)
    let w := (hi.1 / 16 + 2 - x0).toNat
    let h := (hi.2 / 16 + 2 - y0).toNat
    let moved := figs.map (·.map fun (x, y) => (i32 (x - x0 * 16), i32 (y - y0 * 16)))
    let cover := Raster.rasterize moved winding none w h
    let bits := cover.toList.zipIdx.foldl (fun acc (c, i) => if c == 0 then acc else acc + (1 <<< i)) 0
    pure ⟨(x0, y0), w, h, bits⟩

private def regionPoints (contours : List (List Cell)) : List Pt :=
  contours.flatMap fun f => f.zipIdx.map fun ((x, y), i) =>
    ⟨ofInt x, ofInt y, if i == 0 then 0 else 1, i + 1 == f.length⟩

def insetOutline (d : Bool) (pen : Pen) (m : List Nat) (flatness : Nat) (winding : Bool)
    (path : List Pt) : Option (List Int) := do
  let r := ops d
  match inverse r m with
  | none => pure [-1]
  | some minv =>
    let world ← flattenedWorld d m minv flatness path
    if !hasClosedFigure world then
      pure (0 :: encodePoints (centeredFromWorld d pen m minv world))
    else
      let whole ← deviceMask r m winding path
      let figs := figures path
      let double := { pen with width := r.add pen.width pen.width }
      let closed ← (figs.filter figClosed).mapM fun f => do
        let border ← centeredPath d double m minv flatness f
        let own ← deviceMask r identity true border
        pure (regionPoints (← insetRegion own whole))
      let open_ := (figs.filter (!figClosed ·)).flatten
      let center ← centeredPath d pen m minv flatness open_
      pure (0 :: encodePoints (closed.flatten ++ center))

theorem open_figures_keep_the_centered_route (d : Bool) (pen : Pen) (m minv : List Nat)
    (flatness : Nat) (winding : Bool) (path world : List Pt)
    (hinv : inverse (ops d) m = some minv) (hworld : flattenedWorld d m minv flatness path = some world)
    (hopen : hasClosedFigure world = false) :
    insetOutline d pen m flatness winding path = centeredOutline d pen m flatness path := by
  simp [insetOutline, centeredOutline, centeredPath, hinv, hworld, hopen]

private def examplePen : Pen := ⟨one, 0x40800000, 0, 0, 0, []⟩
private def square : List Pt :=
  [⟨0, 0, 0, false⟩, ⟨0x40c00000, 0, 1, false⟩,
   ⟨0x40c00000, 0x40c00000, 1, false⟩, ⟨0, 0x40c00000, 1, true⟩]
private def nestedSquare : List Pt :=
  [⟨0x40000000, 0x40000000, 0, false⟩, ⟨0x40800000, 0x40000000, 1, false⟩,
   ⟨0x40800000, 0x40800000, 1, false⟩, ⟨0x40000000, 0x40800000, 1, true⟩]
private def openLine : List Pt := [⟨0xc0800000, 0x40000000, 0, false⟩, ⟨0xbf800000, 0x40000000, 1, false⟩]

theorem square_inset_forms_a_ring :
    insetOutline false examplePen identity 0x3e800000 false square =
      some [0, 0, 0, 0, 1, 0, 0x40c00000, 1, 0x40c00000, 0x40c00000, 0x81, 0x40c00000, 0,
        0, 0x40a00000, 0x3f800000, 1, 0x40a00000, 0x40a00000,
        1, 0x3f800000, 0x40a00000, 0x81, 0x3f800000, 0x3f800000] := by
  decide +kernel

/-- A separate open segment is centered and appended after the clipped square. -/
theorem mixed_path_appends_open_outline :
    insetOutline false examplePen identity 0x3e800000 false (openLine ++ square) =
      (do
        let clipped ← insetOutline false examplePen identity 0x3e800000 false square
        let centered ← centeredOutline false examplePen identity 0x3e800000 openLine
        pure (clipped ++ centered.drop 1)) := by
  decide +kernel

/-- Two nested closed figures with the same winding can change the inset contours. -/
theorem nested_fill_mode_changes_contours :
    (insetOutline false examplePen identity 0x3e800000 false (square ++ nestedSquare)).map List.length = some 49 ∧
    (insetOutline false examplePen identity 0x3e800000 true (square ++ nestedSquare)).map List.length = some 37 := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.inset-path", fun
    | d :: fill :: width :: miter :: join :: sc :: ec :: flatness :: a :: b :: c :: e :: x :: y ::
          count :: rest => do
      let n ← nat count
      let _ ← if n % 2 == 0 && n ≤ rest.length then some () else none
      let compound ← (rest.take n).mapM nat
      let pen : Pen := ⟨← nat width, ← nat miter, join, ← nat sc, ← nat ec, compound⟩
      insetOutline (d != 0) pen (← [a, b, c, e, x, y].mapM nat) (← nat flatness) (fill != 0)
        (← WideRoute.points (rest.drop n))
    | _ => none⟩
]

end Kikizake.GdiPlus.InsetRoute
