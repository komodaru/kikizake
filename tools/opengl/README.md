# Windows WGL font outline captures

The Arial outline table and test fixtures come from a generic 32-bit DIB with
the same `CreateFontA` settings and zero deviation and extrusion as the
OpenGL runner. The byte range covers both WGL formats and both exports on
native x86 and x64. The original host ran Windows build 26300.9974 with
`opengl32.dll` 10.0.26100.9818 for both architectures.

Build and run the capture in each architecture's Visual Studio Developer
Command Prompt. Replace `x64` with `x86` in the x86 prompt. Run both sets
from the repository root and save the outputs in the same directory.

```bat
cl /nologo /W4 /O2 /MT tools\opengl\capture_wgl_outlines.c gdi32.lib opengl32.lib
capture_wgl_outlines.exe 0 255 0 > pixels-x64.txt
capture_wgl_outlines.exe 0 255 1 > feedback-x64.txt
capture_wgl_outlines.exe 67 70 2 > unscaled-x64.txt
for %o in (0x100 0x10000 0x80000000 0xffffff00) do capture_wgl_outlines.exe 0 255 0 %o > aliases-%o-x64.txt
```

Run `python tools/opengl/encode_wgl_outlines.py DIRECTORY` with that
directory. The encoder checks that the architecture pairs match, then
recovers ordered loops and triangles from feedback at several viewports.
Feedback rounds window coordinates. The table uses object coordinates
that reproduce the captured rounding. Unscaled feedback for C through F
also constrains the points used by the earlier unscaled regression cases.
The independent pixel fixture covers every RGBA byte at a 16 by 16 viewport.
The alias captures verify the same pixels and metrics for DWORD character
indices with the measured low byte or word.

Verify the table and fixtures with
`cargo test --release --test laws_opengl wgl_`.

## Wide outline captures

The wide outline capture covers all 65,536 input code units in both modes
with the same Arial face. A full feedback scan identifies glyphs whose
metrics and geometry match, including the default glyph for missing
characters. The representative captures measure the remaining unique
shapes at three viewports. A separate run measures every code unit with
the runner's 1024-word feedback cap.

Use the same `capture_wgl_outlines.exe` build and recording directory as
above. Capture the scan on both architectures, using the appropriate
suffix, then produce the representative code list.

```bat
capture_wgl_outlines.exe wide-scan > outline-wide-x64.txt
python -B tools\opengl\encode_wgl_wide_outlines.py . codes > outline-wide-codes.txt
capture_wgl_outlines.exe wide-reps outline-wide-codes.txt > outline-reps-x64.txt
capture_wgl_outlines.exe wide-readback > outline-readback-x64.txt
```

The `codes` command needs both scan files. Once both architectures have
also produced the representative and readback files, run
`python -B tools/opengl/encode_wgl_wide_outlines.py . pack`. The binary
table stores shared geometry, while the independent feedback fixture
checks metrics, returned word counts and the contents of each observed
buffer.

## Windows WGL bitmap captures

The bitmap atlas records all 65,536 input code units of
`wglUseFontBitmapsW` on the same Windows build. The host used system ANSI
code page 932 and its installed system fonts for fallback. The Arial font
file SHA-256 was
`edfbd22a3d088fc2f5d7c79b8c9c732df6dc878a60f0a80be0f92a3c7a634404`.
The model replays this font and fallback configuration.

Run these commands in both Visual Studio Developer Command Prompts from the
repository root. Use the `x86` file suffix in the x86 prompt.

```bat
cl /nologo /W4 /O2 /MT tools\opengl\capture_wgl_bitmaps.c gdi32.lib opengl32.lib
capture_wgl_bitmaps.exe atlas > bitmap-atlas-x64.txt
capture_wgl_bitmaps.exe readback > bitmap-readback-x64.txt
capture_wgl_bitmaps.exe ranges > bitmap-ranges-x64.txt
```

Run `python tools/opengl/encode_wgl_bitmaps.py DIRECTORY` when both sets
are in one directory. The encoder checks each architecture pair and packs
shared monochrome glyphs from the 32 by 32 DIB. The readback witnesses come
from separate draws at two raster positions in a 16 by 16 DIB. They cover
each display list's creation result, position, RGBA bytes and GL error.
The range captures use boundary batches and exercise both clear and
pending GL error state.
