#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int64_t add1(int64_t x) { return x + 1; }
int64_t mul2(int64_t x) { return x * 2; }
int64_t add3(int64_t x) { return x + 3; }
int64_t sub5(int64_t x) { return x - 5; }
int64_t add10(int64_t x) { return x + 10; }

int64_t run_chain(int64_t val) {
    return add1(mul2(add3(sub5(add10(val)))));
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
        sum += run_chain(i);
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
