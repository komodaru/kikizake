# dwrite.dll: GDI-compatible glyph metrics (clean-room analyst notes)

Builds analysed (the golden runner's):
- x64 dwrite.dll, PDB `dwrite.pdb` GUID+age `8996EC01E7F571CDCAFD60B16D0800E51`
- x86 dwrite.dll, PDB `dwrite.pdb` GUID+age `D38FE473E7B88760AC577A31948C178C1`

Public symbols only (no types, no locals). Addresses are RVAs (x64 / x86). These notes give
rules, constants and names only. Where a step was **inferred** (from a measured rule rather
than read in the code), it says so.

Notation: `f32(...)` is IEEE single with round-to-nearest-even. Every float op here is SSE
scalar on both archs, and GetGdiCompatibleGlyphMetrics sets MXCSR to 0x1F80 (RNE, all masked)
for the call. `tdiv(a,b)` is C integer division (truncates toward 0). `>>` is an arithmetic
shift (floor). "16.16" is a signed 32-bit fixed value, "26.6" likewise.

---------------------------------------------------------------------------------------------

## Q1. IDWriteFontFace::GetGdiCompatibleGlyphMetrics

### Call chain

| step | symbol | x64 | x86 |
|---|---|---|---|
| COM entry | `DWriteFontFace::GetGdiCompatibleGlyphMetrics` | 0x6f5a0 | 0x13fd60 |
| | `FontFace::GetGlyphMetrics(ctx, em, ppd, matrix, mode, sideways, ...)` | 0x6f814 | 0x15632c |
| transform | `MakeRasterizerTransform(em, ppdX, ppdY, matrix, isSideways, gdiCompat, &ppemByte)` | 0x710c0 | 0xdb810 |
| cache | `GetGlyphMetrics(ctx, face, Transform, ...)` (GlyphDataElement cache) | 0x40440 | 0x169ff4 |
| | `GlyphMetricsRasterizationState::RasterizeGlyph` (copies 7 ints) | 0x1ae00 | |
| rasterizer | `TrueTypeRasterizer::Implementation::NewTransform` | 0x8406c | 0x838c6 |
| | `FontRasterizer::ComputeInverseTransform` (inlined on x64) | in 0x8406c | 0xc88ab |
| | **`TrueTypeRasterizer::Implementation::GetMetrics`** (builds the result) | **0x71ce8** | **0x16c6a9** |
| | `...::GetBitmap(metricsOnly, GlyphBitmap*)` then `fs_FindBitMapSize` | 0x71fb8 / 0x726b0 | 0x88b23 / 0x892cb |
| | `...::GetDesignBounds(GlyphBitmap)` (black box, side bearings) | 0x7188c | 0x16c3c8 |
| | `...::TryGetCompatibleDisplayWidth` (advance width) | 0x82e04 | 0x16cee8 |
| helpers | `FontRasterizer::DisplayToEmX/DisplayToEmY(TransformF, int x, int y)` | 0xdda44 / 0xdac24 | 0x16ae05 / 0x16aec9 |
| | `Fixed16_16::FloatToFixedInt32` / `Fixed16_16::FromFloat` | 0x71504 | 0xa71b4 |
| | `DWRITE_FixDiv` | 0x827f0 | 0x871d6 |

The cached GlyphMetricsLayout is a plain copy of what `GetMetrics` produces (7 int32 already
in design units), so all the arithmetic is in `TrueTypeRasterizer::Implementation::GetMetrics`
and the fs (TrueType scaler) code under it. x86 and x64 use the same algorithm and the same
float op order. The products `upm * E` are 64-bit on both. I found **no arch difference**.

### Helper rules (exact)

- **FloatToFixed(f)** (FloatToFixedInt32 / FromFloat): `t = trunc_to_int64(f32(f32(f*65536)*65536))`,
  `r = (t + 0x8000) >> 16`, saturated to int32. For |f| < 2^7 this is `floor(f*65536 + 1/2)`
  on the exact float value f: halves go toward +inf, including for negative f.
- **FixedToFloat(v)** = `f32(v) * 2^-16` (0x7FFFFFFF -> +inf, 0x80000000 -> -inf).
- **FixDiv(a,b)** = `tdiv((a<<16) + s*tdiv(b,2), b)` with s = +1 when a and b have the same
  sign, else -1 (64-bit). This rounds half away from zero.
- **em16 -> design units**: `design = (upm * E + 0x8000) >> 16` (64-bit product, floor).
  `upm` is head.unitsPerEm.

### Transform (MakeRasterizerTransform, null matrix, GDI mode)

- `ppem = round_half_up(f32(emSize * pixelsPerDip))`: cvtss2si (RNE), plus 1 if the
  rounded value minus x is exactly -0.5. ppemX and ppemY are the same for one ppd.
- The Transform is four 16.16 values {a, b, c, d}, with `P = ppem << 16`:
  - not sideways: {a=P, b=0, c=0, d=P}
  - sideways: {a=0, b=-P, c=P, d=0}. Display x = -ppem * emY and display y = +ppem * emX:
    em "up" points to display -x.
- `ppemByte = ppem` if |ppem| <= 255, else 0. A byte of 0 turns off hdmx and the compatible
  width path (see the advance width section). This is a real **ppem cap at 256**.
- With a matrix there is a float path, and E_INVALIDARG (0x80070057) when a scaled entry
  is >= 32768 (0x47000000) or negative where it must not be. I did not analyse it further.
- The params also carry `pointSize = round_half_up(f32(emSize*0.75))` (used only by the fs
  for instructions/MPS), **overscale X = 6, Y = 1** (constant 0x10006, both modes, sideways
  too: always in *display* axes), and the rendering flags. GDI classic sets flag 0x10
  (plus 0x40 from a font property), GDI natural sets 0x20.
- **Shortcut**: when not sideways, a == d == upm<<16 (ppem == upm), b == c == 0 and the face
  has none of the face flag bits at FontFace+0x36 set (probably simulations; unverified), `FontFace::GetGlyphMetrics` returns `GetDesignGlyphMetrics` unchanged,
  with no pixel rounding.

### Inverse transform (NewTransform -> TransformF, 4 floats)

```
fa=FixedToFloat(a) fb=..(b) fc=..(c) fd=..(d)
det = f32(f32(fd*fa) - f32(fb*fc));  inv = f32(1/det)
TF = { m0 = f32(inv*fd), m1 = f32(-fb*inv), m2 = f32(-fc*inv), m3 = f32(inv*fa) }
```
So not sideways: m0 = m3 = `f32(f32(1/f32(p*p)) * p)` and m1 = m2 = 0. Sideways: m0 = m3 = 0,
m1 = `+f32(p*f32(1/(p*p)))`, m2 = `-f32(p*f32(1/(p*p)))`. **This is not f32(1/p)**: it is a
reciprocal of p squared, then multiplied by p.

### DisplayToEmY(TF, x, y) / DisplayToEmX(TF, x, y)  (x, y: 16.16 display pixels)

```
if TF.m2 == 0 and TF.m1 == 0:            # axis aligned (not sideways)
    yy = (y + 0x8000) & 0xFFFF0000       # whole pixel, half up (0x80000000 -> -inf)
    emY = f32(f32(yy)*2^-16) * TF.m3     (DisplayToEmX: x, TF.m0)
else:                                     # e.g. sideways: NOT rounded to the pixel
    emY = f32( f32(f32(x)*2^-16)*TF.m2 + f32(f32(y)*2^-16)*TF.m3 )
    emX = f32( f32(f32(x)*2^-16)*TF.m0 + f32(f32(y)*2^-16)*TF.m1 )
return FloatToFixed(emY)                  # em fraction, 16.16
```

### What the scaler hands back (fs_FindBitMapSize, x64 0x726b0)

Outline path, GDI mode with 6x1 overscale (the path where the rasterizer's "subpixel"
flag bit 0 is set and bit 0x40 is clear):
- The outline and its **phantom points** are in overscaled display space: x is 6 times
  the 26.6 value, y is 1 times. The phantom points (in TrueType order after the last
  contour point) are P1/P2 (horizontal origin / advance) and P3/P4 (vertical origin / vertical
  advance end); P5..P8 are copies.
- The black box is scanned in overscaled pixels. It is an int16 rect: top/left/bottom/right
  in os pixels. The scaler keeps it raw (`xMin_os, yMax_os`, ...) and also as a whole-pixel
  rect (left `floor(xMin_os/6)`, right `ceil(xMax_os/6)`; y with overscale 1).
- Device vectors, as 16.16 in *overscaled* units first (26.6 << 10):
  - devAdvanceWidth = P2 - P1
  - devAdvanceHeight = P3 - P4
  - devTopSideBearing = (xMin_os*64 - P3.x, yMax_os*64 - P3.y) << 10
  - and the lsb ones.
- **Scale-down**: then every component is divided back: `v = tdiv(v + ov/2, ov)` with
  ov = 6 for x and 1 for y. This is C division of (v + 3) by 6, so a **negative multiple of 6
  comes out one unit too large** (toward 0): tdiv(-6N + 3, 6) = -N + 1 (in 1/65536 px).

### GetMetrics (x64 0x71ce8): the seven fields

GetBitmap is called metrics-only when the transform is axis aligned or an exact 90 degree
rotation (always for GDI null matrix). The GlyphBitmap holds left, top, width, height and
flags. Flags bit 0 clear means the box is in overscaled units (the normal 6x1 case). Then:

```
# vertical-origin vector, display 16.16
if bitmap.flags & 1:  bx = xMin_px << 16;           by = yMax_px << 16
else:                 bx = tdiv(xMin_os<<16, 6);    by = tdiv(yMax_os<<16, 1)
vx = bx - devTSB.x ;  vy = by - devTSB.y
E_vorg = DisplayToEmY(TF, vx, vy)
E_advH = DisplayToEmY(TF, devAH.x, devAH.y)
E_advW = TryGetCompatibleDisplayWidth(...) or DisplayToEmX(TF, devAW.x, devAW.y)
B = GetDesignBounds(bitmap)       # {left, bottom, top, right} em16, see below
advanceWidth      = design(E_advW)
advanceHeight     = design(E_advH)
verticalOriginY   = design(E_vorg)
leftSideBearing   = design(B.left)
rightSideBearing  = design(E_advW - B.right)
topSideBearing    = design(E_vorg - B.top)
bottomSideBearing = design(B.bottom - E_vorg + E_advH)
```
(the differences are taken in em16, *before* the upm multiply, which matches "difference of
the edges' rounded fractions".)

### Sideways vertical origin: the full rule (explains the 97 ppem / 66 px case)

With A = 1024*P3.x the origin in display 16.16 (sideways P3.x = -64*k for an origin of k
whole pixels; see "unsure" for why it is whole pixels):
```
L      = xMin_os                       # os column of the glyph's top edge (sideways: display x = -emY*p)
tsb_os = (L*64 - 6*P3.x) * 1024        # devTSB.x before scale-down (P3 is 6x in os)
tsbx   = tdiv(tsb_os + 3, 6)           # scale-down
bx     = tdiv(L*65536, 6)              # GetMetrics' own division of the raw os box
vx     = bx - tsbx                     # = A + {-1, 0, +1}: the two truncations do not cancel
E_vorg = FloatToFixed( f32( f32(vx*2^-16) * TF.m2 ) ),  TF.m2 = -f32(p*f32(1/f32(p*p)))
verticalOriginY = (upm*E_vorg + 0x8000) >> 16
```
The +/-1 (in 1/65536 px) depends on `L mod 3` (65536/6 = 32768/3 leaves 0, 1/3 or 2/3), on the
sign of L, and on the sign of (L*64 - 6*P3.x), i.e. whether the glyph's top is above or
below the vertical origin. Case by case: `vx = A+1` when L < 0, the top is below the origin and
(-L) mod 3 == 1; `vx = A-1` when the top is above the origin and (-L) mod 3 != 1, or when
L >= 0 and L mod 3 == 1; else `vx = A`. Because the em fraction is k*65536/p with a tie class
remainder, one unit of vx flips the rounding.

**INFERRED, not read in the code**: the os left bound is `L = floor((x_os + 31)/64)` (the first
os column whose centre is on or right of the edge, as the closed-span rule) with
`x_os = 6 * x26`. Sideways `x26 = -t` (t = the glyph top in 26.6 as the raster scales it), so
`L = floor((31 - 6t)/64)`. With that, the rule above reproduces the measured
"97 ppem, 66 px: down iff 0 <= t < 64*66 and t mod 32 in 6..15, up elsewhere" **exactly**.
I checked every t from -3000 to 64*66+3000 in a simulation with f32 arithmetic.

So the 29 "origin one more" ppems are not a property of the ppem. They come from this +/-1,
which depends on where the test glyph's top falls (L mod 3, above or below the origin) at that
ppem, combined with the f32 scale. The implementer can check this by running the formula on
the gdivtie fonts.

### Sideways vertical advance (explains h = p/2 - 1 and the k-dependent classes)

devAH.x = 6*(P3.x - P4.x)*1024 = -6*65536*k (k pixels), so the scale-down gives
`vx = tdiv(-6*65536*k + 3, 6) = -65536*k + 1` **always**. That is the systematic one-unit bias
that makes the advance's tie round down (h = p/2 - 1):
```
E_advH = FloatToFixed( f32( f32((1 - 65536*k)*2^-16) * TF.m2 ) )
advanceHeight = (upm*E_advH + 0x8000) >> 16
```
Simulated over p = 1..160 and k <= p: it reproduces h = p/2 - 1, the "one less" at 10, 14
and 22, the "one more" at 109, and the k-dependent class at 66 ppem / 2r-p=2 (k=16 down,
k=49.. up). For k up to 8p it gives 97/1 (least k=66 down, others up) and 62/-2 (up from
k=263; horizontal mode up from k=139). These match the measured descriptions "least k
sideways the other way, larger k up at 62, advance up at 97". For k > p the f32 product
starts to matter at many more ppems (31, 45, 51, ...).

### Not sideways (axis-aligned path)

Here P3, P4 and the box y are not overscaled (ovY = 1) and DisplayToEmY rounds y to the whole
pixel first, so there is no +/-1:
```
E = FloatToFixed( f32( f32(k) * m ) ),   m = f32(f32(1/f32(p*p)) * p)     # k whole pixels
```
k for the vertical origin is round_half_up(P3.y/64) and P3.y is itself a 26.6 rounded value.
That is the measured `floor(x + 65/128)` (double rounding through 26.6). For the advance
height, k is from (P3.y - P4.y). This equals `round(k*65536/p)` for small k, but f32 error
flips some ties at large k (predictions below).

### Side bearings: GetDesignBounds (x64 0x7188c)

Metrics-only, so it uses the 4 corners of the box rect, not the bits. With bitmap flags bit 0
clear: `sx = f32(1/6)`, `sy = f32(-1/1)`; with bit 0 set: sx = 1, sy = -1. The corners
`(left*sx, (-top)*sy)`, `((left+w)*sx, ...)`, `(..., (h-top)*sy)` (ints converted to f32) each go
through `GlyphBoundingBox::AddPoint` (0x71c20), which applies TF in float (general form
`x*m0 + y*m1`, `x*m2 + y*m3`) and keeps the float min/max. Then each of minX, minY, maxY, maxX
goes through FloatToFixed: B.left, B.bottom, B.top, B.right. So each edge is
`round16(f32(f32(edge_os * f32(1/6)) * m))`. That is the "6 x 1 samples" (sideways "1 x 6" in
em axes) black box, with the edges of the os pixel rect.

### Advance width (TryGetCompatibleDisplayWidth, x64 0x82e04)

It is used only if `ShouldCheckHdmx` (0x84674) holds: ppemByte != 0 (ppem <= 255), flags & 0x30
set, (flags & 0x60) != 0x20 (so **not GDI natural**), b == 0 and c == 0 (so **not sideways**).
```
if hdmx has a record for ppemByte (and the device-record checks pass):
    w = hdmx_width << 16
else:
    w = tdiv(|a|, upm) * hmtx_advance         # |a| = ppem<<16: floor(ppem*65536/upm), the 16.16 truncated scale
    if w != 0: w += (flags & 2) << 15         # +1 px when flag bit 1 (bold simulation, I think)
w_px = (w + 0x8000) & 0xFFFF0000              # round half up to the pixel
E_advW = FixDiv(w_px, |a|)                    # = floor((k*65536 + p/2)/p): exact rounding, no float
```
Otherwise (natural, sideways, ppem >= 256) `DisplayToEmX(devAdvanceWidth)` is used.
Not sideways: whole pixel then `* m`. Sideways: `f32(dy*2^-16) * m1` with no pixel rounding, but
P1/P2 are whole pixels, which gives the measured sideways `floor(x + 65/128)`.

### Concrete predictions (all from the formulas above, f32 simulated)

1. Sideways, ppem 97, vertical origin 66 px, glyph top t (26.6): t = 6 or 15 or 1930 ->
   E_vorg = 44591 (upm 2048: verticalOriginY 1393). t = 5, 16, -6 or 4230 -> 44592 (1394).
2. Sideways vertical advance: 66 ppem k=16 -> E 15887 (upm 2048: 496; exact rounding would
   give 497). k=49 -> 48656 (1521). 97 ppem k=66 -> 44591 (1393). 62 ppem k=263 -> 278000
   (8688; exact 8687), k=232 -> 245231 (7663).
3. Not sideways (vertical origin or advance height of k px), f32 flips at large k:
   31 ppem k=85 -> E 179696 (upm 2048: 5616; exact 5615). 62 ppem k=139 -> 146928 (4592;
   exact 4591). 62 ppem k=108 -> 114159 (3567, as exact).
4. em*ppd rounding to ppem == upm, not sideways, no simulation: every field equals
   GetDesignGlyphMetrics (e.g. upm 2048, em 2048, ppd 1).
5. ppem >= 256 (e.g. em 300): the GDI classic advance no longer follows
   floor(ppem*65536/upm)*adv. It is then the phantom advance in whole pixels times f32 m.
   For an unhinted font I expect round(adv*ppem/upm) via 26.6 double rounding, but I have not
   verified this.

### Where I am unsure (Q1)

- P3 (and P1, P2, P4) being **whole pixels** in GDI mode is inferred from the measured double
  rounding and from the 97/66 rule needing A = -66*65536 exactly. The rounding step is
  presumably in the scaler's phantom-point handling (`scl_RoundCurrentSideBearingPnt`
  0x8c8d0, `scl_AdjustCompatibleMetrics` 0xd6e30); I did not read it.
- `x_os = 6*x26` (the overscale applied to rounded 26.6) and the os bound rule `+31` are
  inferred. They fit every measured t; the source is `fsc_MeasureGlyph` (0x74050) and the
  subpixel scaling, which I did not read.
- The bitmap flags bit 0 (overscaled box or not) comes from `RasterizeInternal` (0x7239c);
  I assumed it is clear for GDI 6x1, which the fit supports. The FBS branch where bit 0x40 is
  set skips the vector scale-down (vectors would stay in os units); I have not seen when that
  happens.
- Embedded bitmaps (sbit) use another path.

---------------------------------------------------------------------------------------------

## Q2. GetGdiCompatibleGlyphPlacements (x64 0x47900, x86 0x149fe0): partial

The text analyzer hands positioning to the external shaping engine
(`ShapingGetGlyphPositions`, imported) through `ShapingInterface` (ctor 0x42f20, which takes
the measuring mode, em, ppd and matrix). I did not trace its advance callback. If it reads
the font face's GDI metrics, then:
- **non-natural (GDI classic)**: `w = floor(ppem*65536/upm) * adv` (integer division of
  ppem<<16 by upm), `px = (w + 0x8000) >> 16` (half up), or the hdmx width when present for
  ppem <= 255. This **confirms DW.SHAPE.GDIPLACE**, plus the hdmx override and the ppem <= 255
  condition.
- **natural**: TryGetCompatibleDisplayWidth is skipped (flags 0x20), so the advance is the
  devAdvanceWidth phantom vector, x scaled down by `tdiv(v+3, 6)` and rounded to the pixel.
  The measured threshold (frac >= 59/128 = 944/2048 rounds up) is the same as
  `round(64x) >= 30`. I have **not** found where the extra 2/64 comes from. The candidate is
  how the scaler places P2 in natural mode (subpixel 6x advance rounding in
  `scl_ScaleAdvanceWidth` 0x7dfdc / hinting). Not resolved.

## Q3. Design to 26.6 for rasterization: not done

Not analysed in this pass. Where to look: `scl_ScaleOldCharPoints` (0xc817c),
`scl_ScaleFixedCurrentCharPoints` (0x81178), `mth_FixXYMul` (0x7d81c), `mth_IntelMul` (0x82614),
`scl_InitializeScaling` (0x85ab8, which picks a power-of-2 shift fast path versus a general
multiply/divide; this fits "different below zero when upm is not a power of 2").

## What remains

1. Confirm the inferred pieces in `fsc_MeasureGlyph` and the phantom rounding (see Q1 unsure).
2. The GDI natural advance (Q2) and the shaping callback.
3. Q3 scaling arithmetic.

---------------------------------------------------------------------------------------------

## Follow-up A. The three inferred pieces (now read in the code)

All x64 RVAs; x86 has the same functions (see the symbol table above for the pattern).

1. **x_os = 6 * x26: CONFIRMED** (in the scaler's glyph routine after grid fitting, around
   x64 0x87f82..0x8808e, inside the code that follows `fsg_GridFit`). When the scaler's
   subpixel flag (bit 0 of its flag word) is set, it multiplies **every point, including all 8
   phantom points** (indices up to lastPoint + 9), as `x = FixMul(x, ovX<<16)` and
   `y = FixMul(y, ovY<<16)` with ovX = 6, ovY = 1. FixMul is defined below. With integer
   factors this is exactly 6*x and 1*y. This happens after hinting and phantom rounding, so
   the rounding happens at 1x (the os scaling comes after). In the GDI classic
   "compatible width" case (flag bit 1), grid fitting runs again first, then the same multiply.
2. **Phantom points whole pixels: CONFIRMED, with one exception**. This is
   `scl_RoundCurrentSideBearingPnt` (0x8c8d0), called after `scl_ScaleOldPhantomPoints` /
   `scl_AdjustOldSideBearingPoints` / `scl_CopyCurrentPhantomPoints`, before the glyph program.
   - Horizontal advance: `a16 = rha(adv_design * xScale16 / upm)` (64-bit; rha = round half away
     from zero, using a sign-aware +upm/2), `a26 = (a16 + 0x200) >> 10`.
     - If (flags & 5) == 1: `a26q = (a26 + 2) & ~3` (**1/16 pixel**).
     - Else: `a26q = (a26 + 32) & ~63` (whole pixel).
     - Then `P2.x = P1.x + a26q`.
   - Vertical: `P3.y = (P3.y + 32) & ~63` (whole pixel). `h16 = rha((ooy[P4]-ooy[P3]) * yScale16 / upm)`
     (negative), `P4.y = (P3.y + 32 + ((h16 + 0x200) >> 10)) & ~63`.
   - Flag bit 2 (value 4) is set by `fs_NewTransformation` (around 0x854b8) exactly when the
     caller asks for subpixel and **matrix m00 == 0 (the sideways 90 degree case)**. Bit 1 (2,
     compatible widths) is set only when m01 == 0. So: **not sideways (classic and natural):
     the advance phantom rounds to 1/16 px. Sideways: whole pixels.** (Bit 0 is always set in
     both GDI modes, unless the face property that sets rendering flag 0x40 is present. Then
     there is no subpixel at all.)
3. **os bounds rule: CONFIRMED in form** in `fsc_MeasureGlyph` (0x74050). Each extremum (a
   "reversal") becomes a scan-line index: a minimum v (coming in rising) uses `(v + 31) >> 6`,
   a maximum uses `(v + 32) >> 6`, in os 26.6. So the left/bottom bound is
   `floor((x_os + 31)/64)`, as inferred. MeasureGlyph does no scaling itself; it receives the
   already 6x outline.

## Follow-up B (Q3). Design units to 26.6: the scaler's scale record

Built by `scl_InitializeScaling` (0x85ab8), applied by `scl_ScaleOldCharPoints` (0xc817c) and
the phantom/twilight variants. Per axis (x record and y record):
```
N = axis scale, 16.16 px per em     # GDI: ppem<<16 (rounded to a whole px when the round flag is set);
                                    # "hint at em square": upm<<16
D = upm << 16
t = ctz(N | D);  if t > 1: N >>= (t-1); D >>= (t-1)    # keeps exactly one common factor of 2
if N < 0x2000000: numer = N*64           else: numer = N; D >>= 6
fixed = sat32( FixDivRound(numer<<16, D) )    # (numer<<16 +- D/2)/D, half away from zero
if numer > 0x7FFF:            x26 = FixMul(v, fixed)                     # half away from 0
elif D is a power of two:     x26 = (v*numer + D/2) >> log2(D)           # 32-bit, floor: half UP, also below 0
else:                         x26 = v>=0 ? (v*numer + D/2) / D           # C division
                                         : -((D/2 - v*numer) / D)        # half AWAY from zero
```
Here `FixMul(a,b) = sat32((a*b + 0x8000 - (a*b < 0)) >> 16)`, which rounds half away from 0.
After the reduction, `numer = 128*ppem / 2^g` and `D = 2*upm / 2^g`, with
g = min(ctz(ppem), ctz(upm)). D is a power of two **exactly when upm is a power of two**. That
is your measured "half up, but away from zero below zero when upm is not a power of 2".
**New limit:** when `ppem / 2^g > 255` (numer > 32767) the fixed path is used instead, at
any upm.

Predictions:
- upm 512, ppem 4, v = -1 (exact -0.5 in 26.6): 0 (shift path, half up).
- upm 768, ppem 3, v = -2 (exact -0.5 in 26.6): -1 (divide path, away from zero).
- **upm 2048, ppem 257, v = -16** (exact -128.5): fixed = 526336, so -129 (half away from zero).
  The shift path would give -128. For v = +16 both give 129.

The GDI classic / DISABLED-fit scale you measured (`s16 = ((em<<17)/upm + 1) >> 1`, then
`(v*s16 + 0x200) >> 10`) is **not** this record. I did not locate it in this pass. It looks like
`FixDivRound(em<<16, upm<<16)` followed by a multiply in 26.6. If it uses `FixMul(v*64, s16)`,
negative ties would round away from zero instead of half up: check `v*s16 ≡ -0x200 (mod 0x400)`
cases.

## Follow-up C (Q2). The GDI natural advance, and the advance past 255 ppem

- **Natural (useGdiNatural, not sideways)**: no compatible width (ShouldCheckHdmx rejects
  flags 0x20). The advance phantom is rounded to **1/16 px** (Follow-up A.2). Then x6 in os,
  scaled back by `tdiv(v+3,6)` (exact for positive multiples of 6), then DisplayToEmX rounds to
  the whole pixel:
  ```
  a26 = (rha(adv*(ppem<<16)/upm) + 0x200) >> 10
  q   = (a26 + 2) & ~3
  k   = (q + 32) >> 6            # rounds up iff round(64x) mod 64 >= 30  <=>  frac >= 59/128 (via 26.6)
  E   = FloatToFixed(f32(f32(k) * m0))      # m0 = f32(f32(1/(p*p))*p); advance = (upm*E+0x8000)>>16
  ```
  This is where 69/128 comes from: 30/64 after the 26.6 rounding equals floor(x + 69/128) in
  terms of x. In placements, the pixel k is the advance.
- **Classic past 255 ppem** (ppemByte = 0, so no hdmx and no linear compatible width): the
  same phantom path with the same 1/16 rounding (not sideways), so **the same formula as
  natural**. Below 256 ppem, classic keeps `floor(ppem*65536/upm)*adv` rounded and FixDiv
  (Q1). Sideways uses whole-pixel phantom rounding at every ppem.
- **Unsure**: `rha(adv*xScale16/upm)`. The divisor is the G argument of
  `scl_RoundCurrentSideBearingPnt` (passed by its caller), and I take it to be upm (it fits
  the measured natural threshold exactly). The 0x40 face property (no subpixel) path is untested.

## Not done in this pass (superseded: see Follow-ups D and E below)

- Q4 (IDWriteGlyphRunAnalysis sample positions, coverage, dropout).
- Locating the DWrite-side Fixed16 scale for the DISABLED grid fit.

## Follow-up D (Q4). IDWriteGlyphRunAnalysis: sample grid, coverage, 3x1 filter, bounds

### D.1 Per-mode attribute word (from GetRenderingModeAttributes)

There is one 32-bit word for each DWRITE_RENDERING_MODE value (index 0..9). One table is for
ClearType analyses and one is for grayscale (antialias-mode grayscale) analyses. The bit fields
below were confirmed from GetAlphaTextureBounds and CreateAlphaTexture3x1:

| bits | meaning |
|---|---|
| 0-3 | ovX (horizontal samples per pixel) |
| 4-7 | ovY (vertical samples per pixel) |
| 8-11 | log2(ovX) when ovX is a power of two (shift path), else 0 (divide path) |
| 12-15 | log2(ovY), same rule |
| 16 | texture type produced: 0 = ALIASED_1x1, 1 = CLEARTYPE_3x1 |
| 24-25 | filter margin in samples, added on each side in x |
| others | not identified |

ClearType table, mode -> (ovX x ovY, texture type, margin):
- ALIASED: 1x1, aliased, 0
- GDI_CLASSIC, GDI_NATURAL, NATURAL: 6x1, 3x1, 2
- NATURAL_SYMMETRIC, OUTLINE-slot and the symmetric modes: 6x5, 3x1, 2

Grayscale table:
- classic and natural: 8x1, aliased texture, 0, shift 3
- symmetric: 4x4, aliased texture, 0, shift 2/2

A grayscale analysis therefore answers only the ALIASED_1x1 texture type. GetAlphaTextureBounds
for the other type returns an empty rect with S_OK. A type other than 0 or 1 returns
E_INVALIDARG and an empty rect.

### D.2 Glyph placement (sample units)

This is in addition to round 3 (ComputeGlyphBitmapPositions). Each glyph origin is
`round_half_up(f32(f32(ov*scale) * coord))` in samples: x uses ovX, y uses ovY, and in the
run-level variant y is taken once from the first glyph. The cached glyph bitmap carries its
own left and top in samples. The glyph is placed at `left = sx + bm.left` and
`top = sy - bm.top`. There is no extra 1/4 px or 1/64 px pen quantisation in this step; any
such quantisation is already in the origins handed in. I did not trace the advance snapping
of the GDI modes in ComputeGlyphOrigins this round.

The outline is scaled by the scaler record at the real ppem (Follow-up B). The overscale
(×ovX, ×ovY) is applied after hinting, as found before. Disabled grid fit is Follow-up E.

### D.3 Coverage is binary per sample

- Each glyph is rasterised by the TrueType scan converter into a **1-bit** mask at the sample
  grid (one bit per sample).
- The run is composed by **OR**ing the glyph masks into one 1-bit buffer, in 32-bit words with
  the low bit leftmost. The function names are MergeUncompressedGlyph1Bit and
  MergeCompressedGlyph1Bit. Overlaps saturate, so there is no accumulation.
- In ALIASED mode (1x1) a set bit gives an output byte of 255 and a clear bit gives 0.
- In the 3x1 mode, samples x in [6*L - 2, 6*R + 2) are composed for the pixel rect
  [L, R). The 2-sample margin feeds the filter.

The scan-conversion rule, as far as traced (fsc_CalcLine):
- Sample centres are at 64k + 32 in 26.6 (pixel centres of the sample grid).
- For an edge from c1 to c2 with c1 < c2 along the stepping axis, the centres it crosses are
  those with **c1 < centre < c2**: first index `(c1 + 32) >> 6`, last index `(c2 - 33) >> 6`.
- The descending case mirrors this.
- Horizontal edges and edges that end exactly on a centre take separate branches. I have not
  resolved those branches, so treat vertices that land exactly on a sample centre as
  unconfirmed.
- I did **not** trace dropout control (SCANCTRL / SCANTYPE handling in fsc_FillBitMap) this
  round.

### D.4 CLEARTYPE_3x1 values (CreateAlphaTexture3x1)

The alpha texture always uses **g_classicPlattFilterRGB**. It does not depend on rendering
params, pixel geometry or ClearType level; those apply only to the D2D/GDI render paths
(D.5).

Notation:
- `b[s]` is the 1-bit sample at sample column s.
- For output pixel X, `s0 = 6*X`.
- `n(a, k)` is the number of set bits among `b[a .. a+k-1]` (k consecutive bits).

**6x1 modes** (ApplyFilterImpl with no vertical taps):
```
nR = n(s0-2, 6)   nG = n(s0, 6)   nB = n(s0+2, 6)          # each 0..6
out[3X+0] = R = T1[nR], out[3X+1] = G = T1[nG], out[3X+2] = B = T1[nB]
T1[n] = (255*n + 3) / 6  (integer)  = {0,43,85,128,170,213,255}
```
Each channel is therefore a 6-sample box (two subpixels wide on each side) centred on its own
subpixel: R on samples 0-1, G on 2-3 and B on 4-5. The bytes are stored R, G, B in that order.

**6x5 modes** (symmetric): there are 5 sample rows per pixel row, and each channel's per-row
counts are combined with vertical weights:
```
idx = 4*n_row0 + 9*n_row1 + 10*n_row2 + 9*n_row3 + 4*n_row4       # 0..216
value = T5[idx],  T5[i] = (255*i + 108) / 216  (integer, round half up)
```

The tables in the binary are linear: I checked every entry against the per-bit weights. The
dll stores them as 10-bit-index LUTs of packed 5:5:5 channel counts, which is only an
implementation detail.

### D.5 The other ClearType filters (render paths, not the alpha texture)

GetFilterInternal(pixelGeometry, clearTypeLevel) selects:
- FLAT geometry, or level < 0.25: grayscale. Each channel is n(s0, 6), so R = G = B.
- 0.25 <= level <= 0.75: "intermediate".
  - RGB: R = n(s0-1, 6), G = n(s0, 6), B = n(s0+1, 6).
  - BGR mirrors this.
  - The margin is 1 sample.
- level > 0.75: "classic". RGB is as in D.4; BGR swaps the R and B windows.

All of these normalise through the same T1 / T5 tables.

### D.6 GetAlphaTextureBounds

The input is the sample-space union rect from round 3 (empty bitmaps are skipped).
1. If the texture type matches and the rect is non-empty, apply the margin:
   `left -= m; right += m` (m = 2 for the ClearType modes, 0 otherwise).
2. Convert to pixels, flooring left and top and ceiling right and bottom:
   - x: `left = floor(left/ovX)`, `right = ceil(right/ovX)`. The shift path gives identical
     results.
   - y: the same with ovY, only when ovY > 1.
3. The result is unioned with a second rect stored in the analysis object, used only when both
   rects are non-empty. If the first is empty, the second is returned as is. I did not
   identify the second rect; it is presumably empty for plain runs.

For ClearType the result is `[floor((sl-2)/6), ceil((sr+2)/6))`, which matches the column range
CreateAlphaTexture3x1 fills.

## Follow-up E. Fixed16 scale for the disabled grid fit

- **DWRITE_FixMul rounding (confirmed):** `(p + 0x8000 - (p < 0)) >> 16`, arithmetic shift,
  saturated to int32. This is round half up **except negative exact ties, which round down
  (away from zero)**. It agrees with floor((p + 0x8000)/65536) everywhere except
  p ≡ -0x8000 (mod 0x10000) with p < 0.
- **Transform hand-off (NewTransform):**
  - The rasterizer receives an integer size (`size<<16`) plus a 16.16 matrix, with resolution
    96/96.
  - Render flag 0x4 adds the synthetic oblique: m01 += FixMul(m00, 0x5700) and
    m11 += FixMul(m10, 0x5700), or the sideways variant m00 -= FixMul(m01, ..), and so on.
  - The fractional part of the em size travels in the matrix, which is FloatToFixed per entry
    (round 3).
  - Outline points are then transformed with mth_FixXYMul: a sum of separately rounded
    FixMuls (round 3).
- **I have not yet tied the measured s16 to a specific instruction.** The structure is
  consistent with s16 = FixDiv(em<<16, upm<<16), which rounds half away from zero and
  therefore equals `((em<<17)/upm + 1) >> 1` for positive values. That would be followed by
  x26 = FixMul(64*v, s16). For v >= 0 this is exactly the measured `(v*s16 + 0x200) >> 10`.
- **Prediction to test on Windows:** for v < 0 with `|v|*s16 ≡ 0x200 (mod 0x400)`, Windows
  gives `-((|v|*s16 + 0x200) >> 10)`. That is one less than the half-up formula's
  `(v*s16 + 0x200) >> 10`. Every other negative v agrees with half-up.
  - Example: upm 2048, em 16 gives s16 = 512, and v = -1 is a tie. Half-up gives 0;
    FixMul gives -1.
  - If Windows shows half-up there instead, the multiply is not DWRITE_FixMul (a float or
    floor path).

## Still open after round 4

- Dropout control and the exact on-centre vertex rule in the scan converter.
- Advance and pen snapping for the GDI modes inside the GlyphRunAnalysis constructor and
  ComputeGlyphOrigins.
- The instruction that produces s16 for the disabled grid fit (E is consistent but unconfirmed).
- The second rect in GetAlphaTextureBounds.
