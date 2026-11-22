---------------------------- MODULE GdiPlusFill ----------------------------
(* Graphics::Clear and FillRectangleI through a Graphics of a GDI+ bitmap, from spec/gdiplus.txt
   (GP.FILL, GP.FILL.NOGRAPHICS, GP.FILL.RECT, GP.FILL.NEGATIVE, GP.FILL.COPY, GP.FILL.OVER,
   GP.FILL.BUFFER, GP.FILL.INDEXED, GP.FILL.QUALITY.OTHER), on the formats and conversions of
   GdiPlusBitmap (its operators, through INSTANCE). The rectangle clipped to the bitmap, a negative
   size none; SourceCopy the brush premultiplied, then as from 32bppPARGB; SourceOver, without gamma
   correction, each premultiplied channel s + d (255 - sa) / 255 rounded, at most 255 (x86, the
   executor `kikizake tla` answers for); 32bppRGB's bytes as they are; a 1bppIndexed bitmap through a
   32bppPARGB copy of it, written back as the index of the premultiplied color. Every behavior makes
   a bitmap, then takes at most MaxOps calls that set a pixel, fill or clear; after each, the
   bitmap is read in its own format and in 32bppARGB and GetPixel asked of its pixels.
   `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES bmp, log, n
vars == <<bmp, log, n>>

B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
R(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])

Ask(b) == B!Read(0, b, IF b.ok THEN b.f ELSE "32bppARGB") \o B!Read(0, b, "32bppARGB")
          \o B!Get(0, b, <<0, 0>>) \o B!Get(0, b, <<1, 0>>) \o B!Get(0, b, <<0, 1>>)

Call(lines, b) == bmp' = [bmp EXCEPT ![0] = b] /\ log' = log \o lines \o Ask(b) /\ n' = n + 1

\* the formats filled here (16bppRGB555/565 dither, GP.FILL.DITHER; the 16-bit channel ones go
\* through the sRGB curve, GP.CONV.WIDEN, NARROW)
Formats == {"1bppIndexed", "16bppGrayScale", "16bppARGB1555", "24bppRGB", "32bppRGB", "32bppARGB", "32bppPARGB"}
Sizes == {<<2, 1>>, <<1, 2>>, <<0, 1>>}
Init == \E f \in Formats, s \in Sizes :
          /\ bmp = [k \in {0, 1} |-> IF k = 0 THEN B!Made(f, s) ELSE B!NoBitmap]
          /\ log = <<"> gpbitmap 0 " \o S(s[1]) \o " " \o S(s[2]) \o " " \o f,
                    IF s[1] > 0 /\ s[2] > 0 THEN "bitmap 0" ELSE "bitmap 2">> \o Ask(B!Made(f, s))
          /\ n = 0

SetPixel(p, c) ==
  LET b == bmp[0] call == "> gpset 0 " \o S(p[1]) \o " " \o S(p[2]) \o " 0x" \o B!Hex8(c) IN
  IF ~b.ok THEN Call(<<call, "gpset nobitmap">>, b)
  ELSE IF b.f = "16bppGrayScale" \/ p[1] >= b.w \/ p[2] >= b.h THEN Call(<<call, "set 2">>, b)
  ELSE Call(<<call, "set 0">>, [b EXCEPT !.px[p[2] * b.w + p[1] + 1] = B!FromArgb(b.f, c)])

\* GP.FILL.OVER: premultiplied s over d, each channel rounded, x86 saturating at 255
Over(s, d) == [k \in 1..4 |-> Min(255, s[k] + (d[k] * (255 - s[1]) + 127) \div 255)]
Blend(copy, s, d) == IF copy THEN s ELSE Over(s, d)

\* GP.FILL.RECT, GP.FILL.NEGATIVE: the pixels of the rectangle inside the bitmap
Inside(b, r, x, y) == Max(r[1], 0) <= x /\ x < Min(r[1] + r[3], b.w) /\ Max(r[2], 0) <= y /\ y < Min(r[2] + r[4], b.h)
\* one pixel, of value v at (x, y), after the fill
Filled(b, r, sp, copy, x, y, v) ==
  IF ~Inside(b, r, x, y) THEN v
  \* GP.FILL.BUFFER, GP.FILL.INDEXED: the palette color, premultiplied (opaque), blended, its index
  ELSE IF b.f = "1bppIndexed" THEN B!FromArgb(b.f, Blend(copy, sp, b.pal[v + 1]))
  ELSE IF b.f = "32bppRGB" THEN Blend(copy, sp, v)
  ELSE B!Convert("32bppPARGB", b.f, Blend(copy, sp, B!Convert(b.f, "32bppPARGB", v, b.pal)), b.pal)

\* GP.FILL, GP.FILL.NOGRAPHICS, GP.FILL.COPY (the brush premultiplied, GP.CONV.PREMULTIPLY)
Fill(name, call, r, c, copy) ==
  LET b == bmp[0] IN
  IF ~b.ok THEN Call(<<call, "gp" \o name \o " nobitmap">>, b)
  ELSE IF b.f \in {"16bppGrayScale", "16bppARGB1555"} THEN Call(<<call, name \o " 3">>, b)
  ELSE LET sp == B!FromArgb("32bppPARGB", c) IN
       Call(<<call, name \o " 0 0">>,
            [b EXCEPT !.px = [i \in DOMAIN b.px |-> Filled(b, r, sp, copy, (i - 1) % b.w, (i - 1) \div b.w, b.px[i])]])

Colors == {B!Ch(128, 255, 64, 32), B!Ch(255, 51, 102, 153), B!Ch(0, 255, 255, 255)}
Rects == {<<0, 0, 1, 1>>, <<-1, 0, 2, 1>>, <<1, 0, -1, 1>>, <<0, 0, 5, 5>>, <<2, 0, 1, 1>>}
\* GP.FILL.QUALITY.OTHER: CompositingQuality 5, past the enum, blends as Default (0); HighSpeed 1
\* and AssumeLinear 4 too (no gamma correction)
Qualities == {0, 1, 4, 5}

Next == /\ n < MaxOps
        /\ \/ \E p \in {<<0, 0>>, <<1, 0>>, <<0, 1>>}, c \in Colors : SetPixel(p, c)
           \/ \E r \in Rects, c \in Colors, m \in {"copy", "over"}, q \in Qualities :
                Fill("fill", "> gpfill 0 " \o R(r) \o " 0x" \o B!Hex8(c) \o " " \o m \o " " \o S(q), r, c, m = "copy")
           \/ \E c \in Colors : Fill("clear", "> gpclear 0 0x" \o B!Hex8(c), <<0, 0, 2, 2>>, c, TRUE)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* GP.FILL.OVER, GP.FILL.COPY: a 32bppPARGB bitmap's colors stay under their alpha
Premultiplied == bmp[0].ok /\ bmp[0].f = "32bppPARGB" => \A i \in DOMAIN bmp[0].px : \A k \in 2..4 : bmp[0].px[i][k] <= bmp[0].px[i][1]
=============================================================================
