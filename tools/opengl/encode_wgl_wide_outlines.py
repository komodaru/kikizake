"""Pack representative Windows wide glyph outlines and their code-unit aliases."""

from collections import Counter
from functools import cache
from hashlib import sha256
from pathlib import Path
import struct
import sys

from encode_wgl_outlines import VIEWPORTS, coordinate_interval, primitives, viewport_coordinate

CODE_UNITS = 65536
BYTE_ALIAS = 1 << 31


def native_pair(directory, name):
    x86 = directory / f"{name}-x86.txt"
    x64 = directory / f"{name}-x64.txt"
    def digest(path):
        h = sha256()
        with path.open("rb") as source:
            for block in iter(lambda: source.read(65536), b""):
                h.update(block)
        return h.digest()
    if digest(x86) != digest(x64):
        raise ValueError(f"{name}: native Windows x86 and x64 captures differ")
    return x64


def representatives(directory):
    path = native_pair(directory, "outline-wide")
    lines = path.read_text(encoding="ascii").splitlines()
    assert lines.pop() == "done" and len(lines) == CODE_UNITS
    aliases = {}
    result = []
    for code, row in enumerate(lines):
        character, lines_id, fill_id = row.split()
        assert int(character, 16) == code
        key = (lines_id, fill_id)
        result.append(aliases.setdefault(key, code))
    return result, list(aliases.values())


def capture_index(source):
    positions = {}
    while True:
        header = source.readline()
        if header.strip() == b"done":
            return positions
        kind, size, code, mode, wide, ok, error, *metrics = header.split()
        assert kind == b"glyph" and (wide, ok, error) == (b"1", b"1", b"0")
        key = (int(size), int(code), int(mode))
        assert key not in positions and len(metrics) == 6
        positions[key] = (source.tell(), list(map(float, metrics)))
        source.readline()


@cache
def recover_float(observations, y_axis):
    lower, upper = coordinate_interval(observations, y_axis)
    midpoint = (lower + upper) / 2
    bits = struct.unpack("<I", struct.pack("<f", midpoint))[0]
    words = sorted(range(max(0, bits - 20), bits + 21), key=lambda word: abs(word - bits))
    candidates = [round(midpoint * 2048) / 2048]
    candidates.extend(struct.unpack("<f", struct.pack("<I", word))[0] for word in words)
    for candidate in candidates:
        if all(viewport_coordinate(size, divisor, candidate, y_axis) == value
               for size, divisor, value in observations):
            return candidate
    raise ValueError(f"inconsistent wide glyph coordinates {observations}, y_axis={y_axis}")


def geometry(source, index, code, mode):
    observed = []
    all_metrics = []
    for size in VIEWPORTS:
        start, metrics = index[size, code, mode]
        source.seek(start)
        kind, length, *words = source.readline().split()
        assert kind == b"data" and len(words) == int(length)
        coords = struct.unpack(f"<{len(words)}f", b"".join(int(word, 16).to_bytes(4, "little") for word in words))
        observed.append(primitives(coords, mode))
        all_metrics.append(metrics)
    assert all(m == all_metrics[0] for m in all_metrics)
    recovered = []
    for group in zip(*observed, strict=True):
        shape = []
        for vertices in zip(*group, strict=True):
            point = tuple(
                recover_float(tuple((size, 2, value) for size, value in zip(VIEWPORTS, values, strict=True)), axis == 1)
                for axis, values in enumerate(zip(*vertices))
            )
            shape.append(point)
        recovered.append(shape)
    return all_metrics[0], recovered


def outline_record(source, index, code):
    metrics, loops = geometry(source, index, code, 0)
    fill_metrics, fill = geometry(source, index, code, 1)
    assert metrics == fill_metrics
    triangle_vertices = fill[0] if fill else []
    vertices = {}
    for group in [*loops, triangle_vertices]:
        for point in group:
            vertices.setdefault(point, len(vertices))

    out = bytearray(struct.pack("<6fHHH", *metrics, len(vertices), len(loops), len(triangle_vertices)))
    for point in vertices:
        out.extend(struct.pack("<ff", *point))
    for loop in loops:
        out.extend(struct.pack("<H", len(loop)))
        out.extend(struct.pack(f"<{len(loop)}H", *(vertices[p] for p in loop)))
    out.extend(struct.pack(f"<{len(triangle_vertices)}H", *(vertices[p] for p in triangle_vertices)))
    recover_float.cache_clear()
    return out


def pack(directory, aliases, codes):
    data = bytearray()
    offsets = {}
    with native_pair(directory, "outline-reps").open("rb") as source:
        index = capture_index(source)
        for code in codes:
            if code < 256:
                offsets[code] = BYTE_ALIAS | code
            else:
                offsets[code] = len(data)
                data.extend(outline_record(source, index, code))
    table = struct.pack(f"<{CODE_UNITS}I", *(offsets[code] for code in aliases))
    return table + data


def readback_fixture(directory):
    source = native_pair(directory, "outline-readback")
    rows = source.read_text(encoding="ascii").splitlines()
    assert rows.pop() == "done" and len(rows) == CODE_UNITS
    counts = []
    digests = []
    for code, row in enumerate(rows):
        character, lines, faces = row.split()
        assert int(character, 16) == code
        if code % 256 == 0:
            digest = 14695981039346656037
        returns = []
        for capture in (lines, faces):
            written, contents = capture.split(":")
            written = int(written)
            assert -1 <= written <= 1024
            returns.append(written)
            for byte in struct.pack("<iQ", written, int(contents, 16)):
                digest = ((digest ^ byte) * 1099511628211) & 0xffffffffffffffff
        counts.append(tuple(returns))
        if code % 256 == 255:
            digests.append(f"{digest:016x}")
    default, _ = Counter(counts).most_common(1)[0]
    exceptions = [f"{code:04x} {line} {face}" for code, (line, face) in enumerate(counts) if (line, face) != default]
    return "\n".join([f"{default[0]} {default[1]}", *exceptions, "", *digests]) + "\n"


def main(directory, phase):
    aliases, codes = representatives(directory)
    if phase == "codes":
        for code in codes:
            if code >= 256:
                print(code)
        return
    if phase != "pack":
        sys.exit(f"unknown phase {phase}")
    root = Path(__file__).resolve().parents[2]
    path = root / "src/model/opengl_arial_outline_wide.bin"
    path.write_bytes(pack(directory, aliases, codes))
    (root / "tests/fixtures/opengl_wide_outline_feedback.txt").write_text(
        readback_fixture(directory), encoding="ascii", newline="\n"
    )


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit("usage: encode_wgl_wide_outlines.py CAPTURE_DIRECTORY codes|pack")
    main(Path(sys.argv[1]), sys.argv[2])
