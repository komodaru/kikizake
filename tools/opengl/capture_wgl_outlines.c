/* Capture the runner's Arial font outlines in a generic 32-bit DIB context. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <GL/gl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned long fbits(GLfloat value) {
    unsigned long bits;
    memcpy(&bits, &value, sizeof bits);
    return bits;
}

int main(int argc, char **argv) {
    if (argc != 4 && argc != 5) return 1;
    const int first = atoi(argv[1]);
    const int last = atoi(argv[2]);
    const int sample = atoi(argv[3]); /* 0: pixels, 1: half-scale feedback, 2: unscaled feedback */
    if (first < 0 || last < first || last > 255 || sample < 0 || sample > 2) return 1;
    const DWORD offset = argc == 5 ? strtoul(argv[4], NULL, 0) : 0;
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
    static GLfloat feedback[65536];
    for (int size = sample == 2 ? 2048 : (sample == 1 ? 4096 : 16); size <= (sample ? 16384 : 16); size *= 2) {
        glViewport(0, 0, size, size);
        GLint viewport[4];
        glGetIntegerv(GL_VIEWPORT, viewport);
        if (viewport[2] != size || viewport[3] != size) return 6;
        for (int code = first; code <= last; ++code) {
            for (int mode = 0; mode <= 1; ++mode) {
                for (int wide = 0; wide <= 1; ++wide) {
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
