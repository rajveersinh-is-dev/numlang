fn is_even(n: i64) -> bool {
    return (n % 2) == 0;
}

fn square(n: i64) -> i64 {
    return n * n;
}

fn stream_pipeline(n: i64) -> i64 {
    let mut total: i64 = 0;
    for i in 0..n {
        if is_even(i) {
            total = total + square(i);
        }
    }
    return total;
}

fn main() -> i64 {
    let mut grand_total: i64 = 0;
    for rep in 0..1000 {
        grand_total = grand_total + (stream_pipeline(50) % 10000);
    }
    println(grand_total);
    return grand_total % 256;
}
