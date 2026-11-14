#include "core.h"
#include <d2d1.h>
#include <wincodec.h>
/* Direct2D (d2d1.dll loaded when a case first asks for it): the matrix and math helpers d2d1.dll
 * exports, stroke styles, geometries (bounds, hit tests, area, length, relations, the path
 * geometry's sink and its states) and drawing into a WIC bitmap (32bppPBGRA, the software
 * rasterizer). Every float a line gives is a decimal or 0x<its bits>; every float printed is
 * its bits in hex. Ops start with "d2". */

static HRESULT (WINAPI *create_factory)(D2D1_FACTORY_TYPE, REFIID, const D2D1_FACTORY_OPTIONS *, void **);
static ID2D1Factory *fac;
static HMODULE d2;
static int tried;
static const GUID iid_factory = { 0x06152247, 0x6f50, 0x465a, { 0x92, 0x45, 0x11, 0x8b, 0xfd, 0x3b, 0x60, 0x07 } };

#define GEOMS 8
static ID2D1Geometry *geom[GEOMS];
static ID2D1PathGeometry *sinkpath;
static ID2D1GeometrySink *sink;
static ID2D1RenderTarget *rt;
static IWICBitmap *bmp;
static IWICImagingFactory *wicf;
static int tw, th;

static void *d2fn(const char *n) {
    if (!tried) {
        tried = 1;
        d2 = LoadLibraryA("d2d1.dll");
        if (d2) create_factory = (void *)GetProcAddress(d2, "D2D1CreateFactory");
        if (create_factory && FAILED(create_factory(D2D1_FACTORY_TYPE_SINGLE_THREADED, &iid_factory, NULL, (void **)&fac))) fac = NULL;
    }
    return d2 ? (void *)GetProcAddress(d2, n) : NULL;
}
static ID2D1Factory *factory(void) {
    d2fn("D2D1CreateFactory");
    return fac;
}

static void d2d_reset(void) {
    if (rt) { ID2D1RenderTarget_EndDraw(rt, NULL, NULL); ID2D1RenderTarget_Release(rt); rt = NULL; }
    if (bmp) { IWICBitmap_Release(bmp); bmp = NULL; }
    if (sink) { ID2D1GeometrySink_Release(sink); sink = NULL; }
    if (sinkpath) { ID2D1PathGeometry_Release(sinkpath); sinkpath = NULL; }
    for (int i = 0; i < GEOMS; i++)
        if (geom[i]) { ID2D1Geometry_Release(geom[i]); geom[i] = NULL; }
}

/* token i as a float: a decimal, or 0x<bits> */
static float fl(int i) {
    if (i >= ntok) { bad = 1; return 0; }
    char *e;
    float f;
    if (tok[i][0] == '0' && tok[i][1] == 'x') {
        unsigned v = (unsigned)strtoul(tok[i], &e, 16);
        memcpy(&f, &v, 4);
    } else f = (float)strtod(tok[i], &e);
    if (*e) bad = 1;
    return f;
}
static unsigned bits(float f) { unsigned v; memcpy(&v, &f, 4); return v; }
static void outf(float f) { out(" %08x", bits(f)); }
static void matrix(int at, D2D1_MATRIX_3X2_F *m) {
    m->_11 = fl(at); m->_12 = fl(at + 1); m->_21 = fl(at + 2); m->_22 = fl(at + 3); m->_31 = fl(at + 4); m->_32 = fl(at + 5);
}
static void outm(const D2D1_MATRIX_3X2_F *m) {
    outf(m->_11); outf(m->_12); outf(m->_21); outf(m->_22); outf(m->_31); outf(m->_32);
}
/* "<color>": 4 tokens r g b a, each 0..255, as float / 255 */
static D2D1_COLOR_F color4(int at) {
    D2D1_COLOR_F c = { num(at) / 255.0f, num(at + 1) / 255.0f, num(at + 2) / 255.0f, num(at + 3) / 255.0f };
    return c;
}

/* ---------------------------------------------------------------------------------------- */
/* d2d1.dll's functions */

/* d2rot <angle> <cx> <cy>: D2D1MakeRotateMatrix -> "rot <m11 m12 m21 m22 dx dy>" */
static void op_rot(void) {
    void (WINAPI *f)(FLOAT, D2D1_POINT_2F, D2D1_MATRIX_3X2_F *) = d2fn("D2D1MakeRotateMatrix");
    D2D1_POINT_2F c = { fl(2), fl(3) };
    float a = fl(1);
    if (bad || ntok != 4) { bad = 1; return; }
    if (!f) { out("rot none\n"); return; }
    D2D1_MATRIX_3X2_F m;
    f(a, c, &m);
    out("rot"); outm(&m); out("\n");
}
/* d2skew <ax> <ay> <cx> <cy>: D2D1MakeSkewMatrix -> "skew <matrix>" */
static void op_skew(void) {
    void (WINAPI *f)(FLOAT, FLOAT, D2D1_POINT_2F, D2D1_MATRIX_3X2_F *) = d2fn("D2D1MakeSkewMatrix");
    D2D1_POINT_2F c = { fl(3), fl(4) };
    float ax = fl(1), ay = fl(2);
    if (bad || ntok != 5) { bad = 1; return; }
    if (!f) { out("skew none\n"); return; }
    D2D1_MATRIX_3X2_F m;
    f(ax, ay, c, &m);
    out("skew"); outm(&m); out("\n");
}
/* d2inv <matrix>: D2D1IsMatrixInvertible, then D2D1InvertMatrix -> "inv <is> <ret> <matrix after>" */
static void op_inv(void) {
    BOOL (WINAPI *isinv)(const D2D1_MATRIX_3X2_F *) = d2fn("D2D1IsMatrixInvertible");
    BOOL (WINAPI *inv)(D2D1_MATRIX_3X2_F *) = d2fn("D2D1InvertMatrix");
    D2D1_MATRIX_3X2_F m;
    matrix(1, &m);
    if (bad || ntok != 7) { bad = 1; return; }
    if (!isinv || !inv) { out("inv none\n"); return; }
    int a = isinv(&m) != 0;
    int b = inv(&m) != 0;
    out("inv %d %d", a, b); outm(&m); out("\n");
}
/* d2sincos <angle>: D2D1SinCos -> "sincos <s> <c>"; d2tan <angle>: D2D1Tan; d2vec3 x y z:
 * D2D1Vec3Length (Windows 8's d2d1.dll; "none" before) */
static void op_sincos(void) {
    void (WINAPI *f)(FLOAT, FLOAT *, FLOAT *) = d2fn("D2D1SinCos");
    float a = fl(1), s = 0, c = 0;
    if (bad || ntok != 2) { bad = 1; return; }
    if (!f) { out("sincos none\n"); return; }
    f(a, &s, &c);
    out("sincos"); outf(s); outf(c); out("\n");
}
static void op_tan(void) {
    FLOAT (WINAPI *f)(FLOAT) = d2fn("D2D1Tan");
    float a = fl(1);
    if (bad || ntok != 2) { bad = 1; return; }
    if (!f) { out("tan none\n"); return; }
    out("tan"); outf(f(a)); out("\n");
}
static void op_vec3(void) {
    FLOAT (WINAPI *f)(FLOAT, FLOAT, FLOAT) = d2fn("D2D1Vec3Length");
    float x = fl(1), y = fl(2), z = fl(3);
    if (bad || ntok != 4) { bad = 1; return; }
    if (!f) { out("vec3 none\n"); return; }
    out("vec3"); outf(f(x, y, z)); out("\n");
}
/* d2cs <from> <to> <r g b a as floats>: D2D1ConvertColorSpace (0 custom, 1 sRGB, 2 scRGB) ->
 * "cs <r g b a>" */
static void op_cs(void) {
    D2D1_COLOR_F (WINAPI *f)(int, int, const D2D1_COLOR_F *) = d2fn("D2D1ConvertColorSpace");
    long a = num(1), b = num(2);
    D2D1_COLOR_F c = { fl(3), fl(4), fl(5), fl(6) };
    if (bad || ntok != 7) { bad = 1; return; }
    if (!f) { out("cs none\n"); return; }
    D2D1_COLOR_F r = f((int)a, (int)b, &c);
    out("cs"); outf(r.r); outf(r.g); outf(r.b); outf(r.a); out("\n");
}

/* ---------------------------------------------------------------------------------------- */
/* stroke styles */

/* d2stroke <startcap> <endcap> <dashcap> <join> <miter> <dashstyle> <offset> [dashes...]:
 * CreateStrokeStyle (dashes NULL when none) -> "stroke hr <hr>" or
 * "stroke = <start end dash join> <miter> <style> <offset> <count> <dashes GetDashes gives>" */
static void op_stroke(void) {
    if (!factory()) { out("stroke none\n"); return; }
    D2D1_STROKE_STYLE_PROPERTIES p = { num(1), num(2), num(3), num(4), fl(5), num(6), fl(7) };
    float d[32];
    int n = ntok - 8;
    if (n > 32) { bad = 1; return; }
    for (int i = 0; i < n; i++) d[i] = fl(8 + i);
    if (bad) return;
    ID2D1StrokeStyle *s = NULL;
    HRESULT hr = ID2D1Factory_CreateStrokeStyle(fac, &p, n ? d : NULL, n, &s);
    if (FAILED(hr) || !s) { out("stroke hr %08lx\n", hr); return; }
    out("stroke = %d %d %d %d", ID2D1StrokeStyle_GetStartCap(s), ID2D1StrokeStyle_GetEndCap(s), ID2D1StrokeStyle_GetDashCap(s), ID2D1StrokeStyle_GetLineJoin(s));
    outf(ID2D1StrokeStyle_GetMiterLimit(s));
    out(" %d", ID2D1StrokeStyle_GetDashStyle(s));
    outf(ID2D1StrokeStyle_GetDashOffset(s));
    UINT32 k = ID2D1StrokeStyle_GetDashesCount(s);
    out(" %u", k);
    float g[64];
    if (k <= 64) {
        ID2D1StrokeStyle_GetDashes(s, g, k);
        for (UINT32 i = 0; i < k; i++) outf(g[i]);
    }
    out("\n");
    ID2D1StrokeStyle_Release(s);
}

/* ---------------------------------------------------------------------------------------- */
/* geometries: slots 0..7 */

static int slot(int i) {
    long g = num(i);
    if (g < 0 || g >= GEOMS) { bad = 1; return 0; }
    return (int)g;
}
static void put(int g, void *o) {
    if (geom[g]) ID2D1Geometry_Release(geom[g]);
    geom[g] = o;
}
static ID2D1Geometry *get(int i) {
    int g = slot(i);
    if (bad) return NULL;
    if (!geom[g]) out("nogeom\n");
    return geom[g];
}

/* d2rect <g> l t r b, d2ell <g> cx cy rx ry, d2rrect <g> l t r b rx ry -> "<op> <hr>" */
static void op_rect(void) {
    int g = slot(1);
    D2D1_RECT_F r = { fl(2), fl(3), fl(4), fl(5) };
    if (bad || ntok != 6) { bad = 1; return; }
    if (!factory()) { out("rect none\n"); return; }
    ID2D1RectangleGeometry *o = NULL;
    HRESULT hr = ID2D1Factory_CreateRectangleGeometry(fac, &r, &o);
    put(g, o);
    out("rect %08lx\n", hr);
}
static void op_ell(void) {
    int g = slot(1);
    D2D1_ELLIPSE e = { { fl(2), fl(3) }, fl(4), fl(5) };
    if (bad || ntok != 6) { bad = 1; return; }
    if (!factory()) { out("ell none\n"); return; }
    ID2D1EllipseGeometry *o = NULL;
    HRESULT hr = ID2D1Factory_CreateEllipseGeometry(fac, &e, &o);
    put(g, o);
    out("ell %08lx\n", hr);
}
static void op_rrect(void) {
    int g = slot(1);
    D2D1_ROUNDED_RECT r = { { fl(2), fl(3), fl(4), fl(5) }, fl(6), fl(7) };
    if (bad || ntok != 8) { bad = 1; return; }
    if (!factory()) { out("rrect none\n"); return; }
    ID2D1RoundedRectangleGeometry *o = NULL;
    HRESULT hr = ID2D1Factory_CreateRoundedRectangleGeometry(fac, &r, &o);
    put(g, o);
    out("rrect %08lx\n", hr);
}
/* d2xf <g> <source g> <matrix>: CreateTransformedGeometry -> "xf <hr>" */
static void op_xf(void) {
    int g = slot(1);
    ID2D1Geometry *src = get(2);
    D2D1_MATRIX_3X2_F m;
    matrix(3, &m);
    if (bad || ntok != 9) { bad = 1; return; }
    if (!src) return;
    ID2D1TransformedGeometry *o = NULL;
    HRESULT hr = ID2D1Factory_CreateTransformedGeometry(fac, src, &m, &o);
    put(g, o);
    out("xf %08lx\n", hr);
}
/* d2path <g> alt|wind <figures>: a path geometry, each figure "m x y [x y]... z|o" (BeginFigure
 * filled, AddLine for each point, EndFigure closed / open) -> "path <Open's hr> <Close's hr>" */
static void op_path(void) {
    int g = slot(1);
    int wind = is(2, "wind");
    if (bad || (!wind && !is(2, "alt"))) { bad = 1; return; }
    /* (checked before anything is made) */
    int infig = 0;
    for (int i = 3; i < ntok; i++) {
        if (is(i, "m")) { if (infig) bad = 1; fl(i + 1); fl(i + 2); i += 2; infig = 1; }
        else if (is(i, "z") || is(i, "o")) { if (!infig) bad = 1; infig = 0; }
        else { if (!infig) bad = 1; fl(i); fl(i + 1); i++; }
    }
    if (bad || infig) { bad = 1; return; }
    if (!factory()) { out("path none\n"); return; }
    ID2D1PathGeometry *p = NULL;
    ID2D1GeometrySink *s = NULL;
    HRESULT hr = ID2D1Factory_CreatePathGeometry(fac, &p);
    if (FAILED(hr)) { out("path hr %08lx\n", hr); return; }
    HRESULT ho = ID2D1PathGeometry_Open(p, &s), hc = 0;
    if (SUCCEEDED(ho)) {
        ID2D1GeometrySink_SetFillMode(s, wind ? D2D1_FILL_MODE_WINDING : D2D1_FILL_MODE_ALTERNATE);
        for (int i = 3; i < ntok; i++) {
            if (is(i, "m")) {
                D2D1_POINT_2F q = { fl(i + 1), fl(i + 2) };
                ID2D1GeometrySink_BeginFigure(s, q, D2D1_FIGURE_BEGIN_FILLED);
                i += 2;
            } else if (is(i, "z") || is(i, "o")) ID2D1GeometrySink_EndFigure(s, is(i, "z") ? D2D1_FIGURE_END_CLOSED : D2D1_FIGURE_END_OPEN);
            else {
                D2D1_POINT_2F q = { fl(i), fl(i + 1) };
                ID2D1GeometrySink_AddLine(s, q);
                i++;
            }
        }
        hc = ID2D1GeometrySink_Close(s);
        ID2D1GeometrySink_Release(s);
    }
    put(g, p);
    out("path %08lx %08lx\n", ho, hc);
}

/* d2sink <step>...: one path geometry (made on the first step, kept for the case) driven step
 * by step: open, begin x y, line x y, end, close, fill alt|wind, count. Prints "sink" and for
 * open, close and count their results (count: GetFigureCount's hr and count, GetSegmentCount's) */
static void op_sink(void) {
    if (!factory()) { out("sink none\n"); return; }
    if (!sinkpath && FAILED(ID2D1Factory_CreatePathGeometry(fac, &sinkpath))) { sinkpath = NULL; out("sink none\n"); return; }
    out("sink");
    for (int i = 1; i < ntok && !bad; i++) {
        if (is(i, "open")) {
            ID2D1GeometrySink *s = NULL;
            HRESULT hr = ID2D1PathGeometry_Open(sinkpath, &s);
            out(" open %08lx", hr);
            if (SUCCEEDED(hr) && s) { if (sink) ID2D1GeometrySink_Release(sink); sink = s; }
        } else if (is(i, "close")) {
            if (!sink) { out(" close -"); continue; }
            out(" close %08lx", ID2D1GeometrySink_Close(sink));
        } else if (is(i, "count")) {
            UINT32 f = 0xdead, n = 0xdead;
            HRESULT a = ID2D1PathGeometry_GetFigureCount(sinkpath, &f), b = ID2D1PathGeometry_GetSegmentCount(sinkpath, &n);
            out(" count %08lx %u %08lx %u", a, f, b, n);
        } else if (is(i, "begin") || is(i, "line")) {
            D2D1_POINT_2F q = { fl(i + 1), fl(i + 2) };
            if (bad) break;
            if (sink && is(i, "begin")) ID2D1GeometrySink_BeginFigure(sink, q, D2D1_FIGURE_BEGIN_FILLED);
            else if (sink) ID2D1GeometrySink_AddLine(sink, q);
            i += 2;
        } else if (is(i, "end")) {
            if (sink) ID2D1GeometrySink_EndFigure(sink, D2D1_FIGURE_END_CLOSED);
        } else if (is(i, "fill")) {
            if (!is(i + 1, "alt") && !is(i + 1, "wind")) { bad = 1; break; }
            if (sink) ID2D1GeometrySink_SetFillMode(sink, is(i + 1, "wind") ? D2D1_FILL_MODE_WINDING : D2D1_FILL_MODE_ALTERNATE);
            i++;
        } else bad = 1;
    }
    out("\n");
}

/* d2bounds <g> [matrix]: GetBounds (world transform NULL without one) -> "bounds <hr> l t r b" */
static void op_bounds(void) {
    ID2D1Geometry *g = get(1);
    D2D1_MATRIX_3X2_F m;
    int hasm = ntok > 2;
    if (hasm) matrix(2, &m);
    if (bad || (ntok != 2 && ntok != 8)) { bad = 1; return; }
    if (!g) return;
    D2D1_RECT_F r = { 0, 0, 0, 0 };
    HRESULT hr = g->lpVtbl->GetBounds(g, hasm ? &m : NULL, &r);
    out("bounds %08lx", hr); outf(r.left); outf(r.top); outf(r.right); outf(r.bottom); out("\n");
}
/* d2in <g> x y [tolerance]: FillContainsPoint (no transform; tolerance 0.25 when not given) ->
 * "in <hr> <0|1>" */
static void op_in(void) {
    ID2D1Geometry *g = get(1);
    D2D1_POINT_2F q = { fl(2), fl(3) };
    float tol = ntok > 4 ? fl(4) : D2D1_DEFAULT_FLATTENING_TOLERANCE;
    if (bad || (ntok != 4 && ntok != 5)) { bad = 1; return; }
    if (!g) return;
    BOOL c = 7;
    HRESULT hr = g->lpVtbl->FillContainsPoint(g, q, NULL, tol, &c);
    out("in %08lx %d\n", hr, c);
}
/* d2area <g>, d2len <g>: ComputeArea / ComputeLength (no transform, tolerance 0.25) ->
 * "area <hr> <v>" */
static void op_area(void) {
    ID2D1Geometry *g = get(1);
    if (bad || ntok != 2) { bad = 1; return; }
    if (!g) return;
    float v = -1;
    HRESULT hr = is(0, "d2area") ? g->lpVtbl->ComputeArea(g, NULL, D2D1_DEFAULT_FLATTENING_TOLERANCE, &v) : g->lpVtbl->ComputeLength(g, NULL, D2D1_DEFAULT_FLATTENING_TOLERANCE, &v);
    out("%s %08lx", is(0, "d2area") ? "area" : "len", hr); outf(v); out("\n");
}
/* d2cmp <a> <b>: a's CompareWithGeometry(b) (no transform, tolerance 0.25) -> "cmp <hr> <relation>" */
static void op_cmp(void) {
    ID2D1Geometry *a = get(1), *b = get(2);
    if (bad || ntok != 3) { bad = 1; return; }
    if (!a || !b) return;
    D2D1_GEOMETRY_RELATION r = 77;
    HRESULT hr = a->lpVtbl->CompareWithGeometry(a, b, NULL, D2D1_DEFAULT_FLATTENING_TOLERANCE, &r);
    out("cmp %08lx %d\n", hr, r);
}
/* d2count <g>: a path geometry's GetFigureCount and GetSegmentCount -> "count <hr> <n> <hr> <n>" */
static void op_count(void) {
    ID2D1Geometry *g = get(1);
    if (bad || ntok != 2) { bad = 1; return; }
    if (!g) return;
    static const GUID iid_path = { 0x2cd906a5, 0x12e2, 0x11dc, { 0x9f, 0xed, 0x00, 0x11, 0x43, 0xa0, 0x55, 0xf9 } };
    ID2D1PathGeometry *p = NULL;
    if (FAILED(ID2D1Geometry_QueryInterface(g, &iid_path, (void **)&p)) || !p) { out("count nopath\n"); return; }
    UINT32 f = 0xdead, n = 0xdead;
    HRESULT a = ID2D1PathGeometry_GetFigureCount(p, &f), b = ID2D1PathGeometry_GetSegmentCount(p, &n);
    out("count %08lx %u %08lx %u\n", a, f, b, n);
    ID2D1PathGeometry_Release(p);
}


/* ---------------------------------------------------------------------------------------- */
/* what a geometry gives a sink: Simplify, Outline, Widen, CombineWithGeometry, Tessellate */

/* a sink that prints each call: " F <mode>" " S <flags>" " B x y <begin>" " L x y" " C x1 y1 x2 y2
 * x3 y3" " E <end>" " Z" (Close), " T <6 floats>" (a triangle) */
static HRESULT WINAPI ks_qi(void *t, REFIID r, void **o) { *o = t; return S_OK; }
static ULONG WINAPI ks_ref(void *t) { return 1; }
static void WINAPI ks_fill(void *t, D2D1_FILL_MODE m) { out(" F %d", m); }
static void WINAPI ks_flags(void *t, D2D1_PATH_SEGMENT f) { out(" S %d", f); }
static void WINAPI ks_begin(void *t, D2D1_POINT_2F p, D2D1_FIGURE_BEGIN b) { out(" B"); outf(p.x); outf(p.y); out(" %d", b); }
static void WINAPI ks_lines(void *t, const D2D1_POINT_2F *p, UINT n) {
    for (UINT i = 0; i < n; i++) { out(" L"); outf(p[i].x); outf(p[i].y); }
}
static void WINAPI ks_beziers(void *t, const D2D1_BEZIER_SEGMENT *b, UINT n) {
    for (UINT i = 0; i < n; i++) { out(" C"); outf(b[i].point1.x); outf(b[i].point1.y); outf(b[i].point2.x); outf(b[i].point2.y); outf(b[i].point3.x); outf(b[i].point3.y); }
}
static void WINAPI ks_end(void *t, D2D1_FIGURE_END e) { out(" E %d", e); }
static HRESULT WINAPI ks_close(void *t) { out(" Z"); return S_OK; }
static void WINAPI ks_tris(void *t, const D2D1_TRIANGLE *tr, UINT n) {
    for (UINT i = 0; i < n; i++) { out(" T"); outf(tr[i].point1.x); outf(tr[i].point1.y); outf(tr[i].point2.x); outf(tr[i].point2.y); outf(tr[i].point3.x); outf(tr[i].point3.y); }
}
static void *ks_vt[] = { ks_qi, ks_ref, ks_ref, ks_fill, ks_flags, ks_begin, ks_lines, ks_beziers, ks_end, ks_close };
static void *ks_tvt[] = { ks_qi, ks_ref, ks_ref, ks_tris, ks_close };
static struct { void **vt; } ksink = { ks_vt }, ktess = { ks_tvt };

/* d2simp <g> cubics|lines [tolerance]: Simplify (no transform; tolerance 0.25 when not given);
 * d2outline <g> [tolerance]: Outline; d2widen <g> <width> [tolerance]: Widen (no stroke style);
 * d2combine <a> <b> union|intersect|xor|exclude [tolerance]: a's CombineWithGeometry(b);
 * d2tess <g> [tolerance]: Tessellate. Each prints "<op> <hr>" and then on the same line what the
 * sink was given */
static void op_simp(void) {
    int combine = is(0, "d2combine"), simp = is(0, "d2simp"), widen = is(0, "d2widen"), tess = is(0, "d2tess");
    ID2D1Geometry *g = get(1), *b = NULL;
    int at = 2, opt = 0, mode = 0;
    float width = 0;
    if (simp) { opt = is(2, "lines"); if (!opt && !is(2, "cubics")) bad = 1; at = 3; }
    if (widen) { width = fl(2); at = 3; }
    if (combine) {
        b = get(2);
        static const char *modes[] = { "union", "intersect", "xor", "exclude" };
        for (mode = 0; mode < 4 && !is(3, modes[mode]); mode++) {}
        if (mode == 4) bad = 1;
        at = 4;
    }
    float tol = ntok > at ? fl(at) : D2D1_DEFAULT_FLATTENING_TOLERANCE;
    if (bad || ntok > at + 1) { bad = 1; return; }
    if (!g || (combine && !b)) return;
    out("%s", tok[0] + 2);
    HRESULT hr;
    void *s = tess ? (void *)&ktess : (void *)&ksink;
    if (simp) hr = g->lpVtbl->Simplify(g, opt, NULL, tol, s);
    else if (widen) hr = g->lpVtbl->Widen(g, width, NULL, NULL, tol, s);
    else if (combine) hr = g->lpVtbl->CombineWithGeometry(g, b, mode, NULL, tol, s);
    else if (tess) hr = g->lpVtbl->Tessellate(g, NULL, tol, s);
    else hr = g->lpVtbl->Outline(g, NULL, tol, s);
    out(" hr %08lx\n", hr);
}

/* ---------------------------------------------------------------------------------------- */
/* drawing: a WIC bitmap render target */

/* d2target <w> <h>: a w x h 32bppPBGRA WIC bitmap (zeroed), CreateWicBitmapRenderTarget with
 * the default properties, BeginDraw -> "target <hr>" */
static void op_target(void) {
    long w = num(1), h = num(2);
    if (bad || ntok != 3 || w < 1 || h < 1 || w > 64 || h > 64) { bad = 1; return; }
    if (rt) { ID2D1RenderTarget_EndDraw(rt, NULL, NULL); ID2D1RenderTarget_Release(rt); rt = NULL; }
    if (bmp) { IWICBitmap_Release(bmp); bmp = NULL; }
    if (!wicf) {
        CoInitializeEx(NULL, COINIT_MULTITHREADED);
        if (FAILED(CoCreateInstance(&CLSID_WICImagingFactory, NULL, CLSCTX_INPROC_SERVER, &IID_IWICImagingFactory, (void **)&wicf))) wicf = NULL;
    }
    if (!factory() || !wicf) { out("target none\n"); return; }
    HRESULT hr = IWICImagingFactory_CreateBitmap(wicf, w, h, &GUID_WICPixelFormat32bppPBGRA, WICBitmapCacheOnLoad, &bmp);
    if (FAILED(hr)) { bmp = NULL; out("target wic %08lx\n", hr); return; }
    D2D1_RENDER_TARGET_PROPERTIES p = { D2D1_RENDER_TARGET_TYPE_DEFAULT, { DXGI_FORMAT_UNKNOWN, D2D1_ALPHA_MODE_UNKNOWN }, 0, 0, D2D1_RENDER_TARGET_USAGE_NONE, D2D1_FEATURE_LEVEL_DEFAULT };
    hr = ID2D1Factory_CreateWicBitmapRenderTarget(fac, bmp, &p, &rt);
    if (FAILED(hr)) rt = NULL;
    else ID2D1RenderTarget_BeginDraw(rt);
    tw = w; th = h;
    out("target %08lx\n", hr);
}
static int have_rt(void) {
    if (!rt) out("nort\n");
    return rt != NULL;
}
/* d2aa per|aliased: SetAntialiasMode */
static void op_aa(void) {
    int al = is(1, "aliased");
    if (ntok != 2 || (!al && !is(1, "per"))) { bad = 1; return; }
    if (have_rt()) ID2D1RenderTarget_SetAntialiasMode(rt, al ? D2D1_ANTIALIAS_MODE_ALIASED : D2D1_ANTIALIAS_MODE_PER_PRIMITIVE);
}
/* d2xform <matrix>: the render target's SetTransform */
static void op_xform(void) {
    D2D1_MATRIX_3X2_F m;
    matrix(1, &m);
    if (bad || ntok != 7) { bad = 1; return; }
    if (have_rt()) ID2D1RenderTarget_SetTransform(rt, &m);
}
/* d2clear <color>: Clear */
static void op_clear(void) {
    D2D1_COLOR_F c = color4(1);
    if (bad || ntok != 5) { bad = 1; return; }
    if (have_rt()) ID2D1RenderTarget_Clear(rt, &c);
}
/* d2fill l t r b <color>: FillRectangle with a solid color brush of that color; d2fillg <g>
 * <color>: FillGeometry (no opacity brush) */
static void op_fill(void) {
    int geo = is(0, "d2fillg");
    D2D1_RECT_F r = { 0, 0, 0, 0 };
    ID2D1Geometry *g = NULL;
    D2D1_COLOR_F c;
    if (geo) { g = get(1); c = color4(2); if (ntok != 6) bad = 1; }
    else { r.left = fl(1); r.top = fl(2); r.right = fl(3); r.bottom = fl(4); c = color4(5); if (ntok != 9) bad = 1; }
    if (bad || !have_rt() || (geo && !g)) return;
    ID2D1SolidColorBrush *b = NULL;
    HRESULT hr = ID2D1RenderTarget_CreateSolidColorBrush(rt, &c, NULL, &b);
    if (FAILED(hr)) { out("brush %08lx\n", hr); return; }
    if (geo) ID2D1RenderTarget_FillGeometry(rt, g, (ID2D1Brush *)b, NULL);
    else ID2D1RenderTarget_FillRectangle(rt, &r, (ID2D1Brush *)b);
    ID2D1SolidColorBrush_Release(b);
}
/* d2brush <r g b a as floats>: CreateSolidColorBrush -> "brush <GetColor> <GetOpacity>" */
static void op_brush(void) {
    D2D1_COLOR_F c = { fl(1), fl(2), fl(3), fl(4) }, g;
    if (bad || ntok != 5) { bad = 1; return; }
    if (!have_rt()) return;
    ID2D1SolidColorBrush *b = NULL;
    HRESULT hr = ID2D1RenderTarget_CreateSolidColorBrush(rt, &c, NULL, &b);
    if (FAILED(hr)) { out("brush %08lx\n", hr); return; }
    /* (GetColor returns a struct: the hidden pointer the Microsoft ABI passes, explicitly) */
    D2D1_COLOR_F *(WINAPI *getc)(ID2D1SolidColorBrush *, D2D1_COLOR_F *) = (void *)b->lpVtbl->GetColor;
    getc(b, &g);
    out("brush"); outf(g.r); outf(g.g); outf(g.b); outf(g.a); outf(ID2D1SolidColorBrush_GetOpacity(b)); out("\n");
    ID2D1SolidColorBrush_Release(b);
}
/* d2read: EndDraw, the bitmap's pixels, BeginDraw again -> "end <hr>", "px 0 <w> <h> 32" and
 * each row's bytes (B G R A) */
static void op_read(void) {
    if (ntok != 1) { bad = 1; return; }
    if (!have_rt()) return;
    out("end %08lx\n", ID2D1RenderTarget_EndDraw(rt, NULL, NULL));
    unsigned char *px = calloc(tw * th, 4);
    HRESULT hr = IWICBitmap_CopyPixels(bmp, NULL, tw * 4, tw * th * 4, px);
    if (FAILED(hr)) out("copy %08lx\n", hr);
    else {
        out("px 0 %d %d 32\n", tw, th);
        for (int y = 0; y < th; y++) out_hex(px + y * tw * 4, tw * 4);
    }
    free(px);
    ID2D1RenderTarget_BeginDraw(rt);
}

static const struct kz_op d2d_ops[] = {
    { "d2rot", op_rot }, { "d2skew", op_skew }, { "d2inv", op_inv },
    { "d2sincos", op_sincos }, { "d2tan", op_tan }, { "d2vec3", op_vec3 }, { "d2cs", op_cs },
    { "d2stroke", op_stroke },
    { "d2rect", op_rect }, { "d2ell", op_ell }, { "d2rrect", op_rrect }, { "d2xf", op_xf }, { "d2path", op_path },
    { "d2sink", op_sink }, { "d2bounds", op_bounds }, { "d2in", op_in }, { "d2area", op_area }, { "d2len", op_area },
    { "d2cmp", op_cmp }, { "d2count", op_count },
    { "d2simp", op_simp }, { "d2outline", op_simp }, { "d2widen", op_simp }, { "d2combine", op_simp }, { "d2tess", op_simp },
    { "d2target", op_target }, { "d2aa", op_aa }, { "d2xform", op_xform }, { "d2clear", op_clear },
    { "d2fill", op_fill }, { "d2fillg", op_fill }, { "d2brush", op_brush }, { "d2read", op_read },
    { 0 },
};
const struct kz_api d2d_api = { "d2d", d2d_ops, d2d_reset };
