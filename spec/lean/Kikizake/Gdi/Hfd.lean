import Kikizake.Mirror
/-! GDI's Bézier flattener (spec/gdipath.txt GDI.PATH.FLATTENHFD): the HFD32 hybrid forward
differencing of dotnet/wpf's bezier.cpp (BEZIER_FLATTEN_GDI_COMPATIBLE) in its own fixed point,
as `bezier::flatten` computes it (mirror `gdi.flatten`). A coordinate is taken from the curve's
bound box, a pixel loose, in 28.4; it starts in 18.14 (shift 10) and the steady state runs in
15.17 (3 more); a curve is cut while its second differences exceed six times 2/3 pixel. -/
namespace Kikizake.Gdi.Hfd

/-- x >> k and x << k on Int: floor division and multiplication by 2^k -/
def shr (x : Int) (k : Nat) : Int := x / (2 ^ k : Nat)
def shl (x : Int) (k : Nat) : Int := x * (2 ^ k : Nat)

def initialShift : Nat := 10
def additionalShift : Nat := 3
def shift : Nat := initialShift + additionalShift
def roundBit : Int := 2 ^ (shift - 1)
/-- six times 2/3 pixel in 18.14, and the same in 15.17 -/
def testInitial : Int := 6 * 0x2aa0
def test : Int := shl testInitial additionalShift
/-- the 32 bit flattener's reach from the loose bound box's corner, in 28.4 -/
def maxSize : Int := 0x4000 + 16

/-- one coordinate's forward differences: the value, the first difference, the second ones
at the two ends -/
structure Basis where
  e0 : Int
  e1 : Int
  e2 : Int
  e3 : Int
deriving Repr, DecidableEq

def Basis.new (p0 p1 p2 p3 : Int) : Basis :=
  ⟨shl p0 initialShift, shl (p3 - p0) initialShift, shl (6 * (p1 - 2 * p2 + p3)) initialShift,
    shl (6 * (p0 - 2 * p1 + p2)) initialShift⟩

def Basis.error (b : Basis) : Int := max b.e2.natAbs b.e3.natAbs
def Basis.parentErrorBy4 (b : Basis) : Int := max b.e3.natAbs (2 * b.e2 - b.e3).natAbs
/-- the point in 28.4, the fixed point rounded half up -/
def Basis.value (b : Basis) : Int := shr (b.e0 + roundBit) shift

/-- the initial halving before the steady state: the step's second difference averaged, the
first difference corrected by it at the shift so far -/
def Basis.lazyHalve (b : Basis) (s : Nat) : Basis :=
  let e2 := shr (b.e2 + b.e3) 1
  { b with e2 := e2, e1 := shr (b.e1 - shr e2 s) 1 }

/-- into the steady fixed point: value and first difference 3 bits finer, the second
differences by the halvings' shift -/
def Basis.steady (b : Basis) (s : Nat) : Basis :=
  { e0 := shl b.e0 additionalShift, e1 := shl b.e1 additionalShift,
    e2 := if s < additionalShift then shl b.e2 (additionalShift - s) else shr b.e2 (s - additionalShift),
    e3 := if s < additionalShift then shl b.e3 (additionalShift - s) else shr b.e3 (s - additionalShift) }

/-- the step halved -/
def Basis.halve (b : Basis) : Basis :=
  let e2 := shr (b.e2 + b.e3) 3
  ⟨b.e0, shr (b.e1 - e2) 1, e2, shr b.e3 2⟩

/-- the step doubled -/
def Basis.double (b : Basis) : Basis :=
  let e3 := shl b.e3 2
  ⟨b.e0, b.e1 + b.e1 + b.e2, shl b.e2 3 - e3, e3⟩

/-- one step along the curve -/
def Basis.step (b : Basis) : Basis :=
  ⟨b.e0 + b.e1, b.e1 + b.e2, b.e2 + b.e2 - b.e3, b.e2⟩

/-- the initial halvings: while either error exceeds the test at the shift so far -/
def initial : Nat → Basis → Basis → Nat → Nat → Basis × Basis × Nat × Nat
  | 0, x, y, s, n => (x, y, s, n)
  | fuel + 1, x, y, s, n =>
    if x.error > shl testInitial s ∨ y.error > shl testInitial s then
      initial fuel (x.lazyHalve (s + 2)) (y.lazyHalve (s + 2)) (s + 2) (2 * n)
    else (x, y, s, n)

/-- the doublings: while the step count is even and both parents' errors are within a
quarter of the test -/
def doubling : Nat → Basis → Basis → Nat → Basis × Basis × Nat
  | 0, x, y, n => (x, y, n)
  | fuel + 1, x, y, n =>
    if n % 2 = 0 ∧ x.parentErrorBy4 ≤ shr test 2 ∧ y.parentErrorBy4 ≤ shr test 2 then
      doubling fuel x.double y.double (n / 2)
    else (x, y, n)

/-- the steady loop: each point (left, top added back), until the steps run out; `none` past
the fuel -/
def steadyLoop (left top : Int) : Nat → Basis → Basis → Nat → Option (List (Int × Int))
  | 0, _, _, _ => none
  | fuel + 1, x, y, n =>
    let p := (x.value + left, y.value + top)
    if n = 0 then some [p]
    else
      let (x, y, n) := if max x.error y.error > test then (x.halve, y.halve, 2 * n) else (x, y, n)
      let (x, y, n) := doubling 64 x y n
      (steadyLoop left top fuel x.step y.step (n - 1)).map (p :: ·)

def minimum (l : List Int) : Int := l.foldl min (l.headD 0)

/-- GDI.PATH.FLATTENHFD: the points after the first that flatten the Bézier p (28.4), or
`none` for a curve the 32 bit flattener does not take (a control point 0x4000 or more from the
loose bound box's corner) -/
def flatten (p : List (Int × Int)) : Option (List (Int × Int)) :=
  match p with
  | [a, b, c, d] =>
    let left := minimum [a.1, b.1, c.1, d.1] - 16
    let top := minimum [a.2, b.2, c.2, d.2] - 16
    let px := [a.1 - left, b.1 - left, c.1 - left, d.1 - left]
    let py := [a.2 - top, b.2 - top, c.2 - top, d.2 - top]
    if (px ++ py).any (· ≥ maxSize) then none
    else
      let x := Basis.new (a.1 - left) (b.1 - left) (c.1 - left) (d.1 - left)
      let y := Basis.new (a.2 - top) (b.2 - top) (c.2 - top) (d.2 - top)
      let (x, y, s, n) := initial 64 x y 0 1
      steadyLoop left top 100000 ((x.steady s).step) ((y.steady s).step) (n - 1)
  | _ => none

/-- `value` is the fixed point rounded to 28.4, halves up (2^13 the fixed point's unit) -/
theorem value_nearest (b : Basis) :
    8192 * b.value ≤ b.e0 + 4096 ∧ b.e0 + 4096 < 8192 * b.value + 8192 := by
  have u : ((2 ^ shift : Nat) : Int) = 8192 := by decide
  have r : roundBit = 4096 := by decide
  unfold Basis.value shr
  rw [u, r]
  have e := Int.emod_add_mul_ediv (b.e0 + 4096) 8192
  have l := Int.emod_nonneg (b.e0 + 4096) (show (8192 : Int) ≠ 0 by decide)
  have h := Int.emod_lt_of_pos (b.e0 + 4096) (show (0 : Int) < 8192 by decide)
  constructor <;> omega

/-- a curve reaching 0x4000 (1024 pixels) or more from its loose bound box's corner is not
flattened here -/
theorem flatten_far (a b c d : Int × Int) (h : d.1 - (minimum [a.1, b.1, c.1, d.1] - 16) ≥ maxSize) :
    flatten [a, b, c, d] = none := by
  simp [flatten, h]

/-- the curves with every control point on a grid of 28.4 values (whole pixels, halves, odd
sixteenths, some pixels apart): flattened by the 32 bit flattener -/
def grid : List (Int × Int) := [(0, 0), (40, 8), (13, 70), (96, 96)]

def curves : List (List (Int × Int)) :=
  grid.flatMap fun a => grid.flatMap fun b => grid.flatMap fun c => grid.map fun d => [a, b, c, d]

/-- what the clause promises of one curve: flattened, ending at its last control point, in at
most 2^6 lines, every point within the control points' bound box widened by a pixel -/
def keeps (p : List (Int × Int)) : Bool :=
  match p, flatten p with
  | [a, b, c, d], some f =>
    let lo := (minimum [a.1, b.1, c.1, d.1] - 16, minimum [a.2, b.2, c.2, d.2] - 16)
    let hi := (-minimum [-a.1, -b.1, -c.1, -d.1] + 16, -minimum [-a.2, -b.2, -c.2, -d.2] + 16)
    f.getLast? == some d && decide (f.length ≤ 64) &&
      f.all fun q => decide (lo.1 ≤ q.1 ∧ q.1 ≤ hi.1 ∧ lo.2 ≤ q.2 ∧ q.2 ≤ hi.2)
  | _, _ => false

set_option maxRecDepth 100000 in
/-- GDI.PATH.FLATTENHFD: every curve of the grid is flattened to lines ending at its last
control point, at most 64 of them, inside the control points' loose box -/
theorem flatten_keeps_the_end : curves.all keeps = true := by decide

def mirrors : List Mirror := [
  ⟨"gdi.flatten", fun
    | [x0, y0, x1, y1, x2, y2, x3, y3] => do
      let f ← flatten [(x0, y0), (x1, y1), (x2, y2), (x3, y3)]
      pure (f.flatMap fun q => [q.1, q.2])
    | _ => none⟩
]

end Kikizake.Gdi.Hfd
