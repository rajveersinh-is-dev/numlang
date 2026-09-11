
fn collatz_steps(limit: i64) -> i64 {
    let mut total_steps: i64 = 0;
    let mut n: i64 = 1;
    while n <= limit {
        let mut curr: i64 = n;
        while curr > 1 {
            if curr % 2 == 0 {
                curr /= 2;
            } else {
                curr = curr * 3 + 1;
            }
            total_steps += 1;
        }
        n += 1;
    }
    total_steps % 256
}
fn main() {
    let t0 = std::time::Instant::now();
    let res = collatz_steps(100000);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(res as i32);
}
