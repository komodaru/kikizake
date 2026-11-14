#include "core.h"
#include <d3d9.h>
#include <cpuid.h>
/* D3DX 9 (d3dx9_43.dll, loaded when a line first asks for it; `x9 none` without it): vertex
 * formats and declarations, 16-bit floats, and surface loading from memory (SPEC.md "D3DX 9").
 * The function types are written here (d3dx9.h is not needed). */

void *kz_d3d9_device(void);
int kz_fmt_bits(DWORD f);
void kz_fill_rows(unsigned char *p, int pitch, int w, int h, DWORD fmt, long seed);
void kz_dump_rows(const unsigned char *p, int pitch, int w, int h, DWORD fmt);
int kz_words(int i, DWORD *w, int max);

static HMODULE x9;
static void *fn(const char *name) {
    static int tried;
    if (!tried) { tried = 1; x9 = LoadLibraryA("d3dx9_43.dll"); }
    return x9 ? (void *)GetProcAddress(x9, name) : NULL;
}
#define NEED(f) if (!(f)) { out("x9 none\n"); return; }

/* words in pairs -> elements, D3DDECL_END after them */
static void elements(DWORD *w, int n, D3DVERTEXELEMENT9 *e) {
    for (int i = 0; i < n / 2; i++) {
        e[i].Stream = w[2 * i] & 0xffff; e[i].Offset = w[2 * i] >> 16;
        e[i].Type = w[2 * i + 1] & 255; e[i].Method = w[2 * i + 1] >> 8 & 255;
        e[i].Usage = w[2 * i + 1] >> 16 & 255; e[i].UsageIndex = w[2 * i + 1] >> 24;
    }
    D3DVERTEXELEMENT9 end = D3DDECL_END();
    e[n / 2] = end;
}
static void print_elements(const char *what, const D3DVERTEXELEMENT9 *e) {
    int n = 0;
    while (n < MAXD3DDECLLENGTH && e[n].Stream != 0xff) n++;
    out("%s %d", what, n);
    for (int i = 0; i < n; i++)
        out(" %08lx %08lx", (DWORD)(e[i].Stream | e[i].Offset << 16), (DWORD)(e[i].Type | e[i].Method << 8 | e[i].Usage << 16 | e[i].UsageIndex << 24));
    out("\n");
}

/* x9fvfsize <fvf>: D3DXGetFVFVertexSize */
static void op_fvfsize(void) {
    UINT (WINAPI *f)(DWORD) = fn("D3DXGetFVFVertexSize");
    DWORD v = num(1);
    if (bad) return;
    NEED(f);
    out("fvfsize %u\n", f(v));
}

/* x9fvfdecl <fvf>: D3DXDeclaratorFromFVF */
static void op_fvfdecl(void) {
    HRESULT (WINAPI *f)(DWORD, D3DVERTEXELEMENT9 *) = fn("D3DXDeclaratorFromFVF");
    DWORD v = num(1);
    if (bad) return;
    NEED(f);
    D3DVERTEXELEMENT9 e[MAXD3DDECLLENGTH + 1];
    memset(e, 0xcd, sizeof e);
    HRESULT hr = f(v, e);
    if (hr) out("fvfdecl hr %08lx\n", hr);
    else print_elements("fvfdecl", e);
}

/* x9declfvf <words>: D3DXFVFFromDeclarator; x9declsize <words>: D3DXGetDeclVertexSize of
 * stream 0 and D3DXGetDeclLength */
static void op_declfvf(void) {
    HRESULT (WINAPI *f)(const D3DVERTEXELEMENT9 *, DWORD *) = fn("D3DXFVFFromDeclarator");
    DWORD w[MAXTOK];
    int n = kz_words(1, w, MAXTOK);
    if (bad || n & 1) { bad = 1; return; }
    NEED(f);
    D3DVERTEXELEMENT9 e[MAXTOK / 2 + 1];
    elements(w, n, e);
    DWORD v = 0xdeadbeef;
    HRESULT hr = f(e, &v);
    if (hr) out("declfvf hr %08lx\n", hr);
    else out("declfvf %08lx\n", v);
}
static void op_declsize(void) {
    UINT (WINAPI *f)(const D3DVERTEXELEMENT9 *, DWORD) = fn("D3DXGetDeclVertexSize");
    UINT (WINAPI *g)(const D3DVERTEXELEMENT9 *) = fn("D3DXGetDeclLength");
    DWORD w[MAXTOK];
    int n = kz_words(1, w, MAXTOK);
    if (bad || n & 1) { bad = 1; return; }
    NEED(f && g);
    D3DVERTEXELEMENT9 e[MAXTOK / 2 + 1];
    elements(w, n, e);
    out("declsize %u len %u\n", f(e, 0), g(e));
}

/* x9cpu: the processor D3DX picks its code by (cpuid: vendor, leaf 1 ecx and edx, leaf 7 ebx) */
static void op_cpu(void) {
    unsigned a, b, c, d;
    char v[13] = { 0 };
    __cpuid(0, a, b, c, d);
    memcpy(v, &b, 4); memcpy(v + 4, &d, 4); memcpy(v + 8, &c, 4);
    unsigned max = a, e1c = 0, e1d = 0, e7b = 0;
    if (max >= 1) { __cpuid(1, a, b, c, d); e1c = c; e1d = d; }
    if (max >= 7) { __cpuid_count(7, 0, a, b, c, d); e7b = b; }
    out("cpu %s %08x %08x %08x\n", v, e1c, e1d, e7b);
}

/* x9f16 <float bits>...: D3DXFloat32To16Array; x9f32 <half bits>...: D3DXFloat16To32Array */
static void op_f16(void) {
    WORD *(WINAPI *f)(WORD *, const float *, UINT) = fn("D3DXFloat32To16Array");
    DWORD w[MAXTOK];
    int n = kz_words(1, w, MAXTOK);
    if (bad) return;
    NEED(f);
    WORD h[MAXTOK];
    f(h, (const float *)w, n);
    out("f16");
    for (int i = 0; i < n; i++) out(" %04x", h[i]);
    out("\n");
}
static void op_f32(void) {
    float *(WINAPI *f)(float *, const WORD *, UINT) = fn("D3DXFloat16To32Array");
    DWORD w[MAXTOK];
    int n = kz_words(1, w, MAXTOK);
    if (bad) return;
    NEED(f);
    WORD h[MAXTOK];
    DWORD r[MAXTOK];
    for (int i = 0; i < n; i++) h[i] = (WORD)w[i];
    f((float *)r, h, n);
    out("f32");
    for (int i = 0; i < n; i++) out(" %08lx", r[i]);
    out("\n");
}

/* x9load <dst fmt> <src fmt> <w> <h> <seed|ramp> <filter>: D3DXLoadSurfaceFromMemory of a
 * w x h picture of the source format (rows packed, bytes as d9texfill) into a system memory
 * surface of the destination format (`device` first), then the surface's picture */
static void op_load(void) {
    HRESULT (WINAPI *f)(IDirect3DSurface9 *, const PALETTEENTRY *, const RECT *, const void *, D3DFORMAT, UINT, const PALETTEENTRY *, const RECT *, DWORD, D3DCOLOR) = fn("D3DXLoadSurfaceFromMemory");
    long df = num(1), sf = num(2), w = num(3), h = num(4), seed = is(5, "ramp") ? -1 : num(5), filter = num(6);
    if (bad || w <= 0 || h <= 0 || w > 256 || h > 64 || !kz_fmt_bits(sf)) { bad = 1; return; }
    IDirect3DDevice9 *dev = kz_d3d9_device();
    if (!dev) { out("nodev\n"); return; }
    NEED(f);
    IDirect3DSurface9 *s;
    HRESULT hr = IDirect3DDevice9_CreateOffscreenPlainSurface(dev, w, h, df, D3DPOOL_SYSTEMMEM, &s, NULL);
    if (FAILED(hr)) { out("load surface hr %08lx\n", hr); return; }
    int b = kz_fmt_bits(sf), pitch = b < 0 ? (w + 3) / 4 * -b : (w * b + 7) / 8;
    unsigned char *src = calloc(pitch, h + 4);
    kz_fill_rows(src, pitch, w, h, sf, seed);
    RECT r = { 0, 0, w, h };
    hr = f(s, NULL, NULL, src, sf, pitch, NULL, &r, filter, 0);
    free(src);
    if (hr) out("load hr %08lx\n", hr);
    else {
        D3DLOCKED_RECT lr;
        if (SUCCEEDED(IDirect3DSurface9_LockRect(s, &lr, NULL, D3DLOCK_READONLY))) {
            kz_dump_rows(lr.pBits, lr.Pitch, w, h, df);
            IDirect3DSurface9_UnlockRect(s);
        }
    }
    IDirect3DSurface9_Release(s);
}

static const struct kz_op d3dx9_ops[] = {
    { "x9fvfsize", op_fvfsize },
    { "x9fvfdecl", op_fvfdecl },
    { "x9declfvf", op_declfvf },
    { "x9declsize", op_declsize },
    { "x9cpu", op_cpu },
    { "x9f16", op_f16 },
    { "x9f32", op_f32 },
    { "x9load", op_load },
    { 0 },
};
const struct kz_api d3dx9_api = { "d3dx9", d3dx9_ops, NULL };
