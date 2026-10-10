fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i.wrapping_mul(i));
        i += 1;
    }
    acc
}

fn main() {
    let n: i64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10000000);
    let t0 = std::time::Instant::now();
    let r = cubic_sum(n);
    let r_norm = (r % 256 + 256) % 256;
    let elapsed = t0.elapsed();
    println!("{}", r);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(r_norm as i32);
}
