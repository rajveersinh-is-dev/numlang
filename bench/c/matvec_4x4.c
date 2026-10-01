#include <stdio.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int64_t dot4(int64_t m0, int64_t m1, int64_t m2, int64_t m3, int64_t v0, int64_t v1, int64_t v2, int64_t v3) {
    return m0 * v0 + m1 * v1 + m2 * v2 + m3 * v3;
}

int64_t matvec_step(int64_t v0, int64_t v1, int64_t v2, int64_t v3) {
    int64_t r0 = dot4(2, 1, -1, 0, v0, v1, v2, v3);
    int64_t r1 = dot4(-1, 3, 0, 2, v0, v1, v2, v3);
    int64_t r2 = dot4(0, -2, 4, 1, v0, v1, v2, v3);
    int64_t r3 = dot4(1, 0, 1, 3, v0, v1, v2, v3);
    return (r0 + r1 + r2 + r3) % 1000;
}

int main(void) {
    #ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    int64_t sum = 0;
    for (int64_t i = 0; i < 1000; i++) {
        int64_t val = matvec_step(i % 10, (i + 1) % 10, (i + 2) % 10, (i + 3) % 10);
        sum += val;
    }

    #ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
