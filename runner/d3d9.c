#include "core.h"
#include <d3d9.h>
/* Direct3D 9 (d3d9.dll loaded only by a case that asks for a device: the GDI suite runs where
 * there is none) */

static IDirect3D9 *d3d;
static IDirect3DDevice9 *dev;
static HWND hwnd;
#define BLOCKS 8
static IDirect3DStateBlock9 *blocks[BLOCKS];

static void d3d9_reset(void) {
    for (int i = 0; i < BLOCKS; i++)
        if (blocks[i]) { IDirect3DStateBlock9_Release(blocks[i]); blocks[i] = NULL; }
    if (dev) { IDirect3DDevice9_Release(dev); dev = NULL; }
    if (d3d) { IDirect3D9_Release(d3d); d3d = NULL; }
}

/* device [hwvp|swvp|mixed]: a windowed HAL device, 64x64 X8R8G8B8 back buffer, D24S8 */
static void d3d8_reset(void);
static void op_device(void) {
    d3d9_reset();
    d3d8_reset();
    static IDirect3D9 *(WINAPI *create)(UINT);
    if (!create) {
        HMODULE m = LoadLibraryA("d3d9.dll");
        if (m) create = (void *)GetProcAddress(m, "Direct3DCreate9");
    }
    if (!create || !(d3d = create(D3D_SDK_VERSION))) { out("device none\n"); return; }
    if (!hwnd) {
        hwnd = kz_window();
    }
    DWORD flags = D3DCREATE_HARDWARE_VERTEXPROCESSING;
    if (is(1, "swvp")) flags = D3DCREATE_SOFTWARE_VERTEXPROCESSING;
    else if (is(1, "mixed")) flags = D3DCREATE_MIXED_VERTEXPROCESSING;
    D3DPRESENT_PARAMETERS pp = { 0 };
    pp.BackBufferWidth = 64;
    pp.BackBufferHeight = 64;
    pp.BackBufferFormat = D3DFMT_X8R8G8B8;
    pp.BackBufferCount = 1;
    pp.SwapEffect = D3DSWAPEFFECT_DISCARD;
    pp.hDeviceWindow = hwnd;
    pp.Windowed = TRUE;
    pp.EnableAutoDepthStencil = TRUE;
    pp.AutoDepthStencilFormat = D3DFMT_D24S8;
    HRESULT hr = IDirect3D9_CreateDevice(d3d, D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, hwnd, flags, &pp, &dev);
    if (FAILED(hr)) {
        /* (a software vertex processing device where hardware is refused: the state is the same) */
        hr = IDirect3D9_CreateDevice(d3d, D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, hwnd, D3DCREATE_SOFTWARE_VERTEXPROCESSING, &pp, &dev);
    }
    if (FAILED(hr)) { out("device failed %08lx\n", hr); dev = NULL; return; }
    out("device ok\n");
}

/* rs|tss|ss set <args> <value> | get <args>: "<what> <args> = <hex>" or "... hr <hex>" */
static void op_state8(void);
static void op_sb8(void);
static void *dev8;
static void op_state(void) {
    if (dev8) { op_state8(); return; }
    if (!dev) { out("nodev\n"); return; }
    int set = is(1, "set");
    if (!set && !is(1, "get")) { bad = 1; return; }
    int two = !is(0, "rs");
    long a = num(2), b = two ? num(3) : 0;
    DWORD v = set ? (DWORD)num(two ? 4 : 3) : 0xdeadbeef;
    if (bad) return;
    HRESULT hr;
    if (is(0, "rs")) hr = set ? IDirect3DDevice9_SetRenderState(dev, a, v) : IDirect3DDevice9_GetRenderState(dev, a, &v);
    else if (is(0, "tss")) hr = set ? IDirect3DDevice9_SetTextureStageState(dev, a, b, v) : IDirect3DDevice9_GetTextureStageState(dev, a, b, &v);
    else hr = set ? IDirect3DDevice9_SetSamplerState(dev, a, b, v) : IDirect3DDevice9_GetSamplerState(dev, a, b, &v);
    if (set) {
        if (hr) out("%s %ld%s%s set hr %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", hr);
    } else if (hr) out("%s %ld%s%s hr %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", hr);
    else out("%s %ld%s%s = %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", v);
}

/* sb begin | end <id> | create <id> <type 1 all, 2 pixel, 3 vertex> | capture <id> | apply <id> | release <id> */
static void op_sb(void) {
    if (dev8) { op_sb8(); return; }
    if (!dev) { out("nodev\n"); return; }
    HRESULT hr;
    if (is(1, "begin")) hr = IDirect3DDevice9_BeginStateBlock(dev);
    else {
        long i = num(2);
        if (bad || i < 0 || i >= BLOCKS) { bad = 1; return; }
        if (is(1, "end") || is(1, "create")) {
            if (blocks[i]) { IDirect3DStateBlock9_Release(blocks[i]); blocks[i] = NULL; }
            if (is(1, "end")) hr = IDirect3DDevice9_EndStateBlock(dev, &blocks[i]);
            else {
                long t = num(3);
                if (bad) return;
                hr = IDirect3DDevice9_CreateStateBlock(dev, t, &blocks[i]);
            }
            if (FAILED(hr)) blocks[i] = NULL;
        } else if (!blocks[i]) { out("sb %ld none\n", i); return; }
        else if (is(1, "capture")) hr = IDirect3DStateBlock9_Capture(blocks[i]);
        else if (is(1, "apply")) hr = IDirect3DStateBlock9_Apply(blocks[i]);
        else if (is(1, "release")) { IDirect3DStateBlock9_Release(blocks[i]); blocks[i] = NULL; hr = 0; }
        else { bad = 1; return; }
    }
    if (hr) out("sb %s hr %08lx\n", tok[1], hr);
}


/* ------------------------------------------------------------------------------------------ */
/* Direct3D 8 (d3d8.dll loaded when a case asks for it; its header clashes with d3d9's, so the
 * calls go through the vtables by index, d3d8.h's order) */

static void *d3d8;
#define VT(o, i) ((*(void ***)(o))[i])
typedef ULONG (WINAPI *Rel8)(void *);
static DWORD blocks8[BLOCKS];

static void d3d8_reset(void) {
    if (dev8) {
        for (int i = 0; i < BLOCKS; i++)
            if (blocks8[i]) ((HRESULT (WINAPI *)(void *, DWORD))VT(dev8, 56))(dev8, blocks8[i]);
        ((Rel8)VT(dev8, 2))(dev8);
    }
    memset(blocks8, 0, sizeof blocks8);
    dev8 = NULL;
    if (d3d8) { ((Rel8)VT(d3d8, 2))(d3d8); d3d8 = NULL; }
}

/* device8 [hwvp|swvp|mixed]: as device, for Direct3D 8 */
static void op_device8(void) {
    d3d9_reset();
    d3d8_reset();
    static void *(WINAPI *create)(UINT);
    if (!create) {
        HMODULE m = LoadLibraryA("d3d8.dll");
        if (m) create = (void *)GetProcAddress(m, "Direct3DCreate8");
    }
    if (!create || !(d3d8 = create(220))) { out("device8 none\n"); return; }
    if (!hwnd) {
        hwnd = kz_window();
    }
    DWORD flags = D3DCREATE_HARDWARE_VERTEXPROCESSING;
    if (is(1, "swvp")) flags = D3DCREATE_SOFTWARE_VERTEXPROCESSING;
    else if (is(1, "mixed")) flags = D3DCREATE_MIXED_VERTEXPROCESSING;
    /* D3DPRESENT_PARAMETERS of Direct3D 8: no MultiSampleQuality */
    struct { UINT w, h; D3DFORMAT fmt; UINT count; DWORD ms, swap; HWND win; BOOL windowed, autods; D3DFORMAT dsfmt; DWORD flags; UINT rate, interval; } pp = { 0 };
    pp.w = 64; pp.h = 64; pp.fmt = D3DFMT_X8R8G8B8; pp.count = 1; pp.swap = D3DSWAPEFFECT_DISCARD;
    pp.win = hwnd; pp.windowed = TRUE; pp.autods = TRUE; pp.dsfmt = D3DFMT_D24S8;
    typedef HRESULT (WINAPI *Create8)(void *, UINT, D3DDEVTYPE, HWND, DWORD, void *, void **);
    HRESULT hr = ((Create8)VT(d3d8, 15))(d3d8, 0, D3DDEVTYPE_HAL, hwnd, flags, &pp, &dev8);
    if (FAILED(hr)) hr = ((Create8)VT(d3d8, 15))(d3d8, 0, D3DDEVTYPE_HAL, hwnd, D3DCREATE_SOFTWARE_VERTEXPROCESSING, &pp, &dev8);
    if (FAILED(hr)) { out("device8 failed %08lx\n", hr); dev8 = NULL; return; }
    out("device8 ok\n");
}

static void op_state8(void) {
    int set = is(1, "set");
    if ((!set && !is(1, "get")) || is(0, "ss")) { bad = 1; return; }
    int two = is(0, "tss");
    long a = num(2), b = two ? num(3) : 0;
    DWORD v = set ? (DWORD)num(two ? 4 : 3) : 0xdeadbeef;
    if (bad) return;
    HRESULT hr;
    if (!two) hr = set ? ((HRESULT (WINAPI *)(void *, DWORD, DWORD))VT(dev8, 50))(dev8, a, v) : ((HRESULT (WINAPI *)(void *, DWORD, DWORD *))VT(dev8, 51))(dev8, a, &v);
    else hr = set ? ((HRESULT (WINAPI *)(void *, DWORD, DWORD, DWORD))VT(dev8, 63))(dev8, a, b, v) : ((HRESULT (WINAPI *)(void *, DWORD, DWORD, DWORD *))VT(dev8, 62))(dev8, a, b, &v);
    if (set) {
        if (hr) out("%s %ld%s%s set hr %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", hr);
    } else if (hr) out("%s %ld%s%s hr %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", hr);
    else out("%s %ld%s%s = %08lx\n", tok[0], a, two ? " " : "", two ? tok[3] : "", v);
}

static void op_sb8(void) {
    typedef HRESULT (WINAPI *Tok)(void *, DWORD);
    HRESULT hr;
    if (is(1, "begin")) hr = ((HRESULT (WINAPI *)(void *))VT(dev8, 52))(dev8);
    else {
        long i = num(2);
        if (bad || i < 0 || i >= BLOCKS) { bad = 1; return; }
        if (is(1, "end") || is(1, "create")) {
            if (blocks8[i]) { ((Tok)VT(dev8, 56))(dev8, blocks8[i]); blocks8[i] = 0; }
            if (is(1, "end")) hr = ((HRESULT (WINAPI *)(void *, DWORD *))VT(dev8, 53))(dev8, &blocks8[i]);
            else {
                long t = num(3);
                if (bad) return;
                hr = ((HRESULT (WINAPI *)(void *, DWORD, DWORD *))VT(dev8, 57))(dev8, t, &blocks8[i]);
            }
            if (FAILED(hr)) blocks8[i] = 0;
        } else if (!blocks8[i]) { out("sb %ld none\n", i); return; }
        else if (is(1, "capture")) hr = ((Tok)VT(dev8, 55))(dev8, blocks8[i]);
        else if (is(1, "apply")) hr = ((Tok)VT(dev8, 54))(dev8, blocks8[i]);
        else if (is(1, "release")) { hr = ((Tok)VT(dev8, 56))(dev8, blocks8[i]); blocks8[i] = 0; }
        else { bad = 1; return; }
    }
    if (hr) out("sb %s hr %08lx\n", tok[1], hr);
}

static void d3d9_reset_all(void) {
    d3d9_reset();
    d3d8_reset();
}

static const struct kz_op d3d9_ops[] = {
    { "device", op_device },
    { "rs", op_state },
    { "tss", op_state },
    { "ss", op_state },
    { "sb", op_sb },
    { "device8", op_device8 },
    { 0 },
};
const struct kz_api d3d9_api = { "d3d9", d3d9_ops, d3d9_reset_all };
