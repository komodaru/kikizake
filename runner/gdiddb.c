#include "core.h"
#include "gdi.h"
/* device-dependent bitmaps in gdi.c's slots: CreateBitmap, CreateCompatibleBitmap, GetObject,
 * GetBitmapBits / SetBitmapBits (through gdi.c's dump / fill), GetDeviceCaps */

/* ddb <n> mono <w> <h> | bitmap <w> <h> <planes> <bpp> | compat <slot> <w> <h> | screen <w> <h>:
 * the bitmap selected into a new memory DC (compatible with the screen) in slot n */
static void op_ddb(void) {
    long n = num(1);
    if (n < 0 || n >= 8) { bad = 1; return; }
    HBITMAP b = NULL;
    if (is(2, "mono")) { int w = num(3), h = num(4); if (bad) return; b = CreateBitmap(w, h, 1, 1, NULL); }
    else if (is(2, "bitmap")) {
        int w = num(3), h = num(4), p = num(5), bpp = num(6);
        if (bad) return;
        b = CreateBitmap(w, h, p, bpp, NULL);
    } else if (is(2, "compat")) {
        HDC dc = gdi_dc(3);
        int w = num(4), h = num(5);
        if (bad) return;
        b = CreateCompatibleBitmap(dc, w, h);
    } else if (is(2, "screen")) {
        int w = num(3), h = num(4);
        if (bad) return;
        HDC sc = GetDC(NULL);
        b = CreateCompatibleBitmap(sc, w, h);
        ReleaseDC(NULL, sc);
    } else { bad = 1; return; }
    if (!b) { out("ddb failed\n"); return; }
    HDC dc = CreateCompatibleDC(NULL);
    HGDIOBJ old = SelectObject(dc, b);
    if (!old) { out("select failed\n"); DeleteObject(b); DeleteDC(dc); return; }
    gdi_adopt(n, dc, b, old);
}

/* the size GetObject gave, by what it is (so that x86 and x86-64 print the same) */
static const char *size_name(int k) {
    return k == sizeof(DIBSECTION) ? "dibsection" : k == sizeof(BITMAP) ? "bitmap" : k ? "other" : "0";
}

/* bmobject <s>: GetObject of the slot's bitmap: "bm <size> <w> <h> <widthbytes> <planes> <bpp>
 * <bits 0|1>", for a DIB section "dib <bitcount> <height> <compression> <clrused>", and
 * "bmsize <size>" (GetObject with no buffer) */
static void op_bmobject(void) {
    HBITMAP b = gdi_bitmap(1);
    if (bad) return;
    DIBSECTION ds;
    memset(&ds, 0, sizeof ds);
    int k = GetObjectA(b, sizeof ds, &ds);
    out("bm %s %ld %ld %ld %d %d %d\n", size_name(k), ds.dsBm.bmWidth, ds.dsBm.bmHeight, ds.dsBm.bmWidthBytes, ds.dsBm.bmPlanes, ds.dsBm.bmBitsPixel, ds.dsBm.bmBits ? 1 : 0);
    if (k == sizeof ds) out("dib %d %ld %lu %lu\n", ds.dsBmih.biBitCount, ds.dsBmih.biHeight, ds.dsBmih.biCompression, ds.dsBmih.biClrUsed);
    out("bmsize %s\n", size_name(GetObjectA(b, 0, NULL)));
}

/* bmbits <s>: GetBitmapBits of the whole bitmap ("bmbits <count>" then the bytes) */
static void op_bmbits(void) {
    HBITMAP b = gdi_bitmap(1);
    if (bad) return;
    BITMAP bm;
    GetObjectA(b, sizeof bm, &bm);
    int n = bm.bmWidthBytes * bm.bmHeight;
    unsigned char *p = calloc(1, n + 16);
    long k = GetBitmapBits(b, n + 16, p);
    out("bmbits %ld\n", k);
    if (k > 0) out_hex(p, k);
    free(p);
}

/* devcaps <s> <index>...: GetDeviceCaps */
static void op_devcaps(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    out("caps");
    for (int i = 2; i < ntok; i++) { int k = num(i); if (bad) return; out(" %d", GetDeviceCaps(dc, k)); }
    out("\n");
}

/* objtype <s> pen|brush|font|bitmap: GetObjectType of the DC's current object */
static void op_curobj(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    UINT t = is(2, "pen") ? OBJ_PEN : is(2, "brush") ? OBJ_BRUSH : is(2, "font") ? OBJ_FONT : is(2, "bitmap") ? OBJ_BITMAP : 0;
    if (!t) { bad = 1; return; }
    out("objtype %lu\n", GetObjectType(GetCurrentObject(dc, t)));
}

static const struct kz_op gdiddb_ops[] = {
    { "ddb", op_ddb },
    { "bmobject", op_bmobject },
    { "bmbits", op_bmbits },
    { "devcaps", op_devcaps },
    { "objtype", op_curobj },
    { 0 },
};
const struct kz_api gdiddb_api = { "gdiddb", gdiddb_ops, NULL };
