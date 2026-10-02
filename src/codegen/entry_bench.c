#ifdef _WIN32
#include <windows.h>
#include <stdint.h>
#include <intrin.h>

extern long long main(void);

void mainCRTStartup() {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
    long long ret = main();
    QueryPerformanceCounter(&t1);

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
#else
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <unistd.h>

extern long long numlang_main(void);

int main(void) {
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
    long long ret = numlang_main();
    clock_gettime(CLOCK_MONOTONIC, &t1);

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
