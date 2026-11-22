import Kikizake.Mirror
import Kikizake.F32
/-! Direct3D 9's texture stage arguments (D3DTA, spec/d3d9draw.txt D3D9.STAGE.ARG, STAGE.NOTEX), as
src/model/d3d9draw.rs `stage_arg` reads them: binary64 channels holding binary32 values
(Kikizake.Float) (mirror `d3d9.arg`). -/
namespace Kikizake.D3d9.Arg

/-- four channels: red, green, blue, alpha (binary64 bits) -/
abbrev C4 := Nat × Nat × Nat × Nat

def C4.a (c : C4) : Nat := c.2.2.2
def C4.map (f : Nat → Nat) (c : C4) : C4 := (f c.1, f c.2.1, f c.2.2.1, f c.2.2.2)

def one64 : Nat := 0x3ff0000000000000

/-- an 8-bit channel k as k / 255 in binary32, as binary64 -/
def unit (k : Nat) : Nat := F64.ofF32 (F32.div (F32.ofNat k) 0x437f0000)

/-- D3DCOLOR's channels (ARGB) -/
def argb (c : Nat) : C4 := (unit ((c >>> 16) % 256), unit ((c >>> 8) % 256), unit (c % 256), unit ((c >>> 24) % 256))

/-- what the arguments read -/
structure Regs where
  diffuse : C4
  current : C4
  specular : C4
  temp : C4
  tex : Option C4
  factor : Nat
  constant : Nat

/-- D3D9.STAGE.ARG, STAGE.NOTEX: the register, then ALPHAREPLICATE and COMPLEMENT; none for a
selector past CONSTANT, TEMP or CONSTANT on Direct3D 8, or bits past 0x3f -/
def arg (a : Nat) (r : Regs) (v8 : Bool) : Option C4 :=
  let v : Option C4 := match a % 16 with
    | 0 => some r.diffuse
    | 1 => some r.current
    | 2 => some (r.tex.getD (one64, one64, one64, r.current.a))
    | 3 => some (argb r.factor)
    | 4 => some r.specular
    | 5 => if v8 then none else some r.temp
    | 6 => if v8 then none else some (argb r.constant)
    | _ => none
  v.bind fun v =>
    let v := if (a >>> 5) % 2 = 1 then (v.a, v.a, v.a, v.a) else v
    let v := if (a >>> 4) % 2 = 1 then C4.map (F64.sub one64) v else v
    if a / 64 ≠ 0 then none else some v

/-- D3D9.STAGE.NOTEX: the texture argument of a stage without a texture is color 1 with the
current alpha -/
theorem no_texture (r : Regs) (v8 : Bool) (h : r.tex = none) :
    arg 2 r v8 = some (one64, one64, one64, r.current.a) := by
  simp [arg, h]

/-- D3D9.STAGE.ARG: TEMP and CONSTANT are Direct3D 9's: Direct3D 8 takes neither; D3D9 reads the
TEMP register and the stage's CONSTANT -/
theorem temp_constant (r : Regs) :
    arg 5 r true = none ∧ arg 6 r true = none ∧ arg 5 r false = some r.temp ∧
    arg 6 r false = some (argb r.constant) := by
  simp [arg]

/-- D3D9.STAGE.ARG: ALPHAREPLICATE makes every channel the alpha, COMPLEMENT each 1 - x, and
with both the alpha is replicated first -/
theorem modifiers (r : Regs) (v8 : Bool) :
    arg 0x20 r v8 = some (r.diffuse.a, r.diffuse.a, r.diffuse.a, r.diffuse.a) ∧
    arg 0x10 r v8 = some (C4.map (F64.sub one64) r.diffuse) ∧
    arg 0x30 r v8 = some (C4.map (F64.sub one64) (r.diffuse.a, r.diffuse.a, r.diffuse.a, r.diffuse.a)) ∧
    arg 0x40 r v8 = none := by
  simp [arg]

def c4 (l : List Int) (i : Nat) : Option C4 := do
  let g (k : Nat) : Option Nat := do F64.join (← nat (← l[i + 2 * k]?)) (← nat (← l[i + 2 * k + 1]?))
  pure (← g 0, ← g 1, ← g 2, ← g 3)

def mirrors : List Mirror := [
  ⟨"d3d9.arg", fun l => do
    let a ← nat (← l[0]?)
    let v8 := (← l[1]?) != 0
    let t ← c4 l 35
    let r : Regs := ⟨← c4 l 2, ← c4 l 10, ← c4 l 18, ← c4 l 26,
      if (← l[34]?) != 0 then some t else none, ← nat (← l[43]?), ← nat (← l[44]?)⟩
    match arg a r v8 with
    | some v => pure (F64.halves v.1 ++ F64.halves v.2.1 ++ F64.halves v.2.2.1 ++ F64.halves v.2.2.2)
    | none => pure []⟩
]

end Kikizake.D3d9.Arg
