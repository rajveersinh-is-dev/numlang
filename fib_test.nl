fn fib(n: i64) -> i64 {
    if n <= 1 {
        return n;
    } else {
        return fib(n - 1) + fib(n - 2);
    }
}
fn main() -> i64 {
    let res: i64 = fib(35);
    return res % 256;
}
