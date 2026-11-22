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

/* arrays a call reads (glp): 16 slots used in turn from the first at each glctx, so an array pointer
 * kept by GL (glVertexPointer ..) stays valid for the next 15 glp lines and glGetPointerv's slot
 * depends on the case alone */
static union { unsigned char b[4096]; double d[512]; } pslot[16];
static int pnext;

/* glctx <bpp> <pixel format> | glctx <bpp> choose <flags> <type> <color> <alpha> <accum> <depth> <stencil>:
 * the DIB, the format given or ChoosePixelFormat's, SetPixelFormat, wglCreateContext, wglMakeCurrent */
static void op_glctx(void) {
    opengl_reset();
    pnext = 0;
    memset(pslot, 0, sizeof pslot);
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
    { "glPixelTransferi", "ii" }, { "glLineStipple", "ii" },
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
    /* texturing (GL 1.1 3.8) */
    { "glBindTexture", "ii" }, { "glTexParameteri", "iii" }, { "glTexParameterf", "iif" }, { "glTexEnvi", "iii" },
    { "glTexEnvf", "iif" }, { "glTexGeni", "iii" }, { "glTexGenf", "iif" }, { "glTexGend", "iid" },
    { "glTexCoord1f", "f" }, { "glTexCoord2f", "ff" }, { "glTexCoord3f", "fff" }, { "glTexCoord4f", "ffff" },
    { "glCopyTexImage1D", "iiiiiii" }, { "glCopyTexImage2D", "iiiiiiii" }, { "glCopyTexSubImage1D", "iiiiii" },
    { "glCopyTexSubImage2D", "iiiiiiii" },
    /* display lists, arrays, evaluators, selection, lights, pixels and other forms (spec/opengl.txt
     * GL.LIST .. GL.VARIANTS) */
    { "glNewList", "ii" }, { "glEndList", "" }, { "glCallList", "i" }, { "glDeleteLists", "ii" }, { "glListBase", "i" },
    { "glEnableClientState", "i" }, { "glDisableClientState", "i" }, { "glArrayElement", "i" }, { "glDrawArrays", "iii" },
    { "glPushClientAttrib", "i" }, { "glPopClientAttrib", "" }, { "glEdgeFlag", "i" },
    { "glEvalCoord1f", "f" }, { "glEvalCoord2f", "ff" }, { "glEvalCoord1d", "d" }, { "glEvalCoord2d", "dd" },
    { "glEvalPoint1", "i" }, { "glEvalPoint2", "ii" }, { "glEvalMesh1", "iii" }, { "glEvalMesh2", "iiiii" },
    { "glMapGrid1f", "iff" }, { "glMapGrid2f", "iffiff" }, { "glMapGrid1d", "idd" }, { "glMapGrid2d", "iddidd" },
    { "glInitNames", "" }, { "glLoadName", "i" }, { "glPushName", "i" }, { "glPopName", "" }, { "glPassThrough", "f" },
    { "glLightf", "iif" }, { "glLighti", "iii" }, { "glLightModelf", "if" }, { "glLightModeli", "ii" },
    { "glMaterialf", "iif" }, { "glMateriali", "iii" }, { "glColorMaterial", "ii" },
    { "glCopyPixels", "iiiii" }, { "glFogi", "ii" },
    { "glColor3b", "iii" }, { "glColor3d", "ddd" }, { "glColor3i", "iii" }, { "glColor3s", "iii" }, { "glColor3ui", "iii" }, { "glColor3us", "iii" },
    { "glColor4b", "iiii" }, { "glColor4d", "dddd" }, { "glColor4i", "iiii" }, { "glColor4s", "iiii" }, { "glColor4ui", "iiii" }, { "glColor4us", "iiii" },
    { "glIndexd", "d" }, { "glIndexf", "f" }, { "glIndexs", "i" }, { "glIndexub", "i" },
    { "glNormal3b", "iii" }, { "glNormal3d", "ddd" }, { "glNormal3i", "iii" }, { "glNormal3s", "iii" },
    { "glVertex2d", "dd" }, { "glVertex2s", "ii" }, { "glVertex3d", "ddd" }, { "glVertex3s", "iii" }, { "glVertex4d", "dddd" }, { "glVertex4i", "iiii" }, { "glVertex4s", "iiii" },
    { "glRasterPos2d", "dd" }, { "glRasterPos2s", "ii" }, { "glRasterPos3d", "ddd" }, { "glRasterPos3f", "fff" }, { "glRasterPos3i", "iii" }, { "glRasterPos3s", "iii" },
    { "glRasterPos4d", "dddd" }, { "glRasterPos4f", "ffff" }, { "glRasterPos4i", "iiii" }, { "glRasterPos4s", "iiii" },
    { "glRectd", "dddd" }, { "glRects", "iiii" },
    { "glTexCoord1d", "d" }, { "glTexCoord1i", "i" }, { "glTexCoord1s", "i" }, { "glTexCoord2d", "dd" }, { "glTexCoord2i", "ii" }, { "glTexCoord2s", "ii" },
    { "glTexCoord3d", "ddd" }, { "glTexCoord3i", "iii" }, { "glTexCoord3s", "iii" }, { "glTexCoord4d", "dddd" }, { "glTexCoord4i", "iiii" }, { "glTexCoord4s", "iiii" },
    { "glRotated", "dddd" }, { "glScaled", "ddd" }, { "glTranslated", "ddd" },
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
    long a[8];
    float f[8];
    double d[8];
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
    else if (SIG("iif")) ((void (WINAPI *)(long, long, float))p)(a[0], a[1], f[2]);
    else if (SIG("iid")) ((void (WINAPI *)(long, long, double))p)(a[0], a[1], d[2]);
    else if (SIG("iiiiii")) ((void (WINAPI *)(long, long, long, long, long, long))p)(a[0], a[1], a[2], a[3], a[4], a[5]);
    else if (SIG("iiiiiii")) ((void (WINAPI *)(long, long, long, long, long, long, long))p)(a[0], a[1], a[2], a[3], a[4], a[5], a[6]);
    else if (SIG("iiiiiiii")) ((void (WINAPI *)(long, long, long, long, long, long, long, long))p)(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]);
    else if (SIG("iiiii")) ((void (WINAPI *)(long, long, long, long, long))p)(a[0], a[1], a[2], a[3], a[4]);
    else if (SIG("iff")) ((void (WINAPI *)(long, float, float))p)(a[0], f[1], f[2]);
    else if (SIG("iffiff")) ((void (WINAPI *)(long, float, float, long, float, float))p)(a[0], f[1], f[2], a[3], f[4], f[5]);
    else if (SIG("idd")) ((void (WINAPI *)(long, double, double))p)(a[0], d[1], d[2]);
    else if (SIG("iddidd")) ((void (WINAPI *)(long, double, double, long, double, double))p)(a[0], d[1], d[2], a[3], d[4], d[5]);
    else if (SIG("ddd")) ((void (WINAPI *)(double, double, double))p)(d[0], d[1], d[2]);
    else if (SIG("dddd")) ((void (WINAPI *)(double, double, double, double))p)(d[0], d[1], d[2], d[3]);
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

/* fill rbuf's first n bytes from xorshift32 seeded with seed (0: the golden ratio), zeros after */
static void fill_rbuf(long seed, long n) {
    unsigned x = seed ? (unsigned)seed : 0x9e3779b9;
    memset(rbuf, 0, sizeof rbuf);
    for (int i = 0; i < n; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        rbuf[i] = x;
    }
}

/* gldraw <w> <h> <format> <type> <seed> <bytes>: glDrawPixels of <bytes> bytes from xorshift32,
 * zeros after them to the end of rbuf (what an image laid out past its bytes reads) */
static void op_gldraw(void) {
    NOCTX();
    long w = num(1), h = num(2), f = num(3), t = num(4), seed = num(5), n = num(6);
    if (bad || n < 1 || n > 65536) { bad = 1; return; }
    fill_rbuf(seed, n);
    ((void (WINAPI *)(GLsizei, GLsizei, GLenum, GLenum, const void *))glf("glDrawPixels"))(w, h, f, t, rbuf);
}

/* gltex <target> <level> <internal format> <w> <h> <border> <format> <type> <seed> <bytes>:
 * glTexImage1D (target GL_TEXTURE_1D, h unused) or glTexImage2D of <bytes> bytes from
 * xorshift32 (0 bytes: a null pointer) */
static void op_gltex(void) {
    NOCTX();
    long tg = num(1), lv = num(2), ifmt = num(3), w = num(4), h = num(5), b = num(6), f = num(7), t = num(8), seed = num(9), n = num(10);
    if (bad || ntok != 11 || n < 0 || n > 65536) { bad = 1; return; }
    fill_rbuf(seed, n);
    const void *px = n ? rbuf : NULL;
    if (tg == 0x0de0) ((void (WINAPI *)(GLenum, GLint, GLint, GLsizei, GLint, GLenum, GLenum, const void *))glf("glTexImage1D"))(tg, lv, ifmt, w, b, f, t, px);
    else ((void (WINAPI *)(GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, const void *))glf("glTexImage2D"))(tg, lv, ifmt, w, h, b, f, t, px);
}

/* gltexsub <target> <level> <x> <y> <w> <h> <format> <type> <seed> <bytes>: glTexSubImage1D
 * (GL_TEXTURE_1D, y and h unused) or glTexSubImage2D */
static void op_gltexsub(void) {
    NOCTX();
    long tg = num(1), lv = num(2), x = num(3), y = num(4), w = num(5), h = num(6), f = num(7), t = num(8), seed = num(9), n = num(10);
    if (bad || ntok != 11 || n < 1 || n > 65536) { bad = 1; return; }
    fill_rbuf(seed, n);
    if (tg == 0x0de0) ((void (WINAPI *)(GLenum, GLint, GLint, GLsizei, GLenum, GLenum, const void *))glf("glTexSubImage1D"))(tg, lv, x, w, f, t, rbuf);
    else ((void (WINAPI *)(GLenum, GLint, GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, const void *))glf("glTexSubImage2D"))(tg, lv, x, y, w, h, f, t, rbuf);
}

/* glteximg <target> <level> <format> <type> <bytes>: glGetTexImage into rbuf, "read <hex>" as
 * glread, "over" if it wrote past <bytes> */
static void op_glteximg(void) {
    NOCTX();
    long tg = num(1), lv = num(2), f = num(3), t = num(4), n = num(5);
    if (bad || ntok != 6 || n < 1 || n > 65536) { bad = 1; return; }
    memset(rbuf, 0xcc, sizeof rbuf);
    ((void (WINAPI *)(GLenum, GLint, GLenum, GLenum, void *))glf("glGetTexImage"))(tg, lv, f, t, rbuf);
    out("read ");
    out_hex(rbuf, n);
    for (int i = 0; i < 64; i++)
        if (rbuf[n + i] != 0xcc) { out("over\n"); break; }
}

/* gltexq param|level|env|gen <target or coord> [<level>] <pname> i|f <n>: glGetTexParameter,
 * glGetTexLevelParameter, glGetTexEnv, glGetTexGen (iv or fv, dv for gen d), "texq <values>" */
static void op_gltexq(void) {
    NOCTX();
    int lvl = is(1, "level"), at = 2;
    long tg = num(at++), lv = lvl ? num(at++) : 0, pn = num(at++);
    int isf = is(at, "f"), isd = is(at, "d");
    if (!isf && !isd && !is(at, "i")) bad = 1;
    long n = num(at + 1);
    if (bad || ntok != at + 2 || n < 1 || n > 16) { bad = 1; return; }
    const char *base = is(1, "param") ? "glGetTexParameter" : lvl ? "glGetTexLevelParameter" : is(1, "env") ? "glGetTexEnv" : is(1, "gen") ? "glGetTexGen" : NULL;
    if (!base || (isd && !is(1, "gen"))) { bad = 1; return; }
    char name[64];
    snprintf(name, sizeof name, "%s%s", base, isd ? "dv" : isf ? "fv" : "iv");
    FARPROC p = glf(name);
    GLint iv[16]; GLfloat fv[16]; GLdouble dv[16];
    for (int i = 0; i < 16; i++) { iv[i] = 0x7eadbeef; fv[i] = -12345.0f; dv[i] = -12345.0; }
    void *v = isd ? (void *)dv : isf ? (void *)fv : (void *)iv;
    if (lvl) ((void (WINAPI *)(GLenum, GLint, GLenum, void *))p)(tg, lv, pn, v);
    else ((void (WINAPI *)(GLenum, GLenum, void *))p)(tg, pn, v);
    out("texq");
    for (int i = 0; i < n; i++) {
        if (isd) out(" %.9g", dv[i]);
        else if (isf) out(" %.9g", fv[i]);
        else out(" %ld", (long)iv[i]);
    }
    out("\n");
}

/* gltexv param|env|gen <target or coord> <pname> i|f|d <1..4 values>: glTexParameter, glTexEnv,
 * glTexGen in their iv, fv or dv forms */
static void op_gltexv(void) {
    NOCTX();
    if (ntok < 6 || ntok > 9) { bad = 1; return; }
    long tg = num(2), pn = num(3);
    int isf = is(4, "f"), isd = is(4, "d");
    if (!isf && !isd && !is(4, "i")) bad = 1;
    GLint iv[4] = { 0, 0, 0, 0 }; GLfloat fv[4] = { 0, 0, 0, 0 }; GLdouble dv[4] = { 0, 0, 0, 0 };
    for (int i = 5; i < ntok; i++) {
        if (isf || isd) { dv[i - 5] = fl(i); fv[i - 5] = (float)dv[i - 5]; }
        else iv[i - 5] = num(i);
    }
    const char *base = is(1, "param") ? "glTexParameter" : is(1, "env") ? "glTexEnv" : is(1, "gen") ? "glTexGen" : NULL;
    if (bad || !base || (isd && !is(1, "gen"))) { bad = 1; return; }
    char name[64];
    snprintf(name, sizeof name, "%s%s", base, isd ? "dv" : isf ? "fv" : "iv");
    void *v = isd ? (void *)dv : isf ? (void *)fv : (void *)iv;
    ((void (WINAPI *)(GLenum, GLenum, const void *))glf(name))(tg, pn, v);
}

/* glgentex <n>: glGenTextures, "tex <names>"; gldeltex <names>: glDeleteTextures;
 * glistex <name>: "istex <glIsTexture>"; glprio <prio> <names>: glPrioritizeTextures (each the one
 * priority); glresident <names>: "resident <ret> <flags>" from glAreTexturesResident */
static void op_glgentex(void) {
    NOCTX();
    long n = num(1);
    if (bad || ntok != 2 || n < 0 || n > 16) { bad = 1; return; }
    GLuint v[16] = { 0 };
    ((void (WINAPI *)(GLsizei, GLuint *))glf("glGenTextures"))(n, v);
    out("tex");
    for (int i = 0; i < n; i++) out(" %lu", (unsigned long)v[i]);
    out("\n");
}

static int read_names(int from, GLuint *v) {
    int n = 0;
    for (int i = from; i < ntok && n < 16; i++) v[n++] = (GLuint)num(i);
    if (ntok - from > 16) bad = 1;
    return n;
}

static void op_gldeltex(void) {
    NOCTX();
    GLuint v[16];
    int n = read_names(1, v);
    if (bad) return;
    ((void (WINAPI *)(GLsizei, const GLuint *))glf("glDeleteTextures"))(n, v);
}

static void op_glistex(void) {
    NOCTX();
    long t = num(1);
    if (bad || ntok != 2) { bad = 1; return; }
    out("istex %d\n", (int)((GLboolean (WINAPI *)(GLuint))glf("glIsTexture"))((GLuint)t));
}

static void op_glprio(void) {
    NOCTX();
    float pr = (float)fl(1);
    GLuint v[16];
    GLclampf p[16];
    int n = read_names(2, v);
    if (bad) return;
    for (int i = 0; i < n; i++) p[i] = pr;
    ((void (WINAPI *)(GLsizei, const GLuint *, const GLclampf *))glf("glPrioritizeTextures"))(n, v, p);
}

static void op_glresident(void) {
    NOCTX();
    GLuint v[16];
    GLboolean r[16];
    int n = read_names(1, v);
    if (bad) return;
    memset(r, 0x5a, sizeof r);
    GLboolean ret = ((GLboolean (WINAPI *)(GLsizei, const GLuint *, GLboolean *))glf("glAreTexturesResident"))(n, v, r);
    out("resident %d", (int)ret);
    for (int i = 0; i < n; i++) out(" %d", (int)r[i]);
    out("\n");
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

/* obuf: the buffer GL writes into (glSelectBuffer, glFeedbackBuffer), shown by globuf */
static union { unsigned char b[4096]; GLuint u[1024]; GLfloat f[1024]; } obuf;

/* the size of an element of an array type: b ub s us i ui f d */
static int elem_size(const char *t) {
    if (!strcmp(t, "b") || !strcmp(t, "ub")) return 1;
    if (!strcmp(t, "s") || !strcmp(t, "us")) return 2;
    if (!strcmp(t, "i") || !strcmp(t, "ui") || !strcmp(t, "f")) return 4;
    if (!strcmp(t, "d")) return 8;
    return 0;
}

/* the leading arguments of a sig at tok[at..]: a ints, f floats, d doubles */
static int lead_args(const char *sig, int at, long *a, float *f, double *d) {
    if (!strcmp(sig, "-")) return 0;
    int n = (int)strlen(sig);
    if (n > 9) { bad = 1; return 0; }
    for (int i = 0; i < n; i++) {
        if (sig[i] == 'i') a[i] = num(at + i);
        else if (sig[i] == 'f' || sig[i] == 'd') f[i] = (float)(d[i] = fl(at + i));
        else bad = 1;
    }
    return n;
}

/* a call of a sig and a trailing pointer q (a void return) */
static void call_lead(FARPROC p, const char *s, const long *a, const float *f, const double *d, void *q) {
#define SIG(str) (!strcmp(s, str))
    if (SIG("-")) ((void (WINAPI *)(void *))p)(q);
    else if (SIG("i")) ((void (WINAPI *)(long, void *))p)(a[0], q);
    else if (SIG("ii")) ((void (WINAPI *)(long, long, void *))p)(a[0], a[1], q);
    else if (SIG("iii")) ((void (WINAPI *)(long, long, long, void *))p)(a[0], a[1], a[2], q);
    else if (SIG("iiii")) ((void (WINAPI *)(long, long, long, long, void *))p)(a[0], a[1], a[2], a[3], q);
    else if (SIG("iiiii")) ((void (WINAPI *)(long, long, long, long, long, void *))p)(a[0], a[1], a[2], a[3], a[4], q);
    else if (SIG("iiiiii")) ((void (WINAPI *)(long, long, long, long, long, long, void *))p)(a[0], a[1], a[2], a[3], a[4], a[5], q);
    else if (SIG("iiiiiii")) ((void (WINAPI *)(long, long, long, long, long, long, long, void *))p)(a[0], a[1], a[2], a[3], a[4], a[5], a[6], q);
    else if (SIG("ffff")) ((void (WINAPI *)(float, float, float, float, void *))p)(f[0], f[1], f[2], f[3], q);
    else if (SIG("iiffff")) ((void (WINAPI *)(long, long, float, float, float, float, void *))p)(a[0], a[1], f[2], f[3], f[4], f[5], q);
    else if (SIG("iffii")) ((void (WINAPI *)(long, float, float, long, long, void *))p)(a[0], f[1], f[2], a[3], a[4], q);
    else if (SIG("iddii")) ((void (WINAPI *)(long, double, double, long, long, void *))p)(a[0], d[1], d[2], a[3], a[4], q);
    else if (SIG("iffiiffii")) ((void (WINAPI *)(long, float, float, long, long, float, float, long, long, void *))p)(a[0], f[1], f[2], a[3], a[4], f[5], f[6], a[7], a[8], q);
    else if (SIG("iddiiddii")) ((void (WINAPI *)(long, double, double, long, long, double, double, long, long, void *))p)(a[0], d[1], d[2], a[3], a[4], d[5], d[6], a[7], a[8], q);
    else bad = 1;
#undef SIG
}

/* glp <name> <sig> <leading args> <type> <values> | glp <name> <sig> <leading args> obuf | glp
 * <name> <sig> <leading args> null: name(<leading args>, a pointer to the values as an array of
 * type, to obuf (filled with 0xcc first), or NULL); sig "-" for none (glColor3fv, glLightfv,
 * glVertexPointer, glCallLists, glDrawElements, glMap1f, glSelectBuffer, ..) */
static void op_glp(void) {
    NOCTX();
    if (ntok < 3) { bad = 1; return; }
    const char *sig = tok[2];
    long a[9]; float f[9]; double d[9];
    int n = lead_args(sig, 3, a, f, d), at = 3 + n;
    if (bad || at >= ntok) { bad = 1; return; }
    void *q;
    if (is(at, "obuf")) { memset(obuf.b, 0xcc, sizeof obuf.b); q = obuf.b; if (ntok != at + 1) bad = 1; }
    else if (is(at, "null")) { q = NULL; if (ntok != at + 1) bad = 1; }
    else {
        int sz = elem_size(tok[at]), cnt = ntok - at - 1;
        if (!sz || cnt * sz > 4096) { bad = 1; return; }
        unsigned char *b = pslot[pnext].b;
        pnext = (pnext + 1) % 16;
        memset(b, 0, 4096);
        for (int i = 0; i < cnt; i++) {
            const char *t = tok[at];
            if (!strcmp(t, "f")) { float v = (float)fl(at + 1 + i); memcpy(b + 4 * i, &v, 4); }
            else if (!strcmp(t, "d")) { double v = fl(at + 1 + i); memcpy(b + 8 * i, &v, 8); }
            else { long v = num(at + 1 + i); memcpy(b + sz * i, &v, sz); }
        }
        q = b;
    }
    if (bad) return;
    FARPROC p = glf(tok[1]);
    if (!p) { out("glp %s none\n", tok[1]); return; }
    /* (glRect*v take two pointers, the corners: the array's first and second halves) */
    size_t len = strlen(tok[1]);
    if (!strncmp(tok[1], "glRect", 6) && len > 7 && tok[1][len - 1] == 'v') {
        if (!q || strcmp(sig, "-")) { bad = 1; return; }
        int sz = elem_size(tok[at]);
        ((void (WINAPI *)(const void *, const void *))p)(q, (unsigned char *)q + 2 * sz);
        return;
    }
    call_lead(p, sig, a, f, d, q);
}

/* glq <name> <sig> <leading args> <type> <n>: name(<leading args>, out) into an array of type filled
 * with a sentinel first (0x7eadbeef for i ui, -12345 for f d, 0xcc bytes for b ub s us; p: pointers,
 * shown as the glp slot they point into, 0 for NULL, -1 for elsewhere); "q <n values>" */
static void op_glq(void) {
    NOCTX();
    if (ntok < 5) { bad = 1; return; }
    const char *sig = tok[2];
    long a[9]; float f[9]; double d[9];
    int k = lead_args(sig, 3, a, f, d), at = 3 + k;
    if (bad || ntok != at + 2) { bad = 1; return; }
    const char *t = tok[at];
    long n = num(at + 1);
    int sz = is(at, "p") ? (int)sizeof(void *) : elem_size(t);
    if (bad || !sz || n < 1 || n * sz > 4096) { bad = 1; return; }
    static union { unsigned char b[4096 + 64]; double d[520]; } v;
    for (int i = 0; i < 4096 / 4; i++) {
        if (!strcmp(t, "i") || !strcmp(t, "ui")) { unsigned w = 0x7eadbeef; memcpy(v.b + 4 * i, &w, 4); }
        else if (!strcmp(t, "f")) { float w = -12345.0f; memcpy(v.b + 4 * i, &w, 4); }
        else memset(v.b + 4 * i, 0xcc, 4);
    }
    if (!strcmp(t, "d")) for (int i = 0; i < 512; i++) v.d[i] = -12345.0;
    if (is(at, "p")) memset(v.b, 0, 4096);
    FARPROC p = glf(tok[1]);
    if (!p) { out("glq %s none\n", tok[1]); return; }
    call_lead(p, sig, a, f, d, v.b);
    out("q");
    for (long i = 0; i < n; i++) {
        if (!strcmp(t, "f")) { float w; memcpy(&w, v.b + 4 * i, 4); out(" %.9g", w); }
        else if (!strcmp(t, "d")) out(" %.9g", v.d[i]);
        else if (!strcmp(t, "i")) { int w; memcpy(&w, v.b + 4 * i, 4); out(" %d", w); }
        else if (!strcmp(t, "ui")) { unsigned w; memcpy(&w, v.b + 4 * i, 4); out(" %u", w); }
        else if (!strcmp(t, "s")) { short w; memcpy(&w, v.b + 2 * i, 2); out(" %d", w); }
        else if (!strcmp(t, "us")) { unsigned short w; memcpy(&w, v.b + 2 * i, 2); out(" %u", w); }
        else if (!strcmp(t, "b")) out(" %d", (int)(signed char)v.b[i]);
        else if (!strcmp(t, "ub")) out(" %u", v.b[i]);
        else {
            unsigned char *w;
            memcpy(&w, v.b + sizeof(void *) * i, sizeof w);
            int slot = -1;
            for (int j = 0; j < 16; j++) if (w >= pslot[j].b && w < pslot[j].b + 4096) slot = j + 1;
            out(" %d", w ? slot : 0);
        }
    }
    out("\n");
}

/* glr <name> b|i|u <sig of integers or -> <args>: name(args) returning GLboolean (its low byte), GLint or
 * GLuint; "ret <value>" (glGenLists, glIsList, glRenderMode) */
static void op_glr(void) {
    NOCTX();
    if (ntok < 4) { bad = 1; return; }
    const char *sig = tok[3];
    long a[9]; float f[9]; double d[9];
    int k = lead_args(sig, 4, a, f, d);
    if (bad || ntok != 4 + k || strchr(sig, 'f') || strchr(sig, 'd')) { bad = 1; return; }
    FARPROC p = glf(tok[1]);
    if (!p) { out("glr %s none\n", tok[1]); return; }
    unsigned long r;
    if (k == 0) r = ((unsigned long (WINAPI *)(void))p)();
    else if (k == 1) r = ((unsigned long (WINAPI *)(long))p)(a[0]);
    else if (k == 2) r = ((unsigned long (WINAPI *)(long, long))p)(a[0], a[1]);
    else { bad = 1; return; }
    if (is(2, "b")) out("ret %u\n", (unsigned)(r & 0xff));
    else if (is(2, "i")) out("ret %ld\n", (long)(int)r);
    else out("ret %lu\n", r & 0xffffffffUL);
}

/* globuf u|f <n>: the first n elements of obuf */
static void op_globuf(void) {
    long n = num(2);
    if (bad || ntok != 3 || n < 1 || n > 1024 || !(is(1, "u") || is(1, "f"))) { bad = 1; return; }
    out("obuf");
    for (long i = 0; i < n; i++) {
        if (is(1, "u")) out(" %u", obuf.u[i]);
        else out(" %.9g", obuf.f[i]);
    }
    out("\n");
}

/* wglx <what> <args>: the other wgl functions on the context's DC (opengl32's own exports), each
 * "wglx <what> <return value> [<last error>]":
 * getpf; setpf <n>; descpf <n> (and the descriptor); choose <flags> <type> <color> <alpha> <accum>
 * <depth> <stencil>; swap; swaplayer <planes>; swapmultiple <n>; proc <name> (1 for a non-null
 * address); defproc <name>; layer <pf> <plane>; getlayerpal <plane> <start> <n>; setlayerpal <plane>
 * <start> <n>; realize <plane> <0|1>; layerctx <plane>; share; copy <mask>; fontbmp a|w <first>
 * <count> <list base>; fontoutline a|w <first> <count> <list base> <format> */
static void op_wglx(void) {
    NOCTX();
    if (ntok < 2) { bad = 1; return; }
    const char *w = tok[1];
    SetLastError(0);
#define WGL(name) glf(name)
    if (!strcmp(w, "getpf")) {
        if (ntok != 2) { bad = 1; return; }
        out("wglx getpf %d\n", ((int (WINAPI *)(HDC))WGL("wglGetPixelFormat"))(gdc));
    } else if (!strcmp(w, "setpf")) {
        int n = num(2);
        if (bad || ntok != 3) { bad = 1; return; }
        PIXELFORMATDESCRIPTOR p;
        DescribePixelFormat(gdc, n, sizeof p, &p);
        int r = ((BOOL (WINAPI *)(HDC, int, const PIXELFORMATDESCRIPTOR *))WGL("wglSetPixelFormat"))(gdc, n, &p);
        out("wglx setpf %d %lu\n", r, r ? 0 : GetLastError());
    } else if (!strcmp(w, "descpf")) {
        int n = num(2);
        if (bad || ntok != 3) { bad = 1; return; }
        PIXELFORMATDESCRIPTOR p;
        memset(&p, 0xcc, sizeof p);
        int r = ((int (WINAPI *)(HDC, int, UINT, PIXELFORMATDESCRIPTOR *))WGL("wglDescribePixelFormat"))(gdc, n, sizeof p, &p);
        out("wglx descpf %d\n", r);
        if (r) print_pfd(n, &p);
    } else if (!strcmp(w, "choose")) {
        PIXELFORMATDESCRIPTOR p;
        want_pfd(&p, 2);
        if (bad || ntok != 9) { bad = 1; return; }
        out("wglx choose %d\n", ((int (WINAPI *)(HDC, const PIXELFORMATDESCRIPTOR *))WGL("wglChoosePixelFormat"))(gdc, &p));
    } else if (!strcmp(w, "swap")) {
        if (ntok != 2) { bad = 1; return; }
        int r = ((BOOL (WINAPI *)(HDC))WGL("wglSwapBuffers"))(gdc);
        out("wglx swap %d %lu\n", r, r ? 0 : GetLastError());
    } else if (!strcmp(w, "swaplayer")) {
        unsigned planes = (unsigned)num(2);
        if (bad || ntok != 3) { bad = 1; return; }
        int r = ((BOOL (WINAPI *)(HDC, UINT))WGL("wglSwapLayerBuffers"))(gdc, planes);
        out("wglx swaplayer %d %lu\n", r, r ? 0 : GetLastError());
    } else if (!strcmp(w, "swapmultiple")) {
        long n = num(2);
        if (bad || ntok != 3 || n < 0 || n > 16) { bad = 1; return; }
        struct { HDC hdc; UINT flags; } s[16];
        for (int i = 0; i < 16; i++) { s[i].hdc = gdc; s[i].flags = 1; }
        DWORD r = ((DWORD (WINAPI *)(UINT, const void *))WGL("wglSwapMultipleBuffers"))((UINT)n, s);
        out("wglx swapmultiple %lu %lu\n", (unsigned long)r, GetLastError());
    } else if (!strcmp(w, "proc") || !strcmp(w, "defproc")) {
        if (ntok != 3) { bad = 1; return; }
        FARPROC f = WGL(!strcmp(w, "proc") ? "wglGetProcAddress" : "wglGetDefaultProcAddress");
        void *r = f ? ((void *(WINAPI *)(const char *))f)(tok[2]) : NULL;
        out("wglx %s %d\n", w, r != NULL);
    } else if (!strcmp(w, "layer")) {
        int pf = num(2), plane = num(3);
        if (bad || ntok != 4) { bad = 1; return; }
        LAYERPLANEDESCRIPTOR l;
        memset(&l, 0xcc, sizeof l);
        int r = ((BOOL (WINAPI *)(HDC, int, int, UINT, LAYERPLANEDESCRIPTOR *))WGL("wglDescribeLayerPlane"))(gdc, pf, plane, sizeof l, &l);
        out("wglx layer %d %lu\n", r, r ? 0 : GetLastError());
        if (r) out("layer %lx %d %d %d\n", l.dwFlags, l.iPixelType, l.cColorBits, l.iLayerPlane);
    } else if (!strcmp(w, "getlayerpal") || !strcmp(w, "setlayerpal")) {
        int plane = num(2), start = num(3), n = num(4);
        if (bad || ntok != 5 || n < 0 || n > 256) { bad = 1; return; }
        COLORREF c[256];
        for (int i = 0; i < 256; i++) c[i] = 0x00010203 * (DWORD)i;
        int r = !strcmp(w, "getlayerpal")
            ? ((int (WINAPI *)(HDC, int, int, int, COLORREF *))WGL("wglGetLayerPaletteEntries"))(gdc, plane, start, n, c)
            : ((int (WINAPI *)(HDC, int, int, int, const COLORREF *))WGL("wglSetLayerPaletteEntries"))(gdc, plane, start, n, c);
        out("wglx %s %d %lu\n", w, r, GetLastError());
    } else if (!strcmp(w, "realize")) {
        int plane = num(2), on = num(3);
        if (bad || ntok != 4) { bad = 1; return; }
        int r = ((BOOL (WINAPI *)(HDC, int, BOOL))WGL("wglRealizeLayerPalette"))(gdc, plane, on);
        out("wglx realize %d %lu\n", r, r ? 0 : GetLastError());
    } else if (!strcmp(w, "layerctx")) {
        int plane = num(2);
        if (bad || ntok != 3) { bad = 1; return; }
        HGLRC r = ((HGLRC (WINAPI *)(HDC, int))WGL("wglCreateLayerContext"))(gdc, plane);
        out("wglx layerctx %d %lu\n", r != NULL, r ? 0 : GetLastError());
        if (r) p_delete(r);
    } else if (!strcmp(w, "share") || !strcmp(w, "copy")) {
        unsigned mask = !strcmp(w, "copy") ? (unsigned)num(2) : 0;
        if (bad || ntok != (!strcmp(w, "copy") ? 3 : 2)) { bad = 1; return; }
        HGLRC other = p_create(gdc);
        if (!other) { out("wglx %s create failed %lu\n", w, GetLastError()); return; }
        int r = !strcmp(w, "share")
            ? ((BOOL (WINAPI *)(HGLRC, HGLRC))WGL("wglShareLists"))(grc, other)
            : ((BOOL (WINAPI *)(HGLRC, HGLRC, UINT))WGL("wglCopyContext"))(grc, other, mask);
        out("wglx %s %d %lu\n", w, r, r ? 0 : GetLastError());
        p_delete(other);
    } else if (!strcmp(w, "fontbmp") || !strcmp(w, "fontoutline")) {
        int outline = !strcmp(w, "fontoutline");
        long first = num(3), count = num(4), base = num(5), fmt = outline ? num(6) : 0;
        if (bad || ntok != (outline ? 7 : 6) || !(is(2, "a") || is(2, "w")) || count < 0 || count > 64) { bad = 1; return; }
        HFONT font = CreateFontA(-12, 0, 0, 0, 400, 0, 0, 0, ANSI_CHARSET, 0, 0, NONANTIALIASED_QUALITY, 0, "Arial");
        HGDIOBJ old = SelectObject(gdc, font);
        int r;
        if (!outline) {
            r = ((BOOL (WINAPI *)(HDC, DWORD, DWORD, DWORD))WGL(is(2, "a") ? "wglUseFontBitmapsA" : "wglUseFontBitmapsW"))(gdc, first, count, base);
            out("wglx %s %d %lu\n", w, r, r ? 0 : GetLastError());
        } else {
            static GLYPHMETRICSFLOAT gm[64];
            memset(gm, 0, sizeof gm);
            r = ((BOOL (WINAPI *)(HDC, DWORD, DWORD, DWORD, FLOAT, FLOAT, int, GLYPHMETRICSFLOAT *))WGL(is(2, "a") ? "wglUseFontOutlinesA" : "wglUseFontOutlinesW"))(gdc, first, count, base, 0.0f, 0.0f, (int)fmt, gm);
            out("wglx %s %d %lu\n", w, r, r ? 0 : GetLastError());
            if (r) for (long i = 0; i < count; i++) out("glyph %.9g %.9g %.9g %.9g %.9g %.9g\n", gm[i].gmfBlackBoxX, gm[i].gmfBlackBoxY, gm[i].gmfptGlyphOrigin.x, gm[i].gmfptGlyphOrigin.y, gm[i].gmfCellIncX, gm[i].gmfCellIncY);
        }
        SelectObject(gdc, old);
        DeleteObject(font);
    } else bad = 1;
#undef WGL
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
    { "gltex", op_gltex },
    { "gltexsub", op_gltexsub },
    { "glteximg", op_glteximg },
    { "gltexq", op_gltexq },
    { "gltexv", op_gltexv },
    { "glgentex", op_glgentex },
    { "gldeltex", op_gldeltex },
    { "glistex", op_glistex },
    { "glprio", op_glprio },
    { "glresident", op_glresident },
    { "gldib", op_gldib },
    { "glcur", op_glcur },
    { "glrelease", op_glrelease },
    { "glp", op_glp },
    { "glq", op_glq },
    { "glr", op_glr },
    { "globuf", op_globuf },
    { "wglx", op_wglx },
    { 0 },
};
const struct kz_api opengl_api = { "opengl", opengl_ops, opengl_reset };
