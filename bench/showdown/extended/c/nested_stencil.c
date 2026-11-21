/* Extended workload 4: multi-pass 2D stencil with clamped halo (C reference). */
#include <stdint.h>
#include <stdio.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

static int64_t clamp_index(int64_t v, int64_t hi) {
    if (v < 0) return 0;
    if (v > hi) return hi;
    return v;
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

    const int64_t control[4] = {3, 6, 1, 2};
    int64_t passes = control[0];
    int64_t dim = control[1];
    int64_t w0 = control[2];
    int64_t w1 = control[3];

    int64_t grid[36] = {
        0, 7, 14, 21, 28, 6, 13, 20, 0, 8, 16, 19, 26, 6, 15, 19, 28, 3,
        10, 20, 25, 6, 11, 16, 23, 5, 11, 17, 23, 0, 7, 14, 21, 28, 6, 13
    };
    int64_t out[36] = {0};

    for (int64_t p = 0; p < passes; p++) {
        for (int64_t r = 0; r < dim; r++) {
            for (int64_t c = 0; c < dim; c++) {
                int64_t acc = 0;
                for (int64_t k = 0; k < 9; k++) {
                    int64_t dr = k / 3 - 1;
                    int64_t dc = k % 3 - 1;
                    int64_t rr = clamp_index(r + dr, dim - 1);
                    int64_t cc = clamp_index(c + dc, dim - 1);
                    int64_t coeff = w1;
                    if (dr * dr + dc * dc == 0) {
                        coeff = 4;
                    } else if ((dr + dc + 4) % 2 == 0) {
                        coeff = w0;
                    }
                    acc += grid[rr * dim + cc] * coeff;
                }
                out[r * dim + c] = (acc + 8) / 10;
            }
        }
        for (int64_t i = 0; i < dim * dim; i++) {
            grid[i] = out[i];
        }
    }

    int64_t result = grid[2 * dim + 3];
    printf("%lld\n", (long long)result);
#ifdef _WIN32
    QueryPerformanceCounter(&t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart));
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    printf("COMPUTE_NS: %lld\n", (long long)((t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec)));
#endif
    return (int)(result % 256);
}
