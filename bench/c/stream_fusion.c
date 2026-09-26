#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <windows.h>

bool is_even(int64_t n) {
    return (n % 2) == 0;
}

int64_t square(int64_t n) {
    return n * n;
}

int64_t stream_pipeline(int64_t n) {
    int64_t total = 0;
    for (int64_t i = 0; i < n; i++) {
        if (is_even(i)) {
            total += square(i);
        }
    }
    return total;
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t grand_total = 0;
    for (int rep = 0; rep < 1000; rep++) {
        grand_total += (stream_pipeline(50) % 10000);
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)grand_total);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(grand_total % 256);
}
