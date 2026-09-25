#include <stdio.h>
#include <stdint.h>

int64_t fib_coupled(int64_t n) {
    int64_t a = 0;
    int64_t b = 1;
    int64_t i = 0;
    while (i < n) {
        int64_t next_a = b;
        int64_t next_b = (a + b) % 1000000007LL;
        a = next_a;
        b = next_b;
        i++;
    }
    return a;
}

int main(void) {
    int64_t sum = 0;
    for (int i = 0; i < 1000; i++) {
        sum = (sum + fib_coupled(40)) % 1000000007LL;
    }
    printf("%lld\n", (long long)sum);
    return (int)(sum % 256);
}
