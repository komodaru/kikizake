#!/usr/bin/env python3
"""Writes spec/tla/D3D9States.tla and D3D8States.tla: the device states' answers Windows gave
(golden d3d9-64, d3d8-32, the defaults cases) as the specs' tables. Run from the repository root:
python3 -B spec/tla/states.py"""
import re

SUITES = [
    ("D3D9States", "d3d9", "golden/d3d9-64/stdout.txt", "D3D9.RS.DEFAULT, D3D9.RS.POINTSIZEMAX, D3D9.RS.UNDOC, D3D9.TSS.DEFAULT, D3D9.TSS.UNDOC, D3D9.SS.DEFAULT, D3D9.SET.UNDOC"),
    ("D3D8States", "d3d8", "golden/d3d8-32/stdout.txt", "D3D8.RS.DEFAULT, D3D8.RS.OWN, D3D8.RS.UNDOC, D3D8.TSS.DEFAULT, D3D8.TSS.UNDOC, D3D8.SET.UNDOC"),
]


def cases(path):
    out, name = {}, None
    for line in open(path):
        line = line.rstrip("\n")
        if line.startswith("case "):
            name = line[5:]
            out[name] = []
        elif name is not None:
            out[name].append(line)
    return out


def device_lines(path):
    """each defaults case's device line (the call) by case name"""
    out, name = {}, None
    for line in open(path):
        if line.startswith("case "):
            name = line[5:].strip()
        elif name and line.startswith("device"):
            out.setdefault(name, line.strip())
    return out


def main():
    for module, suite, golden, clauses in SUITES:
        got = cases(golden)
        devs = device_lines(f"cases/{suite}.txt")
        modes = {}
        for name, call in devs.items():
            if not name.startswith("defaults"):
                continue
            lines = got[name]
            table = {}
            for l in lines[1:]:
                m = re.match(r"^((?:rs|tss|ss) [0-9 ]+?)( = [0-9a-f]{8}| hr [0-9a-f]{8})$", l)
                if not m:
                    raise SystemExit(f"{golden} {name}: unexpected line {l!r}")
                table[m.group(1)] = m.group(2)
            modes[call] = (lines[0], table)
        keys = sorted(next(iter(modes.values()))[1], key=lambda k: [k.split()[0]] + [int(x) for x in k.split()[1:]])
        def tla_table(table):
            return "[k \\in Keys |-> CASE " + "\n      [] ".join(f'k = "{k}" -> "{table[k]}"' for k in keys) + "]"
        mode_names = list(modes)
        text = f"""---------------------------- MODULE {module} ----------------------------
(* Direct3D {suite[-1]}'s device states as Windows answers them ({clauses}): every render, texture
   stage and sampler state read on a new device of each kind (the documentation's defaults, and
   0xbaadcafe or D3DERR_INVALIDCALL for the undocumented ones), and each one set to 0x1234 and read
   back: a state that reads a value reads what was set, one that fails still fails. The tables
   are Windows' answers, golden {golden.split('/')[1]} cases {', '.join(n for n in devs if n.startswith('defaults'))};
   written by spec/tla/states.py. `log` as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES mode, log, n
vars == <<mode, log, n>>

Modes == {{{", ".join(f'"{m}"' for m in mode_names)}}}
Keys == {{{", ".join(f'"{k}"' for k in keys)}}}
DeviceAnswer == [m \\in Modes |-> CASE {" [] ".join(f'm = "{m}" -> "{modes[m][0]}"' for m in mode_names)}]
"""
        for i, m in enumerate(mode_names):
            text += f"Default{i} == {tla_table(modes[m][1])}\n"
        text += "Default(m) == CASE " + " [] ".join(f'm = "{m}" -> Default{i}' for i, m in enumerate(mode_names)) + "\n"
        text += r"""
\* the call that sets a state: "rs set 7 4660" of "rs 7"
SetCall(k) == LET w == k IN
  IF SubSeq(w, 1, 3) = "rs " THEN "rs set " \o SubSeq(w, 4, Len(w)) \o " 4660"
  ELSE IF SubSeq(w, 1, 4) = "tss " THEN "tss set " \o SubSeq(w, 5, Len(w)) \o " 4660"
  ELSE "ss set " \o SubSeq(w, 4, Len(w)) \o " 4660"
GetCall(k) ==
  IF SubSeq(k, 1, 3) = "rs " THEN "rs get " \o SubSeq(k, 4, Len(k))
  ELSE IF SubSeq(k, 1, 4) = "tss " THEN "tss get " \o SubSeq(k, 5, Len(k))
  ELSE "ss get " \o SubSeq(k, 4, Len(k))
Fails(a) == SubSeq(a, 1, 3) = " hr"

Init == /\ mode \in Modes /\ log = <<"> " \o mode, DeviceAnswer[mode]>> /\ n = 0

\* the documentation's default (or the undocumented state's answer) of a new device
Read(k) == /\ log' = log \o <<"> " \o GetCall(k), k \o Default(mode)[k]>> /\ n' = n + 1 /\ UNCHANGED mode
\* D3D9.SET.UNDOC, D3D8.SET.UNDOC: set, then read
SetRead(k) == /\ log' = log \o <<"> " \o SetCall(k), "> " \o GetCall(k),
                                k \o (IF Fails(Default(mode)[k]) THEN Default(mode)[k] ELSE " = 00001234")>>
              /\ n' = n + 1 /\ UNCHANGED mode

Next == \E k \in Keys : Read(k) \/ SetRead(k)
Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == mode \in Modes /\ n \in 0..(MaxOps + 1)
=============================================================================
"""
        open(f"spec/tla/{module}.tla", "w").write(text)
        open(f"spec/tla/{module}.cfg", "w").write("CONSTANTS MaxOps = 1\nINIT Init\nNEXT Next\nCONSTRAINT Bound\nINVARIANTS TypeOK\n")


GROUPS = [
    ("D3D9Groups", "d3d9", "golden/d3d9-64/stdout.txt", "D3D9.SB.TYPES"),
    ("D3D8Groups", "d3d8", "golden/d3d8-32/stdout.txt", "D3D8.SB.GROUPS"),
]


def case_calls(path):
    out, name = {}, None
    for line in open(path):
        line = line.strip()
        if line.startswith("case "):
            name = line[5:]
            out[name] = []
        elif name is not None and line != "end":
            out[name].append(line)
    return out


def key_of(call):
    w = call.split()
    return " ".join([w[0]] + w[2:-1]) if w[1] == "set" else " ".join([w[0]] + w[2:])


def groups():
    for module, suite, golden, clauses in GROUPS:
        got = cases(golden)
        calls = case_calls(f"cases/{suite}.txt")
        device = calls["create-1"][0]
        member = {}
        keys = None
        for t in (1, 2, 3):
            name = f"create-{t}"
            sets = {key_of(c): int(c.split()[-1], 0) for c in calls[name] if " set " in c}
            answers = {}
            for l in got[name][1:]:
                m = re.match(r"^((?:rs|tss|ss) [0-9 ]+?)( = ([0-9a-f]{8})| hr [0-9a-f]{8})$", l)
                if not m:
                    raise SystemExit(f"{golden} {name}: unexpected line {l!r}")
                answers[m.group(1)] = int(m.group(3), 16) if m.group(3) else None
            ks = [k for k in sets if answers.get(k) is not None]
            keys = ks if keys is None else [k for k in keys if k in ks]
            member[t] = {k for k in ks if answers[k] != sets[k]}
        keys = sorted(keys, key=lambda k: [k.split()[0]] + [int(x) for x in k.split()[1:]])
        def members(t):
            return "{" + ", ".join(f'"{k}"' for k in keys if k in member[t]) + "}"
        text = f"""---------------------------- MODULE {module} ----------------------------
(* Direct3D {suite[-1]}'s state block types as Windows answers them ({clauses}): a block made by
   CreateStateBlock of a type (1 ALL, 2 PIXELSTATE, 3 VERTEXSTATE) holds the type's states at the
   device's values, so a state set after it and then the block applied reads the device's value
   back where the state is the type's, and what was set where not. The lists are Windows' answers
   (golden {golden.split('/')[1]} cases create-1, create-2, create-3), written by spec/tla/states.py;
   the device's values are the defaults (D3D{suite[-1]}States). `log` as in GdiPath.tla. *)
EXTENDS Integers, Sequences, TLC

CONSTANTS MaxOps

VARIABLES type, log, n
vars == <<type, log, n>>

Keys == {{{", ".join(f'"{k}"' for k in keys)}}}
Group(t) == CASE t = 1 -> {members(1)}
              [] t = 2 -> {members(2)}
              [] t = 3 -> {members(3)}
"""
        text += r"""
S == INSTANCE """ + module.replace("Groups", "States") + r""" WITH mode <- "DEVICE", log <- log, n <- n

Init == /\ type \in 1..3 /\ n = 0
        /\ log = <<"> DEVICE", "DEVICEOK", "> sb create 0 " \o ToString(type)>>

\* set after the block was made, then the block applied: the type's states take the device's
\* values back
SetApplyRead(k) == /\ log' = log \o <<"> " \o S!SetCall(k), "> sb apply 0", "> " \o S!GetCall(k),
                                     k \o (IF k \in Group(type) THEN S!Default("DEVICE")[k] ELSE " = 00001234")>>
                   /\ n' = n + 1 /\ UNCHANGED type

Next == \E k \in Keys : SetApplyRead(k)
Spec == Init /\ [][Next]_vars
Bound == n <= MaxOps
TypeOK == type \in 1..3 /\ n \in 0..(MaxOps + 1)
=============================================================================
"""
        text = text.replace('"DEVICEOK"', '"' + got["create-1"][0] + '"').replace('"> DEVICE"', '"> ' + device + '"').replace('"DEVICE"', '"' + device + '"')
        open(f"spec/tla/{module}.tla", "w").write(text)
        open(f"spec/tla/{module}.cfg", "w").write("CONSTANTS MaxOps = 1\nINIT Init\nNEXT Next\nCONSTRAINT Bound\nINVARIANTS TypeOK\n")


main()
groups()
