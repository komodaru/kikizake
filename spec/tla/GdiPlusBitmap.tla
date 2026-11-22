---------------------------- MODULE GdiPlusBitmap ----------------------------
(* GDI+ bitmaps, from spec/gdiplus.txt: GdiplusStartup, Bitmap::Bitmap, GetPixelFormat and
   GetPalette, LockBits, GetPixel, SetPixel, Bitmap::Clone (GP.START, GP.CREATE,
   GP.CREATE.SIZE, GP.CREATE.ZERO, GP.INFO, GP.INFO.NOPALETTE, GP.LOCK.READ, GP.LOCK.RECT,
   GP.GRAY16, the GP.PIXEL and GP.CLONE clauses). A bitmap is its format, its size and its pixel values; the
   conversions are the integer rules of the GP.CONV clauses for the formats here, and an index of
   1bppIndexed is 1 where the 5-bit channels halved add to 23 or more (GP.CONV.TO.INDEXED's table
   as it is). Every behavior makes a bitmap, then takes at most MaxOps calls that change it or clone it;
   after each, the bitmap made or changed is asked everything: its format and palette, its pixels read in
   every format here, rectangles inside, past and empty, and GetPixel inside and past it.
   `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES bmp, log, n
vars == <<bmp, log, n>>

S(i) == ToString(i)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
P2(k) == 2 ^ k

\* the formats asked of: name, PixelFormat, bits a pixel
Code == [f \in {"1bppIndexed", "16bppGrayScale", "16bppRGB565", "16bppARGB1555", "24bppRGB",
                "32bppRGB", "32bppARGB", "32bppPARGB"} |->
           CASE f = "1bppIndexed" -> "00030101" [] f = "16bppGrayScale" -> "00101004"
             [] f = "16bppRGB565" -> "00021006" [] f = "16bppARGB1555" -> "00061007"
             [] f = "24bppRGB" -> "00021808" [] f = "32bppRGB" -> "00022009"
             [] f = "32bppARGB" -> "0026200a" [] f = "32bppPARGB" -> "000e200b"]
Formats == DOMAIN Code
Bits(f) == CASE f = "1bppIndexed" -> 1 [] f = "24bppRGB" -> 24
             [] f \in {"32bppRGB", "32bppARGB", "32bppPARGB"} -> 32 [] OTHER -> 16
\* the order the reads are asked in
ReadOrder == <<"32bppARGB", "32bppPARGB", "32bppRGB", "24bppRGB", "16bppRGB565", "16bppARGB1555",
               "1bppIndexed", "16bppGrayScale">>

RECURSIVE Hex(_, _)
Hex(v, k) == IF k = 0 THEN "" ELSE Hex(v \div 16, k - 1) \o SubSeq("0123456789abcdef", (v % 16) + 1, (v % 16) + 1)

\* (TLC's integers are 32-bit: a color is its channels <<a, r, g, b>>, and so is a pixel of a 24
\* or 32-bit format, its bytes from the high one (24bppRGB's a 0); a 16 or 1 bit pixel a number)
Ch(a, r, g, b) == <<a, r, g, b>>
X5(v) == (v * 8 + v \div 4) % 256
X6(v) == (v * 4 + v \div 16) % 256
\* GP.CONV.PREMULTIPLY, GP.CONV.UNPREMULTIPLY (c floor((255 << 16) / a) >> 16 split so as not to
\* overflow)
Pm(c, a) == (c * a + 127) \div 255
Un(c, a) == IF a = 0 THEN c
            ELSE LET q == 16711680 \div a IN Min(255, c * (q \div 65536) + (c * (q % 65536)) \div 65536)
\* GP.CONV.TO.INDEXED for 1 bpp: the color's 5-bit channels halved, white from 23 on
Nearest1(c) == IF (c[2] \div 16) + (c[3] \div 16) + (c[4] \div 16) >= 23 THEN 1 ELSE 0

\* GP.CONV.INDEXED, EXPAND, OPAQUE, UNPREMULTIPLY: a pixel as straight ARGB
ToArgb(f, v, pal) ==
  CASE f = "1bppIndexed" -> pal[v + 1]
    [] f = "16bppRGB565" -> Ch(255, X5((v \div 2048) % 32), X6((v \div 32) % 64), X5(v % 32))
    [] f = "16bppARGB1555" -> Ch(IF v >= 32768 THEN 255 ELSE 0, X5((v \div 1024) % 32), X5((v \div 32) % 32), X5(v % 32))
    [] f \in {"24bppRGB", "32bppRGB"} -> Ch(255, v[2], v[3], v[4])
    [] f = "32bppARGB" -> v
    [] f = "32bppPARGB" -> Ch(v[1], Un(v[2], v[1]), Un(v[3], v[1]), Un(v[4], v[1]))
\* GP.CONV.TO.INDEXED, TRUNCATE, OPAQUE, KEEP, PREMULTIPLY: straight ARGB as a pixel
FromArgb(f, c) ==
  LET a == c[1] r == c[2] g == c[3] b == c[4] IN
  CASE f = "1bppIndexed" -> Nearest1(c)
    [] f = "16bppRGB565" -> (r \div 8) * 2048 + (g \div 4) * 32 + b \div 8
    [] f = "16bppARGB1555" -> (a \div 128) * 32768 + (r \div 8) * 1024 + (g \div 8) * 32 + b \div 8
    [] f = "24bppRGB" -> Ch(0, r, g, b)
    [] f \in {"32bppRGB", "32bppARGB"} -> c
    [] f = "32bppPARGB" -> Ch(a, Pm(r, a), Pm(g, a), Pm(b, a))
\* LockBits' conversion: the bytes as they are in the bitmap's own format, 32bppPARGB to an opaque
\* format as it is (GP.CONV.PREMULTIPLIED.OPAQUE), else through straight ARGB
Convert(from, to, v, pal) ==
  IF from = to THEN v
  ELSE IF from = "32bppPARGB" /\ to \in {"16bppRGB565", "24bppRGB", "32bppRGB"} THEN FromArgb(to, v)
  ELSE FromArgb(to, ToArgb(from, v, pal))
\* a pixel's bytes as LockBits gives them, little-endian
Bytes(f, v) == CASE Bits(f) = 16 -> Hex(v % 256, 2) \o Hex(v \div 256, 2)
                 [] Bits(f) = 24 -> Hex(v[4], 2) \o Hex(v[3], 2) \o Hex(v[2], 2)
                 [] Bits(f) = 32 -> Hex(v[4], 2) \o Hex(v[3], 2) \o Hex(v[2], 2) \o Hex(v[1], 2)
Hex8(c) == Hex(c[1], 2) \o Hex(c[2], 2) \o Hex(c[3], 2) \o Hex(c[4], 2)
Zero(f) == IF Bits(f) >= 24 THEN Ch(0, 0, 0, 0) ELSE 0

BW == <<Ch(255, 0, 0, 0), Ch(255, 255, 255, 255)>>
NoBitmap == [ok |-> FALSE]
\* GP.CREATE.ZERO: every pixel 0; GP.INFO: an indexed bitmap's default palette (black and
\* white, PaletteFlagsGrayScale), GP.INFO.NOPALETTE: none for the others
New(f, w, h) == [ok |-> TRUE, f |-> f, w |-> w, h |-> h, px |-> [i \in 1..(w * h) |-> Zero(f)],
                 pal |-> IF f = "1bppIndexed" THEN BW ELSE <<>>, flags |-> IF f = "1bppIndexed" THEN 2 ELSE 0]
Px(b, x, y) == b.px[y * b.w + x + 1]

PalLine(p) == IF p = <<>> THEN <<>> ELSE <<Hex8(p[1]) \o " " \o Hex8(p[2])>>
Info(k, b) == IF ~b.ok THEN <<"> gpinfo " \o S(k), "gpinfo nobitmap">>
              ELSE <<"> gpinfo " \o S(k), "info " \o Code[b.f] \o " " \o S(b.w) \o " " \o S(b.h) \o " palette 0 "
                       \o S(b.flags) \o " " \o S(Len(b.pal))>> \o PalLine(b.pal)

\* a row of the rectangle's pixels in format f: 1 bpp from the high bit (the rest of the byte 0)
Row(b, f, x, y, w) ==
  IF f = "1bppIndexed"
  THEN Hex(LET v(i) == IF i < w THEN Convert(b.f, f, Px(b, x + i, y), b.pal) * P2(7 - i) ELSE 0
           IN v(0) + v(1), 2)
  ELSE LET one(i) == Bytes(f, Convert(b.f, f, Px(b, x + i, y), b.pal))
       IN IF w = 1 THEN one(0) ELSE one(0) \o one(1)

\* GP.LOCK.RECT: inside the bitmap and not empty; GP.GRAY16: 16bppGrayScale only as itself;
\* GP.LOCK.READ: the rectangle's pixels in the format asked
ReadAt(k, b, f, r, at) ==
  LET call == "> gpread " \o S(k) \o " " \o f \o at IN
  IF ~b.ok THEN <<call, "gpread nobitmap">>
  ELSE IF ~(r[1] >= 0 /\ r[2] >= 0 /\ r[3] > 0 /\ r[4] > 0 /\ r[1] + r[3] <= b.w /\ r[2] + r[4] <= b.h)
       THEN <<call, "read 2">>
  ELSE IF f # b.f /\ (f = "16bppGrayScale" \/ b.f = "16bppGrayScale") THEN <<call, "read 2">>
  ELSE <<call, "read 0", "px " \o S(r[3]) \o " " \o S(r[4]) \o " " \o S(Bits(f))>>
       \o [j \in 1..r[4] |-> Row(b, f, r[1], r[2] + j - 1, r[3])]
Read(k, b, f) == IF b.ok THEN ReadAt(k, b, f, <<0, 0, b.w, b.h>>, "") ELSE ReadAt(k, b, f, <<0, 0, 0, 0>>, "")

\* GP.PIXEL.GET: the pixel as straight ARGB; GP.PIXEL.BOUNDS; GP.GRAY16
Get(k, b, p) ==
  LET call == "> gpget " \o S(k) \o " " \o S(p[1]) \o " " \o S(p[2]) IN
  IF ~b.ok THEN <<call, "gpget nobitmap">>
  ELSE IF b.f = "16bppGrayScale" THEN <<call, "get 2">>
  ELSE IF p[1] < 0 \/ p[2] < 0 \/ p[1] >= b.w \/ p[2] >= b.h THEN <<call, "get 2">>
  ELSE <<call, "get 0 " \o Hex8(ToArgb(b.f, Px(b, p[1], p[2]), b.pal))>>

Probes == <<<<1, 0, 1, 1>>, <<0, 1, 1, 1>>, <<0, 0, 3, 1>>, <<0, 0, 1, 3>>, <<0, 0, 0, 1>>, <<-1, 0, 1, 1>>>>
Points == <<<<0, 0>>, <<1, 0>>, <<0, 1>>, <<2, 0>>, <<-1, 0>>>>
At(r) == " at " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
RECURSIVE CatFrom(_, _)
CatFrom(s, i) == IF i > Len(s) THEN <<>> ELSE s[i] \o CatFrom(s, i + 1)
Cat(s) == CatFrom(s, 1)

\* everything asked of slot k after a call
Ask(k, b) == Info(k, b)
             \o Cat([i \in 1..Len(ReadOrder) |-> Read(k, b, ReadOrder[i])])
             \o Cat([i \in 1..Len(Probes) |-> ReadAt(k, b, "32bppARGB", Probes[i], At(Probes[i]))])
             \o Cat([i \in 1..Len(Points) |-> Get(k, b, Points[i])])

Call(k, lines, b) == bmp' = [bmp EXCEPT ![k] = b] /\ log' = log \o lines \o Ask(k, b) /\ n' = n + 1

Sizes == {<<2, 1>>, <<1, 2>>, <<0, 1>>, <<1, -1>>}
\* GP.START (the first call starts GDI+), GP.CREATE, GP.CREATE.SIZE: a width or height of 0 or
\* less InvalidParameter, the slot empty; slot 1 asked before anything is made in it
Made(f, s) == IF s[1] > 0 /\ s[2] > 0 THEN New(f, s[1], s[2]) ELSE NoBitmap
Init == \E f \in Formats, s \in Sizes :
          /\ bmp = [k \in {0, 1} |-> IF k = 0 THEN Made(f, s) ELSE NoBitmap]
          /\ log = <<"> gpbitmap 0 " \o S(s[1]) \o " " \o S(s[2]) \o " " \o f,
                    IF s[1] > 0 /\ s[2] > 0 THEN "bitmap 0" ELSE "bitmap 2">>
                   \o Ask(0, Made(f, s)) \o Ask(1, NoBitmap)
          /\ n = 0

Colors == {Ch(128, 255, 64, 32), Ch(255, 51, 102, 153), Ch(0, 255, 255, 255)}
\* GP.PIXEL.SET, GP.PIXEL.SET.INDEXED: the color as the bitmap's format stores it
SetPixel(p, c) ==
  LET b == bmp[0] call == "> gpset 0 " \o S(p[1]) \o " " \o S(p[2]) \o " 0x" \o Hex8(c) IN
  IF ~b.ok THEN Call(0, <<call, "gpset nobitmap">>, b)
  ELSE IF b.f = "16bppGrayScale" \/ p[1] < 0 \/ p[2] < 0 \/ p[1] >= b.w \/ p[2] >= b.h THEN Call(0, <<call, "set 2">>, b)
  ELSE Call(0, <<call, "set 0">>, [b EXCEPT !.px[p[2] * b.w + p[1] + 1] = FromArgb(b.f, c)])

\* GP.CLONE, GP.CLONE.RECT (OutOfMemory, the slot as it was), GP.GRAY16, GP.CLONE.FORMAT (each pixel
\* as LockBits converts it), GP.CLONE.PALETTE (into 1 bpp from another format: FixedBW, 0x200)
Clone(r, f) ==
  LET b == bmp[0] call == "> gpclone 1 0 " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4]) \o " " \o f IN
  IF ~b.ok THEN Call(1, <<call, "gpclone nobitmap">>, bmp[1])
  ELSE IF ~(r[1] >= 0 /\ r[2] >= 0 /\ r[3] > 0 /\ r[4] > 0 /\ r[1] + r[3] <= b.w /\ r[2] + r[4] <= b.h)
       \/ (f # b.f /\ (f = "16bppGrayScale" \/ b.f = "16bppGrayScale"))
       THEN Call(1, <<call, "clone 3">>, bmp[1])
  ELSE LET c == New(f, r[3], r[4])
           pal == IF f = b.f THEN b.pal ELSE c.pal
           flags == IF f = b.f THEN b.flags ELSE IF f = "1bppIndexed" THEN 512 ELSE 0
       IN Call(1, <<call, "clone 0">>,
               [c EXCEPT !.pal = pal, !.flags = flags,
                         !.px = [i \in 1..(r[3] * r[4]) |->
                                  Convert(b.f, f, Px(b, r[1] + ((i - 1) % r[3]), r[2] + ((i - 1) \div r[3])), b.pal)]])

\* a write of the runner's ramp (byte k of the buffer is k, row after row) in format f: pixel x of
\* row y from its bytes (1 bpp from the high bit)
RampBytes(f, w) == IF f = "1bppIndexed" THEN 1 ELSE w * (Bits(f) \div 8)
RampPx(f, w, x, y) ==
  LET k0 == y * RampBytes(f, w) IN
  CASE f = "1bppIndexed" -> (k0 \div P2(7 - x)) % 2
    [] Bits(f) = 16 -> LET k == k0 + 2 * x IN k + 256 * (k + 1)
    [] Bits(f) = 24 -> LET k == k0 + 3 * x IN Ch(0, k + 2, k + 1, k)
    [] Bits(f) = 32 -> LET k == k0 + 4 * x IN Ch(k + 3, k + 2, k + 1, k)
\* GP.LOCK.WRITE through LockBits and UnlockBits of the whole bitmap; GP.GRAY16: a write between
\* 16bppGrayScale and another format locks but UnlockBits is InvalidParameter, nothing changed
Write(f) ==
  LET b == bmp[0] call == "> gpwrite 0 " \o f \o " ramp" IN
  IF ~b.ok THEN Call(0, <<call, "gpwrite nobitmap">>, b)
  ELSE IF f # b.f /\ (f = "16bppGrayScale" \/ b.f = "16bppGrayScale") THEN Call(0, <<call, "write 0 2">>, b)
  ELSE Call(0, <<call, "write 0 0">>,
            [b EXCEPT !.px = [i \in DOMAIN b.px |-> Convert(f, b.f, RampPx(f, b.w, (i - 1) % b.w, (i - 1) \div b.w), <<>>)]])

CloneRects == {<<0, 0, 2, 1>>, <<1, 0, 1, 1>>, <<0, 0, 1, 2>>, <<0, 0, 3, 1>>, <<0, 0, 0, 1>>}
CloneFormats == {"32bppARGB", "24bppRGB", "1bppIndexed", "16bppGrayScale", "32bppPARGB", "16bppRGB565"}

Next == /\ n < MaxOps
        /\ \/ \E p \in {<<0, 0>>, <<1, 0>>, <<2, 0>>, <<0, 1>>}, c \in Colors : SetPixel(p, c)
           \/ \E f \in {"32bppARGB", "24bppRGB", "16bppRGB565", "16bppGrayScale"} : Write(f)
           \/ \E r \in CloneRects, f \in CloneFormats : Clone(r, f)

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* every pixel a value of its format, and a palette only for 1 bpp
InFormat == \A k \in {0, 1} : bmp[k].ok =>
              /\ \A i \in DOMAIN bmp[k].px : IF Bits(bmp[k].f) >= 24 THEN bmp[k].px[i] \in [1..4 -> 0..255]
                                                ELSE bmp[k].px[i] \in 0..(P2(Bits(bmp[k].f)) - 1)
              /\ (bmp[k].pal # <<>>) = (bmp[k].f = "1bppIndexed")
=============================================================================
