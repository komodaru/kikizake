---------------------------- MODULE GdiClip ----------------------------
(* A device context's clip region, from spec/gdi.txt and spec/gdirgn.txt: IntersectClipRect,
   ExcludeClipRect, SelectClipRgn / ExtSelectClipRgn, OffsetClipRgn, GetClipBox, GetClipRgn,
   PtVisible, RectVisible (the GDI.CLIP clauses below), on a 6 by 4 bitmap. The clip region is
   the set of its pixels, or none; a region is said of its pixels as in GdiRegion.tla (its
   type, its box, its rectangles in bands). Every behavior selects two rectangle regions into
   region slots and takes at most MaxOps calls that change the clip; after each, every query
   is asked, so the log holds every answer; `kikizake tla` replays every state's log through
   the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

\* the bitmap, the region slots' rectangles, the rectangles intersected and excluded (one with
\* its corners swapped, one empty, one past the bitmap), the offsets, the points and rectangles
\* asked
W == 6
H == 4
Slot1 == <<0, 0, 3, 2>>
Slot2 == <<2, 1, 5, 4>>
Rects == {<<1, 1, 4, 3>>, <<4, 3, 2, 0>>, <<2, 0, 2, 3>>, <<-1, -1, 3, 2>>}
Offsets == {<<1, 0>>, <<-1, -2>>}
Modes == {0, 1, 2, 3, 4, 5}
Points == <<<<0, 0>>, <<3, 2>>, <<5, 3>>, <<6, 0>>, <<-1, 1>>>>
Probes == <<<<0, 0, 6, 4>>, <<4, 2, 6, 4>>, <<1, 1, 1, 1>>, <<5, 3, 3, 1>>>>

\* the clip region: whether there is one, and its pixels
NONE == <<FALSE, {}>>
Some(P) == <<TRUE, P>>
VARIABLES clip, log, n
vars == <<clip, log, n>>

S(i) == ToString(i)
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a

\* a rectangle's pixels: the corners in either order, right and bottom excluded
\* (GDI.CLIP.UNORDERED, GDI.CLIP.EXUNORDERED)
Pixels(r) == {<<x, y>> : x \in Min(r[1], r[3])..(Max(r[1], r[3]) - 1), y \in Min(r[2], r[4])..(Max(r[2], r[4]) - 1)}
Dev == Pixels(<<0, 0, W, H>>)

Xs(P) == {p[1] : p \in P}
Ys(P) == {p[2] : p \in P}
SetMin(X) == CHOOSE m \in X : \A k \in X : m <= k
SetMax(X) == CHOOSE m \in X : \A k \in X : m >= k
Row(P, y) == {p[1] : p \in {q \in P : q[2] = y}}
RunL(X, x) == CHOOSE l \in X : l <= x /\ (\A k \in l..x : k \in X) /\ (l - 1) \notin X
RunR(X, x) == (CHOOSE r \in X : r >= x /\ (\A k \in x..r : k \in X) /\ (r + 1) \notin X) + 1
BandT(P, y) == CHOOSE t \in Ys(P) : t <= y /\ (\A k \in t..y : Row(P, k) = Row(P, y)) /\ Row(P, t - 1) # Row(P, y)
BandB(P, y) == (CHOOSE b \in Ys(P) : b >= y /\ (\A k \in y..b : Row(P, k) = Row(P, y)) /\ Row(P, b + 1) # Row(P, y)) + 1
RectsOf(P) == {<<RunL(Row(P, p[2]), p[1]), BandT(P, p[2]), RunR(Row(P, p[2]), p[1]), BandB(P, p[2])>> : p \in P}
\* NULLREGION, SIMPLEREGION, COMPLEXREGION
Kind(P) == IF P = {} THEN 1 ELSE IF Cardinality(RectsOf(P)) = 1 THEN 2 ELSE 3
Box(P) == IF P = {} THEN <<0, 0, 0, 0>> ELSE <<SetMin(Xs(P)), SetMin(Ys(P)), SetMax(Xs(P)) + 1, SetMax(Ys(P)) + 1>>
Overlap(P, q) == \E x \in RectsOf(P) : Min(q[1], q[3]) < x[3] /\ Max(q[1], q[3]) > x[1] /\ Min(q[2], q[4]) < x[4] /\ Max(q[2], q[4]) > x[2]

\* the visible area: the clip region within the bitmap (all of it without one)
Visible(c) == IF c[1] THEN c[2] \cap Dev ELSE Dev

\* GDI.CLIP.BOX, GETRGN, PTVISIBLE, RECTVISIBLE: asked after every call
Ask(c) == <<"> clipbox 0", "clipbox " \o S(Kind(Visible(c))) \o " " \o R(Box(Visible(c))),
            "> getcliprgn 0 7", "getcliprgn " \o (IF c[1] THEN "1" ELSE "0")>>
          \o [i \in 1..(2 * Len(Points)) |-> IF i % 2 = 1
                THEN "> ptvisible 0 " \o S(Points[(i + 1) \div 2][1]) \o " " \o S(Points[(i + 1) \div 2][2])
                ELSE "ptvisible " \o (IF Points[i \div 2] \in Visible(c) THEN "1" ELSE "0")]
          \o [i \in 1..(2 * Len(Probes)) |-> IF i % 2 = 1
                THEN "> rectvisible 0 " \o R(Probes[(i + 1) \div 2])
                ELSE "rectvisible " \o (IF Overlap(Visible(c), Probes[i \div 2]) THEN "1" ELSE "0")]

Call(lines, c) == clip' = c /\ log' = log \o lines \o Ask(c) /\ n' = n + 1

Init == /\ clip = NONE
        /\ log = <<"> dib 0 6 4 32", "> rgn 1 rect " \o R(Slot1), "> rgn 2 rect " \o R(Slot2)>> \o Ask(NONE)
        /\ n = 0

\* GDI.CLIP.INTERSECT, RESULT, AGAIN: without a clip region the rectangle becomes it,
\* COMPLEXREGION; with one, their intersection, COMPLEXREGION, but NULLREGION when that is empty
\* because the rectangle is or the clip region was not one rectangle
Intersect(r) ==
  LET q == Pixels(r)
      v == IF clip[1] THEN clip[2] \cap q ELSE q
      ret == IF clip[1] /\ v = {} /\ (Kind(clip[2]) # 2 \/ q = {}) THEN 1 ELSE 3
  IN Call(<<"> clip 0 " \o R(r), "clip " \o S(ret)>>, Some(v))

\* GDI.CLIP.EXCLUDE, EXRESULT: the clip region (or the bitmap) minus the rectangle; NULLREGION
\* when that is empty, else COMPLEXREGION
Exclude(r) ==
  LET v == (IF clip[1] THEN clip[2] ELSE Dev) \ Pixels(r)
  IN Call(<<"> exclude 0 " \o R(r), "exclude " \o (IF v = {} THEN "1" ELSE "3")>>, Some(v))

\* GDI.CLIP.SELCOPY, SELMODE, SELNOCLIP, SELNOCLIPAND: RGN_COPY a copy of the region; the other
\* modes (1 AND, 2 OR, 3 XOR, 4 DIFF) combine it with the clip region, without one RGN_AND takes
\* the region, the others combine it with the bitmap; a mode outside them ERROR, the clip as it
\* was (GDI.CLIP.SELBADMODE); the answer is the type of the visible area
Combined(a, b, m) == CASE m = 1 -> a \cap b [] m = 2 -> a \cup b [] m = 3 -> (a \cup b) \ (a \cap b) [] m = 4 -> a \ b
Select(k, m) ==
  LET g == IF k = 1 THEN Pixels(Slot1) ELSE Pixels(Slot2)
      v == Some(IF m = 5 THEN g ELSE IF clip[1] THEN Combined(clip[2], g, m) ELSE IF m = 1 THEN g ELSE Combined(Dev, g, m))
      line == "> selclip 0 " \o S(k) \o " " \o S(m)
  IN IF m \in 1..5 THEN Call(<<line, "selclip " \o S(Kind(Visible(v)))>>, v) ELSE Call(<<line, "selclip 0">>, clip)

\* GDI.CLIP.SELNONE, SELNULLMODE: a NULL region with RGN_COPY removes the clip region; with
\* another mode ERROR, the clip region kept
SelectNone(m) ==
  IF m = 5 THEN Call(<<"> selclip 0 none 5", "selclip " \o S(Kind(Dev))>>, NONE)
  ELSE Call(<<"> selclip 0 none " \o S(m), "selclip 0">>, clip)

\* GDI.CLIP.OFFSET, OFFSETNONE: the clip region moved, its own type; without one SIMPLEREGION,
\* still none
Offset(o) ==
  LET line == "> offsetclip 0 " \o S(o[1]) \o " " \o S(o[2]) IN
  IF ~clip[1] THEN Call(<<line, "offsetclip 2">>, NONE)
  ELSE LET v == {<<p[1] + o[1], p[2] + o[2]>> : p \in clip[2]} IN Call(<<line, "offsetclip " \o S(Kind(v))>>, Some(v))

Next == \/ \E r \in Rects : Intersect(r) \/ Exclude(r)
        \/ \E k \in {1, 2}, m \in Modes : Select(k, m)
        \/ \E m \in {2, 5} : SelectNone(m)
        \/ \E o \in Offsets : Offset(o)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* the visible area is within the bitmap, and inside the clip region when there is one
VisibleInside == Visible(clip) \subseteq Dev /\ (clip[1] => Visible(clip) \subseteq clip[2])
=============================================================================
