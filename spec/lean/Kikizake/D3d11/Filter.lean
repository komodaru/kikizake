import Kikizake.Mirror
import Kikizake.F32
/-! The float arithmetic of Direct3D 10 and 11's texture filtering (spec/d3dres.txt
D3DRASTER.FILTER.LINEAR, POINT, FLOAT, LEVEL, MIPLINEAR, SAMPLE.CMP) and the depth view's
bounds (DEPTH.SMALL), in binary32 and binary64 (Kikizake.F32) as src/model/d3draster.rs computes
them: `split`, `pointTexel`, `lerpSse`, `pixels`, `linearFloat`, `level`, `sampleCmp`,
`pastDepth` are its functions of the same names (mirrors `d3d11.split`, `d3d11.pointtexel`,
`d3d11.lerp`, `d3d11.pixels`, `d3d11.linearfloat`, `d3d11.level`, `d3d11.samplecmp`,
`d3d11.pastdepth`). A NaN an operation makes is SSE's default NaN, 0xffc00000; a NaN operand is
passed on quieted. -/
namespace Kikizake.D3d11.Filter

open Kikizake.Float (isNaN isInf sign toRat sbit)

/-- Rust's `floor` of a binary float: the largest whole value not above it (-0 stays -0, a NaN
and the infinities stay) -/
def floorF (F : Float.Fmt) (x : Nat) : Nat :=
  if isNaN F x || isInf F x then x
  else
    let (s, n, d) := toRat F x
    let k : Nat := if s then (n + d - 1) / d else n / d
    if k = 0 then (if s then sbit F else 0)
    else Float.round F s k 1

/-- Rust's `trunc`: the whole value toward zero (the sign of a zero kept) -/
def truncF (F : Float.Fmt) (x : Nat) : Nat :=
  if isNaN F x || isInf F x then x
  else
    let (s, n, d) := toRat F x
    if n / d = 0 then (if s then sbit F else 0) else Float.round F s (n / d) 1

/-- Rust's `round`: the nearest whole value, halves away from zero -/
def roundF (F : Float.Fmt) (x : Nat) : Nat :=
  if isNaN F x || isInf F x then x
  else
    let (s, n, d) := toRat F x
    let k := (2 * n + d) / (2 * d)
    if k = 0 then (if s then sbit F else 0) else Float.round F s k 1

/-- Rust's `as` from a float to an integer type of `lo..hi`: toward zero, saturated, a NaN 0 -/
def castInt (F : Float.Fmt) (lo hi : Int) (x : Nat) : Int :=
  if isNaN F x then 0
  else if isInf F x then (if sign F x then lo else hi)
  else
    let (s, n, d) := toRat F x
    let v : Int := if s then -((n / d : Nat) : Int) else ((n / d : Nat) : Int)
    max lo (min hi v)

def I64 (x : Nat) : Int := castInt F32.fmt (-(2 ^ 63)) (2 ^ 63 - 1) x

def half : Nat := 0x3f000000
def one : Nat := 0x3f800000
def f256 : Nat := 0x43800000
def dhalf : Nat := 0x3fe0000000000000
def d256 : Nat := 0x4070000000000000
/-- SSE's default NaN -/
def defaultNaN : Nat := 0xffc00000

/-- an SSE operation's result: its first NaN operand quieted, else the result (a NaN it makes
the default NaN) -/
def sse (x y r : Nat) : Nat :=
  if isNaN F32.fmt x then x ||| 0x400000
  else if isNaN F32.fmt y then y ||| 0x400000
  else if isNaN F32.fmt r then defaultNaN else r

/-! ## D3DRASTER.FILTER.LINEAR, FILTER.POINT -/

/-- a coordinate's first texel and the next's weight in 1/256: s = c n - 0.5 in f32, floor(s),
floor(frac(s) 256) -/
def split (c n : Nat) : Int × Int :=
  let s := F32.sub (F32.mul c (F32.ofNat n)) half
  let i := floorF F32.fmt s
  (I64 i, I64 (floorF F32.fmt (F32.mul (F32.sub s i) f256)))

/-- point sampling's texel: floor(c n), the product in f32 -/
def pointTexel (c n : Nat) : Int := I64 (floorF F32.fmt (F32.mul c (F32.ofNat n)))

/-- 8-bit UNORM's filtered value n (in 1/128 of a byte) as a float: the float nearest
n ceil(2^46 / 32640) / 2^46 -/
def linearFloat (n : Nat) : Nat := Float.round F32.fmt false (n * (2 ^ 46 / 32640 + 1)) (2 ^ 46)

/-! ## D3DRASTER.FILTER.FLOAT, MIPLINEAR -/

/-- a weight in 1/256 as a float -/
def pixels (v : Nat) : Nat := F32.div (F32.ofNat v) f256

/-- a + (b - a) t as SSE computes it, NaNs passed on -/
def lerpSse (a b t : Nat) : Nat :=
  let d := sse b a (F32.sub b a)
  let m := sse d t (F32.mul d t)
  sse a m (F32.add a m)

/-! ## D3DRASTER.FILTER.LEVEL, MIPLINEAR -/

/-- binary64 `min` (a NaN gives the other) -/
def minD (x y : Nat) : Nat :=
  if isNaN F64.fmt x then y else if isNaN F64.fmt y then x else if F64.lt y x then y else x

def U32 (x : Nat) : Int := castInt F64.fmt 0 (2 ^ 32 - 1) x

/-- the level of a lod (log2 of the span, binary64) under the last level `last`: (the level,
none) for one level, (the level, some the next's weight in 1/256) between two -/
def level (lod last : Nat) (point : Bool) : Int × Option Int :=
  let l := Float.ofNat F64.fmt last
  if point then (U32 (floorF F64.fmt (minD (F64.add lod dhalf) l)), none)
  else
    let fr := F64.sub lod (truncF F64.fmt lod)
    let zero := !isNaN F64.fmt fr && (fr = 0 || fr = sbit F64.fmt)
    let ge := !isNaN F64.fmt lod && !isNaN F64.fmt l && !F64.lt lod l
    if zero || ge then (U32 (floorF F64.fmt (minD lod l)), none)
    else (U32 (floorF F64.fmt lod),
      some (castInt F64.fmt (-(2 ^ 63)) (2 ^ 63 - 1) (roundF F64.fmt (F64.mul fr d256))))

/-! ## D3DRASTER.SAMPLE.CMP -/

/-- Floating-point `<` and `==` (false with a NaN; -0 = +0). -/
def ltWith (fmt : Float.Fmt) (x y : Nat) : Bool :=
  !isNaN fmt x && !isNaN fmt y && Float.lt fmt x y
def eqWith (fmt : Float.Fmt) (x y : Nat) : Bool :=
  !isNaN fmt x && !isNaN fmt y && !Float.lt fmt x y && !Float.lt fmt y x

abbrev ltF := ltWith F32.fmt
abbrev eqF := eqWith F32.fmt

/-- D3D11_COMPARISON_FUNC: NEVER 1, LESS, EQUAL, LESS_EQUAL, GREATER, NOT_EQUAL,
GREATER_EQUAL, ALWAYS 8 (and any other). -/
def cmpWith (fmt : Float.Fmt) (func a s : Nat) : Bool :=
  match func with
  | 1 => false
  | 2 => ltWith fmt a s
  | 3 => eqWith fmt a s
  | 4 => ltWith fmt a s || eqWith fmt a s
  | 5 => ltWith fmt s a
  | 6 => !eqWith fmt a s
  | 7 => ltWith fmt s a || eqWith fmt a s
  | _ => true

abbrev cmp := cmpWith F32.fmt

/-- sample_c's texel: its red against the reference, 1 or 0 -/
def sampleCmp (func ref red : Nat) : Nat := if cmp func ref red then one else 0

/-! ## D3DRASTER.DEPTH.SMALL -/

/-- Direct3D 10: a pixel past a smaller depth view's width or height is not drawn -/
def pastDepth (dw dh x y : Nat) : Bool := dw ≤ x || dh ≤ y

/-! ## theorems -/

private theorem qnan_nan : isNaN F32.fmt (Float.qnan F32.fmt) = true := by decide

/-- D3DRASTER.FILTER.LINEAR, POINT: a NaN coordinate reads the texel 0 with the weight 0 -/
theorem split_nan (c n : Nat) (h : isNaN F32.fmt c = true) : split c n = (0, 0) ∧ pointTexel c n = 0 := by
  simp [split, pointTexel, F32.mul, F32.sub, Float.mul, Float.sub, Float.add, h, qnan_nan, floorF, I64, castInt]

/-- D3DRASTER.FILTER.LINEAR: no byte (n 0) is 0, a whole byte (255, n 32640) exactly 1 -/
theorem linear_ends : linearFloat 0 = 0 ∧ linearFloat 32640 = one := by decide

/-- D3DRASTER.FILTER.FLOAT, MIPLINEAR: the weights 0, a half and 1 are exact -/
theorem pixels_exact : pixels 0 = 0 ∧ pixels 128 = half ∧ pixels 256 = one := by decide

/-- D3DRASTER.FILTER.FLOAT: a NaN first texel is passed on quieted, whatever the second and the
weight -/
theorem lerp_nan_first (a b t : Nat) (ha : isNaN F32.fmt a = true) : lerpSse a b t = a ||| 0x400000 := by
  simp [lerpSse, sse, ha]

/-- D3DRASTER.SAMPLE.CMP: against a NaN (the reference or the texel) every function but
NOT_EQUAL and ALWAYS fails -/
theorem cmp_nan (func a s : Nat) (h : isNaN F32.fmt a = true ∨ isNaN F32.fmt s = true) :
    cmp func a s = !(func = 1 ∨ func = 2 ∨ func = 3 ∨ func = 4 ∨ func = 5 ∨ func = 7) := by
  rcases h with h | h <;> unfold cmp cmpWith <;> split <;> simp_all [ltWith, eqWith]

private theorem lt_asymm (x y : Nat) (h : Float.lt F32.fmt x y = true) : Float.lt F32.fmt y x = false := by
  simp only [Float.lt] at *
  cases hx : isInf F32.fmt x <;> cases hy : isInf F32.fmt y <;> cases sx : sign F32.fmt x <;>
    cases sy : sign F32.fmt y <;> simp_all <;> omega

/-- D3DRASTER.SAMPLE.CMP: LESS and GREATER_EQUAL, GREATER and LESS_EQUAL, EQUAL and NOT_EQUAL are
each other's opposites away from NaNs -/
theorem cmp_opposites (a s : Nat) (ha : isNaN F32.fmt a = false) (hs : isNaN F32.fmt s = false) :
    cmp 7 a s = !cmp 2 a s ∧ cmp 4 a s = !cmp 5 a s ∧ cmp 6 a s = !cmp 3 a s := by
  simp only [cmp, cmpWith, ltWith, eqWith, ha, hs]
  cases h1 : Float.lt F32.fmt a s
  · cases Float.lt F32.fmt s a <;> decide
  · rw [lt_asymm a s h1]; decide

/-- D3DRASTER.DEPTH.SMALL: inside the depth view, the depth is tested as usual -/
theorem past_depth (dw dh x y : Nat) : pastDepth dw dh x y = false ↔ x < dw ∧ y < dh := by
  simp [pastDepth]

/-- the mirrors: a binary64 as its two 32-bit halves -/
def mirrors : List Mirror := [
  ⟨"d3d11.split", fun | [c, n] => do let (i, w) := split (← nat c) (← nat n); pure [i, w] | _ => none⟩,
  ⟨"d3d11.pointtexel", fun | [c, n] => do pure [pointTexel (← nat c) (← nat n)] | _ => none⟩,
  ⟨"d3d11.linearfloat", fun | [n] => do pure [linearFloat (← nat n)] | _ => none⟩,
  ⟨"d3d11.pixels", fun | [v] => do pure [pixels (← nat v)] | _ => none⟩,
  ⟨"d3d11.lerp", fun | [a, b, t] => do pure [lerpSse (← nat a) (← nat b) (← nat t)] | _ => none⟩,
  ⟨"d3d11.level", fun
    | [hi, lo, last, point] => do
      match level (F64.join (← nat hi) (← nat lo)) (← nat last) (point != 0) with
      | (n, none) => pure [0, n]
      | (n, some w) => pure [1, n, w]
    | _ => none⟩,
  ⟨"d3d11.samplecmp", fun | [f, r, t] => do pure [sampleCmp (← nat f) (← nat r) (← nat t)] | _ => none⟩,
  ⟨"d3d11.pastdepth", fun | [dw, dh, x, y] => do pure [if pastDepth (← nat dw) (← nat dh) (← nat x) (← nat y) then 1 else 0] | _ => none⟩
]

end Kikizake.D3d11.Filter
