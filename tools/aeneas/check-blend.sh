#!/usr/bin/env bash
# Tested with Charon b23c297f, Aeneas fb44cb40, and Lean 4.36.0.
# Build the supplied Aeneas Lean backend and its dependencies before running this check.
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <charon> <aeneas> <aeneas Lean backend directory>" >&2
    exit 2
fi

charon=$(realpath -- "$1")
aeneas=$(realpath -- "$2")
backend=$(realpath -- "$3")
repo=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
scratch=$(mktemp -d)
trap 'rm -f -- "$scratch/gdi_mix.rs" "$scratch/gdi_mix.llbc" "$scratch/GdiMix.lean" "$scratch"/GdiMix.{olean,olean.private,olean.server,ir,ir.sig}; rmdir -- "$scratch"' EXIT

(cd "$repo/spec/lean" && lake build Kikizake.Gdi.Blend)

# Keep the production arithmetic and omit its coverage trace.
tr -d '\r' < "$repo/src/model/gdiblend.rs" |
    sed -n '/^pub fn mix(/,/^}/ { /^    hit(clause::GDI_ALPHA_ROUND);$/d; p; }' > "$scratch/gdi_mix.rs"
test -s "$scratch/gdi_mix.rs"

(
    cd "$scratch"
    RUSTUP_TOOLCHAIN="$("$charon" toolchain-version)" \
        "$charon" rustc --preset=aeneas --dest-file=gdi_mix.llbc -- \
        --crate-type=lib --crate-name=gdi_mix gdi_mix.rs
    "$aeneas" -backend lean -namespace BlendTrial -dest . gdi_mix.llbc
)

(
    cd "$backend"
    lake env bash -e -c '
        cd "$1"
        export LEAN_PATH="$PWD:$2/spec/lean/.lake/build/lib/lean${LEAN_PATH:+:$LEAN_PATH}"
        lean -o GdiMix.olean GdiMix.lean
        lean "$2/tools/aeneas/BlendEquivalence.lean"
    ' bash "$scratch" "$repo"
)
