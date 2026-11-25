/* Capture the runner's Arial font outlines in a generic 32-bit DIB context. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <GL/gl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned long fbits(GLfloat value) {
    unsigned long bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

static uint64_t hash_word(uint64_t hash, uint32_t word) {
    for (int i = 0; i < 4; ++i) {
        hash = (hash ^ (uint8_t)word) * UINT64_C(1099511628211);
        word >>= 8;
    }
    return hash;
}

/* Full-size feedback groups identical glyphs. A second run uses the runner's
 * 1024-word cap to observe the output independently, including overflow. */
static int scan_wide(HDC dc, int bounded) {
    static GLfloat feedback[65536];
    GLsizei capacity = bounded ? 1024 : (GLsizei)(sizeof feedback / sizeof *feedback);
    glViewport(0, 0, 4096, 4096);
    glMatrixMode(GL_MODELVIEW);
    for (DWORD code = 0; code <= 65535; ++code) {
        printf("%04lx", code);
        for (int mode = 0; mode <= 1; ++mode) {
            glLoadIdentity();
            glScalef(0.5f, 0.5f, 1);
            glDeleteLists(100, 1);
            GLYPHMETRICSFLOAT metrics = {0};
            if (!wglUseFontOutlinesW(dc, code, 1, 100, 0, 0, mode, &metrics)) return 7;
            glFeedbackBuffer(capacity, GL_2D, feedback);
            if (glRenderMode(GL_FEEDBACK) != 0) return 8;
            glCallList(100);
            GLint n = glRenderMode(GL_RENDER);
            if (n < (bounded ? -1 : 0) || n > capacity || glGetError()) return 8;
            const GLfloat values[] = {metrics.gmfBlackBoxX, metrics.gmfBlackBoxY,
                                      metrics.gmfptGlyphOrigin.x, metrics.gmfptGlyphOrigin.y,
                                      metrics.gmfCellIncX, metrics.gmfCellIncY};
            uint64_t hash = UINT64_C(14695981039346656037);
            for (size_t i = 0; i < sizeof values / sizeof values[0]; ++i) {
                hash = hash_word(hash, fbits(values[i]));
            }
            int written = n < 0 ? capacity : n;
            for (int i = 0; i < written; ++i) hash = hash_word(hash, fbits(feedback[i]));
            printf(" %d:%016llx", (int)n, (unsigned long long)hash);
        }
        putchar('\n');
    }
    puts("done");
    return 0;
}

static int font_modes(HDC dc) {
    const GLenum modes[] = {GL_RENDER, GL_FEEDBACK, GL_SELECT};
    glMatrixMode(GL_MODELVIEW);
    glColor4ub(255, 255, 255, 255);
    for (int render = 0; render < 3; ++render) {
        for (int api = 0; api <= 2; ++api) {
            for (int wide = 0; wide <= 1; ++wide) {
                for (int named = 0; named <= 1; ++named) {
                    for (int pending = 0; pending <= 1; ++pending) {
                        GLuint base = named ? 100 : 0;
                        GLuint selection[64];
                        GLfloat feedback[64];
                        glDeleteLists(1, 1);
                        glDeleteLists(100, 2);
                        glClear(GL_COLOR_BUFFER_BIT);
                        glLoadIdentity();
                        glRasterPos2f(-0.5f, -0.5f);
                        glSelectBuffer(64, selection);
                        glFeedbackBuffer(64, GL_2D, feedback);
                        if (pending) glEnable(0xffff);
                        glRenderMode(modes[render]);
                        if (render == 2) {
                            glInitNames();
                            glPushName(42);
                        }
                        GLYPHMETRICSFLOAT metrics[2] = {{0}};
                        SetLastError(0);
                        BOOL ok = !api
                            ? (wide ? wglUseFontBitmapsW(dc, 65, 2, base)
                                    : wglUseFontBitmapsA(dc, 65, 2, base))
                            : (wide ? wglUseFontOutlinesW(dc, 65, 2, base, 0, 0, api - 1, metrics)
                                    : wglUseFontOutlinesA(dc, 65, 2, base, 0, 0, api - 1, metrics));
                        DWORD error = ok ? 0 : GetLastError();
                        GLenum gl_error = glGetError();
                        GLfloat raster[4];
                        glGetFloatv(GL_CURRENT_RASTER_POSITION, raster);
                        GLint written = glRenderMode(GL_RENDER);
                        int defined = glIsList(base);
                        int next = glIsList(base + 1);
                        GLubyte pixels[16 * 16 * 4];
                        glReadPixels(0, 0, 16, 16, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
                        int ink = 0;
                        for (size_t i = 0; i < sizeof pixels; i += 4) ink += pixels[i] != 0;
                        if (glGetError()) return 9;
                        printf("%d %d %d %u %d | %d %lu %x %d %d %d %d",
                               render, api, wide, base, pending, ok, error, gl_error, (int)written,
                               defined, next, ink);
                        for (size_t i = 0; i < sizeof raster / sizeof raster[0]; ++i) {
                            printf(" %08lx", fbits(raster[i]));
                        }
                        if (render == 1) {
                            int items = written < 0 ? 64 : (int)written;
                            for (int i = 0; i < items; ++i) printf(" %lu", fbits(feedback[i]));
                        }
                        if (render == 2 && written == 1) {
                            for (int i = 0; i < 4; ++i) printf(" %u", selection[i]);
                        }
                        putchar('\n');
                    }
                }
            }
        }
    }
    puts("done");
    return 0;
}

static void begin_primitive(int phase) {
    if (phase) glNewList(55, phase == 1 ? GL_COMPILE : GL_COMPILE_AND_EXECUTE);
    glBegin(GL_POINTS);
}

static void end_primitive(int phase) {
    glEnd();
    if (phase) glEndList();
}

static int bitmap_primitives(HDC dc) {
    const DWORD bases[] = {0, 100, 65536};
    const DWORD starts[] = {0, 65, 19968, 65535};
    for (int phase = 0; phase <= 2; ++phase) {
        for (size_t b = 0; b < sizeof bases / sizeof bases[0]; ++b) {
            for (int pending = 0; pending <= 1; ++pending) {
                for (size_t f = 0; f < sizeof starts / sizeof starts[0]; ++f) {
                    for (DWORD count = 0; count <= 2; ++count) {
                        DWORD base = bases[b], first = starts[f];
                        glDeleteLists(55, 1);
                        glDeleteLists(base ? base : 1, base ? 2 : 1);
                        if (pending) glEnable(0xffff);
                        begin_primitive(phase);
                        SetLastError(0);
                        BOOL ok = wglUseFontBitmapsW(dc, first, count, base);
                        DWORD error = ok ? 0 : GetLastError();
                        end_primitive(phase);
                        GLenum gl_error = glGetError();
                        printf("%d %lu %d %lu %lu | %d %lu %x %d %d %d\n",
                               phase, base, pending, first, count, ok, error, gl_error,
                               glIsList(55), glIsList(base), glIsList(base + 1));
                    }
                }
            }
        }
    }
    puts("done");
    return 0;
}

static int outline_primitive(HDC dc, int phase, int format, int count) {
    if (phase < 0 || phase > 2 || format < 0 || format > 2 || count < 0 || count > 2) return 1;
    GLYPHMETRICSFLOAT metrics[2] = {{0}};
    begin_primitive(phase);
    SetLastError(0);
    puts("calling");
    fflush(stdout);
    BOOL ok = wglUseFontOutlinesW(dc, 65, (DWORD)count, 100, 0, 0, format, metrics);
    DWORD error = ok ? 0 : GetLastError();
    end_primitive(phase);
    printf("returned %d %lu %x\n", ok, error, glGetError());
    return 0;
}

int main(int argc, char **argv) {
    const int scan = argc == 2 && !strcmp(argv[1], "wide-scan");
    const int bounded = argc == 2 && !strcmp(argv[1], "wide-readback");
    const int reps = argc == 3 && !strcmp(argv[1], "wide-reps");
    const int states = argc == 2 && !strcmp(argv[1], "font-modes");
    const int bitmap_begin = argc == 2 && !strcmp(argv[1], "bitmap-primitives");
    const int outline_begin = argc == 5 && !strcmp(argv[1], "outline-primitive");
    const int byte_capture = !(scan || bounded || reps || states || bitmap_begin || outline_begin);
    if (byte_capture && argc != 4 && argc != 5) return 1;
    const int first = byte_capture ? atoi(argv[1]) : 0;
    const int last = byte_capture ? atoi(argv[2]) : 0;
    const int sample = byte_capture ? atoi(argv[3]) : 1; /* 0: pixels, 1: half-scale feedback, 2: unscaled feedback */
    if (first < 0 || last < first || last > 255 || sample < 0 || sample > 2) return 1;
    const DWORD offset = byte_capture && argc == 5 ? strtoul(argv[4], NULL, 0) : 0;
    HDC dc = CreateCompatibleDC(NULL);
    BITMAPINFO info = { 0 };
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = 16;
    info.bmiHeader.biHeight = 16;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    void *bits = NULL;
    HBITMAP bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &bits, NULL, 0);
    if (!dc || !bmp || !SelectObject(dc, bmp)) return 2;
    PIXELFORMATDESCRIPTOR pfd = { 0 };
    pfd.nSize = sizeof(pfd);
    pfd.nVersion = 1;
    pfd.dwFlags = PFD_SUPPORT_GDI | PFD_SUPPORT_OPENGL | PFD_DRAW_TO_BITMAP;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.cColorBits = 32;
    pfd.cDepthBits = 32;
    pfd.cStencilBits = 8;
    int fmt = ChoosePixelFormat(dc, &pfd);
    if (!fmt || !SetPixelFormat(dc, fmt, &pfd)) return 3;
    HGLRC rc = wglCreateContext(dc);
    if (!rc || !wglMakeCurrent(dc, rc)) return 4;
    HFONT font = CreateFontA(-12, 0, 0, 0, 400, 0, 0, 0, ANSI_CHARSET, 0, 0, NONANTIALIASED_QUALITY, 0, "Arial");
    if (!font || !SelectObject(dc, font)) return 5;
    if (scan || bounded) return scan_wide(dc, bounded);
    if (states) return font_modes(dc);
    if (bitmap_begin) return bitmap_primitives(dc);
    if (outline_begin) return outline_primitive(dc, atoi(argv[2]), atoi(argv[3]), atoi(argv[4]));
    static int codes[65536];
    int count = 0;
    if (reps) {
        FILE *source = NULL;
        if (fopen_s(&source, argv[2], "r") || !source) return 1;
        int code;
        while (fscanf_s(source, "%d", &code) == 1) {
            if (count >= 65536 || code < 0 || code > 65535) return 1;
            codes[count++] = code;
        }
        if (!feof(source) || fclose(source) || count == 0) return 1;
    } else {
        for (int code = first; code <= last; ++code) codes[count++] = code;
    }
    static GLfloat feedback[65536];
    for (int size = sample == 2 ? 2048 : (sample == 1 ? 4096 : 16); size <= (sample ? 16384 : 16); size *= 2) {
        glViewport(0, 0, size, size);
        GLint viewport[4];
        glGetIntegerv(GL_VIEWPORT, viewport);
        if (viewport[2] != size || viewport[3] != size) return 6;
        for (int index = 0; index < count; ++index) {
            int code = codes[index];
            for (int mode = 0; mode <= 1; ++mode) {
                for (int wide = reps; wide <= 1; ++wide) {
                    glMatrixMode(GL_MODELVIEW);
                    glLoadIdentity();
                    if (sample == 1) glScalef(0.5f, 0.5f, 1);
                    else if (!sample) {
                        glTranslatef(-0.75f, -0.75f, 0);
                        glScalef(2, 2, 1);
                        glClear(GL_COLOR_BUFFER_BIT);
                    }
                    glDeleteLists(100, 1);
                    GLYPHMETRICSFLOAT metrics = { 0 };
                    SetLastError(0);
                    const DWORD character = (DWORD)code + offset;
                    BOOL ok = wide ? wglUseFontOutlinesW(dc, character, 1, 100, 0, 0, mode, &metrics) :
                                     wglUseFontOutlinesA(dc, character, 1, 100, 0, 0, mode, &metrics);
                    DWORD err = ok ? 0 : GetLastError();
                    printf("glyph %d %d %d %d %d %lu %.9g %.9g %.9g %.9g %.9g %.9g\n", size, code, mode, wide, ok,
                           err, metrics.gmfBlackBoxX, metrics.gmfBlackBoxY, metrics.gmfptGlyphOrigin.x,
                           metrics.gmfptGlyphOrigin.y, metrics.gmfCellIncX, metrics.gmfCellIncY);
                    if (!ok) {
                        printf("err %d %u\n", glIsList(100), glGetError());
                        continue;
                    }
                    if (sample) {
                        glFeedbackBuffer((GLsizei)(sizeof feedback / sizeof *feedback), GL_2D, feedback);
                        glRenderMode(GL_FEEDBACK);
                        glCallList(100);
                        GLint n = glRenderMode(GL_RENDER);
                        if (n < 0 || n > (GLint)(sizeof feedback / sizeof *feedback) || glGetError()) return 8;
                        printf("data %d", n);
                        for (int i = 0; i < n; ++i) printf(" %08lx", fbits(feedback[i]));
                    } else {
                        GLubyte pixels[16 * 16 * 4] = { 0 };
                        glCallList(100);
                        glReadPixels(0, 0, 16, 16, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
                        if (glGetError()) return 9;
                        printf("pixels ");
                        for (size_t i = 0; i < sizeof pixels; ++i) printf("%02x", (unsigned)pixels[i]);
                    }
                    putchar('\n');
                }
            }
        }
    }
    puts("done");
    return 0;
}
