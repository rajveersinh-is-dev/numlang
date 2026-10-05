fn mut_x(n: i64, x: i64, y: i64) -> i64 {
    if n == 0 {
        return x;
    }
    mut_y(n - 1, x + y, x)
}

fn mut_y(n: i64, x: i64, y: i64) -> i64 {
    if n == 0 {
        return y;
    }
    mut_x(n - 1, y, x + y)
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..10000 {
        sum += mut_x(6, 1, 2);
    }
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
