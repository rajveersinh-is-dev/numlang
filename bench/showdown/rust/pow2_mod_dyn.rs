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
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let t0 = std::time::Instant::now();
    let r = pow2(n);
    let elapsed = t0.elapsed();
    println!("{}", r);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(r as i32);
}
