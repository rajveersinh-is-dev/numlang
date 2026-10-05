#include <stdio.h>
#include <stdint.h>

static int64_t count_flips(int64_t p0, int64_t p1, int64_t p2, int64_t p3, int64_t p4) {
    int64_t q0 = p0;
    int64_t q1 = p1;
    int64_t q2 = p2;
    int64_t q3 = p3;
    int64_t q4 = p4;
    int64_t flips = 0;

    while (q0 != 1) {
        if (q0 == 2) {
            int64_t t = q0;
            q0 = q1;
            q1 = t;
        } else if (q0 == 3) {
            int64_t t = q0;
            q0 = q2;
            q2 = t;
        } else if (q0 == 4) {
            int64_t t = q0;
            q0 = q3;
            q3 = t;
            int64_t t2 = q1;
            q1 = q2;
            q2 = t2;
        } else if (q0 == 5) {
            int64_t t = q0;
            q0 = q4;
            q4 = t;
            int64_t t2 = q1;
            q1 = q3;
            q3 = t2;
        }
        flips++;
    }
    return flips;
}

int main(void) {
    int64_t max_flips = 0;
    int64_t total_flips = 0;

    for (int64_t i0 = 1; i0 <= 5; i0++) {
        for (int64_t i1 = 1; i1 <= 5; i1++) {
            if (i1 != i0) {
                for (int64_t i2 = 1; i2 <= 5; i2++) {
                    if (i2 != i0 && i2 != i1) {
                        for (int64_t i3 = 1; i3 <= 5; i3++) {
                            if (i3 != i0 && i3 != i1 && i3 != i2) {
                                for (int64_t i4 = 1; i4 <= 5; i4++) {
                                    if (i4 != i0 && i4 != i1 && i4 != i2 && i4 != i3) {
                                        int64_t f = count_flips(i0, i1, i2, i3, i4);
                                        if (f > max_flips) {
                                            max_flips = f;
                                        }
                                        total_flips += f;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    printf("%lld\n", (long long)max_flips);
    printf("%lld\n", (long long)total_flips);
    return (int)((max_flips + total_flips) % 256);
}
