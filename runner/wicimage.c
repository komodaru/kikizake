#include "core.h"
#include <wincodec.h>
/* WIC bitmap sources other than the format converter (runner/wic.c): bitmaps in memory, scalers,
 * flip/rotators, clippers, predefined palettes, color transforms, the decoders and the encoders.
 *
 * One current source (`cur`): wbits/wdecode+wframe make it, wscale/wflip/wclip/wconv/wcolor wrap
 * it, winfo/wpixels/wsrcpal read it, wencode writes it into a file in memory that wbytes prints
 * and wredecode decodes. Every op prints one line first ("<op> <hr>" and what it found). */

static IWICImagingFactory *fac;
static IWICBitmapSource *cur;
static IWICBitmapDecoder *dec;
static IWICStream *dstream;
static unsigned char *dbytes, *ebytes; /* the bytes being decoded, the last encoded file */
static int nebytes;

static const struct { const char *n; GUID g; int bits; } fmts[] = {
    { "bw", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x05 } }, 1 },
    { "gray2", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x06 } }, 2 },
    { "gray4", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x07 } }, 4 },
    { "gray8", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x08 } }, 8 },
    { "gray16", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0b } }, 16 },
    { "bgr555", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x09 } }, 16 },
    { "bgr565", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0a } }, 16 },
    { "bgr24", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0c } }, 24 },
    { "rgb24", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0d } }, 24 },
    { "bgr32", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0e } }, 32 },
    { "bgra", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x0f } }, 32 },
    { "pbgra", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x10 } }, 32 },
    { "rgb32", { 0xd98c6b95, 0x3efe, 0x47d6, { 0xbb, 0x25, 0xeb, 0x17, 0x48, 0xab, 0x0c, 0xf1 } }, 32 },
    { "rgba", { 0xf5c7ad2d, 0x6a8d, 0x43dd, { 0xa7, 0xa8, 0xa2, 0x99, 0x35, 0x26, 0x1a, 0xe9 } }, 32 },
    { "prgba", { 0x3cc4a650, 0xa527, 0x4d37, { 0xa9, 0x16, 0x31, 0x42, 0xc7, 0xeb, 0xed, 0xba } }, 32 },
    { "rgb48", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x15 } }, 48 },
    { "rgba64", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x16 } }, 64 },
    { "cmyk", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1c } }, 32 },
    { "alpha8", { 0xe6cd0116, 0xeeba, 0x4161, { 0xaa, 0x85, 0x27, 0xdd, 0x9f, 0xb3, 0xa8, 0x95 } }, 8 },
    { "idx1", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x01 } }, 1 },
    { "idx2", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x02 } }, 2 },
    { "idx4", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x03 } }, 4 },
    { "idx8", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x04 } }, 8 },
    /* (formats a decoder or a scaler may answer with; not made by wbits) */
    { "bgr48", { 0xe605a384, 0xb468, 0x46ce, { 0xbb, 0x2e, 0x36, 0xf1, 0x80, 0xe6, 0x43, 0x13 } }, 48 },
    { "bgra64", { 0x1562ff7c, 0xd352, 0x46f9, { 0x97, 0x9e, 0x42, 0x97, 0x6b, 0x79, 0x22, 0x46 } }, 64 },
    { "prgba64", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x17 } }, 64 },
    { "pbgra64", { 0x8c518e8e, 0xa4ec, 0x468b, { 0xae, 0x70, 0xc9, 0xa3, 0x5a, 0x9c, 0x55, 0x30 } }, 64 },
    { "bgr101010", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x14 } }, 32 },
    { "grayfloat", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x11 } }, 32 },
    { "rgbafloat", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x19 } }, 128 },
    { "prgbafloat", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1a } }, 128 },
    { "rgbfloat", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1b } }, 128 },
    { "rgba64fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1d } }, 64 },
    { "bgra64fixed", { 0x356de33c, 0x54d2, 0x4a23, { 0xbb, 0x04, 0x9b, 0x7b, 0xf9, 0xb1, 0xd4, 0x2d } }, 64 },
    { "rgba64half", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3a } }, 64 },
    { "rgb96float", { 0xe3fed78f, 0xe8db, 0x4acf, { 0x84, 0xc1, 0xe9, 0x7f, 0x61, 0x36, 0xb3, 0x27 } }, 96 },
    { "cmyk64", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1f } }, 64 },
    { "cmyka40", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x2c } }, 40 },
    { "cmyka80", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x2d } }, 80 },
    /* (fixed point and half formats, as conversion targets) */
    { "grayfix16", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x13 } }, 16 },
    { "grayhalf", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3e } }, 16 },
    { "grayfix32", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3f } }, 32 },
    { "rgb48fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x12 } }, 48 },
    { "rgb48half", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x3b } }, 48 },
    { "rgb64fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x40 } }, 64 },
    { "rgb64half", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x42 } }, 64 },
    { "rgba128fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x1e } }, 128 },
    { "rgb128fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x41 } }, 128 },
    { "rgb96fixed", { 0x6fddc324, 0x4e03, 0x4bfe, { 0xb1, 0x85, 0x3d, 0x77, 0x76, 0x8d, 0xc9, 0x18 } }, 96 },
};
#define NFMTS (int)(sizeof fmts / sizeof *fmts)
#define NMADE 23 /* the formats wbits makes; from 19 on indexed */

static const struct { const char *n; GUID g; } containers[] = {
    { "bmp", { 0x0af1d87e, 0xfcfe, 0x4188, { 0xbd, 0xeb, 0xa7, 0x90, 0x64, 0x71, 0xcb, 0xe3 } } },
    { "png", { 0x1b7cfaf4, 0x713f, 0x473c, { 0xbb, 0xcd, 0x61, 0x37, 0x42, 0x5f, 0xae, 0xaf } } },
    { "ico", { 0xa3a860c4, 0x338f, 0x4c17, { 0x91, 0x9a, 0xfb, 0xa4, 0xb5, 0x62, 0x8f, 0x21 } } },
    { "jpeg", { 0x19e4a5aa, 0x5662, 0x4fc5, { 0xa0, 0xc0, 0x17, 0x58, 0x02, 0x8e, 0x10, 0x57 } } },
    { "tiff", { 0x163bcc30, 0xe2e9, 0x4f0b, { 0x96, 0x1d, 0xa3, 0xe9, 0xfd, 0xb7, 0x88, 0xa3 } } },
    { "gif", { 0x1f8a5601, 0x7d4d, 0x4cbd, { 0x9c, 0x82, 0x1b, 0xc8, 0xd4, 0xee, 0xb9, 0xa5 } } },
    { "wmp", { 0x57a37caa, 0x367a, 0x4540, { 0x91, 0x6b, 0xf1, 0x83, 0xc5, 0x09, 0x3a, 0x4b } } },
    { "dds", { 0x9967cb95, 0x2e85, 0x4ac8, { 0x8c, 0xa2, 0x83, 0xd7, 0xcc, 0xd4, 0x25, 0xc9 } } },
    { "heif", { 0xe1e62521, 0x6787, 0x405b, { 0xa3, 0x39, 0x50, 0x07, 0x15, 0xb5, 0x76, 0x3f } } },
    { "webp", { 0xe094b0e2, 0x67f2, 0x45b3, { 0xb0, 0xea, 0x11, 0x53, 0x37, 0xca, 0x7c, 0xf3 } } },
};
#define NCONT (int)(sizeof containers / sizeof *containers)

static int init(void) {
    if (!fac) {
        CoInitializeEx(NULL, COINIT_MULTITHREADED);
        if (FAILED(CoCreateInstance(&CLSID_WICImagingFactory, NULL, CLSCTX_INPROC_SERVER, &IID_IWICImagingFactory, (void **)&fac))) fac = NULL;
    }
    if (!fac) out("%s none\n", tok[0]);
    return fac != NULL;
}

static int fmt_at(int i) {
    for (int f = 0; f < NFMTS; f++)
        if (is(i, fmts[f].n)) return f;
    bad = 1;
    return 0;
}
static void put_fmt(const GUID *g) {
    for (int f = 0; f < NFMTS; f++)
        if (IsEqualGUID(g, &fmts[f].g)) { out("%s", fmts[f].n); return; }
    out("{%08lx-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x}", g->Data1, g->Data2, g->Data3, g->Data4[0], g->Data4[1], g->Data4[2], g->Data4[3], g->Data4[4], g->Data4[5], g->Data4[6], g->Data4[7]);
}
static int fmt_bits(const GUID *g) {
    for (int f = 0; f < NFMTS; f++)
        if (IsEqualGUID(g, &fmts[f].g)) return fmts[f].bits;
    return 0;
}
static int cont_at(int i) {
    for (int c = 0; c < NCONT; c++)
        if (is(i, containers[c].n)) return c;
    bad = 1;
    return 0;
}

static void set_cur(IWICBitmapSource *s) {
    if (cur) IWICBitmapSource_Release(cur);
    cur = s;
}
/* (an op on no current source, as after a call that failed: "<op> -") */
static int need_cur(void) {
    if (!cur) out("%s -\n", tok[0]);
    return cur != NULL;
}

/* the executor's palette (runner/wic.c's): 16 colors, entry 5 with alpha 0x40 */
static IWICPalette *kz_palette(void) {
    IWICPalette *p = NULL;
    if (FAILED(IWICImagingFactory_CreatePalette(fac, &p)) || !p) return NULL;
    WICColor c[16];
    for (int i = 0; i < 16; i++) c[i] = (i == 5 ? 0x40000000u : 0xff000000u) | (unsigned)(i * 17) << 16 | (unsigned)(255 - i * 13) << 8 | (unsigned)((i * 71) & 255);
    IWICPalette_InitializeCustom(p, c, 16);
    return p;
}

/* wbits <fmt> <w> <h> <seed|ramp>: a bitmap in memory, rows (w * bits + 7) / 8 bytes from
 * xorshift32 or byte i = i mod 256 (runner/wic.c's convert), the palette for indexed formats.
 * Prints "wbits <hr>". */
static void op_bits(void) {
    int f = fmt_at(1);
    long w = num(2), h = num(3);
    int ramp = is(4, "ramp");
    unsigned seed = ramp ? 0 : (unsigned)num(4);
    if (bad || f >= NMADE || w < 1 || h < 1 || w > 1024 || h > 1024) { bad = 1; return; }
    if (!init()) return;
    int ss = (w * fmts[f].bits + 7) / 8;
    unsigned char *px = malloc(ss * h);
    unsigned x = seed ? seed : 0x9e3779b9;
    for (int i = 0; i < ss * h; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        px[i] = ramp ? (unsigned char)i : (unsigned char)x;
    }
    IWICBitmap *b = NULL;
    HRESULT hr = IWICImagingFactory_CreateBitmapFromMemory(fac, w, h, &fmts[f].g, ss, ss * h, px, &b);
    free(px);
    out("wbits %08lx\n", hr);
    if (FAILED(hr)) return;
    if (f >= 19) {
        IWICPalette *p = kz_palette();
        if (p) { IWICBitmap_SetPalette(b, p); IWICPalette_Release(p); }
    }
    set_cur((IWICBitmapSource *)b);
}

/* wscale <w> <h> <nn|linear|cubic|fant|hqcubic>: "wscale <hr>" */
static void op_scale(void) {
    static const char *modes[] = { "nn", "linear", "cubic", "fant", "hqcubic" };
    long w = num(1), h = num(2);
    int m = -1;
    for (int i = 0; i < 5; i++)
        if (is(3, modes[i])) m = i;
    if (bad || m < 0) { bad = 1; return; }
    if (!need_cur()) return;
    IWICBitmapScaler *s = NULL;
    HRESULT hr = IWICImagingFactory_CreateBitmapScaler(fac, &s);
    if (SUCCEEDED(hr)) hr = IWICBitmapScaler_Initialize(s, cur, (UINT)w, (UINT)h, (WICBitmapInterpolationMode)m);
    out("wscale %08lx\n", hr);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)s);
    else if (s) IWICBitmapScaler_Release(s);
}

/* wflip <WICBitmapTransformOptions>: "wflip <hr>" */
static void op_flip(void) {
    long o = num(1);
    if (bad || !need_cur()) return;
    IWICBitmapFlipRotator *r = NULL;
    HRESULT hr = IWICImagingFactory_CreateBitmapFlipRotator(fac, &r);
    if (SUCCEEDED(hr)) hr = IWICBitmapFlipRotator_Initialize(r, cur, (WICBitmapTransformOptions)o);
    out("wflip %08lx\n", hr);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)r);
    else if (r) IWICBitmapFlipRotator_Release(r);
}

/* wclip <x> <y> <w> <h>: "wclip <hr>" */
static void op_clip(void) {
    WICRect rc = { num(1), num(2), num(3), num(4) };
    if (bad || !need_cur()) return;
    IWICBitmapClipper *c = NULL;
    HRESULT hr = IWICImagingFactory_CreateBitmapClipper(fac, &c);
    if (SUCCEEDED(hr)) hr = IWICBitmapClipper_Initialize(c, cur, &rc);
    out("wclip %08lx\n", hr);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)c);
    else if (c) IWICBitmapClipper_Release(c);
}

/* wconv <fmt>: a format converter (no dither, the executor's palette for indexed): "wconv <hr>" */
static void op_conv(void) {
    int f = fmt_at(1);
    if (bad || !need_cur()) return;
    IWICFormatConverter *c = NULL;
    IWICPalette *p = f >= 19 && f < NMADE ? kz_palette() : NULL;
    HRESULT hr = IWICImagingFactory_CreateFormatConverter(fac, &c);
    if (SUCCEEDED(hr)) hr = IWICFormatConverter_Initialize(c, cur, &fmts[f].g, WICBitmapDitherTypeNone, p, 0, p ? WICBitmapPaletteTypeCustom : WICBitmapPaletteTypeMedianCut);
    if (p) IWICPalette_Release(p);
    out("wconv %08lx\n", hr);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)c);
    else if (c) IWICFormatConverter_Release(c);
}

static IWICColorContext *exif_ctx(long n, HRESULT *hr) {
    IWICColorContext *c = NULL;
    *hr = IWICImagingFactory_CreateColorContext(fac, &c);
    if (SUCCEEDED(*hr)) *hr = IWICColorContext_InitializeFromExifColorSpace(c, (UINT)n);
    return c;
}

/* wcolor <exif from> <exif to> <fmt>: a color transform between two EXIF color spaces (1 sRGB,
 * 2 Adobe RGB): "wcolor <hr of the contexts> <hr>" */
static void op_color(void) {
    long a = num(1), b = num(2);
    int f = fmt_at(3);
    if (bad || !need_cur()) return;
    HRESULT h1, h2, hr;
    IWICColorContext *ca = exif_ctx(a, &h1), *cb = exif_ctx(b, &h2);
    IWICColorTransform *t = NULL;
    hr = IWICImagingFactory_CreateColorTransformer(fac, &t);
    if (SUCCEEDED(hr)) hr = IWICColorTransform_Initialize(t, cur, ca, cb, &fmts[f].g);
    out("wcolor %08lx %08lx %08lx\n", h1, h2, hr);
    if (ca) IWICColorContext_Release(ca);
    if (cb) IWICColorContext_Release(cb);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)t);
    else if (t) IWICColorTransform_Release(t);
}

/* wctx <exif n>: a color context from an EXIF color space: "wctx <hr> type <t> exif <hr> <n>" */
static void op_ctx(void) {
    long n = num(1);
    if (bad || !init()) return;
    HRESULT hr;
    IWICColorContext *c = exif_ctx(n, &hr);
    WICColorContextType t = 0;
    UINT e = 0;
    if (c) IWICColorContext_GetType(c, &t);
    HRESULT he = c ? IWICColorContext_GetExifColorSpace(c, &e) : E_FAIL;
    out("wctx %08lx type %d exif %08lx %u\n", hr, (int)t, he, e);
    if (c) IWICColorContext_Release(c);
}

/* winfo: "winfo <hr> <w> <h> <fmt> <dpi x> <dpi y>" of the current source */
static void op_info(void) {
    if (!need_cur()) return;
    UINT w = 0, h = 0;
    double dx = 0, dy = 0;
    GUID g;
    memset(&g, 0, sizeof g);
    HRESULT hr = IWICBitmapSource_GetSize(cur, &w, &h);
    HRESULT h2 = IWICBitmapSource_GetPixelFormat(cur, &g);
    HRESULT h3 = IWICBitmapSource_GetResolution(cur, &dx, &dy);
    out("winfo %08lx %u %u ", FAILED(hr) ? hr : FAILED(h2) ? h2 : h3, w, h);
    put_fmt(&g);
    out(" %.3f %.3f\n", dx, dy);
}

/* wpixels [x y w h [stride size]]: CopyPixels of the current source (all of it, a rectangle, or a
 * rectangle into a buffer of that stride and size): "wpixels <hr> <bits>" then each row's bytes
 * (the bits past the row's last pixel 0) */
static void op_pixels(void) {
    if (!need_cur()) return;
    UINT w = 0, h = 0;
    GUID g;
    IWICBitmapSource_GetSize(cur, &w, &h);
    IWICBitmapSource_GetPixelFormat(cur, &g);
    int bits = fmt_bits(&g);
    WICRect rc = { 0, 0, w, h };
    int whole = ntok == 1;
    if (!whole) { rc.X = num(1); rc.Y = num(2); rc.Width = num(3); rc.Height = num(4); }
    if (bad) return;
    if (!bits) { out("wpixels unknown\n"); return; }
    long rw = rc.Width > 0 ? rc.Width : 0, rh = rc.Height > 0 ? rc.Height : 0;
    if (rw > 4096 || rh > 4096) { rw = 1; rh = 1; }
    long stride = (rw * bits + 7) / 8, size = stride * rh;
    if (ntok == 7) { stride = num(5); size = num(6); }
    if (bad || stride < 0 || size < 0 || size > 1 << 24) { bad = 1; return; }
    unsigned char *b = calloc(size + 16, 1);
    HRESULT hr = IWICBitmapSource_CopyPixels(cur, whole ? NULL : &rc, (UINT)stride, (UINT)size, b);
    out("wpixels %08lx %d\n", hr, bits);
    if (SUCCEEDED(hr)) {
        long n = (rw * bits + 7) / 8;
        int tail = (rw * bits) % 8;
        for (long y = 0; y < rh && y * stride + n <= size; y++) {
            unsigned char *r = b + y * stride;
            if (tail) r[n - 1] &= (unsigned char)(0xff00 >> tail);
            out_hex(r, n);
        }
    }
    free(b);
}

/* wsteps <grayfloat|rgbafloat> <gray16|gray8|rgba64|bgra>: where a float of 0..1 steps into each
 * level of the target: per level n (1 up to the largest) the smallest float (its bits, f32 little
 * endian, 64 a line) whose conversion is n or more, by bisection over the bits of 0..1 (all levels
 * at once, one bitmap of them a round). For rgbafloat red, green and blue all the float, alpha 1,
 * the red of the target read. "wsteps <hr> <levels> <level of 1.0> <pixels whose green or blue
 * differs from red at the end>" */
static void steps_conv(int from, int to, const unsigned *v, int n, unsigned *lv, int *odd, HRESULT *hr) {
    int ch = fmts[from].bits == 128 ? 4 : 1;
    int W = 256, H = (n + W - 1) / W;
    float *px = calloc((size_t)W * H * ch, sizeof(float));
    for (int i = 0; i < W * H; i++) {
        float x;
        unsigned b = i < n ? v[i] : 0x3f800000u;
        memcpy(&x, &b, 4);
        for (int k = 0; k < ch; k++) px[i * ch + k] = k == 3 ? 1.0f : x;
    }
    IWICBitmap *bm = NULL;
    IWICFormatConverter *c = NULL;
    int tb = fmts[to].bits;
    int stride = (W * tb + 7) / 8;
    unsigned char *o = calloc((size_t)stride * H, 1);
    *hr = IWICImagingFactory_CreateBitmapFromMemory(fac, W, H, &fmts[from].g, W * ch * 4, W * H * ch * 4, (BYTE *)px, &bm);
    if (SUCCEEDED(*hr)) *hr = IWICImagingFactory_CreateFormatConverter(fac, &c);
    if (SUCCEEDED(*hr)) *hr = IWICFormatConverter_Initialize(c, (IWICBitmapSource *)bm, &fmts[to].g, WICBitmapDitherTypeNone, NULL, 0, WICBitmapPaletteTypeMedianCut);
    if (SUCCEEDED(*hr)) *hr = IWICFormatConverter_CopyPixels(c, NULL, stride, stride * H, o);
    *odd = 0;
    for (int i = 0; i < n && SUCCEEDED(*hr); i++) {
        const unsigned char *q = o + (i / W) * stride + (i % W) * (tb / 8);
        unsigned r, g, b;
        if (tb == 16) r = g = b = q[0] | q[1] << 8;
        else if (tb == 8) r = g = b = q[0];
        else if (tb == 64) { r = q[0] | q[1] << 8; g = q[2] | q[3] << 8; b = q[4] | q[5] << 8; }
        else { b = q[0]; g = q[1]; r = q[2]; }
        lv[i] = r;
        if (g != r || b != r) (*odd)++;
    }
    if (c) IWICFormatConverter_Release(c);
    if (bm) IWICBitmap_Release(bm);
    free(px);
    free(o);
}
static void op_steps(void) {
    int from = fmt_at(1), to = fmt_at(2);
    if (bad || !(is(1, "grayfloat") || is(1, "rgbafloat")) || !(is(2, "gray16") || is(2, "gray8") || is(2, "rgba64") || is(2, "bgra"))) { bad = 1; return; }
    if (!init()) return;
    int n = fmts[to].bits == 8 || fmts[to].bits == 32 ? 255 : 65535;
    unsigned *lo = calloc(n, 4), *hi = calloc(n, 4), *mid = calloc(n, 4), *lv = calloc(n, 4);
    unsigned one = 0x3f800000u, top = 0;
    int odd = 0;
    HRESULT hr;
    steps_conv(from, to, &one, 1, lv, &odd, &hr);
    top = lv[0];
    for (int i = 0; i < n; i++) { lo[i] = 0; hi[i] = one; }
    for (int round = 0; round < 32 && SUCCEEDED(hr); round++) {
        int open = 0;
        for (int i = 0; i < n; i++) { mid[i] = lo[i] + (hi[i] - lo[i]) / 2; open += hi[i] - lo[i] > 1; }
        if (!open) break;
        steps_conv(from, to, mid, n, lv, &odd, &hr);
        for (int i = 0; i < n && SUCCEEDED(hr); i++) {
            if (hi[i] - lo[i] <= 1) continue;
            if (lv[i] >= (unsigned)(i + 1)) hi[i] = mid[i]; else lo[i] = mid[i];
        }
    }
    if (SUCCEEDED(hr)) steps_conv(from, to, hi, n, lv, &odd, &hr);
    out("wsteps %08lx %d %u %d\n", hr, n, top, odd);
    if (SUCCEEDED(hr))
        for (int i = 0; i < n; i += 64) out_hex((const unsigned char *)(hi + i), (n - i < 64 ? n - i : 64) * 4);
    free(lo); free(hi); free(mid); free(lv);
}

static void put_palette(const char *op, IWICPalette *p, HRESULT hr) {
    WICColor c[256];
    UINT n = 0;
    WICBitmapPaletteType t = 0;
    BOOL bw = 0, gray = 0, alpha = 0;
    if (SUCCEEDED(hr)) hr = IWICPalette_GetColors(p, 256, c, &n);
    if (SUCCEEDED(hr)) {
        IWICPalette_GetType(p, &t);
        IWICPalette_IsBlackWhite(p, &bw);
        IWICPalette_IsGrayscale(p, &gray);
        IWICPalette_HasAlpha(p, &alpha);
    }
    out("%s %08lx type %d count %u bw %d gray %d alpha %d\n", op, hr, (int)t, n, bw, gray, alpha);
    if (SUCCEEDED(hr)) {
        for (UINT i = 0; i < n; i++) out("%08x", c[i]);
        out("\n");
    }
}

/* wpalette <WICBitmapPaletteType> <transparent 0|1>: InitializePredefined:
 * "wpalette <hr> type <t> count <n> bw <b> gray <g> alpha <a>" then the colors (AARRGGBB) */
static void op_palette(void) {
    long t = num(1), a = num(2);
    if (bad || !init()) return;
    IWICPalette *p = NULL;
    HRESULT hr = IWICImagingFactory_CreatePalette(fac, &p);
    if (SUCCEEDED(hr)) hr = IWICPalette_InitializePredefined(p, (WICBitmapPaletteType)t, (BOOL)a);
    put_palette("wpalette", p, hr);
    if (p) IWICPalette_Release(p);
}

/* wsrcpal: CopyPalette of the current source, as wpalette prints it */
static void op_srcpal(void) {
    if (!need_cur()) return;
    IWICPalette *p = NULL;
    HRESULT hr = IWICImagingFactory_CreatePalette(fac, &p);
    if (SUCCEEDED(hr)) hr = IWICBitmapSource_CopyPalette(cur, p);
    put_palette("wsrcpal", p, hr);
    if (p) IWICPalette_Release(p);
}

static void close_decoder(void) {
    if (dec) IWICBitmapDecoder_Release(dec);
    if (dstream) IWICStream_Release(dstream);
    free(dbytes);
    dec = NULL; dstream = NULL; dbytes = NULL;
}

/* decodes n bytes at p (copied): "<op> <hr> <container> frames <n>" */
static void decode(const char *op, const unsigned char *p, int n) {
    close_decoder();
    dbytes = malloc(n + 1);
    memcpy(dbytes, p, n);
    HRESULT hr = IWICImagingFactory_CreateStream(fac, &dstream);
    if (SUCCEEDED(hr)) hr = IWICStream_InitializeFromMemory(dstream, dbytes, n);
    if (SUCCEEDED(hr)) hr = IWICImagingFactory_CreateDecoderFromStream(fac, (IStream *)dstream, NULL, WICDecodeMetadataCacheOnDemand, &dec);
    GUID g;
    UINT frames = 0;
    const char *cn = "-";
    if (SUCCEEDED(hr)) {
        if (SUCCEEDED(IWICBitmapDecoder_GetContainerFormat(dec, &g)))
            for (int c = 0; c < NCONT; c++)
                if (IsEqualGUID(&g, &containers[c].g)) cn = containers[c].n;
        IWICBitmapDecoder_GetFrameCount(dec, &frames);
    }
    out("%s %08lx %s frames %u\n", op, hr, cn, frames);
    if (FAILED(hr)) close_decoder();
}

/* wdecode <hex bytes>: a decoder for a file in memory (the decoder WIC picks for it) */
static void op_decode(void) {
    if (ntok != 2 || strlen(tok[1]) % 2) { bad = 1; return; }
    if (!init()) return;
    int n = strlen(tok[1]) / 2;
    unsigned char *b = malloc(n + 1);
    for (int i = 0; i < n; i++) {
        char h[3] = { tok[1][2 * i], tok[1][2 * i + 1], 0 };
        char *e;
        b[i] = (unsigned char)strtoul(h, &e, 16);
        if (*e) bad = 1;
    }
    if (!bad) decode("wdecode", b, n);
    free(b);
}

/* wframe <i>: the decoder's frame i becomes the current source: "wframe <hr>" */
static void op_frame(void) {
    long i = num(1);
    if (bad) return;
    if (!dec) { out("wframe -\n"); return; }
    IWICBitmapFrameDecode *f = NULL;
    HRESULT hr = IWICBitmapDecoder_GetFrame(dec, (UINT)i, &f);
    out("wframe %08lx\n", hr);
    if (SUCCEEDED(hr)) set_cur((IWICBitmapSource *)f);
}

/* encoder options: name -> VARTYPE */
static const struct { const char *n; VARTYPE t; } options[] = {
    { "ImageQuality", VT_R4 }, { "CompressionQuality", VT_R4 }, { "Lossless", VT_BOOL },
    { "BitmapTransform", VT_UI1 }, { "InterlaceOption", VT_BOOL }, { "FilterOption", VT_UI1 },
    { "TiffCompressionMethod", VT_UI1 }, { "JpegYCrCbSubsampling", VT_UI1 }, { "SuppressApp0", VT_BOOL },
    { "EnableV5Header32bppBGRA", VT_BOOL }, { "Quality", VT_UI1 }, { "Overlap", VT_UI1 },
    { "Subsampling", VT_UI1 }, { "AlphaQuality", VT_UI1 }, { "HeifCompressionMethod", VT_UI1 },
};

/* wencode <container> [fmt] [<option>=<value> ...]: the current source written by that encoder
 * into memory (one frame, SetSize from the source, SetPixelFormat of the source's format or the
 * one named, the source's palette for indexed formats, WriteSource of all of it):
 * "wencode <hr> <the format SetPixelFormat answered> <step that failed or ok>" */
static void op_encode(void) {
    int c = cont_at(1);
    int first = 2, fmt = -1;
    if (ntok > 2 && !strchr(tok[2], '=')) { fmt = fmt_at(2); first = 3; }
    if (bad || !need_cur()) return;
    free(ebytes);
    ebytes = NULL;
    nebytes = 0;
    IWICBitmapEncoder *e = NULL;
    IWICBitmapFrameEncode *fe = NULL;
    IPropertyBag2 *bag = NULL;
    IStream *st = NULL;
    const char *step = "create";
    GUID g;
    memset(&g, 0, sizeof g);
    HRESULT hr = IWICImagingFactory_CreateEncoder(fac, &containers[c].g, NULL, &e);
    if (SUCCEEDED(hr)) { step = "stream"; hr = CreateStreamOnHGlobal(NULL, TRUE, &st); }
    if (SUCCEEDED(hr)) { step = "init"; hr = IWICBitmapEncoder_Initialize(e, st, WICBitmapEncoderNoCache); }
    if (SUCCEEDED(hr)) { step = "frame"; hr = IWICBitmapEncoder_CreateNewFrame(e, &fe, &bag); }
    for (int i = first; i < ntok && SUCCEEDED(hr); i++) {
        char name[64], *eq = strchr(tok[i], '=');
        int o = -1;
        if (!eq || eq - tok[i] >= 63) { bad = 1; break; }
        memcpy(name, tok[i], eq - tok[i]);
        name[eq - tok[i]] = 0;
        for (int k = 0; k < (int)(sizeof options / sizeof *options); k++)
            if (!strcmp(name, options[k].n)) o = k;
        if (o < 0) { bad = 1; break; }
        WCHAR wn[64];
        MultiByteToWideChar(CP_ACP, 0, name, -1, wn, 64);
        PROPBAG2 pb;
        memset(&pb, 0, sizeof pb);
        pb.pstrName = wn;
        VARIANT v;
        memset(&v, 0, sizeof v);
        v.vt = options[o].t;
        if (v.vt == VT_R4) v.fltVal = (float)atof(eq + 1);
        else if (v.vt == VT_BOOL) v.boolVal = atoi(eq + 1) ? VARIANT_TRUE : VARIANT_FALSE;
        else v.bVal = (BYTE)atoi(eq + 1);
        step = "option";
        hr = IPropertyBag2_Write(bag, 1, &pb, &v);
    }
    if (bad) goto done;
    UINT w = 0, h = 0;
    IWICBitmapSource_GetSize(cur, &w, &h);
    if (SUCCEEDED(hr)) { step = "frameinit"; hr = IWICBitmapFrameEncode_Initialize(fe, bag); }
    if (SUCCEEDED(hr)) { step = "size"; hr = IWICBitmapFrameEncode_SetSize(fe, w, h); }
    if (fmt >= 0) g = fmts[fmt].g;
    else IWICBitmapSource_GetPixelFormat(cur, &g);
    if (SUCCEEDED(hr)) { step = "format"; hr = IWICBitmapFrameEncode_SetPixelFormat(fe, &g); }
    if (SUCCEEDED(hr)) {
        IWICPalette *p = NULL;
        if (SUCCEEDED(IWICImagingFactory_CreatePalette(fac, &p)) && SUCCEEDED(IWICBitmapSource_CopyPalette(cur, p)))
            IWICBitmapFrameEncode_SetPalette(fe, p);
        if (p) IWICPalette_Release(p);
        step = "write";
        hr = IWICBitmapFrameEncode_WriteSource(fe, cur, NULL);
    }
    if (SUCCEEDED(hr)) { step = "commit"; hr = IWICBitmapFrameEncode_Commit(fe); }
    if (SUCCEEDED(hr)) hr = IWICBitmapEncoder_Commit(e);
    if (SUCCEEDED(hr)) {
        step = "ok";
        LARGE_INTEGER z = { 0 };
        ULARGE_INTEGER end;
        IStream_Seek(st, z, STREAM_SEEK_END, &end);
        nebytes = (int)end.QuadPart;
        ebytes = malloc(nebytes + 1);
        IStream_Seek(st, z, STREAM_SEEK_SET, NULL);
        ULONG got = 0;
        IStream_Read(st, ebytes, nebytes, &got);
        nebytes = got;
    }
    out("wencode %08lx ", hr);
    put_fmt(&g);
    out(" %s\n", step);
done:
    if (bag) IPropertyBag2_Release(bag);
    if (fe) IWICBitmapFrameEncode_Release(fe);
    if (e) IWICBitmapEncoder_Release(e);
    if (st) IStream_Release(st);
}

/* wbytes [from n]: the encoded file: "wbytes <length>" then its bytes (n of them from `from`) */
static void op_bytes(void) {
    long from = 0, n = nebytes;
    if (ntok == 3) { from = num(1); n = num(2); }
    if (bad) return;
    if (from > nebytes) from = nebytes;
    if (from < 0 || n < 0) { bad = 1; return; }
    if (n > nebytes - from) n = nebytes - from;
    out("wbytes %d\n", nebytes);
    out_hex(ebytes ? ebytes + from : (unsigned char *)"", ebytes ? n : 0);
}

/* wredecode: decodes the encoded file, as wdecode */
static void op_redecode(void) {
    if (!ebytes) { out("wredecode -\n"); return; }
    decode("wredecode", ebytes, nebytes);
}

static void reset(void) {
    set_cur(NULL);
    close_decoder();
    free(ebytes);
    ebytes = NULL;
    nebytes = 0;
}

static const struct kz_op wicimage_ops[] = {
    { "wbits", op_bits },
    { "wscale", op_scale },
    { "wflip", op_flip },
    { "wclip", op_clip },
    { "wconv", op_conv },
    { "wcolor", op_color },
    { "wctx", op_ctx },
    { "winfo", op_info },
    { "wpixels", op_pixels },
    { "wpalette", op_palette },
    { "wsrcpal", op_srcpal },
    { "wdecode", op_decode },
    { "wframe", op_frame },
    { "wencode", op_encode },
    { "wbytes", op_bytes },
    { "wredecode", op_redecode },
    { "wsteps", op_steps },
    { 0 },
};
const struct kz_api wicimage_api = { "wicimage", wicimage_ops, reset };
