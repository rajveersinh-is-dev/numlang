#include <stdio.h>
#include <stdint.h>

int64_t ack(int64_t m, int64_t n) {
    if (m == 0) {
        return n + 1;
    } else if (n == 0) {
        return ack(m - 1, 1);
    } else {
        return ack(m - 1, ack(m, n - 1));
    }
}

int main(void) {
    int64_t sum = 0;
    for (int i = 0; i < 50; i++) {
        sum += ack(3, 4);
    }
    printf("%lld\n", (long long)sum);
    return (int)(sum % 256);
}
