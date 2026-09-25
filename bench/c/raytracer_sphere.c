#include <stdio.h>
#include <stdint.h>

int64_t dot3(int64_t x1, int64_t y1, int64_t z1, int64_t x2, int64_t y2, int64_t z2) {
    return x1 * x2 + y1 * y2 + z1 * z2;
}

int64_t intersect_sphere(int64_t ox, int64_t oy, int64_t oz, int64_t dx, int64_t dy, int64_t dz, int64_t cx, int64_t cy, int64_t cz, int64_t r) {
    int64_t oc_x = ox - cx;
    int64_t oc_y = oy - cy;
    int64_t oc_z = oz - cz;

    int64_t a = dot3(dx, dy, dz, dx, dy, dz);
    int64_t b = 2 * dot3(oc_x, oc_y, oc_z, dx, dy, dz);
    int64_t c = dot3(oc_x, oc_y, oc_z, oc_x, oc_y, oc_z) - r * r;

    int64_t disc = b * b - 4 * a * c;
    if (disc >= 0) return 1;
    else return 0;
}

int main(void) {
    int64_t hits = 0;
    for (int64_t x = 0; x < 50; x++) {
        for (int64_t y = 0; y < 50; y++) {
            int64_t hit = intersect_sphere(0, 0, 0, x - 25, y - 25, 50, 0, 0, 50, 10);
            hits += hit;
        }
    }
    printf("%lld\n", (long long)hits);
    return (int)(hits % 256);
}
