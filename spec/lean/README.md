# Lean 4 proofs

What the clauses (spec/) say about arithmetic (rounding, blending, fixed point, stepping,
coverage) is proved here in Lean 4 for every input of the stated domain, with Lean's kernel as
the only trust (kikizake-lint `lean-trust`: no `sorry`, `axiom`, `native_decide`, `bv_decide`, ...;
`warningAsError`). Each file defines its API's functions as the model (src/model/) computes them
and proves the clauses of them; spec/laws.txt lists each theorem (kind `lean`) with its clauses.

A proof about a Lean definition says nothing of the Rust model unless the two are the same
function, so every file has mirrors: src/lean/ names, for each definition, the model function it
is, and `kikizake lean` runs both on the same arguments (the `mirror` executable answers for the
Lean side) and fails on the first difference. `Every` mirrors enumerate the whole domain (the
equality is then a proof by enumeration); `Sampled` ones take every combination of the
arguments' ends and a fixed pseudo-random sample, and say so.

The ledger keeps "proved of the specification" and "holds of the implementation" apart
(src/lean/ledger.rs): a theorem reaches the model as far as the weakest mirror of the
definitions its statement names (`every`, `sampled`), and one naming no mirrored definition is a
fact of the specification alone (`spec-only`). `kikizake trace` shows each clause's best reach
(its `impl` column, and per API how many clauses reach the models over the domain, on samples
or not at all); spec/sampled.txt lists the settled clauses whose proofs reach the models only on
samples or not at all, and tests/spec.rs holds it to what the laws give
(`cargo run --release -- ledger --update`). A clause leaves it when a law reaches the model
over its whole domain: an `Every` mirror, an exhaustive law, a Kani harness or a TLA+ replay.

    cd spec/lean && lake build
    cargo run --release -- lean spec/lean/.lake/build/bin/mirror

Which tool for which clause:

| what the clause says | tool |
|---|---|
| a formula over integers (rounding, blends, fixed point, DDA steps, dithering) | Lean (here) |
| a sequence of calls and their answers (state, objects, lifetimes, fences, queues) | TLA+ (spec/tla), every state replayed against the models |
| a property of the Rust code itself over machine integers and floats | Kani (src/proofs.rs) |
| a small finite domain | an exhaustive law (tests/) |

spec/unproven.txt lists the settled clauses of the finished APIs that none of these proves yet;
it only shrinks.
