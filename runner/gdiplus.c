#include "core.h"
/* GDI+ (gdiplus.dll, loaded and started by the first case that asks for it; its flat API through
 * GetProcAddress, declared here: the C headers of GDI+ are C++). Bitmaps in two slots, their
 * pixels through LockBits in any format, GetPixel / SetPixel, palettes, fills and clears through a
 * Graphics of the bitmap, RotateFlip, CloneBitmapArea, and matrices.
 *
 * Every op prints the Status it got (decimal, the Status enum of gdiplustypes.h) and, when that
 * is Ok, what it read. */

typedef int GpStatus;
typedef struct { UINT Width, Height; INT Stride; INT PixelFormat; void *Scan0; UINT_PTR Reserved; } GpBitmapData;
typedef struct { INT X, Y, Width, Height; } GpRect;
typedef struct { float X, Y; } GpPointF;
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
    X(GdipTransformMatrixPoints, (void *, GpPointF *, INT))

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

static void gdiplus_reset(void) {
    for (int i = 0; i < SLOTS; i++) {
        if (bmp[i]) { GdipDisposeImage(bmp[i]); bmp[i] = NULL; }
        if (mat[i]) { GdipDeleteMatrix(mat[i]); mat[i] = NULL; }
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
 * gpmatpts <slot> <x> <y>...: TransformMatrixPoints: "pts <st> <x> <y>..."
 * (a last word "x86" is ignored: the model's mark for x86's rounding, suite gdiplusmat) */
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
    int nt = ntok - is(ntok - 1, "x86"); /* (a last "x86" marks the cases of an x86-only suite) */
    int n = (nt - 2) / 2;
    if ((nt - 2) % 2 || n < 1 || n > 16) bad = 1;
    for (int i = 0; i < n && !bad; i++) { p[i].X = fbits(2 + 2 * i); p[i].Y = fbits(3 + 2 * i); }
    if (bad || !gp() || !the_matrix(s)) return;
    GpStatus st = GdipTransformMatrixPoints(mat[s], p, n);
    out("pts %d", st);
    for (int i = 0; i < n && !st; i++) out(" %08lx %08lx", bitsf(p[i].X), bitsf(p[i].Y));
    out("\n");
}

static const struct kz_op gdiplus_ops[] = {
    { "gpbitmap", op_bitmap }, { "gpwrite", op_write }, { "gpread", op_read }, { "gpset", op_set },
    { "gpget", op_get }, { "gpinfo", op_info }, { "gpfill", op_fill }, { "gpclear", op_clear },
    { "gprotate", op_rotate }, { "gpclone", op_clone }, { "gpmatrix", op_matrix },
    { "gpmatmul", op_matmul }, { "gpmatinv", op_matinv }, { "gpmatpts", op_matpts },
    { 0 },
};
const struct kz_api gdiplus_api = { "gdiplus", gdiplus_ops, gdiplus_reset };
