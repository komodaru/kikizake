#include "core.h"
#include <ddraw.h>
#define SLOTS 8
static HWND hwnd;
/* DirectDraw 7: system-memory offscreen surfaces in slots (ddraw.dll loaded when first needed) */

static IDirectDraw7 *dd7;
static IDirectDrawSurface7 *dds[SLOTS];
static HDC ddc[SLOTS];                 /* GetDC's DC while held */
static unsigned char *dlock[SLOTS];    /* ddlock's pointer while locked */
static long dpitch[SLOTS];
static unsigned char *dbase[SLOTS];    /* the whole surface's first byte (a Lock at ddsurf) */
static IDirectDrawPalette *dpal[4];
static int droot[SLOTS];               /* 1 + the slot of the chain's front buffer, for its other surfaces */
static int dexcl;                      /* ddcoop exclusive set */

static void ddraw_reset(void) {
    for (int i = 0; i < SLOTS; i++) {
        if (dds[i] && ddc[i]) IDirectDrawSurface7_ReleaseDC(dds[i], ddc[i]);
        if (dds[i] && dlock[i]) IDirectDrawSurface7_Unlock(dds[i], NULL);
        ddc[i] = NULL; dlock[i] = NULL; dbase[i] = NULL;
    }
    /* (later slots first: a flipping chain's back buffers before its front buffer) */
    for (int i = SLOTS - 1; i >= 0; i--) {
        droot[i] = 0;
        if (dds[i]) { IDirectDrawSurface7_Release(dds[i]); dds[i] = NULL; }
    }
    for (int i = 0; i < 4; i++)
        if (dpal[i]) { IDirectDrawPalette_Release(dpal[i]); dpal[i] = NULL; }
    if (dd7 && dexcl) IDirectDraw7_SetCooperativeLevel(dd7, hwnd, DDSCL_NORMAL);
    dexcl = 0;
    if (dd7) { IDirectDraw7_Release(dd7); dd7 = NULL; }
}

static void ddfree(int i) {
    /* (a front buffer takes its chain with it: drop the slots that hold the rest first) */
    for (int k = SLOTS - 1; k >= 0; k--)
        if (k != i && droot[k] == i + 1) { droot[k] = 0; ddfree(k); }
    droot[i] = 0;
    if (dds[i] && ddc[i]) IDirectDrawSurface7_ReleaseDC(dds[i], ddc[i]);
    if (dds[i] && dlock[i]) IDirectDrawSurface7_Unlock(dds[i], NULL);
    ddc[i] = NULL; dlock[i] = NULL; dbase[i] = NULL;
    if (dds[i]) { IDirectDrawSurface7_Release(dds[i]); dds[i] = NULL; }
}

/* a line naming an empty slot (a surface, palette or DC that a failed call did not make)
 * prints "noslot" / "nopal" / "nodc" once and does nothing */
static int skipline = -1;
static int gone(void) { return bad || skipline == lineno; }
static void skip(const char *what) {
    if (skipline != lineno) out("%s\n", what);
    skipline = lineno;
}

static IDirectDrawSurface7 *dslot(int i) {
    long s = num(i);
    if (bad || s < 0 || s >= SLOTS) { bad = 1; return NULL; }
    if (!dds[s]) skip("noslot");
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

/* the base address of a surface (sysmem surfaces do not move) */
static void ddbase(int i) {
    DDSURFACEDESC2 d = { sizeof d };
    if (SUCCEEDED(IDirectDrawSurface7_Lock(dds[i], NULL, &d, DDLOCK_WAIT, NULL))) {
        dbase[i] = d.lpSurface;
        IDirectDrawSurface7_Unlock(dds[i], NULL);
    }
}

/* ddsurf <s> <w> <h> <8|555|565|24|32|a32> [vid]: vid = DDSCAPS_VIDEOMEMORY in place of SYSTEMMEMORY */
static void op_ddsurf(void) {
    long i = num(1), w = num(2), h = num(3);
    if (bad || i < 0 || i >= SLOTS || w < 1 || h < 1 || w > 256 || h > 256) { bad = 1; return; }
    if (!dd7) { out("nodev\n"); return; }
    ddfree(i);
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
    else if (is(4, "a32")) { d.ddpfPixelFormat.dwFlags |= DDPF_ALPHAPIXELS; d.ddpfPixelFormat.dwRGBBitCount = 32; d.ddpfPixelFormat.dwRBitMask = 0xff0000; d.ddpfPixelFormat.dwGBitMask = 0xff00; d.ddpfPixelFormat.dwBBitMask = 0xff; d.ddpfPixelFormat.dwRGBAlphaBitMask = 0xff000000; }
    else if (is(4, "24")) { d.ddpfPixelFormat.dwRGBBitCount = 24; d.ddpfPixelFormat.dwRBitMask = 0xff0000; d.ddpfPixelFormat.dwGBitMask = 0xff00; d.ddpfPixelFormat.dwBBitMask = 0xff; }
    else { bad = 1; return; }
    if (is(5, "vid")) d.ddsCaps.dwCaps = DDSCAPS_OFFSCREENPLAIN | DDSCAPS_VIDEOMEMORY;
    else if (ntok > 5) { bad = 1; return; }
    HRESULT hr = IDirectDraw7_CreateSurface(dd7, &d, &dds[i], NULL);
    if (FAILED(hr)) { out("ddsurf failed %08lx\n", hr); dds[i] = NULL; return; }
    ddbase(i);
}

static int ddrect(int i, RECT *r, RECT **pr);

/* ddfill <s> <seed> | dddump <s> [x0 y0 x1 y1]: the bytes of each row's pixels through Lock */
static void op_ddlock(void) {
    IDirectDrawSurface7 *s = dslot(1);
    int fill = is(0, "ddfill");
    unsigned x = fill ? (unsigned)num(2) : 0;
    RECT r, *pr = NULL;
    if (!fill && ntok > 2) ddrect(2, &r, &pr);
    if (gone()) return;
    DDSURFACEDESC2 d = { sizeof d };
    HRESULT hr = IDirectDrawSurface7_Lock(s, pr, &d, DDLOCK_WAIT, NULL);
    if (FAILED(hr)) { out("lock hr %08lx\n", hr); return; }
    if (pr) { d.dwWidth = r.right - r.left; d.dwHeight = r.bottom - r.top; }
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
    IDirectDrawSurface7_Unlock(s, pr);
}

/* ddkey <s> src|dest <value>|none: SetColorKey DDCKEY_SRCBLT / DESTBLT, low = high = value */
static void op_ddkey(void) {
    IDirectDrawSurface7 *s = dslot(1);
    DWORD which = is(2, "src") ? DDCKEY_SRCBLT : is(2, "dest") ? DDCKEY_DESTBLT : 0;
    if (!which) bad = 1;
    DDCOLORKEY k = { 0, 0 };
    int none = is(3, "none");
    if (!none) k.dwColorSpaceLowValue = k.dwColorSpaceHighValue = (DWORD)num(3);
    if (gone()) return;
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
        if (gone()) return;
        hr = IDirectDrawSurface7_BltFast(d, x, y, s, psr, f);
    } else if (is(0, "ddcolor")) {
        int i = ddrect(2, &dr, &pdr);
        fx.dwFillColor = (DWORD)num(i);
        if (bad || ntok != i + 1) { bad = 1; return; }
        if (gone()) return;
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
        if (gone()) return;
        hr = IDirectDrawSurface7_Blt(d, pdr, s, psr, f, &fx);
    }
    out("ret %08lx\n", hr);
}


/* ------------------------------------------------------------------------------------------ */
/* clippers */

/* ddclip <s> none | empty | <x0 y0 x1 y1>...: SetClipper(NULL), or CreateClipper with that clip
 * list (empty: none set) attached by SetClipper */
static void op_ddclip(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    if (is(2, "none")) {
        HRESULT hr = IDirectDrawSurface7_SetClipper(s, NULL);
        if (hr) out("clip set hr %08lx\n", hr);
        return;
    }
    int n = is(2, "empty") ? 0 : (ntok - 2) / 4;
    if (!is(2, "empty") && (ntok - 2) % 4) { bad = 1; return; }
    struct { RGNDATAHEADER h; RECT r[16]; } rd = { { sizeof(RGNDATAHEADER), RDH_RECTANGLES, n, n * sizeof(RECT) } };
    if (n > 16) { bad = 1; return; }
    for (int k = 0; k < n; k++) {
        RECT *r = &rd.r[k];
        r->left = num(2 + 4 * k); r->top = num(3 + 4 * k); r->right = num(4 + 4 * k); r->bottom = num(5 + 4 * k);
        if (!k) rd.h.rcBound = *r;
        else {
            if (r->left < rd.h.rcBound.left) rd.h.rcBound.left = r->left;
            if (r->top < rd.h.rcBound.top) rd.h.rcBound.top = r->top;
            if (r->right > rd.h.rcBound.right) rd.h.rcBound.right = r->right;
            if (r->bottom > rd.h.rcBound.bottom) rd.h.rcBound.bottom = r->bottom;
        }
    }
    if (gone()) return;
    IDirectDrawClipper *c;
    HRESULT hr = IDirectDraw7_CreateClipper(dd7, 0, &c, NULL);
    if (hr) { out("clip create hr %08lx\n", hr); return; }
    if (n && (hr = IDirectDrawClipper_SetClipList(c, (RGNDATA *)&rd, 0))) out("clip list hr %08lx\n", hr);
    if ((hr = IDirectDrawSurface7_SetClipper(s, c))) out("clip set hr %08lx\n", hr);
    IDirectDrawClipper_Release(c);
}

/* ddcliplist <s>: GetClipper, GetClipList: "cliplist <n> <bound> : <rects>" */
static void op_ddcliplist(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    IDirectDrawClipper *c;
    HRESULT hr = IDirectDrawSurface7_GetClipper(s, &c);
    if (hr) { out("cliplist clipper hr %08lx\n", hr); return; }
    DWORD size = 0;
    hr = IDirectDrawClipper_GetClipList(c, NULL, NULL, &size);
    if (hr) { out("cliplist hr %08lx\n", hr); IDirectDrawClipper_Release(c); return; }
    RGNDATA *rd = calloc(1, size + sizeof(RGNDATAHEADER));
    hr = IDirectDrawClipper_GetClipList(c, NULL, rd, &size);
    if (hr) out("cliplist hr %08lx\n", hr);
    else {
        RECT *r = (RECT *)rd->Buffer, *b = &rd->rdh.rcBound;
        out("cliplist %lu %ld %ld %ld %ld :", rd->rdh.nCount, b->left, b->top, b->right, b->bottom);
        for (DWORD k = 0; k < rd->rdh.nCount && k < 64; k++) out(" %ld %ld %ld %ld", r[k].left, r[k].top, r[k].right, r[k].bottom);
        out("\n");
    }
    free(rd);
    IDirectDrawClipper_Release(c);
}

/* ------------------------------------------------------------------------------------------ */
/* palettes: four slots */

static int pslot(int i) {
    long p = num(i);
    if (bad || p < 0 || p >= 4) { bad = 1; return 0; }
    return (int)p;
}

/* 256 entries from xorshift32 (r, g, b; peFlags 0) */
static void pal_entries(PALETTEENTRY *e, unsigned x) {
    if (!x) x = 0x9e3779b9;
    for (int i = 0; i < 256; i++) {
        unsigned char c[3];
        for (int k = 0; k < 3; k++) { x ^= x << 13; x ^= x >> 17; x ^= x << 5; c[k] = (unsigned char)x; }
        e[i].peRed = c[0]; e[i].peGreen = c[1]; e[i].peBlue = c[2]; e[i].peFlags = 0;
    }
}

/* ddpal <p> <seed> <flag>...: CreatePalette (1bit 2bit 4bit 8bit 8bitentries allow256 primary
 * alpha initialize) */
static void op_ddpal(void) {
    int p = pslot(1);
    unsigned seed = (unsigned)num(2);
    static const struct { const char *n; DWORD f; } fl[] = {
        { "1bit", DDPCAPS_1BIT }, { "2bit", DDPCAPS_2BIT }, { "4bit", DDPCAPS_4BIT }, { "8bit", DDPCAPS_8BIT },
        { "8bitentries", DDPCAPS_8BITENTRIES }, { "allow256", DDPCAPS_ALLOW256 }, { "primary", DDPCAPS_PRIMARYSURFACE },
        { "alpha", DDPCAPS_ALPHA }, { "initialize", DDPCAPS_INITIALIZE },
    };
    DWORD f = 0;
    for (int i = 3; i < ntok; i++) {
        int k = 0;
        while (k < (int)(sizeof fl / sizeof *fl) && !is(i, fl[k].n)) k++;
        if (k == (int)(sizeof fl / sizeof *fl)) bad = 1;
        else f |= fl[k].f;
    }
    if (gone()) return;
    if (!dd7) { out("nodev\n"); return; }
    if (dpal[p]) { IDirectDrawPalette_Release(dpal[p]); dpal[p] = NULL; }
    PALETTEENTRY e[256];
    pal_entries(e, seed);
    HRESULT hr = IDirectDraw7_CreatePalette(dd7, f, e, &dpal[p], NULL);
    if (hr) { out("ddpal hr %08lx\n", hr); dpal[p] = NULL; }
}

static IDirectDrawPalette *pal_at(int i) {
    int p = pslot(i);
    if (!bad && !dpal[p]) skip("nopal");
    return bad ? NULL : dpal[p];
}

/* ddpalget <p> <start> <n>: GetEntries, 4 bytes an entry (r g b flags) */
static void op_ddpalget(void) {
    IDirectDrawPalette *p = pal_at(1);
    long st = num(2), n = num(3);
    if (bad || n < 0 || n > 256) { bad = 1; return; }
    if (gone()) return;
    PALETTEENTRY e[257];
    memset(e, 0xcc, sizeof e);
    HRESULT hr = IDirectDrawPalette_GetEntries(p, 0, st, n, e);
    if (hr) { out("palget hr %08lx\n", hr); return; }
    out("pal ");
    out_hex((unsigned char *)e, n * 4);
}

/* ddpalset <p> <start> <n> <seed>: SetEntries */
static void op_ddpalset(void) {
    IDirectDrawPalette *p = pal_at(1);
    long st = num(2), n = num(3);
    unsigned seed = (unsigned)num(4);
    if (bad || n < 0 || n > 256) { bad = 1; return; }
    if (gone()) return;
    PALETTEENTRY e[256];
    pal_entries(e, seed);
    HRESULT hr = IDirectDrawPalette_SetEntries(p, 0, st, n, e);
    if (hr) out("palset hr %08lx\n", hr);
}

/* ddpalcaps <p>: GetCaps */
static void op_ddpalcaps(void) {
    IDirectDrawPalette *p = pal_at(1);
    if (gone()) return;
    DWORD c = 0;
    HRESULT hr = IDirectDrawPalette_GetCaps(p, &c);
    if (hr) out("palcaps hr %08lx\n", hr);
    else out("palcaps %08lx\n", c);
}

/* ddsetpal <s> <p>|none: SetPalette; ddgetpal <s>: GetPalette ("getpal <p>" / "getpal hr <hr>") */
static void op_ddsetpal(void) {
    IDirectDrawSurface7 *s = dslot(1);
    IDirectDrawPalette *p = is(2, "none") ? NULL : pal_at(2);
    if (gone()) return;
    HRESULT hr = IDirectDrawSurface7_SetPalette(s, p);
    if (hr) out("setpal hr %08lx\n", hr);
}
static void op_ddgetpal(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    IDirectDrawPalette *p = NULL;
    HRESULT hr = IDirectDrawSurface7_GetPalette(s, &p);
    if (hr) { out("getpal hr %08lx\n", hr); return; }
    int k = 0;
    while (k < 4 && dpal[k] != p) k++;
    if (k < 4) out("getpal %d\n", k);
    else out("getpal other\n");
    IDirectDrawPalette_Release(p);
}

/* ------------------------------------------------------------------------------------------ */
/* GetDC: GDI on a surface */

/* ddgetdc <s> / ddreldc <s>: GetDC, ReleaseDC */
static void op_ddgetdc(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    int i = (int)num(1);
    HRESULT hr;
    if (is(0, "ddgetdc")) {
        HDC dc = NULL;
        hr = IDirectDrawSurface7_GetDC(s, &dc);
        if (hr) out("getdc hr %08lx\n", hr);
        else if (ddc[i]) { out("getdc again\n"); IDirectDrawSurface7_ReleaseDC(s, dc); }
        else ddc[i] = dc;
    } else {
        hr = IDirectDrawSurface7_ReleaseDC(s, ddc[i]);
        if (hr) out("reldc hr %08lx\n", hr);
        ddc[i] = NULL;
    }
}

static HDC dcat(int i) {
    dslot(i);
    if (gone()) return NULL;
    HDC dc = ddc[num(i)];
    if (!dc) skip("nodc");
    return dc;
}

/* dcpix <s> <x> <y> #c: SetPixel ("pixel RRGGBB" | "pixel invalid"); dcget <s> <x> <y>: GetPixel;
 * dcrect <s> <x0> <y0> <x1> <y1> #c: FillRect with a solid brush; dcbpp <s>: GetDeviceCaps
 * BITSPIXEL and the bitmap's bmBitsPixel */
static void op_dcpix(void) {
    HDC dc = dcat(1);
    if (is(0, "dcbpp")) {
        if (gone()) return;
        BITMAP bm = { 0 };
        HGDIOBJ b = GetCurrentObject(dc, OBJ_BITMAP);
        int got = b ? GetObjectA(b, sizeof bm, &bm) : 0;
        out("dcbpp %d %d %d %d\n", GetDeviceCaps(dc, BITSPIXEL), got ? bm.bmBitsPixel : -1, got ? bm.bmWidth : -1, got ? bm.bmHeight : -1);
        return;
    }
    if (is(0, "dcrect")) {
        RECT r = { num(2), num(3), num(4), num(5) };
        COLORREF c = color(6);
        if (gone()) return;
        HBRUSH br = CreateSolidBrush(c);
        out("ret %d\n", FillRect(dc, &r, br) != 0);
        DeleteObject(br);
        return;
    }
    long x = num(2), y = num(3);
    COLORREF c = is(0, "dcpix") ? color(4) : 0;
    if (gone()) return;
    COLORREF r = is(0, "dcpix") ? SetPixel(dc, x, y, c) : GetPixel(dc, x, y);
    if (r == CLR_INVALID) out("pixel invalid\n");
    else out("pixel %02x%02x%02x\n", GetRValue(r), GetGValue(r), GetBValue(r));
}

/* ------------------------------------------------------------------------------------------ */
/* Lock */

/* ddlock <s> <rect> [readonly writeonly nosyslock nowait]: Lock ("lock <pitch> <offset>" from
 * the surface's first byte, "lock hr <hr>"); ddpoke <s> <x> <y> <hex>: bytes at byte x of row y
 * of the locked rectangle; ddunlock <s> <rect>|none: Unlock ("unlock <hr>"; none: never locked
 * pointer NULL) */
static void op_ddlk(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    int i = (int)num(1);
    RECT r, *pr;
    if (is(0, "ddlock")) {
        int k = ddrect(2, &r, &pr);
        DWORD f = DDLOCK_WAIT;
        for (; k < ntok; k++)
            if (is(k, "readonly")) f |= DDLOCK_READONLY;
            else if (is(k, "writeonly")) f |= DDLOCK_WRITEONLY;
            else if (is(k, "nosyslock")) f |= DDLOCK_NOSYSLOCK;
            else if (is(k, "nowait")) f &= ~DDLOCK_WAIT;
            else bad = 1;
        if (gone()) return;
        DDSURFACEDESC2 d = { sizeof d };
        HRESULT hr = IDirectDrawSurface7_Lock(s, pr, &d, f, NULL);
        if (hr) { out("lock hr %08lx\n", hr); return; }
        if (!dlock[i]) { dlock[i] = d.lpSurface; dpitch[i] = d.lPitch; }
        out("lock %ld %ld\n", d.lPitch, dbase[i] ? (long)((unsigned char *)d.lpSurface - dbase[i]) : -1L);
    } else if (is(0, "ddpoke")) {
        long x = num(2), y = num(3);
        if (bad || ntok != 5 || strlen(tok[4]) % 2 || x < 0 || y < 0) { bad = 1; return; }
        if (gone()) return;
        if (!dlock[i]) { out("nolock\n"); return; }
        for (int k = 0; tok[4][2 * k]; k++) {
            char h[3] = { tok[4][2 * k], tok[4][2 * k + 1], 0 };
            dlock[i][y * dpitch[i] + x + k] = (unsigned char)strtoul(h, NULL, 16);
        }
    } else {
        if (is(2, "none")) pr = NULL;
        else ddrect(2, &r, &pr);
        if (gone()) return;
        HRESULT hr = IDirectDrawSurface7_Unlock(s, pr);
        out("unlock %08lx\n", hr);
        if (!hr) dlock[i] = NULL;
    }
}

/* dddesc <s>: GetSurfaceDesc: "desc <flags> <w> <h> <pitch> <caps> <pf flags> <bits> <r g b a masks>" */
static void op_dddesc(void) {
    IDirectDrawSurface7 *s = dslot(1);
    if (gone()) return;
    DDSURFACEDESC2 d = { sizeof d };
    HRESULT hr = IDirectDrawSurface7_GetSurfaceDesc(s, &d);
    if (hr) { out("desc hr %08lx\n", hr); return; }
    DDPIXELFORMAT *f = &d.ddpfPixelFormat;
    out("desc %08lx %lu %lu %ld %08lx %08lx %lu %08lx %08lx %08lx %08lx\n", d.dwFlags, d.dwWidth, d.dwHeight, d.lPitch, d.ddsCaps.dwCaps,
        f->dwFlags, f->dwRGBBitCount, f->dwRBitMask, f->dwGBitMask, f->dwBBitMask, f->dwRGBAlphaBitMask);
}

/* ------------------------------------------------------------------------------------------ */
/* cooperative levels, flipping chains */

/* ddcoop normal|exclusive: SetCooperativeLevel (exclusive: DDSCL_EXCLUSIVE | DDSCL_FULLSCREEN) */
static void op_ddcoop(void) {
    DWORD f = is(1, "normal") ? DDSCL_NORMAL : is(1, "exclusive") ? DDSCL_EXCLUSIVE | DDSCL_FULLSCREEN : 0;
    if (!f) { bad = 1; return; }
    if (!dd7) { out("nodev\n"); return; }
    HRESULT hr = IDirectDraw7_SetCooperativeLevel(dd7, hwnd, f);
    out("coop %08lx\n", hr);
    if (!hr) dexcl = f != DDSCL_NORMAL;
}

/* ddchain <s> <backs> <w> <h> <caps>...: CreateSurface DDSCAPS_FLIP | COMPLEX (backs > 0) with
 * caps primary offscreen texture 3ddevice sysmem vidmem (primary: no size, no pixel format;
 * else 32 bpp RGB), then slots s, s+1, ... hold the chain's surfaces in GetAttachedSurface
 * (DDSCAPS_FLIP) order ("chain <n>", "chain hr <hr>") */
static void op_ddchain(void) {
    long s = num(1), backs = num(2), w = num(3), h = num(4);
    if (bad || s < 0 || s >= SLOTS || backs < 0 || backs > 6 || w < 1 || h < 1 || w > 256 || h > 256) { bad = 1; return; }
    DDSURFACEDESC2 d = { sizeof d };
    d.dwFlags = DDSD_CAPS;
    int primary = 0;
    for (int i = 5; i < ntok; i++)
        if (is(i, "primary")) { d.ddsCaps.dwCaps |= DDSCAPS_PRIMARYSURFACE; primary = 1; }
        else if (is(i, "offscreen")) d.ddsCaps.dwCaps |= DDSCAPS_OFFSCREENPLAIN;
        else if (is(i, "texture")) d.ddsCaps.dwCaps |= DDSCAPS_TEXTURE;
        else if (is(i, "3ddevice")) d.ddsCaps.dwCaps |= DDSCAPS_3DDEVICE;
        else if (is(i, "sysmem")) d.ddsCaps.dwCaps |= DDSCAPS_SYSTEMMEMORY;
        else if (is(i, "vidmem")) d.ddsCaps.dwCaps |= DDSCAPS_VIDEOMEMORY;
        else bad = 1;
    if (gone()) return;
    if (!dd7) { out("nodev\n"); return; }
    if (backs) { d.ddsCaps.dwCaps |= DDSCAPS_FLIP | DDSCAPS_COMPLEX; d.dwFlags |= DDSD_BACKBUFFERCOUNT; d.dwBackBufferCount = backs; }
    if (!primary) {
        d.dwFlags |= DDSD_WIDTH | DDSD_HEIGHT | DDSD_PIXELFORMAT;
        d.dwWidth = w; d.dwHeight = h;
        d.ddpfPixelFormat.dwSize = sizeof d.ddpfPixelFormat;
        d.ddpfPixelFormat.dwFlags = DDPF_RGB;
        d.ddpfPixelFormat.dwRGBBitCount = 32; d.ddpfPixelFormat.dwRBitMask = 0xff0000; d.ddpfPixelFormat.dwGBitMask = 0xff00; d.ddpfPixelFormat.dwBBitMask = 0xff;
    }
    for (int i = s + backs < SLOTS ? s + backs : SLOTS - 1; i >= s; i--) ddfree(i);
    IDirectDrawSurface7 *f;
    HRESULT hr = IDirectDraw7_CreateSurface(dd7, &d, &f, NULL);
    if (hr) { out("chain hr %08lx\n", hr); return; }
    dds[s] = f;
    int n = 1;
    IDirectDrawSurface7 *cur = f;
    while (backs && s + n < SLOTS) {
        DDSCAPS2 c = { DDSCAPS_FLIP };
        IDirectDrawSurface7 *next = NULL;
        if (IDirectDrawSurface7_GetAttachedSurface(cur, &c, &next) || !next) break;
        if (next == f) { IDirectDrawSurface7_Release(next); break; }
        droot[s + n] = s + 1;
        dds[s + n++] = next;
        cur = next;
    }
    for (int i = s; i < s + n; i++) if (!primary || i > s) ddbase(i);
    out("chain %d\n", n);
}

/* ddflip <s> <target slot>|- [novsync nowait odd even interval2]: Flip ("flip <hr>") */
static void op_ddflip(void) {
    IDirectDrawSurface7 *s = dslot(1), *t = NULL;
    if (!is(2, "-")) t = dslot(2);
    DWORD f = DDFLIP_WAIT;
    for (int i = 3; i < ntok; i++)
        if (is(i, "novsync")) f |= DDFLIP_NOVSYNC;
        else if (is(i, "nowait")) f &= ~DDFLIP_WAIT;
        else if (is(i, "odd")) f |= DDFLIP_ODD;
        else if (is(i, "even")) f |= DDFLIP_EVEN;
        else if (is(i, "interval2")) f |= DDFLIP_INTERVAL2;
        else bad = 1;
    if (gone()) return;
    out("flip %08lx\n", IDirectDrawSurface7_Flip(s, t, f));
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
    { "ddclip", op_ddclip },
    { "ddcliplist", op_ddcliplist },
    { "ddpal", op_ddpal },
    { "ddpalget", op_ddpalget },
    { "ddpalset", op_ddpalset },
    { "ddpalcaps", op_ddpalcaps },
    { "ddsetpal", op_ddsetpal },
    { "ddgetpal", op_ddgetpal },
    { "ddgetdc", op_ddgetdc },
    { "ddreldc", op_ddgetdc },
    { "dcpix", op_dcpix },
    { "dcget", op_dcpix },
    { "dcrect", op_dcpix },
    { "dcbpp", op_dcpix },
    { "ddlock", op_ddlk },
    { "ddpoke", op_ddlk },
    { "ddunlock", op_ddlk },
    { "dddesc", op_dddesc },
    { "ddcoop", op_ddcoop },
    { "ddchain", op_ddchain },
    { "ddflip", op_ddflip },
    { 0 },
};
const struct kz_api ddraw_api = { "ddraw", ddraw_ops, ddraw_reset };
