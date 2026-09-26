#include <stdio.h>
#include <stdint.h>
#include <windows.h>

void mat_mul(int64_t a[2][2], int64_t b[2][2], int64_t res[2][2]) {
    int64_t r00 = (a[0][0]*b[0][0] + a[0][1]*b[1][0]) % 1000000007;
    int64_t r01 = (a[0][0]*b[0][1] + a[0][1]*b[1][1]) % 1000000007;
    int64_t r10 = (a[1][0]*b[0][0] + a[1][1]*b[1][0]) % 1000000007;
    int64_t r11 = (a[1][0]*b[0][1] + a[1][1]*b[1][1]) % 1000000007;
    res[0][0] = r00; res[0][1] = r01;
    res[1][0] = r10; res[1][1] = r11;
}

int64_t fib(int64_t n) {
    if (n <= 0) return 0;
    int64_t result[2][2] = {{1, 0}, {0, 1}};
    int64_t base[2][2] = {{1, 1}, {1, 0}};
    while (n > 0) {
        if (n % 2 == 1) {
            mat_mul(result, base, result);
        }
        mat_mul(base, base, base);
        n /= 2;
    }
    return result[0][1];
}

int main(void) {
    LARGE_INTEGER freq, t0, t1;
    QueryPerformanceFrequency(&freq);
    QueryPerformanceCounter(&t0);

    int64_t sum = 0;
    for (int i = 0; i < 1000; i++) {
        sum = (sum + fib(50)) % 1000000007;
    }

    QueryPerformanceCounter(&t1);
    int64_t ns = (int64_t)((t1.QuadPart - t0.QuadPart) * 1000000000LL / freq.QuadPart);

    printf("%lld\n", (long long)sum);
    printf("COMPUTE_NS: %lld\n", (long long)ns);
    return (int)(sum % 256);
}
