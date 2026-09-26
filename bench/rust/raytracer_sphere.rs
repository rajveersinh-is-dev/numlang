fn intersect(ox: i64, oy: i64, oz: i64, dx: i64, dy: i64, dz: i64) -> bool {
    let a = dx*dx + dy*dy + dz*dz;
    let b = 2 * (ox*dx + oy*dy + oz*dz);
    let c = ox*ox + oy*oy + oz*oz - 100;
    let disc = b*b - 4*a*c;
    disc >= 0
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut hits: i64 = 0;
    for y in -20..=20 {
        for x in -20..=20 {
            if intersect(0, 0, -50, x, y, 50) {
                hits += 1;
            }
        }
    }
    let elapsed = t0.elapsed();
    println!("{}", hits);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((hits % 256) as i32);
}
