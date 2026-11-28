import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d11.Filter
import Kikizake.D3d11.Raster
/-! The float arithmetic of Direct3D 10 and 11's viewport and interpolation (spec/d3dres.txt
D3DRASTER.VIEWPORT, VP10, INTERP.PLANE, POSW.PLANE, INTERP.DEPTH, INTERP.CLIP, LINE.COLOR,
LINE.DEPTH) in binary32 and binary64 (Kikizake.F32) as src/model/d3draster.rs computes it:
`planeValue`, `lineValue`, `clampRange`, `vertexDepth`, `viewportXY`, `vp10`, `clipLerp`,
`madeScreen` are its `plane_value`, `line_value`, `clamp_range`, `vertex_depth`, `viewport_xy`,
`vp10`, `clip_lerp`, `made_screen` (mirrors `d3d11.plane`, `d3d11.lineplane`, `d3d11.clamprange`,
`d3d11.vertexdepth`, `d3d11.viewport`, `d3d11.vp10`, `d3d11.cliplerp`, `d3d11.madescreen`; a
NaN result compared as the one quiet NaN). The multisample checks also model per-sample depth
interpolation, coverage, storage and write behavior in src/model/d3draster.rs and d3drules.rs. -/
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

/-- A pixel's center along one axis, rounded in binary32. -/
def centerOf (p : Int) : Nat := add (ofInt p) half

/-- The SV_Position components the generated pixel shader copies to its output. -/
def positionColor (x y : Int) (depth w : Nat) : List Nat :=
  [centerOf x, centerOf y, depth, w]

/-- The constant input takes the leading vertex's values bit for bit. -/
def flatColor (r g b a : Nat) : List Nat := [r, g, b, a]

/-- Identical clip w values take the first vertex's representation. A NaN never compares equal. -/
def constantClipW : List Nat → Option Nat
  | [] => none
  | w :: ws => if (w :: ws).all (Filter.eqF w) then some w else none

/-- The reciprocal of the plane of 1/w is SV_Position.w for varying clip w. -/
def positionWFromReciprocal (inverse : Nat) : Nat := div one inverse

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

/-! ## Texture coordinates -/

abbrev Triple := Nat × Nat × Nat

/-- The texture coordinates use the same perspective plane evaluation as color. -/
def sampleUV (xs ys iw u v : Triple) (cx cy : Nat) : Nat × Nat :=
  let plane (a : Triple) :=
    planeValue xs.1 xs.2.1 xs.2.2 ys.1 ys.2.1 ys.2.2 a.1 a.2.1 a.2.2 cx cy
  let q := div one (plane iw)
  (mul (plane u) q, mul (plane v) q)

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
theorem texture_coordinates_use_reciprocal_w :
    sampleUV (0, 0x40000000, 0) (0, 0, 0x40000000) (one, 0x40000000, 0x40800000)
      (0, 0x40800000, 0) (0, 0, 0x41000000) 0x3f000000 0x3f000000 =
      (0x3f000000, one) := by
  decide +kernel

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

/-! ## D3DRASTER.MS.DEPTH, MS.ZPLANE -/

/-- Depth at the sample's offset in sixteenths of a pixel, clamped to the viewport. -/
def sampleDepthPlane (x0 x1 x2 y0 y1 y2 z0 z1 z2 lo hi cx cy : Nat) (ox oy : Int) : Nat :=
  let shift := fun p o => add p (div (ofInt o) 0x41800000)
  clampRange lo hi (planeValue x0 x1 x2 y0 y1 y2 z0 z1 z2 (shift cx ox) (shift cy oy))

/-- The covered sample indices, each with its own depth or the pixel depth as fallback. -/
def sampleDepthJobs (coverage : List Bool) (center : Option Nat) (depths : List Nat) :
    List (Nat × Option Nat) :=
  (List.range coverage.length).filterMap fun k =>
    if coverage[k]?.getD false then some (k, depths[k]?.orElse (fun _ => center)) else none

/-- The byte range of a pixel element in its sample's plane. -/
def sampleSpan (bytes count k off elem : Nat) : Option (Nat × Nat) :=
  let plane := bytes / count
  if 0 < count ∧ bytes % count = 0 ∧ k < count ∧ off + elem ≤ plane
    then some (k * plane + off, k * plane + (off + elem))
    else none

inductive DepthAction where
  | reject
  | keep
  | write (depth : Nat)
  deriving DecidableEq

/-- Compare the quantized and stored depths. A passing write uses the original depth. -/
def depthAction (func : Nat) (write : Bool) (test : Option (Nat × Nat × Nat)) : DepthAction :=
  match test with
  | none => .keep
  | some (q, stored, original) =>
    if Filter.cmpWith F64.fmt func q stored then
      if write then .write original else .keep
    else .reject

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
    positionWFromReciprocal (planeValue 0 0x40800000 0 0 0 0x40800000 half half half
      0x40000000 0x40000000) = 0x40000000 := by decide

/-- The position shader passes through the selected depth and clip w as binary32 bits. -/
theorem position_depth_w (x y : Int) (depth w : Nat) :
    (positionColor x y depth w)[2]? = some depth ∧ (positionColor x y depth w)[3]? = some w := by
  simp [positionColor]

/-- Position x and y are shifted to the pixel center, including for negative coordinates. -/
theorem position_centers :
    positionColor (-3) 7 0x3e800000 0x3f000000 = [0xc0200000, 0x40f00000, 0x3e800000, 0x3f000000] := by
  decide

/-- The constant-w path is selected exactly when every vertex's w compares equal to the first. -/
theorem constant_w_selection (w : Nat) (ws : List Nat) :
    constantClipW (w :: ws) = some w ↔ (w :: ws).all (Filter.eqF w) = true := by
  cases h : (w :: ws).all (Filter.eqF w) <;> simp [constantClipW, h]

/-- Equal signed zeros keep the first representation. Differing finite w goes to interpolation. -/
theorem constant_w_examples :
    constantClipW [0x80000000, 0, 0] = some 0x80000000 ∧
    constantClipW [half, one] = none ∧
    constantClipW [0x7fc00000, 0x7fc00000] = none := by
  decide

/-- A constant shader input performs no arithmetic that could change a component's bits. -/
theorem flat_color_bits (r g b a : Nat) : flatColor r g b a = [r, g, b, a] := rfl

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

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- The standard 4× pattern yields separate triangle depths at all four positions of this plane. -/
theorem sample_depth_plane_example :
    (Raster.pattern 4).map (fun (x, y) =>
      sampleDepthPlane 0 0x40800000 0 0 0 0x40800000 0 one 0x40000000
        0 0x40000000 0x40000000 0x40000000 x y) =
      [0x3fa40000, 0x3fc40000, 0x3fbc0000, 0x3fdc0000] := by decide

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- The viewport clamps each evaluated depth, including samples on the high side of the plane. -/
theorem sample_depth_plane_clamp :
    (Raster.pattern 4).map (fun (x, y) =>
      sampleDepthPlane 0 0x40800000 0 0 0 0x40800000 0 one 0x40000000
        0 0x3fc00000 0x40000000 0x40000000 x y) =
      [0x3fa40000, 0x3fc00000, 0x3fbc0000, 0x3fc00000] := by decide

/-- Exactly the covered sample indices participate, with their own depth first. -/
theorem sample_jobs_covered (coverage : List Bool) (center : Option Nat) (depths : List Nat)
    (k : Nat) (z : Option Nat) :
    (k, z) ∈ sampleDepthJobs coverage center depths ↔
      k < coverage.length ∧ coverage[k]?.getD false = true ∧ z = depths[k]?.orElse (fun _ => center) := by
  simp only [sampleDepthJobs, List.mem_filterMap, List.mem_range]
  constructor
  · rintro ⟨i, hi, h⟩
    split at h
    · cases h
      exact ⟨hi, by assumption, rfl⟩
    · contradiction
  · rintro ⟨hk, hc, rfl⟩
    exact ⟨k, hk, by simp [hc]⟩

/-- An in-bounds element in a packed buffer is addressed by its sample plane. -/
theorem sample_span_in_plane (count plane k off elem : Nat)
    (pos : 0 < count) (within : k < count) (inside : off + elem ≤ plane) :
    sampleSpan (plane * count) count k off elem = some (k * plane + off, k * plane + (off + elem)) := by
  simp [sampleSpan, pos, within, inside, Nat.mul_div_cancel _ pos]

/-- Consecutive or distant samples' byte ranges never alias. -/
theorem sample_spans_disjoint (count plane k j off elem : Nat)
    (ordered : k < j) (within : j < count) (inside : off + elem ≤ plane) :
    ∃ a b c d, sampleSpan (plane * count) count k off elem = some (a, b) ∧
      sampleSpan (plane * count) count j off elem = some (c, d) ∧ b ≤ c := by
  have pos : 0 < count := by omega
  refine ⟨_, _, _, _, sample_span_in_plane count plane k off elem pos (by omega) inside,
    sample_span_in_plane count plane j off elem pos within inside, ?_⟩
  have next := Nat.mul_le_mul_right plane (Nat.succ_le_of_lt ordered)
  rw [Nat.succ_mul] at next
  omega

/-- A depth-disabled sample still reaches color independently of the write flag and function. -/
theorem depth_disabled (func : Nat) (write : Bool) :
    depthAction func write none = .keep := rfl

/-- Failing the comparison suppresses color and depth. A pass honors the depth write flag. -/
theorem depth_test (func q stored z : Nat) (write : Bool) :
    (depthAction func write (some (q, stored, z)) = .reject ↔ Filter.cmpWith F64.fmt func q stored = false) ∧
    (depthAction func write (some (q, stored, z)) = .write z ↔
      Filter.cmpWith F64.fmt func q stored = true ∧ write = true) := by
  cases h : Filter.cmpWith F64.fmt func q stored <;> cases write <;> simp [depthAction, h]

set_option exponentiation.threshold 2048 in
set_option maxRecDepth 20000 in
/-- Four covered samples use separate triangle depths for the same LESS comparison. -/
theorem sample_plane_depth_test :
    (sampleDepthJobs [true, true, true, true] none ((Raster.pattern 4).map (fun (x, y) =>
      sampleDepthPlane 0 0x40800000 0 0 0 0x40800000 0 one 0x40000000
        0 0x40000000 0x40000000 0x40000000 x y))).map (fun (_, depth) =>
      depthAction 2 true (depth.map (fun z => (F64.ofF32 z, 0x3ff8000000000000, z)))) =
      [.write 0x3fa40000, .reject, .write 0x3fbc0000, .reject] := by decide

/-- the mirrors (an f32 NaN answer as the one quiet NaN) -/
def canon (x : Nat) : Int := (F32.canon x : Nat)

private def u32Argument (value : Int) : Option Nat := do
  let n ← nat value
  if n < 2 ^ 32 then some n else none

private def i64Argument (value : Int) : Option Int :=
  if -(2 ^ 63) ≤ value ∧ value < 2 ^ 63 then some value else none

def mirrors : List Mirror := [
  ⟨"d3d11.sampleuv", fun
    | [x0, x1, x2, y0, y1, y2, w0, w1, w2, u0, u1, u2, v0, v1, v2, cx, cy] => do
      let (u, v) := sampleUV (← nat x0, ← nat x1, ← nat x2) (← nat y0, ← nat y1, ← nat y2)
        (← nat w0, ← nat w1, ← nat w2) (← nat u0, ← nat u1, ← nat u2)
        (← nat v0, ← nat v1, ← nat v2) (← nat cx) (← nat cy)
      pure [Int.ofNat (F32.canon u), Int.ofNat (F32.canon v)]
    | _ => none⟩,
  ⟨"d3d11.positioncolor", fun
    | [x, y, depth, w] => some ((do
        let px ← i64Argument x
        let py ← i64Argument y
        pure ((positionColor px py (← u32Argument depth) (← u32Argument w)).map Int.ofNat)).getD [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.flatcolor", fun args => some ((do
    let [r, g, b, a] ← args.mapM u32Argument | none
    pure ((flatColor r g b a).map Int.ofNat)).getD [-1])⟩,
  ⟨"d3d11.constantclipw", fun args => some ((do
    let ws ← args.mapM u32Argument
    let w ← constantClipW ws
    pure [(w : Int)]).getD [-1])⟩,
  ⟨"d3d11.positionreciprocalw", fun
    | [r] => some ((u32Argument r).map (fun n => [canon (positionWFromReciprocal n)]) |>.getD [-1])
    | _ => some [-1]⟩,
  ⟨"d3d11.plane", fun
    | [x0, x1, x2, y0, y1, y2, a0, a1, a2, cx, cy] => do
      pure [canon (planeValue (← nat x0) (← nat x1) (← nat x2) (← nat y0) (← nat y1) (← nat y2) (← nat a0) (← nat a1) (← nat a2) (← nat cx) (← nat cy))]
    | _ => none⟩,
  ⟨"d3d11.sampledepthplane", fun
    | [x0, x1, x2, y0, y1, y2, z0, z1, z2, lo, hi, cx, cy, ox, oy] => do
      pure [canon (sampleDepthPlane (← nat x0) (← nat x1) (← nat x2) (← nat y0) (← nat y1)
        (← nat y2) (← nat z0) (← nat z1) (← nat z2) (← nat lo) (← nat hi) (← nat cx) (← nat cy) ox oy)]
    | _ => none⟩,
  ⟨"d3d11.sampledepthjobs", fun
    | mask :: width :: center :: depths => do
      let mask ← nat mask
      let width ← nat width
      if width > 16 || mask ≥ 2 ^ 16 then none else
        let coverage := (List.range width).map fun k => mask / (2 ^ k) % 2 == 1
        let depths ← depths.mapM nat
        pure ((sampleDepthJobs coverage (nat center) depths).flatMap fun (k, z) =>
          [(k : Int), (z.map (fun n => (n : Int))).getD (-1)])
    | _ => none⟩,
  ⟨"d3d11.samplespan", fun
    | [bytes, count, k, off, elem] => do
      match sampleSpan (← nat bytes) (← nat count) (← nat k) (← nat off) (← nat elem) with
      | none => pure [-1]
      | some (start, stop) => pure [(start : Int), (stop : Int)]
    | _ => none⟩,
  ⟨"d3d11.depthaction", fun
    | [func, write, enabled, qHi, qLo, storedHi, storedLo, z] => do
      let values := (← nat64 qHi qLo, ← nat64 storedHi storedLo, ← nat z)
      match depthAction (← nat func) (write != 0) (if enabled != 0 then some values else none) with
      | .reject => pure [0]
      | .keep => pure [1]
      | .write depth => pure [2, (depth : Int)]
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
