import Kikizake.GdiPlus.Linear
import Kikizake.GdiPlus.Stroke
/-! GDI+'s ellipses, arcs and pies (spec/gdiplus.txt GP.PATH.ARC) as src/model/gppath.rs
`ellipse_points`, `normalize`, `arc_points` and `Path::add_ellipse`, `add_arc`, `add_pie` compute
them, on binary32 and binary64 bits rounded as the executor rounds (GdiPlus.Matrix,
GdiPlus.Linear); libm's sin, cos and atan2 are taken as given, in the order of the calls, each call's
arguments answered (`Lib`); mirror `gp.arc`. -/
namespace Kikizake.GdiPlus.Arc
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Linear

private abbrev F := F32.fmt
private abbrev D := F64.fmt

/-- libm's answers still to give, and the arguments of the calls made (binary64) -/
abbrev Lib := StateM (List Nat × List Nat)

/-- a libm call of these arguments: its answer the next one given -/
def call (args : List Nat) : Lib Nat :=
  modifyGet fun (ans, made) => (ans.headD 0, (ans.tail, made ++ args))

/-- the CRT's sinf and cosf: libm's binary64 answer for the binary32 argument, rounded as the
executor rounds -/
def libf (d : Bool) (a : Nat) : Lib Nat := do pure (to32 d (← call [F64.ofF32 a]))

/-- float pi, 2 pi, pi / 2, pi / 4 -/
def piF : Nat := 0x40490fdb
def twoPiF : Nat := 0x40c90fdb
def halfPiF : Nat := 0x3fc90fdb
def quarterPiF : Nat := 0x3f490fdb
/-- pi / 2 in binary64, 0.017_453_292 in binary32, 3, 4 -/
def quarterD : Nat := 0x3ff921fb54442d18
def degF : Nat := 0x3c8efa35
def k3 : Nat := 0x40400000
def k4 : Nat := 0x40800000

/-- x <= y, false when either is a NaN -/
def fle (x y : Nat) : Bool := !isNaN F x && !isNaN F y && !F32.lt y x

/-- AddEllipse's 13 points: four quarter Beziers from angle 0 toward +y, kappa ((sqrt 2 - 1) 4) / 3 -/
def ellipsePoints (d : Bool) (x y w h : Nat) : List (Nat × Nat) :=
  let r := ops d
  let k := r.div (r.mul (r.sub (Stroke.sqrt32 d 0x40000000) one) k4) k3
  let (rx, ry) := (r.mul w half, r.mul h half)
  let (cx, cy) := (r.add rx x, r.add ry y)
  let n1 := neg F one
  let nk := neg F k
  let unit := [(one, 0), (one, k), (k, one), (0, one), (nk, one), (n1, k), (n1, 0), (n1, nk), (nk, n1), (0, n1), (k, n1), (one, nk), (one, 0)]
  unit.map fun (u, v) => (r.add (r.mul rx u) cx, r.add (r.mul ry v) cy)

/-- NormalizeAngle: degrees to the ellipse's parameter in radians (a circle's the angle itself; else
atan2 of sin t w, h cos t quadrant by quadrant, stored rounded down) -/
def normalize (d : Bool) (a w h : Nat) : Lib Nat := do
  let r := ops d
  let turns (a : Nat) := r.sub a (r.mul (ofInt (truncSat (r.div a k360))) k360)
  let a := if flt 0 a then turns a
    else if flt a 0 then (let b := turns (neg F a); if flt 0 b then r.sub k360 b else b)
    else 0
  let a := if fle 0 a && fle a k360 then a else 0
  if feq w h then return to32 d (div64 d (mul64 d (F64.ofF32 a) piD) k180D)
  let (q, b) :=
    if fle a k90 then (1, a)
    else if fle a k180 then (2, r.sub k180 a)
    else if fle a k270 then (3, r.sub a k180)
    else (4, r.sub k360 a)
  let t := F64.ofF32 (to32 d (div64 d (mul64 d (F64.ofF32 b) piD) k180D))
  let s ← call [t]
  let c ← call [t]
  let ya := mul64 d s (F64.ofF32 w)
  let xa := mul64 d (F64.ofF32 h) c
  let f := to32 true (← call [ya, xa])
  pure (if q = 1 then f else if q = 2 then r.sub piF f else if q = 3 then r.add f piF else r.sub twoPiF f)

/-- a binary64 truncated to an i32 (Rust `as i32`: saturated, a NaN 0) -/
def trunc64 (x : Nat) : Int :=
  if isNaN D x then 0 else
  if isInf D x then (if sign D x then -2 ^ 31 else 2 ^ 31 - 1) else
  let (s, n, q) := toRat D x
  let t : Int := if s then -((n / q : Nat) : Int) else ((n / q : Nat) : Int)
  max (-2 ^ 31) (min (2 ^ 31 - 1) t)

/-- an i32 as a binary64 (exact) -/
def ofInt64 (v : Int) : Nat := if v < 0 then neg D (Float.round D false v.natAbs 1) else Float.round D false v.toNat 1

/-- GetArcPoints: the Beziers of an arc, 3 n + 1 points of n pieces of at most a quarter; none for
an empty rectangle, no points for a sweep of 0 -/
def arcPoints (d : Bool) (x y w h start sweep : Nat) : Lib (Option (List (Nat × Nat))) := do
  let r := ops d
  if fle w 0 || fle h 0 then return none
  if isZero sweep then return some []
  let pos := flt 0 sweep
  let dir := if pos then one else neg F one
  let en := r.add start sweep
  let st0 ← normalize d start w h
  let en ← normalize d en w h
  let span0 :=
    if fle k360 (fabs sweep) then twoPiF
    else
      let s := if pos then r.sub en st0 else r.sub st0 en
      if flt s 0 then r.add s twoPiF else s
  let (rx, ry) := (r.mul w half, r.mul h half)
  let (cx, cy) := (r.add rx x, r.add ry y)
  let n0 := trunc64 (F64.div (F64.ofF32 span0) quarterD)
  let n0 := if F64.lt (F64.mul (ofInt64 n0) quarterD) (F64.ofF32 span0) && !isNaN D (F64.ofF32 span0) then n0 + 1 else n0
  let n := (max 1 (min 4 n0)).toNat
  let base := [rx, r.mul rx 0, r.mul ry 0, ry, r.add (r.add (r.mul cy 0) cx) 0, r.add (r.add (r.mul cx 0) cy) 0]
  let b (i : Nat) := base.getD i 0
  let mut out : List (Nat × Nat) := []
  let mut st := st0
  let mut span := span0
  for i in [0:n] do
    let hf := if F64.lt quarterD (F64.ofF32 span) && !isNaN F span then quarterPiF else r.mul span half
    let c ← libf d hf
    let s ← libf d hf
    let t := r.div (r.mul (r.sub k3 c) s) (r.add (r.mul c k3) k3)
    let kx := r.div (r.sub k4 c) k3
    let (ns, nt) := (neg F s, neg F t)
    let unit := if pos then [(c, ns), (kx, nt), (kx, t), (c, s)] else [(c, s), (kx, t), (kx, nt), (c, ns)]
    let th := r.add (r.mul dir hf) st
    let th := r.mul (to32 d (div64 d (F64.ofF32 (r.mul th k180)) piD)) degF
    let sn ← libf d th
    let cs ← libf d th
    let m := [r.add (r.mul cs (b 0)) (r.mul sn (b 2)), r.add (r.mul sn (b 3)) (r.mul cs (b 1)),
      r.sub (r.mul cs (b 2)) (r.mul sn (b 0)), r.sub (r.mul cs (b 3)) (r.mul sn (b 1)), b 4, b 5]
    let take := if i + 1 = n then 4 else 3
    out := out ++ (unit.take take).map fun (u, v) => apply r m u v
    st := r.add st (if pos then halfPiF else neg F halfPiF)
    span := r.add span (neg F halfPiF)
  pure (some out)

/-- a path's point: x, y, its type byte (0 start, 1 line, 3 Bezier, 0x80 closing) -/
abbrev Pt := Nat × Nat × Nat

/-- Path::join on a path whose last figure is open at l (or new): the first of new points skipped
within FLT_EPSILON of l, else a line to it, a new figure's start -/
def join (r : Ops) (l : Option Pt) (p : Nat × Nat) : List Pt :=
  match l with
  | none => [(p.1, p.2, 0)]
  | some (lx, ly, _) => if flt (fabs (r.sub p.1 lx)) epsF && flt (fabs (r.sub p.2 ly)) epsF then [] else [(p.1, p.2, 1)]

/-- the figure closed: its last point's type 0x80 added -/
def close (ps : List Pt) : List Pt :=
  match ps.getLast? with
  | none => ps
  | some (x, y, t) => ps.dropLast ++ [(x, y, t ||| 0x80)]

/-- Path::add_arc onto an open figure ending at l (or none): status (0 Ok, 2 InvalidParameter) and
the points added; a sweep past +-360 the whole ellipse, closed -/
def addArc (d : Bool) (l : Option Pt) (x y w h start sweep : Nat) : Lib (Nat × List Pt) := do
  let (sweep, closed) :=
    if fle k360 sweep then (k360, true)
    else if fle sweep (neg F k360) then (neg F k360, true)
    else (sweep, false)
  match ← arcPoints d x y w h start sweep with
  | none => pure (2, [])
  | some [] => pure (0, [])
  | some (p :: rest) =>
    let pts := join (ops d) l p ++ rest.map fun (a, b) => (a, b, 3)
    pure (0, if closed then close pts else pts)

/-- GP.PATH.ARC on a new path: 0 AddEllipse, 1 AddArc, 2 AddPie (the center, then the arc, closed) -/
def add (d : Bool) (kind : Nat) (x y w h start sweep : Nat) : Lib (Nat × List Pt) := do
  let r := ops d
  if kind = 0 then
    let e := ellipsePoints d x y w h
    pure (0, close (e.zipIdx.map fun ((a, b), i) => (a, b, if i = 0 then 0 else 3)))
  else if kind = 1 then addArc d none x y w h start sweep
  else
    let c : Pt := (r.add (r.mul w half) x, r.add (r.mul h half) y, 0)
    let (st, pts) ← addArc d (some c) x y w h start sweep
    pure (st, if st = 0 then close (c :: pts) else [c])

/-- the whole answer: status, the libm calls' arguments (halves), the points -/
def run (d : Bool) (kind : Nat) (x y w h start sweep : Nat) (lib : List Nat) : List Int :=
  let ((st, pts), (_, made)) := (add d kind x y w h start sweep).run (lib, [])
  [(st : Int)] ++ made.flatMap (fun a => halves (F64.canon a)) ++ pts.flatMap fun (a, b, t) => [(F32.canon a : Int), F32.canon b, t]

/-- GP.PATH.ARC: AddEllipse of (0, 0, 2, 2): the unit circle's 13 points from (2, 1), kappa
0.5522847 out from each axis point (1.5522847 is 0x3fc6b144); an empty rectangle is InvalidParameter -/
theorem ellipse_unit :
    (ellipsePoints false 0 0 0x40000000 0x40000000).take 3 = [(0x40000000, one), (0x40000000, 0x3fc6b144), (0x3fc6b144, 0x40000000)] ∧
    ((add false 1 0 0 0 one 0 0x42b40000).run ([], [])).1 = (2, []) := by decide +kernel

private def answers : List Int → Option (List Nat)
  | hi :: lo :: rest => do pure ((← nat64 hi lo) :: (← answers rest))
  | [] => some []
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.arc", fun
    | dd :: kind :: x :: y :: w :: h :: start :: sweep :: lib => do
      pure (run (dd != 0) (← nat kind) (← nat x) (← nat y) (← nat w) (← nat h) (← nat start) (← nat sweep) (← answers lib))
    | _ => none⟩
]

end Kikizake.GdiPlus.Arc
