fn fib_coupled(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next_a = b;
        let next_b = (a + b) % 1_000_000_007;
        a = next_a;
        b = next_b;
        i += 1;
    }
    a
}

fn main() {
    let mut sum: i64 = 0;
    for _ in 0..1000 {
        sum = (sum + fib_coupled(40)) % 1_000_000_007;
    }
    println!("{}", sum);
    std::process::exit((sum % 256) as i32);
}
