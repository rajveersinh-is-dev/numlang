#include <stdio.h>
#include <stdint.h>

int is_prime(int64_t n) {
    if (n <= 1) return 0;
    int64_t d = 2;
    while (d * d <= n) {
        if ((n % d) == 0) return 0;
        d++;
    }
    return 1;
}

int64_t count_primes(int64_t limit) {
    int64_t sum = 0;
    for (int64_t i = 2; i <= limit; i++) {
        if (is_prime(i)) {
            sum += i;
        }
    }
    return sum;
}

int main(void) {
    int64_t total = 0;
    for (int rep = 0; rep < 100; rep++) {
        total += count_primes(200);
    }
    printf("%lld\n", (long long)total);
    return (int)(total % 256);
}
