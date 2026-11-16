#include "core.h"
#include "gdi.h"
/* GDI fonts and text in gdi.c's slots. A text token is ASCII with '_' for a space. */

static HGDIOBJ made[64];
static int nmade;
static HANDLE mem[16];
static int nmem;

/* text of token i ('_' -> ' '), in a static buffer */
static char *txt(int i) {
    static char b[256];
    if (i >= ntok) { bad = 1; return b; }
    int k = 0;
    for (const char *p = tok[i]; *p && k < 255; p++) b[k++] = *p == '_' ? ' ' : *p;
    b[k] = 0;
    return b;
}

/* font <s> <height> <width> <escapement> <weight> <italic> <quality> <face...> (the face's
 * words as tokens; CreateFontA, DEFAULT_CHARSET) */
static void op_font(void) {
    HDC dc = gdi_dc(1);
    int h = num(2), w = num(3), e = num(4), wt = num(5), it = num(6), q = num(7);
    if (bad || ntok < 9) { bad = 1; return; }
    char face[64] = "";
    for (int i = 8; i < ntok; i++) {
        if (i > 8) strcat(face, " ");
        strncat(face, tok[i], 60 - strlen(face));
    }
    HFONT f = CreateFontA(h, w, e, e, wt, it, 0, 0, DEFAULT_CHARSET, OUT_DEFAULT_PRECIS, CLIP_DEFAULT_PRECIS, q, DEFAULT_PITCH, face);
    if (!f) { out("font failed\n"); return; }
    if (nmade < 64) made[nmade++] = f;
    SelectObject(dc, f);
}

static void op_metrics(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    TEXTMETRICA t;
    BOOL r = GetTextMetricsA(dc, &t);
    out("tm %d h %ld asc %ld desc %ld int %ld ext %ld ave %ld max %ld weight %ld over %ld dig %ld %ld chars %d %d %d %d it %d ul %d st %d pitch %d charset %d\n", r ? 1 : 0,
        t.tmHeight, t.tmAscent, t.tmDescent, t.tmInternalLeading, t.tmExternalLeading, t.tmAveCharWidth, t.tmMaxCharWidth, t.tmWeight, t.tmOverhang,
        t.tmDigitizedAspectX, t.tmDigitizedAspectY, t.tmFirstChar, t.tmLastChar, t.tmDefaultChar, t.tmBreakChar, t.tmItalic, t.tmUnderlined, t.tmStruckOut, t.tmPitchAndFamily, t.tmCharSet);
}

static void op_face(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    char b[64] = "";
    int n = GetTextFaceA(dc, sizeof b, b);
    for (char *p = b; *p; p++) if (*p == ' ') *p = '_';
    out("face %d %s\n", n, b);
}

static void op_extent(void) {
    HDC dc = gdi_dc(1);
    char *s = txt(2);
    if (bad) return;
    SIZE z = { -1, -1 };
    BOOL r = GetTextExtentPoint32A(dc, s, strlen(s), &z);
    out("extent %d %ld %ld\n", r ? 1 : 0, z.cx, z.cy);
}

/* extentex <s> <max> <text>: GetTextExtentExPoint ("extentex <ret> <fit> <cx> <cy> dx...") */
static void op_extentex(void) {
    HDC dc = gdi_dc(1);
    int m = num(2);
    char *s = txt(3);
    if (bad) return;
    int fit = -1, dx[256];
    SIZE z = { -1, -1 };
    int n = strlen(s);
    BOOL r = GetTextExtentExPointA(dc, s, n, m, &fit, dx, &z);
    out("extentex %d %d %ld %ld", r ? 1 : 0, fit, z.cx, z.cy);
    for (int i = 0; i < n && i < fit; i++) out(" %d", dx[i]);
    out("\n");
}

/* widths / abc <s> <first> <last> */
static void op_widths(void) {
    HDC dc = gdi_dc(1);
    int a = num(2), b = num(3);
    if (bad || b < a || b - a > 255) { bad = 1; return; }
    if (is(0, "widths")) {
        INT w[256];
        BOOL r = GetCharWidth32A(dc, a, b, w);
        out("widths %d", r ? 1 : 0);
        for (int i = 0; r && i <= b - a; i++) out(" %d", w[i]);
    } else {
        ABC w[256];
        BOOL r = GetCharABCWidthsA(dc, a, b, w);
        out("abc %d", r ? 1 : 0);
        for (int i = 0; r && i <= b - a; i++) out(" %d,%u,%d", w[i].abcA, w[i].abcB, w[i].abcC);
    }
    out("\n");
}

static void op_textout(void) {
    HDC dc = gdi_dc(1);
    int x = num(2), y = num(3);
    char *s = txt(4);
    if (!bad) out("ret %d\n", TextOutA(dc, x, y, s, strlen(s)) ? 1 : 0);
}

/* exttext <s> <x> <y> <options> <l t r b> <text> */
static void op_exttext(void) {
    HDC dc = gdi_dc(1);
    int x = num(2), y = num(3), o = num(4);
    RECT rc = { num(5), num(6), num(7), num(8) };
    char *s = ntok > 9 ? txt(9) : "";
    if (!bad) out("ret %d\n", ExtTextOutA(dc, x, y, o, &rc, s, strlen(s), NULL) ? 1 : 0);
}

/* align <s> <v>, extra <s> <n>: the old value */
static void op_set(void) {
    HDC dc = gdi_dc(1);
    int v = num(2);
    if (bad) return;
    if (is(0, "align")) out("align %u\n", SetTextAlign(dc, v));
    else out("extra %d\n", SetTextCharacterExtra(dc, v));
}

/* glyph <s> <char> <format>: GetGlyphOutlineA with the identity matrix: "glyph <size> <bw>
 * <bh> <ox> <oy> <incx> <incy>" and the buffer as hex (GGO_METRICS: size 1 for any success, no buffer) */
static void op_glyph(void) {
    HDC dc = gdi_dc(1);
    int ch = num(2), f = num(3);
    if (bad) return;
    GLYPHMETRICS gm;
    memset(&gm, 0, sizeof gm);
    MAT2 m = { { 0, 1 }, { 0, 0 }, { 0, 0 }, { 0, 1 } };
    DWORD n = GetGlyphOutlineA(dc, ch, f, &gm, 0, NULL, &m);
    /* GGO_METRICS: only success is documented ("greater than zero"), not the value */
    long r = n == GDI_ERROR ? -1 : f == GGO_METRICS && n > 0 ? 1 : (long)n;
    out("glyph %ld %u %u %ld %ld %d %d\n", r, gm.gmBlackBoxX, gm.gmBlackBoxY, gm.gmptGlyphOrigin.x, gm.gmptGlyphOrigin.y, gm.gmCellIncX, gm.gmCellIncY);
    if (f == GGO_METRICS || n == GDI_ERROR || n == 0 || n > 65536) return;
    unsigned char *b = calloc(1, n);
    GetGlyphOutlineA(dc, ch, f, &gm, n, b, &m);
    out_hex(b, n);
    free(b);
}

/* memfont <hex>: AddFontMemResourceEx of the font file given ("memfont <fonts added>", 0 when
 * it fails); the fonts stay until the case ends */
static void op_memfont(void) {
    if (ntok != 2 || nmem >= 16) { bad = 1; return; }
    int n = (int)strlen(tok[1]) / 2;
    unsigned char *b = malloc(n + 1);
    for (int i = 0; i < n; i++) {
        unsigned v;
        if (sscanf(tok[1] + 2 * i, "%2x", &v) != 1) { bad = 1; free(b); return; }
        b[i] = (unsigned char)v;
    }
    DWORD count = 0;
    HANDLE h = AddFontMemResourceEx(b, n, NULL, &count);
    free(b);
    if (h) mem[nmem++] = h;
    out("memfont %lu\n", h ? count : 0);
}

static void gditext_reset(void) {
    while (nmade) DeleteObject(made[--nmade]);
    while (nmem) RemoveFontMemResourceEx(mem[--nmem]);
}

static const struct kz_op gditext_ops[] = {
    { "font", op_font },
    { "metrics", op_metrics },
    { "face", op_face },
    { "extent", op_extent },
    { "extentex", op_extentex },
    { "widths", op_widths },
    { "abc", op_widths },
    { "textout", op_textout },
    { "exttext", op_exttext },
    { "align", op_set },
    { "extra", op_set },
    { "glyph", op_glyph },
    { "memfont", op_memfont },
    { 0 },
};
const struct kz_api gditext_api = { "gditext", gditext_ops, gditext_reset };
