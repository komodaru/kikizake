#include "core.h"
/* ntdll's case tables, every UTF-16 unit: RtlUpcaseUnicodeChar, RtlDowncaseUnicodeChar and ntdll's
 * own towupper / towlower (found with GetProcAddress; a missing one prints "<op> none"). */

static FARPROC nt(const char *name) {
    return GetProcAddress(GetModuleHandleA("ntdll.dll"), name);
}

/* tab <fn> <block>: the function over the 256 units block * 256 .. block * 256 + 255; "tab <fn>"
 * and 256 units as 4 hex digits each. fn: up, down (Rtl*caseUnicodeChar), towupper, towlower */
static void op_tab(void) {
    if (ntok < 3) { bad = 1; return; }
    long block = num(2);
    if (bad) return;
    if (block < 0 || block > 255) { bad = 1; return; }
    const char *fn = tok[1];
    const char *name = !strcmp(fn, "up") ? "RtlUpcaseUnicodeChar" : !strcmp(fn, "down") ? "RtlDowncaseUnicodeChar"
        : !strcmp(fn, "towupper") ? "towupper" : !strcmp(fn, "towlower") ? "towlower" : NULL;
    if (!name) { bad = 1; return; }
    FARPROC p = nt(name);
    if (!p) { out("tab none\n"); return; }
    out("tab %s ", fn);
    for (int k = 0; k < 256; k++) {
        WCHAR u = (WCHAR)(block * 256 + k), r;
        if (fn[0] == 't') r = (WCHAR)((int (__cdecl *)(int))p)(u);
        else r = ((WCHAR (WINAPI *)(WCHAR))p)(u);
        out("%04x", (unsigned)r);
    }
    out("\n");
}

static const struct kz_op ops[] = { { "tab", op_tab }, { 0 } };
const struct kz_api ntcase_api = { "ntcase", ops, NULL };
