#include <stdio.h>
#include <stdint.h>
#ifdef _WIN32
#include <windows.h>
#else
#define _POSIX_C_SOURCE 199309L
#include <time.h>
#endif

int intersect(int64_t ox, int64_t oy, int64_t oz, int64_t dx, int64_t dy, int64_t dz) {
    int64_t a = dx*dx + dy*dy + dz*dz;
    int64_t b = 2 * (ox*dx + oy*dy + oz*dz);
    int64_t c = ox*ox + oy*oy + oz*oz - 100;
    int64_t disc = b*b - 4*a*c;
    return disc >= 0;
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

    int64_t hits = 0;
    for (int y = -20; y <= 20; y++) {
        for (int x = -20; x <= 20; x++) {
            if (intersect(0, 0, -50, x, y, 50)) {
                hits++;
            }
        }
    }

    #ifdef _WIN32
    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);
#else
    clock_gettime(CLOCK_MONOTONIC, &t1);
    int64_t ns = (int64_t)(t1.tv_sec - t0.tv_sec) * 1000000000LL + (t1.tv_nsec - t0.tv_nsec);
#endif

    printf("%lld\n", (long long)hits);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(hits % 256);
}
