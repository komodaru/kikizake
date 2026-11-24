/* The Windows SDK C vtables return these structs through an explicit pointer.
 * Give the shared runner the value-returning spelling provided by MinGW-w64. */
#pragma once
#include <um/d3d12.h>

static inline D3D12_RESOURCE_ALLOCATION_INFO kz_alloc_info(
    ID3D12Device *dev, UINT mask, UINT count, const D3D12_RESOURCE_DESC *descs) {
    D3D12_RESOURCE_ALLOCATION_INFO info;
    ID3D12Device_GetResourceAllocationInfo(dev, &info, mask, count, descs);
    return info;
}

static inline D3D12_CPU_DESCRIPTOR_HANDLE kz_cpu_handle(ID3D12DescriptorHeap *heap) {
    D3D12_CPU_DESCRIPTOR_HANDLE handle;
    ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart(heap, &handle);
    return handle;
}

static inline D3D12_RESOURCE_DESC kz_resource_desc(ID3D12Resource *resource) {
    D3D12_RESOURCE_DESC desc;
    ID3D12Resource_GetDesc(resource, &desc);
    return desc;
}

#undef ID3D12Device_GetResourceAllocationInfo
#undef ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart
#undef ID3D12Resource_GetDesc
#define ID3D12Device_GetResourceAllocationInfo kz_alloc_info
#define ID3D12DescriptorHeap_GetCPUDescriptorHandleForHeapStart kz_cpu_handle
#define ID3D12Resource_GetDesc kz_resource_desc
