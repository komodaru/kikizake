/* Exercise the executor's submission barrier with controlled COM results.
 * A return from kz_d3d12_wait is the readback caller's permission to map. */
#include "../runner/d3d12.c"

static const char *scenario;
static UINT64 completed;

static HRESULT STDMETHODCALLTYPE signal_test(ID3D12CommandQueue *queue, ID3D12Fence *fence, UINT64 value) {
    return strcmp(scenario, "signal") == 0 ? E_FAIL : S_OK;
}
static UINT64 STDMETHODCALLTYPE completed_test(ID3D12Fence *fence) {
    return strcmp(scenario, "removed") == 0 ? ~(UINT64)0 : completed;
}
static HRESULT STDMETHODCALLTYPE register_test(ID3D12Fence *fence, UINT64 value, HANDLE event) {
    if (strcmp(scenario, "register") == 0) return E_FAIL;
    if (strcmp(scenario, "timeout") != 0) {
        if (strcmp(scenario, "early") != 0) completed = value;
        SetEvent(event);
    }
    return S_OK;
}

char *tok[MAXTOK];
int bad, ntok;
long num(int i) { abort(); }
int is(int i, const char *s) { abort(); }
void out_hex(const unsigned char *p, int n) { abort(); }
void out(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    vprintf(fmt, args);
    va_end(args);
}
void oflush(void) { fflush(stdout); }

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    scenario = argv[1];
    ID3D12CommandQueueVtbl queue_methods = { .Signal = signal_test };
    ID3D12FenceVtbl fence_methods = { .GetCompletedValue = completed_test, .SetEventOnCompletion = register_test };
    ID3D12CommandQueue queue = { &queue_methods };
    ID3D12Fence fence = { &fence_methods };
    queue12 = &queue;
    fence12 = &fence;
    event12 = CreateEventA(NULL, FALSE, FALSE, NULL);
    if (!event12) return 2;
    kz_d3d12_wait();
    puts("ready for readback");
    CloseHandle(event12);
    return 0;
}
