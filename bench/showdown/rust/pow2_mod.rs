fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc.wrapping_mul(2);
        i += 1;
    }
    acc % 256
}

fn main() {
    let t0 = std::time::Instant::now();
    let r = pow2(100);
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(r as i32);
}
