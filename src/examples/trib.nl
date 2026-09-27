fn trib(n: i64) -> i64 {
    let mut a: i64 = 0;
    let mut b: i64 = 0;
    let mut c: i64 = 1;
    let mut i: i64 = 0;
    while i < n {
        let next: i64 = a + b + c;
        a = b;
        b = c;
        c = next;
        i = i + 1;
    }
    return a;
}

fn main() -> i64 {
    return 0;
}

fn run_trib(n: i64) -> i64 {
    return trib(n);
}
