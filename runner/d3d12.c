#include "core.h"
#define WIDL_C_INLINE_WRAPPERS
#include <initguid.h>
#include <d3d12.h>
#include <dxgi1_6.h> /* (the DXGI interface ids are defined here, for runner/dxgi.c too) */
/* Direct3D 12 (d3d12.dll loaded only when a case asks for a device): resource descriptions
 * (GetCopyableFootprints, GetResourceAllocationInfo, CreateCommittedResource's validation) and
 * clears read back through a copy. runner/dxgi.c takes the command queue (kz_d3d12_queue). */

static ID3D12Device *dev12;
static ID3D12CommandQueue *queue12;
static ID3D12CommandAllocator *alloc12;
static ID3D12GraphicsCommandList *list12;
static ID3D12Fence *fence12;
static UINT64 fenced;
static HANDLE event12;

#define REL(p) do { if (p) { IUnknown_Release((IUnknown *)(p)); (p) = NULL; } } while (0)

/* the device of this case (device12 asked for), or NULL: the device itself is made once and kept
 * for every case after (a 32-bit executor that made and released one per case crashed after some
 * 600 of them, golden d3d12-32 c4c7692) */
static int on12;

static void d3d12_reset(void) {
    on12 = 0;
    /* (a removed device is not kept, nor released: left as it is) */
    if (dev12 && ID3D12Device_GetDeviceRemovedReason(dev12) != S_OK) list12 = NULL, alloc12 = NULL, fence12 = NULL, queue12 = NULL, dev12 = NULL;
}

void *kz_d3d12_queue(void) { return on12 ? queue12 : NULL; }

/* the queue's work done */
void kz_d3d12_wait(void) {
    if (!queue12) return;
    ID3D12CommandQueue_Signal(queue12, fence12, ++fenced);
    if (ID3D12Fence_GetCompletedValue(fence12) < fenced) {
        ID3D12Fence_SetEventOnCompletion(fence12, fenced, event12);
        WaitForSingleObject(event12, 10000);
    }
}

static void hex64(UINT64 v) {
    if (v >> 32) out(" %lx%08lx", (unsigned long)(v >> 32), (unsigned long)v);
    else out(" %lx", (unsigned long)v);
}

/* device12: D3D12CreateDevice on the default adapter, feature level 11_0 (on a system with no
 * GPU the default adapter is the Basic Render Driver, WARP); a direct queue, allocator, list */
static void op_device12(void) {
    on12 = 0;
    if (dev12) { on12 = 1; out("device12 ok\n"); return; }
    static HRESULT (WINAPI *create)(IUnknown *, D3D_FEATURE_LEVEL, REFIID, void **);
    if (!create) {
        HMODULE m = LoadLibraryA("d3d12.dll");
        if (m) create = (void *)GetProcAddress(m, "D3D12CreateDevice");
    }
    if (!create) { out("device12 none\n"); return; }
    HRESULT hr = create(NULL, D3D_FEATURE_LEVEL_11_0, &IID_ID3D12Device, (void **)&dev12);
    if (FAILED(hr)) { out("device12 failed %08lx\n", hr); dev12 = NULL; return; }
    D3D12_COMMAND_QUEUE_DESC qd = { D3D12_COMMAND_LIST_TYPE_DIRECT, 0, D3D12_COMMAND_QUEUE_FLAG_NONE, 0 };
    if (FAILED(hr = ID3D12Device_CreateCommandQueue(dev12, &qd, &IID_ID3D12CommandQueue, (void **)&queue12))
        || FAILED(hr = ID3D12Device_CreateCommandAllocator(dev12, D3D12_COMMAND_LIST_TYPE_DIRECT, &IID_ID3D12CommandAllocator, (void **)&alloc12))
        || FAILED(hr = ID3D12Device_CreateCommandList(dev12, 0, D3D12_COMMAND_LIST_TYPE_DIRECT, alloc12, NULL, &IID_ID3D12GraphicsCommandList, (void **)&list12))
        || FAILED(hr = ID3D12Device_CreateFence(dev12, 0, D3D12_FENCE_FLAG_NONE, &IID_ID3D12Fence, (void **)&fence12))) {
        out("device12 failed %08lx\n", hr);
        REL(list12); REL(alloc12); REL(fence12); REL(queue12); REL(dev12);
        return;
    }
    ID3D12GraphicsCommandList_Close(list12);
    if (!event12) event12 = CreateEventA(NULL, FALSE, FALSE, NULL);
    on12 = 1;
    out("device12 ok\n");
}

/* a D3D12_RESOURCE_DESC from 11 tokens at i: dimension alignment width height depth mips
 * format samples quality layout flags */
static D3D12_RESOURCE_DESC rdesc(int i) {
    D3D12_RESOURCE_DESC d;
    if (ntok - i != 11) bad = 1;
    d.Dimension = num(i);
    d.Alignment = (UINT)num(i + 1);
    d.Width = (UINT)num(i + 2);
    d.Height = (UINT)num(i + 3);
    d.DepthOrArraySize = (UINT16)num(i + 4);
    d.MipLevels = (UINT16)num(i + 5);
    d.Format = num(i + 6);
    d.SampleDesc.Count = num(i + 7);
    d.SampleDesc.Quality = num(i + 8);
    d.Layout = num(i + 9);
    d.Flags = num(i + 10);
    return d;
}

/* footprint12 <first> <count> <base> <desc>: GetCopyableFootprints; per subresource
 * "fp <offset> <format> <width> <height> <depth> <row pitch> <rows> <row size>", then
 * "total <bytes>" (the outputs start as 0xcc bytes) */
#define FPS 16
static void op_footprint(void) {
    if (!on12) { out("nodev\n"); return; }
    UINT first = num(1), count = num(2);
    UINT64 base = (UINT)num(3);
    D3D12_RESOURCE_DESC d = rdesc(4);
    if (bad || count > FPS) { bad = 1; return; }
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT fp[FPS];
    UINT rows[FPS];
    UINT64 size[FPS], total;
    memset(fp, 0xcc, sizeof fp);
    memset(rows, 0xcc, sizeof rows);
    memset(size, 0xcc, sizeof size);
    memset(&total, 0xcc, sizeof total);
    ID3D12Device_GetCopyableFootprints(dev12, &d, first, count, base, fp, rows, size, &total);
    for (UINT k = 0; k < count; k++) {
        out("fp");
        hex64(fp[k].Offset);
        out(" %lx %lx %lx %lx %lx %lx", (unsigned long)fp[k].Footprint.Format, (unsigned long)fp[k].Footprint.Width, (unsigned long)fp[k].Footprint.Height,
            (unsigned long)fp[k].Footprint.Depth, (unsigned long)fp[k].Footprint.RowPitch, (unsigned long)rows[k]);
        hex64(size[k]);
        out("\n");
    }
    out("total");
    hex64(total);
    out("\n");
}

/* alloc12 <desc>: GetResourceAllocationInfo(0, 1, desc): "alloc <size> <alignment>" */
static void op_alloc(void) {
    if (!on12) { out("nodev\n"); return; }
    D3D12_RESOURCE_DESC d = rdesc(1);
    if (bad) return;
    D3D12_RESOURCE_ALLOCATION_INFO a = ID3D12Device_GetResourceAllocationInfo(dev12, 0, 1, &d);
    out("alloc");
    hex64(a.SizeInBytes);
    hex64(a.Alignment);
    out("\n");
}

/* create12 <heap type> <initial state> <desc>: CreateCommittedResource with no heap flags, no
 * clear12 value and ppvResource NULL (validation only): "create hr <hr>" */
static void op_create(void) {
    if (!on12) { out("nodev\n"); return; }
    D3D12_HEAP_PROPERTIES hp = { num(1), D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN, 0, 0 };
    D3D12_RESOURCE_STATES st = num(2);
    D3D12_RESOURCE_DESC d = rdesc(3);
    if (bad) return;
    HRESULT hr = ID3D12Device_CreateCommittedResource(dev12, &hp, D3D12_HEAP_FLAG_NONE, &d, st, NULL, &IID_ID3D12Resource, NULL);
    out("create hr %08lx\n", hr);
}

/* feature12 <format>: CheckFeatureSupport FORMAT_SUPPORT, FORMAT_INFO: "fsup <support1> <support2>
 * planes <n>" (or "fsup hr <hr>", "planes hr <hr>"); msq12 <format> <count>: MULTISAMPLE_QUALITY_LEVELS
 * (no flags): "msq <levels>" or "msq hr <hr>"; incr12: GetDescriptorHandleIncrementSize of the four
 * heap types: "incr <cbv srv uav> <sampler> <rtv> <dsv>" */
static void op_feature(void) {
    if (!on12) { out("nodev\n"); return; }
    if (is(0, "incr12")) {
        if (ntok != 1) { bad = 1; return; }
        out("incr");
        for (int t = 0; t < 4; t++) out(" %x", ID3D12Device_GetDescriptorHandleIncrementSize(dev12, t));
        out("\n");
        return;
    }
    int ms = is(0, "msq12");
    DXGI_FORMAT f = num(1);
    if (ntok != (ms ? 3 : 2)) bad = 1;
    if (bad) return;
    HRESULT hr;
    if (ms) {
        D3D12_FEATURE_DATA_MULTISAMPLE_QUALITY_LEVELS q = { f, num(2), 0, 0xcccccccc };
        hr = ID3D12Device_CheckFeatureSupport(dev12, D3D12_FEATURE_MULTISAMPLE_QUALITY_LEVELS, &q, sizeof q);
        if (FAILED(hr)) out("msq hr %08lx\n", hr);
        else out("msq %x\n", q.NumQualityLevels);
        return;
    }
    D3D12_FEATURE_DATA_FORMAT_SUPPORT fs = { f, 0xcccccccc, 0xcccccccc };
    hr = ID3D12Device_CheckFeatureSupport(dev12, D3D12_FEATURE_FORMAT_SUPPORT, &fs, sizeof fs);
    if (FAILED(hr)) out("fsup hr %08lx", hr);
    else out("fsup %x %x", fs.Support1, fs.Support2);
    D3D12_FEATURE_DATA_FORMAT_INFO fi = { f, 0xcc };
    hr = ID3D12Device_CheckFeatureSupport(dev12, D3D12_FEATURE_FORMAT_INFO, &fi, sizeof fi);
    if (FAILED(hr)) out(" planes hr %08lx\n", hr);
    else out(" planes %x\n", fi.PlaneCount);
}

/* object creation checked, the object made and released (NULL where the API takes it):
 *   heap12 <type> <size> <alignment> <heap flags>            CreateHeap
 *   dheap12 <type> <descriptors> <flags>                     CreateDescriptorHeap
 *   queue12 <type> <priority> <flags>                        CreateCommandQueue
 *   cmdlist12 <type>                                         CreateCommandAllocator, CreateCommandList
 *   qheap12 <type> <count>                                   CreateQueryHeap
 *   fence12 <initial> <flags>                                CreateFence, GetCompletedValue
 *   placed12 <heap type> <heap flags> <heap size> <offset> <state> <desc>   CreatePlacedResource
 * "<op> hr <hr>" (fence12 adds "value <v>") */
static void op_make(void) {
    if (!on12) { out("nodev\n"); return; }
    HRESULT hr = E_FAIL;
    IUnknown *o = NULL;
    if (is(0, "heap12")) {
        if (ntok != 5) bad = 1;
        D3D12_HEAP_DESC d = { num(2), { num(1), D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN, 0, 0 }, num(3), num(4) };
        if (bad) return;
        hr = ID3D12Device_CreateHeap(dev12, &d, &IID_ID3D12Heap, (void **)&o);
    } else if (is(0, "dheap12")) {
        if (ntok != 4) bad = 1;
        D3D12_DESCRIPTOR_HEAP_DESC d = { num(1), num(2), num(3), 0 };
        if (bad) return;
        hr = ID3D12Device_CreateDescriptorHeap(dev12, &d, &IID_ID3D12DescriptorHeap, (void **)&o);
    } else if (is(0, "queue12")) {
        if (ntok != 4) bad = 1;
        D3D12_COMMAND_QUEUE_DESC d = { num(1), (INT)num(2), num(3), 0 };
        if (bad) return;
        hr = ID3D12Device_CreateCommandQueue(dev12, &d, &IID_ID3D12CommandQueue, (void **)&o);
    } else if (is(0, "cmdlist12")) {
        if (ntok != 2) bad = 1;
        D3D12_COMMAND_LIST_TYPE t = num(1);
        if (bad) return;
        ID3D12CommandAllocator *a = NULL;
        hr = ID3D12Device_CreateCommandAllocator(dev12, t, &IID_ID3D12CommandAllocator, (void **)&a);
        if (FAILED(hr)) { out("cmdlist12 allocator hr %08lx\n", hr); return; }
        hr = ID3D12Device_CreateCommandList(dev12, 0, t, a, NULL, &IID_ID3D12GraphicsCommandList, (void **)&o);
        if (o) ID3D12GraphicsCommandList_Close((ID3D12GraphicsCommandList *)o);
        REL(o);
        ID3D12CommandAllocator_Release(a);
    } else if (is(0, "qheap12")) {
        if (ntok != 3) bad = 1;
        D3D12_QUERY_HEAP_DESC d = { num(1), num(2), 0 };
        if (bad) return;
        hr = ID3D12Device_CreateQueryHeap(dev12, &d, &IID_ID3D12QueryHeap, (void **)&o);
    } else if (is(0, "fence12")) {
        if (ntok != 3) bad = 1;
        UINT64 v0 = (UINT)num(1);
        D3D12_FENCE_FLAGS fl = num(2);
        if (bad) return;
        ID3D12Fence *f = NULL;
        hr = ID3D12Device_CreateFence(dev12, v0, fl, &IID_ID3D12Fence, (void **)&f);
        out("fence12 hr %08lx", hr);
        if (f) { out(" value"); hex64(ID3D12Fence_GetCompletedValue(f)); ID3D12Fence_Release(f); }
        out("\n");
        return;
    } else {
        if (ntok != 17) { bad = 1; return; }
        D3D12_HEAP_DESC hd = { num(3), { num(1), D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN, 0, 0 }, 0, num(2) };
        UINT64 off = (UINT)num(4);
        D3D12_RESOURCE_STATES st = num(5);
        D3D12_RESOURCE_DESC d = rdesc(6);
        if (bad) return;
        ID3D12Heap *h = NULL;
        hr = ID3D12Device_CreateHeap(dev12, &hd, &IID_ID3D12Heap, (void **)&h);
        if (FAILED(hr)) { out("placed12 heap hr %08lx\n", hr); return; }
        hr = ID3D12Device_CreatePlacedResource(dev12, h, off, &d, st, NULL, &IID_ID3D12Resource, (void **)&o);
        REL(o);
        ID3D12Heap_Release(h);
    }
    REL(o);
    out("%s hr %08lx\n", tok[0], hr);
}

static ID3D12Resource *committed(D3D12_HEAP_TYPE heap, D3D12_RESOURCE_STATES st, const D3D12_RESOURCE_DESC *d, HRESULT *hr) {
    D3D12_HEAP_PROPERTIES hp = { heap, D3D12_CPU_PAGE_PROPERTY_UNKNOWN, D3D12_MEMORY_POOL_UNKNOWN, 0, 0 };
    ID3D12Resource *r = NULL;
    *hr = ID3D12Device_CreateCommittedResource(dev12, &hp, D3D12_HEAP_FLAG_NONE, d, st, NULL, &IID_ID3D12Resource, (void **)&r);
    return SUCCEEDED(*hr) ? r : NULL;
}

/* clear12 <format> <r> <g> <b> <a> (float bits) | cleardsv <format> <depth bits> <stencil>: a
 * 1x1 texture of that format cleared, each plane copied to a readback buffer:
 * "clear <plane 0's texel bytes> [<plane 1's>]" or "clear hr <hr>" */
static void op_clear(void) {
    if (!on12) { out("nodev\n"); return; }
    int dsv = is(0, "cleardsv12");
    if (ntok != (dsv ? 4 : 6)) { bad = 1; return; }
    DXGI_FORMAT f = num(1);
    DWORD v[4];
    for (int i = 0; i < ntok - 2; i++) v[i] = (DWORD)num(2 + i);
    if (bad) return;
    D3D12_RESOURCE_DESC d = { D3D12_RESOURCE_DIMENSION_TEXTURE2D, 0, 1, 1, 1, 1, f, { 1, 0 }, D3D12_TEXTURE_LAYOUT_UNKNOWN,
                              dsv ? D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL : D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET };
    HRESULT hr;
    ID3D12Resource *tex = committed(D3D12_HEAP_TYPE_DEFAULT, dsv ? D3D12_RESOURCE_STATE_DEPTH_WRITE : D3D12_RESOURCE_STATE_RENDER_TARGET, &d, &hr);
    if (!tex) { out("%s hr %08lx\n", tok[0], hr); return; }
    D3D12_FEATURE_DATA_FORMAT_INFO fi = { f, 1 };
    if (FAILED(ID3D12Device_CheckFeatureSupport(dev12, D3D12_FEATURE_FORMAT_INFO, &fi, sizeof fi)) || fi.PlaneCount < 1 || fi.PlaneCount > 2) fi.PlaneCount = 1;
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT fp[2];
    UINT64 rowsize[2], total = 0;
    ID3D12Device_GetCopyableFootprints(dev12, &d, 0, fi.PlaneCount, 0, fp, NULL, rowsize, &total);
    D3D12_RESOURCE_DESC bd = { D3D12_RESOURCE_DIMENSION_BUFFER, 0, total, 1, 1, 1, DXGI_FORMAT_UNKNOWN, { 1, 0 }, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, 0 };
    ID3D12Resource *buf = total && total < (1 << 20) ? committed(D3D12_HEAP_TYPE_READBACK, D3D12_RESOURCE_STATE_COPY_DEST, &bd, &hr) : NULL;
    D3D12_DESCRIPTOR_HEAP_DESC hd = { dsv ? D3D12_DESCRIPTOR_HEAP_TYPE_DSV : D3D12_DESCRIPTOR_HEAP_TYPE_RTV, 1, 0, 0 };
    ID3D12DescriptorHeap *heap = NULL;
    if (buf) hr = ID3D12Device_CreateDescriptorHeap(dev12, &hd, &IID_ID3D12DescriptorHeap, (void **)&heap);
    if (!buf || !heap) {
        out("%s hr %08lx\n", tok[0], hr);
        REL(heap); REL(buf); REL(tex);
        return;
    }
    D3D12_CPU_DESCRIPTOR_HANDLE h = ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart(heap);
    ID3D12CommandAllocator_Reset(alloc12);
    ID3D12GraphicsCommandList_Reset(list12, alloc12, NULL);
    if (dsv) {
        ID3D12Device_CreateDepthStencilView(dev12, tex, NULL, h);
        FLOAT depth;
        memcpy(&depth, &v[0], 4);
        ID3D12GraphicsCommandList_ClearDepthStencilView(list12, h, D3D12_CLEAR_FLAG_DEPTH | (fi.PlaneCount > 1 ? D3D12_CLEAR_FLAG_STENCIL : 0), depth, (UINT8)v[1], 0, NULL);
    } else {
        ID3D12Device_CreateRenderTargetView(dev12, tex, NULL, h);
        FLOAT c[4];
        memcpy(c, v, sizeof c);
        ID3D12GraphicsCommandList_ClearRenderTargetView(list12, h, c, 0, NULL);
    }
    D3D12_RESOURCE_BARRIER b = { D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, 0 };
    b.Transition.pResource = tex;
    b.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
    b.Transition.StateBefore = dsv ? D3D12_RESOURCE_STATE_DEPTH_WRITE : D3D12_RESOURCE_STATE_RENDER_TARGET;
    b.Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
    ID3D12GraphicsCommandList_ResourceBarrier(list12, 1, &b);
    for (UINT p = 0; p < fi.PlaneCount; p++) {
        D3D12_TEXTURE_COPY_LOCATION dst = { buf, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT }, src = { tex, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX };
        dst.PlacedFootprint = fp[p];
        src.SubresourceIndex = p;
        ID3D12GraphicsCommandList_CopyTextureRegion(list12, &dst, 0, 0, 0, &src, NULL);
    }
    hr = ID3D12GraphicsCommandList_Close(list12);
    if (SUCCEEDED(hr)) {
        ID3D12CommandQueue_ExecuteCommandLists(queue12, 1, (ID3D12CommandList **)&list12);
        kz_d3d12_wait();
        unsigned char *m = NULL;
        D3D12_RANGE r = { 0, total };
        hr = ID3D12Resource_Map(buf, 0, &r, (void **)&m);
        if (SUCCEEDED(hr)) {
            out("%s", tok[0]);
            for (UINT p = 0; p < fi.PlaneCount; p++) {
                out(" ");
                for (UINT64 k = 0; k < rowsize[p] && k < 16; k++) out("%02x", m[fp[p].Offset + k]);
            }
            out("\n");
            ID3D12Resource_Unmap(buf, 0, NULL);
        }
    }
    if (FAILED(hr)) out("%s hr %08lx\n", tok[0], hr);
    REL(heap); REL(buf); REL(tex);
}

/* copy12 <format> <width> <height> <depth stencil 0|1>: a 2D texture of that format (all its
 * planes) filled from an upload buffer with byte k of the footprint layout (k * 7 + 3) & 255, then
 * copied back to a readback buffer: "copy <bytes that came back different> first <the first 16
 * bytes back>" or "copy hr <hr>" (only the bytes inside each row's size are compared) */
static void op_copy(void) {
    if (!on12) { out("nodev\n"); return; }
    if (ntok != 5) bad = 1;
    DXGI_FORMAT f = num(1);
    UINT w = num(2), h = num(3), ds = num(4);
    if (bad || w > 4096 || h > 4096) { bad = 1; return; }
    D3D12_RESOURCE_DESC d = { D3D12_RESOURCE_DIMENSION_TEXTURE2D, 0, w, h, 1, 1, f, { 1, 0 }, D3D12_TEXTURE_LAYOUT_UNKNOWN,
                              ds ? D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL : D3D12_RESOURCE_FLAG_NONE };
    HRESULT hr;
    ID3D12Resource *tex = committed(D3D12_HEAP_TYPE_DEFAULT, D3D12_RESOURCE_STATE_COPY_DEST, &d, &hr);
    if (!tex) { out("copy hr %08lx\n", hr); return; }
    D3D12_FEATURE_DATA_FORMAT_INFO fi = { f, 1 };
    if (FAILED(ID3D12Device_CheckFeatureSupport(dev12, D3D12_FEATURE_FORMAT_INFO, &fi, sizeof fi)) || fi.PlaneCount < 1 || fi.PlaneCount > 2) fi.PlaneCount = 1;
    D3D12_PLACED_SUBRESOURCE_FOOTPRINT fp[2];
    UINT rows[2];
    UINT64 rowsize[2], total = 0;
    ID3D12Device_GetCopyableFootprints(dev12, &d, 0, fi.PlaneCount, 0, fp, rows, rowsize, &total);
    /* (no layout to copy through: 420_OPAQUE, golden e.g. d3d12-64 copy-106 stopped the executor) */
    if (total == 0 || total == ~(UINT64)0) { out("copy nofootprint\n"); REL(tex); return; }
    D3D12_RESOURCE_DESC bd = { D3D12_RESOURCE_DIMENSION_BUFFER, 0, total, 1, 1, 1, DXGI_FORMAT_UNKNOWN, { 1, 0 }, D3D12_TEXTURE_LAYOUT_ROW_MAJOR, 0 };
    ID3D12Resource *up = committed(D3D12_HEAP_TYPE_UPLOAD, D3D12_RESOURCE_STATE_GENERIC_READ, &bd, &hr);
    ID3D12Resource *back = up ? committed(D3D12_HEAP_TYPE_READBACK, D3D12_RESOURCE_STATE_COPY_DEST, &bd, &hr) : NULL;
    unsigned char *m = NULL;
    if (back && SUCCEEDED(hr = ID3D12Resource_Map(up, 0, NULL, (void **)&m))) {
        for (UINT64 k = 0; k < total; k++) m[k] = (unsigned char)(k * 7 + 3);
        ID3D12Resource_Unmap(up, 0, NULL);
    }
    if (!back || FAILED(hr)) { out("copy hr %08lx\n", hr); REL(back); REL(up); REL(tex); return; }
    ID3D12CommandAllocator_Reset(alloc12);
    ID3D12GraphicsCommandList_Reset(list12, alloc12, NULL);
    for (int way = 0; way < 2; way++) {
        for (UINT p = 0; p < fi.PlaneCount; p++) {
            D3D12_TEXTURE_COPY_LOCATION b = { way ? back : up, D3D12_TEXTURE_COPY_TYPE_PLACED_FOOTPRINT }, t = { tex, D3D12_TEXTURE_COPY_TYPE_SUBRESOURCE_INDEX };
            b.PlacedFootprint = fp[p];
            t.SubresourceIndex = p;
            if (way) ID3D12GraphicsCommandList_CopyTextureRegion(list12, &b, 0, 0, 0, &t, NULL);
            else ID3D12GraphicsCommandList_CopyTextureRegion(list12, &t, 0, 0, 0, &b, NULL);
        }
        if (!way) {
            D3D12_RESOURCE_BARRIER br = { D3D12_RESOURCE_BARRIER_TYPE_TRANSITION, 0 };
            br.Transition.pResource = tex;
            br.Transition.Subresource = D3D12_RESOURCE_BARRIER_ALL_SUBRESOURCES;
            br.Transition.StateBefore = D3D12_RESOURCE_STATE_COPY_DEST;
            br.Transition.StateAfter = D3D12_RESOURCE_STATE_COPY_SOURCE;
            ID3D12GraphicsCommandList_ResourceBarrier(list12, 1, &br);
        }
    }
    hr = ID3D12GraphicsCommandList_Close(list12);
    if (SUCCEEDED(hr)) {
        ID3D12CommandQueue_ExecuteCommandLists(queue12, 1, (ID3D12CommandList **)&list12);
        kz_d3d12_wait();
        hr = ID3D12Resource_Map(back, 0, NULL, (void **)&m);
    }
    if (FAILED(hr)) out("copy hr %08lx\n", hr);
    else {
        UINT64 diff = 0;
        for (UINT p = 0; p < fi.PlaneCount; p++)
            for (UINT r = 0; r < rows[p]; r++)
                for (UINT64 k = 0; k < rowsize[p]; k++) {
                    UINT64 at = fp[p].Offset + (UINT64)r * fp[p].Footprint.RowPitch + k;
                    diff += m[at] != (unsigned char)(at * 7 + 3);
                }
        out("copy %lx first ", (unsigned long)diff);
        for (UINT64 k = 0; k < 16 && k < rowsize[0]; k++) out("%02x", m[fp[0].Offset + k]);
        out("\n");
        ID3D12Resource_Unmap(back, 0, NULL);
    }
    REL(back); REL(up); REL(tex);
}

static const struct kz_op d3d12_ops[] = {
    { "copy12", op_copy },
    { "device12", op_device12 },
    { "footprint12", op_footprint },
    { "alloc12", op_alloc },
    { "create12", op_create },
    { "clear12", op_clear },
    { "cleardsv12", op_clear },
    { "feature12", op_feature },
    { "msq12", op_feature },
    { "incr12", op_feature },
    { "heap12", op_make },
    { "dheap12", op_make },
    { "queue12", op_make },
    { "cmdlist12", op_make },
    { "qheap12", op_make },
    { "fence12", op_make },
    { "placed12", op_make },
    { 0 },
};
const struct kz_api d3d12_api = { "d3d12", d3d12_ops, d3d12_reset };
