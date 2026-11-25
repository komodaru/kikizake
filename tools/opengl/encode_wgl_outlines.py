"""Build the Arial outline geometry and test fixtures from paired native captures."""

from functools import cache
import math
from pathlib import Path
import struct
import sys

VIEWPORTS = (4096, 8192, 16384)
COORD_UNIT = 1 << 23
RASTER_BIAS = 40960
DIB_HEIGHT = 16
OUTPUT_WORDS = 1024


def f32(value):
    return struct.unpack("<f", struct.pack("<f", value))[0]


def read_recordings(directory, name):
    x86 = (directory / f"{name}-x86.txt").read_text(encoding="ascii")
    x64 = (directory / f"{name}-x64.txt").read_text(encoding="ascii")
    if x86 != x64:
        raise ValueError(f"{name}: native x86 and x64 output differ")
    lines = x64.splitlines()
    assert lines.pop() == "done"
    cases = {}
    for header, body in zip(lines[::2], lines[1::2], strict=True):
        kind, size, code, mode, wide, ok, err, *metrics = header.split()
        assert kind == "glyph" and (ok, err) == ("1", "0") and len(metrics) == 6, header
        payload, quantity, *words = body.split()
        if payload == "pixels":
            assert not words
            result = bytes.fromhex(quantity)
            assert len(result) == DIB_HEIGHT * DIB_HEIGHT * 4
        else:
            assert payload == "data" and len(words) == int(quantity), header
            result = tuple(struct.unpack("<f", int(word, 16).to_bytes(4, "little"))[0] for word in words)
        key = tuple(map(int, (size, code, mode, wide)))
        assert key not in cases
        cases[key] = (" ".join(metrics), result)
    return cases


def primitives(data, mode):
    words = iter(data)
    if mode == 0:
        loops = []
        for kind, x, y, next_x, next_y in zip(*([words] * 5), strict=True):
            if kind == 1799:  # GL_LINE_RESET_TOKEN
                loops.append([(x, y), (next_x, next_y)])
            else:
                assert kind == 1794 and (x, y) == loops[-1][-1]
                loops[-1].append((next_x, next_y))
        for loop in loops:
            assert loop[0] == loop.pop()
        return loops

    triangles = []
    for kind, count, x, y, x2, y2, x3, y3 in zip(*([words] * 8), strict=True):
        assert (kind, count) == (1795, 3)  # GL_POLYGON_TOKEN
        triangles.extend(((x, y), (x2, y2), (x3, y3)))
    return [triangles] if triangles else []


def viewport_coordinate(size, divisor, vertex, y_axis):
    """Match the generic rasterizer's viewport transform and feedback bias."""
    half = size / 2
    delta = f32(vertex * ((half - 1 / 256) / divisor))
    if y_axis:
        center = RASTER_BIAS + DIB_HEIGHT - half
        return RASTER_BIAS + DIB_HEIGHT - f32(center - delta)
    center = RASTER_BIAS + half
    return f32(center + delta) - RASTER_BIAS


def coordinate_interval(observations, y_axis):
    # Each viewport reports a rounded coordinate.
    intervals = []
    for size, divisor, value in observations:
        half = size / 2
        scale = (half - 1 / 256) / divisor
        raster_coord = RASTER_BIAS + (DIB_HEIGHT - value if y_axis else value)
        ulp = 2 ** (math.frexp(raster_coord)[1] - 24)
        estimate = (value - half) / scale
        margin = ulp / (2 * scale)
        intervals.append((estimate - margin, estimate + margin))
    return max(lo for lo, _ in intervals), min(hi for _, hi in intervals)


@cache
def recover_coordinate(observations, y_axis):
    lower, upper = coordinate_interval(observations, y_axis)
    midpoint = round((lower + upper) * COORD_UNIT / 2)
    # Prefer an exact 1/2048 font unit when it fits the measured rounding.
    candidates = sorted(
        range(midpoint - 12, midpoint + 13),
        key=lambda candidate: (candidate % (COORD_UNIT // 2048) != 0, abs(candidate - midpoint)),
    )
    for candidate in candidates:
        if all(
            viewport_coordinate(size, divisor, candidate / COORD_UNIT, y_axis) == measured
            for size, divisor, measured in observations
        ):
            return candidate
    raise ValueError(f"inconsistent native coordinates {observations}, y_axis={y_axis}")


def recover_primitives(feedback, unscaled, code, mode, wide):
    captures = [(size, 2, feedback[size, code, mode, wide][1]) for size in VIEWPORTS]
    captures.extend(
        (size, 1, data)
        for (size, c, m, w), (_, data) in unscaled.items()
        if (c, m, w) == (code, mode, wide)
    )
    observed = [primitives(data, mode) for _, _, data in captures]
    camera = [(size, divisor) for size, divisor, _ in captures]
    coords = []
    for groups in zip(*observed, strict=True):
        recovered = []
        for vertices in zip(*groups, strict=True):
            xy = []
            for axis, values in enumerate(zip(*vertices)):
                measurements = tuple(
                    (size, divisor, value)
                    for (size, divisor), value in zip(camera, values, strict=True)
                )
                xy.append(recover_coordinate(measurements, axis == 1))
            recovered.append(tuple(xy))
        coords.append(recovered)
    return coords


def geometry(feedback, unscaled, code, wide):
    metrics = feedback[VIEWPORTS[-1], code, 0, wide][0]
    assert all(feedback[size, code, mode, wide][0] == metrics for size in VIEWPORTS for mode in (0, 1))
    loops = recover_primitives(feedback, unscaled, code, 0, wide)
    face = recover_primitives(feedback, unscaled, code, 1, wide)
    triangles = face[0] if face else []
    indices = {}
    for vertex in (vertex for group in [*loops, triangles] for vertex in group):
        indices.setdefault(vertex, len(indices))
    xy = " ".join(f"{x:x} {y:x}" for x, y in indices)
    contours = "/".join(" ".join(f"{indices[v]:x}" for v in loop) for loop in loops)
    faces = " ".join(f"{indices[v]:x}" for v in triangles)
    return " | ".join((metrics, xy, contours, faces)).rstrip()


def checksum(data):
    digest = 2166136261
    for byte in data:
        digest = ((digest ^ byte) * 16777619) & 0xffffffff
    return f"{digest:08x}"


def main(directory):
    feedback = read_recordings(directory, "feedback")
    unscaled = read_recordings(directory, "unscaled")
    pixels = read_recordings(directory, "pixels")
    for offset in ("0x100", "0x10000", "0x80000000", "0xffffff00"):
        aliases = read_recordings(directory, f"aliases-{offset}")
        for key, value in pixels.items():
            if key[3] == 0 or int(offset, 16) % 65536 == 0:
                assert aliases[key] == value, (key, offset)
    keys = [(code, wide) for wide in (0, 1) for code in range(256)]
    rows = {}
    lookup = []
    for code, wide in keys:
        row = geometry(feedback, unscaled, code, wide)
        lookup.append(rows.setdefault(row, len(rows)))

    root = Path(__file__).resolve().parents[2]
    model = root / "src/model/opengl_arial_outline_bytes.txt"
    model.write_text(" ".join(f"{i:x}" for i in lookup) + "\n" + "\n".join(rows) + "\n", newline="\n")

    fixture = root / "tests/fixtures/opengl_outline_byte_range.txt"
    with fixture.open("w", encoding="ascii", newline="\n") as out:
        for code, wide in keys:
            for mode in (0, 1):
                metrics, rgba = pixels[16, code, mode, wide]
                observations = []
                for size in VIEWPORTS:
                    measured_metrics, values = feedback[size, code, mode, wide]
                    assert measured_metrics == metrics
                    prefix = values[:OUTPUT_WORDS]
                    words = struct.pack(f"<{len(prefix)}f", *prefix)
                    observations.append(f"{len(values)}:{checksum(words)}")
                print("aw"[wide], code, mode, checksum(rgba), *observations, "| glyph", metrics, file=out)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python encode_wgl_outlines.py CAPTURE_DIRECTORY")
    main(Path(sys.argv[1]))
