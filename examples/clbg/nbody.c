#include <stdio.h>
#include <stdint.h>

static int64_t advance(int64_t steps) {
    double dt = 0.01;
    double x0 = 0.0, y0 = 0.0, z0 = 0.0;
    double vx0 = 0.0, vy0 = 0.0, vz0 = 0.0;
    double m0 = 1.0;

    double x1 = 4.841431;
    double y1 = -1.160320;
    double z1 = -0.103622;
    double vx1 = 0.001660;
    double vy1 = 0.007699;
    double vz1 = -0.000069;
    double m1 = 0.000954;

    for (int64_t s = 0; s < steps; s++) {
        double dx = x0 - x1;
        double dy = y0 - y1;
        double dz = z0 - z1;
        double d2 = dx * dx + dy * dy + dz * dz;
        double mag = dt / (d2 * 5.0);

        vx0 -= dx * m1 * mag;
        vy0 -= dy * m1 * mag;
        vz0 -= dz * m1 * mag;

        vx1 += dx * m0 * mag;
        vy1 += dy * m0 * mag;
        vz1 += dz * m0 * mag;

        x0 += dt * vx0;
        y0 += dt * vy0;
        z0 += dt * vz0;

        x1 += dt * vx1;
        y1 += dt * vy1;
        z1 += dt * vz1;
    }

    double final_pos_sum = (x0 + y0 + z0 + x1 + y1 + z1) * 1000.0;
    int64_t res = (int64_t)final_pos_sum;
    return res;
}

int main(void) {
    int64_t res = advance(5);
    printf("%lld\n", (long long)res);
    return (int)(res % 256);
}
