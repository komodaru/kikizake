#include "core.h"
#include <GL/gl.h>
/* OpenGL through WGL on a DIB section (PFD_DRAW_TO_BITMAP): opengl32.dll is loaded when a case
 * first asks for it and every gl / wgl function is looked up by name (only gdi32 is linked; its
 * ChoosePixelFormat / DescribePixelFormat / SetPixelFormat load opengl32 themselves). On a
 * system with no GPU driver (Windows' runner) this is Microsoft's GDI generic OpenGL 1.1. */

#define GLW 16 /* the DIB: 16x16, bottom-up (row 0 is GL's y 0) */
#define GLH 16

static HMODULE gl32;
static HDC gdc;
static HBITMAP gbmp, goldbmp;
static unsigned char *gbits;
static int gbpp;
static HGLRC grc;

static HGLRC (WINAPI *p_create)(HDC);
static BOOL (WINAPI *p_current)(HDC, HGLRC);
static BOOL (WINAPI *p_delete)(HGLRC);
static HGLRC (WINAPI *p_getctx)(void);
static HDC (WINAPI *p_getdc)(void);

static int load_gl(void) {
    if (!gl32) {
        gl32 = LoadLibraryA("opengl32.dll");
        if (!gl32) return 0;
        p_create = (void *)GetProcAddress(gl32, "wglCreateContext");
        p_current = (void *)GetProcAddress(gl32, "wglMakeCurrent");
        p_delete = (void *)GetProcAddress(gl32, "wglDeleteContext");
        p_getctx = (void *)GetProcAddress(gl32, "wglGetCurrentContext");
        p_getdc = (void *)GetProcAddress(gl32, "wglGetCurrentDC");
    }
    return p_create && p_current && p_delete && p_getctx && p_getdc;
}
static FARPROC glf(const char *name) { return gl32 ? GetProcAddress(gl32, name) : NULL; }

/* a memory DC with a 16x16 bottom-up DIB section of bpp (16: 5-5-5; 8: the GDI default table) */
static HDC dib_dc(int bpp, HBITMAP *bmp, HBITMAP *old, unsigned char **bits) {
    struct { BITMAPINFOHEADER h; RGBQUAD pal[256]; } bi;
    memset(&bi, 0, sizeof bi);
    bi.h.biSize = sizeof bi.h;
    bi.h.biWidth = GLW;
    bi.h.biHeight = GLH;
    bi.h.biPlanes = 1;
    bi.h.biBitCount = bpp;
    for (int i = 0; bpp <= 8 && i < 1 << bpp; i++) {
        bi.pal[i].rgbRed = (i >> 5 & 7) * 255 / 7;
        bi.pal[i].rgbGreen = (i >> 2 & 7) * 255 / 7;
        bi.pal[i].rgbBlue = (i & 3) * 255 / 3;
    }
    HDC dc = CreateCompatibleDC(NULL);
    *bmp = CreateDIBSection(dc, (BITMAPINFO *)&bi, DIB_RGB_COLORS, (void **)bits, NULL, 0);
    if (!*bmp) { DeleteDC(dc); return NULL; }
    *old = SelectObject(dc, *bmp);
    return dc;
}
static void free_dc(HDC dc, HBITMAP bmp, HBITMAP old) {
    SelectObject(dc, old);
    DeleteObject(bmp);
    DeleteDC(dc);
}

static void opengl_reset(void) {
    if (grc) { p_current(NULL, NULL); p_delete(grc); grc = NULL; }
    if (gdc) { free_dc(gdc, gbmp, goldbmp); gdc = NULL; }
}

static void print_pfd(int i, const PIXELFORMATDESCRIPTOR *p) {
    out("pf %d %lx %d %d %d,%d %d,%d %d,%d %d,%d accum %d %d,%d,%d,%d depth %d stencil %d aux %d %d %d %lx %lx %lx\n", i, p->dwFlags,
        p->iPixelType, p->cColorBits, p->cRedBits, p->cRedShift, p->cGreenBits, p->cGreenShift, p->cBlueBits, p->cBlueShift,
        p->cAlphaBits, p->cAlphaShift, p->cAccumBits, p->cAccumRedBits, p->cAccumGreenBits, p->cAccumBlueBits, p->cAccumAlphaBits,
        p->cDepthBits, p->cStencilBits, p->cAuxBuffers, p->iLayerType, p->bReserved, p->dwLayerMask, p->dwVisibleMask, p->dwDamageMask);
}

/* glpf <bpp>: DescribePixelFormat of every format on a DIB of bpp */
static void op_glpf(void) {
    int bpp = num(1);
    if (bad) return;
    HBITMAP b, o;
    unsigned char *bits;
    HDC dc = dib_dc(bpp, &b, &o, &bits);
    if (!dc) { out("glpf dib failed\n"); return; }
    PIXELFORMATDESCRIPTOR p;
    int n = DescribePixelFormat(dc, 1, sizeof p, &p);
    out("glpf %d n %d\n", bpp, n);
    for (int i = 1; i <= n; i++) {
        memset(&p, 0xcc, sizeof p);
        if (!DescribePixelFormat(dc, i, sizeof p, &p)) out("pf %d failed\n", i);
        else print_pfd(i, &p);
    }
    free_dc(dc, b, o);
}

/* the descriptor asked for: <flags> <pixel type> <color> <alpha> <accum> <depth> <stencil> from token at */
static void want_pfd(PIXELFORMATDESCRIPTOR *p, int at) {
    memset(p, 0, sizeof *p);
    p->nSize = sizeof *p;
    p->nVersion = 1;
    p->dwFlags = num(at);
    p->iPixelType = num(at + 1);
    p->cColorBits = num(at + 2);
    p->cAlphaBits = num(at + 3);
    p->cAccumBits = num(at + 4);
    p->cDepthBits = num(at + 5);
    p->cStencilBits = num(at + 6);
}

/* glchoose <bpp> <flags> <type> <color> <alpha> <accum> <depth> <stencil>: ChoosePixelFormat */
static void op_glchoose(void) {
    int bpp = num(1);
    PIXELFORMATDESCRIPTOR p;
    want_pfd(&p, 2);
    if (bad || ntok != 9) { bad = 1; return; }
    HBITMAP b, o;
    unsigned char *bits;
    HDC dc = dib_dc(bpp, &b, &o, &bits);
    if (!dc) { out("glchoose dib failed\n"); return; }
    out("glchoose %d\n", ChoosePixelFormat(dc, &p));
    free_dc(dc, b, o);
}

/* glctx <bpp> <pixel format> | glctx <bpp> choose <flags> <type> <color> <alpha> <accum> <depth> <stencil>:
 * the DIB, the format given or ChoosePixelFormat's, SetPixelFormat, wglCreateContext, wglMakeCurrent */
static void op_glctx(void) {
    opengl_reset();
    int bpp = num(1), pf = 0;
    PIXELFORMATDESCRIPTOR want;
    if (is(2, "choose")) { want_pfd(&want, 3); if (ntok != 10) bad = 1; }
    else { pf = num(2); if (ntok != 3) bad = 1; }
    if (bad) return;
    if (!load_gl()) { out("glctx none\n"); return; }
    gdc = dib_dc(bpp, &gbmp, &goldbmp, &gbits);
    if (!gdc) { out("glctx dib failed\n"); return; }
    gbpp = bpp;
    if (!pf) pf = ChoosePixelFormat(gdc, &want);
    PIXELFORMATDESCRIPTOR p;
    DescribePixelFormat(gdc, pf, sizeof p, &p);
    if (!SetPixelFormat(gdc, pf, &p)) { out("glctx set failed %lu\n", GetLastError()); return; }
    grc = p_create(gdc);
    if (!grc) { out("glctx create failed %lu\n", GetLastError()); return; }
    if (!p_current(gdc, grc)) { out("glctx current failed %lu\n", GetLastError()); return; }
    out("glctx ok %d\n", GetPixelFormat(gdc));
}

/* the calls `gl <name> <args>` may name, with their argument types: i integer (any integer
 * type, enums), f GLfloat, d GLdouble */
static const struct { const char *name, *sig; } glcalls[] = {
    { "glLoadIdentity", "" }, { "glPushMatrix", "" }, { "glPopMatrix", "" }, { "glEnd", "" }, { "glFlush", "" },
    { "glFinish", "" }, { "glPopAttrib", "" },
    { "glEnable", "i" }, { "glDisable", "i" }, { "glMatrixMode", "i" }, { "glBegin", "i" }, { "glClear", "i" },
    { "glLogicOp", "i" }, { "glDepthFunc", "i" }, { "glShadeModel", "i" }, { "glCullFace", "i" }, { "glFrontFace", "i" },
    { "glStencilMask", "i" }, { "glDepthMask", "i" }, { "glPushAttrib", "i" }, { "glDrawBuffer", "i" },
    { "glReadBuffer", "i" }, { "glClearStencil", "i" }, { "glIndexMask", "i" }, { "glIndexi", "i" },
    { "glBlendFunc", "ii" }, { "glPixelStorei", "ii" }, { "glHint", "ii" }, { "glPolygonMode", "ii" }, { "glVertex2i", "ii" },
    { "glPixelTransferi", "ii" },
    { "glStencilOp", "iii" }, { "glStencilFunc", "iii" }, { "glColor3ub", "iii" }, { "glVertex3i", "iii" },
    { "glViewport", "iiii" }, { "glScissor", "iiii" }, { "glColorMask", "iiii" }, { "glColor4ub", "iiii" }, { "glRecti", "iiii" },
    { "glLineWidth", "f" }, { "glPointSize", "f" }, { "glClearIndex", "f" },
    { "glVertex2f", "ff" }, { "glPolygonOffset", "ff" },
    { "glTranslatef", "fff" }, { "glScalef", "fff" }, { "glColor3f", "fff" }, { "glVertex3f", "fff" }, { "glNormal3f", "fff" },
    { "glClearColor", "ffff" }, { "glColor4f", "ffff" }, { "glRotatef", "ffff" }, { "glRectf", "ffff" }, { "glClearAccum", "ffff" },
    { "glVertex4f", "ffff" },
    { "glClearDepth", "d" }, { "glDepthRange", "dd" }, { "glOrtho", "dddddd" }, { "glFrustum", "dddddd" },
    { "glAlphaFunc", "if" }, { "glPixelStoref", "if" }, { "glPixelTransferf", "if" }, { "glAccum", "if" }, { "glFogf", "if" },
    { "glPixelZoom", "ff" }, { "glRasterPos2i", "ii" }, { "glRasterPos2f", "ff" },
    { 0 },
};

static double fl(int i) {
    if (i >= ntok) { bad = 1; return 0; }
    char *e;
    double v = strtod(tok[i], &e);
    if (*e) bad = 1;
    return v;
}

#define NOCTX() if (!grc) { out("noctx\n"); return; }

/* gl <name> <args> */
static void op_gl(void) {
    NOCTX();
    int k = 0;
    while (glcalls[k].name && !is(1, glcalls[k].name)) k++;
    if (!glcalls[k].name) { bad = 1; return; }
    const char *s = glcalls[k].sig;
    if (ntok != 2 + (int)strlen(s)) { bad = 1; return; }
    long a[6];
    float f[6];
    double d[6];
    for (int i = 0; s[i]; i++) {
        if (s[i] == 'i') a[i] = num(2 + i);
        else f[i] = d[i] = fl(2 + i);
    }
    if (bad) return;
    FARPROC p = glf(glcalls[k].name);
    if (!p) { out("gl %s none\n", glcalls[k].name); return; }
#define SIG(str) (!strcmp(s, str))
    if (SIG("")) ((void (WINAPI *)(void))p)();
    else if (SIG("i")) ((void (WINAPI *)(long))p)(a[0]);
    else if (SIG("ii")) ((void (WINAPI *)(long, long))p)(a[0], a[1]);
    else if (SIG("iii")) ((void (WINAPI *)(long, long, long))p)(a[0], a[1], a[2]);
    else if (SIG("iiii")) ((void (WINAPI *)(long, long, long, long))p)(a[0], a[1], a[2], a[3]);
    else if (SIG("f")) ((void (WINAPI *)(float))p)(f[0]);
    else if (SIG("ff")) ((void (WINAPI *)(float, float))p)(f[0], f[1]);
    else if (SIG("fff")) ((void (WINAPI *)(float, float, float))p)(f[0], f[1], f[2]);
    else if (SIG("ffff")) ((void (WINAPI *)(float, float, float, float))p)(f[0], f[1], f[2], f[3]);
    else if (SIG("d")) ((void (WINAPI *)(double))p)(d[0]);
    else if (SIG("dd")) ((void (WINAPI *)(double, double))p)(d[0], d[1]);
    else if (SIG("dddddd")) ((void (WINAPI *)(double, double, double, double, double, double))p)(d[0], d[1], d[2], d[3], d[4], d[5]);
    else if (SIG("if")) ((void (WINAPI *)(long, float))p)(a[0], f[1]);
    else bad = 1;
#undef SIG
}

/* glmat load|mult <16 floats>: glLoadMatrixf / glMultMatrixf (column-major, as GL takes them) */
static void op_glmat(void) {
    NOCTX();
    float m[16];
    for (int i = 0; i < 16; i++) m[i] = fl(2 + i);
    if (bad || ntok != 18 || !(is(1, "load") || is(1, "mult"))) { bad = 1; return; }
    FARPROC p = glf(is(1, "load") ? "glLoadMatrixf" : "glMultMatrixf");
    ((void (WINAPI *)(const float *))p)(m);
}

/* glfogv <pname> <1..4 floats>: glFogfv with the floats, the rest of four 0 */
static void op_glfogv(void) {
    NOCTX();
    float v[4] = { 0, 0, 0, 0 };
    if (ntok < 3 || ntok > 6) { bad = 1; return; }
    long p = num(1);
    for (int i = 2; i < ntok; i++) v[i - 2] = (float)fl(i);
    if (bad) return;
    ((void (WINAPI *)(long, const float *))glf("glFogfv"))(p, v);
}

/* glerr: glGetError until GL_NO_ERROR (at most 8): "glerr <codes...>", or "glerr 0" */
static void op_glerr(void) {
    NOCTX();
    GLenum (WINAPI *p)(void) = (void *)glf("glGetError");
    out("glerr");
    int n = 0;
    for (GLenum e; n < 8 && (e = p()); n++) out(" %x", e);
    out(n ? "\n" : " 0\n");
}

/* glget i|f|b|d <pname> <n>: glGet{Integer,Float,Boolean,Double}v into an array of n (1..16)
 * filled with 0xcc first: "get <pname> = <n values>", an untouched value as "-" (i decimal,
 * f the float's bits, b the byte, d the double's bits) */
static void op_glget(void) {
    NOCTX();
    long pname = num(2), n = num(3);
    if (bad || n < 1 || n > 16) { bad = 1; return; }
    union { GLint i[16]; GLfloat f[16]; GLboolean b[16]; GLdouble d[16]; unsigned char raw[128]; } u;
    memset(&u, 0xcc, sizeof u);
    int sz;
    if (is(1, "i")) { ((void (WINAPI *)(GLenum, GLint *))glf("glGetIntegerv"))(pname, u.i); sz = 4; }
    else if (is(1, "f")) { ((void (WINAPI *)(GLenum, GLfloat *))glf("glGetFloatv"))(pname, u.f); sz = 4; }
    else if (is(1, "b")) { ((void (WINAPI *)(GLenum, GLboolean *))glf("glGetBooleanv"))(pname, u.b); sz = 1; }
    else if (is(1, "d")) { ((void (WINAPI *)(GLenum, GLdouble *))glf("glGetDoublev"))(pname, u.d); sz = 8; }
    else { bad = 1; return; }
    out("get %lx =", pname);
    for (int k = 0; k < n; k++) {
        unsigned char *e = u.raw + k * sz;
        int untouched = 1;
        for (int j = 0; j < sz; j++) untouched &= e[j] == 0xcc;
        if (untouched) { out(" -"); continue; }
        if (sz == 1) out(" %d", e[0]);
        else if (is(1, "i")) out(" %ld", (long)u.i[k]);
        else if (sz == 4) out(" %08lx", *(DWORD *)e);
        else out(" %08lx%08lx", ((DWORD *)e)[1], ((DWORD *)e)[0]);
    }
    out("\n");
}

/* glisen <cap>: glIsEnabled */
static void op_glisen(void) {
    NOCTX();
    long cap = num(1);
    if (bad) return;
    out("isen %lx = %d\n", cap, ((GLboolean (WINAPI *)(GLenum))glf("glIsEnabled"))(cap));
}

/* glstr <name>: glGetString */
static void op_glstr(void) {
    NOCTX();
    long name = num(1);
    if (bad) return;
    const char *s = (const char *)((const GLubyte *(WINAPI *)(GLenum))glf("glGetString"))(name);
    out("str %lx = %s\n", name, s ? s : "(null)");
}

/* glread <x> <y> <w> <h> <format> <type> <bytes>: glReadPixels into <bytes> (1..65536) bytes filled
 * with 0xcc and 64 more: "read" and the bytes as hex, then "over" if any of the 64 changed */
static unsigned char rbuf[65536 + 64];
static void op_glread(void) {
    NOCTX();
    long x = num(1), y = num(2), w = num(3), h = num(4), f = num(5), t = num(6), n = num(7);
    if (bad || n < 1 || n > 65536) { bad = 1; return; }
    memset(rbuf, 0xcc, sizeof rbuf);
    ((void (WINAPI *)(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, void *))glf("glReadPixels"))(x, y, w, h, f, t, rbuf);
    out("read ");
    out_hex(rbuf, n);
    for (int i = 0; i < 64; i++)
        if (rbuf[n + i] != 0xcc) { out("over\n"); break; }
}

/* gldraw <w> <h> <format> <type> <seed> <bytes>: glDrawPixels of <bytes> bytes from xorshift32 */
static void op_gldraw(void) {
    NOCTX();
    long w = num(1), h = num(2), f = num(3), t = num(4), seed = num(5), n = num(6);
    if (bad || n < 1 || n > 65536) { bad = 1; return; }
    unsigned x = seed ? (unsigned)seed : 0x9e3779b9;
    for (int i = 0; i < n; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        rbuf[i] = x;
    }
    ((void (WINAPI *)(GLsizei, GLsizei, GLenum, GLenum, const void *))glf("glDrawPixels"))(w, h, f, t, rbuf);
}

/* gldib: glFinish, GdiFlush, then the DIB: "px gl 16 16 <bpp>" and each row, bottom row first */
static void op_gldib(void) {
    NOCTX();
    ((void (WINAPI *)(void))glf("glFinish"))();
    GdiFlush();
    out("px gl %d %d %d\n", GLW, GLH, gbpp);
    int stride = (GLW * gbpp + 31) / 32 * 4;
    for (int y = 0; y < GLH; y++) out_hex(gbits + y * stride, GLW * gbpp / 8);
}

/* glcur: wglGetCurrentContext / DC are this case's (1) or none (0) */
static void op_glcur(void) {
    if (!load_gl()) { out("glcur none\n"); return; }
    HGLRC c = p_getctx();
    HDC d = p_getdc();
    out("glcur %d %d\n", c ? (c == grc ? 1 : 2) : 0, d ? (d == gdc ? 1 : 2) : 0);
}

/* glrelease: wglMakeCurrent(NULL, NULL), wglDeleteContext; "glrelease <ret> <ret>" */
static void op_glrelease(void) {
    NOCTX();
    BOOL a = p_current(NULL, NULL), b = p_delete(grc);
    grc = NULL;
    out("glrelease %d %d\n", a, b);
}

static const struct kz_op opengl_ops[] = {
    { "glpf", op_glpf },
    { "glchoose", op_glchoose },
    { "glctx", op_glctx },
    { "gl", op_gl },
    { "glmat", op_glmat },
    { "glfogv", op_glfogv },
    { "glerr", op_glerr },
    { "glget", op_glget },
    { "glisen", op_glisen },
    { "glstr", op_glstr },
    { "glread", op_glread },
    { "gldraw", op_gldraw },
    { "gldib", op_gldib },
    { "glcur", op_glcur },
    { "glrelease", op_glrelease },
    { 0 },
};
const struct kz_api opengl_api = { "opengl", opengl_ops, opengl_reset };
