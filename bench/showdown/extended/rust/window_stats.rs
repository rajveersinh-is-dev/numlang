// Extended workload 2: sliding-window sensor statistics with dynamic window size.
// Reference implementation for bench/showdown/extended/numlang/window_stats.nl.
use std::time::Instant;

fn main() {
    let t0 = Instant::now();

    let control: [i64; 3] = [6, 24, 3];
    let window = control[0];
    let n = control[1];
    let stride = control[2];

    let samples: [i64; 24] = [
        4, 9, 2, 17, 6, 11, 3, 20, 8, 14, 1, 7, 19, 5, 12, 10, 16, 2, 13, 6, 18, 4, 9, 15,
    ];

    let mut acc: i64 = 0;
    let mut start: i64 = 0;
    while start + window <= n {
        let mut sum: i64 = 0;
        let mut lo = samples[start as usize];
        let mut hi = lo;
        let mut j: i64 = 0;
        while j < window {
            let v = samples[(start + j) as usize];
            sum = sum.wrapping_add(v);
            if v < lo {
                lo = v;
            }
            if v > hi {
                hi = v;
            }
            j += 1;
        }
        let score = hi - lo + sum.rem_euclid(7);
        if start % stride == 0 {
            acc = acc.wrapping_add(score);
        } else {
            acc = acc.wrapping_add(score / 2);
        }
        start += 1;
    }

    println!("{}", acc);
    println!("COMPUTE_NS: {}", t0.elapsed().as_nanos());
    std::process::exit(acc.rem_euclid(256) as i32);
}
