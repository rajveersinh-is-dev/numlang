fn dot3(x1: i64, y1: i64, z1: i64, x2: i64, y2: i64, z2: i64) -> i64 {
    x1 * x2 + y1 * y2 + z1 * z2
}

fn intersect_sphere(ox: i64, oy: i64, oz: i64, dx: i64, dy: i64, dz: i64, cx: i64, cy: i64, cz: i64, r: i64) -> i64 {
    let oc_x = ox - cx;
    let oc_y = oy - cy;
    let oc_z = oz - cz;

    let a = dot3(dx, dy, dz, dx, dy, dz);
    let b = 2 * dot3(oc_x, oc_y, oc_z, dx, dy, dz);
    let c = dot3(oc_x, oc_y, oc_z, oc_x, oc_y, oc_z) - r * r;

    let disc = b * b - 4 * a * c;
    if disc >= 0 { 1 } else { 0 }
}

fn main() {
    let mut hits = 0;
    for x in 0..50 {
        for y in 0..50 {
            let hit = intersect_sphere(0, 0, 0, x - 25, y - 25, 50, 0, 0, 50, 10);
            hits += hit;
        }
    }
    println!("{}", hits);
    std::process::exit((hits % 256) as i32);
}
