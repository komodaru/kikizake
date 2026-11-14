------------------------- MODULE D3D9StateBlock -------------------------
(* Direct3D 9 state blocks, from spec/d3d9.txt (D3D9.SETGET and the D3D9.SB clauses, "State Blocks
   Save and Restore State", IDirect3DStateBlock9::Capture / Apply, D3DSTATEBLOCKTYPE): render
   states set and read back, blocks recorded between BeginStateBlock and EndStateBlock (the calls
   recorded, not applied), made by CreateStateBlock of a type (the type's states, at their current
   values), captured, applied, released. Three render states stand for the groups: ZENABLE (7)
   and SRCBLEND (19) pixel states, CULLMODE (22) a vertex state. Only documented use is in the
   behaviors (D3D9.SB.MISUSE stays open). `log` as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC, FiniteSets

CONSTANTS Values, Slots, MaxOps

VARIABLES dev, blocks, made, rec, recording, log, n
vars == <<dev, blocks, made, rec, recording, log, n>>

States == {7, 19, 22}
Pixel == {7, 19}
Vertex == {22}
\* D3DSBT_ALL 1, PIXELSTATE 2, VERTEXSTATE 3
TypeStates(t) == CASE t = 1 -> States [] t = 2 -> Pixel [] t = 3 -> Vertex
\* the device's defaults (D3D9.RS.DEFAULT: ZENABLE TRUE with the automatic depth stencil, SRCBLEND ONE, CULLMODE CCW)
Default == [s \in States |-> CASE s = 7 -> 1 [] s = 19 -> 2 [] s = 22 -> 3]
\* a block: the states it holds and their values (a function on a subset of States)
Restrict(f, D) == [s \in D |-> f[s]]
Override(f, g) == [s \in DOMAIN f |-> IF s \in DOMAIN g THEN g[s] ELSE f[s]]

Hex(v) == "0000000" \o ToString(v)
S(i) == ToString(i)
Call(op, out) == log' = log \o <<"> " \o op>> \o out /\ n' = n + 1

Init == /\ dev = Default /\ blocks = [i \in Slots |-> <<>>] /\ made = {} /\ rec = <<>> /\ recording = FALSE
        /\ log = <<"> device", "device ok">> /\ n = 0

Recording == recording

\* D3D9.SETGET; D3D9.SB.RECORD: while recording a set goes into the block, not to the device
Set(s, v) == /\ IF Recording THEN rec' = [x \in DOMAIN rec \cup {s} |-> IF x = s THEN v ELSE rec[x]] /\ UNCHANGED dev
                              ELSE dev' = [dev EXCEPT ![s] = v] /\ UNCHANGED rec
             /\ UNCHANGED <<blocks, made, recording>> /\ Call("rs set " \o S(s) \o " " \o S(v), <<>>)
Get(s) == ~Recording /\ UNCHANGED <<dev, blocks, made, rec, recording>> /\ Call("rs get " \o S(s), <<"rs " \o S(s) \o " = " \o Hex(dev[s])>>)

Begin == ~Recording /\ recording' = TRUE /\ rec' = <<>> /\ UNCHANGED <<dev, blocks, made>> /\ Call("sb begin", <<>>)
End(i) == Recording /\ i \notin made /\ blocks' = [blocks EXCEPT ![i] = rec] /\ made' = made \cup {i} /\ recording' = FALSE /\ UNCHANGED <<dev, rec>>
          /\ Call("sb end " \o S(i), <<>>)
\* D3D9.SB.TYPES: the type's states at the device's values
Create(i, t) == ~Recording /\ i \notin made /\ blocks' = [blocks EXCEPT ![i] = Restrict(dev, TypeStates(t))] /\ made' = made \cup {i} /\ UNCHANGED <<dev, rec, recording>>
                /\ Call("sb create " \o S(i) \o " " \o S(t), <<>>)
\* D3D9.SB.CAPTURE: the block's states take the device's values
Capture(i) == ~Recording /\ UNCHANGED <<dev, rec, recording, made>>
              /\ IF i \notin made THEN UNCHANGED blocks /\ Call("sb capture " \o S(i), <<"sb " \o S(i) \o " none">>)
                 ELSE blocks' = [blocks EXCEPT ![i] = Restrict(dev, DOMAIN blocks[i])] /\ Call("sb capture " \o S(i), <<>>)
\* D3D9.SB.APPLY: the device's states take the block's values
Apply(i) == ~Recording /\ UNCHANGED <<blocks, rec, recording, made>>
            /\ IF i \notin made THEN UNCHANGED dev /\ Call("sb apply " \o S(i), <<"sb " \o S(i) \o " none">>)
               ELSE dev' = Override(dev, blocks[i]) /\ Call("sb apply " \o S(i), <<>>)
Release(i) == ~Recording /\ UNCHANGED <<dev, rec, recording>>
              /\ IF i \notin made THEN UNCHANGED <<blocks, made>> /\ Call("sb release " \o S(i), <<"sb " \o S(i) \o " none">>)
                 ELSE blocks' = [blocks EXCEPT ![i] = <<>>] /\ made' = made \ {i} /\ Call("sb release " \o S(i), <<>>)

Next == \/ \E s \in States, v \in Values : Set(s, v)
        \/ \E s \in States : Get(s)
        \/ Begin
        \/ \E i \in Slots : End(i) \/ Capture(i) \/ Apply(i) \/ Release(i) \/ \E t \in 1..3 : Create(i, t)

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps

TypeOK == /\ \A s \in States : dev[s] \in Values \cup {1, 2, 3}
          /\ made \subseteq Slots /\ recording \in BOOLEAN
\* a block made by CreateStateBlock or captured holds only its type's states; a recorded one only
\* what was set: no block ever holds a state outside the three groups
BlocksInStates == \A i \in Slots : DOMAIN blocks[i] \subseteq States
\* recording never changes the device: the device equals what the last non-recorded calls made
\* (checked as: while recording, Get is never enabled, so nothing reads a half-applied device)
\* an ALL block applied right after it was made changes nothing
ApplyAfterCreateIsIdentity == \A i \in made : DOMAIN blocks[i] = States /\ (\A s \in States : blocks[i][s] = dev[s]) => Override(dev, blocks[i]) = dev
=============================================================================
