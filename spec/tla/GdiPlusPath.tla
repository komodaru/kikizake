---------------------------- MODULE GdiPlusPath ----------------------------
(* GDI+ paths, from spec/gdiplus.txt: GraphicsPath::GraphicsPath, AddLine, AddLines, AddBeziers,
   AddRectangle, AddPolygon, AddCurve, AddClosedCurve, CloseFigure, StartFigure, GetPathPoints,
   GetPathTypes, GetBounds, FillPath and Flatten of a flatness of 0 (GP.PATH.BUILD, GP.PATH.CURVE,
   GP.PATH.BOUNDS, GP.PATH.FILLMODE, GP.PATH.FLATTEN.ZERO). The points are whole and the tensions 0, 1.5 and 3 (T 0, 1/2, 1), so every
   control point is exact in single precision (the floats of GdiPlusMatrix, INSTANCE); a fill is
   asked only of a path of rectangles, aliased with no pixel offset, where Alternate fills the
   pixels an odd number of them cover and Winding (their corners all turning the same way) every
   pixel one covers. Every behavior makes a path of some FillMode, a 4 x 1 bitmap and a solid brush,
   and takes at most MaxOps calls; after each, the path's points, types and bounds are read, and the
   bitmap.
   `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES path, bmp, log, n
vars == <<path, bmp, log, n>>

M == INSTANCE GdiPlusMatrix WITH mat <- [k \in {0, 1} |-> <<>>]
B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
F(k) == M!F(k)
Bits(a) == M!Bits(a)
RECURSIVE CatFrom(_, _)
CatFrom(s, i) == IF i > Len(s) THEN "" ELSE s[i] \o CatFrom(s, i + 1)
Cat(s) == CatFrom(s, 1)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
RECURSIVE Cat2From(_, _)
Cat2From(s, i) == IF i > Len(s) THEN <<>> ELSE s[i] \o Cat2From(s, i + 1)
Cat2(s) == Cat2From(s, 1)

\* a point: x, y (floats), its PathPointType (0 start, 1 line, 3 Bezier) and the close flag
Pt(x, y, t) == [x |-> x, y |-> y, t |-> t, close |-> FALSE]
TypeHex(p) == B!Hex(p.t + (IF p.close THEN 128 ELSE 0), 2)

\* GP.PATH.BOUNDS: the box of every point, its size max - min; zeros for fewer than two points or
\* a box of no size both ways
RECURSIVE SMinFrom(_, _)
SMinFrom(s, i) == IF i = Len(s) THEN s[i] ELSE Min(s[i], SMinFrom(s, i + 1))
RECURSIVE SMaxFrom(_, _)
SMaxFrom(s, i) == IF i = Len(s) THEN s[i] ELSE Max(s[i], SMaxFrom(s, i + 1))
SMin(s) == SMinFrom(s, 1)
SMax(s) == SMaxFrom(s, 1)
Bounds(ps) ==
  IF Len(ps) < 2 THEN "bounds 0 00000000 00000000 00000000 00000000"
  ELSE LET xs == [i \in 1..Len(ps) |-> ps[i].x.v] ys == [i \in 1..Len(ps) |-> ps[i].y.v]
           x0 == SMin(xs) y0 == SMin(ys) w == SMax(xs) - x0 h == SMax(ys) - y0
           \* (the points here are never -0)
           corner(v) == [v |-> v, nz |-> FALSE]
       IN IF w = 0 /\ h = 0 THEN "bounds 0 00000000 00000000 00000000 00000000"
          ELSE "bounds 0 " \o Bits(corner(x0)) \o " " \o Bits(corner(y0)) \o " " \o Bits([v |-> w, nz |-> FALSE])
               \o " " \o Bits([v |-> h, nz |-> FALSE])

Ask(p, b) == LET ps == p.pts IN
  <<"> gppathinfo 0", "pathinfo 0 " \o S(Len(ps))>>
  \o (IF ps = <<>> THEN <<>> ELSE <<"pathpts 0 0" \o Cat([i \in 1..Len(ps) |-> " " \o TypeHex(ps[i]) \o " " \o Bits(ps[i].x) \o " " \o Bits(ps[i].y)])>>)
  \o <<Bounds(ps)>> \o B!Read(0, b[0], "32bppARGB")

Call(lines, p, b) == path' = p /\ bmp' = b /\ log' = log \o lines \o Ask(p, b) /\ n' = n + 1

Fills == {0, 1, 2, -1}
Color == B!Ch(255, 51, 102, 153)
NewPath(fm) == [fill |-> fm, pts |-> <<>>, fresh |-> TRUE, rects |-> <<>>, other |-> FALSE]
Init == \E fm \in Fills :
          /\ path = NewPath(fm)
          /\ bmp = [k \in {0, 1} |-> IF k = 0 THEN B!New("32bppARGB", 4, 1) ELSE B!NoBitmap]
          /\ log = <<"> gpbitmap 0 4 1 32bppARGB", "bitmap 0", "> gpbrush 0 solid 0x" \o B!Hex8(Color), "brush 0",
                     "> gppath 0 " \o S(fm), "path 0">> \o Ask(NewPath(fm), [k \in {0, 1} |-> IF k = 0 THEN B!New("32bppARGB", 4, 1) ELSE B!NoBitmap])
          /\ n = 0

\* GP.PATH.BUILD: new points go on the open figure, the first a line to it, or skipped on its last
\* point; a figure's start otherwise
Join(p, q) ==
  IF ~p.fresh /\ p.pts # <<>> /\ p.pts[Len(p.pts)].x.v = q[1].v /\ p.pts[Len(p.pts)].y.v = q[2].v
  THEN p
  ELSE IF ~p.fresh /\ p.pts # <<>> THEN [p EXCEPT !.pts = Append(@, Pt(q[1], q[2], 1))]
  ELSE [p EXCEPT !.pts = Append(@, Pt(q[1], q[2], 0)), !.fresh = FALSE]
Lines(p, qs) == LET j == Join(p, qs[1]) IN [j EXCEPT !.pts = @ \o [i \in 1..(Len(qs) - 1) |-> Pt(qs[i + 1][1], qs[i + 1][2], 1)], !.other = TRUE]
Beziers(p, qs) == LET j == Join(p, qs[1]) IN [j EXCEPT !.pts = @ \o [i \in 1..(Len(qs) - 1) |-> Pt(qs[i + 1][1], qs[i + 1][2], 3)], !.other = TRUE]
Start(p) == [p EXCEPT !.fresh = TRUE]
Close(p) == [p EXCEPT !.fresh = TRUE, !.pts = IF @ = <<>> THEN @ ELSE [@ EXCEPT ![Len(@)].close = TRUE]]

P(xy) == <<F(xy[1]), F(xy[2])>>
Ps(l) == [i \in 1..Len(l) |-> P(l[i])]
Txt(l) == S(Len(l)) \o Cat([i \in 1..Len(l) |-> " " \o S(l[i][1]) \o " " \o S(l[i][2])])
Add(what, args, ok, p) == Call(<<"> gpadd 0 " \o what \o args, IF ok THEN "add 0" ELSE "add 2">>, IF ok THEN p ELSE path, bmp)

AddLine(a, b) == Add("line", " " \o S(a[1]) \o " " \o S(a[2]) \o " " \o S(b[1]) \o " " \o S(b[2]), TRUE, Lines(path, <<P(a), P(b)>>))
AddLines(l) == Add("lines", " " \o Txt(l), l # <<>>, IF l = <<>> THEN path ELSE Lines(path, Ps(l)))
AddBeziers(l) == Add("beziers", " " \o Txt(l), Len(l) >= 4 /\ (Len(l) - 1) % 3 = 0, IF Len(l) >= 4 THEN Beziers(path, Ps(l)) ELSE path)
\* AddRectangle: a closed figure of its own, the corners from (x, y) east first; nothing for a
\* width or height of 0
AddRect(r) == Add("rect", " " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4]), TRUE,
                  IF r[3] <= 0 \/ r[4] <= 0 THEN path
                  ELSE LET l == Close(Lines(Start(path), <<P(<<r[1], r[2]>>), P(<<r[1] + r[3], r[2]>>), P(<<r[1] + r[3], r[2] + r[4]>>), P(<<r[1], r[2] + r[4]>>)>>))
                       IN [l EXCEPT !.rects = Append(path.rects, r), !.other = path.other])
\* AddPolygon: a closed figure of its own, a last point on the first dropped past three; fewer than 3
\* points InvalidParameter
AddPolygon(l) == Add("polygon", " " \o Txt(l), Len(l) >= 3,
                     IF Len(l) < 3 THEN path
                     ELSE Close(Lines(Start(path), Ps(IF Len(l) > 3 /\ l[Len(l)] = l[1] THEN SubSeq(l, 1, Len(l) - 1) ELSE l))))

\* GP.PATH.CURVE: with T = tension / 3, segment p1 -> p2 of neighbours p0, p3 (clamped at an open
\* curve's ends, around a closed one) has control points (p1 - T p0) + T p2 and (T p1 + p2) - T p3
C1(t, w, k) == M!Add(M!Sub(w[2][k], M!Mul(t, w[1][k])), M!Mul(t, w[3][k]))
C2(t, w, k) == M!Sub(M!Add(M!Mul(t, w[2][k]), w[3][k]), M!Mul(t, w[4][k]))
Spline(ps, t, closed) ==
  LET c == Len(ps)
      at(i) == IF closed THEN ps[(((i % c) + c) % c) + 1] ELSE ps[Max(0, Min(i, c - 1)) + 1]
      w(i) == <<at(i - 1), at(i), at(i + 1), at(i + 2)>>
      segs == IF closed THEN c ELSE c - 1
  IN <<ps[1]>> \o Cat2([i \in 1..segs |-> LET q == w(i - 1) IN <<<<C1(t, q, 1), C1(t, q, 2)>>, <<C2(t, q, 1), C2(t, q, 2)>>, q[3]>>])
Tensions == {<<"0", 0>>, <<"1.5", 32>>, <<"3", 64>>}
AddCurve(tn, l) == Add("curve", " " \o tn[1] \o " " \o Txt(l), Len(l) >= 2,
                       IF Len(l) < 2 THEN path ELSE Beziers(path, Spline(Ps(l), [v |-> tn[2], nz |-> FALSE], FALSE)))
AddClosedCurve(tn, l) == Add("closedcurve", " " \o tn[1] \o " " \o Txt(l), Len(l) >= 3,
                             IF Len(l) < 3 THEN path ELSE Close(Beziers(Start(path), Spline(Ps(l), [v |-> tn[2], nz |-> FALSE], TRUE))))

\* GP.PATH.FILLMODE: FillMode 0 Alternate, every other value Winding; each rectangle's pixels
Covers(r, i) == r[1] <= i /\ i < r[1] + r[3] /\ r[2] <= 0 /\ 0 < r[2] + r[4]
RECURSIVE CountFrom(_, _, _)
CountFrom(rs, i, j) == IF j > Len(rs) THEN 0 ELSE (IF Covers(rs[j], i) THEN 1 ELSE 0) + CountFrom(rs, i, j + 1)
FillPath ==
  LET rs == path.rects
      k(i) == CountFrom(rs, i, 1)
      in(i) == IF path.fill = 0 THEN k(i) % 2 = 1 ELSE k(i) > 0
      b == bmp[0]
  IN Call(<<"> gpfillpath 0 0 0 copy 0 3 3", "fillpath 0 0">>, path,
          [bmp EXCEPT ![0].px = [i \in 1..4 |-> IF in(i - 1) THEN Color ELSE b.px[i]]])

\* GP.PATH.FLATTEN.ZERO: Flatten of a flatness of 0 (and no matrix) makes each figure its start and a
\* line point for each line point and each Bezier segment (the last one closing a closed figure),
\* every point 0 (+0 here, -0 + 0 rounded to nearest: GP.MATRIX.X86); a path of no curves stays
Z(t, c) == [x |-> F(0), y |-> F(0), t |-> t, close |-> c]
RECURSIVE FlatFrom(_, _, _)
FlatFrom(ps, i, run) ==
  IF i > Len(ps) THEN <<>>
  ELSE IF ps[i].t = 3 THEN (IF run % 3 = 2 THEN <<Z(1, ps[i].close)>> ELSE <<>>) \o FlatFrom(ps, i + 1, run + 1)
  ELSE <<Z(ps[i].t, ps[i].close)>> \o FlatFrom(ps, i + 1, 0)
Flatten0 ==
  Call(<<"> gpflat 0 - 0", "flat 0">>,
       IF \E i \in DOMAIN path.pts : path.pts[i].t = 3
       THEN [path EXCEPT !.pts = FlatFrom(path.pts, 1, 0), !.fresh = TRUE, !.other = TRUE]
       ELSE path, bmp)

Next == /\ n < MaxOps
        /\ \/ AddLine(<<0, 0>>, <<2, 1>>) \/ AddLine(<<2, 1>>, <<3, 0>>)
           \/ \E l \in {<<>>, <<<<1, 1>>>>, <<<<0, 1>>, <<2, 0>>, <<2, 1>>>>} : AddLines(l)
           \/ \E l \in {<<<<0, 0>>, <<1, 2>>, <<3, 2>>, <<4, 0>>>>, <<<<0, 0>>, <<1, 2>>, <<3, 2>>>>} : AddBeziers(l)
           \/ \E r \in {<<0, 0, 2, 1>>, <<1, 0, 2, 1>>, <<3, 0, 0, 1>>} : AddRect(r)
           \/ \E l \in {<<<<0, 0>>, <<2, 0>>>>, <<<<0, 0>>, <<2, 0>>, <<2, 2>>, <<0, 0>>>>} : AddPolygon(l)
           \/ \E tn \in Tensions : \/ AddCurve(tn, <<<<0, 0>>, <<2, 4>>, <<4, 2>>>>)
                                   \/ AddClosedCurve(tn, <<<<0, 0>>, <<4, 2>>, <<2, 4>>>>)
           \/ AddCurve(<<"1.5", 32>>, <<<<1, 1>>>>) \/ AddClosedCurve(<<"3", 64>>, <<<<0, 0>>, <<1, 1>>>>)
           \/ Call(<<"> gpadd 0 close", "add 0">>, Close(path), bmp)
           \/ Call(<<"> gpadd 0 start", "add 0">>, Start(path), bmp)
           \/ (~path.other /\ FillPath)
           \/ Flatten0

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* GP.PATH.BUILD: a path's first point starts a figure
Starts == path.pts # <<>> => path.pts[1].t = 0
=============================================================================
