/* Extended workload 3: struct/array record transform pipeline (C reference). */
#include <stdint.h>
#include <stdio.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

typedef struct {
    int64_t id, raw, weight, bucket;
} Reading;

int main(void) {
#ifdef _WIN32
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);
#else
    struct timespec t0, t1;
    clock_gettime(CLOCK_MONOTONIC, &t0);
#endif

    const Reading records[10] = {
        {0, 137, 3, 2}, {1, 89, 5, 1}, {2, 204, 2, 3}, {3, 45, 7, 0}, {4, 176, 4, 2},
        {5, 63, 6, 1}, {6, 151, 3, 3}, {7, 28, 8, 0}, {8, 199, 2, 2}, {9, 112, 5, 1}
    };
    int64_t norm[10];
    for (int i = 0; i < 10; i++) {
        norm[i] = (records[i].raw % 101) * records[i].weight;
    }

    int64_t buckets[4] = {0, 0, 0, 0};
    for (int i = 0; i < 10; i++) {
        int64_t b = records[i].id % 4;
        buckets[b] += norm[i] + records[i].bucket;
    }

    int64_t total = 0;
    for (int k = 0; k < 4; k++) {
        int64_t boost = k * 2;
        if (k == 0) boost = 3;
        total += buckets[k] * boost;
    }

    printf("%lld\n", (long long)total);
#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart));
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec)));
#endif
    return (int)(total % 256);
}
