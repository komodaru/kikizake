import Kikizake.Mirror
import Kikizake.F32
/-! GDI's arcs in a path (spec/gdipath.txt GDI.PATH.ARC), in binary32 (Kikizake.F32) as
src/model/gdiarc.rs `arc` computes them (mirror `gdi.arc`): the box in 28.4, each radial's
parametric angle by GDI's arctangent table (32 steps per 45 degrees, linear between) with its
quadrant from the signs, the unit points by GDI's sine table (32 steps per 90 degrees, linear)
or for a sweep under 3 degrees by the series to the 12th power, a part quarter's control points
on the unit circle s + k (T - s), e + k (T - e), Ellipse's whole quarters between, each point
mapped by the half axes and rounded to 28.4 halves away from 0. A value is its bits. -/
namespace Kikizake.Gdi.Arc

open Kikizake.F32 (add sub mul div isNaN)

def ff : Float.Fmt := F32.fmt
def zero : Nat := 0
def one : Nat := 0x3f800000
def neg (x : Nat) : Nat := Float.neg ff x
def abs (x : Nat) : Nat := if Float.sign ff x then x - Float.sbit ff else x
/-- an integer to binary32, nearest (Rust `n as f32`) -/
def ofInt (n : Int) : Nat := if n < 0 then neg (F32.ofNat n.natAbs) else F32.ofNat n.toNat
def lit (n : Nat) : Nat := F32.ofNat n
/-- Rust's comparisons: false with a NaN -/
def ltF (x y : Nat) : Bool := !isNaN x && !isNaN y && F32.lt x y
def leF (x y : Nat) : Bool := !isNaN x && !isNaN y && !F32.lt y x
def eqF (x y : Nat) : Bool := leF x y && leF y x

/-- truncation toward zero to i32 (Rust `as i32`: saturating, NaN 0) -/
def trunc (x : Nat) : Int :=
  if isNaN x then 0
  else if Float.isInf ff x then (if Float.sign ff x then -2147483648 else 2147483647)
  else
    let (s, n, d) := Float.toRat ff x
    let t : Int := ((n / d : Nat) : Int)
    max (-2147483648) (min 2147483647 (if s then -t else t))

/-- the fraction: x itself below 1, 0 from 2^23 on -/
def frac (x : Nat) : Nat :=
  if ltF (abs x) one then x
  else if leF (lit 8388608) (abs x) then zero
  else sub x (ofInt (trunc (abs x)))

/-- GDI's sine table: sin(k pi / 64), k = 0..32, in binary32 -/
def sinTable : List Nat := [0x00000000, 0x3d48fb30, 0x3dc8bd36, 0x3e164083, 0x3e47c5c2, 0x3e78cfcc, 0x3e94a031,
  0x3eac7cd4, 0x3ec3ef15, 0x3edae880, 0x3ef15aea, 0x3f039c3d, 0x3f0e39da, 0x3f187fc0, 0x3f226799, 0x3f2beb4a,
  0x3f3504f3, 0x3f3daef9, 0x3f45e403, 0x3f4d9f02, 0x3f54db31, 0x3f5b941a, 0x3f61c598, 0x3f676bd8, 0x3f6c835e,
  0x3f710908, 0x3f74fa0b, 0x3f7853f8, 0x3f7b14be, 0x3f7d3aac, 0x3f7ec46d, 0x3f7fb10f, 0x3f800000]
/-- GDI's arctangent table: atan(k / 32) in degrees, k = 0..32, and a 0 after it -/
def atanTable : List Nat := [0x00000000, 0x3fe51bca, 0x4064e2aa, 0x40ab62eb, 0x40e40022, 0x410e172e, 0x4129ea1c,
  0x41456ce7, 0x41609474, 0x417b5695, 0x418ad50b, 0x4197c365, 0x41a472c8, 0x41b0e026, 0x41bd08f7, 0x41c8eb2f,
  0x41d4853a, 0x41dfd5f7, 0x41eadcae, 0x41f59908, 0x42000583, 0x4205197c, 0x420a08ba, 0x420ed3a7, 0x42137ac6,
  0x4217feb4, 0x421c601d, 0x42209fbe, 0x4224be63, 0x4228bcdf, 0x422c9c0c, 0x42305ccb, 0x42340000, 0]

def ts (k : Int) : Nat := sinTable.getD k.toNat 0
def ta (k : Int) : Nat := atanTable.getD k.toNat 0

/-- 32 / 90, in binary32 -/
def sineFactor : Nat := div (lit 32) (lit 90)

/-- a unit point (x right, y up) -/
structure Unit where
  x : Nat
  y : Nat
deriving DecidableEq, Repr

/-- the table's cosine and sine of an angle in degrees: 32 steps a quarter, linear between -/
def cosSin (a : Nat) : Unit :=
  let n := ltF a zero
  let v := mul sineFactor (abs a)
  let i := trunc v
  let f := frac v
  let (k, j) := (i / 32, i % 32)
  let atk := fun (k : Int) => if k % 2 = 0 then add (mul (sub (ts (j + 1)) (ts j)) f) (ts j)
    else sub (ts (32 - j)) (mul (sub (ts (32 - j)) (ts (31 - j))) f)
  let s := atk k
  let s := if n != ((k / 2) % 2 = 1) then neg s else s
  let c := atk (k + 1)
  let c := if (k + 1) / 2 % 2 = 1 then neg c else c
  ⟨c, s⟩

/-- the table's cosine (the sine 90 degrees on) -/
def cosT (a : Nat) : Nat := (cosSin (add a (lit 90))).y

/-- the series' cosine and sine for sweeps under 3 degrees: the angle's fraction of a turn
folded into the first quadrant, then x - x^3/3! + ... and 1 - x^2/2! + ... to the 12th power -/
def cosSinPrecise (a : Nat) : Unit :=
  let pi : Nat := 0x40490FDA
  let n := ltF a zero
  let r := mul (frac (div (abs a) (lit 360))) (lit 360)
  let (r, sn) := if ltF (sub (lit 180) r) zero then (sub (lit 360) r, true) else (r, false)
  let (r, cn) := if ltF (sub (lit 90) r) zero then (sub (lit 180) r, true) else (r, false)
  let x := div (mul pi r) (lit 180)
  let (s, c, _, _, _) := (List.range 11).foldl (fun (s, c, term, den, k) i =>
    let e := i + 2
    let term := mul term x
    let v := div term den
    let v := if e / 2 % 2 = 1 then neg v else v
    let (s, c) := if e % 2 = 1 then (add s v, c) else (s, add c v)
    let k := add k one
    (s, c, term, mul den k, k)) (x, one, x, lit 2, lit 2)
  ⟨if cn then neg c else c, if n != sn then neg s else s⟩

/-- the table's angle of (x, y) in [0, 360] and its quadrant from the signs only -/
def arctan (x y : Nat) : Nat × Nat :=
  let (x, code) := if ltF x zero then (neg x, 1) else (x, 0)
  let (y, code) := if ltF y zero then (neg y, code + 2) else (y, code)
  let (x, y, code) := if ltF x y then (y, x, code + 4) else (x, y, code)
  if eqF x zero then (zero, 0) else
  let v := div (mul y (lit 32)) x
  let i := max 0 (min 32 (trunc v))
  let a := add (mul (sub (ta (i + 1)) (ta i)) (frac v)) (ta i)
  let d := match code with
    | 0 => a
    | 1 => sub (lit 180) a
    | 2 => sub (lit 360) a
    | 3 => add (lit 180) a
    | 4 => sub (lit 90) a
    | 5 => add a (lit 90)
    | 6 => add a (lit 270)
    | _ => sub (lit 270) a
  (d, [0, 1, 3, 2, 0, 1, 3, 2].getD code 0)

/-- the axis point and angle quadrant q begins at -/
def axis (q : Nat) : Unit × Nat :=
  match q % 4 with
  | 0 => (⟨one, zero⟩, zero)
  | 1 => (⟨zero, one⟩, lit 90)
  | 2 => (⟨neg one, zero⟩, lit 180)
  | _ => (⟨zero, neg one⟩, lit 270)

/-- the box in 28.4: P0 = (R, T), P1 = (L, T), P2 = (L, B), P3, the half axes and middle -/
structure EBox where
  p0 : Int × Int
  p1 : Int × Int
  p2 : Int × Int
  p3 : Int × Int
  ha : Int × Int
  hb : Int × Int
  c : Int × Int

def EBox.new (p0 p1 p2 : Int × Int) : EBox :=
  let p3 := (p2.1 + p0.1 - p1.1, p2.2 + p0.2 - p1.2)
  let a := (p0.1 - p1.1, p0.2 - p1.2)
  let b := (p1.1 - p2.1, p1.2 - p2.2)
  let ha := ((a.1 + 1) / 2, (a.2 + 1) / 2)
  let hb := ((b.1 + 1) / 2, (b.2 + 1) / 2)
  ⟨p0, p1, p2, p3, ha, hb, (p2.1 + ha.1 + hb.1, p2.2 + ha.2 + hb.2)⟩

/-- a binary32 rounded to 28.4 halves away from 0 (NaN 0) -/
def rnd (v : Nat) : Int :=
  if isNaN v then 0
  else if Float.isInf ff v then (if Float.sign ff v then -2147483647 else 2147483647)
  else
    let (s, n, d) := Float.toRat ff v
    let m : Int := min 2147483647 (((2 * n + d) / (2 * d) : Nat) : Int)
    if s then -m else m

/-- a unit point on the box's ellipse in 28.4 -/
def EBox.at (bx : EBox) (u : Unit) : Int × Int :=
  let x := add (mul (ofInt bx.ha.1) u.x) (mul (ofInt bx.hb.1) u.y)
  let y := add (mul (ofInt bx.ha.2) u.x) (mul (ofInt bx.hb.2) u.y)
  (rnd x + bx.c.1, rnd y + bx.c.2)

abbrev Bez := (Int × Int) × (Int × Int) × (Int × Int)

/-- quarter q's whole Bézier, Ellipse's (control points the half axis times 0x729D7775 / 2^32
in, rounded down) -/
def EBox.quarter (bx : EBox) (q : Nat) : Bez :=
  let k := fun (v : Int) => v * 0x729D7775 / 4294967296
  let (ca, cb) := ((k bx.ha.1, k bx.ha.2), (k bx.hb.1, k bx.hb.2))
  let ad := fun (a b : Int × Int) => (a.1 + b.1, a.2 + b.2)
  let sb := fun (a b : Int × Int) => (a.1 - b.1, a.2 - b.2)
  match q % 4 with
  | 0 => (sb bx.p0 cb, sb bx.p0 ca, sb bx.p0 bx.ha)
  | 1 => (ad bx.p1 ca, sb bx.p1 cb, sb bx.p1 bx.hb)
  | 2 => (ad bx.p2 cb, ad bx.p2 ca, ad bx.p2 bx.ha)
  | _ => (sb bx.p3 ca, ad bx.p3 cb, ad bx.p3 bx.hb)

/-- a part of a quarter from s (angle a0) to e (a1): the control points s + k (T - s) and
e + k (T - e), T where the tangents at s and e cross, k = (4/3 c) / (c + 1) for c the table
cosine of half the span (s and e themselves when the two nearly coincide) -/
def EBox.part (bx : EBox) (s : Unit) (a0 : Nat) (e : Unit) (a1 : Nat) : Bez :=
  let cr := sub (mul s.x e.y) (mul s.y e.x)
  let acr := abs cr
  let near := leF acr 0x37800000
  let tx := div (sub e.y s.y) acr
  let ty := div (sub s.x e.x) acr
  let c := abs (cosT (mul (sub a1 a0) 0x3f000000))
  let k := div (mul 0x3FAAAAAB c) (add c one)
  let m := sub one k
  let kx := mul k tx
  let ky := mul k ty
  let p1 : Unit := if near then s else ⟨add (mul m s.x) kx, add (mul m s.y) ky⟩
  let p2 : Unit := if near then e else ⟨add (mul m e.x) kx, add (mul m e.y) ky⟩
  (bx.at p1, bx.at p2, bx.at e)

/-- a part ends at its end point -/
theorem part_end (bx : EBox) (s : Unit) (a0 : Nat) (e : Unit) (a1 : Nat) : (bx.part s a0 e a1).2.2 = bx.at e := rfl

/-- the arc from s to e (each a unit point, its quadrant, its angle): one part of a quarter, or
(multi) a part to the next axis, the whole quarters on to e's and a part from its axis -/
def EBox.partial (bx : EBox) (s e : Unit × Nat × Nat) (multi : Bool) : List Bez :=
  if !multi then [bx.part s.1 s.2.2 e.1 e.2.2]
  else
    let q := (s.2.1 + 1) % 4
    let (u, d) := axis q
    let first := bx.part s.1 s.2.2 u d
    let mid := ((List.range 4).foldl (fun (acc : List Bez × Nat × Bool) _ =>
      let (l, q, go) := acc
      if go && q != e.2.1 % 4 then (l ++ [bx.quarter q], (q + 1) % 4, true) else (l, q, false)) ([], q, true)).1
    let (u, d) := axis e.2.1
    (first :: mid) ++ [bx.part u d e.1 e.2.2]

/-- the arc's first point, its Béziers and the box's 28.4 middle -/
structure Arc where
  start : Int × Int
  beziers : List Bez
  middle : Int × Int

/-- GDI.PATH.ARC: the arc of the box (l, t) - (r, b) (ordered) from radial rs to radial re,
counterclockwise or (cw) clockwise; none for a box under a pixel either way -/
def arc (l t r b : Int) (rs re : Int × Int) (cw : Bool) : Option Arc :=
  let (t, b) := if cw then (b, t) else (t, b)
  let (fl, ft, fr, fb) := (l * 16, t * 16, r * 16, b * 16)
  if (fr - fl).natAbs < 16 ∨ (fb - ft).natAbs < 16 then none else
  let (fl, fr) := if fr - fl > 0 then (fl, fr - 16) else (fl - 16, fr)
  let (ft, fb) := if fb - ft > 0 then (ft, fb - 16) else (ft - 16, fb)
  let bx := EBox.new (fr, ft) (fl, ft) (fl, fb)
  let half := fun (a b : Int) => Float.round ff (decide (a - b < 0)) (a - b).natAbs 2
  let (cx, cy) := (half l (-r), half t (-b))
  let (ax, ay) := (half r l, half t b)
  let angle := fun (q : Int × Int) => arctan (div (sub (ofInt q.1) cx) ax) (div (sub (ofInt q.2) cy) ay)
  let (a0, q0) := angle rs
  let (a1, q1) := angle re
  let sweep := abs (sub a1 a0)
  let precise := !eqF sweep zero && ltF (sub sweep (lit 3)) zero
  let cs := fun a => if precise then cosSinPrecise a else cosSin a
  let (s, e) := (cs a0, cs a1)
  let multi := q0 != q1 || !ltF a0 a1
  some ⟨bx.at s, bx.partial (s, q0, a0) (e, q1, a1) multi, bx.c⟩

/-- GDI.PATH.ARC: the arc ends at its end radial's point, whatever path it takes there -/
theorem partial_ends (bx : EBox) (s e : Unit × Nat × Nat) (multi : Bool) :
    ((bx.partial s e multi).getLast?).map (·.2.2) = some (bx.at e.1) := by
  cases multi
  · rfl
  · simp only [EBox.partial, Bool.not_true, Bool.false_eq_true, ↓reduceIte]
    rw [List.getLast?_concat]
    rfl

/-- GDI.PATH.ARC: a box under a pixel either way records nothing -/
theorem arc_thin (l t r b : Int) (rs re : Int × Int) (h : (r - l).natAbs = 0) :
    arc l t r b rs re false = none := by
  have : r * 16 - l * 16 = 16 * (r - l) := by omega
  have z : r - l = 0 := by omega
  simp [arc, this, z]

/-- GDI.PATH.ARC: the table's unit points at 0, 90, 180 and 270 degrees are the axes' exactly
(the zeros signed as the table's sign rule gives them), and the arctangent table gives each axis
its angle, its quadrant from the signs only (an axis point counts in the quadrant whose signs
it has: (0, 1) in 0, (-1, 0) in 1) -/
theorem axes_exact :
    cosSin zero = ⟨one, zero⟩ ∧ cosSin (lit 90) = ⟨neg zero, one⟩ ∧ cosSin (lit 180) = ⟨neg one, neg zero⟩ ∧
    cosSin (lit 270) = ⟨zero, neg one⟩ ∧
    arctan one zero = (zero, 0) ∧ arctan zero one = (lit 90, 0) ∧ arctan (neg one) zero = (lit 180, 1) ∧
    arctan zero (neg one) = (lit 270, 3) := by decide +kernel

/-- angles of whole degrees from 1 to 357 in steps of 4 -/
def degrees : List Nat := (List.range 90).map fun k => lit (4 * k + 1)

/-- GDI.PATH.ARC: the sine table is symmetric about 0: the angle turned the other way has the
same cosine and the opposite sine -/
theorem cos_sin_symmetric : degrees.all (fun a => cosSin (neg a) == ⟨(cosSin a).x, neg (cosSin a).y⟩) = true := by decide +kernel

/-- GDI.PATH.ARC: the arctangent of a point mirrored top to bottom is 360 less it (its
quadrant the mirror's: 0 and 3) -/
theorem arctan_mirror : ((List.range 16).map fun k => lit (k + 1)).all (fun y =>
    (arctan (lit 17) (neg y)).1 == sub (lit 360) (arctan (lit 17) y).1 && (arctan (lit 17) (neg y)).2 == 3) = true := by decide +kernel

def pt (p : Int × Int) : List Int := [p.1, p.2]

def mirrors : List Mirror := [
  ⟨"gdi.arc", fun
    | [l, t, r, b, sx, sy, ex, ey, cw] => do
      let a ← arc l t r b (sx, sy) (ex, ey) (cw != 0)
      pure (pt a.start ++ a.beziers.flatMap (fun z => pt z.1 ++ pt z.2.1 ++ pt z.2.2) ++ pt a.middle)
    | _ => none⟩
]

end Kikizake.Gdi.Arc
