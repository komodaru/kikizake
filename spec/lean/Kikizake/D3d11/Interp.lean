import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
/-! The float arithmetic of Direct3D 10 and 11's viewport and interpolation (spec/d3dres.txt
D3DRASTER.VIEWPORT, VP10, INTERP.PLANE, POSW.PLANE, INTERP.DEPTH, INTERP.CLIP, LINE.COLOR,
LINE.DEPTH) in binary32 and binary64 (Kikizake.F32) as src/model/d3draster.rs computes it:
`planeValue`, `lineValue`, `clampRange`, `vertexDepth`, `viewportXY`, `vp10`, `clipLerp`,
`madeScreen` are its `plane_value`, `line_value`, `clamp_range`, `vertex_depth`, `viewport_xy`,
`vp10`, `clip_lerp`, `made_screen` (mirrors `d3d11.plane`, `d3d11.lineplane`, `d3d11.clamprange`,
`d3d11.vertexdepth`, `d3d11.viewport`, `d3d11.vp10`, `d3d11.cliplerp`, `d3d11.madescreen`; a
NaN result compared as the one quiet NaN). -/
namespace Kikizake.D3d11.Interp

open Kikizake.Float (isNaN)
open Kikizake.F32 (add sub mul div)
open Kikizake.D3d11.Filter (ltF truncF one half)

def neg (x : Nat) : Nat := Float.neg F32.fmt x
def f256 : Nat := 0x43800000
/-- f32::MIN_POSITIVE, the least normal binary32 -/
def minPos : Nat := 0x00800000

/-- Rust `f32::max` and `min` as x86-64 computes them: a NaN gives the other; of equal values
(+0 and -0) the second -/
def maxF (x y : Nat) : Nat :=
  if isNaN F32.fmt x then y else if isNaN F32.fmt y then x else if F32.lt y x then x else y
def minF (x y : Nat) : Nat :=
  if isNaN F32.fmt x then y else if isNaN F32.fmt y then x else if F32.lt x y then x else y

/-- an integer as the nearest binary32 (Rust `as f32`) -/
def ofInt (v : Int) : Nat := Float.round F32.fmt (decide (v < 0)) v.natAbs 1

/-- cvtss2si: to nearest, ties to even; a NaN or a value past i32 the integer indefinite -/
def cvt (v : Nat) : Int :=
  let indefinite : Int := -(2 ^ 31)
  if isNaN F32.fmt v || Float.isInf F32.fmt v then indefinite
  else
    let (s, n, d) := Float.toRat F32.fmt v
    let q := n / d
    let r := n % d
    let k := if 2 * r > d ∨ (2 * r = d ∧ q % 2 = 1) then q + 1 else q
    let x : Int := if s then -(k : Int) else k
    if -(2 ^ 31) ≤ x ∧ x < 2 ^ 31 then x else indefinite

/-- a value in pixels snapped to 1/256 -/
def snap (v : Nat) : Int := cvt (mul v f256)

/-! ## D3DRASTER.INTERP.PLANE, POSW.PLANE -/

/-- an attribute's plane over a triangle at (cx, cy): f32 gradients times 1 / the doubled area
(exact in binary64, then to binary32), C = (a0 - gx x0) - gy y0, (C + gy cy) + gx cx -/
def planeValue (x0 x1 x2 y0 y1 y2 a0 a1 a2 cx cy : Nat) : Nat :=
  let area := F64.toF32 (F64.sub (F64.mul (F64.ofF32 (sub y2 y0)) (F64.ofF32 (sub x1 x0)))
    (F64.mul (F64.ofF32 (sub y0 y1)) (F64.ofF32 (sub x0 x2))))
  let r := div one area
  let d1 := sub a1 a0
  let d2 := sub a2 a0
  let gx := mul (sub (mul d1 (sub y2 y0)) (mul d2 (sub y1 y0))) r
  let gy := mul (sub (mul d2 (sub x1 x0)) (mul d1 (sub x2 x0))) r
  let c := sub (sub a0 (mul gx x0)) (mul gy y0)
  add (add c (mul gy cy)) (mul gx cx)

/-! ## D3DRASTER.LINE.COLOR, LINE.DEPTH -/

/-- a value along a line's major axis (ends a, b in 1/256 pixels) at c -/
def lineValue (a b : Int) (c va vb : Nat) : Nat :=
  let a := div (ofInt a) f256
  let b := div (ofInt b) f256
  let g := div one (sub b a)
  let grad := mul (sub vb va) g
  add (sub va (mul grad a)) (mul grad c)

/-! ## D3DRASTER.INTERP.DEPTH -/

/-- a depth onto the viewport's range, clamped: max then min -/
def clampRange (minz maxz z : Nat) : Nat :=
  let le := !isNaN F32.fmt minz && !isNaN F32.fmt maxz && !F32.lt maxz minz
  let (lo, hi) := if le then (minz, maxz) else (maxz, minz)
  minF (maxF z lo) hi

/-- a vertex's depth: minz + z (1/w) (maxz - minz) -/
def vertexDepth (minz maxz z w : Nat) : Nat := add minz (mul (mul z (div one w)) (sub maxz minz))

/-! ## D3DRASTER.VIEWPORT, VP10 -/

/-- a clip position through the viewport transform, snapped: x + (x/w + 1) (width/2),
y + (1 - y/w) (height/2) -/
def viewportXY (vx vy vw vh x y w : Nat) : Int × Int :=
  (snap (add vx (mul (add (div x w) one) (mul vw half))), snap (add vy (mul (sub one (div y w)) (mul vh half))))

/-- Direct3D 10's viewport: the (INT) cast toward zero -/
def vp10 (x : Nat) : Nat := truncF F32.fmt x

/-! ## D3DRASTER.INTERP.CLIP -/

/-- a value of a vertex the clipping makes: f32(((out - in) as f64 dIn) / (dIn - dOut) + in) -/
def clipLerp (din dout vi vo : Nat) : Nat :=
  F64.toF32 (F64.add (F64.div (F64.mul (F64.ofF32 (sub vo vi)) (F64.ofF32 din)) (F64.ofF32 (sub din dout))) (F64.ofF32 vi))

/-- a vertex the clipping makes onto the screen: w' = max(w, the least normal), x and y clamped
to -w'..w' (z to 0..w' with depth clipping), X, Y snapped, the depth, 1/w' -/
def madeScreen (vx vy vw vh minz maxz x y z w : Nat) (zclip : Bool) : Int × Int × Nat × Nat :=
  let w := if ltF w minPos then minPos else w
  let clamp := fun v => if ltF (if ltF (neg w) v then v else neg w) w then (if ltF (neg w) v then v else neg w) else w
  let x := clamp x
  let y := clamp y
  let z := if zclip then minF (maxF z 0) w else z
  let rhw := div one w
  let sx := mul vw half
  let ox := add (mul vw half) vx
  let sy := mul (neg vh) half
  let oy := add (mul vh half) vy
  (snap (add (mul (mul rhw x) sx) ox), snap (add (mul (mul rhw y) sy) oy),
    add (mul (mul rhw z) (sub maxz minz)) minz, rhw)

/-! ## theorems -/

private theorem qnan_nan : isNaN F32.fmt (Float.qnan F32.fmt) = true := by decide
private theorem negq_nan : isNaN F32.fmt (Float.neg F32.fmt (Float.qnan F32.fmt)) = true := by decide

/-- D3DRASTER.VIEWPORT: a NaN w snaps the position to the integer indefinite, -2^31 -/
theorem viewport_nan (vx vy vw vh x y w : Nat) (h : isNaN F32.fmt w = true) :
    viewportXY vx vy vw vh x y w = (-(2 ^ 31), -(2 ^ 31)) := by
  simp [viewportXY, snap, cvt, F32.mul, F32.add, F32.sub, F32.div, Float.mul, Float.sub, Float.add, Float.div, h, qnan_nan, negq_nan]

/-- D3DRASTER.VP10: a fractional origin is cast toward zero: 1.5 is 1, -1.5 is -1 -/
theorem vp10_toward_zero : vp10 0x3fc00000 = one ∧ vp10 0xbfc00000 = 0xbf800000 := by decide

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- D3DRASTER.INTERP.PLANE: the plane of 0, 1, 2 at (0, 0), (4, 0), (0, 4) is 1.5 at (2, 2) -/
theorem plane_example :
    planeValue 0 0x40800000 0 0 0 0x40800000 0 one 0x40000000 0x40000000 0x40000000 = 0x3fc00000 := by decide

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- D3DRASTER.POSW.PLANE: where every vertex has 1/w = 1/2, SV_Position.w is 2 at the center -/
theorem posw_example :
    div one (planeValue 0 0x40800000 0 0 0 0x40800000 half half half 0x40000000 0x40000000) = 0x40000000 := by decide

/-- D3DRASTER.INTERP.DEPTH, LINE.DEPTH: a range of NaN leaves the depth -/
theorem clamp_nan_range (minz maxz z : Nat) (h1 : isNaN F32.fmt minz = true) (h2 : isNaN F32.fmt maxz = true)
    (hz : isNaN F32.fmt z = false) : clampRange minz maxz z = z := by
  simp [clampRange, maxF, minF, h1, h2, hz]

/-- D3DRASTER.INTERP.DEPTH: a vertex at depth 1/2, w 2 on the range 0..1 is at 1/4 -/
theorem vertex_depth_example : vertexDepth 0 one half 0x40000000 = 0x3e800000 := by decide

/-- D3DRASTER.INTERP.CLIP: a made vertex at w below the least normal (or negative) is put at
w' = the least normal -/
theorem made_least (vx vy vw vh minz maxz x y z w : Nat) (zclip : Bool) (h : ltF w minPos = true) :
    (madeScreen vx vy vw vh minz maxz x y z w zclip).2.2.2 = div one minPos := by
  simp [madeScreen, h]

/-- D3DRASTER.LINE.COLOR: from 0 at x = 0 to 1 at x = 4 (in 1/256: 1024), 1/2 at x = 2 -/
theorem line_example : lineValue 0 1024 0x40000000 0 one = half := by decide

/-- the mirrors (an f32 NaN answer as the one quiet NaN) -/
def canon (x : Nat) : Int := (F32.canon x : Nat)

def mirrors : List Mirror := [
  ⟨"d3d11.plane", fun
    | [x0, x1, x2, y0, y1, y2, a0, a1, a2, cx, cy] => do
      pure [canon (planeValue (← nat x0) (← nat x1) (← nat x2) (← nat y0) (← nat y1) (← nat y2) (← nat a0) (← nat a1) (← nat a2) (← nat cx) (← nat cy))]
    | _ => none⟩,
  ⟨"d3d11.lineplane", fun
    | [a, b, c, va, vb] => do pure [canon (lineValue a b (← nat c) (← nat va) (← nat vb))]
    | _ => none⟩,
  ⟨"d3d11.clamprange", fun
    | [lo, hi, z] => do pure [canon (clampRange (← nat lo) (← nat hi) (← nat z))]
    | _ => none⟩,
  ⟨"d3d11.vertexdepth", fun
    | [lo, hi, z, w] => do pure [canon (vertexDepth (← nat lo) (← nat hi) (← nat z) (← nat w))]
    | _ => none⟩,
  ⟨"d3d11.viewport", fun
    | [vx, vy, vw, vh, x, y, w] => do
      let (a, b) := viewportXY (← nat vx) (← nat vy) (← nat vw) (← nat vh) (← nat x) (← nat y) (← nat w)
      pure [a, b]
    | _ => none⟩,
  ⟨"d3d11.vp10", fun | [x] => do pure [canon (vp10 (← nat x))] | _ => none⟩,
  ⟨"d3d11.cliplerp", fun
    | [a, b, c, d] => do pure [canon (clipLerp (← nat a) (← nat b) (← nat c) (← nat d))]
    | _ => none⟩,
  ⟨"d3d11.madescreen", fun
    | [vx, vy, vw, vh, lo, hi, x, y, z, w, zc] => do
      let (a, b, dz, rhw) := madeScreen (← nat vx) (← nat vy) (← nat vw) (← nat vh) (← nat lo) (← nat hi) (← nat x) (← nat y) (← nat z) (← nat w) (zc != 0)
      pure [a, b, canon dz, canon rhw]
    | _ => none⟩
]

end Kikizake.D3d11.Interp
