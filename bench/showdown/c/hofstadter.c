#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int64_t hofstadter_m(int64_t limit) {
    int64_t f[101] = {0};
    int64_t m[101] = {0};
    f[0] = 1;
    m[0] = 0;

    for (int64_t i = 1; i <= limit; i++) {
        int64_t f_prev = f[i - 1];
        int64_t m_of_f = m[f_prev];
        f[i] = i - m_of_f;

        int64_t m_prev = m[i - 1];
        int64_t f_of_m = f[m_prev];
        m[i] = i - f_of_m;
    }
    return m[limit];
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

    int64_t res = hofstadter_m(100);

#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(res % 256);
}
