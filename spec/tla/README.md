# TLA+ specs

Each spec writes what the clauses it names (spec/laws.txt, kind `tla`) say about a sequence of
calls, as a state machine whose variable `log` is that sequence as a kikizake case: lines
starting `> ` are calls, the rest the answers the documentation gives. TLC explores every
sequence up to `MaxOps` calls (the .cfg), checks the invariants, and dumps every state;
`kikizake tla <dump>` runs each state's log through the models and fails if one answers
differently. So a model that breaks a clause on any short sequence fails, without Windows.

    java -cp tla2tools.jar tlc2.TLC -workers auto -dump /tmp/GdiPath.dump -config GdiPath.cfg GdiPath.tla
    cargo run --release -- tla /tmp/GdiPath.dump

| spec | clauses | states (MaxOps) |
|---|---|---|
| GdiPath | BeginPath, EndPath, AbortPath, CloseFigure, GetPath, MoveToEx, LineTo (GDI.PATH.*, GDI.POS.MOVE) | 402,234 (5) |
| GdiRegion | CreateRectRgn, SetRectRgn, CombineRgn, OffsetRgn, GetRgnBox, PtInRegion, RectInRegion, EqualRgn (GDI.RGN.*): a region as its set of pixels, every query asked after every call | 8,379 (2) |
| GdiClip | IntersectClipRect, ExcludeClipRect, SelectClipRgn / ExtSelectClipRgn, OffsetClipRgn, GetClipBox, GetClipRgn, PtVisible, RectVisible (GDI.CLIP.*): the clip region as its set of pixels on a 6 by 4 bitmap, every query asked after every call | 14,425 (3) |
| GdiMap | SetMapMode, Set/Scale Window and Viewport ExtEx, Set/Offset origins, the state read back (GDI.MAP.*): MM_ISOTROPIC's shrunk extents stored at the first read, in exact rationals | 168,421 (4) |
| GdiWorld | SetGraphicsMode, SetWorldTransform, ModifyWorldTransform, GetWorldTransform, CombineTransform (GDI.XFORM.*): whole non-negative entries, exact in f32, answered as float bits | 30,784 (3) |
| GdiModes | SetROP2, SetPolyFillMode, SetArcDirection, SetMiterLimit (GDI.STATE.OLD, GDI.PATH.MITER): each the previous value, out of range refused | 111,151 (4) |
| GdiPaint | PatBlt's operations of the pattern and destination, solid and DIB pattern brushes, the brush origin, the clip region, BitBlt from a 16 bpp and a color table source, GetPixel / SetPixel (GDI.PATBLT.ROPS, GDI.BLT.*, GDI.BRUSH.*, GDI.PIXEL.*): every pixel of a 4 by 2 32 bpp DIB after each call | 230,764 (3) |
| GdiShapes | the default pen, CreatePen, SetROP2(0), LineTo, Polyline(To), FillRect, FrameRect, InvertRect, Rectangle, FillRgn, PaintRgn, InvertRgn (GDI.LINE.POLY*, GDI.RECT.*, GDI.SHAPE.RECT*, GDI.RGN.FILL/PAINT/INVERT, GDI.ROP2.ZERO) on a 4 by 2 8 bpp DIB | 120,100 (3) |
| GdiDdb | CreateBitmap, CreateCompatibleBitmap and their failures, GetObject, GetBitmapBits, GetDeviceCaps, GetObjectType, BitBlt between monochrome and color DDBs (GDI.DDB.*) | 219,661 (3) |
| GdiBlit | AlphaBlend, TransparentBlt, MaskBlt, PlgBlt: refusals and pixels (GDI.ALPHA.*, GDI.TRANS.*, GDI.MASK.*, GDI.PLG.*) on a 4 by 2 32 bpp DIB | 87,165 (3) |
| GdiText | SetTextAlign, SetTextCharacterExtra, ExtTextOut's opaque rectangle without characters (GDI.TEXT.ALIGN, EXTRA, NOTEXT, OPAQUE, OPAQUEORDER) | 65,641 (3) |
| GdiPathFill | FlattenPath, StrokePath, FillPath, StrokeAndFillPath, PathToRegion, SelectClipPath of whole figures (GDI.PATH.FLATTEN, STROKE, NOPATH, FILL, EMPTYFILL) on a 3 by 3 8 bpp DIB | 1,952,343 (5) |
| D3D9StateBlock | SetRenderState, Create/Begin/EndStateBlock, Capture, Apply (D3D9.SETGET, D3D9.SB.*, D3D9.RS.DEFAULT) | 180,869 (4) |
| D3D9States, D3D8States | every render, stage and sampler state read on a new device and set then read (D3D9/D3D8 .DEVICE, RS.DEFAULT, RS.UNDOC, TSS.DEFAULT, TSS.UNDOC, SS.DEFAULT, SET.UNDOC, D3D9.RS.POINTSIZEMAX, D3D8.RS.OWN): the tables Windows' answers (golden d3d9-64, d3d8-32), written by states.py | 1,645 / 2,979 (1) |
| D3D9Groups, D3D8Groups | CreateStateBlock of each type, a state set after it, the block applied (D3D9.SB.TYPES, D3D8.SB.GROUPS): each type's states as Windows' create-1..3 answers list them, written by states.py | 1,563 / 876 (1) |
| D3DSbMisuse | Begin twice, End without Begin, Apply/Capture/Create while recording, types 0 and 4, Release while recording, on Direct3D 9 and 8 (D3D9.SB.MISUSE, SB.BADTYPE) | 289,286 (4) |
| DxgiSwapChain | CreateSwapChainForHwnd, GetBuffer, Present, ResizeBuffers, Set/GetMaximumFrameLatency, GetFrameLatencyWaitableObject (DXGI.BUFFER, PRESENT, PRESENT.TEST, PRESENT.RESTART, RESIZE, LATENCY, CHAIN.SIZE) | 211,932 (3) |
| DxgiFactory | CreateDXGIFactory2, EnumWarpAdapter, CheckFeatureSupport (ALLOW_TEARING), EnumAdapters1, EnumAdapterByGpuPreference (DXGI.FACTORY, FACTORY.DEBUG, WARP, TEARING, ADAPTERS, ADAPTER.PREF): what the windows-2025 runner answers, nofactory before a factory | 40,495 (3) |
| GdiPlusBitmap | GdiplusStartup, Bitmap::Bitmap, GetPixelFormat / GetPalette, LockBits, GetPixel, SetPixel, Clone (GP.START, CREATE, INFO, LOCK, GRAY16, PIXEL, CLONE): 8 formats' integer conversions, every lock rectangle and pixel asked after every call | 69,216 (2) |
| GdiPlusFill | Graphics::Clear, FillRectangleI (GP.FILL, NOGRAPHICS, RECT, NEGATIVE, COPY, OVER, INDEXED, QUALITY.OTHER): SourceCopy and SourceOver without gamma, the operators of GdiPlusBitmap by INSTANCE | 368,697 (2) |
| GdiPlusMatrix | Matrix::Matrix, Multiply, Invert, TransformPoints (GP.MATRIX, MUL, ORDER, POINT, INVERT): entries on the 1/64 grid, exact in f32 but for the signs of zeros, answered as float bits | 22,765 (3) |
| GdiPlusObjects | solid, hatch and texture brushes and GetBrushType, Pen and its setters, FillRectangle, SetColorMatrix and DrawImage at 1:1 (GP.BRUSH, HATCH.RANGE, TEXTURE, TEXTURE.STATUS, INFO, PEN.MAKE, PAINT, NOBRUSH, MODES, NEGATIVE, ATTR, FLAGS, DRAW.NOATTR, SKIPGRAYS, ALTGRAY, ALTGRAY.TRANSLATE) | 7,429 (2) |

D3D9StateBlock models documented use; D3DSbMisuse the measured misuse. Comments cannot hold `*)`.
