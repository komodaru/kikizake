#include "core.h"
/* ntdll's integer <-> string conversions (the first Win32 suite, not graphics): RtlCharToInteger,
 * RtlUnicodeStringToInteger, RtlIntegerToChar, RtlIntegerToUnicodeString. ntdll is in every
 * process; the functions are found with GetProcAddress and a missing one prints "<op> none".
 * Strings are given as hex: bytes for ANSI, 4 hex digits (big-endian, as written) per UTF-16 unit. */

typedef struct { USHORT Length, MaximumLength; PWSTR Buffer; } kz_ustr;
typedef struct { USHORT Length, MaximumLength; PCHAR Buffer; } kz_astr;

static FARPROC nt(const char *name) {
    return GetProcAddress(GetModuleHandleA("ntdll.dll"), name);
}
static int hexval(char c) {
    return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : -1;
}
/* token i as bytes ("-" is none): their count, or -1 (bad) */
static int hexbytes(int i, unsigned char *b, int max) {
    if (i >= ntok) { bad = 1; return -1; }
    if (!strcmp(tok[i], "-")) return 0;
    int n = (int)strlen(tok[i]);
    if (n % 2 || n / 2 > max) { bad = 1; return -1; }
    for (int k = 0; k < n / 2; k++) {
        int h = hexval(tok[i][2 * k]), l = hexval(tok[i][2 * k + 1]);
        if (h < 0 || l < 0) { bad = 1; return -1; }
        b[k] = (unsigned char)(h << 4 | l);
    }
    return n / 2;
}

/* a2i <base> <hex bytes>: RtlCharToInteger of the bytes and a NUL; "a2i <status> <value>", the
 * value 0xdeadbeef where the call left it */
static void op_a2i(void) {
    unsigned char b[80];
    unsigned long base = (unsigned long)num(1);
    int n = hexbytes(2, b, (int)sizeof b - 1);
    if (bad) return;
    b[n] = 0;
    LONG (WINAPI *f)(const char *, ULONG, ULONG *) = (void *)nt("RtlCharToInteger");
    if (!f) { out("a2i none\n"); return; }
    ULONG v = 0xdeadbeef;
    LONG st = f((const char *)b, base, &v);
    out("a2i %08lx %08lx\n", (unsigned long)st, (unsigned long)v);
}

/* u2i <base> <length> <hex units>: RtlUnicodeStringToInteger of a UNICODE_STRING of that Length
 * (bytes; at most the units given, an odd one allowed) over the units; "u2i <status> <value>" */
static void op_u2i(void) {
    unsigned char b[158];
    WCHAR w[80];
    unsigned long base = (unsigned long)num(1);
    long len = num(2);
    int n = hexbytes(3, b, (int)sizeof b);
    if (bad) return;
    if (n % 2 || len < 0 || len > n + 1 || len > 0xffff) { bad = 1; return; }
    for (int k = 0; k < n / 2; k++) w[k] = (WCHAR)(b[2 * k] << 8 | b[2 * k + 1]);
    if (len > n) w[n / 2] = 0; /* (an odd length past the units reads half of this) */
    kz_ustr s = { (USHORT)len, (USHORT)(n + 2), w };
    LONG (WINAPI *f)(kz_ustr *, ULONG, ULONG *) = (void *)nt("RtlUnicodeStringToInteger");
    if (!f) { out("u2i none\n"); return; }
    ULONG v = 0xdeadbeef;
    LONG st = f(&s, base, &v);
    out("u2i %08lx %08lx\n", (unsigned long)st, (unsigned long)v);
}

/* i2a <value> <base> <length>: RtlIntegerToChar into 40 bytes of 0xcc (|length| <= 40);
 * "i2a <status>" and the 40 bytes */
static void op_i2a(void) {
    unsigned char b[40];
    unsigned long v = (unsigned long)num(1), base = (unsigned long)num(2);
    long len = num(3);
    if (bad) return;
    if (len > 40 || len < -40) { bad = 1; return; }
    memset(b, 0xcc, sizeof b);
    LONG (WINAPI *f)(ULONG, ULONG, LONG, PCHAR) = (void *)nt("RtlIntegerToChar");
    if (!f) { out("i2a none\n"); return; }
    LONG st = f(v, base, len, (PCHAR)b);
    out("i2a %08lx ", (unsigned long)st);
    out_hex(b, sizeof b);
}

/* i2u <value> <base> <maximum> [null]: RtlIntegerToUnicodeString into a UNICODE_STRING
 * {Length 0x5555, MaximumLength maximum (<= 80), Buffer 80 bytes of 0xcc, or NULL};
 * "i2u <status> <Length> <MaximumLength>" and the 80 bytes */
static void op_i2u(void) {
    unsigned char b[80];
    unsigned long v = (unsigned long)num(1), base = (unsigned long)num(2);
    long max = num(3);
    int null = is(4, "null");
    if (bad) return;
    if (max < 0 || max > 80 || (ntok > 4 && !null)) { bad = 1; return; }
    memset(b, 0xcc, sizeof b);
    kz_ustr s = { 0x5555, (USHORT)max, null ? NULL : (PWSTR)b };
    LONG (WINAPI *f)(ULONG, ULONG, kz_ustr *) = (void *)nt("RtlIntegerToUnicodeString");
    if (!f) { out("i2u none\n"); return; }
    LONG st = f(v, base, &s);
    out("i2u %08lx %04x %04x %s ", (unsigned long)st, s.Length, s.MaximumLength, s.Buffer ? "buf" : "null");
    out_hex(b, sizeof b);
}

/* a2u <length> <maximum> <hex bytes> [null]: RtlAnsiStringToUnicodeString(dst, src, FALSE) of an
 * ANSI_STRING {length, length + 1, the bytes} (ASCII only: the ANSI code page is the system's)
 * into {0x5555, maximum (<= 80), 80 bytes of 0xcc or NULL}; printed as i2u */
static void op_a2u(void) {
    unsigned char a[80], b[80];
    long len = num(1), max = num(2);
    int n = hexbytes(3, a, (int)sizeof a - 1);
    int null = is(4, "null");
    if (bad) return;
    if (max < 0 || max > 80 || len < 0 || len > n || (ntok > 4 && !null)) { bad = 1; return; }
    for (int k = 0; k < n; k++) if (a[k] >= 0x80) { bad = 1; return; }
    a[n] = 0;
    memset(b, 0xcc, sizeof b);
    kz_astr src = { (USHORT)len, (USHORT)(len + 1), (PCHAR)a };
    kz_ustr s = { 0x5555, (USHORT)max, null ? NULL : (PWSTR)b };
    LONG (WINAPI *f)(kz_ustr *, kz_astr *, BOOLEAN) = (void *)nt("RtlAnsiStringToUnicodeString");
    if (!f) { out("a2u none\n"); return; }
    LONG st = f(&s, &src, FALSE);
    out("a2u %08lx %04x %04x %s ", (unsigned long)st, s.Length, s.MaximumLength, s.Buffer ? "buf" : "null");
    out_hex(b, sizeof b);
}

static const struct kz_op ops[] = {
    { "a2i", op_a2i }, { "u2i", op_u2i }, { "i2a", op_i2a }, { "i2u", op_i2u }, { "a2u", op_a2u }, { 0 }
};
const struct kz_api ntstr_api = { "ntstr", ops, NULL };
