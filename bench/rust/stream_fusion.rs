fn is_even(n: i64) -> bool {
    (n % 2) == 0
}

fn square(n: i64) -> i64 {
    n * n
}

fn stream_pipeline(n: i64) -> i64 {
    let mut total: i64 = 0;
    for i in 0..n {
        if is_even(i) {
            total += square(i);
        }
    }
    total
}

fn main() {
    let mut grand_total: i64 = 0;
    for _ in 0..1000 {
        grand_total += stream_pipeline(50) % 10000;
    }
    println!("{}", grand_total);
    std::process::exit((grand_total % 256) as i32);
}
