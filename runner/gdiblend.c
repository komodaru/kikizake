#include "core.h"
#include "gdi.h"
/* GDI's blits beyond the raster operations, into gdi.c's slots: AlphaBlend, TransparentBlt,
 * GradientFill (gdi32's GdiAlphaBlend / GdiTransparentBlt / GdiGradientFill, which msimg32's
 * exports forward to), MaskBlt and PlgBlt */

/* alphablend <d> x y w h <s> sx sy sw sh <constant alpha> <AC_SRC_ALPHA: 0|1> */
static void op_alphablend(void) {
    HDC d = gdi_dc(1), s = gdi_dc(6);
    int x = num(2), y = num(3), w = num(4), h = num(5), sx = num(7), sy = num(8), sw = num(9), sh = num(10), a = num(11), f = num(12);
    if (bad) return;
    BLENDFUNCTION bf = { AC_SRC_OVER, 0, (BYTE)a, (BYTE)(f ? AC_SRC_ALPHA : 0) };
    out("ret %d\n", GdiAlphaBlend(d, x, y, w, h, s, sx, sy, sw, sh, bf) ? 1 : 0);
}

/* transparent <d> x y w h <s> sx sy sw sh #key */
static void op_transparent(void) {
    HDC d = gdi_dc(1), s = gdi_dc(6);
    int x = num(2), y = num(3), w = num(4), h = num(5), sx = num(7), sy = num(8), sw = num(9), sh = num(10);
    COLORREF k = color(11);
    if (!bad) out("ret %d\n", GdiTransparentBlt(d, x, y, w, h, s, sx, sy, sw, sh, k) ? 1 : 0);
}

/* gradrect <s> <0 h|1 v> then two vertices, gradtri <s> then three: each "x y r g b a"
 * (16 bit channels) */
static void op_gradient(void) {
    HDC dc = gdi_dc(1);
    int tri = is(0, "gradtri"), from = tri ? 2 : 3, nv = tri ? 3 : 2;
    int mode = tri ? GRADIENT_FILL_TRIANGLE : num(2) ? GRADIENT_FILL_RECT_V : GRADIENT_FILL_RECT_H;
    TRIVERTEX v[3];
    for (int i = 0; i < nv; i++) {
        int k = from + 6 * i;
        v[i].x = num(k); v[i].y = num(k + 1);
        v[i].Red = num(k + 2); v[i].Green = num(k + 3); v[i].Blue = num(k + 4); v[i].Alpha = num(k + 5);
    }
    if (bad) return;
    GRADIENT_RECT gr = { 0, 1 };
    GRADIENT_TRIANGLE gt = { 0, 1, 2 };
    out("ret %d\n", GdiGradientFill(dc, v, nv, tri ? (void *)&gt : (void *)&gr, 1, mode) ? 1 : 0);
}

/* maskblt <d> x y w h <s> sx sy <m|none> mx my <rop4> */
static void op_maskblt(void) {
    HDC d = gdi_dc(1), s = gdi_dc(6);
    int x = num(2), y = num(3), w = num(4), h = num(5), sx = num(7), sy = num(8);
    HBITMAP m = is(9, "none") ? NULL : gdi_bitmap(9);
    int mx = num(10), my = num(11);
    DWORD rop = (DWORD)num(12);
    if (bad) return;
    /* (a mask bitmap must not be selected into a DC: its slot lends it meanwhile) */
    HDC mdc = m ? gdi_dc(9) : NULL;
    HGDIOBJ keep = m ? SelectObject(mdc, CreateBitmap(1, 1, 1, 1, NULL)) : NULL;
    BOOL r = MaskBlt(d, x, y, w, h, s, sx, sy, m, mx, my, rop);
    if (m) DeleteObject(SelectObject(mdc, keep));
    out("ret %d\n", r ? 1 : 0);
}

/* plgblt <d> x0 y0 x1 y1 x2 y2 <s> sx sy w h [<m> mx my] */
static void op_plgblt(void) {
    HDC d = gdi_dc(1), s = gdi_dc(8);
    POINT p[3] = { { num(2), num(3) }, { num(4), num(5) }, { num(6), num(7) } };
    int sx = num(9), sy = num(10), w = num(11), h = num(12);
    HBITMAP m = ntok > 13 ? gdi_bitmap(13) : NULL;
    int mx = ntok > 13 ? num(14) : 0, my = ntok > 13 ? num(15) : 0;
    if (bad) return;
    HDC mdc = m ? gdi_dc(13) : NULL;
    HGDIOBJ keep = m ? SelectObject(mdc, CreateBitmap(1, 1, 1, 1, NULL)) : NULL;
    BOOL r = PlgBlt(d, p, s, sx, sy, w, h, m, mx, my);
    if (m) DeleteObject(SelectObject(mdc, keep));
    out("ret %d\n", r ? 1 : 0);
}

/* halftone <s>: CreateHalftonePalette selected into the slot's DC (the gradients' 6 levels) */
static HPALETTE halftones[16];
static int nhalftone;
static void op_halftone(void) {
    HDC dc = gdi_dc(1);
    if (bad || !dc || nhalftone == 16) { bad = 1; return; }
    HPALETTE p = CreateHalftonePalette(dc);
    if (!p) { out("halftone 0\n"); return; }
    halftones[nhalftone++] = p;
    out("halftone %d\n", SelectPalette(dc, p, FALSE) != NULL);
}

static void gdiblend_reset(void) {
    while (nhalftone) DeleteObject(halftones[--nhalftone]);
}

static const struct kz_op gdiblend_ops[] = {
    { "halftone", op_halftone },
    { "alphablend", op_alphablend },
    { "transparent", op_transparent },
    { "gradrect", op_gradient },
    { "gradtri", op_gradient },
    { "maskblt", op_maskblt },
    { "plgblt", op_plgblt },
    { 0 },
};
const struct kz_api gdiblend_api = { "gdiblend", gdiblend_ops, gdiblend_reset };
