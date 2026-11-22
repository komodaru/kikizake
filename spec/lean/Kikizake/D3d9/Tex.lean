import Kikizake.Mirror
/-! Direct3D 9's sampler filters, the checks before a draw, palette texels and two-sided
stencil, as src/model/d3d9draw.rs computes them: `sampleFilter` `sample_filter` and `d3d10Setup`
`d3d10_setup` (D3DTEXTUREFILTERTYPE: D3D9.TEX.MAGONLY, D3D9.TEX.FILTEROTHER, D3D9.TEX.QUAD),
`gate` `draw_gate` (D3D9.BLEND.OPX86, D3D9.TEX.ADDRESSX86, D3D9.TEX.PALETTE.NONE),
`paletteTexel` `palette_texel` (D3D9.TEX.PALETTE), `stencilStates` `stencil_states`
(D3D9.STENCIL.TWOSIDED) (mirrors `d3d9.filter`, `d3d9.gate`, `d3d9.palette`, `d3d9.stencil2`). -/
namespace Kikizake.D3d9.Tex

/-- a D3DTEXTUREFILTERTYPE as WARP takes it -/
inductive TF where
  | none | point | linear | aniso | quad | other
  deriving DecidableEq, Repr

/-- NONE, POINT, LINEAR, ANISOTROPIC; Direct3D 9's PYRAMIDALQUAD and GAUSSIANQUAD (4, 5);
CONVOLUTIONMONO, values past them and Direct3D 8's FLATCUBIC, GAUSSIANCUBIC (4, 5) -/
def tf (v : Nat) (v8 : Bool) : TF :=
  if v = 0 then .none else if v = 1 then .point else if v = 2 then .linear
  else if v = 3 then .aniso else if (v = 4 ∨ v = 5) ∧ v8 = false then .quad else .other

/-- the filter a texture of one level is sampled with: a MINFILTER that makes all of it POINT
(Direct3D 8's NONE and cubic ones, Direct3D 9's CONVOLUTIONMONO and past) or ANISOTROPIC, else
MAGFILTER's alone, the quad ones as LINEAR, NONE and the others as POINT -/
def sampleFilter (mag min : Nat) (v8 : Bool) : TF :=
  let m := match tf min v8 with
    | .other => TF.point
    | .none => if v8 then .point else tf mag v8
    | .aniso => .aniso
    | _ => tf mag v8
  match m with
  | .quad => .linear
  | .none | .other => .point
  | f => f

/-- Direct3D 10's attribute setup: a quad filter as MAGFILTER, or (9) a quad one or NONE as
MINFILTER -/
def d3d10Setup (mag min : Nat) (v8 : Bool) : Bool :=
  tf mag v8 == .quad || (!v8 && (tf min v8 == .quad || tf min v8 == .none))

private theorem tf_big {v : Nat} (b : Bool) (h : 6 ≤ v) : tf v b = .other := by
  have : ¬(v = 4 ∨ v = 5) := by omega
  simp only [tf, show ¬v = 0 by omega, show ¬v = 1 by omega, show ¬v = 2 by omega, show ¬v = 3 by omega,
    this, false_and, ↓reduceIte]

/-- a filter value is 0 .. 5 or past them, and past them it is CONVOLUTIONMONO's -/
private theorem seven (v : Nat) : (v = 0 ∨ v = 1 ∨ v = 2 ∨ v = 3 ∨ v = 4 ∨ v = 5) ∨ (6 ≤ v ∧ ∀ b, tf v b = .other) := by
  by_cases h : 6 ≤ v
  · exact .inr ⟨h, fun b => tf_big b h⟩
  · omega

-- every case of `mag` and `min` (0 .. 5, past them) and `v8`
set_option hygiene false in
macro "filters" : tactic => `(tactic| (
  rcases seven mag with hm | ⟨hm, tm⟩ <;> rcases seven min with hn | ⟨hn, tn⟩ <;>
  (try rcases hm with rfl | rfl | rfl | rfl | rfl | rfl) <;>
  (try rcases hn with rfl | rfl | rfl | rfl | rfl | rfl) <;>
  (try cases v8) <;>
  simp only [sampleFilter, d3d10Setup] <;> (try simp only [tm]) <;> (try simp only [tn]) <;>
  (try simp [tf]) <;> (try omega)))

/-- D3D9.TEX.MAGONLY: what a texture of one level is sampled with is POINT, LINEAR or
ANISOTROPIC, and MINFILTER POINT or LINEAR takes no part (MAGFILTER's own; Direct3D 9's NONE and
quad ones as MINFILTER neither) -/
theorem mag_only (mag : Nat) (v8 : Bool) (min : Nat) :
    (sampleFilter mag min v8 = .point ∨ sampleFilter mag min v8 = .linear ∨ sampleFilter mag min v8 = .aniso) ∧
    sampleFilter mag 1 v8 = sampleFilter mag 2 v8 ∧
    sampleFilter mag 0 false = sampleFilter mag 1 false ∧ sampleFilter mag 4 false = sampleFilter mag 1 false ∧
    sampleFilter mag 5 false = sampleFilter mag 1 false ∧
    sampleFilter 1 1 v8 = .point ∧ sampleFilter 2 1 v8 = .linear ∧ sampleFilter 3 1 v8 = .aniso ∧
    sampleFilter mag 3 v8 = .aniso := by
  filters

/-- D3D9.TEX.FILTEROTHER: NONE, CONVOLUTIONMONO and past as MAGFILTER filter as POINT, and so do
Direct3D 8's cubic ones (4, 5); Direct3D 8's NONE, 4, 5 and past as MINFILTER make all of it
POINT, and so do Direct3D 9's CONVOLUTIONMONO and past; Direct3D 9's NONE as MINFILTER leaves
MAGFILTER's filter with Direct3D 10's setup -/
theorem filter_other (mag min : Nat) (v8 : Bool) :
    ((mag = 0 ∨ 6 ≤ mag) → (min = 1 ∨ min = 2) → sampleFilter mag min v8 = .point) ∧
    ((mag = 4 ∨ mag = 5) → (min = 1 ∨ min = 2) → sampleFilter mag min true = .point) ∧
    ((min = 0 ∨ 4 ≤ min) → sampleFilter mag min true = .point) ∧
    (6 ≤ min → sampleFilter mag min false = .point) ∧
    (sampleFilter mag 0 false = sampleFilter mag 1 false ∧ d3d10Setup mag 0 false = true) := by
  filters

/-- D3D9.TEX.QUAD: Direct3D 9's PYRAMIDALQUAD and GAUSSIANQUAD as MAGFILTER filter as LINEAR, as
MINFILTER leave MAGFILTER's filter, and either way set the attributes up as Direct3D 10 does;
Direct3D 8 has neither -/
theorem quad (mag min : Nat) :
    ((mag = 4 ∨ mag = 5) → (min = 1 ∨ min = 2) → sampleFilter mag min false = .linear ∧ d3d10Setup mag min false = true) ∧
    ((min = 4 ∨ min = 5) → sampleFilter mag min false = sampleFilter mag 1 false ∧ d3d10Setup mag min false = true) ∧
    ((mag = 1 ∨ mag = 2 ∨ mag = 3) → (min = 1 ∨ min = 2 ∨ min = 3) → d3d10Setup mag min false = false) ∧
    (d3d10Setup mag min true = false) := by
  filters

/-- what d9draw does before it draws -/
inductive Gate where
  | draw | blendOpX86 (op : Nat) | addressX86 (a : Nat) | noPalette
  deriving DecidableEq, Repr

/-- a value of the enumerations ADD..MAX and WRAP..MIRRORONCE (1..5) -/
def named (v : Nat) : Bool := 1 ≤ v && v ≤ 5

/-- x86: a BLENDOP outside ADD..MAX, then the first address mode of a textured stage outside
WRAP..MIRRORONCE, ends it before drawing; then a palette texture with no palette -/
def gate (x86 : Bool) (op : Nat) (addrs : List Nat) (pal ptex : Bool) : Gate :=
  if x86 && !named op then .blendOpX86 op
  else match addrs.find? (fun a => x86 && !named a) with
    | some a => .addressX86 a
    | none => if !pal && ptex then .noPalette else .draw

/-- D3D9.BLEND.OPX86: on x86 a BLENDOP outside ADD..MAX (0, 6, 7 asked) does not draw, the
BLENDOP read back; x64 draws whatever BLENDOP and address modes -/
theorem blend_op_x86 (op : Nat) (addrs : List Nat) (pal ptex : Bool) :
    ((op = 0 ∨ 5 < op) → gate true op addrs pal ptex = .blendOpX86 op) ∧
    gate false op addrs pal ptex = (if !pal && ptex then .noPalette else .draw) := by
  refine ⟨fun h => ?_, ?_⟩
  · have : named op = false := by simp [named]; omega
    simp [gate, this]
  · have : addrs.find? (fun _ => false) = none := List.find?_eq_none.2 (by simp)
    simp [gate, this]

/-- D3D9.TEX.ADDRESSX86: on x86 with a BLENDOP of the enumeration, an address mode outside
WRAP..MIRRORONCE does not draw, the first such read back; with every mode in it the address
modes do not stop it -/
theorem address_x86 (op : Nat) (addrs : List Nat) (pal ptex : Bool) (h : named op = true) :
    (∀ a, gate true op addrs pal ptex = .addressX86 a → a ∈ addrs ∧ named a = false) ∧
    ((∀ a ∈ addrs, named a = true) → gate true op addrs pal ptex = (if !pal && ptex then .noPalette else .draw)) ∧
    (named 0 = false ∧ named 6 = false ∧ ∀ m, named m = true ↔ 1 ≤ m ∧ m ≤ 5) := by
  refine ⟨fun a ha => ?_, fun hall => ?_, by simp [named]⟩
  · simp only [gate, h, Bool.not_true, Bool.and_false, Bool.false_eq_true, ↓reduceIte] at ha
    split at ha
    · rename_i b hb
      cases ha
      have := List.find?_some hb
      exact ⟨List.mem_of_find?_eq_some hb, by simpa using this⟩
    · split at ha <;> cases ha
  · have : addrs.find? (fun a => !named a) = none := by
      rw [List.find?_eq_none]; intro a ha; simp [hall a ha]
    simp [gate, h, this]

/-- D3D9.TEX.PALETTE.NONE: past the x86 checks, a palette texture before any palette is set fails
the draw (nothing drawn); with a palette set or no palette texture it draws -/
theorem palette_none (x86 : Bool) (op : Nat) (addrs : List Nat) (pal ptex : Bool)
    (h : gate x86 op addrs pal ptex ≠ .blendOpX86 op ∧ ∀ a, gate x86 op addrs pal ptex ≠ .addressX86 a) :
    gate x86 op addrs pal ptex = (if !pal && ptex then .noPalette else .draw) := by
  revert h
  simp only [gate]
  split
  · simp
  · split
    · rename_i a _; intro h; exact absurd rfl (h.2 a)
    · simp

/-- a palette texel: the entry's color (r, g, b, flags), the alpha A8P8's (40) own byte, P8's
the entry's flags -/
def paletteTexel (fmt a8 r g b flags : Nat) : List Nat := [r, g, b, if fmt = 40 then a8 else flags]

/-- D3D9.TEX.PALETTE: a P8 texel is its entry's color with the entry's flags as alpha, an A8P8
texel the entry's color with its own alpha -/
theorem palette (a8 r g b flags : Nat) :
    paletteTexel 41 a8 r g b flags = [r, g, b, flags] ∧ paletteTexel 40 a8 r g b flags = [r, g, b, a8] := by
  simp [paletteTexel]

/-- the stencil states (FAIL, ZFAIL, PASS, FUNC) a pixel takes -/
def stencilStates (v8 : Bool) (twoSided : Nat) (ccw : Bool) : List Nat :=
  if !v8 && twoSided ≠ 0 && ccw then [186, 187, 188, 189] else [53, 54, 55, 56]

/-- D3D9.STENCIL.TWOSIDED: (9) under TWOSIDEDSTENCILMODE a counterclockwise triangle is tested
and written with the CCW states, a clockwise one with the others; Direct3D 8 has no two-sided
stencil -/
theorem two_sided (t : Nat) (ccw : Bool) :
    (t ≠ 0 → stencilStates false t true = [186, 187, 188, 189]) ∧ stencilStates false t false = [53, 54, 55, 56] ∧
    stencilStates false 0 ccw = [53, 54, 55, 56] ∧ stencilStates true t ccw = [53, 54, 55, 56] := by
  refine ⟨fun h => ?_, ?_, ?_, ?_⟩ <;> simp_all [stencilStates]

def tfCode : TF → Int
  | .none => 0 | .point => 1 | .linear => 2 | .aniso => 3 | .quad => 4 | .other => 5

def gateCode : Gate → List Int
  | .draw => [0, 0]
  | .blendOpX86 op => [1, op]
  | .addressX86 a => [2, a]
  | .noPalette => [3, 0]

def mirrors : List Mirror := [
  ⟨"d3d9.filter", fun
    | [mag, min, v8] => do
      let (a, b) := (← nat mag, ← nat min)
      pure [tfCode (sampleFilter a b (v8 != 0)), if d3d10Setup a b (v8 != 0) then 1 else 0]
    | _ => none⟩,
  ⟨"d3d9.gate", fun
    | x86 :: op :: pal :: ptex :: addrs => do
      pure (gateCode (gate (x86 != 0) (← nat op) (← addrs.mapM nat) (pal != 0) (ptex != 0)))
    | _ => none⟩,
  ⟨"d3d9.palette", fun
    | [f, a8, r, g, b, fl] => do
      pure ((paletteTexel (← nat f) (← nat a8) (← nat r) (← nat g) (← nat b) (← nat fl)).map Int.ofNat)
    | _ => none⟩,
  ⟨"d3d9.stencil2", fun
    | [v8, t, ccw] => do pure ((stencilStates (v8 != 0) (← nat t) (ccw != 0)).map Int.ofNat)
    | _ => none⟩
]

end Kikizake.D3d9.Tex
