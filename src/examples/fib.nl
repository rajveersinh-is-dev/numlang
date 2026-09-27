fn fib(n: i64) -> i64 {
    let mut f: [i64; 2] = [0, 1];
    let mut i: i64 = 0;
    while i < n {
        let next: i64 = f[0] + f[1];
        f[0] = f[1];
        f[1] = next;
        i = i + 1;
    }
    return f[0];
}

fn main() -> i64 {
    let mut sum: i64 = 0;
    for i in 0..10 {
        sum = sum + fib(i);
    }
    return sum;
}
