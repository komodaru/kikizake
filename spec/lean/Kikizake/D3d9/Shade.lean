import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's shading (spec/d3d9draw.txt D3D9.SHADE.GOURAUD, SHADE.FLAT) and the strip a
clipped polygon is cut into (D3D9.RASTER.CLIP), in binary32 (Kikizake.F32) as
src/model/d3d9draw.rs `grad9`, `at9`, `plane10`, `shaded` and `strip` compute them (mirrors
`d3d9.grad`, `d3d9.at`, `d3d9.plane`, `d3d9.shaded`, `d3d9.strip`). -/
namespace Kikizake.D3d9.Shade

open Kikizake.F32 (add sub mul div)

def one : Nat := 0x3f800000

/-- the doubled area at the pixel centers x, y: (x1 - x0)(y2 - y0) - (y1 - y0)(x2 - x0) -/
def area (x0 x1 x2 y0 y1 y2 : Nat) : Nat := sub (mul (sub x1 x0) (sub y2 y0)) (mul (sub y1 y0) (sub x2 x0))

/-- D3D9.SHADE.GOURAUD: an attribute's gradients d/dx = ((a1 - a0)(y2 - y0) - (a2 - a0)(y1 - y0)) /
area, d/dy = ((a2 - a0)(x1 - x0) - (a1 - a0)(x2 - x0)) / area -/
def grad (x0 x1 x2 y0 y1 y2 a0 a1 a2 : Nat) : Nat × Nat :=
  let A := area x0 x1 x2 y0 y1 y2
  let d1 := sub a1 a0
  let d2 := sub a2 a0
  (div (sub (mul d1 (sub y2 y0)) (mul d2 (sub y1 y0))) A, div (sub (mul d2 (sub x1 x0)) (mul d1 (sub x2 x0))) A)

/-- D3D9.SHADE.GOURAUD: the attribute at the pixel center, v0 + d/dy (y - y0) + d/dx (x - x0) in
that order -/
def at9 (a0 : Nat) (g : Nat × Nat) (x0 y0 sx sy : Nat) : Nat := add (add a0 (mul g.2 (sub sy y0))) (mul g.1 (sub sx x0))

/-- Direct3D 10's plane: gx, gy times the area's reciprocal, C = (a0 - gx x0) - gy y0, the value
(C + gy cy) + gx cx -/
def plane (x0 x1 x2 y0 y1 y2 a0 a1 a2 cx cy : Nat) : Nat :=
  let r := div one (area x0 x1 x2 y0 y1 y2)
  let d1 := sub a1 a0
  let d2 := sub a2 a0
  let gx := mul (sub (mul d1 (sub y2 y0)) (mul d2 (sub y1 y0))) r
  let gy := mul (sub (mul d2 (sub x1 x0)) (mul d1 (sub x2 x0))) r
  add (add (sub (sub a0 (mul gx x0)) (mul gy y0)) (mul gy cy)) (mul gx cx)

/-- D3D9.SHADE.GOURAUD: the gradients and the value are the clause's formulas, term for term -/
theorem gouraud (x0 x1 x2 y0 y1 y2 a0 a1 a2 sx sy : Nat) :
    at9 a0 (grad x0 x1 x2 y0 y1 y2 a0 a1 a2) x0 y0 sx sy =
      add (add a0 (mul (div (sub (mul (sub a2 a0) (sub x1 x0)) (mul (sub a1 a0) (sub x2 x0)))
        (sub (mul (sub x1 x0) (sub y2 y0)) (mul (sub y1 y0) (sub x2 x0)))) (sub sy y0)))
        (mul (div (sub (mul (sub a1 a0) (sub y2 y0)) (mul (sub a2 a0) (sub y1 y0)))
        (sub (mul (sub x1 x0) (sub y2 y0)) (mul (sub y1 y0) (sub x2 x0)))) (sub sx x0)) := rfl

/-- D3D9.SHADE.FLAT, GOURAUD: the colors a pixel takes, the first vertex's with flat shading -/
def shaded {α : Type} (flat : Bool) (first interpolated : α) : α := if flat then first else interpolated

theorem flat {α : Type} (first interpolated : α) : shaded true first interpolated = first := rfl
theorem gouraud_colors {α : Type} (first interpolated : α) : shaded false first interpolated = interpolated := rfl

/-- D3D9.RASTER.CLIP: a convex polygon of n vertices cut into the strip v1 v2 v0 v3 v(n-1) v4 ...
(the order 0, 1, n-1, 2, n-2, ... turned by one) -/
def order (n : Nat) : List Nat :=
  let rec go (lo hi fuel : Nat) : List Nat :=
    match fuel with
    | 0 => []
    | f + 1 => if lo ≤ hi then (if lo = hi then [lo] else [lo, hi]) ++ go (lo + 1) (hi - 1) f else []
  (0 :: go 1 (n - 1) n).map (fun k => (k + 1) % (max n 1))

def windows3 : List Nat → List (List Nat)
  | a :: b :: c :: r => [a, b, c] :: windows3 (b :: c :: r)
  | _ => []

def strip (n : Nat) : List (List Nat) := windows3 (order n)

/-- D3D9.RASTER.CLIP: the polygons clipping makes (3 to 9 vertices) are cut into n - 2 triangles
of their own vertices, each sharing an edge with the one before, every vertex used -/
theorem strip_cuts : ∀ n ∈ List.range' 3 7,
    (strip n).length = n - 2 ∧ (strip n).all (fun t => t.all (· < n)) ∧
    (List.range n).all (fun k => (strip n).any (·.contains k)) := by decide

theorem strip_examples : strip 3 = [[1, 2, 0]] ∧ strip 4 = [[1, 2, 0], [2, 0, 3]] ∧
    strip 5 = [[1, 2, 0], [2, 0, 3], [0, 3, 4]] ∧
    strip 6 = [[1, 2, 0], [2, 0, 3], [0, 3, 5], [3, 5, 4]] := by decide

def mirrors : List Mirror := [
  ⟨"d3d9.grad", fun
    | [x0, x1, x2, y0, y1, y2, a0, a1, a2] => do
      let g := grad (← nat x0) (← nat x1) (← nat x2) (← nat y0) (← nat y1) (← nat y2) (← nat a0) (← nat a1) (← nat a2)
      pure [F32.canon g.1, F32.canon g.2]
    | _ => none⟩,
  ⟨"d3d9.at", fun
    | [a0, gx, gy, x0, y0, sx, sy] => do
      pure [F32.canon (at9 (← nat a0) (← nat gx, ← nat gy) (← nat x0) (← nat y0) (← nat sx) (← nat sy))]
    | _ => none⟩,
  ⟨"d3d9.plane", fun
    | [x0, x1, x2, y0, y1, y2, a0, a1, a2, cx, cy] => do
      pure [F32.canon (plane (← nat x0) (← nat x1) (← nat x2) (← nat y0) (← nat y1) (← nat y2) (← nat a0) (← nat a1) (← nat a2) (← nat cx) (← nat cy))]
    | _ => none⟩,
  ⟨"d3d9.shaded", fun
    | [fl, a, b] => pure [shaded (fl != 0) a b]
    | _ => none⟩,
  ⟨"d3d9.strip", fun
    | [n] => do pure ((strip (← nat n)).flatten.map (fun k : Nat => (k : Int)))
    | _ => none⟩
]

end Kikizake.D3d9.Shade
