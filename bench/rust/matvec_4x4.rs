fn main() {
    let t0 = std::time::Instant::now();
    let m = [
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 1, 2, 3],
        [4, 5, 6, 7],
    ];
    let mut v = [2, 3, 5, 7];
    let mut sum: i64 = 0;

    for _ in 0..1000 {
        let mut res = [0i64; 4];
        for r in 0..4 {
            for c in 0..4 {
                res[r] += m[r][c] * v[c];
            }
        }
        sum += res[0] + res[1] + res[2] + res[3];
        v[0] = (v[0] + 1) % 10;
    }
    let elapsed = t0.elapsed();
    println!("{}", sum);
    println!("COMPUTE_NS: {}", elapsed.as_nanos());
    std::process::exit((sum % 256) as i32);
}
