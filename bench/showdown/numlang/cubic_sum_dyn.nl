fn cubic_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + (i * i);
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    let mut n: i64 = read_i64();
    if n <= 0 {
        n = 10000000;
    }
    let res: i64 = cubic_sum(n);
    println(res);
    return res % 256;
}
