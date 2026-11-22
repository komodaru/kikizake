---------------------------- MODULE GdiWorld ----------------------------
(* A device context's graphics mode and world transform, from spec/gdi.txt (GDI.XFORM clauses
   below): SetGraphicsMode, SetWorldTransform, ModifyWorldTransform, GetWorldTransform,
   CombineTransform. The transforms here have whole, non-negative entries, so every product is
   exact in single precision and the answers (each entry's float bits) follow from the XFORM
   page's formulas. Every behavior takes at most MaxOps calls on a fresh DC; after each, the
   world transform is read back; `kikizake tla` replays every state's log through the Rust
   model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES gmode, world, log, n
vars == <<gmode, world, log, n>>

S(i) == ToString(i)
HD == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
RECURSIVE Hex8(_, _)
\* v in k hex digits
Hex8(v, k) == IF k = 0 THEN "" ELSE Hex8(v \div 16, k - 1) \o HD[(v % 16) + 1]
RECURSIVE Log2(_)
Log2(v) == IF v < 2 THEN 0 ELSE 1 + Log2(v \div 2)
\* the single-precision bits of a whole number 0 <= v < 2^24
F32(v) == IF v = 0 THEN "00000000"
          ELSE LET e == Log2(v) IN Hex8((127 + e) * 2 ^ 23 + (v - 2 ^ e) * 2 ^ (23 - e), 8)
Show(x) == "xform " \o F32(x[1]) \o " " \o F32(x[2]) \o " " \o F32(x[3]) \o " " \o F32(x[4]) \o " " \o F32(x[5]) \o " " \o F32(x[6])
Args(x) == S(x[1]) \o " " \o S(x[2]) \o " " \o S(x[3]) \o " " \o S(x[4]) \o " " \o S(x[5]) \o " " \o S(x[6])

IDENTITY == <<1, 0, 0, 1, 0, 0>>
\* the transforms set and multiplied (one singular)
Xforms == {<<2, 0, 0, 1, 3, 0>>, <<1, 1, 0, 1, 0, 2>>, <<1, 1, 1, 1, 0, 0>>}

\* GDI.XFORM.COMBINE: a, then b (eM11 eM12 eM21 eM22 eDx eDy)
Mul(a, b) == <<a[1] * b[1] + a[2] * b[3], a[1] * b[2] + a[2] * b[4], a[3] * b[1] + a[4] * b[3],
               a[3] * b[2] + a[4] * b[4], a[5] * b[1] + a[6] * b[3] + b[5], a[5] * b[2] + a[6] * b[4] + b[6]>>
Det(x) == x[1] * x[4] - x[2] * x[3]

\* GDI.XFORM.GET: read back after every call (the identity on a new DC)
Ask(w) == <<"> getxform 0", "getxform 1", Show(w)>>
Call(lines, g, w) == gmode' = g /\ world' = w /\ log' = log \o lines \o Ask(w) /\ n' = n + 1

Init == gmode = 1 /\ world = IDENTITY /\ n = 0 /\ log = <<"> dib 0 8 8 32">> \o Ask(IDENTITY)

\* GDI.XFORM.GMODE: the previous mode, 0 for one outside GM_COMPATIBLE .. GM_ADVANCED; set even
\* over a world transform other than the identity, the transform staying
SetMode(v) == IF v \in 1..2 THEN Call(<<"> gmode 0 " \o S(v), "gmode " \o S(gmode)>>, v, world)
              ELSE Call(<<"> gmode 0 " \o S(v), "gmode 0">>, gmode, world)

\* GDI.XFORM.SET, SINGULAR: only in GM_ADVANCED, and not a determinant of 0
Set(x) == IF gmode = 2 /\ Det(x) # 0 THEN Call(<<"> setxform 0 " \o Args(x), "setxform 1">>, gmode, x)
          ELSE Call(<<"> setxform 0 " \o Args(x), "setxform 0">>, gmode, world)

\* GDI.XFORM.MODIFY: MWT_IDENTITY resets, MWT_LEFTMULTIPLY the matrix times the transform,
\* MWT_RIGHTMULTIPLY the transform times the matrix; fails in GM_COMPATIBLE or another mode
Modify(m, x) ==
  LET line == "> modxform 0 " \o S(m) \o " " \o Args(x) IN
  IF gmode = 2 /\ m \in 1..3
  THEN Call(<<line, "modxform 1">>, gmode, CASE m = 1 -> IDENTITY [] m = 2 -> Mul(x, world) [] m = 3 -> Mul(world, x))
  ELSE Call(<<line, "modxform 0">>, gmode, world)

\* GDI.XFORM.COMBINE: CombineTransform of two transforms, no DC
Combine(a, b) == Call(<<"> combinexform " \o Args(a) \o " " \o Args(b), "combinexform 1", Show(Mul(a, b))>>, gmode, world)

Next == \/ \E v \in {0, 1, 2, 3} : SetMode(v)
        \/ \E x \in Xforms : Set(x)
        \/ \E m \in {0, 1, 2, 3, 4}, x \in Xforms : Modify(m, x)
        \/ \E a, b \in Xforms : Combine(a, b)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* the entries stay whole numbers single precision holds exactly
Exact == \A i \in 1..6 : world[i] >= 0 /\ world[i] < 2 ^ 24
=============================================================================
