const N: usize = 10000;

fn main() {
    let t0 = std::time::Instant::now();
    let mut count: i64 = 0;
    for _ in 0..10 {
        let mut prime = vec![true; N + 1];
        prime[0] = false;
        prime[1] = false;
        let mut p = 2;
        while p * p <= N {
            if prime[p] {
                let mut i = p * p;
                while i <= N {
                    prime[i] = false;
                    i += p;
                }
            }
            p += 1;
        }
        count = prime.iter().filter(|&&b| b).count() as i64;
    }
    let elapsed = t0.elapsed();
    println!("{}", count);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((count % 256) as i32);
}
