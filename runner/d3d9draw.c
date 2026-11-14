#include "core.h"
#include <d3d9.h>
/* Direct3D 9 and 8 drawing (SPEC.md "Direct3D 9 and 8 drawing"): render targets, clears,
 * textures, vertex formats and declarations, DrawPrimitiveUP, and the pictures read back. The
 * lines act on the device the case made with `device` or `device8` (runner/d3d9.c); on a
 * Direct3D 8 device each op is runner/d3d8draw.c's. */

void *kz_d3d9(void);
void *kz_d3d9_device(void);
void *kz_d3d8_device(void);
/* runner/d3d8draw.c */
void d8_target(void), d8_clear(void), d8_rt(void), d8_fvf(void), d8_getfvf(void), d8_draw(void);
void d8_tex(void), d8_texfill(void), d8_settex(void), d8_viewport(void), d8_xform(void), d8_check(void);

#define TEXS 8
static IDirect3DSurface9 *rt, *ds;
static IDirect3DTexture9 *texs[TEXS];
static IDirect3DVertexDeclaration9 *decl;
static int texw[TEXS], texh[TEXS];
static DWORD texfmt[TEXS];

static void d3d9draw_reset(void) {
    for (int i = 0; i < TEXS; i++)
        if (texs[i]) { IDirect3DTexture9_Release(texs[i]); texs[i] = NULL; }
    if (rt) { IDirect3DSurface9_Release(rt); rt = NULL; }
    if (ds) { IDirect3DSurface9_Release(ds); ds = NULL; }
    if (decl) { IDirect3DVertexDeclaration9_Release(decl); decl = NULL; }
}

/* bits per pixel of a D3DFORMAT, or for block-compressed formats -(bytes per 4x4 block); 0 unknown */
int kz_fmt_bits(DWORD f) {
    switch (f) {
    case 20: return 24;
    case 21: case 22: case 31: case 32: case 33: case 34: case 35: case 62: case 63: case 64: case 67: case 114: case 112: return 32;
    case 23: case 24: case 25: case 26: case 29: case 30: case 40: case 51: case 60: case 61: case 81: case 111: case 117: return 16;
    case 27: case 28: case 41: case 50: case 52: return 8;
    case 36: case 110: case 113: case 115: return 64;
    case 116: return 128;
    case MAKEFOURCC('D', 'X', 'T', '1'): return -8;
    case MAKEFOURCC('D', 'X', 'T', '2'): case MAKEFOURCC('D', 'X', 'T', '3'): case MAKEFOURCC('D', 'X', 'T', '4'): case MAKEFOURCC('D', 'X', 'T', '5'): return -16;
    case MAKEFOURCC('U', 'Y', 'V', 'Y'): case MAKEFOURCC('Y', 'U', 'Y', '2'): return 16;
    }
    return 0;
}

/* the bytes of each row of a locked level (row by row: block rows for compressed formats), from
 * xorshift32 (seed) or byte i = i mod 256 (seed -1) */
void kz_fill_rows(unsigned char *p, int pitch, int w, int h, DWORD fmt, long seed) {
    int b = kz_fmt_bits(fmt);
    int rows = b < 0 ? (h + 3) / 4 : h, n = b < 0 ? (w + 3) / 4 * -b : (w * b + 7) / 8;
    unsigned x = seed ? (unsigned)seed : 0x9e3779b9u, k = 0;
    for (int y = 0; y < rows; y++)
        for (int i = 0; i < n; i++) {
            if (seed == -1) { p[y * pitch + i] = (unsigned char)k++; continue; }
            x ^= x << 13; x ^= x >> 17; x ^= x << 5;
            p[y * pitch + i] = (unsigned char)x;
        }
}

/* "px rt <w> <h> <bpp>" and the rows of a locked picture as hex */
void kz_dump_rows(const unsigned char *p, int pitch, int w, int h, DWORD fmt) {
    int b = kz_fmt_bits(fmt);
    out("px rt %d %d %d\n", w, h, b);
    for (int y = 0; y < h; y++) out_hex(p + y * pitch, w * b / 8);
}

/* words of the line from token i on (vertex data, matrices, declarations) */
int kz_words(int i, DWORD *w, int max) {
    int n = 0;
    for (; i < ntok && n < max; i++) w[n++] = (DWORD)num(i);
    return n;
}

/* the primitive count of n vertices */
int kz_prims(int type, int n) {
    switch (type) {
    case 1: return n;
    case 2: return n / 2;
    case 3: return n - 1;
    case 4: return n / 3;
    case 5: case 6: return n - 2;
    }
    return 0;
}

#define DEV ((IDirect3DDevice9 *)kz_d3d9_device())
#define ON8(f) if (kz_d3d8_device()) { f(); return; } if (!DEV) { out("nodev\n"); return; }

/* d9target <w> <h> <fmt>: a lockable render target and a D24S8 depth stencil of its size, set */
static void op_target(void) {
    ON8(d8_target);
    long w = num(1), h = num(2), f = num(3);
    if (bad) return;
    if (rt) { IDirect3DSurface9_Release(rt); rt = NULL; }
    if (ds) { IDirect3DSurface9_Release(ds); ds = NULL; }
    HRESULT hr = IDirect3DDevice9_CreateRenderTarget(DEV, w, h, f, D3DMULTISAMPLE_NONE, 0, TRUE, &rt, NULL);
    if (FAILED(hr)) { rt = NULL; out("target failed %08lx\n", hr); return; }
    hr = IDirect3DDevice9_CreateDepthStencilSurface(DEV, w, h, D3DFMT_D24S8, D3DMULTISAMPLE_NONE, 0, FALSE, &ds, NULL);
    if (FAILED(hr)) { ds = NULL; out("target depth failed %08lx\n", hr); }
    IDirect3DDevice9_SetRenderTarget(DEV, 0, rt);
    IDirect3DDevice9_SetDepthStencilSurface(DEV, ds);
}

/* d9clear <flags> <color> <z bits> <stencil> [<x0> <y0> <x1> <y1>]...: Clear (no rects: count 0, NULL) */
static void op_clear(void) {
    ON8(d8_clear);
    D3DRECT r[16];
    int n = 0;
    for (int i = 5; i + 3 < ntok && n < 16; i += 4, n++) {
        r[n].x1 = num(i); r[n].y1 = num(i + 1); r[n].x2 = num(i + 2); r[n].y2 = num(i + 3);
    }
    DWORD flags = num(1), c = num(2), zb = num(3), st = num(4);
    if (bad) return;
    float z;
    memcpy(&z, &zb, 4);
    HRESULT hr = IDirect3DDevice9_Clear(DEV, n, n ? r : NULL, flags, c, z, st);
    if (hr) out("clear hr %08lx\n", hr);
}

/* d9rt: the render target's picture */
static void op_rt(void) {
    ON8(d8_rt);
    if (!rt) { out("rt none\n"); return; }
    D3DSURFACE_DESC d;
    IDirect3DSurface9_GetDesc(rt, &d);
    D3DLOCKED_RECT lr;
    HRESULT hr = IDirect3DSurface9_LockRect(rt, &lr, NULL, D3DLOCK_READONLY);
    if (FAILED(hr)) { out("rt lock hr %08lx\n", hr); return; }
    kz_dump_rows(lr.pBits, lr.Pitch, d.Width, d.Height, d.Format);
    IDirect3DSurface9_UnlockRect(rt);
}

/* d9fvf <fvf>: SetFVF */
static void op_fvf(void) {
    ON8(d8_fvf);
    DWORD f = num(1);
    if (bad) return;
    HRESULT hr = IDirect3DDevice9_SetFVF(DEV, f);
    if (hr) out("fvf hr %08lx\n", hr);
}

/* d9getfvf: GetFVF */
static void op_getfvf(void) {
    ON8(d8_getfvf);
    DWORD f = 0xdeadbeef;
    HRESULT hr = IDirect3DDevice9_GetFVF(DEV, &f);
    if (hr) out("getfvf hr %08lx\n", hr);
    else out("getfvf %08lx\n", f);
}

/* d9decl <words>...: CreateVertexDeclaration of the elements given as pairs of words (Stream |
 * Offset << 16, Type | Method << 8 | Usage << 16 | UsageIndex << 24), D3DDECL_END added; set */
static void op_decl(void) {
    if (kz_d3d8_device()) { bad = 1; return; }
    if (!DEV) { out("nodev\n"); return; }
    DWORD w[MAXTOK];
    int n = kz_words(1, w, MAXTOK);
    if (bad || n & 1) { bad = 1; return; }
    D3DVERTEXELEMENT9 e[MAXTOK / 2 + 1];
    for (int i = 0; i < n / 2; i++) {
        e[i].Stream = w[2 * i] & 0xffff; e[i].Offset = w[2 * i] >> 16;
        e[i].Type = w[2 * i + 1] & 255; e[i].Method = w[2 * i + 1] >> 8 & 255;
        e[i].Usage = w[2 * i + 1] >> 16 & 255; e[i].UsageIndex = w[2 * i + 1] >> 24;
    }
    D3DVERTEXELEMENT9 end = D3DDECL_END();
    e[n / 2] = end;
    if (decl) { IDirect3DVertexDeclaration9_Release(decl); decl = NULL; }
    HRESULT hr = IDirect3DDevice9_CreateVertexDeclaration(DEV, e, &decl);
    if (FAILED(hr)) { decl = NULL; out("decl hr %08lx\n", hr); return; }
    hr = IDirect3DDevice9_SetVertexDeclaration(DEV, decl);
    if (hr) out("decl set hr %08lx\n", hr);
}

/* d9getdecl: GetVertexDeclaration, its elements (before the end) as "decl <n>" and pairs of words */
static void op_getdecl(void) {
    if (kz_d3d8_device()) { bad = 1; return; }
    if (!DEV) { out("nodev\n"); return; }
    IDirect3DVertexDeclaration9 *d = NULL;
    HRESULT hr = IDirect3DDevice9_GetVertexDeclaration(DEV, &d);
    if (hr) { out("getdecl hr %08lx\n", hr); return; }
    if (!d) { out("getdecl null\n"); return; }
    D3DVERTEXELEMENT9 e[MAXD3DDECLLENGTH + 1];
    UINT n = 0;
    hr = IDirect3DVertexDeclaration9_GetDeclaration(d, NULL, &n);
    if (!hr && n <= MAXD3DDECLLENGTH + 1) hr = IDirect3DVertexDeclaration9_GetDeclaration(d, e, &n);
    if (hr || n > MAXD3DDECLLENGTH + 1) out("getdecl elements hr %08lx %u\n", hr, n);
    else {
        out("getdecl %u", n - 1);
        for (UINT i = 0; i + 1 < n; i++)
            out(" %08lx %08lx", (DWORD)(e[i].Stream | e[i].Offset << 16), (DWORD)(e[i].Type | e[i].Method << 8 | e[i].Usage << 16 | e[i].UsageIndex << 24));
        out("\n");
    }
    IDirect3DVertexDeclaration9_Release(d);
}

/* d9draw <type> <n> <words>...: DrawPrimitiveUP of n vertices, stride = the words / n, between
 * BeginScene and EndScene */
static void op_draw(void) {
    ON8(d8_draw);
    long type = num(1), nv = num(2);
    DWORD w[MAXTOK];
    int n = kz_words(3, w, MAXTOK);
    if (bad || nv <= 0 || n % nv) { bad = 1; return; }
    IDirect3DDevice9_BeginScene(DEV);
    HRESULT hr = IDirect3DDevice9_DrawPrimitiveUP(DEV, type, kz_prims(type, nv), w, n / nv * 4);
    IDirect3DDevice9_EndScene(DEV);
    if (hr) out("draw hr %08lx\n", hr);
}

/* d9tex <i> <w> <h> <fmt>: CreateTexture, one level, D3DPOOL_MANAGED */
static void op_tex(void) {
    ON8(d8_tex);
    long i = num(1), w = num(2), h = num(3), f = num(4);
    if (bad || i < 0 || i >= TEXS) { bad = 1; return; }
    if (texs[i]) { IDirect3DTexture9_Release(texs[i]); texs[i] = NULL; }
    HRESULT hr = IDirect3DDevice9_CreateTexture(DEV, w, h, 1, 0, f, D3DPOOL_MANAGED, &texs[i], NULL);
    if (FAILED(hr)) { texs[i] = NULL; out("tex failed %08lx\n", hr); return; }
    texw[i] = w; texh[i] = h; texfmt[i] = f;
}

/* d9texfill <i> <seed|ramp>: level 0's bytes (kz_fill_rows) */
static void op_texfill(void) {
    ON8(d8_texfill);
    long i = num(1), seed = is(2, "ramp") ? -1 : num(2);
    if (bad || i < 0 || i >= TEXS) { bad = 1; return; }
    if (!texs[i]) { out("tex %ld none\n", i); return; }
    D3DLOCKED_RECT lr;
    HRESULT hr = IDirect3DTexture9_LockRect(texs[i], 0, &lr, NULL, 0);
    if (FAILED(hr)) { out("texfill hr %08lx\n", hr); return; }
    kz_fill_rows(lr.pBits, lr.Pitch, texw[i], texh[i], texfmt[i], seed);
    IDirect3DTexture9_UnlockRect(texs[i], 0);
}

/* d9settex <stage> <i|none>: SetTexture */
static void op_settex(void) {
    ON8(d8_settex);
    long s = num(1), i = is(2, "none") ? -1 : num(2);
    if (bad || i >= TEXS) { bad = 1; return; }
    HRESULT hr = IDirect3DDevice9_SetTexture(DEV, s, i < 0 ? NULL : (IDirect3DBaseTexture9 *)texs[i]);
    if (hr) out("settex hr %08lx\n", hr);
}

/* d9viewport <x> <y> <w> <h> <minz bits> <maxz bits> */
static void op_viewport(void) {
    ON8(d8_viewport);
    D3DVIEWPORT9 v = { num(1), num(2), num(3), num(4) };
    DWORD a = num(5), b = num(6);
    if (bad) return;
    memcpy(&v.MinZ, &a, 4);
    memcpy(&v.MaxZ, &b, 4);
    HRESULT hr = IDirect3DDevice9_SetViewport(DEV, &v);
    if (hr) out("viewport hr %08lx\n", hr);
}

/* d9xform <D3DTRANSFORMSTATETYPE> <16 words>: SetTransform */
static void op_xform(void) {
    ON8(d8_xform);
    long t = num(1);
    DWORD w[16];
    if (kz_words(2, w, 16) != 16 || bad) { bad = 1; return; }
    HRESULT hr = IDirect3DDevice9_SetTransform(DEV, t, (D3DMATRIX *)w);
    if (hr) out("xform hr %08lx\n", hr);
}

/* d9scissor <x0> <y0> <x1> <y1>: SetScissorRect (Direct3D 9 only) */
static void op_scissor(void) {
    if (kz_d3d8_device()) { bad = 1; return; }
    if (!DEV) { out("nodev\n"); return; }
    RECT r = { num(1), num(2), num(3), num(4) };
    if (bad) return;
    HRESULT hr = IDirect3DDevice9_SetScissorRect(DEV, &r);
    if (hr) out("scissor hr %08lx\n", hr);
}

/* d9check <fmt> <usage> <D3DRESOURCETYPE>: CheckDeviceFormat (HAL, adapter format X8R8G8B8) */
static void op_check(void) {
    ON8(d8_check);
    long f = num(1), u = num(2), t = num(3);
    if (bad) return;
    HRESULT hr = IDirect3D9_CheckDeviceFormat((IDirect3D9 *)kz_d3d9(), D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, D3DFMT_X8R8G8B8, u, t, f);
    out("check %ld %lx %ld hr %08lx\n", f, u, t, hr);
}

/* d9palette <seed>: palette 0's 256 entries from kz_fill_rows's bytes (r, g, b, flags as alpha),
 * made the current texture palette */
void d8_palette(void);
static void op_palette(void) {
    ON8(d8_palette);
    long seed = num(1);
    if (bad) return;
    PALETTEENTRY e[256];
    kz_fill_rows((unsigned char *)e, 1024, 1024, 1, D3DFMT_L8, seed);
    HRESULT hr = IDirect3DDevice9_SetPaletteEntries(DEV, 0, e);
    if (hr) out("palette hr %08lx\n", hr);
    hr = IDirect3DDevice9_SetCurrentTexturePalette(DEV, 0);
    if (hr) out("palette current hr %08lx\n", hr);
}

static const struct kz_op d3d9draw_ops[] = {
    { "d9target", op_target },
    { "d9clear", op_clear },
    { "d9rt", op_rt },
    { "d9fvf", op_fvf },
    { "d9getfvf", op_getfvf },
    { "d9decl", op_decl },
    { "d9getdecl", op_getdecl },
    { "d9draw", op_draw },
    { "d9tex", op_tex },
    { "d9texfill", op_texfill },
    { "d9settex", op_settex },
    { "d9viewport", op_viewport },
    { "d9xform", op_xform },
    { "d9scissor", op_scissor },
    { "d9check", op_check },
    { "d9palette", op_palette },
    { 0 },
};
const struct kz_api d3d9draw_api = { "d3d9draw", d3d9draw_ops, d3d9draw_reset };
