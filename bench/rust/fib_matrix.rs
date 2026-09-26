fn mat_mul(a: [[i64; 2]; 2], b: [[i64; 2]; 2]) -> [[i64; 2]; 2] {
    [
        [
            (a[0][0]*b[0][0] + a[0][1]*b[1][0]) % 1000000007,
            (a[0][0]*b[0][1] + a[0][1]*b[1][1]) % 1000000007,
        ],
        [
            (a[1][0]*b[0][0] + a[1][1]*b[1][0]) % 1000000007,
            (a[1][0]*b[0][1] + a[1][1]*b[1][1]) % 1000000007,
        ],
    ]
}

fn fib(mut n: i64) -> i64 {
    if n <= 0 { return 0; }
    let mut result = [[1, 0], [0, 1]];
    let mut base = [[1, 1], [1, 0]];
    while n > 0 {
        if n % 2 == 1 {
            result = mat_mul(result, base);
        }
        base = mat_mul(base, base);
        n /= 2;
    }
    result[0][1]
}

fn main() {
    let t0 = std::time::Instant::now();
    let mut sum: i64 = 0;
    for _ in 0..1000 {
        sum = (sum + fib(50)) % 1000000007;
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
