------------------------------- MODULE D3D9Fvf -------------------------------
(* Direct3D 9's SetFVF, SetVertexDeclaration and what GetFVF and GetVertexDeclaration answer
   after them (D3D9.FVF.SET, D3D9.FVF.GET, D3D9.DECL.GET; golden d3d9draw fvf-*, decl-* and
   fvf-decl-order-* cases): SetFVF of an FVF other than 0 sets it and its runtime declaration;
   SetFVF(0) changes nothing with no declaration or one SetVertexDeclaration set, and after another
   FVF sets 0 and a declaration of no elements; SetVertexDeclaration sets the declaration as given
   and the FVF it converts to; a declaration CreateVertexDeclaration refuses (E_FAIL) leaves both.
   `log` as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES fvf, decl, user, log, n
vars == <<fvf, decl, user, log, n>>

\* an FVF: its argument, what GetFVF prints, its runtime declaration as GetVertexDeclaration prints it
Fvfs == {[arg |-> "0x1", hex |-> "00000001", decl |-> "0"],
         [arg |-> "0x42", hex |-> "00000042", decl |-> "2 00000000 00000002 000c0000 000a0004"],
         [arg |-> "0x144", hex |-> "00000144", decl |-> "3 00000000 00090003 00100000 000a0004 00140000 00050001"]}
\* a declaration CreateVertexDeclaration takes (POSITIONT, D3DCOLOR) and the FVF it converts to
Good == [args |-> "0x0 0x90003 0x100000 0xa0004", hex |-> "00000044", decl |-> "2 00000000 00090003 00100000 000a0004"]
\* one it refuses: POSITION of index 0 twice
Bad == "0x0 0x2 0x0 0x2"
Null == "null"
Call(op, out) == log' = log \o <<"> " \o op>> \o out /\ n' = n + 1

Init == /\ fvf = "00000000" /\ decl = Null /\ user = FALSE /\ n = 0
        /\ log = <<"> device swvp", "device ok">>

SetFvf(f) == /\ fvf' = f.hex /\ decl' = f.decl /\ user' = FALSE /\ Call("d9fvf " \o f.arg, <<>>)
SetFvf0 == /\ IF decl = Null \/ user THEN UNCHANGED <<fvf, decl, user>>
              ELSE fvf' = "00000000" /\ decl' = "0" /\ UNCHANGED user
           /\ Call("d9fvf 0x0", <<>>)
SetGood == /\ fvf' = Good.hex /\ decl' = Good.decl /\ user' = TRUE /\ Call("d9decl " \o Good.args, <<>>)
SetBad == /\ UNCHANGED <<fvf, decl, user>> /\ Call("d9decl " \o Bad, <<"decl hr 80004005">>)
GetFvf == /\ UNCHANGED <<fvf, decl, user>> /\ Call("d9getfvf", <<"getfvf " \o fvf>>)
GetDecl == /\ UNCHANGED <<fvf, decl, user>> /\ Call("d9getdecl", <<"getdecl " \o decl>>)

Next == \/ \E f \in Fvfs : SetFvf(f)
        \/ SetFvf0 \/ SetGood \/ SetBad \/ GetFvf \/ GetDecl

Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == /\ user \in BOOLEAN /\ n \in 0 .. MaxOps + 1
=============================================================================
