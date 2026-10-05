#include <stdio.h>
#include <stdint.h>

static int64_t compute_pi_digits(int64_t n) {
    int64_t sum = 0;
    int64_t scale = 100000000LL;
    for (int64_t k = 0; k < n; k++) {
        int64_t denom = 2 * k + 1;
        int64_t term = (4 * scale) / denom;
        if ((k % 2) == 0) {
            sum += term;
        } else {
            sum -= term;
        }
    }
    return sum / 10000;
}

int main(void) {
    int64_t pi_val = compute_pi_digits(100);
    printf("%lld\n", (long long)pi_val);
    return (int)(pi_val % 256);
}
