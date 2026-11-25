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

## GDI+ mutation checks (#19)

The issue lists six checks. Apply each substitution independently to the Rust model, keep the
Lean mirror unchanged, and run `cargo run --release -- lean spec/lean/.lake/build/bin/mirror FILTER`
from the repository root. The original model matches each mirror. Each substitution below produces
a model mismatch and exit status 1.

| Rust function | Temporary substitution | Filter |
|---|---|---|
| `gpimage::chain` | Compute `span.margin(r, ratio).floor()` in place of `.ceil()` | `gp.chain` |
| `gpstretch::stretch` | Omit `+ xscale + 1` from the numerator of `hi` in the negative-xscale cubic MMX branch | `gp.render` |
| `gpstretch::weights` | Compute `(v - prev) >> shift` as the weight and store `prev = v` | `gp.render` |
| `gplinear::sin_quirk` | Return `1.0` in the special round-down branch | `gp.linear` |
| `gplinear::round_sat` | Replace `r.add(v, 0.5)` with `r.add(v, 0.0)` | `gp.linear_span` |
| `gpimage::most` | Replace `a > b` with `a >= b` | `gp.chain` |

The negative-xscale mutation survived the original 104,000 render comparisons. The
`gp.render` fixture for a mirrored cubic draw with a single grey texel near the source's
right edge detects the wrong MMX column range.

## Closed dashed inset boundaries (#19)

The `gdipluspath` cases `pathinset-dash-*` capture flat dash caps on Windows x86 and
x64. They include the unit-width 6×6 square, which gives one concave contour with
eight points, and a zero-gap case in which a one-element pattern ends a span
exactly at a vertex.

Two remaining cases reproduce differences in the model.

```text
case inset-round-gap
gpmatrix 0 0x3f800000 0 0 0x3f800000 0x40000000 0xbf800000
gppen 0 0xff000000 1 align 1 dash 1 cap 0 0 2
gppath 0 0
gpadd 0 rect 1 2 12 10
gpwiden 0 0 0 0.25
gppathinfo 0
end
case inset-contour-direction
gpmatrix 0 0x3f000000 0 0 0x3f000000 0x40000000 0xbf800000
gppen 0 0xff000000 3 align 1 dash 1 dashoff 1.25
gppath 0 0
gpadd 0 rect 1 2 12 10
gpwiden 0 0 0 0.25
gppathinfo 0
end
```

In `inset-round-gap` Windows x64 returns 20 points and the model returns 17.
In `inset-contour-direction` the paths have the same 12 vertices. Windows x64 starts
the second contour at `(5,3)` and walks down, and the model starts it at `(3,3)`
and walks down.
