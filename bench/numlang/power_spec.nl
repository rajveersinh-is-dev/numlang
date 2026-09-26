fn power(x: i64, n: i64) -> i64 {
    if n <= 0 {
        return 1;
    } else {
        return x * power(x, n - 1);
    }
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..1000 {
        sum = (sum + power(i % 10, 8)) % 1000000007;
    }
    println(sum);
    return sum % 256;
}
