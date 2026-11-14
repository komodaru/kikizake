#include "core.h"
#include <d3d.h>
/* Direct3D 1, 2, 3 and 7 (Direct3D immediate mode through DirectDraw; ddraw.dll loaded when
 * first needed): the device list, devices on an offscreen render target with an optional
 * z-buffer, render states, Clear and pretransformed triangles, the target's pixels through Lock */

static const GUID iid_dd2 = { 0xb3a6f3e0, 0x2b43, 0x11cf, { 0xa2, 0xde, 0x00, 0xaa, 0x00, 0xb9, 0x33, 0x56 } };
static const GUID iid_dd4 = { 0x9c59509a, 0x39bd, 0x11d1, { 0x8c, 0x4a, 0x00, 0xc0, 0x4f, 0xd9, 0x30, 0xc5 } };
static const GUID iid_dd7 = { 0x15e65ec0, 0x3b9c, 0x11d2, { 0xb9, 0x2f, 0x00, 0x60, 0x97, 0x97, 0xea, 0x5b } };
static const GUID iid_d3d1 = { 0x3bba0080, 0x2421, 0x11cf, { 0xa3, 0x1a, 0x00, 0xaa, 0x00, 0xb9, 0x33, 0x56 } };
static const GUID iid_d3d2 = { 0x6aae1ec1, 0x662a, 0x11d0, { 0x88, 0x9d, 0x00, 0xaa, 0x00, 0xbb, 0xb7, 0x6a } };
static const GUID iid_d3d3 = { 0xbb223240, 0xe72b, 0x11d0, { 0xa9, 0xb4, 0x00, 0xaa, 0x00, 0xc0, 0x99, 0x3e } };
static const GUID iid_d3d7 = { 0xf5049e77, 0x4861, 0x11d2, { 0xa4, 0x07, 0x00, 0xa0, 0xc9, 0x06, 0x29, 0xa8 } };
static const GUID iid_s4 = { 0x0b2b8630, 0xad35, 0x11d0, { 0x8e, 0xa6, 0x00, 0x60, 0x97, 0x97, 0xea, 0x5b } };
static const struct { const char *n; GUID g; int hw; } devs[] = {
    { "rgb", { 0xa4665c60, 0x2673, 0x11cf, { 0xa3, 0x1a, 0x00, 0xaa, 0x00, 0xb9, 0x33, 0x56 } }, 0 },
    { "hal", { 0x84e63de0, 0x46aa, 0x11cf, { 0x81, 0x6f, 0x00, 0x00, 0xc0, 0x20, 0x15, 0x6e } }, 1 },
    { "tnl", { 0xf5049e78, 0x4861, 0x11d2, { 0xa4, 0x07, 0x00, 0xa0, 0xc9, 0x06, 0x29, 0xa8 } }, 1 },
    { "ref", { 0x50936643, 0x13e9, 0x11d1, { 0x89, 0xaa, 0x00, 0xa0, 0xc9, 0x05, 0x41, 0x29 } }, 0 },
    { "mmx", { 0x881949a1, 0xd6f3, 0x11d0, { 0x89, 0xab, 0x00, 0xa0, 0xc9, 0x05, 0x41, 0x29 } }, 0 },
    { "ramp", { 0xf2086b20, 0x259f, 0x11cf, { 0xa3, 0x1a, 0x00, 0xaa, 0x00, 0xb9, 0x33, 0x56 } }, 0 },
    { "null", { 0x8767df22, 0xbacc, 0x11d1, { 0x89, 0x69, 0x00, 0xa0, 0xc9, 0x06, 0x29, 0xa8 } }, 0 },
};
#define NDEVS (int)(sizeof devs / sizeof *devs)

static int ver;
static IDirectDraw *dd1;
static IDirectDraw4 *dd4;
static IDirectDraw7 *dd7;
static IDirect3D *d1;
static IDirect3D2 *d2;
static IDirect3D3 *d3;
static IDirect3D7 *d7;
static IUnknown *rt, *zb;        /* IDirectDrawSurface (1, 2), 4 (3) or 7 (7) */
static IDirect3DDevice *dev1;
static IDirect3DDevice2 *dev2;
static IDirect3DDevice3 *dev3;
static IDirect3DDevice7 *dev7;
static IDirect3DViewport2 *vp2;
static IDirect3DViewport3 *vp3;
static IDirect3DMaterial2 *mat2;

#define REL(p) do { if (p) { IUnknown_Release((IUnknown *)(p)); (p) = NULL; } } while (0)

static void d3d7_reset(void) {
    REL(mat2);
    if (vp2 && dev2) IDirect3DDevice2_DeleteViewport(dev2, vp2);
    if (vp3 && dev3) IDirect3DDevice3_DeleteViewport(dev3, vp3);
    REL(vp2); REL(vp3);
    REL(dev1); REL(dev2); REL(dev3); REL(dev7);
    REL(zb); REL(rt);
    REL(d1); REL(d2); REL(d3); REL(d7);
    REL(dd4); REL(dd7); REL(dd1);
    ver = 0;
}

static HMODULE ddraw(void) {
    static HMODULE m;
    if (!m) m = LoadLibraryA("ddraw.dll");
    return m;
}

/* d3d <1|2|3|7>: DirectDrawCreate (DirectDrawCreateEx for 7), SetCooperativeLevel(DDSCL_NORMAL),
 * QueryInterface for IDirect3D, 2, 3 or 7 ("d3d ok", "d3d none", "d3d failed <hr>", "d3d qi <hr>") */
static void op_d3d(void) {
    long v = num(1);
    if (bad || (v != 1 && v != 2 && v != 3 && v != 7)) { bad = 1; return; }
    d3d7_reset();
    HMODULE m = ddraw();
    HRESULT (WINAPI *create)(GUID *, IDirectDraw **, IUnknown *) = m ? (void *)GetProcAddress(m, "DirectDrawCreate") : NULL;
    HRESULT (WINAPI *createex)(GUID *, void **, REFIID, IUnknown *) = m ? (void *)GetProcAddress(m, "DirectDrawCreateEx") : NULL;
    if (!create || !createex) { out("d3d none\n"); return; }
    HRESULT hr;
    if (v == 7) {
        hr = createex(NULL, (void **)&dd7, &iid_dd7, NULL);
        if (hr) { out("d3d failed %08lx\n", hr); dd7 = NULL; return; }
        IDirectDraw7_SetCooperativeLevel(dd7, kz_window(), DDSCL_NORMAL);
        hr = IDirectDraw7_QueryInterface(dd7, &iid_d3d7, (void **)&d7);
    } else {
        hr = create(NULL, &dd1, NULL);
        if (hr) { out("d3d failed %08lx\n", hr); dd1 = NULL; return; }
        IDirectDraw_SetCooperativeLevel(dd1, kz_window(), DDSCL_NORMAL);
        if (v == 3) {
            hr = IDirectDraw_QueryInterface(dd1, &iid_dd4, (void **)&dd4);
            if (!hr) hr = IDirectDraw4_QueryInterface(dd4, &iid_d3d3, (void **)&d3);
        } else hr = IDirectDraw_QueryInterface(dd1, v == 2 ? &iid_d3d2 : &iid_d3d1, v == 2 ? (void **)&d2 : (void **)&d1);
    }
    if (hr) { out("d3d qi %08lx\n", hr); d1 = NULL; d2 = NULL; d3 = NULL; d7 = NULL; return; }
    ver = v;
    out("d3d ok\n");
}

static void guid_out(const GUID *g) {
    out("%08lx-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x", g->Data1, g->Data2, g->Data3, g->Data4[0], g->Data4[1], g->Data4[2], g->Data4[3], g->Data4[4], g->Data4[5], g->Data4[6], g->Data4[7]);
}

static void desc_out(const char *what, const D3DDEVICEDESC *d) {
    DWORD n = d->dwSize;
    if (n > sizeof *d) n = sizeof *d;
    out("%s %lu ", what, d->dwSize);
    out_hex((const unsigned char *)d, n);
}

static HRESULT CALLBACK enum_cb(GUID *g, char *desc, char *name, D3DDEVICEDESC *hal, D3DDEVICEDESC *hel, void *ctx) {
    out("dev ");
    guid_out(g);
    out(" \"%s\" \"%s\"\n", name, desc);
    if (*(int *)ctx) { desc_out("hal", hal); desc_out("hel", hel); }
    return D3DENUMRET_OK;
}

static HRESULT CALLBACK enum7_cb(char *desc, char *name, D3DDEVICEDESC7 *d, void *ctx) {
    out("dev ");
    guid_out(&d->deviceGUID);
    out(" \"%s\" \"%s\"\n", name, desc);
    if (*(int *)ctx) {
        out("caps ");
        out_hex((const unsigned char *)d, sizeof *d);
    }
    return D3DENUMRET_OK;
}

/* d3denum [caps]: EnumDevices: "dev <guid> "<name>" "<description>"" each, with caps: the
 * D3DDEVICEDESCs (hal, hel: dwSize and its bytes) or the D3DDEVICEDESC7 as hex */
static void op_d3denum(void) {
    int caps = is(1, "caps");
    if (ntok > 2 || (ntok == 2 && !caps)) { bad = 1; return; }
    if (!ver) { out("nodev\n"); return; }
    HRESULT hr = ver == 7 ? IDirect3D7_EnumDevices(d7, enum7_cb, &caps)
               : ver == 3 ? IDirect3D3_EnumDevices(d3, enum_cb, &caps)
               : ver == 2 ? IDirect3D2_EnumDevices(d2, enum_cb, &caps)
               : IDirect3D_EnumDevices(d1, enum_cb, &caps);
    if (hr) out("enum hr %08lx\n", hr);
}

/* a surface of the current version's DirectDraw */
static HRESULT surface(DDSURFACEDESC2 *d, IUnknown **s) {
    if (ver == 7) return IDirectDraw7_CreateSurface(dd7, d, (IDirectDrawSurface7 **)s, NULL);
    if (ver == 3) return IDirectDraw4_CreateSurface(dd4, d, (IDirectDrawSurface4 **)s, NULL);
    DDSURFACEDESC d1d;
    memcpy(&d1d, d, sizeof d1d);
    d1d.dwSize = sizeof d1d;
    d1d.ddsCaps.dwCaps = d->ddsCaps.dwCaps;
    d1d.ddpfPixelFormat = d->ddpfPixelFormat;
    return IDirectDraw_CreateSurface(dd1, &d1d, (IDirectDrawSurface **)s, NULL);
}

static int devw, devh;

/* d3ddev <rgb|hal|tnl|ref|mmx|ramp|null> <565|555|32|24|8> <w> <h> [z16|z24|z32]: the render
 * target (DDSCAPS_OFFSCREENPLAIN | 3DDEVICE; system memory, video memory for hal and tnl), the
 * z-buffer attached to it, the device (version 1: the surface's QueryInterface), and for 2 and 3
 * a viewport of the whole target made current ("d3ddev ok", "d3ddev <step> hr <hr>") */
static void op_d3ddev(void) {
    int k = 0;
    while (k < NDEVS && !is(1, devs[k].n)) k++;
    long w = num(3), h = num(4);
    int zbits = is(5, "z16") ? 16 : is(5, "z24") ? 24 : is(5, "z32") ? 32 : 0;
    if (k == NDEVS || bad || w < 1 || h < 1 || w > 256 || h > 256 || (ntok > 5 && !zbits) || ntok > 6) { bad = 1; return; }
    DDSURFACEDESC2 d = { sizeof d };
    d.dwFlags = DDSD_CAPS | DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT;
    d.dwWidth = w; d.dwHeight = h;
    DWORD mem = devs[k].hw ? DDSCAPS_VIDEOMEMORY : DDSCAPS_SYSTEMMEMORY;
    d.ddsCaps.dwCaps = DDSCAPS_OFFSCREENPLAIN | DDSCAPS_3DDEVICE | mem;
    DDPIXELFORMAT *f = &d.ddpfPixelFormat;
    f->dwSize = sizeof *f;
    f->dwFlags = DDPF_RGB;
    if (is(2, "565")) { f->dwRGBBitCount = 16; f->dwRBitMask = 0xf800; f->dwGBitMask = 0x7e0; f->dwBBitMask = 0x1f; }
    else if (is(2, "555")) { f->dwRGBBitCount = 16; f->dwRBitMask = 0x7c00; f->dwGBitMask = 0x3e0; f->dwBBitMask = 0x1f; }
    else if (is(2, "32")) { f->dwRGBBitCount = 32; f->dwRBitMask = 0xff0000; f->dwGBitMask = 0xff00; f->dwBBitMask = 0xff; }
    else if (is(2, "24")) { f->dwRGBBitCount = 24; f->dwRBitMask = 0xff0000; f->dwGBitMask = 0xff00; f->dwBBitMask = 0xff; }
    else if (is(2, "8")) { f->dwFlags |= DDPF_PALETTEINDEXED8; f->dwRGBBitCount = 8; }
    else { bad = 1; return; }
    if (!ver) { out("nodev\n"); return; }
    REL(mat2);
    if (vp2 && dev2) IDirect3DDevice2_DeleteViewport(dev2, vp2);
    if (vp3 && dev3) IDirect3DDevice3_DeleteViewport(dev3, vp3);
    REL(vp2); REL(vp3); REL(dev1); REL(dev2); REL(dev3); REL(dev7); REL(zb); REL(rt);
    HRESULT hr = surface(&d, &rt);
    if (hr) { out("d3ddev surface hr %08lx\n", hr); rt = NULL; return; }
    if (zbits) {
        DDSURFACEDESC2 z = { sizeof z };
        z.dwFlags = DDSD_CAPS | DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT;
        z.dwWidth = w; z.dwHeight = h;
        z.ddsCaps.dwCaps = DDSCAPS_ZBUFFER | mem;
        z.ddpfPixelFormat.dwSize = sizeof z.ddpfPixelFormat;
        z.ddpfPixelFormat.dwFlags = DDPF_ZBUFFER;
        z.ddpfPixelFormat.dwZBufferBitDepth = zbits;
        z.ddpfPixelFormat.dwZBitMask = zbits == 32 ? 0xffffffff : (1u << zbits) - 1;
        if (ver < 3) {
            /* (DDSURFACEDESC's dwZBufferBitDepth, DDSD_ZBUFFERBITDEPTH, no pixel format) */
            DDSURFACEDESC z1 = { sizeof z1 };
            z1.dwFlags = DDSD_CAPS | DDSD_WIDTH | DDSD_HEIGHT | DDSD_ZBUFFERBITDEPTH;
            z1.dwWidth = w; z1.dwHeight = h;
            z1.ddsCaps.dwCaps = z.ddsCaps.dwCaps;
            z1.dwZBufferBitDepth = zbits;
            hr = IDirectDraw_CreateSurface(dd1, &z1, (IDirectDrawSurface **)&zb, NULL);
        } else hr = surface(&z, &zb);
        if (hr) { out("d3ddev z hr %08lx\n", hr); zb = NULL; return; }
        hr = ver == 7 ? IDirectDrawSurface7_AddAttachedSurface((IDirectDrawSurface7 *)rt, (IDirectDrawSurface7 *)zb)
           : ver == 3 ? IDirectDrawSurface4_AddAttachedSurface((IDirectDrawSurface4 *)rt, (IDirectDrawSurface4 *)zb)
           : IDirectDrawSurface_AddAttachedSurface((IDirectDrawSurface *)rt, (IDirectDrawSurface *)zb);
        if (hr) { out("d3ddev attach hr %08lx\n", hr); return; }
    }
    const GUID *g = &devs[k].g;
    if (ver == 7) hr = IDirect3D7_CreateDevice(d7, g, (IDirectDrawSurface7 *)rt, &dev7);
    else if (ver == 3) hr = IDirect3D3_CreateDevice(d3, g, (IDirectDrawSurface4 *)rt, &dev3, NULL);
    else if (ver == 2) hr = IDirect3D2_CreateDevice(d2, g, (IDirectDrawSurface *)rt, &dev2);
    else hr = IDirectDrawSurface_QueryInterface((IDirectDrawSurface *)rt, g, (void **)&dev1);
    if (hr) { out("d3ddev hr %08lx\n", hr); dev1 = NULL; dev2 = NULL; dev3 = NULL; dev7 = NULL; return; }
    devw = w; devh = h;
    D3DVIEWPORT2 v = { sizeof v, 0, 0, w, h, -1.0f, 1.0f, 2.0f, 2.0f, 0.0f, 1.0f };
    if (ver == 3) {
        if (!(hr = IDirect3D3_CreateViewport(d3, &vp3, NULL)) && !(hr = IDirect3DDevice3_AddViewport(dev3, vp3)) && !(hr = IDirect3DViewport3_SetViewport2(vp3, &v)))
            hr = IDirect3DDevice3_SetCurrentViewport(dev3, vp3);
        if (hr) { out("d3ddev viewport hr %08lx\n", hr); return; }
    } else if (ver == 2) {
        if (!(hr = IDirect3D2_CreateViewport(d2, &vp2, NULL)) && !(hr = IDirect3DDevice2_AddViewport(dev2, vp2)) && !(hr = IDirect3DViewport2_SetViewport2(vp2, &v)))
            hr = IDirect3DDevice2_SetCurrentViewport(dev2, vp2);
        if (!hr) hr = IDirect3D2_CreateMaterial(d2, &mat2, NULL);
        if (hr) { out("d3ddev viewport hr %08lx\n", hr); return; }
    }
    out("d3ddev ok\n");
}

/* d3drs get <state> / d3drs set <state> <v>: GetRenderState / SetRenderState (version 2, 3, 7;
 * "rs <state> = <v>", "rs <state> hr <hr>", "rs set <state> hr <hr>" on failure) */
static void op_d3drs(void) {
    int set = is(1, "set");
    if (!set && !is(1, "get")) { bad = 1; return; }
    long st = num(2);
    DWORD v = set ? (DWORD)num(3) : 0;
    if (bad || ntok != (set ? 4 : 3)) { bad = 1; return; }
    if (!dev2 && !dev3 && !dev7) { out("nodev\n"); return; }
    HRESULT hr;
    if (set) {
        hr = dev7 ? IDirect3DDevice7_SetRenderState(dev7, st, v) : dev3 ? IDirect3DDevice3_SetRenderState(dev3, st, v) : IDirect3DDevice2_SetRenderState(dev2, st, v);
        if (hr) out("rs set %ld hr %08lx\n", st, hr);
    } else {
        v = 0xcccccccc;
        hr = dev7 ? IDirect3DDevice7_GetRenderState(dev7, st, &v) : dev3 ? IDirect3DDevice3_GetRenderState(dev3, st, &v) : IDirect3DDevice2_GetRenderState(dev2, st, &v);
        if (hr) out("rs %ld hr %08lx\n", st, hr);
        else out("rs %ld = %08lx\n", st, v);
    }
}

static D3DVALUE comp(DWORD c, int sh) { return (D3DVALUE)(c >> sh & 255) / 255.0f; }

/* d3dclear <argb> [z <millis>] [<x0> <y0> <x1> <y1>]: Clear of the target (and the z-buffer to
 * z / 1000): version 7 Clear, 3 Clear2, 2 the viewport's background material and Clear */
static void op_d3dclear(void) {
    DWORD c = (DWORD)num(1);
    int i = 2, zf = 0;
    float z = 0;
    if (is(i, "z")) { zf = 1; z = num(i + 1) / 1000.0f; i += 2; }
    D3DRECT r;
    r.x1 = 0; r.y1 = 0; r.x2 = devw; r.y2 = devh;
    if (ntok == i + 4) { r.x1 = num(i); r.y1 = num(i + 1); r.x2 = num(i + 2); r.y2 = num(i + 3); }
    else if (ntok != i) bad = 1;
    if (bad) return;
    if (!dev2 && !dev3 && !dev7) { out("nodev\n"); return; }
    HRESULT hr;
    DWORD f = D3DCLEAR_TARGET | (zf ? D3DCLEAR_ZBUFFER : 0);
    if (dev7) hr = IDirect3DDevice7_Clear(dev7, 1, &r, f, c, z, 0);
    else if (dev3) hr = IDirect3DViewport3_Clear2(vp3, 1, &r, f, c, z, 0);
    else {
        D3DMATERIAL m = { sizeof m };
        m.dcvDiffuse.r = comp(c, 16); m.dcvDiffuse.g = comp(c, 8); m.dcvDiffuse.b = comp(c, 0); m.dcvDiffuse.a = comp(c, 24);
        D3DMATERIALHANDLE mh = 0;
        if (!(hr = IDirect3DMaterial2_SetMaterial(mat2, &m)) && !(hr = IDirect3DMaterial2_GetHandle(mat2, dev2, &mh)) && !(hr = IDirect3DViewport2_SetBackground(vp2, mh)))
            hr = IDirect3DViewport2_Clear(vp2, 1, &r, f);
    }
    if (hr) out("clear hr %08lx\n", hr);
}

/* d3dtri <c0>[/<c1>/<c2>] <x0> <y0> <x1> <y1> <x2> <y2> [z <millis>]: BeginScene, a D3DTLVERTEX
 * triangle list of one triangle (x, y in 16ths of a pixel, rhw 1, the diffuse colors ARGB,
 * specular 0), EndScene ("tri <step> hr <hr>" on failure) */
static void op_d3dtri(void) {
    D3DTLVERTEX v[3];
    memset(v, 0, sizeof v);
    if (ntok < 8) { bad = 1; return; }
    DWORD c[3];
    char *p = tok[1], *e;
    int nc = 0;
    while (nc < 3) {
        c[nc++] = (DWORD)strtoul(p, &e, 0);
        if (e == p) bad = 1;
        if (*e != '/') break;
        p = e + 1;
    }
    if (*e || (nc != 1 && nc != 3)) bad = 1;
    if (nc == 1) c[1] = c[2] = c[0];
    float z = 0.5f;
    DWORD sp[3] = { 0, 0, 0 };
    for (int k = 8; k < ntok; k += 2) {
        if (k + 1 >= ntok) bad = 1;
        else if (is(k, "z")) z = num(k + 1) / 1000.0f;
        else if (is(k, "spec")) {
            /* the specular colors (alpha: the vertex fog factor), one or one a vertex */
            int ns = 0;
            p = tok[k + 1];
            while (ns < 3) {
                sp[ns++] = (DWORD)strtoul(p, &e, 0);
                if (e == p) bad = 1;
                if (*e != '/') break;
                p = e + 1;
            }
            if (*e || (ns != 1 && ns != 3)) bad = 1;
            if (ns == 1) sp[1] = sp[2] = sp[0];
        } else bad = 1;
    }
    for (int k = 0; k < 3; k++) {
        v[k].sx = num(2 + 2 * k) / 16.0f;
        v[k].sy = num(3 + 2 * k) / 16.0f;
        v[k].sz = z;
        v[k].rhw = 1.0f;
        v[k].color = c[k];
        v[k].specular = sp[k];
    }
    if (bad) return;
    if (!dev2 && !dev3 && !dev7) { out("nodev\n"); return; }
    HRESULT hr = dev7 ? IDirect3DDevice7_BeginScene(dev7) : dev3 ? IDirect3DDevice3_BeginScene(dev3) : IDirect3DDevice2_BeginScene(dev2);
    if (hr) { out("tri begin hr %08lx\n", hr); return; }
    hr = dev7 ? IDirect3DDevice7_DrawPrimitive(dev7, D3DPT_TRIANGLELIST, D3DFVF_TLVERTEX, v, 3, 0)
       : dev3 ? IDirect3DDevice3_DrawPrimitive(dev3, D3DPT_TRIANGLELIST, D3DFVF_TLVERTEX, v, 3, 0)
       : IDirect3DDevice2_DrawPrimitive(dev2, D3DPT_TRIANGLELIST, D3DVT_TLVERTEX, v, 3, 0);
    if (hr) out("tri draw hr %08lx\n", hr);
    hr = dev7 ? IDirect3DDevice7_EndScene(dev7) : dev3 ? IDirect3DDevice3_EndScene(dev3) : IDirect3DDevice2_EndScene(dev2);
    if (hr) out("tri end hr %08lx\n", hr);
}

/* d3dfill <seed> / d3ddump / d3dzdump: the target's (z-buffer's) bytes through Lock, as ddfill
 * and dddump ("dd rt|z <w> <h> <bits>" and the rows) */
static void op_d3dlock(void) {
    int z = is(0, "d3dzdump"), fill = is(0, "d3dfill");
    unsigned x = fill ? (unsigned)num(1) : 0;
    if (bad || ntok != (fill ? 2 : 1)) { bad = 1; return; }
    IUnknown *s = z ? zb : rt;
    if (!s || !(dev1 || dev2 || dev3 || dev7)) { out("nodev\n"); return; }
    DDSURFACEDESC2 d = { sizeof d };
    DDSURFACEDESC d1d = { sizeof d1d };
    HRESULT hr;
    unsigned char *base;
    long pitch;
    DWORD w, h, bits;
    if (ver == 7 || ver == 3) {
        hr = ver == 7 ? IDirectDrawSurface7_Lock((IDirectDrawSurface7 *)s, NULL, &d, DDLOCK_WAIT, NULL) : IDirectDrawSurface4_Lock((IDirectDrawSurface4 *)s, NULL, (DDSURFACEDESC2 *)&d, DDLOCK_WAIT, NULL);
        base = d.lpSurface; pitch = d.lPitch; w = d.dwWidth; h = d.dwHeight;
        bits = z ? d.ddpfPixelFormat.dwZBufferBitDepth : d.ddpfPixelFormat.dwRGBBitCount;
    } else {
        hr = IDirectDrawSurface_Lock((IDirectDrawSurface *)s, NULL, &d1d, DDLOCK_WAIT, NULL);
        base = d1d.lpSurface; pitch = d1d.lPitch; w = d1d.dwWidth; h = d1d.dwHeight;
        bits = z ? (d1d.dwFlags & DDSD_ZBUFFERBITDEPTH ? d1d.dwZBufferBitDepth : d1d.ddpfPixelFormat.dwZBufferBitDepth) : d1d.ddpfPixelFormat.dwRGBBitCount;
    }
    if (hr) { out("lock hr %08lx\n", hr); return; }
    /* (a 24-bit z-buffer is 32 bits a pixel) */
    int bpp = z && bits == 24 ? 32 : (int)bits;
    if (!fill) out("dd %s %lu %lu %lu\n", z ? "z" : "rt", w, h, bits);
    if (!x) x = 0x9e3779b9;
    for (DWORD y = 0; y < h; y++) {
        unsigned char *p = base + y * pitch;
        int row = w * bpp / 8;
        for (int i = 0; i < row; i++)
            if (fill) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; p[i] = (unsigned char)x; }
        if (!fill) out_hex(p, row);
    }
    if (ver == 7) IDirectDrawSurface7_Unlock((IDirectDrawSurface7 *)s, NULL);
    else if (ver == 3) IDirectDrawSurface4_Unlock((IDirectDrawSurface4 *)s, NULL);
    else IDirectDrawSurface_Unlock((IDirectDrawSurface *)s, NULL);
}

static const struct kz_op d3d7_ops[] = {
    { "d3d", op_d3d },
    { "d3denum", op_d3denum },
    { "d3ddev", op_d3ddev },
    { "d3drs", op_d3drs },
    { "d3dclear", op_d3dclear },
    { "d3dtri", op_d3dtri },
    { "d3dfill", op_d3dlock },
    { "d3ddump", op_d3dlock },
    { "d3dzdump", op_d3dlock },
    { 0 },
};
const struct kz_api d3d7_api = { "d3d7", d3d7_ops, d3d7_reset };
