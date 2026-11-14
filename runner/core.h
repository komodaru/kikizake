/* kikizake executor: what every API module shares (runner/README.md). A module is runner/<api>.c:
 * its ops (KZ_OPS), a reset run after every case, and nothing else visible. */
#define COBJMACROS
#define __USE_MINGW_ANSI_STDIO 0
#include <windows.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* output: one fact per line, WriteFile on the standard output, flushed after every case */
void out(const char *fmt, ...);
void oflush(void);
void out_hex(const unsigned char *p, int n); /* n bytes as hex, then a newline */

/* the tokens of the current line */
#define MAXTOK 80
extern char *tok[MAXTOK];
extern int ntok, lineno;
extern int bad; /* set by an op: the current line could not be run as written ("bad line N") */
long num(int i);        /* token i as a number (0x.. hex, -..), else bad */
COLORREF color(int i);  /* "#RRGGBB" */
int is(int i, const char *s);

/* a 64x64 popup window, created once (devices and cooperative levels want one) */
HWND kz_window(void);

/* a module's op table: the line's first token -> its function, ended by { 0 } */
struct kz_op { const char *name; void (*run)(void); };
struct kz_api { const char *name; const struct kz_op *ops; void (*reset)(void); };
