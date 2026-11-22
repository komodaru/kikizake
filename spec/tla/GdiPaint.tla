---------------------------- MODULE GdiPaint ----------------------------
(* PatBlt and BitBlt into a 4 by 2 32 bpp DIB section, from spec/gdi.txt: the operations of the
   pattern and the destination (GDI.PATBLT.ROPS, a negative extent GDI.BLT.PATNEG), solid and DIB
   pattern brushes and the brush origin (GDI.BRUSH.SOLID, DIBPATTERN, ORIGIN), the clip region and
   the bitmap's edges (GDI.BLT.CLIP), BitBlt converting a 16 bpp and a color table source
   (GDI.BLT.CONVERT), GetPixel and SetPixel (GDI.PIXEL.GET, SET) on the 32 bpp DIB and on a 16 bpp
   one (its colors truncated to 5 bits and widened back). A pixel is its four memory bytes
   B G R x; every operation is the ternary operation's truth table on each bit (GDI.ROP3.TRUTH).
   Every behavior takes at most MaxOps calls and dumps the bitmap after each, so the log holds
   every pixel; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 4
H == 2
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES px, src, clip, brush, org, log, n
vars == <<px, src, clip, brush, org, log, n>>

S(i) == ToString(i)
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
Bit(v, k) == (v \div (2 ^ k)) % 2

\* a ternary raster operation on k bits: each result bit is bit (P<<2 | S<<1 | D) of the index
RECURSIVE Op3(_, _, _, _, _)
Op3(idx, p, s, d, k) == IF k = 0 THEN 0
                        ELSE Bit(idx, 4 * (p % 2) + 2 * (s % 2) + (d % 2)) + 2 * Op3(idx, p \div 2, s \div 2, d \div 2, k - 1)
Rop(idx, p, s, d) == [i \in 1..4 |-> Op3(idx, p[i], s[i], d[i], 8)]

\* colors <<r, g, b>>; a 32 bpp pixel B G R x, a color written with x = 0
Px(c) == <<c[3], c[2], c[1], 0>>
Zero == <<0, 0, 0, 0>>
CHex(c) == H2(c[1]) \o H2(c[2]) \o H2(c[3])
PHex(p) == H2(p[3]) \o H2(p[2]) \o H2(p[1])
Mem(p) == H2(p[1]) \o H2(p[2]) \o H2(p[3]) \o H2(p[4])

\* the solid brushes, and the pattern: DIB 3, 3 by 2 at 8 bpp, pixel (x, y) entry (x + y) % 3 of
\* its own color table
Solids == {<<18, 52, 86>>, <<255, 255, 255>>}
Pal == <<<<160, 16, 32>>, <<48, 64, 255>>, <<0, 192, 96>>>>
PatAt(x, y) == Pal[((x + y) % 3) + 1]

\* the 16 bpp DIB 1 (2 by 1, 5-5-5): truncated, widened by bit replication
To555(c) == (c[1] \div 8) * 1024 + (c[2] \div 8) * 32 + c[3] \div 8
X5(v) == v * 8 + v \div 4
From555(v) == <<X5((v \div 1024) % 32), X5((v \div 32) % 32), X5(v % 32)>>

\* the clip region as in GdiClip.tla: none, or its pixels
NONE == <<FALSE, {}>>
Some(P) == <<TRUE, P>>
Pixels(r) == {<<x, y>> : x \in Min(r[1], r[3])..(Max(r[1], r[3]) - 1), y \in Min(r[2], r[4])..(Max(r[2], r[4]) - 1)}
Xs(P) == {p[1] : p \in P}
Ys(P) == {p[2] : p \in P}
SetMin(X) == CHOOSE m \in X : \A k \in X : m <= k
SetMax(X) == CHOOSE m \in X : \A k \in X : m >= k
\* one rectangle (SIMPLEREGION): its bounding box
IsRect(P) == P # {} /\ P = (SetMin(Xs(P))..SetMax(Xs(P))) \X (SetMin(Ys(P))..SetMax(Ys(P)))
Visible(c) == IF c[1] THEN c[2] \cap Dev ELSE Dev

Dump(p) == <<"> dump 0", "px 0 4 2 32">> \o [y \in 1..H |-> Mem(p[<<0, y - 1>>]) \o Mem(p[<<1, y - 1>>]) \o Mem(p[<<2, y - 1>>]) \o Mem(p[<<3, y - 1>>])]

Call(lines, p) == px' = p /\ log' = log \o lines \o Dump(p) /\ n' = n + 1

Init == /\ px = [q \in Dev |-> Zero] /\ src = <<0, 0>> /\ clip = NONE /\ brush = <<"solid", <<255, 255, 255>>>> /\ org = <<0, 0>>
        /\ log = <<"> dib 0 4 2 32", "> dib 1 2 1 16", "> dib 3 3 2 8 #a01020 #3040ff #00c060",
                   "> poke 3 1 0 01", "> poke 3 2 0 02", "> poke 3 0 1 01", "> poke 3 1 1 02">>
        /\ n = 0

\* the brush's pixel at device pixel q: its color, or the pattern's pixel whose (0, 0) lies at
\* the brush origin, repeated from there (GDI.BRUSH.ORIGIN), in its color table's color
\* (GDI.BRUSH.DIBPATTERN)
BrushAt(q) == IF brush[1] = "pat" THEN Px(PatAt((q[1] - org[1]) % 3, (q[2] - org[2]) % 2)) ELSE Px(brush[2])

SetBrush(b) == /\ brush' = b /\ UNCHANGED <<src, clip, org>>
               /\ Call(<<IF b[1] = "pat" THEN "> brush 0 dib 3" ELSE "> brush 0 solid #" \o CHex(b[2])>>, px)
SetOrg(o) == /\ org' = o /\ UNCHANGED <<src, clip, brush>> /\ Call(<<"> org 0 " \o S(o[1]) \o " " \o S(o[2])>>, px)

\* GDI.PATBLT.ROPS, GDI.BLT.PATNEG, GDI.BLT.CLIP: the operation of the brush and the destination
\* on the rectangle (a negative extent on the other side of the point) where it is visible
PatRops == {<<"0xf00021", 240>>, <<"0x5a0049", 90>>, <<"0x550009", 85>>, <<"0x000042", 0>>, <<"0xff0062", 255>>, <<"0xa000c9", 160>>}
PatRects == {<<1, 0, 2, 2>>, <<3, 1, -2, -1>>, <<-1, 1, 3, 5>>, <<2, 0, 0, 2>>}
PatBlt(r, op) ==
  LET area == Pixels(<<r[1], r[2], r[1] + r[3], r[2] + r[4]>>) \cap Visible(clip)
  IN /\ UNCHANGED <<src, clip, brush, org>>
     /\ Call(<<"> patblt 0 " \o R(r) \o " " \o op[1], "ret 1">>, [q \in Dev |-> IF q \in area THEN Rop(op[2], BrushAt(q), Zero, px[q]) ELSE px[q]])

\* GDI.BLT.CONVERT, GDI.BLT.CLIP: the 16 bpp DIB or the pattern's DIB (its color table) as colors
\* of the destination, at (x, y)
BltRops == {<<"0xcc0020", 204>>, <<"0x660046", 102>>, <<"0x8800c6", 136>>, <<"0xb8074a", 184>>}
Blt(ks, at, op) ==
  LET w == IF ks = 1 THEN 2 ELSE 3
      h == IF ks = 1 THEN 1 ELSE 2
      area == Pixels(<<at[1], at[2], at[1] + w, at[2] + h>>) \cap Visible(clip)
      sp(q) == IF ks = 1 THEN Px(From555(src[q[1] - at[1] + 1])) ELSE Px(PatAt(q[1] - at[1], q[2] - at[2]))
  IN /\ UNCHANGED <<src, clip, brush, org>>
     /\ Call(<<"> bitblt 0 " \o S(at[1]) \o " " \o S(at[2]) \o " " \o S(w) \o " " \o S(h) \o " " \o S(ks) \o " 0 0 " \o op[1], "ret 1">>,
             [q \in Dev |-> IF q \in area THEN Rop(op[2], BrushAt(q), sp(q), px[q]) ELSE px[q]])

\* GDI.CLIP.INTERSECT, RESULT, AGAIN (as GdiClip.tla)
Clip(r) ==
  LET q == Pixels(r)
      v == IF clip[1] THEN clip[2] \cap q ELSE q
      ret == IF clip[1] /\ v = {} /\ (~IsRect(clip[2]) \/ q = {}) THEN 1 ELSE 3
  IN clip' = Some(v) /\ UNCHANGED <<src, brush, org>> /\ Call(<<"> clip 0 " \o R(r), "clip " \o S(ret)>>, px)

\* GDI.PIXEL.GET: the pixel's color; CLR_INVALID outside the clip region (or the bitmap)
GetPixel(q) == /\ UNCHANGED <<src, clip, brush, org>>
               /\ Call(<<"> getpixel 0 " \o S(q[1]) \o " " \o S(q[2]), IF q \in Visible(clip) THEN "pixel " \o PHex(px[q]) ELSE "pixel invalid">>, px)
\* GDI.PIXEL.SET: the color set (exact in 32 bpp), invalid where it cannot be
SetPixel(q, c) == /\ UNCHANGED <<src, clip, brush, org>>
                  /\ IF q \in Visible(clip)
                     THEN Call(<<"> setpixel 0 " \o S(q[1]) \o " " \o S(q[2]) \o " #" \o CHex(c), "pixel " \o CHex(c)>>, [px EXCEPT ![q] = Px(c)])
                     ELSE Call(<<"> setpixel 0 " \o S(q[1]) \o " " \o S(q[2]) \o " #" \o CHex(c), "pixel invalid">>, px)
\* GDI.PIXEL.SET, GET on the 16 bpp DIB: the closest color it holds, which it answers
SetPixel1(x, c) == /\ UNCHANGED <<clip, brush, org>>
                   /\ IF x < 2
                      THEN src' = [src EXCEPT ![x + 1] = To555(c)] /\ Call(<<"> setpixel 1 " \o S(x) \o " 0 #" \o CHex(c), "pixel " \o CHex(From555(To555(c)))>>, px)
                      ELSE src' = src /\ Call(<<"> setpixel 1 " \o S(x) \o " 0 #" \o CHex(c), "pixel invalid">>, px)
GetPixel1(x) == /\ UNCHANGED <<src, clip, brush, org>>
                /\ Call(<<"> getpixel 1 " \o S(x) \o " 0", IF x < 2 THEN "pixel " \o CHex(From555(src[x + 1])) ELSE "pixel invalid">>, px)

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ \E b \in {<<"solid", c>> : c \in Solids} \cup {<<"pat", <<0, 0, 0>>>>} : SetBrush(b)
           \/ \E o \in {<<1, 0>>, <<-1, 1>>} : SetOrg(o)
           \/ \E r \in PatRects, op \in PatRops : PatBlt(r, op)
           \/ \E at \in {<<1, 0>>, <<3, 1>>}, op \in BltRops : Blt(1, at, op)
           \/ \E op \in BltRops : Blt(3, <<0, 0>>, op)
           \/ \E r \in {<<1, 0, 4, 2>>, <<0, 1, 3, 2>>, <<3, 0, 3, 2>>} : Clip(r)
           \/ \E q \in {<<0, 0>>, <<3, 1>>, <<4, 0>>, <<1, -1>>} : GetPixel(q)
           \/ \E q \in {<<0, 1>>, <<4, 1>>}, c \in {<<1, 2, 3>>, <<200, 100, 50>>} : SetPixel(q, c)
           \/ \E x \in 0..2, c \in {<<255, 135, 67>>, <<7, 8, 250>>} : SetPixel1(x, c)
           \/ \E x \in 0..2 : GetPixel1(x)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* every pixel is four bytes; the 16 bpp pixels keep their top bit 0
TypeOK == /\ \A q \in Dev : \A i \in 1..4 : px[q][i] \in 0..255
          /\ \A i \in 1..2 : src[i] \in 0..32767
=============================================================================
