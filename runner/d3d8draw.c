#include "core.h"
#include <d3d8.h>
/* Direct3D 8 drawing: runner/d3d9draw.c's lines on a Direct3D 8 device (`device8`). d3d8.h and
 * d3d9.h clash, so this file is its own; d3d9draw.c's ops call these while a Direct3D 8 device
 * is current. */

void *kz_d3d8(void);
void *kz_d3d8_device(void);
int kz_fmt_bits(DWORD f);
void kz_fill_rows(unsigned char *p, int pitch, int w, int h, DWORD fmt, long seed);
void kz_dump_rows(const unsigned char *p, int pitch, int w, int h, DWORD fmt);
int kz_words(int i, DWORD *w, int max);
int kz_prims(int type, int n);

#define TEXS 8
static IDirect3DSurface8 *rt, *ds;
static IDirect3DTexture8 *texs[TEXS];
static int texw[TEXS], texh[TEXS];
static DWORD texfmt[TEXS];
#define DEV ((IDirect3DDevice8 *)kz_d3d8_device())

static void d3d8draw_reset(void) {
    for (int i = 0; i < TEXS; i++)
        if (texs[i]) { IDirect3DTexture8_Release(texs[i]); texs[i] = NULL; }
    if (rt) { IDirect3DSurface8_Release(rt); rt = NULL; }
    if (ds) { IDirect3DSurface8_Release(ds); ds = NULL; }
}

void d8_target(void) {
    long w = num(1), h = num(2), f = num(3);
    if (bad) return;
    if (rt) { IDirect3DSurface8_Release(rt); rt = NULL; }
    if (ds) { IDirect3DSurface8_Release(ds); ds = NULL; }
    HRESULT hr = IDirect3DDevice8_CreateRenderTarget(DEV, w, h, f, D3DMULTISAMPLE_NONE, TRUE, &rt);
    if (FAILED(hr)) { rt = NULL; out("target failed %08lx\n", hr); return; }
    hr = IDirect3DDevice8_CreateDepthStencilSurface(DEV, w, h, D3DFMT_D24S8, D3DMULTISAMPLE_NONE, &ds);
    if (FAILED(hr)) { ds = NULL; out("target depth failed %08lx\n", hr); }
    hr = IDirect3DDevice8_SetRenderTarget(DEV, rt, ds);
    if (hr) out("target set hr %08lx\n", hr);
}

void d8_clear(void) {
    D3DRECT r[16];
    int n = 0;
    for (int i = 5; i + 3 < ntok && n < 16; i += 4, n++) {
        r[n].x1 = num(i); r[n].y1 = num(i + 1); r[n].x2 = num(i + 2); r[n].y2 = num(i + 3);
    }
    DWORD flags = num(1), c = num(2), zb = num(3), st = num(4);
    if (bad) return;
    float z;
    memcpy(&z, &zb, 4);
    HRESULT hr = IDirect3DDevice8_Clear(DEV, n, n ? r : NULL, flags, c, z, st);
    if (hr) out("clear hr %08lx\n", hr);
}

void d8_rt(void) {
    if (!rt) { out("rt none\n"); return; }
    D3DSURFACE_DESC d;
    IDirect3DSurface8_GetDesc(rt, &d);
    D3DLOCKED_RECT lr;
    HRESULT hr = IDirect3DSurface8_LockRect(rt, &lr, NULL, D3DLOCK_READONLY);
    if (FAILED(hr)) { out("rt lock hr %08lx\n", hr); return; }
    kz_dump_rows(lr.pBits, lr.Pitch, d.Width, d.Height, d.Format);
    IDirect3DSurface8_UnlockRect(rt);
}

/* (Direct3D 8: an FVF is the vertex shader handle) */
void d8_fvf(void) {
    DWORD f = num(1);
    if (bad) return;
    HRESULT hr = IDirect3DDevice8_SetVertexShader(DEV, f);
    if (hr) out("fvf hr %08lx\n", hr);
}

void d8_getfvf(void) {
    DWORD f = 0xdeadbeef;
    HRESULT hr = IDirect3DDevice8_GetVertexShader(DEV, &f);
    if (hr) out("getfvf hr %08lx\n", hr);
    else out("getfvf %08lx\n", f);
}

void d8_draw(void) {
    long type = num(1), nv = num(2);
    DWORD w[MAXTOK];
    int n = kz_words(3, w, MAXTOK);
    if (bad || nv <= 0 || n % nv) { bad = 1; return; }
    IDirect3DDevice8_BeginScene(DEV);
    HRESULT hr = IDirect3DDevice8_DrawPrimitiveUP(DEV, type, kz_prims(type, nv), w, n / nv * 4);
    IDirect3DDevice8_EndScene(DEV);
    if (hr) out("draw hr %08lx\n", hr);
}

void d8_tex(void) {
    long i = num(1), w = num(2), h = num(3), f = num(4);
    if (bad || i < 0 || i >= TEXS) { bad = 1; return; }
    if (texs[i]) { IDirect3DTexture8_Release(texs[i]); texs[i] = NULL; }
    HRESULT hr = IDirect3DDevice8_CreateTexture(DEV, w, h, 1, 0, f, D3DPOOL_MANAGED, &texs[i]);
    if (FAILED(hr)) { texs[i] = NULL; out("tex failed %08lx\n", hr); return; }
    texw[i] = w; texh[i] = h; texfmt[i] = f;
}

void d8_texfill(void) {
    long i = num(1), seed = is(2, "ramp") ? -1 : num(2);
    if (bad || i < 0 || i >= TEXS) { bad = 1; return; }
    if (!texs[i]) { out("tex %ld none\n", i); return; }
    D3DLOCKED_RECT lr;
    HRESULT hr = IDirect3DTexture8_LockRect(texs[i], 0, &lr, NULL, 0);
    if (FAILED(hr)) { out("texfill hr %08lx\n", hr); return; }
    kz_fill_rows(lr.pBits, lr.Pitch, texw[i], texh[i], texfmt[i], seed);
    IDirect3DTexture8_UnlockRect(texs[i], 0);
}

void d8_settex(void) {
    long s = num(1), i = is(2, "none") ? -1 : num(2);
    if (bad || i >= TEXS) { bad = 1; return; }
    HRESULT hr = IDirect3DDevice8_SetTexture(DEV, s, i < 0 ? NULL : (IDirect3DBaseTexture8 *)texs[i]);
    if (hr) out("settex hr %08lx\n", hr);
}

void d8_viewport(void) {
    D3DVIEWPORT8 v = { num(1), num(2), num(3), num(4) };
    DWORD a = num(5), b = num(6);
    if (bad) return;
    memcpy(&v.MinZ, &a, 4);
    memcpy(&v.MaxZ, &b, 4);
    HRESULT hr = IDirect3DDevice8_SetViewport(DEV, &v);
    if (hr) out("viewport hr %08lx\n", hr);
}

void d8_xform(void) {
    long t = num(1);
    DWORD w[16];
    if (kz_words(2, w, 16) != 16 || bad) { bad = 1; return; }
    HRESULT hr = IDirect3DDevice8_SetTransform(DEV, t, (D3DMATRIX *)w);
    if (hr) out("xform hr %08lx\n", hr);
}

void d8_check(void) {
    long f = num(1), u = num(2), t = num(3);
    if (bad) return;
    HRESULT hr = IDirect3D8_CheckDeviceFormat((IDirect3D8 *)kz_d3d8(), D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, D3DFMT_X8R8G8B8, u, t, f);
    out("check %ld %lx %ld hr %08lx\n", f, u, t, hr);
}

void d8_palette(void) {
    long seed = num(1);
    if (bad) return;
    PALETTEENTRY e[256];
    kz_fill_rows((unsigned char *)e, 1024, 1024, 1, D3DFMT_L8, seed);
    HRESULT hr = IDirect3DDevice8_SetPaletteEntries(DEV, 0, e);
    if (hr) out("palette hr %08lx\n", hr);
    hr = IDirect3DDevice8_SetCurrentTexturePalette(DEV, 0);
    if (hr) out("palette current hr %08lx\n", hr);
}

static void never(void) { bad = 1; }
/* (never looked up: d3d9draw comes first in apis.h and owns these names; listed so that
 * runner/golden.py counts this file among the sources of the suites that use them) */
static const struct kz_op d3d8draw_ops[] = {
    { "d9target", never }, { "d9clear", never }, { "d9rt", never }, { "d9fvf", never },
    { "d9getfvf", never }, { "d9draw", never }, { "d9tex", never }, { "d9texfill", never },
    { "d9settex", never }, { "d9viewport", never }, { "d9xform", never }, { "d9check", never }, { "d9palette", never },
    { 0 },
};
const struct kz_api d3d8draw_api = { "d3d8draw", d3d8draw_ops, d3d8draw_reset };
