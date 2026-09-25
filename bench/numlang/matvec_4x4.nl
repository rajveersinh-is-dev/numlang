fn dot4(m0: i64, m1: i64, m2: i64, m3: i64, v0: i64, v1: i64, v2: i64, v3: i64) -> i64 {
    return m0 * v0 + m1 * v1 + m2 * v2 + m3 * v3;
}

fn matvec_step(v0: i64, v1: i64, v2: i64, v3: i64) -> i64 {
    let r0: i64 = dot4(2, 1, -1, 0, v0, v1, v2, v3);
    let r1: i64 = dot4(-1, 3, 0, 2, v0, v1, v2, v3);
    let r2: i64 = dot4(0, -2, 4, 1, v0, v1, v2, v3);
    let r3: i64 = dot4(1, 0, 1, 3, v0, v1, v2, v3);
    return (r0 + r1 + r2 + r3) % 1000;
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..1000 {
        let val: i64 = matvec_step(i % 10, (i + 1) % 10, (i + 2) % 10, (i + 3) % 10);
        sum = sum + val;
    }
    println(sum);
    return sum % 256;
}
