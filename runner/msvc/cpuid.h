/* The x86 and x64 D3DX runner uses the GCC cpuid.h register order. */
#pragma once
#include <intrin.h>

static inline void kz_cpuid(unsigned leaf, unsigned subleaf, unsigned *a,
                           unsigned *b, unsigned *c, unsigned *d) {
    int registers[4];
    __cpuidex(registers, (int)leaf, (int)subleaf);
    *a = (unsigned)registers[0];
    *b = (unsigned)registers[1];
    *c = (unsigned)registers[2];
    *d = (unsigned)registers[3];
}
#define __cpuid(leaf,a,b,c,d) kz_cpuid((leaf), 0, &(a), &(b), &(c), &(d))
#define __cpuid_count(leaf,subleaf,a,b,c,d) kz_cpuid((leaf), (subleaf), &(a), &(b), &(c), &(d))
