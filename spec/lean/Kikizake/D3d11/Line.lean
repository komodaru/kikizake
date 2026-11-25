import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
import Kikizake.D3d11.Interp
/-! Direct3D 10 and 11's lines (spec/d3dres.txt D3DRASTER.LINE, LINE.END, LINE.QUAD,
LINE.ALPHA, MS.LINES) in binary32 as src/model/d3draster.rs computes them: `lineCovers` is
`line_covers` (`Cover::of` and `Cover::covers`), `quadCorners` its `quad_corners`,
`alphaCorners` its `alpha_corners`, `alphaCover` its `alpha_coverage`, `lineSamples` its
`line_samples` (mirrors `d3d11.linecovers`, `d3d11.quadcorners`, `d3d11.alphacorners`,
`d3d11.alphacover`, `d3d11.linesamples`; a NaN answer compared as the one quiet NaN). Ends are
in 1/256 pixels, y down. -/
namespace Kikizake.D3d11.Line

open Kikizake.Float (isNaN)
open Kikizake.F32 (add sub mul div sqrt)
open Kikizake.D3d11.Filter (ltF eqF floorF half one)
open Kikizake.D3d11.Interp (centerOf neg ofInt snap f256)

/-- an integer position in 1/256 pixels as pixels: `v as f32 / 256.0` -/
def pixelsI (v : Int) : Nat := div (ofInt v) f256

/-- f32's x <= y (false with a NaN) -/
def leF (x y : Nat) : Bool := ltF x y || eqF x y

/-- f32::abs: the sign bit cleared -/
def absF (x : Nat) : Nat := x % 2 ^ 31

/-- f32::ceil, through floor: -floor(-x) -/
def ceilF (x : Nat) : Nat := neg (floorF F32.fmt (neg x))

/-- ceil(v) - 0.5: the center of the pixel whose diamond may hold v -/
def owner (v : Nat) : Nat := sub (ceilF v) half

/-- 0.5 and the next binary32 up -/
def halfUp : Nat := 0x3f000001
/-- 1/256 -/
def e256 : Nat := 0x3b800000

/-- (x, y) in the diamond about (cx, cy): its lower half with its edges, its upper half
without; on the middle row a y-major line's diamond holds its right corner -/
def diamondHolds (x y cx cy : Nat) (majorY : Bool) : Bool :=
  let d := add (absF (sub x cx)) (absF (sub y cy))
  let inclusive := ltF cy y || eqF y cy && majorY && ltF cx x
  if inclusive then leF d half else ltF d half

/-- the major axis: y when |dy| > |dx| -/
def majorOf (a b : Int × Int) : Bool := (b.2 - a.2).natAbs > (b.1 - a.1).natAbs

/-- an aliased line as the rasterizer tests it: (x0, y0, dx, dy, the bounds of the column's and
the row's crossing) -/
structure Cover where
  x0 : Nat
  y0 : Nat
  dx : Nat
  dy : Nat
  hiCol : Nat
  hiRow : Nat

/-- D3DRASTER.LINE, LINE.END: the ends in f32; a start in its diamond moved back, an end in its
diamond pulled back, along the line to 1/256 past the center's major coordinate; both ends in
one diamond: no pixel -/
def coverOf (a b : Int × Int) (majorY : Bool) : Cover :=
  let x0 := pixelsI a.1
  let y0 := pixelsI a.2
  let x1 := pixelsI b.1
  let y1 := pixelsI b.2
  let dx := sub x1 x0
  let dy := sub y1 y0
  let cx0 := owner x0
  let cy0 := owner y0
  let cx1 := owner x1
  let cy1 := owner y1
  let ins := diamondHolds x0 y0 cx0 cy0 majorY
  let ine := diamondHolds x1 y1 cx1 cy1 majorY
  let hiRow := if majorY then halfUp else half
  if ins && ine && eqF cx0 cx1 && eqF cy0 cy1 then ⟨neg half, neg half, 0, 0, halfUp, hiRow⟩
  else
    let sgn := fun d => if ltF 0 d then e256 else neg e256
    let (cx, cy, cdx, cdy) :=
      if ins then
        let (u, v) :=
          if majorY then let v := add (sub y0 cy0) (sgn dy); (mul (div dx dy) v, v)
          else let u := add (sub x0 cx0) (sgn dx); (u, mul (div dy dx) u)
        (sub x0 u, sub y0 v, add dx u, add dy v)
      else (x0, y0, dx, dy)
    let (cdx, cdy) :=
      if ine then
        let (u, v) :=
          if majorY then let v := add (sub (add cy cdy) cy1) (sgn cdy); (mul (div cdx cdy) v, v)
          else let u := add (sub (add cx cdx) cx1) (sgn cdx); (u, mul (div cdy cdx) u)
        (sub cdx u, sub cdy v)
      else (cdx, cdy)
    ⟨cx, cy, cdx, cdy, halfUp, hiRow⟩

/-- D3DRASTER.LINE: the line crosses the pixel's column center line or its row center line
inside the pixel, each f32 operation rounded on its own -/
def Cover.covers (c : Cover) (px py : Int) : Bool :=
  let invDx := div (neg one) c.dx
  let invDy := div (neg one) c.dy
  let a := sub c.x0 (centerOf px)
  let b := sub c.y0 (centerOf py)
  let inside := fun t => leF 0 t && leF t one
  let t := mul a invDx
  let e := add (mul t c.dy) b
  let col := inside t && ltF (neg half) e && ltF e c.hiCol
  let t2 := mul b invDy
  let e2 := add (mul t2 c.dx) a
  let row := inside t2 && ltF (neg half) e2 && ltF e2 c.hiRow
  col || row

def lineCovers (a b p : Int × Int) : Bool := (coverOf a b (majorOf a b)).covers p.1 p.2

/-- 0.7 in binary32: half a multisample line's width -/
def quadHalf : Nat := 0x3f333333

/-- D3DRASTER.LINE.QUAD: the rectangle 1.4 pixels wide along the line: corners a + n, b + n,
b - n, a - n, n = (-dy, dx) 0.7 / sqrt(dx^2 + dy^2), snapped -/
def quadCorners (a b : Int × Int) : List (Int × Int) :=
  let ax := pixelsI a.1
  let ay := pixelsI a.2
  let bx := pixelsI b.1
  let by' := pixelsI b.2
  let dx := sub bx ax
  let dy := sub by' ay
  let k := div quadHalf (sqrt (add (mul dx dx) (mul dy dy)))
  let nx := mul (neg dy) k
  let ny := mul dx k
  let q := fun x y => (snap x, snap y)
  [q (add ax nx) (add ay ny), q (add bx nx) (add by' ny), q (sub bx nx) (sub by' ny), q (sub ax nx) (sub ay ny)]

/-- D3DRASTER.LINE.ALPHA: the rectangle 4 pixels wide along the line: o = (dy, -dx) 2 / length
in f32, snapped, the corners a - o, a + o, b + o, b - o -/
def alphaOffset (a b : Int × Int) : Int × Int :=
  let dyf := pixelsI (b.2 - a.2)
  let ndx := pixelsI (a.1 - b.1)
  let len := sqrt (add (mul ndx ndx) (mul dyf dyf))
  let k := div (F32.ofNat 2) len
  (snap (mul dyf k), snap (mul ndx k))

def alphaCorners (a b : Int × Int) : List (Int × Int) :=
  let (ox, oy) := alphaOffset a b
  [(a.1 - ox, a.2 - oy), (a.1 + ox, a.2 + oy), (b.1 + ox, b.2 + oy), (b.1 - ox, b.2 - oy)]

/-- 0.45 in binary32 -/
def c045 : Nat := 0x3ee66666
def c15 : Nat := 0x3fc00000

/-- D3DRASTER.LINE.ALPHA: the coverage at the pixel: s the distance from the line by its f32
plane (A, B, C) = (dy, -dx, y0 dx - x0 dy) / sqrt(dx^2 + dy^2), clamp((1.5 - |s|) 0.45, 0, 1) -/
def alphaCover (a b p : Int × Int) : Nat :=
  let dyf := pixelsI (b.2 - a.2)
  let dx := pixelsI (b.1 - a.1)
  let a0 := dyf
  let b0 := neg dx
  let c0 := sub (mul (pixelsI a.2) dx) (mul (pixelsI a.1) dyf)
  let inv := div one (sqrt (add (mul b0 b0) (mul dyf dyf)))
  let s := add (mul (mul inv a0) (centerOf p.1)) (add (mul (mul inv b0) (centerOf p.2)) (mul inv c0))
  F32.clamp (mul (sub c15 (absF s)) c045) 0 one

/-- D3DRASTER.MS.LINES: the samples a line writes at a pixel it lights: with the multisample
state those its quad covers, else every one -/
def lineSamples (multisample : Bool) (covered : List Bool) : List Bool :=
  if multisample then covered else covered.map fun _ => true

/-! ## theorems -/

/-- D3DRASTER.MS.LINES: an aliased or alpha line lights every sample of a pixel it lights -/
theorem every_sample (covered : List Bool) : ∀ s ∈ lineSamples false covered, s = true := by
  simp [lineSamples]

/-- D3DRASTER.LINE.QUAD, LINE.ALPHA: a rectangle of four corners -/
theorem four_corners (a b : Int × Int) : (quadCorners a b).length = 4 ∧ (alphaCorners a b).length = 4 := by
  simp [quadCorners, alphaCorners]

/-- D3DRASTER.LINE.ALPHA: the alpha rectangle is symmetric about the line: its corners at a are
a -+ o, at b b +- o -/
theorem alpha_symmetric (a b : Int × Int) :
    ∃ o : Int × Int, alphaCorners a b = [(a.1 - o.1, a.2 - o.2), (a.1 + o.1, a.2 + o.2), (b.1 + o.1, b.2 + o.2), (b.1 - o.1, b.2 - o.2)] :=
  ⟨alphaOffset a b, rfl⟩

/-- D3DRASTER.LINE: a horizontal line from (0.5, 0.5) to (4.5, 0.5) lights the pixels it
crosses the column center of and not the one past its end -/
theorem horizontal_example :
    lineCovers (128, 128) (1152, 128) (1, 0) = true ∧ lineCovers (128, 128) (1152, 128) (4, 0) = false := by
  decide

/-- D3DRASTER.LINE.END: an end inside a diamond is pulled back: a line from the center of
pixel 0 to the center of pixel 3 lights pixel 0 and not pixel 3 -/
theorem end_pulled_back :
    lineCovers (128, 128) (896, 128) (0, 0) = true ∧ lineCovers (128, 128) (896, 128) (3, 0) = false := by
  decide

/-- D3DRASTER.LINE.ALPHA: on the line the coverage is 1.5 times 0.45, below 1 -/
theorem alpha_on_line : alphaCover (0, 128) (1024, 128) (1, 0) = 0x3f2ccccc := by decide

def mirrors : List Mirror := [
  ⟨"d3d11.linecovers", fun
    | [ax, ay, bx, by', px, py] => pure [if lineCovers (ax, ay) (bx, by') (px, py) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d11.quadcorners", fun
    | [ax, ay, bx, by'] => pure ((quadCorners (ax, ay) (bx, by')).flatMap fun c => [c.1, c.2])
    | _ => none⟩,
  ⟨"d3d11.alphacorners", fun
    | [ax, ay, bx, by'] => pure ((alphaCorners (ax, ay) (bx, by')).flatMap fun c => [c.1, c.2])
    | _ => none⟩,
  ⟨"d3d11.alphacover", fun
    | [ax, ay, bx, by', px, py] => pure [(F32.canon (alphaCover (ax, ay) (bx, by') (px, py)) : Nat)]
    | _ => none⟩,
  ⟨"d3d11.linesamples", fun
    | ms :: covered => pure ((lineSamples (ms != 0) (covered.map (· != 0))).map fun b => if b then 1 else 0)
    | _ => none⟩
]

end Kikizake.D3d11.Line
