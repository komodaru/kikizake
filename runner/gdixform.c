#include "core.h"
#include "gdi.h"
/* GDI coordinate spaces of gdi.c's slots: mapping modes, window / viewport extents and
 * origins, the world transform, LPtoDP / DPtoLP. Floats are written as decimals and printed
 * as their IEEE bits (hex), so nothing is lost to printf. */

static float fnum(int i) {
    if (i >= ntok) { bad = 1; return 0; }
    char *e;
    float v = strtof(tok[i], &e);
    if (*e) bad = 1;
    return v;
}
static unsigned bits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }
static void out_xform(const char *what, const XFORM *x) {
    out("%s %08x %08x %08x %08x %08x %08x\n", what, bits(x->eM11), bits(x->eM12), bits(x->eM21), bits(x->eM22), bits(x->eDx), bits(x->eDy));
}
static int xform(int from, XFORM *x) {
    x->eM11 = fnum(from); x->eM12 = fnum(from + 1); x->eM21 = fnum(from + 2);
    x->eM22 = fnum(from + 3); x->eDx = fnum(from + 4); x->eDy = fnum(from + 5);
    return !bad;
}

/* mapmode <s> <m>, gmode <s> <m>: the old value (0: failed) */
static void op_mode(void) {
    HDC dc = gdi_dc(1);
    int m = num(2);
    if (bad) return;
    if (is(0, "mapmode")) out("mapmode %d\n", SetMapMode(dc, m));
    else out("gmode %d\n", SetGraphicsMode(dc, m));
}

/* winext / vpext / winorg / vporg <s> <x> <y>, offsetwin / offsetvp <s> <dx> <dy>,
 * scalewin / scalevp <s> <xn> <xd> <yn> <yd>: "<ret> <old x> <old y>" */
static void op_ext(void) {
    HDC dc = gdi_dc(1);
    int x = num(2), y = num(3);
    if (bad) return;
    SIZE s = { -1, -1 };
    POINT p = { -1, -1 };
    BOOL r;
    const char *o = tok[0];
    if (!strcmp(o, "winext")) { r = SetWindowExtEx(dc, x, y, &s); out("ext %d %ld %ld\n", r ? 1 : 0, s.cx, s.cy); }
    else if (!strcmp(o, "vpext")) { r = SetViewportExtEx(dc, x, y, &s); out("ext %d %ld %ld\n", r ? 1 : 0, s.cx, s.cy); }
    else if (!strcmp(o, "winorg")) { r = SetWindowOrgEx(dc, x, y, &p); out("org %d %ld %ld\n", r ? 1 : 0, p.x, p.y); }
    else if (!strcmp(o, "vporg")) { r = SetViewportOrgEx(dc, x, y, &p); out("org %d %ld %ld\n", r ? 1 : 0, p.x, p.y); }
    else if (!strcmp(o, "offsetwin")) { r = OffsetWindowOrgEx(dc, x, y, &p); out("org %d %ld %ld\n", r ? 1 : 0, p.x, p.y); }
    else if (!strcmp(o, "offsetvp")) { r = OffsetViewportOrgEx(dc, x, y, &p); out("org %d %ld %ld\n", r ? 1 : 0, p.x, p.y); }
    else {
        int yn = num(4), yd = num(5);
        if (bad) return;
        r = (!strcmp(o, "scalewin") ? ScaleWindowExtEx : ScaleViewportExtEx)(dc, x, y, yn, yd, &s);
        out("ext %d %ld %ld\n", r ? 1 : 0, s.cx, s.cy);
    }
}

/* getmap <s>: "map <mode> <gmode> wext x y vext x y worg x y vorg x y" */
static void op_getmap(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    SIZE we, ve;
    POINT wo, vo;
    GetWindowExtEx(dc, &we); GetViewportExtEx(dc, &ve);
    GetWindowOrgEx(dc, &wo); GetViewportOrgEx(dc, &vo);
    out("map %d %d wext %ld %ld vext %ld %ld worg %ld %ld vorg %ld %ld\n", GetMapMode(dc), GetGraphicsMode(dc), we.cx, we.cy, ve.cx, ve.cy, wo.x, wo.y, vo.x, vo.y);
}

/* lptodp / dptolp <s> x y ... (at most 32 points): "pts <ret> x y ..." */
static void op_conv(void) {
    HDC dc = gdi_dc(1);
    POINT p[32];
    int n = 0;
    for (int i = 2; i + 1 < ntok && n < 32; i += 2, n++) { p[n].x = num(i); p[n].y = num(i + 1); }
    if (bad || ntok % 2 || 2 + 2 * n < ntok) { bad = 1; return; }
    BOOL r = is(0, "lptodp") ? LPtoDP(dc, p, n) : DPtoLP(dc, p, n);
    out("pts %d", r ? 1 : 0);
    for (int i = 0; i < n; i++) out(" %ld %ld", p[i].x, p[i].y);
    out("\n");
}

/* setxform <s> m11 m12 m21 m22 dx dy, modxform <s> <mode> m11 ..., getxform <s> */
static void op_xform(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    XFORM x;
    if (is(0, "getxform")) {
        BOOL r = GetWorldTransform(dc, &x);
        out("getxform %d\n", r ? 1 : 0);
        out_xform("xform", &x);
    } else if (is(0, "setxform")) {
        if (xform(2, &x)) out("setxform %d\n", SetWorldTransform(dc, &x) ? 1 : 0);
    } else {
        int m = num(2);
        if (xform(3, &x)) out("modxform %d\n", ModifyWorldTransform(dc, &x, m) ? 1 : 0);
    }
}

/* combinexform a11 .. ady b11 .. bdy: "combine <ret>" and the product */
static void op_combinexform(void) {
    XFORM a, b, r;
    if (!xform(1, &a) || !xform(7, &b)) return;
    memset(&r, 0, sizeof(r));
    BOOL k = CombineTransform(&r, &a, &b);
    out("combinexform %d\n", k ? 1 : 0);
    out_xform("xform", &r);
}

static const struct kz_op gdixform_ops[] = {
    { "mapmode", op_mode },
    { "gmode", op_mode },
    { "winext", op_ext },
    { "vpext", op_ext },
    { "winorg", op_ext },
    { "vporg", op_ext },
    { "offsetwin", op_ext },
    { "offsetvp", op_ext },
    { "scalewin", op_ext },
    { "scalevp", op_ext },
    { "getmap", op_getmap },
    { "lptodp", op_conv },
    { "dptolp", op_conv },
    { "setxform", op_xform },
    { "modxform", op_xform },
    { "getxform", op_xform },
    { "combinexform", op_combinexform },
    { 0 },
};
const struct kz_api gdixform_api = { "gdixform", gdixform_ops, NULL };
