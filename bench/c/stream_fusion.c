#include <stdio.h>
#include <stdint.h>

int is_even(int64_t n) {
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
    int64_t grand_total = 0;
    for (int rep = 0; rep < 1000; rep++) {
        grand_total += stream_pipeline(50) % 10000;
    }
    printf("%lld\n", (long long)grand_total);
    return (int)(grand_total % 256);
}
