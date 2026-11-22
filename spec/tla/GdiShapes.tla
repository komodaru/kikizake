---------------------------- MODULE GdiShapes ----------------------------
(* Lines, rectangles and regions on a 4 by 2 8 bpp DIB section (its default color table), from
   spec/gdiline.txt and spec/gdirgn.txt: the DC's default pen and CreatePen's (GDI.PEN.CREATE),
   SetROP2 with 0 (GDI.ROP2.ZERO), LineTo, Polyline and PolylineTo (GDI.LINE.POLY, POLYSHORT),
   FillRect, FrameRect, InvertRect (GDI.RECT.FILL, FRAME, INVERT, EMPTY), Rectangle
   (GDI.SHAPE.RECT, RECTTHIN), FillRgn, PaintRgn, InvertRgn (GDI.RGN.FILL, PAINT, INVERT). The
   points are those of a 3 by 3 grid two pixels apart, so every line is level, upright or
   diagonal and its pixels are the ones it steps through from its first point, its last not
   drawn (GDI.LINE.PIXELS, GDI.LINE.LAST); the colors are the color table's own, so a pixel is
   its entry's index and every binary raster operation the truth table (R2 code - 1) on each of
   its 8 bits (GDI.ROP2.TRUTH). Every behavior takes at most MaxOps calls and asks for the
   current position and the pixels after each; `kikizake tla` replays every state's log through
   the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 4
H == 2
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES px, pen, brush, rop2, pos, log, n
vars == <<px, pen, brush, rop2, pos, log, n>>

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Abs(a) == IF a < 0 THEN -a ELSE a
Sg(a) == IF a < 0 THEN -1 ELSE IF a > 0 THEN 1 ELSE 0
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
Bit(v, k) == (v \div (2 ^ k)) % 2

\* a binary raster operation on 8 bits: result bit = bit (P<<1 | D) of code - 1 (0 - 1: every bit)
RECURSIVE Op2(_, _, _, _)
Op2(t, p, d, k) == IF k = 0 THEN 0 ELSE Bit(t, 2 * (p % 2) + (d % 2)) + 2 * Op2(t, p \div 2, d \div 2, k - 1)
Rop2(code, p, d) == Op2((code - 1) % 16, p, d, 8)

\* colors of the default 8 bpp table and their entries: (r >> 5) << 5 | (g >> 5) << 2 | b >> 6
Colors == [c \in {"#000000", "#ffffff", "#ff0000", "#00ff00", "#0000ff"} |->
             CASE c = "#000000" -> 0 [] c = "#ffffff" -> 255 [] c = "#ff0000" -> 224 [] c = "#00ff00" -> 28 [] c = "#0000ff" -> 3]

\* a rectangle's pixels, left and top included, right and bottom not (none when out of order)
Span(l, t, r, b) == (l..(r - 1)) \X (t..(b - 1))

\* a line's pixels in order, from a, the last point not drawn
LinePx(a, b) == LET dx == b[1] - a[1]
                    dy == b[2] - a[2]
                IN [k \in 1..Max(Abs(dx), Abs(dy)) |-> <<a[1] + (k - 1) * Sg(dx), a[2] + (k - 1) * Sg(dy)>>]
RECURSIVE PolyPx(_)
PolyPx(s) == IF Len(s) < 2 THEN <<>> ELSE LinePx(s[1], s[2]) \o PolyPx(Tail(s))

\* the pen's index through ROP2 on each pixel of q in turn, those in the bitmap
RECURSIVE Plot(_, _, _)
Plot(img, q, v) == IF q = <<>> THEN img
                   ELSE Plot(IF Head(q) \in Dev THEN [img EXCEPT ![Head(q)] = Rop2(rop2, v, img[Head(q)])] ELSE img, Tail(q), v)
Draw(img, q) == IF pen[1] = "null" THEN img ELSE Plot(img, q, Colors[pen[1]])
\* the brush's index through code on every pixel of the set
Fill(img, A, code) == [q \in Dev |-> IF q \in A THEN Rop2(code, Colors[brush], img[q]) ELSE img[q]]

Dump(img, p) == <<"> getpos 0", "pos " \o P(p), "> dump 0", "px 0 4 2 8">>
                \o [y \in 1..H |-> H2(img[<<0, y - 1>>]) \o H2(img[<<1, y - 1>>]) \o H2(img[<<2, y - 1>>]) \o H2(img[<<3, y - 1>>])]
Call(lines, img, p) == px' = img /\ pos' = p /\ log' = log \o lines \o Dump(img, p) /\ n' = n + 1

\* the regions in slots 1 (a rectangle) and 2 (two rectangles touching at a corner)
Rgn1 == Span(1, 0, 3, 2)
Rgn2 == Span(0, 0, 1, 1) \cup Span(2, 1, 4, 2)

\* GDI.PEN.CREATE: a new DC's pen is BLACK_PEN; its brush WHITE_BRUSH, ROP2 R2_COPYPEN
Init == /\ px = [q \in Dev |-> 0] /\ pen = <<"#000000", 1>> /\ brush = "#ffffff" /\ rop2 = 13 /\ pos = <<0, 0>>
        /\ log = <<"> dib 0 4 2 8", "> rgn 1 rect 1 0 3 2", "> rgn 2 rect 0 0 1 1", "> rgn 3 rect 2 1 4 2",
                   "> combine 2 2 3 2", "combine 3">>
        /\ n = 0

\* GDI.PEN.CREATE: a solid pen of width 0 or 1 draws one pixel wide in its color
SetPen(p) == /\ pen' = p /\ UNCHANGED <<brush, rop2>>
             /\ Call(<<IF p[1] = "null" THEN "> pen 0 null" ELSE "> pen 0 0 " \o S(p[2]) \o " " \o p[1]>>, px, pos)
SetBrush(c) == /\ brush' = c /\ UNCHANGED <<pen, rop2>> /\ Call(<<"> brush 0 solid " \o c>>, px, pos)
\* GDI.STATE.OLD, GDI.ROP2.ZERO: the previous mode; 0 taken, 17 refused (0)
SetRop2(v) == /\ rop2' = (IF v \in 0..16 THEN v ELSE rop2) /\ UNCHANGED <<pen, brush>>
              /\ Call(<<"> rop2 0 " \o S(v), "rop2 " \o (IF v \in 0..16 THEN S(rop2) ELSE "0")>>, px, pos)

MoveTo(p) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> moveto 0 " \o P(p), "pos " \o P(pos)>>, px, p)
LineTo(p) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> lineto 0 " \o P(p), "ret 1">>, Draw(px, LinePx(pos, p)), p)
\* GDI.LINE.POLY: Polyline neither uses nor moves the current position; PolylineTo starts at it
\* and leaves it at the last point; GDI.LINE.POLYSHORT: one point fails, drawing nothing
Pts(s) == IF s = <<>> THEN "" ELSE " " \o P(s[1]) \o (IF Len(s) > 1 THEN " " \o P(s[2]) ELSE "") \o (IF Len(s) > 2 THEN " " \o P(s[3]) ELSE "")
Polyline(s) == /\ UNCHANGED <<pen, brush, rop2>>
               /\ IF Len(s) < 2 THEN Call(<<"> polyline 0" \o Pts(s), "ret 0">>, px, pos)
                  ELSE Call(<<"> polyline 0" \o Pts(s), "ret 1">>, Draw(px, PolyPx(s)), pos)
PolylineTo(s) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> polylineto 0" \o Pts(s), "ret 1">>, Draw(px, PolyPx(<<pos>> \o s)), s[Len(s)])

\* GDI.RECT.FILL, INVERT, EMPTY: the corners in either order, right column and bottom row
\* excluded, the brush copied (whatever ROP2) or every bit inverted
Norm(r) == Span(Min(r[1], r[3]), Min(r[2], r[4]), Max(r[1], r[3]), Max(r[2], r[4]))
FillRect(r) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> fillrect 0 " \o R(r), "ret 1">>, Fill(px, Norm(r), 13), pos)
InvertRect(r) == UNCHANGED <<pen, brush, rop2>>
                 /\ Call(<<"> invertrect 0 " \o R(r), "ret 1">>, [q \in Dev |-> IF q \in Norm(r) THEN 255 - px[q] ELSE px[q]], pos)
\* GDI.RECT.FRAME, EMPTY: a border one unit wide, its four edges as given (top row t, bottom row
\* b - 1, left column l, right column r - 1); corners out of order fail
FrameRect(r) == /\ UNCHANGED <<pen, brush, rop2>>
                /\ IF r[3] < r[1] \/ r[4] < r[2] THEN Call(<<"> framerect 0 " \o R(r), "ret 0">>, px, pos)
                   ELSE LET l == r[1] t == r[2] rr == r[3] b == r[4]
                        IN Call(<<"> framerect 0 " \o R(r), "ret 1">>,
                                Fill(px, Span(l, t, rr, t + 1) \cup Span(l, b - 1, rr, b) \cup Span(l, t, l + 1, b) \cup Span(rr - 1, t, rr, b), 13), pos)

\* GDI.SHAPE.RECT, RECTTHIN: corners in either order; with a pen the brush over the inside
\* through ROP2, then the outline (l, t) -> (r - 1, t) -> (r - 1, b - 1) -> (l, b - 1) -> (l, t)
\* as four lines without their last pixels; an empty one draws nothing; with the null pen the
\* brush over (l, t) - (r - 1, b - 1)
Rectangle(r) ==
  LET l == Min(r[1], r[3]) t == Min(r[2], r[4]) rr == Max(r[1], r[3]) b == Max(r[2], r[4])
      img == IF pen[1] = "null" THEN Fill(px, Span(l, t, rr - 1, b - 1), rop2)
             ELSE IF rr - l < 1 \/ b - t < 1 THEN px
             ELSE Draw(Fill(px, Span(l + 1, t + 1, rr - 1, b - 1), rop2), PolyPx(<<<<l, t>>, <<rr - 1, t>>, <<rr - 1, b - 1>>, <<l, b - 1>>, <<l, t>>>>))
  IN UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> rectangle 0 " \o R(r), "ret 1">>, img, pos)

\* GDI.RGN.FILL: the brush over the region; GDI.RGN.PAINT: through ROP2; GDI.RGN.INVERT
RgnOf(k) == IF k = 1 THEN Rgn1 ELSE Rgn2
FillRgn(k) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> fillrgn 0 " \o S(k), "ret 1">>, Fill(px, RgnOf(k), 13), pos)
PaintRgn(k) == UNCHANGED <<pen, brush, rop2>> /\ Call(<<"> paintrgn 0 " \o S(k), "ret 1">>, Fill(px, RgnOf(k), rop2), pos)
InvertRgn(k) == UNCHANGED <<pen, brush, rop2>>
                /\ Call(<<"> invertrgn 0 " \o S(k), "ret 1">>, [q \in Dev |-> IF q \in RgnOf(k) THEN 255 - px[q] ELSE px[q]], pos)

Grid == {0, 2} \X {0, 2}
Rects == {<<0, 0, 3, 2>>, <<3, 2, 1, 0>>, <<1, 0, 1, 2>>, <<0, 1, 4, 1>>, <<1, 1, 2, 2>>}

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ \E p \in {<<"#ff0000", 0>>, <<"#0000ff", 1>>, <<"null", 0>>} : SetPen(p)
           \/ \E c \in {"#00ff00", "#ffffff"} : SetBrush(c)
           \/ \E v \in {13, 7, 6, 0, 17} : SetRop2(v)
           \/ \E p \in Grid : MoveTo(p) \/ LineTo(p)
           \/ \E s \in {<<<<2, 0>>>>, <<<<0, 0>>, <<2, 2>>, <<2, 0>>>>, <<<<2, 2>>, <<0, 2>>>>} : Polyline(s)
           \/ \E s \in {<<<<2, 2>>>>, <<<<0, 2>>, <<2, 0>>>>} : PolylineTo(s)
           \/ \E r \in Rects : FillRect(r) \/ InvertRect(r) \/ FrameRect(r) \/ Rectangle(r)
           \/ \E k \in {1, 2} : FillRgn(k) \/ PaintRgn(k) \/ InvertRgn(k)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == \A q \in Dev : px[q] \in 0..255
\* Polyline leaves the position where MoveToEx, LineTo or PolylineTo put it: on the grid
PosOnGrid == pos \in Grid
=============================================================================
