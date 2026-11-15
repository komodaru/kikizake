#include "core.h"
#include <initguid.h>
#include <dwrite_3.h>
/* DirectWrite (dwrite.dll loaded when a case first asks for it): a font file the case gives
 * (an in-memory font file loader), its face, glyph metrics, shaping, glyph run rasterization,
 * text analysis (script, bidi, line breaks) and text layout. An isolated IDWriteFactory5 made
 * once; "<op> none" without one. */

static IDWriteFactory5 *dw;
static IDWriteInMemoryFontFileLoader *memload;
static IDWriteTextAnalyzer1 *ana;
static IDWriteFontFile *dwfile;
static IDWriteFontFace *face;
static int tried;

static void dwrite_reset(void) {
    if (face) { IDWriteFontFace_Release(face); face = NULL; }
    if (dwfile) { IDWriteFontFile_Release(dwfile); dwfile = NULL; }
}

static int factory(void) {
    if (!tried) {
        tried = 1;
        HMODULE m = LoadLibraryA("dwrite.dll");
        HRESULT (WINAPI *create)(DWRITE_FACTORY_TYPE, REFIID, IUnknown **) = m ? (void *)GetProcAddress(m, "DWriteCreateFactory") : NULL;
        if (create && SUCCEEDED(create(DWRITE_FACTORY_TYPE_ISOLATED, &IID_IDWriteFactory5, (IUnknown **)&dw)) && dw) {
            IDWriteTextAnalyzer *a = NULL;
            if (FAILED(IDWriteFactory5_CreateInMemoryFontFileLoader(dw, &memload)) || FAILED(IDWriteFactory5_RegisterFontFileLoader(dw, (IDWriteFontFileLoader *)memload))) memload = NULL;
            if (SUCCEEDED(IDWriteFactory5_CreateTextAnalyzer(dw, &a)) && a) {
                IDWriteTextAnalyzer_QueryInterface(a, &IID_IDWriteTextAnalyzer1, (void **)&ana);
                IDWriteTextAnalyzer_Release(a);
            }
        } else dw = NULL;
    }
    if (!dw || !memload || !ana) { out("%s none\n", tok[0]); return 0; }
    return 1;
}

static int needface(void) {
    if (!factory()) return 0;
    if (!face) { out("%s noface\n", tok[0]); return 0; }
    return 1;
}

static unsigned fbits(float f) { unsigned u; memcpy(&u, &f, 4); return u; }

/* the text of tokens i.. (code points, decimal or 0x hex) as UTF-16; returns its length */
static int utf16(int i, WCHAR *w, int max) {
    int n = 0;
    for (; i < ntok; i++) {
        unsigned long c = (unsigned long)num(i);
        if (bad || c > 0x10ffff || n + 2 > max) { bad = 1; return 0; }
        if (c >= 0x10000) { c -= 0x10000; w[n++] = (WCHAR)(0xd800 + (c >> 10)); w[n++] = (WCHAR)(0xdc00 + (c & 0x3ff)); }
        else w[n++] = (WCHAR)c;
    }
    return n;
}

/* dwfont <hex>: the bytes as a font file (CreateInMemoryFontFileReference, a copy), Analyze,
 * CreateFontFace of its first face. Prints "dwfont analyze hr <hr> supported <0|1> file <t>
 * face <t> faces <n>" and "dwfont face hr <hr>". */
static void op_dwfont(void) {
    dwrite_reset();
    if (ntok != 2) { bad = 1; return; }
    int n = (int)strlen(tok[1]) / 2;
    unsigned char *b = malloc(n + 1);
    for (int i = 0; i < n; i++) {
        unsigned v;
        if (sscanf(tok[1] + 2 * i, "%2x", &v) != 1) { bad = 1; free(b); return; }
        b[i] = (unsigned char)v;
    }
    if (!factory()) { free(b); return; }
    HRESULT hr = IDWriteInMemoryFontFileLoader_CreateInMemoryFontFileReference(memload, (IDWriteFactory *)dw, b, n, NULL, &dwfile);
    free(b);
    if (FAILED(hr)) { out("dwfont ref hr %08lx\n", hr); dwfile = NULL; return; }
    BOOL sup = FALSE;
    DWRITE_FONT_FILE_TYPE ft = 0;
    DWRITE_FONT_FACE_TYPE fft = 0;
    UINT32 faces = 0;
    hr = IDWriteFontFile_Analyze(dwfile, &sup, &ft, &fft, &faces);
    out("dwfont analyze hr %08lx supported %d file %d face %d faces %u\n", hr, sup, ft, fft, faces);
    if (FAILED(hr) || !sup) return;
    hr = IDWriteFactory5_CreateFontFace(dw, fft, 1, &dwfile, 0, DWRITE_FONT_SIMULATIONS_NONE, &face);
    out("dwfont face hr %08lx\n", hr);
    if (FAILED(hr)) face = NULL;
}

/* dwinfo: GetGlyphCount, GetType, GetIndex, GetSimulations, IsSymbolFont, IsMonospacedFont,
 * HasVerticalGlyphVariants */
static void op_dwinfo(void) {
    if (!needface()) return;
    IDWriteFontFace1 *f1 = NULL;
    IDWriteFontFace_QueryInterface(face, &IID_IDWriteFontFace1, (void **)&f1);
    out("dwinfo glyphs %u type %d index %u sim %d symbol %d mono %d vert %d\n", IDWriteFontFace_GetGlyphCount(face), IDWriteFontFace_GetType(face), IDWriteFontFace_GetIndex(face),
        IDWriteFontFace_GetSimulations(face), IDWriteFontFace_IsSymbolFont(face), f1 ? IDWriteFontFace1_IsMonospacedFont(f1) : -1, f1 ? IDWriteFontFace1_HasVerticalGlyphVariants(f1) : -1);
    if (f1) IDWriteFontFace1_Release(f1);
}

static void print_metrics(const char *name, const DWRITE_FONT_METRICS1 *m, int ext) {
    out("%s upm %u asc %u desc %u gap %d cap %u x %u ulpos %d ulthick %u stpos %d stthick %u\n", name, m->designUnitsPerEm, m->ascent, m->descent, m->lineGap, m->capHeight, m->xHeight,
        m->underlinePosition, m->underlineThickness, m->strikethroughPosition, m->strikethroughThickness);
    if (ext)
        out("%s box %d %d %d %d sub %d %d %d %d sup %d %d %d %d typo %d\n", name, m->glyphBoxLeft, m->glyphBoxTop, m->glyphBoxRight, m->glyphBoxBottom, m->subscriptPositionX, m->subscriptPositionY,
            m->subscriptSizeX, m->subscriptSizeY, m->superscriptPositionX, m->superscriptPositionY, m->superscriptSizeX, m->superscriptSizeY, m->hasTypographicMetrics);
}

/* dwmetrics: IDWriteFontFace1::GetMetrics (DWRITE_FONT_METRICS1) */
static void op_dwmetrics(void) {
    if (!needface()) return;
    DWRITE_FONT_METRICS1 m;
    memset(&m, 0, sizeof m);
    IDWriteFontFace1 *f1 = NULL;
    IDWriteFontFace_QueryInterface(face, &IID_IDWriteFontFace1, (void **)&f1);
    if (f1) { IDWriteFontFace1_GetMetrics(f1, &m); IDWriteFontFace1_Release(f1); }
    else IDWriteFontFace_GetMetrics(face, (DWRITE_FONT_METRICS *)&m);
    print_metrics("dwmetrics", &m, f1 != NULL);
}

/* dwgdimetrics <em> <ppd>: GetGdiCompatibleMetrics, no transform */
static void op_dwgdimetrics(void) {
    long em = num(1), ppd = num(2);
    if (bad || ntok != 3) { bad = 1; return; }
    if (!needface()) return;
    DWRITE_FONT_METRICS1 m;
    memset(&m, 0, sizeof m);
    HRESULT hr = IDWriteFontFace_GetGdiCompatibleMetrics(face, (float)em, (float)ppd, NULL, (DWRITE_FONT_METRICS *)&m);
    if (FAILED(hr)) { out("dwgdimetrics hr %08lx\n", hr); return; }
    print_metrics("dwgdimetrics", &m, 0);
}

/* dwcmap <cp>...: GetGlyphIndices */
static void op_dwcmap(void) {
    UINT32 cp[MAXTOK];
    UINT16 g[MAXTOK];
    int n = ntok - 1;
    for (int i = 0; i < n; i++) cp[i] = (UINT32)num(i + 1);
    if (bad || n < 1) { bad = 1; return; }
    if (!needface()) return;
    HRESULT hr = IDWriteFontFace_GetGlyphIndices(face, cp, n, g);
    if (FAILED(hr)) { out("dwcmap hr %08lx\n", hr); return; }
    out("dwcmap");
    for (int i = 0; i < n; i++) out(" %u", g[i]);
    out("\n");
}

static void print_glyph_metrics(const DWRITE_GLYPH_METRICS *m, int n) {
    for (int i = 0; i < n; i++)
        out("glyph adv %u lsb %d rsb %d tsb %d advh %u bsb %d voy %d\n", m[i].advanceWidth, m[i].leftSideBearing, m[i].rightSideBearing, m[i].topSideBearing, m[i].advanceHeight, m[i].bottomSideBearing,
            m[i].verticalOriginY);
}

/* dwglyph <sideways 0|1> <gid>...: GetDesignGlyphMetrics */
static void op_dwglyph(void) {
    UINT16 g[MAXTOK];
    DWRITE_GLYPH_METRICS m[MAXTOK];
    long side = num(1);
    int n = ntok - 2;
    for (int i = 0; i < n; i++) g[i] = (UINT16)num(i + 2);
    if (bad || n < 1) { bad = 1; return; }
    if (!needface()) return;
    HRESULT hr = IDWriteFontFace_GetDesignGlyphMetrics(face, g, n, m, side != 0);
    out("dwglyph hr %08lx\n", hr);
    if (SUCCEEDED(hr)) print_glyph_metrics(m, n);
}

/* dwgdiglyph <em> <ppd> <natural 0|1> <sideways 0|1> <gid>...: GetGdiCompatibleGlyphMetrics */
static void op_dwgdiglyph(void) {
    UINT16 g[MAXTOK];
    DWRITE_GLYPH_METRICS m[MAXTOK];
    long em = num(1), ppd = num(2), nat = num(3), side = num(4);
    int n = ntok - 5;
    for (int i = 0; i < n; i++) g[i] = (UINT16)num(i + 5);
    if (bad || n < 1) { bad = 1; return; }
    if (!needface()) return;
    HRESULT hr = IDWriteFontFace_GetGdiCompatibleGlyphMetrics(face, (float)em, (float)ppd, NULL, nat != 0, g, n, m, side != 0);
    out("dwgdiglyph hr %08lx\n", hr);
    if (SUCCEEDED(hr)) print_glyph_metrics(m, n);
}

/* ------------------------------------------------------------------------------------------ */
/* text analysis: a source over the case's text and a sink that prints what it is told */

static WCHAR atext[256];
static int alen, artl;

static HRESULT STDMETHODCALLTYPE src_qi(IDWriteTextAnalysisSource *s, REFIID r, void **o) {
    if (IsEqualIID(r, &IID_IUnknown) || IsEqualIID(r, &IID_IDWriteTextAnalysisSource)) { *o = s; return S_OK; }
    *o = NULL;
    return E_NOINTERFACE;
}
static ULONG STDMETHODCALLTYPE one_ref(IUnknown *s) { return 1; }
static HRESULT STDMETHODCALLTYPE src_at(IDWriteTextAnalysisSource *s, UINT32 p, const WCHAR **t, UINT32 *n) {
    if ((int)p >= alen) { *t = NULL; *n = 0; }
    else { *t = atext + p; *n = alen - p; }
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE src_before(IDWriteTextAnalysisSource *s, UINT32 p, const WCHAR **t, UINT32 *n) {
    if (!p || (int)p > alen) { *t = NULL; *n = 0; }
    else { *t = atext; *n = p; }
    return S_OK;
}
static DWRITE_READING_DIRECTION STDMETHODCALLTYPE src_dir(IDWriteTextAnalysisSource *s) { return artl ? DWRITE_READING_DIRECTION_RIGHT_TO_LEFT : DWRITE_READING_DIRECTION_LEFT_TO_RIGHT; }
static HRESULT STDMETHODCALLTYPE src_locale(IDWriteTextAnalysisSource *s, UINT32 p, UINT32 *n, const WCHAR **l) {
    *l = L"en-us";
    *n = alen - p;
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE src_numsub(IDWriteTextAnalysisSource *s, UINT32 p, UINT32 *n, IDWriteNumberSubstitution **ns) {
    *ns = NULL;
    *n = alen - p;
    return S_OK;
}
static IDWriteTextAnalysisSourceVtbl srcvt = { src_qi, (void *)one_ref, (void *)one_ref, src_at, src_before, src_dir, src_locale, src_numsub };
static IDWriteTextAnalysisSource src = { &srcvt };

/* what the sink was told: scripts by run, and per UTF-16 unit the breakpoints and levels */
static DWRITE_SCRIPT_ANALYSIS sa_unit[256];
static DWRITE_LINE_BREAKPOINT bp_unit[256];
static UINT8 lv_explicit[256], lv_resolved[256];
static int sink_runs; /* SetScriptAnalysis / SetBidiLevel calls */
static int quiet; /* SetScriptAnalysis prints nothing */

static HRESULT STDMETHODCALLTYPE sink_qi(IDWriteTextAnalysisSink *s, REFIID r, void **o) {
    if (IsEqualIID(r, &IID_IUnknown) || IsEqualIID(r, &IID_IDWriteTextAnalysisSink)) { *o = s; return S_OK; }
    *o = NULL;
    return E_NOINTERFACE;
}
static HRESULT STDMETHODCALLTYPE sink_script(IDWriteTextAnalysisSink *s, UINT32 p, UINT32 n, const DWRITE_SCRIPT_ANALYSIS *a) {
    for (UINT32 i = p; i < p + n && i < 256; i++) sa_unit[i] = *a;
    if (quiet) return S_OK;
    out("run %u %u", p, n);
    DWRITE_SCRIPT_PROPERTIES pr;
    memset(&pr, 0, sizeof pr);
    if (SUCCEEDED(IDWriteTextAnalyzer1_GetScriptProperties(ana, *a, &pr))) {
        char iso[5] = { 0 };
        memcpy(iso, &pr.isoScriptCode, 4);
        out(" %s %u shapes %d", iso[0] ? iso : "-", pr.isoScriptNumber, a->shapes);
    }
    out("\n");
    sink_runs++;
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE sink_break(IDWriteTextAnalysisSink *s, UINT32 p, UINT32 n, const DWRITE_LINE_BREAKPOINT *b) {
    for (UINT32 i = 0; i < n && p + i < 256; i++) bp_unit[p + i] = b[i];
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE sink_bidi(IDWriteTextAnalysisSink *s, UINT32 p, UINT32 n, UINT8 e, UINT8 r) {
    for (UINT32 i = p; i < p + n && i < 256; i++) { lv_explicit[i] = e; lv_resolved[i] = r; }
    sink_runs++;
    return S_OK;
}
static HRESULT STDMETHODCALLTYPE sink_numsub(IDWriteTextAnalysisSink *s, UINT32 p, UINT32 n, IDWriteNumberSubstitution *ns) { return S_OK; }
static IDWriteTextAnalysisSinkVtbl sinkvt = { sink_qi, (void *)one_ref, (void *)one_ref, sink_script, sink_break, sink_bidi, sink_numsub };
static IDWriteTextAnalysisSink sink = { &sinkvt };

/* the text of tokens `at`.. into atext */
static int analysis_text(int at) {
    alen = utf16(at, atext, 256);
    if (bad || !alen) { bad = 1; return 0; }
    return factory();
}

/* dwscript <cp>...: AnalyzeScript; "run <pos> <len> <iso> <iso number> shapes <s>" per run */
static void op_dwscript(void) {
    artl = 0;
    if (!analysis_text(1)) return;
    HRESULT hr = IDWriteTextAnalyzer1_AnalyzeScript(ana, &src, 0, alen, &sink);
    out("dwscript hr %08lx\n", hr);
}

/* dwbidi <ltr|rtl> <cp>...: AnalyzeBidi; the explicit and resolved level of each UTF-16 unit */
static void op_dwbidi(void) {
    artl = is(1, "rtl");
    if (!artl && !is(1, "ltr")) { bad = 1; return; }
    if (!analysis_text(2)) return;
    memset(lv_explicit, 0xff, sizeof lv_explicit);
    memset(lv_resolved, 0xff, sizeof lv_resolved);
    HRESULT hr = IDWriteTextAnalyzer1_AnalyzeBidi(ana, &src, 0, alen, &sink);
    out("dwbidi hr %08lx\nexplicit", hr);
    for (int i = 0; i < alen; i++) out(" %d", lv_explicit[i]);
    out("\nresolved");
    for (int i = 0; i < alen; i++) out(" %d", lv_resolved[i]);
    out("\n");
}

/* dwbreak <cp>...: AnalyzeLineBreakpoints; each UTF-16 unit as before.after.ws.shy */
static void op_dwbreak(void) {
    artl = 0;
    if (!analysis_text(1)) return;
    memset(bp_unit, 0xff, sizeof bp_unit);
    HRESULT hr = IDWriteTextAnalyzer1_AnalyzeLineBreakpoints(ana, &src, 0, alen, &sink);
    out("dwbreak hr %08lx\nbreaks", hr);
    for (int i = 0; i < alen; i++) out(" %d%d%d%d", bp_unit[i].breakConditionBefore, bp_unit[i].breakConditionAfter, bp_unit[i].isWhitespace, bp_unit[i].isSoftHyphen);
    out("\n");
}

/* ------------------------------------------------------------------------------------------ */

/* dwshape <em> <cp>...: the face's glyphs for the text (AnalyzeScript's first run, en-us, left
 * to right, no features): GetGlyphs, GetGlyphPlacements, GetGdiCompatibleGlyphPlacements (ppd 1,
 * GDI classic then natural). Advances and offsets as float bits. */
static void op_dwshape(void) {
    long em = num(1);
    if (bad) return;
    artl = 0;
    alen = utf16(2, atext, 256);
    if (bad || !alen) { bad = 1; return; }
    if (!needface()) return;
    memset(sa_unit, 0, sizeof sa_unit);
    quiet = 1;
    HRESULT hr = IDWriteTextAnalyzer1_AnalyzeScript(ana, &src, 0, alen, &sink);
    quiet = 0;
    DWRITE_SCRIPT_ANALYSIS sa = sa_unit[0];
    UINT16 cl[256], gid[512];
    DWRITE_SHAPING_TEXT_PROPERTIES tp[256];
    DWRITE_SHAPING_GLYPH_PROPERTIES gp[512];
    UINT32 ng = 0;
    hr = IDWriteTextAnalyzer1_GetGlyphs(ana, atext, alen, face, FALSE, FALSE, &sa, L"en-us", NULL, NULL, NULL, 0, 512, cl, tp, gid, gp, &ng);
    out("dwshape glyphs hr %08lx n %u\n", hr, ng);
    if (FAILED(hr)) return;
    out("gids");
    for (UINT32 i = 0; i < ng; i++) out(" %u", gid[i]);
    out("\nclusters");
    for (int i = 0; i < alen; i++) out(" %u", cl[i]);
    out("\ntextprops");
    for (int i = 0; i < alen; i++) { UINT16 v; memcpy(&v, &tp[i], 2); out(" %x", v); }
    out("\nglyphprops");
    for (UINT32 i = 0; i < ng; i++) { UINT16 v; memcpy(&v, &gp[i], 2); out(" %x", v); }
    out("\n");
    FLOAT adv[512];
    DWRITE_GLYPH_OFFSET off[512];
    for (int k = 0; k < 3; k++) {
        if (k == 0) hr = IDWriteTextAnalyzer1_GetGlyphPlacements(ana, atext, cl, tp, alen, gid, gp, ng, face, (float)em, FALSE, FALSE, &sa, L"en-us", NULL, NULL, 0, adv, off);
        else hr = IDWriteTextAnalyzer1_GetGdiCompatibleGlyphPlacements(ana, atext, cl, tp, alen, gid, gp, ng, face, (float)em, 1.0f, NULL, k == 2, FALSE, FALSE, &sa, L"en-us", NULL, NULL, 0, adv, off);
        static const char *names[] = { "place", "gdiplace", "natplace" };
        out("%s hr %08lx", names[k], hr);
        if (SUCCEEDED(hr))
            for (UINT32 i = 0; i < ng; i++) out(" %08x/%08x/%08x", fbits(adv[i]), fbits(off[i].advanceOffset), fbits(off[i].ascenderOffset));
        out("\n");
    }
}

/* dwrun <mode> <em> <x4> <y4> <gid> <adv4> [<gid> <adv4>]...: IDWriteFactory3::CreateGlyphRunAnalysis
 * of the face's glyphs at em size <em>, the baseline origin (x4 / 4, y4 / 4), each advance
 * adv4 / 4 pixels, no transform. Modes: aliased (rendering aliased, antialias grayscale, texture
 * ALIASED_1x1), gray (natural symmetric, grayscale, ALIASED_1x1), cleartype (natural symmetric,
 * cleartype, CLEARTYPE_3x1), natural (natural, grayscale), gdi (GDI classic rendering and
 * measuring, grayscale), gdinatural (GDI natural rendering and measuring, grayscale); grid fit
 * default, or enabled when the mode ends in .fit, disabled in .nofit. Prints "dwrun hr", the texture bounds, then each row's bytes. */
static void op_dwrun(void) {
    static const char *modes[] = { "aliased", "gray", "cleartype", "natural", "gdi", "gdinatural" };
    int mode = -1;
    DWRITE_GRID_FIT_MODE fit = DWRITE_GRID_FIT_MODE_DEFAULT;
    char m[32] = "";
    if (ntok > 1 && strlen(tok[1]) < sizeof m) strcpy(m, tok[1]);
    char *dot = strchr(m, '.');
    if (dot) {
        *dot++ = 0;
        if (!strcmp(dot, "fit")) fit = DWRITE_GRID_FIT_MODE_ENABLED;
        else if (!strcmp(dot, "nofit")) fit = DWRITE_GRID_FIT_MODE_DISABLED;
        else bad = 1;
    }
    for (int i = 0; i < 6; i++)
        if (!strcmp(m, modes[i])) mode = i;
    long em = num(2), x4 = num(3), y4 = num(4);
    int n = (ntok - 5) / 2;
    UINT16 g[MAXTOK];
    FLOAT adv[MAXTOK];
    DWRITE_GLYPH_OFFSET off[MAXTOK];
    for (int i = 0; i < n; i++) {
        g[i] = (UINT16)num(5 + 2 * i);
        adv[i] = num(6 + 2 * i) / 4.0f;
        off[i].advanceOffset = off[i].ascenderOffset = 0;
    }
    if (bad || mode < 0 || n < 1 || (ntok - 5) % 2) { bad = 1; return; }
    if (!needface()) return;
    static const DWRITE_RENDERING_MODE1 rm[] = { DWRITE_RENDERING_MODE1_ALIASED, DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, DWRITE_RENDERING_MODE1_NATURAL_SYMMETRIC, DWRITE_RENDERING_MODE1_NATURAL,
                                                 DWRITE_RENDERING_MODE1_GDI_CLASSIC, DWRITE_RENDERING_MODE1_GDI_NATURAL };
    static const DWRITE_MEASURING_MODE mm[] = { DWRITE_MEASURING_MODE_NATURAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_MEASURING_MODE_NATURAL, DWRITE_MEASURING_MODE_NATURAL,
                                                DWRITE_MEASURING_MODE_GDI_CLASSIC, DWRITE_MEASURING_MODE_GDI_NATURAL };
    DWRITE_TEXTURE_TYPE tt = mode == 2 ? DWRITE_TEXTURE_CLEARTYPE_3x1 : DWRITE_TEXTURE_ALIASED_1x1;
    DWRITE_GLYPH_RUN run = { face, (float)em, n, g, adv, off, FALSE, 0 };
    IDWriteGlyphRunAnalysis *a = NULL;
    HRESULT hr = IDWriteFactory3_CreateGlyphRunAnalysis((IDWriteFactory3 *)dw, &run, NULL, rm[mode], mm[mode], fit,
                                                        mode == 2 ? DWRITE_TEXT_ANTIALIAS_MODE_CLEARTYPE : DWRITE_TEXT_ANTIALIAS_MODE_GRAYSCALE, x4 / 4.0f, y4 / 4.0f, &a);
    out("dwrun hr %08lx\n", hr);
    if (FAILED(hr) || !a) return;
    RECT r = { 0 };
    hr = IDWriteGlyphRunAnalysis_GetAlphaTextureBounds(a, tt, &r);
    out("bounds hr %08lx %ld %ld %ld %ld\n", hr, r.left, r.top, r.right, r.bottom);
    int w = r.right - r.left, h = r.bottom - r.top, bpp = tt == DWRITE_TEXTURE_CLEARTYPE_3x1 ? 3 : 1;
    if (SUCCEEDED(hr) && w > 0 && h > 0) {
        if (w > 256 || h > 256) out("big\n");
        else {
            unsigned char *t = calloc(w * h * bpp, 1);
            hr = IDWriteGlyphRunAnalysis_CreateAlphaTexture(a, tt, &r, t, w * h * bpp);
            if (FAILED(hr)) out("texture hr %08lx\n", hr);
            else
                for (int y = 0; y < h; y++) out_hex(t + y * w * bpp, w * bpp);
            free(t);
        }
    }
    IDWriteGlyphRunAnalysis_Release(a);
}

/* dwlayout <em> <maxw4> <wrap 0|1> <cp>...: the face's file as the only font of a collection
 * (font set builder), a text format of its first family (normal weight, style, stretch, en-us),
 * a layout of the text maxw4 / 4 pixels wide, 1000 high. Prints the family, GetMetrics, each
 * line's metrics and each cluster's. Floats as bits. */
/* dwlayout em maxwidth4 wrap chars..., and dwlayoutnf the same with an empty font fallback set
   on the format (characters the font does not map stay in it) */
static int nofallback;
static void op_dwlayout(void) {
    long em = num(1), mw = num(2), wrap = num(3);
    if (bad) return;
    artl = 0;
    WCHAR t[256];
    int n = utf16(4, t, 256);
    if (bad || !n) { bad = 1; return; }
    if (!needface()) return;
    IDWriteFontSetBuilder1 *sb = NULL;
    IDWriteFontSet *set = NULL;
    IDWriteFontCollection1 *col = NULL;
    IDWriteFontFamily *fam = NULL;
    IDWriteLocalizedStrings *names = NULL;
    IDWriteTextFormat *fmt = NULL;
    IDWriteTextLayout *lay = NULL;
    WCHAR fname[128] = { 0 };
    HRESULT hr = IDWriteFactory5_CreateFontSetBuilder(dw, &sb);
    if (SUCCEEDED(hr)) hr = IDWriteFontSetBuilder1_AddFontFile(sb, dwfile);
    if (SUCCEEDED(hr)) hr = IDWriteFontSetBuilder1_CreateFontSet(sb, &set);
    if (SUCCEEDED(hr)) hr = IDWriteFactory5_CreateFontCollectionFromFontSet(dw, set, &col);
    if (FAILED(hr)) { out("dwlayout set hr %08lx\n", hr); goto done; }
    UINT32 nf = IDWriteFontCollection1_GetFontFamilyCount(col);
    hr = nf ? IDWriteFontCollection1_GetFontFamily(col, 0, (IDWriteFontFamily1 **)&fam) : E_FAIL;
    if (SUCCEEDED(hr)) hr = IDWriteFontFamily_GetFamilyNames(fam, &names);
    if (SUCCEEDED(hr)) hr = IDWriteLocalizedStrings_GetString(names, 0, fname, 128);
    out("dwlayout families %u hr %08lx name", nf, hr);
    for (int i = 0; fname[i]; i++) out(" %u", fname[i]);
    out("\n");
    if (FAILED(hr)) goto done;
    hr = IDWriteFactory5_CreateTextFormat(dw, fname, (IDWriteFontCollection *)col, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, (float)em, L"en-us", &fmt);
    if (SUCCEEDED(hr)) IDWriteTextFormat_SetWordWrapping(fmt, wrap ? DWRITE_WORD_WRAPPING_WRAP : DWRITE_WORD_WRAPPING_NO_WRAP);
    if (SUCCEEDED(hr) && nofallback) {
        IDWriteFontFallbackBuilder *fb = NULL;
        IDWriteFontFallback *ff = NULL;
        IDWriteTextFormat1 *f1 = NULL;
        hr = IDWriteFactory5_CreateFontFallbackBuilder(dw, &fb);
        if (SUCCEEDED(hr)) hr = IDWriteFontFallbackBuilder_CreateFontFallback(fb, &ff);
        if (SUCCEEDED(hr)) hr = IDWriteTextFormat_QueryInterface(fmt, &IID_IDWriteTextFormat1, (void **)&f1);
        if (SUCCEEDED(hr)) hr = IDWriteTextFormat1_SetFontFallback(f1, ff);
        if (f1) IDWriteTextFormat1_Release(f1);
        if (ff) IDWriteFontFallback_Release(ff);
        if (fb) IDWriteFontFallbackBuilder_Release(fb);
        if (FAILED(hr)) { out("dwlayout fallback hr %08lx\n", hr); goto done; }
    }
    if (SUCCEEDED(hr)) hr = IDWriteFactory5_CreateTextLayout(dw, t, n, fmt, mw / 4.0f, 1000.0f, &lay);
    if (FAILED(hr)) { out("dwlayout layout hr %08lx\n", hr); goto done; }
    DWRITE_TEXT_METRICS m;
    hr = IDWriteTextLayout_GetMetrics(lay, &m);
    out("metrics hr %08lx left %08x top %08x width %08x wtrail %08x height %08x lines %u depth %u\n", hr, fbits(m.left), fbits(m.top), fbits(m.width), fbits(m.widthIncludingTrailingWhitespace),
        fbits(m.height), m.lineCount, m.maxBidiReorderingDepth);
    DWRITE_LINE_METRICS lm[64];
    UINT32 nl = 0;
    hr = IDWriteTextLayout_GetLineMetrics(lay, lm, 64, &nl);
    for (UINT32 i = 0; i < nl && i < 64; i++)
        out("line %u trail %u nl %u height %08x base %08x trimmed %d\n", lm[i].length, lm[i].trailingWhitespaceLength, lm[i].newlineLength, fbits(lm[i].height), fbits(lm[i].baseline), lm[i].isTrimmed);
    DWRITE_CLUSTER_METRICS cm[256];
    UINT32 nc = 0;
    hr = IDWriteTextLayout_GetClusterMetrics(lay, cm, 256, &nc);
    out("clusters hr %08lx n %u", hr, nc);
    for (UINT32 i = 0; i < nc && i < 256; i++) out(" %08x/%u/%d%d%d%d%d", fbits(cm[i].width), cm[i].length, cm[i].canWrapLineAfter, cm[i].isWhitespace, cm[i].isNewline, cm[i].isSoftHyphen, cm[i].isRightToLeft);
    out("\n");
done:
    if (lay) IDWriteTextLayout_Release(lay);
    if (fmt) IDWriteTextFormat_Release(fmt);
    if (names) IDWriteLocalizedStrings_Release(names);
    if (fam) IDWriteFontFamily_Release(fam);
    if (col) IDWriteFontCollection1_Release(col);
    if (set) IDWriteFontSet_Release(set);
    if (sb) IDWriteFontSetBuilder1_Release(sb);
}
static void op_dwlayoutnf(void) {
    nofallback = 1;
    op_dwlayout();
    nofallback = 0;
}

/* dwfallback <cp>...: the system font fallback (IDWriteFactory2::GetSystemFontFallback) asked
 * to map the text, its base font the face's family in a collection of the face's file alone
 * (normal weight, style, stretch, en-us), as a layout of it would. Prints each mapped piece:
 * "map <length> scale <bits>" and, when a system font maps it, "font <weight> <style> <stretch>
 * sim <n> name <family name's UTF-16 units>", "fmetrics upm <n> asc <n> desc <n> gap <n>" and
 * "glyphs <glyph>/<design advance>..." of its code points; "nofont" when none does. */
static void op_dwfallback(void) {
    artl = 0;
    alen = utf16(1, atext, 256);
    if (bad || !alen) { bad = 1; return; }
    if (!needface()) return;
    IDWriteFontSetBuilder1 *sb = NULL;
    IDWriteFontSet *set = NULL;
    IDWriteFontCollection1 *col = NULL;
    IDWriteFontFamily *fam = NULL;
    IDWriteLocalizedStrings *names = NULL;
    IDWriteFontFallback *ff = NULL;
    WCHAR fname[128] = { 0 };
    HRESULT hr = IDWriteFactory5_CreateFontSetBuilder(dw, &sb);
    if (SUCCEEDED(hr)) hr = IDWriteFontSetBuilder1_AddFontFile(sb, dwfile);
    if (SUCCEEDED(hr)) hr = IDWriteFontSetBuilder1_CreateFontSet(sb, &set);
    if (SUCCEEDED(hr)) hr = IDWriteFactory5_CreateFontCollectionFromFontSet(dw, set, &col);
    UINT32 nf = SUCCEEDED(hr) ? IDWriteFontCollection1_GetFontFamilyCount(col) : 0;
    if (SUCCEEDED(hr)) hr = nf ? IDWriteFontCollection1_GetFontFamily(col, 0, (IDWriteFontFamily1 **)&fam) : E_FAIL;
    if (SUCCEEDED(hr)) hr = IDWriteFontFamily_GetFamilyNames(fam, &names);
    if (SUCCEEDED(hr)) hr = IDWriteLocalizedStrings_GetString(names, 0, fname, 128);
    if (SUCCEEDED(hr)) hr = IDWriteFactory5_GetSystemFontFallback(dw, &ff);
    out("dwfallback hr %08lx\n", hr);
    if (FAILED(hr)) goto done;
    for (UINT32 at = 0; at < (UINT32)alen;) {
        UINT32 len = 0;
        IDWriteFont *f = NULL;
        FLOAT scale = 0;
        hr = IDWriteFontFallback_MapCharacters(ff, &src, at, alen - at, (IDWriteFontCollection *)col, fname, DWRITE_FONT_WEIGHT_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_STRETCH_NORMAL, &len, &f, &scale);
        if (FAILED(hr) || !len) { out("map hr %08lx\n", hr); if (f) IDWriteFont_Release(f); break; }
        out("map %u scale %08x\n", len, fbits(scale));
        if (!f) { out("nofont\n"); at += len; continue; }
        IDWriteFontFamily *mf = NULL;
        IDWriteLocalizedStrings *mn = NULL;
        WCHAR mname[128] = { 0 };
        if (SUCCEEDED(IDWriteFont_GetFontFamily(f, &mf)) && SUCCEEDED(IDWriteFontFamily_GetFamilyNames(mf, &mn))) {
            UINT32 k = 0;
            BOOL ex = FALSE;
            if (FAILED(IDWriteLocalizedStrings_FindLocaleName(mn, L"en-us", &k, &ex)) || !ex) k = 0;
            IDWriteLocalizedStrings_GetString(mn, k, mname, 128);
        }
        out("font %d %d %d sim %d name", IDWriteFont_GetWeight(f), IDWriteFont_GetStyle(f), IDWriteFont_GetStretch(f), IDWriteFont_GetSimulations(f));
        for (int i = 0; mname[i]; i++) out(" %u", mname[i]);
        out("\n");
        DWRITE_FONT_METRICS m;
        IDWriteFont_GetMetrics(f, &m);
        out("fmetrics upm %u asc %u desc %u gap %d\n", m.designUnitsPerEm, m.ascent, m.descent, m.lineGap);
        IDWriteFontFace *mface = NULL;
        out("glyphs");
        if (SUCCEEDED(IDWriteFont_CreateFontFace(f, &mface))) {
            for (UINT32 i = at; i < at + len; i++) {
                UINT32 c = atext[i];
                if (c >= 0xd800 && c < 0xdc00 && i + 1 < at + len) { c = 0x10000 + ((c - 0xd800) << 10) + (atext[i + 1] - 0xdc00); i++; }
                UINT16 g = 0;
                DWRITE_GLYPH_METRICS gm;
                memset(&gm, 0, sizeof gm);
                IDWriteFontFace_GetGlyphIndices(mface, &c, 1, &g);
                IDWriteFontFace_GetDesignGlyphMetrics(mface, &g, 1, &gm, FALSE);
                out(" %u/%u", g, gm.advanceWidth);
            }
            IDWriteFontFace_Release(mface);
        }
        out("\n");
        if (mn) IDWriteLocalizedStrings_Release(mn);
        if (mf) IDWriteFontFamily_Release(mf);
        IDWriteFont_Release(f);
        at += len;
    }
done:
    if (ff) IDWriteFontFallback_Release(ff);
    if (names) IDWriteLocalizedStrings_Release(names);
    if (fam) IDWriteFontFamily_Release(fam);
    if (col) IDWriteFontCollection1_Release(col);
    if (set) IDWriteFontSet_Release(set);
    if (sb) IDWriteFontSetBuilder1_Release(sb);
}

static const struct kz_op dwrite_ops[] = {
    { "dwfont", op_dwfont }, { "dwinfo", op_dwinfo }, { "dwmetrics", op_dwmetrics }, { "dwgdimetrics", op_dwgdimetrics }, { "dwcmap", op_dwcmap }, { "dwglyph", op_dwglyph },
    { "dwgdiglyph", op_dwgdiglyph }, { "dwscript", op_dwscript }, { "dwbidi", op_dwbidi }, { "dwbreak", op_dwbreak }, { "dwshape", op_dwshape }, { "dwrun", op_dwrun },
    { "dwlayout", op_dwlayout }, { "dwlayoutnf", op_dwlayoutnf }, { "dwfallback", op_dwfallback }, { 0 },
};
const struct kz_api dwrite_api = { "dwrite", dwrite_ops, dwrite_reset };
