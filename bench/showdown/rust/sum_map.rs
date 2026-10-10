fn square(x: i64) -> i64 { x * x }

fn main() {
    let t0 = std::time::Instant::now();
    let mut total: i64 = 0;
    for i in 1..=1000000 {
        total = total.wrapping_add(square(i));
    }
    let elapsed = t0.elapsed();
    println!("{}", total);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit(((total % 256 + 256) % 256) as i32);
}
