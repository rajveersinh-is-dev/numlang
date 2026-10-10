#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int64_t tri_sum(int64_t n) {
    int64_t acc = 0;
    int64_t i = 1;
    while (i <= n) {
        acc += i;
        i++;
    }
    return acc;
}

int main(int argc, char** argv) {
    int64_t n = (argc > 1) ? atoll(argv[1]) : 50000000LL;
#ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    int64_t full_res = tri_sum(n);
    int64_t r = full_res % 256;
    if (r < 0) r += 256;

#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("%lld\n", (long long)full_res);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)r;
}
