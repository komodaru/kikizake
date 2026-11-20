#!/usr/bin/env python3
"""Windows' answers in golden/<suite>-<arch>/ (stdout.txt, exit, src).

    runner/golden.py stale <exe dir>    executors whose answer is missing or older than its sources
    runner/golden.py import <run dir>   <run dir>/<name>/{stdout.txt,exit} -> golden/<name>/ (files skipped)
    runner/golden.py verify <golden dir> [suite ...]   validate all required references
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
        with open(f, "rb") as source:
            h.update(os.path.basename(f).encode() + b"\0" + source.read())
    return h.hexdigest()


def read(p):
    try:
        with open(p) as f:
            return f.read().strip()
    except FileNotFoundError:
        return None


def measurement_error(directory):
    stdout = os.path.join(directory, "stdout.txt")
    if not os.path.isfile(stdout) or os.stat(stdout).st_size == 0:
        return "missing or empty stdout.txt"
    status = read(os.path.join(directory, "exit"))
    if status is None or not status.isdecimal() or int(status) in (124, 125):
        return f"incomplete measurement (exit {status})"
    return None


def reference_error(directory, digest):
    if read(os.path.join(directory, "src")) != digest:
        return "missing or stale source hash"
    return measurement_error(directory)


if __name__ == "__main__":
    a = sys.argv[1:]
    if a[:1] == ["stale"] and len(a) == 2:
        for f in sorted(os.listdir(a[1])):
            if f.endswith(".exe"):
                n = f[:-4].removeprefix("kikizake_")
                if reference_error(os.path.join(GOLDEN, n), src_hash(n)):
                    print(f[:-4])
    elif a[:1] == ["import"] and len(a) == 2:
        for d in sorted(os.listdir(a[1])):
            run = os.path.join(a[1], d)
            # (ref/winbins.txt, the binaries' keys, is not an executor's answer)
            if not os.path.isdir(run):
                continue
            error = measurement_error(run)
            if error:
                sys.exit(f"{d}: {error}")
            n = d.removeprefix("kikizake_")
            out = os.path.join(GOLDEN, n)
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(out)
            shutil.copy(os.path.join(run, "stdout.txt"), out)
            open(os.path.join(out, "exit"), "w").write(read(os.path.join(run, "exit")) + "\n")
            open(os.path.join(out, "src"), "w").write(src_hash(n) + "\n")
            print(n)
    elif a[:1] == ["verify"] and len(a) >= 2:
        suites = a[2:] or sorted(f[:-4] for f in os.listdir(os.path.join(ROOT, "cases")) if f.endswith(".txt"))
        errors = []
        for suite in suites:
            with open(os.path.join(ROOT, "cases", suite + ".txt")) as f:
                case_lines = f.readlines()
            digest = src_hash(suite)
            for arch, skip in [(32, "# x64 only"), (64, "# x86 only")]:
                if any(line.startswith(skip) for line in case_lines):
                    continue
                name = f"{suite}-{arch}"
                error = reference_error(os.path.join(a[1], name), digest)
                if error:
                    errors.append(f"{name}: {error}")
        if errors:
            sys.exit("\n".join(errors))
    elif a[:1] == ["src"] and len(a) == 2:
        print(src_hash(a[1]))
    else:
        sys.exit(__doc__)
