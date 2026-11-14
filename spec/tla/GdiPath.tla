---------------------------- MODULE GdiPath ----------------------------
(* GDI's path bracket on one DC, from spec/gdipath.txt: BeginPath, EndPath, AbortPath,
   CloseFigure, GetPath and the lines MoveToEx and LineTo record (the GDI.PATH clauses). TLC checks the
   invariants below on every behavior of at most MaxOps calls; `log` holds each behavior as the
   case language's lines ("> " + a call) and what the executor prints for it, and
   `kikizake tla <dump>` replays every state's log through the Rust model (spec/tla/README.md). *)
EXTENDS Integers, Sequences, TLC

CONSTANTS Coords, MaxOps

VARIABLES bracket, path, pos, log, n
vars == <<bracket, path, pos, log, n>>

PT_CLOSEFIGURE == 1
PT_LINETO == 2
PT_MOVETO == 6
Types == {PT_MOVETO, PT_LINETO, PT_LINETO + PT_CLOSEFIGURE}
Points == Coords \X Coords

S(i) == ToString(i)
P(p) == S(p[1]) \o " " \o S(p[2])
Last == path[Len(path)]
\* the last figure is closed, or there is none yet
NoOpenFigure == path = <<>> \/ Last[3] = PT_LINETO + PT_CLOSEFIGURE

Call(op, out) == log' = log \o <<"> " \o op>> \o out /\ n' = n + 1

Init == bracket = "none" /\ path = <<>> /\ pos = <<0, 0>> /\ log = <<"> dib 0 8 8 32">> /\ n = 0

\* GDI.PATH.BEGIN, BEGINAGAIN: a new empty path, the current position stays
Begin == bracket' = "open" /\ path' = <<>> /\ UNCHANGED pos /\ Call("beginpath 0", <<"beginpath 1">>)

\* GDI.PATH.END, ENDMOVE: closes an open bracket (a trailing PT_MOVETO dropped); else fails
End == IF bracket = "open"
       THEN /\ bracket' = "closed"
            /\ path' = IF path # <<>> /\ Last[3] = PT_MOVETO THEN SubSeq(path, 1, Len(path) - 1) ELSE path
            /\ UNCHANGED pos /\ Call("endpath 0", <<"endpath 1">>)
       ELSE UNCHANGED <<bracket, path, pos>> /\ Call("endpath 0", <<"endpath 0">>)

\* GDI.PATH.ABORT: no path, whatever there was; succeeds
Abort == bracket' = "none" /\ path' = <<>> /\ UNCHANGED pos /\ Call("abortpath 0", <<"abortpath 1">>)

\* GDI.PATH.CLOSE, CLOSEEMPTY
Close == IF bracket = "open"
         THEN /\ path' = IF path # <<>> /\ Last[3] = PT_LINETO THEN [path EXCEPT ![Len(path)][3] = PT_LINETO + PT_CLOSEFIGURE] ELSE path
              /\ UNCHANGED <<bracket, pos>> /\ Call("closefig 0", <<"closefig 1">>)
         ELSE UNCHANGED <<bracket, path, pos>> /\ Call("closefig 0", <<"closefig 0">>)

\* GDI.PATH.GET: a closed path's points and types; -1 without one
Get == /\ UNCHANGED <<bracket, path, pos>>
       /\ Call("getpath 0", IF bracket = "closed"
                            THEN <<"path " \o S(Len(path))>> \o [i \in 1..Len(path) |-> "pt " \o P(path[i]) \o " " \o S(path[i][3])]
                            ELSE <<"path -1">>)

\* GDI.POS.MOVE, GDI.PATH.MOVETO: the previous position returned; in a bracket a new figure
\* (a lone PT_MOVETO replaced)
MoveTo(p) == /\ pos' = p /\ UNCHANGED bracket
             /\ path' = IF bracket # "open" THEN path
                        ELSE IF path # <<>> /\ Last[3] = PT_MOVETO THEN [path EXCEPT ![Len(path)] = <<p[1], p[2], PT_MOVETO>>]
                        ELSE Append(path, <<p[1], p[2], PT_MOVETO>>)
             /\ Call("moveto 0 " \o P(p), <<"pos " \o P(pos)>>)

\* GDI.PATH.RECORD: in a bracket the line's point, a figure started at the current position
\* when none is open; outside one it is drawn
LineTo(p) == /\ pos' = p /\ UNCHANGED bracket
             /\ path' = IF bracket # "open" THEN path
                        ELSE IF NoOpenFigure THEN path \o <<<<pos[1], pos[2], PT_MOVETO>>, <<p[1], p[2], PT_LINETO>>>>
                        ELSE Append(path, <<p[1], p[2], PT_LINETO>>)
             /\ Call("lineto 0 " \o P(p), <<"ret 1">>)

Next == \/ Begin \/ End \/ Abort \/ Close \/ Get
        \/ \E p \in Points : MoveTo(p) \/ LineTo(p)

Spec == Init /\ [][Next]_vars

Bound == n <= MaxOps

TypeOK == /\ bracket \in {"none", "open", "closed"}
          /\ \A i \in 1..Len(path) : path[i][3] \in Types
          /\ pos \in Points \cup {<<0, 0>>}

\* a path starts with a figure, and a line follows a point
StartsWithMove == path # <<>> => path[1][3] = PT_MOVETO
\* MoveToEx in a row keeps the last one only
NoTwoMoves == \A i \in 1..(Len(path) - 1) : ~(path[i][3] = PT_MOVETO /\ path[i + 1][3] = PT_MOVETO)
\* EndPath leaves no PT_MOVETO at the end
ClosedEndsInLine == bracket = "closed" /\ path # <<>> => Last[3] # PT_MOVETO
\* only an open or closed bracket holds points
NoBracketNoPath == bracket = "none" => path = <<>>
\* CloseFigure follows a line
CloseAfterLine == \A i \in 1..Len(path) : path[i][3] = PT_LINETO + PT_CLOSEFIGURE => i > 1
=============================================================================
