import Kikizake.Mirror
import Kikizake.D3d9.Pixel
import Kikizake.D3d9.Raster
/-! The integer rules of drawing on Direct3D 10 and 11 (spec/d3dres.txt D3DRASTER.*), as
src/model/d3drules.rs computes them: `covers` is `covers` (the top-left rule in 1/256 pixel, the
same rule as Direct3D 9's), `culls` `culls` (D3D11_CULL_MODE), `scissorHolds` `scissor_holds`,
`pointPixel` `point_pixel`, `pointCovers` `point_covers`, `pattern` `sample_pattern`
(D3D11_STANDARD_MULTISAMPLE_PATTERN), `mask` `sample_mask` (mirrors `d3d11.covers`, `d3d11.cull`,
`d3d11.scissor`, `d3d11.point`, `d3d11.pointms`, `d3d11.pattern`, `d3d11.mask`). -/
namespace Kikizake.D3d11.Raster

open Kikizake.D3d9.Raster (Pt area incl)

/-- D3DRASTER.TOPLEFT, DEGENERATE: the triangle draws s (1/256 pixel units, y down), turned to
wind with a positive area; nothing for no area -/
def covers (a b c s : Pt) : Bool := D3d9.Raster.covers a b c s

/-- D3DRASTER.TOPLEFT: two triangles that share the edge a b from opposite sides never both draw
a point, and where their other edges hold it exactly one does: no pixel twice, none missed -/
theorem shared_edge (a b c d s : Pt) (hc : area a b c > 0) (hd : area b a d > 0) :
    !(covers a b c s && covers b a d s) ∧
    (incl b c s = true → incl c a s = true → incl a d s = true → incl d b s = true →
      covers a b c s = !covers b a d s) :=
  D3d9.Raster.shared_edge a b c d s hc hd

/-- D3DRASTER.TOPLEFT, CULL: a triangle draws the same points whichever vertex comes first and
whichever way it winds (the winding decides culling alone) -/
theorem covers_turn (a b c s : Pt) : covers b c a s = covers a b c s ∧ covers a c b s = covers a b c s :=
  D3d9.Raster.covers_turn a b c s

/-- D3DRASTER.DEGENERATE: a triangle of zero area draws nothing -/
theorem degenerate (a b c s : Pt) (h : area a b c = 0) : covers a b c s = false := by
  simp [covers, D3d9.Raster.covers, h]

/-- A triangle faces front when its screen winding agrees with the rasterizer state. -/
def frontFace (ccw cw : Bool) : Bool := cw != ccw

/-- D3DRASTER.CULL: whether CullMode (NONE 1, FRONT 2, BACK 3) culls a triangle clockwise on the
screen (cw) or not, FrontCounterClockwise (ccw) or not -/
def culls (mode : Nat) (ccw cw : Bool) : Bool :=
  let front := frontFace ccw cw
  (mode == 2 && front) || (mode == 3 && !front)

/-- D3DRASTER.CULL: clockwise triangles face front unless FrontCounterClockwise; NONE culls
nothing, FRONT the front faces, BACK the others, so FRONT and BACK cull complements -/
theorem cull_modes (ccw cw : Bool) :
    culls 1 ccw cw = false ∧ culls 2 ccw cw = (cw != ccw) ∧ culls 3 ccw cw = !(cw != ccw) ∧
    culls 2 false true = true ∧ culls 3 true true = true ∧ culls 2 ccw cw = !culls 3 ccw cw := by
  cases ccw <;> cases cw <;> decide

/-- Culling front triangles uses the same facing value exposed to the pixel shader. -/
theorem cull_front_face (ccw cw : Bool) : culls 2 ccw cw = frontFace ccw cw := by
  cases ccw <;> cases cw <;> decide

/-- D3DRASTER.SCISSOR: pixel (px, py) inside the scissor rectangle l t r b when ScissorEnable -/
def scissorHolds (on : Bool) (l t r b px py : Int) : Bool :=
  !on || (decide (l ≤ px) && decide (px < r) && decide (t ≤ py) && decide (py < b))

/-- D3DRASTER.SCISSOR: off, every pixel; on, exactly the pixels from left and top up to (not
including) right and bottom -/
theorem scissor_rect (l t r b px py : Int) :
    scissorHolds false l t r b px py = true ∧
    (scissorHolds true l t r b px py = true ↔ l ≤ px ∧ px < r ∧ t ≤ py ∧ py < b) := by
  simp [scissorHolds, and_assoc]

/-- D3DRASTER.POINT: the pixel a point snapped at s (one axis, 1/256 pixel) lights -/
def pointPixel (s : Int) : Int := (s - 1) / 256

/-- D3DRASTER.POINT: the pixel is ceil(x) - 1 of the snapped x = s / 256: the one whose span
(256 p, 256 p + 256] holds s -/
theorem point_pixel (s : Int) : 256 * pointPixel s < s ∧ s ≤ 256 * pointPixel s + 256 := by
  simp only [pointPixel]
  omega

/-- a sample of pixel p, o/16 pixel off its center, in 1/256 pixel -/
def sampleSub (p o : Int) : Int := p * 256 + 128 + o * 16

/-- D3DRASTER.MS.POINTS: a sample inside the pixel square about a point snapped at s, its left
and top edges in, its right and bottom ones out -/
def pointCovers (sx sy x y : Int) : Bool :=
  decide (sx - 128 ≤ x) && decide (x < sx + 128) && decide (sy - 128 ≤ y) && decide (y < sy + 128)

/-- D3DRASTER.MS.POINTS: the square's left and top edges are in, its right and bottom ones out;
and of the samples at one offset o the point covers exactly one pixel's along each axis (the
squares of points a pixel apart tile the target) -/
theorem point_square (sx sy o p : Int) :
    pointCovers sx sy (sx - 128) (sy - 128) = true ∧ pointCovers sx sy (sx + 128) sy = false ∧
    pointCovers sx sy sx (sy + 128) = false ∧
    (sx - 128 ≤ sampleSub p o ∧ sampleSub p o < sx + 128 ↔ p = (sx - 16 * o - 1) / 256) := by
  refine ⟨?_, ?_, ?_, ?_⟩
  · simp only [pointCovers, Bool.and_eq_true, decide_eq_true_eq]; omega
  · simp only [pointCovers, Bool.and_eq_false_iff, decide_eq_false_iff_not]; omega
  · simp only [pointCovers, Bool.and_eq_false_iff, decide_eq_false_iff_not]; omega
  · simp only [sampleSub]
    constructor
    · intro ⟨h1, h2⟩
      omega
    · intro h
      subst h
      omega

/-- D3DRASTER.MS.SAMPLES: the standard pattern's sample positions (1/16 pixel from the center) of
a target of n samples -/
def pattern : Nat → List (Int × Int)
  | 2 => [(4, 4), (-4, -4)]
  | 4 => [(-2, -6), (6, -2), (-6, 2), (2, 6)]
  | 8 => [(1, -3), (-1, 3), (5, 1), (-3, -5), (-5, 5), (-7, -1), (3, 7), (7, -7)]
  | 16 => [(1, 1), (-1, -3), (-3, 2), (4, -1), (-5, -2), (2, 5), (5, 3), (3, -5), (-2, 6), (0, -7),
           (-4, -6), (-6, 4), (-8, 0), (7, -4), (6, 7), (-7, -8)]
  | _ => []

/-- every offset of the pattern inside the pixel: -8..7 sixteenths each way -/
def within (n : Nat) : Bool := (pattern n).all fun o => decide (-8 ≤ o.1 ∧ o.1 ≤ 7 ∧ -8 ≤ o.2 ∧ o.2 ≤ 7)

/-- D3DRASTER.MS.SAMPLES: 2, 4, 8 and 16 samples at as many distinct positions, each inside its
pixel (the pixel p's sample lies in 256 p .. 256 p + 255); no other count has a pattern -/
theorem pattern_samples (n : Nat) (p o : Int) (hn : n = 2 ∨ n = 4 ∨ n = 8 ∨ n = 16) (ho : -8 ≤ o ∧ o ≤ 7) :
    (pattern n).length = n ∧ (pattern n).Nodup ∧ within n = true ∧
    256 * p ≤ sampleSub p o ∧ sampleSub p o < 256 * p + 256 ∧
    (n ≠ 2 → n ≠ 4 → n ≠ 8 → n ≠ 16 → pattern n = []) := by
  refine ⟨?_, ?_, ?_, by simp only [sampleSub]; omega, by simp only [sampleSub]; omega, ?_⟩
  · rcases hn with h | h | h | h <;> subst h <;> rfl
  · rcases hn with h | h | h | h <;> subst h <;> decide
  · rcases hn with h | h | h | h <;> subst h <;> decide
  · intro a b c d
    rcases hn with h | h | h | h <;> simp_all

/-- D3DRASTER.MS.SAMPLES: the samples of pixel (px, py) the triangle covers by the top-left rule -/
def mask (a b c : Pt) (px py : Int) (n : Nat) : List Bool :=
  (pattern n).map fun o => covers a b c (sampleSub px o.1, sampleSub py o.2)

/-- D3DRASTER.MS.SAMPLES: two triangles sharing an edge from opposite sides never cover one
sample both, and a sample their other edges hold is covered by exactly one of them: per sample
as per pixel center -/
theorem mask_shared (a b c d : Pt) (px py : Int) (n : Nat) (hc : area a b c > 0) (hd : area b a d > 0) :
    ∀ k (h1 : k < (mask a b c px py n).length) (h2 : k < (mask b a d px py n).length),
      (!((mask a b c px py n)[k] && (mask b a d px py n)[k])) = true := by
  intro k h1 h2
  simp only [mask, List.getElem_map]
  exact (shared_edge a b c d _ hc hd).1

def mirrors : List Mirror := [
  ⟨"d3d11.covers", fun
    | [ax, ay, bx, by', cx, cy, sx, sy] => pure [if covers (ax, ay) (bx, by') (cx, cy) (sx, sy) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.cull", fun
    | [m, ccw, cw] => do pure [if culls (← nat m) (ccw != 0) (cw != 0) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.frontface", fun
    | [ccw, cw] => some (if (ccw = 0 ∨ ccw = 1) ∧ (cw = 0 ∨ cw = 1) then
        [if frontFace (ccw = 1) (cw = 1) then 1 else 0] else [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.scissor", fun
    | [on, l, t, r, b, px, py] => pure [if scissorHolds (on != 0) l t r b px py then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.point", fun | [s] => pure [pointPixel s] | _ => none⟩,
  ⟨"d3d11.pointms", fun
    | [sx, sy, x, y] => pure [if pointCovers sx sy x y then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.pattern", fun
    | [n] => do pure ((pattern (← nat n)).flatMap fun o => [o.1, o.2])
    | _ => none⟩,
  ⟨"d3d11.mask", fun
    | [ax, ay, bx, by', cx, cy, px, py, n] => do
      pure ((mask (ax, ay) (bx, by') (cx, cy) px py (← nat n)).map fun c => if c then 1 else 0)
    | _ => none⟩
]

end Kikizake.D3d11.Raster
