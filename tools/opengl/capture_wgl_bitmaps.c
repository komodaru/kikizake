/* Capture the runner's Arial bitmap lists in a generic 32-bit DIB context. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <GL/gl.h>
#include <stdio.h>
#include <stdint.h>
#include <math.h>
#include <string.h>

static unsigned long fbits(GLfloat value) {
    unsigned long bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

static HDC make_context(int side) {
    HDC dc = CreateCompatibleDC(NULL);
    BITMAPINFO info = { 0 };
    info.bmiHeader.biSize = sizeof(info.bmiHeader);
    info.bmiHeader.biWidth = side;
    info.bmiHeader.biHeight = side;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    void *bits = NULL;
    HBITMAP bmp = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &bits, NULL, 0);
    if (!dc || !bmp || !SelectObject(dc, bmp)) return NULL;
    PIXELFORMATDESCRIPTOR pfd = { 0 };
    pfd.nSize = sizeof(pfd);
    pfd.nVersion = 1;
    pfd.dwFlags = PFD_SUPPORT_GDI | PFD_SUPPORT_OPENGL | PFD_DRAW_TO_BITMAP;
    pfd.iPixelType = PFD_TYPE_RGBA;
    pfd.cColorBits = 32;
    pfd.cDepthBits = 32;
    pfd.cStencilBits = 8;
    int fmt = ChoosePixelFormat(dc, &pfd);
    PIXELFORMATDESCRIPTOR selected;
    if (!fmt || !DescribePixelFormat(dc, fmt, sizeof(selected), &selected) ||
        !(selected.dwFlags & PFD_GENERIC_FORMAT) || !SetPixelFormat(dc, fmt, &pfd)) return NULL;
    HGLRC rc = wglCreateContext(dc);
    if (!rc || !wglMakeCurrent(dc, rc)) return NULL;
    HFONT font = CreateFontA(-12, 0, 0, 0, 400, 0, 0, 0, ANSI_CHARSET, 0, 0, NONANTIALIASED_QUALITY, 0, "Arial");
    char face[LF_FACESIZE];
    if (!font || !SelectObject(dc, font) ||
        !GetTextFaceA(dc, sizeof(face), face) || strcmp(face, "Arial")) return NULL;

    glViewport(0, 0, side, side);
    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(0, side, 0, side, -1, 1);
    glMatrixMode(GL_MODELVIEW);
    glColor4ub(255, 255, 255, 255);
    glDisable(GL_DITHER);
    return dc;
}

static void record_atlas(HDC dc) {
    enum { SIDE = 32, POSITION = 16 };
    for (unsigned long code = 0; code <= 65535; ++code) {
        GLubyte pixels[SIDE * SIDE * 4];
        GLfloat before[4], after[4];
        glDeleteLists(100, 1);
        SetLastError(0);
        int ok = wglUseFontBitmapsW(dc, code, 1, 100);
        unsigned long error = ok ? 0 : GetLastError();
        int defined = glIsList(100);
        glClear(GL_COLOR_BUFFER_BIT);
        glRasterPos2i(POSITION, POSITION);
        glGetFloatv(GL_CURRENT_RASTER_POSITION, before);
        glCallList(100);
        glGetFloatv(GL_CURRENT_RASTER_POSITION, after);
        glReadPixels(0, 0, SIDE, SIDE, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
        unsigned glerr = glGetError();

        int min_x = SIDE, min_y = SIDE, max_x = -1, max_y = -1;
        for (int y = 0; y < SIDE; ++y) {
            for (int x = 0; x < SIDE; ++x) {
                if (!pixels[4 * (y * SIDE + x)]) continue;
                if (x < min_x) min_x = x;
                if (x > max_x) max_x = x;
                if (y < min_y) min_y = y;
                if (y > max_y) max_y = y;
            }
        }
        printf("%04lx | %d %lu %d %x | ", code, ok, error, defined, glerr);
        if (max_x < 0) {
            printf("0 0 0 0 %.9g %.9g null\n", after[0] - before[0], after[1] - before[1]);
        } else {
            int w = max_x + 1 - min_x, h = max_y + 1 - min_y;
            printf("%d %d %d %d %.9g %.9g ub", w, h, (int)floorf(before[0]) - min_x, (int)floorf(before[1]) - min_y,
                   after[0] - before[0], after[1] - before[1]);
            for (int y = min_y; y <= max_y; ++y) {
                for (int first = min_x; first <= max_x; first += 8) {
                    unsigned packed = 0;
                    for (int bit = 0; bit < 8 && first + bit <= max_x; ++bit) {
                        if (pixels[4 * (y * SIDE + first + bit)]) packed |= 0x80 >> bit;
                    }
                    printf(" %u", packed);
                }
            }
            putchar('\n');
        }
    }
}

static uint64_t fnv_byte(uint64_t hash, uint8_t value) {
    return (hash ^ value) * UINT64_C(1099511628211);
}

static uint64_t fnv_word(uint64_t hash, uint32_t value) {
    for (int i = 0; i < 4; ++i) {
        hash = fnv_byte(hash, (uint8_t)value);
        value >>= 8;
    }
    return hash;
}

static void record_readback(HDC dc) {
    enum { SIDE = 16 };
    uint64_t hash = 0;
    for (DWORD code = 0; code <= 65535; ++code) {
        if (code % 256 == 0) hash = UINT64_C(14695981039346656037);
        glDeleteLists(100, 1);
        SetLastError(0);
        BOOL ok = wglUseFontBitmapsW(dc, code, 1, 100);
        DWORD error = ok ? 0 : GetLastError();
        hash = fnv_byte(hash, (uint8_t)ok);
        hash = fnv_word(hash, error);
        hash = fnv_byte(hash, glIsList(100));
        for (int pose = 0; pose < 2; ++pose) {
            GLubyte pixels[SIDE * SIDE * 4];
            GLfloat after[4];
            glClear(GL_COLOR_BUFFER_BIT);
            glRasterPos2i(pose ? 8 : 2, pose ? 5 : 4);
            glCallList(100);
            glGetFloatv(GL_CURRENT_RASTER_POSITION, after);
            glReadPixels(0, 0, SIDE, SIDE, GL_RGBA, GL_UNSIGNED_BYTE, pixels);
            for (size_t i = 0; i < sizeof after / sizeof after[0]; ++i) {
                hash = fnv_word(hash, fbits(after[i]));
            }
            for (size_t i = 0; i < sizeof pixels; ++i) hash = fnv_byte(hash, pixels[i]);
            hash = fnv_word(hash, glGetError());
        }
        if (code % 256 == 255) printf("%016llx\n", (unsigned long long)hash);
    }
}

static void record_ranges(HDC dc) {
    const unsigned long firsts[] = {0, 192, 193, 254, 255, 256, 0x10000, 0x100ff, 0xffbf,
                                    0xffc0, 0xffc1, 0xfffe, 0xffff, 0x10041, 0xffffffff, 0xffffffc0, 0xffffffc1};
    const unsigned long lengths[] = {0, 1, 2, 3, 63, 64};
    for (int wide = 0; wide <= 1; ++wide) {
        for (size_t f = 0; f < sizeof firsts / sizeof firsts[0]; ++f) {
            for (size_t c = 0; c < sizeof lengths / sizeof lengths[0]; ++c) {
                for (int pending = 0; pending <= 1; ++pending) {
                    const unsigned long first = firsts[f], count = lengths[c];
                    const unsigned long base = 100;
                    glDeleteLists(base, 64);
                    if (pending) glEnable(0xFFFF);
                    SetLastError(0);
                    BOOL ok = wide ? wglUseFontBitmapsW(dc, first, count, base)
                                   : wglUseFontBitmapsA(dc, first, count, base);
                    DWORD err = ok ? 0 : GetLastError();
                    int list_count = 0;
                    for (unsigned long i = 0; i < 64; ++i) { list_count += !!glIsList(base + i); }
                    printf("%d %08lx %lu %d %d %lu %d %x\n", wide, first, count, pending, ok, err, list_count, glGetError());
                }
            }
        }
    }
}

int main(int argc, char **argv) {
    if (argc != 2) return 1;
    int atlas = !strcmp(argv[1], "atlas");
    HDC dc = make_context(atlas ? 32 : 16);
    if (!dc) return 2;
    if (atlas) record_atlas(dc);
    else if (!strcmp(argv[1], "readback")) record_readback(dc);
    else if (!strcmp(argv[1], "ranges")) record_ranges(dc);
    else return 1;
    puts("done");
    return 0;
}
