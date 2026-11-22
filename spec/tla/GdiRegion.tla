---------------------------- MODULE GdiRegion ----------------------------
(* GDI regions, from spec/gdirgn.txt: CreateRectRgn, SetRectRgn, CombineRgn, OffsetRgn, GetRgnBox,
   PtInRegion, RectInRegion, EqualRgn (the GDI.RGN clauses below). A region is the set of its
   pixels; what the documentation says of it is said of that set: CombineRgn's modes are the set
   operations, its type is NULLREGION for no pixel, SIMPLEREGION for one rectangle, COMPLEXREGION
   otherwise. RectInRegion (GDI.RGN.OVERLAP) is said of the region's rectangles: each pixel's run
   in its row, over the rows around it with the same runs (bands, equal neighbours merged).
   Every behavior starts with two regions and takes at most MaxOps calls that change one; after
   each, every query is asked of both, so TLC's states hold every answer. `kikizake tla` replays
   every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

\* the rectangles made (one with its corners swapped, one empty), the offsets, the points and the
\* rectangles asked (zero wide, zero high, a point, a cover, swapped corners)
Rects == {<<0, 0, 2, 2>>, <<3, 1, 1, 0>>, <<1, 1, 1, 3>>}
Offsets == {<<1, 0>>, <<-1, -2>>}
Points == <<<<0, 0>>, <<1, 1>>, <<2, 0>>, <<-1, 0>>>>
Probes == <<<<1, 0, 1, 2>>, <<0, 1, 2, 1>>, <<1, 1, 1, 1>>, <<0, 0, 4, 4>>, <<2, 2, 0, 1>>>>

VARIABLES rgn, log, n
vars == <<rgn, log, n>>

Slots == {0, 1}
S(i) == ToString(i)
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a

\* GDI.RGN.RECT, GDI.RGN.SETRECT: the corners in either order, right and bottom excluded
Pixels(r) == {<<x, y>> : x \in Min(r[1], r[3])..(Max(r[1], r[3]) - 1), y \in Min(r[2], r[4])..(Max(r[2], r[4]) - 1)}

Xs(P) == {p[1] : p \in P}
Ys(P) == {p[2] : p \in P}
SetMin(X) == CHOOSE m \in X : \A k \in X : m <= k
SetMax(X) == CHOOSE m \in X : \A k \in X : m >= k
Row(P, y) == {p[1] : p \in {q \in P : q[2] = y}}
\* the run of row X through x: [left, right)
RunL(X, x) == CHOOSE l \in X : l <= x /\ (\A k \in l..x : k \in X) /\ (l - 1) \notin X
RunR(X, x) == (CHOOSE r \in X : r >= x /\ (\A k \in x..r : k \in X) /\ (r + 1) \notin X) + 1
\* the band of row y: the rows around it with the same runs, [top, bottom)
BandT(P, y) == CHOOSE t \in Ys(P) : t <= y /\ (\A k \in t..y : Row(P, k) = Row(P, y)) /\ Row(P, t - 1) # Row(P, y)
BandB(P, y) == (CHOOSE b \in Ys(P) : b >= y /\ (\A k \in y..b : Row(P, k) = Row(P, y)) /\ Row(P, b + 1) # Row(P, y)) + 1
\* the region's rectangles (left, top, right, bottom)
RectsOf(P) == {<<RunL(Row(P, p[2]), p[1]), BandT(P, p[2]), RunR(Row(P, p[2]), p[1]), BandB(P, p[2])>> : p \in P}

\* GDI.RGN.KIND
Kind(P) == IF P = {} THEN 1 ELSE IF Cardinality(RectsOf(P)) = 1 THEN 2 ELSE 3
\* GDI.RGN.BOX: the type and the bounding rectangle, 0 0 0 0 for no region
Box(P) == IF P = {} THEN <<0, 0, 0, 0>> ELSE <<SetMin(Xs(P)), SetMin(Ys(P)), SetMax(Xs(P)) + 1, SetMax(Ys(P)) + 1>>
\* GDI.RGN.OVERLAP: the rectangle normalized; some rectangle of the region overlaps it
Overlap(P, q) == \E x \in RectsOf(P) : Min(q[1], q[3]) < x[3] /\ Max(q[1], q[3]) > x[1] /\ Min(q[2], q[4]) < x[4] /\ Max(q[2], q[4]) > x[2]

\* the queries asked after every call: GDI.RGN.BOX, PTIN, RECTIN, EQUAL
Ask(g) == LET one(k) == <<"> rgnbox " \o S(k), "box " \o S(Kind(g[k])) \o " " \o R(Box(g[k]))>>
                        \o [i \in 1..(2 * Len(Points)) |-> IF i % 2 = 1
                                THEN "> ptin " \o S(k) \o " " \o S(Points[(i + 1) \div 2][1]) \o " " \o S(Points[(i + 1) \div 2][2])
                                ELSE "ptin " \o (IF Points[i \div 2] \in g[k] THEN "1" ELSE "0")]
                        \o [i \in 1..(2 * Len(Probes)) |-> IF i % 2 = 1
                                THEN "> rectin " \o S(k) \o " " \o R(Probes[(i + 1) \div 2])
                                ELSE "rectin " \o (IF Overlap(g[k], Probes[i \div 2]) THEN "1" ELSE "0")]
          IN one(0) \o one(1) \o <<"> equal 0 1", "equal " \o (IF g[0] = g[1] THEN "1" ELSE "0")>>

Call(lines, g) == rgn' = g /\ log' = log \o lines \o Ask(g) /\ n' = n + 1

Init == \E a, b \in Rects :
          /\ rgn = [k \in Slots |-> IF k = 0 THEN Pixels(a) ELSE Pixels(b)]
          /\ log = <<"> rgn 0 rect " \o R(a), "> rgn 1 rect " \o R(b)>>
                   \o Ask([k \in Slots |-> IF k = 0 THEN Pixels(a) ELSE Pixels(b)])
          /\ n = 0

\* GDI.RGN.RECT: a new rectangle region in a slot
NewRect(k, r) == Call(<<"> rgn " \o S(k) \o " rect " \o R(r)>>, [rgn EXCEPT ![k] = Pixels(r)])

\* GDI.RGN.SETRECT
SetRect(k, r) == Call(<<"> setrect " \o S(k) \o " " \o R(r), "setrect 1">>, [rgn EXCEPT ![k] = Pixels(r)])

\* GDI.RGN.COMBINE (1 AND, 2 OR, 3 XOR, 4 DIFF), GDI.RGN.COPY (5), GDI.RGN.BADMODE (others: ERROR,
\* the destination as it was), the result's type GDI.RGN.KIND
Combined(a, b, m) == CASE m = 1 -> a \cap b [] m = 2 -> a \cup b [] m = 3 -> (a \cup b) \ (a \cap b)
                       [] m = 4 -> a \ b [] m = 5 -> a
Combine(d, a, b, m) ==
  LET line == "> combine " \o S(d) \o " " \o S(a) \o " " \o S(b) \o " " \o S(m) IN
  IF m \in 1..5
  THEN LET g == [rgn EXCEPT ![d] = Combined(rgn[a], rgn[b], m)] IN Call(<<line, "combine " \o S(Kind(g[d]))>>, g)
  ELSE Call(<<line, "combine 0">>, rgn)

\* GDI.RGN.OFFSET: moved; its type
Offset(k, o) == LET p == {<<q[1] + o[1], q[2] + o[2]>> : q \in rgn[k]} IN
                Call(<<"> offsetrgn " \o S(k) \o " " \o S(o[1]) \o " " \o S(o[2]), "offset " \o S(Kind(p))>>, [rgn EXCEPT ![k] = p])

Next == \/ \E k \in Slots, r \in Rects : NewRect(k, r) \/ SetRect(k, r)
        \/ \E d \in Slots, m \in {0, 1, 2, 3, 4, 5, 6} : Combine(d, d, 1 - d, m)
        \/ \E k \in Slots, o \in Offsets : Offset(k, o)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* the documented facts the answers rest on
\* a region is the union of its rectangles, which do not overlap
RectsCover == \A k \in Slots : rgn[k] = UNION {Pixels(x) : x \in RectsOf(rgn[k])}
RectsApart == \A k \in Slots : \A x, y \in RectsOf(rgn[k]) : x # y => Pixels(x) \cap Pixels(y) = {}
\* the box holds the region
BoxHolds == \A k \in Slots : rgn[k] \subseteq Pixels(Box(rgn[k]))
\* SIMPLEREGION is exactly a region that is its box
SimpleIsBox == \A k \in Slots : Kind(rgn[k]) = 2 <=> (rgn[k] # {} /\ rgn[k] = Pixels(Box(rgn[k])))
=============================================================================
