fn dot3(x1: i64, y1: i64, z1: i64, x2: i64, y2: i64, z2: i64) -> i64 {
    return x1 * x2 + y1 * y2 + z1 * z2;
}

fn intersect_sphere(ox: i64, oy: i64, oz: i64, dx: i64, dy: i64, dz: i64, cx: i64, cy: i64, cz: i64, r: i64) -> i64 {
    let oc_x: i64 = ox - cx;
    let oc_y: i64 = oy - cy;
    let oc_z: i64 = oz - cz;

    let a: i64 = dot3(dx, dy, dz, dx, dy, dz);
    let b: i64 = 2 * dot3(oc_x, oc_y, oc_z, dx, dy, dz);
    let c: i64 = dot3(oc_x, oc_y, oc_z, oc_x, oc_y, oc_z) - r * r;

    let disc: i64 = b * b - 4 * a * c;
    if disc >= 0 {
        return 1;
    } else {
        return 0;
    }
}

fn main() -> i64 {
    let mut hits: i64 = 0;
    for x in 0..50 {
        for y in 0..50 {
            let hit: i64 = intersect_sphere(0, 0, 0, x - 25, y - 25, 50, 0, 0, 50, 10);
            hits = hits + hit;
        }
    }
    println(hits);
    return hits % 256;
}
