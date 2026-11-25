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
```

Run `python tools/opengl/encode_wgl_outlines.py DIRECTORY` with that
directory. The encoder checks that the architecture pairs match, then
recovers ordered loops and triangles from feedback at several viewports.
Feedback rounds window coordinates. The table uses object coordinates
that reproduce the captured rounding. Unscaled feedback for C through F
also constrains the points used by the earlier unscaled regression cases.
The independent pixel fixture covers every RGBA byte at a 16 by 16 viewport.

Verify the table and fixtures with
`cargo test --release --test laws_opengl wgl_`.
