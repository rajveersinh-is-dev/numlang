fn add1(x: i64) -> i64 { x + 1 }
fn mul2(x: i64) -> i64 { x * 2 }
fn add3(x: i64) -> i64 { x + 3 }
fn sub5(x: i64) -> i64 { x - 5 }
fn add10(x: i64) -> i64 { x + 10 }

fn run_chain(val: i64) -> i64 {
    add1(mul2(add3(sub5(add10(val)))))
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for i in 0..1000 {
        sum += run_chain(i);
    }
    let elapsed = t0.elapsed();
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
