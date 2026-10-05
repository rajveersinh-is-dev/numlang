#include <stdio.h>
#include <stdint.h>

static int64_t generate_fasta(int64_t n) {
    int64_t seed = 42;
    int64_t im = 139968;
    int64_t ia = 3877;
    int64_t ic = 29573;
    int64_t checksum = 0;

    for (int64_t i = 0; i < n; i++) {
        seed = (seed * ia + ic) % im;
        int64_t base_code;
        if (seed < 37791) {
            base_code = 65; // 'A'
        } else if (seed < 54587) {
            base_code = 67; // 'C'
        } else if (seed < 71383) {
            base_code = 71; // 'G'
        } else {
            base_code = 84; // 'T'
        }
        checksum = (checksum * 31 + base_code) % 1000000007;
    }

    return checksum;
}

int main(void) {
    int64_t cs = generate_fasta(1000);
    printf("%lld\n", (long long)cs);
    return (int)(cs % 256);
}
