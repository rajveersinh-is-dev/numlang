#ifdef _WIN32
#include <windows.h>
#include <stdint.h>
#include <intrin.h>

extern long long main(void);

static LARGE_INTEGER g_bench_stop;
static int g_bench_stopped = 0;

/* Called by generated print helpers so the timed region excludes I/O, matching
   the C/Rust/GHC harnesses which stop their clocks before printing. */
void __nl_bench_stop(void) {
    if (!g_bench_stopped) {
        QueryPerformanceCounter(&g_bench_stop);
        g_bench_stopped = 1;
    }
}

void mainCRTStartup() {
    LARGE_INTEGER freq, t0, t1;
    /* The stop hook touches g_bench_stopped/g_bench_stop, which live in the
       zero-initialized .data page.  Fault that page in before t0 so the
       demand-zero page fault is not charged to the timed region. */
    *(volatile int *)&g_bench_stopped = 0;
    *(volatile LARGE_INTEGER *)&g_bench_stop = g_bench_stop;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long ret = main();
    if (g_bench_stopped) {
        t1 = g_bench_stop;
    } else {
        QueryPerformanceCounter(&t1);
    }

    long long ns = (t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart;

    char buf[64];
    int len = 0;
    const char prefix[] = "COMPUTE_NS: ";
    for (int i = 0; prefix[i]; i++) buf[len++] = prefix[i];
    char digits[32];
    int dlen = 0;
    long long temp = ns;
    while (temp > 0) {
        digits[dlen++] = '0' + (temp % 10);
        temp /= 10;
    }
    if (dlen == 0) digits[dlen++] = '0';
    for (int i = dlen - 1; i >= 0; i--) buf[len++] = digits[i];
    buf[len++] = '\n';
    DWORD written;
    WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), buf, len, &written, NULL);
    ExitProcess((UINT)ret);
}

long long __nl_read_i64(void) {
    /* Read the authoritative Unicode command line straight from the PEB.
       GetCommandLineA() transcodes the whole command line on its first call
       (and GetCommandLineW() still crosses the Win32 import boundary); neither
       is needed here because every character this parser consumes (quote,
       blank, '-' and '0'..'9') has the same 16-bit code unit in Unicode and in
       the process ANSI code page.  PEB->ProcessParameters->CommandLine.Buffer
       is at [gs:0x60] -> +0x20 -> +0x78 on x86-64 Windows. */
#if defined(_M_X64) || defined(__x86_64__)
    unsigned char* peb = (unsigned char*)__readgsqword(0x60);
    unsigned char* params = peb ? *(unsigned char**)(peb + 0x20) : 0;
    const unsigned short* cmd = params ? *(const unsigned short**)(params + 0x78) : 0;
#else
    const unsigned short* cmd = (const unsigned short*)GetCommandLineW();
#endif
    if (cmd) {
        const unsigned short* p = cmd;
        if (*p == '"') {
            p++;
            while (*p && *p != '"') p++;
            if (*p == '"') p++;
        } else {
            while (*p && *p != ' ' && *p != '\t') p++;
        }
        while (*p == ' ' || *p == '\t') p++;
        if ((*p >= '0' && *p <= '9') || *p == '-') {
            long long v = 0;
            int neg = 0;
            if (*p == '-') { neg = 1; p++; }
            while (*p >= '0' && *p <= '9') {
                v = v * 10 + (*p - '0');
                p++;
            }
            return neg ? -v : v;
        }
    }
    return 0;
}
#else
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

extern long long numlang_main(void);

static struct timespec g_bench_stop;
static int g_bench_stopped = 0;

void __nl_bench_stop(void) {
    if (!g_bench_stopped) {
        clock_gettime(CLOCK_MONOTONIC, &g_bench_stop);
        g_bench_stopped = 1;
    }
}

long long __nl_read_i64(void) {
    char buf[64];
    ssize_t n = read(0, buf, sizeof(buf) - 1);
    if (n > 0) {
        buf[n] = '\0';
        return atoll(buf);
    }
    return 0;
}

int main(void) {
    struct timespec t0, t1;
    /* Fault in the zero-initialized page holding the stop-hook state before t0;
       otherwise the first read of g_bench_stopped in __nl_bench_stop charges a
       demand-zero page fault to the timed region. */
    *(volatile int *)&g_bench_stopped = 0;
    *(volatile struct timespec *)&g_bench_stop = g_bench_stop;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    long long ret = numlang_main();
    if (g_bench_stopped) {
        t1 = g_bench_stop;
    } else {
        clock_gettime(CLOCK_MONOTONIC, &t1);
    }

    long long ns = (t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
    char buf[64];
    int len = snprintf(buf, sizeof(buf), "COMPUTE_NS: %lld\n", (long long)ns);
    if (len > 0) {
        ssize_t _w = write(1, buf, (size_t)len);
        (void)_w;
    }
    exit((int)ret);
}
#endif
