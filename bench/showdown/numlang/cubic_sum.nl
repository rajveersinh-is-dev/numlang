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
    return cubic_sum(10000000) % 256;
}
