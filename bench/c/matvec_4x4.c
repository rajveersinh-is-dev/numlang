#include <stdio.h>
#include <stdint.h>
#include <windows.h>

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t m[4][4] = {
        {1, 2, 3, 4},
        {5, 6, 7, 8},
        {9, 1, 2, 3},
        {4, 5, 6, 7}
    };
    int64_t v[4] = {2, 3, 5, 7};
    int64_t sum = 0;

    for (int i = 0; i < 1000; i++) {
        int64_t res[4] = {0, 0, 0, 0};
        for (int r = 0; r < 4; r++) {
            for (int c = 0; c < 4; c++) {
                res[r] += m[r][c] * v[c];
            }
        }
        sum += res[0] + res[1] + res[2] + res[3];
        v[0] = (v[0] + 1) % 10;
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
