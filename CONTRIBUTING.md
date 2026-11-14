# Adding an API or a suite

Each API is a handful of files of its own; the lists that name them have one line per entry,
so work on different APIs does not collide.

| what | where | listed in |
|---|---|---|
| executor module: its ops (`struct kz_op`), its reset | `runner/<api>.c` (`#include "core.h"`) | `runner/apis.h` (`API(<api>)`) |
| clauses | `spec/<api>.txt` | `src/spec.rs` `FILES` |
| model | `src/model/<api>.rs` (`impl Api`) | `src/model/mod.rs` `apis()` |
| generator | `src/gen/<api>.rs` | `src/gen/mod.rs` `GENERATORS` |
| case file | `cases/<suite>.txt` (`cargo run -- gen <suite> > cases/<suite>.txt`) | |
| Windows' answers | `golden/<suite>-<arch>/` (committed by the `windows` workflow) | |

## The executor

* One C program, `runner/main.c` plus every module, built per suite with the suite's cases
  compiled in (`runner/build.sh`). Output only through `out` / `out_hex` (core.h).
* Link only gdi32, user32, ole32, windowscodecs, uuid. Every other DLL is loaded with
  `LoadLibraryA` when a case first asks for it, and a missing one prints `<op> none`: the same
  exe must start on systems that lack an API (Wine, emulators, sake in a browser, x86-64 without
  Direct3D 8).
* Call COM through the C macros (`COBJMACROS`) or, where headers clash, through vtable
  indices (`runner/d3d9.c`'s Direct3D 8).
* A line the module cannot run as written sets `bad` ("bad line N"). Reset everything the
  case made in the module's reset (run after every case).
* GPU suites start with `# gpu` (hosts that run GPU programs elsewhere read browser.txt);
  `# x86 only` builds no x86-64 exe.
* Windows' runner has no GPU: Direct3D 9-12, Direct2D and DXGI run on WARP or the reference
  rasterizer there, OpenGL on Microsoft's GDI generic 1.1.

## Clauses and models

* Write the clauses from Microsoft Learn first (`doc`), with the page name as the source. What
  the documentation leaves open is `open` (the model returns `Fail::Unmodeled(<id>)`) or, when
  a guess is worth checking, `assumed`.
* The model calls `spec::hit(<id>)` where it applies a clause; `cargo test` fails if a clause is
  never reached or a hit names no clause.
* Once Windows has answered (`golden/`), `cargo test` compares every modelled case. Settle each
  difference from the answers: correct the model, mark the clause `measured` with the golden
  named, and say how it was found (a table of every value, a fitted formula and on how many
  samples). Never widen `unmodeled` to hide a difference you can explain.

## Tests

`cargo test` (case files current, laws, clauses reached, models against Windows). Add laws
(`tests/laws.rs`: proptest, exhaustive over small domains) for what the documentation states as
properties, the fuzz target covers every parser and model through `model::run`, and
`cargo mutants` must find no surviving mutant in a new model.

## Windows' answers

Push to main: the `windows` workflow builds the executors, runs those whose sources changed
since their answer on windows-2025 and commits `golden/`. Run it by hand (workflow_dispatch,
`all`) to record everything again.
