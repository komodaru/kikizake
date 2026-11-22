---------------------------- MODULE GdiDdb ----------------------------
(* Device-dependent bitmaps on memory DCs, from spec/gdiddb.txt: CreateBitmap, CreateCompatibleBitmap
   (of the display, of a DDB, of a DIB section) and their failures (GDI.DDB.ZERO, PLANES,
   NEGATIVE, NOBITS, COMPAT, COMPATDIB), GetObject of what was made (GDI.DDB.OBJECT,
   DIBOBJECT), GetBitmapBits of DDBs and DIB sections (GDI.DDB.BITS, DIBBITS), GetDeviceCaps of
   a memory DC (GDI.DDB.CAPS, CAPS.RASTER), GetObjectType (GDI.DDB.OBJTYPE), and BitBlt between a
   monochrome DDB and the display's format (GDI.DDB.MONOBLT). Slot 0 is made again by each
   creating call; slots 1 (8 by 1 monochrome), 2 (2 by 1 of the display), 3 (5 by 2 at 8 bpp,
   bottom-up) and 4 (1 by 2 at 16 bpp 5-6-5, bottom-up) are made first. After every call every
   slot that holds a bitmap is asked all of it; `kikizake tla` replays every state's log through
   the Rust model (spec/tla/README.md). *)
EXTENDS Integers, FiniteSets, Sequences, TLC

CONSTANTS MaxOps

\* the display of Windows' runner: 32 bits per pixel
Screen == 32
\* no bitmap in slot 0
Nothing == <<"none", 0, 0, 0>>

VARIABLES made, mono, color, bk, text, dib3, dib4, log, n
vars == <<made, mono, color, bk, text, dib3, dib4, log, n>>

S(i) == ToString(i)
Hex == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
H2(v) == Hex[v \div 16 + 1] \o Hex[(v % 16) + 1]
RECURSIVE Zeros(_)
Zeros(k) == IF k = 0 THEN "" ELSE "00" \o Zeros(k - 1)
RECURSIVE Cat(_)
Cat(s) == IF s = <<>> THEN "" ELSE Head(s) \o Cat(Tail(s))
CeilDiv(a, b) == (a + b - 1) \div b

\* a bitmap: <<"ddb", w, h, bpp>> or <<"dib", w, h, bpp>>; a failed creation leaves "none"
\* GDI.DDB.BITS, OBJECT: a DDB's rows are WORD aligned; a DIB section's (GDI.DDB.DIBOBJECT) DWORD
Words(w, bpp) == CeilDiv(w * bpp, 16) * 2
Stride(b) == IF b[1] = "ddb" THEN Words(b[2], b[4]) ELSE CeilDiv(b[2] * b[4], 32) * 4
Object(b) == IF b[1] = "ddb"
             THEN <<"bm bitmap " \o S(b[2]) \o " " \o S(b[3]) \o " " \o S(Stride(b)) \o " 1 " \o S(b[4]) \o " 0", "bmsize bitmap">>
             ELSE <<"bm dibsection " \o S(b[2]) \o " " \o S(b[3]) \o " " \o S(Stride(b)) \o " 1 " \o S(b[4]) \o " 1",
                    "dib " \o S(b[4]) \o " " \o S(b[3]) \o " " \o (IF b[4] = 16 THEN "3" ELSE "0") \o " " \o (IF b[4] <= 8 THEN S(2 ^ b[4]) ELSE "0"),
                    "bmsize bitmap">>
\* GDI.DDB.CAPS, CAPS.RASTER: TECHNOLOGY, BITSPIXEL, PLANES, NUMBRUSHES, NUMPENS, NUMCOLORS,
\* RASTERCAPS (0x7e99), SIZEPALETTE, NUMRESERVED, COLORRES, SHADEBLENDCAPS
Caps(k) == <<"> devcaps " \o S(k) \o " 2 12 14 16 18 24 38 104 106 108 120", "caps 1 32 1 -1 -1 -1 32409 0 20 24 0">>
\* GDI.DDB.OBJTYPE
Types(k) == <<"> objtype " \o S(k) \o " pen", "objtype 1", "> objtype " \o S(k) \o " brush", "objtype 2",
              "> objtype " \o S(k) \o " font", "objtype 6", "> objtype " \o S(k) \o " bitmap", "objtype 7">>

\* GDI.DDB.BITS, DIBBITS: every row top to bottom, each cut to its WORD aligned length
Bits(k, b, rows) == <<"> bmbits " \o S(k), "bmbits " \o S(Words(b[2], b[4]) * b[3]), Cat(rows)>>
MonoRow(m) == LET byte(i) == 128 * m[8 * i] + 64 * m[8 * i + 1] + 32 * m[8 * i + 2] + 16 * m[8 * i + 3] + 8 * m[8 * i + 4] + 4 * m[8 * i + 5] + 2 * m[8 * i + 6] + m[8 * i + 7]
              IN H2(byte(0)) \o "00"
Px(c) == H2(c[3]) \o H2(c[2]) \o H2(c[1]) \o "00"
Slot0(b) == IF b[1] = "none" THEN <<>> ELSE <<"> bmobject 0">> \o Object(b) \o Bits(0, b, <<Zeros(Words(b[2], b[4]) * b[3])>>) \o Caps(0) \o Types(0)

Mono == <<"ddb", 8, 1, 1>>
Color == <<"ddb", 2, 1, Screen>>
Dib3 == <<"dib", 5, 2, 8>>
Dib4 == <<"dib", 1, 2, 16>>
Row3(d, y) == H2(d[<<0, y>>]) \o H2(d[<<1, y>>]) \o H2(d[<<2, y>>]) \o H2(d[<<3, y>>]) \o H2(d[<<4, y>>]) \o "00"
Ask(b, m, c, d3, d4) == Slot0(b)
  \o <<"> bmobject 1">> \o Object(Mono) \o Bits(1, Mono, <<MonoRow(m)>>)
  \o <<"> bmobject 2">> \o Object(Color) \o Bits(2, Color, <<Px(c[0]) \o Px(c[1])>>)
  \o <<"> bmobject 3">> \o Object(Dib3) \o Bits(3, Dib3, <<Row3(d3, 0), Row3(d3, 1)>>)
  \o <<"> bmobject 4">> \o Object(Dib4) \o Bits(4, Dib4, <<H2(d4[0] % 256) \o H2(d4[0] \div 256), H2(d4[1] % 256) \o H2(d4[1] \div 256)>>)
  \o Caps(1) \o Types(2)

Call(lines, b) == made' = b /\ log' = log \o lines \o Ask(b, mono', color', dib3', dib4') /\ n' = n + 1

White == <<255, 255, 255>>
Black == <<0, 0, 0>>
Init == /\ made = Nothing /\ mono = [i \in 0..7 |-> 0] /\ color = [i \in 0..1 |-> Black] /\ bk = White /\ text = Black
        /\ dib3 = [q \in (0..4) \X (0..1) |-> 0] /\ dib4 = [i \in 0..1 |-> 0]
        /\ log = <<"> ddb 1 mono 8 1", "> ddb 2 screen 2 1", "> dib 3 5 2 8 bu", "> dib 4 1 2 16 bu 565">>
        /\ n = 0
Keep == UNCHANGED <<mono, color, bk, text, dib3, dib4>>

\* GDI.DDB.NEGATIVE: a negative width or height fails; GDI.DDB.ZERO: 0 makes 1 x 1 monochrome
Sized(w, h, b) == IF w = 0 \/ h = 0 THEN <<"ddb", 1, 1, 1>> ELSE <<b[1], w, h, b[4]>>
Mono0(w, h) == /\ Keep
               /\ IF w < 0 \/ h < 0 THEN Call(<<"> ddb 0 mono " \o S(w) \o " " \o S(h), "ddb failed">>, Nothing)
                  ELSE Call(<<"> ddb 0 mono " \o S(w) \o " " \o S(h)>>, Sized(w, h, <<"ddb", 0, 0, 1>>))
\* GDI.DDB.NOBITS: 0 planes or bits monochrome; GDI.DDB.PLANES: planes times bits one plane; a
\* memory DC selects only a monochrome bitmap or one of the display's format
Bitmap0(w, h, pb) ==
  LET line == "> ddb 0 bitmap " \o S(w) \o " " \o S(h) \o " " \o S(pb[1]) \o " " \o S(pb[2])
      bits == IF pb[1] < 1 \/ pb[2] < 1 THEN 1 ELSE pb[1] * pb[2]
  IN /\ Keep
     /\ IF w < 0 \/ h < 0 THEN Call(<<line, "ddb failed">>, Nothing)
        ELSE IF w = 0 \/ h = 0 THEN Call(<<line>>, <<"ddb", 1, 1, 1>>)
        ELSE IF bits # 1 /\ bits # Screen THEN Call(<<line, "select failed">>, Nothing)
        ELSE Call(<<line>>, <<"ddb", w, h, bits>>)
\* CreateCompatibleBitmap of the display: its format
Screen0(w, h) == /\ Keep
                 /\ IF w < 0 \/ h < 0 THEN Call(<<"> ddb 0 screen " \o S(w) \o " " \o S(h), "ddb failed">>, Nothing)
                    ELSE Call(<<"> ddb 0 screen " \o S(w) \o " " \o S(h)>>, Sized(w, h, <<"ddb", 0, 0, Screen>>))
\* GDI.DDB.COMPAT: of a memory DC, the format of its bitmap; GDI.DDB.COMPATDIB: a DIB section of
\* the DIB section's format
Of(k) == CASE k = 1 -> Mono [] k = 2 -> Color [] k = 3 -> Dib3 [] k = 4 -> Dib4
Compat0(k, w, h) ==
  LET line == "> ddb 0 compat " \o S(k) \o " " \o S(w) \o " " \o S(h)
  IN /\ Keep
     /\ IF w < 0 \/ h < 0 THEN Call(<<line, "ddb failed">>, Nothing) ELSE Call(<<line>>, Sized(w, h, Of(k)))

\* GDI.DDB.MONOBLT: monochrome to color, 1 bits the destination's background color, 0 its text
\* color; color to monochrome, the source DC's background color 1, every other 0
ToColor(sx) == /\ color' = [i \in 0..1 |-> IF sx + i > 7 THEN color[i] ELSE IF mono[sx + i] = 1 THEN bk ELSE text]
               /\ UNCHANGED <<mono, bk, text, dib3, dib4>>
               /\ Call(<<"> bitblt 2 0 0 2 1 1 " \o S(sx) \o " 0 0xcc0020", "ret 1">>, made)
ToMono(dx) == /\ mono' = [i \in 0..7 |-> IF i >= dx /\ i < dx + 2 THEN (IF color[i - dx] = bk THEN 1 ELSE 0) ELSE mono[i]]
              /\ UNCHANGED <<color, bk, text, dib3, dib4>>
              /\ Call(<<"> bitblt 1 " \o S(dx) \o " 0 2 1 2 0 0 0xcc0020", "ret 1">>, made)
\* the monochrome bitmap's bits set (WHITENESS) or cleared (BLACKNESS) with PatBlt
MonoPat(x, w, v) == /\ mono' = [i \in 0..7 |-> IF i >= x /\ i < x + w THEN v ELSE mono[i]]
                    /\ UNCHANGED <<color, bk, text, dib3, dib4>>
                    /\ Call(<<"> patblt 1 " \o S(x) \o " 0 " \o S(w) \o " 1 " \o (IF v = 1 THEN "0xff0062" ELSE "0x000042"), "ret 1">>, made)
CHex(c) == H2(c[1]) \o H2(c[2]) \o H2(c[3])
SetBk(c) == bk' = c /\ UNCHANGED <<mono, color, text, dib3, dib4>> /\ Call(<<"> bk 2 #" \o CHex(c)>>, made)
SetText(c) == text' = c /\ UNCHANGED <<mono, color, bk, dib3, dib4>> /\ Call(<<"> text 2 #" \o CHex(c)>>, made)
PokeColor(x, c) == /\ color' = [color EXCEPT ![x] = c] /\ UNCHANGED <<mono, bk, text, dib3, dib4>>
                   /\ Call(<<"> poke 2 " \o S(x) \o " 0 " \o H2(c[3]) \o H2(c[2]) \o H2(c[1])>>, made)
\* GDI.DDB.DIBBITS: pixels written into the DIB sections' rows
Poke3(q, v) == /\ dib3' = [dib3 EXCEPT ![q] = v] /\ UNCHANGED <<mono, color, bk, text, dib4>>
               /\ Call(<<"> poke 3 " \o S(q[1]) \o " " \o S(q[2]) \o " " \o H2(v)>>, made)
Poke4(y, v) == /\ dib4' = [dib4 EXCEPT ![y] = v] /\ UNCHANGED <<mono, color, bk, text, dib3>>
               /\ Call(<<"> poke 4 0 " \o S(y) \o " " \o H2(v % 256) \o H2(v \div 256)>>, made)

Next == /\ n < MaxOps
        /\ \/ \E s \in {<<3, 2>>, <<0, 2>>, <<-1, 2>>, <<2, -3>>} : Mono0(s[1], s[2])
           \/ \E s \in {<<3, 2>>, <<0, 1>>, <<-2, 1>>}, pb \in {<<1, 1>>, <<1, 32>>, <<4, 8>>, <<3, 8>>, <<4, 1>>, <<0, 8>>, <<1, 0>>, <<1, 8>>} : Bitmap0(s[1], s[2], pb)
           \/ \E s \in {<<2, 1>>, <<1, 0>>, <<-1, 1>>} : Screen0(s[1], s[2])
           \/ \E k \in 1..4, s \in {<<2, 1>>, <<0, 3>>, <<3, -1>>} : Compat0(k, s[1], s[2])
           \/ \E sx \in {0, 3} : ToColor(sx)
           \/ \E dx \in {0, 5} : ToMono(dx)
           \/ \E x \in {0, 2}, v \in {0, 1} : MonoPat(x, 3 + x, v)
           \/ \E c \in {White, <<16, 32, 48>>} : SetBk(c) \/ PokeColor(0, c)
           \/ \E c \in {Black, <<160, 176, 192>>} : SetText(c)
           \/ \E q \in {<<4, 0>>, <<0, 1>>} : Poke3(q, 171)
           \/ Poke4(1, 13330)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* slot 0 holds a selectable bitmap or nothing; monochrome bits stay bits
TypeOK == /\ made[1] = "none" \/ made[4] \in {1, Screen, 8, 16}
          /\ \A i \in 0..7 : mono[i] \in {0, 1}
=============================================================================
