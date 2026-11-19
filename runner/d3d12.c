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
    if (dev12 && ID3D12Device_GetDeviceRemovedReason(dev12) != S_OK) {
        /* (WARP x64 crashed some runs after a removal, the output still buffered lost: golden
         * 465fd71 empty, 317e806 whole; written out at once) */
        oflush();
        list12 = NULL, alloc12 = NULL, fence12 = NULL, queue12 = NULL, dev12 = NULL;
    }
}

void *kz_d3d12_queue(void) { return on12 ? queue12 : NULL; }

/* An incomplete submission cannot supply a measurement. Stop before reading or
 * releasing resources still owned by the GPU. Exit 125 is reserved by windows.yml. */
static void wait_error(HRESULT hr) {
    out("measurement failed: d3d12 wait %08lx\n", hr);
    oflush();
    ExitProcess(125);
}

void kz_d3d12_wait(void) {
    if (!queue12 || !fence12 || !event12) wait_error(E_HANDLE);
    HRESULT hr = ID3D12CommandQueue_Signal(queue12, fence12, ++fenced);
    if (FAILED(hr)) wait_error(hr);
    UINT64 done = ID3D12Fence_GetCompletedValue(fence12);
    if (done == (~(UINT64)0)) wait_error(DXGI_ERROR_DEVICE_REMOVED);
    if (done >= fenced) return;
    hr = ID3D12Fence_SetEventOnCompletion(fence12, fenced, event12);
    if (FAILED(hr)) wait_error(hr);
    DWORD result = WaitForSingleObject(event12, 10000);
    if (result == WAIT_TIMEOUT) wait_error(HRESULT_FROM_WIN32(ERROR_TIMEOUT));
    if (result != WAIT_OBJECT_0) wait_error(HRESULT_FROM_WIN32(GetLastError()));
    done = ID3D12Fence_GetCompletedValue(fence12);
    if (done == (~(UINT64)0) || done < fenced) wait_error(E_FAIL);
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

/* rootsig12 <version 1|2> <flags> <parameter>...: D3D12SerializeVersionedRootSignature, then
 * CreateRootSignature of the blob. Parameters:
 *   c <register> <space> <values> <visibility>          32-bit constants
 *   cbv|srv|uav <register> <space> <visibility> <flags> a root descriptor (flags: 1.1)
 *   t <visibility> <ranges> then per range <type> <descriptors> <base> <space> <offset> <flags>
 *   s <filter> <register> <space> <visibility>           a static sampler (wrap, LOD 0..max)
 * "rs hr <hr> size <bytes>", the blob past its 4-byte magic and 16-byte checksum as "rs <hex>"
 * lines of 64 bytes, "rs sum <checksum hex>", then "rsmake hr <hr>" */
#define RSP 16
#define RSR 32
static void op_rootsig(void) {
    if (!on12) { out("nodev\n"); return; }
    static HRESULT (WINAPI *ser)(const D3D12_VERSIONED_ROOT_SIGNATURE_DESC *, ID3DBlob **, ID3DBlob **);
    if (!ser) ser = (void *)GetProcAddress(GetModuleHandleA("d3d12.dll"), "D3D12SerializeVersionedRootSignature");
    if (!ser) { out("rs none\n"); return; }
    UINT ver = num(1), np = 0, ns = 0, nr = 0;
    D3D12_ROOT_PARAMETER p0[RSP];
    D3D12_ROOT_PARAMETER1 p1[RSP];
    D3D12_DESCRIPTOR_RANGE r0[RSR];
    D3D12_DESCRIPTOR_RANGE1 r1[RSR];
    D3D12_STATIC_SAMPLER_DESC sm[RSP];
    int i = 3;
    while (i < ntok && !bad) {
        if (is(i, "s")) {
            if (ns == RSP || i + 4 >= ntok) { bad = 1; break; }
            D3D12_STATIC_SAMPLER_DESC d = { num(i + 1), 1, 1, 1, 0, 16, 1, 0, 0, 3.402823466e+38f, num(i + 2), num(i + 3), num(i + 4) };
            sm[ns++] = d;
            i += 5;
            continue;
        }
        if (np == RSP) { bad = 1; break; }
        D3D12_ROOT_PARAMETER *a = &p0[np];
        D3D12_ROOT_PARAMETER1 *b = &p1[np];
        if (is(i, "c")) {
            if (i + 4 >= ntok) { bad = 1; break; }
            a->ParameterType = b->ParameterType = D3D12_ROOT_PARAMETER_TYPE_32BIT_CONSTANTS;
            a->Constants.ShaderRegister = b->Constants.ShaderRegister = num(i + 1);
            a->Constants.RegisterSpace = b->Constants.RegisterSpace = num(i + 2);
            a->Constants.Num32BitValues = b->Constants.Num32BitValues = num(i + 3);
            a->ShaderVisibility = b->ShaderVisibility = num(i + 4);
            i += 5;
        } else if (is(i, "cbv") || is(i, "srv") || is(i, "uav")) {
            if (i + 4 >= ntok) { bad = 1; break; }
            a->ParameterType = b->ParameterType = is(i, "cbv") ? D3D12_ROOT_PARAMETER_TYPE_CBV : is(i, "srv") ? D3D12_ROOT_PARAMETER_TYPE_SRV : D3D12_ROOT_PARAMETER_TYPE_UAV;
            a->Descriptor.ShaderRegister = b->Descriptor.ShaderRegister = num(i + 1);
            a->Descriptor.RegisterSpace = b->Descriptor.RegisterSpace = num(i + 2);
            a->ShaderVisibility = b->ShaderVisibility = num(i + 3);
            b->Descriptor.Flags = num(i + 4);
            i += 5;
        } else if (is(i, "t")) {
            if (i + 2 >= ntok) { bad = 1; break; }
            UINT n = num(i + 2);
            if (n > RSR || nr + n > RSR || i + 3 + 6 * (int)n > ntok) { bad = 1; break; }
            a->ParameterType = b->ParameterType = D3D12_ROOT_PARAMETER_TYPE_DESCRIPTOR_TABLE;
            a->ShaderVisibility = b->ShaderVisibility = num(i + 1);
            a->DescriptorTable.NumDescriptorRanges = b->DescriptorTable.NumDescriptorRanges = n;
            a->DescriptorTable.pDescriptorRanges = &r0[nr];
            b->DescriptorTable.pDescriptorRanges = &r1[nr];
            for (UINT k = 0; k < n; k++) {
                int j = i + 3 + 6 * k;
                D3D12_DESCRIPTOR_RANGE x = { num(j), num(j + 1), num(j + 2), num(j + 3), num(j + 4) };
                D3D12_DESCRIPTOR_RANGE1 y = { num(j), num(j + 1), num(j + 2), num(j + 3), num(j + 5), num(j + 4) };
                r0[nr] = x;
                r1[nr++] = y;
            }
            i += 3 + 6 * n;
        } else { bad = 1; break; }
        np++;
    }
    if (bad || ntok < 3) { bad = 1; return; }
    D3D12_VERSIONED_ROOT_SIGNATURE_DESC d;
    memset(&d, 0, sizeof d);
    d.Version = ver;
    if (ver == D3D_ROOT_SIGNATURE_VERSION_1_1) {
        D3D12_ROOT_SIGNATURE_DESC1 x = { np, np ? p1 : NULL, ns, ns ? sm : NULL, num(2) };
        d.Desc_1_1 = x;
    } else {
        D3D12_ROOT_SIGNATURE_DESC x = { np, np ? p0 : NULL, ns, ns ? sm : NULL, num(2) };
        d.Desc_1_0 = x;
    }
    ID3DBlob *blob = NULL, *err = NULL;
    HRESULT hr = ser(&d, &blob, &err);
    if (err) ID3D10Blob_Release(err);
    if (FAILED(hr) || !blob) { out("rs hr %08lx\n", hr); if (blob) ID3D10Blob_Release(blob); return; }
    const unsigned char *m = ID3D10Blob_GetBufferPointer(blob);
    SIZE_T n = ID3D10Blob_GetBufferSize(blob);
    out("rs hr %08lx size %lx\n", hr, (unsigned long)n);
    for (SIZE_T k = 20; k < n; k += 64) {
        out("rs ");
        for (SIZE_T j = k; j < k + 64 && j < n; j++) out("%02x", m[j]);
        out("\n");
    }
    out("rs sum ");
    for (SIZE_T j = 4; j < 20 && j < n; j++) out("%02x", m[j]);
    out("\n");
    ID3D12RootSignature *rs = NULL;
    hr = ID3D12Device_CreateRootSignature(dev12, 0, m, n, &IID_ID3D12RootSignature, (void **)&rs);
    out("rsmake hr %08lx\n", hr);
    REL(rs);
    ID3D10Blob_Release(blob);
}

/* remove12: ID3D12Device5::RemoveDevice (vtable slot 58: IUnknown 3, ID3D12Object 4, ID3D12Device 37,
 * Device1 3, Device2 1, Device3 3, Device4 6, Device5's CreateLifetimeTracker; mingw's d3d12.h
 * stops at Device1): "remove12 ok", or "remove12 none" without ID3D12Device5 */
static const GUID IID_ID3D12Device5_ = { 0x8b4f173b, 0x2fea, 0x4b80, { 0x8f, 0x58, 0x43, 0x07, 0x19, 0x1a, 0xb9, 0x5d } };
static void op_remove(void) {
    if (!on12) { out("nodev\n"); return; }
    IUnknown *d5 = NULL;
    if (FAILED(ID3D12Device_QueryInterface(dev12, &IID_ID3D12Device5_, (void **)&d5)) || !d5) { out("remove12 none\n"); return; }
    void (STDMETHODCALLTYPE *remove)(IUnknown *) = (void *)(*(void ***)d5)[58];
    remove(d5);
    IUnknown_Release(d5);
    out("remove12 ok\n");
    oflush();
}

/* featscan12 <feature> [<input word>...]: CheckFeatureSupport of the feature with a zeroed buffer
 * whose first words are the inputs, at each size 4, 8, .. 512 until one succeeds: "feat size <n>"
 * and the n bytes, or "feat none hr <hr at size 4>" */
static void op_featscan(void) {
    if (!on12) { out("nodev\n"); return; }
    if (ntok < 2 || ntok > 2 + 16) { bad = 1; return; }
    UINT f = num(1);
    UINT in[16];
    int nin = ntok - 2;
    for (int i = 0; i < nin; i++) in[i] = num(2 + i);
    if (bad) return;
    HRESULT first = S_OK;
    for (UINT n = 4; n <= 512; n += 4) {
        UINT b[128];
        memset(b, 0, sizeof b);
        for (int i = 0; i < nin && i < (int)(n / 4); i++) b[i] = in[i];
        HRESULT hr = ID3D12Device_CheckFeatureSupport(dev12, (D3D12_FEATURE)f, b, n);
        if (n == 4) first = hr;
        if (SUCCEEDED(hr)) {
            out("feat size %x hr %08lx ", n, hr);
            out_hex((const unsigned char *)b, n);
            return;
        }
    }
    out("feat none hr %08lx\n", first);
}

static const struct kz_op d3d12_ops[] = {
    { "featscan12", op_featscan },
    { "remove12", op_remove },
    { "rootsig12", op_rootsig },
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
