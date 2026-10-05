fn square(x: i64) -> i64 {
    return x * x;
}

fn main() -> i64 {
    let mut total: i64 = 0;
    for i in 1..=1000000 {
        total = total + square(i);
    }
    return total % 256;
}
