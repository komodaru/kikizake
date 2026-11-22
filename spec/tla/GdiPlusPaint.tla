---------------------------- MODULE GdiPlusPaint ----------------------------
(* GDI+ FillRectangle, from spec/gdiplus.txt: the pixels of a rectangle of eighth-pixel
   coordinates under every PixelOffsetMode and the aliased SmoothingModes (GP.PAINT.ALIASED) or each
   pixel's share of subsamples (GP.PAINT.AA), filled with a solid brush, a hatch brush anchored at
   the rendering origin, its texels between fore and back premultiplied f w + b (256 - w) >> 8
   (GP.PAINT.HATCH; the styles Horizontal, Vertical, ForwardDiagonal, BackwardDiagonal and Cross,
   whose patterns are rows, columns and diagonals of the 8 x 8 tile), or a texture brush of a 2 x 2
   bitmap tiled from the device's (0, 0) under each WrapMode, its texels the bitmap as it was when
   the brush was made (GP.BRUSH.TEXTURE.PAINT); SourceCopy (GP.FILL.COPY) or SourceOver without
   gamma correction (GP.FILL.OVER). The pixels and conversions are GdiPlusBitmap's (INSTANCE). Every
   behavior starts with a blank 4 x 3 32bppARGB bitmap and a 2 x 2 source of four colors, and takes
   at most MaxOps fills, each making its brush first (a texture brush: then the source's first pixel
   is set to another color); after each, the bitmap is read. `kikizake tla` replays every state's
   log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, FiniteSets, TLC

CONSTANTS MaxOps

VARIABLES bmp, src, log, n
vars == <<bmp, src, log, n>>

B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Abs(v) == IF v < 0 THEN -v ELSE v

\* a number of eighths as a decimal
Fr == <<"", ".125", ".25", ".375", ".5", ".625", ".75", ".875">>
Q(v) == (IF v < 0 THEN "-" ELSE "") \o S(Abs(v) \div 8) \o Fr[(Abs(v) % 8) + 1]
\* ceil(v / 8)
Ceil8(v) == -((-v) \div 8)

W == 4
H == 3
Blank == B!New("32bppARGB", W, H)
\* the source's pixels, row by row, and the color its first one is set to after a texture brush is
\* made from it (and back)
Src == <<B!Ch(255, 255, 0, 0), B!Ch(128, 0, 200, 100), B!Ch(255, 0, 0, 255), B!Ch(64, 255, 255, 0)>>
Other == B!Ch(255, 0, 255, 0)

Init == /\ bmp = Blank
        /\ src = Src
        /\ log = <<"> gpbitmap 0 4 3 32bppARGB", "bitmap 0", "> gpbitmap 1 2 2 32bppARGB", "bitmap 0",
                   "> gpset 1 0 0 0x" \o B!Hex8(Src[1]), "set 0", "> gpset 1 1 0 0x" \o B!Hex8(Src[2]), "set 0",
                   "> gpset 1 0 1 0x" \o B!Hex8(Src[3]), "set 0", "> gpset 1 1 1 0x" \o B!Hex8(Src[4]), "set 0">>
                  \o B!Read(0, Blank, "32bppARGB")
        /\ n = 0

Fore == B!Ch(128, 200, 100, 50)
Back == B!Ch(255, 0, 64, 255)
Solids == <<B!Ch(255, 10, 20, 30), B!Ch(64, 255, 128, 0)>>

\* GP.PAINT.HATCH: the fore color's weight at texel (hx, hy) of the tile, 0 ..= 255
Weight(st, hx, hy) ==
  CASE st = 0 -> (IF hy = 0 THEN 255 ELSE 0)
    [] st = 1 -> (IF hx = 0 THEN 255 ELSE 0)
    [] st = 2 -> (LET d == (hx - hy) % 8 IN IF d = 0 THEN 234 ELSE IF d \in {1, 7} THEN 64 ELSE 0)
    [] st = 3 -> (LET d == (hx + hy) % 8 IN IF d = 7 THEN 234 ELSE IF d \in {0, 6} THEN 64 ELSE 0)
    [] st = 4 -> (IF hx = 0 \/ hy = 0 THEN 255 ELSE 0)
Mix(f, b, w) == (f * w + b * (256 - w)) \div 256
\* a texel premultiplied: fore, back, or between them per channel
Texel(w) ==
  LET f == B!FromArgb("32bppPARGB", Fore) b == B!FromArgb("32bppPARGB", Back) IN
  CASE w = 255 -> f [] w = 0 -> b [] OTHER -> [k \in 1..4 |-> Mix(f[k], b[k], w)]

\* GP.BRUSH.TEXTURE.PAINT: tile coordinate v of a tile n wide, every other tile flipped
Fold(v, k, flip) == IF flip /\ (v \div k) % 2 = 1 THEN k - 1 - (v % k) ELSE v % k
\* the texel of WrapMode wr (Tile 0, TileFlipX 1, TileFlipY 2, TileFlipXY 3, Clamp 4) at (x, y),
\* premultiplied; past Clamp's one tile transparent
TexAt(wr, x, y) ==
  IF wr = 4 /\ ~(x \in 0..1 /\ y \in 0..1) THEN B!Ch(0, 0, 0, 0)
  ELSE B!FromArgb("32bppPARGB", src[Fold(y, 2, wr \in {2, 3}) * 2 + Fold(x, 2, wr \in {1, 3}) + 1])

\* a brush: [t |-> "solid", c |-> color], [t |-> "hatch", st |-> style, o |-> rendering origin] or
\* [t |-> "texture", wr |-> WrapMode, o |-> rendering origin]; the premultiplied color at (x, y)
Color(br, x, y) ==
  CASE br.t = "solid" -> B!FromArgb("32bppPARGB", br.c)
    [] br.t = "hatch" -> Texel(Weight(br.st, (x - br.o[1]) % 8, (y - br.o[2]) % 8))
    [] br.t = "texture" -> TexAt(br.wr, x, y)

Made(br) == CASE br.t = "solid" -> <<"> gpbrush 0 solid 0x" \o B!Hex8(br.c), "brush 0">>
              [] br.t = "hatch" -> <<"> gpbrush 0 hatch " \o S(br.st) \o " 0x" \o B!Hex8(Fore) \o " 0x" \o B!Hex8(Back), "brush 0">>
              [] br.t = "texture" -> <<"> gpbrush 0 texture 1 " \o S(br.wr), "brush 0",
                                      "> gpset 1 0 0 0x" \o B!Hex8(IF src[1] = Other THEN Src[1] ELSE Other), "set 0">>

\* GP.FILL.OVER: premultiplied s over d, each channel rounded, x86 saturating at 255
Over(sc, d) == [k \in 1..4 |-> Min(255, sc[k] + (d[k] * (255 - sc[1]) + 127) \div 255)]
\* a pixel v of the bitmap after a source color sc (premultiplied) lands on it
Put(copy, sc, v) ==
  B!Convert("32bppPARGB", "32bppARGB", IF copy THEN sc ELSE Over(sc, B!Convert("32bppARGB", "32bppPARGB", v, <<>>)), <<>>)

\* GP.PAINT.ALIASED: the pixels i with ceil(x - o) <= i < ceil(x + w - o) and the same down, o a
\* half pixel for PixelOffsetMode HighQuality (2) and Half (4); SourceCopy of the brush's color
Paint(br, r, sm, po, copy) ==
  LET o == IF po \in {2, 4} THEN 4 ELSE 0
      in(x, y) == Max(Ceil8(r[1] - o), 0) <= x /\ x < Min(Ceil8(r[1] + r[3] - o), W)
                  /\ Max(Ceil8(r[2] - o), 0) <= y /\ y < Min(Ceil8(r[2] + r[4] - o), H)
      org == IF br.t = "solid" THEN <<0, 0>> ELSE br.o
      call == "> gppaint 0 0 " \o Q(r[1]) \o " " \o Q(r[2]) \o " " \o Q(r[3]) \o " " \o Q(r[4])
              \o (IF copy THEN " copy 0 " ELSE " over 0 ") \o S(sm) \o " " \o S(po) \o " " \o S(org[1]) \o " " \o S(org[2])
      \* GP.PAINT.AA: 8 subsamples across, 4 (HighQuality 2, AntiAlias 4) or 8 (AntiAlias8x8 5)
      \* down, at p + k / n in [x + s, x + w + s), s a half pixel but under HighQuality and Half
      \* offsets; each premultiplied channel times the count of the 8 n, rounded, halves up
      rows == IF sm = 5 THEN 8 ELSE 4
      s == 4 - o
      cnt(p, a, w, m) == Cardinality({k \in 0..(m - 1) : a + s <= 8 * p + (8 \div m) * k /\ 8 * p + (8 \div m) * k < a + w + s})
      cov(x, y) == cnt(x, r[1], r[3], 8) * cnt(y, r[2], r[4], rows)
      share(v, c) == (2 * v * c + 8 * rows) \div (16 * rows)
      px == [i \in 1..(W * H) |->
               LET x == (i - 1) % W y == (i - 1) \div W IN
               IF sm \in {2, 4, 5}
               THEN (IF cov(x, y) > 0
                     THEN Put(copy, [k \in 1..4 |-> share(Color(br, x, y)[k], cov(x, y))], bmp.px[i])
                     ELSE bmp.px[i])
               ELSE IF in(x, y) THEN Put(copy, Color(br, x, y), bmp.px[i]) ELSE bmp.px[i]]
      b == [bmp EXCEPT !.px = px]
  IN /\ bmp' = b
     /\ src' = IF br.t = "texture" THEN [src EXCEPT ![1] = IF @ = Other THEN Src[1] ELSE Other] ELSE src
     /\ log' = log \o Made(br) \o <<call, "paint 0 0">> \o B!Read(0, b, "32bppARGB")
     /\ n' = n + 1

\* rectangles in eighths (x, y, w, h): edges on whole pixels, halves, quarters and eighths, past
\* the bitmap, empty
Whole == <<-6, 0, 40, 24>>
Rects == {<<2, -4, 18, 8>>, <<4, 4, 8, 6>>, Whole, <<12, 6, 4, 2>>, <<0, 0, 0, 8>>, <<10, 2, 14, 0>>, <<3, 1, 19, 10>>}
\* (SmoothingMode, PixelOffsetMode): the aliased smoothings Default, HighSpeed and None under every
\* offset mode, the antialiased ones under both offsets
Modes == {<<0, 0>>, <<0, 2>>, <<1, 4>>, <<3, 1>>, <<3, 3>>, <<2, 0>>, <<4, 2>>, <<5, 4>>, <<5, 1>>}
Solid == {[t |-> "solid", c |-> Solids[1]], [t |-> "solid", c |-> Solids[2]]}
Hatch == {[t |-> "hatch", st |-> st, o |-> o] : st \in 0..4, o \in {<<0, 0>>, <<3, -2>>, <<-1, 5>>}}
Texture == {[t |-> "texture", wr |-> wr, o |-> o] : wr \in 0..4, o \in {<<0, 0>>, <<3, -2>>}}

\* (the first fill any of them; after it, the SourceOver fills and the texture brushes, whose texels
\* were taken before the source changed)
First ==
  \/ \E br \in Solid, r \in Rects, m \in Modes : Paint(br, r, m[1], m[2], TRUE)
  \* (a hatch over the whole bitmap, or the rectangles under one offset)
  \/ \E br \in Hatch, r \in Rects, m \in Modes : (r = Whole \/ m = <<0, 2>>) /\ Paint(br, r, m[1], m[2], TRUE)
Later ==
  \/ \E br \in Solid, r \in Rects, m \in {<<0, 0>>, <<5, 4>>} : Paint(br, r, m[1], m[2], FALSE)
  \/ \E br \in Hatch : Paint(br, Whole, 0, 0, FALSE)
  \/ \E br \in Texture, m \in {<<0, 0>>, <<0, 2>>, <<5, 4>>}, copy \in BOOLEAN : Paint(br, Whole, m[1], m[2], copy)
  \/ \E br \in Texture : Paint(br, <<3, 1, 19, 10>>, 0, 0, TRUE)

Next == /\ n < MaxOps
        /\ \/ Later
           \/ n = 0 /\ First

Spec == Init /\ [][Next]_vars

\* every pixel a straight color of the bitmap's format: alpha 0 only for the blank one
Straight == \A i \in 1..(W * H) : bmp.px[i][1] = 0 => bmp.px[i] = B!Ch(0, 0, 0, 0)
=============================================================================
