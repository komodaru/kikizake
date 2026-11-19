#include "core.h"
#include "gdi.h"
/* GDI: surfaces in slots, each a DIB section selected into its own memory DC */

#define SLOTS 8
static struct slot {
    HDC dc;
    HBITMAP bmp, oldbmp;
    unsigned char *bits;
    int w, h, bpp, stride, topdown;
    RGBQUAD pal[256];
    HBRUSH brush;
    HGDIOBJ oldbrush;
} sl[SLOTS];

static struct slot *slot(int i) {
    long n = num(i);
    if (n < 0 || n >= SLOTS || !sl[n].dc) { bad = 1; return NULL; }
    return &sl[n];
}

/* the default color tables (SPEC.md "dib") */
static void default_palette(int bpp, RGBQUAD *p) {
    static const unsigned vga[16] = { 0x000000, 0x800000, 0x008000, 0x808000, 0x000080, 0x800080, 0x008080, 0xc0c0c0,
                                      0x808080, 0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff };
    int n = 1 << bpp;
    for (int i = 0; i < n; i++) {
        unsigned c;
        if (bpp == 1) c = i ? 0xffffff : 0;
        else if (bpp == 4) c = vga[i];
        else c = ((i >> 5 & 7) * 255 / 7) << 16 | ((i >> 2 & 7) * 255 / 7) << 8 | (i & 3) * 255 / 3;
        p[i].rgbRed = c >> 16; p[i].rgbGreen = c >> 8 & 255; p[i].rgbBlue = c & 255; p[i].rgbReserved = 0;
    }
}

HDC gdi_dc(int i) { struct slot *s = slot(i); return s ? s->dc : NULL; }
HBITMAP gdi_bitmap(int i) { struct slot *s = slot(i); return s ? s->bmp : NULL; }

static void free_slot(struct slot *s);
/* a bitmap made elsewhere (gdiddb.c): a DIB section keeps its bits, a DDB is read and written
 * through GetBitmapBits / SetBitmapBits (bits NULL) */
void gdi_adopt(long n, HDC dc, HBITMAP bmp, HGDIOBJ oldbmp) {
    struct slot *s = &sl[n];
    free_slot(s);
    DIBSECTION ds;
    memset(&ds, 0, sizeof(ds));
    int k = GetObjectA(bmp, sizeof(ds), &ds);
    s->dc = dc; s->bmp = bmp; s->oldbmp = oldbmp;
    s->w = ds.dsBm.bmWidth; s->h = ds.dsBm.bmHeight; s->bpp = ds.dsBm.bmBitsPixel * ds.dsBm.bmPlanes;
    s->stride = ds.dsBm.bmWidthBytes;
    s->topdown = 1;
    if (k == sizeof(ds)) {
        s->bits = ds.dsBm.bmBits;
        s->topdown = ds.dsBmih.biHeight < 0;
        if (s->bpp <= 8) {
            GetDIBColorTable(dc, 0, 1 << s->bpp, s->pal);
        }
    }
}

static void free_slot(struct slot *s) {
    if (!s->dc) return;
    if (s->brush) { SelectObject(s->dc, s->oldbrush); DeleteObject(s->brush); }
    SelectObject(s->dc, s->oldbmp);
    DeleteObject(s->bmp);
    DeleteDC(s->dc);
    memset(s, 0, sizeof(*s));
}

/* dib <slot> <w> <h> <bpp> [td|bu] [565] [#c0 #c1 ...] */
static void op_dib(void) {
    long n = num(1);
    if (n < 0 || n >= SLOTS) { bad = 1; return; }
    struct slot *s = &sl[n];
    free_slot(s);
    int w = num(2), h = num(3), bpp = num(4), i = 5, f565 = 0;
    s->topdown = 1;
    if (is(i, "td")) i++;
    else if (is(i, "bu")) { s->topdown = 0; i++; }
    if (is(i, "565")) { f565 = 1; i++; }
    if (bad) return;
    struct { BITMAPINFOHEADER h; union { RGBQUAD pal[256]; DWORD masks[3]; } u; } bi;
    memset(&bi, 0, sizeof(bi));
    bi.h.biSize = sizeof(bi.h);
    bi.h.biWidth = w;
    bi.h.biHeight = s->topdown ? -h : h;
    bi.h.biPlanes = 1;
    bi.h.biBitCount = bpp;
    bi.h.biCompression = BI_RGB;
    if (bpp <= 8) {
        default_palette(bpp, s->pal);
        for (int k = 0; i < ntok && k < (1 << bpp); i++, k++) {
            COLORREF c = color(i);
            s->pal[k].rgbRed = GetRValue(c); s->pal[k].rgbGreen = GetGValue(c); s->pal[k].rgbBlue = GetBValue(c);
        }
        memcpy(bi.u.pal, s->pal, sizeof(RGBQUAD) << bpp);
        bi.h.biClrUsed = 1 << bpp;
    } else if (f565) {
        bi.h.biCompression = BI_BITFIELDS;
        bi.u.masks[0] = 0xf800; bi.u.masks[1] = 0x07e0; bi.u.masks[2] = 0x001f;
    }
    s->dc = CreateCompatibleDC(NULL);
    /* (cleared: what CreateDIBSection leaves, not what the C runtime left before; x86 mingw's
     * runtime leaves 126, so "dib failed" printed 126 on x86 and 0 on x86-64, shirusu calls/) */
    SetLastError(0);
    s->bmp = CreateDIBSection(s->dc, (BITMAPINFO *)&bi, DIB_RGB_COLORS, (void **)&s->bits, NULL, 0);
    if (!s->bmp) { out("dib failed %lu\n", GetLastError()); DeleteDC(s->dc); s->dc = NULL; return; }
    s->oldbmp = SelectObject(s->dc, s->bmp);
    s->w = w; s->h = h; s->bpp = bpp; s->stride = ((w * bpp + 31) / 32) * 4;
    memset(s->bits, 0, s->stride * h);
}

/* fill <slot> <seed>: every byte of the pixel buffer from xorshift32 (SPEC.md) */
static void op_fill(void) {
    struct slot *s = slot(1);
    unsigned x = (unsigned)num(2);
    if (bad) return;
    if (!x) x = 0x9e3779b9;
    GdiFlush();
    unsigned char *b = s->bits ? s->bits : malloc(s->stride * s->h);
    for (int i = 0; i < s->stride * s->h; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        b[i] = (unsigned char)x;
    }
    if (!s->bits) { SetBitmapBits(s->bmp, s->stride * s->h, b); free(b); }
}

/* the row of the picture's top-to-bottom y in memory */
static unsigned char *row(struct slot *s, int y) { return s->bits + (s->topdown ? y : s->h - 1 - y) * s->stride; }

/* dump <slot>: "px <slot> <w> <h> <bpp>" then each row's bytes (only those of the w pixels; a
 * partial last byte as it is in memory), top to bottom */
static void op_dump(void) {
    struct slot *s = slot(1);
    if (bad) return;
    GdiFlush();
    out("px %ld %d %d %d\n", num(1), s->w, s->h, s->bpp);
    int bytes = (s->w * s->bpp + 7) / 8;
    if (!s->bits) {
        /* a DDB: its bits as GetBitmapBits gives them (rows top to bottom, WORD aligned) */
        unsigned char *b = calloc(1, s->stride * s->h);
        GetBitmapBits(s->bmp, s->stride * s->h, b);
        for (int y = 0; y < s->h; y++) out_hex(b + y * s->stride, bytes);
        free(b);
        return;
    }
    for (int y = 0; y < s->h; y++) {
        unsigned char *p = row(s, y);
        out_hex(p, bytes);
    }
}

/* poke <slot> <x> <y> <hex bytes>: raw bytes at a pixel (bpp >= 8) */
static void op_poke(void) {
    struct slot *s = slot(1);
    int x = num(2), y = num(3);
    if (bad || !s->bits || s->bpp < 8 || x < 0 || y < 0 || x >= s->w || y >= s->h || ntok < 5) { bad = 1; return; }
    GdiFlush();
    unsigned char *p = row(s, y) + x * (s->bpp / 8);
    for (int i = 0; tok[4][2 * i] && tok[4][2 * i + 1] && i < s->bpp / 8; i++) {
        char b[3] = { tok[4][2 * i], tok[4][2 * i + 1], 0 };
        p[i] = (unsigned char)strtoul(b, NULL, 16);
    }
}

static void set_brush(struct slot *s, HBRUSH b) {
    if (!b) { out("brush failed\n"); return; }
    HGDIOBJ o = SelectObject(s->dc, b);
    if (s->brush) DeleteObject(s->brush);
    else s->oldbrush = o;
    s->brush = b;
}

/* brush <slot> solid #c | hatch <style> #c | dib <src slot> | null */
static void op_brush(void) {
    struct slot *s = slot(1);
    if (bad) return;
    if (is(2, "solid")) {
        COLORREF c = color(3);
        if (!bad) set_brush(s, CreateSolidBrush(c));
    } else if (is(2, "hatch")) {
        int st = num(3);
        COLORREF c = color(4);
        if (!bad) set_brush(s, CreateHatchBrush(st, c));
    } else if (is(2, "dib")) {
        struct slot *p = slot(3);
        if (bad || !p->bits) { bad = 1; return; }
        /* a packed DIB of the slot's picture: header, color table, bits */
        int ncol = p->bpp <= 8 ? 1 << p->bpp : 0;
        int size = sizeof(BITMAPINFOHEADER) + ncol * 4 + p->stride * p->h;
        BITMAPINFOHEADER *bh = calloc(1, size);
        bh->biSize = sizeof(*bh);
        bh->biWidth = p->w;
        bh->biHeight = p->topdown ? -p->h : p->h;
        bh->biPlanes = 1;
        bh->biBitCount = p->bpp;
        bh->biCompression = BI_RGB;
        bh->biClrUsed = ncol;
        GdiFlush();
        memcpy((char *)(bh + 1), p->pal, ncol * 4);
        GdiFlush();
        memcpy((char *)(bh + 1) + ncol * 4, p->bits, p->stride * p->h);
        set_brush(s, CreateDIBPatternBrushPt(bh, DIB_RGB_COLORS));
        free(bh);
    } else if (is(2, "null")) {
        set_brush(s, (HBRUSH)GetStockObject(NULL_BRUSH));
        s->brush = NULL; /* (stock: never deleted) */
    } else bad = 1;
}

static void op_patblt(void) {
    struct slot *s = slot(1);
    int x = num(2), y = num(3), w = num(4), h = num(5);
    DWORD rop = (DWORD)num(6);
    if (bad) return;
    BOOL r = PatBlt(s->dc, x, y, w, h, rop);
    out("ret %d\n", r ? 1 : 0);
}

static void op_bitblt(void) {
    struct slot *d = slot(1);
    int x = num(2), y = num(3), w = num(4), h = num(5);
    struct slot *s = slot(6);
    int sx = num(7), sy = num(8);
    DWORD rop = (DWORD)num(9);
    if (bad) return;
    BOOL r = BitBlt(d->dc, x, y, w, h, s->dc, sx, sy, rop);
    out("ret %d\n", r ? 1 : 0);
}

/* stretchblt <d> <x> <y> <w> <h> <s> <sx> <sy> <sw> <sh> <rop> */
static void op_stretchblt(void) {
    struct slot *d = slot(1);
    int x = num(2), y = num(3), w = num(4), h = num(5);
    struct slot *s = slot(6);
    int sx = num(7), sy = num(8), sw = num(9), sh = num(10);
    DWORD rop = (DWORD)num(11);
    if (bad) return;
    BOOL r = StretchBlt(d->dc, x, y, w, h, s->dc, sx, sy, sw, sh, rop);
    out("ret %d\n", r ? 1 : 0);
}

static void print_color(const char *what, COLORREF c) {
    if (c == CLR_INVALID) out("%s invalid\n", what);
    else out("%s %02x%02x%02x\n", what, GetRValue(c), GetGValue(c), GetBValue(c));
}

/* a top-down BITMAPINFO of w x h at <bpp> [565] (color table: the default one, SPEC.md) */
typedef struct { BITMAPINFOHEADER h; union { RGBQUAD pal[256]; DWORD masks[3]; } u; } BI;
static void make_bi(BI *bi, int w, int h, int bpp, int f565) {
    memset(bi, 0, sizeof(*bi));
    bi->h.biSize = sizeof(bi->h);
    bi->h.biWidth = w;
    bi->h.biHeight = -h;
    bi->h.biPlanes = 1;
    bi->h.biBitCount = bpp;
    bi->h.biCompression = f565 ? BI_BITFIELDS : BI_RGB;
    if (f565) { bi->u.masks[0] = 0xf800; bi->u.masks[1] = 0x07e0; bi->u.masks[2] = 0x001f; }
    if (bpp <= 8) { default_palette(bpp, bi->u.pal); bi->h.biClrUsed = 1 << bpp; }
}

/* getdibits <s> <bpp> [565]: GetDIBits of the slot's bitmap (deselected meanwhile) as a
 * top-down DIB of that format: "dibits <lines>", for bpp <= 8 "pal" and the table GetDIBits
 * wrote, then the rows as dump prints them */
static void op_getdibits(void) {
    struct slot *s = slot(1);
    int bpp = num(2), f565 = is(3, "565");
    if (bad) return;
    BI bi;
    make_bi(&bi, s->w, s->h, bpp, f565);
    int stride = ((s->w * bpp + 31) / 32) * 4;
    unsigned char *buf = calloc(1, stride * s->h);
    SelectObject(s->dc, s->oldbmp);
    int n = GetDIBits(s->dc, s->bmp, 0, s->h, buf, (BITMAPINFO *)&bi, DIB_RGB_COLORS);
    SelectObject(s->dc, s->bmp);
    out("dibits %d\n", n);
    if (bpp <= 8) {
        out("pal");
        for (int i = 0; i < (1 << bpp); i++) out(" %02x%02x%02x", bi.u.pal[i].rgbRed, bi.u.pal[i].rgbGreen, bi.u.pal[i].rgbBlue);
        out("\n");
    }
    int bytes = (s->w * bpp + 7) / 8;
    for (int y = 0; y < s->h && n > 0; y++) {
        for (int i = 0; i < bytes; i++) out("%02x", buf[y * stride + i]);
        out("\n");
    }
    free(buf);
}

/* setdibits <s> <bpp> [565] <seed>: SetDIBits of a top-down DIB of that format (default color
 * table), its bytes from xorshift32 like fill, onto the slot's bitmap (deselected meanwhile) */
static void op_setdibits(void) {
    struct slot *s = slot(1);
    int bpp = num(2), f565 = is(3, "565");
    unsigned x = (unsigned)num(f565 ? 4 : 3);
    if (bad) return;
    BI bi;
    make_bi(&bi, s->w, s->h, bpp, f565);
    int stride = ((s->w * bpp + 31) / 32) * 4;
    unsigned char *buf = calloc(1, stride * s->h);
    if (!x) x = 0x9e3779b9;
    for (int i = 0; i < stride * s->h; i++) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; buf[i] = (unsigned char)x; }
    SelectObject(s->dc, s->oldbmp);
    int n = SetDIBits(s->dc, s->bmp, 0, s->h, buf, (BITMAPINFO *)&bi, DIB_RGB_COLORS);
    SelectObject(s->dc, s->bmp);
    out("setdibits %d\n", n);
    free(buf);
}

static void op_pixel(void) {
    struct slot *s = slot(1);
    int x = num(2), y = num(3);
    if (bad) return;
    if (is(0, "getpixel")) print_color("pixel", GetPixel(s->dc, x, y));
    else { COLORREF c = color(4); if (!bad) print_color("pixel", SetPixel(s->dc, x, y, c)); }
}

static void op_dcstate(void) {
    struct slot *s = slot(1);
    if (bad) return;
    if (is(0, "text")) { COLORREF c = color(2); if (!bad) SetTextColor(s->dc, c); }
    else if (is(0, "bk")) { COLORREF c = color(2); if (!bad) SetBkColor(s->dc, c); }
    else if (is(0, "bkmode")) { int m = num(2); if (!bad) SetBkMode(s->dc, m); }
    else if (is(0, "org")) { int x = num(2), y = num(3); if (!bad) SetBrushOrgEx(s->dc, x, y, NULL); }
    else if (is(0, "clip")) {
        int a = num(2), b = num(3), c = num(4), d = num(5);
        if (!bad) out("clip %d\n", IntersectClipRect(s->dc, a, b, c, d));
    } else if (is(0, "noclip")) SelectClipRgn(s->dc, NULL);
    else if (is(0, "mode")) { int m = num(2); if (!bad) SetStretchBltMode(s->dc, m); }
}

static void gdi_reset(void) {
    for (int i = 0; i < SLOTS; i++) free_slot(&sl[i]);
}

static const struct kz_op gdi_ops[] = {
    { "dib", op_dib },
    { "fill", op_fill },
    { "poke", op_poke },
    { "dump", op_dump },
    { "brush", op_brush },
    { "patblt", op_patblt },
    { "bitblt", op_bitblt },
    { "stretchblt", op_stretchblt },
    { "getdibits", op_getdibits },
    { "setdibits", op_setdibits },
    { "getpixel", op_pixel },
    { "setpixel", op_pixel },
    { "text", op_dcstate },
    { "bk", op_dcstate },
    { "bkmode", op_dcstate },
    { "org", op_dcstate },
    { "clip", op_dcstate },
    { "noclip", op_dcstate },
    { "mode", op_dcstate },
    { 0 },
};
const struct kz_api gdi_api = { "gdi", gdi_ops, gdi_reset };
