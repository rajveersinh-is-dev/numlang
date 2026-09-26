fn power(x: i64, n: i64) -> i64 {
    if n <= 0 {
        1
    } else {
        x * power(x, n - 1)
    }
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for i in 0..1000 {
        sum = (sum + power(i % 10, 8)) % 1000000007;
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
