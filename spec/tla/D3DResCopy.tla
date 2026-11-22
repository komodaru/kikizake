----------------------------- MODULE D3DResCopy -----------------------------
(* What UpdateSubresource, CopyResource, CopySubresourceRegion and ResolveSubresource leave in two
   resources, read back after each call, on Direct3D 11 and 10 (spec/d3dres.txt: D3DRES.UPDATE,
   UPDATE.BOX, UPDATE.EMPTY, UPDATE.IGNORED, UPDATE.BAD, REMOVED, COPY, COPY.FORMAT, COPY.IGNORED,
   COPYSUB, COPYSUB.BAD, COPYSUB.IGNORED, MS.REGION, RESOLVE, RESOLVE.IGNORED, RESOLVE.BAD,
   RESOLVE.SIZE, READ, READ.MS, READ.BAD; the cases seq-* of d3d11res and d3d10res make the same
   resources and calls, src/generators/d3dres.rs seq_desc, seq_ops). Two slots, each one of the
   descriptions Descs, made with the executor's initial data (xorshift32 bytes of the seed: 1 in
   slot 0, 2 in slot 1, rows 8 and slices 16 bytes apart) but the 4-sample one, cleared to magenta
   through a render target view; then one call, and every subresource of both read. A read gives
   the staging copy row by row: zeros past the last subresource, zeros of a multisampled
   resource (its copy does not run). Updates take the bytes of seed 5, rows 8 bytes apart. A box
   past a subresource removes the device: every read after fails with DXGI_ERROR_DEVICE_REMOVED
   (11: the staging copy; 10: its Map). The samples here are all one value in a pixel (a clear),
   so resolving copies. What WARP does past a level or with a single-sampled source is as the
   seq-* and probe-box-* cases measured. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES dev, d, made, bytes, removed, log, n
vars == <<dev, d, made, bytes, removed, log, n>>

Slots == 0 .. 1
S(i) == ToString(i)

\* xorshift32 from the seed (0 is a fixed point: 0x9e3779b9 instead), the low byte of each step
Data == [s \in {1, 2, 5} |->
    CASE s = 1 -> <<33, 1, 197, 79, 209, 208, 26, 178, 37, 116, 203, 55, 138, 174, 245, 177>>
      [] s = 2 -> <<66, 2, 130, 6, 26, 35, 89, 182, 42, 59, 202, 61, 9, 36, 62, 254>>
      [] s = 5 -> <<165, 5, 193, 67, 229, 183, 227, 225, 133, 49, 55, 217, 161, 218, 123, 203>>]

\* each description: its line after the slot, initial data's pitches (none: none), GetDesc's
\* words, dimension (4 a buffer), format, usage (0 default, 1 immutable, 3 staging), samples, mip
\* levels, each subresource's row bytes and rows, the bytes of an element (a buffer's box is in
\* bytes), the reads (subresource, row bytes, rows) the case asks
Sq(rb, rows) == [rb |-> rb, rows |-> rows]
Tex2(mips, f, usage, bind, cpu, count) ==
    "tex " \o "2 2 2 1 " \o S(mips) \o " " \o f \o " " \o S(usage) \o " " \o bind \o " " \o cpu \o " 0 " \o S(count) \o " 0"
Square == <<<<0, 8, 2>>, <<1, 8, 2>>>>
Descs == [
  P |-> [line |-> Tex2(1, "0x1c", 0, "0x28", "0", 1), init |-> " 8 16", words |-> "tex = 2 2 1 1 1c 1 0 0 28 0 0",
         dim |-> 2, fmt |-> 28, usage |-> 0, count |-> 1, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square],
  M |-> [line |-> Tex2(2, "0x1c", 0, "0x28", "0", 1), init |-> " 8 16", words |-> "tex = 2 2 2 1 1c 1 0 0 28 0 0",
         dim |-> 2, fmt |-> 28, usage |-> 0, count |-> 1, mips |-> 2, lay |-> <<Sq(8, 2), Sq(4, 1)>>, el |-> 4,
         reads |-> <<<<0, 8, 2>>, <<1, 4, 1>>, <<2, 8, 2>>>>],
  MS |-> [line |-> Tex2(1, "0x1c", 0, "0x20", "0", 4), init |-> "", words |-> "tex = 2 2 1 1 1c 4 0 0 20 0 0",
         dim |-> 2, fmt |-> 28, usage |-> 0, count |-> 4, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square],
  I |-> [line |-> Tex2(1, "0x1c", 1, "8", "0", 1), init |-> " 8 16", words |-> "tex = 2 2 1 1 1c 1 0 1 8 0 0",
         dim |-> 2, fmt |-> 28, usage |-> 1, count |-> 1, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square],
  G |-> [line |-> Tex2(1, "0x57", 0, "0x28", "0", 1), init |-> " 8 16", words |-> "tex = 2 2 1 1 57 1 0 0 28 0 0",
         dim |-> 2, fmt |-> 87, usage |-> 0, count |-> 1, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square],
  U |-> [line |-> Tex2(1, "0x1e", 0, "0x28", "0", 1), init |-> " 8 16", words |-> "tex = 2 2 1 1 1e 1 0 0 28 0 0",
         dim |-> 2, fmt |-> 30, usage |-> 0, count |-> 1, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square],
  B |-> [line |-> "buf 16 0 1 0 0 0", init |-> "", words |-> "buf = 10 0 1 0 0 0",
         dim |-> 4, fmt |-> 0, usage |-> 0, count |-> 1, mips |-> 1, lay |-> <<Sq(16, 1)>>, el |-> 1,
         reads |-> <<<<0, 16, 1>>, <<1, 16, 1>>>>],
  D |-> [line |-> "tex 1 4 1 1 1 0x1c 0 0x28 0 0 1 0", init |-> " 16 16", words |-> "tex = 4 1 1 1c 0 28 0 0",
         dim |-> 1, fmt |-> 28, usage |-> 0, count |-> 1, mips |-> 1, lay |-> <<Sq(16, 1)>>, el |-> 4,
         reads |-> <<<<0, 16, 1>>, <<1, 16, 1>>>>],
  R |-> [line |-> Tex2(1, "0x1c", 3, "0", "0x20000", 1), init |-> " 8 16", words |-> "tex = 2 2 1 1 1c 1 0 3 0 20000 0",
         dim |-> 2, fmt |-> 28, usage |-> 3, count |-> 1, mips |-> 1, lay |-> <<Sq(8, 2)>>, el |-> 4, reads |-> Square]]
Names == DOMAIN Descs

\* the typeless group of the formats here (R8G8B8A8 27, B8G8R8A8 90, a buffer's none) and the
\* integer one
Group(f) == IF f \in {28, 30} THEN 27 ELSE IF f = 87 THEN 90 ELSE 0
IntFmt(f) == f = 30

Min(x, y) == IF x < y THEN x ELSE y
Max(x, y) == IF x < y THEN y ELSE x
\* x - y, 0 below
Sat(x, y) == IF x > y THEN x - y ELSE 0

Hd == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
RECURSIVE Hex(_)
Hex(s) == IF s = <<>> THEN "" ELSE Hd[(Head(s) \div 16) + 1] \o Hd[(Head(s) % 16) + 1] \o Hex(Tail(s))
Zeros(k) == [i \in 1 .. k |-> 0]
\* the bytes of sq from offset `at` replaced by vals
Put(sq, at, vals) == [i \in 1 .. Len(sq) |-> IF at < i /\ i <= at + Len(vals) THEN vals[i - at] ELSE sq[i]]
\* the bytes at offset `at`, k of them
Take(sq, at, k) == SubSeq(sq, at + 1, at + k)

Desc(k) == Descs[d[k]]
NSubs(k) == Len(Desc(k).lay)
\* a subresource's bytes made with initial data: rows `rp` apart from the seed's first byte (a
\* buffer: its bytes)
RECURSIVE InitRows(_, _, _, _)
InitRows(data, rp, l, y) == IF y = l.rows THEN <<>> ELSE Take(data, y * rp, l.rb) \o InitRows(data, rp, l, y + 1)
Made(name, seed) ==
    LET x == Descs[name] IN
    IF name = "MS" THEN <<[i \in 1 .. 16 |-> IF i % 4 = 2 THEN 0 ELSE 255]>>
    ELSE [s \in 1 .. Len(x.lay) |-> InitRows(Data[seed], IF x.init = " 16 16" \/ x.dim = 4 THEN 16 ELSE 8, x.lay[s], 0)]

Answer(k) == IF d[k] = "G" /\ dev = "d10" THEN "tex hr 80070057"
             ELSE IF d[k] = "B" /\ dev = "d10" THEN "buf = 10 0 1 0 0" ELSE Desc(k).words
Makes(k) == ~(d[k] = "G" /\ dev = "d10")
MakeLines(k, name) ==
    LET x == Descs[name]
        seed == k + 1
        line == IF name = "B" THEN "buf " \o S(k) \o " 16 0 1 0 0 0 init " \o S(seed)
                ELSE IF name = "D" THEN "tex " \o S(k) \o " 1 4 1 1 1 0x1c 0 0x28 0 0 1 0 init " \o S(seed) \o " 16 16"
                ELSE "tex " \o S(k) \o SubSeq(x.line, 4, Len(x.line)) \o (IF x.init = "" THEN "" ELSE " init " \o S(seed) \o x.init)
    IN <<"> " \o line>>

\* the reads after each call: each subresource the description asks
\* (of the bytes b, the device removed or not: rm)
ReadLines(b, rm, k, r) ==
    LET c == "> read " \o S(k) \o " " \o S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " 1"
    IN IF ~made[k] THEN <<c, "read nores">>
    ELSE IF rm THEN <<c, IF dev = "d11" THEN "read hr 887a0005" ELSE "read map hr 887a0005">>
    ELSE IF r[1] >= NSubs(k) \/ Desc(k).count > 1 THEN <<c, "read " \o S(r[3]) \o " 1">> \o [y \in 1 .. r[3] |-> Hex(Zeros(r[2]))]
    ELSE <<c, "read " \o S(r[3]) \o " 1">> \o [y \in 1 .. r[3] |-> Hex(Take(b[k][r[1] + 1], (y - 1) * r[2], r[2]))]
RECURSIVE Cat(_, _, _, _)
Cat(b, rm, k, rs) == IF rs = <<>> THEN <<>> ELSE ReadLines(b, rm, k, Head(rs)) \o Cat(b, rm, k, Tail(rs))
Reads(b, rm) == Cat(b, rm, 0, Desc(0).reads) \o Cat(b, rm, 1, Desc(1).reads)

Init ==
    /\ dev \in {"d11", "d10"}
    /\ d \in [Slots -> Names]
    /\ made = [k \in Slots |-> Makes(k)]
    /\ bytes = [k \in Slots |-> IF Makes(k) THEN Made(d[k], k + 1) ELSE <<>>]
    /\ removed = FALSE
    /\ n = 0
    /\ log = (IF dev = "d11" THEN <<"> device11", "device11 ok">> ELSE <<"> device10", "device10 ok">>)
             \o MakeLines(0, d[0]) \o <<Answer(0)>> \o (IF d[0] = "MS" THEN <<"> rtv 0 0 nodesc", "rtv = 1c 6", "> clear 0 0x3f800000 0 0x3f800000 0x3f800000">> ELSE <<>>)
             \o MakeLines(1, d[1]) \o <<Answer(1)>> \o (IF d[1] = "MS" THEN <<"> rtv 1 1 nodesc", "rtv = 1c 6", "> clear 1 0x3f800000 0 0x3f800000 0x3f800000">> ELSE <<>>)
             \o Reads(bytes, FALSE)

\* a call, what it answers, the bytes and whether it removed the device; then every read
Call(c, out, b, rm) ==
    /\ bytes' = b /\ removed' = rm /\ UNCHANGED <<dev, d, made>> /\ n' = n + 1
    /\ log' = log \o <<"> " \o c>> \o out \o Reads(b, rm)
Nothing(c) == Call(c, <<>>, bytes, removed)
Removes(c) == Call(c, <<>>, bytes, TRUE)
Sets(c, k, s, v) == Call(c, <<>>, [bytes EXCEPT ![k][s + 1] = v], removed)

\* the boxes (left top front right bottom back; <<>> none) and where a region copy goes
Boxes == {<<>>, <<0, 0, 0, 1, 1, 1>>, <<1, 0, 0, 2, 2, 1>>, <<1, 0, 0, 1, 2, 1>>, <<0, 0, 0, 3, 1, 1>>, <<0, 1, 0, 2, 3, 1>>}
BoxArgs(bx) == IF bx = <<>> THEN "" ELSE " " \o S(bx[1]) \o " " \o S(bx[2]) \o " " \o S(bx[3]) \o " " \o S(bx[4]) \o " " \o S(bx[5]) \o " " \o S(bx[6])
Regions == {<<<<0, 0>>, <<>>>>, <<<<0, 0>>, <<0, 0, 0, 1, 1, 1>>>>, <<<<1, 0>>, <<0, 0, 0, 1, 1, 1>>>>, <<<<0, 1>>, <<1, 1, 0, 2, 2, 1>>>>,
            <<<<0, 0>>, <<1, 0, 0, 1, 2, 1>>>>, <<<<0, 0>>, <<0, 0, 0, 3, 1, 1>>>>, <<<<1, 1>>, <<>>>>}

\* rows t .. b - 1 of a subresource of row bytes rb from offset l of each row: row y the bytes of
\* `src` at so + (y - t) * sp, w of them
RECURSIVE PutRows(_, _, _, _, _, _, _, _, _)
PutRows(sq, rb, l, t, b, w, src, so, sp) ==
    IF t >= b THEN sq
    ELSE PutRows(Put(sq, t * rb + l, Take(src, so, w)), rb, l, t + 1, b, w, src, so + sp, sp)

\* UpdateSubresource (D3DRES.UPDATE, UPDATE.BOX, UPDATE.EMPTY, UPDATE.IGNORED, UPDATE.BAD,
\* REMOVED, MS.REGION): nothing to a subresource past the last, to a resource not of default
\* usage or multisampled, for an empty box; a box past the resource's top level removes the
\* device; else the box's rows from the source, 8 bytes apart, cut to the subresource (a buffer's
\* box in bytes; a 1D texture's one row from its top whatever its bottom)
Update(k, s, bx) ==
    LET c == "update " \o S(k) \o " " \o S(s) \o " 5 8 16" \o BoxArgs(bx)
        x == Desc(k)
        flat == x.dim = 1
        lay == x.lay[s + 1]
        top == x.lay[1]
        l == IF bx = <<>> THEN 0 ELSE bx[1]
        t == IF bx = <<>> THEN 0 ELSE bx[2]
        f == IF bx = <<>> THEN 0 ELSE bx[3]
        r == IF bx = <<>> THEN lay.rb \div x.el ELSE bx[4]
        b == IF bx = <<>> THEN lay.rows ELSE IF flat THEN t + 1 ELSE bx[5]
        bk == IF bx = <<>> THEN 1 ELSE IF flat THEN f + 1 ELSE bx[6]
        cr == Min(r, lay.rb \div x.el)
        cb == Min(b, lay.rows)
    IN IF ~made[k] THEN Call(c, <<"update nores">>, bytes, removed)
    ELSE IF x.count > 1 \/ s >= NSubs(k) \/ x.usage /= 0 \/ l >= r \/ t >= b \/ f >= bk THEN Nothing(c)
    ELSE IF r > top.rb \div x.el \/ b > top.rows \/ bk > 1 THEN Removes(c)
    ELSE IF l >= cr \/ t >= cb THEN Nothing(c)
    ELSE Sets(c, k, s, PutRows(bytes[k][s + 1], lay.rb, l * x.el, t, cb, (cr - l) * x.el, Data[5], 0, 8))

\* CopyResource (D3DRES.COPY, COPY.FORMAT, COPY.IGNORED, COPY.SAMPLES): every subresource, when
\* the dimension, levels and samples agree, the formats share a typeless group (no block
\* formats here), the destination is not immutable, and Direct3D 10 is not copying samples
Copy(dst, src) ==
    LET c == "copy " \o S(dst) \o " " \o S(src)
        a == Desc(dst) b == Desc(src)
    IN IF ~made[dst] \/ ~made[src] THEN Call(c, <<"copy nores">>, bytes, removed)
    ELSE IF a.dim = b.dim /\ a.mips = b.mips /\ a.count = b.count /\ Group(a.fmt) = Group(b.fmt) /\ a.usage /= 1
            /\ ~(dev = "d10" /\ a.count > 1)
         THEN Call(c, <<>>, [bytes EXCEPT ![dst] = bytes[src]], removed)
    ELSE Nothing(c)

\* CopySubresourceRegion (D3DRES.COPYSUB, COPYSUB.BAD, COPYSUB.IGNORED, MS.REGION, REMOVED): a
\* box or an offset to or from a multisampled resource, a missing subresource, an immutable
\* destination, sample counts that differ, a buffer and a texture, formats of other groups:
\* nothing; a box past the source or a region past the destination removes the device, an empty
\* box's too; an empty box inside both: nothing; else the box's elements to (x, y) (a buffer's to
\* x)
CopySub(dst, ds, at, src, ss, bx) ==
    LET c == "copysub " \o S(dst) \o " " \o S(ds) \o " " \o S(at[1]) \o " " \o S(at[2]) \o " 0 " \o S(src) \o " " \o S(ss) \o BoxArgs(bx)
        a == Desc(dst) b == Desc(src)
        whole == bx = <<>> /\ at = <<0, 0>>
        sl == b.lay[ss + 1] dl == a.lay[ds + 1]
        l == IF bx = <<>> THEN 0 ELSE bx[1]
        t == IF bx = <<>> THEN 0 ELSE bx[2]
        f == IF bx = <<>> THEN 0 ELSE bx[3]
        r == IF bx = <<>> THEN sl.rb \div b.el ELSE bx[4]
        bt == IF bx = <<>> THEN sl.rows ELSE bx[5]
        bk == IF bx = <<>> THEN 1 ELSE bx[6]
        dx == at[1]
        dy == IF a.dim = 4 THEN 0 ELSE at[2]
    IN IF ~made[dst] \/ ~made[src] THEN Call(c, <<"copysub nores">>, bytes, removed)
    ELSE IF ~whole /\ (a.count > 1 \/ b.count > 1) THEN Nothing(c)
    ELSE IF ds >= NSubs(dst) \/ ss >= NSubs(src) \/ a.usage = 1 \/ a.count /= b.count THEN Nothing(c)
    ELSE IF (a.dim = 4) /= (b.dim = 4) THEN Nothing(c)
    ELSE IF a.dim /= 4 /\ Group(a.fmt) /= Group(b.fmt) THEN Nothing(c)
    ELSE IF Max(l, r) > sl.rb \div b.el \/ Max(t, bt) > sl.rows \/ Max(f, bk) > 1 THEN Removes(c)
    ELSE IF dx + Sat(r, l) > dl.rb \div a.el \/ dy + Sat(bt, t) > dl.rows \/ Sat(bk, f) > 1 THEN Removes(c)
    ELSE IF r <= l \/ bt <= t \/ bk <= f THEN Nothing(c)
    ELSE Sets(c, dst, ds, PutRows(bytes[dst][ds + 1], dl.rb, dx * a.el, dy, dy + (bt - t), (r - l) * b.el,
                                 bytes[src][ss + 1], t * sl.rb + l * b.el, sl.rb))

\* ResolveSubresource (D3DRES.RESOLVE, RESOLVE.IGNORED, RESOLVE.BAD, RESOLVE.SIZE): a
\* multisampled destination or one not of default usage, a format of another group or an integer
\* one, a missing subresource or other dimensions: nothing; a single-sampled source larger than
\* the destination removes the device; else the samples' value where both are (a single-sampled
\* source's own)
Resolve(dst, ds, src, ss, fm) ==
    LET c == "resolve " \o S(dst) \o " " \o S(ds) \o " " \o S(src) \o " " \o S(ss) \o " " \o S(fm)
        a == Desc(dst) b == Desc(src)
    IN IF ~made[dst] \/ ~made[src] THEN Call(c, <<"resolve nores">>, bytes, removed)
    ELSE IF a.count /= 1 \/ a.usage /= 0 \/ Group(fm) /= Group(b.fmt) \/ Group(fm) /= Group(a.fmt) \/ IntFmt(fm) THEN Nothing(c)
    ELSE IF ds >= NSubs(dst) \/ ss >= NSubs(src) \/ a.dim /= b.dim THEN Nothing(c)
    ELSE LET sl == b.lay[ss + 1] dl == a.lay[ds + 1] rows == Min(sl.rows, dl.rows) w == Min(sl.rb, dl.rb)
         IN IF b.count < 2 /\ (sl.rb > dl.rb \/ sl.rows > dl.rows) THEN Removes(c)
            ELSE Sets(c, dst, ds, PutRows(bytes[dst][ds + 1], dl.rb, 0, 0, rows, w, bytes[src][ss + 1], 0, sl.rb))

Pairs == {<<0, 1>>, <<1, 0>>}
Next ==
    /\ n < MaxOps
    /\ \/ \E k \in Slots, s \in 0 .. 1, bx \in Boxes : Update(k, s, bx)
       \/ \E p \in Pairs : Copy(p[1], p[2])
       \/ \E p \in Pairs, ds \in 0 .. 1, ss \in 0 .. 1, g \in Regions : CopySub(p[1], ds, g[1], p[2], ss, g[2])
       \/ \E p \in Pairs, q \in {<<0, 0>>, <<1, 0>>, <<0, 1>>}, fm \in {28, 30, 87} : Resolve(p[1], q[1], p[2], q[2], fm)

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ dev \in {"d11", "d10"} /\ d \in [Slots -> Names] /\ removed \in BOOLEAN
          /\ \A k \in Slots : made[k] => Len(bytes[k]) = NSubs(k)
=============================================================================
