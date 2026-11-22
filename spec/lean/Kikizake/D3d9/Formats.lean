import Kikizake.Mirror
/-! Direct3D 9's and 8's formats on WARP (spec/d3d9draw.txt D3D9.FORMATS, FORMATS.CHECK,
FORMATS.USAGE): which formats a texture and a render target take, as src/model/d3d9draw.rs
`supported` says, and CheckDeviceFormat's answer for each usage and resource type it is asked,
as `check_hr` (mirrors `d3d9.supported`, `d3d9.check`). -/
namespace Kikizake.D3d9.Formats

def DXT1 : Nat := 827611204
def DXT2 : Nat := 844388420
def DXT3 : Nat := 861165636
def DXT4 : Nat := 877942852
def DXT5 : Nat := 894720068
def UYVY : Nat := 1498831189
def YUY2 : Nat := 844715353

/-- the D3DFORMATs the suites know (TEXTURE_FORMATS) -/
def known : List Nat := [20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 40, 41,
  50, 51, 52, 60, 61, 62, 63, 64, 67, 81, 110, 111, 112, 113, 114, 115, 116, 117,
  DXT1, DXT2, DXT3, DXT4, DXT5, UYVY, YUY2]
def dxt : List Nat := [DXT1, DXT2, DXT3, DXT4, DXT5]
def palette : List Nat := [40, 41]
/-- R8G8B8, R3G3B2, A8R3G3B2, X8B8G8R8, A4L4, L6V5U5, X8L8V8U8, A2W10V10U10, CxV8U8, UYVY, YUY2 -/
def noTex : List Nat := [20, 27, 29, 33, 52, 61, 62, 67, 117, UYVY, YUY2]

/-- D3D9.FORMATS: whether WARP takes the format as a texture (rt false) or a render target -/
def supported (f : Nat) (rt v8 : Bool) : Bool :=
  if noTex.contains f || (v8 && !rt && [35, 36, 81].contains f) || !known.contains f then false
  else !rt || !(palette.contains f || dxt.contains f)

def NOTAVAILABLE : Nat := 0x8876086a
def INVALIDCALL : Nat := 0x8876086c
def NOAUTOGEN : Nat := 0x0876086f

def ok (b : Bool) : Nat := if b then 0 else NOTAVAILABLE

/-- D3D9.FORMATS.CHECK, FORMATS.USAGE: CheckDeviceFormat of a format for a usage (0, RENDERTARGET
1, DEPTHSTENCIL 2, DYNAMIC 0x200, AUTOGENMIPMAP 0x400) and a resource type (SURFACE 1, TEXTURE 3,
VOLUMETEXTURE 4, CUBETEXTURE 5); none for the pairs the suites do not ask -/
def check (f u t : Nat) (v8 : Bool) : Option Nat :=
  let tex := supported f false false
  let isDxt := dxt.contains f
  if v8 && ((u, t) = (0, 1) || (u, t) = (0x400, 3)) then some INVALIDCALL
  else if v8 && (u, t) = (1, 4) then some (ok (tex && !palette.contains f && !isDxt))
  else if v8 && (u, t) = (2, 1) && [35, 36, 81].contains f then some 0
  else match u, t with
  | 0, 3 => some (ok tex)
  | 1, 1 | 1, 3 => some (ok (supported f true false))
  | 0x200, 3 | 0, 4 | 0, 5 => some (ok tex)
  | 0, 1 => some (ok (tex && f != 40 && !isDxt || f == YUY2))
  | 1, 4 => some INVALIDCALL
  | 2, 1 => some (if [75, 77, 80, 82].contains f then 0
      else if [70, 71, 73, 79, 83, 84].contains f then NOTAVAILABLE else INVALIDCALL)
  | 0x400, 3 => some (if !tex then NOTAVAILABLE
      else if isDxt || [22, 23, 40, 41, 60, 63, 64, 110].contains f then NOAUTOGEN else 0)
  | _, _ => none

private theorem targets_known : ∀ f ∈ known, supported f true false =
    (supported f false false && !(palette.contains f || dxt.contains f)) := by decide

/-- D3D9.FORMATS: Direct3D 9 takes as targets the formats it takes as textures but the palette and
DXT ones -/
theorem targets (f : Nat) : supported f true false =
    (supported f false false && !(palette.contains f || dxt.contains f)) := by
  by_cases h : f ∈ known
  · exact targets_known f h
  · simp [supported, h]

private theorem d3d8_known : ∀ f ∈ known, supported f false true =
    (supported f false false && ![35, 36, 81].contains f) ∧ supported f true true = supported f true false := by decide

/-- D3D9.FORMATS: Direct3D 8 takes the textures 9 does but A2R10G10B10, A16B16G16R16 and L16,
and the same targets -/
theorem d3d8 (f : Nat) : supported f false true = (supported f false false && ![35, 36, 81].contains f) ∧
    supported f true true = supported f true false := by
  by_cases h : f ∈ known
  · exact d3d8_known f h
  · simp [supported, h]

/-- D3D9.FORMATS.CHECK: a texture format as a texture, a target one as a target, NOTAVAILABLE
else; Direct3D 8 answers the same (its CreateTexture refuses three more) -/
theorem check_formats (f : Nat) (v8 : Bool) :
    check f 0 3 v8 = some (ok (supported f false false)) ∧ check f 1 1 v8 = some (ok (supported f true false)) ∧
    check f 1 3 v8 = some (ok (supported f true false)) := by
  cases v8 <;> simp [check]

/-- D3D9.FORMATS.USAGE: (9) a DXT format and A8P8 are no plain surface; the depth stencil
surfaces are D16, D24S8, D24X8, D32F_LOCKABLE alone; a render target volume is INVALIDCALL; (8) a
plain surface and AUTOGENMIPMAP INVALIDCALL -/
theorem usage (f : Nat) :
    (dxt.contains f || f == 40 → check f 0 1 false = some NOTAVAILABLE) ∧
    (check f 2 1 false = some 0 ↔ f ∈ [75, 77, 80, 82]) ∧
    check f 1 4 false = some INVALIDCALL ∧
    check f 0 1 true = some INVALIDCALL ∧ check f 0x400 3 true = some INVALIDCALL := by
  refine ⟨fun h => ?_, ?_, ?_, ?_, ?_⟩
  · have hy : f ≠ YUY2 := by
      intro e; subst e; simp [dxt, DXT1, DXT2, DXT3, DXT4, DXT5, YUY2] at h
    have : (supported f false false && f != 40 && !dxt.contains f || f == YUY2) = false := by
      cases hd : dxt.contains f <;> simp_all
    have e : check f 0 1 false = some (ok (supported f false false && f != 40 && !dxt.contains f || f == YUY2)) := rfl
    rw [e, this]; rfl
  · have e : check f 2 1 false = some (if [75, 77, 80, 82].contains f then 0
        else if [70, 71, 73, 79, 83, 84].contains f then NOTAVAILABLE else INVALIDCALL) := rfl
    rw [e]
    by_cases h : f ∈ [75, 77, 80, 82]
    · simp [h]
    · have : ([75, 77, 80, 82] : List Nat).contains f = false := by simpa using h
      simp only [this, h, iff_false, Bool.false_eq_true, ↓reduceIte]
      split <;> simp [NOTAVAILABLE, INVALIDCALL]
  · rfl
  · rfl
  · rfl

def mirrors : List Mirror := [
  ⟨"d3d9.supported", fun
    | [f, rt, v8] => do pure [if supported (← nat f) (rt != 0) (v8 != 0) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d9.check", fun
    | [f, u, t, v8] => do pure [← check (← nat f) (← nat u) (← nat t) (v8 != 0)]
    | _ => none⟩
]

end Kikizake.D3d9.Formats
