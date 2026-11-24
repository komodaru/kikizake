"""Fail if a settled clause loses its formal law, including when its ledger entry was edited."""
from pathlib import Path
import subprocess
import sys


def previous_gaps(revision):
    def show(path):
        return subprocess.check_output(["git", "show", f"{revision}:{path}"], encoding="utf-8")

    paths = subprocess.check_output(
        ["git", "ls-tree", "--name-only", revision, "spec/"], encoding="utf-8"
    ).splitlines()
    settled = set()
    for path in paths:
        if not path.endswith(".txt"):
            continue
        for line in show(path).splitlines():
            fields = line.partition(" | ")[0].split()
            if len(fields) == 2 and fields[1] in {"doc", "measured"} and " | " in line:
                settled.add(fields[0])
    formal = set()
    for line in show("spec/laws.txt").splitlines():
        fields = line.split()
        if fields and not line.startswith("#") and fields[1] in {"exhaustive", "kani", "tla", "lean"}:
            formal.update(fields[2:])
    return settled - formal


if __name__ == "__main__":
    current = {line for line in Path("spec/unproven.txt").read_text(encoding="utf-8").splitlines()
               if line and not line.startswith("#")}
    new = current - previous_gaps(sys.argv[1])
    if new:
        sys.exit("New formal gaps relative to the base revision:\n" + "\n".join(sorted(new)))
