---------------------------- MODULE D3DSbMisuse ----------------------------
(* Direct3D 9 and 8 state blocks used out of turn, as Windows answers (D3D9.SB.MISUSE,
   D3D9.SB.BADTYPE; golden d3d9, d3d8 edge-end-without-begin, edge-begin-twice,
   edge-calls-while-recording, edge-type-0, edge-type-4, edge-release-while-recording):
   BeginStateBlock while recording, EndStateBlock without Begin, and Apply, Capture,
   CreateStateBlock while recording fail with D3DERR_INVALIDCALL and change nothing (the
   recording goes on); CreateStateBlock of type 0 makes a block of no state, of a type past 3
   fails and makes none; Release while recording forgets the block, and on Direct3D 8 also
   fails. ZENABLE (7, a pixel state, default TRUE) stands for the states: D3D9StateBlock.tla
   covers the documented use. `log` as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS Values, Slots, MaxOps

VARIABLES v8, dev, blocks, made, rec, recording, log, n
vars == <<v8, dev, blocks, made, rec, recording, log, n>>

HR == " hr 8876086c"
S(i) == ToString(i)
Hex(v) == "0000000" \o ToString(v)
\* a block: ZENABLE's value it holds, or -1 for a block of no state
None == -1
Call(op, out) == log' = log \o <<"> " \o op>> \o out /\ n' = n + 1

Init == /\ v8 \in BOOLEAN /\ dev = 1 /\ blocks = [i \in Slots |-> None] /\ made = {} /\ rec = None
        /\ recording = FALSE /\ n = 0
        /\ log = IF v8 THEN <<"> device8 swvp", "device8 ok">> ELSE <<"> device swvp", "device ok">>

Set(v) == /\ IF recording THEN rec' = v /\ UNCHANGED dev ELSE dev' = v /\ UNCHANGED rec
          /\ UNCHANGED <<v8, blocks, made, recording>> /\ Call("rs set 7 " \o S(v), <<>>)
Get == ~recording /\ UNCHANGED <<v8, dev, blocks, made, rec, recording>> /\ Call("rs get 7", <<"rs 7 = " \o Hex(dev)>>)

Begin == /\ UNCHANGED <<v8, dev, blocks, made>>
         /\ IF recording THEN UNCHANGED <<rec, recording>> /\ Call("sb begin", <<"sb begin" \o HR>>)
            ELSE recording' = TRUE /\ rec' = None /\ Call("sb begin", <<>>)
End(i) == /\ i \notin made /\ UNCHANGED <<v8, dev, rec>>
          /\ IF recording THEN blocks' = [blocks EXCEPT ![i] = rec] /\ made' = made \cup {i} /\ recording' = FALSE /\ Call("sb end " \o S(i), <<>>)
             ELSE UNCHANGED <<blocks, made, recording>> /\ Call("sb end " \o S(i), <<"sb end" \o HR>>)
\* type 1 ALL and 2 PIXELSTATE hold ZENABLE, 3 VERTEXSTATE and 0 nothing; past 3 refused
Create(i, t) == /\ i \notin made /\ UNCHANGED <<v8, dev, rec, recording>>
                /\ IF recording \/ t > 3 THEN UNCHANGED <<blocks, made>> /\ Call("sb create " \o S(i) \o " " \o S(t), <<"sb create" \o HR>>)
                   ELSE blocks' = [blocks EXCEPT ![i] = IF t \in {1, 2} THEN dev ELSE None] /\ made' = made \cup {i}
                        /\ Call("sb create " \o S(i) \o " " \o S(t), <<>>)
Capture(i) == /\ UNCHANGED <<v8, dev, rec, recording, made>>
              /\ IF recording THEN UNCHANGED blocks /\ Call("sb capture " \o S(i), <<"sb capture" \o HR>>)
                 ELSE IF i \notin made THEN UNCHANGED blocks /\ Call("sb capture " \o S(i), <<"sb " \o S(i) \o " none">>)
                 ELSE blocks' = [blocks EXCEPT ![i] = IF @ = None THEN None ELSE dev] /\ Call("sb capture " \o S(i), <<>>)
Apply(i) == /\ UNCHANGED <<v8, blocks, rec, recording, made>>
            /\ IF recording THEN UNCHANGED dev /\ Call("sb apply " \o S(i), <<"sb apply" \o HR>>)
               ELSE IF i \notin made THEN UNCHANGED dev /\ Call("sb apply " \o S(i), <<"sb " \o S(i) \o " none">>)
               ELSE dev' = (IF blocks[i] = None THEN dev ELSE blocks[i]) /\ Call("sb apply " \o S(i), <<>>)
\* released while recording: forgotten; Direct3D 8 also answers INVALIDCALL
Release(i) == /\ UNCHANGED <<v8, dev, rec, recording>>
              /\ IF i \notin made THEN ~recording /\ UNCHANGED <<blocks, made>> /\ Call("sb release " \o S(i), <<"sb " \o S(i) \o " none">>)
                 ELSE blocks' = [blocks EXCEPT ![i] = None] /\ made' = made \ {i}
                      /\ Call("sb release " \o S(i), IF recording /\ v8 THEN <<"sb release" \o HR>> ELSE <<>>)

Next == \/ \E v \in Values : Set(v)
        \/ Get
        \/ Begin
        \/ \E i \in Slots : End(i) \/ Capture(i) \/ Apply(i) \/ Release(i) \/ \E t \in 0..4 : Create(i, t)

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ dev \in Values \cup {1} /\ made \subseteq Slots /\ recording \in BOOLEAN
\* a refused call never ends the recording
=============================================================================
