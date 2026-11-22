import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's specular add and pixel fog (spec/d3d9draw.txt D3D9.SPECULAR, D3D9.FOG) in binary64
and binary32 (Kikizake.Float) as src/model/d3d9draw.rs `specular_add`, `fog_factor`, `fog_blend`
compute them (mirrors `d3d9.specular`, `d3d9.fogfactor`, `d3d9.fogblend`). EXP and EXP2 take the
exponential the model's libm gives (`e`, an input: the mirror passes Rust's `exp` of the same
argument, which `expArg` writes). -/
namespace Kikizake.D3d9.Fog

open Kikizake.F64 (add sub mul div clamp lt)

def zero64 : Nat := 0
def one64 : Nat := 0x3ff0000000000000

/-- f64::min of non-NaN values -/
def min64 (a b : Nat) : Nat := if lt b a then b else a

/-- D3D9.SPECULAR: the specular color added, to 1 at most -/
def specularAdd (c s : Nat) : Nat := min64 (add c s) one64

/-- the exponential's argument of EXP (mode 1) and EXP2 (mode 2): -(z d), -(z d) (z d) -/
def expArg (mode z d : Nat) : Nat :=
  let zd := mul z d
  let n := Float.neg F64.fmt zd
  if mode = 1 then n else mul n zd

/-- D3D9.FOG: the factor: NONE (0) the specular alpha, EXP and EXP2 the exponential e, LINEAR (3)
(end - z) / (end - start), clamped to 0..1 and taken to binary32; none past LINEAR -/
def factor (mode z sa start fin e : Nat) : Option Nat :=
  let f := match mode with
    | 0 => some sa
    | 1 | 2 => some e
    | 3 => some (div (sub fin z) (sub fin start))
    | _ => none
  f.map fun v => F64.toF32 (clamp v zero64 one64)

/-- D3D9.FOG: a channel f c + (1 - f) fog, in binary32 -/
def blend (f c fog : Nat) : Nat := F32.add (F32.mul f c) (F32.mul (F32.sub 0x3f800000 f) fog)

/-- D3D9.FOG: NONE takes the specular alpha, LINEAR the distance to FOGEND over the fog's span;
FOGTABLEMODE past LINEAR is not a mode -/
theorem factor_modes (z sa start fin e : Nat) :
    factor 0 z sa start fin e = some (F64.toF32 (clamp sa zero64 one64)) ∧
    factor 3 z sa start fin e = some (F64.toF32 (clamp (div (sub fin z) (sub fin start)) zero64 one64)) ∧
    factor 1 z sa start fin e = factor 2 z sa start fin e ∧
    ∀ m, 3 < m → factor m z sa start fin e = none := by
  refine ⟨rfl, rfl, rfl, fun m h => ?_⟩
  unfold factor
  split <;> first | omega | rfl

/-- D3D9.FOG: the blend is symmetric in the weights: f with c and fog is 1 - f's products the
other way round (the two products commute) -/
theorem blend_products (f c fog : Nat) :
    blend f c fog = F32.add (F32.mul c f) (F32.mul fog (F32.sub 0x3f800000 f)) := by
  simp only [blend, F32.mul_comm f c, F32.mul_comm (F32.sub 0x3f800000 f) fog]

/-- D3D9.SPECULAR: adding is commutative, the color and the specular color alike -/
theorem specular_comm (c s : Nat) : specularAdd c s = specularAdd s c := by
  simp only [specularAdd, F64.add_comm c s]

def mirrors : List Mirror := [
  ⟨"d3d9.specular", fun | [a, b, c, d] => do pure (F64.halves (F64.canon (specularAdd (F64.join (← nat a) (← nat b)) (F64.join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"d3d9.fogexp", fun | [m, a, b, c, d] => do pure (F64.halves (F64.canon (expArg (← nat m) (F64.join (← nat a) (← nat b)) (F64.join (← nat c) (← nat d))))) | _ => none⟩,
  ⟨"d3d9.fogfactor", fun
    | [m, za, zb, sa, sb, s0, s1, e0, e1, x0, x1, _, _] => do
      pure ((factor (← nat m) (F64.join (← nat za) (← nat zb)) (F64.join (← nat sa) (← nat sb)) (F64.join (← nat s0) (← nat s1))
        (F64.join (← nat e0) (← nat e1)) (F64.join (← nat x0) (← nat x1))).map (fun v => (F32.canon v : Int))).toList
    | _ => none⟩,
  ⟨"d3d9.fogblend", fun | [f, c, g] => do pure [F32.canon (blend (← nat f) (← nat c) (← nat g))] | _ => none⟩
]

end Kikizake.D3d9.Fog
