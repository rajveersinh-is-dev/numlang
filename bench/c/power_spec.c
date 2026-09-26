#include <stdio.h>
#include <stdint.h>
#include <windows.h>

int64_t power(int64_t x, int64_t n) {
    if (n <= 0) return 1;
    return x * power(x, n - 1);
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 1000; i++) {
        sum = (sum + power(i % 10, 8)) % 1000000007;
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
