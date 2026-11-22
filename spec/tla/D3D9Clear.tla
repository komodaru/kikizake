------------------------------ MODULE D3D9Clear ------------------------------
(* Direct3D 9's Clear and what bounds it (D3D9.TARGET, D3D9.CLEAR, D3D9.CLEAR.FLAGS,
   D3D9.CLEAR.BACKBUFFER, D3D9.CLEAR.COLOR, D3D9.SCISSOR; golden d3d9draw clear-*,
   clear-viewport-*, scissor-* cases): on a 3 by 2 A8R8G8B8 target, Clear with no rectangle fills the
   viewport, else each rectangle clipped to it and, with D3DRS_SCISSORTESTENABLE (174), to the
   scissor rectangle; flags with none of TARGET, ZBUFFER, STENCIL are D3DERR_INVALIDCALL; ZBUFFER
   and STENCIL alone leave the pixels; SetRenderTarget resets the viewport and the scissor
   rectangle to the new target, whose pixels are not defined until cleared (no read before); a
   target of a format no target takes (P8) fails, and Clear then goes to the device's own back
   buffer, which is not read. A draw keeps to the scissor rectangle as Clear does. The color is written as its 8-bit channels, bytes B G R A. `log`
   as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES rt, px, vp, sc, on, log, n
vars == <<rt, px, vp, sc, on, log, n>>

W == 3
H == 2
Pixels == (0 .. W - 1) \X (0 .. H - 1)
\* a color: the argument Clear takes and the pixel's bytes as d9rt prints them
Colors == <<[arg |-> "0x0", hex |-> "00000000"], [arg |-> "0xff4080c0", hex |-> "c08040ff"],
            [arg |-> "0x12345678", hex |-> "78563412"]>>
\* a rectangle: left, top, right, bottom (right and bottom outside); None for none set
None == <<>>
Whole == <<0, 0, W, H>>
Viewports == {<<1, 0, 2, 2>>, <<0, 1, 3, 1>>}
Scissors == {<<1, 0, 3, 1>>, <<2, 1, 5, 4>>}
RectLists == {<<>>, <<<<0, 0, 2, 1>>>>, <<<<-1, 1, 1, 3>>, <<2, 0, 4, 2>>>>, <<<<2, 1, 1, 2>>>>}
Undefined == 0
Inside(r, p) == r[1] <= p[1] /\ p[1] < r[3] /\ r[2] <= p[2] /\ p[2] < r[4]
S(i) == ToString(i)
Rect(r) == S(r[1]) \o " " \o S(r[2]) \o " " \o S(r[3]) \o " " \o S(r[4])
RECURSIVE Rects(_)
Rects(l) == IF l = <<>> THEN "" ELSE " " \o Rect(Head(l)) \o Rects(Tail(l))
Call(op, out) == log' = log \o <<"> " \o op>> \o out /\ n' = n + 1

Init == /\ rt = TRUE /\ px = [p \in Pixels |-> 1] /\ vp = None /\ sc = None /\ on = FALSE /\ n = 0
        /\ log = <<"> device swvp", "device ok", "> d9target 3 2 21", "> d9clear 1 0x0 0 0">>

\* (21 A8R8G8B8 makes a target, 40 P8 none)
Target(f) == /\ UNCHANGED on /\ vp' = None /\ sc' = None
             /\ IF f = 21 THEN rt' = TRUE /\ px' = [p \in Pixels |-> Undefined]
                ELSE rt' = FALSE /\ UNCHANGED px
             /\ Call("d9target 3 2 " \o S(f), IF f = 21 THEN <<>> ELSE <<"target failed 8876086c">>)
\* (a viewport as SetViewport takes it: x, y, width, height)
Viewport(v) == /\ rt /\ UNCHANGED <<rt, px, sc, on>> /\ vp' = <<v[1], v[2], v[1] + v[3], v[2] + v[4]>>
               /\ Call("d9viewport " \o Rect(v) \o " 0 1065353216", <<>>)
Scissor(r) == /\ UNCHANGED <<rt, px, vp, on>> /\ sc' = r /\ Call("d9scissor " \o Rect(r), <<>>)
Enable(b) == /\ UNCHANGED <<rt, px, vp, sc>> /\ on' = b /\ Call("rs set 174 " \o (IF b THEN "1" ELSE "0"), <<>>)
Bounds(p) == /\ Inside(IF vp = None THEN Whole ELSE vp, p)
             /\ (on /\ sc /= None) => Inside(sc, p)
Cleared(l, p) == /\ Inside(Whole, p) /\ Bounds(p)
                 /\ (l = <<>> \/ \E i \in 1 .. Len(l) : Inside(l[i], p))
Clear(fl, c, l) == /\ UNCHANGED <<rt, vp, sc, on>>
                   /\ IF fl = 0 \/ ~rt \/ fl % 2 = 0 THEN UNCHANGED px
                      ELSE px' = [p \in Pixels |-> IF Cleared(l, p) THEN c ELSE px[p]]
                   /\ Call("d9clear " \o S(fl) \o " " \o Colors[c].arg \o " 0 0" \o Rects(l),
                           IF fl = 0 THEN <<"clear hr 8876086c">> ELSE <<>>)
\* a strip of two triangles over the whole target, pretransformed (XYZRHW, DIFFUSE), of color 2:
\* with the scissor test on, only the pixels in the scissor rectangle (the viewport left whole,
\* as the scissor-* cases draw)
Corner(x, y) == x \o " " \o y \o " 0x0 0x3f800000 0xff4080c0"
Draw == /\ rt /\ vp = None /\ UNCHANGED <<rt, vp, sc, on>>
        /\ px' = [p \in Pixels |-> IF (on /\ sc /= None) => Inside(sc, p) THEN 2 ELSE px[p]]
        /\ log' = log \o <<"> d9fvf 0x44", "> d9draw 5 4 " \o Corner("0xbf000000", "0xbf000000") \o " "
              \o Corner("0x40200000", "0xbf000000") \o " " \o Corner("0xbf000000", "0x3fc00000") \o " "
              \o Corner("0x40200000", "0x3fc00000")>>
        /\ n' = n + 1
RECURSIVE Row(_, _)
Row(x, y) == IF x = W THEN "" ELSE Colors[px[<<x, y>>]].hex \o Row(x + 1, y)
Read == /\ UNCHANGED <<rt, px, vp, sc, on>>
        /\ (rt => \A p \in Pixels : px[p] /= Undefined)
        /\ Call("d9rt", IF rt THEN <<"px rt 3 2 32", Row(0, 0), Row(0, 1)>> ELSE <<"rt none">>)

Next == \/ \E f \in {21, 40} : Target(f)
        \/ \E v \in Viewports : Viewport(v)
        \/ \E r \in Scissors : Scissor(r)
        \/ \E b \in BOOLEAN : Enable(b)
        \/ \E fl \in {0, 1, 2, 5}, c \in 2 .. 3, l \in RectLists : Clear(fl, c, l)
        \/ Draw
        \/ Read

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ rt \in BOOLEAN /\ px \in [Pixels -> 0 .. 3] /\ on \in BOOLEAN /\ n \in 0 .. MaxOps + 1
=============================================================================
