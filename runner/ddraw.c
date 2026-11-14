#include "core.h"
#include <ddraw.h>
#define SLOTS 8
static HWND hwnd;
/* DirectDraw 7: system-memory offscreen surfaces in slots (ddraw.dll loaded when first needed) */

static IDirectDraw7 *dd7;
static IDirectDrawSurface7 *dds[SLOTS];

static void ddraw_reset(void) {
    for (int i = 0; i < SLOTS; i++)
        if (dds[i]) { IDirectDrawSurface7_Release(dds[i]); dds[i] = NULL; }
    if (dd7) { IDirectDraw7_Release(dd7); dd7 = NULL; }
}

static IDirectDrawSurface7 *dslot(int i) {
    long s = num(i);
    if (bad || s < 0 || s >= SLOTS || !dds[s]) { bad = 1; return NULL; }
    return dds[s];
}

/* dd: DirectDrawCreateEx (IDirectDraw7), SetCooperativeLevel(DDSCL_NORMAL) */
static void op_dd(void) {
    ddraw_reset();
    static HRESULT (WINAPI *create)(GUID *, void **, REFIID, IUnknown *);
    if (!create) {
        HMODULE m = LoadLibraryA("ddraw.dll");
        if (m) create = (void *)GetProcAddress(m, "DirectDrawCreateEx");
    }
    static const GUID iid7 = { 0x15e65ec0, 0x3b9c, 0x11d2, { 0xb9, 0x2f, 0x00, 0x60, 0x97, 0x97, 0xea, 0x5b } };
    if (!create) { out("dd none\n"); return; }
    HRESULT hr = create(NULL, (void **)&dd7, &iid7, NULL);
    if (FAILED(hr)) { out("dd failed %08lx\n", hr); dd7 = NULL; return; }
    if (!hwnd) {
        hwnd = kz_window();
    }
    IDirectDraw7_SetCooperativeLevel(dd7, hwnd, DDSCL_NORMAL);
    out("dd ok\n");
}

/* ddsurf <s> <w> <h> <8|555|565|32> */
static void op_ddsurf(void) {
    long i = num(1), w = num(2), h = num(3);
    if (bad || i < 0 || i >= SLOTS || w < 1 || h < 1 || w > 256 || h > 256) { bad = 1; return; }
    if (!dd7) { out("nodev\n"); return; }
    if (dds[i]) { IDirectDrawSurface7_Release(dds[i]); dds[i] = NULL; }
    DDSURFACEDESC2 d = { sizeof d };
    d.dwFlags = DDSD_CAPS | DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT;
    d.dwWidth = w;
    d.dwHeight = h;
    d.ddsCaps.dwCaps = DDSCAPS_OFFSCREENPLAIN | DDSCAPS_SYSTEMMEMORY;
    d.ddpfPixelFormat.dwSize = sizeof d.ddpfPixelFormat;
    d.ddpfPixelFormat.dwFlags = DDPF_RGB;
    if (is(4, "8")) { d.ddpfPixelFormat.dwFlags |= DDPF_PALETTEINDEXED8; d.ddpfPixelFormat.dwRGBBitCount = 8; }
    else if (is(4, "555")) { d.ddpfPixelFormat.dwRGBBitCount = 16; d.ddpfPixelFormat.dwRBitMask = 0x7c00; d.ddpfPixelFormat.dwGBitMask = 0x3e0; d.ddpfPixelFormat.dwBBitMask = 0x1f; }
    else if (is(4, "565")) { d.ddpfPixelFormat.dwRGBBitCount = 16; d.ddpfPixelFormat.dwRBitMask = 0xf800; d.ddpfPixelFormat.dwGBitMask = 0x7e0; d.ddpfPixelFormat.dwBBitMask = 0x1f; }
    else if (is(4, "32")) { d.ddpfPixelFormat.dwRGBBitCount = 32; d.ddpfPixelFormat.dwRBitMask = 0xff0000; d.ddpfPixelFormat.dwGBitMask = 0xff00; d.ddpfPixelFormat.dwBBitMask = 0xff; }
    else { bad = 1; return; }
    HRESULT hr = IDirectDraw7_CreateSurface(dd7, &d, &dds[i], NULL);
    if (FAILED(hr)) { out("ddsurf failed %08lx\n", hr); dds[i] = NULL; }
}

/* ddfill <s> <seed> | dddump <s>: the bytes of each row's pixels through Lock */
static void op_ddlock(void) {
    IDirectDrawSurface7 *s = dslot(1);
    int fill = is(0, "ddfill");
    unsigned x = fill ? (unsigned)num(2) : 0;
    if (bad) return;
    DDSURFACEDESC2 d = { sizeof d };
    HRESULT hr = IDirectDrawSurface7_Lock(s, NULL, &d, DDLOCK_WAIT, NULL);
    if (FAILED(hr)) { out("lock hr %08lx\n", hr); return; }
    int row = d.dwWidth * d.ddpfPixelFormat.dwRGBBitCount / 8;
    if (!x) x = 0x9e3779b9;
    if (!fill) out("dd %s %lu %lu %lu\n", tok[1], d.dwWidth, d.dwHeight, d.ddpfPixelFormat.dwRGBBitCount);
    for (DWORD y = 0; y < d.dwHeight; y++) {
        unsigned char *p = (unsigned char *)d.lpSurface + y * d.lPitch;
        for (int i = 0; i < row; i++) {
            if (fill) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; p[i] = (unsigned char)x; }
            else out("%02x", p[i]);
        }
        if (!fill) out("\n");
    }
    IDirectDrawSurface7_Unlock(s, NULL);
}

/* ddkey <s> src|dest <value>|none: SetColorKey DDCKEY_SRCBLT / DESTBLT, low = high = value */
static void op_ddkey(void) {
    IDirectDrawSurface7 *s = dslot(1);
    DWORD which = is(2, "src") ? DDCKEY_SRCBLT : is(2, "dest") ? DDCKEY_DESTBLT : 0;
    if (!which) bad = 1;
    DDCOLORKEY k = { 0, 0 };
    int none = is(3, "none");
    if (!none) k.dwColorSpaceLowValue = k.dwColorSpaceHighValue = (DWORD)num(3);
    if (bad) return;
    HRESULT hr = IDirectDrawSurface7_SetColorKey(s, which, none ? NULL : &k);
    if (hr) out("ddkey hr %08lx\n", hr);
}

/* a rect at token i: "all" (NULL) or x0 y0 x1 y1; returns the next token */
static int ddrect(int i, RECT *r, RECT **pr) {
    if (is(i, "all")) { *pr = NULL; return i + 1; }
    r->left = num(i); r->top = num(i + 1); r->right = num(i + 2); r->bottom = num(i + 3);
    *pr = r;
    return i + 4;
}

/* ddblt <d> <rect> <s|-> <rect> [keysrc keydest mirrorlr mirrorud keysrcov <v> keydestov <v>]
 * ddcolor <d> <rect> <value>: Blt DDBLT_COLORFILL
 * ddfast <d> <x> <y> <s> <rect> [srckey destkey]: BltFast */
static void op_ddblt(void) {
    IDirectDrawSurface7 *d = dslot(1), *s = NULL;
    RECT dr, sr, *pdr = NULL, *psr = NULL;
    DDBLTFX fx = { sizeof fx };
    HRESULT hr;
    if (is(0, "ddfast")) {
        long x = num(2), y = num(3);
        s = dslot(4);
        int i = ddrect(5, &sr, &psr);
        DWORD f = DDBLTFAST_WAIT;
        for (; i < ntok; i++)
            if (is(i, "srckey")) f |= DDBLTFAST_SRCCOLORKEY;
            else if (is(i, "destkey")) f |= DDBLTFAST_DESTCOLORKEY;
            else bad = 1;
        if (bad) return;
        hr = IDirectDrawSurface7_BltFast(d, x, y, s, psr, f);
    } else if (is(0, "ddcolor")) {
        int i = ddrect(2, &dr, &pdr);
        fx.dwFillColor = (DWORD)num(i);
        if (bad || ntok != i + 1) { bad = 1; return; }
        hr = IDirectDrawSurface7_Blt(d, pdr, NULL, NULL, DDBLT_COLORFILL | DDBLT_WAIT, &fx);
    } else {
        int i = ddrect(2, &dr, &pdr);
        if (!is(i, "-")) s = dslot(i);
        i = ddrect(i + 1, &sr, &psr);
        DWORD f = DDBLT_WAIT;
        for (; i < ntok; i++)
            if (is(i, "keysrc")) f |= DDBLT_KEYSRC;
            else if (is(i, "keydest")) f |= DDBLT_KEYDEST;
            else if (is(i, "mirrorlr")) { f |= DDBLT_DDFX; fx.dwDDFX |= DDBLTFX_MIRRORLEFTRIGHT; }
            else if (is(i, "mirrorud")) { f |= DDBLT_DDFX; fx.dwDDFX |= DDBLTFX_MIRRORUPDOWN; }
            else if (is(i, "keysrcov")) { f |= DDBLT_KEYSRCOVERRIDE; fx.ddckSrcColorkey.dwColorSpaceLowValue = fx.ddckSrcColorkey.dwColorSpaceHighValue = (DWORD)num(++i); }
            else if (is(i, "keydestov")) { f |= DDBLT_KEYDESTOVERRIDE; fx.ddckDestColorkey.dwColorSpaceLowValue = fx.ddckDestColorkey.dwColorSpaceHighValue = (DWORD)num(++i); }
            else bad = 1;
        if (bad) return;
        hr = IDirectDrawSurface7_Blt(d, pdr, s, psr, f, &fx);
    }
    out("ret %08lx\n", hr);
}

static const struct kz_op ddraw_ops[] = {
    { "dd", op_dd },
    { "ddsurf", op_ddsurf },
    { "ddfill", op_ddlock },
    { "dddump", op_ddlock },
    { "ddkey", op_ddkey },
    { "ddblt", op_ddblt },
    { "ddcolor", op_ddblt },
    { "ddfast", op_ddblt },
    { 0 },
};
const struct kz_api ddraw_api = { "ddraw", ddraw_ops, ddraw_reset };
