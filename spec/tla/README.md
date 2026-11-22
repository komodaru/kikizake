# TLA+ specs

Each spec writes what the clauses it names (spec/laws.txt, kind `tla`) say about a sequence of
calls, as a state machine whose variable `log` is that sequence as a kikizake case: lines
starting `> ` are calls, the rest the answers the documentation gives. TLC explores every
sequence up to `MaxOps` calls (the .cfg), checks the invariants, and dumps every state;
`kikizake tla <dump>` runs each state's log through the models and fails if one answers
differently. So a model that breaks a clause on any short sequence fails, without Windows.

    java -cp tla2tools.jar tlc2.TLC -workers auto -dump /tmp/GdiPath.dump -config GdiPath.cfg GdiPath.tla
    cargo run --release -- tla /tmp/GdiPath.dump

| spec | clauses | states (MaxOps) |
|---|---|---|
| GdiPath | BeginPath, EndPath, AbortPath, CloseFigure, GetPath, MoveToEx, LineTo (GDI.PATH.*, GDI.POS.MOVE) | 402,234 (5) |
| GdiRegion | CreateRectRgn, SetRectRgn, CombineRgn, OffsetRgn, GetRgnBox, PtInRegion, RectInRegion, EqualRgn (GDI.RGN.*): a region as its set of pixels, every query asked after every call | 8,379 (2) |
| D3D9StateBlock | SetRenderState, Create/Begin/EndStateBlock, Capture, Apply (D3D9.SETGET, D3D9.SB.*, D3D9.RS.DEFAULT) | 180,869 (4) |

The specs model documented use only: what the documentation leaves open (D3D9.SB.MISUSE) stays
open. Comments cannot hold `*)`.
