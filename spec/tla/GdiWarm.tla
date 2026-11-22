---------------------------- MODULE GdiWarm ----------------------------
(* LPtoDP on 32-bit Windows, from spec/gdixform.txt: an LPtoDP after another with the mapping
   unchanged truncates the scaled point toward zero to 28.4, the first after a change rounds it
   (GDI.MAP.ROUNDX86); through a world transform the same after an LPtoDP or a multiplying
   ModifyWorldTransform, SetWorldTransform leaving it cold (GDI.XFORM.WARMX86); MM_ISOTROPIC's
   points through the viewport extents of GDI.MAP.ISOPOINTS (GDI.MAP.ISOLP); an extent of 0
   (GDI.MAP.ZEROEXT): MM_ANISOTROPIC keeps the extents, MM_ISOTROPIC fails, and an isotropic
   shrink to 0 leaves DPtoLP failing and LPtoDP mapping that axis to the origin. The page rule
   (GDI.MAP.ROUND) is integer arithmetic; the world transforms here have entries k / 32 and
   are asked only over window extents of 1, 2 or 4, so every f32 step of GDI.XFORM.WORLDPOINT
   is exact and the world is kept in units of 2^-15. Every behavior takes at most MaxOps calls;
   `kikizake tla` replays every state's log through the Rust model as the 32-bit executor
   (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES mode, wext, vext, worg, vorg, unsettled, gmode, world, warm, log, n
vars == <<mode, wext, vext, worg, vorg, unsettled, gmode, world, warm, log, n>>

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
Abs(a) == IF a < 0 THEN -a ELSE a
\* a / b toward zero, halves away from zero, and n / d floored halves up (fix), b # 0
Trunc(a, b) == IF (a < 0) = (b < 0) THEN Abs(a) \div Abs(b) ELSE -(Abs(a) \div Abs(b))
RoundAway(a, b) == LET q == (2 * Abs(a) + Abs(b)) \div (2 * Abs(b)) IN IF (a < 0) = (b < 0) THEN q ELSE -q
Fix(a, b) == IF b < 0 THEN (32 * (-a) + (-b)) \div (2 * (-b)) ELSE (32 * a + b) \div (2 * b)

U == 32768
Identity == <<U, 0, 0, U, 0, 0>>
\* CombineTransform's product (the first, then the second); TLC stops on a product these units
\* cannot hold (f32 holds every one of them exactly)
Mul(a, b) == LET m(x, y) == IF (x * y) % U = 0 THEN (x * y) \div U ELSE Assert(FALSE, "a product off the 2^-15 grid") IN
             <<m(a[1], b[1]) + m(a[2], b[3]), m(a[1], b[2]) + m(a[2], b[4]), m(a[3], b[1]) + m(a[4], b[3]),
               m(a[3], b[2]) + m(a[4], b[4]), m(a[5], b[1]) + m(a[6], b[3]) + b[5], m(a[5], b[2]) + m(a[6], b[4]) + b[6]>>
Xforms == {<<"0.5 0 0 1 0.03125 0", <<16384, 0, 0, 32768, 1024, 0>>>>,
           <<"0 1 -1 0 0 0.5", <<0, 32768, -32768, 0, 0, 16384>>>>,
           <<"0.03125 0 0 0.5 0 0", <<1024, 0, 0, 16384, 0, 0>>>>}

\* GDI.MAP.ISOTROPIC: the axis of the larger scale shrunk, v |the other scale / its own|,
\* floor(r + 1/2)
\* (v1 |v2 / w2| / |v1 / w1| = sign(v1) |v2 w1| / |w2|)
Sign(a) == IF a < 0 THEN -1 ELSE 1
Shrunk(v, w) ==
  IF Abs(v[1] * w[2]) > Abs(v[2] * w[1])
  THEN <<(2 * Sign(v[1]) * Abs(v[2] * w[1]) + Abs(w[2])) \div (2 * Abs(w[2])), v[2]>>
  ELSE IF v[2] = 0 THEN v
  ELSE <<v[1], (2 * Sign(v[2]) * Abs(v[1] * w[2]) + Abs(w[1])) \div (2 * Abs(w[1]))>>
VE(m, v, u) == IF m = 7 /\ u THEN Shrunk(v, wext) ELSE v
\* GDI.MAP.ISOPOINTS: points through the stored extents shrunk again when that shrinks y
Points(v) == IF mode # 7 THEN v ELSE LET s == Shrunk(v, wext) IN IF s[1] = v[1] THEN s ELSE v

\* GDI.MAP.ROUND, ROUNDX86: one axis, the scaled point to 28.4 (truncated when warm), the offset
\* to 28.4, their sum to the pixel halves up
Page(a, w, v, wo, vo, t) == ((IF t THEN Trunc(16 * a * v, w) ELSE Fix(a * v, w)) + Fix(vo * w - wo * v, w) + 8) \div 16
\* GDI.XFORM.WORLDPOINT: the point through world times page, exactly, times 16 (truncated when
\* warm), the offset times 16, each rounded halves away; then (v + 8) >> 4
World(p, e, t) ==
  LET ax(i, a, b, d) == LET num == 16 * (p[1] * world[a] + p[2] * world[b]) * e[i]
                            den == U * wext[i]
                        IN ((IF t THEN Trunc(num, den) ELSE RoundAway(num, den))
                            + RoundAway(16 * (world[d] * e[i] + U * vorg[i] * wext[i] - U * worg[i] * e[i]), U * wext[i]) + 8) \div 16
  IN <<ax(1, 1, 3, 5), ax(2, 2, 4, 6)>>
Map(p, e, t) == IF world = Identity THEN <<Page(p[1], wext[1], e[1], worg[1], vorg[1], t), Page(p[2], wext[2], e[2], worg[2], vorg[2], t)>> ELSE World(p, e, t)

Init == /\ mode = 1 /\ wext = <<1, 1>> /\ vext = <<1, 1>> /\ worg = <<0, 0>> /\ vorg = <<0, 0>> /\ unsettled = FALSE
        /\ gmode = 1 /\ world = Identity /\ warm = FALSE /\ n = 0 /\ log = <<"> dib 0 8 8 32">>

Say(lines) == log' = log \o lines /\ n' = n + 1
Cold == warm' = FALSE

\* the mapping's calls (GdiMap.tla): each leaves it cold
SetMode(v) == /\ Say(<<"> mapmode 0 " \o S(v), "mapmode " \o S(mode)>>) /\ mode' = v /\ unsettled' = TRUE /\ Cold
              /\ IF v = 1 THEN wext' = <<1, 1>> /\ vext' = <<1, 1>>
                 ELSE IF v = 7 THEN wext' = <<2709, 2032>> /\ vext' = <<1024, -768>>
                 ELSE UNCHANGED <<wext, vext>>
              /\ UNCHANGED <<worg, vorg, gmode, world>>
\* GDI.MAP.ZEROEXT: MM_ISOTROPIC fails on a 0, the extents written as they were; MM_ANISOTROPIC
\* succeeds keeping them
SetExt(win, e) ==
  LET old == IF win THEN wext ELSE vext
      line == "> " \o (IF win THEN "winext" ELSE "vpext") \o " 0 " \o P(e)
      zero == e[1] = 0 \/ e[2] = 0
  IN /\ Cold /\ UNCHANGED <<mode, worg, vorg, gmode, world>>
     /\ IF mode = 7 /\ zero THEN Say(<<line, "ext 0 " \o P(old)>>) /\ UNCHANGED <<wext, vext, unsettled>>
        ELSE /\ Say(<<line, "ext 1 " \o P(old)>>) /\ unsettled' = TRUE
             /\ IF mode >= 7 /\ ~zero THEN (IF win THEN wext' = e /\ UNCHANGED vext ELSE vext' = e /\ UNCHANGED wext)
                ELSE UNCHANGED <<wext, vext>>
SetOrg(win, o) == /\ Say(<<"> " \o (IF win THEN "winorg" ELSE "vporg") \o " 0 " \o P(o), "org 1 " \o P(IF win THEN worg ELSE vorg)>>)
                  /\ IF win THEN worg' = o /\ UNCHANGED vorg ELSE vorg' = o /\ UNCHANGED worg
                  /\ Cold /\ UNCHANGED <<mode, wext, vext, unsettled, gmode, world>>
GMode == /\ Say(<<"> gmode 0 2", "gmode " \o S(gmode)>>) /\ gmode' = 2 /\ Cold
         /\ UNCHANGED <<mode, wext, vext, worg, vorg, unsettled, world>>
\* GDI.XFORM.WARMX86: SetWorldTransform leaves it cold; a multiplying ModifyWorldTransform warm,
\* MWT_IDENTITY cold
SetX(x) == /\ Say(<<"> setxform 0 " \o x[1], IF gmode = 2 THEN "setxform 1" ELSE "setxform 0">>)
           /\ world' = (IF gmode = 2 THEN x[2] ELSE world) /\ Cold
           /\ UNCHANGED <<mode, wext, vext, worg, vorg, unsettled, gmode>>
ModX(k, x) == /\ Say(<<"> modxform 0 " \o S(k) \o " " \o x[1], IF gmode = 2 THEN "modxform 1" ELSE "modxform 0">>)
              /\ IF gmode # 2 THEN world' = world /\ Cold
                 ELSE IF k = 1 THEN world' = Identity /\ Cold
                 ELSE world' = (IF k = 2 THEN Mul(x[2], world) ELSE Mul(world, x[2])) /\ warm' = TRUE
              /\ UNCHANGED <<mode, wext, vext, worg, vorg, unsettled, gmode>>

\* LPtoDP: the mapping read (the shrunk extents stored), each point mapped, warm after it;
\* asked through a world transform only over window extents its f32 steps keep exact
\* (10 and 30 over 3 / 64 and 1 / 64, 15 through 1 / 32: 7.5 sixteenths, so truncating
\* and rounding land on different pixels)
Pts == <<<<1, 3>>, <<-5, 2>>, <<10, 30>>, <<15, 15>>>>
PtsText == " 1 3 -5 2 10 30 15 15"
LPtoDP ==
  LET v == VE(mode, vext, unsettled)
      e == Points(v)
      q(i) == Map(Pts[i], e, warm)
  IN /\ world = Identity \/ (wext[1] \in {1, 2, 4} /\ wext[2] \in {1, 2, 4})
     /\ Say(<<"> lptodp 0" \o PtsText, "pts 1 " \o P(q(1)) \o " " \o P(q(2)) \o " " \o P(q(3)) \o " " \o P(q(4))>>)
     /\ vext' = v /\ unsettled' = FALSE /\ warm' = TRUE
     /\ UNCHANGED <<mode, wext, worg, vorg, gmode, world>>
\* GDI.MAP.ZEROEXT: a shrink to 0 leaves no inverse: DPtoLP fails, the points as they were
DPtoLP ==
  LET v == VE(mode, vext, unsettled)
      e == Points(v)
  IN /\ world = Identity /\ (e[1] = 0 \/ e[2] = 0)
     /\ Say(<<"> dptolp 0" \o PtsText, "pts 0" \o PtsText>>)
     /\ vext' = v /\ unsettled' = FALSE
     /\ UNCHANGED <<mode, wext, worg, vorg, gmode, world, warm>>

\* (no call past MaxOps: the last states have no successors, CHECK_DEADLOCK off)
Next == /\ n < MaxOps
        /\ \/ \E v \in {1, 7, 8} : SetMode(v)
           \/ \E e \in {<<4, 2>>, <<1, 64>>, <<64, 64>>, <<0, 4>>} : SetExt(TRUE, e)
           \/ \E e \in {<<3, -1>>, <<1, 1>>, <<0, 2>>} : SetExt(FALSE, e)
           \/ SetOrg(TRUE, <<1, -2>>) \/ SetOrg(FALSE, <<3, 1>>)
           \/ GMode
           \/ \E x \in Xforms : SetX(x) \/ ModX(2, x) \/ ModX(3, x)
           \/ ModX(1, <<"1 0 0 1 0 0", Identity>>)
           \/ LPtoDP \/ DPtoLP

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

\* only a call after an LPtoDP or a multiplying ModifyWorldTransform finds it warm
TypeOK == warm \in BOOLEAN /\ mode \in {1, 7, 8} /\ wext[1] # 0 /\ wext[2] # 0
=============================================================================
