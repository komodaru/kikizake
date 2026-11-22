----------------------------- MODULE D3DResClear -----------------------------
(* ClearRenderTargetView, ClearDepthStencilView and ClearUnorderedAccessViewUint / Float on
   Direct3D 11 and 10 (spec/d3dres.txt D3DRES.CLEAR.RTV, CLEAR.DSV, CLEAR.DEPTHRANGE, CLEAR.UAV,
   CLEAR.UAVRAW, CLEAR.UAVRAWF, CLEAR.UAVFINT; the clear-*, cleards-* and clearuav-* cases of
   d3d11res and d3d10res make the same calls). One resource in slot 0, made without initial data
   (zeros), and views of it in view slots 0 to 3: a 2 by 1 R8G8B8A8_UNORM texture of two levels
   (render target views and, on 11, unordered access views of each level), R32_FLOAT (both
   kinds), R32_UINT and R8G8B8A8_UINT (unordered access), D24_UNORM_S8_UINT and D32_FLOAT (depth
   stencil), a 16-byte raw buffer (an unordered access view of words 1 and 2). Then calls, each
   followed by a read of every subresource. A clear writes each element of the view's
   subresource: the color converted to the view's format (UNORM: 0 and below, NaN, give 0, 1 and
   above the largest value; 32-bit float: the value's bits), the depth converted (D24: 0 or
   1 times 2^24 - 1; D32: the bits) and the stencil's low 8 bits, as the flags (1 depth, 2
   stencil) say; a depth outside 0..1 or NaN drops the call. A uint UAV clear writes each
   component the low bits of its value, a float one the converted color but changes nothing of
   an integer view; a raw view's words take the first value, and a float clear of it changes
   nothing. Direct3D 10 has no unordered access views: clearuav prints none. A clear of a slot
   without a view of its kind prints noview. The values are ones whose conversion is exact here
   (0, 1, 2, -1, -2, NaN; 0.5 for 32-bit floats and D32): the conversions' roundings are the
   CONV.* clauses'. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES dev, cfg, subs, log, n
vars == <<dev, cfg, subs, log, n>>

S(i) == ToString(i)
Hd == <<"0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f">>
RECURSIVE Hex(_)
Hex(s) == IF s = <<>> THEN "" ELSE Hd[(Head(s) \div 16) + 1] \o Hd[(Head(s) % 16) + 1] \o Hex(Tail(s))
LE(x, k) == [i \in 1 .. k |-> (x \div (2 ^ (8 * (i - 1)))) % 256]
Zeros(k) == [i \in 1 .. k |-> 0]

\* the floats: their bits as the call writes them and as little-endian bytes, what a UNORM
\* component takes (TLC's integers stop at 2^31, so 32-bit words are their bytes)
Fs == {"0", "1", "2", "m1", "m2", "nan", "half"}
Top(x) == CASE x = "0" -> 0 [] x = "1" -> 63 [] x = "2" -> 64 [] x = "m1" -> 191 [] x = "m2" -> 192
               [] x = "nan" -> 127 [] x = "half" -> 63
Third(x) == CASE x = "1" -> 128 [] x = "m1" -> 128 [] x = "nan" -> 192 [] OTHER -> 0
FB(x) == <<0, 0, Third(x), Top(x)>>
Arg(x) == "0x" \o Hex(<<FB(x)[4], FB(x)[3], FB(x)[2], FB(x)[1]>>)
Unorm8(x) == IF x \in {"1", "2"} THEN 255 ELSE 0
\* colors (r, g, b, a): the UNORM ones, and for 32-bit floats one of 0.5 too
Colors == {<<"1", "0", "2", "nan">>, <<"m1", "1", "0", "1">>, <<"nan", "m2", "1", "2">>}
FColors == Colors \cup {<<"half", "0", "0", "0">>}
\* uint values: each its bytes, little-endian
W(b) == [b |-> b, arg |-> "0x" \o Hex(<<b[4], b[3], b[2], b[1]>>)]
Uints == {<<W(<<239, 190, 173, 222>>), W(<<1, 0, 0, 0>>), W(<<2, 0, 0, 0>>), W(<<3, 0, 0, 0>>)>>,
          <<W(<<255, 1, 0, 0>>), W(<<2, 1, 0, 0>>), W(<<3, 0, 0, 0>>), W(<<255, 255, 255, 255>>)>>}
Depths == {"0", "1", "half", "2", "m1", "nan"}

\* each resource: its line and GetDesc words per device, its subresources' row bytes, its
\* element ("u8n" R8G8B8A8_UNORM, "f32", "u32", "u8i" R8G8B8A8_UINT, "d24", "d32", "raw"), its
\* views (view slot -> kind, subresource, line, words per device)
V(kind, sub, line, w11, w10) == [kind |-> kind, sub |-> sub, line |-> line, w11 |-> w11, w10 |-> w10]
Tex(fmt, mips, bind) == "tex 0 2 2 1 1 " \o S(mips) \o " " \o S(fmt) \o " 0 " \o bind \o " 0 0 1 0"
Cfgs == [
    rt8 |-> [line11 |-> Tex(28, 2, "0xa0"), line10 |-> Tex(28, 2, "0x20"),
             w11 |-> "tex = 2 1 2 1 1c 1 0 0 a0 0 0", w10 |-> "tex = 2 1 2 1 1c 1 0 0 20 0 0",
             rbs |-> <<8, 4>>, el |-> "u8n",
             views |-> <<V("rtv", 0, "rtv 0 0 nodesc", "rtv = 1c 4 0", "rtv = 1c 4 0"),
                         V("rtv", 1, "rtv 1 0 28 4 1", "rtv = 1c 4 1", "rtv = 1c 4 1"),
                         V("uav", 0, "uav 2 0 nodesc", "uav = 1c 4 0", ""),
                         V("uav", 1, "uav 3 0 28 4 1", "uav = 1c 4 1", "")>>],
    f32 |-> [line11 |-> Tex(41, 1, "0xa0"), line10 |-> Tex(41, 1, "0x20"),
             w11 |-> "tex = 2 1 1 1 29 1 0 0 a0 0 0", w10 |-> "tex = 2 1 1 1 29 1 0 0 20 0 0",
             rbs |-> <<8>>, el |-> "f32",
             views |-> <<V("rtv", 0, "rtv 0 0 nodesc", "rtv = 29 4 0", "rtv = 29 4 0"),
                         V("uav", 0, "uav 1 0 nodesc", "uav = 29 4 0", "")>>],
    u32 |-> [line11 |-> Tex(42, 1, "0x80"), w11 |-> "tex = 2 1 1 1 2a 1 0 0 80 0 0",
             rbs |-> <<8>>, el |-> "u32",
             views |-> <<V("uav", 0, "uav 0 0 nodesc", "uav = 2a 4 0", "")>>],
    u8i |-> [line11 |-> Tex(30, 1, "0x80"), w11 |-> "tex = 2 1 1 1 1e 1 0 0 80 0 0",
             rbs |-> <<8>>, el |-> "u8i",
             views |-> <<V("uav", 0, "uav 0 0 nodesc", "uav = 1e 4 0", "")>>],
    d24 |-> [line11 |-> Tex(45, 1, "0x40"), line10 |-> Tex(45, 1, "0x40"),
             w11 |-> "tex = 2 1 1 1 2d 1 0 0 40 0 0", w10 |-> "tex = 2 1 1 1 2d 1 0 0 40 0 0",
             rbs |-> <<8>>, el |-> "d24",
             views |-> <<V("dsv", 0, "dsv 0 0 nodesc", "dsv = 2d 3 0 0", "dsv = 2d 3 0")>>],
    d32 |-> [line11 |-> Tex(40, 1, "0x40"), line10 |-> Tex(40, 1, "0x40"),
             w11 |-> "tex = 2 1 1 1 28 1 0 0 40 0 0", w10 |-> "tex = 2 1 1 1 28 1 0 0 40 0 0",
             rbs |-> <<8>>, el |-> "d32",
             views |-> <<V("dsv", 0, "dsv 0 0 nodesc", "dsv = 28 3 0 0", "dsv = 28 3 0")>>],
    raw |-> [line11 |-> "buf 0 16 0 0x88 0 0x20 0", w11 |-> "buf = 10 0 88 0 20 0",
             rbs |-> <<16>>, el |-> "raw",
             views |-> <<V("uav", 0, "uav 0 0 0x27 1 1 2 1", "uav = 27 1 1 2 1", "")>>]]
On(d) == IF d = "d11" THEN {"rt8", "f32", "u32", "u8i", "d24", "d32", "raw"} ELSE {"rt8", "f32", "d24", "d32"}

C == Cfgs[cfg]
D11 == dev = "d11"
\* the views made: on Direct3D 10 no unordered access view is
Made == {i \in 1 .. Len(C.views) : D11 \/ C.views[i].kind /= "uav"}
\* the view slot of a made view of `kind`
Holds(v, kind) == \E i \in Made : i - 1 = v /\ C.views[i].kind = kind
At(v) == CHOOSE i \in Made : i - 1 = v

RECURSIVE ViewLines(_)
ViewLines(i) == IF i > Len(C.views) THEN <<>>
    ELSE (IF i \in Made THEN <<"> " \o C.views[i].line, IF D11 THEN C.views[i].w11 ELSE C.views[i].w10>> ELSE <<>>) \o ViewLines(i + 1)

RECURSIVE ReadSubs(_, _)
ReadSubs(b, s) == IF s >= Len(b) THEN <<>>
    ELSE <<"> read 0 " \o S(s) \o " " \o S(C.rbs[s + 1]) \o " 1 1", "read 1 1", Hex(b[s + 1])>> \o ReadSubs(b, s + 1)

\* the subresource sq with each element of k bytes the element e (with `keep` its bytes the
\* element keeps)
Fill(sq, e) == [i \in 1 .. Len(sq) |-> e[((i - 1) % Len(e)) + 1]]
FillKeep(sq, e, keep) == [i \in 1 .. Len(sq) |-> LET j == ((i - 1) % Len(e)) + 1 IN IF j \in keep THEN sq[i] ELSE e[j]]

Init ==
    /\ dev \in {"d11", "d10"}
    /\ cfg \in On(dev)
    /\ subs = [s \in 1 .. Len(Cfgs[cfg].rbs) |-> Zeros(Cfgs[cfg].rbs[s])]
    /\ log = <<"> " \o (IF dev = "d11" THEN "device11" ELSE "device10"), (IF dev = "d11" THEN "device11" ELSE "device10") \o " ok",
               "> " \o (IF dev = "d11" THEN Cfgs[cfg].line11 ELSE Cfgs[cfg].line10),
               IF dev = "d11" THEN Cfgs[cfg].w11 ELSE Cfgs[cfg].w10>>
           \o ViewLines(1)
    /\ n = 0

Call(c, out, b) == /\ subs' = b /\ log' = log \o <<"> " \o c>> \o out \o ReadSubs(b, 0)
                   /\ UNCHANGED <<dev, cfg>> /\ n' = n + 1

\* ClearRenderTargetView of view slot v to color c
ClearRtv(v, c) ==
    LET l == "clear " \o S(v) \o " " \o Arg(c[1]) \o " " \o Arg(c[2]) \o " " \o Arg(c[3]) \o " " \o Arg(c[4])
        e == IF C.el = "u8n" THEN [i \in 1 .. 4 |-> Unorm8(c[i])] ELSE FB(c[1])
    IN IF ~Holds(v, "rtv") THEN Call(l, <<"clear noview">>, subs)
       ELSE LET s == C.views[At(v)].sub IN Call(l, <<>>, [subs EXCEPT ![s + 1] = Fill(subs[s + 1], e)])

\* ClearDepthStencilView of view slot v: flags, depth, stencil
ClearDs(v, fl, z, st) ==
    LET l == "cleards " \o S(v) \o " " \o S(fl) \o " " \o Arg(z) \o " " \o S(st)
        dz == fl % 2 = 1
        ds == fl \div 2 = 1
    IN IF ~Holds(v, "dsv") THEN Call(l, <<"cleards noview">>, subs)
       ELSE IF z \notin {"0", "1", "half"} THEN Call(l, <<>>, subs)
       ELSE IF C.el = "d24" THEN
           LET e == LE(IF z = "1" THEN 16777215 ELSE 0, 3) \o <<st % 256>>
               keep == (IF dz THEN {} ELSE {1, 2, 3}) \cup (IF ds THEN {} ELSE {4})
           IN Call(l, <<>>, [subs EXCEPT ![1] = FillKeep(subs[1], e, keep)])
       ELSE Call(l, <<>>, [subs EXCEPT ![1] = FillKeep(subs[1], FB(z), IF dz THEN {} ELSE {1, 2, 3, 4})])

\* ClearUnorderedAccessViewUint (u) or Float of view slot v
ClearUav(v, u, x) ==
    LET l == "clearuav " \o S(v) \o (IF u THEN " u " ELSE " f ")
             \o (IF u THEN x[1].arg \o " " \o x[2].arg \o " " \o x[3].arg \o " " \o x[4].arg
                 ELSE Arg(x[1]) \o " " \o Arg(x[2]) \o " " \o Arg(x[3]) \o " " \o Arg(x[4]))
    IN IF ~D11 THEN Call(l, <<"clearuav none">>, subs)
       ELSE IF ~Holds(v, "uav") THEN Call(l, <<"clearuav noview">>, subs)
       ELSE LET s == C.views[At(v)].sub IN
         IF C.el = "raw" THEN
             IF u THEN Call(l, <<>>, [subs EXCEPT ![1] = [i \in 1 .. 16 |-> IF 4 < i /\ i <= 12 THEN x[1].b[((i - 1) % 4) + 1] ELSE subs[1][i]]])
             ELSE Call(l, <<>>, subs)
         ELSE IF u THEN
             LET e == IF C.el \in {"u8n", "u8i"} THEN [i \in 1 .. 4 |-> x[i].b[1]] ELSE x[1].b
             IN Call(l, <<>>, [subs EXCEPT ![s + 1] = Fill(subs[s + 1], e)])
         ELSE IF C.el \in {"u32", "u8i"} THEN Call(l, <<>>, subs)
         ELSE LET e == IF C.el = "u8n" THEN [i \in 1 .. 4 |-> Unorm8(x[i])] ELSE FB(x[1])
              IN Call(l, <<>>, [subs EXCEPT ![s + 1] = Fill(subs[s + 1], e)])

Next ==
    /\ n < MaxOps
    /\ \/ \E v \in 0 .. 3, c \in (IF cfg = "f32" THEN FColors ELSE Colors) : ClearRtv(v, c)
       \/ \E v \in 0 .. 1, fl \in 1 .. 3, z \in (IF cfg = "d24" THEN Depths \ {"half"} ELSE Depths), st \in {7, 511} : ClearDs(v, fl, z, st)
       \/ \E v \in 0 .. 3, x \in Uints : ClearUav(v, TRUE, x)
       \/ \E v \in 0 .. 3, c \in (IF cfg = "f32" THEN FColors ELSE Colors) : ClearUav(v, FALSE, c)

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ dev \in {"d11", "d10"} /\ cfg \in On(dev) /\ Len(subs) = Len(Cfgs[cfg].rbs)
=============================================================================
