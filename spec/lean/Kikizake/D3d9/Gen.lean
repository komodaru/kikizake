import Kikizake.Mirror
import Kikizake.F32
import Kikizake.D3d9.Xyz
/-! Direct3D 9's generated texture coordinates (spec/d3d9draw.txt D3D9.STAGE.TCIGEN) and the
footprint of projected ones (STAGE.PROJFOOT), as src/model/d3d9draw.rs `camera_space`,
`camera_normal`, `tex_gen` (binary32, Kikizake.F32), `quad_origin` and `quad_duv` (binary64,
Kikizake.F64) compute them (mirrors `d3d9.camera`, `d3d9.normal`, `d3d9.texgen`, `d3d9.quad`,
`d3d9.quadduv`). -/
namespace Kikizake.D3d9.Gen

open Kikizake.F32 (add sub mul div)
open Kikizake.D3d9.Clip (zero half)
open Kikizake.D3d9.Line (one)
open Kikizake.D3d9.Xyz (xv two)

/-- D3D9.STAGE.TCIGEN: the camera-space position: the row (p, 1) times WORLD, then VIEW -/
def cameraSpace (p world view : List Nat) : List Nat := xv (xv [p.getD 0 0, p.getD 1 0, p.getD 2 0, one] world) view

/-- the sum from 0 of f k for k = 0, 1, 2 in turn, in f32 -/
def sum3 (f : Nat → Nat) : Nat := (List.range 3).foldl (fun s k => add s (f k)) zero

/-- D3D9.STAGE.TCIGEN: the normal through the inverse transpose of WORLD VIEW's upper 3 x 3 (its
cofactors over the determinant), normalized when asked -/
def cameraNormal (n a b : List Nat) (normalize : Bool) : List Nat :=
  let wv := (List.range 9).map fun i => sum3 fun k => mul (a.getD (i / 3 * 4 + k) 0) (b.getD (k * 4 + i % 3) 0)
  let e := fun r c => wv.getD (r * 3 + c) 0
  let cof := (List.range 9).map fun i =>
    let r := i / 3
    let c := i % 3
    sub (mul (e ((r + 1) % 3) ((c + 1) % 3)) (e ((r + 2) % 3) ((c + 2) % 3)))
      (mul (e ((r + 1) % 3) ((c + 2) % 3)) (e ((r + 2) % 3) ((c + 1) % 3)))
  let det := add (add (mul (e 0 0) (cof.getD 0 0)) (mul (e 0 1) (cof.getD 1 0))) (mul (e 0 2) (cof.getD 2 0))
  let t := (List.range 3).map fun j => div (sum3 fun k => mul (n.getD k 0) (cof.getD (k * 3 + j) 0)) det
  if normalize then
    let l := F32.sqrt (add (add (mul (t.getD 0 0) (t.getD 0 0)) (mul (t.getD 1 0) (t.getD 1 0))) (mul (t.getD 2 0) (t.getD 2 0)))
    t.map fun x => div x l
  else t

/-- D3D9.STAGE.TCIGEN: the coordinates of mode g (0 passed through, 1 CAMERASPACENORMAL, 2
CAMERASPACEPOSITION, 3 CAMERASPACEREFLECTIONVECTOR, 4 SPHEREMAP) from camera-space c and normal nn,
as the row (g, 1): R = 2 (E.N) N - E, E = -P / |P| under LOCALVIEWER, else (0, 0, 1); SPHEREMAP
(Rx / m + 1/2, Ry / m + 1/2), m = 2 sqrt(Rx^2 + Ry^2 + (Rz - 1)^2) -/
def texGen (g : Nat) (c nn : List Nat) (local_ : Bool) : List Nat :=
  let c0 := c.getD 0 0
  let c1 := c.getD 1 0
  let c2 := c.getD 2 0
  let n0 := nn.getD 0 0
  let n1 := nn.getD 1 0
  let n2 := nn.getD 2 0
  let pl := F32.sqrt (add (add (mul c0 c0) (mul c1 c1)) (mul c2 c2))
  let neg := Float.neg F32.fmt
  let (e0, e1, e2) := if local_ then (div (neg c0) pl, div (neg c1) pl, div (neg c2) pl) else (zero, zero, one)
  let en := add (add (mul e0 n0) (mul e1 n1)) (mul e2 n2)
  let r0 := sub (mul (mul two en) n0) e0
  let r1 := sub (mul (mul two en) n1) e1
  let r2 := sub (mul (mul two en) n2) e2
  let gs : List Nat := match g with
    | 0 => [zero, zero, zero]
    | 1 => [n0, n1, n2]
    | 2 => [c0, c1, c2]
    | 3 => [r0, r1, r2]
    | _ =>
      let m := mul two (F32.sqrt (add (add (mul r0 r0) (mul r1 r1)) (mul (sub r2 one) (sub r2 one))))
      [add (div r0 m) half, add (div r1 m) half, zero]
  gs ++ [one]

/-- D3D9.STAGE.TCIGEN: CAMERASPACENORMAL is the normal, CAMERASPACEPOSITION the position -/
theorem normal_position (c nn : List Nat) (l : Bool) :
    texGen 1 c nn l = [nn.getD 0 0, nn.getD 1 0, nn.getD 2 0, one] ∧ texGen 2 c nn l = [c.getD 0 0, c.getD 1 0, c.getD 2 0, one] := by
  simp [texGen]

/-- D3D9.STAGE.TCIGEN: the position is through WORLD first, then VIEW -/
theorem world_then_view (p world view : List Nat) :
    cameraSpace p world view = xv (xv [p.getD 0 0, p.getD 1 0, p.getD 2 0, one] world) view := rfl

/-- D3D9.STAGE.TCIGEN: without LOCALVIEWER the reflection is about E = (0, 0, 1) -/
theorem reflect_infinite (c nn : List Nat) :
    texGen 3 c nn false = [sub (mul (mul two (add (add (mul zero (nn.getD 0 0)) (mul zero (nn.getD 1 0))) (mul one (nn.getD 2 0)))) (nn.getD 0 0)) zero,
      sub (mul (mul two (add (add (mul zero (nn.getD 0 0)) (mul zero (nn.getD 1 0))) (mul one (nn.getD 2 0)))) (nn.getD 1 0)) zero,
      sub (mul (mul two (add (add (mul zero (nn.getD 0 0)) (mul zero (nn.getD 1 0))) (mul one (nn.getD 2 0)))) (nn.getD 2 0)) one, one] := by
  simp [texGen]

/-- D3D9.STAGE.PROJFOOT: the offset from pixel (px, py) to its 2 x 2 quad's top-left pixel -/
def quadOrigin (px py : Int) : Int × Int := (if px % 2 = 0 then 0 else -1, if py % 2 = 0 then 0 else -1)

/-- D3D9.STAGE.PROJFOOT: all four pixels of a quad take their footprint from one pixel -/
theorem quad_shared (px py : Int) :
    let o := quadOrigin px py
    px + o.1 = 2 * (px / 2) ∧ py + o.2 = 2 * (py / 2) := by
  simp only [quadOrigin]
  constructor <;> split <;> omega

/-- the sets' (s/q, t/q) of an attribute vector: sets at 9, 12, 14, their q at 16, 17, 18 (f64) -/
def stOf (at' : List Nat) : List (Nat × Nat) :=
  [(9, 16), (12, 17), (14, 18)].map fun (u, q) => (F64.div (at'.getD u 0) (at'.getD q 0), F64.div (at'.getD (u + 1) 0) (at'.getD q 0))

/-- D3D9.STAGE.PROJFOOT: each set's footprint: the top-left pixel's (s/q, t/q) against its right
(r) and lower (d) neighbours', ((ds/dx, ds/dy), (dt/dx, dt/dy)) -/
def quadDuv (c r d : List Nat) : List ((Nat × Nat) × (Nat × Nat)) :=
  ((stOf c).zip ((stOf r).zip (stOf d))).map fun (c, r, d) => ((F64.sub r.1 c.1, F64.sub d.1 c.1), (F64.sub r.2 c.2, F64.sub d.2 c.2))

/-- D3D9.STAGE.PROJFOOT: the footprint is of the projected coordinates s/q, t/q -/
theorem projected (c r d : List Nat) (i : Nat) :
    (quadDuv c r d)[i]? = (((stOf c).zip ((stOf r).zip (stOf d)))[i]?).map fun (c, r, d) =>
      ((F64.sub r.1 c.1, F64.sub d.1 c.1), (F64.sub r.2 c.2, F64.sub d.2 c.2)) := by
  simp [quadDuv]

theorem three_sets (c r d : List Nat) : (quadDuv c r d).length = 3 := by simp [quadDuv, stOf]

private def mat16 (ws : List Nat) (k : Nat) : List Nat := (ws.drop (16 * k)).take 16

def mirrors : List Mirror := [
  ⟨"d3d9.camera", fun
    | px :: py :: pz :: ms => do
      let m ← ms.mapM nat
      if m.length != 32 then none
      pure ((cameraSpace [← nat px, ← nat py, ← nat pz] (mat16 m 0) (mat16 m 1)).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.normal", fun
    | nm :: n0 :: n1 :: n2 :: ms => do
      let m ← ms.mapM nat
      if m.length != 32 then none
      pure ((cameraNormal [← nat n0, ← nat n1, ← nat n2] (mat16 m 0) (mat16 m 1) (nm != 0)).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.texgen", fun
    | [g, l, c0, c1, c2, c3, n0, n1, n2] => do
      pure ((texGen (← nat g) [← nat c0, ← nat c1, ← nat c2, ← nat c3] [← nat n0, ← nat n1, ← nat n2] (l != 0)).map fun b => (F32.canon b : Int))
    | _ => none⟩,
  ⟨"d3d9.quad", fun
    | [px, py] => do let o := quadOrigin px py; pure [o.1, o.2]
    | _ => none⟩,
  ⟨"d3d9.quadduv", fun
    | ws => do
      let ns ← ws.mapM nat
      if ns.length != 114 then none
      let rec joins : List Nat → List Nat
        | h :: l :: r => F64.join h l :: joins r
        | _ => []
      let vs := joins ns
      let out := quadDuv (vs.take 19) ((vs.drop 19).take 19) (vs.drop 38)
      pure (out.flatMap fun ((a, b), (c, d)) => [a, b, c, d].flatMap fun x => F64.halves (F64.canon x))⟩
]

end Kikizake.D3d9.Gen
