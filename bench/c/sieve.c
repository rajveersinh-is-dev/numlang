#include <stdio.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

#define N 10000
int prime[N + 1];

int main(void) {
    #ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    int64_t count = 0;
    for (int iter = 0; iter < 10; iter++) {
        for (int i = 0; i <= N; i++) prime[i] = 1;
        prime[0] = prime[1] = 0;
        for (int p = 2; p * p <= N; p++) {
            if (prime[p]) {
                for (int i = p * p; i <= N; i += p) prime[i] = 0;
            }
        }
        count = 0;
        for (int i = 2; i <= N; i++) if (prime[i]) count++;
    }

    #ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("%lld\n", (long long)count);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(count % 256);
}
