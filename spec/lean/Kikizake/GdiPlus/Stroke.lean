import Kikizake.GdiPlus.Flatten
/-! How GDI+ chooses a stroke (spec/gdiplus.txt GP.PEN.ONEPIXEL, GP.PEN.WORLD) as
src/model/gppath.rs computes it: `sqrt32` is `sqrt32` (sqrtss to nearest, or the largest binary32
at most the root rounding down), `axes` GetMajorAndMinorAxis, `stroke` DpDriver::StrokePath's choice
of the one-pixel line, the half pixel diamond, the dashed one-pixel line or the widened outline; on
binary32 bits rounded as the executor rounds (GdiPlus.Matrix); mirrors `gp.axes`, `gp.stroke`. -/
namespace Kikizake.GdiPlus.Stroke
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten

private abbrev F := F32.fmt

/-- the integer square root (floor), by Newton's method from above -/
def isqrtGo : Nat → Nat → Nat → Nat
  | 0, _, x => x
  | fuel + 1, n, x => let y := (x + n / x) / 2; if y < x then isqrtGo fuel n y else x
def isqrt (n : Nat) : Nat := if n = 0 then 0 else isqrtGo 300 n (2 ^ (Nat.log2 n / 2 + 1))

/-- sqrtss: the binary32 nearest the square root (d false) or the largest at most it (d true); a
NaN or a value below -0 a NaN, ±0 and +infinity themselves -/
def sqrt32 (d : Bool) (x : Nat) : Nat :=
  if isNaN F x then qnan F
  else if isZero x then x
  else if sign F x then qnan F
  else if isInf F x then x
  else
    let e := ex F x
    let s := sig F x
    let (n, e) := if e % 2 = 0 then (s, e) else (2 * s, e - 1)
    -- (the root of n 2^60 has 30 bits or more past the 24 kept: an inexact root rounds as the
    -- midpoint of its two integers)
    let N := n * 2 ^ 60
    let q := isqrt N
    let (a, b) := if q * q = N then (q, 1) else (2 * q + 1, 2)
    let (pn, pd) := pow2 (e / 2 - 30)
    if d then roundD false (a * pn) (b * pd) else round F false (a * pn) (b * pd)

/-- GP.PEN.WORLD (GetMajorAndMinorAxis): the axes a matrix stretches a unit circle to, each at least
5.96e-4 -/
def axes (d : Bool) (m : List Nat) : Nat × Nat :=
  let r := ops d
  let e (i : Nat) := m.getD i 0
  let sq (v : Nat) := r.mul v v
  let a := r.add (sq (e 1)) (sq (e 0))
  let b := r.add (r.mul (e 3) (e 1)) (r.mul (e 2) (e 0))
  let hf := r.mul (r.sub a (r.add (sq (e 2)) (sq (e 3)))) half
  let r2 := r.add (sq b) (sq hf)
  let rr := if flt 0 r2 then sqrt32 d r2 else r2
  let mean := r.mul (r.add (r.add (sq (e 2)) a) (sq (e 3))) half
  (fmax near (sqrt32 d (r.add mean rr)), fmax near (sqrt32 d (r.sub mean rr)))

/-- 1.5 -/
def k15 : Nat := 0x3fc00000

/-- a pen as the stroke choice reads it -/
structure Pen where
  color : Nat
  width : Nat
  dash : Int
  startCap : Nat
  endCap : Nat

/-- GP.PEN.ONEPIXEL, GP.PEN.WORLD: 0 the one-pixel line (opaque), 1 the half pixel diamond (not
opaque), 2 the dashed one-pixel line, 3 the widened outline; a pen at most 1.5 wide in device
space (its width times the world matrix's major axis unless that only moves) with no custom cap
-/
def stroke (d : Bool) (p : Pen) (world : Option (List Nat)) (aliased dashed : Bool) : Nat :=
  let r := ops d
  let device := match world with
    | some m => if complexity r m ≥ 2 then r.mul p.width (axes d m).1 else p.width
    | none => p.width
  let custom := (p.startCap ||| p.endCap) &&& 0xf0 ≠ 0
  let thin := !isNaN F device && !flt k15 device && !custom
  let solid := p.color / 2 ^ 24 % 256 = 255
  if thin && p.dash = 0 then (if solid then 0 else 1)
  else if solid && thin && dashed && aliased then 2
  else 3

/-- GP.PEN.WORLD: a scale by 2 has axes 2 and 2, a shear [1 1 0 1] the golden ratio and its
inverse (x86 rounding to nearest; rounding down the major a unit below, the minor, a difference, a
unit above); a 1.5 wide pen is one pixel
untransformed, widened under that scale -/
theorem axes_scale :
    axes false [0x40000000, 0, 0, 0x40000000, 0, 0] = (0x40000000, 0x40000000) ∧
    axes false [one, one, 0, one, 0, 0] = (0x3fcf1bbd, 0x3f1e377a) ∧
    axes true [one, one, 0, one, 0, 0] = (0x3fcf1bbc, 0x3f1e377b) ∧
    stroke false ⟨0xff000000, k15, 0, 0, 0⟩ none true false = 0 ∧
    stroke false ⟨0xff000000, k15, 0, 0, 0⟩ (some [0x40000000, 0, 0, 0x40000000, 0, 0]) true false = 3 := by decide +kernel

/-- GP.PEN.ONEPIXEL: a thin pen is one pixel wide when opaque, the diamond when not, the dashed
one-pixel line when dashed, opaque and aliased, widened with a custom cap; 0 and negative widths
are thin -/
theorem choices :
    stroke false ⟨0x80000000, one, 0, 0, 0⟩ none true false = 1 ∧
    stroke true ⟨0xff000000, one, 1, 0, 0⟩ none true true = 2 ∧
    stroke true ⟨0xff000000, one, 1, 0, 0⟩ none false true = 3 ∧
    stroke true ⟨0xff000000, one, 0, 0x10, 0⟩ none true false = 3 ∧
    stroke false ⟨0xff000000, 0, 0, 0, 0⟩ none true false = 0 ∧
    stroke false ⟨0xff000000, 0xc0000000, 0, 0, 0⟩ none true false = 0 := by decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.sqrt32", fun | [d, x] => do pure [F32.canon (sqrt32 (d != 0) (← nat x))] | _ => none⟩,
  ⟨"gp.axes", fun
    | d :: m => do
      let m ← m.mapM nat
      let (a, b) := axes (d != 0) m
      pure [F32.canon a, F32.canon b]
    | _ => none⟩,
  ⟨"gp.stroke", fun
    | [d, color, width, dash, sc, ec, aliased, dashed, hasW, m0, m1, m2, m3, m4, m5] => do
      let m ← [m0, m1, m2, m3, m4, m5].mapM nat
      let p : Pen := ⟨← nat color, ← nat width, dash, ← nat sc, ← nat ec⟩
      pure [stroke (d != 0) p (if hasW = 0 then none else some m) (aliased != 0) (dashed != 0)]
    | _ => none⟩
]

end Kikizake.GdiPlus.Stroke
