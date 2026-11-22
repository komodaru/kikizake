---------------------------- MODULE GdiDibits ----------------------------
(* GetDIBits and SetDIBits between DIB sections of 2 by 1 pixels and DIBs of the color formats,
   from spec/gdi.txt: GetDIBits gives the bitmap's pixels converted into the format asked, the
   raw values when it is the bitmap's own, and answers the scan lines copied
   (GDI.DIBITS.GET); asked for 16 bpp 5-6-5 it gives a 16 bpp bitmap's own format, 5-5-5 for any
   other (GDI.DIBITS.GET16); SetDIBits converts the DIB's pixels into the bitmap's format
   (GDI.DIBITS.SET). Colors widen by bit replication and narrow by truncation, 32 bpp written
   from a color has x = 0 (GDI.FMT.EXPAND, NARROW, LAYOUT). The DIBs SetDIBits writes hold the
   bytes of the executor's xorshift32 for seeds 7 and 1234 (SPEC.md), here as constants. Every
   behavior takes at most MaxOps calls and dumps the four bitmaps after each; `kikizake tla`
   replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES bm, log, n
vars == <<bm, log, n>>

S(i) == ToString(i)
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
RECURSIVE Bytes(_)
Bytes(s) == IF s = <<>> THEN "" ELSE H2(Head(s)) \o Bytes(Tail(s))

\* the formats: <<bits per pixel, 5-6-5>>; the bitmaps 1..4 and their formats
F555 == <<16, FALSE>>
F565 == <<16, TRUE>>
F24 == <<24, FALSE>>
F32 == <<32, FALSE>>
F8 == <<8, FALSE>>
Fmt == <<F555, F565, F24, F32>>
Size(f) == f[1] \div 8
\* the executor's DIB line for a format, and its row's bytes for two pixels (DWORD aligned)
Name(f) == S(f[1]) \o (IF f[2] THEN " 565" ELSE "")
Stride(f) == ((2 * f[1] + 31) \div 32) * 4

\* a pixel's color <<r, g, b>> from its bytes (little-endian), and the bytes of a color
X5(c) == c * 8 + c \div 4
X6(c) == c * 4 + c \div 16
Pal8(i) == <<(((i \div 32) % 8) * 255) \div 7, (((i \div 4) % 8) * 255) \div 7, ((i % 4) * 255) \div 3>>
ToRgb(f, p) == CASE f = F555 -> LET v == p[1] + 256 * p[2] IN <<X5((v \div 1024) % 32), X5((v \div 32) % 32), X5(v % 32)>>
                 [] f = F565 -> LET v == p[1] + 256 * p[2] IN <<X5((v \div 2048) % 32), X6((v \div 32) % 64), X5(v % 32)>>
                 [] f = F8 -> Pal8(p[1])
                 [] OTHER -> <<p[3], p[2], p[1]>>
FromRgb(f, c) == CASE f = F555 -> LET v == (c[1] \div 8) * 1024 + (c[2] \div 8) * 32 + c[3] \div 8 IN <<v % 256, v \div 256>>
                   [] f = F565 -> LET v == (c[1] \div 8) * 2048 + (c[2] \div 4) * 32 + c[3] \div 8 IN <<v % 256, v \div 256>>
                   [] f = F24 -> <<c[3], c[2], c[1]>>
                   [] f = F32 -> <<c[3], c[2], c[1], 0>>
\* GDI.FMT.RAW: between the same formats the values themselves
Conv(from, to, p) == IF from = to THEN p ELSE FromRgb(to, ToRgb(from, p))

Row(f, px) == px[1] \o px[2]
Ask(b) == [i \in 1..12 |-> IF i % 3 = 1 THEN "> dump " \o S((i + 2) \div 3)
                           ELSE IF i % 3 = 2 THEN "px " \o S((i + 2) \div 3) \o " 2 1 " \o S(Fmt[(i + 2) \div 3][1])
                           ELSE Bytes(Row(Fmt[i \div 3], b[i \div 3]))]
Call(lines, b) == bm' = b /\ log' = log \o lines \o Ask(b) /\ n' = n + 1

Init == /\ bm = [k \in 1..4 |-> [x \in 1..2 |-> [i \in 1..Size(Fmt[k]) |-> 0]]]
        /\ log = <<"> dib 1 2 1 16", "> dib 2 2 1 16 565", "> dib 3 2 1 24", "> dib 4 2 1 32">>
        /\ n = 0

\* pixels written into a bitmap's memory
Pokes == <<<<52, 210>>, <<255, 127>>, <<10, 200, 77>>, <<1, 2, 3, 255>>>>
Poke(k, x) == Call(<<"> poke " \o S(k) \o " " \o S(x - 1) \o " 0 " \o Bytes(Pokes[k])>>, [bm EXCEPT ![k][x] = Pokes[k]])

\* GDI.DIBITS.GET, GET16
Get(k, f) ==
  LET to == IF f = F565 THEN (IF Fmt[k] = F565 THEN F565 ELSE F555) ELSE f
  IN Call(<<"> getdibits " \o S(k) \o " " \o Name(f), "dibits 1", Bytes(Conv(Fmt[k], to, bm[k][1]) \o Conv(Fmt[k], to, bm[k][2]))>>, bm)

\* GDI.DIBITS.SET: the DIB's two pixels, the first bytes of its row of xorshift32 bytes
Seeds == <<<<7, <<231, 7, 67, 69, 255, 148, 186, 87>>>>, <<1234, <<127, 193, 50, 178, 32, 207, 87, 178>>>>>>
Set(k, f, s) ==
  LET r == Seeds[s][2]
      sz == Size(f)
      px(x) == [i \in 1..sz |-> r[(x - 1) * sz + i]]
  IN Call(<<"> setdibits " \o S(k) \o " " \o Name(f) \o " " \o S(Seeds[s][1]), "setdibits 1">>,
          [bm EXCEPT ![k] = [x \in 1..2 |-> Conv(f, Fmt[k], px(x))]])

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ \E k \in 1..4, x \in 1..2 : Poke(k, x)
           \/ \E k \in 1..4, f \in {F555, F565, F24, F32} : Get(k, f)
           \/ \E k \in 1..4, f \in {F555, F565, F24, F32, F8}, s \in 1..2 : Set(k, f, s)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == \A k \in 1..4 : \A x \in 1..2 : Len(bm[k][x]) = Size(Fmt[k])
=============================================================================
