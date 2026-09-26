fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..10 {
        for i in 0..100000i64 {
            let v = i * 2;
            if v % 3 == 0 {
                sum += v + 1;
            }
        }
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
