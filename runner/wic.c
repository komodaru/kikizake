#include "core.h"
#include <wincodec.h>
/* WIC pixel format conversion (windowscodecs.dll through COM, initialized when first needed) */

static const struct { const char *n; const GUID *g; int bits; } wicfmts[] = {
    { "bw", &GUID_WICPixelFormatBlackWhite, 1 }, { "gray2", &GUID_WICPixelFormat2bppGray, 2 },
    { "gray4", &GUID_WICPixelFormat4bppGray, 4 }, { "gray8", &GUID_WICPixelFormat8bppGray, 8 },
    { "gray16", &GUID_WICPixelFormat16bppGray, 16 }, { "bgr555", &GUID_WICPixelFormat16bppBGR555, 16 },
    { "bgr565", &GUID_WICPixelFormat16bppBGR565, 16 }, { "bgr24", &GUID_WICPixelFormat24bppBGR, 24 },
    { "rgb24", &GUID_WICPixelFormat24bppRGB, 24 }, { "bgr32", &GUID_WICPixelFormat32bppBGR, 32 },
    { "bgra", &GUID_WICPixelFormat32bppBGRA, 32 }, { "pbgra", &GUID_WICPixelFormat32bppPBGRA, 32 },
    { "rgb32", &GUID_WICPixelFormat32bppRGB, 32 }, { "rgba", &GUID_WICPixelFormat32bppRGBA, 32 },
    { "prgba", &GUID_WICPixelFormat32bppPRGBA, 32 }, { "rgb48", &GUID_WICPixelFormat48bppRGB, 48 },
    { "rgba64", &GUID_WICPixelFormat64bppRGBA, 64 }, { "cmyk", &GUID_WICPixelFormat32bppCMYK, 32 },
    { "alpha8", &GUID_WICPixelFormat8bppAlpha, 8 }, { "idx1", &GUID_WICPixelFormat1bppIndexed, 1 },
    { "idx2", &GUID_WICPixelFormat2bppIndexed, 2 }, { "idx4", &GUID_WICPixelFormat4bppIndexed, 4 },
    { "idx8", &GUID_WICPixelFormat8bppIndexed, 8 },
};
static IWICImagingFactory *wic;

static int wicfmt(int i) {
    for (int f = 0; f < (int)(sizeof wicfmts / sizeof *wicfmts); f++)
        if (is(i, wicfmts[f].n)) return f;
    bad = 1;
    return 0;
}

/* the palette of indexed sources and targets: 16 colors, entry 5 not opaque (alpha 0x40) */
static IWICPalette *wic_palette(void) {
    IWICPalette *p = NULL;
    if (FAILED(IWICImagingFactory_CreatePalette(wic, &p)) || !p) return NULL;
    WICColor c[16];
    for (int i = 0; i < 16; i++) c[i] = (i == 5 ? 0x40000000u : 0xff000000u) | (unsigned)(i * 17) << 16 | (unsigned)(255 - i * 13) << 8 | (unsigned)((i * 71) & 255);
    IWICPalette_InitializeCustom(p, c, 16);
    return p;
}

/* convert <from> <w> <h> <seed|ramp> <to>: a bitmap of `from` (rows (w * bits + 7) / 8 bytes,
 * from xorshift32 or byte i = i mod 256), a format converter to `to` (no dither, the palette
 * for indexed formats), CopyPixels of all of it. Prints "conv <w> <h>" and each row's bytes (the
 * bits past the row's last pixel 0), or "conv can <0|1> init|copy hr <hr>". */
static void op_convert(void) {
    int from = wicfmt(1);
    long w = num(2), h = num(3);
    int ramp = is(4, "ramp");
    unsigned seed = ramp ? 0 : (unsigned)num(4);
    int to = wicfmt(5);
    if (bad || w < 1 || h < 1 || w > 256 || h > 64) { bad = 1; return; }
    if (!wic) {
        CoInitializeEx(NULL, COINIT_MULTITHREADED);
        if (FAILED(CoCreateInstance(&CLSID_WICImagingFactory, NULL, CLSCTX_INPROC_SERVER, &IID_IWICImagingFactory, (void **)&wic))) wic = NULL;
    }
    if (!wic) { out("conv none\n"); return; }
    int ss = (w * wicfmts[from].bits + 7) / 8, ds = (w * wicfmts[to].bits + 7) / 8;
    unsigned char *px = malloc(ss * h), *o = calloc(ds * h, 1);
    unsigned x = seed ? seed : 0x9e3779b9;
    for (int i = 0; i < ss * h; i++) {
        x ^= x << 13; x ^= x >> 17; x ^= x << 5;
        px[i] = ramp ? (unsigned char)i : (unsigned char)x;
    }
    IWICBitmap *b = NULL;
    IWICFormatConverter *c = NULL;
    IWICPalette *pal = wic_palette();
    HRESULT hr = IWICImagingFactory_CreateBitmapFromMemory(wic, w, h, wicfmts[from].g, ss, ss * h, px, &b);
    if (FAILED(hr)) { out("conv bitmap hr %08lx\n", hr); goto done; }
    if (from >= 19 && pal) IWICBitmap_SetPalette(b, pal);
    IWICImagingFactory_CreateFormatConverter(wic, &c);
    BOOL can = FALSE;
    IWICFormatConverter_CanConvert(c, wicfmts[from].g, wicfmts[to].g, &can);
    hr = IWICFormatConverter_Initialize(c, (IWICBitmapSource *)b, wicfmts[to].g, WICBitmapDitherTypeNone, to >= 19 ? pal : NULL, 0, to >= 19 ? WICBitmapPaletteTypeCustom : WICBitmapPaletteTypeMedianCut);
    if (FAILED(hr)) { out("conv can %d init hr %08lx\n", can, hr); goto done; }
    memset(o, 0, ds * h);
    hr = IWICFormatConverter_CopyPixels(c, NULL, ds, ds * h, o);
    if (FAILED(hr)) { out("conv can %d copy hr %08lx\n", can, hr); goto done; }
    out("conv %ld %ld can %d\n", w, h, can);
    for (int y = 0; y < h; y++) {
        unsigned char *r = o + y * ds;
        int tail = (w * wicfmts[to].bits) % 8;
        if (tail) r[ds - 1] &= (unsigned char)(0xff00 >> tail);
        for (int i = 0; i < ds; i++) out("%02x", r[i]);
        out("\n");
    }
done:
    if (c) IWICFormatConverter_Release(c);
    if (b) IWICBitmap_Release(b);
    if (pal) IWICPalette_Release(pal);
    free(px);
    free(o);
}

static const struct kz_op wic_ops[] = {
    { "convert", op_convert },
    { 0 },
};
const struct kz_api wic_api = { "wic", wic_ops, 0 };
