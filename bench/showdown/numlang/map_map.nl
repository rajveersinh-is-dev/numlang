fn inc(x: i64) -> i64 {
    return x + 1;
}

fn double(x: i64) -> i64 {
    return x * 2;
}

fn main() -> i64 {
    let xs: [i64; 20] = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
        10, 11, 12, 13, 14, 15, 16, 17, 18, 19
    ];
    let mut sum: i64 = 0;
    for i in 0..20 {
        let v1: i64 = inc(xs[i]);
        let v2: i64 = double(v1);
        sum = sum + v2;
    }
    println(sum);
    return sum % 256;
}
