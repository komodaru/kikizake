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

static const struct kz_op dxgi_ops[] = {
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
