#include "core.h"
#include "gdi.h"
/* GDI regions in slots of their own (0..7), and the clip region of gdi.c's slots */

#define RGNS 8
static HRGN rg[RGNS];

HRGN *rgn_slot(int i) {
    long n = num(i);
    if (n < 0 || n >= RGNS) { bad = 1; return NULL; }
    return &rg[n];
}
static HRGN get(int i) {
    HRGN *r = rgn_slot(i);
    if (r && !*r) bad = 1;
    return r ? *r : NULL;
}
static void put(HRGN *r, HRGN n) {
    if (*r) DeleteObject(*r);
    *r = n;
}

/* rgn <r> rect l t r b | ellipse l t r b | round l t r b w h | poly <mode> x y ... | null */
static void op_rgn(void) {
    HRGN *r = rgn_slot(1);
    if (bad) return;
    HRGN n = NULL;
    if (is(2, "null")) n = CreateRectRgn(0, 0, 0, 0);
    else if (is(2, "rect") || is(2, "ellipse")) {
        int a = num(3), b = num(4), c = num(5), d = num(6);
        if (bad) return;
        n = is(2, "rect") ? CreateRectRgn(a, b, c, d) : CreateEllipticRgn(a, b, c, d);
    } else if (is(2, "round")) {
        int a = num(3), b = num(4), c = num(5), d = num(6), w = num(7), h = num(8);
        if (bad) return;
        n = CreateRoundRectRgn(a, b, c, d, w, h);
    } else if (is(2, "poly")) {
        POINT p[64];
        int mode = num(3), k = 0;
        for (int i = 4; i + 1 < ntok && k < 64; i += 2, k++) { p[k].x = num(i); p[k].y = num(i + 1); }
        if (bad || (ntok - 4) % 2) { bad = 1; return; }
        n = CreatePolygonRgn(p, k, mode);
    } else { bad = 1; return; }
    if (!n) out("rgn failed\n");
    put(r, n);
}

/* setrect <r> l t r b */
static void op_setrect(void) {
    HRGN r = get(1);
    int a = num(2), b = num(3), c = num(4), d = num(5);
    if (!bad) out("setrect %d\n", SetRectRgn(r, a, b, c, d) ? 1 : 0);
}

/* combine <dst> <a> <b> <mode> (dst made empty first when it has none) */
static void op_combine(void) {
    HRGN *d = rgn_slot(1);
    HRGN a = get(2), b = get(3);
    int m = num(4);
    if (bad) return;
    if (!*d) *d = CreateRectRgn(0, 0, 0, 0);
    out("combine %d\n", CombineRgn(*d, a, m == RGN_COPY ? a : b, m));
}

static void op_query(void) {
    HRGN r = get(1);
    if (bad) return;
    if (is(0, "offsetrgn")) { int x = num(2), y = num(3); if (!bad) out("offset %d\n", OffsetRgn(r, x, y)); }
    else if (is(0, "rgnbox")) { RECT b; int t = GetRgnBox(r, &b); out("box %d %ld %ld %ld %ld\n", t, b.left, b.top, b.right, b.bottom); }
    else if (is(0, "ptin")) { int x = num(2), y = num(3); if (!bad) out("ptin %d\n", PtInRegion(r, x, y) ? 1 : 0); }
    else if (is(0, "rectin")) {
        RECT b = { num(2), num(3), num(4), num(5) };
        if (!bad) out("rectin %d\n", RectInRegion(r, &b) ? 1 : 0);
    } else if (is(0, "equal")) { HRGN o = get(2); if (!bad) out("equal %d\n", EqualRgn(r, o)); }
    else if (is(0, "rgndata")) {
        /* "rgndata <count> <bound l t r b>" then "rects l t r b ..." */
        DWORD n = GetRegionData(r, 0, NULL);
        RGNDATA *d = calloc(1, n);
        GetRegionData(r, n, d);
        RECT *b = &d->rdh.rcBound, *k = (RECT *)d->Buffer;
        out("rgndata %lu %ld %ld %ld %ld\nrects", d->rdh.nCount, b->left, b->top, b->right, b->bottom);
        for (DWORD i = 0; i < d->rdh.nCount; i++) out(" %ld %ld %ld %ld", k[i].left, k[i].top, k[i].right, k[i].bottom);
        out("\n");
        free(d);
    }
}

/* fillrgn <s> <r> (the slot's brush), framergn <s> <r> <w> <h>, invertrgn <s> <r>, paintrgn <s> <r> */
static void op_draw(void) {
    HDC dc = gdi_dc(1);
    HRGN r = get(2);
    if (bad) return;
    HBRUSH b = (HBRUSH)GetCurrentObject(dc, OBJ_BRUSH);
    BOOL k;
    if (is(0, "fillrgn")) k = FillRgn(dc, r, b);
    else if (is(0, "framergn")) { int w = num(3), h = num(4); if (bad) return; k = FrameRgn(dc, r, b, w, h); }
    else if (is(0, "invertrgn")) k = InvertRgn(dc, r);
    else k = PaintRgn(dc, r);
    out("ret %d\n", k ? 1 : 0);
}

/* selclip <s> <r>|none [mode]: ExtSelectClipRgn */
static void op_selclip(void) {
    HDC dc = gdi_dc(1);
    HRGN r = is(2, "none") ? NULL : get(2);
    int m = ntok > 3 ? num(3) : RGN_COPY;
    if (!bad) out("selclip %d\n", ExtSelectClipRgn(dc, r, m));
}

static void op_clipq(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    if (is(0, "clipbox")) { RECT b; int t = GetClipBox(dc, &b); out("clipbox %d %ld %ld %ld %ld\n", t, b.left, b.top, b.right, b.bottom); }
    else if (is(0, "getcliprgn")) {
        HRGN *r = rgn_slot(2);
        if (bad) return;
        if (!*r) *r = CreateRectRgn(0, 0, 0, 0);
        out("getcliprgn %d\n", GetClipRgn(dc, *r));
    } else if (is(0, "exclude")) {
        int a = num(2), b = num(3), c = num(4), d = num(5);
        if (!bad) out("exclude %d\n", ExcludeClipRect(dc, a, b, c, d));
    } else if (is(0, "offsetclip")) { int x = num(2), y = num(3); if (!bad) out("offsetclip %d\n", OffsetClipRgn(dc, x, y)); }
    else if (is(0, "ptvisible")) { int x = num(2), y = num(3); if (!bad) out("ptvisible %d\n", PtVisible(dc, x, y)); }
    else if (is(0, "rectvisible")) {
        RECT b = { num(2), num(3), num(4), num(5) };
        if (!bad) out("rectvisible %d\n", RectVisible(dc, &b));
    }
}

static void gdirgn_reset(void) {
    for (int i = 0; i < RGNS; i++) put(&rg[i], NULL);
}

static const struct kz_op gdirgn_ops[] = {
    { "rgn", op_rgn },
    { "setrect", op_setrect },
    { "combine", op_combine },
    { "offsetrgn", op_query },
    { "rgnbox", op_query },
    { "ptin", op_query },
    { "rectin", op_query },
    { "equal", op_query },
    { "rgndata", op_query },
    { "fillrgn", op_draw },
    { "framergn", op_draw },
    { "invertrgn", op_draw },
    { "paintrgn", op_draw },
    { "selclip", op_selclip },
    { "clipbox", op_clipq },
    { "getcliprgn", op_clipq },
    { "exclude", op_clipq },
    { "offsetclip", op_clipq },
    { "ptvisible", op_clipq },
    { "rectvisible", op_clipq },
    { 0 },
};
const struct kz_api gdirgn_api = { "gdirgn", gdirgn_ops, gdirgn_reset };
