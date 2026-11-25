import Kikizake.GdiPlus.Inset
import Kikizake.GdiPlus.Raster
import Kikizake.GdiPlus.WideRoute

/-! Inset widening clips the solid or dashed outline of each closed figure to the whole
path mask. Open figures use ordinary centered widening. -/
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

private def insetWith (fuel : Nat) (d : Bool) (pen : Pen) (dash : Option DashStyle) (m : List Nat)
    (flatness : Nat) (winding : Bool) (path : List Pt) : Option (List Int) := do
  let r := ops d
  match inverse r m with
  | none => pure [-1]
  | some minv =>
    let outline p world :=
      match dash with
      | none => pure (centeredFromWorld d p m minv world)
      | some ds => dashedFromWorld fuel d p ds m minv world
    let world ← flattenedWorld d m minv flatness path
    if !hasClosedFigure world then
      pure (0 :: encodePoints (← outline pen world))
    else
      let whole ← deviceMask r m winding path
      let double := { pen with width := r.add pen.width pen.width }
      let outlines ← (figures path).mapM fun f => do
        let world ← flattenedWorld d m minv flatness f
        if figClosed f then do
          let border ← match dash with
            | some ds =>
              if (dashPattern ds).isSome then
                insetDashFromWorld fuel d pen ds m minv world
              else outline double world
            | none => outline double world
          let own ← deviceMask r identity true border
          pure (regionPoints (← insetRegion own whole))
        else outline pen world
      pure (0 :: encodePoints outlines.flatten)

def insetOutline (d : Bool) (pen : Pen) (m : List Nat) (flatness : Nat) (winding : Bool)
    (path : List Pt) : Option (List Int) :=
  insetWith 0 d pen none m flatness winding path

def dashedInsetOutline (fuel : Nat) (d : Bool) (pen : Pen) (dash : DashStyle) (m : List Nat)
    (flatness : Nat) (winding : Bool) (path : List Pt) : Option (List Int) :=
  insetWith fuel d pen (some dash) m flatness winding path

theorem open_figures_keep_the_centered_route (d : Bool) (pen : Pen) (m minv : List Nat)
    (flatness : Nat) (winding : Bool) (path world : List Pt)
    (hinv : inverse (ops d) m = some minv) (hworld : flattenedWorld d m minv flatness path = some world)
    (hopen : hasClosedFigure world = false) :
    insetOutline d pen m flatness winding path = centeredOutline d pen m flatness path := by
  simp [insetOutline, insetWith, centeredOutline, centeredPath, hinv, hworld, hopen, Option.pure_def]

theorem open_dashed_figures_keep_the_centered_route (fuel : Nat) (d : Bool) (pen : Pen)
    (dash : DashStyle) (m minv : List Nat) (flatness : Nat) (winding : Bool) (path world : List Pt)
    (hinv : inverse (ops d) m = some minv) (hworld : flattenedWorld d m minv flatness path = some world)
    (hopen : hasClosedFigure world = false) :
    dashedInsetOutline fuel d pen dash m flatness winding path =
      dashedOutline fuel d pen dash m flatness path := by
  simp [dashedInsetOutline, insetWith, dashedOutline, hinv, hworld, hopen]

private def examplePen : Pen := ⟨one, 0x40800000, 0, 0, 0, []⟩
private def square : List Pt :=
  [⟨0, 0, 0, false⟩, ⟨0x40c00000, 0, 1, false⟩,
   ⟨0x40c00000, 0x40c00000, 1, false⟩, ⟨0, 0x40c00000, 1, true⟩]
private def nestedSquare : List Pt :=
  [⟨0x40000000, 0x40000000, 0, false⟩, ⟨0x40800000, 0x40000000, 1, false⟩,
   ⟨0x40800000, 0x40800000, 1, false⟩, ⟨0x40000000, 0x40800000, 1, true⟩]
private def openLine : List Pt := [⟨0xc0800000, 0x40000000, 0, false⟩, ⟨0xbf800000, 0x40000000, 1, false⟩]

private def squareRing : List Int :=
  [0, 0, 0, 1, 0, 0x40c00000, 1, 0x40c00000, 0x40c00000, 0x81, 0x40c00000, 0,
    0, 0x40a00000, 0x3f800000, 1, 0x40a00000, 0x40a00000,
    1, 0x3f800000, 0x40a00000, 0x81, 0x3f800000, 0x3f800000]

theorem square_inset_forms_a_ring :
    insetOutline false examplePen identity 0x3e800000 false square = some (0 :: squareRing) := by
  decide +kernel

/-- The centered and clipped contours follow the two source figures in either order. -/
theorem mixed_path_keeps_figure_order :
    let centered : List Int := [0, 0xc0800000, 0x3fbffffc, 1, 0xbf800000, 0x3fbffffc,
      1, 0xbf800000, 0x40200002, 0x81, 0xc0800000, 0x40200002]
    insetOutline false examplePen identity 0x3e800000 false (openLine ++ square) =
      some (0 :: (centered ++ squareRing)) ∧
    insetOutline false examplePen identity 0x3e800000 false (square ++ openLine) =
      some (0 :: (squareRing ++ centered)) := by
  decide +kernel

/-- Two nested closed figures with the same winding can change the inset contours. -/
theorem nested_fill_mode_changes_contours :
    (insetOutline false examplePen identity 0x3e800000 false (square ++ nestedSquare)).map List.length = some 49 ∧
    (insetOutline false examplePen identity 0x3e800000 true (square ++ nestedSquare)).map List.length = some 37 := by
  decide +kernel

/-- The measured unit-width dashed square has one concave contour under flat dash caps. -/
theorem flat_dashed_square :
    dashedInsetOutline 64 false examplePen ⟨1, 0, 0, []⟩ identity 0x3e800000 false square =
    some [0, 0, 0, 0, 1, 0, 0x40c00000, 1, 0x40c00000, 0x40c00000,
      1, 0x40c00000, 0x40800000, 1, 0x40000000, 0x40800000, 1, 0x40000000, 0x40000000,
      1, 0x40c00000, 0x40000000, 0x81, 0x40c00000, 0] := by
  decide +kernel

theorem round_dashed_square :
    dashedInsetOutline 64 false examplePen ⟨1, 2, 0, []⟩ identity 0x3e800000 false square =
    some [0, 0, 0, 0, 1, 0, 0x40a00000, 1, 0x3f800000, 0x40a00000,
      1, 0x40c00000, 0x40c00000, 1, 0x40c00000, 0x40800000, 1, 0x40000000, 0x40800000,
      0x81, 0x40000000, 0, 0, 0x40c00000, 0, 1, 0x40c00000, 0x40400000,
      1, 0x40800000, 0x40000000, 1, 0x40400000, 0x3f800000, 129, 0x40800000, 0] := by
  decide +kernel

theorem triangular_dashed_square :
    dashedInsetOutline 64 false examplePen ⟨1, 3, 0, []⟩ identity 0x3e800000 false square =
    some [0, 0, 0, 0, 1, 0, 0x40800000, 1, 0x40000000, 0x40800000,
      0x81, 0x40000000, 0, 0, 0x40800000, 0, 1, 0x40800000, 0x40000000,
      1, 0x40c00000, 0x40000000, 129, 0x40c00000, 0, 0, 0x40000000, 0x40800000,
      1, 0x40000000, 0x40c00000, 1, 0x40c00000, 0x40c00000, 129, 0x40c00000, 0x40800000] := by
  decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.inset-path", fun
    | d :: fill :: rest => do
      let (pen, m, flatness, path) ← routeArgs rest
      insetOutline (d != 0) pen m flatness (fill != 0) path
    | _ => none⟩,
  ⟨"gp.inset-dashed", fun
    | fill :: d :: rest => do
      let (dash, rest) ← dashArgs rest
      let (pen, m, flatness, path) ← routeArgs rest
      dashedInsetOutline 8192 (d != 0) pen dash m flatness (fill != 0) path
    | _ => none⟩
]

end Kikizake.GdiPlus.InsetRoute
