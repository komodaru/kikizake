#include "core.h"
#include <d3d10_1.h>
#include <d3d11.h>
#include <float.h>
/* Direct3D 10, 10.1 and 11 resources: format support, textures, buffers, views, clears,
 * updates, copies and reading back (SPEC.md "Direct3D 10 and 11 resources"). The lines act on
 * whichever device the case made: device10 / device10_1 (here) or device11 (runner/d3d11.c). */

extern ID3D11Device *dev11;
static ID3D10Device *dev10;

#define SLOTS 16
static IUnknown *res[SLOTS], *view[SLOTS];
static int resdim[SLOTS]; /* 1, 2, 3: textures; 4: buffer */
static int viewkind[SLOTS]; /* 0 srv, 1 rtv, 2 dsv, 3 uav */

static void draw_reset(void);
static void d3dres_reset(void) {
    draw_reset();
    for (int i = 0; i < SLOTS; i++) {
        if (view[i]) IUnknown_Release(view[i]);
        if (res[i]) IUnknown_Release(res[i]);
        view[i] = res[i] = NULL;
    }
    if (dev10) { ID3D10Device_Release(dev10); dev10 = NULL; }
}

static ID3D11DeviceContext *ctx11(void) {
    ID3D11DeviceContext *c;
    ID3D11Device_GetImmediateContext(dev11, &c);
    ID3D11DeviceContext_Release(c); /* (the device keeps it) */
    return c;
}

static int nodev(void) {
    if (dev10 || dev11) return 0;
    out("nodev\n");
    return 1;
}

/* device10: D3D10CreateDevice (d3d10.dll); device10_1 <feature level>: D3D10CreateDevice1
 * (d3d10_1.dll); hardware, else WARP */
static void op_device10(void) {
    d3dres_reset();
    int one = is(0, "device10_1");
    D3D10_FEATURE_LEVEL1 fl = one ? (D3D10_FEATURE_LEVEL1)num(1) : 0;
    if (bad) return;
    static HRESULT (WINAPI *c10)(IDXGIAdapter *, D3D10_DRIVER_TYPE, HMODULE, UINT, UINT, ID3D10Device **);
    static HRESULT (WINAPI *c101)(IDXGIAdapter *, D3D10_DRIVER_TYPE, HMODULE, UINT, D3D10_FEATURE_LEVEL1, UINT, ID3D10Device1 **);
    if (!one && !c10) {
        HMODULE m = LoadLibraryA("d3d10.dll");
        if (m) c10 = (void *)GetProcAddress(m, "D3D10CreateDevice");
    }
    if (one && !c101) {
        HMODULE m = LoadLibraryA("d3d10_1.dll");
        if (m) c101 = (void *)GetProcAddress(m, "D3D10CreateDevice1");
    }
    if (one ? !c101 : !c10) { out("%s none\n", tok[0]); return; }
    HRESULT hr = E_FAIL;
    D3D10_DRIVER_TYPE types[] = { D3D10_DRIVER_TYPE_HARDWARE, D3D10_DRIVER_TYPE_WARP };
    for (int i = 0; i < 2 && FAILED(hr); i++)
        hr = one ? c101(NULL, types[i], NULL, 0, fl, D3D10_1_SDK_VERSION, (ID3D10Device1 **)&dev10)
                 : c10(NULL, types[i], NULL, 0, D3D10_SDK_VERSION, &dev10);
    if (FAILED(hr)) { out("%s failed %08lx\n", tok[0], hr); dev10 = NULL; return; }
    out("%s ok\n", tok[0]);
}

/* fmt <format>: CheckFormatSupport, FORMAT_SUPPORT2 (11 only) and the quality levels of 2, 4, 8
 * and 16 samples */
static void op_fmt(void) {
    UINT f = (UINT)num(1), s = 0, q;
    if (bad || nodev()) return;
    HRESULT hr;
    if (dev11) {
        hr = ID3D11Device_CheckFormatSupport(dev11, f, &s);
        D3D11_FEATURE_DATA_FORMAT_SUPPORT2 s2 = { f, 0 };
        HRESULT hr2 = ID3D11Device_CheckFeatureSupport(dev11, D3D11_FEATURE_FORMAT_SUPPORT2, &s2, sizeof s2);
        out("fmt %u %08lx %x %08lx %x ms", f, hr, s, hr2, s2.OutFormatSupport2);
    } else {
        hr = ID3D10Device_CheckFormatSupport(dev10, f, &s);
        out("fmt %u %08lx %x - - ms", f, hr, s);
    }
    for (UINT c = 2; c <= 16; c *= 2) {
        q = 0xdead;
        hr = dev11 ? ID3D11Device_CheckMultisampleQualityLevels(dev11, f, c, &q) : ID3D10Device_CheckMultisampleQualityLevels(dev10, f, c, &q);
        if (FAILED(hr)) out(" x%lx", hr);
        else out(" %u", q);
    }
    out("\n");
}

/* initial data: 4 MiB from xorshift32, every subresource reading it from its start */
#define INIT (4 << 20)
static unsigned char *initbuf;
static void fillinit(unsigned seed) {
    if (!initbuf) initbuf = VirtualAlloc(NULL, INIT, MEM_COMMIT, PAGE_READWRITE);
    unsigned x = seed ? seed : 0x9e3779b9;
    for (int i = 0; i < INIT; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        initbuf[i] = (unsigned char)x;
    }
}
#define MAXSUB 4096
static D3D11_SUBRESOURCE_DATA subs[MAXSUB]; /* (the same layout as D3D10_SUBRESOURCE_DATA) */

static int slot(int i, int max) {
    int s = (int)num(i);
    if (s < 0 || s >= max) bad = 1;
    return bad ? 0 : s;
}

/* tex <slot|check> <dim> <w> <h> <depth|array> <mips> <format> <usage> <bind> <cpu> <misc>
 *   <samples> <quality> [init <seed> <row pitch> <slice pitch>] */
static void op_tex(void) {
    int check = is(1, "check"), s = check ? 0 : slot(1, SLOTS);
    UINT v[12];
    for (int i = 0; i < 12; i++) v[i] = (UINT)num(2 + i);
    int init = ntok > 14;
    if (init && (ntok != 18 || !is(14, "init"))) bad = 1;
    if (!init && ntok != 14) bad = 1;
    if (bad || nodev()) return;
    UINT dim = v[0], w = v[1], h = v[2], da = v[3], mips = v[4];
    if (dim < 1 || dim > 3) { bad = 1; return; }
    if (!check && res[s]) { IUnknown_Release(res[s]); res[s] = NULL; }
    void *pd = NULL;
    if (init) {
        fillinit((unsigned)num(15));
        UINT n = mips;
        if (!n) { UINT m = w > h ? w : h; if (dim == 3 && da > m) m = da; for (n = 1; m > 1; m >>= 1) n++; }
        n *= dim == 3 ? 1 : da;
        if (n > MAXSUB || n == 0) n = MAXSUB; /* (a description Windows refuses anyway) */
        for (UINT i = 0; i < n; i++) {
            subs[i].pSysMem = initbuf;
            subs[i].SysMemPitch = (UINT)num(16);
            subs[i].SysMemSlicePitch = (UINT)num(17);
        }
        pd = subs;
    }
    HRESULT hr;
    UINT got[11];
    memset(got, 0xcc, sizeof got);
    int words = dim == 1 ? 8 : dim == 2 ? 11 : 9;
    IUnknown *o = NULL;
    void **po = check ? NULL : (void **)&o;
    if (dim == 1) {
        D3D11_TEXTURE1D_DESC d = { w, mips, da, v[5], v[6], v[7], v[8], v[9] };
        hr = dev11 ? ID3D11Device_CreateTexture1D(dev11, &d, pd, (ID3D11Texture1D **)po) : ID3D10Device_CreateTexture1D(dev10, (void *)&d, pd, (ID3D10Texture1D **)po);
        if (o) { if (dev11) ID3D11Texture1D_GetDesc((ID3D11Texture1D *)o, (void *)got); else ID3D10Texture1D_GetDesc((ID3D10Texture1D *)o, (void *)got); }
    } else if (dim == 2) {
        D3D11_TEXTURE2D_DESC d = { w, h, mips, da, v[5], { v[10], v[11] }, v[6], v[7], v[8], v[9] };
        hr = dev11 ? ID3D11Device_CreateTexture2D(dev11, &d, pd, (ID3D11Texture2D **)po) : ID3D10Device_CreateTexture2D(dev10, (void *)&d, pd, (ID3D10Texture2D **)po);
        if (o) { if (dev11) ID3D11Texture2D_GetDesc((ID3D11Texture2D *)o, (void *)got); else ID3D10Texture2D_GetDesc((ID3D10Texture2D *)o, (void *)got); }
    } else {
        D3D11_TEXTURE3D_DESC d = { w, h, da, mips, v[5], v[6], v[7], v[8], v[9] };
        hr = dev11 ? ID3D11Device_CreateTexture3D(dev11, &d, pd, (ID3D11Texture3D **)po) : ID3D10Device_CreateTexture3D(dev10, (void *)&d, pd, (ID3D10Texture3D **)po);
        if (o) { if (dev11) ID3D11Texture3D_GetDesc((ID3D11Texture3D *)o, (void *)got); else ID3D10Texture3D_GetDesc((ID3D10Texture3D *)o, (void *)got); }
    }
    if (!o) { out("tex hr %08lx\n", hr); return; }
    res[s] = o;
    resdim[s] = dim;
    out("tex =");
    for (int i = 0; i < words; i++) out(" %x", got[i]);
    out("\n");
}

/* buf <slot|check> <bytes> <usage> <bind> <cpu> <misc> <stride> [init <seed>] */
static void op_buf(void) {
    int check = is(1, "check"), s = check ? 0 : slot(1, SLOTS);
    UINT v[6];
    for (int i = 0; i < 6; i++) v[i] = (UINT)num(2 + i);
    int init = ntok > 8;
    if (init && (ntok != 10 || !is(8, "init"))) bad = 1;
    if (!init && ntok != 8) bad = 1;
    if (bad || nodev()) return;
    if (!check && res[s]) { IUnknown_Release(res[s]); res[s] = NULL; }
    D3D11_SUBRESOURCE_DATA sd = { 0 };
    if (init) { fillinit((unsigned)num(9)); sd.pSysMem = initbuf; }
    IUnknown *o = NULL;
    void **po = check ? NULL : (void **)&o;
    UINT got[6];
    memset(got, 0xcc, sizeof got);
    HRESULT hr;
    if (dev11) {
        D3D11_BUFFER_DESC d = { v[0], v[1], v[2], v[3], v[4], v[5] };
        hr = ID3D11Device_CreateBuffer(dev11, &d, init ? &sd : NULL, (ID3D11Buffer **)po);
        if (o) ID3D11Buffer_GetDesc((ID3D11Buffer *)o, (void *)got);
    } else {
        D3D10_BUFFER_DESC d = { v[0], v[1], v[2], v[3], v[4] };
        hr = ID3D10Device_CreateBuffer(dev10, &d, init ? (void *)&sd : NULL, (ID3D10Buffer **)po);
        if (o) ID3D10Buffer_GetDesc((ID3D10Buffer *)o, (void *)got);
    }
    if (!o) { out("buf hr %08lx\n", hr); return; }
    res[s] = o;
    resdim[s] = 4;
    out("buf =");
    for (int i = 0; i < (dev11 ? 6 : 5); i++) out(" %x", got[i]);
    out("\n");
}

/* the words of a view description after Format and ViewDimension that the dimension uses
 * (11: srv 0..11 (11 BUFFEREX), rtv 0..8, dsv 0..6, uav 0..8; 10: srv 0..10 (10.1: 10 cube
 * array), rtv, dsv) */
static int union_words(int kind, int dim) {
    switch (kind) {
    case 0: { static const int n[] = { 0, 2, 2, 4, 2, 4, 0, 2, 2, 2, 4, 3 }; return dim < 12 ? n[dim] : 0; }
    case 1: { static const int n[] = { 0, 2, 1, 3, 1, 3, 0, 2, 3 }; return dim < 9 ? n[dim] : 0; }
    case 2: { static const int n[] = { 0, 1, 3, 1, 3, 0, 2 }; return dim < 7 ? n[dim] : 0; }
    default: { static const int n[] = { 0, 3, 1, 3, 1, 3, 0, 0, 3 }; return dim < 9 ? n[dim] : 0; }
    }
}

/* srv|rtv|dsv|uav <view slot> <resource slot> [check] nodesc | <description words>: Create...View,
 * then GetDesc (Format, ViewDimension, [Flags (dsv, 11)], the union words the dimension uses) */
static void op_view(void) {
    static const char *kinds[] = { "srv", "rtv", "dsv", "uav" };
    int k = 0;
    while (!is(0, kinds[k])) k++;
    int v = slot(1, SLOTS), r = slot(2, SLOTS);
    int check = is(3, "check"), at = 3 + check, nodesc = is(at, "nodesc");
    UINT d[8] = { 0 };
    if (nodesc) { if (ntok != at + 1) bad = 1; }
    else {
        if (ntok - at > 8) bad = 1;
        for (int i = at; i < ntok; i++) d[i - at] = (UINT)num(i);
    }
    if (bad || nodev()) return;
    if (!res[r]) { out("%s nores\n", kinds[k]); return; }
    if (view[v] && !check) { IUnknown_Release(view[v]); view[v] = NULL; }
    IUnknown *o = NULL;
    void **po = check ? NULL : (void **)&o, *pd = nodesc ? NULL : d;
    UINT got[8];
    memset(got, 0xcc, sizeof got);
    HRESULT hr;
    if (dev11) {
        ID3D11Resource *rs = (ID3D11Resource *)res[r];
        switch (k) {
        case 0: hr = ID3D11Device_CreateShaderResourceView(dev11, rs, pd, (ID3D11ShaderResourceView **)po); if (o) ID3D11ShaderResourceView_GetDesc((ID3D11ShaderResourceView *)o, (void *)got); break;
        case 1: hr = ID3D11Device_CreateRenderTargetView(dev11, rs, pd, (ID3D11RenderTargetView **)po); if (o) ID3D11RenderTargetView_GetDesc((ID3D11RenderTargetView *)o, (void *)got); break;
        case 2: hr = ID3D11Device_CreateDepthStencilView(dev11, rs, pd, (ID3D11DepthStencilView **)po); if (o) ID3D11DepthStencilView_GetDesc((ID3D11DepthStencilView *)o, (void *)got); break;
        default: hr = ID3D11Device_CreateUnorderedAccessView(dev11, rs, pd, (ID3D11UnorderedAccessView **)po); if (o) ID3D11UnorderedAccessView_GetDesc((ID3D11UnorderedAccessView *)o, (void *)got); break;
        }
    } else {
        ID3D10Resource *rs = (ID3D10Resource *)res[r];
        switch (k) {
        case 0:
            /* (10.1 devices: the 10.1 description, which adds the cube array) */
            {
                ID3D10Device1 *d1 = NULL;
                if (SUCCEEDED(ID3D10Device_QueryInterface(dev10, &IID_ID3D10Device1, (void **)&d1))) {
                    hr = ID3D10Device1_CreateShaderResourceView1(d1, rs, pd, (ID3D10ShaderResourceView1 **)po);
                    if (o) ID3D10ShaderResourceView1_GetDesc1((ID3D10ShaderResourceView1 *)o, (void *)got);
                    ID3D10Device1_Release(d1);
                } else {
                    hr = ID3D10Device_CreateShaderResourceView(dev10, rs, pd, (ID3D10ShaderResourceView **)po);
                    if (o) ID3D10ShaderResourceView_GetDesc((ID3D10ShaderResourceView *)o, (void *)got);
                }
            }
            break;
        case 1: hr = ID3D10Device_CreateRenderTargetView(dev10, rs, pd, (ID3D10RenderTargetView **)po); if (o) ID3D10RenderTargetView_GetDesc((ID3D10RenderTargetView *)o, (void *)got); break;
        case 2: hr = ID3D10Device_CreateDepthStencilView(dev10, rs, pd, (ID3D10DepthStencilView **)po); if (o) ID3D10DepthStencilView_GetDesc((ID3D10DepthStencilView *)o, (void *)got); break;
        default: out("uav none\n"); return;
        }
    }
    if (!o) { out("%s hr %08lx\n", kinds[k], hr); return; }
    view[v] = o;
    viewkind[v] = k;
    int head = k == 2 && dev11 ? 3 : 2;
    out("%s =", kinds[k]);
    for (int i = 0; i < head + union_words(k, got[1]) && i < 8; i++) out(" %x", got[i]);
    out("\n");
}

static float fl(int i) {
    UINT u = (UINT)num(i);
    float f;
    memcpy(&f, &u, 4);
    return f;
}

/* clear <rtv> <r> <g> <b> <a> (float bits): ClearRenderTargetView
 * cleards <dsv> <flags> <depth bits> <stencil>: ClearDepthStencilView
 * clearuav <uav> f|u <4 words>: ClearUnorderedAccessViewFloat / Uint (11) */
static void op_clear(void) {
    int v = slot(1, SLOTS);
    if (bad || nodev()) return;
    if (is(0, "clear")) {
        float c[4];
        for (int i = 0; i < 4; i++) c[i] = fl(2 + i);
        if (ntok != 6 || bad) { bad = 1; return; }
        if (!view[v] || viewkind[v] != 1) { out("clear noview\n"); return; }
        if (dev11) ID3D11DeviceContext_ClearRenderTargetView(ctx11(), (void *)view[v], c);
        else ID3D10Device_ClearRenderTargetView(dev10, (void *)view[v], c);
    } else if (is(0, "cleards")) {
        UINT flags = (UINT)num(2), st = (UINT)num(4);
        float dep = fl(3);
        if (ntok != 5 || bad) { bad = 1; return; }
        if (!view[v] || viewkind[v] != 2) { out("cleards noview\n"); return; }
        if (dev11) ID3D11DeviceContext_ClearDepthStencilView(ctx11(), (void *)view[v], flags, dep, (UINT8)st);
        else ID3D10Device_ClearDepthStencilView(dev10, (void *)view[v], flags, dep, (UINT8)st);
    } else {
        UINT u[4];
        for (int i = 0; i < 4; i++) u[i] = (UINT)num(3 + i);
        if (ntok != 7 || bad || !(is(2, "f") || is(2, "u"))) { bad = 1; return; }
        if (!dev11) { out("clearuav none\n"); return; }
        if (!view[v] || viewkind[v] != 3) { out("clearuav noview\n"); return; }
        if (is(2, "u")) ID3D11DeviceContext_ClearUnorderedAccessViewUint(ctx11(), (void *)view[v], u);
        else ID3D11DeviceContext_ClearUnorderedAccessViewFloat(ctx11(), (void *)view[v], (float *)u);
    }
}

static int box(int at, D3D11_BOX *b) {
    if (ntok == at) return 0;
    if (ntok != at + 6) { bad = 1; return 0; }
    UINT *p = (UINT *)b;
    for (int i = 0; i < 6; i++) p[i] = (UINT)num(at + i);
    return 1;
}

/* update <res> <sub> <seed> <row pitch> <depth pitch> [<box: left top front right bottom back>]:
 *   UpdateSubresource from xorshift32 bytes
 * copy <dst> <src>: CopyResource
 * copysub <dst> <dst sub> <x> <y> <z> <src> <src sub> [<box>]: CopySubresourceRegion
 * resolve <dst> <dst sub> <src> <src sub> <format>: ResolveSubresource */
static void op_copy(void) {
    D3D11_BOX b;
    if (is(0, "update")) {
        int r = slot(1, SLOTS);
        UINT sub = (UINT)num(2), seed = (UINT)num(3), rp = (UINT)num(4), dp = (UINT)num(5);
        int hb = box(6, &b);
        if (bad || nodev()) return;
        if (!res[r]) { out("update nores\n"); return; }
        fillinit(seed);
        if (dev11) ID3D11DeviceContext_UpdateSubresource(ctx11(), (void *)res[r], sub, hb ? &b : NULL, initbuf, rp, dp);
        else ID3D10Device_UpdateSubresource(dev10, (void *)res[r], sub, hb ? (void *)&b : NULL, initbuf, rp, dp);
    } else if (is(0, "copy")) {
        int d = slot(1, SLOTS), s = slot(2, SLOTS);
        if (ntok != 3) bad = 1;
        if (bad || nodev()) return;
        if (!res[d] || !res[s]) { out("copy nores\n"); return; }
        if (dev11) ID3D11DeviceContext_CopyResource(ctx11(), (void *)res[d], (void *)res[s]);
        else ID3D10Device_CopyResource(dev10, (void *)res[d], (void *)res[s]);
    } else if (is(0, "copysub")) {
        int d = slot(1, SLOTS), s = slot(6, SLOTS);
        UINT ds = (UINT)num(2), x = (UINT)num(3), y = (UINT)num(4), z = (UINT)num(5), ss = (UINT)num(7);
        int hb = box(8, &b);
        if (bad || nodev()) return;
        if (!res[d] || !res[s]) { out("copysub nores\n"); return; }
        if (dev11) ID3D11DeviceContext_CopySubresourceRegion(ctx11(), (void *)res[d], ds, x, y, z, (void *)res[s], ss, hb ? &b : NULL);
        else ID3D10Device_CopySubresourceRegion(dev10, (void *)res[d], ds, x, y, z, (void *)res[s], ss, hb ? (void *)&b : NULL);
    } else {
        int d = slot(1, SLOTS), s = slot(3, SLOTS);
        UINT ds = (UINT)num(2), ss = (UINT)num(4), f = (UINT)num(5);
        if (ntok != 6) bad = 1;
        if (bad || nodev()) return;
        if (!res[d] || !res[s]) { out("resolve nores\n"); return; }
        if (dev11) ID3D11DeviceContext_ResolveSubresource(ctx11(), (void *)res[d], ds, (void *)res[s], ss, f);
        else ID3D10Device_ResolveSubresource(dev10, (void *)res[d], ds, (void *)res[s], ss, f);
    }
}

/* read <res> <sub> <row bytes> <rows> <slices>: the subresource copied into a staging resource of
 * its size (one subresource, CPU read), mapped; prints "read <rows> <slices>" and each row's first
 * <row bytes> bytes as hex, slice after slice ("read hr <hr>" if the staging copy fails) */
static void op_read(void) {
    int r = slot(1, SLOTS);
    UINT sub = (UINT)num(2), rb = (UINT)num(3), rows = (UINT)num(4), slices = (UINT)num(5);
    if (ntok != 6) bad = 1;
    if (bad || nodev()) return;
    if (!res[r]) { out("read nores\n"); return; }
    IUnknown *st = NULL;
    HRESULT hr;
    int dim = resdim[r];
    if (dev11) {
        if (dim == 4) {
            D3D11_BUFFER_DESC d;
            ID3D11Buffer_GetDesc((ID3D11Buffer *)res[r], &d);
            D3D11_BUFFER_DESC s = { d.ByteWidth, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ, 0, 0 };
            hr = ID3D11Device_CreateBuffer(dev11, &s, NULL, (ID3D11Buffer **)&st);
        } else if (dim == 1) {
            D3D11_TEXTURE1D_DESC d;
            ID3D11Texture1D_GetDesc((ID3D11Texture1D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D11_TEXTURE1D_DESC s = { max(d.Width >> m, 1), 1, 1, d.Format, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ, 0 };
            hr = ID3D11Device_CreateTexture1D(dev11, &s, NULL, (ID3D11Texture1D **)&st);
        } else if (dim == 2) {
            D3D11_TEXTURE2D_DESC d;
            ID3D11Texture2D_GetDesc((ID3D11Texture2D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D11_TEXTURE2D_DESC s = { max(d.Width >> m, 1), max(d.Height >> m, 1), 1, 1, d.Format, { 1, 0 }, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ, 0 };
            hr = ID3D11Device_CreateTexture2D(dev11, &s, NULL, (ID3D11Texture2D **)&st);
        } else {
            D3D11_TEXTURE3D_DESC d;
            ID3D11Texture3D_GetDesc((ID3D11Texture3D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D11_TEXTURE3D_DESC s = { max(d.Width >> m, 1), max(d.Height >> m, 1), max(d.Depth >> m, 1), 1, d.Format, D3D11_USAGE_STAGING, 0, D3D11_CPU_ACCESS_READ, 0 };
            hr = ID3D11Device_CreateTexture3D(dev11, &s, NULL, (ID3D11Texture3D **)&st);
        }
        if (FAILED(hr)) { out("read hr %08lx\n", hr); return; }
        ID3D11DeviceContext *c = ctx11();
        ID3D11DeviceContext_CopySubresourceRegion(c, (void *)st, 0, 0, 0, 0, (void *)res[r], sub, NULL);
        D3D11_MAPPED_SUBRESOURCE m;
        hr = ID3D11DeviceContext_Map(c, (void *)st, 0, D3D11_MAP_READ, 0, &m);
        if (FAILED(hr)) { out("read map hr %08lx\n", hr); IUnknown_Release(st); return; }
        out("read %u %u\n", rows, slices);
        for (UINT z = 0; z < slices; z++)
            for (UINT y = 0; y < rows; y++) out_hex((unsigned char *)m.pData + z * m.DepthPitch + y * m.RowPitch, rb);
        ID3D11DeviceContext_Unmap(c, (void *)st, 0);
    } else {
        if (dim == 4) {
            D3D10_BUFFER_DESC d;
            ID3D10Buffer_GetDesc((ID3D10Buffer *)res[r], &d);
            D3D10_BUFFER_DESC s = { d.ByteWidth, D3D10_USAGE_STAGING, 0, D3D10_CPU_ACCESS_READ, 0 };
            hr = ID3D10Device_CreateBuffer(dev10, &s, NULL, (ID3D10Buffer **)&st);
        } else if (dim == 1) {
            D3D10_TEXTURE1D_DESC d;
            ID3D10Texture1D_GetDesc((ID3D10Texture1D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D10_TEXTURE1D_DESC s = { max(d.Width >> m, 1), 1, 1, d.Format, D3D10_USAGE_STAGING, 0, D3D10_CPU_ACCESS_READ, 0 };
            hr = ID3D10Device_CreateTexture1D(dev10, &s, NULL, (ID3D10Texture1D **)&st);
        } else if (dim == 2) {
            D3D10_TEXTURE2D_DESC d;
            ID3D10Texture2D_GetDesc((ID3D10Texture2D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D10_TEXTURE2D_DESC s = { max(d.Width >> m, 1), max(d.Height >> m, 1), 1, 1, d.Format, { 1, 0 }, D3D10_USAGE_STAGING, 0, D3D10_CPU_ACCESS_READ, 0 };
            hr = ID3D10Device_CreateTexture2D(dev10, &s, NULL, (ID3D10Texture2D **)&st);
        } else {
            D3D10_TEXTURE3D_DESC d;
            ID3D10Texture3D_GetDesc((ID3D10Texture3D *)res[r], &d);
            UINT m = sub % d.MipLevels;
            D3D10_TEXTURE3D_DESC s = { max(d.Width >> m, 1), max(d.Height >> m, 1), max(d.Depth >> m, 1), 1, d.Format, D3D10_USAGE_STAGING, 0, D3D10_CPU_ACCESS_READ, 0 };
            hr = ID3D10Device_CreateTexture3D(dev10, &s, NULL, (ID3D10Texture3D **)&st);
        }
        if (FAILED(hr)) { out("read hr %08lx\n", hr); return; }
        ID3D10Device_CopySubresourceRegion(dev10, (void *)st, 0, 0, 0, 0, (void *)res[r], sub, NULL);
        void *p = NULL;
        UINT rp = 0, dp = 0;
        if (dim == 4) hr = ID3D10Buffer_Map((ID3D10Buffer *)st, D3D10_MAP_READ, 0, &p);
        else if (dim == 1) hr = ID3D10Texture1D_Map((ID3D10Texture1D *)st, 0, D3D10_MAP_READ, 0, &p);
        else if (dim == 2) {
            D3D10_MAPPED_TEXTURE2D m;
            hr = ID3D10Texture2D_Map((ID3D10Texture2D *)st, 0, D3D10_MAP_READ, 0, &m);
            p = m.pData, rp = m.RowPitch;
        } else {
            D3D10_MAPPED_TEXTURE3D m;
            hr = ID3D10Texture3D_Map((ID3D10Texture3D *)st, 0, D3D10_MAP_READ, 0, &m);
            p = m.pData, rp = m.RowPitch, dp = m.DepthPitch;
        }
        if (FAILED(hr)) { out("read map hr %08lx\n", hr); IUnknown_Release(st); return; }
        out("read %u %u\n", rows, slices);
        for (UINT z = 0; z < slices; z++)
            for (UINT y = 0; y < rows; y++) out_hex((unsigned char *)p + z * dp + y * rp, rb);
        if (dim == 4) ID3D10Buffer_Unmap((ID3D10Buffer *)st);
        else if (dim == 1) ID3D10Texture1D_Unmap((ID3D10Texture1D *)st, 0);
        else if (dim == 2) ID3D10Texture2D_Unmap((ID3D10Texture2D *)st, 0);
        else ID3D10Texture3D_Unmap((ID3D10Texture3D *)st, 0);
    }
    IUnknown_Release(st);
}


/* ------------------------------------------------------------------------------------------ */
/* drawing: shaders given as DXBC bytes, one vertex buffer, the output merger on the slots' views */

#define SHADERS 8
static unsigned char *blob[SHADERS];
static int bloblen[SHADERS], blobkind[SHADERS];
static IUnknown *shader[SHADERS], *layout, *states[8], *vbuf;
static int nstates;
#define VTX 4096
static UINT vtx[VTX];
static int nvtx;

static void draw_reset(void) {
    for (int i = 0; i < SHADERS; i++) {
        if (shader[i]) IUnknown_Release(shader[i]);
        shader[i] = NULL;
        free(blob[i]);
        blob[i] = NULL;
    }
    for (int i = 0; i < nstates; i++) IUnknown_Release(states[i]);
    nstates = 0;
    if (layout) IUnknown_Release(layout);
    if (vbuf) IUnknown_Release(vbuf);
    layout = vbuf = NULL;
    nvtx = 0;
}

static void keep(IUnknown *o) {
    if (nstates == 8) { IUnknown_Release(states[0]); memmove(states, states + 1, 7 * sizeof *states); nstates--; }
    states[nstates++] = o;
}

/* shader <slot> vs|ps <hex bytes>: CreateVertexShader / CreatePixelShader, then set */
static void op_shader(void) {
    int sl = slot(1, SHADERS), vs = is(2, "vs");
    if (ntok != 4 || !(vs || is(2, "ps"))) bad = 1;
    if (bad || nodev()) return;
    int n = (int)strlen(tok[3]) / 2;
    unsigned char *b = malloc(n);
    for (int i = 0; i < n; i++) { unsigned v; sscanf(tok[3] + 2 * i, "%2x", &v); b[i] = (unsigned char)v; }
    if (shader[sl]) IUnknown_Release(shader[sl]);
    free(blob[sl]);
    shader[sl] = NULL;
    blob[sl] = b, bloblen[sl] = n, blobkind[sl] = vs;
    HRESULT hr;
    if (dev11) {
        hr = vs ? ID3D11Device_CreateVertexShader(dev11, b, n, NULL, (ID3D11VertexShader **)&shader[sl]) : ID3D11Device_CreatePixelShader(dev11, b, n, NULL, (ID3D11PixelShader **)&shader[sl]);
        if (SUCCEEDED(hr) && vs) ID3D11DeviceContext_VSSetShader(ctx11(), (ID3D11VertexShader *)shader[sl], NULL, 0);
        if (SUCCEEDED(hr) && !vs) ID3D11DeviceContext_PSSetShader(ctx11(), (ID3D11PixelShader *)shader[sl], NULL, 0);
    } else {
        hr = vs ? ID3D10Device_CreateVertexShader(dev10, b, n, (ID3D10VertexShader **)&shader[sl]) : ID3D10Device_CreatePixelShader(dev10, b, n, (ID3D10PixelShader **)&shader[sl]);
        if (SUCCEEDED(hr) && vs) ID3D10Device_VSSetShader(dev10, (ID3D10VertexShader *)shader[sl]);
        if (SUCCEEDED(hr) && !vs) ID3D10Device_PSSetShader(dev10, (ID3D10PixelShader *)shader[sl]);
    }
    if (FAILED(hr)) { out("shader hr %08lx\n", hr); shader[sl] = NULL; }
}

/* layout <vs slot> <format>...: CreateInputLayout of TEXCOORD0.. in slot 0, appended, then set */
static void op_layout(void) {
    int sl = slot(1, SHADERS), n = ntok - 2;
    if (n < 1 || n > 8) bad = 1;
    D3D11_INPUT_ELEMENT_DESC e[8];
    for (int i = 0; i < n && !bad; i++) {
        D3D11_INPUT_ELEMENT_DESC d = { "TEXCOORD", i, (DXGI_FORMAT)num(2 + i), 0, D3D11_APPEND_ALIGNED_ELEMENT, D3D11_INPUT_PER_VERTEX_DATA, 0 };
        e[i] = d;
    }
    if (bad || nodev()) return;
    if (!blob[sl]) { out("layout noshader\n"); return; }
    if (layout) IUnknown_Release(layout);
    layout = NULL;
    HRESULT hr;
    if (dev11) {
        hr = ID3D11Device_CreateInputLayout(dev11, e, n, blob[sl], bloblen[sl], (ID3D11InputLayout **)&layout);
        if (SUCCEEDED(hr)) ID3D11DeviceContext_IASetInputLayout(ctx11(), (ID3D11InputLayout *)layout);
    } else {
        hr = ID3D10Device_CreateInputLayout(dev10, (D3D10_INPUT_ELEMENT_DESC *)e, n, blob[sl], bloblen[sl], (ID3D10InputLayout **)&layout);
        if (SUCCEEDED(hr)) ID3D10Device_IASetInputLayout(dev10, (ID3D10InputLayout *)layout);
    }
    if (FAILED(hr)) { out("layout hr %08lx\n", hr); layout = NULL; }
}

/* vtx <words>...: appended to the vertex data */
static void op_vtx(void) {
    for (int i = 1; i < ntok; i++) {
        if (nvtx == VTX) { bad = 1; return; }
        vtx[nvtx++] = (UINT)num(i);
    }
}

/* draw <topology> <vertices> <stride> [<instances>]: the vertex data as an immutable vertex
 * buffer in slot 0, Draw / DrawInstanced; the vertex data starts again empty */
static void op_draw(void) {
    UINT top = (UINT)num(1), n = (UINT)num(2), stride = (UINT)num(3), inst = ntok > 4 ? (UINT)num(4) : 0, off = 0;
    if (ntok > 5) bad = 1;
    if (bad || nodev()) return;
    if (!nvtx) vtx[nvtx++] = 0;
    if (vbuf) IUnknown_Release(vbuf);
    vbuf = NULL;
    D3D11_SUBRESOURCE_DATA sd = { vtx, 0, 0 };
    HRESULT hr;
    if (dev11) {
        D3D11_BUFFER_DESC d = { nvtx * 4, D3D11_USAGE_IMMUTABLE, D3D11_BIND_VERTEX_BUFFER, 0, 0, 0 };
        hr = ID3D11Device_CreateBuffer(dev11, &d, &sd, (ID3D11Buffer **)&vbuf);
        if (FAILED(hr)) { out("draw hr %08lx\n", hr); vbuf = NULL; nvtx = 0; return; }
        ID3D11DeviceContext *c = ctx11();
        ID3D11DeviceContext_IASetVertexBuffers(c, 0, 1, (ID3D11Buffer **)&vbuf, &stride, &off);
        ID3D11DeviceContext_IASetPrimitiveTopology(c, top);
        if (inst) ID3D11DeviceContext_DrawInstanced(c, n, inst, 0, 0);
        else ID3D11DeviceContext_Draw(c, n, 0);
    } else {
        D3D10_BUFFER_DESC d = { nvtx * 4, D3D10_USAGE_IMMUTABLE, D3D10_BIND_VERTEX_BUFFER, 0, 0 };
        hr = ID3D10Device_CreateBuffer(dev10, &d, (void *)&sd, (ID3D10Buffer **)&vbuf);
        if (FAILED(hr)) { out("draw hr %08lx\n", hr); vbuf = NULL; nvtx = 0; return; }
        ID3D10Device_IASetVertexBuffers(dev10, 0, 1, (ID3D10Buffer **)&vbuf, &stride, &off);
        ID3D10Device_IASetPrimitiveTopology(dev10, top);
        if (inst) ID3D10Device_DrawInstanced(dev10, n, inst, 0, 0);
        else ID3D10Device_Draw(dev10, n, 0);
    }
    nvtx = 0;
}

/* target <rtv|-> [<dsv>]: OMSetRenderTargets
 * viewport <x> <y> <w> <h> <min z> <max z> (float bits): RSSetViewports
 * scissor <left> <top> <right> <bottom>: RSSetScissorRects */
static void op_target(void) {
    if (is(0, "target")) {
        int r = is(1, "-") ? -1 : slot(1, SLOTS), d = ntok > 2 ? slot(2, SLOTS) : -1;
        if (ntok > 3) bad = 1;
        if (bad || nodev()) return;
        IUnknown *rv = r >= 0 && viewkind[r] == 1 ? view[r] : NULL, *dv = d >= 0 && viewkind[d] == 2 ? view[d] : NULL;
        if (dev11) ID3D11DeviceContext_OMSetRenderTargets(ctx11(), rv ? 1 : 0, (ID3D11RenderTargetView **)&rv, (ID3D11DepthStencilView *)dv);
        else ID3D10Device_OMSetRenderTargets(dev10, rv ? 1 : 0, (ID3D10RenderTargetView **)&rv, (ID3D10DepthStencilView *)dv);
    } else if (is(0, "viewport")) {
        float v[6];
        for (int i = 0; i < 6; i++) v[i] = fl(1 + i);
        if (ntok != 7) bad = 1;
        if (bad || nodev()) return;
        if (dev11) {
            D3D11_VIEWPORT vp = { v[0], v[1], v[2], v[3], v[4], v[5] };
            ID3D11DeviceContext_RSSetViewports(ctx11(), 1, &vp);
        } else {
            /* (Direct3D 10's viewports are integers) */
            D3D10_VIEWPORT vp = { (INT)v[0], (INT)v[1], (UINT)v[2], (UINT)v[3], v[4], v[5] };
            ID3D10Device_RSSetViewports(dev10, 1, &vp);
        }
    } else {
        RECT rc = { num(1), num(2), num(3), num(4) };
        if (ntok != 5) bad = 1;
        if (bad || nodev()) return;
        if (dev11) ID3D11DeviceContext_RSSetScissorRects(ctx11(), 1, &rc);
        else ID3D10Device_RSSetScissorRects(dev10, 1, &rc);
    }
}

/* rast <fill> <cull> <front ccw> <depth bias> <bias clamp> <slope bias> <depth clip> <scissor>
 *   <multisample> <aa lines>: CreateRasterizerState, RSSetState
 * blend1 <enable> <src> <dest> <op> <src alpha> <dest alpha> <op alpha> <mask> [<factor r g b a>]:
 *   a blend state with render target 0's blending (the others off), OMSetBlendState
 * depthst <enable> <write mask> <func>: a depth stencil state (stencil off), OMSetDepthStencilState */
static void op_state(void) {
    UINT v[12];
    int n = ntok - 1;
    for (int i = 0; i < n && i < 12; i++) v[i] = (UINT)num(1 + i);
    HRESULT hr;
    IUnknown *o = NULL;
    if (is(0, "rast")) {
        if (n != 10) bad = 1;
        if (bad || nodev()) return;
        /* (the same layout in 10 and 11) */
        hr = dev11 ? ID3D11Device_CreateRasterizerState(dev11, (void *)v, (ID3D11RasterizerState **)&o) : ID3D10Device_CreateRasterizerState(dev10, (void *)v, (ID3D10RasterizerState **)&o);
        if (SUCCEEDED(hr)) { if (dev11) ID3D11DeviceContext_RSSetState(ctx11(), (ID3D11RasterizerState *)o); else ID3D10Device_RSSetState(dev10, (ID3D10RasterizerState *)o); }
    } else if (is(0, "blend1")) {
        if (n != 8 && n != 12) bad = 1;
        if (bad || nodev()) return;
        float f[4] = { 0, 0, 0, 0 };
        if (n == 12) memcpy(f, v + 8, 16);
        if (dev11) {
            D3D11_BLEND_DESC d;
            memset(&d, 0, sizeof d);
            d.IndependentBlendEnable = TRUE;
            D3D11_RENDER_TARGET_BLEND_DESC t = { v[0], v[1], v[2], v[3], v[4], v[5], v[6], (UINT8)v[7] };
            d.RenderTarget[0] = t;
            for (int i = 1; i < 8; i++) { D3D11_RENDER_TARGET_BLEND_DESC u = { 0, 2, 1, 1, 2, 1, 1, 0xf }; d.RenderTarget[i] = u; }
            hr = ID3D11Device_CreateBlendState(dev11, &d, (ID3D11BlendState **)&o);
            if (SUCCEEDED(hr)) ID3D11DeviceContext_OMSetBlendState(ctx11(), (ID3D11BlendState *)o, f, 0xffffffff);
        } else {
            D3D10_BLEND_DESC d;
            memset(&d, 0, sizeof d);
            d.BlendEnable[0] = v[0];
            d.SrcBlend = v[1], d.DestBlend = v[2], d.BlendOp = v[3], d.SrcBlendAlpha = v[4], d.DestBlendAlpha = v[5], d.BlendOpAlpha = v[6];
            d.RenderTargetWriteMask[0] = (UINT8)v[7];
            for (int i = 1; i < 8; i++) d.RenderTargetWriteMask[i] = 0xf;
            hr = ID3D10Device_CreateBlendState(dev10, &d, (ID3D10BlendState **)&o);
            if (SUCCEEDED(hr)) ID3D10Device_OMSetBlendState(dev10, (ID3D10BlendState *)o, f, 0xffffffff);
        }
    } else {
        if (n != 3) bad = 1;
        if (bad || nodev()) return;
        D3D11_DEPTH_STENCIL_DESC d = { v[0], v[1], v[2], 0, 0xff, 0xff, { 1, 1, 1, 8 }, { 1, 1, 1, 8 } };
        hr = dev11 ? ID3D11Device_CreateDepthStencilState(dev11, &d, (ID3D11DepthStencilState **)&o) : ID3D10Device_CreateDepthStencilState(dev10, (void *)&d, (ID3D10DepthStencilState **)&o);
        if (SUCCEEDED(hr)) { if (dev11) ID3D11DeviceContext_OMSetDepthStencilState(ctx11(), (ID3D11DepthStencilState *)o, 0); else ID3D10Device_OMSetDepthStencilState(dev10, (ID3D10DepthStencilState *)o, 0); }
    }
    if (FAILED(hr)) { out("%s hr %08lx\n", tok[0], hr); return; }
    keep(o);
}

/* tsrv <view>: the pixel shader's shader resource 0
 * samp <filter> <address u> <v> <w>: a sampler (bias 0, anisotropy 1, NEVER, border 0, lod
 *   -FLT_MAX..FLT_MAX) as the pixel shader's sampler 0
 * cbuf <buffer>: the vertex and pixel shaders' constant buffer 0 */
static void op_bindres(void) {
    if (is(0, "samp")) {
        UINT v[4];
        for (int i = 0; i < 4; i++) v[i] = (UINT)num(1 + i);
        if (ntok != 5) bad = 1;
        if (bad || nodev()) return;
        D3D11_SAMPLER_DESC d = { v[0], v[1], v[2], v[3], 0, 1, D3D11_COMPARISON_NEVER, { 0, 0, 0, 0 }, -FLT_MAX, FLT_MAX };
        IUnknown *o = NULL;
        HRESULT hr = dev11 ? ID3D11Device_CreateSamplerState(dev11, &d, (ID3D11SamplerState **)&o) : ID3D10Device_CreateSamplerState(dev10, (void *)&d, (ID3D10SamplerState **)&o);
        if (FAILED(hr)) { out("samp hr %08lx\n", hr); return; }
        if (dev11) ID3D11DeviceContext_PSSetSamplers(ctx11(), 0, 1, (ID3D11SamplerState **)&o);
        else ID3D10Device_PSSetSamplers(dev10, 0, 1, (ID3D10SamplerState **)&o);
        keep(o);
        return;
    }
    int v = slot(1, SLOTS);
    if (ntok != 2) bad = 1;
    if (bad || nodev()) return;
    if (is(0, "tsrv")) {
        if (!view[v] || viewkind[v] != 0) { out("tsrv noview\n"); return; }
        if (dev11) ID3D11DeviceContext_PSSetShaderResources(ctx11(), 0, 1, (ID3D11ShaderResourceView **)&view[v]);
        else ID3D10Device_PSSetShaderResources(dev10, 0, 1, (ID3D10ShaderResourceView **)&view[v]);
    } else {
        if (!res[v] || resdim[v] != 4) { out("cbuf nores\n"); return; }
        if (dev11) {
            ID3D11DeviceContext_VSSetConstantBuffers(ctx11(), 0, 1, (ID3D11Buffer **)&res[v]);
            ID3D11DeviceContext_PSSetConstantBuffers(ctx11(), 0, 1, (ID3D11Buffer **)&res[v]);
        } else {
            ID3D10Device_VSSetConstantBuffers(dev10, 0, 1, (ID3D10Buffer **)&res[v]);
            ID3D10Device_PSSetConstantBuffers(dev10, 0, 1, (ID3D10Buffer **)&res[v]);
        }
    }
}

static const struct kz_op d3dres_ops[] = {
    { "device10", op_device10 },
    { "device10_1", op_device10 },
    { "fmt", op_fmt },
    { "tex", op_tex },
    { "buf", op_buf },
    { "srv", op_view },
    { "rtv", op_view },
    { "dsv", op_view },
    { "uav", op_view },
    { "clear", op_clear },
    { "cleards", op_clear },
    { "clearuav", op_clear },
    { "update", op_copy },
    { "copy", op_copy },
    { "copysub", op_copy },
    { "resolve", op_copy },
    { "read", op_read },
    { "shader", op_shader },
    { "layout", op_layout },
    { "vtx", op_vtx },
    { "draw", op_draw },
    { "target", op_target },
    { "viewport", op_target },
    { "scissor", op_target },
    { "rast", op_state },
    { "blend1", op_state },
    { "depthst", op_state },
    { "tsrv", op_bindres },
    { "samp", op_bindres },
    { "cbuf", op_bindres },
    { 0 },
};
const struct kz_api d3dres_api = { "d3dres", d3dres_ops, d3dres_reset };
