---------------------------- MODULE GdiMapDraw ----------------------------
(* Drawing through a mapping or a world transform on a 5 by 4 8 bpp DIB section, from
   spec/gdixform.txt and spec/gdiline.txt: under MM_ANISOTROPIC halving x (window 2 1, viewport
   1 1), LineTo, Rectangle and PatBlt draw as the same op in device coordinates, each point
   through LPtoDP (a logical x of 1 falls between pixels and rounds to 1), the current position
   staying logical, a pen of width 1 thin while x shrinks (GDI.MAP.DRAWPTS); through a GM_ADVANCED
   world turning a quarter (0 1 -1 0 3 0) or turning an eighth and stretching by √2 (1 1 -1 1 2 0),
   PatBlt fills its parallelogram by the path fill rule (GDI.MAP.DRAWPTS), LineTo draws the 28.4
   line between its ends and Rectangle fills its parallelogram and outlines it with four such
   lines, a pen of width 1 staying thin up to √2 (GDI.MAP.DRAWADV); CreatePen's widths 0 and 1
   draw one pixel wide through all of them (GDI.PEN.CREATE). The logical points are chosen so the
   device lines are level, upright or diagonal (whole ends: GDI.LINE.GIQ is GDI.LINE.PIXELS), and
   every parallelogram's interior is the pixels left of an odd number of its edges' crossings
   at the row's top (GDI.PATH.FILLFIX, GDI.RGN.POLYSCAN). Every behavior starts in one of the
   three mappings and takes at most MaxOps calls, dumping the bitmap after each; `kikizake tla`
   replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 5
H == 4
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES map, px, pen, pos, log, n
vars == <<map, px, pen, pos, log, n>>

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Abs(a) == IF a < 0 THEN -a ELSE a
Sg(a) == IF a < 0 THEN -1 ELSE IF a > 0 THEN 1 ELSE 0
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]

\* the default 8 bpp color table's entries of the colors used
Brush == 28
Colors == [c \in {"#ff0000", "#0000ff"} |-> IF c = "#ff0000" THEN 224 ELSE 3]

\* a point to device pixels: the page rule (x / 2, halves up) or the world's
Dp(p) == CASE map = "page" -> <<(p[1] + 1) \div 2, p[2]>>
           [] map = "rot" -> <<3 - p[2], p[1]>>
           [] map = "diag" -> <<p[1] - p[2] + 2, p[1] + p[2]>>
Grid == CASE map = "page" -> {<<0, 0>>, <<4, 0>>, <<0, 2>>, <<4, 2>>, <<1, 1>>}
          [] map = "rot" -> {<<0, 0>>, <<2, 0>>, <<0, 2>>, <<2, 2>>, <<1, 1>>}
          [] map = "diag" -> {<<0, 0>>, <<1, 0>>, <<0, 1>>, <<1, 1>>}

LinePx(a, b) == LET dx == b[1] - a[1]
                    dy == b[2] - a[2]
                IN [k \in 1..Max(Abs(dx), Abs(dy)) |-> <<a[1] + (k - 1) * Sg(dx), a[2] + (k - 1) * Sg(dy)>>]
RECURSIVE PolyPx(_)
PolyPx(s) == IF Len(s) < 2 THEN <<>> ELSE LinePx(s[1], s[2]) \o PolyPx(Tail(s))
Range(s) == {s[i] : i \in 1..Len(s)}
\* the pen copied over the pixels, the brush over a set (R2_COPYPEN)
Draw(img, q) == LET on == Range(q) \cap Dev IN [d \in Dev |-> IF d \in on THEN Colors[pen[1]] ELSE img[d]]
Fill(img, A) == [d \in Dev |-> IF d \in A THEN Brush ELSE img[d]]
Span(l, t, r, b) == (l..(r - 1)) \X (t..(b - 1))
\* a closed polygon's interior: pixels with an odd number of its edges' crossings at or left of
\* them, at the row's top (every edge here crosses rows at whole x)
Edges(f) == [i \in 1..Len(f) |-> <<f[i], f[(i % Len(f)) + 1]>>]
Active(e, y) == e[1][2] # e[2][2] /\ y >= Min(e[1][2], e[2][2]) /\ y < Max(e[1][2], e[2][2])
XAt(e, y) == e[1][1] + ((y - e[1][2]) * (e[2][1] - e[1][1])) \div (e[2][2] - e[1][2])
Interior(f) == {d \in Dev : LET es == Edges(f) IN Cardinality({i \in 1..Len(es) : Active(es[i], d[2]) /\ XAt(es[i], d[2]) <= d[1]}) % 2 = 1}
Corners(r) == <<Dp(<<r[1], r[2]>>), Dp(<<r[3], r[2]>>), Dp(<<r[3], r[4]>>), Dp(<<r[1], r[4]>>)>>

Dump(img) == <<"> dump 0", "px 0 5 4 8">> \o [y \in 1..H |-> H2(img[<<0, y - 1>>]) \o H2(img[<<1, y - 1>>]) \o H2(img[<<2, y - 1>>]) \o H2(img[<<3, y - 1>>]) \o H2(img[<<4, y - 1>>])]
Call(lines, img, p) == px' = img /\ pos' = p /\ log' = log \o lines \o <<"> getpos 0", "pos " \o P(p)>> \o Dump(img) /\ n' = n + 1

Setup(m) == <<"> dib 0 5 4 8", "> brush 0 solid #00ff00">>
            \o (CASE m = "page" -> <<"> mapmode 0 8", "mapmode 1", "> winext 0 2 1", "ext 1 1 1", "> vpext 0 1 1", "ext 1 1 1">>
                  [] m = "rot" -> <<"> gmode 0 2", "gmode 1", "> setxform 0 0 1 -1 0 3 0", "setxform 1">>
                  [] m = "diag" -> <<"> gmode 0 2", "gmode 1", "> setxform 0 1 1 -1 1 2 0", "setxform 1">>)
\* GDI.PEN.CREATE: the DC's own pen is BLACK_PEN
Init == /\ map \in {"page", "rot", "diag"} /\ px = [d \in Dev |-> 0] /\ pen = <<"#000000", 1>> /\ pos = <<0, 0>> /\ n = 0
        /\ log = Setup(map)

SetPen(p) == UNCHANGED map /\ pen' = p /\ Call(<<"> pen 0 0 " \o S(p[2]) \o " " \o p[1]>>, px, pos)
MoveTo(p) == UNCHANGED <<map, pen>> /\ Call(<<"> moveto 0 " \o P(p), "pos " \o P(pos)>>, px, p)
\* the line between the ends' device points, the position staying logical (black draws 0)
LineTo(p) == /\ UNCHANGED <<map, pen>>
             /\ Call(<<"> lineto 0 " \o P(p), "ret 1">>, IF pen[1] = "#000000" THEN [d \in Dev |-> IF d \in Range(LinePx(Dp(pos), Dp(p))) THEN 0 ELSE px[d]] ELSE Draw(px, LinePx(Dp(pos), Dp(p))), p)
\* through the mapping: Rectangle of the device corners (GDI.SHAPE.RECT, RECTTHIN); through the
\* world: the parallelogram filled, then its four lines
Rectangle(r) ==
  LET c == Corners(r)
      outline(img, q) == IF pen[1] = "#000000" THEN [d \in Dev |-> IF d \in Range(q) THEN 0 ELSE img[d]] ELSE Draw(img, q)
      img == IF map = "page"
             THEN LET l == Min(c[1][1], c[3][1]) t == Min(c[1][2], c[3][2]) rr == Max(c[1][1], c[3][1]) b == Max(c[1][2], c[3][2]) IN
                  IF rr - l < 1 \/ b - t < 1 THEN px
                  ELSE outline(Fill(px, Span(l + 1, t + 1, rr - 1, b - 1)), PolyPx(<<<<l, t>>, <<rr - 1, t>>, <<rr - 1, b - 1>>, <<l, b - 1>>, <<l, t>>>>))
             ELSE outline(Fill(px, Interior(c)), PolyPx(c \o <<c[1]>>))
  IN UNCHANGED <<map, pen>> /\ Call(<<"> rectangle 0 " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4]), "ret 1">>, img, pos)
\* PATCOPY: the device rectangle of the corners through the mapping; the parallelogram through
\* the world
PatBlt(r) ==
  LET c == Corners(<<r[1], r[2], r[1] + r[3], r[2] + r[4]>>)
      area == IF map = "page" THEN Span(Min(c[1][1], c[3][1]), Min(c[1][2], c[3][2]), Max(c[1][1], c[3][1]), Max(c[1][2], c[3][2])) ELSE Interior(c)
  IN UNCHANGED <<map, pen>> /\ Call(<<"> patblt 0 " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4]) \o " 0xf00021", "ret 1">>, Fill(px, area), pos)

Rects == CASE map = "page" -> {<<0, 0, 4, 2>>, <<1, 1, 4, 3>>} [] map = "rot" -> {<<0, 0, 2, 2>>} [] map = "diag" -> {<<0, 0, 1, 1>>}
Blts == CASE map = "page" -> {<<0, 0, 4, 2>>, <<1, 0, 3, 1>>} [] map = "rot" -> {<<0, 0, 2, 2>>} [] map = "diag" -> {<<0, 0, 1, 1>>}

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ \E p \in {<<"#ff0000", 0>>, <<"#0000ff", 1>>} : SetPen(p)
           \/ \E p \in Grid : MoveTo(p) \/ LineTo(p)
           \/ \E r \in Rects : Rectangle(r)
           \/ \E r \in Blts : PatBlt(r)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* the position stays a logical point of the grid
TypeOK == pos \in Grid \/ pos = <<0, 0>>
=============================================================================
