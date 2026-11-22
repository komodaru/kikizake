---------------------------- MODULE GdiModes ----------------------------
(* A device context's drawing modes, from spec/gdi.txt and spec/gdipath.txt: SetROP2,
   SetPolyFillMode, SetArcDirection (GDI.STATE.OLD, GDI.ROP2.ZERO) and SetMiterLimit
   (GDI.PATH.MITER), each answering the previous value. Every behavior takes at most MaxOps
   calls on a fresh DC; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES rop2, polyfill, arcdir, miter, log, n
vars == <<rop2, polyfill, arcdir, miter, log, n>>

S(i) == ToString(i)

\* the defaults: R2_COPYPEN, ALTERNATE, AD_COUNTERCLOCKWISE, a miter limit of 10
Init == rop2 = 13 /\ polyfill = 1 /\ arcdir = 1 /\ miter = 10000 /\ n = 0 /\ log = <<"> dib 0 8 8 8">>

\* GDI.STATE.OLD: the previous mode; a mode out of range answers 0 and leaves the mode;
\* GDI.ROP2.ZERO: SetROP2(0) is taken
SetRop2(v) == /\ log' = log \o <<"> rop2 0 " \o S(v), "rop2 " \o (IF v \in 0..16 THEN S(rop2) ELSE "0")>>
              /\ rop2' = (IF v \in 0..16 THEN v ELSE rop2) /\ UNCHANGED <<polyfill, arcdir, miter>>
SetFill(v) == /\ log' = log \o <<"> polyfill 0 " \o S(v), "polyfill " \o (IF v \in 1..2 THEN S(polyfill) ELSE "0")>>
              /\ polyfill' = (IF v \in 1..2 THEN v ELSE polyfill) /\ UNCHANGED <<rop2, arcdir, miter>>
SetArc(v) == /\ log' = log \o <<"> arcdir 0 " \o S(v), "arcdir " \o (IF v \in 1..2 THEN S(arcdir) ELSE "0")>>
             /\ arcdir' = (IF v \in 1..2 THEN v ELSE arcdir) /\ UNCHANGED <<rop2, polyfill, miter>>

\* GDI.PATH.MITER: a limit below 1 fails without writing the old value (-1000 thousandths read
\* back); otherwise the old limit, the new one kept; the limits here in thousandths
Limits == {<<"3", 3000>>, <<"0.5", 500>>, <<"20", 20000>>, <<"1", 1000>>}
SetMiter(l) == /\ log' = log \o <<"> miter 0 " \o l[1], IF l[2] < 1000 THEN "miter 0 -1000" ELSE "miter 1 " \o S(miter)>>
               /\ miter' = (IF l[2] < 1000 THEN miter ELSE l[2]) /\ UNCHANGED <<rop2, polyfill, arcdir>>

Next == /\ n' = n + 1
        /\ \/ \E v \in {0, 1, 13, 16, 17, -1} : SetRop2(v)
           \/ \E v \in {0, 1, 2, 3} : SetFill(v) \/ SetArc(v)
           \/ \E l \in Limits : SetMiter(l)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* every mode stays one the DC takes
InRange == rop2 \in 0..16 /\ polyfill \in 1..2 /\ arcdir \in 1..2 /\ miter >= 1000
=============================================================================
