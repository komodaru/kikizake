#!/usr/bin/env python3
"""(A copy of masu's tools/lint-ratchet.py (a-kaibu/masu) for this repository: the same lints
and rules, its code in src/ instead of crates/; kikizake stays a repository of its own, so it
keeps its own copy. masu-relay is masu's server only.)

The type-safety lint ratchet: clippy's lints for what the compiler could have caught (an `as`
that truncates or changes sign, a `_` arm that swallows a new enum variant, an unwrap), counted per
file against tools/lint-ratchet.tsv. A file may only go down: a push that adds one fails CI
(write it with From/TryFrom, an exhaustive match, `?`/a typed error instead).

  tools/lint-ratchet.py            check (CI)
  tools/lint-ratchet.py --update   write the counts down after fixing some (never up: a file
                                      above its line still fails)
  tools/lint-ratchet.py --self-test
                                      every counted lint against tools/lint-fixtures/ratchet (one
                                      marked line per regression it must catch; CI)
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

CUSTOM ("masu-*"): what clippy has no lint of its own for. The unsafe functions are clippy's
disallowed_methods (clippy.toml), resolved by type, not by name (review 2026-11-15: Vec::set_len was
"a `.set_len(` with `unsafe` in the 5 lines before"): Vec::set_len, slice::from_raw_parts(_mut),
mem::transmute(_copy), mem::zeroed / MaybeUninit::assume_init, each counted under its masu-* name
(DISALLOWED); and masu-relay, masu's Storage::get: each place the server reads an object's bytes
(no relay, 2026-11-16: the page fetches chunks from the bucket itself, a new reader is a new relay). What is syntax, not a call, is found in each file's code with its comments and
string literals taken out, across lines: `static mut`, `unsafe impl ... Send/Sync for`; and the
store's tree format ("vnvm-tree", a string, so searched in the raw text) spelled outside
crates/kura/src/tree_list.rs (each new reader of a wire format is a second parser to keep in step).

PINNED: the unsafe kinds (masu-*, undocumented unsafe, unsafe ops in unsafe fns, pointer casts,
allow-attr) are also kept site by site in tools/lint-ratchet-sites.tsv: each one's file, lint and
the text of its line (spaces collapsed). A count alone let one go for another (review 2026-11-15:
"a from_raw_parts removed here, a transmute added there"); here every site that is not in the list
fails, even when the count stays. A line with one on it that is edited is a new site: fix it, or
for code moved as it is, --raise its file (and say so in the commit).
"""
import collections, json, os, re, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
TSV = os.path.join(ROOT, "tools", "lint-ratchet.tsv")
SITES = os.path.join(ROOT, "tools", "lint-ratchet-sites.tsv")
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
PINNED = {"undocumented_unsafe_blocks", "multiple_unsafe_ops_per_block", "missing_safety_doc", "not_unsafe_ptr_arg_deref", "unsafe_op_in_unsafe_fn", "cast_ptr_alignment", "ptr_as_ptr", "borrow_as_ptr", "missing_transmute_annotations", "allow-attr"}
TRACKED = re.compile(r"\b(?:clippy::(?:" + "|".join(LINTS + ROBUST + GROUPS) + ")|" + "|".join(RUSTC.values()) + r")\b")
# (code with comments and strings taken out: across lines, `[^{]*` stops at the impl's body)
CUSTOM = {
    "masu-static_mut": re.compile(r"\bstatic\s+mut\b"),
    "masu-unsafe_send_sync": re.compile(r"\bunsafe\s+impl\b[^{;]*?\b(?:Send|Sync)\s+for\b"),
}
# (searched in the code with its strings: it is one)
RAW = {"masu-wire_tree": re.compile(r"vnvm-tree")}
CUSTOM_HOME = {"masu-wire_tree": "crates/kura/src/tree_list.rs"}
# clippy's disallowed_methods (clippy.toml lists the same paths), by the path its message names
DISALLOWED = {
    "alloc::vec::Vec::set_len": "masu-set_len",
    "core::slice::from_raw_parts": "masu-from_raw_parts",
    "core::slice::from_raw_parts_mut": "masu-from_raw_parts",
    "core::intrinsics::transmute": "masu-transmute",
    "core::mem::transmute_copy": "masu-transmute",
    "core::mem::zeroed": "masu-uninit",
    "core::mem::MaybeUninit::assume_init": "masu-uninit",
}
DISALLOWED_PATH = re.compile(r"`([^`]+)`")
ALLOW = re.compile(r"#!?\[\s*(?:allow|expect)\s*\(([^\]]*)\)\s*\]")


def clippy(root=ROOT):
    yield from run_clippy(["--all-targets"], LINTS + ["clippy::" + g for g in GROUPS], root)
    yield from run_clippy(["--lib", "--bins"], ROBUST + ["unsafe_op_in_unsafe_fn", "disallowed_methods"], root)


seen = set()


def run_clippy(targets, lints, root=ROOT):
    args = ["cargo", "clippy", "--workspace", *targets, "--keep-going", "--message-format=json", "--", "--cap-lints", "warn", "-A", "clippy::all"]
    for l in lints:
        args += ["-W", l if "::" in l or l in RUSTC.values() else "clippy::" + l]
    # (the warnings are what it counts: CI's CARGO_BUILD_WARNINGS=deny would fail the run)
    env = dict(os.environ, CARGO_BUILD_WARNINGS="warn")
    if root != ROOT:
        # (the self-test's fixture: the repository's clippy.toml, a target dir of its own)
        env.update(CLIPPY_CONF_DIR=ROOT, CARGO_TARGET_DIR=os.path.join(ROOT, "target", "lint-fixtures"))
    p = subprocess.run(args, cwd=root, stdout=subprocess.PIPE, text=True, env=env)
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
        if code == "clippy::disallowed_methods":
            m = DISALLOWED_PATH.search(msg["message"])
            name = DISALLOWED.get(m.group(1) if m else "")
            if not name:
                sys.exit(f"lint-ratchet: a disallowed method this script does not name: {msg['message']}")
            code = "clippy::" + name
        sp = [s for s in msg["spans"] if s["is_primary"]]
        if not sp:
            continue
        # (a lib's code is checked twice with --all-targets: as the lib and as its tests)
        key = (os.path.relpath(os.path.join(root, sp[0]["file_name"]), root), sp[0]["line_start"], sp[0]["column_start"], code[8:])
        if key not in seen:
            seen.add(key)
            yield key
    if p.returncode != 0:
        sys.exit("lint-ratchet: clippy failed (above)")


def allow_attrs(root=ROOT):
    for top in ("src",):
        for d, _, fs in os.walk(os.path.join(root, top)):
            for f in fs:
                if not f.endswith(".rs"):
                    continue
                path = os.path.join(d, f)
                rel = os.path.relpath(path, root)
                for i, line in enumerate(open(path, encoding="utf-8", errors="replace"), 1):
                    for m in ALLOW.finditer(line):
                        if TRACKED.search(m.group(1)):
                            yield (rel, i, m.start() + 1, "allow-attr")


RAW_STR = re.compile(r'b?r(#*)"')
CHAR_LIT = re.compile(r"'(?:\\(?:[^u]|u\{[0-9a-fA-F]+\})[^']*|[^'\\\n])'")
PLAIN = re.compile(r"[^/\"'br]+")


def code_only(text, strings=False):
    """`text` with its comments and string/char literals blanked (newlines kept, so that line
    numbers stay; `strings`: the string literals kept): a small Rust lexer, enough for raw
    strings, nested block comments, lifetimes."""
    out, i, n = [], 0, len(text)
    def blank(j):
        out.append("".join(c if c == "\n" else " " for c in text[i:j]))
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(j)
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(j)
            i = j
        elif c in "br" and (m := RAW_STR.match(text, i)) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            end = '"' + m.group(1)
            j = text.find(end, m.end())
            j = n if j < 0 else j + len(end)
            out.append(text[i:j]) if strings else blank(j)
            i = j
        elif c == '"' or (c == "b" and text.startswith('b"', i) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_"))):
            j = i + (2 if c == "b" else 1)
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append(text[i:min(j + 1, n)]) if strings else blank(min(j + 1, n))
            i = min(j + 1, n)
        elif c == "'" and (m := CHAR_LIT.match(text, i)):
            blank(m.end())
            i = m.end()
        else:
            # (a run of plain code at once: one character at a time made big files slow)
            m = PLAIN.match(text, i)
            j = m.end() if m and m.end() > i else i + 1
            out.append(text[i:j])
            i = j
    return "".join(out)


def custom(root=ROOT):
    for d, _, fs in os.walk(os.path.join(root, "src")):
        for f in fs:
            if not f.endswith(".rs"):
                continue
            path = os.path.join(d, f)
            rel = os.path.relpath(path, root)
            text = open(path, encoding="utf-8", errors="replace").read()
            for src, rules in ((code_only(text), CUSTOM), (code_only(text, strings=True), RAW)):
                for name, rx in rules.items():
                    if CUSTOM_HOME.get(name) == rel:
                        continue
                    for m in rx.finditer(src):
                        yield (rel, src.count("\n", 0, m.start()) + 1, m.start() + 1, name)


def pinned(lint):
    return lint in PINNED or lint.startswith("masu-")


lines = {}


def line_text(f, n):
    """line `n` of `f` without its comments, its runs of whitespace one space: what a site is known by"""
    if f not in lines:
        # (comments out: a reworded comment is the same site)
        lines[f] = code_only(open(os.path.join(ROOT, f), encoding="utf-8", errors="replace").read(), strings=True).split("\n")
    v = lines[f]
    return " ".join(v[n - 1].split()) if 0 < n <= len(v) else ""


def count():
    per_file = collections.Counter()
    sites = collections.Counter()
    where = {}
    for f, n, _, lint in list(clippy()) + list(allow_attrs()) + list(custom()):
        if not TEST_FILE.search(f):
            per_file[(f, lint)] += 1
            if pinned(lint):
                k = (f, lint, line_text(f, n))
                sites[k] += 1
                where.setdefault(k, n)
    return per_file, sites, where


def load_sites():
    sites = collections.Counter()
    if os.path.exists(SITES):
        for line in open(SITES, encoding="utf-8"):
            if line.startswith("#") or not line.strip():
                continue
            f, lint, text = line.rstrip("\n").split("\t", 2)
            sites[(f, lint, text)] += 1
    return sites


def save_sites(sites):
    with open(SITES, "w", encoding="utf-8") as o:
        o.write("# tools/lint-ratchet.py: every unsafe-kind site (file, lint, its line's text); a site not here fails CI\n")
        for (f, l, t), n in sorted(sites.items()):
            for _ in range(n):
                o.write(f"{f}\t{l}\t{t}\n")


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


def fresh_sites(sites, old_sites, raise_files=()):
    """the pinned sites more often in `sites` than in the list: a count that stays the same does
    not hide one site traded for another"""
    return sorted(k for k, n in sites.items() if n > old_sites.get(k, 0) and k[0] not in raise_files)


FIXTURE = os.path.join(ROOT, "tools", "lint-fixtures", "ratchet")
MARK = re.compile(r"^(?!\s*//).*//~\s*(.*)$")


def self_test():
    """Every lint this script counts, against tools/lint-fixtures/ratchet: a small workspace
    whose lines each carry the regression one lint is there for, marked `//~ lint...`. Each mark
    must be found on its line and nothing unmarked may be: a lint renamed or dropped by a clippy
    upgrade, a clippy.toml path that stops resolving, a regex that stops matching, all fail here
    instead of silently counting 0. And every lint in the lists has a mark."""
    want, groups = set(), set()
    for d, _, fs in os.walk(FIXTURE):
        for f in fs:
            if f.endswith(".rs"):
                path = os.path.join(d, f)
                rel = os.path.relpath(path, FIXTURE)
                for i, line in enumerate(open(path, encoding="utf-8"), 1):
                    if m := MARK.search(line):
                        for l in m.group(1).split():
                            # (`eq_op@correctness`: a lint counted through its group)
                            l, _, g = l.partition("@")
                            want.add((rel, i, l))
                            if g:
                                groups.add(g)
    got = {(f, n, l) for f, n, _, l in list(clippy(FIXTURE)) + list(allow_attrs(FIXTURE)) + list(custom(FIXTURE))}
    bad = 0
    for f, n, l in sorted(want - got):
        print(f"lint-ratchet self-test: {f}:{n}: {l} did not fire")
        bad += 1
    for f, n, l in sorted(got - want):
        print(f"lint-ratchet self-test: {f}:{n}: {l} fired where no mark says it should")
        bad += 1
    names = {l for _, _, l in want}
    every = set(LINTS + ROBUST + list(RUSTC.values()) + list(CUSTOM) + list(RAW) + list(DISALLOWED.values()) + ["allow-attr"])
    for l in sorted(every - names):
        print(f"lint-ratchet self-test: {l} has no fixture line (tools/lint-fixtures/ratchet)")
        bad += 1
    for g in sorted(set(GROUPS) - groups):
        print(f"lint-ratchet self-test: no fixture line for clippy's {g} group (mark it `<lint>@{g}`)")
        bad += 1
    # (the site list: a site traded for another at the same count is new)
    a, b = ("f.rs", "masu-transmute", "x"), ("f.rs", "masu-transmute", "y")
    if fresh_sites(collections.Counter([b]), collections.Counter([a])) != [b] or fresh_sites(collections.Counter([a]), collections.Counter([a])):
        print("lint-ratchet self-test: a traded site is not caught")
        bad += 1
    if bad:
        sys.exit(f"lint-ratchet self-test: {bad} failures")
    print(f"lint-ratchet self-test: ok ({len(want)} marks, {len(names)} lints)")


def main():
    if "--self-test" in sys.argv:
        return self_test()
    update = "--update" in sys.argv
    raise_files = set(sys.argv[sys.argv.index("--raise") + 1 :]) if "--raise" in sys.argv else set()
    files, sites, where = count()
    old = load()
    old_sites = load_sites()
    up = sorted(k for k, n in files.items() if n > old.get(k, 0) and k[0] not in raise_files)
    down = sorted(k for k, n in old.items() if files.get(k, 0) < n)
    # (the first --update writes every site down: before it there was no list)
    first = not os.path.exists(SITES)
    new_sites = [] if first else fresh_sites(sites, old_sites, raise_files)
    for f, l in up:
        print(f"lint-ratchet: {f}: clippy::{l} {old.get((f, l), 0)} -> {files[(f, l)]}")
    for k in new_sites:
        print(f"lint-ratchet: {k[0]}:{where[k]}: {k[1]}: a new site (not in {os.path.relpath(SITES, ROOT)}): {k[2]}")
    up += new_sites
    if update:
        if up:
            sys.exit("lint-ratchet: counts above went up or sites are new: fix them (the baseline only goes down)")
        new = {k: (n if k[0] in raise_files else min(n, old.get(k, n))) for k, n in files.items()}
        save(new)
        save_sites(collections.Counter({k: (n if first or k[0] in raise_files else min(n, old_sites.get(k, 0))) for k, n in sites.items()}))
        print(f"lint-ratchet: wrote {os.path.relpath(TSV, ROOT)} ({sum(new.values())} in all, {sum(old.values()) - sum(new.values())} fewer)")
        return
    if down:
        print(f"lint-ratchet: {sum(old[k] - files.get(k, 0) for k in down)} fewer than the baseline in {len(set(f for f, _ in down))} files (tools/lint-ratchet.py --update writes it down)")
    if up:
        sys.exit("lint-ratchet: lints went up or unsafe sites are new (above): From/TryFrom instead of `as`, every variant named instead of `_`, `?` or a typed error instead of unwrap/panic, get()/checked_*/wrapping_* instead of a[i] and bare arithmetic, // SAFETY: on unsafe")
    print(f"lint-ratchet: ok ({sum(files.values())} left)")


if __name__ == "__main__":
    main()
