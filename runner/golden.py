#!/usr/bin/env python3
"""Windows' answers in golden/<suite>-<arch>/ (stdout.txt, exit, src).

    runner/golden.py stale <exe dir>    executors whose answer is missing or older than its sources
    runner/golden.py import <run dir>   <run dir>/<name>/{stdout.txt,exit} -> golden/<name>/
    runner/golden.py src <name>         the sha256 of an executor's sources

src covers the case file, build script and all linked runner modules and headers.
Every registered reset participates even in suites with no operations from its module.
"""
import hashlib
import os
import re
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GOLDEN = os.path.join(ROOT, "golden")


def sources(name):
    suite = re.sub(r"^kikizake_|-(32|64)$", "", name)
    r = os.path.join(ROOT, "runner")
    cases = os.path.join(ROOT, "cases", suite + ".txt")
    # build.sh links every module and main.c invokes every registered reset.
    files = sorted(f for f in os.listdir(r) if f.endswith((".c", ".h")) or f == "build.sh")
    return [cases] + [os.path.join(r, f) for f in files]


def src_hash(name):
    h = hashlib.sha256()
    for f in sources(name):
        h.update(os.path.basename(f).encode() + b"\0" + open(f, "rb").read())
    return h.hexdigest()


def read(p):
    return open(p).read().strip() if os.path.exists(p) else None


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["stale"] and len(a) == 2:
        for f in sorted(os.listdir(a[1])):
            if f.endswith(".exe"):
                n = f[:-4].removeprefix("kikizake_")
                if read(os.path.join(GOLDEN, n, "src")) != src_hash(n):
                    print(f[:-4])
    elif a[:1] == ["import"] and len(a) == 2:
        for d in sorted(os.listdir(a[1])):
            run = os.path.join(a[1], d)
            if not os.path.isfile(os.path.join(run, "stdout.txt")):
                continue
            n = d.removeprefix("kikizake_")
            out = os.path.join(GOLDEN, n)
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(out)
            shutil.copy(os.path.join(run, "stdout.txt"), out)
            open(os.path.join(out, "exit"), "w").write((read(os.path.join(run, "exit")) or "?") + "\n")
            open(os.path.join(out, "src"), "w").write(src_hash(n) + "\n")
            print(n)
    elif a[:1] == ["src"] and len(a) == 2:
        print(src_hash(a[1]))
    else:
        sys.exit(__doc__)
