---------------------------- MODULE GdiPathFill ----------------------------
(* What GDI does with a path, from spec/gdipath.txt, on a 3 by 3 8 bpp DIB section with a red
   pen and a green brush: FlattenPath of lines (GDI.PATH.FLATTEN), StrokePath (GDI.PATH.STROKE),
   FillPath, StrokeAndFillPath, PathToRegion and SelectClipPath (GDI.PATH.FILL), without a closed
   path (GDI.PATH.NOPATH) or with an empty one (GDI.PATH.EMPTYFILL), and Polygon outside a path
   (GDI.SHAPE.POLYFILL). A path is built in the
   bracket as GdiPath.tla says, of whole figures on the corners of a two pixel grid, so every
   edge is level, upright or diagonal: the interior is CreatePolygonRgn's in ALTERNATE mode
   (GDI.RGN.POLYSCAN), the pixels of each row y between the x where its edges (y in their
   [upper, lower) end) cross it, paired in x order: a pixel lies inside when an odd number of
   those x are at or left of it. Every behavior takes at most MaxOps calls and dumps the bitmap
   after each; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

W == 3
H == 3
Dev == (0..(W - 1)) \X (0..(H - 1))

VARIABLES px, clip, bracket, path, pos, rgn, log, n
vars == <<px, clip, bracket, path, pos, rgn, log, n>>

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
Max(a, b) == IF a < b THEN b ELSE a
Abs(a) == IF a < 0 THEN -a ELSE a
Sg(a) == IF a < 0 THEN -1 ELSE IF a > 0 THEN 1 ELSE 0
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]

\* the pen's and the brush's entries of the default 8 bpp color table
Pen == 224
Brush == 28

PT_CLOSEFIGURE == 1
PT_LINETO == 2
PT_MOVETO == 6

\* a path's figures: each a sequence of points, and whether it was closed
RECURSIVE Figs(_, _)
Figs(p, acc) == IF p = <<>> THEN acc
                ELSE IF Head(p)[3] = PT_MOVETO THEN Figs(Tail(p), Append(acc, <<<<Head(p)[1], Head(p)[2]>>>>))
                ELSE Figs(Tail(p), [acc EXCEPT ![Len(acc)] = Append(acc[Len(acc)], <<Head(p)[1], Head(p)[2]>>)])
Closed(p) == [i \in 1..Len(Figs(p, <<>>)) |-> LET ends == {j \in 1..Len(p) : p[j][3] = PT_MOVETO} IN
                \E j \in 1..Len(p) : p[j][3] = PT_LINETO + PT_CLOSEFIGURE /\ Cardinality({e \in ends : e <= j}) = i]

\* a figure closed: its edges, the last back to the first
Edges(f) == [i \in 1..Len(f) |-> <<f[i], f[(i % Len(f)) + 1]>>]
RECURSIVE AllEdges(_)
AllEdges(fs) == IF fs = <<>> THEN <<>> ELSE Edges(Head(fs)) \o AllEdges(Tail(fs))
\* an edge's x at row y, when the row is in its [upper, lower) span
Active(e, y) == e[1][2] # e[2][2] /\ y >= (IF e[1][2] < e[2][2] THEN e[1][2] ELSE e[2][2]) /\ y < Max(e[1][2], e[2][2])
XAt(e, y) == e[1][1] + ((y - e[1][2]) * (e[2][1] - e[1][1])) \div (e[2][2] - e[1][2])
Inside(es, q) == Cardinality({i \in 1..Len(es) : Active(es[i], q[2]) /\ XAt(es[i], q[2]) <= q[1]}) % 2 = 1
Interior(fs) == {q \in Dev : Inside(AllEdges(fs), q)}

\* a line's pixels from a, its last point not drawn (GDI.LINE.PIXELS on these slopes)
LinePx(a, b) == LET dx == b[1] - a[1]
                    dy == b[2] - a[2]
                IN {<<a[1] + k * Sg(dx), a[2] + k * Sg(dy)>> : k \in 0..(Max(Abs(dx), Abs(dy)) - 1)}
Visible(c) == c \cap Dev
\* the pen copied over the lines' pixels (R2_COPYPEN: their order does not matter)
Lines(img, ls, c) == LET on == (UNION {LinePx(l[1], l[2]) : l \in ls}) \cap Visible(c) IN [q \in Dev |-> IF q \in on THEN Pen ELSE img[q]]
Fill(img, A, c) == [q \in Dev |-> IF q \in A \cap Visible(c) THEN Brush ELSE img[q]]
\* StrokePath's lines: each figure's, its closing one when closed
StrokeLines(p) == LET fs == Figs(p, <<>>) cl == Closed(p) IN
                  (UNION {{<<fs[i][j], fs[i][j + 1]>> : j \in 1..(Len(fs[i]) - 1)} : i \in 1..Len(fs)}) \cup {<<fs[i][Len(fs[i])], fs[i][1]>> : i \in {k \in 1..Len(fs) : cl[k]}}
OutlineLines(p) == LET fs == Figs(p, <<>>) IN UNION {{Edges(fs[i])[j] : j \in 1..Len(fs[i])} : i \in 1..Len(fs)}

Dump(img) == <<"> dump 0", "px 0 3 3 8">> \o [y \in 1..H |-> H2(img[<<0, y - 1>>]) \o H2(img[<<1, y - 1>>]) \o H2(img[<<2, y - 1>>])]
Call(lines, img) == px' = img /\ log' = log \o lines \o Dump(img) /\ n' = n + 1

Init == /\ px = [q \in Dev |-> 0] /\ clip = Dev /\ bracket = "none" /\ path = <<>> /\ pos = <<0, 0>> /\ rgn = {}
        /\ log = <<"> dib 0 3 3 8", "> pen 0 0 1 #ff0000", "> brush 0 solid #00ff00", "> rgn 1 null">>
        /\ n = 0

Begin == bracket' = "open" /\ path' = <<>> /\ UNCHANGED <<clip, pos, rgn>> /\ Call(<<"> beginpath 0", "beginpath 1">>, px)
End == IF bracket = "open"
       THEN /\ bracket' = "closed" /\ UNCHANGED <<clip, pos, rgn>>
            /\ path' = IF path # <<>> /\ path[Len(path)][3] = PT_MOVETO THEN SubSeq(path, 1, Len(path) - 1) ELSE path
            /\ Call(<<"> endpath 0", "endpath 1">>, px)
       ELSE UNCHANGED <<clip, bracket, path, pos, rgn>> /\ Call(<<"> endpath 0", "endpath 0">>, px)
Abort == bracket' = "none" /\ path' = <<>> /\ UNCHANGED <<clip, pos, rgn>> /\ Call(<<"> abortpath 0", "abortpath 1">>, px)

\* a figure recorded in the bracket (GDI.PATH.MOVETO, RECORD, CLOSE): MoveToEx to its first
\* point, LineTo to each other, CloseFigure when it is closed; outside a bracket they draw
RECURSIVE Moves(_)
Moves(f) == IF f = <<>> THEN <<>> ELSE <<"> lineto 0 " \o P(Head(f)), "ret 1">> \o Moves(Tail(f))
Figure(f, cl) ==
  LET lines == <<"> moveto 0 " \o P(f[1]), "pos " \o P(pos)>> \o Moves(Tail(f)) \o (IF cl THEN <<"> closefig 0", IF bracket = "open" THEN "closefig 1" ELSE "closefig 0">> ELSE <<>>)
      pts == [i \in 1..Len(f) |-> <<f[i][1], f[i][2], IF i = 1 THEN PT_MOVETO ELSE IF cl /\ i = Len(f) THEN PT_LINETO + PT_CLOSEFIGURE ELSE PT_LINETO>>]
      drawn == {<<f[i], f[i + 1]>> : i \in 1..(Len(f) - 1)}
  IN /\ pos' = f[Len(f)] /\ UNCHANGED <<clip, bracket, rgn>>
     /\ IF bracket = "open"
        THEN path' = (IF path # <<>> /\ path[Len(path)][3] = PT_MOVETO THEN SubSeq(path, 1, Len(path) - 1) ELSE path) \o pts /\ Call(lines, px)
        ELSE path' = path /\ Call(lines, Lines(px, drawn, clip))

\* GDI.PATH.FLATTEN: a path of lines stays as it is (GetPath answers it again)
Flatten == /\ UNCHANGED <<clip, bracket, path, pos, rgn>>
           /\ Call(<<"> flatten 0", IF bracket = "closed" THEN "flatten 1" ELSE "flatten 0", "> getpath 0">>
                   \o (IF bracket = "closed" THEN <<"path " \o S(Len(path))>> \o [i \in 1..Len(path) |-> "pt " \o P(path[i]) \o " " \o S(path[i][3])] ELSE <<"path -1">>), px)
\* GDI.PATH.STROKE: the path's lines with the pen, the path discarded; without a path it fails
\* (asked outside an open bracket: what StrokePath does to an open one is not said)
Stroke == bracket # "open" /\ IF bracket = "closed"
          THEN bracket' = "none" /\ path' = <<>> /\ UNCHANGED <<clip, pos, rgn>> /\ Call(<<"> strokepath 0", "strokepath 1">>, Lines(px, StrokeLines(path), clip))
          ELSE UNCHANGED <<clip, bracket, path, pos, rgn>> /\ Call(<<"> strokepath 0", "strokepath 0">>, px)
\* GDI.PATH.NOPATH: no closed path fails, the DC as it was; GDI.PATH.EMPTYFILL: an empty one
\* is discarded, with success for drawing and failure for a region; GDI.PATH.FILL: the interior filled (and the figures outlined), made
\* a region or the clip region, the path discarded
FillOp(op, line) ==
  LET fs == Figs(path, <<>>)
      inner == Interior(fs)
  IN IF bracket # "closed" THEN UNCHANGED <<clip, bracket, path, pos, rgn>> /\ Call(<<line, op \o " 0">>, px)
     ELSE /\ bracket' = "none" /\ path' = <<>> /\ UNCHANGED pos
          /\ IF fs = <<>> THEN UNCHANGED <<clip, rgn>> /\ Call(<<line, op \o (IF op \in {"fillpath", "strokefill"} THEN " 1" ELSE " 0")>>, px)
             ELSE CASE op = "fillpath" -> UNCHANGED <<clip, rgn>> /\ Call(<<line, "fillpath 1">>, Fill(px, inner, clip))
                    [] op = "strokefill" -> UNCHANGED <<clip, rgn>> /\ Call(<<line, "strokefill 1">>, Lines(Fill(px, inner, clip), OutlineLines(path), clip))
                    [] op = "pathrgn" -> rgn' = inner /\ UNCHANGED clip /\ Call(<<line, "pathrgn 1">>, px)
                    [] op = "selclippath" -> clip' = inner /\ UNCHANGED rgn /\ Call(<<line, "selclippath 1">>, px)
\* the region made, drawn
FillRgn == UNCHANGED <<clip, bracket, path, pos, rgn>> /\ Call(<<"> fillrgn 0 1", "ret 1">>, Fill(px, rgn, clip))
\* GDI.SHAPE.POLYFILL: Polygon outside a bracket, the brush over CreatePolygonRgn's region of the
\* points, then the pen's outline closed back to the first point; the position kept
RECURSIVE Coords(_)
Coords(f) == IF f = <<>> THEN "" ELSE " " \o P(Head(f)) \o Coords(Tail(f))
Polygon(f) == /\ bracket # "open" /\ UNCHANGED <<clip, bracket, path, pos, rgn>>
              /\ Call(<<"> polygon 0" \o Coords(f), "ret 1">>, Lines(Fill(px, Interior(<<f>>), clip), {Edges(f)[i] : i \in 1..Len(f)}, clip))

Square == <<<<0, 0>>, <<2, 0>>, <<2, 2>>, <<0, 2>>>>
Triangle == <<<<0, 0>>, <<2, 2>>, <<2, 0>>>>
Bow == <<<<0, 0>>, <<2, 2>>, <<2, 0>>, <<0, 2>>>>

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ Begin \/ End \/ Abort \/ Flatten \/ Stroke \/ FillRgn
           \/ \E f \in {Square, Triangle, Bow} : Polygon(f)
           \/ \E f \in {Square, Triangle, Bow}, cl \in BOOLEAN : Figure(f, cl)
           \/ Figure(<<<<2, 0>>, <<0, 2>>>>, FALSE)
           \/ Figure(<<<<2, 2>>>>, FALSE)
           \/ \E op \in {"fillpath", "strokefill"} : FillOp(op, "> " \o op \o " 0")
           \/ FillOp("pathrgn", "> pathrgn 0 1")
           \/ FillOp("selclippath", "> selclippath 0 5")

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == /\ bracket \in {"none", "open", "closed"} /\ \A q \in Dev : px[q] \in {0, Pen, Brush}
          /\ clip \subseteq Dev /\ rgn \subseteq Dev
\* stroking and filling discard the path: only a bracket holds points
NoBracketNoPath == bracket = "none" => path = <<>>
=============================================================================
