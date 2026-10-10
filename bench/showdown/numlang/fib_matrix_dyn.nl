fn fib_coupled(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next_a: i64 = b;
        let next_b: i64 = (a + b) % 1000000007;
        a = next_a;
        b = next_b;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    let mut iters: i64 = read_i64();
    if iters <= 0 {
        iters = 1000;
    }
    let f40: i64 = fib_coupled(40);
    let mut sum: i64 = 0;
    let mut j: i64 = 0;
    while j < iters {
        sum = (sum + f40) % 1000000007;
        j = j + 1;
    }
    println(sum);
    return sum % 256;
}
