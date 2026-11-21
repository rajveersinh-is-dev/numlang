/* Extended workload 2: sliding-window sensor statistics (C reference). */
#include <stdint.h>
#include <stdio.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int main(void) {
#ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    const int64_t control[3] = {6, 24, 3};
    int64_t window = control[0];
    int64_t n = control[1];
    int64_t stride = control[2];
    const int64_t samples[24] = {
        4, 9, 2, 17, 6, 11, 3, 20, 8, 14, 1, 7, 19, 5, 12, 10, 16, 2, 13, 6, 18, 4, 9, 15
    };

    int64_t acc = 0;
    int64_t start = 0;
    while (start + window <= n) {
        int64_t sum = 0;
        int64_t lo = samples[start];
        int64_t hi = lo;
        int64_t j = 0;
        while (j < window) {
            int64_t v = samples[start + j];
            sum += v;
            if (v < lo) lo = v;
            if (v > hi) hi = v;
            j += 1;
        }
        int64_t score = hi - lo + sum % 7;
        if (start % stride == 0) {
            acc += score;
        } else {
            acc += score / 2;
        }
        start += 1;
    }

    printf("%lld\n", (long long)acc);
#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart));
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec)));
#endif
    return (int)(acc % 256);
}
