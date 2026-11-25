"""Encode the paired native WGL bitmap captures and their readback witnesses."""

from pathlib import Path
import struct
import sys

N_CODES = 65536


def read_pair(directory, name):
    paths = [directory / f"bitmap-{name}-{arch}.txt" for arch in ("x86", "x64")]
    x86, x64 = (p.read_bytes() for p in paths)
    if x86 != x64:
        raise ValueError(f"{name}: Windows x86 and x64 captures differ")
    rows = x64.decode("ascii").splitlines()
    if rows.pop() != "done":
        raise ValueError(f"{name}: missing capture completion marker")
    return rows


def atlas_bytes(rows):
    if len(rows) != N_CODES:
        raise ValueError(f"atlas: expected {N_CODES} code units")
    palette = {}
    offsets = bytearray()
    glyphs = bytearray()
    for index, row in enumerate(rows):
        code, status, raster = row.split(" | ")
        assert int(code, 16) == index
        if index == N_CODES - 1:
            assert status == "1 0 0 0" and raster == "0 0 0 0 0 0 null"
            continue
        assert status == "1 0 1 0"
        width, height, origin_x, origin_y, move_x, move_y, kind, *bit_bytes = raster.split()
        width, height = int(width), int(height)
        bits = bytes(map(int, bit_bytes))
        assert len(bits) == ((width + 7) // 8) * height
        assert kind == ("ub" if bits else "null")
        glyph = struct.pack("BBbbbb", width, height, int(origin_x), int(origin_y), int(move_x), int(move_y)) + bits
        if glyph not in palette:
            palette[glyph] = len(glyphs)
            glyphs.extend(glyph)
        offsets.extend(palette[glyph].to_bytes(3, "little"))
    return bytes(offsets + glyphs)


def main(directory):
    atlas = atlas_bytes(read_pair(directory, "atlas"))
    readback = read_pair(directory, "readback")
    ranges = read_pair(directory, "ranges")
    assert len(readback) == N_CODES // 256
    assert all(len(row) == 16 and int(row, 16) >= 0 for row in readback)

    root = Path(__file__).resolve().parents[2]
    (root / "src/model/opengl_arial_bitmap_wide.bin").write_bytes(atlas)
    fixtures = root / "tests/fixtures"
    (fixtures / "opengl_wide_bitmap_readback.txt").write_text("\n".join(readback) + "\n", newline="\n")
    (fixtures / "opengl_bitmap_ranges.txt").write_text("\n".join(ranges) + "\n", newline="\n")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: python encode_wgl_bitmaps.py CAPTURE_DIRECTORY")
    main(Path(sys.argv[1]))
