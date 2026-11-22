---------------------------- MODULE GdiPlusPaint ----------------------------
(* GDI+ FillRectangle without antialiasing, from spec/gdiplus.txt: the pixels of a rectangle of
   quarter-pixel coordinates under every PixelOffsetMode and the aliased SmoothingModes
   (GP.PAINT.ALIASED), filled with a solid brush or a hatch brush anchored at the rendering origin,
   its texels between fore and back premultiplied f w + b (256 - w) >> 8 (GP.PAINT.HATCH; the
   styles Horizontal, Vertical, ForwardDiagonal, BackwardDiagonal and Cross, whose patterns are
   rows, columns and diagonals of the 8 x 8 tile), SourceCopy (GP.FILL.COPY). The pixels and
   conversions are GdiPlusBitmap's (INSTANCE). Every behavior starts with a blank 4 x 2 32bppARGB
   bitmap and takes at most MaxOps fills, each making its brush first; after each, the bitmap is
   read. `kikizake tla` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES bmp, log, n
vars == <<bmp, log, n>>

B == INSTANCE GdiPlusBitmap

S(i) == ToString(i)
Min(a, b) == IF a < b THEN a ELSE b
Max(a, b) == IF a < b THEN b ELSE a
Abs(v) == IF v < 0 THEN -v ELSE v

\* a number of quarters as a decimal
Q(v) == (IF v < 0 THEN "-" ELSE "") \o S(Abs(v) \div 4)
        \o (CASE Abs(v) % 4 = 0 -> "" [] Abs(v) % 4 = 1 -> ".25" [] Abs(v) % 4 = 2 -> ".5" [] OTHER -> ".75")
\* ceil(v / 4)
Ceil4(v) == -((-v) \div 4)

W == 4
H == 2
Blank == B!New("32bppARGB", W, H)

Init == /\ bmp = Blank
        /\ log = <<"> gpbitmap 0 4 2 32bppARGB", "bitmap 0">> \o B!Read(0, Blank, "32bppARGB")
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

\* a brush: [t |-> "solid", c |-> color] or [t |-> "hatch", st |-> style, o |-> rendering origin];
\* the premultiplied color at pixel (x, y)
Color(br, x, y) ==
  IF br.t = "solid" THEN B!FromArgb("32bppPARGB", br.c)
  ELSE Texel(Weight(br.st, (x - br.o[1]) % 8, (y - br.o[2]) % 8))

Made(br) == IF br.t = "solid" THEN "> gpbrush 0 solid 0x" \o B!Hex8(br.c)
            ELSE "> gpbrush 0 hatch " \o S(br.st) \o " 0x" \o B!Hex8(Fore) \o " 0x" \o B!Hex8(Back)

\* GP.PAINT.ALIASED: the pixels i with ceil(x - o) <= i < ceil(x + w - o) and the same down, o a
\* half pixel for PixelOffsetMode HighQuality (2) and Half (4); SourceCopy of the brush's color
Paint(br, r, sm, po) ==
  LET o == IF po \in {2, 4} THEN 2 ELSE 0
      in(x, y) == Max(Ceil4(r[1] - o), 0) <= x /\ x < Min(Ceil4(r[1] + r[3] - o), W)
                  /\ Max(Ceil4(r[2] - o), 0) <= y /\ y < Min(Ceil4(r[2] + r[4] - o), H)
      org == IF br.t = "hatch" THEN br.o ELSE <<0, 0>>
      call == "> gppaint 0 0 " \o Q(r[1]) \o " " \o Q(r[2]) \o " " \o Q(r[3]) \o " " \o Q(r[4])
              \o " copy 0 " \o S(sm) \o " " \o S(po) \o " " \o S(org[1]) \o " " \o S(org[2])
      px == [i \in 1..(W * H) |->
               LET x == (i - 1) % W y == (i - 1) \div W IN
               IF in(x, y) THEN B!Convert("32bppPARGB", "32bppARGB", Color(br, x, y), <<>>) ELSE bmp.px[i]]
      b == [bmp EXCEPT !.px = px]
  IN /\ bmp' = b
     /\ log' = log \o <<Made(br), "brush 0", call, "paint 0 0">> \o B!Read(0, b, "32bppARGB")
     /\ n' = n + 1

\* rectangles in quarters (x, y, w, h): edges on whole pixels, halves and quarters, past the bitmap,
\* empty
Rects == {<<1, -2, 9, 4>>, <<2, 2, 4, 3>>, <<-3, 0, 20, 8>>, <<6, 3, 2, 1>>, <<0, 0, 0, 4>>, <<5, 1, 7, 0>>}
\* (SmoothingMode, PixelOffsetMode): the aliased smoothings Default, HighSpeed and None under every
\* offset mode
Modes == {<<0, 0>>, <<0, 2>>, <<1, 4>>, <<3, 1>>, <<3, 3>>}
Brushes == {[t |-> "solid", c |-> Solids[1]], [t |-> "solid", c |-> Solids[2]]}
           \cup {[t |-> "hatch", st |-> st, o |-> o] : st \in 0..4, o \in {<<0, 0>>, <<3, -2>>, <<-1, 5>>}}

Next == /\ n < MaxOps
        /\ \E br \in Brushes, r \in Rects, m \in Modes :
             \* (a hatch over the whole bitmap, or the rectangles under the first offsets)
             /\ (br.t = "solid" \/ r = <<-3, 0, 20, 8>> \/ m \in {<<0, 0>>, <<0, 2>>})
             /\ Paint(br, r, m[1], m[2])

Spec == Init /\ [][Next]_vars

\* every pixel a straight color of the bitmap's format: alpha 0 only for the blank one
Straight == \A i \in 1..(W * H) : bmp.px[i][1] = 0 => bmp.px[i] = B!Ch(0, 0, 0, 0)
=============================================================================
