fn newton_isqrt(n: i64) -> i64 {
    let mut x: i64 = 1000;
    let mut prev: i64 = 0;
    while x != prev {
        prev = x;
        x = (x + (n / x)) / 2;
    }
    return x;
}

fn main() -> i64 {
    // Integer square root of 2,000,000 to get sqrt(2) * 1000 = 1414
    let res: i64 = newton_isqrt(2000000);
    println(res);
    return res % 256;
}
