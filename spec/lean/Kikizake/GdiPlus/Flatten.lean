import Kikizake.GdiPlus.Matrix
/-! GDI+'s path flattening (spec/gdiplus.txt GP.PATH.FLATTEN, GP.PATH.FLATTEN.BIG) as
src/model/gppath.rs computes it: `ceil16` is `ceil16` (RasterizerCeiling), `complexity` and
`apply` how GpMatrix transforms a point, `bezier32` and `bezier64` GDI's 32-bit and 64-bit forward
differencing, `flatten` `Path::flatten`; on binary32 bits rounded as the executor rounds
(GdiPlus.Matrix); mirrors `gp.ceil16`, `gp.apply`, `gp.bezier32`, `gp.bezier64`, `gp.flatten`. -/
namespace Kikizake.GdiPlus.Flatten
open Kikizake.Float
open Kikizake.GdiPlus.Matrix

private abbrev F := F32.fmt

/-- |x|: the sign bit cleared -/
def fabs (x : Nat) : Nat := x % 2 ^ 31
/-- x < y, false when either is a NaN -/
def flt (x y : Nat) : Bool := !isNaN F x && !isNaN F y && F32.lt x y
/-- Rust's f32::max of two values of 0 or more: a NaN gives the other -/
def fmax (x y : Nat) : Nat := if isNaN F x then y else if isNaN F y then x else if F32.lt x y then y else x
def isZero (x : Nat) : Bool := fabs x == 0

/-- a finite binary32 floored to an integer -/
def floorInt (x : Nat) : Int :=
  let (s, n, d) := toRat F x
  if s then -(((n + d - 1) / d : Nat) : Int) else ((n / d : Nat) : Int)

/-- 16 and 1/2 -/
def k16 : Nat := 0x41800000

/-- GP.PATH.FLATTEN (RasterizerCeiling): v 16 + 1/2 floored, past the int range the integer
indefinite, then ceiled to an integer of 1/16 in 32 bits -/
def ceil16 (r : Ops) (v : Nat) : Int :=
  let t := r.add (r.mul v k16) half
  let i : Int := if isNaN F t || isInf F t then -2 ^ 31 else
    let f := floorInt t
    if -2 ^ 31 ≤ f ∧ f < 2 ^ 31 then f else -2 ^ 31
  let u := i + 15
  let u := if u ≥ 2 ^ 31 then u - 2 ^ 32 else u
  u / 16

/-- about 5.96e-4 -/
def near : Nat := 0x3a1c4000

/-- ComputeComplexity: 0 identity, 1 translation, 2 scale, 3 general -/
def complexity (r : Ops) (m : List Nat) : Nat :=
  let e (i : Nat) := m.getD i 0
  let big := fmax (fmax (fmax (fabs (e 0)) (fabs (e 1))) (fabs (e 2))) (fabs (e 3))
  let tol := r.mul near big
  let moved := !(isZero (e 4) && isZero (e 5))
  if !(flt (fabs (e 1)) tol && flt (fabs (e 2)) tol) then 3
  else if flt (fabs (r.sub (e 0) one)) near && flt (fabs (r.sub (e 3) one)) near then (if moved then 1 else 0)
  else 2

/-- a point through a matrix by its complexity -/
def apply (r : Ops) (m : List Nat) (x y : Nat) : Nat × Nat :=
  let e (i : Nat) := m.getD i 0
  match complexity r m with
  | 0 => (x, y)
  | 1 => (r.add (e 4) x, r.add (e 5) y)
  | 2 => (r.add (r.mul (e 0) x) (e 4), r.add (r.mul y (e 3)) (e 5))
  | _ => (r.add (r.add (r.mul x (e 0)) (r.mul y (e 2))) (e 4), r.add (r.add (r.mul x (e 1)) (r.mul y (e 3))) (e 5))

-- ---- GDI's 32-bit forward differencing ----

def shr (v : Int) (k : Nat) : Int := v / (2 ^ k : Int)
def shl (v : Int) (k : Nat) : Int := v * (2 ^ k : Int)
def bigAbs (a b : Int) : Int := max a.natAbs b.natAbs

structure Ax where
  x : Int
  dx : Int
  c : Int
  p : Int

def Ax.new (v : List Int) : Ax :=
  let g (i : Nat) := v.getD i 0
  ⟨shl (g 0) 10, shl (g 3 - g 0) 10, shl (3 * (g 1 - 2 * g 2 + g 3)) 11, shl (3 * (g 0 - 2 * g 1 + g 2)) 11⟩
def Ax.step (e : Ax) : Ax := ⟨e.x + e.dx, e.dx + e.c, 2 * e.c - e.p, e.c⟩
def Ax.err (e : Ax) : Int := bigAbs e.c e.p

/-- the first subdivision: until both errors are within 0x6000 << sh -/
def split32 : Nat → Ax → Ax → Nat → Nat → Ax × Ax × Nat × Nat
  | 0, a, b, sh, n => (a, b, sh, n)
  | fuel + 1, a, b, sh, n =>
    let tol : Int := shl 0x6000 sh
    if a.err > tol ∨ b.err > tol then
      let sh := sh + 2
      let f (e : Ax) : Ax := let c := shr (e.c + e.p) 1; { e with c := c, dx := shr (e.dx - shr c sh) 1 }
      split32 fuel (f a) (f b) sh (2 * n)
    else (a, b, sh, n)

/-- the steps while n allows doubling them -/
def double32 : Nat → Ax → Ax → Nat → Ax × Ax × Nat
  | 0, a, b, n => (a, b, n)
  | fuel + 1, a, b, n =>
    if n % 2 = 0 then
      if bigAbs a.p (2 * a.c - a.p) > 0xc000 ∨ bigAbs b.p (2 * b.c - b.p) > 0xc000 then (a, b, n)
      else
        let f (e : Ax) : Ax := let p := 4 * e.p; ⟨e.x, e.c + 2 * e.dx, 8 * e.c - p, p⟩
        double32 fuel (f a) (f b) (n / 2)
    else (a, b, n)

def walk32 : Nat → Ax → Ax → Nat → Int → Int → List (Int × Int)
  | 0, _, _, _, _, _ => []
  | fuel + 1, a, b, n, ox, oy =>
    let pt := (shr (a.x + 0x1000) 13 + ox, shr (b.x + 0x1000) 13 + oy)
    if n = 0 then [pt] else
    let (a, b, n) :=
      if max a.err b.err > 0x30000 then
        let f (e : Ax) : Ax := let c := shr (e.c + e.p) 3; ⟨e.x, shr (e.dx - c) 1, c, shr e.p 2⟩
        (f a, f b, 2 * n)
      else (a, b, n)
    let (a, b, n) := double32 64 a b n
    pt :: walk32 fuel a.step b.step (n - 1) ox oy

def listMin : List Int → Int
  | [] => 0
  | x :: xs => xs.foldl min x

/-- Bezier32: none past its 14-bit range -/
def bezier32 (q : List (Int × Int)) : Option (List (Int × Int)) :=
  let ox := listMin (q.map (·.1)) - 16
  let oy := listMin (q.map (·.2)) - 16
  let xs := q.map (·.1 - ox)
  let ys := q.map (·.2 - oy)
  if (xs ++ ys).any (fun v => v < 0 ∨ v > 0x3fff) then none else
  let (a, b, sh, n) := split32 64 (Ax.new xs) (Ax.new ys) 0 1
  let rs : Int := (sh : Int) - 3
  let f (v : Int) : Int := if 0 ≤ rs then shr v rs.toNat else shl v (-rs).toNat
  let g (e : Ax) : Ax := (Ax.mk (shl e.x 3) (shl e.dx 3) (f e.c) (f e.p)).step
  some (walk32 (2 ^ 20) (g a) (g b) (n - 1) ox oy)

-- ---- GDI's 64-bit hybrid forward differencing ----

structure Ba where
  e0 : Int
  e1 : Int
  e2 : Int
  e3 : Int

def Ba.new (p : List Int) : Ba :=
  let g (i : Nat) := p.getD i 0
  ⟨shl (g 0) 28, shl (g 3 - g 0) 28, shl (3 * (g 1 - 2 * g 2 + g 3)) 29, shl (3 * (g 0 - 2 * g 1 + g 2)) 29⟩
def Ba.err (e : Ba) : Int := max e.e2.natAbs e.e3.natAbs
def Ba.parentErr (e : Ba) : Int := max (4 * e.e3).natAbs (8 * e.e2 - 4 * e.e3).natAbs
def Ba.halve (e : Ba) : Ba := let e2 := shr (e.e2 + e.e3) 3; ⟨e.e0, shr (e.e1 - e2) 1, e2, shr e.e3 2⟩
def Ba.double (e : Ba) : Ba := let e3 := shl e.e3 2; ⟨e.e0, e.e2 + 2 * e.e1, 8 * e.e2 - e3, e3⟩
def Ba.step (e : Ba) : Ba := ⟨e.e0 + e.e1, e.e1 + e.e2, 2 * e.e2 - e.e3, e.e2⟩
def Ba.untransform (e : Ba) : List Int :=
  let h : Int := 2 ^ 27
  let r := 6 * e.e1 - e.e2
  [shr (e.e0 + h) 28, shr (e.e0 + h + Int.tdiv (r - 2 * e.e3) 18) 28, shr (e.e0 + h + Int.tdiv (2 * r - e.e3) 18) 28, shr (e.e0 + e.e1 + h) 28]

def t1 : Int := 3 * 2 ^ 44
def t2 : Int := 2 ^ 34

/-- halve both while either error passes t, doubling n -/
def halves : Nat → Int → Ba → Ba → Nat → Ba × Ba × Nat
  | 0, _, a, b, n => (a, b, n)
  | fuel + 1, t, a, b, n => if a.err > t ∨ b.err > t then halves fuel t a.halve b.halve (2 * n) else (a, b, n)
/-- double both while n is even and both parents' errors are within t -/
def doubles : Nat → Int → Ba → Ba → Nat → Ba × Ba × Nat
  | 0, _, a, b, n => (a, b, n)
  | fuel + 1, t, a, b, n =>
    if n % 2 = 0 ∧ a.parentErr ≤ t ∧ b.parentErr ≤ t then doubles fuel t a.double b.double (n / 2) else (a, b, n)

def walk64 : Nat → Ba → Ba → Nat → Ba → Ba → Nat → List (Int × Int)
  | 0, _, _, _, _, _, _ => []
  | fuel + 1, ox, oy, n, ix, iy, ni =>
    let (ox, oy, n, ix, iy, ni) :=
      if ni = 0 then
        let (ix, iy, ni) := halves 128 t2 (Ba.new ox.untransform) (Ba.new oy.untransform) 1
        let n := n - 1
        if n ≠ 0 then
          let (ox, oy) := (ox.step, oy.step)
          let (ox, oy, n) := if ox.err > t1 ∨ oy.err > t1 then (ox.halve, oy.halve, 2 * n) else (ox, oy, n)
          let (ox, oy, n) := doubles 128 t1 ox oy n
          (ox, oy, n, ix, iy, ni)
        else (ox, oy, n, ix, iy, ni)
      else (ox, oy, n, ix, iy, ni)
    let (ix, iy) := (ix.step, iy.step)
    let h : Int := 2 ^ 27
    let pt := (shr (ix.e0 + h) 28, shr (iy.e0 + h) 28)
    let ni := ni - 1
    if ni = 0 ∧ n = 0 then [pt] else
    let (ix, iy, ni) := if ix.err > t2 ∨ iy.err > t2 then (ix.halve, iy.halve, 2 * ni) else (ix, iy, ni)
    let (ix, iy, ni) := doubles 128 t2 ix iy ni
    pt :: walk64 fuel ox oy n ix iy ni

/-- Bezier64 -/
def bezier64 (q : List (Int × Int)) : List (Int × Int) :=
  let (ox, oy, n) := halves 128 t1 (Ba.new (q.map (·.1))) (Ba.new (q.map (·.2))) 1
  walk64 (2 ^ 22) ox oy n ox oy 0

-- ---- Flatten ----

/-- a path's point: x, y (bits), its type (0 start, 1 line, 3 Bezier), whether it closes -/
structure Pt where
  x : Nat
  y : Nat
  t : Nat
  close : Bool
deriving Inhabited

/-- the figures: each starting at a type 0 point (or the path's first) -/
def figures (ps : List Pt) : List (List Pt) :=
  (ps.foldl (fun (acc : List (List Pt)) p =>
    match acc with
    | [] => [[p]]
    | f :: rest => if p.t = 0 then [p] :: f :: rest else (p :: f) :: rest) []).reverse.map List.reverse

def i32 (v : Int) : Int := let u := v % 2 ^ 32; if u ≥ 2 ^ 31 then u - 2 ^ 32 else u

/-- FixedPointPathEnumerate of one figure: its points through m ceiled, its Beziers flattened -/
def enumFig (r : Ops) (m : List Nat) (f : List Pt) : Option (List (Int × Int)) :=
  let conv (p : Pt) : Int × Int := let (x, y) := apply r m p.x p.y; (ceil16 r x, ceil16 r y)
  let arr := f.toArray
  let rec go (fuel : Nat) (i : Nat) (acc : List (Int × Int)) : Option (List (Int × Int)) :=
    match fuel with
    | 0 => some acc.reverse
    | fuel + 1 =>
      if h : i < arr.size then
        let p := arr[i]
        if p.t = 3 then
          if i + 2 ≥ arr.size then none else
          let q := [conv (arr[i - 1]!), conv p, conv (arr[i + 1]!), conv (arr[i + 2]!)]
          let flat := match bezier32 q with
            | some v => v
            | none => bezier64 q
          go fuel (i + 3) ((flat.map (fun (a, b) => (i32 a, i32 b))).reverse ++ acc)
        else go fuel (i + 1) (conv p :: acc)
      else some acc.reverse
  match f with
  | [] => some []
  | p :: _ => go (f.length + 1) 1 [conv p]

def sixteenth : Nat := 0x3d800000

/-- an i32 as the nearest binary32 (Rust `as f32`) -/
def ofInt (v : Int) : Nat := if v < 0 then neg F (F32.ofNat v.natAbs) else F32.ofNat v.toNat

/-- Path::flatten, its points out (x, y, type byte) -/
def flatten (r : Ops) (m : Option (List Nat)) (fl : Nat) (ps : List Pt) : Option (List (Nat × Nat × Nat)) :=
  let tb (p : Pt) : Nat × Nat × Nat := (p.x, p.y, p.t + (if p.close then 128 else 0))
  if !ps.any (·.t = 3) then
    some (ps.map fun p => match m with
      | none => tb p
      | some m => let (x, y) := apply r m p.x p.y; tb { p with x := x, y := y })
  else if isZero fl then
    let z := r.add 0x80000000 0
    some ((figures ps).flatMap fun f =>
      let cnt := (f.drop 1).foldl (fun (acc : Nat × Nat) (p : Pt) => if p.t = 3 then (acc.1 + (if acc.2 % 3 = 0 then 1 else 0), acc.2 + 1) else (acc.1 + 1, 0)) (0, 0)
      let n := 1 + cnt.1
      let closed := (f.getLast?.map (·.close)).getD false
      (List.range n).map fun (j : Nat) => (z, z, (if j = 0 then 0 else 1) + (if closed && j + 1 == n then 128 else 0)))
  else
    let s := r.mul fl 0x40800000
    let k := r.div k16 s
    let mm := (m.getD [one, 0, 0, one, 0, 0]).map (r.mul · k)
    let back := [s, 0, 0, s, 0, 0]
    let figs := figures ps
    let one_ (f : List Pt) : Option (List (Nat × Nat × Nat)) := do
      let q ← enumFig r mm f
      let closed := (f.getLast?.map (·.close)).getD false
      let n := q.length
      pure ((List.range n).zip q |>.map fun (j, (a, b)) =>
        let (x, y) := apply r back (r.mul (ofInt a) sixteenth) (r.mul (ofInt b) sixteenth)
        (x, y, (if j = 0 then 0 else 1) + (if closed && j + 1 == n then 128 else 0)))
    (figs.mapM one_).map List.flatten

/-- GP.PATH.FLATTEN (RasterizerCeiling): 1 and 1 + 1/32 (16.5 + 1/2 floored, 17, ceiled to 2) and its
negation (-1), a NaN the integer indefinite's 1/16 (-2^27), the same rounding down -/
theorem ceil16_steps :
    (ceil16 nearest one, ceil16 nearest 0x3f840000, ceil16 nearest 0xbf840000, ceil16 nearest 0x7fc00000, ceil16 down 0x3f840000) =
      (1, 2, -1, -134217728, 2) := by decide

/-- GP.PATH.FLATTEN: GDI's 32-bit forward differencing makes a straight Bezier its end alone and an
arch of 160 its 8 points -/
theorem bezier32_steps :
    bezier32 [(0, 0), (16, 0), (32, 0), (48, 0)] = some [(48, 0)] ∧
    bezier32 [(0, 0), (0, 160), (160, 160), (160, 0)] =
      some [(7, 53), (25, 90), (51, 113), (80, 120), (109, 113), (135, 90), (153, 53), (160, 0)] := by decide +kernel

/-- GP.PATH.FLATTEN.BIG: a Bezier spanning 2^14 is past the 32-bit flattener; the 64-bit one makes it
64 points -/
theorem bezier_big :
    bezier32 [(0, 0), (0, 16384), (100, 16384), (100, 0)] = none ∧
    (bezier64 [(0, 0), (0, 16384), (100, 16384), (100, 0)]).length = 64 := by decide +kernel

/-- GP.PATH.FLATTEN: a closed figure of one Bezier of 10, flatness 1/4: its points on the grid of
1/16, the last on the first figure's end and closing it -/
theorem flatten_arch :
    flatten nearest none 0x3e800000 [⟨0, 0, 0, false⟩, ⟨0, 0x41200000, 3, false⟩, ⟨0x41200000, 0x41200000, 3, false⟩, ⟨0x41200000, 0, 3, true⟩] =
      some [(0, 0, 0), (1054867456, 1079246848, 1), (1070071808, 1085538304, 1), (1078722560, 1088552960, 1),
        (1084227584, 1089470464, 1), (1088028672, 1088552960, 1), (1090977792, 1085538304, 1), (1092157440, 1079246848, 1),
        (1092616192, 0, 129)] := by decide +kernel

private def pairs : List Int → Option (List (Int × Int))
  | [] => some []
  | a :: b :: rest => do pure ((a, b) :: (← pairs rest))
  | _ => none

private def flat (l : List (Int × Int)) : List Int := l.flatMap fun (a, b) => [a, b]

private def pts : List Int → Option (List Pt)
  | [] => some []
  | x :: y :: t :: rest => do pure (⟨← nat x, ← nat y, (← nat t) % 128, (← nat t) ≥ 128⟩ :: (← pts rest))
  | _ => none

def mirrors : List Mirror := [
  ⟨"gp.ceil16", fun | [d, v] => do pure [ceil16 (ops (d != 0)) (← nat v)] | _ => none⟩,
  ⟨"gp.apply", fun
    | [d, a, b, c, e, f, g, x, y] => do
      let m ← [a, b, c, e, f, g].mapM nat
      let (px, py) := apply (ops (d != 0)) m (← nat x) (← nat y)
      pure [F32.canon px, F32.canon py]
    | _ => none⟩,
  ⟨"gp.bezier32", fun l => do pure (((bezier32 (← pairs l)).map flat).getD [])⟩,
  ⟨"gp.bezier64", fun l => do pure (flat (bezier64 (← pairs l)))⟩,
  ⟨"gp.flatten", fun
    | d :: hasM :: a :: b :: c :: e :: f :: g :: fl :: rest => do
      let m ← if hasM = 0 then pure none else some <$> [a, b, c, e, f, g].mapM nat
      let out ← flatten (ops (d != 0)) m (← nat fl) (← pts rest)
      pure (out.flatMap fun (x, y, t) => [(F32.canon x : Int), F32.canon y, t])
    | _ => none⟩
]

end Kikizake.GdiPlus.Flatten
