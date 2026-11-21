#include "core.h"
#include "gdi.h"
/* GDI lines and filled shapes into gdi.c's slots: pens, the binary raster operations (SetROP2),
 * the current position, polylines, polygons, rectangles, ellipses, arcs, FillRect & co. The
 * pens made in a case are deleted at its end (after gdi.c has deleted the DCs). */

static HGDIOBJ made[512];
static int nmade;
static void track(HGDIOBJ o) {
    if (nmade < 512) made[nmade++] = o;
}

static void select_pen(HDC dc, HPEN p) {
    if (!p) { out("pen failed %lu\n", GetLastError()); return; }
    track(p);
    SelectObject(dc, p);
}

/* pen <s> <style> <width> #c | pen <s> null */
static void op_pen(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    if (is(2, "null")) { SelectObject(dc, GetStockObject(NULL_PEN)); return; }
    int st = num(2), w = num(3);
    COLORREF c = color(4);
    if (!bad) select_pen(dc, CreatePen(st, w, c));
}

/* extpen <s> <style> <width> #c [hatch]: ExtCreatePen with a solid (or hatched) brush */
static void op_extpen(void) {
    HDC dc = gdi_dc(1);
    DWORD st = (DWORD)num(2);
    int w = num(3);
    COLORREF c = color(4);
    if (bad) return;
    LOGBRUSH lb = { BS_SOLID, c, 0 };
    if (ntok > 5) { lb.lbStyle = BS_HATCHED; lb.lbHatch = num(5); }
    if (!bad) select_pen(dc, ExtCreatePen(st, w, &lb, 0, NULL));
}

/* the points of tokens from..ntok-1 (x y pairs) */
static POINT pts[64];
static int points(int from) {
    int n = 0;
    for (int i = from; i + 1 < ntok && n < 64; i += 2, n++) { pts[n].x = num(i); pts[n].y = num(i + 1); }
    if ((ntok - from) % 2) bad = 1;
    return n;
}

static void op_line(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    const char *o = tok[0];
    BOOL r = 0;
    if (!strcmp(o, "moveto")) {
        POINT p;
        int x = num(2), y = num(3);
        if (bad) return;
        r = MoveToEx(dc, x, y, &p);
        out("pos %ld %ld\n", p.x, p.y);
        return;
    }
    if (!strcmp(o, "getpos")) {
        POINT p;
        GetCurrentPositionEx(dc, &p);
        out("pos %ld %ld\n", p.x, p.y);
        return;
    }
    if (!strcmp(o, "lineto")) { int x = num(2), y = num(3); if (bad) return; r = LineTo(dc, x, y); }
    else if (!strcmp(o, "anglearc")) {
        /* anglearc <s> x y radius start sweep (degrees, decimal) */
        int x = num(2), y = num(3), rad = num(4);
        char *e1 = NULL, *e2 = NULL;
        float a = ntok == 7 ? (float)strtod(tok[5], &e1) : 0, sw = ntok == 7 ? (float)strtod(tok[6], &e2) : 0;
        if (bad || ntok != 7 || !e1 || *e1 || !e2 || *e2) { bad = 1; return; }
        r = AngleArc(dc, x, y, (DWORD)rad, a, sw);
    }
    else {
        int n = points(2);
        if (bad) return;
        if (!strcmp(o, "polyline")) r = Polyline(dc, pts, n);
        else if (!strcmp(o, "polylineto")) r = PolylineTo(dc, pts, n);
        else if (!strcmp(o, "polygon")) r = Polygon(dc, pts, n);
        else if (!strcmp(o, "polybezier")) r = PolyBezier(dc, pts, n);
        else if (!strcmp(o, "polybezierto")) r = PolyBezierTo(dc, pts, n);
        else if (!strcmp(o, "rectangle")) { if (n != 2) { bad = 1; return; } r = Rectangle(dc, pts[0].x, pts[0].y, pts[1].x, pts[1].y); }
        else if (!strcmp(o, "ellipse")) { if (n != 2) { bad = 1; return; } r = Ellipse(dc, pts[0].x, pts[0].y, pts[1].x, pts[1].y); }
        else if (!strcmp(o, "roundrect")) { if (n != 3) { bad = 1; return; } r = RoundRect(dc, pts[0].x, pts[0].y, pts[1].x, pts[1].y, pts[2].x, pts[2].y); }
        else if (!strcmp(o, "arc") || !strcmp(o, "arcto") || !strcmp(o, "pie") || !strcmp(o, "chord")) {
            if (n != 4) { bad = 1; return; }
            BOOL (WINAPI *f)(HDC, int, int, int, int, int, int, int, int) = !strcmp(o, "arc") ? Arc : !strcmp(o, "arcto") ? ArcTo : !strcmp(o, "pie") ? Pie : Chord;
            r = f(dc, pts[0].x, pts[0].y, pts[1].x, pts[1].y, pts[2].x, pts[2].y, pts[3].x, pts[3].y);
        } else if (!strcmp(o, "fillrect") || !strcmp(o, "framerect") || !strcmp(o, "invertrect")) {
            if (n != 2) { bad = 1; return; }
            RECT rc = { pts[0].x, pts[0].y, pts[1].x, pts[1].y };
            HBRUSH b = (HBRUSH)GetCurrentObject(dc, OBJ_BRUSH);
            if (o[0] == 'f' && o[1] == 'i') r = FillRect(dc, &rc, b) != 0;
            else if (o[0] == 'f') r = FrameRect(dc, &rc, b) != 0;
            else r = InvertRect(dc, &rc);
        } else { bad = 1; return; }
    }
    out("ret %d\n", r ? 1 : 0);
}

/* rop2 <s> <mode>, polyfill <s> <mode>, arcdir <s> <dir>: the old value */
static void op_state(void) {
    HDC dc = gdi_dc(1);
    int m = num(2);
    if (bad) return;
    if (is(0, "rop2")) out("rop2 %d\n", SetROP2(dc, m));
    else if (is(0, "polyfill")) out("polyfill %d\n", SetPolyFillMode(dc, m));
    else out("arcdir %d\n", SetArcDirection(dc, m));
}

static void gdiline_reset(void) {
    while (nmade) DeleteObject(made[--nmade]);
}

static const struct kz_op gdiline_ops[] = {
    { "pen", op_pen },
    { "extpen", op_extpen },
    { "moveto", op_line },
    { "getpos", op_line },
    { "lineto", op_line },
    { "anglearc", op_line },
    { "polyline", op_line },
    { "polylineto", op_line },
    { "polygon", op_line },
    { "polybezier", op_line },
    { "polybezierto", op_line },
    { "rectangle", op_line },
    { "ellipse", op_line },
    { "roundrect", op_line },
    { "arc", op_line },
    { "arcto", op_line },
    { "pie", op_line },
    { "chord", op_line },
    { "fillrect", op_line },
    { "framerect", op_line },
    { "invertrect", op_line },
    { "rop2", op_state },
    { "polyfill", op_state },
    { "arcdir", op_state },
    { 0 },
};
const struct kz_api gdiline_api = { "gdiline", gdiline_ops, gdiline_reset };
