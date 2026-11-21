#include "core.h"
/* GDI+ (gdiplus.dll, loaded and started by the first case that asks for it; its flat API through
 * GetProcAddress, declared here: the C headers of GDI+ are C++). Bitmaps in two slots, their
 * pixels through LockBits in any format, GetPixel / SetPixel, palettes, fills and clears through a
 * Graphics of the bitmap, RotateFlip, CloneBitmapArea, matrices, and solid and hatch brushes
 * painting rectangles in float coordinates, image attributes (color matrices) and DrawImage of
 * one bitmap onto another.
 *
 * Every op prints the Status it got (decimal, the Status enum of gdiplustypes.h) and, when that
 * is Ok, what it read. */

typedef int GpStatus;
typedef struct { UINT Width, Height; INT Stride; INT PixelFormat; void *Scan0; UINT_PTR Reserved; } GpBitmapData;
typedef struct { INT X, Y, Width, Height; } GpRect;
typedef struct { float X, Y; } GpPointF;
typedef struct { INT X, Y; } GpPoint;
typedef struct { float X, Y, Width, Height; } GpRectF;
typedef struct { UINT32 GdiplusVersion; void *DebugEventCallback; BOOL SuppressBackgroundThread, SuppressExternalCodecs; } GpStartupInput;

#define GP_FUNCS(X) \
    X(GdiplusStartup, (ULONG_PTR *, const GpStartupInput *, void *)) \
    X(GdipCreateBitmapFromScan0, (INT, INT, INT, INT, BYTE *, void **)) \
    X(GdipDisposeImage, (void *)) \
    X(GdipBitmapLockBits, (void *, const GpRect *, UINT, INT, GpBitmapData *)) \
    X(GdipBitmapUnlockBits, (void *, GpBitmapData *)) \
    X(GdipBitmapGetPixel, (void *, INT, INT, DWORD *)) \
    X(GdipBitmapSetPixel, (void *, INT, INT, DWORD)) \
    X(GdipGetImagePaletteSize, (void *, INT *)) \
    X(GdipGetImagePalette, (void *, void *, INT)) \
    X(GdipGetImagePixelFormat, (void *, INT *)) \
    X(GdipGetImageWidth, (void *, UINT *)) \
    X(GdipGetImageHeight, (void *, UINT *)) \
    X(GdipGetImageGraphicsContext, (void *, void **)) \
    X(GdipDeleteGraphics, (void *)) \
    X(GdipSetCompositingMode, (void *, INT)) \
    X(GdipSetCompositingQuality, (void *, INT)) \
    X(GdipGraphicsClear, (void *, DWORD)) \
    X(GdipCreateSolidFill, (DWORD, void **)) \
    X(GdipDeleteBrush, (void *)) \
    X(GdipFillRectangleI, (void *, void *, INT, INT, INT, INT)) \
    X(GdipImageRotateFlip, (void *, INT)) \
    X(GdipCloneBitmapAreaI, (INT, INT, INT, INT, INT, void *, void **)) \
    X(GdipCreateMatrix2, (float, float, float, float, float, float, void **)) \
    X(GdipDeleteMatrix, (void *)) \
    X(GdipGetMatrixElements, (void *, float *)) \
    X(GdipMultiplyMatrix, (void *, void *, INT)) \
    X(GdipInvertMatrix, (void *)) \
    X(GdipIsMatrixInvertible, (void *, BOOL *)) \
    X(GdipTransformMatrixPoints, (void *, GpPointF *, INT)) \
    X(GdipCreateHatchBrush, (INT, DWORD, DWORD, void **)) \
    X(GdipCreateLineBrushI, (const GpPoint *, const GpPoint *, DWORD, DWORD, INT, void **)) \
    X(GdipCreateTexture, (void *, INT, void **)) \
    X(GdipGetBrushType, (void *, INT *)) \
    X(GdipGetLineRect, (void *, GpRectF *)) \
    X(GdipGetLineTransform, (void *, void *)) \
    X(GdipGetTextureTransform, (void *, void *)) \
    X(GdipCreatePen1, (DWORD, float, INT, void **)) \
    X(GdipDeletePen, (void *)) \
    X(GdipSetPenDashStyle, (void *, INT)) \
    X(GdipSetPenDashOffset, (void *, float)) \
    X(GdipSetPenDashArray, (void *, const float *, INT)) \
    X(GdipSetPenLineCap197819, (void *, INT, INT, INT)) \
    X(GdipSetPenLineJoin, (void *, INT)) \
    X(GdipSetPenMode, (void *, INT)) \
    X(GdipSetPenMiterLimit, (void *, float)) \
    X(GdipSetWorldTransform, (void *, void *)) \
    X(GdipDrawLine, (void *, void *, float, float, float, float)) \
    X(GdipDrawLines, (void *, void *, const GpPointF *, INT)) \
    X(GdipDrawRectangle, (void *, void *, float, float, float, float)) \
    X(GdipDrawEllipse, (void *, void *, float, float, float, float)) \
    X(GdipFillEllipse, (void *, void *, float, float, float, float)) \
    X(GdipDrawPolygon, (void *, void *, const GpPointF *, INT)) \
    X(GdipFillPolygon, (void *, void *, const GpPointF *, INT, INT)) \
    X(GdipDrawBezier, (void *, void *, float, float, float, float, float, float, float, float)) \
    X(GdipDrawArc, (void *, void *, float, float, float, float, float, float)) \
    X(GdipDrawPie, (void *, void *, float, float, float, float, float, float)) \
    X(GdipFillPie, (void *, void *, float, float, float, float, float, float)) \
    X(GdipDrawCurve2, (void *, void *, const GpPointF *, INT, float)) \
    X(GdipDrawClosedCurve2, (void *, void *, const GpPointF *, INT, float)) \
    X(GdipFillClosedCurve2, (void *, void *, const GpPointF *, INT, float, INT)) \
    X(GdipFillRectangle, (void *, void *, float, float, float, float)) \
    X(GdipSetSmoothingMode, (void *, INT)) \
    X(GdipSetPixelOffsetMode, (void *, INT)) \
    X(GdipSetRenderingOrigin, (void *, INT, INT)) \
    X(GdipCreateImageAttributes, (void **)) \
    X(GdipDisposeImageAttributes, (void *)) \
    X(GdipSetImageAttributesColorMatrix, (void *, INT, BOOL, const float *, const float *, INT)) \
    X(GdipSetInterpolationMode, (void *, INT)) \
    X(GdipDrawImageRectRectI, (void *, void *, INT, INT, INT, INT, INT, INT, INT, INT, INT, void *, void *, void *)) \
    X(GdipCreatePath, (INT, void **)) \
    X(GdipDeletePath, (void *)) \
    X(GdipAddPathLine, (void *, float, float, float, float)) \
    X(GdipAddPathLine2, (void *, const GpPointF *, INT)) \
    X(GdipAddPathBezier, (void *, float, float, float, float, float, float, float, float)) \
    X(GdipAddPathBeziers, (void *, const GpPointF *, INT)) \
    X(GdipAddPathRectangle, (void *, float, float, float, float)) \
    X(GdipAddPathEllipse, (void *, float, float, float, float)) \
    X(GdipAddPathArc, (void *, float, float, float, float, float, float)) \
    X(GdipAddPathPie, (void *, float, float, float, float, float, float)) \
    X(GdipAddPathPolygon, (void *, const GpPointF *, INT)) \
    X(GdipAddPathCurve2, (void *, const GpPointF *, INT, float)) \
    X(GdipAddPathClosedCurve2, (void *, const GpPointF *, INT, float)) \
    X(GdipClosePathFigure, (void *)) \
    X(GdipStartPathFigure, (void *)) \
    X(GdipFlattenPath, (void *, void *, float)) \
    X(GdipWidenPath, (void *, void *, void *, float)) \
    X(GdipGetPointCount, (void *, INT *)) \
    X(GdipGetPathTypes, (void *, BYTE *, INT)) \
    X(GdipGetPathPoints, (void *, GpPointF *, INT)) \
    X(GdipGetPathWorldBounds, (void *, GpRectF *, void *, void *)) \
    X(GdipFillPath, (void *, void *, void *)) \
    X(GdipSetPenCompoundArray, (void *, const float *, INT)) \
    X(GdipCreateAdjustableArrowCap, (float, float, BOOL, void **)) \
    X(GdipSetAdjustableArrowCapMiddleInset, (void *, float)) \
    X(GdipCreateCustomLineCap, (void *, void *, INT, float, void **)) \
    X(GdipSetCustomLineCapWidthScale, (void *, float)) \
    X(GdipSetCustomLineCapStrokeJoin, (void *, INT)) \
    X(GdipSetCustomLineCapStrokeCaps, (void *, INT, INT)) \
    X(GdipSetPenCustomStartCap, (void *, void *)) \
    X(GdipSetPenCustomEndCap, (void *, void *)) \
    X(GdipDeleteCustomLineCap, (void *))

#define X(n, a) static GpStatus (WINAPI *n) a;
GP_FUNCS(X)
#undef X

static int gp_state; /* 0 not tried, 1 started, -1 none */
/* 1 when GDI+ is there and started; otherwise prints "<op> none" */
static int gp(void) {
    if (!gp_state) {
        HMODULE m = LoadLibraryA("gdiplus.dll");
        gp_state = -1;
        if (m) {
            gp_state = 1;
#define X(n, a) if (!(*(FARPROC *)&n = GetProcAddress(m, #n))) gp_state = -1;
            GP_FUNCS(X)
#undef X
        }
        ULONG_PTR token;
        GpStartupInput in = { 1, NULL, FALSE, TRUE };
        if (gp_state == 1 && GdiplusStartup(&token, &in, NULL)) gp_state = -1;
    }
    if (gp_state < 0) out("%s none\n", tok[0]);
    return gp_state > 0;
}

/* the pixel formats by their gdipluspixelformats.h names, without "PixelFormat" */
static const struct { const char *n; INT f; } gpfmts[] = {
    { "1bppIndexed", 0x00030101 }, { "4bppIndexed", 0x00030402 }, { "8bppIndexed", 0x00030803 },
    { "16bppGrayScale", 0x00101004 }, { "16bppRGB555", 0x00021005 }, { "16bppRGB565", 0x00021006 },
    { "16bppARGB1555", 0x00061007 }, { "24bppRGB", 0x00021808 }, { "32bppRGB", 0x00022009 },
    { "32bppARGB", 0x0026200A }, { "32bppPARGB", 0x000E200B }, { "48bppRGB", 0x0010300C },
    { "64bppARGB", 0x0034400D }, { "64bppPARGB", 0x001C400E },
};
static INT gpfmt(int i) {
    for (int f = 0; f < (int)(sizeof gpfmts / sizeof *gpfmts); f++)
        if (is(i, gpfmts[f].n)) return gpfmts[f].f;
    bad = 1;
    return 0;
}
static int fmtbits(INT f) { return f >> 8 & 255; }

#define SLOTS 2
static void *bmp[SLOTS];
static void *mat[SLOTS];
static void *brush[SLOTS];
static void *attr[SLOTS];
static void *pen[SLOTS];
static void *path[SLOTS];

static void gdiplus_reset(void) {
    for (int i = 0; i < SLOTS; i++) {
        if (bmp[i]) { GdipDisposeImage(bmp[i]); bmp[i] = NULL; }
        if (mat[i]) { GdipDeleteMatrix(mat[i]); mat[i] = NULL; }
        if (brush[i]) { GdipDeleteBrush(brush[i]); brush[i] = NULL; }
        if (attr[i]) { GdipDisposeImageAttributes(attr[i]); attr[i] = NULL; }
        if (pen[i]) { GdipDeletePen(pen[i]); pen[i] = NULL; }
        if (path[i]) { GdipDeletePath(path[i]); path[i] = NULL; }
    }
}

static int slot(int i) {
    long s = num(i);
    if (s < 0 || s >= SLOTS) bad = 1;
    return bad ? 0 : (int)s;
}

/* gpbitmap <slot> <w> <h> <format>: CreateBitmapFromScan0 (stride 0, no scan0): "bitmap <st>" */
static void op_bitmap(void) {
    int s = slot(1);
    long w = num(2), h = num(3);
    INT f = gpfmt(4);
    if (bad || !gp()) return;
    if (bmp[s]) { GdipDisposeImage(bmp[s]); bmp[s] = NULL; }
    void *b = NULL;
    GpStatus st = GdipCreateBitmapFromScan0(w, h, 0, f, NULL, &b);
    out("bitmap %d\n", st);
    if (!st) bmp[s] = b;
}

/* the bitmap in a slot, or "<op> nobitmap" */
static void *the(int s) {
    if (!bmp[s]) out("%s nobitmap\n", tok[0]);
    return bmp[s];
}

/* the bytes a fill writes: "seed <n>" xorshift32 bytes; "ramp" byte k = k mod 256; "pairs" (32-bit
 * formats) pixel (x, y) = bytes x, 255 - x, 7x mod 256, y; "levels" (48 and 64-bit formats) pixel
 * k's color words k mod 8193, its alpha word 8192; "grid" (24-bit) pixel k = the 5-bit blue, green,
 * red of k (bits 0-4, 5-9, 10-14), each widened to 8 bits by repeating its high bits */
static void fill_bytes(unsigned char *p, int stride, int w, int h, int bits, int i) {
    int rb = (w * bits + 7) / 8;
    unsigned x = 0;
    if (is(i, "seed")) x = (unsigned)num(i + 1);
    else if (!is(i, "ramp") && !is(i, "pairs") && !is(i, "levels") && !is(i, "grid")) bad = 1;
    if (is(i, "grid") && bits != 24) bad = 1;
    if (is(i, "pairs") && bits != 32) bad = 1;
    if (is(i, "levels") && bits != 48 && bits != 64) bad = 1;
    if (bad) return;
    if (!x) x = 0x9e3779b9;
    long k = 0;
    for (int y = 0; y < h; y++) {
        unsigned char *r = p + (long)y * stride;
        for (int b = 0; b < rb; b++, k++) {
            if (is(i, "seed")) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; r[b] = (unsigned char)x; }
            else if (is(i, "ramp")) r[b] = (unsigned char)k;
            else if (is(i, "grid")) { long px = (long)y * w + b / 3; int v = px >> (5 * (b % 3)) & 31; r[b] = (unsigned char)(v << 3 | v >> 2); }
            else if (is(i, "pairs")) { int px = b / 4, c = b % 4; r[b] = (unsigned char)(c == 0 ? px : c == 1 ? 255 - px : c == 2 ? px * 7 : y); }
            else {
                long pix = (long)y * w + b / (bits / 8);
                int word = b % (bits / 8) / 2;
                unsigned v = bits == 64 && word == 3 ? 8192 : (unsigned)(pix % 8193);
                r[b] = (unsigned char)(b & 1 ? v >> 8 : v);
            }
        }
    }
}

/* the rectangle tokens i..i+3, or the whole bitmap for i < 0 */
static GpRect rect_at(int s, int i) {
    GpRect r = { 0 };
    if (i < 0) {
        UINT w = 0, h = 0;
        GdipGetImageWidth(bmp[s], &w);
        GdipGetImageHeight(bmp[s], &h);
        r.Width = w;
        r.Height = h;
    } else {
        r.X = num(i); r.Y = num(i + 1); r.Width = num(i + 2); r.Height = num(i + 3);
    }
    return r;
}

/* gpwrite <slot> <format> <fill...> [at <x> <y> <w> <h>]: LockBits write-only in that format, the
 * fill, UnlockBits: "write <lock st> <unlock st>" */
static void op_write(void) {
    int s = slot(1);
    INT f = gpfmt(2);
    int at = 0;
    for (int i = 3; i < ntok; i++) if (is(i, "at")) at = i;
    if (bad || !gp() || !the(s)) return;
    GpRect r = rect_at(s, at ? at + 1 : -1);
    if (bad) return;
    GpBitmapData d = { 0 };
    GpStatus st = GdipBitmapLockBits(bmp[s], &r, 2 /* ImageLockModeWrite */, f, &d);
    if (st) { out("write %d\n", st); return; }
    if ((INT)d.Width != r.Width || (INT)d.Height != r.Height || !d.Scan0) {
        out("write 0 locked %u %u\n", d.Width, d.Height);
        GdipBitmapUnlockBits(bmp[s], &d);
        return;
    }
    fill_bytes(d.Scan0, d.Stride, d.Width, d.Height, fmtbits(f), 3);
    out("write 0 %d\n", GdipBitmapUnlockBits(bmp[s], &d));
}

/* gpread <slot> <format> [at <x> <y> <w> <h>]: LockBits read-only in that format: "read <st>", then
 * "px <w> <h> <bits>" and each row's bytes (bits past the last pixel 0) */
static void op_read(void) {
    int s = slot(1);
    INT f = gpfmt(2);
    int at = is(3, "at");
    if (bad || !gp() || !the(s)) return;
    GpRect r = rect_at(s, at ? 4 : -1);
    if (bad) return;
    GpBitmapData d = { 0 };
    GpStatus st = GdipBitmapLockBits(bmp[s], &r, 1 /* ImageLockModeRead */, f, &d);
    out("read %d\n", st);
    if (st) return;
    if ((INT)d.Width != r.Width || (INT)d.Height != r.Height || !d.Scan0) {
        /* (not the rectangle asked: nothing read past it) */
        out("locked %u %u\n", d.Width, d.Height);
        GdipBitmapUnlockBits(bmp[s], &d);
        return;
    }
    int bits = fmtbits(f), rb = (d.Width * bits + 7) / 8;
    out("px %u %u %d\n", d.Width, d.Height, bits);
    unsigned char *row = malloc(rb + 1);
    for (UINT y = 0; y < d.Height; y++) {
        memcpy(row, (unsigned char *)d.Scan0 + (long)y * d.Stride, rb);
        int tail = d.Width * bits % 8;
        if (tail) row[rb - 1] &= (unsigned char)(0xff00 >> tail);
        out_hex(row, rb);
    }
    free(row);
    st = GdipBitmapUnlockBits(bmp[s], &d);
    if (st) out("unlock %d\n", st);
}

/* gpset <slot> <x> <y> <argb hex>: "set <st>"; gpget <slot> <x> <y>: "get <st> [<argb>]" */
static void op_set(void) {
    int s = slot(1);
    long x = num(2), y = num(3);
    DWORD c = (DWORD)num(4);
    if (bad || !gp() || !the(s)) return;
    out("set %d\n", GdipBitmapSetPixel(bmp[s], x, y, c));
}
static void op_get(void) {
    int s = slot(1);
    long x = num(2), y = num(3);
    if (bad || !gp() || !the(s)) return;
    DWORD c = 0;
    GpStatus st = GdipBitmapGetPixel(bmp[s], x, y, &c);
    if (st) out("get %d\n", st);
    else out("get 0 %08lx\n", c);
}

/* gpinfo <slot>: "info <format hex> <w> <h> palette <st> <flags> <count>" and the entries */
static void op_info(void) {
    int s = slot(1);
    if (bad || !gp() || !the(s)) return;
    INT f = 0, size = 0;
    UINT w = 0, h = 0;
    GdipGetImagePixelFormat(bmp[s], &f);
    GdipGetImageWidth(bmp[s], &w);
    GdipGetImageHeight(bmp[s], &h);
    GpStatus st = GdipGetImagePaletteSize(bmp[s], &size);
    out("info %08x %u %u palette %d", f, w, h, st);
    if (st || size < 8 || size > 8 + 256 * 4) { out(" size %d\n", size); return; }
    UINT *p = calloc(1, size);
    st = GdipGetImagePalette(bmp[s], p, size);
    if (st) { out(" get %d\n", st); free(p); return; }
    out(" %u %u\n", p[0], p[1]);
    for (UINT i = 0; i < p[1] && i < 256; i++) out("%08x%s", p[2 + i], i + 1 == p[1] || i % 16 == 15 ? "\n" : " ");
    free(p);
}

/* gpfill <slot> <x> <y> <w> <h> <argb> <over|copy> <quality>: a Graphics of the bitmap, its
 * compositing mode and quality (CompositingQuality: 0 default .. 4 assume linear), FillRectangleI
 * with a solid brush: "fill <graphics st> [<fill st>]"; gpclear <slot> <argb>: GraphicsClear */
static void *graphics(int s, const char *what) {
    void *g = NULL;
    GpStatus st = GdipGetImageGraphicsContext(bmp[s], &g);
    if (st) { out("%s %d\n", what, st); return NULL; }
    return g;
}
static void op_fill(void) {
    int s = slot(1);
    long x = num(2), y = num(3), w = num(4), h = num(5);
    DWORD c = (DWORD)num(6);
    int copy = is(7, "copy");
    if (!copy && !is(7, "over")) bad = 1;
    long q = num(8);
    if (bad || !gp() || !the(s)) return;
    void *g = graphics(s, "fill");
    if (!g) return;
    void *br = NULL;
    GdipSetCompositingMode(g, copy);
    GpStatus st = GdipSetCompositingQuality(g, q);
    if (!st) st = GdipCreateSolidFill(c, &br);
    if (!st) st = GdipFillRectangleI(g, br, x, y, w, h);
    if (br) GdipDeleteBrush(br);
    GdipDeleteGraphics(g);
    out("fill 0 %d\n", st);
}
static void op_clear(void) {
    int s = slot(1);
    DWORD c = (DWORD)num(2);
    if (bad || !gp() || !the(s)) return;
    void *g = graphics(s, "clear");
    if (!g) return;
    GpStatus st = GdipGraphicsClear(g, c);
    GdipDeleteGraphics(g);
    out("clear 0 %d\n", st);
}

/* gprotate <slot> <RotateFlipType 0..7>: "rotate <st> <w> <h>" */
static void op_rotate(void) {
    int s = slot(1);
    long t = num(2);
    if (bad || !gp() || !the(s)) return;
    GpStatus st = GdipImageRotateFlip(bmp[s], t);
    UINT w = 0, h = 0;
    GdipGetImageWidth(bmp[s], &w);
    GdipGetImageHeight(bmp[s], &h);
    out("rotate %d %u %u\n", st, w, h);
}

/* gpclone <dst slot> <src slot> <x> <y> <w> <h> <format>: CloneBitmapAreaI: "clone <st>" */
static void op_clone(void) {
    int d = slot(1), s = slot(2);
    long x = num(3), y = num(4), w = num(5), h = num(6);
    INT f = gpfmt(7);
    if (bad || !gp() || !the(s)) return;
    void *b = NULL;
    GpStatus st = GdipCloneBitmapAreaI(x, y, w, h, f, bmp[s], &b);
    out("clone %d\n", st);
    if (st) return;
    if (bmp[d]) GdipDisposeImage(bmp[d]);
    bmp[d] = b;
}

/* matrices; floats are written and printed as the hex of their bits.
 * gpmatrix <slot> <m11> <m12> <m21> <m22> <dx> <dy>: CreateMatrix2 "matrix <st>"
 * gpmatmul <slot> <other slot> <0 prepend|1 append>: MultiplyMatrix, then the elements
 * gpmatinv <slot>: IsMatrixInvertible, InvertMatrix, then the elements
 * gpmatpts <slot> <x> <y>...: TransformMatrixPoints: "pts <st> <x> <y>..." */
static float fbits(int i) {
    union { DWORD u; float f; } v;
    v.u = (DWORD)num(i);
    return v.f;
}
static DWORD bitsf(float f) {
    union { DWORD u; float f; } v;
    v.f = f;
    return v.u;
}
static void elements(int s) {
    float e[6];
    GpStatus st = GdipGetMatrixElements(mat[s], e);
    out("elements %d", st);
    for (int i = 0; i < 6 && !st; i++) out(" %08lx", bitsf(e[i]));
    out("\n");
}
static void *the_matrix(int s) {
    if (!mat[s]) out("%s nomatrix\n", tok[0]);
    return mat[s];
}
static void op_matrix(void) {
    int s = slot(1);
    float e[6];
    for (int i = 0; i < 6; i++) e[i] = fbits(2 + i);
    if (bad || !gp()) return;
    if (mat[s]) { GdipDeleteMatrix(mat[s]); mat[s] = NULL; }
    void *m = NULL;
    GpStatus st = GdipCreateMatrix2(e[0], e[1], e[2], e[3], e[4], e[5], &m);
    out("matrix %d\n", st);
    if (!st) { mat[s] = m; elements(s); }
}
static void op_matmul(void) {
    int s = slot(1), o = slot(2);
    long order = num(3);
    if (bad || !gp() || !the_matrix(s) || !the_matrix(o)) return;
    out("matmul %d\n", GdipMultiplyMatrix(mat[s], mat[o], order));
    elements(s);
}
static void op_matinv(void) {
    int s = slot(1);
    if (bad || !gp() || !the_matrix(s)) return;
    BOOL can = 0;
    GpStatus st = GdipIsMatrixInvertible(mat[s], &can);
    out("matinv %d %d %d\n", st, can, GdipInvertMatrix(mat[s]));
    elements(s);
}
static void op_matpts(void) {
    int s = slot(1);
    GpPointF p[16];
    int nt = ntok;
    int n = (nt - 2) / 2;
    if ((nt - 2) % 2 || n < 1 || n > 16) bad = 1;
    for (int i = 0; i < n && !bad; i++) { p[i].X = fbits(2 + 2 * i); p[i].Y = fbits(3 + 2 * i); }
    if (bad || !gp() || !the_matrix(s)) return;
    GpStatus st = GdipTransformMatrixPoints(mat[s], p, n);
    out("pts %d", st);
    for (int i = 0; i < n && !st; i++) out(" %08lx %08lx", bitsf(p[i].X), bitsf(p[i].Y));
    out("\n");
}

/* brushes in slots of their own:
 * gpbrush <b> solid <argb>: CreateSolidFill; gpbrush <b> hatch <HatchStyle> <fore> <back>:
 * CreateHatchBrush; gpbrush <b> linear <x1> <y1> <x2> <y2> <argb1> <argb2> <WrapMode>:
 * CreateLineBrushI; gpbrush <b> texture <bitmap slot> <WrapMode>: CreateTexture of the bitmap as
 * it is then: "brush <st>"
 * gppaint <slot> <brush> <x> <y> <w> <h> <over|copy> <CompositingQuality> <SmoothingMode>
 * <PixelOffsetMode> <origin x> <origin y>: a Graphics of the bitmap in those modes, its rendering
 * origin, FillRectangle (floats, written as decimals): "paint 0 <st>" */
static float fdec(int i) {
    if (i >= ntok) { bad = 1; return 0; }
    char *e;
    float v = strtof(tok[i], &e);
    if (*e) bad = 1;
    return v;
}
static void op_brush(void) {
    int b = slot(1);
    int solid = is(2, "solid"), hatch = is(2, "hatch"), linear = is(2, "linear"), texture = is(2, "texture");
    if (!solid && !hatch && !linear && !texture) bad = 1;
    DWORD c = 0, back = 0;
    long style = 0, wrap = 0;
    int src = 0;
    GpPoint p1 = { 0, 0 }, p2 = { 0, 0 };
    if (solid) c = (DWORD)num(3);
    if (hatch) { style = num(3); c = (DWORD)num(4); back = (DWORD)num(5); }
    if (linear) {
        p1.X = num(3); p1.Y = num(4); p2.X = num(5); p2.Y = num(6);
        c = (DWORD)num(7); back = (DWORD)num(8); wrap = num(9);
    }
    if (texture) { src = slot(3); wrap = num(4); }
    if (bad || !gp()) return;
    if (texture && !the(src)) return;
    if (brush[b]) { GdipDeleteBrush(brush[b]); brush[b] = NULL; }
    void *br = NULL;
    GpStatus st = solid ? GdipCreateSolidFill(c, &br)
        : hatch ? GdipCreateHatchBrush(style, c, back, &br)
        : linear ? GdipCreateLineBrushI(&p1, &p2, c, back, wrap, &br)
        : GdipCreateTexture(bmp[src], wrap, &br);
    out("brush %d\n", st);
    if (!st) brush[b] = br;
}
static void op_paint(void) {
    int s = slot(1), b = slot(2);
    float x = fdec(3), y = fdec(4), w = fdec(5), h = fdec(6);
    int copy = is(7, "copy");
    if (!copy && !is(7, "over")) bad = 1;
    long q = num(8), sm = num(9), po = num(10), ox = num(11), oy = num(12);
    if (bad || !gp() || !the(s)) return;
    if (!brush[b]) { out("paint nobrush\n"); return; }
    void *g = graphics(s, "paint");
    if (!g) return;
    GdipSetCompositingMode(g, copy);
    GpStatus st = GdipSetCompositingQuality(g, q);
    if (!st) st = GdipSetSmoothingMode(g, sm);
    if (!st) st = GdipSetPixelOffsetMode(g, po);
    if (!st) st = GdipSetRenderingOrigin(g, ox, oy);
    if (!st) st = GdipFillRectangle(g, brush[b], x, y, w, h);
    GdipDeleteGraphics(g);
    out("paint 0 %d\n", st);
}

/* gpattr <a> none | matrix <flags> <25 floats, row by row> [gray <25 floats>]: an ImageAttributes,
 * and SetColorMatrix (ColorAdjustTypeDefault) of the matrix (and gray matrix): "attr <st> <st>" */
static void op_attr(void) {
    int a = slot(1);
    int none = is(2, "none"), matrix = is(2, "matrix");
    if (!none && !matrix) bad = 1;
    float m[25], g[25];
    long flags = 0;
    int gray = 0;
    if (matrix) {
        flags = num(3);
        for (int i = 0; i < 25; i++) m[i] = fdec(4 + i);
        if (ntok > 29 && is(29, "gray")) {
            gray = 1;
            for (int i = 0; i < 25; i++) g[i] = fdec(30 + i);
        }
    }
    if (bad || !gp()) return;
    if (attr[a]) { GdipDisposeImageAttributes(attr[a]); attr[a] = NULL; }
    void *ia = NULL;
    GpStatus st = GdipCreateImageAttributes(&ia);
    GpStatus sm = 0;
    if (!st && matrix) sm = GdipSetImageAttributesColorMatrix(ia, 0, TRUE, m, gray ? g : NULL, flags);
    out("attr %d %d\n", st, sm);
    if (!st) attr[a] = ia;
}
/* gpdraw <dst> <src> <attr | -> <dx> <dy> <dw> <dh> <sx> <sy> <sw> <sh> <over|copy> <quality>
 * <interpolation> <pixeloffset>: DrawImageRectRectI in pixels through a Graphics of the destination:
 * "draw 0 <st>" */
static void op_draw(void) {
    int d = slot(1), s = slot(2);
    int a = is(3, "-") ? -1 : slot(3);
    long v[8];
    for (int i = 0; i < 8; i++) v[i] = num(4 + i);
    int copy = is(12, "copy");
    if (!copy && !is(12, "over")) bad = 1;
    long q = num(13), im = num(14), po = num(15);
    if (bad || !gp() || !the(d) || !the(s)) return;
    if (a >= 0 && !attr[a]) { out("draw noattr\n"); return; }
    void *g = graphics(d, "draw");
    if (!g) return;
    GdipSetCompositingMode(g, copy);
    GpStatus st = GdipSetCompositingQuality(g, q);
    if (!st) st = GdipSetInterpolationMode(g, im);
    if (!st) st = GdipSetPixelOffsetMode(g, po);
    if (!st) st = GdipDrawImageRectRectI(g, bmp[s], v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], 2, a >= 0 ? attr[a] : NULL, NULL, NULL);
    GdipDeleteGraphics(g);
    out("draw 0 %d\n", st);
}

/* gpbrushinfo <b>: the brush's type; a linear gradient's rectangle and transform, a texture's
 * transform (floats as the hex of their bits): "brushinfo <st> <type> [<st> <rect x y w h>]
 * [<st> <6 elements>]" */
static void op_brushinfo(void) {
    int b = slot(1);
    if (bad || !gp()) return;
    if (!brush[b]) { out("brushinfo nobrush\n"); return; }
    INT type = -1;
    GpStatus st = GdipGetBrushType(brush[b], &type);
    out("brushinfo %d %d", st, type);
    if (!st && (type == 4 || type == 2)) {
        if (type == 4) {
            GpRectF r = { 0 };
            GpStatus rs = GdipGetLineRect(brush[b], &r);
            out(" %d %08lx %08lx %08lx %08lx", rs, bitsf(r.X), bitsf(r.Y), bitsf(r.Width), bitsf(r.Height));
        }
        void *m = NULL;
        GpStatus ms = GdipCreateMatrix2(1, 0, 0, 1, 0, 0, &m);
        if (!ms) ms = type == 4 ? GdipGetLineTransform(brush[b], m) : GdipGetTextureTransform(brush[b], m);
        float e[6] = { 0 };
        if (!ms) ms = GdipGetMatrixElements(m, e);
        out(" %d", ms);
        for (int i = 0; i < 6 && !ms; i++) out(" %08lx", bitsf(e[i]));
        if (m) GdipDeleteMatrix(m);
    }
    out("\n");
}

/* a custom cap set on a pen's start (0) or end (1): its status, then the first failing call's */
static GpStatus set_cap(void *pn, int end, void *cap, GpStatus st) {
    if (!st) st = end ? GdipSetPenCustomEndCap(pn, cap) : GdipSetPenCustomStartCap(pn, cap);
    if (cap) GdipDeleteCustomLineCap(cap);
    return st;
}

/* pens in slots of their own:
 * gppen <p> <argb> <width> [dash <DashStyle>] [cap <start> <end> <DashCap>] [join <LineJoin>]
 * [align <PenAlignment>] [miter <limit>] [compound <n> <n floats>] [dashoff <offset>]
 * [dasharray <n> <n floats>]
 * [arrowcap <0 start|1 end> <height> <width> <filled> <middle inset>]
 * [customcap <0 start|1 end> <0 fill|1 stroke> <baseCap> <baseInset> <widthScale> <stroke join>
 *  <stroke start cap> <stroke end cap> <n> <n points>]: CreatePen1 (UnitWorld), then each setter
 * asked (a cap: made, set on the pen, deleted; the first failing call's status; the custom cap's
 * path a polygon for fill, open lines for stroke): "pen <st> <st of each setter>" */
static void op_pen(void) {
    int p = slot(1);
    DWORD c = (DWORD)num(2);
    float w = fdec(3);
    if (bad || !gp()) return;
    if (pen[p]) { GdipDeletePen(pen[p]); pen[p] = NULL; }
    void *pn = NULL;
    GpStatus st = GdipCreatePen1(c, w, 0, &pn);
    out("pen %d", st);
    for (int i = 4; i < ntok && !st && !bad;) {
        GpStatus r;
        if (is(i, "dash")) { r = GdipSetPenDashStyle(pn, num(i + 1)); i += 2; }
        else if (is(i, "cap")) { r = GdipSetPenLineCap197819(pn, num(i + 1), num(i + 2), num(i + 3)); i += 4; }
        else if (is(i, "join")) { r = GdipSetPenLineJoin(pn, num(i + 1)); i += 2; }
        else if (is(i, "align")) { r = GdipSetPenMode(pn, num(i + 1)); i += 2; }
        else if (is(i, "miter")) { r = GdipSetPenMiterLimit(pn, fdec(i + 1)); i += 2; }
        else if (is(i, "compound")) {
            int n = num(i + 1);
            float v[16];
            if (n < 0 || n > 16) { bad = 1; break; }
            for (int k = 0; k < n; k++) v[k] = fdec(i + 2 + k);
            r = GdipSetPenCompoundArray(pn, v, n);
            i += 2 + n;
        } else if (is(i, "dashoff")) { r = GdipSetPenDashOffset(pn, fdec(i + 1)); i += 2; }
        else if (is(i, "dasharray")) {
            int n = num(i + 1);
            float v[16];
            if (n < 0 || n > 16) { bad = 1; break; }
            for (int k = 0; k < n; k++) v[k] = fdec(i + 2 + k);
            r = GdipSetPenDashArray(pn, v, n);
            i += 2 + n;
        } else if (is(i, "arrowcap")) {
            void *cap = NULL;
            r = GdipCreateAdjustableArrowCap(fdec(i + 2), fdec(i + 3), num(i + 4) != 0, &cap);
            if (!r) r = GdipSetAdjustableArrowCapMiddleInset(cap, fdec(i + 5));
            r = set_cap(pn, num(i + 1) != 0, cap, r);
            i += 6;
        } else if (is(i, "customcap")) {
            int n = num(i + 9);
            GpPointF q[16];
            if (n < 0 || n > 16) { bad = 1; break; }
            for (int k = 0; k < n; k++) { q[k].X = fdec(i + 10 + 2 * k); q[k].Y = fdec(i + 11 + 2 * k); }
            void *path = NULL, *cap = NULL;
            int stroke = num(i + 2) != 0;
            r = GdipCreatePath(0, &path);
            if (!r) r = stroke ? GdipAddPathLine2(path, q, n) : GdipAddPathPolygon(path, q, n);
            if (!r) r = GdipCreateCustomLineCap(stroke ? NULL : path, stroke ? path : NULL, num(i + 3), fdec(i + 4), &cap);
            if (!r) r = GdipSetCustomLineCapWidthScale(cap, fdec(i + 5));
            if (!r) r = GdipSetCustomLineCapStrokeJoin(cap, num(i + 6));
            if (!r) r = GdipSetCustomLineCapStrokeCaps(cap, num(i + 7), num(i + 8));
            r = set_cap(pn, num(i + 1) != 0, cap, r);
            if (path) GdipDeletePath(path);
            i += 10 + 2 * n;
        } else { bad = 1; break; }
        out(" %d", r);
    }
    out("\n");
    if (!st) pen[p] = pn;
}

/* gpshape <slot> <pen|brush> <k> <matrix | -> <over|copy> <CompositingQuality> <SmoothingMode>
 * <PixelOffsetMode> <shape> <args>: a Graphics of the bitmap in those modes and that world
 * transform, then
 *   pen:   line x1 y1 x2 y2 | rect x y w h | ellipse x y w h | bezier 4 points | arc x y w h start
 *          sweep | pie x y w h start sweep | lines n points | polygon n points | curve tension n
 *          points | closedcurve tension n points
 *   brush: ellipse x y w h | pie x y w h start sweep | polygon FillMode n points | closedcurve
 *          tension FillMode n points
 * (floats as decimals): "shape 0 <st>" */
#define SHAPE_PTS 32
static void op_shape(void) {
    int s = slot(1);
    int usepen = is(2, "pen");
    if (!usepen && !is(2, "brush")) bad = 1;
    int k = slot(3);
    int m = is(4, "-") ? -1 : slot(4);
    int copy = is(5, "copy");
    if (!copy && !is(5, "over")) bad = 1;
    long q = num(6), sm = num(7), po = num(8);
    const char *shape = ntok > 9 ? tok[9] : "";
    float a[8] = { 0 };
    GpPointF pt[SHAPE_PTS];
    int n = 0, fixed = 0;
    float tension = 0;
    long fm = 0;
    int i = 10;
    if (!strcmp(shape, "line") || !strcmp(shape, "rect") || !strcmp(shape, "ellipse")) fixed = 4;
    else if (!strcmp(shape, "arc") || !strcmp(shape, "pie")) fixed = 6;
    else if (!strcmp(shape, "bezier")) fixed = 8;
    if (fixed) for (int j = 0; j < fixed; j++) a[j] = fdec(i + j);
    else {
        if (!strcmp(shape, "curve") || !strcmp(shape, "closedcurve")) tension = fdec(i++);
        if (!usepen && (!strcmp(shape, "polygon") || !strcmp(shape, "closedcurve"))) fm = num(i++);
        long c = num(i++);
        if (c < 0 || c > SHAPE_PTS) bad = 1;
        n = bad ? 0 : (int)c;
        for (int j = 0; j < n; j++) { pt[j].X = fdec(i + 2 * j); pt[j].Y = fdec(i + 2 * j + 1); }
    }
    static const char *const penshapes[] = { "line", "rect", "ellipse", "bezier", "arc", "pie", "lines", "polygon", "curve", "closedcurve", 0 };
    static const char *const brushshapes[] = { "ellipse", "pie", "polygon", "closedcurve", 0 };
    int known = 0;
    for (const char *const *e = usepen ? penshapes : brushshapes; *e; e++) known |= !strcmp(shape, *e);
    if (!known) bad = 1;
    if (bad || !gp() || !the(s)) return;
    if (m >= 0 && !mat[m]) { out("shape nomatrix\n"); return; }
    void *tool = usepen ? pen[k] : brush[k];
    if (!tool) { out("shape no%s\n", usepen ? "pen" : "brush"); return; }
    void *g = graphics(s, "shape");
    if (!g) return;
    GdipSetCompositingMode(g, copy);
    GpStatus st = GdipSetCompositingQuality(g, q);
    if (!st) st = GdipSetSmoothingMode(g, sm);
    if (!st) st = GdipSetPixelOffsetMode(g, po);
    if (!st && m >= 0) st = GdipSetWorldTransform(g, mat[m]);
    if (!st) {
#define SH(name) !strcmp(shape, name)
        if (usepen && SH("line")) st = GdipDrawLine(g, tool, a[0], a[1], a[2], a[3]);
        else if (usepen && SH("rect")) st = GdipDrawRectangle(g, tool, a[0], a[1], a[2], a[3]);
        else if (usepen && SH("ellipse")) st = GdipDrawEllipse(g, tool, a[0], a[1], a[2], a[3]);
        else if (usepen && SH("bezier")) st = GdipDrawBezier(g, tool, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
        else if (usepen && SH("arc")) st = GdipDrawArc(g, tool, a[0], a[1], a[2], a[3], a[4], a[5]);
        else if (usepen && SH("pie")) st = GdipDrawPie(g, tool, a[0], a[1], a[2], a[3], a[4], a[5]);
        else if (usepen && SH("lines")) st = GdipDrawLines(g, tool, pt, n);
        else if (usepen && SH("polygon")) st = GdipDrawPolygon(g, tool, pt, n);
        else if (usepen && SH("curve")) st = GdipDrawCurve2(g, tool, pt, n, tension);
        else if (usepen && SH("closedcurve")) st = GdipDrawClosedCurve2(g, tool, pt, n, tension);
        else if (!usepen && SH("ellipse")) st = GdipFillEllipse(g, tool, a[0], a[1], a[2], a[3]);
        else if (!usepen && SH("pie")) st = GdipFillPie(g, tool, a[0], a[1], a[2], a[3], a[4], a[5]);
        else if (!usepen && SH("polygon")) st = GdipFillPolygon(g, tool, pt, n, fm);
        else st = GdipFillClosedCurve2(g, tool, pt, n, tension, fm);
#undef SH
    }
    GdipDeleteGraphics(g);
    out("shape 0 %d\n", st);
}

/* paths in slots of their own:
 * gppath <p> <FillMode>: CreatePath: "path <st>"
 * gpadd <p> line x1 y1 x2 y2 | bezier (8) | rect x y w h | ellipse x y w h | arc x y w h start
 * sweep | pie (6) | lines <n> pts | beziers <n> pts | polygon <n> pts | curve <tension> <n> pts |
 * closedcurve <tension> <n> pts | close | start: AddPath*, ClosePathFigure, StartPathFigure (floats
 * as decimals): "add <st>"
 * gpflat <p> <matrix slot|-> <flatness>: FlattenPath: "flat <st>"
 * gpwiden <p> <pen> <matrix slot|-> <flatness>: WidenPath: "widen <st>"
 * gppathinfo <p>: "pathinfo <st> <count>" then "pathpts" and each point's type (2 hex digits) and
 * float bits "tt xxxxxxxx yyyyyyyy", at most 4096; then "bounds <st> x y w h" (GetPathWorldBounds,
 * no matrix, no pen)
 * gpfillpath <bitmap> <brush> <p> <over|copy> <CompositingQuality> <SmoothingMode>
 * <PixelOffsetMode>: FillPath: "fillpath 0 <st>" */
#define PATH_PTS 4096
static void *the_path(int p) {
    if (!path[p]) out("%s nopath\n", tok[0]);
    return path[p];
}
static void op_path(void) {
    int p = slot(1);
    long fm = num(2);
    if (bad || !gp()) return;
    if (path[p]) { GdipDeletePath(path[p]); path[p] = NULL; }
    out("path %d\n", GdipCreatePath(fm, &path[p]));
}
static void op_add(void) {
    int p = slot(1);
    const char *what = ntok > 2 ? tok[2] : "";
    float a[8] = { 0 };
    static GpPointF pt[64];
    int n = 0, fixed = -1;
    float tension = 0;
#define W(name) !strcmp(what, name)
    if (W("line") || W("rect") || W("ellipse")) fixed = 4;
    else if (W("arc") || W("pie")) fixed = 6;
    else if (W("bezier")) fixed = 8;
    else if (W("close") || W("start")) fixed = 0;
    else if (W("lines") || W("beziers") || W("polygon") || W("curve") || W("closedcurve")) {
        int i = 3;
        if (W("curve") || W("closedcurve")) tension = fdec(i++);
        long c = num(i++);
        if (c < 0 || c > 64) bad = 1;
        n = bad ? 0 : (int)c;
        for (int j = 0; j < n; j++) { pt[j].X = fdec(i + 2 * j); pt[j].Y = fdec(i + 2 * j + 1); }
    } else bad = 1;
    for (int j = 0; j < fixed; j++) a[j] = fdec(3 + j);
    if (bad || !gp() || !the_path(p)) return;
    void *q = path[p];
    GpStatus st;
    if (W("line")) st = GdipAddPathLine(q, a[0], a[1], a[2], a[3]);
    else if (W("rect")) st = GdipAddPathRectangle(q, a[0], a[1], a[2], a[3]);
    else if (W("ellipse")) st = GdipAddPathEllipse(q, a[0], a[1], a[2], a[3]);
    else if (W("arc")) st = GdipAddPathArc(q, a[0], a[1], a[2], a[3], a[4], a[5]);
    else if (W("pie")) st = GdipAddPathPie(q, a[0], a[1], a[2], a[3], a[4], a[5]);
    else if (W("bezier")) st = GdipAddPathBezier(q, a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
    else if (W("close")) st = GdipClosePathFigure(q);
    else if (W("start")) st = GdipStartPathFigure(q);
    else if (W("lines")) st = GdipAddPathLine2(q, pt, n);
    else if (W("beziers")) st = GdipAddPathBeziers(q, pt, n);
    else if (W("polygon")) st = GdipAddPathPolygon(q, pt, n);
    else if (W("curve")) st = GdipAddPathCurve2(q, pt, n, tension);
    else st = GdipAddPathClosedCurve2(q, pt, n, tension);
#undef W
    out("add %d\n", st);
}
static void op_flat(void) {
    int p = slot(1);
    int m = is(2, "-") ? -1 : slot(2);
    float f = fdec(3);
    if (bad || !gp() || !the_path(p)) return;
    if (m >= 0 && !mat[m]) { out("flat nomatrix\n"); return; }
    out("flat %d\n", GdipFlattenPath(path[p], m >= 0 ? mat[m] : NULL, f));
}
static void op_widen(void) {
    int p = slot(1), k = slot(2);
    int m = is(3, "-") ? -1 : slot(3);
    float f = fdec(4);
    if (bad || !gp() || !the_path(p)) return;
    if (m >= 0 && !mat[m]) { out("widen nomatrix\n"); return; }
    if (!pen[k]) { out("widen nopen\n"); return; }
    out("widen %d\n", GdipWidenPath(path[p], pen[k], m >= 0 ? mat[m] : NULL, f));
}
static void op_pathinfo(void) {
    int p = slot(1);
    if (bad || !gp() || !the_path(p)) return;
    INT n = 0;
    GpStatus st = GdipGetPointCount(path[p], &n);
    out("pathinfo %d %d\n", st, n);
    if (!st && n > 0 && n <= PATH_PTS) {
        static GpPointF pt[PATH_PTS];
        static BYTE ty[PATH_PTS];
        GpStatus a = GdipGetPathTypes(path[p], ty, n), b = GdipGetPathPoints(path[p], pt, n);
        out("pathpts %d %d", a, b);
        for (int i = 0; i < n && !a && !b; i++) out(" %02x %08lx %08lx", ty[i], bitsf(pt[i].X), bitsf(pt[i].Y));
        out("\n");
    }
    GpRectF r = { 0 };
    GpStatus bs = GdipGetPathWorldBounds(path[p], &r, NULL, NULL);
    out("bounds %d %08lx %08lx %08lx %08lx\n", bs, bitsf(r.X), bitsf(r.Y), bitsf(r.Width), bitsf(r.Height));
}
static void op_fillpath(void) {
    int s = slot(1), k = slot(2), p = slot(3);
    int copy = is(4, "copy");
    if (!copy && !is(4, "over")) bad = 1;
    long q = num(5), sm = num(6), po = num(7);
    if (bad || !gp() || !the(s) || !the_path(p)) return;
    if (!brush[k]) { out("fillpath nobrush\n"); return; }
    void *g = graphics(s, "fillpath");
    if (!g) return;
    GdipSetCompositingMode(g, copy);
    GpStatus st = GdipSetCompositingQuality(g, q);
    if (!st) st = GdipSetSmoothingMode(g, sm);
    if (!st) st = GdipSetPixelOffsetMode(g, po);
    if (!st) st = GdipFillPath(g, brush[k], path[p]);
    GdipDeleteGraphics(g);
    out("fillpath 0 %d\n", st);
}

static const struct kz_op gdiplus_ops[] = {
    { "gpbitmap", op_bitmap }, { "gpwrite", op_write }, { "gpread", op_read }, { "gpset", op_set },
    { "gpget", op_get }, { "gpinfo", op_info }, { "gpfill", op_fill }, { "gpclear", op_clear },
    { "gprotate", op_rotate }, { "gpclone", op_clone }, { "gpmatrix", op_matrix },
    { "gpmatmul", op_matmul }, { "gpmatinv", op_matinv }, { "gpmatpts", op_matpts },
    { "gpbrush", op_brush }, { "gppaint", op_paint }, { "gpattr", op_attr }, { "gpdraw", op_draw },
    { "gppen", op_pen }, { "gpshape", op_shape }, { "gpbrushinfo", op_brushinfo },
    { "gppath", op_path }, { "gpadd", op_add }, { "gpflat", op_flat }, { "gpwiden", op_widen },
    { "gppathinfo", op_pathinfo }, { "gpfillpath", op_fillpath },
    { 0 },
};
const struct kz_api gdiplus_api = { "gdiplus", gdiplus_ops, gdiplus_reset };
