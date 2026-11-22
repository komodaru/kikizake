---------------------------- MODULE GdiPlusObjects ----------------------------
(* GDI+ brushes, pens, image attributes and the calls that take them, from spec/gdiplus.txt:
   SolidBrush, HatchBrush, TextureBrush, GetBrushType (GP.BRUSH, GP.BRUSH.HATCH.RANGE,
   GP.BRUSH.TEXTURE, GP.BRUSH.TEXTURE.STATUS, GP.BRUSH.INFO), Pen and its setters (GP.PEN.MAKE),
   FillRectangle of a solid brush (GP.PAINT, GP.PAINT.NOBRUSH, GP.PAINT.MODES, GP.PAINT.NEGATIVE,
   GP.PAINT.ALIASED on whole coordinates), ImageAttributes::SetColorMatrix (GP.ATTR,
   GP.ATTR.FLAGS) and DrawImage at 1:1 through them (GP.DRAW.NOATTR, GP.DRAW.SKIPGRAYS,
   GP.DRAW.ALTGRAY, GP.DRAW.ALTGRAY.TRANSLATE; color matrices of whole entries, so every sum is
   exact). The pixels and conversions are GdiPlusBitmap's (INSTANCE). Every behavior starts with a
   blank 2 x 1 32bppARGB bitmap to draw on and a 2 x 1 source of a translucent gray and a color,
   and takes at most MaxOps calls; after each, the target is read and both brushes asked their
   type. `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES bmp, brush, attr, log, n
vars == <<bmp, brush, attr, log, n>>

B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])

None == [t |-> "none"]
Gray == B!Ch(128, 64, 64, 64)
Color == B!Ch(255, 16, 32, 48)
Target == B!New("32bppARGB", 2, 1)
Source == [B!New("32bppARGB", 2, 1) EXCEPT !.px = <<Gray, Color>>]

\* GP.BRUSH.INFO: BrushTypeSolidColor 0, HatchFill 1, TextureFill 2, a texture's transform the identity
Ask(g, br) == B!Read(0, g[0], "32bppARGB") \o
              (LET call(k) == "> gpbrushinfo " \o S(k)
                   info(k) == CASE br[k].t = "none" -> <<call(k), "brushinfo nobrush">>
                                [] br[k].t = "solid" -> <<call(k), "brushinfo 0 0">>
                                [] br[k].t = "hatch" -> <<call(k), "brushinfo 0 1">>
                                [] br[k].t = "texture" -> <<call(k), "brushinfo 0 2 0 3f800000 00000000 00000000 3f800000 00000000 00000000">>
               IN info(0) \o info(1))

Call(lines, g, br, at) == bmp' = g /\ brush' = br /\ attr' = at /\ log' = log \o lines \o Ask(g, br) /\ n' = n + 1

Init == /\ bmp = [k \in {0, 1} |-> IF k = 0 THEN Target ELSE Source]
        /\ brush = [k \in {0, 1} |-> None]
        /\ attr = [k \in {0, 1} |-> None]
        /\ log = <<"> gpbitmap 0 2 1 32bppARGB", "bitmap 0", "> gpbitmap 1 2 1 32bppARGB", "bitmap 0",
                   "> gpset 1 0 0 0x" \o B!Hex8(Gray), "set 0", "> gpset 1 1 0 0x" \o B!Hex8(Color), "set 0">>
                 \o Ask([k \in {0, 1} |-> IF k = 0 THEN Target ELSE Source], [k \in {0, 1} |-> None])
        /\ n = 0

Solids == {B!Ch(255, 51, 102, 153), B!Ch(128, 255, 64, 32)}
\* GP.BRUSH: a solid brush of one color
Solid(k, c) == Call(<<"> gpbrush " \o S(k) \o " solid 0x" \o B!Hex8(c), "brush 0">>, bmp,
                    [brush EXCEPT ![k] = [t |-> "solid", c |-> c]], attr)
\* GP.BRUSH, GP.BRUSH.HATCH.RANGE: a HatchStyle past 0 ..= 52 InvalidParameter, no brush
Hatch(k, st) == Call(<<"> gpbrush " \o S(k) \o " hatch " \o S(st) \o " 0xff000000 0xffffffff",
                       IF st \in 0..52 THEN "brush 0" ELSE "brush 2">>, bmp,
                     [brush EXCEPT ![k] = IF st \in 0..52 THEN [t |-> "hatch"] ELSE None], attr)
\* GP.BRUSH.TEXTURE, GP.BRUSH.TEXTURE.STATUS: a WrapMode past 0 ..= 4 or a 16bppGrayScale bitmap
\* OutOfMemory, no brush
Texture(k, w) == LET ok == w \in 0..4 /\ bmp[1].f # "16bppGrayScale" IN
  Call(<<"> gpbrush " \o S(k) \o " texture 1 " \o S(w), IF ok THEN "brush 0" ELSE "brush 3">>, bmp,
       [brush EXCEPT ![k] = IF ok THEN [t |-> "texture"] ELSE None], attr)
\* the source made again, of 16bppGrayScale (nothing drawn from it then)
GraySource == Call(<<"> gpbitmap 1 2 1 16bppGrayScale", "bitmap 0">>, [bmp EXCEPT ![1] = B!New("16bppGrayScale", 2, 1)], brush, attr)

\* GP.PEN.MAKE: any width, every setter any value, Ok
Pen(w) == Call(<<"> gppen 0 0xff000000 " \o w \o " dash 7 dashoff -3 cap 3 -1 255 join 9 align 1 miter -1",
                 "pen 0 0 0 0 0 0 0">>, bmp, brush, attr)

\* GP.PAINT.MODES: SmoothingMode 0 ..= 5 and PixelOffsetMode 0 ..= 4, past them InvalidParameter
\* and nothing drawn; GP.PAINT.NEGATIVE; GP.PAINT.ALIASED: ceil(x - o) <= i < ceil(x + w - o), the
\* pixels of a rectangle of whole coordinates under either offset; SourceCopy of the brush
\* premultiplied (GP.FILL.COPY)
Paint(k, r, sm, po) ==
  LET call == "> gppaint 0 " \o S(k) \o " " \o R(r) \o " copy 0 " \o S(sm) \o " " \o S(po) \o " 0 0"
      b == bmp[0]
      in(i) == r[3] >= 0 /\ r[4] >= 0 /\ Max(r[1], 0) <= i - 1 /\ i - 1 < Min(r[1] + r[3], b.w) /\ r[2] <= 0 /\ 0 < r[2] + r[4]
  IN IF brush[k].t = "none" THEN Call(<<call, "paint nobrush">>, bmp, brush, attr)
     ELSE IF sm \notin 0..5 \/ po \notin 0..4 THEN Call(<<call, "paint 0 2">>, bmp, brush, attr)
     ELSE Call(<<call, "paint 0 0">>,
               [bmp EXCEPT ![0].px = [i \in 1..2 |-> IF in(i) THEN B!Convert("32bppPARGB", "32bppARGB", B!FromArgb("32bppPARGB", brush[k].c), <<>>)
                                                       ELSE b.px[i]]], brush, attr)

\* color matrices, row by row (the rows r, g, b, a, 1; the columns r, g, b, a and the fifth)
Id == <<1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1>>
SwapRB == <<0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1>>
\* (the identity but for its last row: green made full)
GreenUp == <<1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 1, 0, 0, 1>>
NoRed == <<0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 1>>
RECURSIVE MtxFrom(_, _)
MtxFrom(m, i) == IF i > 25 THEN "" ELSE " " \o S(m[i]) \o MtxFrom(m, i + 1)
Mtx(m) == MtxFrom(m, 1)
TranslationOnly(m) == \A i, k \in 0..3 : m[i * 5 + k + 1] = (IF i = k THEN 1 ELSE 0)

\* GP.ATTR, GP.ATTR.FLAGS: flags past 0 ..= 2 or AltGray (2) without a gray matrix
\* InvalidParameter, the attributes made with no matrix
Attr(a, m, fl, gray) ==
  LET ok == fl \in {0, 1} \/ (fl = 2 /\ gray) IN
  Call(<<"> gpattr " \o S(a) \o " matrix " \o S(fl) \o Mtx(m) \o (IF gray THEN " gray" \o Mtx(NoRed) ELSE ""),
         IF ok THEN "attr 0 0" ELSE "attr 0 2">>, bmp, brush,
       [attr EXCEPT ![a] = IF ok THEN [t |-> "matrix", m |-> m, fl |-> fl] ELSE [t |-> "plain"]])
AttrNone(a) == Call(<<"> gpattr " \o S(a) \o " none", "attr 0 0">>, bmp, brush, [attr EXCEPT ![a] = [t |-> "plain"]])

\* [r g b a 1] times m, each channel clamped (whole entries: the sums exact)
Times(v, m) == [k \in 1..4 |-> Max(0, Min(255, v[1] * m[k] + v[2] * m[5 + k] + v[3] * m[10 + k] + v[4] * m[15 + k] + v[5] * m[20 + k]))]
\* GP.DRAW.SKIPGRAYS: a gray pixel as it is; GP.DRAW.ALTGRAY: through the gray matrix;
\* GP.DRAW.ALTGRAY.TRANSLATE: under a color matrix that only translates, as if opaque
Recolor(c, at) ==
  LET v == <<c[2], c[3], c[4], c[1], 255>> gray == c[2] = c[3] /\ c[3] = c[4]
      o == CASE at.fl = 1 /\ gray -> <<c[2], c[3], c[4], c[1]>>
             [] at.fl = 2 /\ gray /\ TranslationOnly(at.m) -> Times(<<c[2], c[3], c[4], 255, 255>>, NoRed)
             [] at.fl = 2 /\ gray -> Times(v, NoRed)
             [] OTHER -> Times(v, at.m)
  IN B!Ch(o[4], o[1], o[2], o[3])
\* GP.DRAW.NOATTR: an attributes slot never made, no call; a 1:1 DrawImage (nearest, half pixel
\* offset), SourceCopy: each pixel premultiplied, through the attributes' matrix if they have one
Draw(a, interp) ==
  LET call == "> gpdraw 0 1 " \o (IF a = -1 THEN "-" ELSE S(a)) \o " 0 0 2 1 0 0 2 1 copy 0 " \o S(interp) \o " 4"
      at == IF a = -1 THEN [t |-> "plain"] ELSE attr[a]
      px(c) == B!Convert("32bppPARGB", "32bppARGB", B!FromArgb("32bppPARGB", IF at.t = "matrix" THEN Recolor(c, at) ELSE c), <<>>)
  IN IF at.t = "none" THEN Call(<<call, "draw noattr">>, bmp, brush, attr)
     ELSE IF interp \notin 0..7 THEN Call(<<call, "draw 0 2">>, bmp, brush, attr)
     ELSE Call(<<call, "draw 0 0">>, [bmp EXCEPT ![0].px = [i \in 1..2 |-> px(bmp[1].px[i])]], brush, attr)

Next == /\ n < MaxOps
        /\ \/ \E k \in {0, 1} : \/ \E c \in Solids : Solid(k, c)
                                \/ \E st \in {0, 52, 53, -1} : Hatch(k, st)
                                \/ \E w \in {0, 4, 5, -1} : Texture(k, w)
           \/ GraySource
           \/ \E w \in {"1", "0", "-2.5"} : Pen(w)
           \/ \E k \in {0, 1}, r \in {<<0, 0, 1, 1>>, <<1, 0, -1, 1>>, <<-1, 0, 5, 2>>},
                 mode \in {<<0, 0>>, <<3, 2>>, <<6, 0>>, <<0, 5>>, <<-1, 0>>} :
                \* (a hatch or texture brush only where nothing is drawn)
                /\ (brush[k].t \in {"none", "solid"} \/ mode[1] \notin 0..5 \/ mode[2] \notin 0..4)
                /\ Paint(k, r, mode[1], mode[2])
           \/ \E a \in {0, 1} : \/ AttrNone(a)
                                \/ \E m \in {Id, SwapRB, GreenUp}, fl \in {0, 1, 2} : Attr(a, m, fl, fl = 2)
                                \/ \E fl \in {2, 3, -1} : Attr(a, Id, fl, FALSE)
           \/ \E a \in {-1, 0, 1}, interp \in {5, 8} : bmp[1].f # "16bppGrayScale" /\ Draw(a, interp)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* a brush slot holds one of the kinds made
Kinds == \A k \in {0, 1} : brush[k].t \in {"none", "solid", "hatch", "texture"}
=============================================================================
