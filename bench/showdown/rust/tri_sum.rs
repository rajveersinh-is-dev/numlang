fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc.wrapping_add(i);
        i += 1;
    }
    acc
}

fn main() {
    let t0 = std::time::Instant::now();
    let r = tri_sum(50000000) % 256;
    let r_norm = (r % 256 + 256) % 256;
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(r_norm as i32);
}
