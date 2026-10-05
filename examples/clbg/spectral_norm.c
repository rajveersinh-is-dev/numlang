#include <stdio.h>
#include <stdint.h>

static double a_elem(int64_t i, int64_t j) {
    double denom = (double)(((i + j) * (i + j + 1)) / 2 + i + 1);
    return 1.0 / denom;
}

static int64_t compute_spectral_norm(int64_t n, int64_t iters) {
    double u0 = 1.0, u1 = 1.0, u2 = 1.0, u3 = 1.0, u4 = 1.0;
    double v0 = 0.0, v1 = 0.0, v2 = 0.0, v3 = 0.0, v4 = 0.0;

    for (int64_t it = 0; it < iters; it++) {
        v0 = a_elem(0, 0) * u0 + a_elem(0, 1) * u1 + a_elem(0, 2) * u2 + a_elem(0, 3) * u3 + a_elem(0, 4) * u4;
        v1 = a_elem(1, 0) * u0 + a_elem(1, 1) * u1 + a_elem(1, 2) * u2 + a_elem(1, 3) * u3 + a_elem(1, 4) * u4;
        v2 = a_elem(2, 0) * u0 + a_elem(2, 1) * u1 + a_elem(2, 2) * u2 + a_elem(2, 3) * u3 + a_elem(2, 4) * u4;
        v3 = a_elem(3, 0) * u0 + a_elem(3, 1) * u1 + a_elem(3, 2) * u2 + a_elem(3, 3) * u3 + a_elem(3, 4) * u4;
        v4 = a_elem(4, 0) * u0 + a_elem(4, 1) * u1 + a_elem(4, 2) * u2 + a_elem(4, 3) * u3 + a_elem(4, 4) * u4;

        u0 = a_elem(0, 0) * v0 + a_elem(1, 0) * v1 + a_elem(2, 0) * v2 + a_elem(3, 0) * v3 + a_elem(4, 0) * v4;
        u1 = a_elem(0, 1) * v0 + a_elem(1, 1) * v1 + a_elem(2, 1) * v2 + a_elem(3, 1) * v3 + a_elem(4, 1) * v4;
        u2 = a_elem(0, 2) * v0 + a_elem(1, 2) * v1 + a_elem(2, 2) * v2 + a_elem(3, 2) * v3 + a_elem(4, 2) * v4;
        u3 = a_elem(0, 3) * v0 + a_elem(1, 3) * v1 + a_elem(2, 3) * v2 + a_elem(3, 3) * v3 + a_elem(4, 3) * v4;
        u4 = a_elem(0, 4) * v0 + a_elem(1, 4) * v1 + a_elem(2, 4) * v2 + a_elem(3, 4) * v3 + a_elem(4, 4) * v4;
    }

    double vbv = u0 * v0 + u1 * v1 + u2 * v2 + u3 * v3 + u4 * v4;
    double vv = v0 * v0 + v1 * v1 + v2 * v2 + v3 * v3 + v4 * v4;
    double ratio = vbv / vv;
    int64_t scaled = (int64_t)(ratio * 1000000.0);
    return scaled;
}

int main(void) {
    int64_t norm = compute_spectral_norm(5, 10);
    printf("%lld\n", (long long)norm);
    return (int)(norm % 256);
}
