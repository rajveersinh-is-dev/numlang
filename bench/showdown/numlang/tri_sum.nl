fn tri_sum(n: i64) -> i64 {
    let mut acc: i64 = 0;
    let mut i: i64 = 1;
    while i <= n {
        acc = acc + i;
        i = i + 1;
    }
    return acc;
}

fn main() -> i64 {
    return tri_sum(50000000) % 256;
}
