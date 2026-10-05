fn advance(steps: i64) -> i64 {
    let dt: f64 = 0.01;
    // Body 0 (Sun)
    let mut x0: f64 = 0.0;
    let mut y0: f64 = 0.0;
    let mut z0: f64 = 0.0;
    let mut vx0: f64 = 0.0;
    let mut vy0: f64 = 0.0;
    let mut vz0: f64 = 0.0;
    let m0: f64 = 1.0;

    // Body 1 (Jupiter)
    let mut x1: f64 = 4.841431;
    let mut y1: f64 = -1.160320;
    let mut z1: f64 = -0.103622;
    let mut vx1: f64 = 0.001660;
    let mut vy1: f64 = 0.007699;
    let mut vz1: f64 = -0.000069;
    let m1: f64 = 0.000954;

    for s in 0..steps {
        let dx: f64 = x0 - x1;
        let dy: f64 = y0 - y1;
        let dz: f64 = z0 - z1;
        let d2: f64 = dx * dx + dy * dy + dz * dz;
        let mag: f64 = dt / (d2 * 5.0);

        vx0 = vx0 - dx * m1 * mag;
        vy0 = vy0 - dy * m1 * mag;
        vz0 = vz0 - dz * m1 * mag;

        vx1 = vx1 + dx * m0 * mag;
        vy1 = vy1 + dy * m0 * mag;
        vz1 = vz1 + dz * m0 * mag;

        x0 = x0 + dt * vx0;
        y0 = y0 + dt * vy0;
        z0 = z0 + dt * vz0;

        x1 = x1 + dt * vx1;
        y1 = y1 + dt * vy1;
        z1 = z1 + dt * vz1;
    }

    let final_pos_sum: f64 = (x0 + y0 + z0 + x1 + y1 + z1) * 1000.0;
    let res: i64 = f64_to_i64(final_pos_sum);
    return res;
}

fn main() -> i64 {
    let res: i64 = advance(5);
    println(res);
    return res % 256;
}
