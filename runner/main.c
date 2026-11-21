/* kikizake executor: interprets a case file (SPEC.md) against the Windows graphics APIs of
 * whatever system runs it (Windows: the reference; Wine, sake, ...: under test) and prints the
 * results, one fact per line. The cases are compiled in (build.sh: KIKIZAKE_CASES, a header made
 * from cases/<suite>.txt), so the exe takes no arguments and needs no files: it runs the same
 * under a native console, Wine, sake's node build or sake in a browser page. An argument names a
 * case file to run instead (ad-hoc runs of freshly generated cases).
 *
 * Output goes through WriteFile on the standard output only (a C runtime's stdio is not what this
 * tests) and is flushed after every case, so a crash keeps every finished case.
 *
 * Each API is a module, runner/<api>.c, listed in runner/apis.h. */
#include "core.h"

#include KIKIZAKE_CASES /* static const char kikizake_cases[] */

#define API(n) extern const struct kz_api n##_api;
#include "apis.h"
#undef API
static const struct kz_api *apis[] = {
#define API(n) &n##_api,
#include "apis.h"
#undef API
};
#define NAPIS (int)(sizeof apis / sizeof *apis)

/* output */

static char obuf[1 << 16];
static int olen;
void oflush(void) {
    DWORD n;
    if (olen) WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), obuf, olen, &n, NULL);
    olen = 0;
}
void out(const char *fmt, ...) {
    va_list va;
    if (olen > (int)sizeof(obuf) - 8192) oflush();
    va_start(va, fmt);
    olen += vsprintf(obuf + olen, fmt, va);
    va_end(va);
}

/* ------------------------------------------------------------------------------------------ */
/* tokens of one line */

char *tok[MAXTOK];
int ntok, lineno;
int bad; /* the current line could not be run as written */

void out_hex(const unsigned char *p, int n) {
    for (int i = 0; i < n; i += 2048) {
        if (olen > (int)sizeof(obuf) - 8192) oflush();
        char *o = obuf + olen;
        for (int k = i; k < n && k < i + 2048; k++) o += sprintf(o, "%02x", p[k]);
        olen = o - obuf;
    }
    out("\n");
}

long num(int i) {
    if (i >= ntok) { bad = 1; return 0; }
    char *e;
    long v = (long)strtoul(tok[i], &e, 0);
    if (tok[i][0] == '-') v = strtol(tok[i], &e, 0);
    if (*e) bad = 1;
    return v;
}
/* "#RRGGBB" -> COLORREF */
COLORREF color(int i) {
    if (i >= ntok || tok[i][0] != '#') { bad = 1; return 0; }
    unsigned v = (unsigned)strtoul(tok[i] + 1, NULL, 16);
    return RGB(v >> 16 & 255, v >> 8 & 255, v & 255);
}
int is(int i, const char *s) { return i < ntok && !strcmp(tok[i], s); }


HWND kz_window(void) {
    static HWND w;
    if (!w) {
        WNDCLASSA wc = { 0, DefWindowProcA, 0, 0, GetModuleHandleA(NULL), 0, 0, 0, 0, "kikizake" };
        RegisterClassA(&wc);
        w = CreateWindowA("kikizake", "kikizake", WS_POPUP, 0, 0, 64, 64, 0, 0, wc.hInstance, 0);
    }
    return w;
}

/* ------------------------------------------------------------------------------------------ */

static void reset_all(void) {
    for (int a = 0; a < NAPIS; a++)
        if (apis[a]->reset) apis[a]->reset();
}

static void dispatch(void) {
    for (int a = 0; a < NAPIS; a++)
        for (const struct kz_op *o = apis[a]->ops; o->name; o++)
            if (is(0, o->name)) { o->run(); return; }
    bad = 1;
}

static void run(char *text) {
    int incase = 0;
    for (char *l = text, *next; l && *l; l = next) {
        next = strchr(l, '\n');
        if (next) *next++ = 0;
        lineno++;
        char *hash = strchr(l, '#');
        /* (a comment starts with "# " or is the whole line; "#RRGGBB" is a color) */
        if (hash && (hash[1] == ' ' || hash == l)) *hash = 0;
        ntok = 0;
        for (char *t = strtok(l, " \t\r"); t && ntok < MAXTOK; t = strtok(NULL, " \t\r")) tok[ntok++] = t;
        if (!ntok) continue;
        bad = 0;
        if (is(0, "case")) {
            if (incase) { reset_all(); oflush(); }
            out("case %s\n", ntok > 1 ? tok[1] : "?");
            /* (out at once: a stop inside the case shows which case it was, not the one before) */
            oflush();
            incase = 1;
        } else if (is(0, "end")) { reset_all(); oflush(); incase = 0; }
        else dispatch();
        if (bad) out("bad line %d\n", lineno);
    }
    reset_all();
    oflush();
}

int main(int argc, char **argv) {
    SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX);
    char *text;
    if (argc > 1) {
        HANDLE f = CreateFileA(argv[1], GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, 0, NULL);
        if (f == INVALID_HANDLE_VALUE) { out("cannot open %s\n", argv[1]); oflush(); return 2; }
        DWORD n = GetFileSize(f, NULL), got;
        text = malloc(n + 1);
        ReadFile(f, text, n, &got, NULL);
        text[got] = 0;
        CloseHandle(f);
    } else text = strdup(kikizake_cases);
    out("kikizake %s\n", KIKIZAKE_SUITE);
    run(text);
    out("done\n");
    oflush();
    return 0;
}
