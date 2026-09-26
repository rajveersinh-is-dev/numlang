#include <stdio.h>
#include <stdint.h>
#include <windows.h>

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int iter = 0; iter < 10; iter++) {
        for (int64_t i = 0; i < 100000; i++) {
            int64_t v = i * 2;
            if (v % 3 == 0) {
                sum += v + 1;
            }
        }
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
