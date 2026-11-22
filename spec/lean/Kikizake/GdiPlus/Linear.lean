import Kikizake.GdiPlus.Flatten
/-! GDI+'s LinearGradientBrush made from two points (spec/gdiplus.txt GP.BRUSH.LINEAR,
GP.BRUSH.LINEAR.SIN, GP.BRUSH.LINEAR.PAINT) as src/model/gplinear.rs computes it: `new` is
`new_with`, its binary64 steps (`add64`, `mul64`, `div64`, `to32`) and binary32 ones rounded as the
executor rounds; libm's atan2, sin and cos are not computed here but taken as given, `new` answering
the arguments it calls them with; `span` and `at` are `Span::new` and `Span::at`, the 16.16 DDA and
its 32-entry color tables; mirrors `gp.linear`, `gp.linear_span`. -/
namespace Kikizake.GdiPlus.Linear
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten

private abbrev F := F32.fmt
private abbrev D := F64.fmt

/-- the largest value of format G at most a / b (b > 0) of sign s (GdiPlus.Matrix.roundD in any
format) -/
def roundDown (G : Fmt) (s : Bool) (a b : Nat) : Nat :=
  let sb := if s then sbit G else 0
  if a = 0 then sb else
  let d : Int := (Nat.log2 a : Int) - (Nat.log2 b : Int)
  let L : Int := if a * (pow2 d).2 < b * (pow2 d).1 then d - 1 else d
  let Q : Int := Max.max (L - G.m) (emin G)
  let (num, den) := if 0 ≤ Q then (a, b * 2 ^ Q.toNat) else (a * 2 ^ (-Q).toNat, b)
  let m0 := num / den
  let m := if s ∧ num % den ≠ 0 then m0 + 1 else m0
  let (m, Q) := if m = 2 ^ (G.m + 1) then (2 ^ G.m, Q + 1) else (m, Q)
  if Q + G.m > (bias G : Int) then (if s then sb + inf G else inf G - 1)
  else if m < 2 ^ G.m then sb + m
  else sb + (Q + bias G + G.m).toNat * 2 ^ G.m + (m - 2 ^ G.m)

def finite64 (x : Nat) : Bool := !isNaN D x && !isInf D x

/-- gppath.rs `add64`: rounding down, the sum toward minus infinity, but the nearest one when that
is not finite; an exact zero -0 unless both terms are +0 -/
def add64 (d : Bool) (x y : Nat) : Nat :=
  let s := F64.add x y
  if !d || !finite64 s then s else
  let (sg, a, b) := addExact D x y
  if a = 0 then (if !sign D x && !sign D y then 0 else sbit D) else roundDown D sg a b

/-- gppath.rs `mul64`: rounding down, the product toward minus infinity, but the nearest one when
that is a zero or not finite -/
def mul64 (d : Bool) (x y : Nat) : Nat :=
  let p := F64.mul x y
  if !d || !finite64 p || sig D p = 0 then p else
  let (pn, pd) := pow2 (ex D x + ex D y)
  roundDown D (sign D x != sign D y) (sig D x * sig D y * pn) pd

/-- gppath.rs `div64`: the same for a quotient -/
def div64 (d : Bool) (x y : Nat) : Nat :=
  let q := F64.div x y
  if !d || !finite64 q || sig D q = 0 then q else
  let (pn, pd) := pow2 (ex D x - ex D y)
  roundDown D (sign D x != sign D y) (sig D x * pn) (sig D y * pd)

/-- gppath.rs `to32`: rounding down, the largest binary32 at most v, but the nearest one when that
is not finite -/
def to32 (d : Bool) (v : Nat) : Nat :=
  let f := F64.toF32 v
  if !d || isNaN F f || isInf F f || sig D v = 0 then f else
  let (s, n, q) := toRat D v
  roundDown F s n q

/-- pi, 180 and 1 in binary64; 360, 270, 180, 90, 1/2 in binary32 -/
def piD : Nat := 0x400921fb54442d18
def k180D : Nat := 0x4066800000000000
def oneD : Nat := 0x3ff0000000000000
def k360 : Nat := 0x43b40000
def k270 : Nat := 0x43870000
def k180 : Nat := 0x43340000
def k90 : Nat := 0x42b40000
/-- 1.192_092_9e-6 and FLT_EPSILON -/
def nearE : Nat := 0x35a00000
def epsF : Nat := 0x34000000
/-- 1e-11 and 1 + 2^-52 in binary64 -/
def e11 : Nat := 0x3da5fd7fe1796495
def onePlus : Nat := 0x3ff0000000000001

/-- a binary32 truncated to an i32 as Rust's `as i32` does: saturated, a NaN 0 -/
def truncSat (x : Nat) : Int :=
  if isNaN F x then 0 else
  if isInf F x then (if sign F x then -2 ^ 31 else 2 ^ 31 - 1) else
  let (s, n, q) := toRat F x
  let t : Int := if s then -((n / q : Nat) : Int) else ((n / q : Nat) : Int)
  max (-2 ^ 31) (min (2 ^ 31 - 1) t)

/-- a binary32 floored to an i32 as Rust's `.floor() as i32` does -/
def floorSat (x : Nat) : Int :=
  if isNaN F x then 0 else
  if isInf F x then (if sign F x then -2 ^ 31 else 2 ^ 31 - 1) else
  max (-2 ^ 31) (min (2 ^ 31 - 1) (floorInt x))

/-- GpModF: v modulo m by a truncated quotient, a negative v folded up -/
def modf (r : Ops) (v m : Nat) : Nat :=
  let turns (v : Nat) := r.sub v (r.mul (ofInt (truncSat (r.div v m))) m)
  if flt 0 v then turns v
  else if flt v 0 then
    let b := turns (neg F v)
    if flt 0 b then r.sub m b else b
  else 0

/-- GP.BRUSH.LINEAR.SIN: rounding down, libm's sin within 1e-11 of 1 is 1 + 2^-52 -/
def sinQuirk (d : Bool) (s : Nat) : Nat :=
  if d && !isNaN D s && F64.lt ((F64.sub s oneD) % 2 ^ 63) e11 then onePlus else s

/-- InferAffineMatrix: the matrix taking the rectangle's corners (x, y), (x + w, y), (x, y + h) to
p0, p1, p2; none when the determinant is under FLT_EPSILON -/
def infer (r : Ops) (p : List (Nat × Nat)) (x y w h : Nat) : Option (List Nat) :=
  let det := r.add (r.sub (r.mul (r.add w x) (r.add y h)) (r.mul y x)) (r.sub (r.mul (neg F h) x) (r.mul y w))
  if flt (fabs det) epsF then none else
  let q := r.div one det
  let a := r.sub (r.mul (r.add x w) (r.add y h)) (r.mul x y)
  let pt (i : Nat) := p.getD i (0, 0)
  let pick (v : Nat × Nat) (k : Nat) := if k = 0 then v.1 else v.2
  let lin (s : Nat) (to : Nat × Nat) (k : Nat) := r.mul (r.add (r.mul s (pick to k)) (r.mul (neg F s) (pick (pt 0) k))) q
  let tr (k : Nat) := r.mul (r.add (r.add (r.mul (r.mul (neg F x) h) (pick (pt 1) k)) (r.mul a (pick (pt 0) k))) (r.mul (r.mul (neg F y) w) (pick (pt 2) k))) q
  some [lin h (pt 1) 0, lin h (pt 1) 1, lin w (pt 2) 0, lin w (pt 2) 1, tr 0, tr 1]

/-- a brush: its rectangle, matrix, colors and whether every other period is mirrored -/
structure Brush where
  rect : List Nat
  matrix : List Nat
  colors : Nat × Nat
  mirror : Bool

/-- what GdipCreateLineBrushI does with libm's answers `lib` (atan2, sin, cos): its status (0 Ok, 2
InvalidParameter, 3 OutOfMemory), the arguments of the libm calls it made (binary64), and the brush -/
def new (d : Bool) (p1x p1y p2x p2y wrap : Int) (colors : Nat × Nat) (lib : List Nat) : Nat × List Nat × Option Brush :=
  if wrap = 4 then (2, [], none) else
  if p1x = p2x ∧ p1y = p2y then (3, [], none) else
  let r := ops d
  let (ax, ay, bx, by_) := (ofInt p1x, ofInt p1y, ofInt p2x, ofInt p2y)
  let x := if flt bx ax then bx else ax
  let y := if flt by_ ay then by_ else ay
  let w := fabs (r.sub ax bx)
  let h := fabs (r.sub ay by_)
  let den := if isZero bx then one else bx
  let (w, x) := if flt (fabs (r.div (r.sub ax bx) den)) nearE then (h, r.sub x (r.mul h half)) else (w, x)
  let den := if isZero by_ then one else by_
  let (h, y) := if flt (fabs (r.div (r.sub ay by_) den)) nearE then (w, r.sub y (r.mul w half)) else (h, y)
  let (dx, dy) := (r.sub bx ax, r.sub by_ ay)
  let atan := lib.getD 0 0
  let angle := to32 d (div64 d (mul64 d atan k180D) piD)
  let a := modf r angle k360
  let (q, k) :=
    if flt a k90 then (0, a)
    else if flt a k180 then (1, r.sub k180 a)
    else if flt a k270 then (2, r.sub a k180)
    else (3, r.sub k360 a)
  let th := div64 d (mul64 d (F64.ofF32 k) piD) k180D
  let s := sinQuirk d (lib.getD 1 0)
  let c := lib.getD 2 0
  let n64 := neg D
  let (xd, yd, wd, hd) := (F64.ofF32 x, F64.ofF32 y, F64.ofF32 w, F64.ofF32 h)
  let (kk, sp, cp, x0, y0) :=
    if q = 0 then (mul64 d wd s, s, c, xd, yd)
    else if q = 1 then (mul64 d hd c, s, n64 c, add64 d wd xd, yd)
    else if q = 2 then (mul64 d wd s, n64 s, n64 c, add64 d wd xd, add64 d hd yd)
    else (mul64 d hd c, n64 s, c, xd, add64 d hd yd)
  let l1 := add64 d (mul64 d hd c) (mul64 d wd s)
  let l2 := add64 d (mul64 d wd c) (mul64 d hd s)
  let p0 := (to32 d (add64 d (mul64 d sp kk) x0), to32 d (add64 d y0 (n64 (mul64 d cp kk))))
  let (a1, b1) := (to32 d (mul64 d cp l2), to32 d (mul64 d sp l2))
  let (a2, b2) := (to32 d (mul64 d (n64 l1) sp), to32 d (mul64 d cp l1))
  let pa := (r.add a1 p0.1, r.add b1 p0.2)
  let pb := (r.add a2 p0.1, r.add b2 p0.2)
  let calls := [F64.ofF32 dy, F64.ofF32 dx, th, th]
  match infer r [p0, pa, pb] x y w h with
  | none => (2, calls, none)
  | some m => (0, calls, some ⟨[x, y, w, h], m, colors, wrap = 1 ∨ wrap = 3⟩)

/-- GpMatrix::MultiplyMatrix (points as row vectors: p a b) -/
def multiply (r : Ops) (a b : List Nat) : List Nat :=
  let e (i : Nat) := a.getD i 0
  let f (i : Nat) := b.getD i 0
  [r.add (r.mul (f 0) (e 0)) (r.mul (f 2) (e 1)), r.add (r.mul (e 0) (f 1)) (r.mul (e 1) (f 3)),
   r.add (r.mul (f 2) (e 3)) (r.mul (f 0) (e 2)), r.add (r.mul (f 1) (e 2)) (r.mul (e 3) (f 3)),
   r.add (r.add (r.mul (e 4) (f 0)) (r.mul (e 5) (f 2))) (f 4), r.add (r.add (r.mul (e 4) (f 1)) (r.mul (e 5) (f 3))) (f 5)]

/-- binary32 equality (Rust `==`: -0 is 0, a NaN equals nothing) -/
def feq (x y : Nat) : Bool := !isNaN F x && !isNaN F y && (x == y || (isZero x && isZero y))

/-- GpMatrix::Invert: an identity as it is -/
def invert (r : Ops) (m : List Nat) : Option (List Nat) :=
  if m.length = 6 ∧ (m.zip [one, 0, 0, one, 0, 0]).all (fun (a, b) => feq a b) then some m else inverse r m

/-- RoundSat: floor(v + 1/2) saturated to int32 -/
def roundSat (r : Ops) (v : Nat) : Int :=
  if flt v 0xcf000000 then -2 ^ 31
  else if !isNaN F v && !flt v 0x4f000000 then 2 ^ 31 - 1
  else floorSat (r.add v half)

/-- a binary32 floored to a u32 (Rust `.floor() as u32`) -/
def floorU (x : Nat) : Nat :=
  if isNaN F x then 0 else if isInf F x then (if sign F x then 0 else 2 ^ 32 - 1) else
  let f := floorInt x
  if f < 0 then 0 else min f.toNat (2 ^ 32 - 1)

/-- a color's channels as binary32, premultiplied by a / 255, alpha last; 0 for alpha 0 -/
def linColor (r : Ops) (c : Nat) : List Nat :=
  let a := c / 2 ^ 24 % 256
  let ch (k : Nat) := F32.ofNat (c / 2 ^ (8 * k) % 256)
  if a = 0 then [0, 0, 0, 0] else
  let fa := F32.ofNat a
  let kq := if a ≠ 255 then r.div fa 0x437f0000 else one
  let f (v : Nat) := if a ≠ 255 then r.mul kq v else v
  [f (ch 0), f (ch 1), f (ch 2), fa]

/-- the 17 entries of a period: the two colors blended in float at 0, 1/16, .. 1, rounded -/
def entries (r : Ops) (l1 l2 : List Nat) : List (List Nat) :=
  let step := r.div one k16
  ((List.range 17).foldl (fun (acc : List (List Nat) × Nat) _ =>
    let tf := acc.2
    let e := (List.range 4).map fun k =>
      floorU (r.add (r.add (r.mul (l1.getD k 0) (r.sub one tf)) (r.mul (l2.getD k 0) tf)) half)
    (e :: acc.1, r.add tf step)) ([], 0)).1.reverse

/-- the DDA (du, dv, u0) and the tables a, b of a brush's span -/
structure Span where
  du : Int
  dv : Int
  u0 : Int
  a : List (List Nat)
  b : List (List Nat)

/-- Span::new: none when the matrix has no inverse -/
def span (r : Ops) (l : Brush) (halfOffset : Bool) : Option Span :=
  let bw := if halfOffset then multiply r l.matrix [one, 0, 0, one, 0xbf000000, 0xbf000000] else l.matrix
  let t := r.mul (F32.ofNat (32 * 2 ^ 16)) half
  let e (i : Nat) := l.rect.getD i 0
  let (x, y, w, h) := (e 0, e 1, e 2, e 3)
  let (right, bottom) := (r.add x w, r.add y h)
  let (m11, m22) := (r.div (r.sub right x) (r.sub t 0), r.div (r.sub bottom y) (r.sub t 0))
  let m0 := [m11, 0, 0, m22, r.sub right (r.mul m11 t), r.sub bottom (r.mul m22 t)]
  match invert r (multiply r m0 bw) with
  | none => none
  | some mi =>
    let g (i : Nat) := mi.getD i 0
    let es := entries r (linColor r l.colors.1) (linColor r l.colors.2)
    let en (i : Nat) := es.getD i [0, 0, 0, 0]
    let a := (List.range 32).map fun i =>
      if i < 16 then en i else if l.mirror then en (32 - i) else en (i - 16)
    let b := (List.range 32).map fun i =>
      if i < 16 then en (i + 1) else if l.mirror then en (31 - i) else en (i - 15)
    some ⟨floorSat (r.add (g 0) half), floorSat (r.add (g 2) half), roundSat r (g 4), a, b⟩

/-- Span::at: device pixel (x, y)'s premultiplied ARGB: u = x du + y dv + u0 in 32 bits, the entry
(u >> 16) & 31, the two entries mixed by (u >> 8) & 255 -/
def at_ (s : Span) (x y : Int) : Nat :=
  let u := ((x * s.du + y * s.dv + s.u0) % 2 ^ 32).toNat
  let idx := u / 2 ^ 16 % 32
  let w := u / 2 ^ 8 % 256
  let ea := s.a.getD idx [0, 0, 0, 0]
  let eb := s.b.getD idx [0, 0, 0, 0]
  (List.range 4).foldl (fun o k => o ||| (((256 - w) * ea.getD k 0 + w * eb.getD k 0 + 128) / 256) * 2 ^ (8 * k)) 0

/-- GP.BRUSH.LINEAR.SIN: rounding down, a sin of 1 or one unit below it is 1 + 2^-52, to nearest it
stays; 0.5 stays either way -/
theorem sin_up :
    sinQuirk true oneD = onePlus ∧ sinQuirk true 0x3fefffffffffffff = onePlus ∧ sinQuirk false oneD = oneD ∧
    sinQuirk true 0x3fe0000000000000 = 0x3fe0000000000000 := by decide +kernel

/-- GP.BRUSH.LINEAR: WrapModeClamp is InvalidParameter, the same two points OutOfMemory, without
calling libm -/
theorem refusals :
    (new false 0 0 5 5 4 (0, 0) []).1 = 2 ∧ (new true 3 4 3 4 0 (0, 0) []).1 = 3 ∧ (new true 3 4 3 4 0 (0, 0) []).2.1 = [] := by
  decide +kernel

/-- GP.BRUSH.LINEAR: a horizontal line from (0, 0) to (10, 0) (atan2 0, sin 0, cos 1): its box made
square about it, (0, -5, 10, 10), and the identity -/
theorem horizontal :
    (new false 0 0 10 0 0 (0, 0) [0, 0, oneD]).2.2.map (fun b => (b.rect, b.matrix)) =
      some ([0, 0xc0a00000, 0x41200000, 0x41200000], [one, 0, 0, one, 0, 0]) := by decide +kernel

/-- GP.BRUSH.LINEAR.PAINT: that brush's span: a step of 16 entries of 2^16 over its 10 pixels, its table of white
to black over a period and white again at 10 -/
theorem horizontal_span :
    ((new false 0 0 10 0 0 (0xffffffff, 0xff000000) [0, 0, oneD]).2.2.bind (span nearest · false)).map
      (fun s => (s.du, s.dv, s.u0, at_ s 0 0, at_ s 5 0, at_ s 10 0)) = some (104858, 0, 0, 0xffffffff, 0xff808080, 0xffffffff) := by
  decide +kernel

private def outRect (b : Brush) : List Int := (b.rect ++ b.matrix).map (fun v => (F32.canon v : Int)) ++ [if b.mirror then 1 else 0]

private def brushOut (d : Bool) (p1x p1y p2x p2y wrap : Int) (lib : List Nat) : List Int :=
  let (st, calls, b) := new d p1x p1y p2x p2y wrap (0, 0) lib
  [(st : Int)] ++ calls.flatMap (fun c => halves (F64.canon c)) ++ ((b.map outRect).getD [])

private def pixels (s : Span) : List Int → List Int
  | px :: py :: rest => (at_ s px py : Int) :: pixels s rest
  | _ => []

def mirrors : List Mirror := [
  ⟨"gp.linear", fun
    | [d, p1x, p1y, p2x, p2y, wrap, a0, a1, s0, s1, c0, c1] => do
      pure (brushOut (d != 0) p1x p1y p2x p2y wrap [← nat64 a0 a1, ← nat64 s0 s1, ← nat64 c0 c1])
    | _ => none⟩,
  ⟨"gp.linear_span", fun
    | d :: hf :: x :: y :: w :: h :: m0 :: m1 :: m2 :: m3 :: m4 :: m5 :: c1 :: c2 :: mir :: pxs => do
      let rect ← [x, y, w, h].mapM nat
      let m ← [m0, m1, m2, m3, m4, m5].mapM nat
      let b : Brush := ⟨rect, m, (← nat c1, ← nat c2), mir != 0⟩
      match span (ops (d != 0)) b (hf != 0) with
      | none => pure [-1]
      | some s => pure ([s.du, s.dv, s.u0] ++ (s.a ++ s.b).flatMap (fun e => e.map Int.ofNat) ++ pixels s pxs)
    | _ => none⟩
]

end Kikizake.GdiPlus.Linear
