#include <windows.h>

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
