---------------------------- MODULE GdiBlit ----------------------------
(* AlphaBlend, TransparentBlt, MaskBlt and PlgBlt into a 4 by 2 32 bpp DIB section, from
   spec/gdiblend.txt: their refusals (GDI.ALPHA.SRCRECT, EXTENTS, PPFORMAT, GDI.TRANS.SRCOUT,
   EXTENTS, GDI.MASK.EMPTY, OUTSIDE), what they draw (GDI.ALPHA.CONSTANT with the constant alpha
   255, 128 or 0, GDI.ALPHA.OPAQUE, FORMATS, STRETCH; GDI.TRANS.KEY, COPY, STRETCH, FORMATS;
   GDI.MASK.ROP4, NONE, NEGATIVE; GDI.PLG.RECT, TURN). The sources: slot 1, 3 by 2 at 32 bpp of
   six opaque colors; slot 2, 3 by 2 at 8 bpp, pixel (x, y) entry (x + y) % 3 of its own color
   table; slot 3, the 3 by 2 monochrome mask with (0, 0), (1, 0) and (2, 1) set. Every behavior
   takes at most MaxOps calls and dumps the destination after each; `kikizake tla` replays every
   state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 4
H == 2
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES px, log, n
vars == <<px, log, n>>

S(i) == ToString(i)
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Abs(a) == IF a < 0 THEN -a ELSE a
Sg(a) == IF a < 0 THEN -1 ELSE IF a > 0 THEN 1 ELSE 0
\* Rust's div_euclid: the remainder never negative
DivE(a, b) == IF b > 0 THEN a \div b ELSE -(a \div (-b))
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
Bit(v, k) == (v \div (2 ^ k)) % 2
RECURSIVE Op3(_, _, _, _, _)
Op3(idx, p, s, d, k) == IF k = 0 THEN 0
                        ELSE Bit(idx, 4 * (p % 2) + 2 * (s % 2) + (d % 2)) + 2 * Op3(idx, p \div 2, s \div 2, d \div 2, k - 1)
Rop(idx, p, s, d) == [i \in 1..4 |-> Op3(idx, p[i], s[i], d[i], 8)]

\* pixels are their memory bytes B G R A; colors <<r, g, b>>
Px(c) == <<c[3], c[2], c[1], 0>>
Mem(p) == H2(p[1]) \o H2(p[2]) \o H2(p[3]) \o H2(p[4])
Color(p) == <<p[3], p[2], p[1]>>
CHex(c) == H2(c[1]) \o H2(c[2]) \o H2(c[3])

Src1(x, y) == CASE <<x, y>> = <<0, 0>> -> <<16, 32, 48, 255>> [] <<x, y>> = <<1, 0>> -> <<64, 80, 96, 255>>
                [] <<x, y>> = <<2, 0>> -> <<255, 255, 255, 255>> [] <<x, y>> = <<0, 1>> -> <<1, 2, 3, 255>>
                [] <<x, y>> = <<1, 1>> -> <<48, 32, 16, 255>> [] <<x, y>> = <<2, 1>> -> <<0, 0, 0, 255>>
Pal == <<<<160, 16, 32>>, <<48, 64, 255>>, <<0, 192, 96>>>>
Src2(x, y) == Pal[((x + y) % 3) + 1]
Mask(x, y) == IF <<x, y>> \in {<<0, 0>>, <<1, 0>>, <<2, 1>>} THEN 1 ELSE 0
\* the white stock brush
Brush == <<255, 255, 255, 0>>

Dump(p) == <<"> dump 0", "px 0 4 2 32">> \o [y \in 1..H |-> Mem(p[<<0, y - 1>>]) \o Mem(p[<<1, y - 1>>]) \o Mem(p[<<2, y - 1>>]) \o Mem(p[<<3, y - 1>>])]
Call(lines, p) == px' = p /\ log' = log \o lines \o Dump(p) /\ n' = n + 1

Init == /\ px = [q \in Dev |-> <<0, 0, 0, 0>>]
        /\ log = <<"> dib 0 4 2 32", "> dib 1 3 2 32", "> poke 1 0 0 102030ff", "> poke 1 1 0 405060ff",
                   "> poke 1 2 0 ffffffff", "> poke 1 0 1 010203ff", "> poke 1 1 1 302010ff", "> poke 1 2 1 000000ff",
                   "> dib 2 3 2 8 #a01020 #3040ff #00c060", "> poke 2 1 0 01", "> poke 2 2 0 02", "> poke 2 0 1 01", "> poke 2 1 1 02",
                   "> dib 3 3 2 1", "> patblt 3 0 0 2 1 0xff0062", "ret 1", "> patblt 3 2 1 1 1 0xff0062", "ret 1">>
        /\ n = 0

\* GDI.STRETCH.DDA: the source pixel of destination pixel i of n from sn; flat when the other
\* direction is unscaled
Sample(i, sn, k, flat) == IF sn = k THEN i
                          ELSE IF flat /\ k + 1 = sn THEN (i * sn) \div k
                          ELSE IF flat /\ k = sn + 1 THEN (i * sn + sn - 1) \div k
                          ELSE ((2 * i + 1) * sn) \div (2 * k)
\* the destination pixels of rectangle d, each with the source pixel (of rectangle s) it takes
Under(d, s, q) == <<s[1] + Sample(q[1] - d[1], s[3], d[3], s[4] = d[4]), s[2] + Sample(q[2] - d[2], s[4], d[4], s[3] = d[3])>>
Area(d) == {q \in Dev : q[1] >= d[1] /\ q[1] < d[1] + d[3] /\ q[2] >= d[2] /\ q[2] < d[2] + d[4]}

\* GDI.ALPHA.ROUND: s a / 255 + d (255 - a) / 255 rounded
Mix(s, d, a) == (s * a + d * (255 - a) + 127) \div 255
\* (slot 3's color table: black, white)
SrcPx(k, q) == CASE k = 1 -> Src1(q[1], q[2]) [] k = 2 -> Px(Src2(q[1], q[2]))
                 [] k = 3 -> IF Mask(q[1], q[2]) = 1 THEN <<255, 255, 255, 0>> ELSE <<0, 0, 0, 0>>
Inside(s, k) == s[1] >= 0 /\ s[2] >= 0 /\ s[1] + s[3] <= 3 /\ s[2] + s[4] <= 2
AlphaBlend(d, k, s, sca, pp) ==
  LET line == "> alphablend 0 " \o R(d) \o " " \o S(k) \o " " \o R(s) \o " " \o S(sca) \o " " \o S(pp)
      \* GDI.ALPHA.FORMATS: a source with a color table, or stretched, at a constant alpha
      \* below 255 is opaque; otherwise it has alpha 0
      opaque == (k = 2 \/ d[3] # s[3] \/ d[4] # s[4]) /\ sca < 255
      s32(q) == IF k = 1 THEN Src1(q[1], q[2]) ELSE [Px(Src2(q[1], q[2])) EXCEPT ![4] = IF opaque THEN 255 ELSE 0]
      blend(q) == LET a == s32(Under(d, s, q)) IN [i \in 1..4 |-> Mix(a[i], px[q][i], sca)]
  IN \* GDI.ALPHA.EXTENTS: a negative source extent fails, then an empty one succeeds drawing
     \* nothing, then a negative destination extent fails
     IF d[3] <= 0 \/ d[4] <= 0 \/ s[3] <= 0 \/ s[4] <= 0
     THEN Call(<<line, IF s[3] >= 0 /\ s[4] >= 0 /\ (d[3] = 0 \/ d[4] = 0 \/ s[3] = 0 \/ s[4] = 0) THEN "ret 1" ELSE "ret 0">>, px)
     \* GDI.ALPHA.SRCRECT, GDI.ALPHA.PPFORMAT
     ELSE IF ~Inside(s, k) \/ (pp = 1 /\ k # 1) THEN Call(<<line, "ret 0">>, px)
     \* GDI.ALPHA.CONSTANT, OPAQUE (every pixel of slot 1 has alpha 255), STRETCH
     ELSE Call(<<line, "ret 1">>, [q \in Dev |-> IF q \in Area(d) THEN blend(q) ELSE px[q]])

\* GDI.TRANS.EXTENTS, SRCOUT; GDI.TRANS.KEY: pixels of the key's color are left; GDI.TRANS.COPY,
\* FORMATS: the others written as their color (x 0); GDI.TRANS.STRETCH
Transparent(d, k, s, key) ==
  LET line == "> transparent 0 " \o R(d) \o " " \o S(k) \o " " \o R(s) \o " #" \o CHex(key)
      c(q) == Color(SrcPx(k, Under(d, s, q)))
  IN IF d[3] <= 0 \/ d[4] <= 0 \/ s[3] <= 0 \/ s[4] <= 0 \/ ~Inside(s, k) THEN Call(<<line, "ret 0">>, px)
     ELSE Call(<<line, "ret 1">>, [q \in Dev |-> IF q \in Area(d) /\ c(q) # key THEN Px(c(q)) ELSE px[q]])

\* GDI.MASK.NONE: BitBlt with the foreground operation; GDI.MASK.NEGATIVE: the origins moved back
\* by a negative extent, drawn unmirrored; GDI.MASK.EMPTY; GDI.MASK.OUTSIDE: a rectangle landing
\* where both bitmaps are fails unless the mask holds it all; GDI.MASK.ROP4: the foreground
\* operation where the mask is 1, the background one where it is 0, pixels outside the source
\* or the mask left
Lands(p, sp, e, de, se) == Max(Max(-p, -sp), 0) < Min(Min(e, de - p), se - sp)
MaskNone(d, sp, rop) ==
  LET line == "> maskblt 0 " \o R(d) \o " 1 " \o S(sp[1]) \o " " \o S(sp[2]) \o " none 0 0 " \o rop[1]
      sq(q) == <<sp[1] + q[1] - d[1], sp[2] + q[2] - d[2]>>
      ok(q) == sq(q)[1] \in 0..2 /\ sq(q)[2] \in 0..1
  IN Call(<<line, "ret 1">>, [q \in Dev |-> IF q \in Area(d) /\ ok(q) THEN Rop(rop[2] % 256, Brush, Src1(sq(q)[1], sq(q)[2]), px[q]) ELSE px[q]])
MaskBlt(d0, sp0, mp0, rop) ==
  LET line == "> maskblt 0 " \o R(d0) \o " 1 " \o S(sp0[1]) \o " " \o S(sp0[2]) \o " 3 " \o S(mp0[1]) \o " " \o S(mp0[2]) \o " " \o rop[1]
      x == IF d0[3] < 0 THEN d0[1] + d0[3] ELSE d0[1]
      y == IF d0[4] < 0 THEN d0[2] + d0[4] ELSE d0[2]
      sx == IF d0[3] < 0 THEN sp0[1] + d0[3] ELSE sp0[1]
      sy == IF d0[4] < 0 THEN sp0[2] + d0[4] ELSE sp0[2]
      mx == IF d0[3] < 0 THEN mp0[1] + d0[3] ELSE mp0[1]
      my == IF d0[4] < 0 THEN mp0[2] + d0[4] ELSE mp0[2]
      w == Abs(d0[3])
      h == Abs(d0[4])
      fore == rop[2] % 256
      back == rop[2] \div 256
      ok(i, j) == sx + i >= 0 /\ sy + j >= 0 /\ sx + i < 3 /\ sy + j < 2 /\ mx + i < 3 /\ my + j < 2
      img == [q \in Dev |-> LET i == q[1] - x j == q[2] - y IN
                IF i >= 0 /\ i < w /\ j >= 0 /\ j < h /\ ok(i, j)
                THEN Rop(IF Mask(mx + i, my + j) = 1 THEN fore ELSE back, Brush, Src1(sx + i, sy + j), px[q])
                ELSE px[q]]
  IN IF w = 0 \/ h = 0 THEN Call(<<line, "ret 1">>, px)
     ELSE IF Lands(x, sx, w, W, 3) /\ Lands(y, sy, h, H, 2) /\ (mx < 0 \/ my < 0 \/ mx + w > 3 \/ my + h > 2) THEN Call(<<line, "ret 0">>, px)
     ELSE Call(<<line, "ret 1">>, img)

\* GDI.PLG.RECT: a rectangle draws as StretchBlt, a negative side from P0's pixel back;
\* GDI.PLG.TURN: a quarter turn, each pixel the source pixel under its center
PlgBlt(p, s) ==
  LET line == "> plgblt 0 " \o S(p[1]) \o " " \o S(p[2]) \o " " \o S(p[3]) \o " " \o S(p[4]) \o " " \o S(p[5]) \o " " \o S(p[6]) \o " 1 " \o R(s)
      ax == p[3] - p[1]
      ay == p[4] - p[2]
      bx == p[5] - p[1]
      by == p[6] - p[2]
      dd == ax * by - ay * bx
      rect == {<<p[1] + ox * Sg(ax), p[2] + oy * Sg(by)>> : ox \in 0..(Abs(ax) - 1), oy \in 0..(Abs(by) - 1)}
      rsrc(q) == <<s[1] + Sample(Abs(q[1] - p[1]), s[3], Abs(ax), s[4] = Abs(by)), s[2] + Sample(Abs(q[2] - p[2]), s[4], Abs(by), s[3] = Abs(ax))>>
      tu(q) == DivE(s[3] * ((2 * (q[1] - p[1]) + 1) * by - (2 * (q[2] - p[2]) + 1) * bx), 2 * dd)
      tv(q) == DivE(s[4] * (ax * (2 * (q[2] - p[2]) + 1) - ay * (2 * (q[1] - p[1]) + 1)), 2 * dd)
  IN IF ay = 0 /\ bx = 0
     THEN Call(<<line, "ret 1">>, [q \in Dev |-> IF q \in rect THEN Src1(rsrc(q)[1], rsrc(q)[2]) ELSE px[q]])
     ELSE Call(<<line, "ret 1">>, [q \in Dev |-> IF tu(q) \in 0..(s[3] - 1) /\ tv(q) \in 0..(s[4] - 1) THEN Src1(s[1] + tu(q), s[2] + tv(q)) ELSE px[q]])

Next == /\ n < MaxOps
        /\ \/ \E sca \in {255, 128, 0} : AlphaBlend(<<0, 0, 2, 2>>, 1, <<1, 0, 2, 2>>, sca, 0) \/ AlphaBlend(<<1, 0, 3, 2>>, 2, <<0, 0, 3, 2>>, sca, 0)
           \/ \E sca \in {255, 128} : AlphaBlend(<<1, 0, 3, 2>>, 1, <<0, 1, 2, 1>>, sca, 0) \/ AlphaBlend(<<0, 0, 4, 2>>, 2, <<1, 0, 2, 2>>, sca, 0)
           \/ AlphaBlend(<<0, 0, 3, 2>>, 1, <<0, 0, 3, 2>>, 128, 1)
           \/ AlphaBlend(<<0, 0, 3, 2>>, 2, <<0, 0, 3, 2>>, 255, 1)
           \/ AlphaBlend(<<0, 0, 2, 2>>, 1, <<2, 0, 2, 2>>, 255, 0)
           \/ \E e \in {<<0, 2, 2, 2>>, <<2, 2, -1, 2>>, <<-2, 2, 2, 2>>, <<-2, 2, 0, 2>>, <<2, 0, 2, -1>>} :
                 AlphaBlend(<<1, 0, e[1], e[2]>>, 1, <<0, 0, e[3], e[4]>>, 255, 0)
           \/ Transparent(<<0, 0, 3, 2>>, 1, <<0, 0, 3, 2>>, <<48, 32, 16>>)
           \/ Transparent(<<0, 0, 4, 2>>, 1, <<1, 0, 2, 1>>, <<255, 255, 255>>)
           \/ Transparent(<<1, 0, 3, 2>>, 2, <<0, 0, 3, 2>>, <<48, 64, 255>>)
           \/ Transparent(<<0, 0, 3, 2>>, 3, <<0, 0, 3, 2>>, <<255, 255, 255>>)
           \/ Transparent(<<0, 0, 2, 2>>, 1, <<2, 1, 2, 2>>, <<0, 0, 0>>)
           \/ \E e \in {<<0, 2, 2, 2>>, <<2, -1, 2, 2>>, <<2, 2, 0, 2>>} : Transparent(<<0, 0, e[1], e[2]>>, 1, <<0, 0, e[3], e[4]>>, <<0, 0, 0>>)
           \/ \E rop \in {<<"0x55cc0020", 21964>>, <<"0xccf00021", 52464>>} :
                 \/ MaskBlt(<<0, 0, 3, 2>>, <<0, 0>>, <<0, 0>>, rop)
                 \/ MaskBlt(<<1, 0, 3, 2>>, <<0, 0>>, <<0, 0>>, rop)
                 \/ MaskBlt(<<3, 2, -3, -2>>, <<3, 2>>, <<3, 2>>, rop)
                 \/ MaskBlt(<<0, 0, 3, 2>>, <<0, 0>>, <<1, 0>>, rop)
                 \/ MaskBlt(<<0, 0, 3, 2>>, <<3, 0>>, <<1, 0>>, rop)
                 \/ MaskBlt(<<0, 0, 0, 2>>, <<0, 0>>, <<5, 5>>, rop)
           \/ MaskNone(<<0, 0, 2, 2>>, <<1, 0>>, <<"0x55cc0020", 204>>)
           \/ PlgBlt(<<0, 0, 3, 0, 0, 2>>, <<0, 0, 3, 2>>)
           \/ PlgBlt(<<3, 1, 0, 1, 3, -1>>, <<0, 0, 3, 2>>)
           \/ PlgBlt(<<0, 0, 4, 0, 0, 2>>, <<1, 0, 2, 1>>)
           \/ PlgBlt(<<2, 0, 2, 2, 0, 0>>, <<0, 0, 2, 2>>)
           \/ PlgBlt(<<0, 1, 0, -2, 1, 1>>, <<0, 0, 3, 1>>)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == \A q \in Dev : \A i \in 1..4 : px[q][i] \in 0..255
=============================================================================
