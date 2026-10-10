fn pow2(n: i64) -> i64 {
    let mut acc: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        acc = acc * 2;
        i = i + 1;
    }
    return acc % 256;
}

fn main() -> i64 {
    let mut n: i64 = read_i64();
    if n <= 0 {
        n = 100;
    }
    let res: i64 = pow2(n);
    println(res);
    return res;
}
