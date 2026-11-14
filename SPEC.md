# kikizake case language and output

A **case file** (`cases/<suite>.txt`) is plain text, one call per line, tokens separated by
spaces. `# ` starts a comment (and a line starting with `#` is one); `#RRGGBB` is a color.
Numbers are decimal (`-3`), or hex with `0x`; never write a leading zero (C's `strtoul` reads
it as octal). A case runs from `case <name>` to `end`; everything it made is released at `end`,
so cases are independent and their order does not matter.

The executor prints `kikizake <suite>`, then for each case `case <name>` and the lines its calls
print, then `done`. A line it cannot run prints `bad line <n>` (n counts every line of the
file from 1). Output is flushed at the end of each case: a crash keeps the finished cases and
has no `done`.

## GDI (DIB sections)

Eight slots, 0..7; each holds a DIB section selected into a memory DC of its own, with that
DC's state (a memory DC starts with black text color, white background color, OPAQUE, the
white stock brush, brush origin 0,0, no clip region).

| line | calls | prints |
|---|---|---|
| `dib <s> <w> <h> <bpp> [td\|bu] [565] [#c0 #c1 ...]` | CreateDIBSection (bpp 1, 4, 8, 16, 24, 32; top-down by default; `565` = BI_BITFIELDS 5-6-5 for 16 bpp, else 5-5-5) with the color table given or the default one, and the pixels zeroed | `dib failed <error>` on failure |
| `fill <s> <seed>` | every byte of the pixel memory, in memory order, from xorshift32 (below) | |
| `poke <s> <x> <y> <hex>` | raw bytes of one pixel (bpp >= 8) | |
| `dump <s>` | GdiFlush, then the picture | `px <s> <w> <h> <bpp>`, then each row top to bottom as hex bytes, only the bytes that hold the row's w pixels |
| `brush <s> solid #c` / `hatch <style> #c` / `dib <slot>` / `null` | CreateSolidBrush / CreateHatchBrush / CreateDIBPatternBrushPt of a packed BI_RGB copy of a slot's picture (DIB_RGB_COLORS) / the NULL_BRUSH stock object, selected | `brush failed` |
| `text <s> #c`, `bk <s> #c`, `bkmode <s> <1\|2>`, `org <s> <x> <y>` | SetTextColor, SetBkColor, SetBkMode, SetBrushOrgEx | |
| `clip <s> <x0> <y0> <x1> <y1>`, `noclip <s>` | IntersectClipRect, SelectClipRgn(NULL) | `clip <region type>` |
| `patblt <s> <x> <y> <w> <h> <rop>` | PatBlt | `ret <0\|1>` |
| `bitblt <d> <x> <y> <w> <h> <s> <sx> <sy> <rop>` | BitBlt | `ret <0\|1>` |
| `stretchblt <d> <x> <y> <w> <h> <s> <sx> <sy> <sw> <sh> <rop>`, `mode <s> <m>` | StretchBlt, SetStretchBltMode (a memory DC starts with BLACKONWHITE) | `ret <0\|1>` |
| `getpixel <s> <x> <y>`, `setpixel <s> <x> <y> #c` | GetPixel, SetPixel | `pixel RRGGBB` or `pixel invalid` (CLR_INVALID) |
| `getdibits <s> <bpp> [565]` | GetDIBits of the slot's bitmap (deselected meanwhile) into a top-down DIB of that format with the default color table | `dibits <lines>`, for bpp <= 8 `pal` and the color table GetDIBits wrote, then the rows as `dump` prints them |
| `setdibits <s> <bpp> [565] <seed>` | SetDIBits from a top-down DIB of that format (default color table), its bytes from xorshift32 | `setdibits <lines>` |

Default color tables: 1 bpp black, white; 4 bpp the 16 VGA colors (000000 800000 008000
808000 000080 800080 008080 c0c0c0 808080 ff0000 00ff00 ffff00 0000ff ff00ff 00ffff ffffff);
8 bpp entry i = (r, g, b) = ((i>>5&7)*255/7, (i>>2&7)*255/7, (i&3)*255/3), integer division.

xorshift32: state = seed (0 becomes 0x9e3779b9); per byte `x ^= x << 13; x ^= x >> 17;
x ^= x << 5`, the byte is the low 8 bits of x.

## Direct3D 9

| line | calls | prints |
|---|---|---|
| `device [hwvp\|swvp\|mixed]` | Direct3DCreate9 (d3d9.dll loaded when first needed), CreateDevice HAL, windowed, 64x64 X8R8G8B8, auto depth stencil D24S8 (HAL software vertex processing if the one asked is refused) | `device ok`, `device none` (no Direct3D 9), `device failed <hr>` |
| `rs get <state>`, `tss get <stage> <type>`, `ss get <sampler> <type>` | Get{Render,TextureStage,Sampler}State | `rs <state> = <value>` (8 hex digits), or `... hr <hr>` |
| `rs set <state> <v>`, `tss set <stage> <type> <v>`, `ss set <sampler> <type> <v>` | Set...State | `... set hr <hr>` only on failure |
| `sb begin`, `sb end <i>`, `sb create <i> <type>`, `sb capture <i>`, `sb apply <i>`, `sb release <i>` | BeginStateBlock, EndStateBlock into block i (0..7), CreateStateBlock, Capture, Apply, Release | `sb <verb> hr <hr>` on failure; `sb <i> none` when block i does not exist |

`device8 [hwvp|swvp|mixed]` makes a Direct3D 8 device the same way (`device8 ok` / `none` / `failed <hr>`); while it is the current device, `rs` and `tss` (stage state types 0..29: Direct3D 8's sampler states are stage states) and `sb` act on it (state blocks are DWORD tokens: Create / End / Capture / Apply / DeleteStateBlock), and `ss` is a bad line.

Without a device the state lines print `nodev`. (Not asked by the generators: stages past 7,
samplers past 15 other than 256..260, sampler state types past 13: Windows reads past its arrays.)

## Direct3D 11

| line | calls | prints |
|---|---|---|
| `device11` | D3D11CreateDevice (d3d11.dll loaded when first needed), hardware (WARP if refused), feature level 11_0 only | `device11 ok`, `device11 none`, `device11 failed <hr>` |
| `blend\|depth\|raster\|sampler [check\|nodesc] <words>` | Create{Blend,DepthStencil,Rasterizer,Sampler}State of the description given as its 32-bit words (66, 13, 10, 13; missing trailing words 0; UINT8 fields with their padding in one word), then GetDesc; `check`: ppState NULL; `nodesc`: pDesc NULL and no words | `<kind> hr <hr>` on failure and for `check` / `nodesc`, else `<kind> = <GetDesc's words> same <n\|->` (hex, the UINT8 fields without padding; n: the case's earlier state line, counted from 0, that gave the same object) |

Without a device these lines print `nodev`. A case holds at most 64 state lines.

## Direct3D 10 and 11 resources

The lines act on the case's device: `device11 [<feature level>]` (above; feature level as a
D3D_FEATURE_LEVEL number, 11_0 by default), `device10` (D3D10CreateDevice, d3d10.dll) or
`device10_1 <feature level>` (D3D10CreateDevice1, d3d10_1.dll); hardware, WARP if refused
(`device10 ok` / `none` / `failed <hr>`). Sixteen resource slots and sixteen view slots.

| line | calls | prints |
|---|---|---|
| `fmt <format>` | CheckFormatSupport, CheckFeatureSupport FORMAT_SUPPORT2 (11), CheckMultisampleQualityLevels of 2, 4, 8, 16 samples | `fmt <format> <hr> <support> <hr2> <support2> ms <q2> <q4> <q8> <q16>` (hex; 10: `- -` for the second pair; a failed quality query `x<hr>`) |
| `tex <slot\|check> <dim 1\|2\|3> <w> <h> <depth or array> <mips> <format> <usage> <bind> <cpu> <misc> <samples> <quality> [init <seed> <row pitch> <slice pitch>]` | CreateTexture1D / 2D / 3D (h unused for 1D, samples for 1D and 3D; `check`: ppTexture NULL), every subresource's initial data the same 4 MiB of xorshift32 bytes from the seed with the pitches given, then GetDesc | `tex hr <hr>`, else `tex = <the description's words>` (hex: 1D 8 words, 2D 11, 3D 9) |
| `buf <slot\|check> <bytes> <usage> <bind> <cpu> <misc> <stride> [init <seed>]` | CreateBuffer (10: no stride), GetDesc | `buf hr <hr>` / `buf = <6 words (10: 5)>` |
| `srv\|rtv\|dsv\|uav <view> <res> [check] nodesc\|<words>` | Create...View of the resource with no description or the one given as words (Format, ViewDimension, [Flags: dsv on 11], the union), GetDesc (10.1 devices: CreateShaderResourceView1); no uav on 10 | `<kind> hr <hr>`, `<kind> = <Format ViewDimension [Flags] and the union words the dimension uses>`, `<kind> nores`, `uav none` |
| `clear <rtv> <r> <g> <b> <a>`, `cleards <dsv> <flags> <depth> <stencil>`, `clearuav <uav> f\|u <4 words>` | ClearRenderTargetView, ClearDepthStencilView, ClearUnorderedAccessViewFloat / Uint (11); floats as their bits | `<op> noview` when the slot holds no view of that kind |
| `update <res> <sub> <seed> <row pitch> <depth pitch> [<left top front right bottom back>]` | UpdateSubresource from xorshift32 bytes | |
| `copy <dst> <src>`, `copysub <dst> <dsub> <x> <y> <z> <src> <ssub> [<box>]`, `resolve <dst> <dsub> <src> <ssub> <format>` | CopyResource, CopySubresourceRegion, ResolveSubresource | `<op> nores` |
| `read <res> <sub> <row bytes> <rows> <slices>` | the subresource copied into a staging resource of its size (CopySubresourceRegion), mapped | `read <rows> <slices>` and each row's first bytes as hex, `read hr <hr>` |

Without a device these lines print `nodev`.

## WIC

| line | calls | prints |
|---|---|---|
| `convert <from> <w> <h> <seed\|ramp> <to>` | CreateBitmapFromMemory of format `from` (w 1..256, h 1..64, rows of (w * bits + 7) / 8 bytes from xorshift32, or byte i = i mod 256 for `ramp`; indexed formats get the palette below), CreateFormatConverter, CanConvert, Initialize to `to` (WICBitmapDitherTypeNone; the palette for indexed targets), CopyPixels of the whole picture | `conv <w> <h> can <0\|1>` and each row's bytes as hex (the bits after the row's last pixel 0); `conv can <0\|1> init hr <hr>` / `copy hr <hr>`, `conv bitmap hr <hr>`, `conv none` (no WIC) |

Formats: `bw gray2 gray4 gray8 gray16 bgr555 bgr565 bgr24 rgb24 bgr32 bgra pbgra rgb32 rgba prgba rgb48
rgba64 cmyk alpha8 idx1 idx2 idx4 idx8` (GUID_WICPixelFormat...). The palette: 16 colors, entry i =
ARGB (i == 5 ? 0x40 : 0xff, i * 17, 255 - i * 13, i * 71 mod 256).

## DirectDraw

Eight surface slots, separate from the GDI ones.

| line | calls | prints |
|---|---|---|
| `dd` | DirectDrawCreateEx (IDirectDraw7; ddraw.dll loaded when first needed), SetCooperativeLevel DDSCL_NORMAL | `dd ok`, `dd none`, `dd failed <hr>` |
| `ddsurf <s> <w> <h> <8\|555\|565\|32>` | CreateSurface, DDSCAPS_OFFSCREENPLAIN \| SYSTEMMEMORY, that RGB format (8: DDPF_PALETTEINDEXED8, no palette) | `ddsurf failed <hr>` |
| `ddfill <s> <seed>`, `dddump <s>` | Lock; each row's pixel bytes from xorshift32 / printed | `dd <s> <w> <h> <bpp>` and the rows as hex |
| `ddkey <s> src\|dest <v>\|none` | SetColorKey DDCKEY_SRCBLT / DESTBLT, low = high = v (none: NULL) | `ddkey hr <hr>` on failure |
| `ddblt <d> <rect> <s\|-> <rect> [keysrc] [keydest] [keysrcov <v>] [keydestov <v>] [mirrorlr] [mirrorud]` | Blt (DDBLT_WAIT and the flags; the overrides and mirrors through DDBLTFX) | `ret <hr>` |
| `ddcolor <d> <rect> <v>` | Blt DDBLT_COLORFILL | `ret <hr>` |
| `ddfast <d> <x> <y> <s> <rect> [srckey] [destkey]` | BltFast (DDBLTFAST_WAIT) | `ret <hr>` |

A rect is `all` (NULL) or `x0 y0 x1 y1`. Without `dd` the lines print `nodev`.

## OpenGL

opengl32.dll is loaded when a case first asks for it and every gl / wgl function is looked up by
name. Every DIB here is 16x16, bottom-up (row 0 is GL's y 0), in a memory DC of its own; 16 bpp
is 5-5-5, 8 bpp has the `dib` default color table. On a system with no GPU driver (Windows'
runner) this is Microsoft's GDI generic OpenGL 1.1.

| line | calls | prints |
|---|---|---|
| `glpf <bpp>` | DescribePixelFormat of every format on a DIB of bpp | `glpf <bpp> n <count>`, then per format `pf <i> <flags> <type> <color> <r>,<rshift> <g>,<gs> <b>,<bs> <a>,<as> accum <bits> <r>,<g>,<b>,<a> depth <d> stencil <s> aux <n> <layer type> <reserved> <layer mask> <visible mask> <damage mask>` (masks, flags hex) |
| `glchoose <bpp> <flags> <type> <color> <alpha> <accum> <depth> <stencil>` | ChoosePixelFormat of that PIXELFORMATDESCRIPTOR (the rest 0) on a DIB of bpp | `glchoose <index>` |
| `glctx <bpp> <index>` / `glctx <bpp> choose <flags> ... <stencil>` | the case's DIB, that format (or ChoosePixelFormat's), SetPixelFormat, wglCreateContext, wglMakeCurrent | `glctx ok <GetPixelFormat>`, `glctx none` (no opengl32), `glctx set\|create\|current failed <GetLastError>` |
| `gl <function> <args>` | that function (runner/opengl.c lists the ones allowed and their argument types); numbers as above, floats as decimals | `gl <function> none` if opengl32 lacks it |
| `glmat load\|mult <16 floats>` | glLoadMatrixf / glMultMatrixf (column-major) | |
| `glerr` | glGetError until GL_NO_ERROR, at most 8 | `glerr <codes>` (hex) or `glerr 0` |
| `glget i\|f\|b\|d <pname> <n>` | glGet{Integer,Float,Boolean,Double}v into n values (1..16) filled with 0xcc bytes | `get <pname> = <n values>`: i decimal, f / d the bits in hex, b the byte; a value left untouched is `-` |
| `glisen <cap>`, `glstr <name>` | glIsEnabled, glGetString | `isen <cap> = <0\|1>`, `str <name> = <string\|(null)>` |
| `glread <x> <y> <w> <h> <format> <type> <bytes>` | glReadPixels into bytes (1..65536) filled with 0xcc, 64 more after them | `read <hex>`, then `over` if any of the 64 changed |
| `gldraw <w> <h> <format> <type> <seed> <bytes>` | glDrawPixels of that many bytes from xorshift32 | |
| `gldib` | glFinish, GdiFlush, the DIB | `px gl 16 16 <bpp>` and its rows, bottom row first |
| `glcur` | wglGetCurrentContext, wglGetCurrentDC | `glcur <c> <d>`: 0 none, 1 the case's, 2 another |
| `glrelease` | wglMakeCurrent(NULL, NULL), wglDeleteContext | `glrelease <ret> <ret>` |

Without a context the gl lines print `noctx`. A case's context and DIB are released at `end`.

## Models

`kikizake model <suite>` prints what Windows prints, case by case. A case whose behavior the
documentation does not settle and Windows' answers have not shown yet prints
`unmodeled <why>` instead; it is then checked against Windows' recorded answers only.
