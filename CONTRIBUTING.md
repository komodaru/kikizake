# Adding an API or a suite

Each API is a handful of files of its own; the lists that name them have one line per entry,
so work on different APIs does not collide.

| what | where | listed in |
|---|---|---|
| executor module: its ops (`struct kz_op`), its reset | `runner/<api>.c` (`#include "core.h"`) | `runner/apis.h` (`API(<api>)`) |
| clauses | `spec/<api>.txt` | `src/spec.rs` `FILES` |
| model | `src/model/<api>.rs` (`impl Api`) | `src/model/mod.rs` `apis()` |
| generator | `src/generators/<api>.rs` | `src/generators/mod.rs` `GENERATORS` |
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
  the documentation leaves open is `open` (the model returns `Fail::Unmodeled(open::<ID>)`) or,
  when a guess is worth checking, `assumed`.
* The model calls `spec::hit(clause::<ID>)` where it applies a clause (`GDI.ROP3.TRUTH` is
  `clause::GDI_ROP3_TRUTH`). build.rs makes the constants from spec/: a clause that does not
  exist, or an `Unmodeled` of a clause that is not `open` or `excluded`, does not compile, and a
  clause settled out of `open` breaks the build where a model still calls it open. Never pass
  clause names as strings. `cargo test` fails if a clause is never reached.
* Once Windows has answered (`golden/`), `cargo test` compares every modelled case. Settle each
  difference from the answers: correct the model, mark the clause `measured` with the golden
  named, and say how it was found (a table of every value, a fitted formula and on how many
  samples). Never widen `unmodeled` to hide a difference you can explain.
* A fitted approximation of an open clause answers only through `model::approx(open::X)?`:
  by default it is unmodeled, so every answer `cargo test` compares is one Windows gives.
  `KIKIZAKE_FITTED=1 cargo run -- model <suite>` makes the approximations answer too, to fit
  them against the goldens with `compare`.

## Evidence (spec/laws.txt, spec/learn.txt, spec/unresolved.txt)

Every clause has to show the evidence its status claims; `cargo test` (tests/spec.rs) checks it and
`cargo run -- trace` prints each clause's evidence and the gaps per API:

* `doc`: its source names pages that `spec/learn.txt` knows (`cargo run -- learn resolve` finds
  missing ones on Microsoft Learn; check the line by eye), and a law (below) or a case Windows
  agrees with checks it. What it quotes must be the page's words (`cargo run -- learn check`,
  also the weekly `learn` workflow).
* `measured`: a case that reaches it agrees with Windows' answer (a case that stops at an open
  clause compares nothing: give each description its own case).
* `assumed`: a law or a case Windows agrees with.
* `Fail::Unmodeled(open::<ID>)` names an `open` or `excluded` clause only (the compiler checks).
* Nothing is left unresolved: every `open`, `assumed` and `excluded` clause is listed in
  `spec/unresolved.txt`, and that list only shrinks. Settle a clause (model it from the
  documentation, fit it to Windows' answers) and take its line out; a new clause comes settled.
* A law (tests/) that checks a clause from its text, independent of Windows' answers, is listed in
  `spec/laws.txt` with the clauses it checks and whether it is `exhaustive` (every input of its
  domain: a proof there), `pbt`, `kani` (a proof in `src/proofs.rs`, `cargo kani`) or `tla` (a
  TLA+ spec in `spec/tla`, see its README). Every Kani harness and TLA+ spec must be listed.
  The `formal` workflow runs both; `cargo test` cannot.
* Case names are unique in a suite (answers are matched to cases by name).

## Tests

`cargo test` (case files current, laws, clauses reached, models against Windows). Add laws
(`tests/laws.rs`: proptest, exhaustive over small domains) for what the documentation states as
properties, the fuzz target covers every parser and model through `model::run`, and
`cargo mutants` must find no surviving mutant in a new model.

## Windows' answers

Push to main: the `windows` workflow builds the executors, runs those whose sources changed
since their answer on windows-2025 and commits `golden/`. A suite's sources are its case file,
`build.sh`, `main.c`, `core.h` and the modules whose ops its lines use (`runner/golden.py`), so
work on one API does not make the other suites' answers stale. Run it by hand (workflow_dispatch,
`all`) to record everything again.
