import Kikizake.GdiPlus.Linear
import Kikizake.GdiPlus.Line
import Kikizake.GdiPlus.Stroke
/-! GDI+ DrawImage's chain from the rectangles to the span (spec/gdiplus.txt GP.DRAW.NEAREST) as
src/model/gpimage.rs `chain` computes it: GpGraphics::DrawImage's matrix from the source rectangle
as given, the source normalized, the half pixel offset; the rotate / flip branch of a scale of +-1
(`analyze`, `integerTranslate`); ApplyVisibleClipToSourceRect; PipeLockBits' rows and Margin; the
span matrix by InferAffineMatrix (GdiPlus.Linear), the device rectangle and pixels, the copy span of
an integer translation; on binary32 bits rounded as the executor rounds (GdiPlus.Matrix), binary64
where the model takes it; mirror `gp.chain`. -/
namespace Kikizake.GdiPlus.Chain
open Kikizake.Float
open Kikizake.GdiPlus.Matrix
open Kikizake.GdiPlus.Flatten
open Kikizake.GdiPlus.Linear
open Kikizake.GdiPlus.Line

private abbrev F := F32.fmt
private abbrev D := F64.fmt

/-- the square root in binary64: to nearest, or the largest binary64 at most it (gpwiden.rs
`sqrt64`) -/
def sqrt64 (d : Bool) (x : Nat) : Nat :=
  if isNaN D x then qnan D
  else if sig D x = 0 then x
  else if sign D x then qnan D
  else if isInf D x then x
  else
    let e := ex D x
    let s := sig D x
    let (n, e) := if e % 2 = 0 then (s, e) else (2 * s, e - 1)
    let N := n * 2 ^ 120
    let q := Stroke.isqrt N
    let (a, b) := if q * q = N then (q, 1) else (2 * q + 1, 2)
    let (pn, pd) := pow2 (e / 2 - 60)
    if d then roundDown D false (a * pn) (b * pd) else Float.round D false (a * pn) (b * pd)

def fle (x y : Nat) : Bool := !isNaN F x && !isNaN F y && !F32.lt y x

/-- the span kinds: 0 nearest, 1 bilinear, 2 bicubic, 3 HQ tent, 4 HQ cubic -/
def margin (r : Ops) (span : Nat) (ratio : Nat) : Nat :=
  let k := if span = 3 then one else if span = 4 then 0x40000000 else 0
  if span = 0 then 0 else if span = 1 then one else if span = 2 then 0x40000000
  else if fle one ratio then k else r.div k ratio

/-- a binary32 ceiled to an i64 (Rust `.ceil() as i64`) -/
def ceilI (x : Nat) : Int := -(floorI (neg F x))

/-- IsIntegerTranslate: no scale, each translation within 1/64 of an integer -/
def integerTranslate (r : Ops) (a : List Nat) : Bool :=
  complexity r a ≤ 1 && [a.getD 4 0, a.getD 5 0].all fun v => fle (fabs (r.sub (floorF (r.add v half)) v)) 0x3c800000

/-- AnalyzeRotateFlip: 0 none, 1 X, 2 Y, 3 both -/
def analyze (r : Ops) (m : List Nat) : Nat :=
  let e (i : Nat) := m.getD i 0
  let nr (a c : Nat) := flt (fabs (r.sub a c)) epsF
  if !(flt (fabs (e 1)) epsF && flt (fabs (e 2)) epsF) then 0
  else if nr (e 0) one && nr (e 3) (neg F one) then 2
  else if nr (e 0) (neg F one) && nr (e 3) one then 1
  else if nr (e 0) (neg F one) && nr (e 3) (neg F one) then 3
  else 0

/-- what the span gets -/
structure Out where
  top : Int
  rows : Int
  sf : List Nat
  ms2 : List Nat
  dr : List Nat
  cols : Int × Int
  lines : Int × Int
  flip : Nat
  copy : Option (Int × Int)

/-- the binary64 to 1/16, then ceiled (the device pixels' edges) -/
def edge (v t : Nat) : Int :=
  let s := F64.add (F64.ofF32 v) (F64.ofF32 t)
  let q := F64.add (F64.mul s 0x4030000000000000) 0x3fe0000000000000
  -- (floor(q) / 16, ceiled: an integer over 16 ceiled is the integer's ceiling of a sixteenth)
  if isNaN D q then 0 else
  if isInf D q then (if sign D q then -2 ^ 63 else 2 ^ 63 - 1) else
  let (sg, n, dd) := toRat D q
  let f : Int := if sg then -(((n + dd - 1) / dd : Nat) : Int) else ((n / dd : Nat) : Int)
  Int.neg ((-f) / 16)


/-- the larger, the second on a tie (the signed zeros among them), the other past a NaN
(gpimage.rs `most`) -/
def most (x y : Nat) : Nat := if isNaN F y then x else if F32.lt y x then x else y

/-- the smaller, the second on a tie, the other past a NaN (gpimage.rs `least`) -/
def least (x y : Nat) : Nat := if isNaN F y then x else if F32.lt x y then x else y

/-- GP.DRAW.NEAREST: the chain of a draw of src onto a bw x bh bitmap at dst (integers, sizes
signed) from an iw x ih image: none where the model fails (a source of no area), else the empty
draw (no rows locked) or what the span gets -/
def chain (d : Bool) (span : Nat) (dst src : List Int) (iw ih bw bh : Int) (half_ : Bool) : Option (Option Out) := Id.run do
  let r := ops d
  let g (l : List Int) (i : Nat) := ofI (l.getD i 0)
  let (dx, dy, dw, dh) := (g dst 0, g dst 1, g dst 2, g dst 3)
  let mut (sx, sy, sw, sh) := (g src 0, g src 1, g src 2, g src 3)
  let (xr, yb, sxr, syb) := (r.add dx dw, r.add dy dh, r.add sw sx, r.add sh sy)
  let mut (m11, m22, tx, ty) := (one, one, 0, 0)
  if !(feq sx sxr || feq sy syb) then
    m11 := r.div (r.sub xr dx) (r.sub sxr sx)
    tx := r.sub xr (r.mul m11 sxr)
    m22 := r.div (r.sub yb dy) (r.sub syb sy)
    ty := r.sub yb (r.mul m22 syb)
  if flt sw 0 then
    sx := sxr
    sw := neg F sw
  if flt sh 0 then
    sy := syb
    sh := neg F sh
  if half_ then
    tx := r.add tx (r.add (r.mul 0 half) (r.mul m11 half))
    ty := r.add ty (r.add (r.mul 0 half) (r.mul m22 half))
    sx := r.add sx 0xbf000000
    sy := r.add sy 0xbf000000
  let ms := [m11, 0, 0, m22, tx, ty]
  let mut p := [apply r ms sx sy, apply r ms (r.add sx sw) sy, apply r ms sx (r.add sy sh)]
  let mut s := [sx, sy, sw, sh]
  let si (i : Nat) (s : List Nat) := s.getD i 0
  let o := if half_ then 0xbf000000 else 0
  let mut c := [one, 0, 0, one, o, o]
  let some m0 := infer r p sx sy sw sh | return none
  let mut m := matMul r m0 c
  let flip := if integerTranslate r m then 0 else analyze r m
  if flip ≠ 0 then
    let mut fx := m.getD 4 0
    let mut fy := m.getD 5 0
    if half_ then
      fx := r.add (r.add fx half) (r.sub (r.mul (m.getD 0 0) 0xbf000000) (r.mul (m.getD 2 0) half))
      fy := r.add (r.add fy half) (r.sub (r.mul (m.getD 3 0) 0xbf000000) (r.mul (m.getD 1 0) half))
      s := s.set 0 (r.add (si 0 s) half) |>.set 1 (r.add (si 1 s) half)
    let (w, h) := (ofI iw, ofI ih)
    let mirror (v n size : Nat) := r.add v (r.sub n (r.add (r.mul v 0x40000000) size))
    if flip = 1 then
      fx := r.add fx (r.add (r.mul (neg F w) one) (r.mul 0 0))
      fy := r.add fy (r.add (r.mul (neg F w) 0) (r.mul 0 one))
      s := s.set 0 (mirror (si 0 s) w (si 2 s))
    else if flip = 2 then
      fy := r.add fy (r.add (neg F h) 0)
      fx := r.add fx (r.add (r.mul (neg F h) 0) 0)
      s := s.set 1 (mirror (si 1 s) h (si 3 s))
    else
      fx := r.add fx (r.add (r.mul (neg F w) one) (r.mul (neg F h) 0))
      fy := r.add fy (r.add (r.mul (neg F w) 0) (r.mul (neg F h) one))
      s := s.set 1 (mirror (si 1 s) h (si 3 s))
      s := s.set 0 (mirror (si 0 s) w (si 2 s))
    c := [one, 0, 0, one, fx, fy]
    p := [(si 0 s, si 1 s), (r.add (si 0 s) (si 2 s), si 1 s), (si 0 s, r.add (si 1 s) (si 3 s))]
    let some m1 := infer r p (si 0 s) (si 1 s) (si 2 s) (si 3 s) | return none
    m := matMul r m1 c
  -- (ApplyVisibleClipToSourceRect)
  let some shift := inverse r c | return none
  let (vx, vy) := (r.add 0 (shift.getD 4 0), r.add 0 (shift.getD 5 0))
  let (vw, vh) := (r.sub (r.add (ofI bw) (shift.getD 4 0)) vx, r.sub (r.add (ofI bh) (shift.getD 5 0)) vy)
  let some q := inverse r (matMul r m shift) | return none
  let ys := [(vx, vy), (r.add vx vw, vy), (vx, r.add vy vh), (r.add vx vw, r.add vy vh)].map fun (x, y) => (apply r q x y).2
  let eps := neg F epsF
  let mut top := r.add (si 1 s) one
  let mut hh := r.add (si 3 s) 0xc0000000
  let mut left := r.add (si 0 s) one
  let mut ww := r.add (si 2 s) 0xc0000000
  let mut changed := false
  for k in [0:4] do
    let yk := ys.getD k 0
    if flt top yk && flt yk (r.add hh top) then
      let others := (ys.zipIdx.filter (·.2 ≠ k)).map (·.1)
      if others.all (fun yj => flt eps (r.sub yj yk)) then
        let dd := r.sub yk top
        top := yk
        hh := r.sub hh dd
        changed := true
      else if others.all (fun yj => flt eps (r.sub yk yj)) then
        hh := r.sub yk top
        changed := true
  if changed then
    let e2 := m.getD 2 0
    let e3 := m.getD 3 0
    let scale := to32 d (sqrt64 d (F64.ofF32 (r.add (r.mul e2 e2) (r.mul e3 e3))))
    let mg := margin r span scale
    ww := r.add ww 0x40000000
    left := r.sub left one
    top := r.sub top one
    hh := r.add hh 0x40000000
    left := r.sub left 0
    ww := r.add ww 0
    top := r.sub top mg
    let bot := least (r.add (r.add (r.add mg mg) hh) top) (r.add (si 1 s) (si 3 s))
    let right := least (r.add ww left) (r.add (si 0 s) (si 2 s))
    let (lf, tp) := (most left (si 0 s), most top (si 1 s))
    s := [lf, tp, r.sub right lf, r.sub bot tp]
    let some b := inverse r q | return none
    p := [apply r b lf tp, apply r b (r.add (si 2 s) lf) tp, apply r b lf (r.add (si 3 s) tp)]
  -- (PipeLockBits)
  let s1 := if half_ then [r.add (si 0 s) half, r.add (si 1 s) half, si 2 s, si 3 s] else s
  let mut l := m
  if half_ then
    l := l.set 4 (r.add (l.getD 4 0) half) |>.set 5 (r.add (l.getD 5 0) half)
    l := l.set 4 (r.add (l.getD 4 0) (r.add (r.mul 0xbf000000 (l.getD 0 0)) (r.mul 0xbf000000 (l.getD 2 0))))
    l := l.set 5 (r.add (l.getD 5 0) (r.add (r.mul 0xbf000000 (l.getD 1 0)) (r.mul 0xbf000000 (l.getD 3 0))))
  let ya := (apply r l (si 0 s1) (si 1 s1)).2
  let yz := (apply r l (r.add (si 2 s1) (si 0 s1)) (r.add (si 3 s1) (si 1 s1))).2
  let oy := least ya yz
  let oh := r.sub (most ya yz) oy
  let oh := if flt oh near then 0 else oh
  let ratio := r.div (fabs (r.sub (r.add oy oh) oy)) (fabs (si 3 s1))
  let mg := ceilI (margin r span ratio)
  let first := floorI (si 1 s1) - mg
  let last := ceilI (r.add (fabs (si 3 s1)) (si 1 s1)) + mg
  if first > ih ∨ last < 0 then return some none
  let (first, last) := (max first 0, min last ih)
  let lock := [si 0 s1, r.sub (si 1 s1) (ofI first), si 2 s1, si 3 s1]
  let sf := if half_ then [r.add (si 0 lock) 0xbf000000, r.add (si 1 lock) 0xbf000000, si 2 lock, si 3 lock] else lock
  let some m2 := infer r p (si 0 sf) (si 1 sf) (si 2 sf) (si 3 sf) | return none
  let ms2 := matMul r m2 c
  let e (i : Nat) := ms2.getD i 0
  let x0 := r.add (r.mul (si 0 sf) (e 0)) (e 4)
  let x2 := r.add (r.mul (r.add (si 0 sf) (si 2 sf)) (e 0)) (e 4)
  let y0 := r.add (r.mul (si 1 sf) (e 3)) (e 5)
  let y2 := r.add (r.mul (r.add (si 1 sf) (si 3 sf)) (e 3)) (e 5)
  let pp (i : Nat) := p.getD i (0, 0)
  let (xl, xh) := (least (pp 0).1 (pp 1).1, most (pp 0).1 (pp 1).1)
  let (yl, yh) := (least (pp 0).2 (pp 2).2, most (pp 0).2 (pp 2).2)
  let cc (i : Nat) := c.getD i 0
  let copy := if integerTranslate r ms2 then some (-(floorI (r.add (e 4) half)), -(floorI (r.add (e 5) half))) else none
  return some (some ⟨first, last - first, sf, ms2, [x0, y0, r.sub x2 x0, r.sub y2 y0],
    (max (edge xl (cc 4)) 0, min (edge xh (cc 4)) bw), (max (edge yl (cc 5)) 0, min (edge yh (cc 5)) bh), flip, copy⟩)

/-- the answer: -1 for a failure, 0 for an empty draw, else 1 and what the span gets -/
def answer (o : Option (Option Out)) : List Int :=
  match o with
  | none => [-1]
  | some none => [0]
  | some (some c) =>
    let fl (l : List Nat) : List Int := l.map fun v => (F32.canon v : Int)
    [1, c.top, c.rows] ++ fl c.sf ++ fl c.ms2 ++ fl c.dr ++ [c.cols.1, c.cols.2, c.lines.1, c.lines.2, c.flip] ++
      (match c.copy with | none => [0] | some (a, b) => [1, a, b])

/-- GP.DRAW.NEAREST: a 4 x 4 image drawn 1:1 at (2, 3) onto 8 x 8 locks its 4 rows and is the copy
span reading (x - 2, y - 3); drawn at twice the size it is a scale, columns 2 .. 10 cut to 8 -/
theorem copies :
    answer (chain false 0 [2, 3, 4, 4] [0, 0, 4, 4] 4 4 8 8 false) =
      [1, 0, 4, 0, 0, 0x40800000, 0x40800000, 0x3f800000, 0, 0, 0x3f800000, 0x40000000, 0x40400000, 0x40000000, 0x40400000, 0x40800000, 0x40800000,
        2, 6, 3, 7, 0, 1, -2, -3] ∧
    (answer (chain false 0 [2, 3, 8, 8] [0, 0, 4, 4] 4 4 8 8 false)).take 3 = [1, 0, 4] := by decide +kernel

/-- a 32-bit wrapping integer -/
def w32 (v : Int) : Int := i32 v

/-- floor(v 65536 + 1/2) as an i32, the add rounded (`fix16`) -/
def fix16 (r : Ops) (v : Nat) : Int := floorSat (r.add (r.mul v 0x47800000) half)
/-- floor(v 2048 + 1/2) as an i32 (`fix11`) -/
def fix11 (r : Ops) (v : Nat) : Int := floorSat (r.add (r.mul v 0x45000000) half)

def channel (p : Nat) (c : Nat) : Int := (p / 2 ^ (8 * c) % 256 : Nat)
def pack (c : List Int) : Nat := (List.range 4).foldl (fun o k => o + (c.getD k 0 % 256).toNat * 2 ^ (8 * k)) 0

/-- the span's pixels of one row y: nearest (0) or bilinear (1, MMX on x86) -/
def row (r : Ops) (span : Nat) (x86 : Bool) (inv : List Nat) (xmin xmax y : Int) (rows iw : Int) (tap : Int → Int → Nat) : List (Int × Int × Nat) :=
  let n := ofI (xmax - xmin)
  let e (i : Nat) := inv.getD i 0
  let (u0, v0) := apply r inv (ofI xmin) (ofI y)
  let (u1, v1) := apply r inv (ofI xmax) (ofI y)
  let xs : List Int := (List.range (xmax - xmin).toNat).map Int.ofNat
  if span = 0 then
    let (bu, bv) := (fix16 r u0, fix16 r v0)
    let du := floorSat (r.add (r.div (r.mul (r.sub u1 u0) 0x47800000) n) half)
    let dv := floorSat (r.add (r.div (r.mul (r.sub v1 v0) 0x47800000) n) half)
    xs.map fun i =>
      let su := w32 (bu + w32 (i * du) + 0x8000) / 2 ^ 16
      let sv := w32 (bv + w32 (i * dv) + 0x8000) / 2 ^ 16
      (xmin + i, y, tap su sv)
  else if x86 then
    let du := fix16 r (r.add (r.mul (e 2) 0) (e 0))
    let dv := fix16 r (r.add (r.mul 0 (e 3)) (e 1))
    let f (i : Nat) := fix16 r (e i)
    let bu := w32 (w32 (f 0 * xmin) + w32 (f 2 * y) + f 4)
    let bv := w32 (w32 (f 1 * xmin) + w32 (f 3 * y) + f 5)
    let lerp (a b t : Int) : Int := (a + (((b - a) * t + 128) % 65536) / 256) % 256
    xs.map fun i =>
      let (pu, pv) := (w32 (bu + w32 (i * du)), w32 (bv + w32 (i * dv)))
      let (ix, iy) := (pu / 2 ^ 16, pv / 2 ^ 16)
      let (fx, fy) := (pu % 65536 / 256, pv % 65536 / 256)
      let c := if -1 ≤ ix ∧ ix < iw ∧ -1 ≤ iy ∧ iy < rows then
          let q := [tap ix iy, tap (ix + 1) iy, tap ix (iy + 1), tap (ix + 1) (iy + 1)]
          pack ((List.range 4).map fun k =>
            let ch (j : Nat) := channel (q.getD j 0) k
            lerp (lerp (ch 0) (ch 1) fx) (lerp (ch 2) (ch 3) fx) fy)
        else 0
      (xmin + i, y, c)
  else
    let (du, dv) := (r.div (r.sub u1 u0) n, r.div (r.sub v1 v0) n)
    ((xs.foldl (fun (acc : List (Int × Int × Nat) × Nat × Nat) (i : Int) =>
      let (out, u, v) := acc
      let (fu, fv) := (floorF u, floorF v)
      let fx := fix11 r (r.sub u fu)
      let fy := fix11 r (r.sub v fv)
      let (ix, iy) := (floorI fu, floorI fv)
      let c := if ix + 1 < 0 ∨ ix ≥ iw ∨ iy + 1 < 0 ∨ iy ≥ rows then 0 else
        let q := [tap ix iy, tap (ix + 1) iy, tap ix (iy + 1), tap (ix + 1) (iy + 1)]
        pack ((List.range 4).map fun k =>
          let ch (j : Nat) := channel (q.getD j 0) k
          let t := (ch 1 - ch 0) * fx + ch 0 * 2048
          let b := (ch 3 - ch 2) * fx + ch 2 * 2048
          (t * (2048 - fy) + b * fy + 0x200000) / 2 ^ 22)
      ((xmin + i, y, c) :: out, r.add u du, r.add v dv)) ([], u0, v0)).1).reverse

/-- the tent's running integral every 1/128 of a pixel, in 1/65536 -/
def tent (i : Int) : Int :=
  if i ≤ -128 then 0 else if i ≤ 0 then 2 * (128 + i) * (128 + i)
  else if i ≤ 128 then 65536 - 2 * (128 - i) * (128 - i) else 65536

/-- floor(a / b + 1/2) of a rational, b > 0 -/
def roundQ (a : Int) (b : Nat) : Int := (2 * a + b) / (2 * b : Nat)

/-- the entries the single precision table holds one off the exact integral (i >= 0) -/
def cubicOff : List (Int × Int) := [(26, 1), (29, 1), (44, 1), (56, 1), (76, 1), (85, 1), (86, 1), (88, 1), (89, 1), (95, 1), (96, 1), (106, 1), (108, 1), (118, 1), (124, 1), (134, 1), (135, 1), (139, 1), (147, 1), (150, 1), (153, 1), (213, -1)]

/-- Keys' cubic (a = -1) integrated from -inf to u = (i + 1/2) / 128 >= 0, times 65536, rounded, as
an exact rational: u = n / 256, the integral over 12 256^4 -/
def cubicExact (i : Int) : Int :=
  let n : Int := min (2 * i + 1) 512
  let d : Int := 256
  -- (12 d^4 times the integral past 1/2: u^4 / 4 - 2 u^3 / 3 + u under 1, else 7/12 - 17/12 +
  -- (-u^4 / 4 + 5 u^3 / 3 - 4 u^2 + 4 u))
  let num : Int :=
    if n < d then 3 * n ^ 4 - 8 * n ^ 3 * d + 12 * n * d ^ 3
    else -10 * d ^ 4 - 3 * n ^ 4 + 20 * n ^ 3 * d - 48 * n ^ 2 * d ^ 2 + 48 * n * d ^ 3
  -- (65536 (1/2 + num / (12 d^4)))
  roundQ (32768 * 12 * d ^ 4 + 65536 * num) (12 * 256 ^ 4)

/-- the cubic's running integral every 1/128 of a pixel at the half, in 1/65536 (gpstretch.rs
`cubic`) -/
def cubic (i : Int) : Int :=
  if i ≤ -256 then 0 else if i ≥ 256 then 65536
  else
    let pos (j : Int) := cubicExact j + ((cubicOff.find? (·.1 = j)).map (·.2)).getD 0
    if i < 0 then 65536 - pos (-1 - i) else pos i

/-- one times Keys' cubic (a = -1/2) at i / 64, rounded (gpimage.rs `keys`): exact rationals over
2 64^3 -/
def keys (one : Nat) (i : Nat) : Int :=
  let x : Int := i
  let num : Int :=
    if x < 64 then 3 * x ^ 3 - 5 * x ^ 2 * 64 + 2 * 64 ^ 3
    else if x < 128 then -x ^ 3 + 5 * x ^ 2 * 64 - 8 * x * 64 ^ 2 + 4 * 64 ^ 3
    else 0
  roundQ (one * num) (2 * 64 ^ 3)

/-- the kernel's running integral at a 16.16 place: 3 the tent, 4 the cubic -/
def kAt (k : Nat) (t : Int) : Int := if k = 3 then tent (t / 2 ^ 9) else cubic (t / 2 ^ 8)

/-- the source pixels one center reaches: (pixel, the running integral at its far edge) -/
def taps (k : Nat) (c half inv : Int) : List (Int × Int) :=
  let first := (c - half + 0xffff) / 2 ^ 16
  let last := (c + half + 0xffff) / 2 ^ 16
  let t := ((first * 2 ^ 16 - c) * inv) / 2 ^ 16
  (List.range (last - first + 1).toNat).map fun (m : Nat) => (first + m, kAt k (t + m * inv))

/-- each running integral less the one before -/
def weights (t : List (Int × Int)) (shift : Nat) : List (Int × Int) :=
  ((t.foldl (fun (acc : List (Int × Int) × Int) (j, v) =>
    let v := v / 2 ^ shift
    ((j, v - acc.2) :: acc.1, v)) ([], 0)).1).reverse

def clampI (v lo hi : Int) : Int := max lo (min hi v)
def packC (c : List Int) : Nat := (List.range 4).foldl (fun o k => o + (clampI (c.getD k 0) 0 255).toNat * 2 ^ (8 * k)) 0
def capped (c : List Int) : List Int := let a := c.getD 3 0; [min (c.getD 0 0) a, min (c.getD 1 0) a, min (c.getD 2 0) a, a]
def sums (line : List (Nat × Int)) : List Int := (List.range 4).map fun k => (line.map fun (p, w) => channel p k * w).sum
def mmx (s : List Int) : List Int := s.map fun v => clampI (clampI ((v + 0x2000) / 2 ^ 14) (-32768) 32767) 0 255

/-- floor(v + 1/2) as an i64, the add rounded -/
def fl (r : Ops) (v : Nat) : Int := floorI (r.add v half)

/-- GP.DRAW.HQ_BILINEAR (k 3), GP.DRAW.HQ_BICUBIC (k 4): DpOutputSpanStretch's pixels from the
chain's source and device rectangles, the source iw wide read through the locked rows -/
def stretch (r : Ops) (k : Nat) (c : Out) (iw : Int) (x86 : Bool) (read : Int → Int → Nat) : Option (List (Int × Int × Nat)) := do
  let g (l : List Nat) (i : Nat) := l.getD i 0
  let (ux, uy, sw, sh) := (g c.sf 0, g c.sf 1, g c.sf 2, g c.sf 3)
  let (ddx, ddy, ddw, ddh) := (g c.dr 0, g c.dr 1, g c.dr 2, g c.dr 3)
  let lh := c.rows
  let k65536 : Nat := 0x47800000
  let kInv : Nat := 0x37800000
  let xscale := fl r (r.mul (r.div sw ddw) k65536)
  let yscale := fl r (r.mul (r.div sh ddh) k65536)
  let (rx, ry) := (r.div ddw sw, r.div ddh sh)
  let edges (d0 s ratio : Nat) : Int × Int :=
    let (a, b) := (fl r (r.mul d0 k16), fl r (r.mul (r.add (r.mul s ratio) d0) k16))
    (min a b, max a b)
  let (left, right) := edges ddx sw rx
  let (tp, bt) := edges ddy sh ry
  let (l16, r16, t16, b16) := (left * 2 ^ 12, right * 2 ^ 12, tp * 2 ^ 12, bt * 2 ^ 12)
  let firstcol := (left + 15) / 16
  let sx0 := if xscale ≥ 0 then
      fl r (r.mul (r.add (r.mul (r.mul (r.sub ddx (ofI (l16 / 2 ^ 16))) (ofI xscale)) kInv) ux) k65536)
    else fl r (r.mul (r.add (r.mul (r.mul (r.sub (r.add ddx ddw) (ofI ((r16 + 0xffff) / 2 ^ 16))) (ofI xscale)) kInv) ux) k65536)
  let sy0 := if yscale ≥ 0 then
      fl r (r.mul (r.add (r.mul (r.mul (r.sub (ofI (t16 / 2 ^ 16)) ddy) (ofI yscale)) kInv) uy) k65536)
    else fl r (r.mul (r.sub uy (r.mul (r.mul (r.sub (ofI ((b16 + 0xffff) / 2 ^ 16)) (r.add ddh ddy)) (ofI yscale)) kInv)) k65536)
  let (kmul, kmin) : Nat × Int := if k = 3 then (one, 65536) else (0x40000000, 131072)
  let halfW (s d : Nat) : Int := max ((fl r (r.mul (r.div (r.mul kmul s) d) k65536)).natAbs : Int) kmin
  let (kx, ky) := (halfW sw ddw, halfW sh ddh)
  let inv (h : Int) := fl r (r.mul (r.div k65536 (ofI h)) k65536)
  let (kxi, kyi) := (inv kx, inv ky)
  let row0 := t16 / 2 ^ 16
  let width := (r16 + 0xffff) / 2 ^ 16 - l16 / 2 ^ 16 + 1
  let c0 := sx0 - 0x8000
  let ((mlo, mhi), mmx2) : (Int × Int) × Option (Int × Int) :=
    if x86 ∧ k = 4 ∧ xscale > 0 then
      let lo := max ((kx - c0 + xscale - 1).tdiv xscale) 0
      let hi := min (((iw - 1) * 2 ^ 16 - c0 - kx).tdiv xscale) width
      let n := hi - lo
      let pos := c0 + lo * xscale
      let first := (pos - kx + 0xffff) / 2 ^ 16
      let last := ((n - 1) * xscale + pos + kx) / 2 ^ 16
      let two := n ≥ 16 ∧ (last = first + 2 * n + 5 ∨ last = first + 2 * n + 6)
      ((lo, hi), if two then some (first, (pos + 0x8000) % 65536) else none)
    else if x86 ∧ k = 4 ∧ xscale < 0 then
      let lo := (kx - c0).tdiv xscale
      let hi := (((iw - 1) * 2 ^ 16 - c0 - kx) + xscale + 1).tdiv xscale
      if -2 ^ 31 ≤ lo ∧ lo < 2 ^ 31 ∧ -2 ^ 31 ≤ hi ∧ hi < 2 ^ 31 then ((max hi 0, min lo width), none) else ((0, 0), none)
    else ((0, 0), none)
  let inMmx (i : Int) := mlo ≤ i ∧ i < mhi
  let w8 : List Int := match mmx2 with
    | none => []
    | some (_, frac) => (List.range 8).map fun (m : Nat) => keys 65536 ((((frac / 4) - 0x10000) + m * 0x4000) / 2 ^ 9).natAbs / 8
  -- (a cached line's pixel i by the kernel's taps)
  let stretchPx (i : Int) (px : Int → Nat) : Nat :=
    let t := taps k (c0 + i * xscale) kx kxi
    let s := sums ((weights t (if inMmx i then 2 else 0)).map fun (j, w) => (px j, w))
    if k = 3 then packC (s.map fun v => min ((v + 0x8000) / 2 ^ 16) 255)
    else if inMmx i then packC (capped (mmx s))
    else
      let v := s.map fun v => (v + 0x8000) / 2 ^ 16
      let a := min (v.getD 3 0) 255
      packC ((capped [v.getD 0 0, v.getD 1 0, v.getD 2 0, a]).map fun c => max c 0)
  -- (the horizontal pass, one cached line a locked row)
  let lines : Array (Array Nat) := ((List.range lh.toNat).map fun (j : Nat) =>
    let px (x : Int) : Nat := if 0 ≤ x ∧ x < iw then read x j else 0
    ((List.range width.toNat).map fun (iN : Nat) =>
      let i : Int := iN
      match mmx2 with
      | some (first, _) =>
        if inMmx i then
          let at_ := first + 2 * (i - mlo)
          packC (capped (mmx (sums ((List.range 8).map fun (m : Nat) => (px (at_ + m), w8.getD m 0)))))
        else stretchPx i px
      | none => stretchPx i px).toArray).toArray
  -- (the vertical pass)
  let ys := (List.range (c.lines.2 - c.lines.1).toNat).map fun (j : Nat) => c.lines.1 + j
  let xs := (List.range (c.cols.2 - c.cols.1).toNat).map fun (j : Nat) => c.cols.1 + j
  let rowsOut ← ys.mapM fun y =>
    let cc := sy0 - 0x8000 + (y - row0) * yscale
    let ws := (weights (taps k cc ky kyi) 0).filter fun (j, _) => 0 ≤ j ∧ j < lh
    xs.mapM fun x => do
      let i := x - firstcol
      if i < 0 ∨ i ≥ width then none else
      let s := sums (ws.map fun (j, w) => ((lines.getD j.toNat #[]).getD i.toNat 0, if x86 then w / 4 else w))
      let p := if x86 then packC (capped (mmx s))
        else if k = 3 then packC (s.map fun v => min ((v + 0x8000) / 2 ^ 16) 255)
        else packC (capped (s.map fun v => clampI ((v + 0x8000) / 2 ^ 16) 0 255))
      pure (x, y, p)
  pure rowsOut.flatten

/-- GP.DRAW.NEAREST, GP.DRAW.BILINEAR, GP.DRAW.HQ_BILINEAR, GP.DRAW.HQ_BICUBIC (`render`): the pixels a draw writes, (x, y, premultiplied
color), the image iw x ih row by row; none where the model fails -/
def render (d : Bool) (span : Nat) (x86 : Bool) (dst src : List Int) (iw ih bw bh : Int) (half_ : Bool) (img : List Nat) : Option (List (Int × Int × Nat)) := do
  let r := ops d
  let some c ← chain d span dst src iw ih bw bh half_ | pure []
  let texel (x y : Int) : Nat := if 0 ≤ x ∧ x < iw ∧ 0 ≤ y ∧ y < ih then img.getD (y * iw + x).toNat 0 else 0
  let read (x y : Int) : Nat :=
    let y := c.top + y
    if c.flip = 0 then texel x y else if c.flip = 1 then texel (iw - 1 - x) y
    else if c.flip = 2 then texel x (ih - 1 - y) else texel (iw - 1 - x) (ih - 1 - y)
  let ys := (List.range (c.lines.2 - c.lines.1).toNat).map fun (j : Nat) => c.lines.1 + j
  match c.copy with
  | some (ox, oy) =>
    let xs := (List.range (c.cols.2 - c.cols.1).toNat).map fun (j : Nat) => c.cols.1 + j
    pure (ys.flatMap fun y => xs.map fun x =>
      let (sx, sy) := (x + ox, y + oy)
      (x, y, if 0 ≤ sx ∧ sx < iw ∧ 0 ≤ sy ∧ sy < c.rows then read sx sy else 0))
  | none =>
    if span ≥ 3 then stretch r span c iw x86 read else
    let inv ← inverse r c.ms2
    if c.cols.1 ≥ c.cols.2 then pure [] else
    let tap (x y : Int) : Nat := if 0 ≤ x ∧ x < iw ∧ 0 ≤ y ∧ y < c.rows then read x y else 0
    pure (ys.flatMap fun y => row r span x86 inv c.cols.1 c.cols.2 y c.rows iw tap)

/-- GP.DRAW.BILINEAR: a 2 x 1 image of black and white stretched to 4 pixels: x86-64's fractions
in 1/2048 and x86's MMX in 1/256 give the same ramp here -/
theorem ramp :
    (render true 1 false [0, 0, 4, 1] [0, 0, 2, 1] 2 1 4 1 true [0xff000000, 0xffffffff]).map (·.map (·.2.2)) =
      (render false 1 true [0, 0, 4, 1] [0, 0, 2, 1] 2 1 4 1 true [0xff000000, 0xffffffff]).map (·.map (·.2.2)) := by
  decide +kernel

/-- GP.DRAW.HQ_BILINEAR, GP.DRAW.HQ_BICUBIC: the tent's integral is half way at 0, the cubic's
two middle entries sum to the whole, past 2 pixels the whole; Keys' a = -1/2 is one at 0 and
none at a pixel; a 2 x 2 checker stretched to 4 x 4 by the tent is mid grey at (1, 1) -/
theorem tables :
    tent 0 = 32768 ∧ cubic 0 + cubic (-1) = 65536 ∧ cubic 255 = 65536 ∧ keys 16384 0 = 16384 ∧ keys 65536 64 = 0 ∧
    ((render true 3 false [0, 0, 4, 4] [0, 0, 2, 2] 2 2 4 4 false [0xff000000, 0xffffffff, 0xffffffff, 0xff000000]).getD []).getD 5 (0, 0, 0) =
      (1, 1, 0xff808080) := by decide +kernel

def mirrors : List Mirror := [
  ⟨"gp.chain", fun
    | [dd, span, dx, dy, dw, dh, sx, sy, sw, sh, iw, ih, bw, bh, hf] => do
      pure (answer (chain (dd != 0) (← nat span) [dx, dy, dw, dh] [sx, sy, sw, sh] iw ih bw bh (hf != 0)))
    | _ => none⟩,
  ⟨"gp.cubic", fun | [i] => some [cubic i] | _ => none⟩,
  ⟨"gp.keys", fun | [o, i] => do pure [keys (← nat o) (← nat i)] | _ => none⟩,
  ⟨"gp.render", fun
    | dd :: x86 :: span :: dx :: dy :: dw :: dh :: sx :: sy :: sw :: sh :: iw :: ih :: bw :: bh :: hf :: img => do
      let img ← img.mapM nat
      match render (dd != 0) (← nat span) (x86 != 0) [dx, dy, dw, dh] [sx, sy, sw, sh] iw ih bw bh (hf != 0) img with
      | none => pure [-1]
      | some px => pure (px.flatMap fun (x, y, c) => [x, y, (c : Int)])
    | _ => none⟩
]

end Kikizake.GdiPlus.Chain
