fn a_elem(i: i64, j: i64) -> f64 {
    let denom_int: i64 = ((i + j) * (i + j + 1)) / 2 + i + 1;
    let denom: f64 = i64_to_f64(denom_int);
    return 1.0 / denom;
}

fn compute_spectral_norm(n: i64, iters: i64) -> i64 {
    // Vectors of size 5
    let mut u0: f64 = 1.0;
    let mut u1: f64 = 1.0;
    let mut u2: f64 = 1.0;
    let mut u3: f64 = 1.0;
    let mut u4: f64 = 1.0;

    let mut v0: f64 = 0.0;
    let mut v1: f64 = 0.0;
    let mut v2: f64 = 0.0;
    let mut v3: f64 = 0.0;
    let mut v4: f64 = 0.0;

    for it in 0..iters {
        // Multiply by A
        v0 = a_elem(0, 0) * u0 + a_elem(0, 1) * u1 + a_elem(0, 2) * u2 + a_elem(0, 3) * u3 + a_elem(0, 4) * u4;
        v1 = a_elem(1, 0) * u0 + a_elem(1, 1) * u1 + a_elem(1, 2) * u2 + a_elem(1, 3) * u3 + a_elem(1, 4) * u4;
        v2 = a_elem(2, 0) * u0 + a_elem(2, 1) * u1 + a_elem(2, 2) * u2 + a_elem(2, 3) * u3 + a_elem(2, 4) * u4;
        v3 = a_elem(3, 0) * u0 + a_elem(3, 1) * u1 + a_elem(3, 2) * u2 + a_elem(3, 3) * u3 + a_elem(3, 4) * u4;
        v4 = a_elem(4, 0) * u0 + a_elem(4, 1) * u1 + a_elem(4, 2) * u2 + a_elem(4, 3) * u3 + a_elem(4, 4) * u4;

        // Multiply by At
        u0 = a_elem(0, 0) * v0 + a_elem(1, 0) * v1 + a_elem(2, 0) * v2 + a_elem(3, 0) * v3 + a_elem(4, 0) * v4;
        u1 = a_elem(0, 1) * v0 + a_elem(1, 1) * v1 + a_elem(2, 1) * v2 + a_elem(3, 1) * v3 + a_elem(4, 1) * v4;
        u2 = a_elem(0, 2) * v0 + a_elem(1, 2) * v1 + a_elem(2, 2) * v2 + a_elem(3, 2) * v3 + a_elem(4, 2) * v4;
        u3 = a_elem(0, 3) * v0 + a_elem(1, 3) * v1 + a_elem(2, 3) * v2 + a_elem(3, 3) * v3 + a_elem(4, 3) * v4;
        u4 = a_elem(0, 4) * v0 + a_elem(1, 4) * v1 + a_elem(2, 4) * v2 + a_elem(3, 4) * v3 + a_elem(4, 4) * v4;
    }

    let vbv: f64 = u0 * v0 + u1 * v1 + u2 * v2 + u3 * v3 + u4 * v4;
    let vv: f64 = v0 * v0 + v1 * v1 + v2 * v2 + v3 * v3 + v4 * v4;
    let ratio: f64 = vbv / vv;
    let scaled: i64 = f64_to_i64(ratio * 1000000.0);
    return scaled;
}

fn main() -> i64 {
    let norm: i64 = compute_spectral_norm(5, 10);
    println(norm);
    return norm % 256;
}
