#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int64_t mut_x(int64_t n, int64_t x, int64_t y);
int64_t mut_y(int64_t n, int64_t x, int64_t y);

int64_t mut_x(int64_t n, int64_t x, int64_t y) {
    if (n == 0) return x;
    return mut_y(n - 1, x + y, x);
}

int64_t mut_y(int64_t n, int64_t x, int64_t y) {
    if (n == 0) return y;
    return mut_x(n - 1, y, x + y);
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
    for (int i = 0; i < 10000; i++) {
        sum += mut_x(6, 1, 2);
    }

#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
