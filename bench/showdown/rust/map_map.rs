fn inc(x: i64) -> i64 {
    x + 1
}

fn double_val(x: i64) -> i64 {
    x * 2
}

fn main() {
    let t0 = std::time::Instant::now();
    let xs: [i64; 20] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
        10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
    ];
    let mut sum: i64 = 0;
    for x in &xs {
        let v1 = inc(*x);
        let v2 = double_val(v1);
        sum += v2;
    }
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
