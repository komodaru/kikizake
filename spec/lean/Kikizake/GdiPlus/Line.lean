import Kikizake.GdiPlus.Raster
/-! GDI+'s one-pixel lines (spec/gdiplus.txt GP.PEN.ALIASED, GP.PEN.NOMINAL, GP.PEN.OUTLINE,
GP.PEN.EMPTY) as src/model/gppath.rs computes them: `dda` is `dda` (DrawSolidLineOnePixelAliased:
the ends in 28.4 along the major axis, clipped at the bitmap's pixel-centre edges, a float slope,
an integer error), `sweep` `sweep` (InitializeNominal's half pixel diamond) and `nominal` its fill
by the scan converter with Winding (GdiPlus.Raster), `outlineDda` and `outlineNominal` a path's
figures stroked so, `tooSmall` the device box under 5.96e-4 both ways; on binary32 bits rounded as
the executor rounds (GdiPlus.Matrix); mirrors `gp.dda`, `gp.nominal`, `gp.outline`, `gp.too_small`.
-/
namespace Kikizake.GdiPlus.Line
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Raster

private abbrev F := F32.fmt

/-- an integer as the nearest binary32 (Rust `as f32`) -/
def ofI (v : Int) : Nat := ofInt v

/-- a binary32 floored, as an i64 (Rust `.floor() as i64`: saturated, a NaN 0) -/
def floorI (x : Nat) : Int :=
  if isNaN F x then 0 else
  if isInf F x then (if sign F x then -2 ^ 63 else 2 ^ 63 - 1) else
  max (-2 ^ 63) (min (2 ^ 63 - 1) (floorInt x))

/-- a binary32 floored, as a binary32 -/
def floorF (x : Nat) : Nat := if isNaN F x || isInf F x || isZero x then x else ofI (floorInt x)

/-- x != 0 (a NaN is not 0) -/
def ne0 (x : Nat) : Bool := isNaN F x || !isZero x

/-- v with its low 4 bits cleared (v & !15) -/
def lo4 (v : Int) : Int := v - v % 16

/-- the DDA's step: the error by 2 dmin, past 0 the minor by d -/
def step (d dmaj dmin minor err : Int) : Int × Int × Bool :=
  let err := err + 2 * dmin
  if err > 0 then (minor + d, err - 2 * dmaj, true) else (minor, err, false)

/-- the pixels from major on while one is off the bitmap: skipped -/
def skipOff : Nat → Int → Int → Int → Int → Int → Int → Int → Int → Int × Int × Int
  | 0, major, minor, err, _, _, _, _, _ => (major, minor, err)
  | fuel + 1, major, minor, err, last, cmin, d, dmaj, dmin =>
    if major ≤ last ∧ (major < 0 ∨ ¬(0 ≤ minor ∧ minor ≤ cmin)) then
      let (minor, err, _) := step d dmaj dmin minor err
      skipOff fuel (major + 1) minor err last cmin d dmaj dmin
    else (major, minor, err)

/-- the pixels drawn: each (major, minor) set, until last or the minor's count runs out -/
def plot (ymajor : Bool) (w h : Nat) (d dmaj dmin last : Int) : Nat → Int → Int → Int → Int → Array Nat → Array Nat
  | 0, _, _, _, _, cov => cov
  | fuel + 1, major, minor, err, cnt, cov =>
    if major > last then cov else
    let (x, y) := if ymajor then (minor, major) else (major, minor)
    let cov := if 0 ≤ x ∧ x < w ∧ 0 ≤ y ∧ y < h then cov.set! (y * w + x).toNat 1 else cov
    let (minor, err, stepped) := step d dmaj dmin minor err
    let cnt := if stepped then cnt - 1 else cnt
    if stepped ∧ cnt < 0 then cov else plot ymajor w h d dmaj dmin last fuel (major + 1) minor err cnt cov

/-- GP.PEN.ALIASED: one segment between 28.4 points by the aliased one-pixel DDA, its last pixel
left out when noLast; a segment of no length draws nothing -/
def dda (r : Ops) (p1 p2 : Int × Int) (noLast0 : Bool) (w h : Nat) (cov : Array Nat) : Array Nat := Id.run do
  if p1 = p2 then return cov
  let ymajor := (p2.2 - p1.2).natAbs ≥ (p2.1 - p1.1).natAbs
  let mm (p : Int × Int) : Int × Int := if ymajor then (p.2, p.1) else (p.1, p.2)
  let (a, b) := (mm p1, mm p2)
  let reversed := b.1 < a.1
  let (a, b) := if reversed then (b, a) else (a, b)
  let mut (aMaj, aMin) := a
  let mut (bMaj, bMin) := b
  let mut noLast := noLast0
  let d : Int := if bMin - aMin < 0 then -1 else 1
  let (dmaj, dmin) := (bMaj - aMaj, (bMin - aMin) * d)
  let slope := r.div (ofI (d * (dmin.natAbs : Int))) (ofI dmaj)
  let invslope := if isZero slope then 0 else r.div one slope
  let (wmaj, wmin) : Int × Int := if ymajor then (h, w) else (w, h)
  let (lo, hi) := ((-8 : Int), wmaj * 16 - 8)
  let (nlo, nhi) := ((-8 : Int), wmin * 16 - 8)
  if bMaj < lo ∨ aMaj > hi then return cov
  let mut o50 := 0
  let mut o54 := 0
  if aMaj < lo then
    let v := r.add (r.mul (ofI (lo - aMaj)) slope) (ofI aMin)
    let f := floorF v
    aMin := floorI f
    aMaj := lo
    o50 := r.sub v f
  if bMaj > hi then
    let v := r.add (r.mul (ofI (hi - bMaj)) slope) (ofI bMin)
    let f := floorF v
    bMin := floorI f
    bMaj := hi
    o54 := r.sub v f
    noLast := false
  let (startEdge, endEdge) := if d > 0 then (nlo, nhi) else (nhi, nlo)
  let outside (v edge : Int) (start : Bool) : Bool := if (d > 0) == start then v < edge else v > edge
  if outside aMin startEdge true then
    let k := floorI (r.mul (r.sub (ofI startEdge) (r.add (ofI aMin) o50)) invslope)
    aMaj := aMaj + k
    aMin := startEdge
    o50 := 0
  if outside bMin endEdge false then
    let k := floorI (r.mul (r.sub (ofI endEdge) (r.add (ofI bMin) o54)) invslope)
    bMaj := bMaj + k
    bMin := endEdge
    o54 := 0
    noLast := false
  if bMaj < aMaj then return cov
  if ne0 o54 ∧ bMin % 16 = 8 then bMin := bMin + 1
  let e3c := if ne0 o50 then floorI (r.mul (ofI (2 * dmaj * d)) o50) else 0
  let e := !ymajor && dmaj == dmin && d == 1
  let rnd : Int := if e then 8 else 7
  let mut ebp := lo4 (aMaj + rnd)
  let mut esi := lo4 (bMaj + rnd)
  let inside (r9 r10 : Int) : Bool :=
    let s := (r9.natAbs : Int) + r10.natAbs
    s < 8 || (if ymajor then (r9 == 0 && r10 == 8) || (r10 == 0 && r9 == 8)
      else (r10 == 0 && r9 == (if e then -8 else 8)) || (r9 == 0 && r10 == 8) ||
        (dmaj == dmin && s == 8 && (if e then r9 < 0 else r9 > 0) && r10 > 0))
  let startIn := inside (aMaj - ebp) (aMin - lo4 (aMin + 7))
  let endIn := inside (bMaj - esi) (bMin - lo4 (bMin + 7))
  if (reversed && noLast && (startIn || aMaj % 16 ≤ 8)) || (!(reversed && noLast) && aMaj % 16 ≤ 8 && !startIn) then
    ebp := ebp + 16
  let m0 := floorI (r.add (r.add (r.mul (ofI (ebp - aMaj)) slope) (ofI aMin)) o50)
  if (!reversed && noLast && (bMaj % 16 > 8 || endIn)) || (!(!reversed && noLast) && !endIn && bMaj % 16 > 8) then
    esi := esi - 16
  let m1 := floorI (r.add (r.add (r.mul (ofI (esi - bMaj)) slope) (ofI bMin)) o54)
  let first := ebp / 16
  let last := esi / 16
  if last < first then return cov
  let minor := lo4 (m0 + 7) / 16
  let minorEnd := (m1 + 7) / 16
  let err := (e3c - ((lo4 (m0 + 7) - m0) * d + 8) * 2 * dmaj) / 16
  let (cmaj, cmin) := (wmaj - 1, wmin - 1)
  let last := min last cmaj
  let minorEnd := if d > 0 then min minorEnd cmin else max minorEnd 0
  let fuel := (last - first + 2).toNat
  let (major, minor, err) := skipOff fuel first minor err last cmin d dmaj dmin
  let cnt := (minorEnd - minor) * d
  return plot ymajor w h d dmaj dmin last fuel major minor err cnt cov

/-- the half pixel offsets along the minor axis, by octant class -/
def O (k : Nat) : Int × Int := [(0, -8), (-8, 0), (0, 8), (8, 0)].getD (k % 4) (0, 0)
/-- a segment's octant class: x-major right 0, left 2; y-major down 3, up 1 (a diagonal x-major) -/
def cls (p q : Int × Int) : Nat :=
  let (dx, dy) := (q.1 - p.1, q.2 - p.2)
  let idx := (if dx < 0 then 1 else 0) + (if dy < 0 then 2 else 0) + (if dy.natAbs > dx.natAbs then 4 else 0)
  [0, 2, 0, 2, 3, 3, 1, 1].getD idx 0
def add2 (p q : Int × Int) : Int × Int := (p.1 + q.1, p.2 + q.2)
def sub2 (p q : Int × Int) : Int × Int := (p.1 - q.1, p.2 - q.2)

/-- repeats dropped -/
def dedup (ps : List (Int × Int)) : List (Int × Int) :=
  (ps.foldl (fun acc q => if acc.head? = some q then acc else q :: acc) []).reverse

/-- the forward chain and the backward stack along the polyline from segment i, of class k -/
def chains (p : Array (Int × Int)) : Nat → Nat → Nat → List (Int × Int) × List (Int × Int) → List (Int × Int) × List (Int × Int) × Nat
  | 0, _, k, (fwd, back) => (fwd, back, k)
  | fuel + 1, i, k, (fwd, back) =>
    if i + 1 ≥ p.size then (fwd, back, k) else
    let (pi, pn) := (p[i]!, p[i + 1]!)
    let fwd := fwd ++ [add2 pi (O k), add2 pn (O k)]
    let back := back ++ [sub2 pi (O k), sub2 pn (O k)]
    if i + 2 ≥ p.size then (fwd, back, k) else
    let next := p[i + 2]!
    let k2 := cls pn next
    if k2 = k then chains p fuel (i + 1) k (fwd, back) else
    let a := (pn.1 - pi.1) * (next.2 - pn.2)
    let b := (pn.2 - pi.2) * (next.1 - pn.1)
    let (fwd, back) :=
      if a < b then (fwd ++ [pn], if k2 ≠ (k + 1) % 4 then back ++ [sub2 pn (O (k + 1))] else back)
      else ((if k2 ≠ (k + 3) % 4 then fwd ++ [add2 pn (O (k + 3))] else fwd), back ++ [pn])
    chains p fuel (i + 1) k2 (fwd, back)

/-- GP.PEN.NOMINAL (InitializeNominal): a polyline swept by the half pixel diamond, the forward
chain then the backward stack reversed; one point its segment of no length (the whole diamond);
none for no point -/
def sweep (pts : List (Int × Int)) : Option (List (Int × Int)) :=
  let p := dedup pts
  match p with
  | [] => none
  | _ =>
    let p := if p.length = 1 then p ++ p else p
    let arr := p.toArray
    let k := cls arr[0]! arr[1]!
    let (fwd, back, k) := chains arr arr.size 0 k ([sub2 arr[0]! (O k), add2 arr[0]! (O (k + 1))], [])
    let e := arr[arr.size - 1]!
    some (fwd ++ [add2 e (O (k + 3)), sub2 e (O k)] ++ back.reverse)

/-- GP.PEN.NOMINAL: figures (28.4, closed or not) stroked one pixel wide: each swept (a closed one
with its closing segment), filled with Winding -/
def nominal (figs : List (List (Int × Int) × Bool)) (rows : Option Nat) (w h : Nat) : Array Nat :=
  let outlines := figs.filterMap fun (pts, closed) => sweep (if closed then pts ++ pts.take 1 else pts)
  rasterize outlines true rows w h

/-- FixedPointPathEnumerate with each figure's close -/
def enumerate (r : Ops) (m : List Nat) (ps : List Pt) : Option (List (List (Int × Int) × Bool)) :=
  (figures ps).mapM fun f => do
    let q ← enumFig r m f
    pure (q, (f.getLast?.map (·.close)).getD false)

/-- GP.PEN.OUTLINE (SolidStrokePathOnePixel): each segment by the DDA, an open figure's last pixel
by noLast, a closed figure's closing segment too, always with its last pixel -/
def outlineDda (r : Ops) (ps : List Pt) (m : List Nat) (noLast : Bool) (w h : Nat) : Option (Array Nat) := do
  let figs ← enumerate r m ps
  pure (figs.foldl (fun cov (pts, closed) =>
    let pts := if closed then pts ++ pts.take 1 else pts
    (pts.zip (pts.drop 1)).foldl (fun cov (a, b) => dda r a b (if closed then false else noLast) w h cov) cov)
    (Array.replicate (w * h) 0))

/-- GP.PEN.OUTLINE: a path stroked one pixel wide by the nominal rasterizer -/
def outlineNominal (r : Ops) (ps : List Pt) (m : List Nat) (rows : Option Nat) (w h : Nat) : Option (Array Nat) := do
  pure (nominal (← enumerate r m ps) rows w h)

/-- f32 min and max ignoring a NaN (Rust `f32::min`, `f32::max`) -/
def fmin (x y : Nat) : Nat := if isNaN F x then y else if isNaN F y then x else if F32.lt y x then y else x
def fmax2 (x y : Nat) : Nat := if isNaN F x then y else if isNaN F y then x else if F32.lt x y then y else x

/-- GP.PEN.EMPTY: the points through the world to device matrix span at most 5.96e-4 both ways -/
def tooSmall (r : Ops) (m : List Nat) (pts : List (Nat × Nat)) : Bool :=
  let d := pts.map fun (x, y) => apply r m x y
  let size (k : Nat × Nat → Nat) :=
    let (lo, hi) := d.foldl (fun (acc : Nat × Nat) v => (fmin acc.1 (k v), fmax2 acc.2 (k v))) (0x7f800000, 0xff800000)
    r.sub hi lo
  let wide (v : Nat) := flt near v
  !wide (size (·.1)) && !wide (size (·.2))

/-- GP.PEN.ALIASED: a horizontal segment from pixel (0, 1) to (3, 1) draws 4 pixels, 3 without its
last; GP.PEN.EMPTY: a segment of no length none; the nominal rasterizer's one point its whole
diamond, one pixel; (3, 3) - (3.0001, 3) is too small, (3, 3) - (3.001, 3) not -/
theorem lines :
    dda nearest (0, 16) (48, 16) false 4 2 (Array.replicate 8 0) = #[0, 0, 0, 0, 1, 1, 1, 1] ∧
    dda nearest (0, 16) (48, 16) true 4 2 (Array.replicate 8 0) = #[0, 0, 0, 0, 1, 1, 1, 0] ∧
    dda nearest (32, 16) (32, 16) false 4 2 (Array.replicate 8 0) = Array.replicate 8 0 ∧
    nominal [([(32, 32)], false)] none 4 4 = #[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0] ∧
    tooSmall nearest [one, 0, 0, one, 0, 0] [(0x40400000, 0x40400000), (0x404001a3, 0x40400000)] = true ∧
    tooSmall nearest [one, 0, 0, one, 0, 0] [(0x40400000, 0x40400000), (0x40401062, 0x40400000)] = false := by
  decide +kernel

private def ipairs : List Int → Option (List (Int × Int))
  | [] => some []
  | a :: b :: rest => do pure ((a, b) :: (← ipairs rest))
  | _ => none

private def npairs : List Int → Option (List (Nat × Nat))
  | [] => some []
  | a :: b :: rest => do pure ((← nat a, ← nat b) :: (← npairs rest))
  | _ => none

private def pts : List Int → Option (List Pt)
  | [] => some []
  | x :: y :: t :: rest => do pure (⟨← nat x, ← nat y, (← nat t) % 128, (← nat t) ≥ 128⟩ :: (← pts rest))
  | _ => none

/-- figures of 28.4 points: each its closed flag, its count, then its points -/
private def figs : Nat → List Int → Option (List (List (Int × Int) × Bool))
  | 0, _ => some []
  | fuel + 1, l => match l with
    | [] => some []
    | closed :: n :: rest => do
      let k := (← nat n) * 2
      let f ← ipairs (rest.take k)
      pure ((f, closed != 0) :: (← figs fuel (rest.drop k)))
    | _ => none

private def out (a : Array Nat) : List Int := a.toList.map fun (v : Nat) => (v : Int)

def mirrors : List Mirror := [
  ⟨"gp.dda", fun
    | [d, x1, y1, x2, y2, noLast, w, h] => do
      let (w, h) := (← nat w, ← nat h)
      pure (out (dda (ops (d != 0)) (x1, y1) (x2, y2) (noLast != 0) w h (Array.replicate (w * h) 0)))
    | _ => none⟩,
  ⟨"gp.sweep", fun
    | rest => do
      pure (match sweep (← ipairs rest) with
        | none => [-1]
        | some q => q.flatMap fun (x, y) => [x, y])⟩,
  ⟨"gp.nominal", fun
    | rows :: w :: h :: rest => do
      let rows ← if rows = 0 then pure none else some <$> nat rows
      pure (out (nominal (← figs rest.length rest) rows (← nat w) (← nat h)))
    | _ => none⟩,
  ⟨"gp.outline", fun
    | d :: how :: rows :: w :: h :: a :: b :: c :: e :: f :: g :: rest => do
      let m ← [a, b, c, e, f, g].mapM nat
      let r := ops (d != 0)
      let res ← if how = 2 then
          (do let rows ← if rows = 0 then pure none else some <$> nat rows
              pure (outlineNominal r (← pts rest) m rows (← nat w) (← nat h)))
        else pure (outlineDda r (← pts rest) m (how = 1) (← nat w) (← nat h))
      pure ((res.map out).getD [-1])
    | _ => none⟩,
  ⟨"gp.too_small", fun
    | d :: a :: b :: c :: e :: f :: g :: rest => do
      let m ← [a, b, c, e, f, g].mapM nat
      pure [if tooSmall (ops (d != 0)) m (← npairs rest) then 1 else 0]
    | _ => none⟩
]

end Kikizake.GdiPlus.Line
