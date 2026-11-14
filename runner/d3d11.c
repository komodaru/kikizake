#include "core.h"
#include <d3d11.h>
/* Direct3D 11 state objects (d3d11.dll loaded only when a case asks for a device) */

ID3D11Device *dev11; /* (runner/d3dres.c uses it too) */
#define OBJS 64
static IUnknown *objs[OBJS];
static int nobjs;

static void d3d11_reset(void) {
    for (int i = 0; i < nobjs; i++)
        if (objs[i]) IUnknown_Release(objs[i]);
    nobjs = 0;
    if (dev11) { ID3D11Device_Release(dev11); dev11 = NULL; }
}

/* device11 [<feature level>]: D3D11CreateDevice, hardware (WARP if refused), feature level 11_0
 * or the one given asked */
static void op_device11(void) {
    d3d11_reset();
    static HRESULT (WINAPI *create)(void *, D3D_DRIVER_TYPE, HMODULE, UINT, const D3D_FEATURE_LEVEL *, UINT, UINT, ID3D11Device **, D3D_FEATURE_LEVEL *, void **);
    if (!create) {
        HMODULE m = LoadLibraryA("d3d11.dll");
        if (m) create = (void *)GetProcAddress(m, "D3D11CreateDevice");
    }
    if (!create) { out("device11 none\n"); return; }
    D3D_FEATURE_LEVEL fl = ntok > 1 ? (D3D_FEATURE_LEVEL)num(1) : D3D_FEATURE_LEVEL_11_0;
    if (bad || ntok > 2) { bad = 1; return; }
    HRESULT hr = create(NULL, D3D_DRIVER_TYPE_HARDWARE, NULL, 0, &fl, 1, D3D11_SDK_VERSION, &dev11, NULL, NULL);
    if (FAILED(hr)) hr = create(NULL, D3D_DRIVER_TYPE_WARP, NULL, 0, &fl, 1, D3D11_SDK_VERSION, &dev11, NULL, NULL);
    if (FAILED(hr)) { out("device11 failed %08lx\n", hr); dev11 = NULL; return; }
    out("device11 ok\n");
}

/* blend|depth|raster|sampler [check|nodesc] <the description's 32-bit words, missing ones 0>: Create...State
 * (ppState NULL for check, pDesc NULL for nodesc), then GetDesc. Prints "<kind> hr <hr>" on
 * failure (and for check / nodesc), else "<kind> = <GetDesc's words> same <n|->", n the
 * case's earlier state line (counted from 0) that gave the same object. */
static void op_state11(void) {
    static const char *kinds[] = { "blend", "depth", "raster", "sampler" };
    static const int words[] = { 66, 13, 10, 13 };
    int k = 0;
    while (!is(0, kinds[k])) k++;
    if (!dev11) { out("nodev\n"); return; }
    int check = is(1, "check"), nodesc = is(1, "nodesc"), at = 1 + check + nodesc;
    DWORD d[66] = { 0 }, got[66];
    if (!nodesc) {
        if (ntok - at > words[k]) { bad = 1; return; }
        for (int i = 0; i < ntok - at; i++) d[i] = (DWORD)num(at + i);
    } else if (ntok != 2) { bad = 1; return; }
    if (bad || nobjs >= OBJS) { bad = 1; return; }
    void *o = NULL, **po = check ? NULL : &o, *pd = nodesc ? NULL : d;
    HRESULT hr;
    memset(got, 0xcc, sizeof got);
    switch (k) {
    case 0:
        hr = ID3D11Device_CreateBlendState(dev11, pd, (ID3D11BlendState **)po);
        if (o) ID3D11BlendState_GetDesc((ID3D11BlendState *)o, (D3D11_BLEND_DESC *)got);
        break;
    case 1:
        hr = ID3D11Device_CreateDepthStencilState(dev11, pd, (ID3D11DepthStencilState **)po);
        if (o) ID3D11DepthStencilState_GetDesc((ID3D11DepthStencilState *)o, (D3D11_DEPTH_STENCIL_DESC *)got);
        break;
    case 2:
        hr = ID3D11Device_CreateRasterizerState(dev11, pd, (ID3D11RasterizerState **)po);
        if (o) ID3D11RasterizerState_GetDesc((ID3D11RasterizerState *)o, (D3D11_RASTERIZER_DESC *)got);
        break;
    default:
        hr = ID3D11Device_CreateSamplerState(dev11, pd, (ID3D11SamplerState **)po);
        if (o) ID3D11SamplerState_GetDesc((ID3D11SamplerState *)o, (D3D11_SAMPLER_DESC *)got);
        break;
    }
    objs[nobjs++] = o;
    if (!o) { out("%s hr %08lx\n", kinds[k], hr); return; }
    out("%s =", kinds[k]);
    for (int i = 0; i < words[k]; i++) {
        DWORD w = got[i];
        /* (UINT8 fields: the write mask of each target, the stencil masks; padding not printed) */
        if (k == 0 && i >= 2 && (i - 2) % 8 == 7) w &= 0xff;
        if (k == 1 && i == 4) w &= 0xffff;
        out(" %lx", w);
    }
    int same = -1;
    for (int j = 0; j < nobjs - 1; j++)
        if (objs[j] == o) { same = j; break; }
    if (same < 0) out(" same -\n");
    else out(" same %d\n", same);
}

static const struct kz_op d3d11_ops[] = {
    { "device11", op_device11 },
    { "blend", op_state11 },
    { "depth", op_state11 },
    { "raster", op_state11 },
    { "sampler", op_state11 },
    { 0 },
};
const struct kz_api d3d11_api = { "d3d11", d3d11_ops, d3d11_reset };
