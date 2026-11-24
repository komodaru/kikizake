--------------------------- MODULE GdiPathEmpty ---------------------------
EXTENDS GdiPath

\* Finish an open bracket after one zero-width or zero-height shape. GetPath and
\* GetCurrentPositionEx observe the inherited state, including a trailing move.
EmptyBox(op, box, args) ==
  LET saved == IF path # <<>> /\ Last[3] = PT_MOVETO THEN SubSeq(path, 1, Len(path) - 1) ELSE path
      pts == [i \in 1..Len(saved) |-> "pt " \o P(saved[i]) \o " " \o S(saved[i][3])]
  IN /\ bracket = "open" /\ bracket' = "closed" /\ path' = saved /\ UNCHANGED pos
     /\ Call(op \o " 0 " \o box \o args,
             <<"ret 1", "> getpos 0", "pos " \o P(pos), "> endpath 0", "endpath 1",
               "> getpath 0", "path " \o S(Len(saved))>> \o pts)

EmptyNext == /\ n < MaxOps
             /\ \/ /\ bracket = "none" /\ Begin
                \/ /\ bracket = "open"
                   /\ \/ Close
                      \/ \E p \in Points : MoveTo(p) \/ LineTo(p)
                      \/ \E box \in {"2 1 2 2", "2 1 1 1", "2 2 2 2"} :
                           \/ \E op \in {"rectangle", "ellipse"} : EmptyBox(op, box, "")
                           \/ \E args \in {" 0 5", " -2 4", " 4 4"} : EmptyBox("roundrect", box, args)
                           \/ \E op \in {"arc", "arcto", "pie", "chord"} : EmptyBox(op, box, " 1 2 3 4")
=============================================================================
