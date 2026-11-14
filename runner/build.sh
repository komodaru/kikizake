#!/bin/bash
# Builds the executor (runner/*.c: main.c and the API modules apis.h lists) once per suite (cases/<suite>.txt compiled in) for x86 and x86-64 with
# mingw-w64: <out>/kikizake_<suite>-32.exe and -64.exe (-32 only for a suite that says
# "# x86 only"). Suites that need a GPU (their first line
# says "# gpu") are listed in <out>/browser.txt, appended to if it exists (a host that runs GPU
# programs elsewhere, e.g. sake in a browser page, reads it).
#
#   runner/build.sh <out dir>
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
OUT=${1:?out dir}
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
# (the API modules are compiled once per architecture, in parallel; each suite then compiles
# only main.c with its cases and links)
cc() { if [ "$1" = 64 ]; then echo x86_64-w64-mingw32-gcc; else echo i686-w64-mingw32-gcc; fi; }
obj() { # <arch> <module.c>
  $(cc "$1") -O2 -Wall -Wno-format -Wno-unused -I"$HERE" -c "$2" -o "$TMP/o$1/$(basename "$2" .c).o"
}
exe() { # <arch> <suite>
  $(cc "$1") -O2 -Wall -Wno-format -Wno-unused -I"$TMP" -I"$HERE" -DKIKIZAKE_CASES="\"cases_$2.h\"" -DKIKIZAKE_SUITE="\"$2\"" -o "$OUT/kikizake_$2-$1.exe" "$HERE/main.c" "$TMP/o$1"/*.o -lgdi32 -luser32 -lole32 -lwindowscodecs -luuid
}
export HERE TMP OUT
export -f cc obj exe
mkdir -p "$TMP/o32" "$TMP/o64"
for arch in 32 64; do
  for c in "$HERE"/*.c; do [ "$(basename "$c")" = main.c ] || echo "$arch $c"; done
done | xargs -P "$(nproc)" -L 1 bash -c 'obj "$0" "$1"'
for cases in "$HERE"/../cases/*.txt; do
  suite=$(basename "$cases" .txt)
  { echo "static const char kikizake_cases[] = {"; od -An -v -tx1 "$cases" | sed 's/ \([0-9a-f][0-9a-f]\)/0x\1,/g'; echo "0};"; } > "$TMP/cases_$suite.h"
  echo "32 $suite"
  grep -q '^# x86 only' "$cases" || echo "64 $suite"
  if head -1 "$cases" | grep -q '^# gpu'; then
    echo "kikizake_$suite-32" >> "$OUT/browser.txt"
    if ! grep -q '^# x86 only' "$cases"; then echo "kikizake_$suite-64" >> "$OUT/browser.txt"; fi
  fi
done | xargs -P "$(nproc)" -L 1 bash -c 'exe "$0" "$1"'
