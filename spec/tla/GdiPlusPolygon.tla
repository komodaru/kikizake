---------------------------- MODULE GdiPlusPolygon ----------------------------
(* GDI+ FillPolygon of a solid brush, from spec/gdiplus.txt GP.SHAPE.POLYGON: which pixels a
   polygon of points on the 1/8 grid fills, by the samples it holds. A sample is held where its
   winding number is odd (FillMode Alternate, 0) or nonzero (Winding, 1): each edge not horizontal
   whose rows, taken half open from its top, hold the sample's row, and which crosses that row at or
   left of the sample, adds 1 going down and -1 going up. Aliased, a pixel's sample is (x, y), or
   (x + 1/2, y + 1/2) under a half pixel offset; antialiased, each premultiplied channel scales by
   the share of the 8 x 4 (HighQuality, AntiAlias) or 8 x 8 (AntiAlias8x8) subsamples at
   (x - s + i / 8, y - s + j / n) held, s 1/2 but 0 under a half pixel offset, rounded halves up
   (GP.PAINT.AA's share); a FillMode past 0 ..= 1 InvalidParameter, nothing filled. SourceCopy; the
   pixels and conversions are GdiPlusBitmap's (INSTANCE). Every behavior starts with a blank 4 x 3
   32bppARGB bitmap and takes at most MaxOps fills (some through a world transform), the first of one color and the next of another;
   after each, the bitmap is read. `kikizake tla` replays every state's log through the Rust model,
   whose fills go through its scan converter (GP.RASTER.FILL) (spec/tla/README.md). *)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS MaxOps

VARIABLES bmp, log, n
vars == <<bmp, log, n>>

B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
Abs(v) == IF v < 0 THEN -v ELSE v
\* a number of eighths as a decimal
Fr == <<"", ".125", ".25", ".375", ".5", ".625", ".75", ".875">>
Q(v) == (IF v < 0 THEN "-" ELSE "") \o S(Abs(v) \div 8) \o Fr[(Abs(v) % 8) + 1]
RECURSIVE PtsFrom(_, _)
PtsFrom(ps, i) == IF i > Len(ps) THEN "" ELSE " " \o Q(ps[i][1]) \o " " \o Q(ps[i][2]) \o PtsFrom(ps, i + 1)

W == 4
H == 3
Blank == B!New("32bppARGB", W, H)
Colors == <<B!Ch(255, 10, 200, 30), B!Ch(255, 250, 0, 120)>>

Init == /\ bmp = Blank
        /\ log = <<"> gpbitmap 0 4 3 32bppARGB", "bitmap 0">> \o B!Read(0, Blank, "32bppARGB")
        /\ n = 0

\* the winding number at sample (sx, sy), all in eighths
Edge(ps, i, sx, sy) ==
  LET a == ps[i] b == ps[(i % Len(ps)) + 1]
      down == a[2] < b[2]
      top == IF down THEN a[2] ELSE b[2]
      bottom == IF down THEN b[2] ELSE a[2]
      l == (sx - a[1]) * (b[2] - a[2])
      r == (sy - a[2]) * (b[1] - a[1])
  IN IF a[2] = b[2] \/ ~(top <= sy /\ sy < bottom) THEN 0
     ELSE IF down THEN (IF r <= l THEN 1 ELSE 0)
     ELSE (IF r >= l THEN -1 ELSE 0)
RECURSIVE WindFrom(_, _, _, _)
WindFrom(ps, i, sx, sy) == IF i > Len(ps) THEN 0 ELSE Edge(ps, i, sx, sy) + WindFrom(ps, i + 1, sx, sy)
Holds(ps, fm, sx, sy) == LET w == WindFrom(ps, 1, sx, sy) IN IF fm = 0 THEN w % 2 = 1 ELSE w # 0

\* a world transform: none, the identity, or one of no inverse (GP.MATRIX.INVERT: GdipSetWorldTransform
\* refuses it InvalidParameter, nothing filled)
One == "3f800000"
Zero == "00000000"
MatLine(wm) == CASE wm = "id" -> <<"> gpmatrix 0 0x" \o One \o " 0x" \o Zero \o " 0x" \o Zero \o " 0x" \o One \o " 0x" \o Zero \o " 0x" \o Zero,
                                   "matrix 0", "elements 0 " \o One \o " " \o Zero \o " " \o Zero \o " " \o One \o " " \o Zero \o " " \o Zero>>
                 [] wm = "sing" -> <<"> gpmatrix 0 0x" \o One \o " 0x" \o One \o " 0x" \o One \o " 0x" \o One \o " 0x" \o Zero \o " 0x" \o Zero,
                                     "matrix 0", "elements 0 " \o One \o " " \o One \o " " \o One \o " " \o One \o " " \o Zero \o " " \o Zero>>
                 [] OTHER -> <<>>

Fill(ps, fm, sm, po, wm) ==
  LET c == B!FromArgb("32bppPARGB", Colors[(n % 2) + 1])
      half == po \in {2, 4}
      call == "> gpshape 0 brush 0 " \o (IF wm = "-" THEN "-" ELSE "0") \o " copy 0 " \o S(sm) \o " " \o S(po) \o " polygon " \o S(fm) \o " "
              \o S(Len(ps)) \o PtsFrom(ps, 1)
      rows == IF sm = 5 THEN 8 ELSE 4
      s == IF half THEN 0 ELSE 4
      cov(x, y) == Cardinality({ij \in (0..7) \X (0..(rows - 1)) :
                                  Holds(ps, fm, 8 * x - s + ij[1], 8 * y - s + ij[2] * (8 \div rows))})
      share(v, k) == (2 * v * k + 8 * rows) \div (16 * rows)
      in(x, y) == Holds(ps, fm, 8 * x + (IF half THEN 4 ELSE 0), 8 * y + (IF half THEN 4 ELSE 0))
      px == [i \in 1..(W * H) |->
               LET x == (i - 1) % W y == (i - 1) \div W IN
               IF sm \in {2, 4, 5}
               THEN (IF cov(x, y) > 0
                     THEN B!Convert("32bppPARGB", "32bppARGB", [k \in 1..4 |-> share(c[k], cov(x, y))], <<>>)
                     ELSE bmp.px[i])
               ELSE IF in(x, y) THEN B!Convert("32bppPARGB", "32bppARGB", c, <<>>) ELSE bmp.px[i]]
      ok == fm \in {0, 1} /\ wm # "sing"
      b == IF ok THEN [bmp EXCEPT !.px = px] ELSE bmp
  IN /\ bmp' = b
     /\ log' = log \o MatLine(wm) \o <<"> gpbrush 0 solid 0x" \o B!Hex8(Colors[(n % 2) + 1]), "brush 0", call,
                        IF ok THEN "shape 0 0" ELSE "shape 0 2">> \o B!Read(0, b, "32bppARGB")
     /\ n' = n + 1

\* polygons in eighths: a triangle, one past the bitmap, a bow tie, a square wound twice, a
\* concave one on odd eighths
Polys == {<<<<4, 2>>, <<28, 6>>, <<10, 22>>>>,
          <<<<-8, -4>>, <<40, 4>>, <<12, 30>>>>,
          <<<<0, 0>>, <<32, 24>>, <<32, 0>>, <<0, 24>>>>,
          <<<<0, 0>>, <<24, 0>>, <<24, 16>>, <<0, 16>>, <<0, 0>>, <<24, 0>>, <<24, 16>>, <<0, 16>>>>,
          <<<<3, 1>>, <<29, 3>>, <<13, 9>>, <<27, 21>>, <<1, 17>>>>}
\* (SmoothingMode, PixelOffsetMode): aliased without and with a half pixel offset, antialiased 8 x 4
\* and 8 x 8 under both
Modes == {<<0, 0>>, <<0, 4>>, <<3, 2>>, <<4, 0>>, <<2, 4>>, <<5, 0>>, <<5, 2>>}

Next == /\ n < MaxOps
        /\ \/ \E ps \in Polys, fm \in {0, 1, 2}, m \in Modes : Fill(ps, fm, m[1], m[2], "-")
           \/ \E ps \in Polys, wm \in {"id", "sing"} : Fill(ps, 1, 0, 0, wm)

Spec == Init /\ [][Next]_vars

\* at most MaxOps fills
Bound == n <= MaxOps
=============================================================================
