fn ack(m: i64, n: i64) -> i64 {
    if m == 0 {
        n + 1
    } else if n == 0 {
        ack(m - 1, 1)
    } else {
        ack(m - 1, ack(m, n - 1))
    }
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..50 {
        sum += ack(3, 4);
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
