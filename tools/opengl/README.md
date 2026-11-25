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

## Font creation and render modes

Run `capture_wgl_outlines.exe font-modes` on both architectures. The
72 rows exercise creation in render, feedback and selection modes with
clear and pending GL error state. Compare the two files and copy their
rows up to the `done` marker to
`tests/fixtures/opengl_font_modes.txt`. The recording includes GL and
Win32 errors, list state, raster position and feedback or selection data.

Run `capture_wgl_outlines.exe font-compiles` on both architectures for the
192 observations in `tests/fixtures/opengl_font_compiles.txt`. These
exercise base zero, an overlapping list name and a distinct destination
under both list compilation modes.

## Font creation in primitives

Run `capture_wgl_outlines.exe bitmap-primitives` on both architectures.
The 216 rows cover immediate mode and both display-list compilation modes.
Compare the two outputs and copy the rows up to `done` to
`tests/fixtures/opengl_bitmap_primitives.txt`.

`tests/fixtures/opengl_outline_watchdog.txt` records the six wide outline
inputs and shared x86/x64 stdout from a separate worker probe. Its columns
give the phase, encoding, first code unit, count, base, format and watchdog
duration in milliseconds. Phase 0 selects an immediate primitive and
phases 1 and 2 select the two display-list compilation modes. In each run
the worker flushed `calling` just before the WGL call and was stopped after
a 1.5-second watchdog wait. The worker flushes `returned` before ending the
primitive, and no run recorded that marker. The terminal behavior remains
unknown. The archived inputs exercise the model's `WGL.OTHER` guard before
outline format and count handling. The model regression separately checks
that guard for both encodings.

## Upper display-list names

The x86 and x64 recordings agreed on creation and replay for the sampled
large list names. The single maximum name `0xffffffff` remained usable
after creation. Unbinding succeeded, and deleting the context with that
list live produced `0xc0000005`. Deleting that name directly and deleting
the two-name range starting at `0xfffffffe` with the maximum live also
produced that status on both architectures.

`tests/fixtures/opengl_large_list_state.txt` keeps the paired observations
from the probe that flushed output around unbinding and deletion.
`tests/fixtures/opengl_max_list_lifetime.txt` keeps ad hoc runner
programs and their recorded output. A record header gives the case name
and signed process status. Lines starting with `>` are input and lines
starting with `<` are output. The Rust regression uses these saved
observations, including the output flushed before an interrupted case.

`tests/fixtures/opengl_upper_batch_prefix.txt` preserves the separate
two-glyph ANSI bitmap case that starts at `0xfffffffe` and its shared
x86/x64 stdout. Both native outputs stop at the case header with no saved
exit status. The runner flushes the header before dispatch, then resets
the context before flushing the case body. The saved prefix cannot locate
the interruption among setup, font generation, readback and cleanup. The
model reports `WGL.OTHER` when a multi-glyph request reaches generation
and ends at or crosses `0xffffffff`.

`tests/fixtures/opengl_upper_font_validation.txt` records 24 completed
requests at the two largest names, paired across architectures. Each
row gives the pending GL error flag, the font command, and the readback
lines after context creation. The test checks validation, error state,
survival of the neighboring list, and normal context release. The model
checks bitmap character range, primitive and GL error state, and empty
wide intervals before the list-name guard can reject generation. Outline
format and zero-count handling also precede that guard outside primitives.

## WGL regression coverage

| Conditions | Evidence |
| --- | --- |
| DIB pixel-format tables, selection, context state and format errors | The descriptor export regression covers all 36 formats of the reference 32-bit DIB. Other supported DIB depths use `pf-*`, `choose-*`, `ctx-pf-*` and `edge-pf-range` golden cases. |
| Extension lookup, layer contexts, sharing and copying | `gap-wgl-0` golden samples the export names and plane flags. Share and copy each use a temporary context, and the copy case uses mask `0x40000`. |
| Swaps on the single-buffer DIB | `gap-wgl-0` golden, the pixel and GL error preservation property test, and all counts 0 through 16 for multiple-buffer swaps with a Kani check of the result mask. |
| Palette and debug entry | Palette and debug entry laws, including Kani coverage of unsupported palette planes. `spec/opengl.txt` records the main-plane last-error difference between the reference and local hosts. |
| Bitmap and outline generation and replay | ANSI bytes and wide code-unit captures, GL error and input-range probes, both outline formats, render/feedback/select, and both display-list compilation modes |
| Display-list name arithmetic | Generated replacement-law cases and Kani checks of the last name and generation guard for arbitrary unsigned bases and signed counts |

The archive-based Rust tests for the two incomplete recordings exercise
model guards. Native terminal behavior for those inputs remains open in
`WGL.OTHER`.

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
