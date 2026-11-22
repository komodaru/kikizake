import Kikizake.Mirror
import Kikizake.F32
/-! D3DX's DXT encoder (spec/d3dx9.txt D3DX9.LOAD.DXTENC) as src/model/dxtenc.rs `encode_bc1`,
`encode_alpha4`, `encode_alpha8` compute it (mirrors `x9.bc1`, `x9.alpha4`, `x9.alpha8`), on
both builds: x64 in binary32 rounded to nearest; x86 on the x87 at single precision, every
operation's exact result rounded to binary32 (Kikizake.Float), toward zero inside the loops
where the encoder sets the rounding control (`Ar.on`), to nearest elsewhere. Values are binary32
bit patterns. -/
namespace Kikizake.D3dx9.DxtEnc

open Kikizake.Float (isNaN isInf sign toRat)

def fm : Float.Fmt := F32.fmt

/-- the binary32 of sign s nearest a / b toward zero (b > 0); past the largest finite value the
largest finite value -/
def roundZ (s : Bool) (a b : Nat) : Nat :=
  let F := fm
  let sb := if s then Float.sbit F else 0
  if a = 0 then sb else
  let d : Int := (Nat.log2 a : Int) - (Nat.log2 b : Int)
  let L : Int := if a * (Float.pow2 d).2 < b * (Float.pow2 d).1 then d - 1 else d
  let Q : Int := Max.max (L - F.m) (Float.emin F)
  let (num, den) := if 0 ≤ Q then (a, b * 2 ^ Q.toNat) else (a * 2 ^ (-Q).toNat, b)
  let m := num / den
  if Q + F.m > (Float.bias F : Int) then sb + Float.inf F - 1
  else if m < 2 ^ F.m then sb + m
  else sb + (Q + Float.bias F + F.m).toNat * 2 ^ F.m + (m - 2 ^ F.m)

/-- the rounding: toward zero (chop) or to nearest, halves to even -/
def rnd (chop : Bool) (s : Bool) (a b : Nat) : Nat := if chop then roundZ s a b else Float.round fm s a b

def addR (chop : Bool) (x y : Nat) : Nat :=
  if isNaN fm x || isNaN fm y then Float.qnan fm
  else if isInf fm x && isInf fm y then (if sign fm x == sign fm y then (if sign fm x then Float.sbit fm else 0) + Float.inf fm else Float.qnan fm)
  else if isInf fm x then x
  else if isInf fm y then y
  else
    let (s, a, b) := Float.addExact fm x y
    if a = 0 then (if sign fm x && sign fm y then Float.sbit fm else 0) else rnd chop s a b

def mulR (chop : Bool) (x y : Nat) : Nat :=
  let s := sign fm x != sign fm y
  let sb := if s then Float.sbit fm else 0
  if isNaN fm x || isNaN fm y then Float.qnan fm
  else if isInf fm x || isInf fm y then (if Float.sig fm x = 0 && !isInf fm x || Float.sig fm y = 0 && !isInf fm y then Float.qnan fm else sb + Float.inf fm)
  else
    let (pn, pd) := Float.pow2 (Float.ex fm x + Float.ex fm y)
    rnd chop s (Float.sig fm x * Float.sig fm y * pn) pd

def divR (chop : Bool) (x y : Nat) : Nat :=
  let s := sign fm x != sign fm y
  let sb := if s then Float.sbit fm else 0
  if isNaN fm x || isNaN fm y then Float.qnan fm
  else if isInf fm x then (if isInf fm y then Float.qnan fm else sb + Float.inf fm)
  else if isInf fm y then sb
  else if Float.sig fm y = 0 then (if Float.sig fm x = 0 then Float.qnan fm else sb + Float.inf fm)
  else
    let (pn, pd) := Float.pow2 (Float.ex fm x - Float.ex fm y)
    rnd chop s (Float.sig fm x * pn) (Float.sig fm y * pd)

/-- to nearest the rounding is Kikizake.Float's binary32 one -/
theorem nearest_add (x y : Nat) : addR false x y = F32.add x y := by
  simp only [addR, rnd, F32.add, Float.add, fm, Bool.false_eq_true, ite_false]
  rfl
theorem nearest_mul (x y : Nat) : mulR false x y = F32.mul x y := by
  simp only [mulR, rnd, F32.mul, Float.mul, fm, Bool.false_eq_true, ite_false]
  rfl
theorem nearest_div (x y : Nat) : divR false x y = F32.div x y := by
  simp only [divR, rnd, F32.div, Float.div, fm, Bool.false_eq_true, ite_false]
  rfl

/-- one build's arithmetic: x86 or x64, and whether the rounding control is toward zero (only
the x87 has one) -/
structure Ar where
  x86 : Bool
  chop : Bool

namespace Ar
def c (a : Ar) : Bool := a.x86 && a.chop
/-- the rounding control set toward zero from here on -/
def on (a : Ar) : Ar := { a with chop := true }
def add (a : Ar) := addR a.c
def sub (a : Ar) (x y : Nat) := addR a.c x (Float.neg fm y)
def mul (a : Ar) := mulR a.c
def div (a : Ar) := divR a.c
/-- toward zero to an integer, as C's (int) cast: x64's cvttss2si saturating by Rust's `as`
(a NaN 0), the x87's fistp the integer indefinite -2^31 out of range -/
def trunc (a : Ar) (x : Nat) : Int :=
  if isNaN fm x then (if a.x86 then -2147483648 else 0)
  else if isInf fm x then (if a.x86 || sign fm x then -2147483648 else 2147483647)
  else
    let (s, n, d) := toRat fm x
    let v : Int := if s then -((n / d : Nat) : Int) else ((n / d : Nat) : Int)
    if a.x86 then (if v ≥ 2147483648 ∨ v < -2147483648 then -2147483648 else v)
    else Max.max (-2147483648) (Min.min v 2147483647)
end Ar

/-- the binary32 order, false on a NaN -/
def lt (x y : Nat) : Bool := !isNaN fm x && !isNaN fm y && Float.lt fm x y
def gt (x y : Nat) : Bool := lt y x
def le (x y : Nat) : Bool := !isNaN fm x && !isNaN fm y && !Float.lt fm y x
def ge (x y : Nat) : Bool := le y x
def eq (x y : Nat) : Bool := le x y && le y x

def zero : Nat := 0
def one : Nat := 0x3f800000
def half : Nat := 0x3f000000
def of (n : Nat) : Nat := F32.ofNat n
/-- the binary32 literals 0.2125, 0.7154, 0.0721, 0.2 -/
def c2125 : Nat := 0x3e59999a
def c7154 : Nat := 0x3f372474
def c0721 : Nat := 0x3d93a92a
def c02 : Nat := 0x3e4ccccd
/-- 1/4096, 1/256, 1/64, 1/8, f32::MIN_POSITIVE -/
def r4096 : Nat := 0x39800000
def r256 : Nat := 0x3b800000
def r64 : Nat := 0x3c800000
def r8 : Nat := 0x3e000000
def minPos : Nat := 0x800000

/-- the luminance weights (0.2125/0.7154, 1, 0.0721/0.7154) and their inverses -/
def lum : Array Nat := #[F32.div c2125 c7154, one, F32.div c0721 c7154]
def lumInv : Array Nat := #[F32.div c7154 c2125, one, F32.div c7154 c0721]

def clamp01 (v : Nat) : Nat := if lt v zero then zero else if gt v one then one else v

/-- an int to u16, or 0 outside it (Rust `u16::try_from(..).unwrap_or(0)`) -/
def u16of (t : Int) : Nat := if 0 ≤ t ∧ t ≤ 65535 then t.toNat else 0
/-- an int clamped to 0..255 -/
def byteOf (t : Int) : Nat := (Max.max 0 (Min.min t 255)).toNat
/-- an int to an index, 0 below 0 -/
def idx (t : Int) : Nat := if t < 0 then 0 else t.toNat

/-- a colour to 565, toward zero: each channel clamped, times 31 or 63 plus 1/2 truncated -/
def encode565 (a : Ar) (c : Array Nat) : Nat :=
  let ch := a.on
  let q := fun (v m : Nat) => u16of (ch.trunc (ch.add (ch.mul (clamp01 v) (of m)) half))
  ((q c[0]! 31 <<< 11) % 65536) ||| ((q c[1]! 63 <<< 5) % 65536) ||| q c[2]! 31

def decode565 (a : Ar) (w : Nat) : Array Nat :=
  let u := fun (v m : Nat) => a.mul (of v) (F32.div one (of m))
  #[u ((w >>> 11) % 32) 31, u ((w >>> 5) % 64) 63, u (w % 32) 31]

/-- a point's step in a Newton round: n - 1 where the dot is not below the steps' count, else
trunc(dot + 0.5); below 0 the build's fault (x64 -1, x86 -2) -/
def stepIndex (a : Ar) (dot fsteps n : Nat) : Except Int Nat :=
  if !gt fsteps dot then .ok (n - 1)
  else
    let t := a.trunc (a.add dot half)
    if t < 0 then .error (if a.x86 then -2 else -1) else .ok t.toNat

def sum3 (a : Ar) (u v : Array Nat) : Nat :=
  a.add (a.add (a.mul u[0]! v[0]!) (a.mul u[1]! v[1]!)) (a.mul u[2]! v[2]!)

/-- the endpoints of the block's points: the box, its diagonal's sign pattern by the points'
spread, then 8 Newton rounds toward zero -/
def optimizeRgb (a : Ar) (pts : List (Array Nat)) (n : Nat) : Except Int (Array Nat × Array Nat) := Id.run do
  let q := F32.div 0x3e800000 (of 64)
  let eps := F32.mul q q
  let (pc, pd) : Array Nat × Array Nat :=
    if n = 3 then (#[one, half, zero], #[zero, half, one])
    else (#[one, F32.div (of 2) (of 3), F32.div one (of 3), zero], #[zero, F32.div one (of 3), F32.div (of 2) (of 3), one])
  let mut x := lum
  let mut y := #[zero, zero, zero]
  for p in pts do
    for k in [0, 1, 2] do
      if lt p[k]! x[k]! then x := x.set! k p[k]!
      if gt p[k]! y[k]! then y := y.set! k p[k]!
  let ab : Array Nat := #[a.sub y[0]! x[0]!, a.sub y[1]! x[1]!, a.sub y[2]! x[2]!]
  let fab := sum3 a ab ab
  if lt fab minPos then return .ok (x, y)
  let inv := a.div one fab
  let dir := ab.map (a.mul · inv)
  let mid : Array Nat := #[0, 1, 2].map fun k => a.mul (a.add x[k]! y[k]!) half
  let mut fdir : Array Nat := #[zero, zero, zero, zero]
  for p in pts do
    let pt : Array Nat := #[0, 1, 2].map fun k => a.mul (a.sub p[k]! mid[k]!) dir[k]!
    let fs : Array Nat := #[a.add (a.add pt[0]! pt[1]!) pt[2]!, a.sub (a.add pt[0]! pt[1]!) pt[2]!,
      a.add (a.sub pt[0]! pt[1]!) pt[2]!, a.sub (a.sub pt[0]! pt[1]!) pt[2]!]
    for i in [0, 1, 2, 3] do
      fdir := fdir.set! i (a.add fdir[i]! (a.mul fs[i]! fs[i]!))
  let mut best := 0
  for i in [1, 2, 3] do
    if gt fdir[i]! fdir[best]! then best := i
  if best &&& 2 != 0 then
    let t := x[1]!
    x := x.set! 1 y[1]!
    y := y.set! 1 t
  if best &&& 1 != 0 then
    let t := x[2]!
    x := x.set! 2 y[2]!
    y := y.set! 2 t
  if lt fab r4096 then return .ok (x, y)
  let fsteps := of (n - 1)
  let c := a.on
  for _ in [0:8] do
    let st : Array (Array Nat) := (Array.range n).map fun i => #[0, 1, 2].map fun k => c.add (c.mul pd[i]! y[k]!) (c.mul x[k]! pc[i]!)
    let d0 : Array Nat := #[0, 1, 2].map fun k => c.sub y[k]! x[k]!
    let len := sum3 c d0 d0
    if lt len r4096 then break
    let scale := c.div fsteps len
    let d := d0.map (c.mul · scale)
    let mut d2x := zero
    let mut d2y := zero
    let mut dx : Array Nat := #[zero, zero, zero]
    let mut dy : Array Nat := #[zero, zero, zero]
    for p in pts do
      let dot := c.add (c.add (c.mul (c.sub p[1]! x[1]!) d[1]!) (c.mul (c.sub p[0]! x[0]!) d[0]!)) (c.mul (c.sub p[2]! x[2]!) d[2]!)
      match stepIndex c dot fsteps n with
      | .error b => return .error b
      | .ok i =>
        let diff : Array Nat := #[0, 1, 2].map fun k => c.sub st[i]![k]! p[k]!
        let fc := c.mul pc[i]! r8
        let fd := c.mul r8 pd[i]!
        d2x := c.add (c.mul pc[i]! fc) d2x
        dx := #[0, 1, 2].map fun k => c.add (c.mul fc diff[k]!) dx[k]!
        d2y := c.add (c.mul fd pd[i]!) d2y
        dy := #[0, 1, 2].map fun k => c.add (c.mul fd diff[k]!) dy[k]!
    if gt d2x zero then
      let f := Float.neg fm (c.div one d2x)
      x := #[0, 1, 2].map fun k => c.add (c.mul f dx[k]!) x[k]!
    if gt d2y zero then
      let f := Float.neg fm (c.div one d2y)
      y := #[0, 1, 2].map fun k => c.add (c.mul f dy[k]!) y[k]!
    if [0, 1, 2].all fun k => lt (c.mul dx[k]! dx[k]!) eps && lt (c.mul dy[k]! dy[k]!) eps then break
  return .ok (x, y)

/-- the 8 bytes of a colour block -/
def block (c0 c1 bits : Nat) : List Nat :=
  [c0 % 256, c0 / 256, c1 % 256, c1 / 256, bits % 256, bits / 256 % 256, bits / 65536 % 256, bits / 16777216 % 256]

/-- the block's points: each channel quantized to 5, 6, 5 bits toward zero (times 31 or 63
plus 1/2 truncated, clamped to 0..255, back times 1/31 or 1/63), luminance-weighted -/
def points (a : Ar) (px : List (Array Nat)) : List (Array Nat) :=
  let ch := a.on
  let q := fun (v m : Nat) => ch.mul (of (byteOf (ch.trunc (ch.add (ch.mul v (of m)) half)))) (F32.div one (of m))
  px.map fun c => #[ch.mul (q c[0]! 31) lum[0]!, ch.mul (q c[1]! 63) lum[1]!, ch.mul (q c[2]! 31) lum[2]!]

/-- a texel's index toward zero: 3 where three colours and its alpha below 1/2, else its
projection on the endpoints' line: 0 at or before the first, 1 at or past the last, else the
nearest step in the block's order -/
def texelIndex (a : Ar) (three : Bool) (s0 dir : Array Nat) (fsteps : Nat) (order : List Nat) (c : Array Nat) : Nat :=
  let ch := a.on
  if three && lt c[3]! half then 3 else
    let p : Array Nat := #[0, 1, 2].map fun k => ch.mul c[k]! lum[k]!
    let dot := ch.add (ch.add (ch.mul (ch.sub p[1]! s0[1]!) dir[1]!) (ch.mul (ch.sub p[0]! s0[0]!) dir[0]!)) (ch.mul (ch.sub p[2]! s0[2]!) dir[2]!)
    if le dot zero then 0 else if ge dot fsteps then 1 else order.getD (idx (ch.trunc (ch.add dot half))) 0

/-- the block from its endpoints: each to 565 (unweighted), four colours of one 565 colour
every index 0; else the endpoints ordered (four colours the greater first, three the lesser),
the indices by texelIndex -/
def finish (a : Ar) (px : List (Array Nat)) (three : Bool) (ea eb : Array Nat) : List Nat :=
  let n := if three then 3 else 4
  let wa := encode565 a (#[0, 1, 2].map fun k => a.mul ea[k]! lumInv[k]!)
  let wb := encode565 a (#[0, 1, 2].map fun k => a.mul eb[k]! lumInv[k]!)
  if !three && wa == wb then block wa wb 0 else
  let ca := #[0, 1, 2].map fun k => a.mul (decode565 a wa)[k]! lum[k]!
  let cb := #[0, 1, 2].map fun k => a.mul (decode565 a wb)[k]! lum[k]!
  let ((c0, s0), (c1, s1)) := if three == decide (wa ≤ wb) then ((wa, ca), (wb, cb)) else ((wb, cb), (wa, ca))
  let order : List Nat := if three then [0, 2, 1] else [0, 2, 3, 1]
  let fsteps := of (n - 1)
  let dir0 : Array Nat := #[0, 1, 2].map fun k => a.sub s1[k]! s0[k]!
  let scale := if wa != wb then a.div fsteps (sum3 a dir0 dir0) else zero
  let dir := dir0.map (a.mul · scale)
  let dw := px.foldl (fun dw c => ((texelIndex a three s0 dir fsteps order c <<< 30) ||| (dw >>> 2)) % 2 ^ 32) 0
  block c0 c1 dw

/-- a DXT1 colour block of 16 (r, g, b, a), four colours or (three) three and transparent black
where the alpha is below 1/2 -/
def encodeBc1 (a : Ar) (px : List (Array Nat)) (three : Bool) : Except Int (List Nat) := do
  let (ea, eb) ← optimizeRgb a (points a px) (if three then 3 else 4)
  return finish a px three ea eb

/-- DXT2 and DXT3's alpha: each texel's alpha clamped, a 15 + 1/2 truncated toward zero, 4 bits -/
def encodeAlpha4 (a : Ar) (px : List (Array Nat)) : List Nat :=
  let ch := a.on
  let w := (px.zip (List.range 16)).foldl (fun w (c, i) =>
    let t := ch.trunc (ch.add (ch.mul (clamp01 c[3]!) (of 15)) half)
    w ||| ((if t < 0 then 0 else t.toNat % 16) <<< (4 * i))) 0
  (List.range 8).map fun k => w / 2 ^ (8 * k) % 256

/-- the alpha endpoints: Newton's method on six steps (and 0 and 1) or eight -/
def optimizeAlpha (a : Ar) (pts : List Nat) (six : Bool) : Nat × Nat := Id.run do
  let n := if six then 6 else 8
  let fs := fun (i : Nat) => F32.div (of i) (of (n - 1))
  let pc := fun (i : Nat) => F32.div (of (n - 1 - i)) (of (n - 1))
  let mut x := one
  let mut y := zero
  if !six then
    for p in pts do
      if lt p x then x := p
      if gt p y then y := p
  else
    for p in pts do
      if lt p x && gt p zero then x := p
      if gt p y && lt p one then y := p
    if eq x y then y := one
  let fsteps := of (n - 1)
  let c := a.on
  for _ in [0:8] do
    if lt (c.sub y x) r256 then break
    let scale := c.div fsteps (c.sub y x)
    let st : Array Nat := (Array.range n).map (fun i => c.add (c.mul (fs i) y) (c.mul (pc i) x)) ++ (if six then #[zero, one] else #[])
    let mut dx := zero
    let mut dy := zero
    let mut d2x := zero
    let mut d2y := zero
    for p in pts do
      let dot := c.mul (c.sub p x) scale
      let i := if le dot zero then (if six && le p (c.mul x half) then 6 else 0)
        else if ge dot fsteps then (if six && ge p (c.mul (c.add y one) half) then 7 else n - 1)
        else idx (c.trunc (c.add dot half))
      if i < n then
        let diff := c.sub p st[i]!
        dx := c.add (c.mul diff (pc i)) dx
        d2x := c.add (c.mul (pc i) (pc i)) d2x
        dy := c.add (c.mul diff (fs i)) dy
        d2y := c.add (c.mul (fs i) (fs i)) d2y
    if gt d2x zero then x := c.sub x (c.div dx d2x)
    if gt d2y zero then y := c.sub y (c.div dy d2y)
    if gt x y then
      let t := x
      x := y
      y := t
    if lt (c.mul dx dx) r64 && lt (c.mul dy dy) r64 then break
  return (clamp01 x, clamp01 y)

/-- the alphas to 1/255 toward zero: times 255 plus 1/2 truncated, clamped to 0..255, times
1/255 -/
def alphas (a : Ar) (px : List (Array Nat)) : List Nat :=
  let ch := a.on
  px.map fun c => ch.mul (of (byteOf (ch.trunc (ch.add (ch.mul c[3]! (of 255)) half)))) (F32.div one (of 255))

/-- the least and the greatest of the alphas, from the first: a value below the least is the
least, else one above the greatest the greatest -/
def lowHigh (al : List Nat) : Nat × Nat :=
  al.foldl (fun (lo, hi) v => if lt v lo then (v, hi) else if gt v hi then (lo, v) else (lo, hi)) (al.headD zero, al.headD zero)

/-- DXT4 and DXT5's alpha: the alphas to 1/255 toward zero, endpoints by optimizeAlpha (six
steps where 0 or 1 is among them), 3 bit indices by projection -/
def encodeAlpha8 (a : Ar) (px : List (Array Nat)) : List Nat := Id.run do
  let ch := a.on
  let al := alphas a px
  let (lo, hi) := lowHigh al
  if eq lo one then return [255, 255, 0, 0, 0, 0, 0, 0]
  let six := eq lo zero || eq hi one
  let (fa, fb) := optimizeAlpha a al six
  let byte := fun (v : Nat) => byteOf (ch.trunc (ch.add (ch.mul v (of 255)) half))
  let (ba, bb) := (byte fa, byte fb)
  let (fa, fb) := (a.mul (of ba) (F32.div one (of 255)), a.mul (of bb) (F32.div one (of 255)))
  if !six && ba == bb then return [ba, bb, 0, 0, 0, 0, 0, 0]
  let (o0, o1, s0, s1) := if six then (ba, bb, fa, fb) else (bb, ba, fb, fa)
  let st : Array Nat :=
    if six then #[s0, s1] ++ (#[1, 2, 3, 4].map fun i => a.mul (a.add (a.mul (of (5 - i)) s0) (a.mul (of i) s1)) c02) ++ #[zero, one]
    else #[s0, s1] ++ (#[1, 2, 3, 4, 5, 6].map fun i => a.mul (a.add (a.mul (of (7 - i)) s0) (a.mul (of i) s1)) (F32.div one (of 7)))
  let order : List Nat := if six then [0, 2, 3, 4, 5, 1] else [0, 2, 3, 4, 5, 6, 7, 1]
  let n := if six then 6 else 8
  let fsteps := of (n - 1)
  let scale := if !eq st[0]! st[1]! then a.div fsteps (a.sub st[1]! st[0]!) else zero
  let bits := (px.zip (List.range 16)).foldl (fun bits (c, i) =>
    let p := c[3]!
    let dot := ch.mul (ch.sub p st[0]!) scale
    let k := if le dot zero then (if six && le p (ch.mul st[0]! half) then 6 else 0)
      else if ge dot fsteps then (if six && ge p (ch.mul (ch.add st[1]! one) half) then 7 else 1)
      else order.getD (idx (ch.trunc (ch.add dot half))) 0
    bits ||| (k <<< (3 * i))) 0
  return [o0, o1] ++ (List.range 6).map fun k => bits / 2 ^ (8 * k) % 256

/-- D3DX9.LOAD.DXTENC: x64 has no rounding control, its arithmetic binary32 to nearest inside
the loops too -/
theorem x64_nearest (chop : Bool) : (Ar.mk false chop).c = false := rfl

/-- D3DX9.LOAD.DXTENC: x86's loops round toward zero, its other steps to nearest -/
theorem x86_loops_chop : (Ar.on ⟨true, false⟩).c = true ∧ (Ar.mk true false).c = false := ⟨rfl, rfl⟩

/-- D3DX9.LOAD.DXTENC: toward zero never rounds up: the result's magnitude is the exact one's
truncated (for a positive a / b, roundZ's significand is floor) -/
theorem roundZ_zero (s : Bool) (b : Nat) : roundZ s 0 b = (if s then Float.sbit fm else 0) := by
  simp [roundZ]

/-- D3DX9.LOAD.DXTENC: DXT3's alpha: each texel's 4 bits are the alpha clamped to 0..1, times
15 plus 1/2 truncated, in the texel's place (texel i at bits 4 i) -/
theorem alpha4_texel (a : Ar) (px : List (Array Nat)) :
    encodeAlpha4 a px = (List.range 8).map (fun k => ((px.zip (List.range 16)).foldl (fun w (c, i) =>
      w ||| ((if a.on.trunc (a.on.add (a.on.mul (clamp01 c[3]!) (of 15)) half) < 0 then 0
        else (a.on.trunc (a.on.add (a.on.mul (clamp01 c[3]!) (of 15)) half)).toNat % 16) <<< (4 * i))) 0) / 2 ^ (8 * k) % 256) := rfl

/-- D3DX9.LOAD.DXTENC: DXT1's endpoints are Newton's method on the quantized,
luminance-weighted points (four colours or three), the block then finished from them (a fault
of the Newton round the block's answer) -/
theorem bc1_newton (a : Ar) (px : List (Array Nat)) (three : Bool) :
    encodeBc1 a px three = (optimizeRgb a (points a px) (if three then 3 else 4)).map (fun e => finish a px three e.1 e.2) := by
  unfold encodeBc1
  cases optimizeRgb a (points a px) (if three then 3 else 4) <;> rfl

/-- D3DX9.LOAD.DXTENC: DXT1 three colours: a texel whose alpha is below 1/2 is index 3 -/
theorem bc1_transparent (a : Ar) (s0 dir : Array Nat) (fsteps : Nat) (order : List Nat) (c : Array Nat) (h : lt c[3]! half = true) :
    texelIndex a true s0 dir fsteps order c = 3 := by
  simp [texelIndex, h]

/-- D3DX9.LOAD.DXTENC: DXT5 of a block whose least alpha (to 1/255) is 1: the endpoints 255,
255, every index 0 -/
theorem alpha8_opaque (a : Ar) (px : List (Array Nat)) (h : eq (lowHigh (alphas a px)).1 one = true) :
    encodeAlpha8 a px = [255, 255, 0, 0, 0, 0, 0, 0] := by
  simp [encodeAlpha8, h]

def px (xs : List Int) : Option (List (Array Nat)) := do
  let v ← xs.mapM nat
  if v.length = 64 then some ((List.range 16).map fun i => #[v[4 * i]!, v[4 * i + 1]!, v[4 * i + 2]!, v[4 * i + 3]!]) else none

def ar (b : Int) : Option Ar := if b = 0 then some ⟨false, false⟩ else if b = 1 then some ⟨true, false⟩ else none

def mirrors : List Mirror := [
  ⟨"x9.bc1", fun
    | b :: t :: xs => do
      let a ← ar b
      let p ← px xs
      if t > 1 then none
      pure (match encodeBc1 a p (t = 1) with
        | .ok v => v.map Int.ofNat
        | .error e => [e])
    | _ => none⟩,
  ⟨"x9.alpha4", fun | b :: xs => do pure ((encodeAlpha4 (← ar b) (← px xs)).map Int.ofNat) | _ => none⟩,
  ⟨"x9.alpha8", fun | b :: xs => do pure ((encodeAlpha8 (← ar b) (← px xs)).map Int.ofNat) | _ => none⟩
]

end Kikizake.D3dx9.DxtEnc
