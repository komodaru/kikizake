---------------------------- MODULE GdiMap ----------------------------
(* A device context's mapping mode, from spec/gdi.txt (GDI.MAP clauses below): SetMapMode,
   SetWindowExtEx, SetViewportExtEx, SetWindowOrgEx, OffsetViewportOrgEx, ScaleWindowExtEx,
   ScaleViewportExtEx, and the state they read back (GetMapMode, GetWindowExtEx,
   GetViewportExtEx, GetWindowOrgEx, GetViewportOrgEx as one call, getmap). The state is the
   mode, the window and viewport extents and origins, and whether MM_ISOTROPIC's shrunk
   viewport extents are still to be stored (GDI.MAP.ISOTROPIC: the first read after a change
   stores them). Every behavior takes at most MaxOps calls on a fresh DC of the 1024 by 768
   display; `kikizake tla` replays every state's log through the Rust model
   (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES mode, wext, vext, worg, vorg, unsettled, log, n
vars == <<mode, wext, vext, worg, vorg, unsettled, log, n>>

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
Abs(a) == IF a < 0 THEN -a ELSE a
\* a / b truncated toward zero, b # 0
Trunc(a, b) == IF (a < 0) = (b < 0) THEN Abs(a) \div Abs(b) ELSE -(Abs(a) \div Abs(b))
\* N / D to the nearest, halves up, D > 0
Near(N, D) == (2 * N + D) \div (2 * D)

\* the display
SW == 1024
SH == 768

\* GDI.MAP.METRIC: MM_LOMETRIC, MM_HIMETRIC, MM_LOENGLISH, MM_HIENGLISH, MM_TWIPS, MM_ISOTROPIC:
\* the window extents the display's size in the mode's units at 96 pixels an inch (rounded), the
\* viewport's its pixels, y up
Units(m) == <<254, 2540, 100, 1000, 1440, 254>>[m - 1]

\* GDI.MAP.ISOTROPIC: the viewport extents with the axis of the larger scale shrunk so that a
\* logical unit is as wide as high: v times |the other axis' scale / its own|, to the nearest
Shrunk(v, w) ==
  IF Abs(v[1]) * Abs(w[2]) > Abs(v[2]) * Abs(w[1])
  THEN <<IF v[1] < 0 THEN -Near(Abs(v[2] * w[1]), Abs(w[2])) ELSE Near(Abs(v[2] * w[1]), Abs(w[2])), v[2]>>
  ELSE <<v[1], IF v[2] < 0 THEN -Near(Abs(v[1] * w[2]), Abs(w[1])) ELSE Near(Abs(v[1] * w[2]), Abs(w[1]))>>
\* the viewport extents in effect
VE == IF mode = 7 /\ unsettled THEN Shrunk(vext, wext) ELSE vext

Init == /\ mode = 1 /\ wext = <<1, 1>> /\ vext = <<1, 1>> /\ worg = <<0, 0>> /\ vorg = <<0, 0>>
        /\ unsettled = FALSE /\ n = 0
        /\ log = <<"> dib 0 8 8 32">>

Say(lines) == log' = log \o lines

\* GDI.MAP.MODE, METRIC: the previous mode, 0 for one outside MM_TEXT .. MM_ANISOTROPIC; MM_TEXT's
\* extents 1, MM_ANISOTROPIC keeps the extents it finds
SetMode(v) ==
  /\ Say(<<"> mapmode 0 " \o S(v), "mapmode " \o (IF v \in 1..8 THEN S(mode) ELSE "0")>>)
  /\ IF v \in 1..8
     THEN /\ mode' = v
          /\ unsettled' = TRUE
          /\ IF v = 1 THEN wext' = <<1, 1>> /\ vext' = <<1, 1>>
             ELSE IF v \in 2..7 THEN wext' = <<Near(SW * Units(v), 96), Near(SH * Units(v), 96)>> /\ vext' = <<SW, -SH>>
             ELSE UNCHANGED <<wext, vext>>
     ELSE UNCHANGED <<mode, wext, vext, unsettled>>
  /\ UNCHANGED <<worg, vorg>>

\* GDI.MAP.EXT, ZEROEXT, FIXEDEXT: the extents stored before (unshrunk); MM_ISOTROPIC refuses a 0
\* (failing, nothing changed), MM_ANISOTROPIC takes it as the extents as they were, the fixed
\* modes ignore the call, succeeding
SetExt(win, e) ==
  LET old == IF win THEN wext ELSE vext
      line == "> " \o (IF win THEN "winext" ELSE "vpext") \o " 0 " \o P(e)
      zero == e[1] = 0 \/ e[2] = 0
  IN IF mode = 7 /\ zero
     THEN Say(<<line, "ext 0 " \o P(old)>>) /\ UNCHANGED <<mode, wext, vext, worg, vorg, unsettled>>
     ELSE /\ Say(<<line, "ext 1 " \o P(old)>>)
          /\ unsettled' = TRUE
          /\ IF mode >= 7 /\ ~zero
             THEN IF win THEN wext' = e /\ UNCHANGED vext ELSE vext' = e /\ UNCHANGED wext
             ELSE UNCHANGED <<wext, vext>>
          /\ UNCHANGED <<mode, worg, vorg>>

\* GDI.MAP.ORG: set or moved in any mode; the previous origin
SetOrg(win, offset, o) ==
  LET old == IF win THEN worg ELSE vorg
      new == IF offset THEN <<old[1] + o[1], old[2] + o[2]>> ELSE o
      line == "> " \o (IF offset THEN "offset" ELSE "") \o (IF win THEN "win" ELSE "vp") \o (IF offset THEN "" ELSE "org") \o " 0 " \o P(o)
  IN /\ Say(<<line, "org 1 " \o P(old)>>)
     /\ IF win THEN worg' = new /\ UNCHANGED vorg ELSE vorg' = new /\ UNCHANGED worg
     /\ UNCHANGED <<mode, wext, vext, unsettled>>

\* GDI.MAP.SCALE (a read: the shrunk extents stored first): the fixed modes keep their extents,
\* succeeding; else each extent times num / denom truncated toward zero; a 0 among the four, or
\* an extent that comes out 0, fails and changes nothing (ScaleWindowExtEx writing nothing,
\* ScaleViewportExtEx the old extents)
Scale(win, s) ==
  LET v == VE
      old == IF win THEN wext ELSE v
      line == "> " \o (IF win THEN "scalewin" ELSE "scalevp") \o " 0 " \o S(s[1]) \o " " \o S(s[2]) \o " " \o S(s[3]) \o " " \o S(s[4])
      bad == s[1] = 0 \/ s[2] = 0 \/ s[3] = 0 \/ s[4] = 0
      new == IF bad THEN <<0, 0>> ELSE <<Trunc(old[1] * s[1], s[2]), Trunc(old[2] * s[3], s[4])>>
      fails == bad \/ new[1] = 0 \/ new[2] = 0
  IN IF mode < 7
     THEN Say(<<line, "ext 1 " \o P(old)>>) /\ vext' = v /\ unsettled' = FALSE /\ UNCHANGED <<mode, wext, worg, vorg>>
     ELSE IF fails
     THEN Say(<<line, IF win THEN "ext 0 -1 -1" ELSE "ext 0 " \o P(old)>>) /\ vext' = v /\ unsettled' = FALSE
          /\ UNCHANGED <<mode, wext, worg, vorg>>
     ELSE /\ Say(<<line, "ext 1 " \o P(old)>>)
          /\ IF win THEN wext' = new /\ vext' = v ELSE vext' = new /\ UNCHANGED wext
          /\ unsettled' = TRUE
          /\ UNCHANGED <<mode, worg, vorg>>

\* GDI.MAP.GET, ISOTROPIC: the state as set, the viewport extents in effect (now stored)
Get ==
  /\ Say(<<"> getmap 0", "map " \o S(mode) \o " 1 wext " \o P(wext) \o " vext " \o P(VE) \o " worg " \o P(worg) \o " vorg " \o P(vorg)>>)
  /\ vext' = VE /\ unsettled' = FALSE
  /\ UNCHANGED <<mode, wext, worg, vorg>>

Next == /\ n' = n + 1
        /\ \/ \E v \in {1, 2, 7, 8, 9} : SetMode(v)
           \/ \E e \in {<<10, 20>>, <<-30, 40>>, <<0, 5>>} : SetExt(TRUE, e)
           \/ \E e \in {<<30, -40>>, <<7, 3>>, <<0, 5>>} : SetExt(FALSE, e)
           \/ SetOrg(TRUE, FALSE, <<2, -3>>) \/ SetOrg(FALSE, TRUE, <<1, 1>>)
           \/ \E w \in BOOLEAN, s \in {<<2, 1, 3, 1>>, <<1, 3, 1, 2>>, <<0, 1, 1, 1>>} : Scale(w, s)
           \/ Get

Bound == n <= MaxOps

Spec == Init /\ [][Next]_vars

\* extents are never 0 in the modes that keep them
NoZero == mode >= 7 => wext[1] # 0 /\ wext[2] # 0
=============================================================================
