fn sum_of_squares(n: i64) -> i64 {
    let mut sum: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        sum = sum + i * i;
        i = i + 1;
    }
    return sum;
}

fn main() -> i64 {
    let res: i64 = sum_of_squares(100);
    println(res);
    return 0;
}
