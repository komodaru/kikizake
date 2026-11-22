---------------------------- MODULE GdiText ----------------------------
(* A DC's text state and ExtTextOut's rectangle on a 4 by 2 32 bpp DIB section, from
   spec/gditext.txt: SetTextAlign and SetTextCharacterExtra answer the previous value, a new DC's
   0 (GDI.TEXT.ALIGN, EXTRA); ExtTextOut with no characters draws only its rectangle, with
   ETO_OPAQUE, in the background color (GDI.TEXT.NOTEXT, OPAQUE), its corners in either order
   (GDI.TEXT.OPAQUEORDER), and succeeds. Every behavior takes at most MaxOps calls and dumps the
   bitmap after each; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 4
H == 2
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES px, align, extra, bk, log, n
vars == <<px, align, extra, bk, log, n>>

S(i) == ToString(i)
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
Mem(c) == H2(c[3]) \o H2(c[2]) \o H2(c[1]) \o "00"
CHex(c) == H2(c[1]) \o H2(c[2]) \o H2(c[3])

Dump(p) == <<"> dump 0", "px 0 4 2 32">> \o [y \in 1..H |-> Mem(p[<<0, y - 1>>]) \o Mem(p[<<1, y - 1>>]) \o Mem(p[<<2, y - 1>>]) \o Mem(p[<<3, y - 1>>])]
Call(lines, p) == px' = p /\ log' = log \o lines \o Dump(p) /\ n' = n + 1

\* a new DC: TA_LEFT | TA_TOP | TA_NOUPDATECP, no extra, a white background
Init == px = [q \in Dev |-> <<0, 0, 0>>] /\ align = 0 /\ extra = 0 /\ bk = <<255, 255, 255>> /\ log = <<"> dib 0 4 2 32">> /\ n = 0

SetAlign(v) == align' = v /\ UNCHANGED <<extra, bk>> /\ Call(<<"> align 0 " \o S(v), "align " \o S(align)>>, px)
SetExtra(v) == extra' = v /\ UNCHANGED <<align, bk>> /\ Call(<<"> extra 0 " \o S(v), "extra " \o S(extra)>>, px)
SetBk(c) == bk' = c /\ UNCHANGED <<align, extra>> /\ Call(<<"> bk 0 #" \o CHex(c)>>, px)
\* (no characters: the text token is left out)
ExtText(at, o, r) ==
  LET area == (Min(r[1], r[3])..(Max(r[1], r[3]) - 1)) \X (Min(r[2], r[4])..(Max(r[2], r[4]) - 1))
  IN /\ UNCHANGED <<align, extra, bk>>
     /\ Call(<<"> exttext 0 " \o S(at[1]) \o " " \o S(at[2]) \o " " \o S(o) \o " " \o R(r), "ret 1">>,
             [q \in Dev |-> IF (o \div 2) % 2 = 1 /\ q \in area THEN bk ELSE px[q]])

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off; TA_UPDATECP
\* is not asked for)
Next == /\ n < MaxOps
        /\ \/ \E v \in {24, 6, 0} : SetAlign(v)
           \/ \E v \in {3, -2, 0} : SetExtra(v)
           \/ \E c \in {<<18, 52, 86>>, <<255, 255, 255>>} : SetBk(c)
           \/ \E at \in {<<0, 0>>, <<2, 1>>}, o \in {0, 2, 4, 6}, r \in {<<1, 0, 3, 2>>, <<3, 2, 0, 1>>, <<-1, 1, 5, 9>>, <<2, 0, 2, 2>>} : ExtText(at, o, r)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == align \in {0, 6, 24} /\ extra \in {-2, 0, 3}
=============================================================================
