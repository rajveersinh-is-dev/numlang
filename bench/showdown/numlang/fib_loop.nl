fn fib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let t: i64 = a + b;
        a = b;
        b = t;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    return fib(1000000) % 256;
}
