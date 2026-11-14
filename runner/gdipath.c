#include "core.h"
#include "gdi.h"
/* GDI paths of gdi.c's slots (the shapes come from gdiline.c's ops between beginpath and
 * endpath) */

static void op_path(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    const char *o = tok[0];
    BOOL r;
    if (!strcmp(o, "beginpath")) r = BeginPath(dc);
    else if (!strcmp(o, "endpath")) r = EndPath(dc);
    else if (!strcmp(o, "abortpath")) r = AbortPath(dc);
    else if (!strcmp(o, "closefig")) r = CloseFigure(dc);
    else if (!strcmp(o, "flatten")) r = FlattenPath(dc);
    else if (!strcmp(o, "widen")) r = WidenPath(dc);
    else if (!strcmp(o, "strokepath")) r = StrokePath(dc);
    else if (!strcmp(o, "fillpath")) r = FillPath(dc);
    else if (!strcmp(o, "strokefill")) r = StrokeAndFillPath(dc);
    else if (!strcmp(o, "selclippath")) { int m = num(2); if (bad) return; r = SelectClipPath(dc, m); }
    else { bad = 1; return; }
    out("%s %d\n", o, r ? 1 : 0);
}

/* getpath <s>: "path <n>" and "pt x y type" for each point (n -1: no closed path) */
static void op_getpath(void) {
    HDC dc = gdi_dc(1);
    if (bad) return;
    int n = GetPath(dc, NULL, NULL, 0);
    out("path %d\n", n);
    if (n <= 0) return;
    POINT *p = calloc(n, sizeof(POINT));
    BYTE *t = calloc(n, 1);
    int k = GetPath(dc, p, t, n);
    for (int i = 0; i < k; i++) out("pt %ld %ld %d\n", p[i].x, p[i].y, t[i]);
    free(p);
    free(t);
}

/* pathrgn <s> <r>: PathToRegion into region slot r ("pathrgn 0": failed, no region) */
static void op_pathrgn(void) {
    HDC dc = gdi_dc(1);
    HRGN *r = rgn_slot(2);
    if (bad) return;
    HRGN n = PathToRegion(dc);
    out("pathrgn %d\n", n ? 1 : 0);
    if (!n) return;
    if (*r) DeleteObject(*r);
    *r = n;
}

/* miter <s> <limit>: SetMiterLimit ("miter <ret> <old>") */
static void op_miter(void) {
    HDC dc = gdi_dc(1);
    if (bad || ntok < 3) { bad = 1; return; }
    float old = -1;
    BOOL r = SetMiterLimit(dc, strtof(tok[2], NULL), &old);
    out("miter %d %d\n", r ? 1 : 0, (int)(old * 1000));
}

static const struct kz_op gdipath_ops[] = {
    { "beginpath", op_path },
    { "endpath", op_path },
    { "abortpath", op_path },
    { "closefig", op_path },
    { "flatten", op_path },
    { "widen", op_path },
    { "strokepath", op_path },
    { "fillpath", op_path },
    { "strokefill", op_path },
    { "selclippath", op_path },
    { "getpath", op_getpath },
    { "pathrgn", op_pathrgn },
    { "miter", op_miter },
    { 0 },
};
const struct kz_api gdipath_api = { "gdipath", gdipath_ops, NULL };
