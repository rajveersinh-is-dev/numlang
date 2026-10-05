fn mut_x(n: i64, x: i64, y: i64) -> i64 {
    if n == 0 {
        return x;
    }
    return mut_y(n - 1, x + y, x);
}

fn mut_y(n: i64, x: i64, y: i64) -> i64 {
    if n == 0 {
        return y;
    }
    return mut_x(n - 1, y, x + y);
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for _ in 0..10000 {
        sum = sum + mut_x(6, 1, 2);
    }
    return sum % 256;
}
