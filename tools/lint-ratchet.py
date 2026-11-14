#!/usr/bin/env python3
"""(A copy of masu's tools/ci/lint-ratchet.py (a-kaibu/masu) for this repository: the same lints
and rules; kikizake stays a repository of its own, so it keeps its own copy.)

The type-safety lint ratchet: clippy's lints for what the compiler could have caught (an `as`
that truncates or changes sign, a `_` arm that swallows a new enum variant, an unwrap), counted per
file against tools/lint-ratchet.tsv. A file may only go down: a push that adds one fails CI
(write it with From/TryFrom, an exhaustive match, `?`/a typed error instead).

  tools/lint-ratchet.py            check (CI)
  tools/lint-ratchet.py --update   write the counts down after fixing some (never up: a file
                                      above its line still fails)
  tools/lint-ratchet.py --update --raise FILE...
                                      those files' lines to what they have now: only for code
                                      moved in from another file (say so in the commit)

Per file, not per crate: threads that each lower a crate's total on their own merge their tsv
lines without a conflict and without one's total hiding the other's additions (two sake totals
merged by rebase let 2 new unwraps through). `allow`/`expect` attributes naming a tracked lint
count as one of their own ("allow-attr"), so silencing is not a way past it. Code under
#[cfg(test)] may unwrap (clippy.toml), and test files (tests/, *_tests.rs, tests.rs) are not
counted: the tests that pin behaviour before a rewrite may cast freely.

ROBUST: the lints for code that can panic, overflow, misread memory or drop an error (indexing,
arithmetic that can overflow, undocumented unsafe, `let _ =` on a Result, ...). They are counted
in a second clippy run over the libraries and binaries only (no --all-targets): tests may index,
add and panic as they like. GROUPS: clippy's correctness, suspicious and perf groups, every lint
in them (mostly at 0; they stay there). rustc's unsafe_op_in_unsafe_fn (E0133 as a warning) is
counted as "unsafe_op_in_unsafe_fn".

CUSTOM ("masu-*"): what clippy has no lint for, found line by line in crates/ (comments and test
files skipped): `static mut`, `unsafe impl Send/Sync`, transmute, Vec::set_len, mem::zeroed /
assume_init on uninit, slice::from_raw_parts, and the store's tree format ("vnvm-tree") spelled
outside crates/kura/src/tree_list.rs (each new reader of a wire format is a second parser to keep in step).
"""
import collections, json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TSV = os.path.join(ROOT, "tools", "lint-ratchet.tsv")
LINTS = [
    "as_conversions",  # every `as`: From/TryFrom/a named helper says what may happen
    "cast_possible_truncation",  # the subset of those that drop bits
    "cast_sign_loss",
    "cast_possible_wrap",
    "wildcard_enum_match_arm",  # `_ =>` on an enum: a new variant compiles silently
    "match_wildcard_for_single_variants",
    "unwrap_used",
    "expect_used",
    "fn_to_numeric_cast_any",
    "ptr_as_ptr",
]
TEST_FILE = re.compile(r"(^|/)(tests|benches)/|(^|/|_)tests?\.rs$")
ROBUST = [
    "indexing_slicing",  # a[i] / a[x..y] that panics: get()/split_at_checked/an iterator
    "arithmetic_side_effects",  # + - * / % << that can overflow or divide by zero: checked_/wrapping_/saturating_
    "string_slice",  # &s[a..b] on a str panics off a char boundary
    "get_unwrap",
    "panic",
    "todo",
    "unimplemented",
    "unreachable",
    "panic_in_result_fn",
    "unwrap_in_result",
    "exit",
    "dbg_macro",
    "undocumented_unsafe_blocks",  # every unsafe block says why it is sound (// SAFETY:)
    "multiple_unsafe_ops_per_block",
    "missing_safety_doc",
    "cast_precision_loss",
    "cast_ptr_alignment",
    "ptr_cast_constness",
    "borrow_as_ptr",
    "as_underscore",
    "char_lit_as_u8",
    "transmute_ptr_to_ptr",
    "transmute_undefined_repr",
    "cast_slice_different_sizes",
    "cast_enum_truncation",
    "fn_to_numeric_cast_with_truncation",
    "invalid_upcast_comparisons",
    "checked_conversions",
    "let_underscore_must_use",  # `let _ = r;` drops an error unread
    "unused_result_ok",
    "map_err_ignore",
    "float_cmp",
    "lossy_float_literal",
    "iter_over_hash_type",  # HashMap order differs run to run: replays and goldens must not see it
    "unchecked_time_subtraction",
    "mem_forget",
    "large_stack_arrays",
    "maybe_infinite_iter",
    "mixed_read_write_in_expression",
    "debug_assert_with_mut_call",
    "fallible_impl_from",
    "empty_drop",
    "same_name_method",
    "rest_pat_in_fully_bound_structs",
    "wildcard_in_or_patterns",
    "suspicious_xor_used_as_pow",
    "unnecessary_safety_comment",
    "try_err",
    "mut_mut",
]
GROUPS = ["correctness", "suspicious", "perf"]
RUSTC = {"E0133": "unsafe_op_in_unsafe_fn"}
TRACKED = re.compile(r"\b(?:clippy::(?:" + "|".join(LINTS + ROBUST + GROUPS) + ")|" + "|".join(RUSTC.values()) + r")\b")
CUSTOM = {
    "masu-static_mut": re.compile(r"\bstatic\s+mut\b"),
    "masu-unsafe_send_sync": re.compile(r"\bunsafe\s+impl\b[^{]*\b(?:Send|Sync)\s+for\b"),
    "masu-transmute": re.compile(r"\btransmute(?:_copy)?\s*(?:::<|\()"),
    "masu-set_len": re.compile(r"\.set_len\s*\("),
    "masu-uninit": re.compile(r"\bmem::(?:zeroed|uninitialized)\b|\buninit\(\)\s*\.assume_init\b"),
    "masu-from_raw_parts": re.compile(r"\bfrom_raw_parts(?:_mut)?\s*\("),
    "masu-wire_tree": re.compile(r"vnvm-tree"),
}
CUSTOM_HOME = {"masu-wire_tree": "crates/kura/src/tree_list.rs"}
ALLOW = re.compile(r"#!?\[\s*(?:allow|expect)\s*\(([^\]]*)\)\s*\]")


def clippy():
    yield from run_clippy(["--all-targets"], LINTS + ["clippy::" + g for g in GROUPS])
    yield from run_clippy(["--lib", "--bins"], ROBUST + ["unsafe_op_in_unsafe_fn"])


seen = set()


def run_clippy(targets, lints):
    args = ["cargo", "clippy", *targets, "--keep-going", "--message-format=json", "--", "--cap-lints", "warn", "-A", "clippy::all"]
    for l in lints:
        args += ["-W", l if "::" in l or l in RUSTC.values() else "clippy::" + l]
    p = subprocess.run(args, cwd=ROOT, stdout=subprocess.PIPE, text=True)
    for line in p.stdout.splitlines():
        try:
            m = json.loads(line)
        except ValueError:
            continue
        if m.get("reason") != "compiler-message":
            continue
        msg = m["message"]
        if msg["level"] == "error":
            sys.stderr.write(msg.get("rendered") or msg["message"])
        code = (msg.get("code") or {}).get("code") or ""
        if msg["level"] == "warning" and code in RUSTC:
            code = "clippy::" + RUSTC[code]
        if not code.startswith("clippy::"):
            continue
        sp = [s for s in msg["spans"] if s["is_primary"]]
        if not sp:
            continue
        # (a lib's code is checked twice with --all-targets: as the lib and as its tests)
        key = (sp[0]["file_name"], sp[0]["line_start"], sp[0]["column_start"], code[8:])
        if key not in seen:
            seen.add(key)
            yield key
    if p.returncode != 0:
        sys.exit("lint-ratchet: clippy failed (above)")


def allow_attrs():
    for top in ("src",):
        for d, _, fs in os.walk(os.path.join(ROOT, top)):
            for f in fs:
                if not f.endswith(".rs"):
                    continue
                path = os.path.join(d, f)
                rel = os.path.relpath(path, ROOT)
                for i, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
                    for m in ALLOW.finditer(line):
                        if TRACKED.search(m.group(1)):
                            yield (rel, i, m.start() + 1, "allow-attr")


def custom():
    for d, _, fs in os.walk(os.path.join(ROOT, "src")):
        for f in fs:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(d, f)
            rel = os.path.relpath(path, ROOT)
            for i, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
                code = line.split("//", 1)[0] if "//" in line and '"' not in line.split("//", 1)[0] else line
                if code.lstrip().startswith(("//", "*", "/*")):
                    continue
                for name, rx in CUSTOM.items():
                    if CUSTOM_HOME.get(name) == rel:
                        continue
                    for m in rx.finditer(code):
                        yield (rel, i, m.start() + 1, name)


def count():
    per_file = collections.Counter()
    for f, _, _, lint in list(clippy()) + list(allow_attrs()) + list(custom()):
        if not TEST_FILE.search(f):
            per_file[(f, lint)] += 1
    return per_file


def load():
    files = {}
    if os.path.exists(TSV):
        for line in open(TSV):
            if line.startswith("#") or not line.strip():
                continue
            kind, name, lint, n = line.rstrip("\n").split("\t")
            # (crate lines: what the ratchet enforced before 2026-11-14 18:40Z; ignored)
            if kind == "file":
                files[(name, lint)] = int(n)
    return files


def save(files):
    with open(TSV, "w") as o:
        o.write("# tools/lint-ratchet.py: type-safety, robustness and masu-* lints per file (enforced: never up)\n")
        for (f, l), n in sorted(files.items()):
            if n:
                o.write(f"file\t{f}\t{l}\t{n}\n")


def main():
    update = "--update" in sys.argv
    raise_files = set(sys.argv[sys.argv.index("--raise") + 1 :]) if "--raise" in sys.argv else set()
    files = count()
    old = load()
    up = sorted(k for k, n in files.items() if n > old.get(k, 0) and k[0] not in raise_files)
    down = sorted(k for k, n in old.items() if files.get(k, 0) < n)
    for f, l in up:
        print(f"lint-ratchet: {f}: clippy::{l} {old.get((f, l), 0)} -> {files[(f, l)]}")
    if update:
        if up:
            sys.exit("lint-ratchet: counts above went up: fix them (the baseline only goes down)")
        new = {k: (n if k[0] in raise_files else min(n, old.get(k, n))) for k, n in files.items()}
        save(new)
        print(f"lint-ratchet: wrote {os.path.relpath(TSV, ROOT)} ({sum(new.values())} in all, {sum(old.values()) - sum(new.values())} fewer)")
        return
    if down:
        print(f"lint-ratchet: {sum(old[k] - files.get(k, 0) for k in down)} fewer than the baseline in {len(set(f for f, _ in down))} files (tools/lint-ratchet.py --update writes it down)")
    if up:
        sys.exit("lint-ratchet: lints went up (above): From/TryFrom instead of `as`, every variant named instead of `_`, `?` or a typed error instead of unwrap/panic, get()/checked_*/wrapping_* instead of a[i] and bare arithmetic, // SAFETY: on unsafe")
    print(f"lint-ratchet: ok ({sum(files.values())} left)")


main()
