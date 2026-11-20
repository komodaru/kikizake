#include "core.h"
#define WIDL_C_INLINE_WRAPPERS
#include <d3d12.h>
#include <dxgi1_6.h>
/* DXGI (dxgi.dll loaded only when a case asks for a factory): the factory, the WARP adapter,
 * window association, and flip-model swap chains on the 64x64 window for runner/d3d12.c's
 * queue (a case says device12 first) */

void *kz_d3d12_queue(void);
void kz_d3d12_wait(void);

static IDXGIFactory4 *factory;
static IDXGISwapChain3 *chain;

static void drop_chain(void) {
    if (!chain) return;
    kz_d3d12_wait();
    IDXGISwapChain3_Release(chain);
    chain = NULL;
}

static void dxgi_reset(void) {
    drop_chain();
    if (factory) { IDXGIFactory4_Release(factory); factory = NULL; }
}

/* dxgifactory <flags>: CreateDXGIFactory2(flags, IDXGIFactory4) */
static void op_factory(void) {
    dxgi_reset();
    static HRESULT (WINAPI *create)(UINT, REFIID, void **);
    if (!create) {
        HMODULE m = LoadLibraryA("dxgi.dll");
        if (m) create = (void *)GetProcAddress(m, "CreateDXGIFactory2");
    }
    UINT flags = num(1);
    if (bad || ntok != 2) { bad = 1; return; }
    if (!create) { out("factory none\n"); return; }
    HRESULT hr = create(flags, &IID_IDXGIFactory4, (void **)&factory);
    if (FAILED(hr)) { factory = NULL; out("factory hr %08lx\n", hr); return; }
    out("factory ok\n");
}

/* warp: EnumWarpAdapter, its DXGI_ADAPTER_DESC1: "warp vendor <id> device <id> flags <flags>" */
static void op_warp(void) {
    if (!factory) { out("nofactory\n"); return; }
    IDXGIAdapter1 *a = NULL;
    HRESULT hr = IDXGIFactory4_EnumWarpAdapter(factory, &IID_IDXGIAdapter1, (void **)&a);
    if (FAILED(hr)) { out("warp hr %08lx\n", hr); return; }
    DXGI_ADAPTER_DESC1 d;
    hr = IDXGIAdapter1_GetDesc1(a, &d);
    if (FAILED(hr)) out("warp hr %08lx\n", hr);
    else out("warp vendor %x device %x flags %x\n", d.VendorId, d.DeviceId, d.Flags);
    IDXGIAdapter1_Release(a);
}

/* dxgiassoc <flags> [null]: MakeWindowAssociation on the window (or NULL): "assoc hr <hr>" */
static void op_assoc(void) {
    if (!factory) { out("nofactory\n"); return; }
    UINT flags = num(1);
    int null = is(2, "null");
    if (bad || ntok != 2 + null) { bad = 1; return; }
    out("assoc hr %08lx\n", IDXGIFactory4_MakeWindowAssociation(factory, null ? NULL : kz_window(), flags));
}

static void print_chain(const char *what) {
    DXGI_SWAP_CHAIN_DESC1 d;
    memset(&d, 0xcc, sizeof d);
    HRESULT hr = IDXGISwapChain3_GetDesc1(chain, &d);
    if (FAILED(hr)) { out("%s desc hr %08lx\n", what, hr); return; }
    out("%s = %x %x %x %x %x %x %x %x %x %x %x %x index %x\n", what, d.Width, d.Height, d.Format, d.Stereo, d.SampleDesc.Count, d.SampleDesc.Quality,
        d.BufferUsage, d.BufferCount, d.Scaling, d.SwapEffect, d.AlphaMode, d.Flags, IDXGISwapChain3_GetCurrentBackBufferIndex(chain));
}

/* swapchain <width> <height> <format> <stereo> <samples> <quality> <usage> <buffers> <scaling>
 * <effect> <alpha mode> <flags>: CreateSwapChainForHwnd (windowed) on the window and the queue
 * (the case's earlier chain released first): "swapchain = <GetDesc1's 12 fields> index <i>" or
 * "swapchain hr <hr>" */
static void op_swapchain(void) {
    if (!factory) { out("nofactory\n"); return; }
    IUnknown *q = kz_d3d12_queue();
    if (!q) { out("nodev\n"); return; }
    if (ntok != 13) { bad = 1; return; }
    DXGI_SWAP_CHAIN_DESC1 d = { num(1), num(2), num(3), num(4), { num(5), num(6) }, num(7), num(8), num(9), num(10), num(11), num(12) };
    if (bad) return;
    drop_chain();
    IDXGISwapChain1 *c1 = NULL;
    HRESULT hr = IDXGIFactory4_CreateSwapChainForHwnd(factory, q, kz_window(), &d, NULL, NULL, &c1);
    if (FAILED(hr)) { out("swapchain hr %08lx\n", hr); return; }
    hr = IDXGISwapChain1_QueryInterface(c1, &IID_IDXGISwapChain3, (void **)&chain);
    IDXGISwapChain1_Release(c1);
    if (FAILED(hr)) { chain = NULL; out("swapchain qi hr %08lx\n", hr); return; }
    print_chain("swapchain");
}

/* scresize <buffers> <width> <height> <format> <flags>: ResizeBuffers, then as swapchain */
static void op_resize(void) {
    if (!chain) { out("nochain\n"); return; }
    if (ntok != 6) { bad = 1; return; }
    UINT n = num(1), w = num(2), h = num(3), f = num(4), fl = num(5);
    if (bad) return;
    kz_d3d12_wait();
    HRESULT hr = IDXGISwapChain3_ResizeBuffers(chain, n, w, h, f, fl);
    if (FAILED(hr)) { out("resize hr %08lx\n", hr); return; }
    print_chain("resize");
}

/* scbuffer <i>: GetBuffer(i) as an ID3D12Resource: "buffer <dimension> <width> <height> <depth>
 * <mips> <format> <samples> <layout> <flags>" or "buffer hr <hr>" */
static void op_buffer(void) {
    if (!chain) { out("nochain\n"); return; }
    UINT i = num(1);
    if (bad || ntok != 2) { bad = 1; return; }
    ID3D12Resource *r = NULL;
    HRESULT hr = IDXGISwapChain3_GetBuffer(chain, i, &IID_ID3D12Resource, (void **)&r);
    if (FAILED(hr)) { out("buffer hr %08lx\n", hr); return; }
    D3D12_RESOURCE_DESC d = ID3D12Resource_GetDesc(r);
    out("buffer %x %lx %x %x %x %x %x %x %x\n", d.Dimension, (unsigned long)d.Width, d.Height, d.DepthOrArraySize, d.MipLevels, d.Format, d.SampleDesc.Count, d.Layout, d.Flags);
    ID3D12Resource_Release(r);
}

/* scpresent <sync interval> <flags>: "present hr <hr> index <i>" */
static void op_present(void) {
    if (!chain) { out("nochain\n"); return; }
    UINT s = num(1), f = num(2);
    if (bad || ntok != 3) { bad = 1; return; }
    HRESULT hr = IDXGISwapChain3_Present(chain, s, f);
    out("present hr %08lx index %x\n", hr, IDXGISwapChain3_GetCurrentBackBufferIndex(chain));
}

/* sclatency <n>: SetMaximumFrameLatency(n), then GetMaximumFrameLatency:
 * "latency hr <set's hr> get <get's hr> <value>" */
static void op_latency(void) {
    if (!chain) { out("nochain\n"); return; }
    UINT n = num(1), got = 0xcccccccc;
    if (bad || ntok != 2) { bad = 1; return; }
    HRESULT hr = IDXGISwapChain3_SetMaximumFrameLatency(chain, n);
    HRESULT g = IDXGISwapChain3_GetMaximumFrameLatency(chain, &got);
    out("latency hr %08lx get %08lx %x\n", hr, g, got);
}

/* dxgitearing: IDXGIFactory5::CheckFeatureSupport(PRESENT_ALLOW_TEARING): "tearing <BOOL>" or
 * "tearing hr <hr>" */
static void op_tearing(void) {
    if (!factory) { out("nofactory\n"); return; }
    if (ntok != 1) { bad = 1; return; }
    IDXGIFactory5 *f5 = NULL;
    HRESULT hr = IDXGIFactory4_QueryInterface(factory, &IID_IDXGIFactory5, (void **)&f5);
    if (FAILED(hr)) { out("tearing hr %08lx\n", hr); return; }
    BOOL b = 0xcccccccc;
    hr = IDXGIFactory5_CheckFeatureSupport(f5, DXGI_FEATURE_PRESENT_ALLOW_TEARING, &b, sizeof b);
    if (FAILED(hr)) out("tearing hr %08lx\n", hr);
    else out("tearing %x\n", b);
    IDXGIFactory5_Release(f5);
}

/* sccolor <color space>: CheckColorSpaceSupport, then SetColorSpace1: "color <flags> set <hr>" or
 * "color hr <hr> set <hr>" */
static void op_color(void) {
    if (!chain) { out("nochain\n"); return; }
    UINT cs = num(1), fl = 0xcccccccc;
    if (bad || ntok != 2) { bad = 1; return; }
    HRESULT hr = IDXGISwapChain3_CheckColorSpaceSupport(chain, cs, &fl);
    if (FAILED(hr)) out("color hr %08lx", hr);
    else out("color %x", fl);
    out(" set %08lx\n", IDXGISwapChain3_SetColorSpace1(chain, cs));
}

/* dxgiadapters: EnumAdapters1 until NOT_FOUND: "adapters <n> last <vendor> <device> <flags>" (the
 * last is the Basic Render Driver on a machine without a GPU) */
static void op_adapters(void) {
    if (!factory) { out("nofactory\n"); return; }
    if (ntok != 1) { bad = 1; return; }
    UINT n = 0;
    DXGI_ADAPTER_DESC1 d = { 0 };
    IDXGIAdapter1 *a;
    while (n < 64 && SUCCEEDED(IDXGIFactory4_EnumAdapters1(factory, n, &a))) {
        IDXGIAdapter1_GetDesc1(a, &d);
        IDXGIAdapter1_Release(a);
        n++;
    }
    out("adapters %x last %x %x %x\n", n, d.VendorId, d.DeviceId, d.Flags);
}

/* scpresent1 <sync> <flags> <scroll 0|1> <l t r b> <dx dy> <n> <n rects l t r b>: Present1 with
 * those dirty rects and scroll: "present1 hr <hr> index <i>" */
static void op_present1(void) {
    if (!chain) { out("nochain\n"); return; }
    if (ntok < 11) { bad = 1; return; }
    UINT n = num(10);
    if (n > 8 || ntok != 11 + 4 * (int)n) { bad = 1; return; }
    RECT r[8], sc = { num(4), num(5), num(6), num(7) };
    POINT off = { num(8), num(9) };
    for (UINT i = 0; i < n; i++) { r[i].left = num(11 + 4 * i); r[i].top = num(12 + 4 * i); r[i].right = num(13 + 4 * i); r[i].bottom = num(14 + 4 * i); }
    DXGI_PRESENT_PARAMETERS pp = { n, n ? r : NULL, num(3) ? &sc : NULL, num(3) ? &off : NULL };
    UINT s = num(1), f = num(2);
    if (bad) return;
    HRESULT hr = IDXGISwapChain3_Present1(chain, s, f, &pp);
    out("present1 hr %08lx index %x\n", hr, IDXGISwapChain3_GetCurrentBackBufferIndex(chain));
}

/* schdr <type> <size> <null 0|1>: SetHDRMetaData with a zeroed buffer of size bytes (at most 128)
 * or NULL: "hdr hr <hr>"; scwait: GetFrameLatencyWaitableObject: "wait null" or "wait handle" */
static void op_hdr(void) {
    if (!chain) { out("nochain\n"); return; }
    if (is(0, "scwait")) {
        if (ntok != 1) { bad = 1; return; }
        HANDLE h = IDXGISwapChain3_GetFrameLatencyWaitableObject(chain);
        out("wait %s\n", h ? "handle" : "null");
        if (h) CloseHandle(h);
        return;
    }
    if (ntok != 4) { bad = 1; return; }
    UINT t = num(1), size = num(2), null = num(3);
    if (bad || size > 128) { bad = 1; return; }
    static BYTE buf[128];
    memset(buf, 0, sizeof buf);
    IDXGISwapChain4 *c4 = NULL;
    HRESULT hr = IDXGISwapChain3_QueryInterface(chain, &IID_IDXGISwapChain4, (void **)&c4);
    if (FAILED(hr)) { out("hdr qi hr %08lx\n", hr); return; }
    hr = IDXGISwapChain4_SetHDRMetaData(c4, t, size, null ? NULL : buf);
    IDXGISwapChain4_Release(c4);
    out("hdr hr %08lx\n", hr);
}

/* dxgifeat <feature> <size>: IDXGIFactory5::CheckFeatureSupport into 16 bytes of 0xcc: "feat hr
 * <hr> <first dword>"; dxgipref <index> <preference>: IDXGIFactory6::EnumAdapterByGpuPreference:
 * "pref hr <hr>" and " <vendor> <device> <flags>" */
static void op_factory5(void) {
    if (!factory) { out("nofactory\n"); return; }
    if (ntok != 3) { bad = 1; return; }
    UINT a = num(1), b = num(2);
    if (bad) return;
    if (is(0, "dxgifeat")) {
        if (b > 16) { bad = 1; return; }
        IDXGIFactory5 *f5 = NULL;
        HRESULT hr = IDXGIFactory4_QueryInterface(factory, &IID_IDXGIFactory5, (void **)&f5);
        if (FAILED(hr)) { out("feat qi hr %08lx\n", hr); return; }
        UINT v[4] = { 0xcccccccc, 0xcccccccc, 0xcccccccc, 0xcccccccc };
        hr = IDXGIFactory5_CheckFeatureSupport(f5, a, v, b);
        IDXGIFactory5_Release(f5);
        out("feat hr %08lx %x\n", hr, v[0]);
        return;
    }
    IDXGIFactory6 *f6 = NULL;
    HRESULT hr = IDXGIFactory4_QueryInterface(factory, &IID_IDXGIFactory6, (void **)&f6);
    if (FAILED(hr)) { out("pref qi hr %08lx\n", hr); return; }
    IDXGIAdapter1 *ad = NULL;
    hr = IDXGIFactory6_EnumAdapterByGpuPreference(f6, a, b, &IID_IDXGIAdapter1, (void **)&ad);
    IDXGIFactory6_Release(f6);
    out("pref hr %08lx", hr);
    if (SUCCEEDED(hr) && ad) {
        DXGI_ADAPTER_DESC1 d;
        if (SUCCEEDED(IDXGIAdapter1_GetDesc1(ad, &d))) out(" %x %x %x", d.VendorId, d.DeviceId, d.Flags);
        IDXGIAdapter1_Release(ad);
    }
    out("\n");
}

static const struct kz_op dxgi_ops[] = {
    { "scpresent1", op_present1 },
    { "schdr", op_hdr },
    { "scwait", op_hdr },
    { "dxgifeat", op_factory5 },
    { "dxgipref", op_factory5 },
    { "dxgitearing", op_tearing },
    { "sccolor", op_color },
    { "dxgiadapters", op_adapters },
    { "dxgifactory", op_factory },
    { "dxgiwarp", op_warp },
    { "dxgiassoc", op_assoc },
    { "swapchain", op_swapchain },
    { "scresize", op_resize },
    { "scbuffer", op_buffer },
    { "scpresent", op_present },
    { "sclatency", op_latency },
    { 0 },
};
const struct kz_api dxgi_api = { "dxgi", dxgi_ops, dxgi_reset };
