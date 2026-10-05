fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t = a.wrapping_add(b);
        a = b;
        b = t;
        i += 1;
    }
    a
}

fn main() {
    let t0 = std::time::Instant::now();
    let r = fib(1000000) % 256;
    let r_norm = (r % 256 + 256) % 256;
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(r_norm as i32);
}
